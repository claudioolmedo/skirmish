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
    /// Armor against the anti-building attack class
    /// (STANDARD_BUILDINGS pairs — the Petard's +500 today):
    /// DE's "building armor". 0 on every unit row; placed
    /// buildings and the Town Center carry theirs through
    /// `Sim::building_armor` / the Town Center patch in
    /// `Sim::stats_for` (Masonry's +3, the Fortified Wall's 8 —
    /// Castle P3c).
    pub armor_building: i32,
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
    /// Blast radius in world units (PRJ-07, Castle P4b): ZERO =
    /// not a blast weapon. A blast weapon's projectile resolves
    /// its whole attack table over every unit and building inside
    /// this radius of the impact point — distance-tapered,
    /// accuracy rolled per unit, friend and foe alike (CMB-12) —
    /// instead of the single-target resolution (see
    /// `combat::Sim::resolve_projectile`'s blast branch).
    pub blast_radius: I16F16,
    /// Pass-through flag (PRJ-09): the Scorpion line's bolt
    /// pierces its target and keeps travelling. Inert in P4b —
    /// the field lands with the shared row work so P4c's slice
    /// adds no row churn; only the Scorpion row sets it.
    pub pass_through: bool,
    /// Extra minimum-damage projectiles released with the main
    /// shot (PRJ-07's secondary projectiles, Castle P4b): the
    /// Mangonel line throws 5 extra stones with the main one
    /// (wiki, Mangonel article — update 153015 names them
    /// individually, "even when using attack ground"); each
    /// applies minimum damage (1) to everything inside the
    /// blast radius, inside the same blast pass, with no extra
    /// RNG draws. 0 on every other row.
    pub blast_secondaries: u8,
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
    /// CLS-06: kings and heroes are data flags in the unit row,
    /// not kinds with special code paths. `regicide_unit`: the
    /// death sweep eliminates this unit's owner when it dies
    /// (VIC-06). Only the King's row sets it.
    pub regicide_unit: bool,
    /// CLS-06: the unit can never be converted (VIC-06's King;
    /// heroes likewise). Read by the conversion eligibility gate
    /// (`Sim::eligible_target`, MNK-08) since the Package C
    /// landing put the King on a roster where conversion exists.
    pub immune_conversion: bool,
    /// CLS-06: hit points regenerated per second (the heroes'
    /// rate column). 0 = no regeneration. The King does NOT
    /// regenerate (wiki, King) — only heroes do, and no hero
    /// exists in the game yet, so no code path reads this yet.
    pub regenerates: i32,
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
        EntityKind::Knight => &KNIGHT,
        EntityKind::LightCavalry => &LIGHT_CAVALRY,
        EntityKind::CavalryArcher => &CAVALRY_ARCHER,
        EntityKind::Petard => &PETARD,
        EntityKind::TradeCart => &TRADE_CART,
        EntityKind::BatteringRam => &BATTERING_RAM,
        EntityKind::Mangonel => &MANGONEL,
        EntityKind::Scorpion => &SCORPION,
        EntityKind::Monk => &MONK,
        EntityKind::Relic => &RELIC,
        EntityKind::King => &KING,
        EntityKind::FishingShip => &FISHING_SHIP,
        EntityKind::Galley => &GALLEY,
        EntityKind::FireGalley => &FIRE_GALLEY,
        EntityKind::DemolitionRaft => &DEMOLITION_RAFT,
        EntityKind::Hulk => &HULK,
        EntityKind::WarGalley => &WAR_GALLEY,
        EntityKind::FireShip => &FIRE_SHIP,
        EntityKind::DemolitionShip => &DEMOLITION_SHIP,
        EntityKind::WarHulk => &WAR_HULK,
        EntityKind::TransportShip => &TRANSPORT_SHIP,
        EntityKind::TradeCog => &TRADE_COG,
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
/// separately — see `Sim::stats_for`). The +6 vs Monks bonus rides
/// along unchanged — it is age-independent in the reference (the
/// Light Cavalry upgrade later adds +4 more on top of it; wiki,
/// "Light Cavalry (Age of Empires II)", Changelog).
pub(crate) static SCOUT_FEUDAL_ATTACKS: &[(u8, i32)] = &[(cls::MELEE, 5), (cls::MONK, 6)];

/// The Cavalry Archer's attack table with Fletching researched
/// (Castle P2): the tech's +1 pierce; the wiki's Cavalry Archer
/// article lists Fletching among the unit's attack technologies.
/// The +1 range is patched separately — see `Sim::stats_for`.
pub(crate) static CAVALRY_ARCHER_FLETCHING_ATTACKS: &[(u8, i32)] =
    &[(cls::PIERCE, 7), (cls::SPEARMAN, 2)];

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

/// The Galley's attack table with Fletching researched (Water
/// W3a, CLS-07): the tech's +1 pierce (the wiki's Galley article
/// lists Fletching among its attack technologies); the bonus
/// pairs are untouched, exactly the reference's arithmetic. The
/// +1 range is patched separately — see `Sim::stats_for`.
pub(crate) static GALLEY_FLETCHING_ATTACKS: &[(u8, i32)] = &[
    (cls::PIERCE, 7),
    (cls::LONG_RANGE_WARSHIP, 4),
    (cls::STANDARD_BUILDINGS, 6),
    (cls::RAM, 3),
];

/// The Hulk's attack table with Forging researched (Water W3a,
/// CLS-07): the tech's +1 melee (the wiki's Forging article
/// names the Hulk line explicitly — "Infantry, Cavalry and
/// Hulk-line +1 attack" in `tech.rs`'s module docs; the Hulk
/// article's own technology table agrees); the +1 vs Fire Ships
/// pair is untouched.
pub(crate) static HULK_FORGING_ATTACKS: &[(u8, i32)] =
    &[(cls::MELEE, 5), (cls::FIRE_SHIP, 1)];

/// The War Galley's attack table with Fletching researched
/// (Water W3b, CLS-07): the same +1 pierce swap on the Castle
/// tier's table (the wiki's War Galley article lists Fletching
/// among its attack technologies, like the Galley's); the bonus
/// pairs are untouched. The +1 range is patched separately —
/// see `Sim::stats_for`.
pub(crate) static WAR_GALLEY_FLETCHING_ATTACKS: &[(u8, i32)] = &[
    (cls::PIERCE, 8),
    (cls::LONG_RANGE_WARSHIP, 5),
    (cls::STANDARD_BUILDINGS, 6),
    (cls::RAM, 3),
];

/// The War Hulk's attack table with Forging researched (Water
/// W3b, CLS-07): the tech's +1 melee on the Castle tier (the
/// War Hulk article's technology table lists Forging, like the
/// Hulk's); the +1 vs Fire Ships pair is untouched.
pub(crate) static WAR_HULK_FORGING_ATTACKS: &[(u8, i32)] =
    &[(cls::MELEE, 5), (cls::FIRE_SHIP, 1)];

/// Villager (study §3.1): HP 25, attack 3 melee, armor 0/0, RoF 2.0 s,
/// attack delay ≈0.64 s (community table), speed 0.8, LoS 4.
/// Cost 50 F / 1500 ticks is the Town Center training contract (econ).
static VILLAGER: UnitSpec = UnitSpec {
    max_hp: 25,
    attacks: &[(cls::MELEE, 3)],
    armor_melee: 0,
    armor_pierce: 0,
    armor_building: 0,
    classes: cls::mask(&[cls::VILLAGER]),
    range_max: I16F16::ZERO,
    range_min: I16F16::ZERO,
    reload_ticks: 120,
    attack_delay_ticks: 38,
    accuracy: 100,
    projectile_speed: I16F16::ZERO,
    scatter: I16F16::ZERO,
    blast_radius: I16F16::ZERO,
    pass_through: false,
    blast_secondaries: 0,
    speed: I16F16::from_bits(3_495), // 0.8 tiles/s (absolute scale, see header)
    los: I16F16::from_bits(1_048_576), // 16 (4 tiles)
    radius: I16F16::from_bits(65_536), // 1.0
    carcass_food: 0,
    carcass_decay_bits: 0,
    food_cost: 50,
    wood_cost: 0,
    gold_cost: 0,
    train_ticks: 1500,
    regicide_unit: false,
    immune_conversion: false,
    regenerates: 0,
};

/// Town Center (study §5, Dark Age row): HP 2400, armor 3/5.
/// Buildings take no damage in M3 ("units now, buildings later") — the
/// row exists so the client can display full HP; nothing targets it.
static TOWN_CENTER: UnitSpec = UnitSpec {
    max_hp: 2400,
    attacks: NO_ATTACKS,
    armor_melee: 3,
    armor_pierce: 5,
    armor_building: 0,
    classes: cls::mask(&[cls::ALL_BUILDINGS, cls::STANDARD_BUILDINGS]),
    range_max: I16F16::ZERO,
    range_min: I16F16::ZERO,
    reload_ticks: 0,
    attack_delay_ticks: 0,
    accuracy: 100,
    projectile_speed: I16F16::ZERO,
    scatter: I16F16::ZERO,
    blast_radius: I16F16::ZERO,
    pass_through: false,
    blast_secondaries: 0,
    speed: I16F16::ZERO,
    los: I16F16::ZERO,
    radius: I16F16::from_bits(235_930), // 3.6 — matches the client footprint half-extent
    carcass_food: 0,
    carcass_decay_bits: 0,
    food_cost: 0,
    wood_cost: 0,
    gold_cost: 0,
    train_ticks: 0,
    regicide_unit: false,
    immune_conversion: false,
    regenerates: 0,
};

