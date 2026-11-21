// Moving heads: six beams from lights above the stage, sweeping through
// haze to the beat. Every 16 beats they change their routine: swinging
// together, fanning out, or crossing.

fn atmosphere(uv: vec2f) -> vec4f {
    let p = centred(uv);
    let aspect = u.res_time.x / u.res_time.y;
    let beats = u.flow.w;
    let routine = floor(hash21(vec2f(floor(beats / 16.0), 2.7)) * 3.0);
    var col = vec3f(0.0);
    for (var i = 0; i < 6; i++) {
        let fi = f32(i);
        let k = fi / 5.0 - 0.5;   // -0.5 .. 0.5 across the rig
        let source = vec2f(k * aspect * 1.6, 1.08);
        let swing = sin(beats * 0.7854 + fi * 0.5);   // a full swing every 8 beats
        var angle = 0.45 * swing;                                  // together
        if (routine == 1.0) { angle = k * 1.3 + 0.2 * swing; }     // fanned out
        if (routine == 2.0) { angle = -k * 1.4 * swing; }          // crossing
        let dir = vec2f(sin(angle), -cos(angle));
        let rel = p - source;
        let along = dot(rel, dir);
        let across = abs(rel.x * dir.y - rel.y * dir.x);
        let width = 0.015 + max(along, 0.0) * 0.08;
        let beam = exp(-pow(across / width, 2.0)) * smoothstep(0.0, 0.15, along) * exp(-max(along, 0.0) * 0.35);
        let haze = 0.55 + 0.45 * noise(vec2f(along * 4.0 - u.flow.z * 0.6, fi * 3.0 + across * 10.0));
        col += pal(0.6 + fi * 0.08) * beam * haze * (1.1 + 0.9 * u.beat.y);
        col += pal(0.9) * 0.02 / (dot(rel, rel) + 0.002);   // the lamp itself
    }
    return vec4f(col * 0.9, 0.0);
}
