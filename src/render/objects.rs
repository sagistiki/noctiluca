//! Where the 3D object stands, how it moves, and the camera that sees it.

use std::f32::consts::{PI, TAU};

use bytemuck::{Pod, Zeroable};
use glam::{Mat4, Quat, Vec3};

use super::uniforms::Uniforms;
use crate::director::OBJECTS;

/// The camera sits on the Z axis, looking at the origin.
pub const EYE: Vec3 = Vec3::new(0.0, 0.0, 6.0);
const FOV_Y_DEGREES: f32 = 32.0;
/// Models are 2 units tall; this makes them 1.64.
const SIZE: f32 = 0.82;
/// In the middle of the screen. When the wet floor is on, its horizon sits
/// just under the object's foot.
const CENTRE: Vec3 = Vec3::ZERO;

/// What the object shader needs for one object. Must match `struct Obj` in
/// shaders/object.wgsl.
#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub struct ObjectUniforms {
    pub model: [[f32; 4]; 4],
    pub view_proj: [[f32; 4]; 4],
    pub eye: [f32; 4],
    /// [material (0 chrome, 1 glass), body light, glow strength, sparkle]
    pub params: [f32; 4],
    /// [how far parts sway or flap, how fast (per second), paint pattern,
    /// unused]; see `Placed`.
    pub extra: [f32; 4],
}

/// The camera moves, a little: it drifts around in the groove, pushes in
/// (narrows its view) through a build-up, and shakes on the drop. Always
/// small enough that every composition still frames the same way.
#[derive(Clone, Copy, Debug)]
pub struct Camera {
    pub eye: Vec3,
    target: Vec3,
    fov_degrees: f32,
    /// Tilt of the picture, radians.
    roll: f32,
}

/// The furthest the camera drifts from its rest position, sideways and up.
const SWAY: Vec3 = Vec3::new(0.55, 0.25, 0.0);
/// How far it moves in, and how much it narrows its view, at the top of a build-up.
const PUSH_IN: f32 = 0.5;
const ZOOM_DEGREES: f32 = 7.0;
/// How far a drop throws it, and how much it tilts.
const SHAKE: f32 = 0.15;
const SHAKE_ROLL: f32 = 0.035;
/// The closest the camera ever comes (z): objects must stay in front of this.
pub const NEAREST_EYE_Z: f32 = EYE.z - PUSH_IN;

impl Camera {
    pub const REST: Camera = Camera {
        eye: EYE,
        target: Vec3::ZERO,
        fov_degrees: FOV_Y_DEGREES,
        roll: 0.0,
    };

    pub fn new(u: &Uniforms) -> Self {
        let [_, mid, level, _] = u.flow;
        let tension = u.state[2].clamp(0.0, 1.0);
        let shake = u.events[3].clamp(0.0, 1.0);
        let t = u.res_time[2];
        // A slow drift on two clocks that never line up, so it never repeats
        // a path; it stills as a build-up holds its breath.
        let drift = Vec3::new((mid * 0.06).sin(), (level * 0.045 + 1.0).sin(), 0.0)
            * SWAY
            * (1.0 - tension);
        // The shake: fast, uneven, dying away with `shake`.
        let jolt = Vec3::new(
            (t * 23.0).sin() + 0.5 * (t * 37.0).sin(),
            (t * 29.0 + 1.0).sin() + 0.5 * (t * 41.0).sin(),
            0.0,
        ) / 1.5
            * SHAKE
            * shake;
        Camera {
            eye: EYE + drift + jolt - Vec3::Z * PUSH_IN * tension,
            target: drift * 0.3 + jolt * 0.5,
            fov_degrees: FOV_Y_DEGREES - ZOOM_DEGREES * tension,
            roll: SHAKE_ROLL * shake * (t * 31.0).sin(),
        }
    }

    /// The camera's projection for a picture of the given width / height.
    pub fn view_proj(&self, aspect: f32) -> Mat4 {
        // wgpu, like Direct3D and Metal, keeps depth in 0..1 with Y pointing up:
        // glam's "directx" projection, not its OpenGL or Vulkan one.
        use glam::camera::rh::{proj::directx, view};
        let proj = directx::perspective(self.fov_degrees.to_radians(), aspect, 0.1, 50.0);
        let up = Vec3::new(self.roll.sin(), self.roll.cos(), 0.0);
        proj * view::look_at_mat4(self.eye, self.target, up)
    }
}

/// The camera at rest, for a picture of the given width / height.
pub fn view_proj(aspect: f32) -> Mat4 {
    Camera::REST.view_proj(aspect)
}

/// Where object number `object` (an index into OBJECTS) stands this frame.
/// `appear` runs 0..1 as it grows in during a cross-fade.
pub fn placement(object: usize, u: &Uniforms, appear: f32) -> Mat4 {
    // The same clocks the shaders use, so the object moves with the visuals:
    // `mid` runs faster with the mids, `level` with the overall level.
    let [_, mid, level, beats] = u.flow;
    let pulse = u.beat[1];
    let bass = u.bands[0].max(u.bands[1]);

    let turn = match OBJECTS.get(object).copied().unwrap_or("none") {
        // Faces look around the room but never turn their back on it.
        "skull" => {
            Quat::from_rotation_y(0.6 * (mid * 0.18).sin())
                * Quat::from_rotation_x(0.08 * (level * 0.3).sin())
        }
        "eye" => {
            Quat::from_rotation_y(0.35 * (mid * 0.25).sin())
                * Quat::from_rotation_x(0.2 * (level * 0.21).sin())
        }
        // The chain hangs and swings, once a bar, harder with the bass.
        "chain" => {
            Quat::from_rotation_z(0.12 * (beats * PI / 2.0).sin() * (0.4 + bass))
                * Quat::from_rotation_y(mid * 0.2)
        }
        // Everything else turns all the way round.
        _ => Quat::from_rotation_y(mid * 0.35) * Quat::from_rotation_x(0.15 * (level * 0.2).sin()),
    };
    // The chain swings from its top; everything else turns about its middle.
    let pivot = if OBJECTS.get(object) == Some(&"chain") {
        Vec3::Y
    } else {
        Vec3::ZERO
    };
    let bob = Vec3::new(0.0, 0.04 * (level * 0.8).sin(), 0.0);
    let scale = SIZE * appear.clamp(0.0, 1.0) * (1.0 + 0.06 * pulse);

    Mat4::from_translation(CENTRE + bob + pivot * scale)
        * Mat4::from_quat(turn)
        * Mat4::from_scale(Vec3::splat(scale))
        * Mat4::from_translation(-pivot)
}

/// One copy of an object, as drawn this frame.
#[derive(Clone, Copy, Debug)]
pub struct Placed {
    pub at: Mat4,
    /// How lit the chrome or glass is, 0..1 (1 for most; the skull wall
    /// dims the skulls that are not their turn).
    pub body: f32,
    /// How strongly the glowing parts glow (1 is normal).
    pub glow: f32,
    /// How much single tiles twinkle with the hi-hats (the mirror ball).
    pub sparkle: f32,
    /// Which part of the model to draw (its "pN_" in Blender); 0 for all.
    pub part: u8,
    /// How far its "_tail" and "_flex" parts sway, in model units, and how
    /// many times a second.
    pub sway: f32,
    pub sway_hz: f32,
    /// The pattern its "_paint" parts are painted in (see object.wgsl):
    /// 0 a gradient, 1 koi spots, 2 one glossy colour, 3 a smiling face,
    /// 4 an ornament (the hamsa).
    pub paint: f32,
}

impl Default for Placed {
    fn default() -> Self {
        Self::plain(Mat4::IDENTITY)
    }
}

impl Placed {
    fn plain(at: Mat4) -> Self {
        Self {
            at,
            body: 1.0,
            glow: 1.0,
            sparkle: 0.0,
            part: 0,
            sway: 0.0,
            sway_hz: 0.0,
            paint: 0.0,
        }
    }
}

/// Every copy of object `object` to draw this frame. Each object has its
/// own composition: one huge head, a wall of skulls, a rain of prisms...
pub fn placements(object: usize, u: &Uniforms, appear: f32) -> Vec<Placed> {
    let name = OBJECTS.get(object).copied().unwrap_or("none");
    let mut placed = compose(name, u, appear);
    // The compositions made before frames of every shape fill a 16:9 frame;
    // in a narrower one (a tall surface in Mapping) they shrink to fit its
    // width rather than spill off its sides. The newer ones fit themselves.
    if !FITS_ITSELF.contains(&name) {
        let k = (aspect(u) / (16.0 / 9.0)).min(1.0);
        if k < 1.0 {
            let shrink = Mat4::from_scale(Vec3::splat(k));
            for p in &mut placed {
                p.at = shrink * p.at;
            }
        }
    }
    placed
}

/// Compositions that size and spread themselves to the frame they are in.
const FITS_ITSELF: [&str; 19] = [
    "teddy", "koi", "manta", "wings", "helmet", "lips", "cards", "dice", "chess", "hearts", "gyro",
    "balloon_dog", "ducks", "smiley", "neon", "mobius", "nautilus", "cranes", "hamsa",
];

/// The frame's shape, width over height (16:9 until it is known).
fn aspect(u: &Uniforms) -> f32 {
    let (w, h) = (u.res_time[0], u.res_time[1]);
    if w > 0.0 && h > 0.0 { (w / h).clamp(0.2, 5.0) } else { 16.0 / 9.0 }
}

/// How large to draw a model `w` by `h` (in model units: its longest side
/// is 2) standing at depth `z`, so it spans `fill` of the frame across or
/// up, whichever it meets first: a wide thing fills a wide frame's width, a
/// tall one a tall frame's height, and neither ever spills out.
fn fit(u: &Uniforms, z: f32, w: f32, h: f32, fill: f32) -> f32 {
    let hh = half_height(z);
    let hw = hh * aspect(u);
    // A build-up pushes the camera in; allow for it, so what fills the
    // frame does not spill out at the top of the build.
    let push = 1.0 + 0.22 * u.state[2].clamp(0.0, 1.0);
    (fill * 2.0 * hw / w).min(fill * 2.0 * hh / h) / push
}

/// For compositions of several copies laid out for 16:9: how far to spread
/// them across (more in a wider frame), how much to shrink them (in a
/// narrower one), and how far to spread them up (more in a taller one).
fn spread(u: &Uniforms) -> (f32, f32, f32) {
    let k = aspect(u) / (16.0 / 9.0);
    (k, k.min(1.0), (1.0 / k).clamp(1.0, 1.8))
}

