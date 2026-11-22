// Background: a raw concrete wall, the kind under a railway arch: panel
// seams, the holes the formwork ties leave, and two coloured lights moving
// over it.

fn background(p: vec2f) -> vec3f {
    let tex = 0.5 + 0.5 * fbm(p * 6.0);
    let pores = step(0.84, noise(p * 90.0));
    var wall = vec3f(0.1, 0.1, 0.105) * (0.6 + 0.6 * tex) * (1.0 - pores * 0.3);
    let panel = p * vec2f(1.2, 1.6);
    let seam = max(smoothstep(0.485, 0.5, abs(fract(panel.x) - 0.5)), smoothstep(0.485, 0.5, abs(fract(panel.y) - 0.5)));
    wall *= 1.0 - 0.6 * seam;
    let hole = smoothstep(0.035, 0.025, length(fract(panel * 2.0) - 0.5));
    wall *= 1.0 - 0.7 * hole;
    let s1 = vec2f(0.8 * sin(u.flow.y * 0.3), 0.35 * cos(u.flow.y * 0.23));
    let s2 = vec2f(0.8 * sin(u.flow.y * 0.21 + 2.0), 0.4 * sin(u.flow.y * 0.17));
    let light = pal(0.15) * exp(-length(p - s1) * 2.6) * (1.2 + 1.5 * u.bands.x) + pal(0.65) * exp(-length(p - s2) * 2.6) * (1.0 + 1.5 * u.bands.y);
    return wall * (0.15 + light * 2.5);
}
