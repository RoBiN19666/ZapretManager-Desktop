//! Тихий режим TG WS Proxy: официальные исходники Flowseal + портативный Python.
//!
//! Порт C#-класса `TgSource`: скачивает архив исходников и embeddable-сборку
//! Python с python.org, ставит зависимости (httpx, certifi, cryptography) и
//! запускает `proxy\tg_ws_proxy.py` через `pythonw.exe` без окна и без иконки
//! в трее. Пользователю не нужен установленный Python.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use super::http;
use super::log;
use super::process;
use super::sys;
use super::textfile;
use super::tgproxy;
use super::update;

pub const REPO: &str = "Flowseal/tg-ws-proxy";
pub const PYTHON_VERSION: &str = "3.13.5";
const REQUIRED_PACKAGES: &[&str] = &["httpx[http2]==0.28.1", "certifi", "cryptography"];

fn parent_dir() -> PathBuf {
    tgproxy::default_dir()
        .parent()
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| PathBuf::from("."))
}

/// Корень тихого режима: ...\ZapretManager\components\tgwsproxy-src.
pub fn root_dir() -> PathBuf {
    parent_dir().join("tgwsproxy-src")
}

pub fn src_dir() -> PathBuf {
    root_dir().join("src")
}

pub fn python_dir() -> PathBuf {
    root_dir().join("python")
}

pub fn python_exe() -> PathBuf {
    python_dir().join("python.exe")
}

pub fn pythonw_exe() -> PathBuf {
    python_dir().join("pythonw.exe")
}

pub fn site_packages() -> PathBuf {
    python_dir().join("Lib").join("site-packages")
}

pub fn marker_path() -> PathBuf {
    root_dir().join("prepared.txt")
}

pub fn script_path() -> PathBuf {
    src_dir().join("proxy").join("tg_ws_proxy.py")
}

pub fn log_path() -> PathBuf {
    parent_dir().join("tgwsproxy-source.log")
}

fn staging_dir() -> PathBuf {
    parent_dir().join("tgwsproxy-src.new")
}

fn old_dir() -> PathBuf {
    parent_dir().join("tgwsproxy-src.old")
}

/// Подготовлен ли тихий режим (исходники + Python + маркер).
pub fn is_prepared() -> bool {
    python_exe().is_file() && script_path().is_file() && marker_path().is_file()
}

/// Содержимое маркера подготовки.
pub fn prepared_info() -> String {
    textfile::read(&marker_path())
}

/// Версия из маркера: «v1.12.0» или «main».
pub fn prepared_version() -> String {
    for line in prepared_info().replace("\r\n", "\n").split('\n') {
        let line = line.trim();
        if let Some(value) = line
            .strip_prefix("версия:")
            .or_else(|| line.strip_prefix("Версия:"))
        {
            let value = value.trim();
            if !value.is_empty() {
                return value.to_string();
            }
        }
    }
    "исходники".to_string()
}

/// Запущенные процессы тихого режима (python.exe/pythonw.exe с tg_ws_proxy.py).
pub fn find_processes() -> Vec<sys::ProcInfo> {
    let marker = root_dir().to_string_lossy().to_lowercase();
    sys::list_processes()
        .into_iter()
        .filter(|p| {
            let name = p.name.to_ascii_lowercase();
            if name != "python.exe" && name != "pythonw.exe" {
                return false;
            }
            match sys::process_command_line(p.pid) {
                Some(cmd) => {
                    let cmd = cmd.to_lowercase();
                    cmd.contains("tg_ws_proxy.py") && cmd.contains(&marker)
                }
                None => false,
            }
        })
        .collect()
}

fn python_zip_name() -> String {
    let arch = std::env::var("PROCESSOR_ARCHITECTURE")
        .unwrap_or_default()
        .to_uppercase();
    let suffix = if arch.contains("ARM64") { "arm64" } else { "amd64" };
    format!("python-{}-embed-{}.zip", PYTHON_VERSION, suffix)
}

