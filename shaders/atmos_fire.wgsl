// Fire: flames licking up from the bottom of the screen. Their height
// follows the bass, and they flare on the kick.

// Black through deep red, orange and yellow to white, tinted toward the palette.
fn flame_colour(h: f32) -> vec3f {
    let ramp = vec3f(smoothstep(0.0, 0.4, h), smoothstep(0.25, 0.8, h), smoothstep(0.6, 1.0, h));
    return mix(ramp, ramp * (pal(0.85) + 0.3), 0.25);
}

fn atmosphere(uv: vec2f) -> vec4f {
    let p = centred(uv);
    let t = u.flow.z;
    let up = p.y + 1.0;   // 0 at the bottom of the screen
    let height = 0.3 + 0.45 * max(u.bands.x, u.bands.y) + 0.2 * u.beat.y;
    let q = vec2f(p.x * 2.6, up * 1.8 - t * 2.0);
    let turbulence = fbm(q + vec2f(1.5 * fbm(q * 0.7 + vec2f(0.0, -t)), 0.0));
    // Separate tongues of flame rather than one wall: the heat comes in
    // columns that rise and fall at their own pace.
    let tongues = 0.65 + 0.35 * sin(p.x * 9.0 + 2.0 * fbm(vec2f(p.x * 3.0, t * 0.7)));
    let heat = clamp((height * tongues - up) / height + (turbulence - 0.5) * 1.5, 0.0, 1.0);
    return vec4f(flame_colour(heat * 0.9) * heat * 1.6, heat * 0.35);
}
