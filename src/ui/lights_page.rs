//! Lights: the DMX lights, laid out as the rest of the window is, in pads
//! and cards with icons.
//!
//! Left, top to bottom: the interface and the master controls over every
//! light; the effect the lights play; the rig, one card per light with its
//! live colour; and the channel finder, folded away until it is needed.
//! Right, the inspector: only what belongs to the selected card, a light's
//! settings and tests, or the smoke machine's.

use egui::{Color32, CornerRadius, Rect, RichText, Sense, Stroke, Ui, pos2, vec2};

use super::icons::{self, Icon};
use super::style;
use super::widgets;
use super::{Live, UiAction};
use crate::lights::{Channel, Effect, Fixture, LightStrobe, Lighting, MAX_FIXTURES, Profile, Role, SmokeMode, UNIVERSE};
use crate::settings::Settings;

/// A test that takes over the lights for a moment.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum LightTest {
    #[default]
    None,
    /// Only this channel (1-based) at this value, and the held channel (a
    /// dimmer, 0: none) at full; every other channel 0.
    Channel(u16, u8, u16),
    /// This light in this colour at full; the others as the show has them.
    Colour(usize, [f32; 3]),
}

#[derive(Clone, Debug)]
pub struct LightsState {
    pub selected: usize,
    pub test: LightTest,
    /// The smoke button is held down.
    pub smoke_held: bool,
    /// The channel finder's channel and value.
    pub channel: u16,
    pub value: u8,
    /// Kept at full with it (a light's dimmer); 0: none.
    pub hold: u16,
    /// The channel finder is open.
    pub finder: bool,
}

impl Default for LightsState {
    fn default() -> Self {
        LightsState { selected: 0, test: LightTest::None, smoke_held: false, channel: 1, value: 255, hold: 0, finder: false }
    }
}

/// The interface's state, for the page.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct DmxLive {
    pub port: Option<String>,
    pub kind: Option<&'static str>,
    pub sending: bool,
    /// Frames a second lately.
    pub rate: f32,
    pub error: Option<String>,
}

pub(super) fn rgb(c: [f32; 3]) -> Color32 {
    // Linear light to screen colour.
    let s = |v: f32| (v.clamp(0.0, 1.0).powf(1.0 / 2.2) * 255.0) as u8;
    Color32::from_rgb(s(c[0]), s(c[1]), s(c[2]))
}

/// The icon for each effect.
pub(super) fn effect_icon(e: Effect) -> Icon {
    match e {
        Effect::Pulse => Icon::Pulse,
        Effect::Pop => Icon::Burst,
        Effect::Split => Icon::Split,
        Effect::Wave => Icon::Wave,
        Effect::Chase => Icon::Chase,
        Effect::Breathe => Icon::Breathe,
        Effect::Riser => Icon::Riser,
    }
}

/// The icon for each role a light plays in the show.
fn role_icon(r: Role) -> Icon {
    match r {
        Role::Follow => Icon::Pulse,
        Role::Wash => Icon::Breathe,
        Role::Chase => Icon::Chase,
        Role::Off => Icon::Moon,
    }
}

fn role_about(r: Role) -> &'static str {
    match r {
        Role::Follow => "The show's colours, a soft bump on every kick, the vibe's brightness",
        Role::Wash => "The colours only, slowly: a calm wash, no bumps",
        Role::Chase => "Takes turns with the other chase lights on the beat",
        Role::Off => "Left dark by the show (tests still light it)",
    }
}

/// The widest a pad gets, so pads match across the page.
const PAD_MAX: f32 = 104.0;

/// The inspector's width for a page `width` wide.
fn inspector_width(width: f32) -> f32 {
    (width * 0.3).clamp(300.0, 380.0)
}