fn compose(name: &str, u: &Uniforms, appear: f32) -> Vec<Placed> {
    match name {
        "none" => Vec::new(),
        "chain" => chains(u, appear).into_iter().map(Placed::plain).collect(),
        "mirrorball" => mirror_ball(u, appear),
        "skull" => skull_wall(u, appear),
        "prism" => prism_rain(u, appear),
        "gem" => gem_close_up(u, appear),
        "knot" => knot_around(u, appear),
        "eye" => eye_wall(u, appear),
        "monolith" => avenue(u, appear),
        "speakers" => speaker_wall(u, appear),
        "crystal" => crystal(u, appear),
        "planet" => planet(u, appear),
        "corridor" => corridor(u, appear),
        "bust" => bust(u, appear),
        "diamond" => diamond_orbit(u, appear),
        "vinyl" => vinyl(u, appear),
        "jellyfish" => jellyfish(u, appear),
        "rose" => rose(u, appear),
        "blob" => blob(u, appear),
        "pyramid" => pyramid(u, appear),
        "moon" => moon(u, appear),
        "dna" => dna(u, appear),
        "butterfly" => butterflies(u, appear),
        "cubes" => cube_swarm(u, appear),
        "astronaut" => astronaut(u, appear),
        "teddy" => teddy(u, appear),
        "koi" => koi(u, appear),
        "manta" => manta(u, appear),
        "wings" => wings(u, appear),
        "helmet" => helmet(u, appear),
        "lips" => lips(u, appear),
        "cards" => cards(u, appear),
        "dice" => dice(u, appear),
        "chess" => chess(u, appear),
        "hearts" => balloon_hearts(u, appear),
        "gyro" => gyro(u, appear),
        "balloon_dog" => balloon_dog(u, appear),
        "ducks" => ducks(u, appear),
        "smiley" => smiley(u, appear),
        "neon" => neon(u, appear),
        "mobius" => mobius(u, appear),
        "nautilus" => nautilus(u, appear),
        "cranes" => cranes(u, appear),
        "hamsa" => hamsa(u, appear),
        _ => {
            let object = OBJECTS.iter().position(|o| *o == name).unwrap_or(0);
            vec![Placed::plain(placement(object, u, appear))]
        }
    }
}

/// A repeatable random number in 0..1 for copy `i` and purpose `k`, so each
/// copy gets its own but unchanging character.
fn rand(i: f32, k: f32) -> f32 {
    ((i * 12.9898 + k * 78.233).sin() * 43758.547).fract().abs()
}

/// Half the height of the view at depth `z`.
fn half_height(z: f32) -> f32 {
    (EYE.z - z) * (FOV_Y_DEGREES.to_radians() / 2.0).tan()
}

/// A giant mirror ball: taller than the screen, cropped at the top by the
/// frame, its full width in view, hanging from a rod far above. It turns
/// once every 32 beats, as mirror balls turn slowly.
fn mirror_ball(u: &Uniforms, appear: f32) -> Vec<Placed> {
    let beats = u.flow[3];
    // The model's ball has radius 0.4 and is centred 0.6 below the model's
    // middle (see the test); this makes it 3.8 across, close to the camera.
    let scale = 4.75 * appear.clamp(0.0, 1.0);
    let spin = Quat::from_rotation_y(beats * TAU / 32.0);
    let at = Mat4::from_translation(Vec3::new(0.0, 0.35 + 0.6 * scale, 0.0))
        * Mat4::from_quat(spin)
        * Mat4::from_scale(Vec3::splat(scale));
    vec![Placed {
        at,
        body: 1.0,
        glow: 1.0,
        sparkle: 1.0,
        ..Placed::default()
    }]
}

/// A wall of skulls, five by three, that light up one after another on the
/// beat like the steps of a sequencer. The rest stay dim.
fn skull_wall(u: &Uniforms, appear: f32) -> Vec<Placed> {
    let [_, mid, _, beats] = u.flow;
    let (cols, rows) = (5, 3);
    let count = cols * rows;
    let scale = 0.55 * appear.clamp(0.0, 1.0);
    (0..count)
        .map(|n| {
            let (c, r) = (n % cols, n / cols);
            let k = n as f32;
            // Snake through the wall: left to right, then right to left.
            let step = if r % 2 == 0 {
                r * cols + c
            } else {
                r * cols + cols - 1 - c
            };
            let since = (beats - step as f32).rem_euclid(count as f32);
            let lit = (-2.5 * since).exp();
            let pos = Vec3::new((c as f32 - 2.0) * 1.35, (1.0 - r as f32) * 1.25, -1.0);
            let turn = Quat::from_rotation_y(0.25 * (mid * 0.3 + k).sin())
                * Quat::from_rotation_x(0.1 * (mid * 0.21 + 2.0 * k).sin());
            let at = Mat4::from_translation(pos)
                * Mat4::from_quat(turn)
                * Mat4::from_scale(Vec3::splat(scale * (1.0 + 0.08 * lit)));
            Placed {
                at,
                body: 0.3 + 0.7 * lit,
                glow: 1.0,
                sparkle: 0.0,
                ..Placed::default()
            }
        })
        .collect()
}

/// Prisms falling through the room at different depths and speeds, each
/// tumbling on its own axis.
fn prism_rain(u: &Uniforms, appear: f32) -> Vec<Placed> {
    let [_, _, level, _] = u.flow;
    (0..22)
        .map(|n| {
            let k = n as f32;
            let z = -4.0 + 5.0 * rand(k, 1.0);
            let size = (0.3 + 0.25 * rand(k, 2.0)) * appear.clamp(0.0, 1.0);
            let h = half_height(z) + 0.6;
            let w = h * 16.0 / 9.0;
            let x = (rand(k, 3.0) * 2.0 - 1.0) * (w - 0.6);
            // Falls from above the top to below the bottom, then starts again.
            let speed = 0.25 + 0.35 * rand(k, 4.0);
            let fallen = (level * speed + rand(k, 5.0)).rem_euclid(1.0);
            let y = h - 2.0 * h * fallen;
            let axis = Vec3::new(rand(k, 6.0) - 0.5, rand(k, 7.0) - 0.5, rand(k, 8.0) - 0.5)
                .normalize_or(Vec3::X);
            let spin = Quat::from_axis_angle(axis, level * (0.6 + rand(k, 9.0)) + k);
            Placed::plain(
                Mat4::from_translation(Vec3::new(x, y, z))
                    * Mat4::from_quat(spin)
                    * Mat4::from_scale(Vec3::splat(size)),
            )
        })
        .collect()
}

/// The gem seen from very close: wider than the screen, its facets filling
/// the view, turning slowly with its crown toward the camera.
fn gem_close_up(u: &Uniforms, appear: f32) -> Vec<Placed> {
    let [_, mid, level, _] = u.flow;
    let scale = 2.6 * appear.clamp(0.0, 1.0) * (1.0 + 0.02 * u.beat[1]);
    let turn = Quat::from_rotation_x(1.0 + 0.15 * (level * 0.25).sin())
        * Quat::from_rotation_y(mid * 0.12);
    vec![Placed::plain(
        Mat4::from_translation(Vec3::new(0.0, 0.0, 0.5))
            * Mat4::from_quat(turn)
            * Mat4::from_scale(Vec3::splat(scale)),
    )]
}

/// The knot so big and close that its tube sweeps round the edges of the
/// view, turning on two axes.
fn knot_around(u: &Uniforms, appear: f32) -> Vec<Placed> {
    let [_, mid, level, _] = u.flow;
    let scale = 2.6 * appear.clamp(0.0, 1.0);
    let turn = Quat::from_rotation_z(mid * 0.1) * Quat::from_rotation_x(0.4 * (level * 0.13).sin());
    vec![Placed::plain(
        Mat4::from_translation(Vec3::new(0.0, 0.0, 0.0))
            * Mat4::from_quat(turn)
            * Mat4::from_scale(Vec3::splat(scale)),
    )]
}

/// A wall of eyes, six by three. Each looks somewhere of its own and jumps
/// to a new spot at moments of its own, the way eyes dart about; now and
/// then one looks straight at the room.
fn eye_wall(u: &Uniforms, appear: f32) -> Vec<Placed> {
    let seconds = u.res_time[2];
    let (cols, rows) = (6, 3);
    let scale = 0.45 * appear.clamp(0.0, 1.0);
    (0..cols * rows)
        .map(|n| {
            let (c, r) = (n % cols, n / cols);
            let k = n as f32;
            // Each eye holds its gaze for its own 0.8 to 3.3 seconds.
            let hold = 0.8 + 2.5 * rand(k, 1.0);
            let t = seconds / hold + rand(k, 2.0) * 7.0;
            let (look, since) = (t.floor(), t.fract() * hold);
            let target = |look: f32| {
                if rand(k + look, 3.0) < 0.15 {
                    Vec3::ZERO // at the room
                } else {
                    Vec3::new(
                        (rand(k + look, 4.0) - 0.5) * 1.1,
                        (rand(k + look, 5.0) - 0.5) * 0.7,
                        0.0,
                    )
                }
            };
            // A quick jump from the old spot to the new one, as eyes move.
            let jump = (since / 0.12).clamp(0.0, 1.0);
            let jump = jump * jump * (3.0 - 2.0 * jump);
            let g = target(look - 1.0).lerp(target(look), jump);
            let turn = Quat::from_rotation_y(g.x) * Quat::from_rotation_x(-g.y);
            let pos = Vec3::new((c as f32 - 2.5) * 1.25, (1.0 - r as f32) * 1.2, -1.5);
            Placed::plain(
                Mat4::from_translation(pos)
                    * Mat4::from_quat(turn)
                    * Mat4::from_scale(Vec3::splat(scale)),
            )
        })
        .collect()
}

/// Two rows of monoliths down a long avenue to a vanishing point. On every
/// beat a light runs through their cracks from the front to the back.
fn avenue(u: &Uniforms, appear: f32) -> Vec<Placed> {
    let beats = u.flow[3];
    let per_row = 7;
    let scale = 0.95 * appear.clamp(0.0, 1.0);
    let wave = beats.fract() * per_row as f32;
    (0..2 * per_row)
        .map(|n| {
            let (side, k) = (if n % 2 == 0 { -1.0 } else { 1.0 }, (n / 2) as f32);
            let z = 1.0 - k * 2.4;
            let pos = Vec3::new(side * 1.8, -1.15 + scale, z);
            let glow = 0.3 + 1.7 * (-1.5 * (wave - k) * (wave - k)).exp();
            let turn = Quat::from_rotation_y(-side * 0.35);
            Placed {
                at: Mat4::from_translation(pos)
                    * Mat4::from_quat(turn)
                    * Mat4::from_scale(Vec3::splat(scale)),
                body: 1.0,
                glow,
                sparkle: 0.0,
                ..Placed::default()
            }
        })
        .collect()
}

