//! The geometry of projection mapping: a surface is a four-cornered shape on
//! the projector's picture, and the show is warped into it the way a
//! projector's keystone correction does it, so straight lines stay straight.
//!
//! Positions are fractions of the output picture: (0, 0) its top left,
//! (1, 1) its bottom right. A surface's own coordinates run 0..1 across and
//! down it, its corners being top left, top right, bottom right, bottom left.

use glam::{Mat3, Vec2, Vec3};

pub type Corners = [[f32; 2]; 4];

pub const FULL_FRAME: Corners = [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]];
/// A surface smaller than this share of the picture is a mistake, not a surface.
const MIN_AREA: f32 = 0.002;
/// How far outside the picture a surface may reach (projectors often
/// overshoot what they are pointed at).
const REACH: (f32, f32) = (-0.5, 1.5);
const FINE: f32 = 0.1;
const SNAP: f32 = 0.01;

fn v(c: [f32; 2]) -> Vec2 {
    Vec2::new(c[0], c[1])
}

/// The projective map from the unit square (the surface's own coordinates)
/// to the quad `corners` (Heckbert, "Fundamentals of Texture Mapping", 1989).
/// `None` for a quad that cannot be mapped (collapsed or crossed).
pub fn homography(corners: &Corners) -> Option<Mat3> {
    if !is_valid(corners) {
        return None;
    }
    let [p0, p1, p2, p3] = corners.map(v);
    let s = p0 - p1 + p2 - p3;
    let d1 = p1 - p2;
    let d2 = p3 - p2;
    let det = d1.x * d2.y - d2.x * d1.y;
    if det.abs() < 1e-9 {
        return None;
    }
    let g = (s.x * d2.y - d2.x * s.y) / det;
    let h = (d1.x * s.y - s.x * d1.y) / det;
    let a = p1.x - p0.x + g * p1.x;
    let b = p3.x - p0.x + h * p3.x;
    let d = p1.y - p0.y + g * p1.y;
    let e = p3.y - p0.y + h * p3.y;
    // Columns: what u, v and 1 contribute to (x, y, w).
    Some(Mat3::from_cols(
        Vec3::new(a, d, g),
        Vec3::new(b, e, h),
        Vec3::new(p0.x, p0.y, 1.0),
    ))
}

/// A point of the surface (0..1 each way) on the output picture.
pub fn to_output(h: &Mat3, uv: Vec2) -> Vec2 {
    let p = *h * uv.extend(1.0);
    p.truncate() / p.z
}

/// Where output point `p` falls on the surface, given the inverse of its
/// homography; `None` if it lies on the far side of the projection.
pub fn to_surface(inverse: &Mat3, p: Vec2) -> Option<Vec2> {
    let q = *inverse * p.extend(1.0);
    (q.z.abs() > 1e-9).then(|| q.truncate() / q.z)
}

/// Twice the signed area of a polygon (positive for clockwise on a
/// picture whose y runs down).
fn signed_area2(corners: &Corners) -> f32 {
    (0..4)
        .map(|i| {
            let (a, b) = (v(corners[i]), v(corners[(i + 1) % 4]));
            a.x * b.y - b.x * a.y
        })
        .sum()
}

/// A quad that can be projected into: convex, not crossed, corners in the
/// order top left, top right, bottom right, bottom left (turning one way),
/// big enough, and within reach.
pub fn is_valid(corners: &Corners) -> bool {
    if corners
        .iter()
        .flatten()
        .any(|c| !c.is_finite() || *c < REACH.0 || *c > REACH.1)
    {
        return false;
    }
    let turn = |i: usize| {
        let (a, b, c) = (
            v(corners[i]),
            v(corners[(i + 1) % 4]),
            v(corners[(i + 2) % 4]),
        );
        (b - a).perp_dot(c - b)
    };
    let turns = [turn(0), turn(1), turn(2), turn(3)];
    let all_one_way = turns.iter().all(|t| *t > 1e-7) || turns.iter().all(|t| *t < -1e-7);
    // Same winding as the full frame (so top stays top), and not too small.
    all_one_way
        && signed_area2(corners) * signed_area2(&FULL_FRAME) > 0.0
        && signed_area2(corners).abs() / 2.0 > MIN_AREA
}

fn snap(x: f32) -> f32 {
    (x / SNAP).round() * SNAP
}

