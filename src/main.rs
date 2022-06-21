mod config;
mod torznab;

use config::Config;
use lava_torrent::bencode::BencodeElem;
use tracing::{info, Level, debug};

use std::path::{Path, PathBuf};
use std::error::Error;

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
        .with_max_level(Level::INFO)
        .finish();

    tracing::subscriber::set_global_default(subscriber).expect("Failed to set global default log subscriber");

    // Get config and debug the torrents
    let config = Config::new();
    info!("Searching for torrents in: {}", config.torrents_path_str());

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
    info!("Found {} torrents", torrent_files.len());

    // Convert the indexers to be async friendly.
    let mut indexers = indexers.iter()
        .map(|indexer| Arc::new(RwLock::new(indexer.clone())))
        .collect::<Vec<_>>();

    let mut indexer_handles = vec![];

    for torrent_path in torrent_files.iter() {
        let torrent = Torrent::read_from_file(torrent_path).unwrap();
        let torrent = Arc::new(torrent);

        for indexer in indexers.iter() {
            info!("Checking for \"{}\"", torrent.name);

            let mut indexer = Arc::clone(indexer);
            let torrent = Arc::clone(&torrent);
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

                            if let Some(found_announces) = &found_torrent.announce_list {
                                // Some urls can be encoded so we need to decode to compare them.
                                let found_announces: Vec<Vec<String>> = found_announces.iter()
                                    .map(|a_list| a_list.iter().map(|a| urlencoding::decode(a).unwrap().to_string()).collect::<Vec<String>>())
                                    .collect();

                                if let Some(torrent_announces) = &torrent.announce_list {
                                    let mut found_announces_flat: Vec<&String> = Vec::new();
                                    for i in found_announces.iter() {
                                        for j in i.iter() {
                                            found_announces_flat.push(j);
                                        }
                                    }

                                    let mut flat_announces: Vec<&String> = Vec::new();
                                    for i in torrent_announces.iter() {
                                        for j in i.iter() {
                                            flat_announces.push(j);
                                        }
                                    }

                                    // Check if the announce urls from the found torrent are in the one
                                    // that is on the file system.
                                    let mut in_tracker = true;
                                    for found_url in found_announces_flat.iter() {
                                        in_tracker = in_tracker && flat_announces.contains(found_url);
                                    }

                                    if !in_tracker {
                                        info!("Found a cross-seedable torrent for {}", found_torrent.name);
                                    } else {
                                        debug!("Found the torrent in its original indexer, skipping...");
                                    }
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