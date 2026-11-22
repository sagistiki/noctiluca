// The last pass: takes the finished picture and puts it on the screen,
// with the data overlay (HUD) on top when it is switched on.

@group(1) @binding(0) var src: texture_2d<f32>;
@group(1) @binding(1) var glow_tex: texture_2d<f32>;
@group(1) @binding(2) var samp: sampler;

// Squeezes bright values into the displayable range without clipping them
// to flat white (the ACES filmic curve, Narkowicz fit).
fn tonemap(x: vec3f) -> vec3f {
    return clamp((x * (2.51 * x + 0.03)) / (x * (2.43 * x + 0.59) + 0.14), vec3f(0.0), vec3f(1.0));
}

// 1 inside the rectangle lo..hi, 0 outside.
fn box(p: vec2f, lo: vec2f, hi: vec2f) -> f32 {
    let inside = step(lo, p) * step(p, hi);
    return inside.x * inside.y;
}

// A seven-segment digit. p runs 0..0.6 across and 0..1 down the digit.
fn digit(p: vec2f, n: u32) -> f32 {
    // One bit per segment: a (top), b, c (right side), d (bottom), e, f (left side), g (middle).
    var masks = array<u32, 10>(0x3Fu, 0x06u, 0x5Bu, 0x4Fu, 0x66u, 0x6Du, 0x7Du, 0x07u, 0x7Fu, 0x6Fu);
    let m = masks[min(n, 9u)];
    let t = 0.11;
    let w = 0.6;
    var lit = 0.0;
    lit += f32(m & 1u) * box(p, vec2f(t, 0.0), vec2f(w - t, t));
    lit += f32((m >> 1u) & 1u) * box(p, vec2f(w - t, t * 0.5), vec2f(w, 0.5));
    lit += f32((m >> 2u) & 1u) * box(p, vec2f(w - t, 0.5), vec2f(w, 1.0 - t * 0.5));
    lit += f32((m >> 3u) & 1u) * box(p, vec2f(t, 1.0 - t), vec2f(w - t, 1.0));
    lit += f32((m >> 4u) & 1u) * box(p, vec2f(0.0, 0.5), vec2f(t, 1.0 - t * 0.5));
    lit += f32((m >> 5u) & 1u) * box(p, vec2f(0.0, t * 0.5), vec2f(t, 0.5));
    lit += f32((m >> 6u) & 1u) * box(p, vec2f(t, 0.5 - t * 0.5), vec2f(w - t, 0.5 + t * 0.5));
    return min(lit, 1.0);
}

// A letter from a 3x5 pixel font. `bits` holds the 15 pixels, top row first.
// p runs 0..1 across and down the letter.
fn glyph(p: vec2f, bits: u32) -> f32 {
    if (p.x < 0.0 || p.x >= 1.0 || p.y < 0.0 || p.y >= 1.0) {
        return 0.0;
    }
    let cell = vec2u(u32(p.x * 3.0), u32(p.y * 5.0));
    return f32((bits >> ((4u - cell.y) * 3u + (2u - cell.x))) & 1u);
}

const MOOD_COLOURS = array<vec3f, 5>(
    vec3f(0.45, 0.45, 0.45),   // silent
    vec3f(0.30, 1.00, 0.45),   // groove
    vec3f(1.00, 0.85, 0.20),   // build
    vec3f(1.00, 0.25, 0.30),   // drop
    vec3f(0.30, 0.60, 1.00),   // break
);