/// Scout Cavalry, Dark Age row (study §3.5): HP 45, attack 3, armor
/// 0/2, RoF 2.0 s, attack delay ≈0.68 s (41 ticks), speed 1.20,
/// LoS 4. (Feudal row — attack 5, speed 1.55, LoS 6 — arrives with
/// ages, as age-keyed row patches per CLS-04.) Cost 80 F is documented
/// but the Scout is not trainable in M3: one spawns with the start.
/// The +6 vs Monks bonus (Castle P2) is the line's documented
/// anti-Monk row — the Light Cavalry article's changelog records
/// the Castle upgrade granting "+4 anti-Monk attack" over the
/// Scout's existing bonus on the way to the Light Cavalry's +10;
/// no monk kind exists yet, so the row is data (see cls::MONK).
static SCOUT_CAVALRY: UnitSpec = UnitSpec {
    max_hp: 45,
    attacks: &[(cls::MELEE, 3), (cls::MONK, 6)],
    armor_melee: 0,
    armor_pierce: 2,
    armor_building: 0,
    classes: cls::mask(&[cls::CAVALRY]),
    range_max: I16F16::ZERO,
    range_min: I16F16::ZERO,
    reload_ticks: 120,
    attack_delay_ticks: 41,
    accuracy: 100,
    projectile_speed: I16F16::ZERO,
    scatter: I16F16::ZERO,
    blast_radius: I16F16::ZERO,
    pass_through: false,
    blast_secondaries: 0,
    speed: I16F16::from_bits(5_243), // 1.20 tiles/s (absolute scale, see header)
    los: I16F16::from_bits(1_048_576), // 16 (4 tiles, Dark Age)
    radius: I16F16::from_bits(65_536),
    carcass_food: 0,
    carcass_decay_bits: 0,
    food_cost: 80,
    wood_cost: 0,
    gold_cost: 0,
    train_ticks: 1800,
    regicide_unit: false,
    immune_conversion: false,
    regenerates: 0,
};

/// Militia (study §3.2): HP 40, attack 4 melee, armor 0/1, RoF 2.0 s,
/// attack delay ≈0.50 s (30 ticks), speed 0.90, LoS 4. The base tier
/// has no bonus attacks (Man-at-Arms gains them later). Cost
/// 60 F / 20 G — Claudio's decision, 2026-11-05 (voice call, main
/// chat): "o custo da Militia fica 60F/20G — fiel ao AoE2 DE, sem
/// exceção". This reverses the 2026-10-21 correction (commit
/// 9471419), which had adopted update 141935's 50 F / 20 G (the
/// Chronicles: Battle for Greece patch had removed Supplies and set
/// the line to 50 F). The owner's word is the fidelity call for
/// this project and there is no exception path: 60 F / 20 G.
/// Train 21 s.
static MILITIA: UnitSpec = UnitSpec {
    max_hp: 40,
    attacks: &[(cls::MELEE, 4)],
    armor_melee: 0,
    armor_pierce: 1,
    armor_building: 0,
    classes: cls::mask(&[cls::INFANTRY]),
    range_max: I16F16::ZERO,
    range_min: I16F16::ZERO,
    reload_ticks: 120,
    attack_delay_ticks: 30,
    accuracy: 100,
    projectile_speed: I16F16::ZERO,
    scatter: I16F16::ZERO,
    blast_radius: I16F16::ZERO,
    pass_through: false,
    blast_secondaries: 0,
    speed: I16F16::from_bits(3_932), // 0.90 tiles/s (absolute scale, see header)
    los: I16F16::from_bits(1_048_576), // 16 (4 tiles)
    radius: I16F16::from_bits(65_536),
    carcass_food: 0,
    carcass_decay_bits: 0,
    food_cost: 60,
    wood_cost: 0,
    gold_cost: 20,
    train_ticks: 1260, // 21 s
    regicide_unit: false,
    immune_conversion: false,
    regenerates: 0,
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
    armor_building: 0,
    classes: cls::mask(&[cls::ARCHERY]),
    range_max: I16F16::from_bits(1_048_576), // 16 (4 tiles)
    range_min: I16F16::ZERO,
    reload_ticks: 120,
    attack_delay_ticks: 21,
    accuracy: 80,
    projectile_speed: I16F16::from_bits(32_768), // 0.5 — design constant (see field doc)
    scatter: I16F16::from_bits(131_072),          // 2.0 — design constant (see field doc)
    blast_radius: I16F16::ZERO,
    pass_through: false,
    blast_secondaries: 0,
    speed: I16F16::from_bits(4_194),            // 0.96 tiles/s (absolute scale, see header)
    los: I16F16::from_bits(1_572_864),            // 24 (6 tiles)
    radius: I16F16::from_bits(65_536),
    carcass_food: 0,
    carcass_decay_bits: 0,
    food_cost: 0,
    wood_cost: 25,
    gold_cost: 45,
    train_ticks: 2100, // 35 s
    regicide_unit: false,
    immune_conversion: false,
    regenerates: 0,
};

/// Man-at-Arms (M6, study §3.2): the Militia line's Feudal tier —
/// HP 45, attack 6 melee with +2 vs Shock Infantry and +2 vs
/// Standard Buildings (the bonuses the base tier lacks), armor 0/1,
/// RoF 2.0 s, speed 0.96, LoS 4. Attack delay 30 ticks matches the
/// Militia row's adopted community value (the study's FD column
/// reads 0 for the whole line; the shipped Militia row records the
/// same choice). The cost fields carry the line's cost
/// (60 F / 20 G — Claudio's 2026-11-05 decision, see the Militia
/// row) but are inert for training: Man-at-Arms is never trained
/// directly — it is produced by the Barracks upgrade (CLS-03), and
/// post-upgrade Militia production spawns this row instead. The
/// fields still feed the row's score value (VIC-04), exactly as
/// the Light Cavalry row mirrors the Scout Cavalry's 80 F.
static MAN_AT_ARMS: UnitSpec = UnitSpec {
    max_hp: 45,
    attacks: &[
        (cls::MELEE, 6),
        (cls::SHOCK_INFANTRY, 2),
        (cls::STANDARD_BUILDINGS, 2),
    ],
    armor_melee: 0,
    armor_pierce: 1,
    armor_building: 0,
    classes: cls::mask(&[cls::INFANTRY]),
    range_max: I16F16::ZERO,
    range_min: I16F16::ZERO,
    reload_ticks: 120,
    attack_delay_ticks: 30,
    accuracy: 100,
    projectile_speed: I16F16::ZERO,
    scatter: I16F16::ZERO,
    blast_radius: I16F16::ZERO,
    pass_through: false,
    blast_secondaries: 0,
    speed: I16F16::from_bits(4_194), // 0.96 tiles/s (absolute scale, see header)
    los: I16F16::from_bits(1_048_576), // 16 (4 tiles)
    radius: I16F16::from_bits(65_536),
    carcass_food: 0,
    carcass_decay_bits: 0,
    food_cost: 60,
    wood_cost: 0,
    gold_cost: 20,
    train_ticks: 1260, // inert (upgrade-only kind); mirrors Militia
    regicide_unit: false,
    immune_conversion: false,
    regenerates: 0,
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
    armor_building: 0,
    classes: cls::mask(&[cls::INFANTRY, cls::SPEARMAN]),
    range_max: I16F16::ZERO,
    range_min: I16F16::ZERO,
    reload_ticks: 180, // 3.0 s
    attack_delay_ticks: 30,
    accuracy: 100,
    projectile_speed: I16F16::ZERO,
    scatter: I16F16::ZERO,
    blast_radius: I16F16::ZERO,
    pass_through: false,
    blast_secondaries: 0,
    speed: I16F16::from_bits(4_369), // 1.0 tiles/s (absolute scale, see header)
    los: I16F16::from_bits(1_048_576), // 16 (4 tiles)
    radius: I16F16::from_bits(65_536),
    carcass_food: 0,
    carcass_decay_bits: 0,
    food_cost: 35,
    wood_cost: 25,
    gold_cost: 0,
    train_ticks: 1320, // 22 s
    regicide_unit: false,
    immune_conversion: false,
    regenerates: 0,
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
    armor_building: 0,
    classes: cls::mask(&[cls::ARCHERY, cls::SKIRMISHER]),
    range_max: I16F16::from_bits(1_048_576), // 16 (4 tiles)
    range_min: I16F16::from_bits(262_144),  // 4 (1 tile — PRJ-08)
    reload_ticks: 180,                      // 3.0 s
    attack_delay_ticks: 31,
    accuracy: 90,
    projectile_speed: I16F16::from_bits(32_768), // 0.5 — design constant (see Archer)
    scatter: I16F16::from_bits(131_072),          // 2.0 — design constant (see Archer)
    blast_radius: I16F16::ZERO,
    pass_through: false,
    blast_secondaries: 0,
    speed: I16F16::from_bits(4_194),             // 0.96 tiles/s (absolute scale, see header)
    los: I16F16::from_bits(1_572_864),           // 24 (6 tiles)
    radius: I16F16::from_bits(65_536),
    carcass_food: 0,
    carcass_decay_bits: 0,
    food_cost: 25,
    wood_cost: 35,
    gold_cost: 0,
    train_ticks: 1560, // 26 s (update 153015 — see above)
    regicide_unit: false,
    immune_conversion: false,
    regenerates: 0,
};

/// Knight (Castle P2; wiki "Knight (Age of Empires II)" infobox,
/// fetched 2026-10-22): HP 100, attack 10 melee with NO bonus rows
/// (the Knight's identity is raw stats), armor 2/2, RoF 1.8 s,
/// speed 1.35, LoS 4. Cost 60 F / 75 G, train 30 s, Stable,
/// Castle Age (`EntityKind::train_age`). Attack delay adopts the
/// Scout Cavalry's 41 ticks — the sim's only other cavalry row;
/// the wiki article pins no separate Knight frame value in its
/// text, so the adoption is recorded here as a design constant.
static KNIGHT: UnitSpec = UnitSpec {
    max_hp: 100,
    attacks: &[(cls::MELEE, 10)],
    armor_melee: 2,
    armor_pierce: 2,
    armor_building: 0,
    classes: cls::mask(&[cls::CAVALRY]),
    range_max: I16F16::ZERO,
    range_min: I16F16::ZERO,
    reload_ticks: 108, // 1.8 s
    attack_delay_ticks: 41, // adopted from the Scout row (see above)
    accuracy: 100,
    projectile_speed: I16F16::ZERO,
    scatter: I16F16::ZERO,
    blast_radius: I16F16::ZERO,
    pass_through: false,
    blast_secondaries: 0,
    speed: I16F16::from_bits(5_898), // 1.35 tiles/s (absolute scale, see header)
    los: I16F16::from_bits(1_048_576), // 16 (4 tiles)
    radius: I16F16::from_bits(65_536),
    carcass_food: 0,
    carcass_decay_bits: 0,
    food_cost: 60,
    wood_cost: 0,
    gold_cost: 75,
    train_ticks: 1800, // 30 s
    regicide_unit: false,
    immune_conversion: false,
    regenerates: 0,
};

