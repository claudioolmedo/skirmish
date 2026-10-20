//! Repair tests (BLD-07): villagers restoring damaged placed
//! buildings at the documented DE rate and cost.
//!
//! Numbers under test come from the public mechanics study
//! (Fandom Villager page, Phase 0 public-documentation study):
//! the first repairer restores 750 HP/min (12.5 HP/s), each
//! additional repairer +375 HP/min; restoring a building from
//! 0 to full costs 50% of its build cost, charged as a
//! proportional drip; if the owner cannot pay, repairers stop
//! and stand idle. Repair is NOT construction-rate: a House
//! (550 HP) takes 44 s of solo repair regardless of its 25 s
//! build time.
//!
//! Scenarios run in TwoPlayer matches: the opponent builds, the
//! human Scout does the damage (the only way to hurt a building
//! through the public API), the opponent repairs. Everything is
//! driven through ordinary commands — no state is poked.

use fixed::types::I16F16;
use skirmish_sim::{
    BuildingKind, Command, Entity, EntityKind, MatchMode, QueuedOrder, ResourceKind, Sim, Task,
    OWNER_OPPONENT, OWNER_PLAYER,
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

fn wood(sim: &Sim, owner: u8) -> f64 {
    f(sim.stockpile_of(owner).amount(ResourceKind::Wood))
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

/// A world point inside anchor cell `(cx, cy)` (its center).
fn anchor_point(sim: &Sim, cx: u32, cy: u32) -> (I16F16, I16F16) {
    sim.grid().cell_center(cx, cy)
}

/// The first anchor cell near entity `center` where `kind` may be
/// built by `entity` — the construction suite's ring scan, used
/// around the opponent's Town Center.
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
                let (x, y) = anchor_point(sim, ax, ay);
                if sim.can_build(entity, kind, x, y) {
                    return (ax, ay);
                }
            }
        }
    }
    panic!("no buildable ground near the Town Center");
}

struct Damaged {
    sim: Sim,
    bid: u32,
    /// HP after the attacker withdrew (f64 at the test boundary).
    hp: f64,
    opp_villagers: Vec<u32>,
}

