use ftlog::{appender::FileAppender, Logger, LevelFilter, Record};
use log::Log; // Added trait import
use chrono::Local;
use std::sync::atomic::{AtomicI64, Ordering};

/// Global simulation time in Unix timestamp (seconds).
/// Used by the logger to emit simulation time instead of wall clock time.
pub static SIMULATION_TIME: AtomicI64 = AtomicI64::new(0);

pub struct LoggerGuard {
    _guard: ftlog::LoggerGuard,
}

pub fn init(level: &str) -> LoggerGuard {
    // Determine log level
    let level_filter = match level.to_lowercase().as_str() {
        "trace" => LevelFilter::Trace,
        "debug" => LevelFilter::Debug,
        "warn" => LevelFilter::Warn,
        "error" => LevelFilter::Error,
        _ => LevelFilter::Info,
    };

    // Configure logger with custom format for simulation time
    // Custom format temporarily disabled due to edition2024/dependency issues
    let logger = ftlog::builder()
        /*.format(move |record: &Record, logger: &Logger| {
             // ...
        })*/
        .max_log_level(level_filter)
        .try_init()
        .expect("Failed to initialize logger");

    LoggerGuard { _guard: logger }
}

/// update the global simulation time
pub fn set_simulation_time(ts: i64) {
    SIMULATION_TIME.store(ts, Ordering::Relaxed);
}
