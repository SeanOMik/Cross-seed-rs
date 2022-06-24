use std::sync::Arc;

use lava_torrent::torrent::v1::Torrent;
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;

use crate::torznab::{TorznabClient, GenericSearchParameters, SearchFunction};

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Indexer {
    #[serde(skip_deserializing)]
    /// Name of the indexer
    pub name: String,
    /// Whether the indexer is enabled or not for searching
    pub enabled: Option<bool>,
    /// URL to query for searches
    pub url: String,
    /// API key to pass to prowlarr/jackett
    pub api_key: String,

    #[serde(skip)]
    pub client: Option<Arc<RwLock<TorznabClient>>>, // TODO: Create a client pool.
}

impl Indexer {
    pub async fn create_client(&mut self) -> Result<&Arc<RwLock<TorznabClient>>, crate::torznab::ClientError> {
        if self.client.is_none() {
            self.client = Some(Arc::new(RwLock::new(TorznabClient::new(self.name.clone(), &self.url, &self.api_key).await?)));
        }

        Ok(self.client.as_ref().unwrap())
    }

    /// Search an indexer for a torrent with its name, and return the found torrent.
    pub async fn search_indexer(&self, torrent: &Torrent) -> Result<Option<Torrent>, crate::torznab::ClientError> {
        // The client should be set to something already
        let client = self.client.as_ref().unwrap().read().await;

        let generic = GenericSearchParameters::builder()
            .query(torrent.name.clone())
            .build();
        let results = client.search(SearchFunction::Search, generic).await.unwrap();
        
        // Drop the indexer client asap for other torrent searches.
        drop(client);

        // The first result should be the correct one.
        if let Some(result) = results.first() {
            let found_torrent = result.download_torrent().await?;

            Ok(Some(found_torrent)) 
        } else {
            Ok(None)
        }
    }
}

#[derive(Debug)]
pub enum IndexerSearchError {
    TorznabError(crate::torznab::ClientError),
}

impl From<crate::torznab::ClientError> for IndexerSearchError {
    fn from(err: crate::torznab::ClientError) -> Self {
        Self::TorznabError(err)
    }
}