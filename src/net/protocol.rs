//! The messages between Noctiluca and a network screen (the Android app in
//! android/, Protocol.kt). Each message: length (u32, of everything after
//! it), type (u8), payload; numbers little endian.

use std::io::{self, Read, Write};

use serde::{Deserialize, Serialize};

/// The screen app listens here.
pub const PORT: u16 = 47800;
/// And announces itself as this (mDNS / DNS-SD).
pub const SERVICE_TYPE: &str = "_noctiluca._tcp.local.";
pub const VERSION: u32 = 1;
/// Anything bigger is a broken stream.
pub const MAX_MESSAGE: usize = 16 * 1024 * 1024;

/// What the screen tells about itself when the Mac connects.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Hello {
    pub name: String,
    pub model: String,
    pub maker: String,
    pub android: String,
    pub width: u32,
    pub height: u32,
    pub version: u32,
    pub app: String,
    /// The HDR kinds the TV takes over HDMI ("HDR10", "HLG", "DOLBY_VISION",
    /// "HDR10_PLUS"); empty when it takes SDR only.
    pub hdr: Vec<String>,
}

/// How the screen is doing, once a second (counts since the stream began).
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Stats {
    pub decoded: u64,
    pub shown: u64,
    pub late: u64,
    pub dropped: u64,
    pub decode_ms: f64,
}

/// The stream's shape, sent before the first frame.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Config {
    pub width: u32,
    pub height: u32,
    pub fps: u32,
    /// Ask the TV for its game mode (HDMI ALLM).
    #[serde(default)]
    pub game_mode: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Message {
    Hello(Hello),
    /// The Mac's clock, in nanoseconds.
    Ping(i64),
    /// The Mac's time from the ping, and the screen's clock when it answered.
    Pong { mac_ns: i64, screen_ns: i64 },
    Config(Config),
    /// One H.264 access unit (Annex B), shown at `show_at` in the screen's clock.
    Frame { show_at: i64, key: bool, data: Vec<u8> },
    Stats(Stats),
    /// Stop showing: back to the screen's waiting picture.
    Idle,
}

const HELLO: u8 = 1;
const PING: u8 = 2;
const PONG: u8 = 3;
const CONFIG: u8 = 4;
const FRAME: u8 = 5;
const STATS: u8 = 6;
const IDLE: u8 = 7;

impl Message {
    /// The message as it goes on the wire.
    pub fn encode(&self) -> Vec<u8> {
        let (kind, payload): (u8, Vec<u8>) = match self {
            Message::Hello(h) => (HELLO, serde_json::to_vec(h).unwrap_or_default()),
            Message::Ping(t) => (PING, t.to_le_bytes().to_vec()),
            Message::Pong { mac_ns, screen_ns } => {
                let mut p = mac_ns.to_le_bytes().to_vec();
                p.extend_from_slice(&screen_ns.to_le_bytes());
                (PONG, p)
            }
            Message::Config(c) => (CONFIG, serde_json::to_vec(c).unwrap_or_default()),
            Message::Frame { show_at, key, data } => {
                let mut p = Vec::with_capacity(9 + data.len());
                p.extend_from_slice(&show_at.to_le_bytes());
                p.push(u8::from(*key));
                p.extend_from_slice(data);
                (FRAME, p)
            }
            Message::Stats(s) => (STATS, serde_json::to_vec(s).unwrap_or_default()),
            Message::Idle => (IDLE, Vec::new()),
        };
        let mut out = Vec::with_capacity(5 + payload.len());
        out.extend_from_slice(&(payload.len() as u32 + 1).to_le_bytes());
        out.push(kind);
        out.extend_from_slice(&payload);
        out
    }

    pub fn write(&self, w: &mut impl Write) -> io::Result<()> {
        w.write_all(&self.encode())
    }

