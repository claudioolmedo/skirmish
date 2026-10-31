//! Town Center repair tests (Package J, BLD-07's TC slice +
//! BLD-17 parity): the Town Center is an ENTITY in this
//! architecture (BLD-16), so `Command::Repair`'s building-record
//! path can never name it — `Command::RepairTc` is the sibling
//! path. Everything else is the buildings' documented model,
//! mirrored: 750 HP/min for the first repairer, +375 HP/min per
//! additional; the stop-and-idle rule when a tick's drip is
//! unaffordable; the queued form; the Town Bell snapshot. The
//! ONE documented difference is the price: a full Town Center
//! repair costs a flat 550 wood and NO stone (wiki, Villager —
//! the TC exception to the generic 50%-of-build-cost formula).
//!
//! Scenarios are driven through ordinary commands in TwoPlayer
//! matches (the opponent's Scout does the damage — the only way
//! to hurt a Town Center through the public API), plus one
//! AiNormal soak for the BLD-17 parity side. No state is poked.

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

fn wood(sim: &Sim, owner: u8) -> f64 {
    f(sim.stockpile_of(owner).amount(ResourceKind::Wood))
}

fn stone(sim: &Sim, owner: u8) -> f64 {
    f(sim.stockpile_of(owner).amount(ResourceKind::Stone))
}

