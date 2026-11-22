// 3D objects in chrome or glass. They have no colour of their own: they show
// the visuals around them, mirrored (chrome) or seen through and bent
// (glass), plus a few studio lights in the palette colour that pulse with
// the kick. Parts can be other things, by a code each corner carries (see
// `code` in src/render/model.rs): shining in the palette colour, glossy
// paint in the palette's colours, a light running round, a screen showing
// the spectrum; and they can sway (a fish's tail) or flap (a ray's wings).

// The picture behind the objects (the scene before any object was drawn).
@group(1) @binding(0) var behind: texture_2d<f32>;
@group(1) @binding(1) var samp: sampler;

// Must match ObjectUniforms in src/render/objects.rs.
struct Obj {
    model: mat4x4f,
    view_proj: mat4x4f,
    eye: vec4f,
    params: vec4f,   // material (0 chrome, 1 glass, 2 stone), body light 0..1, glow strength, sparkle
    extra: vec4f,    // how far parts sway, how fast (per second), paint pattern, unused
}
@group(2) @binding(0) var<uniform> obj: Obj;

struct ObjOut {
    @builtin(position) pos: vec4f,
    @location(0) world: vec3f,
    @location(1) normal: vec3f,
    // The part's code without its weight (0 body, 1 glow, 8 paint, 9 run,
    // 10 screen), and where on the model this is, for patterns.
    @location(2) glow: f32,
    @location(3) local: vec3f,
}

@vertex
fn vs_object(@location(0) p: vec3f, @location(1) n: vec3f, @location(2) glow: f32) -> ObjOut {
    var q = p;
    var g = glow;
    let code = floor(glow + 1e-4);
    let weight = glow - code;
    let phase = u.res_time.z * obj.extra.y * TAU;
    if (glow >= 1.5 && glow <= 3.0) {
        // Loudspeaker cones (2..3) jump forward on the kick and wobble with
        // the sub, most in the middle; the rest of the model stays put.
        q.z += (glow - 2.0) * (0.09 * u.beat.y + 0.03 * u.bands.x);
        g = 0.0;
    } else if (code == 4.0 || code == 12.0) {
        // A tail: a wave running down the body, strongest at the tip.
        q.z += sin(phase - weight * 3.0) * weight * weight * obj.extra.x;
        g = select(0.0, 8.0, code == 12.0);
    } else if (code == 6.0 || code == 14.0) {
        // Wings: a slow flap that ripples out to the tips.
        q.y += sin(phase - weight * 2.0) * weight * weight * obj.extra.x;
        g = select(0.0, 8.0, code == 14.0);
    } else {
        g = code;
    }
    let world = obj.model * vec4f(q, 1.0);
    var out: ObjOut;
    out.pos = obj.view_proj * world;
    out.world = world.xyz;
    // The model matrix only turns and scales evenly, so it keeps normals
    // perpendicular to the surface; they are renormalised per pixel.
    out.normal = (obj.model * vec4f(n, 0.0)).xyz;
    out.glow = g;
    out.local = p;
    return out;
}

// The palette's most vivid colour: the one that is both bright and
// coloured, wherever in the palette it lies (a smiley in the acid palette
// is its acid yellow, not the dark teal at the far end).
fn vivid() -> vec3f {
    var best = pal(0.0);
    var score = -1.0;
    for (var i = 0; i < 10; i++) {
        let c = pal(f32(i) / 10.0);
        let s = 0.6 * (max(c.r, max(c.g, c.b)) - min(c.r, min(c.g, c.b))) + 0.4 * dot(c, vec3f(0.3, 0.59, 0.11));
        if (s > score) {
            score = s;
            best = c;
        }
    }
    return best;
}

// An ornament in the palette's colours, as painted on a hamsa: petals and
// rings round the eye low in the palm, stripes and a diamond up the
// fingers, light lines on the palette's deep colour.
fn ornament(p: vec3f) -> vec3f {
    let e = p.xy - vec2f(0.0, -0.52);
    let r = length(e);
    let a = atan2(e.y, e.x);
    let petals = smoothstep(0.55, 0.9, 0.5 + 0.5 * cos(a * 14.0)) * smoothstep(0.34, 0.4, r) * (1.0 - smoothstep(0.62, 0.7, r));
    let rings = smoothstep(0.025, 0.0, abs(fract(r * 6.0) - 0.5) - 0.44) * smoothstep(0.3, 0.36, r);
    let up = smoothstep(0.05, 0.2, p.y);
    let stripes = smoothstep(0.03, 0.0, abs(fract(p.x * 4.6 + 0.5) - 0.5) - 0.45) * up;
    let q = p.xy - vec2f(0.0, 0.5);
    let diamond = smoothstep(0.03, 0.0, abs(abs(q.x) + abs(q.y) * 0.7 - 0.16)) * up;
    let dots = smoothstep(0.05, 0.02, length(fract(p.xy * 5.0) - 0.5)) * (1.0 - up) * smoothstep(0.75, 0.8, r) * 0.6;
    let deep = vivid() * 0.32 + vec3f(0.015);
    let light = mix(vivid(), vec3f(1.0), 0.45);
    let line = max(max(petals, rings * 0.8), max(max(stripes * 0.6, diamond), dots));
    return mix(deep, light, line);
}