pub fn draw(ui: &mut Ui, area: Rect, settings: &mut Settings, live: &Live, state: &mut LightsState, actions: &mut Vec<UiAction>) {
    let l = &mut settings.lighting;
    if !l.fixtures.is_empty() {
        state.selected = state.selected.min(l.fixtures.len() - 1);
    }
    let panel_w = inspector_width(area.width());
    let left = Rect::from_min_max(area.min, pos2(area.right() - panel_w - style::GAP, area.bottom()));
    let panel = Rect::from_min_max(pos2(left.right() + style::GAP, area.top()), area.max);

    // The left column scrolls as one, and everything in it takes its width
    // from the column: nothing reaches under the inspector.
    ui.scope_builder(egui::UiBuilder::new().max_rect(left), |ui| {
        egui::ScrollArea::vertical().id_salt("lights left").auto_shrink([false, false]).show(ui, |ui| {
            let w = left.width();
            ui.set_width(w);
            card(ui, w, |ui, inner| master(ui, inner, l, live));
            ui.add_space(style::GAP);
            card(ui, w, |ui, inner| effects(ui, inner, l, live));
            ui.add_space(style::GAP);
            card(ui, w, |ui, inner| rig(ui, inner, l, live, state));
            ui.add_space(style::GAP);
            card(ui, w, |ui, inner| channel_finder(ui, inner, state));
        });
    });
    ui.scope_builder(egui::UiBuilder::new().max_rect(panel), |ui| {
        ui.painter().rect(panel, CornerRadius::same(style::CARD_RADIUS), style::CARD, Stroke::new(1.0, style::LINE), egui::StrokeKind::Inside);
        ui.scope_builder(egui::UiBuilder::new().max_rect(panel.shrink(16.0)), |ui| {
            egui::ScrollArea::vertical().id_salt("lights inspector").auto_shrink([false, false]).show(ui, |ui| {
                ui.set_width(panel.width() - 32.0);
                match l.fixtures.get(state.selected) {
                    None => empty_inspector(ui),
                    Some(f) if f.is_smoke() => smoke_inspector(ui, l, state, actions),
                    Some(_) => light_inspector(ui, l, live, state, actions),
                }
            });
        });
    });
    ui.advance_cursor_after_rect(area);
}

/// A card `width` wide; `add` gets the width inside it.
fn card(ui: &mut Ui, width: f32, add: impl FnOnce(&mut Ui, f32)) {
    style::card_frame().show(ui, |ui| {
        let inner = width - 30.0;
        ui.set_width(inner);
        add(ui, inner);
    });
}

/// The interface, and the controls over every light.
fn master(ui: &mut Ui, width: f32, l: &mut Lighting, live: &Live) {
    widgets::card_header(ui, "Interface", |ui| {
        let (dot, text) = match &live.dmx {
            Some(d) if d.sending => (style::GREEN, format!("{} · {} · {:.0} frames/s", d.kind.unwrap_or("DMX"), d.port.as_deref().map(crate::dmx::short_name).unwrap_or(""), d.rate)),
            Some(d) => (style::RED, d.error.clone().unwrap_or_else(|| "Looking for the interface…".into())),
            None => (style::MUTED, "DMX is not running".into()),
        };
        style::dot(ui, dot);
        ui.label(style::semibold(text, 13.0));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let ports = &live.dmx_ports;
            let shown = l.port.as_deref().map(crate::dmx::short_name).unwrap_or("Automatic").to_string();
            egui::ComboBox::from_id_salt("dmx port").selected_text(shown).width(170.0).show_ui(ui, |ui| {
                ui.selectable_value(&mut l.port, None, "Automatic (the first found)");
                for p in ports {
                    ui.selectable_value(&mut l.port, Some(p.clone()), crate::dmx::short_name(p));
                }
            });
            let (r, _) = ui.allocate_exact_size(vec2(16.0, 16.0), Sense::hover());
            icons::paint(ui, r, Icon::Plug, style::MUTED);
        });
    });
    // Three switches as pads, then the two amounts.
    let gap = 8.0;
    let pad_w = 86.0_f32.min(PAD_MAX);
    let sliders_w = width - 3.0 * pad_w - 3.0 * gap;
    let wide = sliders_w >= 220.0;
    ui.horizontal_top(|ui| {
        ui.spacing_mut().item_spacing = vec2(gap, gap);
        let dark = l.blackout;
        if widgets::pad_coloured(ui, vec2(pad_w, 74.0), Icon::Moon, "Lights out", if dark { "on" } else { "" }, dark, style::RED.gamma_multiply(0.75), "Every light dark at once; the screens go on").clicked() {
            l.blackout = !dark;
        }
        if widgets::pad(ui, vec2(pad_w, 74.0), Icon::Pulse, "Follow vibe", "", l.follow_vibe, "Brightness follows the music's vibe: darker for dark music").clicked() {
            l.follow_vibe = !l.follow_vibe;
        }
        if widgets::pad(ui, vec2(pad_w, 74.0), Icon::Bolt, "Flashes", "", l.flashes, "White on the show's strobe flashes, never more than three a second").clicked() {
            l.flashes = !l.flashes;
        }
        if wide {
            ui.vertical(|ui| {
                ui.set_width(sliders_w);
                ui.add_space(6.0);
                master_sliders(ui, l);
            });
        }
    });
    if !wide {
        ui.add_space(8.0);
        master_sliders(ui, l);
    }
}

