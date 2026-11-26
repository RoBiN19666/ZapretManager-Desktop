//! Управление установкой zapret: состояние, запуск/остановка, служба, списки IP,
//! фейк-пакеты, hosts, кэш Discord, резервные копии.

use std::io::BufRead;
use std::path::{Path, PathBuf};
use std::process::Child;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use crate::util::{format_size, sha256_file};

use super::bat;
use super::gamefilter::GameFilterState;
use super::install::{StrategyDef, ZapretInstall};
use super::log;
use super::process::{self, ProcResult};
use super::service;
use super::sys;
use super::textfile;

#[derive(Debug, Clone, Default)]
pub struct RuntimeState {
    pub winws_running: bool,
    pub winws_pid: u32,
    pub winws_command_line: String,
    pub winws_started_unix: Option<u64>,
    pub matched_strategy: String,
    pub service_installed: bool,
    pub service_status: String,
    pub service_strategy: String,
    pub game_filter: GameFilterState,
    pub ipset_mode: String,
    pub ipset_lines: u64,
    pub check_updates: bool,
}

#[derive(Debug, Clone, Default)]
pub struct StartResult {
    pub ok: bool,
    pub message: String,
    pub pid: u32,
}

#[derive(Debug, Clone, Default)]
pub struct FakeFile {
    pub name: String,
    pub full_path: PathBuf,
    pub sha256: String,
}

#[derive(Debug, Clone, Default)]
pub struct HostsStatus {
    pub downloaded: bool,
    pub remote_lines: usize,
    pub missing_lines: usize,
    pub error: String,
    pub temp_file: PathBuf,
}

struct WinwsHandle {
    child: Child,
    pid: u32,
    started_unix: u64,
}

static WINWS: OnceLock<Mutex<Option<WinwsHandle>>> = OnceLock::new();
static WINWS_OUTPUT: OnceLock<Mutex<Vec<String>>> = OnceLock::new();
static MATCH_CACHE: OnceLock<Mutex<(String, String)>> = OnceLock::new();

fn winws_slot() -> &'static Mutex<Option<WinwsHandle>> {
    WINWS.get_or_init(|| Mutex::new(None))
}

fn output_slot() -> &'static Mutex<Vec<String>> {
    WINWS_OUTPUT.get_or_init(|| Mutex::new(Vec::new()))
}

fn push_output(line: String) {
    if let Ok(mut out) = output_slot().lock() {
        out.push(line);
        while out.len() > 500 {
            out.remove(0);
        }
    }
}

/// Последний вывод winws.exe (для сообщений об ошибке).
pub fn recent_output() -> String {
    output_slot()
        .lock()
        .map(|o| o.join("\n"))
        .unwrap_or_default()
}

fn clear_output() {
    if let Ok(mut out) = output_slot().lock() {
        out.clear();
    }
}

pub struct ZapretOps {
    pub install: ZapretInstall,
}

impl ZapretOps {
    pub fn new(install: ZapretInstall) -> ZapretOps {
        ZapretOps { install }
    }

    // ------------------------------------------------------------------
    // Состояние
    // ------------------------------------------------------------------

    pub fn get_state(&self) -> RuntimeState {
        let mut st = RuntimeState {
            game_filter: GameFilterState::load(&self.install.root),
            check_updates: self.install.check_updates_file().is_file(),
            ipset_mode: self.get_ipset_mode(),
            ipset_lines: count_list_lines(&self.install.ipset_file()),
            ..Default::default()
        };
        let snap = service::query("zapret");
        st.service_installed = snap.installed;
        st.service_status = snap.status.clone();
        st.service_strategy = snap.strategy.clone();

        let procs = list_winws();
        if let Some(first) = procs.first() {
            st.winws_running = true;
            st.winws_pid = first.0;
            st.winws_command_line = first.1.clone();
            st.matched_strategy = self.match_strategy(&first.1);
        }
        if st.matched_strategy.is_empty()
            && snap.installed
            && snap.status == "работает"
            && !snap.strategy.is_empty()
        {
            st.matched_strategy = snap.strategy;
        }
        if st.winws_running {
            st.winws_started_unix = sys::process_start_unix(st.winws_pid);
        }
        st
    }

