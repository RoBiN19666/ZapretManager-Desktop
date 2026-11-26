//! Главное окно: сайдбар, страницы, строка состояния, уведомления, фоновые задачи.

use std::collections::HashMap;
use std::sync::atomic::AtomicBool;
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use eframe::egui::{self, Color32, FontId, RichText, Rounding, Sense, Stroke, Vec2};

use crate::engine::appprobe::AppEntry;
use crate::engine::autotune::{ApplyReport, AutoReport};
use crate::engine::config::Ini;
use crate::engine::diagnostics::CheckResult;
use crate::engine::doctor::SiteProbe;
use crate::engine::guard;
use crate::engine::install::{StrategyDef, ZapretInstall};
use crate::engine::lists::{ListChanges, ListNote};
use crate::engine::log::{self, LogLevel, LogLine};
use crate::engine::ops::{FakeFile, HostsStatus, RuntimeState, StartResult, ZapretOps};
use crate::engine::paths;
use crate::engine::service;
use crate::engine::tgproxy::{self, TgState};
use crate::engine::tester::{LoadedReport, OfficialReport, StrategyTestRow, TargetGroup};
use crate::engine::update::{Release, SelfUpdateInfo, UpdateCheckResult, UpdateOptions};
use crate::engine::sys;

use super::theme::{self, Palette};
use super::tray::{TrayEvent, TrayHandle, TrayState};
use super::widgets as w;

// ---------------------------------------------------------------------------
// Страницы
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Page {
    Overview,
    Strategies,
    Test,
    Sites,
    TgProxy,
    Settings,
    Lists,
    Update,
    Diagnostics,
    Tools,
    About,
}

impl Page {
    pub const ALL: [Page; 11] = [
        Page::Overview,
        Page::Strategies,
        Page::Test,
        Page::Sites,
        Page::TgProxy,
        Page::Settings,
        Page::Lists,
        Page::Update,
        Page::Diagnostics,
        Page::Tools,
        Page::About,
    ];

    pub fn key(&self) -> &'static str {
        match self {
            Page::Overview => "overview",
            Page::Strategies => "strategies",
            Page::Test => "test",
            Page::Sites => "sites",
            Page::TgProxy => "tgproxy",
            Page::Settings => "settings",
            Page::Lists => "lists",
            Page::Update => "update",
            Page::Diagnostics => "diagnostics",
            Page::Tools => "tools",
            Page::About => "about",
        }
    }

    pub fn from_key(key: &str) -> Option<Page> {
        Page::ALL.into_iter().find(|p| p.key().eq_ignore_ascii_case(key))
    }

    pub fn title(&self) -> &'static str {
        match self {
            Page::Overview => "Обзор",
            Page::Strategies => "Стратегии",
            Page::Test => "Тест стратегий",
            Page::Sites => "Проверка сайта",
            Page::TgProxy => "TG-прокси",
            Page::Settings => "Настройки",
            Page::Lists => "Списки",
            Page::Update => "Обновление",
            Page::Diagnostics => "Диагностика",
            Page::Tools => "Инструменты",
            Page::About => "О программе",
        }
    }

    pub fn subtitle(&self) -> &'static str {
        match self {
            Page::Overview => "Состояние защиты, быстрые действия и журнал",
            Page::Strategies => "Выбор и запуск стратегий ускорения (general*.bat)",
            Page::Test => "Автоматический прогон стратегий с наглядными результатами",
            Page::Sites => "Диагностика блокировки и добавление адресов в списки",
            Page::TgProxy => "Ускорение Telegram через WebSocket (TG WS Proxy)",
            Page::Settings => "Игровой фильтр, IP-список, фейк-пакеты и прочие параметры",
            Page::Lists => "Домены и IP, к которым применяется ускорение",
            Page::Update => "Обновление zapret с сохранением ваших списков",
            Page::Diagnostics => "Поиск конфликтов и типичных проблем с ускорением",
            Page::Tools => "Служба, hosts, тест стратегий и файлы",
            Page::About => "Версия, ссылки и поддержка проекта",
        }
    }

    /// Иконка из Segoe MDL2 Assets (Windows 10/11).
    pub fn glyph(&self) -> &'static str {
        match self {
            Page::Overview => "\u{E80F}",
            Page::Strategies => "\u{E945}",
            Page::Test => "\u{E768}",
            Page::Sites => "\u{E774}",
            Page::TgProxy => "\u{E8BD}",
            Page::Settings => "\u{E713}",
            Page::Lists => "\u{E8FD}",
            Page::Update => "\u{E896}",
            Page::Diagnostics => "\u{E9D2}",
            Page::Tools => "\u{E90F}",
            Page::About => "\u{E946}",
        }
    }

    /// Запасные символы, если Segoe MDL2 Assets недоступен.
    pub fn glyph_fallback(&self) -> &'static str {
        match self {
            Page::Overview => "◉",
            Page::Strategies => "⚡",
            Page::Test => "▤",
            Page::Sites => "⌖",
            Page::TgProxy => "✈",
            Page::Settings => "⚙",
            Page::Lists => "☰",
            Page::Update => "↧",
            Page::Diagnostics => "✚",
            Page::Tools => "▣",
            Page::About => "ⓘ",
        }
    }
}

// ---------------------------------------------------------------------------
// Сообщения фоновых задач
// ---------------------------------------------------------------------------

pub enum Msg {
    BusyDone,
    Notify(String, LogLevel),
    State(RuntimeState),
    StartDone(StartResult),
    Text(String),
    Probe(Box<SiteProbe>, bool),
    ProbeList(String),
    Compare(Box<UpdateCheckResult>),
    SelfCheck(Box<SelfUpdateInfo>),
    Apps(Vec<AppEntry>),
    WatchDone(Vec<String>, Vec<String>),
    TestProgress(usize, usize, String),
    TestRow(Box<StrategyTestRow>),
    TestDone(Vec<StrategyTestRow>),
    AutoProgress(usize, usize, String),
    AutoDone(Result<AutoReport, String>),
    ApplyDone(Result<ApplyReport, String>),
    DiagStep(String),
    DiagDone(Vec<CheckResult>),
    TgLatest(Result<(String, String), String>),
    TgInstallDone(Result<String, String>),
    TgPrepared(Vec<String>),
    TgPrepareDone(Result<String, String>),
    TgState(TgState),
    Progress(&'static str, u32, String),
    UpdateDone(Result<String, String>),
    BackupDone(Result<(bool, String), String>),
    SelfApplyDone(Result<String, String>),
    Releases(Result<Vec<Release>, String>),
    Installed(Box<ZapretInstall>),
    WizardDone(Result<String, String>),
    Report(Option<LoadedReport>),
    Official(Option<OfficialReport>),
    Fakes(Vec<FakeFile>),
    HostsChecked(HostsStatus),
    GuardTick,
}

// ---------------------------------------------------------------------------
// Состояния страниц
// ---------------------------------------------------------------------------

#[derive(Default)]
pub struct OverviewState {
    pub checking: bool,
}

#[derive(Default)]
pub struct StrategiesState {
    pub selected: usize,
    pub preview: String,
    pub caption: String,
    pub warning: String,
    pub search: String,
    pub only_favorites: bool,
}

pub struct TestState {
    pub running: bool,
    pub cancel: Arc<AtomicBool>,
    pub rows: Vec<StrategyTestRow>,
    pub groups: Vec<TargetGroup>,
    pub enabled: std::collections::HashSet<String>,
    pub timeout_ms: u32,
    pub deep: bool,
    pub done: usize,
    pub total: usize,
    pub summary: String,
    pub status: String,
    pub selected: Option<usize>,
    pub reports: Vec<std::path::PathBuf>,
    pub report_index: usize,
    pub official: Option<OfficialReport>,
    pub default_enabled: bool,
    pub auto_running: bool,
    pub auto_done: usize,
    pub auto_total: usize,
    pub auto_stage: String,
    pub auto_candidates: usize,
    pub auto_best: Option<StrategyTestRow>,
    pub auto_best_file: Option<std::path::PathBuf>,
    pub auto_cancel: Arc<AtomicBool>,
    pub auto_base: String,
    pub auto_limit: usize,
    pub auto_apply_running: bool,
    pub auto_apply_stage: String,
}

impl Default for TestState {
    fn default() -> Self {
        TestState {
            running: false,
            cancel: Arc::new(AtomicBool::new(false)),
            rows: Vec::new(),
            groups: Vec::new(),
            enabled: std::collections::HashSet::new(),
            timeout_ms: 4000,
            deep: false,
            done: 0,
            total: 0,
            summary: "Прогон ещё не выполнялся.".to_string(),
            status: String::new(),
            selected: None,
            reports: Vec::new(),
            report_index: 0,
            official: None,
            default_enabled: false,
            auto_running: false,
            auto_done: 0,
            auto_total: 0,
            auto_stage: String::new(),
            auto_candidates: 0,
            auto_best: None,
            auto_best_file: None,
            auto_cancel: Arc::new(AtomicBool::new(false)),
            auto_base: String::new(),
            auto_limit: 120,
            auto_apply_running: false,
            auto_apply_stage: String::new(),
        }
    }
}

pub struct SitesState {
    pub input: String,
    pub verdict: String,
    pub verdict_kind: crate::engine::doctor::Verdict,
    pub steps: String,
    pub editor: String,
    pub working: bool,
    pub status: String,
    pub apps: Vec<AppEntry>,
    pub app_index: usize,
    pub apps_loaded: bool,
    pub watching: bool,
    pub guard_enabled: bool,
    pub guard_auto: bool,
    pub guard_status: String,
    pub probe: Option<SiteProbe>,
}

impl Default for SitesState {
    fn default() -> Self {
        SitesState {
            input: "https://".to_string(),
            verdict: "Введите адрес сайта и нажмите «Проверить» — менеджер по шагам определит, где рвётся соединение.".to_string(),
            verdict_kind: crate::engine::doctor::Verdict::Down,
            steps: String::new(),
            editor: String::new(),
            working: false,
            status: String::new(),
            apps: Vec::new(),
            app_index: 0,
            apps_loaded: false,
            watching: false,
            guard_enabled: false,
            guard_auto: false,
            guard_status: "Сторож выключен.".to_string(),
            probe: None,
        }
    }
}

#[derive(Default)]
pub struct TgPageState {
    pub state: TgState,
    pub latest: Option<String>,
    pub check_error: String,
    pub stage: String,
    pub progress: u32,
    pub working: bool,
    pub quiet_busy: bool,
    pub quiet_status: String,
    pub quiet_progress: u32,
    pub quiet_prepared: bool,
}

pub struct SettingsState {
    pub mode: crate::engine::gamefilter::GameFilterMode,
    pub tcp: String,
    pub udp: String,
    pub theme_index: usize,
    pub scale: i32,
    pub fakes: Vec<FakeFile>,
    pub active_discord: String,
    pub active_game: String,
    pub fake_discord_idx: usize,
    pub fake_game_idx: usize,
    pub ipset_info: String,
    pub autostart_enabled: bool,
    pub autostart_checked: Option<Instant>,
}

impl Default for SettingsState {
    fn default() -> Self {
        SettingsState {
            mode: crate::engine::gamefilter::GameFilterMode::Disabled,
            tcp: "1024-65535".to_string(),
            udp: "1024-65535".to_string(),
            theme_index: 0,
            scale: 100,
            fakes: Vec::new(),
            active_discord: String::new(),
            active_game: String::new(),
            fake_discord_idx: 0,
            fake_game_idx: 0,
            ipset_info: String::new(),
            autostart_enabled: false,
            autostart_checked: None,
        }
    }
}

#[derive(Clone)]
pub struct ListFileInfo {
    pub name: String,
    pub label: &'static str,
    pub official: bool,
    pub tracked: bool,
    pub hint: &'static str,
    pub template: &'static str,
}

pub fn list_files() -> Vec<ListFileInfo> {
    let official = |name: &str, label, hint: &'static str| ListFileInfo {
        name: name.to_string(),
        label,
        official: true,
        tracked: crate::engine::lists::is_official(name),
        hint,
        template: "",
    };
    vec![
        ListFileInfo {
            name: "list-general-user.txt".to_string(),
            label: "Мои домены",
            official: false,
            tracked: false,
            hint: "Домены, которые нужно ускорять (добавляются к официальному списку). Файл может быть пустым.",
            template: "# Мои домены (файл может быть пустым)\r\n",
        },
        ListFileInfo {
            name: "list-exclude-user.txt".to_string(),
            label: "Исключения",
            official: false,
            tracked: false,
            hint: "Домены, которые ускорять НЕ нужно (перекрывают оба списка). Файл может быть пустым.",
            template: "# Исключения (файл может быть пустым)\r\n",
        },
        ListFileInfo {
            name: "ipset-exclude-user.txt".to_string(),
            label: "Исключения IP",
            official: false,
            tracked: false,
            hint: "IP-подсети, которые ускорять не нужно. Файл может быть пустым.",
            template: "# Исключения IP (файл может быть пустым)\r\n",
        },
        official(
            "list-general.txt",
            "Официальный список",
            "Основной список доменов из репозитория (обновляется вместе с zapret)",
        ),
        official("list-google.txt", "Google / YouTube", "Список доменов Google и YouTube"),
        official(
            "list-exclude.txt",
            "Официальные исключения",
            "Исключения из репозитория (банки, игры, обновления)",
        ),
        official(
            "ipset-exclude.txt",
            "Официальные IP-исключения",
            "Локальные и служебные подсети из репозитория",
        ),
        ListFileInfo {
            name: "ipset-all.txt".to_string(),
            label: "IP-список (рабочий)",
            official: true,
            tracked: false,
            hint: "Текущий IP-лист: полный список, пустой файл или заглушка",
            template: "",
        },
    ]
}

