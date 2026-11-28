//! Обновление zapret и самого менеджера: GitHub API, архив, установка, откат.

use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use crate::util::{sha256_file, version_cmp};

use super::http;
use super::install::{Core, ZapretInstall};
use super::log;
use super::ops::{self, ZapretOps};
use super::process;
use super::textfile;
use super::winverify::{self, SignatureState};

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
    /// `digest` из GitHub API (`sha256:...`), если сервер его отдал.
    pub digest: String,
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
    /// GitHub помечает так тестовые сборки (`2.0.0-rc` и т.п.).
    pub prerelease: bool,
}

impl Release {
    /// Ядро, на котором выпущен релиз (по номеру версии в теге).
    pub fn core(&self) -> Core {
        Core::from_version(&self.tag)
    }

    /// Можно ли предлагать и ставить этот релиз этого менеджера.
    pub fn is_supported(&self) -> bool {
        !self.prerelease && self.core().is_supported()
    }

    /// Человеческое объяснение, почему релиз не предлагается.
    pub fn unsupported_reason(&self) -> Option<String> {
        if !self.core().is_supported() {
            return Some(format!(
                "{} — это zapret 2.x на новом ядре (winws2.exe + правила Lua). Менеджер работает с zapret 1.x и такую версию не устанавливает.",
                self.tag
            ));
        }
        if self.prerelease {
            return Some(format!(
                "{} — тестовая сборка (prerelease). Менеджер предлагает только стабильные релизы.",
                self.tag
            ));
        }
        None
    }

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
        prerelease: value
            .get("prerelease")
            .and_then(|v| v.as_bool())
            .unwrap_or(false),
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
                digest: asset
                    .get("digest")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string(),
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

/// Список релизов для выбора версии: только те, что менеджер умеет ставить.
///
/// Отброшенные теги возвращаются отдельно — чтобы интерфейс мог сказать
/// «есть более новые, но это zapret 2.x / тестовая сборка».
pub fn list_supported_zapret(count: usize) -> Result<(Vec<Release>, Vec<String>), String> {
    let mut kept = Vec::new();
    let mut skipped = Vec::new();
    for release in list_releases(ZAPRET_REPO, count)? {
        if release.is_supported() {
            kept.push(release);
        } else {
            skipped.push(release.tag);
        }
    }
    if kept.is_empty() {
        return Err(
            "Среди релизов zapret нет ни одного на поддерживаемом ядре 1.x.".to_string(),
        );
    }
    Ok((kept, skipped))
}

/// Последний стабильный релиз репозитория — тестовые сборки пропускаются.
pub fn latest_stable(repo: &str) -> Result<Release, String> {
    list_releases(repo, 10)?
        .into_iter()
        .find(|r| !r.prerelease)
        .ok_or_else(|| "В репозитории нет стабильных релизов (только тестовые).".to_string())
}

/// Последний релиз zapret, который менеджер умеет ставить.
///
/// GitHub отдаёт релизы в порядке публикации, включая prerelease и zapret2.
/// Берём первый поддерживаемый, а отброшенные возвращаем отдельно — чтобы
/// интерфейс мог честно объяснить, почему «2.0.0-rc» не предлагается.
pub fn latest_supported_zapret() -> Result<(Release, Vec<String>), String> {
    let mut skipped: Vec<String> = Vec::new();
    for release in list_releases(ZAPRET_REPO, 15)? {
        if release.is_supported() {
            return Ok((release, skipped));
        }
        skipped.push(release.tag.clone());
    }
    Err(
        "Среди свежих релизов zapret нет ни одного на поддерживаемом ядре 1.x \
         (все новые — zapret 2.x или тестовые сборки)."
            .to_string(),
    )
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
                    digest: String::new(),
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
    /// Более новые релизы, которые не предлагаются (zapret 2.x, prerelease).
    pub skipped: Vec<String>,
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
    // Важно: «последний» релиз zapret — не то же самое, что «последний,
    // который мы умеем ставить». Flowseal выпускает prerelease и уже перешёл
    // на zapret2 (winws2.exe + Lua) — такие релизы отбрасываем и показываем
    // пользователю как «пропущенные».
    let mut release = match latest_supported_zapret() {
        Ok((r, skipped)) => {
            res.source = "api".to_string();
            if !skipped.is_empty() {
                log::info(format!(
                    "Обновления zapret: пропущены неподдерживаемые релизы {}",
                    skipped.join(", ")
                ));
            }
            res.skipped = skipped;
            Some(r)
        }
        Err(api_error) => {
            log::warn(format!(
                "GitHub API недоступен или нет подходящих релизов ({}), версия получена через version.txt.",
                api_error
            ));
            match latest_version_fallback() {
                Ok(tag) => {
                    let candidate = Release {
                        tag: tag.clone(),
                        name: format!("zapret-discord-youtube {}", tag),
                        body: format!(
                            "Список изменений доступен на странице релиза:\nhttps://github.com/{}/releases",
                            ZAPRET_REPO
                        ),
                        html_url: format!("https://github.com/{}/releases", ZAPRET_REPO),
                        ..Default::default()
                    };
                    if candidate.is_supported() {
                        res.source = "version.txt".to_string();
                        Some(candidate)
                    } else {
                        // version.txt тоже может указывать на 2.x — не предлагаем.
                        res.error = candidate
                            .unsupported_reason()
                            .unwrap_or_else(|| api_error.clone());
                        res.skipped = vec![candidate.tag];
                        res.ok = false;
                        if let Ok(mut cache) = CHECK_CACHE.lock() {
                            *cache = Some(res.clone());
                        }
                        return res;
                    }
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

/// Тег релиза приходит из сети — в пути файловой системы его пускать нельзя.
///
/// Разрешаем только `[A-Za-z0-9._-]`, схлопываем остальное в `_`, убираем точки
/// по краям (иначе `..`), ограничиваем длину. Пустой результат → `release`.
fn safe_file_tag(tag: &str) -> String {
    let cleaned: String = tag
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_') {
                c
            } else {
                '_'
            }
        })
        .take(40)
        .collect();
    let cleaned = cleaned.trim_matches('.').trim_matches('_').to_string();
    if cleaned.is_empty() {
        "release".to_string()
    } else {
        cleaned
    }
}

/// `sha256:<hex>` из GitHub API → hex.
fn digest_sha256(digest: &str) -> Option<String> {
    let value = digest.trim();
    let hex = value.strip_prefix("sha256:").unwrap_or(value);
    if hex.len() == 64 && hex.chars().all(|c| c.is_ascii_hexdigit()) {
        Some(hex.to_lowercase())
    } else {
        None
    }
}

/// Проверить Authenticode-подпись `winws.exe` из распакованного архива.
///
/// Подпись zapret обычно отсутствует — это нормально. Недействительная подпись
/// означает испорченный или подменённый файл, и установку нужно остановить.
fn check_winws_signature(payload: &Path) -> Result<(), String> {
    let exe = payload.join("bin").join("winws.exe");
    if !exe.is_file() {
        return Ok(());
    }
    match winverify::check(&exe) {
        SignatureState::Valid => {
            log::ok("winws.exe: подпись действительна");
            Ok(())
        }
        SignatureState::Unsigned => {
            log::warn("winws.exe без Authenticode-подписи (для zapret это обычное состояние)");
            Ok(())
        }
        SignatureState::Invalid(code) => Err(format!(
            "winws.exe: подпись недействительна (0x{:08X}) — установка отменена",
            code as u32
        )),
    }
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

/// Пределы распаковки: защита от zip-bomb и переполнения диска.
const MAX_ZIP_ENTRIES: usize = 20_000;
const MAX_ZIP_ENTRY: u64 = 256 * 1024 * 1024;
const MAX_ZIP_TOTAL: u64 = 1024 * 1024 * 1024;

pub(crate) fn extract_zip(zip_path: &Path, dest: &Path) -> Result<(), String> {
    let file = std::fs::File::open(zip_path).map_err(|e| e.to_string())?;
    let mut archive = zip::ZipArchive::new(file).map_err(|e| format!("Не удалось открыть архив: {}", e))?;
    if archive.len() > MAX_ZIP_ENTRIES {
        return Err(format!(
            "В архиве слишком много файлов ({}), распаковка отменена.",
            archive.len()
        ));
    }
    std::fs::create_dir_all(dest).map_err(|e| e.to_string())?;
    let mut total: u64 = 0;
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
        // Размер из каталога архива может быть подделан, поэтому ограничиваем
        // само чтение, а не доверяем заголовку.
        if entry.size() > MAX_ZIP_ENTRY {
            return Err(format!(
                "Файл {} слишком большой ({} байт).",
                entry.name(),
                entry.size()
            ));
        }
        if let Some(parent) = out.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let mut data = Vec::new();
        {
            let mut limited = entry.by_ref().take(MAX_ZIP_ENTRY + 1);
            limited
                .read_to_end(&mut data)
                .map_err(|e| e.to_string())?;
        }
        if data.len() as u64 > MAX_ZIP_ENTRY {
            return Err(format!(
                "Файл {} больше допустимого размера.",
                out.to_string_lossy()
            ));
        }
        total += data.len() as u64;
        if total > MAX_ZIP_TOTAL {
            return Err(
                "Архив распаковывается более чем в 1 ГиБ — распаковка остановлена.".to_string(),
            );
        }
        std::fs::write(&out, data).map_err(|e| e.to_string())?;
    }
    Ok(())
}

fn find_payload(extract: &Path) -> Result<PathBuf, String> {
    if ZapretInstall::looks_valid(extract) {
        return Ok(extract.to_path_buf());
    }
    let entries = std::fs::read_dir(extract).map_err(|e| e.to_string())?;
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() && ZapretInstall::looks_valid(&path) {
            return Ok(path);
        }
    }
    // на случай вложенных папок
    for entry in std::fs::read_dir(extract).map_err(|e| e.to_string())?.flatten() {
        if !entry.path().is_dir() {
            continue;
        }
        if let Ok(inner) = std::fs::read_dir(entry.path()) {
            for inner in inner.flatten() {
                if inner.path().is_dir() && ZapretInstall::looks_valid(&inner.path()) {
                    return Ok(inner.path());
                }
            }
        }
    }
    Err(if looks_like_zapret2(extract) {
        "В архиве zapret 2.x (bin\\winws2.exe, правила Lua) — это другое ядро, менеджер его не устанавливает. \
         Обновление отменено, текущая установка не тронута."
            .to_string()
    } else {
        "В архиве не найдена папка zapret (service.bat + bin\\winws.exe).".to_string()
    })
}

/// Похож ли распакованный архив на zapret 2.x (winws2.exe и/или каталог lua).
fn looks_like_zapret2(dir: &Path) -> bool {
    fn hit(path: &Path) -> bool {
        path.join("bin").join("winws2.exe").is_file() || path.join("lua").is_dir()
    }
    if hit(dir) {
        return true;
    }
    let Ok(entries) = std::fs::read_dir(dir) else {
        return false;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() && hit(&path) {
            return true;
        }
    }
    false
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
    // Защита в глубину: даже если релиз пришёл не из `check_update`
    // (ручной тег, старая ссылка), zapret 2.x и prerelease не ставим.
    if let Some(reason) = release.unsupported_reason() {
        return Err(format!("Установка отменена: {}", reason));
    }
    if !install.is_valid() {
        return Err("Папка zapret не найдена или повреждена.".to_string());
    }
    let url = release
        .zip_url()
        .ok_or_else(|| "В релизе нет zip-архива.".to_string())?;
    http::trusted_github_url(&url)?;

    let work = super::paths::temp_dir().join(format!("update-{}", safe_file_tag(&release.tag)));
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

    // Сверяем `digest` из GitHub API, если сервер его отдал, и всегда пишем
    // SHA-256 в журнал: без этого нечем проверить архив постфактум.
    let archive_hash = sha256_file(&zip_path);
    log::info(format!(
        "Обновление zapret {}: SHA-256 архива {}",
        release.tag, archive_hash
    ));
    if let Some(expected) = release
        .assets
        .iter()
        .find(|a| a.name.to_lowercase().ends_with(".zip"))
        .and_then(|a| digest_sha256(&a.digest))
    {
        if !archive_hash.eq_ignore_ascii_case(&expected) {
            return Err(
                "Контрольная сумма архива не совпала с данными GitHub — обновление отменено."
                    .to_string(),
            );
        }
        log::ok("Архив zapret: SHA-256 совпал с digest GitHub.");
    }

    progress(38, "Распаковка архива…");
    let extract = work.join("extract");
    extract_zip(&zip_path, &extract)?;
    let payload = find_payload(&extract)?;
    check_winws_signature(&payload)?;

    let ops = ZapretOps::new(install.clone());
    let before = ops.get_state();
    let snap = super::service::query("zapret");
    let service_installed = snap.installed;
    let service_strategy = snap.strategy.clone();

    progress(44, "Остановка ускорения…");
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
            safe_file_tag(&release.tag),
            timestamp()
        ));
        let filter = |p: &Path| {
            let rel = relative(root, p);
            rel == "manager.log" || rel.starts_with("data")
        };
        copy_tree(root, &backup_path, &filter)?;
        backup = Some(backup_path);
    }

    progress(56, "Сохранение ваших правок в списках…");
    for message in super::lists::capture_blocks_before_update(root, &super::paths::data_dir()) {
        log::info(message);
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

    // Перезапуск ускорения.
    let mut rescanned = install.clone();
    rescanned.rescan();
    progress(95, "Перезапуск ускорения…");
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
    if let Some(reason) = release.unsupported_reason() {
        return Err(format!("Установка отменена: {}", reason));
    }
    let url = release
        .zip_url()
        .ok_or_else(|| "В релизе нет zip-архива.".to_string())?;
    http::trusted_github_url(&url)?;
    let work = super::paths::temp_dir().join(format!("install-{}", safe_file_tag(&release.tag)));
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
    let payload = find_payload(&extract)?;
    check_winws_signature(&payload)?;
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
    /// Размер ассета из GitHub API (0 — API его не сообщил).
    pub asset_size: u64,
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
    match latest_stable(SELF_REPO) {
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
            info.asset_size = release
                .assets
                .iter()
                .find(|a| a.name.eq_ignore_ascii_case("ZapretManager.exe"))
                .map(|a| a.size)
                .unwrap_or(0);
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
    let target = dir.join(format!("ZapretManager-{}.exe", safe_file_tag(&info.remote)));
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
                if let Err(e) = check_asset_size(&target, info.asset_size) {
                    let _ = std::fs::remove_file(&target);
                    return Err(e);
                }
                return install_self_exe(&target, &info.remote);
            }
            Err(e) => last_error = e,
        }
    }
    Err(format!("Не удалось скачать обновление: {}", last_error))
}

