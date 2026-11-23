//! AutoExplore (SEL-17): a Scout Cavalry explores its owner's
//! fog frontier on its own.
//!
//! Pins the feature's contract: EVERY Scout Cavalry is eligible
//! — the starting scout and trained scouts alike (current DE,
//! update 111772; widened 2026-10-20 on Claudio's ruling after
//! the first version shipped starting-only, matching DE
//! 2020–2023) — while every other kind still rejects the toggle.
//! The walker actually explores, any manual order cancels the
//! mode, toggling off stops the scout where it stands, and two
//! runs fed the same commands stay bit-identical (the mode is
//! hashed state, not a client trick).

use fixed::types::I16F16;
use skirmish_sim::{
    Age, BuildingKind, Command, EntityKind, NodeKind, QueuedOrder, ResourceKind, Sim, TechId,
    OWNER_OPPONENT, OWNER_PLAYER,
};

fn scout_of(sim: &Sim, owner: u8) -> u32 {
    sim.entities()
        .iter()
        .find(|e| e.kind == EntityKind::ScoutCavalry && e.owner == owner)
        .expect("a scout")
        .id
}

fn explored_cells(sim: &Sim, owner: u8) -> usize {
    sim.visibility(owner).iter().filter(|&&v| v >= 1).count()
}

fn pos_of(sim: &Sim, id: u32) -> (f64, f64) {
    let e = &sim.entities()[id as usize];
    (e.pos_x.to_num::<f64>(), e.pos_y.to_num::<f64>())
}

fn dist(a: (f64, f64), b: (f64, f64)) -> f64 {
    ((a.0 - b.0).powi(2) + (a.1 - b.1).powi(2)).sqrt()
}

#[test]
fn auto_explore_rides_and_the_frontier_grows() {
    let seed = 42;
    let mut auto = Sim::new(seed);
    let mut control = Sim::new(seed);
    let scout = scout_of(&auto, OWNER_PLAYER);
    let spawn = pos_of(&auto, scout);
    auto.push_command(Command::SetAutoExplore {
        entity: scout,
        enabled: true,
    });
    // Three minutes of game time, no further orders.
    for _ in 0..10_800 {
        auto.tick();
        control.tick();
    }
    assert!(auto.auto_explore_on(scout), "the mode switched itself off");
    let travelled = dist(pos_of(&auto, scout), spawn);
    assert!(
        travelled > 40.0,
        "the scout barely moved ({travelled:.1} world units)"
    );
    let (grown, base) = (
        explored_cells(&auto, OWNER_PLAYER),
        explored_cells(&control, OWNER_PLAYER),
    );
    assert!(
        grown > base + 500,
        "exploration did not grow the frontier: {grown} vs control {base}"
    );
}

#[test]
fn auto_explore_is_bit_deterministic() {
    let run = |seed: u64| -> Vec<u64> {
        let mut sim = Sim::new(seed);
        let scout = scout_of(&sim, OWNER_PLAYER);
        sim.push_command(Command::SetAutoExplore {
            entity: scout,
            enabled: true,
        });
        let mut hashes = Vec::new();
        for i in 0..7_200 {
            let h = sim.tick();
            if i % 600 == 0 {
                hashes.push(h);
            }
        }
        hashes
    };
    assert_eq!(run(99), run(99), "identical auto-explore runs diverged");
    // And the mode is real hashed state: the same seed without the
    // toggle produces a different stream.
    let mut plain = Sim::new(99);
    let mut plain_hashes = Vec::new();
    for i in 0..7_200 {
        let h = plain.tick();
        if i % 600 == 0 {
            plain_hashes.push(h);
        }
    }
    assert_ne!(run(99), plain_hashes, "the toggle changed nothing");
}

#[test]
fn a_manual_move_cancels_auto_explore() {
    let mut sim = Sim::new(7);
    let scout = scout_of(&sim, OWNER_PLAYER);
    sim.push_command(Command::SetAutoExplore {
        entity: scout,
        enabled: true,
    });
    for _ in 0..600 {
        sim.tick();
    }
    assert!(sim.auto_explore_on(scout));
    // Ride home: any manual order takes the scout back.
    let tc = &sim.entities()[0];
    let (tx, ty) = (tc.pos_x, tc.pos_y);
    sim.push_command(Command::Move {
        entity: scout,
        x: tx,
        y: ty,
    });
    sim.tick();
    assert!(
        !sim.auto_explore_on(scout),
        "a manual Move left AutoExplore on"
    );
    // And the scout obeys the order (ends up by the Town Center).
    for _ in 0..12_000 {
        sim.tick();
    }
    let d = dist(pos_of(&sim, scout), (tx.to_num::<f64>(), ty.to_num::<f64>()));
    assert!(d < 12.0, "the scout never rode home (still {d:.1} away)");
}

