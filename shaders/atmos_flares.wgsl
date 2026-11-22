// Lens flares: two stage lights just out of the picture, seen through a
// camera lens: a long anamorphic streak through each, and the ghosts a
// lens throws across the middle.

fn atmosphere(uv: vec2f) -> vec4f {
    let p = centred(uv);
    let aspect = u.res_time.x / u.res_time.y;
    var col = vec3f(0.0);
    for (var k = 0; k < 2; k++) {
        let fk = f32(k);
        let light = vec2f((0.6 * sin(u.flow.y * 0.11 + fk * 2.5)) * aspect, 0.55 * cos(u.flow.y * 0.08 + fk * 1.3));
        let rel = p - light;
        let level = 0.4 + 0.6 * u.bands.x;
        col += (pal(0.6 + fk * 0.2) * 0.6 + vec3f(0.3)) * exp(-abs(rel.y) * 90.0) * exp(-abs(rel.x) * 1.2) * 0.6 * level;
        col += vec3f(1.0) * exp(-dot(rel, rel) * 400.0) * level;
        for (var g = 0; g < 4; g++) {
            let fg = f32(g);
            let at = light * (-0.4 - 0.45 * fg);
            let d = length(p - at);
            let r = 0.03 + 0.04 * fract(fg * 0.37 + 0.2);
            col += pal(0.2 + fg * 0.15) * smoothstep(r, r * 0.6, d) * 0.12 * level;
        }
    }
    return vec4f(col, 0.0);
}
