//! Аудит fake-файлов (blob) winws и докачивание отсутствующих.
//!
//! Стратегии ссылаются на `*.bin` через `--dpi-desync-fake-*`,
//! `--dpi-desync-split-seqovl-pattern` и `--dpi-desync-fakedsplit-pattern`.
//! Если файла нет, winws.exe завершается с ошибкой до создания WinDivert.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use super::bat;
use super::gamefilter::GameFilterState;
use super::http;
use super::install::ZapretInstall;
use super::log;
use super::paths;

#[derive(Debug, Clone)]
pub struct BlobRef {
    pub key: String,
    pub path: PathBuf,
    pub strategy: String,
}

#[derive(Debug, Clone, Default)]
pub struct BlobAudit {
    pub referenced: Vec<BlobRef>,
    pub missing: Vec<BlobRef>,
}

impl BlobAudit {
    pub fn is_ok(&self) -> bool {
        self.missing.is_empty()
    }

    pub fn missing_names(&self) -> Vec<String> {
        let mut names: Vec<String> = Vec::new();
        for blob in &self.missing {
            let name = blob
                .path
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| blob.path.to_string_lossy().to_string());
            if !names.iter().any(|n| n.eq_ignore_ascii_case(&name)) {
                names.push(name);
            }
        }
        names
    }
}

fn is_blob_key(key: &str) -> bool {
    if key.eq_ignore_ascii_case("--dpi-desync-split-seqovl-pattern")
        || key.eq_ignore_ascii_case("--dpi-desync-fakedsplit-pattern")
    {
        return true;
    }
    key.starts_with("--dpi-desync-fake-") && !key.ends_with("-mod")
}

fn blob_path(install: &ZapretInstall, value: &str) -> PathBuf {
    let path = Path::new(value);
    if path.is_absolute() {
        return path.to_path_buf();
    }
    if value.starts_with("bin\\") || value.starts_with("bin/") || value.starts_with("files\\") {
        Path::new(&install.root).join(value)
    } else {
        install.bin_dir().join(value)
    }
}

/// Собрать все blob-ссылки стратегий и проверить наличие файлов.
pub fn audit(install: &ZapretInstall) -> BlobAudit {
    let mut audit = BlobAudit::default();
    let gf = GameFilterState::load(&install.root);
    let mut seen: HashSet<String> = HashSet::new();
    for strategy in &install.strategies {
        let Ok(argv) = bat::get_argv(install, strategy, &gf) else {
            continue;
        };
        for token in &argv {
            let Some((key, value)) = token.split_once('=') else {
                continue;
            };
            let key = key.trim_matches('"');
            if !is_blob_key(key) {
                continue;
            }
            let value = value.trim_matches('"').trim();
            if value.is_empty() || value.contains('%') {
                continue;
            }
            // fake-аргумент может быть hex-строкой (0x00...), а не файлом.
            if !value.to_lowercase().ends_with(".bin") {
                continue;
            }
            let path = blob_path(install, value);
            let marker = format!("{}|{}", key.to_lowercase(), path.to_string_lossy().to_lowercase());
            if !seen.insert(marker) {
                continue;
            }
            let exists = path.is_file()
                && std::fs::metadata(&path)
                    .map(|m| m.len() > 0)
                    .unwrap_or(false);
            let blob = BlobRef {
                key: key.to_string(),
                path,
                strategy: strategy.name.clone(),
            };
            if exists {
                audit.referenced.push(blob);
            } else {
                audit.missing.push(blob);
            }
        }
    }
    audit
}

fn download_candidates(name: &str) -> Vec<String> {
    vec![
        format!(
            "https://raw.githubusercontent.com/Flowseal/zapret-discord-youtube/main/bin/{}",
            name
        ),
        format!(
            "https://raw.githubusercontent.com/bol-van/zapret/master/files/fake/{}",
            name
        ),
    ]
}

/// Свернуть `.` и `..` без обращения к файловой системе.
fn normalize_lexically(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for comp in path.components() {
        match comp {
            std::path::Component::ParentDir => {
                out.pop();
            }
            std::path::Component::CurDir => {}
            other => out.push(other.as_os_str()),
        }
    }
    out
}

