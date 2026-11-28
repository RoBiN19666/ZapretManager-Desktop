//! Диагностика: службы, конфликты, сеть, hosts, списки, TG WS Proxy.

use std::path::Path;

use super::bat;
use super::blobs;
use super::gamefilter::GameFilterState;
use super::install::ZapretInstall;
use super::ops::ZapretOps;
use super::process;
use super::service;
use super::sys;
use super::textfile;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Severity {
    Ok,
    Warn,
    Error,
}

impl Severity {
    pub fn text(&self) -> &'static str {
        match self {
            Severity::Ok => "OK",
            Severity::Warn => "ВНИМАНИЕ",
            Severity::Error => "ОШИБКА",
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct CheckResult {
    pub name: String,
    pub severity: Severity,
    pub message: String,
    pub hint: String,
    pub action_id: String,
    pub action_text: String,
    pub group: String,
}

impl Default for Severity {
    fn default() -> Self {
        Severity::Ok
    }
}

struct Builder {
    results: Vec<CheckResult>,
}

impl Builder {
    fn add(
        &mut self,
        group: &str,
        name: &str,
        severity: Severity,
        message: String,
        hint: &str,
    ) {
        self.results.push(CheckResult {
            name: name.to_string(),
            severity,
            message,
            // Подсказку сохраняем и для «всё хорошо»: в ней бывают полезные
            // советы (шифрованный DNS, кэш Discord, установка TG WS Proxy).
            hint: hint.to_string(),
            group: group.to_string(),
            ..Default::default()
        });
    }

    fn action(&mut self, id: &str, text: &str) {
        if let Some(last) = self.results.last_mut() {
            last.action_id = id.to_string();
            last.action_text = text.to_string();
        }
    }
}

fn service_names() -> Vec<String> {
    sys::enum_subkeys(true, r"SYSTEM\CurrentControlSet\Services")
}

fn find_services(names: &[String], predicate: impl Fn(&str) -> bool) -> Vec<String> {
    names
        .iter()
        .filter(|n| predicate(&n.to_lowercase()))
        .cloned()
        .collect()
}

fn join_names(names: &[String]) -> String {
    names.join(", ")
}

/// Все проверки по порядку.
pub fn run(
    install: &ZapretInstall,
    ops: &ZapretOps,
    mut step: impl FnMut(&str),
) -> Vec<CheckResult> {
    let mut b = Builder {
        results: Vec::new(),
    };

    step("Права администратора…");
    if sys::is_admin() {
        b.add(
            "Система",
            "Права администратора",
            Severity::Ok,
            "Приложение запущено с правами администратора.".to_string(),
            "",
        );
    } else {
        b.add(
            "Система",
            "Права администратора",
            Severity::Error,
            "Приложение запущено без прав администратора — запуск ускорения и управление службами недоступны."
                .to_string(),
            "Запустите Zapret Manager от имени администратора.",
        );
    }

    step("Установка zapret…");
    let valid = install.is_valid();
    if let Some(note) = install.unsupported_note() {
        b.add(
            "Система",
            "Ядро zapret",
            Severity::Warn,
            note,
            "Это не ошибка установки: zapret 2.x работает сам через свой service.bat. Менеджер не запускает его стратегии и не предлагает обновления на 2.x.",
        );
    }
    if valid {
        let version = if install.version.is_empty() {
            "неизвестной версии".to_string()
        } else {
            format!("версии {}", install.version)
        };
        b.add(
            "Система",
            "Файлы zapret",
            Severity::Ok,
            format!("Найдена установка {} в {}", version, install.root),
            "",
        );
    } else {
        b.add(
            "Система",
            "Файлы zapret",
            Severity::Error,
            "Папка zapret не найдена: нет service.bat или bin\\winws.exe.".to_string(),
            "Укажите папку zapret вручную на странице «Обзор».",
        );
    }

    if valid {
        let sys_count = std::fs::read_dir(install.bin_dir())
            .map(|entries| {
                entries
                    .flatten()
                    .filter(|e| {
                        e.path()
                            .extension()
                            .map(|x| x.to_string_lossy().eq_ignore_ascii_case("sys"))
                            .unwrap_or(false)
                    })
                    .count()
            })
            .unwrap_or(0);
        if sys_count > 0 {
            b.add(
                "Система",
                "Драйвер WinDivert",
                Severity::Ok,
                "WinDivert64.sys на месте.".to_string(),
                "",
            );
        } else {
            b.add(
                "Система",
                "Драйвер WinDivert",
                Severity::Error,
                "Файл WinDivert64.sys не найден в папке bin.".to_string(),
                "Без драйвера ускорение не работает. Обновите или переустановите zapret.",
            );
        }

        let has_cyrillic = install.root.chars().any(|c| ('\u{0410}'..='\u{044F}').contains(&c) || c == 'ё' || c == 'Ё');
        if has_cyrillic {
            b.add(
                "Система",
                "Путь установки",
                Severity::Warn,
                format!("Путь содержит кириллицу: {}", install.root),
                "Если ускорение не работает, перенесите папку, например, в C:\\zapret.",
            );
        } else {
            b.add(
                "Система",
                "Путь установки",
                Severity::Ok,
                format!("Путь без кириллицы — {}", install.root),
                "",
            );
        }

        let one_drive = std::env::var("OneDrive").unwrap_or_default();
        if !one_drive.is_empty() && install.root.to_lowercase().starts_with(&one_drive.to_lowercase()) {
            b.add(
                "Система",
                "OneDrive",
                Severity::Error,
                "zapret установлен в папку OneDrive.".to_string(),
                "Перенесите папку из OneDrive, например в C:\\zapret.",
            );
        } else {
            b.add(
                "Система",
                "OneDrive",
                Severity::Ok,
                "Установка не находится в OneDrive.".to_string(),
                "",
            );
        }
    }

    step("Службы Windows…");
    let names = service_names();
    let bfe = service::query("BFE");
    if !bfe.installed {
        b.add(
            "Система",
            "Base Filtering Engine",
            Severity::Error,
            "Служба BFE не найдена.".to_string(),
            "Базовый фильтрующий движок Windows остановлен. Он обязателен для работы zapret.",
        );
    } else if bfe.status == "работает" {
        b.add(
            "Система",
            "Base Filtering Engine",
            Severity::Ok,
            "Служба BFE: работает".to_string(),
            "",
        );
    } else {
        b.add(
            "Система",
            "Base Filtering Engine",
            Severity::Error,
            format!("Служба BFE: не работает ({})", bfe.status),
            "Базовый фильтрующий движок Windows остановлен. Он обязателен для работы zapret.",
        );
    }

    step("Состояние ускорения…");
    let state = ops.get_state();
    let snap = service::query("zapret");
    if state.winws_running {
        b.add(
            "Ускорение",
            "Процесс winws.exe",
            Severity::Ok,
            "winws.exe запущен.".to_string(),
            "",
        );
    } else if !snap.installed {
        b.add(
            "Ускорение",
            "Процесс winws.exe",
            Severity::Warn,
            "Ускорение не запущено.".to_string(),
            "Запустите стратегию на странице «Стратегии».",
        );
    }
    if snap.installed {
        let mut message = format!("Служба установлена, состояние: {}", snap.status);
        if !snap.strategy.is_empty() {
            message.push_str(&format!(", стратегия: {}", snap.strategy));
        }
        b.add(
            "Ускорение",
            "Служба zapret",
            if snap.status == "работает" {
                Severity::Ok
            } else {
                Severity::Warn
            },
            message,
            "Служба установлена, но не работает. Запустите её кнопкой ниже.",
        );
        if snap.status != "работает" {
            b.action("restart_service", "Перезапустить службу");
        }
    }
    if valid {
        if ops.service_points_elsewhere() {
            b.add(
                "Ускорение",
                "Папка службы",
                Severity::Warn,
                format!(
                    "Служба zapret установлена из другой папки: {}",
                    ops.service_install_root().unwrap_or_default()
                ),
                &format!(
                    "Менеджер управляет папкой {}. {}",
                    install.root,
                    if install.is_zapret2() {
                        "Службой zapret 2.x управляет его собственный service.bat — менеджер её не переустанавливает."
                    } else {
                        "Переустановите службу из нужной папки (страница «Обзор»)."
                    }
                ),
            );
        } else if ops.service_path_missing() {
            b.add(
                "Ускорение",
                "Папка службы",
                Severity::Error,
                "Служба zapret указывает на несуществующую папку.".to_string(),
                "После перезагрузки ускорение не запустится. Переустановите службу из текущей папки.",
            );
        }
    }
    let windivert = service::query("WinDivert");
    if !state.winws_running && windivert.installed && windivert.status == "работает" {
        b.add(
            "Ускорение",
            "Зависший драйвер WinDivert",
            Severity::Error,
            "Служба WinDivert активна, но winws.exe не запущен — драйвер занят и блокирует запуск."
                .to_string(),
            "Удалите зависший драйвер WinDivert.",
        );
        b.action("windivert", "Удалить WinDivert");
    }
    if valid {
        let mode = ops.get_ipset_mode();
        let lines = super::ops::count_list_lines(&install.ipset_file());
        if mode == "loaded" {
            if lines > 1000 {
                b.add(
                    "Ускорение",
                    "IPSet список",
                    Severity::Ok,
                    format!("Загружен полный список: {} строк.", lines),
                    "",
                );
            } else {
                b.add(
                    "Ускорение",
                    "IPSet список",
                    Severity::Warn,
                    format!("Загружен полный список: {} строк.", lines),
                    "Список подозрительно мал — обновите его на странице «Настройки».",
                );
            }
        } else {
            let backup_lines = super::ops::count_list_lines(&install.ipset_backup());
            b.add(
                "Ускорение",
                "IPSet список",
                Severity::Ok,
                format!(
                    "Режим «{}». Резервный список: {} строк.",
                    super::ops::ipset_mode_text(&mode),
                    backup_lines
                ),
                "",
            );
        }
    }

    step("Системный прокси…");
    let proxy_key = sys::RegKey::open(
        false,
        r"Software\Microsoft\Windows\CurrentVersion\Internet Settings",
        false,
    );
    let (proxy_enabled, proxy_server) = match &proxy_key {
        Some(k) => (
            k.get_dword("ProxyEnable").unwrap_or(0),
            k.get_string("ProxyServer").unwrap_or_default(),
        ),
        None => (0, String::new()),
    };
    if proxy_enabled == 1 {
        b.add(
            "Система",
            "Системный прокси",
            Severity::Warn,
            format!("Системный прокси включён: {}", proxy_server),
            "Убедитесь, что прокси рабочий, или отключите его — иначе Discord/YouTube могут не работать.",
        );
    } else {
        b.add(
            "Система",
            "Системный прокси",
            Severity::Ok,
            "Системный прокси не используется.".to_string(),
            "",
        );
    }

    step("TCP timestamps…");
    if valid {
        match ops.tcp_timestamps_enabled() {
            None => b.add(
                "Сеть",
                "TCP timestamps",
                Severity::Ok,
                "Состояние TCP timestamps определить не удалось (не критично).".to_string(),
                "",
            ),
            Some(true) => b.add(
                "Сеть",
                "TCP timestamps",
                Severity::Ok,
                "TCP timestamps включены.".to_string(),
                "",
            ),
            Some(false) => {
                b.add(
                    "Сеть",
                    "TCP timestamps",
                    Severity::Warn,
                    "TCP timestamps выключены — часть стратегий без них не работает.".to_string(),
                    "Включите параметр (кнопка ниже).",
                );
                b.action("tcp", "Включить timestamps");
            }
        }
    }

    step("Конфликтующие программы…");
    let adguard = sys::list_processes()
        .iter()
        .any(|p| p.name.eq_ignore_ascii_case("AdguardSvc.exe"));
    if adguard {
        b.add(
            "Конфликты",
            "Adguard",
            Severity::Error,
            "Найден процесс AdguardSvc.exe.".to_string(),
            "Adguard конфликтует с Discord при работе zapret. Закройте его.",
        );
    } else {
        b.add(
            "Конфликты",
            "Adguard",
            Severity::Ok,
            "Adguard не запущен.".to_string(),
            "",
        );
    }

    let killer = find_services(&names, |n| n.contains("killer"));
    if killer.is_empty() {
        b.add(
            "Конфликты",
            "Killer (Rivet Networks)",
            Severity::Ok,
            "Служба Killer не найдена.".to_string(),
            "",
        );
    } else {
        b.add(
            "Конфликты",
            "Killer (Rivet Networks)",
            Severity::Error,
            format!("Найдены службы Killer: {}", join_names(&killer)),
            "Killer конфликтует с zapret — отключите его.",
        );
    }

    let intel = find_services(&names, |n| {
        n.contains("intel") && n.contains("connectivity") && n.contains("network")
    });
    if intel.is_empty() {
        b.add(
            "Конфликты",
            "Intel Connectivity",
            Severity::Ok,
            "Службы Intel Connectivity не найдены.".to_string(),
            "",
        );
    } else {
        b.add(
            "Конфликты",
            "Intel Connectivity",
            Severity::Error,
            format!("Найдены службы Intel Connectivity: {}", join_names(&intel)),
            "Известный конфликт с zapret.",
        );
    }

    let mut checkpoint = find_services(&names, |n| n.contains("tracsrvwrapper"));
    checkpoint.extend(find_services(&names, |n| n.contains("epwd")));
    if checkpoint.is_empty() {
        b.add(
            "Конфликты",
            "Check Point",
            Severity::Ok,
            "Check Point не найден.".to_string(),
            "",
        );
    } else {
        b.add(
            "Конфликты",
            "Check Point",
            Severity::Error,
            format!("Найдены службы Check Point: {}", join_names(&checkpoint)),
            "Check Point конфликтует с zapret, попробуйте удалить.",
        );
    }

    let smartbyte = find_services(&names, |n| n.contains("smartbyte"));
    if smartbyte.is_empty() {
        b.add(
            "Конфликты",
            "SmartByte",
            Severity::Ok,
            "SmartByte не найден.".to_string(),
            "",
        );
    } else {
        b.add(
            "Конфликты",
            "SmartByte",
            Severity::Error,
            format!("Найдены службы SmartByte: {}", join_names(&smartbyte)),
            "SmartByte конфликтует с zapret — отключите службу.",
        );
    }

    let vpn = find_services(&names, |n| n.contains("vpn") || n.contains("happservice"));
    if vpn.is_empty() {
        b.add(
            "Конфликты",
            "VPN",
            Severity::Ok,
            "VPN-службы не найдены.".to_string(),
            "",
        );
    } else {
        b.add(
            "Конфликты",
            "VPN",
            Severity::Warn,
            format!("Найдены VPN-службы: {}", join_names(&vpn)),
            "Некоторые VPN конфликтуют с zapret — отключите их на время проверки.",
        );
    }

    let conflicts = find_services(&names, |n| {
        matches!(n, "goodbyedpi" | "discordfix_zapret" | "winws1" | "winws2")
    });
    if conflicts.is_empty() {
        b.add(
            "Конфликты",
            "Другие службы ускорения",
            Severity::Ok,
            "Конкурирующих служб ускорения не найдено.".to_string(),
            "",
        );
    } else {
        b.add(
            "Конфликты",
            "Другие службы ускорения",
            Severity::Error,
            format!("Найдены конкурирующие службы: {}", join_names(&conflicts)),
            "Остановите и удалите их, иначе два ускорения будут мешать друг другу.",
        );
        b.action("conflicts", "Удалить конфликты");
    }

    if windivert.installed && windivert.status == "работает" && !state.winws_running {
        b.add(
            "Ускорение",
            "Служба WinDivert",
            Severity::Warn,
            "Служба WinDivert запущена без winws.exe.".to_string(),
            "Очистите её, чтобы избежать конфликта.",
        );
        b.action("windivert", "Очистить WinDivert");
    }

    step("DNS и hosts…");
    let doh_count = sys::enum_subkeys(
        true,
        r"SYSTEM\CurrentControlSet\Services\Dnscache\InterfaceSpecificParameters",
    )
    .iter()
    .filter(|iface| {
        sys::RegKey::open(
            true,
            &format!(
                r"SYSTEM\CurrentControlSet\Services\Dnscache\InterfaceSpecificParameters\{}",
                iface
            ),
            false,
        )
        .and_then(|k| k.get_dword("DohFlags"))
        .unwrap_or(0)
            > 0
    })
    .count();
    if doh_count > 0 {
        b.add(
            "Сеть",
            "Шифрованный DNS (DoH)",
            Severity::Ok,
            format!("Шифрованный DNS настроен в системе ({} интерф.), проверить нечего.", doh_count),
            "",
        );
    } else {
        b.add(
            "Сеть",
            "Шифрованный DNS (DoH)",
            Severity::Ok,
            "Шифрованный DNS в системе не настроен.".to_string(),
            "Включите «Шифрованный DNS» в настройках Windows 11 или в браузере — так ускорение работает стабильнее.",
        );
    }

    let hosts = super::ops::hosts_path();
    let hosts_text = textfile::read(&hosts).to_lowercase();
    if hosts_text.contains("youtube.com") || hosts_text.contains("youtu.be") {
        b.add(
            "Сеть",
            "Файл hosts",
            Severity::Warn,
            "В hosts есть записи youtube.com / youtu.be — это может ломать доступ к YouTube."
                .to_string(),
            if hosts_text.contains("# >>> zapret-manager") {
                "Записи добавлены менеджером — их можно убрать кнопкой."
            } else {
                "Проверьте файл hosts и удалите эти строки."
            },
        );
        if hosts_text.contains("# >>> zapret-manager") {
            b.action("hosts", "Убрать блок менеджера");
        }
    } else {
        b.add(
            "Сеть",
            "Файл hosts",
            Severity::Ok,
            "В hosts нет записей YouTube.".to_string(),
            "",
        );
    }

    step("Файлы списков…");
    if valid {
        let mut missing: Vec<String> = Vec::new();
        let gf = GameFilterState::load(&install.root);
        for strategy in &install.strategies {
            let Ok(argv) = bat::get_argv(install, strategy, &gf) else {
                continue;
            };
            for token in &argv {
                let Some(eq) = token.find('=') else { continue };
                let key = token[..eq].trim_matches('"');
                if !matches!(
                    key,
                    "--hostlist" | "--hostlist-exclude" | "--ipset" | "--ipset-exclude"
                ) {
                    continue;
                }
                let value = token[eq + 1..].trim_matches('"');
                if value.is_empty() || value.contains('%') {
                    continue;
                }
                let path = if Path::new(value).is_absolute() {
                    std::path::PathBuf::from(value)
                } else {
                    install.bin_dir().join(value)
                };
                if !path.is_file() {
                    let name = path
                        .file_name()
                        .map(|s| s.to_string_lossy().to_string())
                        .unwrap_or_else(|| value.to_string());
                    if !missing.contains(&name) {
                        missing.push(name);
                    }
                }
            }
        }
        if missing.is_empty() {
            b.add(
                "Списки",
                "Файлы списков",
                Severity::Ok,
                "Все списки, на которые ссылаются стратегии, существуют.".to_string(),
                "",
            );
        } else {
            b.add(
                "Списки",
                "Файлы списков",
                Severity::Error,
                format!("Не найдены файлы списков: {}", join_names(&missing)),
                "Откройте «Списки» и сохраните существующие файлы или обновите zapret.",
            );
        }
    }

    if valid {
        step("Файлы ускорения winws…");
        let audit = blobs::audit(install);
        if audit.is_ok() {
            b.add(
                "Ускорение",
                "Файлы ускорения (blobs)",
                Severity::Ok,
                blobs::summary(&audit),
                "",
            );
        } else {
            b.add(
                "Ускорение",
                "Файлы ускорения (blobs)",
                Severity::Error,
                blobs::summary(&audit),
                "Не хватает .bin-файлов, на которые ссылаются стратегии. Их можно докачать из официальных репозиториев.",
            );
            b.action("blobs", "Проверить и докачать");
        }
    }

    let discord = super::ops::discord_cache_dirs();
    if discord.is_empty() {
        b.add(
            "Ускорение",
            "Кэш Discord",
            Severity::Ok,
            "Установки Discord не найдены.".to_string(),
            "",
        );
    } else {
        let dirs: Vec<String> = discord.iter().map(|p| p.to_string_lossy().to_string()).collect();
        b.add(
            "Ускорение",
            "Кэш Discord",
            Severity::Ok,
            format!("Найдены установки Discord: {}", dirs.join(", ")),
            "При проблемах со звуком/подключением очистите кэш.",
        );
        b.action("discord", "Очистить кэш Discord");
    }

    step("TG WS Proxy…");
    let tg = super::tgproxy::get_state(&super::config::Ini::load(&super::paths::config_path()));
    if !tg.installed {
        b.add(
            "Telegram",
            "TG WS Proxy",
            Severity::Ok,
            "Прокси Telegram не установлен (необязательный компонент).".to_string(),
            "Установить можно на странице «TG-прокси» или через «Мастер установки».",
        );
    } else {
        b.add(
            "Telegram",
            "TG WS Proxy",
            Severity::Ok,
            format!(
                "Установлен: версия {}, {}",
                if tg.version.is_empty() {
                    "неизвестна"
                } else {
                    &tg.version
                },
                tg.dir.to_string_lossy()
            ),
            "",
        );
        if tg.running {
            if tg.port_listening {
                b.add(
                    "Telegram",
                    "Прокси Telegram",
                    Severity::Ok,
                    format!(
                        "Работает на {}:{} (PID {}), секрет {}.",
                        tg.host,
                        tg.port,
                        tg.pid,
                        if tg.secret.is_empty() { "нет" } else { "есть" }
                    ),
                    "",
                );
            } else {
                b.add(
                    "Telegram",
                    "Прокси Telegram",
                    Severity::Warn,
                    format!("Процесс запущен (PID {}), но порт {} не слушается.", tg.pid, tg.port),
                    "Проверьте журнал TG WS Proxy и свободен ли порт.",
                );
            }
        } else {
            b.add(
                "Telegram",
                "Прокси Telegram",
                Severity::Ok,
                "Остановлен.".to_string(),
                "",
            );
        }
        if tg.instance_count > 2 {
            b.add(
                "Telegram",
                "Экземпляры прокси",
                Severity::Warn,
                format!("Запущено несколько копий TG WS Proxy: {}.", tg.instance_count),
                "Лишние копии мешают друг другу. Нажмите «Остановить», затем «Запустить» на странице «TG-прокси».",
            );
        }
        if !tg.running && tg.port_listening {
            b.add(
                "Telegram",
                &format!("Порт {}", tg.port),
                Severity::Warn,
                format!("Порт {} занят другим процессом, хотя TG WS Proxy не запущен.", tg.port),
                "Освободите порт или измените его в config.json прокси.",
            );
        }
        if !tg.config_error.is_empty() {
            b.add(
                "Telegram",
                "Конфигурация прокси",
                Severity::Ok,
                format!("config.json: {}", tg.config_error),
                "",
            );
        }
    }

    step("Готово.");
    b.results
}

fn admin_error() -> String {
    if !sys::is_admin() {
        "Нужны права администратора.".to_string()
    } else {
        String::new()
    }
}

/// Выполнить исправление по идентификатору действия.
pub fn run_action(action_id: &str, ops: &ZapretOps) -> String {
    match action_id {
        "windivert" => {
            let err = admin_error();
            if !err.is_empty() {
                return err;
            }
            let mut deleted = false;
            for name in ["WinDivert", "WinDivert14"] {
                if service::exists(name) {
                    let _ = service::stop(name);
                    if service::delete(name).is_ok() {
                        deleted = true;
                    }
                }
            }
            if deleted {
                "Драйвер WinDivert остановлен и удалён.".to_string()
            } else {
                "Не удалось удалить драйвер WinDivert (служба занята или нет прав).".to_string()
            }
        }
        "conflicts" => {
            let err = admin_error();
            if !err.is_empty() {
                return err;
            }
            let mut report = String::new();
            let mut found = false;
            for name in ["GoodbyeDPI", "discordfix_zapret", "winws1", "winws2"] {
                if service::exists(name) {
                    found = true;
                    let _ = service::stop(name);
                    if service::delete(name).is_ok() {
                        report.push_str(&format!("Удалена служба {}. ", name));
                    } else {
                        report.push_str(&format!("Не удалось удалить {}. ", name));
                    }
                }
            }
            if !found {
                "Конфликтующих служб не найдено.".to_string()
            } else {
                report.trim().to_string()
            }
        }
        "tcp" => {
            let err = admin_error();
            if !err.is_empty() {
                return err;
            }
            match ops.enable_tcp_timestamps() {
                Some(true) => "TCP timestamps включены.".to_string(),
                _ => "Не удалось включить TCP timestamps.".to_string(),
            }
        }
        "discord" => ops.clear_discord_cache(),
        "hosts" => ops.clear_hosts_block(),
        "blobs" => blobs::repair(&ops.install),
        "restart_service" => {
            let err = admin_error();
            if !err.is_empty() {
                return err;
            }
            match service::start("zapret") {
                Ok(()) => "Служба zapret запущена.".to_string(),
                Err(e) => format!("Не удалось запустить службу: {}", e),
            }
        }
        _ => "Действие не поддерживается.".to_string(),
    }
}

/// Текст отчёта для буфера обмена.
pub fn report_text(install: &ZapretInstall, results: &[CheckResult]) -> String {
    let now = sys::local_now();
    let mut text = format!(
        "Zapret Manager {} — отчёт диагностики {:02}.{:02}.{:04} {:02}:{:02}\n",
        crate::APP_VERSION,
        now.day,
        now.month,
        now.year,
        now.hour,
        now.minute
    );
    if install.is_valid() {
        text.push_str(&format!(
            "Установка: {} (zapret {})\n",
            install.root, install.version
        ));
    }
    text.push('\n');
    for r in results {
        text.push_str(&format!("[{}] {}: {}", r.severity.text(), r.name, r.message));
        if !r.hint.is_empty() {
            text.push_str(&format!(" | {}", r.hint));
        }
        text.push('\n');
    }
    text
}

#[allow(dead_code)]
fn unused(_: &process::ProcResult) {}