/// A festival sound system: six stacks of two cabinets across the whole
/// screen, the outer ones turned in toward the room. Every cone pumps on the
/// kick (in the shader); the light rings chase from left to right, one stack
/// per beat.
fn speaker_wall(u: &Uniforms, appear: f32) -> Vec<Placed> {
    let beats = u.flow[3];
    let (cols, rows) = (6, 2);
    let scale = 0.98 * appear.clamp(0.0, 1.0);
    let chase = beats.rem_euclid(cols as f32);
    (0..cols * rows)
        .map(|n| {
            let (c, r) = (n % cols, n / cols);
            let x = (c as f32 - 2.5) * 1.24;
            let pos = Vec3::new(x, (0.5 - r as f32) * 1.98, -1.0 - 0.12 * x.abs());
            let turn = Quat::from_rotation_y(-x * 0.09);
            let since = (chase - c as f32).rem_euclid(cols as f32);
            let glow = 0.5 + 1.6 * (-2.0 * since).exp();
            // The cabinets a little dark, so the cones and the light rings stand out.
            Placed {
                at: Mat4::from_translation(pos)
                    * Mat4::from_quat(turn)
                    * Mat4::from_scale(Vec3::splat(scale)),
                body: 0.6,
                glow,
                sparkle: 0.0,
                ..Placed::default()
            }
        })
        .collect()
}

/// The shards of the crystal, as many directions as evenly spread over a
/// sphere as can be (the golden-angle spiral).
const SHARDS: usize = 34;

/// A crystal grown from many long shards pointing outward from one centre,
/// turning slowly. A drop blows it apart (u.events[2], see src/burst.rs):
/// the shards fly out, tumbling, and later gather into the crystal again.
fn crystal(u: &Uniforms, appear: f32) -> Vec<Placed> {
    let [_, mid, level, _] = u.flow;
    let burst = u.events[2].clamp(0.0, 1.0);
    let grow = appear.clamp(0.0, 1.0);
    let whole =
        Quat::from_rotation_y(mid * 0.15) * Quat::from_rotation_x(0.3 * (level * 0.1).sin());
    (0..SHARDS)
        .map(|n| {
            let k = n as f32;
            // Golden-angle spiral: point n of SHARDS spread evenly on a sphere.
            let y = 1.0 - 2.0 * (k + 0.5) / SHARDS as f32;
            let ring = (1.0 - y * y).sqrt();
            let a = k * 2.399_963;
            let dir = whole * Vec3::new(ring * a.cos(), y, ring * a.sin());
            let length = 0.55 + 0.35 * rand(k, 1.0);
            // Flying apart: further for some than others, and less toward the
            // camera than across, so nothing comes too close.
            let fly = burst * (2.0 + 3.0 * rand(k, 2.0));
            let travel = dir * fly * Vec3::new(1.0, 1.0, 0.45);
            let tumble = Quat::from_axis_angle(
                Vec3::new(rand(k, 3.0) - 0.5, rand(k, 4.0) - 0.5, rand(k, 5.0) - 0.5)
                    .normalize_or(Vec3::X),
                burst * (2.0 + 4.0 * rand(k, 6.0)) + 0.4 * burst * level,
            );
            let point = tumble
                * Quat::from_rotation_arc(Vec3::Y, dir)
                * Quat::from_rotation_y(mid * 0.4 + k);
            let scale = length * grow * (1.0 + 0.05 * u.beat[1]);
            // Each shard's foot sits at the centre, so its point reaches out.
            let at = Mat4::from_translation(dir * scale + travel)
                * Mat4::from_quat(point)
                * Mat4::from_scale(Vec3::splat(scale));
            Placed {
                at,
                body: 1.0,
                glow: 1.0,
                sparkle: 0.0,
                ..Placed::default()
            }
        })
        .collect()
}

/// A planet rising out of the bottom of the screen, half of it in view,
/// its rings of light reaching past both sides. It turns slowly and rocks
/// a little, so the rings tilt.
fn planet(u: &Uniforms, appear: f32) -> Vec<Placed> {
    let [_, mid, level, _] = u.flow;
    let scale = 3.6 * appear.clamp(0.0, 1.0) * (1.0 + 0.015 * u.beat[1]);
    let turn =
        Quat::from_rotation_z(0.08 * (level * 0.12).sin()) * Quat::from_rotation_y(mid * 0.05);
    let at = Mat4::from_translation(Vec3::new(0.0, -1.1, -1.5))
        * Mat4::from_quat(turn)
        * Mat4::from_scale(Vec3::splat(scale));
    // The rings are big and close: at full glow they would drown the planet.
    vec![Placed {
        at,
        body: 1.0,
        glow: 0.22,
        sparkle: 0.0,
        ..Placed::default()
    }]
}

/// The corridor's sections: how many, how far apart, and where they leave
/// (z): close enough that the frame is already wider than the view.
const CORRIDOR_SECTIONS: usize = 14;
const CORRIDOR_GAP: f32 = 2.2;
const CORRIDOR_EXIT: f32 = 2.0;

/// Flying down an endless corridor: frames come out of the distance, one
/// per beat, and pass round the camera. The corridor twists as it goes, and
/// a pulse of light runs down it from the front on every beat.
fn corridor(u: &Uniforms, appear: f32) -> Vec<Placed> {
    let [_, mid, _, beats] = u.flow;
    let scale = 2.05 * appear.clamp(0.0, 1.0);
    let length = CORRIDOR_SECTIONS as f32 * CORRIDOR_GAP;
    (0..CORRIDOR_SECTIONS)
        .map(|n| {
            // How far this section is from the exit, travelling one gap per beat.
            let along = (n as f32 * CORRIDOR_GAP + beats * CORRIDOR_GAP).rem_euclid(length);
            let z = CORRIDOR_EXIT - along;
            let far = along / length;
            let twist = Quat::from_rotation_z(0.06 * z + 0.25 * (mid * 0.2).sin());
            let wave = beats.fract() * CORRIDOR_SECTIONS as f32;
            let place = along / CORRIDOR_GAP;
            let glow =
                (0.4 + 1.8 * (-1.2 * (wave - place) * (wave - place)).exp()) * (1.0 - 0.7 * far);
            // Sections fade in out of the dark far away instead of popping up.
            let fade = ((1.0 - far) * 6.0).clamp(0.0, 1.0);
            let at = Mat4::from_translation(Vec3::new(0.0, 0.0, z))
                * Mat4::from_quat(twist)
                * Mat4::from_scale(Vec3::splat(scale));
            Placed {
                at,
                body: (1.0 - 0.8 * far) * fade,
                glow: glow * fade,
                sparkle: 0.0,
                ..Placed::default()
            }
        })
        .collect()
}

/// One chain's place in the room: its slant (radians from horizontal), how
/// far it sits from the camera's line of sight across its length, its depth,
/// and which way it rolls.
struct ChainLayout {
    slant: f32,
    offset: f32,
    depth: f32,
    roll: f32,
}

/// Five chains laid out so that they cross: two lean one way, two the
/// other, one near level; some close to the camera, some far back.
const CHAIN_LAYOUT: [ChainLayout; 5] = [
    ChainLayout {
        slant: 0.35,
        offset: 0.5,
        depth: -2.6,
        roll: 1.0,
    },
    ChainLayout {
        slant: -0.45,
        offset: -0.4,
        depth: -0.4,
        roll: -1.0,
    },
    ChainLayout {
        slant: 0.1,
        offset: -0.6,
        depth: 1.0,
        roll: 1.0,
    },
    ChainLayout {
        slant: -0.2,
        offset: 1.1,
        depth: -3.6,
        roll: -1.0,
    },
    ChainLayout {
        slant: 0.55,
        offset: -0.2,
        depth: 0.3,
        roll: 1.0,
    },
];
/// The chain model is 2 units long; this makes each chain 16 units, longer
/// than the widest view of the room, so its ends are never seen.
const CHAIN_SCALE: f32 = 8.0;
/// Two links (the pattern repeats every two, as they alternate) in model
/// units: 18 links span the model's 2 units, at 0.78 of a link's spacing
/// per link out of 14.42 for the whole chain.
const CHAIN_PERIOD: f32 = 2.0 * 0.78 * 2.0 / 14.42;

fn chains(u: &Uniforms, appear: f32) -> Vec<Mat4> {
    let [_, mid, level, beats] = u.flow;
    let pulse = u.beat[1];
    let drift = level * 0.15; // slow: the room rearranges itself over minutes
    CHAIN_LAYOUT
        .iter()
        .enumerate()
        .map(|(i, c)| {
            let k = i as f32;
            let slant = c.slant + 0.12 * (drift * 0.3 + k).sin();
            let offset = c.offset + 0.35 * (drift * 0.5 + 3.0 * k).sin();
            let depth = c.depth + 0.5 * (drift * 0.4 + 2.0 * k).sin();
            // The links slide along by one link per beat, with a jolt on the
            // kick, and wrap every two links so the movement never ends.
            let slide =
                (beats * CHAIN_PERIOD / 2.0 + 0.03 * pulse).rem_euclid(CHAIN_PERIOD) * c.roll;
            let roll = mid * 0.25 * c.roll + k;
            let across = Vec3::new(-slant.sin(), slant.cos(), 0.0);
            Mat4::from_translation(across * offset + Vec3::new(0.0, 0.0, depth))
                // The model runs along Y; turn that to the slant across the screen.
                * Mat4::from_rotation_z(slant - PI / 2.0)
                * Mat4::from_rotation_y(roll)
                * Mat4::from_scale(Vec3::splat(CHAIN_SCALE * appear.clamp(0.0, 1.0)))
                * Mat4::from_translation(Vec3::new(0.0, slide, 0.0))
        })
        .collect()
}

// ------------------------------------------------ the 2026-11-22 objects

/// A placement from a position, a turn and a size (a number, or one per axis).
fn at(pos: Vec3, turn: Quat, scale: Vec3) -> Mat4 {
    Mat4::from_translation(pos) * Mat4::from_quat(turn) * Mat4::from_scale(scale)
}

