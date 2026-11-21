// Pattern: a square spiral. Squares inside squares, each a little smaller and
// turned a little further than the one around it, sinking toward the centre
// one square per beat. The twist turns back and forth with the bar.

const SPIRAL_SQUARES: i32 = 28;
const SPIRAL_SHRINK: f32 = 0.86;

fn pattern(p: vec2f, base: vec3f) -> vec3f {
    let beats = pace();
    let sink = fract(beats);              // one square's worth per beat
    let eased = 1.0 - pow(1.0 - sink, 3.0);
    let twist = 0.11 + 0.05 * sin(u.flow.w * TAU / 16.0);   // breathes over the phrase
    let spin = u.flow.y * 0.05;
    var lit = vec3f(0.0);
    for (var i = 0; i < SPIRAL_SQUARES; i++) {
        let k = f32(i) + eased;                              // fractional place: they move inward
        let size = 1.6 * pow(SPIRAL_SHRINK, k);
        let q = rot(k * twist + spin) * p;
        let d = abs(max(abs(q.x), abs(q.y)) - size);
        let width = size * (0.035 + 0.02 * u.beat.y);
        let line = smoothstep(width + fwidth(d), width, d);
        let fade_in = smoothstep(0.0, 1.0, k);               // the outer square slides in
        let fade_out = smoothstep(0.015, 0.06, size);         // tiny ones melt away
        lit += pal(0.55 + k * 0.035) * line * fade_in * fade_out;
    }
    return base * 0.15 + lit * (0.9 + 0.5 * u.beat.y);
}
