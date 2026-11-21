//! Audio in: a live input device or a WAV file, delivered as mono samples
//! through a lock-free ring buffer.
//!
//! The audio callback runs on a real-time thread owned by the operating
//! system. It must never block, lock or allocate, so the only thing it does
//! is push samples into the ring; the main thread pops them once per frame.

use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use anyhow::{Context, Result, anyhow, bail};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{FromSample, Sample, SampleFormat, SizedSample};
use ringbuf::traits::{Producer, Split};
use ringbuf::{HeapCons, HeapProd, HeapRb};

/// Two seconds at 48 kHz. If the main thread stalls for longer than that the
/// ring fills up and the audio that arrives meanwhile is dropped; once the
/// stall ends, the two seconds from before it are analysed first.
const RING_CAPACITY: usize = 96_000;

pub struct AudioIn {
    /// The reading end of the ring buffer.
    pub rx: HeapCons<f32>,
    pub sample_rate: u32,
    /// What we are listening to, for the log.
    pub label: String,
    /// False for the stand-in used when no audio source could be opened.
    pub connected: bool,
    /// Problems the device has reported so far, counted on the audio thread.
    errors: Arc<AtomicUsize>,
    /// Dropping the stream stops it, so it lives as long as `AudioIn`.
    _stream: Option<cpal::Stream>,
}

impl AudioIn {
    /// How many problems the audio device has reported since it was opened.
    pub fn stream_errors(&self) -> usize {
        self.errors.load(Ordering::Relaxed)
    }

    /// An input that never produces samples. Used when no device is usable so
    /// the visuals still run.
    pub fn silent() -> Self {
        let (_tx, rx) = HeapRb::<f32>::new(16).split();
        Self {
            rx,
            sample_rate: 48_000,
            label: "no audio".into(),
            connected: false,
            errors: Arc::default(),
            _stream: None,
        }
    }
}

/// Averages interleaved multi-channel audio down to one channel.
pub fn mixdown(interleaved: &[f32], channels: usize) -> impl Iterator<Item = f32> + '_ {
    interleaved
        .chunks_exact(channels.max(1))
        .map(move |frame| frame.iter().sum::<f32>() / frame.len() as f32)
}

pub fn list_inputs() -> Vec<String> {
    let Ok(devices) = cpal::default_host().input_devices() else {
        return Vec::new();
    };
    devices.filter_map(|d| device_name(&d)).collect()
}

/// Opens the default input device, or the first one whose name contains
/// `device` (case-insensitive).
/// Below this sample rate an input is a Bluetooth headset in call mode
/// (AirPods and the like): it hears the room from someone's ears, at phone
/// quality, and the beat detection cannot work with it.
const POOR_INPUT_RATE: u32 = 44_100;

/// Which input to use when none was asked for, given each input's name and
/// sample rate and which one is the system default. The default, unless it
/// is a headset microphone and something better exists; then the built-in
/// microphone, or else the first good input.
pub fn best_default(inputs: &[(&str, u32)], default: Option<usize>) -> Option<usize> {
    let good = |i: &usize| inputs[*i].1 >= POOR_INPUT_RATE;
    if let Some(d) = default.filter(good) {
        return Some(d);
    }
    let built_in = (0..inputs.len())
        .filter(good)
        .find(|i| inputs[*i].0.to_lowercase().contains("macbook"));
    built_in
        .or_else(|| (0..inputs.len()).find(good))
        .or(default)
        .or((!inputs.is_empty()).then_some(0))
}

