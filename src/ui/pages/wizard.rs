//! Мастер первой настройки (модальное окно).

use eframe::egui::{self, RichText};

use crate::engine::log::LogLevel;
use crate::engine::install::ZapretInstall;
use crate::engine::{paths, tgproxy, update};

use crate::ui::app::{App, Msg, WizardState};
use crate::ui::widgets as w;

impl App {
    pub(crate) fn open_wizard(&mut self) {
        if self.pages.wizard.is_some() {
            return;
        }
        let local = std::env::var("LOCALAPPDATA").unwrap_or_default();
        let default_zapret = std::path::Path::new(&local)
            .join("ZapretManager")
            .join("zapret")
            .to_string_lossy()
            .to_string();
        self.pages.wizard = Some(WizardState {
            zapret_dir: default_zapret,
            tg_dir: tgproxy::default_dir().to_string_lossy().to_string(),
            install_zapret: !self.install.is_valid(),
            install_tg: !tgproxy::get_state(&self.ini).installed,
            stage: String::new(),
            progress: 0,
            running: false,
            error: String::new(),
            done: false,
        });
    }

    pub(crate) fn show_wizard(&mut self, ctx: &egui::Context) {
        let Some(mut wizard) = self.pages.wizard.take() else {
            return;
        };
        let p = self.p;
        let mut close = false;
        let mut install = false;
        let mut picked_root: Option<String> = None;
        egui::Window::new("Первая настройка Zapret Manager")
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .default_width(660.0)
            .frame(
                egui::Frame::window(&ctx.style())
                    .fill(p.surface)
                    .stroke(egui::Stroke::new(1.0_f32, p.border_strong)),
            )
            .show(ctx, |ui| {
                ui.label(
                    RichText::new(
                        "Если zapret уже установлен, просто укажите его папку — мастер закроется. Если нет, можно скачать официальную сборку zapret и TG WS Proxy. Папку также можно изменить позже в «Настройки» → «Папка zapret».",
                    )
                    .size(12.0)
                    .color(p.text_dim),
                );
                ui.add_space(10.0);
                // zapret
                ui.label(RichText::new("Ускорение (для Discord и YouTube)").size(13.0).strong().color(p.text));
                let status = if self.install.is_valid() {
                    format!(
                        "Уже установлен: {} (версия {})",
                        self.install.root,
                        if self.install.version.is_empty() {
                            "?"
                        } else {
                            &self.install.version
                        }
                    )
                } else {
                    "Не найден — можно скачать и установить сейчас.".to_string()
                };
                ui.label(
                    RichText::new(status)
                        .size(11.5)
                        .color(if self.install.is_valid() { p.success } else { p.warn }),
                );
                if self.install.is_valid()
                    && !wizard.running
                    && w::ghost(ui, &p, "Указать другую папку…").clicked()
                {
                    if let Some(dir) = rfd::FileDialog::new()
                        .set_title("Выберите папку zapret (где лежит service.bat)")
                        .pick_folder()
                    {
                        if let Some(dir) = ZapretInstall::resolve_valid_dir(&dir) {
                            picked_root = Some(dir.to_string_lossy().to_string());
                        } else {
                            self.notify(
                                "В выбранной папке нет service.bat и bin\\winws.exe",
                                LogLevel::Error,
                            );
                        }
                    }
                }
                if !self.install.is_valid() {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("Куда скачать:").size(11.5).color(p.text_dim));
                        ui.add(
                            egui::TextEdit::singleline(&mut wizard.zapret_dir)
                                .desired_width(360.0),
                        );
                        if w::ghost(ui, &p, "Обзор…").clicked() {
                            if let Some(dir) = rfd::FileDialog::new()
                                .set_title("Куда установить zapret?")
                                .pick_folder()
                            {
                                wizard.zapret_dir = dir.to_string_lossy().to_string();
                            }
                        }
                    });
                    ui.horizontal(|ui| {
                        if !wizard.running
                            && w::secondary(ui, &p, "Указать уже установленную папку…").clicked()
                        {
                            if let Some(dir) = rfd::FileDialog::new()
                                .set_title("Выберите папку zapret (где лежит service.bat)")
                                .pick_folder()
                            {
                                if let Some(dir) = ZapretInstall::resolve_valid_dir(&dir) {
                                    picked_root = Some(dir.to_string_lossy().to_string());
                                } else {
                                    self.notify(
                                        "В выбранной папке нет service.bat и bin\\winws.exe",
                                        LogLevel::Error,
                                    );
                                }
                            }
                        }
                    });
                    w::toggle_row(
                        ui,
                        &p,
                        "Скачать и установить (последняя версия)",
                        "",
                        &mut wizard.install_zapret,
                    );
                }
                ui.add_space(8.0);
                // tg
                ui.label(RichText::new("TG WS Proxy (ускорение Telegram через WebSocket)").size(13.0).strong().color(p.text));
                let tg_state = tgproxy::get_state(&self.ini);
                let tg_status = if tg_state.installed {
                    format!(
                        "Уже установлен: {}{}",
                        tg_state.dir.to_string_lossy(),
                        if tg_state.version.is_empty() {
                            String::new()
                        } else {
                            format!(" (версия {})", tg_state.version)
                        }
                    )
                } else {
                    "Не найден. Прокси слушает 127.0.0.1:1443 и сворачивается в трей.".to_string()
                };
                ui.label(
                    RichText::new(tg_status)
                        .size(11.5)
                        .color(if tg_state.installed { p.success } else { p.warn }),
                );
                if !tg_state.installed {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("Куда установить:").size(11.5).color(p.text_dim));
                        ui.add(
                            egui::TextEdit::singleline(&mut wizard.tg_dir)
                                .desired_width(360.0),
                        );
                        if w::ghost(ui, &p, "Обзор…").clicked() {
                            if let Some(dir) = rfd::FileDialog::new()
                                .set_title("Куда установить TG WS Proxy?")
                                .pick_folder()
                            {
                                wizard.tg_dir = dir.to_string_lossy().to_string();
                            }
                        }
                    });
                    w::toggle_row(
                        ui,
                        &p,
                        "Скачать и установить (~21 МБ)",
                        "",
                        &mut wizard.install_tg,
                    );
                }
                ui.add_space(10.0);
                if wizard.running {
                    w::progress_bar(ui, &p, wizard.progress as f32 / 100.0);
                    ui.label(
                        RichText::new(&wizard.stage)
                            .size(11.5)
                            .color(p.text_muted),
                    );
                }
                if !wizard.error.is_empty() {
                    ui.label(RichText::new(&wizard.error).size(12.0).color(p.danger));
                }
                if wizard.done {
                    ui.label(
                        RichText::new("Готово.")
                            .size(12.5)
                            .color(p.success),
                    );
                }
                ui.add_space(8.0);
                let nothing_to_install = !wizard.install_zapret && !wizard.install_tg;
                ui.horizontal(|ui| {
                    if w::secondary(ui, &p, "Пропустить").clicked() && !wizard.running {
                        close = true;
                    }
                    if wizard.done {
                        if w::primary(ui, &p, "Закрыть").clicked() {
                            close = true;
                        }
                    } else if nothing_to_install {
                        if w::primary(ui, &p, "Готово").clicked() && !wizard.running {
                            close = true;
                        }
                    } else {
                        let label = "Установить и продолжить";
                        if w::primary(ui, &p, label).clicked() && !wizard.running {
                            install = true;
                        }
                    }
                });
            });
        if let Some(root) = picked_root {
            self.set_root(&root);
            self.ini.set("SetupDone", &crate::engine::sys::local_now().to_string());
            let _ = self.ini.save();
            self.pages.wizard = None;
            self.notify(
                format!("Папка zapret выбрана: {}", self.install.root),
                LogLevel::Ok,
            );
            return;
        }
        if install {
            wizard.running = true;
            wizard.error.clear();
            wizard.stage = "Получение информации о последней версии zapret…".to_string();
            let install_zapret = wizard.install_zapret && !self.install.is_valid();
            let install_tg = wizard.install_tg && !tgproxy::get_state(&self.ini).installed;
            let zapret_dir = wizard.zapret_dir.clone();
            let tg_dir = wizard.tg_dir.clone();
            if install_zapret {
                // Проверяем путь до скачивания: иначе ошибку («нет прав»,
                // «папки нет») пользователь увидит только после загрузки архива.
                let path = std::path::PathBuf::from(zapret_dir.trim());
                if zapret_dir.trim().is_empty() || !path.is_absolute() {
                    wizard.error =
                        "Укажите полный путь к папке, например C:\\zapret.".to_string();
                    return;
                }
                if !path.is_dir() {
                    wizard.error = format!("Папка не найдена: {}", zapret_dir);
                    return;
                }
                let probe = path.join(".zapret-write-test");
                match std::fs::write(&probe, b"ok") {
                    Ok(()) => {
                        let _ = std::fs::remove_file(&probe);
                    }
                    Err(e) => {
                        wizard.error = format!("Нет прав на запись в {}: {}", zapret_dir, e);
                        return;
                    }
                }
            }
            self.spawn_bg(move |tx| {
                let mut errors = Vec::new();
                if install_zapret {
                    let parent = std::path::PathBuf::from(&zapret_dir);
                    // Только поддерживаемое ядро 1.x: 2.x и тестовые сборки
                    // мастер не ставит (см. update::latest_supported_zapret).
                    match update::latest_supported_zapret().and_then(|(release, _skipped)| {
                        update::install_fresh(&release, &parent, |percent, stage| {
                            let _ = tx.send(Msg::Progress("wizard", percent, stage.to_string()));
                        })
                    }) {
                        Ok(install) => {
                            let _ = tx.send(Msg::Installed(Box::new(install)));
                        }
                        Err(e) => errors.push(format!("zapret: {}", e)),
                    }
                }
                if install_tg {
                    let _ = tx.send(Msg::Progress(
                        "wizard",
                        60,
                        "Установка TG WS Proxy…".to_string(),
                    ));
                    match tgproxy::check_latest() {
                        Ok((tag, _)) => {
                            let mut ini = crate::engine::config::Ini::load(&paths::config_path());
                            let tx2 = tx.clone();
                            let result = tgproxy::install(
                                &tag,
                                std::path::Path::new(&tg_dir),
                                &mut ini,
                                |percent, stage| {
                                    let _ = tx2.send(Msg::Progress("wizard", percent, stage.to_string()));
                                },
                            );
                            if let Err(e) = result {
                                errors.push(format!("TG WS Proxy: {}", e));
                            }
                        }
                        Err(e) => errors.push(format!("TG WS Proxy: {}", e)),
                    }
                }
                if errors.is_empty() {
                    let _ = tx.send(Msg::WizardDone(Ok(
                        "Установка завершена.".to_string(),
                    )));
                } else {
                    let _ = tx.send(Msg::WizardDone(Err(errors.join("; "))));
                }
            });
            self.pages.wizard = Some(wizard);
        } else if close {
            self.ini.set("SetupDone", &crate::engine::sys::local_now().to_string());
            let _ = self.ini.save();
            self.pages.wizard = None;
        } else {
            wizard.running = self.pages.wizard.as_ref().map(|x| x.running).unwrap_or(false);
            self.pages.wizard = Some(wizard);
        }
    }

    pub(crate) fn handle_wizard_msg(&mut self, result: Result<String, String>) {
        let Some(wizard) = self.pages.wizard.as_mut() else {
            return;
        };
        wizard.running = false;
        match result {
            Ok(message) => {
                wizard.done = true;
                wizard.progress = 100;
                wizard.stage = message;
                self.ini.set("SetupDone", &crate::engine::sys::local_now().to_string());
                let _ = self.ini.save();
                self.notify("Мастер установки: готово.", LogLevel::Ok);
            }
            Err(error) => {
                wizard.stage = String::new();
                wizard.error = format!("Ошибка: {}", error);
                self.notify(format!("Установка не завершена: {}", error), LogLevel::Error);
            }
        }
        let _ = ZapretInstall::detect("");
    }
}
