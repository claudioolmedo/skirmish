//! The scripted AI opponent (M5) — RULEBOOK §15's v1 subset.
//!
//! The AI is a *player*, not a cheat (AI-01/AI-02): it owns the
//! opponent seat (`OWNER_OPPONENT`) with its own stockpile, Town
//! Center, population and buildings, and everything it does goes
//! through the same `Command` queue the human's client uses —
//! `TrainVillagerAt`, `Gather`, `Build`, `TrainAt`, `SetRally`,
//! `Attack`, `AttackBuilding`. It reads only sim state a player at
//! its seat could know — and since M7 that includes the fog rules
//! (VIS-01): its *combat* targeting aims only at units it can
//! currently see and buildings it has explored, its Scout is sent
//! to reveal the enemy start before the first wave can aim at it,
//! and the one positional fact it uses about the enemy base is
//! Arabia's rotational symmetry (the mirror of its own Town
//! Center — what any player knows about a mirrored map). Its
//! economy assigns nodes by Town-Center proximity (its own start's
//! package — its explored home ground). It never draws from the
//! RNG — its decisions are pure functions of state, in fixed
//! iteration order (ascending entity/building/node id), so a match
//! replays bit-identically from (seed, mode, human commands)
//! alone.
//!
//! The script (AI-04's build order, at v1 depth):
//!
//! 1. Keep the Town Center training villagers until the economy
//!    target is reached.
//! 2. Build ahead of the population cap (a House whenever the
//!    population closes on it); raise the economy buildings first
//!    (a Mill by the berries, a Lumber Camp by the woodline), then
//!    a Barracks once the economy is large enough to feed an army,
//!    and an Archery Range once Feudal stands (M6).
//! 2b. Repair its own damaged buildings (BLD-17): an idle or
//!     nearby villager is sent through the same `Repair` command
//!     a player issues — production buildings first, buildings
//!     under attack next, the most damaged first — capped per
//!     profile so the economy is never stripped bare, and only
//!     while the wood stockpile can pay the repair drip.
//! 3. Put every idle villager to work, balancing food / wood / gold
//!    toward fixed worker targets (surplus to food); food comes
//!    from the nearest food source — berries or carcasses first,
//!    and when no food source is near the Town Center it hunts its
//!    own sheep (the killer auto-harvests, ECO-13).
//! 4. Research at the Town Center when the queue stands empty:
//!    Loom once the village is grown, then the Feudal Age when
//!    its requirement buildings stand and a food spare exists
//!    (M6).
//! 5. Rally each production building toward the human's base and
//!    train a mixed army — Militia, Spearman, Archer and
//!    Skirmisher, each kind at its own building — up to the army
//!    target. The mix follows the profile's composition weights
//!    (AI-04): the kind furthest behind its weighted share of
//!    the army so far trains next, and on Normal a visible enemy
//!    archer mass promotes the Skirmisher — the anti-archer
//!    answer — to the head of the ranking.
//! 6. Launch attack waves: when the idle army at home reaches the
//!    wave size, every idle soldier attacks — the nearest
//!    *reachable* human unit, else the human Town Center, else the
//!    nearest human building — then regroups for the cooldown.
//!    Idle soldiers also defend: a reachable human unit near the
//!    AI's Town Center is engaged on sight.
//!
//! Difficulty (AI-03) is *behavior only* — the config below. Costs,
//! stats, production times and the damage formula are the human's;
//! Easy simply decides less often and aims lower.
//!
//! Deliberately out of v1 (documented, not silently missing):
//! villager flee/garrison micro (AI-04 lists reacting to attacks;
//! v1 villagers stand and work), stone collection and Farms (the
//! script lives on berries/hunt/wood/gold — its food sources are
//! finite, which also bounds how long a v1 match can stall),
//! scouting, target prioritization beyond nearest, and difficulty
//! levels past Easy/Normal (AI-03's five named levels collapse to
//! the two shipped profiles).

use fixed::types::I16F16;

use crate::build::BuildingKind;
use crate::econ::ResourceKind;
use crate::entity::{EntityKind, Task, OWNER_GAIA, OWNER_OPPONENT, OWNER_PLAYER};
use crate::{steer, Command, MatchMode, Sim};

/// The AI's behavior profile (AI-03: behavior parameters only).
#[derive(Clone, Copy, Debug)]
pub(crate) struct AiConfig {
    /// Ticks between decisions.
    pub cadence: u64,
    /// Villagers to build the economy to (living + queued).
    villager_target: u32,
    /// Worker allocation targets by resource; villagers beyond the
    /// sum go to food.
    food_workers: u32,
    wood_workers: u32,
    gold_workers: u32,
    /// Living villagers required before the Barracks goes up.
    barracks_min_villagers: u32,
    /// Army size to build toward (living military, Scout included).
    army_target: u32,
    /// Idle soldiers at home needed to launch a wave.
    wave_size: usize,
    /// Ticks between waves.
    wave_cooldown: u64,
    /// Most villagers on repair duty at once, across all buildings
    /// (BLD-17). The cap is what keeps repair from stripping the
    /// economy bare mid-attack: it is the same discipline as the
    /// two-hunter cap on sheep hunts — a special duty may borrow
    /// only a small, fixed slice of the village. Against the
    /// villager targets (10 Easy / 16 Normal) that slice is about
    /// a fifth of the workforce — roughly what a player pulls onto
    /// a burning Barracks — while the rest keeps gathering, so the
    /// wood the drip spends is also being replaced.
    repairers_max: u32,
    /// Army composition weights over the four trainable kinds
    /// (`MIX_KINDS` order: Militia, Spearman, Archer,
    /// Skirmisher). Each production decision trains the kind
    /// furthest behind its weighted share of the army so far
    /// (living + queued), so the standing army converges to the
    /// weights' ratio. The mix is grounded in the counter
    /// table: Militia and Archers are the gold-backed core (the
    /// M5 script's pair); the Spearman is the cheap front line —
    /// no gold, and the game's only answer-in-advance to
    /// cavalry (its +15 already tells against the enemy's
    /// Scout); the Skirmisher is the anti-archer screen (no
    /// gold either, +3 vs Archers behind 3 pierce armor).
    /// Folding the two gold-free kinds in holds the army's
    /// *size* profile — `army_target` is untouched — while
    /// easing the gold draw the old Militia/Archer alternation
    /// put on a handful of gold miners.
    mix_weights: [u32; 4],
    /// Whether the script adapts its mix to the enemy army it
    /// can currently see (the archer-mass answer in
    /// `plan_train_military`). Normal adapts; Easy plays its
    /// flat weights regardless — behavior only, like every
    /// other profile difference (AI-03).
    adaptive: bool,
}

/// The profile for a mode. Sandbox has no AI; its config is the
/// Normal one and is never used (decide never runs there).
pub(crate) fn config(mode: MatchMode) -> AiConfig {
    match mode {
        MatchMode::AiEasy => AiConfig {
            cadence: 120,
            villager_target: 10,
            food_workers: 6,
            wood_workers: 4,
            gold_workers: 2,
            barracks_min_villagers: 10,
            army_target: 5,
            wave_size: 4,
            wave_cooldown: 7200,
            repairers_max: 2,
            mix_weights: [1, 1, 1, 1],
            adaptive: false,
        },
        // Sandbox and TwoPlayer never reach `decide` (the tick gate
        // admits only the AI modes); they share this arm purely so
        // the match stays exhaustive.
        MatchMode::AiNormal | MatchMode::Sandbox | MatchMode::TwoPlayer => AiConfig {
            cadence: 30,
            villager_target: 16,
            food_workers: 7,
            wood_workers: 5,
            gold_workers: 3,
            barracks_min_villagers: 8,
            army_target: 10,
            wave_size: 6,
            wave_cooldown: 3600,
            repairers_max: 3,
            mix_weights: [3, 2, 3, 2],
            adaptive: true,
        },
    }
}

/// The AI's persistent bookkeeping — deliberately almost nothing:
/// every decision re-reads live state, so the only thing worth
/// storing is the wave clock. Part of the hashed state.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct AiState {
    /// The next tick at which a wave may launch.
    pub next_wave_tick: u64,
}

/// How close (world units) a human unit must come to the AI's Town
/// Center before idle soldiers engage it in defense: 10 tiles —
/// the Town Center and its inner resource lines. (M5's arena value
/// was 8 *units*, sized to a 50×50 map whose Town Centers stood
/// ~135 units apart; at M7's 120-tile map scale the same idea is a
/// tile radius. Starting Scouts stand beside their *own* Town
/// Centers, ~450 units from the enemy's, so no cross-triggering.)
const DEFENSE_RADIUS: I16F16 = I16F16::from_bits(40 * 65536);
/// How far from its Town Center the AI looks for a food source
/// before it starts hunting its own sheep instead: 11 tiles — the
/// berry patch sits 5–8 tiles out (20–32 units) and near-sheep
/// carcasses land beside the Town Center, so this covers the
/// start's own food with margin (M7 rescale of the arena's 20).
const FOOD_SEARCH_RADIUS: I16F16 = I16F16::from_bits(44 * 65536);
/// Wood reserve below which the AI starts no repairs (BLD-17):
/// a full repair of even the cheapest building (a House: 25 wood
/// to build) costs 12.5 wood of drip, so under 10 wood in the
/// bank no meaningful restoration can be paid for — and scarce
/// wood belongs to the build queue first, the villagers worth
/// more chopping than standing at a wall they cannot mend. The
/// floor doubles as hysteresis against the sim's own broke-stop:
/// when a repair does drain the stockpile dry, `repair_tick`
/// releases the repairers to idle, and this gate keeps the
/// script from re-assigning them until the wood is back.
const REPAIR_MIN_WOOD: I16F16 = I16F16::from_bits(10 * 65536);