pub fn open_input(device: Option<&str>) -> Result<AudioIn> {
    let host = cpal::default_host();
    let dev = match device {
        None => {
            let devices: Vec<cpal::Device> = host.input_devices()?.collect();
            let default_name = host.default_input_device().and_then(|d| device_name(&d));
            let names: Vec<String> = devices
                .iter()
                .map(|d| device_name(d).unwrap_or_default())
                .collect();
            let rates: Vec<u32> = devices
                .iter()
                .map(|d| d.default_input_config().map_or(0, |c| c.sample_rate()))
                .collect();
            let inputs: Vec<(&str, u32)> = names.iter().map(String::as_str).zip(rates).collect();
            let default = names.iter().position(|n| Some(n) == default_name.as_ref());
            let chosen = best_default(&inputs, default).context("no input device")?;
            if default.is_some_and(|d| d != chosen) {
                println!(
                    "the default input, {}, is a headset microphone at {} Hz; using {} instead (press I to change)",
                    inputs[default.unwrap()].0,
                    inputs[default.unwrap()].1,
                    inputs[chosen].0
                );
            }
            devices
                .into_iter()
                .nth(chosen)
                .expect("index came from this list")
        }
        Some(want) => {
            // An exact name first; otherwise the first name that contains it.
            let want = want.to_lowercase();
            let devices: Vec<cpal::Device> = host.input_devices()?.collect();
            let name_of = |d: &cpal::Device| device_name(d).map(|n| n.to_lowercase());
            let exact = devices
                .iter()
                .position(|d| name_of(d).as_deref() == Some(want.as_str()));
            let partial = || {
                devices
                    .iter()
                    .position(|d| name_of(d).is_some_and(|n| n.contains(&want)))
            };
            let index = exact
                .or_else(partial)
                .ok_or_else(|| anyhow!("no input device matching \"{want}\""))?;
            devices
                .into_iter()
                .nth(index)
                .expect("index came from this list")
        }
    };
    let label = device_name(&dev).unwrap_or_else(|| "input".into());
    let supported = dev
        .default_input_config()
        .context("input device has no usable format")?;
    let format = supported.sample_format();
    let config: cpal::StreamConfig = supported.into();
    let sample_rate = config.sample_rate;

    let (tx, rx) = HeapRb::<f32>::new(RING_CAPACITY).split();
    let errors = Arc::new(AtomicUsize::new(0));
    let stream = match format {
        SampleFormat::F32 => input_stream::<f32>(&dev, config, tx, errors.clone())?,
        SampleFormat::I16 => input_stream::<i16>(&dev, config, tx, errors.clone())?,
        SampleFormat::I32 => input_stream::<i32>(&dev, config, tx, errors.clone())?,
        SampleFormat::U16 => input_stream::<u16>(&dev, config, tx, errors.clone())?,
        other => bail!("unsupported input sample format {other}"),
    };
    stream.play()?;
    Ok(AudioIn {
        rx,
        sample_rate,
        label,
        connected: true,
        errors,
        _stream: Some(stream),
    })
}

/// Plays a WAV file on a loop and feeds the same samples to the ring.
/// With `mute` nothing is sent to the speakers.
pub fn open_file(path: &Path, mute: bool) -> Result<AudioIn> {
    let (samples, file_rate) = read_wav(path)?;
    if samples.is_empty() {
        bail!("{} has no audio in it", path.display());
    }
    let label = path
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .into_owned();
    let (tx, rx) = HeapRb::<f32>::new(RING_CAPACITY).split();
    let errors = Arc::new(AtomicUsize::new(0));

    if mute {
        spawn_paced(samples, file_rate, tx);
        return Ok(AudioIn {
            rx,
            sample_rate: file_rate,
            label,
            connected: true,
            errors: Arc::default(),
            _stream: None,
        });
    }

    let dev = cpal::default_host()
        .default_output_device()
        .context("no output device")?;
    let supported = dev.default_output_config()?;
    let format = supported.sample_format();
    let config: cpal::StreamConfig = supported.into();
    let sample_rate = config.sample_rate;
    let samples = playable(samples, file_rate, sample_rate)?;
    let stream = match format {
        SampleFormat::F32 => output_stream::<f32>(&dev, config, samples, tx, errors.clone())?,
        SampleFormat::I16 => output_stream::<i16>(&dev, config, samples, tx, errors.clone())?,
        SampleFormat::I32 => output_stream::<i32>(&dev, config, samples, tx, errors.clone())?,
        SampleFormat::U16 => output_stream::<u16>(&dev, config, samples, tx, errors.clone())?,
        other => bail!("unsupported output sample format {other}"),
    };
    stream.play()?;
    Ok(AudioIn {
        rx,
        sample_rate,
        label,
        connected: true,
        errors,
        _stream: Some(stream),
    })
}

fn device_name(dev: &cpal::Device) -> Option<String> {
    dev.description().ok().map(|d| d.name().to_string())
}

/// What the audio thread does when the device reports a problem: count it.
/// Printing from here would take the terminal's lock on a thread that must
/// never wait, and these reports tend to come exactly when the machine is
/// overloaded. The main thread prints the count instead.
fn count_errors(errors: Arc<AtomicUsize>) -> impl FnMut(cpal::Error) + Send + 'static {
    move |_| {
        errors.fetch_add(1, Ordering::Relaxed);
    }
}

fn input_stream<T>(
    dev: &cpal::Device,
    config: cpal::StreamConfig,
    mut tx: HeapProd<f32>,
    errors: Arc<AtomicUsize>,
) -> Result<cpal::Stream>
where
    T: SizedSample,
    f32: FromSample<T>,
{
    let channels = config.channels as usize;
    let stream = dev.build_input_stream(
        config,
        move |data: &[T], _: &cpal::InputCallbackInfo| {
            for frame in data.chunks_exact(channels) {
                let sum: f32 = frame.iter().map(|s| f32::from_sample(*s)).sum();
                // A full ring means the main thread is behind; drop the sample.
                let _ = tx.try_push(sum / channels as f32);
            }
        },
        count_errors(errors),
        None,
    )?;
    Ok(stream)
}

