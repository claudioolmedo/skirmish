//! Entities, move orders, and villager work state (M1–M2).
//!
//! M0 proved the *machinery* of determinism with trivial drifting points.
//! M1 gave those entities real behavior: move orders with A* waypoint
//! trails, followed with steering (separation + arrival). M2 adds kinds
//! (villager / Town Center) and the villager work state machine — the
//! determinism rules, however, are permanent.
//!
//! Entities are stored in a `Vec` sorted by `id` (ids are assigned in
//! order at spawn). Iteration order is therefore always deterministic —
//! there is intentionally no hash map anywhere in the sim.

use fixed::types::I16F16;

use crate::build::BuildingKind;
use crate::econ::ResourceKind;
use crate::units;

/// Owner tag: the (single, for now) human player.
pub const OWNER_PLAYER: u8 = 0;
/// Owner tag: Gaia — the neutral world (wild animals). Captured
/// herdables flip to the capturing player's tag; future multiplayer adds
/// per-player tags beside these.
pub const OWNER_GAIA: u8 = 1;
/// Owner tag: the second player (M5) — the scripted AI opponent's
/// seat. Everything the sim does for `OWNER_PLAYER` it does for this
/// tag too (own stockpile, own Town Center, own population): the sim
/// has no favorite seat (RULEBOOK AI-01).
pub const OWNER_OPPONENT: u8 = 2;

/// Whether an owner tag belongs to a player (human or opponent) as
/// opposed to Gaia.
pub fn is_player_owner(owner: u8) -> bool {
    owner != OWNER_GAIA
}

/// What an entity is. Villagers move and work; the Town Center is a static
/// building (drop-off point and villager producer). M3 adds military
/// kinds alongside these.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EntityKind {
    Villager,
    TownCenter,
    /// M3: the starting cavalry — fast, wide-sighted, weak attack.
    ScoutCavalry,
    /// M3: Barracks infantry.
    Militia,
    /// M3: Barracks ranged infantry (projectiles).
    Archer,
    /// M3: herdable Gaia animal — captured by proximity, then tasked
    /// like a unit; killed, it leaves a carcass node.
    Sheep,
    /// M3: aggressive Gaia animal — aggros on its attacker, chases,
    /// gives up and walks home to heal (the lure loop).
    Boar,
    /// M3: passive Gaia animal — flees when attacked.
    Deer,
    /// M6: the Militia line's Feudal tier (CLS-03) — never trained
    /// directly: existing Militia become Man-at-Arms when the
    /// Barracks upgrade completes, and later Militia production
    /// spawns as Man-at-Arms while the upgrade stands.
    ManAtArms,
    /// The anti-cavalry infantry (study §3.3): Barracks, Feudal
    /// Age — the base of the Spearman → Pikeman → Halberdier line.
    Spearman,
    /// The anti-archer foot archer (study §3.5): Archery Range,
    /// Feudal Age — the base of the Skirmisher line, and the
    /// game's first minimum-range weapon (PRJ-08).
    Skirmisher,
    /// AI-08: the predator — a Gaia combat animal that holds its
    /// post until a player unit enters its line of sight, then
    /// hunts it (the wolf that threatens early villagers). Kills
    /// leave no carcass: it holds no food.
    Wolf,
    /// Castle Age heavy cavalry (Castle P2): the Stable's shock
    /// unit — the Knight line's base tier.
    Knight,
    /// The Scout Cavalry line's Castle tier (Castle P2): never
    /// trained directly — existing Scout Cavalry become Light
    /// Cavalry when the Stable upgrade completes, and later Scout
    /// production spawns as Light Cavalry while the upgrade
    /// stands (the CLS-03 pattern, one line over).
    LightCavalry,
    /// Castle Age mounted archer (Castle P2): Archery Range —
    /// carries both the ARCHERY and CAVALRY classes, so foot
    /// counters and cavalry counters both apply to it.
    CavalryArcher,
    /// Castle Age demolition unit (Castle P3a, CMB-17): trained
    /// only at the Castle — a suicide unit whose landed strike
    /// detonates (blast radius 0.5 tiles) and removes it.
    Petard,
    /// The trade unit (Package D, TRD-05..08): trained at the
    /// Market from the Feudal Age — it hauls no resources itself;
    /// it travels empty to another player's Market and returns
    /// loaded, and its owner is credited gold per completed round
    /// trip by the TRD-06 formula. No attack: a civilian hull.
    TradeCart,
    /// Castle Age siege engine (Castle P4a, GAR-10): trained at
    /// the Siege Workshop — a mobile armored container that
    /// carries up to 6 foot units inside, each passenger making
    /// it faster and harder-hitting against buildings. Its
    /// −3 melee armor is real DE data: melee hits against it
    /// AMPLIFY (CMB-01's formula has no clamp that eats it).
    BatteringRam,
    /// Castle Age siege engine (Castle P4b, PRJ-07 + CMB-16):
    /// trained at the Siege Workshop — a wheeled stone-thrower
    /// whose blast-flagged stones damage everything inside a
    /// 1-tile radius of the impact point (friend and foe alike),
    /// taper with distance, fell trees, and can be ordered at
    /// bare ground (Attack Ground). Its 3-tile minimum range
    /// means it can never fire point-blank (PRJ-08).
    Mangonel,
    /// Castle Age siege engine (Castle P4c, PRJ-09): trained
    /// at the Siege Workshop — a wheeled ballista whose bolts
    /// pass through every enemy unit along their line (the
    /// targeted unit takes full damage, every other enemy
    /// half, friendly units nothing — the whole distinction
    /// from the Mangonel's blast). Its 2-tile minimum range
    /// keeps it from firing point-blank (PRJ-08).
    Scorpion,
    /// Castle Age faith unit (Castle P5a, MNK-01): trained at
    /// the Monastery — it heals allied units (MNK-10) and, from
    /// the P5b slice on, converts enemies and carries relics.
    /// It has NO attack at all (CLS-08): its unit row's attack
    /// table is empty, so no order, acquisition or retaliation
    /// path can ever make it strike. Its MONK class row is what
    /// the Light Cavalry's anti-monk bonus keys on.
    Monk,
    /// The relic (Castle P5d, MNK-09): a Gaia-owned object placed
    /// by the map generator (MAP-13) — indestructible, never a
    /// combat/heal/conversion target, moved only by the relic
    /// lifecycle in `monks.rs` (ground | carried by a monk |
    /// garrisoned in a Monastery). It is NOT mobile: it never
    /// steers, pushes or is pushed; while carried, the relic
    /// step copies its carrier's position each tick.
    Relic,
    /// The Regicide king (VIC-06): never trained — one is spawned
    /// beside each Town Center when a match opens in Regicide
    /// mode. Attack-less and unconvertible; its death eliminates
    /// its owner on the spot, whatever else they own. The mode
    /// lives in the unit row's `regicide_unit` flag (CLS-06), not
    /// in special code paths beyond the death sweep's flag check.
    King,
    /// The Fishing Ship (Water W2a, ECO-15): the Dark Age civilian
    /// ship trained at the Dock — it gathers food from fish nodes
    /// on the water domain and deposits at a Dock only. It has NO
    /// attack at all (the CLS-08 pattern, like the Monk): its unit
    /// row's attack table is empty. It is the first member of the
    /// SHIP armor class (CLS-07 narrows: the class existed
    /// memberless since the class table landed). Class membership
    /// decision (WATER-EXECUTION-PLAN §0.5, pinned 2026-11-05):
    /// the wiki Ship article says all ships except the Fishing
    /// Ship carry the Ships class, yet the Fishing Ship "takes the
    /// exact same bonus damage from all units except the Fire
    /// Galley, as if it were a regular ship" — and the Fire
    /// Galley's update-169123 changelog records it LOSING its
    /// attack bonus vs ships entirely, so the one excepted row no
    /// longer exists in current DE. The ship therefore carries
    /// SHIP and every anti-ship row (the Dock's shelter arrows,
    /// the Watch Tower's +7) applies to it uniformly.
    FishingShip,
}