/// A classical bust, big and a little low, looking slowly round the room
/// as a statue in a gallery seems to when you walk past it.
fn bust(u: &Uniforms, appear: f32) -> Vec<Placed> {
    let [_, mid, level, _] = u.flow;
    let scale = 1.25 * appear.clamp(0.0, 1.0);
    let turn = Quat::from_rotation_y(0.55 * (mid * 0.12).sin()) * Quat::from_rotation_x(0.06 * (level * 0.2).sin());
    vec![Placed::plain(at(Vec3::new(0.0, -0.35, 0.0), turn, Vec3::splat(scale)))]
}

/// A big diamond in the middle, turning, with a ring of small ones orbiting
/// it on a tilted path; all of them throw sparkles with the highs.
fn diamond_orbit(u: &Uniforms, appear: f32) -> Vec<Placed> {
    let [_, mid, _, beats] = u.flow;
    let a = appear.clamp(0.0, 1.0);
    let tilt = Quat::from_rotation_x(0.45);
    let mut out = vec![Placed {
        at: at(Vec3::ZERO, tilt * Quat::from_rotation_y(mid * 0.3), Vec3::splat(0.95 * a * (1.0 + 0.05 * u.beat[1]))),
        // A little less lit than the rest: its big table mirrors the light
        // above and would otherwise burn out to a white patch.
        body: 0.7,
        glow: 1.0,
        sparkle: 0.4,
        ..Placed::default()
    }];
    for i in 0..8 {
        let k = i as f32;
        let angle = k * TAU / 8.0 + beats * TAU / 64.0;
        let ring = Quat::from_rotation_x(0.3) * Vec3::new(angle.cos() * 2.4, 0.0, angle.sin() * 2.0 - 0.6);
        // Each turns slowly on its own, mostly showing its side, so its
        // facets catch the light one at a time instead of all at once.
        let turn = Quat::from_rotation_y(mid * 0.25 + k) * Quat::from_rotation_x(0.25 + 0.2 * k.sin());
        out.push(Placed {
            at: at(ring, turn, Vec3::splat(0.38 * a)),
            body: 0.8,
            glow: 1.0,
            sparkle: 0.3,
            ..Placed::default()
        });
    }
    out
}

/// A record spinning on a turntable, seen from above at an angle, a third
/// of a turn a beat; its label glows with the kick.
fn vinyl(u: &Uniforms, appear: f32) -> Vec<Placed> {
    let beats = u.flow[3];
    let scale = 1.35 * appear.clamp(0.0, 1.0);
    let turn = Quat::from_rotation_x(0.95) * Quat::from_rotation_z(0.25) * Quat::from_rotation_y(-beats * TAU / 3.0);
    vec![Placed {
        at: at(Vec3::new(0.0, -0.1, 0.0), turn, Vec3::splat(scale)),
        body: 1.0,
        glow: 0.6 + 0.9 * u.beat[1],
        sparkle: 0.0,
        ..Placed::default()
    }]
}

/// Jellyfish drifting up through dark water at different depths, each
/// pulsing its bell with the kick, their rims glowing.
fn jellyfish(u: &Uniforms, appear: f32) -> Vec<Placed> {
    let [_, mid, level, _] = u.flow;
    let a = appear.clamp(0.0, 1.0);
    let pulse = u.beat[1];
    (0..5)
        .map(|n| {
            let k = n as f32;
            let z = -0.3 - 1.8 * rand(k, 1.0);
            let span = half_height(z) * 2.0 + 2.5;
            let rise = (level * (0.06 + 0.04 * rand(k, 2.0)) + rand(k, 3.0) * span).rem_euclid(span);
            let pos = Vec3::new((k - 2.0) * 1.3 + 0.3 * (mid * 0.1 + k).sin(), rise - span / 2.0, z);
            let squeeze = 1.0 - 0.12 * pulse * (0.6 + 0.4 * rand(k, 5.0));
            let turn = Quat::from_rotation_y(mid * 0.1 + k) * Quat::from_rotation_z(0.15 * (level * 0.3 + k).sin());
            let size = (0.85 + 0.35 * rand(k, 6.0)) * a;
            Placed {
                at: at(pos, turn, Vec3::new(size / squeeze.sqrt(), size * squeeze, size / squeeze.sqrt())),
                body: 0.9,
                glow: 0.7 + 0.8 * pulse,
                sparkle: 0.0,
                ..Placed::default()
            }
        })
        .collect()
}

/// One rose, large, its bloom tipped toward the room, turning slowly.
fn rose(u: &Uniforms, appear: f32) -> Vec<Placed> {
    let [_, mid, level, _] = u.flow;
    let scale = 1.2 * appear.clamp(0.0, 1.0) * (1.0 + 0.04 * u.beat[1]);
    let turn = Quat::from_rotation_x(0.55 + 0.05 * (level * 0.3).sin()) * Quat::from_rotation_y(mid * 0.15);
    vec![Placed::plain(at(Vec3::new(0.0, 0.05, 0.0), turn, Vec3::splat(scale)))]
}

/// A drop of liquid metal, turning and wobbling: it squashes on the kick
/// and springs back.
fn blob(u: &Uniforms, appear: f32) -> Vec<Placed> {
    let [_, mid, level, _] = u.flow;
    let a = appear.clamp(0.0, 1.0);
    let squash = u.beat[1] * 0.14;
    let wobble = 0.05 * (level * 1.3).sin();
    let turn = Quat::from_rotation_y(mid * 0.4) * Quat::from_rotation_x(0.3 * (level * 0.25).sin());
    let scale = Vec3::new(1.0 + squash + wobble, 1.0 - squash, 1.0 + squash - wobble) * 1.05 * a;
    vec![Placed::plain(at(Vec3::ZERO, turn, scale))]
}

/// A big pyramid in the middle of the picture, turning slowly.
fn pyramid(u: &Uniforms, appear: f32) -> Vec<Placed> {
    let mid = u.flow[1];
    let scale = 1.25 * appear.clamp(0.0, 1.0) * (1.0 + 0.03 * u.beat[1]);
    vec![Placed::plain(at(Vec3::ZERO, Quat::from_rotation_x(0.15) * Quat::from_rotation_y(mid * 0.12), Vec3::splat(scale)))]
}

/// A huge moon rising at the top right, half out of the picture, turning
/// slowly enough that its craters seem still.
fn moon(u: &Uniforms, appear: f32) -> Vec<Placed> {
    let level = u.flow[2];
    let scale = 1.9 * appear.clamp(0.0, 1.0);
    let turn = Quat::from_rotation_y(level * 0.02) * Quat::from_rotation_x(0.3);
    vec![Placed::plain(at(Vec3::new(1.5, 0.75, -1.5), turn, Vec3::splat(scale)))]
}

/// A tall DNA helix through the middle of the picture, spinning, its base
/// pairs lighting up with the beat.
fn dna(u: &Uniforms, appear: f32) -> Vec<Placed> {
    let [_, mid, _, beats] = u.flow;
    let scale = 2.1 * appear.clamp(0.0, 1.0);
    let turn = Quat::from_rotation_z(0.35) * Quat::from_rotation_y(mid * 0.5);
    let since = beats.fract();
    vec![Placed {
        at: at(Vec3::ZERO, turn, Vec3::splat(scale)),
        body: 1.0,
        glow: 0.5 + 1.0 * (-4.0 * since).exp(),
        sparkle: 0.0,
        ..Placed::default()
    }]
}

/// Butterflies flying loops through the room, seen from above as they
/// pass: wings open to the camera, heads pointing where they fly, each
/// beating its wings (they close toward the body and open again) at its
/// own pace, their glowing edges brighter on the kick.
fn butterflies(u: &Uniforms, appear: f32) -> Vec<Placed> {
    let t = u.res_time[2];
    let level = u.flow[2];
    let a = appear.clamp(0.0, 1.0);
    (0..10)
        .map(|n| {
            let k = n as f32;
            let s = level * (0.1 + 0.06 * rand(k, 1.0)) + rand(k, 2.0) * TAU;
            let (fx, fy) = (0.7 + 0.3 * rand(k, 5.0), 1.1 + 0.4 * rand(k, 6.0));
            let pos = Vec3::new(3.0 * (s * fx).sin(), 1.3 * (s * fy + k).sin(), -1.2 + 1.4 * rand(k, 7.0));
            // Which way it is going on the screen.
            let (vx, vy) = (3.0 * fx * (s * fx).cos(), 1.3 * fy * (s * fy + k).cos());
            let roll = vy.atan2(vx) - PI / 2.0;
            let flap = (t * (6.0 + 3.0 * rand(k, 3.0)) + rand(k, 2.0) * TAU).sin().abs().max(0.18);
            let size = (0.32 + 0.12 * rand(k, 4.0)) * a;
            let turn = Quat::from_rotation_z(roll) * Quat::from_rotation_x(PI / 2.0 - 0.35);
            Placed {
                at: at(pos, turn, Vec3::new(size * flap, size, size)),
                body: 1.0,
                glow: 0.8 + 0.7 * u.beat[1],
                sparkle: 0.0,
                ..Placed::default()
            }
        })
        .collect()
}

/// Sixty small cubes flowing through the room like a flock of birds: each
/// on its own loop around a shared path, the flock opening up on the kick.
fn cube_swarm(u: &Uniforms, appear: f32) -> Vec<Placed> {
    let [_, mid, level, _] = u.flow;
    let a = appear.clamp(0.0, 1.0);
    let open = 1.0 + 0.35 * u.beat[1];
    (0..60)
        .map(|n| {
            let k = n as f32;
            let s = level * 0.12 + k * 0.045;
            let path = Vec3::new(2.6 * (s * 1.7).sin(), 1.0 * (s * 2.3).sin(), -1.2 + 1.2 * (s * 1.1).cos());
            let spread = Vec3::new(rand(k, 1.0) - 0.5, rand(k, 2.0) - 0.5, rand(k, 3.0) - 0.5) * 0.9 * open;
            let turn = Quat::from_rotation_x(mid * 0.4 + k) * Quat::from_rotation_y(mid * 0.3 + 2.0 * k);
            Placed::plain(at(path + spread, turn, Vec3::splat((0.07 + 0.05 * rand(k, 4.0)) * a)))
        })
        .collect()
}

