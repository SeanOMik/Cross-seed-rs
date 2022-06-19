use serde::{Deserialize,Serialize};
use std::path::Path;
use std::env;
use std::collections::HashMap;
use figment::{Figment, providers::{Format, Toml, Env}};
use figment::value::Value as FigmentValue;

use crate::torznab::TorznabClient;

use super::CliProvider;

#[derive(Debug, Clone)]
pub enum RunMode {
    Script,
    Daemon,
}

#[derive(Debug, Clone)]
pub enum TorrentMode { 
    Inject,
    Search,
}

#[derive(Deserialize, Serialize)]
pub struct Config {
    /// The path of the torrents to search.
    torrents_path: String,
    /// The output path of the torrents.
    output_path: Option<String>,    
    /// Whether or not to strip public trackers from cross-seed torrents.
    strip_public: bool,
    /// When running as script we exit the program after finishing. In daemon mode we run it at set intervals.
    run_mode: RunMode,
    /// When running as inject we inject torrents cross-seed has found directly into the client, when running as search we populate the output folder.
    torrent_mode: TorrentMode,
    /// Whether to cache using an external db (ie regis) or don't cache.
    use_cache: bool,
    /// Whether to keep the original torrent file and create a new one for cross-seed or delete original and upload cross-seed
    replace_torrents: bool,

    //pub indexers: HashMap<String, Indexer>,

    /// Used for deserializing the indexers into a Vec<Indexer>.
    #[serde(rename = "indexers")]
    indexers_map: HashMap<String, FigmentValue>,

    /// The indexers to search.
    #[serde(skip)]
    pub indexers: Vec<Indexer>,
}

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
    pub client: Option<TorznabClient>,
}

impl Indexer {
    pub async fn create_client(&mut self) -> Result<&TorznabClient, crate::torznab::ClientError> {
        if self.client.is_none() {
            self.client = Some(TorznabClient::new(self.name.clone(), &self.url, &self.api_key).await?);
        }

        Ok(self.client.as_ref().unwrap())
    }
}

// Allow dead code for functions. We should probably remove this later on.
#[allow(dead_code)]
impl Config {
    pub fn new() -> Config {
        // The path of the config file without the file extension
        let path = match env::var("CROSS_SEED_CONFIG") {
            Ok(path) => path,
            Err(_) => "config".to_string(),
        };

        // TODO: Create a command line argument `Provider` (https://docs.rs/figment/0.10.6/figment/trait.Provider.html)
        // TODO: Figure out priority
        // Merge the config files
        let figment = Figment::new()
            .join(CliProvider::new())
            .join(Env::prefixed("CROSS_SEED_"))
            .join(Toml::file(format!("{}.toml", path)));

        let mut config: Config = figment.extract().unwrap();

        // Parse the indexers map into a vector.
        for (name, value) in &mut config.indexers_map {
            let mut indexer: Indexer = value.deserialize().unwrap();
            indexer.name = name.to_owned();

            config.indexers.push(indexer);
        }

        config
    }

    pub fn torrents_path(&self) -> &Path {
        Path::new(&self.torrents_path)
    }

    pub fn torrents_path_str(&self) -> &String {
        &self.torrents_path
    }

    pub fn output_path(&self) -> Option<&Path> {
        match self.output_path {
            Some(ref path) => Some(Path::new(path)),
            None => None,
        }
    }

    pub fn output_path_str(&self) -> Option<&String> {
        self.output_path.as_ref()
    }
}