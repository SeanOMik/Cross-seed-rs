use serde::{Deserialize,Serialize};
use tracing::metadata::LevelFilter;
use std::path::Path;
use std::env;
use std::collections::HashMap;
use figment::{Figment, providers::{Format, Toml, Env}};
use figment::value::Value as FigmentValue;

use crate::torznab::TorznabClient;
use crate::indexer::Indexer;

use super::CliProvider;

fn default_bool_true() -> bool {
    true
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Config {
    /// The path of the torrents to search.
    torrents_path: String,
    
    /// The output path of the torrents.
    output_path: Option<String>,
    
    /// When running as script we exit the program after finishing. In daemon mode we run it at set intervals.
    #[serde(default)]
    pub run_mode: RunMode,
    
    /// When running as inject we inject torrents cross-seed has found directly into the client, when running as search we populate the output folder.
    #[serde(default)]
    pub torrent_mode: TorrentMode,

    /// Whether to cache using an external db (ie regis) or don't cache.
    #[serde(default)]
    pub use_cache: bool,

    /// Whether or not to strip public trackers from cross-seed torrents.
    #[serde(default)]
    pub strip_public_trackers: bool,

    #[serde(default)]
    pub log_level: LogLevel,

    /// The category of added cross-seed torrents.
    torrent_category: Option<String>,

    /// Used for deserializing the indexers into a Vec<Indexer>.
    #[serde(rename = "indexers")]
    indexers_map: HashMap<String, FigmentValue>,

    /// The indexers to search.
    #[serde(skip)]
    pub indexers: Vec<Indexer>,

    /// Config section for qbittorrent client
    pub qbittorrent: Option<super::client::qbittorrent::QBittorrentConfig>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub enum RunMode {
    #[serde(alias = "script")]
    Script,
    #[serde(alias = "daemon")]
    Daemon,
}

impl Default for RunMode {
    fn default() -> Self {
        RunMode::Script
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub enum TorrentMode { 
    /// Inject a found torrent's trackers into the torrent being downloaded
    /// by the client.
    #[serde(alias = "inject_trackers", alias = "injecttrackers")]
    InjectTrackers,

    /// Upload the torrent file to the torrent client. This will cause there
    /// to be two uploading torrents on the client. One which is found by cross-seed
    /// and the other which was imported by the user or another application.
    #[serde(alias = "inject_file", alias = "injectfile")]
    InjectFile,
    
    /// Cross-seeded torrents will be stored in the filesystem.
    #[serde(alias = "search")]
    Filesystem,
}

impl Default for TorrentMode {
    fn default() -> Self {
        TorrentMode::InjectTrackers
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub enum LogLevel {
    #[serde(alias = "error")]
    Error,
    
    #[serde(alias = "warn")]
    Warn,
    
    #[serde(alias = "info")]
    Info,
    
    #[serde(alias = "debug")]
    Debug,
    
    #[serde(alias = "trace")]
    Trace,
    
    #[serde(alias = "off", alias = "disabled")]
    Off,
}

impl Default for LogLevel {
    fn default() -> Self {
        Self::Info
    }
}

impl Into<LevelFilter> for LogLevel {
    fn into(self) -> LevelFilter {
        match self {
            LogLevel::Error => LevelFilter::ERROR,
            LogLevel::Warn => LevelFilter::WARN,
            LogLevel::Info => LevelFilter::INFO,
            LogLevel::Debug => LevelFilter::DEBUG,
            LogLevel::Trace => LevelFilter::TRACE,
            LogLevel::Off => LevelFilter::OFF,
        }
    }
}

// Allow dead code for functions. We should probably remove this later on.
#[allow(dead_code)]
impl Config {
    pub fn new() -> Self {
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

    pub fn torrents_path_str(&self) -> &str {
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

    pub fn torrent_category(&self) -> String {
        self.torrent_category.as_ref()
            .unwrap_or(&String::from("cross-seed-rs"))
            .clone()
    }
}