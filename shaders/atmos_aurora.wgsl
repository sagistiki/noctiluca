// Aurora: curtains of light waving slowly across the sky, brightening with
// the mids. Made for breakdowns, at home anywhere.

fn atmosphere(uv: vec2f) -> vec4f {
    let p = centred(uv);
    let t = u.flow.z * 0.15;
    var col = vec3f(0.0);
    for (var k = 0; k < 3; k++) {
        let fk = f32(k);
        let edge = 0.25 + 0.12 * fk + 0.12 * sin(p.x * 1.3 + t + fk * 2.0) + 0.08 * (fbm(vec2f(p.x * 0.8 + t * 0.5, fk)) - 0.5);
        let above = p.y - edge;
        let band = smoothstep(-0.02, 0.03, above) * exp(-max(above, 0.0) * (3.0 + fk));
        let rays = 0.6 + 0.4 * noise(vec2f(p.x * 25.0 + t * 3.0, fk * 5.0));
        col += pal(0.3 + fk * 0.2 + 0.1 * sin(p.x + t)) * band * rays * (0.8 + 0.8 * u.bands.z);
    }
    return vec4f(col, 0.0);
}
