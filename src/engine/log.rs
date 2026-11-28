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
    /// Открытый дескриптор журнала: не открываем файл на каждую строку.
    handle: Option<std::fs::File>,
    /// Сколько байт уже записано в текущий файл (для ротации без `metadata`).
    written: u64,
    lines: VecDeque<LogLine>,
    seq: u64,
}

static LOGGER: OnceLock<Mutex<Logger>> = OnceLock::new();

fn logger() -> &'static Mutex<Logger> {
    LOGGER.get_or_init(|| {
        Mutex::new(Logger {
            file: None,
            handle: None,
            written: 0,
            lines: VecDeque::new(),
            seq: 0,
        })
    })
}

pub fn set_file(path: PathBuf) {
    // Отравленный мьютекс не должен навсегда отключать журнал.
    let mut log = logger().lock().unwrap_or_else(|e| e.into_inner());
    log.file = Some(path);
    log.handle = None;
    log.written = 0;
}

pub fn write(level: LogLevel, message: &str) {
    let now = super::sys::local_now();
    let line = LogLine {
        seq: 0,
        level,
        time: now.time_string(),
        text: message.to_string(),
    };
    let mut log = logger().lock().unwrap_or_else(|e| e.into_inner());
    if let Some(path) = log.file.clone() {
        if log.handle.is_none() {
            log.handle = OpenOptions::new().create(true).append(true).open(&path).ok();
            log.written = 0;
        }
        if let Some(file) = log.handle.as_mut() {
            let bytes = format!("{}  {}\n", now, message);
            if file.write_all(bytes.as_bytes()).is_ok() {
                log.written += bytes.len() as u64;
            }
            // ротация: файл больше 512 КиБ — сохраняем как `.1`, а не удаляем
            if log.written > 512 * 1024 {
                log.handle = None;
                let mut previous = path.clone();
                previous.set_extension("log.1");
                let _ = std::fs::remove_file(&previous);
                let _ = std::fs::rename(&path, &previous);
                log.written = 0;
            }
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
    logger()
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .lines
        .iter()
        .filter(|l| l.seq > seq)
        .cloned()
        .collect()
}

pub fn last_seq() -> u64 {
    logger()
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .seq
}

/// Последние n строк (для стартового заполнения журнала).
pub fn tail(n: usize) -> Vec<LogLine> {
    logger()
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .lines
        .iter()
        .rev()
        .take(n)
        .rev()
        .cloned()
        .collect()
}

/// Форматирование строки журнала: "HH:MM:SS  текст".
pub fn format_line(line: &LogLine) -> String {
    format!("{}  {}", line.time, line.text)
}
