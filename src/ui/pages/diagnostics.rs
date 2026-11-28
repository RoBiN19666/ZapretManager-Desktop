//! Страница «Диагностика».

use eframe::egui::{self, RichText, Ui};

use crate::engine::diagnostics::{self, Severity};
use crate::engine::log::LogLevel;
use crate::engine::ops::ZapretOps;

use crate::ui::app::App;
use crate::ui::widgets as w;

impl App {
    pub(crate) fn page_diagnostics(&mut self, ui: &mut Ui) {
        let p = self.p;
        w::card(ui, &p, "Проверка системы", |ui| {
            w::small(
                ui,
                &p,
                "Проверяются службы Windows, конфликтующие программы, прокси, DNS, файл hosts, целостность списков, файлы ускорения (blobs) и состояние драйвера WinDivert.",
            );
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                let running = self.pages.diagnostics.running;
                let label = if running { "Проверяем…" } else { "Проверить систему" };
                if w::primary(ui, &p, label).clicked() && !running {
                    self.run_diagnostics();
                }
                if !self.pages.diagnostics.results.is_empty()
                    && w::ghost(ui, &p, "Скопировать отчёт").clicked()
                {
                    let text = diagnostics::report_text(
                        &self.install,
                        &self.pages.diagnostics.results,
                    );
                    self.clipboard = Some(text);
                    self.notify("Отчёт скопирован в буфер обмена.", LogLevel::Ok);
                }
                if !self.pages.diagnostics.step.is_empty() {
                    ui.label(
                        RichText::new(&self.pages.diagnostics.step)
                            .size(12.0)
                            .color(p.text_muted),
                    );
                }
            });
        });

        let results = self.pages.diagnostics.results.clone();
        let summary = if results.is_empty() {
            if self.install.is_valid() {
                format!(
                    "{}  •  zapret {}  •  нажмите «Проверить систему»",
                    self.install.root,
                    if self.install.version.is_empty() {
                        "?"
                    } else {
                        &self.install.version
                    }
                )
            } else {
                "Проверка ещё не выполнялась.".to_string()
            }
        } else {
            self.pages.diagnostics.summary.clone()
        };
        let color = if results.iter().any(|r| r.severity == Severity::Error) {
            p.danger
        } else if results.iter().any(|r| r.severity == Severity::Warn) {
            p.warn
        } else {
            p.success
        };
        w::card(ui, &p, "Результаты", |ui| {
            ui.label(RichText::new(summary).size(12.5).color(if results.is_empty() { p.text_dim } else { color }));
            ui.add_space(8.0);
            if results.is_empty() {
                return;
            }
            let mut sorted = results.clone();
            sorted.sort_by_key(|r| std::cmp::Reverse(r.severity));
            for check in &sorted {
                let dot = match check.severity {
                    Severity::Error => p.danger,
                    Severity::Warn => p.warn,
                    Severity::Ok => p.success,
                };
                egui::Frame::NONE
                    .fill(p.editor)
                    .stroke(egui::Stroke::new(1.0_f32, p.border))
                    .corner_radius(egui::CornerRadius::same(7))
                    .inner_margin(egui::Margin::symmetric(12, 8))
                    .show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        ui.horizontal(|ui| {
                            w::status_dot(ui, dot, true);
                            ui.vertical(|ui| {
                                ui.label(
                                    RichText::new(&check.name)
                                        .size(12.5)
                                        .strong()
                                        .color(p.text),
                                );
                                ui.label(
                                    RichText::new(&check.message).size(12.0).color(p.text_dim),
                                );
                                if !check.hint.is_empty() {
                                    ui.label(
                                        RichText::new(&check.hint).size(11.5).color(p.text_muted),
                                    );
                                }
                            });
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                if !check.action_id.is_empty() && !check.action_text.is_empty() {
                                    if w::secondary(ui, &p, &check.action_text).clicked() {
                                        self.ask_fix(&check.action_id, &check.action_text);
                                    }
                                }
                            });
                        });
                    });
                ui.add_space(6.0);
            }
        });
    }

    fn run_diagnostics(&mut self) {
        let install = self.install.clone();
        self.pages.diagnostics.running = true;
        self.pages.diagnostics.results.clear();
        self.pages.diagnostics.step = "Выполняется…".to_string();
        self.spawn_bg(move |tx| {
            let ops = ZapretOps::new(install.clone());
            let results = diagnostics::run(&install, &ops, |step| {
                let _ = tx.send(crate::ui::app::Msg::DiagStep(step.to_string()));
            });
            let _ = tx.send(crate::ui::app::Msg::DiagDone(results));
        });
    }

    fn ask_fix(&mut self, action_id: &str, action_text: &str) {
        let id = action_id.to_string();
        if id != "discord" && !self.require_admin("Для этого действия") {
            return;
        }
        let (title, body) = match id.as_str() {
            "windivert" => (
                "Удалить драйвер WinDivert?",
                "Службы WinDivert и WinDivert14 будут остановлены и удалены. Это может затронуть другие программы, использующие WinDivert.",
            ),
            "conflicts" => (
                "Удалить конфликтующие службы?",
                "Будут остановлены и удалены службы GoodbyeDPI, discordfix_zapret, winws1 и winws2. Если вы ими пользуетесь — они перестанут запускаться.",
            ),
            "discord" => (
                "Очистить кэш Discord?",
                "Discord будет закрыт, а папки Cache, Code Cache и GPUCache удалены.",
            ),
            "hosts" => (
                "Убрать блок менеджера из hosts?",
                "Из файла hosts будет удалён блок, добавленный Zapret Manager. Остальные записи останутся.",
            ),
            "tcp" => (
                "Включить TCP timestamps?",
                "Параметр netsh interface tcp set global timestamps=enabled будет включён для всей системы.",
            ),
            _ => ("Выполнить действие?", action_text),
        };
        self.confirm(title, body, "Выполнить", true, move |app| {
            let ops = ZapretOps::new(app.install.clone());
            let id = id.clone();
            app.spawn_bg(move |tx| {
                let message = diagnostics::run_action(&id, &ops);
                let failed = message.to_lowercase().contains("не удалось")
                    || message.starts_with("Ошибка")
                    || message.contains("нужны права");
                let _ = tx.send(crate::ui::app::Msg::Notify(
                    message,
                    if failed { LogLevel::Error } else { LogLevel::Ok },
                ));
                let _ = tx.send(crate::ui::app::Msg::Text(String::new()));
            });
            app.pages.diagnostics.results.clear();
            app.pages.diagnostics.step = "Повторная проверка…".to_string();
        });
    }
}