/// One decision pass. Called from `Sim::tick` on the cadence;
/// enqueues commands only (see the module docs). Every loop runs in
/// ascending id order and every choice is a pure function of state.
pub(crate) fn decide(sim: &mut Sim) {
    let cfg = config(sim.mode());
    let me = OWNER_OPPONENT;
    // A resigned AI (AI-05: no Town Center, no villagers) issues no
    // further orders — its leftover soldiers simply stand.
    let my_tc = sim.tc_entity_of(me).map(|tc| (tc.id, tc.pos_x, tc.pos_y));
    let my_villagers = count_kind(sim, me, EntityKind::Villager);
    if my_tc.is_none() && my_villagers == 0 {
        return;
    }
    // Entities that already received a command this pass — commands
    // apply next tick, so state still shows them idle; the script
    // must not double-book them within one decision.
    let mut acted: Vec<u32> = Vec::new();

    // 1. Villager production: keep the Town Center busy until the
    //    economy target (living + already queued) is met.
    if let Some((tc_id, _, _)) = my_tc {
        let queued = sim.tc_queued_villagers(tc_id);
        if my_villagers + queued < cfg.villager_target && sim.can_train_villager_at(tc_id) {
            sim.push_command(Command::TrainVillagerAt { tc: tc_id });
        }
    }

    // 1.5 Repair (BLD-17): a damaged building — above all one
    //     under attack right now — outranks new construction, so
    //     repair claims its villagers before the build step does.
    plan_repair(sim, &cfg, &mut acted);

    // 2. Construction (at most one placement per decision): houses
    //    ahead of the cap first, then the economy buildings.
    plan_build(sim, &cfg, &mut acted);

    // 2.5 Research (M6): Loom, then the Feudal advancement — only
    //     ever onto an *empty* Town Center queue, so research
    //     never jumps the villager line (or delays it by more than
    //     its own duration once started).
    plan_research(sim, &cfg);

    // 3. Production-building rally: once each, toward a staging
    //    point short of the enemy start, so fresh troops walk
    //    toward the front.
    plan_rally(sim);

    // 3.5 Scouting (M7): until the enemy start's ground is on the
    //     AI's map, its idle Scout rides there — the wave step can
    //     only aim at what the AI has seen.
    plan_scout(sim, &mut acted);

    // 4. Military production: keep the producing building's queue
    //    busy (one enqueue per decision) toward the army target,
    //    training the kind furthest behind its share of the
    //    profile's composition weights at its own building (see
    //    `plan_train_military`). If the preferred kind cannot be
    //    trained, walk down the ranking before giving up.
    plan_train_military(sim, &cfg);

    // 5. Idle villagers to work.
    assign_idle_villagers(sim, &cfg, &mut acted);

    // 6. Defense, then the wave.
    plan_defense(sim, &mut acted);
    plan_wave(sim, &cfg, &mut acted);
}

/// Living units of one kind owned by `owner`.
fn count_kind(sim: &Sim, owner: u8, kind: EntityKind) -> u32 {
    sim.entities()
        .iter()
        .filter(|e| e.owner == owner && e.kind == kind && !e.dead)
        .count() as u32
}

/// Living military (Scout, Militia, Archer) owned by `owner`.
fn military(sim: &Sim, owner: u8) -> Vec<u32> {
    sim.entities()
        .iter()
        .filter(|e| e.owner == owner && !e.dead && e.kind.is_military())
        .map(|e| e.id)
        .collect()
}

/// Whether an entity is doing nothing at all right now: no work
/// task, no move order, no attack, nothing queued. (For a villager
/// "idle" follows the SEL-08 definition; military use the same
/// fields.)
fn is_idle(sim: &Sim, id: u32) -> bool {
    match sim.entities().iter().find(|e| e.id == id) {
        Some(e) => {
            !e.dead
                && e.task == Task::Idle
                && e.order.is_none()
                && e.attack_target.is_none()
                && e.attack_building.is_none()
                && e.order_queue.is_empty()
        }
        None => false,
    }
}

/// Repair step (BLD-17): send villagers to mend the AI's own
/// damaged, completed buildings through the ordinary
/// `Command::Repair` — the same validation, rates and wood drip
/// a player's order gets (the sim releases repairers on its own
/// when the building is full or a tick's drip is unaffordable;
/// this step only ever *assigns*). Targets in priority order:
/// production buildings first (the army's source — and the Town
/// Center ahead of them all: it trains the villagers everything
/// else runs on, and since Package J it is repairable for the
/// AI through `Command::RepairTc` exactly as for a player,
/// BLD-17's parity), then buildings under attack right now,
/// then the most damaged; ties by ascending id. Farms are never
/// assigned: a field is not repaired, for the AI exactly as for
/// a player.
/// Repairers come nearest-first — idle villagers, then gatherers
/// pulled off the line (the construction step's own fallback) —
/// and never more than `cfg.repairers_max` on duty at once
/// across all buildings, so the economy is never stripped bare.
/// A villager already on repair duty keeps the assignment: it
/// counts against the cap here, and the generic allocator never
/// sees it (`Task::Repair` is not idle). Below the wood floor no
/// new assignment starts at all (see `REPAIR_MIN_WOOD`).
/// Reaction speed is the persona's own decision cadence — Easy
/// decides every 120 ticks against Normal's 30, so Easy simply
/// notices the damage later, the same way it does everything
/// else (AI-03: behavior only, never incapability).
fn plan_repair(sim: &mut Sim, cfg: &AiConfig, acted: &mut Vec<u32>) {
    let me = OWNER_OPPONENT;
    if sim.stockpile_of(me).amount(ResourceKind::Wood) < REPAIR_MIN_WOOD {
        return;
    }
    // Repairers already on duty: they hold their assignments
    // and count against the cap — Town Center repairers
    // (Package J) included, one shared cap for all repair work.
    let mut total: u32 = 0;
    for e in sim.entities() {
        if e.owner != me || e.dead {
            continue;
        }
        if matches!(e.task, Task::Repair { .. } | Task::RepairTc { .. }) {
            total += 1;
        }
    }
    if total >= cfg.repairers_max {
        return;
    }
    // Package J (BLD-17 parity): the AI's own Town Center is a
    // repair target through `Command::RepairTc` — the TC is an
    // entity in this architecture, so it rides its own command,
    // under the same caps, wood floor and cadence as every other
    // target here. It is filled FIRST (see the step's doc: the
    // head of the production-first order).
    if let Some(tc) = sim.tc_entity_of(me) {
        let max_hp = sim.stats_for(me, EntityKind::TownCenter).max_hp;
        if !tc.dead && tc.hp < I16F16::from_num(max_hp) {
            let (tx, ty, tc_id) = (tc.pos_x, tc.pos_y, tc.id);
            while total < cfg.repairers_max {
                let Some(vid) = nearest_repair_candidate(sim, tx, ty, acted) else {
                    break;
                };
                if !sim.can_repair_tc(vid, tc_id) {
                    break;
                }
                sim.push_command(Command::RepairTc {
                    entity: vid,
                    tc: tc_id,
                });
                acted.push(vid);
                total += 1;
            }
        }
    }
    if total >= cfg.repairers_max {
        return;
    }
    // Damaged targets in priority order. The key: production
    // first, under attack next, then lowest HP fraction (per
    // mille, integer — decisions stay float-free), then id.
    let mut targets: Vec<(u8, u8, i64, u32)> = Vec::new();
    for b in sim.buildings() {
        if b.owner != me || !b.complete || b.kind == BuildingKind::Farm {
            continue;
        }
        let max_hp = sim.building_max_hp(b.kind, me);
        if b.hp >= I16F16::from_num(max_hp) {
            continue;
        }
        let production = if b.kind.is_production() { 0 } else { 1 };
        let attacked = if building_under_attack(sim, b.pos_x, b.pos_y) {
            0
        } else {
            1
        };
        let fraction = b.hp.to_bits() as i64 * 1000 / max_hp as i64;
        targets.push((production, attacked, fraction, b.id));
    }
    targets.sort();
    for (_, _, _, bid) in targets {
        if total >= cfg.repairers_max {
            break;
        }
        let (bx, by) = match sim.buildings().iter().find(|b| b.id == bid) {
            Some(b) => (b.pos_x, b.pos_y),
            None => continue,
        };
        // Fill this building before moving to the next: stacked
        // repairers are how a building under fire is held (375
        // HP/min each past the first, BLD-07), and one mended
        // building beats two half-mended ones.
        while total < cfg.repairers_max {
            let Some(vid) = nearest_repair_candidate(sim, bx, by, acted) else {
                break;
            };
            if !sim.can_repair(vid, bid) {
                // The building side failed validation (the
                // candidate side cannot differ between villagers:
                // each is a living own villager by construction)
                // — do not spin on it for the next candidate.
                break;
            }
            sim.push_command(Command::Repair {
                entity: vid,
                building: bid,
            });
            acted.push(vid);
            total += 1;
        }
    }
}

/// Whether an enemy soldier the AI can currently *see* stands
/// within the defense ring of a point — the live-state reading
/// of "under attack": a raider next to the building, inside the
/// same 10-tile ring `plan_defense` guards around the Town
/// Center. Fog applies (VIS-01): an attacker the AI cannot see
/// does not raise the building's priority — its damage alone
/// still queues it, just behind the ones visibly under fire.
fn building_under_attack(sim: &Sim, bx: I16F16, by: I16F16) -> bool {
    let me = OWNER_OPPONENT;
    sim.entities().iter().any(|e| {
        if e.owner != OWNER_PLAYER || e.dead || !e.kind.is_military() {
            return false;
        }
        if steer::vec_len(e.pos_x - bx, e.pos_y - by) > DEFENSE_RADIUS {
            return false;
        }
        let (cx, cy) = sim.grid().world_to_cell(e.pos_x, e.pos_y);
        sim.cell_visible_to(me, cx, cy)
    })
}

