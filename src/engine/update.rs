//! Обновление zapret и самого менеджера: GitHub API, архив, установка, откат.

use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use crate::util::{sha256_file, version_cmp};

use super::http;
use super::install::ZapretInstall;
use super::log;
use super::ops::{self, ZapretOps};
use super::process;
use super::textfile;

pub const ZAPRET_REPO: &str = "Flowseal/zapret-discord-youtube";
pub const SELF_REPO: &str = "RoBiN19666/ZapretManager-Desktop";
pub const VERSION_URL: &str =
    "https://raw.githubusercontent.com/Flowseal/zapret-discord-youtube/main/.service/version.txt";

#[derive(Debug, Clone, Default)]
pub struct Asset {
    pub name: String,
    pub url: String,
    /// API-адрес ассета: нужен для скачивания из приватного репозитория.
    pub api_url: String,
    pub size: u64,
}

/// Токен GitHub из окружения — позволяет работать с приватным репозиторием.
pub fn github_token() -> Option<String> {
    std::env::var("ZAPRET_GITHUB_TOKEN")
        .ok()
        .map(|t| t.trim().to_string())
        .filter(|t| !t.is_empty())
}

#[derive(Debug, Clone, Default)]
pub struct Release {
    pub tag: String,
    pub name: String,
    pub body: String,
    pub html_url: String,
    pub published: String,
    pub assets: Vec<Asset>,
}

impl Release {
    pub fn download_url_for(&self, name: &str) -> Option<String> {
        self.assets
            .iter()
            .find(|a| a.name.eq_ignore_ascii_case(name))
            .map(|a| a.url.clone())
    }

    pub fn zip_url(&self) -> Option<String> {
        if let Some(asset) = self
            .assets
            .iter()
            .find(|a| a.name.to_lowercase().ends_with(".zip"))
        {
            return Some(asset.url.clone());
        }
        if self.tag.is_empty() {
            None
        } else {
            Some(format!(
                "https://github.com/{}/releases/download/{}/zapret-discord-youtube-{}.zip",
                ZAPRET_REPO, self.tag, self.tag
            ))
        }
    }

    pub fn clean_body(&self) -> String {
        let mut text = self.body.replace("\r\n", "\n");
        while let (Some(start), Some(end)) = (text.find("<!--"), text.find("-->")) {
            if end < start {
                break;
            }
            text.replace_range(start..end + 3, "");
        }
        let mut out = String::new();
        let mut in_tag = false;
        for c in text.chars() {
            match c {
                '<' => in_tag = true,
                '>' => in_tag = false,
                _ if !in_tag => out.push(c),
                _ => {}
            }
        }
        out.trim().to_string()
    }
}

fn parse_release(value: &serde_json::Value) -> Option<Release> {
    let tag = value
        .get("tag_name")
        .and_then(|v| v.as_str())
        .or_else(|| value.get("name").and_then(|v| v.as_str()))
        .unwrap_or("")
        .trim()
        .to_string();
    if tag.is_empty() {
        return None;
    }
    let mut release = Release {
        tag,
        name: value.get("name").and_then(|v| v.as_str()).unwrap_or("").to_string(),
        body: value.get("body").and_then(|v| v.as_str()).unwrap_or("").to_string(),
        html_url: value.get("html_url").and_then(|v| v.as_str()).unwrap_or("").to_string(),
        published: value
            .get("published_at")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string(),
        ..Default::default()
    };
    if let Some(assets) = value.get("assets").and_then(|v| v.as_array()) {
        for asset in assets {
            release.assets.push(Asset {
                name: asset.get("name").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                url: asset
                    .get("browser_download_url")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string(),
                api_url: asset
                    .get("url")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string(),
                size: asset.get("size").and_then(|v| v.as_u64()).unwrap_or(0),
            });
        }
    }
    Some(release)
}

fn api_get(url: &str, timeout_ms: u32) -> Result<serde_json::Value, String> {
    let auth = github_token().map(|t| format!("Bearer {}", t));
    let mut headers: Vec<(&str, &str)> = vec![
        ("Accept", "application/vnd.github+json"),
        ("X-GitHub-Api-Version", "2022-11-28"),
    ];
    if let Some(auth) = &auth {
        headers.push(("Authorization", auth));
    }
    let resp = http::request(url, "GET", &headers, None, timeout_ms)?;
    if resp.status == 403 || resp.status == 429 {
        return Err("GitHub ограничил частоту запросов (лимит 60 в час на IP)".to_string());
    }
    if !(200..300).contains(&resp.status) {
        return Err(format!("GitHub вернул HTTP {}", resp.status));
    }
    serde_json::from_slice(&resp.body).map_err(|e| format!("Не удалось разобрать ответ GitHub: {}", e))
}

