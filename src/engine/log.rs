//! Журнал приложения: файл data\manager.log + кольцевой буфер для интерфейса.

use std::collections::VecDeque;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum LogLevel {
    Info,
    Ok,
    Warn,
    Error,
}

impl LogLevel {
    pub fn label(&self) -> &'static str {
        match self {
            LogLevel::Info => "INFO",
            LogLevel::Ok => "OK",
            LogLevel::Warn => "WARN",
            LogLevel::Error => "ERROR",
        }
    }
}

#[derive(Debug, Clone)]
pub struct LogLine {
    pub seq: u64,
    pub level: LogLevel,
    pub time: String,
    pub text: String,
}

struct Logger {
    file: Option<PathBuf>,
    lines: VecDeque<LogLine>,
    seq: u64,
}

static LOGGER: OnceLock<Mutex<Logger>> = OnceLock::new();

fn logger() -> &'static Mutex<Logger> {
    LOGGER.get_or_init(|| {
        Mutex::new(Logger {
            file: None,
            lines: VecDeque::new(),
            seq: 0,
        })
    })
}

pub fn set_file(path: PathBuf) {
    if let Ok(mut log) = logger().lock() {
        log.file = Some(path);
    }
}

pub fn write(level: LogLevel, message: &str) {
    let now = super::sys::local_now();
    let line = LogLine {
        seq: 0,
        level,
        time: now.time_string(),
        text: message.to_string(),
    };
    if let Ok(mut log) = logger().lock() {
        // ротация: файл больше 512 КиБ — удаляем
        if let Some(path) = &log.file {
            if let Ok(meta) = std::fs::metadata(path) {
                if meta.len() > 512 * 1024 {
                    let _ = std::fs::remove_file(path);
                }
            }
            if let Ok(mut f) = OpenOptions::new().create(true).append(true).open(path) {
                let _ = writeln!(f, "{}  {}", now, message);
            }
        }
        log.seq += 1;
        let mut entry = line;
        entry.seq = log.seq;
        log.lines.push_back(entry);
        while log.lines.len() > 3000 {
            log.lines.pop_front();
        }
    }
}

pub fn info(m: impl AsRef<str>) {
    write(LogLevel::Info, m.as_ref());
}
pub fn ok(m: impl AsRef<str>) {
    write(LogLevel::Ok, m.as_ref());
}
pub fn warn(m: impl AsRef<str>) {
    write(LogLevel::Warn, m.as_ref());
}
pub fn error(m: impl AsRef<str>) {
    write(LogLevel::Error, m.as_ref());
}

/// Все строки, появившиеся после seq.
pub fn since(seq: u64) -> Vec<LogLine> {
    match logger().lock() {
        Ok(log) => log
            .lines
            .iter()
            .filter(|l| l.seq > seq)
            .cloned()
            .collect(),
        Err(_) => Vec::new(),
    }
}

pub fn last_seq() -> u64 {
    logger().lock().map(|l| l.seq).unwrap_or(0)
}

/// Последние n строк (для стартового заполнения журнала).
pub fn tail(n: usize) -> Vec<LogLine> {
    match logger().lock() {
        Ok(log) => log.lines.iter().rev().take(n).rev().cloned().collect(),
        Err(_) => Vec::new(),
    }
}

/// Форматирование строки как в C#-версии: "HH:MM:SS  текст".
pub fn format_line(line: &LogLine) -> String {
    format!("{}  {}", line.time, line.text)
}
