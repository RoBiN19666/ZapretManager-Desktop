//! Проверка сайта: DNS → TCP → TLS → HTTP, связанные адреса, добавление в списки.

use std::net::IpAddr;
use std::path::Path;

use super::install::ZapretInstall;
use super::lists;
use super::netcheck;
use super::textfile;

#[derive(Debug, Clone, Default)]
pub struct SiteProbe {
    pub input: String,
    pub host: String,
    pub url: String,
    pub dns_ok: bool,
    pub ips: Vec<IpAddr>,
    pub dns_error: String,
    pub tcp_ok: bool,
    pub tcp_ms: u64,
    pub tcp_error: String,
    pub tls_ok: bool,
    pub tls_ms: u64,
    pub tls_error: String,
    pub http_ok: bool,
    pub http_status: u16,
    pub http_error: String,
    pub html_title: String,
    pub verdict: Verdict,
    pub verdict_text: String,
    pub related: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Verdict {
    #[default]
    Down,
    Ok,
    Dpi,
    Dns,
    Tls,
    Http,
}

impl Verdict {
    pub fn as_str(&self) -> &'static str {
        match self {
            Verdict::Ok => "ok",
            Verdict::Dpi => "dpi",
            Verdict::Dns => "dns",
            Verdict::Tls => "tls",
            Verdict::Http => "http",
            Verdict::Down => "down",
        }
    }
}

/// Нормализация адреса: домен или ссылка → (host, url для браузера).
pub fn normalize_input(input: &str) -> (String, String) {
    let s = input.trim().trim_matches(|c: char| matches!(c, '"' | '\'' | '<' | '>' | ' '));
    if s.is_empty() {
        return (String::new(), String::new());
    }
    let with_scheme = if s.contains("://") {
        s.to_string()
    } else {
        format!("https://{}", s)
    };
    let lower = with_scheme.to_lowercase();
    let (scheme, rest) = if let Some(r) = lower.strip_prefix("https://") {
        ("https", &with_scheme[with_scheme.len() - r.len()..])
    } else if let Some(r) = lower.strip_prefix("http://") {
        ("http", &with_scheme[with_scheme.len() - r.len()..])
    } else {
        return (String::new(), String::new());
    };
    let authority = rest.split(['/', '?', '#']).next().unwrap_or("");
    let (host_raw, port) = match authority.rsplit_once(':') {
        Some((h, p)) if !p.is_empty() && p.chars().all(|c| c.is_ascii_digit()) => (h, Some(p)),
        _ => (authority, None),
    };
    let host = host_raw
        .trim_matches(|c| c == '[' || c == ']')
        .trim_end_matches('.')
        .to_lowercase();
    if host.is_empty() || host.contains(' ') {
        return (String::new(), String::new());
    }
    let url = match port {
        Some(p) => format!("{}://{}:{}/", scheme, host, p),
        None => format!("{}://{}/", scheme, host),
    };
    (host, url)
}

