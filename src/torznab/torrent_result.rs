use lava_torrent::torrent::v1::Torrent;
use rss::Item;

use async_recursion::async_recursion;

use super::ClientError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResultError {
    MissingTitle,
    MissingLink,
    InvalidRedirect,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TorrentResult {
    pub name: String,
    pub link: String,
    /* size: u64,
    categories: Vec<u32>, */
}

impl TorrentResult {
    pub fn from_item(item: Item) -> Result<Self, ResultError> {
        let name = item.title().ok_or(ResultError::MissingTitle)?;
        let link = item.link().ok_or(ResultError::MissingLink)?;
        /* let size = item.enclosure().map(|e| e.length().parse::<u64>());
        let categories = item.categories().ok_or(ResultError::MissingTitle)?; */

        Ok(TorrentResult {
            name: String::from(name.clone()),
            link: String::from(link),
            /* size,
            categories, */
        })
    }

    #[async_recursion]
    async fn download_impl(&self, client: &reqwest::Client, url: &str) -> Result<Torrent, ClientError> {
        let res = client
            .get(&self.link)
            .send().await?;

        if res.status() == 301 {
            let headers = res.headers();
            if let Some(location) = headers.get(reqwest::header::LOCATION) {
                let location = location.to_str().unwrap();
                
                self.download_impl(client, location).await
            } else {
                Err(ClientError::InvalidRedirect)
            }
        } else {
            let bytes = res.bytes().await?;

            if url.starts_with("magnet:?") {
                let magnet = magnet_url::Magnet::new(url).unwrap();

                todo!() // TODO
            } else {
                let torrent = Torrent::read_from_bytes(bytes)?;

                Ok(torrent)
            }
        }
    }

    pub async fn download_torrent(&self) -> Result<Torrent, ClientError> {
        self.download_impl(&reqwest::Client::default(), &self.link).await
    }
}

/* impl<'a> From<Item> for TorrentResult<'a> {
    fn from(item: Item) -> Self {
        TorrentResult {
            name: item.title().unwrap(),
            link: item.link().unwrap(),
            size: item.size().unwrap(),
            categories: item.categories().unwrap(),
        }
    }
} */