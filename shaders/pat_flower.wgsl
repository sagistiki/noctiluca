// Pattern: the flower of life, the old figure of overlapping circles,
// turning slowly, its lines brighter with the highs.

fn pattern(p: vec2f, base: vec3f) -> vec3f {
    let q = rot(u.flow.y * 0.05) * p / (0.92 + 0.06 * u.beat.y);
    let r0 = 0.21;
    var d = abs(length(q) - r0);
    for (var i = 0; i < 6; i++) {
        let a = f32(i) * TAU / 6.0;
        let b = a + TAU / 12.0;
        d = min(d, abs(length(q - r0 * vec2f(cos(a), sin(a))) - r0));
        d = min(d, abs(length(q - 2.0 * r0 * vec2f(cos(a), sin(a))) - r0));
        d = min(d, abs(length(q - 1.732 * r0 * vec2f(cos(b), sin(b))) - r0));
    }
    d = min(d, abs(length(q) - 3.0 * r0));
    let w = 0.003 + 0.004 * u.bands.w;
    let line = smoothstep(w, 0.0, d) + 0.0012 / (d + 0.004);
    return base * 0.55 + pal(0.1 + length(q) * 0.5 + u.flow.w * 0.04) * line * (0.8 + 0.7 * u.beat.y);
}
