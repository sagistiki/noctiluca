// Pattern: a tunnel of hearts. Heart outlines grow out of the centre toward
// the viewer, a new one on every beat, like the frames; the newest is the
// brightest and they all swell a little on the kick.

const HEARTS: i32 = 11;
const HEARTS_GROW: f32 = 1.5;

// Signed distance to a heart about one unit tall, its middle at the origin
// (after Inigo Quilez's exact heart).
fn hearts_sd(q: vec2f) -> f32 {
    var p = vec2f(abs(q.x), q.y + 0.55);
    if (p.y + p.x > 1.0) {
        return length(p - vec2f(0.25, 0.75)) - 0.35355;
    }
    let a = p - vec2f(0.0, 1.0);
    let b = p - 0.5 * max(p.x + p.y, 0.0);
    return sqrt(min(dot(a, a), dot(b, b))) * sign(p.x - p.y);
}

fn pattern(p: vec2f, base: vec3f) -> vec3f {
    let grow = fract(pace());
    let eased = 1.0 - pow(1.0 - grow, 3.0);
    var lit = vec3f(0.0);
    for (var i = 0; i < HEARTS; i++) {
        let k = f32(i) + eased;
        let size = 0.035 * pow(HEARTS_GROW, k);
        let d = abs(hearts_sd(p / size)) * size;
        let width = size * (0.012 + 0.008 * u.beat.y);
        let line = smoothstep(width + fwidth(d) * 1.5, width, d);
        let born = smoothstep(0.0, 1.0, k);                  // the newest fades in
        let gone = 1.0 - smoothstep(1.6, 2.6, size);          // the biggest leave the screen
        // The newest heart is the brightest; older ones dim as they come closer.
        let newest = 0.6 + 0.7 * exp(-k * 0.9);
        lit += pal(0.6 + 0.2 * fract(f32(i) * 0.37)) * line * born * gone * newest;
    }
    return base * 0.2 + lit * (0.9 + 0.6 * u.beat.y);
}