impl EntityKind {
    /// Stable tag for the state hash and the WASM kind array:
    /// 0 = villager, 1 = town center, 2 = scout cavalry, 3 = militia,
    /// 4 = archer, 5 = sheep, 6 = boar, 7 = deer, 8 = man-at-arms (M6),
    /// 9 = spearman, 10 = skirmisher, 11 = wolf (AI-08; the wolf
    /// branch's original 9 was taken by the Spearman on main),
    /// 12 = knight, 13 = light cavalry, 14 = cavalry archer
    /// (Castle P2 — appended after the Wolf took 11 on main,
    /// so no tag moved), 15 = petard (Castle P3a), 16 = trade
    /// cart (Package D), 17 = battering ram (Castle P4a),
    /// 18 = mangonel (Castle P4b), 19 = scorpion (Castle P4c),
    /// 20 = monk (Castle P5a), 21 = relic (Castle P5d),
    /// 22 = king (VIC-06 — the builder tree's 21 collided with
    /// the Relic on current main; renumbered 21 -> 22 at this
    /// landing, appended after the Relic, so no tag moved).
    /// 23 = fishing ship (Water W2a, ECO-15 — appended after the
    /// King, so no tag moved; verified first free on the W2a base
    /// `7e0d294`, where 0–22 were taken).
    pub fn tag(self) -> u8 {
        match self {
            EntityKind::Villager => 0,
            EntityKind::TownCenter => 1,
            EntityKind::ScoutCavalry => 2,
            EntityKind::Militia => 3,
            EntityKind::Archer => 4,
            EntityKind::Sheep => 5,
            EntityKind::Boar => 6,
            EntityKind::Deer => 7,
            EntityKind::ManAtArms => 8,
            EntityKind::Spearman => 9,
            EntityKind::Skirmisher => 10,
            EntityKind::Wolf => 11,
            EntityKind::Knight => 12,
            EntityKind::LightCavalry => 13,
            EntityKind::CavalryArcher => 14,
            EntityKind::Petard => 15,
            EntityKind::TradeCart => 16,
            EntityKind::BatteringRam => 17,
            EntityKind::Mangonel => 18,
            EntityKind::Scorpion => 19,
            EntityKind::Monk => 20,
            EntityKind::Relic => 21,
            EntityKind::King => 22,
            EntityKind::FishingShip => 23,
        }
    }

    /// Inverse of `tag`; unknown values map to `None`.
    pub fn from_tag(tag: u8) -> Option<EntityKind> {
        match tag {
            0 => Some(EntityKind::Villager),
            1 => Some(EntityKind::TownCenter),
            2 => Some(EntityKind::ScoutCavalry),
            3 => Some(EntityKind::Militia),
            4 => Some(EntityKind::Archer),
            5 => Some(EntityKind::Sheep),
            6 => Some(EntityKind::Boar),
            7 => Some(EntityKind::Deer),
            8 => Some(EntityKind::ManAtArms),
            9 => Some(EntityKind::Spearman),
            10 => Some(EntityKind::Skirmisher),
            11 => Some(EntityKind::Wolf),
            12 => Some(EntityKind::Knight),
            13 => Some(EntityKind::LightCavalry),
            14 => Some(EntityKind::CavalryArcher),
            15 => Some(EntityKind::Petard),
            16 => Some(EntityKind::TradeCart),
            17 => Some(EntityKind::BatteringRam),
            18 => Some(EntityKind::Mangonel),
            19 => Some(EntityKind::Scorpion),
            20 => Some(EntityKind::Monk),
            21 => Some(EntityKind::Relic),
            22 => Some(EntityKind::King),
            23 => Some(EntityKind::FishingShip),
            _ => None,
        }
    }

    /// The earliest age at which this kind may be trained (the
    /// unit-level half of BLD-15's availability rule — building
    /// availability alone cannot express it: the Barracks stands
    /// in the Dark Age but its Spearman is a Feudal unit).
    pub fn train_age(self) -> crate::tech::Age {
        match self {
            // The Trade Cart is a Feudal unit (wiki, Feudal Age
            // article's available-units list: at the Market).
            EntityKind::Spearman | EntityKind::Skirmisher | EntityKind::TradeCart => {
                crate::tech::Age::Feudal
            }
            EntityKind::Knight
            | EntityKind::CavalryArcher
            | EntityKind::Petard
            | EntityKind::BatteringRam
            | EntityKind::Mangonel
            | EntityKind::Scorpion => crate::tech::Age::Castle,
            | EntityKind::Monk => crate::tech::Age::Castle,
            _ => crate::tech::Age::Dark,
        }
    }

    /// The movement domain this kind travels in (Water W2a,
    /// MOV-11): the Fishing Ship is the first water-domain kind —
    /// every path query a unit makes consults this lookup
    /// (`Sim::set_move_order` and kin), and every land kind
    /// defaults to Land, so no existing row changes. The domain
    /// is DERIVED from the kind, never stored (the water plan's
    /// hash discipline: no new hashed state for a land-only
    /// match).
    pub fn domain(self) -> crate::grid::Domain {
        match self {
            EntityKind::FishingShip => crate::grid::Domain::Water,
            _ => crate::grid::Domain::Land,
        }
    }

    /// Mobile units join the steering snapshot and can hold move
    /// orders; the Town Center is a static building and never moves.
    /// The relic (Castle P5d) is static the same way: it is placed
    /// by the generator and moved only by the relic lifecycle
    /// writing its position directly (a carried relic tracks its
    /// monk), never by steering.
    pub fn is_mobile(self) -> bool {
        self != EntityKind::TownCenter && self != EntityKind::Relic
    }

