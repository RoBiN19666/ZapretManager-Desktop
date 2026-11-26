//! Страница «Проверка сайта».

use eframe::egui::{self, RichText, Ui};

use crate::engine::appprobe;
use crate::engine::doctor::{self, Verdict};
use crate::engine::guard;
use crate::engine::log::LogLevel;
use crate::engine::sys;

use crate::ui::app::{App, Msg};
use crate::ui::widgets as w;

impl App {
    pub(crate) fn page_sites(&mut self, ui: &mut Ui) {
        let p = self.p;
        let valid = self.install.is_valid();

        w::card(ui, &p, "Проверка сайта", |ui| {
            ui.horizontal(|ui| {
                ui.add(
                    egui::TextEdit::singleline(&mut self.pages.sites.input)
                        .desired_width(400.0)
                        .hint_text("instagram.com или https://…"),
                );
                if w::primary(ui, &p, "Проверить").clicked() && !self.pages.sites.working {
                    self.run_site_check();
                }
                if w::ghost(ui, &p, "Открыть в браузере").clicked() {
                    let (_, url) = doctor::normalize_input(&self.pages.sites.input);
                    if url.is_empty() {
                        self.notify("Введите адрес сайта.", LogLevel::Warn);
                    } else {
                        sys::open_url(&url);
                    }
                }
            });
            ui.add_space(10.0);
            let color = if self.pages.sites.probe.is_none() {
                p.text_dim
            } else {
                match self.pages.sites.verdict_kind {
                    Verdict::Ok => p.success,
                    Verdict::Dpi => p.warn,
                    _ => p.danger,
                }
            };
            ui.label(RichText::new(&self.pages.sites.verdict).size(12.5).color(color));
            ui.add_space(6.0);
            if !self.pages.sites.steps.is_empty() {
                egui::Frame::none()
                    .fill(p.editor)
                    .stroke(egui::Stroke::new(1.0, p.border))
                    .rounding(egui::Rounding::same(6))
                    .inner_margin(egui::Margin::same(10))
                    .show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        ui.label(
                            RichText::new(&self.pages.sites.steps)
                                .font(egui::FontId::new(11.5, egui::FontFamily::Monospace))
                                .color(p.text_dim),
                        );
                    });
            }
            if self.pages.sites.working {
                ui.add(egui::ProgressBar::new(0.5).desired_width(ui.available_width()));
            }
            if !self.pages.sites.status.is_empty() {
                w::muted(ui, &p, &self.pages.sites.status);
            }
        });

        w::card(ui, &p, "Проверка приложения", |ui| {
            w::small(
                ui,
                &p,
                "Игра или программа не запускается? Выберите её в списке и нажмите «Наблюдать»: менеджер соберёт домены, к которым она обращается (журнал DNS), проверит их и добавит заблокированные в ваши списки.",
            );
            ui.add_space(8.0);
            let names: Vec<String> = self.pages.sites.apps.iter().map(|a| a.display()).collect();
            ui.horizontal(|ui| {
                egui::ComboBox::from_id_salt("app_pick")
                    .selected_text(
                        names
                            .get(self.pages.sites.app_index)
                            .cloned()
                            .unwrap_or_else(|| "Список приложений загружается…".to_string()),
                    )
                    .width(380.0)
                    .show_ui(ui, |ui| {
                        for (i, name) in names.iter().enumerate() {
                            ui.selectable_value(&mut self.pages.sites.app_index, i, name);
                        }
                    });
                if w::secondary(ui, &p, "Обновить список").clicked() && !self.pages.sites.watching {
                    self.spawn_bg(move |tx| {
                        let apps = appprobe::list_apps();
                        let _ = tx.send(Msg::Apps(apps));
                    });
                }
            });
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                if w::primary(ui, &p, "Наблюдать 60 с").clicked() && !self.pages.sites.watching {
                    self.watch_app();
                }
                if self.pages.sites.watching {
                    ui.spinner();
                    ui.label(
                        RichText::new("Наблюдаем: домены собираются из журнала DNS…")
                            .size(12.0)
                            .color(p.text_dim),
                    );
                }
                if names.is_empty() && self.pages.sites.apps_loaded {
                    w::muted(ui, &p, "Не нашли приложений с окнами — запустите игру и нажмите «Обновить список».");
                }
            });
        });

        w::card(ui, &p, "Адреса для ускорения", |ui| {
            w::small(
                ui,
                &p,
                "Здесь появляются домены сайта и связанные с ним адреса. Список можно править руками: одна строка — один домен. Поддомены указывать не обязательно, zapret сам учитывает их.",
            );
            ui.add_space(6.0);
            egui::Frame::none()
                .fill(p.editor)
                .stroke(egui::Stroke::new(1.0, p.border))
                .rounding(egui::Rounding::same(6))
                .inner_margin(egui::Margin::same(6))
                .show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.add(
                        egui::TextEdit::multiline(&mut self.pages.sites.editor)
                            .font(egui::FontId::new(12.0, egui::FontFamily::Monospace))
                            .desired_rows(6)
                            .desired_width(f32::INFINITY)
                            .code_editor(),
                    );
                });
            ui.add_space(8.0);
            ui.horizontal_wrapped(|ui| {
                if w::secondary(ui, &p, "Проверить адреса").clicked() {
                    self.probe_address_list();
                }
                if w::primary(ui, &p, "В мой список").clicked() {
                    self.add_addresses_to_list();
                }
                if w::secondary(ui, &p, "Прописать в hosts").clicked() {
                    self.add_addresses_to_hosts();
                }
                if w::ghost(ui, &p, "Скопировать").clicked() {
                    self.clipboard = Some(self.pages.sites.editor.clone());
                    self.notify("Адреса скопированы в буфер обмена.", LogLevel::Ok);
                }
            });
            w::hint(
                ui,
                &p,
                "Подсказка: если сайт открывается, но контент не грузится (как у Instagram), добавьте в список его CDN-домены — их видно по кнопке «Проверить адреса».",
            );
        });

        w::card(ui, &p, "Фоновый сторож", |ui| {
            let mut enabled = self.pages.sites.guard_enabled;
            if w::toggle_row(
                ui,
                &p,
                "Следить за блокировками в фоне",
                "Пока менеджер работает, он читает журнал DNS Windows: когда приложение или браузер запрашивает домен, сторож быстро проверяет его и добавляет в списки те, что рвутся из-за ТСПУ.",
                &mut enabled,
            ) {
                self.pages.sites.guard_enabled = enabled;
                self.ini.set_bool("SiteGuard", enabled);
                let _ = self.ini.save();
                self.pages.sites.guard_seen.clear();
                self.pages.sites.guard_blocked.clear();
                if enabled {
                    guard::reset_cursor();
                    let message = guard::enable_log();
                    let ok = !message.starts_with("Не удалось");
                    self.notify(
                        if ok {
                            "Сторож включён: журнал DNS включён.".to_string()
                        } else {
                            format!("Сторож включён, но журнал DNS: {}", message)
                        },
                        if ok { LogLevel::Ok } else { LogLevel::Error },
                    );
                    self.pages.sites.guard_status = if ok {
                        "Сторож работает: запросы браузеров под наблюдением.".to_string()
                    } else {
                        "Сторож включён, но журнал DNS недоступен.".to_string()
                    };
                } else {
                    self.notify("Сторож выключен.", LogLevel::Info);
                    self.pages.sites.guard_status = "Сторож выключен.".to_string();
                }
            }
            let log_on = guard::log_enabled();
            self.pages.sites.guard_status = if !self.pages.sites.guard_enabled {
                "Сторож выключен.".to_string()
            } else if !log_on {
                "Журнал DNS выключен — сторож не увидит запросы.".to_string()
            } else if !self.pages.sites.guard_seen.is_empty() {
                format!(
                    "Сторож: перехвачено доменов {}, найдено проблемных {} (автодобавление {}).",
                    self.pages.sites.guard_seen.len(),
                    self.pages.sites.guard_blocked.len(),
                    if self.pages.sites.guard_auto { "вкл" } else { "выкл" }
                )
            } else {
                "Сторож работает: журнал DNS включён, запросы приложений под наблюдением.".to_string()
            };
            ui.label(
                RichText::new(&self.pages.sites.guard_status)
                    .size(12.0)
                    .color(if log_on || !self.pages.sites.guard_enabled {
                        p.text_muted
                    } else {
                        p.warn
                    }),
            );
            ui.add_space(6.0);
            let mut auto_add = self.pages.sites.guard_auto;
            if w::toggle_row(
                ui,
                &p,
                "Добавлять найденное автоматически",
                "Иначе сторож только показывает уведомление о блокировке.",
                &mut auto_add,
            ) {
                self.pages.sites.guard_auto = auto_add;
                self.ini.set_bool("SiteGuardAutoAdd", auto_add);
                let _ = self.ini.save();
            }
            ui.add_space(8.0);
            if self.pages.sites.guard_seen.is_empty() {
                w::muted(
                    ui,
                    &p,
                    "Перехваченных доменов пока нет. Включите сторожа и откройте сайт в браузере — здесь появятся запросы из журнала DNS.",
                );
            } else {
                ui.label(
                    RichText::new(format!(
                        "Перехвачено доменов: {}  •  требуют добавления: {}",
                        self.pages.sites.guard_seen.len(),
                        self.pages.sites.guard_blocked.len()
                    ))
                    .size(12.0)
                    .color(p.text_dim),
                );
                egui::Frame::none()
                    .fill(p.editor)
                    .stroke(egui::Stroke::new(1.0, p.border))
                    .rounding(egui::Rounding::same(7))
                    .inner_margin(egui::Margin::same(8))
                    .show(ui, |ui| {
                        egui::ScrollArea::vertical()
                            .max_height(120.0)
                            .auto_shrink([false, false])
                            .show(ui, |ui| {
                                for host in self.pages.sites.guard_seen.iter().take(40) {
                                    let blocked = self
                                        .pages
                                        .sites
                                        .guard_blocked
                                        .iter()
                                        .any(|b| b.eq_ignore_ascii_case(host));
                                    ui.label(
                                        RichText::new(format!(
                                            "{}{}",
                                            host,
                                            if blocked {
                                                if self.pages.sites.guard_auto {
                                                    "  • проблемный (автодобавление вкл)"
                                                } else {
                                                    "  • требует добавления"
                                                }
                                            } else {
                                                ""
                                            }
                                        ))
                                        .size(11.5)
                                        .color(if blocked { p.warn } else { p.text_muted }),
                                    );
                                }
                            });
                    });
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    if !self.pages.sites.guard_blocked.is_empty()
                        && w::primary(ui, &p, "Добавить найденные").clicked()
                    {
                        let install = self.install.clone();
                        let blocked = self.pages.sites.guard_blocked.clone();
                        let found = guard::process_hosts(&install, &blocked, true, |message| {
                            crate::engine::log::warn(format!("Сторож сайтов: {}", message));
                        });
                        if !found.is_empty() {
                            self.pages.sites.guard_blocked.clear();
                            self.pages.sites.guard_status = format!(
                                "Сторож: добавлено доменов {}.",
                                found.len()
                            );
                            self.notify(
                                format!("Сторож: добавлено доменов {}.", found.len()),
                                LogLevel::Ok,
                            );
                            self.restart_after_change("Домены добавлены");
                        }
                    }
                    if w::ghost(ui, &p, "Очистить список").clicked() {
                        self.pages.sites.guard_seen.clear();
                        self.pages.sites.guard_blocked.clear();
                    }
                });
            }
            w::hint(
                ui,
                &p,
                "Если в браузере включён DoH (DNS через HTTPS), запросы идут мимо Windows и сторож их не увидит — тогда проверяйте сайт вручную выше.",
            );
        });

        let _ = valid;
    }

    fn run_site_check(&mut self) {
        let input = self.pages.sites.input.trim().to_string();
        if input.is_empty() || input == "https://" {
            self.notify("Введите домен или ссылку, например instagram.com", LogLevel::Warn);
            return;
        }
        self.pages.sites.working = true;
        self.pages.sites.steps.clear();
        self.pages.sites.verdict = format!("Проверяем {}…", input);
        self.pages.sites.verdict_kind = Verdict::Down;
        let tx_progress = self.tx.clone();
        self.spawn_bg(move |tx| {
            let tx2 = tx_progress;
            let mut probe = doctor::check(&input, |stage| {
                let _ = tx2.send(Msg::DiagStep(stage.to_string()));
            });
            let _ = tx.send(Msg::Probe(Box::new(probe.clone()), false));
            if probe.verdict != Verdict::Dns && probe.verdict != Verdict::Down {
                doctor::discover(&mut probe, |stage| {
                    let _ = tx2.send(Msg::DiagStep(stage.to_string()));
                });
                let _ = tx.send(Msg::Probe(Box::new(probe), true));
            }
        });
    }

    fn watch_app(&mut self) {
        if !self.install.is_valid() {
            self.notify("Папка zapret не найдена.", LogLevel::Error);
            return;
        }
        let Some(app) = self.pages.sites.apps.get(self.pages.sites.app_index).cloned() else {
            self.notify("Сначала выберите приложение.", LogLevel::Warn);
            return;
        };
        if !self.pages.sites.editor.trim().is_empty() {
            self.confirm(
                "Заменить список адресов?",
                "Текущий список в поле «Адреса для ускорения» будет заменён результатами наблюдения.",
                "Продолжить",
                false,
                move |app_state| app_state.start_watch(app),
            );
        } else {
            self.start_watch(app);
        }
    }

    fn start_watch(&mut self, app: appprobe::AppEntry) {
        self.pages.sites.watching = true;
        self.pages.sites.status = format!("Наблюдаем за {}…", app.name);
        let pid = app.pid;
        self.spawn_bg(move |tx| {
            let (hosts, ips) = appprobe::watch(pid, 60, |step, domains, addresses| {
                let _ = tx.send(Msg::Progress(
                    "watch",
                    step * 100 / 60,
                    format!(
                        "Наблюдаем: доменов {}, адресов {} ({} с из 60)",
                        domains, addresses, step
                    ),
                ));
            });
            let filtered = appprobe::filter_hosts(&hosts);
            let (blocked, ok) = doctor::blocked_hosts(&filtered, |_, _, _| {});
            let mut result = blocked.clone();
            for host in ok {
                if !result.iter().any(|h| h.eq_ignore_ascii_case(&host)) {
                    result.push(host);
                }
            }
            let _ = tx.send(Msg::WatchDone(result, ips));
        });
    }

    fn probe_address_list(&mut self) {
        let hosts: Vec<String> = self
            .pages
            .sites
            .editor
            .lines()
            .map(|l| l.split('#').next().unwrap_or("").trim().to_string())
            .filter(|l| !l.is_empty())
            .collect();
        if hosts.is_empty() {
            self.notify("Список пуст.", LogLevel::Warn);
            return;
        }
        self.pages.sites.working = true;
        self.spawn_bg(move |tx| {
            let (lines, _ok, _broken) = doctor::probe_list(&hosts, |_, _| {});
            let _ = tx.send(Msg::ProbeList(lines.join("\r\n")));
        });
    }

    fn add_addresses_to_list(&mut self) {
        if !self.install.is_valid() {
            self.notify("Папка zapret не найдена.", LogLevel::Error);
            return;
        }
        let hosts: Vec<String> = self
            .pages
            .sites
            .editor
            .lines()
            .map(|l| l.split('#').next().unwrap_or("").trim().to_string())
            .filter(|l| !l.is_empty())
            .collect();
        let (message, added, path) = doctor::add_to_user_list(&self.install, &hosts);
        self.notify(
            message.clone(),
            if added > 0 { LogLevel::Ok } else { LogLevel::Warn },
        );
        if added > 0 {
            self.pages.sites.status = format!("{} Файл: {}", message, path);
            self.refresh_state(true);
            self.restart_after_change("Домены добавлены в список");
        }
    }

    fn add_addresses_to_hosts(&mut self) {
        if !self.require_admin("Запись в hosts") {
            return;
        }
        let hosts: Vec<String> = self
            .pages
            .sites
            .editor
            .lines()
            .map(|l| l.split('#').next().unwrap_or("").trim().to_string())
            .filter(|l| !l.is_empty())
            .collect();
        self.confirm(
            "Прописать IP в hosts?",
            "Менеджер определит IP-адреса доменов и добавит записи в системный файл hosts. Будет создана резервная копия. Учтите: привязка CDN-домена к одному IP может замедлить сайт.",
            "Записать",
            false,
            move |app| {
                let hosts = hosts.clone();
                app.spawn_bg(move |tx| {
                    let (message, added, backup) = doctor::add_to_hosts(&hosts);
                    let mut text = message;
                    if !backup.is_empty() && added > 0 {
                        text.push_str(&format!(" Резервная копия: {}", backup));
                    }
                    let _ = tx.send(Msg::Notify(
                        text,
                        if added > 0 { LogLevel::Ok } else { LogLevel::Warn },
                    ));
                });
            },
        );
    }
}
