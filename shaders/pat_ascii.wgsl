// Pattern: the spectrum typed out in characters, like a terminal: each
// column a frequency, denser letters where it is louder, and a little rain
// of characters falling through.

fn ascii_glyph(f: vec2f, level: i32) -> f32 {
    let c = f - 0.5;
    let hline = step(abs(c.y), 0.06) * step(abs(c.x), 0.3);
    let vline = step(abs(c.x), 0.07) * step(abs(c.y), 0.34);
    switch level {
        case 1: { return step(length(c - vec2f(0.0, -0.25)), 0.09); }
        case 2: { return hline; }
        case 3: { return max(hline, vline); }
        case 4: { return step(abs(c.x - c.y * 0.6), 0.07) * step(abs(c.y), 0.38); }
        case 5: {
            let h2 = step(abs(abs(c.y) - 0.13), 0.05) * step(abs(c.x), 0.3);
            let v2 = step(abs(abs(c.x) - 0.12), 0.05) * step(abs(c.y), 0.34);
            return max(h2, v2);
        }
        case 6: { return step(abs(c.x), 0.3) * step(abs(c.y), 0.38); }
        default: { return 0.0; }
    }
}

fn pattern(p: vec2f, base: vec3f) -> vec3f {
    let rows = 36.0;
    let g = vec2f(p.x * rows / 0.6, p.y * rows) * 0.5;
    let id = floor(g);
    let f = fract(g);
    let aspect = u.res_time.x / u.res_time.y;
    // Bass in the middle, highs to both sides; louder rises higher.
    let x = clamp(abs((id.x + 0.5) / (rows / 0.6 * 0.5)) / aspect, 0.0, 1.0);
    let y = id.y / (rows * 0.5) * 0.5 + 0.5;
    let loud = spec(x * 0.7) * 0.9;
    var level = i32(clamp((loud - y) * 9.0 + 1.0, 0.0, 5.0));
    let speed = 2.0 + 6.0 * hash21(vec2f(id.x, 1.0));
    let rain = fract(hash21(vec2f(id.x, 2.0)) - u.res_time.z * speed / rows - y);
    if (rain < 0.06 && level == 0) {
        level = 1 + i32(hash21(id + floor(u.res_time.z * 8.0)) * 5.0);
    }
    let ink = ascii_glyph(f, level);
    let bright = select(0.7 + 0.6 * u.beat.y, 1.2, rain < 0.015);
    return base * 0.35 + pal(0.25 + y * 0.3) * ink * bright;
}
