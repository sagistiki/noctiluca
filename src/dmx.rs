//! Sends a DMX universe through a USB-DMX interface.
//!
//! USB-DMX interfaces are USB serial adapters (FTDI or CH340 chips; macOS
//! has the drivers). Two kinds, told apart by asking:
//!
//! * **Enttec DMX USB Pro** and its clones (DMXking, many "Pro" copies) have
//!   their own processor: they answer a "widget parameters" request and are
//!   sent the universe as a packet, which they put on the line themselves.
//! * **Open DMX** style ones (a bare FTDI chip) answer nothing: the computer
//!   makes the DMX signal itself, 250 000 baud 8N2, a break, a short mark,
//!   the start code and the 512 channels.
//!
//! A thread sends the latest universe 40 times a second; the show only
//! changes values in memory. If the interface goes away it says so and
//! opens it again when it is back. An interface that stops answering
//! without going away (a write that never returns) shows as stalled, and
//! the program starts the sender afresh (see `Status::stalled`).

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use crate::lights::UNIVERSE;

/// Frames a second (DMX allows up to 44 with 512 channels).
const RATE: f64 = 40.0;
/// No frame out for this long while sending: the interface has stopped
/// answering.
pub const STALLED: Duration = Duration::from_millis(1500);
/// How often a session checks that its port is still there.
const CHECK_PORT: Duration = Duration::from_secs(1);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// Enttec DMX USB Pro (or a clone): packets.
    Pro,
    /// A bare FTDI chip: the signal made here.
    Open,
}

