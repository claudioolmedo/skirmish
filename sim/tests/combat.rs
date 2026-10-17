//! Combat tests (M3): the standard start (Scout + Gaia herds), sheep
//! capture/kill/carcass decay, the boar lure loop, hunt carry 35,
//! archer projectiles with RNG replay discipline, Barracks production
//! and population interplay, death, and MOV-04 separation suppression.
//!
//! Entity ids in the standard start: 0 Town Center, 1–3 villagers,
//! 4 Scout Cavalry, 5–8 near sheep, 9–12 far sheep, 13–14 boars,
//! 15–17 deer (see `ANIMAL_SPECS` in lib.rs).

use fixed::types::I16F16;
use skirmish_sim::{
    BuildingKind, Command, Entity, EntityKind, NodeKind, Sim, OWNER_GAIA, OWNER_PLAYER,
};

fn f(v: I16F16) -> f64 {
    v.to_num::<f64>()
}

fn ent(sim: &Sim, id: u32) -> &Entity {
    sim.entities().iter().find(|e| e.id == id).unwrap()
}

fn dist(a: &Entity, b: &Entity) -> f64 {
    let dx = f(a.pos_x) - f(b.pos_x);
    let dy = f(a.pos_y) - f(b.pos_y);
    (dx * dx + dy * dy).sqrt()
}

