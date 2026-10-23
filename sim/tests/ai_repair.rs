//! AI repair soak (BLD-17, AI side): in a real AI match the
//! scripted opponent notices its damaged Barracks, sends
//! villagers to mend it through the ordinary repair path, the
//! building recovers to full once the attackers withdraw, and
//! the repairers are released back to work.
//!
//! The scene is driven entirely through public commands: the
//! human side raises a House and a Barracks from its starting
//! stockpile (200 wood exactly: 25 + 175), trains four Militia
//! (150 food / 60 gold of its 200 / 100), and marches them across
//! the map at the AI's Barracks the moment it stands. Everything
//! the AI does in response is its own script — no state is poked.
//! (The wood-floor gate and the exact drip the AI pays are pinned
//! by the in-crate tests in `sim/src/ai.rs`, where the stockpile
//! can be staged precisely; the repair mechanics themselves are
//! pinned by `sim/tests/repair.rs`.)

use fixed::types::I16F16;
use skirmish_sim::{
    BuildingKind, Command, Entity, EntityKind, MatchMode, Sim, Task, OWNER_OPPONENT, OWNER_PLAYER,
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

fn hp_of(sim: &Sim, bid: u32) -> Option<f64> {
    sim.buildings()
        .iter()
        .find(|b| b.id == bid)
        .map(|b| f(b.hp))
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
/// built by `entity` — the repair suite's ring scan.
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

fn living_militia(sim: &Sim) -> Vec<u32> {
    sim.entities()
        .iter()
        .filter(|e| e.owner == OWNER_PLAYER && e.kind == EntityKind::Militia && !e.dead)
        .map(|e| e.id)
        .collect()
}

#[test]
fn ai_repairs_its_barracks_under_attack() {
    let mut sim = Sim::new_match(7, MatchMode::AiNormal);
    let human_tc = find_id(&sim, OWNER_PLAYER, EntityKind::TownCenter);
    let human_villagers = villagers_of(&sim, OWNER_PLAYER);

    // The human raises a House and a Barracks at once (two
    // different builders) out of its starting wood.
    let (hx, hy) = {
        let (ax, ay) = open_anchor_near(&sim, human_tc, human_villagers[0], BuildingKind::House);
        anchor_point(&sim, ax, ay)
    };
    sim.push_command(Command::Build {
        entity: human_villagers[0],
        kind: BuildingKind::House,
        x: hx,
        y: hy,
    });
    let (bx, by) = {
        let (ax, ay) = open_anchor_near(&sim, human_tc, human_villagers[1], BuildingKind::Barracks);
        anchor_point(&sim, ax, ay)
    };
    sim.push_command(Command::Build {
        entity: human_villagers[1],
        kind: BuildingKind::Barracks,
        x: bx,
        y: by,
    });
    sim.tick();
    let human_barracks = sim
        .buildings()
        .iter()
        .find(|b| b.owner == OWNER_PLAYER && b.kind == BuildingKind::Barracks)
        .map(|b| b.id)
        .expect("human Barracks placement was rejected");

    // Train the squad of four: re-issue TrainAt whenever the
    // completed Barracks has queue room for another Militia.
    let mut guard = 0u64;
    while living_militia(&sim).len() < 4 && guard < 60_000 {
        let b = building(&sim, human_barracks);
        if b.complete && living_militia(&sim).len() + b.queue.len() < 4 {
            sim.push_command(Command::TrainAt {
                building: human_barracks,
                kind: EntityKind::Militia,
            });
        }
        sim.tick();
        guard += 1;
    }
    assert!(
        living_militia(&sim).len() >= 4,
        "the human never fielded four Militia (has {})",
        living_militia(&sim).len()
    );

    // Wait for the AI's own Barracks to stand complete.
    let ai_barracks = run_until(&mut sim, 90_000, |s| {
        s.buildings().iter().any(|b| {
            b.owner == OWNER_OPPONENT && b.kind == BuildingKind::Barracks && b.complete
        })
    });
    assert!(ai_barracks, "the AI never completed a Barracks");
    let bid = sim
        .buildings()
        .iter()
        .find(|b| b.owner == OWNER_OPPONENT && b.kind == BuildingKind::Barracks && b.complete)
        .map(|b| b.id)
        .unwrap();
    let max_hp = hp_of(&sim, bid).expect("the AI Barracks exists");
    assert!(max_hp > 100.0, "a fresh Barracks stands at full HP");

    // Send the squad at it.
    for mid in living_militia(&sim) {
        sim.push_command(Command::AttackBuilding {
            entity: mid,
            building: bid,
        });
    }

    // Attack phase: up to ~12 game-minutes. Track the HP dip and
    // whether an AI villager ever takes the repair assignment.
    // Stall breaker (the repair suite's documented melee-vs-
    // building approach edge): if no damage lands for 3 000 ticks
    // while attackers live, walk the squad onto the building's
    // west-edge ring cells and re-issue the attack.
    let mut min_hp = max_hp;
    let mut saw_repair = false;
    let mut last_hp = max_hp;
    let mut stall = 0u64;
    let mut ticks = 0u64;
    loop {
        if ticks > 45_000 || living_militia(&sim).is_empty() {
            break;
        }
        // Stop early once the story is proven: damage landed,
        // repair answered, and the building is whole again even
        // while the squad is still swinging at it.
        if saw_repair && min_hp <= max_hp - 10.0 && hp_of(&sim, bid) == Some(max_hp) {
            break;
        }
        sim.tick();
        ticks += 1;
        let hp = hp_of(&sim, bid).expect("the AI Barracks was destroyed under repair");
        if hp < min_hp {
            min_hp = hp;
        }
        saw_repair |= sim
            .entities()
            .iter()
            .any(|e| e.owner == OWNER_OPPONENT && e.task == (Task::Repair { building: bid }));
        if hp < last_hp {
            last_hp = hp;
            stall = 0;
        } else {
            stall += 1;
        }
        if stall >= 3_000 {
            stall = 0;
            let squad = living_militia(&sim);
            let (cell_x, cell_y, size) = {
                let b = building(&sim, bid);
                (b.cell_x, b.cell_y, b.kind.footprint_size())
            };
            if cell_x >= 1 {
                for (i, mid) in squad.iter().enumerate() {
                    let (wx, wy) = anchor_point(&sim, cell_x - 1, cell_y + (i as u32) % size);
                    sim.push_command(Command::Move {
                        entity: *mid,
                        x: wx,
                        y: wy,
                    });
                }
                run_until(&mut sim, 10_000, |s| {
                    living_militia(s).iter().all(|m| ent(s, *m).order.is_none())
                });
                for mid in living_militia(&sim) {
                    sim.push_command(Command::AttackBuilding {
                        entity: mid,
                        building: bid,
                    });
                }
                last_hp = hp_of(&sim, bid).expect("the AI Barracks exists");
            }
        }
    }
    assert!(
        min_hp <= max_hp - 10.0,
        "the squad never dented the Barracks (min {min_hp:.1} of {max_hp:.1})"
    );
    assert!(
        saw_repair,
        "no AI villager ever took the repair assignment for its Barracks"
    );

    // Withdraw the survivors home; the repairers finish the job
    // and are released by the sim's own full-HP rule.
    let htc = ent(&sim, human_tc);
    let (htc_x, htc_y) = (htc.pos_x, htc.pos_y);
    for mid in living_militia(&sim) {
        sim.push_command(Command::Move {
            entity: mid,
            x: htc_x,
            y: htc_y,
        });
    }
    let recovered = run_until(&mut sim, 45_000, |s| {
        hp_of(s, bid) == Some(max_hp)
            && !s
                .entities()
                .iter()
                .any(|e| e.owner == OWNER_OPPONENT && matches!(e.task, Task::Repair { .. }))
    });
    assert!(
        recovered,
        "the Barracks never returned to full HP with repairers released (HP {:?} of {max_hp:.1})",
        hp_of(&sim, bid)
    );
}