    /// Player units that occupy one population slot while alive
    /// (study §9.4: almost every unit costs exactly 1). Herdables never
    /// count — a captured sheep is livestock, not population.
    pub fn counts_population(self) -> bool {
        matches!(
            self,
            EntityKind::Villager
                | EntityKind::ScoutCavalry
                | EntityKind::Militia
                | EntityKind::Archer
                | EntityKind::ManAtArms
                | EntityKind::Spearman
                | EntityKind::Skirmisher
                | EntityKind::Knight
                | EntityKind::LightCavalry
                | EntityKind::CavalryArcher
                | EntityKind::Petard
                | EntityKind::TradeCart
                | EntityKind::BatteringRam
                | EntityKind::Mangonel
                | EntityKind::Scorpion
                | EntityKind::Monk
                | EntityKind::King
                | EntityKind::FishingShip
        )
    }

    /// Kinds a production building can train *somewhere* (M6: the
    /// per-building relation is `BuildingKind::trains` — Militia at
    /// the Barracks, Archers at the Archery Range since M6, Scout
    /// Cavalry at the Stable). Light Cavalry is absent on purpose:
    /// like Man-at-Arms it is an upgrade-only kind (Castle P2).
    pub fn is_trainable(self) -> bool {
        matches!(
            self,
            EntityKind::Militia
                | EntityKind::Archer
                | EntityKind::ScoutCavalry
                | EntityKind::Spearman
                | EntityKind::Skirmisher
                | EntityKind::Knight
                | EntityKind::CavalryArcher
                | EntityKind::Petard
                | EntityKind::TradeCart
                | EntityKind::BatteringRam
                | EntityKind::Mangonel
                | EntityKind::Scorpion
                | EntityKind::Monk
                | EntityKind::FishingShip
        )
    }

    /// Foot units that may garrison in a defensive building
    /// (Castle P3b, GAR-01): villagers, infantry, foot archers and
    /// monks (the Monk kind landed with Castle P5a). The
    /// Petard is foot-shaped but is a siege/suicide unit and is
    /// NOT garrisonable (scope resolution 2026-10-29).
    pub fn is_foot_garrisonable(self) -> bool {
        matches!(
            self,
            EntityKind::Villager
                | EntityKind::Militia
                | EntityKind::ManAtArms
                | EntityKind::Spearman
                | EntityKind::Archer
                | EntityKind::Skirmisher
                | EntityKind::Monk
        )
    }

    /// Mounted units (Castle P3b, GAR-01): these may garrison ONLY
    /// in a Castle (defensive) — never in a Town Center or tower.
    pub fn is_mounted_garrisonable(self) -> bool {
        matches!(
            self,
            EntityKind::ScoutCavalry
                | EntityKind::LightCavalry
                | EntityKind::Knight
                | EntityKind::CavalryArcher
        )
    }

    /// Military kinds for the combat/AI semantics that gate on
    /// this predicate (stances, attack-move, the AI's army counts
    /// and threat readings): the fighting units. Villagers have
    /// their own idle query (SEL-08) and herdables are livestock,
    /// not military. NOTE: this is deliberately NOT the SEL-09
    /// idle-key subject list — see `is_idle_military` below.
    pub fn is_military(self) -> bool {
        matches!(
            self,
            EntityKind::ScoutCavalry
                | EntityKind::Militia
                | EntityKind::Archer
                | EntityKind::ManAtArms
                | EntityKind::Spearman
                | EntityKind::Skirmisher
                | EntityKind::Knight
                | EntityKind::LightCavalry
                | EntityKind::CavalryArcher
                | EntityKind::Petard
                | EntityKind::Mangonel
                | EntityKind::Scorpion
        )
    }

    /// The SEL-09 idle-military key's subject list: every
    /// `is_military` kind PLUS the Battering Ram (Castle P4a) and
    /// the Monk (Castle P5a) — in the reference game the idle
    /// military key finds idle siege and idle monks too (a monk
    /// that walked off to heal is exactly the unit the key exists
    /// to find). The two kinds were missed when they landed
    /// because the idle query reused `is_military`, whose combat
    /// semantics they must NOT join (a monk has no attacks and no
    /// stance; a ram must not enter the AI's army counts) —
    /// restored by the bot-loop run of 2026-11-01 16:22 after
    /// three live sessions could not find a trained Monk with any
    /// in-client tool. The Trade Cart stays out on purpose: a
    /// civilian unit with its own trade loop (pinned by
    /// trade_tests), never an idle-military subject.
    pub fn is_idle_military(self) -> bool {
        self.is_military()
            || matches!(self, EntityKind::BatteringRam | EntityKind::Monk)
    }
}

/// One queued order (M4, SEL-12): a data-only record of a player
/// command, stored per entity behind whatever the entity is doing
/// now. When the entity becomes idle, the queue's head executes
/// exactly like a freshly issued command of the same kind. Queued
/// orders are player intent, so — like `Command` — they carry world
/// points and ids, never paths; the path is recomputed when the
/// order executes.
/// Unit stance (CMB-09): a persistent per-unit setting on military
/// units — it governs *automatic* engagement only (acquisition and
/// chase); explicit orders always override it. Set by the `SetStance`
/// command; never a task, so it survives orders and idle time.
///
/// Evidence (AoE wiki, "Unit stance", fetched 2026-10-22): Aggressive
/// "units move without limitation and will follow enemy targets in
/// LOS and range until they are killed"; Defensive "units attack any
/// unit in their range, and will follow for a certain number of tiles
/// before returning to their original position"; Stand Ground "units
/// do not move to attack but will attack any enemy in their range";
/// No Attack "units do not automatically attack unless ordered".
/// The wiki's default is Aggressive for military units — the one
/// exception in this game is the starting Scout Cavalry, whose Dark
/// Age default is Stand Ground (MECHANICS-STUDY §9.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum Stance {
    Aggressive = 0,
    Defensive = 1,
    StandGround = 2,
    NoAttack = 3,
}

impl Stance {
    /// Stable tag for the state hash, the WASM ABI, and the netplay
    /// vocabulary: 0 aggressive, 1 defensive, 2 stand ground,
    /// 3 no attack.
    pub fn tag(self) -> u8 {
        self as u8
    }

    pub fn from_tag(tag: u8) -> Option<Stance> {
        match tag {
            0 => Some(Stance::Aggressive),
            1 => Some(Stance::Defensive),
            2 => Some(Stance::StandGround),
            3 => Some(Stance::NoAttack),
            _ => None,
        }
    }
}

impl Default for Stance {
    fn default() -> Self {
        Stance::Aggressive
    }
}

/// A garrison host (Castle P3b, GAR-01): a placed building, or a
/// Town Center (an entity in this architecture, BLD-16). The two
/// id spaces are distinct, so the host reference names its space.
/// Castle P4a adds the third host kind (GAR-10): a Battering Ram
/// — a MOBILE host; its entity id lives in the entity id space,
/// like the Town Center's.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GarrisonRef {
    Building(u32),
    TownCenter(u32),
    Unit(u32),
}

impl GarrisonRef {
    /// Stable tag for the state hash: 1 = building, 2 = town
    /// center, 3 = unit (a Battering Ram, Castle P4a) — 0 is the
    /// hash's "no host" marker, written by the caller.
    pub fn tag(self) -> u8 {
        match self {
            GarrisonRef::Building(_) => 1,
            GarrisonRef::TownCenter(_) => 2,
            GarrisonRef::Unit(_) => 3,
        }
    }
    pub fn id(self) -> u32 {
        match self {
            GarrisonRef::Building(id) | GarrisonRef::TownCenter(id) | GarrisonRef::Unit(id) => id,
        }
    }
}

