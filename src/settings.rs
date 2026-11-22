//! Everything the menu sets, kept between runs.
//!
//! `~/Library/Application Support/Noctiluca/settings.json` holds the working
//! set-up, written shortly after every change. Named venues are complete
//! presets (the projector and its surfaces, corners and modes, and the show's
//! themes, variety, sync, flash and quality), kept beside it in
//! `venues/<name>.json` and written only when saved on purpose. Loading one
//! keeps the input device: that belongs to the computer, not the place.
//!
//! Files are read forgivingly: a missing field takes its default, a value
//! out of range is pulled back in, and a file that cannot be read at all
//! gives the defaults and a message instead of stopping the program.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::director::THEMES;

/// How long after a change the settings are written, at most.
const SAVE_AFTER: Duration = Duration::from_millis(500);
pub const MAX_SURFACES: usize = 8;
pub const MAX_OUTPUTS: usize = 4;
/// Main surfaces: at least one, at most this many.
pub const MAX_MAINS: usize = 2;
/// The sync offset is kept within this many milliseconds either way.
const MAX_OFFSET_MS: f32 = 300.0;
const MAX_FLASH: f32 = 0.6;
pub const VARIETY_RANGE: (f32, f32) = (0.05, 0.5);

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Quality {
    #[default]
    High,
    Balanced,
    Light,
}

impl Quality {
    /// The tallest a view of the show is drawn.
    pub fn view_height(self) -> u32 {
        match self {
            Quality::High => 1080,
            Quality::Balanced => 900,
            Quality::Light => 720,
        }
    }
}

/// What a surface shows.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Content {
    /// The director's show.
    #[default]
    Main,
    /// A copy of Main.
    Same,
    /// A look of its own that goes with Main.
    Complement,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum ComplementStyle {
    #[default]
    MirrorMotion,
    Chase,
    Echo,
}

/// What the projector shows right now.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum OutputPattern {
    #[default]
    Show,
    /// Grid, circle, cross and number per surface, on black.
    Grid,
    White,
    Black,
}

/// One area of the projection: a four-cornered shape the picture is warped into.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Surface {
    pub name: String,
    /// Which output (projector) it is on: an index into `OutputSetup::outputs`.
    pub output: usize,
    /// Top left, top right, bottom right, bottom left; 0..1 across the output.
    pub corners: [[f32; 2]; 4],
    pub content: Content,
    pub style: ComplementStyle,
    pub flip: bool,
    pub feather_px: f32,
    pub brightness: f32,
    /// Shown at all (hidden ones stay in the layout, dark).
    pub visible: bool,
    /// Its corners cannot be moved by mistake.
    pub locked: bool,
    /// Its real width over height where it lands (a wall seen from the
    /// side, a ceiling): the picture keeps this shape however slanted the
    /// corners are. None: worked out from the corners.
    pub aspect: Option<f32>,
}

impl Default for Surface {
    fn default() -> Self {
        Self {
            name: "Screen".into(),
            output: 0,
            corners: FULL_FRAME,
            content: Content::Main,
            style: ComplementStyle::MirrorMotion,
            flip: false,
            feather_px: 0.0,
            brightness: 1.0,
            visible: true,
            locked: false,
            aspect: None,
        }
    }
}

pub const FULL_FRAME: [[f32; 2]; 4] = [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]];

/// One output: a projector or display, and what it shows right now.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct OutputConf {
    pub name: String,
    /// The display's name (or a network screen's, see `network_display`);
    /// `None` while it has none.
    pub display: Option<String>,
    pub showing: OutputPattern,
    pub black_outside: bool,
    /// A network screen: shown this much later (+) or earlier (−), ms, on
    /// top of the network's own delay, which is measured.
    pub delay_ms: f32,
    /// A network screen: ask the TV for its game mode, the least delay of its own.
    pub tv_game_mode: bool,
}

/// How a network screen is named as an output's display.
const NETWORK_SUFFIX: &str = " · network";

pub fn network_display(name: &str) -> String {
    format!("{name}{NETWORK_SUFFIX}")
}

/// The network screen's name, if the display is one.
pub fn network_name(display: &str) -> Option<&str> {
    display.strip_suffix(NETWORK_SUFFIX)
}

/// A network screen's hand delay is kept within this, ms.
pub const NETWORK_DELAY_RANGE: (f32, f32) = (-200.0, 300.0);

impl Default for OutputConf {
    fn default() -> Self {
        Self {
            name: "Output 1".into(),
            display: None,
            showing: OutputPattern::Show,
            black_outside: true,
            delay_ms: 0.0,
            tv_game_mode: true,
        }
    }
}

