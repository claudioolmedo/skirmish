#!/usr/bin/env python3
"""
Skirmish original sprite pipeline — villager v1.

Renders an original, procedurally-built low-poly humanoid from the fixed
2:1 dimetric isometric camera (the same projection as web-preview/app.js)
into 2D sprite sheets: one PNG per animation state, rows = 8 facing
directions, columns = animation frames.

    python3 tools/sprite-pipeline/render.py [--player-color RRGGBB] [--out DIR]

The model is 100% our own: boxes and tapered boxes (frustums) assembled
in code, posed by joint angles. No external art, no reference models.
Software renderer: numpy z-buffered triangle rasterizer with flat
shading, supersampled 2x and downscaled. See SPRITE-PIPELINE.md in the
goal files for the full method spec; the constants below are mirrored
in web-preview/app.js (VIL_ORIGINAL) and must be kept in sync.
"""
import argparse
import math
import os

import numpy as np
from PIL import Image

# ----------------------------------------------------------------------
# Sheet / camera constants (mirror of SPRITE-PIPELINE.md + app.js)
# ----------------------------------------------------------------------
CELL = 128            # sprite cell size in px (square)
AX, AY = 64, 110      # feet-center anchor inside the cell, in px
PX_PER_UNIT = 46.0    # render resolution: screen px per world unit (Z)
SS = 2                # supersampling factor
DIRECTIONS = 8        # LOW tier: 8 facings, 45 degrees apart

# World-space light direction (unit vector pointing from the surface
# toward the light): screen upper-left and above. Fixed in world space,
# so a unit's shading rotates with its facing, like a real sun.
LIGHT = np.array([-0.52, 0.28, 0.81], dtype=np.float64)
LIGHT /= np.linalg.norm(LIGHT)
AMBIENT = 0.47
SHADE_STEPS = (0.47, 0.64, 0.81, 1.0)  # posterized flat-shading levels

# ----------------------------------------------------------------------
# Materials (base RGB, 0..1). The tunic uses the player-color ramp:
# a dedicated palette derived from one base color, so sheets can be
# regenerated per player (or palette-swapped at runtime later).
# ----------------------------------------------------------------------
def hex2rgb(h):
    h = h.lstrip('#')
    return tuple(int(h[i:i + 2], 16) / 255.0 for i in (0, 2, 4))

MATERIALS = {
    'skin':     hex2rgb('#d9a066'),
    'hair':     hex2rgb('#4a3220'),
    'trousers': hex2rgb('#5a4a3a'),
    'boots':    hex2rgb('#33241a'),
    'belt':     hex2rgb('#4a2f1d'),
    'load':     hex2rgb('#a9803e'),  # generic wooden bundle for carry
    'tunic':    None,                # filled from --player-color
}

# ----------------------------------------------------------------------
# Geometry helpers. Model space: +X right, +Y forward (facing), +Z up.
# A "part" is a tapered box: bottom/top rectangles at z0/z1 with sizes
# (sx0, sy0) -> (sx1, sy1), centered on (cx, cy). Parts belong to a
# group (legL, legR, armL, armR, upper, root) so poses can rotate them
# about joint pivots.
# ----------------------------------------------------------------------
def rot_x(deg):
    """Rotation matrix about the +X axis (right-hand rule)."""
    a = math.radians(deg)
    c, s = math.cos(a), math.sin(a)
    return np.array([[1, 0, 0], [0, c, -s], [0, s, c]], dtype=np.float64)

def rot_z(deg):
    a = math.radians(deg)
    c, s = math.cos(a), math.sin(a)
    return np.array([[c, -s, 0], [s, c, 0], [0, 0, 1]], dtype=np.float64)

def xform(verts, mat, pivot):
    """Apply rotation `mat` about `pivot` to an (N,3) vertex array."""
    p = np.asarray(pivot, dtype=np.float64)
    return (verts - p) @ mat.T + p

