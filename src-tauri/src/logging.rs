use crate::redact::redact;
use chrono::{SecondsFormat, Utc};
use log::{Level, LevelFilter, Log, Metadata, Record};
use serde::Serialize;
use std::collections::VecDeque;
use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Mutex, OnceLock};

const TAIL_CAPACITY: usize = 400;
const KEEP_FILES: usize = 10;

#[derive(Debug, Clone, Serialize)]
pub struct LogEntry {
    pub ts: String,
    pub level: &'static str,
    pub area: String,
    pub msg: String,
}

struct Sink {
    dir: PathBuf,
    day: String,
    file: Option<File>,
}

pub struct Logger {
    level: AtomicUsize,
    sink: Mutex<Sink>,
    tail: Mutex<VecDeque<LogEntry>>,
}

static LOGGER: OnceLock<Logger> = OnceLock::new();

fn level_to_usize(l: LevelFilter) -> usize {
    l as usize
}

fn usize_to_level(v: usize) -> LevelFilter {
    match v {
        0 => LevelFilter::Off,
        1 => LevelFilter::Error,
        2 => LevelFilter::Warn,
        3 => LevelFilter::Info,
        4 => LevelFilter::Debug,
        _ => LevelFilter::Trace,
    }
}

pub fn init(dir: &Path, level: LevelFilter) {
    let logger = LOGGER.get_or_init(|| Logger {
        level: AtomicUsize::new(level_to_usize(level)),
        sink: Mutex::new(Sink { dir: dir.to_path_buf(), day: String::new(), file: None }),
        tail: Mutex::new(VecDeque::with_capacity(TAIL_CAPACITY)),
    });
    if log::set_logger(logger).is_ok() {
        log::set_max_level(LevelFilter::Trace);
    }
    prune(dir, KEEP_FILES);
}

pub fn set_level(level: LevelFilter) {
    if let Some(l) = LOGGER.get() {
        l.level.store(level_to_usize(level), Ordering::Relaxed);
    }
}

pub fn tail(limit: usize) -> Vec<LogEntry> {
    LOGGER
        .get()
        .map(|l| {
            let t = l.tail.lock().unwrap_or_else(|e| e.into_inner());
            t.iter().skip(t.len().saturating_sub(limit)).cloned().collect()
        })
        .unwrap_or_default()
}

pub fn parse_level(s: &str) -> LevelFilter {
    match s.to_ascii_lowercase().as_str() {
        "error" => LevelFilter::Error,
        "warn" => LevelFilter::Warn,
        "debug" => LevelFilter::Debug,
        "trace" => LevelFilter::Trace,
        _ => LevelFilter::Info,
    }
}

fn area(target: &str) -> String {
    target.trim_start_matches("vesper_lib::").trim_start_matches("vesper::").replace("::", ".")
}

fn level_name(l: Level) -> &'static str {
    match l {
        Level::Error => "ERROR",
        Level::Warn => "WARN",
        Level::Info => "INFO",
        Level::Debug => "DEBUG",
        Level::Trace => "TRACE",
    }
}

impl Logger {
    fn accepts(&self, metadata: &Metadata) -> bool {
        let ours = metadata.target().starts_with("vesper");
        let limit = if ours { usize_to_level(self.level.load(Ordering::Relaxed)) } else { LevelFilter::Warn };
        metadata.level() <= limit
    }
}

impl Log for Logger {
    fn enabled(&self, metadata: &Metadata) -> bool {
        self.accepts(metadata)
    }

    fn log(&self, record: &Record) {
        if !self.accepts(record.metadata()) {
            return;
        }
        let now = Utc::now();
        let entry = LogEntry {
            ts: now.to_rfc3339_opts(SecondsFormat::Millis, true),
            level: level_name(record.level()),
            area: area(record.target()),
            msg: redact(&record.args().to_string()),
        };

        if cfg!(debug_assertions) {
            eprintln!("{} {:5} {} {}", entry.ts, entry.level, entry.area, entry.msg);
        }

        if let Ok(line) = serde_json::to_string(&entry) {
            let mut sink = self.sink.lock().unwrap_or_else(|e| e.into_inner());
            let day = now.format("%Y-%m-%d").to_string();
            if sink.day != day || sink.file.is_none() {
                let path = sink.dir.join(format!("vesper-{day}.log"));
                let _ = std::fs::create_dir_all(&sink.dir);
                sink.file = OpenOptions::new().create(true).append(true).open(path).ok();
                sink.day = day;
            }
            if let Some(f) = sink.file.as_mut() {
                let _ = writeln!(f, "{line}");
            }
        }

        let mut tail = self.tail.lock().unwrap_or_else(|e| e.into_inner());
        if tail.len() == TAIL_CAPACITY {
            tail.pop_front();
        }
        tail.push_back(entry);
    }

    fn flush(&self) {
        if let Ok(mut sink) = self.sink.lock() {
            if let Some(f) = sink.file.as_mut() {
                let _ = f.flush();
            }
        }
    }
}

pub fn prune(dir: &Path, keep: usize) {
    let Ok(read) = std::fs::read_dir(dir) else { return };
    let mut files: Vec<PathBuf> = read
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| {
            p.file_name().and_then(|n| n.to_str()).is_some_and(|n| n.starts_with("vesper-") && n.ends_with(".log"))
        })
        .collect();
    files.sort();
    if files.len() > keep {
        for old in &files[..files.len() - keep] {
            let _ = std::fs::remove_file(old);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn area_strips_crate_prefix() {
        assert_eq!(area("vesper_lib::roblox::launch"), "roblox.launch");
        assert_eq!(area("tao::platform"), "tao.platform");
    }

    #[test]
    fn prune_keeps_newest_files() {
        let dir = tempfile::tempdir().unwrap();
        for d in ["2026-01-01", "2026-01-02", "2026-01-03", "2026-01-04"] {
            std::fs::write(dir.path().join(format!("vesper-{d}.log")), "x").unwrap();
        }
        std::fs::write(dir.path().join("unrelated.txt"), "keep me").unwrap();
        prune(dir.path(), 2);
        let mut left: Vec<String> = std::fs::read_dir(dir.path())
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        left.sort();
        assert_eq!(left, vec!["unrelated.txt", "vesper-2026-01-03.log", "vesper-2026-01-04.log"]);
    }

    #[test]
    fn level_parsing_defaults_to_info() {
        assert_eq!(parse_level("DEBUG"), LevelFilter::Debug);
        assert_eq!(parse_level("nonsense"), LevelFilter::Info);
    }
}
