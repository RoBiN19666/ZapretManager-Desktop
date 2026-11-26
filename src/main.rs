#![windows_subsystem = "windows"]

use std::time::Duration;

use zapret_manager::engine::config::Ini;
use zapret_manager::engine::install::ZapretInstall;
use zapret_manager::engine::{diagnostics, doctor, guard, log, paths, sys, tgproxy, update};
use zapret_manager::engine::appprobe;
use zapret_manager::engine::ops::ZapretOps;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();

    if has_flag(&args, "--apply-update") {
        update::apply_update_mode();
        return;
    }

    log::set_file(paths::log_path());
    log::info(format!(
        "=== Zapret Manager {} (Rust) запуск ===",
        zapret_manager::APP_VERSION
    ));

    let root_arg = arg_value(&args, "--root").unwrap_or_default();
    let shot = arg_value(&args, "--shot");
    let shot_page = arg_value(&args, "--shot-page");
    let limited = has_flag(&args, "--limited");
    let elevated = has_flag(&args, "--elevated");
    let autostart = has_flag(&args, "--autostart");
    let tray = has_flag(&args, "--tray") || autostart;

    if !sys::is_admin() && !limited && !elevated && shot.is_none() {
        let mut elevated_args: Vec<String> = vec!["--elevated".to_string()];
        for arg in &args {
            if arg.eq_ignore_ascii_case("--elevated") || arg.eq_ignore_ascii_case("--limited") {
                continue;
            }
            elevated_args.push(arg.clone());
        }
        log::info("Запрашиваю права администратора…");
        if sys::elevate_self(&elevated_args) {
            return;
        }
        log::warn("Пользователь отклонил запрос UAC — работаем без прав администратора.");
    }

    let headless = has_flag(&args, "--diag")
        || has_flag(&args, "--cleanup")
        || has_flag(&args, "--check-self")
        || has_flag(&args, "--self-update")
        || has_flag(&args, "--app-watch")
        || has_flag(&args, "--guard-once")
        || has_flag(&args, "--tg-prepare")
        || arg_value(&args, "--check-site").is_some();
    if shot.is_none() && !headless && !sys::acquire_single_instance("ZapretManagerRust_SingleInstance") {
        for _ in 0..6 {
            if sys::activate_window("Zapret Manager") {
                log::info("Передал запрос на показ окна уже запущенному экземпляру.");
                return;
            }
            std::thread::sleep(Duration::from_millis(300));
        }
        return;
    }

    let mut ini = Ini::load(&paths::config_path());

    // ---- Служебные режимы без интерфейса -------------------------------
    if has_flag(&args, "--check-self") {
        let info = update::self_check(true);
        log::info(format!(
            "Самообновление: локальная {}, релиз {}, доступна {}, ok={}, error={}",
            info.local, info.remote, info.available, info.ok, info.error
        ));
        return;
    }
    if has_flag(&args, "--self-update") {
        let info = update::self_check(true);
        if !info.ok || !info.available {
            log::info("Обновление менеджера не требуется.");
            return;
        }
        match update::self_apply(&info) {
            Ok(message) => log::ok(format!("Самообновление: {}", message)),
            Err(e) => log::error(format!("Самообновление: {}", e)),
        }
        return;
    }
    if has_flag(&args, "--cleanup") {
        log::info(zapret_manager::engine::ops::cleanup_report());
        return;
    }
    if let Some(value) = arg_value(&args, "--app-watch") {
        let Ok(pid) = value.parse::<u32>() else {
            log::error("--app-watch: нужен PID процесса.");
            return;
        };
        let preferred = if root_arg.is_empty() {
            ini.get("Root", "")
        } else {
            root_arg.clone()
        };
        let install = ZapretInstall::detect(&preferred);
        log::info(format!("Проверка приложения: наблюдаем за PID {} 20 с.", pid));
        let (hosts, ips) = appprobe::watch(pid, 20, |_, _, _| {});
        let filtered = appprobe::filter_hosts(&hosts);
        let (blocked, _ok) = doctor::blocked_hosts(&filtered, |_, _, _| {});
        log::info(format!(
            "Проверка приложения: доменов {}, заблокировано {}, адресов {}",
            filtered.len(),
            blocked.len(),
            ips.len()
        ));
        if !blocked.is_empty() {
            let (message, _added, _path) = doctor::add_to_user_list(&install, &blocked);
            log::info(format!("Проверка приложения: {}", message));
        }
        return;
    }
    if has_flag(&args, "--guard-once") {
        let preferred = if root_arg.is_empty() {
            ini.get("Root", "")
        } else {
            root_arg.clone()
        };
        let install = ZapretInstall::detect(&preferred);
        guard::reset_cursor();
        let (events, error, scanned) = guard::new_events(None);
        let hosts: Vec<String> = events.into_iter().map(|e| e.host).collect();
        log::info(format!(
            "Сторож: просканировано событий {}, доменов {}{}",
            scanned,
            hosts.len(),
            if error.is_empty() {
                String::new()
            } else {
                format!(", ошибка: {}", error)
            }
        ));
        let auto_add = ini.get_bool("SiteGuardAutoAdd", false);
        let added = guard::process_hosts(&install, &hosts, auto_add, |message| {
            log::info(message);
        });
        log::info(format!("Сторож: добавлено доменов {}.", added.len()));
        return;
    }
    if has_flag(&args, "--tg-prepare") {
        log::error("Тихий режим TG WS Proxy в Rust-версии пока не поддерживается.");
        return;
    }

    // ---- Обычный запуск ------------------------------------------------
    let preferred = if root_arg.is_empty() {
        ini.get("Root", "")
    } else {
        root_arg.clone()
    };
    let install = ZapretInstall::detect(&preferred);
    if install.is_valid() && ini.get("Root", "") != install.root {
        ini.set("Root", &install.root);
        let _ = ini.save();
    }
    if let Some(host) = arg_value(&args, "--check-site") {
        let mut probe = doctor::check(&host, |step| log::info(step));
        log::info(format!(
            "Проверка сайта: {} — вердикт {}, DNS {}, TCP {}, TLS {}, HTTP {}",
            probe.host,
            probe.verdict.as_str(),
            probe.dns_ok,
            probe.tcp_ok,
            probe.tls_ok,
            probe.http_ok
        ));
        log::info(format!("Проверка сайта: шаги:\n{}", zapret_manager::ui::app::site_steps(&probe)));
        if probe.verdict != doctor::Verdict::Dns && probe.verdict != doctor::Verdict::Down {
            doctor::discover(&mut probe, |step| log::info(step));
            log::info(format!(
                "Проверка сайта: связанных адресов {} — {}",
                probe.related.len(),
                probe.related.join(", ")
            ));
        }
        return;
    }
    if has_flag(&args, "--diag") {
        let ops = ZapretOps::new(install.clone());
        let results = diagnostics::run(&install, &ops, |step| log::info(step));
        for r in &results {
            let level = match r.severity {
                diagnostics::Severity::Error => zapret_manager::engine::LogLevel::Error,
                diagnostics::Severity::Warn => zapret_manager::engine::LogLevel::Warn,
                diagnostics::Severity::Ok => zapret_manager::engine::LogLevel::Ok,
            };
            log::write(
                level,
                &format!(
                    "[{}] {}: {}{}",
                    r.severity.text(),
                    r.name,
                    r.message,
                    if r.hint.is_empty() {
                        String::new()
                    } else {
                        format!(" | {}", r.hint)
                    }
                ),
            );
        }
        return;
    }
    log::info(format!(
        "Папка zapret: {}{}",
        install.root,
        if install.is_valid() {
            format!(
                " (версия {})",
                if install.version.is_empty() {
                    "?"
                } else {
                    &install.version
                }
            )
        } else {
            " (не найдена)".to_string()
        }
    ));
    log::info(format!(
        "Права администратора: {}",
        if sys::is_admin() { "да" } else { "нет" }
    ));

    if let Some(ref page) = shot_page {
        log::info(format!("Снимок окна: страница {}", page));
    }

    let result = zapret_manager::ui::run(
        install,
        ini,
        shot,
        shot_page,
        tray,
        autostart,
        limited || !sys::is_admin(),
    );
    if let Err(e) = result {
        log::error(format!("Не удалось запустить интерфейс: {}", e));
        sys::message_box(
            &format!("Не удалось запустить интерфейс:\n\n{}", e),
            "Zapret Manager",
            sys::MsgKind::Error,
        );
    }
}

fn has_flag(args: &[String], flag: &str) -> bool {
    args.iter().any(|a| a.eq_ignore_ascii_case(flag))
}

fn arg_value(args: &[String], name: &str) -> Option<String> {
    let mut iter = args.iter().peekable();
    while let Some(arg) = iter.next() {
        if arg.eq_ignore_ascii_case(name) {
            return iter.peek().map(|v| v.trim_matches('"').to_string());
        }
        if let Some(rest) = arg
            .strip_prefix(&format!("{}=", name))
            .or_else(|| arg.strip_prefix(&format!("{}=", name.to_uppercase())))
        {
            return Some(rest.trim_matches('"').to_string());
        }
    }
    None
}