/// Сверить размер скачанного файла с тем, что сообщил GitHub API для ассета.
fn check_asset_size(path: &Path, expected: u64) -> Result<(), String> {
    if expected == 0 {
        return Ok(());
    }
    let actual = std::fs::metadata(path).map(|m| m.len()).unwrap_or(0);
    if actual != expected {
        return Err(format!(
            "размер обновления не совпал с релизом: ждём {} байт, получили {}",
            expected, actual
        ));
    }
    Ok(())
}

/// Заменить текущий exe и запустить новую версию.
///
/// Windows разрешает переименовать работающий exe, поэтому не нужно ждать
/// завершения старого процесса. Это надёжнее helper-режима `--apply-update`.
fn install_self_exe(downloaded: &Path, version: &str) -> Result<String, String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let backup = exe.with_file_name(format!(
        "ZapretManager-{}.old.exe",
        std::process::id()
    ));
    let _ = std::fs::remove_file(&backup);
    match std::fs::rename(&exe, &backup) {
        Ok(()) => {
            if let Err(e) = std::fs::copy(downloaded, &exe) {
                let _ = std::fs::rename(&backup, &exe);
                return Err(format!("Не удалось заменить файл программы: {}", e));
            }
            super::sys::release_single_instance();
            match std::process::Command::new(&exe).spawn() {
                Ok(_) => Ok(format!(
                    "Обновление {} установлено. Запускаю новую версию…",
                    version
                )),
                Err(e) => {
                    let _ = std::fs::remove_file(&exe);
                    let _ = std::fs::rename(&backup, &exe);
                    Err(format!("Не удалось запустить новую версию: {}", e))
                }
            }
        }
        Err(e) => {
            log::warn(format!(
                "Не удалось переименовать текущий exe ({}). Использую helper.",
                e
            ));
            spawn_apply_helper(downloaded, version)
        }
    }
}

