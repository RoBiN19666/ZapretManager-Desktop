//! Виджеты интерфейса: карточки, кнопки, переключатели, статусы.

use eframe::egui::{
    self, Color32, FontId, Response, RichText, CornerRadius, Sense, Stroke, Ui, Vec2,
};

use super::theme::Palette;

pub fn card(ui: &mut Ui, p: &Palette, title: &str, add_contents: impl FnOnce(&mut Ui)) {
    egui::Frame::NONE
        .fill(p.surface)
        .stroke(Stroke::new(1.0_f32, p.border))
        .corner_radius(CornerRadius::same(10))
        .inner_margin(egui::Margin::same(16))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            if !title.is_empty() {
                ui.label(
                    RichText::new(title)
                        .font(FontId::new(13.0, egui::FontFamily::Name("bold".into())))
                        .color(p.text_dim),
                );
                ui.add_space(8.0);
            }
            add_contents(ui);
        });
    ui.add_space(10.0);
}

pub fn primary(ui: &mut Ui, p: &Palette, text: &str) -> Response {
    button(ui, p, text, p.accent, p.on_accent, 1.0)
}

pub fn secondary(ui: &mut Ui, p: &Palette, text: &str) -> Response {
    button(ui, p, text, p.surface_alt, p.text, 1.0)
}

pub fn ghost(ui: &mut Ui, p: &Palette, text: &str) -> Response {
    with_widget_fill(ui, Color32::TRANSPARENT, p.hover, |ui| {
        ui.add(
            egui::Button::new(RichText::new(text).color(p.text_dim))
                .stroke(Stroke::NONE)
                .corner_radius(CornerRadius::same(6))
                .min_size(Vec2::new(0.0, 30.0)),
        )
    })
}

pub fn danger(ui: &mut Ui, p: &Palette, text: &str) -> Response {
    button(ui, p, text, p.danger, Color32::WHITE, 1.0)
}

/// Подменить цвета фона виджета на время добавления кнопки.
///
/// Важно: раньше hover-фон рисовался вручную **после** `ui.add(Button)`, то есть
/// поверх текста — при наведении надпись исчезала. Теперь цвет фона задаётся
/// через стиль egui, и кнопка сама рисует фон под текстом.
fn with_widget_fill<R>(
    ui: &mut Ui,
    fill: Color32,
    hover: Color32,
    add: impl FnOnce(&mut Ui) -> R,
) -> R {
    let old_style = ui.style().clone();
    {
        let widgets = &mut ui.style_mut().visuals.widgets;
        widgets.inactive.weak_bg_fill = fill;
        widgets.hovered.weak_bg_fill = hover;
        widgets.active.weak_bg_fill = hover;
    }
    let result = add(ui);
    ui.set_style(old_style);
    result
}

fn button(
    ui: &mut Ui,
    p: &Palette,
    text: &str,
    fill: Color32,
    fg: Color32,
    stroke_width: f32,
) -> Response {
    let hover_fill = if fill == p.accent {
        p.accent_hover
    } else if fill == p.danger {
        p.danger
    } else {
        p.hover
    };
    with_widget_fill(ui, fill, hover_fill, |ui| {
        ui.add(
            egui::Button::new(RichText::new(text).color(fg).strong())
                .stroke(if stroke_width > 0.0 {
                    Stroke::new(1.0_f32, p.border_strong)
                } else {
                    Stroke::NONE
                })
                .corner_radius(CornerRadius::same(7))
                .min_size(Vec2::new(0.0, 32.0)),
        )
    })
}

pub fn link(ui: &mut Ui, p: &Palette, text: &str) -> Response {
    ui.add(egui::Link::new(RichText::new(text).color(p.accent)))
}

/// Переключатель 46×24.
pub fn toggle(ui: &mut Ui, p: &Palette, value: &mut bool) -> Response {
    let desired = Vec2::new(46.0, 24.0);
    let (rect, mut resp) = ui.allocate_exact_size(desired, Sense::click());
    if resp.clicked() {
        *value = !*value;
        resp.mark_changed();
    }
    let bg = if *value { p.accent } else { p.surface_alt };
    ui.painter().rect_filled(rect, CornerRadius::same(12), bg);
    let knob_x = if *value {
        rect.right() - 12.0
    } else {
        rect.left() + 12.0
    };
    ui.painter().circle_filled(
        egui::pos2(knob_x, rect.center().y),
        9.0,
        if *value { p.on_accent } else { p.text_muted },
    );
    resp
}

