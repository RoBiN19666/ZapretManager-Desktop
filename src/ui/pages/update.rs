//! Страница «Обновление».

use eframe::egui::{self, RichText, Ui};

use crate::engine::log::LogLevel;
use crate::engine::update::{self, UpdateOptions};

use crate::ui::app::{App, Msg};
use crate::ui::widgets as w;

impl App {
    pub(crate) fn page_update(&mut self, ui: &mut Ui) {
        let p = self.p;
        let valid = self.install.is_valid();

        w::card(ui, &p, "Установленная версия", |ui| {
            ui.label(
                RichText::new(if valid {
                    format!(
                        "zapret {}",
                        if self.install.version.is_empty() {
                            "— версия не определена"
                        } else {
                            &self.install.version
                        }
                    )
                } else {
                    "zapret не найден".to_string()
                })
                .size(15.0)
                .strong()
                .color(p.text),
            );
            ui.label(
                RichText::new(if valid {
                    self.install.root.clone()
                } else {
                    "Установите zapret или укажите папку в «Настройки» → «Папка zapret».".to_string()
                })
                .size(11.5)
                .color(p.text_muted),
            );
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                if w::secondary(ui, &p, "Проверить обновления").clicked() {
                    self.check_updates();
                }
                if !valid && w::ghost(ui, &p, "Открыть настройки папки").clicked() {
                    self.page = crate::ui::app::Page::Settings;
                    self.on_navigate(crate::ui::app::Page::Settings);
                }
                if !self.pages.update.stage.is_empty() {
                    ui.label(
                        RichText::new(&self.pages.update.stage)
                            .size(11.5)
                            .color(p.text_muted),
                    );
                }
            });
        });

        let release = self
            .update_state
            .as_ref()
            .and_then(|s| s.release.clone());
        let update_available = self
            .update_state
            .as_ref()
            .map(|s| s.ok && (s.update_available || s.local_unknown))
            .unwrap_or(false);
        w::card(ui, &p, "Доступное обновление", |ui| {
            if let Some(rel) = &release {
                let title = if rel.name.is_empty() {
                    rel.tag.clone()
                } else {
                    rel.name.clone()
                };
                ui.label(RichText::new(title).size(15.0).strong().color(p.text));
                if !rel.published.is_empty() {
                    ui.label(
                        RichText::new(format!(
                            "Опубликовано: {}",
                            format_published(&rel.published)
                        ))
                        .size(11.5)
                        .color(p.text_muted),
                    );
                }
                ui.add_space(6.0);
                let notes = rel.clean_body();
                if !self.pages.update.notes.is_empty() && self.pages.update.notes != notes {
                    // оставляем актуальные заметки
                }
                let mut notes_text = if notes.is_empty() {
                    "Описание изменений недоступно.".to_string()
                } else {
                    notes
                };
                egui::Frame::none()
                    .fill(p.editor)
                    .stroke(egui::Stroke::new(1.0, p.border))
                    .rounding(egui::Rounding::same(6))
                    .inner_margin(egui::Margin::same(8))
                    .show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        egui::ScrollArea::vertical().max_height(150.0).show(ui, |ui| {
                            ui.add(
                                egui::TextEdit::multiline(&mut notes_text)
                                    .desired_width(f32::INFINITY)
                                    .font(egui::TextStyle::Small)
                                    .interactive(false),
                            );
                        });
                    });
                ui.add_space(8.0);
                w::toggle_row(
                    ui,
                    &p,
                    "Создать резервную копию",
                    "Перед установкой сохранить текущую папку zapret рядом",
                    &mut self.pages.update.options.create_backup,
                );
                w::toggle_row(
                    ui,
                    &p,
                    "Перезапустить ускорение после обновления",
                    "",
                    &mut self.pages.update.options.restart_after,
                );
                w::toggle_row(
                    ui,
                    &p,
                    "Обновлять официальные списки",
                    "выкл. — останутся ваши list-general / list-google / list-exclude",
                    &mut self.pages.update.options.update_official_lists,
                );
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    let install_label = if update_available {
                        "Скачать и установить"
                    } else {
                        "Переустановить версию"
                    };
                    if w::primary(ui, &p, install_label).clicked() {
                        let release = rel.clone();
                        let options = self.pages.update.options.clone();
                        self.confirm(
                            "Обновить zapret?",
                            &format!(
                                "Будет скачана и установлена версия {} с официального репозитория Flowseal. Ваши пользовательские списки, IP-лист и настройки сохранятся.\n\nВо время обновления ускорение будет остановлено.",
                                release.tag
                            ),
                            "Обновить",
                            false,
                            move |app| app.install_update(release.clone(), options.clone()),
                        );
                    }
                    if w::ghost(ui, &p, "Страница релиза").clicked() && !rel.html_url.is_empty() {
                        crate::engine::sys::open_url(&rel.html_url);
                    }
                });
                if self.pages.update.running {
                    w::progress_bar(ui, &p, self.pages.update.progress as f32 / 100.0);
                }
                if !self.pages.update.hint.is_empty() {
                    w::muted(ui, &p, &self.pages.update.hint);
                }
            } else {
                let text = match &self.update_state {
                    Some(res) if !res.ok => format!("Не удалось проверить обновления: {}", res.error),
                    Some(res) if !res.update_available => {
                        format!("У вас последняя версия {}.", res.local_version)
                    }
                    _ => "Проверяем…".to_string(),
                };
                ui.label(RichText::new(text).size(12.5).color(p.text_dim));
                if w::secondary(ui, &p, "Проверить обновления").clicked() {
                    self.check_updates();
                }
            }
        });

        w::card(ui, &p, "Обновление Zapret Manager", |ui| {
            ui.label(
                RichText::new(format!("Установлена версия {}", crate::APP_VERSION))
                    .size(13.0)
                    .color(p.text),
            );
            match &self.self_update {
                Some(info) if info.ok && info.available => {
                    ui.label(
                        RichText::new(format!("Доступна версия {}.", info.remote))
                            .size(12.0)
                            .color(p.warn),
                    );
                    if !self.pages.update.running
                        && w::primary(ui, &p, "Обновить менеджер").clicked()
                    {
                        self.apply_self_update();
                    }
                }
                Some(info) if info.ok => {
                    ui.label(
                        RichText::new("Установлена актуальная версия.")
                            .size(12.0)
                            .color(p.text_muted),
                    );
                }
                Some(info) => {
                    ui.label(
                        RichText::new(format!("Не удалось проверить: {}", info.error))
                            .size(12.0)
                            .color(p.text_dim),
                    );
                }
                None => {
                    ui.label(RichText::new("Проверка не выполнялась.").size(12.0).color(p.text_dim));
                }
            }
            ui.add_space(6.0);
            if w::ghost(ui, &p, "Проверить").clicked() {
                let tx = self.tx.clone();
                self.spawn_bg(move |_tx| {
                    let info = update::self_check(true);
                    let _ = tx.send(Msg::SelfCheck(Box::new(info)));
                });
            }
            w::hint(
                ui,
                &p,
                "Источник: github.com/RoBiN19666/ZapretManager-Desktop • GitHub Releases, файл ZapretManager.exe с SHA-256",
            );
        });

        w::card(ui, &p, "Версия zapret (сборки Flowseal)", |ui| {
            w::small(
                ui,
                &p,
                "Можно поставить не только последнюю, но и любую другую версию — например, откатиться назад, если в новой что-то сломалось. Списки и настройки сохраняются.",
            );
            ui.add_space(8.0);
            let names: Vec<String> = self
                .pages
                .update
                .releases
                .iter()
                .map(|r| {
                    if !self.install.version.is_empty() && r.tag == self.install.version {
                        format!("{}   •   установлена", r.tag)
                    } else {
                        r.tag.clone()
                    }
                })
                .collect();
            let selected_tag = self
                .pages
                .update
                .releases
                .get(self.pages.update.selected_release())
                .map(|r| r.tag.clone());
            ui.horizontal(|ui| {
                egui::ComboBox::from_id_salt("release_pick")
                    .selected_text(
                        names
                            .get(self.pages.update.selected_release())
                            .cloned()
                            .unwrap_or_else(|| "Список версий ещё не загружен.".to_string()),
                    )
                    .width(280.0)
                    .show_ui(ui, |ui| {
                        for (i, name) in names.iter().enumerate() {
                            ui.selectable_value(
                                &mut self.pages.update.releases_idx,
                                i,
                                name,
                            );
                        }
                    });
                if w::secondary(ui, &p, "Загрузить список").clicked() {
                    self.spawn_bg(move |tx| {
                        let result = update::list_releases(update::ZAPRET_REPO, 20);
                        let _ = tx.send(Msg::Releases(result));
                    });
                }
                let can_install = selected_tag.is_some();
                if ui
                    .add_enabled(can_install, egui::Button::new("Установить версию"))
                    .clicked()
                {
                    if let Some(tag) = selected_tag.clone() {
                        self.install_by_tag(tag);
                    }
                }
            });
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                ui.label(RichText::new("Или тег вручную:").size(12.0).color(p.text_dim));
                ui.add(
                    egui::TextEdit::singleline(&mut self.pages.update.manual_tag)
                        .desired_width(140.0)
                        .hint_text("1.9.9d"),
                );
                if w::secondary(ui, &p, "Поставить по тегу").clicked() {
                    let tag = self.pages.update.manual_tag.trim().to_string();
                    let valid_tag = !tag.is_empty()
                        && tag
                            .trim_start_matches(['v', 'V'])
                            .chars()
                            .next()
                            .map(|c| c.is_ascii_digit())
                            .unwrap_or(false)
                        && tag
                            .chars()
                            .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == 'v' || c == 'V');
                    if !valid_tag {
                        self.notify(format!("Тег выглядит некорректно: {}", tag), LogLevel::Error);
                    } else if tag.eq_ignore_ascii_case(&self.install.version) {
                        self.notify("Эта версия уже установлена.", LogLevel::Warn);
                    } else {
                        self.install_by_tag(tag);
                    }
                }
            });
            if !self.pages.update.hint.is_empty() {
                w::muted(ui, &p, &self.pages.update.hint);
            }
        });

        w::card(ui, &p, "Параметры", |ui| {
            let mut check = self.ini.get_bool("CheckUpdatesOnStartup", true);
            if w::toggle_row(
                ui,
                &p,
                "Проверять обновления при запуске менеджера",
                "Запрос к GitHub при каждом старте программы",
                &mut check,
            ) {
                self.ini.set_bool("CheckUpdatesOnStartup", check);
                let _ = self.ini.save();
            }
        });
    }

    pub(crate) fn install_update(&mut self, release: update::Release, options: UpdateOptions) {
        let install = self.install.clone();
        self.pages.update.running = true;
        self.pages.update.progress = 0;
        self.spawn_bg(move |tx| {
            let tx2 = tx.clone();
            let result = update::update_now(&install, &release, &options, |percent, stage| {
                let _ = tx2.send(Msg::Progress("update", percent, stage.to_string()));
            });
            let _ = tx.send(Msg::UpdateDone(result));
        });
    }

    fn install_by_tag(&mut self, tag: String) {
        let current = self.install.version.clone();
        let older = !current.is_empty()
            && crate::util::version_cmp(&tag, &current) == std::cmp::Ordering::Less;
        self.confirm(
            if older {
                "Откатиться на версию?"
            } else {
                "Установить версию?"
            },
            &format!(
                "Будет скачана и установлена сборка {} с официального репозитория Flowseal{}. Ваши списки, IP-лист и настройки сохраняются, ускорение на время установки останавливается.",
                tag,
                if older {
                    format!(" (это более старая версия, чем установленная {})", current)
                } else {
                    String::new()
                }
            ),
            "Установить",
            false,
            move |app| {
                let install = app.install.clone();
                let options = app.pages.update.options.clone();
                let tag = tag.clone();
                app.pages.update.running = true;
                app.spawn_bg(move |tx| {
                    let tx2 = tx.clone();
                    let result = update::release_by_tag(update::ZAPRET_REPO, &tag).and_then(|rel| {
                        update::update_now(&install, &rel, &options, |percent, stage| {
                            let _ = tx2.send(Msg::Progress("update", percent, stage.to_string()));
                        })
                    });
                    let _ = tx.send(Msg::UpdateDone(result));
                });
            },
        );
    }

    pub(crate) fn fresh_install(&mut self, parent: std::path::PathBuf) {
        self.pages.update.running = true;
        self.pages.update.progress = 0;
        self.pages.update.stage = "Получение информации о релизе…".to_string();
        self.spawn_bg(move |tx| {
            let tx2 = tx.clone();
            let result = update::get_latest_release(update::ZAPRET_REPO).and_then(|release| {
                update::install_fresh(&release, &parent, |percent, stage| {
                    let _ = tx2.send(Msg::Progress("update", percent, stage.to_string()));
                })
            });
            match result {
                Ok(install) => {
                    let _ = tx.send(Msg::Installed(Box::new(install)));
                }
                Err(e) => {
                    let _ = tx.send(Msg::UpdateDone(Err(e)));
                }
            }
        });
    }

    pub(crate) fn apply_self_update(&mut self) {
        let Some(info) = self.self_update.clone() else {
            return;
        };
        self.pages.update.running = true;
        self.pages.update.hint = "Скачивание обновления менеджера…".to_string();
        self.spawn_bg(move |tx| {
            let result = update::self_apply(&info);
            let _ = tx.send(Msg::SelfApplyDone(result));
        });
    }
}

impl crate::ui::app::UpdatePageState {
    pub(crate) fn selected_release(&self) -> usize {
        self.releases_idx.min(self.releases.len().saturating_sub(1))
    }
}

fn format_published(raw: &str) -> String {
    let bytes = raw.as_bytes();
    if raw.len() >= 16 && bytes.get(4) == Some(&b'-') {
        let year = &raw[0..4];
        let month = &raw[5..7];
        let day = &raw[8..10];
        let time = &raw[11..16];
        return format!("{}.{}.{} {}", day, month, year, time);
    }
    raw.to_string()
}