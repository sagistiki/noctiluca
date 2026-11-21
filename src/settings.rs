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
    pub quality: Quality,
    pub output: OutputSetup,
    /// The venue loaded or saved last.
    pub last_venue: Option<String>,
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
            quality: Quality::High,
            output: OutputSetup::default(),
            last_venue: None,
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

/// A venue name made safe as a file name.
fn venue_file(dir: &Path, name: &str) -> PathBuf {
    let safe: String = name
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || " -_·".contains(c) {
                c
            } else {
                '_'
            }
        })
        .collect();
    dir.join("venues").join(format!("{}.json", safe.trim()))
}

impl Settings {
    /// What a venue keeps of these settings: everything (outputs, surfaces,
    /// lights, flash safety, sync, themes, quality) but the input device,
    /// which belongs to the computer, and which venue is active.
    pub fn venue_preset(&self) -> Settings {
        Settings {
            input_device: None,
            last_venue: None,
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
}

pub fn save_venue_in(dir: &Path, name: &str, settings: &Settings) -> Result<()> {
    let name = name.trim();
    if name.is_empty() {
        anyhow::bail!("a venue needs a name");
    }
    let path = venue_file(dir, name);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    // Written beside it first and then moved over it: never half a file.
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, serde_json::to_string_pretty(&settings.venue_preset())?)
        .with_context(|| format!("cannot write {}", tmp.display()))?;
    std::fs::rename(&tmp, &path).with_context(|| format!("cannot write {}", path.display()))
}

/// A venue by this name is saved already.
pub fn venue_exists_in(dir: &Path, name: &str) -> bool {
    venue_file(dir, name.trim()).exists()
}

pub fn delete_venue_in(dir: &Path, name: &str) -> Result<()> {
    std::fs::remove_file(venue_file(dir, name)).with_context(|| format!("cannot delete “{name}”"))
}

/// Renames a venue; refuses to replace another.
pub fn rename_venue_in(dir: &Path, from: &str, to: &str) -> Result<()> {
    let to = to.trim();
    if to.is_empty() {
        anyhow::bail!("a venue needs a name");
    }
    let (a, b) = (venue_file(dir, from), venue_file(dir, to));
    if a == b {
        return Ok(());
    }
    if b.exists() {
        anyhow::bail!("there is a venue called “{to}” already");
    }
    std::fs::rename(&a, &b).with_context(|| format!("cannot rename “{from}”"))
}

pub fn load_venue_in(dir: &Path, name: &str) -> Result<Settings> {
    let path = venue_file(dir, name);
    let text = std::fs::read_to_string(&path)
        .with_context(|| format!("cannot read {}", path.display()))?;
    Ok(serde_json::from_str::<Settings>(&text)?.validated())
}

impl Settings {
    /// These settings with venue `name`'s preset loaded over them: all of
    /// it except the input device, which belongs to the computer.
    pub fn with_venue(&self, venue: Settings, name: &str) -> Settings {
        Settings {
            input_device: self.input_device.clone(),
            last_venue: Some(name.to_string()),
            ..venue
        }
    }
}

/// The saved venues' names, sorted.
pub fn venues_in(dir: &Path) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(dir.join("venues"))
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|e| {
            let p = e.path();
            if p.extension()? != "json" {
                return None;
            }
            Some(p.file_stem()?.to_string_lossy().into_owned())
        })
        .collect();
    names.sort();
    names
}

pub fn save_venue(name: &str, settings: &Settings) -> Result<()> {
    save_venue_in(&dir(), name, settings)
}

pub fn load_venue(name: &str) -> Result<Settings> {
    load_venue_in(&dir(), name)
}

pub fn venues() -> Vec<String> {
    venues_in(&dir())
}

pub fn venue_exists(name: &str) -> bool {
    venue_exists_in(&dir(), name)
}

pub fn delete_venue(name: &str) -> Result<()> {
    delete_venue_in(&dir(), name)
}

