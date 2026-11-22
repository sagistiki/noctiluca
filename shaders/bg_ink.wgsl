// Background: ink dropped in water, blooming in slow clouds in the dark.

fn background(p: vec2f) -> vec3f {
    let t = u.flow.y * 0.05;
    let q = p * 1.1;
    let w1 = fbm3(q * 1.3 + vec2f(t, -t));
    let w2 = fbm3(q * 1.7 + w1 * 2.0 - vec2f(t * 0.7, 0.3));
    let v = fbm(q * 1.2 + w2 * 2.2);
    let ink = smoothstep(0.45, 0.85, v);
    let edge = smoothstep(0.02, 0.0, abs(v - 0.62)) * 0.25;
    return vec3f(0.005, 0.005, 0.01) + pal(0.6 + w2 * 0.4) * (ink * (0.3 + 0.35 * u.bands.x) + edge);
}
