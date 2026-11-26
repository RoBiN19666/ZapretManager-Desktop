//! Обнаружение установки zapret, чтение версии и разбор стратегий (.bat).

use std::path::{Path, PathBuf};

use crate::util::natural_cmp;

use super::{paths, sys};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StrategyKind {
    Base,
    Alt,
    Exp,
    Simple,
    FakeTls,
    Other,
}

impl StrategyKind {
    pub fn label(&self) -> &'static str {
        match self {
            StrategyKind::Base => "BASE",
            StrategyKind::Alt => "ALT",
            StrategyKind::Exp => "EXP",
            StrategyKind::Simple => "SIMPLE",
            StrategyKind::FakeTls => "FAKE TLS",
            StrategyKind::Other => "СВОЯ",
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct StrategyDef {
    pub file: PathBuf,
    pub name: String,
    pub short_name: String,
    pub kind: StrategyKind,
    pub description: String,
    pub warning: bool,
    pub order: usize,
}

impl Default for StrategyKind {
    fn default() -> Self {
        StrategyKind::Other
    }
}

#[derive(Debug, Clone, Default)]
pub struct ZapretInstall {
    pub root: String,
    pub version: String,
    pub version_source: String,
    pub strategies: Vec<StrategyDef>,
}

impl ZapretInstall {
    pub fn new(root: &str) -> ZapretInstall {
        let mut inst = ZapretInstall {
            root: paths::normalize_root(root),
            ..Default::default()
        };
        inst.rescan();
        inst
    }

    pub fn bin_dir(&self) -> PathBuf {
        Path::new(&self.root).join("bin")
    }
    pub fn lists_dir(&self) -> PathBuf {
        Path::new(&self.root).join("lists")
    }
    pub fn utils_dir(&self) -> PathBuf {
        Path::new(&self.root).join("utils")
    }
    pub fn service_bat(&self) -> PathBuf {
        Path::new(&self.root).join("service.bat")
    }
    pub fn winws_exe(&self) -> PathBuf {
        self.bin_dir().join("winws.exe")
    }
    pub fn game_filter_file(&self) -> PathBuf {
        self.utils_dir().join("game_filter.enabled")
    }
    pub fn check_updates_file(&self) -> PathBuf {
        self.utils_dir().join("check_updates.enabled")
    }
    pub fn ipset_file(&self) -> PathBuf {
        self.lists_dir().join("ipset-all.txt")
    }
    pub fn ipset_backup(&self) -> PathBuf {
        self.lists_dir().join("ipset-all.txt.backup")
    }

    pub fn is_valid(&self) -> bool {
        !self.root.is_empty() && self.service_bat().is_file() && self.winws_exe().is_file()
    }

    pub fn looks_valid(dir: &Path) -> bool {
        dir.join("service.bat").is_file() && dir.join("bin").join("winws.exe").is_file()
    }

    /// Папка, из которой запущена служба zapret (по ImagePath в реестре).
    pub fn service_root_from_registry() -> Option<String> {
        let image = sys::reg_get_string(
            true,
            r"System\CurrentControlSet\Services\zapret",
            "ImagePath",
        )?;
        let image = image.trim().to_string();
        let exe = if let Some(rest) = image.strip_prefix('"') {
            match rest.find('"') {
                Some(end) => rest[..end].to_string(),
                None => rest.to_string(),
            }
        } else {
            match image.to_lowercase().find(".exe") {
                Some(pos) => image[..pos + 4].to_string(),
                None => image.clone(),
            }
        };
        let bin = Path::new(&exe).parent()?;
        let root = bin.parent()?;
        Some(paths::normalize_root(&root.to_string_lossy()))
    }

    /// Ищет установку: указанная папка, папка службы, папка exe, подпапки, родитель, C:\zapret.
    pub fn detect(preferred: &str) -> ZapretInstall {
        let mut candidates: Vec<String> = Vec::new();
        if !preferred.trim().is_empty() {
            candidates.push(paths::normalize_root(preferred));
        }
        if let Some(service) = Self::service_root_from_registry() {
            candidates.push(service);
        }
        let exe_dir = paths::exe_dir();
        candidates.push(exe_dir.to_string_lossy().to_string());
        if let Ok(entries) = std::fs::read_dir(&exe_dir) {
            for entry in entries.flatten() {
                let p = entry.path();
                if p.is_dir() {
                    candidates.push(p.to_string_lossy().to_string());
                    if let Ok(inner) = std::fs::read_dir(&p) {
                        for e2 in inner.flatten() {
                            if e2.path().is_dir() {
                                candidates.push(e2.path().to_string_lossy().to_string());
                            }
                        }
                    }
                }
            }
        }
        if let Some(parent) = exe_dir.parent() {
            candidates.push(parent.to_string_lossy().to_string());
        }
        candidates.push(r"C:\zapret".to_string());
        candidates.push(r"C:\Zapret".to_string());

        for candidate in candidates {
            if Self::looks_valid(Path::new(&candidate)) {
                return ZapretInstall::new(&candidate);
            }
        }
        ZapretInstall {
            root: paths::normalize_root(&exe_dir.to_string_lossy()),
            ..Default::default()
        }
    }

    pub fn rescan(&mut self) {
        self.strategies.clear();
        self.version = self.read_version();
        let entries = match std::fs::read_dir(&self.root) {
            Ok(e) => e,
            Err(_) => return,
        };
        let mut names: Vec<String> = Vec::new();
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_file() {
                continue;
            }
            let ext = path
                .extension()
                .map(|e| e.to_string_lossy().to_lowercase())
                .unwrap_or_default();
            if ext != "bat" {
                continue;
            }
            let name = path
                .file_stem()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_default();
            let lower = name.to_lowercase();
            if lower.starts_with("service") || must_ignore_bat(&lower) {
                continue;
            }
            names.push(name);
        }
        names.sort_by(|a, b| natural_cmp(a, b));
        for (order, name) in names.into_iter().enumerate() {
            let mut def = self.describe(&name);
            def.file = Path::new(&self.root).join(format!("{}.bat", name));
            def.order = order;
            self.strategies.push(def);
        }
    }

    fn describe(&self, name: &str) -> StrategyDef {
        let lower = name.to_lowercase();
        let (kind, description) = if lower == "general" {
            (
                StrategyKind::Base,
                "Базовая стратегия. Работает в большинстве случаев — начинайте с неё.",
            )
        } else if lower.contains("simple fake") {
            (
                StrategyKind::Simple,
                "Упрощённая подмена фейк-пакетов. Хорошо работает на части провайдеров.",
            )
        } else if lower.contains("fake tls auto") {
            (
                StrategyKind::FakeTls,
                "Автоматический подбор фейк-TLS. Универсальный вариант, если базовая не помогла.",
            )
        } else if lower.contains("exp") {
            (
                StrategyKind::Exp,
                "Экспериментальная стратегия. Возможны нестабильность и побочные эффекты.",
            )
        } else if lower.contains("alt") {
            (
                StrategyKind::Alt,
                "Альтернативная стратегия. Пробуйте по порядку, если базовая не помогает.",
            )
        } else {
            (
                StrategyKind::Other,
                "Пользовательская или неизвестная стратегия.",
            )
        };

        let short_name = if lower.starts_with("general") {
            let trimmed = name["general".len()..].trim();
            let trimmed = if trimmed.starts_with('(') && trimmed.ends_with(')') && trimmed.len() > 2 {
                trimmed[1..trimmed.len() - 1].trim()
            } else {
                trimmed
            };
            if trimmed.is_empty() {
                "BASE".to_string()
            } else {
                trimmed.to_string()
            }
        } else {
            name.to_string()
        };

        StrategyDef {
            name: name.to_string(),
            short_name,
            kind,
            description: description.to_string(),
            warning: FileComment::has_comment(
                &Path::new(&self.root).join(format!("{}.bat", name)),
                "NOT RECOMMENDED",
            ),
            ..Default::default()
        }
    }

    fn read_version(&self) -> String {
        // service.bat может быть в CP1251 — читаем через автоопределение кодировки.
        let text = super::textfile::read(&self.service_bat());
        if let Some(v) = extract_local_version(&text) {
            return v;
        }
        for file in [
            Path::new(&self.root).join("version.txt"),
            Path::new(&self.root).join(".service").join("version.txt"),
        ] {
            let text = super::textfile::read(&file);
            if let Some(v) = extract_version_token(&text) {
                return v;
            }
        }
        if let Some(idx) = self.root.to_lowercase().find("zapret-discord-youtube-") {
            let rest = &self.root[idx + "zapret-discord-youtube-".len()..];
            let token: String = rest
                .chars()
                .take_while(|c| c.is_ascii_digit() || *c == '.' || c.is_ascii_alphabetic())
                .collect();
            let token = token.trim_end_matches('.').to_string();
            if !token.is_empty() {
                return token;
            }
        }
        String::new()
    }

    pub fn find_strategy(&self, name: &str) -> Option<&StrategyDef> {
        if name.is_empty() {
            return None;
        }
        self.strategies.iter().find(|s| {
            s.name.eq_ignore_ascii_case(name) || format!("{}.bat", s.name).eq_ignore_ascii_case(name)
        })
    }

    /// Стратегия по умолчанию: general, иначе первая.
    pub fn default_strategy(&self) -> Option<&StrategyDef> {
        self.find_strategy("general")
            .or_else(|| self.strategies.first())
    }
}

fn must_ignore_bat(lower: &str) -> bool {
    lower.contains("update")
        || lower.contains("install")
        || lower.contains("uninstall")
        || lower == "start"
        || lower == "stop"
}

/// Первые 14 строк файла содержат комментарий с токеном.
struct FileComment;

impl FileComment {
    fn has_comment(file: &Path, token: &str) -> bool {
        let text = super::textfile::read(file);
        let token_lower = token.to_lowercase();
        text.lines()
            .take(14)
            .any(|line| line.to_lowercase().contains(&token_lower))
    }
}

/// LOCAL_VERSION="1.10.4" в service.bat.
fn extract_local_version(text: &str) -> Option<String> {
    let idx = text.find("LOCAL_VERSION")?;
    let rest = &text[idx + "LOCAL_VERSION".len()..];
    let rest = rest.trim_start();
    let rest = rest.strip_prefix('=')?.trim_start();
    let rest = rest.strip_prefix('"').unwrap_or(rest);
    let token: String = rest
        .chars()
        .take_while(|c| c.is_ascii_digit() || *c == '.' || c.is_ascii_alphabetic())
        .collect();
    let token = token.trim_end_matches('.').to_string();
    if token.starts_with(|c: char| c.is_ascii_digit()) && !token.is_empty() {
        Some(token)
    } else {
        None
    }
}

/// Первое вхождение вида 1.2.3[a-z].
fn extract_version_token(text: &str) -> Option<String> {
    let bytes = text.as_bytes();
    let mut start = None;
    for (i, b) in bytes.iter().enumerate() {
        if b.is_ascii_digit() {
            start = Some(i);
            break;
        }
    }
    let start = start?;
    let mut end = start;
    while end < bytes.len() {
        let b = bytes[end];
        if b.is_ascii_digit() || b == b'.' {
            end += 1;
        } else {
            break;
        }
    }
    if end < bytes.len() && bytes[end].is_ascii_alphabetic() {
        end += 1;
    }
    let token = &text[start..end];
    if token.matches('.').count() >= 1 {
        Some(token.to_string())
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_from_bat() {
        let text = "set \"LOCAL_VERSION=1.10.4\"\r\n";
        assert_eq!(extract_local_version(text).as_deref(), Some("1.10.4"));
    }

    #[test]
    fn version_token() {
        assert_eq!(extract_version_token("v1.9.9d\n").as_deref(), Some("1.9.9d"));
    }
}
