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
//!    population closes on it), then raise a Barracks once the
//!    economy is large enough to feed an army.
//! 3. Put every idle villager to work, balancing food / wood / gold
//!    toward fixed worker targets (surplus to food); food comes
//!    from the nearest food source — berries or carcasses first,
//!    and when no food source is near the Town Center it hunts its
//!    own sheep (the killer auto-harvests, ECO-13).
//! 4. Rally the Barracks toward the human's base and train a mixed
//!    Militia/Archer army up to the army target.
//! 5. Launch attack waves: when the idle army at home reaches the
//!    wave size, every idle soldier attacks — the nearest human
//!    unit, else the human Town Center, else the nearest human
//!    building — then regroups for the cooldown. Idle soldiers also
//!    defend: a human unit near the AI's Town Center is engaged on
//!    sight.
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
        let queued = sim.tc_state(tc_id).map(|s| s.queue).unwrap_or(0);
        if my_villagers + queued < cfg.villager_target && sim.can_train_villager_at(tc_id) {
            sim.push_command(Command::TrainVillagerAt { tc: tc_id });
        }
    }

    // 2. Construction (at most one placement per decision): houses
    //    ahead of the cap first, then the Barracks.
    plan_build(sim, &cfg, &mut acted);

    // 3. Barracks rally: once, toward a staging point short of the
    //    human's Town Center, so fresh troops walk toward the front.
    plan_rally(sim);

    // 4. Military production: fill the Barracks queue (one enqueue
    //    per decision) toward the army target, alternating Militia
    //    and Archers so the mix — and the resource draw — stays
    //    balanced; if the preferred kind is unaffordable, try the
    //    other before giving up.
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
/// around the AI's Town Center offers (`Sim::can_build` is the
/// sim's own validation — the AI plays by the placement rules too).
fn plan_build(sim: &mut Sim, cfg: &AiConfig, acted: &mut Vec<u32>) {
    let me = OWNER_OPPONENT;
    let Some((_, tc_x, tc_y)) = sim.tc_entity_of(me).map(|tc| (tc.id, tc.pos_x, tc.pos_y)) else {
        return;
    };
    let pop = sim.population_of(me);
    let cap = sim.population_cap_of(me);
    let wood = sim.stockpile_of(me).amount(ResourceKind::Wood);
    let house_in_progress = sim
        .buildings()
        .iter()
        .any(|b| b.owner == me && b.kind == BuildingKind::House && !b.complete);
    let barracks_any = sim
        .buildings()
        .iter()
        .any(|b| b.owner == me && b.kind == BuildingKind::Barracks);
    // Priority: pop-cap pressure (house) → the Barracks → a house
    // built ahead while there is still slack for one more.
    let want = if pop + 1 >= cap && !house_in_progress && wood >= BuildingKind::House.wood_cost() {
        Some(BuildingKind::House)
    } else if !barracks_any
        && count_kind(sim, me, EntityKind::Villager) >= cfg.barracks_min_villagers
        && wood >= BuildingKind::Barracks.wood_cost()
    {
        Some(BuildingKind::Barracks)
    } else if pop + 2 >= cap && !house_in_progress && wood >= BuildingKind::House.wood_cost() {
        Some(BuildingKind::House)
    } else {
        None
    };
    let Some(kind) = want else { return };
    let Some(builder) = sim
        .entities()
        .iter()
        .find(|e| {
            e.owner == me
                && e.kind == EntityKind::Villager
                && is_idle(sim, e.id)
                && !acted.contains(&e.id)
        })
        .map(|e| e.id)
    else {
        return;
    };
    if let Some((x, y)) = find_anchor(sim, builder, kind, tc_x, tc_y) {
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
/// Chebyshev rings around the Town Center's cell, in fixed order
/// (ring, row, column), each validated by the sim's own
/// `can_build`. Bounded — if nothing legal shows up within the
/// search, the AI simply tries again next decision.
fn find_anchor(
    sim: &Sim,
    builder: u32,
    kind: BuildingKind,
    tc_x: I16F16,
    tc_y: I16F16,
) -> Option<(I16F16, I16F16)> {
    let (tcx, tcy) = sim.grid().world_to_cell(tc_x, tc_y);
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
                if tried > 48 {
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

/// Barracks rally: set once, when the first Barracks stands
/// complete and has no rally yet — toward the human's Town Center,
/// stopping short of it (the staging point fresh troops gather at).
fn plan_rally(sim: &mut Sim) {
    let me = OWNER_OPPONENT;
    let Some(human_tc) = sim.tc_entity_of(OWNER_PLAYER) else {
        return;
    };
    let (hx, hy) = (human_tc.pos_x, human_tc.pos_y);
    let Some(barracks) = sim
        .buildings()
        .iter()
        .find(|b| b.owner == me && b.kind == BuildingKind::Barracks && b.complete && b.rally.is_none())
    else {
        return;
    };
    let bid = barracks.id;
    let (bx, by) = (barracks.pos_x, barracks.pos_y);
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
    let Some(barracks) = sim
        .buildings()
        .iter()
        .find(|b| b.owner == me && b.kind == BuildingKind::Barracks && b.complete)
    else {
        return;
    };
    let bid = barracks.id;
    if !barracks.queue.is_empty() {
        return; // one production line, kept busy one enqueue at a time
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
    if sim.can_train_at(bid, preferred) {
        sim.push_command(Command::TrainAt {
            building: bid,
            kind: preferred,
        });
    } else if sim.can_train_at(bid, fallback) {
        sim.push_command(Command::TrainAt {
            building: bid,
            kind: fallback,
        });
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
                if let Some(sheep) = nearest_owned_sheep(sim, me, tc_x, tc_y) {
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
fn nearest_owned_sheep(sim: &Sim, owner: u8, x: I16F16, y: I16F16) -> Option<u32> {
    let mut best: Option<(i64, u32)> = None;
    for e in sim.entities() {
        if e.kind != EntityKind::Sheep || e.owner != owner || e.dead {
            continue;
        }
        let d = steer::vec_len(e.pos_x - x, e.pos_y - y);
        let db = d.to_bits() as i64;
        let key = (db * db, e.id);
        if best.map(|b| key < b).unwrap_or(true) {
            best = Some(key);
        }
    }
    best.map(|(_, id)| id)
}

/// Defense step: an idle soldier with a human unit near the AI's
/// Town Center engages the nearest such unit. (Soldiers already
/// fighting, walking a rally, or out on a wave are not recalled —
/// auto-acquisition also covers part of this; the explicit step
/// exists so even the Scout, which never auto-acquires, defends
/// its home.)
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
        let mut best: Option<(i64, u32)> = None;
        for e in sim.entities() {
            if e.owner != OWNER_PLAYER || e.dead || !e.kind.counts_population() {
                continue;
            }
            let d = steer::vec_len(e.pos_x - tc_x, e.pos_y - tc_y);
            if d > DEFENSE_RADIUS {
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
/// target the script always picks: the nearest human population
/// unit to itself; if the human has none left, the human Town
/// Center; if that is gone too, the nearest human building (via
/// `AttackBuilding`). The cooldown restarts on launch — win or
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
    let human_tc = sim.tc_entity_of(OWNER_PLAYER).map(|tc| tc.id);
    for mid in &wave {
        let (mx, my) = sim
            .entities()
            .iter()
            .find(|e| e.id == *mid)
            .map(|e| (e.pos_x, e.pos_y))
            .unwrap_or((I16F16::ZERO, I16F16::ZERO));
        // Nearest human population unit.
        let mut best: Option<(i64, u32)> = None;
        for e in sim.entities() {
            if e.owner != OWNER_PLAYER || e.dead || !e.kind.counts_population() {
                continue;
            }
            let d = steer::vec_len(e.pos_x - mx, e.pos_y - my);
            let db = d.to_bits() as i64;
            let key = (db * db, e.id);
            if best.map(|b| key < b).unwrap_or(true) {
                best = Some(key);
            }
        }
        if let Some((_, tid)) = best {
            sim.push_command(Command::Attack {
                entity: *mid,
                target: tid,
            });
            acted.push(*mid);
            continue;
        }
        // No human units left: the Town Center, then its buildings.
        if let Some(tc_id) = human_tc {
            sim.push_command(Command::Attack {
                entity: *mid,
                target: tc_id,
            });
            acted.push(*mid);
            continue;
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