/// A Town Bell snapshot (Castle P3b, GAR-07): a villager's task and
/// position at the moment the bell rang, restored by the second ring
/// ("back to work"). Plain data — part of the hashed state while set.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BellSnapshot {
    pub pos_x: I16F16,
    pub pos_y: I16F16,
    pub task: Task,
    pub task_node: Option<u32>,
    pub task_building: Option<u32>,
    /// The Town Center whose bell ring took this snapshot
    /// (GAR-07): bells are per-TC toggles, so only THAT TC's
    /// second ring may restore this villager — another TC's ring
    /// cycle neither releases it nor may overwrite the snapshot.
    pub bell_tc: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum QueuedOrder {
    /// Walk to a world point.
    Move { x: I16F16, y: I16F16 },
    /// Work a resource node (villagers).
    Gather { node: u32 },
    /// Attack a target entity.
    Attack { target: u32 },
    /// Build a building anchored at the cell containing `(x, y)`
    /// (villagers). Validation and the wood deduction happen when the
    /// order *executes*, like the base command — a queued build whose
    /// spot or funds have gone bad by then is dropped.
    Build {
        kind: BuildingKind,
        x: I16F16,
        y: I16F16,
    },
    /// Repair a placed building (villagers). Validation happens
    /// when the order *executes*, like the base command — a queued
    /// repair whose target has been destroyed, completed its
    /// repair, or was never valid is dropped.
    Repair { building: u32 },
    /// Repair a Town Center (villagers; Package J, BLD-07's TC
    /// slice). The Town Center is an entity, not a placed
    /// building, so it carries its own queued form. Validation
    /// happens when the order *executes*, like the base command.
    RepairTc { tc: u32 },
    /// Attack-move to a world point (CMB-10): walk there, engaging
    /// whatever enters the unit's sight en route (CMB-08 chain),
    /// then resume the walk and hold at the destination.
    AttackMove { x: I16F16, y: I16F16 },
    /// Patrol toward a world point (CMB-11): installs a loop
    /// between the unit's position at execution and the point. When
    /// the entity already patrols, queueing APPENDS the point to
    /// the loop instead of queueing behind it (the loop never
    /// ends, so a queued patrol could never execute otherwise).
    Patrol { x: I16F16, y: I16F16 },
    /// Attack a world point (CMB-16, Castle P4b): the queued
    /// form of the Attack Ground command — executes exactly like
    /// a freshly issued one when the entity becomes idle.
    AttackGround { x: I16F16, y: I16F16 },
    /// Convert an enemy unit (Castle P5b, MNK-03): the queued
    /// form of the Convert command (SEL-12) — executes exactly
    /// like a freshly issued one when the entity becomes idle.
    Convert { target: u32 },
    /// Convert an enemy building (Castle P5c, MNK-04): the
    /// queued form of the ConvertBuilding command (SEL-12) —
    /// buildings live in their own id space, so the queued form
    /// carries its own variant, like Repair's.
    ConvertBuilding { building: u32 },
    /// Move in formation to a world point (MOV-07): the queued
    /// form of the FormationMove command (SEL-12) — executes
    /// exactly like a freshly issued one when the entity
    /// becomes idle; the slot is computed at execution, from
    /// the group that executes together.
    FormationMove {
        x: I16F16,
        y: I16F16,
        formation: crate::formation::FormationKind,
    },
}

impl QueuedOrder {
    /// Stable tag for the state hash: 0 move, 1 gather, 2 attack,
    /// 3 build, 4 repair, 5 attack-move, 6 patrol, 7 repair-Town-
    /// Center (Package J — append only), 8 attack ground (Castle
    /// P4b landing — its builder tag 7 was already RepairTc here).
    pub fn tag(self) -> u8 {
        match self {
            QueuedOrder::Move { .. } => 0,
            QueuedOrder::Gather { .. } => 1,
            QueuedOrder::Attack { .. } => 2,
            QueuedOrder::Build { .. } => 3,
            QueuedOrder::Repair { .. } => 4,
            QueuedOrder::AttackMove { .. } => 5,
            QueuedOrder::Patrol { .. } => 6,
            QueuedOrder::RepairTc { .. } => 7,
            QueuedOrder::AttackGround { .. } => 8,
            QueuedOrder::Convert { .. } => 9,
            QueuedOrder::ConvertBuilding { .. } => 10,
            QueuedOrder::FormationMove { .. } => 11,
        }
    }
}

/// The villager work state machine (M2). A villager's persistent *order*
/// is its `task_node`; `Task` is the phase of the work cycle it is in:
///
/// - `Gather` command: `Idle → ToNode` (or `→ Returning` first, if the
///   villager carries a different resource — it deposits that first).
/// - `ToNode`: on arrival within gather range, `→ Gathering`.
/// - `Gathering`: when the carry fills up or the node runs dry,
///   `→ Returning` (or `→ Idle` if the node died with an empty carry).
/// - `Returning`: on arrival at the Town Center the carry is deposited,
///   then `→ ToNode` again if the assigned node is still alive,
///   otherwise `→ Idle`.
/// - A `Move` command clears the task from any phase (the villager keeps
///   whatever it carries — see `carry`). The Town Center never works:
///   its task is always `Idle`.
///
/// M2.5 adds the construction phases, driven by `task_building`:
///
/// - `Build` command: `Idle → ToBuild` (travel to the site).
/// - `ToBuild`: on arrival within build range, `→ Building`.
/// - `Building`: each tick in this phase adds progress to the building;
///   if the builder drifts out of build range, `→ ToBuild` again. On
///   completion the builder goes `Idle` — or, for a Farm, straight to
///   gathering the field it just raised.
///
/// Repair (BLD-07) is a single phase carrying its building, the
/// mirror of the construction assignment with the assignment inside
/// the phase instead of `task_building`:
///
/// - `Repair` command: `Idle → Repair { building }` — the villager
///   paths to the building's ring exactly like a builder
///   (`assign_builder`'s approach), and while within the same
///   build range the repair phase restores the building's HP (the
///   restore itself is applied by the repair tick, right after the
///   construction tick). The phase ends — the villager goes `Idle`
///   — when the building reaches full HP, is destroyed, or the
///   owner's stockpile cannot cover a tick's repair drip (the
///   documented DE behavior: repairers stop and stand idle).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Task {
    /// No work order. The entity may still hold a plain move order.
    Idle,
    /// Travelling to the task node to start gathering.
    ToNode,
    /// At the task node, accumulating resources into `carry`.
    Gathering,
    /// Travelling to the Town Center to deposit the carry.
    Returning,
    /// Travelling to the task building's site (M2.5).
    ToBuild,
    /// At the site, actively building (M2.5).
    Building,
    /// Travelling to — or working on — a damaged completed
    /// building of the villager's own player, restoring its HP
    /// (BLD-07). The building id is the repair assignment.
    Repair { building: u32 },
    /// Travelling to — or working on — the villager's own
    /// player's damaged Town Center, restoring the ENTITY's HP
    /// (Package J, BLD-07's TC slice: the Town Center is an
    /// entity in this architecture — BLD-16 — so it cannot ride
    /// `Repair`'s building record; this sibling phase mirrors
    /// `Repair` exactly, with the TC entity id as the assignment
    /// and the 550-wood exception priced in `Sim::repair_tc_tick`).
    RepairTc { tc: u32 },
}

