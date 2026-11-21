//! What each surface of the projector shows, worked out from the settings:
//! which picture (Main, the shared Complement, or Main one beat late), whether
//! it is mirrored, and the chase. Pure logic; src/render/projector.rs draws it.

use glam::Vec2;

use crate::mapping::Corners;
use crate::settings::{ComplementStyle, Content, Surface};

/// Which picture a surface shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Source {
    Main,
    Complement,
    /// Main's frames one beat late.
    Echo,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Planned {
    pub source: Source,
    /// Which surface (0..3): its colour and number.
    pub index: usize,
    pub corners: Corners,
    pub flip: bool,
    pub feather_px: f32,
    pub brightness: f32,
    pub chase: f32,
    /// Its real shape, if given (see `Surface::aspect`).
    pub aspect: Option<f32>,
}

/// The most a chase brightens a surface: a sweep, not a flash.
pub const CHASE_MAX: f32 = 0.2;
const PHRASE_BEATS: f32 = 16.0;
const LAST_BAR: f32 = 12.0;

/// The plan for the visible surfaces on output `output`. Same surfaces
/// share Main's picture; Complement surfaces share one picture, the second
/// of them on this output mirrored in Mirror motion so the stage is symmetric.
pub fn plan(surfaces: &[Surface], output: usize, strobe: bool, beats: f32) -> Vec<Planned> {
    let here: Vec<(usize, &Surface)> = surfaces
        .iter()
        .enumerate()
        .filter(|(_, s)| s.output == output && s.visible)
        .collect();
    let complements: Vec<usize> = here
        .iter()
        .filter(|(_, s)| s.content == Content::Complement)
        .map(|(i, _)| *i)
        .collect();
    here.iter()
        .map(|&(i, s)| {
            let k = complements.iter().position(|c| *c == i);
            let (source, flip, chase_amount) = match (s.content, s.style) {
                (Content::Main | Content::Same, _) => (Source::Main, s.flip, 0.0),
                (Content::Complement, ComplementStyle::MirrorMotion) => (
                    Source::Complement,
                    s.flip ^ (k.is_some_and(|k| k % 2 == 1)),
                    0.0,
                ),
                (Content::Complement, ComplementStyle::Chase) => (
                    Source::Complement,
                    s.flip,
                    chase(k.unwrap_or(0), complements.len(), beats, strobe),
                ),
                (Content::Complement, ComplementStyle::Echo) => (Source::Echo, s.flip, 0.0),
            };
            Planned {
                source,
                index: i,
                corners: s.corners,
                flip,
                feather_px: s.feather_px,
                brightness: s.brightness,
                chase: chase_amount,
                aspect: s.aspect,
            }
        })
        .collect()
}

/// The chase's extra brightness for complement number `k` of `count`: in
/// the last bar of every 16-beat phrase a soft light moves from one
/// complement to the next, one per beat, fading within the beat. Nothing
/// with the strobe off.
pub fn chase(k: usize, count: usize, beats: f32, strobe: bool) -> f32 {
    if !strobe || count == 0 || !beats.is_finite() {
        return 0.0;
    }
    let in_phrase = beats.rem_euclid(PHRASE_BEATS);
    if in_phrase < LAST_BAR {
        return 0.0;
    }
    let step = (in_phrase - LAST_BAR).floor() as usize;
    if step % count != k {
        return 0.0;
    }
    let fade = 1.0 - in_phrase.fract();
    CHASE_MAX * fade * fade
}

/// The size to draw a picture at for surfaces of `extents` (their sizes in
/// output pixels, `mapping::extent`): wide and tall enough for each of them
/// to show its middle in its own shape, no taller than `max_height`, never
/// smaller than 64 pixels a side; `fallback` when there are none.
pub fn view_size(
    extents: impl IntoIterator<Item = Vec2>,
    fallback: (u32, u32),
    max_height: u32,
) -> (u32, u32) {
    let (mut w, mut h) = (0.0f32, 0.0f32);
    for e in extents {
        w = w.max(e.x);
        h = h.max(e.y);
    }
    if w < 1.0 || h < 1.0 {
        return (fallback.0.max(64), fallback.1.max(64));
    }
    let scale = (max_height as f32 / h).min(1.0);
    (
        (w * scale).round().max(64.0) as u32,
        (h * scale).round().max(64.0) as u32,
    )
}

