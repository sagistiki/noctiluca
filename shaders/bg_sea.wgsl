// Background: the night sea. Waves roll in from the horizon; along their
// crests the foam breaks up, and where it does, plankton lights up in the
// palette's colour, with sparks as the water moves. The far water melts
// into the dark.

fn background(p: vec2f) -> vec3f {
    let horizon = 0.32;
    var col = vec3f(0.0, 0.004, 0.012) * (1.2 - p.y);
    let below = horizon - p.y;
    if (below > 0.0) {
        let z = 0.45 / (below + 0.03);
        let x = p.x * z;
        let t = u.flow.x * 0.6;
        // Wave fronts, bent along their length so they are not ruled lines.
        let h = sin(z * 9.0 - t * 2.2 + 2.2 * sin(x * 0.45 + z * 0.2) + 0.8 * fbm3(vec2f(x * 0.3, z * 0.5)))
            + 0.35 * sin(z * 15.0 - t * 3.1 + x * 1.3);
        // A thin crest on each front, broken into foam.
        let crest = exp(-pow((h - 1.05) * 5.0, 2.0));
        let breakup = smoothstep(0.35, 0.75, noise(vec2f(x * 2.2, z * 3.0) + vec2f(t * 0.4, 0.0)));
        let foam = crest * breakup * (0.8 + 0.6 * u.bands.y);
        // Sparks are pin-points on the screen, wherever there is foam.
        let spark = step(0.992, hash21(floor(p * 170.0) + floor(t * 5.0) * 3.7)) * smoothstep(0.15, 0.6, crest);
        let fade = exp(-z * 0.12) * smoothstep(0.0, 0.15, below);
        let water = pal(0.25) * 0.05 * (0.6 + 0.4 * h);
        col += (water + pal(0.45 + 0.08 * sin(x * 0.2)) * foam * 1.1 + pal(0.6) * spark * 2.2) * fade;
    }
    return col;
}