/// Цель записи должна лежать внутри папки установки zapret.
///
/// Путь к blob-файлу берётся из аргументов стратегии (`--dpi-desync-fake-*`),
/// то есть из `.bat` скачанного архива. Абсолютный путь или `..` вывели бы
/// запись за пределы установки — с правами администратора.
fn inside_install(install: &ZapretInstall, path: &Path) -> bool {
    let root = super::paths::normalize_root(&install.root).to_lowercase();
    if root.is_empty() {
        return false;
    }
    let target = normalize_lexically(path).to_string_lossy().to_lowercase();
    target.len() > root.len() + 1 && target.starts_with(&format!("{}\\", root))
}

/// Докачать отсутствующие blob-файлы из официальных репозиториев.
pub fn repair(install: &ZapretInstall) -> String {
    let audit = audit(install);
    if audit.is_ok() {
        return "Все файлы ускорения (blobs) на месте.".to_string();
    }
    let mut downloaded = Vec::new();
    let mut failed = Vec::new();
    let mut done: HashSet<String> = HashSet::new();
    for blob in &audit.missing {
        if !inside_install(install, &blob.path) {
            log::warn(format!(
                "Файл ускорения вне папки zapret пропущен: {}",
                blob.path.to_string_lossy()
            ));
            failed.push(blob.path.to_string_lossy().to_string());
            continue;
        }
        let Some(name) = blob.path.file_name().map(|n| n.to_string_lossy().to_string()) else {
            failed.push(blob.path.to_string_lossy().to_string());
            continue;
        };
        if !done.insert(name.to_lowercase()) {
            continue;
        }
        let mut ok = false;
        for url in download_candidates(&name) {
            let temp = paths::temp_dir().join(format!("blob-{}", name));
            let _ = std::fs::remove_file(&temp);
            if http::download(&url, &temp, 60_000, |_, _| {}).is_err() {
                continue;
            }
            let valid = std::fs::metadata(&temp)
                .map(|m| m.len() > 0)
                .unwrap_or(false);
            if !valid {
                let _ = std::fs::remove_file(&temp);
                continue;
            }
            if let Some(parent) = blob.path.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            if std::fs::copy(&temp, &blob.path).is_ok() {
                ok = true;
                let _ = std::fs::remove_file(&temp);
                downloaded.push(name.clone());
                log::ok(format!(
                    "Докачан файл ускорения: {} -> {}",
                    name,
                    blob.path.to_string_lossy()
                ));
                break;
            }
            let _ = std::fs::remove_file(&temp);
        }
        if !ok {
            failed.push(name);
        }
    }
    let mut message = format!(
        "Проверено ссылок на блобы: {}. Не найдено: {}.",
        audit.referenced.len() + audit.missing.len(),
        failed.len()
    );
    if !downloaded.is_empty() {
        message.push_str(&format!(" Докачано: {}.", downloaded.join(", ")));
    }
    if !failed.is_empty() {
        message.push_str(&format!(
            " Не удалось найти: {}. Обновите zapret.",
            failed.join(", ")
        ));
        log::warn(format!("Не найдены файлы ускорения: {}", failed.join(", ")));
    }
    message
}

/// Короткий отчёт для диагностики.
pub fn summary(audit: &BlobAudit) -> String {
    if audit.is_ok() {
        format!("Все файлы ускорения на месте ({} ссылок).", audit.referenced.len())
    } else {
        format!(
            "Не хватает файлов ускорения: {}",
            audit.missing_names().join(", ")
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::install::StrategyDef;

    #[test]
    fn audit_finds_missing_blob() {
        let root = std::env::temp_dir().join(format!("zapret-blob-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("bin")).unwrap();
        let strategy_file = root.join("general.bat");
        std::fs::write(
            &strategy_file,
            "@echo off\r\n\"%~dp0bin\\winws.exe\" --filter-tcp=443 --dpi-desync=fake --dpi-desync-fake-tls=\"%~dp0bin\\missing.bin\"\r\n",
        )
        .unwrap();
        let install = ZapretInstall {
            root: root.to_string_lossy().to_string(),
            strategies: vec![StrategyDef {
                file: strategy_file,
                name: "general".to_string(),
                ..Default::default()
            }],
            ..Default::default()
        };
        let audit = super::audit(&install);
        assert_eq!(audit.missing.len(), 1);
        assert!(audit.missing_names()[0].contains("missing.bin"));
        std::fs::write(root.join("bin").join("missing.bin"), [1u8, 2, 3]).unwrap();
        let audit = super::audit(&install);
        assert!(audit.is_ok());
        assert_eq!(audit.referenced.len(), 1);
        let _ = std::fs::remove_dir_all(&root);
    }
}