fn latest_ref() -> String {
    match update::get_latest_release(REPO) {
        Ok(release) if !release.tag.trim().is_empty() => release.tag,
        _ => "main".to_string(),
    }
}

fn valid_tag(tag: &str) -> bool {
    !tag.is_empty()
        && tag
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'))
}

fn find_payload(root: &Path, depth: usize) -> Option<PathBuf> {
    if root.join("proxy").join("tg_ws_proxy.py").is_file() {
        return Some(root.to_path_buf());
    }
    if depth == 0 {
        return None;
    }
    for entry in std::fs::read_dir(root).ok()?.flatten() {
        let path = entry.path();
        if path.is_dir() {
            if let Some(found) = find_payload(&path, depth - 1) {
                return Some(found);
            }
        }
    }
    None
}

fn copy_dir_all(src: &Path, dst: &Path) -> Result<(), String> {
    std::fs::create_dir_all(dst).map_err(|e| e.to_string())?;
    for entry in std::fs::read_dir(src).map_err(|e| e.to_string())?.flatten() {
        let path = entry.path();
        let target = dst.join(entry.file_name());
        if path.is_dir() {
            copy_dir_all(&path, &target)?;
        } else {
            std::fs::copy(&path, &target).map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

fn move_dir(src: &Path, dst: &Path) -> Result<(), String> {
    if let Some(parent) = dst.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    match std::fs::rename(src, dst) {
        Ok(()) => Ok(()),
        Err(_) => {
            copy_dir_all(src, dst)?;
            let _ = std::fs::remove_dir_all(src);
            Ok(())
        }
    }
}

/// Скачивает embeddable Python и раскладывает рядом site-packages.
fn ensure_portable(progress: &mut impl FnMut(u32, &str)) -> Result<(), String> {
    let dir = staging_dir().join("python");
    if dir.join("python.exe").is_file() {
        return Ok(());
    }
    let name = python_zip_name();
    let url = format!(
        "https://www.python.org/ftp/python/{}/{}",
        PYTHON_VERSION, name
    );
    let zip = super::paths::temp_dir().join(&name);
    let _ = std::fs::remove_file(&zip);
    progress(
        40,
        &format!("Скачивание портативного Python {} (≈10 МБ)…", PYTHON_VERSION),
    );
    http::download(&url, &zip, 300_000, |read, total| {
        let percent = if total > 0 {
            (read * 100 / total) as u32
        } else {
            0
        };
        progress(
            40 + percent * 18 / 100,
            &format!("Скачивание Python: {}%", percent),
        );
    })?;

    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    update::extract_zip(&zip, &dir)?;
    let _ = std::fs::remove_file(&zip);
    let site = dir.join("Lib").join("site-packages");
    std::fs::create_dir_all(&site).map_err(|e| e.to_string())?;

    // В embeddable-сборке пути задаются в python3xx._pth. Добавляем site-packages
    // и import site, иначе pip не увидит установленные пакеты.
    if let Ok(entries) = std::fs::read_dir(&dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            let name = path
                .file_name()
                .map(|n| n.to_string_lossy().to_lowercase())
                .unwrap_or_default();
            if !name.starts_with("python") || !name.ends_with("._pth") {
                continue;
            }
            let mut content = textfile::read(&path);
            let lower = content.to_lowercase();
            if !lower.contains("lib\\site-packages") {
                if !content.ends_with('\n') {
                    content.push('\n');
                }
                content.push_str("Lib\\site-packages\n");
            }
            if !lower.contains("import site") {
                if !content.ends_with('\n') {
                    content.push('\n');
                }
                content.push_str("import site\n");
            }
            textfile::write_text_atomic(&path, &content).map_err(|e| e.to_string())?;
            break;
        }
    }

    if !dir.join("python.exe").is_file() {
        return Err("Не удалось распаковать портативный Python.".to_string());
    }
    log::ok(format!(
        "Портативный Python {} распакован в {}",
        PYTHON_VERSION,
        dir.to_string_lossy()
    ));
    Ok(())
}

fn python_env(site: &Path) -> Vec<(&'static str, String)> {
    vec![
        ("PYTHONPATH", site.to_string_lossy().to_string()),
        ("PYTHONUTF8", "1".to_string()),
    ]
}

/// Ставит pip (get-pip.py) и пакеты в портативный Python.
fn ensure_packages(progress: &mut impl FnMut(u32, &str)) -> Result<(), String> {
    let dir = staging_dir().join("python");
    let python = dir.join("python.exe");
    let site = dir.join("Lib").join("site-packages");
    let envs = python_env(&site);
    let python_s = python.to_string_lossy().to_string();

    if !site.join("pip").is_dir() {
        progress(62, "Устанавливаем pip в портативный Python…");
        let get_pip = super::paths::temp_dir().join("get-pip.py");
        let _ = std::fs::remove_file(&get_pip);
        http::download(
            "https://bootstrap.pypa.io/get-pip.py",
            &get_pip,
            300_000,
            |_, _| {},
        )?;
        let get_pip_s = get_pip.to_string_lossy().to_string();
        let out = process::run_with_env(
            &python_s,
            &[&get_pip_s, "--no-warn-script-location", "--disable-pip-version-check"],
            Some(&dir),
            &envs,
            Duration::from_secs(300),
        );
        let _ = std::fs::remove_file(&get_pip);
        if !site.join("pip").is_dir() {
            return Err(format!(
                "Не удалось установить pip: {}",
                crate::util::shorten(&out.all(), 300)
            ));
        }
    }

    progress(
        70,
        "Устанавливаем зависимости (httpx, certifi, cryptography)…",
    );
    let mut args: Vec<String> = vec![
        "-m".to_string(),
        "pip".to_string(),
        "install".to_string(),
        "--no-warn-script-location".to_string(),
        "--disable-pip-version-check".to_string(),
        "--no-input".to_string(),
    ];
    for package in REQUIRED_PACKAGES {
        args.push((*package).to_string());
    }
    let refs: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
    let out = process::run_with_env(
        &python_s,
        &refs,
        Some(&dir),
        &envs,
        Duration::from_secs(600),
    );
    let check = process::run_with_env(
        &python_s,
        &["-c", "import httpx, certifi; print(httpx.__version__)"],
        Some(&dir),
        &envs,
        Duration::from_secs(60),
    );
    if check.exit_code != 0 || check.stdout.trim().is_empty() {
        return Err(format!(
            "Не удалось установить зависимости: {}",
            crate::util::shorten(&out.all(), 400)
        ));
    }
    Ok(())
}

fn verify_script(progress: &mut impl FnMut(u32, &str)) -> Result<(), String> {
    progress(92, "Проверяем запуск прокси из исходников…");
    let dir = staging_dir().join("python");
    let script = staging_dir()
        .join("src")
        .join("proxy")
        .join("tg_ws_proxy.py");
    let envs = python_env(&dir.join("Lib").join("site-packages"));
    let python_s = dir.join("python.exe").to_string_lossy().to_string();
    let script_s = script.to_string_lossy().to_string();
    let out = process::run_with_env(
        &python_s,
        &[&script_s, "--help"],
        Some(&dir),
        &envs,
        Duration::from_secs(40),
    );
    let text = out.all();
    if out.exit_code != 0 || text.trim().is_empty() {
        return Err(format!(
            "Прокси не запускается из исходников: {}",
            crate::util::shorten(&text, 300)
        ));
    }
    Ok(())
}

fn swap_into_place() -> Result<(), String> {
    let root = root_dir();
    let staging = staging_dir();
    let old = old_dir();
    let _ = std::fs::remove_dir_all(&old);
    if root.exists() {
        std::fs::rename(&root, &old).map_err(|e| {
            format!(
                "Не удалось освободить {}: {}",
                root.to_string_lossy(),
                e
            )
        })?;
    }
    match std::fs::rename(&staging, &root) {
        Ok(()) => {
            let _ = std::fs::remove_dir_all(&old);
            Ok(())
        }
        Err(e) => {
            if old.exists() {
                let _ = std::fs::rename(&old, &root);
            }
            Err(format!("Не удалось установить тихий режим: {}", e))
        }
    }
}

/// Скачать исходники, портативный Python и зависимости.
pub fn prepare(tag: &str, mut progress: impl FnMut(u32, &str)) -> Result<String, String> {
    let tag = if tag.trim().is_empty() || tag.eq_ignore_ascii_case("main") {
        "main".to_string()
    } else if valid_tag(tag.trim()) {
        tag.trim().to_string()
    } else {
        return Err(format!("Некорректный тег версии: {}", tag));
    };

    let stopped = stop();
    if stopped > 0 {
        log::info(format!(
            "Тихий режим TG WS Proxy остановлен перед обновлением (процессов: {}).",
            stopped
        ));
    }

    let staging = staging_dir();
    let _ = std::fs::remove_dir_all(&staging);
    std::fs::create_dir_all(staging.join("unpack")).map_err(|e| e.to_string())?;

    let work = staging.join("unpack");
    let zip = super::paths::temp_dir().join(format!("tgws-{}.zip", tag));
    let _ = std::fs::remove_file(&zip);
    let url = if tag == "main" {
        format!(
            "https://github.com/{}/archive/refs/heads/main.zip",
            REPO
        )
    } else {
        format!(
            "https://github.com/{}/archive/refs/tags/{}.zip",
            REPO, tag
        )
    };

    progress(8, &format!("Скачивание исходников {}…", tag));
    let download = http::download(&url, &zip, 300_000, |read, total| {
        let percent = if total > 0 {
            (read * 100 / total) as u32
        } else {
            0
        };
        progress(
            8 + percent * 22 / 100,
            &format!("Скачивание исходников: {}%", percent),
        );
    });
    if let Err(error) = download {
        let _ = std::fs::remove_file(&zip);
        let _ = std::fs::remove_dir_all(&staging);
        return Err(format!("Не удалось скачать исходники: {}", error));
    }

    progress(32, "Распаковка…");
    update::extract_zip(&zip, &work)?;
    let found = find_payload(&work, 3).ok_or_else(|| {
        "В архиве не найден proxy/tg_ws_proxy.py — похоже, изменилась структура репозитория."
            .to_string()
    })?;
    let src = staging.join("src");
    move_dir(&found, &src)?;
    let _ = std::fs::remove_file(&zip);
    let _ = std::fs::remove_dir_all(&work);

    if let Err(error) = ensure_portable(&mut progress) {
        let _ = std::fs::remove_dir_all(&staging);
        return Err(error);
    }
    if let Err(error) = ensure_packages(&mut progress) {
        let _ = std::fs::remove_dir_all(&staging);
        return Err(error);
    }
    if let Err(error) = verify_script(&mut progress) {
        let _ = std::fs::remove_dir_all(&staging);
        return Err(error);
    }

    let now = sys::local_now();
    let marker = format!(
        "версия: {}\npython: {}\nпортативный: да\nподготовлено: {}\n",
        tag,
        python_exe().to_string_lossy(),
        now
    );
    textfile::write_text_atomic(&staging.join("prepared.txt"), &marker)
        .map_err(|e| e.to_string())?;

    progress(96, "Устанавливаем тихий режим…");
    if let Err(error) = swap_into_place() {
        let _ = std::fs::remove_dir_all(&staging);
        return Err(error);
    }
    progress(100, "Готово");
    log::ok(format!(
        "Тихий режим TG WS Proxy подготовлен ({}).",
        tag
    ));
    Ok(format!(
        "Тихий режим готов: исходники {} и портативный Python {} в {}",
        tag,
        PYTHON_VERSION,
        root_dir().to_string_lossy()
    ))
}

/// Подготовить по последнему релизу (как кнопка «Подготовить» в интерфейсе).
pub fn prepare_latest(progress: impl FnMut(u32, &str)) -> Result<String, String> {
    prepare(&latest_ref(), progress)
}

/// Запустить исходники через pythonw.exe (без окна и трея).
pub fn start(port: u16, secret: &str) -> Result<(u32, String), String> {
    if !is_prepared() {
        return Err(
            "Тихий режим не подготовлен: нажмите «Подготовить» на странице «TG-прокси»."
                .to_string(),
        );
    }
    if let Some(proc) = find_processes().into_iter().next() {
        return Ok((proc.pid, format!("TG WS Proxy уже запущен (PID {}).", proc.pid)));
    }

    let pythonw = if pythonw_exe().is_file() {
        pythonw_exe()
    } else {
        python_exe()
    };
    let script = script_path();
    let port = if port == 0 { 1443 } else { port };
    let log_file = log_path();
    let mut args: Vec<String> = vec![
        script.to_string_lossy().to_string(),
        "--host".to_string(),
        "127.0.0.1".to_string(),
        "--port".to_string(),
        port.to_string(),
    ];
    if secret.len() == 32 {
        args.push("--secret".to_string());
        args.push(secret.to_string());
    }
    args.push("--log-file".to_string());
    args.push(log_file.to_string_lossy().to_string());

    let site = site_packages();
    let python_s = pythonw.to_string_lossy().to_string();
    let mut cmd = process::command(&python_s);
    cmd.args(&args)
        .current_dir(src_dir())
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .env("PYTHONPATH", site.to_string_lossy().to_string())
        .env("PYTHONUTF8", "1");
    let child = cmd
        .spawn()
        .map_err(|e| format!("Не удалось запустить Python: {}", e))?;
    let spawned_pid = child.id();
    drop(child);

    let deadline = Instant::now() + Duration::from_secs(9);
    while Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(300));
        if let Some(proc) = find_processes().into_iter().next() {
            if sys::process_exit_code(proc.pid).is_none()
                || sys::process_exit_code(proc.pid) == Some(259)
            {
                log::ok(format!(
                    "TG WS Proxy запущен из исходников (PID {}, порт {}).",
                    proc.pid, port
                ));
                return Ok((
                    proc.pid,
                    format!(
                        "TG WS Proxy запущен без трея: 127.0.0.1:{} (PID {}).",
                        port, proc.pid
                    ),
                ));
            }
        }
        if let Some(code) = sys::process_exit_code(spawned_pid) {
            if code != 259 && find_processes().is_empty() {
                break;
            }
        }
    }

    let tail = tail_of_log(8);
    Err(format!(
        "Процесс не появился. Проверьте журнал {} (PID попытки {}).{}",
        log_file.to_string_lossy(),
        spawned_pid,
        if tail.is_empty() {
            String::new()
        } else {
            format!("\n{}", tail)
        }
    ))
}