/// Полная проверка сайта (последовательная, с прогрессом).
pub fn check(input: &str, mut progress: impl FnMut(&str)) -> SiteProbe {
    let mut probe = SiteProbe {
        input: input.to_string(),
        ..Default::default()
    };
    let (host, url) = normalize_input(input);
    if host.is_empty() {
        probe.verdict = Verdict::Down;
        probe.verdict_text =
            "Не удалось разобрать адрес. Введите домен или ссылку, например instagram.com.".to_string();
        return probe;
    }
    probe.host = host.clone();
    probe.url = url;

    progress(&format!("Определяем адрес {}…", host));
    match netcheck::resolve(&host, 5000) {
        Ok(ips) => {
            probe.dns_ok = true;
            probe.ips = ips;
        }
        Err(e) => {
            probe.dns_ok = false;
            probe.dns_error = e;
            probe.verdict = Verdict::Dns;
            probe.verdict_text = format!(
                "Домен не резолвится: {}. Это либо опечатка, либо DNS-блокировка — тогда помогает запись в hosts.",
                probe.dns_error
            );
            return probe;
        }
    }

    progress("Проверяем TCP-соединение…");
    let tcp = netcheck::tcp_connect(&host, 443, 5000);
    probe.tcp_ok = tcp.ok;
    probe.tcp_ms = tcp.ms;
    probe.tcp_error = tcp.error;

    if probe.tcp_ok {
        progress(&format!("Проверяем TLS-рукопожатие (SNI «{}»)…", host));
        let tls = netcheck::tls_handshake(&host, 443, 7000);
        probe.tls_ok = tls.ok;
        probe.tls_ms = tls.ms;
        probe.tls_error = tls.error;
    }
    if probe.tls_ok {
        progress("Запрашиваем страницу…");
        let http = netcheck::http_get(&host, 12000);
        probe.http_status = http.status;
        probe.html_title = http.title;
        probe.http_ok = http.error.is_empty() && (200..400).contains(&http.status);
        probe.http_error = http.error;
    }

    decide(&mut probe);
    probe
}

fn decide(probe: &mut SiteProbe) {
    let tcp_err = probe.tcp_error.to_lowercase();
    let tls_err = probe.tls_error.to_lowercase();
    let dpi_markers = ["таймаут", "reset", "сброс", "forcibly", "eof", "закрыт"];

    if !probe.tcp_ok {
        if dpi_markers.iter().any(|m| tcp_err.contains(m)) {
            probe.verdict = Verdict::Dpi;
        } else {
            probe.verdict = Verdict::Down;
        }
    } else if !probe.tls_ok {
        if dpi_markers.iter().any(|m| tls_err.contains(m)) {
            probe.verdict = Verdict::Dpi;
        } else {
            probe.verdict = Verdict::Tls;
        }
    } else if !probe.http_ok {
        probe.verdict = Verdict::Http;
    } else {
        probe.verdict = Verdict::Ok;
    }

    probe.verdict_text = match probe.verdict {
        Verdict::Ok => "Сайт открывается напрямую: соединение, TLS и HTTP отвечают. Скорее всего, ускорение для него не нужно — но если контент не грузится, блокируются отдельные адреса (CDN, API) — их можно добавить ниже.".to_string(),
        Verdict::Dpi => "Похоже на блокировку ТСПУ: DNS отвечает, а соединение рвётся (reset/таймаут) на этапе TCP или TLS с этим доменом. Такое лечится добавлением домена в списки zapret.".to_string(),
        Verdict::Dns => format!("Домен не резолвится: {}. Это либо опечатка, либо DNS-блокировка — тогда помогает запись в hosts.", probe.dns_error),
        Verdict::Tls => "TCP проходит, но TLS не поднимается. Это может быть блокировка по SNI или проблема сертификата/сервера.".to_string(),
        Verdict::Http => format!("Соединение и TLS в порядке, но сервер ответил ошибкой {}. Это не блокировка провайдера.", probe.http_status),
        Verdict::Down => "Сайт недоступен по сетевой причине; ускорение соединения вряд ли поможет.".to_string(),
    };
}

/// Корневой домен (последние 2 метки, для известных зон — 3).
pub fn registrable(host: &str) -> String {
    let labels: Vec<&str> = host.split('.').collect();
    if labels.len() <= 2 {
        return host.to_string();
    }
    let two = format!("{}.{}", labels[labels.len() - 2], labels[labels.len() - 1]);
    const TWO_LEVEL: [&str; 9] = [
        "co.uk", "org.uk", "com.br", "com.au", "co.jp", "co.kr", "com.tr", "co.in", "com.mx",
    ];
    if TWO_LEVEL.contains(&two.as_str()) && labels.len() >= 3 {
        format!("{}.{}", labels[labels.len() - 3], two)
    } else {
        two
    }
}

