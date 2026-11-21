//! The screen's clock as seen from the Mac, from ping round trips (as NTP
//! does): the offset is trusted from the quickest trips, where the answer
//! cannot have waited long on the way either way.

/// The Mac's clock, in nanoseconds since the program began.
pub fn now_ns() -> i64 {
    use std::sync::OnceLock;
    use std::time::Instant;
    static START: OnceLock<Instant> = OnceLock::new();
    START.get_or_init(Instant::now).elapsed().as_nanos() as i64
}

/// How many recent round trips are kept.
const KEPT: usize = 32;

#[derive(Clone, Debug, Default)]
pub struct ClockSync {
    /// (round trip, offset) of recent pings, newest last.
    samples: Vec<(i64, i64)>,
}

impl ClockSync {
    /// A pong: the Mac sent at `sent`, it came back at `received`, the
    /// screen's clock read `screen` in between.
    pub fn add(&mut self, sent: i64, screen: i64, received: i64) {
        let rtt = received - sent;
        if rtt < 0 {
            return;
        }
        // The screen answered, as far as can be known, half way.
        let offset = screen - (sent + rtt / 2);
        self.samples.push((rtt, offset));
        if self.samples.len() > KEPT {
            self.samples.remove(0);
        }
    }

    pub fn ready(&self) -> bool {
        self.samples.len() >= 3
    }

    /// Screen time minus Mac time: the median offset of the quickest third
    /// of the trips (one slow answer does not move it).
    pub fn offset(&self) -> Option<i64> {
        if self.samples.is_empty() {
            return None;
        }
        let mut s = self.samples.clone();
        s.sort_by_key(|(rtt, _)| *rtt);
        let mut best: Vec<i64> = s.iter().take((s.len() / 3).max(1)).map(|(_, o)| *o).collect();
        best.sort();
        Some(best[best.len() / 2])
    }

    /// A typical round trip lately (the median), in ns.
    pub fn round_trip(&self) -> Option<i64> {
        let mut r: Vec<i64> = self.samples.iter().map(|(rtt, _)| *rtt).collect();
        r.sort();
        r.get(r.len() / 2).copied()
    }

    /// A slow round trip lately (the 90th percentile): how far the network
    /// can lag, for the latency margin.
    pub fn slow_trip(&self) -> Option<i64> {
        let mut r: Vec<i64> = self.samples.iter().map(|(rtt, _)| *rtt).collect();
        r.sort();
        (!r.is_empty()).then(|| r[(r.len() * 9 / 10).min(r.len() - 1)])
    }

    /// A Mac time in the screen's clock.
    pub fn to_screen(&self, mac_ns: i64) -> Option<i64> {
        self.offset().map(|o| mac_ns + o)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_offset_comes_from_the_quick_trips() {
        let true_offset = 5_000_000_000i64; // the screen's clock is 5 s ahead
        let mut c = ClockSync::default();
        assert_eq!(c.offset(), None);
        // Quick trips of 2 ms, symmetric.
        for k in 0..10 {
            let sent = k * 100_000_000;
            c.add(sent, sent + 1_000_000 + true_offset, sent + 2_000_000);
        }
        // Slow ones, where the answer sat 80 ms on the way back: they lie.
        for k in 10..20 {
            let sent = k * 100_000_000;
            c.add(sent, sent + 1_000_000 + true_offset, sent + 82_000_000);
        }
        assert!(c.ready());
        assert!((c.offset().unwrap() - true_offset).abs() < 100_000, "{:?}", c.offset());
        assert_eq!(c.to_screen(10), Some(10 + c.offset().unwrap()));
        assert!(c.slow_trip().unwrap() >= c.round_trip().unwrap());
    }

    #[test]
    fn keeps_recent_trips_only_and_ignores_impossible_ones() {
        let mut c = ClockSync::default();
        for k in 0..100 {
            c.add(k, k + 7, k + 2);
        }
        c.add(10, 0, 5); // came back before it was sent
        assert_eq!(c.samples.len(), KEPT);
        assert_eq!(c.offset(), Some(6));
    }
}