/// Light Cavalry (Castle P2; wiki "Light Cavalry (Age of Empires
/// II)" infobox, fetched 2026-10-22): HP 60, attack 7 melee with
/// +10 vs Monks, armor 0/2, RoF 2.0 s, speed 1.5, LoS 8 — the
/// scout line's Castle-Age sight (the whole line gains +2 LoS per
/// Age advance; the Scout's Castle step is patched in
/// `Sim::stats_for` beside CLS-04). The row's cost fields carry
/// the documented train cost (80 F, 30 s) but are inert: Light
/// Cavalry is never trained directly — it is produced by the
/// Stable upgrade (TechId::LightCavalry), and post-upgrade Scout
/// production spawns this row instead. Attack delay adopts the
/// Scout row's 41 ticks, like the Knight's. The documented
/// conversion resistance has no system to attach to yet (no
/// conversion exists in the sim — Monastery slice, Castle P5)
/// and is recorded in the rulebook, not faked here.
static LIGHT_CAVALRY: UnitSpec = UnitSpec {
    max_hp: 60,
    attacks: &[(cls::MELEE, 7), (cls::MONK, 10)],
    armor_melee: 0,
    armor_pierce: 2,
    armor_building: 0,
    classes: cls::mask(&[cls::CAVALRY]),
    range_max: I16F16::ZERO,
    range_min: I16F16::ZERO,
    reload_ticks: 120, // 2.0 s
    attack_delay_ticks: 41, // adopted from the Scout row (see above)
    accuracy: 100,
    projectile_speed: I16F16::ZERO,
    scatter: I16F16::ZERO,
    blast_radius: I16F16::ZERO,
    pass_through: false,
    blast_secondaries: 0,
    speed: I16F16::from_bits(6_554), // 1.5 tiles/s (absolute scale, see header)
    los: I16F16::from_bits(2_097_152), // 32 (8 tiles, Castle Age)
    radius: I16F16::from_bits(65_536),
    carcass_food: 0,
    carcass_decay_bits: 0,
    food_cost: 80,
    wood_cost: 0,
    gold_cost: 0,
    train_ticks: 1800, // inert (upgrade-only kind); mirrors Scout Cavalry
    regicide_unit: false,
    immune_conversion: false,
    regenerates: 0,
};

/// Cavalry Archer (Castle P2; wiki "Cavalry Archer (Age of
/// Empires II)" infobox, fetched 2026-10-22): HP 50, attack
/// 6 pierce with +2 vs Spearmen, armor 0/0, range 4 (min 0),
/// RoF 2.0 s, accuracy 50%, speed 1.4, LoS 5. Cost 40 W / 60 G,
/// train 37 s, Archery Range, Castle Age. Membership is ARCHERY
/// + CAVALRY — the wiki: "in terms of attack bonus against them
/// they count as Cavalry Archers, Archers, and Cavalry" (the
/// Cavalry-Archer membership class itself has no consumer yet).
/// Attack delay is a recorded design constant: the wiki article
/// calls the unit "hindered somewhat by their high attack delay"
/// without pinning a number in its text, so 30 ticks (0.5 s —
/// above the foot Archer's 21) stands in, marked UNVERIFIED.
/// Projectile speed and scatter reuse the Archer rows' recorded
/// design constants (PRJ-02/PRJ-04).
static CAVALRY_ARCHER: UnitSpec = UnitSpec {
    max_hp: 50,
    attacks: &[(cls::PIERCE, 6), (cls::SPEARMAN, 2)],
    armor_melee: 0,
    armor_pierce: 0,
    armor_building: 0,
    classes: cls::mask(&[cls::ARCHERY, cls::CAVALRY]),
    range_max: I16F16::from_bits(1_048_576), // 16 (4 tiles)
    range_min: I16F16::ZERO,
    reload_ticks: 120, // 2.0 s
    attack_delay_ticks: 30, // UNVERIFIED design constant (see above)
    accuracy: 50,
    projectile_speed: I16F16::from_bits(32_768), // 0.5 — design constant (see Archer)
    scatter: I16F16::from_bits(131_072),          // 2.0 — design constant (see Archer)
    blast_radius: I16F16::ZERO,
    pass_through: false,
    blast_secondaries: 0,
    speed: I16F16::from_bits(6_117),             // 1.4 tiles/s (absolute scale, see header)
    los: I16F16::from_bits(1_310_720),            // 20 (5 tiles)
    radius: I16F16::from_bits(65_536),
    carcass_food: 0,
    carcass_decay_bits: 0,
    food_cost: 0,
    wood_cost: 40,
    gold_cost: 60,
    train_ticks: 2220, // 37 s
    regicide_unit: false,
    immune_conversion: false,
    regenerates: 0,
};

/// The Petard's blast radius (Castle P3a, CMB-17): 0.5 tiles —
/// a landed strike detonates at the target and every enemy unit
/// or building within this radius of the detonation point takes
/// the strike's attack table too; the Petard is removed. (The
/// reference's blast falloff tiers are PRJ-07's siege package;
/// CMB-17 ships the single documented radius.)
pub const PETARD_BLAST_RADIUS: I16F16 = I16F16::from_bits(131_072); // 2.0 = 0.5 tiles

/// Petard (Castle P3a, CMB-17; wiki "Petard (Age of Empires II)"
/// infobox via the Castle Age dossier, fetched 2026-10-22):
/// HP 50, attack 25 melee with +900 vs Walls & Gates, +500 vs
/// Buildings (standard), +100 vs Castles, +60 vs Siege (inert —
/// no siege kind exists until Castle P4), armor 0/2, speed 0.8,
/// LoS 4. Cost 65 F / 20 G, train 25 s, Castle only
/// (`BuildingKind::trains` + `EntityKind::train_age`). Suicide
/// unit: a landed melee strike detonates it — see
/// `combat::Sim::petard_detonate`. The attack delay and reload
/// adopt the foot-melee rows' 30 ticks / 2.0 s as recorded
/// design constants (the strike is single by construction, so
/// the reload never governs a second shot).
static PETARD: UnitSpec = UnitSpec {
    max_hp: 50,
    attacks: &[
        (cls::MELEE, 25),
        (cls::WALLS_AND_GATES, 900),
        (cls::STANDARD_BUILDINGS, 500),
        (cls::CASTLE, 100),
        (cls::SIEGE, 60),
    ],
    armor_melee: 0,
    armor_pierce: 2,
    armor_building: 0,
    classes: cls::mask(&[cls::INFANTRY]),
    range_max: I16F16::ZERO,
    range_min: I16F16::ZERO,
    reload_ticks: 120, // 2.0 s — adopted (see above)
    attack_delay_ticks: 30, // adopted foot-melee value (see above)
    accuracy: 100,
    projectile_speed: I16F16::ZERO,
    scatter: I16F16::ZERO,
    blast_radius: I16F16::ZERO,
    pass_through: false,
    blast_secondaries: 0,
    speed: I16F16::from_bits(3_495), // 0.8 tiles/s (absolute scale, see header)
    los: I16F16::from_bits(1_048_576), // 16 (4 tiles)
    radius: I16F16::from_bits(65_536),
    carcass_food: 0,
    carcass_decay_bits: 0,
    food_cost: 65,
    wood_cost: 0,
    gold_cost: 20,
    train_ticks: 1500, // 25 s
    regicide_unit: false,
    immune_conversion: false,
    regenerates: 0,
};

/// King (VIC-06; wiki, "King (Age of Empires II)", fetched
/// 2026-10-29): HP 75, no attack, armor 0/0, speed 1.32 tiles/s,
/// Line of Sight 6 tiles, occupies 1 population slot (the
/// `counts_population` membership in `entity.rs`), cannot be
/// converted and does not regenerate (unlike heroes — the CLS-06
/// distinction). Class mask empty: no class membership is pinned
/// by the study, and no attack row in the game keys a bonus off a
/// King class. Costs are inert — the King is never trained; one
/// is spawned per player by the Regicide opening (see
/// `Sim::with_settings`).
static KING: UnitSpec = UnitSpec {
    max_hp: 75,
    attacks: NO_ATTACKS,
    armor_melee: 0,
    armor_pierce: 0,
    armor_building: 0,
    classes: cls::mask(&[]),
    range_max: I16F16::ZERO,
    range_min: I16F16::ZERO,
    reload_ticks: 0,
    attack_delay_ticks: 0,
    accuracy: 100,
    projectile_speed: I16F16::ZERO,
    scatter: I16F16::ZERO,
    blast_radius: I16F16::ZERO,
    pass_through: false,
    blast_secondaries: 0,
    speed: I16F16::from_bits(5_767), // 1.32 tiles/s (absolute scale, see header)
    los: I16F16::from_bits(1_572_864), // 24 (6 tiles)
    radius: I16F16::from_bits(65_536), // 1.0 (villager-like; unpinned)
    carcass_food: 0,
    carcass_decay_bits: 0,
    food_cost: 0,
    wood_cost: 0,
    gold_cost: 0,
    train_ticks: 0,
    regicide_unit: true,
    immune_conversion: true,
    regenerates: 0,
};

/// Fishing Ship (Water W2a, ECO-15; wiki, Ship list fresh crawl
/// 2026-11-05 via WATER-SPEC §3.1 — post-update-169123 values):
/// HP 50 (the pre-overhaul 60 is superseded, SPEC §8.3), armor
/// 1/1, NO attack, speed 1.26 tiles/s, LoS 5, cost 75 wood,
/// train 40 s at the Dock (Dark Age). Class: SHIP — the first
/// member (see the kind's §0.5 membership note in `entity.rs`).
/// Radius 1.6 world units (0.4 tiles): DE's collision size for
/// the ship is 0.4 (wiki, Fishing Ship changelog, update 81058);
/// read as this row's radius it is an implementation constant
/// in the plan's §8.9 sense (no DE source documents separation
/// behavior on water), recorded as such.
static FISHING_SHIP: UnitSpec = UnitSpec {
    max_hp: 50,
    attacks: NO_ATTACKS,
    armor_melee: 1,
    armor_pierce: 1,
    armor_building: 0,
    classes: cls::mask(&[cls::SHIP]),
    range_max: I16F16::ZERO,
    range_min: I16F16::ZERO,
    reload_ticks: 0,
    attack_delay_ticks: 0,
    accuracy: 100,
    projectile_speed: I16F16::ZERO,
    scatter: I16F16::ZERO,
    blast_radius: I16F16::ZERO,
    pass_through: false,
    blast_secondaries: 0,
    speed: I16F16::from_bits(5_505), // 1.26 tiles/s (absolute scale, see header)
    los: I16F16::from_bits(1_310_720), // 20 (5 tiles)
    radius: I16F16::from_bits(104_858), // 1.6 (0.4 tiles — see row docs)
    carcass_food: 0,
    carcass_decay_bits: 0,
    food_cost: 0,
    wood_cost: 75,
    gold_cost: 0,
    train_ticks: 2400, // 40 s
    regicide_unit: false,
    immune_conversion: false,
    regenerates: 0,
};

