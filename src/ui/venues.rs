//! Venues: a whole set-up for a place, kept as a project file
//! ("<name>_nocvenue.json": outputs, surfaces, network screens, lights,
//! safety, sync, themes, quality; not the input device). Open, Save and
//! Save As work as in any Mac app (⌘O, ⌘S, ⇧⌘S); the page lists the files
//! in the venue folder and the ones opened from elsewhere lately. Loading
//! over unsaved changes asks first, as do replacing and moving to the Trash.

use std::path::PathBuf;

use egui::{Color32, CornerRadius, RichText, Sense, Stroke, Ui, vec2};

use super::style;
use super::{Live, UiAction};

/// One venue file, for the list.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct VenueInfo {
    pub name: String,
    /// "2 outputs · 4 surfaces · 2 lights".
    pub summary: String,
    pub path: PathBuf,
    /// Its folder as people read it: "~/Desktop".
    pub folder: String,
    /// In the venue folder (else opened from elsewhere lately).
    pub in_library: bool,
}

#[derive(Clone, Debug, PartialEq)]
enum Dialog {
    /// Load this file; the set-up now has unsaved changes.
    Load(PathBuf),
    /// Save the set-up now into this file, replacing what it held.
    Replace(PathBuf),
    Rename(PathBuf),
    Trash(PathBuf),
}

#[derive(Clone, Debug, Default)]
pub struct VenuesState {
    dialog: Option<Dialog>,
    /// The name being typed (Rename).
    name: String,
}

/// Loads `path`, asking first if the set-up now has unsaved changes.
pub fn ask_load(path: PathBuf, live: &Live, state: &mut VenuesState, actions: &mut Vec<UiAction>) {
    if live.venue_changed || live.venue_active.is_none() {
        state.dialog = Some(Dialog::Load(path));
    } else {
        actions.push(UiAction::LoadVenue(path));
    }
}

/// The dot and word for the venue in use.
fn saved_state(live: &Live) -> (Color32, &'static str) {
    if live.venue_changed { (style::AMBER, "changed") } else { (style::GREEN, "saved") }
}

/// The toolbar's venue status, in a right-to-left row: name, saved or
/// changed, and Save when there is something to save.
pub fn status_right(ui: &mut Ui, live: &Live, _state: &mut VenuesState, actions: &mut Vec<UiAction>) {
    match &live.venue_active {
        Some(name) => {
            if live.venue_changed && style::button(ui, "Save").on_hover_text("Save the changes into the venue file (⌘S)").clicked() {
                actions.push(UiAction::SaveVenue);
            }
            let (dot, text) = saved_state(live);
            ui.label(style::muted(text));
            ui.label(style::semibold(style::visual(name), 14.0));
            let (r, _) = ui.allocate_exact_size(vec2(10.0, 10.0), Sense::hover());
            ui.painter().circle_filled(r.center(), 4.0, dot);
        }
        None => {
            if style::button(ui, "Save as venue…").on_hover_text("Save everything set up now as a venue file (⇧⌘S)").clicked() {
                actions.push(UiAction::SaveVenueAs);
            }
        }
    }
}

