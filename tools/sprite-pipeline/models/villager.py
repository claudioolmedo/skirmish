"""Villager — directional milestone model.

Reuses the v1 villager geometry from render.py (100% our own
procedural low-poly humanoid) and adds the BUILD action the v1 sheet
set lacked. Player color: the tunic (torso, skirt, sleeves).

Module contract (shared by every models/<unit>.py):
  PC_MATS        set of material names carrying player color
  STATES         {state: frame_count} at the LOW tier (8 directions)
  tris_for(state, frame) -> flat [(verts, material)] posed, model space
  CANVAS         optional (cw, ch, ax, ay) override; default 128² @(64,110)
"""
import math

import numpy as np

import render
from render import rot_x, xform

PC_MATS = {'tunic'}
STATES = {'idle': 4, 'walk': 12, 'gather': 8, 'carry': 12, 'build': 8,
          'die': 6}
# Death needs a bigger cell than the standing figure: the fallen
# body lies a full height from the feet in model space. The client
# mirrors this canvas in its corpse spec (app.js CORPSE_SPEC).
STATE_CANVAS = {'die': (232, 208, 116, 152)}
_FALL = (0.0, 10.0, 26.0, 48.0, 68.0, 80.0)

_PARTS = render.build_villager()
_PIVOTS = render.PIVOTS
_WAIST = render.WAIST
_LOAD = render.LOAD_TRIS


def _apply(pose):
    """Same pose mechanics as render.pose_model (limb pivots + waist
    pitch + bob + optional overhead load)."""
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
    if pose.get('load'):
        for verts, mat in _LOAD:
            v = xform(verts, torso_mat, _WAIST)
            v = v + np.array([0.0, 0.0, pose.get('bob', 0.0)])
            out.append((v, mat))
    if pose.get('tool'):
        # The gather axe (see render.AXE_TRIS): armR pivot first,
        # then the waist pitch — glued to the right hand.
        for verts, mat in render.AXE_TRIS:
            v = xform(verts, rot_x(pose.get('armR', 0.0)), _PIVOTS['armR'])
            v = xform(v, torso_mat, _WAIST)
            v = v + np.array([0.0, 0.0, pose.get('bob', 0.0)])
            out.append((v, mat))
    return out


def _pose_build(f, n=8):
    # Hammer cycle against a wall/scaffold: both arms work together,
    # torso bent deeper than the gather chop, legs planted wide.
    arm = render.key((-58.0, 8.0, 44.0, -20.0), f, n)
    torso = render.key((16.0, 30.0, 42.0, 26.0), f, n)
    return {'legL': 14.0, 'legR': -14.0,
            'armL': arm * 0.85, 'armR': arm,
            'torso': torso, 'bob': 0.0, 'load': False}


_POSES = {'idle': render.pose_idle, 'walk': render.pose_walk,
          'gather': render.pose_gather, 'carry': render.pose_carry,
          'build': _pose_build}


def tris_for(state, frame):
    if state == 'die':
        # The idle figure tipped backward about its own feet
        # (spin_x over the ground origin, easing in); the last
        # frame is the corpse the client leaves fading on the
        # field (client-only presentation, no sim contact).
        base = _apply(render.pose_idle(0, STATES['idle']))
        return render.spin_x(base, _FALL[frame % len(_FALL)])
    fn = _POSES[state]
    return _apply(fn(frame, STATES[state]))