fn cell_point(sim: &Sim, cx: u32, cy: u32) -> (I16F16, I16F16) {
    sim.grid().cell_center(cx, cy)
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

#[test]
fn standard_start_has_scout_and_gaia_herds() {
    let sim = Sim::new(7);
    assert_eq!(sim.entities().len(), 18);
    let scout = ent(&sim, 4);
    assert_eq!(scout.kind, EntityKind::ScoutCavalry);
    assert_eq!(scout.owner, OWNER_PLAYER);
    assert_eq!(scout.hp, I16F16::from_num(45));
    // Beside the Town Center: TC at (-6,-6), scout offset (+5,+5).
    let tc = sim.town_center();
    assert!(dist(scout, tc) < 8.0, "scout is not beside the TC");
    // The Scout occupies a population slot: 3 villagers + scout = 4.
    assert_eq!(sim.population(), 4);
    assert_eq!(sim.living_population(), 4);
    let count = |k: EntityKind| {
        sim.entities().iter().filter(|e| e.kind == k).count()
    };
    assert_eq!(count(EntityKind::Sheep), 8);
    assert_eq!(count(EntityKind::Boar), 2);
    assert_eq!(count(EntityKind::Deer), 3);
    assert!(
        sim.entities()
            .iter()
            .filter(|e| e.kind == EntityKind::Sheep
                || e.kind == EntityKind::Boar
                || e.kind == EntityKind::Deer)
            .all(|e| e.owner == OWNER_GAIA),
        "animals must start as Gaia"
    );
    // Boars hold their posts when nobody provokes them.
    let mut sim = Sim::new(7);
    let (hx, hy) = (f(ent(&sim, 13).pos_x), f(ent(&sim, 13).pos_y));
    for _ in 0..600 {
        sim.tick();
    }
    let b = ent(&sim, 13);
    assert!(!b.dead && b.hp == I16F16::from_num(75));
    assert!((f(b.pos_x) - hx).abs() < 0.01 && (f(b.pos_y) - hy).abs() < 0.01);
}

#[test]
fn sheep_captured_by_proximity_then_taskable() {
    let mut sim = Sim::new(11);
    // Far sheep 9 at cell (13,18). Walk villager 1 up to it.
    let (sx, sy) = cell_point(&sim, 13, 18);
    sim.push_command(Command::Move {
        entity: 1,
        x: sx,
        y: sy,
    });
    let captured = run_until(&mut sim, 3000, |s| ent(s, 9).owner == OWNER_PLAYER);
    assert!(captured, "sheep was never captured by proximity");
    // A captured sheep takes move orders like a unit.
    let (ox, oy) = (f(ent(&sim, 9).pos_x), f(ent(&sim, 9).pos_y));
    sim.push_command(Command::Move {
        entity: 9,
        x: I16F16::from_num(-30),
        y: I16F16::from_num(-26),
    });
    for _ in 0..600 {
        sim.tick();
    }
    let s = ent(&sim, 9);
    let moved = ((f(s.pos_x) - ox).powi(2) + (f(s.pos_y) - oy).powi(2)).sqrt();
    assert!(moved > 2.5, "captured sheep did not move (moved {moved:.2})");
}

#[test]
fn sheep_kill_leaves_a_decaying_carcass_the_killer_harvests() {
    let mut sim = Sim::new(13);
    // Near sheep 5. Villager 1 attacks it bare-handed (3 dmg vs 7 HP).
    sim.push_command(Command::Attack { entity: 1, target: 5 });
    let died = run_until(&mut sim, 5000, |s| ent(s, 5).dead);
    assert!(died, "villager never killed the sheep");
    let carcass = sim
        .nodes()
        .iter()
        .find(|n| n.kind == NodeKind::CarcassHerd)
        .expect("no herdable carcass node")
        .clone();
    assert!(carcass.amount_left <= I16F16::from_num(100));
    assert!(carcass.amount_left > I16F16::from_num(99));
    // The killer starts harvesting the carcass unprompted.
    assert_eq!(ent(&sim, 1).task_node, Some(carcass.id));
    // Move the killer away: with nobody working it, the carcass must
    // decay at exactly 273 bits/tick (0.25 food/s, ECO-12).
    sim.push_command(Command::Move {
        entity: 1,
        x: I16F16::from_num(60),
        y: I16F16::from_num(60),
    });
    sim.tick();
    let a0 = sim
        .nodes()
        .iter()
        .find(|n| n.id == carcass.id)
        .unwrap()
        .amount_left;
    for _ in 0..600 {
        sim.tick();
    }
    let a1 = sim
        .nodes()
        .iter()
        .find(|n| n.id == carcass.id)
        .unwrap()
        .amount_left;
    assert_eq!(
        a0 - a1,
        I16F16::from_bits(273 * 600),
        "carcass decay is not exactly 0.25 food/s"
    );
}

#[test]
fn boar_lure_loop_aggro_chase_give_up_return_heal() {
    let mut sim = Sim::new(17);
    let boar_home = {
        let b = ent(&sim, 13);
        (f(b.pos_x), f(b.pos_y))
    };
    // Scout rides to the boar and pokes it once (a single military
    // hit does NOT lock the aggro — the spec's lure rule).
    let (bx, by) = (I16F16::from_num(boar_home.0), I16F16::from_num(boar_home.1));
    sim.push_command(Command::Move { entity: 4, x: bx, y: by });
    let close = run_until(&mut sim, 3000, |s| dist(ent(s, 4), ent(s, 13)) < 4.0);
    assert!(close, "scout never reached the boar");
    sim.push_command(Command::Attack { entity: 4, target: 13 });
    let provoked = run_until(&mut sim, 1000, |s| {
        ent(s, 13).aggro_target == Some(4) && ent(s, 13).hp < I16F16::from_num(75)
    });
    assert!(provoked, "boar did not aggro on its attacker");
    // The scout bolts for the far corner; the un-locked boar must
    // give up, walk home, and heal.
    sim.push_command(Command::Move {
        entity: 4,
        x: I16F16::from_num(90),
        y: I16F16::from_num(90),
    });
    let mut max_home_dist = 0.0f64;
    let mut healed_home = false;
    for _ in 0..8000 {
        sim.tick();
        let b = ent(&sim, 13);
        let d = ((f(b.pos_x) - boar_home.0).powi(2) + (f(b.pos_y) - boar_home.1).powi(2)).sqrt();
        max_home_dist = max_home_dist.max(d);
        if b.aggro_target.is_none() && d < 2.0 && b.hp == I16F16::from_num(75) {
            healed_home = true;
            break;
        }
    }
    assert!(max_home_dist > 8.0, "boar never chased (max {max_home_dist:.1})");
    assert!(healed_home, "boar did not give up, return, and heal");
    assert!(!ent(&sim, 4).dead, "the scout should outrun the boar");
}

#[test]
fn villager_dies_to_boar_population_drops_tombstone_stays() {
    let mut sim = Sim::new(19);
    assert_eq!(sim.population(), 4);
    // One villager alone against a boar: the villager's hit locks
    // the aggro, and 7-damage strikes kill 25 HP in four hits.
    sim.push_command(Command::Attack { entity: 1, target: 13 });
    let died = run_until(&mut sim, 20000, |s| ent(s, 1).dead);
    assert!(died, "the lone villager survived the boar");
    let v = ent(&sim, 1);
    assert!(v.hp < I16F16::from_num(1), "death is HP < 1 (CMB-02)");
    assert_eq!(sim.villager_count(), 2);
    assert_eq!(sim.population(), 3, "death must free the pop slot the same tick");
    // Tombstone: the entity is still in the list (ids stay stable),
    // out of play — and the boar, its victim gone, walks home.
    assert_eq!(sim.entities().len(), 18);
    let recovered = run_until(&mut sim, 8000, |s| {
        let b = ent(s, 13);
        b.aggro_target.is_none() && b.hp == I16F16::from_num(75)
    });
    assert!(recovered, "boar did not stand down after the kill");
}

#[test]
fn boar_kill_leaves_340_carcass_and_hunt_carry_is_exactly_35() {
    let mut sim = Sim::new(23);
    // East boar (14): the whole village piles in; villager hits lock
    // the aggro and the damage is spread across attackers.
    for id in [1, 2, 3, 4] {
        sim.push_command(Command::Attack { entity: id, target: 14 });
    }
    let died = run_until(&mut sim, 30000, |s| ent(s, 14).dead);
    assert!(died, "the village never killed the boar");
    let carcass = sim
        .nodes()
        .iter()
        .find(|n| n.kind == NodeKind::CarcassHunt)
        .expect("no hunt carcass node")
        .clone();
    assert!(carcass.amount_left <= I16F16::from_num(340));
    assert!(carcass.amount_left > I16F16::from_num(300));
    // Hunt carry: a villager working the carcass fills to exactly 35.
    let hunter = (1..=3u32)
        .find(|&id| !ent(&sim, id).dead)
        .expect("every villager died killing one boar");
    if ent(&sim, hunter).task_node != Some(carcass.id) {
        sim.push_command(Command::Gather {
            entity: hunter,
            node: carcass.id,
        });
    }
    let mut saw_full_carry = false;
    for _ in 0..30000 {
        sim.tick();
        let v = ent(&sim, hunter);
        assert!(
            v.carry <= I16F16::from_num(35),
            "carry exceeded the hunt cap: {}",
            f(v.carry)
        );
        if v.carry == I16F16::from_num(35) {
            saw_full_carry = true;
            break;
        }
    }
    assert!(saw_full_carry, "villager never filled a 35-meat carry");
}

#[test]
fn melee_fight_replays_bit_identically() {
    fn run() -> Vec<u64> {
        let mut sim = Sim::new(29);
        let mut hashes = Vec::new();
        for id in [1, 2, 3, 4] {
            sim.push_command(Command::Attack { entity: id, target: 13 });
        }
        for _ in 0..6000 {
            hashes.push(sim.tick());
        }
        assert!(ent(&sim, 13).dead, "scripted fight did not happen");
        hashes
    }
    assert_eq!(run(), run(), "identical fight scripts diverged");
}

/// Scripted build-up shared by the archer test: Barracks east of the
/// wall, then an Archer trained there. Returns nothing; asserts as it
/// goes.
fn build_barracks_and_archer(sim: &mut Sim) {
    let (x, y) = cell_point(sim, 27, 20);
    sim.push_command(Command::Build {
        entity: 1,
        kind: BuildingKind::Barracks,
        x,
        y,
    });
    let built = run_until(sim, 30000, |s| {
        s.buildings().iter().any(|b| b.kind == BuildingKind::Barracks && b.complete)
    });
    assert!(built, "barracks never completed");
    let bid = sim
        .buildings()
        .iter()
        .find(|b| b.kind == BuildingKind::Barracks)
        .unwrap()
        .id;
    assert!(sim.can_train_at(bid, EntityKind::Archer));
    sim.push_command(Command::TrainAt {
        building: bid,
        kind: EntityKind::Archer,
    });
    let trained = run_until(sim, 10000, |s| {
        s.entities().iter().any(|e| e.kind == EntityKind::Archer)
    });
    assert!(trained, "archer never trained");
}

#[test]
fn archer_kills_at_range_and_the_rng_replay_is_bit_identical() {
    fn run() -> (Vec<u64>, bool, bool) {
        let mut sim = Sim::new(31);
        build_barracks_and_archer(&mut sim);
        let archer = sim
            .entities()
            .iter()
            .find(|e| e.kind == EntityKind::Archer)
            .unwrap()
            .id;
        sim.push_command(Command::Attack { entity: archer, target: 8 });
        let mut hashes = Vec::new();
        let mut saw_projectile = false;
        let mut kill_distance = 0.0;
        for _ in 0..24000 {
            hashes.push(sim.tick());
            if !sim.projectiles().is_empty() {
                saw_projectile = true;
            }
            if ent(&sim, 8).dead {
                kill_distance = dist(ent(&sim, archer), ent(&sim, 8));
                break;
            }
        }
        (hashes, saw_projectile, ent(&sim, 8).dead && kill_distance > 3.0)
    }

    let (h1, proj1, ranged_kill1) = run();
    let (h2, proj2, ranged_kill2) = run();
    assert!(proj1 && proj2, "no projectile was ever in flight");
    assert!(ranged_kill1, "archer did not kill the sheep from range");
    assert!(ranged_kill2);
    // The accuracy rolls come from the seeded RNG: identical scripts
    // must produce identical hash streams, roll for roll (SIM-04).
    assert_eq!(h1, h2, "archer fight diverged between identical runs");
}

#[test]
fn barracks_placement_rules_3x3() {
    let mut sim = Sim::new(37);
    let start_wood = sim.stockpile().amount(skirmish_sim::ResourceKind::Wood);
    // Anchor (24,20): the 3×3 footprint covers wall column x=25.
    let (x, y) = cell_point(&sim, 24, 20);
    assert!(!sim.can_build(1, BuildingKind::Barracks, x, y));
    sim.push_command(Command::Build {
        entity: 1,
        kind: BuildingKind::Barracks,
        x,
        y,
    });
    sim.tick();
    assert_eq!(sim.buildings().len(), 0);
    // Anchor (48,48): cells 48..50 hang off the 50-cell grid.
    let (x, y) = cell_point(&sim, 48, 48);
    assert!(!sim.can_build(1, BuildingKind::Barracks, x, y));
    // A legal plot deducts 175 and blocks all 9 cells.
    let (x, y) = cell_point(&sim, 27, 20);
    assert!(sim.can_build(1, BuildingKind::Barracks, x, y));
    sim.push_command(Command::Build {
        entity: 1,
        kind: BuildingKind::Barracks,
        x,
        y,
    });
    sim.tick();
    assert_eq!(sim.buildings().len(), 1);
    assert_eq!(
        sim.stockpile().amount(skirmish_sim::ResourceKind::Wood),
        start_wood - I16F16::from_num(175)
    );
    for cx in 27..=29 {
        for cy in 20..=22 {
            assert!(sim.grid().is_dynamic(cx, cy), "barracks cell ({cx},{cy}) not blocked");
        }
    }
}

#[test]
fn militia_production_and_population_interplay() {
    let mut sim = Sim::new(41);
    let (x, y) = cell_point(&sim, 27, 20);
    sim.push_command(Command::Build {
        entity: 1,
        kind: BuildingKind::Barracks,
        x,
        y,
    });
    let built = run_until(&mut sim, 30000, |s| {
        s.buildings().iter().any(|b| b.kind == BuildingKind::Barracks && b.complete)
    });
    assert!(built, "barracks never completed");
    let bid = sim.buildings()[0].id;
    // Train a Militia: 60 food / 20 gold deducted on enqueue.
    let (f0, g0) = (
        sim.stockpile().amount(skirmish_sim::ResourceKind::Food),
        sim.stockpile().amount(skirmish_sim::ResourceKind::Gold),
    );
    sim.push_command(Command::TrainAt {
        building: bid,
        kind: EntityKind::Militia,
    });
    sim.tick();
    assert_eq!(
        sim.stockpile().amount(skirmish_sim::ResourceKind::Food),
        f0 - I16F16::from_num(60)
    );
    assert_eq!(
        sim.stockpile().amount(skirmish_sim::ResourceKind::Gold),
        g0 - I16F16::from_num(20)
    );
    assert_eq!(sim.population(), 5, "queued militia must count toward pop");
    let trained = run_until(&mut sim, 10000, |s| {
        s.entities().iter().any(|e| e.kind == EntityKind::Militia)
    });
    assert!(trained, "militia never trained");
    assert_eq!(sim.living_population(), 5);
    // At the cap (5/5): a second militia enqueues but cannot finish.
    sim.push_command(Command::TrainAt {
        building: bid,
        kind: EntityKind::Militia,
    });
    sim.tick();
    assert_eq!(sim.buildings()[0].queue.len(), 1);
    for _ in 0..3000 {
        sim.tick();
        assert_eq!(
            sim.entities().iter().filter(|e| e.kind == EntityKind::Militia).count(),
            1,
            "a militia completed while housed"
        );
    }
    assert_eq!(
        sim.train_at_block(bid, EntityKind::Militia),
        skirmish_sim::TrainBlock::Housed
    );
    // A house (the last 25 wood) frees room; production resumes alone.
    let (hx, hy) = cell_point(&sim, 18, 27);
    sim.push_command(Command::Build {
        entity: 2,
        kind: BuildingKind::House,
        x: hx,
        y: hy,
    });
    let housed = run_until(&mut sim, 30000, |s| {
        s.entities().iter().filter(|e| e.kind == EntityKind::Militia).count() == 2
    });
    assert!(housed, "stalled militia did not resume after housing");
    assert_eq!(sim.living_population(), 6);
}

#[test]
fn separation_is_suppressed_while_a_melee_unit_attacks() {
    let mut sim = Sim::new(43);
    // Villager 1 attacks sheep 5; villager 2 is ordered to stand on
    // the same spot, crowding the fight. While villager 1 is in melee
    // range of the sheep with the attack engaged, MOV-04 says
    // separation must not push it: the flag is on.
    sim.push_command(Command::Attack { entity: 1, target: 5 });
    let (sx, sy) = {
        let s = ent(&sim, 5);
        (s.pos_x, s.pos_y)
    };
    sim.push_command(Command::Move { entity: 2, x: sx, y: sy });
    let mut saw_suppressed_in_range = false;
    for _ in 0..6000 {
        sim.tick();
        let v1 = ent(&sim, 1);
        if ent(&sim, 5).dead {
            break;
        }
        if v1.attack_target == Some(5)
            && dist(v1, ent(&sim, 5)) <= 3.0
            && dist(v1, ent(&sim, 2)) < 2.6
        {
            assert!(
                v1.suppress_separation,
                "engaged melee attacker was not separation-suppressed"
            );
            saw_suppressed_in_range = true;
        }
    }
    assert!(ent(&sim, 5).dead, "the crowded fight never finished");
    assert!(
        saw_suppressed_in_range,
        "never observed the engaged-and-crowded state"
    );
}
