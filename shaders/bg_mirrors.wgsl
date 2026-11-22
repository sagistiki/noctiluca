// Background: a mirror room. Frame inside frame inside frame, as between
// two mirrors facing each other, each with a row of lights chasing round
// it and lighting up with the beat.

fn mr_box(p: vec2f, b: vec2f) -> f32 {
    let d = abs(p) - b;
    return length(max(d, vec2f(0.0))) + min(max(d.x, d.y), 0.0);
}

fn background(p: vec2f) -> vec3f {
    var col = vec3f(0.0);
    let aspect = u.res_time.x / u.res_time.y;
    for (var i = 0; i < 10; i++) {
        let fi = f32(i);
        let s = pow(0.78, fi - fract(u.flow.x * 0.25));
        let q = rot(0.03 * fi * sin(u.flow.y * 0.2)) * p;
        let b = vec2f(aspect * 0.92, 0.92) * s;
        let d = abs(mr_box(q, b));
        let depth = exp(-fi * 0.28);
        col += pal(0.1 + fi * 0.04) * smoothstep(0.004, 0.0, d) * 0.35 * depth;
        let along = (atan2(q.y / b.y, q.x / b.x) / TAU + 0.5) * 24.0;
        let lamp = smoothstep(0.4, 0.1, length(vec2f(fract(along - u.flow.w * 0.25) - 0.5, d / (0.03 * s))));
        col += pal(0.5 + fi * 0.03) * lamp * depth * (0.3 + 0.9 * u.beat.y);
    }
    return col;
}
