// Pattern: a twelve-fold mandala of petals in four rings, each ring
// swelling with its own band and the whole turning slowly.

fn pattern(p: vec2f, base: vec3f) -> vec3f {
    let q0 = rot(u.flow.y * 0.04) * p;
    let r = length(q0);
    let seg = TAU / 12.0;
    let am = abs(fract(atan2(q0.y, q0.x) / seg + 0.5) - 0.5) * seg;
    let q = r * vec2f(cos(am), sin(am));
    var v = 0.0;
    for (var k = 0; k < 4; k++) {
        let fk = f32(k);
        let rr = 0.15 + fk * 0.17 + 0.02 * sin(u.flow.y * 0.5 + fk);
        let size = (0.05 + 0.02 * fk) * (1.0 + 0.6 * spec(0.1 + fk * 0.15));
        let petal = length((q - vec2f(rr, 0.0)) * vec2f(1.0, 2.2 + fk * 0.5)) - size;
        v = max(v, smoothstep(0.006, 0.0, abs(petal)) + 0.25 * smoothstep(0.0, -0.03, petal));
    }
    return base * 0.5 + pal(r * 0.8 + u.flow.w * 0.04) * v * (0.7 + 0.5 * u.beat.y);
}
