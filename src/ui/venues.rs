//! Venues: a whole set-up saved under a name (outputs, surfaces, lights,
//! safety, sync, themes, quality; not the input device), to load at the
//! next gig there. The menu's card lists them; the active one shows whether
//! it has changed since it was saved; loading over unsaved changes asks
//! first, as do replacing and deleting.

use egui::{Color32, CornerRadius, RichText, Sense, Stroke, Ui, vec2};

use super::style;
use super::{Live, UiAction};

/// One saved venue, for the list.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct VenueInfo {
    pub name: String,
    /// "2 outputs · 4 surfaces · 2 lights".
    pub summary: String,
}

#[derive(Clone, Debug, PartialEq)]
enum Dialog {
    /// Load this one; the current set-up has unsaved changes.
    Load(String),
    /// Save the current set-up under a new name.
    SaveAs,
    /// The name typed is taken: replace it?
    Replace(String),
    Rename(String),
    Delete(String),
}

#[derive(Clone, Debug, Default)]
pub struct VenuesState {
    dialog: Option<Dialog>,
    /// The name being typed (Save as, Rename).
    name: String,
}

impl VenuesState {
    /// Opens "Save as new venue".
    pub fn save_as(&mut self) {
        self.name.clear();
        self.dialog = Some(Dialog::SaveAs);
    }
}

/// Loads `name`, asking first if the current set-up has unsaved changes.
fn ask_load(name: &str, live: &Live, state: &mut VenuesState, actions: &mut Vec<UiAction>) {
    if live.venue_changed || live.venue_active.is_none() {
        state.dialog = Some(Dialog::Load(name.to_string()));
    } else {
        actions.push(UiAction::LoadVenue(name.to_string()));
    }
}

/// The active venue and whether it is saved: a line with a Save button.
pub fn status(ui: &mut Ui, live: &Live, state: &mut VenuesState, actions: &mut Vec<UiAction>) {
    match &live.venue_active {
        Some(name) => {
            let (dot, text) = if live.venue_changed { (style::AMBER, "changed") } else { (style::GREEN, "saved") };
            let (r, _) = ui.allocate_exact_size(vec2(10.0, 10.0), Sense::hover());
            ui.painter().circle_filled(r.center(), 4.0, dot);
            ui.label(style::semibold(style::visual(name), 14.0));
            ui.label(style::muted(text));
            if live.venue_changed && style::button(ui, "Save").on_hover_text("Update this venue with the set-up as it is now").clicked() {
                actions.push(UiAction::SaveVenue(name.clone()));
            }
        }
        None => {
            ui.label(style::muted("No venue"));
            if style::button(ui, "Save as venue…").clicked() {
                state.save_as();
            }
        }
    }
}

/// `status` inside a right-to-left row (a page header's right side): the
/// same pieces, added from the right.
pub fn status_right(ui: &mut Ui, live: &Live, state: &mut VenuesState, actions: &mut Vec<UiAction>) {
    match &live.venue_active {
        Some(name) => {
            if live.venue_changed && style::button(ui, "Save").on_hover_text("Update this venue with the set-up as it is now").clicked() {
                actions.push(UiAction::SaveVenue(name.clone()));
            }
            let (dot, text) = if live.venue_changed { (style::AMBER, "changed") } else { (style::GREEN, "saved") };
            ui.label(style::muted(text));
            ui.label(style::semibold(style::visual(name), 14.0));
            let (r, _) = ui.allocate_exact_size(vec2(10.0, 10.0), Sense::hover());
            ui.painter().circle_filled(r.center(), 4.0, dot);
        }
        None => {
            if style::button(ui, "Save as venue…").clicked() {
                state.save_as();
            }
        }
    }
}

/// The Venues page: what a venue is, and the list to load, save, rename
/// and delete.
pub fn page(ui: &mut Ui, live: &Live, state: &mut VenuesState, actions: &mut Vec<UiAction>) {
    ui.label(style::title("Venues"));
    ui.label(style::muted("A venue keeps everything set up for a place: outputs, surfaces, network screens, lights, flash safety, sync, themes and quality. Not the input device, which belongs to this computer."));
    ui.add_space(12.0);
    let w = ui.available_width().min(720.0);
    style::card_frame().show(ui, |ui| {
        ui.set_width(w - 28.0);
        card(ui, live, state, actions);
    });
}