fn tc_hp(sim: &Sim, owner: u8) -> f64 {
    let tc = find_id(sim, owner, EntityKind::TownCenter);
    f(ent(sim, tc).hp)
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

/// Dent the PLAYER's Town Center with the opponent's Scout:
/// order the attack, wait until the TC has lost at least
/// `amount` HP, then order the Scout far away and wait until it
/// has left the TC's neighborhood (so the repair scene starts
/// clean). Returns the HP the TC stands at.
fn dent_player_tc(sim: &mut Sim, amount: f64) -> f64 {
    let tc = find_id(sim, OWNER_PLAYER, EntityKind::TownCenter);
    let scout = find_id(sim, OWNER_OPPONENT, EntityKind::ScoutCavalry);
    let full = tc_hp(sim, OWNER_PLAYER);
    sim.push_command(Command::Attack {
        entity: scout,
        target: tc,
    });
    let dented = run_until(sim, 240_000, |s| tc_hp(s, OWNER_PLAYER) <= full - amount);
    assert!(dented, "opponent Scout never dented the Town Center");
    // Withdraw: send the Scout back toward its own Town Center.
    let opp_tc = find_id(sim, OWNER_OPPONENT, EntityKind::TownCenter);
    let (x, y) = {
        let t = ent(sim, opp_tc);
        (t.pos_x, t.pos_y)
    };
    sim.push_command(Command::Move {
        entity: scout,
        x,
        y,
    });
    let gone = run_until(sim, 120_000, |s| {
        let e = ent(s, scout);
        let t = ent(s, find_id(s, OWNER_PLAYER, EntityKind::TownCenter));
        let dx = f(e.pos_x - t.pos_x);
        let dy = f(e.pos_y - t.pos_y);
        dx * dx + dy * dy > 40.0 * 40.0
    });
    assert!(gone, "opponent Scout never withdrew");
    tc_hp(sim, OWNER_PLAYER)
}

#[test]
fn repair_tc_validation_matrix() {
    let mut sim = Sim::with_mode(61, MatchMode::TwoPlayer);
    let tc = find_id(&sim, OWNER_PLAYER, EntityKind::TownCenter);
    let vil = villagers_of(&sim, OWNER_PLAYER)[0];
    let scout = find_id(&sim, OWNER_PLAYER, EntityKind::ScoutCavalry);
    let opp_vil = villagers_of(&sim, OWNER_OPPONENT)[0];
    // Undamaged: nobody may repair a full Town Center.
    assert!(!sim.can_repair_tc(vil, tc));
    // The placed-building path can never name the TC either.
    assert!(!sim.can_repair(vil, tc));
    dent_player_tc(&mut sim, 30.0);
    // Damaged: the owner's villager may; everyone else may not.
    assert!(sim.can_repair_tc(vil, tc));
    assert!(!sim.can_repair_tc(scout, tc), "a Scout is not a repairer");
    assert!(!sim.can_repair_tc(opp_vil, tc), "an enemy villager cannot repair it");
    let opp_tc = find_id(&sim, OWNER_OPPONENT, EntityKind::TownCenter);
    assert!(!sim.can_repair_tc(vil, opp_tc), "not our Town Center");
    // A rejected command changes nothing (no task is assigned).
    sim.push_command(Command::RepairTc {
        entity: opp_vil,
        tc,
    });
    sim.tick();
    assert_eq!(ent(&sim, opp_vil).task, Task::Idle);
}

#[test]
fn repair_tc_restores_at_the_documented_rate() {
    let mut sim = Sim::with_mode(62, MatchMode::TwoPlayer);
    let tc = find_id(&sim, OWNER_PLAYER, EntityKind::TownCenter);
    let vil = villagers_of(&sim, OWNER_PLAYER)[0];
    dent_player_tc(&mut sim, 320.0);
    // One repairer: 750 HP/min ⇒ +125 HP over a 600-tick window
    // once it stands in range at the ring.
    sim.push_command(Command::RepairTc { entity: vil, tc });
    let working = run_until(&mut sim, 120_000, |s| {
        ent(s, vil).task == (Task::RepairTc { tc }) && ent(s, vil).order.is_none()
    });
    assert!(working, "repairer never settled at the Town Center ring");
    let hp0 = tc_hp(&sim, OWNER_PLAYER);
    for _ in 0..600 {
        sim.tick();
    }
    let gained = tc_hp(&sim, OWNER_PLAYER) - hp0;
    assert!(
        (118.0..=132.0).contains(&gained),
        "solo TC repair restored {gained} HP in 600 ticks, expected ≈125 (750 HP/min)"
    );
}

#[test]
fn repair_tc_second_repairer_adds_375_per_min() {
    let mut sim = Sim::with_mode(63, MatchMode::TwoPlayer);
    let tc = find_id(&sim, OWNER_PLAYER, EntityKind::TownCenter);
    let vils = villagers_of(&sim, OWNER_PLAYER);
    dent_player_tc(&mut sim, 320.0);
    for v in [vils[0], vils[1]] {
        sim.push_command(Command::RepairTc { entity: v, tc });
    }
    let working = run_until(&mut sim, 120_000, |s| {
        [vils[0], vils[1]].iter().all(|&v| {
            ent(s, v).task == (Task::RepairTc { tc }) && ent(s, v).order.is_none()
        })
    });
    assert!(working, "both repairers never settled at the ring");
    let hp0 = tc_hp(&sim, OWNER_PLAYER);
    for _ in 0..600 {
        sim.tick();
    }
    let gained = tc_hp(&sim, OWNER_PLAYER) - hp0;
    // 750 + 375 = 1125 HP/min ⇒ +187.5 HP over 600 ticks.
    assert!(
        (178.0..=197.0).contains(&gained),
        "two-repairer TC repair restored {gained} HP in 600 ticks, expected ≈187.5"
    );
}

#[test]
fn repair_tc_full_repair_costs_550_wood_and_no_stone() {
    let mut sim = Sim::with_mode(64, MatchMode::TwoPlayer);
    let tc = find_id(&sim, OWNER_PLAYER, EntityKind::TownCenter);
    let vil = villagers_of(&sim, OWNER_PLAYER)[0];
    let hp_after = dent_player_tc(&mut sim, 300.0);
    let max_hp = sim.stats_for(OWNER_PLAYER, EntityKind::TownCenter).max_hp as f64;
    let missing = max_hp - hp_after;
    assert!(missing >= 300.0);
    let wood0 = wood(&sim, OWNER_PLAYER);
    let stone0 = stone(&sim, OWNER_PLAYER);
    sim.push_command(Command::RepairTc { entity: vil, tc });
    let full = run_until(&mut sim, 240_000, |s| tc_hp(s, OWNER_PLAYER) >= max_hp - 0.01);
    assert!(full, "Town Center never repaired to full");
    // The repairer is released at full HP.
    assert_eq!(ent(&sim, vil).task, Task::Idle);
    // The exception price: 550 wood × missing / max, no stone.
    let expected = 550.0 * missing / max_hp;
    let spent = wood0 - wood(&sim, OWNER_PLAYER);
    assert!(
        (spent - expected).abs() <= 2.0,
        "TC repair spent {spent} wood, expected ≈{expected} (550 × {missing}/{max_hp})"
    );
    assert_eq!(stone(&sim, OWNER_PLAYER), stone0, "TC repair must never charge stone");
}

#[test]
fn repair_tc_stops_when_the_wood_runs_out() {
    let mut sim = Sim::with_mode(65, MatchMode::TwoPlayer);
    let tc = find_id(&sim, OWNER_PLAYER, EntityKind::TownCenter);
    let vils = villagers_of(&sim, OWNER_PLAYER);
    // Drain the player's 200 starting wood to exactly 0 through
    // real orders: a Barracks (175) plus a House (25), built by
    // the other villagers while the Scout does its damage.
    let tc_ent = ent(&sim, tc);
    let (tcx, tcy) = sim.grid().world_to_cell(tc_ent.pos_x, tc_ent.pos_y);
    let mut anchors: Vec<(u32, u32)> = Vec::new();
    'scan: for r in 2..=14i32 {
        for dy in -r..=r {
            for dx in -r..=r {
                if dx.abs() != r && dy.abs() != r {
                    continue;
                }
                let (ax, ay) = (tcx as i32 + dx, tcy as i32 + dy);
                if ax < 0 || ay < 0 {
                    continue;
                }
                anchors.push((ax as u32, ay as u32));
                if anchors.len() == 2 {
                    break 'scan;
                }
            }
        }
    }
    let kinds = [BuildingKind::Barracks, BuildingKind::House];
    for (i, (ax, ay)) in anchors.iter().enumerate() {
        let (x, y) = sim.grid().cell_center(*ax, *ay);
        // Find the first anchor the sim accepts for this kind.
        let mut cell = (*ax, *ay);
        let mut pt = (x, y);
        if !sim.can_build(vils[i + 1], kinds[i], x, y) {
            let mut found = false;
            for r in 2..=20i32 {
                for dy in -r..=r {
                    for dx in -r..=r {
                        if dx.abs() != r && dy.abs() != r {
                            continue;
                        }
                        let (nx, ny) = (tcx as i32 + dx, tcy as i32 + dy);
                        if nx < 0 || ny < 0 {
                            continue;
                        }
                        let (nx, ny) = (nx as u32, ny as u32);
                        let (px, py) = sim.grid().cell_center(nx, ny);
                        if sim.can_build(vils[i + 1], kinds[i], px, py) {
                            cell = (nx, ny);
                            pt = (px, py);
                            found = true;
                            break;
                        }
                    }
                    if found {
                        break;
                    }
                }
                if found {
                    break;
                }
            }
            assert!(found, "no anchor for {:?}", kinds[i]);
            let _ = cell;
        }
        sim.push_command(Command::Build {
            entity: vils[i + 1],
            kind: kinds[i],
            x: pt.0,
            y: pt.1,
        });
    }
    let drained = run_until(&mut sim, 240_000, |s| {
        s.buildings()
            .iter()
            .filter(|b| b.owner == OWNER_PLAYER && b.complete)
            .count()
            >= 2
    });
    assert!(drained, "the drain buildings never completed");
    assert_eq!(wood(&sim, OWNER_PLAYER), 0.0);
    let hp_after = dent_player_tc(&mut sim, 200.0);
    // Repair with an empty purse: the first tick's drip is
    // already unaffordable, so the repairer stands down at once
    // and nothing is restored (the same stop rule as buildings).
    sim.push_command(Command::RepairTc {
        entity: vils[0],
        tc,
    });
    for _ in 0..7200 {
        sim.tick();
    }
    let restored = tc_hp(&sim, OWNER_PLAYER) - hp_after;
    assert!(
        restored < 20.0,
        "broke repair restored {restored} HP — it should stall almost at once"
    );
    assert_eq!(
        ent(&sim, vils[0]).task,
        Task::Idle,
        "an unfunded TC repairer must stand down to idle"
    );
}

