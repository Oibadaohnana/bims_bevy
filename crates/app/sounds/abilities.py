#!/usr/bin/env python3
"""The classes' abilities as clips: one `.ogg` beside this script an ability.

There are no recordings of a sentry unfolding or a cloak coming up, so
these are built: the physical parts of each sound modelled one at a time
(a struck plate is a sum of damped partials at a plate's inharmonic
ratios, a motor a sawtooth whose pitch is its speed, sand a burst of
grains through a band, a spark a click through a resonance) and layered,
with a recording from `Sounds/` under the ones that have a near relative
there (the explosion under the grenade, the engine under the dropship,
the holster's leather under the soldier's kit). Everything is seeded, so
a run makes the same sound twice; like `prepare.sh`, a run re-encodes and
the Ogg serials differ, so put back the clips not meant to change.

    nix-shell -p "python3.withPackages(ps: [ps.numpy ps.scipy])" \\
        --run "python3 crates/app/sounds/abilities.py"     # wants ffmpeg

Every clip leaves here the way `prepare.sh`'s one-shots do — mono, 48 kHz,
peak at -1 dBFS, a few milliseconds' fade either end — so every level the
game plays one at is in `sound.rs`.
"""

import os
import subprocess

import numpy as np
from scipy import signal

SR = 48000
HERE = os.path.dirname(os.path.abspath(__file__))
SRC = os.path.join(HERE, "..", "..", "..", "Sounds")


# --- the parts -------------------------------------------------------------


def secs(s):
    return int(round(s * SR))


def silence(length):
    return np.zeros(secs(length))


def times(n):
    return np.arange(n) / SR


def place(out, clip, at, gain=1.0):
    """`clip` added into `out` from `at` seconds, cut at `out`'s end."""
    i = secs(at)
    n = min(len(clip), len(out) - i)
    if n > 0:
        out[i : i + n] += gain * clip[:n]
    return out


def decay(n, tau, attack=0.0005):
    """An exponential fall of time constant `tau` with a short rise."""
    t = times(n)
    env = np.exp(-t / tau)
    if attack > 0:
        env *= np.minimum(1.0, t / attack)
    return env


def band(x, lo, hi, order=2):
    sos = signal.butter(order, [lo, hi], btype="bandpass", fs=SR, output="sos")
    return signal.sosfilt(sos, x)


def low(x, f, order=2):
    sos = signal.butter(order, f, btype="lowpass", fs=SR, output="sos")
    return signal.sosfilt(sos, x)


def high(x, f, order=2):
    sos = signal.butter(order, f, btype="highpass", fs=SR, output="sos")
    return signal.sosfilt(sos, x)


def sweep_band(x, f_from, f_to, q):
    """A band-pass whose centre glides from `f_from` to `f_to`
    (exponentially) over the length of `x`: a state-variable filter, run a
    sample at a time since its tuning changes every sample."""
    n = len(x)
    f = f_from * (f_to / f_from) ** (np.arange(n) / max(1, n - 1))
    g = np.tan(np.pi * np.minimum(f, SR * 0.45) / SR)
    k = 1.0 / q
    out = np.empty(n)
    s1 = s2 = 0.0
    for i in range(n):
        gi = g[i]
        hp = (x[i] - (k + gi) * s1 - s2) / (1.0 + gi * (k + gi))
        bp = gi * hp + s1
        s1 = gi * hp + bp
        lp = gi * bp + s2
        s2 = gi * bp + lp
        out[i] = bp
    return out * k


def modes(length, partials, rng=None, strike=0.0008):
    """A struck body: damped sines `(hz, tau, amp)`, each from a random
    phase, the strike a short rise so the attack is a knock, not a click."""
    n = secs(length)
    t = times(n)
    out = np.zeros(n)
    for hz, tau, amp in partials:
        phase = 0.0 if rng is None else rng.uniform(0, 2 * np.pi)
        out += amp * np.sin(2 * np.pi * hz * t + phase) * decay(n, tau, strike)
    return out


def knock(rng, length, lo, hi, tau):
    """The contact itself: a burst of noise in a band, falling fast."""
    n = secs(length)
    return band(rng.standard_normal(n), lo, hi) * decay(n, tau, 0.0003)


def thump(length, hz_from, hz_to, tau):
    """A body landing: a low sine dropping in pitch, the chest-felt part."""
    n = secs(length)
    t = times(n)
    f = hz_to + (hz_from - hz_to) * np.exp(-t / 0.03)
    phase = 2 * np.pi * np.cumsum(f) / SR
    return np.sin(phase) * decay(n, tau, 0.002)


def motor(length, f_from, f_to, rng, grit=0.3, teeth=0):
    """An electric motor through a gearbox: a band-limited sawtooth whose
    pitch is its speed, a little speed wobble, the gears' teeth as a buzz
    of amplitude `teeth` times the shaft's turn."""
    n = secs(length)
    t = times(n)
    ramp = np.clip(t / length, 0, 1)
    f = f_from + (f_to - f_from) * (0.5 - 0.5 * np.cos(np.pi * ramp))
    f = f * (1 + 0.012 * np.sin(2 * np.pi * 7.3 * t))
    phase = 2 * np.pi * np.cumsum(f) / SR
    out = np.zeros(n)
    for h in range(1, 16):
        out += np.sin(h * phase) / h * (1 if h % 2 else 0.6)
    if teeth:
        out *= 1 + 0.5 * np.sign(np.sin(teeth * phase))
    out += grit * band(rng.standard_normal(n), 1500, 6000) * 0.3
    return band(out, 90, 7000)


