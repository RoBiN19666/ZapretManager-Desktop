//! Виджеты интерфейса: карточки, кнопки, переключатели, статусы.

use eframe::egui::{
    self, Color32, FontId, Response, RichText, Rounding, Sense, Stroke, Ui, Vec2,
};

use super::theme::Palette;

pub fn card(ui: &mut Ui, p: &Palette, title: &str, add_contents: impl FnOnce(&mut Ui)) {
    egui::Frame::none()
        .fill(p.surface)
        .stroke(Stroke::new(1.0, p.border))
        .rounding(Rounding::same(10))
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
    let resp = ui.add(
        egui::Button::new(RichText::new(text).color(p.text_dim))
            .fill(Color32::TRANSPARENT)
            .stroke(Stroke::NONE)
            .min_size(Vec2::new(0.0, 30.0)),
    );
    if resp.hovered() {
        ui.painter().rect_filled(resp.rect, Rounding::same(6), p.hover);
    }
    resp
}

pub fn danger(ui: &mut Ui, p: &Palette, text: &str) -> Response {
    button(ui, p, text, p.danger, Color32::WHITE, 1.0)
}

fn button(
    ui: &mut Ui,
    p: &Palette,
    text: &str,
    fill: Color32,
    fg: Color32,
    stroke_width: f32,
) -> Response {
    let resp = ui.add(
        egui::Button::new(RichText::new(text).color(fg).strong())
            .fill(fill)
            .stroke(if stroke_width > 0.0 {
                Stroke::new(1.0, p.border_strong)
            } else {
                Stroke::NONE
            })
            .rounding(Rounding::same(7))
            .min_size(Vec2::new(0.0, 32.0)),
    );
    if resp.hovered() && fill == p.accent {
        ui.painter().rect_filled(resp.rect, Rounding::same(7), p.accent_hover);
    }
    resp
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
    ui.painter().rect_filled(rect, Rounding::same(12), bg);
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
            .circle_stroke(rect.center(), 4.0, Stroke::new(1.2, color));
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
        .rect_filled(rect, Rounding::same(9), p.surface_alt);
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
        .line_segment([rect.left_center(), rect.right_center()], Stroke::new(1.0, p.border));
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
                .rounding(Rounding::same(7))
                .min_size(Vec2::new(0.0, 30.0)),
        )
    } else {
        secondary(ui, p, text)
    }
}

pub fn progress_bar(ui: &mut Ui, p: &Palette, ratio: f32) {
    let desired = Vec2::new(ui.available_width(), 8.0);
    let (rect, _) = ui.allocate_exact_size(desired, Sense::hover());
    ui.painter().rect_filled(rect, Rounding::same(4), p.surface_alt);
    let filled = egui::Rect::from_min_size(
        rect.min,
        Vec2::new(rect.width() * ratio.clamp(0.0, 1.0), rect.height()),
    );
    ui.painter().rect_filled(filled, Rounding::same(4), p.accent);
}

pub fn code_block(ui: &mut Ui, p: &Palette, text: &str) {
    egui::Frame::none()
        .fill(p.editor)
        .stroke(Stroke::new(1.0, p.border))
        .rounding(Rounding::same(6))
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