fn spawn_apply_helper(target: &Path, version: &str) -> Result<String, String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let dir = target.parent().unwrap_or_else(|| Path::new("."));
    use std::os::windows::process::CommandExt;
    let status = std::process::Command::new(target)
        .arg("--apply-update")
        .env("ZAPRET_UPDATE_TARGET", &exe)
        .env("ZAPRET_UPDATE_PID", std::process::id().to_string())
        .current_dir(dir)
        .creation_flags(process::CREATE_NO_WINDOW)
        .spawn();
    match status {
        Ok(_) => Ok(format!(
            "Обновление {} скачано. Приложение закроется и запустится заново.",
            version
        )),
        Err(e) => Err(format!("Не удалось запустить замену файла: {}", e)),
    }
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
    // Контрольная сумма обязательна: без неё обновление не ставим. Раньше
    // ошибка скачивания checksums.txt молча отключала проверку целиком.
    let mut h: Vec<(&str, &str)> = vec![("Accept", "application/octet-stream")];
    if let Some(auth) = &auth {
        h.push(("Authorization", auth));
    }
    let resp = http::request(checksums_url, "GET", &h, None, 20000)
        .map_err(|e| format!("не удалось получить checksums.txt ({}) — обновление отменено", e))?;
    if !(200..300).contains(&resp.status) {
        return Err(format!(
            "checksums.txt недоступен (HTTP {}) — обновление отменено",
            resp.status
        ));
    }
    let expected = parse_checksum(&resp.text())
        .ok_or_else(|| "в checksums.txt нет SHA-256 — обновление отменено".to_string())?;
    let actual = sha256_file(target);
    if !actual.eq_ignore_ascii_case(&expected) {
        return Err(format!(
            "не совпала контрольная сумма (ждём {}…, получили {}…)",
            &expected[..12.min(expected.len())],
            &actual[..12.min(actual.len())]
        ));
    }
    log::info("Самообновление: контрольная сумма совпала.");
    check_signature(target)?;
    Ok(())
}