// Returns the overlay colour and how opaque it is at this pixel.
fn hud(uv: vec2f) -> vec4f {
    // Work in "720p pixels" so the overlay is the same size on any screen.
    let scale = u.res_time.y / 720.0;
    let px = uv * u.res_time.xy / scale;
    let white = vec3f(0.95);
    var col = vec3f(0.0);
    var a = 0.0;

    // Dark panels behind the two groups, for legibility.
    let backing = max(
        box(px, vec2f(12.0, 12.0), vec2f(250.0, 122.0)),
        box(px, vec2f(12.0, 612.0), vec2f(520.0, 708.0)),
    );
    a = backing * 0.55;

    // Tempo, three digits, no leading zero.
    let bpm = u32(round(u.beat.w));
    var divisors = array<u32, 3>(100u, 10u, 1u);
    for (var i = 0u; i < 3u; i++) {
        if (bpm == 0u || (i == 0u && bpm < 100u)) {
            continue;
        }
        let p = (px - vec2f(24.0 + f32(i) * 34.0, 24.0)) / 44.0;
        if (digit(p, (bpm / divisors[i]) % 10u) > 0.5) {
            col = white;
            a = 1.0;
        }
    }

    // Four dots, one per beat of the bar; the current one flashes on the kick.
    let beat_in_bar = u32(u.misc.y) % 4u;
    for (var i = 0u; i < 4u; i++) {
        let d = length(px - vec2f(150.0 + f32(i) * 24.0, 46.0));
        let current = f32(i == beat_in_bar && bpm > 0u);
        let radius = 6.0 + 3.0 * current * u.beat.y;
        if (d < radius) {
            col = mix(vec3f(0.35), white, current);
            a = 1.0;
        }
    }

    // Position inside the beat, and under it how sure the tempo is.
    if (box(px, vec2f(24.0, 76.0), vec2f(24.0 + 210.0 * u.beat.z, 80.0)) > 0.5) {
        col = white;
        a = 1.0;
    }
    if (box(px, vec2f(24.0, 83.0), vec2f(24.0 + 210.0 * u.misc.z, 85.0)) > 0.5) {
        col = vec3f(0.5);
        a = 1.0;
    }

    // The mood, spelled out: SILENT, GROOVE, BUILD, DROP, BREAK.
    var words = array<u32, 30>(
        14478u, 29847u, 18727u, 31143u, 31597u, 29842u,
        14699u, 27565u, 11114u, 11114u, 23402u, 31143u,
        27566u, 23407u, 29847u, 18727u, 27502u, 0u,
        27502u, 27565u, 11114u, 27556u, 0u, 0u,
        27566u, 27565u, 31143u, 11245u, 23469u, 0u,
    );
    let mood = min(u32(u.state.x), 4u);
    for (var i = 0u; i < 6u; i++) {
        let p = (px - vec2f(24.0 + f32(i) * 16.0, 94.0)) / vec2f(12.0, 20.0);
        if (glyph(p, words[mood * 6u + i]) > 0.5) {
            col = MOOD_COLOURS[mood];
            a = 1.0;
        }
    }

    // LOCK, when the look is held and nothing changes on its own.
    var lock = array<u32, 4>(18727u, 11114u, 14627u, 23469u);
    for (var i = 0u; i < 4u; i++) {
        let p = (px - vec2f(150.0 + f32(i) * 16.0, 94.0)) / vec2f(12.0, 20.0);
        if (u.misc.w > 0.5 && glyph(p, lock[i]) > 0.5) {
            col = white;
            a = 1.0;
        }
    }

    // Spectrum: 64 bars, bass on the left.
    let base = 696.0;
    let sx = (px.x - 24.0) / 6.0;
    if (sx >= 0.0 && sx < 64.0 && fract(sx) < 0.8) {
        let i = u32(sx);
        let height = u.spectrum[i / 4u][i % 4u] * 72.0;
        if (px.y <= base && px.y > base - max(height, 1.0)) {
            col = pal(sx / 64.0);
            a = 1.0;
        }
    }

    // The four bands the visuals actually react to: sub, bass, mid, high.
    let bx = (px.x - 424.0) / 22.0;
    if (bx >= 0.0 && bx < 4.0 && fract(bx) < 0.75) {
        let height = u.bands[u32(bx)] * 72.0;
        if (px.y <= base && px.y > base - max(height, 1.0)) {
            col = white;
            a = 1.0;
        }
    }

    return vec4f(col, a);
}

// Where the water starts, from the top of the screen: the middle. Objects
// keep their full size; whatever is below this line goes under the water.
const HORIZON: f32 = 0.5;

// The finished picture (echoes and glow) at a point on the screen.
fn picture(uv: vec2f) -> vec3f {
    return textureSampleLevel(src, samp, uv, 0.0).rgb
        + textureSampleLevel(glow_tex, samp, uv, 0.0).rgb * u.post[1].z;
}

