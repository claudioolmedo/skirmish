//! Unit data tables (M3) — every combat-relevant number in one place.
//!
//! All values come from the public mechanics documentation
//! (MECHANICS-STUDY §3/§4/§6, OPENING-SPEC §2) — behavior targets from
//! public documented numbers only, per the clean-room rule. Values the
//! docs do not pin are marked UNVERIFIED here and in the milestone
//! report; they are tuning constants, recorded on purpose, never
//! silently invented.
//!
//! Conventions:
//! - Speeds are the study's tiles/second values at the sim's
//!   ABSOLUTE tile scale (the pace correction, post-M7): M7 fixed
//!   1 tile = 1 grid cell = 4 world units, so every row follows
//!   `speed(units/tick) = tiles/s × 4 / 60`. Before this pass the
//!   table preserved only the study's ratios (× 1.25 off a
//!   1.0 units/tick villager inherited from M1/M2), which made
//!   absolute travel ~12× too fast — a villager crossed a tile in
//!   ~0.07 s instead of 1.25 s. I16F16 rounding moves individual
//!   rows by < 0.02% (0.8 tiles/s is 3495 bits vs the exact
//!   3495.25); determinism is unaffected — the rounded value IS
//!   the state, identical on every machine. (Loom's +0.1 tiles/s
//!   increment rounds to 437 bits, which lands the loomed villager
//!   exactly on the 0.90 tiles/s row.)
//! - Ranges and line-of-sight are tiles × 4 world units (one cell is
//!   one tile), so an Archer's range 4 is 16 world units and a Scout's
//!   Dark Age LoS 4 is 16 world units.
//! - Attack delay / reload are in ticks (60 Hz), from the study's
//!   attack-frame tables (§2.1/§2.2: reload is measured from the
//!   landing tick — RULEBOOK CMB-05).

use fixed::types::I16F16;

use crate::classes as cls;
use crate::entity::EntityKind;

/// One unit kind's full combat/stat row.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct UnitSpec {
    /// Hit points at spawn (fractional-capable in state; table values
    /// are whole). Death occurs when HP drops below 1 (CMB-02).
    pub max_hp: i32,
    /// Attack table: (armor class, value) pairs. The first pair is the
    /// base attack (melee or pierce class); the rest are bonuses that
    /// only land on members of their class (CLS-02: counters are
    /// emergent from these rows — the engine never branches on unit
    /// identity for damage).
    pub attacks: &'static [(u8, i32)],
    pub armor_melee: i32,
    pub armor_pierce: i32,
    /// Armor-class membership mask (see `classes`).
    pub classes: u64,
    /// Maximum attack range in world units; 0 = melee (the melee
    /// reach constant in `combat` governs instead).
    pub range_max: I16F16,
    /// Minimum attack range in world units (PRJ-08's dead zone):
    /// a ranged unit cannot fire at a target closer than this and
    /// repositions away instead. 0 for every row except the
    /// Skirmisher (1 tile), the game's first minimum-range weapon.
    pub range_min: I16F16,
    /// Ticks between landed attacks (the study's RoF × 60).
    pub reload_ticks: u64,
    /// Ticks from starting an engagement to its first landed attack
    /// (attack delay; first attack only — CMB-04).
    pub attack_delay_ticks: u64,
    /// Ranged accuracy, percent (melee attacks always land if in
    /// range at the landing tick — CMB-06). Only the Archer uses it.
    pub accuracy: u32,
    /// Projectile travel speed, world units per tick (ranged only).
    /// UNVERIFIED as a generic-arrow value: the docs pin individual
    /// weapons (Mangonel stone 3.5 tiles/s, Janissary bullet 7.5), no
    /// generic arrow speed — 0.5 units/tick is a recorded design
    /// constant (PRJ-02).
    pub projectile_speed: I16F16,
    /// Scatter radius for missed shots, world units. UNVERIFIED
    /// numerically in the docs (PRJ-04) — recorded design constant.
    pub scatter: I16F16,
    /// Movement speed, world units per tick (see module docs).
    pub speed: I16F16,
    /// Line of sight, world units (auto-acquisition radius).
    pub los: I16F16,
    /// Collision radius, world units (separation distance between two
    /// units is the sum of their radii — MOV-05).
    pub radius: I16F16,
    /// Food in the carcass this unit leaves when killed; 0 = no
    /// carcass (player units and the Town Center leave none in M3).
    pub carcass_food: i32,
    /// Carcass decay per tick, raw bits (ECO-12: carcasses are the
    /// only nodes that lose amount over time).
    pub carcass_decay_bits: i32,
    /// Production cost (Food / Wood / Gold / Stone components).
    pub food_cost: i32,
    pub wood_cost: i32,
    pub gold_cost: i32,
    /// Production time in ticks; 0 = not trainable.
    pub train_ticks: u64,
}

