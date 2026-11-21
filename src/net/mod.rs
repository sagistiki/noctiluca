//! Network screens: Android TV boxes running the Noctiluca Screen app
//! (android/), used as outputs. See
//! docs/superpowers/specs/2026-11-21-network-box-output-design.md.

pub mod adb;
pub mod clock;
pub mod discovery;
#[cfg(target_os = "macos")]
pub mod encoder;
#[cfg(target_os = "macos")]
pub mod outputs;
pub mod protocol;
#[cfg(target_os = "macos")]
pub mod stream;
