//! Melee-vs-building approach regression tests.
//!
//! A melee unit ordered to attack a placed building must end its
//! approach within MELEE_REACH (3.0) of the footprint's EDGE and
//! actually deal damage. The pre-fix chase targeted the first
//! ring cell in fixed order — the footprint's NW *corner* ring
//! cell — whose center is already ~2.83 from the footprint edge
//! on the diagonal; steering's arrival shortfall (up to 1.0 short
//! of the cell center) left the attacker up to ~3.8 from the edge,
//! beyond reach, while combat re-issued the identical approach
//! forever (probed pre-fix: 40 000 ticks, HP frozen — see the
//! stall note the repair fixture carried). These tests stage an
//! attacker on a chosen side, issue the attack, and require real
//! damage inside a bounded window — no nudges, no workarounds.

use fixed::types::I16F16;
use skirmish_sim::{
    BuildingKind, Command, Entity, EntityKind, MatchMode, Sim, OWNER_OPPONENT, OWNER_PLAYER,
};

fn ent(sim: &Sim, id: u32) -> &Entity {
    sim.entities().iter().find(|e| e.id == id).unwrap()
}

fn f(v: I16F16) -> f64 {
    v.to_num::<f64>()
}

fn find_id(sim: &Sim, owner: u8, kind: EntityKind) -> u32 {
    sim.entities()
        .iter()
        .find(|e| e.owner == owner && e.kind == kind && !e.dead)
        .unwrap()
        .id
}

fn villagers_of(sim: &Sim, owner: u8) -> Vec<u32> {
    sim.entities()
        .iter()
        .filter(|e| e.owner == owner && e.kind == EntityKind::Villager && !e.dead)
        .map(|e| e.id)
        .collect()
}

fn building(sim: &Sim, bid: u32) -> &skirmish_sim::Building {
    sim.buildings().iter().find(|b| b.id == bid).unwrap()
}

fn building_hp(sim: &Sim, bid: u32) -> Option<f64> {
    sim.buildings().iter().find(|b| b.id == bid).map(|b| f(b.hp))
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

/// The first anchor cell near entity `center` where `kind` may be
/// built by `entity` — the construction suite's ring scan.
fn open_anchor_near(sim: &Sim, center: u32, entity: u32, kind: BuildingKind) -> (u32, u32) {
    let c = ent(sim, center);
    let (tcx, tcy) = sim.grid().world_to_cell(c.pos_x, c.pos_y);
    for r in 2..=14i32 {
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
                let (x, y) = sim.grid().cell_center(ax, ay);
                if sim.can_build(entity, kind, x, y) {
                    return (ax, ay);
                }
            }
        }
    }
    panic!("no buildable ground near the Town Center");
}

struct Scene {
    sim: Sim,
    bid: u32,
    /// Building anchor cell (NW footprint cell).
    anchor: (u32, u32),
    size: u32,
}

/// Raise an opponent building of `kind` near the opponent Town
/// Center with one builder, then send the builder home so the
/// footprint's ring stands completely clear — the scene isolates
/// the attacker's chase, nothing else.
fn scene(seed: u64, kind: BuildingKind) -> Scene {
    let mut sim = Sim::with_mode(seed, MatchMode::TwoPlayer);
    let opp_tc = find_id(&sim, OWNER_OPPONENT, EntityKind::TownCenter);
    let opp_villagers = villagers_of(&sim, OWNER_OPPONENT);
    let opp_scout = find_id(&sim, OWNER_OPPONENT, EntityKind::ScoutCavalry);
    let v1 = opp_villagers[0];

    // Park the opponent's Scout far away: it must not wander into
    // the scene (it would never auto-engage, but keep it clean).
    let (tc_x, tc_y) = {
        let tc = ent(&sim, opp_tc);
        (tc.pos_x, tc.pos_y)
    };
    sim.push_command(Command::Move {
        entity: opp_scout,
        x: tc_x + I16F16::from_num(80),
        y: tc_y + I16F16::from_num(80),
    });

    let (ax, ay) = open_anchor_near(&sim, opp_tc, v1, kind);
    let (x, y) = sim.grid().cell_center(ax, ay);
    sim.push_command(Command::Build {
        entity: v1,
        kind,
        x,
        y,
    });
    sim.tick();
    assert_eq!(sim.buildings().len(), 1, "placement was rejected");
    let bid = sim.buildings()[0].id;
    let up = run_until(&mut sim, 60_000, |s| building(s, bid).complete);
    assert!(up, "building never completed");

    // The builder goes home and stays there.
    sim.push_command(Command::Move {
        entity: v1,
        x: tc_x,
        y: tc_y,
    });
    run_until(&mut sim, 30_000, |s| ent(s, v1).order.is_none());

    Scene {
        sim,
        bid,
        anchor: (ax, ay),
        size: kind.footprint_size(),
    }
}

