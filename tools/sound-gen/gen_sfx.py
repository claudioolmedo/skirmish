#!/usr/bin/env python3
"""Skirmish II — procedural sound generation (Phase 2 of the pipeline).

Renders every non-voice asset in the sound catalog from DSP recipes
(oscillators, filtered noise, Karplus-Strong, formant synthesis) and
post-processes the Kokoro voice clips from gen_voices.py. Everything
is original synthesis — no samples of any kind. Seeded throughout:
the same code always renders the same bytes.

Run with the system python3 (numpy + scipy):
    python3 gen_sfx.py
Outputs: web-preview/play/audio/*.wav (22050 Hz mono 16-bit)
         web-preview/play/audio/manifest.json
"""
import json
import os
import sys

import numpy as np
from scipy import signal
from scipy.io import wavfile

SR = 22050
HERE = os.path.dirname(os.path.abspath(__file__))
REPO = os.path.abspath(os.path.join(HERE, "..", ".."))
ASSETS = os.path.join(REPO, "web-preview", "play", "audio")
VOICE_WORK = os.path.join(HERE, "work", "voice")

# ------------------------------------------------------------------
# primitives
# ------------------------------------------------------------------

def t_axis(dur):
    return np.arange(int(dur * SR)) / SR


def save(name, x, peak=0.89):
    x = np.asarray(x, dtype=np.float64)
    x = x - np.mean(x[: min(len(x), 64)]) if len(x) else x
    m = np.max(np.abs(x)) if len(x) else 0.0
    if m > 0:
        x = x * (peak / m)
    wavfile.write(os.path.join(ASSETS, name), SR,
                  (x * 32767).astype(np.int16))
    return name


def noise(rng, dur):
    return rng.standard_normal(int(dur * SR))


def _bp_sos(freq, q):
    bw = max(40.0, freq / max(q, 0.2))
    lo = max(15.0, freq - bw / 2)
    hi = min(SR / 2 - 15.0, freq + bw / 2)
    return signal.butter(2, [lo, hi], btype="bandpass", fs=SR, output="sos")


def filt(x, kind, freq, q=0.7, order=2):
    def one(seg, f):
        if kind == "bandpass":
            return signal.sosfilt(_bp_sos(f, q), seg)
        sos = signal.butter(order, f, btype=kind, fs=SR, output="sos")
        return signal.sosfilt(sos, seg)

    if np.isscalar(freq):
        return one(x, freq)
    # time-varying cutoff: apply in 4 chunks (cheap sweep)
    chunks = np.array_split(x, 4)
    out = []
    for i, c in enumerate(chunks):
        f = freq[0] + (freq[1] - freq[0]) * (i / 3)
        out.append(one(c, f))
    return np.concatenate(out)


def bp(x, freq, q=1.0):
    return signal.sosfilt(_bp_sos(freq, q), x)


def env_exp(dur, tau, attack=0.004):
    t = t_axis(dur)
    e = np.exp(-t / tau)
    a = int(attack * SR)
    if a > 0:
        e[:a] *= np.linspace(0, 1, a)
    return e


def env_ad(dur, attack, decay_tau):
    t = t_axis(dur)
    e = np.exp(-np.maximum(0, t - attack) / decay_tau)
    a = int(attack * SR)
    if a > 0:
        e[:a] *= np.linspace(0, 1, a)
    return e


def sine(f, dur):
    return np.sin(2 * np.pi * f * t_axis(dur))


def glide(f0, f1, dur, curve=1.0):
    """A pitch envelope from f0 to f1 (exponential-ish)."""
    t = t_axis(dur)
    frac = (t / dur) ** curve
    return f0 * (f1 / f0) ** frac


def saw_from_f0(f0):
    phase = np.cumsum(f0) / SR
    return 2 * (phase - np.floor(phase + 0.5))


def formant_voice(f0, dur, formants, tremolo_hz=0.0, tremolo_depth=0.0,
                  breath=None, rough_hz=0.0):
    """A creature/voice-like sound: sawtooth source through parallel
    bandpass formants, with optional tremolo (bleats) or fast rough
    amplitude modulation (growls)."""
    src = saw_from_f0(f0)
    t = t_axis(dur)
    if tremolo_hz:
        src *= 1 - tremolo_depth * (0.5 + 0.5 * np.sin(2 * np.pi * tremolo_hz * t))
    if rough_hz:
        src *= 0.6 + 0.4 * np.sign(np.sin(2 * np.pi * rough_hz * t)) \
            * (0.5 + 0.5 * np.sin(2 * np.pi * rough_hz * 0.5 * t))
    out = 0.25 * filt(src, "lowpass", 2600)
    for f, g in formants:
        out += g * bp(src, f, q=5.0)
    if breath is not None:
        out += breath
    return out


