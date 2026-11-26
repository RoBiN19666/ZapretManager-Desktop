//! Страница «О программе».

use eframe::egui::{self, RichText, Ui};

use crate::engine::{paths, sys};

use crate::ui::app::App;
use crate::ui::widgets as w;

impl App {
    pub(crate) fn page_about(&mut self, ui: &mut Ui) {
        let p = self.p;
        if self.about_logo.is_none() {
            if let Some(image) = crate::ui::assets::logo_wide() {
                self.about_logo = Some(ui.ctx().load_texture(
                    "about-logo-wide",
                    image,
                    egui::TextureOptions::LINEAR,
                ));
            }
        }

        w::card(ui, &p, "", |ui| {
            ui.vertical_centered(|ui| {
                if let Some(texture) = &self.about_logo {
                    let native = texture.size_vec2();
                    let width = 500.0_f32.min(ui.available_width());
                    let height = width * native.y / native.x.max(1.0);
                    ui.add(egui::Image::from_texture(egui::load::SizedTexture::new(
                        texture.id(),
                        egui::vec2(width, height),
                    )));
                }
                ui.add_space(10.0);
                ui.label(
                    RichText::new("Zapret Manager")
                        .size(28.0)
                        .strong()
                        .color(p.text),
                );
                ui.label(
                    RichText::new(format!("Версия {}", crate::APP_VERSION))
                        .size(14.0)
                        .color(p.accent),
                );
                ui.add_space(4.0);
                ui.label(
                    RichText::new("Графическая оболочка для ускорения соединения в Discord и YouTube")
                        .size(13.0)
                        .color(p.text_dim),
                );
            });
        });

        w::card(ui, &p, "О программе", |ui| {
            ui.label(
                RichText::new(
                    "Zapret Manager — неофициальная графическая оболочка. Ускорение соединения выполняет проект zapret (bol-van), сборка и стратегии — zapret-discord-youtube (Flowseal), прокси Telegram — TG WS Proxy (Flowseal). Само ускорение выполняют winws.exe и драйвер WinDivert из состава zapret.",
                )
                .size(12.5)
                .color(p.text_dim),
            );
            ui.add_space(8.0);
            for line in [
                "Стратегии: general*.bat из zapret-discord-youtube",
                "Тихий TG WS Proxy: исходники Flowseal + портативный Python",
                "Лицензия: MIT",
                "Автор: RoBiN19666",
            ] {
                ui.label(RichText::new(format!("•  {}", line)).size(12.0).color(p.text));
            }
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new(format!(
                        "Настройки и журнал: {}",
                        paths::data_dir().to_string_lossy()
                    ))
                    .size(11.5)
                    .color(p.text_muted),
                );
                if w::ghost(ui, &p, "Открыть папку").clicked() {
                    sys::open_folder(&paths::data_dir());
                }
            });
        });

        w::card(ui, &p, "Ссылки", |ui| {
            let links = [
                (
                    "Репозиторий Zapret Manager",
                    "https://github.com/RoBiN19666/ZapretManager-Desktop",
                ),
                (
                    "Руководство",
                    "https://github.com/RoBiN19666/ZapretManager-Desktop/blob/main/docs/guide.md",
                ),
                (
                    "zapret-discord-youtube (Flowseal)",
                    "https://github.com/Flowseal/zapret-discord-youtube",
                ),
                ("zapret (bol-van)", "https://github.com/bol-van/zapret"),
                ("TG WS Proxy (Flowseal)", "https://github.com/Flowseal/tg-ws-proxy"),
            ];
            ui.horizontal_wrapped(|ui| {
                for (label, url) in links {
                    if w::link(ui, &p, label).clicked() {
                        sys::open_url(url);
                    }
                }
            });
        });

        w::card(ui, &p, "Поддержать проект", |ui| {
            ui.label(
                RichText::new("Если программа оказалась полезной — можно поддержать автора.")
                    .size(12.0)
                    .color(p.text_dim),
            );
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                if w::primary(ui, &p, "Boosty").clicked() {
                    sys::open_url("https://boosty.to/robin19666");
                }
                if w::secondary(ui, &p, "DonationAlerts").clicked() {
                    sys::open_url("https://www.donationalerts.com/r/robin19666");
                }
            });
        });
    }
}