fn master_sliders(ui: &mut Ui, l: &mut Lighting) {
    let pct = format!("{:.0}%", l.master * 100.0);
    widgets::slider_row(ui, Icon::Sun, "Master", &mut l.master, 0.0..=1.0, &pct, "How bright all the lights are, at most");
    ui.add_space(4.0);
    let fade = format!("{:.2} s", l.fade_secs);
    widgets::slider_row(ui, Icon::Clock, "Fade", &mut l.fade_secs, 0.05..=5.0, &fade, "How long a colour change takes");
}

/// The effect the lights play: Auto (follows the show) or one locked; how
/// hard they play; their strobe.
fn effects(ui: &mut Ui, width: f32, l: &mut Lighting, live: &Live) {
    let playing = live.light_effect.map(|e| e.name()).unwrap_or("—");
    widgets::card_header(ui, "Effect", |ui| {
        ui.label(style::muted(if l.effect.is_none() { format!("Auto · playing {playing}") } else { format!("Locked on {playing}") }));
    });
    // Eight pads: one row when they fit at 72 px or more, else two of four.
    let gap = 8.0;
    let per_row = if widgets::pad_width(width, 8, gap, 0.0) >= 72.0 { 8 } else { 4 };
    // Pads keep one size across the page (the master's are 86 wide).
    let w = widgets::pad_width(width, per_row, gap, 56.0).min(PAD_MAX);
    let mut items: Vec<(Option<Effect>, Icon, &str, &str)> = vec![(None, Icon::Sparkle, "Auto", "Follows the show: the section and the vibe choose the effect, changing on phrases")];
    for e in Effect::ALL {
        items.push((Some(e), effect_icon(e), e.name(), e.about()));
    }
    for row in items.chunks(per_row) {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = gap;
            for (effect, icon, name, about) in row {
                let on = l.effect == *effect;
                let is_playing = effect.is_some() && live.light_effect == *effect;
                let hint = if effect.is_none() && on { "on" } else if is_playing { "playing" } else { "" };
                let r = widgets::pad(ui, vec2(w, 74.0), *icon, name, hint, on, about);
                if is_playing && !on {
                    ui.painter().circle_filled(r.rect.right_top() + vec2(-9.0, 9.0), 3.5, style::CYAN);
                }
                if r.clicked() {
                    l.effect = if on && effect.is_some() { None } else { *effect };
                }
            }
        });
        ui.add_space(gap);
    }
    ui.add_space(2.0);
    let word = match (l.drive * 4.0).round() as i32 {
        0 => "Subtle",
        1 => "Soft",
        2 => "Normal",
        3 => "Strong",
        _ => "Wild",
    };
    widgets::slider_row(ui, Icon::Gauge, "Drive", &mut l.drive, 0.0..=1.0, word, "How hard the lights play, from subtle to wild: brightness, the dips between hits, speed, fades, how vivid the colours");
    ui.add_space(8.0);
    ui.horizontal(|ui| {
        let (r, _) = ui.allocate_exact_size(vec2(18.0, 22.0), Sense::hover());
        icons::paint(ui, r.shrink2(vec2(1.0, 3.0)), Icon::Bolt, style::MUTED);
        ui.label(RichText::new("Strobe").size(13.0));
        let seg_w = ui.available_width().min(360.0);
        widgets::icon_segmented(
            ui,
            &mut l.strobe,
            &[(LightStrobe::Off, Icon::BoltOff, "Off"), (LightStrobe::Drops, Icon::Bolt, "On drops"), (LightStrobe::Free, Icon::Bolts, "Drops, often")],
            seg_w,
            |s| match s {
                LightStrobe::Off => "The lights never strobe".into(),
                LightStrobe::Drops => "White hits on drops, at most three a second".into(),
                LightStrobe::Free => "Through drops on every beat or two, at most three a second".into(),
            },
        );
    });
}