def room(x, rng, wet=0.12, tail=0.35):
    """A deck's worth of reflection: the clip convolved with a burst of
    noise falling off over `tail`, darkened as it falls, and mixed in
    under the dry sound — enough that a thing sounds set down *in* a room
    of metal, not in nothing."""
    n = secs(tail)
    t = times(n)
    ir = rng.standard_normal(n) * np.exp(-t / (tail / 5))
    ir = low(ir, 5000)
    ir[: secs(0.006)] = 0.0
    ir /= np.sqrt(np.sum(ir**2))
    wetsig = signal.fftconvolve(x, ir)[: len(x)]
    return x + wet * wetsig * (np.max(np.abs(x)) / (np.max(np.abs(wetsig)) + 1e-9))


def recording(name, start, length, rate=1.0):
    """A stretch of one of `Sounds/`, mono at 48 kHz, `rate` above one
    faster and higher (the way `asetrate` is used in `prepare.sh`)."""
    path = os.path.join(SRC, name)
    af = f"aformat=channel_layouts=mono,asetrate={int(48000 * rate)},aresample=48000"
    raw = subprocess.run(
        ["ffmpeg", "-hide_banner", "-loglevel", "error", "-i", path,
         "-ar", "48000", "-af", af, "-f", "f32le", "-"],
        check=True, capture_output=True,
    ).stdout
    x = np.frombuffer(raw, dtype=np.float32).astype(np.float64)
    a = secs(start / rate)
    out = x[a : a + secs(length)]
    return np.pad(out, (0, max(0, secs(length) - len(out))))


def save(name, x, fade_out=0.03):
    x = np.array(x, dtype=np.float64)
    x -= np.mean(x)
    x = high(x, 35)
    n = len(x)
    x[: secs(0.004)] *= np.linspace(0, 1, secs(0.004))
    fo = secs(fade_out)
    x[n - fo :] *= np.linspace(1, 0, fo)
    x *= 10 ** (-1 / 20) / np.max(np.abs(x))
    subprocess.run(
        ["ffmpeg", "-hide_banner", "-loglevel", "error", "-y",
         "-f", "f32le", "-ar", str(SR), "-ac", "1", "-i", "-",
         "-c:a", "libvorbis", "-q:a", "5", os.path.join(HERE, name + ".ogg")],
        input=x.astype(np.float32).tobytes(), check=True,
    )
    print(f"{name}.ogg  ({n / SR:.2f}s)")


# --- the soldier: blue steel -------------------------------------------------


def grenade_throw():
    """The pin drawn — a ring's scrape and tink — the spoon flying off
    with a spring's ring, and the arm's swing through the air."""
    rng = np.random.default_rng(101)
    out = silence(0.62)
    place(out, knock(rng, 0.05, 3000, 9000, 0.012), 0.0, 0.5)
    place(out, modes(0.12, [(3150, 0.02, 1), (5230, 0.014, 0.6), (7410, 0.01, 0.4)], rng), 0.035, 0.6)
    spoon = modes(0.3, [(2480, 0.07, 1), (4120, 0.045, 0.7), (6060, 0.03, 0.4), (8830, 0.02, 0.2)], rng)
    spoon *= 1 + 0.4 * np.sin(2 * np.pi * 31 * times(len(spoon)))
    place(out, spoon, 0.15, 0.55)
    swing = sweep_band(rng.standard_normal(secs(0.32)), 350, 1300, 1.4)
    swing *= np.sin(np.pi * np.linspace(0, 1, len(swing))) ** 2
    place(out, swing, 0.24, 0.9)
    save("grenade_throw", room(out, rng, 0.1), 0.06)


def grenade_burst():
    """The recorded blast, its low end pushed and its first instant a
    harder crack, with shrapnel ticking off the bulkheads after."""
    rng = np.random.default_rng(102)
    blast = recording("big_droid_explosion.mp3", 0.03, 1.9)
    blast = blast + 0.8 * low(blast, 180)
    out = silence(1.9)
    place(out, blast, 0.0, 1.0)
    place(out, knock(rng, 0.03, 800, 12000, 0.006), 0.0, 0.25 * np.max(np.abs(blast)))
    place(out, thump(0.5, 90, 38, 0.18), 0.0, 0.5 * np.max(np.abs(blast)))
    for _ in range(9):
        at = rng.uniform(0.12, 0.8)
        f = rng.uniform(2200, 5200)
        tick = modes(0.05, [(f, 0.008, 1), (f * 1.73, 0.005, 0.5)], rng)
        place(out, tick, at, rng.uniform(0.03, 0.09) * np.max(np.abs(blast)))
    save("grenade_burst", out, 0.4)


def brace():
    """Kit settling as the soldier drops into his line: the leather and
    buckles of the holster recording slowed, a boot planted and a knee
    down, and a bipod's legs locking."""
    rng = np.random.default_rng(103)
    out = silence(0.62)
    kit = recording("Unholster_and_holstering.mp3", 0.06, 0.45, 0.8)
    place(out, kit, 0.0, 0.5 / (np.max(np.abs(kit)) + 1e-9))
    place(out, thump(0.2, 110, 55, 0.05) + 0.4 * knock(rng, 0.2, 150, 900, 0.03), 0.05, 0.9)
    place(out, thump(0.2, 95, 50, 0.06) + 0.5 * knock(rng, 0.2, 200, 1400, 0.025), 0.2, 0.7)
    for at in (0.12, 0.15, 0.27, 0.31):
        place(out, knock(rng, 0.03, 2500, 8000, 0.004), at, rng.uniform(0.1, 0.2))
    lock = modes(0.18, [(1720, 0.03, 1), (2950, 0.02, 0.7), (4610, 0.012, 0.5)], rng)
    lock += 0.6 * knock(rng, 0.18, 1500, 7000, 0.003)
    place(out, lock, 0.4, 0.8)
    save("brace", room(out, rng, 0.1), 0.08)


