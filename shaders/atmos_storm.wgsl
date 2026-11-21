// Storm: heavy clouds drifting across the top of the room, glowing from
// inside on the kick, and lightning: a jagged bolt with a branch, and a
// flash through the clouds. The flash's strength (u.events.x) comes from
// the program, already limited to a safe rate; u.events.y picks the bolt.

// Where the bolt is at height y: big bends plus fine jitter.
fn bolt_x(y: f32, seed: f32, start: f32) -> f32 {
    return start + (fbm(vec2f(y * 1.8, seed)) - 0.5) * 0.9 + (noise(vec2f(y * 22.0, seed * 3.1)) - 0.5) * 0.07;
}

fn atmosphere(uv: vec2f) -> vec4f {
    let p = centred(uv);
    let t = u.flow.z * 0.06;
    // Clouds: warped noise, thicker toward the top.
    let q = p * vec2f(0.9, 1.6) + vec2f(t, 0.0);
    let warp = vec2f(fbm(q + vec2f(0.0, t * 2.0)), fbm(q + vec2f(5.2, -t)));
    let dense = fbm(q * 1.4 + warp * 1.5);
    let sky = smoothstep(-0.4, 0.35, p.y);
    let cloud = smoothstep(0.28, 0.62, dense) * sky;
    let light = u.events.x;
    // Clouds give off a little light of their own, so they read over black;
    // the kick glows inside them, and a strike lights them up.
    let inner = 0.4 + 0.4 * u.beat.y + 1.8 * light * smoothstep(0.4, 0.8, dense);
    let shade = mix(0.45, 1.0, smoothstep(0.35, 0.9, dense));   // darker underneath, lighter in the billows
    var col = mix(vec3f(0.35, 0.38, 0.45), pal(0.6), 0.35) * cloud * inner * shade;

    if (light > 0.01) {
        let seed = u.events.y;
        let aspect = u.res_time.x / u.res_time.y;
        let start = (fract(seed * 0.137) - 0.5) * 0.8 * aspect;
        let end = -0.2 - fract(seed * 0.71) * 0.8;
        let top = 1.1;
        if (p.y > end && p.y < top) {
            let d = abs(p.x - bolt_x(p.y, seed, start));
            var b = exp(-d * 250.0) + 0.25 * exp(-d * 30.0);
            // A branch splits off part of the way down and dies out.
            let split = mix(top, end, 0.35 + 0.3 * fract(seed * 0.33));
            if (p.y < split && p.y > split - 0.5) {
                let side = select(-1.0, 1.0, fract(seed * 0.91) > 0.5);
                let bx = bolt_x(p.y, seed + 7.0, bolt_x(split, seed, start)) + side * (split - p.y) * 0.5;
                let db = abs(p.x - bx);
                b += (exp(-db * 300.0) + 0.15 * exp(-db * 30.0)) * smoothstep(split - 0.5, split, p.y);
            }
            col += mix(vec3f(0.8, 0.85, 1.0), pal(0.9), 0.3) * b * light * 3.0;
        }
    }
    return vec4f(col, cloud * 0.65);
}