const NO_ATTACKS: &[(u8, i32)] = &[];

/// The stat row for one entity kind.
pub fn spec(kind: EntityKind) -> &'static UnitSpec {
    match kind {
        EntityKind::Villager => &VILLAGER,
        EntityKind::TownCenter => &TOWN_CENTER,
        EntityKind::ScoutCavalry => &SCOUT_CAVALRY,
        EntityKind::Militia => &MILITIA,
        EntityKind::Archer => &ARCHER,
        EntityKind::Sheep => &SHEEP,
        EntityKind::Boar => &BOAR,
        EntityKind::Deer => &DEER,
        EntityKind::ManAtArms => &MAN_AT_ARMS,
        EntityKind::Spearman => &SPEARMAN,
        EntityKind::Skirmisher => &SKIRMISHER,
        EntityKind::Wolf => &WOLF,
    }
}

/// The Archer's attack table with Fletching researched (M6, study
/// §8: +1 pierce attack; the +1 range is patched separately — see
/// `Sim::stats_for`). A second static table rather than a computed
/// row, so every attack table in the game stays `&'static` data.
pub(crate) static ARCHER_FLETCHING_ATTACKS: &[(u8, i32)] =
    &[(cls::PIERCE, 5), (cls::SPEARMAN, 3)];

/// The Scout Cavalry's Feudal attack table (M6, CLS-04: attack
/// 3 → 5 on reaching the Feudal Age; speed and LoS are patched
/// separately — see `Sim::stats_for`).
pub(crate) static SCOUT_FEUDAL_ATTACKS: &[(u8, i32)] = &[(cls::MELEE, 5)];

/// The Skirmisher's attack table with Fletching researched: the
/// tech's +1 pierce (the wiki's Fletching article names Skirmishers
/// explicitly among its beneficiaries — "arrow/javelin-shooting
/// units"), the class bonuses unchanged. The +1 range is patched
/// separately — see `Sim::stats_for`.
pub(crate) static SKIRMISHER_FLETCHING_ATTACKS: &[(u8, i32)] =
    &[(cls::PIERCE, 3), (cls::ARCHERY, 3), (cls::SPEARMAN, 3)];

/// Forging attack tables (2026-10-22; wiki, Forging: infantry
/// and cavalry melee attack +1). One static per affected row,
/// the Fletching pattern — every attack table in the game stays
/// `&'static` data, and `Sim::stats_for` picks the row. Only the
/// base (melee) pair rises; bonus pairs are untouched, exactly
/// the reference's arithmetic.
pub(crate) static MILITIA_FORGING_ATTACKS: &[(u8, i32)] = &[(cls::MELEE, 5)];
pub(crate) static MAN_AT_ARMS_FORGING_ATTACKS: &[(u8, i32)] = &[
    (cls::MELEE, 7),
    (cls::SHOCK_INFANTRY, 2),
    (cls::STANDARD_BUILDINGS, 2),
];
pub(crate) static SCOUT_FORGING_ATTACKS: &[(u8, i32)] = &[(cls::MELEE, 4)];
pub(crate) static SCOUT_FEUDAL_FORGING_ATTACKS: &[(u8, i32)] = &[(cls::MELEE, 6)];
/// The Spearman's Forging table: base melee 3 → 4; the whole
/// bonus ladder (cavalry 15, elephant 15, camel 12, ship 9,
/// shock 1, buildings 1) is untouched, as Forging's rule states.
pub(crate) static SPEARMAN_FORGING_ATTACKS: &[(u8, i32)] = &[
    (cls::MELEE, 4),
    (cls::CAVALRY, 15),
    (cls::ELEPHANT, 15),
    (cls::CAMEL, 12),
    (cls::SHIP, 9),
    (cls::SHOCK_INFANTRY, 1),
    (cls::STANDARD_BUILDINGS, 1),
];