def rampage():
    """A rifle's charging handle racked back and slammed home, and under
    it the soldier's heart kicking in: two hard beats."""
    rng = np.random.default_rng(104)
    out = silence(0.95)
    slide = sweep_band(rng.standard_normal(secs(0.07)), 1800, 3800, 2.0) * decay(secs(0.07), 0.05)
    place(out, slide, 0.0, 0.6)
    back = modes(0.12, [(1380, 0.025, 1), (2270, 0.02, 0.8), (3710, 0.012, 0.5)], rng)
    place(out, back + 0.7 * knock(rng, 0.12, 1200, 8000, 0.004), 0.065, 0.8)
    home = modes(0.2, [(980, 0.035, 1), (1650, 0.03, 0.9), (2830, 0.02, 0.6), (4400, 0.012, 0.3)], rng)
    home += knock(rng, 0.2, 700, 9000, 0.005) + 0.4 * thump(0.2, 220, 140, 0.03)
    place(out, home, 0.19, 1.0)
    for at, g in ((0.46, 1.0), (0.62, 0.75)):
        beat = thump(0.25, 70, 42, 0.07) + 0.3 * low(knock(rng, 0.25, 40, 300, 0.05), 200)
        place(out, beat, at, g)
    save("rampage", room(out, rng, 0.12), 0.12)


# --- the engineer: amber sparks ----------------------------------------------


def emp_throw():
    """The EMP armed and thrown: a toggle's click, its capacitor charging
    with the rising whine a flash gun makes, and the throw's swish."""
    rng = np.random.default_rng(201)
    out = silence(0.85)
    place(out, modes(0.05, [(2600, 0.008, 1), (4100, 0.005, 0.6)], rng) + knock(rng, 0.05, 2000, 9000, 0.002), 0.0, 0.6)
    n = secs(0.75)
    t = times(n)
    f = 1800 + 6400 * (t / t[-1]) ** 0.6
    phase = 2 * np.pi * np.cumsum(f) / SR
    whine = np.sin(phase) + 0.25 * np.sin(2 * phase) + 0.1 * np.sin(3 * phase)
    whine *= np.minimum(1, t / 0.05) * (0.25 + 0.75 * t / t[-1]) * (1 + 0.15 * np.sin(2 * np.pi * 60 * t))
    whine += 0.3 * band(rng.standard_normal(n), 3000, 9000) * (t / t[-1])
    place(out, whine, 0.03, 0.22)
    swing = sweep_band(rng.standard_normal(secs(0.3)), 400, 1400, 1.4)
    swing *= np.sin(np.pi * np.linspace(0, 1, len(swing))) ** 2
    place(out, swing, 0.45, 0.8)
    save("emp_throw", room(out, rng, 0.08), 0.1)


def emp_burst():
    """An electromagnetic pulse going off: a hard electrical crack, arcs
    crackling out of every panel in reach, and the machines' power
    collapsing — a mains buzz dropping away to nothing."""
    rng = np.random.default_rng(202)
    out = silence(1.3)
    crack = knock(rng, 0.12, 400, 14000, 0.004) + 0.8 * thump(0.12, 160, 60, 0.03)
    place(out, crack, 0.0, 1.0)
    n = secs(0.6)
    t = times(n)
    arcs = np.zeros(n)
    density = np.exp(-t / 0.18)
    clicks = rng.random(n) < 0.02 * density
    arcs[clicks] = rng.standard_normal(np.sum(clicks)) * 3
    arcs = band(arcs, 1500, 11000) + 0.3 * band(rng.standard_normal(n), 3000, 12000) * density
    place(out, arcs, 0.005, 0.35)
    n = secs(1.25)
    t = times(n)
    f = 40 + 80 * np.exp(-t / 0.35)
    phase = 2 * np.pi * np.cumsum(f) / SR
    buzz = sum(np.sin(h * phase) / h for h in range(1, 30))
    buzz = low(buzz, 2500) * np.exp(-t / 0.4) * np.minimum(1, t / 0.01)
    place(out, buzz, 0.01, 0.5)
    save("emp_burst", room(out, rng, 0.15, 0.5), 0.3)


def healing_sentry():
    """A medical unit set down and opened: the case landing, a small servo
    unfolding its arm, a latch, and the unit's two soft tones as it comes
    up."""
    rng = np.random.default_rng(203)
    out = silence(1.15)
    place(out, thump(0.2, 140, 80, 0.035) + 0.6 * knock(rng, 0.2, 300, 2500, 0.02), 0.0, 0.8)
    place(out, modes(0.1, [(1240, 0.02, 1), (2090, 0.015, 0.6)], rng), 0.0, 0.25)
    whirr = motor(0.34, 320, 520, rng, grit=0.2, teeth=0) * np.sin(np.pi * np.linspace(0, 1, secs(0.34))) ** 0.5
    place(out, whirr, 0.14, 0.3)
    latch = modes(0.1, [(2350, 0.015, 1), (3900, 0.01, 0.6)], rng) + 0.5 * knock(rng, 0.1, 2000, 8000, 0.003)
    place(out, latch, 0.5, 0.55)
    for at, hz in ((0.66, 880.0), (0.8, 1318.5)):
        n = secs(0.3 if hz > 1000 else 0.14)
        t = times(n)
        tone = (np.sin(2 * np.pi * hz * t) + 0.08 * np.sin(4 * np.pi * hz * t)) * np.minimum(1, t / 0.006) * np.exp(-t / (0.1 if hz > 1000 else 0.05))
        place(out, tone, at, 0.3)
    save("healing_sentry", room(out, rng, 0.1), 0.1)


