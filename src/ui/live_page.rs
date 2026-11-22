//! Live: the show while it plays. The program in the middle, the pads under
//! it, an inspector on the right (Look, Sound, Outputs), and the set's
//! timeline along the bottom: what happened over the last twenty minutes.

use egui::{Color32, CornerRadius, Rect, RichText, Sense, Stroke, Ui, pos2, vec2};

use super::icons::{self, Icon};
use super::lights_page;
use super::style;
use super::widgets;
use super::{Live, PageState, UiAction};
use crate::app_state::{Action, AppState};
use crate::director::{THEMES, ThemeLength};
use crate::lights::{Effect, SmokeMode};
use crate::settings::{OutputPattern, Settings, VARIETY_RANGE};
use crate::timeline::Run;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Tab {
    #[default]
    Look,
    Sound,
    Outputs,
}

/// The section names, by number (as the analysis numbers them).
pub const SECTIONS: [&str; 5] = ["Silence", "Groove", "Build", "Drop", "Breakdown"];

/// "red room" → "Red room".
pub fn title_case(name: &str) -> String {
    let mut c = name.chars();
    c.next().map(|f| f.to_uppercase().collect::<String>() + c.as_str()).unwrap_or_default()
}

const INSPECTOR_W: f32 = 300.0;
const TIMELINE_H: f32 = 132.0;

pub fn draw(ui: &mut Ui, state: &AppState, settings: &mut Settings, live: &Live, preview: Option<egui::TextureId>, pages: &mut PageState, actions: &mut Vec<UiAction>) {
    let area = ui.available_rect_before_wrap();
    // The inspector narrows with the window, and only gives way on a very small one.
    let inspector_w = if area.width() > 640.0 { (area.width() * 0.3).clamp(240.0, INSPECTOR_W) } else { 0.0 };
    let timeline = Rect::from_min_max(pos2(area.left(), area.bottom() - TIMELINE_H), area.max);
    let top = Rect::from_min_max(area.min, pos2(area.right(), timeline.top() - style::GAP));
    let inspector = Rect::from_min_max(pos2(top.right() - inspector_w, top.top()), top.max);
    let program = Rect::from_min_max(top.min, pos2(if inspector_w > 0.0 { inspector.left() - style::GAP } else { top.right() }, top.bottom()));

    ui.scope_builder(egui::UiBuilder::new().max_rect(program), |ui| program_column(ui, state, settings, live, preview, pages, actions));
    if inspector_w > 0.0 {
        ui.scope_builder(egui::UiBuilder::new().max_rect(inspector), |ui| inspector_panel(ui, settings, live, pages, actions));
    }
    ui.scope_builder(egui::UiBuilder::new().max_rect(timeline), |ui| timeline_panel(ui, live));
    ui.advance_cursor_after_rect(area);
}

