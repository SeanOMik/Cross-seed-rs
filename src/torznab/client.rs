use super::{Capabilities, TorznabFunction, SearchFunction, GenericSearchParameters, TorrentResult, ClientError};

use bytes::Bytes;
use bytes::Buf;

use rss::Channel;
use tracing::{span, event, debug, info, Level};

#[derive(Debug, Clone)]
pub struct TorznabClient {
    http: reqwest::Client,
    pub name: String,
    pub base_url: String,
    api_key: String,
    pub capabilities: Capabilities,
    pub client_span: tracing::Span,
}

impl TorznabClient {
    fn client_span(name: &String) -> tracing::Span {
        span!(Level::INFO, "torznab_client", indexer = %name)
    }

    /// Construct a new client without getting the capabilities
    pub fn new_no_capabilities(name: String, base_url: &str, api_key: &str) -> Self {
        TorznabClient {
            name: name.clone(),
            http: reqwest::Client::new(),
            base_url: base_url.to_string(),
            api_key: api_key.to_string(),
            capabilities: Capabilities::default(),
            client_span: Self::client_span(&name),
        }
    }

    /// Construct a new client and get the capabilities.
    pub async fn new(name: String, base_url: &str, api_key: &str) -> Result<Self, reqwest::Error> {
        let mut client = TorznabClient {
            name: name.clone(),
            http: reqwest::Client::new(),
            base_url: base_url.to_string(),
            api_key: api_key.to_string(),
            capabilities: Capabilities::default(),
            client_span: Self::client_span(&name),
        };

        // Get capabilities and store them in the client before returning
        client.store_capabilities().await?;

        Ok(client)
    }

    /// Send a request to the indexer using the query parameters.
    async fn request(&self, param_str: String) -> Result<Bytes, reqwest::Error> {
        let span = span!(parent: &self.client_span, Level::INFO, "client request");
        let _enter = span.enter();

        // Construct the url
        let url = format!("{}?apikey={}{}", self.base_url, self.api_key, param_str);
        debug!("Url: {}", url);

        self.http.get(url).send().await?.error_for_status()?.bytes().await
    }

    /// Request the capabilities of the indexer and return them.
    pub async fn request_capabilities(&self) -> Result<Capabilities, reqwest::Error> {
        let params = TorznabFunction::Capabilities.to_params();

        let res = self.request(params).await?;
        let str_res = String::from_utf8(res.as_ref().to_vec()).unwrap(); // TODO Handle

        let cap: Capabilities = quick_xml::de::from_str(&str_res).unwrap();

        Ok(cap)
    }

    /// Request and store the capabilities of the indexer in the struct.
    pub async fn store_capabilities(&mut self) -> Result<&Capabilities, reqwest::Error> {
        self.capabilities = self.request_capabilities().await?;
        Ok(&self.capabilities)
    }

    /// Search for torrents.
    pub async fn search(&self, func: SearchFunction, generic_params: GenericSearchParameters) -> Result<Vec<TorrentResult>, ClientError> {
        let param_str = format!("{}{}", func.to_params(), generic_params.to_params());

        let bytes = self.request(param_str).await?;
        let reader = bytes.reader();

        let channel = Channel::read_from(reader).unwrap(); // TODO: handle
        let items = channel.into_items();

        let torrents: Vec<TorrentResult> = items.iter()
            .map(|i| TorrentResult::from_item(i.clone()))
            .collect::<Result<Vec<TorrentResult>, super::ResultError>>()?;

        Ok(torrents)
    }
}