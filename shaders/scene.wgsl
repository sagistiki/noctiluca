// The scene pass: one background with one pattern drawn over it.
// Every bg_*.wgsl and pat_*.wgsl file is glued in above this one, renamed
// background_K and pattern_K, with background_at and pattern_at to call
// them by number. The draw call picks the pair: its instance number is
// background * 64 + pattern (SCENE_SLOTS in src/render/shaders.rs).

struct SceneOut {
    @builtin(position) pos: vec4f,
    @location(0) uv: vec2f,
    @location(1) @interpolate(flat) look: u32,
}

@vertex
fn vs_scene(@builtin(vertex_index) i: u32, @builtin(instance_index) look: u32) -> SceneOut {
    // The same one triangle over the whole screen as vs_main.
    let q = vec2f(f32((i << 1u) & 2u), f32(i & 2u));
    var out: SceneOut;
    out.pos = vec4f(q * 2.0 - 1.0, 0.0, 1.0);
    out.uv = vec2f(q.x, 1.0 - q.y);
    out.look = look;
    return out;
}

@fragment
fn fs_main(in: SceneOut) -> @location(0) vec4f {
    let p = centred(in.uv);
    let col = pattern_at(in.look % 64u, p, background_at(in.look / 64u, p));
    return vec4f(max(col, vec3f(0.0)), 1.0);
}