pub struct ListsState {
    pub files: Vec<ListFileInfo>,
    pub selected: usize,
    pub editor: String,
    pub dirty: bool,
    pub loading: bool,
    pub changes: Option<ListChanges>,
    pub info: String,
    pub notes: Vec<ListNote>,
    pub status: String,
    pub paste_open: bool,
    pub paste_text: String,
}

impl Default for ListsState {
    fn default() -> Self {
        ListsState {
            files: list_files(),
            selected: 0,
            editor: String::new(),
            dirty: false,
            loading: false,
            changes: None,
            info: String::new(),
            notes: Vec::new(),
            status: String::new(),
            paste_open: false,
            paste_text: String::new(),
        }
    }
}

pub struct UpdatePageState {
    pub options: UpdateOptions,
    pub releases: Vec<Release>,
    pub releases_idx: usize,
    pub manual_tag: String,
    pub hint: String,
    pub notes: String,
    pub progress: u32,
    pub stage: String,
    pub running: bool,
}

impl Default for UpdatePageState {
    fn default() -> Self {
        UpdatePageState {
            options: UpdateOptions::default(),
            releases: Vec::new(),
            releases_idx: 0,
            manual_tag: String::new(),
            hint: String::new(),
            notes: String::new(),
            progress: 0,
            stage: String::new(),
            running: false,
        }
    }
}

#[derive(Default)]
pub struct DiagState {
    pub results: Vec<CheckResult>,
    pub running: bool,
    pub step: String,
    pub summary: String,
}

#[derive(Default)]
pub struct ToolsState {
    pub hosts_status: Option<HostsStatus>,
    pub hosts_message: String,
    pub setup_open: bool,
}

pub struct WizardState {
    pub zapret_dir: String,
    pub tg_dir: String,
    pub install_zapret: bool,
    pub install_tg: bool,
    pub stage: String,
    pub progress: u32,
    pub running: bool,
    pub error: String,
    pub done: bool,
}

pub struct Pages {
    pub overview: OverviewState,
    pub strategies: StrategiesState,
    pub test: TestState,
    pub sites: SitesState,
    pub tg: TgPageState,
    pub settings: SettingsState,
    pub lists: ListsState,
    pub update: UpdatePageState,
    pub diagnostics: DiagState,
    pub tools: ToolsState,
    pub wizard: Option<WizardState>,
}

impl Pages {
    fn new() -> Pages {
        Pages {
            overview: OverviewState::default(),
            strategies: StrategiesState::default(),
            test: TestState::default(),
            sites: SitesState::default(),
            tg: TgPageState::default(),
            settings: SettingsState::default(),
            lists: ListsState::default(),
            update: UpdatePageState::default(),
            diagnostics: DiagState::default(),
            tools: ToolsState::default(),
            wizard: None,
        }
    }
}

// ---------------------------------------------------------------------------
// Подтверждение действий
// ---------------------------------------------------------------------------

pub struct Confirm {
    pub title: String,
    pub body: String,
    pub ok_text: String,
    pub danger: bool,
    pub action: Option<Box<dyn FnOnce(&mut App)>>,
}

// ---------------------------------------------------------------------------
// Приложение
// ---------------------------------------------------------------------------

pub struct App {
    pub install: ZapretInstall,
    pub ini: Ini,
    pub ops: ZapretOps,
    pub state: RuntimeState,
    pub page: Page,
    pub p: Palette,
    pub dark: bool,
    pub ctx_dark: bool,
    pub toast: Option<(String, LogLevel, Instant)>,
    pub confirm: Option<Confirm>,
    pub tx: Sender<Msg>,
    rx: Receiver<Msg>,
    pub busy: usize,
    pub status_hint: String,
    pub log_lines: Vec<LogLine>,
    pub log_seq: u64,
    pub update_state: Option<UpdateCheckResult>,
    pub self_update: Option<SelfUpdateInfo>,
    update_notice_open: bool,
    update_notice_shown: bool,
    pub pages: Pages,
    tray: Option<TrayHandle>,
    tray_rx: Option<Receiver<TrayEvent>>,
    tray_state: Arc<Mutex<TrayState>>,
    pub exit_requested: bool,
    last_refresh: Instant,
    last_guard: Instant,
    pub(crate) last_tg_refresh: Instant,
    last_normal_size: (f32, f32),
    last_maximized: bool,
    window_hwnd: Option<isize>,
    titlebar_dark: Option<bool>,
    icon_set: bool,
    pub(crate) auto_heal_armed: bool,
    auto_heal_attempts: u32,
    auto_heal_last: Instant,
    pub(crate) about_logo: Option<egui::TextureHandle>,
    pub(crate) winws_mem: Option<u64>,
    pub(crate) tg_mem: Option<u64>,
    pub(crate) resource_checked: Instant,    pub shot: Option<String>,
    shot_frames: u32,
    pub progress: HashMap<&'static str, (u32, String)>,
    pub start_hidden: bool,
    hidden_done: bool,
    window_visible: bool,
    window_visibility_sent: Option<bool>,
    pub autostart_mode: bool,
    pub bootstrap_done: bool,
    pub update_check_started: bool,
    pub limit_edit: bool,
    pub scale: f32,
    pub theme_name: String,
    pub clipboard: Option<String>,

}