/// Подбор связанных адресов и их проверка (проблемные — первыми).
pub fn discover(probe: &mut SiteProbe, mut progress: impl FnMut(&str)) {
    let host = probe.host.clone();
    if host.is_empty() {
        return;
    }
    progress("Ищем адреса в странице…");
    let mut candidates: Vec<String> = Vec::new();

    fn add(cand: &str, list: &mut Vec<String>) {
        let c = cand.trim().trim_end_matches('.').to_lowercase();
        if c.len() < 4 || c.len() > 80 || !c.contains('.') {
            return;
        }
        if c.contains("w3.org")
            || c.contains("schema.org")
            || c.starts_with("fonts.")
            || c.ends_with(".local")
            || c.ends_with(".lan")
            || c.parse::<IpAddr>().is_ok()
        {
            return;
        }
        if !list.iter().any(|x| x.eq_ignore_ascii_case(&c)) {
            list.push(c);
        }
    }

    add(&host, &mut candidates);
    let root = registrable(&host);
    add(&root, &mut candidates);
    for sub in ["www", "m", "api", "static", "cdn", "i"] {
        add(&format!("{}.{}", sub, root), &mut candidates);
    }

    let html = fetch_html(&host);
    if !html.is_empty() {
        for found in extract_hosts(&html) {
            if candidates.len() >= 40 {
                break;
            }
            add(&found, &mut candidates);
        }
    }

    progress("Проверяем связанные адреса…");
    // Проверяем параллельно: последовательно 40 адресов занимали минуты.
    let checked: Vec<(String, bool)> = {
        let index = std::sync::atomic::AtomicUsize::new(0);
        let results = std::sync::Mutex::new(Vec::new());
        std::thread::scope(|scope| {
            for _ in 0..6.min(candidates.len().max(1)) {
                let index = &index;
                let candidates = &candidates;
                let results = &results;
                scope.spawn(move || loop {
                    let i = index.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                    if i >= candidates.len() {
                        break;
                    }
                    let cand = &candidates[i];
                    if netcheck::resolve(cand, 2000).is_err() {
                        continue;
                    }
                    let tcp = netcheck::tcp_connect(cand, 443, 1500);
                    let live = tcp.ok && netcheck::tls_handshake(cand, 443, 2500).ok;
                    if let Ok(mut list) = results.lock() {
                        list.push((cand.clone(), live));
                    }
                });
            }
        });
        results.into_inner().unwrap_or_default()
    };
    let mut broken = Vec::new();
    let mut live = Vec::new();
    for cand in &candidates {
        if let Some((_, is_live)) = checked.iter().find(|(c, _)| c == cand) {
            if *is_live {
                live.push(cand.clone());
            } else {
                broken.push(cand.clone());
            }
        }
    }
    broken.extend(live);
    probe.related = broken;
}

fn fetch_html(host: &str) -> String {
    let url = format!("https://{}/", host);
    match super::http::request(
        &url,
        "GET",
        &[(
            "User-Agent",
            "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/126.0 Safari/537.36",
        )],
        None,
        12000,
    ) {
        Ok(r) => r.text().chars().take(524288).collect(),
        Err(_) => String::new(),
    }
}

/// Простое извлечение имён хостов из HTML (без regex).
fn extract_hosts(html: &str) -> Vec<String> {
    let mut out = Vec::new();
    let lower = html.to_lowercase();
    let bytes = lower.as_bytes();
    let mut i = 0usize;
    while i + 2 < bytes.len() {
        if bytes[i] == b'/' && bytes[i + 1] == b'/' {
            let mut j = i + 2;
            while j < bytes.len()
                && (bytes[j].is_ascii_alphanumeric()
                    || bytes[j] == b'.'
                    || bytes[j] == b'-'
                    || bytes[j] == b'_')
            {
                j += 1;
            }
            let cand = &lower[i + 2..j];
            if cand.len() >= 4 && cand.contains('.') {
                out.push(cand.to_string());
            }
            i = j.max(i + 2);
            continue;
        }
        i += 1;
    }
    for quote in ['"', '\''] {
        let mut rest = lower.as_str();
        while let Some(start) = rest.find(quote) {
            let after = &rest[start + 1..];
            let Some(end) = after.find(quote) else { break };
            let token = &after[..end];
            if token.len() >= 4
                && token.contains('.')
                && token
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-')
            {
                out.push(token.to_string());
            }
            rest = &after[end..];
        }
    }
    out
}

