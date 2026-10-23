"""Scout Cavalry — directional milestone model.

Original design: a stocky horse with tapered chest/rump masses, an
angled neck, and a villager-proportioned rider in player-color tunic
with a player-color saddle blanket. Flat-shaded tapered boxes only,
posed in code — no external art.

Module contract (shared by every models/<unit>.py):
  MATERIALS_ADD  new materials this model needs
  PC_MATS        set of material names carrying player color
  STATES         {state: frame_count} at the LOW tier (8 directions)
  tris_for(state, frame) -> flat [(verts, material)] posed, model space
  CANVAS         (cw, ch, ax, ay); the horse + rider need the big cell

Model space: +X right, +Y forward (the head end), +Z up, origin at
the ground center under the body. Canvas notes: in the fixed
dimetric projection a ground point at (x, y) renders (x + y) / 2
units below the anchor, so the legs sit at (+-0.18, +-0.48) with
slim hooves and the gallop keeps a suspension bob; and in the
diagonal directions the screen width is 1.414 * the model's Y
extent, so nose and tail stay within |y| ~1.18. Together they hold
the >= 3 px margin on the 160 px cell in every direction.
"""
import math

import numpy as np

import render
from render import box_tris, hex2rgb, rot_x, rot_z, xform

MATERIALS_ADD = {'horse': hex2rgb('#6e4f33'),
                 'horse_dark': hex2rgb('#4e3823')}
PC_MATS = {'tunic', 'cloth'}
STATES = {'idle': 4, 'gallop': 12, 'die': 6}
CANVAS = (160, 160, 80, 138)
# Death cell: horse + rider tipped sideways lie a full body
# length from the ground point — bigger than the standing cell.
STATE_CANVAS = {'die': (256, 216, 128, 156)}
_FALL_SIDE = (0.0, 9.0, 24.0, 46.0, 66.0, 78.0)

LEG_X, LEG_Y, LEG_PIVOT_Z = 0.18, 0.48, 0.80
BODY_PIVOT = (0.0, 0.0, 1.05)
NECK_BASE = (0.0, 0.62, 1.26)
NECK_PITCH = -29.0  # neck column tips forward from vertical


def _xf(tris, mat, pivot):
    return [(xform(v, mat, pivot), m) for v, m in tris]


def _sh(tris, dx, dy, dz):
    off = np.array([dx, dy, dz], dtype=np.float64)
    return [(v + off, m) for v, m in tris]


def _body_xform(tris, pitch, bob):
    """Pitch the body group about the barrel pivot, then bob it."""
    if pitch:
        tris = _xf(tris, rot_x(pitch), BODY_PIVOT)
    if bob:
        tris = _sh(tris, 0.0, 0.0, bob)
    return tris


def _leg(sx, sy, angle, bob):
    """One horse leg: upper in coat color, slim dark lower + hoof.
    Swings about its top pivot; positive angle swings forward."""
    x, y = LEG_X * sx, LEG_Y * sy
    tris = box_tris(x, y, 0.44, 0.84, 0.15, 0.18, 0.17, 0.20, 'horse')
    tris += box_tris(x, y, 0.0, 0.46, 0.09, 0.11, 0.13, 0.15,
                     'horse_dark')
    if angle:
        tris = _xf(tris, rot_x(angle), (x, y, LEG_PIVOT_Z))
    if bob:
        tris = _sh(tris, 0.0, 0.0, bob)
    return tris