def metallic(partials, dur, tau, rng, noise_bp=None, noise_gain=0.4):
    t = t_axis(dur)
    x = np.zeros_like(t)
    for f, g in partials:
        x += g * np.sin(2 * np.pi * f * t) * np.exp(-t / (tau * (0.7 + 900 / f)))
    if noise_bp:
        x += noise_gain * bp(noise(rng, dur), noise_bp, q=0.8) * env_exp(dur, 0.03)
    return x


def knock(f0, f1, dur, rng, noise_freq=800, noise_gain=0.7, tau=None):
    """A woody knock: pitch-dropping sine body + a short noise click."""
    t = t_axis(dur)
    f = glide(f0, f1, dur)
    phase = np.cumsum(f) / SR
    body = np.sin(2 * np.pi * phase) * env_exp(dur, tau or dur / 3)
    click = filt(noise(rng, dur), "lowpass", noise_freq) * env_exp(dur, 0.012)
    return body + noise_gain * click


def mix(*xs):
    n = max(len(x) for x in xs)
    out = np.zeros(n)
    for x in xs:
        out[:len(x)] += x
    return out


def place_at(base, clip, at_s):
    at = int(at_s * SR)
    if at + len(clip) > len(base):
        base = np.pad(base, (0, at + len(clip) - len(base)))
    base[at:at + len(clip)] += clip
    return base


def horn(freq, dur, lp=950, sag=0.02):
    """A war-horn note: sawtooth stack, slow swell, slight end sag."""
    t = t_axis(dur)
    f = freq * (1 - sag * (t / dur) ** 2)
    src = saw_from_f0(f)
    for mult, g in [(2, 0.45), (3, 0.22), (4, 0.1)]:
        src += g * saw_from_f0(f * mult)
    swell = np.minimum(1, t / 0.09) * np.exp(-np.maximum(0, t - dur * 0.55) / (dur * 0.4))
    return filt(src * swell, "lowpass", lp)


def bell(freq, dur, strokes=(0.0,), bright=1.0):
    t = t_axis(dur)
    x = np.zeros_like(t)
    for s in strokes:
        for ratio, g, tau in [(1.0, 1.0, 0.55), (2.02, 0.55, 0.34),
                              (2.74, 0.32, 0.2), (3.76, 0.18, 0.11)]:
            seg = np.sin(2 * np.pi * freq * ratio * t) * np.exp(-t / tau) * g * bright
            x = place_at(x, seg, s)
    return x


def karplus(freq, dur, rng, damp=0.996):
    n = int(SR / freq)
    buf = rng.standard_normal(n)
    out = np.zeros(int(dur * SR))
    idx = 0
    for i in range(len(out)):
        cur = buf[idx]
        out[i] = cur
        buf[idx] = damp * 0.5 * (cur + buf[(idx + 1) % n])
        idx = (idx + 1) % n
    return out * env_exp(dur, dur / 1.6)


# ------------------------------------------------------------------
# recipes — each returns {filename: samples}
# ------------------------------------------------------------------

def r_sword(seed):
    rng = np.random.RandomState(seed)
    out = {}
    for v in range(3):
        r = np.random.RandomState(seed + v * 17)
        base = 2400 + r.rand() * 500
        x = metallic([(base, 1.0), (base * 1.34, 0.7), (base * 1.79, 0.5),
                      (base * 2.41, 0.3)], 0.3, 0.055, r, noise_bp=4200,
                     noise_gain=0.5)
        x = place_at(x, 0.3 * sine(310 + r.rand() * 60, 0.09) * env_exp(0.09, 0.03), 0.0)
        out[f"hit.sword-{v + 1}.wav"] = x
    return out


def r_knife(seed):
    out = {}
    for v in range(2):
        r = np.random.RandomState(seed + v * 23)
        base = 3300 + r.rand() * 600
        x = metallic([(base, 1.0), (base * 1.42, 0.6), (base * 2.1, 0.3)],
                     0.16, 0.03, r, noise_bp=5200, noise_gain=0.4)
        out[f"hit.knife-{v + 1}.wav"] = x
    return out


