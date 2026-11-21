//! Lists the screens on the network and where each stands:
//!   cargo run --release --example find_screens
fn main() {
    let d = noctiluca::net::discovery::Discovery::start();
    let secs: u64 = std::env::args().nth(1).and_then(|s| s.parse().ok()).unwrap_or(4);
    std::thread::sleep(std::time::Duration::from_secs(secs));
    for f in d.found() {
        let state = if std::env::var_os("CHECK").is_some() && (f.android_tv || f.app_open) { format!("{:?}", noctiluca::net::adb::check(f.ip)) } else { String::new() };
        println!("{:<22} {:<16} app open: {:<5} android tv: {:<5} {:?} {}", f.name, f.ip, f.app_open, f.android_tv, f.size, state);
    }
}
