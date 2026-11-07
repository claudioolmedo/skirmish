#!/usr/bin/env python3
"""Programmatic command icons for the buildings added after the
Base44 PNG set (2026-10-19): Wonder (VIC-02), Dock (Water W1) and
Fish Trap (Water W2b).

Why this exists: BUILD_PNG in web-preview/app.js names wonder.png,
dock.png and fish-trap.png, but no such files were ever added to
web-preview/play/ui-art/icons/ — the Economic build page rendered
two broken <img>s (blank Wonder, fragment-only Dock) and selecting
a Wonder/Dock/Fish Trap showed a broken portrait. Found live by the
bot loop (2026-11-06, confirmed 2026-11-07).

Style follows the Castle precedent (Castle P3a, 2026-10-29): an
original programmatic pictogram drawn in code — one large, readable
pictogram on the set's dark walnut background with its thin gold
frame, flat colours, 128x128. The pictograms depict Skirmish II's
own buildings as drawn by the game's SVG symbols (i-wonder, i-dock,
i-fish-trap in play/index.html) and the Dock's in-game art; no
reference-game artwork is copied.

Run: python3 tools/art/gen-building-icons.py
Writes into web-preview/play/ui-art/icons/.
"""
from PIL import Image, ImageDraw
import os

OUT = os.path.join(os.path.dirname(__file__), '..', '..',
                   'web-preview', 'play', 'ui-art', 'icons')

WALNUT = (42, 29, 18, 255)
GOLD = (201, 162, 39, 255)
GOLD_BRIGHT = (216, 179, 74, 255)
STONE_LIGHT = (179, 162, 132, 255)
STONE_MID = (168, 157, 136, 255)
STONE_DARK = (141, 130, 113, 255)
STONE_EDGE = (110, 98, 80, 255)
DARK_DOOR = (46, 42, 36, 255)
TIMBER = (138, 107, 66, 255)
TIMBER_DARK = (111, 85, 51, 255)
ROOF = (122, 62, 44, 255)
WATER = (74, 122, 150, 255)
WATER_DEEP = (63, 126, 166, 255)
NET = (233, 226, 205, 255)
CORK = (217, 160, 91, 255)


def base():
    img = Image.new('RGBA', (128, 128), WALNUT)
    d = ImageDraw.Draw(img)
    # Thin gold frame, the set's signature (castle.png carries the
    # same ~3px gold perimeter).
    d.rectangle([0, 0, 127, 127], outline=GOLD, width=3)
    d.rectangle([5, 5, 122, 122], outline=(90, 70, 30, 255), width=1)
    return img, d


def wonder():
    img, d = base()
    # Gold capstone.
    d.polygon([(64, 14), (76, 34), (52, 34)], fill=GOLD_BRIGHT, outline=(138, 109, 31, 255))
    # Three stepped stone tiers, widening downward (the game's
    # i-wonder silhouette).
    d.polygon([(50, 34), (78, 34), (84, 56), (44, 56)], fill=STONE_LIGHT, outline=STONE_EDGE)
    d.polygon([(40, 56), (88, 56), (94, 80), (34, 80)], fill=STONE_MID, outline=STONE_EDGE)
    d.polygon([(30, 80), (98, 80), (106, 110), (22, 110)], fill=STONE_DARK, outline=STONE_EDGE)
    # Tier seams.
    d.line([(44, 56), (84, 56)], fill=STONE_EDGE, width=2)
    d.line([(34, 80), (94, 80)], fill=STONE_EDGE, width=2)
    # Door arch at the base.
    d.pieslice([54, 88, 74, 112], 180, 360, fill=DARK_DOOR)
    d.rectangle([54, 98, 74, 110], fill=DARK_DOOR)
    return img


def dock():
    img, d = base()
    # Water at the foot of the pier.
    d.rectangle([8, 96, 120, 120], fill=WATER_DEEP)
    for y, off in ((102, 0), (110, 10), (117, 4)):
        for x in range(10 + off, 116, 28):
            d.arc([x, y, x + 16, y + 7], 180, 360, fill=NET, width=2)
    # Pier posts.
    for x in (26, 56, 86, 108):
        d.rectangle([x, 78, x + 6, 104], fill=TIMBER_DARK)
    # Deck planks.
    d.rectangle([14, 72, 116, 80], fill=TIMBER, outline=TIMBER_DARK)
    for x in range(30, 116, 16):
        d.line([(x, 72), (x, 80)], fill=TIMBER_DARK, width=2)
    # Hut with rust-red roof (the Dock's in-game colours).
    d.rectangle([22, 44, 52, 72], fill=(168, 154, 124, 255), outline=STONE_EDGE)
    d.polygon([(16, 44), (58, 44), (37, 28)], fill=ROOF, outline=(93, 44, 30, 255))
    d.rectangle([32, 56, 42, 72], fill=DARK_DOOR)
    # Loading crane: mast + jib + hanging hook line.
    d.rectangle([92, 22, 97, 72], fill=TIMBER_DARK)
    d.line([(94, 24), (66, 38)], fill=TIMBER_DARK, width=4)
    d.line([(72, 35), (72, 52)], fill=NET, width=2)
    d.arc([68, 50, 76, 60], 0, 180, fill=GOLD, width=2)
    return img


def fish_trap():
    img, d = base()
    # Water band at the bottom.
    d.rectangle([8, 100, 120, 120], fill=WATER_DEEP)
    for x in range(12, 112, 28):
        d.arc([x, 106, x + 16, 113], 180, 360, fill=NET, width=2)
    # Two posts with cork floats on top.
    for x in (30, 92):
        d.rectangle([x, 30, x + 6, 104], fill=TIMBER_DARK)
        d.ellipse([x - 3, 20, x + 9, 32], fill=CORK, outline=TIMBER_DARK)
    # Woven net: horizontal weeps + vertical strands between posts.
    for y in (44, 60, 76, 92):
        d.arc([33, y - 8, 95, y + 10], 0, 180, fill=NET, width=3)
    for x in (44, 57, 70, 83):
        d.line([(x, 34), (x, 100)], fill=NET, width=2)
    # A caught fish silhouette in the net.
    d.polygon([(52, 66), (68, 58), (76, 66), (68, 74)], fill=GOLD_BRIGHT)
    d.polygon([(52, 66), (44, 60), (44, 72)], fill=GOLD_BRIGHT)
    d.ellipse([66, 63, 70, 67], fill=DARK_DOOR)
    return img


def main():
    os.makedirs(OUT, exist_ok=True)
    for name, fn in (('wonder.png', wonder), ('dock.png', dock),
                     ('fish-trap.png', fish_trap)):
        img = fn()
        path = os.path.normpath(os.path.join(OUT, name))
        img.save(path)
        print('wrote', path, img.size)


if __name__ == '__main__':
    main()
