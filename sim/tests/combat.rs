//! Combat tests (M3): the standard start (Scout + Gaia herds), sheep
//! capture/kill/carcass decay, the boar lure loop, hunt carry 35,
//! archer projectiles with RNG replay discipline, Barracks production
//! and population interplay, death, and MOV-04 separation suppression.
//!
//! Entity ids in the standard start: 0 Town Center, 1–3 villagers,
//! 4 Scout Cavalry, 5–8 near sheep, 9–12 far sheep, 13–14 boars,
//! 15–20 deer (two herds of 3). Since M7 the *cells* come from the
//! seeded generator (the package layout is OPENING-SPEC §2's); the
//! id order is the generator's fixed package order.

use fixed::types::I16F16;
use skirmish_sim::{
    Age, BuildingKind, Command, Entity, EntityKind, MatchMode, NodeKind, ResourceKind, Sim,
    TechId, OWNER_GAIA, OWNER_OPPONENT, OWNER_PLAYER,
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
    // M7 package: TC + 3 villagers + Scout + 8 sheep + 2 boars +
    // 6 deer (near herd 3 + far herd 3, OPENING-SPEC §2.2).
    assert_eq!(sim.entities().len(), 21);
    let scout = ent(&sim, 4);
    assert_eq!(scout.kind, EntityKind::ScoutCavalry);
    assert_eq!(scout.owner, OWNER_PLAYER);
    assert_eq!(scout.hp, I16F16::from_num(45));
    // Beside the Town Center: the scout's fixed (+5,+5) offset.
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
    assert_eq!(count(EntityKind::Deer), 6);
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
    // Far sheep 9 — at its generated cell since M7 (the arena's
    // fixed (13, 18) is gone). Walk villager 1 up to it.
    let (sx, sy) = (ent(&sim, 9).pos_x, ent(&sim, 9).pos_y);
    sim.push_command(Command::Move {
        entity: 1,
        x: sx,
        y: sy,
    });
    let captured = run_until(&mut sim, 6000, |s| ent(s, 9).owner == OWNER_PLAYER);
    assert!(captured, "sheep was never captured by proximity");
    // A captured sheep takes move orders like a unit: send it to
    // the Town Center (open ground by construction).
    let (ox, oy) = (f(ent(&sim, 9).pos_x), f(ent(&sim, 9).pos_y));
    let tc = sim.town_center();
    sim.push_command(Command::Move {
        entity: 9,
        x: tc.pos_x,
        y: tc.pos_y,
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
    assert_eq!(sim.entities().len(), 21); // M7 package size
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

/// The first anchor cell for `kind` that `entity` may build, in
/// expanding rings around a center cell — M7: the build-up's plots
/// are derived from the generated map (the arena's fixed cells are
/// gone), the way a player picks ground near the thing the building
/// serves.
fn anchor_near(sim: &Sim, entity: u32, kind: BuildingKind, center: (u32, u32)) -> (u32, u32) {
    for r in 1..=14i32 {
        for dy in -r..=r {
            for dx in -r..=r {
                if dx.abs() != r && dy.abs() != r {
                    continue;
                }
                let (ax, ay) = (center.0 as i32 + dx, center.1 as i32 + dy);
                if ax < 0 || ay < 0 {
                    continue;
                }
                let (ax, ay) = (ax as u32, ay as u32);
                let (x, y) = cell_point(sim, ax, ay);
                if sim.can_build(entity, kind, x, y) {
                    return (ax, ay);
                }
            }
        }
    }
    panic!("no buildable anchor near {center:?} for {kind:?}");
}

fn cell_of(sim: &Sim, x: I16F16, y: I16F16) -> (u32, u32) {
    sim.grid().world_to_cell(x, y)
}

/// Scripted build-up shared by the archer test: walk the real
/// production path to an Archer. Returns nothing; asserts as it
/// goes.
///
/// M6 adaptation: in M3 this trained the Archer at the Barracks.
/// Since M6 the Archer's home is the Archery Range — Feudal-gated
/// and Barracks-gated — so the helper now plays the opening the
/// way the game does: Mill + Lumber Camp (the Feudal pair), gather
/// up, click Feudal, Barracks east of the wall, Range beside it,
/// then train. The behavior under test (ranged kills, projectile
/// RNG) is unchanged.
fn build_barracks_and_archer(sim: &mut Sim) {
    let build_at = |sim: &mut Sim, entity: u32, kind: BuildingKind, cx: u32, cy: u32| {
        let (x, y) = cell_point(sim, cx, cy);
        sim.push_command(Command::Build {
            entity,
            kind,
            x,
            y,
        });
    };
    let first_node = |sim: &Sim, kind: NodeKind| {
        sim.nodes()
            .iter()
            .find(|n| n.kind == kind)
            .unwrap()
            .id
    };
    // First node of the kind with anything left in it — a Gather
    // order naming a depleted node is ignored by the sim, so a
    // re-assignment late in a long build-up must pick a live one.
    let first_live_node = |sim: &Sim, kind: NodeKind| {
        sim.nodes()
            .iter()
            .find(|n| n.kind == kind && n.amount_left > I16F16::from_num(0))
            .unwrap()
            .id
    };
    // The Feudal pair first: a Mill by the berry patch and a
    // Lumber Camp by the woodline (M7: anchors derived from the
    // generated map — the first buildable ground in rings around
    // the resource each camp serves); villager 3 chops from the
    // start.
    let berry0 = sim
        .nodes()
        .iter()
        .find(|n| n.kind == NodeKind::BerryBush)
        .unwrap();
    let berry_cell = cell_of(sim, berry0.pos_x, berry0.pos_y);
    // The woodline tree: a forest EDGE tree — at least one free
    // standing cell beside it, and several tree neighbors within
    // the retarget radius, so a depleted trunk chains to the next
    // (M7: the first node in id order is a lone straggler now;
    // working it dry strands the chopper with no neighbor in
    // reach, and a deep-interior tree has no standing cell at
    // all — both are correct sim behavior and the wrong script).
    let (tree0_id, tree_cell) = {
        let trees: Vec<_> = sim
            .nodes()
            .iter()
            .filter(|n| n.kind == NodeKind::Tree)
            .collect();
        let neighbors = |t: &skirmish_sim::ResourceNode| {
            trees
                .iter()
                .filter(|o| {
                    let dx = f(o.pos_x) - f(t.pos_x);
                    let dy = f(o.pos_y) - f(t.pos_y);
                    dx * dx + dy * dy <= 144.0
                })
                .count()
        };
        // A standing cell counts only if the pathfinder can
        // actually reach it from the Town Center — a one-cell
        // clearing ringed by trunks is free but unreachable.
        let tc = sim.town_center();
        let (tc_x, tc_y) = (tc.pos_x, tc.pos_y);
        let has_free_side = |t: &skirmish_sim::ResourceNode| {
            let (cx, cy) = cell_of(sim, t.pos_x, t.pos_y);
            (-1i32..=1).any(|dy| {
                (-1i32..=1).any(|dx| {
                    let (nx, ny) = (cx as i32 + dx, cy as i32 + dy);
                    if nx < 0 || ny < 0 || sim.grid().is_blocked(nx as u32, ny as u32) {
                        return false;
                    }
                    let center = sim.grid().cell_center(nx as u32, ny as u32);
                    sim.grid().find_path((tc_x, tc_y), center).is_some()
                })
            })
        };
        let best = trees
            .iter()
            .filter(|t| has_free_side(t) && neighbors(t) >= 3)
            .max_by_key(|t| neighbors(t))
            .expect("no forest edge tree");
        (best.id, cell_of(sim, best.pos_x, best.pos_y))
    };
    let (mx, my) = anchor_near(sim, 1, BuildingKind::Mill, berry_cell);
    build_at(sim, 1, BuildingKind::Mill, mx, my);
    // The camp anchor is searched from a center stepped from the
    // woodline tree TOWARD the Town Center, so the camp lands on
    // the open edge between forest and base — where a player puts
    // it. (Forest contiguity pass, 2026-10-20: woodlines are now
    // solid masses, and the old search's first buildable ring
    // could be a rim bite INSIDE the mass; a 2x2 camp there plugs
    // the only corridor and the choppers' chain route dies —
    // correct sim behavior, wrong script.)
    let tc_cell0 = cell_of(sim, sim.town_center().pos_x, sim.town_center().pos_y);
    let step_toward_tc = |a: u32, b: u32| -> u32 {
        (a as i32 + (b as i32 - a as i32).signum() * 3).clamp(0, 119) as u32
    };
    let camp_center = (
        step_toward_tc(tree_cell.0, tc_cell0.0),
        step_toward_tc(tree_cell.1, tc_cell0.1),
    );
    let (lx, ly) = anchor_near(sim, 2, BuildingKind::LumberCamp, camp_center);
    build_at(sim, 2, BuildingKind::LumberCamp, lx, ly);
    let tree = tree0_id;
    sim.push_command(Command::Gather {
        entity: 3,
        node: tree,
    });
    let camps_up = run_until(sim, 30000, |s| {
        s.buildings().iter().filter(|b| b.complete).count() >= 2
    });
    assert!(camps_up, "the camps never completed");
    // Work: villager 1 to the berries, villager 2 to the woodline.
    let berry = first_node(sim, NodeKind::BerryBush);
    sim.push_command(Command::Gather {
        entity: 1,
        node: berry,
    });
    sim.push_command(Command::Gather {
        entity: 2,
        node: tree,
    });
    // Barracks as soon as its wood is in hand.
    let wood_ready = run_until(sim, 60000, |s| {
        s.stockpile().amount(ResourceKind::Wood) >= I16F16::from_num(175)
    });
    assert!(wood_ready, "never gathered the barracks wood");
    let tc = sim.town_center();
    let tc_cell = cell_of(sim, tc.pos_x, tc.pos_y);
    let (bx, by) = anchor_near(sim, 1, BuildingKind::Barracks, tc_cell);
    build_at(sim, 1, BuildingKind::Barracks, bx, by);
    let barracks_up = run_until(sim, 30000, |s| {
        s.buildings()
            .iter()
            .any(|b| b.kind == BuildingKind::Barracks && b.complete)
    });
    assert!(barracks_up, "barracks never completed");
    // Villager 1 back on food; click Feudal when the 500 is in hand.
    // (Budget raised by the 2026-10 pace correction: at the corrected
    // 0.8 tiles/s the berry trips between patch and Mill take their
    // real game time, so the gatherer needs well over the old 90k
    // ticks for 500 food. The amount asserted is unchanged. The bush
    // is also re-bound LIVE: the phase-1 bush is depleted by now —
    // villager 1 ate its whole 125 — and the sim ignores a Gather
    // order naming a dead node, which at the prototype pace never
    // bit because the re-gather came after a handful of trips.)
    let berry = first_live_node(sim, NodeKind::BerryBush);
    sim.push_command(Command::Gather {
        entity: 1,
        node: berry,
    });
    let food_ready = run_until(sim, 240_000, |s| {
        s.stockpile().amount(ResourceKind::Food) >= I16F16::from_num(500)
    });
    assert!(food_ready, "never gathered the feudal food");
    assert!(sim.can_research_tc(TechId::FeudalAge));
    sim.push_command(Command::ResearchTc {
        tech: TechId::FeudalAge,
    });
    let feudal = run_until(sim, 30000, |s| s.age_of(OWNER_PLAYER) == Age::Feudal);
    assert!(feudal, "never reached the Feudal Age");
    // The Range beside the Barracks; the Archer trains there.
    let wood_ready = run_until(sim, 60000, |s| {
        s.stockpile().amount(ResourceKind::Wood) >= I16F16::from_num(175)
    });
    assert!(wood_ready, "never gathered the range wood");
    let (rx, ry) = anchor_near(sim, 2, BuildingKind::ArcheryRange, (bx, by));
    build_at(sim, 2, BuildingKind::ArcheryRange, rx, ry);
    let range_up = run_until(sim, 30000, |s| {
        s.buildings()
            .iter()
            .any(|b| b.kind == BuildingKind::ArcheryRange && b.complete)
    });
    assert!(range_up, "archery range never completed");
    let rid = sim
        .buildings()
        .iter()
        .find(|b| b.kind == BuildingKind::ArcheryRange)
        .unwrap()
        .id;
    assert!(sim.can_train_at(rid, EntityKind::Archer));
    sim.push_command(Command::TrainAt {
        building: rid,
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
    // M7: the scenario cells are derived from seed 37's generated
    // map (the wall column and the 50-cell edge are gone).
    // An anchor whose 3×3 footprint covers a standing tree's cell
    // (MAP-04: trees block) is refused whole.
    let tree = sim
        .nodes()
        .iter()
        .find(|n| n.kind == NodeKind::Tree)
        .unwrap();
    let (tx, ty) = cell_of(&sim, tree.pos_x, tree.pos_y);
    let (x, y) = cell_point(&sim, tx - 1, ty - 1);
    assert!(!sim.can_build(1, BuildingKind::Barracks, x, y));
    sim.push_command(Command::Build {
        entity: 1,
        kind: BuildingKind::Barracks,
        x,
        y,
    });
    sim.tick();
    assert_eq!(sim.buildings().len(), 0);
    // An anchor at the map's corner: the footprint hangs off the
    // 120-cell grid.
    let dim = sim.grid().width();
    let (x, y) = cell_point(&sim, dim - 2, dim - 2);
    assert!(!sim.can_build(1, BuildingKind::Barracks, x, y));
    // A legal plot deducts 175 and blocks all 9 cells.
    let tc = sim.town_center();
    let tc_cell = cell_of(&sim, tc.pos_x, tc.pos_y);
    let (ax, ay) = anchor_near(&sim, 1, BuildingKind::Barracks, tc_cell);
    let (x, y) = cell_point(&sim, ax, ay);
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
    for cx in ax..ax + 3 {
        for cy in ay..ay + 3 {
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
    // Train a Militia: 50 food / 20 gold deducted on enqueue
    // (current DE, update 141935).
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
        f0 - I16F16::from_num(50)
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

// ---- CMB-08: the auto-acquisition priority chain (2026-10-22) ----
//
// An idle military unit acquires, in order: (1) the closest enemy
// unit in LoS, (2) the enemy Town Center in LoS, (3) the closest
// other enemy building in LoS (Farms never). These scenes run in
// TwoPlayer mode so both seats are scripted: the opponent's units
// are walked to a far corner, leaving its buildings bare.

fn opp_tc(sim: &Sim) -> &Entity {
    sim.entities()
        .iter()
        .find(|e| e.kind == EntityKind::TownCenter && e.owner == OWNER_OPPONENT)
        .unwrap()
}

/// Walk every opponent unit (villagers + scout) far from the
/// opponent Town Center, and wait until they are all at least
/// 60 world units from it.
fn walk_opponent_away(sim: &mut Sim) {
    let tc = opp_tc(sim);
    let (ox, oy) = (tc.pos_x, tc.pos_y);
    let ids: Vec<u32> = sim
        .entities()
        .iter()
        .filter(|e| e.owner == OWNER_OPPONENT && e.kind != EntityKind::TownCenter)
        .map(|e| e.id)
        .collect();
    for id in ids {
        sim.push_command(Command::Move {
            entity: id,
            x: ox + I16F16::from_num(150),
            y: oy + I16F16::from_num(150),
        });
    }
    let tc_pos = (f(ox), f(oy));
    let gone = run_until(sim, 90_000, |s| {
        s.entities()
            .iter()
            .filter(|e| e.owner == OWNER_OPPONENT && e.kind != EntityKind::TownCenter)
            .all(|e| {
                let dx = f(e.pos_x) - tc_pos.0;
                let dy = f(e.pos_y) - tc_pos.1;
                (dx * dx + dy * dy) > 60.0 * 60.0
            })
    });
    assert!(gone, "opponent units never left their Town Center");
}

/// Train one player Militia through the real production path
/// (Barracks by villager 1 beside the player's Town Center).
fn train_player_militia(sim: &mut Sim) -> u32 {
    let tc = sim.town_center();
    let tc_cell = cell_of(sim, tc.pos_x, tc.pos_y);
    let (bx, by) = anchor_near(sim, 1, BuildingKind::Barracks, tc_cell);
    let (x, y) = cell_point(sim, bx, by);
    sim.push_command(Command::Build {
        entity: 1,
        kind: BuildingKind::Barracks,
        x,
        y,
    });
    let built = run_until(sim, 60_000, |s| {
        s.buildings()
            .iter()
            .any(|b| b.kind == BuildingKind::Barracks && b.complete)
    });
    assert!(built, "barracks never completed");
    let bid = sim
        .buildings()
        .iter()
        .find(|b| b.kind == BuildingKind::Barracks)
        .unwrap()
        .id;
    sim.push_command(Command::TrainAt {
        building: bid,
        kind: EntityKind::Militia,
    });
    let trained = run_until(sim, 30_000, |s| {
        s.entities()
            .iter()
            .any(|e| e.kind == EntityKind::Militia && e.owner == OWNER_PLAYER)
    });
    assert!(trained, "militia never trained");
    sim.entities()
        .iter()
        .find(|e| e.kind == EntityKind::Militia && e.owner == OWNER_PLAYER)
        .unwrap()
        .id
}

fn move_and_settle(sim: &mut Sim, entity: u32, x: I16F16, y: I16F16) {
    sim.push_command(Command::Move { entity, x, y });
    let settled = run_until(sim, 90_000, |s| ent(s, entity).order.is_none());
    assert!(settled, "entity {entity} never settled at its staging point");
}

/// The opponent raises a House near the map center — far from
/// its Town Center, so the chain's step (2) cannot pre-empt the
/// building step in these scenes (a house beside the TC stands
/// inside the TC's own step-(2) shadow from most angles).
/// Returns its id.
fn opp_house(sim: &mut Sim) -> u32 {
    let tc = opp_tc(sim);
    let tc_cell = cell_of(sim, tc.pos_x, tc.pos_y);
    let my_tc = sim.town_center();
    let my_cell = cell_of(sim, my_tc.pos_x, my_tc.pos_y);
    let mid = (
        (tc_cell.0 + my_cell.0) / 2,
        (tc_cell.1 + my_cell.1) / 2,
    );
    let builder = sim
        .entities()
        .iter()
        .find(|e| e.owner == OWNER_OPPONENT && e.kind == EntityKind::Villager)
        .unwrap()
        .id;
    let (hx, hy) = anchor_near(sim, builder, BuildingKind::House, mid);
    let (x, y) = cell_point(sim, hx, hy);
    sim.push_command(Command::Build {
        entity: builder,
        kind: BuildingKind::House,
        x,
        y,
    });
    let built = run_until(sim, 60_000, |s| {
        s.buildings()
            .iter()
            .any(|b| b.kind == BuildingKind::House && b.complete)
    });
    assert!(built, "opponent house never completed");
    sim.buildings()
        .iter()
        .find(|b| b.kind == BuildingKind::House)
        .unwrap()
        .id
}

#[test]
fn cmb08_idle_militia_acquires_enemy_building() {
    let mut sim = Sim::with_mode(11, MatchMode::TwoPlayer);
    let house = opp_house(&mut sim);
    walk_opponent_away(&mut sim);
    let mid = train_player_militia(&mut sim);
    let (hx, hy) = {
        let b = sim.buildings().iter().find(|b| b.id == house).unwrap();
        (b.pos_x, b.pos_y)
    };
    // Stage inside the Militia's LoS (16) of the house, with no
    // enemy unit anywhere near: the chain must reach step (3).
    move_and_settle(&mut sim, mid, hx + I16F16::from_num(8), hy);
    let acquired = run_until(&mut sim, 5_000, |s| {
        ent(s, mid).attack_building == Some(house)
    });
    assert!(acquired, "idle militia never auto-acquired the bare enemy house");
    let hp0 = sim.buildings().iter().find(|b| b.id == house).unwrap().hp;
    let dented = run_until(&mut sim, 30_000, |s| {
        s.buildings()
            .iter()
            .find(|b| b.id == house)
            .map(|b| b.hp < hp0)
            .unwrap_or(true)
    });
    assert!(dented, "acquired house took no damage");
}

#[test]
fn cmb08_units_outrank_buildings_then_building_falls_to_step_3() {
    let mut sim = Sim::with_mode(12, MatchMode::TwoPlayer);
    let house = opp_house(&mut sim);
    // One opponent villager stays behind, standing by the house;
    // everyone else walks away.
    let guard = sim
        .entities()
        .iter()
        .filter(|e| e.owner == OWNER_OPPONENT && e.kind == EntityKind::Villager)
        .nth(1)
        .unwrap()
        .id;
    let (hx, hy) = {
        let b = sim.buildings().iter().find(|b| b.id == house).unwrap();
        (b.pos_x, b.pos_y)
    };
    // Walk-off for the rest: reuse the helper, then bring the
    // guard back to the house (the helper moved it too).
    walk_opponent_away(&mut sim);
    move_and_settle(&mut sim, guard, hx - I16F16::from_num(6), hy);
    let mid = train_player_militia(&mut sim);
    move_and_settle(&mut sim, mid, hx + I16F16::from_num(8), hy);
    // Step (1) beats step (3): the villager is the target, not
    // the house, while both stand inside the Militia's LoS.
    let engaged = run_until(&mut sim, 5_000, |s| {
        let m = ent(s, mid);
        m.attack_target == Some(guard) || ent(s, guard).dead
    });
    assert!(engaged, "militia never engaged the guard villager");
    assert!(
        ent(&sim, mid).attack_building.is_none() || ent(&sim, guard).dead,
        "militia targeted the house while a unit stood in sight"
    );
    // The guard dies to the militia; with no unit left in sight,
    // the chain falls through to the house.
    let dead = run_until(&mut sim, 60_000, |s| ent(s, guard).dead);
    assert!(dead, "the guard villager never died");
    let acquired = run_until(&mut sim, 10_000, |s| {
        ent(s, mid).attack_building == Some(house)
    });
    assert!(acquired, "after the kill, the militia never fell through to the house");
}

#[test]
fn cmb08_idle_militia_acquires_enemy_town_center() {
    let mut sim = Sim::with_mode(13, MatchMode::TwoPlayer);
    let tc_id = opp_tc(&sim).id;
    walk_opponent_away(&mut sim);
    let mid = train_player_militia(&mut sim);
    let (tx, ty) = {
        let tc = opp_tc(&sim);
        (tc.pos_x, tc.pos_y)
    };
    // 12 world units from the TC's center: inside the Militia's
    // LoS of 16, with no unit or building of the opponent near —
    // the chain must stop at step (2).
    move_and_settle(&mut sim, mid, tx + I16F16::from_num(12), ty);
    let acquired = run_until(&mut sim, 5_000, |s| {
        ent(s, mid).attack_target == Some(tc_id)
    });
    assert!(acquired, "idle militia never auto-acquired the bare enemy Town Center");
    let dented = run_until(&mut sim, 30_000, |s| {
        ent(s, tc_id).hp < I16F16::from_num(2400)
    });
    assert!(dented, "the acquired Town Center took no damage");
}
