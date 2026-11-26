//! Страница «TG-прокси».

use eframe::egui::{self, RichText, Ui};

use crate::engine::log::LogLevel;
use crate::engine::tgproxy::{self, TgState};
use crate::engine::tgsource;

use crate::ui::app::{App, Msg};
use crate::ui::widgets as w;

impl App {
    pub(crate) fn refresh_tg_state(&mut self) {
        self.pages.tg.state = tgproxy::get_state(&self.ini);
        self.pages.tg.quiet_prepared = tgsource::is_prepared();
        self.last_tg_refresh = std::time::Instant::now();
    }

    pub(crate) fn page_tgproxy(&mut self, ui: &mut Ui) {
        let p = self.p;
        let st: TgState = self.pages.tg.state.clone();
        let link = st.link();

        w::card(ui, &p, "Состояние", |ui| {
            let title = if !st.installed {
                "Не установлен"
            } else if st.running && st.port_listening {
                "Прокси работает"
            } else if st.running {
                "Запускается…"
            } else {
                "Остановлен"
            };
            ui.label(
                RichText::new(title)
                    .font(egui::FontId::new(18.0, egui::FontFamily::Name("bold".into())))
                    .color(if st.running && st.port_listening {
                        p.success
                    } else {
                        p.text_muted
                    }),
            );
            if st.installed {
                let mut line = format!("{}:{}", st.host, st.port);
                if st.running {
                    line.push_str(&format!("   •   PID {}", st.pid));
                    if let Some(started) = st.started_unix {
                        if let Some(secs) = crate::util::unix_now().checked_sub(started) {
                            line.push_str(&format!(
                                "   •   запущен {} назад",
                                crate::util::format_uptime(secs)
                            ));
                        }
                    }
                }
                ui.label(RichText::new(line).size(12.5).color(p.text));
                let mut line2 = format!(
                    "Версия {}   •   секрет {}",
                    if st.version.is_empty() { "?" } else { &st.version },
                    if st.secret.is_empty() {
                        "ещё не создан"
                    } else {
                        "получен"
                    }
                );
                if st.instance_count > 2 {
                    line2.push_str(&format!(
                        "   •   процессов: {} (лишние мешают)",
                        st.instance_count
                    ));
                }
                ui.label(RichText::new(line2).size(12.0).color(p.text_dim));
                ui.label(
                    RichText::new(st.dir.to_string_lossy().to_string())
                        .size(11.0)
                        .color(p.text_muted),
                );
            } else {
                w::small(
                    ui,
                    &p,
                    "Скачайте официальный TgWsProxy_windows.exe — прокси слушает 127.0.0.1:1443 и сворачивается в трей.",
                );
            }
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                let label = if st.running { "Остановить" } else { "Запустить" };
                let resp = if st.running {
                    w::danger(ui, &p, label)
                } else {
                    w::primary(ui, &p, label)
                };
                if resp.clicked() {
                    if !st.installed {
                        self.notify(
                            "TG WS Proxy не установлен. Установите его ниже.",
                            LogLevel::Warn,
                        );
                    } else if st.running {
                        let dir = st.dir.clone();
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
                if st.installed
                    && w::secondary(ui, &p, "Перезапустить").clicked()
                {
                    let dir = st.dir.clone();
                    let ini = self.ini.clone();
                    self.spawn_bg(move |tx| {
                        let _ = tgproxy::stop(&dir);
                        std::thread::sleep(std::time::Duration::from_millis(1200));
                        let (message, ok) = tgproxy::start(&ini);
                        let _ = tx.send(Msg::Notify(
                            message,
                            if ok { LogLevel::Ok } else { LogLevel::Error },
                        ));
                    });
                }
            });
        });

        w::card(ui, &p, "Подключение Telegram", |ui| {
            w::small(
                ui,
                &p,
                &format!(
                    "Прокси работает локально: Telegram → {}:{} → WebSocket → DC Telegram. Кнопка «Открыть в Telegram» подключает прокси сама.",
                    st.host, st.port
                ),
            );
            ui.add_space(6.0);
            let link_text = if !st.installed {
                "TG WS Proxy не установлен".to_string()
            } else if link.is_empty() {
                "Секрет появится после первого запуска прокси".to_string()
            } else {
                link.clone()
            };
            w::code_block(ui, &p, &link_text);
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                if w::primary(ui, &p, "Открыть в Telegram").clicked() {
                    if !st.installed {
                        self.notify("Сначала установите TG WS Proxy.", LogLevel::Warn);
                    } else if link.is_empty() {
                        self.notify(
                            "Сначала запустите TG WS Proxy — он сформирует секрет.",
                            LogLevel::Warn,
                        );
                    } else {
                        tgproxy::open_in_telegram(&link);
                    }
                }
                if w::secondary(ui, &p, "Скопировать ссылку").clicked() {
                    if link.is_empty() {
                        self.notify("Ссылка ещё не сформирована — запустите прокси.", LogLevel::Warn);
                    } else {
                        self.clipboard = Some(link.clone());
                        self.notify("Ссылка скопирована. Вставьте её в Telegram.", LogLevel::Ok);
                    }
                }
            });
        });

        w::card(ui, &p, "Установка и обновление", |ui| {
            ui.label(
                RichText::new(if st.installed {
                    format!(
                        "Установленная версия: {}",
                        if st.version.is_empty() { "?" } else { &st.version }
                    )
                } else {
                    "TG WS Proxy ещё не установлен".to_string()
                })
                .size(12.5)
                .color(p.text),
            );
            if let Some(latest) = &self.pages.tg.latest {
                let newer = crate::util::version_cmp(latest, &st.version) == std::cmp::Ordering::Greater;
                ui.label(
                    RichText::new(if newer {
                        format!("Доступна новая версия: {}", latest)
                    } else {
                        "Установлена актуальная версия.".to_string()
                    })
                    .size(12.0)
                    .color(if newer { p.warn } else { p.text_muted }),
                );
            } else if !self.pages.tg.check_error.is_empty() {
                ui.label(
                    RichText::new(format!("Ошибка: {}", self.pages.tg.check_error))
                        .size(12.0)
                        .color(p.danger),
                );
            }
            if !self.pages.tg.stage.is_empty() {
                ui.label(
                    RichText::new(&self.pages.tg.stage)
                        .size(11.5)
                        .color(p.text_muted),
                );
            }
            if self.pages.tg.progress > 0 && self.pages.tg.progress < 100 {
                w::progress_bar(ui, &p, self.pages.tg.progress as f32 / 100.0);
            }
            ui.add_space(6.0);
            ui.horizontal_wrapped(|ui| {
                if w::secondary(ui, &p, "Проверить обновления").clicked() {
                    self.pages.tg.working = true;
                    self.pages.tg.stage = "Проверка обновлений…".to_string();
                    self.pages.tg.check_error.clear();
                    self.spawn_bg(move |tx| {
                        let result = tgproxy::check_latest();
                        let _ = tx.send(Msg::TgLatest(result));
                    });
                }
                if w::primary(ui, &p, "Скачать и установить").clicked() {
                    if let Some(tag) = self.pages.tg.latest.clone() {
                        let dir = tgproxy::dir_or_default(&self.ini);
                        self.install_tg(tag, dir);
                    } else {
                        self.notify(
                            "Сначала нажмите «Проверить обновления».",
                            LogLevel::Warn,
                        );
                    }
                }
                if w::ghost(ui, &p, "Открыть папку").clicked() {
                    let dir = tgproxy::dir_or_default(&self.ini);
                    crate::engine::sys::open_folder(&dir);
                }
                if w::ghost(ui, &p, "config.json").clicked() {
                    crate::engine::sys::select_in_explorer(&tgproxy::proxy_config_path());
                }
            });
        });

        w::card(ui, &p, "Автозапуск", |ui| {
            let mut auto = self.ini.get_bool("AutostartTgProxy", true);
            if w::toggle_row(
                ui,
                &p,
                "Запускать TG WS Proxy вместе с менеджером",
                "Срабатывает при старте менеджера, в том числе при автозапуске Windows",
                &mut auto,
            ) {
                self.ini.set_bool("AutostartTgProxy", auto);
                let _ = self.ini.save();
            }
            let zapret_auto = self.ini.get_bool("AutostartZapret", true);
            w::muted(
                ui,
                &p,
                if zapret_auto {
                    "Менеджер при автозапуске поднимает и zapret, и TG WS Proxy."
                } else {
                    "Менеджер при автозапуске поднимает только TG WS Proxy (zapret отключён в «Настройки»)."
                },
            );
        });

        w::card(ui, &p, "Режим запуска", |ui| {
            let mut quiet = tgproxy::run_mode(&self.ini) == "quiet";
            if w::toggle_row(
                ui,
                &p,
                "Без иконки в трее (из исходников)",
                "Официальные исходники Flowseal запускаются через портативный Python — без окна и без значка в трее",
                &mut quiet,
            ) {
                self.apply_tg_mode(quiet);
            }

            let prepared = self.pages.tg.quiet_prepared;
            let status = if prepared {
                let version = tgsource::prepared_version();
                let running = if quiet && st.quiet_running {
                    format!("  •  работает без трея (PID {})", st.pid)
                } else {
                    "  •  можно включать".to_string()
                };
                format!("Подготовлено: {}{}", version, running)
            } else {
                "Тихий режим не подготовлен: нажмите «Подготовить» — портативный Python (≈10 МБ) скачается автоматически.".to_string()
            };
            ui.label(RichText::new(status).size(11.5).color(p.text_muted));

            if !self.pages.tg.quiet_status.is_empty() {
                ui.label(
                    RichText::new(&self.pages.tg.quiet_status)
                        .size(11.5)
                        .color(if self.pages.tg.quiet_busy { p.text_dim } else { p.text_muted }),
                );
            }
            if self.pages.tg.quiet_busy {
                w::progress_bar(ui, &p, self.pages.tg.quiet_progress as f32 / 100.0);
            }
            ui.add_space(4.0);
            let label = if prepared {
                "Обновить исходники"
            } else {
                "Подготовить"
            };
            if w::secondary(ui, &p, label).clicked() && !self.pages.tg.quiet_busy {
                self.prepare_tg_quiet();
            }
            w::hint(
                ui,
                &p,
                "Python устанавливать не нужно: менеджер скачает embeddable-сборку и зависимости в свою папку.",
            );
        });

        if !st.config_error.is_empty() {
            w::muted(ui, &p, &format!("config.json: {}", st.config_error));
        }
    }

    /// Включить/выключить тихий режим. Если он ещё не подготовлен — сразу готовим.
    fn apply_tg_mode(&mut self, quiet: bool) {
        tgproxy::set_run_mode(&mut self.ini, quiet);
        self.pages.tg.quiet_status.clear();
        if quiet && !self.pages.tg.quiet_prepared {
            self.notify(
                "Тихий режим включён: сначала подготовим исходники и портативный Python.",
                LogLevel::Info,
            );
            self.prepare_tg_quiet();
            return;
        }
        self.notify(
            if quiet {
                "Тихий режим включён — прокси будет работать без иконки в трее."
            } else {
                "Тихий режим выключен — используется официальный TgWsProxy_windows.exe."
            },
            LogLevel::Ok,
        );
        self.restart_tg_proxy_for_mode();
    }

    /// Перезапустить работающий прокси, чтобы применить режим.
    pub(crate) fn restart_tg_proxy_for_mode(&mut self) {
        if !self.pages.tg.state.running {
            return;
        }
        let ini = self.ini.clone();
        self.spawn_bg(move |tx| {
            let _ = tgproxy::stop_all();
            std::thread::sleep(std::time::Duration::from_millis(1000));
            let (message, ok) = tgproxy::start(&ini);
            let _ = tx.send(Msg::Notify(
                message,
                if ok { LogLevel::Ok } else { LogLevel::Error },
            ));
        });
    }

    /// Подготовка тихого режима в фоне (исходники + Python + зависимости).
    fn prepare_tg_quiet(&mut self) {
        if self.pages.tg.quiet_busy {
            return;
        }
        self.pages.tg.quiet_busy = true;
        self.pages.tg.quiet_progress = 2;
        self.pages.tg.quiet_status = "Подготовка тихого режима…".to_string();
        self.spawn_bg(|tx| {
            let tx2 = tx.clone();
            let result = tgsource::prepare_latest(|percent, stage| {
                let _ = tx2.send(Msg::Progress("tg-quiet", percent, stage.to_string()));
            });
            let _ = tx.send(Msg::TgPrepareDone(result));
        });
    }

    fn install_tg(&mut self, tag: String, dir: std::path::PathBuf) {
        self.pages.tg.working = true;
        self.pages.tg.progress = 0;
        self.pages.tg.stage = format!("Устанавливаю TG WS Proxy {}…", tag);
        self.spawn_bg(move |tx| {
            let mut ini = crate::engine::config::Ini::load(&crate::engine::paths::config_path());
            let tx2 = tx.clone();
            let tag_owned = tag.clone();
            let result = tgproxy::install(&tag_owned, &dir, &mut ini, |percent, stage| {
                let _ = tx2.send(Msg::Progress("tg", percent, stage.to_string()));
            });
            let _ = tx.send(Msg::TgInstallDone(result));
        });
    }
}
