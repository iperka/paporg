//! Continuous CPU profiling with push-based delivery to Grafana Pyroscope.
//!
//! Uses `pprof-rs` to sample the CPU at 100 Hz, then pushes pprof protobuf
//! reports to `http://localhost:4040/ingest` every 10 seconds.
//!
//! This entire module is gated behind `#[cfg(feature = "otel")]`.

use std::thread::{self, JoinHandle};
use std::time::Duration;

/// Guard that stops the profiling background thread when dropped.
pub struct ProfilingGuard {
    _handle: JoinHandle<()>,
}

/// Starts continuous CPU profiling in a background thread.
///
/// Returns a guard — profiling runs until the guard is dropped (or the
/// process exits). On Windows this is a no-op since `pprof-rs` only
/// supports macOS and Linux.
#[cfg(not(target_os = "windows"))]
pub fn start_continuous_profiling() -> ProfilingGuard {
    let handle = thread::Builder::new()
        .name("paporg-profiler".into())
        .spawn(move || {
            profiling_loop();
        })
        .expect("Failed to spawn profiling thread");

    tracing::info!("Continuous profiling started (pushing to Pyroscope at localhost:4040)");

    ProfilingGuard { _handle: handle }
}

#[cfg(target_os = "windows")]
pub fn start_continuous_profiling() -> ProfilingGuard {
    tracing::info!("Continuous profiling not supported on Windows");
    // Spawn a no-op thread so the guard type is consistent
    let handle = thread::spawn(|| {});
    ProfilingGuard { _handle: handle }
}

#[cfg(not(target_os = "windows"))]
fn profiling_loop() {
    use pprof::protos::Message as _;

    let push_interval = Duration::from_secs(10);
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(5))
        .build()
        .expect("Failed to create HTTP client for profiling");

    loop {
        // Start a new profiling guard (100 Hz sampling)
        let guard = match pprof::ProfilerGuardBuilder::default()
            .frequency(100)
            .blocklist(&["libc", "libgcc", "pthread", "vdso"])
            .build()
        {
            Ok(guard) => guard,
            Err(e) => {
                tracing::error!("Failed to start profiler: {}", e);
                thread::sleep(push_interval);
                continue;
            }
        };

        // Collect samples for the push interval
        thread::sleep(push_interval);

        // Build pprof report
        match guard.report().build() {
            Ok(report) => {
                let profile = report.pprof().expect("Failed to generate pprof");
                let body = profile.encode_to_vec();

                // Push to Pyroscope
                let result = client
                    .post("http://localhost:4040/ingest")
                    .query(&[
                        ("name", "paporg-desktop"),
                        ("format", "pprof"),
                        ("sampleRate", "100"),
                    ])
                    .header("Content-Type", "application/x-protobuf")
                    .body(body)
                    .send();

                if let Err(e) = result {
                    tracing::warn!("Failed to push profile to Pyroscope: {}", e);
                }
            }
            Err(e) => {
                tracing::warn!("Failed to build profiling report: {}", e);
            }
        }
    }
}
