fn main() {
    let out = noctiluca::dmx::DmxOut::start(None);
    for _ in 0..8 {
        std::thread::sleep(std::time::Duration::from_millis(500));
        let s = out.status();
        println!("{:?} frames {} sending {} {:?}", s.kind, s.frames, s.sending, s.error);
    }
}
