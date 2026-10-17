# Skirmish II — Mechanics Study (Public Sources Only)

**Date:** 2026-10-16
**Purpose:** Implementation reference for Skirmish II milestone M3 (combat) and later milestones. Values are implementation-ready: times are given in seconds **and** in 60 Hz simulation ticks (1 s = 60 ticks), ranges in tiles, speeds in tiles/sec.
**Clean-room note:** This study was compiled exclusively from public web documentation — the Age of Empires II wiki (ageofempires.fandom.com), aoe2techtree.net references, and reputable community references (AoK Heaven / Jineapple data sheets). No game binary, game file, or extracted game data was downloaded, inspected, or decompiled. Numbers and game facts are recorded as facts; all explanations are paraphrased, not copied. Every section cites its sources. Values that could not be verified from public pages are marked **UNVERIFIED**; values where public sources disagree are marked **AMBIGUOUS** with both readings shown.

**Version caveat:** The reference game receives balance patches. Where a wiki value and an older community sheet disagree, the current Definitive Edition (DE) wiki value is treated as primary and the older value is flagged.

---

## 1. Combat Math

### 1.1 Damage formula

For each attack the attacker makes, damage is computed **per armor class** and summed:

```
Damage = max(1, k_elev × Σ_i max(0, Attack_i − Armor_i))
```

- `Attack_i` is the attacker's attack value against class *i* (its base attack belongs to the Melee or Pierce class; additional entries are its bonus damage).
- `Armor_i` is the target's armor against class *i*.
- Each class contributes at least 0 (a class result can only be negative if the attack value itself is negative).
- The **global minimum damage is 1**, applied after all multipliers.
- Damage is tracked fractionally inside the engine; the HP display floors to an integer, and a unit dies when its HP drops **below 1**, not at 0.

Sources: https://ageofempires.fandom.com/wiki/Attack · https://ageofempires.fandom.com/wiki/Armor_class_(Age_of_Empires_II)

### 1.2 Bonus damage and armor classes — how they work

- Bonus damage is **flat and additive**, computed class-by-class exactly like base damage. It is **not** reduced by the target's melee/pierce armor — only by the target's armor in that specific class.
- If a target belongs to several classes and the attacker has bonuses against several of them, **all of them stack**. Example: a Halberdier attacking a War Elephant applies both its vs-Cavalry and its vs-Elephant bonus, because the elephant is a member of both classes.
- A target that is a member of a class usually has armor **0** in that class (full bonus taken). Positive class armor is partial resistance (e.g. Cataphract has +12 Cavalry armor). Negative class armor is extra vulnerability.
- A target that is **not** a member of a class effectively has very high armor in it, so bonuses against that class contribute 0. **AMBIGUOUS:** worked examples on the wiki use **10,000** as this "non-member" armor, while a formula note on the Attack page says "usually 1000". For implementation the distinction is irrelevant (any realistic bonus minus either value floors at 0), but the sources disagree.

Sources: https://ageofempires.fandom.com/wiki/Armor_class_(Age_of_Empires_II) · https://ageofempires.fandom.com/wiki/Attack · https://ageofempires.fandom.com/wiki/Armor

### 1.3 Full armor class list

Class IDs are the engine's class numbers as documented publicly. Groups follow the wiki's own grouping.

**Base damage classes**

| ID | Class | Notes |
|---|---|---|
| 3 | Pierce | Base damage of ranged attacks; exempt from bonus-damage resistance |
| 4 | Melee | Base damage of melee attacks; exempt from bonus-damage resistance |
| 31 | Unblockable melee vs units | Special base type (former Leitis mechanic); unused by standard units in the current game |
| 33 | (unnamed special pierce) | No unit has this armor class; counted as a base type |

**Unit classes**

| ID | Class | Notes |
|---|---|---|
| 1 | Infantry | Excludes foot archers |
| 5 | Elephant Units | All elephant units |
| 8 | Cavalry | Non-camel mounted units, incl. mounted archers. **Camels are NOT cavalry** |
| 15 | Archers | Incl. Skirmishers, Hand Cannoneers, Janissaries, and similar mounted units |
| 16 | Ships | |
| 17 | Rams | Incl. Armored Elephants; packed Trebuchets do not have it |
| 19 | Unique Units | |
| 20 | Siege Weapons | |
| 23 | Gunpowder Units | Incl. Bombard Towers |
| 25 | Monks | Incl. Missionaries |
| 27 | Spearmen | |
| 28 | Cavalry Archers | Incl. Conquistadors, Arambai, Genitours |
| 29 | Shock Infantry | The Eagle Warrior class |
| 30 | Camel Units | Own class, separate from Cavalry |
| 32 | Condottieri | |
| 34 | Fishing Ships | Currently unused as a separate class |
| 35 | Mamelukes | |
| 36 | Heroes & Kings | |
| 37 | Heavy Siege | Hussite Wagons, Ballista Elephants |
| 38 | Skirmishers | Skirmishers, Genitours |
| 2 | Capital Ships | (Obsolete group) Turtle Ship, Thirisadai |
| 41 | Fire Ship | |
| 60 | Long-Range Warships | Galley line and siege ships |

**Building classes**

| ID | Class | Notes |
|---|---|---|
| 11 | All Buildings | Everything built |
| 21 | Standard Buildings | Armor in this class is **always 0 by definition**; most infantry anti-building bonuses are against this class |
| 13 | Stone Defense | Towers, Castles, Harbors |
| 22 | Walls and Gates | Stone and palisade |
| 26 | Castles | Incl. Kreposts |
| 40 | House | Currently unused |
| 0 | Wonders | Currently unused in standard play |

**Gaia classes**

| ID | Class | Notes |
|---|---|---|
| 14 | Predator Animals | Wolves, lions, etc. |
| 18 | Trees | |
| 24 | Boars / hunted predators | Boars, javelina, elephants, rhinos, tapirs |

Class IDs not listed (6, 7, 9, 10, 12, 42–59, …) do not appear in the public list — **UNVERIFIED** whether they are reserved; they are simply absent from the documentation.

Source: https://ageofempires.fandom.com/wiki/Armor_class_(Age_of_Empires_II)

### 1.4 Elevation (hill) bonus

