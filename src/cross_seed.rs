use std::sync::Arc;

use lava_torrent::torrent::v1::Torrent;
use lava_torrent::bencode::BencodeElem;
use tracing::{debug, error, info};

use crate::{config::{Config, TorrentMode}, indexer::Indexer};

use abstracttorrent::torrent::{TorrentUpload, TorrentState, TorrentInfo};

pub struct CrossSeed {
    config: Arc<Config>,
    indexers: Arc<Vec<Indexer>>,
    torrent_client: Arc<crate::torrent_client::TorrentClient>,
}

#[allow(dead_code)]
impl CrossSeed {
    pub fn new(config: Config, indexers: Vec<Indexer>, torrent_client: crate::torrent_client::TorrentClient) -> Self {
        Self {
            config: Arc::new(config),
            indexers: Arc::new(indexers),
            torrent_client: Arc::new(torrent_client),
        }
    }

    pub fn new_arcs(config: Arc<Config>, indexers: Arc<Vec<Indexer>>, torrent_client: Arc<crate::torrent_client::TorrentClient>) -> Self {
        Self {
            config,
            indexers,
            torrent_client,
        }
    }

    /// Start searching for all torrents, this searches for torrents in sequential order.
    pub async fn start_searching(&self, torrents: Vec<Torrent>) -> Result<(), CrossSeedError> {
        for torrent in torrents.iter() {
            self.search_for_torrent(torrent).await?;
        }

        Ok(())
    }

    /// Search for a specific torrent in the indexers.
    pub async fn search_for_torrent(&self, torrent: &Torrent) -> Result<(), CrossSeedError> {
        // TODO: Add a `tracing` log scope.

        for indexer in self.indexers.iter() {
            match self.torrent_client.get_torrent_info(&torrent).await? {
                Some(info) => match self.search_for_cross_torrent(indexer, torrent, info.clone()).await? {
                    Some(found_torrent) => self.add_cross_seed_torrent(&torrent, found_torrent, info).await?,
                    /* {
                        match self.torrent_client.get_torrent_info(&torrent).await? {
                            Some(info) => self.add_cross_seed_torrent(&torrent, found_torrent, info).await?,
                            None => error!("Failed to find torrent in the client!"),
                        }
                    }, */
                    None => {}, // TODO
                },
                None => error!("Failed to find torrent in the client!"), // TODO
            }
            
        }

        Ok(())
    }

    pub async fn add_cross_seed_torrent(&self, torrent: &Torrent, found_torrent: Torrent, info: TorrentInfo) -> Result<(), CrossSeedError> {
        match info.state {
            TorrentState::Uploading | TorrentState::QueuedUploading => {
                match self.config.torrent_mode {
                    TorrentMode::InjectTrackers => {
                        if found_torrent.is_private() {
                            debug!("The found torrent is private, so we must remove the torrent and re-add it with the new trackers...");

                            // We have to merge the announce urls before we remove the torrent since we retrieve the
                            // urls from the torrent client.
                            let torrent = self.merge_torrent_announces(&torrent, &found_torrent).await?;

                            self.torrent_client.remove_torrent(&info, false).await?;
                            
                            debug!("Re-uploading torrent to client...");

                            // Clone some fields from the torrent due to ownership issues with
                            // torrent.encode()
                            let name = torrent.name.clone();
                            let hash = torrent.info_hash().clone();

                            match torrent.encode() {
                                Ok(bytes) => {
                                    let upload = TorrentUpload::builder()
                                        .category(self.config.torrent_category())
                                        .tags(info.tags)
                                        .torrent_data(format!("{}.torrent", hash), bytes)
                                        //.paused()
                                        .build();

                                    match self.torrent_client.add_torrent(&upload).await {
                                        Ok(()) => info!("Added cross-seed torrent {}!", name),
                                        Err(err) => error!("Error adding cross-seed torrent: {} (error: {:?}", name, err),
                                    }
                                },
                                Err(e) => error!("Error encoding torrent for upload: {}", e),
                            }
                        } else {
                            debug!("Adding trackers to torrent since they aren't private...");
                            // Flatten the announce list
                            let found_announces: Vec<String> = found_torrent.announce_list.as_ref()
                                .unwrap().iter()
                                .flat_map(|array| array.iter())
                                .into_iter().cloned()
                                .collect();

                            if let Err(err) = self.torrent_client.add_torrent_trackers(&info, found_announces).await {
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
                                    .category(self.config.torrent_category())
                                    //.paused() // TODO: don't pause new uploads
                                    .build();

                                match self.torrent_client.add_torrent(&upload).await {
                                    Ok(()) => info!("Added cross-seed torrent {}!", name),
                                    Err(err) => error!("Failure to add cross-seed torrent: {} (Error {:?})", name, err),
                                }
                            }, 
                            Err(e) => error!("Failure to encode ({}) {}", e, name),
                        }
                    },
                    TorrentMode::Filesystem => {
                        todo!(); // TODO: implement
                    }
                }
            },
            _ => debug!("Torrent is not done downloading, skipping..."),
        }

        Ok(())
    }

