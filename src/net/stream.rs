//! The link to one network screen: connects (and reconnects), keeps the two
//! clocks in step, encodes the pictures it is given and sends each with the
//! moment the screen must show it. It never makes the show wait: a picture
//! that finds the link busy is dropped, and the next one is a key frame.

use std::io::{BufReader, BufWriter, Write};
use std::net::{SocketAddr, TcpStream};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{Receiver, RecvTimeoutError, SyncSender, TrySendError, sync_channel};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use super::clock::{ClockSync, now_ns};
use super::protocol::{Config, Hello, Message, Stats};

/// One picture for the screen: BGRA, top row first, `stride` bytes a row,
/// to be shown at `show_at` (the Mac's clock, ns).
pub struct Picture {
    pub bgra: Vec<u8>,
    pub stride: usize,
    pub width: u32,
    pub height: u32,
    pub show_at: i64,
}

/// What the screen is up to, for the settings and the menu.
#[derive(Clone, Debug, Default)]
pub struct Status {
    pub connected: bool,
    pub hello: Option<Hello>,
    /// From drawing to showing, as the stream is scheduled now.
    pub latency_ms: f32,
    /// A typical round trip.
    pub round_trip_ms: f32,
    pub stats: Stats,
    /// Pictures dropped on the Mac (the link was busy).
    pub dropped_here: u64,
    pub error: Option<String>,
}

/// From drawing a picture to sending it: reading it back from the GPU
/// (about two frames) and encoding.
const PIPELINE_MS: f32 = 40.0;
/// Held back for the network's bad moments and the screen's refresh.
const MARGIN_MS: f32 = 30.0;
pub const MIN_LATENCY_MS: f32 = 60.0;
pub const MAX_LATENCY_MS: f32 = 500.0;
const START_LATENCY_MS: f32 = 160.0;

struct Shared {
    status: Mutex<Status>,
    clock: Mutex<ClockSync>,
    /// The latency the pictures are drawn for, as f32 bits (read every frame).
    latency_bits: AtomicU64,
    /// Added by hand, ms (as f32 bits).
    extra_bits: AtomicU64,
    need_key: AtomicBool,
    stop: AtomicBool,
    game_mode: AtomicBool,
    reconfigure: AtomicBool,
}

impl Shared {
    fn latency_ms(&self) -> f32 {
        f32::from_bits(self.latency_bits.load(Ordering::Relaxed) as u32)
    }
    fn set_latency(&self, ms: f32) {
        self.latency_bits.store(ms.to_bits() as u64, Ordering::Relaxed);
    }
    fn extra_ms(&self) -> f32 {
        f32::from_bits(self.extra_bits.load(Ordering::Relaxed) as u32)
    }
}

pub struct ScreenStream {
    pub addr: SocketAddr,
    tx: SyncSender<Picture>,
    shared: Arc<Shared>,
}

/// The latency a link should draw for, from what it measures: the pipeline
/// here, half a slow round trip, the screen's decoding, a margin.
pub fn wanted_latency_ms(slow_trip_ms: f32, decode_ms: f32, extra_ms: f32) -> f32 {
    (PIPELINE_MS + slow_trip_ms / 2.0 + decode_ms + MARGIN_MS + extra_ms).clamp(MIN_LATENCY_MS, MAX_LATENCY_MS)
}

/// Moves toward `wanted`: quickly up (late frames are dropped, worse than
/// a little more delay), slowly down (so it does not wobble).
pub fn ease_latency(current: f32, wanted: f32) -> f32 {
    if wanted > current { current + (wanted - current) * 0.5 } else { current + (wanted - current) * 0.05 }
}

impl ScreenStream {
    pub fn start(addr: SocketAddr, fps: u32, bitrate: u32) -> Self {
        let (tx, rx) = sync_channel::<Picture>(2);
        let shared = Arc::new(Shared {
            status: Mutex::new(Status { latency_ms: START_LATENCY_MS, ..Status::default() }),
            clock: Mutex::new(ClockSync::default()),
            latency_bits: AtomicU64::new(START_LATENCY_MS.to_bits() as u64),
            extra_bits: AtomicU64::new(0f32.to_bits() as u64),
            need_key: AtomicBool::new(true),
            stop: AtomicBool::new(false),
            game_mode: AtomicBool::new(true),
            reconfigure: AtomicBool::new(false),
        });
        let s = shared.clone();
        thread::Builder::new()
            .name(format!("screen {addr}"))
            .spawn(move || link(addr, fps, bitrate, rx, s))
            .ok();
        ScreenStream { addr, tx, shared }
    }

    /// How far ahead pictures for this screen are drawn, in seconds.
    pub fn latency_secs(&self) -> f32 {
        self.shared.latency_ms() / 1000.0
    }

    /// Ready for pictures: connected and the clocks in step.
    pub fn ready(&self) -> bool {
        self.shared.status.lock().is_ok_and(|s| s.connected) && self.shared.clock.lock().is_ok_and(|c| c.ready())
    }

