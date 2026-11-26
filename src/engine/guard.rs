//! Фоновый сторож: читает журнал DNS Windows и находит заблокированные домены.

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use super::install::ZapretInstall;
use super::lists;
use super::log;
use super::netcheck;
use super::process;
use super::textfile;

pub const CHANNEL: &str = "Microsoft-Windows-DNS-Client/Operational";
pub const BROWSERS: [&str; 10] = [
    "chrome", "msedge", "firefox", "opera", "brave", "vivaldi", "yandex", "browser", "thorium",
    "librewolf",
];

pub const NOISE: [&str; 36] = [
    "microsoft.com",
    "windowsupdate.com",
    "windows.com",
    "msftncsi.com",
    "msedge.net",
    "bing.com",
    "live.com",
    "office.com",
    "office365.com",
    "outlook.com",
    "xboxlive.com",
    "microsoftonline.com",
    "gstatic.com",
    "google.com",
    "googleapis.com",
    "googlevideo.com",
    "ytimg.com",
    "googleusercontent.com",
    "cloudflare.com",
    "cloudflare-dns.com",
    "akamai.net",
    "akamaized.net",
    "cloudfront.net",
    "apple.com",
    "icloud.com",
    "mozilla.org",
    "mozilla.net",
    "sentry.io",
    "githubusercontent.com",
    "steamstatic.com",
    "steampowered.com",
    "epicgames.com",
    "battle.net",
    "riotgames.com",
    "digicert.com",
    "lencr.org",
];

fn is_noise(host: &str) -> bool {
    NOISE
        .iter()
        .any(|n| host == *n || host.ends_with(&format!(".{}", n)))
}

static CURSOR: Mutex<Option<i64>> = Mutex::new(None);
static CHECKED: Mutex<Option<HashMap<String, Instant>>> = Mutex::new(None);

pub fn reset_cursor() {
    if let Ok(mut c) = CURSOR.lock() {
        *c = None;
    }
    if let Ok(mut m) = CHECKED.lock() {
        *m = None;
    }
}

fn cursor_get() -> Option<i64> {
    CURSOR.lock().ok().and_then(|c| *c)
}

fn cursor_set(value: i64) {
    if let Ok(mut c) = CURSOR.lock() {
        *c = Some(value);
    }
}

/// Включён ли журнал DNS-клиента.
static LOG_CACHE: Mutex<Option<(Instant, bool)>> = Mutex::new(None);

/// Включён ли журнал DNS (результат кэшируется на 5 секунд: UI спрашивает часто).
pub fn log_enabled() -> bool {
    if let Ok(cache) = LOG_CACHE.lock() {
        if let Some((at, value)) = *cache {
            if at.elapsed() < Duration::from_secs(5) {
                return value;
            }
        }
    }
    let r = process::run("wevtutil.exe", &["gl", CHANNEL], Duration::from_secs(10));
    let value = r.all().to_lowercase().contains("enabled: true");
    if let Ok(mut cache) = LOG_CACHE.lock() {
        *cache = Some((Instant::now(), value));
    }
    value
}

/// Включить журнал DNS-клиента (нужны права администратора).
pub fn enable_log() -> String {
    let r = process::run(
        "wevtutil.exe",
        &["sl", CHANNEL, "/e:true"],
        Duration::from_secs(15),
    );
    if r.exit_code == 0 && log_enabled() {
        "Журнал DNS-клиента включён.".to_string()
    } else {
        "Не удалось включить журнал (нужны права администратора).".to_string()
    }
}

#[derive(Debug, Clone, Default)]
pub struct DnsEvent {
    pub record_id: i64,
    pub host: String,
    pub pid: u32,
}