/// An astronaut floating weightless in the middle of the room, tumbling
/// very slowly.
fn astronaut(u: &Uniforms, appear: f32) -> Vec<Placed> {
    let [_, mid, level, _] = u.flow;
    let scale = 1.0 * appear.clamp(0.0, 1.0);
    let turn = Quat::from_rotation_z(0.35 * (level * 0.05).sin() + 0.2) * Quat::from_rotation_y(mid * 0.08 + 0.6) * Quat::from_rotation_x(0.25 * (level * 0.07).sin());
    let drift = Vec3::new(0.35 * (level * 0.04).sin(), 0.15 * (level * 0.06).cos(), 0.0);
    vec![Placed::plain(at(drift, turn, Vec3::splat(scale)))]
}

/// A teddy bear afloat like the astronaut, in glossy paint the colour of
/// the palette, turning slowly and bobbing with the bass.
fn teddy(u: &Uniforms, appear: f32) -> Vec<Placed> {
    let [bass, mid, level, _] = u.flow;
    let scale = fit(u, 0.0, 1.46, 2.0, 0.7) * appear.clamp(0.0, 1.0) * (1.0 + 0.03 * u.beat[1]);
    let turn = Quat::from_rotation_y(0.5 * (mid * 0.06).sin() - 0.2) * Quat::from_rotation_z(0.1 * (level * 0.05).sin());
    let drift = Vec3::new(0.25 * (level * 0.04).sin(), 0.08 * (bass * 0.5).sin() + 0.05 * u.beat[1], 0.0);
    vec![Placed {
        paint: 0.0,
        ..Placed::plain(at(drift, turn, Vec3::splat(scale)))
    }]
}

/// The beats per second, for motions that keep time (a tail's stroke).
fn beat_hz(u: &Uniforms) -> f32 {
    (u.beat[3].clamp(60.0, 200.0)) / 60.0
}

/// Two koi circling each other, seen from above like yin and yang, their
/// markings painted in the palette and their tails beating in time.
fn koi(u: &Uniforms, appear: f32) -> Vec<Placed> {
    let level = u.flow[2];
    let a = appear.clamp(0.0, 1.0);
    let (sx, size, sy) = spread(u);
    (0..2)
        .map(|n| {
            let ang = level * 0.12 + n as f32 * PI;
            let pos = Vec3::new(0.9 * sx.min(1.6) * ang.cos(), 0.62 * sy * ang.sin(), -0.25);
            // Head first along the circle (the model's head is to -X).
            let heading = ang - PI / 2.0;
            let turn = Quat::from_rotation_z(heading) * Quat::from_rotation_x(PI / 2.0);
            Placed {
                paint: 1.0,
                sway: 0.2,
                sway_hz: beat_hz(u) * 0.5,
                ..Placed::plain(at(pos, turn, Vec3::splat(0.95 * size * a)))
            }
        })
        .collect()
}

/// A manta ray seen from below as it glides up through the frame, banking
/// slowly, its wings flapping.
fn manta(u: &Uniforms, appear: f32) -> Vec<Placed> {
    let [_, mid, level, _] = u.flow;
    let scale = fit(u, -0.3, 1.9, 1.7, 0.8) * appear.clamp(0.0, 1.0);
    let turn = Quat::from_rotation_z(0.25 * (level * 0.05).sin()) * Quat::from_rotation_y(0.3 * (mid * 0.04).sin()) * Quat::from_rotation_x(1.15);
    let pos = Vec3::new(0.6 * (level * 0.03).sin(), 0.15 + 0.1 * (level * 0.05).cos(), -0.3);
    vec![Placed {
        sway: 0.22,
        sway_hz: 0.3,
        ..Placed::plain(at(pos, turn, Vec3::splat(scale)))
    }]
}

/// A pair of angel wings, folded back in the groove, opening through a
/// build-up, spread wide on the drop; each beats a little on the kick.
fn wings(u: &Uniforms, appear: f32) -> Vec<Placed> {
    let mid = u.flow[1];
    let a = appear.clamp(0.0, 1.0);
    let tension = u.state[2].clamp(0.0, 1.0);
    let after_drop = (-u.stage[1] * 0.4).exp();
    let open = (0.3 + 0.55 * tension + 0.7 * after_drop).min(1.0);
    let fold = 1.15 * (1.0 - open) + 0.08 * u.beat[1];
    // Wide as they are, they spread across the frame, whatever its shape.
    let body = at(Vec3::new(0.0, 0.0, -0.3), Quat::from_rotation_y(0.15 * (mid * 0.08).sin()), Vec3::splat(fit(u, -0.3, 2.0, 0.59, 0.84) * a));
    // Each wing turns about a hinge at its root, folding away from the room.
    [(1u8, -1.0f32), (2, 1.0)]
        .into_iter()
        .map(|(part, side)| {
            let hinge = Vec3::new(0.08 * side, 0.35, 0.0);
            let wing = Mat4::from_translation(hinge) * Mat4::from_quat(Quat::from_rotation_y(fold * side)) * Mat4::from_translation(-hinge);
            Placed {
                part,
                glow: 0.8 + 0.6 * u.beat[1],
                ..Placed::plain(body * wing)
            }
        })
        .collect()
}

/// A chrome helmet turning slowly, its visor a screen showing the music's
/// spectrum.
fn helmet(u: &Uniforms, appear: f32) -> Vec<Placed> {
    let [_, mid, level, _] = u.flow;
    let scale = fit(u, 0.0, 2.0, 1.94, 0.72) * appear.clamp(0.0, 1.0);
    let turn = Quat::from_rotation_y(0.55 * (mid * 0.05).sin()) * Quat::from_rotation_x(-0.22 + 0.08 * (level * 0.07).sin());
    vec![Placed {
        glow: 1.0 + 0.4 * u.beat[1],
        ..Placed::plain(at(Vec3::new(0.0, 0.05, 0.0), turn, Vec3::splat(scale)))
    }]
}

/// Pop-art lips, glossy in the palette's colour, turned a little toward
/// the room: closed at rest, parting just a little on the hi-hats and the
/// kick. The model is wider than real lips, so they are drawn a little
/// narrower and fuller.
fn lips(u: &Uniforms, appear: f32) -> Vec<Placed> {
    let mid = u.flow[1];
    let a = appear.clamp(0.0, 1.0);
    let size = Vec3::new(1.2, 1.6, 1.4) * a * fit(u, 0.0, 2.4, 1.33, 0.75);
    let turn = Quat::from_rotation_y(0.3 * (mid * 0.06).sin()) * Quat::from_rotation_x(0.05);
    let part = 0.012 * u.bands[3].clamp(0.0, 1.0) + 0.014 * u.beat[1];
    [(1u8, 1.0f32), (2, -1.0)]
        .into_iter()
        .map(|(n, side)| Placed {
            part: n,
            paint: 2.0,
            // Raised a little, so a wet floor's reflection stays clear of the lower lip.
            ..Placed::plain(at(Vec3::new(0.0, (0.15 + part * side) * size.y, 0.0), turn, size))
        })
        .collect()
}

/// How far past the last drop we are, 1 right on it, fading over a few
/// seconds.
fn after_drop(u: &Uniforms, secs: f32) -> f32 {
    (-u.stage[1].max(0.0) / secs).exp()
}

/// Five playing cards held in a fan, turning a little; every bar one of
/// them spins round on its own, and on a drop the fan opens wide.
fn cards(u: &Uniforms, appear: f32) -> Vec<Placed> {
    let mid = u.flow[1];
    let beats = u.flow[3];
    let a = appear.clamp(0.0, 1.0);
    let spread = 1.0 + 0.7 * after_drop(u, 2.0);
    let base = at(Vec3::new(0.0, 0.2, 0.0), Quat::from_rotation_y(0.3 * (mid * 0.05).sin()), Vec3::splat(fit(u, 0.0, 2.9, 2.5, 0.85) * a));
    let turn = (beats / 4.0).floor();
    let flipping = turn.rem_euclid(5.0) as usize;
    let f = ((beats / 4.0).fract() * 4.0).min(1.0);
    let spin = TAU * f * f * (3.0 - 2.0 * f);
    // The fan turns about a point below the cards, as in a hand.
    let pivot = Vec3::new(0.0, -1.3, 0.0);
    (0..5)
        .map(|i| {
            let angle = (i as f32 - 2.0) * 0.24 * spread;
            let mut card = base
                * Mat4::from_translation(pivot)
                * Mat4::from_quat(Quat::from_rotation_z(-angle))
                * Mat4::from_translation(-pivot + Vec3::new(0.0, 0.0, i as f32 * 0.03));
            if i == flipping {
                card *= Mat4::from_quat(Quat::from_rotation_y(spin));
            }
            Placed {
                part: i as u8 + 1,
                glow: 0.8 + 0.6 * u.beat[1],
                ..Placed::plain(card)
            }
        })
        .collect()
}

/// Two dice tumbling slowly in the air; each drop throws them, and they
/// settle on new faces.
fn dice(u: &Uniforms, appear: f32) -> Vec<Placed> {
    let level = u.flow[2];
    let a = appear.clamp(0.0, 1.0);
    let thrown = after_drop(u, 1.2);
    let drops = u.stage[0];
    let (sx, size, sy) = spread(u);
    [(Vec3::new(-0.8 * sx.min(1.5), -0.1 * sy, 0.0), 0.0f32), (Vec3::new(0.85 * sx.min(1.5), 0.3 * sy, -0.5), 1.0)]
        .into_iter()
        .map(|(pos, n)| {
            // Which face is up: a new quarter turn or two at every drop.
            let k = drops + n * 3.0;
            let face = Quat::from_euler(glam::EulerRot::XYZ, (k * 1.0).floor() * PI / 2.0, (k * 1.7).floor() * PI / 2.0, 0.0);
            let tumble = Quat::from_rotation_y(level * 0.12 + n) * Quat::from_rotation_x(0.35 * (level * 0.09 + n).sin());
            let throw = Quat::from_rotation_x(TAU * thrown * 1.5) * Quat::from_rotation_z(PI * thrown);
            Placed {
                glow: 0.8 + 0.6 * u.beat[1],
                ..Placed::plain(at(pos, tumble * throw * face, Vec3::splat(0.55 * size * a)))
            }
        })
        .collect()
}