def r_spear(seed):
    out = {}
    for v in range(2):
        r = np.random.RandomState(seed + v * 31)
        dur = 0.22
        swish = filt(noise(r, dur), "bandpass", [1400, 500]) * env_ad(dur, 0.05, 0.05)
        thud = knock(210, 130, 0.12, r, noise_freq=600, noise_gain=0.5)
        x = place_at(swish * 0.7, thud, 0.07)
        out[f"hit.spear-{v + 1}.wav"] = x
    return out


def r_bow(seed):
    out = {}
    for v in range(2):
        r = np.random.RandomState(seed + v * 13)
        dur = 0.3
        twang = karplus(150 + r.rand() * 30, 0.16, r)
        whoosh = filt(noise(r, dur), "bandpass", [600, 2600]) * env_ad(dur, 0.03, 0.07)
        x = place_at(whoosh * 0.8, twang, 0.0)
        hit = knock(320, 190, 0.08, r, noise_freq=900, noise_gain=0.5)
        xh = place_at(np.zeros(int(0.14 * SR)), hit, 0.02)
        out[f"bow.release-{v + 1}.wav"] = x
        out[f"arrow.hit-{v + 1}.wav"] = xh
    return out


def r_building_hit(seed):
    out = {}
    for v in range(2):
        r = np.random.RandomState(seed + v * 41)
        x = knock(120, 75, 0.2, r, noise_freq=420, noise_gain=0.9, tau=0.06)
        x += 0.5 * filt(noise(r, 0.2), "lowpass", 300) * env_exp(0.2, 0.05)
        out[f"hit.building-{v + 1}.wav"] = x
    return out


def r_chop(seed):
    out = {}
    for v in range(3):
        r = np.random.RandomState(seed + v * 19)
        x = knock(230 + r.rand() * 50, 140, 0.13, r, noise_freq=1050,
                  noise_gain=0.85, tau=0.035)
        # a woody crack layered on the knock
        crack = bp(noise(r, 0.06), 750 + r.rand() * 300, q=1.2) * env_exp(0.06, 0.014)
        x = place_at(x, crack * 0.8, 0.004)
        out[f"work.chop-{v + 1}.wav"] = x
    return out


def r_hammer(seed):
    out = {}
    for v in range(3):
        r = np.random.RandomState(seed + v * 29)
        x = knock(300 + r.rand() * 70, 190, 0.08, r, noise_freq=1500,
                  noise_gain=0.7, tau=0.02)
        out[f"work.hammer-{v + 1}.wav"] = x
    return out


def r_mines(seed):
    out = {}
    for v in range(2):
        r = np.random.RandomState(seed + v * 7)
        base = 5000 + r.rand() * 700
        x = metallic([(base, 1.0), (base * 1.46, 0.55), (base * 2.13, 0.3)],
                     0.13, 0.03, r)
        tick = filt(noise(r, 0.05), "highpass", 1800) * env_exp(0.05, 0.01)
        out[f"work.minegold-{v + 1}.wav"] = place_at(x, tick * 0.6, 0.002)
        r2 = np.random.RandomState(seed + 500 + v * 7)
        base2 = 1750 + r2.rand() * 250
        y = metallic([(base2, 1.0), (base2 * 1.38, 0.5)], 0.11, 0.028, r2)
        thud = filt(noise(r2, 0.09), "lowpass", 700) * env_exp(0.09, 0.025)
        out[f"work.minestone-{v + 1}.wav"] = y + place_at(np.zeros(len(y)), thud * 0.7, 0.0)[:len(y)]
    return out


def r_farm(seed):
    out = {}
    for v in range(2):
        r = np.random.RandomState(seed + v * 11)
        dur = 0.3
        x = filt(noise(r, dur), "bandpass", [480, 1500]) * env_ad(dur, 0.07, 0.09)
        x += 0.3 * filt(noise(r, dur), "highpass", 3000) * env_ad(dur, 0.1, 0.08)
        out[f"work.farm-{v + 1}.wav"] = x
    return out