/// Прочитать новые события DNS-журнала (не более 200).
pub fn new_events(allowed_pids: Option<&[u32]>) -> (Vec<DnsEvent>, String, usize) {
    let query = "*[System[(EventID=3020 or EventID=3022 or EventID=3021)]]";
    let r = process::run(
        "wevtutil.exe",
        &[
            "qe",
            CHANNEL,
            &format!("/q:{}", query),
            "/f:xml",
            "/c:200",
            "/rd:true",
        ],
        Duration::from_secs(20),
    );
    if r.exit_code != 0 && r.all().trim().is_empty() {
        return (Vec::new(), format!("wevtutil: {}", r.all()), 0);
    }
    let xml = r.all();
    let events = parse_events(&xml);
    let cursor = cursor_get();
    let mut scanned = 0usize;
    let mut out: Vec<DnsEvent> = Vec::new();
    let mut max_id = cursor.unwrap_or(-1);
    for ev in events {
        if let Some(c) = cursor {
            if ev.record_id <= c {
                continue;
            }
        }
        scanned += 1;
        if ev.record_id > max_id {
            max_id = ev.record_id;
        }
        if ev.host.is_empty() {
            continue;
        }
        if let Some(allowed) = allowed_pids {
            if ev.pid == 0 || !allowed.contains(&ev.pid) {
                continue;
            }
        } else if ev.pid > 0 {
            let name = super::sys::list_processes()
                .into_iter()
                .find(|p| p.pid == ev.pid)
                .map(|p| p.name.to_lowercase())
                .unwrap_or_default();
            let base = name.trim_end_matches(".exe").to_string();
            if !BROWSERS.contains(&base.as_str()) {
                continue;
            }
        }
        if !out.iter().any(|e| e.host.eq_ignore_ascii_case(&ev.host)) {
            out.push(ev);
        }
    }
    if max_id >= 0 {
        cursor_set(max_id);
    }
    (out, String::new(), scanned)
}

/// Разбор XML-вывода wevtutil без внешних зависимостей.
pub fn parse_events(xml: &str) -> Vec<DnsEvent> {
    let mut out = Vec::new();
    for chunk in xml.split("<Event ").skip(1) {
        let record_id = extract_tag(chunk, "EventRecordID")
            .and_then(|v| v.trim().parse::<i64>().ok())
            .unwrap_or(0);
        let mut host = String::new();
        let mut pid = 0u32;
        let mut rest = chunk;
        while let Some(pos) = rest.find("<Data ") {
            let data = &rest[pos..];
            let name = extract_attr(data, "Name").unwrap_or_default();
            let value = extract_tag(data, "Data").unwrap_or_default();
            if name.eq_ignore_ascii_case("QueryName") && host.is_empty() {
                host = value.trim().trim_end_matches('.').to_lowercase();
            }
            if name.eq_ignore_ascii_case("ClientPID") {
                pid = value.trim().parse().unwrap_or(0);
            }
            rest = &data[1..];
        }
        if pid == 0 {
            if let Some(exec) = chunk.find("<Execution ") {
                pid = extract_attr(&chunk[exec..], "ProcessID")
                    .and_then(|v| v.parse().ok())
                    .unwrap_or(0);
            }
        }
        if !host.is_empty() && (host.len() < 4 || !host.contains('.')) {
            continue;
        }
        if host.ends_with(".local") || host.ends_with(".lan") || host.ends_with(".arpa") {
            continue;
        }
        if host.starts_with("wpad") || host.starts_with("localhost") {
            continue;
        }
        if is_noise(&host) {
            continue;
        }
        out.push(DnsEvent {
            record_id,
            host,
            pid,
        });
    }
    out
}

fn extract_tag(text: &str, tag: &str) -> Option<String> {
    let open = format!("<{}", tag);
    let start = text.find(&open)?;
    let after = &text[start..];
    let gt = after.find('>')?;
    let end_tag = format!("</{}>", tag);
    let end = after.find(&end_tag)?;
    if end <= gt {
        return None;
    }
    Some(after[gt + 1..end].to_string())
}

fn extract_attr(text: &str, attr: &str) -> Option<String> {
    let key = format!("{}=\"", attr);
    let key2 = format!("{}='", attr);
    let (start, quote) = if let Some(i) = text.find(&key) {
        (i + key.len(), '"')
    } else if let Some(i) = text.find(&key2) {
        (i + key2.len(), '\'')
    } else {
        return None;
    };
    let rest = &text[start..];
    let end = rest.find(quote)?;
    Some(rest[..end].to_string())
}