/// A chess king and queen, big, facing each other and turning slowly the
/// opposite ways; their collars glow with the kick.
fn chess(u: &Uniforms, appear: f32) -> Vec<Placed> {
    let mid = u.flow[1];
    let a = appear.clamp(0.0, 1.0);
    let (sx, size, _) = spread(u);
    [(1u8, -0.85 * sx.min(1.5)), (2, 0.85 * sx.min(1.5))]
        .into_iter()
        .map(|(part, x)| {
            let turn = Quat::from_rotation_y(mid * 0.1 * x.signum());
            Placed {
                part,
                glow: 0.25 + 0.35 * u.beat[1],
                ..Placed::plain(at(Vec3::new(x, -0.05, 0.0), turn, Vec3::splat(0.85 * size * a)))
            }
        })
        .collect()
}

/// Five balloon hearts in glossy paint rising slowly at different depths,
/// each beating on the kick.
fn balloon_hearts(u: &Uniforms, appear: f32) -> Vec<Placed> {
    let [_, mid, level, _] = u.flow;
    let a = appear.clamp(0.0, 1.0);
    let beat = 1.0 + 0.08 * u.beat[1];
    let (sx, fit_size, _) = spread(u);
    (0..5)
        .map(|n| {
            let k = n as f32;
            let z = -0.2 - 1.4 * rand(k, 1.0);
            let span = half_height(z) * 2.0 + 1.6;
            // Spread round the loop, so some are always in view.
            let rise = (level * (0.05 + 0.02 * rand(k, 2.0)) + k / 5.0 * span).rem_euclid(span);
            let pos = Vec3::new(((k - 2.0) * 0.95 + 0.2 * (mid * 0.1 + k).sin()) * sx, rise - span / 2.0, z);
            let turn = Quat::from_rotation_y(0.5 * (mid * 0.07 + k * 2.0).sin()) * Quat::from_rotation_z(0.15 * (level * 0.2 + k).sin());
            let size = (0.55 + 0.2 * rand(k, 4.0)) * a * beat * fit_size;
            Placed {
                paint: 2.0,
                ..Placed::plain(at(pos, turn, Vec3::splat(size)))
            }
        })
        .collect()
}

/// A gyroscope: three rings, each held in the one outside it and turning
/// on its own axis, faster with the bass, about a core that glows.
fn gyro(u: &Uniforms, appear: f32) -> Vec<Placed> {
    let bass = u.flow[0];
    let a = appear.clamp(0.0, 1.0);
    let base = at(Vec3::new(0.0, 0.05, 0.0), Quat::from_rotation_x(0.2), Vec3::splat(fit(u, 0.0, 2.0, 2.0, 0.66) * a));
    let outer = Quat::from_rotation_y(bass * 0.35);
    let middle = outer * Quat::from_rotation_x(bass * 0.5);
    let inner = middle * Quat::from_rotation_y(bass * 0.8);
    let core = Mat4::from_scale(Vec3::splat(1.0 + 0.25 * u.beat[1]));
    // The rings' lines of light stay fine; the core is what shines.
    let lines = 0.25 + 0.2 * u.beat[1];
    vec![
        Placed { part: 1, glow: lines, ..Placed::plain(base * Mat4::from_quat(outer)) },
        Placed { part: 2, glow: lines, ..Placed::plain(base * Mat4::from_quat(middle)) },
        Placed { part: 3, glow: lines, ..Placed::plain(base * Mat4::from_quat(inner)) },
        Placed { part: 4, glow: 0.45 + 0.35 * u.beat[1], ..Placed::plain(base * core) },
    ]
}

/// A balloon dog in glossy paint, turning slowly to show itself off; it
/// swells a little on the kick, as if blown up a breath more.
fn balloon_dog(u: &Uniforms, appear: f32) -> Vec<Placed> {
    let [_, mid, level, _] = u.flow;
    let scale = fit(u, 0.0, 2.0, 1.72, 0.66) * appear.clamp(0.0, 1.0) * (1.0 + 0.04 * u.beat[1]);
    let turn = Quat::from_rotation_y(mid * 0.12 + 0.4) * Quat::from_rotation_x(0.12);
    let drift = Vec3::new(0.0, -0.05 + 0.05 * (level * 0.06).sin(), 0.0);
    vec![Placed {
        paint: 2.0,
        ..Placed::plain(at(drift, turn, Vec3::splat(scale)))
    }]
}

/// Five rubber ducks bobbing round in a ring, as on a bath.
fn ducks(u: &Uniforms, appear: f32) -> Vec<Placed> {
    let level = u.flow[2];
    let t = u.res_time[2];
    let a = appear.clamp(0.0, 1.0);
    let (sx, size, _) = spread(u);
    (0..5)
        .map(|n| {
            let k = n as f32;
            let ang = level * 0.1 + k / 5.0 * TAU;
            let pos = Vec3::new(1.35 * sx.min(1.6) * ang.cos(), -0.45 + 0.05 * (t * 2.5 + k * 1.3).sin(), 1.0 * ang.sin() - 0.5);
            // Facing the way it is going round the ring (the model faces +X).
            let heading = (-ang.cos()).atan2(-ang.sin());
            let bob = Quat::from_rotation_z(0.12 * (t * 2.5 + k).sin());
            Placed {
                paint: 2.0,
                glow: 0.6 + 0.4 * u.beat[1],
                ..Placed::plain(at(pos, Quat::from_rotation_y(heading) * bob, Vec3::splat(0.5 * size * a)))
            }
        })
        .collect()
}

/// The rave smiley: a ball rolling slowly, which turns its face to the room
/// on every drop.
fn smiley(u: &Uniforms, appear: f32) -> Vec<Placed> {
    let [_, mid, level, _] = u.flow;
    let scale = fit(u, 0.0, 2.0, 2.0, 0.6) * appear.clamp(0.0, 1.0) * (1.0 + 0.04 * u.beat[1]);
    let rolling = Quat::from_rotation_y(mid * 0.2) * Quat::from_rotation_x(0.6 * (level * 0.07).sin());
    let facing = Quat::from_rotation_y(0.15 * (mid * 0.3).sin());
    let turn = rolling.slerp(facing, after_drop(u, 3.0));
    vec![Placed {
        paint: 3.0,
        ..Placed::plain(at(Vec3::new(0.0, 0.05, 0.0), turn, Vec3::splat(scale)))
    }]
}

/// Three neon signs (an arrow, a bolt, a star) hanging at different
/// depths, swinging a little; their tubes glow brighter on the kick.
fn neon(u: &Uniforms, appear: f32) -> Vec<Placed> {
    let level = u.flow[2];
    let a = appear.clamp(0.0, 1.0);
    let (sx, size, sy) = spread(u);
    // Side by side in a wide frame; in a tall one they stack instead.
    let tall = sy > 1.2;
    let spots = if tall {
        [Vec3::new(0.0, 1.3, -0.9), Vec3::new(0.0, 0.0, 0.0), Vec3::new(0.0, -1.25, -0.6)]
    } else {
        [Vec3::new(-1.35 * sx.min(1.6), 0.55, -0.9), Vec3::new(0.1, -0.05, 0.0), Vec3::new(1.35 * sx.min(1.6), 0.45, -0.6)]
    };
    let size = if tall { size * 1.6 } else { size };
    [(1u8, spots[0]), (2, spots[1]), (3, spots[2])]
        .into_iter()
        .map(|(part, pos)| {
            let k = part as f32;
            let swing = Quat::from_rotation_y(0.3 * (level * 0.05 + k).sin()) * Quat::from_rotation_z(0.05 * (level * 0.08 + k * 2.0).sin());
            Placed {
                part,
                glow: 0.65 + 0.55 * u.beat[1],
                ..Placed::plain(at(pos, swing, Vec3::splat(0.6 * size * a)))
            }
        })
        .collect()
}

/// A Möbius strip in chrome, tilted toward the room and turning slowly; a
/// light runs round its middle on the beat.
fn mobius(u: &Uniforms, appear: f32) -> Vec<Placed> {
    let [_, mid, level, _] = u.flow;
    let scale = fit(u, 0.0, 2.0, 1.45, 0.7) * appear.clamp(0.0, 1.0);
    let turn = Quat::from_rotation_x(0.95 + 0.1 * (level * 0.05).sin()) * Quat::from_rotation_y(mid * 0.08);
    vec![Placed {
        glow: 0.8 + 0.5 * u.beat[1],
        ..Placed::plain(at(Vec3::new(0.0, 0.05, 0.0), turn, Vec3::splat(scale)))
    }]
}

/// A nautilus shell turning slowly in the water, the seams between its
/// chambers glowing on the kick.
fn nautilus(u: &Uniforms, appear: f32) -> Vec<Placed> {
    let [_, mid, level, _] = u.flow;
    let scale = fit(u, 0.0, 2.0, 1.66, 0.68) * appear.clamp(0.0, 1.0);
    let turn = Quat::from_rotation_y(0.6 * (mid * 0.05).sin()) * Quat::from_rotation_z(0.15 * (level * 0.04).sin());
    let drift = Vec3::new(0.0, 0.05 * (level * 0.06).sin(), 0.0);
    vec![Placed {
        glow: 0.5 + 0.6 * u.beat[1],
        ..Placed::plain(at(drift, turn, Vec3::splat(scale)))
    }]
}

/// A small flock of origami cranes in palette paint flying through the
/// frame, seen from a little above, their wings beating.
fn cranes(u: &Uniforms, appear: f32) -> Vec<Placed> {
    let level = u.flow[2];
    let a = appear.clamp(0.0, 1.0);
    let (sx, size, sy) = spread(u);
    let (ax, ay) = (2.8 * sx, 1.2 * sy);
    (0..6)
        .map(|n| {
            let k = n as f32;
            let s = level * (0.08 + 0.04 * rand(k, 1.0)) + rand(k, 2.0) * TAU;
            let (fx, fy) = (0.6 + 0.3 * rand(k, 5.0), 1.0 + 0.4 * rand(k, 6.0));
            let pos = Vec3::new(ax * (s * fx).sin(), ay * (s * fy + k).sin(), -1.0 + 1.3 * rand(k, 7.0));
            let (vx, vy) = (ax * fx * (s * fx).cos(), ay * fy * (s * fy + k).cos());
            // From above, head first along its path (the model's head is to -Z).
            let turn = Quat::from_rotation_z(vy.atan2(vx) - PI / 2.0) * Quat::from_rotation_x(PI / 2.0 - 0.4);
            Placed {
                paint: 2.0,
                sway: 0.28,
                sway_hz: 1.1 + 0.4 * rand(k, 3.0),
                ..Placed::plain(at(pos, turn, Vec3::splat((0.62 + 0.16 * rand(k, 4.0)) * size * a)))
            }
        })
        .collect()
}

