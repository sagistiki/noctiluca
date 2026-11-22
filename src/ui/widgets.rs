//! The controls every page shares, so the window speaks one language: pads
//! (an icon, a short name, a hint under it), icon segmented controls,
//! sliders led by an icon, round colour swatches, and card headers with
//! icon actions. Built on the Pro Suite tokens in style.rs.

use egui::{Color32, CornerRadius, Rect, Response, Sense, Stroke, Ui, Vec2, vec2};

use super::icons::{self, Icon};
use super::style;

/// Text centred at `at`, at `size` or smaller so it fits in `max_w`.
pub fn fit_text(p: &egui::Painter, at: egui::Pos2, text: &str, size: f32, family: egui::FontFamily, colour: Color32, max_w: f32) {
    let galley = p.layout_no_wrap(text.to_string(), egui::FontId::new(size, family.clone()), colour);
    let size = if galley.size().x > max_w { (size * max_w / galley.size().x).max(7.5) } else { size };
    p.text(at, egui::Align2::CENTER_CENTER, text, egui::FontId::new(size, family), colour);
}

fn semibold() -> egui::FontFamily {
    egui::FontFamily::Name("semibold".into())
}

/// A pad: an icon over a short name and a hint (a key, a state), lit in
/// the accent colour when `on`. `size` is usually width × 74.
pub fn pad(ui: &mut Ui, size: Vec2, icon: Icon, name: &str, hint: &str, on: bool, tip: &str) -> Response {
    pad_coloured(ui, size, icon, name, hint, on, style::ACCENT, tip)
}

/// A pad lit in `colour` instead of the accent (red for lights out).
#[allow(clippy::too_many_arguments)]
pub fn pad_coloured(ui: &mut Ui, size: Vec2, icon: Icon, name: &str, hint: &str, on: bool, colour: Color32, tip: &str) -> Response {
    let (rect, r) = ui.allocate_exact_size(size, Sense::click());
    let hovered = r.hovered();
    let fill = if on { colour } else if hovered { style::LINE } else { style::CARD_LIGHT };
    let p = ui.painter();
    p.rect(rect, CornerRadius::same(9), fill, Stroke::new(1.0, if on { colour } else { style::LINE }), egui::StrokeKind::Inside);
    let fg = if on { Color32::WHITE } else { style::TEXT };
    let has_hint = !hint.is_empty();
    let icon_y = if has_hint { -14.0 } else { -9.0 };
    icons::paint(ui, Rect::from_center_size(rect.center() + vec2(0.0, icon_y), vec2(18.0, 18.0)), icon, fg);
    let p = ui.painter();
    let room = rect.width() - 10.0;
    fit_text(p, rect.center() + vec2(0.0, if has_hint { 7.0 } else { 13.0 }), name, 12.0, semibold(), fg, room);
    if has_hint {
        fit_text(p, rect.center() + vec2(0.0, 23.0), hint, 10.5, egui::FontFamily::Proportional, if on { Color32::from_white_alpha(200) } else { style::TERTIARY }, room);
    }
    r.on_hover_text(tip)
}

/// The width of `n` pads in a row filling `width` with `gap` between them,
/// no narrower than `min`.
pub fn pad_width(width: f32, n: usize, gap: f32, min: f32) -> f32 {
    ((width - gap * (n as f32 - 1.0)) / n as f32).max(min)
}

/// A segmented control whose segments are an icon and a word, all the same
/// width, filling `width`.
pub fn icon_segmented<T: PartialEq + Copy>(ui: &mut Ui, value: &mut T, options: &[(T, Icon, &str)], width: f32, tip: impl Fn(T) -> String) -> Response {
    let h = 32.0;
    let (rect, mut r) = ui.allocate_exact_size(vec2(width, h), Sense::hover());
    ui.painter().rect_filled(rect, CornerRadius::same(style::WIDGET_RADIUS + 1), style::WELL);
    let n = options.len().max(1) as f32;
    let seg = (rect.width() - 4.0) / n;
    for (k, (option, icon, name)) in options.iter().enumerate() {
        let cell = Rect::from_min_size(rect.min + vec2(2.0 + seg * k as f32, 2.0), vec2(seg, h - 4.0));
        let id = ui.scope_id().with(("seg", k, rect.min.x as i32, rect.min.y as i32));
        let cr = ui.interact(cell, id, Sense::click());
        let on = *value == *option;
        if on {
            ui.painter().rect_filled(cell, CornerRadius::same(style::WIDGET_RADIUS - 1), style::LINE);
        } else if cr.hovered() {
            ui.painter().rect_filled(cell, CornerRadius::same(style::WIDGET_RADIUS - 1), style::CARD_LIGHT);
        }
        let fg = if on { style::TEXT } else { style::MUTED };
        let galley = ui.painter().layout_no_wrap(name.to_string(), egui::FontId::proportional(12.0), fg);
        let total = 14.0 + 6.0 + galley.size().x;
        let compact = total > cell.width() - 8.0;
        if compact {
            icons::paint(ui, Rect::from_center_size(cell.center(), vec2(14.0, 14.0)), *icon, fg);
        } else {
            let x0 = cell.center().x - total / 2.0;
            icons::paint(ui, Rect::from_center_size(egui::pos2(x0 + 7.0, cell.center().y), vec2(14.0, 14.0)), *icon, fg);
            ui.painter().galley(egui::pos2(x0 + 20.0, cell.center().y - galley.size().y / 2.0), galley, fg);
        }
        let cr = cr.on_hover_text(if compact { format!("{name}: {}", tip(*option)) } else { tip(*option) });
        if cr.clicked() && !on {
            *value = *option;
            r.mark_changed();
        }
    }
    r
}

