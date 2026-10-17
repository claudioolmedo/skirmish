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