/// The nearest villager that may take a repair order for a
/// building at (bx, by): idle villagers first, then gatherers
/// (the construction step's fallback — pulling one worker off
/// the line is what a player does; a hunter mid-chase or a
/// builder mid-placement is never pulled). Nearest by squared
/// distance, ties by ascending id; `acted` excludes villagers
/// already booked this decision.
fn nearest_repair_candidate(
    sim: &Sim,
    bx: I16F16,
    by: I16F16,
    acted: &[u32],
) -> Option<u32> {
    let me = OWNER_OPPONENT;
    let mut best: Option<(i64, u32)> = None;
    for pass in 0..2 {
        for e in sim.entities() {
            if e.owner != me || e.kind != EntityKind::Villager || e.dead {
                continue;
            }
            if acted.contains(&e.id) {
                continue;
            }
            let eligible = if pass == 0 {
                is_idle(sim, e.id)
            } else {
                e.task == Task::Gathering && e.attack_target.is_none()
            };
            if !eligible {
                continue;
            }
            let d = steer::vec_len(e.pos_x - bx, e.pos_y - by);
            let db = d.to_bits() as i64;
            let key = (db * db, e.id);
            if best.map(|b| key < b).unwrap_or(true) {
                best = Some(key);
            }
        }
        if best.is_some() {
            break;
        }
    }
    best.map(|(_, id)| id)
}

/// Construction step: one placement per decision, placed by the
/// first free villager at the first legal anchor the ring search
/// offers (`Sim::can_build` is the sim's own validation — the AI
/// plays by the placement rules too). Priority: the Archery
/// Range once Feudal stands (it outranks housing — see the chain
/// below) → pop-cap pressure (house) → the Mill by the berries →
/// the Lumber Camp by the woodline → the Barracks → a house built
/// ahead while there is still slack for one more. (M6: the Mill
/// and the Lumber Camp come before the Barracks — they are the
/// AI's Feudal pair, and their drop-offs shorten every haul the
/// script makes.)
fn plan_build(sim: &mut Sim, cfg: &AiConfig, acted: &mut Vec<u32>) {
    let me = OWNER_OPPONENT;
    let Some((_, tc_x, tc_y)) = sim.tc_entity_of(me).map(|tc| (tc.id, tc.pos_x, tc.pos_y)) else {
        return;
    };
    let pop = sim.population_of(me);
    let cap = sim.population_cap_of(me);
    let wood = sim.stockpile_of(me).amount(ResourceKind::Wood);
    let villagers = count_kind(sim, me, EntityKind::Villager);
    let have = |kind: BuildingKind| {
        sim.buildings()
            .iter()
            .any(|b| b.owner == me && b.kind == kind)
    };
    let house_in_progress = sim
        .buildings()
        .iter()
        .any(|b| b.owner == me && b.kind == BuildingKind::House && !b.complete);
    // (kind, anchor center): camps anchor near their resource, the
    // rest around the Town Center. The Archery Range outranks even
    // house pressure: it is age-gated (inert in Dark Age), and at
    // the population cap the house clause would otherwise starve
    // it forever — the army holds the cap full once Feudal lands.
    let want: Option<(BuildingKind, (I16F16, I16F16))> =
        if !have(BuildingKind::ArcheryRange)
            && sim.age_of(me) >= crate::tech::Age::Feudal
            && sim.completed_buildings(me, BuildingKind::Barracks) > 0
            && wood >= BuildingKind::ArcheryRange.wood_cost()
        {
            Some((BuildingKind::ArcheryRange, (tc_x, tc_y)))
        } else if !have(BuildingKind::Blacksmith)
            && sim.age_of(me) >= crate::tech::Age::Castle
            && wood >= BuildingKind::Blacksmith.wood_cost()
        {
            // Castle P4a (AI-04 extension): the Blacksmith is the
            // Siege Workshop's prerequisite (BLD-15) — a Castle-Age
            // AI raises it on the way to siege, like a player.
            Some((BuildingKind::Blacksmith, (tc_x, tc_y)))
        } else if !have(BuildingKind::SiegeWorkshop)
            && sim.age_of(me) >= crate::tech::Age::Castle
            && sim.completed_buildings(me, BuildingKind::Blacksmith) > 0
            && wood >= BuildingKind::SiegeWorkshop.wood_cost()
        {
            // Castle P4a: the siege program's home — without it
            // the AI fields no Rams and razes nothing in Castle.
            Some((BuildingKind::SiegeWorkshop, (tc_x, tc_y)))
        } else if pop + 1 >= cap && !house_in_progress && wood >= BuildingKind::House.wood_cost() {
            Some((BuildingKind::House, (tc_x, tc_y)))
        } else if !have(BuildingKind::Mill)
            && villagers >= 5
            && wood >= BuildingKind::Mill.wood_cost()
        {
            let center = nearest_node_pos(sim, crate::econ::NodeKind::BerryBush, tc_x, tc_y)
                .unwrap_or((tc_x, tc_y));
            Some((BuildingKind::Mill, center))
        } else if !have(BuildingKind::LumberCamp)
            && villagers >= 6
            && wood >= BuildingKind::LumberCamp.wood_cost()
        {
            let center = nearest_node_pos(sim, crate::econ::NodeKind::Tree, tc_x, tc_y)
                .unwrap_or((tc_x, tc_y));
            Some((BuildingKind::LumberCamp, center))
        } else if !have(BuildingKind::Barracks)
            && villagers >= cfg.barracks_min_villagers
            && wood >= BuildingKind::Barracks.wood_cost()
        {
            Some((BuildingKind::Barracks, (tc_x, tc_y)))
        } else if pop + 2 >= cap && !house_in_progress && wood >= BuildingKind::House.wood_cost() {
            Some((BuildingKind::House, (tc_x, tc_y)))
        } else {
            None
        };
    let Some((kind, (cx, cy))) = want else {
        return;
    };
    // Builder: the first idle villager — or, when the whole
    // village is at work (the steady state once the deposit →
    // resume loop closes: nobody is ever idle at a decision
    // boundary, and construction would starve), the first villager
    // standing at a node gathering. Pulling one worker off the
    // line is what a player does; interrupting a gatherer costs
    // nothing but the walk.
    let builder = sim
        .entities()
        .iter()
        .find(|e| {
            e.owner == me
                && e.kind == EntityKind::Villager
                && is_idle(sim, e.id)
                && !acted.contains(&e.id)
        })
        .or_else(|| {
            sim.entities().iter().find(|e| {
                e.owner == me
                    && e.kind == EntityKind::Villager
                    && !e.dead
                    && e.task == Task::Gathering
                    && e.attack_target.is_none()
                    && !acted.contains(&e.id)
            })
        })
        .map(|e| e.id);
    let Some(builder) = builder else {
        return;
    };
    if let Some((x, y)) = find_anchor(sim, builder, kind, cx, cy) {
        sim.push_command(Command::Build {
            entity: builder,
            kind,
            x,
            y,
        });
        acted.push(builder);
    }
}

/// Placement search: anchors (footprint top-left cells) on expanding
/// Chebyshev rings around the given center's cell, in fixed order
/// (ring, row, column), each validated by the sim's own
/// `can_build`. Bounded — if nothing legal shows up within the
/// search, the AI simply tries again next decision.
fn find_anchor(
    sim: &Sim,
    builder: u32,
    kind: BuildingKind,
    cx: I16F16,
    cy: I16F16,
) -> Option<(I16F16, I16F16)> {
    let (tcx, tcy) = sim.grid().world_to_cell(cx, cy);
    let size = kind.footprint_size();
    let mut tried = 0;
    for r in 3..=9i32 {
        for dy in -r..=r {
            for dx in -r..=r {
                if dx.abs() != r && dy.abs() != r {
                    continue;
                }
                let (ax, ay) = (tcx as i32 + dx, tcy as i32 + dy);
                if ax < 0 || ay < 0 {
                    continue;
                }
                let (ax, ay) = (ax as u32, ay as u32);
                let (x, y) = crate::build::footprint_center(ax, ay, size);
                tried += 1;
                // The cap sits far out on purpose: a grown base
                // crowds the inner rings with buildings and with
                // units a placement may not cover, and a 3×3
                // footprint may only fit several rings out. (At 48
                // the Archery Range was never placed at all once
                // the base filled in.)
                if tried > 512 {
                    return None;
                }
                if sim.can_build(builder, kind, x, y) {
                    return Some((x, y));
                }
            }
        }
    }
    None
}

/// The expected enemy start (M7): the rotational mirror of the
/// AI's own Town Center cell — the one fact Arabia's symmetry
/// hands every player about the enemy base. Used for the rally
/// staging point, the Scout's first ride, and a wave that has not
/// seen anything yet. Deliberately computed from the AI's *own*
/// state, never from the human's records.
fn expected_enemy_start(sim: &Sim) -> Option<(I16F16, I16F16)> {
    let tc = sim.tc_entity_of(OWNER_OPPONENT)?;
    let (cx, cy) = sim.grid().world_to_cell(tc.pos_x, tc.pos_y);
    let (mx, my) = (crate::GRID_DIM - 1 - cx, crate::GRID_DIM - 1 - cy);
    Some(sim.grid().cell_center(mx, my))
}

/// Scouting (M7): while the enemy start's ground is still
/// unexplored on the AI's map, an idle Scout is sent there — the
/// fog rules mean the wave step cannot aim at a base it has never
/// seen. Once the ground is explored the Scout is left to the
/// ordinary steps (it defends like any soldier when home).
fn plan_scout(sim: &mut Sim, acted: &mut Vec<u32>) {
    let me = OWNER_OPPONENT;
    let Some((ex, ey)) = expected_enemy_start(sim) else {
        return;
    };
    let (cx, cy) = sim.grid().world_to_cell(ex, ey);
    if sim.cell_explored_by(me, cx, cy) {
        return;
    }
    let scout = sim
        .entities()
        .iter()
        .find(|e| {
            e.owner == me
                && e.kind == EntityKind::ScoutCavalry
                && is_idle(sim, e.id)
                && !acted.contains(&e.id)
        })
        .map(|e| e.id);
    if let Some(sid) = scout {
        sim.push_command(Command::Move {
            entity: sid,
            x: ex,
            y: ey,
        });
        acted.push(sid);
    }
}