#[test]
fn repair_tc_queued_form_executes_after_current_order() {
    let mut sim = Sim::with_mode(66, MatchMode::TwoPlayer);
    let tc = find_id(&sim, OWNER_PLAYER, EntityKind::TownCenter);
    let vil = villagers_of(&sim, OWNER_PLAYER)[0];
    dent_player_tc(&mut sim, 60.0);
    // Queue a move first, then the TC repair behind it.
    let t = ent(&sim, tc);
    let (x, y) = (t.pos_x + I16F16::from_num(30), t.pos_y);
    sim.push_command(Command::Move { entity: vil, x, y });
    assert!(sim.can_queue(vil, QueuedOrder::RepairTc { tc }));
    sim.push_command(Command::Queue {
        entity: vil,
        order: QueuedOrder::RepairTc { tc },
    });
    let took = run_until(&mut sim, 120_000, |s| {
        ent(s, vil).task == (Task::RepairTc { tc })
    });
    assert!(took, "queued RepairTc never executed");
    let healed = run_until(&mut sim, 240_000, |s| {
        let max_hp = s.stats_for(OWNER_PLAYER, EntityKind::TownCenter).max_hp as f64;
        tc_hp(s, OWNER_PLAYER) >= max_hp - 0.01
    });
    assert!(healed, "queued TC repair never finished the job");
}