def sandbag(rng):
    """One sack of sand dropped: the dull landing, the sand inside
    settling as a hiss of grains, the cloth rustling."""
    n = secs(0.45)
    t = times(n)
    body = thump(0.45, 95, 48, 0.06) + 0.9 * low(knock(rng, 0.45, 60, 600, 0.05), 500)
    grains = np.zeros(n)
    density = np.exp(-t / 0.09) * np.minimum(1, t / 0.01)
    hits = rng.random(n) < 0.08 * density
    grains[hits] = rng.standard_normal(np.sum(hits))
    grains = band(grains, 1200, 6500) * 2 + 0.25 * band(rng.standard_normal(n), 2000, 7000) * density
    cloth = band(rng.standard_normal(n), 500, 3000) * np.exp(-t / 0.06) * 0.25
    return body + 0.5 * grains + cloth


def sandbags():
    """Sandbags laid: two sacks hefted down, the second a beat after."""
    rng = np.random.default_rng(204)
    out = silence(0.85)
    place(out, sandbag(rng), 0.0, 1.0)
    place(out, sandbag(rng), 0.3, 0.85)
    save("sandbags", room(out, rng, 0.08), 0.1)


def clank(rng, length, root, tau, spread=1.0):
    """Heavy metal on metal: a thick part's partials, low and short."""
    ratios = [1.0, 1.59, 2.14, 2.83, 3.61, 4.45]
    parts = [(root * r * spread ** i, tau / (1 + 0.45 * i), 1 / (1 + 0.35 * i)) for i, r in enumerate(ratios)]
    return modes(length, parts, rng) + 0.7 * knock(rng, length, 300, 7000, tau * 0.12)


def sentry():
    """The sentry's tripod slammed down leg by leg, its turret motor
    spinning up and slewing, the gun locking, and the targeting unit's
    two beeps as it comes on line."""
    rng = np.random.default_rng(205)
    out = silence(1.4)
    for at, root, g in ((0.0, 410, 0.9), (0.08, 455, 0.7), (0.15, 390, 0.8)):
        place(out, clank(rng, 0.25, root, 0.06) + 0.8 * thump(0.25, 120, 60, 0.04), at, g)
    whirr = motor(0.5, 140, 330, rng, grit=0.25, teeth=12)
    whirr *= np.minimum(1, np.linspace(0, 6, len(whirr))) * np.minimum(1, np.linspace(8, 0, len(whirr)))
    place(out, whirr, 0.3, 0.35)
    place(out, clank(rng, 0.25, 780, 0.04) + 0.4 * thump(0.25, 200, 110, 0.03), 0.8, 0.8)
    for at in (0.98, 1.1):
        n = secs(0.07)
        t = times(n)
        beep = signal.square(2 * np.pi * 1850 * t, 0.5) * np.minimum(1, t / 0.003) * np.minimum(1, (t[-1] - t) / 0.005)
        place(out, low(beep, 5000), at, 0.15)
    save("sentry", room(out, rng, 0.12), 0.12)


# --- the medic: green light -------------------------------------------------


def nanite_burst():
    """A cloud of nanites released at once: the canister's hiss, and a
    swarm of tiny glittering ticks spreading out on a rising rush of
    air."""
    rng = np.random.default_rng(301)
    out = silence(1.0)
    n = secs(0.95)
    t = times(n)
    place(out, thump(0.2, 130, 70, 0.04) + 0.5 * knock(rng, 0.2, 200, 3000, 0.02), 0.0, 0.6)
    hiss = band(rng.standard_normal(n), 3000, 12000) * np.exp(-t / 0.14) * np.minimum(1, t / 0.004)
    place(out, hiss, 0.0, 0.5)
    rush = sweep_band(rng.standard_normal(n), 500, 5000, 1.2) * np.sin(np.pi * np.minimum(1, t / 0.9)) ** 1.5
    place(out, rush, 0.0, 0.8)
    glitter = np.zeros(n)
    for _ in range(140):
        at = 0.02 + 0.7 * rng.random() ** 1.5
        hz = rng.uniform(3500, 10500)
        g = modes(0.02, [(hz, 0.003, 1)], rng) * rng.uniform(0.3, 1.0) * np.exp(-at / 0.35)
        place(glitter, g, at)
    place(out, glitter, 0.0, 0.5)
    save("nanite_burst", room(out, rng, 0.1), 0.15)


def beam(on):
    """A medical beam emitter: switching on is a relay's click and a
    field's hum rising to a steady tone, a mains buzz under a clean
    sine with a slow beat; off is the same falling away."""
    rng = np.random.default_rng(302 if on else 303)
    length = 0.7 if on else 0.4
    out = silence(length)
    n = secs(length)
    t = times(n)
    place(out, modes(0.05, [(3100, 0.006, 1), (5200, 0.004, 0.5)], rng) + 0.6 * knock(rng, 0.05, 2000, 9000, 0.002), 0.0, 0.5)
    if on:
        pitch = 1 - 0.25 * np.exp(-t / 0.08)
        env = np.minimum(1, t / 0.12) * np.minimum(1, (t[-1] - t) / 0.3)
    else:
        pitch = 0.6 + 0.4 * np.exp(-t / 0.12)
        env = np.exp(-t / 0.12) * np.minimum(1, t / 0.01)
    hum_phase = 2 * np.pi * np.cumsum(100 * pitch) / SR
    hum = low(sum(np.sin(h * hum_phase) / h for h in range(1, 18)), 1600)
    tone_phase = 2 * np.pi * np.cumsum(1180 * pitch * (1 + 0.004 * np.sin(2 * np.pi * 5.5 * t))) / SR
    tone = np.sin(tone_phase) + 0.5 * np.sin(1.005 * tone_phase) + 0.15 * np.sin(2 * tone_phase)
    place(out, (0.6 * hum + 0.35 * tone) * env, 0.01, 0.8)
    save("beam_on" if on else "beam_off", room(out, rng, 0.08), 0.1)