- Attacker on higher ground than the target: damage **× 1.25**. Attacker on lower ground: damage **× 0.75**. Same level: × 1.
- The modifier is binary — it does **not** scale with the height difference; any height advantage (≥ 1 elevation level) gives the full modifier.
- It is applied **after** armor subtraction (unlike attack-percentage bonuses from technologies, which are applied to the attack value *before* subtraction — so +25% attack is stronger than the hill bonus's +25% damage).
- **AMBIGUOUS (cliff nuance):** one wiki page states the uphill penalty (× 0.75) applies to hills only, and attacking up a cliff does not incur it, while the downhill × 1.25 applies from both hills and cliffs. Skirmish II has no cliffs in the prototype; treat × 0.75 as applying to any lower-ground attacker unless cliffs are introduced.
- Civilization exceptions exist (e.g. Tatars get × 1.5 from high ground) — out of scope for the generic prototype.

Sources: https://ageofempires.fandom.com/wiki/Attack · https://ageofempires.fandom.com/wiki/Elevation · https://ageofempires.fandom.com/wiki/Pierce_armor

### 1.5 Special cases

- **Rams:** members of the Ram class carry extremely high pierce armor (180–195, see §3.10), so arrows deal the minimum 1 damage (further reduced in practice because their pierce armor exceeds any pierce attack). Their melee armor is **negative** (−3), so melee attackers deal *more* than their listed attack (attack − (−3) = attack + 3). This is why villagers and infantry are the practical ram counter.
- **Mishit penalty:** a projectile that misses its intended target but hits a different unit deals **50% damage** to that unit. A few units are exempt (their missed shots deal full damage by design).
- **Armor-ignoring attacks** exist for a few unique units (target's melee or pierce armor treated as 0) — out of scope for the generic prototype, noted for completeness.
- **Building armor technologies** (Masonry/Architecture) add armor against class 11 (All Buildings) only — they do **not** reduce infantry bonuses against class 21 (Standard Buildings). This asymmetry is deliberate in the reference design.
- **Damage cap:** no general per-hit damage cap is documented in any public source found — the only bounds are the per-class floor of 0 and the global minimum of 1. **UNVERIFIED (apparently none).**

Sources: https://ageofempires.fandom.com/wiki/Armor_class:_Ram · https://ageofempires.fandom.com/wiki/Armor_class_(Age_of_Empires_II) · https://ageofempires.fandom.com/wiki/Masonry

---

## 2. Attack Timing & Projectiles

### 2.1 Rate of fire (reload time)

In this game's terminology, "rate of fire" is the **reload time in seconds between attacks** — a lower number means a faster attacker. The most common value is **2.0 s (120 ticks)**. Percentage attack-speed bonuses divide the reload time (e.g. +25% attack speed → reload × 0.8).

Reference points: Spearman line and Skirmisher line 3.0 s; Knight line 1.8 s; Ram line 5.0 s; Mangonel line 6.0 s; Trebuchet 10.0 s.

Source: https://ageofempires.fandom.com/wiki/Rate_of_Fire

### 2.2 Frame delay / attack delay

- **Attack delay (AD)** is the wind-up between starting an attack and the damage landing (melee) or the projectile leaving (ranged). It derives from the animation **frame delay**: `AD = animation duration × frame delay / frames per angle`, and for melee `AD = animation duration / 2`. The result is rounded to whole frames at a 60 FPS simulation — conveniently, 1 frame = 1 Skirmish II tick.
- Attack delay applies to the **first** attack only; in a sustained attack, hits are spaced by the reload time.
- Ordering a unit to move or attack elsewhere **during** its attack delay **aborts** the attack.
- Most melee infantry/cavalry have frame delay 0 in the data tables, but their damage still lands after half the attack animation — the attack-delay table below (in seconds) is the implementation-usable form. It comes from an older wiki table the page itself flags as potentially outdated; treat individual values as approximate.

| Unit | Attack delay | In ticks (60 Hz) |
|---|---|---|
| Mangonel line, defensive buildings, warships | 0 s | 0 |
| Archer line, Hand Cannoneer | 0.35 s | 21 |
| Skirmisher line | ≈0.51 s (frame delay 19) | ≈31 |
| Scout/Knight lines (melee) | ≈0.68 s | ≈41 |
| Militia, Spearman line (melee) | ≈0.50 s | ≈30 |
| Villager (melee) | ≈0.64 s | ≈38 |
| Ram line (melee) | ≈0.75 s | ≈45 |
| Trebuchet | 0.88 s | ≈53 |

Sources: https://ageofempires.fandom.com/wiki/Attack_delay

### 2.3 Projectiles

- Projectile speed is measured in **tiles/sec**. **No universal per-weapon-class speed table exists in the public documentation — UNVERIFIED as a general convention.** Verified individual values: Mangonel projectile **3.5 tiles/s** (original data: 2.9); Janissary bullet 7.5 tiles/s; Elite Cannon Galleon shell 3.5 tiles/s.
- Projectiles fly a **fixed trajectory set at release** — no homing. Without the Ballistics technology there is no target-leading, so moving targets are missed geometrically; Ballistics adds aim prediction at the moment of release only.
- Archers fire in an arc; the arc height values found (0.6 / 0.4 for the Mangonel's projectiles) are visual/trajectory parameters, not damage parameters.

Sources: https://ageofempires.fandom.com/wiki/Mangonel_(Age_of_Empires_II) · https://ageofempires.fandom.com/wiki/Accuracy · https://ageofempires.fandom.com/wiki/Rocket_Cart

### 2.4 Accuracy and scatter

- **Accuracy %** is the chance a shot hits its intended point against a **stationary** target; the game rolls per shot. It is separate from hitting moving targets (a geometry question, §2.3).
- Typical values: Archer 80%, Crossbowman 85%, Arbalester 90%, Skirmisher 90%, Cavalry Archer 50%, Hand Cannoneer 75%. Siege units, buildings, and ships are typically 100%. The Thumb Ring technology raises affected archers to 100% (against stationary targets) and speeds up their fire.
- When a shot is computed as a miss, where it lands is governed by a separate **dispersion/scatter** attribute — low dispersion keeps misses near the target (this is how 0%-accuracy units like the Organ Gun still function: their projectile hits the first enemy in its path). **Numeric per-unit scatter radii are not published on the pages found — UNVERIFIED.** Implement as a per-unit scatter-radius parameter and tune by feel.
- A missed projectile that hits a different unit deals 50% damage (mishit penalty, §1.5).

Source: https://ageofempires.fandom.com/wiki/Accuracy

### 2.5 Minimum range, blast radius, friendly fire

- **Minimum range** is a dead zone inside which a ranged unit/building cannot attack. Examples: Mangonel 3 (range 3–7), Skirmisher 1, Scorpion 2, Castle/Watch Tower 1. Archers and Hand Cannoneers have minimum range 0. The Murder Holes technology removes minimum range for buildings.
- **Blast radius** (tiles): Mangonel **1.0**, Onager **1.25**, Siege Onager 1.5. Capped Ram 1.5 and Siege Ram 2.0 (against buildings/siege).
- **Blast damage fraction:** non-primary targets inside the radius take a fraction of the computed (attack − armor) damage: 100% for Siege Rams vs buildings, 50% for War Elephants, 33% for the Armored Elephant line, 25% for Battle Elephants. The Mangonel line uses a **tapering** falloff with distance from impact. Accuracy is rolled per unit inside the blast for blast units (most are 100%, so it rarely matters).
- **Friendly fire:** blast projectiles damage the attacker's **own and allied** units in the radius. Documented friendly-fire units: Mangonel line, Bombard Cannons, Cannon Galleons, Warwolf Trebuchets. **Scorpions do NOT friendly-fire** — the Mangonel/Scorpion contrast is explicit in the sources. Most archer-type units are flagged so their attacks cannot friendly-fire at all.

Sources: https://ageofempires.fandom.com/wiki/Area_of_Effect · https://ageofempires.fandom.com/wiki/Mangonel_(Age_of_Empires_II) · https://ageofempires.fandom.com/wiki/Onager_(Age_of_Empires_II) · https://ageofempires.fandom.com/wiki/Technology_(Age_of_Empires_II)

---

## 3. Unit Stat Tables

Base values for a generic civilization — no civilization bonuses, no Blacksmith/Monastery technologies unless noted. Armor is shown as melee/pierce. "RoF" is reload time; ticks = seconds × 60. "FD" is frame delay as listed in the data tables (frames); where a source gives attack delay (AD) directly in seconds, that is shown too (see §2.2 for the relationship). Costs: F = food, W = wood, G = gold, S = stone.

Sources for this section (per line, detailed below): https://ageofempires.fandom.com/wiki/Infantry_unit_(Age_of_Empires_II) · https://ageofempires.fandom.com/wiki/Foot_archer_unit_(Age_of_Empires_II) · https://ageofempires.fandom.com/wiki/Cavalry_unit_(Age_of_Empires_II) · https://ageofempires.fandom.com/wiki/Villager_(Age_of_Empires_II) · https://aok.heavengames.com/university/game-info/stat-tables/units-table/

### 3.1 Villager

| HP | Attack | Armor | Range | RoF | Speed | LoS | Cost | Train |
|---|---|---|---|---|---|---|---|---|
| 25 | 3 melee | 0/0 | melee | 2.0 s (120 t) | 0.8 | 4 | 50 F | 25 s (1500 t) |

- Frame delay / attack delay: **UNVERIFIED** in the tables checked (a community attack-delay table lists ≈0.64 s AD ≈ 38 ticks).
- Small base bonus vs buildings/stone defense exists as a class entry; its exact base number is **UNVERIFIED** in the sources checked (the +15 figure belongs to the Sappers technology, not the base unit).
- Loom (technology): +15 HP, +1 melee / +2 pierce armor.

### 3.2 Militia line (infantry)

| Unit | HP | Atk (melee) | Armor | RoF | FD | Speed | LoS | Cost | Train |
|---|---|---|---|---|---|---|---|---|---|
| Militia | 40 | 4 | 0/1 | 2.0 s | 0 | 0.90 | 4 | 50 F 20 G | 21 s (1260 t) |
| Man-at-Arms | 45 | 6 | 0/1 | 2.0 s | 0 | 0.96 | 4 | 50 F 20 G | 21 s |
| Long Swordsman | 60 | 9 | 1/1 | 2.0 s | 0 | 0.96 | 4 | 50 F 20 G | 21 s |
| Champion | 70 | 14 | 1/1 | 2.0 s | 0 | 0.96 | 5 | 50 F 20 G | 21 s |

- Bonus damage: Man-at-Arms +2 vs Shock Infantry (Eagles), +2 vs Standard Buildings; Long Swordsman and Champion scale up from there. **AMBIGUOUS:** classic community tables give cumulative totals of +4/+6 vs Eagles and +2/+3 vs Standard Buildings; DE upgrade notes give different cumulative arithmetic (Champion unchanged on its final upgrade, implying +6/+3). The Man-at-Arms values (+2/+2) are consistent across sources.
- Patch notes (DE Update 141935): line cost reduced 60 F → 50 F; Man-at-Arms and above speed 0.9 → 0.96; Champion attack 13 → 14. The table above uses the current values.
- (Intermediate tier, for completeness: Two-Handed Swordsman — HP 65, Atk 12, Armor 1/1, LoS 5, same cost/RoF/speed/train.)

### 3.3 Spearman line (anti-cavalry infantry)

| Unit | HP | Atk (melee) | Armor | RoF | Speed | LoS | Cost | Train |
|---|---|---|---|---|---|---|---|---|
| Spearman | 45 | 3 | 0/0 | 3.0 s (180 t) | 1.0 | 4 | 35 F 25 W | 22 s (1320 t) |
| Pikeman | 55 | 4 | 0/0 | 3.0 s | 1.0 | 4 | 35 F 25 W | 22 s |
| Halberdier | 60 | 6 | 0/0 | 3.0 s | 1.0 | 4 | 35 F 25 W | 22 s |

Bonus damage per armor class (bonuses stack on multi-class targets):

| Unit | vs Cavalry | vs Elephant | vs Camel | vs Ship | vs Shock Inf. | vs Std. Building |
|---|---|---|---|---|---|---|
| Spearman | +15 | +15 | +9 | +9 | +1 | +1 |
| Pikeman | +22 | +25 | +16 | +16 | +1 | +1 |
| Halberdier | +32 | +28 | +17* | +17 | +1 | +1* |

- Worked example of stacking: Pikeman vs War Elephant = +22 (Cavalry) + +25 (Elephant) = **+47** bonus.
- **AMBIGUOUS:** the current DE Halberdier page lists +26 vs Camel units and +3 vs Standard Buildings as totals, conflicting with the classic per-class table (+17 / +1) above. Flagged for a decision before M3 balance work; the vs-Cavalry headline numbers (+15/+22/+32) agree across sources.

Sources: https://ageofempires.fandom.com/wiki/Halberdier_(Age_of_Empires_II) · https://aok.heavengames.com/university/game-info/game-statistics/units/infantry/spearman/

### 3.4 Archer line

| Unit | HP | Atk (pierce) | Armor | Range | RoF | FD | Speed | LoS | Cost | Train | Accuracy |
|---|---|---|---|---|---|---|---|---|---|---|---|
| Archer | 30 | 4 | 0/0 | 4 | 2.0 s | 15 | 0.96 | 6 | 25 W 45 G | 35 s (2100 t) | 80% |
| Crossbowman | 35 | 5 | 0/0 | 5 | 2.0 s | 15 | 0.96 | 7 | 25 W 45 G | 27 s (1620 t) | 85% |
| Arbalester | 40 | 6 | 0/0 | 5 | 2.0 s | 20 | 0.96 | 7 | 25 W 45 G | 27 s | 90% |

- Minimum range 0 for the whole line. Attack delay ≈ 0.35 s (≈21 ticks) per community tables.
- Bonus damage: **+3 vs Spearmen** for all three.

### 3.5 Skirmisher line (anti-archer)

| Unit | HP | Atk (pierce) | Armor | Range (min–max) | RoF | FD | Speed | LoS | Cost | Train | Accuracy |
|---|---|---|---|---|---|---|---|---|---|---|---|
| Skirmisher | 30 | 2 | 0/3 | 1–4 | 3.0 s | 19 | 0.96 | 6 | 25 F 35 W | 22 s* | 90% |
| Elite Skirmisher | 35 | 3 | 0/4 | 1–5 | 3.0 s | 19 | 0.96 | 7 | 25 F 35 W | 22 s | 90% |

- Bonus damage: Skirmisher +3 vs Archers, +3 vs Spearmen; Elite Skirmisher +4 vs Archers, +4 vs Spearmen**, +2 vs Cavalry Archers.
- *AMBIGUOUS:** one wiki list extraction shows 26 s training for the base Skirmisher; the archer-unit list and classic sheets show 22 s. 22 s is the multi-source value.
- **AMBIGUOUS:** Elite Skirmisher vs Spearmen appears as +4 (its changelog: raised 3 → 4 in Update 95810) and as +3 in one comparison table.

### 3.6 Scout Cavalry line

| Unit | HP | Atk (melee) | Armor | RoF | Speed | LoS | Cost | Train |
|---|---|---|---|---|---|---|---|---|
| Scout Cavalry | 45 | 3 (Dark Age) / 5 (Feudal+) | 0/2 | 2.0 s | 1.20 (Dark) / 1.55 (Feudal+) | 4 (Dark) / 6 (Feudal+) | 80 F | 30 s (1800 t) |
| Light Cavalry | 60 | 7 | 0/2 | 2.0 s | 1.50 | 8 | 80 F | 30 s |
| Hussar | 75 | 7 | 0/2 | 1.9 s (114 t) | 1.50 | 10 | 80 F | 30 s |

- Bonus damage vs Monks: +6 / +10 / +12. The line is conversion-resistant (Scout class). Frame delay: **UNVERIFIED** in tables (melee, presumed 0; community AD ≈ 0.68 s).
- The Scout Cavalry's Feudal Age stat jumps (attack, speed, LoS) come from the automatic age-up bonus, not a new unit.

### 3.7 Knight line

| Unit | HP | Atk (melee) | Armor | RoF | Speed | LoS | Cost | Train |
|---|---|---|---|---|---|---|---|---|
| Knight | 100 | 10 | 2/2 | 1.8 s (108 t) | 1.35 | 4 | 60 F 75 G | 30 s |
| Cavalier | 120 | 12 | 2/2 | 1.8 s | 1.35 | 4 | 60 F 75 G | 30 s |
| Paladin | 160 | 14 | 2/3 | 1.9 s | 1.35 | 5 | 60 F 75 G | 30 s |

- No bonus damage. Frame delay **UNVERIFIED** (community AD ≈ 0.68 s ≈ 41 ticks).

### 3.8 Mangonel line (siege)

| Unit | HP | Atk (melee, blast) | Armor | Range (min–max) | RoF | Speed | LoS | Cost | Train | Accuracy |
|---|---|---|---|---|---|---|---|---|---|---|
| Mangonel | 50 | 40, blast 1.0 | 0/6 | 3–7 | 6.0 s (360 t) | 0.6 | 9 | 160 W 135 G | 46 s (2760 t) | 100% |
| Onager | 60 | 50, blast 1.25 | 0/7 | 3–8 | 6.0 s | 0.6 | 10 | 160 W 135 G | 46 s | 100% |

- The Mangonel line's attack is class **Melee** despite being a ranged projectile (see §1: it is reduced by melee armor, not pierce armor).
- Bonus damage: +35 vs Buildings (Mangonel), +45 vs Buildings (Onager), +12 vs Siege Weapons. Attack delay 0.
- Onager and above can cut trees. Friendly fire: yes (§2.5).

### 3.9 Battering Ram line (brief)

| Unit | HP | Atk (melee) | Armor | RoF | Speed | LoS | Cost | Train |
|---|---|---|---|---|---|---|---|---|
| Battering Ram | 175 | 2 | −3/180 | 5.0 s (300 t) | 0.6 | 3 | 160 W 75 G | 36 s (2160 t) |
| Capped Ram | 200 | 3 (blast 1.5) | −3/190* | 5.0 s | 0.6 | 3 | 160 W 75 G | 36 s |
| Siege Ram | 270 | 4 (blast 2.0) | −3/195 | 5.0 s | 0.6 | 3 | 160 W 75 G | 36 s |

- Bonus damage vs Buildings: 150 / 160 / **UNVERIFIED** (classic sheet: 125 / 150 / 200; DE updates raised Battering Ram to 150 and Capped Ram to 160; the current Siege Ram total was not confirmed). Vs Siege Weapons (classic): 40 / 50 / 65.
- *A newer wiki comparison gives Capped Ram melee armor −2 (Update 153015); Siege Ram melee armor in current DE **UNVERIFIED**.
- Garrison: up to 6 foot units ride inside; each garrisoned infantry adds +0.05 tiles/s speed (0.6 → max 0.9) and +10 vs Buildings (classic figures).

### 3.10 Monk (brief)

| HP | Attack | Armor | Range | Speed | LoS | Cost | Train |
|---|---|---|---|---|---|---|---|
| 30 | none — conversion | 0/0 | conversion 9, heal 4 | 0.7 | 11 | 100 G | 51 s (3060 t) |

- Healing: 150 HP/min (2.5 HP/s) from one Monk on one target.
- Conversion: faith recharge 62 s after a conversion. Conversion time itself is variable and resistance-dependent — **not a fixed stat** in the sources checked; do not model the classic table's "1.0 s" as a rate of fire.
- Can pick up Relics (LoS −2 while carrying one).

---

## 4. Movement & Sizes

### 4.1 Speeds (tiles/sec, base, no technologies)

| Unit line | Speed | Notes |
|---|---|---|
| Villager | 0.8 | 0.96 fully upgraded (Wheelbarrow + Hand Cart) |
| Militia | 0.9 | |
| Man-at-Arms → Champion | 0.96 | |
| Spearman line | 1.0 | |
| Archer line, Skirmisher line | 0.96 | |
| Scout Cavalry | 1.2 → 1.55 | Dark Age → Feudal Age (auto bonus) |
| Light Cavalry / Hussar | 1.5 | |
| Knight line | 1.35 | |
| Mangonel line | 0.6 | |
| Ram line | 0.6 | Was 0.5 before DE Update 87863; +0.05 per garrisoned infantry, max 0.9 |
| Monk | 0.7 | |

Speed modifiers: Wheelbarrow +10% villager speed, Hand Cart a further +10%; Husbandry +10% mounted speed; Squires +10% infantry speed (reflected in the "fully upgraded" figures on the source page).

Source: https://ageofempires.fandom.com/wiki/Speed · https://ageofempires.fandom.com/wiki/Cavalry_unit_(Age_of_Empires_II)

### 4.2 Tiles and collision size

- All distances (range, LoS, speed) are measured **in tiles**; the engine's documented speed range is 0.5–7.5 tiles/s. There is no separate "game unit" conversion. A real-world size for 1 tile is **UNVERIFIED** — no public page gives one; a tile is simply the base grid unit (a Watch Tower occupies 1 tile, a Town Center 4×4).
- **Per-unit collision radius: UNVERIFIED as a public table.** The wiki stat pages do not publish radius/collision fields. The only general convention found: most comparable infantry units have radius ≈ **0.2 tiles**, with the Karambit Warrior explicitly documented at 0.15 as an exception that "packs tighter". A historical changelog mentions a Siege Tower collision reduction 0.50 → 0.45 (HD era; current-DE status unverified).
- **Skirmish II decision implied:** adopt 0.2 tiles as the default unit radius (documented as the typical value), per-unit overrides where a source exists, and tune by feel. This matches M1/M2's existing steering model in spirit — separation is displacement-based, not radius-formalized.

Sources: https://ageofempires.fandom.com/wiki/Speed · https://ageofempires.fandom.com/wiki/Karambit_Warrior

---

## 5. Buildings

Armor slash values (e.g. 3/4/5/6) progress by Age — first value is the Age the building becomes available, last is Imperial Age. Building armor and HP rise with Age even without technologies. "Garrison" is defensive garrison capacity.

| Building | HP | Armor (melee/pierce) | Cost | Build time | Size | Garrison |
|---|---|---|---|---|---|---|
| Town Center | 2,400 | 3/4/5/6 · 5/6/7/8 | 275 W 100 S | 150 s (9000 t) | 4×4 | 15 |
| House | 550/750/900/900 | −2/−1/1/3 · 7/8/9/10 | 25 W | 25 s (1500 t) | 2×2 | 0 |
| Farm | 480 | 0/0 | 60 W | 15 s (900 t) | 3×3 | 0 |
| Barracks | 1,200/1,500/1,800/2,100 | 0/1/2/3 · 7/8/9/10 | 175 W | 50 s (3000 t) | 3×3 | 10 |
| Archery Range | 1,500/1,800/2,100 | 1/2/3 · 8/9/10 | 175 W | 50 s | 3×3 | 10 |
| Stable | 1,500/1,800/2,100 | 1/2/3 · 8/9/10 | 175 W | 50 s | 3×3 | 10 |
| Siege Workshop | 1,500/1,800/2,100 | 1/2/3 · 8/9/10 | 200 W | 40 s (2400 t) | 4×4 | 10 |
| Watch Tower | 850/1,020/1,020 | 1 · 7 | 35 W 125 S | 80 s (4800 t) | 1×1 | 5 |
| Castle | 4,800 | 8 · 11 | 650 S | 200 s (12000 t) | 4×4 | 20 |

Function notes:

- **Town Center:** trains Villagers; drop-off point for all four resources; Age advancement and villager/building technologies. Population support **+5**. Only one may be built before Castle Age. Fires arrows **only when garrisoned** (range 6, 5 pierce, RoF 2.0 s; arrow count scales with garrison, §9.3); a TC with nobody inside fires nothing. Note: aoe2techtree.net shows a 100 s build time — its maintainers explain that figure is for the non-buildable Dark Age "stack" variant; the player-built TC is 150 s.
- **House:** population support **+5**.
- **Farm:** contains **175 food** base (see §6.3 — not 250). Requires a Mill first. One Villager works one Farm.
- **Barracks:** infantry production; prerequisite for Archery Range and Stable.
- **Archery Range:** requires a Barracks. **Stable:** requires a Barracks. **Siege Workshop:** Castle Age; requires a Blacksmith.
- **Watch Tower:** attack 5 pierce (+7 vs Ships, +2 vs Spearmen), range 1–8, RoF 2.0 s. Cost note: the 35 wood figure is current (Update 111772; it was 50 wood in earlier patches).
- **Castle:** unique-unit and Trebuchet production; population support **+20**. Attack 11 pierce, range 1–8, RoF 2.0 s, fires multiple arrows (more with garrison, §9.3).

Sources: https://ageofempires.fandom.com/wiki/Building_(Age_of_Empires_II) · https://ageofempires.fandom.com/wiki/Town_Center_(Age_of_Empires_II) · https://ageofempires.fandom.com/wiki/House_(Age_of_Empires_II) · https://ageofempires.fandom.com/wiki/Watch_Tower_(Age_of_Empires_II) · https://ageofempires.fandom.com/wiki/Castle_(Age_of_Empires_II) · https://ageofempires.fandom.com/wiki/Population · https://github.com/siegeengineers/aoe2techtree/issues/133

---

## 6. Economy Reference

### 6.1 Gather rates (per second, base — no technologies or bonuses)

| Source | Gatherer | Rate/s | Per-tick (÷60) |
|---|---|---|---|
| Tree (wood) | Villager | **0.388** | 0.00647 |
| Berry / forage bush | Villager | **0.310** | 0.00517 |
| Farm | Villager | **0.319** | 0.00532 |
| Hunt (deer/boar) | Villager | **0.408** | 0.00680 |
| Herdable (sheep/livestock) | Villager | **0.330** | 0.00550 |
| Shore fish | Villager | **0.426** | 0.00710 |
| Shore fish | Fishing Ship | 0.280 | 0.00467 |
| Deep sea fish | Fishing Ship | **0.487** | 0.00812 |
| Gold mine | Villager | **0.379** | 0.00632 |
| Stone mine | Villager | **0.359** | 0.00598 |

Sources: https://ageofempires.fandom.com/wiki/Resource · https://ageofempires.fandom.com/wiki/Villager_(Age_of_Empires_II)

### 6.2 Carry capacity and drop-off

- Base Villager carry capacity: **10 for every resource** — with one exception: **huntables (hunted animal meat) = 35**.
- Wheelbarrow: +25% carry (10 → 13); Hand Cart: a further +50% (→ 20).
- Gatherers work a node until their carry is full (or the node depletes), then walk to the nearest drop-off building and deposit instantly, then return. (This is exactly the loop Skirmish II M2 already implements.)

Sources: https://ageofempires.fandom.com/wiki/Villager_(Age_of_Empires_II) · https://ageofempires.fandom.com/wiki/Hand_Cart

### 6.3 Node amounts and farm mechanics

- Tree: **100 wood** · Berry bush: **125 food** · Gold mine: **800 gold** per tile · Stone mine: **350 stone** per tile.
- Animals: Sheep 100 food, Cow 150, Deer 140, Boar 340.
- **Farm food: 175 base** → 250 with Horse Collar (+75) → 375 with Heavy Plow (+125) → 550 with Crop Rotation (+175).
- A Farm itself generates food at an inherent **0.4/s work-rate cap**, which limits effective farming speed after Wheelbarrow/Hand Cart speed up the Villager.
- Farms already built do **not** retroactively gain food from a farm upgrade researched later (a Farm still under construction when the research completes does gain it).
- Reseeding: Farms can be queued at the Mill (60 wood each, paid in advance); DE also offers an auto-reseed toggle at the Mill that reseeds on depletion without pre-payment.

Sources: https://ageofempires.fandom.com/wiki/Resource · https://ageofempires.fandom.com/wiki/Berry_Bush · https://ageofempires.fandom.com/wiki/Stone · https://ageofempires.fandom.com/wiki/Farm_(Age_of_Empires_II) · https://ageofempires.fandom.com/wiki/Mill_(Age_of_Empires_II)

### 6.4 Cross-check against Skirmish II M2 (already implemented)

| Skirmish II M2 value | Public-source value | Verdict |
|---|---|---|
| Wood ≈ 0.390/s | 0.388/s | ✅ Confirmed (rounding) |
| Berry ≈ 0.310/s | 0.310/s | ✅ Confirmed |
| Gold ≈ 0.380/s | 0.379/s | ✅ Confirmed (rounding) |
| Stone ≈ 0.360/s | 0.359/s | ✅ Confirmed (rounding) |
| Carry capacity 10 | 10 (hunt exception: 35) | ✅ Confirmed — add the hunt exception when hunting is implemented |
| Tree node 100 | 100 wood | ✅ Confirmed |
| Berry node 125 | 125 food | ✅ Confirmed |
| Gold node 800 | 800 gold | ✅ Confirmed |
| Stone node 350 | 350 stone | ✅ Confirmed |
| Farm food (not yet in M2) | **175 base** (250 only after Horse Collar) | ⚠️ Use 175 when farms are implemented — 250 is the post-Horse-Collar value |

**No conflicts with M2.** The only correction affects future work: farm base food is 175.

---

## 7. Ages

All Age advances are researched at the Town Center.

| Advance | Cost | Research time | Ticks | Building requirement |
|---|---|---|---|---|
| Feudal Age | 500 F | 130 s | 7800 | Any 2 Dark Age buildings (excluding Houses, Farms, walls, Outposts) |
| Castle Age | 800 F 200 G | 160 s | 9600 | Any 2 Feudal Age buildings (excluding walls/towers) |
| Imperial Age | 1,000 F 800 G | 190 s | 11400 | A Castle, **or** any 2 other Castle Age buildings |

Sources: https://ageofempires.fandom.com/wiki/Technology_(Age_of_Empires_II) · https://ageofempires.fandom.com/wiki/Feudal_Age_(Age_of_Empires_II) · https://ageofempires.fandom.com/wiki/Castle_Age_(Age_of_Empires_II) · https://ageofempires.fandom.com/wiki/Imperial_Age_(Age_of_Empires_II)

---

## 8. Key Early Technologies (Overview)

| Technology | Researched at / Age | Cost | Time | Ticks | Effect |
|---|---|---|---|---|---|
| Loom | Town Center / Dark | 50 G | 25 s | 1500 | Villagers +15 HP, +1/+2 armor |
| Wheelbarrow | Town Center / Feudal | 175 F 50 W | 75 s | 4500 | Villagers +10% speed, +25% carry (10→13) |
| Double-Bit Axe | Lumber Camp / Feudal | 100 F 50 W | 25 s | 1500 | Wood gathering +20% |
| Horse Collar | Mill / Feudal | 75 F 75 W | 20 s | 1200 | Farms +75 food (175→250) |
| Fletching | Blacksmith / Feudal | 100 F 50 G | 30 s | 1800 | Archers, Castles, Towers +1 attack & +1 range; TC +1 attack & +1 LoS |
| Forging | Blacksmith / Feudal | 150 F | 50 s | 3000 | Infantry & cavalry +1 melee attack |
| Scale Mail Armor | Blacksmith / Feudal | 100 F | 40 s | 2400 | Infantry +1/+1 armor |
| Bloodlines | Stable / Feudal | 150 F 100 G | 50 s | 3000 | Mounted units +20 HP |
| Husbandry | Stable / **Castle** | 150 F | 40 s | 2400 | Mounted units +10% speed |

Technology lines (same pattern — each tier adds the same kind of bonus):

- **Melee attack:** Forging (+1) → Iron Casting (Castle, 220 F 120 G, 75 s, +1) → Blast Furnace (Imperial, 275 F 225 G, 100 s, +2).
- **Ranged attack/range:** Fletching (+1/+1) → Bodkin Arrow (Castle, 200 F 100 G, 35 s) → Bracer (Imperial, 300 F 200 G, 40 s).
- **Infantry armor:** Scale Mail (+1/+1) → Chain Mail (Castle, 200 F 100 G, 55 s) → Plate Mail (Imperial, 300 F 150 G, 70 s, +1/+2).
- **Cavalry armor:** Scale Barding (Feudal, 150 F, 45 s, +1/+1) → Chain Barding (Castle, 250 F 150 G, 60 s) → Plate Barding (Imperial, 350 F 200 G, 75 s, +1/+2).
- **Archer armor:** Padded Archer Armor (Feudal, 100 F, 40 s, +1/+1) → Leather Archer Armor (Castle, 150 F 150 G, 55 s) → Ring Archer Armor (Imperial, 250 F 250 G, 70 s, +1/+2).

Source: https://ageofempires.fandom.com/wiki/Technology_(Age_of_Empires_II) · https://ageofempires.fandom.com/wiki/Scale_Mail_Armor · https://ageofempires.fandom.com/wiki/Hand_Cart

---

## 9. Feel Mechanics

### 9.1 Unit stances

Four stances exist: **Aggressive, Defensive, Stand Ground, No Attack**.

- New military units default to **Aggressive** (DE adds a setting to default to Defensive instead). The starting Scout Cavalry is an exception — it starts on **Stand Ground**. Some siege lines ignore stance settings.
- **AMBIGUOUS / partially UNVERIFIED for AoE2 specifically:** the public wiki's stance page lists the AoE2 stance names, defaults, and target priorities (§9.2), but its prose definitions of each stance's behavior (how far Defensive units chase before returning, Stand Ground firing only in range, No Attack units fleeing) are written for **Age of Mythology**, not AoE2. **No numeric leash/pursuit/return distance for any stance was found in any public source — UNVERIFIED.**
- Skirmish II implementation guidance: the conventional, widely understood behavior is — Aggressive: pursue targets indefinitely once acquired; Defensive: pursue briefly, then return to the guard point; Stand Ground: attack only targets in range, never move; No Attack: never attack. Adopt this as a design decision (it matches player expectations), but note it is **not** verified AoE2 documentation. Leash distances must be tuned by feel in M5.

Source: https://ageofempires.fandom.com/wiki/Unit_stance · https://ageofempires.fandom.com/wiki/Scout_Cavalry_(Age_of_Empires_II)

### 9.2 Attack-move, patrol, and auto-targeting

- **Patrol** is documented in community guides as a **loop**: the unit moves back and forth between its start point and the designated point (waypoints chainable with Shift), engages enemies that enter its area, and does not chase an enemy indefinitely once it leaves that area.
- **Attack Move** exists in DE (patch notes reference units attack-moving and patrolling with waypoints), but **no public page found formally defines it** — **UNVERIFIED** as documented behavior. The conventional understanding (one-way: engage enemies encountered en route, then stop at the destination) is a design decision to adopt, not a documented fact.
- **Default attack priority (documented for AoE2):**
  1. **Rams** auto-target enemy **buildings only, never units**.
  2. **Trebuchets** auto-target enemy buildings and other Trebuchets, never other units.
  3. All other units/buildings target the **closest enemy unit**.
  4. If no enemy unit is nearby, they target an **enemy Town Center in line of sight**.
  5. Other buildings are lower priority than the above.
  6. **Farms, walls, gates, and herdable animals are never auto-targeted.**
- Any finer priority (military over Villagers, weakest-first, current-attacker-first) is **UNVERIFIED** — not documented in the sources found.

Sources: https://ageofempires.fandom.com/wiki/Unit_stance · https://gamefaqs.gamespot.com/pc/260708-age-of-empires-ii-definitive-edition/faqs/38481 · https://ageofempires.fandom.com/wiki/Update_111772

### 9.3 Garrison mechanics

- **Capacities:** Town Center 15, towers 5, Castle 20 (civilization exceptions exist, e.g. Teutons double tower/TC values). Production buildings hold 10 (rally garrison). Eligible unit types vary by building.
- **Protection:** garrisoned units are immune to damage and conversion. If the building is destroyed, drops to **≤ 20% HP**, or is converted, the garrison is ejected unharmed.
- **Healing inside:** Town Centers/towers heal garrisoned units at **0.1 HP/s** (6 HP/min); Castles at **0.2 HP/s** (12 HP/min). Units garrisoned inside units (rams, transports) are not healed.
- **Extra arrows:** only Villagers and archer-type units add arrows (infantry normally adds none). Each contributing unit adds its ranged DPS; the building divides total DPS by its own DPS, rounded **down**, to get extra arrows. Villagers contribute 2.5 DPS each. Caps: towers +4 extra arrows, Town Center up to 10 arrows total, Castle up to +16 extra arrows (a Castle with 20 garrisoned Villagers fires 14 arrows total).
- **Town Bell:** rings all Villagers within **23 tiles** into the nearest shelter; ringing again sends them back to work.

Sources: https://ageofempires.fandom.com/wiki/Garrison · https://ageofempires.fandom.com/wiki/Town_Center_(Age_of_Empires_II) · https://ageofempires.fandom.com/wiki/Herbal_Medicine_(Age_of_Empires_II)

### 9.4 Population cap rules

- Standard game cap: **200** (the lobby setting can go up to 500 since The Forgotten).
- Population room: House +5, Town Center +5, Castle +20.
- Almost every unit costs exactly 1 population slot.
- Being unable to create a unit for lack of room (while below the absolute cap) is called being **housed**. Production is blocked but units can still be **queued**; training proceeds when room frees up.
- Destroying a population-providing building removes its room.

Source: https://ageofempires.fandom.com/wiki/Population

---

## 10. Implementation Notes for Skirmish II

How the above maps onto Skirmish II's deterministic sim: pure Rust, I16F16 fixed-point, 60 ticks/sec, seeded PCG32 RNG inside the hashed state, per-tick FNV-1a hash.

### 10.1 Values that convert cleanly to ticks

Almost every documented time is a whole or half second, so tick conversion is exact: RoF 2.0 s = 120 t, 1.8 s = 108 t, 1.9 s = 114 t, 3.0 s = 180 t, 5.0 s = 300 t, 6.0 s = 360 t; training 21/22/25/27/30/35/46/51 s and research 20–190 s are all whole seconds. Attack delay is the only sub-second family: 0.35 s = 21 t exactly; the ≈0.5–0.68 s community values land on 30–41 t — pick per-unit integers and record them in the unit data table (the source values are approximate anyway, §2.2).

### 10.2 Fixed-point fit

- Damage math is integer-friendly: attack, armor, and bonus values are small integers; the only fractional multipliers are elevation (×1.25 / ×0.75 — both exact in binary fixed-point) and blast fractions (0.5, 0.33, 0.25 — use rational constants; 0.33 should be stored as the documented fraction per unit type, e.g. 1/3, and applied in a fixed order). HP fractional tracking (damage is fractional internally in the reference engine, death at HP < 1) maps directly onto I16F16 HP values with a `< 1.0` death check.
- Gather rates are the least clean fit: 0.388/s = 0.00647/tick is not exact in I16F16, but M2 already accumulates them in fixed-point successfully; keep accumulating in the resource node + carry as fixed-point and only floor at deposit/display, as M2 does.
- Speeds (0.8–1.55 tiles/s) are already handled by M1/M2 steering in fixed-point; per-tick displacement is speed/60.

### 10.3 Determinism watch-list

- **Accuracy rolls** are the first mechanic that *requires* RNG inside combat: one roll per shot, drawn from the sim's seeded PCG32 in a fixed iteration order (e.g. ascending entity id, then projectile spawn order). Never draw from wall-clock or platform RNG — the per-tick hash will catch violations in the determinism tests, as designed.
- **Scatter:** since numeric dispersion values are UNVERIFIED (§2.4), Skirmish II defines its own per-unit scatter radius. The miss offset must be generated from the sim RNG (two draws: angle, distance) at projectile spawn, and the resulting impact point stored in the projectile state — recomputing it later from re-seeded RNG would be fragile.
- **Projectiles** are entities in the sim state: spawn tick, fixed trajectory, impact tick precomputed from distance ÷ speed where possible. Prefer computing flight time in ticks at spawn (ceil of distance/speed × 60) over integrating positions per tick — fewer accumulations, bit-identical across platforms. Blast resolution happens once, at the impact tick, iterating targets in ascending entity id.
- **Frame delay / attack delay:** model as a per-unit integer tick counter started when an attack order begins; abort-on-retask (§2.2) means the counter is part of hashed unit state. Reload likewise. Sustained attacks skip the delay (reload only) — encode that distinction explicitly (`first_attack` flag or equivalent).
- **Mishit 50% and blast fractions** are multipliers applied at damage time, not stored — no state cost, but the *identity* of the hit unit (intended vs. mishit) must be decided deterministically at impact.
- **Elevation:** Skirmish II's current grid is flat. If elevation is added, the multiplier depends only on the sign of the height difference — store a per-cell height byte; comparison at damage time, no floats.
- **Auto-targeting** (§9.2) must iterate in a deterministic order: nearest by squared distance, ties broken by lowest entity id. This is also where the "closest unit → TC in LoS → other buildings" priority chain lives — implement it as an explicit priority function so M5 feel tuning can adjust it in one place.

### 10.4 Ambiguous / undocumented mechanics — decisions needed (AMBÍGUO summary)

| # | Item | What the sources say | Suggested default for M3 |
|---|---|---|---|
| 1 | "Non-member" armor value (§1.2) | 10,000 in worked examples; "usually 1000" in a formula note | Irrelevant numerically — implement as class-membership check, no sentinel value needed |
| 2 | Halberdier vs Camel / vs Std. Building (§3.3) | Classic per-class: +17 / +1. Current DE page totals: +26 / +3 | Decide before balance pass; headline vs-Cavalry numbers are solid |
| 3 | Militia-line bonus totals for Long Swordsman/Champion (§3.2) | Classic totals vs DE upgrade-note arithmetic differ | Use classic totals (+4/+6 Eagles, +2/+3 Buildings) as the visible design; flag in data table |
| 4 | Skirmisher train time (§3.5) | 22 s (multi-source) vs 26 s (one list) | 22 s |
| 5 | Elite Skirmisher vs Spearmen (§3.5) | +4 (changelog) vs +3 (comparison table) | +4 (changelog is a change record) |
| 6 | Siege Ram vs Buildings bonus & melee armor (§3.9) | Classic 200 / −3; current DE totals unconfirmed | Use classic values, mark in data |
| 7 | Cliff uphill penalty (§1.4) | One page: ×0.75 is hills-only | Moot until cliffs exist |
| 8 | Stance leash distances (§9.1) | Not documented anywhere public | Design decision; tune in M5 |
| 9 | Attack Move formal definition (§9.2) | Exists in DE; no formal public definition | Adopt conventional one-way definition |
| 10 | Per-unit collision radius (§4.2) | Only "≈0.2 typical, 0.15 Karambit" documented | Default 0.2 tiles |
| 11 | Scatter/dispersion radius (§2.4) | Mechanism documented, numbers not published | Per-unit parameter, tune by feel |
| 12 | Projectile speeds (§2.3) | Only individual values (Mangonel 3.5 t/s) | Per-unit parameter; arrows need a chosen value — tune by feel |
| 13 | Farm base food (§6.3) | **175** (250 is post-Horse-Collar) | Resolved: 175 — corrects the assumption in the study request itself |

---

## Source List

All sources consulted are public web pages. Primary: the Age of Empires II wiki (Fandom). Secondary: AoK Heaven university pages (classic community data), the Jineapple AoE2 data sheet, aoe2techtree.net's maintainer notes, and a GameFAQs community guide (patrol behavior only).

- https://ageofempires.fandom.com/wiki/Attack
- https://ageofempires.fandom.com/wiki/Armor_class_(Age_of_Empires_II)
- https://ageofempires.fandom.com/wiki/Armor_class:_Ram
- https://ageofempires.fandom.com/wiki/Armor
- https://ageofempires.fandom.com/wiki/Pierce_armor
- https://ageofempires.fandom.com/wiki/Elevation
- https://ageofempires.fandom.com/wiki/Rate_of_Fire
- https://ageofempires.fandom.com/wiki/Attack_delay
- https://ageofempires.fandom.com/wiki/Accuracy
- https://ageofempires.fandom.com/wiki/Area_of_Effect
- https://ageofempires.fandom.com/wiki/Projectile_duplication
- https://ageofempires.fandom.com/wiki/Masonry
- https://ageofempires.fandom.com/wiki/Villager_(Age_of_Empires_II)
- https://ageofempires.fandom.com/wiki/Infantry_unit_(Age_of_Empires_II)
- https://ageofempires.fandom.com/wiki/Foot_archer_unit_(Age_of_Empires_II)
- https://ageofempires.fandom.com/wiki/Archer_(Age_of_Empires_II)
- https://ageofempires.fandom.com/wiki/Halberdier_(Age_of_Empires_II)
- https://ageofempires.fandom.com/wiki/Cavalry_unit_(Age_of_Empires_II)
- https://ageofempires.fandom.com/wiki/Scout_Cavalry_(Age_of_Empires_II)
- https://ageofempires.fandom.com/wiki/Hussar_(Age_of_Empires_II)
- https://ageofempires.fandom.com/wiki/Mangonel_(Age_of_Empires_II)
- https://ageofempires.fandom.com/wiki/Onager_(Age_of_Empires_II)
- https://ageofempires.fandom.com/wiki/Siege_Onager
- https://ageofempires.fandom.com/wiki/Battering_Ram_(Age_of_Empires_II)
- https://ageofempires.fandom.com/wiki/Capped_Ram_(Age_of_Empires_II)
- https://ageofempires.fandom.com/wiki/Siege_Ram_(Age_of_Empires_II)
- https://ageofempires.fandom.com/wiki/Monk_(Age_of_Empires_II)
- https://ageofempires.fandom.com/wiki/Speed
- https://ageofempires.fandom.com/wiki/Karambit_Warrior
- https://ageofempires.fandom.com/wiki/Building_(Age_of_Empires_II)
- https://ageofempires.fandom.com/wiki/Town_Center_(Age_of_Empires_II)
- https://ageofempires.fandom.com/wiki/House_(Age_of_Empires_II)
- https://ageofempires.fandom.com/wiki/Watch_Tower_(Age_of_Empires_II)
- https://ageofempires.fandom.com/wiki/Castle_(Age_of_Empires_II)
- https://ageofempires.fandom.com/wiki/Farm_(Age_of_Empires_II)
- https://ageofempires.fandom.com/wiki/Mill_(Age_of_Empires_II)
- https://ageofempires.fandom.com/wiki/Resource
- https://ageofempires.fandom.com/wiki/Berry_Bush
- https://ageofempires.fandom.com/wiki/Stone
- https://ageofempires.fandom.com/wiki/Hand_Cart
- https://ageofempires.fandom.com/wiki/Technology_(Age_of_Empires_II)
- https://ageofempires.fandom.com/wiki/Feudal_Age_(Age_of_Empires_II)
- https://ageofempires.fandom.com/wiki/Castle_Age_(Age_of_Empires_II)
- https://ageofempires.fandom.com/wiki/Imperial_Age_(Age_of_Empires_II)
- https://ageofempires.fandom.com/wiki/Scale_Mail_Armor
- https://ageofempires.fandom.com/wiki/Unit_stance
- https://ageofempires.fandom.com/wiki/Garrison
- https://ageofempires.fandom.com/wiki/Herbal_Medicine_(Age_of_Empires_II)
- https://ageofempires.fandom.com/wiki/Population
- https://ageofempires.fandom.com/wiki/Rocket_Cart
- https://ageofempires.fandom.com/wiki/Genitour_(Age_of_Empires_II)
- https://ageofempires.fandom.com/wiki/Update_111772
- https://ageofempires.fandom.com/wiki/Update_141935
- https://aok.heavengames.com/university/game-info/stat-tables/units-table/
- https://aok.heavengames.com/university/game-info/game-statistics/units/infantry/spearman/
- https://github.com/siegeengineers/aoe2techtree/issues/133
- https://gamefaqs.gamespot.com/pc/260708-age-of-empires-ii-definitive-edition/faqs/38481

*Compiled 2026-10-16 from public documentation only. No game binary or game files were accessed at any point.*
