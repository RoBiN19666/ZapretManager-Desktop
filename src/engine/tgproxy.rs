//! TG WS Proxy: обнаружение, запуск/остановка, установка обновлений, ссылка tg://.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use super::config::Ini;
use super::http;
use super::log;
use super::process;
use super::sys;
use super::textfile;
use super::tgsource;

#[derive(Debug, Clone, Default)]
pub struct TgState {
    pub installed: bool,
    pub dir: PathBuf,
    pub exe: Option<PathBuf>,
    pub version: String,
    pub running: bool,
    pub pid: u32,
    pub started_unix: Option<u64>,
    pub host: String,
    pub port: u16,
    pub secret: String,
    pub config_error: String,
    pub port_listening: bool,
    pub instance_count: usize,
    pub startup_folder_exe: Option<PathBuf>,
    /// Выбран тихий режим (исходники вместо exe с иконкой в трее).
    pub quiet: bool,
    /// Тихий режим подготовлен (исходники + портативный Python).
    pub quiet_prepared: bool,
    /// Тихий режим реально запущен (процесс pythonw с tg_ws_proxy.py).
    pub quiet_running: bool,
}

impl TgState {
    pub fn link(&self) -> String {
        if self.secret.is_empty() {
            return String::new();
        }
        format!(
            "tg://proxy?server={}&port={}&secret={}",
            self.host, self.port, self.secret
        )
    }

    pub fn status_text(&self) -> String {
        if !self.installed {
            "не установлен".to_string()
        } else if self.running && self.port_listening {
            "работает".to_string()
        } else if self.running {
            "запускается".to_string()
        } else {
            "остановлен".to_string()
        }
    }
}

pub fn default_dir() -> PathBuf {
    let local = std::env::var("LOCALAPPDATA").unwrap_or_default();
    Path::new(&local)
        .join("ZapretManager")
        .join("components")
        .join("tgwsproxy")
}

pub fn proxy_config_path() -> PathBuf {
    let appdata = std::env::var("APPDATA").unwrap_or_default();
    Path::new(&appdata).join("TgWsProxy").join("config.json")
}

/// Режим запуска: "exe" (официальный файл с треем) или "quiet" (исходники без трея).
pub fn run_mode(ini: &Ini) -> String {
    // Если тихий режим уже подготовлен, а ключ потерялся (например, старый
    // менеджер перезаписал manager.ini), считаем выбранным тихий режим.
    let default = if tgsource::is_prepared() {
        "quiet"
    } else {
        "exe"
    };
    if ini
        .get("TgRunMode", default)
        .trim()
        .eq_ignore_ascii_case("quiet")
    {
        "quiet".to_string()
    } else {
        "exe".to_string()
    }
}

pub fn set_run_mode(ini: &mut Ini, quiet: bool) {
    ini.set("TgRunMode", if quiet { "quiet" } else { "exe" });
    let _ = ini.save();
}

pub fn proxy_log_path() -> PathBuf {
    let appdata = std::env::var("APPDATA").unwrap_or_default();
    Path::new(&appdata).join("TgWsProxy").join("proxy.log")
}

pub fn startup_folder() -> PathBuf {
    let appdata = std::env::var("APPDATA").unwrap_or_default();
    Path::new(&appdata)
        .join("Microsoft")
        .join("Windows")
        .join("Start Menu")
        .join("Programs")
        .join("Startup")
}

fn asset_name() -> &'static str {
    let arch = std::env::var("PROCESSOR_ARCHITECTURE").unwrap_or_default();
    if arch.to_uppercase().contains("ARM64") {
        "TgWsProxy_windows_arm64.exe"
    } else {
        "TgWsProxy_windows.exe"
    }
}

fn proxy_processes() -> Vec<sys::ProcInfo> {
    sys::list_processes()
        .into_iter()
        .filter(|p| p.name.to_lowercase().starts_with("tgwsproxy"))
        .collect()
}

/// Слушает ли порт на 127.0.0.1.
pub fn is_port_listening(port: u16) -> bool {
    std::net::TcpStream::connect_timeout(
        &std::net::SocketAddr::from(([127, 0, 0, 1], port)),
        Duration::from_millis(400),
    )
    .is_ok()
}