/// A hamsa floating and turning a little: Noctiluca's own design
/// (blender/design_hamsa.py), its lines raised in chrome over a body
/// lacquered in the palette's most vivid colour, the eye glowing on the kick.
fn hamsa(u: &Uniforms, appear: f32) -> Vec<Placed> {
    let [_, mid, level, _] = u.flow;
    let scale = fit(u, 0.0, 1.44, 2.0, 0.72) * appear.clamp(0.0, 1.0);
    let turn = Quat::from_rotation_y(0.45 * (mid * 0.05).sin()) * Quat::from_rotation_x(0.06 * (level * 0.07).sin());
    let drift = Vec3::new(0.0, 0.08 * (level * 0.05).sin(), 0.0);
    vec![Placed {
        glow: 0.45 + 0.45 * u.beat[1],
        paint: 2.0,
        ..Placed::plain(at(drift, turn, Vec3::splat(scale)))
    }]
}

#[cfg(test)]
mod tests {
    use super::*;

    const BALL: usize = 1;
    const SKULL: usize = 2;
    const PRISM: usize = 3;
    const GEM: usize = 4;
    const KNOT: usize = 5;
    const EYE_: usize = 6;
    const MONOLITH: usize = 7;
    const CHAIN: usize = 8;

    fn uniforms(flow: f32, pulse: f32, bass: f32) -> Uniforms {
        let mut u: Uniforms = Zeroable::zeroed();
        u.flow = [flow, flow * 1.3, flow * 0.7, flow * 2.1];
        u.beat = [0.5, pulse, 0.0, 128.0];
        u.bands = [bass, bass, 0.3, 0.3];
        u.res_time = [1280.0, 720.0, flow, 1.0 / 60.0];
        u
    }

    /// The corners of the 2-unit box every model fits in.
    fn corners() -> impl Iterator<Item = Vec3> {
        (0..8).map(|i| {
            Vec3::new(
                if i & 1 == 0 { -1.0 } else { 1.0 },
                if i & 2 == 0 { -1.0 } else { 1.0 },
                if i & 4 == 0 { -1.0 } else { 1.0 },
            )
        })
    }

    fn vp() -> Mat4 {
        view_proj(16.0 / 9.0)
    }

    fn screen(m: Mat4, p: Vec3) -> Vec3 {
        vp().project_point3(m.transform_point3(p))
    }

    fn on_screen(p: Vec3) -> bool {
        p.x.abs() <= 1.0 && p.y.abs() <= 1.0 && (0.0..1.0).contains(&p.z)
    }

    #[test]
    fn the_new_objects_fit_frames_of_every_shape() {
        // A wide screen, a 16:9 one, a square, and a tall surface in Mapping:
        // the self-fitting objects stay inside, near enough, every time.
        for (w, h) in [(2560.0, 720.0), (1280.0, 720.0), (800.0, 800.0), (720.0, 1280.0)] {
            let mut u = uniforms(5.0, 0.5, 0.5);
            u.res_time[0] = w;
            u.res_time[1] = h;
            let vp = view_proj(w / h);
            for name in FITS_ITSELF {
                // Flocks fly in and out of the frame on purpose.
                if ["cranes", "hearts"].contains(&name) {
                    continue;
                }
                let object = OBJECTS.iter().position(|o| *o == name).unwrap();
                for p in placements(object, &u, 1.0) {
                    let centre = vp.project_point3(p.at.transform_point3(Vec3::ZERO));
                    assert!(centre.x.abs() < 1.0 && centre.y.abs() < 1.0, "{name} in {w}x{h}: {centre}");
                    // Its widest reach (a model's half-size is about 1).
                    let edge = vp.project_point3(p.at.transform_point3(Vec3::X));
                    let reach = (edge - centre).truncate().length() + centre.truncate().length();
                    assert!(reach < 1.35, "{name} in {w}x{h} reaches {reach}");
                }
            }
        }
    }

    #[test]
    fn every_object_has_a_composition() {
        assert!(placements(0, &uniforms(1.0, 0.0, 0.0), 1.0).is_empty());
        for (object, name) in OBJECTS.iter().enumerate().skip(1) {
            assert!(
                !placements(object, &uniforms(1.0, 0.0, 0.0), 1.0).is_empty(),
                "{name}"
            );
        }
    }

    #[test]
    fn nothing_ever_reaches_the_camera() {
        for (object, name) in OBJECTS.iter().enumerate().skip(1) {
            // The model's real extent: a chain is long and very thin.
            let model = crate::render::model::load(
                &crate::render::model::default_dir().join(format!("{name}.glb")),
            )
            .unwrap();
            let (mut lo, mut hi) = (Vec3::splat(f32::MAX), Vec3::splat(f32::MIN));
            for v in &model.vertices {
                lo = lo.min(Vec3::from(v.pos));
                hi = hi.max(Vec3::from(v.pos));
            }
            let box_corners: Vec<Vec3> = corners()
                .map(|c| (c + 1.0) / 2.0 * (hi - lo) + lo)
                .collect();
            for step in 0..200 {
                for p in placements(
                    object,
                    &uniforms(step as f32 * 0.53, (step % 2) as f32, 0.6),
                    1.0,
                ) {
                    for c in &box_corners {
                        let z = p.at.transform_point3(*c).z;
                        assert!(z < NEAREST_EYE_Z - 0.15, "{name} reaches z {z}");
                    }
                }
            }
        }
    }

    #[test]
    fn appearing_grows_from_nothing() {
        for (object, name) in OBJECTS.iter().enumerate().skip(1) {
            for p in placements(object, &uniforms(2.0, 0.0, 0.0), 0.0) {
                assert!(p.at.transform_vector3(Vec3::ONE).length() < 1e-5, "{name}");
            }
        }
    }

    /// The ball's centre and radius in model units: the model is the ball
    /// (diameter 2) with a rod 3 long above it, scaled to 2 units tall.
    const BALL_CENTRE: Vec3 = Vec3::new(0.0, -0.6, 0.0);
    const BALL_RADIUS: f32 = 0.4;

    #[test]
    fn the_mirror_ball_is_giant_and_hangs_from_a_rod_out_of_sight() {
        for step in 0..100 {
            let ps = placements(BALL, &uniforms(step as f32 * 0.7, 0.0, 0.0), 1.0);
            assert_eq!(ps.len(), 1);
            let m = ps[0].at;
            let centre = screen(m, BALL_CENTRE);
            assert!(
                centre.x.abs() < 0.1 && (0.0..0.4).contains(&centre.y),
                "the ball is at {centre}"
            );
            // Taller than the screen...
            let top = screen(m, BALL_CENTRE + Vec3::Y * BALL_RADIUS).y;
            let bottom = screen(m, BALL_CENTRE - Vec3::Y * BALL_RADIUS).y;
            assert!(
                top - bottom > 2.0,
                "the ball is only {:.0}% of the height",
                (top - bottom) * 50.0
            );
            assert!(top > 1.0, "the top is not cropped");
            // ...but its full width is in view.
            for side in [Vec3::X, -Vec3::X] {
                let p = screen(m, BALL_CENTRE + side * BALL_RADIUS);
                assert!(p.x.abs() < 1.0, "the side of the ball is cut off at {p}");
            }
            assert!(
                screen(m, Vec3::new(0.0, 1.0, 0.0)).y > 1.0,
                "the top of the rod shows"
            );
        }
    }

    #[test]
    fn the_mirror_ball_turns_with_the_music_and_stays_upright() {
        let angle = |beats: f32| {
            let mut u = uniforms(1.0, 0.0, 0.0);
            u.flow[3] = beats;
            let m = placements(BALL, &u, 1.0)[0].at;
            assert!(
                m.transform_vector3(Vec3::Y).normalize().y > 0.99,
                "the rod tilts"
            );
            let x = m.transform_vector3(Vec3::X);
            x.z.atan2(x.x)
        };
        let (a, b) = (angle(0.0), angle(4.0));
        assert!(
            (b - a).abs() > 0.3,
            "turned only {} radians in 4 beats",
            (b - a).abs()
        );
        assert!(
            placements(BALL, &uniforms(1.0, 0.0, 0.0), 1.0)[0].sparkle > 0.0,
            "the ball does not sparkle"
        );
        assert!(
            placements(SKULL, &uniforms(1.0, 0.0, 0.0), 1.0)
                .iter()
                .all(|p| p.sparkle == 0.0)
        );
    }

    #[test]
    fn the_skull_wall_fills_the_screen_and_lights_in_sequence() {
        let ps = placements(SKULL, &uniforms(3.0, 0.0, 0.0), 1.0);
        assert!(ps.len() >= 12, "{} skulls", ps.len());
        for p in &ps {
            assert!(
                on_screen(screen(p.at, Vec3::ZERO)),
                "a skull's centre is off screen"
            );
            assert!(
                p.at.transform_vector3(Vec3::Z).normalize().z > 0.6,
                "a skull turned away"
            );
        }
        // One skull is the brightest, and the brightest moves on each beat.
        let brightest = |beats: f32| {
            let mut u = uniforms(3.0, 0.0, 0.0);
            u.flow[3] = beats;
            let ps = placements(SKULL, &u, 1.0);
            let lit = ps.iter().filter(|p| p.body > 0.8).count();
            assert!(lit <= 2, "{lit} skulls fully lit at once");
            (0..ps.len())
                .max_by(|a, b| ps[*a].body.total_cmp(&ps[*b].body))
                .unwrap()
        };
        let order: Vec<usize> = (0..6).map(|b| brightest(b as f32 + 0.05)).collect();
        let mut distinct = order.clone();
        distinct.dedup();
        assert_eq!(distinct.len(), 6, "the light did not move: {order:?}");
    }

    #[test]
    fn prisms_rain_down() {
        let before = placements(PRISM, &uniforms(5.0, 0.0, 0.0), 1.0);
        let after = placements(PRISM, &uniforms(5.2, 0.0, 0.0), 1.0);
        assert!(before.len() >= 20, "{} prisms", before.len());
        let falling = before
            .iter()
            .zip(&after)
            .filter(|(a, b)| {
                b.at.transform_point3(Vec3::ZERO).y < a.at.transform_point3(Vec3::ZERO).y
            })
            .count();
        assert!(
            falling * 10 >= before.len() * 8,
            "only {falling} of {} fall",
            before.len()
        );
        // Spread across the width, not bunched in the middle.
        let xs: Vec<f32> = before.iter().map(|p| screen(p.at, Vec3::ZERO).x).collect();
        assert!(
            xs.iter().any(|x| *x < -0.5) && xs.iter().any(|x| *x > 0.5),
            "{xs:?}"
        );
    }

