//! The Mac's own Open and Save panels, and Show in Finder, for venue files.
//!
//! The panels come from AppleScript's `choose file` and `choose file name`
//! (run by `osascript`, so no extra library), in a thread of their own so
//! the show keeps playing while one is open. The answer arrives later
//! through `poll()`.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::mpsc::{Receiver, channel};

/// What a panel was opened for.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Purpose {
    /// Open a venue file.
    OpenVenue,
    /// Save the set-up as a venue file.
    SaveVenueAs,
}

/// A panel open now, answering later.
pub struct Panel {
    pub purpose: Purpose,
    answer: Receiver<Option<PathBuf>>,
}

impl Panel {
    /// The file chosen, once the panel closes: `Some(None)` if cancelled,
    /// `None` while it is still open.
    pub fn poll(&self) -> Option<Option<PathBuf>> {
        match self.answer.try_recv() {
            Ok(answer) => Some(answer),
            Err(std::sync::mpsc::TryRecvError::Empty) => None,
            Err(std::sync::mpsc::TryRecvError::Disconnected) => Some(None),
        }
    }
}

/// Text made safe inside an AppleScript string.
fn quoted(text: &str) -> String {
    format!("\"{}\"", text.replace('\\', "\\\\").replace('"', "\\\""))
}

/// The script for an Open panel in `dir`.
fn open_script(dir: &Path) -> String {
    format!(
        "set f to choose file with prompt {} of type {{\"public.json\"}} default location (POSIX file {})\nPOSIX path of f",
        quoted("Open a venue (a file ending in _nocvenue.json)"),
        quoted(&dir.to_string_lossy())
    )
}

/// The script for a Save panel in `dir`, suggesting `file_name`.
fn save_script(dir: &Path, file_name: &str) -> String {
    format!(
        "set f to choose file name with prompt {} default name {} default location (POSIX file {})\nPOSIX path of f",
        quoted("Save the venue as"),
        quoted(file_name),
        quoted(&dir.to_string_lossy())
    )
}

fn run(purpose: Purpose, script: String) -> Panel {
    let (tx, answer) = channel();
    std::thread::Builder::new()
        .name("file panel".into())
        .spawn(move || {
            let out = Command::new("/usr/bin/osascript").arg("-e").arg(&script).output();
            let path = out
                .ok()
                .filter(|o| o.status.success())
                .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
                .filter(|p| !p.is_empty())
                .map(PathBuf::from);
            let _ = tx.send(path);
        })
        .ok();
    Panel { purpose, answer }
}

/// Opens the Mac's Open panel in `dir`.
pub fn open_venue(dir: &Path) -> Panel {
    run(Purpose::OpenVenue, open_script(dir))
}

/// Opens the Mac's Save panel in `dir`, suggesting `name`_nocvenue.json.
pub fn save_venue_as(dir: &Path, name: &str) -> Panel {
    let file = format!("{}{}", if name.trim().is_empty() { "My venue" } else { name.trim() }, crate::settings::VENUE_SUFFIX);
    run(Purpose::SaveVenueAs, save_script(dir, &file))
}

/// Shows the file, selected, in a Finder window.
pub fn reveal(path: &Path) {
    let _ = Command::new("/usr/bin/open").arg("-R").arg(path).spawn();
}

/// A folder as people read it: "~/Documents/Noctiluca Venues".
pub fn pretty_folder(dir: &Path) -> String {
    let home = std::env::var_os("HOME").map(PathBuf::from);
    match home.as_deref().and_then(|h| dir.strip_prefix(h).ok()) {
        Some(rest) if rest.as_os_str().is_empty() => "~".into(),
        Some(rest) => format!("~/{}", rest.display()),
        None => dir.display().to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Quotes and backslashes in a folder or a name cannot break out of the
    /// script's strings.
    #[test]
    fn names_are_quoted_safely() {
        assert_eq!(quoted(r#"Club "Ha'Oman" \ 2"#), r#""Club \"Ha'Oman\" \\ 2""#);
        let s = save_script(Path::new("/Users/x/My \"Venues\""), "Barby_nocvenue.json");
        assert!(s.contains(r#"default name "Barby_nocvenue.json""#));
        assert!(s.contains(r#"POSIX file "/Users/x/My \"Venues\"""#));
        assert!(open_script(Path::new("/tmp")).contains(r#"of type {"public.json"}"#));
    }

    #[test]
    fn folders_read_from_home() {
        let home = PathBuf::from(std::env::var_os("HOME").unwrap());
        assert_eq!(pretty_folder(&home.join("Documents/Noctiluca Venues")), "~/Documents/Noctiluca Venues");
        assert_eq!(pretty_folder(Path::new("/Volumes/USB")), "/Volumes/USB");
    }
}
