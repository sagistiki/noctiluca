//! Mapping → Lights: the DMX interface, the lights as cards with their live
//! colour, the selected light's settings, tests, and smoke.

use egui::{Color32, CornerRadius, Rect, RichText, Sense, Stroke, Ui, pos2, vec2};

use super::style;
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
}

impl Default for LightsState {
    fn default() -> Self {
        LightsState { selected: 0, test: LightTest::None, smoke_held: false, channel: 1, value: 255, hold: 0 }
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

fn rgb(c: [f32; 3]) -> Color32 {
    // Linear light to screen colour.
    let s = |v: f32| (v.clamp(0.0, 1.0).powf(1.0 / 2.2) * 255.0) as u8;
    Color32::from_rgb(s(c[0]), s(c[1]), s(c[2]))
}

pub fn draw(ui: &mut Ui, area: Rect, settings: &mut Settings, live: &Live, state: &mut LightsState, actions: &mut Vec<UiAction>) {
    let l = &mut settings.lighting;
    if !l.fixtures.is_empty() {
        state.selected = state.selected.min(l.fixtures.len() - 1);
    }
    let panel_w = (area.width() * 0.32).clamp(320.0, 440.0);
    let left = Rect::from_min_max(area.min, pos2(area.right() - panel_w - style::GAP, area.bottom()));
    let panel = Rect::from_min_max(pos2(left.right() + style::GAP, area.top()), area.max);

    ui.scope_builder(egui::UiBuilder::new().max_rect(left), |ui| {
        interface_bar(ui, l, live, actions);
        ui.add_space(10.0);
        effects(ui, l, live);
        ui.add_space(10.0);
        let rest = ui.available_rect_before_wrap();
        ui.painter().rect(rest, CornerRadius::same(style::CARD_RADIUS), style::CARD, Stroke::new(1.0, style::LINE), egui::StrokeKind::Inside);
        ui.scope_builder(egui::UiBuilder::new().max_rect(rest.shrink(16.0)), |ui| {
            egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                style::label(ui, "Lights");
                cards(ui, l, live, state);
                ui.add_space(12.0);
                ui.horizontal_wrapped(|ui| {
                    let room = l.fixtures.len() < MAX_FIXTURES;
                    if ui.add_enabled(room, egui::Button::new(style::semibold("+ Add light", 14.0))).clicked() {
                        let n = l.fixtures.len();
                        l.fixtures.push(Fixture { name: format!("Spot {}", n + 1), address: l.next_address(), ..Fixture::default() });
                        state.selected = n;
                    }
                    if ui.add_enabled(room, egui::Button::new(style::semibold("+ Smoke machine", 14.0))).clicked() {
                        let n = l.fixtures.len();
                        l.fixtures.push(Fixture { name: "Smoke".into(), address: l.next_address(), profile: Profile::Smoke, ..Fixture::default() });
                        state.selected = n;
                        if l.smoke == SmokeMode::Off {
                            l.smoke = SmokeMode::Hand;
                        }
                    }
                });
                ui.add_space(16.0);
                channel_finder(ui, state);
            });
        });
    });
    ui.scope_builder(egui::UiBuilder::new().max_rect(panel), |ui| {
        ui.painter().rect(panel, CornerRadius::same(style::CARD_RADIUS), style::CARD, Stroke::new(1.0, style::LINE), egui::StrokeKind::Inside);
        ui.scope_builder(egui::UiBuilder::new().max_rect(panel.shrink(16.0)), |ui| {
            egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                fixture_panel(ui, l, state, actions);
                ui.add_space(14.0);
                smoke_panel(ui, l, state);
            });
        });
    });
    ui.advance_cursor_after_rect(area);
}