pub fn get_latest_release(repo: &str) -> Result<Release, String> {
    let url = format!("https://api.github.com/repos/{}/releases?per_page=1", repo);
    let value = api_get(&url, 15000)?;
    let list = value
        .as_array()
        .ok_or_else(|| "GitHub не вернул ни одного релиза.".to_string())?;
    let first = list
        .first()
        .ok_or_else(|| "GitHub не вернул ни одного релиза.".to_string())?;
    parse_release(first).ok_or_else(|| "Не удалось разобрать ответ GitHub.".to_string())
}

pub fn list_releases(repo: &str, count: usize) -> Result<Vec<Release>, String> {
    let count = count.clamp(1, 50);
    let url = format!(
        "https://api.github.com/repos/{}/releases?per_page={}",
        repo, count
    );
    let value = api_get(&url, 25000)?;
    let list = value
        .as_array()
        .ok_or_else(|| "GitHub не вернул ни одного релиза.".to_string())?;
    let mut out = Vec::new();
    for item in list {
        if let Some(release) = parse_release(item) {
            out.push(release);
        }
    }
    if out.is_empty() {
        return Err("GitHub не вернул ни одного релиза.".to_string());
    }
    Ok(out)
}

pub fn release_by_tag(repo: &str, tag: &str) -> Result<Release, String> {
    let url = format!(
        "https://api.github.com/repos/{}/releases/tags/{}",
        repo,
        url_encode(tag)
    );
    match api_get(&url, 20000).ok().and_then(|v| parse_release(&v)) {
        Some(release) => Ok(release),
        None => {
            log::warn(format!(
                "Не удалось получить релиз {} через API, использую прямую ссылку.",
                tag
            ));
            Ok(Release {
                tag: tag.to_string(),
                name: format!("zapret-discord-youtube {}", tag),
                body: format!("Версия {} (сборка Flowseal).", tag),
                html_url: format!("https://github.com/{}/releases/tag/{}", repo, tag),
                assets: vec![Asset {
                    name: format!("zapret-discord-youtube-{}.zip", tag),
                    url: format!(
                        "https://github.com/{}/releases/download/{}/zapret-discord-youtube-{}.zip",
                        repo, tag, tag
                    ),
                    api_url: String::new(),
                    size: 0,
                }],
                ..Default::default()
            })
        }
    }
}

fn url_encode(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b'~') {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{:02X}", b));
        }
    }
    out
}

/// Резервный источник версии без GitHub API.
pub fn latest_version_fallback() -> Result<String, String> {
    let url = format!("{}?t={}", VERSION_URL, crate::util::unix_now());
    let text = http::get_string(&url, 12000)?;
    let token = text
        .split_whitespace()
        .next()
        .unwrap_or("")
        .trim()
        .to_string();
    let digits: String = token
        .chars()
        .take_while(|c| c.is_ascii_digit() || *c == '.')
        .collect();
    if digits.contains('.') {
        Ok(digits)
    } else {
        Err(format!(
            "Не удалось определить версию по version.txt: {}",
            crate::util::shorten(&text, 80)
        ))
    }
}

#[derive(Debug, Clone, Default)]
pub struct UpdateCheckResult {
    pub ok: bool,
    pub error: String,
    pub local_version: String,
    pub remote_version: String,
    pub update_available: bool,
    pub release: Option<Release>,
    pub source: String,
    pub local_unknown: bool,
    pub checked_at: Option<Instant>,
}

static CHECK_CACHE: Mutex<Option<UpdateCheckResult>> = Mutex::new(None);

