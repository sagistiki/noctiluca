// Background: a dusk sky with clouds racing across it, faster with the
// bass, and a low sun that glows with the sub.

fn background(p: vec2f) -> vec3f {
    var sky = mix(pal(0.05) * 0.06, pal(0.45) * 0.35, smoothstep(-1.0, 1.0, p.y));
    let sun = exp(-length(p - vec2f(0.0, -0.75)) * 3.0) * (0.25 + 0.4 * u.bands.x);
    sky += pal(0.7) * sun;
    let c = fbm(vec2f(p.x * 1.2 + u.flow.x * 0.15, p.y * 2.5) * 1.5);
    let cloud = smoothstep(0.45, 0.78, c);
    let lit = pal(0.8) * 0.3 * (0.5 + 0.5 * p.y);
    return mix(sky, lit + sun * 0.5, cloud * 0.75) * 0.8;
}