/// Build an opponent building of `kind` with two builders, have
/// the human Scout beat it down to at most `stop_hp`, withdraw
/// the Scout, and return the scene. Both builders end back at
/// their Town Center (see the deadlock note inside), so a repair
/// order starts with a travel leg, like real play.
fn damaged_building(seed: u64, kind: BuildingKind, stop_hp: f64) -> Damaged {
    let mut sim = Sim::with_mode(seed, MatchMode::TwoPlayer);
    let opp_tc = find_id(&sim, OWNER_OPPONENT, EntityKind::TownCenter);
    let human_tc = find_id(&sim, OWNER_PLAYER, EntityKind::TownCenter);
    let opp_villagers = villagers_of(&sim, OWNER_OPPONENT);
    let human_scout = find_id(&sim, OWNER_PLAYER, EntityKind::ScoutCavalry);
    let opp_scout = find_id(&sim, OWNER_OPPONENT, EntityKind::ScoutCavalry);

    // Park the opponent's Scout far from the building site so the
    // damage phase is a clean one-sided beating.
    let (tc_x, tc_y) = {
        let tc = ent(&sim, opp_tc);
        (tc.pos_x, tc.pos_y)
    };
    sim.push_command(Command::Move {
        entity: opp_scout,
        x: tc_x + I16F16::from_num(80),
        y: tc_y + I16F16::from_num(80),
    });

    // Raise the building with two builders (resume semantics).
    let v1 = opp_villagers[0];
    let (ax, ay) = open_anchor_near(&sim, opp_tc, v1, kind);
    let (x, y) = anchor_point(&sim, ax, ay);
    sim.push_command(Command::Build {
        entity: v1,
        kind,
        x,
        y,
    });
    sim.tick();
    assert_eq!(sim.buildings().len(), 1, "placement was rejected");
    let bid = sim.buildings()[0].id;
    sim.push_command(Command::Build {
        entity: opp_villagers[1],
        kind,
        x,
        y,
    });
    sim.tick();
    assert_eq!(sim.buildings().len(), 1, "resume placed a duplicate");
    let up = run_until(&mut sim, 30_000, |s| building(s, bid).complete);
    assert!(up, "building never completed");

    // Send both builders back to their Town Center. They must not
    // be standing on the building's ring when the attacker
    // arrives: the building chase targets the first ring cell —
    // the same cell a builder idles on — and an idle villager
    // there jams the chase in a separation-vs-arrival deadlock
    // (observed while probing: the Scout held ~4 units off the
    // footprint forever, order active, never striking).
    for &v in &opp_villagers[..2] {
        sim.push_command(Command::Move {
            entity: v,
            x: tc_x,
            y: tc_y,
        });
    }

    // The human Scout beats it down to the threshold.
    sim.push_command(Command::AttackBuilding {
        entity: human_scout,
        building: bid,
    });
    // Stall breaker: the building chase approaches the footprint's
    // NW *corner* ring cell, and steering's arrival shortfall (up
    // to 1.0 short of the cell center) can leave a melee attacker
    // at up to ~3.8 from the footprint edge — beyond MELEE_REACH
    // (3.0) — while combat re-issues the identical approach
    // forever (observed in probing: 40 000 ticks, HP frozen). This
    // is a pre-existing sim edge, not repair behavior; the
    // fixture works around it by walking the Scout to a west
    // *edge* ring cell (2.0 from the footprint) whenever no damage
    // has landed for 3 000 ticks, then re-issuing the attack.
    let mut last_hp = f(building(&sim, bid).hp);
    let mut stall = 0u64;
    let mut ticks = 0u64;
    let beaten = loop {
        if f(building(&sim, bid).hp) <= stop_hp {
            break true;
        }
        if ticks > 400_000 {
            break false;
        }
        sim.tick();
        ticks += 1;
        let hp = f(building(&sim, bid).hp);
        if hp < last_hp {
            last_hp = hp;
            stall = 0;
        } else {
            stall += 1;
        }
        if stall >= 3_000 {
            stall = 0;
            let (wax, way) = {
                let b = building(&sim, bid);
                (b.cell_x - 1, b.cell_y + b.kind.footprint_size() / 2)
            };
            let (wx, wy) = anchor_point(&sim, wax, way);
            sim.push_command(Command::Move {
                entity: human_scout,
                x: wx,
                y: wy,
            });
            run_until(&mut sim, 10_000, |s| ent(s, human_scout).order.is_none());
            sim.push_command(Command::AttackBuilding {
                entity: human_scout,
                building: bid,
            });
        }
    };
    assert!(beaten, "the Scout never beat the building down");
    assert!(
        f(building(&sim, bid).hp) > 1.0,
        "the building must survive the beating"
    );

    // Withdraw the Scout home; the building's HP is stable from
    // the tick the Move lands (melee attacker, combat cleared).
    let htc = ent(&sim, human_tc);
    sim.push_command(Command::Move {
        entity: human_scout,
        x: htc.pos_x,
        y: htc.pos_y,
    });
    sim.tick();
    let hp = f(building(&sim, bid).hp);
    for _ in 0..30 {
        sim.tick();
    }
    assert_eq!(
        f(building(&sim, bid).hp),
        hp,
        "HP moved after the attacker withdrew"
    );

    Damaged {
        sim,
        bid,
        hp,
        opp_villagers,
    }
}