    pub fn match_strategy(&self, command_line: &str) -> String {
        if command_line.is_empty() {
            return String::new();
        }
        let key = format!("{}|{}|{}", self.install.root, self.install.strategies.len(), command_line);
        let cache = MATCH_CACHE.get_or_init(|| Mutex::new((String::new(), String::new())));
        if let Ok(c) = cache.lock() {
            if c.0 == key {
                return c.1.clone();
            }
        }
        let norm = bat::normalize_command_line(command_line);
        let gf = GameFilterState::load(&self.install.root);
        let mut matched = String::new();
        for s in &self.install.strategies {
            if let Ok(argv) = bat::get_argv(&self.install, s, &gf) {
                let candidate = bat::normalize_command_line(&bat::build_command_line(&argv));
                if !candidate.is_empty() && norm.ends_with(&candidate) {
                    matched = s.name.clone();
                    break;
                }
            }
        }
        if let Ok(mut c) = cache.lock() {
            *c = (key, matched.clone());
        }
        matched
    }

    pub fn service_install_root(&self) -> Option<String> {
        ZapretInstall::service_root_from_registry()
    }

    pub fn service_points_elsewhere(&self) -> bool {
        match self.service_install_root() {
            Some(root) if ZapretInstall::looks_valid(Path::new(&root)) => {
                root.to_lowercase() != self.install.root.to_lowercase()
            }
            _ => false,
        }
    }

    pub fn service_path_missing(&self) -> bool {
        let snap = service::query("zapret");
        if !snap.installed {
            return false;
        }
        match self.service_install_root() {
            Some(root) => !ZapretInstall::looks_valid(Path::new(&root)),
            None => true,
        }
    }

    /// Лучшая стратегия для запуска: текущая, последняя из настроек, general, первая.
    pub fn pick_strategy(&self, last: &str) -> Option<&StrategyDef> {
        if !self.install.is_valid() || self.install.strategies.is_empty() {
            return None;
        }
        let state = self.get_state();
        if let Some(s) = self.install.find_strategy(&state.matched_strategy) {
            return Some(s);
        }
        if let Some(s) = self.install.find_strategy(last) {
            return Some(s);
        }
        self.install.default_strategy()
    }

    // ------------------------------------------------------------------
    // Запуск / остановка
    // ------------------------------------------------------------------

    /// Завершает только winws.exe, запущенный из папки этой установки.
    pub fn kill_winws(&self) -> i32 {
        let procs = list_winws();
        if procs.is_empty() {
            return 0;
        }
        let any_cmd = procs.iter().any(|(_, cmd)| !cmd.is_empty());
        if !any_cmd {
            for (pid, _) in &procs {
                sys::kill_process(*pid);
            }
            return -1;
        }
        let root = if self.install.root.ends_with('\\') {
            self.install.root.clone()
        } else {
            format!("{}\\", self.install.root)
        }
        .to_lowercase();
        let mut killed = 0;
        for (pid, cmd) in procs {
            if !cmd.to_lowercase().contains(&root) {
                continue;
            }
            if sys::kill_process(pid) {
                killed += 1;
            }
        }
        if let Ok(mut slot) = winws_slot().lock() {
            if let Some(mut handle) = slot.take() {
                let _ = handle.child.kill();
                let _ = handle.child.wait();
            }
        }
        killed
    }