/// The Venues page: Open, Save, Save As, the venue in use, and the files.
pub fn page(ui: &mut Ui, live: &Live, state: &mut VenuesState, actions: &mut Vec<UiAction>) {
    ui.horizontal(|ui| {
        ui.label(style::title("Venues"));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if style::button(ui, "Save As…").on_hover_text("Save the set-up as a new venue file (⇧⌘S)").clicked() {
                actions.push(UiAction::SaveVenueAs);
            }
            if live.venue_active.is_some() && ui.add_enabled(live.venue_changed, egui::Button::new(style::semibold("Save", 14.0))).on_hover_text("Save the changes into the venue file (⌘S)").clicked() {
                actions.push(UiAction::SaveVenue);
            }
            if style::primary_button(ui, "Open…").on_hover_text("Open a venue file from anywhere (⌘O)").clicked() {
                actions.push(UiAction::OpenVenue);
            }
        });
    });
    ui.label(style::muted(
        "A venue is one readable file, “name_nocvenue.json”, with everything set up for a place: outputs, surfaces, network screens, lights, flash safety, sync, themes and quality. Not the input device, which belongs to this computer.",
    ));
    ui.add_space(12.0);
    let w = ui.available_width().min(760.0);

    // The venue in use.
    style::card_frame().show(ui, |ui| {
        ui.set_width(w - 28.0);
        style::label(ui, "In use");
        match (&live.venue_active, &live.venue_path) {
            (Some(name), Some(path)) => {
                ui.horizontal(|ui| {
                    let (dot, text) = saved_state(live);
                    let (r, _) = ui.allocate_exact_size(vec2(10.0, 10.0), Sense::hover());
                    ui.painter().circle_filled(r.center(), 4.0, dot);
                    ui.label(style::semibold(style::visual(name), 16.0));
                    ui.label(style::muted(text));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if style::button(ui, "Show in Finder").clicked() {
                            actions.push(UiAction::RevealVenue(path.clone()));
                        }
                    });
                });
                if let Some(v) = live.venues.iter().find(|v| &v.path == path) {
                    ui.label(style::muted(format!("{} · {}", v.summary, v.folder)));
                }
            }
            _ => {
                ui.label(style::muted("No venue: the set-up now is not saved in a venue file. Save As… keeps it, Open… loads one."));
            }
        }
    });
    ui.add_space(12.0);

    // The files.
    style::card_frame().show(ui, |ui| {
        ui.set_width(w - 28.0);
        let list_h = (ui.available_height() - 40.0).max(60.0);
        egui::ScrollArea::vertical().max_height(list_h).auto_shrink([false, true]).show(ui, |ui| {
            style::label(ui, &format!("In {}", live.venue_library));
            let library: Vec<&VenueInfo> = live.venues.iter().filter(|v| v.in_library).collect();
            if library.is_empty() {
                ui.label(style::muted("No venue files here yet. Set up the outputs and lights for a place, then Save As…"));
            }
            for v in library {
                row(ui, v, live, state, actions);
            }
            let elsewhere: Vec<&VenueInfo> = live.venues.iter().filter(|v| !v.in_library).collect();
            if !elsewhere.is_empty() {
                ui.add_space(8.0);
                style::label(ui, "Opened from elsewhere");
                for v in elsewhere {
                    row(ui, v, live, state, actions);
                }
            }
        });
    });
    ui.add_space(8.0);
    ui.label(style::muted("Drop a _nocvenue.json file on this window to open it.  ⌘O open · ⌘S save · ⇧⌘S save as"));
}

/// One venue file: click to load; its menu has the rest.
fn row(ui: &mut Ui, v: &VenueInfo, live: &Live, state: &mut VenuesState, actions: &mut Vec<UiAction>) {
    let active = live.venue_path.as_ref() == Some(&v.path);
    let w = ui.available_width();
    let (rect, r) = ui.allocate_exact_size(vec2(w, 46.0), Sense::click());
    let p = ui.painter();
    let fill = if r.hovered() { Color32::from_rgb(0x34, 0x34, 0x38) } else { style::CARD_LIGHT };
    p.rect(rect, CornerRadius::same(8), fill, Stroke::new(if active { 1.5 } else { 1.0 }, if active { style::ACCENT } else { style::LINE }), egui::StrokeKind::Inside);
    p.text(rect.left_top() + vec2(12.0, 14.0), egui::Align2::LEFT_CENTER, style::visual(&v.name), egui::FontId::new(14.0, egui::FontFamily::Name("semibold".into())), style::TEXT);
    let detail = if v.in_library { v.summary.clone() } else { format!("{} · {}", v.summary, v.folder) };
    p.text(rect.left_top() + vec2(12.0, 32.0), egui::Align2::LEFT_CENTER, detail, egui::FontId::proportional(11.5), style::MUTED);
    let menu_rect = egui::Rect::from_center_size(rect.right_center() - vec2(20.0, 0.0), vec2(30.0, 30.0));
    let menu = ui.put(menu_rect, egui::Button::new(RichText::new("···").size(18.0).strong()).frame(false));
    egui::Popup::menu(&menu).show(|ui| {
        if ui.button("Open").clicked() {
            ask_load(v.path.clone(), live, state, actions);
        }
        if ui.button("Save the set-up now into it…").clicked() {
            state.dialog = Some(Dialog::Replace(v.path.clone()));
        }
        if ui.button("Rename…").clicked() {
            state.name = v.name.clone();
            state.dialog = Some(Dialog::Rename(v.path.clone()));
        }
        if ui.button("Show in Finder").clicked() {
            actions.push(UiAction::RevealVenue(v.path.clone()));
        }
        if ui.button(RichText::new("Move to Trash…").color(style::RED)).clicked() {
            state.dialog = Some(Dialog::Trash(v.path.clone()));
        }
    });
    if r.on_hover_text(if active { "The venue in use" } else { "Open this venue" }).clicked() && !active {
        ask_load(v.path.clone(), live, state, actions);
    }
    ui.add_space(4.0);
}

