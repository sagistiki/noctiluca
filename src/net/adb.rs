//! Installs and opens the Noctiluca Screen app on boxes on the network, with
//! Android's network debugging (adb). The adb that ships inside Noctiluca is
//! used, or the Android SDK's. The first time, the TV asks to allow this Mac.

use std::net::IpAddr;
use std::path::PathBuf;
use std::process::Command;

use anyhow::{Context, Result, bail};

pub const PACKAGE: &str = "com.noctiluca.screen";
const ACTIVITY: &str = "com.noctiluca.screen/.MainActivity";

/// Where Noctiluca finds a file it ships: inside the app (Contents/Resources),
/// or in the project while developing.
fn bundled(name: &str) -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    let resources = exe.parent()?.parent()?.join("Resources").join(name);
    resources.exists().then_some(resources)
}

pub fn adb_path() -> Option<PathBuf> {
    bundled("adb")
        .or_else(|| {
            let home = std::env::var_os("HOME")?;
            let p = PathBuf::from(home).join("Library/Android/sdk/platform-tools/adb");
            p.exists().then_some(p)
        })
        .or_else(|| {
            // Homebrew's android-platform-tools, on Apple silicon and on Intel.
            ["/opt/homebrew/bin/adb", "/usr/local/bin/adb"].into_iter().map(PathBuf::from).find(|p| p.exists())
        })
}

/// The app's package to install: shipped inside Noctiluca, or just built.
pub fn apk_path() -> Option<PathBuf> {
    bundled("NoctilucaScreen.apk").or_else(|| {
        let p = crate::render::resources::project_dir()?.join("android/app/build/outputs/apk/release/app-release.apk");
        p.exists().then_some(p)
    })
}

/// How far a box is from playing.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BoxState {
    /// Network debugging is off on the box (or it is not an Android box).
    NoDebugging,
    /// The TV is asking to allow this Mac.
    WaitingForAllow,
    NotInstalled,
    /// Installed, an older version than this Noctiluca ships.
    Outdated(String),
    Installed(String),
}

fn adb(args: &[&str]) -> Result<String> {
    let path = adb_path().context("adb was not found: install it with `brew install android-platform-tools`, then try again")?;
    let out = Command::new(path).args(args).output().context("adb did not run")?;
    let text = String::from_utf8_lossy(&out.stdout).to_string() + &String::from_utf8_lossy(&out.stderr);
    Ok(text)
}

fn serial(ip: IpAddr) -> String {
    format!("{ip}:5555")
}

/// Connects to the box and says where it stands. May take a few seconds.
pub fn check(ip: IpAddr) -> Result<BoxState> {
    let s = serial(ip);
    let connect = adb(&["connect", &s])?;
    if connect.contains("failed") || connect.contains("cannot") || connect.contains("refused") {
        return Ok(BoxState::NoDebugging);
    }
    let devices = adb(&["devices"])?;
    let line = devices.lines().find(|l| l.starts_with(&s)).unwrap_or("");
    if line.contains("unauthorized") || line.contains("authorizing") {
        return Ok(BoxState::WaitingForAllow);
    }
    if !line.contains("device") {
        return Ok(BoxState::NoDebugging);
    }
    let info = adb(&["-s", &s, "shell", "dumpsys", "package", PACKAGE])?;
    let Some(version) = info.lines().find_map(|l| l.trim().strip_prefix("versionName=")).map(str::to_string) else {
        return Ok(BoxState::NotInstalled);
    };
    Ok(if version_older(&version, SHIPPED_VERSION) { BoxState::Outdated(version) } else { BoxState::Installed(version) })
}

/// The app version this Noctiluca ships (android/app/build.gradle.kts).
pub const SHIPPED_VERSION: &str = "1.2";

/// "1.0" < "1.2" < "1.10".
pub fn version_older(a: &str, b: &str) -> bool {
    let parts = |v: &str| v.split('.').map(|p| p.parse::<u32>().unwrap_or(0)).collect::<Vec<_>>();
    parts(a) < parts(b)
}

/// Installs (or updates) the app, then opens it.
pub fn install(ip: IpAddr) -> Result<()> {
    let apk = apk_path().context("the app's package is not inside Noctiluca")?;
    let s = serial(ip);
    let out = adb(&["-s", &s, "install", "-r", &apk.to_string_lossy()])?;
    if !out.contains("Success") {
        bail!("{}", out.lines().last().unwrap_or("install failed").trim());
    }
    open(ip)
}

/// Opens the app on the box (it comes to the front, full screen).
pub fn open(ip: IpAddr) -> Result<()> {
    let out = adb(&["-s", &serial(ip), "shell", "am", "start", "-n", ACTIVITY])?;
    if out.contains("Error") {
        bail!("{}", out.trim());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn versions_compare_by_number() {
        assert!(version_older("1.0", "1.1"));
        assert!(version_older("1.9", "1.10"));
        assert!(!version_older("1.0", "1.0"));
        assert!(!version_older("2.0", "1.9"));
    }
}