/// Moves corner `i` by `delta` (a tenth of it when `fine`), landing on
/// hundredths when `snap_to_grid`. A move that would cross or collapse the
/// quad is refused: the corner stays where it was.
pub fn drag_corner(
    corners: &Corners,
    i: usize,
    delta: Vec2,
    fine: bool,
    snap_to_grid: bool,
) -> Corners {
    let mut out = *corners;
    let step = if fine { delta * FINE } else { delta };
    let mut p = v(corners[i]) + step;
    if snap_to_grid {
        p = Vec2::new(snap(p.x), snap(p.y));
    }
    out[i % 4] = [p.x, p.y];
    if is_valid(&out) { out } else { *corners }
}

/// Moves the whole surface by `delta`, as far as it stays within reach.
pub fn drag_surface(corners: &Corners, delta: Vec2) -> Corners {
    let mut d = delta;
    for c in corners {
        d.x = d.x.clamp(REACH.0 - c[0], REACH.1 - c[0]);
        d.y = d.y.clamp(REACH.0 - c[1], REACH.1 - c[1]);
    }
    corners.map(|c| [c[0] + d.x, c[1] + d.y])
}

/// Moves corner `i` by whole output pixels (the arrow keys).
pub fn nudge(
    corners: &Corners,
    i: usize,
    dx_px: i32,
    dy_px: i32,
    output_px: (u32, u32),
) -> Corners {
    let delta = Vec2::new(
        dx_px as f32 / output_px.0.max(1) as f32,
        dy_px as f32 / output_px.1.max(1) as f32,
    );
    drag_corner(corners, i, delta, false, false)
}

/// The corner nearest `p`, if it is within `radius`.
pub fn hit_corner(corners: &Corners, p: Vec2, radius: f32) -> Option<usize> {
    (0..4)
        .map(|i| (i, v(corners[i]).distance(p)))
        .filter(|(_, d)| *d <= radius)
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(i, _)| i)
}

/// Whether output point `p` is inside the surface.
pub fn contains(corners: &Corners, p: Vec2) -> bool {
    homography(corners)
        .and_then(|h| to_surface(&h.inverse(), p))
        .is_some_and(|uv| (0.0..=1.0).contains(&uv.x) && (0.0..=1.0).contains(&uv.y))
}

/// The middle of the surface (the average of its corners).
pub fn centroid(corners: &Corners) -> Vec2 {
    corners.iter().map(|c| v(*c)).sum::<Vec2>() / 4.0
}

/// From output fractions to a space where both axes are in the same units
/// (output pixels divided by the output's height): rotations and scaling
/// must happen there, or a surface turning on a wide projector would skew.
fn to_square(p: Vec2, aspect: f32) -> Vec2 {
    Vec2::new(p.x * aspect, p.y)
}

fn from_square(p: Vec2, aspect: f32) -> Vec2 {
    Vec2::new(p.x / aspect, p.y)
}

/// The surface turned by `radians` (clockwise on screen) about its middle,
/// on an output `aspect` wide per unit of height. `None` if that would take
/// it out of reach.
pub fn rotate(corners: &Corners, radians: f32, aspect: f32) -> Option<Corners> {
    let c = to_square(centroid(corners), aspect);
    let (sin, cos) = radians.sin_cos();
    let out = corners.map(|p| {
        let d = to_square(v(p), aspect) - c;
        let r = c + Vec2::new(d.x * cos - d.y * sin, d.x * sin + d.y * cos);
        let q = from_square(r, aspect);
        [q.x, q.y]
    });
    is_valid(&out).then_some(out)
}

/// The angle of the surface's top edge, in radians (0 = level), on an
/// output `aspect` wide.
pub fn angle(corners: &Corners, aspect: f32) -> f32 {
    let d = to_square(v(corners[1]), aspect) - to_square(v(corners[0]), aspect);
    d.y.atan2(d.x)
}

/// The surface grown or shrunk by `factor` about its middle. `None` if
/// that would make it too small or take it out of reach.
pub fn scale(corners: &Corners, factor: f32, aspect: f32) -> Option<Corners> {
    if !(factor.is_finite() && factor > 0.0) {
        return None;
    }
    let c = to_square(centroid(corners), aspect);
    let out = corners.map(|p| {
        let q = from_square(c + (to_square(v(p), aspect) - c) * factor, aspect);
        [q.x, q.y]
    });
    is_valid(&out).then_some(out)
}

/// How big the surface is: its average edge length in units of the output's
/// height (1.0 = as tall as the output).
pub fn size(corners: &Corners, aspect: f32) -> f32 {
    (0..4)
        .map(|i| {
            to_square(v(corners[i]), aspect).distance(to_square(v(corners[(i + 1) % 4]), aspect))
        })
        .sum::<f32>()
        / 4.0
}

