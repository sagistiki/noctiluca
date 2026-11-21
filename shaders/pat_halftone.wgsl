// Pattern: halftone, like a printed poster. A grid of dots at 45 degrees
// whose size follows the picture under it, a wave that leaves the centre on
// every kick, and the bass. Bigger dots where it is brighter.

fn pattern(p: vec2f, base: vec3f) -> vec3f {
    let cells = 34.0 - 10.0 * u.bands.x;                  // coarser on the bass
    let q = rot(0.7854) * p * cells * 0.5;
    let id = floor(q) + 0.5;
    let centre = rot(-0.7854) * id / (cells * 0.5);       // the dot's centre back on the screen

    // What the dot shows, sampled at its centre so each dot is round.
    let r = length(centre);
    let wave = exp(-abs(r - u.beat.z * 2.2) * 6.0) * (1.0 - u.beat.z);
    let swirl = 0.5 + 0.5 * sin(r * 7.0 - u.flow.x * 0.6 + atan2(centre.y, centre.x) * 2.0);
    let shade = clamp(0.15 + 0.45 * swirl * (0.5 + u.beat.x) + 0.8 * wave, 0.0, 1.0);
    let luma = dot(base, vec3f(0.2126, 0.7152, 0.0722));
    let amount = clamp(max(shade, luma * 1.5), 0.0, 1.0);

    let d = length(q - id);
    let radius = 0.7 * sqrt(amount);                       // area grows with brightness
    let aa = fwidth(d);
    let dot_ = smoothstep(radius + aa, radius - aa, d);
    let colour = pal(0.4 + 0.35 * swirl + 0.2 * wave);
    return base * 0.1 + colour * dot_ * (0.9 + 0.4 * u.beat.y);
}
