//! The settings page: flash safety, sync, performance, reset. It scrolls,
//! and its two columns stack on a narrow window.

use egui::Ui;

use super::style;
use super::{Live, UiAction};
use crate::settings::{Quality, Settings};

pub fn draw(ui: &mut Ui, settings: &mut Settings, live: &Live, actions: &mut Vec<UiAction>) {
    let _ = actions;
    ui.label(style::title("Settings"));
    ui.add_space(12.0);
    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            // Two columns when they fit, one under the other when not.
            let room = ui.available_width();
            let width = room.min(460.0);
            if room >= 2.0 * width + style::GAP {
                ui.horizontal_top(|ui| {
                    ui.vertical(|ui| {
                        ui.set_width(width);
                        flash_and_sync(ui, settings, width);
                    });
                    ui.add_space(style::GAP);
                    ui.vertical(|ui| {
                        ui.set_width(width);
                        performance_and_reset(ui, settings, live, width);
                    });
                });
            } else {
                ui.vertical(|ui| {
                    ui.set_width(width);
                    flash_and_sync(ui, settings, width);
                    ui.add_space(style::GAP);
                    performance_and_reset(ui, settings, live, width);
                });
            }
        });
}

fn flash_and_sync(ui: &mut Ui, settings: &mut Settings, width: f32) {
    // Sliders that leave room for their label and number inside the card.
    ui.spacing_mut().slider_width = (width - 330.0).clamp(80.0, 200.0);
    style::card_frame().show(ui, |ui| {
        ui.set_width(width - 36.0);
        style::label(ui, "Flash safety");
        ui.horizontal(|ui| {
            ui.label("Strobe and lightning");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                style::toggle(ui, &mut settings.strobe)
            });
        });
        ui.horizontal(|ui| {
            ui.label("Brightest flash");
            ui.add(egui::Slider::new(&mut settings.max_flash, 0.0..=0.6).fixed_decimals(2));
        });
        ui.label(style::muted(
            "Never more than three flashes a second, whatever is set here.",
        ));
    });
    ui.add_space(style::GAP);
    style::card_frame().show(ui, |ui| {
        ui.set_width(width - 36.0);
        style::label(ui, "Sync");
        ui.horizontal(|ui| {
            ui.label("Visuals earlier (+) or later (−)");
            ui.add(
                egui::Slider::new(&mut settings.sync_offset_ms, -300.0..=300.0)
                    .step_by(10.0)
                    .suffix(" ms"),
            );
        });
        ui.label(style::muted("Also with [ and ] during the show."));
    });
}

fn performance_and_reset(ui: &mut Ui, settings: &mut Settings, live: &Live, width: f32) {
    style::card_frame().show(ui, |ui| {
        ui.set_width(width - 36.0);
        style::label(ui, "Performance");
        ui.horizontal(|ui| {
            ui.label("Quality");
            style::segmented(
                ui,
                &mut settings.quality,
                &[
                    (Quality::High, "High"),
                    (Quality::Balanced, "Balanced"),
                    (Quality::Light, "Light"),
                ],
            );
        });
        ui.label(style::muted(format!(
            "Pictures are drawn at most {} pixels tall. Lighter leaves more for Serato.",
            settings.quality.view_height()
        )));
    });
    ui.add_space(style::GAP);
    style::card_frame().show(ui, |ui| {
        ui.set_width(width - 36.0);
        style::label(ui, "Reset");
        ui.label(style::muted(
            "Back to the settings Noctiluca started with. Saved venues stay.",
        ));
        ui.add_space(4.0);
        if style::button(ui, "Reset to defaults").clicked() {
            *settings = Settings::default();
        }
    });
    ui.add_space(8.0);
    ui.label(style::muted(format!("Noctiluca {}", live.version)));
}
