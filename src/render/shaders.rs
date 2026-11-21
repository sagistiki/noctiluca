//! Loads WGSL from disk, glues the pieces together, checks them, and watches
//! the folder so edits show up while the app is running.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use anyhow::{Context, Result};
use notify::{RecursiveMode, Watcher};
use wgpu::naga;

fn read(dir: &Path, name: &str) -> Result<String> {
    let path = dir.join(name);
    std::fs::read_to_string(&path).with_context(|| format!("cannot read {}", path.display()))
}

/// Several .wgsl files glued into one shader, remembering which lines came
/// from which file so an error can point at the file you actually edited.
pub struct Assembled {
    pub source: String,
    /// (file name, number of lines), in order.
    parts: Vec<(String, usize)>,
}

impl Assembled {
    fn from_files(dir: &Path, names: &[String]) -> Result<Self> {
        let mut source = String::new();
        let mut parts = Vec::new();
        for name in names {
            let mut text = read(dir, name)?;
            if !text.ends_with('\n') {
                text.push('\n');
            }
            parts.push((name.clone(), text.lines().count()));
            source.push_str(&text);
        }
        Ok(Self { source, parts })
    }

    /// Which file a line of the glued source came from, and its line number there.
    pub fn locate(&self, line: usize) -> Option<(&str, usize)> {
        let mut first = 1;
        for (name, count) in &self.parts {
            if (first..first + count).contains(&line) {
                return Some((name, line - first + 1));
            }
            first += count;
        }
        None
    }

    /// Checks the shader; an error names the original file and line.
    pub fn validate(&self) -> Result<(), String> {
        validate(&self.source).map_err(|message| {
            // The compiler reports positions as "wgsl:LINE:COLUMN".
            let place = message
                .split("wgsl:")
                .nth(1)
                .and_then(|rest| rest.split(':').next())
                .and_then(|line| line.trim().parse().ok())
                .and_then(|line| self.locate(line));
            match place {
                Some((file, line)) => format!("{file}, line {line}:\n{message}"),
                None => message,
            }
        })
    }
}

/// A scene shader: common + one background + one pattern + scene.
pub fn assemble(dir: &Path, bg: &str, pat: &str) -> Result<Assembled> {
    Assembled::from_files(
        dir,
        &[
            "common.wgsl".into(),
            format!("bg_{bg}.wgsl"),
            format!("pat_{pat}.wgsl"),
            "scene.wgsl".into(),
        ],
    )
}

/// An atmosphere pass: common + atmos_NAME + atmos.
pub fn assemble_atmos(dir: &Path, name: &str) -> Result<Assembled> {
    Assembled::from_files(
        dir,
        &[
            "common.wgsl".into(),
            format!("atmos_{name}.wgsl"),
            "atmos.wgsl".into(),
        ],
    )
}

/// A shader that stands alone, without common.wgsl (the compositor).
pub fn assemble_alone(dir: &Path, name: &str) -> Result<Assembled> {
    Assembled::from_files(dir, &[format!("{name}.wgsl")])
}

/// A full-screen pass that is not a scene: common + the file.
pub fn assemble_pass(dir: &Path, name: &str) -> Result<Assembled> {
    Assembled::from_files(dir, &["common.wgsl".into(), format!("{name}.wgsl")])
}

/// Parses and type-checks WGSL. Handing a broken shader to the GPU would
/// abort the program; checking first lets us keep the old one instead.
pub fn validate(src: &str) -> Result<(), String> {
    let module = naga::front::wgsl::parse_str(src).map_err(|e| e.emit_to_string(src))?;
    naga::valid::Validator::new(
        naga::valid::ValidationFlags::all(),
        naga::valid::Capabilities::empty(),
    )
    .validate(&module)
    .map_err(|e| e.emit_to_string(src))?;
    Ok(())
}

/// Raises a flag whenever a file in the shader folder changes.
pub struct ShaderWatcher {
    dirty: Arc<AtomicBool>,
    _watcher: Option<notify::RecommendedWatcher>,
}