/// Walk `entity` to the center of cell `(cx, cy)` and wait until
/// it gets there.
fn stage_at(sim: &mut Sim, entity: u32, cx: u32, cy: u32) {
    let (x, y) = sim.grid().cell_center(cx, cy);
    sim.push_command(Command::Move { entity, x, y });
    let arrived = run_until(sim, 60_000, |s| ent(s, entity).order.is_none());
    assert!(arrived, "entity {entity} never reached its staging cell");
}

/// Order the attack and require the building's HP to drop within
/// `first_cap` ticks, then keep dropping (or fall) over the next
/// `sustain` ticks. Returns when both hold.
fn assert_damage_flows(sim: &mut Sim, attacker: u32, bid: u32, first_cap: u64, sustain: u64) {
    let hp0 = building_hp(sim, bid).expect("building must exist");
    sim.push_command(Command::AttackBuilding {
        entity: attacker,
        building: bid,
    });
    let hit = run_until(sim, first_cap, |s| match building_hp(s, bid) {
        Some(hp) => hp < hp0,
        None => true, // destroyed outright counts as damage flowing
    });
    assert!(
        hit,
        "no damage within {first_cap} ticks of the attack order (HP frozen at {hp0}) — melee approach deadlock"
    );
    let hp1 = building_hp(sim, bid);
    let keeps = run_until(sim, sustain, |s| match (building_hp(s, bid), hp1) {
        (None, _) => true,
        (Some(hp), Some(prev)) => hp < prev,
        (Some(_), None) => true,
    });
    assert!(keeps, "damage stalled after the first hit");
}

#[test]
fn melee_damages_house_approaching_the_nw_corner() {
    // The probed geometry: the attacker stages north-west of a
    // 2x2 House — its approach enters the footprint's NW corner
    // ring cell on the diagonal, the exact pre-fix deadlock.
    let mut sc = scene(99, BuildingKind::House);
    let scout = find_id(&sc.sim, OWNER_PLAYER, EntityKind::ScoutCavalry);
    let (ax, ay) = sc.anchor;
    stage_at(&mut sc.sim, scout, ax - 6, ay - 6);
    assert_damage_flows(&mut sc.sim, scout, sc.bid, 12_000, 6_000);
}

#[test]
fn melee_damages_barracks_approaching_from_the_south() {
    // A 3x3 Barracks, attacker staged due south: the pre-fix
    // chase still walked it around to the NW corner cell and
    // stranded it there. Any approach direction must work.
    let mut sc = scene(13, BuildingKind::Barracks);
    let scout = find_id(&sc.sim, OWNER_PLAYER, EntityKind::ScoutCavalry);
    let (ax, ay) = sc.anchor;
    let south_x = ax + sc.size / 2;
    let south_y = ay + sc.size + 6;
    stage_at(&mut sc.sim, scout, south_x, south_y);
    assert_damage_flows(&mut sc.sim, scout, sc.bid, 12_000, 6_000);
}