def cloak():
    """A cloak coming up around a body: a quick shimmer of chorused tones
    bending upward through a phasing hiss, then the air settling as the
    figure goes."""
    rng = np.random.default_rng(304)
    out = silence(1.05)
    n = secs(1.0)
    t = times(n)
    env = np.minimum(1, t / 0.25) ** 2 * np.exp(-np.maximum(0, t - 0.35) / 0.18)
    swish = sweep_band(rng.standard_normal(n), 300, 7000, 3.0)
    place(out, swish * env, 0.0, 0.9)
    shimmer = np.zeros(n)
    for detune in (1.0, 1.007, 0.993, 1.5, 1.503, 2.01):
        f = 420 * detune * (1 + 2.2 * (1 - np.exp(-t / 0.35)))
        shimmer += np.sin(2 * np.pi * np.cumsum(f) / SR + rng.uniform(0, 6.3)) / detune
    shimmer *= 1 + 0.3 * np.sin(2 * np.pi * 13 * t)
    place(out, shimmer * env, 0.0, 0.18)
    place(out, band(rng.standard_normal(secs(0.5)), 2000, 8000) * decay(secs(0.5), 0.12, 0.02), 0.42, 0.25)
    save("cloak", room(out, rng, 0.2, 0.6), 0.2)


# --- the tank: red iron ------------------------------------------------------


def plate(rng, length, root, tau):
    """A steel breastplate struck: its first partials at a plate's
    ratios, ringing long."""
    ratios = [1.0, 2.23, 3.72, 5.46, 7.33, 9.51, 11.9]
    parts = [(root * r, tau / (1 + 0.3 * i), 1 / (1 + 0.5 * i)) for i, r in enumerate(ratios)]
    return modes(length, parts, rng, strike=0.0015)


def taunt():
    """A gauntlet beaten twice on the breastplate: the dull strike, and
    the armour ringing under it."""
    rng = np.random.default_rng(401)
    out = silence(0.95)
    for at, g, root in ((0.0, 0.85, 212.0), (0.2, 1.0, 208.0)):
        hit = plate(rng, 0.7, root, 0.22) + 0.9 * thump(0.7, 150, 90, 0.035) + 0.6 * knock(rng, 0.7, 250, 4000, 0.012)
        place(out, hit, at, g)
    save("taunt", room(out, rng, 0.14), 0.25)


def bulwark(on):
    """A tower shield: planted, its rim slammed into the deck plating, the
    ring of the shield and the shudder of the deck; lifted, a scrape of
    metal on metal and a light knock."""
    rng = np.random.default_rng(402 if on else 403)
    if on:
        out = silence(1.2)
        heave = recording("Force_opening_Door_and_Arilock.mp3", 0.19, 0.5, 0.85)
        place(out, low(heave, 3000), 0.0, 0.25 / (np.max(np.abs(heave)) + 1e-9))
        slam = clank(rng, 1.1, 118, 0.5, 1.02) + 1.3 * thump(1.1, 90, 40, 0.12) + 0.5 * plate(rng, 1.1, 330, 0.3)
        place(out, slam, 0.08, 1.0)
        for at in (0.2, 0.26, 0.34):
            place(out, knock(rng, 0.05, 1500, 6000, 0.006), at, 0.08)
        save("bulwark_on", room(out, rng, 0.16, 0.5), 0.3)
    else:
        out = silence(0.45)
        n = secs(0.3)
        t = times(n)
        grind = band(rng.standard_normal(n), 900, 3200) * (1 + 0.8 * rng.standard_normal(n).clip(-1, 1)) * np.sin(np.pi * t / t[-1])
        place(out, grind, 0.0, 0.35)
        place(out, clank(rng, 0.2, 350, 0.06), 0.28, 0.5)
        save("bulwark_off", room(out, rng, 0.1), 0.08)


def juggernaut():
    """Powered armour going to full: its actuators spooling up with a
    hydraulic hiss, then locking — two great clanks — and a deep impact
    as the tank sets his weight."""
    rng = np.random.default_rng(404)
    out = silence(1.45)
    spool = motor(0.7, 55, 190, rng, grit=0.4, teeth=6)
    spool *= np.minimum(1, np.linspace(0, 5, len(spool))) ** 2
    place(out, spool, 0.0, 0.35)
    n = secs(0.35)
    t = times(n)
    hiss = band(rng.standard_normal(n), 2500, 10000) * np.exp(-t / 0.12) * np.minimum(1, t / 0.01)
    place(out, hiss, 0.62, 0.35)
    place(out, clank(rng, 0.5, 170, 0.18), 0.7, 0.8)
    place(out, clank(rng, 0.7, 135, 0.25) + 1.4 * thump(0.7, 80, 34, 0.2), 0.86, 1.0)
    save("juggernaut", room(out, rng, 0.16, 0.5), 0.35)


# --- the commander: violet brass ---------------------------------------------


def whistle(rng, length, hz):
    """A pea whistle blown: the tone warbling as the pea rattles round
    the chamber — irregular, twenty-odd times a second — and breath."""
    n = secs(length)
    t = times(n)
    rattle = 28 + 5 * np.sin(2 * np.pi * 1.7 * t + rng.uniform(0, 6.3))
    rattle_phase = 2 * np.pi * np.cumsum(rattle) / SR
    am = 0.55 + 0.45 * np.sin(rattle_phase)
    fm = 1 + 0.035 * np.sin(rattle_phase + 0.7)
    phase = 2 * np.pi * np.cumsum(hz * fm) / SR
    tone = np.sin(phase) + 0.12 * np.sin(2 * phase) + 0.04 * np.sin(3 * phase)
    breath = band(rng.standard_normal(n), hz * 0.7, hz * 1.5) * 0.35
    env = np.minimum(1, t / 0.02) * np.minimum(1, (t[-1] - t) / 0.03)
    return (tone * am + breath) * env


