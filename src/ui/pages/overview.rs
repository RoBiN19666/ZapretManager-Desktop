//! Страница «Обзор».

use eframe::egui::{self, RichText};
use eframe::egui::Ui;

use crate::engine::gamefilter::{GameFilterMode, GameFilterState};
use crate::engine::log::{self, LogLevel};
use crate::engine::ops;
use crate::engine::tgproxy;
use crate::engine::{paths, sys};

use crate::ui::app::{App, Page};
use crate::ui::widgets as w;

impl App {
    pub(crate) fn page_overview(&mut self, ui: &mut Ui) {
        let p = self.p;
        let valid = self.install.is_valid();
        let running = self.state.winws_running;

        // предупреждение о службе из другой папки
        if valid && (self.ops.service_points_elsewhere() || self.ops.service_path_missing()) {
            let (title, text) = if self.ops.service_path_missing() {
                (
                    "Внимание: служба zapret указывает на несуществующую папку",
                    format!(
                        "Служба установлена из папки «{}», которой больше нет (папку переместили или удалили). После перезагрузки ускорение не запустится. Переустановите службу из текущей папки.",
                        self.ops.service_install_root().unwrap_or_else(|| "неизвестно".to_string())
                    ),
                )
            } else {
                (
                    "Внимание: служба zapret установлена из другой папки",
                    format!(
                        "Служба zapret сейчас запущена из папки «{}», а менеджер управляет папкой «{}». Запуск стратегий и обновление затронут только вторую. Переустановите службу, чтобы переключиться.",
                        self.ops.service_install_root().unwrap_or_default(),
                        self.install.root
                    ),
                )
            };
            let p2 = p;
            w::card(ui, &p2, "", |ui| {
                ui.label(
                    RichText::new(title)
                        .size(14.0)
                        .strong()
                        .color(p2.warn),
                );
                ui.add_space(6.0);
                ui.label(RichText::new(&text).size(12.5).color(p2.text_dim));
                ui.add_space(10.0);
                ui.horizontal(|ui| {
                    if w::secondary(ui, &p2, "Переустановить службу из этой папки").clicked() {
                        if let Some(s) = self.pick_strategy() {
                            self.confirm(
                                "Переустановить службу?",
                                &format!(
                                    "Служба zapret будет пересоздана из папки «{}» со стратегией «{}».",
                                    self.install.root, s.name
                                ),
                                "Переустановить",
                                false,
                                move |app| app.install_service(s.clone()),
                            );
                        }
                    }
                    if w::ghost(ui, &p2, "Открыть папку службы").clicked() {
                        if let Some(root) = self.ops.service_install_root() {
                            sys::open_folder(std::path::Path::new(&root));
                        }
                    }
                });
            });
        }

        if !valid {
            let p2 = p;
            w::card(ui, &p2, "Zapret не найден", |ui| {
                ui.label(
                    RichText::new(
                        "Укажите папку zapret в одном месте — «Настройки» → «Папка zapret». Там же можно скачать и установить zapret с нуля.",
                    )
                    .size(12.5)
                    .color(p2.text_dim),
                );
                ui.add_space(12.0);
                ui.horizontal(|ui| {
                    if w::primary(ui, &p2, "Открыть настройки папки").clicked() {
                        self.page = Page::Settings;
                        self.on_navigate(Page::Settings);
                    }
                });
            });
            return;
        }

        // состояние защиты
        let p2 = p;
        w::card(ui, &p2, "Состояние защиты", |ui| {
            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    ui.label(
                        RichText::new(if running {
                            "Защита активна"
                        } else {
                            "Защита не активна"
                        })
                        .font(egui::FontId::new(19.0, egui::FontFamily::Name("bold".into())))
                        .color(if running { p2.success } else { p2.text_muted }),
                    );
                    ui.add_space(4.0);
                    if running {
                        let name = if self.state.matched_strategy.is_empty() {
                            "пользовательская"
                        } else {
                            &self.state.matched_strategy
                        };
                        let uptime = self
                            .state
                            .winws_started_unix
                            .and_then(|started| crate::util::unix_now().checked_sub(started))
                            .map(uptime_text)
                            .unwrap_or_default();
                        let mut line = format!(
                            "Стратегия: {}   •   PID {}",
                            name, self.state.winws_pid
                        );
                        if !uptime.is_empty() {
                            line.push_str(&format!("   •   работает {}", uptime));
                        }
                        ui.label(RichText::new(line).size(13.0).color(p2.text));
                    } else {
                        ui.label(
                            RichText::new(
                                "Ускорение не запущено. Выберите стратегию на странице «Стратегии».",
                            )
                            .size(13.0)
                            .color(p2.text_dim),
                        );
                    }
                    let gf = &self.state.game_filter;
                    let ipset = match self.state.ipset_mode.as_str() {
                        "loaded" => format!("полный список ({} строк)", self.state.ipset_lines),
                        "any" => "любые IP".to_string(),
                        _ => "без фильтра".to_string(),
                    };
                    ui.label(
                        RichText::new(format!(
                            "Служба zapret: {}   •   IPSet: {}   •   Игровой фильтр: {}   •   Автообновление: {}",
                            if self.state.service_installed {
                                self.state.service_status.clone()
                            } else {
                                "не установлена".to_string()
                            },
                            ipset,
                            gf.mode_text(),
                            if self.state.check_updates { "вкл." } else { "выкл." }
                        ))
                        .size(12.0)
                        .color(p2.text_dim),
                    );
                });
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let main_text = if running { "Остановить" } else { "Запустить" };
                    let resp = if running {
                        w::danger(ui, &p2, main_text)
                    } else {
                        w::primary(ui, &p2, main_text)
                    };
                    if resp.clicked() {
                        if running {
                            self.stop_bypass();
                        } else if let Some(s) = self.pick_strategy() {
                            self.start_strategy(s);
                        } else {
                            self.notify("Стратегии не найдены в папке zapret.", LogLevel::Error);
                        }
                    }
                    if w::secondary(ui, &p2, "Перезапустить").clicked() {
                        if let Some(s) = self.pick_strategy() {
                            self.restart_bypass(s);
                        } else {
                            self.notify("Стратегии не найдены.", LogLevel::Error);
                        }
                    }
                });
            });
        });

        // TG-прокси
        let p2 = p;
        let tg = self.pages.tg.state.clone();
        w::card(ui, &p2, "Telegram-прокси (TG WS Proxy)", |ui| {
            let (title, line, button) = if !tg.installed {
                (
                    "TG WS Proxy не установлен",
                    "Установите прокси Telegram на странице «TG-прокси» — 21 МБ, затем подключение одной кнопкой".to_string(),
                    "Установить",
                )
            } else if tg.running {
                (
                    "Прокси Telegram работает",
                    format!(
                        "{}:{}   •   версия {}   •   Telegram подключается автоматически",
                        tg.host,
                        tg.port,
                        if tg.version.is_empty() { "?" } else { &tg.version }
                    ),
                    "Остановить",
                )
            } else {
                (
                    "Прокси Telegram остановлен",
                    format!(
                        "Версия {}   •   {}",
                        if tg.version.is_empty() { "неизвестна" } else { &tg.version },
                        tg.dir.to_string_lossy()
                    ),
                    "Запустить",
                )
            };
            ui.label(RichText::new(title).size(13.5).color(p2.text).strong());
            ui.label(RichText::new(line).size(12.0).color(p2.text_dim));
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                if w::secondary(ui, &p2, button).clicked() {
                    if !tg.installed {
                        self.page = Page::TgProxy;
                        self.notify(
                            "TG WS Proxy не установлен — установите его на странице «TG-прокси».",
                            LogLevel::Warn,
                        );
                    } else if tg.running {
                        let dir = tg.dir.clone();
                        self.spawn_bg(move |tx| {
                            let message = tgproxy::stop(&dir);
                            let _ = tx.send(crate::ui::app::Msg::Notify(message, LogLevel::Info));
                        });
                    } else {
                        let ini = self.ini.clone();
                        self.spawn_bg(move |tx| {
                            let (message, ok) = tgproxy::start(&ini);
                            let _ = tx.send(crate::ui::app::Msg::Notify(
                                message,
                                if ok { LogLevel::Ok } else { LogLevel::Error },
                            ));
                        });
                    }
                }
                if w::ghost(ui, &p2, "Подробнее").clicked() {
                    self.page = Page::TgProxy;
                }
            });
        });

        // быстрые настройки
        let p2 = p;
        let mut gf = GameFilterState::load(&self.install.root);
        let mut gf_on = gf.mode.is_on();
        let ipset_on = self.state.ipset_mode == "loaded";
        let mut auto_check = self.state.check_updates;
        w::card(ui, &p2, "Быстрые настройки", |ui| {
            if w::toggle_row(
                ui,
                &p2,
                "Игровой фильтр",
                "Расширить ускорение на игровые порты (TCP и UDP)",
                &mut gf_on,
            ) {
                gf.mode = if gf_on {
                    GameFilterMode::All
                } else {
                    GameFilterMode::Disabled
                };
                match gf.save(&self.install.root) {
                    Ok(()) => {
                        self.notify(
                            if gf_on {
                                "Игровой фильтр включён (TCP и UDP)."
                            } else {
                                "Игровой фильтр выключен."
                            },
                            LogLevel::Ok,
                        );
                        self.restart_after_change("Игровой фильтр обновлён");
                    }
                    Err(e) => self.notify(
                        format!("Не удалось сохранить настройку: {}", e),
                        LogLevel::Error,
                    ),
                }
                self.refresh_state(false);
            }
            let mut ipset_checked = ipset_on;
            if w::toggle_row(
                ui,
                &p2,
                "IP-фильтр (полный список)",
                "Ускорение применяется ко всем IP из полного списка",
                &mut ipset_checked,
            ) {
                let mode = if ipset_checked { "loaded" } else { "none" };
                self.set_ipset_mode_auto(mode);
            }
            if w::toggle_row(
                ui,
                &p2,
                "Авто-проверка обновлений zapret",
                "Флаг utils\\check_updates.enabled для самой сборки zapret",
                &mut auto_check,
            ) {
                let file = self.install.check_updates_file();
                let result = if auto_check {
                    crate::engine::textfile::write_text(&file, "ENABLED")
                } else if file.exists() {
                    std::fs::remove_file(&file).map_err(std::io::Error::from)
                } else {
                    Ok(())
                };
                match result {
                    Ok(()) => self.notify(
                        if auto_check {
                            "Авто-проверка обновлений включена."
                        } else {
                            "Авто-проверка обновлений выключена."
                        },
                        LogLevel::Ok,
                    ),
                    Err(e) => self.notify(
                        format!("Не удалось изменить флаг: {}", e),
                        LogLevel::Error,
                    ),
                }
                self.refresh_state(false);
            }
        });

        w::card(ui, &p, "Ресурсы", |ui| {
            if self.resource_checked.elapsed() > std::time::Duration::from_secs(5) {
                self.winws_mem = if self.state.winws_running {
                    sys::process_memory(self.state.winws_pid)
                } else {
                    None
                };
                self.tg_mem = if self.pages.tg.state.running {
                    sys::process_memory(self.pages.tg.state.pid)
                } else {
                    None
                };
                self.resource_checked = std::time::Instant::now();
            }
            ui.horizontal_wrapped(|ui| {
                let winws = if self.state.winws_running {
                    format!(
                        "winws.exe  PID {}  •  {}  •  до {}",
                        self.state.winws_pid,
                        fmt_mem(self.winws_mem),
                        self.state
                            .winws_started_unix
                            .map(|t| uptime_text(crate::util::unix_now().saturating_sub(t)))
                            .unwrap_or_else(|| "—".to_string())
                    )
                } else {
                    "winws.exe: не запущен".to_string()
                };
                ui.label(RichText::new(winws).size(12.0).color(p.text_dim));
                ui.label(RichText::new("•").size(12.0).color(p.text_muted));
                let tg = if self.pages.tg.state.running {
                    format!(
                        "TG WS Proxy  PID {}  •  {}",
                        self.pages.tg.state.pid,
                        fmt_mem(self.tg_mem)
                    )
                } else {
                    "TG WS Proxy: не запущен".to_string()
                };
                ui.label(RichText::new(tg).size(12.0).color(p.text_dim));
            });
        });

        // журнал
        let p2 = p;
        w::card(ui, &p2, "Журнал", |ui| {
            let new_lines = log::since(self.log_seq);
            for line in new_lines {
                self.log_seq = line.seq;
                self.log_lines.push(line);
            }
            if self.log_lines.len() > 2000 {
                let drain = self.log_lines.len() - 1500;
                self.log_lines.drain(0..drain);
            }
            egui::Frame::none()
                .fill(p2.editor)
                .stroke(egui::Stroke::new(1.0, p2.border))
                .rounding(egui::Rounding::same(6))
                .inner_margin(egui::Margin::same(8))
                .show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    egui::ScrollArea::vertical()
                        .max_height(180.0)
                        .auto_shrink([false, false])
                        .stick_to_bottom(true)
                        .show(ui, |ui| {
                            for line in &self.log_lines {
                                let color = match line.level {
                                    LogLevel::Ok => p2.success,
                                    LogLevel::Warn => p2.warn,
                                    LogLevel::Error => p2.danger,
                                    LogLevel::Info => p2.text_dim,
                                };
                                ui.label(
                                    RichText::new(crate::engine::log::format_line(line))
                                        .font(egui::FontId::new(11.5, egui::FontFamily::Monospace))
                                        .color(color),
                                );
                            }
                        });
                });
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                if w::ghost(ui, &p2, "Открыть журнал").clicked() {
                    sys::select_in_explorer(&paths::log_path());
                }
                if w::ghost(ui, &p2, "Очистить вид").clicked() {
                    self.log_lines.clear();
                }
                ui.label(
                    RichText::new(format!("{}", paths::log_path().to_string_lossy()))
                        .size(11.0)
                        .color(p2.text_muted),
                );
            });
        });

        let _ = ops::ipset_mode_text("loaded");
    }
}

fn fmt_mem(bytes: Option<u64>) -> String {
    match bytes {
        Some(b) if b >= 1024 * 1024 => format!("{:.1} МБ", b as f64 / (1024.0 * 1024.0)),
        Some(b) if b >= 1024 => format!("{:.0} КБ", b as f64 / 1024.0),
        Some(b) => format!("{} Б", b),
        None => "память н/д".to_string(),
    }
}

fn uptime_text(seconds: u64) -> String {
    if seconds < 60 {
        "меньше минуты".to_string()
    } else if seconds < 3600 {
        format!("{} мин", seconds / 60)
    } else if seconds < 86400 {
        format!("{} ч {} мин", seconds / 3600, (seconds % 3600) / 60)
    } else {
        format!("{} д {} ч", seconds / 86400, (seconds % 86400) / 3600)
    }
}