def r_treefall(seed):
    r = np.random.RandomState(seed)
    dur = 1.35
    t = t_axis(dur)
    # creak: a wooden groan gliding down, wobbling
    f = glide(230, 65, 0.55) * (1 + 0.06 * np.sin(2 * np.pi * 7 * t_axis(0.55)))
    creak = saw_from_f0(f) * env_ad(0.55, 0.1, 0.22)
    creak = filt(creak, "lowpass", 700)
    x = place_at(np.zeros(int(dur * SR)), creak * 0.7, 0.0)
    # crash: foliage noise swell + canopy hiss + ground thump
    crash = filt(noise(r, 0.5), "lowpass", 900) * env_ad(0.5, 0.05, 0.16)
    x = place_at(x, crash, 0.5)
    leaves = filt(noise(r, 0.6), "highpass", 2600) * env_ad(0.6, 0.1, 0.2)
    x = place_at(x, leaves * 0.5, 0.55)
    thump = sine(58, 0.3) * env_exp(0.3, 0.09)
    x = place_at(x, thump, 0.62)
    return {"work.treefall-1.wav": x}


def r_dropoff(seed):
    r = np.random.RandomState(seed)
    x = knock(180, 120, 0.12, r, noise_freq=700, noise_gain=0.45, tau=0.04)
    for dt in (0.05, 0.1, 0.16):
        tick = filt(noise(r, 0.03), "highpass", 2400) * env_exp(0.03, 0.008)
        x = place_at(x, tick * (0.35 + 0.2 * r.rand()), dt)
    return {"work.dropoff-1.wav": x}


def r_place(seed):
    r = np.random.RandomState(seed)
    x = knock(165, 105, 0.14, r, noise_freq=760, noise_gain=0.8, tau=0.045)
    x = place_at(x, knock(150, 95, 0.13, r, noise_freq=700, noise_gain=0.7, tau=0.04), 0.12)
    return {"build.place-1.wav": x}


def r_destroy(seed):
    r = np.random.RandomState(seed)
    dur = 1.15
    x = filt(noise(r, dur), "lowpass", [1100, 240]) * env_ad(dur, 0.02, 0.3)
    for i in range(7):  # timber cracks scattered through the fall
        at = 0.03 + r.rand() * 0.45
        crack = bp(noise(r, 0.05), 500 + r.rand() * 900, q=1.4) * env_exp(0.05, 0.012)
        x = place_at(x, crack * (0.9 - i * 0.08), at)
    boom = sine(52, 0.5) * env_exp(0.5, 0.16)
    x = place_at(x, boom, 0.1)
    return {"build.destroy-1.wav": x}


def r_horns(seed):
    return {
        "alert.horn-1.wav": horn(196, 0.8, lp=900),
        "build.research-1.wav": horn(294, 0.42, lp=1100),
        "alert.bell-1.wav": bell(660, 1.15, strokes=(0.0, 0.38)),
        "build.complete-1.wav": mix(bell(1318, 0.55, bright=0.5),
                                    0.4 * bell(1760, 0.4, bright=0.4)),
    }


def r_stingers(seed):
    def motif(notes, step, note_dur, lp, base_gain=1.0):
        total = step * (len(notes) - 1) + note_dur + 0.25
        x = np.zeros(int(total * SR))
        for i, f in enumerate(notes):
            x = place_at(x, horn(f, note_dur, lp=lp) * base_gain, i * step)
        return x

    age = motif([262, 330, 392, 523], 0.13, 0.34, 1150)
    r = np.random.RandomState(seed)
    drum = mix(sine(70, 0.3) * env_exp(0.3, 0.1),
               0.4 * filt(noise(r, 0.2), "lowpass", 300) * env_exp(0.2, 0.06))
    age = place_at(age, drum, 0.0)
    vic = motif([392, 523, 659, 784], 0.14, 0.4, 1250)
    vic = place_at(vic, horn(784, 0.55, lp=1250) * 0.8, 0.56)
    defe = motif([220, 175, 147, 110], 0.19, 0.42, 800)
    return {
        "sting.ageup-1.wav": age,
        "sting.victory-1.wav": vic,
        "sting.defeat-1.wav": defe,
    }


