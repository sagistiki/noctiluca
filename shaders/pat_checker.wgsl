// Pattern: a checkerboard bent by a ring wave that leaves the centre with
// the bass, and twisted more toward the edges.

fn pattern(p: vec2f, base: vec3f) -> vec3f {
    let r = length(p);
    let warp = p * (1.0 + 0.3 * sin(r * 4.0 - u.flow.x * 1.2) * (0.5 + u.bands.x));
    let q = rot(u.flow.y * 0.08 + r * 0.5 * sin(u.flow.y * 0.1)) * warp * 5.0;
    let s = floor(q.x) + floor(q.y);
    let check = s - 2.0 * floor(s / 2.0);
    let edge = min(abs(fract(q.x) - 0.5), abs(fract(q.y) - 0.5));
    let soft = smoothstep(0.5, 0.47, edge);
    return mix(base * 0.35, pal(0.3 + r * 0.2 + u.flow.w * 0.02), check * soft * 0.8) * (0.6 + 0.5 * u.bands.y);
}