/// Villager (study §3.1): HP 25, attack 3 melee, armor 0/0, RoF 2.0 s,
/// attack delay ≈0.64 s (community table), speed 0.8, LoS 4.
/// Cost 50 F / 1500 ticks is the Town Center training contract (econ).
static VILLAGER: UnitSpec = UnitSpec {
    max_hp: 25,
    attacks: &[(cls::MELEE, 3)],
    armor_melee: 0,
    armor_pierce: 0,
    classes: cls::mask(&[cls::VILLAGER]),
    range_max: I16F16::ZERO,
    range_min: I16F16::ZERO,
    reload_ticks: 120,
    attack_delay_ticks: 38,
    accuracy: 100,
    projectile_speed: I16F16::ZERO,
    scatter: I16F16::ZERO,
    speed: I16F16::from_bits(3_495), // 0.8 tiles/s (absolute scale, see header)
    los: I16F16::from_bits(1_048_576), // 16 (4 tiles)
    radius: I16F16::from_bits(65_536), // 1.0
    carcass_food: 0,
    carcass_decay_bits: 0,
    food_cost: 50,
    wood_cost: 0,
    gold_cost: 0,
    train_ticks: 1500,
};

/// Town Center (study §5, Dark Age row): HP 2400, armor 3/5.
/// Buildings take no damage in M3 ("units now, buildings later") — the
/// row exists so the client can display full HP; nothing targets it.
static TOWN_CENTER: UnitSpec = UnitSpec {
    max_hp: 2400,
    attacks: NO_ATTACKS,
    armor_melee: 3,
    armor_pierce: 5,
    classes: cls::mask(&[cls::ALL_BUILDINGS, cls::STANDARD_BUILDINGS]),
    range_max: I16F16::ZERO,
    range_min: I16F16::ZERO,
    reload_ticks: 0,
    attack_delay_ticks: 0,
    accuracy: 100,
    projectile_speed: I16F16::ZERO,
    scatter: I16F16::ZERO,
    speed: I16F16::ZERO,
    los: I16F16::ZERO,
    radius: I16F16::from_bits(235_930), // 3.6 — matches the client footprint half-extent
    carcass_food: 0,
    carcass_decay_bits: 0,
    food_cost: 0,
    wood_cost: 0,
    gold_cost: 0,
    train_ticks: 0,
};

/// Scout Cavalry, Dark Age row (study §3.5): HP 45, attack 3, armor
/// 0/2, RoF 2.0 s, attack delay ≈0.68 s (41 ticks), speed 1.20,
/// LoS 4. (Feudal row — attack 5, speed 1.55, LoS 6 — arrives with
/// ages, as age-keyed row patches per CLS-04.) Cost 80 F is documented
/// but the Scout is not trainable in M3: one spawns with the start.
static SCOUT_CAVALRY: UnitSpec = UnitSpec {
    max_hp: 45,
    attacks: &[(cls::MELEE, 3)],
    armor_melee: 0,
    armor_pierce: 2,
    classes: cls::mask(&[cls::CAVALRY]),
    range_max: I16F16::ZERO,
    range_min: I16F16::ZERO,
    reload_ticks: 120,
    attack_delay_ticks: 41,
    accuracy: 100,
    projectile_speed: I16F16::ZERO,
    scatter: I16F16::ZERO,
    speed: I16F16::from_bits(5_243), // 1.20 tiles/s (absolute scale, see header)
    los: I16F16::from_bits(1_048_576), // 16 (4 tiles, Dark Age)
    radius: I16F16::from_bits(65_536),
    carcass_food: 0,
    carcass_decay_bits: 0,
    food_cost: 80,
    wood_cost: 0,
    gold_cost: 0,
    train_ticks: 1800,
};

