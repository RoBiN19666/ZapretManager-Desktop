//! Страница «Тест стратегий».

use std::sync::atomic::Ordering;

use eframe::egui::{self, RichText, Ui};

use crate::engine::log::LogLevel;
use crate::engine::autotune;
use crate::engine::install::{StrategyDef, StrategyKind};
use crate::engine::ops::ZapretOps;
use crate::engine::tester::{self, StrategyTestRow, TesterOptions};

use crate::ui::app::{App, Msg};
use crate::ui::widgets as w;

impl App {
    pub(crate) fn page_test(&mut self, ui: &mut Ui) {
        let p = self.p;
        if !self.install.is_valid() {
            w::card(ui, &p, "Тест стратегий", |ui| {
                w::small(ui, &p, "Папка zapret не найдена. Укажите её в «Настройки» → «Папка zapret».");
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
                "Тест по очереди запускает стратегии через winws.exe и проверяет доступность целей из utils\\targets.txt. Текущее ускорение будет остановлено, после прогона восстановлено.",
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
                if w::primary(ui, &p, label).clicked()
                    && !running
                    && !self.pages.test.auto_running
                    && !self.pages.test.auto_apply_running
                {
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

        w::card(ui, &p, "Автоподбор стратегии", |ui| {
            w::small(
                ui,
                &p,
                "Долгая проверка: берёт выбранную стратегию и перебирает desync-параметры (повторы, split-pos, seqovl, fake-файлы и т.п.), используя значения из всех .bat. Побеждает вариант, который проходит больше целей.",
            );
            if self.pages.test.auto_base.is_empty()
                || !self
                    .install
                    .strategies
                    .iter()
                    .any(|s| s.name.eq_ignore_ascii_case(&self.pages.test.auto_base))
            {
                self.pages.test.auto_base = self
                    .pages
                    .test
                    .selected
                    .and_then(|i| self.pages.test.rows.get(i))
                    .map(|r| r.strategy_name.clone())
                    .or_else(|| self.install.default_strategy().map(|s| s.name.clone()))
                    .unwrap_or_default();
            }
            ui.add_space(6.0);
            ui.horizontal_wrapped(|ui| {
                ui.label(RichText::new("База:").size(12.0).color(p.text_dim));
                egui::ComboBox::from_id_salt("auto_base")
                    .selected_text(if self.pages.test.auto_base.is_empty() {
                        "—".to_string()
                    } else {
                        self.pages.test.auto_base.clone()
                    })
                    .width(300.0)
                    .show_ui(ui, |ui| {
                        for s in &self.install.strategies {
                            ui.selectable_value(
                                &mut self.pages.test.auto_base,
                                s.name.clone(),
                                s.name.clone(),
                            );
                        }
                    });
                ui.add_space(8.0);
                ui.label(RichText::new("Глубина:").size(12.0).color(p.text_dim));
                for (limit, label) in [(40usize, "быстро"), (120, "долго"), (250, "очень долго")] {
                    if w::mode_button(
                        ui,
                        &p,
                        label,
                        self.pages.test.auto_limit == limit,
                    )
                    .clicked()
                    {
                        self.pages.test.auto_limit = limit;
                    }
                }
            });
            ui.add_space(6.0);
            ui.horizontal_wrapped(|ui| {
                if self.pages.test.auto_running {
                    if w::danger(ui, &p, "Остановить автоподбор").clicked() {
                        self.pages.test
                            .auto_cancel
                            .store(true, Ordering::SeqCst);
                    }
                } else if w::primary(ui, &p, "Запустить автоподбор").clicked()
                    && !self.pages.test.running
                {
                    self.start_auto_search();
                }
                if let Some(file) = self.pages.test.auto_best_file.clone() {
                    let busy = self.pages.test.auto_apply_running
                        || self.pages.test.auto_running
                        || self.pages.test.running;
                    if !busy && w::primary(ui, &p, "Применить и проверить").clicked() {
                        self.apply_auto_best(file.clone());
                    }
                    if !busy && w::secondary(ui, &p, "Запустить лучшую").clicked() {
                        self.run_auto_best(file.clone());
                    }
                    if !busy && w::secondary(ui, &p, "Установить службой").clicked() {
                        self.install_auto_best(file.clone());
                    }
                }
                if w::ghost(ui, &p, "Открыть папку zapret").clicked() {
                    crate::engine::sys::open_folder(std::path::Path::new(&self.install.root));
                }
            });
            if self.pages.test.auto_running {
                let ratio = if self.pages.test.auto_total > 0 {
                    self.pages.test.auto_done as f32 / self.pages.test.auto_total as f32
                } else {
                    0.0
                };
                w::progress_bar(ui, &p, ratio);
            }
            if !self.pages.test.auto_stage.is_empty() {
                ui.label(
                    RichText::new(&self.pages.test.auto_stage)
                        .size(11.5)
                        .color(p.text_muted),
                );
            }
            if self.pages.test.auto_apply_running {
                ui.label(
                    RichText::new("Применяю лучшую стратегию и проверяю цели…")
                        .size(11.5)
                        .color(p.warn),
                );
            } else if !self.pages.test.auto_apply_stage.is_empty() {
                ui.label(
                    RichText::new(&self.pages.test.auto_apply_stage)
                        .size(11.5)
                        .color(p.text_muted),
                );
            }
            if let Some(best) = &self.pages.test.auto_best {
                ui.label(
                    RichText::new(format!(
                        "Лучший: {} — {} из {}",
                        best.strategy_name,
                        best.passed(),
                        best.total()
                    ))
                    .size(12.0)
                    .color(if best.passed() > 0 { p.success } else { p.warn }),
                );
                if let Some(file) = &self.pages.test.auto_best_file {
                    w::muted(ui, &p, &format!("Сохранено: {}", file.to_string_lossy()));
                }
            }
        });

        let rows = self.pages.test.rows.clone();
        let group_names: Vec<String> = groups
            .iter()
            .filter(|g| self.pages.test.enabled.contains(&g.name))
            .map(|g| g.name.clone())
            .collect();
        let best_score = rows
            .iter()
            .filter(|r| r.best_candidate())
            .map(|r| r.score())
            .max();
        let best_count = best_score
            .map(|score| {
                rows.iter()
                    .filter(|r| r.best_candidate() && r.score() == score)
                    .count()
            })
            .unwrap_or(0);
        let mut apply_selected = false;
        w::card(ui, &p, "Результаты", |ui| {
            ui.label(
                RichText::new(&self.pages.test.summary)
                    .size(12.5)
                    .color(if rows.is_empty() { p.text_dim } else { p.text }),
            );
            w::muted(
                ui,
                &p,
                "«да»/«нет» — доступность цели, «1/4» — сколько целей группы прошло, «не старт» — winws.exe не запустился (наведите на строку). Нажмите строку, чтобы выбрать стратегию для запуска или установки службой.\n\nЛучшая: сначала больше прошедших целей, затем больше рабочих сервисов (≥60%). Официальный тест Flowseal делает 3 проверки на цель (HTTP/1.1, TLS 1.2, TLS 1.3), поэтому его лучшая может отличаться.",
            );
            ui.add_space(8.0);
            if rows.is_empty() {
                w::muted(ui, &p, "Результатов пока нет — запустите тест.");
                return;
            }
            if best_count > 1 {
                ui.label(
                    RichText::new(format!(
                        "Одинаковый лучший результат у {} стратегий — все отмечены ★",
                        best_count
                    ))
                    .size(11.5)
                    .color(p.accent),
                );
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
                let is_best = best_score
                    .map(|score| row.best_candidate() && row.score() == score)
                    .unwrap_or(false);
                let mut x = rect.left() + 8.0;
                let name = if is_best {
                    format!("★ {}", row.strategy_name)
                } else {
                    row.strategy_name.clone()
                };
                let name_clip = egui::Rect::from_min_max(
                    egui::pos2(x, rect.top()),
                    egui::pos2(rect.left() + widths.0 - 6.0, rect.bottom()),
                );
                ui.painter().with_clip_rect(name_clip).text(
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
                    let cell_clip = egui::Rect::from_min_max(
                        egui::pos2(x, rect.top()),
                        egui::pos2(x + widths.1 - 6.0, rect.bottom()),
                    );
                    ui.painter().with_clip_rect(cell_clip).text(
                        egui::pos2(x, rect.center().y),
                        egui::Align2::LEFT_CENTER,
                        text,
                        egui::FontId::new(11.5, egui::FontFamily::Proportional),
                        color,
                    );
                    x += widths.1;
                }
                let score = if !row.error.is_empty() {
                    "не старт".to_string()
                } else if row.total() == 0 {
                    "—".to_string()
                } else {
                    format!("{} / {}", row.passed(), row.total())
                };
                let color = if !row.error.is_empty() {
                    p.danger
                } else if row.total() == 0 {
                    p.text_muted
                } else if row.all_ok() {
                    p.success
                } else if row.passed() > 0 {
                    p.warn
                } else {
                    p.danger
                };
                let score_clip = egui::Rect::from_min_max(
                    egui::pos2(x, rect.top()),
                    egui::pos2(rect.right() - 4.0, rect.bottom()),
                );
                ui.painter().with_clip_rect(score_clip).text(
                    egui::pos2(x, rect.center().y),
                    egui::Align2::LEFT_CENTER,
                    score,
                    egui::FontId::new(11.5, egui::FontFamily::Proportional),
                    color,
                );
                let resp = if row.error.is_empty() {
                    resp
                } else {
                    resp.on_hover_text(row.error.as_str())
                };
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

    fn start_auto_search(&mut self) {
        if self.pages.test.auto_apply_running {
            return;
        }
        if !self.require_admin("Автоподбор стратегии") {
            return;
        }
        self.auto_heal_armed = false;
        if self.pages.test.enabled.is_empty() {
            self.notify("Выберите хотя бы одну группу целей.", LogLevel::Warn);
            return;
        }
        if self.install.strategies.is_empty() {
            self.notify("Стратегии не найдены.", LogLevel::Error);
            return;
        }
        if self.pages.test.auto_base.is_empty() {
            self.pages.test.auto_base = self
                .pick_strategy()
                .map(|s| s.name.clone())
                .unwrap_or_else(|| self.install.strategies[0].name.clone());
        }
        let install = self.install.clone();
        let base = self.pages.test.auto_base.clone();
        let limit = self.pages.test.auto_limit;
        let groups: Vec<String> = self.pages.test.enabled.iter().cloned().collect();
        let options = TesterOptions {
            timeout_ms: self.pages.test.timeout_ms,
            deep_check: self.pages.test.deep,
            restore_after: true,
        };
        self.pages.test.auto_running = true;
        self.pages.test.auto_done = 0;
        self.pages.test.auto_total = limit;
        self.pages.test.auto_candidates = 0;
        self.pages.test.auto_best = None;
        self.pages.test.auto_best_file = None;
        self.pages.test.auto_stage = "Генерация вариантов…".to_string();
        self.pages.test.auto_apply_stage.clear();
        self.pages.test.auto_cancel =
            std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let cancel = self.pages.test.auto_cancel.clone();
        self.spawn_bg(move |tx| {
            let tx2 = tx.clone();
            let cancelled = move || cancel.load(Ordering::SeqCst);
            let result = autotune::run_search(
                &install,
                &base,
                limit,
                &groups,
                &options,
                |done, total, name| {
                    let _ = tx2.send(Msg::AutoProgress(
                        done,
                        total,
                        name.to_string(),
                    ));
                },
                |_row| {},
                &cancelled,
            );
            let _ = tx.send(Msg::AutoDone(result));
        });
    }

    fn apply_auto_best(&mut self, file: std::path::PathBuf) {
        if !self.require_admin("Применение auto-стратегии") {
            return;
        }
        self.auto_heal_armed = false;
        if self.pages.test.enabled.is_empty() {
            self.notify("Выберите хотя бы одну группу целей.", LogLevel::Warn);
            return;
        }
        let groups: Vec<String> = self.pages.test.enabled.iter().cloned().collect();
        let options = TesterOptions {
            timeout_ms: self.pages.test.timeout_ms,
            deep_check: self.pages.test.deep,
            restore_after: true,
        };
        let install = self.install.clone();
        let previous = self.pick_strategy();
        let best_passed = self
            .pages
            .test
            .auto_best
            .as_ref()
            .map(|r| r.passed())
            .unwrap_or(0);
        self.pages.test.auto_apply_running = true;
        self.pages.test.auto_apply_stage.clear();
        self.spawn_bg(move |tx| {
            let result = autotune::apply_and_verify(
                &install,
                &file,
                best_passed,
                &groups,
                &options,
                previous,
            );
            let _ = tx.send(Msg::ApplyDone(result));
        });
    }

    fn run_auto_best(&mut self, file: std::path::PathBuf) {
        let name = file
            .file_stem()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| "AUTO".to_string());
        self.start_strategy(StrategyDef {
            file,
            name: name.clone(),
            short_name: "AUTO".to_string(),
            kind: StrategyKind::Other,
            description: "Автоподбор".to_string(),
            warning: false,
            order: 0,
        });
    }

    fn install_auto_best(&mut self, file: std::path::PathBuf) {
        let name = file
            .file_stem()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| "AUTO".to_string());
        self.install_service(StrategyDef {
            file,
            name: name.clone(),
            short_name: "AUTO".to_string(),
            kind: StrategyKind::Other,
            description: "Автоподбор".to_string(),
            warning: false,
            order: 0,
        });
    }

    fn start_test(&mut self) {
        if !self.require_admin("Тест стратегий") {
            return;
        }
        self.auto_heal_armed = false;
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
            let mut rows = tester::run(
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
            tester::sort_rows(&mut rows);
            let _ = tester::save_tsv(&rows, &groups_for_report, &options_for_report);
            let _ = tx.send(Msg::TestDone(rows));
        });
    }

    fn selected_test_strategy(&self) -> Option<crate::engine::install::StrategyDef> {
        let idx = self.pages.test.selected?;
        let row = self.pages.test.rows.get(idx)?;
        let install = &self.install;
        // Имя стратегии может содержать пробелы («general (ALT10)»), поэтому сначала
        // пробуем файл из строки отчёта, и только потом — полное имя.
        let by_file = std::path::Path::new(&row.strategy_file)
            .file_stem()
            .map(|s| s.to_string_lossy().to_string());
        by_file
            .as_deref()
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
                "Сейчас ускорение работает как служба. Остановить службу и запустить выбранную стратегию вручную?",
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
