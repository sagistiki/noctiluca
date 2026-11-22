// Snow: soft flakes in three layers, the near ones bigger and faster,
// drifting as they fall.

fn atmosphere(uv: vec2f) -> vec4f {
    let p = centred(uv);
    var a = 0.0;
    for (var k = 0; k < 3; k++) {
        let fk = f32(k);
        let scale = 6.0 + fk * 5.0;
        let q = vec2f(p.x + 0.15 * sin(p.y * 2.0 + u.res_time.z * 0.5 + fk), p.y + u.res_time.z * (0.12 + fk * 0.05)) * scale;
        let id = floor(q);
        let h = hash21(id + fk * 7.0);
        let off = vec2f(h - 0.5, fract(h * 13.0) - 0.5) * 0.6;
        let d = length(fract(q) - 0.5 - off);
        let size = (0.07 + 0.06 * fract(h * 31.0)) / (1.0 + fk * 0.3);
        a += step(0.55, fract(h * 7.7)) * smoothstep(size, size * 0.3, d) * (0.9 - fk * 0.25);
    }
    return vec4f((vec3f(0.85, 0.9, 1.0) + pal(0.8) * 0.15) * a * 0.75, 0.0);
}
