//! Where the program finds its shaders and models.
//!
//! Packed as a Mac app (see package_app.sh) they travel inside it, in
//! Noctiluca.app/Contents/Resources. Run from the project with cargo, they
//! are the folders in the project.

use std::path::{Path, PathBuf};

/// The folder `name` (such as "shaders") inside the app the program runs
/// from; or, for a program cargo built in the project, `in_project` (such
/// as "assets/models") there. Nowhere else: not the current folder, not
/// any folder that merely looks like a project.
pub fn find(name: &str, in_project: &str) -> PathBuf {
    let Ok(exe) = std::env::current_exe() else { return PathBuf::from(name) };
    if let Some(dir) = inside_app(&exe, name) {
        return dir;
    }
    match project_of(&exe) {
        Some(project) => project.join(in_project),
        // Not found: the app's own place for it, so the error names it.
        None => exe.parent().and_then(Path::parent).map(|c| c.join("Resources").join(name)).unwrap_or_else(|| PathBuf::from(name)),
    }
}

/// The project folder, for a program cargo built in it. Found at run time
/// rather than written into the program, so a build carries no path of
/// the computer it was built on.
pub fn project_dir() -> Option<PathBuf> {
    project_of(&std::env::current_exe().ok()?)
}

/// The project a program at `exe` was built in: it must sit under the
/// project's own target/ folder, the project must be Noctiluca's, and the
/// program must not be inside an app (a packed app only ever uses what it
/// carries, so a planted Cargo.toml next to it changes nothing).
fn project_of(exe: &Path) -> Option<PathBuf> {
    if exe.ancestors().any(|d| d.extension().is_some_and(|e| e == "app")) {
        return None;
    }
    let target = exe.ancestors().skip(1).take(8).find(|d| d.file_name().is_some_and(|n| n == "target"))?;
    let project = target.parent()?;
    let manifest = std::fs::read_to_string(project.join("Cargo.toml")).ok()?;
    let ours = manifest
        .lines()
        .map(str::trim)
        .find(|l| l.starts_with("name"))
        .is_some_and(|l| l.split('=').nth(1).map(|v| v.trim().trim_matches('"')) == Some("noctiluca"));
    ours.then(|| project.to_path_buf())
}

/// For a program at App.app/Contents/MacOS/program, the folder
/// App.app/Contents/Resources/`name`, if there is one.
fn inside_app(exe: &Path, name: &str) -> Option<PathBuf> {
    let dir = exe.parent()?.parent()?.join("Resources").join(name);
    dir.is_dir().then_some(dir)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inside_an_app_its_own_resources_are_used() {
        let app = std::env::temp_dir()
            .join(format!("noctiluca-app-{}", std::process::id()))
            .join("Test.app/Contents");
        std::fs::create_dir_all(app.join("MacOS")).unwrap();
        std::fs::create_dir_all(app.join("Resources/shaders")).unwrap();
        let exe = app.join("MacOS/noctiluca");
        assert_eq!(
            inside_app(&exe, "shaders"),
            Some(app.join("Resources/shaders"))
        );
        assert_eq!(
            inside_app(&exe, "models"),
            None,
            "no such folder in the app"
        );
        std::fs::remove_dir_all(app.parent().unwrap().parent().unwrap()).unwrap();
    }

    /// A Cargo.toml that is not Noctiluca's, a program outside target/, or
    /// one inside an app: no project, so nothing is read from there.
    #[test]
    fn only_noctiluca_s_own_project_counts() {
        let root = std::env::temp_dir().join(format!("noctiluca-proj-{}", std::process::id()));
        let make = |dir: &str, name: &str| {
            let d = root.join(dir);
            std::fs::create_dir_all(d.join("target/release")).unwrap();
            std::fs::write(d.join("Cargo.toml"), format!("[package]\nname = \"{name}\"\n")).unwrap();
            d
        };
        let ours = make("ours", "noctiluca");
        let other = make("other", "evil");
        assert_eq!(project_of(&ours.join("target/release/noctiluca")), Some(ours.clone()));
        assert_eq!(project_of(&other.join("target/release/noctiluca")), None);
        assert_eq!(project_of(&ours.join("noctiluca")), None, "not under target/");
        assert_eq!(project_of(&ours.join("target/Noctiluca.app/Contents/MacOS/noctiluca")), None, "inside an app");
        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn outside_an_app_the_project_folders_are_used() {
        // Tests run from target/, which is not inside an app.
        let project = Path::new(env!("CARGO_MANIFEST_DIR"));
        assert_eq!(project_dir().as_deref(), Some(project));
        assert_eq!(find("shaders", "shaders"), project.join("shaders"));
        assert_eq!(find("models", "assets/models"), project.join("assets/models"));
    }
}