/// Militia (study §3.2): HP 40, attack 4 melee, armor 0/1, RoF 2.0 s,
/// attack delay ≈0.50 s (30 ticks), speed 0.90, LoS 4. The base tier
/// has no bonus attacks (Man-at-Arms gains them later). Cost
/// 50 F / 20 G — the current DE value: update 141935 (Chronicles:
/// Battle for Greece) removed Supplies and set the Militia line to
/// 50 F / 20 G (Age of Empires wiki, "Militia", Changelog). The
/// M3 brief's 60 F was the pre-141935 cost; corrected 2026-10-21.
/// Train 21 s.
static MILITIA: UnitSpec = UnitSpec {
    max_hp: 40,
    attacks: &[(cls::MELEE, 4)],
    armor_melee: 0,
    armor_pierce: 1,
    classes: cls::mask(&[cls::INFANTRY]),
    range_max: I16F16::ZERO,
    range_min: I16F16::ZERO,
    reload_ticks: 120,
    attack_delay_ticks: 30,
    accuracy: 100,
    projectile_speed: I16F16::ZERO,
    scatter: I16F16::ZERO,
    speed: I16F16::from_bits(3_932), // 0.90 tiles/s (absolute scale, see header)
    los: I16F16::from_bits(1_048_576), // 16 (4 tiles)
    radius: I16F16::from_bits(65_536),
    carcass_food: 0,
    carcass_decay_bits: 0,
    food_cost: 50,
    wood_cost: 0,
    gold_cost: 20,
    train_ticks: 1260, // 21 s
};

/// Archer (study §3.3): HP 30, attack 4 pierce, +3 vs Spearmen,
/// armor 0/0, range 4 (min 0), RoF 2.0 s, attack delay ≈0.35 s
/// (21 ticks — frame delay 15), accuracy 80%, speed 0.96, LoS 6.
/// Cost 25 W / 45 G, train 35 s. In the reference the Archer trains
/// at the Archery Range; M3 trains it at the Barracks by brief.
static ARCHER: UnitSpec = UnitSpec {
    max_hp: 30,
    attacks: &[(cls::PIERCE, 4), (cls::SPEARMAN, 3)],
    armor_melee: 0,
    armor_pierce: 0,
    classes: cls::mask(&[cls::ARCHERY]),
    range_max: I16F16::from_bits(1_048_576), // 16 (4 tiles)
    range_min: I16F16::ZERO,
    reload_ticks: 120,
    attack_delay_ticks: 21,
    accuracy: 80,
    projectile_speed: I16F16::from_bits(32_768), // 0.5 — design constant (see field doc)
    scatter: I16F16::from_bits(131_072),          // 2.0 — design constant (see field doc)
    speed: I16F16::from_bits(4_194),            // 0.96 tiles/s (absolute scale, see header)
    los: I16F16::from_bits(1_572_864),            // 24 (6 tiles)
    radius: I16F16::from_bits(65_536),
    carcass_food: 0,
    carcass_decay_bits: 0,
    food_cost: 0,
    wood_cost: 25,
    gold_cost: 45,
    train_ticks: 2100, // 35 s
};

/// Man-at-Arms (M6, study §3.2): the Militia line's Feudal tier —
/// HP 45, attack 6 melee with +2 vs Shock Infantry and +2 vs
/// Standard Buildings (the bonuses the base tier lacks), armor 0/1,
/// RoF 2.0 s, speed 0.96, LoS 4. Attack delay 30 ticks matches the
/// Militia row's adopted community value (the study's FD column
/// reads 0 for the whole line; the shipped Militia row records the
/// same choice). The cost fields carry the line's study cost
/// (50 F / 20 G) but are inert: Man-at-Arms is never trained
/// directly — it is produced by the Barracks upgrade (CLS-03), and
/// post-upgrade Militia production spawns this row instead.
static MAN_AT_ARMS: UnitSpec = UnitSpec {
    max_hp: 45,
    attacks: &[
        (cls::MELEE, 6),
        (cls::SHOCK_INFANTRY, 2),
        (cls::STANDARD_BUILDINGS, 2),
    ],
    armor_melee: 0,
    armor_pierce: 1,
    classes: cls::mask(&[cls::INFANTRY]),
    range_max: I16F16::ZERO,
    range_min: I16F16::ZERO,
    reload_ticks: 120,
    attack_delay_ticks: 30,
    accuracy: 100,
    projectile_speed: I16F16::ZERO,
    scatter: I16F16::ZERO,
    speed: I16F16::from_bits(4_194), // 0.96 tiles/s (absolute scale, see header)
    los: I16F16::from_bits(1_048_576), // 16 (4 tiles)
    radius: I16F16::from_bits(65_536),
    carcass_food: 0,
    carcass_decay_bits: 0,
    food_cost: 50,
    wood_cost: 0,
    gold_cost: 20,
    train_ticks: 1260, // inert (upgrade-only kind); mirrors Militia
};

