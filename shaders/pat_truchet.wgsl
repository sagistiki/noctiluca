// Pattern: Truchet tiles. Every tile holds two quarter circles; flipped at
// random they join into endless winding paths. Tiles flip anew every bar,
// and each lights with its own slice of the spectrum.

fn pattern(p: vec2f, base: vec3f) -> vec3f {
    let size = 6.0;
    let g = p * size + vec2f(0.0, u.flow.x * 0.4);
    let id = floor(g);
    var f = fract(g) - 0.5;
    if (hash21(id + floor(pace() / 4.0)) > 0.5) {
        f.x = -f.x;
    }
    let d = abs(min(length(f - 0.5), length(f + 0.5)) - 0.5);
    let w = 0.06 + 0.05 * u.beat.y;
    let line = smoothstep(w, w - 0.03, d);
    let lit = spec(hash21(id) * 0.6);
    return base * 0.5 + pal(hash21(id) * 0.3 + u.flow.w * 0.03) * line * (0.35 + 1.2 * lit);
}
