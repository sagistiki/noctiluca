// Background: a nebula. Clouds of gas lit in the palette's colours, their
// cores glowing with the sub bass, and stars twinkling behind.

fn background(p: vec2f) -> vec3f {
    let q = p * 0.9 + vec2f(u.flow.y * 0.01, u.flow.x * 0.005);
    let n = fbm(q * 1.4 + fbm3(q * 2.0 + u.flow.y * 0.02) * 1.5);
    let dust = smoothstep(0.35, 0.95, n);
    var col = pal(n * 0.6 + 0.2) * dust * dust * (0.3 + 0.25 * u.bands.x) + pal(0.8) * pow(dust, 6.0) * 0.5;
    let s = hash21(floor(p * 180.0));
    col += step(0.997, s) * vec3f(0.8) * (0.6 + 0.4 * sin(u.res_time.z * 2.0 + s * 50.0));
    return col;
}
