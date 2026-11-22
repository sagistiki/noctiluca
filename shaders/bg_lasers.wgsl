// Background: lasers through haze. A fan of beams from below the stage,
// sweeping, each as bright as its slice of the spectrum, the haze lit
// around them.

fn background(p: vec2f) -> vec3f {
    let haze = 0.4 + 0.6 * fbm3(p * 2.0 + vec2f(u.flow.y * 0.05, u.flow.x * 0.02));
    let origin = vec2f(0.0, -1.15);
    var col = vec3f(0.003);
    for (var k = 0; k < 8; k++) {
        let fk = f32(k);
        let a = (fk - 3.5) * 0.17 + 0.35 * sin(u.flow.y * 0.3 + fk * 0.7);
        let dir = vec2f(sin(a), cos(a));
        let rel = p - origin;
        let d = abs(rel.x * dir.y - rel.y * dir.x);
        let ahead = step(0.0, dot(rel, dir));
        let level = 0.3 + spec(fk / 8.0 * 0.6);
        col += pal(fk * 0.06 + u.flow.w * 0.02) * ahead * (exp(-d * d * 30000.0) * 1.2 + exp(-d * 35.0) * 0.12 * haze) * level;
    }
    return col;
}