def box_tris(cx, cy, z0, z1, sx0, sy0, sx1=None, sy1=None, mat='skin'):
    """Tapered box -> list of (verts(3,3), material). 12 triangles."""
    if sx1 is None: sx1 = sx0
    if sy1 is None: sy1 = sy0
    b = [(cx - sx0 / 2, cy - sy0 / 2, z0), (cx + sx0 / 2, cy - sy0 / 2, z0),
         (cx + sx0 / 2, cy + sy0 / 2, z0), (cx - sx0 / 2, cy + sy0 / 2, z0)]
    t = [(cx - sx1 / 2, cy - sy1 / 2, z1), (cx + sx1 / 2, cy - sy1 / 2, z1),
         (cx + sx1 / 2, cy + sy1 / 2, z1), (cx - sx1 / 2, cy + sy1 / 2, z1)]
    v = [np.array(p, dtype=np.float64) for p in b + t]
    faces = [(0, 1, 2), (0, 2, 3),          # bottom
             (4, 6, 5), (4, 7, 6),          # top
             (0, 5, 1), (0, 4, 5),          # back  (-Y)
             (2, 7, 3), (2, 6, 7),          # front (+Y)
             (1, 6, 2), (1, 5, 6),          # right (+X)
             (3, 4, 0), (3, 7, 4)]          # left  (-X)
    return [(np.array([v[a], v[bb], v[c]]), mat) for a, bb, c in faces]

# ----------------------------------------------------------------------
# The villager model (original design, built from tapered boxes).
#
# Proportions (world units, total height ~1.66):
#   boots 0.00-0.10, legs 0.08-0.82, tunic skirt 0.72-0.98,
#   belt 0.93-1.02, torso 0.98-1.34, head 1.35-1.65, hair cap on top.
# Groups: legL/legR swing about hip pivots (+-0.11, 0, 0.82);
# armL/armR swing about shoulder pivots (+-0.28, 0, 1.30);
# 'upper' parts (torso, skirt, belt, arms, head, load) also pitch
# about the waist pivot (0, 0, 0.92).
# ----------------------------------------------------------------------
HIP_Y, HIP_Z = 0.0, 0.82
SH_X, SH_Z = 0.28, 1.30
WAIST = (0.0, 0.0, 0.92)

def build_villager():
    """Return list of (tris, group) in model space, unposed."""
    parts = []  # (tris, group)
    def add(tris, group):
        parts.append((tris, group))
    for side, sgn in (('legL', -1), ('legR', 1)):
        x = 0.11 * sgn
        # leg: tapered trouser leg, hip -> ankle
        add(box_tris(x, 0.0, 0.08, HIP_Z, 0.13, 0.15, 0.16, 0.18, 'trousers'), side)
        # boot: slightly forward, darker
        add(box_tris(x, 0.03, 0.0, 0.11, 0.14, 0.24, 0.14, 0.22, 'boots'), side)
    for side, sgn in (('armL', -1), ('armR', 1)):
        x = 0.30 * sgn
        # sleeve (player color), shoulder -> elbow
        add(box_tris(x, 0.0, 1.05, 1.33, 0.115, 0.125, 0.125, 0.135, 'tunic'), side)
        # forearm + hand (skin), elbow -> hand
        add(box_tris(x, 0.0, 0.76, 1.07, 0.09, 0.10, 0.10, 0.115, 'skin'), side)
    # tunic skirt (flared), torso, belt — player color clothing
    add(box_tris(0, 0, 0.72, 0.99, 0.47, 0.36, 0.415, 0.295, 'tunic'), 'upper')
    add(box_tris(0, 0, 0.98, 1.34, 0.40, 0.275, 0.435, 0.30, 'tunic'), 'upper')
    add(box_tris(0, 0, 0.93, 1.02, 0.45, 0.315, 0.45, 0.315, 'belt'), 'upper')
    # neck + head + hair cap
    add(box_tris(0, 0, 1.30, 1.42, 0.12, 0.12, 0.12, 0.12, 'skin'), 'upper')
    add(box_tris(0, 0.01, 1.36, 1.65, 0.27, 0.26, 0.25, 0.245, 'skin'), 'upper')
    add(box_tris(0, -0.018, 1.585, 1.685, 0.295, 0.285, 0.27, 0.26, 'hair'), 'upper')
    return parts

