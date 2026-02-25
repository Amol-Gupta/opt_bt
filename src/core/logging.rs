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
    let logger = ftlog::builder()
        // Use a custom formatter to inject simulation time
        .format(move |record: &Record, logger: &Logger| {
            let sim_time = SIMULATION_TIME.load(Ordering::Relaxed);
            let dt = chrono::DateTime::from_timestamp(sim_time, 0)
                .map(|t| t.format("%Y-%m-%d %H:%M:%S").to_string())
                .unwrap_or_else(|| "1970-01-01 00:00:00".to_string());
                
            let msg = format!(
                "{} [{}] - {}\n",
                dt,
                record.level(),
                record.args()
            );
            // ftlog::Logger doesn't have a check_level or similar method exposed directly usually?
            // Actually FtLogFormat trait expects the closure to return string or write to something?
            // ftlog 0.2.x: .format(...) -> takes closure (Record, Logger)
            // But how to "log"? logger.log()?
            // logger.log(&msg) is correct IF we import Log trait from `log` crate IF ftlog::Logger implements it.
            // ftlog::Logger documentation says: "call logger.log(&record)"? No.
            // Wait, looking at ftlog examples.
            // Usually we just return output? No, previous version assumed logger.log call.
            // If I look at source code of ftlog (I can't).
            // But error says: `trait Log which provides log is implemented but not in scope`.
            // So `use log::Log;` is needed.
            // But `ftlog::Logger` implements `log::Log`?
            // I'll add `use log::Log;` to imports.
            logger.log(
                &log::Record::builder()
                    .args(format_args!("{}", msg.trim()))
                    .level(record.level())
                    .target(record.target())
                    .module_path(record.module_path())
                    .file(record.file())
                    .line(record.line())
                    .build()
            );
            // Wait, this is recursive? No, `logger` is the underlying logger?
            // The formatter is supposed to format the message.
            // ftlog docs: `format<F>(self, format: F) -> Builder`. `F: Fn(&Record, &Logger) + Send + Sync + 'static`.
            // It doesn't return anything. It lets you write to the logger directly?
            // If I use `logger.log(...)`, I am logging a new record?
            // If I want to write raw string?
            // Has ftlog changed significantly?
            // `ftlog` 0.2.14: `logger` arg is `&Logger`.
            // The closure is executed in the background thread.
            // The intention of `format` is to customize the output string.
            // But how?
            // Maybe `logger` has a method `send` or `write`?
            // Or maybe I am misusing `format`.
            // Usually formatters return a String.
            
            // Let's assume `ftlog` 0.2 format logic allows `logger.log(&msg)` if msg is string? No.
            // The error says "no method named log". Suggests `use log::Log`.
            // But `ftlog` is distinct from `log` crate although it bridges.
            
            // Let's try `use log::Log;` at top.
        })
        .max_log_level(level_filter)
        .try_init()
        .expect("Failed to initialize logger");

    LoggerGuard { _guard: logger }
}

/// update the global simulation time
pub fn set_simulation_time(ts: i64) {
    SIMULATION_TIME.store(ts, Ordering::Relaxed);
}
