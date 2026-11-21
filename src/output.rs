//! Which projector window should be open: worked out from the display the
//! settings ask for and the displays that are connected right now.

/// Why the output window closes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Reason {
    /// The display it was on went away.
    Unplugged,
    /// The settings no longer ask for one.
    TurnedOff,
    /// The display asked for is the one the control window is on.
    SameAsControl,
    /// The settings ask for a different display.
    Moved,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum OutputChange {
    Keep,
    Open(String),
    Close(Reason),
}

/// What to do about the output window.
///
/// * `wanted`: the display the settings name, if any.
/// * `available`: the connected displays, by name.
/// * `control`: the display the control window is on.
/// * `open`: the display the output window is on now, if it is open.
pub fn reconcile(
    wanted: Option<&str>,
    available: &[String],
    control: Option<&str>,
    open: Option<&str>,
) -> OutputChange {
    let usable = |name: &str| available.iter().any(|a| a == name) && Some(name) != control;
    match (wanted, open) {
        (None, None) => OutputChange::Keep,
        (None, Some(_)) => OutputChange::Close(Reason::TurnedOff),
        (Some(w), Some(o)) if w == o => {
            if !available.iter().any(|a| a == o) {
                OutputChange::Close(Reason::Unplugged)
            } else if Some(o) == control {
                OutputChange::Close(Reason::SameAsControl)
            } else {
                OutputChange::Keep
            }
        }
        (Some(_), Some(_)) => OutputChange::Close(Reason::Moved),
        (Some(w), None) if usable(w) => OutputChange::Open(w.to_string()),
        (Some(_), None) => OutputChange::Keep,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn names(n: &[&str]) -> Vec<String> {
        n.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn output_follows_displays() {
        let both = names(&["Built-in", "Epson"]);
        let mac = names(&["Built-in"]);
        let built_in = Some("Built-in");
        // Asked for and connected: open it.
        assert_eq!(
            reconcile(Some("Epson"), &both, built_in, None),
            OutputChange::Open("Epson".into())
        );
        // Open and still there: keep.
        assert_eq!(
            reconcile(Some("Epson"), &both, built_in, Some("Epson")),
            OutputChange::Keep
        );
        // Unplugged while open: close, but the setting stays, so...
        assert_eq!(
            reconcile(Some("Epson"), &mac, built_in, Some("Epson")),
            OutputChange::Close(Reason::Unplugged)
        );
        // ...while it is away nothing happens, and when it is back it opens again.
        assert_eq!(
            reconcile(Some("Epson"), &mac, built_in, None),
            OutputChange::Keep
        );
        assert_eq!(
            reconcile(Some("Epson"), &both, built_in, None),
            OutputChange::Open("Epson".into())
        );
    }

    #[test]
    fn closing_the_output_window_turns_output_off() {
        // The app clears the setting when the window is closed by hand; then:
        assert_eq!(
            reconcile(
                None,
                &names(&["Built-in", "Epson"]),
                Some("Built-in"),
                Some("Epson")
            ),
            OutputChange::Close(Reason::TurnedOff)
        );
        assert_eq!(
            reconcile(None, &names(&["Built-in", "Epson"]), Some("Built-in"), None),
            OutputChange::Keep
        );
    }

    #[test]
    fn no_output_on_the_control_windows_display() {
        let both = names(&["Built-in", "Epson"]);
        assert_eq!(
            reconcile(Some("Built-in"), &both, Some("Built-in"), None),
            OutputChange::Keep
        );
        // The control window was dragged onto the projector: close the output there.
        assert_eq!(
            reconcile(Some("Epson"), &both, Some("Epson"), Some("Epson")),
            OutputChange::Close(Reason::SameAsControl)
        );
    }

    #[test]
    fn choosing_another_display_moves_the_output() {
        let three = names(&["Built-in", "Epson", "LG"]);
        assert_eq!(
            reconcile(Some("LG"), &three, Some("Built-in"), Some("Epson")),
            OutputChange::Close(Reason::Moved)
        );
        assert_eq!(
            reconcile(Some("LG"), &three, Some("Built-in"), None),
            OutputChange::Open("LG".into())
        );
    }
}