// The lower part of the screen as dark water that mirrors what is above
// it. Small fast ripples far off, bigger slower ones close by, stirred
// harder on the kick; the mirror image blurs and fades with nearness, and
// wave crests catch the light.
fn water(uv: vec2f, strength: f32) -> vec3f {
    let depth = clamp((uv.y - HORIZON) / (1.0 - HORIZON), 0.0, 1.0);   // 0 at the horizon, 1 at the bottom
    let t = u.flow.z;
    let stir = 1.0 + 1.5 * u.beat.y;
    // Waves run across the screen; their spacing widens toward the viewer.
    let rows = uv.y * mix(140.0, 40.0, depth);
    let wave = sin(rows - t * 2.5 + 3.0 * noise(vec2f(uv.x * 5.0, rows * 0.15 - t * 0.5)));
    let sway = (noise(vec2f(uv.x * 10.0 + t * 0.3, depth * 25.0 - t * 1.2)) - 0.5) * 0.035;
    let shift = vec2f(sway, wave * 0.006) * (0.15 + depth) * stir;
    let mirrored = vec2f(uv.x, HORIZON - (uv.y - HORIZON)) + shift;

    let blur = 0.002 + 0.025 * depth;
    var reflection = vec3f(0.0);
    for (var k = -2; k <= 2; k++) {
        reflection += picture(mirrored + vec2f(0.0, f32(k) * blur));
    }
    reflection /= 5.0;
    // Water reflects most at a grazing angle, near the horizon.
    let fresnel = mix(0.95, 0.15, smoothstep(0.0, 1.0, depth));
    let glint = pow(max(wave, 0.0), 12.0) * 0.6 * (1.0 - depth) * stir;
    return reflection * strength * fresnel * (1.0 + glint);
}

// One cell of an 8x8 Bayer matrix: thresholds spread so evenly that any
// grey becomes an even pattern of on and off dots.
fn bayer8(p: vec2u) -> f32 {
    var v = 0u;
    for (var i = 0u; i < 3u; i++) {
        let x = (p.x >> i) & 1u;
        let y = (p.y >> i) & 1u;
        v += (2u * (x ^ y) + y) << (2u * (2u - i));
    }
    return (f32(v) + 0.5) / 64.0;
}

// An old monitor: every dot fully on or off (one bit), round phosphor dots,
// scan lines, all in the palette's own colour.
fn crt(col: vec3f, pixel: vec2f) -> vec3f {
    let cell = max(2.0, floor(u.res_time.y / 300.0));
    let q = floor(pixel / cell);
    let luma = dot(col, vec3f(0.2126, 0.7152, 0.0722));
    let on = step(bayer8(vec2u(q) % 8u), luma * 1.15);
    let dot_shape = smoothstep(0.55, 0.25, length(fract(pixel / cell) - 0.5));
    var tint = max(max(pal(0.25), pal(0.5)), max(pal(0.75), pal(1.0)));
    tint /= max(max(tint.r, tint.g), max(tint.b, 1e-3));
    let scan = 0.8 + 0.2 * sin(pixel.y * 3.14159 / cell);
    return tint * on * dot_shape * scan * 1.1;
}

// Treatments: the whole picture as if seen through some medium. Chosen by
// post[2].y (1 VHS, 2 thermal, 3 film, 4 riso, 5 iridescent), faded in and
// out by overlay.z. (6 to 9 are drawn by post.wgsl.)

// VHS: each line slips sideways a little, more in a band that rolls down
// the screen and on the kick; red and blue come apart; the bottom lines
// tear where the tape heads switch.
fn vhs_picture(uv: vec2f) -> vec3f {
    let t = u.res_time.z;
    let line = floor(uv.y * 240.0);
    let roll = fract(t * 0.13);
    let band = exp(-abs(uv.y - roll) * 25.0);
    let head = smoothstep(0.96, 1.0, uv.y);
    let jitter = (noise(vec2f(line * 0.7, t * 30.0)) - 0.5)
        * (0.002 + 0.012 * band + 0.004 * u.beat.y + 0.05 * head);
    let p = vec2f(uv.x + jitter, uv.y);
    let split = 0.003 + 0.004 * u.beat.y;
    return vec3f(
        picture(p + vec2f(split, 0.0)).r,
        picture(p).g,
        picture(p - vec2f(split, 0.0)).b,
    );
}

// The tape's colour and lines, without tape noise: white flecks over the
// show looked like dirt on the screen, not like a tape.
fn vhs(col: vec3f, uv: vec2f) -> vec3f {
    let luma = dot(col, vec3f(0.299, 0.587, 0.114));
    var c = mix(vec3f(luma), col, 0.8) * vec3f(1.02, 0.98, 1.05);   // tape colour: a bit washed out
    c *= 0.9 + 0.1 * sin(uv.y * u.res_time.y * 1.5);                   // scan lines
    return c;
}