/// The list: the active venue, every saved one, save as new.
pub fn card(ui: &mut Ui, live: &Live, state: &mut VenuesState, actions: &mut Vec<UiAction>) {
    style::label(ui, "Venues");
    ui.horizontal(|ui| status(ui, live, state, actions));
    ui.add_space(6.0);
    let list_h = (ui.available_height() - 44.0).max(40.0);
    egui::ScrollArea::vertical().max_height(list_h).auto_shrink([false, true]).show(ui, |ui| {
        if live.venues.is_empty() {
            ui.label(style::muted("None saved yet. Set up the outputs and lights for a place, then save them here as a venue."));
        }
        for v in &live.venues {
            let active = live.venue_active.as_ref() == Some(&v.name);
            let w = ui.available_width();
            let (rect, r) = ui.allocate_exact_size(vec2(w, 46.0), Sense::click());
            let p = ui.painter();
            let fill = if r.hovered() { Color32::from_rgb(0x2a, 0x2a, 0x38) } else { style::CARD_LIGHT };
            p.rect(rect, CornerRadius::same(10), fill, Stroke::new(if active { 1.5 } else { 1.0 }, if active { style::PINK } else { style::LINE }), egui::StrokeKind::Inside);
            p.text(rect.left_top() + vec2(12.0, 14.0), egui::Align2::LEFT_CENTER, style::visual(&v.name), egui::FontId::new(14.0, egui::FontFamily::Name("semibold".into())), style::TEXT);
            p.text(rect.left_top() + vec2(12.0, 32.0), egui::Align2::LEFT_CENTER, &v.summary, egui::FontId::proportional(11.5), style::MUTED);
            // The row's menu, on its right.
            let menu_rect = egui::Rect::from_center_size(rect.right_center() - vec2(20.0, 0.0), vec2(30.0, 30.0));
            let menu = ui.put(menu_rect, egui::Button::new(RichText::new("···").size(18.0).strong()).frame(false));
            egui::Popup::menu(&menu).show(|ui| {
                if ui.button("Load").clicked() {
                    ask_load(&v.name, live, state, actions);
                }
                if ui.button("Update with the set-up now").clicked() {
                    state.dialog = Some(Dialog::Replace(v.name.clone()));
                }
                if ui.button("Rename…").clicked() {
                    state.name = v.name.clone();
                    state.dialog = Some(Dialog::Rename(v.name.clone()));
                }
                if ui.button(RichText::new("Delete…").color(style::RED)).clicked() {
                    state.dialog = Some(Dialog::Delete(v.name.clone()));
                }
            });
            if r.on_hover_text(if active { "The venue in use" } else { "Load this venue" }).clicked() && !active {
                ask_load(&v.name, live, state, actions);
            }
            ui.add_space(4.0);
        }
    });
    if style::button(ui, "+ Save as new venue").clicked() {
        state.save_as();
    }
}