    /// Merge two torrent's announce urls into one torrent.
    pub async fn merge_torrent_announces(&self, torrent: &Torrent, found_torrent: &Torrent) -> Result<Torrent, abstracttorrent::error::ClientError> {
        // Get announce urls of both torrents.
        let request_info = TorrentInfo::from_hash(torrent.info_hash());
        let torrent_announces = self.torrent_client.get_torrent_trackers(&request_info).await?;
        let torrent_announces: Vec<&String> = torrent_announces.iter().map(|t| &t.url).collect();

        // Flatten the announce list
        let found_announces: Vec<&String> = found_torrent.announce_list.as_ref()
            .unwrap().iter()
            .flat_map(|array| array.iter())
            .collect();

        // Combine both announces and deref the Strings by cloning them.
        let mut torrent_announces: Vec<String> = torrent_announces.into_iter()
            .chain(found_announces)
            .cloned()
            .collect();
        // Remove the [DHT], [PeX] and [LSD] announces from the list.
        // The client should handle those.
        torrent_announces.retain(|announce| !(announce.starts_with("** [") && announce.ends_with("] **")));

        // Copy the torrent file and add the announces to it.
        // Additionally, add the private field to the torrent.
        let mut torrent = torrent.clone();
        torrent.announce_list = Some(vec![torrent_announces]);
        if let Some(extra) = torrent.extra_info_fields.as_mut() {
            extra.insert(String::from("private"), BencodeElem::Integer(1));
        } else {
            let mut extra = std::collections::HashMap::new();
            extra.insert(String::from("private"), BencodeElem::Integer(1));
            torrent.extra_info_fields = Some(extra);
        }

        Ok(torrent)
    }

    /// Searches for a torrent in another indexer. Will return the found torrent.
    pub async fn search_for_cross_torrent(&self, indexer: &Indexer, torrent: &Torrent, info: TorrentInfo) -> Result<Option<Torrent>, CrossSeedError> {
        if let Some(found_torrent) = indexer.search_indexer(&torrent).await? {

            // Check if we found the same torrent in its own indexer
            if found_torrent.info_hash() == torrent.info_hash() {
                debug!("Found same torrent in its own indexer, skipping...");
                return Ok(None);
            }

            // Check if we're already seeding this specific torrent file.
            if self.torrent_client.has_exact_torrent(&found_torrent).await? {
                info!("Already cross-seeding to this tracker (with a separate torrent file), skipping...");
                return Ok(None); 
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
                let torrent_announces = self.torrent_client.get_torrent_trackers(&info).await.unwrap(); // TODO: Remove
                let torrent_announces: Vec<&String> = torrent_announces.iter().map(|t| &t.url).collect();

                // Flatten the announce list to make them easier to search.
                let found_announces: Vec<&String> = found_announces.iter()
                    .flat_map(|array| array.iter())
                    .collect();

                // Check if the client has the trackers of the torrent already.
                let client_has_trackers = found_announces.iter()
                    .all(|tracker| torrent_announces.contains(tracker));

                if !client_has_trackers {
                    return Ok(Some(found_torrent));
                } else {
                    info!("Already cross seeding to this tracker, skipping...");
                }
            }
        }

        Ok(None)
    }
}

#[derive(Debug)]
pub enum CrossSeedError {
    TorznabClient(crate::torznab::ClientError),
    TorrentClient(abstracttorrent::error::ClientError),
}

impl From<crate::torznab::ClientError> for CrossSeedError {
    fn from(err: crate::torznab::ClientError) -> Self {
        Self::TorznabClient(err)
    }
}

impl From<abstracttorrent::error::ClientError> for CrossSeedError {
    fn from(err: abstracttorrent::error::ClientError) -> Self {
        Self::TorrentClient(err)
    }
}