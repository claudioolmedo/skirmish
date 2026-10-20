"""Boar — directional milestone model (Gaia animal, no player color).

Original design: a heavy, low body with a shoulder hump and a dark
bristle ridge along the spine, a wedge head carried down-forward
with a dark snout and two small tusks, and short dark legs. Poses:
idle (standing, head low), walk (diagonal-pair swing, low bounce),
graze (rooting: head pitches down and pushes forward).

Module contract (shared by every models/<unit>.py):
  MATERIALS_ADD  new materials this model needs
  PC_MATS        set of material names carrying player color
  STATES         {state: frame_count} at the LOW tier (8 directions)
  tris_for(state, frame) -> flat [(verts, material)] posed, model space

Model space: +X right, +Y forward (the head end), +Z up, origin at
the ground center under the body. Legs sit at (+-0.18, +-0.34) with
slim feet so swung-forward hooves keep the >= 3 px margin of the
default 128 px cell in the front directions.
"""
import math

import numpy as np

import render
from render import box_tris, hex2rgb, rot_x, xform

MATERIALS_ADD = {'boar': hex2rgb('#5d4a38'),
                 'boar_dark': hex2rgb('#3e2f21'),
                 'tusk': hex2rgb('#e9e1cd')}
PC_MATS = set()
STATES = {'idle': 4, 'walk': 8, 'graze': 4}

LEG_X, LEG_Y, LEG_PIVOT_Z = 0.18, 0.34, 0.48
HEAD_PIVOT = (0.0, 0.50, 0.70)


def _xf(tris, mat, pivot):
    return [(xform(v, mat, pivot), m) for v, m in tris]


def _sh(tris, dx, dy, dz):
    off = np.array([dx, dy, dz], dtype=np.float64)
    return [(v + off, m) for v, m in tris]


def _leg(sx, sy, angle, bob):
    x, y = LEG_X * sx, LEG_Y * sy
    tris = box_tris(x, y, 0.26, 0.50, 0.115, 0.135, 0.125, 0.145,
                    'boar')
    tris += box_tris(x, y, 0.0, 0.28, 0.08, 0.10, 0.105, 0.12,
                     'boar_dark')
    if angle:
        tris = _xf(tris, rot_x(angle), (x, y, LEG_PIVOT_Z))
    if bob:
        tris = _sh(tris, 0.0, 0.0, bob)
    return tris


def _head(pitch, dy=0.0):
    """Wedge head group relative to its pivot at the body's front:
    tapered skull, dark snout, tusks at the snout sides, small ears.
    Negative pitch = down-forward (the boar's natural carry)."""
    tris = []
    tris += box_tris(0, 0.15, -0.15, 0.15, 0.33, 0.34, 0.25, 0.27,
                     'boar')                        # wedge skull
    tris += box_tris(0, 0.335, -0.19, -0.03, 0.19, 0.14, 0.17, 0.12,
                     'boar_dark')                   # snout
    for s in (-1, 1):
        tris += box_tris(0.12 * s, 0.295, -0.05, 0.09, 0.04, 0.05,
                         0.035, 0.045, 'tusk')      # tusks
        tris += box_tris(0.125 * s, -0.02, 0.13, 0.27, 0.085, 0.10,
                         0.075, 0.09, 'boar_dark')  # ears
    tris = _xf(tris, rot_x(pitch), (0.0, 0.0, 0.0))
    return _sh(tris, HEAD_PIVOT[0], HEAD_PIVOT[1] + dy, HEAD_PIVOT[2])


def _body(bob):
    tris = []
    tris += box_tris(0, 0, 0.36, 1.06, 0.64, 1.16, 0.62, 1.12, 'boar')
    tris += box_tris(0, 0.24, 0.92, 1.20, 0.56, 0.50, 0.53, 0.46,
                     'boar')                        # shoulder hump
    tris += box_tris(0, -0.46, 0.44, 1.00, 0.55, 0.32, 0.52, 0.28,
                     'boar')                        # tapered rear
    tris += box_tris(0, 0.0, 1.04, 1.17, 0.075, 0.95, 0.07, 0.88,
                     'boar_dark')                   # bristle ridge
    tris += box_tris(0, 0.26, 1.17, 1.28, 0.075, 0.44, 0.07, 0.38,
                     'boar_dark')                   # hump ridge
    if bob:
        tris = _sh(tris, 0.0, 0.0, bob)
    return tris


def tris_for(state, frame):
    tris = []
    if state == 'idle':
        t = frame / STATES['idle']
        sway = 2.5 * math.sin(2 * math.pi * t)
        bob = 0.005 * math.sin(2 * math.pi * t)
        tris += _body(bob)
        tris += _sh(_head(-16.0 + sway), 0.0, 0.0, bob)
        for sx in (-1, 1):
            for sy in (-1, 1):
                tris += _leg(sx, sy, 1.0 * sy, bob)
        return tris
    if state == 'walk':
        t = frame / STATES['walk']
        s = math.sin(2 * math.pi * t)
        bob = 0.05 * abs(s)
        tris += _body(bob)
        tris += _sh(_head(-18.0 + 1.5 * math.sin(4 * math.pi * t)),
                    0.0, 0.0, bob)
        amp = 15.0
        tris += _leg(-1, 1, amp * s, bob)    # front-left
        tris += _leg(1, -1, amp * s, bob)    # hind-right (diagonal)
        tris += _leg(1, 1, -amp * s, bob)    # front-right
        tris += _leg(-1, -1, -amp * s, bob)  # hind-left
        return tris
    if state == 'graze':
        # Rooting: head pitches down and shoves forward, then eases.
        pitch = (-18.0, -34.0, -40.0, -24.0)[frame % 4]
        dy = (0.0, 0.04, 0.06, 0.02)[frame % 4]
        tris += _body(0.0)
        tris += _head(pitch, dy)
        for sx in (-1, 1):
            for sy in (-1, 1):
                tris += _leg(sx, sy, 0.0, 0.0)
        return tris
    raise KeyError(state)
