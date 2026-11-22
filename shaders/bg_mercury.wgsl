// Background: liquid mercury. A slow surface of liquid metal that mirrors
// a room: dark floor, a bright band of light at eye level, the palette's
// colour above. Ripples leave the centre with the sub bass.

fn merc_height(q: vec2f) -> f32 {
    return fbm3(q + vec2f(u.flow.y * 0.05, -u.flow.y * 0.04)) + 0.1 * sin(length(q) * 8.0 - u.flow.x * 2.0) * u.bands.x;
}

fn background(p: vec2f) -> vec3f {
    let q = p * 1.4;
    let e = 0.01;
    let n = normalize(vec3f(
        merc_height(q + vec2f(e, 0.0)) - merc_height(q - vec2f(e, 0.0)),
        merc_height(q + vec2f(0.0, e)) - merc_height(q - vec2f(0.0, e)),
        0.035,
    ));
    let r = reflect(vec3f(0.0, 0.0, -1.0), n);
    // The room it mirrors, by the height of the reflected ray.
    let floor_dark = vec3f(0.015, 0.015, 0.02);
    let band = exp(-pow((r.y - 0.15) * 7.0, 2.0)) * 1.1;
    let above = pal(0.5 + r.x * 0.3) * smoothstep(0.1, 0.8, r.y) * 0.6;
    let env = floor_dark + vec3f(0.85, 0.87, 0.9) * band + above;
    let metal = vec3f(0.75, 0.77, 0.8);
    return env * metal * (0.75 + 0.35 * u.bands.z);
}
