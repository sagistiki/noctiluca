// Pattern: a sunburst. Rays from the centre turn slowly with the bass and
// open a little wider on every kick; every phrase picks how many there are.

fn pattern(p: vec2f, base: vec3f) -> vec3f {
    let phrase = floor(u.flow.w / 16.0);
    let rays = 12.0 + 6.0 * floor(hash21(vec2f(phrase, 8.1)) * 3.0);     // 12, 18 or 24
    let a = atan2(p.y, p.x) / TAU + u.flow.x * 0.01;
    let r = length(p);
    let s = abs(fract(a * rays) - 0.5) * 2.0;                             // 0 in a ray's middle
    let open = 0.32 + 0.12 * u.beat.y;
    let aa = fwidth(s) * 1.5;
    let ray = 1.0 - smoothstep(open - aa, open + aa, s);
    // Rays grow out of a bright core and fade toward the edges.
    let core = exp(-r * 7.0) * (0.8 + 1.2 * u.beat.y);
    let reach = smoothstep(0.04, 0.25, r) * (1.0 - smoothstep(0.9, 1.8, r));
    let lit = pal(0.72 + 0.15 * r) * ray * reach + pal(0.9) * core;
    return base * mix(0.55, 0.2, ray * reach) + lit * (0.8 + 0.4 * u.beat.y);
}