/// The Demolition Raft's blast radius (Water W3a, CLS-07): 2.5
/// tiles (wiki, Update 169123 article's Demolition Raft line via
/// WATER-SPEC §3.2) — the blast tapers radially (the Raft's
/// profile; the Castle-Age Demolition Ship's 3.0 at 100% across
/// is W3b's second profile on the same helper).
pub const DEMOLITION_RAFT_BLAST_RADIUS: I16F16 = I16F16::from_bits(655_360); // 10.0 = 2.5 tiles

/// The Demolition Ship's blast radius (Water W3b, CLS-07):
/// 3.0 tiles at **100% effect across the radius** — no taper,
/// where the Raft's profile decreases radially (wiki,
/// Demolition Ship article's blast-falloff note via WATER-SPEC
/// §3.3). The second blast profile on the shared helper.
pub const DEMOLITION_SHIP_BLAST_RADIUS: I16F16 = I16F16::from_bits(786_432); // 12.0 = 3.0 tiles

/// Galley (Water W3a, CLS-07; wiki, Ship list fresh crawl
/// 2026-11-05 via WATER-SPEC §3.2 — post-update-169123 values):
/// HP 110, armor 0/3, attack 6 pierce with +4 vs Long-Range
/// Warships (wiki, Update 169123 article's Galley line, fetched
/// 2026-11-06), +6 vs Standard Buildings and +3 vs Rams
/// (ADOPTED for the Feudal row from the War Galley's pinned
/// comparison row — wiki, Turtle Ship comparison table — via
/// update 169123's "Galley line: unchanged attack bonuses
/// against buildings and rams"), range 5, RoF 3.0 s, speed 1.36
/// tiles/s, LoS 7. Cost 90 W / 30 G, train 36 s at the Dock,
/// Feudal Age. Classes: SHIP + LONG_RANGE_WARSHIP (the Galley
/// line carries the class it also hunts — DE models both sides
/// of the one surviving bonus case). Accuracy 80, projectile
/// speed and scatter adopt the Archer row's recorded design
/// constants (PRJ-02/PRJ-04) — ADOPTED/UNVERIFIED for this row:
/// no fetched source pins the Galley's own figures. Attack
/// delay adopts the Archer's 21 ticks on the same basis.
/// Radius adopts the Fishing Ship's 1.6 — an implementation
/// constant (WATER-SPEC §8.9), never DE data.
static GALLEY: UnitSpec = UnitSpec {
    max_hp: 110,
    attacks: &[
        (cls::PIERCE, 6),
        (cls::LONG_RANGE_WARSHIP, 4),
        (cls::STANDARD_BUILDINGS, 6),
        (cls::RAM, 3),
    ],
    armor_melee: 0,
    armor_pierce: 3,
    armor_building: 0,
    classes: cls::mask(&[cls::SHIP, cls::LONG_RANGE_WARSHIP]),
    range_max: I16F16::from_bits(1_310_720), // 20 (5 tiles)
    range_min: I16F16::ZERO,
    reload_ticks: 180, // 3.0 s
    attack_delay_ticks: 21, // adopted (see above)
    accuracy: 80, // adopted (see above)
    projectile_speed: I16F16::from_bits(32_768), // 0.5 — design constant (see Archer)
    scatter: I16F16::from_bits(131_072),          // 2.0 — design constant (see Archer)
    blast_radius: I16F16::ZERO,
    pass_through: false,
    blast_secondaries: 0,
    speed: I16F16::from_bits(5_942), // 1.36 tiles/s (absolute scale, see header)
    los: I16F16::from_bits(1_835_008), // 28 (7 tiles)
    radius: I16F16::from_bits(104_858), // 1.6 — adopted (see above)
    carcass_food: 0,
    carcass_decay_bits: 0,
    food_cost: 0,
    wood_cost: 90,
    gold_cost: 30,
    train_ticks: 2160, // 36 s
    regicide_unit: false,
    immune_conversion: false,
    regenerates: 0,
};

/// Fire Galley (Water W3a, CLS-07; wiki, Ship list fresh crawl
/// 2026-11-05 via WATER-SPEC §3.2 + the Fire Galley article's
/// update-169123 changelog, fetched 2026-11-06): HP 110, armor
/// 1/4, attack **2 melee** (the overhaul "changed base attack to
/// 2 melee and pierce attack removed") with +1 vs Long-Range
/// Warships (wiki, Update 169123 article's Fire Galley line) and
/// NO bonus vs buildings (the article's update-185872 line:
/// "Fire Galley have an attack bonus of 0 vs. buildings"),
/// range 2.5, RoF 0.25 s, speed 1.36 tiles/s (the SPEC's fresh
/// Ship-list value governs; the Update 169123 article's 1.43
/// conflicts and is recorded here, not averaged), LoS 5. Cost
/// 75 W / 45 G, train 49 s at the Dock, Feudal Age. Classes:
/// SHIP + FIRE_SHIP. **Spray-as-data (Passo 0 pin):** the
/// documented weapon is ordinary fast melee strikes — 2 melee
/// every 0.25 s inside range 2.5 — so the row is pure data on
/// the standing release: range 2.5 rides the ranged band with
/// the generic projectile constants and accuracy 100 (a spray
/// at this range does not scatter-roll in any fetched source);
/// no continuous-damage model is documented, so none is built.
/// Attack delay 15 ticks (one RoF cycle) is ADOPTED/UNVERIFIED.
/// Radius adopts the Fishing Ship's 1.6 (implementation
/// constant, WATER-SPEC §8.9).
static FIRE_GALLEY: UnitSpec = UnitSpec {
    max_hp: 110,
    attacks: &[(cls::MELEE, 2), (cls::LONG_RANGE_WARSHIP, 1)],
    armor_melee: 1,
    armor_pierce: 4,
    armor_building: 0,
    classes: cls::mask(&[cls::SHIP, cls::FIRE_SHIP]),
    range_max: I16F16::from_bits(655_360), // 10 (2.5 tiles)
    range_min: I16F16::ZERO,
    reload_ticks: 15, // 0.25 s
    attack_delay_ticks: 15, // adopted (see above)
    accuracy: 100,
    projectile_speed: I16F16::from_bits(32_768), // 0.5 — design constant (see Archer)
    scatter: I16F16::ZERO,
    blast_radius: I16F16::ZERO,
    pass_through: false,
    blast_secondaries: 0,
    speed: I16F16::from_bits(5_942), // 1.36 tiles/s (see above)
    los: I16F16::from_bits(1_310_720), // 20 (5 tiles)
    radius: I16F16::from_bits(104_858), // 1.6 — adopted (see above)
    carcass_food: 0,
    carcass_decay_bits: 0,
    food_cost: 0,
    wood_cost: 75,
    gold_cost: 45,
    train_ticks: 2940, // 49 s
    regicide_unit: false,
    immune_conversion: false,
    regenerates: 0,
};

/// Demolition Raft (Water W3a, CLS-07; wiki, Update 169123
/// article's Demolition Raft line via WATER-SPEC §3.2 — the
/// fresh Ship list's own row was internally inconsistent, SPEC
/// §8.10): HP 40, armor 0/0, attack 65 melee delivered as its
/// **blast** (radius 2.5 tiles, radial decrease, NO friendly
/// fire — the wiki's Demolition tactics; see
/// `combat::Sim::demolition_detonate`), contact range, speed
/// 1.42 tiles/s, LoS 6. Cost 70 W / 80 G, train 45 s at the
/// Dock, Feudal Age. Class: SHIP. The row's `attacks` table IS
/// the blast table; the strike machinery never applies it
/// singly — a landed strike (or the raft's death by enemy fire)
/// detonates instead, exactly the Petard's split with the
/// blast profile inverted (tapered, enemies-only). Reload and
/// attack delay adopt the Petard's recorded constants (the
/// strike is single by construction). Radius adopts the
/// Fishing Ship's 1.6 (implementation constant, §8.9).
static DEMOLITION_RAFT: UnitSpec = UnitSpec {
    max_hp: 40,
    attacks: &[(cls::MELEE, 65)],
    armor_melee: 0,
    armor_pierce: 0,
    armor_building: 0,
    classes: cls::mask(&[cls::SHIP]),
    range_max: I16F16::ZERO,
    range_min: I16F16::ZERO,
    reload_ticks: 120, // adopted (see above)
    attack_delay_ticks: 30, // adopted foot-melee value (see above)
    accuracy: 100,
    projectile_speed: I16F16::ZERO,
    scatter: I16F16::ZERO,
    blast_radius: I16F16::ZERO,
    pass_through: false,
    blast_secondaries: 0,
    speed: I16F16::from_bits(6_204), // 1.42 tiles/s (absolute scale, see header)
    los: I16F16::from_bits(1_572_864), // 24 (6 tiles)
    radius: I16F16::from_bits(104_858), // 1.6 — adopted (see above)
    carcass_food: 0,
    carcass_decay_bits: 0,
    food_cost: 0,
    wood_cost: 70,
    gold_cost: 80,
    train_ticks: 2700, // 45 s
    regicide_unit: false,
    immune_conversion: false,
    regenerates: 0,
};