/// The rig: one card per light, with its live colour; add a light or a
/// smoke machine from the card's header.
fn rig(ui: &mut Ui, width: f32, l: &mut Lighting, live: &Live, state: &mut LightsState) {
    let room = l.fixtures.len() < MAX_FIXTURES;
    let has_smoke = l.fixtures.iter().any(|f| f.is_smoke());
    widgets::card_header(ui, &format!("Lights · {}", l.fixtures.len()), |ui| {
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if widgets::icon_text_button(ui, Icon::Smoke, "Smoke machine", room && !has_smoke, "Add a smoke machine (one DMX channel)").clicked() {
                let n = l.fixtures.len();
                l.fixtures.push(Fixture { name: "Smoke".into(), address: l.next_address(), profile: Profile::Smoke, ..Fixture::default() });
                state.selected = n;
                if l.smoke == SmokeMode::Off {
                    l.smoke = SmokeMode::Hand;
                }
            }
            if widgets::icon_text_button(ui, Icon::Add, "Light", room, "Add a light at the next free address").clicked() {
                let n = l.fixtures.len();
                l.fixtures.push(Fixture { name: format!("Spot {}", n + 1), address: l.next_address(), ..Fixture::default() });
                state.selected = n;
            }
        });
    });
    if l.fixtures.is_empty() {
        ui.label(style::muted("No lights yet. Add one with + Light, set its DMX address as on its own display, and test it."));
        return;
    }
    let overlaps = l.overlaps();
    let gap = 10.0;
    let per_row = ((width + gap) / (170.0 + gap)).floor().max(1.0) as usize;
    let w = widgets::pad_width(width, per_row, gap, 150.0);
    for (row_start, row) in l.fixtures.chunks(per_row).enumerate().map(|(k, r)| (k * per_row, r)) {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = gap;
            for (k, f) in row.iter().enumerate() {
                let i = row_start + k;
                let clash = overlaps.iter().any(|(a, b)| *a == i || *b == i);
                if light_card(ui, vec2(w, 96.0), f, live.lights.get(i).copied().unwrap_or([0.0; 3]), i == state.selected, clash).clicked() {
                    state.selected = i;
                }
            }
        });
        ui.add_space(gap);
    }
}