/// `start` with corner `i` put at `to` (snapped to hundredths if asked).
/// `None` if the quad would cross, fold or leave reach: the caller keeps
/// the last good one, so the corner never jumps.
pub fn place_corner(start: &Corners, i: usize, to: Vec2, snap_to_grid: bool) -> Option<Corners> {
    let p = if snap_to_grid {
        Vec2::new(snap(to.x), snap(to.y))
    } else {
        to
    };
    let mut out = *start;
    out[i % 4] = [p.x, p.y];
    is_valid(&out).then_some(out)
}

/// `start` with edge `e` (0 top, 1 right, 2 bottom, 3 left) moved by `delta`.
pub fn move_edge(start: &Corners, e: usize, delta: Vec2) -> Option<Corners> {
    let mut out = *start;
    for i in [e % 4, (e + 1) % 4] {
        out[i] = [start[i][0] + delta.x, start[i][1] + delta.y];
    }
    is_valid(&out).then_some(out)
}

/// A surface's size in output pixels: the average of its opposite sides,
/// or, with a real shape `aspect` given, the same area in that shape (so a
/// slanted wall's picture is as wide as the wall really is).
pub fn extent(corners: &Corners, output_px: (u32, u32), aspect: Option<f32>) -> Vec2 {
    let px = Vec2::new(output_px.0 as f32, output_px.1 as f32);
    let side = |a: usize, b: usize| (v(corners[a]) * px).distance(v(corners[b]) * px);
    let w = (side(0, 1) + side(3, 2)) / 2.0;
    let h = (side(0, 3) + side(1, 2)) / 2.0;
    match aspect.filter(|a| a.is_finite() && *a > 0.0) {
        Some(a) => {
            let area = w * h;
            Vec2::new((area * a).sqrt(), (area / a).sqrt())
        }
        None => Vec2::new(w, h),
    }
}

/// The picture turned a quarter clockwise inside the same corners (the
/// corners stay where they are on the wall; which one is "top left" moves).
pub fn turn_picture(corners: &Corners) -> Corners {
    [corners[3], corners[0], corners[1], corners[2]]
}

/// The middle of edge `e`.
pub fn edge_middle(corners: &Corners, e: usize) -> Vec2 {
    (v(corners[e % 4]) + v(corners[(e + 1) % 4])) / 2.0
}

/// Where a new surface goes: side by side with the others, as in the sketch.
pub fn default_corners(index: usize, count: usize) -> Corners {
    let count = count.max(1) as f32;
    let w = 1.0 / count;
    let x0 = index as f32 * w + 0.02;
    let x1 = (index as f32 + 1.0) * w - 0.02;
    [[x0, 0.08], [x1, 0.08], [x1, 0.92], [x0, 0.92]]
}

#[cfg(test)]
mod tests {
    use super::*;

    const TRAPEZOID: Corners = [[0.1, 0.2], [0.8, 0.05], [0.9, 0.95], [0.05, 0.8]];

    #[test]
    fn corners_map_to_the_unit_square() {
        let h = homography(&TRAPEZOID).unwrap();
        let inv = h.inverse();
        for (c, uv) in TRAPEZOID
            .iter()
            .zip([[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]])
        {
            assert!(to_output(&h, v(uv)).distance(v(*c)) < 1e-5);
            assert!(to_surface(&inv, v(*c)).unwrap().distance(v(uv)) < 1e-5);
        }
    }

    #[test]
    fn centre_maps_to_the_centre_of_a_parallelogram() {
        let q = [[0.2, 0.1], [0.7, 0.2], [0.8, 0.8], [0.3, 0.7]];
        let h = homography(&q).unwrap();
        assert!(to_output(&h, Vec2::splat(0.5)).distance(Vec2::new(0.5, 0.45)) < 1e-5);
        // A full frame is the identity.
        let id = homography(&FULL_FRAME).unwrap();
        assert!(to_output(&id, Vec2::new(0.3, 0.7)).distance(Vec2::new(0.3, 0.7)) < 1e-6);
    }

    #[test]
    fn straight_lines_stay_straight() {
        let h = homography(&TRAPEZOID).unwrap();
        // Points along the surface's line u = 0.3 land on one output line.
        let pts: Vec<Vec2> = (0..=10)
            .map(|k| to_output(&h, Vec2::new(0.3, k as f32 / 10.0)))
            .collect();
        let dir = (pts[10] - pts[0]).normalize();
        for p in &pts {
            assert!((*p - pts[0]).perp_dot(dir).abs() < 1e-5, "bent at {p}");
        }
    }

