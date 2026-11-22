"""Draws Noctiluca's own hamsa, in the classic style, and traces it into
outlines for the builder (blender/build_models.py, build_hamsa).

Run with the system Python (numpy, scipy, Pillow, contourpy):

    python3 blender/design_hamsa.py blender/hamsa_design.json [preview.png]

An original drawing, made here from shapes (no picture is traced), in units
where the hand is about 2 tall, x to the right and y up, the middle at x 0:

- **The hand** (signed distances, negative inside): three fingers side by
  side and joined, the middle one tallest; a round palm; two thumbs grown out
  of the palm's sides, wide where they leave it and narrowing to a round tip
  that turns up, so only the tip stands out.
- **The frame:** a band along the edge and a second line inside it, which
  runs round the fingers together; one line parts each pair of fingers.
- **The ornament** (drawn as lines with a pen): spirals that end in a dot,
  hearts of two spirals, drops, a scalloped cuff under the fingers, and the
  eye (an almond, a glowing iris ring and pupil) in the middle of the palm.

Everything is symmetric: the right half is drawn and mirrored. The drawing is
traced at the middle grey with sub-pixel precision after a touch of
smoothing, so the lines come out as curves. Three sets come out: "outline"
(the hand), "lines" (frame and ornament) and "eye" (which glows).
"""

import json
import math
import sys

import contourpy
import numpy as np
from PIL import Image, ImageDraw
from scipy import ndimage as nd

N = 1600  # pixels across the drawing
HALF = 1.1  # the drawing spans -HALF..HALF both ways
W = 0.0115  # half the width of a line
SUPER = 3  # the pen draws this much finer, then it is averaged down

ys, xs = np.mgrid[0:N, 0:N]
X = (xs + 0.5) / N * 2 * HALF - HALF
Y = HALF - (ys + 0.5) / N * 2 * HALF
AX = np.abs(X)


# ---------------------------------------------------------------- the hand


def smin(a, b, k):
    """A smooth union: the shapes melt together over about `k`."""
    h = np.clip(0.5 + 0.5 * (b - a) / k, 0.0, 1.0)
    return b + (a - b) * h - k * h * (1 - h)


def circle(x, y, cx, cy, r):
    return np.hypot(x - cx, y - cy) - r


def ellipse(x, y, cx, cy, rx, ry):
    k = np.hypot((x - cx) / rx, (y - cy) / ry)
    return (k - 1.0) * min(rx, ry)


def capsule(x, y, ax, ay, bx, by, r):
    px, py, vx, vy = x - ax, y - ay, bx - ax, by - ay
    h = np.clip((px * vx + py * vy) / (vx * vx + vy * vy), 0.0, 1.0)
    return np.hypot(px - vx * h, py - vy * h) - r


def cone(x, y, a, b, ra, rb, steps=28):
    """A round cone: from a circle `ra` at `a` narrowing to `rb` at `b`."""
    d = np.full(x.shape, 9.0)
    for k in range(steps + 1):
        t = k / steps
        d = np.minimum(d, circle(x, y, a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t, ra + (rb - ra) * t))
    return d


def hand():
    """The fingers overlap a little, so only a small notch parts their tips."""
    r = 0.168
    middle = capsule(X, Y, 0.0, -0.3, 0.0, 0.97 - r, r)
    side = capsule(AX, Y, 0.325, -0.3, 0.325, 0.8 - r, r)
    fingers = smin(middle, side, 0.035)
    palm = ellipse(X, Y, 0.0, -0.37, 0.6, 0.63)
    body = smin(fingers, palm, 0.08)
    # The thumb: wide where it leaves the palm, out and a little up to a
    # round tip that curls upward.
    tip = (0.7, -0.06, 0.13)  # the thumb tip's middle and size
    thumb = cone(AX, Y, (0.4, -0.33), (0.66, -0.09), 0.2, 0.12)
    thumb = smin(thumb, circle(AX, Y, *tip), 0.06)
    body = smin(body, thumb, 0.12)
    # Over the thumb, no deep notch: its top rises from the tip to the
    # finger in a gentle curve, sagging a little.
    span = tip[0] - 0.493
    t = np.clip((tip[0] - AX) / span, 0.0, 1.0)
    ridge = tip[1] + tip[2] + 0.05 * t - 0.03 * np.sin(np.pi * t)
    fill = np.maximum.reduce([Y - ridge, AX - tip[0], 0.46 - AX, -0.25 - Y])
    return smin(body, fill, 0.06)


