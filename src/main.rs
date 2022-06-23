mod config;
mod torznab;
mod torrent_client;

use config::{Config, TorrentMode};

use abstracttorrent::common::GetTorrentListParams;
use abstracttorrent::torrent::{TorrentUpload, TorrentState, TorrentInfo};
use lava_torrent::bencode::BencodeElem;
use tracing::{info, Level, debug, warn, error};

use std::ops::Deref;
use std::path::{Path, PathBuf};
use std::error::Error;
use std::vec;

use lava_torrent::torrent::v1::{Torrent, AnnounceList};

use crate::torznab::{GenericSearchParameters, SearchFunction};
use crate::torznab::search_parameters::{GenericSearchParametersBuilder, MovieSearchParametersBuilder};

use tokio::sync::RwLock;

use std::sync::Arc;

fn read_torrents(path: &Path) -> Result<Vec<PathBuf>, Box<dyn Error>> {
    let mut torrents = Vec::new();
    for entry in path.read_dir()? {
        let entry = entry?;
        let path = entry.path();
        if path.is_file() {
            let filename = path.file_name().unwrap().to_str().unwrap();
            if filename.ends_with(".torrent") {
                torrents.push(path);
            }
        } else {
            let mut inner = read_torrents(&path)?;
            torrents.append(&mut inner);
        }
    }

    return Ok(torrents);
}

