//! Проверка приложения: какие домены запрашивает программа и что из них блокируется.

use std::collections::HashSet;
use std::time::Duration;

use super::guard;
use super::log;
use super::process;
use super::sys;

#[derive(Debug, Clone, Default)]
pub struct AppEntry {
    pub pid: u32,
    pub name: String,
    pub title: String,
    pub path: String,
}

impl AppEntry {
    pub fn display(&self) -> String {
        let mut s = format!("{}  (PID {})", self.name, self.pid);
        if !self.title.is_empty() {
            s.push_str(&format!("  —  {}", self.title));
        }
        s
    }
}

const SYSTEM_HOSTS: [&str; 12] = [
    "explorer",
    "dwm",
    "taskhostw",
    "sihost",
    "ctfmon",
    "runtimebroker",
    "searchapp",
    "startmenuexperiencehost",
    "shellexperiencehost",
    "applicationframehost",
    "textinputhost",
    "lockapp",
];

/// Приложения с видимыми окнами (кроме системных оболочек).
pub fn list_apps() -> Vec<AppEntry> {
    let processes = sys::list_processes();
    let windows = sys::visible_windows();
    let mut map: std::collections::HashMap<u32, AppEntry> = std::collections::HashMap::new();
    for (pid, title) in windows {
        if pid <= 4 || map.contains_key(&pid) {
            continue;
        }
        let Some(proc) = processes.iter().find(|p| p.pid == pid) else {
            continue;
        };
        let base = proc.name.trim_end_matches(".exe").to_lowercase();
        if SYSTEM_HOSTS.contains(&base.as_str()) {
            continue;
        }
        let title = if title.chars().count() > 60 {
            let mut short: String = title.chars().take(59).collect();
            short.push('…');
            short
        } else {
            title
        };
        let path = sys::process_exe_path(pid)
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_default();
        map.insert(
            pid,
            AppEntry {
                pid,
                name: proc.name.clone(),
                title,
                path,
            },
        );
    }
    let mut list: Vec<AppEntry> = map.into_values().collect();
    list.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    list
}

/// Все процессы той же программы (то же имя образа и тот же путь).
pub fn same_app(pid: u32) -> Vec<u32> {
    let processes = sys::list_processes();
    let Some(root) = processes.iter().find(|p| p.pid == pid).cloned() else {
        return vec![pid];
    };
    let root_path = sys::process_exe_path(pid).unwrap_or_default();
    processes
        .iter()
        .filter(|p| p.name.eq_ignore_ascii_case(&root.name))
        .filter(|p| {
            if root_path.as_os_str().is_empty() {
                true
            } else {
                sys::process_exe_path(p.pid)
                    .map(|path| {
                        path.to_string_lossy()
                            .eq_ignore_ascii_case(&root_path.to_string_lossy())
                    })
                    .unwrap_or(true)
            }
        })
        .map(|p| p.pid)
        .collect()
}

/// Транзитивное замыкание дочерних процессов.
pub fn with_children(pid: u32) -> Vec<u32> {
    let processes = sys::list_processes();
    let mut result = vec![pid];
    loop {
        let mut added = false;
        for p in &processes {
            if result.contains(&p.pid) {
                continue;
            }
            if result.contains(&p.parent_pid) {
                result.push(p.pid);
                added = true;
            }
        }
        if !added {
            break;
        }
    }
    result
}

/// Внешние IP-адреса соединений процесса (из netstat).
pub fn remote_ips(pid: u32) -> Vec<String> {
    let r = process::run("netstat.exe", &["-ano", "-p", "TCP"], Duration::from_secs(15));
    let text = r.all();
    let mut out: Vec<String> = Vec::new();
    for line in text.lines() {
        let cols: Vec<&str> = line.split_whitespace().collect();
        if cols.len() < 5 || !cols[0].eq_ignore_ascii_case("TCP") {
            continue;
        }
        let Ok(line_pid) = cols[cols.len() - 1].parse::<u32>() else {
            continue;
        };
        if line_pid != pid {
            continue;
        }
        let remote = cols[2];
        let host = match remote.rfind(':') {
            Some(pos) => remote[..pos].trim_matches(|c| c == '[' || c == ']'),
            None => remote,
        };
        if host.is_empty() || host.starts_with("0.0.0.0") || host.starts_with("127.") {
            continue;
        }
        if is_private(host) {
            continue;
        }
        if !out.iter().any(|x| x == host) {
            out.push(host.to_string());
        }
    }
    out
}

fn is_private(host: &str) -> bool {
    let Ok(ip) = host.parse::<std::net::IpAddr>() else {
        return false;
    };
    match ip {
        std::net::IpAddr::V4(v4) => {
            let oct = v4.octets();
            oct[0] == 10
                || (oct[0] == 172 && (16..=31).contains(&oct[1]))
                || (oct[0] == 192 && oct[1] == 168)
                || (oct[0] == 169 && oct[1] == 254)
        }
        std::net::IpAddr::V6(v6) => v6.is_loopback(),
    }
}

/// Наблюдение за приложением: домены из журнала DNS + внешние адреса.
pub fn watch(
    pid: u32,
    seconds: u32,
    mut progress: impl FnMut(u32, usize, usize),
) -> (Vec<String>, Vec<String>) {
    // Наблюдение возможно только по журналу DNS-клиента. Включаем его сами,
    // чтобы пользователю не нужно было искать отдельную кнопку.
    if !guard::log_enabled() {
        let message = guard::enable_log();
        log::info(format!("Наблюдение за приложением: {}", message));
    }
    let mut allowed: Vec<u32> = same_app(pid);
    for child in with_children(pid) {
        if !allowed.contains(&child) {
            allowed.push(child);
        }
    }
    let mut hosts: Vec<String> = Vec::new();
    let mut ips: Vec<String> = Vec::new();
    let mut step = 0u32;
    while step <= seconds {
        let (events, _err, _scanned) = guard::new_events(Some(&allowed), false);
        for ev in events {
            if ev.host.is_empty() {
                continue;
            }
            if hosts.iter().any(|h| h.eq_ignore_ascii_case(&ev.host)) {
                continue;
            }
            hosts.push(ev.host);
        }
        for ip in remote_ips(pid) {
            if !ips.contains(&ip) {
                ips.push(ip);
            }
        }
        progress(step, hosts.len(), ips.len());
        if step >= seconds {
            break;
        }
        std::thread::sleep(Duration::from_secs(2));
        step += 2;
    }
    (hosts, ips)
}

/// Убрать служебные домены из результатов наблюдения.
pub fn filter_hosts(hosts: &[String]) -> Vec<String> {
    let mut seen: HashSet<String> = HashSet::new();
    hosts
        .iter()
        .filter(|h| {
            let h = h.to_lowercase();
            !guard::NOISE
                .iter()
                .any(|n| h == *n || h.ends_with(&format!(".{}", n)))
        })
        .filter(|h| seen.insert(h.to_lowercase()))
        .cloned()
        .collect()
}