    pub fn start(&self, strategy: &StrategyDef) -> StartResult {
        let mut res = StartResult::default();
        if !sys::is_admin() {
            res.message = "Запуск требует прав администратора.".to_string();
            return res;
        }
        self.install.ensure_user_lists();
        let snap = service::query("zapret");
        if snap.installed && snap.status == "работает" {
            res.message =
                "Служба zapret уже запущена. Сначала удалите службу, чтобы запускать стратегии вручную."
                    .to_string();
            return res;
        }
        if !list_winws().is_empty() {
            log::info("Останавливаю winws.exe этой установки перед запуском новой стратегии…");
            self.kill_winws();
            std::thread::sleep(Duration::from_millis(400));
            if !list_winws().is_empty() {
                res.message =
                    "Найден winws.exe из другой установки. Закройте его вручную и повторите."
                        .to_string();
                return res;
            }
        }

        let gf = GameFilterState::load(&self.install.root);
        let argv = match bat::get_argv(&self.install, strategy, &gf) {
            Ok(a) => a,
            Err(e) => {
                res.message = e;
                return res;
            }
        };
        let cmd = bat::build_command_line(&argv);
        // Сначала фиксируем исходное состояние, потом включаем (как в C#-версии).
        let tcp_was_off = self.tcp_timestamps_enabled() == Some(false);
        if tcp_was_off {
            self.enable_tcp_timestamps();
        }

        clear_output();
        let mut child = match spawn_winws(&self.install.winws_exe(), &cmd, &self.install.bin_dir()) {
            Ok(c) => c,
            Err(e) => {
                res.message = format!("Процесс winws.exe не создан: {}", e);
                log::error(&res.message);
                return res;
            }
        };
        let pid = child.id();
        if let Some(stdout) = child.stdout.take() {
            let mut reader = std::io::BufReader::new(stdout);
            std::thread::spawn(move || {
                let mut line = String::new();
                loop {
                    line.clear();
                    match reader.read_line(&mut line) {
                        Ok(0) | Err(_) => break,
                        Ok(_) => push_output(line.trim_end().to_string()),
                    }
                }
            });
        }
        if let Some(stderr) = child.stderr.take() {
            let mut reader = std::io::BufReader::new(stderr);
            std::thread::spawn(move || {
                let mut line = String::new();
                loop {
                    line.clear();
                    match reader.read_line(&mut line) {
                        Ok(0) | Err(_) => break,
                        Ok(_) => push_output(line.trim_end().to_string()),
                    }
                }
            });
        }

        let started_unix = crate::util::unix_now();
        if let Ok(mut slot) = winws_slot().lock() {
            *slot = Some(WinwsHandle {
                child,
                pid,
                started_unix,
            });
        }

        std::thread::sleep(Duration::from_millis(1200));
        if let Some(code) = exited_code() {
            let output = recent_output();
            res.message = format!(
                "winws.exe завершился сразу после запуска (код {}).{}",
                code,
                if output.is_empty() {
                    " Проверьте права администратора и файл WinDivert64.sys.".to_string()
                } else {
                    format!(" Вывод: {}", crate::util::shorten(&output, 400))
                }
            );
            log::error(&res.message);
            return res;
        }
        std::thread::sleep(Duration::from_millis(800));
        if let Some(code) = exited_code() {
            let output = recent_output();
            res.message = format!(
                "winws.exe завершился через пару секунд (код {}).{}",
                code,
                if output.is_empty() {
                    " Проверьте WinDivert и списки.".to_string()
                } else {
                    format!(" Вывод: {}", crate::util::shorten(&output, 400))
                }
            );
            log::error(&res.message);
            return res;
        }
        let early = recent_output();
        if early.to_lowercase().contains("error") || early.to_lowercase().contains("failed") {
            log::warn(format!("winws.exe сообщает: {}", crate::util::shorten(&early, 400)));
        }

        res.ok = true;
        res.pid = pid;
        res.message = format!("Стратегия «{}» запущена (PID {}).", strategy.name, pid);
        if tcp_was_off {
            res.message.push_str(" TCP timestamps были выключены и включены (системная настройка).");
        }
        log::ok(&res.message);
        self.label_services();
        res
    }

    pub fn stop(&self) -> String {
        let mut msg = String::new();
        let snap = service::query("zapret");
        if snap.installed && snap.status == "работает" {
            let _ = service::stop("zapret");
            if wait_service("zapret", "остановлена", Duration::from_secs(15)) {
                msg.push_str("Служба zapret остановлена. ");
                log::info("Служба zapret остановлена.");
            } else {
                msg.push_str("Не удалось дождаться остановки службы zapret. ");
                log::warn("Служба zapret не остановилась.");
            }
        }
        if !list_winws().is_empty() {
            let killed = self.kill_winws();
            if killed != 0 {
                msg.push_str("Процесс winws.exe завершён.");
                log::ok("Процесс winws.exe завершён.");
            } else if !list_winws().is_empty() {
                msg.push_str("Остался winws.exe из другой установки — он не тронут.");
            }
        }
        if msg.is_empty() {
            msg = "Ускорение уже остановлено.".to_string();
        }
        msg.trim().to_string()
    }

    pub fn restart(&self, strategy: &StrategyDef) -> String {
        self.stop();
        std::thread::sleep(Duration::from_millis(500));
        self.start(strategy).message
    }

    // ------------------------------------------------------------------
    // TCP timestamps
    // ------------------------------------------------------------------

    pub fn enable_tcp_timestamps(&self) -> Option<bool> {
        if self.tcp_timestamps_enabled() == Some(true) {
            return Some(true);
        }
        let r = process::run(
            "netsh.exe",
            &["interface", "tcp", "set", "global", "timestamps=enabled"],
            Duration::from_secs(10),
        );
        if r.exit_code != 0 {
            log::warn(format!("netsh timestamps=enabled: {}", r.all()));
            return Some(false);
        }
        self.tcp_timestamps_enabled()
    }