/// Hulk (Water W3a, CLS-07; wiki, Ship list fresh crawl
/// 2026-11-05 via WATER-SPEC §3.2, corroborated by the Hulk
/// article's update-185872 changelog — HP 90 → 85, melee armor
/// 4 → 3 — fetched 2026-11-06): HP 85, armor 3/1, attack 4 melee
/// with +1 vs Fire Ships (wiki, "Armor class: Fire ship" —
/// the anti-fire-ship row is +1 and the class exists for the
/// Hulk line), range 1 (the melee reach constant governs, as
/// for every melee row), RoF 1.75 s, speed 1.42 tiles/s, LoS 5.
/// Cost 75 W / 35 G, train 42 s at the Dock, Feudal Age. Class:
/// SHIP. It takes the Forging line's patches (its article's
/// technology table) and no archer patch. Attack delay adopts
/// the foot-melee rows' 30 ticks (no fetched source pins the
/// Hulk's own frame value) — ADOPTED. Radius adopts the
/// Fishing Ship's 1.6 (implementation constant, §8.9).
static HULK: UnitSpec = UnitSpec {
    max_hp: 85,
    attacks: &[(cls::MELEE, 4), (cls::FIRE_SHIP, 1)],
    armor_melee: 3,
    armor_pierce: 1,
    armor_building: 0,
    classes: cls::mask(&[cls::SHIP]),
    range_max: I16F16::ZERO,
    range_min: I16F16::ZERO,
    reload_ticks: 105, // 1.75 s
    attack_delay_ticks: 30, // adopted (see above)
    accuracy: 100,
    projectile_speed: I16F16::ZERO,
    scatter: I16F16::ZERO,
    blast_radius: I16F16::ZERO,
    pass_through: false,
    blast_secondaries: 0,
    speed: I16F16::from_bits(6_204), // 1.42 tiles/s (absolute scale, see header)
    los: I16F16::from_bits(1_310_720), // 20 (5 tiles)
    radius: I16F16::from_bits(104_858), // 1.6 — adopted (see above)
    carcass_food: 0,
    carcass_decay_bits: 0,
    food_cost: 0,
    wood_cost: 75,
    gold_cost: 35,
    train_ticks: 2520, // 42 s
    regicide_unit: false,
    immune_conversion: false,
    regenerates: 0,
};

/// War Galley (Water W3b, CLS-07; wiki, Ship list fresh crawl
/// 2026-11-06 + WATER-SPEC §3.3): HP 125, armor 0/4, attack
/// 7 pierce with +5 vs Long-Range Warships (the Galley's +4
/// plus the War Galley article's "+1 anti-ship attack" delta —
/// in the post-169123 ecology the anti-ship case IS the
/// long-range class; ADOPTED, no source pins the Castle tier's
/// own bonus row directly), +6 vs Standard Buildings / +3 vs
/// Rams (pinned directly for this tier by the Turtle Ship
/// comparison row the W3a handoff cites; update 169123 left the
/// line's building/ram bonuses unchanged), range 6, RoF 3.0 s,
/// LoS 8, train 27 s. Speed 1.36 tiles/s: the Ship list's speed
/// cell is blank and the article's delta list over the Galley
/// (+15 HP, +1 attack, +1 anti-ship, +1 range, −train) names
/// no speed change — the Galley's 1.36 carries over (ADOPTED).
/// Cost 90 W / 30 G — the line's cost does not change at the
/// upgrade in DE's generic roster (the Dock trains this tier
/// at the Galley's price once Medium Warships lands; in this
/// sim the queue entry stays Galley and the spawn converts,
/// the CLS-03 pattern, so the row's cost is the tier's data
/// record). Classes: SHIP + LONG_RANGE_WARSHIP. Accuracy,
/// projectile constants, attack delay and radius adopt the
/// Galley's recorded constants on the same basis as W3a's.
static WAR_GALLEY: UnitSpec = UnitSpec {
    max_hp: 125,
    attacks: &[
        (cls::PIERCE, 7),
        (cls::LONG_RANGE_WARSHIP, 5),
        (cls::STANDARD_BUILDINGS, 6),
        (cls::RAM, 3),
    ],
    armor_melee: 0,
    armor_pierce: 4,
    armor_building: 0,
    classes: cls::mask(&[cls::SHIP, cls::LONG_RANGE_WARSHIP]),
    range_max: I16F16::from_bits(1_572_864), // 24 (6 tiles)
    range_min: I16F16::ZERO,
    reload_ticks: 180, // 3.0 s
    attack_delay_ticks: 21, // adopted (the Galley's)
    accuracy: 80, // adopted (the Galley's)
    projectile_speed: I16F16::from_bits(32_768), // 0.5 — design constant (see Archer)
    scatter: I16F16::from_bits(131_072),          // 2.0 — design constant (see Archer)
    blast_radius: I16F16::ZERO,
    pass_through: false,
    blast_secondaries: 0,
    speed: I16F16::from_bits(5_942), // 1.36 tiles/s (adopted — see above)
    los: I16F16::from_bits(2_097_152), // 32 (8 tiles)
    radius: I16F16::from_bits(104_858), // 1.6 — adopted (the Galley's)
    carcass_food: 0,
    carcass_decay_bits: 0,
    food_cost: 0,
    wood_cost: 90,
    gold_cost: 30,
    train_ticks: 1620, // 27 s
    regicide_unit: false,
    immune_conversion: false,
    regenerates: 0,
};

/// Fire Ship (Water W3b, CLS-07; wiki, Ship list fresh crawl
/// 2026-11-06 + WATER-SPEC §3.3): HP 120, armor 2/4, attack
/// 3 melee with +2 vs Long-Range Warships, range 2.5, RoF
/// 0.25 s (spray-as-data, exactly the Fire Galley's model),
/// speed 1.47 tiles/s, LoS 5, train 27 s. Cost 75 W / 45 G
/// (the line's cost, as the War Galley's note). The pierce
/// armor cell is blank in the fresh Ship list: 4 is ADOPTED
/// from the update-169123 preview data (Ornlu's showcased
/// numbers: "Fire Ship: 120 HP, 3 melee attack, 2 melee armor,
/// 4 pierce armor") and the Fire Galley's own 4 — flagged
/// ADOPTED/UNVERIFIED. Range and RoF are likewise blank in
/// the list and carry the Fire Galley's documented 2.5 /
/// 0.25 s (the Fire Ship article's trivia: the Fire line is
/// "the only units in the game without an integer range").
/// The +2 vs Long-Range Warships steps the Fire Galley's +1
/// up one rung, the Galley line's own step (4 → 5) — ADOPTED
/// on line continuity, no fetched source pins it. Classes:
/// SHIP + FIRE_SHIP. Attack delay and radius adopt the Fire
/// Galley's recorded constants.
static FIRE_SHIP: UnitSpec = UnitSpec {
    max_hp: 120,
    attacks: &[(cls::MELEE, 3), (cls::LONG_RANGE_WARSHIP, 2)],
    armor_melee: 2,
    armor_pierce: 4,
    armor_building: 0,
    classes: cls::mask(&[cls::SHIP, cls::FIRE_SHIP]),
    range_max: I16F16::from_bits(655_360), // 10 (2.5 tiles)
    range_min: I16F16::ZERO,
    reload_ticks: 15, // 0.25 s
    attack_delay_ticks: 15, // adopted (the Fire Galley's)
    accuracy: 100,
    projectile_speed: I16F16::from_bits(32_768), // 0.5 — design constant (see Archer)
    scatter: I16F16::ZERO,
    blast_radius: I16F16::ZERO,
    pass_through: false,
    blast_secondaries: 0,
    speed: I16F16::from_bits(6_423), // 1.47 tiles/s
    los: I16F16::from_bits(1_310_720), // 20 (5 tiles)
    radius: I16F16::from_bits(104_858), // 1.6 — adopted (the Fire Galley's)
    carcass_food: 0,
    carcass_decay_bits: 0,
    food_cost: 0,
    wood_cost: 75,
    gold_cost: 45,
    train_ticks: 1620, // 27 s
    regicide_unit: false,
    immune_conversion: false,
    regenerates: 0,
};

/// Demolition Ship (Water W3b, CLS-07; wiki, Update 169123
/// article + Demolition Ship article via WATER-SPEC §3.3):
/// HP 50, armor 1/0, attack **80 melee delivered as its
/// blast** — radius 3.0 tiles at 100% across the radius (no
/// taper; `DEMOLITION_SHIP_BLAST_RADIUS`), enemies only,
/// contact range, speed 1.52 tiles/s, LoS 6. Cost 70 W / 80 G
/// (the line's cost, as the War Galley's note), train 31 s.
/// Class: SHIP. CONFLICT RECORDED, NOT AVERAGED: the wiki
/// Ship list as re-crawled 2026-11-06 shows 95 melee for this
/// row (and 75 for the Raft, 120 for the Heavy) — the SPEC's
/// 80, sourced from the Update 169123 article and the
/// Demolition Ship article, governs per the dossier's source
/// order; the discrepancy is flagged in the W3b handoff for
/// the loop. The row's `attacks` table IS the blast table,
/// exactly the Raft's split; reload and attack delay adopt
/// the Raft's (the Petard's) recorded constants; radius adopts
/// the Fishing Ship's 1.6 (implementation constant, §8.9).
static DEMOLITION_SHIP: UnitSpec = UnitSpec {
    max_hp: 50,
    attacks: &[(cls::MELEE, 80)],
    armor_melee: 1,
    armor_pierce: 0,
    armor_building: 0,
    classes: cls::mask(&[cls::SHIP]),
    range_max: I16F16::ZERO,
    range_min: I16F16::ZERO,
    reload_ticks: 120, // adopted (the Raft's)
    attack_delay_ticks: 30, // adopted (the Raft's)
    accuracy: 100,
    projectile_speed: I16F16::ZERO,
    scatter: I16F16::ZERO,
    blast_radius: I16F16::ZERO,
    pass_through: false,
    blast_secondaries: 0,
    speed: I16F16::from_bits(6_641), // 1.52 tiles/s
    los: I16F16::from_bits(1_572_864), // 24 (6 tiles)
    radius: I16F16::from_bits(104_858), // 1.6 — adopted (see above)
    carcass_food: 0,
    carcass_decay_bits: 0,
    food_cost: 0,
    wood_cost: 70,
    gold_cost: 80,
    train_ticks: 1860, // 31 s
    regicide_unit: false,
    immune_conversion: false,
    regenerates: 0,
};