def r_ui(seed):
    r = np.random.RandomState(seed)
    coin = metallic([(5560, 1.0), (7890, 0.6), (10200, 0.3)], 0.1, 0.025, r)
    coin = place_at(coin, metallic([(6100, 0.9), (8600, 0.5)], 0.09, 0.022, r), 0.075)
    err = knock(115, 80, 0.13, r, noise_freq=380, noise_gain=0.55, tau=0.045)
    err = place_at(err, 0.5 * filt(sine(98, 0.14), "lowpass", 300) * env_exp(0.14, 0.05), 0.0)
    ready = place_at(np.zeros(int(0.22 * SR)), sine(1318, 0.07) * env_exp(0.07, 0.02), 0.0)
    ready = place_at(ready, sine(1760, 0.09) * env_exp(0.09, 0.03), 0.08)
    housed = (0.8 * filt(sine(196, 0.16), "lowpass", 500) * env_ad(0.16, 0.01, 0.06))
    housed = place_at(housed, 0.8 * filt(sine(165, 0.2), "lowpass", 500) * env_ad(0.2, 0.01, 0.07), 0.19)
    return {
        "ui.coin-1.wav": coin,
        "ui.error-1.wav": err,
        "ui.ready-1.wav": ready,
        "housed.warn-1.wav": housed,
    }


def r_animals(seed):
    out = {}
    # sheep bleat: mid saw, strong warble (tremolo), nasal formants
    for v in range(2):
        dur = 0.62
        t = t_axis(dur)
        f0 = glide(640 - v * 40, 560 - v * 30, dur) * (1 + 0.03 * np.sin(2 * np.pi * 9 * t))
        x = formant_voice(f0, dur, [(880, 0.9), (1350, 0.55), (2400, 0.2)],
                          tremolo_hz=13, tremolo_depth=0.55)
        out[f"sheep.baa-{v + 1}.wav"] = x * env_ad(dur, 0.03, 0.24)
    # sheep death: the bleat cut and falling
    dur = 0.42
    f0 = glide(700, 330, dur)
    x = formant_voice(f0, dur, [(880, 0.9), (1350, 0.5)], tremolo_hz=11, tremolo_depth=0.5)
    out["sheep.death-1.wav"] = x * env_ad(dur, 0.02, 0.13)
    # boar grunt: rough low bursts; death: a squeal up then down
    r = np.random.RandomState(seed)
    g = np.zeros(int(0.4 * SR))
    for at in (0.0, 0.13, 0.26):
        seg = formant_voice(glide(150, 110, 0.09), 0.09, [(420, 0.8), (760, 0.4)],
                            rough_hz=42)
        g = place_at(g, seg * env_exp(0.09, 0.035), at)
        snort = filt(noise(r, 0.06), "lowpass", 900) * env_exp(0.06, 0.02)
        g = place_at(g, snort * 0.7, at + 0.02)
    out["boar.grunt-1.wav"] = g
    dur = 0.55
    f0 = glide(880, 1650, 0.22)
    f0 = np.concatenate([f0, glide(1650, 480, dur - 0.22)])
    x = formant_voice(f0, dur, [(1250, 0.8), (2100, 0.45)],
                      tremolo_hz=24, tremolo_depth=0.25)
    out["boar.death-1.wav"] = x * env_ad(dur, 0.02, 0.2)
    # deer death: a soft cry falling
    dur = 0.4
    f0 = glide(610, 320, dur)
    x = formant_voice(f0, dur, [(950, 0.7), (1500, 0.35)], tremolo_hz=8, tremolo_depth=0.2)
    out["deer.death-1.wav"] = (x * 0.8 + 0.3 * sine(0, dur)) * env_ad(dur, 0.02, 0.15)
    # wolf growl + death yelp
    dur = 0.55
    f0 = glide(105, 88, dur)
    x = formant_voice(f0, dur, [(380, 0.85), (650, 0.4)], rough_hz=31)
    out["wolf.growl-1.wav"] = x * env_ad(dur, 0.04, 0.2)
    dur = 0.48
    t = t_axis(dur)
    f0 = glide(870, 290, dur) * (1 + 0.04 * np.sin(2 * np.pi * 8 * t))
    x = formant_voice(f0, dur, [(1050, 0.8), (1700, 0.4)])
    out["wolf.death-1.wav"] = x * env_ad(dur, 0.02, 0.16)
    return out