impl App {
    pub fn new(
        install: ZapretInstall,
        ini: Ini,
        shot: Option<String>,
        shot_page: Option<String>,
        start_hidden: bool,
        autostart_mode: bool,
        limited: bool,
    ) -> App {
        let (tx, rx) = mpsc::channel();
        let dark = match ini.get("Theme", "auto").as_str() {
            "dark" => true,
            "light" => false,
            _ => true,
        };
        let initial_page = shot_page
            .as_deref()
            .and_then(Page::from_key)
            .unwrap_or(Page::Overview);
        let p = theme::palette(dark);
        let ops = ZapretOps::new(install.clone());
        let state = ops.get_state();
        let theme_name = ini.get("Theme", "auto");
        let scale = ini.get_int("UiScale", 100) as f32 / 100.0;
        let mut app = App {
            install,
            ini,
            ops,
            state,
            page: initial_page,
            p,
            dark,
            ctx_dark: dark,
            toast: None,
            confirm: None,
            tx,
            rx,
            busy: 0,
            status_hint: String::new(),
            log_lines: Vec::new(),
            log_seq: 0,
            update_state: None,
            self_update: None,
            update_notice_open: false,
            update_notice_shown: false,
            pages: Pages::new(),
            tray: None,
            tray_rx: None,
            tray_state: Arc::new(Mutex::new(TrayState::default())),
            exit_requested: false,
            last_refresh: Instant::now(),
            last_guard: Instant::now(),
            last_tg_refresh: Instant::now(),
            last_normal_size: (1280.0, 880.0),
            last_maximized: true,
            window_hwnd: None,
            titlebar_dark: None,
            icon_set: false,
            auto_heal_armed: false,
            auto_heal_attempts: 0,
            auto_heal_last: Instant::now(),
            about_logo: None,
            winws_mem: None,
            tg_mem: None,
            resource_checked: Instant::now(),
            shot,
            shot_frames: 0,
            progress: HashMap::new(),
            start_hidden,
            hidden_done: !start_hidden,
            window_visible: !start_hidden,
            window_visibility_sent: if start_hidden { None } else { Some(true) },
            autostart_mode,
            bootstrap_done: false,
            update_check_started: false,
            limit_edit: limited,
            scale,
            theme_name,
            clipboard: None,

        };
        app.log_seq = log::last_seq();
        // начальные значения страниц
        app.pages.settings.theme_index = match app.theme_name.as_str() {
            "dark" => 1,
            "light" => 2,
            _ => 0,
        };
        app.pages.settings.scale = (scale * 100.0).round() as i32;
        app.pages.sites.guard_enabled = app.ini.get_bool("SiteGuard", false);
        app.pages.sites.guard_auto = app.ini.get_bool("SiteGuardAutoAdd", false);
        if app.pages.sites.guard_enabled {
            // Если сторож уже был включён в прошлой сессии, журнал DNS мог
            // остаться выключенным. Включаем его сами, в фоне.
            std::thread::spawn(|| {
                if !guard::log_enabled() {
                    let message = guard::enable_log();
                    log::info(format!("Фоновый сторож: {}", message));
                }
            });
        }
        app.pages.update.notes.clear();
        app.init_tray();
        app.load_lists();
        app.load_test_groups();
        app.refresh_state(false);
        app.sync_settings();
        app.refresh_tg_state();
        app.start_update_check();
        // Первый запуск: показываем мастер настройки, как в C#-версии. В тихом
        // автозапуске и в режиме снимков окно не открываем.
        if app.shot.is_none()
            && !start_hidden
            && !autostart_mode
            && app.ini.get("SetupDone", "").trim().is_empty()
        {
            app.open_wizard();
        }
        if shot_page.as_deref() == Some("wizard") {
            app.open_wizard();
        }
        app
    }

    pub(crate) fn on_navigate(&mut self, page: Page) {
        match page {
            Page::Settings => self.sync_settings(),
            Page::TgProxy => self.refresh_tg_state(),
            Page::Test => self.load_test_groups(),
            Page::Lists => {
                if !self.pages.lists.dirty {
                    self.load_lists();
                }
            }
            Page::Sites => {
                if self.pages.sites.apps.is_empty() {
                    self.spawn_bg(|tx| {
                        let apps = crate::engine::appprobe::list_apps();
                        let _ = tx.send(Msg::Apps(apps));
                    });
                }
            }
            Page::Update => {
                if self.update_state.is_none() {
                    self.check_updates();
                }
            }
            _ => {}
        }
    }

    fn init_tray(&mut self) {
        let icon = crate::ui::assets::tray_icon_bytes();
        let (rx, handle) = super::tray::spawn(icon, self.tray_state.clone());
        self.tray_rx = Some(rx);
        self.tray = Some(handle);
    }

    // -- сервисные методы -------------------------------------------------

    pub fn notify(&mut self, message: impl Into<String>, level: LogLevel) {
        let message = message.into();
        log::write(level, &message);
        self.toast = Some((message, level, Instant::now()));
    }

    pub fn require_admin(&mut self, action: &str) -> bool {
        if sys::is_admin() {
            return true;
        }
        self.notify(
            format!(
                "{}: нужны права администратора. Перезапустите программу от имени администратора.",
                action
            ),
            LogLevel::Warn,
        );
        false
    }

    pub fn spawn_bg(&mut self, job: impl FnOnce(Sender<Msg>) + Send + 'static) {
        self.busy += 1;
        let tx = self.tx.clone();
        std::thread::spawn(move || {
            job(tx.clone());
            let _ = tx.send(Msg::BusyDone);
        });
    }

    pub fn refresh_state(&mut self, rescan: bool) {
        if rescan && self.install.is_valid() {
            self.install.rescan();
        }
        self.state = self.ops.get_state();
        self.update_tray_state();
    }

    fn update_tray_state(&mut self) {
        if let Ok(mut st) = self.tray_state.lock() {
            st.zapret_on = self.state.winws_running;
            st.tg_on = self.pages.tg.state.running;
        }
    }

    pub fn set_root(&mut self, root: &str) {
        self.install = ZapretInstall::new(root);
        self.ops = ZapretOps::new(self.install.clone());
        self.ini.set("Root", &self.install.root);
        let _ = self.ini.save();
        self.refresh_state(false);
        log::info(format!("Папка zapret: {}", self.install.root));
    }

    /// Единый выбор уже установленной папки zapret.
    ///
    /// Выбирается папка, а не `service.bat`: пользователю не нужно знать,
    /// какой файл просить у проводника. Валидность проверяется по наличию
    /// `service.bat` и `bin\winws.exe`.
    pub fn pick_zapret_folder(&mut self) {
        let Some(dir) = rfd::FileDialog::new()
            .set_title("Выберите папку zapret (где лежит service.bat)")
            .pick_folder()
        else {
            return;
        };
        if let Some(dir) = ZapretInstall::resolve_valid_dir(&dir) {
            self.set_root(&dir.to_string_lossy());
            self.notify(
                format!("Папка zapret: {}", self.install.root),
                LogLevel::Ok,
            );
        } else {
            self.notify(
                "В выбранной папке нет service.bat и bin\\winws.exe. Выберите папку zapret или её родителя.",
                LogLevel::Error,
            );
        }
    }

    pub fn pick_strategy(&self) -> Option<StrategyDef> {
        let last = self.ini.get("LastStrategy", "");
        self.ops.pick_strategy(&last).cloned()
    }

    pub fn confirm(
        &mut self,
        title: &str,
        body: &str,
        ok_text: &str,
        danger: bool,
        action: impl FnOnce(&mut App) + 'static,
    ) {
        self.update_notice_open = false;
        self.confirm = Some(Confirm {
            title: title.to_string(),
            body: body.to_string(),
            ok_text: ok_text.to_string(),
            danger,
            action: Some(Box::new(action)),
        });
    }

    pub fn start_strategy(&mut self, strategy: StrategyDef) {
        if !self.require_admin("Запуск") {
            return;
        }
        self.ini.set("LastStrategy", &strategy.name);
        let _ = self.ini.save();
        self.auto_heal_armed = true;
        let ops = ZapretOps::new(self.install.clone());
        let name = strategy.name.clone();
        self.spawn_bg(move |tx| {
            let result = ops.start(&strategy);
            if result.ok {
                let _ = tx.send(Msg::Notify(result.message.clone(), LogLevel::Ok));
            } else {
                let _ = tx.send(Msg::Notify(result.message.clone(), LogLevel::Error));
            }
            let _ = tx.send(Msg::StartDone(result));
            let _ = tx.send(Msg::Text(name));
        });
    }

    pub fn stop_bypass(&mut self) {
        if !self.require_admin("Остановка") {
            return;
        }
        self.auto_heal_armed = false;
        self.auto_heal_attempts = 0;
        let ops = ZapretOps::new(self.install.clone());
        self.spawn_bg(move |tx| {
            let message = ops.stop();
            let _ = tx.send(Msg::Notify(message, LogLevel::Ok));
            let _ = tx.send(Msg::Text(String::new()));
        });
    }

    pub fn restart_bypass(&mut self, strategy: StrategyDef) {
        if !self.require_admin("Перезапуск") {
            return;
        }
        self.ini.set("LastStrategy", &strategy.name);
        let _ = self.ini.save();
        self.auto_heal_armed = true;
        let ops = ZapretOps::new(self.install.clone());
        self.spawn_bg(move |tx| {
            let message = ops.restart(&strategy);
            let _ = tx.send(Msg::Notify(message, LogLevel::Ok));
            let _ = tx.send(Msg::Text(String::new()));
        });
    }

    pub fn install_service(&mut self, strategy: StrategyDef) {
        if !self.require_admin("Установка службы") {
            return;
        }
        self.ini.set("LastStrategy", &strategy.name);
        let _ = self.ini.save();
        self.auto_heal_armed = true;
        let ops = ZapretOps::new(self.install.clone());
        self.spawn_bg(move |tx| {
            match ops.install_service(&strategy) {
                Ok(message) => {
                    let _ = tx.send(Msg::Notify(message, LogLevel::Ok));
                }
                Err(message) => {
                    let _ = tx.send(Msg::Notify(message, LogLevel::Error));
                }
            }
            let _ = tx.send(Msg::Text(String::new()));
        });
    }

