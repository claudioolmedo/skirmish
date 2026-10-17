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
    # House materials (v1 cottage): warm plaster, dark timber frame,
    # terracotta roof, stone foundation/chimney — same earthy family
    # as the villager palette.
    'plaster':  hex2rgb('#e6d3a3'),
    'timber':   hex2rgb('#6e4b2c'),
    'roof':     hex2rgb('#c05a36'),
    'stone':    hex2rgb('#a39c8a'),
    'door':     hex2rgb('#54381f'),
    'window':   hex2rgb('#ffcf6e'),  # warm lit glass
    'wood':     hex2rgb('#a9803e'),  # raw construction timber / logs
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

def render_cell(tris, direction, cw=CELL, ch=CELL, ax=AX, ay=AY,
                directions=DIRECTIONS):
    """Render one posed model in one facing direction -> (ch,cw,4)
    uint8 RGBA. Direction d: facing angle = d*(360/directions) in
    world XY, from +X toward +Y (dir 0 faces +X = screen down-right).
    `directions` is the sheet's tier (8/16/32); the canvas size /
    anchor default to the unit-sprite conventions; buildings pass
    their own (see the house section below)."""
    size_w, size_h = cw * SS, ch * SS
    rgb = np.zeros((size_h, size_w, 3), dtype=np.float64)
    alp = np.zeros((size_h, size_w), dtype=np.float64)
    zbuf = np.full((size_h, size_w), -1e9)
    face = rot_z(direction * (360.0 / directions) - 90.0)  # local +Y -> facing
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
        sx = ax * SS + (w[:, 0] - w[:, 1]) * P
        sy = ay * SS + ((w[:, 0] + w[:, 1]) * 0.5 - w[:, 2]) * P
        dep = w[:, 0] + w[:, 1]                 # larger = nearer viewer
        x0, x1 = int(max(0, math.floor(sx.min()))), int(min(size_w - 1, math.ceil(sx.max())))
        y0, y1 = int(max(0, math.floor(sy.min()))), int(min(size_h - 1, math.ceil(sy.max())))
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
        for chan in range(3):
            plane = rgb[y0:y1 + 1, x0:x1 + 1, chan]
            plane[hit] = col[chan]
        ap = alp[y0:y1 + 1, x0:x1 + 1]
        ap[hit] = 1.0
    # Downscale SS x SS, premultiplied so edges have no dark halo.
    out = np.zeros((ch, cw, 4), dtype=np.float64)
    pm = rgb * alp[..., None]
    pm = pm.reshape(ch, SS, cw, SS, 3).mean(axis=(1, 3))
    am = alp.reshape(ch, SS, cw, SS).mean(axis=(1, 3))
    nz = am > 1e-6
    out[..., 3] = am
    out[nz, :3] = pm[nz] / am[nz, None]
    return (np.clip(out, 0, 1) * 255).astype(np.uint8)

def render_sheet(state, directions=DIRECTIONS):
    fn, frames = STATES[state]
    sheet = Image.new('RGBA', (frames * CELL, directions * CELL))
    for d in range(directions):
        for f in range(frames):
            cell = Image.fromarray(
                render_cell(pose_model(fn(f, frames)), d,
                            directions=directions), 'RGBA')
            sheet.paste(cell, (f * CELL, d * CELL))
    return sheet

# Quality tiers (SPRITE-PIPELINE.md §3): a tier is a re-render of the
# same model at a different direction count — never new art. LOW (8)
# lives at the assets-original root for back-compat; MEDIUM (16) and
# HIGH (32) live in tier16/ and tier32/ subfolders. The house has a
# single fixed orientation, so its stage images are identical at every
# tier; they are copied into each tier folder so the client can load
# every original sprite for a tier from one directory.
TIERS = (('tier16', 16), ('tier32', 32))