impl Task {
    /// Stable tag for the state hash: 0 idle, 1 to-node, 2 gathering,
    /// 3 returning, 4 to-build, 5 building, 6 repair, 7 repair-Town-
    /// Center (Package J — append only).
    pub fn tag(self) -> u8 {
        match self {
            Task::Idle => 0,
            Task::ToNode => 1,
            Task::Gathering => 2,
            Task::Returning => 3,
            Task::ToBuild => 4,
            Task::Building => 5,
            Task::Repair { .. } => 6,
            Task::RepairTc { .. } => 7,
        }
    }
}

/// A deposit destination (M6, ECO-06): where a hauling villager
/// delivers its carry. Before M6 the only destination was the
/// owner's Town Center; the drop-off buildings (Lumber Camp, Mill,
/// Mining Camp) join it, each accepting only its resources — the
/// sim routes every haul to the *nearest valid* destination for
/// the resource carried.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Dropoff {
    /// The owner's Town Center (accepts every resource).
    TownCenter,
    /// A placed drop-off building, by building id (accepts per its
    /// kind's resource mask — see `BuildingKind::accepts`).
    Building(u32),
}

impl Dropoff {
    /// Stable tag for the state hash: 1 = town center, 2 = building
    /// (0 is the hash's "no destination" marker, written by the
    /// caller for `None`).
    pub fn tag(self) -> u8 {
        match self {
            Dropoff::TownCenter => 1,
            Dropoff::Building(_) => 2,
        }
    }
}

