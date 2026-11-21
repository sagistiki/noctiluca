// The projector's picture: each surface's view warped into its four-cornered
// shape (one draw per surface over the whole screen; pixels outside the
// shape are discarded). Also draws the calibration patterns.
//
// Stands alone: it does not use common.wgsl.

struct Surface {
    // Output position (0..1, y down) to surface position: the inverse of the
    // surface's homography, as three columns.
    inverse_0: vec4f,
    inverse_1: vec4f,
    inverse_2: vec4f,
    colour: vec4f,   // the surface's colour, and its number (1..8) in w
    params: vec4f,   // flip (0/1), edge softness in output pixels, brightness, chase (0..0.2)
    output: vec4f,   // output width and height in pixels, pattern (0 show, 1 grid, 2 white, 3 black), unused
    extent: vec4f,   // the surface's width and height in output pixels (roughly), outline width (0 = none), unused
    crop: vec4f,     // the part of the picture shown: offset xy, scale zw (the middle, in the surface's shape)
}

@group(0) @binding(0) var<uniform> s: Surface;
@group(1) @binding(0) var view_tex: texture_2d<f32>;
@group(1) @binding(1) var samp: sampler;

@vertex
fn vs_main(@builtin(vertex_index) i: u32) -> @builtin(position) vec4f {
    let p = vec2f(f32((i << 1u) & 2u), f32(i & 2u));
    return vec4f(p * 2.0 - 1.0, 0.0, 1.0);
}

// A digit from the 3x5 font (the same one as the overlay's).
fn digit(p: vec2f, n: u32) -> f32 {
    if (p.x < 0.0 || p.x >= 1.0 || p.y < 0.0 || p.y >= 1.0) {
        return 0.0;
    }
    var bits = array<u32, 10>(31599u, 11415u, 25255u, 25230u, 23497u, 31118u, 14831u, 29330u, 31727u, 31694u);   // 0..9
    let b = bits[min(n, 9u)];
    let cell = vec2u(u32(p.x * 3.0), u32(p.y * 5.0));
    return f32((b >> ((4u - cell.y) * 3u + (2u - cell.x))) & 1u);
}

// 1 on a line `width` pixels wide where `d` (in pixels) is near zero.
fn line(d: f32, width: f32) -> f32 {
    return 1.0 - smoothstep(width * 0.5, width * 0.5 + 1.0, abs(d));
}

// Grid, circle, cross, border and number, in the surface's colour, on black.
fn calibration(uv: vec2f) -> vec3f {
    let px = uv * s.extent.xy;                           // position in (rough) output pixels
    let size = s.extent.xy;
    let colour = s.colour.rgb;
    var lit = 0.0;
    var white = 0.0;
    // An 8 x 6 grid.
    let cell = size / vec2f(8.0, 6.0);
    let g = px - cell * round(px / cell);
    lit = max(lit, max(line(g.x, 1.5), line(g.y, 1.5)) * 0.55);
    // The border, thicker.
    let edge = min(min(px.x, size.x - px.x), min(px.y, size.y - px.y));
    lit = max(lit, line(edge, 6.0));
    // The cross through the middle, and a circle round in real pixels.
    let c = px - size * 0.5;
    white = max(white, max(line(c.x, 2.0), line(c.y, 2.0)));
    let radius = 0.3 * min(size.x, size.y);
    white = max(white, line(length(c) - radius, 3.0));
    white = max(white, line(length(c) - radius * 0.08, 3.0));
    // The number, big, in the top left cell.
    let h = min(size.y * 0.18, cell.y * 0.9);
    let at = (px - vec2f(cell.x * 0.25, cell.y * 0.12)) / vec2f(h * 0.6, h);
    lit = max(lit, digit(at, u32(s.colour.w)));
    return colour * lit + vec3f(white) * (1.0 - lit);
}

@fragment
fn fs_main(@builtin(position) pos: vec4f) -> @location(0) vec4f {
    let p = pos.xy / s.output.xy;
    let q = mat3x3f(s.inverse_0.xyz, s.inverse_1.xyz, s.inverse_2.xyz) * vec3f(p, 1.0);
    if (abs(q.z) < 1e-9) {
        discard;
    }
    let uv = q.xy / q.z;
    if (uv.x < 0.0 || uv.x > 1.0 || uv.y < 0.0 || uv.y > 1.0) {
        discard;
    }
    // Soft edges: fade over `params.y` output pixels from each side.
    let from_edge = min(min(uv.x, 1.0 - uv.x) * s.extent.x, min(uv.y, 1.0 - uv.y) * s.extent.y);
    let alpha = select(1.0, smoothstep(0.0, s.params.y, from_edge), s.params.y > 0.0);

    let pattern = u32(round(s.output.z));
    var col = vec3f(0.0);
    if (pattern == 0u) {
        var t = uv;
        if (s.params.x > 0.5) {
            t.x = 1.0 - t.x;
        }
        t = s.crop.xy + t * s.crop.zw;
        col = textureSampleLevel(view_tex, samp, t, 0.0).rgb * s.params.z * (1.0 + s.params.w);
        // The surface being mapped: its outline and a ring at each corner,
        // so it can be lined up on the wall while the show plays.
        if (s.extent.z > 0.0) {
            let px = uv * s.extent.xy;
            let size = s.extent.xy;
            let edge = min(min(px.x, size.x - px.x), min(px.y, size.y - px.y));
            var mark = line(edge - s.extent.z * 0.5, s.extent.z);
            for (var k = 0u; k < 4u; k++) {
                let corner = vec2f(f32(k & 1u), f32(k >> 1u)) * size;
                mark = max(mark, line(length(px - corner) - 16.0, 4.0));
            }
            col = mix(col, s.colour.rgb, mark);
        }
    } else if (pattern == 1u) {
        col = calibration(uv);
    } else if (pattern == 2u) {
        col = vec3f(1.0);
    }
    return vec4f(col, alpha);
}
