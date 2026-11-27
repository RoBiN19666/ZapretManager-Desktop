//! Быстрые настройки, продублированные в `manager.ini`.
//!
//! Сами переключатели живут в папке zapret:
//!   * «Игровой фильтр» — `utils\game_filter.enabled` (режим и диапазоны портов);
//!   * «IP-фильтр (полный список)» — содержимое `lists\ipset-all.txt`;
//!   * «Авто-проверка обновлений zapret» — `utils\check_updates.enabled`.
//!
//! При установке zapret в новую папку эти файлы создаются заново, поэтому
//! последние значения дублируются в `manager.ini` и применяются после
//! установки — иначе пользователь видел бы сброшенные тумблеры.

use crate::util::is_port_range_valid;

use super::config::Ini;
use super::gamefilter::{GameFilterMode, GameFilterState};
use super::install::ZapretInstall;
use super::ops::{self, ZapretOps};
use super::textfile;

pub const KEY_GAME_FILTER: &str = "GameFilter";
pub const KEY_GAME_TCP: &str = "GameFilterTcp";
pub const KEY_GAME_UDP: &str = "GameFilterUdp";
pub const KEY_IPSET: &str = "IpsetMode";
pub const KEY_CHECK_UPDATES: &str = "ZapretCheckUpdates";

/// Запомнить состояние игрового фильтра (режим и диапазоны портов).
pub fn remember_game_filter(ini: &mut Ini, state: &GameFilterState) {
    ini.set(KEY_GAME_FILTER, state.mode.as_str());
    ini.set(KEY_GAME_TCP, &state.tcp_range);
    ini.set(KEY_GAME_UDP, &state.udp_range);
    let _ = ini.save();
}

/// Запомнить режим IP-фильтра (намерение пользователя: loaded / any / none).
pub fn remember_ipset(ini: &mut Ini, mode: &str) {
    ini.set(KEY_IPSET, mode);
    let _ = ini.save();
}

/// Запомнить флаг «Авто-проверка обновлений zapret».
pub fn remember_check_updates(ini: &mut Ini, enabled: bool) {
    ini.set_bool(KEY_CHECK_UPDATES, enabled);
    let _ = ini.save();
}

fn saved_bool(ini: &Ini, key: &str) -> Option<bool> {
    if ini.get(key, "").trim().is_empty() {
        None
    } else {
        Some(ini.get_bool(key, false))
    }
}

fn saved_port_range(ini: &Ini, key: &str) -> Option<String> {
    let value = ini.get(key, "").trim().to_string();
    if value.is_empty() || !is_port_range_valid(&value) {
        None
    } else {
        Some(value)
    }
}