/// The dialogs, over everything (called once a frame by the page).
pub fn dialogs(ctx: &egui::Context, live: &Live, state: &mut VenuesState, actions: &mut Vec<UiAction>) {
    let Some(dialog) = state.dialog.clone() else { return };
    let mut close = false;
    let modal = egui::Modal::new(egui::Id::unique("venue dialog")).show(ctx, |ui| {
        ui.set_width(420.0);
        match &dialog {
            Dialog::Load(name) => {
                ui.label(style::heavy(format!("Load “{}”?", style::visual(name)), 20.0));
                ui.add_space(6.0);
                ui.label(style::muted(match &live.venue_active {
                    Some(a) => format!("The set-up now (“{}”) has changes that are not saved. Loading replaces them.", style::visual(a)),
                    None => "The set-up now is not saved as a venue. Loading replaces it.".to_string(),
                }));
                ui.add_space(12.0);
                ui.horizontal(|ui| {
                    if style::primary_button(ui, "Load").clicked() {
                        actions.push(UiAction::LoadVenue(name.clone()));
                        close = true;
                    }
                    match &live.venue_active {
                        Some(a) => {
                            if style::button(ui, &format!("Save “{}” first", style::visual(a))).clicked() {
                                actions.push(UiAction::SaveVenue(a.clone()));
                                actions.push(UiAction::LoadVenue(name.clone()));
                                close = true;
                            }
                        }
                        None => {
                            if style::button(ui, "Save it first…").clicked() {
                                state.name.clear();
                                state.dialog = Some(Dialog::SaveAs);
                            }
                        }
                    }
                    if style::button(ui, "Cancel").clicked() {
                        close = true;
                    }
                });
            }
            Dialog::SaveAs | Dialog::Rename(_) => {
                let renaming = matches!(dialog, Dialog::Rename(_));
                ui.label(style::heavy(if renaming { "Rename venue" } else { "Save as a venue" }, 20.0));
                ui.add_space(4.0);
                if !renaming {
                    ui.label(style::muted("Everything set up now: outputs, surfaces, lights, flash safety, sync, themes, quality. Not the input device (it belongs to this computer)."));
                }
                ui.add_space(8.0);
                let edit = ui.add(egui::TextEdit::singleline(&mut state.name).hint_text("Name, like the place: Club Ha'Oman, Home").desired_width(f32::INFINITY));
                if !edit.has_focus() && state.name.is_empty() {
                    edit.request_focus();
                }
                let name = state.name.trim().to_string();
                let taken = live.venues.iter().any(|v| v.name == name) && !matches!(&dialog, Dialog::Rename(old) if *old == name);
                if taken {
                    ui.label(RichText::new(format!("There is a venue called “{}” already", style::visual(&name))).color(style::AMBER).size(13.0));
                }
                ui.add_space(10.0);
                let enter = edit.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                ui.horizontal(|ui| {
                    let go_label = if renaming { "Rename" } else if taken { "Replace…" } else { "Save" };
                    let go = ui.add_enabled(!name.is_empty() && !(renaming && taken), egui::Button::new(style::semibold(go_label, 14.0)).fill(style::PINK)).clicked();
                    if (go || enter) && !name.is_empty() {
                        match &dialog {
                            Dialog::Rename(old) if !taken => {
                                actions.push(UiAction::RenameVenue(old.clone(), name.clone()));
                                close = true;
                            }
                            Dialog::Rename(_) => {}
                            _ if taken => state.dialog = Some(Dialog::Replace(name.clone())),
                            _ => {
                                actions.push(UiAction::SaveVenue(name.clone()));
                                close = true;
                            }
                        }
                    }
                    if style::button(ui, "Cancel").clicked() {
                        close = true;
                    }
                });
            }
            Dialog::Replace(name) => {
                ui.label(style::heavy(format!("Replace “{}”?", style::visual(name)), 20.0));
                ui.add_space(6.0);
                ui.label(style::muted("It will hold the set-up as it is now. What it held before is gone."));
                ui.add_space(12.0);
                ui.horizontal(|ui| {
                    if style::primary_button(ui, "Replace").clicked() {
                        actions.push(UiAction::SaveVenue(name.clone()));
                        close = true;
                    }
                    if style::button(ui, "Cancel").clicked() {
                        close = true;
                    }
                });
            }
            Dialog::Delete(name) => {
                ui.label(style::heavy(format!("Delete “{}”?", style::visual(name)), 20.0));
                ui.add_space(6.0);
                ui.label(style::muted("The saved venue is removed. The set-up now does not change."));
                ui.add_space(12.0);
                ui.horizontal(|ui| {
                    let b = egui::Button::new(style::semibold("Delete", 14.0)).fill(style::RED.gamma_multiply(0.6));
                    if ui.add(b).clicked() {
                        actions.push(UiAction::DeleteVenue(name.clone()));
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
            venues: vec![VenueInfo { name: "בית".into(), summary: String::new() }, VenueInfo { name: "Club".into(), summary: String::new() }],
            venue_active: active.map(str::to_string),
            venue_changed: changed,
            ..Live::default()
        }
    }

    #[test]
    fn loading_over_unsaved_changes_asks_first() {
        // Saved and unchanged: loads at once.
        let mut st = VenuesState::default();
        let mut actions = Vec::new();
        ask_load("Club", &live(Some("בית"), false), &mut st, &mut actions);
        assert_eq!(actions, vec![UiAction::LoadVenue("Club".into())]);
        assert!(st.dialog.is_none());
        // Changed since saved, or never saved: asks.
        for l in [live(Some("בית"), true), live(None, false)] {
            let mut st = VenuesState::default();
            let mut actions = Vec::new();
            ask_load("Club", &l, &mut st, &mut actions);
            assert!(actions.is_empty());
            assert_eq!(st.dialog, Some(Dialog::Load("Club".into())));
        }
    }

    /// The whole dialog, headless: Save as with a taken name asks to replace.
    #[test]
    fn saving_under_a_taken_name_asks_to_replace() {
        let ctx = egui::Context::default();
        style::apply(&ctx);
        let l = live(Some("בית"), true);
        let mut st = VenuesState::default();
        st.save_as();
        st.name = "Club".into();
        let mut actions = Vec::new();
        // Press Enter in the name field: "Club" is taken, so it asks.
        for events in [vec![], vec![egui::Event::Key { key: egui::Key::Enter, physical_key: None, pressed: true, repeat: false, modifiers: Default::default() }]] {
            let input = egui::RawInput { screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, vec2(1280.0, 800.0))), events, ..Default::default() };
            let _ = ctx.run_ui(input, |ui| dialogs(ui.ctx(), &l, &mut st, &mut actions));
        }
        assert!(actions.is_empty(), "{actions:?}");
        assert!(matches!(st.dialog, Some(Dialog::SaveAs) | Some(Dialog::Replace(_))), "{:?}", st.dialog);
    }
}
