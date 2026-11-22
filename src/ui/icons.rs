//! Small line icons for the editor's tools, drawn as vectors so they stay
//! sharp at any size (egui has no icon set of its own, and Inter has few).

use egui::{Color32, Pos2, Rect, Response, Sense, Shape, Stroke, Ui, Vec2, pos2, vec2};

use super::style;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Icon {
    Transform,
    CornerPin,
    Add,
    Duplicate,
    Delete,
    RotateLeft,
    RotateRight,
    Flip,
    Fit,
    Reset,
    Snap,
    Grid,
    Eye,
    EyeOff,
    Lock,
    Unlock,
    Surfaces,
    Move,
    Look,
    Output,
    /// The sidebar's sections and the toolbar.
    Pulse,
    Display,
    Spot,
    Wifi,
    Pin,
    Gear,
    Play,
    Stop,
    Expand,
    Bolt,
    Snow,
    Moon,
    Next,
    Palette,
    Smoke,
    Hold,
    Power,
    /// The Lights page.
    Sparkle,
    Burst,
    Split,
    Wave,
    Chase,
    Breathe,
    Riser,
    Sun,
    Gauge,
    Clock,
    Plug,
    Target,
    Search,
    BoltOff,
    Bolts,
    Warning,
    /// Two arrows chasing round a circle: look again, reconnect.
    Refresh,
}