/// Everything about the projection: the outputs, and the surfaces on them.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct OutputSetup {
    pub outputs: Vec<OutputConf>,
    pub surfaces: Vec<Surface>,
    pub snap: bool,
    /// From before there could be several outputs: read once into output 1.
    #[serde(rename = "display", skip_serializing)]
    pub old_display: Option<String>,
    #[serde(rename = "showing", skip_serializing)]
    pub old_showing: Option<OutputPattern>,
    #[serde(rename = "black_outside", skip_serializing)]
    pub old_black_outside: Option<bool>,
}

impl Default for OutputSetup {
    fn default() -> Self {
        Self {
            outputs: vec![OutputConf::default()],
            surfaces: vec![Surface::default()],
            snap: false,
            old_display: None,
            old_showing: None,
            old_black_outside: None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    /// The input device's name; `None` for the best one found.
    pub input_device: Option<String>,
    pub sync_offset_ms: f32,
    pub strobe: bool,
    pub max_flash: f32,
    /// One per theme: whether it is in rotation.
    pub themes: Vec<bool>,
    pub follow_vibe: bool,
    pub variety: f32,
    /// How long the director holds a theme.
    pub theme_length: crate::director::ThemeLength,
    pub quality: Quality,
    pub output: OutputSetup,
    /// The venue in use, by name (kept for settings from before venue files).
    pub last_venue: Option<String>,
    /// The venue file in use: loaded or saved last.
    pub venue_file: Option<PathBuf>,
    /// Venue files opened or saved lately, newest first (also ones kept
    /// outside the library folder).
    pub recent_venues: Vec<PathBuf>,
    /// DMX lights.
    pub lighting: crate::lights::Lighting,
    /// How sensitive the microphone is: the sound is multiplied by this
    /// before the analysis (1 as it comes; more for a quiet room).
    pub input_gain: f32,
}

/// The microphone sensitivity's range.
pub const INPUT_GAIN_RANGE: (f32, f32) = (0.25, 4.0);

impl Default for Settings {
    fn default() -> Self {
        Self {
            input_device: None,
            sync_offset_ms: 0.0,
            strobe: true,
            max_flash: MAX_FLASH,
            themes: vec![true; THEMES.len()],
            follow_vibe: true,
            variety: 0.15,
            theme_length: crate::director::ThemeLength::default(),
            quality: Quality::High,
            output: OutputSetup::default(),
            last_venue: None,
            venue_file: None,
            recent_venues: Vec::new(),
            lighting: crate::lights::Lighting::default(),
            input_gain: 1.0,
        }
    }
}

/// `value` if it is a number, else `fallback`, limited to lo..=hi.
fn limit(value: f32, lo: f32, hi: f32, fallback: f32) -> f32 {
    if value.is_finite() {
        value.clamp(lo, hi)
    } else {
        fallback
    }
}

impl OutputSetup {
    /// 1 to 4 outputs (no display used twice), 1 to 8 surfaces each on an
    /// output that exists, one or two of them Main, every value in range.
    pub fn validated(mut self) -> Self {
        // Settings from before several outputs: their one output is output 1.
        if self.old_display.is_some()
            || self.old_showing.is_some()
            || self.old_black_outside.is_some()
        {
            if self.outputs.len() <= 1 {
                let o = self.outputs.first().cloned().unwrap_or_default();
                self.outputs = vec![OutputConf {
                    display: self.old_display.take().or(o.display),
                    showing: self.old_showing.take().unwrap_or(o.showing),
                    black_outside: self.old_black_outside.take().unwrap_or(o.black_outside),
                    ..o
                }];
            }
            (self.old_display, self.old_showing, self.old_black_outside) = (None, None, None);
        }
        self.outputs.truncate(MAX_OUTPUTS);
        if self.outputs.is_empty() {
            self.outputs.push(OutputConf::default());
        }
        let mut used: Vec<String> = Vec::new();
        for (k, o) in self.outputs.iter_mut().enumerate() {
            if o.name.trim().is_empty() {
                o.name = format!("Output {}", k + 1);
            }
            o.delay_ms = limit(o.delay_ms, NETWORK_DELAY_RANGE.0, NETWORK_DELAY_RANGE.1, 0.0);
            if let Some(d) = &o.display {
                if used.contains(d) {
                    o.display = None;
                } else {
                    used.push(d.clone());
                }
            }
        }
        let outputs = self.outputs.len();
        self.surfaces.truncate(MAX_SURFACES);
        if self.surfaces.is_empty() {
            self.surfaces.push(Surface::default());
        }
        if !self.surfaces.iter().any(|s| s.content == Content::Main) {
            self.surfaces[0].content = Content::Main;
        }
        let mut mains = 0;
        for s in self.surfaces.iter_mut() {
            if s.content == Content::Main {
                mains += 1;
                if mains > MAX_MAINS {
                    s.content = Content::Complement;
                }
            }
            s.output = s.output.min(outputs - 1);
            s.feather_px = limit(s.feather_px, 0.0, 200.0, 0.0);
            s.brightness = limit(s.brightness, 0.0, 1.5, 1.0);
            s.aspect = s
                .aspect
                .filter(|a| a.is_finite() && (0.1..=10.0).contains(a));
            for corner in &mut s.corners {
                for v in corner {
                    *v = limit(*v, -0.5, 1.5, 0.0);
                }
            }
        }
        self
    }

    /// The surfaces on output `k`, with their index among all surfaces.
    pub fn on_output(&self, k: usize) -> impl Iterator<Item = (usize, &Surface)> {
        self.surfaces
            .iter()
            .enumerate()
            .filter(move |(_, s)| s.output == k)
    }
}

impl Settings {
    pub fn validated(mut self) -> Self {
        self.sync_offset_ms = limit(self.sync_offset_ms, -MAX_OFFSET_MS, MAX_OFFSET_MS, 0.0);
        self.max_flash = limit(self.max_flash, 0.0, MAX_FLASH, MAX_FLASH);
        self.variety = limit(self.variety, VARIETY_RANGE.0, VARIETY_RANGE.1, 0.15);
        self.themes.resize(THEMES.len(), true);
        self.output = self.output.validated();
        self.lighting = self.lighting.validated();
        self.input_gain = limit(self.input_gain, INPUT_GAIN_RANGE.0, INPUT_GAIN_RANGE.1, 1.0);
        self
    }
}

/// `~/Library/Application Support/Noctiluca`.
pub fn dir() -> PathBuf {
    let home = std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir);
    let dir = home.join("Library/Application Support/Noctiluca");
    // Settings and venues from before the name: moved over once.
    let old = home.join("Library/Application Support/TechnoViz");
    if !dir.exists() && old.is_dir() {
        let _ = std::fs::rename(&old, &dir);
    }
    dir
}

/// The settings, and a message if something could not be read and was reset.
pub fn load() -> (Settings, Option<String>) {
    load_in(&dir())
}

pub fn load_in(dir: &Path) -> (Settings, Option<String>) {
    let path = dir.join("settings.json");
    let (mut settings, message) = match std::fs::read_to_string(&path) {
        Ok(text) => match serde_json::from_str::<Settings>(&text) {
            Ok(s) => (s, None),
            Err(e) => (
                Settings::default(),
                Some(format!(
                    "settings were unreadable ({e}); using the defaults"
                )),
            ),
        },
        Err(_) => {
            // First run with settings: take the sync offset from before them.
            let old = std::fs::read_to_string(dir.join("sync_offset_ms.txt"))
                .ok()
                .and_then(|s| s.trim().parse::<f32>().ok());
            (
                Settings {
                    sync_offset_ms: old.unwrap_or(0.0),
                    ..Settings::default()
                },
                None,
            )
        }
    };
    settings = settings.validated();
    (settings, message)
}

pub fn save_in(dir: &Path, s: &Settings) -> Result<()> {
    std::fs::create_dir_all(dir).with_context(|| format!("cannot create {}", dir.display()))?;
    let path = dir.join("settings.json");
    // Written next to it and moved in place, so a crash never leaves half a file.
    let tmp = dir.join("settings.json.tmp");
    std::fs::write(&tmp, serde_json::to_string_pretty(s)?)?;
    std::fs::rename(&tmp, &path)?;
    Ok(())
}

/// Writes the settings a moment after they change, not on every slider step.
pub struct Saver {
    dir: PathBuf,
    pending: Option<Settings>,
    due: Option<Instant>,
}

impl Saver {
    pub fn new() -> Self {
        Self::in_dir(dir())
    }