#[test]
fn ai_repairs_its_own_town_center() {
    // BLD-17 parity: in a real AiNormal match the scripted
    // opponent notices its dented Town Center and mends it
    // through the same RepairTc path a player uses — same caps,
    // same drip — once the attacker is gone.
    let mut sim = Sim::new_match(67, MatchMode::AiNormal);
    let ai_tc = find_id(&sim, OWNER_OPPONENT, EntityKind::TownCenter);
    let scout = find_id(&sim, OWNER_PLAYER, EntityKind::ScoutCavalry);
    let full = tc_hp(&sim, OWNER_OPPONENT);
    sim.push_command(Command::Attack {
        entity: scout,
        target: ai_tc,
    });
    // Package J landing note (bot-loop, 2026-10-30): the dent is
    // delivered by the human's WHOLE starting force — the Scout
    // plus its three villagers, all through ordinary Attack
    // orders. On Package J's own base a lone Scout sufficed, but
    // after the combo landing (P4a + K + L) the same seed's AI
    // economy funds its BLD-17 TC repair continuously, and the
    // repair out-heals a lone Scout: measured on this seed, the
    // TC never fell below 2393/2400 in 240,000 ticks and the
    // Scout died at ~65k with the dent never past −7. The repair
    // itself is the documented behaviour under test (it must not
    // be weakened to save the scenario), so the scenario's dent
    // force grows instead — the combined minimum-damage rate of
    // four attackers out-paces the wood-drip-limited repair.
    let attackers = villagers_of(&sim, OWNER_PLAYER);
    for vil in &attackers {
        sim.push_command(Command::Attack {
            entity: *vil,
            target: ai_tc,
        });
    }
    let dented = run_until(&mut sim, 240_000, |s| tc_hp(s, OWNER_OPPONENT) <= full - 25.0);
    assert!(dented, "human starting force never dented the AI Town Center");
    // Withdraw the attackers for good (the human deletes its own
    // units — an ordinary SEL-14 order).
    sim.push_command(Command::DeleteEntity { entity: scout });
    for vil in &attackers {
        sim.push_command(Command::DeleteEntity { entity: *vil });
    }
    // The AI's script must pick the repair up: some AI villager
    // takes RepairTc duty, and the Town Center returns to full.
    let mut saw_repairer = false;
    let mut healed = false;
    for _ in 0..360_000 {
        sim.tick();
        if sim
            .entities()
            .iter()
            .any(|e| e.owner == OWNER_OPPONENT && matches!(e.task, Task::RepairTc { .. }))
        {
            saw_repairer = true;
        }
        if tc_hp(&sim, OWNER_OPPONENT) >= full - 0.01 {
            healed = true;
            break;
        }
    }
    assert!(saw_repairer, "no AI villager ever took Town Center repair duty");
    assert!(healed, "AI Town Center never returned to full HP");
}