fn output_stream<T>(
    dev: &cpal::Device,
    config: cpal::StreamConfig,
    samples: Vec<f32>,
    mut tx: HeapProd<f32>,
    errors: Arc<AtomicUsize>,
) -> Result<cpal::Stream>
where
    T: SizedSample + FromSample<f32>,
{
    let channels = config.channels as usize;
    let mut pos = 0;
    let stream = dev.build_output_stream(
        config,
        move |data: &mut [T], _: &cpal::OutputCallbackInfo| {
            for frame in data.chunks_exact_mut(channels) {
                let s = samples[pos];
                pos = (pos + 1) % samples.len();
                let _ = tx.try_push(s);
                frame.fill(T::from_sample(s));
            }
        },
        count_errors(errors),
        None,
    )?;
    Ok(stream)
}

/// Feeds samples to the ring at real-time speed without any audio device.
fn spawn_paced(samples: Vec<f32>, sample_rate: u32, mut tx: HeapProd<f32>) {
    std::thread::spawn(move || {
        let start = Instant::now();
        let mut sent = 0usize;
        loop {
            let due = (start.elapsed().as_secs_f64() * sample_rate as f64) as usize;
            while sent < due {
                let _ = tx.try_push(samples[sent % samples.len()]);
                sent += 1;
            }
            std::thread::sleep(Duration::from_millis(5));
        }
    });
}

/// Reads a WAV file as mono samples in -1..1, with its sample rate.
pub fn read_wav(path: &Path) -> Result<(Vec<f32>, u32)> {
    let mut reader =
        hound::WavReader::open(path).with_context(|| format!("cannot open {}", path.display()))?;
    let spec = reader.spec();
    let interleaved: Vec<f32> = match spec.sample_format {
        hound::SampleFormat::Float => reader.samples::<f32>().collect::<Result<_, _>>()?,
        hound::SampleFormat::Int => {
            let scale = 1.0 / (1u64 << (spec.bits_per_sample - 1)) as f32;
            reader
                .samples::<i32>()
                .map(|s| s.map(|v| v as f32 * scale))
                .collect::<Result<_, _>>()?
        }
    };
    let mono = mixdown(&interleaved, spec.channels as usize).collect();
    Ok((mono, spec.sample_rate))
}

/// Where the marks made during a recording are kept: next to the WAV.
pub fn marks_path(wav: &Path) -> std::path::PathBuf {
    wav.with_extension("marks.txt")
}

/// Writes what the app hears to a WAV file (mono, 16-bit), so a session with
/// real music can be replayed through the analysis later, as often as needed.
pub struct Recorder {
    writer: hound::WavWriter<std::io::BufWriter<std::fs::File>>,
    since_flush: usize,
    flush_every: usize,
}

impl Recorder {
    pub fn create(path: &Path, sample_rate: u32) -> Result<Self> {
        if let Some(dir) = path.parent().filter(|d| !d.as_os_str().is_empty()) {
            std::fs::create_dir_all(dir)?;
        }
        let spec = hound::WavSpec {
            channels: 1,
            sample_rate,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };
        let writer = hound::WavWriter::create(path, spec)
            .with_context(|| format!("cannot record to {}", path.display()))?;
        Ok(Self {
            writer,
            since_flush: 0,
            flush_every: sample_rate as usize,
        })
    }

    pub fn write(&mut self, samples: &[f32]) -> Result<()> {
        for s in samples {
            let s = if s.is_finite() {
                s.clamp(-1.0, 1.0)
            } else {
                0.0
            };
            self.writer.write_sample((s * i16::MAX as f32) as i16)?;
        }
        // A WAV file states its own length at the top. Bringing that up to
        // date once a second means the file is usable even if the app is
        // killed rather than closed.
        self.since_flush += samples.len();
        if self.since_flush >= self.flush_every {
            self.since_flush = 0;
            self.writer.flush()?;
        }
        Ok(())
    }
}

/// The file's samples at the output device's rate, or an error if nothing
/// would be left to play.
fn playable(samples: Vec<f32>, from: u32, to: u32) -> Result<Vec<f32>> {
    let out = resample(&samples, from, to);
    if out.is_empty() {
        bail!("the file is too short to play");
    }
    Ok(out)
}

