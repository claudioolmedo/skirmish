"""Man-at-Arms (maa) — directional milestone model.

Original design: the heavy infantry of the set — same villager
proportions, but broader: steel cuirass and pauldrons over a
player-color tabard (front panel + sleeves, 'cloth'), steel_dark
faulds, greaves and belt, a full steel helmet with a dark visor slit
and nasal guard, a tall kite shield (steel_dark rim, player-color
face) on the left forearm and a slightly larger sword in the right
hand. Attack rhythm matches the militia; the walk is stiffer
(legs +-24 deg).

Module contract: see models/villager.py.
"""
import math

import numpy as np

import render
from render import box_tris, hex2rgb, rot_x, xform

MATERIALS_ADD = {'steel': hex2rgb('#9aa3ad'),
                 'steel_dark': hex2rgb('#6d747c')}
PC_MATS = {'cloth'}
STATES = {'idle': 4, 'walk': 8, 'attack': 4}

_PIVOTS = render.PIVOTS
_WAIST = render.WAIST


def _sword_tris():
    """Slightly larger sword than the militia's, at the right hand,
    blade down at rest, swinging with armR. Length is margin-bound
    (see militia._sword_tris): tip at z 0.36."""
    t = []
    t += box_tris(0.30, 0, 0.36, 0.74, 0.032, 0.036, 0.115, 0.042,
                  'steel')                                      # blade
    t += box_tris(0.30, 0, 0.72, 0.79, 0.24, 0.065, mat='steel_dark')  # guard
    t += box_tris(0.30, 0, 0.76, 0.90, 0.06, 0.06, mat='belt')  # grip
    t += box_tris(0.30, 0, 0.90, 0.96, 0.085, 0.085, mat='steel_dark')  # pommel
    return t


def _shield_tris():
    """Tall kite shield on the left forearm: steel_dark rim tapering
    to a point at the bottom, player-color face panel, steel boss."""
    t = []
    t += box_tris(-0.38, 0.05, 0.62, 1.22, 0.30, 0.05, 0.44, 0.05,
                  'steel_dark')                                 # rim
    t += box_tris(-0.38, 0.085, 0.70, 1.16, 0.20, 0.03, 0.34, 0.03,
                  'cloth')                                      # face
    t += box_tris(-0.38, 0.105, 0.88, 1.00, 0.10, 0.04, 0.10, 0.04,
                  'steel')                                      # boss
    return t


def _build():
    parts = []

    def add(tris, group):
        parts.append((tris, group))

    for side, sgn in (('legL', -1), ('legR', 1)):
        x = 0.11 * sgn
        add(box_tris(x, 0.0, 0.08, 0.82, 0.14, 0.16, 0.17, 0.19,
                     'trousers'), side)
        # steel_dark greave over the shin
        add(box_tris(x, 0.01, 0.10, 0.46, 0.155, 0.175, 0.165, 0.185,
                     'steel_dark'), side)
        add(box_tris(x, 0.03, 0.0, 0.11, 0.15, 0.25, 0.15, 0.23,
                     'boots'), side)
    for side, sgn in (('armL', -1), ('armR', 1)):
        x = 0.30 * sgn
        # player-color sleeve (tabard cloth under the plate)
        add(box_tris(x, 0.0, 1.05, 1.33, 0.125, 0.135, 0.135, 0.145,
                     'cloth'), side)
        # steel_dark vambrace instead of a bare forearm
        add(box_tris(x, 0.0, 0.76, 1.07, 0.10, 0.11, 0.11, 0.125,
                     'steel_dark'), side)
        # pauldron riding on the shoulder (rotates about the pivot
        # it sits on, so it barely travels)
        add(box_tris(0.31 * sgn, 0.0, 1.24, 1.39, 0.21, 0.21, 0.19, 0.19,
                     'steel_dark'), side)
    add(_sword_tris(), 'armR')
    add(_shield_tris(), 'armL')
    # faulds (steel_dark skirt) + plate cuirass (steel) + dark belt
    add(box_tris(0, 0, 0.72, 0.99, 0.52, 0.40, 0.47, 0.34,
                 'steel_dark'), 'upper')
    add(box_tris(0, 0, 0.98, 1.34, 0.46, 0.32, 0.50, 0.345,
                 'steel'), 'upper')
    # player-color tabard panel: chest down over the faulds, proud
    # of both the cuirass front and the skirt front
    add(box_tris(0, 0.20, 0.76, 1.30, 0.20, 0.045, 0.22, 0.045,
                 'cloth'), 'upper')
    add(box_tris(0, 0, 0.93, 1.02, 0.50, 0.355, 0.50, 0.355,
                 'steel_dark'), 'upper')
    # neck + head + full steel helmet: cap, dark visor slit, nasal
    add(box_tris(0, 0, 1.30, 1.42, 0.12, 0.12, 0.12, 0.12, 'skin'), 'upper')
    add(box_tris(0, 0.01, 1.36, 1.65, 0.27, 0.26, 0.25, 0.245,
                 'skin'), 'upper')
    add(box_tris(0, -0.01, 1.52, 1.70, 0.315, 0.30, 0.29, 0.28,
                 'steel'), 'upper')
    add(box_tris(0, 0.132, 1.44, 1.49, 0.20, 0.025, 0.20, 0.025,
                 'steel_dark'), 'upper')                        # visor slit
    add(box_tris(0, 0.135, 1.34, 1.52, 0.05, 0.03, 0.05, 0.03,
                 'steel_dark'), 'upper')                        # nasal guard
    return parts


_PARTS = _build()


def _apply(pose):
    out = []
    torso_mat = rot_x(-pose.get('torso', 0.0))
    for tris, group in _PARTS:
        for verts, mat in tris:
            v = verts
            if group in _PIVOTS:
                v = xform(v, rot_x(pose.get(group, 0.0)), _PIVOTS[group])
            if group in ('upper', 'armL', 'armR'):
                v = xform(v, torso_mat, _WAIST)
            v = v + np.array([0.0, 0.0, pose.get('bob', 0.0)])
            out.append((v, mat))
    return out


def _pose_idle(f, n=4):
    p = render.pose_idle(f, n)
    breathe = math.sin(2 * math.pi * (f / n))
    p.update({'armL': 8.0 + 1.5 * breathe,   # kite shield held low
              'armR': -2.0 * breathe})
    return p


def _pose_walk(f, n=8):
    t = f / n
    s = math.sin(2 * math.pi * t)
    # stiffer than the militia: legs +-24, shield arm barely swings
    return {'legL': 24.0 * s, 'legR': -24.0 * s,
            'armL': -8.0 * s, 'armR': 16.0 * s,
            'torso': 4.0, 'bob': -0.07 * abs(s) + 0.02}


def _pose_attack(f, n=4):
    # same rhythm as the militia
    arm_r = (-65.0, 75.0, 35.0, 5.0)[f % n]   # windup, slash, follow, recover
    torso = (-6.0, 10.0, 14.0, 4.0)[f % n]
    return {'legL': 10.0, 'legR': -10.0,
            'armL': 15.0, 'armR': arm_r,    # shield stays on guard
            'torso': torso, 'bob': 0.0}


_POSES = {'idle': _pose_idle, 'walk': _pose_walk, 'attack': _pose_attack}


def tris_for(state, frame):
    return _apply(_POSES[state](frame, STATES[state]))
