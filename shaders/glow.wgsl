// Glow: keep only the bright parts of the picture and blur them. Blurring in
// two passes (across, then down) costs 26 samples instead of 169.

@group(1) @binding(0) var src: texture_2d<f32>;
@group(1) @binding(1) var samp: sampler;

const TAPS: i32 = 6;

fn bright(c: vec3f) -> vec3f {
    let luma = dot(c, vec3f(0.2126, 0.7152, 0.0722));
    return c * smoothstep(0.55, 1.6, luma);
}

fn weight(i: i32) -> f32 {
    let x = f32(i) / f32(TAPS);
    return exp(-3.0 * x * x);
}

// Across, reading the full-size picture into a quarter-size one.
@fragment
fn fs_across(in: VsOut) -> @location(0) vec4f {
    let step = vec2f(4.0 * 1.5 / f32(textureDimensions(src).x), 0.0);
    var sum = vec3f(0.0);
    var total = 0.0;
    for (var i = -TAPS; i <= TAPS; i++) {
        sum += bright(textureSample(src, samp, in.uv + step * f32(i)).rgb) * weight(i);
        total += weight(i);
    }
    return vec4f(sum / total, 1.0);
}

// Down, quarter-size to quarter-size.
@fragment
fn fs_down(in: VsOut) -> @location(0) vec4f {
    let step = vec2f(0.0, 1.5 / f32(textureDimensions(src).y));
    var sum = vec3f(0.0);
    var total = 0.0;
    for (var i = -TAPS; i <= TAPS; i++) {
        sum += textureSample(src, samp, in.uv + step * f32(i)).rgb * weight(i);
        total += weight(i);
    }
    return vec4f(sum / total, 1.0);
}