    pub fn tcp_timestamps_enabled(&self) -> Option<bool> {
        let r = process::run(
            "cmd.exe",
            &["/c", "chcp 437 >nul & netsh interface tcp show global"],
            Duration::from_secs(10),
        );
        let text = r.all().to_lowercase();
        let idx = text.find("timestamps").or_else(|| text.find("метки"))?;
        let rest = &text[idx..];
        let colon = rest.find(':')?;
        let value = rest[colon + 1..].trim();
        if value.starts_with("enabled") || value.contains("включ") {
            Some(true)
        } else if value.starts_with("disabled") || value.contains("выключ") {
            Some(false)
        } else {
            None
        }
    }

    // ------------------------------------------------------------------
    // Служба
    // ------------------------------------------------------------------

    pub fn label_services(&self) {
        if !sys::is_admin() {
            return;
        }
        if service::exists("zapret") {
            service::label(
                "zapret",
                "Zapret — ускорение соединения (Zapret Manager)",
                "Ускорение соединения для Discord и YouTube. Запускает winws.exe; управляется Zapret Manager.",
            );
        }
        for name in ["WinDivert", "WinDivert14"] {
            if service::exists(name) {
                service::label(
                    name,
                    "WinDivert — драйвер ускорения соединения (Zapret Manager)",
                    "Драйвер перехвата пакетов WinDivert, используется winws.exe для ускорения соединения.",
                );
            }
        }
    }

    pub fn install_service(&self, strategy: &StrategyDef) -> Result<String, String> {
        if !sys::is_admin() {
            return Err("Установка службы требует прав администратора.".to_string());
        }
        if !self.install.winws_exe().is_file() {
            return Err("Файл bin\\winws.exe не найден.".to_string());
        }
        self.install.ensure_user_lists();
        let gf = GameFilterState::load(&self.install.root);
        let argv = bat::get_argv(&self.install, strategy, &gf)?;
        let args = bat::build_command_line(&argv);
        self.enable_tcp_timestamps();

        let _ = service::stop("zapret");
        wait_service("zapret", "остановлена", Duration::from_secs(15));
        let _ = service::delete("zapret");
        wait_service_gone("zapret", Duration::from_secs(10));
        self.kill_winws();
        std::thread::sleep(Duration::from_millis(300));

        let bin_path = format!("\"{}\" {} ", self.install.winws_exe().to_string_lossy(), args);
        service::create(
            "zapret",
            "Zapret — ускорение соединения (Zapret Manager)",
            &bin_path,
            "Ускорение соединения для Discord и YouTube. Запускает winws.exe; управляется Zapret Manager.",
        )?;

        let _ = sys::reg_set_string(
            true,
            r"System\CurrentControlSet\Services\zapret",
            "zapret-discord-youtube",
            &strategy.name,
        );
        self.label_services();

        let running = wait_service("zapret", "работает", Duration::from_secs(20));
        let snap = service::query("zapret");
        if snap.installed && running {
            log::ok(format!("Служба zapret установлена: {}", strategy.name));
            Ok(format!(
                "Служба установлена со стратегией «{}». Статус: {}.",
                strategy.name, snap.status
            ))
        } else {
            Err(format!("Служба создана, но не запустилась (статус: {}).", snap.status))
        }
    }

    pub fn remove_service(&self) -> String {
        if !sys::is_admin() {
            return "Удаление службы требует прав администратора.".to_string();
        }
        let mut msg = String::new();
        let snap = service::query("zapret");
        if snap.installed {
            let _ = service::stop("zapret");
            wait_service("zapret", "остановлена", Duration::from_secs(15));
            match service::delete("zapret") {
                Ok(()) if wait_service_gone("zapret", Duration::from_secs(15)) => {
                    msg.push_str("Служба zapret удалена. ");
                    log::ok("Служба zapret удалена.");
                }
                _ => {
                    msg.push_str("Не удалось удалить службу zapret. ");
                    log::warn("Не удалось удалить службу zapret.");
                }
            }
        }
        self.kill_winws();
        for name in ["WinDivert", "WinDivert14"] {
            if service::exists(name) {
                let _ = service::stop(name);
                let _ = service::delete(name);
                msg.push_str(&format!("Драйвер {} остановлен и удалён. ", name));
            }
        }
        if msg.is_empty() {
            msg = "Служба не была установлена.".to_string();
        }
        msg.trim().to_string()
    }

    // ------------------------------------------------------------------
    // IPSet
    // ------------------------------------------------------------------

