// Background: a dive into the Mandelbrot set, toward the seahorse valley.
// The camera sinks in and comes back over a few minutes, deeper on builds.

fn background(p: vec2f) -> vec3f {
    let depth = 4.0 * (1.0 - cos(u.flow.x * 0.02)) + u.state.z * 1.5;
    let zoom = exp(-depth);
    let c = vec2f(-0.743643887, 0.131825904) + rot(u.flow.y * 0.05) * p * zoom * 1.2;
    var z = vec2f(0.0);
    var n = 0.0;
    var escaped = false;
    for (var i = 0; i < 160; i++) {
        z = vec2f(z.x * z.x - z.y * z.y, 2.0 * z.x * z.y) + c;
        if (dot(z, z) > 64.0) {
            escaped = true;
            break;
        }
        n += 1.0;
    }
    if (!escaped) {
        return vec3f(0.0);
    }
    let smooth_n = n - log2(log2(dot(z, z))) + 4.0;
    return pal(smooth_n * 0.02 + u.flow.w * 0.02) * (0.45 + 0.45 * cos(smooth_n * 0.15)) * (0.6 + 0.3 * u.bands.y);
}
