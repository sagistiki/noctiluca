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
    /// The vertex shader's entry point.
    pub vertex: &'static str,
    /// (file name, number of lines), in order.
    parts: Vec<(String, usize)>,
}

impl Assembled {
    fn from_files(dir: &Path, names: &[String]) -> Result<Self> {
        let texts = names.iter().map(|n| Ok((n.clone(), read(dir, n)?))).collect::<Result<Vec<_>>>()?;
        Ok(Self::from_texts(texts))
    }

    /// Pieces of source, each under the name an error should point at.
    fn from_texts(texts: Vec<(String, String)>) -> Self {
        let mut source = String::new();
        let mut parts = Vec::new();
        for (name, mut text) in texts {
            if !text.ends_with('\n') {
                text.push('\n');
            }
            parts.push((name, text.lines().count()));
            source.push_str(&text);
        }
        Self { source, vertex: "vs_main", parts }
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

/// The scene shader: every background and every pattern in one program,
/// which draws the pair a draw call asks for (its instance number is
/// `background * SCENE_SLOTS + pattern`; see scene.wgsl). One program,
/// however many looks there are, instead of one per pair.
///
/// Each bg_NAME.wgsl defines `background(p)` and each pat_NAME.wgsl
/// `pattern(p, base)`; they are renamed `background_K` and `pattern_K` here.
/// Any other function or constant in those files needs a name of its own.
pub fn assemble_scene(dir: &Path, backgrounds: &[&str], patterns: &[&str]) -> Result<Assembled> {
    let mut texts = vec![("common.wgsl".to_string(), read(dir, "common.wgsl")?)];
    for (k, bg) in backgrounds.iter().enumerate() {
        let name = format!("bg_{bg}.wgsl");
        let text = read(dir, &name)?;
        anyhow::ensure!(text.contains("fn background("), "{name} has no `fn background(`");
        texts.push((name, text.replacen("fn background(", &format!("fn background_{k}("), 1)));
    }
    for (k, pat) in patterns.iter().enumerate() {
        let name = format!("pat_{pat}.wgsl");
        let text = read(dir, &name)?;
        anyhow::ensure!(text.contains("fn pattern("), "{name} has no `fn pattern(`");
        texts.push((name, text.replacen("fn pattern(", &format!("fn pattern_{k}("), 1)));
    }
    texts.push(("(the scene's look switch)".to_string(), dispatch(backgrounds.len(), patterns.len())));
    texts.push(("scene.wgsl".to_string(), read(dir, "scene.wgsl")?));
    let mut shader = Assembled::from_texts(texts);
    shader.vertex = "vs_scene";
    Ok(shader)
}

/// How many patterns a background has room for in a draw's instance number.
pub const SCENE_SLOTS: usize = 64;

/// The functions that call background K and pattern K by number.
fn dispatch(backgrounds: usize, patterns: usize) -> String {
    let mut out = String::from("fn background_at(k: u32, p: vec2f) -> vec3f {\n    switch k {\n");
    for k in 0..backgrounds {
        out += &format!("        case {k}u: {{ return background_{k}(p); }}\n");
    }
    out += "        default: { return background_0(p); }\n    }\n}\n";
    out += "fn pattern_at(k: u32, p: vec2f, base: vec3f) -> vec3f {\n    switch k {\n";
    for k in 0..patterns {
        out += &format!("        case {k}u: {{ return pattern_{k}(p, base); }}\n");
    }
    out += "        default: { return pattern_0(p, base); }\n    }\n}\n";
    out
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
    fn the_scene_with_every_look_validates() {
        use crate::director::{BACKGROUNDS, PATTERNS};
        assert!(PATTERNS.len() <= SCENE_SLOTS);
        let shader = assemble_scene(&default_dir(), &BACKGROUNDS, &PATTERNS).unwrap();
        shader.validate().unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(shader.vertex, "vs_scene");
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
        let message = assemble_scene(&dir, &["tunnel"], &["scope", "rings"])
            .unwrap()
            .validate()
            .unwrap_err();
        std::fs::remove_dir_all(&dir).unwrap();
        assert!(message.starts_with("pat_rings.wgsl, line 4:"), "{message}");
    }

    #[test]
    fn locate_maps_every_line_to_its_file() {
        let shader = assemble_scene(&default_dir(), &["tunnel"], &["rings"]).unwrap();
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
        assert!(assemble_scene(&default_dir(), &["does_not_exist"], &["rings"]).is_err());
    }
}
