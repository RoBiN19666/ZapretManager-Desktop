//! Пути приложения: папка exe, data, temp, manager.ini, manager.log.

use std::path::{Path, PathBuf};
use std::sync::OnceLock;

static EXE_DIR: OnceLock<PathBuf> = OnceLock::new();
static DATA_DIR: OnceLock<PathBuf> = OnceLock::new();
static TEMP_DIR: OnceLock<PathBuf> = OnceLock::new();

pub fn exe_path() -> PathBuf {
    std::env::current_exe().unwrap_or_else(|_| PathBuf::from("ZapretManager.exe"))
}

/// Полный путь без завершающего «\» (корень диска сохраняется).
pub fn normalize_root(path: &str) -> String {
    if path.trim().is_empty() {
        return String::new();
    }
    let p = Path::new(path);
    let full = match p.canonicalize() {
        Ok(p) => p,
        Err(_) => p.to_path_buf(),
    };
    let mut s = full.to_string_lossy().to_string();
    if s.starts_with(r"\\?\") {
        s = s[4..].to_string();
    }
    while s.len() > 3 && s.ends_with('\\') {
        s.pop();
    }
    s
}

pub fn exe_dir() -> PathBuf {
    EXE_DIR
        .get_or_init(|| {
            exe_path()
                .parent()
                .map(|p| p.to_path_buf())
                .unwrap_or_else(|| PathBuf::from("."))
        })
        .clone()
}

/// Папка данных: рядом с exe\data, иначе %APPDATA%\ZapretManager.
pub fn data_dir() -> PathBuf {
    DATA_DIR
        .get_or_init(|| {
            let candidate = exe_dir().join("data");
            let writable = std::fs::create_dir_all(&candidate).is_ok()
                && std::fs::write(candidate.join(".write-test"), b"ok").is_ok();
            if writable {
                let _ = std::fs::remove_file(candidate.join(".write-test"));
                return candidate;
            }
            let appdata = std::env::var("APPDATA").unwrap_or_else(|_| ".".to_string());
            let fallback = PathBuf::from(appdata).join("ZapretManager");
            let _ = std::fs::create_dir_all(&fallback);
            fallback
        })
        .clone()
}

pub fn temp_dir() -> PathBuf {
    TEMP_DIR
        .get_or_init(|| {
            let dir = std::env::temp_dir().join("ZapretManager");
            let _ = std::fs::create_dir_all(&dir);
            dir
        })
        .clone()
}

pub fn config_path() -> PathBuf {
    data_dir().join("manager.ini")
}

pub fn log_path() -> PathBuf {
    data_dir().join("manager.log")
}

/// Папка с ресурсами: assets рядом с exe (для иконок), иначе встроенные данные.
pub fn assets_dir() -> PathBuf {
    exe_dir().join("assets")
}