#[test]
fn a_queued_order_cancels_auto_explore() {
    let mut sim = Sim::new(11);
    let scout = scout_of(&sim, OWNER_PLAYER);
    sim.push_command(Command::SetAutoExplore {
        entity: scout,
        enabled: true,
    });
    for _ in 0..300 {
        sim.tick();
    }
    assert!(sim.auto_explore_on(scout));
    sim.push_command(Command::Queue {
        entity: scout,
        order: QueuedOrder::Move {
            x: sim.entities()[scout as usize].pos_x,
            y: sim.entities()[scout as usize].pos_y,
        },
    });
    sim.tick();
    assert!(
        !sim.auto_explore_on(scout),
        "a queued order left AutoExplore on"
    );
}

#[test]
fn toggling_off_stops_the_scout_where_it_stands() {
    let mut sim = Sim::new(13);
    let scout = scout_of(&sim, OWNER_PLAYER);
    sim.push_command(Command::SetAutoExplore {
        entity: scout,
        enabled: true,
    });
    for _ in 0..1_200 {
        sim.tick();
    }
    assert!(sim.auto_explore_on(scout));
    sim.push_command(Command::SetAutoExplore {
        entity: scout,
        enabled: false,
    });
    sim.tick();
    assert!(!sim.auto_explore_on(scout));
    let stopped_at = pos_of(&sim, scout);
    for _ in 0..1_200 {
        sim.tick();
    }
    let drift = dist(pos_of(&sim, scout), stopped_at);
    assert!(
        drift < 3.0,
        "the scout kept riding after toggle-off ({drift:.1} units)"
    );
}

#[test]
fn every_scout_is_eligible_other_kinds_are_not() {
    let sim = Sim::new(17);
    let scout = scout_of(&sim, OWNER_PLAYER);
    assert!(sim.can_auto_explore(scout));
    // The eligibility rule is exactly "living Scout Cavalry owned
    // by a player": every other entity in the start — Town
    // Center, villagers, and the whole Gaia herd (sheep are
    // Scout-shaped in no way, and their owner is Gaia besides) —
    // rejects the toggle.
    for e in sim.entities().iter() {
        let expect = e.kind == EntityKind::ScoutCavalry
            && (e.owner == OWNER_PLAYER || e.owner == OWNER_OPPONENT);
        assert_eq!(
            sim.can_auto_explore(e.id),
            expect,
            "entity {} ({:?}, owner {}) eligibility mismatch",
            e.id,
            e.kind,
            e.owner
        );
    }
    let villager = sim
        .entities()
        .iter()
        .find(|e| e.kind == EntityKind::Villager)
        .map(|e| e.id)
        .expect("a villager");
    let mut sim = sim;
    sim.push_command(Command::SetAutoExplore {
        entity: villager,
        enabled: true,
    });
    sim.tick();
    assert!(!sim.auto_explore_on(villager));

    // Both seats' scouts are eligible in a match.
    let m = Sim::new_match(19, skirmish_sim::MatchMode::TwoPlayer);
    let mine = scout_of(&m, OWNER_PLAYER);
    let theirs = scout_of(&m, OWNER_OPPONENT);
    assert!(m.can_auto_explore(mine));
    assert!(m.can_auto_explore(theirs));
    assert_ne!(mine, theirs);
}

// --- The trained-scout route (helpers mirror sim/tests/tech.rs) --

fn anchor_point(sim: &Sim, cx: u32, cy: u32) -> (I16F16, I16F16) {
    sim.grid().cell_center(cx, cy)
}

fn cell_of(sim: &Sim, x: I16F16, y: I16F16) -> (u32, u32) {
    sim.grid().world_to_cell(x, y)
}

/// The first anchor cell near an arbitrary center cell where `kind`
/// may be built by `entity` right now, scanning rings outward —
/// the M7 pattern from construction.rs / combat.rs: placement
/// scenarios derive their ground from the generated map.
fn open_anchor_near(
    sim: &Sim,
    entity: u32,
    kind: BuildingKind,
    cx: u32,
    cy: u32,
) -> (u32, u32) {
    for r in 0..=14i32 {
        for dy in -r..=r {
            for dx in -r..=r {
                if r > 0 && dx.abs() != r && dy.abs() != r {
                    continue;
                }
                let (ax, ay) = (cx as i32 + dx, cy as i32 + dy);
                if ax < 0 || ay < 0 {
                    continue;
                }
                let (ax, ay) = (ax as u32, ay as u32);
                let (x, y) = anchor_point(sim, ax, ay);
                if sim.can_build(entity, kind, x, y) {
                    return (ax, ay);
                }
            }
        }
    }
    panic!("no buildable ground near ({cx}, {cy}) for {kind:?}");
}