def save_sheet(img, path):
    """Save a tier16/32 sheet palette-optimized. The art is
    posterized flat shading (a handful of materials x 4 shade
    levels), so a 256-color libimagequant palette is visually
    lossless here (measured: 0.013% of opaque pixels off by >8/255,
    all on material boundaries; alpha exact) and cuts the big HIGH
    sheets roughly in half vs truecolor. Falls back to a plain
    truecolor save if libimagequant is unavailable."""
    if img.mode == 'RGBA':
        try:
            img = img.quantize(colors=256, method=Image.LIBIMAGEQUANT,
                               dither=Image.Dither.NONE)
        except Exception:
            pass  # no libimagequant in this Pillow build: truecolor
    img.save(path, optimize=True)

# ======================================================================
# The house (building #1, replacement order #2). Original cottage
# design in the villager's palette family: stone foundation, timber
# frame with warm plaster infill, terracotta gable roof, lit windows,
# stone chimney. Model space: origin at the center of the sim's
# 2x2-cell footprint (CELL_SIZE = 4 world units, so the footprint is
# 8x8), +Y = front (door side), +Z up. Buildings have ONE fixed
# orientation in the world, so each stage is a single render at
# direction 2 (identity rotation = the game's view of the world) on a
# larger canvas: 896x704, ground anchor (footprint center) at
# (448, 448). These constants are the draw contract for the client
# integration; see web-preview/assets-original/README.md.
# ======================================================================
HOUSE_W_PX, HOUSE_H_PX = 896, 704
HOUSE_AX, HOUSE_AY = 448, 448

def shift(tris, dx, dy, dz):
    off = np.array([dx, dy, dz], dtype=np.float64)
    return [(v + off, m) for v, m in tris]

def spin_x(tris, deg, pivot=(0.0, 0.0, 0.0)):
    mat = rot_x(deg)
    return [(xform(v, mat, pivot), m) for v, m in tris]

# Roof geometry: ridge along X at z = 7.0, eaves at |y| = 4.35,
# z = 4.02. Slope rise/run give the pitch; slabs and rafters share it.
ROOF_RISE, ROOF_RUN = 2.98, 4.35
ROOF_PITCH = math.degrees(math.atan2(ROOF_RISE, ROOF_RUN))   # ~34.4 deg
ROOF_SLOPE = math.hypot(ROOF_RISE, ROOF_RUN)                 # ~5.27
ROOF_CY, ROOF_CZ = ROOF_RUN / 2.0, (7.0 + 4.02) / 2.0        # slope center

def roof_slab(sign):
    """One pitched roof slab; sign +1 = front (+Y) slope, -1 = back."""
    tris = box_tris(0, 0, -0.13, 0.13, 8.7, ROOF_SLOPE, mat='roof')
    tris = spin_x(tris, -sign * ROOF_PITCH)
    return shift(tris, 0, sign * ROOF_CY, ROOF_CZ)

