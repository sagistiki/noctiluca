// Pattern: thick bars across the whole screen, so the screen itself becomes
// the pattern. They step one place on every beat, pick a new direction and
// count at each 16-beat phrase, and split in two for the last bar of it.
//
// Safety: bars move, they do not blink. Each step sweeps across, so no part
// of the screen changes faster than the beat, and the share of the screen
// that is lit stays about the same.

fn pattern(p: vec2f, base: vec3f) -> vec3f {
    let beats = u.flow.w;
    let phrase = floor(beats / 16.0);
    let in_phrase = beats - phrase * 16.0;

    // Direction for this phrase: vertical, horizontal, or one of two diagonals.
    let choice = floor(hash21(vec2f(phrase, 3.7)) * 4.0);
    var angle = 0.0;
    if (choice == 1.0) { angle = 1.5708; }
    if (choice == 2.0) { angle = 0.7854; }
    if (choice == 3.0) { angle = -0.7854; }
    let q = rot(angle) * p;

    // Three to five bars across the screen height, twice as many in the last
    // bar of the phrase.
    var count = 3.0 + floor(hash21(vec2f(phrase, 9.1)) * 3.0);
    count *= 1.0 + step(12.0, in_phrase);

    // On each beat the bars slide a third of a bar along, arriving within a
    // quarter of a beat and then holding still: a step, not a glide.
    // pace() doubles the stepping over the last bar of the phrase.
    let steps = pace();
    let step_in = 1.0 - pow(1.0 - min(fract(steps) * 4.0, 1.0), 3.0);
    let travel = (floor(steps) + step_in) / 3.0;

    let x = q.x * count * 0.5 + travel;
    let id = floor(x);
    let d = abs(fract(x) - 0.5);          // 0 in the middle of a bar, 0.5 between
    // About a third of the screen lit: more and echoes or mirrors fill it solid.
    let half = 0.16 + 0.04 * u.beat.y + 0.03 * u.bands.x;
    let aa = fwidth(x) * 0.75;
    var bar = 1.0 - smoothstep(half - aa, half + aa, d);

    // Some bars are hollow: only their edges are lit.
    let hollow = step(hash21(vec2f(id, phrase + 0.5)), 0.25);
    let edge = 1.0 - smoothstep(half * 0.55 - aa, half * 0.55 + aa, d);
    bar *= 1.0 - hollow * edge;

    // Brighter in the middle of each bar, like a light tube.
    let core = 1.0 - d / max(half, 1e-3);
    let colour = pal(0.75 + 0.2 * hash21(vec2f(id, 1.3)));
    let lit = colour * (0.6 + 0.7 * core) * (0.7 + 0.4 * u.beat.y);
    return base * 0.12 + lit * bar;
}