    /// Reads one message. Unknown types are skipped (a newer screen app).
    pub fn read(r: &mut impl Read) -> io::Result<Message> {
        loop {
            let mut len = [0u8; 4];
            r.read_exact(&mut len)?;
            let len = u32::from_le_bytes(len) as usize;
            if len == 0 || len > MAX_MESSAGE {
                return Err(io::Error::new(io::ErrorKind::InvalidData, format!("bad message length {len}")));
            }
            let mut body = vec![0u8; len];
            r.read_exact(&mut body)?;
            let (kind, p) = (body[0], &body[1..]);
            let bad = |what: &str| io::Error::new(io::ErrorKind::InvalidData, what.to_string());
            let i64_at = |at: usize| -> io::Result<i64> {
                p.get(at..at + 8).map(|b| i64::from_le_bytes(b.try_into().unwrap())).ok_or_else(|| bad("short message"))
            };
            return Ok(match kind {
                HELLO => Message::Hello(serde_json::from_slice(p).map_err(|_| bad("bad hello"))?),
                PING => Message::Ping(i64_at(0)?),
                PONG => Message::Pong { mac_ns: i64_at(0)?, screen_ns: i64_at(8)? },
                CONFIG => Message::Config(serde_json::from_slice(p).map_err(|_| bad("bad config"))?),
                FRAME => {
                    let show_at = i64_at(0)?;
                    let key = *p.get(8).ok_or_else(|| bad("short frame"))? & 1 == 1;
                    Message::Frame { show_at, key, data: p[9..].to_vec() }
                }
                STATS => Message::Stats(serde_json::from_slice(p).map_err(|_| bad("bad stats"))?),
                IDLE => Message::Idle,
                _ => continue,
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_message_comes_back_as_sent() {
        let all = [
            Message::Hello(Hello { name: "living-room".into(), model: "MiTV-AFKR0".into(), width: 1920, height: 1080, version: 1, ..Hello::default() }),
            Message::Ping(-5),
            Message::Pong { mac_ns: 123_456_789_012, screen_ns: 987_654_321 },
            Message::Config(Config { width: 1920, height: 1080, fps: 60, game_mode: true }),
            Message::Frame { show_at: 42, key: true, data: vec![0, 0, 0, 1, 0x67, 1, 2, 3] },
            Message::Stats(Stats { decoded: 10, shown: 9, late: 1, dropped: 0, decode_ms: 7.5 }),
            Message::Idle,
        ];
        let mut wire = Vec::new();
        for m in &all {
            m.write(&mut wire).unwrap();
        }
        let mut r = wire.as_slice();
        for m in &all {
            assert_eq!(&Message::read(&mut r).unwrap(), m);
        }
        assert!(Message::read(&mut r).is_err(), "nothing left");
    }

    #[test]
    fn unknown_messages_are_skipped_and_broken_ones_refused() {
        let mut wire = vec![3, 0, 0, 0, 99, 1, 2];
        Message::Ping(7).write(&mut wire).unwrap();
        assert_eq!(Message::read(&mut wire.as_slice()).unwrap(), Message::Ping(7));
        let huge = ((MAX_MESSAGE + 1) as u32).to_le_bytes();
        assert!(Message::read(&mut huge.as_slice()).is_err());
        assert!(Message::read(&mut [1u8, 0, 0, 0, PING].as_slice()).is_err(), "a ping with no time");
    }

    /// The bytes the Android app writes (Protocol.kt): a pong, by hand.
    #[test]
    fn reads_what_the_screen_app_writes() {
        let mut wire = 17u32.to_le_bytes().to_vec();
        wire.push(3);
        wire.extend_from_slice(&1000i64.to_le_bytes());
        wire.extend_from_slice(&2000i64.to_le_bytes());
        assert_eq!(Message::read(&mut wire.as_slice()).unwrap(), Message::Pong { mac_ns: 1000, screen_ns: 2000 });
        let json = br#"{"name":"living-room","model":"MiTV-AFKR0","maker":"Xiaomi","android":"11","width":1920,"height":1080,"version":1,"app":"1.0"}"#;
        let mut wire = (json.len() as u32 + 1).to_le_bytes().to_vec();
        wire.push(1);
        wire.extend_from_slice(json);
        let Message::Hello(h) = Message::read(&mut wire.as_slice()).unwrap() else { panic!() };
        assert_eq!((h.name.as_str(), h.width, h.height), ("living-room", 1920, 1080));
    }
}