/// An entity: id, kind, fixed-point position/velocity, an optional move
/// order, and (for villagers) work state. M3 adds hit points, ownership,
/// combat state, and animal state.
///
/// Death is a *tombstone* (RULEBOOK CMB-14 deviation, recorded in
/// ARCHITECTURE.md): a dead entity stays in the list with `dead` set —
/// removed from play in every observable way (no movement, work,
/// targeting, population, or rendering), but its id is never reused and
/// the Vec never reindexes, which keeps every id ever issued stable as
/// an array index for the WASM ABI and the renderer's mirrors.
///
/// `Vec` inside `MoveOrder` means `Entity` is `Clone` but not `Copy`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entity {
    pub id: u32,
    pub kind: EntityKind,
    pub pos_x: I16F16,
    pub pos_y: I16F16,
    pub vel_x: I16F16,
    pub vel_y: I16F16,
    /// Active move order, if any. Plain data — part of the hashed state.
    /// Movement is the *mechanism*; the work state machine (re)issues
    /// move orders to get villagers where their task needs them.
    pub order: Option<MoveOrder>,
    /// Queued orders behind the current one (M4, SEL-12), FIFO.
    /// A non-queued command replaces the current order *and* clears
    /// this queue; a queued command appends. The queue advances when
    /// the entity is fully idle (no work task, no move order, no
    /// attack target). Plain data — part of the hashed state.
    pub order_queue: Vec<QueuedOrder>,
    /// Provenance of the CURRENT order (INF-09): true while the
    /// order now executing entered execution from the queue —
    /// popped by the queue advance, including a queued order that
    /// started immediately on an idle entity. Set by the advance,
    /// cleared by every non-queued command (they all clear the
    /// queue too). Its one reader is `Sim::order_route`: the
    /// reference flags every point of a queued chain including
    /// the leg in progress — and the last leg, when the queue
    /// behind it is already empty — while a plain order shows no
    /// persistent flag at all; this bit is what tells the two
    /// apart. Plain data, conditionally hashed (see `Sim::hash`).
    pub order_from_queue: bool,
    /// Current phase of the work cycle (villagers only).
    pub task: Task,
    /// The persistent gather assignment: the node this villager works
    /// until re-commanded. Survives deposit trips; cleared by a `Move`
    /// command or when the node is gone and the carry is delivered.
    pub task_node: Option<u32>,
    /// The construction assignment (M2.5): the building this villager
    /// is travelling to or actively building. Mutually exclusive with
    /// `task_node` — a fresh order of either kind clears the other.
    pub task_building: Option<u32>,
    /// The current deposit destination while hauling (M6, ECO-06):
    /// the nearest valid drop-off for the carried resource, resolved
    /// when the haul starts and re-resolved every tick of the
    /// `Returning` phase (a nearer camp completing mid-haul, or the
    /// destination falling, reroutes the trip). The assignment it
    /// serves — `task_node`, or `task_building` after a force
    /// drop-off — is ECO-07's resume marker: it survives the deposit
    /// and the work cycle resumes from it.
    pub task_dropoff: Option<Dropoff>,
    /// Resources currently carried. Counted in the stockpile only when
    /// deposited at the Town Center. A `Move` override keeps the carry —
    /// it is neither deposited nor lost, just still on the villager.
    pub carry: I16F16,
    /// What `carry` holds. `None` iff `carry` is zero.
    pub carry_kind: Option<ResourceKind>,

    // --- M3: combat state ---
    /// Who owns this entity: `OWNER_PLAYER` or `OWNER_GAIA`. Sheep
    /// flip from Gaia to the player on capture.
    pub owner: u8,
    /// Frozen patch snapshot (Castle P5b, MNK-12): the researched-
    /// tech mask (`TechId::bit` set) this unit carried at the
    /// moment it was converted, stamped by `transfer_ownership`
    /// BEFORE ownership moves. While set, the stat lookup
    /// (`Sim::unit_stats`) resolves tech patches from this mask,
    /// never from the (new) owner's set — the one place per-unit
    /// patch state exists. Build availability is NOT frozen: a
    /// converted villager builds with its new owner's set (the
    /// rule's documented exception reads the owner, not the mask).
    /// `None` on every never-converted unit. Plain data — part of
    /// the hashed state, written only while set (marker 0xBD).
    pub frozen_patches: Option<u64>,
    /// Current hit points (fractional; the entity dies the tick this
    /// drops below 1 — CMB-02 — processed in the combat phase's death
    /// sweep, which sets `dead`).
    pub hp: I16F16,
    /// Tombstone flag: set by the death sweep; the entity is out of
    /// play from that tick on (see the struct doc).
    pub dead: bool,
    /// The entity this one is attacking (explicit order, acquisition,
    /// retaliation, or a boar's aggro), if any. The combat phase
    /// chases it, strikes it, and clears this when it dies.
    pub attack_target: Option<u32>,
    /// The placed building this one is attacking (M5), if any — the
    /// building counterpart of `attack_target` (placed buildings are
    /// not entities, so they need their own slot). Mutually exclusive
    /// with `attack_target`; the combat phase's building step chases
    /// and strikes it, and building destruction clears it.
    pub attack_building: Option<u32>,
    /// The ground point this one is bombarding (CMB-16, Castle
    /// P4b), if any — the position counterpart of `attack_target`:
    /// an Attack Ground order's target is a point of earth, not a
    /// thing on it. Mutually exclusive with `attack_target` and
    /// `attack_building`; the combat phase's ground step chases
    /// until the point sits inside the weapon's firing band and
    /// then keeps releasing on the reload, forever, until the
    /// order is replaced (the reference's standing bombardment).
    pub attack_point: Option<(I16F16, I16F16)>,
    /// Tick at which a pending strike lands (the attack-delay window
    /// of an engagement's first attack — CMB-04). `None` when no
    /// strike is pending.
    pub strike_tick: Option<u64>,
    /// Earliest tick the next attack may begin (reload — CMB-05:
    /// counted from the previous strike's landing tick).
    pub next_attack_tick: u64,
    /// Whether this attacker has already landed a strike on its
    /// current target (the attack delay applies only to the first
    /// strike of an engagement).
    pub attack_engaged: bool,
    /// The last entity that damaged this one — kill credit: a villager
    /// credited with killing an animal starts harvesting its carcass.
    pub last_hit_by: Option<u32>,
    /// The last *player* whose attack damaged this one (BLD-18):
    /// `last_hit_by` names an entity, but a building's arrows have
    /// no entity to name (their `last_hit_by` reads the
    /// `BUILDING_ATTACKER_ID` sentinel), so the death sweep's
    /// VIC-04 destruction credit resolves through this owner
    /// instead. Written by `apply_hit` alongside `last_hit_by` on
    /// every hit; cleared by the Delete command with it. Part of
    /// the hashed state (written only while set).
    pub last_hit_owner: Option<u8>,
    /// Set by the combat phase, read by the next tick's movement
    /// phase: a melee unit actively attacking is not pushed by
    /// separation (MOV-04).
    pub suppress_separation: bool,

    // --- M3: animal state (Gaia kinds; inert zeros elsewhere) ---
    /// The post a boar returns to after it gives up a chase; a
    /// wolf's spawn post — the ground it holds, hunts from, and
    /// walks back to when a chase ends.
    pub home_x: I16F16,
    pub home_y: I16F16,
    /// The animal's current victim: for the boar, whoever hit it
    /// last; for the wolf, the prey it acquired (or whoever hit
    /// it). The combat phase mirrors this into `attack_target`.
    pub aggro_target: Option<u32>,
    /// Damage hits the boar has taken since it last rested at its
    /// post. Two hits (or one villager hit) lock the aggro — the
    /// spec's lure rule: a single military poke is shrugged off after
    /// a short chase, a committed lure is followed all the way.
    pub hits_taken: u32,
    /// Whether the aggro is locked (see `hits_taken`): a locked boar
    /// ignores the give-up distances and pursues until its target
    /// dies or it does.
    pub aggro_locked: bool,
    /// The animal is walking back to its post. A boar heals to
    /// full on arrival and resets its hit count; a wolf does not
    /// heal — it just holds its post again.
    pub returning_home: bool,
    /// Deer: flee until this tick, away from (`flee_from_x`,
    /// `flee_from_y`) — the point the attack came from.
    pub flee_until: u64,
    pub flee_from_x: I16F16,
    pub flee_from_y: I16F16,

    // --- AutoExplore state (SEL-17; inert on every other entity) ---
    /// AutoExplore mode: the entity walks its owner's fog frontier
    /// on its own. Only ever set by the `SetAutoExplore` command on
    /// a player's Scout Cavalry (any scout, starting or trained —
    /// SEL-17); cleared by any manual order (Move / Attack /
    /// AttackBuilding / Gather / queued) and by toggling the mode
    /// off.
    pub auto_explore: bool,
    /// The fog cell the auto-explorer is currently riding toward.
    /// Only `Some` while `auto_explore` is on and a target has been
    /// picked; the walker re-picks on arrival, or at the fog
    /// recompute cadence when the cell has been explored meanwhile.
    pub auto_target: Option<(u32, u32)>,
    /// Sheep: the tick at which a Gaia sheep next reconsiders
    /// wandering (short steps around its home post, long graze
    /// pauses between them). Zero means "not yet scheduled" — the
    /// wander step initializes it from the entity id on first sight.
    /// Owned sheep never wander; inert for other kinds.
    pub wander_until: u64,

    // --- Orders & stances state (CMB-09/10/11; inert zeros elsewhere) ---
    /// The unit's stance (CMB-09). Military kinds only — the
    /// `SetStance` command gates on `EntityKind::is_military`.
    /// Plain data — part of the hashed state.
    pub stance: Stance,
    /// The post a Defensive unit leashes to and returns to
    /// (CMB-09): set when the stance is issued, and moved to the
    /// resolved destination of each explicit move order. Written
    /// to the hash only for Defensive units.
    pub stance_anchor_x: I16F16,
    pub stance_anchor_y: I16F16,
    /// Provenance of the current combat target (CMB-09): true when
    /// the target was set by the acquisition chain or by retaliation
    /// (the unit chose it itself, so its stance governs the chase);
    /// false for explicit player orders and for boar aggro (an
    /// ordered attack is followed to the death, as in DE). Plain
    /// data — part of the hashed state.
    pub attack_auto: bool,
    /// Attack-move destination (CMB-10): a standing order — walk
    /// there, engaging what enters sight en route, then hold. The
    /// destination is the *resolved* point (after the blocked-cell
    /// snap), mirroring `apply_move`. Cleared on arrival and by any
    /// other order. Plain data — part of the hashed state.
    pub attack_move: Option<(I16F16, I16F16)>,
    /// Patrol loop (CMB-11): the points the unit walks forever,
    /// engaging en route like an attack-move; the first point is
    /// the position where the patrol was issued, later points are
    /// appended by further patrol orders. Empty = not patrolling.
    /// Plain data — part of the hashed state.
    pub patrol: Vec<(I16F16, I16F16)>,
    /// Index into `patrol` of the leg the unit heads to next.
    pub patrol_next: usize,

    // --- Garrison state (Castle P3b, GAR-01..08) ---
    /// The host this unit is garrisoned inside, if any. While set,
    /// the unit is out of the field entirely (GAR-02): it is skipped
    /// by spatial iteration, targeting, damage, conversion, monk
    /// healing and the idle counts, and it does not move. Its stored
    /// position stays where it entered (beside the host), so ejection
    /// placement starts from a sane point. Part of the hashed state.
    pub garrisoned_in: Option<GarrisonRef>,
    /// The host this unit is walking to garrison in, if any: the
    /// garrison order's target, entered on arrival (the garrison
    /// tick). Part of the hashed state.
    pub garrison_target: Option<GarrisonRef>,
    /// Whether `garrison_target` is a production-garrison entry
    /// (GAR-08's rally/newly-trained case): production buildings
    /// accept entry ONLY this way — manual orders to them are
    /// rejected, and their garrison can never be re-entered.
    pub garrison_prod_entry: bool,
    /// Town Bell snapshot (GAR-07) while the bell holds this
    /// villager: set by the ring that sent it to shelter, consumed
    /// by the ring that sends it back to work.
    pub bell_snapshot: Option<BellSnapshot>,
    /// Shelter-healing fractional accumulator (GAR-04), in the
    /// BLD-07 repair pattern: per tick the heal adds
    /// `rate_hp_per_min × 65536` to a conceptual accumulator whose
    /// quotient over 3600 is the HP bits restored; this holds the
    /// remainder (0 ≤ r < 3600). Reset on garrison entry/exit.
    pub garrison_heal_remainder: i64,
    /// The passenger list of a garrison HOST entity (Castle P4a,
    /// GAR-10): the Battering Ram is the only entity kind that
    /// hosts — this holds the ids of the foot units inside it,
    /// mirroring `Building.garrison` on the placed-building side.
    /// Empty on every other kind and on an empty Ram. The Ram's
    /// effective speed and anti-building attack are DERIVED from
    /// this list's length at lookup time (GAR-10's prescription)
    /// — no cached counters. Part of the hashed state, written
    /// only while non-empty.
    pub garrison: Vec<u32>,
    // --- Trade state (Package D, TRD-05..08; Trade Cart only,
    // inert defaults on every other kind) ---
    /// The home depot: the Market this cart returns its gold to —
    /// the Market that trained it, stamped at spawn (TRD-05). If
    /// that Market falls, the trade step re-resolves home to the
    /// owner's remaining completed Market farthest from the
    /// target (the reference's longest-route preference); with no
    /// home standing, a loaded cart waits at the foreign depot.
    /// Plain data — part of the hashed state (written only while
    /// any trade field is non-default).
    pub trade_home: Option<u32>,
    /// The foreign depot this cart is tasked to: a completed
    /// Market owned by *another* player (TRD-07 — ally, neutral
    /// or enemy; the sim never consults diplomacy for validity).
    /// `None` = not trading. Plain data — part of the hashed
    /// state, on the same conditional write as `trade_home`.
    pub trade_target: Option<u32>,
    /// The leg the cart is on: `false` = travelling empty toward
    /// the foreign depot, `true` = returning loaded. The gold is
    /// credited to the owner's stockpile at the moment the loaded
    /// cart arrives home (TRD-05) — never on the one-way leg.
    pub trade_loaded: bool,
    // --- Monk state (Castle P5a, MNK-10; Monk only, inert
    // defaults on every other kind) ---
    /// The allied unit this monk is healing: set by an explicit
    /// `Command::Heal`, or by the monk phase's auto-task when the
    /// monk stands idle. Cleared when the target dies, reaches
    /// full HP, stops being healable, or the monk is re-tasked.
    /// Plain data — part of the hashed state, written only while
    /// any monk field is non-default (the monk block, marker
    /// 0xBB). The P5b slice extends this same field set with the
    /// faith and conversion state — the block is the extension
    /// point, and pre-P5 streams never write it.
    pub heal_target: Option<u32>,
    /// Faith (Castle P5b, MNK-02): the monk's conversion resource
    /// in milli-percent, 0..=100_000 (100_000 = full). A conversion
    /// may start only at full; a success drains the participant
    /// set to 0 (MNK-06); regen restores full in 62 s (3 720
    /// ticks; 33 s with Illumination — the Imperial tech landed
    /// in Castle P5f, read in `monks.rs`). Default is FULL: a resting
    /// monk is indistinguishable from a pre-P5b stream (the 0xBB
    /// block writes only while any monk field is non-default).
    /// Plain data — part of the hashed state.
    pub faith: i32,
    /// Faith regen fractional accumulator (MNK-02), in the
    /// Treadmill Crane scalar pattern: per tick the regen adds
    /// 100_000 to a conceptual accumulator whose quotient over the
    /// regen span is the milli-percent restored; this holds the
    /// remainder. Reset on drain.
    pub faith_remainder: i64,
    /// The unit this monk is converting (Castle P5b, MNK-03): the
    /// target half of the plan's ConversionRecord, flattened into
    /// the entity record like `heal_target`. Cleared when the
    /// target dies, converts, garrisons or leaves eligibility, or
    /// the monk is re-tasked. Plain data — part of the hashed
    /// state (0xBB block).
    pub conversion_target: Option<u32>,
    /// The BUILDING this monk is converting (Castle P5c,
    /// MNK-04): the building half of the conversion record —
    /// buildings live in their own id space, so the target rides
    /// its own field beside `conversion_target` (exactly one of
    /// the two is ever set). Its presence is ALSO the MNK-07
    /// building→unit reset marker: a unit conversion started
    /// while this is set zeroes the sticky CI counter (the
    /// update-95810 rule). Plain data — part of the hashed state
    /// (0xBB block, appended in P5c) and the save record (v9).
    pub conversion_target_building: Option<u32>,
    /// Sticky CI progress (MNK-07): completed conversion intervals
    /// toward the current engagement. Lives on the MONK, never on
    /// the pairing: a unit→unit target switch and the automatic
    /// chase keep it; a manual move order zeroes it (the
    /// building→unit reset arm is P5c — no building conversion
    /// exists yet to switch from).
    pub ci_progress: u32,
    /// Ticks elapsed inside the current CI (0..75, MNK-03's
    /// ≈1.25 s interval). Advances only while the target is in
    /// conversion range.
    pub ci_ticks: u32,
    /// Resistance level of the current conversion target (MNK-05),
    /// stamped when the conversion starts: chance per rolled CI is
    /// 38 / max(1, level) %. 1 ordinary, 2 Scout-line.
    pub conversion_level: u8,
    /// CI shift of the first roll (MNK-05), stamped at start:
    /// first rolled CI = 5 + this (Scout +3, Devotion +1, Faith
    /// +4 — Faith is the Imperial tech landed in Castle P5f).
    pub conversion_start_shift: i32,
    /// CI shift of the guarantee (MNK-05), stamped at start:
    /// guarantee CI = 9 + this (Scout +1, Devotion +1, Faith +4).
    pub conversion_guarantee_shift: i32,
    /// Healing fractional accumulator (MNK-10), in the GAR-04
    /// shelter pattern: per tick the heal adds
    /// `150 × 65536` to a conceptual accumulator whose quotient
    /// over 3600 is the HP bits restored (150 HP/min = 2.5 HP/s
    /// exactly, with no rounding drift); this holds the
    /// remainder (0 ≤ r < 3600). Reset when the target changes
    /// or reaches full HP.
    pub heal_remainder: i64,
    // --- Relic state (Castle P5d, MNK-09; inert defaults on
    // every kind except the Monk and the Relic) ---
    /// The relic this monk carries, if any (Monk only): while
    /// set, the monk's LoS reads 9 instead of 11 and conversion
    /// orders are refused (he delivers instead — MNK-09);
    /// healing stays available. The inverse of the relic's
    /// `relic_carried_by` (the garrison back-pointer pattern).
    /// Plain data — part of the hashed state (the 0xBB monk
    /// block, extended in P5d) and the save record (v10).
    pub carrying_relic: Option<u32>,
    /// The monk tasked to pick up a relic (Monk only): the relic
    /// entity id — on the ground, or garrisoned in an own
    /// Monastery (the DE retrieve order, MNK-09). The relic step
    /// walks the monk to it and completes the pickup on arrival.
    /// Plain data — hashed/saved with `carrying_relic`.
    pub relic_pickup_target: Option<u32>,
    /// The Monastery this carrying monk is tasked to deposit
    /// its relic in (Monk only): a building id. The relic step
    /// walks the monk adjacent and garrisons the relic on
    /// arrival (ECO-16's trickle starts). Plain data —
    /// hashed/saved with `carrying_relic`.
    pub relic_deposit_target: Option<u32>,
    /// The monk carrying this relic (Relic kind only): the
    /// carrier half of MNK-09's state — `Some` exactly while the
    /// relic's state is carried-by. While set, the relic is out
    /// of the field at its carrier's position (the relic step
    /// copies the position each tick). Plain data — part of the
    /// hashed state (marker 0xBE) and the save record (v10).
    pub relic_carried_by: Option<u32>,
    /// The Monastery this relic is garrisoned in (Relic kind
    /// only): `Some` exactly while the relic's state is
    /// garrisoned-in — the relic has left the map into the
    /// building's relic store (`Building::relics`) and pays the
    /// ECO-16 trickle. Plain data — hashed/saved with
    /// `relic_carried_by`.
    pub relic_garrisoned_in: Option<u32>,
}

