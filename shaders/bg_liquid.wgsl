// Background: slow coloured liquid. Noise is used to bend the coordinates of
// more noise ("domain warping"), which gives the marbled, flowing look. The
// bass bends it harder.

fn background(p: vec2f) -> vec3f {
    let q = p * 1.2;
    let t = u.flow.z * 0.35;
    // The two warp fields only bend the coordinates, so they need no fine
    // detail: three octaves instead of five (this background is expensive).
    let a = vec2f(fbm3(q + vec2f(0.0, t)), fbm3(q + vec2f(5.2, -0.8 * t)));
    let b = vec2f(
        fbm3(q + 3.0 * a + vec2f(1.7, 9.2) + 0.15 * u.flow.x),
        fbm3(q + 3.0 * a + vec2f(8.3, 2.8) - 0.12 * u.flow.y),
    );
    let f = fbm(q + (2.2 + 2.0 * u.bands.y) * b);

    var col = pal(f * 1.4 + 0.5 * length(b) + u.flow.x * 0.03);
    // Push the contrast: most of the screen stays dark so the bright folds stand out.
    let body = smoothstep(0.30, 0.75, f);
    col *= 0.03 + 1.2 * body * body * (0.45 + 0.8 * u.beat.x);
    // Bright veins where the liquid folds over itself; the highs make them sparkle.
    let vein = smoothstep(0.03, 0.0, abs(f - 0.5));
    col += pal(f + 0.3) * vein * (0.12 + 1.0 * u.bands.w + 0.6 * u.beat.y);
    return col;
}