    pub fn in_dir(dir: PathBuf) -> Self {
        Self {
            dir,
            pending: None,
            due: None,
        }
    }

    pub fn changed(&mut self, s: &Settings, now: Instant) {
        if self.pending.as_ref() == Some(s) {
            return;
        }
        self.pending = Some(s.clone());
        self.due.get_or_insert(now + SAVE_AFTER);
    }

    /// Writes if a change is due; true if it wrote.
    pub fn tick(&mut self, now: Instant) -> bool {
        match self.due {
            Some(due) if now >= due => {
                self.due = None;
                if let Some(s) = self.pending.take()
                    && let Err(e) = save_in(&self.dir, &s)
                {
                    eprintln!("could not save the settings: {e:#}");
                }
                true
            }
            _ => false,
        }
    }

    /// Writes whatever is pending now (on quit).
    pub fn flush(&mut self) {
        if self.pending.is_some() {
            self.due = Some(Instant::now());
            self.tick(Instant::now());
        }
    }
}

impl Default for Saver {
    fn default() -> Self {
        Self::new()
    }
}

// Venues are project files: one readable JSON file per place, named
// "<name>_nocvenue.json", kept in ~/Documents/Noctiluca Venues by default
// and openable from anywhere. The file holds every setting of the set-up
// (outputs, surfaces, network screens, lights, safety, sync, themes,
// quality), inside a small header that says what it is.

/// Every venue file's name ends so: "Club Ha'Oman_nocvenue.json".
pub const VENUE_SUFFIX: &str = "_nocvenue.json";
/// What the file's "format" says.
const VENUE_FORMAT: &str = "noctiluca-venue";
/// The file layout's version; files from a newer Noctiluca are refused.
const VENUE_VERSION: u32 = 1;

/// A venue file on disk.
#[derive(Serialize, Deserialize)]
struct VenueFile {
    format: String,
    version: u32,
    name: String,
    /// "Noctiluca 0.1.0".
    saved_by: String,
    /// UTC, "2026-11-22 14:05".
    saved_at: String,
    settings: Settings,
}

/// Where venues are kept unless saved elsewhere: ~/Documents/Noctiluca Venues.
pub fn venue_library() -> PathBuf {
    let home = std::env::var_os("HOME").map(PathBuf::from).unwrap_or_else(std::env::temp_dir);
    home.join("Documents").join("Noctiluca Venues")
}

/// The file is a venue: its name ends in "_nocvenue.json".
pub fn is_venue_file(path: &Path) -> bool {
    path.file_name().and_then(|n| n.to_str()).is_some_and(|n| n.to_lowercase().ends_with(VENUE_SUFFIX) && n.len() > VENUE_SUFFIX.len())
}

/// The venue's name: its file name without "_nocvenue.json".
pub fn venue_name(path: &Path) -> String {
    let file = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    // The suffix is plain ASCII, so its bytes end the name in any case.
    if file.to_lowercase().ends_with(VENUE_SUFFIX) {
        file[..file.len() - VENUE_SUFFIX.len()].to_string()
    } else {
        file.trim_end_matches(".json").to_string()
    }
}

/// The file for venue `name` in folder `dir`, the name made safe for a file.
pub fn venue_path_in(dir: &Path, name: &str) -> PathBuf {
    let safe: String = name.trim().chars().map(|c| if c.is_alphanumeric() || " -_·'&()".contains(c) { c } else { '_' }).collect();
    dir.join(format!("{}{VENUE_SUFFIX}", safe.trim()))
}

/// `path` with "_nocvenue.json" at its end (a save dialog may leave it off).
pub fn with_venue_suffix(path: &Path) -> PathBuf {
    if is_venue_file(path) {
        return path.to_path_buf();
    }
    let name = venue_name(path);
    venue_path_in(path.parent().unwrap_or(Path::new(".")), &name)
}

/// Now in UTC, "2026-11-22 14:05", for the file's header.
fn utc_now() -> String {
    let secs = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0);
    let (days, rem) = (secs.div_euclid(86_400), secs.rem_euclid(86_400));
    // Days to a civil date (Howard Hinnant's algorithm).
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    format!("{y:04}-{m:02}-{d:02} {:02}:{:02}", rem / 3600, rem % 3600 / 60)
}