/// Проверить обновление zapret (с кэшем на 5 минут).
pub fn check_update(local_version: &str, force: bool) -> UpdateCheckResult {
    if !force {
        if let Ok(cache) = CHECK_CACHE.lock() {
            if let Some(last) = cache.as_ref() {
                if last
                    .checked_at
                    .map(|t| t.elapsed() < Duration::from_secs(300))
                    .unwrap_or(false)
                {
                    return last.clone();
                }
            }
        }
    }
    let mut res = UpdateCheckResult {
        local_version: local_version.to_string(),
        checked_at: Some(Instant::now()),
        ..Default::default()
    };
    let mut release = match get_latest_release(ZAPRET_REPO) {
        Ok(r) => {
            res.source = "api".to_string();
            Some(r)
        }
        Err(api_error) => {
            log::warn(format!(
                "GitHub API недоступен ({}), версия получена через version.txt.",
                api_error
            ));
            match latest_version_fallback() {
                Ok(tag) => {
                    res.source = "version.txt".to_string();
                    Some(Release {
                        tag: tag.clone(),
                        name: format!("zapret-discord-youtube {}", tag),
                        body: format!(
                            "Список изменений доступен на странице релиза:\nhttps://github.com/{}/releases",
                            ZAPRET_REPO
                        ),
                        html_url: format!("https://github.com/{}/releases", ZAPRET_REPO),
                        ..Default::default()
                    })
                }
                Err(fallback_error) => {
                    res.ok = false;
                    res.error = if api_error == fallback_error {
                        api_error
                    } else {
                        format!("{} / {}", api_error, fallback_error)
                    };
                    if let Ok(mut cache) = CHECK_CACHE.lock() {
                        *cache = Some(res.clone());
                    }
                    return res;
                }
            }
        }
    };
    if let Some(rel) = release.take() {
        let local = local_version.trim();
        res.local_unknown = local.is_empty() || local == "?";
        res.update_available = !res.local_unknown && version_cmp(&rel.tag, local).is_gt();
        res.remote_version = rel.tag.clone();
        res.release = Some(rel);
        res.ok = true;
    }
    if let Ok(mut cache) = CHECK_CACHE.lock() {
        *cache = Some(res.clone());
    }
    res
}

/// Сбросить кэш проверки обновлений.
pub fn invalidate_cache() {
    if let Ok(mut cache) = CHECK_CACHE.lock() {
        *cache = None;
    }
}

#[derive(Debug, Clone)]
pub struct UpdateOptions {
    pub create_backup: bool,
    pub restart_after: bool,
    pub preserve_user_lists: bool,
    pub update_official_lists: bool,
}

impl Default for UpdateOptions {
    fn default() -> Self {
        UpdateOptions {
            create_backup: true,
            restart_after: true,
            preserve_user_lists: true,
            update_official_lists: true,
        }
    }
}

fn timestamp() -> String {
    let t = super::sys::local_now();
    format!(
        "{:04}{:02}{:02}-{:02}{:02}{:02}",
        t.year, t.month, t.day, t.hour, t.minute, t.second
    )
}

fn copy_tree(src: &Path, dst: &Path, filter: &dyn Fn(&Path) -> bool) -> Result<usize, String> {
    let mut copied = 0usize;
    let entries = std::fs::read_dir(src).map_err(|e| e.to_string())?;
    std::fs::create_dir_all(dst).map_err(|e| e.to_string())?;
    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name();
        let target = dst.join(&name);
        if path.is_dir() {
            if filter(&path) {
                continue;
            }
            copied += copy_tree(&path, &target, filter)?;
        } else {
            if filter(&path) {
                continue;
            }
            if let Some(parent) = target.parent() {
                std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
            }
            std::fs::copy(&path, &target).map_err(|e| e.to_string())?;
            copied += 1;
        }
    }
    Ok(copied)
}

fn extract_zip(zip_path: &Path, dest: &Path) -> Result<(), String> {
    let file = std::fs::File::open(zip_path).map_err(|e| e.to_string())?;
    let mut archive = zip::ZipArchive::new(file).map_err(|e| format!("Не удалось открыть архив: {}", e))?;
    std::fs::create_dir_all(dest).map_err(|e| e.to_string())?;
    for i in 0..archive.len() {
        let mut entry = archive.by_index(i).map_err(|e| e.to_string())?;
        let Some(name) = entry.enclosed_name() else {
            continue;
        };
        let out = dest.join(name);
        if entry.is_dir() {
            std::fs::create_dir_all(&out).map_err(|e| e.to_string())?;
            continue;
        }
        if let Some(parent) = out.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let mut data = Vec::new();
        entry.read_to_end(&mut data).map_err(|e| e.to_string())?;
        std::fs::write(&out, data).map_err(|e| e.to_string())?;
    }
    Ok(())
}