// Riso: a two-ink print, glowing on black: pink takes the warm part of the
// picture and blue the cool part, each as halftone dots on its own screen
// angle, the screens a little out of register as real prints are.
fn riso_screen(px: vec2f, angle: f32, amount: f32) -> f32 {
    let size = 7.0 * max(1.0, u.res_time.y / 720.0);
    let q = rot(angle) * px / size;
    let d = length(fract(q) - 0.5);
    return smoothstep(0.04, 0.0, d - sqrt(clamp(amount, 0.0, 1.0)) * 0.62);
}

fn riso(col: vec3f, px: vec2f) -> vec3f {
    let pink_amount = clamp(col.r * 0.95 + col.b * 0.25 - col.g * 0.15, 0.0, 1.0);
    let blue_amount = clamp(col.b * 0.85 + col.g * 0.55 - col.r * 0.2, 0.0, 1.0);
    let pink = riso_screen(px, 0.26, pink_amount);
    let blue = riso_screen(px + vec2f(2.0, -1.5), 1.31, blue_amount);
    return vec3f(0.02, 0.02, 0.03) + vec3f(1.0, 0.28, 0.65) * pink + vec3f(0.0, 0.47, 0.85) * blue;
}

// Iridescent: highlights take the moving rainbow of a soap film or a
// beetle's shell; shadows stay as they were.
fn iridescent(col: vec3f, uv: vec2f) -> vec3f {
    let luma = dot(col, vec3f(0.299, 0.587, 0.114));
    let phase = luma * 1.6 + (uv.x + uv.y) * 0.45 + u.res_time.z * 0.05 + u.flow.w * 0.02;
    let film = 0.55 + 0.45 * cos(TAU * (vec3f(0.0, 0.33, 0.67) + phase));
    let sheen = film * (luma * 1.25 + 0.04);
    return mix(col, sheen, smoothstep(0.05, 0.6, luma) * 0.85);
}

// Thermal: brightness becomes heat. Black, deep blue, violet, red, orange,
// yellow, white, whatever the palette was.
fn thermal(col: vec3f, uv: vec2f) -> vec3f {
    let heat = clamp(dot(col, vec3f(0.3, 0.55, 0.15)) * 1.2 + (hash21(uv * u.res_time.xy + u.res_time.z) - 0.5) * 0.03, 0.0, 1.0);
    var stops = array<vec3f, 7>(
        vec3f(0.0, 0.0, 0.02), vec3f(0.08, 0.02, 0.35), vec3f(0.45, 0.0, 0.55), vec3f(0.9, 0.05, 0.2),
        vec3f(1.0, 0.45, 0.0), vec3f(1.0, 0.85, 0.1), vec3f(1.0, 1.0, 0.9),
    );
    let x = heat * 6.0;
    let i = min(u32(x), 5u);
    return mix(stops[i], stops[i + 1u], fract(min(x, 5.999)));
}

// Film: a frame shakes a little in the gate, the light flickers, grain
// boils at 24 frames a second, and now and then dust and a scratch.
fn film_uv(uv: vec2f) -> vec2f {
    let frame = floor(u.res_time.z * 24.0);
    return uv + (vec2f(hash21(vec2f(frame, 1.0)), hash21(vec2f(frame, 2.0))) - 0.5) * vec2f(0.0015, 0.003);
}

fn film(col: vec3f, uv: vec2f, pixel: vec2f) -> vec3f {
    let frame = floor(u.res_time.z * 24.0);
    // Flicker: kept to 4%, well under the 10% change that counts as a flash.
    var c = col * (0.98 + 0.04 * hash21(vec2f(frame, 3.0)));
    let luma = dot(c, vec3f(0.299, 0.587, 0.114));
    let grain = hash21(floor(pixel / max(1.0, u.res_time.y / 720.0)) + vec2f(frame * 17.0, frame * 3.0)) - 0.5;
    c += grain * (0.12 * (1.0 - luma) + 0.04);                       // more grain in the shadows
    // A scratch down the frame for a few frames at a time.
    let scratch_at = hash21(vec2f(floor(frame / 6.0), 4.0));
    let scratch = step(0.6, hash21(vec2f(floor(frame / 6.0), 5.0)))
        * smoothstep(0.0015, 0.0, abs(uv.x - scratch_at - 0.003 * sin(uv.y * 20.0 + frame)));
    // Dust: a few dark specks, a new set every frame.
    let cell = floor(uv * vec2f(16.0, 9.0));
    let speck_at = (cell + vec2f(hash21(cell + frame), hash21(cell - frame))) / vec2f(16.0, 9.0);
    let speck = step(0.93, hash21(cell * 1.3 + frame * 0.71))
        * smoothstep(0.004, 0.001, length((uv - speck_at) * vec2f(u.res_time.x / u.res_time.y, 1.0)));
    c = c * (1.0 - 0.8 * speck) + vec3f(0.5 * scratch);
    return max(c, vec3f(0.0)) * vec3f(1.03, 1.0, 0.94);                 // a warm print
}

