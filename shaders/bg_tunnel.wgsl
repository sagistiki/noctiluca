// Background: a tunnel of panels. The panels light up with the spectrum,
// the wireframe and the core flash on every kick, and the bass drives the speed.

fn background(p: vec2f) -> vec3f {
    let q = rot(0.25 * sin(u.flow.y * 0.2)) * p;
    let r = length(q) + 1e-4;
    let angle = atan2(q.y, q.x) / TAU + 0.5;   // 0..1 around the tunnel
    let depth = 0.5 / r + u.flow.x;            // grows toward the centre

    let sides = 12.0;
    let grid = vec2f(depth * 2.0, angle * sides);
    let cell = floor(grid);
    let f = fract(grid);
    let id = hash21(cell);

    // Roughly two thirds of the panels are live; each follows one slice of
    // the spectrum, picked by its id.
    let live = step(0.35, hash21(cell + 7.3));
    let lit = pow(spec(id), 2.0) * live;

    let edge = min(min(f.x, 1.0 - f.x), min(f.y, 1.0 - f.y));
    let wire = smoothstep(0.07, 0.0, edge);

    var col = pal(depth * 0.06 + id * 0.2) * (lit * 1.6 + wire * (0.2 + 0.8 * u.bands.y + 0.8 * u.beat.y));

    // Distance fog, then a core that swells with the sub bass.
    col *= smoothstep(0.02, 0.55, r);
    col += pal(u.flow.x * 0.05 + 0.5) * (0.004 + 0.02 * u.bands.x + 0.03 * u.beat.y) / (r * r + 0.015);
    return col;
}