/// Проверить Authenticode-подпись скачанного обновления.
///
/// Недействительную подпись отбраковываем всегда. Неподписанный файл
/// допускаем только пока и текущая установленная версия не подписана:
/// иначе подмена подписанного канала на неподписанный проходила бы молча.
fn check_signature(target: &Path) -> Result<(), String> {
    match winverify::check(target) {
        SignatureState::Valid => {
            log::ok("Самообновление: подпись скачанного файла действительна.");
            Ok(())
        }
        SignatureState::Unsigned => {
            let current = std::env::current_exe()
                .map(|path| winverify::check(&path))
                .unwrap_or(SignatureState::Unsigned);
            if current.is_valid() {
                return Err(
                    "обновление без Authenticode-подписи, а установленная версия подписана — обновление отменено"
                        .to_string(),
                );
            }
            log::warn("Самообновление: файл без подписи, но контрольная сумма совпала.");
            Ok(())
        }
        SignatureState::Invalid(code) => Err(format!(
            "подпись скачанного файла недействительна (0x{:08X}) — обновление отменено",
            code as u32
        )),
    }
}

/// Достать SHA-256 из `checksums.txt`.
///
/// Предпочитаем строку, где упомянут `ZapretManager.exe`; иначе берём первый
/// 64-символьный hex-токен (формат `sha256sum`: `<hash>  <file>`).
fn parse_checksum(text: &str) -> Option<String> {
    let mut fallback = None;
    for line in text.lines() {
        let line = line.trim();
        let Some(token) = line.split_whitespace().next() else {
            continue;
        };
        if token.len() != 64 || !token.chars().all(|c| c.is_ascii_hexdigit()) {
            continue;
        }
        let lower = line.to_ascii_lowercase();
        if lower.contains("zapretmanager.exe") {
            return Some(token.to_lowercase());
        }
        if fallback.is_none() {
            fallback = Some(token.to_lowercase());
        }
    }
    fallback
}

