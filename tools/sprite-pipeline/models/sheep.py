"""Sheep — directional milestone model (Gaia animal, no player color).

Original design: a chunky wool body with a domed wool top, a small
dark head carried low, short dark legs and a wool tail nub. Poses:
idle (standing, slight head sway), walk (diagonal-pair leg swing with
a small bounce), graze (head group pitches down to nibble).

Module contract (shared by every models/<unit>.py):
  MATERIALS_ADD  new materials this model needs
  PC_MATS        set of material names carrying player color
  STATES         {state: frame_count} at the LOW tier (8 directions)
  tris_for(state, frame) -> flat [(verts, material)] posed, model space

Model space: +X right, +Y forward (the head end), +Z up, origin at
the ground center under the body. Legs sit at (+-0.19, +-0.30) with
slim feet so swung-forward hooves keep the >= 3 px margin of the
default 128 px cell in the front directions.
"""
import math

import numpy as np

import render
from render import box_tris, hex2rgb, rot_x, xform

MATERIALS_ADD = {'wool': hex2rgb('#ddd6c4'),
                 'sheep_face': hex2rgb('#3a3430')}
PC_MATS = set()
STATES = {'idle': 4, 'walk': 8, 'graze': 4}

LEG_X, LEG_Y, LEG_PIVOT_Z = 0.19, 0.30, 0.40
HEAD_PIVOT = (0.0, 0.42, 0.66)


def _xf(tris, mat, pivot):
    return [(xform(v, mat, pivot), m) for v, m in tris]


def _sh(tris, dx, dy, dz):
    off = np.array([dx, dy, dz], dtype=np.float64)
    return [(v + off, m) for v, m in tris]


def _leg(sx, sy, angle, bob):
    x, y = LEG_X * sx, LEG_Y * sy
    tris = box_tris(x, y, 0.0, 0.42, 0.08, 0.10, 0.105, 0.12,
                    'sheep_face')
    if angle:
        tris = _xf(tris, rot_x(angle), (x, y, LEG_PIVOT_Z))
    if bob:
        tris = _sh(tris, 0.0, 0.0, bob)
    return tris


def _head(pitch, dy=0.0):
    """Head group built relative to the neck-base pivot at the front
    of the body, pitched about it (negative pitch = down-forward)."""
    tris = []
    tris += box_tris(0, 0.08, -0.10, 0.14, 0.30, 0.24, 0.27, 0.21,
                     'wool')                        # woolly neck wedge
    tris += box_tris(0, 0.31, -0.20, 0.09, 0.24, 0.30, 0.20, 0.26,
                     'sheep_face')                  # head
    tris += box_tris(0, 0.475, -0.21, -0.05, 0.16, 0.14, 0.15, 0.13,
                     'sheep_face')                  # muzzle
    for s in (-1, 1):                               # ears
        tris += box_tris(0.155 * s, 0.23, -0.07, 0.07, 0.10, 0.14,
                         0.09, 0.13, 'sheep_face')
    tris += box_tris(0, 0.16, 0.05, 0.17, 0.25, 0.22, 0.23, 0.20,
                     'wool')                        # wool cap
    tris = _xf(tris, rot_x(pitch), (0.0, 0.0, 0.0))
    return _sh(tris, HEAD_PIVOT[0], HEAD_PIVOT[1] + dy, HEAD_PIVOT[2])


def _body(bob):
    tris = []
    tris += box_tris(0, 0, 0.38, 0.94, 0.62, 0.95, 0.58, 0.90, 'wool')
    tris += box_tris(0, -0.02, 0.88, 1.04, 0.46, 0.72, 0.42, 0.66,
                     'wool')                        # domed wool top
    tris += box_tris(0, -0.50, 0.60, 0.76, 0.11, 0.12, 0.10, 0.11,
                     'wool')                        # tail nub
    if bob:
        tris = _sh(tris, 0.0, 0.0, bob)
    return tris


def tris_for(state, frame):
    tris = []
    if state == 'idle':
        t = frame / STATES['idle']
        sway = 3.0 * math.sin(2 * math.pi * t)
        bob = 0.006 * math.sin(2 * math.pi * t)
        tris += _body(bob)
        tris += _sh(_head(-14.0 + sway), 0.0, 0.0, bob)
        for sx in (-1, 1):
            for sy in (-1, 1):
                tris += _leg(sx, sy, 1.0 * sy, bob)
        return tris
    if state == 'walk':
        t = frame / STATES['walk']
        s = math.sin(2 * math.pi * t)
        # Bounce peaks at stride extremes, lifting swung-forward feet.
        bob = 0.035 * abs(s)
        tris += _body(bob)
        tris += _sh(_head(-12.0 + 2.0 * math.sin(4 * math.pi * t)),
                    0.0, 0.0, bob)
        amp = 17.0
        tris += _leg(-1, 1, amp * s, bob)    # front-left
        tris += _leg(1, -1, amp * s, bob)    # hind-right (diagonal)
        tris += _leg(1, 1, -amp * s, bob)    # front-right
        tris += _leg(-1, -1, -amp * s, bob)  # hind-left
        return tris
    if state == 'graze':
        pitch = (-16.0, -54.0, -60.0, -20.0)[frame % 4]
        dy = (0.0, 0.05, 0.06, 0.01)[frame % 4]
        tris += _body(0.0)
        tris += _head(pitch, dy)
        for sx in (-1, 1):
            for sy in (-1, 1):
                tris += _leg(sx, sy, 0.0, 0.0)
        return tris
    raise KeyError(state)
