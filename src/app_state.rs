//! What the app is doing: which page the control window shows, and whether
//! the show is running. The two are separate: the show keeps running on
//! the projector (or in the menu's preview) while the menu is open. Esc never
//! stops it; only Stop does.

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Page {
    /// The show: program, pads, inspector, the set's timeline.
    #[default]
    Live,
    /// Mapping: outputs and surfaces.
    Mapping,
    Lights,
    /// Network screens (TV boxes).
    Network,
    Venues,
    Settings,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    /// Start the show: on the projector if there is one, else full screen here.
    Start,
    /// Start (or keep) the show and show it full screen on the Mac too.
    PreviewHere,
    Stop,
    /// Back to Live; the show goes on.
    Escape,
    Open(Page),
    /// From a page back to Live.
    Back,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct AppState {
    pub page: Page,
    pub running: bool,
    /// The control window shows the show itself instead of the menu.
    pub show_here: bool,
}

impl AppState {
    /// `has_output`: a projector window is open.
    pub fn apply(&mut self, action: Action, has_output: bool) {
        match action {
            Action::Start => {
                self.running = true;
                self.show_here = !has_output;
            }
            Action::PreviewHere => {
                self.running = true;
                self.show_here = true;
            }
            Action::Stop => {
                self.running = false;
                self.show_here = false;
                self.page = Page::Live;
            }
            Action::Escape => {
                if self.show_here {
                    self.show_here = false;
                } else {
                    self.page = Page::Live;
                }
            }
            Action::Open(page) => {
                self.page = page;
                self.show_here = false;
            }
            Action::Back => self.page = Page::Live,
        }
    }

    /// The control window shows the show full screen (no menu).
    pub fn control_shows_show(&self) -> bool {
        self.running && self.show_here
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn after(actions: &[Action], has_output: bool) -> AppState {
        let mut s = AppState::default();
        for a in actions {
            s.apply(*a, has_output);
        }
        s
    }

    #[test]
    fn escape_keeps_the_show_running() {
        let s = after(&[Action::Start, Action::Escape], false);
        assert_eq!(s.page, Page::Live);
        assert!(s.running);
        assert!(!s.control_shows_show());
    }

    #[test]
    fn only_stop_stops() {
        let s = after(
            &[
                Action::Start,
                Action::Escape,
                Action::Escape,
                Action::Open(Page::Mapping),
                Action::Back,
            ],
            false,
        );
        assert!(s.running);
        assert!(!after(&[Action::Start, Action::Stop], false).running);
    }

    #[test]
    fn start_with_output_keeps_the_menu_on_the_mac() {
        let s = after(&[Action::Start], true);
        assert!(s.running && !s.control_shows_show());
    }

    #[test]
    fn start_without_output_shows_the_show_here() {
        assert!(after(&[Action::Start], false).control_shows_show());
    }

    #[test]
    fn preview_here_with_an_output_still_feeds_the_output() {
        let s = after(&[Action::Start, Action::PreviewHere], true);
        assert!(s.running && s.control_shows_show());
        let s = after(&[Action::Start, Action::PreviewHere, Action::Escape], true);
        assert!(s.running && !s.control_shows_show());
    }

    #[test]
    fn opening_a_page_while_the_show_is_here_brings_the_menu_back() {
        let s = after(&[Action::Start, Action::Open(Page::Settings)], false);
        assert_eq!(s.page, Page::Settings);
        assert!(s.running && !s.control_shows_show());
    }
}