/// The interface, and the controls over all the lights.
fn interface_bar(ui: &mut Ui, l: &mut Lighting, live: &Live, actions: &mut Vec<UiAction>) {
    let _ = actions;
    style::card_frame().show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.horizontal(|ui| {
            let (dot, text) = match &live.dmx {
                Some(d) if d.sending => (style::GREEN, format!("{} · {} · {:.0} frames/s", d.kind.unwrap_or("DMX"), d.port.as_deref().map(crate::dmx::short_name).unwrap_or(""), d.rate)),
                Some(d) => (style::RED, d.error.clone().unwrap_or_else(|| "Looking for the interface…".into())),
                None => (style::MUTED, "DMX is not running".into()),
            };
            let (r, _) = ui.allocate_exact_size(vec2(12.0, 12.0), Sense::hover());
            ui.painter().circle_filled(r.center(), 5.0, dot);
            ui.label(style::semibold(text, 14.0));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let ports = &live.dmx_ports;
                let shown = l.port.as_deref().map(crate::dmx::short_name).unwrap_or("Automatic").to_string();
                egui::ComboBox::from_id_salt("dmx port").selected_text(shown).width(200.0).show_ui(ui, |ui| {
                    ui.selectable_value(&mut l.port, None, "Automatic (the first found)");
                    for p in ports {
                        ui.selectable_value(&mut l.port, Some(p.clone()), crate::dmx::short_name(p));
                    }
                });
                ui.label(style::muted("Interface"));
            });
        });
        ui.add_space(6.0);
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing.x = 14.0;
            let black = l.blackout;
            let b = egui::Button::new(style::semibold(if black { "Blackout ON" } else { "Blackout" }, 14.0))
                .fill(if black { style::RED.gamma_multiply(0.5) } else { style::CARD_LIGHT });
            if ui.add(b).on_hover_text("Every light dark at once").clicked() {
                l.blackout = !black;
            }
            ui.label("Master");
            ui.spacing_mut().slider_width = 140.0;
            ui.add(egui::Slider::new(&mut l.master, 0.0..=1.0).show_value(false));
            ui.label("Follow the vibe");
            style::toggle(ui, &mut l.follow_vibe);
            ui.label("Flashes").on_hover_text("White on the show's strobe flashes, never more than three a second");
            style::toggle(ui, &mut l.flashes);
            ui.label("Fade");
            ui.add(egui::DragValue::new(&mut l.fade_secs).speed(0.02).range(0.05..=5.0).suffix(" s").fixed_decimals(2));
        });
    });
}

/// The effect the lights play: Auto (follows the show) or one locked; how
/// hard it hits; the lights' own strobe.
fn effects(ui: &mut Ui, l: &mut Lighting, live: &Live) {
    style::card_frame().show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.horizontal(|ui| {
            style::label(ui, "Effect");
            let playing = live.light_effect.map(|e| e.name()).unwrap_or("");
            ui.label(style::muted(if l.effect.is_none() { format!("Auto · playing {playing}") } else { format!("Locked · {playing}") }));
        });
        ui.add_space(4.0);
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = vec2(8.0, 8.0);
            let tile = |ui: &mut Ui, name: &str, about: &str, on: bool, playing: bool| {
                let (rect, r) = ui.allocate_exact_size(vec2(104.0, 46.0), Sense::click());
                let fill = if on { style::PINK.gamma_multiply(0.25) } else if r.hovered() { Color32::from_rgb(0x2a, 0x2a, 0x38) } else { style::CARD_LIGHT };
                let stroke = if on { style::PINK } else if playing { style::CYAN.gamma_multiply(0.8) } else { style::LINE };
                ui.painter().rect(rect, CornerRadius::same(10), fill, Stroke::new(if on || playing { 1.5 } else { 1.0 }, stroke), egui::StrokeKind::Inside);
                ui.painter().text(rect.center(), egui::Align2::CENTER_CENTER, name, egui::FontId::new(14.0, egui::FontFamily::Name("semibold".into())), if on { style::TEXT } else { style::MUTED });
                if playing {
                    ui.painter().circle_filled(rect.right_top() + vec2(-9.0, 9.0), 3.5, style::CYAN);
                }
                r.on_hover_text(about).clicked()
            };
            if tile(ui, "Auto", "Follows the show: the section and the vibe choose the effect, changing on phrases", l.effect.is_none(), false) {
                l.effect = None;
            }
            for e in Effect::ALL {
                let playing = live.light_effect == Some(e);
                if tile(ui, e.name(), e.about(), l.effect == Some(e), playing) {
                    l.effect = if l.effect == Some(e) { None } else { Some(e) };
                }
            }
        });
        ui.add_space(8.0);
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing.x = 14.0;
            ui.label("Drive").on_hover_text("One control for how hard the lights play: brightness, the bumps and chases, how vivid the colours. The lights follow it smoothly.");
            ui.label(style::muted("Subtle"));
            ui.spacing_mut().slider_width = 220.0;
            ui.add(egui::Slider::new(&mut l.drive, 0.0..=1.0).step_by(0.01).show_value(false));
            ui.label(style::muted("Wild"));
            let word = match (l.drive * 4.0).round() as i32 {
                0 => "Subtle",
                1 => "Soft",
                2 => "Normal",
                3 => "Strong",
                _ => "Wild",
            };
            ui.label(style::semibold(word, 14.0).color(style::PINK));
            ui.label("Strobe").on_hover_text("White hits from the lights: on drops, or also at the top of builds. Never more than three a second.");
            style::segmented(ui, &mut l.strobe, &[(LightStrobe::Off, "Off"), (LightStrobe::Drops, "On drops"), (LightStrobe::Free, "Drops + builds")]);
        });
    });
}