    pub fn remove_service(&mut self) {
        if !self.require_admin("Удаление службы") {
            return;
        }
        let ops = ZapretOps::new(self.install.clone());
        self.spawn_bg(move |tx| {
            let message = ops.remove_service();
            let _ = tx.send(Msg::Notify(message, LogLevel::Ok));
            let _ = tx.send(Msg::Text(String::new()));
        });
    }

    pub fn simulate_state(&mut self) {
        self.refresh_state(true);
    }

    fn start_update_check(&mut self) {
        if self.update_check_started {
            return;
        }
        self.update_check_started = true;
        if !self.ini.get_bool("CheckUpdatesOnStartup", true) {
            return;
        }
        let ini = self.ini.clone();
        let _ = ini.save();
        let local = self.install.version.clone();
        let tg_installed = tgproxy::get_state(&ini).installed;
        self.spawn_bg(move |tx| {
            let _ = tx.send(Msg::SelfCheck(Box::new(crate::engine::update::self_check(true))));
            let _ = tx.send(Msg::Compare(Box::new(
                crate::engine::update::check_update(&local, true),
            )));
            if tg_installed {
                let _ = tx.send(Msg::TgLatest(tgproxy::check_latest()));
            }
        });
    }

    fn load_test_groups(&mut self) {
        let groups = crate::engine::tester::load_targets(&self.install.utils_dir().join("targets.txt"));
        if !self.pages.test.default_enabled {
            for g in &groups {
                let lower = g.name.to_lowercase();
                if lower.contains("discord")
                    || lower.contains("youtube")
                    || lower.contains("google")
                    || lower.contains("cloudflare")
                {
                    self.pages.test.enabled.insert(g.name.clone());
                }
            }
            if self.pages.test.enabled.is_empty() {
                for g in &groups {
                    self.pages.test.enabled.insert(g.name.clone());
                }
            }
            self.pages.test.default_enabled = true;
        }
        self.pages.test.groups = groups;
        self.pages.test.reports = crate::engine::tester::list_reports(&self.install);
        if let Some(report) = crate::engine::tester::load_tsv(&crate::engine::tester::report_path()) {
            if self.pages.test.rows.is_empty() {
                self.pages.test.summary = if report.header.is_empty() {
                    "Показаны результаты предыдущего прогона.".to_string()
                } else {
                    format!("Показаны результаты предыдущего прогона ({}).", report.header)
                };
                self.pages.test.rows = report.rows;
            }
        }
        self.refresh_official_report();
    }

    pub fn refresh_official_report(&mut self) {
        let idx = self.pages.test.report_index.min(self.pages.test.reports.len().saturating_sub(1));
        self.pages.test.official = self
            .pages
            .test
            .reports
            .get(idx)
            .map(|p| crate::engine::tester::parse_official_report(p));
    }

    pub fn load_lists(&mut self) {
        let install = self.install.clone();
        let st = &mut self.pages.lists;
        if st.files.is_empty() {
            st.files = list_files();
        }
        let selected = st.selected.min(st.files.len().saturating_sub(1));
        let file = st.files[selected].clone();
        let path = if file.name == "ipset-all.txt" {
            install.ipset_file()
        } else {
            install.lists_dir().join(&file.name)
        };
        st.editor = crate::engine::textfile::read(&path);
        st.loading = true;
        st.dirty = false;
        st.notes = crate::engine::lists::analyze(&st.editor);
        st.changes = if file.tracked && install.is_valid() {
            Some(ListChanges::load(&path, &paths::data_dir()))
        } else {
            None
        };
        st.info = lists_info_of(st);
        st.loading = false;
        st.status.clear();
    }

    pub fn lists_info(&self) -> String {
        lists_info_of(&self.pages.lists)
    }

    // -- обработка сообщений ---------------------------------------------

