// Background: the night sea, as bands. Five soft layers of water lie across
// the screen, from the sand-light of the shore in the top-left corner,
// through the shallows and the glowing blue, to the deep and the night.
// Each layer's edge is a wave: it curves slowly and carries a thin line of
// light, as plankton lights up where the water moves. Points of plankton
// twinkle in the dark water. The colours run along the palette, light end
// to dark: under Noctiluca it is the brand sea.
//
// site/sea.js draws the same sea for the home page: keep the two alike
// (docs/superpowers/specs/2026-11-22-sea-bands-design.md).

// The palette from its light end (k = 0) to its dark end (k = 1), fading to
// night at the very end. Palettes are not all light at the same end.
fn sea_ramp(k: f32) -> vec3f {
    let k1 = clamp(k, 0.0, 1.0);
    let lum = vec3f(0.3, 0.59, 0.11);
    let light_at_half = dot(pal(0.5), lum) >= dot(pal(0.0), lum);
    let t = select(0.5 * k1, 0.5 * (1.0 - k1), light_at_half);
    return pal(t) * (1.0 - 0.85 * smoothstep(0.8, 1.0, k1));
}

// Plankton: points of light in cells, each twinkling at its own pace.
fn sea_plankton(p: vec2f, scale: f32, density: f32, t: f32) -> f32 {
    let g = p * scale;
    let id = floor(g);
    let f = fract(g) - 0.5;
    let h = hash21(id);
    if (h > density) {
        return 0.0;
    }
    let off = (vec2f(hash21(id + 3.1), hash21(id + 7.7)) - 0.5) * 0.6;
    let r = length(f - off);
    let tw = 0.35 + 0.65 * pow(0.5 + 0.5 * sin(t * (0.7 + 2.5 * hash21(id + 1.3)) + h * 40.0), 3.0);
    return (smoothstep(0.11, 0.0, r) + 0.35 * exp(-r * r * 60.0)) * tw;
}

fn background(p: vec2f) -> vec3f {
    // Half-height units, as the site's copy uses.
    let q = p * 0.5;
    // The bands flow with the bass; the plankton keeps its own time.
    let t = u.flow.x * 0.18;
    let secs = u.res_time.z;
    let kick = u.beat.y;
    let highs = u.bands.w;
    let dir = normalize(vec2f(1.0, -0.55));
    let d = dot(q, dir);
    let s = dot(q, vec2f(-dir.y, dir.x));
    var col = sea_ramp(0.05 + (d + 0.9) * 0.08);
    for (var i = 0; i < 5; i++) {
        let fi = f32(i);
        let base = -0.75 + fi * 0.26;
        let edge = base
            + 0.07 * sin(s * (1.6 + fi * 0.35) + t * (1.0 + fi * 0.3) + fi * 2.1)
            + 0.05 * (fbm3(vec2f(s * 0.9 + fi * 3.0, t * 0.5)) - 0.5);
        let x = d - edge;
        let inside = smoothstep(-0.01, 0.01, x);
        col = mix(col, sea_ramp(0.28 + fi * 0.17 + x * 0.35), inside * 0.92);
        let glow = exp(-abs(x) * 140.0) + 0.25 * exp(-abs(x) * 18.0);
        col += sea_ramp(0.32) * glow * (0.55 - fi * 0.07) * (0.8 + 0.5 * kick);
    }
    let drift = q + vec2f(secs * 0.008, secs * 0.012);
    let sparks = sea_plankton(drift, 34.0, 0.12, secs) * 0.7 + sea_plankton(drift + 4.0, 12.0, 0.05, secs) * 1.1;
    col += mix(sea_ramp(0.2), vec3f(1.0), 0.5) * sparks * smoothstep(-0.2, 0.4, d) * (0.7 + 0.8 * highs);
    // A soft vignette, and a little dither so the long gradients never band.
    col *= 1.0 - 0.15 * dot(q, q);
    col += (hash21(p * 431.0 + fract(secs) * 61.0) - 0.5) / 255.0;
    // The colours above are as the eye should see them (the site draws
    // them as they are); the scene works in linear light and is tone-mapped
    // and sRGB-encoded on the way out, so take them back to linear here.
    return pow(max(col, vec3f(0.0)), vec3f(2.2));
}
