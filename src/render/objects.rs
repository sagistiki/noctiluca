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
}

impl Placed {
    fn plain(at: Mat4) -> Self {
        Self {
            at,
            body: 1.0,
            glow: 1.0,
            sparkle: 0.0,
        }
    }
}

/// Every copy of object `object` to draw this frame. Each object has its
/// own composition: one huge head, a wall of skulls, a rain of prisms...
pub fn placements(object: usize, u: &Uniforms, appear: f32) -> Vec<Placed> {
    match OBJECTS.get(object).copied().unwrap_or("none") {
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
        _ => vec![Placed::plain(placement(object, u, appear))],
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