    pub fn get_ipset_mode(&self) -> String {
        let file = self.install.ipset_file();
        let Ok(text) = std::fs::read_to_string(&file) else {
            return "none".to_string();
        };
        if text.trim().is_empty() {
            return "any".to_string();
        }
        if text.contains("203.0.113.113/32") && text.len() < 200 {
            return "none".to_string();
        }
        "loaded".to_string()
    }

    pub fn set_ipset_mode(&self, mode: &str) -> String {
        if !matches!(mode, "loaded" | "none" | "any") {
            return format!("Неизвестный режим IPSet: {}", mode);
        }
        let file = self.install.ipset_file();
        let backup = self.install.ipset_backup();
        let current = self.get_ipset_mode();
        let exists = file.is_file();
        if current == mode && exists {
            return format!("Режим уже установлен: {}", ipset_mode_text(mode));
        }
        if current == "loaded" && exists {
            let _ = std::fs::copy(&file, &backup);
        }
        let result = match mode {
            "loaded" => {
                if !backup.is_file() {
                    return "Нет резервной копии полного списка. Сначала обновите IP-лист.".to_string();
                }
                std::fs::copy(&backup, &file).map(|_| ())
            }
            "none" => textfile::write_text_atomic(&file, "203.0.113.113/32\r\n"),
            _ => textfile::write_text_atomic(&file, ""),
        };
        match result {
            Ok(()) => {
                log::ok(format!("IPSet режим: {}", ipset_mode_text(mode)));
                format!("Готово: {}", ipset_mode_text(mode))
            }
            Err(e) => format!("Ошибка: {}", e),
        }
    }

    pub fn update_ipset(&self, mut progress: impl FnMut(u64, u64)) -> String {
        let url = "https://raw.githubusercontent.com/Flowseal/zapret-discord-youtube/refs/heads/main/.service/ipset-service.txt";
        let tmp = super::paths::temp_dir().join("ipset-all.txt");
        log::info("Скачиваю IP-лист…");
        if let Err(e) = super::http::download(url, &tmp, 60000, |read, total| progress(read, total)) {
            log::error(format!("Не удалось обновить IP-лист: {}", e));
            return format!("Ошибка обновления: {}", e);
        }
        let text = std::fs::read_to_string(&tmp).unwrap_or_default();
        if text.trim().is_empty() {
            return "Скачанный список пуст — обновление отменено.".to_string();
        }
        let lines = text.lines().filter(|l| !l.trim().is_empty()).count();
        if lines < 100 {
            return format!("Список подозрительно мал ({} строк) — обновление отменено.", lines);
        }
        let mode = self.get_ipset_mode();
        let file = self.install.ipset_file();
        let backup = self.install.ipset_backup();
        let write_result = if mode == "loaded" {
            textfile::write_text_atomic(&file, &text).and_then(|_| textfile::write_text_atomic(&backup, &text))
        } else {
            textfile::write_text_atomic(&backup, &text).and_then(|_| {
                if mode == "any" {
                    textfile::write_text_atomic(&file, "")
                } else {
                    textfile::write_text_atomic(&file, "203.0.113.113/32\r\n")
                }
            })
        };
        match write_result {
            Ok(()) => {
                log::ok(format!("IP-лист обновлён: {} строк.", lines));
                format!("IP-лист обновлён: {} строк.", lines)
            }
            Err(e) => format!("Ошибка обновления: {}", e),
        }
    }

    // ------------------------------------------------------------------
    // Фейк-пакеты
    // ------------------------------------------------------------------

    pub fn list_fakes(&self) -> Vec<FakeFile> {
        let mut list = Vec::new();
        if let Ok(entries) = std::fs::read_dir(self.install.bin_dir()) {
            for entry in entries.flatten() {
                let path = entry.path();
                if !path.is_file() {
                    continue;
                }
                let name = path.file_name().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
                if !name.to_lowercase().ends_with(".bin") || name.to_lowercase().starts_with("active_") {
                    continue;
                }
                list.push(FakeFile {
                    name: path.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default(),
                    sha256: sha256_file(&path),
                    full_path: path,
                });
            }
        }
        list.sort_by(|a, b| crate::util::natural_cmp(&a.name, &b.name));
        list
    }

    pub fn find_active_fake(&self, active_file_name: &str) -> String {
        let path = self.install.bin_dir().join(active_file_name);
        if !path.is_file() {
            return "(не найден)".to_string();
        }
        let hash = sha256_file(&path);
        for f in self.list_fakes() {
            if f.sha256.eq_ignore_ascii_case(&hash) {
                return f.name;
            }
        }
        "(пользовательский)".to_string()
    }