/// One card per light: its live colour, name, address and role.
fn cards(ui: &mut Ui, l: &mut Lighting, live: &Live, state: &mut LightsState) {
    let overlaps = l.overlaps();
    let w = 190.0;
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = vec2(12.0, 12.0);
        for (i, f) in l.fixtures.iter().enumerate() {
            let (rect, r) = ui.allocate_exact_size(vec2(w, 132.0), Sense::click());
            let on = i == state.selected;
            let p = ui.painter();
            p.rect(
                rect,
                CornerRadius::same(14),
                style::CARD_LIGHT,
                Stroke::new(if on { 2.0 } else { 1.0 }, if on { style::PINK } else { style::LINE }),
                egui::StrokeKind::Inside,
            );
            // The light as it is now: a glowing disc.
            let seen = live.lights.get(i).copied().unwrap_or([0.0; 3]);
            let c = rect.left_top() + vec2(36.0, 40.0);
            for k in (1..=6).rev() {
                p.circle_filled(c, 14.0 + k as f32 * 3.0, rgb(seen).gamma_multiply(0.06));
            }
            p.circle(c, 16.0, rgb(seen), Stroke::new(1.5, Color32::from_white_alpha(60)));
            if f.is_smoke() {
                p.text(c, egui::Align2::CENTER_CENTER, "≈", egui::FontId::proportional(18.0), style::TEXT);
            }
            p.text(rect.left_top() + vec2(66.0, 26.0), egui::Align2::LEFT_CENTER, style::visual(&f.name), egui::FontId::new(15.0, egui::FontFamily::Name("semibold".into())), style::TEXT);
            let span = f.span();
            p.text(rect.left_top() + vec2(66.0, 48.0), egui::Align2::LEFT_CENTER, format!("DMX {}–{}", span.start(), span.end()), egui::FontId::proportional(12.0), style::MUTED);
            p.text(rect.left_top() + vec2(16.0, 86.0), egui::Align2::LEFT_CENTER, f.profile.name(), egui::FontId::proportional(12.0), style::MUTED);
            let role = if f.is_smoke() { "Smoke" } else if !f.enabled { "Off" } else { f.role.name() };
            p.text(rect.left_top() + vec2(16.0, 108.0), egui::Align2::LEFT_CENTER, role, egui::FontId::new(13.0, egui::FontFamily::Name("semibold".into())), if f.enabled { style::CYAN } else { style::MUTED });
            if overlaps.iter().any(|(a, b)| *a == i || *b == i) {
                p.text(rect.right_top() + vec2(-14.0, 22.0), egui::Align2::RIGHT_CENTER, "overlaps", egui::FontId::proportional(11.0), style::AMBER);
            }
            if r.clicked() {
                state.selected = i;
            }
        }
    });
    if l.fixtures.is_empty() {
        ui.label(style::muted("No lights yet: add one with + Add light."));
    }
}

