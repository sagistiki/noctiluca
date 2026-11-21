// The atmosphere pass: one atmosphere (from an atmos_*.wgsl file glued in
// above this one), drawn at half resolution. Its colour is added over the
// scene and objects in post.wgsl; its alpha darkens what is underneath, as
// clouds and smoke do.

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4f {
    return atmosphere(in.uv);
}
