//! The look of the control window: the "Pro Suite" language (Claude Design
//! canvas, direction C), after Apple's pro apps: graphite, a sidebar and an
//! inspector, small exact type, one blue accent, colour only where it means
//! something (status, surfaces, lights).

use egui::{Color32, CornerRadius, Frame, Margin, Response, RichText, Sense, Stroke, Ui, Vec2};

/// The window.
pub const BG: Color32 = Color32::from_rgb(0x1c, 0x1c, 0x1e);
/// Sidebar, inspector, grouped panels.
pub const CARD: Color32 = Color32::from_rgb(0x25, 0x25, 0x27);
/// Controls: buttons, fields, rows.
pub const CARD_LIGHT: Color32 = Color32::from_rgb(0x2c, 0x2c, 0x2f);
/// The toolbar.
pub const TOOLBAR: Color32 = Color32::from_rgb(0x2a, 0x2a, 0x2d);
/// Hairlines, and a control's hover.
pub const LINE: Color32 = Color32::from_rgb(0x3a, 0x3a, 0x3d);
/// Wells: a field's inside, a segmented control's track, a picture's frame.
pub const WELL: Color32 = Color32::from_rgb(0x1c, 0x1c, 0x1e);
pub const TEXT: Color32 = Color32::from_rgb(0xf2, 0xf2, 0xf7);
/// Secondary text (4.9:1 on the panels).
pub const MUTED: Color32 = Color32::from_rgb(0xa1, 0xa1, 0xa8);
/// Section headers and hints.
pub const TERTIARY: Color32 = Color32::from_rgb(0x8e, 0x8e, 0x96);
/// The one accent: selection, the primary button, the playing pad.
pub const ACCENT: Color32 = Color32::from_rgb(0x0a, 0x84, 0xff);
/// Older names, kept so every page speaks the same language: the accent is
/// blue now; the system colours below carry meaning.
pub const PINK: Color32 = ACCENT;
pub const CYAN: Color32 = Color32::from_rgb(0x64, 0xd2, 0xff);
pub const AMBER: Color32 = Color32::from_rgb(0xff, 0x9f, 0x0a);
pub const GREEN: Color32 = Color32::from_rgb(0x30, 0xd1, 0x58);
pub const RED: Color32 = Color32::from_rgb(0xff, 0x45, 0x3a);
pub const PURPLE: Color32 = Color32::from_rgb(0xbf, 0x5a, 0xf2);
pub const ROSE: Color32 = Color32::from_rgb(0xff, 0x37, 0x5f);
/// Surfaces 1 to 8 (the projector draws its calibration in the same ones).
pub const SURFACE_COLOURS: [Color32; 8] = [
    CYAN,
    ROSE,
    AMBER,
    GREEN,
    PURPLE,
    Color32::from_rgb(0xff, 0x7a, 0x2f),
    ACCENT,
    Color32::from_rgb(0xff, 0xd6, 0x0a),
];

pub const CARD_RADIUS: u8 = 10;
pub const WIDGET_RADIUS: u8 = 7;
pub const GAP: f32 = 12.0;

/// Inter (SIL Open Font License, assets/fonts/Inter-LICENSE.txt), built into
/// the program so the menu looks the same wherever it runs.
const INTER: &[u8] = include_bytes!("../../assets/fonts/Inter-Regular.ttf");
const INTER_SEMIBOLD: &[u8] = include_bytes!("../../assets/fonts/Inter-SemiBold.ttf");
const INTER_EXTRABOLD: &[u8] = include_bytes!("../../assets/fonts/Inter-ExtraBold.ttf");

fn family(name: &str) -> egui::FontFamily {
    egui::FontFamily::Name(name.into())
}