def _neck_assembly(stretch, sway):
    """Neck + mane + head + ears, built in a local frame whose origin
    is the neck base, pitched forward, then placed on the body.
    `stretch` deepens the forward pitch (gallop reach); `sway` is a
    small yaw for idle life."""
    top = (0.0, 0.02, 0.56)  # neck-top point in the local frame
    head = []
    head += box_tris(0, 0.10, 0.38, 0.62, 0.23, 0.30, 0.21, 0.26,
                     'horse')                       # skull
    head += box_tris(0, 0.22, 0.32, 0.50, 0.155, 0.17, 0.135, 0.15,
                     'horse_dark')                  # muzzle
    for s in (-1, 1):                               # ears
        head += box_tris(0.085 * s, -0.05, 0.58, 0.74, 0.055, 0.07,
                         0.045, 0.06, 'horse_dark')
    head += box_tris(0, -0.06, 0.56, 0.66, 0.16, 0.11, 0.15, 0.10,
                     'horse_dark')                  # forelock
    head = _xf(head, rot_x(-14.0), top)  # head nods down off the neck
    tris = box_tris(0, 0.02, 0.0, 0.56, 0.27, 0.32, 0.22, 0.26,
                    'horse')                        # neck column
    tris += box_tris(0, -0.175, 0.02, 0.52, 0.07, 0.09, 0.06, 0.08,
                     'horse_dark')                  # mane ridge
    tris += head
    tris = _xf(tris, rot_x(NECK_PITCH - stretch), (0.0, 0.0, 0.0))
    if sway:
        tris = _xf(tris, rot_z(sway), (0.0, 0.0, 0.0))
    return _sh(tris, *NECK_BASE)


def _tail(sway):
    # Built hanging below its dock at the rump's top rear, tipped
    # back-down; the tip stays inside the diagonal-view width limit.
    tris = box_tris(0, 0, -0.55, 0.02, 0.06, 0.075, 0.085, 0.10,
                    'horse_dark')
    tris = _xf(tris, rot_x(-32.0), (0.0, 0.0, 0.0))
    if sway:
        tris = _xf(tris, rot_z(sway), (0.0, 0.0, 0.0))
    return _sh(tris, 0.0, -0.76, 1.36)


def _rider():
    """Seated rider, axis-aligned boxes hugging the barrel: thigh
    slabs over the barrel top, shins against its sides, boots forward.
    Right arm reaches forward-down to the reins, left arm hangs."""
    tris = []
    # Saddle + player-color blanket (blanket drapes over both sides).
    tris += box_tris(0, 0, 1.33, 1.45, 0.80, 0.66, 0.78, 0.64, 'cloth')
    tris += box_tris(0, 0, 1.44, 1.57, 0.36, 0.54, 0.32, 0.48, 'belt')
    # Torso (player color), neck, head, hair cap.
    tris += box_tris(0, -0.01, 1.54, 1.97, 0.34, 0.25, 0.37, 0.27,
                     'tunic')
    tris += box_tris(0, 0, 1.94, 2.03, 0.11, 0.11, 0.11, 0.11, 'skin')
    tris += box_tris(0, 0, 2.00, 2.26, 0.235, 0.225, 0.22, 0.21,
                     'skin')
    tris += box_tris(0, -0.016, 2.20, 2.30, 0.26, 0.25, 0.235, 0.225,
                     'hair')
    # Arms: sleeves in player color, forearms skin.
    tris += box_tris(0.255, 0.09, 1.70, 1.94, 0.105, 0.13, 0.115,
                     0.14, 'tunic')                 # right sleeve
    tris += box_tris(0.235, 0.26, 1.60, 1.74, 0.085, 0.22, 0.095,
                     0.13, 'skin')                  # right forearm
    tris += box_tris(-0.255, 0.0, 1.70, 1.94, 0.105, 0.125, 0.115,
                     0.135, 'tunic')                # left sleeve
    tris += box_tris(-0.265, 0.01, 1.44, 1.72, 0.085, 0.105, 0.095,
                     0.115, 'skin')                 # left forearm
    # Legs: thigh slab, shin against the barrel, boot forward.
    for s in (-1, 1):
        tris += box_tris(0.245 * s, 0.06, 1.34, 1.58, 0.13, 0.30,
                         0.14, 0.30, 'trousers')
        tris += box_tris(0.315 * s, 0.14, 0.98, 1.36, 0.105, 0.13,
                         0.11, 0.14, 'trousers')
        tris += box_tris(0.315 * s, 0.19, 0.90, 1.00, 0.11, 0.24,
                         0.11, 0.22, 'boots')
    return tris