/// Linear-interpolation resampling. Not hi-fi, but the analysis only needs
/// the tempo and the rough spectrum to survive.
fn resample(samples: &[f32], from: u32, to: u32) -> Vec<f32> {
    if from == to {
        return samples.to_vec();
    }
    let step = from as f64 / to as f64;
    let len = (samples.len() as f64 / step) as usize;
    (0..len)
        .map(|i| {
            let pos = i as f64 * step;
            let a = samples[pos as usize];
            let b = samples[(pos as usize + 1).min(samples.len() - 1)];
            a + (b - a) * pos.fract() as f32
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_headset_microphone_is_passed_over_for_a_better_one() {
        let inputs = [
            ("Dana's AirPods", 24_000),
            ("MacBook Pro Microphone", 48_000),
            ("iPhone Microphone", 48_000),
        ];
        // The default is the AirPods: take the built-in microphone instead.
        assert_eq!(best_default(&inputs, Some(0)), Some(1));
        // A good default is kept.
        assert_eq!(best_default(&inputs, Some(2)), Some(2));
        // With nothing better, a poor default is still better than nothing.
        assert_eq!(best_default(&inputs[..1], Some(0)), Some(0));
        // No default at all: the best there is.
        assert_eq!(best_default(&inputs, None), Some(1));
        assert_eq!(best_default(&[], None), None);
    }

    #[test]
    fn mixdown_averages_channels() {
        let mono: Vec<f32> = mixdown(&[1.0, 0.0, 0.5, 0.5], 2).collect();
        assert_eq!(mono, vec![0.5, 0.5]);
    }

    #[test]
    fn mixdown_of_mono_is_identity() {
        let mono: Vec<f32> = mixdown(&[0.1, -0.2, 0.3], 1).collect();
        assert_eq!(mono, vec![0.1, -0.2, 0.3]);
    }

    #[test]
    fn a_file_too_short_to_play_is_refused() {
        // One sample at 96 kHz is half a sample at 48 kHz: nothing to play.
        // That must be an error here, not an empty buffer for the audio
        // thread to index into.
        assert!(playable(vec![0.5], 96_000, 48_000).is_err());
        assert!(playable(vec![], 48_000, 48_000).is_err());
        assert_eq!(playable(vec![0.5, 0.5], 96_000, 48_000).unwrap().len(), 1);
        assert_eq!(playable(vec![0.1; 480], 48_000, 48_000).unwrap().len(), 480);
    }

    fn temp_wav(tag: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!("noctiluca-{tag}-{}.wav", std::process::id()))
    }

    #[test]
    fn a_recording_reads_back_as_what_was_heard() {
        let path = temp_wav("roundtrip");
        let heard: Vec<f32> = (0..5000).map(|i| (i as f32 * 0.01).sin() * 0.8).collect();
        {
            let mut rec = Recorder::create(&path, 44_100).unwrap();
            for chunk in heard.chunks(333) {
                rec.write(chunk).unwrap();
            }
        } // dropped here, which finishes the file
        let (back, rate) = read_wav(&path).unwrap();
        std::fs::remove_file(&path).unwrap();
        assert_eq!(rate, 44_100);
        assert_eq!(back.len(), heard.len());
        let worst = heard
            .iter()
            .zip(&back)
            .map(|(a, b)| (a - b).abs())
            .fold(0.0, f32::max);
        assert!(worst < 1.0 / 16_000.0, "worst error {worst}");
    }

    #[test]
    fn a_recording_survives_the_app_being_killed() {
        // Never dropped, as when the process is killed: everything up to the
        // last one-second flush must still be readable.
        let path = temp_wav("killed");
        let mut rec = Recorder::create(&path, 8_000).unwrap();
        rec.write(&vec![0.25; 20_000]).unwrap(); // 2.5 s
        let (back, _) = read_wav(&path).unwrap();
        std::mem::forget(rec);
        std::fs::remove_file(&path).unwrap();
        assert!(
            back.len() >= 16_000,
            "only {} samples were readable",
            back.len()
        );
    }

    #[test]
    fn recording_clips_and_cleans_bad_samples() {
        let path = temp_wav("bad");
        {
            let mut rec = Recorder::create(&path, 8_000).unwrap();
            rec.write(&[2.0, -2.0, f32::NAN, f32::INFINITY, 0.5])
                .unwrap();
        }
        let (back, _) = read_wav(&path).unwrap();
        std::fs::remove_file(&path).unwrap();
        let expect = [1.0, -1.0, 0.0, 0.0, 0.5];
        for (b, e) in back.iter().zip(expect) {
            assert!((b - e).abs() < 0.001, "{back:?}");
        }
    }

    #[test]
    fn resample_keeps_duration() {
        let one_second = vec![0.0; 44_100];
        assert_eq!(resample(&one_second, 44_100, 48_000).len(), 48_000);
    }
}