impl Settings {
    /// What a venue keeps of these settings: everything (outputs, surfaces,
    /// lights, flash safety, sync, themes, quality) but the input device,
    /// which belongs to the computer, and the app's own memory of files.
    pub fn venue_preset(&self) -> Settings {
        Settings {
            input_device: None,
            last_venue: None,
            venue_file: None,
            recent_venues: Vec::new(),
            ..self.clone()
        }
        .validated()
    }

    /// One line about what a venue holds: "2 outputs · 4 surfaces · 2 lights".
    pub fn venue_summary(&self) -> String {
        let n = |k: usize, one: &str, many: &str| format!("{k} {}", if k == 1 { one } else { many });
        let lights = self.lighting.fixtures.iter().filter(|f| !f.is_smoke()).count();
        let smoke = self.lighting.fixtures.iter().any(|f| f.is_smoke());
        let mut parts = vec![n(self.output.outputs.len(), "output", "outputs"), n(self.output.surfaces.len(), "surface", "surfaces"), n(lights, "light", "lights")];
        if smoke {
            parts.push("smoke".into());
        }
        parts.join(" · ")
    }

    /// These settings with a venue loaded over them: all of it except the
    /// input device and the list of recent files, which stay this
    /// computer's. `path` becomes the venue in use.
    pub fn with_venue(&self, venue: Settings, path: &Path) -> Settings {
        let mut s = Settings {
            input_device: self.input_device.clone(),
            recent_venues: self.recent_venues.clone(),
            ..venue
        };
        s.set_venue_file(Some(path.to_path_buf()));
        s
    }

    /// Marks `path` as the venue in use (and the most recent one).
    pub fn set_venue_file(&mut self, path: Option<PathBuf>) {
        self.last_venue = path.as_deref().map(venue_name);
        if let Some(p) = &path {
            self.recent_venues.retain(|r| r != p);
            self.recent_venues.insert(0, p.clone());
            self.recent_venues.truncate(MAX_RECENT_VENUES);
        }
        self.venue_file = path;
    }
}