impl Entity {
    /// The shared blank state every constructor starts from: no
    /// orders, no work, full HP for its kind.
    fn blank(id: u32, kind: EntityKind, owner: u8, pos_x: I16F16, pos_y: I16F16) -> Self {
        Entity {
            id,
            kind,
            pos_x,
            pos_y,
            vel_x: I16F16::ZERO,
            vel_y: I16F16::ZERO,
            order: None,
            order_queue: Vec::new(),
            order_from_queue: false,
            task: Task::Idle,
            task_node: None,
            task_building: None,
            task_dropoff: None,
            carry: I16F16::ZERO,
            carry_kind: None,
            owner,
            hp: I16F16::from_num(units::spec(kind).max_hp),
            dead: false,
            attack_target: None,
            attack_building: None,
            attack_point: None,
            strike_tick: None,
            next_attack_tick: 0,
            attack_engaged: false,
            last_hit_by: None,
            last_hit_owner: None,
            suppress_separation: false,
            home_x: pos_x,
            home_y: pos_y,
            aggro_target: None,
            hits_taken: 0,
            aggro_locked: false,
            returning_home: false,
            flee_until: 0,
            flee_from_x: I16F16::ZERO,
            flee_from_y: I16F16::ZERO,
            auto_explore: false,
            auto_target: None,
            wander_until: 0,
            stance: Stance::default(),
            stance_anchor_x: pos_x,
            stance_anchor_y: pos_y,
            attack_auto: false,
            attack_move: None,
            patrol: Vec::new(),
            patrol_next: 0,
            garrisoned_in: None,
            garrison_target: None,
            garrison_prod_entry: false,
            bell_snapshot: None,
            garrison_heal_remainder: 0,
            garrison: Vec::new(),
            trade_home: None,
            trade_target: None,
            trade_loaded: false,
            heal_target: None,
            faith: 100_000,
            faith_remainder: 0,
            conversion_target: None,
            conversion_target_building: None,
            ci_progress: 0,
            ci_ticks: 0,
            conversion_level: 1,
            conversion_start_shift: 0,
            conversion_guarantee_shift: 0,
            heal_remainder: 0,
            carrying_relic: None,
            relic_pickup_target: None,
            relic_deposit_target: None,
            relic_carried_by: None,
            relic_garrisoned_in: None,
            frozen_patches: None,
        }
    }

