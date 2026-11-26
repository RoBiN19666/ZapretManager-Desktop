//! Страница «Списки»: редактор, пометка изменений, массовая вставка.

use eframe::egui::{self, RichText, Ui};

use crate::engine::lists::{self, NoteKind};
use crate::engine::log::LogLevel;
use crate::engine::textfile;

use crate::ui::app::App;
use crate::ui::widgets as w;

impl App {
    pub(crate) fn current_list_path(&self) -> std::path::PathBuf {
        let st = &self.pages.lists;
        let file = match st.files.get(st.selected) {
            Some(f) => f,
            None => return std::path::PathBuf::new(),
        };
        if file.name == "ipset-all.txt" {
            self.install.ipset_file()
        } else if file.official {
            self.install.lists_dir().join(&file.name)
        } else {
            self.install.lists_dir().join(&file.name)
        }
    }

    pub(crate) fn select_list(&mut self, index: usize) {
        if index >= self.pages.lists.files.len() {
            return;
        }
        self.pages.lists.selected = index;
        self.load_lists();
    }

    fn save_list(&mut self) {
        let path = self.current_list_path();
        let file = match self.pages.lists.files.get(self.pages.lists.selected).cloned() {
            Some(f) => f,
            None => return,
        };
        let (fixed, fixed_count) = lists::fix_wildcards(&self.pages.lists.editor);
        let (clean, removed) = lists::remove_duplicates(&fixed);
        if fixed_count > 0 || removed > 0 {
            self.pages.lists.editor = clean;
            self.notify(
                format!(
                    "Список почищен: убрано повторов {}, заменено «*.» {}.",
                    removed, fixed_count
                ),
                LogLevel::Info,
            );
        }
        if file.tracked {
            let Some(mut lc) = self.pages.lists.changes.take() else {
                self.notify(
                    "Не удалось загрузить пометки изменений для этого списка. Перезагрузите страницу.",
                    LogLevel::Error,
                );
                return;
            };
            match lc.save_from_editor(&self.pages.lists.editor) {
                Some(text) => {
                    self.pages.lists.editor = text;
                    let added = lc.added.len();
                    self.pages.lists.changes = Some(lc);
                    self.pages.lists.dirty = false;
                    self.pages.lists.notes = lists::analyze(&self.pages.lists.editor);
                    self.pages.lists.info = self.lists_info();
                    self.notify(
                        format!(
                            "Сохранено. Ваши добавления ({}) помечены и переживут обновление zapret.",
                            added
                        ),
                        LogLevel::Ok,
                    );
                }
                None => {
                    let error = lc.last_error.clone();
                    self.pages.lists.changes = Some(lc);
                    self.notify(format!("Не удалось сохранить: {}", error), LogLevel::Error);
                }
            }
            return;
        }

        let entries = self
            .pages
            .lists
            .editor
            .lines()
            .filter(|l| {
                let t = l.trim();
                !t.is_empty() && !t.starts_with('#')
            })
            .count();
        if file.name == "list-general-user.txt" && entries == 0 {
            self.notify(
                "Список «Мои домены» нельзя сохранить пустым — winws.exe не запустится.",
                LogLevel::Error,
            );
            return;
        }
        if file.name == "ipset-all.txt" && entries == 0 {
            self.notify(
                "Пустой ipset-all.txt означает режим «любые IP». Если нужен полный список — переключите режим в «Настройках».",
                LogLevel::Warn,
            );
        }
        let text = self.pages.lists.editor.replace("\r\n", "\n").replace('\n', "\r\n");
        match textfile::write_text_atomic(&path, &text) {
            Ok(()) => {
                self.pages.lists.dirty = false;
                self.pages.lists.info = self.lists_info();
                self.notify(
                    format!("Список сохранён: {}. Перезапустите bypass, чтобы применить.", file.name),
                    LogLevel::Ok,
                );
            }
            Err(e) => self.notify(format!("Не удалось сохранить список: {}", e), LogLevel::Error),
        }
    }