/// How many venue files opened from anywhere are remembered.
pub const MAX_RECENT_VENUES: usize = 12;

/// Writes the set-up to the venue file `path` ("_nocvenue.json" added if
/// missing), never leaving half a file; returns where it went.
pub fn save_venue_file(path: &Path, settings: &Settings) -> Result<PathBuf> {
    let path = with_venue_suffix(path);
    let name = venue_name(&path);
    if name.trim().is_empty() {
        anyhow::bail!("a venue needs a name");
    }
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).with_context(|| format!("cannot create {}", parent.display()))?;
    }
    let file = VenueFile {
        format: VENUE_FORMAT.into(),
        version: VENUE_VERSION,
        name,
        saved_by: format!("Noctiluca {}", env!("CARGO_PKG_VERSION")),
        saved_at: utc_now(),
        settings: settings.venue_preset(),
    };
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, serde_json::to_string_pretty(&file)? + "\n").with_context(|| format!("cannot write {}", tmp.display()))?;
    std::fs::rename(&tmp, &path).with_context(|| format!("cannot write {}", path.display()))?;
    Ok(path)
}

/// Reads a venue file: one saved by Noctiluca (any name), or a plain
/// settings file from before venue files had a header.
pub fn load_venue_file(path: &Path) -> Result<Settings> {
    let text = std::fs::read_to_string(path).with_context(|| format!("cannot read {}", path.display()))?;
    let value: serde_json::Value = serde_json::from_str(&text).with_context(|| format!("{} is not valid JSON", path.display()))?;
    let settings = match value.get("format").and_then(|f| f.as_str()) {
        Some(VENUE_FORMAT) => {
            let version = value.get("version").and_then(|v| v.as_u64()).unwrap_or(0);
            if version > u64::from(VENUE_VERSION) {
                anyhow::bail!("it was saved by a newer Noctiluca; update the app to open it");
            }
            serde_json::from_value::<Settings>(value.get("settings").cloned().unwrap_or_default()).context("its settings are not readable")?
        }
        Some(other) => anyhow::bail!("it is a “{other}” file, not a Noctiluca venue"),
        None if value.get("output").is_some() || value.get("lighting").is_some() => serde_json::from_value::<Settings>(value)?,
        None => anyhow::bail!("it is not a Noctiluca venue"),
    };
    Ok(settings.validated())
}

/// The venue files in `dir`, by name.
pub fn venues_in(dir: &Path) -> Vec<PathBuf> {
    let mut found: Vec<PathBuf> = std::fs::read_dir(dir).into_iter().flatten().flatten().map(|e| e.path()).filter(|p| is_venue_file(p) && p.is_file()).collect();
    found.sort_by_key(|p| venue_name(p).to_lowercase());
    found
}

/// Gives a venue file a new name in the same folder; refuses to replace another.
pub fn rename_venue_file(path: &Path, to: &str) -> Result<PathBuf> {
    if to.trim().is_empty() {
        anyhow::bail!("a venue needs a name");
    }
    let dest = venue_path_in(path.parent().unwrap_or(Path::new(".")), to);
    if dest == path {
        return Ok(dest);
    }
    if dest.exists() {
        anyhow::bail!("there is a venue called “{}” there already", to.trim());
    }
    std::fs::rename(path, &dest).with_context(|| format!("cannot rename “{}”", venue_name(path)))?;
    Ok(dest)
}

/// Moves a venue file to the Trash (it can be put back from there).
pub fn trash_venue_file(path: &Path) -> Result<()> {
    let home = std::env::var_os("HOME").map(PathBuf::from).context("no home folder")?;
    trash_in(&home.join(".Trash"), path)
}

fn trash_in(trash: &Path, path: &Path) -> Result<()> {
    std::fs::create_dir_all(trash)?;
    let file = path.file_name().context("not a file")?.to_string_lossy().into_owned();
    let mut dest = trash.join(&file);
    let mut n = 2;
    while dest.exists() {
        dest = trash.join(format!("{} {n}{VENUE_SUFFIX}", venue_name(path)));
        n += 1;
    }
    std::fs::rename(path, &dest).with_context(|| format!("cannot move “{}” to the Trash", venue_name(path)))
}

/// Venues from before venue files (Application Support/Noctiluca/venues/
/// <name>.json) become files in the library, once. Returns how many.
pub fn migrate_old_venues(app_dir: &Path, library: &Path) -> usize {
    let old = app_dir.join("venues");
    let mut moved = 0;
    for entry in std::fs::read_dir(&old).into_iter().flatten().flatten() {
        let p = entry.path();
        if p.extension().is_none_or(|e| e != "json") {
            continue;
        }
        let name = p.file_stem().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        let dest = venue_path_in(library, &name);
        if dest.exists() {
            continue;
        }
        if let Ok(s) = load_venue_file(&p)
            && save_venue_file(&dest, &s).is_ok()
            && std::fs::remove_file(&p).is_ok()
        {
            moved += 1;
        }
    }
    let _ = std::fs::remove_dir(&old);
    moved
}