/// Production rally: set once per production building (Barracks,
/// Archery Range), when it stands complete and has no rally yet —
/// toward the expected enemy start, stopping short of it (the
/// staging point fresh troops gather at).
fn plan_rally(sim: &mut Sim) {
    let me = OWNER_OPPONENT;
    let Some((hx, hy)) = expected_enemy_start(sim) else {
        return;
    };
    let Some(building) = sim.buildings().iter().find(|b| {
        b.owner == me && b.kind.is_production() && b.complete && b.rally.is_none()
    }) else {
        return;
    };
    let bid = building.id;
    let (bx, by) = (building.pos_x, building.pos_y);
    let (dx, dy) = (bx - hx, by - hy);
    let dist = steer::vec_len(dx, dy);
    let short = I16F16::from_num(12);
    let (x, y) = if dist > short {
        let k = (dist - short) / dist;
        (hx + dx * k, hy + dy * k)
    } else {
        (hx, hy)
    };
    sim.push_command(Command::SetRally {
        building: Some(bid),
        x,
        y,
    });
}

/// Military production step (see `decide`).
fn plan_train_military(sim: &mut Sim, cfg: &AiConfig) {
    let me = OWNER_OPPONENT;
    // Saving for the Feudal Age (M6): once Loom is in, the
    // requirement pair stands and the village is nearly grown,
    // military production pauses until the advancement is paid
    // for — the same discipline `plan_research` assumes. Without
    // the pause, the army's steady food draw keeps the stockpile
    // under the age's 500-food cost forever and the AI never
    // ages up at all. The pause starts only once a minimal guard
    // stands (the starting Scout and three trained soldiers):
    // the first soldiers are not the age-up's competition, they
    // are its escort.
    if sim.age_of(me) == crate::tech::Age::Dark
        && sim.has_tech(me, crate::tech::TechId::Loom)
        && military(sim, me).len() >= 4
    {
        let qualifying = sim
            .buildings()
            .iter()
            .filter(|b| b.owner == me && b.complete && b.kind.counts_toward_feudal())
            .count();
        let villagers = count_kind(sim, me, EntityKind::Villager);
        if qualifying >= 2 && villagers + 2 >= cfg.villager_target {
            let (cost, _, _, _) = crate::tech::TechId::FeudalAge.cost();
            if sim.stockpile_of(me).amount(ResourceKind::Food) < I16F16::from_num(cost + 50) {
                return;
            }
        }
    }
    // Siege (Castle P4a, AI-04 extension): in the Castle Age a
    // standing Siege Workshop keeps a small ram corps alongside
    // the composition army — Rams are not a MIX kind (they are
    // siege, not a share of the foot army), so they are counted
    // and produced on their own line: living plus queued, capped
    // at RAM_TARGET, one enqueue at a time like every other host.
    if sim.age_of(me) >= crate::tech::Age::Castle {
        let rams = count_kind(sim, me, EntityKind::BatteringRam)
            + sim
                .buildings()
                .iter()
                .filter(|b| b.owner == me)
                .flat_map(|b| b.queue.iter())
                .filter(|entry| {
                    matches!(
                        entry,
                        crate::tech::ProdEntry::Unit(EntityKind::BatteringRam)
                    )
                })
                .count() as u32;
        if rams < RAM_TARGET {
            if let Some(host) = sim.buildings().iter().find(|b| {
                b.owner == me && b.complete && b.kind == BuildingKind::SiegeWorkshop
            }) {
                if host.queue.is_empty()
                    && sim.can_train_at(host.id, EntityKind::BatteringRam)
                {
                    sim.push_command(Command::TrainAt {
                        building: host.id,
                        kind: EntityKind::BatteringRam,
                    });
                    return;
                }
            }
        }
    }
    let army = military(sim, me).len() as u32;
    if army >= cfg.army_target {
        return;
    }
    // Composition (AI-04): rank the four trainable kinds by how
    // far each lags its weighted share of the army so far, and
    // train the first kind in the ranking that can actually be
    // trained right now. The counts are living plus already
    // queued, so a kind in training is not double-booked. The
    // ranking is an apportionment walk — picking the largest
    // deficit (weight × (total + 1) − count × total weight)
    // converges the standing army to the weights' ratio
    // (Normal 3:2:3:2 over Militia / Spearman / Archer /
    // Skirmisher; Easy an even 1:1:1:1) — and because a deficit
    // depends only on counts, the walk is a pure function of
    // state like every other decision here.
    let counts = composition_counts(sim, me);
    let total: i64 = counts.iter().sum::<u32>() as i64;
    let wsum: i64 = cfg.mix_weights.iter().sum::<u32>() as i64;
    let mut ranked: Vec<(i64, usize)> = (0..MIX_KINDS.len())
        .map(|i| (cfg.mix_weights[i] as i64 * (total + 1) - counts[i] as i64 * wsum, i))
        .collect();
    ranked.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
    let mut candidates: Vec<EntityKind> = Vec::new();
    // The adaptive answer (Normal only): an enemy archer mass
    // the AI can currently *see* — fog rules, exactly as in the
    // defense and wave steps — puts the Skirmisher, the
    // anti-archer unit (+3 vs Archers, 3 pierce armor), at the
    // head of the ranking for as long as the mass stays
    // visible. The scan reads nothing a player at the seat
    // could not see: the same `cell_visible_to` test every
    // targeting decision in this script already obeys
    // (VIS-01). No information access is widened — before this
    // change the script's combat steps already read visible
    // enemy units; production now does too, on adaptive
    // profiles only.
    if cfg.adaptive && visible_archer_mass(sim, me) {
        candidates.push(EntityKind::Skirmisher);
    }
    for (_, i) in ranked {
        let kind = MIX_KINDS[i];
        if !candidates.contains(&kind) {
            candidates.push(kind);
        }
    }
    // Each kind trains at its own home (Barracks → Militia and
    // Spearman, Archery Range → Archer and Skirmisher); the
    // host's queue must be empty, as before — one production
    // line per building, kept busy one enqueue at a time.
    for kind in candidates {
        let Some(host) = sim.buildings().iter().find(|b| {
            b.owner == me && b.complete && b.kind.trains(kind)
        }) else {
            continue;
        };
        if !host.queue.is_empty() {
            continue;
        }
        if sim.can_train_at(host.id, kind) {
            sim.push_command(Command::TrainAt {
                building: host.id,
                kind,
            });
            return;
        }
    }
}

/// The Battering Ram corps the siege step keeps (Castle P4a):
/// two rams — enough to crack a building line, few enough that
/// the composition army still gets its wood and gold.
const RAM_TARGET: u32 = 2;

/// The four kinds the composition plan trains, in weight order
/// (`AiConfig::mix_weights` indexes this array).
const MIX_KINDS: [EntityKind; 4] = [
    EntityKind::Militia,
    EntityKind::Spearman,
    EntityKind::Archer,
    EntityKind::Skirmisher,
];

/// Living plus queued counts of the composition kinds for
/// `owner`: the army the composition plan is apportioning.
/// Queued units count — a kind already in training at its
/// building is part of the army being built, and counting it
/// is what stops the walk from double-booking the same share
/// while the first unit is still on the production line. The
/// Scout is not a composition kind (a starting unit, never
/// produced), and neither are villagers.
fn composition_counts(sim: &Sim, owner: u8) -> [u32; 4] {
    let mut counts = [0u32; 4];
    for e in sim.entities() {
        if e.owner != owner || e.dead {
            continue;
        }
        if let Some(i) = MIX_KINDS.iter().position(|k| *k == e.kind) {
            counts[i] += 1;
        }
    }
    for b in sim.buildings() {
        if b.owner != owner {
            continue;
        }
        for entry in &b.queue {
            if let crate::tech::ProdEntry::Unit(kind) = entry {
                if let Some(i) = MIX_KINDS.iter().position(|k| k == kind) {
                    counts[i] += 1;
                }
            }
        }
    }
    counts
}

/// Whether the enemy army the AI can currently see is an
/// archer mass: at least three living enemy Archers on cells
/// visible to the AI, and Archers the majority of the visible
/// enemy military. Fog-honest (VIS-01) — the same visibility
/// test the defense and wave steps aim with; an archer the AI
/// cannot see does not exist for this decision, and no other
/// fact about the enemy is read.
fn visible_archer_mass(sim: &Sim, me: u8) -> bool {
    let mut archers = 0u32;
    let mut total = 0u32;
    for e in sim.entities() {
        if e.owner != OWNER_PLAYER || e.dead || !e.kind.is_military() {
            continue;
        }
        let (cx, cy) = sim.grid().world_to_cell(e.pos_x, e.pos_y);
        if !sim.cell_visible_to(me, cx, cy) {
            continue;
        }
        total += 1;
        if e.kind == EntityKind::Archer {
            archers += 1;
        }
    }
    archers >= 3 && archers * 2 > total
}

