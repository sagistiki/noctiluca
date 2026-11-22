// The hero's sea, drawn live with WebGL: the same sea as the app's "sea"
// background (shaders/bg_sea.wgsl), under its own Noctiluca palette. Five
// soft bands of water lie across the screen, from the sand-light of the
// shore in the bottom-left corner to the night; each band's edge is a wave
// carrying a thin line of light, and plankton twinkles in the dark water.
// A pulse at 124 BPM stands in for the music, and the water lights up
// where the pointer passes. Without WebGL the CSS gradient stays.
// Keep the two alike (docs/superpowers/specs/2026-11-22-sea-bands-design.md).
(() => {
  const canvas = document.querySelector('.sea');
  if (!canvas) return;
  const gl = canvas.getContext('webgl', { antialias: false, alpha: false, powerPreference: 'low-power' });
  if (!gl) return;

  const vert = `attribute vec2 p; void main() { gl_Position = vec4(p, 0.0, 1.0); }`;
  const frag = `
precision highp float;
uniform vec2 res;
uniform float time;
uniform vec2 mouse;
uniform float beat;

// The app's own noise (shaders/common.wgsl), so the two seas agree.
mat2 rot(float a) { float c = cos(a), s = sin(a); return mat2(c, s, -s, c); }
float hash21(vec2 p) {
  vec2 w = p - 1024.0 * floor(p / 1024.0);
  vec2 q = fract(w * vec2(123.34, 456.21));
  q += dot(q, q + 45.32);
  return fract(q.x * q.y);
}
float noise(vec2 p) {
  vec2 i = floor(p), f = fract(p), s = f * f * (3.0 - 2.0 * f);
  return mix(mix(hash21(i), hash21(i + vec2(1.0, 0.0)), s.x), mix(hash21(i + vec2(0.0, 1.0)), hash21(i + vec2(1.0, 1.0)), s.x), s.y);
}
float fbm3(vec2 p) {
  float sum = 0.0, amp = 0.5;
  vec2 q = p;
  for (int i = 0; i < 3; i++) { sum += amp * noise(q); q = rot(0.5) * q * 2.03; amp *= 0.5; }
  return sum / 0.875 * 0.96875;
}
// The Noctiluca palette (src/director.rs, PALETTES[12]).
vec3 pal(float t) {
  return clamp(vec3(0.575, 0.5, 0.375) + vec3(0.7, 0.475, 0.65) * cos(6.2831853 * (t + vec3(0.4, 0.49, 0.64))), 0.0, 1.0);
}
// Light end (0) to dark end (1); this palette is light at t = 0.5.
vec3 seaRamp(float k) {
  float k1 = clamp(k, 0.0, 1.0);
  return pal(0.5 * (1.0 - k1)) * (1.0 - 0.85 * smoothstep(0.8, 1.0, k1));
}
float plankton(vec2 p, float scale, float density, float t) {
  vec2 g = p * scale, id = floor(g), f = fract(g) - 0.5;
  float h = hash21(id);
  if (h > density) return 0.0;
  vec2 off = (vec2(hash21(id + 3.1), hash21(id + 7.7)) - 0.5) * 0.6;
  float r = length(f - off);
  float tw = 0.35 + 0.65 * pow(0.5 + 0.5 * sin(t * (0.7 + 2.5 * hash21(id + 1.3)) + h * 40.0), 3.0);
  return (smoothstep(0.11, 0.0, r) + 0.35 * exp(-r * r * 60.0)) * tw;
}

void main() {
  // The app's sea turned upside down: here the shore lies in the bottom-left
  // corner, below the buttons, so the logo in the top-left corner and the
  // title stay on blue and dark.
  vec2 p = (gl_FragCoord.xy * 2.0 - res) / res.y * vec2(1.0, -1.0);
  vec2 q = p * 0.5;
  vec2 m = (mouse * 2.0 - res) / res.y * vec2(0.5, -0.5);
  float t = time * 0.18;
  float stir = exp(-dot(q - m, q - m) * 12.0);
  vec2 dir = normalize(vec2(1.0, -0.55));
  float d = dot(q, dir), s = dot(q, vec2(-dir.y, dir.x));
  vec3 col = seaRamp(0.05 + (d + 0.9) * 0.08);
  for (int i = 0; i < 5; i++) {
    float fi = float(i);
    float base = -0.75 + fi * 0.26;
    float edge = base + 0.07 * sin(s * (1.6 + fi * 0.35) + t * (1.0 + fi * 0.3) + fi * 2.1)
               + 0.05 * (fbm3(vec2(s * 0.9 + fi * 3.0, t * 0.5)) - 0.5);
    float x = d - edge;
    float inside = smoothstep(-0.01, 0.01, x);
    col = mix(col, seaRamp(0.28 + fi * 0.17 + x * 0.35), inside * 0.92);
    float glow = exp(-abs(x) * 140.0) + 0.25 * exp(-abs(x) * 18.0);
    col += seaRamp(0.32) * glow * (0.55 - fi * 0.07) * (0.8 + 0.5 * beat + 0.6 * stir);
  }
  vec2 drift = q + vec2(time * 0.008, time * 0.012);
  float sparks = plankton(drift, 34.0, 0.12, time) * 0.7 + plankton(drift + 4.0, 12.0, 0.05, time) * 1.1;
  col += mix(seaRamp(0.2), vec3(1.0), 0.5) * sparks * smoothstep(-0.2, 0.4, d) * (0.94 + 1.2 * stir);
  col *= 1.0 - 0.15 * dot(q, q);
  col += (hash21(p * 431.0 + fract(time) * 61.0) - 0.5) / 255.0;
  gl_FragColor = vec4(max(col, 0.0), 1.0);
}`;

  const shader = (type, src) => {
    const s = gl.createShader(type);
    gl.shaderSource(s, src);
    gl.compileShader(s);
    if (!gl.getShaderParameter(s, gl.COMPILE_STATUS)) { console.warn(gl.getShaderInfoLog(s)); return null; }
    return s;
  };
  const vs = shader(gl.VERTEX_SHADER, vert), fs = shader(gl.FRAGMENT_SHADER, frag);
  if (!vs || !fs) return;
  const prog = gl.createProgram();
  gl.attachShader(prog, vs);
  gl.attachShader(prog, fs);
  gl.linkProgram(prog);
  if (!gl.getProgramParameter(prog, gl.LINK_STATUS)) return;
  gl.useProgram(prog);
  const buf = gl.createBuffer();
  gl.bindBuffer(gl.ARRAY_BUFFER, buf);
  gl.bufferData(gl.ARRAY_BUFFER, new Float32Array([-1, -1, 3, -1, -1, 3]), gl.STATIC_DRAW);
  const loc = gl.getAttribLocation(prog, 'p');
  gl.enableVertexAttribArray(loc);
  gl.vertexAttribPointer(loc, 2, gl.FLOAT, false, 0, 0);
  const u = (n) => gl.getUniformLocation(prog, n);
  const uRes = u('res'), uTime = u('time'), uMouse = u('mouse'), uBeat = u('beat');

  // Rendered a little under full resolution: it is soft light, and it
  // keeps laptops cool.
  const scale = Math.min(window.devicePixelRatio || 1, 1.5) * 0.75;
  let w = 0, h = 0;
  const resize = () => {
    w = Math.max(1, Math.floor(canvas.clientWidth * scale));
    h = Math.max(1, Math.floor(canvas.clientHeight * scale));
    canvas.width = w;
    canvas.height = h;
    gl.viewport(0, 0, w, h);
  };
  resize();
  window.addEventListener('resize', resize);

  let mx = w * 0.5, my = h * 0.25, tx = mx, ty = my;
  window.addEventListener('pointermove', (e) => {
    const r = canvas.getBoundingClientRect();
    tx = (e.clientX - r.left) * scale;
    ty = (r.height - (e.clientY - r.top)) * scale;
  }, { passive: true });

  const still = window.matchMedia('(prefers-reduced-motion: reduce)').matches;
  let visible = true;
  new IntersectionObserver(([e]) => { visible = e.isIntersecting; if (visible) requestAnimationFrame(draw); }).observe(canvas);

  const start = performance.now();
  function draw(now) {
    const t = still ? 12.0 : (now - start) / 1000;
    mx += (tx - mx) * 0.06;
    my += (ty - my) * 0.06;
    const beatPhase = (t * 124 / 60) % 1;
    gl.uniform2f(uRes, w, h);
    gl.uniform1f(uTime, t);
    gl.uniform2f(uMouse, mx, my);
    gl.uniform1f(uBeat, Math.exp(-beatPhase * 5.0));
    gl.drawArrays(gl.TRIANGLES, 0, 3);
    if (visible && !still) requestAnimationFrame(draw);
  }
  requestAnimationFrame(draw);
})();