fn fixture_panel(ui: &mut Ui, l: &mut Lighting, state: &mut LightsState, actions: &mut Vec<UiAction>) {
    let Some(i) = (state.selected < l.fixtures.len()).then_some(state.selected) else {
        ui.label(style::muted("Choose a light."));
        return;
    };
    let others: Vec<(String, std::ops::RangeInclusive<u16>)> = l.fixtures.iter().enumerate().filter(|(j, _)| *j != i).map(|(_, f)| (f.name.clone(), f.span())).collect();
    let f = &mut l.fixtures[i];
    style::label(ui, &format!("Light {}", i + 1));
    ui.add(egui::TextEdit::singleline(&mut f.name).desired_width(ui.available_width()));
    ui.add_space(8.0);
    egui::Grid::new("fixture").num_columns(2).spacing(vec2(12.0, 10.0)).show(ui, |ui| {
        ui.label("On");
        style::toggle(ui, &mut f.enabled);
        ui.end_row();
        ui.label("DMX address").on_hover_text("As set on the light's own display (A001, d001…)");
        ui.add(egui::DragValue::new(&mut f.address).range(1..=UNIVERSE as u16).speed(0.2));
        ui.end_row();
        ui.label("Channels");
        egui::ComboBox::from_id_salt("profile").selected_text(f.profile.name()).width(ui.available_width()).show_ui(ui, |ui| {
            for p in Profile::ALL {
                if ui.selectable_value(&mut f.profile, p, p.name()).clicked() && p == Profile::Custom && f.custom.is_empty() {
                    f.custom = Profile::Spot7.channels();
                }
            }
        });
        ui.end_row();
        if !f.is_smoke() {
            ui.label("In the show");
            style::segmented(ui, &mut f.role, &Role::ALL.map(|r| (r, r.name())));
            ui.end_row();
            ui.label("Brightness");
            ui.add(egui::Slider::new(&mut f.brightness, 0.0..=1.0).show_value(false));
            ui.end_row();
        }
    });
    let span = f.span();
    if let Some((name, _)) = others.iter().find(|(_, o)| o.start() <= span.end() && span.start() <= o.end()) {
        ui.label(RichText::new(format!("Shares channels with {name}: check the addresses")).color(style::AMBER).size(13.0));
    }
    if f.profile == Profile::Custom {
        ui.add_space(8.0);
        style::label(ui, "Channels, from its address");
        let mut remove = None;
        for (k, ch) in f.custom.iter_mut().enumerate() {
            ui.horizontal(|ui| {
                ui.label(style::muted(format!("{:>3}", f.address as usize + k)));
                egui::ComboBox::from_id_salt(("custom", k)).selected_text(ch.name()).width(170.0).show_ui(ui, |ui| {
                    for c in Channel::ALL {
                        ui.selectable_value(ch, c, c.name());
                    }
                });
                if ui.small_button("×").clicked() {
                    remove = Some(k);
                }
            });
        }
        if let Some(k) = remove {
            f.custom.remove(k);
        }
        if f.custom.len() < 32 && ui.button("+ Channel").clicked() {
            f.custom.push(Channel::Zero);
        }
    }
    ui.add_space(12.0);
    style::label(ui, "Test this light");
    ui.horizontal_wrapped(|ui| {
        for (name, c) in [("Red", [1.0, 0.0, 0.0]), ("Green", [0.0, 1.0, 0.0]), ("Blue", [0.0, 0.0, 1.0]), ("White", [1.0, 1.0, 1.0])] {
            let on = state.test == LightTest::Colour(i, c);
            let b = egui::Button::new(RichText::new(name).color(if on { Color32::BLACK } else { style::TEXT })).fill(if on { rgb(c) } else { style::CARD_LIGHT });
            if ui.add(b).clicked() {
                state.test = if on { LightTest::None } else { LightTest::Colour(i, c) };
            }
        }
        if ui.button("Identify").on_hover_text("Flashes it white for two seconds").clicked() {
            actions.push(UiAction::LightIdentify(i));
        }
    });
    if matches!(state.test, LightTest::Colour(..)) {
        ui.label(style::muted("Testing: the show is paused on this light until you press the colour again."));
    }
    ui.add_space(14.0);
    if style::button(ui, "Remove this light").clicked() {
        l.fixtures.remove(i);
        state.selected = i.saturating_sub(1);
        state.test = LightTest::None;
    }
}

