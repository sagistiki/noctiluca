// Pattern: the spectrum bent into a circle, plus a shock wave that leaves the
// centre on every beat.

fn pattern(p: vec2f, base: vec3f) -> vec3f {
    let r = length(p);
    let x = abs(atan2(p.x, -p.y)) / (0.5 * TAU);   // 0 at the bottom, 1 at the top, mirrored
    let amp = spec(x * 0.8);
    let ring_r = 0.36 + 0.30 * amp + 0.04 * u.beat.y;
    let d = abs(r - ring_r);
    let ring = smoothstep(0.012, 0.0, d) + 0.004 / (d + 0.01);

    let wave_r = u.beat.z * 1.9;
    let shock = smoothstep(0.035, 0.0, abs(r - wave_r)) * (1.0 - u.beat.z) * step(1.0, u.beat.w);

    // Dim the background inside the circle so the ring reads clearly.
    let inside = smoothstep(ring_r, ring_r - 0.05, r);
    return base * (1.0 - 0.55 * inside)
        + pal(x * 0.7 + u.flow.w * 0.125) * ring * 1.6
        + pal(0.5 + u.flow.w * 0.125) * shock * 0.7;
}