#[test]
fn repair_restores_hp_at_the_documented_rate_and_stops_at_full() {
    let mut fx = damaged_building(71, BuildingKind::House, 120.0);
    let bid = fx.bid;
    let v1 = fx.opp_villagers[0];
    assert!(
        fx.hp <= 120.0 && fx.hp >= 100.0,
        "fixture HP off target: {}",
        fx.hp
    );
    let wood_before = wood(&fx.sim, OWNER_OPPONENT);

    fx.sim.push_command(Command::Repair {
        entity: v1,
        building: bid,
    });
    // The repairer travels from the Town Center first; wait until
    // the restore actually begins.
    let active = run_until(&mut fx.sim, 20_000, |s| f(building(s, bid).hp) > fx.hp);
    assert!(active, "repair never started restoring HP");
    assert_eq!(
        ent(&fx.sim, v1).task,
        Task::Repair { building: bid },
        "the villager is not on repair duty"
    );

    // Ten seconds of solo repair restore exactly 125 HP
    // (750 HP/min = 12.5 HP/s), whatever the building's build
    // time (1500 ticks) or max HP: the flat DE rate.
    let hp0 = f(building(&fx.sim, bid).hp);
    for _ in 0..600 {
        fx.sim.tick();
    }
    let hp1 = f(building(&fx.sim, bid).hp);
    assert!(
        (hp1 - hp0 - 125.0).abs() < 0.05,
        "600 solo repair ticks restored {} HP, expected 125",
        hp1 - hp0
    );

    // Run to full: HP lands exactly on max, never above, and the
    // repairer stands down on its own.
    let full = run_until(&mut fx.sim, 10_000, |s| f(building(s, bid).hp) >= 550.0);
    assert!(full, "the house never reached full HP");
    assert_eq!(f(building(&fx.sim, bid).hp), 550.0, "overheal or undershoot");
    assert_eq!(
        ent(&fx.sim, v1).task,
        Task::Idle,
        "the repairer did not stand down at full HP"
    );

    // No overheal, no further charge after completion.
    let wood_after = wood(&fx.sim, OWNER_OPPONENT);
    for _ in 0..240 {
        fx.sim.tick();
    }
    assert_eq!(f(building(&fx.sim, bid).hp), 550.0);
    assert_eq!(wood(&fx.sim, OWNER_OPPONENT), wood_after);

    // Total drip for the whole repair: 50% of the 25-wood build
    // cost, scaled by the fraction of max HP actually restored.
    let restored = 550.0 - fx.hp;
    let expected = 25.0 * 0.5 * restored / 550.0;
    let spent = wood_before - wood_after;
    assert!(
        (spent - expected).abs() < 0.35,
        "full repair charged {spent} wood, expected ≈{expected}"
    );
}

/// Run one damaged-house scenario to full repair and report
/// (ticks from the repair order to full HP, wood spent, hash).
fn full_repair_run(seed: u64, repairer_slots: &[usize]) -> (u64, f64, String) {
    let mut fx = damaged_building(seed, BuildingKind::House, 120.0);
    let bid = fx.bid;
    let wood_before = wood(&fx.sim, OWNER_OPPONENT);
    for &slot in repairer_slots {
        fx.sim.push_command(Command::Repair {
            entity: fx.opp_villagers[slot],
            building: bid,
        });
    }
    let mut ticks = 0;
    let full = run_until(&mut fx.sim, 60_000, |s| {
        ticks += 1;
        f(building(s, bid).hp) >= 550.0
    });
    assert!(full, "the house never reached full HP");
    (
        ticks,
        wood_before - wood(&fx.sim, OWNER_OPPONENT),
        fx.sim.state_hash_hex(),
    )
}

#[test]
fn two_repairers_stack_at_the_documented_rate() {
    // Same seed, same damage: the only difference is the second
    // repairer. 750 + 375 = 1125 HP/min is exactly 1.5x the solo
    // rate, so the duo must finish in about 2/3 of the solo time
    // (both repairers start together at their Town Center, so the
    // travel leg is common to both runs and only dilutes the
    // ratio toward 1).
    let (solo_ticks, solo_wood, solo_hash) = full_repair_run(73, &[0]);
    let (duo_ticks, duo_wood, _) = full_repair_run(73, &[0, 1]);
    assert!(
        (duo_ticks as f64) < solo_ticks as f64 * 0.8,
        "two repairers took {duo_ticks} ticks vs solo {solo_ticks} — stacking is not alive"
    );
    // The total cost is the same either way (the drip prices HP
    // restored, not repairer time).
    assert!(
        (duo_wood - solo_wood).abs() < 0.05,
        "duo charged {duo_wood} wood, solo {solo_wood}"
    );
    // Determinism: the identical run replays bit-identically.
    let replay = full_repair_run(73, &[0]);
    assert_eq!(replay.0, solo_ticks);
    assert_eq!(replay.1, solo_wood);
    assert_eq!(replay.2, solo_hash);
}

