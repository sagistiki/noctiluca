// The scene pass: one background with one pattern drawn over it.
// `background` and `pattern` come from the bg_*.wgsl and pat_*.wgsl file
// that the app glued in above this one.

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4f {
    let p = centred(in.uv);
    let col = pattern(p, background(p));
    return vec4f(max(col, vec3f(0.0)), 1.0);
}