/// Research step (M6): the script uses the same Town Center
/// research queue as a player. Research is only ever enqueued
/// onto an *empty* queue, so it never jumps the villager line.
/// Loom comes first, once the village is big enough to spare the
/// gold; the Feudal advancement follows when the two requirement
/// buildings stand, the village is nearly grown, and a food spare
/// exists beyond the age's cost — the same discipline the opening
/// spec asks of a player.
fn plan_research(sim: &mut Sim, cfg: &AiConfig) {
    let me = OWNER_OPPONENT;
    let Some((tc_id, _, _)) = sim.tc_entity_of(me).map(|tc| (tc.id, tc.pos_x, tc.pos_y)) else {
        return;
    };
    if !sim.tc_queue_empty(tc_id) {
        return;
    }
    let villagers = count_kind(sim, me, EntityKind::Villager);
    if !sim.has_tech(me, crate::tech::TechId::Loom) {
        let (_, _, gold, _) = crate::tech::TechId::Loom.cost();
        if villagers >= 8
            && sim.stockpile_of(me).amount(ResourceKind::Gold) >= I16F16::from_num(gold)
        {
            sim.push_command(Command::ResearchTcAt {
                tc: tc_id,
                tech: crate::tech::TechId::Loom,
            });
        }
        return;
    }
    if sim.age_of(me) == crate::tech::Age::Dark {
        let qualifying = sim
            .buildings()
            .iter()
            .filter(|b| b.owner == me && b.complete && b.kind.counts_toward_feudal())
            .count();
        let food = sim.stockpile_of(me).amount(ResourceKind::Food);
        if qualifying >= 2 && villagers + 2 >= cfg.villager_target && food >= I16F16::from_num(550) {
            sim.push_command(Command::ResearchTcAt {
                tc: tc_id,
                tech: crate::tech::TechId::FeudalAge,
            });
        }
    }
}

/// Villager assignment step: for every idle villager (ascending
/// id), pick the resource with the largest unfilled worker target
/// (food wins ties; beyond all targets, food) and send it to the
/// nearest live node of that resource to its Town Center. Food
/// special case: with no food source within reach of the Town
/// Center, up to two villagers at a time are sent to hunt the AI's
/// own nearest sheep instead — the kill leaves a carcass at the
/// Town Center's doorstep and the killer starts harvesting it
/// (ECO-13's auto-harvest), after which the node search above
/// finds the carcass on its own.
fn assign_idle_villagers(sim: &mut Sim, cfg: &AiConfig, acted: &mut Vec<u32>) {
    let me = OWNER_OPPONENT;
    let Some(tc) = sim.tc_entity_of(me) else {
        return;
    };
    let (tc_x, tc_y) = (tc.pos_x, tc.pos_y);
    // Current allocation: villagers working a live node, by the
    // node's resource; a villager hunting an owned sheep counts as
    // food.
    let mut counts = [0u32; 3]; // food, wood, gold
    for e in sim.entities() {
        if e.owner != me || e.kind != EntityKind::Villager || e.dead {
            continue;
        }
        if e.task != Task::Idle {
            if let Some(nid) = e.task_node {
                if let Some(n) = sim.nodes().iter().find(|n| n.id == nid) {
                    if !n.is_depleted() {
                        counts[resource_slot(n.resource())] += 1;
                    }
                }
            }
        }
        if let Some(t) = e.attack_target {
            if sim
                .entities()
                .iter()
                .find(|x| x.id == t)
                .map(|x| x.kind == EntityKind::Sheep && x.owner == me)
                .unwrap_or(false)
            {
                counts[0] += 1;
            }
        }
    }
    let targets = [cfg.food_workers, cfg.wood_workers, cfg.gold_workers];
    // Food near *home*: the nearest own-side food source within
    // reach (the raw node scan would also see the human's berries
    // across the gap and wrongly report food as available).
    let food_near = nearest_node(sim, ResourceKind::Food, tc_x, tc_y)
        .and_then(|nid| sim.nodes().iter().find(|n| n.id == nid))
        .map(|n| steer::vec_len(n.pos_x - tc_x, n.pos_y - tc_y) <= FOOD_SEARCH_RADIUS)
        .unwrap_or(false);
    let idle: Vec<u32> = sim
        .entities()
        .iter()
        .filter(|e| {
            e.owner == me
                && e.kind == EntityKind::Villager
                && is_idle(sim, e.id)
                && !acted.contains(&e.id)
        })
        .map(|e| e.id)
        .collect();
    for vid in idle {
        // Largest deficit wins; ties resolve food → wood → gold by
        // scan order (strictly-greater replaces).
        let mut slot = 0usize;
        let mut best_deficit = i64::MIN;
        for s in 0..3 {
            let deficit = targets[s] as i64 - counts[s] as i64;
            if deficit > best_deficit {
                best_deficit = deficit;
                slot = s;
            }
        }
        let resource = [ResourceKind::Food, ResourceKind::Wood, ResourceKind::Gold][slot];
        // Sheep hunt: food wanted, no food source near home, and
        // the AI owns a sheep — send this villager at the nearest
        // one (at most two hunters at once).
        if resource == ResourceKind::Food && !food_near {
            let hunters = sim
                .entities()
                .iter()
                .filter(|e| {
                    e.owner == me
                        && e.kind == EntityKind::Villager
                        && e.attack_target
                            .map(|t| {
                                sim.entities()
                                    .iter()
                                    .find(|x| x.id == t)
                                    .map(|x| x.kind == EntityKind::Sheep)
                                    .unwrap_or(false)
                            })
                            .unwrap_or(false)
                })
                .count();
            if hunters < 2 {
                // Reachability and distance are measured from the
                // hunter itself, not the Town Center (M6): the
                // sheep it can actually hunt is the one *it* can
                // path to.
                let hunter_pos = sim
                    .entities()
                    .iter()
                    .find(|e| e.id == vid)
                    .map(|e| (e.pos_x, e.pos_y));
                if let Some((hx, hy)) = hunter_pos {
                    if let Some(sheep) = nearest_owned_sheep(sim, me, hx, hy) {
                        sim.push_command(Command::Attack {
                            entity: vid,
                            target: sheep,
                        });
                        acted.push(vid);
                        counts[0] += 1;
                        continue;
                    }
                    // No sheep of its own yet (M7): go and get one.
                    // The start's herds stand on the generated map
                    // wherever the generator put them — a villager
                    // walking to a Gaia sheep captures it by
                    // proximity on the approach (the kill then
                    // proceeds on an owned sheep, as above). Without
                    // this step the whole food economy rides on the
                    // luck of a work route crossing a sheep: when
                    // the berries ran out first, the AI starved at
                    // the Feudal gate with full wood and gold (the
                    // M7 long-match tests caught it on seed 9).
                    if let Some(sheep) = nearest_gaia_sheep(sim, hx, hy) {
                        sim.push_command(Command::Attack {
                            entity: vid,
                            target: sheep,
                        });
                        acted.push(vid);
                        counts[0] += 1;
                        continue;
                    }
                }
            }
        }
        match nearest_node(sim, resource, tc_x, tc_y) {
            Some(nid) => {
                sim.push_command(Command::Gather {
                    entity: vid,
                    node: nid,
                });
                acted.push(vid);
                counts[slot] += 1;
            }
            None => {
                // No source of the wanted resource exists at all:
                // count the slot as served so the next villager
                // tries a different one rather than piling onto the
                // same dead end.
                counts[slot] = targets[slot];
            }
        }
    }
}

/// food/wood/gold slot of a resource (stone is never assigned in
/// v1 — nothing the script builds costs stone).
fn resource_slot(resource: ResourceKind) -> usize {
    match resource {
        ResourceKind::Food => 0,
        ResourceKind::Wood => 1,
        ResourceKind::Gold => 2,
        ResourceKind::Stone => 0,
    }
}

/// Position of the nearest live node of a kind to a point — the
/// camp anchor centers for `plan_build` (M6): a Mill goes by the
/// berries, a Lumber Camp by the woodline.
fn nearest_node_pos(
    sim: &Sim,
    kind: crate::econ::NodeKind,
    x: I16F16,
    y: I16F16,
) -> Option<(I16F16, I16F16)> {
    let mut best: Option<(i64, u32, I16F16, I16F16)> = None;
    for n in sim.nodes() {
        if n.is_depleted() || n.kind != kind {
            continue;
        }
        let d = steer::vec_len(n.pos_x - x, n.pos_y - y);
        let db = d.to_bits() as i64;
        let key = (db * db, n.id);
        // Keep the SMALLEST key (nearest). (An inverted comparison
        // here once made camps anchor at the *farthest* node of the
        // kind — at M7's map scale that is the enemy's resource
        // patch, and a builder walks the whole map to raise a Mill
        // beside it. The M7 long-match tests caught what the arena
        // era's could not.)
        if best.map(|b| key < (b.0, b.1)).unwrap_or(true) {
            best = Some((key.0, key.1, n.pos_x, n.pos_y));
        }
    }
    best.map(|(_, _, px, py)| (px, py))
}

/// The nearest live node of `resource` to a point (ties: lowest
/// id, by scan order) — restricted to the AI's own resource set by
/// *owner proximity* (M7): a node is the AI's when it is nearer
/// the AI's Town Center (the query point, always its own TC) than
/// the human's. The generated packages sit around their own
/// starts, so proximity assigns each side its own set with no
/// layout or node-id knowledge at all; neutral and dynamic nodes
/// (mid-map forests, carcasses, farms) fall to whichever Town
/// Center is nearer — where the work actually happened.
fn nearest_node(sim: &Sim, resource: ResourceKind, x: I16F16, y: I16F16) -> Option<u32> {
    let foe_tc = sim.tc_entity_of(OWNER_PLAYER);
    let mut best: Option<(i64, u32)> = None;
    for n in sim.nodes() {
        if n.is_depleted() || n.resource() != resource {
            continue;
        }
        let mine = match foe_tc {
            Some(ftc) => {
                steer::vec_len(n.pos_x - x, n.pos_y - y)
                    <= steer::vec_len(n.pos_x - ftc.pos_x, n.pos_y - ftc.pos_y)
            }
            None => true,
        };
        if !mine {
            continue;
        }
        let d = steer::vec_len(n.pos_x - x, n.pos_y - y);
        let db = d.to_bits() as i64;
        let key = (db * db, n.id);
        if best.map(|b| key < b).unwrap_or(true) {
            best = Some(key);
        }
    }
    best.map(|(_, id)| id)
}