/// A file's venue name, for the dialogs.
fn name_of(path: &std::path::Path) -> String {
    style::visual(&crate::settings::venue_name(path))
}

/// The dialogs, over everything (called once a frame).
pub fn dialogs(ctx: &egui::Context, live: &Live, state: &mut VenuesState, actions: &mut Vec<UiAction>) {
    let Some(dialog) = state.dialog.clone() else { return };
    let mut close = false;
    let modal = egui::Modal::new(egui::Id::unique("venue dialog")).show(ctx, |ui| {
        ui.set_width(440.0);
        match &dialog {
            Dialog::Load(path) => {
                ui.label(style::heavy(format!("Open “{}”?", name_of(path)), 20.0));
                ui.add_space(6.0);
                ui.label(style::muted(match &live.venue_active {
                    Some(a) => format!("“{}” has changes that are not saved. Opening another venue replaces them.", style::visual(a)),
                    None => "The set-up now is not saved in a venue file. Opening a venue replaces it.".to_string(),
                }));
                ui.add_space(12.0);
                ui.horizontal(|ui| {
                    if style::primary_button(ui, "Open").clicked() {
                        actions.push(UiAction::LoadVenue(path.clone()));
                        close = true;
                    }
                    if live.venue_active.is_some() {
                        if style::button(ui, "Save first, then open").clicked() {
                            actions.push(UiAction::SaveVenue);
                            actions.push(UiAction::LoadVenue(path.clone()));
                            close = true;
                        }
                    } else if style::button(ui, "Save As… first").clicked() {
                        actions.push(UiAction::SaveVenueAs);
                        close = true;
                    }
                    if style::button(ui, "Cancel").clicked() {
                        close = true;
                    }
                });
            }
            Dialog::Replace(path) => {
                ui.label(style::heavy(format!("Save into “{}”?", name_of(path)), 20.0));
                ui.add_space(6.0);
                ui.label(style::muted("The file will hold the set-up as it is now. What it held before is gone."));
                ui.add_space(12.0);
                ui.horizontal(|ui| {
                    if style::primary_button(ui, "Replace").clicked() {
                        actions.push(UiAction::SaveVenueTo(path.clone()));
                        close = true;
                    }
                    if style::button(ui, "Cancel").clicked() {
                        close = true;
                    }
                });
            }
            Dialog::Rename(path) => {
                ui.label(style::heavy("Rename venue", 20.0));
                ui.add_space(8.0);
                let edit = ui.add(egui::TextEdit::singleline(&mut state.name).hint_text("Name, like the place: Club Ha'Oman, Home").desired_width(f32::INFINITY));
                if !edit.has_focus() && state.name.is_empty() {
                    edit.request_focus();
                }
                let name = state.name.trim().to_string();
                let dest = crate::settings::venue_path_in(path.parent().unwrap_or(std::path::Path::new(".")), &name);
                let taken = !name.is_empty() && dest != *path && dest.exists();
                ui.label(style::muted(format!("The file becomes “{}”", dest.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default())));
                if taken {
                    ui.label(RichText::new(format!("There is a venue called “{}” there already", style::visual(&name))).color(style::AMBER).size(13.0));
                }
                ui.add_space(10.0);
                let enter = edit.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                ui.horizontal(|ui| {
                    let ok = !name.is_empty() && !taken;
                    let go = ui.add_enabled(ok, egui::Button::new(style::semibold("Rename", 14.0)).fill(style::ACCENT)).clicked();
                    if (go || enter) && ok {
                        actions.push(UiAction::RenameVenue(path.clone(), name.clone()));
                        close = true;
                    }
                    if style::button(ui, "Cancel").clicked() {
                        close = true;
                    }
                });
            }
            Dialog::Trash(path) => {
                ui.label(style::heavy(format!("Move “{}” to the Trash?", name_of(path)), 20.0));
                ui.add_space(6.0);
                ui.label(style::muted("The file goes to the Trash, where it can be put back from. The set-up now does not change."));
                ui.add_space(12.0);
                ui.horizontal(|ui| {
                    let b = egui::Button::new(style::semibold("Move to Trash", 14.0)).fill(style::RED.gamma_multiply(0.6));
                    if ui.add(b).clicked() {
                        actions.push(UiAction::TrashVenue(path.clone()));
                        close = true;
                    }
                    if style::button(ui, "Cancel").clicked() {
                        close = true;
                    }
                });
            }
        }
    });
    if (modal.should_close() || close) && state.dialog.as_ref() == Some(&dialog) {
        state.dialog = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn live(active: Option<&str>, changed: bool) -> Live {
        Live {
            version: "test",
            venues: vec![
                VenueInfo { name: "בית".into(), path: "/v/בית_nocvenue.json".into(), in_library: true, ..Default::default() },
                VenueInfo { name: "Club".into(), path: "/v/Club_nocvenue.json".into(), in_library: true, ..Default::default() },
            ],
            venue_active: active.map(str::to_string),
            venue_path: active.map(|a| PathBuf::from(format!("/v/{a}_nocvenue.json"))),
            venue_changed: changed,
            ..Live::default()
        }
    }

    #[test]
    fn loading_over_unsaved_changes_asks_first() {
        let club = PathBuf::from("/v/Club_nocvenue.json");
        // Saved and unchanged: opens at once.
        let mut st = VenuesState::default();
        let mut actions = Vec::new();
        ask_load(club.clone(), &live(Some("בית"), false), &mut st, &mut actions);
        assert_eq!(actions, vec![UiAction::LoadVenue(club.clone())]);
        assert!(st.dialog.is_none());
        // Changed since saved, or never saved: asks.
        for l in [live(Some("בית"), true), live(None, false)] {
            let mut st = VenuesState::default();
            let mut actions = Vec::new();
            ask_load(club.clone(), &l, &mut st, &mut actions);
            assert!(actions.is_empty());
            assert_eq!(st.dialog, Some(Dialog::Load(club.clone())));
        }
    }

    /// The page and every dialog draw headless, and nothing happens
    /// without a click.
    #[test]
    fn the_page_and_every_dialog_draw() {
        let ctx = egui::Context::default();
        style::apply(&ctx);
        let l = live(Some("בית"), true);
        let club = PathBuf::from("/v/Club_nocvenue.json");
        for dialog in [Dialog::Load(club.clone()), Dialog::Replace(club.clone()), Dialog::Rename(club.clone()), Dialog::Trash(club.clone())] {
            let mut st = VenuesState { dialog: Some(dialog), name: "Club 2".into() };
            let mut actions = Vec::new();
            let input = egui::RawInput { screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, vec2(1280.0, 800.0))), ..Default::default() };
            let _ = ctx.run_ui(input, |ui| {
                page(ui, &l, &mut st, &mut actions);
                dialogs(ui.ctx(), &l, &mut st, &mut actions);
            });
            assert!(actions.is_empty(), "nothing happens without a click: {actions:?}");
        }
    }
}
