# Play-page UI art — provenance

All artwork in this folder is **original to Skirmish II**, generated for
this project with AI image tools and art-directed by hand (warm walnut
wood, aged gold, parchment, painterly medieval game icons). It evokes
the classic RTS genre style; it does **not** reproduce any asset,
icon, texture, or design from Age of Empires or any other game —
Skirmish II is a clean-room project.

| File | Used for |
|---|---|
| `tex-wood.webp` | Top resource bar + bottom bar backgrounds (dark walnut planks) |
| `tex-parchment.webp` | Selection panel + start/pause overlay cards |
| `medallion.webp` | Selection portrait frame (carved wood + worn gold rim) |
| `icon-food.webp` | Food resource / farm cost — golden wheat sheaf |
| `icon-wood.webp` | Wood resource / build costs — stacked logs |
| `icon-gold.webp` | Gold resource — nuggets and coins |
| `icon-stone.webp` | Stone resource — stone blocks |
| `icon-pop.webp` | Population — two hooded villagers |
| `icon-house.webp` | Build House command — timber cottage (matches the in-game house) |
| `icon-farm.webp` | Build Farm command — crop rows with sprouts |
| `icon-villager.webp` | Villager portrait + group cells (blue player-color tunic) |
| `icon-train.webp` | Train Villager command — villager + golden up-arrow |
| `icon-tc.webp` | Town Center portrait — stone keep, green banner |
| `icon-menu.webp` | Menu button — green heraldic banner |

Icons are 256×256 sources, downscaled by CSS. They are carried inside
the page's SVG `<symbol>` set (same ids as icon set v1) so `app.js`
hooks did not change. Total folder size ≈ 323 KB.

## `icons/` — the PNG command-icon set (2026-10-19)

Twenty 128×128 PNGs in `icons/`, generated via **Base44 on Claudio's
account** (2026-10-19) and sliced from two generated sheets. They are
**original drawings**, art-directed to speak the reference game's
icon language — one large, immediately readable pictogram per
button, three-quarter view, warm painterly light, on a dark walnut
background with a thin gold frame — using public documentation of
that language (the Age of Empires II building/unit tables) as the
reference. They are **not copies**: every pictogram depicts
Skirmish II's own buildings, units, and technologies as designed
for this game, and no reference-game artwork was traced, reused,
or reproduced.

| File | Used for |
|---|---|
| `icons/house.png` … `icons/blacksmith.png` | The nine villager build commands (House, Farm, Barracks, Lumber Camp, Mill, Mining Camp, Archery Range, Stable, Blacksmith) + building selection portraits |
| `icons/town-center.png` | Town Center selection portrait (own + enemy) |
| `icons/villager.png` | Train Villager command + villager portrait / group cells |
| `icons/militia.png`, `icons/man-at-arms.png` | Barracks: train Militia, Man-at-Arms upgrade + portraits |
| `icons/archer.png` | Archery Range: train Archer + portrait |
| `icons/scout.png` | Stable: train Scout Cavalry + portrait |
| `icons/loom.png`, `icons/feudal.png` | Town Center researches: Loom, Feudal Age |
| `icons/fletching.png`, `icons/scale-mail.png`, `icons/padded-armor.png` | Blacksmith researches |

Where a PNG exists it replaces the older SVG symbol in command
buttons and selection portraits (`BUILD_PNG` / `KIND_PNG` /
`pngIcon` in `app.js`); the symbols remain for resources, animals,
and anything unmapped. Set size ≈ 427 KB.