/// Spearman (study §3.3; wiki "Spearman (Age of Empires II)"
/// infobox, fetched 2026-10-22): HP 45, attack 3 melee, armor 0/0,
/// RoF 3.0 s, attack delay ≈0.50 s (30 ticks — the study §2.2 melee
/// table's Militia/Spearman row), speed 1.0, LoS 4. The identity is
/// the bonus table: +15 vs Cavalry, +15 vs Elephant, +12 vs Camel,
/// +9 vs Ships, +1 vs Shock Infantry, +1 vs Standard Buildings.
/// One value supersedes the study: the study's per-class table
/// prints +9 vs Camel, but the wiki infobox's changelog tooltip
/// shows +9 was the value "from The Forgotten to patch 5.6" — the
/// current DE value is +12, adopted per the standing rule that the
/// fidelity reference is DE as it is today. Elephant and Ship have
/// no member kinds yet; their rows are data, like the Boar's Camel
/// row before them. LoS: the wiki infobox displays per-age values
/// (Dark 4 / Feudal 6, the same display it gives the Militia); the
/// study pins the line's LoS at 4 and only the Scout carries an
/// age-patched LoS in this sim (CLS-04), so 4 stands and the
/// per-age question is recorded in the rulebook. Cost 35 F / 25 W,
/// train 22 s, Barracks, Feudal Age (`EntityKind::train_age`).
static SPEARMAN: UnitSpec = UnitSpec {
    max_hp: 45,
    attacks: &[
        (cls::MELEE, 3),
        (cls::CAVALRY, 15),
        (cls::ELEPHANT, 15),
        (cls::CAMEL, 12),
        (cls::SHIP, 9),
        (cls::SHOCK_INFANTRY, 1),
        (cls::STANDARD_BUILDINGS, 1),
    ],
    armor_melee: 0,
    armor_pierce: 0,
    classes: cls::mask(&[cls::INFANTRY, cls::SPEARMAN]),
    range_max: I16F16::ZERO,
    range_min: I16F16::ZERO,
    reload_ticks: 180, // 3.0 s
    attack_delay_ticks: 30,
    accuracy: 100,
    projectile_speed: I16F16::ZERO,
    scatter: I16F16::ZERO,
    speed: I16F16::from_bits(4_369), // 1.0 tiles/s (absolute scale, see header)
    los: I16F16::from_bits(1_048_576), // 16 (4 tiles)
    radius: I16F16::from_bits(65_536),
    carcass_food: 0,
    carcass_decay_bits: 0,
    food_cost: 35,
    wood_cost: 25,
    gold_cost: 0,
    train_ticks: 1320, // 22 s
};

/// Skirmisher (study §3.5; wiki "Skirmisher (Age of Empires II)"
/// infobox, fetched 2026-10-22): HP 30, attack 2 pierce with
/// +3 vs Archers and +3 vs Spearmen, armor 0/3, range 1–4 (the
/// game's first minimum-range weapon, PRJ-08), RoF 3.0 s, frame
/// delay 19 ≈ 0.51 s (31 ticks, study §2.2), accuracy 90%,
/// speed 0.96, LoS 6. Cost 25 F / 35 W. Train time is the current
/// DE value, 26 s: the wiki infobox reads "26 (22 before update
/// 153015)", which supersedes the study §3.5 ambiguity resolution
/// that had adopted the then-multi-source 22 s — recorded in the
/// rulebook. The wiki also lists "+2 vs Mameluke" (a
/// civilization-specific unit that does not exist here) and
/// "0 vs Cavalry Archers" (a class with no member kinds); neither
/// row is carried. Projectile speed and scatter reuse the Archer
/// rows' recorded design constants (PRJ-02/PRJ-04).
static SKIRMISHER: UnitSpec = UnitSpec {
    max_hp: 30,
    attacks: &[(cls::PIERCE, 2), (cls::ARCHERY, 3), (cls::SPEARMAN, 3)],
    armor_melee: 0,
    armor_pierce: 3,
    classes: cls::mask(&[cls::ARCHERY, cls::SKIRMISHER]),
    range_max: I16F16::from_bits(1_048_576), // 16 (4 tiles)
    range_min: I16F16::from_bits(262_144),  // 4 (1 tile — PRJ-08)
    reload_ticks: 180,                      // 3.0 s
    attack_delay_ticks: 31,
    accuracy: 90,
    projectile_speed: I16F16::from_bits(32_768), // 0.5 — design constant (see Archer)
    scatter: I16F16::from_bits(131_072),          // 2.0 — design constant (see Archer)
    speed: I16F16::from_bits(4_194),             // 0.96 tiles/s (absolute scale, see header)
    los: I16F16::from_bits(1_572_864),           // 24 (6 tiles)
    radius: I16F16::from_bits(65_536),
    carcass_food: 0,
    carcass_decay_bits: 0,
    food_cost: 25,
    wood_cost: 35,
    gold_cost: 0,
    train_ticks: 1560, // 26 s (update 153015 — see above)
};