// The 3x5 font for ASCII 32..95 (space to underscore), one glyph per
// number, drawn by glyph(). Characters without a design are blank.
var<private> FONT: array<u32, 64> = array<u32, 64>(
    0u, 9346u, 0u, 0u, 0u, 0u, 0u, 9216u,
    5265u, 17556u, 0u, 1488u, 20u, 448u, 2u, 4772u,
    31599u, 11415u, 25255u, 25230u, 23497u, 31118u, 14831u, 29330u,
    31727u, 31694u, 1040u, 0u, 0u, 0u, 0u, 25218u,
    0u, 11245u, 27566u, 14627u, 27502u, 31143u, 31140u, 14699u,
    23533u, 29847u, 4714u, 23469u, 18727u, 24557u, 27501u, 11114u,
    27556u, 11123u, 27565u, 14478u, 29842u, 23407u, 23402u, 23549u,
    23213u, 23186u, 29351u, 0u, 0u, 0u, 0u, 7u,
);

// One line of text at `origin` (720p pixels), letters 12x20 every 16:
// `codes` are font codes (ASCII minus 32), `len` of them.
fn text_line(px: vec2f, origin: vec2f, codes: array<u32, 8>, len: u32) -> f32 {
    var c = codes;
    let i = floor((px.x - origin.x) / 16.0);
    if (i < 0.0 || i >= f32(len)) {
        return 0.0;
    }
    let p = (px - vec2f(origin.x + i * 16.0, origin.y)) / vec2f(12.0, 20.0);
    return glyph(p, FONT[min(c[u32(i)], 63u)]);
}

// The vibe on the overlay: a small map, dark to bright across, calm to
// intense upward, with a dot where the music is now, and its name.
fn vibe_overlay(px: vec2f) -> vec4f {
    let lo = vec2f(24.0, 140.0);
    let size = 72.0;
    var col = vec3f(0.0);
    var a = box(px, vec2f(12.0, 130.0), vec2f(250.0, 224.0)) * 0.55;
    let q = (px - lo) / size;                              // 0..1 inside the map
    if (q.x >= 0.0 && q.x <= 1.0 && q.y >= 0.0 && q.y <= 1.0) {
        // The frame and the cross through the middle.
        let edge = min(min(q.x, 1.0 - q.x), min(q.y, 1.0 - q.y)) * size;
        let cross = min(abs(q.x - 0.5), abs(q.y - 0.5)) * size;
        if (edge < 1.0 || cross < 0.75) {
            col = vec3f(0.35);
            a = 1.0;
        }
        // The dot: brightness across, intensity up; faint until sure.
        let at = vec2f(0.5 + 0.5 * u.vibe.x, 0.5 - 0.5 * u.vibe.y);
        if (length((q - at) * size) < 5.0) {
            col = mix(vec3f(0.9, 0.2, 0.25), vec3f(1.0, 0.95, 0.6), 0.5 + 0.5 * u.vibe.x);
            a = 0.3 + 0.7 * u.vibe.w;
        }
    }
    // The name, in two words.
    var dark = array<u32, 8>(36u, 33u, 50u, 43u, 0u, 0u, 0u, 0u);        // DARK
    var bright = array<u32, 8>(34u, 50u, 41u, 39u, 40u, 52u, 0u, 0u);    // BRIGHT
    var calm = array<u32, 8>(35u, 33u, 44u, 45u, 0u, 0u, 0u, 0u);        // CALM
    var intense = array<u32, 8>(41u, 46u, 52u, 37u, 46u, 51u, 37u, 0u);  // INTENSE
    var lit = 0.0;
    if (u.vibe.w < 0.3) {
        return vec4f(col, a);   // not enough music heard yet to name it
    }
    if (u.vibe.x < 0.0) {
        lit = max(lit, text_line(px, vec2f(112.0, 150.0), dark, 4u));
    } else {
        lit = max(lit, text_line(px, vec2f(112.0, 150.0), bright, 6u));
    }
    if (u.vibe.y < 0.0) {
        lit = max(lit, text_line(px, vec2f(112.0, 182.0), calm, 4u));
    } else {
        lit = max(lit, text_line(px, vec2f(112.0, 182.0), intense, 7u));
    }
    if (lit > 0.5) {
        col = vec3f(0.95);
        a = 0.3 + 0.7 * u.vibe.w;
    }
    return vec4f(col, a);
}

