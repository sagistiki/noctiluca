//! Where the program finds its shaders and models.
//!
//! Packed as a Mac app (see package_app.sh) they travel inside it, in
//! Noctiluca.app/Contents/Resources. Run from the project with cargo, they
//! are the folders in the project.

use std::path::{Path, PathBuf};

/// The folder `name` (such as "shaders") inside the app the program runs
/// from, or else `in_project` (such as "assets/models") in the project.
pub fn find(name: &str, in_project: &str) -> PathBuf {
    let exe = std::env::current_exe().ok();
    exe.as_deref()
        .and_then(|exe| inside_app(exe, name))
        .or_else(|| Some(project_dir()?.join(in_project)))
        .unwrap_or_else(|| PathBuf::from(in_project))
}

/// The project folder, for a program run with cargo from target/: the
/// nearest folder above the program with a Cargo.toml. Found at run time
/// rather than written into the program, so a build carries no path of
/// the computer it was built on.
pub fn project_dir() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    exe.ancestors().skip(1).take(10).find(|d| d.join("Cargo.toml").is_file()).map(Path::to_path_buf)
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

    #[test]
    fn outside_an_app_the_project_folders_are_used() {
        // Tests run from target/, which is not inside an app.
        let project = Path::new(env!("CARGO_MANIFEST_DIR"));
        assert_eq!(project_dir().as_deref(), Some(project));
        assert_eq!(find("shaders", "shaders"), project.join("shaders"));
        assert_eq!(find("models", "assets/models"), project.join("assets/models"));
    }
}
