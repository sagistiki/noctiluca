//! Replays a WAV file through the analysis and the director, and prints the
//! show it would put on: every change of look with the vibe at that moment.
//! Runs as fast as the computer can, so a whole set takes seconds.
//!
//!     cargo run --release --example show -- recording.wav [seed]

use noctiluca::analysis::Analyzer;
use noctiluca::audio::read_wav;
use noctiluca::director::{ATMOSPHERES, Director, OBJECTS, PALETTE_NAMES, PATTERNS, THEMES};

fn main() -> anyhow::Result<()> {
    let mut args = std::env::args().skip(1);
    let path = args
        .next()
        .ok_or_else(|| anyhow::anyhow!("usage: show FILE.wav [seed]"))?;
    let seed: u64 = args.next().and_then(|s| s.parse().ok()).unwrap_or(1);
    let (samples, sr) = read_wav(std::path::Path::new(&path))?;
    let mut a = Analyzer::new(sr);
    let mut d = Director::new(seed);
    let mut last = None;
    let dt = 1.0 / 60.0;
    for (i, chunk) in samples.chunks(sr as usize / 60).enumerate() {
        a.push(chunk);
        let f = a.features();
        let out = d.update(&f, dt);
        let l = out.to;
        if last == Some(l) {
            continue;
        }
        last = Some(l);
        let t = i as f32 * dt;
        println!(
            "{:2}:{:02}  {:<14} {:+.1} {:+.1}  [{:<14}] {:<8} {:<7} {:<10} {}",
            (t / 60.0) as u32,
            t as u32 % 60,
            f.vibe.name(),
            f.vibe.brightness,
            f.vibe.intensity,
            THEMES[out.theme].name,
            PATTERNS[l.pattern],
            PALETTE_NAMES[l.palette],
            if l.object == 0 {
                "-"
            } else {
                OBJECTS[l.object]
            },
            if l.atmos == 0 {
                "-"
            } else {
                ATMOSPHERES[l.atmos]
            },
        );
    }
    Ok(())
}
