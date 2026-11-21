// Shared by every shader: the uniform block, the full-screen vertex shader
// and small helpers. The struct must match `Uniforms` in src/render/uniforms.rs.

struct U {
    res_time: vec4f,          // width, height, seconds, frame time
    bands: vec4f,             // sub, bass, mid, high (0..1)
    beat: vec4f,              // level, pulse, beat phase, bpm
    state: vec4f,             // mood, scene mix, build-up tension, strobe
    misc: vec4f,              // hud opacity, beat count, confidence, look locked
    flow: vec4f,              // clocks that run faster with bass / mids / level, and beats
    palette: array<vec4f, 4>, // cosine palette a, b, c, d
    post: array<vec4f, 3>,    // effect preset, see POST_PRESETS in src/director.rs
    spectrum: array<vec4f, 16>,
    wave: array<vec4f, 64>,
    overlay: vec4f,           // label opacity, label length, treatment strength, unused
    text: array<vec4f, 8>,    // the label: one font code per character (see present.wgsl)
    events: vec4f,            // lightning light (limited), bolt shape, crystal burst, camera shake
    vibe: vec4f,              // brightness, intensity, texture (-1..1), how sure (0..1)
}

@group(0) @binding(0) var<uniform> u: U;

const TAU: f32 = 6.2831853;

struct VsOut {
    @builtin(position) pos: vec4f,
    @location(0) uv: vec2f,   // 0..1, origin top-left
}

// One triangle that covers the whole screen; no vertex buffer needed.
@vertex
fn vs_main(@builtin(vertex_index) i: u32) -> VsOut {
    let p = vec2f(f32((i << 1u) & 2u), f32(i & 2u));
    var out: VsOut;
    out.pos = vec4f(p * 2.0 - 1.0, 0.0, 1.0);
    out.uv = vec2f(p.x, 1.0 - p.y);
    return out;
}

// Screen position with (0,0) in the centre, y up, height spanning -1..1.
fn centred(uv: vec2f) -> vec2f {
    let aspect = u.res_time.x / u.res_time.y;
    return vec2f((uv.x * 2.0 - 1.0) * aspect, 1.0 - uv.y * 2.0);
}

// The current colour palette. t wraps, so any number works.
fn pal(t: f32) -> vec3f {
    return u.palette[0].rgb
        + u.palette[1].rgb * cos(TAU * (u.palette[2].rgb * t + u.palette[3].rgb));
}

fn rot(a: f32) -> mat2x2f {
    let c = cos(a);
    let s = sin(a);
    return mat2x2f(c, s, -s, c);
}

fn hash21(p: vec2f) -> f32 {
    // Tile numbers grow as the scene travels. Wrapping them every 1024 keeps
    // the multiplications below precise, at the price of the randomness
    // repeating every 1024 tiles, which nobody will ever see.
    let w = p - 1024.0 * floor(p / 1024.0);
    var q = fract(w * vec2f(123.34, 456.21));
    q += dot(q, q + 45.32);
    return fract(q.x * q.y);
}

fn noise(p: vec2f) -> f32 {
    let i = floor(p);
    let f = fract(p);
    let s = f * f * (3.0 - 2.0 * f);
    return mix(
        mix(hash21(i), hash21(i + vec2f(1.0, 0.0)), s.x),
        mix(hash21(i + vec2f(0.0, 1.0)), hash21(i + vec2f(1.0, 1.0)), s.x),
        s.y,
    );
}

fn fbm(p: vec2f) -> f32 {
    var sum = 0.0;
    var amp = 0.5;
    var q = p;
    for (var i = 0; i < 5; i++) {
        sum += amp * noise(q);
        q = rot(0.5) * q * 2.03;
        amp *= 0.5;
    }
    return sum;
}

// fbm with three octaves: for warping, where fine detail is not seen.
fn fbm3(p: vec2f) -> f32 {
    var sum = 0.0;
    var amp = 0.5;
    var q = p;
    for (var i = 0; i < 3; i++) {
        sum += amp * noise(q);
        q = rot(0.5) * q * 2.03;
        amp *= 0.5;
    }
    // Scaled up to the same range as five octaves would reach.
    return sum / 0.875 * 0.96875;
}

// Spectrum at x in 0..1 (bass on the left), interpolated.
fn spec(x: f32) -> f32 {
    let pos = clamp(x, 0.0, 1.0) * 63.0;
    let i = u32(pos);
    let j = min(i + 1u, 63u);
    return mix(u.spectrum[i / 4u][i % 4u], u.spectrum[j / 4u][j % 4u], fract(pos));
}

// Waveform at x in 0..1, interpolated, -1..1.
fn wave(x: f32) -> f32 {
    let pos = clamp(x, 0.0, 1.0) * 255.0;
    let i = u32(pos);
    let j = min(i + 1u, 255u);
    return mix(u.wave[i / 4u][i % 4u], u.wave[j / 4u][j % 4u], fract(pos));
}

// The beat count, but running at double speed through the last bar (4
// beats) of every 16-beat phrase, the way a drummer doubles up into the
// next section. Patterns that step with the beat use this instead of
// u.flow.w. It jumps at the start of each phrase, where patterns change
// shape anyway.
fn pace() -> f32 {
    let beats = u.flow.w;
    let in_phrase = beats - 16.0 * floor(beats / 16.0);
    return beats + max(in_phrase - 12.0, 0.0);
}
