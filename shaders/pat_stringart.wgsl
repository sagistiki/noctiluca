// Pattern: string art. Threads run between nails round a circle, nail n to
// nail n times k; as k drifts, the threads draw cardioids, nephroids and
// stars, one into the next.

fn sa_seg(p: vec2f, a: vec2f, b: vec2f) -> f32 {
    let pa = p - a;
    let ba = b - a;
    let h = clamp(dot(pa, ba) / max(dot(ba, ba), 1e-6), 0.0, 1.0);
    return length(pa - ba * h);
}

fn pattern(p: vec2f, base: vec3f) -> vec3f {
    let k = 2.0 + u.flow.y * 0.015;
    let radius = 0.82 + 0.04 * u.beat.y;
    var glow = 0.0;
    for (var i = 0; i < 72; i++) {
        let a1 = f32(i) / 72.0 * TAU;
        let a2 = a1 * k;
        let d = sa_seg(p, radius * vec2f(cos(a1), sin(a1)), radius * vec2f(cos(a2), sin(a2)));
        glow += smoothstep(0.0035, 0.0, d) * 0.6 + 0.00025 / (d + 0.002);
    }
    let rim = smoothstep(0.004, 0.0, abs(length(p) - radius));
    return base * 0.5 + pal(0.3 + length(p) * 0.4 + u.flow.w * 0.03) * (min(glow, 2.0) + rim) * (0.6 + 0.5 * u.bands.z);
}
