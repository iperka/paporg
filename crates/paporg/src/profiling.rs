//! Continuous CPU profiling with push-based delivery to Grafana Pyroscope.
//!
//! Uses `pprof-rs` to sample the CPU at 100 Hz, then pushes pprof protobuf
//! reports to `http://localhost:4040/ingest` every 10 seconds.
//!
//! This entire module is gated behind `#[cfg(feature = "otel")]`.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::Duration;

/// Guard that stops the profiling background thread when dropped.
pub struct ProfilingGuard {
    shutdown: Arc<AtomicBool>,
    handle: Option<JoinHandle<()>>,
}

impl Drop for ProfilingGuard {
    fn drop(&mut self) {
        self.shutdown.store(true, Ordering::Release);
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

/// Starts continuous CPU profiling in a background thread.
///
/// Returns a guard — profiling runs until the guard is dropped (or the
/// process exits). On Windows this is a no-op since `pprof-rs` only
/// supports macOS and Linux.
#[cfg(not(target_os = "windows"))]
pub fn start_continuous_profiling() -> ProfilingGuard {
    let shutdown = Arc::new(AtomicBool::new(false));
    let shutdown_flag = Arc::clone(&shutdown);

    let handle = thread::Builder::new()
        .name("paporg-profiler".into())
        .spawn(move || {
            profiling_loop(shutdown_flag);
        })
        .expect("Failed to spawn profiling thread");

    tracing::info!("Continuous profiling started (pushing to Pyroscope at localhost:4040)");

    ProfilingGuard {
        shutdown,
        handle: Some(handle),
    }
}

#[cfg(target_os = "windows")]
pub fn start_continuous_profiling() -> ProfilingGuard {
    tracing::info!("Continuous profiling not supported on Windows");
    ProfilingGuard {
        shutdown: Arc::new(AtomicBool::new(true)),
        handle: None,
    }
}

#[cfg(not(target_os = "windows"))]
fn profiling_loop(shutdown: Arc<AtomicBool>) {
    use pprof::protos::Message as _;

    let push_interval = Duration::from_secs(10);
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(5))
        .build()
        .expect("Failed to create HTTP client for profiling");

    loop {
        if shutdown.load(Ordering::Acquire) {
            tracing::info!("Profiling loop shutting down");
            return;
        }

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
                let profile = match report.pprof() {
                    Ok(p) => p,
                    Err(e) => {
                        tracing::warn!("Failed to generate pprof: {}", e);
                        continue;
                    }
                };
                let body = profile.encode_to_vec();

                // Push to Pyroscope
                match client
                    .post("http://localhost:4040/ingest")
                    .query(&[
                        ("name", "paporg-desktop"),
                        ("format", "pprof"),
                        ("sampleRate", "100"),
                    ])
                    .header("Content-Type", "application/x-protobuf")
                    .body(body)
                    .send()
                {
                    Ok(resp) => {
                        if let Err(e) = resp.error_for_status() {
                            tracing::warn!("Pyroscope rejected profile: {}", e);
                        }
                    }
                    Err(e) => {
                        tracing::warn!("Failed to push profile to Pyroscope: {}", e);
                    }
                }
            }
            Err(e) => {
                tracing::warn!("Failed to build profiling report: {}", e);
            }
        }
    }
}