    /// Hands a picture over without waiting; false if it was dropped.
    pub fn offer(&self, p: Picture) -> bool {
        match self.tx.try_send(p) {
            Ok(()) => true,
            Err(TrySendError::Full(_)) | Err(TrySendError::Disconnected(_)) => {
                self.shared.need_key.store(true, Ordering::Relaxed);
                if let Ok(mut s) = self.shared.status.lock() {
                    s.dropped_here += 1;
                }
                false
            }
        }
    }

    /// Ask the TV for its game mode (sent with the next stream start).
    pub fn set_game_mode(&self, on: bool) {
        if self.shared.game_mode.swap(on, Ordering::Relaxed) != on {
            // A new Config goes out with the next picture.
            self.shared.reconfigure.store(true, Ordering::Relaxed);
        }
    }

    /// The hand correction for this screen, ms (positive: shown later).
    pub fn set_extra_ms(&self, ms: f32) {
        self.shared.extra_bits.store(ms.clamp(-200.0, 300.0).to_bits() as u64, Ordering::Relaxed);
    }

    pub fn status(&self) -> Status {
        self.shared.status.lock().map(|s| s.clone()).unwrap_or_default()
    }
}

impl Drop for ScreenStream {
    fn drop(&mut self) {
        self.shared.stop.store(true, Ordering::Relaxed);
    }
}

fn set_error(shared: &Shared, e: impl ToString) {
    if let Ok(mut s) = shared.status.lock() {
        s.connected = false;
        s.error = Some(e.to_string());
    }
}

/// Connects, and again after any break, until stopped.
fn link(addr: SocketAddr, fps: u32, bitrate: u32, rx: Receiver<Picture>, shared: Arc<Shared>) {
    while !shared.stop.load(Ordering::Relaxed) {
        match session(addr, fps, bitrate, &rx, &shared) {
            Ok(()) => break,
            Err(e) => set_error(&shared, format!("{e:#}")),
        }
        // Wait a moment, throwing away what comes in meanwhile.
        let until = Instant::now() + Duration::from_secs(1);
        while Instant::now() < until && !shared.stop.load(Ordering::Relaxed) {
            let _ = rx.recv_timeout(Duration::from_millis(100));
        }
    }
    if let Ok(mut s) = shared.status.lock() {
        s.connected = false;
    }
}

