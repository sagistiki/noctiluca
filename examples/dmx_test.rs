//! Lights a DMX light step by step to see what its channels do:
//!   cargo run --release --example dmx_test
//! Steps: white (channels 1-4), red, green, blue (channel 1 as dimmer
//! with 2, 3, 4), a slow colour fade, then off.
use std::time::{Duration, Instant};
use noctiluca::dmx::DmxOut;
use noctiluca::lights::UNIVERSE;

fn main() {
    let out = DmxOut::start(None);
    let t0 = Instant::now();
    while out.status().kind.is_none() && t0.elapsed() < Duration::from_secs(3) {
        std::thread::sleep(Duration::from_millis(50));
    }
    let s = out.status();
    println!("interface: {:?} {:?} {:?}", s.port, s.kind.map(|k| k.name()), s.error);
    let step = |name: &str, ch: &[(usize, u8)], secs: f32| {
        let mut u = [0u8; UNIVERSE];
        for (c, v) in ch {
            u[c - 1] = *v;
        }
        println!("{:5.1}s  {name}", t0.elapsed().as_secs_f32());
        out.set(&u);
        std::thread::sleep(Duration::from_secs_f32(secs));
    };
    step("white: channels 1-4 at full", &[(1, 255), (2, 255), (3, 255), (4, 255)], 3.0);
    step("red: 1 + 2", &[(1, 255), (2, 255)], 3.0);
    step("green: 1 + 3", &[(1, 255), (3, 255)], 3.0);
    step("blue: 1 + 4", &[(1, 255), (4, 255)], 3.0);
    println!("{:5.1}s  colour fade", t0.elapsed().as_secs_f32());
    let start = Instant::now();
    while start.elapsed() < Duration::from_secs(6) {
        let t = start.elapsed().as_secs_f32() / 6.0 * std::f32::consts::TAU;
        let c = |o: f32| (127.5 + 127.5 * (t + o).cos()) as u8;
        let mut u = [0u8; UNIVERSE];
        u[0] = 255;
        u[1] = c(0.0);
        u[2] = c(2.094);
        u[3] = c(4.189);
        out.set(&u);
        std::thread::sleep(Duration::from_millis(20));
    }
    step("off", &[], 1.0);
    println!("frames sent: {}  {:?}", out.status().frames, out.status().error);
}