/// The first buildable anchor for `kind` on rings around the
/// Town Center.
fn open_anchor_near_tc(sim: &Sim, entity: u32, kind: BuildingKind) -> (u32, u32) {
    let tc = sim.town_center();
    let (tcx, tcy) = cell_of(sim, tc.pos_x, tc.pos_y);
    open_anchor_near(sim, entity, kind, tcx, tcy)
}

fn build(sim: &mut Sim, entity: u32, kind: BuildingKind, cx: u32, cy: u32) {
    let (x, y) = anchor_point(sim, cx, cy);
    sim.push_command(Command::Build {
        entity,
        kind,
        x,
        y,
    });
}

fn run_until(sim: &mut Sim, cap: u64, mut done: impl FnMut(&Sim) -> bool) -> bool {
    for _ in 0..cap {
        if done(sim) {
            return true;
        }
        sim.tick();
    }
    done(sim)
}

fn first_node(sim: &Sim, kind: NodeKind) -> u32 {
    sim.nodes().iter().find(|n| n.kind == kind).unwrap().id
}

/// First node of `kind` that still holds resources.
fn first_live_node(sim: &Sim, kind: NodeKind) -> u32 {
    sim.nodes()
        .iter()
        .find(|n| n.kind == kind && n.amount_left > I16F16::from_num(0))
        .unwrap()
        .id
}

fn building_id(sim: &Sim, kind: BuildingKind) -> u32 {
    sim.buildings()
        .iter()
        .find(|b| b.kind == kind && b.complete)
        .unwrap()
        .id
}

/// Raise `kind` at `(cx, cy)` with `entity` and wait for completion.
fn raise(sim: &mut Sim, entity: u32, kind: BuildingKind, cx: u32, cy: u32) {
    build(sim, entity, kind, cx, cy);
    let up = run_until(sim, 30_000, |s| {
        s.buildings()
            .iter()
            .any(|b| b.kind == kind && b.complete)
    });
    assert!(up, "{kind:?} never completed");
}

/// A woodline tree for sustained chopping: a forest EDGE tree —
/// a standing cell beside it that the pathfinder reaches from the
/// Town Center, and several tree neighbors within the retarget
/// radius, so a depleted trunk chains to the next. (Copied from
/// tech.rs: the first tree in id order is a lone straggler whose
/// 100 wood starves any long gather.)
fn woodline_tree(sim: &Sim) -> u32 {
    let tc = sim.town_center();
    let trees: Vec<_> = sim
        .nodes()
        .iter()
        .filter(|n| n.kind == NodeKind::Tree && !n.is_depleted())
        .collect();
    let neighbors = |t: &skirmish_sim::ResourceNode| {
        trees
            .iter()
            .filter(|o| {
                let dx = o.pos_x.to_num::<f64>() - t.pos_x.to_num::<f64>();
                let dy = o.pos_y.to_num::<f64>() - t.pos_y.to_num::<f64>();
                dx * dx + dy * dy <= 144.0
            })
            .count()
    };
    let workable = |t: &skirmish_sim::ResourceNode| {
        let (cx, cy) = sim.grid().world_to_cell(t.pos_x, t.pos_y);
        (-1i32..=1).any(|dy| {
            (-1i32..=1).any(|dx| {
                let (nx, ny) = (cx as i32 + dx, cy as i32 + dy);
                if nx < 0 || ny < 0 || sim.grid().is_blocked(nx as u32, ny as u32) {
                    return false;
                }
                let center = sim.grid().cell_center(nx as u32, ny as u32);
                sim.grid().find_path((tc.pos_x, tc.pos_y), center).is_some()
            })
        })
    };
    // AI-08 (2026-10-22): wolves hunt lumberjacks — DE behaviour,
    // and on seed 101 the densest woodline stands inside a wolf
    // pack's range, so the pack kills the choppers before any
    // long gather finishes. A DE player picks the forest the
    // wolves are not watching; do the same: among workable
    // woodlines prefer one outside every wolf's sight (48 wu)
    // plus a margin, falling back to the plain pick if none is.
    let wolf_posts: Vec<(f64, f64)> = sim
        .entities()
        .iter()
        .filter(|e| e.kind == EntityKind::Wolf)
        .map(|e| (e.pos_x.to_num::<f64>(), e.pos_y.to_num::<f64>()))
        .collect();
    let wolf_safe = |t: &skirmish_sim::ResourceNode| {
        wolf_posts.iter().all(|(wx, wy)| {
            let dx = t.pos_x.to_num::<f64>() - wx;
            let dy = t.pos_y.to_num::<f64>() - wy;
            dx * dx + dy * dy >= 60.0 * 60.0
        })
    };
    let candidates: Vec<_> = trees
        .iter()
        .filter(|t| workable(t) && neighbors(t) >= 3)
        .copied()
        .collect();
    candidates
        .iter()
        .filter(|t| wolf_safe(t))
        .max_by_key(|t| neighbors(t))
        .or_else(|| candidates.iter().max_by_key(|t| neighbors(t)))
        .expect("no workable woodline tree")
        .id
}

