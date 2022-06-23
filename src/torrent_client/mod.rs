use std::ops::{Deref, DerefMut};

use abstracttorrent::{client::qbittorrent, torrent::TorrentInfo, common::GetTorrentListParams};

use crate::config::Config;

pub struct TorrentClient {
    client: Box<dyn abstracttorrent::client::TorrentClient + Send + Sync>,
}

impl TorrentClient {
    pub fn from_config(config: &Config) -> Self {
        // TODO: figure out which client to use if multiple are specified.

        if let Some(qbittorrent) = &config.qbittorrent {
            TorrentClient {
                client: Box::new(qbittorrent::client::QBittorrentClient::new())
            }
        } else {
            panic!("Invalid config!");
        }
    }

    pub async fn login(&mut self, config: &Config) -> abstracttorrent::client::ClientResult<()> {
        let (url, username, password) = match &config.qbittorrent {
            Some(qb) => {
                (&qb.url, &qb.username, &qb.password)
            },
            None => {
                panic!("Invalid config!");
            }
        };

        self.client.login(&url, username, password).await
    }

    /* pub fn login_with_config(&self, config: &Config) -> abstracttorrent::client::ClientResult<()> {
        self.login(url, username, password)
    } */

    pub async fn get_torrent_info(&self, torrent: &lava_torrent::torrent::v1::Torrent) -> abstracttorrent::client::ClientResult<Option<TorrentInfo>> {
        let params = GetTorrentListParams::builder()
            .hash(&torrent.info_hash())
            .build();

        let results = self.client.get_torrent_list(Some(params)).await?;
        Ok(results.first().cloned())
    }
}

impl Deref for TorrentClient {
    type Target = Box<dyn abstracttorrent::client::TorrentClient + Send + Sync>;
    fn deref(&self) -> &Self::Target { &self.client }
}

impl DerefMut for TorrentClient {
    fn deref_mut(&mut self) -> &mut Box<dyn abstracttorrent::client::TorrentClient + Send + Sync> { &mut self.client }
}