#[test]
fn melee_group_arriving_together_razes_a_barracks() {
    // Four attackers (Scout + three villagers — every player unit
    // with an attack row may hit a building) staged south-east of
    // a 3x3 Barracks, all ordered at once: the group must spread
    // around the footprint, get within reach, and raze it inside
    // a generous bound.
    let mut sc = scene(79, BuildingKind::Barracks);
    let scout = find_id(&sc.sim, OWNER_PLAYER, EntityKind::ScoutCavalry);
    let mut attackers = vec![scout];
    attackers.extend(villagers_of(&sc.sim, OWNER_PLAYER));
    assert_eq!(attackers.len(), 4);
    let (ax, ay) = sc.anchor;
    let stage_x = ax + sc.size + 5;
    let stage_y = ay + sc.size + 5;
    for (i, &a) in attackers.iter().enumerate() {
        stage_at(&mut sc.sim, a, stage_x + (i as u32 % 2) * 2, stage_y + (i as u32 / 2) * 2);
    }
    for &a in &attackers {
        sc.sim.push_command(Command::AttackBuilding {
            entity: a,
            building: sc.bid,
        });
    }
    let razed = run_until(&mut sc.sim, 240_000, |s| building_hp(s, sc.bid).is_none());
    assert!(
        razed,
        "the group never razed the barracks (HP left: {:?})",
        building_hp(&sc.sim, sc.bid)
    );
}

#[test]
fn foundation_damage_persists_through_completion_bld05() {
    // BLD-05: HP grows with construction progress, and damage
    // taken mid-build is NOT healed by building — a building that
    // took D damage under construction completes at max - D.
    let mut sim = Sim::with_mode(42, MatchMode::TwoPlayer);
    let opp_tc = find_id(&sim, OWNER_OPPONENT, EntityKind::TownCenter);
    let v1 = villagers_of(&sim, OWNER_OPPONENT)[0];
    let (ax, ay) = open_anchor_near(&sim, opp_tc, v1, BuildingKind::House);
    assert!(ax >= 1, "need a staging cell west of the footprint");
    let (x, y) = sim.grid().cell_center(ax, ay);
    sim.push_command(Command::Build {
        entity: v1,
        kind: BuildingKind::House,
        x,
        y,
    });
    sim.tick();
    assert_eq!(sim.buildings().len(), 1, "placement was rejected");
    let bid = sim.buildings()[0].id;
    let fresh = building_hp(&sim, bid).expect("foundation exists");
    assert!(fresh < 3.0, "fresh foundation must stand at ~1 HP, got {fresh}");
    // Build until the HP line is well clear of one-shot range.
    let grown = run_until(&mut sim, 60_000, |s| {
        building_hp(s, bid).map(|hp| hp >= 200.0).unwrap_or(false)
    });
    assert!(grown, "foundation HP never grew with progress");
    // Freeze the ceiling: send the builder home before the attack.
    let (tc_x, tc_y) = {
        let tc = ent(&sim, opp_tc);
        (tc.pos_x, tc.pos_y)
    };
    sim.push_command(Command::Move {
        entity: v1,
        x: tc_x,
        y: tc_y,
    });
    run_until(&mut sim, 30_000, |s| ent(s, v1).order.is_none());
    let hp_before = building_hp(&sim, bid).expect("foundation exists");
    // The player's Scout dents the foundation (ordered attack).
    let scout = find_id(&sim, OWNER_PLAYER, EntityKind::ScoutCavalry);
    stage_at(&mut sim, scout, ax - 1, ay);
    sim.push_command(Command::AttackBuilding {
        entity: scout,
        building: bid,
    });
    let dropped = run_until(&mut sim, 30_000, |s| {
        building_hp(s, bid).map(|hp| hp <= hp_before - 10.0).unwrap_or(true)
    });
    assert!(dropped, "the scout never dented the foundation");
    let damage_seen = hp_before - building_hp(&sim, bid).expect("foundation exists");
    // Stop the attack (one more hit may land before the order
    // applies — the tolerance below allows exactly one), resume
    // the build, and finish: the damage must persist to the end.
    sim.push_command(Command::Stop { entity: scout });
    sim.push_command(Command::Build {
        entity: v1,
        kind: BuildingKind::House,
        x,
        y,
    });
    let done = run_until(&mut sim, 60_000, |s| building(s, bid).complete);
    assert!(done, "the resumed house never completed");
    let final_hp = building_hp(&sim, bid).expect("house exists");
    let missing = 550.0 - final_hp;
    assert!(
        missing >= damage_seen - 0.01 && missing <= damage_seen + 5.5,
        "final HP {final_hp}: missing {missing}, expected the {damage_seen} damage dealt (plus at most one in-flight hit)"
    );
}