/// Draws `icon` inside `rect` in `colour`.
pub fn paint(ui: &Ui, rect: Rect, icon: Icon, colour: Color32) {
    let p = ui.painter();
    let s = rect.width().min(rect.height());
    let c = rect.center();
    let at = |x: f32, y: f32| c + vec2(x, y) * s / 2.0;
    let line = Stroke::new((s / 12.0).max(1.4), colour);
    let thin = Stroke::new((s / 16.0).max(1.1), colour);
    let poly = |pts: Vec<Pos2>, closed: bool| {
        if closed {
            Shape::closed_line(pts, line)
        } else {
            Shape::line(pts, line)
        }
    };
    let arc = |from: f32, to: f32, r: f32| -> Vec<Pos2> {
        (0..=16)
            .map(|k| {
                let a = from + (to - from) * k as f32 / 16.0;
                at(r * a.cos(), r * a.sin())
            })
            .collect()
    };
    // An arrow round 280° of a circle, its head following the arc's direction
    // at the end. Screen angles grow clockwise (y is down).
    let circular_arrow = |mirror: bool| {
        let (from, to) = (-std::f32::consts::FRAC_PI_4 - 4.9, -std::f32::consts::FRAC_PI_4);
        let flip = |q: Pos2| if mirror { pos2(2.0 * c.x - q.x, q.y) } else { q };
        let pts: Vec<Pos2> = arc(from, to - 0.25, 0.62).into_iter().map(flip).collect();
        p.add(Shape::line(pts, line));
        // The head: a filled triangle along the tangent at the end.
        let end = at(0.62 * to.cos(), 0.62 * to.sin());
        let dir = Vec2::new(-to.sin(), to.cos());
        let side = Vec2::new(-dir.y, dir.x);
        let tip = end + dir * s * 0.16;
        let back = end - dir * s * 0.2;
        let mut head = vec![flip(tip), flip(back + side * s * 0.22), flip(back - side * s * 0.22)];
        if mirror {
            head.reverse(); // keep the winding egui expects
        }
        p.add(Shape::convex_polygon(head, colour, Stroke::NONE));
    };
    match icon {
        Icon::Sparkle => {
            // A four-pointed star, and a small one beside it: automatic.
            let star = |cx: f32, cy: f32, r: f32| {
                let k = 0.28;
                vec![at(cx, cy - r), at(cx + r * k, cy - r * k), at(cx + r, cy), at(cx + r * k, cy + r * k), at(cx, cy + r), at(cx - r * k, cy + r * k), at(cx - r, cy), at(cx - r * k, cy - r * k)]
            };
            p.add(poly(star(-0.15, 0.1, 0.75), true));
            p.add(Shape::closed_line(star(0.6, -0.6, 0.3), thin));
        }
        Icon::Burst => {
            p.circle_filled(c, s * 0.12, colour);
            for k in 0..8 {
                let a = k as f32 * std::f32::consts::TAU / 8.0;
                p.add(poly(vec![at(0.35 * a.cos(), 0.35 * a.sin()), at(0.85 * a.cos(), 0.85 * a.sin())], false));
            }
        }
        Icon::Split => {
            p.circle_stroke(c, s * 0.42, line);
            let half: Vec<Pos2> = arc(-std::f32::consts::FRAC_PI_2, std::f32::consts::FRAC_PI_2, 0.84);
            p.add(Shape::convex_polygon(half, colour, Stroke::NONE));
            p.add(poly(vec![at(0.0, -0.84), at(0.0, 0.84)], false));
        }
        Icon::Wave => {
            let pts: Vec<Pos2> = (0..=24).map(|k| { let t = k as f32 / 24.0 * 2.0 - 1.0; at(t * 0.9, -0.45 * (t * std::f32::consts::PI * 1.5).sin()) }).collect();
            p.add(Shape::line(pts, line));
        }
        Icon::Chase => {
            for (k, x) in [-0.7f32, -0.15, 0.4].into_iter().enumerate() {
                let r = s * (0.07 + 0.035 * k as f32);
                p.circle_filled(at(x, 0.0), r, colour.gamma_multiply(0.45 + 0.27 * k as f32));
            }
            p.add(poly(vec![at(0.62, -0.32), at(0.9, 0.0), at(0.62, 0.32)], false));
        }
        Icon::Breathe => {
            p.circle_stroke(c, s * 0.42, line);
            p.circle_stroke(c, s * 0.24, thin);
            p.circle_filled(c, s * 0.08, colour);
        }
        Icon::Riser => {
            for (k, h) in [0.35f32, 0.6, 0.85].into_iter().enumerate() {
                let x = -0.6 + 0.55 * k as f32;
                p.add(poly(vec![at(x, 0.85), at(x, 0.85 - 2.0 * h * 0.85)], false));
            }
            p.add(poly(vec![at(0.35, -0.6), at(0.65, -0.88), at(0.92, -0.6)], false));
        }
        Icon::Sun => {
            p.circle_stroke(c, s * 0.22, line);
            for k in 0..8 {
                let a = k as f32 * std::f32::consts::TAU / 8.0;
                p.add(Shape::line(vec![at(0.62 * a.cos(), 0.62 * a.sin()), at(0.88 * a.cos(), 0.88 * a.sin())], thin));
            }
        }
        Icon::Gauge => {
            p.add(Shape::line(arc(std::f32::consts::PI * 0.85, std::f32::consts::PI * 2.15, 0.82), line));
            p.add(poly(vec![at(0.0, 0.25), at(0.45, -0.35)], false));
            p.circle_filled(at(0.0, 0.25), s * 0.08, colour);
        }
        Icon::Clock => {
            p.circle_stroke(c, s * 0.42, line);
            p.add(poly(vec![at(0.0, -0.5), at(0.0, 0.0), at(0.35, 0.25)], false));
        }
        Icon::Plug => {
            p.add(poly(vec![at(-0.45, -0.25), at(0.45, -0.25), at(0.45, 0.15), at(0.0, 0.55), at(-0.45, 0.15)], true));
            p.add(poly(vec![at(-0.2, -0.25), at(-0.2, -0.75)], false));
            p.add(poly(vec![at(0.2, -0.25), at(0.2, -0.75)], false));
            p.add(poly(vec![at(0.0, 0.55), at(0.0, 0.9)], false));
        }
        Icon::Target => {
            p.circle_stroke(c, s * 0.42, line);
            p.circle_filled(c, s * 0.1, colour);
            for (x, y) in [(0.0, -1.0), (0.0, 1.0), (-1.0, 0.0), (1.0, 0.0)] {
                p.add(Shape::line(vec![at(x * 0.6, y * 0.6), at(x * 0.95, y * 0.95)], thin));
            }
        }
        Icon::Search => {
            p.circle_stroke(at(-0.15, -0.15), s * 0.3, line);
            p.add(poly(vec![at(0.3, 0.3), at(0.85, 0.85)], false));
        }
        Icon::BoltOff => {
            p.add(Shape::closed_line(vec![at(0.15, -0.9), at(-0.55, 0.15), at(0.0, 0.15), at(-0.15, 0.9), at(0.55, -0.15), at(0.0, -0.15)], thin));
            p.add(poly(vec![at(-0.85, -0.85), at(0.85, 0.85)], false));
        }
        Icon::Bolts => {
            let bolt = |dx: f32| vec![at(0.1 + dx, -0.85), at(-0.4 + dx, 0.1), at(-0.02 + dx, 0.1), at(-0.12 + dx, 0.85), at(0.4 + dx, -0.12), at(0.02 + dx, -0.12)];
            p.add(poly(bolt(-0.32), true));
            p.add(poly(bolt(0.42), true));
        }
        Icon::Refresh => {
            // Two arcs round a circle, each with its head chasing the other's
            // tail (clockwise on screen).
            for start in [-2.6_f32, 0.55] {
                let end = start + 2.25;
                p.add(Shape::line(arc(start, end, 0.62), line));
                let tip_at = at(0.62 * end.cos(), 0.62 * end.sin());
                let dir = Vec2::new(-end.sin(), end.cos());
                let side = Vec2::new(-dir.y, dir.x);
                let tip = tip_at + dir * s * 0.17;
                let back = tip_at - dir * s * 0.12;
                p.add(Shape::convex_polygon(vec![tip, back + side * s * 0.2, back - side * s * 0.2], colour, Stroke::NONE));
            }
        }
        Icon::Warning => {
            p.add(poly(vec![at(0.0, -0.85), at(0.9, 0.75), at(-0.9, 0.75)], true));
            p.add(poly(vec![at(0.0, -0.3), at(0.0, 0.2)], false));
            p.circle_filled(at(0.0, 0.48), s * 0.06, colour);
        }
        Icon::Transform => {
            p.add(Shape::dashed_line(
                &[
                    at(-0.7, -0.7),
                    at(0.7, -0.7),
                    at(0.7, 0.7),
                    at(-0.7, 0.7),
                    at(-0.7, -0.7),
                ],
                thin,
                3.0,
                2.0,
            ));
            for (x, y) in [(-0.7, -0.7), (0.7, -0.7), (0.7, 0.7), (-0.7, 0.7)] {
                p.rect_filled(
                    Rect::from_center_size(at(x, y), Vec2::splat(s * 0.18)),
                    1.0,
                    colour,
                );
            }
            p.add(Shape::line(vec![at(0.0, -0.7), at(0.0, -1.0)], thin));
            p.circle_stroke(at(0.0, -1.0), s * 0.07, thin);
        }
        Icon::CornerPin => {
            let q = vec![
                at(-0.75, -0.55),
                at(0.6, -0.8),
                at(0.8, 0.7),
                at(-0.6, 0.55),
            ];
            p.add(poly(q.clone(), true));
            for pt in q {
                p.circle_filled(pt, s * 0.1, colour);
            }
        }
        Icon::Add => {
            p.add(poly(vec![at(0.0, -0.8), at(0.0, 0.8)], false));
            p.add(poly(vec![at(-0.8, 0.0), at(0.8, 0.0)], false));
        }
        Icon::Duplicate => {
            p.rect_stroke(
                Rect::from_min_max(at(-0.8, -0.8), at(0.3, 0.3)),
                2.0,
                line,
                egui::StrokeKind::Middle,
            );
            p.rect_stroke(
                Rect::from_min_max(at(-0.3, -0.3), at(0.8, 0.8)),
                2.0,
                line,
                egui::StrokeKind::Middle,
            );
        }
        Icon::Delete => {
            p.add(poly(vec![at(-0.8, -0.55), at(0.8, -0.55)], false));
            p.add(poly(
                vec![
                    at(-0.25, -0.55),
                    at(-0.2, -0.8),
                    at(0.2, -0.8),
                    at(0.25, -0.55),
                ],
                false,
            ));
            p.add(poly(
                vec![
                    at(-0.6, -0.55),
                    at(-0.45, 0.85),
                    at(0.45, 0.85),
                    at(0.6, -0.55),
                ],
                false,
            ));
            p.add(Shape::line(vec![at(-0.15, -0.25), at(-0.1, 0.55)], thin));
            p.add(Shape::line(vec![at(0.15, -0.25), at(0.1, 0.55)], thin));
        }
        Icon::RotateLeft | Icon::RotateRight => {
            // Clockwise round most of a circle, the head at the top right
            // pointing on round the arc; the left one is its mirror image.
            circular_arrow(icon == Icon::RotateLeft);
        }
        Icon::Flip => {
            p.add(Shape::dashed_line(
                &[at(0.0, -0.9), at(0.0, 0.9)],
                thin,
                2.5,
                2.0,
            ));
            p.add(poly(
                vec![at(-0.2, -0.6), at(-0.85, 0.6), at(-0.2, 0.6)],
                true,
            ));
            p.add(Shape::convex_polygon(
                vec![at(0.2, -0.6), at(0.2, 0.6), at(0.85, 0.6)],
                colour,
                Stroke::NONE,
            ));
        }
        Icon::Fit => {
            for (x, y) in [(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)] {
                let corner = at(0.8 * x, 0.8 * y);
                p.add(poly(
                    vec![
                        corner + vec2(0.0, -y * s * 0.25),
                        corner,
                        corner + vec2(-x * s * 0.25, 0.0),
                    ],
                    false,
                ));
            }
            p.rect_stroke(
                Rect::from_center_size(c, Vec2::splat(s * 0.55)),
                1.5,
                thin,
                egui::StrokeKind::Middle,
            );
        }
        Icon::Reset => {
            // Back round anticlockwise, to a dot: where it started.
            circular_arrow(true);
            p.circle_filled(c, s * 0.09, colour);
        }
        Icon::Snap => {
            p.add(poly(
                vec![
                    at(-0.6, -0.8),
                    at(-0.6, 0.1),
                    at(-0.45, 0.55),
                    at(0.0, 0.75),
                    at(0.45, 0.55),
                    at(0.6, 0.1),
                    at(0.6, -0.8),
                ],
                false,
            ));
            p.add(poly(vec![at(-0.9, -0.45), at(-0.3, -0.45)], false));
            p.add(poly(vec![at(0.3, -0.45), at(0.9, -0.45)], false));
        }
        Icon::Grid => {
            p.rect_stroke(
                Rect::from_min_max(at(-0.85, -0.85), at(0.85, 0.85)),
                1.5,
                line,
                egui::StrokeKind::Middle,
            );
            for k in [-0.28, 0.28] {
                p.add(Shape::line(vec![at(k, -0.85), at(k, 0.85)], thin));
                p.add(Shape::line(vec![at(-0.85, k), at(0.85, k)], thin));
            }
            p.circle_stroke(c, s * 0.18, thin);
        }
        Icon::Eye | Icon::EyeOff => {
            let top: Vec<Pos2> = (0..=16)
                .map(|k| {
                    let t = k as f32 / 16.0 * 2.0 - 1.0;
                    at(t * 0.9, -0.55 * (1.0 - t * t))
                })
                .collect();
            let bottom: Vec<Pos2> = (0..=16)
                .map(|k| {
                    let t = k as f32 / 16.0 * 2.0 - 1.0;
                    at(t * 0.9, 0.55 * (1.0 - t * t))
                })
                .collect();
            p.add(Shape::line(top, line));
            p.add(Shape::line(bottom, line));
            p.circle_filled(c, s * 0.16, colour);
            if icon == Icon::EyeOff {
                p.add(poly(vec![at(-0.8, -0.8), at(0.8, 0.8)], false));
            }
        }
        Icon::Lock | Icon::Unlock => {
            p.rect_filled(
                Rect::from_min_max(at(-0.6, -0.1), at(0.6, 0.85)),
                2.0,
                colour,
            );
            let shackle = if icon == Icon::Lock {
                arc(std::f32::consts::PI, 2.0 * std::f32::consts::PI, 0.38)
            } else {
                arc(
                    std::f32::consts::PI * 1.1,
                    std::f32::consts::PI * 1.75,
                    0.38,
                )
            };
            let shackle: Vec<Pos2> = shackle.iter().map(|q| *q + vec2(0.0, -0.05 * s)).collect();
            let (first, last) = (shackle[0], shackle[shackle.len() - 1]);
            p.add(Shape::line(shackle, line));
            p.add(Shape::line(vec![first, first + vec2(0.0, s * 0.12)], line));
            if icon == Icon::Lock {
                p.add(Shape::line(vec![last, last + vec2(0.0, s * 0.12)], line));
            }
        }
        Icon::Surfaces => {
            for (k, x) in [-0.55f32, 0.0, 0.55].iter().enumerate() {
                let tilt = (k as f32 - 1.0) * 0.12;
                p.add(poly(
                    vec![
                        at(x - 0.22, -0.6 - tilt),
                        at(x + 0.22, -0.6 + tilt),
                        at(x + 0.22, 0.6 - tilt),
                        at(x - 0.22, 0.6 + tilt),
                    ],
                    true,
                ));
            }
        }
        Icon::Move => {
            for (dx, dy) in [(0.0, -1.0), (1.0, 0.0), (0.0, 1.0), (-1.0, 0.0)] {
                let tip = at(dx * 0.85, dy * 0.85);
                p.add(poly(vec![c, tip], false));
                let side = vec2(-dy, dx) * s * 0.18;
                let back = vec2(-dx, -dy) * s * 0.2;
                p.add(poly(vec![tip + back + side, tip, tip + back - side], false));
            }
        }
        Icon::Look => {
            p.circle_stroke(c, s * 0.42, line);
            p.add(Shape::convex_polygon(
                arc(
                    -std::f32::consts::FRAC_PI_2,
                    std::f32::consts::FRAC_PI_2,
                    0.84,
                )
                .into_iter()
                .chain([c])
                .collect(),
                colour,
                Stroke::NONE,
            ));
        }
        Icon::Pulse => {
            p.add(poly(vec![at(-0.9, 0.0), at(-0.45, 0.0), at(-0.2, -0.6), at(0.15, 0.65), at(0.4, 0.0), at(0.9, 0.0)], false));
        }
        Icon::Display => {
            p.rect_stroke(Rect::from_min_max(at(-0.85, -0.7), at(0.85, 0.4)), 2.0, line, egui::StrokeKind::Middle);
            p.add(poly(vec![at(-0.35, 0.8), at(0.35, 0.8)], false));
            p.add(poly(vec![at(0.0, 0.4), at(0.0, 0.8)], false));
        }
        Icon::Spot => {
            p.add(poly(vec![at(-0.3, -0.85), at(0.3, -0.85), at(0.5, -0.2), at(-0.5, -0.2)], true));
            p.add(Shape::line(vec![at(-0.45, 0.05), at(-0.75, 0.75)], thin));
            p.add(Shape::line(vec![at(0.0, 0.05), at(0.0, 0.85)], thin));
            p.add(Shape::line(vec![at(0.45, 0.05), at(0.75, 0.75)], thin));
        }
        Icon::Wifi => {
            p.add(Shape::line(arc(-2.5, -0.64, 0.85).into_iter().map(|q| q + vec2(0.0, s * 0.35)).collect(), line));
            p.add(Shape::line(arc(-2.4, -0.74, 0.5).into_iter().map(|q| q + vec2(0.0, s * 0.35)).collect(), line));
            p.circle_filled(at(0.0, 0.62), s * 0.08, colour);
        }
        Icon::Pin => {
            p.circle_stroke(at(0.0, -0.3), s * 0.25, line);
            p.add(poly(vec![at(-0.55, -0.15), at(0.0, 0.9), at(0.55, -0.15)], false));
        }
        Icon::Gear => {
            p.circle_stroke(c, s * 0.2, line);
            for k in 0..8 {
                let a = k as f32 * std::f32::consts::TAU / 8.0;
                p.add(Shape::line(vec![at(0.55 * a.cos(), 0.55 * a.sin()), at(0.85 * a.cos(), 0.85 * a.sin())], line));
            }
        }
        Icon::Play => {
            p.add(Shape::convex_polygon(vec![at(-0.5, -0.75), at(0.75, 0.0), at(-0.5, 0.75)], colour, Stroke::NONE));
        }
        Icon::Stop => {
            p.rect_filled(Rect::from_min_max(at(-0.6, -0.6), at(0.6, 0.6)), 2.0, colour);
        }
        Icon::Expand => {
            p.add(poly(vec![at(-0.85, -0.3), at(-0.85, -0.85), at(-0.3, -0.85)], false));
            p.add(poly(vec![at(0.3, -0.85), at(0.85, -0.85), at(0.85, -0.3)], false));
            p.add(poly(vec![at(0.85, 0.3), at(0.85, 0.85), at(0.3, 0.85)], false));
            p.add(poly(vec![at(-0.3, 0.85), at(-0.85, 0.85), at(-0.85, 0.3)], false));
        }
        Icon::Bolt => {
            p.add(poly(vec![at(0.15, -0.9), at(-0.55, 0.15), at(0.0, 0.15), at(-0.15, 0.9), at(0.55, -0.15), at(0.0, -0.15)], true));
        }
        Icon::Snow => {
            for k in 0..3 {
                let a = k as f32 * std::f32::consts::PI / 3.0 + std::f32::consts::FRAC_PI_2;
                p.add(poly(vec![at(-0.85 * a.cos(), -0.85 * a.sin()), at(0.85 * a.cos(), 0.85 * a.sin())], false));
            }
        }
        Icon::Moon => {
            // A spot with a line through it: the lights off.
            p.add(poly(vec![at(-0.3, -0.85), at(0.3, -0.85), at(0.5, -0.2), at(-0.5, -0.2)], true));
            p.add(Shape::line(vec![at(0.0, 0.05), at(0.0, 0.75)], thin));
            p.add(poly(vec![at(-0.85, -0.85), at(0.85, 0.85)], false));
        }
        Icon::Next => {
            p.add(poly(vec![at(-0.8, 0.0), at(0.8, 0.0)], false));
            p.add(poly(vec![at(0.25, -0.55), at(0.8, 0.0), at(0.25, 0.55)], false));
        }
        Icon::Palette => {
            p.circle_stroke(c, s * 0.42, line);
            for (x, y) in [(-0.35, -0.2), (0.0, -0.45), (0.35, -0.2)] {
                p.circle_filled(at(x, y), s * 0.07, colour);
            }
        }
        Icon::Smoke => {
            for y in [-0.3f32, 0.3] {
                let pts: Vec<Pos2> = (0..=16).map(|k| { let t = k as f32 / 16.0 * 2.0 - 1.0; at(t * 0.85, y + 0.15 * (t * 6.0).sin()) }).collect();
                p.add(Shape::line(pts, line));
            }
        }
        Icon::Power => {
            p.add(Shape::line(arc(-std::f32::consts::FRAC_PI_2 + 0.7, -std::f32::consts::FRAC_PI_2 - 0.7 + std::f32::consts::TAU, 0.72), line));
            p.add(poly(vec![at(0.0, -0.9), at(0.0, -0.1)], false));
        }
        Icon::Hold => {
            p.rect_stroke(Rect::from_min_max(at(-0.6, -0.05), at(0.6, 0.85)), 2.0, line, egui::StrokeKind::Middle);
            p.add(Shape::line(arc(std::f32::consts::PI, 2.0 * std::f32::consts::PI, 0.38).into_iter().map(|q| q + vec2(0.0, -0.05 * s)).collect(), line));
        }
        Icon::Output => {
            p.rect_stroke(
                Rect::from_min_max(at(-0.85, -0.7), at(0.85, 0.45)),
                2.0,
                line,
                egui::StrokeKind::Middle,
            );
            p.add(poly(vec![at(-0.35, 0.85), at(0.35, 0.85)], false));
            p.add(poly(vec![at(0.0, 0.45), at(0.0, 0.85)], false));
        }
    }
}

