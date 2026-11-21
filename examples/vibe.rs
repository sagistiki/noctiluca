//! Replays a WAV file through the analysis and prints the vibe it heard,
//! every few seconds: the raw measures (10-second averages) and the vibe
//! they make, so the readings can be checked against what the music is.
//!
//!     cargo run --release --example vibe -- recording.wav [every-seconds]

use noctiluca::analysis::Analyzer;
use noctiluca::audio::read_wav;

fn main() -> anyhow::Result<()> {
    let mut args = std::env::args().skip(1);
    let path = args
        .next()
        .ok_or_else(|| anyhow::anyhow!("usage: vibe FILE.wav [every-seconds]"))?;
    let every: usize = args.next().and_then(|s| s.parse().ok()).unwrap_or(10);
    let (samples, sr) = read_wav(std::path::Path::new(&path))?;
    println!("{path}: {:.0} s", samples.len() as f32 / sr as f32);
    println!();
    println!(
        "   time  mood       centroid  flat  onsets/s  bpm   loud   | bright intense  texture  sure  vibe"
    );
    let mut a = Analyzer::new(sr);
    // The app feeds 1/60 s at a time; so does this.
    for (i, chunk) in samples.chunks(sr as usize / 60).enumerate() {
        a.push(chunk);
        let f = a.features();
        if i % (60 * every) != 60 * every - 1 {
            continue;
        }
        let t = (i + 1) / 60;
        let raw = a.vibe_window();
        let v = f.vibe;
        let measures = match raw {
            Some(s) => format!(
                "{:6.0} Hz {:5.2}  {:6.1}   {:5.1} {:6.1}",
                s.centroid.exp2(),
                s.flatness,
                s.onsets,
                s.bpm,
                s.loudness
            ),
            None => format!("{:>40}", "(no groove yet)"),
        };
        println!(
            "{:4}:{:02}  {:<9} {measures}   | {:6.2} {:7.2} {:8.2} {:5.2}  {}",
            t / 60,
            t % 60,
            format!("{:?}", f.mood),
            v.brightness,
            v.intensity,
            v.texture,
            v.sure,
            v.name(),
        );
    }
    Ok(())
}