fn program_column(ui: &mut Ui, state: &AppState, settings: &mut Settings, live: &Live, preview: Option<egui::TextureId>, pages: &mut PageState, actions: &mut Vec<UiAction>) {
    // The program: as big as fits above the pads, in the picture's shape.
    let avail = ui.available_rect_before_wrap();
    let pads_h = 84.0;
    let strip_h = LIGHTS_STRIP_H;
    let aspect = if live.preview_aspect > 0.0 { live.preview_aspect } else { 16.0 / 9.0 };
    let room = vec2(avail.width(), (avail.height() - pads_h - strip_h - 2.0 * style::GAP).max(120.0));
    let size = if room.x / room.y > aspect { vec2(room.y * aspect, room.y) } else { vec2(room.x, room.x / aspect) };
    let monitor = Rect::from_min_size(pos2(avail.center().x - size.x / 2.0, avail.top()), size);
    let p = ui.painter();
    p.rect_filled(monitor, CornerRadius::same(8), Color32::BLACK);
    if let Some(t) = preview {
        egui::Image::new((t, monitor.size())).corner_radius(CornerRadius::same(8)).paint_at(ui, monitor);
    }
    let p = ui.painter();
    p.rect_stroke(monitor, CornerRadius::same(8), Stroke::new(1.0, style::LINE), egui::StrokeKind::Outside);
    // What it is, top left; what is happening, top right.
    let tag = |p: &egui::Painter, at: egui::Pos2, align: egui::Align2, text: String, colour: Color32| {
        let galley = p.layout_no_wrap(text, egui::FontId::new(11.5, egui::FontFamily::Name("semibold".into())), colour);
        let r = align.anchor_size(at, galley.size() + vec2(14.0, 8.0));
        p.rect_filled(r, CornerRadius::same(5), Color32::from_black_alpha(170));
        p.galley(r.min + vec2(7.0, 4.0), galley, colour);
    };
    let program = if state.running { "PROGRAM" } else { "PREVIEW · not started" };
    tag(p, monitor.left_top() + vec2(10.0, 10.0), egui::Align2::LEFT_TOP, program.into(), style::TEXT);
    let now = format!("{} · {}{}", live.section, live.theme, if live.look_held { " · held" } else { "" });
    tag(p, monitor.right_top() + vec2(-10.0, 10.0), egui::Align2::RIGHT_TOP, now, if live.look_held { style::AMBER } else { style::TEXT });
    if !state.running {
        // Not started: the one thing to do, in the middle.
        let b = Rect::from_center_size(monitor.center(), vec2(180.0, 40.0));
        let r = ui.interact(b, ui.scope_id().with("start in program"), Sense::click());
        let p = ui.painter();
        p.rect_filled(b, CornerRadius::same(8), if r.hovered() { style::ACCENT.gamma_multiply(1.15) } else { style::ACCENT });
        icons::paint(ui, Rect::from_center_size(b.left_center() + vec2(26.0, 0.0), vec2(14.0, 14.0)), Icon::Play, Color32::WHITE);
        ui.painter().text(b.center() + vec2(10.0, 0.0), egui::Align2::CENTER_CENTER, "Start the show", egui::FontId::new(14.0, egui::FontFamily::Name("semibold".into())), Color32::WHITE);
        if r.on_hover_text(if live.output.is_some() { "On the outputs; this window stays the controls" } else { "No output connected: full screen on this display" }).clicked() {
            actions.push(UiAction::App(Action::Start));
        }
    }
    // The pads.
    let pads_rect = Rect::from_min_max(pos2(avail.left(), monitor.bottom() + style::GAP), pos2(avail.right(), monitor.bottom() + style::GAP + pads_h));
    ui.scope_builder(egui::UiBuilder::new().max_rect(pads_rect), |ui| pads(ui, state, settings, live, pages, actions));
    // The lights, as a strip under the pads: how they are doing right now.
    let strip = Rect::from_min_size(pos2(avail.left(), pads_rect.bottom() + style::GAP - 10.0), vec2(avail.width(), strip_h));
    lights_strip(ui, strip, settings, live, actions);
}

/// The height of the lights strip on the Live page.
const LIGHTS_STRIP_H: f32 = 40.0;