    #[test]
    fn crossing_drags_are_refused() {
        // Dragging the top right corner past the bottom left crosses the quad.
        let moved = drag_corner(&FULL_FRAME, 1, Vec2::new(-1.2, 1.2), false, false);
        assert_eq!(moved, FULL_FRAME);
        // Pulling a corner inward a little is fine.
        let moved = drag_corner(&FULL_FRAME, 1, Vec2::new(-0.1, 0.05), false, false);
        assert_eq!(moved[1], [0.9, 0.05]);
        // A concave quad (corner pushed past the diagonal) is refused too.
        assert!(!is_valid(&[[0.0, 0.0], [0.4, 0.6], [1.0, 1.0], [0.0, 1.0]]));
        assert!(
            !is_valid(&[[1.0, 0.0], [0.0, 0.0], [0.0, 1.0], [1.0, 1.0]]),
            "mirrored winding"
        );
    }

    #[test]
    fn fine_drag_moves_a_tenth() {
        let moved = drag_corner(&FULL_FRAME, 2, Vec2::new(-0.1, -0.2), true, false);
        assert!((v(moved[2]) - Vec2::new(0.99, 0.98)).length() < 1e-6);
    }

    #[test]
    fn snap_lands_on_hundredths() {
        let moved = drag_corner(&FULL_FRAME, 0, Vec2::new(0.1234, 0.0567), false, true);
        assert_eq!(moved[0], [0.12, 0.06]);
    }

    #[test]
    fn nudge_moves_one_output_pixel() {
        let moved = nudge(&FULL_FRAME, 3, 1, -1, (1920, 1080));
        assert!(
            (moved[3][0] - 1.0 / 1920.0).abs() < 1e-7
                && (moved[3][1] - (1.0 - 1.0 / 1080.0)).abs() < 1e-6
        );
    }

    #[test]
    fn a_surface_cannot_be_dragged_out_of_reach() {
        let moved = drag_surface(&FULL_FRAME, Vec2::new(5.0, -5.0));
        assert!(
            moved
                .iter()
                .flatten()
                .all(|c| (REACH.0..=REACH.1).contains(c))
        );
        assert!(is_valid(&moved));
    }

    #[test]
    fn corners_are_found_and_insides_known() {
        assert_eq!(hit_corner(&TRAPEZOID, Vec2::new(0.81, 0.06), 0.03), Some(1));
        assert_eq!(hit_corner(&TRAPEZOID, Vec2::new(0.5, 0.5), 0.03), None);
        assert!(contains(&TRAPEZOID, Vec2::new(0.5, 0.5)));
        assert!(!contains(&TRAPEZOID, Vec2::new(0.02, 0.02)));
    }

    #[test]
    fn new_surfaces_sit_side_by_side_and_are_valid() {
        for n in 1..=3 {
            let quads: Vec<Corners> = (0..n).map(|i| default_corners(i, n)).collect();
            for q in &quads {
                assert!(is_valid(q));
            }
            for w in quads.windows(2) {
                assert!(w[0][1][0] < w[1][0][0], "they overlap");
            }
        }
    }

    const ASPECT: f32 = 16.0 / 9.0;

    fn px_lengths(c: &Corners) -> Vec<f32> {
        (0..4)
            .map(|i| to_square(v(c[i]), ASPECT).distance(to_square(v(c[(i + 1) % 4]), ASPECT)))
            .collect()
    }

    #[test]
    fn rotating_keeps_the_shape_in_real_pixels() {
        let q = default_corners(1, 3);
        let turned = rotate(&q, 0.3, ASPECT).unwrap();
        for (a, b) in px_lengths(&q).iter().zip(px_lengths(&turned)) {
            assert!((a - b).abs() < 1e-5, "a side changed length: {a} -> {b}");
        }
        assert!(centroid(&q).distance(centroid(&turned)) < 1e-6, "it moved");
        assert!((angle(&turned, ASPECT) - (angle(&q, ASPECT) + 0.3)).abs() < 1e-5);
        // A full turn comes back.
        let round = (0..4).fold(q, |c, _| {
            rotate(&c, std::f32::consts::FRAC_PI_2, ASPECT).unwrap_or(c)
        });
        assert!(
            round
                .iter()
                .zip(&q)
                .all(|(a, b)| v(*a).distance(v(*b)) < 1e-4)
        );
    }

