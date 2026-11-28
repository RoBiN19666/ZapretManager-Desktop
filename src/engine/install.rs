//! Обнаружение установки zapret, чтение версии и разбор стратегий (.bat).

use std::path::{Path, PathBuf};

use crate::util::natural_cmp;

use super::{log, paths, sys, textfile};

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

/// Ядро zapret: 1.x (`winws.exe` + `--dpi-desync`) или 2.x (`winws2.exe` + Lua-правила).
///
/// Менеджер умеет управлять только 1.x. Второе ядро — это фактически другой
/// продукт: другой exe, другой синтаксис стратегий, другой каталог `lua\`.
/// Поэтому установку 2.x нужно распознавать отдельно, а не считать «сломанной»,
/// и никогда не предлагать её как обновление.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Core {
    #[default]
    V1,
    V2,
}

impl Core {
    pub fn label(self) -> &'static str {
        match self {
            Core::V1 => "zapret 1.x",
            Core::V2 => "zapret 2.x",
        }
    }

    /// Ядро по номеру версии: 2.x — это zapret2, всё остальное (и неизвестное) — 1.x.
    pub fn from_version(version: &str) -> Core {
        match crate::util::parse_version(version)
            .numbers
            .first()
            .copied()
            .unwrap_or(0)
        {
            0 | 1 => Core::V1,
            _ => Core::V2,
        }
    }

    /// Поддерживает ли менеджер это ядро.
    pub fn is_supported(self) -> bool {
        matches!(self, Core::V1)
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
    /// Ядро установки: 1.x (поддерживается) или 2.x (менеджер его не ведёт).
    pub core: Core,
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
    /// Движок zapret2 (2.x). Менеджер его не запускает, но обязан узнавать.
    pub fn winws2_exe(&self) -> PathBuf {
        self.bin_dir().join("winws2.exe")
    }
    pub fn lua_dir(&self) -> PathBuf {
        Path::new(&self.root).join("lua")
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

    /// Установка распознана — любой поддерживаемый ядер.
    ///
    /// Для 2.x возвращает `true`: папка настоящая, просто менеджер её не ведёт.
    /// Проверяйте [`ZapretInstall::is_supported`] там, где нужен запуск.
    pub fn is_valid(&self) -> bool {
        !self.root.is_empty() && self.service_bat().is_file() && Self::looks_valid(Path::new(&self.root))
    }

    /// Менеджер умеет управлять этой установкой (ядро 1.x).
    pub fn is_supported(&self) -> bool {
        self.is_valid() && self.core.is_supported()
    }

    pub fn is_zapret2(&self) -> bool {
        self.is_valid() && self.core == Core::V2
    }

    /// Почему установка не поддерживается (для интерфейса и журнала).
    pub fn unsupported_note(&self) -> Option<String> {
        if !self.is_zapret2() {
            return None;
        }
        Some(format!(
            "В папке {} лежит {} (bin\\winws2.exe, правила Lua). Это другое ядро — менеджер работает только с zapret 1.x и не запускает его стратегии. Обновления на 2.x не предлагаются.",
            self.root,
            if self.version.is_empty() {
                "zapret 2.x".to_string()
            } else {
                format!("zapret {}", self.version)
            }
        ))
    }

    pub fn looks_valid(dir: &Path) -> bool {
        dir.join("service.bat").is_file()
            && (dir.join("bin").join("winws.exe").is_file()
                || dir.join("bin").join("winws2.exe").is_file())
    }

    /// Ядро по содержимому папки: winws.exe → 1.x, winws2.exe → 2.x.
    fn detect_core(&self) -> Core {
        if self.winws_exe().is_file() {
            Core::V1
        } else if self.winws2_exe().is_file() {
            Core::V2
        } else {
            // Файлов движка нет (папка-заготовка): доверяем версии.
            Core::from_version(&self.version)
        }
    }

    /// Принять саму папку zapret или её родителя с единственной установкой внутри.
    pub fn resolve_valid_dir(dir: &Path) -> Option<PathBuf> {
        if Self::looks_valid(dir) {
            return Some(dir.to_path_buf());
        }
        for entry in std::fs::read_dir(dir).ok()?.flatten() {
            let path = entry.path();
            if path.is_dir() && Self::looks_valid(&path) {
                return Some(path);
            }
        }
        None
    }

    /// Создать отсутствующие пользовательские списки.
    ///
    /// `general*.bat` вызывает `service.bat load_user_lists`, но winws.exe в
    /// Rust-версии запускается напрямую, без cmd.exe. Если файлов нет,
    /// winws падает с «cannot access ipset file ...-user.txt». Поэтому
    /// создаём те же шаблоны, что и официальный service.bat.
    pub fn ensure_user_lists(&self) -> Vec<String> {
        let dir = self.lists_dir();
        if !dir.is_dir() {
            return Vec::new();
        }
        let mut created = Vec::new();
        let templates: [(&str, &str); 3] = [
            ("list-general-user.txt", "# Мои домены (файл может быть пустым)\r\n"),
            ("list-exclude-user.txt", "# Исключения (файл может быть пустым)\r\n"),
            ("ipset-exclude-user.txt", "# Исключения IP (файл может быть пустым)\r\n"),
        ];
        for (name, template) in templates {
            let path = dir.join(name);
            if path.is_file() {
                continue;
            }
            if textfile::write_text_atomic(&path, template).is_ok() {
                created.push(name.to_string());
            }
        }
        if !created.is_empty() {
            log::warn(format!(
                "Созданы недостающие файлы списков: {}",
                created.join(", ")
            ));
        }
        created
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
        let explicit = !preferred.trim().is_empty();
        if explicit {
            // Явно выбранная папка (`--root`, `Root` из manager.ini) важнее
            // автопоиска — даже если в ней zapret 2.x: пользователь должен
            // увидеть объяснение про именно эту папку, а не молча другую.
            let path = paths::normalize_root(preferred);
            if Self::looks_valid(Path::new(&path)) {
                return ZapretInstall::new(&path);
            }
            candidates.push(path);
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

        // Сначала — только 1.x: если рядом лежат обе версии, нельзя «терять»
        // рабочую установку из-за найденной папки zapret 2.x.
        for candidate in &candidates {
            let path = Path::new(candidate);
            if Self::looks_valid(path) && !Self::looks_like_v2(path) {
                return ZapretInstall::new(candidate);
            }
        }
        // 1.x нет — берём любую распознанную установку (2.x), чтобы показать
        // объяснение вместо «папка не найдена».
        for candidate in &candidates {
            if Self::looks_valid(Path::new(candidate)) {
                return ZapretInstall::new(candidate);
            }
        }
        ZapretInstall {
            root: paths::normalize_root(&exe_dir.to_string_lossy()),
            ..Default::default()
        }
    }

    /// Папка на ядре zapret 2.x (winws2.exe вместо winws.exe).
    fn looks_like_v2(dir: &Path) -> bool {
        !dir.join("bin").join("winws.exe").is_file()
            && dir.join("bin").join("winws2.exe").is_file()
    }

    pub fn rescan(&mut self) {
        self.strategies.clear();
        self.version = self.read_version();
        self.version_source = if self.version.is_empty() {
            String::new()
        } else {
            "service.bat".to_string()
        };
        self.core = self.detect_core();
        if self.core == Core::V2 {
            // В 2.0.0-rc `service.bat` всё ещё объявляет `LOCAL_VERSION=1.10.4`.
            // Для 2.x версию берём из имени папки, иначе интерфейс пишет
            // «версия 1.10.4, ядро zapret 2.x» и путает пользователя.
            if Core::from_version(&self.version) != Core::V2 {
                if let Some(folder_version) = version_from_folder(&self.root) {
                    if Core::from_version(&folder_version) == Core::V2 {
                        self.version = folder_version;
                        self.version_source = "папка".to_string();
                    }
                }
            }
            // Стратегии 2.x — не «bat-команды winws.exe», а Lua-профили для
            // winws2.exe: разбирать их старым парсером нельзя. Оставляем список
            // пустым, чтобы интерфейс не предлагал запуск.
            return;
        }
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
        self.ensure_user_lists();
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
        if let Some(token) = version_from_folder(&self.root) {
            return token;
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

/// Версия из имени папки: `...\zapret-discord-youtube-2.0.0-rc` → `2.0.0-rc`.
fn version_from_folder(root: &str) -> Option<String> {
    let lower = root.to_lowercase();
    let idx = lower.find("zapret-discord-youtube-")?;
    let rest = &root[idx + "zapret-discord-youtube-".len()..];
    let token: String = rest
        .chars()
        .take_while(|c| {
            c.is_ascii_digit() || matches!(c, '.' | '-' | '+') || c.is_ascii_alphabetic()
        })
        .collect();
    let token = token.trim_end_matches(['.', '-', '+']).to_string();
    if token.is_empty() {
        None
    } else {
        Some(token)
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

    #[test]
    fn version_from_folder_name() {
        assert_eq!(
            version_from_folder(r"C:\Zapret\zapret-discord-youtube-2.0.0-rc").as_deref(),
            Some("2.0.0-rc")
        );
        assert_eq!(
            version_from_folder(r"C:\Zapret\zapret-discord-youtube-1.10.4").as_deref(),
            Some("1.10.4")
        );
        assert_eq!(version_from_folder(r"C:\Zapret\zapret"), None);
    }

    #[test]
    fn core_from_version() {
        assert_eq!(Core::from_version("1.10.4"), Core::V1);
        assert_eq!(Core::from_version("v1.10.4"), Core::V1);
        assert_eq!(Core::from_version("2.0.0"), Core::V2);
        assert_eq!(Core::from_version("2.0.0-rc"), Core::V2);
        // Неизвестная версия не должна объявлять установку zapret2.
        assert_eq!(Core::from_version(""), Core::V1);
        assert_eq!(Core::from_version("?"), Core::V1);
        assert!(Core::V1.is_supported());
        assert!(!Core::V2.is_supported());
    }

    #[test]
    fn detects_zapret2_install() {
        let root = std::env::temp_dir().join(format!(
            "zapret-core-test-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("bin")).unwrap();
        std::fs::create_dir_all(root.join("lists")).unwrap();
        std::fs::create_dir_all(root.join("lua")).unwrap();
        std::fs::write(root.join("service.bat"), "set \"LOCAL_VERSION=2.0.0-rc\"\r\n").unwrap();
        std::fs::write(root.join("bin").join("winws2.exe"), b"MZ").unwrap();
        std::fs::write(root.join("general.bat"), "@echo off\r\n").unwrap();

        let install = ZapretInstall::new(&root.to_string_lossy());
        assert!(install.is_valid(), "папка zapret2 должна распознаваться");
        assert!(install.is_zapret2());
        assert!(!install.is_supported());
        assert_eq!(install.core, Core::V2);
        assert!(install.strategies.is_empty(), "стратегии 2.x не разбираются");
        assert!(install.unsupported_note().is_some());

        // Тот же каталог, но с winws.exe — обычный zapret 1.x.
        std::fs::write(root.join("bin").join("winws.exe"), b"MZ").unwrap();
        let v1 = ZapretInstall::new(&root.to_string_lossy());
        assert!(v1.is_supported());
        assert_eq!(v1.core, Core::V1);
        assert_eq!(v1.strategies.len(), 1);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn creates_missing_user_lists() {
        let root = std::env::temp_dir().join(format!(
            "zapret-user-lists-test-{}",
            std::process::id()
        ));
        let lists = root.join("lists");
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&lists).unwrap();
        let install = ZapretInstall {
            root: root.to_string_lossy().to_string(),
            ..Default::default()
        };
        let created = install.ensure_user_lists();
        assert_eq!(created.len(), 3);
        assert!(lists.join("list-general-user.txt").is_file());
        assert!(lists.join("list-exclude-user.txt").is_file());
        assert!(lists.join("ipset-exclude-user.txt").is_file());
        // Повторный вызов ничего не перезаписывает.
        assert!(install.ensure_user_lists().is_empty());
        let _ = std::fs::remove_dir_all(&root);
    }
}