/// A glance at the DMX lights from Live: the interface, the effect playing,
/// Drive, and each light's colour now. Clicking it opens the Lights page.
fn lights_strip(ui: &mut Ui, rect: Rect, settings: &Settings, live: &Live, actions: &mut Vec<UiAction>) {
    use crate::app_state::Page;
    let l = &settings.lighting;
    let r = ui.interact(rect, ui.scope_id().with("lights strip"), Sense::click());
    let p = ui.painter();
    p.rect(rect, CornerRadius::same(9), if r.hovered() { style::CARD_LIGHT } else { style::CARD }, Stroke::new(1.0, style::LINE), egui::StrokeKind::Inside);
    let y = rect.center().y;
    let mut x = rect.left() + 14.0;
    let small = |ui: &Ui, x: f32, icon: Icon, colour: Color32| icons::paint(ui, Rect::from_center_size(pos2(x + 7.0, y), vec2(14.0, 14.0)), icon, colour);
    let text = |ui: &Ui, x: f32, t: &str, colour: Color32, bold: bool| -> f32 {
        let family = if bold { egui::FontFamily::Name("semibold".into()) } else { egui::FontFamily::Proportional };
        ui.painter().text(pos2(x, y), egui::Align2::LEFT_CENTER, t, egui::FontId::new(12.5, family), colour).width()
    };
    small(ui, x, Icon::Spot, style::TEXT);
    x += 22.0;
    x += text(ui, x, "Lights", style::TEXT, true) + 14.0;
    if l.fixtures.is_empty() {
        text(ui, x, "None set up yet: open Lights to add them", style::MUTED, false);
    } else {
        // The interface.
        let (dot, state) = match &live.dmx {
            Some(d) if d.sending => (style::GREEN, d.kind.unwrap_or("DMX")),
            Some(_) => (style::RED, "No interface"),
            None => (style::TERTIARY, "Off"),
        };
        ui.painter().circle_filled(pos2(x + 4.0, y), 4.0, dot);
        x += 14.0;
        x += text(ui, x, state, style::MUTED, false) + 16.0;
        // What they play.
        if l.blackout {
            small(ui, x, Icon::Moon, style::RED);
            x += 20.0;
            x += text(ui, x, "Lights out", style::RED, true) + 16.0;
        } else if let Some(e) = live.light_effect {
            small(ui, x, lights_page::effect_icon(e), style::CYAN);
            x += 20.0;
            x += text(ui, x, e.name(), style::TEXT, true) + 6.0;
            x += text(ui, x, if l.effect.is_none() { "Auto" } else { "locked" }, style::TERTIARY, false) + 16.0;
        }
        let word = match (l.drive * 4.0).round() as i32 {
            0 => "Subtle",
            1 => "Soft",
            2 => "Normal",
            3 => "Strong",
            _ => "Wild",
        };
        small(ui, x, Icon::Gauge, style::MUTED);
        x += 20.0;
        x += text(ui, x, word, style::MUTED, false) + 18.0;
        // Each light's colour now, as long as there is room before the arrow.
        let end = rect.right() - 40.0;
        for (i, f) in l.fixtures.iter().enumerate() {
            if x + 20.0 > end {
                break;
            }
            let c = pos2(x + 8.0, y);
            if f.is_smoke() {
                icons::paint(ui, Rect::from_center_size(c, vec2(14.0, 14.0)), Icon::Smoke, style::MUTED);
            } else {
                let colour = lights_page::rgb(live.lights.get(i).copied().unwrap_or([0.0; 3]));
                ui.painter().circle_filled(c, 10.0, colour.gamma_multiply(0.18));
                ui.painter().circle(c, 6.5, colour, Stroke::new(1.0, Color32::from_white_alpha(50)));
            }
            x += 22.0;
        }
    }
    icons::paint(ui, Rect::from_center_size(pos2(rect.right() - 20.0, y), vec2(14.0, 14.0)), Icon::Next, if r.hovered() { style::TEXT } else { style::TERTIARY });
    if r.on_hover_text("Open Lights").clicked() {
        actions.push(UiAction::App(Action::Open(Page::Lights)));
    }
}

/// The director's pad: "Director" over its state, Auto (changing by itself)
/// or Held, with a moving dot while it is in charge.
fn director_pad(ui: &mut Ui, w: f32, word: &str, sub: &str, auto: bool) -> egui::Response {
    let (rect, r) = ui.allocate_exact_size(vec2(w, 74.0), Sense::click());
    let fill = if r.hovered() { style::LINE } else { style::CARD_LIGHT };
    let edge = if auto { style::GREEN } else { style::AMBER };
    let p = ui.painter();
    p.rect(rect, CornerRadius::same(9), fill, Stroke::new(1.5, edge), egui::StrokeKind::Inside);
    let room = rect.width() - 10.0;
    widgets::fit_text(p, rect.center_top() + vec2(0.0, 14.0), "Director", 10.5, egui::FontFamily::Proportional, style::MUTED, room);
    widgets::fit_text(p, rect.center() + vec2(0.0, 1.0), word, 16.0, egui::FontFamily::Name("semibold".into()), edge, room);
    widgets::fit_text(p, rect.center() + vec2(0.0, 22.0), sub, 10.0, egui::FontFamily::Proportional, style::TERTIARY, room);
    if auto {
        // A slow pulse: it is working.
        let t = ui.input(|i| i.time) as f32;
        let a = 0.35 + 0.65 * (0.5 + 0.5 * (t * 2.2).sin());
        ui.painter().circle_filled(rect.right_top() + vec2(-10.0, 10.0), 3.5, style::GREEN.gamma_multiply(a));
        ui.ctx().request_repaint();
    }
    r.on_hover_text(if auto { "Auto: the looks change by themselves, on phrases, following the vibe. Press to hold this look." } else { "Held: nothing changes on its own. Press to go back to Auto." })
}

