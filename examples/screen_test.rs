//! Streams a test picture to a network screen and reports how it does:
//!   cargo run --release --example screen_test -- 192.168.1.50 20
//! A bar sweeps across; a white square flashes on every beat at 120 BPM of
//! the Mac's clock, drawn for the moment the screen shows it.
use std::time::{Duration, Instant};
use noctiluca::net::clock::now_ns;
use noctiluca::net::stream::{Picture, ScreenStream};

fn main() {
    let mut args = std::env::args().skip(1);
    let ip = args.next().unwrap_or("192.168.1.50".into());
    let secs: u64 = args.next().and_then(|s| s.parse().ok()).unwrap_or(15);
    let addr = format!("{ip}:{}", noctiluca::net::protocol::PORT).parse().unwrap();
    let stream = ScreenStream::start(addr, 60, 10_000_000);
    let t0 = Instant::now();
    while !stream.ready() && t0.elapsed() < Duration::from_secs(5) {
        std::thread::sleep(Duration::from_millis(20));
    }
    println!("ready: {} {:?}", stream.ready(), stream.status().hello);
    let (w, h) = (1920u32, 1080u32);
    let stride = (w * 4) as usize;
    let mut sent = 0;
    let start = Instant::now();
    let mut next = Instant::now();
    let mut last_report = Instant::now();
    while start.elapsed() < Duration::from_secs(secs) {
        let lead = stream.latency_secs();
        let show_at = now_ns() + (lead * 1e9) as i64;
        let t = show_at as f64 / 1e9;
        let beat_phase = (t * 2.0).fract();
        let bar = ((t * 0.5).fract() * w as f64) as usize;
        let mut px = vec![0u8; stride * h as usize];
        for y in 0..h as usize {
            let row = &mut px[y * stride..(y + 1) * stride];
            for x in 0..w as usize {
                let i = x * 4;
                let on_bar = x.abs_diff(bar) < 40;
                let square = beat_phase < 0.1 && x.abs_diff(960) < 200 && y.abs_diff(540) < 200;
                let (b, g, r) = if square { (255, 255, 255) } else if on_bar { (214, 43, 255) } else { ((x / 8) as u8, (y / 6) as u8, 40) };
                row[i] = b; row[i + 1] = g; row[i + 2] = r; row[i + 3] = 255;
            }
        }
        if stream.offer(Picture { bgra: px, stride, width: w, height: h, show_at }) { sent += 1; }
        if last_report.elapsed() > Duration::from_secs(2) {
            let s = stream.status();
            println!("{:>4.1}s sent {sent:4}  latency {:5.1} ms  rtt {:5.1} ms  shown {:5}  late {:3}  dropped {:3}/{:3}  decode {:4.1} ms  {}",
                start.elapsed().as_secs_f32(), s.latency_ms, s.round_trip_ms, s.stats.shown, s.stats.late, s.stats.dropped, s.dropped_here, s.stats.decode_ms,
                s.error.unwrap_or_default());
            last_report = Instant::now();
        }
        next += Duration::from_micros(16_667);
        if let Some(d) = next.checked_duration_since(Instant::now()) { std::thread::sleep(d); }
    }
}
