// Shooting stars: now and then a streak crosses the sky, its tail fading
// behind it, over a faint field of stars.

fn atmosphere(uv: vec2f) -> vec4f {
    let p = centred(uv);
    let aspect = u.res_time.x / u.res_time.y;
    var col = vec3f(step(0.998, hash21(floor(p * 140.0))) * 0.35);
    for (var k = 0; k < 6; k++) {
        let fk = f32(k);
        let period = 3.0 + 4.0 * hash21(vec2f(fk, 9.0));
        let tt = u.res_time.z + hash21(vec2f(fk, 4.0)) * 10.0;
        let cycle = floor(tt / period);
        let phase = fract(tt / period);
        let progress = phase / 0.22;
        if (progress > 1.0) {
            continue;
        }
        let start = vec2f((hash21(vec2f(cycle, fk)) * 2.0 - 0.6) * aspect, 0.3 + 0.7 * hash21(vec2f(fk, cycle + 3.0)));
        let dir = normalize(vec2f(-0.8 - 0.4 * hash21(vec2f(cycle, 7.0)), -0.45));
        let head = start + dir * progress * 1.6;
        let rel = p - head;
        let back = dot(rel, -dir);
        let side = rel.x * dir.y - rel.y * dir.x;
        let tail = step(0.0, back) * smoothstep(0.4, 0.0, back) * exp(-side * side * 40000.0);
        col += (vec3f(0.9) + pal(0.7) * 0.3) * (tail + exp(-dot(rel, rel) * 6000.0)) * sin(progress * 3.1416);
    }
    return vec4f(col, 0.0);
}