/// Sheep (OPENING-SPEC §2.2): 100 food, speed 0.7, worked at the
/// shepherd rate once dead. Line of Sight 2 tiles: the wiki's
/// Herdable animal article gives the per-animal breakdown ("Cows,
/// Sheep, and Turkeys have 2 Line of Sight, while the rest have
/// 3"); the Sheep article's prose says 3 — the per-animal table is
/// adopted and the conflict is recorded in the rulebook (AI-06).
/// HP 7 VERIFIED (AI-06): the wiki's Turkey and Goat articles
/// give their stats as the Sheep's — 7 HP, 100 food — and the
/// Cow article corroborates; a villager's 3-damage strikes kill
/// in 3 hits, matching the reference feel that sheep fall quickly
/// but not to a single poke.
static SHEEP: UnitSpec = UnitSpec {
    max_hp: 7, // VERIFIED (AI-06, see above)
    attacks: NO_ATTACKS,
    armor_melee: 0,
    armor_pierce: 0,
    classes: cls::mask(&[cls::HERDABLE]),
    range_max: I16F16::ZERO,
    range_min: I16F16::ZERO,
    reload_ticks: 0,
    attack_delay_ticks: 0,
    accuracy: 100,
    projectile_speed: I16F16::ZERO,
    scatter: I16F16::ZERO,
    speed: I16F16::from_bits(3_058), // 0.7 tiles/s (absolute scale, see header)
    los: I16F16::from_bits(524_288),  // 8 (2 tiles — wiki, Herdable animal)
    radius: I16F16::from_bits(65_536),
    carcass_food: 100,
    carcass_decay_bits: 273, // 0.25 food/s (ECO-12)
    food_cost: 0,
    wood_cost: 0,
    gold_cost: 0,
    train_ticks: 0,
};

/// Wild Boar (OPENING-SPEC §2.2, study §3.10): HP 75, 340 food,
/// attack 7 melee with +3 vs Cavalry / +4 vs Shock Infantry / +8 vs
/// Camel, ~2.0 s strike cycle, speed 1.0 attacking / 0.8 roaming.
/// Attack delay UNVERIFIED — 30 ticks (the melee-infantry value) is
/// adopted as a tuning constant.
static BOAR: UnitSpec = UnitSpec {
    max_hp: 75,
    attacks: &[
        (cls::MELEE, 7),
        (cls::CAVALRY, 3),
        (cls::SHOCK_INFANTRY, 4),
        (cls::CAMEL, 8),
    ],
    armor_melee: 0,
    armor_pierce: 0,
    classes: cls::mask(&[cls::BOAR_HUNTED]),
    range_max: I16F16::ZERO,
    range_min: I16F16::ZERO,
    reload_ticks: 120,
    attack_delay_ticks: 30, // UNVERIFIED (see above)
    accuracy: 100,
    projectile_speed: I16F16::ZERO,
    scatter: I16F16::ZERO,
    speed: I16F16::from_bits(3_495), // 0.8 tiles/s roaming/return (absolute scale); chase uses BOAR_CHASE_SPEED
    los: I16F16::from_bits(1_310_720), // 20 (5 tiles)
    radius: I16F16::from_bits(78_643), // 1.2 — the boar is bulky
    carcass_food: 340,
    carcass_decay_bits: 437, // 0.40 food/s hunted-animal rate (SPEC §2.2)
    food_cost: 0,
    wood_cost: 0,
    gold_cost: 0,
    train_ticks: 0,
};

/// The boar's chase speed while it has an aggro target: its
/// 1.0 tiles/s attacking speed at the absolute tile scale (see
/// the module header for the conversion).
pub const BOAR_CHASE_SPEED: I16F16 = I16F16::from_bits(4_369);