/// A square icon button, highlighted when `on`; the tooltip names it.
pub fn button(ui: &mut Ui, icon: Icon, on: bool, tip: &str) -> Response {
    let size = Vec2::splat(38.0);
    let (rect, response) = ui.allocate_exact_size(size, Sense::click());
    let hovered = response.hovered();
    let fill = if on {
        style::PINK.gamma_multiply(0.22)
    } else if hovered {
        Color32::from_rgb(0x2a, 0x2a, 0x38)
    } else {
        Color32::TRANSPARENT
    };
    ui.painter().rect(
        rect,
        9.0,
        fill,
        Stroke::new(
            1.0,
            if on {
                style::PINK.gamma_multiply(0.8)
            } else {
                Color32::TRANSPARENT
            },
        ),
        egui::StrokeKind::Inside,
    );
    let colour = if on {
        style::PINK
    } else if hovered {
        style::TEXT
    } else {
        Color32::from_rgb(0xb8, 0xb8, 0xc8)
    };
    paint(ui, rect.shrink(10.0), icon, colour);
    response.on_hover_text(tip)
}

/// A small icon toggle for list rows (eye, lock).
pub fn small(ui: &mut Ui, icon: Icon, tip: &str, colour: Color32) -> Response {
    let (rect, response) = ui.allocate_exact_size(Vec2::splat(22.0), Sense::click());
    let c = if response.hovered() {
        style::TEXT
    } else {
        colour
    };
    paint(ui, rect.shrink(4.0), icon, c);
    response.on_hover_text(tip)
}