fn tail_of_log(lines: usize) -> String {
    let text = textfile::read(&log_path());
    let all: Vec<&str> = text.lines().filter(|l| !l.trim().is_empty()).collect();
    let start = all.len().saturating_sub(lines);
    all[start..].join("\n")
}

/// Остановить все процессы тихого режима.
pub fn stop() -> usize {
    let mut killed = 0usize;
    for proc in find_processes() {
        if sys::kill_process(proc.pid) {
            killed += 1;
        }
    }
    if killed > 0 {
        let deadline = Instant::now() + Duration::from_secs(3);
        while Instant::now() < deadline && !find_processes().is_empty() {
            std::thread::sleep(Duration::from_millis(150));
        }
        log::info(format!(
            "Тихий режим TG WS Proxy остановлен (процессов: {}).",
            killed
        ));
    }
    killed
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn marker_version_parse() {
        let text = "версия: v1.12.0\npython: C:\\x\\python.exe\n";
        let mut found = String::new();
        for line in text.lines() {
            if let Some(value) = line.strip_prefix("версия:") {
                found = value.trim().to_string();
            }
        }
        assert_eq!(found, "v1.12.0");
    }

    #[test]
    fn tag_validation() {
        assert!(valid_tag("v1.12.0"));
        assert!(!valid_tag("../evil"));
        assert!(!valid_tag(""));
    }
}
