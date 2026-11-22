// The scene pass: one background with one pattern drawn over it.
// Every bg_*.wgsl and pat_*.wgsl file is glued in above this one, renamed
// background_K and pattern_K, with background_at and pattern_at to call
// them by number. The draw call picks the pair by its instance number:
// background * 64 + pattern (SCENE_SLOTS in src/render/shaders.rs), plus
// 2048 for the look coming in, or 4096 for the one going out while it does.
//
// The look coming in is drawn over the one going out with the alpha this
// shader gives: the fade itself, an ink blot spreading, or a circle opening
// from the middle as the camera flies through the old look (u.overlay.w:
// 0 fade, 1 ink, 2 zoom through; u.state.y how far the change has come).

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
    let arriving = (in.look & 2048u) != 0u;
    let leaving = (in.look & 4096u) != 0u;
    let look = in.look % 2048u;
    let screen = centred(in.uv);
    let mix = clamp(u.state.y, 0.0, 1.0);
    let style = u32(round(u.overlay.w));
    var p = screen;
    var alpha = 1.0;
    if (arriving) {
        alpha = mix;
    }
    if (style == 1u && arriving) {
        // Ink: a blot with ragged edges that spreads from the middle out.
        let blot = fbm3(screen * 1.6 + vec2f(7.3, 1.1)) * 0.6 + length(screen) * 0.35;
        let reach = mix * 1.35 - 0.05;
        alpha = smoothstep(blot - 0.06, blot + 0.06, reach);
    }
    if (style == 2u) {
        if (leaving) {
            // Flying in: the old look grows past the camera.
            p = screen / (1.0 + mix * mix * 4.0);
        }
        if (arriving) {
            // The new one opens from the middle and settles from a little farther away.
            p = screen * (1.0 + (1.0 - mix) * 0.6);
            let r = mix * 2.3;
            alpha = smoothstep(r, r - 0.25, length(screen));
        }
    }
    let col = pattern_at(look % 64u, p, background_at(look / 64u, p));
    return vec4f(max(col, vec3f(0.0)), alpha);
}
