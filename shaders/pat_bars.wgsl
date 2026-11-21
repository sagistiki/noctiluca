// Pattern: spectrum bars built from LED segments, mirrored left-right and up-down.

fn pattern(p: vec2f, base: vec3f) -> vec3f {
    let aspect = u.res_time.x / u.res_time.y;
    let x = abs(p.x) / aspect;                 // 0 in the middle, 1 at the edge
    let count = 24.0;
    let i = floor(x * count);
    let fx = fract(x * count);
    let amp = spec((i + 0.5) / count * 0.85);
    let height = amp * 0.75 * (0.6 + 0.6 * u.beat.x);

    let column = step(0.14, fx) * step(fx, 0.86);
    let inside = step(abs(p.y), height) * column;
    let led = step(0.28, fract(abs(p.y) * 18.0));
    let colour = pal(i / count * 0.6 + abs(p.y) * 0.4 + u.flow.w * 0.0625);

    return base * mix(0.65, 0.2, inside) + colour * inside * led * (0.9 + 0.9 * u.beat.y);
}