fn read_version_file(dir: &Path) -> String {
    textfile::read(&dir.join("version.txt")).trim().to_string()
}

fn read_proxy_config() -> (String, u16, String, String) {
    let mut host = "127.0.0.1".to_string();
    let mut port = 1443u16;
    let mut secret = String::new();
    let mut error = String::new();
    let path = proxy_config_path();
    if !path.is_file() {
        return (
            host,
            port,
            secret,
            "файл конфигурации ещё не создан (появится после первого запуска)".to_string(),
        );
    }
    match std::fs::read(&path)
        .map_err(|e| e.to_string())
        .and_then(|bytes| serde_json::from_slice::<serde_json::Value>(&bytes).map_err(|e| e.to_string()))
    {
        Ok(value) => {
            if let Some(h) = value.get("host").and_then(|v| v.as_str()) {
                host = h.to_string();
            }
            if let Some(p) = value.get("port").and_then(|v| v.as_u64()) {
                if p > 0 && p < 65536 {
                    port = p as u16;
                }
            }
            if let Some(s) = value.get("secret").and_then(|v| v.as_str()) {
                secret = s.to_string();
            }
        }
        Err(e) => error = format!("не удалось разобрать config.json: {}", e),
    }
    (host, port, secret, error)
}

/// Текущее состояние прокси.
pub fn get_state(ini: &Ini) -> TgState {
    let mut st = TgState {
        host: "127.0.0.1".to_string(),
        port: 1443,
        ..Default::default()
    };
    let (host, port, secret, config_error) = read_proxy_config();
    st.host = host;
    st.port = port;
    st.secret = secret;
    st.config_error = config_error;
    let quiet = run_mode(ini) == "quiet";
    st.quiet = quiet;

    // 1. Путь из настроек.
    let from_ini = ini.get("TgExe", "");
    let mut exe: Option<PathBuf> = None;
    if !from_ini.is_empty() && Path::new(&from_ini).is_file() {
        exe = Some(PathBuf::from(&from_ini));
    }
    // 2. Папка из настроек.
    if exe.is_none() {
        let dir = ini.get("TgDir", "");
        if !dir.is_empty() {
            exe = find_exe_in(Path::new(&dir));
        }
    }
    // 3. Папка по умолчанию.
    if exe.is_none() {
        exe = find_exe_in(&default_dir());
    }
    // 4. Запущенный процесс.
    let procs = proxy_processes();
    if exe.is_none() {
        if let Some(p) = procs.iter().find_map(|p| sys::process_exe_path(p.pid)) {
            exe = Some(p);
        }
    }
    // 5. Папка «Автозагрузка».
    if exe.is_none() {
        if let Some(p) = find_exe_in(&startup_folder()) {
            st.startup_folder_exe = Some(p.clone());
            exe = Some(p);
        }
    }

    st.instance_count = procs.len();
    if let Some(path) = exe.clone() {
        st.installed = true;
        st.dir = path
            .parent()
            .map(|p| p.to_path_buf())
            .unwrap_or_else(|| default_dir());
        st.version = read_version_file(&st.dir);
        if st.version.is_empty() {
            st.version = ini.get("TgVersion", "");
        }
        let exe_path = path.clone();
        if let Some(p) = procs.iter().find(|p| {
            sys::process_exe_path(p.pid)
                .map(|path| {
                    path.to_string_lossy()
                        .eq_ignore_ascii_case(&exe_path.to_string_lossy())
                })
                .unwrap_or(false)
        }) {
            st.running = true;
            st.pid = p.pid;
            st.started_unix = sys::process_start_unix(p.pid);
        }
        st.exe = Some(path);
    } else if !ini.get("TgDir", "").is_empty() {
        st.dir = PathBuf::from(ini.get("TgDir", ""));
    } else {
        st.dir = default_dir();
    }

    // Тихий режим: работаем из исходников, без иконки в трее.
    if quiet {
        st.quiet_prepared = tgsource::is_prepared();
        let quiet_procs = tgsource::find_processes();
        if !quiet_procs.is_empty() {
            st.running = true;
            st.quiet_running = true;
            st.instance_count = quiet_procs.len();
            st.pid = quiet_procs[0].pid;
            st.started_unix = sys::process_start_unix(quiet_procs[0].pid);
        }
        if st.quiet_prepared {
            st.installed = true;
            st.dir = tgsource::root_dir();
            st.exe = Some(tgsource::script_path());
            st.version = tgsource::prepared_version();
        }
    }
    st.port_listening = is_port_listening(st.port);
    st
}