fn find_payload(extract: &Path) -> Option<PathBuf> {
    if ZapretInstall::looks_valid(extract) {
        return Some(extract.to_path_buf());
    }
    let entries = std::fs::read_dir(extract).ok()?;
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() && ZapretInstall::looks_valid(&path) {
            return Some(path);
        }
    }
    // на случай вложенных папок
    for entry in std::fs::read_dir(extract).ok()?.flatten() {
        if !entry.path().is_dir() {
            continue;
        }
        for inner in std::fs::read_dir(entry.path()).ok()?.flatten() {
            if inner.path().is_dir() && ZapretInstall::looks_valid(&inner.path()) {
                return Some(inner.path());
            }
        }
    }
    None
}

fn relative(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .map(|p| p.to_string_lossy().replace('\\', "/").to_lowercase())
        .unwrap_or_default()
}

fn should_preserve(rel: &str, opt: &UpdateOptions, exe_name: &str) -> bool {
    if rel.is_empty() {
        return false;
    }
    if rel == "manager.ini"
        || rel == "manager.log"
        || rel.starts_with("data/")
        || rel == exe_name
        || rel == "utils/game_filter.enabled"
        || rel == "utils/check_updates.enabled"
    {
        return true;
    }
    if opt.preserve_user_lists
        && (rel.ends_with("-user.txt")
            || rel == "lists/ipset-all.txt"
            || rel == "lists/ipset-all.txt.backup"
            || rel == "utils/targets.txt")
    {
        return true;
    }
    if !opt.update_official_lists
        && matches!(
            rel,
            "lists/list-general.txt"
                | "lists/list-google.txt"
                | "lists/list-exclude.txt"
                | "lists/ipset-exclude.txt"
        )
    {
        return true;
    }
    false
}

