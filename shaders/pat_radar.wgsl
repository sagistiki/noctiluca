// Pattern: a radar screen. A sweep turns once every bar, leaving a fading
// trail; range rings and a cross mark the screen. Each kick drops a few
// contacts on the scope, and each one lights up when the sweep passes it,
// then fades until the sweep comes round again.

fn pattern(p: vec2f, base: vec3f) -> vec3f {
    let r = length(p);
    let beats = u.flow.w;
    let sweep = fract(beats / 4.0);                         // one turn per bar
    let a = fract(atan2(p.x, p.y) / TAU);                   // 0 at the top, clockwise
    let behind = fract(sweep - a);                          // how long ago the sweep passed here

    let edge = 0.95;
    let inside = smoothstep(edge + 0.01, edge - 0.01, r);
    let aa = fwidth(r) * 1.5;
    // Range rings, the cross and the rim.
    let rings = smoothstep(aa, 0.0, abs(fract(r * 4.0 / edge + 0.5) - 0.5) * edge / 4.0 - 0.002);
    let cross = smoothstep(aa, 0.0, min(abs(p.x), abs(p.y)) - 0.001) * inside;
    let rim = smoothstep(0.012, 0.0, abs(r - edge));
    // The beam and its afterglow, brighter on the kick.
    let trail = pow(1.0 - behind, 6.0) * inside;
    let beam = smoothstep(0.012, 0.0, behind * r * TAU) * inside;

    // Contacts: three per beat, from the last eight beats, each at its own
    // place. A contact is lit by the sweep and fades over the next turn.
    var blips = 0.0;
    for (var k = 0; k < 8; k++) {
        let b = floor(beats) - f32(k);
        for (var j = 0; j < 3; j++) {
            let h = vec2f(b, f32(j) * 7.1);
            let ang = hash21(h) * TAU;
            let dist = 0.2 + 0.7 * hash21(h + vec2f(3.3, 1.7));
            let at = dist * vec2f(sin(ang), cos(ang));
            let since = fract(sweep - ang / TAU);         // since the sweep crossed it
            let age = beats - b;                          // since it appeared
            let seen = step(since * 4.0, age);            // the sweep has passed it since it appeared
            let fade = pow(1.0 - since, 2.0) * (1.0 - age / 8.0);
            blips += smoothstep(0.03, 0.0, length(p - at)) * seen * fade;
        }
    }

    let colour = pal(0.6);
    let grid = (rings * inside + cross) * 0.25 + rim * 0.7;
    let lit = colour * (grid + trail * (0.35 + 0.4 * u.beat.y) + beam * 1.4)
        + pal(0.9) * blips * 2.0;
    return base * mix(0.3, 0.1, inside) + lit;
}