    fn handle_messages(&mut self) {
        let mut state_changed = false;
        loop {
            let msg = match self.rx.try_recv() {
                Ok(m) => m,
                Err(_) => break,
            };
            match msg {
                Msg::BusyDone => {
                    self.busy = self.busy.saturating_sub(1);
                }
                Msg::Notify(text, level) => {
                    self.notify(text, level);
                }
                Msg::State(st) => {
                    self.state = st;
                    self.update_tray_state();
                }
                Msg::StartDone(result) => {
                    self.status_hint = if result.ok {
                        format!("Ускорение запущено (PID {})", result.pid)
                    } else {
                        String::new()
                    };
                    state_changed = true;
                }
                Msg::Text(text) => {
                    self.status_hint = text;
                    state_changed = true;
                }
                Msg::Probe(probe, with_related) => {
                    self.pages.sites.working = false;
                    let probe = *probe;
                    self.pages.sites.verdict = probe.verdict_text.clone();
                    self.pages.sites.verdict_kind = probe.verdict;
                    self.pages.sites.steps = site_steps(&probe);
                    if with_related {
                        self.pages.sites.editor = probe.related.join("\r\n");
                    } else if !probe.host.is_empty() {
                        self.pages.sites.editor = probe.host.clone();
                    }
                    self.pages.sites.status = format!(
                        "Проверено. Время: TLS {} мс, TCP {} мс.",
                        if probe.tls_ok { probe.tls_ms } else { 0 },
                        probe.tcp_ms
                    );
                    self.pages.sites.probe = Some(probe);
                }
                Msg::ProbeList(text) => {
                    self.pages.sites.editor = text;
                    self.pages.sites.working = false;
                }
                Msg::Compare(res) => {
                    self.update_state = Some(*res);
                }
                Msg::SelfCheck(info) => {
                    self.self_update = Some(*info);
                }
                Msg::Apps(apps) => {
                    self.pages.sites.apps = apps;
                    self.pages.sites.app_index = 0;
                    self.pages.sites.apps_loaded = true;
                }
                Msg::WatchDone(hosts, ips) => {
                    self.pages.sites.watching = false;
                    self.pages.sites.editor = hosts.join("\r\n");
                    self.pages.sites.status = if ips.is_empty() {
                        format!("Наблюдение завершено: доменов {}.", hosts.len())
                    } else {
                        format!(
                            "Наблюдение завершено: доменов {}, адресов {}. Проверьте список ниже.",
                            hosts.len(),
                            ips.len()
                        )
                    };
                }
                Msg::TestProgress(done, total, name) => {
                    self.pages.test.done = done;
                    self.pages.test.total = total;
                    self.pages.test.status = format!("Стратегия {} из {}: {}", done, total, name);
                }
                Msg::TestRow(row) => {
                    self.pages.test.rows.push(*row);
                    self.pages.test.summary = test_summary(&self.pages.test);
                }
                Msg::TestDone(rows) => {
                    self.pages.test.rows = rows;
                    self.pages.test.running = false;
                    self.pages.test.status = "Готово.".to_string();
                    self.pages.test.summary = test_summary(&self.pages.test);
                    // Сразу выделяем лучшую стратегию, чтобы кнопка
                    // «Запустить выбранную» была понятной и доступной.
                    let best = self
                        .pages
                        .test
                        .rows
                        .iter()
                        .filter(|r| r.error.is_empty() && r.total() > 0)
                        .map(|r| r.passed())
                        .max()
                        .unwrap_or(0);
                    self.pages.test.selected = if best == 0 {
                        None
                    } else {
                        self.pages.test.rows.iter().position(|r| {
                            r.error.is_empty() && r.total() > 0 && r.passed() == best
                        })
                    };
                }
                Msg::AutoProgress(done, total, name) => {
                    self.pages.test.auto_done = done;
                    self.pages.test.auto_total = total;
                    self.pages.test.auto_stage = format!("{} из {}: {}", done, total, name);
                }
                Msg::AutoDone(result) => {
                    self.pages.test.auto_running = false;
                    match result {
                        Ok(report) => {
                            self.pages.test.auto_candidates = report.candidates;
                            self.pages.test.auto_best = report.best.clone();
                            self.pages.test.auto_best_file = report.best_file.clone();
                            self.pages.test.rows = report.rows;
                            self.pages.test.running = false;
                            let best = self
                                .pages
                                .test
                                .rows
                                .iter()
                                .filter(|r| r.error.is_empty() && r.total() > 0)
                                .max_by_key(|r| r.score())
                                .cloned();
                            self.pages.test.selected = best.as_ref().and_then(|b| {
                                self.pages
                                    .test
                                    .rows
                                    .iter()
                                    .position(|r| r.strategy_name == b.strategy_name)
                            });
                            self.pages.test.summary = test_summary(&self.pages.test);
                            self.pages.test.status = "Автоподбор завершён.".to_string();
                            self.install.rescan();
                            match &report.best {
                                Some(best) => {
                                    self.pages.test.auto_stage = format!(
                                        "Лучший вариант: {} — {} из {}",
                                        best.strategy_name,
                                        best.passed(),
                                        best.total()
                                    );
                                    let message = match &report.best_file {
                                        Some(path) => format!(
                                            "Автоподбор: лучший вариант «{}» сохранён как {}",
                                            best.strategy_name,
                                            path.to_string_lossy()
                                        ),
                                        None => format!(
                                            "Автоподбор: лучший вариант «{}» ({} из {}), но сохранить файл не удалось.",
                                            best.strategy_name,
                                            best.passed(),
                                            best.total()
                                        ),
                                    };
                                    self.notify(message, LogLevel::Ok);
                                }
                                None => {
                                    self.pages.test.auto_stage =
                                        "Автоподбор не нашёл рабочего варианта.".to_string();
                                    self.notify(
                                        "Автоподбор не нашёл варианта, который проходит проверки.",
                                        LogLevel::Warn,
                                    );
                                }
                            }
                        }
                        Err(error) => {
                            self.pages.test.auto_stage =
                                format!("Ошибка автоподбора: {}", error);
                            self.notify(
                                format!("Автоподбор: {}", error),
                                LogLevel::Error,
                            );
                        }
                    }
                }
                Msg::ApplyDone(result) => {
                    self.pages.test.auto_apply_running = false;
                    match result {
                        Ok(report) => {
                            self.pages.test.auto_apply_stage = report.message.clone();
                            self.pages.test.auto_stage = report.message.clone();
                            self.refresh_state(true);
                            if report.applied {
                                self.notify(report.message, LogLevel::Ok);
                            } else {
                                self.notify(report.message, LogLevel::Warn);
                            }
                        }
                        Err(error) => {
                            self.pages.test.auto_apply_stage =
                                format!("Ошибка применения: {}", error);
                            self.notify(
                                format!("Применение auto-стратегии: {}", error),
                                LogLevel::Error,
                            );
                        }
                    }
                }
                Msg::DiagStep(step) => {
                    self.pages.diagnostics.step = step;
                }
                Msg::DiagDone(results) => {
                    self.pages.diagnostics.running = false;
                    self.pages.diagnostics.results = results;
                    self.pages.diagnostics.summary = diag_summary(&self.pages.diagnostics.results);
                    self.pages.diagnostics.step = "Готово.".to_string();
                }
                Msg::TgLatest(result) => {
                    self.pages.tg.working = false;
                    match result {
                        Ok((tag, _url)) => {
                            self.pages.tg.latest = Some(tag);
                            self.pages.tg.check_error.clear();
                        }
                        Err(e) => {
                            self.pages.tg.check_error = e;
                            self.pages.tg.latest = None;
                        }
                    }
                    self.pages.tg.stage = String::new();
                }
                Msg::TgInstallDone(result) => {
                    self.pages.tg.working = false;
                    match result {
                        Ok(message) => {
                            self.notify(message, LogLevel::Ok);
                            self.pages.tg.state = tgproxy::get_state(&self.ini);
                        }
                        Err(e) => {
                            self.notify(format!("Ошибка установки TG WS Proxy: {}", e), LogLevel::Error);
                        }
                    }
                }
                Msg::TgPrepared(messages) => {
                    self.pages.tg.working = false;
                    for m in messages {
                        self.notify(m, LogLevel::Info);
                    }
                }
                Msg::TgState(st) => {
                    self.pages.tg.quiet_prepared = st.quiet_prepared;
                    self.pages.tg.state = st;
                }
                Msg::TgPrepareDone(result) => {                    self.pages.tg.quiet_busy = false;
                    self.pages.tg.quiet_progress = 0;
                    self.pages.tg.quiet_prepared = crate::engine::tgsource::is_prepared();
                    match result {
                        Ok(message) => {
                            self.pages.tg.quiet_status = message.clone();
                            self.notify(message, LogLevel::Ok);
                            if crate::engine::tgproxy::run_mode(&self.ini) == "quiet" {
                                self.pages.tg.quiet_status = format!(
                                    "{}\nВключён тихий режим — прокси будет запущен без трея.",
                                    self.pages.tg.quiet_status
                                );
                                self.restart_tg_proxy_for_mode();
                            }
                        }
                        Err(error) => {
                            self.pages.tg.quiet_status =
                                format!("Ошибка подготовки: {}", error);
                            self.notify(
                                format!("Тихий режим TG WS Proxy: {}", error),
                                LogLevel::Error,
                            );
                            if !self.pages.tg.quiet_prepared
                                && crate::engine::tgproxy::run_mode(&self.ini) == "quiet"
                            {
                                crate::engine::tgproxy::set_run_mode(&mut self.ini, false);
                            }
                        }
                    }
                }
                Msg::Progress(tag, percent, stage) => {
                    self.progress.insert(tag, (percent, stage.clone()));
                    if tag == "update" {
                        self.pages.update.progress = percent;
                        self.pages.update.stage = stage;
                    } else if tag == "self" {
                        self.pages.update.progress = percent;
                        self.pages.update.stage = stage;
                    } else if tag == "tg" {
                        self.pages.tg.progress = percent;
                        self.pages.tg.stage = stage;
                    } else if tag == "tg-quiet" {
                        self.pages.tg.quiet_progress = percent;
                        self.pages.tg.quiet_status = stage;
                    } else if tag == "wizard" {
                        if let Some(wizard) = self.pages.wizard.as_mut() {
                            wizard.progress = percent;
                            wizard.stage = stage;
                        }
                    }
                }
                Msg::Installed(install) => {
                    self.install = *install;
                    self.ops = ZapretOps::new(self.install.clone());
                    self.ini.set("Root", &self.install.root);
                    let _ = self.ini.save();
                    self.refresh_state(true);
                    self.notify(
                        format!("zapret установлен: {}", self.install.root),
                        LogLevel::Ok,
                    );
                }
                Msg::WizardDone(result) => {
                    self.handle_wizard_msg(result);
                }
                Msg::UpdateDone(result) => {
                    self.pages.update.running = false;
                    match result {
                        Ok(summary) => {
                            self.notify(summary.replace('\n', " "), LogLevel::Ok);
                            self.pages.update.hint = summary;
                            self.install.rescan();
                            self.refresh_state(true);
                            crate::engine::update::invalidate_cache();
                        }
                        Err(e) => {
                            self.notify(format!("Ошибка обновления: {}", e), LogLevel::Error);
                            self.pages.update.hint = format!("Ошибка: {}", e);
                        }
                    }
                }
                Msg::BackupDone(result) => match result {
                    Ok((restored, message)) => {
                        self.notify(message, LogLevel::Ok);
                        if restored {
                            let root = self.install.root.clone();
                            self.ini = Ini::load(&paths::config_path());
                            self.ini.set("Root", &root);
                            let _ = self.ini.save();
                            self.install = ZapretInstall::new(&root);
                            self.ops = ZapretOps::new(self.install.clone());
                            self.refresh_state(true);
                        }
                    }
                    Err(e) => self.notify(format!("Резервная копия: {}", e), LogLevel::Error),
                },
                Msg::SelfApplyDone(result) => match result {
                    Ok(message) => {
                        self.notify(message, LogLevel::Ok);
                        self.exit_requested = true;
                    }
                    Err(e) => self.notify(e, LogLevel::Error),
                },
                Msg::Releases(result) => {
                    match result {
                        Ok(list) => {
                            self.pages.update.releases = list;
                            self.pages.update.hint =
                                format!("Доступно версий: {}.", self.pages.update.releases.len());
                        }
                        Err(e) => {
                            self.pages.update.hint = format!("Не удалось загрузить список: {}", e);
                        }
                    }
                }
                Msg::Report(report) => {
                    if let Some(report) = report {
                        self.pages.test.rows = report.rows;
                        self.pages.test.summary = if report.header.is_empty() {
                            "Показаны результаты предыдущего прогона.".to_string()
                        } else {
                            format!("Показаны результаты предыдущего прогона ({}).", report.header)
                        };
                    }
                }
                Msg::Official(report) => {
                    self.pages.test.official = report;
                }
                Msg::Fakes(fakes) => {
                    self.pages.settings.fakes = fakes;
                }
                Msg::HostsChecked(status) => {
                    let msg = if status.downloaded {
                        if status.missing_lines == 0 {
                            format!("hosts актуален: все {} записей на месте.", status.remote_lines)
                        } else {
                            format!(
                                "В hosts не хватает {} из {} записей — нажмите «Применить список».",
                                status.missing_lines, status.remote_lines
                            )
                        }
                    } else {
                        format!("Не удалось получить список: {}", status.error)
                    };
                    self.pages.tools.hosts_message = msg;
                    self.pages.tools.hosts_status = Some(status);
                }
                Msg::GuardTick => {}
            }
        }
        if state_changed {
            self.refresh_state(false);
            self.update_status_hint();
        }
        self.poll_update_notice();
    }

    fn update_items(&self) -> Vec<(String, String, String)> {
        let mut items = Vec::new();
        if let Some(info) = &self.self_update {
            if info.ok && info.available {
                items.push((
                    "Zapret Manager".to_string(),
                    info.local.clone(),
                    info.remote.clone(),
                ));
            }
        }
        if let Some(st) = &self.update_state {
            if st.ok && st.update_available {
                items.push((
                    "zapret".to_string(),
                    st.local_version.clone(),
                    st.remote_version.clone(),
                ));
            }
        }
        if self.pages.tg.state.installed {
            if let Some(latest) = &self.pages.tg.latest {
                let current = &self.pages.tg.state.version;
                if crate::util::version_cmp(latest, current) == std::cmp::Ordering::Greater {
                    items.push((
                        "TG WS Proxy".to_string(),
                        current.clone(),
                        latest.clone(),
                    ));
                }
            }
        }
        items
    }

    fn update_notice_key(items: &[(String, String, String)]) -> String {
        format!(
            "{}|{}",
            crate::APP_VERSION,
            items
                .iter()
                .map(|(n, c, v)| format!("{}:{}>{}", n, c, v))
                .collect::<Vec<_>>()
                .join(";")
        )
    }

    fn poll_update_notice(&mut self) {
        if self.update_notice_shown || self.start_hidden || self.confirm.is_some() {
            return;
        }
        if self.pages.wizard.is_some() {
            return;
        }
        let items = self.update_items();
        if items.is_empty() {
            return;
        }
        let key = Self::update_notice_key(&items);
        if self.ini.get("UpdateNoticeDismissed", "") == key {
            self.update_notice_shown = true;
            return;
        }
        self.update_notice_shown = true;
        self.update_notice_open = true;
    }