fn pads(ui: &mut Ui, state: &AppState, settings: &mut Settings, live: &Live, pages: &mut PageState, actions: &mut Vec<UiAction>) {
    let n = 8.0;
    let gap = 8.0;
    let w = ((ui.available_width() - gap * (n - 1.0)) / n).max(64.0);
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = gap;
        // The director: Auto while it changes the looks by itself (on phrases,
        // following the vibe); press to hold the look, again to let it go.
        let (word, sub) = if live.look_held { ("Held", "L · press for auto") } else { ("Auto", "L · press to hold") };
        if director_pad(ui, w, word, sub, !live.look_held).clicked() {
            actions.push(UiAction::HoldLook);
        }
        if widgets::pad(ui, vec2(w, 74.0), Icon::Next, "Next look", "Space", false, "A new look now (the director changes theme, background and pattern)").clicked() {
            actions.push(UiAction::NextLook);
        }
        if widgets::pad(ui, vec2(w, 74.0), Icon::Palette, "Palette", "P", false, "The next colour palette").clicked() {
            actions.push(UiAction::NextPalette);
        }
        if widgets::pad(ui, vec2(w, 74.0), Icon::Bolt, "Strobe", "S", settings.strobe, "Flashes on or off (never more than three a second)").clicked() {
            settings.strobe = !settings.strobe;
        }
        let dark = settings.lighting.blackout;
        if widgets::pad_coloured(ui, vec2(w, 74.0), Icon::Moon, "Lights out", "", dark, style::RED.gamma_multiply(0.75), "Every DMX light dark; the screens go on").clicked() {
            settings.lighting.blackout = !dark;
        }
        let has_smoke = settings.lighting.fixtures.iter().any(|f| f.is_smoke());
        let held = pages.lights.smoke_held;
        let smoke = ui.add_enabled_ui(has_smoke, |ui| widgets::pad(ui, vec2(w, 74.0), Icon::Smoke, "Smoke", "hold", held, if has_smoke { "Smoke while held" } else { "No smoke machine set up (Lights)" })).inner;
        pages.lights.smoke_held = smoke.is_pointer_button_down_on() && has_smoke;
        if pages.lights.smoke_held && settings.lighting.smoke == SmokeMode::Off {
            settings.lighting.smoke = SmokeMode::Hand;
        }
        let grid = settings.output.outputs.iter().all(|o| o.showing == OutputPattern::Grid);
        if widgets::pad(ui, vec2(w, 74.0), Icon::Grid, "Grid", "", grid, "The calibration grid on every output, instead of the show").clicked() {
            for o in &mut settings.output.outputs {
                o.showing = if grid { OutputPattern::Show } else { OutputPattern::Grid };
            }
        }
        if widgets::pad(ui, vec2(w, 74.0), Icon::Expand, "Full screen", "F", state.show_here, "The show full screen on this display (Esc comes back)").clicked() {
            actions.push(UiAction::App(Action::PreviewHere));
        }
    });
}

fn inspector_panel(ui: &mut Ui, settings: &mut Settings, live: &Live, pages: &mut PageState, actions: &mut Vec<UiAction>) {
    let rect = ui.available_rect_before_wrap();
    ui.painter().rect(rect, CornerRadius::same(style::CARD_RADIUS), style::CARD, Stroke::new(1.0, style::LINE), egui::StrokeKind::Inside);
    ui.scope_builder(egui::UiBuilder::new().max_rect(rect.shrink(12.0)), |ui| {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 0.0;
            let w = ui.available_width();
            ui.scope(|ui| {
                ui.set_width(w);
                style::segmented(ui, &mut pages.live_tab, &[(Tab::Look, "Look"), (Tab::Sound, "Sound"), (Tab::Outputs, "Outputs")]);
            });
        });
        ui.add_space(10.0);
        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| match pages.live_tab {
            Tab::Look => look_tab(ui, settings, live),
            Tab::Sound => sound_tab(ui, settings, live, actions),
            Tab::Outputs => outputs_tab(ui, settings, live),
        });
    });
}

