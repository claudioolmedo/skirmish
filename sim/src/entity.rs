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
}

impl EntityKind {
    /// Stable tag for the state hash and the WASM kind array:
    /// 0 = villager, 1 = town center, 2 = scout cavalry, 3 = militia,
    /// 4 = archer, 5 = sheep, 6 = boar, 7 = deer, 8 = man-at-arms (M6),
    /// 9 = spearman, 10 = skirmisher, 11 = wolf (AI-08; the wolf
    /// branch's original 9 was taken by the Spearman on main),
    /// 12 = knight, 13 = light cavalry, 14 = cavalry archer
    /// (Castle P2 — appended after the Wolf took 11 on main,
    /// so no tag moved), 15 = petard (Castle P3a).
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
            _ => None,
        }
    }

    /// The earliest age at which this kind may be trained (the
    /// unit-level half of BLD-15's availability rule — building
    /// availability alone cannot express it: the Barracks stands
    /// in the Dark Age but its Spearman is a Feudal unit).
    pub fn train_age(self) -> crate::tech::Age {
        match self {
            EntityKind::Spearman | EntityKind::Skirmisher => crate::tech::Age::Feudal,
            EntityKind::Knight | EntityKind::CavalryArcher | EntityKind::Petard => {
                crate::tech::Age::Castle
            }
            _ => crate::tech::Age::Dark,
        }
    }

    /// Mobile units join the steering snapshot and can hold move
    /// orders; the Town Center is a static building and never moves.
    pub fn is_mobile(self) -> bool {
        self != EntityKind::TownCenter
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
        )
    }

    /// Foot units that may garrison in a defensive building
    /// (Castle P3b, GAR-01): villagers, infantry, foot archers and
    /// monks. Monks do not exist as a kind yet (Castle P5); the
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

    /// Military kinds for the idle-military query (SEL-09): the
    /// fighting units. Villagers have their own idle query (SEL-08)
    /// and herdables are livestock, not military.
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
        )
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
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GarrisonRef {
    Building(u32),
    TownCenter(u32),
}

impl GarrisonRef {
    /// Stable tag for the state hash: 1 = building, 2 = town center
    /// (0 is the hash's "no host" marker, written by the caller).
    pub fn tag(self) -> u8 {
        match self {
            GarrisonRef::Building(_) => 1,
            GarrisonRef::TownCenter(_) => 2,
        }
    }
    pub fn id(self) -> u32 {
        match self {
            GarrisonRef::Building(id) | GarrisonRef::TownCenter(id) => id,
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
}

impl QueuedOrder {
    /// Stable tag for the state hash: 0 move, 1 gather, 2 attack,
    /// 3 build, 4 repair, 5 attack-move, 6 patrol.
    pub fn tag(self) -> u8 {
        match self {
            QueuedOrder::Move { .. } => 0,
            QueuedOrder::Gather { .. } => 1,
            QueuedOrder::Attack { .. } => 2,
            QueuedOrder::Build { .. } => 3,
            QueuedOrder::Repair { .. } => 4,
            QueuedOrder::AttackMove { .. } => 5,
            QueuedOrder::Patrol { .. } => 6,
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
}

impl Task {
    /// Stable tag for the state hash: 0 idle, 1 to-node, 2 gathering,
    /// 3 returning, 4 to-build, 5 building, 6 repair.
    pub fn tag(self) -> u8 {
        match self {
            Task::Idle => 0,
            Task::ToNode => 1,
            Task::Gathering => 2,
            Task::Returning => 3,
            Task::ToBuild => 4,
            Task::Building => 5,
            Task::Repair { .. } => 6,
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
}
