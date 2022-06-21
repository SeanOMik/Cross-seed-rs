mod config;
mod torznab;

use config::Config;
use lava_torrent::bencode::BencodeElem;
use tracing::{info, Level, debug};

use std::path::{Path, PathBuf};
use std::error::Error;

use lava_torrent::torrent::v1::Torrent;

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
    info!("Searching torrents in: {}", config.torrents_path_str());

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

                        //println!("Results: {:?}", results);
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