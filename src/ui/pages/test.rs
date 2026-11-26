//! Страница «Тест стратегий».

use std::sync::atomic::Ordering;

use eframe::egui::{self, RichText, Ui};

use crate::engine::log::LogLevel;
use crate::engine::ops::ZapretOps;
use crate::engine::tester::{self, StrategyTestRow, TesterOptions};

use crate::ui::app::{App, Msg};
use crate::ui::widgets as w;

impl App {
    pub(crate) fn page_test(&mut self, ui: &mut Ui) {
        let p = self.p;
        if !self.install.is_valid() {
            w::card(ui, &p, "Тест стратегий", |ui| {
                w::small(ui, &p, "Папка zapret не найдена. Укажите её на странице «Обзор».");
            });
            return;
        }
        let st = &self.pages.test;
        let groups = st.groups.clone();
        let enabled = st.enabled.clone();
        let running = st.running;

        w::card(ui, &p, "Параметры прогона", |ui| {
            w::small(
                ui,
                &p,
                "Тест по очереди запускает стратегии через winws.exe и проверяет доступность целей из utils\\targets.txt. Текущий bypass будет остановлен, после прогона восстановлен.",
            );
            ui.add_space(6.0);
            ui.label(RichText::new("Проверяемые цели:").size(12.5).color(p.text_dim));
            ui.horizontal_wrapped(|ui| {
                for group in &groups {
                    let on = self.pages.test.enabled.contains(&group.name);
                    if w::mode_button(ui, &p, &group.name, on).clicked() {
                        if on {
                            self.pages.test.enabled.remove(&group.name);
                        } else {
                            self.pages.test.enabled.insert(group.name.clone());
                        }
                    }
                }
                if groups.is_empty() {
                    w::muted(ui, &p, "Файл utils\\targets.txt не найден.");
                }
            });
            let _ = enabled;
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                ui.label(RichText::new("Таймаут:").size(12.5).color(p.text_dim));
                for ms in [3000u32, 5000, 8000] {
                    let selected = self.pages.test.timeout_ms == ms;
                    if w::mode_button(ui, &p, &format!("{} с", ms / 1000), selected).clicked() {
                        self.pages.test.timeout_ms = ms;
                    }
                }
                let mut deep = self.pages.test.deep;
                if w::toggle(ui, &p, &mut deep).changed() {
                    self.pages.test.deep = deep;
                }
                ui.label(
                    RichText::new("Глубокая проверка (загрузка 16 КБ)")
                        .size(12.0)
                        .color(p.text_dim),
                );
            });
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                let label = if running { "Идёт прогон…" } else { "Запустить тест" };
                if w::primary(ui, &p, label).clicked() && !running {
                    self.start_test();
                }
                let stop = ui.add_enabled(
                    running,
                    egui::Button::new(RichText::new("Остановить").color(p.danger)),
                );
                if stop.clicked() && running {
                    self.pages.test.cancel.store(true, Ordering::SeqCst);
                    self.pages.test.status = "Останавливаю…".to_string();
                }
                if w::ghost(ui, &p, "targets.txt").clicked() {
                    let path = self.install.utils_dir().join("targets.txt");
                    crate::engine::sys::select_in_explorer(&path);
                }
            });
            if running || self.pages.test.done > 0 {
                let ratio = if self.pages.test.total > 0 {
                    self.pages.test.done as f32 / self.pages.test.total as f32
                } else {
                    0.0
                };
                w::progress_bar(ui, &p, ratio);
            }
            ui.label(
                RichText::new(if self.pages.test.status.is_empty() {
                    format!(
                        "Стратегий: {} · групп целей: {} · выбрано: {}",
                        self.install.strategies.len(),
                        groups.len(),
                        self.pages.test.enabled.len()
                    )
                } else {
                    self.pages.test.status.clone()
                })
                .size(12.0)
                .color(p.text_muted),
            );
        });

        let rows = self.pages.test.rows.clone();
        let group_names: Vec<String> = groups
            .iter()
            .filter(|g| self.pages.test.enabled.contains(&g.name))
            .map(|g| g.name.clone())
            .collect();
        let best_score = rows
            .iter()
            .filter(|r| r.error.is_empty())
            .map(|r| r.passed())
            .max()
            .unwrap_or(0);
        let mut apply_selected = false;
        w::card(ui, &p, "Результаты", |ui| {
            ui.label(
                RichText::new(&self.pages.test.summary)
                    .size(12.5)
                    .color(if rows.is_empty() { p.text_dim } else { p.text }),
            );
            ui.add_space(8.0);
            if rows.is_empty() {
                w::muted(ui, &p, "Результатов пока нет — запустите тест.");
                return;
            }
            // заголовок таблицы
            let widths = table_widths(ui.available_width(), group_names.len());
            ui.horizontal(|ui| {
                cell(ui, &p, "Стратегия", widths.0, true);
                for name in &group_names {
                    cell(ui, &p, name, widths.1, true);
                }
                cell(ui, &p, "Итог", widths.2, true);
            });
            w::separator(ui, &p);
            for (idx, row) in rows.iter().enumerate() {
                let selected = self.pages.test.selected == Some(idx);
                let (rect, resp) = ui.allocate_exact_size(
                    egui::vec2(ui.available_width(), 30.0),
                    egui::Sense::click(),
                );
                if selected {
                    ui.painter().rect_filled(rect, egui::Rounding::same(4), p.selection);
                } else if resp.hovered() {
                    ui.painter().rect_filled(rect, egui::Rounding::same(4), p.hover);
                }
                let is_best = !row.error.is_empty() == false
                    && row.passed() == best_score
                    && row.total() > 0;
                let mut x = rect.left() + 8.0;
                let name = if is_best {
                    format!("★ {}", row.strategy_name)
                } else {
                    row.strategy_name.clone()
                };
                ui.painter().text(
                    egui::pos2(x, rect.center().y),
                    egui::Align2::LEFT_CENTER,
                    name,
                    egui::FontId::new(12.0, egui::FontFamily::Proportional),
                    if row.error.is_empty() { p.text } else { p.danger },
                );
                x += widths.0;
                for gname in &group_names {
                    let text = match row.groups.iter().find(|g| g.group.eq_ignore_ascii_case(gname)) {
                        Some(g) => g.cell_text(),
                        None => "—".to_string(),
                    };
                    let color = match row.groups.iter().find(|g| g.group.eq_ignore_ascii_case(gname)) {
                        Some(g) if g.total == 0 => p.text_muted,
                        Some(g) if g.ok() => p.success,
                        Some(g) if g.partial() => p.warn,
                        Some(_) => p.danger,
                        None => p.text_muted,
                    };
                    ui.painter().text(
                        egui::pos2(x, rect.center().y),
                        egui::Align2::LEFT_CENTER,
                        text,
                        egui::FontId::new(11.5, egui::FontFamily::Proportional),
                        color,
                    );
                    x += widths.1;
                }
                let score = if row.total() == 0 {
                    "—".to_string()
                } else {
                    format!("{} / {}", row.passed(), row.total())
                };
                let color = if row.total() == 0 {
                    p.text_muted
                } else if row.all_ok() {
                    p.success
                } else if row.passed() > 0 {
                    p.warn
                } else {
                    p.danger
                };
                ui.painter().text(
                    egui::pos2(x, rect.center().y),
                    egui::Align2::LEFT_CENTER,
                    score,
                    egui::FontId::new(11.5, egui::FontFamily::Proportional),
                    color,
                );
                if !row.error.is_empty() {
                    ui.painter().text(
                        egui::pos2(rect.right() - 8.0, rect.center().y),
                        egui::Align2::RIGHT_CENTER,
                        "не запустилась",
                        egui::FontId::new(11.0, egui::FontFamily::Proportional),
                        p.danger,
                    );
                }
                if resp.clicked() {
                    self.pages.test.selected = Some(idx);
                }
            }
            ui.add_space(10.0);
            ui.horizontal(|ui| {
                let can_apply = self.pages.test.selected.is_some() && !self.pages.test.running;
                if ui.add_enabled(can_apply, egui::Button::new("Запустить выбранную")).clicked() {
                    apply_selected = true;
                }
                if ui.add_enabled(can_apply, egui::Button::new("Установить как службу")).clicked() {
                    self.install_selected_from_test();
                }
                if w::ghost(ui, &p, "Сохранить отчёт…").clicked() {
                    self.save_test_report();
                }
            });
        });
        if apply_selected {
            self.run_selected_from_test();
        }

        // официальный отчёт
        let reports = self.pages.test.reports.clone();
        if !reports.is_empty() {
            w::card(ui, &p, "Отчёт официального теста (utils\\test zapret.ps1)", |ui| {
                let names: Vec<String> = reports
                    .iter()
                    .map(|p| p.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default())
                    .collect();
                ui.horizontal(|ui| {
                    egui::ComboBox::from_id_salt("report_pick")
                        .selected_text(
                            names
                                .get(self.pages.test.report_index)
                                .cloned()
                                .unwrap_or_default(),
                        )
                        .width(330.0)
                        .show_ui(ui, |ui| {
                            let mut changed = false;
                            for (i, name) in names.iter().enumerate() {
                                if ui
                                    .selectable_value(
                                        &mut self.pages.test.report_index,
                                        i,
                                        name,
                                    )
                                    .changed()
                                {
                                    changed = true;
                                }
                            }
                            if changed {
                                self.refresh_official_report();
                            }
                        });
                    if w::ghost(ui, &p, "Обновить список").clicked() {
                        self.pages.test.reports = tester::list_reports(&self.install);
                        self.refresh_official_report();
                    }
                    if w::ghost(ui, &p, "Открыть папку").clicked() {
                        crate::engine::sys::open_folder(&tester::reports_dir(&self.install));
                    }
                });
                ui.add_space(6.0);
                if let Some(report) = &self.pages.test.official {
                    ui.label(
                        RichText::new(format!(
                            "{}  •  {}  •  стратегий в отчёте: {}  •  лучшая: {}",
                            report.file.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default(),
                            report.created,
                            report.rows.len(),
                            if report.best.is_empty() { "—" } else { &report.best }
                        ))
                        .size(11.5)
                        .color(p.text_muted),
                    );
                    egui::ScrollArea::vertical().max_height(200.0).show(ui, |ui| {
                        for row in &report.rows {
                            let best = !report.best.is_empty()
                                && row.name.eq_ignore_ascii_case(&report.best);
                            ui.horizontal(|ui| {
                                ui.label(
                                    RichText::new(format!(
                                        "{}{}",
                                        if best { "★ " } else { "" },
                                        row.name
                                    ))
                                    .size(12.0)
                                    .color(if best { p.accent } else { p.text }),
                                );
                                ui.label(
                                    RichText::new(row.details())
                                        .size(11.5)
                                        .color(p.text_muted),
                                );
                            });
                        }
                    });
                }
            });
        }
    }

    fn start_test(&mut self) {
        if !self.require_admin("Тест стратегий") {
            return;
        }
        if self.pages.test.enabled.is_empty() {
            self.notify("Выберите хотя бы одну группу целей.", LogLevel::Warn);
            return;
        }
        if self.install.strategies.is_empty() {
            self.notify("Стратегии не найдены.", LogLevel::Error);
            return;
        }
        let install = self.install.clone();
        let groups: Vec<String> = self.pages.test.enabled.iter().cloned().collect();
        let options = TesterOptions {
            timeout_ms: self.pages.test.timeout_ms,
            deep_check: self.pages.test.deep,
            restore_after: true,
        };
        self.pages.test.running = true;
        self.pages.test.cancel = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let cancel = self.pages.test.cancel.clone();
        self.pages.test.rows.clear();
        self.pages.test.selected = None;
        self.pages.test.summary = "Прогон…".to_string();
        self.pages.test.done = 0;
        self.pages.test.total = install.strategies.len();
        self.pages.test.status = "Прогон стратегий…".to_string();
        let options_for_report = options.clone();
        let groups_for_report = groups.clone();
        self.spawn_bg(move |tx| {
            let progress_tx = tx.clone();
            let row_tx = tx.clone();
            let cancel_flag = cancel.clone();
            let rows = tester::run(
                &install,
                &groups,
                &options,
                |done, total, name| {
                    let _ = progress_tx.send(Msg::TestProgress(done, total, name.to_string()));
                },
                |row| {
                    let _ = row_tx.send(Msg::TestRow(Box::new(row.clone())));
                },
                &move || cancel_flag.load(Ordering::SeqCst),
            );
            let _ = tester::save_tsv(&rows, &groups_for_report, &options_for_report);
            let _ = tx.send(Msg::TestDone(rows));
        });
    }

    fn selected_test_strategy(&self) -> Option<crate::engine::install::StrategyDef> {
        let idx = self.pages.test.selected?;
        let row = self.pages.test.rows.get(idx)?;
        let install = &self.install;
        row.strategy_name
            .trim_end_matches(".bat")
            .split_whitespace()
            .next()
            .and_then(|name| install.find_strategy(name))
            .or_else(|| install.find_strategy(row.strategy_name.trim_end_matches(".bat")))
            .cloned()
    }

    fn run_selected_from_test(&mut self) {
        let Some(strategy) = self.selected_test_strategy() else {
            self.notify("Стратегия не найдена в папке zapret.", LogLevel::Error);
            return;
        };
        if self.state.service_installed && self.state.service_status == "работает" {
            self.confirm(
                "Остановить службу?",
                "Сейчас bypass работает как служба. Остановить службу и запустить выбранную стратегию вручную?",
                "Остановить службу и запустить",
                false,
                move |app| {
                    app.stop_bypass();
                    app.start_strategy(strategy.clone());
                },
            );
        } else {
            self.start_strategy(strategy);
        }
    }

    fn install_selected_from_test(&mut self) {
        let Some(strategy) = self.selected_test_strategy() else {
            self.notify("Стратегия не найдена в папке zapret.", LogLevel::Error);
            return;
        };
        self.confirm(
            "Установить службу?",
            &format!(
                "Стратегия «{}» будет установлена как служба zapret (автозапуск).",
                strategy.name
            ),
            "Установить",
            false,
            move |app| app.install_service(strategy.clone()),
        );
    }

    fn save_test_report(&mut self) {
        let path = rfd::FileDialog::new()
            .set_title("Сохранить отчёт теста")
            .add_filter("Таблица", &["tsv"])
            .set_file_name(format!(
                "zapret-strategy-test-{}.tsv",
                crate::util::datetime_from_secs(crate::util::unix_now() as i64)
                    .to_string()
                    .replace(['-', ':', ' '], "")
            ))
            .save_file();
        let Some(path) = path else { return };
        let groups: Vec<String> = self
            .pages
            .test
            .rows
            .first()
            .map(|r| r.groups.iter().map(|g| g.group.clone()).collect())
            .unwrap_or_default();
        let options = TesterOptions {
            timeout_ms: self.pages.test.timeout_ms,
            deep_check: self.pages.test.deep,
            restore_after: true,
        };
        let rows: Vec<StrategyTestRow> = self.pages.test.rows.clone();
        match tester::save_tsv_to(&path, &rows, &groups, &options) {
            Ok(()) => self.notify(
                format!("Отчёт сохранён: {}", path.to_string_lossy()),
                LogLevel::Ok,
            ),
            Err(e) => self.notify(format!("Не удалось сохранить отчёт: {}", e), LogLevel::Error),
        }
    }
}

fn cell(ui: &mut Ui, p: &crate::ui::theme::Palette, text: &str, width: f32, header: bool) {
    let (rect, _) = ui.allocate_exact_size(egui::vec2(width, 20.0), egui::Sense::hover());
    ui.painter().text(
        egui::pos2(rect.left() + 8.0, rect.center().y),
        egui::Align2::LEFT_CENTER,
        crate::util::shorten(text, 22),
        egui::FontId::new(
            if header { 11.5 } else { 12.0 },
            if header {
                egui::FontFamily::Name("bold".into())
            } else {
                egui::FontFamily::Proportional
            },
        ),
        if header { p.text_muted } else { p.text },
    );
}

fn table_widths(total: f32, groups: usize) -> (f32, f32, f32) {
    let name = (total - 90.0) * 0.34;
    let group = if groups > 0 {
        (total - name - 80.0) / groups as f32
    } else {
        0.0
    };
    (name.max(150.0), group.max(70.0), 80.0)
}

#[allow(dead_code)]
fn unused(_: &ZapretOps) {}