/// Inter for all text, with its semibold and extrabold weights under the
/// names "semibold" and "heavy"; egui's own fonts stay behind them for any
/// character Inter lacks.
pub fn install_fonts(ctx: &egui::Context) {
    use std::sync::Arc;
    let mut fonts = egui::FontDefinitions::default();
    for (name, data) in [
        ("Inter", INTER),
        ("Inter SemiBold", INTER_SEMIBOLD),
        ("Inter ExtraBold", INTER_EXTRABOLD),
    ] {
        fonts
            .font_data
            .insert(name.into(), Arc::new(egui::FontData::from_static(data)));
    }
    let mut fallback = fonts
        .families
        .get(&egui::FontFamily::Proportional)
        .cloned()
        .unwrap_or_default();
    // Hebrew (box and venue names): the Mac's own Hebrew font, read where
    // macOS keeps it rather than shipped.
    if let Ok(data) = std::fs::read("/System/Library/Fonts/SFHebrew.ttf") {
        fonts.font_data.insert("SF Hebrew".into(), Arc::new(egui::FontData::from_owned(data)));
        fallback.insert(0, "SF Hebrew".into());
    }
    let with = |first: &str| {
        std::iter::once(first.to_string())
            .chain(fallback.iter().cloned())
            .collect::<Vec<_>>()
    };
    fonts
        .families
        .insert(egui::FontFamily::Proportional, with("Inter"));
    fonts
        .families
        .insert(family("semibold"), with("Inter SemiBold"));
    fonts
        .families
        .insert(family("heavy"), with("Inter ExtraBold"));
    ctx.set_fonts(fonts);
}

fn hebrew(c: char) -> bool {
    ('\u{0590}'..='\u{05FF}').contains(&c) || ('\u{FB1D}'..='\u{FB4F}').contains(&c)
}

/// Text that may hold Hebrew, in the order to draw it left to right: egui
/// lays every line out left to right, so a Hebrew word would come out
/// backwards. A line with Hebrew reads right to left: its runs in reverse
/// order, the Hebrew ones each reversed, Latin words and numbers kept.
pub fn visual(text: &str) -> String {
    if !text.chars().any(hebrew) {
        return text.to_string();
    }
    // Runs: Hebrew (with the spaces inside it) or not.
    let mut runs: Vec<(bool, String)> = Vec::new();
    for c in text.chars() {
        let h = hebrew(c);
        let strong = h || c.is_alphanumeric();
        match runs.last_mut() {
            Some((kind, run)) if !strong || *kind == h => run.push(c),
            _ => runs.push((h, c.to_string())),
        }
    }
    runs.iter()
        .rev()
        .map(|(h, run)| {
            if *h {
                run.chars().rev().collect::<String>()
            } else {
                // A Latin run keeps its order; spaces at its ends go to the other side.
                let trimmed = run.trim();
                let (lead, tail) = (run.len() - run.trim_start().len(), run.len() - run.trim_end().len());
                format!("{}{}{}", " ".repeat(tail), trimmed, " ".repeat(lead))
            }
        })
        .collect::<String>()
        .trim()
        .to_string()
}

/// Text in Inter SemiBold.
pub fn semibold(text: impl Into<String>, size: f32) -> RichText {
    RichText::new(text).family(family("semibold")).size(size)
}

/// Text in Inter ExtraBold, for big numbers.
pub fn heavy(text: impl Into<String>, size: f32) -> RichText {
    RichText::new(text).family(family("heavy")).size(size)
}

/// A page's title, at the top of its content.
pub fn title(text: impl Into<String>) -> RichText {
    semibold(text, 20.0)
}

