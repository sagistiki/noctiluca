// Pattern: a hypnotic swirl. Two soft spiral arms wind into the centre and
// turn with the bass; the kick pulls them in a little tighter.

fn pattern(p: vec2f, base: vec3f) -> vec3f {
    let r = max(length(p), 1e-4);
    let a = atan2(p.y, p.x);
    let tight = 5.0 + 0.8 * u.beat.y;
    let s = sin(2.0 * a + log(r) * tight - u.flow.x * 0.9);
    let arm = smoothstep(0.1, 0.95, 0.5 + 0.5 * s);
    let fade = smoothstep(0.02, 0.18, r) * (1.0 - smoothstep(1.2, 2.2, r));
    let lit = mix(pal(0.45), pal(0.8), arm) * arm * fade;
    return base * (0.25 + 0.25 * (1.0 - arm)) + lit * (0.8 + 0.4 * u.beat.y);
}
