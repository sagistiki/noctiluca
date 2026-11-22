// Pattern: chevrons. Rows of arrows flow up the screen, a row per beat;
// every drop turns them round, and they stay that way until the next.

fn pattern(p: vec2f, base: vec3f) -> vec3f {
    let flip = select(1.0, -1.0, (u.stage.x % 2.0) >= 1.0);
    let v = (p.y * flip + abs(p.x) * 0.75) * 2.2 - pace() * 0.5;
    let band = fract(v);
    let width = 0.32 + 0.1 * u.beat.y;
    let aa = fwidth(v) * 1.5;
    let line = smoothstep(0.0, aa, band) * (1.0 - smoothstep(width - aa, width, band));
    // A narrow column down the middle, so the arrows read as arrows.
    let column = 1.0 - smoothstep(0.75, 0.95, abs(p.x));
    let row = floor(v);
    let lit = pal(0.62 + 0.2 * fract(row * 0.37)) * line * column;
    return base * mix(0.5, 0.15, line * column) + lit * (0.9 + 0.5 * u.beat.y);
}