/// Wolf (AI-08; Age of Empires wiki, "Wolf" — fetched 2026-10-22):
/// HP 25, attack 3 melee, armor 0/0, and it "holds no food" — a
/// kill leaves no carcass (`carcass_food` 0). Line of Sight 12 on
/// Standard difficulty and harder (the wiki's difficulty ladder:
/// 4 on Easiest, 6 on Easy, 12 otherwise) — the wolf's LoS doubles
/// as its aggression radius in `combat::wolf_ai`. UNVERIFIED
/// fields, adopted as recorded tuning constants: speed 1.0 tiles/s
/// (the boar's documented chase speed — faster than a villager's
/// 0.8 so a chase is a real threat, slower than a Scout's 1.2; the
/// wiki's two-animation note — walk vs run-when-attacking — is
/// client presentation), reload 120 ticks and attack delay 30
/// (the house melee rows: villager/militia/boar).
static WOLF: UnitSpec = UnitSpec {
    max_hp: 25,
    attacks: &[(cls::MELEE, 3)],
    armor_melee: 0,
    armor_pierce: 0,
    classes: cls::mask(&[cls::PREDATOR]),
    range_max: I16F16::ZERO,
    range_min: I16F16::ZERO,
    reload_ticks: 120,       // UNVERIFIED (see above)
    attack_delay_ticks: 30,  // UNVERIFIED (see above)
    accuracy: 100,
    projectile_speed: I16F16::ZERO,
    scatter: I16F16::ZERO,
    speed: I16F16::from_bits(4_369), // 1.0 tiles/s (absolute scale) — UNVERIFIED
    los: I16F16::from_bits(3_145_728), // 48 (12 tiles — wiki, Wolf)
    radius: I16F16::from_bits(65_536), // 1.0
    carcass_food: 0,                  // "holds no food" — no carcass
    carcass_decay_bits: 0,
    food_cost: 0,
    wood_cost: 0,
    gold_cost: 0,
    train_ticks: 0,
};

