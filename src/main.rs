mod config;
mod torznab;
mod torrent_client;
mod indexer;
mod cross_seed;

use config::Config;

use indexer::Indexer;
use torrent_client::TorrentClient;
use tracing::metadata::LevelFilter;
use tracing::{info};

use std::path::{Path, PathBuf};
use std::error::Error;
use std::vec;

use lava_torrent::torrent::v1::Torrent;

use crate::cross_seed::CrossSeed;

use std::sync::Arc;

#[tokio::main]
async fn main() {
    // Get config and debug the torrents
    let config = Arc::new(Config::new());

    let subscriber = tracing_subscriber::fmt()
        .with_max_level(Into::<LevelFilter>::into(config.log_level.clone()))
        .finish();
    tracing::subscriber::set_global_default(subscriber)
        .expect("Failed to set global default log subscriber");

    info!("Searching for torrents in: {}", config.torrents_path_str());

    // Get torrent client
    let torrent_client = get_torrent_client(&config).await;

    // Get indexers
    let indexers = get_indexers(&config).await;
    info!("Searching {} trackers: ", indexers.len());

    // Parse torrents from filesystem
    let torrents = parse_torrents(&config, Arc::clone(&torrent_client)).await;
    info!("Found {} torrents possibly eligible for cross-seeding.", torrents.len());

    // Store async tasks to wait for them to finish
    let mut indexer_handles = vec![];

    let seed = Arc::new(CrossSeed::new_arcs(config, indexers, torrent_client));
    for torrent in torrents {
        let seed = Arc::clone(&seed);
        
        indexer_handles.push(tokio::spawn(async move {
            seed.search_for_torrent(&torrent).await.unwrap();
        }));
    }

    futures::future::join_all(indexer_handles).await;
}

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

async fn get_indexers(config: &Config) -> Arc<Vec<Indexer>> {
    let mut indexers = config.indexers.clone();

    // Create torznab clients for each indexer.
    for indexer in indexers.iter_mut() {
        indexer.create_client().await.unwrap();
    }

    // Create arc of indexers
    Arc::new(indexers)
}

async fn get_torrent_client(config: &Config) -> Arc<TorrentClient> {
    // Get a torrent client from the config.
    let mut torrent_client = torrent_client::TorrentClient::from_config(&config);
    torrent_client.login(&config).await.unwrap();

    // Torrent client no longer needs to mut, so we can just create an `Arc` without a mutex.
    Arc::new(torrent_client)
}

async fn parse_torrents(config: &Config, torrent_client: Arc<TorrentClient>) -> Vec<Torrent> {
    // Read the torrents from the config as `PathBuf`s
    let torrent_files = read_torrents(config.torrents_path()).unwrap();
    info!("Found {} torrent files...", torrent_files.len());

    // Parse the torrent files as `Torrent` structs.
    info!("Parsing all torrent files...");
    let mut stop = stopwatch::Stopwatch::start_new();

    // Get the torrents and from the paths
    let mut torrents: Vec<Result<Torrent, lava_torrent::LavaTorrentError>> = torrent_files.iter()
        .map(|path| Torrent::read_from_file(path))
        .collect();
    stop.stop();
    
    info!("Took {} seconds to parse all torrents", stop.elapsed().as_secs());
    drop(stop); // Drop for memory

    // Remove the torrents that failed to be read from the file, and
    // are not in the download client.
    //
    // NOTE: It might be better to get all torrents on the client and check that the torrents are on the
    // client locally.

    /* let torrents = torrents.iter()
        .map(|res| res.map(|torrent| {
            let info = futures::executor::block_on(torrent_client.get_torrent_info(&torrent))
                .unwrap_or(None);

            torrent.ha
            //(torrent, info)
        })).collect(); */
    torrents.retain(|torrent| {
        if let Ok(torrent) = torrent {
            let info = futures::executor::block_on(torrent_client.get_torrent_info(&torrent))
                .unwrap_or(None);

            info.is_some()
        } else {
            false
        }
    });

    // Unwrap the results, all errored ones were removed from the `.retain`
    let torrents: Vec<Torrent> = torrents.iter()
        .map(|res| res.as_ref().unwrap().clone())
        .collect();

    torrents
}