/// One light: its colour now as a glowing disc, its name and channels, and
/// what it does in the show.
fn light_card(ui: &mut Ui, size: egui::Vec2, f: &Fixture, seen: [f32; 3], selected: bool, clash: bool) -> egui::Response {
    let (rect, r) = ui.allocate_exact_size(size, Sense::click());
    let hovered = r.hovered();
    let p = ui.painter();
    p.rect(
        rect,
        CornerRadius::same(12),
        if hovered && !selected { style::LINE } else { style::CARD_LIGHT },
        Stroke::new(if selected { 2.0 } else { 1.0 }, if selected { style::ACCENT } else { style::LINE }),
        egui::StrokeKind::Inside,
    );
    let c = rect.left_top() + vec2(32.0, 34.0);
    let colour = rgb(seen);
    for k in (1..=5).rev() {
        p.circle_filled(c, 13.0 + k as f32 * 3.0, colour.gamma_multiply(0.07));
    }
    p.circle(c, 15.0, colour, Stroke::new(1.5, Color32::from_white_alpha(60)));
    if f.is_smoke() {
        icons::paint(ui, Rect::from_center_size(c, vec2(16.0, 16.0)), Icon::Smoke, style::TEXT);
    }
    let p = ui.painter();
    let x = rect.left() + 58.0;
    p.text(pos2(x, rect.top() + 24.0), egui::Align2::LEFT_CENTER, style::visual(&f.name), egui::FontId::new(14.0, egui::FontFamily::Name("semibold".into())), style::TEXT);
    let span = f.span();
    let channels = if span.start() == span.end() { format!("DMX {}", span.start()) } else { format!("DMX {}–{}", span.start(), span.end()) };
    p.text(pos2(x, rect.top() + 43.0), egui::Align2::LEFT_CENTER, channels, egui::FontId::proportional(11.5), if clash { style::AMBER } else { style::MUTED });
    // What it does: an icon and a word along the bottom.
    let (icon, word, tint) = if f.is_smoke() {
        (Icon::Smoke, "Smoke", style::MUTED)
    } else if !f.enabled {
        (Icon::Moon, "Off", style::TERTIARY)
    } else {
        (role_icon(f.role), f.role.name(), style::CYAN)
    };
    let base = rect.left_bottom() + vec2(16.0, -20.0);
    icons::paint(ui, Rect::from_center_size(base + vec2(6.0, 0.0), vec2(13.0, 13.0)), icon, tint);
    ui.painter().text(base + vec2(18.0, 0.0), egui::Align2::LEFT_CENTER, word, egui::FontId::new(12.0, egui::FontFamily::Name("semibold".into())), tint);
    if clash {
        icons::paint(ui, Rect::from_center_size(rect.right_top() + vec2(-18.0, 18.0), vec2(14.0, 14.0)), Icon::Warning, style::AMBER);
    }
    r.on_hover_text(if clash { "Shares DMX channels with another light: check the addresses" } else { "Select to set it up and test it" })
}

fn empty_inspector(ui: &mut Ui) {
    ui.add_space(30.0);
    ui.vertical_centered(|ui| {
        let (r, _) = ui.allocate_exact_size(vec2(34.0, 34.0), Sense::hover());
        icons::paint(ui, r, Icon::Spot, style::TERTIARY);
        ui.add_space(8.0);
        ui.label(style::muted("Add a light to set it up here."));
    });
}

/// The inspector's top: the light's colour, its name, and on or off.
fn inspector_head(ui: &mut Ui, f: &mut Fixture, colour: Color32) {
    ui.horizontal(|ui| {
        let (r, _) = ui.allocate_exact_size(vec2(34.0, 34.0), Sense::hover());
        ui.painter().circle(r.center(), 14.0, colour, Stroke::new(1.5, Color32::from_white_alpha(60)));
        if f.is_smoke() {
            icons::paint(ui, r.shrink(10.0), Icon::Smoke, style::TEXT);
        }
        ui.add(egui::TextEdit::singleline(&mut f.name).font(egui::FontId::new(15.0, egui::FontFamily::Name("semibold".into()))).desired_width(ui.available_width() - 50.0));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            style::toggle(ui, &mut f.enabled).on_hover_text("On: the show plays this light");
        });
    });
    ui.add_space(10.0);
}

