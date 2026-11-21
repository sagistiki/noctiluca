// Background: two laser grids, floor and ceiling, rushing toward the viewer.
// Tiles light up with the spectrum and the lines flash on every kick.

fn background(p: vec2f) -> vec3f {
    let tilt = rot(0.12 * sin(u.flow.y * 0.17)) * p;
    let y = tilt.y - 0.06 * sin(u.flow.y * 0.3);
    let dist = 1.0 / (abs(y) + 0.02);                       // far away near the horizon
    let plane = vec2f(tilt.x * dist, dist + u.flow.x * 4.0) * 0.5;

    let cell = floor(plane);
    let f = abs(fract(plane) - 0.5);
    // A line keeps its width on screen, so in grid units it must widen with distance.
    let width = 0.012 * dist + 0.01;
    let wire = smoothstep(0.5 - width, 0.5, max(f.x, f.y));
    let fog = exp(-dist * 0.16);

    let id = hash21(cell);
    // Tiles right under the camera would fill the screen, so they fade out.
    let lit = pow(spec(id), 3.0) * step(0.55, hash21(cell + 3.1)) * smoothstep(1.5, 5.0, dist);

    var col = pal(plane.y * 0.04 + id * 0.15) * fog
        * (wire * (0.35 + 0.5 * u.bands.y + 1.2 * u.beat.y) + lit * 0.9);
    // The horizon glows with the low end.
    col += pal(u.flow.x * 0.04) * exp(-abs(y) * 9.0) * (0.15 + 0.7 * u.bands.x + 0.5 * u.beat.y);
    return col;
}