/// Установить обновление zapret.
pub fn update_now(
    install: &ZapretInstall,
    release: &Release,
    options: &UpdateOptions,
    mut progress: impl FnMut(u32, &str),
) -> Result<String, String> {
    if release.tag.is_empty() {
        return Err("Нет данных о релизе.".to_string());
    }
    if !install.is_valid() {
        return Err("Папка zapret не найдена или повреждена.".to_string());
    }
    let url = release
        .zip_url()
        .ok_or_else(|| "В релизе нет zip-архива.".to_string())?;
    http::trusted_github_url(&url)?;

    let work = super::paths::temp_dir().join(format!("update-{}", release.tag));
    let _ = std::fs::remove_dir_all(&work);
    std::fs::create_dir_all(&work).map_err(|e| e.to_string())?;
    let zip_path = work.join("update.zip");
    progress(2, &format!("Скачивание архива {}…", release.tag));
    http::download(&url, &zip_path, 120_000, |read, total| {
        let percent = if total > 0 {
            (read * 100 / total) as u32
        } else {
            10
        };
        let mapped = 2 + percent * 35 / 100;
        progress(mapped, &format!("Скачивание архива: {}%", percent));
    })?;

    progress(38, "Распаковка архива…");
    let extract = work.join("extract");
    extract_zip(&zip_path, &extract)?;
    let payload = find_payload(&extract).ok_or_else(|| {
        "В архиве не найдена папка zapret (service.bat + bin\\winws.exe).".to_string()
    })?;

    let ops = ZapretOps::new(install.clone());
    let before = ops.get_state();
    let snap = super::service::query("zapret");
    let service_installed = snap.installed;
    let service_strategy = snap.strategy.clone();

    progress(44, "Остановка bypass…");
    if service_installed {
        let _ = super::service::stop("zapret");
        let _ = ops::wait_service("zapret", "остановлена", Duration::from_secs(25));
    }
    ops.kill_winws();
    std::thread::sleep(Duration::from_millis(600));

    let root = Path::new(&install.root);
    let exe_name = super::paths::exe_path()
        .file_name()
        .map(|s| s.to_string_lossy().to_lowercase())
        .unwrap_or_default();

    let mut backup: Option<PathBuf> = None;
    if options.create_backup {
        progress(48, "Создание резервной копии…");
        let parent = root.parent().unwrap_or(Path::new("."));
        let name = root
            .file_name()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| "zapret".to_string());
        let backup_path = parent.join(format!(
            "{}_backup_{}_{}",
            name,
            release.tag,
            timestamp()
        ));
        let filter = |p: &Path| {
            let rel = relative(root, p);
            rel == "manager.log" || rel.starts_with("data")
        };
        copy_tree(root, &backup_path, &filter)?;
        backup = Some(backup_path);
    }

    progress(58, "Установка файлов…");
    let mut failures: Vec<String> = Vec::new();
    let mut file_count = 0usize;
    let payload_owned = payload.clone();
    let root_owned = root.to_path_buf();
    let preserve = |p: &Path| should_preserve(&relative(&payload_owned, p), options, &exe_name);
    let result = copy_tree(&payload, &root_owned, &preserve);
    match &result {
        Ok(count) => file_count = *count,
        Err(e) => failures.push(e.clone()),
    }
    if !failures.is_empty() {
        rollback(&root_owned, backup.as_deref(), &exe_name);
        return Err(format!(
            "Не удалось скопировать файлов: {}. Обновление остановлено, резервная копия: {}",
            failures.len(),
            backup
                .as_deref()
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_else(|| "не создавалась".to_string())
        ));
    }
    progress(92, &format!("Установлено файлов: {}", file_count));

    // Сохраняем пользовательские пометки в официальных списках.
    if options.update_official_lists {
        let messages = super::lists::apply_after_update(root, &super::paths::data_dir());
        for m in messages {
            log::info(m);
        }
    }
    if options.preserve_user_lists {
        let mode = ops.get_ipset_mode();
        if mode == "none" {
            let _ = textfile::write_text_atomic(&install.ipset_file(), "203.0.113.113/32\r\n");
        } else if mode == "any" {
            let _ = textfile::write_text_atomic(&install.ipset_file(), "");
        }
    }

    // Перезапуск bypass.
    let mut rescanned = install.clone();
    rescanned.rescan();
    progress(95, "Перезапуск bypass…");
    if options.restart_after {
        let ops2 = ZapretOps::new(rescanned.clone());
        if service_installed {
            let target = rescanned
                .find_strategy(&service_strategy)
                .cloned()
                .or_else(|| rescanned.default_strategy().cloned());
            if let Some(strategy) = target {
                let _ = ops2.install_service(&strategy);
            }
        } else if before.winws_running {
            let target = rescanned
                .find_strategy(&before.matched_strategy)
                .cloned()
                .or_else(|| rescanned.default_strategy().cloned());
            if let Some(strategy) = target {
                let _ = ops2.start(&strategy);
            }
        }
    }

    let _ = std::fs::remove_dir_all(&work);
    let (freed, removed) = ops::prune_backups(&install.root, 2);

    let mut summary = format!("Обновлено до версии {}.", release.tag);
    if let Some(path) = &backup {
        summary.push_str(&format!("\nРезервная копия: {}", path.to_string_lossy()));
    }
    if removed > 0 {
        summary.push_str(&format!(
            "\nСтарые копии убраны, освобождено {}",
            crate::util::format_size(freed)
        ));
    }
    log::ok(format!("Обновление zapret: {}", summary.replace('\n', " ")));
    progress(100, "Готово");
    Ok(summary)
}

fn rollback(root: &Path, backup: Option<&Path>, exe_name: &str) {
    let Some(backup) = backup else {
        return;
    };
    let filter = |p: &Path| {
        let rel = relative(backup, p);
        rel == "manager.log" || rel.starts_with("data") || rel == exe_name
    };
    match copy_tree(backup, root, &filter) {
        Ok(_) => log::warn("Выполнен откат из резервной копии"),
        Err(e) => log::error(format!("Откат не удался: {}", e)),
    }
}

/// Установка zapret с нуля в выбранную папку.
pub fn install_fresh(
    release: &Release,
    parent_dir: &Path,
    mut progress: impl FnMut(u32, &str),
) -> Result<ZapretInstall, String> {
    let url = release
        .zip_url()
        .ok_or_else(|| "В релизе нет zip-архива.".to_string())?;
    http::trusted_github_url(&url)?;
    let work = super::paths::temp_dir().join(format!("install-{}", release.tag));
    let _ = std::fs::remove_dir_all(&work);
    std::fs::create_dir_all(&work).map_err(|e| e.to_string())?;
    let zip_path = work.join("update.zip");
    progress(5, &format!("Скачивание архива {}…", release.tag));
    http::download(&url, &zip_path, 300_000, |read, total| {
        let percent = if total > 0 { (read * 100 / total) as u32 } else { 20 };
        progress(20 + percent * 50 / 100, &format!("Скачивание архива: {}%", percent));
    })?;
    progress(75, "Распаковка…");
    let extract = work.join("extract");
    extract_zip(&zip_path, &extract)?;
    let payload = find_payload(&extract).ok_or_else(|| "В архиве нет папки zapret.".to_string())?;
    let folder = payload
        .file_name()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| format!("zapret-discord-youtube-{}", release.tag));
    let target = parent_dir.join(&folder);
    if target.exists() {
        return Err(format!("Папка уже существует: {}", target.to_string_lossy()));
    }
    progress(85, &format!("Копирование в {}…", target.to_string_lossy()));
    std::fs::create_dir_all(&target).map_err(|e| e.to_string())?;
    copy_tree(&payload, &target, &|_| false)?;
    let _ = std::fs::remove_dir_all(&work);
    progress(100, "Готово");
    let mut install = ZapretInstall::new(&target.to_string_lossy());
    install.rescan();
    Ok(install)
}