/// War Hulk (Water W3b, CLS-07; wiki, Ship list fresh crawl
/// 2026-11-06 + the War Hulk article's update-185872
/// changelog): HP 110, armor 4/1, attack 4 melee with +1 vs
/// Fire Ships, range 1 (the melee reach constant), RoF 1.5 s,
/// speed 1.42 tiles/s, LoS 5, train 27 s. Cost 75 W / 35 G
/// (the line's cost, as the War Galley's note). The article's
/// changelog pins the 185872 step directly (HP 115 → 110,
/// melee armor 5 → 4 — the same update that took the Hulk
/// 90 → 85 / 4 → 3 in W3a). The fresh list's attack, pierce
/// armor, LoS and speed cells are blank for this row: attack
/// 4 melee is the Hulk's own (no source names an attack change
/// at this tier; the tier's documented gain is the faster
/// 1.5 s reload the list DOES pin), pierce armor 1 and LoS 5
/// and speed 1.42 carry the Hulk's — ADOPTED on line
/// continuity, flagged. The +1 vs the FIRE_SHIP class carries
/// unchanged (the class's purpose is the line's, both tiers).
/// Class: SHIP. Attack delay and radius adopt the Hulk's.
static WAR_HULK: UnitSpec = UnitSpec {
    max_hp: 110,
    attacks: &[(cls::MELEE, 4), (cls::FIRE_SHIP, 1)],
    armor_melee: 4,
    armor_pierce: 1,
    armor_building: 0,
    classes: cls::mask(&[cls::SHIP]),
    range_max: I16F16::ZERO,
    range_min: I16F16::ZERO,
    reload_ticks: 90, // 1.5 s
    attack_delay_ticks: 30, // adopted (the Hulk's)
    accuracy: 100,
    projectile_speed: I16F16::ZERO,
    scatter: I16F16::ZERO,
    blast_radius: I16F16::ZERO,
    pass_through: false,
    blast_secondaries: 0,
    speed: I16F16::from_bits(6_204), // 1.42 tiles/s (adopted — see above)
    los: I16F16::from_bits(1_310_720), // 20 (5 tiles)
    radius: I16F16::from_bits(104_858), // 1.6 — adopted (the Hulk's)
    carcass_food: 0,
    carcass_decay_bits: 0,
    food_cost: 0,
    wood_cost: 75,
    gold_cost: 35,
    train_ticks: 1620, // 27 s
    regicide_unit: false,
    immune_conversion: false,
    regenerates: 0,
};

/// Transport Ship (Water W4, GAR-09; wiki, Ship list fresh crawl
/// 2026-11-05 via WATER-SPEC §3.1 — post-update-169123 values):
/// HP 70 (the pre-overhaul 100 is superseded, SPEC §8.3), armor
/// 0/2 (the pre-overhaul 4/8 likewise), NO attack, speed 1.45
/// tiles/s, LoS 5, cost 125 wood, train 46 s at the Dock (Dark
/// Age). Capacity 5 land units (WATER-SPEC §6: the wiki
/// Transport Ship changelog's "garrison capacity of 5"; the
/// GAP "5 → 20" progression is Chronicles-mode content, NOT
/// standard DE — SPEC §8.6). Class: SHIP, like every ship but
/// with no weapon. Radius adopts the Fishing Ship's 1.6 — an
/// implementation constant (WATER-SPEC §8.9), never DE data.
static TRANSPORT_SHIP: UnitSpec = UnitSpec {
    max_hp: 70,
    attacks: NO_ATTACKS,
    armor_melee: 0,
    armor_pierce: 2,
    armor_building: 0,
    classes: cls::mask(&[cls::SHIP]),
    range_max: I16F16::ZERO,
    range_min: I16F16::ZERO,
    reload_ticks: 0,
    attack_delay_ticks: 0,
    accuracy: 100,
    projectile_speed: I16F16::ZERO,
    scatter: I16F16::ZERO,
    blast_radius: I16F16::ZERO,
    pass_through: false,
    blast_secondaries: 0,
    speed: I16F16::from_bits(6_335), // 1.45 tiles/s (absolute scale, see header)
    los: I16F16::from_bits(1_310_720), // 20 (5 tiles)
    radius: I16F16::from_bits(104_858), // 1.6 — adopted (see above)
    carcass_food: 0,
    carcass_decay_bits: 0,
    food_cost: 0,
    wood_cost: 125,
    gold_cost: 0,
    train_ticks: 2760, // 46 s
    regicide_unit: false,
    immune_conversion: false,
    regenerates: 0,
};

/// Trade Cog (Water W5, TRD-05/07/08; wiki, Ship list fresh
/// crawl 2026-11-05 via WATER-SPEC §3.2 — post-update-169123
/// values, corroborated at W5 Passo 0 against the live Ship
/// list and the Trade Cog article, fetched 2026-11-06): HP 80,
/// armor 0/6, no attack, speed 1.65 tiles/s (the Trade Cog
/// article's changelog: update 141935 raised the base speed to
/// 1.65 and cut Caravan to +20% in the same patch — the cart's
/// own history, one domain over), LoS 6. Cost 100 W / 50 G,
/// train 36 s at the Dock, Feudal Age. Class: SHIP (the Ship
/// article's overview: all ships except the Fishing Ship carry
/// the Ships class — and the Fishing Ship carries it too in
/// this roster, per the W2a §0.5 decision). Radius adopts the
/// Fishing Ship's 1.6 — an implementation constant
/// (WATER-SPEC §8.9), never DE data. NOT shipped: the update-
/// 169123 preview's gold/wood cargo-ratio configuration on the
/// Cog's UI (ageofempires.com naval preview, 2026-01-28) — no
/// fetched source pins its ratios or its TRD-06 interaction,
/// so the Cog credits gold by the shipped formula exactly
/// like the Cart; the ratio options are recorded as an open
/// item in the W5 handoff, not invented here.
static TRADE_COG: UnitSpec = UnitSpec {
    max_hp: 80,
    attacks: NO_ATTACKS,
    armor_melee: 0,
    armor_pierce: 6,
    armor_building: 0,
    classes: cls::mask(&[cls::SHIP]),
    range_max: I16F16::ZERO,
    range_min: I16F16::ZERO,
    reload_ticks: 0,
    attack_delay_ticks: 0,
    accuracy: 100,
    projectile_speed: I16F16::ZERO,
    scatter: I16F16::ZERO,
    blast_radius: I16F16::ZERO,
    pass_through: false,
    blast_secondaries: 0,
    speed: I16F16::from_bits(7_209), // 1.65 tiles/s (absolute scale, see header)
    los: I16F16::from_bits(1_572_864), // 24 (6 tiles)
    radius: I16F16::from_bits(104_858), // 1.6 — adopted (see above)
    carcass_food: 0,
    carcass_decay_bits: 0,
    food_cost: 0,
    wood_cost: 100,
    gold_cost: 50,
    train_ticks: 2160, // 36 s
    regicide_unit: false,
    immune_conversion: false,
    regenerates: 0,
};

/// Trade Cart (Package D, TRD-05..08; wiki, "Trade Cart (Age of
/// Empires II)", fetched 2026-10-29, cross-checked against the
/// Aoe2Sheet unit overview): HP 70, armor 0/0, no attack, LoS 7,
/// cost 100 W / 50 G, train 51 s at the Market, Feudal Age
/// (`BuildingKind::trains` + `EntityKind::train_age`). Speed is
/// the CURRENT Definitive Edition row: update 141935 raised the
/// base speed 1 → 1.25 tiles/s and cut Caravan to +20% in the
/// same patch, so a Caravan cart (1.5) matches the pre-patch
/// upgraded speed exactly (the wiki's changelog note). Radius
/// 0.25 tiles (the trade-formula study's cart radius; update
/// 125283 set the collision multiplier to 0.25). The row carries
/// no armor-class membership: nothing in the current roster's
/// attack tables keys on a trade class (CLS-01 data discipline —
/// a membership ships when a documented bonus row needs it).
static TRADE_CART: UnitSpec = UnitSpec {
    max_hp: 70,
    attacks: NO_ATTACKS,
    armor_melee: 0,
    armor_pierce: 0,
    armor_building: 0,
    classes: 0,
    range_max: I16F16::ZERO,
    range_min: I16F16::ZERO,
    reload_ticks: 0,
    attack_delay_ticks: 0,
    accuracy: 100,
    projectile_speed: I16F16::ZERO,
    scatter: I16F16::ZERO,
    blast_radius: I16F16::ZERO,
    pass_through: false,
    blast_secondaries: 0,
    speed: I16F16::from_bits(5_461), // 1.25 tiles/s (update 141935; absolute scale, see header)
    los: I16F16::from_bits(1_835_008), // 28 (7 tiles)
    radius: I16F16::from_bits(65_536), // 1.0 (0.25 tiles)
    carcass_food: 0,
    carcass_decay_bits: 0,
    food_cost: 0,
    wood_cost: 100,
    gold_cost: 50,
    train_ticks: 3060, // 51 s
    regicide_unit: false,
    immune_conversion: false,
    regenerates: 0,
};

/// Battering Ram (Castle P4a, GAR-10; wiki "Battering Ram (Age
/// of Empires II)" infobox via the Castle Age dossier, fetched
/// 2026-10-22): HP 175, attack 2 melee with +150 vs Buildings
/// (standard) and +40 vs Siege, armor **−3 / 180** — the melee
/// armor is NEGATIVE in the reference: CMB-01's formula
/// (Σ max(0, atk − armor)) amplifies melee hits against it
/// (a 10-melee strike lands 13), and the pierce armor makes
/// arrows a minimum-damage nuisance only. RoF 5.0 s, speed 0.6
/// tiles/s (the EMPTY speed — each garrisoned passenger adds
/// +0.05 to a 0.9 cap, derived at lookup in `garrison.rs`),
/// LoS 5. Cost 160 W / 75 G, train 36 s, Siege Workshop only
/// (`BuildingKind::trains` + `EntityKind::train_age`). Its
/// classes carry SIEGE — the membership the Petard's +60 row
/// (Castle P3a) waited for — and, since Castle P4c, RAM: the
/// distinct Ram armor class of MECHANICS-STUDY §1.3, carried
/// so the Scorpion's +1 vs Rams row has its documented target
/// (the P4 plan's Passo 0.3). The attack delay adopts the
/// foot-melee rows' 30 ticks as a recorded design constant
/// (the dossier pins no separate ram frame value). The radius
/// adopts the largest shipped row's 1.2 (the Boar's bulk) as an
/// implementation constant — the dossier gives no ram radius —
/// never presented as DE data.
static BATTERING_RAM: UnitSpec = UnitSpec {
    max_hp: 175,
    attacks: &[
        (cls::MELEE, 2),
        (cls::STANDARD_BUILDINGS, 150),
        (cls::SIEGE, 40),
    ],
    armor_melee: -3,
    armor_pierce: 180,
    armor_building: 0,
    classes: cls::mask(&[cls::SIEGE, cls::RAM]),
    range_max: I16F16::ZERO,
    range_min: I16F16::ZERO,
    reload_ticks: 300,       // 5.0 s
    attack_delay_ticks: 30,  // adopted foot-melee value (see above)
    accuracy: 100,
    projectile_speed: I16F16::ZERO,
    scatter: I16F16::ZERO,
    blast_radius: I16F16::ZERO,
    pass_through: false,
    blast_secondaries: 0,
    speed: I16F16::from_bits(2_621), // 0.6 tiles/s, empty (see above)
    los: I16F16::from_bits(1_310_720), // 20 (5 tiles)
    radius: I16F16::from_bits(78_643), // 1.2 — adopted (see above)
    carcass_food: 0,
    carcass_decay_bits: 0,
    food_cost: 0,
    wood_cost: 160,
    gold_cost: 75,
    train_ticks: 2160, // 36 s
    regicide_unit: false,
    immune_conversion: false,
    regenerates: 0,
};