    #[test]
    fn scaling_grows_about_the_middle() {
        let q = default_corners(1, 3);
        let big = scale(&q, 1.2, ASPECT).unwrap();
        assert!((size(&big, ASPECT) / size(&q, ASPECT) - 1.2).abs() < 1e-5);
        assert!(centroid(&q).distance(centroid(&big)) < 1e-6);
        assert!(scale(&q, 0.0001, ASPECT).is_none(), "too small");
        assert!(scale(&FULL_FRAME, 5.0, ASPECT).is_none(), "out of reach");
    }

    #[test]
    fn placing_a_corner_is_absolute_and_refuses_folds() {
        let q = FULL_FRAME;
        let moved = place_corner(&q, 1, Vec2::new(0.8, 0.1), false).unwrap();
        assert_eq!(moved[1], [0.8, 0.1]);
        assert!(place_corner(&q, 1, Vec2::new(-0.2, 1.2), false).is_none());
        let snapped = place_corner(&q, 0, Vec2::new(0.123, 0.456), true).unwrap()[0];
        assert!(
            v(snapped).distance(Vec2::new(0.12, 0.46)) < 1e-6,
            "{snapped:?}"
        );
    }

    #[test]
    fn the_middle_of_the_picture_lands_where_the_diagonals_cross() {
        // A strong keystone, as from a projector low and to the side: the
        // mapping is a true perspective, not a stretch.
        let c: Corners = [[0.1, 0.05], [0.95, 0.2], [0.85, 0.9], [0.2, 0.7]];
        let h = homography(&c).unwrap();
        let mid = to_output(&h, Vec2::splat(0.5));
        let (a, b, p, q) = (v(c[0]), v(c[2]), v(c[1]), v(c[3]));
        // Where the line a-b crosses the line p-q.
        let d1 = b - a;
        let d2 = q - p;
        let t = ((p - a).perp_dot(d2)) / d1.perp_dot(d2);
        assert!((a + d1 * t - mid).length() < 1e-5);
        // And the corners are the picture's corners, in order.
        for (i, uv) in [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]]
            .iter()
            .enumerate()
        {
            assert!((to_output(&h, Vec2::from(*uv)) - v(c[i])).length() < 1e-5);
        }
    }

    #[test]
    fn turning_and_mirroring_keep_the_corners_on_the_wall() {
        let c: Corners = [[0.1, 0.1], [0.6, 0.15], [0.55, 0.8], [0.12, 0.7]];
        let t = turn_picture(&c);
        let mut a = t.to_vec();
        let mut b = c.to_vec();
        a.sort_by(|x, y| x.partial_cmp(y).unwrap());
        b.sort_by(|x, y| x.partial_cmp(y).unwrap());
        assert_eq!(a, b);
        // Still a quad the editor can drag (same winding), every quarter.
        assert!(is_valid(&c) && is_valid(&t) && is_valid(&turn_picture(&t)));
        assert_eq!(
            turn_picture(&turn_picture(&turn_picture(&turn_picture(&c)))),
            c
        );
        // A quarter turn swaps width and height.
        let e = extent(&c, (1920, 1080), None);
        let f = extent(&t, (1920, 1080), None);
        assert!((e.x - f.y).abs() < 1e-3 && (e.y - f.x).abs() < 1e-3);
    }

    #[test]
    fn a_real_shape_keeps_the_area() {
        let e = extent(&FULL_FRAME, (1920, 1080), Some(1.0));
        assert!((e.x - e.y).abs() < 1e-3);
        assert!((e.x * e.y - 1920.0 * 1080.0).abs() < 1.0);
        assert_eq!(
            extent(&FULL_FRAME, (1920, 1080), None),
            Vec2::new(1920.0, 1080.0)
        );
    }

    #[test]
    fn an_edge_moves_both_its_corners() {
        let moved = move_edge(&FULL_FRAME, 1, Vec2::new(-0.2, 0.0)).unwrap();
        assert_eq!(moved[1], [0.8, 0.0]);
        assert_eq!(moved[2], [0.8, 1.0]);
        assert_eq!(moved[0], FULL_FRAME[0]);
        assert!(
            move_edge(&FULL_FRAME, 1, Vec2::new(-1.5, 0.0)).is_none(),
            "through the other side"
        );
        assert_eq!(edge_middle(&FULL_FRAME, 2), Vec2::new(0.5, 1.0));
    }
}