# ----------------------------------------------------------------- the pen


class Pen:
    """Lines and dots drawn finely, each on the right and mirrored left."""

    def __init__(self):
        size = N * SUPER
        self.img = Image.new("L", (size, size), 0)
        self.draw = ImageDraw.Draw(self.img)
        self.k = size / (2 * HALF)

    def _px(self, x, y):
        return ((x + HALF) * self.k, (HALF - y) * self.k)

    def line(self, pts, width=1.0, mirror=True):
        for m in ((1, -1) if mirror else (1,)):
            q = [self._px(m * x, y) for x, y in pts]
            w = max(1, round(2 * W * width * self.k))
            self.draw.line(q, fill=255, width=w, joint="curve")
            for end in (q[0], q[-1]):
                self.dot_px(end, w / 2)

    def dot(self, x, y, r, mirror=True):
        for m in ((1, -1) if mirror and x != 0 else (1,)):
            self.dot_px(self._px(m * x, y), r * self.k)

    def dot_px(self, c, r):
        self.draw.ellipse([c[0] - r, c[1] - r, c[0] + r, c[1] + r], fill=255)

    def coverage(self):
        a = np.asarray(self.img, dtype=np.float32) / 255.0
        return a.reshape(N, SUPER, N, SUPER).mean(axis=(1, 3))


def spiral(cx, cy, r_out, turns, start, cw):
    """An Archimedean spiral from `r_out` at angle `start` winding in by
    `turns` (clockwise when `cw`)."""
    pts = []
    n = int(60 * turns)
    for k in range(n + 1):
        t = k / n
        a = start + (-1 if cw else 1) * t * turns * 2 * math.pi
        r = r_out * (1 - 0.8 * t)
        pts.append((cx + r * math.cos(a), cy + r * math.sin(a)))
    return pts


def bezier(p0, p1, p2, p3, n=40):
    out = []
    for k in range(n + 1):
        t = k / n
        u = 1 - t
        out.append(tuple(u ** 3 * a + 3 * u * u * t * b + 3 * u * t * t * c + t ** 3 * d for a, b, c, d in zip(p0, p1, p2, p3)))
    return out


def scroll(pen, c, r, turns, start, cw, tail=None, width=1.0, dot=1.7, mirror=True):
    """A spiral ending in a dot inside. `tail`: (point, direction) the outer
    end flows on to, in a smooth curve leaving along the spiral's own line
    and arriving at the point going in `direction`."""
    pts = spiral(c[0], c[1], r, turns, start, cw)
    if tail is not None:
        (tx, ty), (dx, dy) = tail
        p0, p1 = pts[0], pts[1]
        ox, oy = p0[0] - p1[0], p0[1] - p1[1]
        n = math.hypot(ox, oy)
        reach = 0.45 * math.hypot(tx - p0[0], ty - p0[1])
        curve = bezier((tx, ty), (tx - dx * reach, ty - dy * reach), (p0[0] + ox / n * reach, p0[1] + oy / n * reach), p0)
        pts = curve[:-1] + pts
    pen.line(pts, width, mirror)
    end = pts[-1]
    pen.dot(end[0], end[1], W * dot, mirror)


def heart(pen, cx, cy, r, point, width=1.0, centre=0.0):
    """A heart of two spirals: from the point, up each side, over the top and
    winding in. `point`: where the two lines meet below; `centre`: the
    heart's middle (0, or a side finger's, drawn there and mirrored)."""
    for s in (1, -1):
        lobe = (centre + s * (cx - centre), cy)
        down = (-s * 0.35, -1.0)
        n = math.hypot(*down)
        pts = spiral(lobe[0], lobe[1], r, 1.3, 0.0 if s > 0 else math.pi, cw=s < 0)
        (tx, ty), (dx, dy) = point, (down[0] / n, down[1] / n)
        p0, p1 = pts[0], pts[1]
        ox, oy = p0[0] - p1[0], p0[1] - p1[1]
        m = math.hypot(ox, oy)
        reach = 0.45 * math.hypot(tx - p0[0], ty - p0[1])
        curve = bezier((tx, ty), (tx - dx * reach, ty - dy * reach), (p0[0] + ox / m * reach, p0[1] + oy / m * reach), p0)
        line = curve[:-1] + pts
        pen.line(line, width, mirror=centre != 0.0)
        pen.dot(line[-1][0], line[-1][1], W * 1.7, mirror=centre != 0.0)


