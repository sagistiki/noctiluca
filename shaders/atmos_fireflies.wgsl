// Fireflies: warm points of light wandering in the dark, each glowing up
// and fading slowly on its own rhythm.

fn atmosphere(uv: vec2f) -> vec4f {
    let p = centred(uv);
    let aspect = u.res_time.x / u.res_time.y;
    var col = vec3f(0.0);
    for (var i = 0; i < 36; i++) {
        let fi = f32(i);
        let h = vec2f(hash21(vec2f(fi, 1.0)), hash21(vec2f(fi, 2.0)));
        let t = u.res_time.z * (0.05 + 0.05 * h.x);
        let at = vec2f((h.x * 2.0 - 1.0) * aspect + 0.3 * sin(t * 3.0 + fi), h.y * 2.0 - 1.0 + 0.25 * sin(t * 2.3 + fi * 1.7));
        let glow = 0.5 + 0.5 * sin(u.res_time.z * (0.8 + h.y) + fi * 2.1);
        let d = length(p - at);
        col += mix(vec3f(0.9, 1.0, 0.45), pal(0.3), 0.3) * (exp(-d * d * 3000.0) * 1.5 + exp(-d * 30.0) * 0.12) * glow * glow;
    }
    return vec4f(col * (0.8 + 0.4 * u.bands.z), 0.0);
}
