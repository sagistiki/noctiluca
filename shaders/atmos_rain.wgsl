// Rain: fine streaks falling at a slant, in two layers, the room a little
// darker under it.

fn atmosphere(uv: vec2f) -> vec4f {
    let p = centred(uv);
    var a = 0.0;
    for (var k = 0; k < 2; k++) {
        let fk = f32(k);
        let q = vec2f((p.x - p.y * 0.12) * (55.0 + fk * 35.0), p.y * 3.0 + u.res_time.z * (7.0 + fk * 2.5));
        let column = floor(q.x);
        let h = hash21(vec2f(column, fk * 3.0));
        let y = fract(q.y * 0.5 + h * 9.0);
        let streak = smoothstep(0.0, 0.04, y) * smoothstep(0.3, 0.05, y);
        let thin = smoothstep(0.12, 0.0, abs(fract(q.x) - 0.5));
        a += step(0.75, h) * streak * thin * (0.6 - fk * 0.2);
    }
    return vec4f((vec3f(0.6, 0.65, 0.75) + pal(0.6) * 0.2) * a, 0.12);
}
