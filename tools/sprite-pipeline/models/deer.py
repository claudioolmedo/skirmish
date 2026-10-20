"""Deer — directional milestone model (Gaia animal, no player color).

Original design: a slender body on long thin legs, an upright neck,
a small head with ears and two modest branched antlers (beam + tine
per side), and a white tail nub held up at the rear. Poses: idle
(head up, alert), walk (long diagonal stride with a gentle bounce),
graze (neck + head pitch far down about the neck base).

Module contract (shared by every models/<unit>.py):
  MATERIALS_ADD  new materials this model needs
  PC_MATS        set of material names carrying player color
  STATES         {state: frame_count} at the LOW tier (8 directions)
  tris_for(state, frame) -> flat [(verts, material)] posed, model space

Model space: +X right, +Y forward (the head end), +Z up, origin at
the ground center under the body. Antler tips stay near z 1.57 and
within 0.62 of the neck pivot: in the back directions the far head
rides higher on screen by 0.707 * y, and in the diagonal directions
a grazing deer's head parts swing forward by their full pivot
distance (screen width there is 1.414 * y) — a longer neck or
taller antlers would clip the default 128 px cell, which is also
why the graze pitch tops out near 56 degrees. Legs at (+-0.15,
+-0.37).
"""
import math

import numpy as np

import render
from render import box_tris, hex2rgb, rot_x, xform

MATERIALS_ADD = {'deer': hex2rgb('#9a7350'),
                 'deer_dark': hex2rgb('#755631'),
                 'antler': hex2rgb('#6e5133'),
                 'deer_white': hex2rgb('#efe9db')}
PC_MATS = set()
STATES = {'idle': 4, 'walk': 8, 'graze': 4}

LEG_X, LEG_Y, LEG_PIVOT_Z = 0.15, 0.37, 0.62
NECK_PIVOT = (0.0, 0.42, 0.98)
NECK_PITCH = -16.0  # neck tips slightly forward from vertical


def _xf(tris, mat, pivot):
    return [(xform(v, mat, pivot), m) for v, m in tris]


def _sh(tris, dx, dy, dz):
    off = np.array([dx, dy, dz], dtype=np.float64)
    return [(v + off, m) for v, m in tris]


def _leg(sx, sy, angle, bob):
    x, y = LEG_X * sx, LEG_Y * sy
    tris = box_tris(x, y, 0.32, 0.66, 0.095, 0.11, 0.105, 0.12, 'deer')
    tris += box_tris(x, y, 0.0, 0.34, 0.07, 0.085, 0.085, 0.10,
                     'deer_dark')
    if angle:
        tris = _xf(tris, rot_x(angle), (x, y, LEG_PIVOT_Z))
    if bob:
        tris = _sh(tris, 0.0, 0.0, bob)
    return tris


def _head(extra_pitch):
    """Neck + head + ears + antlers, built in a local frame at the
    neck base (chest top, front of the body) and pitched about it.
    `extra_pitch` adds to the upright base pitch (graze goes far
    negative: the whole neck swings down-forward)."""
    tris = []
    tris += box_tris(0, 0, 0.0, 0.36, 0.19, 0.23, 0.155, 0.185,
                     'deer')                        # neck column
    tris += box_tris(0, 0.03, 0.32, 0.50, 0.185, 0.22, 0.17, 0.20,
                     'deer')                        # head
    tris += box_tris(0, 0.17, 0.345, 0.455, 0.12, 0.11, 0.11, 0.10,
                     'deer_dark')                   # muzzle
    for s in (-1, 1):
        tris += box_tris(0.115 * s, -0.01, 0.42, 0.52, 0.085, 0.12,
                         0.075, 0.11, 'deer')       # ears
        tris += box_tris(0.075 * s, -0.035, 0.44, 0.56, 0.038, 0.045,
                         0.034, 0.04, 'antler')     # antler beam
        tris += box_tris(0.095 * s, -0.02, 0.50, 0.60, 0.032, 0.038,
                         0.028, 0.034, 'antler')    # antler tine
    tris = _xf(tris, rot_x(NECK_PITCH + extra_pitch), (0.0, 0.0, 0.0))
    return _sh(tris, *NECK_PIVOT)


def _body(bob):
    tris = []
    tris += box_tris(0, 0, 0.56, 1.20, 0.48, 1.16, 0.46, 1.12, 'deer')
    tris += box_tris(0, 0.44, 0.62, 1.16, 0.42, 0.32, 0.40, 0.30,
                     'deer')                        # chest
    tris += box_tris(0, -0.44, 0.64, 1.18, 0.44, 0.32, 0.42, 0.30,
                     'deer')                        # rump
    tris += box_tris(0, -0.64, 1.02, 1.22, 0.09, 0.12, 0.08, 0.11,
                     'deer_white')                  # raised white tail
    if bob:
        tris = _sh(tris, 0.0, 0.0, bob)
    return tris


def tris_for(state, frame):
    tris = []
    if state == 'idle':
        t = frame / STATES['idle']
        sway = 1.5 * math.sin(2 * math.pi * t)
        bob = 0.004 * math.sin(2 * math.pi * t)
        tris += _body(bob)
        tris += _sh(_head(sway), 0.0, 0.0, bob)
        for sx in (-1, 1):
            for sy in (-1, 1):
                tris += _leg(sx, sy, 1.0 * sy, bob)
        return tris
    if state == 'walk':
        t = frame / STATES['walk']
        s = math.sin(2 * math.pi * t)
        bob = 0.045 + 0.03 * abs(s)
        tris += _body(bob)
        tris += _sh(_head(2.0 * math.sin(4 * math.pi * t + 1.0)),
                    0.0, 0.0, bob)
        amp = 18.0
        tris += _leg(-1, 1, amp * s, bob)    # front-left
        tris += _leg(1, -1, amp * s, bob)    # hind-right (diagonal)
        tris += _leg(1, 1, -amp * s, bob)    # front-right
        tris += _leg(-1, -1, -amp * s, bob)  # hind-left
        return tris
    if state == 'graze':
        extra = (0.0, -22.0, -40.0, -12.0)[frame % 4]
        tris += _body(0.0)
        tris += _head(extra)
        for sx in (-1, 1):
            for sy in (-1, 1):
                tris += _leg(sx, sy, 0.0, 0.0)
        return tris
    raise KeyError(state)