    fn show_update_notice(&mut self, ctx: &egui::Context) {
        if !self.update_notice_open {
            return;
        }
        let p = self.p;
        let items = self.update_items();
        if items.is_empty() {
            self.update_notice_open = false;
            return;
        }
        let painter = ctx.layer_painter(egui::LayerId::new(
            egui::Order::Middle,
            egui::Id::new("update_notice_overlay"),
        ));
        painter.rect_filled(
            ctx.screen_rect(),
            Rounding::ZERO,
            Color32::from_black_alpha(140),
        );
        let mut go_updates = false;
        let mut close = false;
        egui::Window::new("Доступны обновления")
            .collapsible(false)
            .resizable(false)
            .order(egui::Order::Foreground)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .default_width(540.0)
            .frame(
                egui::Frame::window(&ctx.style())
                    .fill(p.surface)
                    .stroke(Stroke::new(1.0, p.border_strong)),
            )
            .show(ctx, |ui| {
                ui.label(
                    RichText::new("Найдены новые версии:")
                        .size(13.0)
                        .color(p.text_dim),
                );
                ui.add_space(8.0);
                for (name, current, new) in &items {
                    egui::Frame::none()
                        .fill(p.editor)
                        .stroke(Stroke::new(1.0, p.border))
                        .rounding(Rounding::same(8))
                        .inner_margin(egui::Margin::symmetric(12, 9))
                        .show(ui, |ui| {
                            ui.horizontal(|ui| {
                                ui.label(
                                    RichText::new(name)
                                        .size(13.5)
                                        .strong()
                                        .color(p.text),
                                );
                                ui.label(
                                    RichText::new(format!("{}  →  {}", current, new))
                                        .size(12.5)
                                        .color(p.success),
                                );
                            });
                        });
                    ui.add_space(6.0);
                }
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    if w::primary(ui, &p, "Открыть обновления").clicked() {
                        go_updates = true;
                        close = true;
                    }
                    if w::secondary(ui, &p, "Позже").clicked() {
                        close = true;
                    }
                });
            });
        if go_updates {
            self.page = Page::Update;
            self.on_navigate(Page::Update);
        }
        if close {
            let key = Self::update_notice_key(&items);
            self.ini.set("UpdateNoticeDismissed", &key);
            let _ = self.ini.save();
            self.update_notice_open = false;
        }
    }

    fn update_status_hint(&mut self) {
        if self.busy > 0 {
            self.status_hint = "Выполняется операция…".to_string();
        }
    }

    fn auto_heal_tick(&mut self) {
        if !self.ini.get_bool("AutoHeal", false) {
            self.auto_heal_armed = self.state.winws_running;
            return;
        }
        if self.state.winws_running {
            self.auto_heal_armed = true;
            self.auto_heal_attempts = 0;
            return;
        }
        if !self.auto_heal_armed || !self.install.is_valid() {
            return;
        }
        if self.auto_heal_last.elapsed() < Duration::from_secs(15) {
            return;
        }
        if self.auto_heal_attempts >= 3 {
            self.auto_heal_armed = false;
            self.notify(
                "Автолечение: три попытки поднять ускорение не удались. Проверьте журнал и драйвер WinDivert.",
                LogLevel::Warn,
            );
            return;
        }
        self.auto_heal_last = Instant::now();
        self.auto_heal_attempts += 1;
        if self.state.service_installed {
            self.notify("Автолечение: поднимаю службу zapret…", LogLevel::Warn);
            let tx = self.tx.clone();
            std::thread::spawn(move || {
                let message = match service::start("zapret") {
                    Ok(()) => "Автолечение: служба zapret снова запущена.".to_string(),
                    Err(e) => format!("Автолечение: не удалось запустить службу: {}", e),
                };
                let _ = tx.send(Msg::Notify(message, LogLevel::Warn));
            });
        } else if let Some(strategy) = self.pick_strategy() {
            self.notify(
                format!("Автолечение: перезапускаю «{}»…", strategy.name),
                LogLevel::Warn,
            );
            self.start_strategy(strategy);
        } else {
            self.auto_heal_armed = false;
            self.notify(
                "Автолечение: нет сохранённой стратегии для перезапуска.",
                LogLevel::Warn,
            );
        }
    }

    fn poll_tray(&mut self) {
        let mut events = Vec::new();
        if let Some(rx) = &self.tray_rx {
            while let Ok(event) = rx.try_recv() {
                events.push(event);
            }
        }
        for event in events {
            match event {
                TrayEvent::Show => {
                    self.hidden_done = true;
                    self.window_visible = true;
                }
                TrayEvent::ToggleZapret => {
                    if self.state.winws_running {
                        self.stop_bypass();
                    } else if let Some(s) = self.pick_strategy() {
                        self.start_strategy(s);
                    } else {
                        self.notify("Стратегии не найдены.", LogLevel::Error);
                    }
                }
                TrayEvent::ToggleTg => {
                    if self.pages.tg.state.running {
                        let dir = self.pages.tg.state.dir.clone();
                        self.spawn_bg(move |tx| {
                            let message = tgproxy::stop(&dir);
                            let _ = tx.send(Msg::Notify(message, LogLevel::Info));
                        });
                    } else {
                        let ini = self.ini.clone();
                        self.spawn_bg(move |tx| {
                            let (message, ok) = tgproxy::start(&ini);
                            let _ = tx.send(Msg::Notify(
                                message,
                                if ok { LogLevel::Ok } else { LogLevel::Error },
                            ));
                        });
                    }
                }
                TrayEvent::CheckUpdates => {
                    self.page = Page::Update;
                    self.hidden_done = true;
                    self.window_visible = true;
                    self.check_updates();
                }
                TrayEvent::Exit => {
                    self.exit_requested = true;
                }
            }
        }
    }

    pub fn check_updates(&mut self) {
        let local = self.install.version.clone();
        self.pages.update.running = true;
        self.pages.update.stage = "Проверка обновлений…".to_string();
        self.spawn_bg(move |tx| {
            let result = crate::engine::update::check_update(&local, true);
            let _ = tx.send(Msg::Compare(Box::new(result)));
            let _ = tx.send(Msg::SelfCheck(Box::new(crate::engine::update::self_check(true))));
            let _ = tx.send(Msg::Progress("update", 100, "Готово".to_string()));
        });
    }

    fn poll_guard(&mut self) {
        if !self.pages.sites.guard_enabled || self.busy > 0 || !self.install.is_valid() {
            return;
        }
        if self.last_guard.elapsed() < Duration::from_secs(10) {
            return;
        }
        self.last_guard = Instant::now();
        let install = self.install.clone();
        let auto_add = self.pages.sites.guard_auto;
        let tx_outer = self.tx.clone();
        std::thread::spawn(move || {
            let (events, _err, _scanned) = guard::new_events(None, false);
            let hosts: Vec<String> = events.into_iter().map(|e| e.host).collect();
            let _ = tx_outer.send(Msg::GuardTick);
            if hosts.is_empty() {
                return;
            }
            let found = guard::process_hosts(&install, &hosts, auto_add, |message| {
                log::warn(format!("Сторож сайтов: {}", message));
            });
            if !found.is_empty() {
                guard::trim();
                let _ = tx_outer.send(Msg::Notify(
                    format!("Сторож сайтов: добавлено доменов {}", found.len()),
                    LogLevel::Warn,
                ));
                let _ = tx_outer.send(Msg::Text(String::new()));
            }
        });
    }

    fn handle_confirm(&mut self, ctx: &egui::Context) {
        let Some(mut confirm) = self.confirm.take() else {
            return;
        };
        let mut close = false;
        let mut run = false;
        let p = self.p;
        let screen = ctx.screen_rect();
        egui::Window::new(&confirm.title)
            .collapsible(false)
            .resizable(false)
            .order(egui::Order::Foreground)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .default_width(520.0)
            .frame(
                egui::Frame::window(&ctx.style())
                    .fill(p.surface)
                    .stroke(Stroke::new(1.0, p.border_strong)),
            )
            .show(ctx, |ui| {
                ui.label(RichText::new(&confirm.body).size(13.0).color(p.text_dim));
                ui.add_space(14.0);
                ui.horizontal(|ui| {
                    if ui
                        .add(
                            egui::Button::new(
                                RichText::new("Отмена").color(p.text),
                            )
                            .fill(p.surface_alt)
                            .min_size(Vec2::new(110.0, 32.0)),
                        )
                        .clicked()
                    {
                        close = true;
                    }
                    ui.add_space(6.0);
                    if ui
                        .add(
                            egui::Button::new(
                                RichText::new(&confirm.ok_text)
                                    .color(if confirm.danger { Color32::WHITE } else { p.on_accent }),
                            )
                            .fill(if confirm.danger { p.danger } else { p.accent })
                            .min_size(Vec2::new(170.0, 32.0)),
                        )
                        .clicked()
                    {
                        run = true;
                        close = true;
                    }
                });
            });
        let _ = screen;
        if run {
            if let Some(action) = confirm.action.take() {
                action(self);
            }
        } else if close {
            // просто закрываем
        } else {
            self.confirm = Some(confirm);
        }
    }

    fn show_toast(&mut self, ctx: &egui::Context) {
        let Some((text, level, at)) = self.toast.clone() else {
            return;
        };
        if at.elapsed() > Duration::from_secs(6) {
            self.toast = None;
            return;
        }
        let p = self.p;
        let color = match level {
            LogLevel::Ok => p.success,
            LogLevel::Warn => p.warn,
            LogLevel::Error => p.danger,
            LogLevel::Info => p.accent,
        };
        let screen = ctx.screen_rect();
        let width = 420.0_f32.min(screen.width() - 40.0);
        egui::Area::new("toast".into())
            .fixed_pos(egui::pos2(screen.right() - width - 20.0, screen.bottom() - 90.0))
            .interactable(false)
            .show(ctx, |ui| {
                egui::Frame::none()
                    .fill(Color32::from_rgb(30, 25, 20))
                    .stroke(Stroke::new(1.0, p.border_strong))
                    .rounding(Rounding::same(10))
                    .inner_margin(egui::Margin::same(12))
                    .show(ui, |ui| {
                        ui.set_width(width);
                        ui.horizontal(|ui| {
                            let (rect, _) = ui.allocate_exact_size(Vec2::new(3.0, 34.0), Sense::hover());
                            ui.painter().rect_filled(rect, Rounding::same(2), color);
                            ui.label(RichText::new(text).size(12.5).color(Color32::from_rgb(244, 238, 231)));
                        });
                    });
            });
        ctx.request_repaint_after(Duration::from_millis(400));
    }

    fn sidebar(&mut self, ctx: &egui::Context) {
        let p = self.p;
        egui::SidePanel::left("sidebar")
            .exact_width(236.0)
            .resizable(false)
            .frame(egui::Frame::none().fill(p.sidebar).inner_margin(egui::Margin::same(0)))
            .show(ctx, |ui| {
                ui.add_space(18.0);
                ui.horizontal(|ui| {
                    ui.add_space(18.0);
                    let (rect, _) = ui.allocate_exact_size(Vec2::new(34.0, 34.0), Sense::hover());
                    ui.painter().rect_filled(rect, Rounding::same(9), p.accent);
                    ui.painter().text(
                        rect.center(),
                        egui::Align2::CENTER_CENTER,
                        "Z",
                        FontId::new(19.0, egui::FontFamily::Name("bold".into())),
                        p.on_accent,
                    );
                    ui.vertical(|ui| {
                        ui.label(
                            RichText::new("Zapret Manager")
                                .size(15.0)
                                .strong()
                                .color(p.text),
                        );
                        ui.label(
                            RichText::new(format!("v{}", crate::APP_VERSION))
                                .size(11.0)
                                .color(p.text_muted),
                        );
                    });
                });
                ui.add_space(22.0);
                ui.horizontal(|ui| {
                    ui.add_space(24.0);
                    ui.label(
                        RichText::new("УПРАВЛЕНИЕ")
                            .size(10.0)
                            .color(p.text_muted)
                            .strong(),
                    );
                });
                ui.add_space(8.0);
                let mut go_to = None;
                for page in Page::ALL {
                    let selected = self.page == page;
                    let (rect, resp) = ui.allocate_exact_size(
                        Vec2::new(ui.available_width() - 20.0, 40.0),
                        Sense::click(),
                    );
                    let rect = rect.translate(Vec2::new(10.0, 0.0));
                    if selected {
                        ui.painter().rect_filled(rect, Rounding::same(8), p.selection);
                        ui.painter().rect_filled(
                            egui::Rect::from_min_size(
                                rect.left_top(),
                                Vec2::new(3.0, rect.height()),
                            ),
                            Rounding::same(2),
                            p.accent,
                        );
                    } else if resp.hovered() {
                        ui.painter().rect_filled(rect, Rounding::same(8), p.hover);
                    }
                    let (glyph, family) = if crate::ui::theme::has_icon_font() {
                        (page.glyph(), egui::FontFamily::Name("icons".into()))
                    } else {
                        (page.glyph_fallback(), egui::FontFamily::Proportional)
                    };
                    ui.painter().text(
                        egui::pos2(rect.left() + 21.0, rect.center().y),
                        egui::Align2::CENTER_CENTER,
                        glyph,
                        FontId::new(17.0, family),
                        if selected { p.accent } else { p.text_muted },
                    );
                    ui.painter().text(
                        egui::pos2(rect.left() + 44.0, rect.center().y),
                        egui::Align2::LEFT_CENTER,
                        page.title(),
                        FontId::new(13.5, egui::FontFamily::Proportional),
                        if selected { p.text } else { p.text_dim },
                    );
                    if resp.clicked() {
                        go_to = Some(page);
                    }
                }
                if let Some(page) = go_to {
                    self.page = page;
                    self.on_navigate(page);
                }
                ui.add_space(12.0);
                ui.separator();
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    ui.add_space(18.0);
                    let admin = sys::is_admin();
                    w::status_dot(ui, if admin { p.success } else { p.warn }, true);
                    ui.label(
                        RichText::new(if admin {
                            "Права администратора"
                        } else {
                            "Без прав администратора"
                        })
                        .size(11.0)
                        .color(if admin { p.text_muted } else { p.warn }),
                    );
                });
            });
    }

    fn status_bar(&mut self, ctx: &egui::Context) {
        let p = self.p;
        egui::TopBottomPanel::bottom("status")
            .exact_height(30.0)
            .frame(
                egui::Frame::none()
                    .fill(p.sidebar)
                    .stroke(Stroke::new(1.0, p.border))
                    .inner_margin(egui::Margin::symmetric(16, 5)),
            )
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    w::status_dot(
                        ui,
                        if self.state.winws_running { p.success } else { p.text_muted },
                        self.state.winws_running,
                    );
                    let accel_text = if self.state.winws_running {
                        let name = if self.state.matched_strategy.is_empty() {
                            "пользовательская"
                        } else {
                            &self.state.matched_strategy
                        };
                        format!("Ускорение: работает — {}", name)
                    } else {
                        "Ускорение: не запущено".to_string()
                    };
                    ui.label(
                        RichText::new(accel_text)
                            .size(11.5)
                            .color(if self.state.winws_running { p.text } else { p.text_muted }),
                    );
                    ui.add_space(14.0);
                    w::status_dot(
                        ui,
                        if self.state.service_status == "работает" {
                            p.success
                        } else if self.state.service_installed {
                            p.warn
                        } else {
                            p.text_muted
                        },
                        false,
                    );
                    ui.label(
                        RichText::new(format!(
                            "Служба: {}",
                            if self.state.service_installed {
                                self.state.service_status.clone()
                            } else {
                                "не установлена".to_string()
                            }
                        ))
                        .size(11.5)
                        .color(p.text_muted),
                    );
                    ui.add_space(14.0);
                    ui.label(
                        RichText::new(format!(
                            "IPSet: {}",
                            match self.state.ipset_mode.as_str() {
                                "loaded" => "полный список",
                                "any" => "любые IP",
                                _ => "без фильтра",
                            }
                        ))
                        .size(11.5)
                        .color(p.text_muted),
                    );
                    if !self.status_hint.is_empty() {
                        ui.add_space(14.0);
                        ui.label(
                            RichText::new(&self.status_hint)
                                .size(11.0)
                                .color(p.text_dim),
                        );
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(
                            RichText::new(format!(
                                "zapret {} • {}",
                                if self.install.version.is_empty() {
                                    "—"
                                } else {
                                    &self.install.version
                                },
                                if sys::is_admin() {
                                    "администратор"
                                } else {
                                    "без прав администратора"
                                }
                            ))
                            .size(11.0)
                            .color(if sys::is_admin() { p.text_muted } else { p.warn }),
                        );
                    });
                });
            });
    }

    fn header(&mut self, ui: &mut egui::Ui) {
        let p = self.p;
        ui.add_space(14.0);
        ui.horizontal(|ui| {
            ui.add_space(4.0);
            let (glyph, family) = if theme::has_icon_font() {
                (self.page.glyph(), egui::FontFamily::Name("icons".into()))
            } else {
                (self.page.glyph_fallback(), egui::FontFamily::Proportional)
            };
            ui.label(
                RichText::new(glyph)
                    .font(FontId::new(22.0, family))
                    .color(p.accent),
            );
            ui.add_space(2.0);
            ui.vertical(|ui| {
                ui.label(
                    RichText::new(self.page.title())
                        .font(FontId::new(20.0, egui::FontFamily::Name("bold".into())))
                        .color(p.text),
                );
                ui.label(RichText::new(self.page.subtitle()).size(12.5).color(p.text_dim));
            });
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if self.busy > 0 {
                    ui.spinner();
                    ui.label(RichText::new("выполняется…").size(11.5).color(p.text_muted));
                }
            });
        });
        ui.add_space(6.0);
        ui.separator();
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // События трея обрабатываем до управления видимостью: если окно было
        // скрыто, пункт «Открыть» уже показал его напрямую, и цикл кадров снова
        // работает. Иначе visibility-код спрятал бы окно обратно.
        if super::tray::take_exit_request() {
            self.exit_requested = true;
        }
        self.poll_tray();
        self.auto_heal_tick();

        // Пользовательский масштаб интерфейса (90/100/110 %).
        if (ctx.zoom_factor() - self.scale).abs() > 0.01 {
            ctx.set_zoom_factor(self.scale);
        }        // тема и масштаб
        if self.ctx_dark != self.dark {
            theme::apply(ctx, self.dark);
            self.ctx_dark = self.dark;
        }
        self.p = theme::palette(self.dark);


        // снимок окна
        if let Some(path) = self.shot.clone() {
            self.shot_frames += 1;

            if self.shot_frames == 12 {

                ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(Default::default()));
            }
            if self.shot_frames > 12 {
                let mut saved = false;
                let events = ctx.input(|i| i.events.clone());
                for event in events {
                    if let egui::Event::Screenshot { image, .. } = event {

                        saved = crate::ui::assets::save_png(&path, &image);
                        break;
                    }
                }
                if saved || self.shot_frames > 40 {
                    if !saved {
                        log::error("Не удалось сохранить снимок окна");
                    }
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    return;
                }
                ctx.request_repaint();
            }
        }

        // закрытие окна: свернуть в трей вместо выхода
        if ctx.input(|i| i.viewport().close_requested()) {
            if self.shot.is_none() && !self.exit_requested && self.ini.get_bool("CloseToTray", true) {
                ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
                self.window_visible = false;
            }
        }
        // видимость окна (тихий старт, трей, показ из трея)
        if self.window_visibility_sent != Some(self.window_visible) {
            ctx.send_viewport_cmd(egui::ViewportCommand::Visible(self.window_visible));
            self.window_visibility_sent = Some(self.window_visible);
        }

        self.handle_messages();
        if self.last_refresh.elapsed() > Duration::from_secs(5) {
            self.last_refresh = Instant::now();
            // Перебор процессов не должен подвешивать UI-поток во время скролла.
            let ops = ZapretOps::new(self.install.clone());
            let tx = self.tx.clone();
            std::thread::spawn(move || {
                let st = ops.get_state();
                let _ = tx.send(Msg::State(st));
            });
        }
        self.poll_guard();
        // Состояние TG-прокси обновляем не чаще раза в 2 секунды и в фоне:
        // get_state перебирает процессы и иначе даёт микро-фризы при скролле.
        if self.last_tg_refresh.elapsed() > Duration::from_secs(2) {
            self.last_tg_refresh = Instant::now();
            let ini = self.ini.clone();
            let tx = self.tx.clone();
            std::thread::spawn(move || {
                let st = tgproxy::get_state(&ini);
                let _ = tx.send(Msg::TgState(st));
            });
        }
        let maximized = ctx.input(|i| i.viewport().maximized.unwrap_or(false));
        self.last_maximized = maximized;
        if !maximized {
            self.last_normal_size = (ctx.screen_rect().width(), ctx.screen_rect().height());
        }
        if self.window_hwnd.is_none() {
            self.window_hwnd = sys::window_for_current_process("Zapret Manager");
        }
        if let Some(hwnd) = self.window_hwnd {
            if !self.icon_set {
                let ok = sys::set_window_icon(hwnd);
                if !ok {
                    log::warn("Не удалось установить иконку окна из ресурса exe.");
                }
                self.icon_set = true;
            }
            if self.titlebar_dark != Some(self.dark) {
                sys::set_titlebar_theme_isize(hwnd, self.dark);
                self.titlebar_dark = Some(self.dark);
            }
        }

        if !self.bootstrap_done && self.autostart_mode {
            self.bootstrap_done = true;
            if self.ini.get_bool("AutostartZapret", true) && self.install.is_valid() {
                if let Some(s) = self.pick_strategy() {
                    if !self.state.winws_running && self.state.service_status != "работает" {
                        let ops = ZapretOps::new(self.install.clone());
                        std::thread::spawn(move || {
                            let _ = ops.start(&s);
                        });
                    }
                }
            }
            if self.ini.get_bool("AutostartTgProxy", true) {
                let ini = self.ini.clone();
                std::thread::spawn(move || {
                    let st = tgproxy::get_state(&ini);
                    if st.installed && !st.running {
                        let _ = tgproxy::start(&ini);
                    }
                });
            }
        }

        self.sidebar(ctx);
        self.status_bar(ctx);
        let screen_w = ctx.screen_rect().width();
        let side = ((screen_w - 236.0 - 1200.0) * 0.5).max(24.0).min(400.0) as i8;
        egui::CentralPanel::default()
            .frame(
                egui::Frame::none()
                    .fill(self.p.bg)
                    .inner_margin(egui::Margin::symmetric(side, 0)),
            )
            .show(ctx, |ui| {
                self.header(ui);
                egui::ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        ui.add_space(8.0);
                        match self.page {
                            Page::Overview => self.page_overview(ui),
                            Page::Strategies => self.page_strategies(ui),
                            Page::Test => self.page_test(ui),
                            Page::Sites => self.page_sites(ui),
                            Page::TgProxy => self.page_tgproxy(ui),
                            Page::Settings => self.page_settings(ui),
                            Page::Lists => self.page_lists(ui),
                            Page::Update => self.page_update(ui),
                            Page::Diagnostics => self.page_diagnostics(ui),
                            Page::Tools => self.page_tools(ui),
                            Page::About => self.page_about(ui),
                        }
                        ui.add_space(24.0);
                    });
            });

        self.handle_confirm(ctx);
        self.show_confirm_overlay(ctx);
        self.show_update_notice(ctx);
        self.show_toast(ctx);
        if let Some(text) = self.clipboard.take() {
            ctx.copy_text(text);
        }
        self.show_wizard(ctx);

        if self.exit_requested {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
        if self.shot.is_some() {
            ctx.request_repaint();
        }
        if self.busy > 0 || self.pages.test.running || self.pages.sites.watching {
            ctx.request_repaint_after(Duration::from_millis(300));
        } else {
            ctx.request_repaint_after(Duration::from_millis(900));
        }
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        if let Some(tray) = self.tray.take() {
            tray.close();
        }
        self.ini
            .set_int("WindowW", self.last_normal_size.0.round().max(800.0) as i64);
        self.ini
            .set_int("WindowH", self.last_normal_size.1.round().max(600.0) as i64);
        self.ini.set_bool("Maximized", self.last_maximized);
        self.ini.set_int("UiScale", self.pages.settings.scale as i64);
        let _ = self.ini.save();
        log::info("=== Завершение работы ===");
    }
}