fn look_tab(ui: &mut Ui, settings: &mut Settings, live: &Live) {
    style::label(ui, "Theme length");
    let options: Vec<(ThemeLength, Icon, &str)> = ThemeLength::ALL
        .iter()
        .map(|l| {
            let icon = match l {
                ThemeLength::Short => Icon::Bolt,
                ThemeLength::Medium => Icon::Pulse,
                ThemeLength::Long => Icon::Hold,
            };
            (*l, icon, l.name())
        })
        .collect();
    let bpm = live.bpm;
    widgets::icon_segmented(ui, &mut settings.theme_length, &options, ui.available_width(), |l| {
        format!("A theme holds {} phrases, {} at this tempo, unless a drop or a new track ends it sooner", l.phrases(), l.about(bpm))
    });
    style::separator(ui);
    style::label(ui, "Themes in rotation");
    for (i, theme) in THEMES.iter().enumerate() {
        let mut on = settings.themes.get(i).copied().unwrap_or(true);
        ui.horizontal(|ui| {
            let (r, _) = ui.allocate_exact_size(vec2(10.0, 10.0), Sense::hover());
            ui.painter().circle_filled(r.center(), 4.0, if live.theme == title_case(theme.name) { style::ACCENT } else { style::TERTIARY });
            ui.label(title_case(theme.name));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if style::toggle(ui, &mut on).changed()
                    && let Some(t) = settings.themes.get_mut(i)
                {
                    *t = on;
                }
            });
        });
    }
    style::separator(ui);
    style::label(ui, "Vibe");
    vibe_meter(ui, settings, live);
    style::row(ui, "Follow the vibe", |ui| style::toggle(ui, &mut settings.follow_vibe));
    style::row(ui, "Variety", |ui| {
        ui.spacing_mut().slider_width = (ui.available_width() - 10.0).max(60.0);
        ui.add(egui::Slider::new(&mut settings.variety, VARIETY_RANGE.0..=VARIETY_RANGE.1).show_value(false))
            .on_hover_text("Left: the vibe leads. Right: more surprises.")
    });
}

fn vibe_meter(ui: &mut Ui, settings: &Settings, live: &Live) {
    let w = ui.available_width();
    let (rect, _) = ui.allocate_exact_size(vec2(w, 74.0), Sense::hover());
    let p = ui.painter();
    p.rect(rect, CornerRadius::same(7), style::WELL, Stroke::new(1.0, style::LINE), egui::StrokeKind::Inside);
    p.line_segment([rect.center_top(), rect.center_bottom()], Stroke::new(1.0, style::LINE));
    p.line_segment([rect.left_center(), rect.right_center()], Stroke::new(1.0, style::LINE));
    let font = egui::FontId::proportional(10.0);
    p.text(rect.left_center() + vec2(6.0, -8.0), egui::Align2::LEFT_CENTER, "dark", font.clone(), style::TERTIARY);
    p.text(rect.right_center() + vec2(-6.0, -8.0), egui::Align2::RIGHT_CENTER, "bright", font.clone(), style::TERTIARY);
    p.text(rect.center_top() + vec2(6.0, 8.0), egui::Align2::LEFT_CENTER, "intense", font.clone(), style::TERTIARY);
    p.text(rect.center_bottom() + vec2(6.0, -8.0), egui::Align2::LEFT_CENTER, "calm", font, style::TERTIARY);
    let v = live.vibe;
    if v.sure > 0.0 && settings.follow_vibe {
        let at = pos2(rect.center().x + v.brightness * rect.width() * 0.45, rect.center().y - v.intensity * rect.height() * 0.42);
        let c = style::ACCENT.gamma_multiply(0.4 + 0.6 * v.sure);
        p.circle_filled(at, 8.0, c.gamma_multiply(0.3));
        p.circle_filled(at, 5.0, c);
    }
    ui.label(style::semibold(if settings.follow_vibe { title_case(&v.name().to_lowercase()) } else { "Not followed".into() }, 13.0));
}