/// A slider led by an icon and a name, with its value (or a word) after it.
pub fn slider_row(ui: &mut Ui, icon: Icon, name: &str, value: &mut f32, range: std::ops::RangeInclusive<f32>, shown: &str, tip: &str) -> Response {
    ui.horizontal(|ui| {
        let (rect, _) = ui.allocate_exact_size(vec2(18.0, 22.0), Sense::hover());
        icons::paint(ui, rect.shrink2(vec2(1.0, 3.0)), icon, style::MUTED);
        ui.label(egui::RichText::new(name).size(13.0)).on_hover_text(tip);
        let value_w = 64.0;
        ui.spacing_mut().slider_width = (ui.available_width() - value_w - 12.0).max(60.0);
        let r = ui.add(egui::Slider::new(value, range).show_value(false)).on_hover_text(tip);
        ui.label(style::semibold(shown, 13.0).color(style::ACCENT));
        r
    })
    .inner
}

/// A round colour button, ringed when `on`.
pub fn swatch(ui: &mut Ui, colour: Color32, on: bool, tip: &str) -> Response {
    let (rect, r) = ui.allocate_exact_size(Vec2::splat(30.0), Sense::click());
    let p = ui.painter();
    p.circle_filled(rect.center(), 11.0, colour);
    let ring = if on { style::TEXT } else if r.hovered() { style::MUTED } else { style::LINE };
    p.circle_stroke(rect.center(), 14.0, Stroke::new(if on { 2.0 } else { 1.0 }, ring));
    r.on_hover_text(tip)
}

/// A card's header: a small label on the left, then whatever follows
/// (a state, icon actions) laid out by `rest`.
pub fn card_header(ui: &mut Ui, label: &str, rest: impl FnOnce(&mut Ui)) {
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new(label).size(11.5).strong().color(style::TERTIARY));
        rest(ui);
    });
    ui.add_space(8.0);
}

/// A button with an icon and a word, for card headers ("+ Light").
pub fn icon_text_button(ui: &mut Ui, icon: Icon, text: &str, enabled: bool, tip: &str) -> Response {
    let galley = ui.painter().layout_no_wrap(text.to_string(), egui::FontId::new(12.5, semibold()), style::TEXT);
    let size = vec2(galley.size().x + 36.0, 28.0);
    let (rect, r) = ui.allocate_exact_size(size, if enabled { Sense::click() } else { Sense::hover() });
    let fill = if !enabled { style::CARD } else if r.hovered() { style::LINE } else { style::CARD_LIGHT };
    let fg = if enabled { style::TEXT } else { style::TERTIARY };
    ui.painter().rect(rect, CornerRadius::same(style::WIDGET_RADIUS), fill, Stroke::new(1.0, style::LINE), egui::StrokeKind::Inside);
    icons::paint(ui, Rect::from_center_size(rect.left_center() + vec2(15.0, 0.0), vec2(14.0, 14.0)), icon, fg);
    ui.painter().text(rect.left_center() + vec2(28.0, 0.0), egui::Align2::LEFT_CENTER, text, egui::FontId::new(12.5, semibold()), fg);
    r.on_hover_text(tip)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pads_share_a_row_and_never_get_too_narrow() {
        assert_eq!(pad_width(8.0 * 100.0 + 7.0 * 8.0, 8, 8.0, 64.0), 100.0);
        assert_eq!(pad_width(300.0, 8, 8.0, 64.0), 64.0);
    }
}
