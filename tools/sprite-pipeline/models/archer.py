"""Archer — directional milestone model.

Original design: villager-proportioned hunter in a leather vest over
a player-color shirt (sleeves + chest panel carry the player color),
leather hood and an angled quiver on the back. The bow lives in the
armL group so every pose carries it; it is baked with a -55 deg
pitch about the grip so that when the shoot pose raises the arm to
~80 deg the bow stands near-vertical, and at rest it reads as a
diagonal carry.

Module contract: see models/villager.py.
"""
import math

import numpy as np

import render
from render import box_tris, hex2rgb, rot_x, rot_z, xform

MATERIALS_ADD = {'leather': hex2rgb('#7a4a26')}
PC_MATS = {'cloth'}
STATES = {'idle': 4, 'walk': 12, 'shoot': 8, 'die': 6}
# Death cell: the fallen body lies a full height from the feet.
STATE_CANVAS = {'die': (232, 208, 116, 152)}
_FALL = (0.0, 10.0, 26.0, 48.0, 68.0, 80.0)

_PIVOTS = render.PIVOTS
_WAIST = render.WAIST

_GRIP = (-0.30, 0.12, 0.82)  # bow grip, at the left hand (arm hanging)


def _xf(tris, mat, pivot):
    return [(xform(v, mat, pivot), m) for v, m in tris]


def _bow_tris():
    """Chunky bow in the YZ plane in front of the left hand: grip,
    two limbs tilted for an arc, and a string. Built in arm-local
    (arm hanging) space, then pitched -55 deg about the grip (see
    module docstring). Material: leather limbs, wood grip/string."""
    t = []
    # grip at the hand
    t += box_tris(-0.30, 0.12, 0.72, 0.92, 0.075, 0.075, mat='wood')
    # upper limb, tilted so its tip leans forward (+Y): the arc.
    # Limb length is margin-bound: a longer bow breaches the 3px
    # cell margin at the shoot extremes (see units.py margin check).
    up = box_tris(-0.30, 0.12, 0.90, 1.34, 0.075, 0.075,
                  0.06, 0.06, mat='leather')
    t += _xf(up, rot_x(-20.0), _GRIP)
    # lower limb, mirrored tilt
    lo = box_tris(-0.30, 0.12, 0.30, 0.74, 0.06, 0.06,
                  0.075, 0.075, mat='leather')
    t += _xf(lo, rot_x(20.0), _GRIP)
    # string: one chunky slab behind the grip line, tip to tip
    t += box_tris(-0.30, 0.045, 0.38, 1.28, 0.03, 0.03, mat='wood')
    # whole bow: pitch about the grip so the shoot pose reads vertical
    return _xf(t, rot_x(-55.0), _GRIP)


def _quiver_tris():
    """Thin leather quiver angled on the back (-Y) + two arrow
    shafts (wood) poking out of the top. Baked tilt about its center;
    lives in the 'upper' group so the torso pitch carries it."""
    c = (0.16, -0.185, 1.17)
    t = []
    t += box_tris(0.16, -0.185, 0.98, 1.36, 0.11, 0.09, mat='leather')
    t += box_tris(0.115, -0.19, 1.32, 1.53, 0.04, 0.04, mat='wood')
    t += box_tris(0.20, -0.19, 1.32, 1.50, 0.04, 0.04, mat='wood')
    return _xf(t, rot_z(-18.0), c)


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
        # player-color shirt sleeve, shoulder -> elbow
        add(box_tris(x, 0.0, 1.05, 1.33, 0.115, 0.125, 0.125, 0.135,
                     'cloth'), side)
        # forearm + hand (skin)
        add(box_tris(x, 0.0, 0.76, 1.07, 0.09, 0.10, 0.10, 0.115,
                     'skin'), side)
    # the bow rides with the left arm in every pose
    add(_bow_tris(), 'armL')
    # skirt + vest (leather), belt
    add(box_tris(0, 0, 0.72, 0.99, 0.47, 0.36, 0.415, 0.295,
                 'leather'), 'upper')
    add(box_tris(0, 0, 0.98, 1.34, 0.40, 0.275, 0.435, 0.30,
                 'leather'), 'upper')
    # player-color chest panel, proud of the vest front
    add(box_tris(0, 0.148, 1.02, 1.30, 0.20, 0.05, 0.22, 0.05,
                 'cloth'), 'upper')
    add(box_tris(0, 0, 0.93, 1.02, 0.45, 0.315, 0.45, 0.315,
                 'belt'), 'upper')
    # neck + head + leather hood (cap + short back drape)
    add(box_tris(0, 0, 1.30, 1.42, 0.12, 0.12, 0.12, 0.12, 'skin'), 'upper')
    add(box_tris(0, 0.01, 1.36, 1.65, 0.27, 0.26, 0.25, 0.245,
                 'skin'), 'upper')
    add(box_tris(0, -0.015, 1.56, 1.69, 0.31, 0.30, 0.285, 0.275,
                 'leather'), 'upper')
    add(box_tris(0, -0.155, 1.04, 1.44, 0.22, 0.05, 0.26, 0.05,
                 'leather'), 'upper')
    # quiver on the back
    add(_quiver_tris(), 'upper')
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
    p.update({'armL': 8.0 + 2.0 * breathe})  # bow arm relaxed, low carry
    return p


def _pose_walk(f, n=8):
    p = render.pose_walk(f, n)
    # left arm keeps the bow carried; only the right arm counter-swings
    p.update({'armL': 25.0})
    return p


def _pose_shoot(f, n=8):
    # armL holds the bow forward in every frame; armR draws.
    arm_r = render.key((70.0, -25.0, 85.0, 30.0), f, n)     # aim, draw, release, lower
    arm_l = render.key((80.0, 80.0, 80.0, 30.0), f, n)
    torso = render.key((4.0, 2.0, 6.0, 3.0), f, n)
    return {'legL': 8.0, 'legR': -8.0,
            'armL': arm_l, 'armR': arm_r,
            'torso': torso, 'bob': 0.0}


_POSES = {'idle': _pose_idle, 'walk': _pose_walk, 'shoot': _pose_shoot}


def tris_for(state, frame):
    if state == 'die':
        # Idle figure tipped backward about its own feet; the
        # last frame is the corpse (see models/villager.py).
        base = _apply(_POSES['idle'](0, STATES['idle']))
        return render.spin_x(base, _FALL[frame % len(_FALL)])
    return _apply(_POSES[state](frame, STATES[state]))