fn sound_tab(ui: &mut Ui, settings: &mut Settings, live: &Live, actions: &mut Vec<UiAction>) {
    style::label(ui, "Input");
    let shown = settings.input_device.clone().unwrap_or_else(|| format!("Automatic · {}", live.input));
    egui::ComboBox::from_id_salt("input").selected_text(shown).width(ui.available_width()).show_ui(ui, |ui| {
        if ui.selectable_label(settings.input_device.is_none(), "Automatic").clicked() {
            actions.push(UiAction::SelectInput(None));
        }
        for name in &live.inputs {
            if ui.selectable_label(settings.input_device.as_ref() == Some(name), name).clicked() {
                actions.push(UiAction::SelectInput(Some(name.clone())));
            }
        }
    });
    ui.add_space(6.0);
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), 40.0), Sense::hover());
    let n = live.levels.len().max(1);
    let w = rect.width() / n as f32;
    for (i, level) in live.levels.iter().enumerate() {
        let h = (level.clamp(0.0, 1.0) * rect.height()).max(2.0);
        let bar = Rect::from_min_max(pos2(rect.left() + i as f32 * w + 1.0, rect.bottom() - h), pos2(rect.left() + (i + 1) as f32 * w - 1.0, rect.bottom()));
        ui.painter().rect_filled(bar, CornerRadius::same(2), if *level > 0.75 { style::AMBER } else { style::GREEN });
    }
    let status = if live.levels.iter().all(|l| *l < 0.02) {
        "No sound arriving"
    } else if live.kick_found {
        "Level good · kick found"
    } else {
        "Listening · no kick yet"
    };
    ui.label(style::muted(status));
    style::separator(ui);
    style::label(ui, "Tempo");
    style::row(ui, "BPM", |ui| ui.label(style::semibold(if live.bpm > 0.0 { format!("{:.1}", live.bpm) } else { "—".into() }, 13.0)));
    style::row(ui, "Beat grid", |ui| ui.label(if live.locked { RichText::new("locked").color(style::GREEN) } else { RichText::new("searching").color(style::MUTED) }));
    style::separator(ui);
    style::label(ui, "Sync");
    style::row(ui, "Visuals", |ui| {
        ui.spacing_mut().slider_width = (ui.available_width() - 60.0).max(60.0);
        ui.add(egui::Slider::new(&mut settings.sync_offset_ms, -300.0..=300.0).step_by(5.0).suffix(" ms"))
    });
    ui.label(style::muted("+ shows the visuals earlier. Also [ and ] while the show plays."));
    style::separator(ui);
    style::label(ui, "Microphone sensitivity");
    style::row(ui, "Sensitivity", |ui| {
        ui.spacing_mut().slider_width = (ui.available_width() - 60.0).max(60.0);
        ui.add(
            egui::Slider::new(&mut settings.input_gain, crate::settings::INPUT_GAIN_RANGE.0..=crate::settings::INPUT_GAIN_RANGE.1)
                .logarithmic(true)
                .custom_formatter(|v, _| format!("{:+.0} dB", 20.0 * v.log10()))
                .custom_parser(|t| t.trim().trim_end_matches("dB").trim().parse::<f64>().ok().map(|db| 10f64.powf(db / 20.0))),
        )
        .on_hover_text("Up for a quiet room or a far microphone, down if the meter is always red. The middle (0 dB) is the sound as it comes.")
    });
    ui.label(style::muted("Watch the meter above: green most of the time, amber on the loudest kicks."));
}

fn outputs_tab(ui: &mut Ui, settings: &mut Settings, live: &Live) {
    style::label(ui, "Screens");
    if settings.output.outputs.is_empty() {
        ui.label(style::muted("No outputs"));
    }
    for (k, o) in settings.output.outputs.iter().enumerate() {
        let connected = live.output_sizes.get(k).is_some_and(|s| s.is_some());
        let link = live.net_links.get(k).cloned().flatten();
        let detail = match (&o.display, connected, &link) {
            (None, _, _) => "No display chosen".to_string(),
            (Some(_), true, Some(l)) => format!("Network · {:.0} ms ahead", l.latency_ms),
            (Some(_), true, None) => {
                let (w, h) = live.output_sizes.get(k).copied().flatten().unwrap_or((0, 0));
                format!("{w} × {h}")
            }
            (Some(_), false, _) => "Not connected".into(),
        };
        ui.horizontal(|ui| {
            style::dot(ui, if connected { style::GREEN } else if o.display.is_some() { style::AMBER } else { style::TERTIARY });
            ui.vertical(|ui| {
                ui.label(RichText::new(format!("{} · {}", o.name, style::visual(o.display.as_deref().unwrap_or("—")))).size(12.5));
                ui.label(style::muted(detail));
            });
        });
    }
    style::separator(ui);
    style::label(ui, "Lights");
    let (dot, text) = match &live.dmx {
        Some(d) if d.sending => (style::GREEN, format!("{} · {:.0} frames/s", d.kind.unwrap_or("DMX"), d.rate)),
        Some(d) => (style::AMBER, d.error.clone().unwrap_or_else(|| "Looking for the interface…".into())),
        None => (style::TERTIARY, "Off".into()),
    };
    ui.horizontal(|ui| {
        style::dot(ui, dot);
        ui.label(RichText::new(text).size(12.5));
    });
    let effect = match settings.lighting.effect {
        Some(e) => format!("{} (locked)", e.name()),
        None => format!("Auto · {}", live.light_effect.map(Effect::name).unwrap_or("—")),
    };
    style::row(ui, "Effect", |ui| ui.label(RichText::new(effect).size(12.5)));
    style::row(ui, "Drive", |ui| {
        ui.spacing_mut().slider_width = (ui.available_width() - 10.0).max(60.0);
        ui.add(egui::Slider::new(&mut settings.lighting.drive, 0.0..=1.0).show_value(false))
    });
    style::separator(ui);
    style::label(ui, "Flash safety");
    style::row(ui, "Strobe", |ui| style::toggle(ui, &mut settings.strobe));
    style::row(ui, "Brightest", |ui| {
        ui.spacing_mut().slider_width = (ui.available_width() - 50.0).max(60.0);
        ui.add(egui::Slider::new(&mut settings.max_flash, 0.0..=0.6).fixed_decimals(2))
    });
    ui.label(style::muted("Never more than three flashes a second."));
}