// A line of text (the label), centred near the top of the screen, for
// messages such as which input is being listened to.
fn label(uv: vec2f) -> vec4f {
    let opacity = u.overlay.x;
    let count = u32(u.overlay.y);
    if (opacity <= 0.0 || count == 0u) {
        return vec4f(0.0);
    }
    let scale = u.res_time.y / 720.0;
    let px = uv * u.res_time.xy / scale;
    let advance = 16.0;
    let width = f32(count) * advance;
    let left = (u.res_time.x / scale - width) / 2.0;
    let top = 30.0;
    var a = box(px, vec2f(left - 14.0, top - 12.0), vec2f(left + width + 10.0, top + 32.0)) * 0.7;
    var col = vec3f(0.0);
    let i = floor((px.x - left) / advance);
    if (i >= 0.0 && i < f32(count)) {
        let n = u32(i);
        let code = min(u32(u.text[n / 4u][n % 4u]), 63u);
        let p = (px - vec2f(left + i * advance, top)) / vec2f(12.0, 20.0);
        if (glyph(p, FONT[code]) > 0.5) {
            col = vec3f(1.0);
            a = 1.0;
        }
    }
    return vec4f(col, a * opacity);
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4f {
    let treatment = u32(round(u.post[2].y));
    let treat_amount = clamp(u.overlay.z, 0.0, 1.0);
    var col = picture(in.uv);
    if (treatment == 1u && treat_amount > 0.0) {
        col = mix(col, vhs_picture(in.uv), treat_amount);
    } else if (treatment == 3u && treat_amount > 0.0) {
        col = mix(col, picture(film_uv(in.uv)), treat_amount);
    }
    let floor_strength = u.post[2].x;
    if (floor_strength > 0.0) {
        // No hard edge: from just above the horizon the picture melts into
        // its reflection, like a shoreline seen from a little height.
        let below = smoothstep(HORIZON - 0.035, HORIZON + 0.09, in.uv.y);
        // While the floor fades in or out, everything it does (the water and
        // the darkening under it) scales with how far it has come.
        let present = clamp(floor_strength / 0.55, 0.0, 1.0);
        if (below > 0.0) {
            col = mix(col, col * 0.12 + water(in.uv, floor_strength / max(present, 1e-3)), below * present);
        }
    }
    // Strobe: already limited to a safe rate and brightness by the app.
    col += vec3f(u.state.w);
    // A little extra contrast: echoes and glow fill the dark areas with haze,
    // and techno wants its blacks black. Values under 1 get darker, over 1 brighter.
    col = pow(col, vec3f(1.15));
    col = tonemap(col);
    let c = in.uv - 0.5;
    col *= 1.0 - 0.6 * dot(c, c);   // vignette

    // (A build-up used to close the picture in to a shrinking black circle
    // here. The camera pushing in alone reads better: see objects.rs.)

    if (treat_amount > 0.0) {
        var treated = col;
        if (treatment == 1u) { treated = vhs(col, in.uv); }
        if (treatment == 2u) { treated = thermal(col, in.uv); }
        if (treatment == 3u) { treated = film(col, in.uv, in.pos.xy); }
        if (treatment == 4u) { treated = riso(col, in.pos.xy); }
        if (treatment == 5u) { treated = iridescent(col, in.uv); }
        col = mix(col, treated, treat_amount);
    }
    let dither = u.post[2].z;
    if (dither > 0.0) {
        col = mix(col, crt(col, in.pos.xy), min(dither, 1.0));
    }
    if (u.misc.x > 0.0) {
        let overlay = hud(in.uv);
        col = mix(col, overlay.rgb, overlay.a * u.misc.x);   // fades with the mouse
        let scale = u.res_time.y / 720.0;
        let vibe = vibe_overlay(in.uv * u.res_time.xy / scale);
        col = mix(col, vibe.rgb, vibe.a * u.misc.x);
    }
    let text = label(in.uv);
    col = mix(col, text.rgb, text.a);
    return vec4f(col, 1.0);
}
