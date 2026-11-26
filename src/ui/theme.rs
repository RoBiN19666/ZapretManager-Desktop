//! Оформление: палитра, шрифты, стили egui.

use std::sync::atomic::{AtomicBool, Ordering};

use eframe::egui::{self, Color32, FontData, FontDefinitions, FontFamily, TextStyle};

#[derive(Debug, Clone, Copy)]
pub struct Palette {
    pub bg: Color32,
    pub sidebar: Color32,
    pub surface: Color32,
    pub surface_alt: Color32,
    pub editor: Color32,
    pub hover: Color32,
    pub border: Color32,
    pub border_strong: Color32,
    pub text: Color32,
    pub text_dim: Color32,
    pub text_muted: Color32,
    pub accent: Color32,
    pub accent_hover: Color32,
    pub on_accent: Color32,
    pub selection: Color32,
    pub success: Color32,
    pub warn: Color32,
    pub danger: Color32,
}

pub const DARK: Palette = Palette {
    bg: Color32::from_rgb(11, 10, 10),
    sidebar: Color32::from_rgb(16, 14, 13),
    surface: Color32::from_rgb(26, 22, 20),
    surface_alt: Color32::from_rgb(38, 32, 27),
    editor: Color32::from_rgb(22, 19, 17),
    hover: Color32::from_rgb(50, 43, 36),
    border: Color32::from_rgb(52, 44, 37),
    border_strong: Color32::from_rgb(78, 66, 54),
    text: Color32::from_rgb(244, 238, 231),
    text_dim: Color32::from_rgb(178, 167, 155),
    text_muted: Color32::from_rgb(126, 116, 105),
    accent: Color32::from_rgb(249, 146, 56),
    accent_hover: Color32::from_rgb(255, 172, 96),
    on_accent: Color32::from_rgb(46, 25, 8),
    selection: Color32::from_rgb(46, 37, 30),
    success: Color32::from_rgb(79, 208, 138),
    warn: Color32::from_rgb(255, 194, 74),
    danger: Color32::from_rgb(255, 97, 82),
};

pub const LIGHT: Palette = Palette {
    bg: Color32::from_rgb(248, 248, 247),
    sidebar: Color32::from_rgb(243, 243, 242),
    surface: Color32::from_rgb(255, 255, 255),
    surface_alt: Color32::from_rgb(246, 245, 244),
    editor: Color32::from_rgb(252, 252, 251),
    hover: Color32::from_rgb(236, 235, 233),
    border: Color32::from_rgb(220, 219, 216),
    border_strong: Color32::from_rgb(182, 180, 176),
    text: Color32::from_rgb(31, 27, 24),
    text_dim: Color32::from_rgb(90, 82, 75),
    text_muted: Color32::from_rgb(126, 116, 105),
    accent: Color32::from_rgb(249, 146, 56),
    accent_hover: Color32::from_rgb(226, 116, 26),
    on_accent: Color32::from_rgb(255, 255, 255),
    selection: Color32::from_rgb(238, 226, 212),
    success: Color32::from_rgb(26, 150, 90),
    warn: Color32::from_rgb(176, 118, 0),
    danger: Color32::from_rgb(199, 49, 38),
};

static ICONS_LOADED: AtomicBool = AtomicBool::new(false);

/// Доступен ли шрифт Segoe MDL2 Assets для иконок.
pub fn has_icon_font() -> bool {
    ICONS_LOADED.load(Ordering::Relaxed)
}

pub fn palette(dark: bool) -> Palette {
    if dark {
        DARK
    } else {
        LIGHT
    }
}

