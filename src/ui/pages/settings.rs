//! Страница «Настройки».

use eframe::egui::{self, RichText, Ui};

use crate::engine::gamefilter::{GameFilterMode, GameFilterState};
use crate::engine::log::LogLevel;
use crate::engine::ops::ZapretOps;
use crate::engine::{autostart, sys};
use crate::util::is_port_range_valid;

use crate::ui::app::{App, Msg, Page};
use crate::ui::widgets as w;

impl App {
    pub(crate) fn sync_settings(&mut self) {
        let gf = GameFilterState::load(&self.install.root);
        self.pages.settings.mode = gf.mode.clone();
        self.pages.settings.tcp = gf.tcp_range.clone();
        self.pages.settings.udp = gf.udp_range.clone();
        self.pages.settings.theme_index = match self.ini.get("Theme", "auto").as_str() {
            "dark" => 1,
            "light" => 2,
            _ => 0,
        };
        // Ручная правка manager.ini (например, 300) не должна делать окно
        // неиспользуемым: держим масштаб в разумных пределах.
        self.pages.settings.scale = (self.ini.get_int("UiScale", 100) as i32).clamp(75, 150);
        self.pages.settings.ipset_info = format!(
            "Текущий режим: {}. Строк в рабочем файле: {}, в резервном: {}.",
            crate::engine::ops::ipset_mode_text(&self.state.ipset_mode),
            self.state.ipset_lines,
            crate::engine::ops::count_list_lines(&self.install.ipset_backup())
        );
        self.reload_fakes();
        // `schtasks /Query` — сотни миллисекунд, а на медленной машине до
        // нескольких секунд: спрашиваем в фоне, а не в кадре.
        if self
            .pages
            .settings
            .autostart_checked
            .map(|t| t.elapsed() > std::time::Duration::from_secs(30))
            .unwrap_or(true)
        {
            self.pages.settings.autostart_checked = Some(std::time::Instant::now());
            let tx = self.tx.clone();
            std::thread::spawn(move || {
                let enabled = autostart::is_enabled();
                let _ = tx.send(Msg::Autostart(enabled));
            });
        }
    }

    pub(crate) fn reload_fakes(&mut self) {
        let ops = ZapretOps::new(self.install.clone());
        self.pages.settings.fakes = ops.list_fakes();
        self.pages.settings.active_discord = ops.find_active_fake("ACTIVE_DISCORD_UDP.bin");
        self.pages.settings.active_game = ops.find_active_fake("ACTIVE_GAME_UDP.bin");
        let names: Vec<String> = self.pages.settings.fakes.iter().map(|f| f.name.clone()).collect();
        self.pages.settings.fake_discord_idx = names
            .iter()
            .position(|n| n == &self.pages.settings.active_discord)
            .unwrap_or(0);
        self.pages.settings.fake_game_idx = names
            .iter()
            .position(|n| n == &self.pages.settings.active_game)
            .unwrap_or(0);
    }

