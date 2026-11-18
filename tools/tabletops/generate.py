"""Paint the tabletop textures (original art) used by the board themes.

    python3 tools/tabletops/generate.py

Writes app/src/assets/tables/<theme>.jpg. Everything is procedural and seeded, so the output is
the same every run. Needs numpy and Pillow.
"""

import math
import os

import numpy as np
from PIL import Image, ImageFilter

W, H = 2560, 1440
OUT = os.path.join(os.path.dirname(__file__), "..", "..", "app", "src", "assets", "tables")


# ---------------------------------------------------------------- noise helpers

def value_noise(rng, w, h, cell):
    """Smooth noise in [0, 1]: a random grid scaled up with bicubic filtering."""
    gw, gh = max(2, int(w / cell) + 2), max(2, int(h / cell) + 2)
    grid = (rng.random((gh, gw)) * 255).astype(np.uint8)
    img = Image.fromarray(grid, "L").resize((int(gw * cell), int(gh * cell)), Image.BICUBIC)
    return np.asarray(img, dtype=np.float32)[:h, :w] / 255.0


def fbm(rng, w, h, cell, octaves=5, gain=0.5):
    total, amp, norm = np.zeros((h, w), np.float32), 1.0, 0.0
    for _ in range(octaves):
        total += amp * value_noise(rng, w, h, cell)
        norm += amp
        amp *= gain
        cell = max(1.0, cell / 2)
    return total / norm


def stretch(a, lo=None, hi=None):
    lo = a.min() if lo is None else lo
    hi = a.max() if hi is None else hi
    return np.clip((a - lo) / (hi - lo + 1e-6), 0, 1)


def grid():
    y, x = np.mgrid[0:H, 0:W].astype(np.float32)
    return x, y


def vignette(strength=0.55, cx=0.5, cy=0.5, power=2.2):
    x, y = grid()
    d = np.sqrt(((x / W - cx) * 1.25) ** 2 + ((y / H - cy) * 1.0) ** 2)
    return 1 - strength * np.clip(d / 0.75, 0, 1) ** power


def mix(a, b, t):
    t = t[..., None] if t.ndim == 2 else t
    return a * (1 - t) + b * t


def color(hexstr):
    hexstr = hexstr.lstrip("#")
    return np.array([int(hexstr[i:i + 2], 16) / 255 for i in (0, 2, 4)], np.float32)


def ramp(t, stops):
    """Map t in [0, 1] through a list of (position, hex color) stops."""
    out = np.zeros(t.shape + (3,), np.float32)
    pos = [p for p, _ in stops]
    cols = [color(c) for _, c in stops]
    for i in range(len(stops) - 1):
        a, b = pos[i], pos[i + 1]
        m = (t >= a) & (t <= b)
        k = ((t - a) / (b - a + 1e-6))[m][..., None]
        out[m] = cols[i] * (1 - k) + cols[i + 1] * k
    out[t < pos[0]] = cols[0]
    out[t > pos[-1]] = cols[-1]
    return out


def glow(cx, cy, rx, ry):
    x, y = grid()
    d = ((x - cx) / rx) ** 2 + ((y - cy) / ry) ** 2
    return np.exp(-d)


