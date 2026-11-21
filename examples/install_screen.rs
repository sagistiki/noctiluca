//! Installs (or updates) the screen app on a box and opens it, as the
//! settings page's Install button does:
//!   cargo run --release --example install_screen -- 192.168.1.50
fn main() {
    let ip: std::net::IpAddr = std::env::args().nth(1).unwrap_or("192.168.1.50".into()).parse().unwrap();
    println!("before: {:?}", noctiluca::net::adb::check(ip));
    println!("install: {:?}", noctiluca::net::adb::install(ip));
    println!("after: {:?}", noctiluca::net::adb::check(ip));
}