/// Sets egui's colours, spacing, rounding and fonts to the Pro Suite look.
pub fn apply(ctx: &egui::Context) {
    install_fonts(ctx);
    ctx.all_styles_mut(|style| {
        let v = &mut style.visuals;
        *v = egui::Visuals::dark();
        v.panel_fill = BG;
        v.window_fill = TOOLBAR;
        v.window_stroke = Stroke::new(1.0, LINE);
        v.window_corner_radius = CornerRadius::same(CARD_RADIUS);
        v.menu_corner_radius = CornerRadius::same(8);
        v.extreme_bg_color = WELL;
        v.faint_bg_color = CARD_LIGHT;
        v.override_text_color = Some(TEXT);
        v.selection.bg_fill = ACCENT.gamma_multiply(0.55);
        v.selection.stroke = Stroke::new(1.0, ACCENT);
        v.hyperlink_color = Color32::from_rgb(0x4d, 0xa3, 0xff);
        let radius = CornerRadius::same(WIDGET_RADIUS);
        for w in [
            &mut v.widgets.noninteractive,
            &mut v.widgets.inactive,
            &mut v.widgets.hovered,
            &mut v.widgets.active,
            &mut v.widgets.open,
        ] {
            w.corner_radius = radius;
        }
        v.widgets.noninteractive.bg_stroke = Stroke::new(1.0, LINE);
        v.widgets.inactive.bg_fill = CARD_LIGHT;
        v.widgets.inactive.weak_bg_fill = CARD_LIGHT;
        v.widgets.inactive.bg_stroke = Stroke::new(1.0, LINE);
        v.widgets.hovered.bg_fill = LINE;
        v.widgets.hovered.weak_bg_fill = LINE;
        v.widgets.hovered.bg_stroke = Stroke::new(1.0, Color32::from_rgb(0x4a, 0x4a, 0x4e));
        v.widgets.active.bg_fill = ACCENT;
        v.widgets.active.weak_bg_fill = ACCENT.gamma_multiply(0.8);
        v.widgets.open.weak_bg_fill = LINE;
        v.slider_trailing_fill = true;
        style.spacing.item_spacing = Vec2::new(8.0, 6.0);
        style.spacing.button_padding = Vec2::new(10.0, 5.0);
        style.spacing.slider_width = 160.0;
        style.spacing.interact_size.y = 26.0;
        style.spacing.menu_margin = Margin::same(6);
        for (text_style, font) in style.text_styles.iter_mut() {
            font.size = match text_style {
                egui::TextStyle::Heading => 20.0,
                egui::TextStyle::Body | egui::TextStyle::Button => 13.0,
                egui::TextStyle::Small => 11.0,
                egui::TextStyle::Monospace => 12.5,
                _ => font.size,
            };
        }
    });
}

/// A grouped panel (macOS "inset grouped"): the inspector's and the pages' boxes.
pub fn card_frame() -> Frame {
    Frame::new()
        .fill(CARD)
        .stroke(Stroke::new(1.0, LINE))
        .corner_radius(CornerRadius::same(CARD_RADIUS))
        .inner_margin(Margin::same(14))
}

/// A section's header: small, grey, semibold, as in an inspector.
pub fn label(ui: &mut Ui, text: &str) {
    ui.label(semibold(text, 11.5).color(TERTIARY));
}

pub fn muted(text: impl Into<String>) -> RichText {
    RichText::new(text).color(MUTED).size(12.0)
}

/// An on/off switch, blue when on (as on a Mac).
pub fn toggle(ui: &mut Ui, on: &mut bool) -> Response {
    let size = Vec2::new(34.0, 20.0);
    let (rect, mut response) = ui.allocate_exact_size(size, Sense::click());
    if response.clicked() {
        *on = !*on;
        response.mark_changed();
    }
    let t = ui.ctx().animate_bool_responsive(response.id, *on);
    let fill = Color32::from_rgb(0x48, 0x48, 0x4c).lerp_to_gamma(ACCENT, t);
    ui.painter().rect_filled(rect, CornerRadius::same(10), fill);
    let x = egui::lerp((rect.left() + 10.0)..=(rect.right() - 10.0), t);
    ui.painter().circle_filled(egui::pos2(x, rect.center().y), 8.0, Color32::WHITE);
    response
}

/// The blue button: the one main thing to do here.
pub fn primary_button(ui: &mut Ui, text: &str) -> Response {
    let button = egui::Button::new(semibold(text, 13.0).color(Color32::WHITE))
        .fill(ACCENT)
        .stroke(Stroke::NONE)
        .corner_radius(CornerRadius::same(WIDGET_RADIUS))
        .min_size(Vec2::new(0.0, 28.0));
    ui.add(button)
}