/// The set's last twenty minutes: sections, looks, the lights' effect.
fn timeline_panel(ui: &mut Ui, live: &Live) {
    let rect = ui.available_rect_before_wrap();
    ui.painter().rect(rect, CornerRadius::same(style::CARD_RADIUS), style::CARD, Stroke::new(1.0, style::LINE), egui::StrokeKind::Inside);
    let inner = rect.shrink2(vec2(14.0, 10.0));
    let p = ui.painter();
    p.text(inner.left_top(), egui::Align2::LEFT_TOP, "The set · last 20 minutes", egui::FontId::new(12.5, egui::FontFamily::Name("semibold".into())), style::TEXT);
    let mins = live.timeline_secs / 60;
    p.text(inner.right_top(), egui::Align2::RIGHT_TOP, format!("{}:{:02} recorded", mins / 60, mins % 60), egui::FontId::proportional(11.5), style::MUTED);
    let lanes_top = inner.top() + 22.0;
    let lane_h = ((inner.bottom() - lanes_top) / 3.0 - 6.0).max(14.0);
    let label_w = 76.0;
    let names = ["Sections", "Looks", "Lights"];
    let section_colour = |v: u8| match v {
        0 => style::TERTIARY,
        1 => Color32::from_rgb(0x8e, 0x8e, 0x96),
        2 => style::AMBER,
        3 => style::ROSE,
        _ => style::PURPLE,
    };
    for (k, (name, runs)) in names.iter().zip(&live.timeline).enumerate() {
        let y = lanes_top + k as f32 * (lane_h + 6.0);
        p.text(pos2(inner.left(), y + lane_h / 2.0), egui::Align2::LEFT_CENTER, *name, egui::FontId::proportional(11.5), style::MUTED);
        let track = Rect::from_min_size(pos2(inner.left() + label_w, y), vec2(inner.width() - label_w, lane_h));
        p.rect_filled(track, CornerRadius::same(4), style::WELL);
        for r in runs {
            let (label, colour) = match k {
                0 => (SECTIONS.get(r.value as usize).copied().unwrap_or("").to_string(), section_colour(r.value)),
                1 => (THEMES.get(r.value as usize).map(|t| title_case(t.name)).unwrap_or_default(), style::SURFACE_COLOURS[r.value as usize % 8]),
                _ => (Effect::ALL.get(r.value as usize).map(|e| e.name().to_string()).unwrap_or_default(), style::ACCENT),
            };
            clip(p, track, r, &label, colour);
        }
    }
    // Now, at the right end.
    let x = inner.right() - 1.0;
    p.line_segment([pos2(x, lanes_top - 4.0), pos2(x, inner.bottom())], Stroke::new(2.0, style::RED));
}

fn clip(p: &egui::Painter, track: Rect, r: &Run, label: &str, colour: Color32) {
    let x0 = track.left() + r.from * track.width();
    let x1 = track.left() + r.to * track.width();
    if x1 - x0 < 1.0 {
        return;
    }
    let c = Rect::from_min_max(pos2(x0 + 1.0, track.top() + 2.0), pos2(x1 - 1.0, track.bottom() - 2.0));
    p.rect(c, CornerRadius::same(3), colour.gamma_multiply(0.22), Stroke::new(1.0, colour.gamma_multiply(0.9)), egui::StrokeKind::Inside);
    if c.width() > 40.0 {
        p.with_clip_rect(c).text(c.left_center() + vec2(6.0, 0.0), egui::Align2::LEFT_CENTER, label, egui::FontId::proportional(11.0), style::TEXT);
    }
}
