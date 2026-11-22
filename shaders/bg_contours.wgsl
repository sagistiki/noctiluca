// Background: a topographic map. Contour lines of a slowly shifting
// landscape, every fifth one drawn heavier, as on a real map.

fn background(p: vec2f) -> vec3f {
    let h = fbm(p * 1.3 + vec2f(u.flow.y * 0.02, 0.0)) + 0.08 * sin(length(p) * 3.0 - u.flow.x * 0.5);
    let level = h * 14.0 - u.flow.x * 0.05;
    let c = abs(fract(level) - 0.5);
    let line = smoothstep(0.08, 0.0, c - 0.42);
    // Every fifth line is an index contour, drawn heavier.
    let index = step(fract(floor(level + 0.5) / 5.0 + 0.01), 0.1);
    let heavy = select(0.45, 1.0, index > 0.5);
    return vec3f(0.01) + pal(h * 0.8) * line * heavy * (0.25 + 0.5 * u.bands.y);
}
