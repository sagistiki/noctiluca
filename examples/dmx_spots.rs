//! Tests two spots at DMX addresses given (default 3 and 12), step by step.
use std::time::{Duration, Instant};
use noctiluca::dmx::DmxOut;
use noctiluca::lights::UNIVERSE;

fn main() {
    let a: Vec<usize> = std::env::args().skip(1).filter_map(|s| s.parse().ok()).collect();
    let addrs = if a.is_empty() { vec![3, 12] } else { a };
    let out = DmxOut::start(None);
    let t0 = Instant::now();
    while out.status().kind.is_none() && t0.elapsed() < Duration::from_secs(3) {
        std::thread::sleep(Duration::from_millis(50));
    }
    println!("interface: {:?} {:?} {:?}", out.status().port, out.status().kind, out.status().error);
    // Offsets from each light's address: (name, [(offset, value)]).
    let steps: Vec<(&str, Vec<(usize, u8)>)> = vec![
        ("A  first channel only (dimmer?)", vec![(0, 255)]),
        ("B  dimmer + 2nd (red?)", vec![(0, 255), (1, 255)]),
        ("C  dimmer + 3rd (green?)", vec![(0, 255), (2, 255)]),
        ("D  dimmer + 4th (blue?)", vec![(0, 255), (3, 255)]),
        ("E  1st, 2nd, 3rd only (RGB without dimmer: white?)", vec![(0, 255), (1, 255), (2, 255)]),
        ("F  off", vec![]),
    ];
    for (name, chans) in &steps {
        let mut u = [0u8; UNIVERSE];
        for base in &addrs {
            for (o, v) in chans {
                u[base - 1 + o] = *v;
            }
        }
        out.set(&u);
        println!("{:5.1}s  {name}   connected: {}", t0.elapsed().as_secs_f32(), out.status().sending);
        std::thread::sleep(Duration::from_secs(4));
    }
    println!("frames {} {:?}", out.status().frames, out.status().error);
}