#[test]
fn repair_rejects_enemy_undamaged_and_unfinished_buildings() {
    // (a) Enemy building: the human villager cannot repair the
    // opponent's damaged house — no task, no charge, no restore.
    let mut fx = damaged_building(79, BuildingKind::House, 120.0);
    let bid = fx.bid;
    let human_villager = villagers_of(&fx.sim, OWNER_PLAYER)[0];
    assert!(!fx.sim.can_repair(human_villager, bid));
    let opp_wood = wood(&fx.sim, OWNER_OPPONENT);
    fx.sim.push_command(Command::Repair {
        entity: human_villager,
        building: bid,
    });
    for _ in 0..10 {
        fx.sim.tick();
    }
    assert_eq!(ent(&fx.sim, human_villager).task, Task::Idle);
    assert_eq!(f(building(&fx.sim, bid).hp), fx.hp);
    assert_eq!(wood(&fx.sim, OWNER_OPPONENT), opp_wood);

    // (b) Undamaged building: a completed, full-HP house is not a
    // repair target.
    let mut sim = Sim::with_mode(81, MatchMode::TwoPlayer);
    let opp_tc = find_id(&sim, OWNER_OPPONENT, EntityKind::TownCenter);
    let opp_villagers = villagers_of(&sim, OWNER_OPPONENT);
    let v1 = opp_villagers[0];
    let (ax, ay) = open_anchor_near(&sim, opp_tc, v1, BuildingKind::House);
    let (x, y) = anchor_point(&sim, ax, ay);
    sim.push_command(Command::Build {
        entity: v1,
        kind: BuildingKind::House,
        x,
        y,
    });
    sim.tick();
    let bid = sim.buildings()[0].id;
    let up = run_until(&mut sim, 30_000, |s| building(s, bid).complete);
    assert!(up, "house never completed");
    assert!(!sim.can_repair(v1, bid), "a full-HP house took a repair order");
    let wood_now = wood(&sim, OWNER_OPPONENT);
    sim.push_command(Command::Repair {
        entity: v1,
        building: bid,
    });
    sim.tick();
    assert_eq!(ent(&sim, v1).task, Task::Idle);
    assert_eq!(wood(&sim, OWNER_OPPONENT), wood_now, "undamaged repair charged");

    // (c) Under construction: a foundation is resumed via Build,
    // never repaired — a second villager's Repair is a no-op and
    // the original builder keeps working.
    let (ax, ay) = open_anchor_near(&sim, opp_tc, v1, BuildingKind::House);
    let (x, y) = anchor_point(&sim, ax, ay);
    sim.push_command(Command::Build {
        entity: v1,
        kind: BuildingKind::House,
        x,
        y,
    });
    sim.tick();
    let foundations: Vec<u32> = sim
        .buildings()
        .iter()
        .filter(|b| !b.complete)
        .map(|b| b.id)
        .collect();
    assert_eq!(foundations.len(), 1);
    let fid = foundations[0];
    let v2 = opp_villagers[1];
    assert!(!sim.can_repair(v2, fid));
    let wood_now = wood(&sim, OWNER_OPPONENT);
    sim.push_command(Command::Repair {
        entity: v2,
        building: fid,
    });
    sim.tick();
    assert!(
        !matches!(ent(&sim, v2).task, Task::Repair { .. }),
        "a villager took repair duty on a foundation"
    );
    assert_eq!(wood(&sim, OWNER_OPPONENT), wood_now);
    for _ in 0..100 {
        sim.tick();
    }
    assert!(
        building(&sim, fid).progress_thirds > 0,
        "the original builder stopped working"
    );
}

