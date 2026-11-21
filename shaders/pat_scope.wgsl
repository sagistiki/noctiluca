// Pattern: oscilloscope. Three neon traces of the live waveform.

fn pattern(p: vec2f, base: vec3f) -> vec3f {
    let aspect = u.res_time.x / u.res_time.y;
    let x = p.x / aspect * 0.5 + 0.5;
    var col = base * 0.65;
    for (var i = 0; i < 3; i++) {
        let fi = f32(i);
        let y = wave(fract(x * (1.0 - 0.15 * fi) + 0.07 * fi))
            * (0.36 - 0.09 * fi) * (0.35 + 0.9 * u.beat.x);
        let d = abs(p.y - y);
        col += pal(x * 0.5 + fi * 0.22 + u.flow.w * 0.0625) * (0.005 / (d + 0.004)) * (1.0 - 0.3 * fi);
    }
    return col;
}
