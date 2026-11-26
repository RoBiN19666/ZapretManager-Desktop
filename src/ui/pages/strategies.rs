//! Страница «Стратегии».

use eframe::egui::{self, RichText, Ui};

use crate::engine::log::LogLevel;
use crate::engine::tester;

use crate::ui::app::App;
use crate::ui::widgets as w;

impl App {
    fn favorite_strategies(&self) -> std::collections::HashSet<String> {
        self.ini
            .get("Favorites", "")
            .split(',')
            .map(|s| s.trim().to_lowercase())
            .filter(|s| !s.is_empty())
            .collect()
    }

    fn toggle_favorite(&mut self, name: &str) {
        let mut names: Vec<String> = self
            .ini
            .get("Favorites", "")
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();
        if let Some(pos) = names.iter().position(|n| n.eq_ignore_ascii_case(name)) {
            names.remove(pos);
        } else {
            names.push(name.to_string());
            names.sort_by_key(|n| n.to_lowercase());
        }
        self.ini.set("Favorites", &names.join(","));
        let _ = self.ini.save();
    }

    pub(crate) fn page_strategies(&mut self, ui: &mut Ui) {
        let p = self.p;
        if !self.install.is_valid() {
            w::card(ui, &p, "Стратегии не найдены", |ui| {
                w::small(
                    ui,
                    &p,
                    "Укажите папку zapret на странице «Обзор» — менеджер найдёт все general*.bat.",
                );
            });
            return;
        }
        let strategies = self.install.strategies.clone();
        let favorites = self.favorite_strategies();
        let query = self.pages.strategies.search.trim().to_lowercase();
        let visible: Vec<usize> = strategies
            .iter()
            .enumerate()
            .filter(|(_, s)| {
                if self.pages.strategies.only_favorites
                    && !favorites.contains(&s.name.to_lowercase())
                {
                    return false;
                }
                query.is_empty()
                    || s.name.to_lowercase().contains(&query)
                    || s.description.to_lowercase().contains(&query)
            })
            .map(|(idx, _)| idx)
            .collect();
        let mut selected = self
            .pages
            .strategies
            .selected
            .min(strategies.len().saturating_sub(1));
        if !visible.is_empty() && !visible.contains(&selected) {
            selected = visible[0];
            self.pages.strategies.selected = selected;
        }
        let strategy = strategies.get(selected).cloned();
        let preview = strategy
            .as_ref()
            .map(|s| tester::strategy_command(&self.install, s))
            .unwrap_or_default();
        let caption = strategy
            .as_ref()
            .map(|s| format!("{}  •  {}", s.name, s.description))
            .unwrap_or_default();
        let warning = strategy
            .as_ref()
            .filter(|s| s.warning)
            .map(|_| "Внимание: авторы помечают эту стратегию как «NOT RECOMMENDED».".to_string())
            .unwrap_or_default();

        w::card(ui, &p, "Доступные стратегии", |ui| {
            let active = self.state.matched_strategy.clone();
            ui.horizontal(|ui| {
                ui.add(
                    egui::TextEdit::singleline(&mut self.pages.strategies.search)
                        .hint_text("Поиск по названию или описанию…")
                        .desired_width(280.0),
                );
                let mut only = self.pages.strategies.only_favorites;
                if ui.checkbox(&mut only, "Только избранные").changed() {
                    self.pages.strategies.only_favorites = only;
                }
                ui.label(
                    RichText::new(format!("{} из {}", visible.len(), strategies.len()))
                        .size(11.5)
                        .color(p.text_muted),
                );
            });
            ui.add_space(6.0);
            let mut toggle_fav: Option<String> = None;
            egui::Frame::none()
                .fill(p.editor)
                .stroke(egui::Stroke::new(1.0, p.border))
                .rounding(egui::Rounding::same(7))
                .inner_margin(egui::Margin::same(4))
                .show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    egui::ScrollArea::vertical()
                        .max_height(330.0)
                        .auto_shrink([false, false])
                        .show(ui, |ui| {
                            if visible.is_empty() {
                                w::muted(ui, &p, "Ничего не найдено.");
                                return;
                            }
                            for idx in visible.iter().copied() {
                                let s = &strategies[idx];
                                let is_selected = idx == selected;
                                let (rect, resp) = ui.allocate_exact_size(
                                    egui::vec2(ui.available_width(), 38.0),
                                    egui::Sense::click(),
                                );
                                let star_rect = egui::Rect::from_center_size(
                                    egui::pos2(rect.right() - 13.0, rect.center().y),
                                    egui::vec2(22.0, 22.0),
                                );
                                let star_resp = ui.interact(
                                    star_rect,
                                    ui.id().with(("favorite", idx)),
                                    egui::Sense::click(),
                                );
                                if is_selected {
                                    ui.painter().rect_filled(rect, egui::Rounding::same(6), p.selection);
                                } else if resp.hovered() {
                                    ui.painter().rect_filled(rect, egui::Rounding::same(6), p.hover);
                                }
                                let is_active = !active.is_empty() && s.name.eq_ignore_ascii_case(&active);
                                ui.painter().text(
                                    egui::pos2(rect.left() + 12.0, rect.top() + 12.0),
                                    egui::Align2::LEFT_TOP,
                                    &s.name,
                                    egui::FontId::new(13.0, egui::FontFamily::Name("bold".into())),
                                    if is_selected { p.text } else { p.text_dim },
                                );
                                ui.painter().text(
                                    egui::pos2(rect.left() + 12.0, rect.top() + 26.0),
                                    egui::Align2::LEFT_TOP,
                                    &s.description,
                                    egui::FontId::new(10.5, egui::FontFamily::Proportional),
                                    p.text_muted,
                                );
                                let is_fav = favorites.contains(&s.name.to_lowercase());
                                ui.painter().text(
                                    star_rect.center(),
                                    egui::Align2::CENTER_CENTER,
                                    if is_fav { "★" } else { "☆" },
                                    egui::FontId::new(16.0, egui::FontFamily::Proportional),
                                    if is_fav { p.accent } else { p.text_muted },
                                );
                                let badge_color = match s.kind {
                                    crate::engine::install::StrategyKind::Base => p.accent,
                                    crate::engine::install::StrategyKind::Alt => p.text_dim,
                                    crate::engine::install::StrategyKind::Exp => p.warn,
                                    crate::engine::install::StrategyKind::Simple => p.success,
                                    crate::engine::install::StrategyKind::FakeTls => p.text_dim,
                                    crate::engine::install::StrategyKind::Other => p.text_muted,
                                };
                                let badge = s.kind.label();
                                let galley = ui.painter().layout_no_wrap(
                                    badge.to_string(),
                                    egui::FontId::new(10.0, egui::FontFamily::Proportional),
                                    badge_color,
                                );
                                let bw = galley.rect.width() + 12.0;
                                let brect = egui::Rect::from_min_size(
                                    egui::pos2(rect.right() - bw - 36.0, rect.center().y - 9.0),
                                    egui::vec2(bw, 18.0),
                                );
                                ui.painter().rect_filled(brect, egui::Rounding::same(9), p.surface_alt);
                                ui.painter().galley(brect.min + egui::vec2(6.0, 2.0), galley, badge_color);
                                if is_active {
                                    ui.painter().circle_filled(
                                        egui::pos2(brect.left() - 14.0, rect.center().y),
                                        4.0,
                                        p.success,
                                    );
                                }
                                if star_resp.clicked() {
                                    toggle_fav = Some(s.name.clone());
                                } else if resp.clicked() {
                                    self.pages.strategies.selected = idx;
                                }
                            }
                        });
                });
            if let Some(name) = toggle_fav {
                self.toggle_favorite(&name);
            }
            ui.add_space(8.0);
            if !warning.is_empty() {
                ui.label(RichText::new(&warning).size(12.0).color(p.warn));
            }
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                if w::primary(ui, &p, "Запустить").clicked() {
                    if let Some(s) = strategy.clone() {
                        if self.state.service_installed && self.state.service_status == "работает" {
                            self.confirm(
                                "Служба запущена",
                                "Стратегия сейчас работает как служба zapret. Остановить службу и запустить выбранную стратегию вручную?",
                                "Остановить службу и запустить",
                                false,
                                move |app| {
                                    app.stop_bypass();
                                    app.start_strategy(s.clone());
                                },
                            );
                        } else {
                            self.start_strategy(s);
                        }
                    }
                }
                if w::secondary(ui, &p, "Установить как службу").clicked() {
                    if let Some(s) = strategy.clone() {
                        self.confirm(
                            "Установить службу?",
                            &format!(
                                "Стратегия «{}» будет установлена как служба Windows «zapret» и будет запускаться автоматически при загрузке. Текущий запущенный bypass будет остановлен.",
                                s.name
                            ),
                            "Установить",
                            false,
                            move |app| app.install_service(s.clone()),
                        );
                    }
                }
                if w::ghost(ui, &p, "Открыть .bat").clicked() {
                    if let Some(s) = strategy.as_ref() {
                        let file = s.file.to_string_lossy().to_string();
                        let _ = crate::engine::process::spawn_hidden("notepad.exe", &[&file], None);
                    }
                }
                if w::ghost(ui, &p, "Копировать команду").clicked() {
                    if !preview.is_empty() {
                        self.clipboard = Some(preview.clone());
                        self.notify("Команда скопирована в буфер обмена.", LogLevel::Ok);
                    }
                }
            });
            ui.add_space(10.0);
            ui.label(RichText::new(&caption).size(11.5).color(p.text_muted));
        });

        w::card(ui, &p, "Команда запуска (winws.exe)", |ui| {
            w::code_block(ui, &p, &preview);
        });
    }
}