#[test]
fn wood_exhaustion_stops_repair_and_the_repairer_goes_idle() {
    // A Barracks costs 175 of the 200 starting wood, leaving 25.
    // Beaten to <= 800 of 1200 HP, a full repair would cost
    // 0.5 x 175 x (deficit/1200) > 25 wood — the money runs out
    // mid-repair: the repairer stops and stands idle (BLD-07),
    // the stockpile never goes negative, HP freezes below full.
    let mut fx = damaged_building(89, BuildingKind::Barracks, 800.0);
    let bid = fx.bid;
    let v1 = fx.opp_villagers[0];
    assert_eq!(wood(&fx.sim, OWNER_OPPONENT), 25.0);
    assert!(fx.hp <= 800.0 && fx.hp > 400.0, "fixture HP: {}", fx.hp);

    fx.sim.push_command(Command::Repair {
        entity: v1,
        building: bid,
    });
    let stopped = run_until(&mut fx.sim, 120_000, |s| {
        ent(s, v1).task == Task::Idle && f(building(s, bid).hp) > fx.hp
    });
    assert!(stopped, "the repairer never stopped after the money ran out");
    let hp_stop = f(building(&fx.sim, bid).hp);
    assert!(
        hp_stop > 1_000.0,
        "only {hp_stop} HP reached — the 25 wood should buy ~340 HP"
    );
    assert!(hp_stop < 1_200.0, "the building reached full HP on 25 wood");
    let wood_left = wood(&fx.sim, OWNER_OPPONENT);
    assert!(
        (0.0..0.1).contains(&wood_left),
        "stockpile should be drained to under a tick's drip, got {wood_left}"
    );
    // And it stays stopped: no resume, no negative stockpile.
    for _ in 0..300 {
        fx.sim.tick();
    }
    assert_eq!(f(building(&fx.sim, bid).hp), hp_stop);
    assert_eq!(ent(&fx.sim, v1).task, Task::Idle);
    assert_eq!(wood(&fx.sim, OWNER_OPPONENT), wood_left);
}

#[test]
fn queued_repair_executes_when_the_villager_goes_idle() {
    let mut fx = damaged_building(97, BuildingKind::House, 120.0);
    let bid = fx.bid;
    let v2 = fx.opp_villagers[1];
    let opp_tc = find_id(&fx.sim, OWNER_OPPONENT, EntityKind::TownCenter);
    // A plain move first — ten cells east of the Town Center, a
    // real walk — with the repair queued behind it (SEL-12): the
    // repair sits in the queue while the move runs, then executes
    // on its own and finishes the job.
    let (tx, ty) = {
        let tc = ent(&fx.sim, opp_tc);
        let (cx, cy) = fx.sim.grid().world_to_cell(tc.pos_x, tc.pos_y);
        fx.sim.grid().cell_center(cx + 10, cy)
    };
    fx.sim.push_command(Command::Move {
        entity: v2,
        x: tx,
        y: ty,
    });
    fx.sim.push_command(Command::Queue {
        entity: v2,
        order: QueuedOrder::Repair { building: bid },
    });
    fx.sim.tick();
    assert_eq!(
        ent(&fx.sim, v2).order_queue.len(),
        1,
        "the repair did not queue behind the move"
    );
    assert!(
        !matches!(ent(&fx.sim, v2).task, Task::Repair { .. }),
        "the queued repair started before the move finished"
    );
    let full = run_until(&mut fx.sim, 60_000, |s| f(building(s, bid).hp) >= 550.0);
    assert!(full, "the queued repair never completed the house");
    assert_eq!(ent(&fx.sim, v2).task, Task::Idle);
}