/// Уже покрыт ли домен существующими списками.
pub fn already_listed(install: &ZapretInstall, host: &str) -> bool {
    let Ok(entries) = std::fs::read_dir(install.lists_dir()) else {
        return false;
    };
    let mut files: Vec<std::path::PathBuf> = Vec::new();
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().to_lowercase();
        if name.contains("list") && name.ends_with(".txt") && !name.contains("exclude") {
            files.push(entry.path());
        }
    }
    let mut labels: Vec<&str> = host.split('.').collect();
    while labels.len() >= 2 {
        let suffix = labels.join(".");
        for file in &files {
            for line in textfile::read(file).lines() {
                let normalized = lists::normalize(line);
                let normalized = normalized.trim_start_matches('^');
                if normalized == suffix {
                    return true;
                }
            }
        }
        labels.remove(0);
    }
    false
}

/// Проверить найденные домены и (по желанию) добавить в список.
pub fn process_hosts(
    install: &ZapretInstall,
    hosts: &[String],
    auto_add: bool,
    mut on_found: impl FnMut(&str),
) -> Vec<String> {
    let mut added = Vec::new();
    if !install.is_valid() {
        return added;
    }
    let now = Instant::now();
    let mut per_cycle = 0usize;
    for host in hosts {
        if per_cycle >= 5 {
            break;
        }
        if host.is_empty() {
            continue;
        }
        {
            let mut guard = CHECKED.lock().unwrap_or_else(|e| e.into_inner());
            let map = guard.get_or_insert_with(HashMap::new);
            if let Some(last) = map.get(host) {
                if now.duration_since(*last) < Duration::from_secs(3600) {
                    continue;
                }
            }
            map.insert(host.clone(), now);
            if map.len() > 500 {
                map.retain(|_, t| now.duration_since(*t) < Duration::from_secs(6 * 3600));
            }
        }
        if already_listed(install, host) {
            continue;
        }
        per_cycle += 1;
        let (blocked, reason) = netcheck::looks_blocked(host);
        if !blocked {
            continue;
        }
        log::warn(format!("Сторож: {} похоже заблокирован ({}).", host, reason));
        if auto_add {
            let (message, count, _) = super::doctor::add_to_user_list(install, &[host.clone()]);
            if count > 0 {
                log::ok(format!("Сторож: {} добавлен в список ({})", host, reason));
                added.push(host.clone());
            } else {
                log::info(format!("Сторож: {} — {}", host, message));
            }
        }
        on_found(&format!(
            "{} — {}{}",
            host,
            reason,
            if auto_add {
                " (добавлен в список)"
            } else {
                " (требуется добавление)"
            }
        ));
    }
    added
}

/// Почистить устаревшие записи троттлинга.
pub fn trim() {
    if let Ok(mut guard) = CHECKED.lock() {
        if let Some(map) = guard.as_mut() {
            let now = Instant::now();
            map.retain(|_, t| now.duration_since(*t) < Duration::from_secs(6 * 3600));
        }
    }
}

/// Покрыт ли домен списками.
pub fn is_covered(install: &ZapretInstall, host: &str) -> bool {
    already_listed(install, host)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_dns_xml() {
        let xml = r#"<Event xmlns="http://schemas.microsoft.com/win/2004/08/events/event"><System><EventID>3020</EventID><EventRecordID>42</EventRecordID><Execution ProcessID="1234"/></System><EventData><Data Name="QueryName">Example.COM.</Data><Data Name="ClientPID">777</Data></EventData></Event>"#;
        let events = parse_events(xml);
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].host, "example.com");
        assert_eq!(events[0].pid, 777);
        assert_eq!(events[0].record_id, 42);
    }

    #[test]
    fn noise_filter() {
        assert!(is_noise("www.google.com"));
        assert!(!is_noise("example.com"));
    }
}
