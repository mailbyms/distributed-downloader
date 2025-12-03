//! HTTP downloader implementation

use crate::error::Result;
use bytes::Bytes;
use reqwest::Client;

/// A new, standalone function to download a specific byte range from a URL into memory.
pub async fn download_range(url: &str, start: u64, end: u64) -> Result<Bytes> {
    let downloader = HttpDownloader::new();
    downloader.partial_request(url, start, end).await
}

/// HTTP Downloader
pub struct HttpDownloader {
    client: Client,
}

impl HttpDownloader {
    /// Creates a new HTTP downloader
    pub fn new() -> Self {
        let client = Client::builder()
            .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/91.0.4472.124 Safari/537.36")
            .build()
            .expect("Failed to build HTTP client");
        Self { client }
    }

    /// Downloads a segment using a range request into a Bytes buffer.
    pub async fn partial_request(&self, url: &str, left_point: u64, right_point: u64) -> Result<Bytes> {
        let range_header = format!("bytes={}-{}", left_point, right_point);

        let response = self
            .client
            .get(url)
            .header("Range", range_header)
            .send()
            .await?;
        
        // Ensure the request was successful
        let status = response.status();
        if !status.is_success() {
            return Err(crate::error::DistributedDownloaderError::HttpError(
                format!("HTTP request failed with status: {}", status)
            ));
        }

        let body = response.bytes().await?;
        Ok(body)
    }
}

impl Clone for HttpDownloader {
    fn clone(&self) -> Self {
        Self {
            client: self.client.clone(),
        }
    }
}