impl ShaderWatcher {
    pub fn new(dir: &Path) -> Self {
        let dirty = Arc::new(AtomicBool::new(false));
        let flag = dirty.clone();
        // The callback runs on the watcher's own thread. An atomic flag is
        // the simplest thing two threads can share safely.
        let watcher = notify::recommended_watcher(move |event: notify::Result<notify::Event>| {
            if event.is_ok_and(|e| e.kind.is_modify() || e.kind.is_create()) {
                flag.store(true, Ordering::Relaxed);
            }
        })
        .and_then(|mut w| w.watch(dir, RecursiveMode::NonRecursive).map(|_| w));
        if let Err(e) = &watcher {
            eprintln!("shader hot reload is off: {e}");
        }
        Self {
            dirty,
            _watcher: watcher.ok(),
        }
    }

    /// True once after each change.
    pub fn take_dirty(&self) -> bool {
        self.dirty.swap(false, Ordering::Relaxed)
    }
}

/// The shader folder: inside the app when packed as one, else the project's.
pub fn default_dir() -> PathBuf {
    super::resources::find("shaders", "shaders")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_combination_validates() {
        use crate::director::{BACKGROUNDS, PATTERNS};
        for bg in BACKGROUNDS {
            for pattern in PATTERNS {
                let shader = assemble(&default_dir(), bg, pattern).unwrap();
                shader
                    .validate()
                    .unwrap_or_else(|e| panic!("{bg} + {pattern}: {e}"));
            }
        }
    }

    #[test]
    fn every_atmosphere_validates() {
        for name in crate::director::ATMOSPHERES.iter().skip(1) {
            let shader = assemble_atmos(&default_dir(), name).unwrap();
            shader.validate().unwrap_or_else(|e| panic!("{name}: {e}"));
        }
    }

    #[test]
    fn every_pass_validates() {
        for name in ["post", "glow", "present"] {
            let shader = assemble_pass(&default_dir(), name).unwrap();
            shader.validate().unwrap_or_else(|e| panic!("{name}: {e}"));
        }
    }

    /// A private copy of the shader folder that a test may break.
    fn scratch_copy(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("noctiluca-{tag}-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        for entry in std::fs::read_dir(default_dir()).unwrap() {
            let path = entry.unwrap().path();
            std::fs::copy(&path, dir.join(path.file_name().unwrap())).unwrap();
        }
        dir
    }

    #[test]
    fn error_names_the_file_and_its_own_line() {
        let dir = scratch_copy("error");
        let broken =
            "// a comment\n\nfn pattern(p: vec2f, base: vec3f) -> vec3f {\n    return oops;\n}\n";
        std::fs::write(dir.join("pat_rings.wgsl"), broken).unwrap();
        let message = assemble(&dir, "tunnel", "rings")
            .unwrap()
            .validate()
            .unwrap_err();
        std::fs::remove_dir_all(&dir).unwrap();
        assert!(message.starts_with("pat_rings.wgsl, line 4:"), "{message}");
    }

    #[test]
    fn locate_maps_every_line_to_its_file() {
        let shader = assemble(&default_dir(), "tunnel", "rings").unwrap();
        assert_eq!(shader.locate(1), Some(("common.wgsl", 1)));
        let total = shader.source.lines().count();
        let (file, line) = shader.locate(total).unwrap();
        assert_eq!(file, "scene.wgsl");
        let scene_lines = std::fs::read_to_string(default_dir().join("scene.wgsl"))
            .unwrap()
            .lines()
            .count();
        assert_eq!(line, scene_lines);
        assert_eq!(shader.locate(total + 1), None);
        assert_eq!(shader.locate(0), None);
    }

    #[test]
    fn broken_shader_reports_error() {
        assert!(validate("fn x( {").is_err());
    }

    #[test]
    fn type_error_reports_error() {
        // Parses fine, but adds a float to a vector of the wrong size.
        let src = "fn f() -> vec2f { return vec3f(1.0) + vec2f(1.0); }";
        assert!(validate(src).is_err());
    }

    #[test]
    fn missing_file_is_an_error_not_a_panic() {
        assert!(assemble(&default_dir(), "does_not_exist", "rings").is_err());
    }
}
