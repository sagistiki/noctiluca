//! Writes the synthesized test track to assets/test_track.wav.
//!
//!     cargo run --example gen_track

use std::path::Path;

fn main() -> anyhow::Result<()> {
    let sr = 48_000;
    let samples = noctiluca::synth::test_track(sr);
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/test_track.wav");
    std::fs::create_dir_all(path.parent().unwrap())?;

    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: sr,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut writer = hound::WavWriter::create(&path, spec)?;
    for s in &samples {
        writer.write_sample((s * i16::MAX as f32) as i16)?;
    }
    writer.finalize()?;
    println!("wrote {} ({} s)", path.display(), samples.len() as u32 / sr);
    Ok(())
}
