// Background: silk. A sheet of fabric flowing in folds, its sheen sliding
// over them as they move.

fn silk_h(q: vec2f) -> f32 {
    let t = u.flow.y * 0.12;
    return sin(q.x * 2.0 + sin(q.y * 1.5 + t) * 1.5 + t * 0.7) * 0.5 + sin(q.y * 2.7 + q.x * 0.8 - t * 0.9) * 0.3;
}

fn background(p: vec2f) -> vec3f {
    let q = p * 1.4;
    let e = 0.01;
    let n = normalize(vec3f(silk_h(q + vec2f(e, 0.0)) - silk_h(q - vec2f(e, 0.0)), silk_h(q + vec2f(0.0, e)) - silk_h(q - vec2f(0.0, e)), 0.04));
    let light = normalize(vec3f(-0.4, 0.6, 0.7));
    let diffuse = max(dot(n, light), 0.0);
    let sheen = pow(max(dot(reflect(-light, n), vec3f(0.0, 0.0, 1.0)), 0.0), 18.0);
    return pal(0.3 + silk_h(q) * 0.15) * (0.05 + 0.3 * diffuse) * (0.7 + 0.4 * u.bands.y) + pal(0.8) * sheen * 0.4;
}
