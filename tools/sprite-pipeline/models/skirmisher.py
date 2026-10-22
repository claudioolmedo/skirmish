"""Skirmisher — directional milestone model.

Original design: the archer's villager-proportioned frame,
leather vest over a player-color shirt and leather hood, re-propped
for the javelin role — a short throwing spear in the right hand
(wood shaft, iron tip, held upright at rest, hurled overhand in
the shoot cycle) and a LARGE shield on the left forearm, the
unit's second silhouette half: wood rim, player-color ('cloth')
front panel, iron boss, noticeably wider than the Spearman's.
No bow, no quiver — the javelin + big shield read is the wiki
Skirmisher's recognizability cue.

Module contract: see models/villager.py.
"""
import math

import numpy as np

import render
from render import box_tris, hex2rgb, rot_x, xform

MATERIALS_ADD = {'iron': hex2rgb('#8b8f96'),
                 'leather': hex2rgb('#7a4a26')}
PC_MATS = {'cloth'}
STATES = {'idle': 4, 'walk': 8, 'shoot': 4}

_PIVOTS = render.PIVOTS
_WAIST = render.WAIST


def _javelin_tris():
    """Short throwing spear at the right hand, pointing up at
    rest (it sweeps overhand with armR in the throw). Wood shaft,
    iron tip tapering to a point; shorter than the Spearman's
    pike — a one-handed missile, butt below the grip."""
    t = []
    t += box_tris(0.30, 0, 0.50, 1.58, 0.04, 0.04, 0.04, 0.04,
                  'wood')                                   # shaft
    t += box_tris(0.30, 0, 1.58, 1.76, 0.065, 0.026, 0.010, 0.010,
                  'iron')                                   # tip
    return t


def _shield_tris():
    """Large shield on the left forearm: wood rim, player-color
    front panel, iron boss. Taller and wider than the
    Spearman's — the skirmisher hides behind it between throws.
    Faces +Y (forward) when the arm hangs."""
    t = []
    t += box_tris(-0.37, 0.05, 0.48, 1.32, 0.52, 0.05, 0.56, 0.05,
                  'wood')                                       # rim
    t += box_tris(-0.37, 0.085, 0.56, 1.24, 0.44, 0.03, 0.48, 0.03,
                  'cloth')                                      # panel
    t += box_tris(-0.37, 0.105, 0.83, 0.99, 0.12, 0.04, 0.12, 0.04,
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
        # player-color shirt sleeve, shoulder -> elbow
        add(box_tris(x, 0.0, 1.05, 1.33, 0.115, 0.125, 0.125, 0.135,
                     'cloth'), side)
        # forearm + hand (skin)
        add(box_tris(x, 0.0, 0.76, 1.07, 0.09, 0.10, 0.10, 0.115,
                     'skin'), side)
    # the javelin rides with the right arm, the shield with the left
    add(_javelin_tris(), 'armR')
    add(_shield_tris(), 'armL')
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
    p.update({'armL': 8.0 + 2.0 * breathe,   # shield held low
              'armR': -2.0 * breathe})      # javelin upright at rest
    return p


def _pose_walk(f, n=8):
    p = render.pose_walk(f, n)
    s = math.sin(2 * math.pi * (f / n))
    # shield arm carries its weight at half swing; the javelin
    # arm steadies the shaft near-upright
    p.update({'armL': -8.0 * s, 'armR': -5.0 * s})
    return p


def _pose_shoot(f, n=4):
    # The throw: wind back, hurl overhand, follow through, recover.
    # armL raises the shield on guard throughout.
    arm_r = (-48.0, 96.0, 55.0, 8.0)[f % n]
    arm_l = (30.0, 38.0, 34.0, 25.0)[f % n]
    torso = (-5.0, 13.0, 8.0, 2.0)[f % n]
    return {'legL': 8.0, 'legR': -8.0,
            'armL': arm_l, 'armR': arm_r,
            'torso': torso, 'bob': 0.0}


_POSES = {'idle': _pose_idle, 'walk': _pose_walk, 'shoot': _pose_shoot}


def tris_for(state, frame):
    return _apply(_POSES[state](frame, STATES[state]))
