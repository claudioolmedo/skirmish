#!/usr/bin/env python3
"""
Skirmish sprite pipeline — directional unit/animal sheets.

Renders the models in tools/sprite-pipeline/models/<unit>.py into
sprite sheets for the directional-sprite milestone: rows = 8 facing
directions (dir 0 faces +X world, increasing toward +Y — the client
quantizes the facing angle the same way), columns = animation
frames, cell/anchor per the pipeline contract (128x128, feet-center
at (64, 110), 46 px per world unit, 2x supersampled).

For every sheet with player-color materials it also renders a
`<unit>-<state>-pcmask.png` twin: player-color pixels grayscale at
their posterized shade level, everything else black, alpha/z-buffer
identical to the color pass. The client builds the red player's
variant by mapping masked pixels from the blue base ramp to the red
base ramp at the same shade — DE's palette-remap mechanism, with
PNGs instead of a hardware palette.

    python3 tools/sprite-pipeline/units.py [unit ...]
        [--out DIR] [--contact DIR] [--player-color RRGGBB]

With no unit names, renders every registered model. Contact sheets
(one row per state, one column per direction, mid-cycle frame) go to
--contact DIR for eyeball QA; they are working files, not game art.
"""
import argparse
import importlib
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

import numpy as np
from PIL import Image

import render
from render import DIRECTIONS, hex2rgb

REGISTRY = ('villager', 'archer', 'militia', 'maa', 'scout',
            'sheep', 'boar', 'deer')
DEFAULT_CANVAS = (128, 128, 64, 110)  # cw, ch, ax, ay


def load_unit(name):
    mod = importlib.import_module(f'models.{name}')
    extra = getattr(mod, 'MATERIALS_ADD', None)
    if extra:
        render.MATERIALS.update(extra)
    return mod


def render_sheet(mod, state, mask=False):
    frames = mod.STATES[state]
    cw, ch, ax, ay = getattr(mod, 'CANVAS', DEFAULT_CANVAS)
    sheet = Image.new('RGBA', (frames * cw, DIRECTIONS * ch))
    worst_margin = 99
    for d in range(DIRECTIONS):
        for f in range(frames):
            tris = mod.tris_for(state, f)
            cell = render.render_cell(
                tris, d, cw=cw, ch=ch, ax=ax, ay=ay,
                mask_mats=mod.PC_MATS if mask else None)
            img = Image.fromarray(cell, 'RGBA')
            sheet.paste(img, (f * cw, d * ch))
            if not mask:
                a = cell[..., 3]
                ys, xs = np.nonzero(a > 8)
                if len(xs):
                    m = min(xs.min(), ys.min(),
                            cw - 1 - xs.max(), ch - 1 - ys.max())
                    worst_margin = min(worst_margin, int(m))
    return sheet, worst_margin


def contact_sheet(mod, name):
    """Rows = states, columns = the 8 directions, mid-cycle frame."""
    cw, ch, ax, ay = getattr(mod, 'CANVAS', DEFAULT_CANVAS)
    states = list(mod.STATES)
    grid = Image.new('RGBA', (DIRECTIONS * cw, len(states) * ch),
                     (24, 24, 28, 255))
    for r, state in enumerate(states):
        f = mod.STATES[state] // 2
        tris = mod.tris_for(state, f)
        for d in range(DIRECTIONS):
            cell = render.render_cell(tris, d, cw=cw, ch=ch, ax=ax, ay=ay)
            grid.paste(Image.fromarray(cell, 'RGBA'), (d * cw, r * ch),
                       Image.fromarray(cell, 'RGBA'))
    return grid


def main():
    ap = argparse.ArgumentParser(description='Directional sprite renderer')
    ap.add_argument('units', nargs='*', default=None)
    ap.add_argument('--out', default=None,
                    help='default: web-preview/assets-original')
    ap.add_argument('--contact', default=None,
                    help='also write <unit>-contact.png sheets here')
    ap.add_argument('--player-color', default='#2e6fd8')
    args = ap.parse_args()
    render.MATERIALS['tunic'] = hex2rgb(args.player_color)
    render.MATERIALS.setdefault('cloth', hex2rgb(args.player_color))
    root = os.path.dirname(os.path.dirname(
        os.path.dirname(os.path.abspath(__file__))))
    out = args.out or os.path.join(root, 'web-preview', 'assets-original')
    os.makedirs(out, exist_ok=True)
    names = args.units or list(REGISTRY)
    for name in names:
        mod = load_unit(name)
        for state in mod.STATES:
            sheet, margin = render_sheet(mod, state)
            path = os.path.join(out, f'{name}-{state}.png')
            sheet.save(path)
            note = '' if margin >= 3 else f'  MARGIN {margin}px (<3!)'
            print(f'{path}: {sheet.width}x{sheet.height}{note}')
            if mod.PC_MATS:
                mask, _ = render_sheet(mod, state, mask=True)
                mpath = os.path.join(out, f'{name}-{state}-pcmask.png')
                mask.save(mpath)
                print(f'{mpath}: player-color mask')
        if args.contact:
            os.makedirs(args.contact, exist_ok=True)
            cs = contact_sheet(mod, name)
            cpath = os.path.join(args.contact, f'{name}-contact.png')
            cs.save(cpath)
            print(f'{cpath}: contact sheet {cs.width}x{cs.height}')


if __name__ == '__main__':
    main()