impl App {
    fn show_confirm_overlay(&mut self, ctx: &egui::Context) {
        if self.confirm.is_some() {
            let rect = ctx.screen_rect();
            let painter = ctx.layer_painter(egui::LayerId::new(
                egui::Order::Middle,
                egui::Id::new("confirm_overlay"),
            ));
            painter.rect_filled(rect, Rounding::ZERO, Color32::from_black_alpha(140));
        }
    }
}

// ---------------------------------------------------------------------------
// Вспомогательные тексты
// ---------------------------------------------------------------------------

pub fn site_steps(probe: &SiteProbe) -> String {
    let mut lines = Vec::new();
    if probe.dns_ok {
        let first = probe
            .ips
            .first()
            .map(|ip| ip.to_string())
            .unwrap_or_else(|| "ок".to_string());
        lines.push(format!("DNS: {}  ({} шт.)", first, probe.ips.len()));
    } else {
        lines.push(format!("DNS: ошибка — {}", probe.dns_error));
    }
    if probe.dns_ok {
        if probe.tcp_ok {
            lines.push(format!("TCP :443 — ок, {} мс", probe.tcp_ms));
        } else {
            lines.push(format!("TCP :443 — ошибка: {}", probe.tcp_error));
        }
    } else {
        lines.push("TCP :443 — пропущено".to_string());
    }
    if probe.tcp_ok {
        if probe.tls_ok {
            lines.push(format!("TLS (SNI {}) — ок, {} мс", probe.host, probe.tls_ms));
        } else {
            lines.push(format!("TLS — ошибка: {}", probe.tls_error));
        }
    } else {
        lines.push("TLS — пропущено".to_string());
    }
    if probe.tls_ok {
        if probe.http_ok {
            let mut line = format!("HTTP — {}", probe.http_status);
            if !probe.html_title.is_empty() {
                line.push_str(&format!(
                    ", заголовок: «{}»",
                    crate::util::shorten(&probe.html_title, 60)
                ));
            }
            lines.push(line);
        } else {
            lines.push(format!("HTTP — ошибка: {}", probe.http_error));
        }
    }
    lines.join("\n")
}