/// The nearest living sheep owned by `owner` to a point.
/// The nearest sheep owned by `owner` *that can be reached* from
/// (x, y) — an entombed sheep (standing in a blocked cell) is
/// skipped rather than absorbing every hunter forever (M6 fix).
/// The nearest living Gaia sheep within hunting range of
/// `(x, y)` that a hunter standing there can actually path to —
/// the capture run (M7). The range is generous (the start's far
/// sheep stand up to ~16 tiles from the Town Center) but bounded,
/// so the hunter never crosses the map for lunch: every sheep on
/// an Arabia layout belongs to one of the two starts.
fn nearest_gaia_sheep(sim: &Sim, x: I16F16, y: I16F16) -> Option<u32> {
    // The range cap, in the candidates' squared-bits units:
    // (88 world units × 65536 bits/unit)² — fits i64 comfortably.
    const HUNT_RANGE_SQ_BITS: i64 = (88 * 65536) * (88 * 65536);
    let mut candidates: Vec<(i64, u32, I16F16, I16F16)> = sim
        .entities()
        .iter()
        .filter(|e| e.kind == EntityKind::Sheep && e.owner == OWNER_GAIA && !e.dead)
        .map(|e| {
            let d = steer::vec_len(e.pos_x - x, e.pos_y - y);
            let db = d.to_bits() as i64;
            (db * db, e.id, e.pos_x, e.pos_y)
        })
        .filter(|c| c.0 <= HUNT_RANGE_SQ_BITS)
        .collect();
    candidates.sort();
    candidates
        .iter()
        .find(|c| sim.grid().find_path((x, y), (c.2, c.3)).is_some())
        .map(|c| c.1)
}

fn nearest_owned_sheep(sim: &Sim, owner: u8, x: I16F16, y: I16F16) -> Option<u32> {
    let mut candidates: Vec<(i64, u32, I16F16, I16F16)> = sim
        .entities()
        .iter()
        .filter(|e| e.kind == EntityKind::Sheep && e.owner == owner && !e.dead)
        .map(|e| {
            let d = steer::vec_len(e.pos_x - x, e.pos_y - y);
            let db = d.to_bits() as i64;
            (db * db, e.id, e.pos_x, e.pos_y)
        })
        .collect();
    candidates.sort();
    candidates
        .iter()
        .find(|c| sim.grid().find_path((x, y), (c.2, c.3)).is_some())
        .map(|c| c.1)
}

/// Defense step: an idle soldier with a human unit near the AI's
/// Town Center engages the nearest such unit *it can reach*.
/// (Soldiers already fighting, walking a rally, or out on a wave
/// are not recalled — auto-acquisition also covers part of this;
/// the explicit step exists so even the Scout, which never
/// auto-acquires, defends its home.)
fn plan_defense(sim: &mut Sim, acted: &mut Vec<u32>) {
    let me = OWNER_OPPONENT;
    let Some(tc) = sim.tc_entity_of(me) else {
        return;
    };
    let (tc_x, tc_y) = (tc.pos_x, tc.pos_y);
    for mid in military(sim, me) {
        if !is_idle(sim, mid) || acted.contains(&mid) {
            continue;
        }
        let Some((sx, sy)) = sim
            .entities()
            .iter()
            .find(|e| e.id == mid)
            .map(|e| (e.pos_x, e.pos_y))
        else {
            continue;
        };
        // A defender is a soldier *at home*: one standing twice
        // the defense radius away (the Scout on its scouting ride,
        // M7) is not recalled across the map for a home raid.
        if steer::vec_len(sx - tc_x, sy - tc_y) > DEFENSE_RADIUS * I16F16::from_num(2) {
            continue;
        }
        let mut best: Option<(i64, u32)> = None;
        for e in sim.entities() {
            if e.owner != OWNER_PLAYER || e.dead || !e.kind.counts_population() {
                continue;
            }
            let d = steer::vec_len(e.pos_x - tc_x, e.pos_y - tc_y);
            if d > DEFENSE_RADIUS {
                continue;
            }
            // Fog (M7, VIS-01): the AI defends against what it can
            // currently see — a raider outside its sight is not a
            // target it may pick, however close to the Town Center
            // it stands in truth.
            let (ecx, ecy) = sim.grid().world_to_cell(e.pos_x, e.pos_y);
            if !sim.cell_visible_to(me, ecx, ecy) {
                continue;
            }
            // Only engage targets the defender can actually reach
            // (M6 fix): an unreachable one — classically the human's
            // forward Scout standing in a plugged wall gap — would
            // otherwise re-absorb every defender on every decision,
            // starving the wave step below via `acted`.
            if sim.grid().find_path((sx, sy), (e.pos_x, e.pos_y)).is_none() {
                continue;
            }
            let db = d.to_bits() as i64;
            let key = (db * db, e.id);
            if best.map(|b| key < b).unwrap_or(true) {
                best = Some(key);
            }
        }
        if let Some((_, tid)) = best {
            sim.push_command(Command::Attack {
                entity: mid,
                target: tid,
            });
            acted.push(mid);
        }
    }
}

/// The wave step: when the cooldown has passed and at least
/// `wave_size` trained soldiers (every kind production trains:
/// Militia, Spearmen, Archers, Skirmishers) stand idle at
/// home, they all attack at once. Targeting obeys the AI's own
/// fog of war (M7, VIS-01/06): a human *unit* is a target only
/// while the AI can currently see it; the human Town Center and
/// buildings are targets once *explored* — remembered knowledge,
/// like a player's (a building razed unseen stays on the AI's map
/// until someone looks again). When the AI knows nothing yet —
/// its Scout has not reached the enemy start — the wave instead
/// marches on the expected enemy start (the mirrored Town Center
/// cell): an attack-move in spirit, executed as an ordinary Move;
/// auto-acquisition and later decisions take over on contact.
/// Among known targets each soldier picks as the script always
/// has: the nearest *reachable* visible unit; else the Town
/// Center; else the nearest explored building. The cooldown
/// restarts on launch — win or lose, the survivors' state (dead,
/// fighting, walking home via rally-less idleness) is simply
/// re-read next decision.
fn plan_wave(sim: &mut Sim, cfg: &AiConfig, acted: &mut Vec<u32>) {
    let me = OWNER_OPPONENT;
    if sim.tick_count() < sim.ai.next_wave_tick {
        return;
    }
    let wave: Vec<u32> = sim
        .entities()
        .iter()
        .filter(|e| {
            e.owner == me
                && !e.dead
                && matches!(
                    e.kind,
                    EntityKind::Militia
                        | EntityKind::Spearman
                        | EntityKind::Archer
                        | EntityKind::Skirmisher
                        // Castle P4a: Rams march with the wave —
                        // they are its building-crackers.
                        | EntityKind::BatteringRam
                )
                && is_idle(sim, e.id)
                && !acted.contains(&e.id)
        })
        .map(|e| e.id)
        .collect();
    if wave.len() < cfg.wave_size {
        return;
    }
    let human_tc = sim
        .tc_entity_of(OWNER_PLAYER)
        .filter(|tc| !tc.dead)
        .map(|tc| (tc.id, tc.pos_x, tc.pos_y));
    let tc_known = human_tc
        .map(|(_, tx, ty)| {
            let (cx, cy) = sim.grid().world_to_cell(tx, ty);
            sim.cell_explored_by(me, cx, cy)
        })
        .unwrap_or(false);
    for mid in &wave {
        let (mx, my) = sim
            .entities()
            .iter()
            .find(|e| e.id == *mid)
            .map(|e| (e.pos_x, e.pos_y))
            .unwrap_or((I16F16::ZERO, I16F16::ZERO));
        // Castle P4a: a Ram is a building weapon — it skips the
        // visible-unit hunt below and goes straight to the Town
        // Center / building targeting (the ordinary Attack and
        // AttackBuilding paths; no AI-only orders, AI-01).
        let is_ram = sim
            .entities()
            .iter()
            .find(|e| e.id == *mid)
            .map(|e| e.kind == EntityKind::BatteringRam)
            .unwrap_or(false);
        // Nearest human population unit the AI can currently see
        // *and* actually reach. A target standing inside a building
        // footprint (or behind a plugged chokepoint) has no path;
        // ordering the wave onto it fizzles every soldier at once
        // (M6 fix). Candidates are walked nearest-first; the
        // reachability probe is one A* per candidate per soldier,
        // once per wave — affordable, and deterministic like every
        // other path query.
        let mut candidates: Vec<(i64, u32, I16F16, I16F16)> = if is_ram {
            Vec::new()
        } else {
            sim.entities()
                .iter()
                .filter(|e| e.owner == OWNER_PLAYER && !e.dead && e.kind.counts_population())
                .filter(|e| {
                    let (cx, cy) = sim.grid().world_to_cell(e.pos_x, e.pos_y);
                    sim.cell_visible_to(me, cx, cy)
                })
                .map(|e| {
                    let d = steer::vec_len(e.pos_x - mx, e.pos_y - my);
                    let db = d.to_bits() as i64;
                    (db * db, e.id, e.pos_x, e.pos_y)
                })
                .collect()
        };
        candidates.sort();
        let reachable = |sim: &Sim, x: I16F16, y: I16F16| {
            sim.grid().find_path((mx, my), (x, y)).is_some()
        };
        let tid = candidates
            .iter()
            .find(|c| reachable(sim, c.2, c.3))
            .map(|c| c.1);
        if let Some(tid) = tid {
            sim.push_command(Command::Attack {
                entity: *mid,
                target: tid,
            });
            acted.push(*mid);
            continue;
        }
        // No visible units: the Town Center, once explored.
        if tc_known {
            if let Some((tc_id, tx, ty)) = human_tc {
                if reachable(sim, tx, ty) {
                    sim.push_command(Command::Attack {
                        entity: *mid,
                        target: tc_id,
                    });
                    acted.push(*mid);
                    continue;
                }
            }
        }
        // Then the nearest human building the AI has explored.
        let mut best_b: Option<(i64, u32)> = None;
        for b in sim.buildings() {
            if b.owner != OWNER_PLAYER {
                continue;
            }
            let (cx, cy) = sim.grid().world_to_cell(b.pos_x, b.pos_y);
            if !sim.cell_explored_by(me, cx, cy) {
                continue;
            }
            let d = steer::vec_len(b.pos_x - mx, b.pos_y - my);
            let db = d.to_bits() as i64;
            let key = (db * db, b.id);
            if best_b.map(|x| key < x).unwrap_or(true) {
                best_b = Some(key);
            }
        }
        if let Some((_, bid)) = best_b {
            sim.push_command(Command::AttackBuilding {
                entity: *mid,
                building: bid,
            });
            acted.push(*mid);
            continue;
        }
        // Nothing is known yet: march on the expected enemy start.
        // The soldiers' own eyes (auto-acquisition within their
        // LoS) and the next decisions do the rest.
        if let Some((ex, ey)) = expected_enemy_start(sim) {
            sim.push_command(Command::Move {
                entity: *mid,
                x: ex,
                y: ey,
            });
            acted.push(*mid);
        }
    }
    sim.ai.next_wave_tick = sim.tick_count() + cfg.wave_cooldown;
}