/// Play the standard start up to the Feudal Age: Mill + Lumber
/// Camp (the requirement pair), berries until 500 food, click up.
/// (Mirrors tech.rs's `reach_feudal`.)
fn reach_feudal(sim: &mut Sim) {
    // AI-08 landing adaptation (2026-10-22): the historical anchors
    // (21, 20) / (16, 20) stand ~55 cells south-west of seed 101's
    // Town Center, and since AI-08 the walk there crosses the wolf
    // pack's ground — the pack killed both builders on the commute
    // (verified by trace: villagers 1 and 3 dead at the pack's LoS
    // edge, the Stable's builder among them). A DE player builds
    // beside his TC, not across a wolf pack.
    //
    // MAP-05 re-stage (2026-11-22): the by-the-TC anchors that
    // replaced them — Mill (39, 77) beside the old TC position
    // (36, 76), Lumber Camp (30, 73) — are stale again: MAP-05's
    // clump elevation changed the shared RNG draw count and seed
    // 101's Town Center now stands at (37, 53), so (39, 77) is
    // refused ground and the Mill never completed. The pair is
    // derived from the generated map at runtime instead: the Mill
    // beside the berry patch (first berry node's cell, ring
    // scan), the Lumber Camp on the open edge between the
    // woodline and the Town Center (the combat.rs camp pattern).
    let berry0 = sim
        .nodes()
        .iter()
        .find(|n| n.kind == NodeKind::BerryBush)
        .unwrap();
    let berry_cell = cell_of(sim, berry0.pos_x, berry0.pos_y);
    let (mx, my) = open_anchor_near(sim, 2, BuildingKind::Mill, berry_cell.0, berry_cell.1);
    raise(sim, 2, BuildingKind::Mill, mx, my);
    let tree_node = sim
        .nodes()
        .iter()
        .find(|n| n.id == woodline_tree(sim))
        .unwrap();
    let tree_cell = cell_of(sim, tree_node.pos_x, tree_node.pos_y);
    let tc = sim.town_center();
    let tc_cell = cell_of(sim, tc.pos_x, tc.pos_y);
    let step_toward_tc = |a: u32, b: u32| -> u32 {
        (a as i32 + (b as i32 - a as i32).signum() * 3).clamp(0, 119) as u32
    };
    let camp_center = (
        step_toward_tc(tree_cell.0, tc_cell.0),
        step_toward_tc(tree_cell.1, tc_cell.1),
    );
    let (lx, ly) = open_anchor_near(sim, 3, BuildingKind::LumberCamp, camp_center.0, camp_center.1);
    raise(sim, 3, BuildingKind::LumberCamp, lx, ly);
    let berry = first_node(sim, NodeKind::BerryBush);
    sim.push_command(Command::Gather {
        entity: 1,
        node: berry,
    });
    sim.push_command(Command::Gather {
        entity: 2,
        node: berry,
    });
    let food = run_until(sim, 120_000, |s| {
        s.stockpile().amount(ResourceKind::Food) >= I16F16::from_num(500)
    });
    assert!(food, "never gathered the Feudal food");
    assert_eq!(sim.feudal_building_count(OWNER_PLAYER), 2);
    assert!(sim.can_research_tc(TechId::FeudalAge));
    sim.push_command(Command::ResearchTc {
        tech: TechId::FeudalAge,
    });
    let up = run_until(sim, 30_000, |s| s.age_of(OWNER_PLAYER) == Age::Feudal);
    assert!(up, "never reached Feudal");
}

/// Wait until the wood stockpile holds at least `want`.
fn wait_wood(sim: &mut Sim, want: i32) {
    let ok = run_until(sim, 240_000, |s| {
        s.stockpile().amount(ResourceKind::Wood) >= I16F16::from_num(want)
    });
    assert!(ok, "never stocked {want} wood");
}

