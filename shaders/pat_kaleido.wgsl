// Pattern: six-fold mirrored line work that marches outward with the beat.

fn pattern(p: vec2f, base: vec3f) -> vec3f {
    let r = length(p);
    let slice = TAU / 6.0;
    let a = atan2(p.y, p.x) + u.flow.y * 0.15;
    let folded = abs(fract(a / slice + 0.5) - 0.5) * slice;
    let q = r * vec2f(cos(folded), sin(folded));

    let bands = abs(fract(q.x * 3.0 - u.flow.w * 0.5) - 0.5);
    let hatch = abs(fract((q.x + q.y * 1.7) * 2.0 + u.flow.x * 0.3) - 0.5);
    let thick = smoothstep(0.03 + 0.06 * u.bands.z, 0.0, bands);
    let thin = smoothstep(0.04, 0.0, hatch) * u.bands.w;
    let mask = smoothstep(1.5, 0.15, r);

    return base * (1.0 - 0.5 * thick * mask)
        + pal(r * 0.6 - u.flow.w * 0.125) * (thick * 0.9 + thin * 0.8) * mask * (0.7 + 0.6 * u.beat.y);
}