/// Where the light is: its DMX address (with − and +) and its channels.
fn address_rows(ui: &mut Ui, f: &mut Fixture, others: &[(String, std::ops::RangeInclusive<u16>)]) {
    style::row(ui, "DMX address", |ui| {
        if ui.small_button("−").clicked() {
            f.address = f.address.saturating_sub(1).max(1);
        }
        ui.add(egui::DragValue::new(&mut f.address).range(1..=UNIVERSE as u16).speed(0.2)).on_hover_text("As set on the light's own display (A001, d001…)");
        if ui.small_button("+").clicked() {
            f.address = (f.address + 1).min(UNIVERSE as u16);
        }
    });
    if !f.is_smoke() {
        style::row(ui, "Channels", |ui| {
            egui::ComboBox::from_id_salt("profile").selected_text(f.profile.name()).width(ui.available_width()).show_ui(ui, |ui| {
                for p in Profile::ALL.into_iter().filter(|p| *p != Profile::Smoke) {
                    if ui.selectable_value(&mut f.profile, p, p.name()).clicked() && p == Profile::Custom && f.custom.is_empty() {
                        f.custom = Profile::Spot7.channels();
                    }
                }
            });
        });
    }
    let span = f.span();
    if let Some((name, _)) = others.iter().find(|(_, o)| o.start() <= span.end() && span.start() <= o.end()) {
        ui.horizontal(|ui| {
            let (r, _) = ui.allocate_exact_size(vec2(16.0, 16.0), Sense::hover());
            icons::paint(ui, r, Icon::Warning, style::AMBER);
            ui.label(RichText::new(format!("Shares channels with {}", style::visual(name))).color(style::AMBER).size(12.5));
        });
    }
}

fn light_inspector(ui: &mut Ui, l: &mut Lighting, live: &Live, state: &mut LightsState, actions: &mut Vec<UiAction>) {
    let i = state.selected;
    let others: Vec<(String, std::ops::RangeInclusive<u16>)> = l.fixtures.iter().enumerate().filter(|(j, _)| *j != i).map(|(_, f)| (f.name.clone(), f.span())).collect();
    let seen = rgb(live.lights.get(i).copied().unwrap_or([0.0; 3]));
    let width = ui.available_width();
    let f = &mut l.fixtures[i];
    inspector_head(ui, f, seen);
    address_rows(ui, f, &others);
    ui.add_space(6.0);
    let pct = format!("{:.0}%", f.brightness * 100.0);
    widgets::slider_row(ui, Icon::Sun, "Brightness", &mut f.brightness, 0.0..=1.0, &pct, "This light's brightness, under the master");
    if f.profile == Profile::Custom {
        ui.add_space(10.0);
        style::label(ui, "Channels, from its address");
        let mut remove = None;
        for (k, ch) in f.custom.iter_mut().enumerate() {
            ui.horizontal(|ui| {
                ui.label(style::muted(format!("{:>3}", f.address as usize + k)));
                egui::ComboBox::from_id_salt(("custom", k)).selected_text(ch.name()).width(ui.available_width() - 34.0).show_ui(ui, |ui| {
                    for c in Channel::ALL {
                        ui.selectable_value(ch, c, c.name());
                    }
                });
                if icons::small(ui, Icon::Delete, "Remove this channel", style::MUTED).clicked() {
                    remove = Some(k);
                }
            });
        }
        if let Some(k) = remove {
            f.custom.remove(k);
        }
        if f.custom.len() < 32 && widgets::icon_text_button(ui, Icon::Add, "Channel", true, "Add a channel after the last").clicked() {
            f.custom.push(Channel::Zero);
        }
    }
    ui.add_space(14.0);
    style::label(ui, "In the show");
    widgets::icon_segmented(ui, &mut f.role, &Role::ALL.map(|r| (r, role_icon(r), r.name())), width, |r| role_about(r).to_string());
    ui.add_space(16.0);
    style::label(ui, "Test");
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 6.0;
        for (name, c) in [("Red", [1.0, 0.0, 0.0]), ("Green", [0.0, 1.0, 0.0]), ("Blue", [0.0, 0.0, 1.0]), ("White", [1.0, 1.0, 1.0])] {
            let on = state.test == LightTest::Colour(i, c);
            if widgets::swatch(ui, rgb(c), on, &format!("{name}: this light in {}, at full, until pressed again", name.to_lowercase())).clicked() {
                state.test = if on { LightTest::None } else { LightTest::Colour(i, c) };
            }
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if widgets::icon_text_button(ui, Icon::Target, "Identify", true, "Flashes it white for two seconds").clicked() {
                actions.push(UiAction::LightIdentify(i));
            }
        });
    });
    if matches!(state.test, LightTest::Colour(..)) {
        ui.label(RichText::new("Testing: the show is off this light until you press the colour again.").color(style::AMBER).size(12.5));
    }
    remove_button(ui, l, state);
}