/// `--apply-update` обрабатывается до UAC и до журнала, а путь берёт из
/// окружения. Разрешаем заменять только файл вида `ZapretManager*.exe`,
/// лежащий вне системных каталогов.
fn is_plausible_update_target(path: &Path) -> bool {
    if !path.is_absolute() {
        return false;
    }
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().to_lowercase())
        .unwrap_or_default();
    if !name.starts_with("zapretmanager") || !name.ends_with(".exe") {
        return false;
    }
    let Some(parent) = path.parent() else {
        return false;
    };
    if !parent.is_dir() {
        return false;
    }
    let parent_l = parent.to_string_lossy().to_lowercase();
    let windir = std::env::var("SystemRoot")
        .unwrap_or_else(|_| r"C:\Windows".to_string())
        .to_lowercase();
    !parent_l.starts_with(&windir)
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
    if !is_plausible_update_target(&target_path) {
        log::warn(format!(
            "--apply-update: отклонена цель {}",
            target_path.to_string_lossy()
        ));
        let _ = std::fs::write(&result_file, "bad target");
        return;
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    fn release_json(tag: &str, prerelease: bool) -> serde_json::Value {
        serde_json::json!({
            "tag_name": tag,
            "name": tag,
            "body": "",
            "html_url": "",
            "published_at": "2026-11-27T17:37:59Z",
            "prerelease": prerelease,
            "assets": [{
                "name": "zapret-discord-youtube-x.zip",
                "browser_download_url": "https://github.com/Flowseal/zapret-discord-youtube/releases/download/x/x.zip",
                "url": "",
                "size": 1,
                "digest": ""
            }]
        })
    }

    #[test]
    fn prerelease_and_zapret2_are_not_offered() {
        let rc = parse_release(&release_json("2.0.0-rc", true)).unwrap();
        assert_eq!(rc.core(), Core::V2);
        assert!(!rc.is_supported());
        assert!(rc.unsupported_reason().unwrap().contains("2.x"));

        // Даже финальный 2.0.0 — другое ядро, его тоже не предлагаем.
        let stable_v2 = parse_release(&release_json("2.0.0", false)).unwrap();
        assert!(!stable_v2.is_supported());
        assert!(stable_v2.unsupported_reason().is_some());

        // Тестовая сборка 1.x — тоже не для пользователя.
        let test_v1 = parse_release(&release_json("1.10.5-beta1", true)).unwrap();
        assert!(!test_v1.is_supported());
        assert!(test_v1.unsupported_reason().unwrap().contains("тестовая"));

        let stable_v1 = parse_release(&release_json("1.10.4", false)).unwrap();
        assert!(stable_v1.is_supported());
        assert!(stable_v1.unsupported_reason().is_none());
        assert!(stable_v1.zip_url().is_some());
    }

    /// Установка zapret 2.x отклоняется до скачивания — даже если релиз
    /// пришёл «ручным» путём (тег, старая ссылка).
    #[test]
    fn update_and_fresh_install_reject_zapret2() {
        let install = ZapretInstall::default();
        let release = Release {
            tag: "2.0.0-rc".to_string(),
            prerelease: true,
            ..Default::default()
        };
        let err = update_now(&install, &release, &UpdateOptions::default(), |_, _| {})
            .expect_err("2.x не должен устанавливаться");
        assert!(err.contains("2.x"), "неожиданная ошибка: {}", err);

        let dir = std::env::temp_dir().join(format!("zm-fresh-test-{}", std::process::id()));
        let err = install_fresh(&release, &dir, |_, _| {}).expect_err("2.x не должен ставиться");
        assert!(err.contains("2.x"), "неожиданная ошибка: {}", err);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Сетевой тест: реальный GitHub API сейчас отдаёт `2.0.0-rc` первым,
    /// и он обязан быть пропущен. Запускается вручную:
    /// `cargo test -- --ignored live_api`.
    #[test]
    #[ignore = "требует доступа к api.github.com"]
    fn live_api_skips_zapret2_and_prerelease() {
        let (release, skipped) = latest_supported_zapret().expect("GitHub API недоступен");
        assert_eq!(release.core(), Core::V1);
        assert!(!release.prerelease);
        assert!(
            skipped.iter().any(|tag| tag.starts_with("2.")),
            "ожидали пропущенный 2.x, получили {:?}",
            skipped
        );
        for tag in &skipped {
            assert!(
                Core::from_version(tag) == Core::V2 || tag.contains('-'),
                "пропущенный релиз {} должен быть 2.x или тестовым",
                tag
            );
        }
    }
}