/// Lights one channel of the universe alone: to learn what a light's
/// channels do, or find its address.
fn channel_finder(ui: &mut Ui, state: &mut LightsState) {
    style::card_frame().show(ui, |ui| {
        ui.set_width(ui.available_width());
        style::label(ui, "Find the channels");
        ui.label(style::muted(
            "Lights one DMX channel alone, every other channel at 0. Step through 1, 2, 3… and watch the light: which channel is its dimmer, red, green, blue. A light that ignores every channel is set to another mode on its own display (choose its DMX mode, like d001 or A001).",
        ));
        ui.add_space(6.0);
        let on = matches!(state.test, LightTest::Channel(..));
        ui.horizontal_wrapped(|ui| {
            ui.label("Channel");
            if ui.button("−").clicked() {
                state.channel = state.channel.saturating_sub(1).max(1);
            }
            ui.add(egui::DragValue::new(&mut state.channel).range(1..=UNIVERSE as u16));
            if ui.button("+").clicked() {
                state.channel = (state.channel + 1).min(UNIVERSE as u16);
            }
            ui.label("Value");
            ui.spacing_mut().slider_width = 140.0;
            ui.add(egui::Slider::new(&mut state.value, 0..=255));
            let b = egui::Button::new(style::semibold(if on { "Stop" } else { "Light it" }, 14.0)).fill(if on { style::PINK.gamma_multiply(0.6) } else { style::CARD_LIGHT });
            if ui.add(b).clicked() {
                state.test = if on { LightTest::None } else { LightTest::Channel(state.channel, state.value, state.hold) };
            }
        });
        ui.horizontal_wrapped(|ui| {
            ui.label("Also hold at full").on_hover_text("Colours often stay dark while the dimmer is at 0: hold the dimmer channel open (usually the light's first channel)");
            ui.add(egui::DragValue::new(&mut state.hold).range(0..=UNIVERSE as u16).custom_formatter(|v, _| if v == 0.0 { "none".into() } else { format!("{v}") }));
        });
        if let LightTest::Channel(..) = state.test {
            state.test = LightTest::Channel(state.channel, state.value, state.hold);
            let hold = if state.hold > 0 { format!(", channel {} at 255", state.hold) } else { String::new() };
            ui.label(RichText::new(format!("Channel {} at {}{hold} · the show is off the lights while this runs", state.channel, state.value)).color(style::AMBER).size(13.0));
        }
        ui.label(style::muted("Careful: on some lights a high value on the strobe channel flashes fast."));
    });
}

fn smoke_panel(ui: &mut Ui, l: &mut Lighting, state: &mut LightsState) {
    style::label(ui, "Smoke");
    let has = l.fixtures.iter().any(|f| f.is_smoke());
    if !has {
        ui.label(style::muted("No smoke machine yet: + Smoke machine adds one (one DMX channel)."));
        return;
    }
    style::segmented(ui, &mut l.smoke, &[(SmokeMode::Off, "Off"), (SmokeMode::Hand, "By hand"), (SmokeMode::Auto, "Auto")]);
    ui.add_space(6.0);
    let (rect, r) = ui.allocate_exact_size(vec2(ui.available_width(), 46.0), Sense::click_and_drag());
    state.smoke_held = r.is_pointer_button_down_on() && l.smoke != SmokeMode::Off;
    let held = state.smoke_held;
    ui.painter().rect(rect, CornerRadius::same(12), if held { style::CYAN.gamma_multiply(0.5) } else { style::CARD_LIGHT }, Stroke::new(1.0, style::LINE), egui::StrokeKind::Inside);
    ui.painter().text(rect.center(), egui::Align2::CENTER_CENTER, if held { "Smoke…" } else { "Hold for smoke" }, egui::FontId::new(15.0, egui::FontFamily::Name("semibold".into())), style::TEXT);
    ui.add_space(6.0);
    egui::Grid::new("smoke").num_columns(2).spacing(vec2(12.0, 8.0)).show(ui, |ui| {
        ui.label("Amount");
        ui.add(egui::Slider::new(&mut l.smoke_amount, 0.0..=1.0).show_value(false));
        ui.end_row();
        if l.smoke == SmokeMode::Auto {
            ui.label("Burst");
            ui.add(egui::DragValue::new(&mut l.smoke_secs).range(0.5..=15.0).speed(0.1).suffix(" s"));
            ui.end_row();
            ui.label("At most every");
            ui.add(egui::DragValue::new(&mut l.smoke_gap_secs).range(10.0..=600.0).speed(1.0).suffix(" s"));
            ui.end_row();
        }
    });
    if l.smoke == SmokeMode::Auto {
        ui.label(style::muted("A burst on each drop and at the top of a build, never more often than set. Holding the button gives a burst too."));
    }
}