def battle_cry():
    """The whistle for the attack: a short blast and a long one."""
    rng = np.random.default_rng(501)
    out = silence(0.9)
    place(out, whistle(rng, 0.14, 2950), 0.0, 0.85)
    place(out, whistle(rng, 0.55, 2950), 0.22, 1.0)
    save("battle_cry", room(out, rng, 0.14, 0.5), 0.12)


def rally():
    """The rally called over the squad's radio: the key's squelch, a
    burst of band-limited static carrying a two-tone call, and the
    squelch tail as the key is let go."""
    rng = np.random.default_rng(502)
    out = silence(0.72)
    radio = lambda x: band(np.tanh(2.5 * x), 350, 3200, 3)
    place(out, radio(knock(rng, 0.04, 300, 6000, 0.004) * 3), 0.0, 0.6)
    n = secs(0.5)
    t = times(n)
    static = rng.standard_normal(n) * (0.25 + 0.1 * np.sin(2 * np.pi * 9 * t))
    tones = np.where(t < 0.16, np.sin(2 * np.pi * 1200 * t), np.sin(2 * np.pi * 1600 * t)) * (t < 0.36)
    place(out, radio(0.35 * static + tones * np.minimum(1, t / 0.01)), 0.04, 0.8)
    place(out, radio(rng.standard_normal(secs(0.1)) * decay(secs(0.1), 0.03)), 0.56, 0.9)
    save("rally", out, 0.05)


def reinforcements():
    """A dropship coming in low and setting down: the recorded engine
    swelling and dropping in pitch as it passes, then boots on the deck
    as the squad jumps down."""
    rng = np.random.default_rng(503)
    out = silence(2.2)
    engine = recording("running_engine.mp3", 1.0, 1.7, 1.25)
    n = len(engine)
    t = times(n)
    engine = engine + 1.2 * low(engine, 250)
    engine *= np.sin(np.pi * np.minimum(1, t / 1.7)) ** 1.5
    whoosh = sweep_band(rng.standard_normal(n), 2500, 500, 1.1) * np.sin(np.pi * np.minimum(1, t / 1.7)) ** 3
    place(out, engine / (np.max(np.abs(engine)) + 1e-9) + 0.35 * whoosh, 0.0, 0.8)
    for at in (1.2, 1.32, 1.47, 1.55, 1.7):
        step = thump(0.2, 110, 60, 0.035) + 0.5 * knock(rng, 0.2, 200, 2500, 0.015)
        step += 0.15 * knock(rng, 0.2, 2500, 8000, 0.004)
        place(out, step, at + rng.uniform(-0.02, 0.02), rng.uniform(0.55, 0.8))
    save("reinforcements", room(out, rng, 0.12, 0.45), 0.2)


# --- the crew: a body down ---------------------------------------------------


def downed():
    """A crew member downed (no ability, but built the same way): the
    hurt cry of `ouch`, slowed and lower, a knee and then a body hitting
    the deck with the kit rattling, and over it the suit's vitals alarm,
    two falling tones twice — the part a teammate across the deck hears
    and knows someone wants picking up."""
    rng = np.random.default_rng(601)
    out = silence(1.5)
    cry = recording("own_bim_getting_hit.mp3", 2.545, 0.45, 0.85)
    cry *= np.minimum(1, (len(cry) - np.arange(len(cry))) / secs(0.12))
    place(out, cry, 0.0, 0.7 / (np.max(np.abs(cry)) + 1e-9))
    place(out, thump(0.25, 100, 50, 0.05) + 0.4 * knock(rng, 0.25, 150, 1200, 0.025), 0.22, 0.6)
    body = thump(0.4, 85, 40, 0.11) + 0.6 * knock(rng, 0.4, 120, 1500, 0.04)
    place(out, body, 0.36, 1.0)
    for _ in range(6):
        at = 0.37 + rng.uniform(0.0, 0.18)
        f = rng.uniform(1400, 3600)
        rattle = modes(0.08, [(f, 0.015, 1), (f * 1.61, 0.01, 0.5)], rng)
        place(out, rattle + 0.5 * knock(rng, 0.08, 2000, 8000, 0.004), at, rng.uniform(0.08, 0.18))
    radio = lambda x: band(np.tanh(1.8 * x), 400, 4000, 3)
    for at in (0.55, 0.95):
        for k, hz in enumerate((988, 740)):
            n = secs(0.15)
            t = times(n)
            tone = np.sin(2 * np.pi * hz * t) + 0.3 * np.sin(2 * np.pi * 2 * hz * t)
            env = np.minimum(1, t / 0.006) * np.minimum(1, (t[-1] - t) / 0.02)
            place(out, radio(tone * env), at + 0.17 * k, 0.32)
    save("downed", room(out, rng, 0.12, 0.4), 0.12)


