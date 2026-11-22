// Pattern: three ribbons of the waveform, twisting as they run across the
// screen; a face turned to the light is bright, one seen edge-on is a line.

fn pattern(p: vec2f, base: vec3f) -> vec3f {
    let aspect = u.res_time.x / u.res_time.y;
    let x = clamp(p.x / aspect * 0.5 + 0.5, 0.0, 1.0);
    var col = base * 0.6;
    for (var k = 0; k < 3; k++) {
        let fk = f32(k);
        let twist = x * TAU * 1.5 + u.flow.y * 0.6 + fk * 2.1;
        let centre = wave(x) * 0.3 * (0.6 + u.bands.z) + (fk - 1.0) * 0.3 + 0.07 * sin(x * 5.0 + u.flow.x);
        let width = 0.012 + 0.06 * abs(cos(twist)) * (0.6 + spec(x * 0.5));
        let d = abs(p.y - centre);
        let body = smoothstep(width, width * 0.6, d);
        let shade = 0.3 + 0.7 * abs(sin(twist));
        col = mix(col, pal(0.15 * fk + x * 0.3 + u.flow.w * 0.05) * shade * 1.2, body * 0.92);
    }
    return col;
}