def _body():
    tris = []
    tris += box_tris(0, 0, 0.74, 1.44, 0.62, 1.06, 0.60, 1.04,
                     'horse')                       # barrel
    tris += box_tris(0, 0.55, 0.80, 1.40, 0.55, 0.48, 0.53, 0.46,
                     'horse')                       # chest
    tris += box_tris(0, -0.55, 0.82, 1.46, 0.57, 0.48, 0.55, 0.46,
                     'horse')                       # rump
    return tris


# Gallop cycle, hand-tuned transverse gallop: extended suspension,
# front stance, gathered suspension, hind push. (front angle, hind
# angle, body pitch, bob, neck stretch). The bob keeps swung-forward
# hooves inside the cell's bottom margin in the front directions.
_GALLOP = [
    ( 14.0, -11.0, -2.5, 0.110,  2.5),
    (  4.0, -11.0, -1.5, 0.085,  1.5),
    ( -7.0,  -5.0,  0.0, 0.040,  0.0),
    (-13.0,   7.0,  1.5, 0.055, -1.5),
    ( -9.0,  13.0,  2.5, 0.095, -3.0),
    (  2.0,   9.0,  1.5, 0.060, -1.5),
    ( 10.0,  -2.0, -0.5, 0.080,  0.0),
    ( 14.0,  -9.0, -1.5, 0.105,  2.0),
]


def tris_for(state, frame):
    tris = []
    if state == 'idle':
        t = frame / STATES['idle']
        sway = 2.0 * math.sin(2 * math.pi * t)       # tiny head sway
        tail_sway = 5.0 * math.sin(2 * math.pi * t + 0.7)
        bob = 0.008 * math.sin(2 * math.pi * t)
        body = _body() + _rider() + _tail(tail_sway)
        body += _neck_assembly(0.0, sway)
        tris += _body_xform(body, 0.0, bob)
        for sx in (-1, 1):
            for sy in (-1, 1):
                tris += _leg(sx, sy, 1.5 * sy, bob)
        return tris
    if state == 'gallop':
        # The 8 hand-tuned keys sampled at the state's frame
        # count (render.key's mechanism, tuple-wise): at 8 frames
        # this is exactly the old table; at 12 the cycle is the
        # same motion, sampled finer.
        n = STATES['gallop']
        m = len(_GALLOP)
        pos = (frame % n) / n * m
        i0 = int(math.floor(pos)) % m
        frac = pos - math.floor(pos)
        ga, gb = _GALLOP[i0], _GALLOP[(i0 + 1) % m]
        front, hind, pitch, bob, stretch = (
            ga[k] + (gb[k] - ga[k]) * frac for k in range(5))
        off = 2.0 if i0 % 2 == 0 else -2.0  # slight pair offset
        body = _body() + _rider() + _tail(0.0)
        body += _neck_assembly(stretch, 0.0)
        tris += _body_xform(body, pitch, bob)
        for sx in (-1, 1):
            tris += _leg(sx, 1, front + off * sx, bob)
            tris += _leg(sx, -1, hind - off * sx, bob)
        return tris
    if state == 'die':
        # Horse and rider tip sideways as one mass — rotation
        # about the forward (Y) axis through the ground origin,
        # easing in; the last frame is the corpse the client
        # leaves fading on the field. The base assembly is the
        # idle frame-0 pose.
        body = _body() + _rider() + _tail(5.0 * math.sin(0.7))
        body += _neck_assembly(0.0, 0.0)
        tris += _body_xform(body, 0.0, 0.0)
        for sx in (-1, 1):
            for sy in (-1, 1):
                tris += _leg(sx, sy, 1.5 * sy, 0.0)
        mat = render.rot_y(_FALL_SIDE[frame % len(_FALL_SIDE)])
        return [(xform(v, mat, (0.0, 0.0, 0.0)), m) for v, m in tris]
    raise KeyError(state)