fn smoke_inspector(ui: &mut Ui, l: &mut Lighting, state: &mut LightsState, actions: &mut Vec<UiAction>) {
    let _ = actions;
    let i = state.selected;
    let others: Vec<(String, std::ops::RangeInclusive<u16>)> = l.fixtures.iter().enumerate().filter(|(j, _)| *j != i).map(|(_, f)| (f.name.clone(), f.span())).collect();
    let width = ui.available_width();
    {
        let f = &mut l.fixtures[i];
        inspector_head(ui, f, style::CARD_LIGHT);
        address_rows(ui, f, &others);
    }
    ui.add_space(14.0);
    style::label(ui, "Smoke");
    widgets::icon_segmented(ui, &mut l.smoke, &[(SmokeMode::Off, Icon::BoltOff, "Off"), (SmokeMode::Hand, Icon::Hold, "By hand"), (SmokeMode::Auto, Icon::Sparkle, "Auto")], width, |m| match m {
        SmokeMode::Off => "No smoke, even when the button is held".into(),
        SmokeMode::Hand => "Smoke only while the button is held".into(),
        SmokeMode::Auto => "A burst on each drop and at the top of a build, never more often than set".into(),
    });
    ui.add_space(10.0);
    let (rect, r) = ui.allocate_exact_size(vec2(width, 64.0), Sense::click_and_drag());
    state.smoke_held = r.is_pointer_button_down_on() && l.smoke != SmokeMode::Off;
    let held = state.smoke_held;
    let fill = if held { style::ACCENT } else if r.hovered() { style::LINE } else { style::CARD_LIGHT };
    ui.painter().rect(rect, CornerRadius::same(10), fill, Stroke::new(1.0, if held { style::ACCENT } else { style::LINE }), egui::StrokeKind::Inside);
    icons::paint(ui, Rect::from_center_size(rect.center() - vec2(0.0, 10.0), vec2(20.0, 20.0)), Icon::Smoke, if held { Color32::WHITE } else { style::TEXT });
    ui.painter().text(rect.center() + vec2(0.0, 14.0), egui::Align2::CENTER_CENTER, if held { "Smoke…" } else { "Hold for smoke" }, egui::FontId::new(13.0, egui::FontFamily::Name("semibold".into())), if held { Color32::WHITE } else { style::TEXT });
    ui.add_space(12.0);
    let pct = format!("{:.0}%", l.smoke_amount * 100.0);
    widgets::slider_row(ui, Icon::Smoke, "Amount", &mut l.smoke_amount, 0.0..=1.0, &pct, "How hard the machine pushes smoke");
    if l.smoke == SmokeMode::Auto {
        ui.add_space(6.0);
        style::row(ui, "Burst", |ui| {
            ui.add(egui::DragValue::new(&mut l.smoke_secs).range(0.5..=15.0).speed(0.1).suffix(" s")).on_hover_text("How long each burst lasts");
        });
        style::row(ui, "At most every", |ui| {
            ui.add(egui::DragValue::new(&mut l.smoke_gap_secs).range(10.0..=600.0).speed(1.0).suffix(" s")).on_hover_text("Never two bursts closer than this");
        });
    }
    remove_button(ui, l, state);
}

/// Removes the selected light, at the inspector's foot.
fn remove_button(ui: &mut Ui, l: &mut Lighting, state: &mut LightsState) {
    ui.add_space(22.0);
    style::separator(ui);
    ui.add_space(10.0);
    let i = state.selected;
    let what = if l.fixtures[i].is_smoke() { "Remove the smoke machine" } else { "Remove this light" };
    let r = widgets::icon_text_button(ui, Icon::Delete, what, true, "Take it out of the rig (the light itself is untouched)");
    if r.clicked() {
        l.fixtures.remove(i);
        state.selected = i.saturating_sub(1);
        state.test = LightTest::None;
    }
}

