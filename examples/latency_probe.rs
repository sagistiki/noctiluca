//! Measures how late the analysis is: plays kicks whose times are known and
//! compares them with when the beat grid and the kick detector say they came.
use noctiluca::analysis::Analyzer;

fn main() {
    for (name, signal, bpm) in [
        (
            "kick train 128",
            noctiluca::synth::kick_train(128.0, 40.0, 48000),
            128.0f64,
        ),
        (
            "dark intense 140",
            noctiluca::synth::styled_track(48000, 40.0, true, true),
            140.0,
        ),
        (
            "bright calm 122",
            noctiluca::synth::styled_track(48000, 40.0, false, false),
            122.0,
        ),
    ] {
        let sr = 48000usize;
        let beat = 60.0 / bpm;
        let mut a = Analyzer::new(sr as u32);
        let dt = 1.0 / 60.0;
        let mut prev = None::<f64>;
        let (mut grid, mut kicks) = (Vec::new(), Vec::new());
        for (i, chunk) in signal.chunks(sr / 60).enumerate() {
            a.push(chunk);
            let f = a.features();
            let t = (i + 1) as f64 * dt;
            if t < 15.0 || f.bpm <= 0.0 {
                prev = None;
                continue;
            }
            let pos = f.beat_count as f64 + f.beat_phase as f64;
            if let Some(p) = prev
                && pos.floor() > p.floor()
            {
                // Crossed a beat during this frame: interpolate when.
                let frac = (pos.floor() - p) / (pos - p);
                let at = t - dt + frac * dt;
                let nearest = (at / beat).round() * beat;
                grid.push(at - nearest);
            }
            if f.kick {
                let nearest = (t / beat).round() * beat;
                kicks.push(t - nearest);
            }
            prev = Some(pos);
        }
        let stats = |v: &[f64]| {
            let n = v.len() as f64;
            let mean = v.iter().sum::<f64>() / n;
            let sd = (v.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / n).sqrt();
            format!(
                "{:+.1} ms mean, {:.1} ms spread ({} beats)",
                1000.0 * mean,
                1000.0 * sd,
                v.len()
            )
        };
        println!(
            "{name:18}  beat grid {}   detected kicks {}",
            stats(&grid),
            stats(&kicks)
        );
    }
}
