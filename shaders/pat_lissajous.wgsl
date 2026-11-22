// Pattern: a Lissajous figure, as a scope draws one in X-Y mode: two
// sines a step apart in frequency, their size from the spectrum; the ratio
// changes every phrase.

fn lis_seg(p: vec2f, a: vec2f, b: vec2f) -> f32 {
    let pa = p - a;
    let ba = b - a;
    let h = clamp(dot(pa, ba) / max(dot(ba, ba), 1e-6), 0.0, 1.0);
    return length(pa - ba * h);
}

fn pattern(p: vec2f, base: vec3f) -> vec3f {
    let fa = 2.0 + (floor(pace() / 16.0) % 3.0);
    let fb = fa + 1.0;
    let turn = u.flow.y * 0.3;
    var d = 1e9;
    var prev = vec2f(0.0);
    for (var i = 0; i <= 96; i++) {
        let s = f32(i) / 96.0;
        let t = s * TAU;
        let r = 0.5 + 0.22 * spec(s * 0.5) + 0.05 * u.beat.y;
        let q = vec2f(sin(fa * t + turn) * r * 1.35, sin(fb * t) * r);
        if (i > 0) {
            d = min(d, lis_seg(p, prev, q));
        }
        prev = q;
    }
    let line = smoothstep(0.007, 0.0, d) + 0.002 / (d + 0.005);
    return base * 0.55 + pal(0.2 + u.flow.w * 0.06) * line * 1.3;
}
