// Background: a flight through a glowing 3D lattice (a gyroid). For every
// pixel a ray is walked forward in steps; each step adds light the closer it
// passes to the surface. The bass thickens the lattice.

fn lattice(p: vec3f) -> f32 {
    let g = dot(sin(p), cos(p.zxy));
    return abs(g) - (0.05 + 0.22 * u.bands.y + 0.12 * u.beat.y);
}

fn background(p: vec2f) -> vec3f {
    // The lattice repeats every TAU, so the camera can stay within one cell
    // however far it has "travelled". Small numbers keep the lattice sharp.
    let travel = u.flow.x * 2.5;
    let eye = vec3f(1.57, 0.0, travel - TAU * floor(travel / TAU));
    var dir = normalize(vec3f(p, 1.3));
    let roll = rot(u.flow.y * 0.12) * dir.xy;
    let yaw = rot(0.35 * sin(u.flow.y * 0.21)) * vec2f(roll.x, dir.z);
    dir = vec3f(yaw.x, roll.y, yaw.y);

    var col = vec3f(0.0);
    var t = 0.05;
    for (var i = 0; i < 56; i++) {
        let pos = eye + dir * t;
        let d = lattice(pos) * 0.6;
        let glow = 0.008 / (0.008 + abs(d));
        col += pal((travel + dir.z * t) * 0.07 + t * 0.03) * glow * glow * exp(-t * 0.25) * 0.22;
        t += max(abs(d) * 0.8, 0.03);
    }
    return col * (0.7 + 0.8 * u.beat.x);
}
