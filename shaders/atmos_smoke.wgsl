// Smoke: haze rolling low across the room and thinning upward, tinted by
// the palette, swelling with the sub bass.

fn atmosphere(uv: vec2f) -> vec4f {
    let p = centred(uv);
    let t = u.flow.z * 0.08;
    let q = p * vec2f(1.2, 2.2);
    let warp = vec2f(fbm(q + vec2f(t, 0.3)), fbm(q + vec2f(-t * 0.7, 4.1)));
    let puff = fbm(q * 1.3 + warp * 1.8 + vec2f(t * 1.5, 0.0));
    let low = smoothstep(0.6, -0.9, p.y);   // thick at the bottom, gone toward the top
    let density = clamp(smoothstep(0.25, 0.75, puff) * (0.3 + 0.9 * low) * (0.8 + 0.5 * u.bands.x), 0.0, 1.0);
    let col = pal(0.75) * 0.45 + vec3f(0.18);
    return vec4f(col * density * 1.1, density * 0.4);
}
