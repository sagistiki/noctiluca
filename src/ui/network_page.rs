//! Network screens: Android TV boxes on this network. Install the
//! Noctiluca Screen app on one, open it, use it as an output.

use egui::Ui;

use super::style;
use super::{Live, NetScreen, UiAction};

pub fn draw(ui: &mut Ui, live: &Live, actions: &mut Vec<UiAction>) {
    ui.label(style::title("Network screens"));
    ui.label(style::muted("TV boxes on this network. Noctiluca streams to them, drawn ahead by the network's delay so they stay on the beat."));
    ui.add_space(12.0);
    egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
        let width = ui.available_width().min(860.0);
        network_screens(ui, live, actions, width);
    });
}

/// Android TV boxes on the network: install the screen app, open it, use
/// it as an output.
fn network_screens(ui: &mut Ui, live: &Live, actions: &mut Vec<UiAction>, width: f32) {
    style::card_frame().show(ui, |ui| {
        ui.set_width(width - 36.0);
        ui.horizontal(|ui| {
            style::label(ui, "On this network");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if live.net_searching {
                    ui.add(egui::Spinner::new());
                    ui.label(style::muted("Searching…"));
                } else if super::icons::small(ui, super::icons::Icon::Refresh, "Search the network again", style::MUTED).clicked() {
                    actions.push(UiAction::NetSearch);
                }
            });
        });
        ui.add_space(4.0);
        let screens: Vec<&NetScreen> = live.net_screens.iter().filter(|s| s.android_tv || s.app_open).collect();
        if screens.is_empty() {
            ui.label(style::muted("No Android TV found yet on this network. Is the Mac on the same Wi-Fi as the box?"));
        }
        for screen in screens {
            egui::Frame::new()
                .fill(style::CARD_LIGHT)
                .stroke(egui::Stroke::new(1.0, style::LINE))
                .corner_radius(egui::CornerRadius::same(12))
                .inner_margin(egui::Margin::symmetric(14, 10))
                .show(ui, |ui| {
                    ui.set_width(width - 36.0 - 28.0);
                    screen_row(ui, screen, actions);
                });
            ui.add_space(6.0);
        }
        ui.add_space(4.0);
        ui.collapsing(style::muted("A box is not listed, or says network debugging is off"), |ui| {
            ui.label(style::muted(
                "On the box: Settings → Device Preferences → About → select Build seven times. \
                 Then Settings → Device Preferences → Developer options → turn on USB debugging (it also allows the network). \
                 The first install asks on the TV to allow this Mac: tick Always allow and choose OK.",
            ));
        });
    });
}

fn screen_row(ui: &mut Ui, s: &NetScreen, actions: &mut Vec<UiAction>) {
    use crate::net::adb::{BoxState, SHIPPED_VERSION, version_older};
    let outdated = s.app_version.as_deref().is_some_and(|v| version_older(v, SHIPPED_VERSION))
        || matches!(s.install, Some(BoxState::Outdated(_)));
    let (dot, state): (egui::Color32, String) = if let Some(busy) = &s.busy {
        (style::AMBER, busy.clone())
    } else if s.app_open {
        let v = s.app_version.clone().unwrap_or_default();
        if outdated { (style::AMBER, format!("App open · v{v} · update available")) } else { (style::GREEN, format!("App open · v{v}")) }
    } else {
        match &s.install {
            None => (style::MUTED, "Android TV · app not open".into()),
            Some(BoxState::NoDebugging) => (style::RED, "Network debugging is off on this box".into()),
            Some(BoxState::WaitingForAllow) => (style::AMBER, "Allow this Mac on the TV (tick Always allow), then press Install again".into()),
            Some(BoxState::NotInstalled) => (style::MUTED, "App not installed".into()),
            Some(BoxState::Outdated(v)) => (style::AMBER, format!("App v{v} installed · update available")),
            Some(BoxState::Installed(v)) => (style::MUTED, format!("App v{v} installed · not open")),
        }
    };
    ui.horizontal(|ui| {
        ui.vertical(|ui| {
            ui.label(style::semibold(style::visual(&s.name), 15.0));
            let model = if s.model.is_empty() { String::new() } else { format!("{} · ", s.model) };
            ui.label(style::muted(format!("{model}{}", s.ip)));
        });
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let busy = s.busy.is_some();
            ui.add_enabled_ui(!busy, |ui| {
                if s.app_open {
                    match s.used_by {
                        Some(k) => {
                            ui.label(style::muted(format!("Output {}", k + 1)));
                        }
                        None => {
                            if style::primary_button(ui, "Use as output").clicked() {
                                actions.push(UiAction::NetUse(s.name.clone()));
                            }
                        }
                    }
                    if outdated && style::button(ui, "Update").clicked() {
                        actions.push(UiAction::NetInstall(s.ip.clone()));
                    }
                } else if s.android_tv {
                    let installed = matches!(s.install, Some(BoxState::Installed(_)));
                    if installed {
                        if style::primary_button(ui, "Open app").clicked() {
                            actions.push(UiAction::NetOpen(s.ip.clone()));
                        }
                    } else if style::primary_button(ui, if outdated { "Update" } else { "Install" }).clicked() {
                        actions.push(UiAction::NetInstall(s.ip.clone()));
                    }
                }
            });
            if busy {
                ui.add(egui::Spinner::new());
            }
        });
    });
    ui.horizontal(|ui| {
        let (rect, _) = ui.allocate_exact_size(egui::vec2(10.0, 10.0), egui::Sense::hover());
        ui.painter().circle_filled(rect.center(), 4.0, dot);
        ui.label(egui::RichText::new(state).size(13.0).color(style::TEXT));
    });
    if let Some(e) = &s.error {
        ui.label(egui::RichText::new(e).size(12.0).color(style::RED));
    }
}

