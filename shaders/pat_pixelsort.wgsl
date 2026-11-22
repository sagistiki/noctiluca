// Pattern: pixel sorting. In thin columns, runs of the picture are
// replaced by their colours sorted from dark to light, each run as long as
// its part of the spectrum; new runs every bar.

fn pattern(p: vec2f, base: vec3f) -> vec3f {
    let width = 0.012;
    let column = floor(p.x / width);
    let h = hash21(vec2f(column, floor(pace() / 4.0)));
    let top = mix(-0.2, 1.0, h);
    let len = 0.15 + 1.0 * spec(fract(column * 0.013));
    let t = (top - p.y) / len;
    let run = step(0.0, t) * step(t, 1.0) * step(0.3, hash21(vec2f(column, 7.0)));
    let sorted = pal(0.2 + t * 0.5 + h * 0.2) * (0.25 + 0.9 * t);
    return mix(base, sorted, run * 0.85);
}
