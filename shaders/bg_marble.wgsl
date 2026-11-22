// Background: black marble. Veins wind through the stone, drawn by warped
// noise, and the palette's light runs along them, brighter with the bass.

fn background(p: vec2f) -> vec3f {
    let q = p * 1.1 + vec2f(u.flow.y * 0.015, 0.0);
    let w = vec2f(fbm3(q * 1.5 + u.flow.y * 0.02), fbm3(q * 1.5 + 5.2 - u.flow.y * 0.015));
    let v = fbm(q * 2.0 + w * 2.5);
    let m = 0.5 + 0.5 * sin(q.x * 2.5 + q.y * 1.2 + v * 10.0);
    let vein = pow(1.0 - m, 14.0) + 0.35 * pow(1.0 - m, 4.0);
    let stone = mix(vec3f(0.012, 0.012, 0.014), pal(0.85) * 0.05, v);
    return stone + pal(0.55 + v * 0.3) * vein * (0.35 + 0.6 * u.bands.y);
}
