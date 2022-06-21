#[derive(Debug)]
pub enum ClientError {
    HttpError(reqwest::Error),
    SearchResultError(super::ResultError),
    InvalidRedirect,
    TorrentError(lava_torrent::LavaTorrentError),
}

impl From<reqwest::Error> for ClientError {
    fn from(e: reqwest::Error) -> Self {
        ClientError::HttpError(e)
    }
}

impl From<super::ResultError> for ClientError {
    fn from(e: super::ResultError) -> Self {
        ClientError::SearchResultError(e)
    }
}

impl From<lava_torrent::LavaTorrentError> for ClientError {
    fn from(e: lava_torrent::LavaTorrentError) -> Self {
        ClientError::TorrentError(e)
    }
}