/// A plain button.
pub fn button(ui: &mut Ui, text: &str) -> Response {
    ui.add(
        egui::Button::new(RichText::new(text).size(13.0))
            .fill(LINE)
            .stroke(Stroke::NONE)
            .corner_radius(CornerRadius::same(WIDGET_RADIUS))
            .min_size(Vec2::new(0.0, 28.0)),
    )
}

/// A red button, for what cannot be undone.
pub fn danger_button(ui: &mut Ui, text: &str) -> Response {
    ui.add(
        egui::Button::new(semibold(text, 13.0).color(Color32::WHITE))
            .fill(RED)
            .stroke(Stroke::NONE)
            .corner_radius(CornerRadius::same(WIDGET_RADIUS))
            .min_size(Vec2::new(0.0, 28.0)),
    )
}

/// A status capsule: a coloured dot (or none) and text.
pub fn pill(ui: &mut Ui, dot: Option<Color32>, text: &str) {
    Frame::new()
        .fill(WELL)
        .corner_radius(CornerRadius::same(255))
        .inner_margin(Margin::symmetric(10, 4))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 6.0;
                if let Some(c) = dot {
                    let (r, _) = ui.allocate_exact_size(Vec2::splat(7.0), Sense::hover());
                    ui.painter().circle_filled(r.center(), 3.5, c);
                }
                ui.label(RichText::new(text).size(12.0).color(TEXT));
            });
        });
}

/// A status dot.
pub fn dot(ui: &mut Ui, colour: Color32) {
    let (r, _) = ui.allocate_exact_size(Vec2::splat(8.0), Sense::hover());
    ui.painter().circle_filled(r.center(), 3.5, colour);
}

/// A macOS segmented control: a dark track, the chosen part raised.
pub fn segmented<T: PartialEq + Copy>(ui: &mut Ui, value: &mut T, options: &[(T, &str)]) -> Response {
    let mut changed = false;
    let inner = Frame::new()
        .fill(WELL)
        .corner_radius(CornerRadius::same(WIDGET_RADIUS + 1))
        .inner_margin(Margin::same(2))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 2.0;
                for (option, name) in options {
                    let on = *value == *option;
                    let text = RichText::new(*name).size(12.0).color(if on { TEXT } else { MUTED });
                    let b = egui::Button::new(text)
                        .fill(if on { LINE } else { Color32::TRANSPARENT })
                        .stroke(Stroke::NONE)
                        .corner_radius(CornerRadius::same(WIDGET_RADIUS - 1))
                        .min_size(Vec2::new(0.0, 22.0));
                    if ui.add(b).clicked() && !on {
                        *value = *option;
                        changed = true;
                    }
                }
            });
        });
    let mut r = inner.response;
    if changed {
        r.mark_changed();
    }
    r
}

/// An inspector row: the label on the left, right-aligned in its column,
/// the control after it.
pub fn row<R>(ui: &mut Ui, label: &str, add: impl FnOnce(&mut Ui) -> R) -> R {
    ui.horizontal(|ui| {
        let (rect, _) = ui.allocate_exact_size(Vec2::new(104.0, 24.0), Sense::hover());
        ui.painter().text(rect.right_center() - Vec2::new(4.0, 0.0), egui::Align2::RIGHT_CENTER, label, egui::FontId::proportional(12.0), MUTED);
        add(ui)
    })
    .inner
}

/// A thin line between groups.
pub fn separator(ui: &mut Ui) {
    let w = ui.available_width();
    let (r, _) = ui.allocate_exact_size(Vec2::new(w, 9.0), Sense::hover());
    ui.painter().line_segment([r.left_center(), r.right_center()], Stroke::new(1.0, LINE));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hebrew_reads_right_to_left() {
        assert_eq!(visual("living-room"), "living-room");
        // "סלון TV": the Hebrew word on the right, the Latin one on the left.
        assert_eq!(visual("סלון TV"), "TV ןולס");
        assert_eq!(visual("סלון"), "ןולס");
        assert_eq!(visual("מסך 2"), "2 ךסמ");
    }
}