    pub fn replace_active_fake(&self, active_file_name: &str, source: &Path) -> String {
        let target = self.install.bin_dir().join(active_file_name);
        match std::fs::copy(source, &target) {
            Ok(_) => {
                let name = source
                    .file_stem()
                    .map(|s| s.to_string_lossy().to_string())
                    .unwrap_or_default();
                log::ok(format!("Активный фейк {} заменён на {}", active_file_name, name));
                format!("Активный фейк заменён на «{}». Перезапустите ускорение, чтобы применить.", name)
            }
            Err(e) => {
                log::error(format!("Не удалось заменить фейк: {}", e));
                format!("Ошибка: {}", e)
            }
        }
    }

    // ------------------------------------------------------------------
    // hosts
    // ------------------------------------------------------------------

    pub fn check_hosts(&self) -> HostsStatus {
        let mut st = HostsStatus::default();
        let url = "https://raw.githubusercontent.com/Flowseal/zapret-discord-youtube/refs/heads/main/.service/hosts";
        let tmp = super::paths::temp_dir().join("hosts-service.txt");
        match super::http::download(url, &tmp, 30000, |_, _| {}) {
            Ok(_) => {
                st.downloaded = true;
                st.temp_file = tmp.clone();
                let remote = read_hosts_entries(&tmp);
                st.remote_lines = remote.len();
                let current = std::fs::read_to_string(hosts_path()).unwrap_or_default();
                for line in &remote {
                    if !current.to_lowercase().contains(&line.to_lowercase()) {
                        st.missing_lines += 1;
                    }
                }
            }
            Err(e) => st.error = e,
        }
        st
    }

