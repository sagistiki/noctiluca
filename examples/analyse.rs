//! Replays a WAV file through the analysis and prints what it concluded,
//! second by second. For finding out why real music was read the way it was.
//!
//!     cargo run --release --example analyse -- recording.wav
//!
//! If the recording has marks (made with M while recording), they are shown
//! in the last column.

use std::path::Path;

use noctiluca::analysis::Mood;
use noctiluca::analysis::beat::BeatTracker;
use noctiluca::analysis::mood::MoodDetector;
use noctiluca::analysis::spectrum::{HOP, Spectrum};
use noctiluca::audio::{marks_path, read_wav};

/// What happened during one second of audio.
#[derive(Default)]
struct Second {
    frames: u32,
    rms: f32,
    low_ratio: f32,
    hit_ratio: f32,
    bands: [f32; 4],
    kicks: u32,
    bpm: f32,
    confidence: f32,
    mood: Option<Mood>,
    drop: bool,
}

fn main() -> anyhow::Result<()> {
    let path = std::env::args()
        .nth(1)
        .ok_or_else(|| anyhow::anyhow!("usage: analyse FILE.wav"))?;
    let path = Path::new(&path);
    let (samples, sr) = read_wav(path)?;
    let marks: Vec<f32> = std::fs::read_to_string(marks_path(path))
        .unwrap_or_default()
        .lines()
        .filter_map(|l| l.trim().parse().ok())
        .collect();

    let mut spectrum = Spectrum::new(sr);
    let mut beat = BeatTracker::new(spectrum.hop_rate());
    let mut mood = MoodDetector::new();
    let hop_secs = HOP as f32 / sr as f32;

    let mut seconds: Vec<Second> = Vec::new();
    let mut drop_times: Vec<f32> = Vec::new();
    let mut frame_no = 0usize;
    spectrum.push(&samples, |frame| {
        let b = beat.push(frame.flux_low);
        let (now, drop) = mood.push(frame, &b, hop_secs);
        let levels = mood.levels();

        let t = (frame_no as f32 * hop_secs) as usize;
        frame_no += 1;
        if seconds.len() <= t {
            seconds.resize_with(t + 1, Second::default);
        }
        let s = &mut seconds[t];
        s.frames += 1;
        s.rms += frame.rms;
        s.low_ratio += levels.low_fast / levels.low_slow.max(1e-9);
        s.hit_ratio = s.hit_ratio.max(frame.low / levels.low_peak.max(1e-9));
        for (sum, v) in s.bands.iter_mut().zip(frame.bands) {
            *sum += v;
        }
        s.kicks += b.kick as u32;
        s.bpm = b.bpm;
        s.confidence = b.confidence;
        s.mood = Some(now);
        s.drop |= drop;
        if drop {
            drop_times.push(frame_no as f32 * hop_secs);
        }
    });

    println!(
        "{}: {:.1} s at {sr} Hz",
        path.display(),
        samples.len() as f32 / sr as f32
    );
    println!();
    println!("  sec   level  lows vs  biggest   sub bass  mid high  kicks    bpm  sure  mood");
    println!("         (dB)    usual  low hit");
    let mut last_mood = None;
    let mut changes = Vec::new();
    let mut unsure = 0;
    for (t, s) in seconds.iter().enumerate() {
        let n = s.frames.max(1) as f32;
        let mood = s.mood.unwrap_or(Mood::Silence);
        if last_mood != Some(mood) {
            changes.push((t, mood));
            last_mood = Some(mood);
        }
        unsure += (s.confidence < 0.3) as usize;
        let marked: Vec<String> = marks
            .iter()
            .enumerate()
            .filter(|(_, m)| **m as usize == t)
            .map(|(i, m)| format!("<- mark {} ({m:.1} s)", i + 1))
            .collect();
        println!(
            "{t:5}  {:6.1}  {:6.0}%  {:6.0}%  {:4.2} {:4.2} {:4.2} {:4.2}  {:5}  {:5.1}  {:4.2}  {:<9} {}{}",
            20.0 * (s.rms / n).max(1e-9).log10(),
            100.0 * s.low_ratio / n,
            100.0 * s.hit_ratio,
            s.bands[0] / n,
            s.bands[1] / n,
            s.bands[2] / n,
            s.bands[3] / n,
            s.kicks,
            s.bpm,
            s.confidence,
            format!("{mood:?}"),
            if s.drop { "DROP " } else { "" },
            marked.join(" "),
        );
    }

    println!();
    println!("mood changes: {changes:?}");
    println!(
        "drops at: {:?}",
        seconds
            .iter()
            .enumerate()
            .filter(|(_, s)| s.drop)
            .map(|(t, _)| t)
            .collect::<Vec<_>>()
    );
    println!(
        "tempo was unsure (under 0.30) for {unsure} of {} seconds",
        seconds.len()
    );
    println!();
    println!("How to read it: a breakdown is called when \"lows vs usual\" stays under 35% for a");
    println!(
        "second with no kick in that second; a drop when \"biggest low hit\" passes 50% again."
    );
    if !marks.is_empty() {
        compare(&marks, &drop_times);
    }
    Ok(())
}

/// A drop the program found counts as matching a mark this far before it
/// (the program reacts within a moment)...
const EARLY: f32 = 1.5;
/// ...or this far after it (a finger on M is a little late, or early).
const LATE: f32 = 2.5;

/// Compares the drops the program found with the marks made by hand, and
/// prints a score.
fn compare(marks: &[f32], found: &[f32]) {
    println!();
    println!("=== your marks against the program's drops ===");
    let mut used = vec![false; found.len()];
    let mut caught = 0;
    for (i, mark) in marks.iter().enumerate() {
        let best = found
            .iter()
            .enumerate()
            .filter(|(j, t)| !used[*j] && **t >= mark - EARLY && **t <= mark + LATE)
            .min_by(|a, b| (a.1 - mark).abs().total_cmp(&(b.1 - mark).abs()));
        match best {
            Some((j, t)) => {
                used[j] = true;
                caught += 1;
                let off = t - mark;
                let when = if off.abs() < 0.3 {
                    "on time".to_string()
                } else if off > 0.0 {
                    format!("{off:.1} s late")
                } else {
                    format!("{:.1} s early", -off)
                };
                println!(
                    "  mark {:>2} at {mark:6.1} s   caught at {t:6.1} s  ({when})",
                    i + 1
                );
            }
            None => println!("  mark {:>2} at {mark:6.1} s   MISSED", i + 1),
        }
    }
    let extra: Vec<f32> = found
        .iter()
        .zip(&used)
        .filter(|(_, u)| !**u)
        .map(|(t, _)| *t)
        .collect();
    for t in &extra {
        println!("  a drop at {t:6.1} s with no mark near it");
    }
    println!();
    println!(
        "caught {caught} of {} marked drops, {} drops without a mark",
        marks.len(),
        extra.len()
    );
}