#[cfg(test)]
mod tests {
    //! AI repair gate tests (BLD-17): staged in-crate because the
    //! scenario needs exact control of the AI's stockpile, which
    //! the public API deliberately does not offer. The staging
    //! cheats (grants, drains, direct damage) set the scene only —
    //! every repair decision asserted on is the script's own,
    //! taken through the ordinary command path in a live AI match.
    use super::*;

    fn drain_wood(sim: &mut Sim) {
        let w = sim.stockpile_of(OWNER_OPPONENT).amount(ResourceKind::Wood);
        sim.stockpile_mut(OWNER_OPPONENT)
            .spend(ResourceKind::Wood, w);
    }

    fn set_wood(sim: &mut Sim, amount: i32) {
        drain_wood(sim);
        sim.stockpile_mut(OWNER_OPPONENT)
            .add(ResourceKind::Wood, I16F16::from_num(amount));
    }

    /// Order every AI villager onto its nearest food node (the
    /// allocator only ever reassigns the idle, so a herded
    /// village stays off the woodline until the herding stops).
    fn herd_to_food(sim: &mut Sim) {
        let me = OWNER_OPPONENT;
        let Some(tc) = sim.tc_entity_of(me) else {
            return;
        };
        let Some(nid) = nearest_node(sim, ResourceKind::Food, tc.pos_x, tc.pos_y) else {
            return;
        };
        let villagers: Vec<u32> = sim
            .entities()
            .iter()
            .filter(|e| e.owner == me && e.kind == EntityKind::Villager && !e.dead)
            .map(|e| e.id)
            .collect();
        for v in villagers {
            sim.push_command(Command::Gather {
                entity: v,
                node: nid,
            });
        }
    }

    fn wood(sim: &Sim) -> f64 {
        sim.stockpile_of(OWNER_OPPONENT)
            .amount(ResourceKind::Wood)
            .to_num::<f64>()
    }

    fn opp_building(sim: &Sim, kind: BuildingKind) -> Option<u32> {
        sim.buildings()
            .iter()
            .find(|b| b.owner == OWNER_OPPONENT && b.kind == kind && b.complete)
            .map(|b| b.id)
    }

    fn repairers_on(sim: &Sim, bid: u32) -> usize {
        sim.entities()
            .iter()
            .filter(|e| {
                e.owner == OWNER_OPPONENT && e.task == (Task::Repair { building: bid })
            })
            .count()
    }

    fn any_repairers(sim: &Sim) -> usize {
        sim.entities()
            .iter()
            .filter(|e| e.owner == OWNER_OPPONENT && matches!(e.task, Task::Repair { .. }))
            .count()
    }

    /// Stage: an AI match where two Houses and a Barracks stand
    /// complete by the opponent Town Center, raised by its three
    /// starting villagers. Each placement is granted its exact
    /// cost on the tick it is ordered and the bank is drained
    /// back to zero every tick, so the script itself places
    /// nothing and the bank reads exactly what the test leaves.
    fn staged_barracks(mode: MatchMode) -> (Sim, u32) {
        let mut sim = Sim::new_match(11, mode);
        let me = OWNER_OPPONENT;
        let tc = sim.tc_entity_of(me).expect("the AI has a Town Center");
        let (tc_x, tc_y) = (tc.pos_x, tc.pos_y);
        let villagers: Vec<u32> = sim
            .entities()
            .iter()
            .filter(|e| e.owner == me && e.kind == EntityKind::Villager && !e.dead)
            .map(|e| e.id)
            .collect();
        assert_eq!(villagers.len(), 3, "the AI starts with three villagers");
        let jobs = [
            (villagers[0], BuildingKind::House),
            (villagers[1], BuildingKind::House),
            (villagers[2], BuildingKind::Barracks),
        ];
        for (v, kind) in jobs {
            set_wood(&mut sim, kind.wood_cost().to_num::<i32>());
            let (x, y) = find_anchor(&sim, v, kind, tc_x, tc_y).expect("a legal anchor");
            sim.push_command(Command::Build {
                entity: v,
                kind,
                x,
                y,
            });
            sim.tick();
            drain_wood(&mut sim);
        }
        let mut ticks = 0u64;
        while opp_building(&sim, BuildingKind::Barracks).is_none() && ticks < 60_000 {
            sim.tick();
            drain_wood(&mut sim);
            ticks += 1;
        }
        let bid = opp_building(&sim, BuildingKind::Barracks)
            .expect("the staged Barracks never completed");
        (sim, bid)
    }

    /// Beat the staged Barracks down by `amount` HP through the
    /// combat path (armor and the minimum-1 clamp apply per hit,
    /// so loop to the target).
    fn damage(sim: &mut Sim, bid: u32, amount: i32) -> I16F16 {
        let (bx, by, max_hp) = {
            let b = sim.buildings().iter().find(|b| b.id == bid).unwrap();
            (b.pos_x, b.pos_y, sim.building_max_hp(b.kind, b.owner))
        };
        let elev = sim.height_at_world(bx, by);
        let target = I16F16::from_num(max_hp - amount);
        let mut hits = 0;
        loop {
            let hp = sim
                .buildings()
                .iter()
                .find(|b| b.id == bid)
                .unwrap()
                .hp;
            if hp <= target || hits >= 10_000 {
                return hp;
            }
            sim.damage_building(bid, &[(crate::classes::MELEE, 50)], elev, OWNER_PLAYER);
            hits += 1;
        }
    }

    /// One scenario, run for both personas: with wood in the
    /// bank the AI assigns repairers (never more than the
    /// profile cap), the Barracks returns to full, the stockpile
    /// pays while HP rises; with the bank held at zero nothing
    /// is assigned and nobody gets stuck; when wood flows again
    /// the repair resumes.
    fn ai_repair_scenario(mode: MatchMode, cap: usize) {
        let (mut sim, bid) = staged_barracks(mode);
        let max_hp = {
            let b = sim.buildings().iter().find(|b| b.id == bid).unwrap();
            I16F16::from_num(sim.building_max_hp(b.kind, b.owner))
        };

        // Phase 1 — funded repair.
        let hurt = damage(&mut sim, bid, 300);
        assert!(hurt < max_hp, "the staging damage did not land");
        set_wood(&mut sim, 50);
        let mut series: Vec<(f64, f64, usize)> = Vec::new(); // (hp, wood, buildings)
        let mut saw_repair = false;
        let mut max_on_duty = 0usize;
        let mut ticks = 0u64;
        while ticks < 12_000 {
            sim.tick();
            ticks += 1;
            let hp = sim.buildings().iter().find(|b| b.id == bid).unwrap().hp;
            saw_repair |= repairers_on(&sim, bid) > 0;
            max_on_duty = max_on_duty.max(any_repairers(&sim));
            series.push((
                hp.to_num::<f64>(),
                wood(&sim),
                sim.buildings().len(),
            ));
            if hp >= max_hp && repairers_on(&sim, bid) == 0 && ticks > 600 {
                break;
            }
        }
        let full = sim.buildings().iter().find(|b| b.id == bid).unwrap().hp;
        assert!(saw_repair, "the AI never assigned a repairer ({mode:?})");
        assert!(
            max_on_duty <= cap,
            "the AI fielded {max_on_duty} repairers over its cap of {cap} ({mode:?})"
        );
        assert_eq!(full, max_hp, "the Barracks never returned to full ({mode:?})");
        // The stockpile paid for the mending: somewhere in the
        // series HP climbs over a 120-tick stretch with no new
        // building placed (no other wood sink runs) while the
        // wood balance strictly falls — the drip, observed.
        let paid = series.windows(121).any(|w| {
            w[120].0 > w[0].0 + 5.0 && w[120].1 < w[0].1 && w[120].2 == w[0].2
        });
        assert!(paid, "no stretch of rising HP at falling wood ({mode:?})");

        // Phase 2 — the bank held at zero: no assignment, no
        // stuck villagers, HP frozen (nothing is attacking; only
        // repair could raise it). The bank alone cannot prove the
        // gate from outside a tick (a deposit lands before the
        // decide step), so the village is also herded onto food
        // for the duration: with every villager on berries and a
        // re-herd every 300 ticks, no wood trip (a ≥1 800-tick
        // round trip) can ever complete, no deposit can reach a
        // decision, and the floor is shut at every cadence.
        let _ = damage(&mut sim, bid, 200);
        herd_to_food(&mut sim);
        // Settle: any repairer an accidental first-ticks deposit
        // could have started is released by the broke-stop as the
        // drain bites; then the HP snapshot is taken.
        for _ in 0..300 {
            sim.tick();
            drain_wood(&mut sim);
        }
        let frozen = sim.buildings().iter().find(|b| b.id == bid).unwrap().hp;
        for i in 0..3_300u64 {
            if i % 300 == 0 {
                herd_to_food(&mut sim);
            }
            sim.tick();
            drain_wood(&mut sim);
            assert_eq!(
                any_repairers(&sim),
                0,
                "a repairer was assigned with an empty bank ({mode:?})"
            );
            let hp = sim.buildings().iter().find(|b| b.id == bid).unwrap().hp;
            assert_eq!(hp, frozen, "HP moved while the AI was broke ({mode:?})");
        }
        let working = sim
            .entities()
            .iter()
            .filter(|e| {
                e.owner == OWNER_OPPONENT
                    && e.kind == EntityKind::Villager
                    && !e.dead
                    && e.task != Task::Idle
            })
            .count();
        assert!(
            working > 0,
            "the whole village stood idle in the no-wood case ({mode:?})"
        );

        // Phase 3 — wood flows again (the drain stops): the gate
        // reopens and the repair resumes on the script's own
        // schedule.
        let mut resumed = false;
        for _ in 0..12_000 {
            sim.tick();
            if repairers_on(&sim, bid) > 0 {
                resumed = true;
                break;
            }
        }
        assert!(resumed, "repair never resumed once wood returned ({mode:?})");
    }

