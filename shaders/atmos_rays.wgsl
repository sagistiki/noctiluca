// God rays: shafts of light falling at a slant from high on the left,
// through dust that drifts in them, brighter with the sub.

fn atmosphere(uv: vec2f) -> vec4f {
    let p = centred(uv);
    let aspect = u.res_time.x / u.res_time.y;
    let rel = p - vec2f(-0.9 * aspect, 1.35);
    let angle = atan2(rel.x, -rel.y);
    let rays = pow(noise(vec2f(angle * 18.0, u.flow.y * 0.08)), 3.0) + 0.6 * pow(noise(vec2f(angle * 41.0 + 3.0, u.flow.y * 0.05)), 4.0);
    let fall = exp(-length(rel) * 0.55);
    let dust = 0.6 + 0.4 * fbm3(p * 3.0 + vec2f(u.res_time.z * 0.05, 0.0));
    let light = rays * fall * dust * (0.3 + 0.35 * u.bands.x);
    return vec4f((pal(0.7) * 0.5 + vec3f(0.35)) * light, 0.0);
}