/// Применить сохранённые значения к папке zapret (после установки с нуля).
///
/// Возвращает сообщения для журнала и уведомлений; пустой список — менять
/// было нечего.
pub fn apply_saved(install: &ZapretInstall, ini: &Ini) -> Vec<String> {
    let mut messages = Vec::new();
    if !install.is_valid() {
        return messages;
    }

    let saved_mode = ini.get(KEY_GAME_FILTER, "").trim().to_string();
    if !saved_mode.is_empty() {
        let mut gf = GameFilterState::load(&install.root);
        let mut changed = false;
        if gf.mode.as_str() != saved_mode {
            gf.mode = GameFilterMode::from_str(&saved_mode);
            changed = true;
        }
        if let Some(range) = saved_port_range(ini, KEY_GAME_TCP) {
            if gf.tcp_range != range {
                gf.tcp_range = range;
                changed = true;
            }
        }
        if let Some(range) = saved_port_range(ini, KEY_GAME_UDP) {
            if gf.udp_range != range {
                gf.udp_range = range;
                changed = true;
            }
        }
        if changed {
            match gf.save(&install.root) {
                Ok(()) => messages.push(format!("Игровой фильтр: {}.", gf.mode.label())),
                Err(e) => messages.push(format!("Игровой фильтр: не удалось сохранить ({e}).")),
            }
        }
    }

    if let Some(saved) = saved_bool(ini, KEY_CHECK_UPDATES) {
        let file = install.check_updates_file();
        if file.is_file() != saved {
            let result = if saved {
                textfile::write_text(&file, "ENABLED")
            } else {
                std::fs::remove_file(&file)
            };
            if result.is_ok() {
                messages.push(if saved {
                    "Авто-проверка обновлений zapret включена.".to_string()
                } else {
                    "Авто-проверка обновлений zapret выключена.".to_string()
                });
            }
        }
    }

    let saved_ipset = ini.get(KEY_IPSET, "").trim().to_lowercase();
    if matches!(saved_ipset.as_str(), "loaded" | "any" | "none") {
        let ops = ZapretOps::new(install.clone());
        if ops.get_ipset_mode() != saved_ipset {
            let message = ops.set_ipset_mode(&saved_ipset);
            if message.starts_with("Ошибка") || message.starts_with("Нет ") {
                messages.push(format!("IP-фильтр: {}", message));
            } else {
                messages.push(format!("IP-фильтр: {}.", ops::ipset_mode_text(&saved_ipset)));
            }
        }
    }

    messages
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_root(tag: &str) -> std::path::PathBuf {
        let root = std::env::temp_dir().join(format!(
            "zapret-prefs-{}-{}",
            tag,
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("lists")).unwrap();
        std::fs::create_dir_all(root.join("utils")).unwrap();
        std::fs::create_dir_all(root.join("bin")).unwrap();
        // Минимальный «валидный» zapret: is_valid проверяет эти два файла.
        std::fs::write(root.join("service.bat"), "@echo off\r\n").unwrap();
        std::fs::write(root.join("bin").join("winws.exe"), b"MZ").unwrap();
        root
    }

    fn install_at(root: &std::path::Path) -> ZapretInstall {
        ZapretInstall {
            root: root.to_string_lossy().to_string(),
            ..Default::default()
        }
    }

    #[test]
    fn applies_game_filter_and_flag_to_new_folder() {
        let root = temp_root("apply");
        let install = install_at(&root);
        let ini_path = root.join("manager.ini");
        let mut ini = Ini::load(&ini_path);
        remember_game_filter(
            &mut ini,
            &GameFilterState {
                mode: GameFilterMode::Tcp,
                tcp_range: "2000-3000".to_string(),
                udp_range: "1024-65535".to_string(),
            },
        );
        remember_check_updates(&mut ini, true);

        let messages = apply_saved(&install, &ini);
        assert_eq!(messages.len(), 2, "{messages:?}");

        let gf = GameFilterState::load(&install.root);
        assert_eq!(gf.mode, GameFilterMode::Tcp);
        assert_eq!(gf.tcp_range, "2000-3000");
        assert!(install.check_updates_file().is_file());

        // Повторный запуск ничего не меняет.
        let again = apply_saved(&install, &ini);
        assert!(again.is_empty(), "{again:?}");
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn switches_game_filter_off() {
        let root = temp_root("off");
        let install = install_at(&root);
        let mut gf = GameFilterState::load(&install.root);
        gf.mode = GameFilterMode::Tcp;
        gf.save(&install.root).unwrap();

        let ini_path = root.join("manager.ini");
        let mut ini = Ini::load(&ini_path);
        remember_game_filter(
            &mut ini,
            &GameFilterState {
                mode: GameFilterMode::Disabled,
                ..gf.clone()
            },
        );

        let messages = apply_saved(&install, &ini);
        assert_eq!(messages.len(), 1, "{messages:?}");
        assert_eq!(
            GameFilterState::load(&install.root).mode,
            GameFilterMode::Disabled
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn ignores_broken_port_range() {
        let root = temp_root("badrange");
        let install = install_at(&root);
        let ini_path = root.join("manager.ini");
        let mut ini = Ini::load(&ini_path);
        ini.set(KEY_GAME_FILTER, "all");
        ini.set(KEY_GAME_TCP, "999-1-999");
        let messages = apply_saved(&install, &ini);
        assert_eq!(messages.len(), 1, "{messages:?}");
        let gf = GameFilterState::load(&install.root);
        assert_eq!(gf.mode, GameFilterMode::All);
        assert_eq!(gf.tcp_range, "1024-65535");
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn without_saved_values_nothing_changes() {
        let root = temp_root("empty");
        let install = install_at(&root);
        let ini = Ini::load(&root.join("manager.ini"));
        assert!(apply_saved(&install, &ini).is_empty());
        assert!(!install.check_updates_file().is_file());
        let _ = std::fs::remove_dir_all(&root);
    }
}