// ----------------------------------------------------------------------
// Самообновление менеджера
// ----------------------------------------------------------------------

#[derive(Debug, Clone, Default)]
pub struct SelfUpdateInfo {
    pub local: String,
    pub remote: String,
    pub available: bool,
    pub ok: bool,
    pub error: String,
    pub asset_url: String,
    pub checksums_url: String,
}

static SELF_CACHE: Mutex<Option<(Instant, SelfUpdateInfo)>> = Mutex::new(None);

pub fn self_check(force: bool) -> SelfUpdateInfo {
    if !force {
        if let Ok(cache) = SELF_CACHE.lock() {
            if let Some((at, info)) = cache.as_ref() {
                if at.elapsed() < Duration::from_secs(300) {
                    return info.clone();
                }
            }
        }
    }
    let mut info = SelfUpdateInfo {
        local: crate::APP_VERSION.to_string(),
        ..Default::default()
    };
    match get_latest_release(SELF_REPO) {
        Ok(release) => {
            info.remote = release.tag.clone();
            info.available = version_cmp(&release.tag, crate::APP_VERSION).is_gt();
            let token = github_token();
            let pick = |name: &str| -> String {
                let asset = release.assets.iter().find(|a| a.name.eq_ignore_ascii_case(name));
                if token.is_some() {
                    if let Some(asset) = asset {
                        if !asset.api_url.is_empty() {
                            return asset.api_url.clone();
                        }
                    }
                }
                asset
                    .map(|a| a.url.clone())
                    .filter(|u| !u.is_empty())
                    .unwrap_or_else(|| {
                        format!(
                            "https://github.com/{}/releases/download/{}/{}",
                            SELF_REPO, release.tag, name
                        )
                    })
            };
            info.asset_url = pick("ZapretManager.exe");
            info.checksums_url = pick("checksums.txt");
            info.ok = true;
        }
        Err(e) => {
            info.ok = false;
            info.error = if e.contains("404") {
                "репозиторий релизов недоступен: нет доступа или ещё нет ни одного релиза (для приватного репозитория задайте ZAPRET_GITHUB_TOKEN)".to_string()
            } else {
                e
            };
        }
    }
    if let Ok(mut cache) = SELF_CACHE.lock() {
        *cache = Some((Instant::now(), info.clone()));
    }
    info
}

fn update_temp_dir() -> PathBuf {
    std::env::temp_dir().join("ZapretManagerUpdate")
}

/// Скачать новую версию и запустить замену файла.
pub fn self_apply(info: &SelfUpdateInfo) -> Result<String, String> {
    if !info.ok {
        return Err("Сначала проверьте обновления.".to_string());
    }
    if !info.available {
        return Err(format!("Установлена последняя версия {}.", info.local));
    }
    let dir = update_temp_dir();
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let target = dir.join(format!("ZapretManager-{}.exe", info.remote));
    let mut candidate = info.asset_url.clone();
    let mut last_error = String::new();
    if candidate.is_empty() {
        candidate = format!(
            "https://github.com/{}/releases/download/{}/ZapretManager.exe",
            SELF_REPO, info.remote
        );
    }
    for url in [
        candidate.clone(),
        format!(
            "https://github.com/{}/releases/download/{}/ZapretManager.exe",
            SELF_REPO, info.remote
        ),
    ] {
        match download_verify(&url, &target, &info.checksums_url) {
            Ok(()) => {
                let exe = std::env::current_exe().map_err(|e| e.to_string())?;
                use std::os::windows::process::CommandExt;
                let status = std::process::Command::new(&target)
                    .arg("--apply-update")
                    .env("ZAPRET_UPDATE_TARGET", &exe)
                    .env("ZAPRET_UPDATE_PID", std::process::id().to_string())
                    .current_dir(&dir)
                    .creation_flags(process::CREATE_NO_WINDOW)
                    .spawn();
                return match status {
                    Ok(_) => Ok(format!(
                        "Обновление {} скачано из релиза. Приложение закроется и запустится заново уже новой версией.",
                        info.remote
                    )),
                    Err(e) => Err(format!("Не удалось запустить замену файла: {}", e)),
                };
            }
            Err(e) => last_error = e,
        }
    }
    Err(format!("Не удалось скачать обновление: {}", last_error))
}

