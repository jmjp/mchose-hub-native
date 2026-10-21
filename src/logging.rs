//! Logging: file log (like the original's daily logs) plus an in-app ring buffer
//! that the Settings/Log page renders.

use std::collections::VecDeque;
use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};

use log::{Level, LevelFilter, Log, Metadata, Record};

/// One captured log line.
#[derive(Debug, Clone)]
pub struct LogEntry {
    pub level: Level,
    pub target: String,
    pub message: String,
}

/// Maximum number of entries kept for the in-app viewer.
const RING_CAPACITY: usize = 512;

struct AppLogger {
    file: Mutex<Option<File>>,
    ring: Mutex<VecDeque<LogEntry>>,
    path: PathBuf,
}

static LOGGER: OnceLock<AppLogger> = OnceLock::new();

impl Log for AppLogger {
    fn enabled(&self, metadata: &Metadata) -> bool {
        metadata.level() <= log::max_level()
    }

    fn log(&self, record: &Record) {
        if !self.enabled(record.metadata()) {
            return;
        }
        let entry = LogEntry {
            level: record.level(),
            target: record.target().to_string(),
            message: record.args().to_string(),
        };
        if let Ok(mut ring) = self.ring.lock() {
            if ring.len() == RING_CAPACITY {
                ring.pop_front();
            }
            ring.push_back(entry.clone());
        }
        if let Ok(mut file) = self.file.lock() {
            if let Some(file) = file.as_mut() {
                let _ = writeln!(
                    file,
                    "[{}] [{:<5}] {}: {}",
                    chrono_like_timestamp(),
                    entry.level,
                    entry.target,
                    entry.message
                );
                let _ = file.flush();
            }
        }
    }

    fn flush(&self) {
        if let Ok(mut file) = self.file.lock() {
            if let Some(file) = file.as_mut() {
                let _ = file.flush();
            }
        }
    }
}

/// Initialise the global logger, returning the log file path (if one was opened).
pub fn init(level: &str) -> Option<PathBuf> {
    let level = match level.to_ascii_lowercase().as_str() {
        "error" => LevelFilter::Error,
        "warn" => LevelFilter::Warn,
        "info" => LevelFilter::Info,
        "debug" => LevelFilter::Debug,
        "trace" => LevelFilter::Trace,
        _ => LevelFilter::Info,
    };

    let dir = crate::config::log_dir();
    let _ = std::fs::create_dir_all(&dir);
    let path = dir.join("mchose-hub-native.log");
    let file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .ok();

    let logger = AppLogger {
        file: Mutex::new(file),
        ring: Mutex::new(VecDeque::new()),
        path: path.clone(),
    };
    if LOGGER.set(logger).is_err() {
        // Logger already initialised (e.g. tests); keep the existing one.
        return Some(path);
    }
    let _ = log::set_logger(LOGGER.get().expect("logger just set"));
    log::set_max_level(level);
    Some(path)
}

/// Snapshot of the in-app log buffer.
pub fn entries() -> Vec<LogEntry> {
    LOGGER
        .get()
        .and_then(|logger| logger.ring.lock().ok().map(|ring| ring.iter().cloned().collect()))
        .unwrap_or_default()
}

/// Path of the log file on disk.
pub fn log_file() -> Option<PathBuf> {
    LOGGER.get().map(|logger| logger.path.clone())
}

fn chrono_like_timestamp() -> String {
    // Avoid a chrono dependency: format seconds since the Unix epoch as a stable
    // monotonic stamp. Human-friendly local time is rendered in the app header instead.
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    format!("{secs}")
}
