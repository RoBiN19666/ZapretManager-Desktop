//! Самопроверка: быстрый headless-прогон ключевых частей приложения.

use std::path::{Path, PathBuf};

use crate::util::{sha256_bytes, hex_lower, version_cmp};

use super::install::ZapretInstall;
use super::{bat, config::Ini, gamefilter::{GameFilterMode, GameFilterState}, lists, service, textfile};

#[derive(Debug, Clone)]
pub struct Check {
    pub name: String,
    pub ok: bool,
    pub detail: String,
}

fn temp_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "zapret-selftest-{}-{}",
        tag,
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    let _ = std::fs::create_dir_all(&dir);
    dir
}

/// Все проверки; текст результата пишется в журнал вызывающей стороной.
pub fn run(install: &ZapretInstall) -> Vec<Check> {
    let mut checks = Vec::new();
    let mut add = |name: &str, ok: bool, detail: String| {
        checks.push(Check {
            name: name.to_string(),
            ok,
            detail,
        });
    };

    // Версии
    let versions_ok = version_cmp("1.10.4", "1.9.9").is_gt()
        && version_cmp("1.4.0-pre1", "1.4.0").is_lt()
        && version_cmp("1.4.0", "1.4.0.0").is_eq();
    add("Сравнение версий", versions_ok, "1.10.4 > 1.9.9, pre < release".to_string());

    // SHA-256
    let sha = hex_lower(&sha256_bytes(b"abc"));
    add(
        "SHA-256",
        sha == "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad",
        sha,
    );

    // Конфиг
    let dir = temp_dir("config");
    let path = dir.join("manager.ini");
    let mut ini = Ini::load(&path);
    ini.set("Root", r"C:\zapret");
    ini.set_bool("CloseToTray", true);
    ini.set_int("UiScale", 110);
    let saved = ini.save().is_ok();
    let loaded = Ini::load(&path);
    let config_ok = saved
        && loaded.get("Root", "") == r"C:\zapret"
        && loaded.get_bool("CloseToTray", false)
        && loaded.get_int("UiScale", 0) == 110;
    add("manager.ini", config_ok, path.to_string_lossy().to_string());
    let _ = std::fs::remove_dir_all(&dir);

    // Игровой фильтр
    let dir = temp_dir("gf");
    let gf_path = dir.join("utils").join("game_filter.enabled");
    let mut gf = GameFilterState {
        mode: GameFilterMode::Tcp,
        tcp_range: "1024-2048,3000-4000".to_string(),
        udp_range: "5000-6000".to_string(),
    };
    let saved = gf.save(&dir.to_string_lossy()).is_ok();
    let mut loaded = GameFilterState::load(&dir.to_string_lossy());
    let gf_ok = saved
        && loaded.mode == GameFilterMode::Tcp
        && loaded.tcp_range == "1024-2048,3000-4000"
        && loaded.tcp_token() == "1024-2048,3000-4000"
        && loaded.udp_token() == "12";
    add(
        "Игровой фильтр",
        gf_ok,
        format!("{} / {}", loaded.mode.as_str(), loaded.tcp_range),
    );
    let _ = gf_path;
    let _ = std::fs::remove_dir_all(&dir);

    // Стратегии и .bat-парсер
    if install.is_valid() {
        let mut total = 0usize;
        let mut failed = Vec::new();
        let gf = GameFilterState::load(&install.root);
        for strategy in &install.strategies {
            total += 1;
            match bat::get_argv(install, strategy, &gf) {
                Ok(argv) if !argv.is_empty() => {}
                Ok(_) => failed.push(format!("{}: пустые аргументы", strategy.name)),
                Err(e) => failed.push(format!("{}: {}", strategy.name, e)),
            }
        }
        add(
            "Стратегии (.bat → argv)",
            total > 0 && failed.is_empty(),
            if failed.is_empty() {
                format!("разобрано {}/{}", total, total)
            } else {
                failed.join("; ")
            },
        );
        add(
            "Установка zapret",
            true,
            format!(
                "{} (версия {})",
                install.root,
                if install.version.is_empty() { "?" } else { &install.version }
            ),
        );
    } else {
        add(
            "Стратегии (.bat → argv)",
            false,
            "папка zapret не найдена — укажите --root".to_string(),
        );
    }

    // Списки: baseline + сохранение из редактора
    let dir = temp_dir("lists");
    let data = dir.join("data");
    let file = dir.join("list-general.txt");
    let _ = std::fs::create_dir_all(&data);
    let _ = textfile::write_text(
        &file,
        "# official\na.com\nb.com\n",
    );
    let mut lc = lists::ListChanges::load(&file, &data);
    let baseline_ok = lc.baseline.len() == 2 && lc.added.is_empty();
    let editor = "# official\na.com\nb.com\nmy.example\n";
    let saved = lc.save_from_editor(editor).is_some();
    let reloaded = lists::ListChanges::load(&file, &data);
    let text = textfile::read(&file);
    let lists_ok = baseline_ok
        && saved
        && reloaded
            .added
            .iter()
            .any(|e| e.eq_ignore_ascii_case("my.example"))
        && text.contains(lists::BLOCK_START)
        && text.contains("my.example");
    add(
        "Списки: baseline и блок добавлений",
        lists_ok,
        format!(
            "baseline {}, добавлено {}, файл {} байт",
            reloaded.baseline.len(),
            reloaded.added.len(),
            text.len()
        ),
    );

    // Официальные списки: применение после обновления
    let changed = lists::apply_after_update(&dir, &data);
    let reloaded = lists::ListChanges::load(&file, &data);
    add(
        "Списки: перенос после обновления",
        reloaded.added.iter().any(|e| e.eq_ignore_ascii_case("my.example")),
        format!("сообщений {}", changed.len()),
    );
    let _ = std::fs::remove_dir_all(&dir);

    // Службы
    let bfe = service::query("BFE");
    add(
        "Service Control Manager",
        bfe.installed,
        format!("BFE: {}", bfe.status),
    );

    // hosts
    let hosts = super::ops::hosts_path();
    let hosts_ok = Path::new(&hosts).is_file() && !textfile::read(&hosts).is_empty();
    add(
        "Файл hosts",
        hosts_ok,
        hosts.to_string_lossy().to_string(),
    );

    // TG-прокси
    let tg_ini = Ini::load(&super::paths::config_path());
    let tg = super::tgproxy::get_state(&tg_ini);
    add(
        "TG WS Proxy",
        true,
        format!(
            "{}:{}; установлен: {}; запущен: {}",
            tg.host, tg.port, tg.installed, tg.running
        ),
    );

    checks
}
