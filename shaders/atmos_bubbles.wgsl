// Bubbles: rings of light rising and wobbling, each with a bright spot
// where it catches the light.

fn atmosphere(uv: vec2f) -> vec4f {
    let p = centred(uv);
    var col = vec3f(0.0);
    for (var k = 0; k < 2; k++) {
        let fk = f32(k);
        let scale = 3.5 + fk * 3.0;
        let q = vec2f(p.x + 0.08 * sin(p.y * 4.0 + u.res_time.z + fk), p.y - u.res_time.z * (0.1 + 0.05 * fk) - u.flow.x * 0.02) * scale;
        let id = floor(q);
        let h = hash21(id + fk * 5.0);
        let r = 0.12 + 0.2 * fract(h * 17.0);
        let c = fract(q) - 0.5 - (vec2f(h, fract(h * 7.0)) - 0.5) * (0.4 - r);
        let d = length(c);
        let ring = smoothstep(0.03, 0.0, abs(d - r)) * 0.6 + smoothstep(r, r * 0.5, d) * 0.05;
        let glint = smoothstep(r * 0.25, 0.0, length(c - vec2f(-0.4, 0.4) * r));
        col += step(0.6, h) * (pal(0.55 + h * 0.2) * ring + vec3f(glint)) * (0.8 - fk * 0.25);
    }
    return vec4f(col * (0.7 + 0.4 * u.bands.y), 0.0);
}