/// How many frames make a beat: how far back the Echo looks.
pub fn echo_frames(fps: f32, bpm: f32) -> usize {
    if !(fps > 0.0 && bpm > 0.0) {
        return 30;
    }
    ((fps * 60.0 / bpm).ceil() as usize).clamp(1, 40)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mapping::default_corners;

    fn surface(content: Content, style: ComplementStyle, i: usize, n: usize) -> Surface {
        Surface {
            content,
            style,
            corners: default_corners(i, n),
            ..Surface::default()
        }
    }

    #[test]
    fn same_surfaces_share_the_main_view() {
        let s = [
            surface(Content::Main, ComplementStyle::MirrorMotion, 0, 2),
            surface(Content::Same, ComplementStyle::Echo, 1, 2),
        ];
        let p = plan(&s, 0, true, 0.0);
        assert!(p.iter().all(|x| x.source == Source::Main));
    }

    #[test]
    fn two_complements_share_one_view_and_mirror_motion_flips_the_second() {
        let s = [
            surface(Content::Complement, ComplementStyle::MirrorMotion, 0, 3),
            surface(Content::Main, ComplementStyle::MirrorMotion, 1, 3),
            surface(Content::Complement, ComplementStyle::MirrorMotion, 2, 3),
        ];
        let p = plan(&s, 0, true, 0.0);
        assert_eq!(p[0].source, Source::Complement);
        assert_eq!(p[2].source, Source::Complement);
        assert!(!p[0].flip && p[2].flip, "left plain, right mirrored");
        assert!(!p[1].flip);
    }

    #[test]
    fn chase_is_soft_short_and_off_without_strobe() {
        // Over a whole phrase: nothing in the first three bars.
        for step in 0..(16 * 20) {
            let beats = step as f32 / 20.0;
            for k in 0..2 {
                let c = chase(k, 2, beats, true);
                assert!((0.0..=CHASE_MAX).contains(&c));
                if beats < 12.0 {
                    assert_eq!(c, 0.0, "chase at {beats}");
                }
                assert_eq!(chase(k, 2, beats, false), 0.0);
            }
        }
        // In the last bar it takes turns: beat 12 the first, 13 the second.
        assert!(chase(0, 2, 12.05, true) > 0.15 && chase(1, 2, 12.05, true) == 0.0);
        assert!(chase(1, 2, 13.05, true) > 0.15 && chase(0, 2, 13.05, true) == 0.0);
        assert!(chase(0, 2, 12.95, true) < 0.01, "it fades within the beat");
    }

    #[test]
    fn echo_looks_one_beat_back() {
        assert_eq!(echo_frames(60.0, 120.0), 30);
        assert_eq!(echo_frames(60.0, 128.0), 29);
        assert_eq!(echo_frames(60.0, 0.0), 30);
        assert!(echo_frames(240.0, 60.0) <= 40);
    }

    #[test]
    fn views_are_sized_by_their_quads() {
        // Three surfaces side by side on 1920 x 1080: each about a third wide.
        let quads: Vec<Corners> = (0..3).map(|i| default_corners(i, 3)).collect();
        let px = (1920, 1080);
        let ext = |c: &Corners| crate::mapping::extent(c, px, None);
        let (w, h) = view_size([ext(&quads[0])], px, 1080);
        assert!(
            (560..=600).contains(&w) && (900..=910).contains(&h),
            "{w} x {h}"
        );
        // A cap on height keeps the shape.
        let (w2, h2) = view_size([ext(&quads[0])], px, 720);
        assert_eq!(h2, 720);
        assert!(((w2 as f32 / h2 as f32) - (w as f32 / h as f32)).abs() < 0.01);
        // The full frame is the output's size.
        assert_eq!(
            view_size([ext(&crate::mapping::FULL_FRAME)], px, 1080),
            (1920, 1080)
        );
        // A wide surface and a tall one: the picture covers both.
        let wide: Corners = [[0.0, 0.0], [1.0, 0.0], [1.0, 0.3], [0.0, 0.3]];
        let tall: Corners = [[0.4, 0.0], [0.6, 0.0], [0.6, 1.0], [0.4, 1.0]];
        let (w3, h3) = view_size([ext(&wide), ext(&tall)], px, 1080);
        assert_eq!((w3, h3), (1920, 1080));
    }

    #[test]
    fn each_output_plans_only_its_visible_surfaces() {
        let mut s = vec![
            surface(Content::Main, ComplementStyle::MirrorMotion, 0, 2),
            surface(Content::Complement, ComplementStyle::MirrorMotion, 1, 2),
            surface(Content::Complement, ComplementStyle::MirrorMotion, 0, 2),
        ];
        s[2].output = 1;
        s[1].visible = false;
        let first = plan(&s, 0, true, 0.0);
        assert_eq!(first.iter().map(|p| p.index).collect::<Vec<_>>(), vec![0]);
        let second = plan(&s, 1, true, 0.0);
        assert_eq!(second.iter().map(|p| p.index).collect::<Vec<_>>(), vec![2]);
        assert!(
            !second[0].flip,
            "the only complement on its output is not mirrored"
        );
    }
}