def arc(cx, cy, r, a0, a1, n=40):
    return [(cx + r * math.cos(a0 + (a1 - a0) * k / n), cy + r * math.sin(a0 + (a1 - a0) * k / n)) for k in range(n + 1)]


def drop(pen, cx, cy, w, h, up=False, width=1.0, mirror=True):
    """A teardrop outline: round at one end, a point at the other."""
    s = 1 if up else -1
    tip = (cx, cy + s * h)
    right = bezier((cx, cy - s * w * 0.9), (cx + w * 1.35, cy - s * w * 0.9), (cx + w * 0.7, cy + s * h * 0.55), tip)
    left = [(2 * cx - x, y) for x, y in right]
    pen.line(right[::-1] + left[1:], width, mirror)


def ornament(pen):
    down = (0.0, -1.0)
    # Each finger: a heart of spirals, a drop under it, a dot.
    heart(pen, 0.048, 0.6, 0.044, (0.0, 0.47))
    pen.dot(0.0, 0.78, 0.024, mirror=False)
    drop(pen, 0.0, 0.31, 0.045, 0.1, mirror=False)
    pen.dot(0.0, 0.1, 0.026, mirror=False)
    for k, side in enumerate((0.325,)):
        heart(pen, side + 0.046, 0.45, 0.042, (side, 0.33), centre=side)
        pen.dot(side, 0.6, 0.022)
        drop(pen, side, 0.19, 0.04, 0.09)
        pen.dot(side, 0.03, 0.024)

    # The cuff under the fingers: an arc hung with scallops.
    cuff_c, cuff_r = (0.0, -1.47), 1.44
    pen.line(arc(cuff_c[0], cuff_c[1], cuff_r, math.radians(70), math.radians(110)), mirror=False)
    for k in range(-3, 4):
        a = math.pi / 2 + k * 0.09
        cx, cy = cuff_r * math.cos(a), cuff_c[1] + cuff_r * math.sin(a)
        pen.line(arc(cx, cy, 0.064, a + math.pi / 2 + 0.05, a + 3 * math.pi / 2 - 0.05), width=0.9, mirror=False)

    # The thumbs: a big spiral in each tip.
    scroll(pen, (0.7, -0.06), 0.06, 1.7, math.pi * 1.15, cw=True, dot=2.0)
    # By each thumb's root: a spiral, and a dot under it.
    scroll(pen, (0.44, -0.25), 0.055, 1.6, math.pi * 0.25, cw=False)
    pen.dot(0.45, -0.39, 0.02)

    # A heart of spirals over the eye.
    heart(pen, 0.12, -0.16, 0.055, (0.0, -0.27))

    # The eye: an almond, where two circles cross.
    ey, R, d = -0.5, 0.541, 0.341
    half = math.degrees(math.asin(0.42 / R))
    pen.line(arc(0.0, ey - d, R, math.radians(90 - half), math.radians(90 + half), 60), 1.1, mirror=False)
    pen.line(arc(0.0, ey + d, R, math.radians(-90 - half), math.radians(-90 + half), 60), 1.1, mirror=False)

    # Dots round the almond, above and below.
    for k in range(-3, 4):
        t = k * 0.13
        pen.dot(0.53 * math.sin(t * 2.4), ey + 0.27 * math.cos(t * 2.4) - 0.0, 0.016, mirror=False)
        pen.dot(0.53 * math.sin(t * 2.4), ey - 0.27 * math.cos(t * 2.4), 0.016, mirror=False)
    # Under the eye: a big heart of spirals pointing down, a dot at its point.
    heart(pen, 0.11, -0.76, 0.075, (0.0, -0.93))
    # Low on each side: a small spiral curling the other way, and a dot.
    scroll(pen, (0.39, -0.7), 0.05, 1.5, math.pi * 0.75, cw=True)
    pen.dot(0.3, -0.86, 0.02)