/// Загрузить системные шрифты Windows (Segoe UI, Consolas).
pub fn install_fonts(ctx: &egui::Context) {
    let mut fonts = FontDefinitions::default();
    let mut loaded = false;
    for (name, file) in [
        ("ui", "segoeui.ttf"),
        ("ui_bold", "segoeuib.ttf"),
        ("mono", "consola.ttf"),
        ("icons", "segmdl2.ttf"),
    ] {
        let path = std::path::Path::new("C:\\Windows\\Fonts").join(file);
        if let Ok(bytes) = std::fs::read(&path) {
            fonts
                .font_data
                .insert(name.to_owned(), std::sync::Arc::new(FontData::from_owned(bytes)));
            loaded = true;
        }
    }
    if loaded {
        if let Some(list) = fonts.families.get_mut(&FontFamily::Proportional) {
            list.insert(0, "ui".to_owned());
        }
        if let Some(list) = fonts.families.get_mut(&FontFamily::Monospace) {
            list.insert(0, "mono".to_owned());
        }
        let bold: FontFamily = FontFamily::Name("bold".into());
        fonts
            .families
            .insert(bold.clone(), vec!["ui_bold".to_owned(), "ui".to_owned()]);
        if fonts.font_data.contains_key("icons") {
            let icons: FontFamily = FontFamily::Name("icons".into());
            fonts
                .families
                .insert(icons, vec!["icons".to_owned(), "ui".to_owned()]);
            ICONS_LOADED.store(true, std::sync::atomic::Ordering::Relaxed);
        }
        ctx.set_fonts(fonts);
        let mut style = (*ctx.style()).clone();
        style.text_styles.insert(
            TextStyle::Heading,
            egui::FontId::new(21.0, bold.clone()),
        );
        style.text_styles.insert(
            TextStyle::Name("Big".into()),
            egui::FontId::new(19.0, bold.clone()),
        );
        style.text_styles.insert(
            TextStyle::Name("Bold".into()),
            egui::FontId::new(14.0, bold.clone()),
        );
        style.text_styles.insert(
            TextStyle::Body,
            egui::FontId::new(14.0, FontFamily::Proportional),
        );
        style.text_styles.insert(
            TextStyle::Button,
            egui::FontId::new(13.5, FontFamily::Proportional),
        );
        style.text_styles.insert(
            TextStyle::Small,
            egui::FontId::new(12.5, FontFamily::Proportional),
        );
        style.text_styles.insert(
            TextStyle::Monospace,
            egui::FontId::new(12.5, FontFamily::Monospace),
        );
        ctx.set_style(style);
    }
}

/// Применить палитру к стилю egui.
pub fn apply(ctx: &egui::Context, dark: bool) -> Palette {
    let p = palette(dark);
    let mut style = (*ctx.style()).clone();
    let mut visuals = if dark {
        egui::Visuals::dark()
    } else {
        egui::Visuals::light()
    };
    visuals.panel_fill = p.bg;
    visuals.window_fill = p.surface;
    visuals.extreme_bg_color = p.editor;
    visuals.faint_bg_color = p.surface_alt;
    visuals.code_bg_color = p.editor;
    visuals.override_text_color = Some(p.text);
    visuals.hyperlink_color = p.accent;
    visuals.selection.bg_fill = p.selection;
    visuals.selection.stroke = egui::Stroke::new(1.0, p.accent);
    visuals.window_stroke = egui::Stroke::new(1.0, p.border);
    visuals.widgets.noninteractive.bg_fill = p.surface;
    visuals.widgets.noninteractive.fg_stroke = egui::Stroke::new(1.0, p.text_dim);
    visuals.widgets.inactive.bg_fill = p.surface_alt;
    visuals.widgets.inactive.weak_bg_fill = p.surface_alt;
    visuals.widgets.inactive.fg_stroke = egui::Stroke::new(1.0, p.text);
    visuals.widgets.hovered.bg_fill = p.hover;
    visuals.widgets.hovered.weak_bg_fill = p.hover;
    visuals.widgets.hovered.fg_stroke = egui::Stroke::new(1.0, p.text);
    visuals.widgets.active.bg_fill = p.selection;
    visuals.widgets.active.weak_bg_fill = p.selection;
    visuals.widgets.active.fg_stroke = egui::Stroke::new(1.0, p.accent);
    style.visuals = visuals;
    style.spacing.item_spacing = egui::vec2(8.0, 8.0);
    style.spacing.button_padding = egui::vec2(12.0, 6.0);
    ctx.set_style(style);
    p
}