fn session(addr: SocketAddr, fps: u32, bitrate: u32, rx: &Receiver<Picture>, shared: &Arc<Shared>) -> anyhow::Result<()> {
    let socket = TcpStream::connect_timeout(&addr, Duration::from_secs(2))?;
    socket.set_nodelay(true)?;
    socket.set_write_timeout(Some(Duration::from_secs(2)))?;
    let mut out = BufWriter::with_capacity(1 << 18, socket.try_clone()?);
    *shared.clock.lock().unwrap() = ClockSync::default();
    shared.need_key.store(true, Ordering::Relaxed);
    if let Ok(mut s) = shared.status.lock() {
        s.connected = true;
        s.error = None;
        s.stats = Stats::default();
    }
    // Answers from the screen, on their own thread.
    let reader = socket.try_clone()?;
    let s2 = shared.clone();
    let alive = Arc::new(AtomicBool::new(true));
    let alive2 = alive.clone();
    thread::spawn(move || {
        let mut r = BufReader::new(reader);
        while let Ok(m) = Message::read(&mut r) {
            match m {
                Message::Hello(h) => {
                    if let Ok(mut s) = s2.status.lock() {
                        s.hello = Some(h);
                    }
                }
                Message::Pong { mac_ns, screen_ns } => {
                    let mut c = s2.clock.lock().unwrap();
                    c.add(mac_ns, screen_ns, now_ns());
                    if let Ok(mut s) = s2.status.lock() {
                        s.round_trip_ms = c.round_trip().unwrap_or(0) as f32 / 1e6;
                    }
                }
                Message::Stats(st) => {
                    if let Ok(mut s) = s2.status.lock() {
                        s.stats = st;
                    }
                    // The latency follows the network and the decoder.
                    let slow = s2.clock.lock().ok().and_then(|c| c.slow_trip()).unwrap_or(0) as f32 / 1e6;
                    let wanted = wanted_latency_ms(slow, st.decode_ms as f32, s2.extra_ms());
                    let l = ease_latency(s2.latency_ms(), wanted);
                    s2.set_latency(l);
                    if let Ok(mut s) = s2.status.lock() {
                        s.latency_ms = l;
                    }
                }
                _ => {}
            }
        }
        alive2.store(false, Ordering::Relaxed);
    });

    let mut encoder: Option<super::encoder::Encoder> = None;
    let mut last_ping = Instant::now() - Duration::from_secs(1);
    let started = Instant::now();
    let result = (|| -> anyhow::Result<()> {
        loop {
            if shared.stop.load(Ordering::Relaxed) {
                Message::Idle.write(&mut out)?;
                out.flush()?;
                return Ok(());
            }
            if !alive.load(Ordering::Relaxed) {
                anyhow::bail!("the screen closed the connection");
            }
            // Pings: often at first, to find the clock quickly, then twice a second.
            let every = if started.elapsed() < Duration::from_secs(2) { 100 } else { 500 };
            if last_ping.elapsed() >= Duration::from_millis(every) {
                Message::Ping(now_ns()).write(&mut out)?;
                out.flush()?;
                last_ping = Instant::now();
            }
            let picture = match rx.recv_timeout(Duration::from_millis(50)) {
                Ok(p) => p,
                Err(RecvTimeoutError::Timeout) => continue,
                Err(RecvTimeoutError::Disconnected) => return Ok(()),
            };
            let offset = shared.clock.lock().ok().and_then(|c| c.ready().then(|| c.offset()).flatten());
            let Some(offset) = offset else { continue };
            if shared.reconfigure.swap(false, Ordering::Relaxed) {
                encoder = None;
            }
            if encoder.as_ref().is_none_or(|e| (e.width, e.height) != (picture.width, picture.height)) {
                encoder = Some(super::encoder::Encoder::new(picture.width, picture.height, fps, bitrate)?);
                let game_mode = shared.game_mode.load(Ordering::Relaxed);
                Message::Config(Config { width: picture.width, height: picture.height, fps, game_mode }).write(&mut out)?;
                shared.need_key.store(true, Ordering::Relaxed);
            }
            let enc = encoder.as_mut().unwrap();
            let key = shared.need_key.swap(false, Ordering::Relaxed);
            enc.encode(&picture.bgra, picture.stride, picture.show_at / 1000, key)?;
            enc.flush();
            for e in enc.take() {
                let show_at = e.time_us * 1000 + offset + (shared.extra_ms() * 1e6) as i64;
                Message::Frame { show_at, key: e.key, data: e.data }.write(&mut out)?;
            }
            out.flush()?;
        }
    })();
    let _ = socket.shutdown(std::net::Shutdown::Both);
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::TcpListener;

    #[test]
    fn latency_follows_the_network_quickly_up_and_slowly_down() {
        assert_eq!(wanted_latency_ms(0.0, 0.0, 0.0), 70.0);
        assert_eq!(wanted_latency_ms(2000.0, 0.0, 0.0), MAX_LATENCY_MS);
        assert_eq!(wanted_latency_ms(0.0, 0.0, -300.0), MIN_LATENCY_MS);
        let up = ease_latency(100.0, 200.0);
        let down = ease_latency(200.0, 100.0);
        assert!(up >= 150.0 && down > 190.0);
    }

    /// A pretend screen on this Mac: the link connects, syncs the clocks,
    /// and the first picture arrives as a configured key frame, scheduled
    /// on the screen's clock.
    #[test]
    fn a_picture_reaches_a_screen_as_a_scheduled_key_frame() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let screen_offset: i64 = 3_000_000_000; // its clock is 3 s ahead
        let got = Arc::new(Mutex::new(Vec::<Message>::new()));
        let got2 = got.clone();
        thread::spawn(move || {
            let (s, _) = listener.accept().unwrap();
            let mut w = s.try_clone().unwrap();
            Message::Hello(Hello { name: "test".into(), width: 320, height: 180, ..Hello::default() }).write(&mut w).unwrap();
            let mut r = BufReader::new(s);
            while let Ok(m) = Message::read(&mut r) {
                if let Message::Ping(t) = m {
                    Message::Pong { mac_ns: t, screen_ns: now_ns() + screen_offset }.write(&mut w).unwrap();
                } else {
                    got2.lock().unwrap().push(m);
                }
            }
        });
        let stream = ScreenStream::start(addr, 60, 2_000_000);
        let until = Instant::now() + Duration::from_secs(5);
        while !stream.ready() && Instant::now() < until {
            thread::sleep(Duration::from_millis(20));
        }
        assert!(stream.ready(), "{:?}", stream.status());
        assert_eq!(stream.status().hello.unwrap().name, "test");
        let show_at = now_ns() + 150_000_000;
        let (w, h) = (320u32, 180u32);
        let have_encoder = super::super::encoder::Encoder::new(w, h, 60, 1_000_000).is_ok();
        assert!(stream.offer(Picture { bgra: vec![90; (w * h * 4) as usize], stride: (w * 4) as usize, width: w, height: h, show_at }));
        let until = Instant::now() + Duration::from_secs(5);
        while got.lock().unwrap().iter().filter(|m| matches!(m, Message::Frame { .. })).count() == 0 && Instant::now() < until && have_encoder {
            thread::sleep(Duration::from_millis(20));
        }
        if !have_encoder {
            return;
        }
        let got = got.lock().unwrap();
        assert!(matches!(got[0], Message::Config(Config { width: 320, height: 180, .. })), "{:?}", got.first());
        let Message::Frame { show_at: at, key, .. } = &got[1] else { panic!("{:?}", got.get(1)) };
        assert!(*key);
        // Scheduled on the screen's clock, within a few ms on this machine.
        let expected = show_at / 1000 * 1000 + screen_offset;
        assert!((at - expected).abs() < 5_000_000, "{} ms off", (at - expected) as f64 / 1e6);
    }
}
