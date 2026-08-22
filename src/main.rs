use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use clap::Parser;
use indicatif::{ProgressBar, ProgressStyle};

use nst::measurement::{
    self, LATENCY_SAMPLES, MeasurementResult, PHASE_DURATION, Phase, STREAM_COUNT,
};
use nst::{CloudflareTransport, Transport};

/// A lightweight CLI network speed test.
#[derive(Parser)]
#[command(name = "nst", version, about)]
struct Cli {
    /// Print the result as JSON instead of human-readable text.
    #[arg(long)]
    json: bool,
}

#[tokio::main]
async fn main() {
    if let Err(err) = run().await {
        eprintln!("nst: error: {err:#}");
        std::process::exit(1);
    }
}

async fn run() -> anyhow::Result<()> {
    let cli = Cli::parse();
    let transport: Arc<dyn Transport> = Arc::new(CloudflareTransport::new()?);

    let result = measure(transport).await?;

    if cli.json {
        println!("{}", serde_json::to_string_pretty(&result)?);
    } else {
        println!("Download: {:>8.2} Mbps", result.download_mbps);
        println!("Upload:   {:>8.2} Mbps", result.upload_mbps);
        println!("Latency:  {:>8.1} ms", result.idle_latency_ms);
    }

    Ok(())
}

async fn measure(transport: Arc<dyn Transport>) -> anyhow::Result<MeasurementResult> {
    let spinner = ProgressBar::new_spinner();
    spinner.set_style(ProgressStyle::with_template("{spinner} {msg}").unwrap());
    spinner.enable_steady_tick(Duration::from_millis(100));
    spinner.set_message("Measuring latency...");

    let idle_latency_ms =
        measurement::measure_idle_latency(transport.as_ref(), LATENCY_SAMPLES).await?;
    spinner.finish_and_clear();

    let download_mbps =
        run_phase_with_progress(transport.clone(), Phase::Download, "Download").await?;
    let upload_mbps = run_phase_with_progress(transport, Phase::Upload, "Upload").await?;

    Ok(MeasurementResult {
        download_mbps,
        upload_mbps,
        idle_latency_ms,
    })
}

/// Runs one Phase while driving an indicatif progress bar off the Phase's
/// shared byte counter — the bar's position tracks elapsed time against
/// `PHASE_DURATION`, and its message shows the running Mbps rate.
async fn run_phase_with_progress(
    transport: Arc<dyn Transport>,
    phase: Phase,
    label: &str,
) -> anyhow::Result<f64> {
    let total_ms = PHASE_DURATION.as_millis() as u64;
    let bar = ProgressBar::new(total_ms);
    bar.set_style(
        ProgressStyle::with_template("{prefix:9} [{bar:30.cyan/blue}] {msg}")
            .unwrap()
            .progress_chars("=> "),
    );
    bar.set_prefix(label.to_string());

    let progress = Arc::new(AtomicU64::new(0));
    let start = Instant::now();

    let ticker_bar = bar.clone();
    let ticker_progress = progress.clone();
    let ticker = tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_millis(150));
        loop {
            interval.tick().await;
            let elapsed = start.elapsed();
            let bytes = ticker_progress.load(Ordering::Relaxed);
            let mbps = (bytes as f64 * 8.0) / elapsed.as_secs_f64().max(0.001) / 1_000_000.0;
            ticker_bar.set_position((elapsed.as_millis() as u64).min(total_ms));
            ticker_bar.set_message(format!("{mbps:.1} Mbps"));
        }
    });

    let result =
        measurement::run_phase(transport, phase, PHASE_DURATION, STREAM_COUNT, progress).await;
    ticker.abort();
    let mbps = result?;

    bar.set_position(total_ms);
    bar.finish_with_message(format!("{mbps:.2} Mbps"));

    Ok(mbps)
}
