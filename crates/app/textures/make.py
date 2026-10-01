#!/usr/bin/env python3
"""The surfaces' textures (`crates/app/src/surfaces.rs`), made from nothing.

Every picture is 1024 texels square and repeats without a seam: the noise
is made in the frequency domain (which repeats by construction), the
stones, plates and boards are laid on a torus, and anything drawn near an
edge is drawn again across it. Relief is a height field lit from the
north-west — the side the trees' shadows fall away from — with the
crevices darkened, so a plate's bevel, a stone's face and a board's
grain read as depth on a flat deck.

What is written is **the surface over its own average, a quarter scale**:
each channel averages 64, and the shader multiplies by four and by the
painter's colour. So the hue here is only the variation about the average
(a dry patch in the grass, rust on a plate); the painter's colour is the
surface's colour. `preview/` gets each one tinted by its painter's colour
too, for looking at.

    nix-shell -p "python3.withPackages(ps: [ps.numpy ps.scipy ps.pillow])" \\
        --run "python3 crates/app/textures/make.py"

Seeded: the same pictures every run.
"""

import os
import sys

import numpy as np
from PIL import Image, ImageDraw, ImageFilter
from scipy.spatial import cKDTree

N = 1024
HERE = os.path.dirname(os.path.abspath(__file__))
PREVIEW = os.path.join(HERE, "preview")

# --- tools -------------------------------------------------------------------------


def freq(n=N):
    k = np.fft.fftfreq(n) * n
    return np.meshgrid(k, k)


def spectral(rng, beta, lo=1.0, hi=None, aniso=(1.0, 1.0)):
    """Repeating noise whose power falls as 1/f^beta between lo and hi
    cycles a picture; zero mean, unit deviation. `aniso` stretches it: the
    noise runs along the axis with the larger number, (1, 0.05) being
    streaks along x twenty times as long as they are wide; `lo` and `hi`
    are along the longer way."""
    kx, ky = freq()
    f = np.sqrt((kx * aniso[0]) ** 2 + (ky * aniso[1]) ** 2)
    f[0, 0] = 1.0
    amp = f ** (-beta / 2.0)
    amp[f < lo] = 0.0
    if hi is not None:
        amp *= np.exp(-((f / hi) ** 2))
    amp[0, 0] = 0.0
    z = rng.normal(size=(N, N)) + 1j * rng.normal(size=(N, N))
    img = np.real(np.fft.ifft2(amp * z))
    return (img - img.mean()) / (img.std() + 1e-12)


def blur(img, sigma):
    """A repeating Gaussian blur (over the last two axes' first two)."""
    if sigma <= 0:
        return img
    kx, ky = freq(img.shape[0])
    g = np.exp(-2.0 * (np.pi * sigma / img.shape[0]) ** 2 * (kx**2 + ky**2))
    if img.ndim == 3:
        return np.stack([blur(img[..., c], sigma) for c in range(img.shape[2])], -1)
    return np.real(np.fft.ifft2(np.fft.fft2(img) * g))


def smooth(t):
    t = np.clip(t, 0.0, 1.0)
    return t * t * (3 - 2 * t)


def lit(h, strength=1.0):
    """How lit a height field is from the north-west, 1 where it is flat."""
    gx = (np.roll(h, -1, 1) - np.roll(h, 1, 1)) * 0.5 * strength
    gy = (np.roll(h, -1, 0) - np.roll(h, 1, 0)) * 0.5 * strength
    light = np.array([-1.0, -1.0, 1.7])
    light /= np.linalg.norm(light)
    d = (-gx * light[0] - gy * light[1] + light[2]) / np.sqrt(gx * gx + gy * gy + 1.0)
    return np.clip(d / light[2], 0.0, None)


def occlusion(h, radius, strength):
    """Darker where the height is below its surroundings."""
    return np.clip(1.0 - strength * np.clip(blur(h, radius) - h, 0, None), 0.0, 1.0)


def mix(a, b, t):
    t = np.asarray(t)
    if t.ndim == 2:
        t = t[..., None]
    return a + (b - a) * t


def colour(c):
    return np.array(c, dtype=np.float64)[None, None, :]


def torus_distance(points, n=N):
    """Distance to the nearest of `points` (x, y) and which, on the torus."""
    tree = cKDTree(np.mod(points, n), boxsize=n)
    ys, xs = np.mgrid[0:n, 0:n]
    q = np.stack([xs.ravel() + 0.5, ys.ravel() + 0.5], -1)
    d, i = tree.query(q, k=2)
    return d[:, 0].reshape(n, n), d[:, 1].reshape(n, n), i[:, 0].reshape(n, n)


def wrapped_draw(draw_fn, n=N, margin=64):
    """Calls draw_fn(dx, dy) for every shift that can bring a shape drawn
    near an edge back in across the other."""
    for dy in (-n, 0, n):
        for dx in (-n, 0, n):
            draw_fn(dx, dy)


def finish(name, rgb, tint, seed_note=""):
    """Writes `rgb` as the surface over its average, a quarter scale, and a
    preview tinted by the painter's colour."""
    rgb = np.clip(rgb, 1e-4, None)
    out = rgb / rgb.reshape(-1, 3).mean(0)[None, None, :] * 64.0
    # The clip at 255 lowers the average a hair; put it back.
    for _ in range(4):
        out = np.clip(out, 0, 255)
        out *= 64.0 / out.reshape(-1, 3).mean(0)[None, None, :]
    out = np.clip(np.round(out), 0, 255).astype(np.uint8)
    Image.fromarray(out, "RGB").save(os.path.join(HERE, name + ".png"), optimize=True)
    os.makedirs(PREVIEW, exist_ok=True)
    shown = np.clip(out.astype(np.float64) * 4.0 / 255.0 * colour(tint)[0, 0], 0, 1)
    Image.fromarray((shown * 255).astype(np.uint8), "RGB").save(
        os.path.join(PREVIEW, name + ".png")
    )
    means = out.reshape(-1, 3).mean(0)
    print(f"{name}: mean {means.round(2)} min {out.min()} max {out.max()}", file=sys.stderr)