/// Mangonel (Castle P4b, PRJ-07 + CMB-16; wiki "Mangonel (Age
/// of Empires II)" infobox via the Castle Age dossier, fetched
/// 2026-10-22; secondary projectiles confirmed against the live
/// article 2026-10-30 — update 153015's changelog names them):
/// HP 50, attack 40 MELEE class with +35 vs Buildings (standard)
/// and +12 vs Siege, armor 0/6, range 7 with a 3-tile minimum
/// (PRJ-08's dead zone), blast radius 1.0 tile — the stone's
/// damage tapers linearly from 100% at the impact center
/// (PRJ-07; the engine's "delete trick" full-damage-to-half-
/// radius is a reference bug and is NOT reproduced), accuracy
/// 100% rolled PER UNIT inside the blast, plus 5 secondary
/// projectiles at minimum damage released with the main stone.
/// Projectile speed 3.5 tiles/s (the one per-weapon speed the
/// docs pin — see the `projectile_speed` field note), RoF 6.0 s,
/// attack delay 0 (the dossier's row: the first stone leaves
/// with the engagement), speed 0.6 tiles/s, LoS 9. Cost 160 W /
/// 135 G, train 46 s, Siege Workshop only (`BuildingKind::trains`
/// + `EntityKind::train_age`). Its classes carry SIEGE, like the
/// Ram's. The collision radius adopts the Ram's 1.2 — an
/// implementation constant (§3.2.4 of the P4 plan), never
/// presented as DE data. The weapon is blast-flagged (CMB-12):
/// its stones hurt friend and foe alike, fell trees inside the
/// radius (MAP-04), and it is the game's first Attack Ground
/// weapon (CMB-16).
static MANGONEL: UnitSpec = UnitSpec {
    max_hp: 50,
    attacks: &[
        (cls::MELEE, 40),
        (cls::STANDARD_BUILDINGS, 35),
        (cls::SIEGE, 12),
    ],
    armor_melee: 0,
    armor_pierce: 6,
    armor_building: 0,
    classes: cls::mask(&[cls::SIEGE]),
    range_max: I16F16::from_bits(1_835_008), // 28 (7 tiles)
    range_min: I16F16::from_bits(786_432),   // 12 (3 tiles)
    reload_ticks: 360,                       // 6.0 s
    attack_delay_ticks: 0,
    accuracy: 100,
    projectile_speed: I16F16::from_bits(15_291), // 3.5 tiles/s (absolute scale, see header)
    scatter: I16F16::ZERO,                       // blast weapons do not scatter (PRJ-07)
    blast_radius: I16F16::from_bits(262_144),    // 4 (1 tile)
    pass_through: false,
    blast_secondaries: 5,
    speed: I16F16::from_bits(2_621), // 0.6 tiles/s (absolute scale, see header)
    los: I16F16::from_bits(2_359_296), // 36 (9 tiles)
    radius: I16F16::from_bits(78_643), // 1.2 — adopted (see above)
    carcass_food: 0,
    carcass_decay_bits: 0,
    food_cost: 0,
    wood_cost: 160,
    gold_cost: 135,
    train_ticks: 2760, // 46 s
    regicide_unit: false,
    immune_conversion: false,
    regenerates: 0,
};

/// Scorpion (Castle P4c, PRJ-09; wiki "Scorpion (Age of
/// Empires II)" infobox via the Castle Age dossier §4, fetched
/// 2026-10-22, article re-read 2026-11-03): HP 40, attack
/// 11 pierce with +7 vs Elephants, +3 vs Buildings (standard),
/// +1 vs Rams and +1 vs Infantry, armor 0/7, range 7 with a
/// 2-tile minimum (PRJ-08's dead zone), RoF 3.6 s, speed 0.65
/// tiles/s, LoS 9. Cost 75 W / 75 G, train 30 s, Siege
/// Workshop only (`BuildingKind::trains` +
/// `EntityKind::train_age`). Two documented quirks ride the
/// table itself: the (MELEE, 0) pair — "Scorpions deal 0
/// melee damage in addition to their pierce attack" (the
/// article's changelog, The Conquerors) — which against the
/// Ram's −3 melee armor lands 3 damage a bolt, the soft
/// anti-ram bite the article's trivia calls out; and the
/// +1 vs Rams keys on the distinct RAM armor class
/// (MECHANICS-STUDY §1.3), not SIEGE — the P4 plan's
/// Passo 0.3. Accuracy 100: no source in the corpus gives
/// the Scorpion a sub-100 figure (the article's only
/// accuracy technology is Ballistics, which since update
/// 125283 lets its bolts track moving targets — the generic
/// release already leads under Ballistics), so its misses
/// are dodges, never scatter; the bolt flies at the generic
/// 0.5 units/tick design constant like every unpinned
/// projectile (PRJ-02). Attack delay 13 ticks from the
/// article's 0.21 s figure (its comparison section: the
/// Scorpion HAS a significant attack delay, the Mangonel
/// none). The weapon is pass-through-flagged (PRJ-09): the
/// bolt damages every ENEMY along its line — the targeted
/// unit full, the rest half, friendly units nothing — out
/// to 3 tiles past its maximum range. Its classes carry
/// SIEGE, like its workshop kin. The collision radius adopts
/// the Ram/Mangonel 1.2 — an implementation constant (§3.2.4
/// of the P4 plan), never presented as DE data.
static SCORPION: UnitSpec = UnitSpec {
    max_hp: 40,
    attacks: &[
        (cls::PIERCE, 11),
        (cls::MELEE, 0),
        (cls::ELEPHANT, 7),
        (cls::STANDARD_BUILDINGS, 3),
        (cls::RAM, 1),
        (cls::INFANTRY, 1),
    ],
    armor_melee: 0,
    armor_pierce: 7,
    armor_building: 0,
    classes: cls::mask(&[cls::SIEGE]),
    range_max: I16F16::from_bits(1_835_008), // 28 (7 tiles)
    range_min: I16F16::from_bits(524_288),   // 8 (2 tiles — PRJ-08)
    reload_ticks: 216,                       // 3.6 s
    attack_delay_ticks: 13,                  // 0.21 s (see above)
    accuracy: 100,
    projectile_speed: I16F16::from_bits(32_768), // 0.5 — design constant (see Archer)
    scatter: I16F16::from_bits(131_072),         // 2.0 — design constant (see Archer)
    blast_radius: I16F16::ZERO,
    pass_through: true,
    blast_secondaries: 0,
    speed: I16F16::from_bits(2_840), // 0.65 tiles/s (absolute scale, see header)
    los: I16F16::from_bits(2_359_296), // 36 (9 tiles)
    radius: I16F16::from_bits(78_643), // 1.2 — adopted (see above)
    carcass_food: 0,
    carcass_decay_bits: 0,
    food_cost: 0,
    wood_cost: 75,
    gold_cost: 75,
    train_ticks: 1800, // 30 s
    regicide_unit: false,
    immune_conversion: false,
    regenerates: 0,
};

/// Monk (Castle P5a, MNK-01/10; wiki "Monk (Age of Empires
/// II)" infobox via the Castle Age dossier §5.6, fetched
/// 2026-10-22): HP 30 (45 with Sanctity — the +15 patches in
/// `Sim::stats_for`), armor 0/0, **no attack table at all**
/// (CLS-08: the empty table IS the rule — `can_attack` refuses
/// the order, acquisition never lists the kind, and the strike
/// machines clear any combat state), speed 0.7 tiles/s (the
/// Sheep row's absolute-scale bits; Fervor's +15% patches at
/// lookup), LoS 11 (9 while carrying a relic — the P5d slice).
/// Its class membership is MONK — until this row landed, the
/// class was memberless and the Scout's +6 / Light Cavalry's
/// +10 anti-monk attack rows keyed on nothing. Cost 100 gold,
/// train 51 s, Monastery only (`BuildingKind::trains` +
/// `EntityKind::train_age`). The radius adopts the foot rows'
/// 1.0 as an implementation constant — the dossier gives no
/// monk radius — never presented as DE data. Reload/delay/
/// range are the inert zeros of the other attack-less rows
/// (Sheep, Trade Cart): with no attack table they are never
/// read.
static MONK: UnitSpec = UnitSpec {
    max_hp: 30,
    attacks: NO_ATTACKS,
    armor_melee: 0,
    armor_pierce: 0,
    armor_building: 0,
    classes: cls::mask(&[cls::MONK]),
    range_max: I16F16::ZERO,
    range_min: I16F16::ZERO,
    reload_ticks: 0,
    attack_delay_ticks: 0,
    accuracy: 100,
    projectile_speed: I16F16::ZERO,
    scatter: I16F16::ZERO,
    blast_radius: I16F16::ZERO,
    pass_through: false,
    blast_secondaries: 0,
    speed: I16F16::from_bits(3_058), // 0.7 tiles/s (absolute scale, see header)
    los: I16F16::from_bits(2_883_584), // 44 (11 tiles)
    radius: I16F16::from_bits(65_536), // 1.0 — adopted (see above)
    carcass_food: 0,
    carcass_decay_bits: 0,
    food_cost: 0,
    wood_cost: 0,
    gold_cost: 100,
    train_ticks: 3060, // 51 s
    regicide_unit: false,
    immune_conversion: false,
    regenerates: 0,
};