    pub fn apply_hosts(&self) -> String {
        if !sys::is_admin() {
            return "Изменение hosts требует прав администратора.".to_string();
        }
        let st = self.check_hosts();
        if !st.downloaded {
            return format!("Не удалось скачать список hosts: {}", st.error);
        }
        if st.missing_lines == 0 {
            return "Файл hosts уже содержит все записи из репозитория.".to_string();
        }
        let path = hosts_path();
        let mut content = std::fs::read_to_string(&path).unwrap_or_default();
        if let Some(start) = content.find(HOSTS_MARKER_START) {
            let cut = match content[start..].find(HOSTS_MARKER_END) {
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
        let remote = read_hosts_entries(&st.temp_file);
        let mut out = content.trim_end().to_string();
        out.push_str("\r\n\r\n");
        out.push_str(HOSTS_MARKER_START);
        out.push_str("\r\n");
        for line in &remote {
            out.push_str(line);
            out.push_str("\r\n");
        }
        out.push_str(HOSTS_MARKER_END);
        out.push_str("\r\n");

        let backup = path.with_extension("zapret-manager.bak");
        if !backup.exists() && path.exists() {
            let _ = std::fs::copy(&path, &backup);
        }
        match textfile::write_text_atomic(&path, &out) {
            Ok(()) => {
                log::ok(format!("Файл hosts обновлён: добавлено {} строк.", st.missing_lines));
                format!(
                    "Файл hosts обновлён (добавлено {} строк). Резервная копия: {}",
                    st.missing_lines,
                    backup.to_string_lossy()
                )
            }
            Err(e) => {
                log::error(format!("Не удалось обновить hosts: {}", e));
                format!("Ошибка: {}", e)
            }
        }
    }

    pub fn clear_hosts_block(&self) -> String {
        if !sys::is_admin() {
            return "Изменение hosts требует прав администратора.".to_string();
        }
        let path = hosts_path();
        let Ok(content) = std::fs::read_to_string(&path) else {
            return "Файл hosts не найден.".to_string();
        };
        let Some(start) = content.find(HOSTS_MARKER_START) else {
            return "Блок zapret-manager в hosts не найден.".to_string();
        };
        let Some(rel_end) = content[start..].find(HOSTS_MARKER_END) else {
            return "Маркер конца блока в hosts не найден — файл не изменён.".to_string();
        };
        let end = start + rel_end;
        let cut = match content[end..].find('\n') {
            Some(nl) => end + nl + 1,
            None => content.len(),
        };
        let result = format!("{}{}", &content[..start], &content[cut..]);
        match textfile::write_text_atomic(&path, &result) {
            Ok(()) => {
                log::ok("Блок zapret-manager удалён из hosts.");
                "Блок zapret-manager удалён из hosts.".to_string()
            }
            Err(e) => format!("Ошибка: {}", e),
        }
    }

    // ------------------------------------------------------------------
    // Кэш Discord
    // ------------------------------------------------------------------

    pub fn clear_discord_cache(&self) -> String {
        let mut report = String::new();
        let dirs = discord_cache_dirs();
        if dirs.is_empty() {
            return "Установки Discord не найдены.".to_string();
        }
        for proc_name in ["Discord", "DiscordPTB", "DiscordCanary", "DiscordDevelopment"] {
            for p in sys::list_processes() {
                if p.name.eq_ignore_ascii_case(proc_name) && sys::kill_process(p.pid) {
                    report.push_str(&format!("Закрыт {}. ", proc_name));
                }
            }
        }
        for dir in &dirs {
            for sub in ["Cache", "Code Cache", "GPUCache"] {
                let path = dir.join(sub);
                if path.is_dir() {
                    match std::fs::remove_dir_all(&path) {
                        Ok(()) => report.push_str(&format!("Очищено: {}. ", path.to_string_lossy())),
                        Err(e) => report.push_str(&format!(
                            "Не удалось удалить {} ({}). ",
                            path.to_string_lossy(),
                            e
                        )),
                    }
                }
            }
        }
        log::info(format!("Кэш Discord очищен: {}", report));
        report.trim().to_string()
    }
}

// ----------------------------------------------------------------------
// Вспомогательные функции
// ----------------------------------------------------------------------

pub const HOSTS_MARKER_START: &str = "# >>> zapret-manager: начало блока";
pub const HOSTS_MARKER_END: &str = "# <<< zapret-manager: конец блока";

pub fn hosts_path() -> PathBuf {
    let system_root = std::env::var("SystemRoot").unwrap_or_else(|_| r"C:\Windows".to_string());
    Path::new(&system_root).join("System32").join("drivers").join("etc").join("hosts")
}

fn read_hosts_entries(file: &Path) -> Vec<String> {
    textfile::read_lines(file)
        .into_iter()
        .map(|l| l.trim().to_string())
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .collect()
}

pub fn discord_cache_dirs() -> Vec<PathBuf> {
    let appdata = std::env::var("APPDATA").unwrap_or_default();
    let mut dirs = Vec::new();
    for name in ["discord", "discordptb", "discordcanary", "discorddevelopment"] {
        let dir = Path::new(&appdata).join(name);
        if dir.is_dir() {
            dirs.push(dir);
        }
    }
    dirs
}

pub fn ipset_mode_text(mode: &str) -> &'static str {
    match mode {
        "any" => "любые IP (фильтр по списку хостов)",
        "none" => "без фильтра по IP",
        _ => "загружен полный список",
    }
}

pub fn count_list_lines(file: &Path) -> u64 {
    match std::fs::read(file) {
        Ok(bytes) => {
            if bytes.is_empty() {
                return 0;
            }
            let mut n = bytes.iter().filter(|b| **b == b'\n').count() as u64;
            if !bytes.ends_with(b"\n") {
                n += 1;
            }
            n
        }
        Err(_) => 0,
    }
}

fn list_winws() -> Vec<(u32, String)> {
    sys::list_processes()
        .into_iter()
        .filter(|p| p.name.eq_ignore_ascii_case("winws.exe"))
        .map(|p| {
            let cmd = sys::process_command_line(p.pid).unwrap_or_default();
            (p.pid, cmd)
        })
        .collect()
}

pub fn any_winws_running() -> bool {
    sys::list_processes()
        .iter()
        .any(|p| p.name.eq_ignore_ascii_case("winws.exe"))
}

fn exited_code() -> Option<i32> {
    let mut slot = winws_slot().lock().ok()?;
    let handle = slot.as_mut()?;
    match handle.child.try_wait() {
        Ok(Some(status)) => Some(status.code().unwrap_or(-1)),
        _ => None,
    }
}

/// Запуск winws.exe с готовой командной строкой (без переэкранирования).
fn spawn_winws(exe: &Path, cmdline: &str, cwd: &Path) -> std::io::Result<Child> {
    use std::os::windows::process::CommandExt;
    use std::process::Stdio;
    let mut cmd = std::process::Command::new(exe);
    cmd.raw_arg(cmdline)
        .current_dir(cwd)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .creation_flags(process::CREATE_NO_WINDOW);
    cmd.spawn()
}

pub fn wait_service(name: &str, expected: &str, timeout: Duration) -> bool {
    let deadline = Instant::now() + timeout;
    loop {
        if service::status(name).eq_ignore_ascii_case(expected) {
            return true;
        }
        if Instant::now() >= deadline {
            return false;
        }
        std::thread::sleep(Duration::from_millis(300));
    }
}

pub fn wait_service_gone(name: &str, timeout: Duration) -> bool {
    let deadline = Instant::now() + timeout;
    loop {
        if !service::exists(name) {
            return true;
        }
        if Instant::now() >= deadline {
            return false;
        }
        std::thread::sleep(Duration::from_millis(300));
    }
}

// ----------------------------------------------------------------------
// Уборка
// ----------------------------------------------------------------------

/// Папки вида "<zapret>_backup_<тег>_<дата>" рядом с установкой, от новых к старым.
pub fn backups(install_root: &str) -> Vec<PathBuf> {
    let mut list = Vec::new();
    if install_root.is_empty() {
        return list;
    }
    let path = Path::new(install_root);
    let Some(parent) = path.parent() else {
        return list;
    };
    let Some(name) = path.file_name() else {
        return list;
    };
    let prefix = format!("{}_backup_", name.to_string_lossy());
    if let Ok(entries) = std::fs::read_dir(parent) {
        for entry in entries.flatten() {
            let p = entry.path();
            if p.is_dir()
                && p.file_name()
                    .map(|n| n.to_string_lossy().starts_with(&prefix))
                    .unwrap_or(false)
            {
                list.push(p);
            }
        }
    }
    list.sort_by_key(|p| {
        std::cmp::Reverse(
            std::fs::metadata(p)
                .and_then(|m| m.modified())
                .ok()
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|d| d.as_secs())
                .unwrap_or(0),
        )
    });
    list
}

