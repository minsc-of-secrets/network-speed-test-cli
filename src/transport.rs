//! Transport abstraction over the Cloudflare speed-test endpoint.
//!
//! Kept as a trait so [`crate::measurement`] can be unit-tested against a
//! fake implementation, independent of any real network I/O.

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use async_trait::async_trait;

/// Size of each chunk requested/sent while a stream is running. Small enough
/// that a stream can react quickly to a deadline, large enough to keep HTTP
/// overhead negligible relative to transfer time.
pub const CHUNK_BYTES: usize = 10_000_000;

#[async_trait]
pub trait Transport: Send + Sync {
    /// Repeatedly download chunks until `deadline`, returning the total
    /// number of bytes actually received. `progress` is incremented by each
    /// chunk's size as it arrives, so callers can report live progress.
    async fn run_download_stream(
        &self,
        deadline: Instant,
        progress: Arc<AtomicU64>,
    ) -> anyhow::Result<u64>;

    /// Repeatedly upload chunks until `deadline`, returning the total
    /// number of bytes actually sent. `progress` is incremented by each
    /// chunk's size as it is sent.
    async fn run_upload_stream(
        &self,
        deadline: Instant,
        progress: Arc<AtomicU64>,
    ) -> anyhow::Result<u64>;

    /// A single round trip against the endpoint, used to sample latency
    /// while idle (no streams in flight).
    async fn ping(&self) -> anyhow::Result<Duration>;
}

pub struct CloudflareTransport {
    client: reqwest::Client,
    base_url: String,
}

impl CloudflareTransport {
    pub fn new() -> anyhow::Result<Self> {
        Ok(Self {
            client: reqwest::Client::builder().build()?,
            base_url: "https://speed.cloudflare.com".to_string(),
        })
    }

    #[cfg(test)]
    fn with_base_url(base_url: impl Into<String>) -> Self {
        Self {
            client: reqwest::Client::new(),
            base_url: base_url.into(),
        }
    }
}

#[async_trait]
impl Transport for CloudflareTransport {
    async fn run_download_stream(
        &self,
        deadline: Instant,
        progress: Arc<AtomicU64>,
    ) -> anyhow::Result<u64> {
        let mut total = 0u64;
        while Instant::now() < deadline {
            let url = format!("{}/__down?bytes={}", self.base_url, CHUNK_BYTES);
            let send = self.client.get(&url).send();
            let mut resp = tokio::select! {
                biased;
                _ = tokio::time::sleep_until(deadline.into()) => break,
                resp = send => resp?.error_for_status()?,
            };

            loop {
                let chunk = tokio::select! {
                    biased;
                    _ = tokio::time::sleep_until(deadline.into()) => break,
                    chunk = resp.chunk() => chunk?,
                };
                match chunk {
                    Some(bytes) => {
                        total += bytes.len() as u64;
                        progress.fetch_add(bytes.len() as u64, Ordering::Relaxed);
                    }
                    None => break,
                }
            }
        }
        Ok(total)
    }

    async fn run_upload_stream(
        &self,
        deadline: Instant,
        progress: Arc<AtomicU64>,
    ) -> anyhow::Result<u64> {
        let mut total = 0u64;
        while Instant::now() < deadline {
            let remaining = deadline.saturating_duration_since(Instant::now());
            let body = vec![0u8; CHUNK_BYTES];
            let url = format!("{}/__up", self.base_url);
            let send = self.client.post(&url).body(body).send();

            let result = tokio::time::timeout(remaining.max(Duration::from_millis(1)), send).await;
            match result {
                Ok(resp) => {
                    resp?.error_for_status()?;
                    total += CHUNK_BYTES as u64;
                    progress.fetch_add(CHUNK_BYTES as u64, Ordering::Relaxed);
                }
                Err(_) => break, // deadline hit mid-upload; the transfer itself doesn't survive cancellation
            }
        }
        Ok(total)
    }

    async fn ping(&self) -> anyhow::Result<Duration> {
        let url = format!("{}/__down?bytes=0", self.base_url);
        let start = Instant::now();
        self.client.get(&url).send().await?.error_for_status()?;
        Ok(start.elapsed())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[tokio::test]
    async fn ping_measures_round_trip_against_mock_server() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/__down"))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(Vec::<u8>::new()))
            .mount(&server)
            .await;

        let transport = CloudflareTransport::with_base_url(server.uri());
        let rtt = transport.ping().await.unwrap();

        assert!(rtt < Duration::from_secs(5));
    }

    #[tokio::test]
    async fn download_stream_counts_received_bytes_until_deadline() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/__down"))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(vec![0u8; 1_000]))
            .mount(&server)
            .await;

        let transport = CloudflareTransport::with_base_url(server.uri());
        let deadline = Instant::now() + Duration::from_millis(500);
        let progress = Arc::new(AtomicU64::new(0));
        let bytes = transport
            .run_download_stream(deadline, progress.clone())
            .await
            .unwrap();

        assert!(bytes > 0, "expected at least one chunk to be received");
        assert_eq!(bytes, progress.load(Ordering::Relaxed));
    }
}
