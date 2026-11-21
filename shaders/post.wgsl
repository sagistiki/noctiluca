// The effects pass. It mixes the fresh scene with the previous finished
// frame; that feedback loop is where trails, zoom tunnels and echoes come from.
//
//   u.post[0] = trail seconds, zoom per second, rotation per second, kaleidoscope segments
//   u.post[1] = colour split, wobble, glow, extra zoom on each kick

@group(1) @binding(0) var scene_tex: texture_2d<f32>;
@group(1) @binding(1) var prev_tex: texture_2d<f32>;
@group(1) @binding(2) var atmos_tex: texture_2d<f32>;
@group(1) @binding(3) var samp: sampler;

// Turns a colour around the colour wheel by `angle` radians without washing
// it out (a rotation about the grey axis).
fn hue_shift(c: vec3f, angle: f32) -> vec3f {
    let k = vec3f(0.57735);
    return c * cos(angle) + cross(k, c) * sin(angle) + k * dot(k, c) * (1.0 - cos(angle));
}

// Rainbow colour for a wavelength t in 0..1 (red to violet).
fn spectral(t: f32) -> vec3f {
    let h = t * 0.8;
    return clamp(vec3f(abs(h * 6.0 - 3.0) - 1.0, 2.0 - abs(h * 6.0 - 2.0), 2.0 - abs(h * 6.0 - 4.0)), vec3f(0.0), vec3f(1.0));
}

// The scene seen through a sheet of cut glass: the screen is divided into
// triangular facets, each facet shifts the picture its own way, and every
// shift spreads the light into a rainbow the way a prism does.
fn through_prism(at: vec2f, aspect: vec2f, amount: f32) -> vec3f {
    let g = rot(u.flow.y * 0.04) * at * 2.2;
    // Skew onto a triangle grid; `upper` says which half of the cell we are in.
    let s = vec2f(g.x - g.y * 0.57735, g.y * 1.1547);
    let cell = floor(s);
    let f = fract(s);
    let upper = step(1.0, f.x + f.y);
    let h = hash21(cell + upper * 17.3);
    let angle = h * TAU + u.flow.y * 0.3;
    let shift = vec2f(cos(angle), sin(angle)) * amount * (0.04 + 0.03 * u.beat.y);

    var sum = vec3f(0.0);
    var weight = vec3f(0.0);
    for (var i = 0; i < 7; i++) {
        let t = f32(i) / 6.0;
        let w = spectral(t);
        let uv = (at + shift * (0.3 + 1.2 * t)) / aspect + 0.5;
        sum += textureSampleLevel(scene_tex, samp, uv, 0.0).rgb * w;
        weight += w;
    }
    var col = sum / weight;

    // A thin bright line where two facets meet, as on a cut edge.
    let edge = min(min(f.x, f.y), abs(1.0 - f.x - f.y));
    col += pal(h) * smoothstep(0.035, 0.0, edge) * 0.35 * amount;
    return col;
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4f {
    let aspect = vec2f(u.res_time.x / u.res_time.y, 1.0);
    let dt = u.res_time.w;
    let centre = (in.uv - 0.5) * aspect;
    let r = length(centre);

    // Where to read the fresh scene: straight, or folded like a kaleidoscope.
    var scene_at = centre;
    let segments = u.post[0].w;
    if (segments >= 2.0) {
        let slice = TAU / segments;
        let a = abs(fract(atan2(centre.y, centre.x) / slice + 0.5) - 0.5) * slice;
        scene_at = r * vec2f(cos(a), sin(a));
    }
    let scene_uv = scene_at / aspect + 0.5;

    // Colour split: red and blue are read slightly further out and in, more so on a kick.
    let split = normalize(centre + 1e-5) / aspect * u.post[1].x * (0.4 + r) * (1.0 + 2.0 * u.beat.y);
    var fresh = vec3f(
        textureSample(scene_tex, samp, scene_uv + split).r,
        textureSample(scene_tex, samp, scene_uv).g,
        textureSample(scene_tex, samp, scene_uv - split).b,
    );
    let prism = u.post[2].w;
    if (prism > 0.0) {
        fresh = mix(fresh, through_prism(scene_at, aspect, prism), min(prism, 1.0));
    }

    // The atmosphere over everything: its alpha darkens (clouds, smoke),
    // its colour adds light (fire, beams, lightning).
    let atmos = textureSample(atmos_tex, samp, in.uv);
    fresh = fresh * (1.0 - atmos.a) + atmos.rgb;

    // Where to read the previous frame: slightly smaller and turned, so that
    // whatever was there appears to fly outward and rotate.
    let zoom = u.post[0].y * (1.0 + u.post[1].w * u.beat.y);
    var old_at = rot(u.post[0].z * dt) * centre * exp(-zoom * dt);
    let wobble = u.post[1].y;
    old_at += wobble * vec2f(
        noise(centre * 3.0 + u.flow.z) - 0.5,
        noise(centre * 3.0 - u.flow.z + 7.0) - 0.5,
    );
    var old = textureSample(prev_tex, samp, old_at / aspect + 0.5).rgb;

    // The echo fades with the chosen half-life and its hue drifts a little.
    let keep = select(0.0, exp(-dt / max(u.post[0].x, 1e-4)), u.post[0].x > 0.0);
    old = max(hue_shift(old, 0.4 * dt), vec3f(0.0)) * keep;

    // "Brightest wins" instead of adding: an echo can never pile up into white.
    return vec4f(max(fresh, old), 1.0);
}
