// Pattern: frames that come out of the centre, one on every beat, and grow
// toward the viewer like a corridor. Square, round or triangular, picked
// anew at each 16-beat phrase.

fn pattern(p: vec2f, base: vec3f) -> vec3f {
    let beats = u.flow.w;
    let shape = floor(hash21(vec2f(floor(beats / 16.0), 5.3)) * 3.0);
    var d = max(abs(p.x), abs(p.y));                                  // square
    if (shape == 1.0) { d = length(p); }                              // circle
    if (shape == 2.0) { d = max(abs(p.x) * 0.866 + p.y * 0.5, -p.y); } // triangle

    // Frames sit at equal steps of log(size), so each is twice the size of
    // the one inside it, like evenly spaced frames seen down a corridor.
    // Subtracting the beat count moves them all one step out per beat.
    let lz = log2(max(d, 1e-4)) * 1.4 - pace();  // twice as many frames in the last bar
    let gap = min(fract(lz), 1.0 - fract(lz));        // 0 on a frame
    let thick = 0.12 + 0.06 * u.beat.y;
    let aa = fwidth(lz) * 0.75;
    let frame = 1.0 - smoothstep(thick - aa, thick + aa, gap);

    // Tiny frames in the middle merge into noise; fade them out.
    let visible = smoothstep(0.03, 0.2, d);
    let colour = pal(0.7 + 0.25 * fract(floor(lz) * 0.37));
    // The frame that has just been born (inside the beat) is the brightest.
    let lit = colour * frame * visible * (1.0 + 0.6 * u.beat.y);
    return base * 0.15 + lit;
}
