// Pattern: blinking eyes. A sparse grid of graphic eyes (five by three)
// that blink on the kick and wander about; on a drop they all look at the
// middle for a few seconds.

// Distance to an almond eye shape, open by `open` (0 shut, 1 wide).
fn eyes_lid(q: vec2f, open: f32) -> f32 {
    let h = 0.55 * max(open, 0.02);
    let r = (1.0 + h * h) / (2.0 * h);                 // circles through the eye's corners
    let top = length(q - vec2f(0.0, h - r)) - r;
    let bottom = length(q + vec2f(0.0, h - r)) - r;
    return max(top, bottom);
}

fn pattern(p: vec2f, base: vec3f) -> vec3f {
    let cell = vec2f(0.72, 0.62);
    let id = round(p / cell);
    if (abs(id.x) > 2.0 || abs(id.y) > 1.0) {
        return base * 0.3;
    }
    let q = (p - id * cell) / 0.26;
    let h = hash21(id + 4.0);
    // On each kick a few of them blink, different ones every beat.
    let chosen = step(0.72, hash21(id * 1.3 + floor(u.flow.w) * 7.1));
    let blink = smoothstep(0.55, 0.95, u.beat.y) * chosen;
    let open = 1.0 - blink * 0.95;
    let lid = eyes_lid(q, open);
    let aa = fwidth(lid) * 1.5;
    let outline = smoothstep(0.07 + aa, 0.07, abs(lid));
    let inside = smoothstep(aa, -aa, lid);
    // Where it looks: wandering, or at the middle just after a drop.
    let wander = vec2f(noise(vec2f(u.flow.y * 0.3, h * 9.0)), noise(vec2f(h * 7.0, u.flow.y * 0.3))) - 0.5;
    let centre = -normalize(id + vec2f(1e-3)) * 0.35 * select(1.0, 0.0, length(id) < 0.5);
    let look = mix(wander * 0.7, centre, exp(-u.stage.y * 0.6));
    let d = length(q - look * 0.5);
    let iris = smoothstep(0.42 + aa, 0.42, d) * inside;
    let pupil = smoothstep(0.17 + aa, 0.17, d) * inside;
    let colour = pal(0.7 + 0.2 * fract(h * 3.7));
    let lit = colour * (outline + iris * 0.8);
    return base * 0.3 * (1.0 - inside * 0.6) + lit * (0.9 + 0.4 * u.beat.y) - vec3f(pupil) * 0.6 * iris;
}
