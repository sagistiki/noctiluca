// Pattern: a barcode. Bars of every width scroll sideways, each as tall as
// its part of the spectrum; the code changes every two beats.

fn pattern(p: vec2f, base: vec3f) -> vec3f {
    let x = p.x * 3.0 + u.flow.x * 0.3;
    let column = floor(x * 12.0);
    let h = hash21(vec2f(column, floor(pace() / 2.0)));
    let bar = step(fract(x * 12.0), 0.15 + 0.7 * h) * step(0.35, hash21(vec2f(column, 3.0)));
    let height = 0.35 + 0.55 * spec(fract(column * 0.037));
    let inside = smoothstep(height, height - 0.01, abs(p.y));
    let ink = pal(0.1 + h * 0.2);
    return base * (1.0 - 0.7 * bar * inside) + ink * bar * inside * (0.45 + 0.8 * u.beat.y);
}