pub fn rename_venue(from: &str, to: &str) -> Result<()> {
    rename_venue_in(&dir(), from, to)
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn venue_round_trip() {
        let dir = scratch("venue");
        let mut preset = Settings {
            strobe: false,
            variety: 0.4,
            input_device: Some("Mic A".into()),
            ..Settings::default()
        };
        preset.output.outputs[0].display = Some("Projector".into());
        preset.output.snap = true;
        preset.themes[1] = false;
        save_venue_in(&dir, "Barby · 3 screens", &preset).unwrap();
        assert_eq!(venues_in(&dir), vec!["Barby · 3 screens".to_string()]);
        let loaded = load_venue_in(&dir, "Barby · 3 screens").unwrap();
        // Loaded over other settings: everything comes from the preset but
        // the input device, which stays this computer's.
        let here = Settings {
            input_device: Some("Mic B".into()),
            ..Settings::default()
        };
        let now = here.with_venue(loaded, "Barby · 3 screens");
        assert_eq!(now.input_device.as_deref(), Some("Mic B"));
        assert_eq!(now.last_venue.as_deref(), Some("Barby · 3 screens"));
        assert!(!now.strobe && now.variety == 0.4 && now.output.snap && !now.themes[1]);
        assert_eq!(now.output.outputs[0].display.as_deref(), Some("Projector"));
        assert!(load_venue_in(&dir, "nowhere").is_err());
    }

    #[test]
    fn a_venue_keeps_everything_but_the_input_and_can_be_renamed_and_deleted() {
        let dir = scratch("venue-all");
        let mut s = Settings { input_device: Some("Mic".into()), sync_offset_ms: 40.0, strobe: false, ..Settings::default() };
        s.output.outputs[0].display = Some(network_display("living-room"));
        s.output.outputs[0].delay_ms = 25.0;
        s.output.surfaces[0].corners = [[0.1, 0.1], [0.9, 0.12], [0.88, 0.9], [0.12, 0.88]];
        s.lighting.fixtures = vec![
            crate::lights::Fixture { name: "Spot 1".into(), address: 30, ..Default::default() },
            crate::lights::Fixture { name: "Smoke".into(), address: 50, profile: crate::lights::Profile::Smoke, ..Default::default() },
        ];
        s.lighting.drive = 0.9;
        s.lighting.effect = Some(crate::lights::Effect::Wave);
        s.lighting.strobe = crate::lights::LightStrobe::Free;
        save_venue_in(&dir, "בית", &s).unwrap();
        assert!(venue_exists_in(&dir, "בית"));
        let back = load_venue_in(&dir, "בית").unwrap();
        assert_eq!(back, s.venue_preset(), "everything but the input comes back");
        assert_eq!(back.input_device, None);
        assert_eq!(back.lighting.fixtures[0].address, 30);
        assert_eq!(s.venue_summary(), "1 output · 1 surface · 1 light · smoke");
        // Loading keeps this computer's input and marks the venue active.
        let here = Settings { input_device: Some("Other mic".into()), ..Settings::default() }.with_venue(back, "בית");
        assert_eq!((here.input_device.as_deref(), here.last_venue.as_deref()), (Some("Other mic"), Some("בית")));
        assert_eq!(here.venue_preset(), s.venue_preset());
        // Rename (not over another), delete.
        save_venue_in(&dir, "Club", &Settings::default()).unwrap();
        assert!(rename_venue_in(&dir, "בית", "Club").is_err());
        rename_venue_in(&dir, "בית", "Home").unwrap();
        assert_eq!(venues_in(&dir), vec!["Club".to_string(), "Home".to_string()]);
        delete_venue_in(&dir, "Club").unwrap();
        assert_eq!(venues_in(&dir), vec!["Home".to_string()]);
        assert!(save_venue_in(&dir, "  ", &s).is_err());
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
