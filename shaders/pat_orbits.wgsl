// Pattern: little planets on circular orbits, the inner ones faster, each
// one swelling with its band.

fn pattern(p: vec2f, base: vec3f) -> vec3f {
    var col = base * 0.6;
    for (var k = 0; k < 5; k++) {
        let fk = f32(k);
        let r = 0.18 + fk * 0.15;
        col += pal(0.1 * fk) * smoothstep(0.003, 0.0, abs(length(p) - r)) * 0.2;
        for (var j = 0; j < 3; j++) {
            let fj = f32(j);
            let a = u.flow.y * (0.5 / (1.0 + fk)) * (1.0 + 0.3 * fj) + fj * TAU / 3.0 + fk;
            let d = length(p - r * vec2f(cos(a), sin(a)));
            let size = 0.01 + 0.02 * spec(0.1 + fk * 0.15);
            col += pal(0.1 * fk + 0.3) * (smoothstep(size, size * 0.5, d) * 1.4 + 0.0004 / (d * d + 0.002));
        }
    }
    return col;
}
