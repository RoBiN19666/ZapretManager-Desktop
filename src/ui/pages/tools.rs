//! Страница «Инструменты».

use eframe::egui::{RichText, Ui};

use crate::engine::log::LogLevel;
use crate::engine::ops::ZapretOps;
use crate::engine::{backup, paths, sys, tester, tgproxy, uninstall};

use crate::ui::app::App;
use crate::ui::widgets as w;

impl App {
    pub(crate) fn page_tools(&mut self, ui: &mut Ui) {
        let p = self.p;

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
                        "Служба zapret будет остановлена и удалена, драйвер WinDivert освобождён. Ускорение перестанет работать, пока вы снова не запустите его вручную.",
                        "Удалить",
                        true,
                        |app| app.remove_service(),
                    );
                }
            });
            w::hint(
                ui,
                &p,
                "Служба запускает ускорение автоматически при загрузке Windows. Пока служба активна, ручной запуск стратегий недоступен.",
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
                            "Тест запущен в отдельном окне PowerShell. Он останавливает текущее ускорение и перебирает стратегии.",
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
            let tg_dir = if self.pages.tg.state.installed
                && !self.pages.tg.state.dir.as_os_str().is_empty()
            {
                self.pages.tg.state.dir.clone()
            } else {
                tgproxy::dir_or_default(&self.ini)
            };
            ui.horizontal_wrapped(|ui| {
                let folders = [
                    ("Папка zapret", Some(self.install.root.clone())),
                    ("bin", Some(self.install.bin_dir().to_string_lossy().to_string())),
                    ("lists", Some(self.install.lists_dir().to_string_lossy().to_string())),
                    ("utils", Some(self.install.utils_dir().to_string_lossy().to_string())),
                    (
                        "Папка TG WS Proxy",
                        Some(tg_dir.to_string_lossy().to_string()),
                    ),
                ];
                for (label, path) in folders {
                    if w::ghost(ui, &p, label).clicked() {
                        if let Some(path) = path.filter(|p| !p.is_empty()) {
                            let path = std::path::Path::new(&path);
                            if path.is_dir() {
                                sys::open_folder(path);
                            } else {
                                self.notify(
                                    format!("Папка не найдена: {}", path.to_string_lossy()),
                                    LogLevel::Warn,
                                );
                            }
                        }
                    }
                }
                if w::ghost(ui, &p, "Журнал менеджера").clicked() {
                    sys::select_in_explorer(&paths::log_path());
                }
            });
        });
        w::card(ui, &p, "Удаление", |ui| {
            w::small(
                ui,
                &p,
                "Можно удалить отдельные компоненты или всё сразу. Удаление необратимо; при необходимости сначала сделайте резервную копию.",
            );
            ui.add_space(6.0);
            let root = self.install.root.clone();
            let mut keep_lists = self.ini.get_bool("RemoveKeepLists", true);
            if w::toggle_row(
                ui,
                &p,
                "Сохранить списки при удалении zapret",
                "Папка lists будет скопирована в data\\removed-zapret-lists",
                &mut keep_lists,
            ) {
                self.ini.set_bool("RemoveKeepLists", keep_lists);
                let _ = self.ini.save();
            }
            ui.add_space(6.0);
            ui.horizontal_wrapped(|ui| {
                if w::secondary(ui, &p, "Открыть папку zapret").clicked() {
                    let path = std::path::Path::new(&root);
                    if path.is_dir() {
                        sys::open_folder(path);
                    } else {
                        self.notify("Папка zapret не найдена.", LogLevel::Warn);
                    }
                }
                if w::secondary(ui, &p, "Удалить zapret").clicked() {
                    let keep = keep_lists;
                    let root = root.clone();
                    self.confirm(
                        "Удалить zapret?",
                        &format!(
                            "Будут остановлены служба и winws.exe, удалены папка\n{}\nслужба zapret, WinDivert и записи из hosts.{}",
                            root,
                            if keep {
                                "\n\nСписки будут сохранены в data\\removed-zapret-lists."
                            } else {
                                ""
                            }
                        ),
                        "Удалить zapret",
                        true,
                        move |app| match uninstall::remove_zapret(&app.install, keep) {
                            Ok(message) => {
                                app.notify(message, LogLevel::Ok);
                                app.install = crate::engine::install::ZapretInstall::new("");
                                app.ops = ZapretOps::new(app.install.clone());
                                app.refresh_state(true);
                            }
                            Err(e) => app.notify(e, LogLevel::Error),
                        },
                    );
                }
                if w::secondary(ui, &p, "Удалить TG WS Proxy").clicked() {
                    self.confirm(
                        "Удалить TG WS Proxy?",
                        "Прокси будет остановлен, его папка, тихий режим, данные в %APPDATA%\\TgWsProxy и автозапуск будут удалены.",
                        "Удалить TG WS Proxy",
                        true,
                        |app| match uninstall::remove_tg(&mut app.ini) {
                            Ok(message) => {
                                app.notify(message, LogLevel::Ok);
                                app.refresh_tg_state();
                            }
                            Err(e) => app.notify(e, LogLevel::Error),
                        },
                    );
                }
                if w::danger(ui, &p, "Очистить данные менеджера").clicked() {
                    self.confirm(
                        "Очистить данные менеджера?",
                        "Будут удалены настройки, журнал, резервные копии и отчёты. Приложение закроется, чтобы освободить файлы.",
                        "Очистить данные",
                        true,
                        |app| match uninstall::clean_manager_data() {
                            Ok(message) => {
                                app.notify(message, LogLevel::Ok);
                                app.exit_requested = true;
                            }
                            Err(e) => app.notify(e, LogLevel::Error),
                        },
                    );
                }
                if w::danger(ui, &p, "Удалить всё").clicked() {
                    self.confirm(
                        "Удалить Zapret Manager полностью?",
                        "Будут удалены zapret, TG WS Proxy, службы, автозапуск, настройки, журнал, резервные копии, отчёты и сам ZapretManager.exe. Приложение закроется.",
                        "Удалить всё",
                        true,
                        |app| {
                            let result = uninstall::remove_everything(&app.install, &mut app.ini);
                            match result {
                                Ok(message) => {
                                    app.notify(message, LogLevel::Ok);
                                    app.exit_requested = true;
                                }
                                Err(e) => app.notify(e, LogLevel::Error),
                            }
                        },
                    );
                }
            });
        });

        w::card(ui, &p, "Резервная копия", |ui| {
            w::small(
                ui,
                &p,
                "Сохраняет настройки менеджера, ваши списки, targets.txt и игровой фильтр в один файл .zmb. Перед восстановлением автоматически создаётся копия текущего состояния.",
            );
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                if w::primary(ui, &p, "Создать копию…").clicked() {
                    let dir = paths::data_dir().join("backups");
                    let _ = std::fs::create_dir_all(&dir);
                    if let Some(path) = rfd::FileDialog::new()
                        .set_title("Сохранить резервную копию Zapret Manager")
                        .set_directory(&dir)
                        .set_file_name(backup::suggested_name())
                        .add_filter("Zapret Manager backup", &["zmb"])
                        .save_file()
                    {
                        let install = self.install.clone();
                        self.spawn_bg(move |tx| {
                            let result = backup::create(&install, &path)
                                .map(|b| {
                                    (
                                        false,
                                        format!(
                                            "Резервная копия создана: {} ({} файлов).",
                                            b.path.to_string_lossy(),
                                            b.files
                                        ),
                                    )
                                })
                                .map_err(|e| e.to_string());
                            let _ = tx.send(crate::ui::app::Msg::BackupDone(result));
                        });
                    }
                }
                if w::secondary(ui, &p, "Восстановить…").clicked() {
                    if let Some(path) = rfd::FileDialog::new()
                        .set_title("Выберите резервную копию Zapret Manager")
                        .set_directory(paths::data_dir().join("backups"))
                        .add_filter("Zapret Manager backup", &["zmb"])
                        .pick_file()
                    {
                        self.confirm(
                            "Восстановить резервную копию?",
                            "Текущие настройки менеджера, пользовательские списки, targets.txt и игровой фильтр будут заменены. Перед восстановлением создаётся автоматическая копия текущего состояния.",
                            "Восстановить",
                            true,
                            move |app| {
                                let install = app.install.clone();
                                let path = path.clone();
                                app.spawn_bg(move |tx| {
                                    let result = backup::restore(&install, &path)
                                        .map(|r| {
                                            (
                                                true,
                                                format!(
                                                    "Настройки восстановлены из {} ({} файлов).",
                                                    path.to_string_lossy(),
                                                    r.files
                                                ),
                                            )
                                        })
                                        .map_err(|e| e.to_string());
                                    let _ = tx.send(crate::ui::app::Msg::BackupDone(result));
                                });
                            },
                        );
                    }
                }
                if w::ghost(ui, &p, "Открыть папку копий").clicked() {
                    let dir = paths::data_dir().join("backups");
                    let _ = std::fs::create_dir_all(&dir);
                    sys::open_folder(&dir);
                }
            });
        });
    }
}
