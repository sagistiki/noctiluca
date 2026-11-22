// Background: an LED tunnel. Round lamps on the walls of a tunnel; light
// chases along them with the beat, each column on its own frequency.

fn background(p: vec2f) -> vec3f {
    let r = length(p) + 1e-4;
    let a = atan2(p.y, p.x) / TAU + 0.5;
    let depth = 0.4 / r + u.flow.x * 0.8;
    let g = vec2f(a * 48.0, depth * 6.0);
    let id = floor(g);
    let f = fract(g) - 0.5;
    let led = smoothstep(0.38, 0.2, length(f));
    let chase = 0.5 + 0.5 * sin(id.y * 0.7 - u.flow.w * TAU * 0.25 + id.x * 0.13);
    let lit = led * (0.08 + 1.1 * pow(chase, 3.0) * (0.3 + spec(fract(id.x / 48.0) * 0.7)));
    return pal(depth * 0.04 + id.x / 48.0 * 0.3) * lit * smoothstep(0.03, 0.45, r);
}