def build_house(stage):
    """House triangles for construction stage 0..3:
    0 foundation, 1 timber frame + scaffold, 2 walls + roof rising,
    3 complete. Later stages include the earlier structure.
    Frame faces sit a decisive 0.14 proud of the plaster panels —
    coplanar faces z-fight in this renderer, so timber always
    overlaps plaster, never touches it."""
    t = []
    # --- Foundation (every stage): stone slab + low curb ------------
    t += box_tris(0, 0, 0.0, 0.38, 8.2, 8.2, mat='stone')
    if stage == 0:
        t += box_tris(0, 3.9, 0.38, 0.83, 7.8, 0.4, mat='stone')
        t += box_tris(0, -3.9, 0.38, 0.83, 7.8, 0.4, mat='stone')
        t += box_tris(3.9, 0, 0.38, 0.83, 0.4, 7.8, mat='stone')
        t += box_tris(-3.9, 0, 0.38, 0.83, 0.4, 7.8, mat='stone')
        # Corner stub posts + loose stone blocks: the first marks of
        # the build, so stage 0 reads as a started site, not a patio.
        for sx_ in (-1, 1):
            for sy_ in (-1, 1):
                t += box_tris(3.28 * sx_, 3.28 * sy_, 0.38, 1.35, 0.5, 0.5, mat='timber')
        t += box_tris(-2.6, 1.8, 0.38, 1.14, 0.76, 0.76, mat='stone')
        t += box_tris(-1.7, 1.95, 0.38, 1.0, 0.62, 0.62, mat='stone')
    # --- Log pile: building material on the foundation (stages 0-1) -
    if stage <= 1:
        t += box_tris(2.45, 2.75, 0.38, 0.8, 2.5, 0.42, mat='wood')
        t += box_tris(2.45, 3.22, 0.38, 0.8, 2.5, 0.42, mat='wood')
        t += box_tris(2.45, 2.98, 0.8, 1.22, 2.5, 0.42, mat='wood')
    if stage == 0:
        return t
    # --- Timber frame (stage 1+): floor, sills, posts, top plates ---
    t += box_tris(0, 0, 0.38, 0.54, 7.2, 7.2, mat='wood')            # floor
    for sx_ in (-1, 1):
        for sy_ in (-1, 1):                                          # corners
            t += box_tris(3.28 * sx_, 3.28 * sy_, 0.38, 4.5, 0.56, 0.56, mat='timber')
        t += box_tris(3.28 * sx_, 0, 0.38, 0.78, 0.56, 7.0, mat='timber')  # sill X-side
        t += box_tris(0, 3.28 * sx_, 0.38, 0.78, 7.0, 0.56, mat='timber')  # sill Y-side
        t += box_tris(3.28 * sx_, 0, 4.28, 4.62, 0.56, 7.0, mat='timber')  # plate X-side
        t += box_tris(0, 3.28 * sx_, 4.28, 4.62, 7.0, 0.56, mat='timber')  # plate Y-side
        t += box_tris(3.28 * sx_, 0, 0.38, 4.45, 0.56, 0.4, mat='timber')  # mid post X
        t += box_tris(0, 3.28 * sx_, 0.38, 4.45, 0.4, 0.56, mat='timber')  # mid post Y
    if stage == 1:
        # Scaffold on the front (+Y): poles, two ledgers, a plank deck.
        for px in (-3.3, 0.0, 3.3):
            t += box_tris(px, 4.35, 0.0, 4.9, 0.22, 0.22, mat='wood')
        t += box_tris(0, 4.35, 1.7, 1.88, 7.0, 0.13, mat='wood')
        t += box_tris(0, 4.35, 3.5, 3.68, 7.0, 0.13, mat='wood')
        t += box_tris(0, 4.35, 3.68, 3.82, 7.0, 0.9, mat='wood')
        return t
    # --- Plaster walls between the frame (stage 2+), inset so the
    # timber reads as a frame ON the wall -----------------------------
    t += box_tris(0, 3.23, 0.38, 4.3, 6.6, 0.38, mat='plaster')
    t += box_tris(0, -3.23, 0.38, 4.3, 6.6, 0.38, mat='plaster')
    t += box_tris(3.23, 0, 0.38, 4.3, 0.38, 6.6, mat='plaster')
    t += box_tris(-3.23, 0, 0.38, 4.3, 0.38, 6.6, mat='plaster')
    # Gable ends: tapered box whose top edge collapses to the ridge.
    t += box_tris(0, 0, 4.35, 6.8, 6.84, 6.9, 6.84, 0.12, mat='plaster')
    # Door (front, left of center): panel proud of the wall, lintel,
    # threshold stone. Windows: timber frame + warm lit pane.
    t += box_tris(-1.7, 3.56, 0.38, 2.62, 1.5, 0.14, mat='door')
    t += box_tris(-1.7, 3.56, 2.62, 2.86, 1.8, 0.2, mat='timber')
    t += box_tris(-1.7, 3.62, 0.38, 0.52, 1.7, 0.75, mat='stone')
    t += box_tris(1.75, 3.54, 2.05, 3.15, 1.1, 0.16, mat='timber')   # front frame
    t += box_tris(1.75, 3.58, 2.2, 3.0, 0.86, 0.1, mat='window')     # front pane
    t += box_tris(3.54, -1.4, 2.05, 3.15, 0.16, 1.1, mat='timber')   # side frame
    t += box_tris(3.58, -1.4, 2.2, 3.0, 0.1, 0.86, mat='window')     # side pane
    # --- Roof: front slope covered first (stage 2), back slope is
    # bare rafters until completion ----------------------------------
    t += roof_slab(+1)
    if stage == 2:
        for rx in (-3.5, -1.75, 0.0, 1.75, 3.5):
            r = box_tris(0, 0, -0.09, 0.09, 0.3, ROOF_SLOPE, mat='timber')
            r = spin_x(r, ROOF_PITCH)
            t += shift(r, rx, -ROOF_CY, ROOF_CZ - 0.1)
        return t
    # --- Complete: back slope, chimney + cap, window shutters -------
    t += roof_slab(-1)
    t += box_tris(2.3, 0, 5.4, 7.95, 1.15, 1.15, mat='stone')
    t += box_tris(2.3, 0, 7.95, 8.2, 1.5, 1.5, mat='stone')
    t += box_tris(0.97, 3.53, 2.05, 3.15, 0.42, 0.08, mat='timber')
    t += box_tris(2.53, 3.53, 2.05, 3.15, 0.42, 0.08, mat='timber')
    return t

