//! The scripted AI opponent (M5) — RULEBOOK §15's v1 subset.
//!
//! The AI is a *player*, not a cheat (AI-01/AI-02): it owns the
//! opponent seat (`OWNER_OPPONENT`) with its own stockpile, Town
//! Center, population and buildings, and everything it does goes
//! through the same `Command` queue the human's client uses —
//! `TrainVillagerAt`, `Gather`, `Build`, `TrainAt`, `SetRally`,
//! `Attack`, `AttackBuilding`. It reads only sim state (never the
//! human's hidden anything — there is no fog yet, VIS rules are a
//! later milestone, and the script uses nothing a player at the
//! same screen could not see: positions, counts, stockpiles of its
//! own seat) and it never draws from the RNG — its decisions are
//! pure functions of state, in fixed iteration order (ascending
//! entity/building/node id), so a match replays bit-identically
//! from (seed, mode, human commands) alone.
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
//!    train a mixed Militia/Archer army — each kind at its own
//!    building — up to the army target.
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
use crate::entity::{EntityKind, Task, OWNER_OPPONENT, OWNER_PLAYER};
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
        },
        MatchMode::AiNormal | MatchMode::Sandbox => AiConfig {
            cadence: 30,
            villager_target: 16,
            food_workers: 7,
            wood_workers: 5,
            gold_workers: 3,
            barracks_min_villagers: 8,
            army_target: 10,
            wave_size: 6,
            wave_cooldown: 3600,
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
/// Center before idle soldiers engage it in defense. Sized to the
/// arena: the two Town Centers stand ~17 units apart and each
/// side's *starting Scout* stands ~10 from the other's Town
/// Center, so the radius must stay under that — 8 covers the Town
/// Center and the inner resource lines while keeping defenders on
/// their own half of the map. (An early larger value had the AI's
/// Scout raiding the human's base from the first minute, which is
/// an attack, not a defense.)
const DEFENSE_RADIUS: I16F16 = I16F16::from_bits(8 * 65536);
/// How far from its Town Center the AI looks for a food source
/// before it starts hunting its own sheep instead.
const FOOD_SEARCH_RADIUS: I16F16 = I16F16::from_bits(20 * 65536);

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

    // 2. Construction (at most one placement per decision): houses
    //    ahead of the cap first, then the economy buildings.
    plan_build(sim, &cfg, &mut acted);

    // 2.5 Research (M6): Loom, then the Feudal advancement — only
    //     ever onto an *empty* Town Center queue, so research
    //     never jumps the villager line (or delays it by more than
    //     its own duration once started).
    plan_research(sim, &cfg);

    // 3. Production-building rally: once each, toward a staging
    //    point short of the human's Town Center, so fresh troops
    //    walk toward the front.
    plan_rally(sim);

    // 4. Military production: keep the producing building's queue
    //    busy (one enqueue per decision) toward the army target,
    //    alternating Militia and Archers so the mix — and the
    //    resource draw — stays balanced; each kind is trained at
    //    its M6 home (Militia at the Barracks, Archers at the
    //    Archery Range). If the preferred kind cannot be trained,
    //    try the other before giving up.
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

/// Production rally: set once per production building (Barracks,
/// Archery Range), when it stands complete and has no rally yet —
/// toward the human's Town Center, stopping short of it (the
/// staging point fresh troops gather at).
fn plan_rally(sim: &mut Sim) {
    let me = OWNER_OPPONENT;
    let Some(human_tc) = sim.tc_entity_of(OWNER_PLAYER) else {
        return;
    };
    let (hx, hy) = (human_tc.pos_x, human_tc.pos_y);
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
    let army = military(sim, me).len() as u32;
    if army >= cfg.army_target {
        return;
    }
    let preferred = if army % 2 == 0 {
        EntityKind::Militia
    } else {
        EntityKind::Archer
    };
    let fallback = if preferred == EntityKind::Militia {
        EntityKind::Archer
    } else {
        EntityKind::Militia
    };
    // Each kind trains at its M6 home (Barracks → Militia, Archery
    // Range → Archer); the host's queue must be empty, as before —
    // one production line per building, kept busy one enqueue at a
    // time.
    for kind in [preferred, fallback] {
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

/// The nearest live node of `resource` to a point (ties: lowest
/// id, by scan order) — restricted to the AI's own resource set.
/// On this arena the two layouts interleave geometrically (each
/// side's far gold mine sits a hair nearer the *enemy* Town
/// Center than its own), so "my side" cannot be derived from
/// distances: it is the mirrored block of the layout — the second
/// `NODE_SPECS` block of node ids is the opponent's set by
/// construction (see `Sim::spawn_opponent_start`). Dynamic nodes
/// (id past the layout: carcasses, farms) belong to whichever
/// Town Center is nearer — a carcass falls where the kill
/// happened.
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
        if best.map(|b| (b.0, b.1) < key).unwrap_or(true) {
            best = Some((key.0, key.1, n.pos_x, n.pos_y));
        }
    }
    best.map(|(_, _, px, py)| (px, py))
}

fn nearest_node(sim: &Sim, resource: ResourceKind, x: I16F16, y: I16F16) -> Option<u32> {
    let shared = crate::econ::NODE_SPECS.len() as u32;
    let foe_tc = sim.tc_entity_of(OWNER_PLAYER);
    let mut best: Option<(i64, u32)> = None;
    for n in sim.nodes() {
        if n.is_depleted() || n.resource() != resource {
            continue;
        }
        let mine = if n.id < shared * 2 {
            n.id >= shared
        } else {
            match foe_tc {
                Some(ftc) => {
                    steer::vec_len(n.pos_x - x, n.pos_y - y)
                        <= steer::vec_len(n.pos_x - ftc.pos_x, n.pos_y - ftc.pos_y)
                }
                None => true,
            }
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
        let mut best: Option<(i64, u32)> = None;
        for e in sim.entities() {
            if e.owner != OWNER_PLAYER || e.dead || !e.kind.counts_population() {
                continue;
            }
            let d = steer::vec_len(e.pos_x - tc_x, e.pos_y - tc_y);
            if d > DEFENSE_RADIUS {
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
/// `wave_size` trained soldiers (Militia/Archers) stand idle at
/// home, they all attack at once. Each picks the same kind of
/// target the script always picks: the nearest *reachable* human
/// population unit to itself; if none can be reached, the human
/// Town Center; if that is gone too, the nearest human building
/// (via `AttackBuilding`). The cooldown restarts on launch — win or
/// lose, the survivors' state (dead, fighting, walking home via
/// rally-less idleness) is simply re-read next decision.
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
                && matches!(e.kind, EntityKind::Militia | EntityKind::Archer)
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
    for mid in &wave {
        let (mx, my) = sim
            .entities()
            .iter()
            .find(|e| e.id == *mid)
            .map(|e| (e.pos_x, e.pos_y))
            .unwrap_or((I16F16::ZERO, I16F16::ZERO));
        // Nearest human population unit *that can actually be
        // reached*. A target standing inside a building footprint
        // (or behind a plugged chokepoint) has no path; ordering the
        // wave onto it fizzles every soldier at once (M6 fix — the
        // AI's own base can plug the map's wall gap, and the two
        // forward human units stand in it). Candidates are walked
        // nearest-first; the reachability probe is one A* per
        // candidate per soldier, once per wave — affordable, and
        // deterministic like every other path query.
        let mut candidates: Vec<(i64, u32, I16F16, I16F16)> = sim
            .entities()
            .iter()
            .filter(|e| e.owner == OWNER_PLAYER && !e.dead && e.kind.counts_population())
            .map(|e| {
                let d = steer::vec_len(e.pos_x - mx, e.pos_y - my);
                let db = d.to_bits() as i64;
                (db * db, e.id, e.pos_x, e.pos_y)
            })
            .collect();
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
        // No reachable human units: the Town Center, then buildings.
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
        let mut best_b: Option<(i64, u32)> = None;
        for b in sim.buildings() {
            if b.owner != OWNER_PLAYER {
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
        }
    }
    sim.ai.next_wave_tick = sim.tick_count() + cfg.wave_cooldown;
}
