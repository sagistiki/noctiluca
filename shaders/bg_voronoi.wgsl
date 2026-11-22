// Background: cells of coloured glass. Each cell glows with its own slice
// of the spectrum; the leading between them catches the light on the beat.

fn background(p: vec2f) -> vec3f {
    let q = p * 4.0 + vec2f(u.flow.y * 0.05, 0.0);
    let i = floor(q);
    let f = fract(q);
    var d1 = 8.0;
    var d2 = 8.0;
    var cell = vec2f(0.0);
    for (var y = -1; y <= 1; y++) {
        for (var x = -1; x <= 1; x++) {
            let g = vec2f(f32(x), f32(y));
            let o = 0.5 + 0.4 * sin(u.flow.y * 0.3 + TAU * vec2f(hash21(i + g), hash21(i + g + 3.1)));
            let r = g + o - f;
            let d = dot(r, r);
            if (d < d1) {
                d2 = d1;
                d1 = d;
                cell = i + g;
            } else if (d < d2) {
                d2 = d;
            }
        }
    }
    let edge = sqrt(d2) - sqrt(d1);
    let lit = spec(hash21(cell));
    return pal(hash21(cell) * 0.5 + u.flow.w * 0.02) * (0.06 + 0.5 * lit * lit) + pal(0.9) * smoothstep(0.05, 0.0, edge) * (0.25 + 0.6 * u.beat.y);
}