impl Kind {
    pub fn name(self) -> &'static str {
        match self {
            Kind::Pro => "DMX USB Pro",
            Kind::Open => "Open DMX",
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct Status {
    pub port: Option<String>,
    pub kind: Option<Kind>,
    pub sending: bool,
    pub frames: u64,
    pub error: Option<String>,
    /// When the last frame went out (None before the first).
    pub last_frame: Option<Instant>,
}

impl Status {
    /// Meant to be sending, but no frame has gone out for a while: a write
    /// is stuck on an interface that stopped answering.
    pub fn stalled(&self) -> bool {
        self.stalled_at(Instant::now())
    }

    fn stalled_at(&self, now: Instant) -> bool {
        self.sending && self.last_frame.is_some_and(|t| now.saturating_duration_since(t) > STALLED)
    }
}

/// The ports that look like USB-DMX interfaces (USB serial adapters).
pub fn ports() -> Vec<String> {
    let mut out: Vec<String> = std::fs::read_dir("/dev")
        .map(|d| {
            d.filter_map(|e| e.ok())
                .map(|e| e.file_name().to_string_lossy().to_string())
                .filter(|n| n.starts_with("cu.usbserial") || n.starts_with("cu.wchusbserial") || n.starts_with("cu.usbmodem") || n.starts_with("cu.SLAB_USBtoUART"))
                .map(|n| format!("/dev/{n}"))
                .collect()
        })
        .unwrap_or_default();
    out.sort();
    out
}

/// A port's short name for the screen ("usbserial-A10KXYZ").
pub fn short_name(port: &str) -> &str {
    port.strip_prefix("/dev/cu.").unwrap_or(port)
}

struct Shared {
    universe: Mutex<[u8; UNIVERSE]>,
    status: Mutex<Status>,
    /// The port to use; `None`: the first found.
    wanted: Mutex<Option<String>>,
    stop: AtomicBool,
}

pub struct DmxOut {
    shared: Arc<Shared>,
}

impl DmxOut {
    pub fn start(port: Option<String>) -> Self {
        let shared = Arc::new(Shared {
            universe: Mutex::new([0; UNIVERSE]),
            status: Mutex::new(Status::default()),
            wanted: Mutex::new(port),
            stop: AtomicBool::new(false),
        });
        let s = shared.clone();
        thread::Builder::new().name("dmx".into()).spawn(move || run(s)).ok();
        DmxOut { shared }
    }

    /// The values to send from now on.
    pub fn set(&self, values: &[u8; UNIVERSE]) {
        if let Ok(mut u) = self.shared.universe.lock() {
            *u = *values;
        }
    }

    /// Use this port (or the first found, with `None`).
    pub fn set_port(&self, port: Option<String>) {
        if let Ok(mut w) = self.shared.wanted.lock() {
            *w = port;
        }
    }

    pub fn status(&self) -> Status {
        self.shared.status.lock().map(|s| s.clone()).unwrap_or_default()
    }
}

impl Drop for DmxOut {
    fn drop(&mut self) {
        self.shared.stop.store(true, Ordering::Relaxed);
    }
}

fn set_status(shared: &Shared, f: impl FnOnce(&mut Status)) {
    if let Ok(mut s) = shared.status.lock() {
        f(&mut s);
    }
}

fn run(shared: Arc<Shared>) {
    while !shared.stop.load(Ordering::Relaxed) {
        let wanted = shared.wanted.lock().ok().and_then(|w| w.clone());
        let found = ports();
        let port = match &wanted {
            Some(p) if found.contains(p) => Some(p.clone()),
            Some(_) => None,
            None => found.first().cloned(),
        };
        let Some(port) = port else {
            set_status(&shared, |s| {
                *s = Status { error: Some(if found.is_empty() { "No USB-DMX interface connected".into() } else { "The chosen interface is not connected".into() }), ..Status::default() };
            });
            thread::sleep(Duration::from_secs(1));
            continue;
        };
        if let Err(e) = session(&shared, &port, wanted.as_deref()) {
            set_status(&shared, |s| {
                s.sending = false;
                s.error = Some(format!("{e:#}"));
            });
            thread::sleep(Duration::from_secs(1));
        }
    }
    // Lights off when Noctiluca closes is up to the last frame sent: an
    // interface keeps sending it (Pro) or the lights hold it (Open).
}

/// Opens `port`, finds out what it is, and sends until it fails or another
/// port is chosen.
fn session(shared: &Shared, port: &str, wanted: Option<&str>) -> anyhow::Result<()> {
    let serial = Serial::open(port)?;
    let kind = if serial.is_pro() { Kind::Pro } else { Kind::Open };
    if kind == Kind::Open {
        serial.set_dmx_line()?;
    }
    set_status(shared, |s| {
        *s = Status { port: Some(port.to_string()), kind: Some(kind), sending: true, frames: 0, error: None, last_frame: Some(Instant::now()) };
    });
    let period = Duration::from_secs_f64(1.0 / RATE);
    let mut next = Instant::now();
    let mut checked = Instant::now();
    loop {
        if shared.stop.load(Ordering::Relaxed) {
            return Ok(());
        }
        // Unplugged: some interfaces take writes into the void for a while
        // before failing, so look for the port itself.
        if checked.elapsed() >= CHECK_PORT {
            checked = Instant::now();
            if !std::path::Path::new(port).exists() {
                anyhow::bail!("{} was unplugged", short_name(port));
            }
        }
        let now_wanted = shared.wanted.lock().ok().and_then(|w| w.clone());
        if now_wanted.as_deref() != wanted && !(wanted.is_none() && now_wanted.as_deref() == Some(port)) {
            return Ok(());
        }
        let universe = *shared.universe.lock().map_err(|_| anyhow::anyhow!("poisoned"))?;
        match kind {
            Kind::Pro => serial.write_all(&pro_packet(&universe))?,
            Kind::Open => serial.send_open_frame(&universe)?,
        }
        set_status(shared, |s| {
            s.frames += 1;
            s.last_frame = Some(Instant::now());
        });
        next += period;
        let now = Instant::now();
        if next > now {
            thread::sleep(next - now);
        } else {
            next = now;
        }
    }
}

/// The Enttec Pro "send DMX" packet: label 6, start code 0, the channels.
pub fn pro_packet(universe: &[u8; UNIVERSE]) -> Vec<u8> {
    let len = UNIVERSE + 1;
    let mut p = Vec::with_capacity(len + 5);
    p.extend_from_slice(&[0x7E, 6, (len & 0xFF) as u8, (len >> 8) as u8, 0]);
    p.extend_from_slice(universe);
    p.push(0xE7);
    p
}

/// A serial port, raw, through the system's calls.
struct Serial {
    fd: libc::c_int,
}

/// macOS: set any baud rate (`IOSSIOSPEED` in IOKit/serial/ioss.h).
const IOSSIOSPEED: libc::c_ulong = 0x8008_5402;

impl Serial {
    fn open(port: &str) -> anyhow::Result<Self> {
        let path = std::ffi::CString::new(port)?;
        let fd = unsafe { libc::open(path.as_ptr(), libc::O_RDWR | libc::O_NOCTTY | libc::O_NONBLOCK) };
        if fd < 0 {
            anyhow::bail!("cannot open {}: {}", short_name(port), std::io::Error::last_os_error());
        }
        let s = Serial { fd };
        // Ours alone: two programs writing to one interface garble each other.
        if unsafe { libc::ioctl(fd, libc::TIOCEXCL as libc::c_ulong) } != 0 {
            anyhow::bail!("{} is in use by another program", short_name(port));
        }
        unsafe {
            // Blocking from here on, raw, 8 bits, two stop bits, no flow control.
            let flags = libc::fcntl(fd, libc::F_GETFL);
            libc::fcntl(fd, libc::F_SETFL, flags & !libc::O_NONBLOCK);
            let mut t: libc::termios = std::mem::zeroed();
            if libc::tcgetattr(fd, &mut t) != 0 {
                anyhow::bail!("{} is not a serial port", short_name(port));
            }
            libc::cfmakeraw(&mut t);
            t.c_cflag |= libc::CS8 | libc::CSTOPB | libc::CLOCAL | libc::CREAD;
            t.c_cflag &= !(libc::PARENB | libc::CRTSCTS);
            t.c_cc[libc::VMIN] = 0;
            t.c_cc[libc::VTIME] = 2; // reads give up after 0.2 s
            libc::cfsetspeed(&mut t, libc::B57600);
            libc::tcsetattr(fd, libc::TCSANOW, &t);
            libc::tcflush(fd, libc::TCIOFLUSH);
        }
        Ok(s)
    }

    /// Asks the Pro question: an Enttec Pro answers its parameters (label 3).
    fn is_pro(&self) -> bool {
        if self.write_all(&[0x7E, 3, 2, 0, 0, 0, 0xE7]).is_err() {
            return false;
        }
        let mut got = Vec::new();
        let until = Instant::now() + Duration::from_millis(400);
        let mut buf = [0u8; 64];
        while Instant::now() < until {
            let n = unsafe { libc::read(self.fd, buf.as_mut_ptr().cast(), buf.len()) };
            if n > 0 {
                got.extend_from_slice(&buf[..n as usize]);
                if got.windows(2).any(|w| w == [0x7E, 3]) {
                    return true;
                }
            }
        }
        false
    }

    /// 250 000 baud for the DMX signal itself.
    fn set_dmx_line(&self) -> anyhow::Result<()> {
        let speed: libc::speed_t = 250_000;
        let r = unsafe { libc::ioctl(self.fd, IOSSIOSPEED, &speed) };
        if r != 0 {
            anyhow::bail!("the interface would not take DMX speed: {}", std::io::Error::last_os_error());
        }
        Ok(())
    }

    /// One DMX frame made here: break (at least 88 µs), mark after break,
    /// start code 0, the channels.
    fn send_open_frame(&self, universe: &[u8; UNIVERSE]) -> anyhow::Result<()> {
        unsafe {
            libc::tcdrain(self.fd);
            libc::ioctl(self.fd, libc::TIOCSBRK as libc::c_ulong);
        }
        spin(Duration::from_micros(110));
        unsafe {
            libc::ioctl(self.fd, libc::TIOCCBRK as libc::c_ulong);
        }
        spin(Duration::from_micros(16));
        let mut frame = [0u8; UNIVERSE + 1];
        frame[1..].copy_from_slice(universe);
        self.write_all(&frame)
    }

    fn write_all(&self, mut data: &[u8]) -> anyhow::Result<()> {
        while !data.is_empty() {
            let n = unsafe { libc::write(self.fd, data.as_ptr().cast(), data.len()) };
            if n <= 0 {
                anyhow::bail!("the interface stopped taking data: {}", std::io::Error::last_os_error());
            }
            data = &data[n as usize..];
        }
        Ok(())
    }
}

/// Waits a few microseconds exactly (sleep is too coarse for DMX timing).
fn spin(d: Duration) {
    let until = Instant::now() + d;
    while Instant::now() < until {
        std::hint::spin_loop();
    }
}

impl Drop for Serial {
    fn drop(&mut self) {
        unsafe {
            libc::close(self.fd);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_pro_packet_is_label_six_with_start_code() {
        let mut u = [0u8; UNIVERSE];
        u[0] = 255;
        u[511] = 7;
        let p = pro_packet(&u);
        assert_eq!(p.len(), 518);
        assert_eq!(&p[..6], &[0x7E, 6, 0x01, 0x02, 0, 255]);
        assert_eq!(p[516], 7);
        assert_eq!(p[517], 0xE7);
    }

    #[test]
    fn a_sender_with_no_frame_lately_is_stalled() {
        let now = Instant::now();
        let sent = |ago_ms: u64, sending: bool| Status { sending, last_frame: now.checked_sub(Duration::from_millis(ago_ms)), ..Status::default() };
        assert!(!sent(25, true).stalled_at(now));
        assert!(sent(2000, true).stalled_at(now));
        // Not meant to be sending (no interface, or between tries): not stalled.
        assert!(!sent(2000, false).stalled_at(now));
        assert!(!Status::default().stalled_at(now));
    }

    #[test]
    fn only_usb_serial_ports_are_offered() {
        // Bluetooth and the debug console are never DMX interfaces.
        assert!(ports().iter().all(|p| !p.contains("Bluetooth") && !p.contains("debug") && p.starts_with("/dev/cu.")));
        assert_eq!(short_name("/dev/cu.usbserial-A10K"), "usbserial-A10K");
    }
}