    #[test]
    fn the_gem_is_seen_close_up() {
        let ps = placements(GEM, &uniforms(2.0, 0.0, 0.0), 1.0);
        assert_eq!(ps.len(), 1);
        let xs: Vec<f32> = corners().map(|c| screen(ps[0].at, c).x).collect();
        let width = xs.iter().copied().fold(f32::MIN, f32::max)
            - xs.iter().copied().fold(f32::MAX, f32::min);
        assert!(
            width > 1.6,
            "the gem covers only {:.0}% of the width",
            width * 50.0
        );
    }

    #[test]
    fn the_knot_surrounds_the_view() {
        let ps = placements(KNOT, &uniforms(2.0, 0.0, 0.0), 1.0);
        assert_eq!(ps.len(), 1);
        let pts: Vec<Vec3> = corners().map(|c| screen(ps[0].at, c)).collect();
        let span = |f: fn(&Vec3) -> f32| {
            pts.iter().map(f).fold(f32::MIN, f32::max) - pts.iter().map(f).fold(f32::MAX, f32::min)
        };
        assert!(
            span(|p| p.x) > 2.0 && span(|p| p.y) > 2.0,
            "the knot fits inside the screen"
        );
    }

    #[test]
    fn the_eyes_fill_a_wall_and_stay_whole() {
        let ps = placements(EYE_, &uniforms(1.0, 0.0, 0.0), 1.0);
        assert!(ps.len() >= 12, "{} eyes", ps.len());
        for p in &ps {
            for c in corners() {
                let q = screen(p.at, c * 0.5); // the eyeball's own sphere, radius 1 in the model
                assert!(
                    q.x.abs() < 1.0 && q.y.abs() < 1.0,
                    "an eye is cut off at {q}"
                );
            }
        }
    }

    #[test]
    fn each_eye_looks_its_own_way_at_its_own_times() {
        // Sample ten seconds; note when each eye's gaze jumps.
        let gaze = |t: f32| -> Vec<Vec3> {
            placements(EYE_, &uniforms(t, 0.0, 0.0), 1.0)
                .iter()
                .map(|p| p.at.transform_vector3(Vec3::Z).normalize())
                .collect()
        };
        let eyes = gaze(0.0).len();
        let mut first_jump = vec![None; eyes];
        let mut directions: Vec<Vec<Vec3>> = vec![Vec::new(); eyes];
        let mut prev = gaze(0.0);
        for k in 1..600 {
            let t = k as f32 / 60.0;
            let now = gaze(t);
            for e in 0..eyes {
                if (now[e] - prev[e]).length() > 0.02 && first_jump[e].is_none() {
                    first_jump[e] = Some(k);
                }
                directions[e].push(now[e]);
            }
            prev = now;
        }
        assert!(first_jump.iter().all(|j| j.is_some()), "an eye never moved");
        let mut times: Vec<usize> = first_jump.iter().map(|j| j.unwrap() / 6).collect();
        times.sort();
        times.dedup();
        assert!(times.len() >= eyes / 2, "the eyes move together: {times:?}");
        // At one moment the eyes look in many different directions.
        let now = gaze(4.3);
        let mut unalike = 0;
        for a in 0..eyes {
            for b in a + 1..eyes {
                unalike += ((now[a] - now[b]).length() > 0.15) as usize;
            }
        }
        assert!(
            unalike * 2 > eyes * (eyes - 1) / 2,
            "the eyes mostly look the same way"
        );
        // And they always look out toward the room, never backwards.
        assert!(directions.iter().flatten().all(|d| d.z > 0.5));
    }

    #[test]
    fn monoliths_line_an_avenue() {
        let ps = placements(MONOLITH, &uniforms(2.0, 0.0, 0.0), 1.0);
        assert!(ps.len() >= 10, "{} monoliths", ps.len());
        let centres: Vec<Vec3> = ps
            .iter()
            .map(|p| p.at.transform_point3(Vec3::ZERO))
            .collect();
        let left = centres.iter().filter(|c| c.x < -0.5).count();
        let right = centres.iter().filter(|c| c.x > 0.5).count();
        assert!(left >= 5 && right >= 5, "{left} left, {right} right");
        let depth = centres.iter().map(|c| c.z).fold(f32::MIN, f32::max)
            - centres.iter().map(|c| c.z).fold(f32::MAX, f32::min);
        assert!(depth > 10.0, "the avenue is only {depth} deep");
    }

    #[test]
    fn the_cracks_light_front_to_back_on_each_beat() {
        let nearest_lit = |phase: f32| {
            let mut u = uniforms(2.0, 0.0, 0.0);
            u.flow[3] = 10.0 + phase;
            let ps = placements(MONOLITH, &u, 1.0);
            let lit = ps.iter().max_by(|a, b| a.glow.total_cmp(&b.glow)).unwrap();
            assert!(lit.glow > 1.2, "nothing lights up");
            lit.at.transform_point3(Vec3::ZERO).z
        };
        assert!(
            nearest_lit(0.1) > nearest_lit(0.8) + 4.0,
            "the light does not travel back"
        );
    }

    #[test]
    fn chains_cross_the_screen_with_their_ends_out_of_sight() {
        for step in 0..300 {
            let u = uniforms(step as f32 * 0.41, (step % 3) as f32 * 0.5, 0.5);
            let chains = placements(CHAIN, &u, 1.0);
            assert!((4..=6).contains(&chains.len()), "{} chains", chains.len());
            for (i, p) in chains.iter().enumerate() {
                for end in [-1.0, 1.0] {
                    let q = screen(p.at, Vec3::new(0.0, end, 0.0));
                    assert!(
                        !on_screen(q),
                        "chain {i}: an end shows at {q} (step {step})"
                    );
                }
                let seen = (0..=40)
                    .any(|k| on_screen(screen(p.at, Vec3::new(0.0, -1.0 + k as f32 / 20.0, 0.0))));
                assert!(seen, "chain {i} is entirely off screen (step {step})");
            }
        }
    }

    #[test]
    fn chains_are_big_and_spread_through_space() {
        let chains = placements(CHAIN, &uniforms(3.0, 0.0, 0.5), 1.0);
        let depths: Vec<f32> = chains
            .iter()
            .map(|p| p.at.transform_point3(Vec3::ZERO).z)
            .collect();
        let spread = depths.iter().copied().fold(f32::MIN, f32::max)
            - depths.iter().copied().fold(f32::MAX, f32::min);
        assert!(spread > 2.5, "depths {depths:?}");
        let leans: Vec<f32> = chains
            .iter()
            .map(|p| {
                let d = p.at.transform_vector3(Vec3::Y);
                d.y / d.x
            })
            .collect();
        assert!(
            leans.iter().any(|l| *l > 0.1) && leans.iter().any(|l| *l < -0.1),
            "leans {leans:?}"
        );
        for p in &chains {
            let a = screen(p.at, Vec3::ZERO);
            let b = screen(p.at, Vec3::new(0.0, 0.11, 0.0));
            assert!(
                (b - a).truncate().length() / 2.0 > 0.05,
                "a chain is too thin"
            );
        }
    }

    #[test]
    fn chains_move() {
        let a = placements(CHAIN, &uniforms(1.0, 0.0, 0.5), 1.0);
        let b = placements(CHAIN, &uniforms(4.0, 0.0, 0.5), 1.0);
        for (i, (pa, pb)) in a.iter().zip(&b).enumerate() {
            let moved =
                (pa.at.transform_point3(Vec3::ZERO) - pb.at.transform_point3(Vec3::ZERO)).length();
            assert!(moved > 0.05, "chain {i} hardly moved: {moved}");
        }
    }

    /// Cameras at the edges of what the moving camera does: drifted to each
    /// side, pushed all the way in, and mid-shake.
    fn extreme_cameras() -> Vec<Camera> {
        let mut out = Vec::new();
        for (mid, level) in [
            (0.0, 0.0),
            (26.18, 0.0),
            (-26.18, -12.0),
            (26.18, 12.68),
            (0.0, 34.9),
        ] {
            for (tension, shake, t) in [
                (0.0, 0.0, 0.0),
                (1.0, 0.0, 0.0),
                (0.0, 1.0, 0.05),
                (0.0, 1.0, 0.11),
            ] {
                let mut u: Uniforms = Zeroable::zeroed();
                u.flow = [0.0, mid, level, 0.0];
                u.state[2] = tension;
                u.events[3] = shake;
                u.res_time[2] = t;
                out.push(Camera::new(&u));
            }
        }
        out
    }

    #[test]
    fn the_camera_at_rest_is_the_old_fixed_one() {
        let u: Uniforms = Zeroable::zeroed();
        let c = Camera::new(&u);
        // Drift starts at sin(1) on one clock: only the other is at zero.
        assert_eq!(c.eye.z, EYE.z);
        assert_eq!(Camera::REST.view_proj(1.5), view_proj(1.5));
    }

    #[test]
    fn the_camera_moves_only_a_little() {
        for c in extreme_cameras() {
            let off = c.eye - EYE;
            assert!(
                off.x.abs() <= SWAY.x + SHAKE + 1e-4 && off.y.abs() <= SWAY.y + SHAKE + 1e-4,
                "{off:?}"
            );
            assert!(c.eye.z >= NEAREST_EYE_Z - 1e-4 && c.eye.z <= EYE.z + 1e-4);
            assert!(c.view_proj(16.0 / 9.0).is_finite());
        }
    }

    #[test]
    fn every_composition_stays_in_view_wherever_the_camera_goes() {
        for c in extreme_cameras() {
            let vp = c.view_proj(16.0 / 9.0);
            for (object, name) in OBJECTS.iter().enumerate().skip(1) {
                let ps = placements(object, &uniforms(3.0, 0.0, 0.3), 1.0);
                // Some part of some copy: its middle or a corner of its box.
                let seen = ps.iter().any(|p| {
                    corners().chain([Vec3::ZERO]).any(|c| {
                        let q = vp.project_point3(p.at.transform_point3(c));
                        q.x.abs() <= 1.0 && q.y.abs() <= 1.0 && (0.0..1.0).contains(&q.z)
                    })
                });
                assert!(seen, "{name}: no copy in view from {c:?}");
            }
        }
    }
}