    #[test]
    fn ai_normal_repairs_and_respects_the_wood_floor() {
        ai_repair_scenario(MatchMode::AiNormal, 3);
    }

    #[test]
    fn ai_easy_repairs_too_but_capped_lower() {
        // Easy is slower to react (its 120-tick cadence against
        // Normal's 30 decides when the damage is first answered)
        // and fields fewer repairers — never incapable (AI-03).
        ai_repair_scenario(MatchMode::AiEasy, 2);
    }

    // --------------------------------------------------------------
    // Composition (AI-04): the Spearman and the Skirmisher join
    // the production plan. Staged like the repair scenarios
    // above — the scene is set with records and stockpile
    // grants, but every production decision asserted on is the
    // script's own, taken on its natural cadence through the
    // ordinary command path in a live AI match.

    /// A complete, idle building record of the AI's at a cell
    /// anchor (the shape of new_units_tests' `placed` fixture).
    fn staged_building(
        sim: &Sim,
        id: u32,
        kind: BuildingKind,
        cell: (u32, u32),
    ) -> crate::build::Building {
        let size = kind.footprint_size();
        let (x0, y0) = sim.grid().cell_center(cell.0, cell.1);
        let half = I16F16::from_num(size as i32 * 2);
        crate::build::Building {
            id,
            kind,
            owner: OWNER_OPPONENT,
            hp: I16F16::from_num(kind.max_hp()),
            cell_x: cell.0,
            cell_y: cell.1,
            pos_x: x0 + half - I16F16::from_num(2),
            pos_y: y0 + half - I16F16::from_num(2),
            progress_thirds: 3,
            repair_hp_remainder: 0,
            repair_cost_remainder: 0,
            repair_stone_remainder: 0,
            build_rate_remainder: 0,
            gate_along_x: true,
            complete: true,
            node_id: None,
            queue: Vec::new(),
            train_progress: 0,
            rally: None,
            rally_node: None,
            weapon_target: None,
            weapon_target_building: None,
            weapon_strike_tick: None,
            weapon_next_tick: 0,
            weapon_engaged: false,
            garrison: Vec::new(),
            garrison_arrows: 0,
        }
    }

    /// A cell near the AI's Town Center, clamped into the grid.
    fn near_tc_cell(sim: &Sim, dx: i32, dy: i32) -> (u32, u32) {
        let tc = sim.tc_entity_of(OWNER_OPPONENT).expect("AI Town Center");
        let (cx, cy) = sim.grid().world_to_cell(tc.pos_x, tc.pos_y);
        let x = (cx as i32 + dx).clamp(2, crate::GRID_DIM as i32 - 8);
        let y = (cy as i32 + dy).clamp(2, crate::GRID_DIM as i32 - 8);
        (x as u32, y as u32)
    }

    /// Stage a production-ready AI: Feudal age, a completed
    /// Barracks and Archery Range plus spare Houses by its Town
    /// Center, and deep stockpiles.
    fn staged_production(mode: MatchMode) -> Sim {
        let mut sim = Sim::new_match(11, mode);
        let slot = crate::player_slot(OWNER_OPPONENT).expect("opponent slot");
        sim.players[slot].age = crate::tech::Age::Feudal;
        let mut bid = 900u32;
        let mut place = |sim: &mut Sim, kind: BuildingKind, dx: i32, dy: i32| {
            let cell = near_tc_cell(sim, dx, dy);
            let b = staged_building(sim, bid, kind, cell);
            bid += 1;
            sim.buildings.push(b);
        };
        place(&mut sim, BuildingKind::Barracks, 7, 0);
        place(&mut sim, BuildingKind::ArcheryRange, 0, 7);
        for i in 0..6i32 {
            place(&mut sim, BuildingKind::House, -7 + i * 3, -7);
        }
        for res in [ResourceKind::Food, ResourceKind::Wood, ResourceKind::Gold] {
            sim.stockpile_mut(OWNER_OPPONENT)
                .add(res, I16F16::from_num(5000));
        }
        sim
    }

    /// Run the live match, harvesting each military unit the
    /// script enqueues at a production building: the queued kind
    /// is recorded in order, the queue emptied, and a living
    /// unit of the kind staged beside the building — the
    /// training completion the composition counts read — so the
    /// next decision apportions against the grown army. Stops
    /// after `want` enqueues or `cap` ticks.
    fn collect_enqueues(sim: &mut Sim, want: usize, cap: u64) -> Vec<EntityKind> {
        let mut got: Vec<EntityKind> = Vec::new();
        let mut next_id = 9500u32;
        for _ in 0..cap {
            if got.len() >= want {
                break;
            }
            sim.tick();
            let mut harvested: Vec<(EntityKind, I16F16, I16F16)> = Vec::new();
            for b in sim.buildings.iter_mut() {
                if b.owner != OWNER_OPPONENT || !b.kind.is_production() {
                    continue;
                }
                let (bx, by) = (b.pos_x, b.pos_y);
                for entry in b.queue.drain(..) {
                    if let crate::tech::ProdEntry::Unit(kind) = entry {
                        harvested.push((kind, bx, by));
                    }
                }
                b.train_progress = 0;
            }
            for (kind, bx, by) in harvested {
                got.push(kind);
                sim.entities.push(crate::entity::Entity::spawned(
                    next_id,
                    kind,
                    OWNER_OPPONENT,
                    bx + I16F16::from_num(6),
                    by,
                ));
                next_id += 1;
            }
        }
        got
    }

    #[test]
    fn ai_normal_composition_follows_the_profile_weights() {
        let mut sim = staged_production(MatchMode::AiNormal);
        // The apportionment walk over weights 3:2:3:2 from an
        // empty army: each step trains the kind furthest behind
        // its share (ties in Militia → Spearman → Archer →
        // Skirmisher order). Nine enqueues fill the army target
        // — the tenth slot is the starting Scout.
        let got = collect_enqueues(&mut sim, 9, 6_000);
        assert_eq!(
            got,
            vec![
                EntityKind::Militia,
                EntityKind::Archer,
                EntityKind::Spearman,
                EntityKind::Skirmisher,
                EntityKind::Militia,
                EntityKind::Archer,
                EntityKind::Spearman,
                EntityKind::Skirmisher,
                EntityKind::Militia,
            ]
        );
    }

    #[test]
    fn ai_easy_composition_rotates_evenly() {
        let mut sim = staged_production(MatchMode::AiEasy);
        // Weights 1:1:1:1 walk the four kinds in turn; four
        // enqueues fill the Easy army target (Scout included).
        let got = collect_enqueues(&mut sim, 4, 6_000);
        assert_eq!(
            got,
            vec![
                EntityKind::Militia,
                EntityKind::Spearman,
                EntityKind::Archer,
                EntityKind::Skirmisher,
            ]
        );
    }

    /// Spawn `n` living human Archers on cells a few tiles from
    /// the AI's Town Center — an archer mass standing where the
    /// AI can genuinely see it, inside its Town Center's sight.
    fn spawn_archer_mass(sim: &mut Sim, n: u32) {
        let tc = sim.tc_entity_of(OWNER_OPPONENT).expect("AI Town Center");
        let (tcx, tcy) = sim.grid().world_to_cell(tc.pos_x, tc.pos_y);
        for i in 0..n {
            let cell = (
                (tcx as i32 + 4 + i as i32).clamp(0, crate::GRID_DIM as i32 - 1) as u32,
                (tcy as i32 + 4).clamp(0, crate::GRID_DIM as i32 - 1) as u32,
            );
            let (x, y) = sim.grid().cell_center(cell.0, cell.1);
            sim.entities.push(crate::entity::Entity::spawned(
                9700 + i,
                EntityKind::Archer,
                OWNER_PLAYER,
                x,
                y,
            ));
        }
        // The staging is only meaningful if the AI can actually
        // see the mass — pin that, so a fog change fails here
        // rather than silently voiding the scenario.
        assert!(
            visible_archer_mass(sim, OWNER_OPPONENT),
            "the staged archer mass is not visible to the AI"
        );
    }

    #[test]
    fn ai_normal_answers_a_visible_archer_mass_with_skirmishers() {
        let mut sim = staged_production(MatchMode::AiNormal);
        spawn_archer_mass(&mut sim, 5);
        // The base walk would open with a Militia; the visible
        // archer mass promotes the counter — the Skirmisher —
        // to the very first enqueue instead.
        let got = collect_enqueues(&mut sim, 1, 2_000);
        assert_eq!(got, vec![EntityKind::Skirmisher]);
    }

    #[test]
    fn ai_easy_does_not_adapt_its_mix() {
        let mut sim = staged_production(MatchMode::AiEasy);
        spawn_archer_mass(&mut sim, 5);
        // Easy is not adaptive: the same visible mass leaves its
        // flat rotation untouched — the first enqueue is the
        // rotation's Militia, not the counter.
        let got = collect_enqueues(&mut sim, 1, 2_000);
        assert_eq!(got, vec![EntityKind::Militia]);
    }
}