impl Settings {
    /// After a load: an active venue named the old way (by name only)
    /// pointed at its file in the library.
    pub fn adopt_venue_files(&mut self, library: &Path) {
        if self.venue_file.is_none()
            && let Some(name) = self.last_venue.clone()
        {
            let p = venue_path_in(library, &name);
            if p.exists() {
                self.set_venue_file(Some(p));
            }
        }
        self.recent_venues.retain(|p| p.exists());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn theme_length_is_kept_and_old_files_start_on_medium() {
        use crate::director::ThemeLength;
        let old: Settings = serde_json::from_str("{}").unwrap();
        assert_eq!(old.theme_length, ThemeLength::Medium);
        let short = Settings { theme_length: ThemeLength::Short, ..Settings::default() };
        let text = serde_json::to_string(&short).unwrap();
        assert!(text.contains(r#""theme_length":"short""#));
        assert_eq!(serde_json::from_str::<Settings>(&text).unwrap().theme_length, ThemeLength::Short);
    }

    /// A fresh, empty folder for one test.
    fn scratch(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("noctiluca-settings-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn round_trip_keeps_everything() {
        let dir = scratch("round");
        let mut s = Settings {
            strobe: false,
            sync_offset_ms: 40.0,
            quality: Quality::Light,
            ..Settings::default()
        };
        s.output.outputs[0].display = Some("Epson".into());
        s.output.surfaces.push(Surface {
            name: "Left".into(),
            content: Content::Complement,
            style: ComplementStyle::Echo,
            ..Surface::default()
        });
        s.themes[2] = false;
        save_in(&dir, &s).unwrap();
        assert_eq!(load_in(&dir), (s, None));
    }

    #[test]
    fn broken_file_gives_defaults() {
        let dir = scratch("broken");
        std::fs::write(dir.join("settings.json"), "{ this is not json").unwrap();
        let (s, message) = load_in(&dir);
        assert_eq!(s, Settings::default());
        assert!(message.is_some());
    }

    #[test]
    fn missing_fields_take_defaults() {
        let dir = scratch("missing");
        std::fs::write(
            dir.join("settings.json"),
            r#"{"strobe": false, "output": {"snap": true}}"#,
        )
        .unwrap();
        let (s, message) = load_in(&dir);
        assert!(message.is_none());
        assert!(!s.strobe && s.output.snap);
        assert_eq!(s.output.surfaces.len(), 1);
        assert_eq!(s.themes.len(), THEMES.len());
        assert_eq!(s.variety, 0.15);
    }

    #[test]
    fn old_sync_offset_is_migrated() {
        let dir = scratch("migrate");
        std::fs::write(dir.join("sync_offset_ms.txt"), "40\n").unwrap();
        assert_eq!(load_in(&dir).0.sync_offset_ms, 40.0);
    }

    #[test]
    fn one_or_two_mains() {
        let main = Surface::default();
        let comp = Surface {
            content: Content::Complement,
            ..Surface::default()
        };
        let mains = |surfaces: Vec<Surface>| {
            OutputSetup {
                surfaces,
                ..OutputSetup::default()
            }
            .validated()
            .surfaces
            .iter()
            .filter(|s| s.content == Content::Main)
            .count()
        };
        assert_eq!(mains(vec![comp.clone(), comp.clone()]), 1);
        assert_eq!(mains(vec![main.clone(), main.clone(), main.clone()]), 2);
        assert_eq!(mains(vec![comp.clone(), main.clone()]), 1);
        assert_eq!(mains(vec![]), 1);
        assert_eq!(
            OutputSetup {
                surfaces: vec![comp.clone(); 11],
                ..OutputSetup::default()
            }
            .validated()
            .surfaces
            .len(),
            MAX_SURFACES
        );
    }

    #[test]
    fn values_are_clamped() {
        let mut s = Settings {
            sync_offset_ms: 9000.0,
            max_flash: f32::NAN,
            variety: 2.0,
            ..Settings::default()
        };
        s.output.surfaces[0].brightness = -1.0;
        s.output.surfaces[0].corners[0] = [f32::INFINITY, 7.0];
        let s = s.validated();
        assert_eq!(s.sync_offset_ms, 300.0);
        assert_eq!(s.max_flash, 0.6);
        assert_eq!(s.variety, 0.5);
        assert_eq!(s.output.surfaces[0].brightness, 0.0);
        assert_eq!(s.output.surfaces[0].corners[0], [0.0, 1.5]);
    }

    #[test]
    fn saver_writes_at_most_twice_a_second() {
        let dir = scratch("saver");
        let mut saver = Saver::in_dir(dir.clone());
        let t0 = Instant::now();
        let mut s = Settings::default();
        let mut writes = 0;
        // A slider dragged for a second, a change every frame.
        for i in 0..60 {
            s.variety = 0.05 + i as f32 * 0.005;
            let now = t0 + Duration::from_millis(i * 16);
            saver.changed(&s, now);
            writes += saver.tick(now) as usize;
        }
        writes += saver.tick(t0 + Duration::from_secs(2)) as usize;
        assert!((1..=3).contains(&writes), "{writes} writes");
        assert_eq!(
            load_in(&dir).0.variety,
            s.variety,
            "the last value is the one kept"
        );
    }

    #[test]
    fn a_venue_file_round_trips_everything_but_the_input() {
        let dir = scratch("venue");
        let mut s = Settings { input_device: Some("Mic".into()), sync_offset_ms: 40.0, strobe: false, variety: 0.4, ..Settings::default() };
        s.output.outputs[0].display = Some(network_display("living-room"));
        s.output.outputs[0].delay_ms = 25.0;
        s.output.snap = true;
        s.themes[1] = false;
        s.output.surfaces[0].corners = [[0.1, 0.1], [0.9, 0.12], [0.88, 0.9], [0.12, 0.88]];
        s.lighting.fixtures = vec![
            crate::lights::Fixture { name: "Spot 1".into(), address: 30, ..Default::default() },
            crate::lights::Fixture { name: "Smoke".into(), address: 50, profile: crate::lights::Profile::Smoke, ..Default::default() },
        ];
        s.lighting.drive = 0.9;
        s.lighting.effect = Some(crate::lights::Effect::Wave);
        s.recent_venues = vec![PathBuf::from("/elsewhere/a_nocvenue.json")];
        // Saved without the suffix: it is added.
        let path = save_venue_file(&dir.join("Barby · 3 screens"), &s).unwrap();
        assert_eq!(path.file_name().unwrap(), "Barby · 3 screens_nocvenue.json");
        assert_eq!(venues_in(&dir), vec![path.clone()]);
        assert_eq!(venue_name(&path), "Barby · 3 screens");
        // One readable file, with a header saying what it is.
        let text = std::fs::read_to_string(&path).unwrap();
        let v: serde_json::Value = serde_json::from_str(&text).unwrap();
        assert_eq!((v["format"].as_str(), v["version"].as_u64(), v["name"].as_str()), (Some("noctiluca-venue"), Some(1), Some("Barby · 3 screens")));
        assert!(text.contains("\n  \"settings\": {\n"), "pretty, one key a line");
        assert!(v["settings"]["input_device"].is_null() && v["settings"]["recent_venues"].as_array().unwrap().is_empty());
        let back = load_venue_file(&path).unwrap();
        assert_eq!(back, s.venue_preset(), "everything but the input comes back");
        assert_eq!(back.lighting.fixtures[0].address, 30);
        assert_eq!(s.venue_summary(), "1 output · 1 surface · 1 light · smoke");
        // Loaded here: this computer's input and recent files stay; the file is in use.
        let here = Settings { input_device: Some("Other mic".into()), ..Settings::default() }.with_venue(back, &path);
        assert_eq!(here.input_device.as_deref(), Some("Other mic"));
        assert_eq!((here.venue_file.as_deref(), here.last_venue.as_deref()), (Some(path.as_path()), Some("Barby · 3 screens")));
        assert_eq!(here.recent_venues, vec![path.clone()]);
        assert_eq!(here.venue_preset(), s.venue_preset());
        assert!(load_venue_file(&dir.join("nowhere_nocvenue.json")).is_err());
        assert!(save_venue_file(&dir.join(VENUE_SUFFIX), &s).is_err(), "no name");
    }

    #[test]
    fn venue_files_are_told_apart_and_old_ones_still_open() {
        let dir = scratch("venue-kinds");
        assert!(is_venue_file(Path::new("/x/Club_nocvenue.json")) && is_venue_file(Path::new("Club_NOCVENUE.JSON")));
        assert!(!is_venue_file(Path::new("/x/Club.json")) && !is_venue_file(Path::new("/x/_nocvenue.json")));
        // A plain settings file (a venue from before venue files) opens.
        let old = dir.join("old.json");
        std::fs::write(&old, serde_json::to_string(&Settings { strobe: false, ..Settings::default() }).unwrap()).unwrap();
        assert!(!load_venue_file(&old).unwrap().strobe);
        // Anything else, or a newer format, is refused with a reason.
        let other = dir.join("other_nocvenue.json");
        std::fs::write(&other, r#"{"hello": 1}"#).unwrap();
        assert!(format!("{:#}", load_venue_file(&other).unwrap_err()).contains("not a Noctiluca venue"));
        std::fs::write(&other, r#"{"format": "noctiluca-venue", "version": 99, "settings": {}}"#).unwrap();
        assert!(format!("{:#}", load_venue_file(&other).unwrap_err()).contains("newer Noctiluca"));
        std::fs::write(&other, "not json").unwrap();
        assert!(load_venue_file(&other).is_err());
    }

    #[test]
    fn venue_files_rename_trash_and_migrate() {
        let dir = scratch("venue-files");
        let home = save_venue_file(&venue_path_in(&dir, "בית"), &Settings::default()).unwrap();
        let club = save_venue_file(&venue_path_in(&dir, "Club"), &Settings::default()).unwrap();
        // Rename: not over another; the file keeps its suffix.
        assert!(rename_venue_file(&home, "Club").is_err());
        let home = rename_venue_file(&home, "Home").unwrap();
        assert_eq!(home.file_name().unwrap(), "Home_nocvenue.json");
        assert_eq!(venues_in(&dir), vec![club.clone(), home.clone()]);
        // Delete goes to a trash folder, twice under one name too.
        let trash = dir.join("Trash");
        trash_in(&trash, &club).unwrap();
        let again = save_venue_file(&venue_path_in(&dir, "Club"), &Settings::default()).unwrap();
        trash_in(&trash, &again).unwrap();
        assert_eq!(venues_in(&dir), vec![home]);
        assert_eq!(std::fs::read_dir(&trash).unwrap().count(), 2);
        // Old venues (app folder, "<name>.json") move to the library once.
        let app = dir.join("app");
        std::fs::create_dir_all(app.join("venues")).unwrap();
        std::fs::write(app.join("venues/Barby.json"), serde_json::to_string(&Settings { variety: 0.3, ..Settings::default() }).unwrap()).unwrap();
        let lib = dir.join("lib");
        assert_eq!(migrate_old_venues(&app, &lib), 1);
        let moved = venue_path_in(&lib, "Barby");
        assert_eq!(load_venue_file(&moved).unwrap().variety, 0.3);
        assert!(!app.join("venues").exists());
        // Settings that named it the old way now point at its file.
        let mut s = Settings { last_venue: Some("Barby".into()), recent_venues: vec![dir.join("gone_nocvenue.json")], ..Settings::default() };
        s.adopt_venue_files(&lib);
        assert_eq!(s.venue_file.as_deref(), Some(moved.as_path()));
        assert_eq!(s.recent_venues, vec![moved]);
    }

    #[test]
    fn old_single_output_settings_move_into_output_one() {
        let dir = scratch("old-output");
        std::fs::write(
            dir.join("settings.json"),
            r#"{"output": {"display": "Monitor #3641", "showing": "Grid", "black_outside": false,
                "surfaces": [{"name": "Left"}, {"name": "Centre", "content": "Main"}]}}"#,
        )
        .unwrap();
        let (s, message) = load_in(&dir);
        assert!(message.is_none());
        assert_eq!(s.output.outputs.len(), 1);
        let o = &s.output.outputs[0];
        assert_eq!(o.display.as_deref(), Some("Monitor #3641"));
        assert_eq!(o.showing, OutputPattern::Grid);
        assert!(!o.black_outside);
        assert!(s.output.surfaces.iter().all(|x| x.output == 0 && x.visible));
        // And saved again, the old fields are gone.
        save_in(&dir, &s).unwrap();
        let text = std::fs::read_to_string(dir.join("settings.json")).unwrap();
        assert!(
            !text.contains("\"showing\": \"Grid\",\n    \"black_outside\": false,\n    \"snap\""),
            "{text}"
        );
        assert_eq!(load_in(&dir).0, s);
    }

    #[test]
    fn eight_surfaces_and_four_outputs_at_most_each_on_one_that_exists() {
        let mut setup = OutputSetup {
            surfaces: vec![
                Surface {
                    output: 7,
                    ..Surface::default()
                };
                12
            ],
            ..OutputSetup::default()
        };
        setup.outputs = vec![OutputConf::default(); 6];
        let v = setup.validated();
        assert_eq!(v.surfaces.len(), 8);
        assert_eq!(v.outputs.len(), 4);
        assert!(v.surfaces.iter().all(|s| s.output == 3));
        assert_eq!(
            v.surfaces
                .iter()
                .filter(|s| s.content == Content::Main)
                .count(),
            2
        );
        assert_eq!(v.outputs[2].name, "Output 1", "names are kept as given");
    }

    #[test]
    fn a_display_belongs_to_one_output() {
        let setup = OutputSetup {
            outputs: vec![
                OutputConf {
                    display: Some("XG2705".into()),
                    ..OutputConf::default()
                },
                OutputConf {
                    name: String::new(),
                    display: Some("XG2705".into()),
                    ..OutputConf::default()
                },
            ],
            ..OutputSetup::default()
        };
        let v = setup.validated();
        assert_eq!(v.outputs[0].display.as_deref(), Some("XG2705"));
        assert_eq!(v.outputs[1].display, None);
        assert_eq!(v.outputs[1].name, "Output 2");
    }
}