LOAD_TRIS = box_tris(0, 0.0, 1.72, 1.97, 0.50, 0.38, 0.43, 0.33, 'load')
VILLAGER = build_villager()
PIVOTS = {'legL': (-0.11, HIP_Y, HIP_Z), 'legR': (0.11, HIP_Y, HIP_Z),
          'armL': (-SH_X, 0.0, SH_Z), 'armR': (SH_X, 0.0, SH_Z)}

def pose_model(pose):
    """Apply a pose dict to the villager; return flat tri list.
    Pose keys (degrees, + = swing/pitch forward): legL, legR, armL,
    armR, torso (pitch forward), bob (world units, + up), load (bool).
    """
    out = []
    torso_mat = rot_x(-pose.get('torso', 0.0))  # pitch fwd = -rot about X
    for tris, group in VILLAGER:
        for verts, mat in tris:
            v = verts
            if group in PIVOTS:
                v = xform(v, rot_x(pose.get(group, 0.0)), PIVOTS[group])
            if group in ('upper', 'armL', 'armR'):
                v = xform(v, torso_mat, WAIST)
            v = v + np.array([0.0, 0.0, pose.get('bob', 0.0)])
            out.append((v, mat))
    if pose.get('load'):
        for verts, mat in LOAD_TRIS:
            v = xform(verts, torso_mat, WAIST)
            v = v + np.array([0.0, 0.0, pose.get('bob', 0.0)])
            out.append((v, mat))
    return out

# ----------------------------------------------------------------------
# Poses. Walk/idle/gather/carry, frame counts per SPRITE-PIPELINE.md.
# ----------------------------------------------------------------------
def pose_idle(f, n=4):
    t = f / n
    breathe = math.sin(2 * math.pi * t)
    return {'legL': 2.0, 'legR': -2.0,
            'armL': 2.0 * breathe, 'armR': -2.0 * breathe,
            'torso': 1.5, 'bob': 0.012 * breathe, 'load': False}

def pose_walk(f, n=8):
    t = f / n
    s = math.sin(2 * math.pi * t)
    return {'legL': 30.0 * s, 'legR': -30.0 * s,
            'armL': -21.0 * s, 'armR': 21.0 * s,
            'torso': 5.0, 'bob': -0.085 * abs(s) + 0.02, 'load': False}

def pose_carry(f, n=8):
    p = pose_walk(f, n)
    p.update({'armL': 152.0, 'armR': 152.0, 'torso': 2.5, 'load': True})
    return p

def pose_gather(f, n=4):
    # Chop cycle: raise back, strike down-forward, impact, recover.
    arm = (-70.0, 30.0, 58.0, -12.0)[f % n]
    torso = (5.0, 18.0, 30.0, 12.0)[f % n]
    return {'legL': 12.0, 'legR': -10.0,
            'armL': -6.0, 'armR': arm,
            'torso': torso, 'bob': 0.0, 'load': False}

STATES = {'idle': (pose_idle, 4), 'walk': (pose_walk, 8),
          'gather': (pose_gather, 4), 'carry': (pose_carry, 8)}

# ----------------------------------------------------------------------
# Software renderer: orthographic 2:1 dimetric projection (identical to
# web-preview/app.js w2s), z-buffered flat-shaded triangles.
# ----------------------------------------------------------------------
def shade_of(normal):
    d = max(0.0, float(normal @ LIGHT))
    b = AMBIENT + (1.0 - AMBIENT) * d
    return min(SHADE_STEPS, key=lambda s: abs(s - b))

