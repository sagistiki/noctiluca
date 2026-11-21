// 3D objects in chrome or glass. They have no colour of their own: they show
// the visuals around them, mirrored (chrome) or seen through and bent
// (glass), plus a few studio lights in the palette colour that pulse with
// the kick. Parts named "..._glow" in Blender shine instead.

// The picture behind the objects (the scene before any object was drawn).
@group(1) @binding(0) var behind: texture_2d<f32>;
@group(1) @binding(1) var samp: sampler;

// Must match ObjectUniforms in src/render/objects.rs.
struct Obj {
    model: mat4x4f,
    view_proj: mat4x4f,
    eye: vec4f,
    params: vec4f,   // material (0 chrome, 1 glass), body light 0..1, glow strength, unused
}
@group(2) @binding(0) var<uniform> obj: Obj;

struct ObjOut {
    @builtin(position) pos: vec4f,
    @location(0) world: vec3f,
    @location(1) normal: vec3f,
    @location(2) glow: f32,
}

@vertex
fn vs_object(@location(0) p: vec3f, @location(1) n: vec3f, @location(2) glow: f32) -> ObjOut {
    // Loudspeaker cones (glow 2..3) jump forward on the kick and wobble with
    // the sub, most in the middle; the rest of the model stays put.
    var q = p;
    var g = glow;
    if (glow >= 1.5) {
        let weight = glow - 2.0;
        q.z += weight * (0.09 * u.beat.y + 0.03 * u.bands.x);
        g = 0.0;
    }
    let world = obj.model * vec4f(q, 1.0);
    var out: ObjOut;
    out.pos = obj.view_proj * world;
    out.world = world.xyz;
    // The model matrix only turns and scales evenly, so it keeps normals
    // perpendicular to the surface; they are renormalised per pixel.
    out.normal = (obj.model * vec4f(n, 0.0)).xyz;
    out.glow = g;
    return out;
}

// What the object sees when it looks in direction d: the visuals, as if
// they covered a big screen around the room, and three studio lights.
fn surroundings(d: vec3f) -> vec3f {
    let uv = vec2f(0.5 + d.x * 0.42, 0.5 - d.y * 0.42);
    // Five samples, slightly blurred: a sharp reflection of sharp stripes
    // turns small surface details into flickering specks.
    let r = 0.012;
    var c = (textureSampleLevel(behind, samp, uv, 0.0).rgb * 2.0
        + textureSampleLevel(behind, samp, uv + vec2f(r, 0.0), 0.0).rgb
        + textureSampleLevel(behind, samp, uv - vec2f(r, 0.0), 0.0).rgb
        + textureSampleLevel(behind, samp, uv + vec2f(0.0, r), 0.0).rgb
        + textureSampleLevel(behind, samp, uv - vec2f(0.0, r), 0.0).rgb) / 6.0 * 1.3;
    let kick = 0.6 + 0.9 * u.beat.y;
    let side = smoothstep(0.5, 0.0, abs(d.y));
    c += pal(0.9) * smoothstep(0.75, 0.95, d.y) * 2.5 * kick;          // a light above
    c += vec3f(1.0) * smoothstep(0.8, 0.97, d.x) * side * 1.5;         // a white strip on the right
    c += pal(0.6) * smoothstep(0.8, 0.97, -d.x) * side * 1.2 * kick;   // a coloured strip on the left
    c += pal(0.75) * exp(-abs(d.y + 0.15) * 18.0) * 0.6;               // the horizon of the floor
    return c;
}

@fragment
fn fs_object(in: ObjOut, @builtin(front_facing) front: bool) -> @location(0) vec4f {
    if (in.glow > 0.5) {
        return vec4f(pal(0.88) * (2.0 + 5.0 * u.beat.y) * obj.params.z, 1.0);
    }
    var n = normalize(in.normal);
    if (!front) {
        n = -n;
    }
    let v = normalize(obj.eye.xyz - in.world);
    let r = reflect(-v, n);
    let facing = clamp(dot(n, v), 0.0, 1.0);
    let fresnel = pow(1.0 - facing, 5.0);
    let key = normalize(vec3f(0.5, 0.8, 0.6));
    let highlight = vec3f(pow(max(dot(r, key), 0.0), 90.0) * (2.0 + 4.0 * u.beat.y));

    var col: vec3f;
    if (obj.params.x < 0.5) {
        // Chrome: a mirror, a little darker straight on than at the edges.
        col = surroundings(r) * mix(0.55, 1.0, fresnel) + highlight;
    } else {
        // Glass: the picture behind, bent by the surface and split into
        // colours, with the mirror image taking over at grazing angles.
        // The camera looks straight down -Z, so the normal's X and Y say
        // which way on screen the surface leans.
        let uv = in.pos.xy / vec2f(textureDimensions(behind));
        let bend = n.xy * vec2f(1.0, -1.0) * 0.07;
        let seen = vec3f(
            textureSampleLevel(behind, samp, uv + bend * 1.0, 0.0).r,
            textureSampleLevel(behind, samp, uv + bend * 1.2, 0.0).g,
            textureSampleLevel(behind, samp, uv + bend * 1.4, 0.0).b,
        );
        col = mix(seen * vec3f(0.85, 0.92, 1.0), surroundings(r), 0.06 + 0.94 * fresnel) + highlight;
        col += pal(0.8) * pow(1.0 - facing, 3.0) * 0.5;   // the bright rim glass gets
    }
    // The mirror ball: each flat tile is a little mirror that flashes when it
    // catches one of four lights as the ball turns, and single tiles twinkle
    // at random, more of them the busier the hi-hats.
    let sparkle = obj.params.w;
    if (sparkle > 0.0) {
        let lights = array<vec3f, 4>(
            normalize(vec3f(0.6, 0.7, 0.4)), normalize(vec3f(-0.7, 0.4, 0.6)),
            normalize(vec3f(0.2, -0.3, 1.0)), normalize(vec3f(-0.3, 0.9, -0.2)),
        );
        for (var i = 0; i < 4; i++) {
            col += pal(0.25 * f32(i) + 0.8) * pow(max(dot(r, lights[i]), 0.0), 400.0) * 6.0 * sparkle;
        }
        // The tile's normal is the same over the whole tile, so it names it.
        let tile = hash21(floor(n.xy * 60.0) + floor(n.z * 60.0) * 0.37 + floor(u.res_time.z * 6.0));
        col += vec3f(step(0.985 - 0.05 * u.bands.w, tile)) * 3.0 * sparkle;
    }
    // Copies that are not their turn (the skull wall) are dimmed.
    return vec4f(col * obj.params.y, 1.0);
}

// Lays the scene down in the multisampled picture before the objects.
@fragment
fn fs_backdrop(in: VsOut) -> @location(0) vec4f {
    return textureSampleLevel(behind, samp, in.uv, 0.0);
}