def r_horse(seed):
    out = {}
    for v in range(2):
        dur = 0.95
        t = t_axis(dur)
        # whinny: high start, pulsed amplitude (the classic "hee-hee"
        # pulses), vibrato, pitch sagging to ~2/3 by the end
        base = glide(1150 - v * 60, 720 - v * 40, dur, curve=0.8)
        f0 = base * (1 + 0.045 * np.sin(2 * np.pi * 11 * t))
        x = formant_voice(f0, dur, [(1080, 0.85), (1750, 0.5), (2600, 0.22)],
                          tremolo_hz=5.2, tremolo_depth=0.5)
        r = np.random.RandomState(seed + v)
        breath = bp(noise(r, dur), 1200, q=0.7) * env_ad(dur, 0.1, 0.3)
        out[f"horse.neigh-{v + 1}.wav"] = (x + 0.25 * breath) * env_ad(dur, 0.04, 0.34)
    # hoofbeats: a four-beat gallop of dirt thumps
    r = np.random.RandomState(seed + 99)
    x = np.zeros(int(0.85 * SR))
    for at, g in [(0.0, 0.8), (0.13, 1.0), (0.27, 0.85), (0.44, 0.95), (0.62, 0.6)]:
        thump = mix(filt(noise(r, 0.07), "lowpass", 260) * env_exp(0.07, 0.02),
                    sine(88, 0.06) * env_exp(0.06, 0.018))
        x = place_at(x, thump * g, at)
    out["horse.hooves-1.wav"] = x
    # cavalry death: a neigh cut short into a heavy fall
    dur = 0.7
    f0 = glide(1040, 430, 0.3)
    frag = formant_voice(f0, 0.3, [(1080, 0.85), (1750, 0.45)],
                         tremolo_hz=11, tremolo_depth=0.3)
    x = place_at(np.zeros(int(dur * SR)), frag * env_ad(0.3, 0.02, 0.1), 0.0)
    thud = mix(sine(55, 0.35) * env_exp(0.35, 0.11),
               filt(noise(r, 0.25), "lowpass", 320) * env_exp(0.25, 0.07))
    x = place_at(x, thud, 0.27)
    out["death.cavalry-1.wav"] = x
    return out


def r_death_human(seed):
    out = {}
    for v in range(2):
        dur = 0.42
        f0 = glide(215 - v * 25, 82, 0.17)
        grunt = formant_voice(f0, 0.17, [(480, 0.85), (880, 0.45)], rough_hz=24)
        x = place_at(np.zeros(int(dur * SR)), grunt * env_ad(0.17, 0.008, 0.06), 0.0)
        r = np.random.RandomState(seed + v * 3)
        fall = mix(filt(noise(r, 0.2), "lowpass", 300) * env_exp(0.2, 0.06),
                   sine(64, 0.18) * env_exp(0.18, 0.05))
        x = place_at(x, fall * 0.85, 0.15)
        out[f"death.human-{v + 1}.wav"] = x
    return out


def r_ambient(seed):
    out = {}
    for v in range(3):
        r = np.random.RandomState(seed + v * 53)
        dur = 0.7 + 0.15 * v
        t = t_axis(dur)
        x = np.zeros(int(dur * SR))
        # 2-4 quick FM chirps, bird-like and faint
        n_ch = 2 + v
        for c in range(n_ch):
            at = 0.05 + c * (0.14 + 0.05 * r.rand())
            f = glide(2900 + 500 * r.rand(), 3900 + 600 * r.rand(), 0.055)
            chirp = np.sin(2 * np.pi * np.cumsum(f) / SR) * env_exp(0.055, 0.016)
            x = place_at(x, chirp * 0.5, at)
        out[f"ambient.bird-{v + 1}.wav"] = x * env_ad(dur, 0.02, dur / 2)
    # wind: a slow breathing of filtered noise, edges faded (4 s)
    r = np.random.RandomState(seed + 7)
    dur = 4.0
    t = t_axis(dur)
    x = filt(noise(r, dur), "bandpass", [320, 640])
    lfo = 0.55 + 0.45 * np.sin(2 * np.pi * 0.11 * t + 1.2) * np.sin(2 * np.pi * 0.043 * t)
    fade = np.minimum(1, np.minimum(t / 0.8, (dur - t) / 0.8))
    out["ambient.wind-1.wav"] = x * lfo * fade * 0.7
    return out