/// Проверить список адресов (TCP/TLS), проблемные помечаются комментарием.
pub fn probe_list(
    hosts: &[String],
    mut progress: impl FnMut(usize, usize),
) -> (Vec<String>, usize, usize) {
    let mut ok = Vec::new();
    let mut broken = 0usize;
    let total = hosts.len();
    for (i, host) in hosts.iter().enumerate() {
        progress(i + 1, total);
        let tcp = netcheck::tcp_connect(host, 443, 2500);
        if !tcp.ok {
            ok.push(format!("{}     # TCP: {}", host, tcp.error));
            broken += 1;
            continue;
        }
        let tls = netcheck::tls_handshake(host, 443, 4000);
        if tls.ok {
            ok.push(host.clone());
        } else {
            ok.push(format!("{}     # TLS: {}", host, tls.error));
            broken += 1;
        }
    }
    (ok, total - broken, broken)
}

/// Дописать домены в lists\list-general-user.txt.
pub fn add_to_user_list(install: &ZapretInstall, hosts: &[String]) -> (String, usize, String) {
    if !install.is_valid() {
        return ("Папка zapret не найдена.".to_string(), 0, String::new());
    }
    let path = install.lists_dir().join("list-general-user.txt");
    let existing = lists::parse_entries(&textfile::read_lines(&path));
    let mut added = 0usize;
    let mut lines: Vec<String> = existing.clone();
    for raw in hosts {
        let mut host = raw.trim().to_lowercase();
        host = host.trim_start_matches('^').to_string();
        if let Some(rest) = host.strip_prefix("*.") {
            host = rest.to_string();
        }
        if host.is_empty() || !lists::is_host_like(&host) {
            continue;
        }
        if lines.iter().any(|l| l.eq_ignore_ascii_case(&host)) {
            continue;
        }
        lines.push(host);
        added += 1;
    }
    if added == 0 {
        return (
            "Все эти адреса уже есть в списке.".to_string(),
            0,
            path.to_string_lossy().to_string(),
        );
    }
    let mut text = String::new();
    for line in &lines {
        text.push_str(line);
        text.push_str("\r\n");
    }
    match textfile::write_text_atomic(&path, &text) {
        Ok(()) => (
            format!("Добавлено адресов: {}.", added),
            added,
            path.to_string_lossy().to_string(),
        ),
        Err(e) => (
            format!("Ошибка: {}", e),
            0,
            path.to_string_lossy().to_string(),
        ),
    }
}

pub const SITES_MARKER_START: &str = "# >>> Zapret Manager: адреса сайтов";
pub const SITES_MARKER_END: &str = "# <<< Zapret Manager";