def save(name, rgb):
    rgb = np.clip(rgb, 0, 1)
    # A whisper of grain so gradients never band.
    rng = np.random.default_rng(99)
    rgb = rgb + (rng.random(rgb.shape[:2], dtype=np.float32)[..., None] - 0.5) / 255 * 2
    img = Image.fromarray((np.clip(rgb, 0, 1) * 255).astype(np.uint8), "RGB")
    os.makedirs(OUT, exist_ok=True)
    path = os.path.join(OUT, f"{name}.jpg")
    img.save(path, quality=86, optimize=True, progressive=True)
    print(path, os.path.getsize(path) // 1024, "KB")


def shade(height, strength=2.0, light=(-0.6, -0.8)):
    """Lambert-style shading from a height field, lit from the top left."""
    gy, gx = np.gradient(height)
    lx, ly = light
    n = 1 / np.sqrt(1 + (gx * strength * 100) ** 2 + (gy * strength * 100) ** 2)
    d = (-(gx * strength * 100) * lx - (gy * strength * 100) * ly + 1) * n
    return np.clip(d, 0, 2)


# ---------------------------------------------------------------- themes

def night_table():
    """Night Table: violet baize under a low lamp, with a faint stitched ring."""
    rng = np.random.default_rng(1)
    x, y = grid()
    base = ramp(stretch(fbm(rng, W, H, 420, 4)), [(0, "#150c1d"), (0.5, "#22132d"), (1, "#2c1838")])
    # Felt: dense short fibers, slightly stretched horizontally.
    fib = value_noise(rng, W, H, 1.6) * 0.6 + value_noise(rng, W, H, 3.2) * 0.4
    fib = np.asarray(Image.fromarray((fib * 255).astype(np.uint8)).filter(ImageFilter.GaussianBlur(0.6)), np.float32) / 255
    nap = fbm(rng, W, H, 90, 4)
    rgb = base * (0.86 + 0.22 * fib[..., None]) * (0.92 + 0.16 * nap[..., None])
    # Woven diagonal twill, barely there.
    twill = 0.5 + 0.5 * np.sin((x + y) * 2 * math.pi / 7)
    rgb *= 0.985 + 0.03 * twill[..., None]
    # Lamp light from above.
    lamp = glow(W * 0.5, H * 0.35, W * 0.55, H * 0.75)
    rgb = rgb * (0.7 + 0.55 * lamp[..., None]) + color("#ffb98a") * 0.05 * lamp[..., None]
    # Embroidered sigil: rings and rune ticks in faded gold thread.
    cx, cy = W / 2, H / 2
    r = np.hypot(x - cx, y - cy)
    ang = np.arctan2(y - cy, x - cx)
    thread = np.zeros((H, W), np.float32)
    for rad, wdt in ((430, 2.2), (452, 1.2), (300, 1.4)):
        thread += np.exp(-((r - rad) / wdt) ** 2)
    ticks = (np.abs(np.sin(ang * 36)) > 0.985) & (r > 455) & (r < 478)
    thread += ticks * 0.9
    stitch = 0.55 + 0.45 * (np.sin((x * 0.9 + y * 0.4)) > 0)
    rgb = mix(rgb, color("#c9a35c") * (0.7 + 0.3 * fib[..., None]), np.clip(thread * stitch * 0.16, 0, 1))
    rgb *= vignette(0.6)[..., None]
    save("classic", rgb)


def ember_forge():
    """Ember Forge: riveted black iron plates over a bed of coals."""
    rng = np.random.default_rng(2)
    x, y = grid()
    # Plates.
    pw, ph = 520, 360
    px = (x + (np.floor(y / ph) % 2) * pw / 2) % pw
    py = y % ph
    edge = np.minimum(np.minimum(px, pw - px), np.minimum(py, ph - py))
    seam = np.clip(edge / 7, 0, 1)
    plate_id = (np.floor((x + (np.floor(y / ph) % 2) * pw / 2) / pw) * 7 + np.floor(y / ph) * 13) % 5
    rough = fbm(rng, W, H, 60, 6)
    brushed = value_noise(rng, W, H, 2.0)
    brushed = np.asarray(Image.fromarray((brushed * 255).astype(np.uint8)).resize((W, H)).filter(ImageFilter.BoxBlur(0)), np.float32) / 255
    streak = np.asarray(Image.fromarray((value_noise(rng, W, H, 3) * 255).astype(np.uint8)).filter(ImageFilter.GaussianBlur((12, 0)) if False else ImageFilter.BoxBlur(1)), np.float32) / 255
    height = rough * 0.6 + seam * 0.5 + streak * 0.08
    lit = shade(height, 1.4)
    tone = 0.16 + 0.05 * plate_id / 4 + 0.08 * rough
    rgb = np.stack([tone * 1.02, tone * 0.98, tone * 0.95], -1) * (0.55 + 0.6 * lit[..., None])
    rgb *= (0.35 + 0.65 * seam)[..., None]
    # Rivets near plate corners and along edges.
    rv = np.zeros((H, W), np.float32)
    for (ox, oy) in ((26, 26), (pw - 26, 26), (26, ph - 26), (pw - 26, ph - 26), (pw / 2, 26), (pw / 2, ph - 26)):
        d = np.hypot(px - ox, py - oy)
        rv = np.maximum(rv, np.clip(1 - d / 11, 0, 1))
    rivet_lit = shade(rv * 0.25, 3.0)
    rgb = mix(rgb, np.stack([0.28 * rivet_lit] * 3, -1) * np.array([1.0, 0.95, 0.9]), (rv > 0) * 0.95)
    # Soot and rust.
    soot = stretch(fbm(rng, W, H, 300, 5), 0.35, 0.75)
    rgb *= (1 - 0.45 * soot)[..., None]
    rust = stretch(fbm(rng, W, H, 140, 5), 0.6, 0.85)
    rgb = mix(rgb, color("#4a2414") * (0.6 + 0.6 * rough[..., None]), rust * 0.35)
    # Heat from the coals below: glowing seams and an orange wash.
    heat = glow(W * 0.5, H * 1.05, W * 0.75, H * 0.55)
    flick = fbm(rng, W, H, 160, 4)
    rgb += color("#ff5a1f") * (heat * (0.32 + 0.25 * flick))[..., None]
    rgb += color("#ffb347") * ((1 - seam) * heat * 0.9)[..., None]
    # Embers.
    for _ in range(420):
        ex, ey = rng.random() * W, H * (0.35 + 0.65 * rng.random() ** 0.6)
        rr = 0.8 + rng.random() * 2.6
        x0, x1 = int(max(0, ex - 14)), int(min(W, ex + 14))
        y0, y1 = int(max(0, ey - 14)), int(min(H, ey + 14))
        if x1 <= x0 or y1 <= y0:
            continue
        sx, sy = x[y0:y1, x0:x1], y[y0:y1, x0:x1]
        d = np.hypot(sx - ex, sy - ey)
        g = np.exp(-(d / rr) ** 2) + 0.25 * np.exp(-(d / (rr * 4)) ** 2)
        c = color("#ffd27a") if rng.random() < 0.4 else color("#ff7a3d")
        rgb[y0:y1, x0:x1] += c * g[..., None] * (0.5 + 0.5 * (ey / H))
    rgb *= vignette(0.55, cy=0.45)[..., None]
    save("forge", rgb)


def star_library():
    """Star Library: a deep-blue sky over an arcane circle, with drifting runes."""
    rng = np.random.default_rng(3)
    x, y = grid()
    # Nebula: domain-warped noise in blues and violets.
    wx = fbm(rng, W, H, 500, 4) - 0.5
    wy = fbm(rng, W, H, 500, 4) - 0.5
    n1 = fbm(rng, W, H, 380, 6)
    xi = np.clip((x + wx * 600).astype(int), 0, W - 1)
    yi = np.clip((y + wy * 600).astype(int), 0, H - 1)
    neb = n1[yi, xi]
    neb2 = fbm(rng, W, H, 220, 5)[yi, xi]
    sky = ramp(stretch(y / H * 0.6 + neb * 0.4), [(0, "#0b1433"), (0.5, "#0a1028"), (1, "#060915")])
    cloud = stretch(neb, 0.45, 0.85) ** 1.6
    rgb = sky + color("#3a4fb8") * (cloud * 0.35)[..., None] + color("#7a3fa8") * (stretch(neb2, 0.55, 0.9) ** 2 * 0.25)[..., None]
    rgb += color("#2fc6c9") * (stretch(neb * neb2, 0.35, 0.6) ** 3 * 0.12)[..., None]
    # Stars: many faint, a few bright with soft halos and cross flares.
    stars = np.zeros((H, W), np.float32)
    n = 5200
    sx, sy = rng.integers(0, W, n), rng.integers(0, H, n)
    br = rng.random(n) ** 3
    np.add.at(stars, (sy, sx), 0.35 + br * 1.6)
    stars = np.asarray(Image.fromarray(np.clip(stars * 160, 0, 255).astype(np.uint8)).filter(ImageFilter.GaussianBlur(0.7)), np.float32) / 255 * 2.2
    halo = np.asarray(Image.fromarray(np.clip(stars * 120, 0, 255).astype(np.uint8)).filter(ImageFilter.GaussianBlur(5)), np.float32) / 255
    rgb += (stars + halo * 0.6)[..., None] * color("#dfe8ff")
    for _ in range(22):
        bx, by = rng.random() * W, rng.random() * H
        x0, x1, y0, y1 = int(max(0, bx - 60)), int(min(W, bx + 60)), int(max(0, by - 60)), int(min(H, by + 60))
        ssx, ssy = x[y0:y1, x0:x1] - bx, y[y0:y1, x0:x1] - by
        d = np.hypot(ssx, ssy)
        g = np.exp(-(d / 2.2) ** 2) * 1.2 + np.exp(-(d / 14) ** 2) * 0.25
        g += (np.exp(-(ssy / 0.9) ** 2) * np.exp(-(np.abs(ssx) / 34))) * 0.35
        g += (np.exp(-(ssx / 0.9) ** 2) * np.exp(-(np.abs(ssy) / 34))) * 0.35
        rgb[y0:y1, x0:x1] += color("#cfe0ff") * g[..., None]
    # Arcane circle: rings, rune ticks and a few constellation lines.
    cx, cy = W / 2, H / 2
    r = np.hypot(x - cx, y - cy)
    ang = np.arctan2(y - cy, x - cx)
    line = np.zeros((H, W), np.float32)
    for rad, wdt, a in ((560, 1.4, 0.6), (585, 0.9, 0.45), (380, 1.1, 0.5), (200, 0.9, 0.35)):
        line += np.exp(-((r - rad) / wdt) ** 2) * a
    seg = (np.sin(ang * 64) > 0.2) & (r > 590) & (r < 612)
    line += seg * 0.18
    glyph = (np.abs(np.sin(ang * 12)) > 0.97) & (r > 390) & (r < 545)
    line += glyph * 0.12
    for k in range(6):
        a1, a2 = k * math.pi / 3, k * math.pi / 3 + 2 * math.pi / 3
        x1, y1 = cx + 380 * math.cos(a1), cy + 380 * math.sin(a1)
        x2, y2 = cx + 380 * math.cos(a2), cy + 380 * math.sin(a2)
        dx, dy = x2 - x1, y2 - y1
        t = np.clip(((x - x1) * dx + (y - y1) * dy) / (dx * dx + dy * dy), 0, 1)
        d = np.hypot(x - (x1 + t * dx), y - (y1 + t * dy))
        line += np.exp(-(d / 0.9) ** 2) * 0.28
    line_glow = np.asarray(Image.fromarray(np.clip(line * 255, 0, 255).astype(np.uint8)).filter(ImageFilter.GaussianBlur(6)), np.float32) / 255
    rgb += color("#8fb2ff") * (line * 0.55 + line_glow * 0.5)[..., None]
    rgb *= vignette(0.5)[..., None]
    save("arcane", rgb)


def voronoi(rng, w, h, cell):
    """Distance to nearest and second-nearest jittered grid point, plus the cell id."""
    gw, gh = int(w / cell) + 3, int(h / cell) + 3
    pts = (np.stack(np.meshgrid(np.arange(gw), np.arange(gh)), -1) + rng.random((gh, gw, 2)) * 0.9 + 0.05) * cell - cell
    x, y = grid()
    cxi, cyi = (x // cell).astype(int) + 1, (y // cell).astype(int) + 1
    d1 = np.full((h, w), 1e9, np.float32)
    d2 = np.full((h, w), 1e9, np.float32)
    cid = np.zeros((h, w), np.float32)
    for oy in (-1, 0, 1):
        for ox in (-1, 0, 1):
            gx = np.clip(cxi + ox, 0, gw - 1)
            gy = np.clip(cyi + oy, 0, gh - 1)
            p = pts[gy, gx]
            d = np.hypot(x - p[..., 0], y - p[..., 1])
            closer = d < d1
            d2 = np.where(closer, d1, np.minimum(d2, d))
            cid = np.where(closer, (gx * 31 + gy * 17) % 97 / 97, cid)
            d1 = np.where(closer, d, d1)
    return d1, d2, cid


def mossy_keep():
    """Mossy Keep: worn flagstones with moss creeping through the joints."""
    rng = np.random.default_rng(4)
    d1, d2, cid = voronoi(rng, W, H, 340)
    joint = np.clip((d2 - d1) / 16, 0, 1)
    grain = fbm(rng, W, H, 40, 6)
    big = fbm(rng, W, H, 300, 4)
    height = joint ** 0.6 * 0.7 + grain * 0.25 + (1 - np.clip(d1 / 180, 0, 1)) * 0.08
    lit = shade(height, 1.1)
    stone = ramp(stretch(cid * 0.5 + big * 0.5), [(0, "#4a4d47"), (0.5, "#5d6058"), (1, "#6e6a5f")])
    rgb = stone * (0.55 + 0.55 * lit[..., None]) * (0.8 + 0.3 * grain[..., None])
    rgb *= (0.3 + 0.7 * joint)[..., None]
    # Pits and lichen spots.
    pits = stretch(fbm(rng, W, H, 12, 3), 0.62, 0.8)
    rgb *= (1 - 0.25 * pits)[..., None]
    lichen = stretch(fbm(rng, W, H, 70, 4), 0.68, 0.82)
    rgb = mix(rgb, color("#8c8a5a") * (0.7 + 0.4 * grain[..., None]), lichen * 0.35)
    # Moss in the joints and where water sits.
    wet = stretch(fbm(rng, W, H, 220, 5), 0.42, 0.75)
    moss_amt = np.clip((1 - joint) * 1.4 * (0.4 + wet) + wet ** 3 * 0.5, 0, 1)
    tuft = fbm(rng, W, H, 6, 3)
    moss = ramp(stretch(tuft + wet * 0.5), [(0, "#1f3317"), (0.5, "#3c5e24"), (1, "#6f9a3a")])
    rgb = mix(rgb, moss * (0.6 + 0.6 * tuft[..., None]), np.clip(moss_amt * (0.6 + 0.6 * tuft), 0, 1) * 0.9)
    # Cool light from a high window.
    lightwash = glow(W * 0.3, H * 0.15, W * 0.7, H * 0.8)
    rgb = rgb * (0.65 + 0.5 * lightwash[..., None]) + color("#b9d0ff") * 0.03 * lightwash[..., None]
    rgb *= 0.82 * vignette(0.55)[..., None]
    save("moss", rgb)


def old_tavern():
    """Old Tavern: oiled oak planks, knots and nails, warm in the firelight."""
    rng = np.random.default_rng(5)
    x, y = grid()
    plank_w = 210
    pi = np.floor(x / plank_w)
    # Each plank is its own board: offset grain, its own tone, staggered end joints.
    off = (pi * 7919 % 1000) / 1000
    tone = (pi * 104729 % 997) / 997
    # Grain runs along the plank, so its noise is stretched tall before use.
    def tall(cell, octaves, k=12):
        n = fbm(rng, W, H // k, cell, octaves)
        return np.asarray(Image.fromarray((n * 255).astype(np.uint8)).resize((W, H), Image.BICUBIC), np.float32) / 255 - 0.5
    warp = tall(60, 4)
    warp2 = tall(8, 3, 20)
    # Cathedral arches: the growth rings bow toward one end of each board.
    arch = 0.5 - 0.5 * np.cos((y + off * 3000) * 2 * math.pi / 1400)
    u = (x % plank_w) / plank_w
    grain_coord = u * 5 + off * 50 + warp * 1.6 + warp2 * 0.35 + arch * 1.1 * np.sin(u * math.pi)
    rings = 0.5 + 0.5 * np.sin(grain_coord * 2 * math.pi)
    rings = rings ** 1.6
    fine = value_noise(rng, W, H, 1.5)
    fine = np.asarray(Image.fromarray((fine * 255).astype(np.uint8)).resize((W // 6, H)).resize((W, H), Image.BICUBIC), np.float32) / 255
    wood = ramp(stretch(rings * 0.55 + tone[..., ] * 0.3 + fine * 0.15), [(0, "#3b2213"), (0.45, "#6a4024"), (0.8, "#8a5832"), (1, "#a06a3c")])
    # Knots: dark ellipses with grain bending round them.
    for _ in range(14):
        kx, ky = rng.random() * W, rng.random() * H
        kx = (math.floor(kx / plank_w) + 0.3 + rng.random() * 0.4) * plank_w
        rx, ry = 10 + rng.random() * 14, 18 + rng.random() * 26
        d = np.sqrt(((x - kx) / rx) ** 2 + ((y - ky) / ry) ** 2)
        ring = 0.5 + 0.5 * np.sin(d * 7)
        k = np.exp(-(d / 1.0) ** 2)
        wood = mix(wood, color("#2a170c") * (0.7 + 0.5 * ring[..., None]), np.clip(k * 1.2, 0, 1))
        wood *= (1 - 0.18 * np.exp(-(d / 2.4) ** 2) * ring)[..., None]
    # Seams between planks and staggered end joints.
    seam = np.clip(np.minimum(x % plank_w, plank_w - x % plank_w) / 3.0, 0, 1)
    joint_y = (pi * 431 % 7) / 7 * H * 0.9 + H * 0.05
    jseam = np.clip(np.abs(y - joint_y) / 2.5, 0, 1)
    edge = seam * jseam
    height = rings * 0.05 + fine * 0.05 + edge * 0.4
    lit = shade(height, 1.2)
    rgb = wood * (0.6 + 0.5 * lit[..., None]) * (0.25 + 0.75 * edge)[..., None]
    # Nails near the plank ends.
    for i in range(int(W / plank_w) + 1):
        for ny in (joint_y[0, min(W - 1, int(i * plank_w + 5))] - 22, joint_y[0, min(W - 1, int(i * plank_w + 5))] + 22):
            for nx in (i * plank_w + 26, i * plank_w + plank_w - 26):
                d = np.hypot(x - nx, y - ny)
                m = np.clip(1 - d / 5.5, 0, 1)
                rgb = mix(rgb, np.stack([0.18 + 0.25 * m] * 3, -1) * np.array([1.0, 0.95, 0.88]), (m > 0) * 0.9)
    # Wear, spills and the warmth of the hearth.
    wear = stretch(fbm(rng, W, H, 260, 5), 0.5, 0.8)
    rgb *= (1 - 0.18 * wear)[..., None]
    fire = glow(W * 0.5, H * 0.55, W * 0.7, H * 0.8)
    rgb = rgb * (0.55 + 0.6 * fire[..., None]) + color("#ff9a4a") * (0.06 * fire)[..., None]
    rgb *= vignette(0.6)[..., None]
    save("tavern", rgb)


if __name__ == "__main__":
    for f in (night_table, ember_forge, star_library, mossy_keep, old_tavern):
        f()
