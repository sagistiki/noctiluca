// Caustics: the moving net of light on the bottom of a pool, as if the
// whole room were under water: cell edges of a slowly warping pattern,
// two layers drifting against each other.

fn caustic_net(q: vec2f, t: f32) -> f32 {
    let i = floor(q);
    let f = fract(q);
    var d1 = 8.0;
    var d2 = 8.0;
    for (var y = -1; y <= 1; y++) {
        for (var x = -1; x <= 1; x++) {
            let g = vec2f(f32(x), f32(y));
            let o = 0.5 + 0.45 * sin(t + TAU * vec2f(hash21(i + g), hash21(i + g + 5.3)));
            let d = length(g + o - f);
            if (d < d1) {
                d2 = d1;
                d1 = d;
            } else if (d < d2) {
                d2 = d;
            }
        }
    }
    return pow(1.0 - smoothstep(0.0, 0.07, d2 - d1), 2.0);
}

fn atmosphere(uv: vec2f) -> vec4f {
    let p = centred(uv);
    let t = u.flow.y * 0.12;
    let warp = vec2f(fbm3(p * 1.5 + t * 0.3), fbm3(p * 1.5 - t * 0.3 + 4.0)) * 1.1;
    let a = caustic_net(p * 4.5 + warp, t);
    let b = caustic_net(p * 5.7 - warp + 11.0, -t * 0.8);
    // Faint on their own, bright where the two layers cross.
    let light = (a + b) * 0.12 + a * b * 1.4;
    return vec4f(pal(0.55) * light * (0.3 + 0.35 * u.bands.y), 0.05);
}