def revived():
    """A crew member brought round (no ability either): `downed` turned
    the other way. A defibrillator's charge whining up and its zap with
    a thump into the chest, the heart kicking in — two beats — and the
    suit's vitals tones rising where `downed`'s fell, three of them,
    while the kit rattles as the body gets up off the deck."""
    rng = np.random.default_rng(602)
    out = silence(1.45)
    # The charge: a thin whine sliding up an octave and a half.
    n = secs(0.34)
    t = times(n)
    f = 1100 * (2.8 ** (t / t[-1]))
    whine = np.sin(2 * np.pi * np.cumsum(f) / SR) + 0.25 * np.sin(4 * np.pi * np.cumsum(f) / SR)
    whine *= np.minimum(1, t / 0.05) * (0.4 + 0.6 * t / t[-1])
    place(out, whine, 0.0, 0.16)
    # The zap: a crackle through a high band and a thump under it.
    zap = band(rng.standard_normal(secs(0.09)), 1800, 9000) * decay(secs(0.09), 0.02, 0.0002)
    zap *= 1 + 0.8 * np.sign(np.sin(2 * np.pi * 120 * times(len(zap))))
    place(out, zap, 0.34, 0.55)
    place(out, thump(0.3, 120, 55, 0.07) + 0.3 * knock(rng, 0.3, 120, 900, 0.02), 0.34, 0.9)
    # The heart: lub-dub, twice.
    for at in (0.62, 0.95):
        place(out, thump(0.18, 75, 45, 0.045), at, 0.75)
        place(out, thump(0.15, 90, 55, 0.035), at + 0.11, 0.5)
    # Getting up: a knee and the kit settling.
    place(out, thump(0.2, 110, 60, 0.04) + 0.4 * knock(rng, 0.2, 200, 1400, 0.02), 0.72, 0.35)
    for _ in range(5):
        at = 0.7 + rng.uniform(0.0, 0.25)
        fr = rng.uniform(1400, 3600)
        rattle = modes(0.08, [(fr, 0.015, 1), (fr * 1.61, 0.01, 0.5)], rng)
        place(out, rattle + 0.5 * knock(rng, 0.08, 2000, 8000, 0.004), at, rng.uniform(0.05, 0.12))
    # The vitals back: three rising tones on the suit's radio.
    radio = lambda x: band(np.tanh(1.8 * x), 400, 4000, 3)
    for k, hz in enumerate((740, 988, 1319)):
        n = secs(0.13 if k < 2 else 0.26)
        t = times(n)
        tone = np.sin(2 * np.pi * hz * t) + 0.3 * np.sin(2 * np.pi * 2 * hz * t)
        env = np.minimum(1, t / 0.006) * np.minimum(1, (t[-1] - t) / 0.03)
        place(out, radio(tone * env), 0.78 + 0.14 * k, 0.3)
    save("revived", room(out, rng, 0.12, 0.4), 0.12)


# --- the reloads' laser layer (October 2026) ----------------------------------


def glide(length, f_from, f_to, partials=((1, 1.0), (2, 0.25))):
    """A tone gliding exponentially from `f_from` to `f_to` over `length`,
    with a few harmonics: a capacitor's whine."""
    n = secs(length)
    t = times(n)
    f = f_from * (f_to / f_from) ** (t / max(t[-1], 1e-9))
    phase = 2 * np.pi * np.cumsum(f) / SR
    return sum(a * np.sin(h * phase) for h, a in partials)


def spark(rng, length=0.02, gain=1.0):
    """A cell's contacts meeting: a crackle high up, gone at once."""
    n = secs(length)
    return gain * band(rng.standard_normal(n), 3000, 11000) * decay(n, length / 4, 0.0002)


def ready(rng):
    """The gun's cell full: two bright pips a fifth apart and a shimmer
    after them."""
    out = silence(0.3)
    for k, hz in enumerate((2637, 3951)):
        n = secs(0.07)
        t = times(n)
        pip = np.sin(2 * np.pi * hz * t) + 0.2 * np.sin(4 * np.pi * hz * t)
        place(out, pip * np.minimum(1, t / 0.003) * np.exp(-t / 0.03), 0.055 * k, 0.5)
    n = secs(0.25)
    t = times(n)
    shimmer = np.sin(2 * np.pi * 3951 * t) * (1 + 0.5 * np.sin(2 * np.pi * 31 * t))
    place(out, shimmer * np.exp(-t / 0.07) * np.minimum(1, t / 0.01), 0.06, 0.18)
    return out + 0.3 * place(silence(0.3), spark(rng, 0.03), 0.0)


def reload_laser():
    """Laid over `reload.ogg` (a magazine's clicks), so the reload is a
    laser's and not only a rifle's: the spent cell powering down as it
    comes out with the first click, a whine charging up an octave and a
    half from the second, and the two pips of a full cell as the last
    click seats it. Timed to the recording's clicks (0.07, 0.52, 0.96 s)
    and as long as it."""
    rng = np.random.default_rng(701)
    out = silence(1.10)
    # Out: the charge left in the spent cell falling away.
    down = glide(0.26, 2200, 240, ((1, 1.0), (2, 0.3), (3, 0.12)))
    t = times(len(down))
    down *= np.minimum(1, t / 0.004) * np.exp(-t / 0.09)
    place(out, band(down, 150, 8000), 0.07, 0.55)
    place(out, spark(rng, 0.025), 0.07, 0.35)
    # In: the fresh cell's contacts, then the charge whining up, pulsing
    # quicker as it fills.
    place(out, spark(rng, 0.02), 0.52, 0.4)
    length = 0.44
    up = glide(length, 330, 2500, ((1, 1.0), (2, 0.35), (3, 0.1)))
    t = times(len(up))
    rate = 14 + 30 * t / length
    pulse = 0.75 + 0.25 * np.sin(2 * np.pi * np.cumsum(rate) / SR)
    up *= pulse * (0.25 + 0.75 * (t / length) ** 1.5) * np.minimum(1, (length - t) / 0.01)
    place(out, up, 0.52, 0.3)
    # Seated: full.
    place(out, ready(rng), 0.955, 0.8)
    save("reload_laser", room(out, rng, 0.1, 0.3), 0.06)


