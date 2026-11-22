// Background: plasma, the old demoscene effect: sines summed over the
// screen and looked up in the palette, flowing faster with the mids.

fn background(p: vec2f) -> vec3f {
    let t = u.flow.y * 0.4;
    let v = sin(p.x * 3.0 + t) + sin(p.y * 4.0 - t * 1.3) + sin((p.x + p.y) * 3.0 + t * 0.7)
        + sin(length(p * 4.0 + vec2f(sin(t), cos(t * 0.8)) * 2.0) - t);
    return pal(v * 0.15 + u.flow.x * 0.05) * (0.4 + 0.35 * u.bands.y);
}