/// Lights one channel of the universe alone: to learn what a light's
/// channels do, or find its address. Folded away until it is needed.
fn channel_finder(ui: &mut Ui, width: f32, state: &mut LightsState) {
    let on = matches!(state.test, LightTest::Channel(..));
    let (rect, r) = ui.allocate_exact_size(vec2(width, 26.0), Sense::click());
    icons::paint(ui, Rect::from_center_size(rect.left_center() + vec2(9.0, 0.0), vec2(16.0, 16.0)), Icon::Search, style::MUTED);
    ui.painter().text(rect.left_center() + vec2(26.0, 0.0), egui::Align2::LEFT_CENTER, "Find a light's channels", egui::FontId::new(13.5, egui::FontFamily::Name("semibold".into())), style::TEXT);
    ui.painter().text(rect.right_center(), egui::Align2::RIGHT_CENTER, if state.finder { "Hide" } else { "Show" }, egui::FontId::proportional(12.5), style::ACCENT);
    if r.on_hover_text("For a light whose manual is lost").clicked() {
        state.finder = !state.finder;
    }
    if !state.finder && !on {
        return;
    }
    ui.add_space(6.0);
    ui.label(style::muted(
        "Lights one DMX channel alone, every other at 0. Step through 1, 2, 3… and watch the light to learn which channel is its dimmer, red, green, blue. A light that ignores every channel is in another mode on its own display (choose its DMX mode, like d001 or A001).",
    ));
    ui.add_space(8.0);
    ui.horizontal_wrapped(|ui| {
        ui.label("Channel");
        if ui.small_button("−").clicked() {
            state.channel = state.channel.saturating_sub(1).max(1);
        }
        ui.add(egui::DragValue::new(&mut state.channel).range(1..=UNIVERSE as u16));
        if ui.small_button("+").clicked() {
            state.channel = (state.channel + 1).min(UNIVERSE as u16);
        }
        ui.add_space(10.0);
        ui.label("Value");
        ui.spacing_mut().slider_width = 140.0;
        ui.add(egui::Slider::new(&mut state.value, 0..=255));
        ui.add_space(10.0);
        ui.label("Also hold at full").on_hover_text("Colours often stay dark while the dimmer is at 0: hold the dimmer channel open (usually the light's first)");
        ui.add(egui::DragValue::new(&mut state.hold).range(0..=UNIVERSE as u16).custom_formatter(|v, _| if v == 0.0 { "none".into() } else { format!("{v}") }));
    });
    ui.add_space(8.0);
    ui.horizontal(|ui| {
        let r = widgets::icon_text_button(ui, if on { Icon::Stop } else { Icon::Play }, if on { "Stop" } else { "Light it" }, true, "Light this channel alone");
        if r.clicked() {
            state.test = if on { LightTest::None } else { LightTest::Channel(state.channel, state.value, state.hold) };
        }
        ui.label(style::muted("On some lights a high value on the strobe channel flashes fast."));
    });
    if let LightTest::Channel(..) = state.test {
        state.test = LightTest::Channel(state.channel, state.value, state.hold);
        let hold = if state.hold > 0 { format!(", channel {} at 255", state.hold) } else { String::new() };
        ui.label(RichText::new(format!("Channel {} at {}{hold} · the show is off the lights while this runs", state.channel, state.value)).color(style::AMBER).size(12.5));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_effect_and_role_has_its_own_icon() {
        let mut icons: Vec<Icon> = Effect::ALL.iter().map(|e| effect_icon(*e)).collect();
        icons.push(Icon::Sparkle);
        for (k, a) in icons.iter().enumerate() {
            assert!(!icons[k + 1..].contains(a), "{a:?} twice among the effects");
        }
        let roles: Vec<Icon> = Role::ALL.iter().map(|r| role_icon(*r)).collect();
        for (k, a) in roles.iter().enumerate() {
            assert!(!roles[k + 1..].contains(a), "{a:?} twice among the roles");
        }
    }

    /// The inspector never takes the page: the left column always keeps
    /// room for the effect pads.
    #[test]
    fn the_inspector_leaves_the_page_its_room() {
        for width in [700.0, 1000.0, 1400.0, 2400.0] {
            let left = width - inspector_width(width) - style::GAP;
            assert!(left >= 380.0, "{width}: {left}");
        }
    }
}
