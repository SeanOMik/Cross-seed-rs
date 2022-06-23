use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize)]
pub struct QBittorrentConfig {
    pub url: String,
    pub username: String,
    pub password: String,
}