#[test]
fn a_trained_scout_is_eligible_and_explores() {
    // The full production route (seed and flow mirror tech.rs's
    // Stable test): Feudal, chop, raise a Barracks (BLD-15: the
    // Stable requires one standing), then the Stable, bank the
    // Scout's 80 food, train one — a scout that did NOT spawn
    // with the match must take the toggle exactly like the
    // starting one.
    let mut sim = Sim::new(101);
    let starting = scout_of(&sim, OWNER_PLAYER);
    reach_feudal(&mut sim);
    let tree = woodline_tree(&sim);
    for id in 1..=3 {
        sim.push_command(Command::Gather {
            entity: id,
            node: tree,
        });
    }
    wait_wood(&mut sim, 175);
    // AI-08 landing adaptation: same move as reach_feudal — the
    // Barracks and Stable stand by the TC now, not across the
    // wolf pack's ground.
    // MAP-05 re-stage (2026-11-22): the by-the-TC cells verified
    // for the old layout — Barracks (36, 80), Stable (29, 81) —
    // belong to the old Town Center position; both anchors are
    // derived from the generated map now (Barracks on rings
    // around the Town Center, the Stable beside the Barracks).
    let (bx, by) = open_anchor_near_tc(&sim, 1, BuildingKind::Barracks);
    raise(&mut sim, 1, BuildingKind::Barracks, bx, by);
    // The Barracks spent the stockpile; refill for the Stable.
    let tree = woodline_tree(&sim);
    for id in 1..=3 {
        sim.push_command(Command::Gather {
            entity: id,
            node: tree,
        });
    }
    wait_wood(&mut sim, 175);
    // MOV-03 landing adaptation (2026-11-17): the historical
    // Stable anchor (29, 80) was rejected in the MOV-03 timeline —
    // at the scripted tick a sheep stood on a footprint cell,
    // and a living unit standing on the footprint blocks
    // placement (the reference game's rule), so the Build
    // command was dropped and the raise waited forever.
    // MAP-05 re-stage (2026-11-22): instead of pinning another
    // fixed cell for the new layout, the Stable's anchor is
    // derived at the scripted tick itself — the ring scan beside
    // the Barracks only returns ground `can_build` accepts right
    // now (explored, unblocked, unit- and node-free), which is
    // exactly the property the fixed-cell probes kept verifying
    // by hand after every layout shift.
    let (sx, sy) = open_anchor_near(&sim, 1, BuildingKind::Stable, bx, by);
    raise(&mut sim, 1, BuildingKind::Stable, sx, sy);
    let stable = building_id(&sim, BuildingKind::Stable);
    let berry = first_live_node(&sim, NodeKind::BerryBush);
    sim.push_command(Command::Gather {
        entity: 2,
        node: berry,
    });
    let fed = run_until(&mut sim, 180_000, |s| {
        s.stockpile().amount(ResourceKind::Food) >= I16F16::from_num(80)
    });
    assert!(fed, "never banked the Scout's 80 food");
    assert!(sim.can_train_at(stable, EntityKind::ScoutCavalry));
    sim.push_command(Command::TrainAt {
        building: stable,
        kind: EntityKind::ScoutCavalry,
    });
    let trained = run_until(&mut sim, 30_000, |s| {
        s.entities()
            .iter()
            .any(|e| e.kind == EntityKind::ScoutCavalry && e.owner == OWNER_PLAYER && e.id != starting)
    });
    assert!(trained, "the Stable never produced its scout");
    let trained = sim
        .entities()
        .iter()
        .find(|e| e.kind == EntityKind::ScoutCavalry && e.owner == OWNER_PLAYER && e.id != starting)
        .expect("the trained scout")
        .id;
    assert!(
        sim.can_auto_explore(trained),
        "a trained scout must be auto-explore eligible"
    );
    sim.push_command(Command::SetAutoExplore {
        entity: trained,
        enabled: true,
    });
    sim.tick();
    assert!(sim.auto_explore_on(trained), "the trained scout refused the mode");
    // And it actually rides: three minutes unattended.
    let spawn = pos_of(&sim, trained);
    for _ in 0..10_800 {
        sim.tick();
    }
    assert!(sim.auto_explore_on(trained), "the trained scout's mode died");
    let travelled = dist(pos_of(&sim, trained), spawn);
    assert!(
        travelled > 20.0,
        "the trained scout barely moved ({travelled:.1} world units)"
    );
}