/// Deer (OPENING-SPEC §2.2): 140 food, passive, flees when attacked,
/// decays at the herdable 0.25/s. HP UNVERIFIED — the spec only
/// records "two villager shots kill one", which bounds HP at ≤ 6
/// against a villager's 3 damage; 6 adopted. Speed UNVERIFIED —
/// 1.0 tiles/s adopted (it must out-walk a villager to be pushable
/// rather than trivially cornered).
static DEER: UnitSpec = UnitSpec {
    max_hp: 6, // UNVERIFIED (bounded by the spec, see above)
    attacks: NO_ATTACKS,
    armor_melee: 0,
    armor_pierce: 0,
    classes: cls::mask(&[cls::BOAR_HUNTED]),
    range_max: I16F16::ZERO,
    range_min: I16F16::ZERO,
    reload_ticks: 0,
    attack_delay_ticks: 0,
    accuracy: 100,
    projectile_speed: I16F16::ZERO,
    scatter: I16F16::ZERO,
    speed: I16F16::from_bits(4_369), // 1.0 tiles/s (absolute scale) — UNVERIFIED
    los: I16F16::from_bits(1_048_576),
    radius: I16F16::from_bits(65_536),
    carcass_food: 140,
    carcass_decay_bits: 273, // 0.25 food/s
    food_cost: 0,
    wood_cost: 0,
    gold_cost: 0,
    train_ticks: 0,
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn table_matches_the_study_rows() {
        let v = spec(EntityKind::Villager);
        assert_eq!(v.max_hp, 25);
        assert_eq!(v.attacks, &[(cls::MELEE, 3)]);
        let m = spec(EntityKind::Militia);
        assert_eq!((m.max_hp, m.armor_melee, m.armor_pierce), (40, 0, 1));
        assert_eq!(m.train_ticks, 21 * 60);
        let a = spec(EntityKind::Archer);
        assert_eq!((a.max_hp, a.accuracy), (30, 80));
        assert_eq!(a.attacks, &[(cls::PIERCE, 4), (cls::SPEARMAN, 3)]);
        let s = spec(EntityKind::ScoutCavalry);
        assert_eq!((s.max_hp, s.armor_pierce), (45, 2));
        // Man-at-Arms (M6, study §3.2): HP 45, attack 6 + the two
        // bonus rows, armor 0/1.
        let maa = spec(EntityKind::ManAtArms);
        assert_eq!((maa.max_hp, maa.armor_melee, maa.armor_pierce), (45, 0, 1));
        assert_eq!(
            maa.attacks,
            &[
                (cls::MELEE, 6),
                (cls::SHOCK_INFANTRY, 2),
                (cls::STANDARD_BUILDINGS, 2)
            ]
        );
        let b = spec(EntityKind::Boar);
        assert_eq!((b.max_hp, b.carcass_food), (75, 340));
        assert_eq!(spec(EntityKind::Sheep).carcass_food, 100);
        assert_eq!(spec(EntityKind::Deer).carcass_food, 140);
        // Spearman (study §3.3 + wiki infobox): HP 45, the full
        // documented bonus table, train 22 s at 35 F / 25 W.
        let sp = spec(EntityKind::Spearman);
        assert_eq!((sp.max_hp, sp.armor_melee, sp.armor_pierce), (45, 0, 0));
        assert_eq!(
            sp.attacks,
            &[
                (cls::MELEE, 3),
                (cls::CAVALRY, 15),
                (cls::ELEPHANT, 15),
                (cls::CAMEL, 12),
                (cls::SHIP, 9),
                (cls::SHOCK_INFANTRY, 1),
                (cls::STANDARD_BUILDINGS, 1)
            ]
        );
        assert_eq!((sp.reload_ticks, sp.train_ticks), (180, 1320));
        assert_eq!((sp.food_cost, sp.wood_cost, sp.gold_cost), (35, 25, 0));
        // Skirmisher (study §3.5 + wiki infobox): HP 30, armor 0/3,
        // range band 1–4 tiles, accuracy 90, train 26 s (update
        // 153015) at 25 F / 35 W.
        let sk = spec(EntityKind::Skirmisher);
        assert_eq!((sk.max_hp, sk.armor_melee, sk.armor_pierce), (30, 0, 3));
        assert_eq!(
            sk.attacks,
            &[(cls::PIERCE, 2), (cls::ARCHERY, 3), (cls::SPEARMAN, 3)]
        );
        assert_eq!(sk.range_max, I16F16::from_bits(1_048_576));
        assert_eq!(sk.range_min, I16F16::from_bits(262_144));
        assert_eq!((sk.reload_ticks, sk.accuracy, sk.train_ticks), (180, 90, 1560));
        assert_eq!((sk.food_cost, sk.wood_cost, sk.gold_cost), (25, 35, 0));
        // Every other row keeps a zero dead zone (PRJ-08).
        for k in [
            EntityKind::Villager,
            EntityKind::ScoutCavalry,
            EntityKind::Militia,
            EntityKind::Archer,
            EntityKind::ManAtArms,
            EntityKind::Spearman,
        ] {
            assert_eq!(spec(k).range_min, I16F16::ZERO);
        }
        // Wolf (AI-08; wiki, Wolf): HP 25, attack 3, no food.
        let w = spec(EntityKind::Wolf);
        assert_eq!((w.max_hp, w.carcass_food), (25, 0));
        assert_eq!(w.attacks, &[(cls::MELEE, 3)]);
    }

    #[test]
    fn speed_ratios_match_the_study() {
        // Study tiles/s: villager 0.8, militia 0.90, archer 0.96,
        // scout 1.20, sheep 0.7. Ratios must hold exactly (within the
        // fixed-point rounding of the 1.2 value).
        let v = spec(EntityKind::Villager).speed;
        let ratio = |k: EntityKind| spec(k).speed.to_num::<f64>() / v.to_num::<f64>();
        assert!((ratio(EntityKind::Militia) - 0.90 / 0.80).abs() < 0.001);
        assert!((ratio(EntityKind::Archer) - 0.96 / 0.80).abs() < 0.001);
        assert!((ratio(EntityKind::Spearman) - 1.00 / 0.80).abs() < 0.001);
        assert!((ratio(EntityKind::Skirmisher) - 0.96 / 0.80).abs() < 0.001);
        assert!((ratio(EntityKind::ScoutCavalry) - 1.20 / 0.80).abs() < 0.001);
        assert!((ratio(EntityKind::Sheep) - 0.70 / 0.80).abs() < 0.001);
        // The scout is the fastest unit on the field.
        assert!(spec(EntityKind::ScoutCavalry).speed > spec(EntityKind::Archer).speed);
    }
}