def shotgun_reload_laser():
    """Laid over `shotgun_reload.ogg` (six shells pushed in): every shell
    a cell slotted, a quick blip upward a step higher than the last over
    a hum that builds with them, and the last a charge whining up into a
    full cell's pips. Timed to the recording's clicks (0.21, 0.54, 1.61,
    1.85, 3.0 and 3.4 s) and as long as it."""
    rng = np.random.default_rng(702)
    out = silence(3.55)
    shells = (0.21, 0.54, 1.61, 1.85, 3.0)
    for k, at in enumerate(shells):
        f0 = 520 * 1.16**k
        blip = glide(0.09, f0, f0 * 1.9, ((1, 1.0), (2, 0.3)))
        t = times(len(blip))
        blip *= np.minimum(1, t / 0.004) * np.exp(-t / 0.035)
        place(out, blip, at, 0.45)
        place(out, spark(rng, 0.02), at, 0.3)
    # The hum: a low buzz stepping up in level and pitch at every shell.
    n = len(out)
    t = times(n)
    steps = sum((t >= at).astype(float) for at in shells)
    level = low(steps / len(shells), 12, 1)
    pitch = 110 * (1 + 0.06 * steps)
    pitch = low(pitch, 20, 1)
    phase = 2 * np.pi * np.cumsum(pitch) / SR
    hum = low(sum(np.sin(h * phase) / h for h in range(1, 12)), 1400)
    hum += 0.4 * np.sin(4 * phase) * (1 + 0.3 * np.sin(2 * np.pi * 6 * t))
    hum *= level * np.minimum(1, (t[-1] - t) / 0.15)
    place(out, hum, 0.0, 0.12)
    # The last shell: the charge up to full.
    up = glide(0.36, 600, 2600, ((1, 1.0), (2, 0.3)))
    tu = times(len(up))
    up *= (0.2 + 0.8 * (tu / tu[-1]) ** 1.5) * (0.8 + 0.2 * np.sin(2 * np.pi * np.cumsum(20 + 30 * tu) / SR))
    place(out, up, 3.04, 0.28)
    place(out, ready(rng), 3.4, 0.8)
    save("shotgun_reload_laser", room(out, rng, 0.1, 0.3), 0.06)


# --- a level gained (October 2026) --------------------------------------------


def bell(rng, length, hz, tau):
    """A small glass bell: a bright fundamental and a bell's partials over
    it, the higher falling sooner."""
    parts = [(1.0, 1.0), (2.0, 0.5), (2.76, 0.32), (4.07, 0.2), (5.4, 0.1)]
    return modes(length, [(hz * r, tau / (1 + 0.6 * k), a) for k, (r, a) in enumerate(parts)], rng, 0.001)


def level_up():
    """A level gained, after Dota 2's: a breath of air rising, a quick
    run of glass bells up an E major arpeggio, and on top a big bright
    chord that rings out over a swell of choir-like pad with sparkles
    scattered up through it — the run says *up*, the chord *arrived*."""
    rng = np.random.default_rng(801)
    out = silence(2.0)
    # The rise: air through a band gliding up, swelling into the chord.
    n = secs(0.5)
    t = times(n)
    air = sweep_band(rng.standard_normal(n), 350, 7000, 2.5)
    air *= (t / t[-1]) ** 1.6 * np.minimum(1, (t[-1] - t) / 0.03)
    place(out, air, 0.0, 0.5)
    # The run: E5 G#5 B5 E6 G#6, sixty milliseconds apart.
    for k, hz in enumerate((659.3, 830.6, 987.8, 1318.5, 1661.2)):
        place(out, bell(rng, 0.6, hz, 0.18), 0.06 * k, 0.4 + 0.06 * k)
    # Arrived: E6 B6 E7 rung long, and a low E under them for weight.
    at = 0.34
    for hz, g in ((1318.5, 1.0), (1975.5, 0.7), (2637.0, 0.5)):
        place(out, bell(rng, 1.6, hz, 0.55), at, g)
    place(out, thump(0.7, 140, 82, 0.22), at, 0.7)
    # The pad: E4 B4 E5 G#5, each three slightly detuned voices, swelling
    # in under the chord and fading out slowly.
    n = secs(1.6)
    t = times(n)
    pad = np.zeros(n)
    for hz in (329.6, 493.9, 659.3, 830.6):
        for d in (-0.006, 0.0, 0.006):
            ph = rng.uniform(0, 2 * np.pi)
            f = hz * (1 + d)
            pad += np.sin(2 * np.pi * f * t + ph) + 0.3 * np.sin(4 * np.pi * f * t + ph)
    pad = low(pad, 3500)
    pad *= np.minimum(1, t / 0.12) * np.exp(-np.maximum(0, t - 0.25) / 0.45)
    place(out, pad / np.max(np.abs(pad)), at - 0.04, 0.35)
    # Sparkles: high pips scattered up through the ring.
    for _ in range(28):
        when = at + rng.uniform(0.0, 0.9) ** 1.5
        f = rng.uniform(3000, 8500)
        pip = modes(0.12, [(f, 0.025, 1.0), (f * 1.5, 0.015, 0.3)], rng, 0.0005)
        place(out, pip, when, rng.uniform(0.05, 0.14) * (1.3 - (when - at)))
    save("level_up", room(out, rng, 0.22, 0.8), 0.2)


if __name__ == "__main__":
    import sys

    os.chdir(HERE)
    # `abilities.py downed`: only the clips named, so the rest keep their
    # bytes.
    if len(sys.argv) > 1:
        for name in sys.argv[1:]:
            globals()[name]()
        sys.exit()
    grenade_throw()
    grenade_burst()
    brace()
    rampage()
    emp_throw()
    emp_burst()
    healing_sentry()
    sandbags()
    sentry()
    nanite_burst()
    beam(True)
    beam(False)
    cloak()
    taunt()
    bulwark(True)
    bulwark(False)
    juggernaut()
    battle_cry()
    rally()
    reinforcements()
    downed()
    revived()
    reload_laser()
    shotgun_reload_laser()
    level_up()