/// Relic (Castle P5d, MNK-09): the Gaia object monks carry.
/// Indestructible BY CONSTRUCTION, not by a big HP number: no
/// damage path may ever name it — `can_attack` refuses it,
/// acquisition never lists Gaia objects, the mishit and blast
/// scans skip its kind, and it has no attack, no class and no
/// orders. The HP row (100) is therefore inert state that no
/// system reads down; it exists only because every entity
/// carries an HP field. Speed 0 and LoS 0: it never moves on
/// its own (the relic step writes a carried relic's position
/// directly) and it reveals nothing. The radius (0.5 world
/// units) is an implementation constant by analogy with the
/// smallest shipped rows, recorded as such in the P5 plan
/// §3.5.1 — the sources give no relic radius. Cost/train are
/// the inert zeros: relics are placed by the generator
/// (MAP-13), never produced.
static RELIC: UnitSpec = UnitSpec {
    max_hp: 100,
    attacks: NO_ATTACKS,
    armor_melee: 0,
    armor_pierce: 0,
    armor_building: 0,
    classes: 0,
    range_max: I16F16::ZERO,
    range_min: I16F16::ZERO,
    reload_ticks: 0,
    attack_delay_ticks: 0,
    accuracy: 100,
    projectile_speed: I16F16::ZERO,
    scatter: I16F16::ZERO,
    blast_radius: I16F16::ZERO,
    pass_through: false,
    blast_secondaries: 0,
    speed: I16F16::ZERO,
    los: I16F16::ZERO,
    radius: I16F16::from_bits(32_768), // 0.5 — adopted (see above)
    carcass_food: 0,
    carcass_decay_bits: 0,
    food_cost: 0,
    wood_cost: 0,
    gold_cost: 0,
    train_ticks: 0,
    regicide_unit: false,
    immune_conversion: false,
    regenerates: 0,
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
    armor_building: 0,
    classes: cls::mask(&[cls::HERDABLE]),
    range_max: I16F16::ZERO,
    range_min: I16F16::ZERO,
    reload_ticks: 0,
    attack_delay_ticks: 0,
    accuracy: 100,
    projectile_speed: I16F16::ZERO,
    scatter: I16F16::ZERO,
    blast_radius: I16F16::ZERO,
    pass_through: false,
    blast_secondaries: 0,
    speed: I16F16::from_bits(3_058), // 0.7 tiles/s (absolute scale, see header)
    los: I16F16::from_bits(524_288),  // 8 (2 tiles — wiki, Herdable animal)
    radius: I16F16::from_bits(65_536),
    carcass_food: 100,
    carcass_decay_bits: 273, // 0.25 food/s (ECO-12)
    food_cost: 0,
    wood_cost: 0,
    gold_cost: 0,
    train_ticks: 0,
    regicide_unit: false,
    immune_conversion: false,
    regenerates: 0,
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
    armor_building: 0,
    classes: cls::mask(&[cls::BOAR_HUNTED]),
    range_max: I16F16::ZERO,
    range_min: I16F16::ZERO,
    reload_ticks: 120,
    attack_delay_ticks: 30, // UNVERIFIED (see above)
    accuracy: 100,
    projectile_speed: I16F16::ZERO,
    scatter: I16F16::ZERO,
    blast_radius: I16F16::ZERO,
    pass_through: false,
    blast_secondaries: 0,
    speed: I16F16::from_bits(3_495), // 0.8 tiles/s roaming/return (absolute scale); chase uses BOAR_CHASE_SPEED
    los: I16F16::from_bits(1_310_720), // 20 (5 tiles)
    radius: I16F16::from_bits(78_643), // 1.2 — the boar is bulky
    carcass_food: 340,
    carcass_decay_bits: 437, // 0.40 food/s hunted-animal rate (SPEC §2.2)
    food_cost: 0,
    wood_cost: 0,
    gold_cost: 0,
    train_ticks: 0,
    regicide_unit: false,
    immune_conversion: false,
    regenerates: 0,
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
    armor_building: 0,
    classes: cls::mask(&[cls::PREDATOR]),
    range_max: I16F16::ZERO,
    range_min: I16F16::ZERO,
    reload_ticks: 120,       // UNVERIFIED (see above)
    attack_delay_ticks: 30,  // UNVERIFIED (see above)
    accuracy: 100,
    projectile_speed: I16F16::ZERO,
    scatter: I16F16::ZERO,
    blast_radius: I16F16::ZERO,
    pass_through: false,
    blast_secondaries: 0,
    speed: I16F16::from_bits(4_369), // 1.0 tiles/s (absolute scale) — UNVERIFIED
    los: I16F16::from_bits(3_145_728), // 48 (12 tiles — wiki, Wolf)
    radius: I16F16::from_bits(65_536), // 1.0
    carcass_food: 0,                  // "holds no food" — no carcass
    carcass_decay_bits: 0,
    food_cost: 0,
    wood_cost: 0,
    gold_cost: 0,
    train_ticks: 0,
    regicide_unit: false,
    immune_conversion: false,
    regenerates: 0,
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
    armor_building: 0,
    classes: cls::mask(&[cls::BOAR_HUNTED]),
    range_max: I16F16::ZERO,
    range_min: I16F16::ZERO,
    reload_ticks: 0,
    attack_delay_ticks: 0,
    accuracy: 100,
    projectile_speed: I16F16::ZERO,
    scatter: I16F16::ZERO,
    blast_radius: I16F16::ZERO,
    pass_through: false,
    blast_secondaries: 0,
    speed: I16F16::from_bits(4_369), // 1.0 tiles/s (absolute scale) — UNVERIFIED
    los: I16F16::from_bits(1_048_576),
    radius: I16F16::from_bits(65_536),
    carcass_food: 140,
    carcass_decay_bits: 273, // 0.25 food/s
    food_cost: 0,
    wood_cost: 0,
    gold_cost: 0,
    train_ticks: 0,
    regicide_unit: false,
    immune_conversion: false,
    regenerates: 0,
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
        // Knight (Castle P2, wiki infobox): HP 100, attack 10 with
        // no bonus rows, armor 2/2, RoF 1.8 s, train 30 s at
        // 60 F / 75 G.
        let kn = spec(EntityKind::Knight);
        assert_eq!((kn.max_hp, kn.armor_melee, kn.armor_pierce), (100, 2, 2));
        assert_eq!(kn.attacks, &[(cls::MELEE, 10)]);
        assert_eq!((kn.reload_ticks, kn.train_ticks), (108, 1800));
        assert_eq!((kn.food_cost, kn.wood_cost, kn.gold_cost), (60, 0, 75));
        // Light Cavalry (Castle P2, wiki infobox): HP 60, attack 7
        // +10 vs Monks, armor 0/2, LoS 8 tiles (Castle).
        let lc = spec(EntityKind::LightCavalry);
        assert_eq!((lc.max_hp, lc.armor_melee, lc.armor_pierce), (60, 0, 2));
        assert_eq!(lc.attacks, &[(cls::MELEE, 7), (cls::MONK, 10)]);
        assert_eq!(lc.los, I16F16::from_bits(2_097_152));
        // The Scout's documented anti-Monk row (+6 — the Light
        // Cavalry changelog's "+4 on upgrade" lands on +10).
        let sc = spec(EntityKind::ScoutCavalry);
        assert_eq!(sc.attacks, &[(cls::MELEE, 3), (cls::MONK, 6)]);
        // Cavalry Archer (Castle P2, wiki infobox): HP 50, attack
        // 6 +2 vs Spearmen, range 4, accuracy 50, train 37 s at
        // 40 W / 60 G; membership ARCHERY + CAVALRY.
        let ca = spec(EntityKind::CavalryArcher);
        assert_eq!((ca.max_hp, ca.armor_melee, ca.armor_pierce), (50, 0, 0));
        assert_eq!(ca.attacks, &[(cls::PIERCE, 6), (cls::SPEARMAN, 2)]);
        assert_eq!(ca.range_max, I16F16::from_bits(1_048_576));
        assert_eq!((ca.reload_ticks, ca.accuracy, ca.train_ticks), (120, 50, 2220));
        assert_eq!((ca.food_cost, ca.wood_cost, ca.gold_cost), (0, 40, 60));
        assert_ne!(ca.classes & cls::bit(cls::ARCHERY), 0);
        assert_ne!(ca.classes & cls::bit(cls::CAVALRY), 0);
        // Every other row keeps a zero dead zone (PRJ-08).
        for k in [
            EntityKind::Villager,
            EntityKind::ScoutCavalry,
            EntityKind::Militia,
            EntityKind::Archer,
            EntityKind::ManAtArms,
            EntityKind::Spearman,
            EntityKind::Knight,
            EntityKind::LightCavalry,
            EntityKind::CavalryArcher,
        ] {
            assert_eq!(spec(k).range_min, I16F16::ZERO);
        }
        // Wolf (AI-08; wiki, Wolf): HP 25, attack 3, no food.
        let w = spec(EntityKind::Wolf);
        assert_eq!((w.max_hp, w.carcass_food), (25, 0));
        assert_eq!(w.attacks, &[(cls::MELEE, 3)]);
        // Monk (Castle P5a, MNK-01/CLS-08; wiki Monk infobox via
        // dossier §5.6): HP 30, armor 0/0, NO attack table (the
        // empty table is CLS-08 itself), speed 0.7 tiles/s, LoS
        // 11 tiles, class MONK, 100 gold, train 51 s.
        let mo = spec(EntityKind::Monk);
        assert_eq!((mo.max_hp, mo.armor_melee, mo.armor_pierce), (30, 0, 0));
        assert!(mo.attacks.is_empty());
        assert_eq!(mo.speed, I16F16::from_bits(3_058));
        assert_eq!(mo.los, I16F16::from_bits(2_883_584));
        assert_ne!(mo.classes & cls::bit(cls::MONK), 0);
        assert_eq!((mo.food_cost, mo.wood_cost, mo.gold_cost), (0, 0, 100));
        assert_eq!(mo.train_ticks, 3060);
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
        assert!((ratio(EntityKind::Knight) - 1.35 / 0.80).abs() < 0.001);
        assert!((ratio(EntityKind::LightCavalry) - 1.50 / 0.80).abs() < 0.001);
        assert!((ratio(EntityKind::CavalryArcher) - 1.40 / 0.80).abs() < 0.001);
        assert!((ratio(EntityKind::Sheep) - 0.70 / 0.80).abs() < 0.001);
        // The scout is the fastest unit on the field.
        assert!(spec(EntityKind::ScoutCavalry).speed > spec(EntityKind::Archer).speed);
    }
}