pub fn test_summary(state: &TestState) -> String {
    if state.rows.is_empty() {
        return "Прогон ещё не выполнялся.".to_string();
    }
    if state.running {
        return "Прогон…".to_string();
    }
    let total_targets = state.rows.iter().map(|r| r.total()).max().unwrap_or(0);
    let launched: Vec<&StrategyTestRow> = state.rows.iter().filter(|r| r.error.is_empty()).collect();
    if launched.is_empty() {
        return "Ни одна стратегия не запустилась — проверьте права администратора и драйвер WinDivert."
            .to_string();
    }
    let best = launched.iter().max_by_key(|r| r.score()).unwrap();
    if best.passed() == 0 {
        return format!(
            "Проверено {} стратегий, ни одна не дала доступа к целям (0 из {}). Возможно, проблема на стороне сети.",
            state.rows.len(),
            total_targets
        );
    }
    format!(
        "Лучшая: {} — {} из {} (сервисов: {} из {}). Проверено стратегий: {}.",
        best.strategy_name,
        best.passed(),
        best.total(),
        best.healthy_groups(),
        best.groups.len(),
        state.rows.len()
    )
}

pub fn lists_info_of(st: &crate::ui::app::ListsState) -> String {
    let file = match st.files.get(st.selected) {
        Some(f) => f,
        None => return String::new(),
    };
    let lines = st.editor.lines().filter(|l| !l.trim().is_empty()).count();
    let entries = st
        .editor
        .lines()
        .filter(|l| {
            let t = l.trim_start();
            !t.is_empty() && !t.starts_with('#')
        })
        .count();
    let dups = crate::engine::lists::count_kind(
        &st.notes,
        crate::engine::lists::NoteKind::Duplicate,
    );
    let wild = crate::engine::lists::count_kind(
        &st.notes,
        crate::engine::lists::NoteKind::Wildcard,
    );
    let mut info = format!(
        "{}  •  строк: {}, записей: {}",
        if st.dirty {
            "Есть несохранённые изменения"
        } else {
            "Сохранено"
        },
        lines,
        entries
    );
    if !file.official && entries == 0 {
        info.push_str("  •  файл пуст — список не будет применяться");
    }
    if dups > 0 {
        info.push_str(&format!("  •  дубликатов: {}", dups));
    }
    if wild > 0 {
        info.push_str(&format!("  •  «*.» не поддерживается: {}", wild));
    }
    info
}

pub fn diag_summary(results: &[CheckResult]) -> String {    use crate::engine::diagnostics::Severity;
    let errors = results.iter().filter(|r| r.severity == Severity::Error).count();
    let warns = results.iter().filter(|r| r.severity == Severity::Warn).count();
    let ok = results.iter().filter(|r| r.severity == Severity::Ok).count();
    format!(
        "Ошибок: {}   •   предупреждений: {}   •   в порядке: {}",
        errors, warns, ok
    )
}
