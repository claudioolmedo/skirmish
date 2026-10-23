"""Militia — directional milestone model.

Original design: villager-proportioned levy in a player-color tunic
with a simple iron helmet, a short sword in the right hand (part of
the armR group, so the attack swing carries it) and a small round-ish
shield on the left forearm — wood rim, player-color ('cloth') front
panel, iron boss.

Module contract: see models/villager.py.
"""
import math

import numpy as np

import render
from render import box_tris, hex2rgb, rot_x, xform

MATERIALS_ADD = {'iron': hex2rgb('#8b8f96')}
PC_MATS = {'tunic', 'cloth'}
STATES = {'idle': 4, 'walk': 12, 'attack': 8, 'die': 6}
# Death cell: the fallen body lies a full height from the feet.
STATE_CANVAS = {'die': (232, 208, 116, 152)}
_FALL = (0.0, 10.0, 26.0, 48.0, 68.0, 80.0)

_PIVOTS = render.PIVOTS
_WAIST = render.WAIST


def _sword_tris():
    """Short sword at the right hand, blade pointing down at rest
    (it swings with armR). Guard + pommel iron, grip belt-leather,
    blade iron tapering to a point. Kept short on purpose: a longer
    blade would breach the 3px cell margin at the windup/slash
    extremes (see units.py margin check)."""
    t = []
    t += box_tris(0.30, 0, 0.40, 0.74, 0.03, 0.034, 0.10, 0.038,
                  'iron')                                   # blade
    t += box_tris(0.30, 0, 0.72, 0.79, 0.20, 0.06, mat='iron')  # guard
    t += box_tris(0.30, 0, 0.76, 0.90, 0.055, 0.055, mat='belt')  # grip
    t += box_tris(0.30, 0, 0.90, 0.95, 0.075, 0.075, mat='iron')  # pommel
    return t


def _shield_tris():
    """Small shield plate on the left forearm: wood rim, player-color
    front panel, iron boss. Faces +Y (forward) when the arm hangs."""
    t = []
    t += box_tris(-0.37, 0.05, 0.76, 1.14, 0.32, 0.05, 0.36, 0.05,
                  'wood')                                       # rim
    t += box_tris(-0.37, 0.085, 0.82, 1.08, 0.26, 0.03, 0.26, 0.03,
                  'cloth')                                      # panel
    t += box_tris(-0.37, 0.105, 0.90, 1.00, 0.09, 0.04, 0.09, 0.04,
                  'iron')                                       # boss
    return t


def _build():
    parts = []

    def add(tris, group):
        parts.append((tris, group))

    for side, sgn in (('legL', -1), ('legR', 1)):
        x = 0.11 * sgn
        add(box_tris(x, 0.0, 0.08, 0.82, 0.13, 0.15, 0.16, 0.18,
                     'trousers'), side)
        add(box_tris(x, 0.03, 0.0, 0.11, 0.14, 0.24, 0.14, 0.22,
                     'boots'), side)
    for side, sgn in (('armL', -1), ('armR', 1)):
        x = 0.30 * sgn
        # tunic sleeve (player color), shoulder -> elbow
        add(box_tris(x, 0.0, 1.05, 1.33, 0.115, 0.125, 0.125, 0.135,
                     'tunic'), side)
        # forearm + hand (skin)
        add(box_tris(x, 0.0, 0.76, 1.07, 0.09, 0.10, 0.10, 0.115,
                     'skin'), side)
    add(_sword_tris(), 'armR')
    add(_shield_tris(), 'armL')
    # tunic skirt + torso + belt (player color)
    add(box_tris(0, 0, 0.72, 0.99, 0.47, 0.36, 0.415, 0.295,
                 'tunic'), 'upper')
    add(box_tris(0, 0, 0.98, 1.34, 0.40, 0.275, 0.435, 0.30,
                 'tunic'), 'upper')
    add(box_tris(0, 0, 0.93, 1.02, 0.45, 0.315, 0.45, 0.315,
                 'belt'), 'upper')
    # neck + head + iron helmet (cap + brim)
    add(box_tris(0, 0, 1.30, 1.42, 0.12, 0.12, 0.12, 0.12, 'skin'), 'upper')
    add(box_tris(0, 0.01, 1.36, 1.65, 0.27, 0.26, 0.25, 0.245,
                 'skin'), 'upper')
    add(box_tris(0, 0, 1.56, 1.70, 0.31, 0.30, 0.29, 0.28, 'iron'), 'upper')
    add(box_tris(0, 0, 1.545, 1.59, 0.38, 0.37, 0.38, 0.37, 'iron'), 'upper')
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
    p.update({'armL': 6.0 + 1.5 * breathe,   # shield held low
              'armR': -2.0 * breathe})      # sword arm at rest
    return p


def _pose_walk(f, n=8):
    p = render.pose_walk(f, n)
    s = math.sin(2 * math.pi * (f / n))
    # shield arm swings at half amplitude; sword arm swings fully
    p.update({'armL': -10.0 * s})
    return p


def _pose_attack(f, n=8):
    arm_r = render.key((-65.0, 75.0, 35.0, 5.0), f, n)   # windup, slash, follow, recover
    torso = render.key((-6.0, 10.0, 14.0, 4.0), f, n)
    return {'legL': 10.0, 'legR': -10.0,
            'armL': 15.0, 'armR': arm_r,    # shield stays on guard
            'torso': torso, 'bob': 0.0}


_POSES = {'idle': _pose_idle, 'walk': _pose_walk, 'attack': _pose_attack}


def tris_for(state, frame):
    if state == 'die':
        # Idle figure tipped backward about its own feet; the
        # last frame is the corpse (see models/villager.py).
        base = _apply(_POSES['idle'](0, STATES['idle']))
        return render.spin_x(base, _FALL[frame % len(_FALL)])
    return _apply(_POSES[state](frame, STATES[state]))