pub fn dir_size(path: &Path) -> u64 {
    let mut total = 0u64;
    if let Ok(entries) = std::fs::read_dir(path) {
        for entry in entries.flatten() {
            let p = entry.path();
            if p.is_dir() {
                total += dir_size(&p);
            } else if let Ok(meta) = p.metadata() {
                total += meta.len();
            }
        }
    }
    total
}

pub fn prune_backups(install_root: &str, keep: usize) -> (u64, usize) {
    let mut freed = 0u64;
    let mut removed = 0usize;
    for path in backups(install_root).into_iter().skip(keep) {
        let size = dir_size(&path);
        if std::fs::remove_dir_all(&path).is_ok() {
            freed += size;
            removed += 1;
            log::info(format!("Удалена старая копия: {}", path.file_name().unwrap_or_default().to_string_lossy()));
        }
    }
    (freed, removed)
}

/// Чистит временные файлы обновлений старше указанного числа дней.
pub fn clean_temp(days: u32) -> u64 {
    let mut freed = 0u64;
    let limit = std::time::SystemTime::now()
        .checked_sub(Duration::from_secs(days.max(1) as u64 * 86400))
        .unwrap_or(std::time::SystemTime::UNIX_EPOCH);
    let temp = std::env::temp_dir();
    for name in ["ZapretManagerUpdate", "ZapretManager"] {
        let dir = temp.join(name);
        freed += clean_temp_dir(&dir, limit);
    }
    freed
}

fn clean_temp_dir(dir: &Path, limit: std::time::SystemTime) -> u64 {
    let mut freed = 0u64;
    let Ok(entries) = std::fs::read_dir(dir) else {
        return 0;
    };
    for entry in entries.flatten() {
        let p = entry.path();
        if p.is_dir() {
            if is_reparse_point(&p) {
                continue;
            }
            freed += clean_temp_dir(&p, limit);
        } else if let Ok(meta) = p.metadata() {
            if meta.modified().map(|m| m < limit).unwrap_or(false) {
                let size = meta.len();
                if std::fs::remove_file(&p).is_ok() {
                    freed += size;
                }
            }
        }
    }
    freed
}

pub fn is_reparse_point(path: &Path) -> bool {
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x400;
        return std::fs::symlink_metadata(path)
            .map(|m| m.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0)
            .unwrap_or(true);
    }
    #[allow(unreachable_code)]
    false
}

pub fn cleanup_report() -> String {
    let install = ZapretInstall::detect("");
    let copies = backups(&install.root);
    let total: u64 = copies.iter().map(|p| dir_size(p)).sum();
    let (freed, removed) = prune_backups(&install.root, 1);
    let temp = clean_temp(1);
    format!(
        "Уборка: копий {}, занято {}. Удалено копий: {}, освобождено {} + {} временных файлов.",
        copies.len(),
        format_size(total),
        removed,
        format_size(freed),
        format_size(temp)
    )
}

/// Запуск внешней команды и возврат вывода (для отладки и CLI).
pub fn run_capture(exe: &str, args: &[&str], timeout: Duration) -> ProcResult {
    process::run(exe, args, timeout)
}
