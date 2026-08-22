//! Domain logic for a Measurement Run: Phases, Streams, Throughput and Idle
//! Latency aggregation. See `CONTEXT.md` at the repo root for the glossary
//! these types implement.

use std::sync::Arc;
use std::sync::atomic::AtomicU64;
use std::time::{Duration, Instant};

use serde::Serialize;

use crate::transport::Transport;

/// Number of Streams run concurrently within a Phase.
pub const STREAM_COUNT: usize = 4;
/// Wall-clock duration of a single Phase.
pub const PHASE_DURATION: Duration = Duration::from_secs(10);
/// Number of RTT Samples taken to compute Idle Latency.
pub const LATENCY_SAMPLES: usize = 5;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    Download,
    Upload,
}

/// The Result of a Measurement Run.
#[derive(Debug, Clone, Copy, Serialize)]
pub struct MeasurementResult {
    pub download_mbps: f64,
    pub upload_mbps: f64,
    pub idle_latency_ms: f64,
}

/// Runs one Phase: `stream_count` Streams concurrently, for `duration`, and
/// returns the Phase's aggregate Throughput in Mbps.
pub async fn run_phase(
    transport: Arc<dyn Transport>,
    phase: Phase,
    duration: Duration,
    stream_count: usize,
    progress: Arc<AtomicU64>,
) -> anyhow::Result<f64> {
    let start = Instant::now();
    let deadline = start + duration;
    let total_bytes = run_phase_bytes(transport, phase, deadline, stream_count, progress).await?;
    Ok(bytes_to_mbps(total_bytes, start.elapsed().as_secs_f64()))
}

/// The concurrency/aggregation core of [`run_phase`], separated out so it can
/// be unit-tested without depending on wall-clock Throughput.
///
/// Uses a [`tokio::task::JoinSet`] rather than a `Vec<JoinHandle>` so that if
/// one Stream errors, the rest can be aborted immediately instead of
/// continuing to transfer data for the remainder of the Phase only to have
/// their results discarded.
async fn run_phase_bytes(
    transport: Arc<dyn Transport>,
    phase: Phase,
    deadline: Instant,
    stream_count: usize,
    progress: Arc<AtomicU64>,
) -> anyhow::Result<u64> {
    let mut streams = tokio::task::JoinSet::new();
    for _ in 0..stream_count {
        let transport = transport.clone();
        let progress = progress.clone();
        streams.spawn(async move {
            match phase {
                Phase::Download => transport.run_download_stream(deadline, progress).await,
                Phase::Upload => transport.run_upload_stream(deadline, progress).await,
            }
        });
    }

    let mut total = 0u64;
    while let Some(result) = streams.join_next().await {
        match result {
            Ok(Ok(bytes)) => total += bytes,
            Ok(Err(err)) => {
                streams.abort_all();
                return Err(err);
            }
            Err(join_err) => {
                streams.abort_all();
                return Err(join_err.into());
            }
        }
    }
    Ok(total)
}

fn bytes_to_mbps(bytes: u64, seconds: f64) -> f64 {
    if seconds <= 0.0 {
        return 0.0;
    }
    (bytes as f64 * 8.0) / seconds / 1_000_000.0
}

/// Measures Idle Latency: `samples` RTT Samples via [`Transport::ping`],
/// reduced to a single value via the median.
pub async fn measure_idle_latency(
    transport: &dyn Transport,
    samples: usize,
) -> anyhow::Result<f64> {
    let mut readings = Vec::with_capacity(samples);
    for _ in 0..samples {
        readings.push(transport.ping().await?.as_secs_f64() * 1000.0);
    }
    Ok(median(&mut readings))
}

/// The median of a slice of readings, used wherever this project reduces
/// several raw measurements to one representative number (see
/// `CONTEXT.md` § Conventions).
fn median(values: &mut [f64]) -> f64 {
    assert!(!values.is_empty(), "median of an empty slice is undefined");
    values.sort_by(|a, b| a.partial_cmp(b).expect("NaN in latency samples"));
    let mid = values.len() / 2;
    if values.len().is_multiple_of(2) {
        (values[mid - 1] + values[mid]) / 2.0
    } else {
        values[mid]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use std::sync::Mutex;
    use std::sync::atomic::Ordering;
    use std::time::Duration;

    #[test]
    fn bytes_to_mbps_converts_bytes_per_second_to_megabits_per_second() {
        // 125,000,000 bytes/s * 8 bits/byte = 1,000,000,000 bits/s = 1000 Mbps
        assert_eq!(bytes_to_mbps(125_000_000, 1.0), 1000.0);
    }

    #[test]
    fn bytes_to_mbps_is_zero_for_zero_elapsed_time() {
        assert_eq!(bytes_to_mbps(1_000_000, 0.0), 0.0);
    }

    #[test]
    fn median_of_odd_length_slice_is_the_middle_value() {
        let mut values = vec![30.0, 10.0, 20.0];
        assert_eq!(median(&mut values), 20.0);
    }

    #[test]
    fn median_of_even_length_slice_averages_the_two_middle_values() {
        let mut values = vec![10.0, 20.0, 30.0, 40.0];
        assert_eq!(median(&mut values), 25.0);
    }

    /// A [`Transport`] whose stream/ping behaviour is fully controlled by the
    /// test, so aggregation logic can be verified without any real network
    /// I/O or wall-clock dependency.
    struct FakeTransport {
        bytes_per_stream_call: u64,
        ping_readings_ms: Mutex<std::collections::VecDeque<f64>>,
    }

    #[async_trait]
    impl Transport for FakeTransport {
        async fn run_download_stream(
            &self,
            _deadline: Instant,
            progress: Arc<AtomicU64>,
        ) -> anyhow::Result<u64> {
            progress.fetch_add(self.bytes_per_stream_call, Ordering::Relaxed);
            Ok(self.bytes_per_stream_call)
        }

        async fn run_upload_stream(
            &self,
            _deadline: Instant,
            progress: Arc<AtomicU64>,
        ) -> anyhow::Result<u64> {
            progress.fetch_add(self.bytes_per_stream_call, Ordering::Relaxed);
            Ok(self.bytes_per_stream_call)
        }

        async fn ping(&self) -> anyhow::Result<Duration> {
            let ms = self
                .ping_readings_ms
                .lock()
                .unwrap()
                .pop_front()
                .expect("test provided fewer ping readings than samples requested");
            Ok(Duration::from_secs_f64(ms / 1000.0))
        }
    }

    #[tokio::test]
    async fn run_phase_bytes_sums_across_all_streams() {
        let transport = Arc::new(FakeTransport {
            bytes_per_stream_call: 1_000,
            ping_readings_ms: Mutex::new(Default::default()),
        });
        let progress = Arc::new(AtomicU64::new(0));

        let total = run_phase_bytes(
            transport,
            Phase::Download,
            Instant::now(),
            4,
            progress.clone(),
        )
        .await
        .unwrap();

        assert_eq!(total, 4_000);
        assert_eq!(progress.load(Ordering::Relaxed), 4_000);
    }

    #[tokio::test]
    async fn measure_idle_latency_reduces_samples_via_median() {
        let transport = FakeTransport {
            bytes_per_stream_call: 0,
            ping_readings_ms: Mutex::new(vec![10.0, 50.0, 20.0, 30.0, 40.0].into()),
        };

        let latency = measure_idle_latency(&transport, 5).await.unwrap();

        assert_eq!(latency, 30.0);
    }
}
