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
from render import box_tris, rot_x, xform

PC_MATS = {'tunic'}
STATES = {'idle': 4, 'walk': 12, 'gather': 8, 'carry': 12, 'build': 8,
          'attack': 4, 'die': 6,
          # Visible-carry sheets (2026-10-25): the carry cycle
          # hauling ONE resource — the overhead prop is that
          # resource (render.LOAD_SETS), so a loaded villager
          # reads its cargo; empty hands use the plain walk.
          'carry-wood': 12, 'carry-gold': 12, 'carry-stone': 12,
          'carry-food': 12}
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
        for verts, mat in render.LOAD_SETS[pose.get('load_kind', 'generic')]:
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
    if pose.get('knife'):
        # The fighting knife (attack readability pass, 2026-10-22):
        # same glue as the axe — armR pivot, waist pitch, bob.
        for verts, mat in KNIFE_TRIS:
            v = xform(verts, rot_x(pose.get('armR', 0.0)), _PIVOTS['armR'])
            v = xform(v, torso_mat, _WAIST)
            v = v + np.array([0.0, 0.0, pose.get('bob', 0.0)])
            out.append((v, mat))
    return out


def _knife_tris():
    """Short dagger in the right hand, built unposed like the axe
    (arm hanging straight down): belt-leather grip in the fist, a
    steel crossguard, and a bright steel blade running ~0.36 units
    below the guard to a stepped point. Steel is the palette's
    highest-value material on purpose — at game zoom the blade must
    read as a silhouette against grass and bodies, and it is NOT a
    player-color material (the mask keeps only the tunic)."""
    t = []
    t += box_tris(0.30, 0.005, 0.78, 0.94, 0.055, 0.06, mat='belt')
    t += box_tris(0.30, 0.01, 0.74, 0.79, 0.15, 0.05, mat='steel')
    t += box_tris(0.30, 0.02, 0.50, 0.75, 0.085, 0.035, 0.10, 0.04,
                  'steel')                                    # blade
    t += box_tris(0.30, 0.03, 0.38, 0.51, 0.045, 0.028, 0.085, 0.035,
                  'steel')                                    # point
    return t


KNIFE_TRIS = _knife_tris()


def _pose_build(f, n=8):
    # Hammer cycle against a wall/scaffold: both arms work together,
    # torso bent deeper than the gather chop, legs planted wide.
    arm = render.key((-58.0, 8.0, 44.0, -20.0), f, n)
    torso = render.key((16.0, 30.0, 42.0, 26.0), f, n)
    return {'legL': 14.0, 'legR': -14.0,
            'armL': arm * 0.85, 'armR': arm,
            'torso': torso, 'bob': 0.0, 'load': False}


def _pose_attack(f, n=4):
    # Knife fight (attack readability pass, 2026-10-22): the same
    # four-beat arc as the Militia's sword swing — windup back,
    # STRIKE (frame 1: arm whipped forward, blade extended away
    # from the body), follow-through, recover — so a fighting
    # villager reads at game zoom instead of standing still next
    # to its opponent. The left arm rises as a guard/counterweight.
    arm_r = (-65.0, 66.0, 32.0, 5.0)[f % n]
    torso = (-6.0, 10.0, 14.0, 4.0)[f % n]
    arm_l = (18.0, -14.0, -6.0, 4.0)[f % n]
    return {'legL': 12.0, 'legR': -10.0,
            'armL': arm_l, 'armR': arm_r,
            'torso': torso, 'bob': 0.0, 'load': False,
            'knife': True}


def _carry_kind(kind):
    """The carry cycle hauling one resource (see STATES): render's
    pose_carry with the load kind set — only the prop differs."""
    def pose(f, n=12):
        p = render.pose_carry(f, n)
        p['load_kind'] = kind
        return p
    return pose


_POSES = {'idle': render.pose_idle, 'walk': render.pose_walk,
          'gather': render.pose_gather, 'carry': render.pose_carry,
          'build': _pose_build, 'attack': _pose_attack,
          'carry-wood': _carry_kind('wood'),
          'carry-gold': _carry_kind('gold'),
          'carry-stone': _carry_kind('stone'),
          'carry-food': _carry_kind('food')}


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