    pub(crate) fn page_lists(&mut self, ui: &mut Ui) {
        let p = self.p;
        if !self.install.is_valid() {
            w::card(ui, &p, "Списки", |ui| {
                w::small(ui, &p, "Папка zapret не найдена. Укажите её на странице «Обзор».");
            });
            return;
        }
        let files = self.pages.lists.files.clone();
        let selected = self.pages.lists.selected;
        let file = files.get(selected).cloned();

        w::card(ui, &p, "Файлы списков", |ui| {
            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    ui.set_width(230.0);
                    egui::Frame::none()
                        .fill(p.editor)
                        .stroke(egui::Stroke::new(1.0, p.border))
                        .rounding(egui::Rounding::same(6))
                        .inner_margin(egui::Margin::same(4))
                        .show(ui, |ui| {
                            for (i, f) in files.iter().enumerate() {
                                let is_sel = i == selected;
                                let resp = ui.add(
                                    egui::Button::new(
                                        RichText::new(format!(
                                            "{}\n{}",
                                            f.label,
                                            if f.official { "официальный" } else { "мой список" }
                                        ))
                                        .size(12.0)
                                        .color(if is_sel { p.text } else { p.text_dim }),
                                    )
                                    .fill(if is_sel { p.selection } else { egui::Color32::TRANSPARENT })
                                    .min_size(egui::vec2(214.0, 42.0))
                                    .wrap(),
                                );
                                if resp.clicked() && !is_sel {
                                    if self.pages.lists.dirty {
                                        let target = i;
                                        self.confirm(
                                            "Несохранённые изменения",
                                            "В текущем файле есть несохранённые изменения. Переключиться и потерять их?",
                                            "Не сохранять",
                                            true,
                                            move |app| app.select_list(target),
                                        );
                                    } else {
                                        self.select_list(i);
                                    }
                                }
                            }
                        });
                });
                ui.add_space(10.0);
                ui.vertical(|ui| {
                    ui.set_width(ui.available_width());
                    if let Some(f) = &file {
                        let path = self.current_list_path();
                        ui.label(
                            RichText::new(format!(
                                "{}  —  {}",
                                path.to_string_lossy(),
                                f.hint
                            ))
                            .size(11.0)
                            .color(p.text_muted),
                        );
                        ui.add_space(4.0);
                        egui::Frame::none()
                            .fill(p.editor)
                            .stroke(egui::Stroke::new(1.0, p.border))
                            .rounding(egui::Rounding::same(6))
                            .inner_margin(egui::Margin::same(6))
                            .show(ui, |ui| {
                                ui.set_width(ui.available_width());
                                let resp = ui.add(
                                    egui::TextEdit::multiline(&mut self.pages.lists.editor)
                                        .font(egui::FontId::new(12.0, egui::FontFamily::Monospace))
                                        .desired_rows(16)
                                        .desired_width(f32::INFINITY)
                                        .code_editor(),
                                );
                                if resp.changed() && !self.pages.lists.loading {
                                    self.pages.lists.dirty = true;
                                    self.pages.lists.notes = lists::analyze(&self.pages.lists.editor);
                                    self.pages.lists.info = self.lists_info();
                                }
                            });
                        ui.add_space(6.0);
                        let issue_color = {
                            let notes = &self.pages.lists.notes;
                            if lists::count_kind(notes, NoteKind::Duplicate) > 0
                                || lists::count_kind(notes, NoteKind::Wildcard) > 0
                            {
                                p.warn
                            } else {
                                p.text_muted
                            }
                        };
                        ui.label(
                            RichText::new(&self.pages.lists.info)
                                .size(11.0)
                                .color(issue_color),
                        );
                        ui.add_space(6.0);
                        ui.horizontal_wrapped(|ui| {
                            if w::primary(ui, &p, "Сохранить").clicked() {
                                self.save_list();
                            }
                            if w::secondary(ui, &p, "Добавить строку").clicked() {
                                let sample = if f.name.to_lowercase().contains("ipset") {
                                    "203.0.113.113/32"
                                } else {
                                    "domain.example.com"
                                };
                                if !self.pages.lists.editor.is_empty()
                                    && !self.pages.lists.editor.ends_with('\n')
                                {
                                    self.pages.lists.editor.push_str("\r\n");
                                }
                                self.pages.lists.editor.push_str(sample);
                                self.pages.lists.dirty = true;
                            }
                            if w::ghost(ui, &p, "Открыть в блокноте").clicked() {
                                let path = self.current_list_path();
                                if !path.exists() && !f.template.is_empty() {
                                    let _ = textfile::write_text(&path, f.template);
                                }
                                let _ = crate::engine::process::spawn_hidden(
                                    "notepad.exe",
                                    &[&path.to_string_lossy()],
                                    None,
                                );
                            }
                            if !f.official && w::danger(ui, &p, "Сброс к шаблону").clicked() {
                                let template = f.template.to_string();
                                let label = f.label.to_string();
                                self.confirm(
                                    "Сбросить файл?",
                                    &format!(
                                        "Содержимое «{}» будет заменено шаблоном по умолчанию.",
                                        label
                                    ),
                                    "Сбросить",
                                    true,
                                    move |app| {
                                        app.pages.lists.editor = template.clone();
                                        app.pages.lists.dirty = true;
                                        app.save_list();
                                    },
                                );
                            }
                        });
                        ui.add_space(4.0);
                        ui.horizontal_wrapped(|ui| {
                            if w::ghost(ui, &p, "Вставить список").clicked() {
                                self.pages.lists.paste_open = !self.pages.lists.paste_open;
                            }
                            if w::ghost(ui, &p, "Причесать строки").clicked() {
                                let ip_mode = f.name.to_lowercase().contains("ipset");
                                let (text, changed, removed) =
                                    lists::tidy(&self.pages.lists.editor, ip_mode);
                                if changed == 0 && removed == 0 {
                                    self.notify(
                                        "Список и так в порядке: ничего чистить не нужно.",
                                        LogLevel::Info,
                                    );
                                } else {
                                    self.pages.lists.editor = text;
                                    self.pages.lists.dirty = true;
                                    self.pages.lists.notes = lists::analyze(&self.pages.lists.editor);
                                    self.pages.lists.info = self.lists_info();
                                    self.notify(
                                        format!(
                                            "Причёсано строк: {}, удалено мусора и повторов: {}. Не забудьте сохранить.",
                                            changed, removed
                                        ),
                                        LogLevel::Ok,
                                    );
                                }
                            }
                            let dups = lists::count_kind(&self.pages.lists.notes, NoteKind::Duplicate);
                            if dups > 0 && w::ghost(ui, &p, "Убрать дубликаты").clicked() {
                                let (text, removed) = lists::remove_duplicates(&self.pages.lists.editor);
                                self.pages.lists.editor = text;
                                self.pages.lists.dirty = true;
                                self.pages.lists.notes = lists::analyze(&self.pages.lists.editor);
                                self.pages.lists.info = self.lists_info();
                                self.notify(
                                    format!("Удалено повторов: {}. Не забудьте сохранить.", removed),
                                    LogLevel::Ok,
                                );
                            }
                        });
                        if self.pages.lists.paste_open {
                            ui.add_space(4.0);
                            ui.label(
                                RichText::new("Вставьте домены через запятую, пробел, точку с запятой или с новой строки.")
                                    .size(11.0)
                                    .color(p.text_muted),
                            );
                            ui.add(
                                egui::TextEdit::multiline(&mut self.pages.lists.paste_text)
                                    .desired_rows(4)
                                    .desired_width(f32::INFINITY)
                                    .font(egui::FontId::new(12.0, egui::FontFamily::Monospace)),
                            );
                            ui.horizontal(|ui| {
                                if w::primary(ui, &p, "Добавить").clicked() {
                                    self.paste_domains();
                                }
                                if w::ghost(ui, &p, "Отмена").clicked() {
                                    self.pages.lists.paste_open = false;
                                    self.pages.lists.paste_text.clear();
                                }
                            });
                        }
                        if !self.pages.lists.status.is_empty() {
                            w::muted(ui, &p, &self.pages.lists.status);
                        }
                    }
                });
            });
        });

        let tracked = file.as_ref().map(|f| f.tracked).unwrap_or(false);
        if tracked {
            let changes = self.pages.lists.changes.clone();
            if let Some(lc) = changes {
                w::card(ui, &p, "Мои изменения в официальном списке", |ui| {
                    ui.label(
                        RichText::new(format!(
                            "Файл: {}  •  строк официальной версии: {}",
                            lc.file_name,
                            lc.baseline.len()
                        ))
                        .size(11.5)
                        .color(p.text_muted),
                    );
                    ui.label(
                        RichText::new(format!(
                            "Добавлено вами: {}   •   удалено вами: {}   •   новых строк от разработчика в последней версии: {}",
                            lc.added.len(),
                            lc.removed.len(),
                            lc.news.len()
                        ))
                        .size(12.5)
                        .color(if lc.added.is_empty() && lc.removed.is_empty() {
                            p.text_dim
                        } else {
                            p.text
                        }),
                    );
                    ui.add_space(8.0);
                    ui.columns(2, |columns| {
                        columns[0].label(
                            RichText::new(format!("Добавлено вами ({})", lc.added.len()))
                                .size(11.5)
                                .color(p.success),
                        );
                        if lc.added.is_empty() {
                            columns[0].label(RichText::new("—").size(11.5).color(p.text_muted));
                        } else {
                            for entry in lc.added.iter().take(8) {
                                columns[0].label(
                                    RichText::new(entry)
                                        .size(11.5)
                                        .color(p.text_dim)
                                        .monospace(),
                                );
                            }
                        }
                        columns[1].label(
                            RichText::new(format!("Удалено вами ({})", lc.removed.len()))
                                .size(11.5)
                                .color(p.danger),
                        );
                        if lc.removed.is_empty() {
                            columns[1].label(RichText::new("—").size(11.5).color(p.text_muted));
                        } else {
                            for entry in lc.removed.iter().take(8) {
                                columns[1].label(
                                    RichText::new(entry)
                                        .size(11.5)
                                        .color(p.text_dim)
                                        .monospace(),
                                );
                            }
                        }
                    });
                    ui.add_space(8.0);
                    ui.horizontal_wrapped(|ui| {
                        let can_restore = !lc.removed.is_empty();
                        if ui
                            .add_enabled(can_restore, egui::Button::new("Вернуть удалённые"))
                            .clicked()
                        {
                            let count = lc.removed.len();
                            self.confirm(
                                "Вернуть удалённые строки?",
                                &format!("{} строк официального списка будут возвращены в файл.", count),
                                "Вернуть",
                                false,
                                |app| app.restore_removed_lines(),
                            );
                        }
                        let can_move = !lc.added.is_empty() && user_list_for(&lc.file_name).is_some();
                        if ui
                            .add_enabled(can_move, egui::Button::new("Перенести в мой список"))
                            .clicked()
                        {
                            let count = lc.added.len();
                            let target = user_list_for(&lc.file_name).unwrap_or_default().to_string();
                            self.confirm(
                                "Перенести добавления?",
                                &format!(
                                    "Ваши строки ({}) будут перенесены в {} — этот файл обновление никогда не перезаписывает. Из официального списка они исчезнут.",
                                    count, target
                                ),
                                "Перенести",
                                false,
                                |app| app.move_added_to_user(),
                            );
                        }
                        let can_accept = !lc.added.is_empty() || !lc.removed.is_empty();
                        if ui
                            .add_enabled(can_accept, egui::Button::new("Принять как официальное"))
                            .clicked()
                        {
                            self.confirm(
                                "Принять как официальное?",
                                "Текущее содержимое станет «официальной версией»: пометки «добавлено/удалено» сбросятся, и при следующем обновлении эти строки больше не будут переноситься отдельно.",
                                "Принять",
                                true,
                                |app| app.accept_as_official(),
                            );
                        }
                    });
                    w::hint(
                        ui,
                        &p,
                        "Строки «+» переносятся при обновлении zapret, «−» — снова удаляются. Новые строки разработчика при этом не теряются.",
                    );
                });
            }
        }
    }

    fn paste_domains(&mut self) {
        let file = match self.pages.lists.files.get(self.pages.lists.selected) {
            Some(f) => f.clone(),
            None => return,
        };
        let ip_mode = file.name.to_lowercase().contains("ipset");
        let (items, skipped) = lists::parse_batch(&self.pages.lists.paste_text, ip_mode);
        if items.is_empty() {
            self.notify(
                format!("Не нашёл ни одного домена во вставленном тексте (пропущено {}).", skipped),
                LogLevel::Warn,
            );
            return;
        }
        let existing: std::collections::HashSet<String> = self
            .pages
            .lists
            .editor
            .lines()
            .map(lists::normalize)
            .filter(|l| !l.is_empty())
            .collect();
        let mut added = 0usize;
        for item in &items {
            if existing.contains(&item.to_lowercase()) {
                continue;
            }
            if !self.pages.lists.editor.is_empty() && !self.pages.lists.editor.ends_with('\n') {
                self.pages.lists.editor.push_str("\r\n");
            }
            self.pages.lists.editor.push_str(item);
            self.pages.lists.editor.push_str("\r\n");
            added += 1;
        }
        if added == 0 {
            self.notify(
                format!("Все эти домены уже есть в списке (пропущено {}).", skipped),
                LogLevel::Warn,
            );
            return;
        }
        self.pages.lists.dirty = true;
        self.pages.lists.paste_open = false;
        self.pages.lists.paste_text.clear();
        self.pages.lists.notes = lists::analyze(&self.pages.lists.editor);
        self.pages.lists.info = self.lists_info();
        self.notify(
            format!("Добавлено доменов: {}, пропущено: {}. Не забудьте сохранить.", added, skipped),
            LogLevel::Ok,
        );
    }

    pub(crate) fn restore_removed_lines(&mut self) {
        let Some(mut lc) = self.pages.lists.changes.take() else {
            return;
        };
        let text = lc.restore_removed(&self.pages.lists.editor);
        self.pages.lists.editor = text;
        self.pages.lists.changes = Some(lc);
        self.pages.lists.notes = lists::analyze(&self.pages.lists.editor);
        self.pages.lists.info = self.lists_info();
        self.notify("Удалённые строки возвращены.", LogLevel::Ok);
    }

    pub(crate) fn move_added_to_user(&mut self) {
        let Some(mut lc) = self.pages.lists.changes.take() else {
            return;
        };
        let Some(user_name) = user_list_for(&lc.file_name) else {
            self.pages.lists.changes = Some(lc);
            self.notify(
                "Для этого списка нет пользовательского аналога — оставьте строки в размеченном блоке.",
                LogLevel::Warn,
            );
            return;
        };
        let path = self.install.lists_dir().join(user_name);
        let moved = lc.move_added_to_user_list(&path);
        if moved < 0 {
            self.pages.lists.changes = Some(lc);
            self.notify(
                "Не удалось перенести добавления в личный список — официальный файл не изменён.",
                LogLevel::Error,
            );
            return;
        }
        // убрать блок из официального файла
        let (official, _) = lists::split_block(
            &self
                .pages
                .lists
                .editor
                .replace("\r\n", "\n")
                .split('\n')
                .map(|s| s.to_string())
                .collect::<Vec<_>>(),
        );
        let text = lists::compose_text(&official, &[]);
        let _ = textfile::write_text(&lc.file_path, &text);
        self.pages.lists.changes = Some(lc);
        self.pages.lists.editor = text;
        self.pages.lists.dirty = false;
        self.pages.lists.notes = lists::analyze(&self.pages.lists.editor);
        self.pages.lists.info = self.lists_info();
        self.notify(
            format!("Перенесено строк: {} → {}. Перезапустите bypass.", moved, user_name),
            LogLevel::Ok,
        );
    }

    pub(crate) fn accept_as_official(&mut self) {
        let Some(mut lc) = self.pages.lists.changes.take() else {
            return;
        };
        lc.accept_as_official(&self.pages.lists.editor);
        self.pages.lists.changes = Some(lc);
        self.load_lists();
        self.notify("Текущее содержимое зафиксировано как официальное.", LogLevel::Ok);
    }
}

fn user_list_for(file: &str) -> Option<&'static str> {
    if file.eq_ignore_ascii_case("list-general.txt") {
        Some("list-general-user.txt")
    } else if file.eq_ignore_ascii_case("list-exclude.txt") {
        Some("list-exclude-user.txt")
    } else if file.eq_ignore_ascii_case("ipset-exclude.txt") {
        Some("ipset-exclude-user.txt")
    } else {
        None
    }
}