#[tokio::main]
async fn main() {
    let subscriber = tracing_subscriber::fmt()
        .with_max_level(Level::DEBUG)
        .finish();

    tracing::subscriber::set_global_default(subscriber).expect("Failed to set global default log subscriber");

    // Get config and debug the torrents
    let config = Arc::new(Config::new());
    info!("Searching for torrents in: {}", config.torrents_path_str());

    // Get a torrent client from the config.
    let mut torrent_client = torrent_client::TorrentClient::from_config(&config);
    torrent_client.login(&config).await.unwrap();

    // Torrent client no longer needs to mut, so we can just create an `Arc` without a mutex.
    let torrent_client = Arc::new(torrent_client);

    let mut indexers = config.indexers.clone();

    // Create torznab clients for each indexer.
    for indexer in indexers.iter_mut() {
        indexer.create_client().await.unwrap();
    }

    // Log the trackers
    info!("Searching {} trackers: ", indexers.len());
    for indexer in indexers.iter() {
        info!("  {}: {}", indexer.name, indexer.url);
        debug!("    Can Search: {:?}", indexer.client.as_ref().unwrap().capabilities.searching_capabilities);
    }

    // Log the amount of torrents.
    let torrent_files = read_torrents(config.torrents_path()).unwrap();
    info!("Found {} torrent files...", torrent_files.len());

    // Convert the indexers to be async friendly.
    let mut indexers = indexers.iter()
        .map(|indexer| Arc::new(RwLock::new(indexer.clone())))
        .collect::<Vec<_>>();

    // Store async tasks to wait for them to finish
    let mut indexer_handles = vec![];

    info!("Parsing all torrent files...");

    let mut stop = stopwatch::Stopwatch::start_new();
    // Get the torrents and from the paths
    let mut torrents: Vec<Result<Torrent, lava_torrent::LavaTorrentError>> = torrent_files.iter()
        .map(|path| Torrent::read_from_file(path))
        .collect();
    stop.stop();
    info!("Took {} seconds to parse all torrents", stop.elapsed().as_secs());
    drop(stop);

    // Remove the torrents that failed to be read from the file, and
    // are not in the download client.

    // NOTE: It might be better to get all torrents on the client and check that the torrents are on the
    // client locally.
    torrents.retain(|torrent| {
        if let Ok(torrent) = torrent {
            let info = futures::executor::block_on(torrent_client.get_torrent_info(&torrent)).unwrap();
            info.is_some()
        } else {
            false
        }
    });
    // Unwrap the results, all errored ones were removed from the `.retain`
    let torrents: Vec<Torrent> = torrents.iter()
        .map(|res| res.as_ref().unwrap().clone())
        .collect();

    info!("Found {} torrents that are in the client and on the filesystem", torrents.len());

    for torrent in torrents {
        let torrent = Arc::new(torrent);

        for indexer in indexers.iter() {
            info!("Checking for \"{}\"", torrent.name);

            // Clone some `Arc`s for the new async task.
            let mut indexer = Arc::clone(indexer);
            let torrent = Arc::clone(&torrent);
            let torrent_client = Arc::clone(&torrent_client);
            let config = Arc::clone(&config);

            indexer_handles.push(tokio::spawn(async move {
                let lock = indexer.read().await;
                match &lock.client {
                    Some(client) => {
                        let generic = GenericSearchParametersBuilder::new()
                            .query(torrent.name.clone())
                            .build();
                        let results = client.search(SearchFunction::Search, generic).await.unwrap();

                        // The first result should be the correct one.
                        if let Some(result) = results.first() {
                            let found_torrent = result.download_torrent().await.unwrap();

                            // Check if we found the same torrent in its own indexer
                            if found_torrent.info_hash() == torrent.info_hash() {
                                debug!("Found same torrent in its own indexer, skipping...");
                                return;
                            }

                            if let Some(found_announces) = &found_torrent.announce_list {
                                // Some urls can be encoded so we need to decode to compare them.
                                let found_announces: Vec<Vec<String>> = found_announces.iter()
                                    .map(|a_list| 
                                        a_list.iter().map(|a| urlencoding::decode(a)
                                                .unwrap().to_string())
                                            .collect::<Vec<String>>())
                                    .collect();

                                // Get the trackers of the torrent from the download client.
                                let request_info = TorrentInfo::from_hash(torrent.info_hash());
                                let torrent_announces = torrent_client.get_torrent_trackers(&request_info).await.unwrap();
                                let torrent_announces: Vec<&String> = torrent_announces.iter().map(|t| &t.url).collect();

                                // Flatten the announce list to make them easier to search.
                                let found_announces: Vec<&String> = found_announces.iter()
                                    .flat_map(|array| array.iter())
                                    .collect();

                                // Check if the client has the trackers of the torrent already.
                                let mut client_has_trackers = found_announces.iter()
                                    .all(|tracker| torrent_announces.contains(tracker));

                                if !client_has_trackers {
                                    info!("Found a cross-seedable torrent for {}", found_torrent.name);

                                    match torrent_client.get_torrent_info(&torrent).await.unwrap() {
                                        Some(info) => {
                                            info!("Got info: {:?}", info);

                                            match info.state {
                                                TorrentState::Uploading | TorrentState::QueuedUploading => {
                                                    debug!("The torrent is being uploaded on the client");

                                                    //if config.add_trackers {
                                                    match config.torrent_mode {
                                                        TorrentMode::InjectTrackers => {
                                                            debug!("Can add trackers to the torrent");

                                                            if found_torrent.is_private() {
                                                                debug!("The found torrent is private, so we must remove the torrent and re-add it with the new trackers...");

                                                                match torrent_client.remove_torrent(&info, false).await {
                                                                    Ok(()) => {
                                                                        debug!("Re-uploading torrent to client...");

                                                                        info!("Found announces: {:?}", found_announces);

                                                                        // Combine both announces and deref the Strings by cloning them.
                                                                        let mut torrent_announces: Vec<String> = torrent_announces.into_iter()
                                                                            .chain(found_announces)
                                                                            .cloned()
                                                                            .collect();
                                                                        // Remove the [DHT], [PeX] and [LSD] announces from the list.
                                                                        // The client should handle those.
                                                                        torrent_announces.retain(|announce| !(announce.starts_with("** [") && announce.ends_with("] **")));

                                                                        info!("Old torrent: {:?}", torrent.announce_list);

                                                                        let mut torrent = (*torrent).clone();
                                                                        torrent.announce_list = Some(vec![torrent_announces]);
                                                                        if let Some(extra) = torrent.extra_info_fields.as_mut() {
                                                                            extra.insert(String::from("private"), BencodeElem::Integer(1));
                                                                        } else {
                                                                            let mut extra = std::collections::HashMap::new();
                                                                            extra.insert(String::from("private"), BencodeElem::Integer(1));
                                                                            torrent.extra_info_fields = Some(extra);
                                                                        }
                                                                        /* torrent.extra_info_fields.as_mut()
                                                                            .unwrap_or(&mut std::collections::HashMap::new())
                                                                            .insert(String::from("private"), BencodeElem::Integer(1)); */


                                                                        info!("Torrent that will be uploaded: {:?}, private: {}", torrent.announce_list, torrent.is_private());

                                                                        // Clone some fields from the torrent due to ownership issues with
                                                                        // torrent.encode()
                                                                        let name = torrent.name.clone();
                                                                        let hash = torrent.info_hash().clone();

                                                                        match torrent.encode() {
                                                                            Ok(bytes) => {
                                                                                let upload = TorrentUpload::builder()
                                                                                    .category(config.torrent_category())
                                                                                    .tags(info.tags)
                                                                                    .torrent_data(format!("{}.torrent", hash), bytes)
                                                                                    //.paused()
                                                                                    .build();

                                                                                match torrent_client.add_torrent(&upload).await {
                                                                                    Ok(()) => info!("Added cross-seed torrent {}!", name),
                                                                                    Err(err) => error!("Error adding cross-seed torrent: {} (error: {:?}", name, err),
                                                                                }
                                                                            },
                                                                            Err(e) => error!("Error encoding torrent for upload: {}", e),
                                                                        }
                                                                    },
                                                                    Err(err) => error!("Error removing torrent from client: {} (error: {:?})", torrent.name, err),
                                                                }
                                                            } else {
                                                                debug!("Adding trackers to torrent since they aren't private...");
                                                                let torrent_announces = torrent_announces.iter()
                                                                    .map(|u| u.to_owned().to_owned())
                                                                    .collect();

                                                                if let Err(err) =  torrent_client.add_torrent_trackers(&info, torrent_announces).await {
                                                                    error!("Error adding torrent trackers to torrent: {} (err: {:?})", torrent.name, err);
                                                                }       
                                                            }
                                                        },
                                                        TorrentMode::InjectFile => {
                                                            debug!("Cannot add trackers, uploading new torrent...");
    
                                                            // Clone some fields from the torrent due to ownership issues with
                                                            // found_torrent.encode()
                                                            let name = found_torrent.name.clone();
                                                            let hash = found_torrent.info_hash().clone();
    
                                                            match found_torrent.encode() {
                                                                Ok(bytes) => {
                                                                    let upload = TorrentUpload::builder()
                                                                        .torrent_data(format!("{}.torrent", hash), bytes)
                                                                        .category(config.torrent_category())
                                                                        //.paused() // TODO: don't pause new uploads
                                                                        .build();
            
                                                                    match torrent_client.add_torrent(&upload).await {
                                                                        Ok(()) => info!("Added cross-seed torrent {}!", name),
                                                                        Err(err) => error!("Failure to add cross-seed torrent: {} (Error {:?})", name, err),
                                                                    }
                                                                }, 
                                                                Err(e) => warn!("Failure to encode ({}) {}", e, name),
                                                            }
                                                        },
                                                        TorrentMode::Filesystem => {
                                                            todo!(); // TODO: implement
                                                        }
                                                    }
                                                },
                                                _ => debug!("Torrent is not done downloading, skipping..."),
                                            }
                                            
                                        },
                                        None => info!("Torrent file {} was not found in the client, skipping...", torrent.name),
                                    }
                                } else {
                                    debug!("Found the torrent in its original indexer, skipping...");
                                }
                            }
                        }
                    },
                    None => {
                        panic!("idfk");
                    }
                }
            }));
        }
    }

    futures::future::join_all(indexer_handles).await;
}