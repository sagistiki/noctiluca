// Pattern: op-art moiré. Two sets of rings, their centres drifting apart
// and back, cross into bands that seem to move by themselves.

fn pattern(p: vec2f, base: vec3f) -> vec3f {
    let c = vec2f(0.35 * sin(u.flow.y * 0.21), 0.22 * cos(u.flow.y * 0.17));
    let f = 70.0 + 18.0 * u.bands.y;
    let a = sin(length(p - c) * f - u.flow.x * 2.0);
    let b = sin(length(p + c) * f + u.flow.x * 2.0);
    let m = smoothstep(0.0, 0.35, a * b);
    let ink = pal(0.6 + 0.1 * sin(u.flow.w * 0.25));
    return base * (0.3 + 0.35 * (1.0 - m)) + ink * m * (0.45 + 0.7 * u.beat.y);
}
