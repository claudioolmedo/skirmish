"""Wolf — AI-08 predator model (Gaia animal, no player color).

Original design: a lean grey predator, taller and narrower than
the boar — long legs, a level back with a darker saddle, a deep
chest, a bushy tail carried low behind, and a long narrow head
with a pale muzzle, dark nose, and two pointed ears. Poses: idle
(standing alert, head up, slight sway), walk (long diagonal
stride, head level), attack (the lunge: rear crouches, head
thrusts forward and down, jaws at the target).

Module contract (shared by every models/<unit>.py):
  MATERIALS_ADD  new materials this model needs
  PC_MATS        set of material names carrying player color
  STATES         {state: frame_count} at the LOW tier (8 directions)
  tris_for(state, frame) -> flat [(verts, material)] posed, model space

Model space: +X right, +Y forward (the head end), +Z up, origin at
the ground center under the body. Legs sit at (+-0.16, +-0.38);
the head pivot is kept close to the body so the
lunging head stays inside the default 128 px cell in the diagonal
directions (see the deer model's margin note).
"""
import math

import numpy as np

import render
from render import box_tris, hex2rgb, rot_x, xform

MATERIALS_ADD = {'wolf': hex2rgb('#83898f'),
                 'wolf_dark': hex2rgb('#4a4e55'),
                 'wolf_light': hex2rgb('#c3c7cb'),
                 'fang': hex2rgb('#e9e1cd')}
PC_MATS = set()
STATES = {'idle': 4, 'walk': 8, 'attack': 4}

LEG_X, LEG_Y, LEG_PIVOT_Z = 0.16, 0.38, 0.62
HEAD_PIVOT = (0.0, 0.48, 0.90)
TAIL_PIVOT = (0.0, -0.47, 0.80)


def _xf(tris, mat, pivot):
    return [(xform(v, mat, pivot), m) for v, m in tris]


def _sh(tris, dx, dy, dz):
    off = np.array([dx, dy, dz], dtype=np.float64)
    return [(v + off, m) for v, m in tris]


def _leg(sx, sy, angle, bob):
    x, y = LEG_X * sx, LEG_Y * sy
    tris = box_tris(x, y, 0.32, 0.66, 0.10, 0.115, 0.115, 0.13, 'wolf')
    tris += box_tris(x, y, 0.0, 0.34, 0.075, 0.09, 0.095, 0.11,
                     'wolf_dark')                    # dark lower leg / paw
    if angle:
        tris = _xf(tris, rot_x(angle), (x, y, LEG_PIVOT_Z))
    if bob:
        tris = _sh(tris, 0.0, 0.0, bob)
    return tris


def _head(pitch, dy=0.0):
    """Long narrow head relative to its pivot: tapered skull, pale
    muzzle with a dark nose, two pointed ears standing up. Positive
    pitch = snout down-forward (the bite)."""
    tris = []
    tris += box_tris(0, 0.10, -0.12, 0.18, 0.33, 0.32, 0.28, 0.28,
                     'wolf')                          # skull
    tris += box_tris(0, 0.30, -0.18, 0.03, 0.22, 0.24, 0.16, 0.19,
                     'wolf_light')                    # muzzle
    tris += box_tris(0, 0.385, -0.07, 0.07, 0.12, 0.11, 0.11, 0.10,
                     'wolf_dark')                     # nose
    for s in (-1, 1):
        tris += box_tris(0.11 * s, -0.02, 0.15, 0.40, 0.09, 0.095,
                         0.03, 0.035, 'wolf_dark')    # pointed ears
    tris = _xf(tris, rot_x(pitch), (0.0, 0.0, 0.0))
    return _sh(tris, HEAD_PIVOT[0], HEAD_PIVOT[1] + dy, HEAD_PIVOT[2])


def _tail(pitch):
    """Bushy tail off the rear pivot, carried low: a thick tapered
    brush angling down-back, dark at the tip."""
    tris = []
    tris += box_tris(0, -0.13, -0.10, 0.06, 0.17, 0.28, 0.15, 0.25,
                     'wolf')
    tris += box_tris(0, -0.27, -0.20, -0.02, 0.13, 0.16, 0.11, 0.14,
                     'wolf_dark')                     # dark tip
    tris = _xf(tris, rot_x(pitch), (0.0, 0.0, 0.0))
    return _sh(tris, TAIL_PIVOT[0], TAIL_PIVOT[1], TAIL_PIVOT[2])


def _body(bob, crouch=0.0):
    tris = []
    tris += box_tris(0, 0, 0.48, 1.02, 0.48, 1.03, 0.46, 1.00, 'wolf')
    tris += box_tris(0, 0.30, 0.44, 1.00, 0.53, 0.44, 0.51, 0.40,
                     'wolf')                          # deep chest
    tris += box_tris(0, -0.02, 0.98, 1.08, 0.26, 0.88, 0.25, 0.85,
                     'wolf_dark')                     # dark spine strip
    tris += box_tris(0, 0.10, 0.48, 0.60, 0.42, 0.76, 0.40, 0.73,
                     'wolf_light')                    # pale belly
    if bob or crouch:
        tris = _sh(tris, 0.0, 0.0, bob - crouch)
    return tris


def tris_for(state, frame):
    tris = []
    if state == 'idle':
        t = frame / STATES['idle']
        sway = 2.0 * math.sin(2 * math.pi * t)
        bob = 0.004 * math.sin(2 * math.pi * t)
        tris += _body(bob)
        tris += _tail(-24.0 + sway)
        tris += _sh(_head(-2.0 + sway), 0.0, 0.0, bob)
        for sx in (-1, 1):
            for sy in (-1, 1):
                tris += _leg(sx, sy, 0.5 * sy, bob)
        return tris
    if state == 'walk':
        t = frame / STATES['walk']
        s = math.sin(2 * math.pi * t)
        bob = 0.045 * abs(s)
        tris += _body(bob)
        tris += _tail(-20.0 + 3.0 * math.sin(2 * math.pi * t))
        tris += _sh(_head(4.0 + 1.5 * math.sin(4 * math.pi * t)),
                    0.0, 0.0, bob)
        amp = 15.0
        tris += _leg(-1, 1, amp * s, bob)    # front-left
        tris += _leg(1, -1, amp * s, bob)    # hind-right (diagonal)
        tris += _leg(1, 1, -amp * s, bob)    # front-right
        tris += _leg(-1, -1, -amp * s, bob)  # hind-left
        return tris
    if state == 'attack':
        # The lunge, 4 frames: coil (rear down, head back), thrust
        # (body stretches forward, head drives down), bite (head
        # lowest, muzzle foremost), recover.
        coil = (0.06, 0.02, 0.0, 0.03)[frame % 4]
        pitch = (-6.0, 11.0, 18.0, 7.0)[frame % 4]
        dy = (-0.02, 0.02, 0.03, 0.01)[frame % 4]
        tris += _body(0.0, crouch=coil)
        tris += _tail(-18.0)
        tris += _sh(_head(pitch, dy), 0.0, 0.0, -coil)
        tris += _leg(-1, 1, -7.0, -coil)     # front legs brace
        tris += _leg(1, 1, -7.0, -coil)
        tris += _leg(-1, -1, 8.0, -coil)     # hind legs drive
        tris += _leg(1, -1, 8.0, -coil)
        return tris
    raise KeyError(state)
