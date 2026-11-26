//! Интерфейс приложения.

pub mod app;
pub mod assets;
pub mod pages;
pub mod theme;
pub mod tray;
pub mod widgets;

pub use app::{App, Page};

/// Запустить графический интерфейс.
#[allow(clippy::too_many_arguments)]
pub fn run(
    install: crate::engine::install::ZapretInstall,
    ini: crate::engine::config::Ini,
    shot: Option<String>,
    shot_page: Option<String>,
    start_hidden: bool,
    autostart_mode: bool,
    limited: bool,
) -> eframe::Result<()> {
    let icon = assets::window_icon();
    let window_w = (ini.get_int("WindowW", 1280).clamp(800, 3840)) as f32;
    let window_h = (ini.get_int("WindowH", 880).clamp(600, 2160)) as f32;
    let maximized = ini.get_bool("Maximized", true);
    let mut viewport = eframe::egui::ViewportBuilder::default()
        .with_inner_size([window_w, window_h])
        .with_min_inner_size([1000.0, 700.0])
        .with_title("Zapret Manager")
        .with_app_id("ZapretManager");
    if maximized {
        viewport = viewport.with_maximized(true);
    }
    if let Some(icon) = icon {
        viewport = viewport.with_icon(icon);
    }
    let options = eframe::NativeOptions {
        viewport,
        centered: true,
        ..Default::default()
    };
    eframe::run_native(
        "Zapret Manager",
        options,
        Box::new(move |cc| {
            theme::install_fonts(&cc.egui_ctx);
            theme::apply(&cc.egui_ctx, true);
            let app = App::new(
                install,
                ini,
                shot,
                shot_page,
                start_hidden,
                autostart_mode,
                limited,
            );
            Ok(Box::new(app))
        }),
    )
}
