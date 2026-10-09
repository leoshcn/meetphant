//! File logging via `tracing`. See `.trellis/spec/backend/logging-guidelines.md`.

use std::path::Path;
use std::sync::OnceLock;

use tracing_appender::non_blocking::WorkerGuard;
use tracing_appender::rolling::{Builder, Rotation};
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::{fmt, EnvFilter};

/// Overrides the default filter, e.g. `MEETPHANT_LOG=meetphant_lib=trace`.
const FILTER_ENV: &str = "MEETPHANT_LOG";
const FILE_PREFIX: &str = "meetphant";
const MAX_LOG_FILES: usize = 7;

/// Keeps the non-blocking writer alive for the whole process so buffered lines flush.
static FILE_GUARD: OnceLock<WorkerGuard> = OnceLock::new();

fn default_filter() -> EnvFilter {
    let own = if cfg!(debug_assertions) {
        "debug"
    } else {
        "info"
    };
    EnvFilter::try_from_env(FILTER_ENV)
        .unwrap_or_else(|_| EnvFilter::new(format!("warn,meetphant_lib={own}")))
}

/// Install the global subscriber: daily-rotated files in `log_dir` (7 kept), plus stderr
/// in debug builds. Never fails app startup: if the directory is unusable, falls back
/// to stderr only.
pub fn init(log_dir: &Path) {
    let file_writer = std::fs::create_dir_all(log_dir)
        .ok()
        .and_then(|_| {
            Builder::new()
                .rotation(Rotation::DAILY)
                .filename_prefix(FILE_PREFIX)
                .filename_suffix("log")
                .max_log_files(MAX_LOG_FILES)
                .build(log_dir)
                .ok()
        })
        .map(|appender| {
            let (writer, guard) = tracing_appender::non_blocking(appender);
            let _ = FILE_GUARD.set(guard);
            writer
        });

    let file_ok = file_writer.is_some();
    let file_layer = file_writer.map(|w| fmt::layer().with_writer(w).with_ansi(false));
    let stderr_layer =
        (cfg!(debug_assertions) || !file_ok).then(|| fmt::layer().with_writer(std::io::stderr));

    let installed = tracing_subscriber::registry()
        .with(default_filter())
        .with(file_layer)
        .with(stderr_layer)
        .try_init()
        .is_ok();

    if !file_ok {
        eprintln!("meetphant: log directory unavailable; logging to stderr only");
    }
    if installed {
        tracing::info!(
            version = env!("CARGO_PKG_VERSION"),
            file_ok,
            "logging initialized"
        );
    }
}
