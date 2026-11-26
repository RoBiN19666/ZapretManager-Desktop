//! Страница «Инструменты».

use eframe::egui::{self, RichText, Ui};

use crate::engine::log::LogLevel;
use crate::engine::ops::ZapretOps;
use crate::engine::{doctor, paths, sys, tester};

use crate::ui::app::App;
use crate::ui::widgets as w;

impl App {
    pub(crate) fn page_tools(&mut self, ui: &mut Ui) {
        let p = self.p;
        let valid = self.install.is_valid();

        w::card(ui, &p, "Служба Windows", |ui| {
            let mut status = format!(
                "Служба zapret: {}",
                if self.state.service_installed {
                    self.state.service_status.clone()
                } else {
                    "не установлена".to_string()
                }
            );
            if !self.state.service_strategy.is_empty() {
                status.push_str(&format!(
                    "  •  стратегия из реестра: {}",
                    self.state.service_strategy
                ));
            }
            ui.label(RichText::new(status).size(13.0).color(p.text));
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                if w::secondary(ui, &p, "Установить службу").clicked() {
                    if let Some(s) = self.pick_strategy() {
                        self.confirm(
                            "Установить службу?",
                            &format!(
                                "Стратегия «{}» будет установлена как служба «zapret» с автозапуском.",
                                s.name
                            ),
                            "Установить",
                            false,
                            move |app| app.install_service(s.clone()),
                        );
                    } else {
                        self.notify("Стратегии не найдены.", LogLevel::Error);
                    }
                }
                if self.state.service_installed && w::danger(ui, &p, "Удалить службу").clicked() {
                    self.confirm(
                        "Удалить службу?",
                        "Служба zapret будет остановлена и удалена, драйвер WinDivert освобождён. Bypass перестанет работать, пока вы снова не запустите его вручную.",
                        "Удалить",
                        true,
                        |app| app.remove_service(),
                    );
                }
            });
            w::hint(
                ui,
                &p,
                "Служба запускает bypass автоматически при загрузке Windows. Пока служба активна, ручной запуск стратегий недоступен.",
            );
        });

        w::card(ui, &p, "Файл hosts", |ui| {
            let message = if self.pages.tools.hosts_message.is_empty() {
                "Нажмите «Проверить», чтобы сравнить hosts с официальным списком.".to_string()
            } else {
                self.pages.tools.hosts_message.clone()
            };
            ui.label(RichText::new(message).size(12.5).color(p.text_dim));
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                if w::secondary(ui, &p, "Проверить").clicked() {
                    let ops = ZapretOps::new(self.install.clone());
                    self.spawn_bg(move |tx| {
                        let status = ops.check_hosts();
                        let _ = tx.send(crate::ui::app::Msg::HostsChecked(status));
                    });
                }
                if w::secondary(ui, &p, "Применить список").clicked() {
                    if self.require_admin("Обновление hosts") {
                        let ops = ZapretOps::new(self.install.clone());
                        self.spawn_bg(move |tx| {
                            let message = ops.apply_hosts();
                            let _ = tx.send(crate::ui::app::Msg::Notify(message, LogLevel::Ok));
                        });
                    }
                }
                if w::ghost(ui, &p, "Убрать блок менеджера").clicked() {
                    if self.require_admin("Очистка hosts") {
                        let ops = ZapretOps::new(self.install.clone());
                        self.spawn_bg(move |tx| {
                            let message = ops.clear_hosts_block();
                            let _ = tx.send(crate::ui::app::Msg::Notify(message, LogLevel::Ok));
                        });
                    }
                }
            });
        });

        w::card(ui, &p, "Проверка работоспособности", |ui| {
            w::small(
                ui,
                &p,
                "Официальный скрипт авторов перебирает стратегии в отдельном окне PowerShell и сохраняет отчёт в utils\\test results.",
            );
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                if w::primary(ui, &p, "Запустить официальный тест").clicked() {
                    match tester::run_official_test(&self.install) {
                        Ok(()) => self.notify(
                            "Тест запущен в отдельном окне PowerShell. Он останавливает текущий bypass и перебирает стратегии.",
                            LogLevel::Ok,
                        ),
                        Err(e) => self.notify(e, LogLevel::Error),
                    }
                }
                if w::ghost(ui, &p, "Папка отчётов").clicked() {
                    sys::open_folder(&tester::reports_dir(&self.install));
                }
            });
        });

        w::card(ui, &p, "Кэш Discord", |ui| {
            let dirs = crate::engine::ops::discord_cache_dirs();
            if dirs.is_empty() {
                ui.label(
                    RichText::new("Установки Discord не найдены.")
                        .size(12.5)
                        .color(p.text_dim),
                );
            } else {
                let list: Vec<String> = dirs.iter().map(|d| d.to_string_lossy().to_string()).collect();
                ui.label(
                    RichText::new(format!("Найдено: {}", list.join(", ")))
                        .size(12.0)
                        .color(p.text_dim),
                );
            }
            ui.add_space(8.0);
            if w::secondary(ui, &p, "Очистить кэш Discord").clicked() {
                self.confirm(
                    "Очистить кэш Discord?",
                    "Discord будет закрыт, папки Cache, Code Cache и GPUCache будут удалены.",
                    "Очистить",
                    true,
                    |app| {
                        let ops = ZapretOps::new(app.install.clone());
                        app.spawn_bg(move |tx| {
                            let message = ops.clear_discord_cache();
                            let _ = tx.send(crate::ui::app::Msg::Notify(message, LogLevel::Ok));
                        });
                    },
                );
            }
            w::hint(
                ui,
                &p,
                "Помогает, если Discord зависает на «Загрузка…» или не подключается к голосовому каналу.",
            );
        });

        w::card(ui, &p, "Папки и файлы", |ui| {
            ui.horizontal_wrapped(|ui| {
                let folders = [
                    ("Папка zapret", Some(self.install.root.clone())),
                    ("bin", Some(self.install.bin_dir().to_string_lossy().to_string())),
                    ("lists", Some(self.install.lists_dir().to_string_lossy().to_string())),
                    ("utils", Some(self.install.utils_dir().to_string_lossy().to_string())),
                    ("Папка TG WS Proxy", Some(self.ini.get("TgDir", ""))),
                ];
                for (label, path) in folders {
                    if w::ghost(ui, &p, label).clicked() {
                        if let Some(path) = path.filter(|p| !p.is_empty()) {
                            sys::open_folder(std::path::Path::new(&path));
                        }
                    }
                }
                if w::ghost(ui, &p, "Журнал менеджера").clicked() {
                    sys::select_in_explorer(&paths::log_path());
                }
                if w::ghost(ui, &p, "Мастер установки").clicked() {
                    self.open_wizard();
                }
            });
        });

        w::card(ui, &p, "О программе", |ui| {
            ui.label(
                RichText::new(format!(
                    "Zapret Manager {} — неофициальная графическая оболочка.",
                    crate::APP_VERSION
                ))
                .size(13.0)
                .color(p.text),
            );
            ui.label(
                RichText::new(
                    "Обход DPI — проект zapret (bol-van), сборка и стратегии — zapret-discord-youtube (Flowseal), прокси Telegram — TG WS Proxy (Flowseal). Сам обход выполняют winws.exe и драйвер WinDivert из состава zapret.",
                )
                .size(12.0)
                .color(p.text_dim),
            );
            ui.add_space(8.0);
            ui.horizontal_wrapped(|ui| {
                let links = [
                    (
                        "Репозиторий",
                        "https://github.com/RoBiN19666/ZapretManager-Desktop",
                    ),
                    (
                        "Руководство",
                        "https://github.com/RoBiN19666/ZapretManager-Desktop/blob/main/docs/guide.md",
                    ),
                    (
                        "zapret-discord-youtube",
                        "https://github.com/Flowseal/zapret-discord-youtube",
                    ),
                    ("zapret (bol-van)", "https://github.com/bol-van/zapret"),
                    ("TG WS Proxy", "https://github.com/Flowseal/tg-ws-proxy"),
                ];
                for (label, url) in links {
                    if w::link(ui, &p, label).clicked() {
                        sys::open_url(url);
                    }
                }
            });
            ui.add_space(8.0);
            ui.label(RichText::new("Поддержать проект").size(12.5).color(p.text_dim));
            ui.horizontal(|ui| {
                if w::link(ui, &p, "Boosty").clicked() {
                    sys::open_url("https://boosty.to/robin19666");
                }
                if w::link(ui, &p, "DonationAlerts").clicked() {
                    sys::open_url("https://www.donationalerts.com/r/robin19666");
                }
            });
            ui.add_space(6.0);
            w::hint(
                ui,
                &p,
                &format!(
                    "Настройки и журнал: {}",
                    paths::data_dir().to_string_lossy()
                ),
            );
            let _ = doctor::hosts_file_exists();
            let _ = valid;
        });
    }
}