HOUSE_STAGES = [(0, 'stage0'), (1, 'stage1'), (2, 'stage2'), (3, 'complete')]

def render_house(stage):
    cell = render_cell(build_house(stage), 2,
                       cw=HOUSE_W_PX, ch=HOUSE_H_PX,
                       ax=HOUSE_AX, ay=HOUSE_AY)
    return Image.fromarray(cell, 'RGBA')

def main():
    ap = argparse.ArgumentParser(description='Skirmish sprite pipeline — villager v1 + house v1')
    ap.add_argument('--player-color', default='#2e6fd8',
                    help='base color of the player-color (tunic) palette')
    ap.add_argument('--out', default=None,
                    help='output dir (default: web-preview/assets-original)')
    ap.add_argument('--only', choices=('all', 'villager', 'house'),
                    default='all', help='render only one model family')
    ap.add_argument('--tiers', action='store_true',
                    help='also render the MEDIUM (16) and HIGH (32) '
                         'direction tiers into tier16/ and tier32/ '
                         'subfolders, palette-optimized')
    args = ap.parse_args()
    MATERIALS['tunic'] = hex2rgb(args.player_color)
    root = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
    out = args.out or os.path.join(root, 'web-preview', 'assets-original')
    os.makedirs(out, exist_ok=True)
    if args.only in ('all', 'villager'):
        for state in STATES:
            img = render_sheet(state)
            path = os.path.join(out, f'villager-{state}.png')
            img.save(path)
            print(f'{path}: {img.width}x{img.height} '
                  f'({STATES[state][1]} frames x {DIRECTIONS} directions)')
        print(f'player color: {args.player_color}')
        if args.tiers:
            for folder, dirs in TIERS:
                tdir = os.path.join(out, folder)
                os.makedirs(tdir, exist_ok=True)
                for state in STATES:
                    img = render_sheet(state, dirs)
                    path = os.path.join(tdir, f'villager-{state}.png')
                    save_sheet(img, path)
                    print(f'{path}: {img.width}x{img.height} '
                          f'({STATES[state][1]} frames x {dirs} directions, '
                          f'{os.path.getsize(path)} bytes)')
    if args.only in ('all', 'house'):
        for stage, name in HOUSE_STAGES:
            img = render_house(stage)
            path = os.path.join(out, f'house-{name}.png')
            img.save(path)
            print(f'{path}: {img.width}x{img.height} (construction stage {stage})')
            if args.tiers:
                # Single fixed orientation: the stage art is the same
                # at every tier — copy it so each tier folder is a
                # complete, self-contained sprite set for the client.
                import shutil
                for folder, _dirs in TIERS:
                    tdir = os.path.join(out, folder)
                    os.makedirs(tdir, exist_ok=True)
                    dst = os.path.join(tdir, f'house-{name}.png')
                    shutil.copyfile(path, dst)
                    print(f'{dst}: tier copy ({os.path.getsize(dst)} bytes)')

if __name__ == '__main__':
    main()