def eye_glow():
    ey = -0.5
    ring = np.abs(circle(X, Y, 0.0, ey, 0.128)) - 0.03
    pupil = circle(X, Y, 0.0, ey, 0.045)
    return np.minimum(ring, pupil)


# ------------------------------------------------------------ tracing


def simplify(points, tol):
    """Ramer-Douglas-Peucker on a closed loop (first point not repeated)."""
    def rdp(pts):
        if len(pts) < 3:
            return pts
        a, b = pts[0], pts[-1]
        ab = b - a
        n = np.hypot(*ab)
        d = np.hypot(*(pts - a).T) if n == 0 else np.abs(ab[0] * (pts[:, 1] - a[1]) - ab[1] * (pts[:, 0] - a[0])) / n
        i = int(np.argmax(d))
        if d[i] > tol:
            return np.vstack([rdp(pts[: i + 1])[:-1], rdp(pts[i:])])
        return np.vstack([a, b])

    j = int(np.argmax(np.hypot(*(points - points[0]).T)))
    first = rdp(points[: j + 1])
    second = rdp(np.vstack([points[j:], points[:1]]))
    return np.vstack([first[:-1], second[:-1]])


def loops(inside, tol, min_len=8):
    """Closed outlines of where `inside` (a 0..1 field) crosses one half."""
    out = []
    for line in contourpy.contour_generator(z=nd.gaussian_filter(inside, 1.0)).lines(0.5):
        if len(line) < min_len:
            continue
        pts = line[:-1] if np.allclose(line[0], line[-1]) else line
        out.append(simplify(pts, tol))
    return out


def main(dst, preview=None):
    px = 2 * HALF / N
    cover = lambda d: np.clip(0.5 - d / px, 0.0, 1.0)
    body = hand()
    # The true distance in from the edge (the shapes' own distances are only
    # right near the edge: where fingers overlap they think the inside thin).
    depth = nd.distance_transform_edt(body < 0) * px
    depth = nd.gaussian_filter(depth, 1.5)
    inner = np.where(body < 0, 0.062 - depth, 1.0)
    band = np.where(body < 0, depth - 2.2 * W, body)
    frame = np.minimum(band, np.abs(inner) - W)
    # One line parts each pair of fingers, from the notch down to the cuff.
    frame = np.minimum(frame, np.maximum(capsule(AX, Y, 0.163, -0.03, 0.163, 0.75, W), inner))
    pen = Pen()
    ornament(pen)
    drawn = pen.coverage() * (inner < 0)  # the ornament stays inside the frame
    sil = cover(body)
    glow = cover(np.maximum(eye_glow(), body))
    lines = np.maximum(cover(frame), drawn) * sil
    lines = np.where(glow > 0.5, 0.0, lines)

    rows = np.nonzero(sil.max(axis=1) > 0.5)[0]
    top, bottom = rows.min(), rows.max()
    scale = 2.0 / (bottom - top)
    middle = (top + bottom) / 2.0
    centre = (N - 1) / 2.0

    def units(loop):
        return [[round((x - centre) * scale, 5), round((middle - y) * scale, 5)] for x, y in loop]

    data = {
        "source": "Noctiluca's own hamsa, drawn by blender/design_hamsa.py",
        "outline": units(max(loops(sil, 0.3), key=len)),
        "lines": [units(l) for l in loops(lines, 0.6)],
        "eye": [units(l) for l in loops(glow, 0.3)],
    }
    with open(dst, "w") as f:
        json.dump(data, f, separators=(",", ":"))
    count = sum(len(l) for l in data["lines"])
    print(f"outline {len(data['outline'])} points, {len(data['lines'])} line loops ({count} points), {len(data['eye'])} eye loops")
    if preview:
        img = np.full((N, N, 3), 255.0)
        img[sil > 0.5] = (236, 230, 214)
        img = img * (1 - lines[..., None]) + np.array([20, 20, 24]) * lines[..., None]
        img = img * (1 - glow[..., None]) + np.array([40, 110, 230]) * glow[..., None]
        Image.fromarray(img.astype(np.uint8)).resize((800, 800), Image.LANCZOS).save(preview)


if __name__ == "__main__":
    main(*sys.argv[1:])