/// Прописать IP-адреса связанных доменов в hosts.
pub fn add_to_hosts(hosts: &[String]) -> (String, usize, String) {
    if !super::sys::is_admin() {
        return (
            "Изменение hosts требует прав администратора.".to_string(),
            0,
            String::new(),
        );
    }
    let path = super::ops::hosts_path();
    let mut resolved: Vec<(String, String)> = Vec::new();
    for raw in hosts {
        let host = raw.trim().trim_start_matches('^').to_lowercase();
        if !lists::is_host_like(&host) {
            continue;
        }
        if let Ok(ips) = netcheck::resolve(&host, 3000) {
            if let Some(ip) = ips.iter().find(|i| i.is_ipv4()) {
                resolved.push((ip.to_string(), host));
            }
        }
    }
    if resolved.is_empty() {
        return (
            "Не удалось определить IP-адреса для этих доменов.".to_string(),
            0,
            String::new(),
        );
    }
    if !path.exists() {
        return ("Файл hosts не найден.".to_string(), 0, String::new());
    }
    let mut content = std::fs::read_to_string(&path).unwrap_or_default();
    if let Some(start) = content.find(SITES_MARKER_START) {
        let cut = match content[start..].find(SITES_MARKER_END) {
            Some(rel) => {
                let end = start + rel;
                match content[end..].find('\n') {
                    Some(nl) => end + nl + 1,
                    None => content.len(),
                }
            }
            None => content.len(),
        };
        content = format!("{}{}", &content[..start], &content[cut..]);
    }
    let stamp = super::sys::local_now();
    let backup = path.with_file_name(format!(
        "hosts.zapret-backup-{:04}{:02}{:02}-{:02}{:02}{:02}",
        stamp.year, stamp.month, stamp.day, stamp.hour, stamp.minute, stamp.second
    ));
    let _ = std::fs::copy(&path, &backup);

    let mut out = content.trim_end().to_string();
    out.push_str("\r\n\r\n");
    out.push_str(&format!(
        "{} ({:04}-{:02}-{:02} {:02}:{:02})\r\n",
        SITES_MARKER_START, stamp.year, stamp.month, stamp.day, stamp.hour, stamp.minute
    ));
    let mut added = 0usize;
    for (ip, host) in &resolved {
        let existing = content.lines().any(|l| {
            let mut parts = l.split_whitespace();
            matches!((parts.next(), parts.next()),
                (Some(a), Some(b)) if a == ip && b.eq_ignore_ascii_case(host))
        });
        if existing {
            continue;
        }
        out.push_str(&format!("{} {}\r\n", ip, host));
        added += 1;
    }
    out.push_str(SITES_MARKER_END);
    out.push_str("\r\n");
    match textfile::write_text_atomic(&path, &out) {
        Ok(()) => {
            let _ = super::process::run(
                "ipconfig.exe",
                &["/flushdns"],
                std::time::Duration::from_secs(8),
            );
            (
                format!("Записей в hosts добавлено: {}.", added),
                added,
                backup.to_string_lossy().to_string(),
            )
        }
        Err(e) => (
            format!("Ошибка: {}", e),
            0,
            backup.to_string_lossy().to_string(),
        ),
    }
}

/// Проверка списка доменов на блокировку (для проверки приложения).
pub fn blocked_hosts(
    hosts: &[String],
    mut progress: impl FnMut(usize, usize, &str),
) -> (Vec<String>, Vec<String>) {
    let mut blocked = Vec::new();
    let mut ok = Vec::new();
    let total = hosts.len();
    for (i, host) in hosts.iter().enumerate() {
        progress(i + 1, total, host);
        let (is_blocked, _reason) = netcheck::looks_blocked(host);
        if is_blocked {
            blocked.push(host.clone());
        } else {
            ok.push(host.clone());
        }
    }
    (blocked, ok)
}

/// Существует ли файл hosts.
pub fn hosts_file_exists() -> bool {
    Path::new(&super::ops::hosts_path()).is_file()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize() {
        assert_eq!(
            normalize_input("instagram.com"),
            ("instagram.com".to_string(), "https://instagram.com/".to_string())
        );
        assert_eq!(
            normalize_input("https://Example.COM/path?x=1"),
            ("example.com".to_string(), "https://example.com/".to_string())
        );
        assert!(normalize_input("").0.is_empty());
    }

    #[test]
    fn registrable_domains() {
        assert_eq!(registrable("a.b.example.com"), "example.com");
        assert_eq!(registrable("www.example.co.uk"), "example.co.uk");
        assert_eq!(registrable("example.com"), "example.com");
    }

    #[test]
    fn html_hosts() {
        let hosts = extract_hosts("<a href=\"//cdn.example.com/x\">x</a> \"api.test.io\"");
        assert!(hosts.contains(&"cdn.example.com".to_string()));
        assert!(hosts.contains(&"api.test.io".to_string()));
    }
}