fn find_exe_in(dir: &Path) -> Option<PathBuf> {
    if !dir.is_dir() {
        return None;
    }
    let mut candidates: Vec<PathBuf> = Vec::new();
    for entry in std::fs::read_dir(dir).ok()?.flatten() {
        let path = entry.path();
        let name = path.file_name().map(|s| s.to_string_lossy().to_lowercase()).unwrap_or_default();
        if name.starts_with("tgwsproxy") && name.ends_with(".exe") {
            candidates.push(path);
        }
    }
    candidates.sort_by_key(|p| {
        if p.file_name()
            .map(|n| n.to_string_lossy().eq_ignore_ascii_case("TgWsProxy_windows.exe"))
            .unwrap_or(false)
        {
            0
        } else {
            1
        }
    });
    candidates.into_iter().next()
}

/// Запустить прокси в выбранном режиме (exe или тихий из исходников).
pub fn start(ini: &Ini) -> (String, bool) {
    let st = get_state(ini);
    if st.quiet {
        if !st.quiet_prepared {
            return (
                "Тихий режим не подготовлен: нажмите «Подготовить» на странице «TG-прокси»."
                    .to_string(),
                false,
            );
        }
        if st.quiet_running {
            return (
                format!("TG WS Proxy уже запущен без трея (PID {}).", st.pid),
                true,
            );
        }
        if st.running {
            return (
                "Тихий режим включён, но сейчас работает TgWsProxy_windows.exe. Остановите его или нажмите «Перезапустить»."
                    .to_string(),
                false,
            );
        }
        return match tgsource::start(st.port, &st.secret) {
            Ok((_pid, message)) => {
                // Даём прокси время открыть порт, как в C#-версии.
                let deadline = Instant::now() + Duration::from_secs(6);
                while Instant::now() < deadline {
                    if is_port_listening(st.port) {
                        break;
                    }
                    std::thread::sleep(Duration::from_millis(300));
                }
                (
                    format!(
                        "{} Подключение в Telegram — кнопкой «Открыть в Telegram».",
                        message
                    ),
                    true,
                )
            }
            Err(error) => (error, false),
        };
    }
    if !st.installed {
        return (
            "TG WS Proxy не установлен. Установите его на странице «TG-прокси».".to_string(),
            false,
        );
    }
    if st.running {
        return (format!("TG WS Proxy уже запущен (PID {}).", st.pid), true);
    }
    let Some(exe) = &st.exe else {
        return ("Не удалось определить папку TG WS Proxy.".to_string(), false);
    };
    match process::spawn_hidden(&exe.to_string_lossy(), &[], Some(&st.dir)) {
        Ok(_) => {
            let deadline = Instant::now() + Duration::from_secs(7);
            while Instant::now() < deadline {
                if !proxy_processes().is_empty() {
                    return (
                        format!(
                            "TG WS Proxy запущен: {}:{}. Подключение в Telegram — кнопкой «Открыть в Telegram».",
                            st.host, st.port
                        ),
                        true,
                    );
                }
                std::thread::sleep(Duration::from_millis(300));
            }
            (
                "Файл запущен, но процесс TgWsProxy не найден. Проверьте журнал приложения и антивирус."
                    .to_string(),
                false,
            )
        }
        Err(e) => (
            format!("Не удалось запустить TG WS Proxy: {}", e),
            false,
        ),
    }
}

/// Остановить только копии из указанной папки (и тихий режим, если запущен).
pub fn stop(dir: &Path) -> String {
    let quiet_stopped = tgsource::stop();
    let mut killed = 0;
    for p in proxy_processes() {
        let Some(exe) = sys::process_exe_path(p.pid) else {
            continue;
        };
        let same = exe
            .parent()
            .map(|d| d.to_string_lossy().eq_ignore_ascii_case(&dir.to_string_lossy()))
            .unwrap_or(false);
        if same && sys::kill_process(p.pid) {
            killed += 1;
        }
    }
    if killed > 0 {
        std::thread::sleep(Duration::from_millis(400));
        format!("TG WS Proxy остановлен (процессов: {}).", killed)
    } else if quiet_stopped > 0 {
        "TG WS Proxy (тихий режим) остановлен.".to_string()
    } else {
        "TG WS Proxy уже остановлен.".to_string()
    }
}