# --- the deck ----------------------------------------------------------------------


def deck():
    """Steel deck plates, a tile (256 texels) each, drawn for the zoom the
    game is played at — a tile is forty to a hundred pixels, so nothing
    that matters is under a dozen texels: a bevelled seam and grime in it,
    a countersunk bolt in every corner, the paint a tone a plate and worn
    through to bright steel along the walkways and at the edges, scuffs,
    a stain here and there. The grain of the steel turns a quarter from
    one plate to the next, as plates are laid."""
    rng = np.random.default_rng(11)
    P = 256
    ys, xs = np.mgrid[0:N, 0:N].astype(np.float64)
    px, py = (xs % P), (ys % P)
    plate = (ys // P).astype(int) * 4 + (xs // P).astype(int)
    edge = np.minimum(np.minimum(px, P - 1 - px), np.minimum(py, P - 1 - py))
    seam = 4.0
    h = smooth((edge - seam) / 9.0) * 2.0
    # The bolts: a dished well and a domed head with a hex socket.
    bolt = np.zeros((N, N))
    for cx in (30.0, P - 31.0):
        for cy in (30.0, P - 31.0):
            r = np.sqrt((px - cx) ** 2 + (py - cy) ** 2)
            head = smooth((11.0 - r) / 2.5)
            dish = smooth((15.0 - r) / 2.0)
            socket = smooth((4.5 - r) / 1.2)
            h = h - dish * 0.8 + head * 1.4 - socket * 0.9
            bolt = np.maximum(bolt, head)
    # The grain: brushed along x on one plate, along y on the next.
    along = spectral(rng, 1.0, lo=3, hi=25, aniso=(1.0, 0.06))
    across = spectral(rng, 1.0, lo=3, hi=25, aniso=(0.06, 1.0))
    turn = ((plate + plate // 4) % 2).astype(bool)
    grain = np.where(turn, across, along)
    # Paint: one tone a plate, mottled.
    tone = rng.normal(0, 0.09, 16)[plate]
    mottle = spectral(rng, 2.4, lo=2, hi=40)
    paint = colour((0.30, 0.33, 0.37))
    bare = colour((0.58, 0.60, 0.62))
    base = paint * (1.0 + tone[..., None] + 0.08 * mottle[..., None])
    # Worn through: at the plate's edges and bolts where boots catch,
    # and in broad lanes where the traffic goes.
    wear_noise = spectral(rng, 2.0, lo=4, hi=60)
    edge_wear = smooth((0.6 + wear_noise * 0.45 - edge / 40.0) / 0.3) * (edge > seam)
    lanes = smooth((spectral(rng, 2.8, lo=1, hi=12) - 0.2) / 1.2)
    lane_wear = lanes * smooth((spectral(rng, 1.8, lo=6, hi=80) + 0.1) / 1.0)
    worn = np.clip(np.maximum(edge_wear, lane_wear * 0.75) + bolt * 0.6, 0, 1)
    base = mix(base, bare * (1.0 + 0.10 * grain[..., None]), worn * 0.65)
    base = base * (1.0 + 0.035 * grain[..., None])
    # Scuffs: dark smudges of boot rubber, and long drag marks.
    scuff = smooth((spectral(rng, 2.2, lo=3, hi=40) - 1.2) / 0.8)
    img = Image.new("L", (N, N), 0)
    d = ImageDraw.Draw(img)
    for _ in range(140):
        x, y = rng.uniform(0, N, 2)
        a = rng.normal(0, 0.25) + (np.pi / 2 if rng.random() < 0.5 else 0)
        length = rng.uniform(40, 200)
        x2, y2 = x + np.cos(a) * length, y + np.sin(a) * length
        v = int(rng.uniform(60, 160))
        w = int(rng.integers(2, 5))

        def one(dx, dy, x=x, y=y, x2=x2, y2=y2, v=v, w=w):
            d.line([(x + dx, y + dy), (x2 + dx, y2 + dy)], fill=v, width=w)

        wrapped_draw(one)
    drags = blur(np.asarray(img, np.float64) / 255.0, 1.5)
    base = base * (1.0 - 0.30 * scuff[..., None]) * (1.0 - 0.35 * drags[..., None])
    # Rust blooms and oil.
    stains = smooth((spectral(rng, 2.6, lo=1.5, hi=30) - 1.3) / 1.0)
    rust = colour((0.46, 0.27, 0.15))
    base = mix(base, rust, stains * 0.45)
    oil = smooth((spectral(rng, 2.8, lo=2, hi=25) - 1.6) / 0.6)
    base = base * (1.0 - 0.45 * oil[..., None])
    # Grime gathers towards the seams and round the bolts.
    grime = smooth((22.0 - edge) / 20.0) * (0.5 + 0.5 * smooth((wear_noise + 0.5) / 1.5))
    base = base * (1.0 - 0.30 * grime[..., None])
    shade = lit(h, 1.6) * occlusion(h, 5.0, 0.8)
    rgb = base * shade[..., None]
    rgb[edge < seam] *= 0.18
    finish("deck", rgb, (0.13, 0.15, 0.18))


# --- the bulkhead ------------------------------------------------------------------


def bulkhead():
    """A bulkhead's face, a panel a tile: a raised frame with rivets along
    it, a recessed panel inside with a bevel, the paint streaked and
    chipped, a vent on one panel in five."""
    rng = np.random.default_rng(12)
    P = 256
    ys, xs = np.mgrid[0:N, 0:N].astype(np.float64)
    px, py = xs % P, ys % P
    panel = (ys // P).astype(int) * 4 + (xs // P).astype(int)
    edge = np.minimum(np.minimum(px, P - 1 - px), np.minimum(py, P - 1 - py))
    # Frame up to 30 texels in, then a bevel down to the panel.
    frame_w = 30.0
    h = 1.0 - smooth((edge - frame_w) / 7.0) * 0.6
    h -= smooth((3.0 - edge) / 2.0) * 0.7  # the joint between panels
    # Rivets along the frame.
    for k in range(6):
        t = 18.0 + k * (P - 36.0) / 5.0
        for cx, cy in ((t, 15.0), (t, P - 16.0), (15.0, t), (P - 16.0, t)):
            r = np.sqrt((px - cx) ** 2 + (py - cy) ** 2)
            h += smooth((6.5 - r) / 2.0) * 0.6
    # A vent: slots let into some panels.
    vents = rng.random(16) < 0.22
    inside = (edge > frame_w + 12)
    slot_rows = (np.abs(((py - P / 2) % 18) - 9) < 4) & (np.abs(px - P / 2) < 60) & (np.abs(py - P / 2) < 50)
    h -= (vents[panel] & slot_rows & inside) * 0.5
    # Paint and wear.
    tone = rng.normal(0, 0.045, 16)[panel]
    mottle = spectral(rng, 2.2, lo=2, hi=150)
    grain = spectral(rng, 1.0, lo=4, hi=28, aniso=(1.0, 0.07))
    paint = colour((0.36, 0.40, 0.46))
    bare = colour((0.62, 0.64, 0.66))
    base = paint * (1.0 + tone[..., None] + 0.05 * mottle[..., None] + 0.03 * grain[..., None])
    # The recessed panel a shade darker than the frame.
    base *= (1.0 - 0.10 * smooth((edge - frame_w) / 7.0))[..., None]
    chips_n = spectral(rng, 1.4, lo=20, hi=400)
    ridge = smooth((frame_w + 2 - np.abs(edge - frame_w)) / 4.0) * 0.0 + smooth((10 - edge) / 8.0)
    chips = smooth((chips_n - 2.3 + ridge * 1.4) / 0.25)
    base = mix(base, bare, chips * 0.8)
    # Streaks running down from the rivets and the top of each panel.
    streak = spectral(rng, 1.2, lo=1, hi=12, aniso=(0.04, 1.0))
    streak = smooth((streak - 0.6) / 1.4)
    base = base * (1.0 - 0.18 * streak[..., None])
    rust = colour((0.42, 0.25, 0.14))
    rusty = smooth((spectral(rng, 2.4, lo=2, hi=80) - 1.2) / 1.0)
    base = mix(base, rust, rusty * 0.25 * smooth((40 - edge) / 30.0))
    rgb = base * (lit(h, 2.6) * occlusion(h, 4.0, 1.5))[..., None]
    finish("bulkhead", rgb, (0.30, 0.34, 0.40))


# --- the ground --------------------------------------------------------------------


def grass():
    """A lawn from above: blades in a dozen greens, yellowing in dry
    patches, the soil showing where it is thin, clover and the odd
    dandelion leaf, all over a soil with clods in it."""
    rng = np.random.default_rng(21)
    S = 2  # drawn at twice the size, then halved, for clean blades
    M = N * S
    soil_n = spectral(rng, 1.6, lo=4, hi=500)
    soil = colour((0.22, 0.22, 0.13)) * (1.0 + 0.12 * soil_n[..., None])
    density = spectral(rng, 2.4, lo=2, hi=80)
    dry = smooth((spectral(rng, 2.6, lo=1.5, hi=40) - 0.6) / 1.4)
    img = Image.fromarray((np.clip(soil, 0, 1) * 255).astype(np.uint8), "RGB").resize((M, M))
    d = ImageDraw.Draw(img)
    greens = np.array(
        [
            (0.22, 0.36, 0.12), (0.26, 0.42, 0.14), (0.30, 0.47, 0.16), (0.20, 0.32, 0.11),
            (0.34, 0.50, 0.20), (0.24, 0.38, 0.17), (0.28, 0.40, 0.12), (0.18, 0.30, 0.10),
        ]
    )
    yellows = np.array([(0.40, 0.42, 0.18), (0.44, 0.44, 0.22), (0.36, 0.38, 0.16), (0.46, 0.45, 0.25)])
    blades = 340000
    xs = rng.uniform(0, N, blades)
    ys = rng.uniform(0, N, blades)
    ix, iy = xs.astype(int) % N, ys.astype(int) % N
    keep = rng.random(blades) < smooth(0.75 + 0.30 * density[iy, ix])
    for x, y, i, j in zip(xs[keep], ys[keep], ix[keep], iy[keep]):
        a = rng.uniform(0, 2 * np.pi)
        length = rng.uniform(4, 13) * S
        bend = rng.normal(0, 0.35)
        if rng.random() < dry[j, i] * 0.9:
            c = yellows[rng.integers(len(yellows))]
        else:
            c = greens[rng.integers(len(greens))]
        c = c * rng.uniform(0.8, 1.2)
        x0, y0 = x * S, y * S
        mx, my = x0 + np.cos(a) * length * 0.5, y0 + np.sin(a) * length * 0.5
        x1, y1 = x0 + np.cos(a + bend) * length, y0 + np.sin(a + bend) * length
        base_c = tuple(int(v * 255 * 0.7) for v in np.clip(c, 0, 1))
        tip_c = tuple(int(v * 255) for v in np.clip(c * 1.25, 0, 1))
        w = int(rng.integers(2, 4))

        def one(dx, dy):
            d.line([(x0 + dx, y0 + dy), (mx + dx, my + dy)], fill=base_c, width=w)
            d.line([(mx + dx, my + dy), (x1 + dx, y1 + dy)], fill=tip_c, width=max(1, w - 1))

        if min(x, y) < 12 or max(x, y) > N - 12:
            wrapped_draw(lambda dx, dy: one(dx * S, dy * S))
        else:
            one(0, 0)
    # Clover: little three-leafed rosettes in a few clumps.
    clover = smooth((spectral(rng, 2.2, lo=3, hi=60) - 1.3) / 0.6)
    for _ in range(2500):
        x, y = rng.uniform(0, N, 2)
        if rng.random() > clover[int(y) % N, int(x) % N]:
            continue
        c = tuple(int(v * 255 * rng.uniform(0.85, 1.1)) for v in (0.17, 0.30, 0.12))
        turn = rng.uniform(0, 2 * np.pi)
        for k in range(3):
            a = k * 2 * np.pi / 3 + turn
            lx, ly = (x + np.cos(a) * 1.6) * S, (y + np.sin(a) * 1.6) * S
            d.ellipse([lx - 1.7 * S, ly - 1.7 * S, lx + 1.7 * S, ly + 1.7 * S], fill=c)
    img = img.resize((N, N), Image.LANCZOS)
    rgb = np.asarray(img, np.float64) / 255.0
    # Depth: the blades over the soil, the soil in the shade under them.
    lum = rgb.mean(-1)
    h = blur(lum, 0.8) * 6.0
    rgb = rgb * (lit(h, 1.2) * occlusion(h, 3.0, 0.5))[..., None]
    rgb *= 1.0 + 0.10 * spectral(rng, 2.0, lo=2, hi=40)[..., None]
    finish("grass", rgb, (0.27, 0.35, 0.21))


def sand():
    """Desert sand: wind ripples, warped and lit low from the north-west,
    a fine grain of a dozen colours in it, darker coarse sand in the
    troughs, and a scatter of pebbles."""
    rng = np.random.default_rng(31)
    ys, xs = np.mgrid[0:N, 0:N].astype(np.float64)
    warp_x = spectral(rng, 3.0, lo=1, hi=20) * 40.0
    warp_y = spectral(rng, 3.0, lo=1, hi=20) * 40.0
    # Ripples across the wind (which blows along x, a little off), eleven
    # wavelengths of about 93 texels... and a second set finer.
    k1 = 2 * np.pi * 11 / N
    k2 = 2 * np.pi * 26 / N
    phase1 = (xs + warp_x) * k1 * 0.25 + (ys + warp_y) * k1
    phase2 = (xs * 0.4 + ys + warp_y * 1.6) * k2
    # The ripple's profile: a gentle stoss side and a steep lee side.
    def profile(p):
        t = (p / (2 * np.pi)) % 1.0
        return np.where(t < 0.75, t / 0.75, (1.0 - t) / 0.25)
    strength = smooth((spectral(rng, 2.5, lo=1, hi=30) + 1.2) / 2.0)
    h = profile(phase1) * 5.0 * strength + profile(phase2) * 1.2 * (1 - 0.5 * strength)
    h += spectral(rng, 2.8, lo=1, hi=60) * 2.5
    grain = rng.normal(0, 1, (N, N))
    tones = np.array([
        (0.80, 0.66, 0.46), (0.74, 0.58, 0.38), (0.86, 0.74, 0.54), (0.66, 0.50, 0.32),
        (0.90, 0.80, 0.62), (0.70, 0.56, 0.40), (0.60, 0.46, 0.30), (0.82, 0.70, 0.52),
    ])
    pick = rng.integers(0, len(tones), (N, N))
    speck = tones[pick]
    base = colour((0.78, 0.64, 0.44)) * (1.0 + 0.04 * blur(grain, 0.6)[..., None])
    base = mix(base, speck, 0.25)
    trough = smooth((1.2 - profile(phase1) * strength * 3.0) / 1.5)
    base = base * (1.0 - 0.08 * trough[..., None])
    # Pebbles.
    img = Image.new("RGBA", (N, N), (0, 0, 0, 0))
    d = ImageDraw.Draw(img)
    hp = Image.new("L", (N, N), 0)
    dh = ImageDraw.Draw(hp)
    for _ in range(260):
        x, y = rng.uniform(0, N, 2)
        r = rng.uniform(1.5, 5.0) if rng.random() < 0.9 else rng.uniform(5, 9)
        e = rng.uniform(0.6, 1.0)
        g = rng.uniform(0.45, 0.85)
        c = tuple(int(255 * g * v) for v in (rng.uniform(0.85, 1.0), rng.uniform(0.72, 0.85), rng.uniform(0.55, 0.7)))

        def one(dx, dy, x=x, y=y, r=r, e=e, c=c):
            box = [x - r + dx, y - r * e + dy, x + r + dx, y + r * e + dy]
            d.ellipse(box, fill=c + (255,))
            dh.ellipse(box, fill=200)

        wrapped_draw(one)
    peb = np.asarray(img, np.float64) / 255.0
    ph = blur(np.asarray(hp, np.float64) / 255.0, 1.2) * 4.0
    base = mix(base, peb[..., :3], peb[..., 3])
    h = h + ph
    rgb = base * (lit(h, 0.9) * occlusion(ph, 2.0, 0.6))[..., None]
    finish("sand", rgb, (0.44, 0.35, 0.23))


def snow():
    """Wind-packed snow: soft drifts and sastrugi lit from the north-west
    with blue in the shade, a fine crystalline grain, and a few bright
    glints."""
    rng = np.random.default_rng(41)
    h = spectral(rng, 3.4, lo=1, hi=30) * 22.0
    h += spectral(rng, 2.8, lo=2, hi=20, aniso=(1.0, 0.3)) * 3.0
    h += spectral(rng, 1.2, lo=100, hi=400) * 0.05
    shade = lit(h, 0.35)
    white = colour((0.96, 0.97, 1.0))
    blue = colour((0.78, 0.84, 0.95))
    rgb = mix(blue, white, smooth((shade - 0.6) / 0.6))
    rgb = rgb * (0.86 + 0.14 * shade[..., None])
    rgb *= 1.0 + 0.025 * rng.normal(0, 1, (N, N))[..., None]
    glints = (rng.random((N, N)) < 0.0012) & (shade > 1.0)
    rgb[glints] = np.array([1.6, 1.6, 1.6])
    rgb = blur(rgb, 0.35)
    finish("snow", rgb, (0.47, 0.51, 0.56))


def rock():
    """Bare rock for the cliffs: ridged, cracked into slabs, bedded in
    strata, lichen in the hollows."""
    rng = np.random.default_rng(51)
    ridged = 1.0 - np.abs(spectral(rng, 2.6, lo=2, hi=120))
    h = spectral(rng, 3.0, lo=1, hi=200) * 14.0 + ridged * 4.0 + spectral(rng, 1.6, lo=40, hi=300) * 0.3
    ys, xs = np.mgrid[0:N, 0:N].astype(np.float64)
    strata = np.sin((ys + spectral(rng, 3.0, lo=1, hi=10) * 50.0) * 2 * np.pi * 14 / N)
    h += strata * 2.0
    pts = rng.uniform(0, N, (60, 2))
    d1, d2, _ = torus_distance(pts)
    crack = smooth((3.0 - (d2 - d1) - spectral(rng, 1.5, lo=10, hi=200) * 1.5) / 2.5)
    h -= crack * 6.0
    base = colour((0.50, 0.48, 0.44)) * (1.0 + 0.10 * spectral(rng, 1.8, lo=3, hi=300)[..., None])
    base = base * (1.0 + 0.06 * strata[..., None])
    lichen = smooth((spectral(rng, 2.0, lo=4, hi=120) - 1.3) / 0.5)
    base = mix(base, colour((0.48, 0.50, 0.32)), lichen * 0.5)
    rgb = base * (lit(h, 0.45) * occlusion(h, 4.0, 0.10))[..., None]
    rgb *= (1.0 - 0.5 * crack)[..., None]
    finish("rock", rgb, (0.40, 0.38, 0.34))


# --- the town's walls and floors ---------------------------------------------------


def stone():
    """Dressed stone in courses, two a tile (128 texels): blocks of
    different lengths and tones, their arrises chipped, their faces
    tooled, set in a recessed mortar."""
    rng = np.random.default_rng(61)
    C = 128
    ys, xs = np.mgrid[0:N, 0:N].astype(np.float64)
    course = (ys // C).astype(int)
    h = np.zeros((N, N))
    block_id = np.full((N, N), -1)
    dist = np.zeros((N, N))
    edges_noise = spectral(rng, 2.0, lo=8, hi=120) * 2.0
    ids = 0
    for c in range(N // C):
        # Block boundaries along this course.
        cuts = [rng.uniform(0, 60)]
        while cuts[-1] < N + cuts[0] - 120:
            cuts.append(cuts[-1] + rng.uniform(150, 300))
        span = N + cuts[0] - cuts[-1]
        if span < 120:
            cuts.pop()
        cuts.append(cuts[0] + N)
        rows = (course == c)
        y0 = c * C
        py = ys[rows] - y0
        for a, b in zip(cuts[:-1], cuts[1:]):
            x = xs[rows]
            u = (x - a) % N
            inside = u < (b - a)
            ex = np.minimum(u, (b - a) - u)
            ey = np.minimum(py, C - 1 - py)
            dd = np.minimum(ex, ey)
            sel = np.zeros_like(rows)
            sel[rows] = inside
            block_id[sel] = ids
            dist[sel] = dd[inside]
            ids += 1
    dist = dist + edges_noise
    mortar = 5.0
    face = smooth((dist - mortar) / 6.0)
    tool = spectral(rng, 2.6, lo=3, hi=50) * 1.4 + spectral(rng, 1.2, lo=80, hi=400) * 0.05
    h = face * (3.0 + tool) + (1 - face) * spectral(rng, 1.0, lo=60) * 0.3
    tones = rng.normal(0, 0.07, ids + 1)
    warm = rng.normal(0, 0.03, ids + 1)
    t = tones[block_id]
    w = warm[block_id]
    stone_c = colour((0.62, 0.59, 0.53))
    base = stone_c * (1.0 + t[..., None] + 0.06 * spectral(rng, 1.8, lo=6, hi=300)[..., None])
    base[..., 0] *= 1 + w
    base[..., 2] *= 1 - w
    mortar_c = colour((0.42, 0.40, 0.36)) * (1.0 + 0.12 * rng.normal(0, 1, (N, N))[..., None])
    base = mix(mortar_c, base, face)
    moss = smooth((spectral(rng, 2.3, lo=2, hi=60) - 1.0) / 0.8) * (1 - face * 0.7)
    base = mix(base, colour((0.30, 0.36, 0.20)), moss * 0.5)
    rgb = base * (lit(h, 0.9) * occlusion(h, 3.0, 0.3))[..., None]
    finish("stone", rgb, (0.48, 0.45, 0.40))


def adobe():
    """Sun-baked plaster over mud brick: trowel-smoothed, crazed with fine
    cracks, stained at the foot, and fallen away in a patch or two to the
    bricks under it."""
    rng = np.random.default_rng(71)
    ys, xs = np.mgrid[0:N, 0:N].astype(np.float64)
    trowel = spectral(rng, 2.8, lo=3, hi=60) * 2.4 + spectral(rng, 1.5, lo=30, hi=400) * 0.12
    pts = rng.uniform(0, N, (220, 2))
    d1, d2, _ = torus_distance(pts)
    crack_line = (d2 - d1) + spectral(rng, 2.4, lo=6, hi=60) * 1.2
    cracky = smooth((spectral(rng, 2.0, lo=2, hi=30) - 0.2) / 1.0)
    crack = smooth((1.1 - crack_line) / 0.8) * cracky * 0.8
    # Patches fallen away, showing the bricks.
    fallen = smooth((spectral(rng, 2.8, lo=1.5, hi=25) - 2.3) / 0.4) * 0.0
    B = 64
    row = (ys // (B / 2)).astype(int)
    bx = (xs + (row % 2) * B / 2) % B
    by = ys % (B / 2)
    brick_edge = np.minimum(np.minimum(bx, B - bx), np.minimum(by, B / 2 - by))
    brick = smooth((brick_edge - 2.0) / 2.0)
    h = trowel * (1 - fallen) + (brick * 1.5 - 3.0) * fallen
    h -= crack * 1.5
    plaster = colour((0.82, 0.66, 0.46)) * (1.0 + 0.05 * spectral(rng, 1.8, lo=4, hi=300)[..., None])
    mud = colour((0.62, 0.44, 0.28)) * (1.0 + 0.10 * rng.normal(0, 1, (N, N))[..., None])
    mud = mix(mud * 0.7, mud, brick)
    base = mix(plaster, mud, fallen)
    stain = smooth((spectral(rng, 2.5, lo=1.5, hi=40) - 0.8) / 1.0)
    base = base * (1.0 - 0.12 * stain[..., None])
    base = base * (1.0 - 0.55 * crack[..., None])
    rgb = base * (lit(h, 0.8) * occlusion(h, 3.0, 0.15))[..., None]
    finish("adobe", rgb, (0.70, 0.54, 0.36))


def boards(seed, width, lengths, wood, gap_dark, name, tint, knots, finish_sheen):
    """Planks along x, `width` texels each, butt joints staggered, grain
    stretched along them with growth rings where the saw cut them, knots,
    a tone each, and nail heads at the ends."""
    rng = np.random.default_rng(seed)
    ys, xs = np.mgrid[0:N, 0:N].astype(np.float64)
    row = (ys // width).astype(int)
    rows = N // width
    plank_id = np.zeros((N, N), int)
    ex = np.zeros((N, N))
    ids = 0
    for r in range(rows):
        cuts = [rng.uniform(0, N)]
        total = 0.0
        while total < N - lengths[0]:
            step = rng.uniform(*lengths)
            if total + step > N - lengths[0] * 0.6:
                step = N - total
            total += step
            cuts.append(cuts[0] + total)
        if cuts[-1] - cuts[0] != N:
            cuts[-1] = cuts[0] + N
        sel_row = row == r
        x = xs[sel_row]
        for a, b in zip(cuts[:-1], cuts[1:]):
            u = (x - a) % N
            inside = u < (b - a)
            sel = np.zeros_like(sel_row)
            sel[sel_row] = inside
            plank_id[sel] = ids
            ex[sel] = np.minimum(u, (b - a) - u)[inside]
            ids += 1
    py = ys % width
    ey = np.minimum(py, width - 1 - py)
    # The grain: noise stretched hard along x, a different slice of it
    # for every plank, and rings that bend round where the noise does.
    offset = rng.uniform(0, 1, ids)[plank_id]
    stretch = spectral(rng, 3.0, lo=1, hi=12, aniso=(1.0, 0.5))
    fine = spectral(rng, 1.2, lo=3, hi=30, aniso=(1.0, 0.025))
    rings = np.sin(ys * 0.30 + stretch * 6.0 + offset * 40.0)
    ring_t = smooth((rings + 0.2) / 0.9)
    tones = rng.normal(0, 0.09, ids)[plank_id]
    base = wood[0] * (1.0 + tones[..., None])
    base = mix(base, wood[1] * (1.0 + tones[..., None]), ring_t * 0.75)
    base = base * (1.0 + 0.10 * fine[..., None])
    # Knots.
    for _ in range(knots):
        kx, ky = rng.uniform(0, N), (rng.integers(0, rows) + rng.uniform(0.3, 0.7)) * width
        rx, ry = rng.uniform(5, 12), rng.uniform(3, 7)
        dx = (xs - kx + N / 2) % N - N / 2
        dy = (ys - ky + N / 2) % N - N / 2
        r = np.sqrt((dx / rx) ** 2 + (dy / ry) ** 2)
        swirl = smooth((3.0 - r) / 2.0)
        base = base * (1.0 - 0.45 * smooth((1.2 - r) / 0.5)[..., None])
        base = base * (1.0 - 0.15 * swirl[..., None] * np.sin(r * 4.0)[..., None] ** 2)
    # Ends and sides worn a little rounder, the gap between dark.
    h = smooth((np.minimum(ey, ex) - 1.0) / 3.0) * 2.0 + fine * 0.04 + rings * 0.05
    h += tones * 3.0
    gap = np.minimum(ey, ex) < 1.0
    # Nails at each end of each plank.
    for nail_x in (8.0,):
        near_end = np.abs(ex - nail_x) < 2.6
        for frac in (0.3, 0.7):
            on = near_end & (np.abs(py - width * frac) < 2.6)
            base[on] = base[on] * 0.45
            h[on] += 0.6
    sheen = 1.0 + finish_sheen * spectral(rng, 2.0, lo=2, hi=40)[..., None]
    rgb = base * sheen * (lit(h, 1.0) * occlusion(h, 2.0, 0.3))[..., None]
    rgb[gap] *= gap_dark
    finish(name, rgb, tint)


def timber():
    boards(
        81,
        width=128,
        lengths=(300, 700),
        wood=(colour((0.50, 0.35, 0.22)), colour((0.38, 0.25, 0.15))),
        gap_dark=0.25,
        name="timber",
        tint=(0.44, 0.30, 0.19),
        knots=26,
        finish_sheen=0.03,
    )


def floorboard():
    boards(
        91,
        width=64,
        lengths=(200, 520),
        wood=(colour((0.62, 0.46, 0.30)), colour((0.50, 0.35, 0.21))),
        gap_dark=0.35,
        name="floorboard",
        tint=(0.36, 0.28, 0.20),
        knots=40,
        finish_sheen=0.05,
    )


def concrete():
    """The landing pad: concrete slabs two tiles square (512 texels) with
    saw-cut joints, the aggregate showing where it has worn, hairline
    cracks, oil and scorch where ships stand."""
    rng = np.random.default_rng(101)
    P = 512
    ys, xs = np.mgrid[0:N, 0:N].astype(np.float64)
    px, py = xs % P, ys % P
    slab = (ys // P).astype(int) * 2 + (xs // P).astype(int)
    edge = np.minimum(np.minimum(px, P - 1 - px), np.minimum(py, P - 1 - py))
    tone = rng.normal(0, 0.04, 4)[slab]
    base = colour((0.62, 0.62, 0.60)) * (1.0 + tone[..., None])
    base = base * (1.0 + 0.07 * spectral(rng, 1.8, lo=3, hi=300)[..., None])
    # Aggregate: stones in the surface, worn through in places.
    pts = rng.uniform(0, N, (14000, 2))
    d1, d2, which = torus_distance(pts)
    radii = rng.uniform(1.0, 4.0, len(pts))
    stone_here = d1 < radii[which]
    worn = smooth((spectral(rng, 2.0, lo=2, hi=60) + 0.2) / 1.2)
    stone_tone = rng.uniform(0.75, 1.25, len(pts))[which]
    agg = stone_here & (rng.random((N, N)) < 1.0)
    base = np.where(
        agg[..., None],
        mix(base, base * stone_tone[..., None], worn[..., None] * 0.6),
        base,
    )
    base *= 1.0 + 0.05 * rng.normal(0, 1, (N, N))[..., None]
    # Cracks.
    cpts = rng.uniform(0, N, (40, 2))
    c1, c2, _ = torus_distance(cpts)
    crack = smooth((1.0 - (c2 - c1) - spectral(rng, 1.4, lo=10, hi=300) * 1.2) / 0.8)
    crack *= smooth((spectral(rng, 2.0, lo=2, hi=20) - 0.5) / 0.6)
    # Oil and scorch.
    oil = smooth((spectral(rng, 2.6, lo=2, hi=50) - 1.4) / 0.8)
    scorch = smooth((spectral(rng, 3.0, lo=1, hi=20) - 1.0) / 1.2)
    base = base * (1.0 - 0.35 * oil[..., None]) * (1.0 - 0.25 * scorch[..., None])
    h = smooth((edge - 2.5) / 2.0) * 1.5 - crack * 1.0 + stone_here * worn * 0.3
    rgb = base * (lit(h, 1.0) * occlusion(h, 2.0, 0.3))[..., None]
    rgb[edge < 2.5] *= 0.45
    rgb *= (1.0 - 0.5 * crack)[..., None]
    finish("concrete", rgb, (0.24, 0.25, 0.27))


def water():
    """A lake from above: long swells and fine wind ripples on them, the
    deep water dark and the shallows lighter, and the sky caught bright
    on the ripples that face it."""
    rng = np.random.default_rng(111)
    swell = spectral(rng, 3.8, lo=1, hi=16, aniso=(1.0, 0.55)) * 7.0
    ripple = spectral(rng, 3.0, lo=10, hi=60, aniso=(1.0, 0.45)) * 0.9
    h = swell + ripple
    gx = (np.roll(h, -1, 1) - np.roll(h, 1, 1)) * 0.5
    gy = (np.roll(h, -1, 0) - np.roll(h, 1, 0)) * 0.5
    nz = 1.0 / np.sqrt(gx * gx + gy * gy + 1.0)
    nx, ny = -gx * nz, -gy * nz
    # The sky's glare: the half vector between the eye (straight down)
    # and a sun low in the north-west.
    half = np.array([-0.35, -0.35, 1.0])
    half /= np.linalg.norm(half)
    spec = np.clip(nx * half[0] + ny * half[1] + nz * half[2], 0, 1) ** 400
    depth = smooth((spectral(rng, 3.0, lo=1, hi=10) + 1.0) / 2.0)
    deep = colour((0.10, 0.26, 0.40))
    shallow = colour((0.22, 0.48, 0.60))
    base = mix(deep, shallow, depth * 0.8)
    # The face of a ripple towards the light a shade lighter, away darker.
    tilt = (nx * -0.7 + ny * -0.7)
    base = base * (1.0 + 0.7 * tilt[..., None])
    rgb = base + spec[..., None] * colour((0.85, 0.92, 1.0)) * 0.9
    rgb = blur(rgb, 0.5)
    finish("water", rgb, (0.24, 0.44, 0.64))


def ice():
    """A frozen lake: clear blue ice with the depth showing dark under it,
    frost and snow dusted over in patches, white fracture lines and a
    scatter of bubbles caught as it froze."""
    rng = np.random.default_rng(121)
    depth = smooth((spectral(rng, 2.8, lo=1, hi=30) + 0.8) / 2.0)
    base = mix(colour((0.46, 0.62, 0.76)), colour((0.74, 0.86, 0.94)), depth)
    frost = smooth((spectral(rng, 2.4, lo=2, hi=80) - 0.5) / 1.2)
    base = mix(base, colour((0.92, 0.95, 0.98)), frost * 0.7)
    # Fractures: the edges of plates the ice cracked into, white where
    # the crack scatters the light, with a dark line down the middle.
    pts = rng.uniform(0, N, (34, 2))
    d1, d2, _ = torus_distance(pts)
    gap = (d2 - d1) + spectral(rng, 3.0, lo=4, hi=40) * 1.2
    white = smooth((3.5 - gap) / 3.0)
    dark = smooth((0.9 - gap) / 0.6)
    pts2 = rng.uniform(0, N, (160, 2))
    e1, e2, _ = torus_distance(pts2)
    fine = smooth((1.4 - ((e2 - e1) + spectral(rng, 2.2, lo=8, hi=100) * 2.0)) / 1.0)
    fine *= smooth((spectral(rng, 2.0, lo=2, hi=20) - 0.3) / 0.8)
    base = mix(base, colour((0.95, 0.97, 1.0)), np.clip(white * 0.8 + fine * 0.5, 0, 1))
    base = base * (1.0 - 0.25 * dark[..., None])
    # Bubbles.
    img = Image.new("L", (N, N), 0)
    d = ImageDraw.Draw(img)
    for _ in range(900):
        x, y = rng.uniform(0, N, 2)
        r = rng.uniform(1.0, 3.5)

        def one(dx, dy, x=x, y=y, r=r):
            d.ellipse([x - r + dx, y - r + dy, x + r + dx, y + r + dy], fill=255)

        wrapped_draw(one)
    bubbles = blur(np.asarray(img, np.float64) / 255.0, 0.6)
    base = mix(base, colour((0.96, 0.98, 1.0)), bubbles * 0.6)
    h = depth * 2.0 + frost * 1.5
    rgb = base * (0.9 + 0.1 * lit(h, 1.0))[..., None]
    finish("ice", rgb, (0.70, 0.82, 0.90))


def object_wear():
    """What every object on a deck wears over its paint (`object.png`,
    laid over a fill rather than painted as a surface): grime gathered in
    blotches, scuffs and scratches, a little dust and a faint grain. Grey
    — the object's colour is the painter's — and gentle, since it lies
    over a counter's top and a door's leaf alike. Two tiles to a repeat."""
    rng = np.random.default_rng(131)
    grime = smooth((spectral(rng, 2.6, lo=2, hi=40) + 0.3) / 1.6)
    v = 1.0 - 0.16 * grime
    v *= 1.0 + 0.05 * spectral(rng, 1.8, lo=4, hi=120)
    v *= 1.0 + 0.03 * spectral(rng, 1.0, lo=6, hi=60, aniso=(1.0, 0.08))
    img = Image.new("L", (N, N), 0)
    d = ImageDraw.Draw(img)
    for _ in range(420):
        x, y = rng.uniform(0, N, 2)
        a = rng.uniform(0, np.pi)
        length = rng.uniform(30, 160)
        x2, y2 = x + np.cos(a) * length, y + np.sin(a) * length
        val = int(rng.uniform(80, 220))
        w = int(rng.integers(3, 8))

        def one(dx, dy, x=x, y=y, x2=x2, y2=y2, val=val, w=w):
            d.line([(x + dx, y + dy), (x2 + dx, y2 + dy)], fill=val, width=w)

        wrapped_draw(one)
    scratch = blur(np.asarray(img, np.float64) / 255.0, 1.5)
    v *= 1.0 + 0.10 * scratch
    scuff = smooth((spectral(rng, 2.2, lo=4, hi=60) - 1.5) / 0.7)
    v *= 1.0 - 0.18 * scuff
    v *= 1.0 + 0.04 * rng.normal(0, 1, (N, N))
    warm = colour((1.02, 1.0, 0.97))
    rgb = v[..., None] * mix(colour((1.0, 1.0, 1.0)), warm, grime)
    finish("object", rgb, (0.5, 0.5, 0.5))


def foliage():
    """Leaves from above, for a tree's crown, a bush and a potted plant
    (`foliage.png`, laid over the crown's ellipses): thousands of leaves
    in layers, the lower ones in the shade of the upper, each lit on its
    north-west side, a vein down it, and dark gaps between the clusters.
    Two tiles to a repeat."""
    rng = np.random.default_rng(141)
    S = 2
    M = N * S
    img = Image.new("RGB", (M, M), (14, 24, 10))
    d = ImageDraw.Draw(img)
    clumps = spectral(rng, 2.6, lo=3, hi=40)
    for layer in range(4):
        light = 0.45 + 0.2 * layer
        count = 3800 if layer < 3 else 2600
        xs = rng.uniform(0, N, count)
        ys = rng.uniform(0, N, count)
        for x, y in zip(xs, ys):
            if rng.random() > smooth(0.55 + 0.45 * clumps[int(y) % N, int(x) % N] + 0.15 * layer):
                continue
            a = rng.uniform(0, np.pi)
            length = rng.uniform(26, 48)
            width = length * rng.uniform(0.38, 0.55)
            hue = rng.uniform(-1, 1)
            g = light * rng.uniform(0.8, 1.15)
            base = np.array([0.30 + 0.04 * hue, 0.50, 0.24 - 0.03 * hue]) * g
            lit = np.clip(base * 1.18, 0, 1)
            dark = base * 0.78
            ca, sa = np.cos(a), np.sin(a)

            def leaf(dx, dy, x=x, y=y, ca=ca, sa=sa, length=length, width=width, base=base, lit=lit, dark=dark):
                pts = []
                for k in range(12):
                    t = k / 12 * 2 * np.pi
                    u = np.cos(t) * length / 2
                    w2 = np.sin(t) * width / 2 * (1 - 0.35 * np.cos(t))
                    pts.append(((x + dx) * S + (u * ca - w2 * sa) * S, (y + dy) * S + (u * sa + w2 * ca) * S))
                d.polygon(pts, fill=tuple(int(c * 255) for c in dark))
                # The lit half, nudged towards the north-west.
                off = 1.4 * S
                d.polygon([(px - off, py - off) for px, py in pts], fill=tuple(int(c * 255) for c in base))
                hx, hy = (x + dx) * S - 3.0 * S, (y + dy) * S - 3.0 * S
                d.ellipse([hx - width * 0.3 * S, hy - width * 0.3 * S, hx + width * 0.3 * S, hy + width * 0.3 * S],
                          fill=tuple(int(c * 255) for c in lit))
                # The vein.
                d.line([((x + dx) * S - length / 2 * ca * S * 0.8, (y + dy) * S - length / 2 * sa * S * 0.8),
                        ((x + dx) * S + length / 2 * ca * S * 0.8, (y + dy) * S + length / 2 * sa * S * 0.8)],
                       fill=tuple(int(c * 255) for c in dark), width=S)

            if min(x, y) < 20 or max(x, y) > N - 20:
                wrapped_draw(leaf)
            else:
                leaf(0, 0)
    img = img.resize((N, N), Image.LANCZOS)
    rgb = np.asarray(img, np.float64) / 255.0
    lum = rgb.mean(-1)
    rgb = rgb * (0.75 + 0.25 * occlusion(blur(lum, 1.0) * 8.0, 6.0, 0.4))[..., None]
    finish("foliage", rgb, (0.30, 0.56, 0.30))


MAKERS = {
    "deck": deck,
    "bulkhead": bulkhead,
    "grass": grass,
    "sand": sand,
    "snow": snow,
    "stone": stone,
    "adobe": adobe,
    "timber": timber,
    "floorboard": floorboard,
    "concrete": concrete,
    "rock": rock,
    "water": water,
    "ice": ice,
    "object": object_wear,
    "foliage": foliage,
}

if __name__ == "__main__":
    for name in sys.argv[1:] or MAKERS:
        MAKERS[name]()