pub fn status_dot(ui: &mut Ui, color: Color32, filled: bool) {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(10.0, 10.0), Sense::hover());
    if filled {
        ui.painter().circle_filled(rect.center(), 4.5, color);
    } else {
        ui.painter()
            .circle_stroke(rect.center(), 4.0, Stroke::new(1.2_f32, color));
    }
}

pub fn badge(ui: &mut Ui, p: &Palette, text: &str, color: Color32) {
    let text = RichText::new(text).size(11.0).color(color);
    let galley = ui.painter().layout_no_wrap(
        text.text().to_string(),
        FontId::new(11.0, egui::FontFamily::Proportional),
        color,
    );
    let size = Vec2::new(galley.rect.width() + 12.0, 18.0);
    let (rect, _) = ui.allocate_exact_size(size, Sense::hover());
    ui.painter()
        .rect_filled(rect, CornerRadius::same(9), p.surface_alt);
    ui.painter()
        .galley(rect.min + Vec2::new(6.0, 2.0), galley, color);
}

pub fn small(ui: &mut Ui, p: &Palette, text: &str) {
    ui.label(RichText::new(text).size(12.5).color(p.text_dim));
}

pub fn muted(ui: &mut Ui, p: &Palette, text: &str) {
    ui.label(RichText::new(text).size(12.0).color(p.text_muted));
}

pub fn hint(ui: &mut Ui, p: &Palette, text: &str) {
    ui.label(
        RichText::new(text)
            .size(12.0)
            .color(p.text_muted)
            .italics(),
    );
}

pub fn kv_row(ui: &mut Ui, p: &Palette, key: &str, value: &str) {
    ui.horizontal(|ui| {
        ui.label(RichText::new(key).size(12.5).color(p.text_dim));
        ui.label(RichText::new(value).size(12.5).color(p.text));
    });
}

pub fn separator(ui: &mut Ui, p: &Palette) {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 1.0), Sense::hover());
    ui.painter()
        .line_segment([rect.left_center(), rect.right_center()], Stroke::new(1.0_f32, p.border));
    ui.add_space(4.0);
}

/// Строка с переключателем: заголовок + описание + сам переключатель справа.
pub fn toggle_row(
    ui: &mut Ui,
    p: &Palette,
    title: &str,
    description: &str,
    value: &mut bool,
) -> bool {
    let mut changed = false;
    ui.horizontal(|ui| {
        let width = ui.available_width() - 60.0;
        ui.allocate_ui(Vec2::new(width.max(100.0), 40.0), |ui| {
            ui.vertical(|ui| {
                ui.label(RichText::new(title).size(13.0).color(p.text).strong());
                if !description.is_empty() {
                    ui.label(RichText::new(description).size(11.5).color(p.text_muted));
                }
            });
        });
        if toggle(ui, p, value).changed() {
            changed = true;
        }
    });
    changed
}

/// Кнопка-режим (выбранная — акцентная).
pub fn mode_button(ui: &mut Ui, p: &Palette, text: &str, selected: bool) -> Response {
    if selected {
        ui.add(
            egui::Button::new(RichText::new(text).color(p.on_accent).strong())
                .fill(p.accent)
                .corner_radius(CornerRadius::same(7))
                .min_size(Vec2::new(0.0, 30.0)),
        )
    } else {
        secondary(ui, p, text)
    }
}

pub fn progress_bar(ui: &mut Ui, p: &Palette, ratio: f32) {
    let desired = Vec2::new(ui.available_width(), 8.0);
    let (rect, _) = ui.allocate_exact_size(desired, Sense::hover());
    ui.painter().rect_filled(rect, CornerRadius::same(4), p.surface_alt);
    let filled = egui::Rect::from_min_size(
        rect.min,
        Vec2::new(rect.width() * ratio.clamp(0.0, 1.0), rect.height()),
    );
    ui.painter().rect_filled(filled, CornerRadius::same(4), p.accent);
}

pub fn code_block(ui: &mut Ui, p: &Palette, text: &str) {
    egui::Frame::NONE
        .fill(p.editor)
        .stroke(Stroke::new(1.0_f32, p.border))
        .corner_radius(CornerRadius::same(6))
        .inner_margin(egui::Margin::same(8))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.label(
                RichText::new(text)
                    .font(FontId::new(12.0, egui::FontFamily::Monospace))
                    .color(p.text_dim),
            );
        });
}