RECIPES = [
    ("hit.sword", r_sword, 1101),
    ("hit.knife", r_knife, 1202),
    ("hit.spear", r_spear, 1303),
    ("bow", r_bow, 1404),           # emits bow.release-* and arrow.hit-*
    ("hit.building", r_building_hit, 1505),
    ("work.chop", r_chop, 1606),
    ("work.hammer", r_hammer, 1707),
    ("work.mines", r_mines, 1808),  # emits minegold + minestone
    ("work.farm", r_farm, 1909),
    ("work.treefall", r_treefall, 2010),
    ("work.dropoff", r_dropoff, 2111),
    ("build.place", r_place, 2212),
    ("build.destroy", r_destroy, 2313),
    ("horns", r_horns, 2414),       # alert.horn, build.research, alert.bell, build.complete
    ("stings", r_stingers, 2515),   # sting.ageup / victory / defeat
    ("ui", r_ui, 2616),             # ui.coin, ui.error, ui.ready, housed.warn
    ("animals", r_animals, 2717),   # sheep/boar/deer/wolf
    ("horse", r_horse, 2818),       # horse.neigh, horse.hooves, death.cavalry
    ("death.human", r_death_human, 2919),
    ("ambient", r_ambient, 3020),   # ambient.bird, ambient.wind
]

# Legacy cue name -> generated sound key (engine tries the asset
# first, the runtime synth recipe second).
CUE_SOUNDS = {
    "ageAdvance": "sting.ageup",
    "victory": "sting.victory",
    "defeat": "sting.defeat",
    "researchComplete": "build.research",
    "constructionComplete": "build.complete",
    "buildingPlaced": "build.place",
    "housed": "housed.warn",
    "villagerTrained": "ui.ready",
    "trained": "ui.ready",
    "combat": "hit.sword",
    "work": "work.chop",
}


def postprocess_voice(src_path):
    """24 kHz Kokoro float -> trimmed, gently band-limited 22050 Hz."""
    sr_in, data = wavfile.read(src_path)
    x = data.astype(np.float64)
    if x.ndim > 1:
        x = x[:, 0]
    if sr_in != SR:
        x = signal.resample_poly(x, SR, sr_in)
    peak = np.max(np.abs(x)) or 1.0
    idx = np.nonzero(np.abs(x) > 0.04 * peak)[0]
    if len(idx):
        a = max(0, idx[0] - int(0.03 * SR))
        b = min(len(x), idx[-1] + int(0.05 * SR))
        x = x[a:b]
    x = filt(x, "highpass", 70)
    x = filt(x, "lowpass", 7600)
    fade = int(0.006 * SR)
    if len(x) > 2 * fade:
        x[:fade] *= np.linspace(0, 1, fade)
        x[-fade:] *= np.linspace(1, 0, fade)
    return x


def main() -> int:
    os.makedirs(ASSETS, exist_ok=True)
    sounds = {}

    def register(fname, samples, peak=0.89):
        save(fname, samples, peak=peak)
        key = fname.rsplit("-", 1)[0] if "-" in fname else fname[:-4]
        sounds.setdefault(key, []).append(fname)

    for _name, fn, seed in RECIPES:
        for fname, samples in fn(seed).items():
            register(fname, samples)

    # voices: post-process the Kokoro renders into the asset set
    voices_meta = {}
    meta_path = os.path.join(HERE, "work", "voices.json")
    if not os.path.exists(meta_path):
        print("gen_voices.py has not run — no voice assets", file=sys.stderr)
        return 1
    with open(meta_path) as f:
        voices_meta = json.load(f)
    for key, info in voices_meta.items():
        for fname in info["files"]:
            x = postprocess_voice(os.path.join(VOICE_WORK, fname))
            register(fname, x, peak=0.87)
        sounds[key] = sorted(sounds[key])

    manifest = {
        "version": 1,
        "generator": "tools/sound-gen (see SOUND-CATALOG.md)",
        "phrases": {k: v["phrases"] for k, v in voices_meta.items()},
        "voices": {k: v["voice"] for k, v in voices_meta.items()},
        "sounds": {k: sorted(v) for k, v in sorted(sounds.items())},
        "cueSounds": CUE_SOUNDS,
    }
    with open(os.path.join(ASSETS, "manifest.json"), "w") as f:
        json.dump(manifest, f, indent=1)

    total = 0
    for files in sounds.values():
        for fname in files:
            total += os.path.getsize(os.path.join(ASSETS, fname))
    n = sum(len(v) for v in sounds.values())
    print(f"assets: {n} files, {total / 1024:.0f} KiB, {len(sounds)} sound keys")
    return 0


if __name__ == "__main__":
    sys.exit(main())