    pub(crate) fn page_settings(&mut self, ui: &mut Ui) {
        let p = self.p;
        let valid = self.install.is_valid();
        let p2 = p;

        // Единое место, где выбирается папка zapret и ставится новая версия.
        w::card(ui, &p2, "Папка zapret", |ui| {
            w::small(
                ui,
                &p2,
                "Здесь указывается уже установленная папка или ставится zapret с нуля. В других разделах папку выбирать не нужно.",
            );
            ui.add_space(6.0);
            let path = if self.install.root.is_empty() {
                "не найдена".to_string()
            } else {
                self.install.root.clone()
            };
            ui.label(
                RichText::new(path)
                    .size(12.0)
                    .color(if valid { p2.text } else { p2.warn }),
            );
            ui.add_space(6.0);
            ui.horizontal_wrapped(|ui| {
                if w::secondary(ui, &p2, "Указать папку…").clicked() {
                    self.pick_zapret_folder();
                }
                if w::primary(ui, &p2, "Скачать и установить…").clicked() {
                    if let Some(parent) = rfd::FileDialog::new()
                        .set_title("Куда установить zapret?")
                        .pick_folder()
                    {
                        self.fresh_install(parent);
                    }
                }
                if valid && w::ghost(ui, &p2, "Открыть папку").clicked() {
                    crate::engine::sys::open_folder(std::path::Path::new(&self.install.root));
                }
                if w::ghost(ui, &p2, "Мастер установки…").clicked() {
                    self.open_wizard();
                }
            });
            if !valid {
                w::muted(
                    ui,
                    &p2,
                    "Для уже установленного zapret выберите папку, где лежат service.bat и bin\\winws.exe.",
                );
            }
        });

        // игровой фильтр
        w::card(ui, &p2, "Игровой фильтр", |ui| {
            w::small(
                ui,
                &p2,
                "Фильтр расширяет ускорение на игровые порты. Включайте, если игры или голосовой чат работают плохо.",
            );
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                for (mode, label) in [
                    (GameFilterMode::Disabled, "Выключен"),
                    (GameFilterMode::All, "TCP и UDP"),
                    (GameFilterMode::Tcp, "Только TCP"),
                    (GameFilterMode::Udp, "Только UDP"),
                ] {
                    let selected = self.pages.settings.mode == mode;
                    if w::mode_button(ui, &p2, label, selected).clicked() {
                        self.pages.settings.mode = mode;
                    }
                }
            });
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                ui.label(RichText::new("TCP-порты").size(12.0).color(p2.text_dim));
                ui.add(
                    egui::TextEdit::singleline(&mut self.pages.settings.tcp)
                        .desired_width(190.0)
                        .font(egui::TextStyle::Monospace),
                );
                ui.add_space(10.0);
                ui.label(RichText::new("UDP-порты").size(12.0).color(p2.text_dim));
                ui.add(
                    egui::TextEdit::singleline(&mut self.pages.settings.udp)
                        .desired_width(190.0)
                        .font(egui::TextStyle::Monospace),
                );
                if w::primary(ui, &p2, "Применить").clicked() {
                    if !is_port_range_valid(&self.pages.settings.tcp)
                        || !is_port_range_valid(&self.pages.settings.udp)
                    {
                        self.notify(
                            "Неверный формат портов. Пример: 1024-65535 или 1024-1934,1936-65535",
                            LogLevel::Error,
                        );
                    } else {
                        let gf = GameFilterState {
                            mode: self.pages.settings.mode.clone(),
                            tcp_range: self.pages.settings.tcp.clone(),
                            udp_range: self.pages.settings.udp.clone(),
                        };
                        match gf.save(&self.install.root) {
                            Ok(()) => {
                                crate::engine::prefs::remember_game_filter(&mut self.ini, &gf);
                                self.notify(
                                    "Настройки игрового фильтра сохранены.",
                                    LogLevel::Ok,
                                );
                                self.restart_after_change("Игровой фильтр обновлён");
                                self.refresh_state(false);
                            }
                            Err(e) => self.notify(
                                format!("Не удалось сохранить: {}", e),
                                LogLevel::Error,
                            ),
                        }
                    }
                }
            });
        });

        // IPSet
        w::card(ui, &p2, "IP-фильтр (IPSet)", |ui| {
            w::small(
                ui,
                &p2,
                "Определяет, к каким адресам применяется ускорение: только к доменам из списков или ко всем IP из полного списка.",
            );
            ui.add_space(8.0);
            let mut chosen = None;
            ui.horizontal(|ui| {
                for (mode, label) in [
                    ("loaded", "Полный список"),
                    ("any", "Любые IP"),
                    ("none", "Без фильтра"),
                ] {
                    let selected = self.state.ipset_mode == mode;
                    if w::mode_button(ui, &p2, label, selected).clicked() && !selected {
                        chosen = Some(mode);
                    }
                }
            });
            if let Some(mode) = chosen {
                self.set_ipset_mode_auto(mode);
            }
            ui.label(RichText::new(&self.pages.settings.ipset_info).size(11.5).color(p2.text_muted));
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                let progress = self.progress.get("ipset").map(|(p, _)| *p).unwrap_or(0);
                if w::secondary(ui, &p2, "Обновить IP-лист").clicked() {
                    let ops = ZapretOps::new(self.install.clone());
                    self.progress.insert("ipset", (0, "Скачивание IP-листа…".to_string()));
                    self.spawn_bg(move |tx| {
                        let message = ops.update_ipset(|read, total| {
                            let percent = if total > 0 { (read * 100 / total) as u32 } else { 30 };
                            let _ = tx.send(crate::ui::app::Msg::Progress(
                                "ipset",
                                percent,
                                format!("Скачивание IP-листа: {}%", percent),
                            ));
                        });
                        let level = if message.starts_with("Ошибка") {
                            LogLevel::Error
                        } else {
                            LogLevel::Ok
                        };
                        let _ = tx.send(crate::ui::app::Msg::Notify(message, level));
                    });
                }
                if progress > 0 {
                    ui.add(egui::ProgressBar::new(progress as f32 / 100.0).desired_width(240.0));
                }
            });
        });

        // фейки
        w::card(ui, &p2, "Активные фейк-пакеты", |ui| {
            w::small(
                ui,
                &p2,
                "Подмена «фейковых» пакетов для Discord и игрового фильтра. Если голосовой чат не работает — попробуйте другой файл.",
            );
            let names: Vec<String> = self.pages.settings.fakes.iter().map(|f| f.name.clone()).collect();
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                ui.label(RichText::new("Discord (UDP)").size(12.0).color(p2.text_dim));
                egui::ComboBox::from_id_salt("fake_discord")
                    .selected_text(
                        names
                            .get(self.pages.settings.fake_discord_idx)
                            .cloned()
                            .unwrap_or_default(),
                    )
                    .width(260.0)
                    .show_ui(ui, |ui| {
                        for (i, name) in names.iter().enumerate() {
                            ui.selectable_value(&mut self.pages.settings.fake_discord_idx, i, name);
                        }
                    });
                if w::secondary(ui, &p2, "Заменить").clicked() {
                    if let Some(name) = names.get(self.pages.settings.fake_discord_idx).cloned() {
                        let src = self.install.bin_dir().join(format!("{}.bin", name));
                        self.confirm(
                            "Заменить фейк?",
                            &format!(
                                "ACTIVE_DISCORD_UDP.bin будет заменён содержимым файла {}.bin. Ускорение будет перезапущено автоматически.",
                                name
                            ),
                            "Заменить",
                            false,
                            move |app| {
                                let ops = ZapretOps::new(app.install.clone());
                                let message = ops.replace_active_fake("ACTIVE_DISCORD_UDP.bin", &src);
                                app.notify(message, LogLevel::Ok);
                                app.reload_fakes();
                                app.restart_after_change("Фейк заменён");
                            },
                        );
                    }
                }
            });
            ui.horizontal(|ui| {
                ui.label(RichText::new("Игровой фильтр (UDP)").size(12.0).color(p2.text_dim));
                egui::ComboBox::from_id_salt("fake_game")
                    .selected_text(
                        names
                            .get(self.pages.settings.fake_game_idx)
                            .cloned()
                            .unwrap_or_default(),
                    )
                    .width(260.0)
                    .show_ui(ui, |ui| {
                        for (i, name) in names.iter().enumerate() {
                            ui.selectable_value(&mut self.pages.settings.fake_game_idx, i, name);
                        }
                    });
                if w::secondary(ui, &p2, "Заменить").clicked() {
                    if let Some(name) = names.get(self.pages.settings.fake_game_idx).cloned() {
                        let src = self.install.bin_dir().join(format!("{}.bin", name));
                        self.confirm(
                            "Заменить фейк?",
                            &format!(
                                "ACTIVE_GAME_UDP.bin будет заменён содержимым файла {}.bin. Ускорение будет перезапущено автоматически.",
                                name
                            ),
                            "Заменить",
                            false,
                            move |app| {
                                let ops = ZapretOps::new(app.install.clone());
                                let message = ops.replace_active_fake("ACTIVE_GAME_UDP.bin", &src);
                                app.notify(message, LogLevel::Ok);
                                app.reload_fakes();
                                app.restart_after_change("Фейк заменён");
                            },
                        );
                    }
                }
            });
            ui.label(
                RichText::new(format!(
                    "Сейчас: Discord — {}, игровой — {}. Файлов в bin: {}",
                    self.pages.settings.active_discord,
                    self.pages.settings.active_game,
                    self.pages.settings.fakes.len()
                ))
                .size(11.5)
                .color(p2.text_muted),
            );
        });

        // прочее
        w::card(ui, &p2, "Прочее", |ui| {
            ui.horizontal(|ui| {
                ui.label(RichText::new("Оформление").size(12.5).color(p2.text_dim));
                let items = ["Как в Windows", "Тёмная", "Светлая"];
                egui::ComboBox::from_id_salt("theme")
                    .selected_text(items[self.pages.settings.theme_index.min(2)])
                    .width(200.0)
                    .show_ui(ui, |ui| {
                        for (i, item) in items.iter().enumerate() {
                            if ui
                                .selectable_value(&mut self.pages.settings.theme_index, i, *item)
                                .changed()
                            {
                                self.apply_theme_choice(i);
                            }
                        }
                    });
            });
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                ui.label(RichText::new("Масштаб интерфейса").size(12.5).color(p2.text_dim));
                for pct in [90, 100, 110] {
                    let selected = self.pages.settings.scale == pct;
                    if w::mode_button(ui, &p2, &format!("{} %", pct), selected).clicked() {
                        self.pages.settings.scale = pct;
                        self.scale = pct as f32 / 100.0;
                        self.ini.set_int("UiScale", pct as i64);
                        let _ = self.ini.save();
                        self.notify(format!("Масштаб интерфейса: {} %.", pct), LogLevel::Ok);
                    }
                }
            });
            ui.add_space(6.0);
            let mut check_updates = self.ini.get_bool("CheckUpdatesOnStartup", true);
            if w::toggle_row(
                ui,
                &p2,
                "Проверять обновления при запуске менеджера",
                "При старте и далее раз в 6 часов проверять GitHub (zapret, TG WS Proxy, менеджер)",
                &mut check_updates,
            ) {
                self.ini.set_bool("CheckUpdatesOnStartup", check_updates);
                let _ = self.ini.save();
                self.notify(
                    if check_updates {
                        "Автопроверка обновлений менеджера включена."
                    } else {
                        "Автопроверка обновлений менеджера выключена."
                    },
                    LogLevel::Ok,
                );
            }
            ui.add_space(8.0);
            // Значение приходит из фонового опроса состояния: `netsh` в кадре
            // подвешивал окно на секунды (а на медленной машине — до таймаута).
            let tcp_on = self.state.tcp_timestamps == Some(true);
            ui.horizontal(|ui| {
                let label = if tcp_on {
                    "Выключить TCP timestamps"
                } else {
                    "Включить TCP timestamps"
                };
                if w::secondary(ui, &p2, label).clicked() {
                    if self.require_admin("TCP timestamps") {
                        let enable = !tcp_on;
                        let ops = ZapretOps::new(self.install.clone());
                        self.spawn_bg(move |tx| {
                            let ok = if enable {
                                ops.enable_tcp_timestamps() == Some(true)
                            } else {
                                ops.disable_tcp_timestamps() == Some(false)
                            };
                            let _ = tx.send(crate::ui::app::Msg::Notify(
                                if ok {
                                    if enable {
                                        "TCP timestamps включены.".to_string()
                                    } else {
                                        "TCP timestamps выключены.".to_string()
                                    }
                                } else {
                                    "Не удалось изменить TCP timestamps.".to_string()
                                },
                                if ok { LogLevel::Ok } else { LogLevel::Error },
                            ));
                        });
                    }
                }
                if w::secondary(ui, &p2, "Очистить кэш Discord").clicked() {
                    self.confirm(
                        "Очистить кэш Discord?",
                        "Discord будет закрыт, а папки Cache, Code Cache и GPUCache удалены.",
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
            });
        });

        // запуск и автозапуск
        w::card(ui, &p2, "Запуск и автозапуск", |ui| {
            let mut autostart_on = self.pages.settings.autostart_enabled;
            if w::toggle_row(
                ui,
                &p2,
                "Запускать Zapret Manager при входе в Windows",
                "Создаётся задача планировщика с наивысшими правами — запуск без запроса UAC",
                &mut autostart_on,
            ) {
                self.pages.settings.autostart_enabled = autostart_on;
                let start_minimized = self.ini.get_bool("StartMinimized", true);
                self.spawn_bg(move |tx| {
                    let message = if autostart_on {
                        autostart::enable(&crate::engine::paths::exe_path(), start_minimized)
                    } else {
                        autostart::disable()
                    };
                    let ok = !message.starts_with("Не удалось") && !message.contains("нужны права");
                    let _ = tx.send(crate::ui::app::Msg::Notify(
                        message,
                        if ok { LogLevel::Ok } else { LogLevel::Error },
                    ));
                });
            }
            let mut close_tray = self.ini.get_bool("CloseToTray", true);
            if w::toggle_row(
                ui,
                &p2,
                "При закрытии окна сворачиваться в трей",
                "Приложение продолжает работать в фоне, ускорение и прокси не останавливаются",
                &mut close_tray,
            ) {
                self.ini.set_bool("CloseToTray", close_tray);
                let _ = self.ini.save();
            }
            let mut auto_zapret = self.ini.get_bool("AutostartZapret", true);
            if w::toggle_row(
                ui,
                &p2,
                "При автозапуске поднимать zapret",
                "Запускать выбранную стратегию (или службу), если ускорение ещё не работает",
                &mut auto_zapret,
            ) {
                self.ini.set_bool("AutostartZapret", auto_zapret);
                let _ = self.ini.save();
            }
            let mut auto_heal = self.ini.get_bool("AutoHeal", false);
            if w::toggle_row(
                ui,
                &p2,
                "Автолечение ускорения",
                "Если winws.exe неожиданно завершился, перезапустить службу или последнюю стратегию (до 3 попыток). Не трогает ускорение, если вы остановили его вручную.",
                &mut auto_heal,
            ) {
                self.ini.set_bool("AutoHeal", auto_heal);
                let _ = self.ini.save();
                if auto_heal && self.state.winws_running {
                    self.auto_heal_armed = true;
                } else if !auto_heal {
                    self.auto_heal_armed = false;
                }
            }
            let mut auto_tg = self.ini.get_bool("AutostartTgProxy", true);
            if w::toggle_row(
                ui,
                &p2,
                "При автозапуске запускать TG WS Proxy",
                "Прокси Telegram стартует вместе с менеджером",
                &mut auto_tg,
            ) {
                self.ini.set_bool("AutostartTgProxy", auto_tg);
                let _ = self.ini.save();
            }
            let mut minimize = self.ini.get_bool("MinimizeToTray", true);
            if w::toggle_row(
                ui,
                &p2,
                "Сворачивать в трей по кнопке «Свернуть»",
                "Окно прячется в трей, а не висит в панели задач — приложение и сервисы продолжают работать",
                &mut minimize,
            ) {
                self.ini.set_bool("MinimizeToTray", minimize);
                let _ = self.ini.save();
            }
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new(if self.pages.settings.autostart_enabled {
                        "Автозапуск включён"
                    } else {
                        "Автозапуск выключен"
                    })
                    .size(12.0)
                    .color(if self.pages.settings.autostart_enabled {
                        p2.success
                    } else {
                        p2.text_muted
                    }),
                );
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if w::ghost(ui, &p2, "Планировщик заданий").clicked() {
                        autostart::open_task_scheduler();
                    }
                });
            });
        });

        if !valid {
            w::muted(ui, &p2, "Папка zapret не найдена — часть настроек недоступна.");
        }
        let _ = Page::Settings;
        let _ = sys::is_admin();
    }

    fn apply_theme_choice(&mut self, index: usize) {
        let mode = match index {
            1 => "dark",
            2 => "light",
            _ => "auto",
        };
        self.dark = crate::ui::theme::resolve_dark(mode);
        self.theme_name = mode.to_string();
        self.ini.set("Theme", mode);
        let _ = self.ini.save();
        self.notify(
            format!(
                "Оформление: {}.",
                match index {
                    1 => "тёмная",
                    2 => "светлая",
                    _ => {
                        if self.dark {
                            "как в Windows (тёмная)"
                        } else {
                            "как в Windows (светлая)"
                        }
                    }
                }
            ),
            LogLevel::Ok,
        );
    }
}