    /// A fresh idle villager at a position.
    pub fn villager(id: u32, pos_x: I16F16, pos_y: I16F16) -> Self {
        Entity::blank(id, EntityKind::Villager, OWNER_PLAYER, pos_x, pos_y)
    }

    /// A fresh unit or animal of any kind at a position, with the
    /// given owner. Animals spawn as Gaia; military units as the
    /// player's. HP comes from the kind's table row.
    pub fn spawned(id: u32, kind: EntityKind, owner: u8, pos_x: I16F16, pos_y: I16F16) -> Self {
        Entity::blank(id, kind, owner, pos_x, pos_y)
    }

    /// The static Town Center building at a position.
    pub fn town_center(id: u32, pos_x: I16F16, pos_y: I16F16) -> Self {
        Entity::blank(id, EntityKind::TownCenter, OWNER_PLAYER, pos_x, pos_y)
    }

    /// Clear all combat engagement state (target, pending strike,
    /// engagement flag). Used on death, on re-tasking, and when a
    /// target becomes invalid.
    pub fn clear_combat(&mut self) {
        self.attack_target = None;
        self.attack_building = None;
        self.attack_point = None;
        self.strike_tick = None;
        self.attack_engaged = false;
        // The engagement is over, so its provenance is too (CMB-09).
        self.attack_auto = false;
    }
}

/// A move order: the commanded target plus the A* waypoint trail leading
/// to it. `wp_index` is the waypoint currently being steered toward.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MoveOrder {
    pub target_x: I16F16,
    pub target_y: I16F16,
    /// Waypoints in world coordinates: cell centers, ending exactly on
    /// the commanded target point.
    pub waypoints: Vec<(I16F16, I16F16)>,
    pub wp_index: usize,
    /// Arrival radius for the FINAL waypoint (steer::WAYPOINT_REACH
    /// for ordinary orders). A woodcutter's tree order carries
    /// steer::TREE_ARRIVE (0.5): with the generic radius, steering
    /// completed his order while he was still ~4.3 units from the
    /// trunk and he chopped from a full tile away (2026-10-24).
    pub arrive: I16F16,
    /// MOV-06 group stamp: the temporary group this order belongs
    /// to, stamped at order issue when several units received the
    /// same move order in one issue action (see `Sim`'s grouping
    /// session). `0` = no group (every internal order, every
    /// single-unit order, and — by design — the slowest member(s)
    /// of a group, who need no clamp). Plain data — part of the
    /// hashed state, written only while stamped (see `Sim::hash`).
    pub group_id: u32,
    /// MOV-06 speed cap: the slowest member's speed at issue
    /// time, in the same world-units-per-tick scale as the unit
    /// table. Steering clamps a stamped order's speed to this
    /// (`Sim`'s movement phase), so the group advances no faster
    /// than its slowest member and arrives together. Meaningful
    /// only while `group_id != 0`; the stamp is fixed at issue —
    /// a member dying or leaving does not recompute it, and any
    /// new order replaces the stamp with the new order's own
    /// (usually none), which is how the cap ends.
    pub group_cap: I16F16,
}
