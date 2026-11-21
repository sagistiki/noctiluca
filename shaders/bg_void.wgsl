// Background: almost nothing. A dark haze that drifts and swells a little
// with the sub bass, so that bold patterns and objects stand out on black.

fn background(p: vec2f) -> vec3f {
    let haze = fbm(p * 1.3 + vec2f(u.flow.z * 0.10, -u.flow.z * 0.05));
    let centre = exp(-dot(p, p) * 0.8);
    return pal(0.85 + 0.1 * haze) * (0.015 + centre * (0.05 * haze + 0.06 * u.bands.x));
}