/// Остановить все экземпляры (нужно при смене режима exe ↔ тихий).
pub fn stop_all() -> String {
    let quiet_stopped = tgsource::stop();
    let mut killed = 0usize;
    for p in proxy_processes() {
        if sys::kill_process(p.pid) {
            killed += 1;
        }
    }
    if killed > 0 {
        std::thread::sleep(Duration::from_millis(400));
        format!("TG WS Proxy остановлен (процессов: {}).", killed)
    } else if quiet_stopped > 0 {
        "TG WS Proxy (тихий режим) остановлен.".to_string()
    } else {
        "TG WS Proxy уже остановлен.".to_string()
    }
}

/// Последняя версия TG WS Proxy из GitHub.
pub fn check_latest() -> Result<(String, String), String> {
    let release = super::update::get_latest_release("Flowseal/tg-ws-proxy")?;
    let url = release
        .download_url_for(asset_name())
        .unwrap_or_else(|| {
            format!(
                "https://github.com/Flowseal/tg-ws-proxy/releases/download/{}/{}",
                release.tag,
                asset_name()
            )
        });
    Ok((release.tag, url))
}

/// Скачать и установить прокси.
pub fn install(
    tag: &str,
    dir: &Path,
    ini: &mut Ini,
    mut progress: impl FnMut(u32, &str),
) -> Result<String, String> {
    if tag.is_empty()
        || !tag
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'))
    {
        return Err(format!("Некорректный тег версии: {}", tag));
    }
    let url = format!(
        "https://github.com/Flowseal/tg-ws-proxy/releases/download/{}/{}",
        tag,
        asset_name()
    );
    let temp = super::paths::temp_dir().join(format!("tgwsproxy-{}.exe", tag));
    progress(2, &format!("Скачивание TG WS Proxy {} (~21 МБ)…", tag));
    http::download(&url, &temp, 300_000, |read, total| {
        let percent = if total > 0 { (read * 100 / total) as u32 } else { 50 };
        progress(2 + percent * 80 / 100, &format!("Скачивание TG WS Proxy: {}%", percent));
    })?;
    let size = std::fs::metadata(&temp).map(|m| m.len()).unwrap_or(0);
    if size < 5 * 1024 * 1024 {
        return Err(format!(
            "Скачанный файл слишком мал ({} байт) — вероятно, ошибка загрузки или блокировка антивирусом.",
            size
        ));
    }
    let head = std::fs::read(&temp).ok().filter(|b| b.len() >= 2).map(|b| b[0] == b'M' && b[1] == b'Z');
    if head != Some(true) {
        return Err(
            "Скачанный файл не является исполняемым (возможно, страница ошибки вместо файла)."
                .to_string(),
        );
    }
    progress(85, "Установка…");
    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let target = dir.join(asset_name());
    stop(dir);
    if target.exists() {
        let _ = std::fs::remove_file(&target);
    }
    std::fs::copy(&temp, &target).map_err(|e| e.to_string())?;
    let _ = textfile::write_text_atomic(&dir.join("version.txt"), tag);
    let _ = std::fs::remove_file(&temp);
    ini.set("TgExe", &target.to_string_lossy());
    ini.set("TgDir", &dir.to_string_lossy());
    ini.set("TgVersion", tag);
    let _ = ini.save();
    log::ok(format!("TG WS Proxy {} установлен: {}", tag, target.to_string_lossy()));
    progress(100, "Готово");
    Ok(format!(
        "TG WS Proxy {} установлен: {}",
        tag,
        target.to_string_lossy()
    ))
}

/// Открыть ссылку tg:// в Telegram.
pub fn open_in_telegram(link: &str) {
    if !link.is_empty() {
        sys::open_url(link);
    }
}

/// Папка прокси по умолчанию (для интерфейса).
pub fn dir_or_default(ini: &Ini) -> PathBuf {
    let saved = ini.get("TgDir", "");
    if !saved.is_empty() {
        PathBuf::from(saved)
    } else {
        default_dir()
    }
}