def render_cell(tris, direction):
    """Render one posed model in one facing direction -> (CELL,CELL,4)
    uint8 RGBA. Direction d: facing angle = d*45deg in world XY, from
    +X toward +Y (dir 0 faces +X = screen down-right)."""
    size = CELL * SS
    rgb = np.zeros((size, size, 3), dtype=np.float64)
    alp = np.zeros((size, size), dtype=np.float64)
    zbuf = np.full((size, size), -1e9)
    face = rot_z(direction * 45.0 - 90.0)  # local +Y -> facing vector
    P = PX_PER_UNIT * SS
    for verts, mat in tris:
        w = verts @ face.T                      # world space
        n = np.cross(w[1] - w[0], w[2] - w[0])
        ln = np.linalg.norm(n)
        if ln < 1e-12:
            continue
        n /= ln
        base = np.array(MATERIALS[mat], dtype=np.float64)
        col = base * shade_of(n)
        sx = AX * SS + (w[:, 0] - w[:, 1]) * P
        sy = AY * SS + ((w[:, 0] + w[:, 1]) * 0.5 - w[:, 2]) * P
        dep = w[:, 0] + w[:, 1]                 # larger = nearer viewer
        x0, x1 = int(max(0, math.floor(sx.min()))), int(min(size - 1, math.ceil(sx.max())))
        y0, y1 = int(max(0, math.floor(sy.min()))), int(min(size - 1, math.ceil(sy.max())))
        if x1 < x0 or y1 < y0:
            continue
        ys, xs = np.mgrid[y0:y1 + 1, x0:x1 + 1]
        xs = xs.astype(np.float64); ys = ys.astype(np.float64)
        den = (sy[1] - sy[2]) * (sx[0] - sx[2]) + (sx[2] - sx[1]) * (sy[0] - sy[2])
        if abs(den) < 1e-9:
            continue
        l0 = ((sy[1] - sy[2]) * (xs - sx[2]) + (sx[2] - sx[1]) * (ys - sy[2])) / den
        l1 = ((sy[2] - sy[0]) * (xs - sx[2]) + (sx[0] - sx[2]) * (ys - sy[2])) / den
        l2 = 1.0 - l0 - l1
        inside = (l0 >= -1e-6) & (l1 >= -1e-6) & (l2 >= -1e-6)
        if not inside.any():
            continue
        d = l0 * dep[0] + l1 * dep[1] + l2 * dep[2]
        sub = zbuf[y0:y1 + 1, x0:x1 + 1]
        hit = inside & (d > sub)
        if not hit.any():
            continue
        sub[hit] = d[hit]
        for ch in range(3):
            plane = rgb[y0:y1 + 1, x0:x1 + 1, ch]
            plane[hit] = col[ch]
        ap = alp[y0:y1 + 1, x0:x1 + 1]
        ap[hit] = 1.0
    # Downscale SS x SS, premultiplied so edges have no dark halo.
    out = np.zeros((CELL, CELL, 4), dtype=np.float64)
    pm = rgb * alp[..., None]
    pm = pm.reshape(CELL, SS, CELL, SS, 3).mean(axis=(1, 3))
    am = alp.reshape(CELL, SS, CELL, SS).mean(axis=(1, 3))
    nz = am > 1e-6
    out[..., 3] = am
    out[nz, :3] = pm[nz] / am[nz, None]
    return (np.clip(out, 0, 1) * 255).astype(np.uint8)

def render_sheet(state):
    fn, frames = STATES[state]
    sheet = Image.new('RGBA', (frames * CELL, DIRECTIONS * CELL))
    for d in range(DIRECTIONS):
        for f in range(frames):
            cell = Image.fromarray(render_cell(pose_model(fn(f, frames)), d), 'RGBA')
            sheet.paste(cell, (f * CELL, d * CELL))
    return sheet

def main():
    ap = argparse.ArgumentParser(description='Skirmish sprite pipeline — villager v1')
    ap.add_argument('--player-color', default='#2e6fd8',
                    help='base color of the player-color (tunic) palette')
    ap.add_argument('--out', default=None,
                    help='output dir (default: web-preview/assets-original)')
    args = ap.parse_args()
    MATERIALS['tunic'] = hex2rgb(args.player_color)
    root = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
    out = args.out or os.path.join(root, 'web-preview', 'assets-original')
    os.makedirs(out, exist_ok=True)
    for state in STATES:
        img = render_sheet(state)
        path = os.path.join(out, f'villager-{state}.png')
        img.save(path)
        print(f'{path}: {img.width}x{img.height} '
              f'({STATES[state][1]} frames x {DIRECTIONS} directions)')
    print(f'player color: {args.player_color}')

if __name__ == '__main__':
    main()
