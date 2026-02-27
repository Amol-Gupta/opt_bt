use chrono::{DateTime, Local};
use log::{LevelFilter, Log, Metadata, Record};
use std::fs::{File, OpenOptions};
use std::io::Write;
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::{Mutex, Once};

/// Global simulation time in Unix timestamp (seconds).
/// Used by the logger to emit simulation time instead of wall clock time.
pub static SIMULATION_TIME: AtomicI64 = AtomicI64::new(0);

pub struct LoggerGuard {
    _private: (),
}

#[derive(Clone, Copy)]
enum TimeMode {
    Simulation,
    Wall,
}

struct ModeLogger {
    level: LevelFilter,
    time_mode: TimeMode,
    file: Option<Mutex<File>>,
}

impl Log for ModeLogger {
    fn enabled(&self, metadata: &Metadata) -> bool {
        metadata.level() <= self.level
    }

    fn log(&self, record: &Record) {
        if !self.enabled(record.metadata()) {
            return;
        }

        let ts_label = match self.time_mode {
            TimeMode::Simulation => {
                let sim_time = SIMULATION_TIME.load(Ordering::Relaxed);
                DateTime::from_timestamp(sim_time, 0)
                    .map(|ts| ts.format("%Y-%m-%d %H:%M:%S").to_string())
                    .unwrap_or_else(|| "1970-01-01 00:00:00".to_string())
            }
            TimeMode::Wall => Local::now().format("%Y-%m-%d %H:%M:%S").to_string(),
        };

        let time_prefix = match self.time_mode {
            TimeMode::Simulation => "SIM",
            TimeMode::Wall => "WALL",
        };

        let line = format!(
            "{}[{}] {} [{}:{}] {}",
            time_prefix,
            ts_label,
            record.level(),
            record.file().unwrap_or(""),
            record.line().unwrap_or(0),
            record.args()
        );

        eprintln!("{}", line);
        if let Some(file_lock) = &self.file {
            if let Ok(mut file) = file_lock.lock() {
                let _ = writeln!(file, "{}", line);
                let _ = file.flush();
            }
        }
    }

    fn flush(&self) {}
}

static LOGGER_INIT: Once = Once::new();

fn parse_time_mode(value: &str) -> TimeMode {
    if value.eq_ignore_ascii_case("wall") {
        TimeMode::Wall
    } else {
        TimeMode::Simulation
    }
}

pub fn init(level: &str) -> LoggerGuard {
    init_with_time_mode_and_file(level, None, None)
}

pub fn init_with_time_mode(level: &str, mode: Option<&str>) -> LoggerGuard {
    init_with_time_mode_and_file(level, mode, None)
}

pub fn init_with_time_mode_and_file(
    level: &str,
    mode: Option<&str>,
    log_file: Option<&str>,
) -> LoggerGuard {
    // Determine log level
    let level_filter = match level.to_lowercase().as_str() {
        "trace" => LevelFilter::Trace,
        "debug" => LevelFilter::Debug,
        "warn" => LevelFilter::Warn,
        "error" => LevelFilter::Error,
        _ => LevelFilter::Info,
    };

    let time_mode = parse_time_mode(mode.unwrap_or("simulation"));
    let file_handle = log_file.and_then(|path| {
        OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
            .ok()
            .map(Mutex::new)
    });

    LOGGER_INIT.call_once(|| {
        let logger = ModeLogger {
            level: level_filter,
            time_mode,
            file: file_handle,
        };
        let _ = log::set_boxed_logger(Box::new(logger));
    });
    log::set_max_level(level_filter);

    LoggerGuard { _private: () }
}

/// update the global simulation time
pub fn set_simulation_time(ts: i64) {
    SIMULATION_TIME.store(ts, Ordering::Relaxed);
}