// The colour of glossy paint at `p` on the model, from the palette, in the
// object's pattern: 0 a gradient up the body, 1 koi markings (red patches on
// white), 2 one colour, 3 a smiling face on the front (+Z), 4 an ornament.
fn paint_colour(p: vec3f) -> vec3f {
    let pattern = obj.extra.z;
    if (pattern < 0.5) {
        return pal(0.55 + p.y * 0.25);
    } else if (pattern < 1.5) {
        let spots = smoothstep(0.44, 0.5, noise(p.xz * 3.2 + vec2f(3.7, 1.3)) * 0.7 + noise(p.xz * 7.0) * 0.3);
        return mix(vec3f(0.78, 0.76, 0.72), pal(0.25) * 0.8, spots);
    } else if (pattern < 2.5) {
        return vivid();
    }
    if (pattern > 3.5) {
        return ornament(p);
    }
    // The face: two eyes and a smile, dark on the palette's colour.
    let front = smoothstep(0.1, 0.35, p.z);
    let eyes = min(length((p.xy - vec2f(-0.3, 0.25)) * vec2f(1.0, 0.6)), length((p.xy - vec2f(0.3, 0.25)) * vec2f(1.0, 0.6)));
    let r = length(p.xy - vec2f(0.0, 0.05));
    let smile = smoothstep(0.06, 0.03, abs(r - 0.5)) * smoothstep(0.0, -0.15, p.y - 0.05);
    let ink = max(smoothstep(0.13, 0.1, eyes), smile) * front;
    return mix(vivid(), vec3f(0.02), ink);
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
    if (in.glow > 0.5 && in.glow < 1.5) {
        return vec4f(pal(0.88) * (2.0 + 5.0 * u.beat.y) * obj.params.z, 1.0);
    }
    if (in.glow > 8.5 && in.glow < 9.5) {
        // A light that runs round the part once every four beats.
        let a = atan2(in.local.z, in.local.x) / TAU;
        let behind_head = fract(a - u.flow.w * 0.25);
        let lit = 0.25 + 3.0 * exp(-behind_head * 10.0) + 1.5 * exp(-(1.0 - behind_head) * 40.0);
        return vec4f(pal(0.88) * lit * (0.8 + 0.6 * u.beat.y) * obj.params.z, 1.0);
    }
    if (in.glow > 9.5 && in.glow < 10.5) {
        // A screen: the spectrum as columns across it, bass on the left.
        let x = clamp(in.local.x * 0.5 + 0.5, 0.0, 1.0);
        let level = spec(x);
        let height = clamp(in.local.y * 2.0 + 0.5, 0.0, 1.0);
        let column = step(0.25, fract(x * 28.0));
        let bar = step(height, level * 1.1) * column;
        return vec4f(pal(0.6 + x * 0.35) * (0.12 + 2.6 * bar) * obj.params.z, 1.0);
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
    if (in.glow > 7.5) {
        // Paint: a glossy lacquer in the palette's colours, lit by the room,
        // with the surroundings reflected at the edges.
        let albedo = paint_colour(in.local);
        let lit = max(dot(n, key), 0.0);
        let fill = max(dot(n, normalize(vec3f(-0.6, 0.2, 0.7))), 0.0);
        col = albedo * (vec3f(0.18) + pal(0.9) * lit * (0.9 + 0.5 * u.beat.y) + vec3f(0.4) * fill * 0.35)
            + surroundings(r) * (0.08 + 0.6 * fresnel)
            + highlight * 0.8;
    } else if (obj.params.x < 0.5) {
        // Chrome: a mirror, a little darker straight on than at the edges.
        col = surroundings(r) * mix(0.55, 1.0, fresnel) + highlight;
    } else if (obj.params.x > 1.5) {
        // Stone: a matte surface lit by the room: a key light from above in
        // the palette's colour that swells with the kick, a cool fill from
        // the left, a rim of light round the edge, and a little of the
        // visuals bouncing off it.
        // The key comes from the side, as the sun lights the moon, so
        // every crater throws its shadow.
        let albedo = vec3f(0.62, 0.6, 0.58);
        let sun = normalize(vec3f(0.85, 0.35, 0.4));
        let lit = max(dot(n, sun), 0.0);
        let fill = max(dot(n, normalize(vec3f(-0.6, 0.2, 0.7))), 0.0);
        let rim = pow(1.0 - facing, 3.0);
        col = albedo * (pal(0.9) * pow(lit, 1.3) * (1.4 + 0.6 * u.beat.y) + vec3f(0.025) + vec3f(0.35, 0.4, 0.5) * fill * 0.18)
            + surroundings(n) * 0.06 * albedo
            + pal(0.8) * rim * 0.3;
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
