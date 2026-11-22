// Pattern: twelve strings, low to high; each one shakes with its own part
// of the spectrum, the way a harp string sings at its note.

fn pattern(p: vec2f, base: vec3f) -> vec3f {
    var col = base * 0.55;
    for (var i = 0; i < 12; i++) {
        let fi = f32(i);
        let y0 = (fi - 5.5) / 6.0 * 0.85;
        let a = spec((fi + 0.5) / 12.0 * 0.8);
        let shake = sin(p.x * (6.0 + fi * 1.3) + u.res_time.z * (14.0 + fi * 2.0)) * a * 0.05 * cos(p.x * 1.1);
        let d = abs(p.y - y0 - shake);
        let line = smoothstep(0.003 + 0.004 * a, 0.0, d) + 0.001 * a / (d + 0.003);
        col += pal(fi / 12.0 * 0.5 + u.flow.w * 0.03) * line * (0.35 + 1.5 * a);
    }
    return col;
}