fn download_verify(url: &str, target: &Path, checksums_url: &str) -> Result<(), String> {
    let token = github_token();
    let auth = token.as_ref().map(|t| format!("Bearer {}", t));
    let mut headers: Vec<(&str, &str)> = Vec::new();
    if let Some(auth) = &auth {
        headers.push(("Authorization", auth));
        headers.push(("Accept", "application/octet-stream"));
    }
    http::download_with_headers(url, target, 240_000, &headers, |_, _| {})?;
    let size = std::fs::metadata(target).map(|m| m.len()).unwrap_or(0);
    if size < 200 * 1024 {
        return Err(format!("файл слишком мал ({} байт)", size));
    }
    let head = std::fs::read(target)
        .ok()
        .filter(|b| b.len() >= 2)
        .map(|b| b[0] == b'M' && b[1] == b'Z');
    if head != Some(true) {
        return Err("файл не является программой".to_string());
    }
    let checksums = {
        let mut h: Vec<(&str, &str)> = vec![("Accept", "application/octet-stream")];
        if let Some(auth) = &auth {
            h.push(("Authorization", auth));
        }
        http::request(checksums_url, "GET", &h, None, 20000)
            .ok()
            .map(|r| r.text())
    };
    if let Some(text) = checksums {
        if let Some(expected) = parse_checksum(&text) {
            let actual = sha256_file(target);
            if !actual.eq_ignore_ascii_case(&expected) {
                return Err(format!(
                    "не совпала контрольная сумма (ждём {}…, получили {}…)",
                    &expected[..12.min(expected.len())],
                    &actual[..12.min(actual.len())]
                ));
            }
        }
    } else {
        log::warn("в релизе нет checksums.txt, проверяю только размер и версию файла.");
    }
    Ok(())
}

fn parse_checksum(text: &str) -> Option<String> {
    for line in text.lines() {
        let line = line.trim();
        if let Some(token) = line.split_whitespace().next() {
            if token.len() == 64 && token.chars().all(|c| c.is_ascii_hexdigit()) {
                return Some(token.to_lowercase());
            }
        }
    }
    None
}

/// Режим --apply-update: дождаться выхода старого процесса и заменить exe.
pub fn apply_update_mode() {
    let dir = update_temp_dir();
    let result_file = dir.join("apply-result.txt");
    let target = std::env::var("ZAPRET_UPDATE_TARGET").unwrap_or_default();
    let old_pid: u32 = std::env::var("ZAPRET_UPDATE_PID")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(0);
    if target.is_empty() {
        let _ = std::fs::write(&result_file, "no target");
        return;
    }
    let target_path = PathBuf::from(&target);
    if old_pid > 0 {
        let deadline = Instant::now() + Duration::from_secs(120);
        while Instant::now() < deadline {
            if !super::sys::list_processes().iter().any(|p| p.pid == old_pid) {
                break;
            }
            std::thread::sleep(Duration::from_millis(250));
        }
    }
    let me = match std::env::current_exe() {
        Ok(p) => p,
        Err(e) => {
            let _ = std::fs::write(&result_file, format!("error: {}", e));
            return;
        }
    };
    let mut copied = false;
    for _ in 0..30 {
        match std::fs::copy(&me, &target_path) {
            Ok(_) => {
                copied = true;
                break;
            }
            Err(_) => std::thread::sleep(Duration::from_millis(500)),
        }
    }
    if copied {
        let now = super::sys::local_now();
        let _ = std::fs::write(&result_file, format!("ok {}", now));
    } else {
        let _ = std::fs::write(&result_file, "copy failed");
    }
    let _ = process::spawn_hidden(
        &target_path.to_string_lossy(),
        &[],
        target_path.parent(),
    );
}
