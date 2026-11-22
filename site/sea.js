// The hero's night sea, drawn live with WebGL: the same idea as the app's
// "sea" background. Waves roll in from the horizon, their crests break into
// foam where plankton lights up blue, with sparks as the water moves, and
// the water glows brighter where the pointer passes. A slow pulse at 124
// BPM stands in for the music. Without WebGL the CSS gradient stays.
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

float hash(vec2 p) { p = fract(p * vec2(123.34, 456.21)); p += dot(p, p + 45.32); return fract(p.x * p.y); }
float noise(vec2 p) {
  vec2 i = floor(p), f = fract(p), s = f * f * (3.0 - 2.0 * f);
  return mix(mix(hash(i), hash(i + vec2(1, 0)), s.x), mix(hash(i + vec2(0, 1)), hash(i + vec2(1, 1)), s.x), s.y);
}
float fbm(vec2 p) { float v = 0.0, a = 0.5; for (int i = 0; i < 3; i++) { v += a * noise(p); p *= 2.03; a *= 0.5; } return v; }
// The palette: night, deep sea, glowing blue, foam, sand.
vec3 pal(float t) { return clamp(vec3(0.32, 0.49, 0.75) + vec3(0.42, 0.42, 0.36) * cos(6.2831853 * (t + vec3(0.5))), 0.0, 1.0); }

void main() {
  vec2 p = (gl_FragCoord.xy * 2.0 - res) / res.y;
  float horizon = -0.32;
  vec3 col = mix(vec3(0.004, 0.024, 0.06), vec3(0.0, 0.01, 0.03), clamp(p.y, 0.0, 1.0));
  // A faint sky of stars above the horizon.
  float star = step(0.9965, hash(floor(gl_FragCoord.xy / 2.0))) * smoothstep(horizon, horizon + 0.6, p.y);
  col += vec3(0.7, 0.8, 1.0) * star * (0.5 + 0.5 * sin(time * 2.0 + hash(floor(gl_FragCoord.xy)) * 40.0));
  float below = horizon - p.y;
  if (below > 0.0) {
    float z = 0.45 / (below + 0.03);
    float x = p.x * z;
    float t = time * 0.55;
    float h = sin(z * 9.0 - t * 2.2 + 2.2 * sin(x * 0.45 + z * 0.2) + 0.8 * fbm(vec2(x * 0.3, z * 0.5)))
            + 0.35 * sin(z * 15.0 - t * 3.1 + x * 1.3);
    float crest = exp(-pow((h - 1.05) * 5.0, 2.0));
    float breakup = smoothstep(0.35, 0.75, noise(vec2(x * 2.2, z * 3.0) + vec2(t * 0.4, 0.0)));
    // The pointer stirs the water: foam there glows brighter.
    vec2 m = (mouse * 2.0 - res) / res.y;
    float stir = exp(-dot(p - m, p - m) * 3.5);
    float foam = crest * breakup * (0.8 + 0.5 * beat + 1.4 * stir);
    float spark = step(0.992, hash(floor(gl_FragCoord.xy / 2.5) + floor(t * 5.0) * 3.7)) * smoothstep(0.15, 0.6, crest);
    float fade = exp(-z * 0.12) * smoothstep(0.0, 0.15, below);
    vec3 water = pal(0.25) * 0.05 * (0.6 + 0.4 * h);
    col += (water + pal(0.45 + 0.08 * sin(x * 0.2)) * foam * 1.1 + pal(0.6) * spark * 2.2) * fade;
  }
  // A soft glow where sea meets sky.
  col += vec3(0.18, 0.42, 0.95) * exp(-abs(p.y - horizon) * 9.0) * 0.18;
  gl_FragColor = vec4(pow(col, vec3(0.95)), 1.0);
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
