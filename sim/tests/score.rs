//! Score tests (VIC-04; wiki, Score).
//!
//! The formula, as the wiki pins it: Military = 20% of the
//! resource cost of enemy units and buildings destroyed (or
//! converted); Economy = 10% of resources currently held +
//! 20% of the resource cost of surviving units and standard
//! buildings; Technology = 20% of researched-technology cost +
//! 10 points per 1% of the map explored; Society = 20% of
//! Castles and Wonders constructed (none exist here — the term
//! is structurally 0). These tests pin every term against live
//! sims: the opening position, a Loom research, a scripted
//! unit kill, a building razing, and the no-credit exclusions
//! (self-deletes, Gaia kills).

use skirmish_sim::{
    BuildingKind, Command, EntityKind, MatchMode, Sim, TechId, OWNER_GAIA, OWNER_OPPONENT,
    OWNER_PLAYER,
};

fn find_id(sim: &Sim, owner: u8, kind: EntityKind) -> u32 {
    sim.entities()
        .iter()
        .find(|e| e.owner == owner && e.kind == kind && !e.dead)
        .unwrap()
        .id
}

fn find_all(sim: &Sim, owner: u8, kind: EntityKind) -> Vec<u32> {
    sim.entities()
        .iter()
        .filter(|e| e.owner == owner && e.kind == kind && !e.dead)
        .map(|e| e.id)
        .collect()
}

fn hp_of(sim: &Sim, id: u32) -> f64 {
    sim.entities()
        .iter()
        .find(|e| e.id == id)
        .unwrap()
        .hp
        .to_num::<f64>()
}

fn is_dead(sim: &Sim, id: u32) -> bool {
    sim.entities().iter().find(|e| e.id == id).unwrap().dead
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

/// Explored share of the map for an owner, counted cell by cell
/// through the public fog view — the independent yardstick for
/// the Technology term's exploration points.
fn explored_pct(sim: &Sim, owner: u8) -> i64 {
    let g = sim.grid();
    let (w, h) = (g.width(), g.height());
    let mut seen = 0i64;
    for cy in 0..h {
        for cx in 0..w {
            if sim.cell_explored_by(owner, cx, cy) {
                seen += 1;
            }
        }
    }
    seen * 100 / (w as i64 * h as i64)
}

/// The first anchor cell near entity `center` where `kind` may
/// be built by `entity` — the construction suite's ring scan.
fn open_anchor_near(sim: &Sim, center: u32, entity: u32, kind: BuildingKind) -> (u32, u32) {
    let (cx0, cy0) = {
        let c = sim.entities().iter().find(|e| e.id == center).unwrap();
        (c.pos_x, c.pos_y)
    };
    let (tcx, tcy) = sim.grid().world_to_cell(cx0, cy0);
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

fn build_near(sim: &mut Sim, builder: u32, center: u32, kind: BuildingKind) -> u32 {
    let (ax, ay) = open_anchor_near(sim, center, builder, kind);
    let (x, y) = sim.grid().cell_center(ax, ay);
    let before: Vec<u32> = sim.buildings().iter().map(|b| b.id).collect();
    sim.push_command(Command::Build {
        entity: builder,
        kind,
        x,
        y,
    });
    // The placement registers the building record within a tick.
    let placed = run_until(sim, 600, |s| {
        s.buildings().iter().any(|b| !before.contains(&b.id))
    });
    assert!(placed, "building was never placed");
    sim.buildings()
        .iter()
        .find(|b| !before.contains(&b.id))
        .unwrap()
        .id
}

#[test]
fn opening_scores_match_the_formula() {
    let mut sim = Sim::with_mode(42, MatchMode::TwoPlayer);
    // Let the opening fog settle; nobody moves or gathers without
    // orders, so the position is otherwise exactly the start.
    for _ in 0..30 {
        sim.tick();
    }
    for owner in [OWNER_PLAYER, OWNER_OPPONENT] {
        let s = sim.score_for(owner);
        // Military: nothing destroyed yet.
        assert_eq!(s.military, 0, "owner {owner}");
        // Society: no Castles or Wonders exist in the game.
        assert_eq!(s.society, 0, "owner {owner}");
        // Economy: 10% of the 200/200/100/200 start stockpile
        // (700 -> 70) + 20% of surviving value: 3 Villagers at
        // 50 F and the Scout at 80 F (230 -> 46); the Town
        // Center's table cost is 0.
        assert_eq!(s.economy, 116, "owner {owner}");
        // Technology: no techs researched; 10 per 1% explored,
        // counted independently through the fog view.
        assert_eq!(s.technology, explored_pct(&sim, owner) * 10, "owner {owner}");
        assert_eq!(s.total, s.military + s.economy + s.technology + s.society);
    }
    // Gaia has no score.
    assert_eq!(sim.score_for(OWNER_GAIA).total, 0);
}

#[test]
fn loom_research_adds_its_cost_share_to_technology() {
    let mut sim = Sim::with_mode(42, MatchMode::TwoPlayer);
    for _ in 0..30 {
        sim.tick();
    }
    let before = sim.score_for(OWNER_PLAYER);
    sim.push_command(Command::ResearchTc { tech: TechId::Loom });
    let done = run_until(&mut sim, 20_000, |s| s.has_tech(OWNER_PLAYER, TechId::Loom));
    assert!(done, "Loom never completed");
    let after = sim.score_for(OWNER_PLAYER);
    // Loom costs 50 gold: Technology gains exactly 20% of it.
    // Nobody moved, so the explored share cannot have moved.
    assert_eq!(after.technology - before.technology, 10);
    // The 50 gold left the stockpile: the 10% term drops by 5.
    assert_eq!(after.economy - before.economy, -5);
}

#[test]
fn a_scripted_kill_credits_the_killers_military() {
    let mut sim = Sim::with_mode(7, MatchMode::TwoPlayer);
    let mut twin = Sim::with_mode(7, MatchMode::TwoPlayer);
    let scout = find_id(&sim, OWNER_PLAYER, EntityKind::ScoutCavalry);
    // The foe villager farthest from the foe's own Scout — the
    // cleanest victim: no guard in the way of the ride over.
    let foe_scout_pos = {
        let e = sim
            .entities()
            .iter()
            .find(|e| e.owner == OWNER_OPPONENT && e.kind == EntityKind::ScoutCavalry)
            .unwrap();
        (e.pos_x, e.pos_y)
    };
    let victim = *find_all(&sim, OWNER_OPPONENT, EntityKind::Villager)
        .iter()
        .max_by(|&&a, &&b| {
            let dist2 = |id: u32| {
                let e = sim.entities().iter().find(|e| e.id == id).unwrap();
                let dx = (e.pos_x - foe_scout_pos.0).to_num::<f64>();
                let dy = (e.pos_y - foe_scout_pos.1).to_num::<f64>();
                dx * dx + dy * dy
            };
            dist2(a).partial_cmp(&dist2(b)).unwrap()
        })
        .unwrap();
    for s in [&mut sim, &mut twin] {
        s.push_command(Command::Attack {
            entity: scout,
            target: victim,
        });
    }
    assert_eq!(sim.score_for(OWNER_PLAYER).military, 0);
    let dead = run_until(&mut sim, 180_000, |s| is_dead(s, victim));
    for _ in 0..sim.tick_count() {
        twin.tick();
    }
    assert!(dead, "the scout never killed the villager");
    // A Villager costs 50 food: destroyed value 50, Military 10.
    assert_eq!(sim.destroyed_value_of(OWNER_PLAYER), 50);
    assert_eq!(sim.score_for(OWNER_PLAYER).military, 10);
    // The victim's side killed nothing.
    assert_eq!(sim.destroyed_value_of(OWNER_OPPONENT), 0);
    assert_eq!(sim.score_for(OWNER_OPPONENT).military, 0);
    // The counter is hashed state: the twin fed the identical
    // stream is still bit-identical, kill included.
    assert_eq!(sim.state_hash(), twin.state_hash());
    assert_eq!(
        sim.score_for(OWNER_PLAYER),
        twin.score_for(OWNER_PLAYER),
        "twin scores must agree"
    );
}

#[test]
fn a_deleted_unit_credits_no_one_even_after_being_hit() {
    let mut sim = Sim::with_mode(7, MatchMode::TwoPlayer);
    let scout = find_id(&sim, OWNER_PLAYER, EntityKind::ScoutCavalry);
    let victim = find_id(&sim, OWNER_OPPONENT, EntityKind::Villager);
    sim.push_command(Command::Attack {
        entity: scout,
        target: victim,
    });
    let full_hp = hp_of(&sim, victim);
    let hit = run_until(&mut sim, 180_000, |s| hp_of(s, victim) < full_hp);
    assert!(hit, "the scout never landed a hit");
    // The owner deletes its own wounded villager: a deliberate
    // delete has no killer (SEL-14), so the death sweep must
    // credit nothing to the attacker who wounded it.
    sim.push_command(Command::DeleteEntity { entity: victim });
    let gone = run_until(&mut sim, 600, |s| is_dead(s, victim));
    assert!(gone, "the delete never landed");
    assert_eq!(sim.destroyed_value_of(OWNER_PLAYER), 0);
    assert_eq!(sim.score_for(OWNER_PLAYER).military, 0);
}

#[test]
fn razing_a_building_credits_its_wood_cost() {
    let mut sim = Sim::with_mode(7, MatchMode::TwoPlayer);
    let my_tc = find_id(&sim, OWNER_PLAYER, EntityKind::TownCenter);
    let foe_tc = find_id(&sim, OWNER_OPPONENT, EntityKind::TownCenter);
    let my_villager = find_id(&sim, OWNER_PLAYER, EntityKind::Villager);
    let foe_villager = find_id(&sim, OWNER_OPPONENT, EntityKind::Villager);

    // The foe raises a House by its Town Center.
    let house = build_near(&mut sim, foe_villager, foe_tc, BuildingKind::House);
    let up = run_until(&mut sim, 60_000, |s| {
        s.buildings().iter().find(|b| b.id == house).unwrap().complete
    });
    assert!(up, "the foe house never completed");
    // A House is no Castle: Society stays 0 with it standing.
    assert_eq!(sim.score_for(OWNER_OPPONENT).society, 0);

    // We answer with a Barracks and a Militia.
    let barracks = build_near(&mut sim, my_villager, my_tc, BuildingKind::Barracks);
    let up = run_until(&mut sim, 60_000, |s| {
        s.buildings()
            .iter()
            .find(|b| b.id == barracks)
            .unwrap()
            .complete
    });
    assert!(up, "the barracks never completed");
    sim.push_command(Command::TrainAt {
        building: barracks,
        kind: EntityKind::Militia,
    });
    let trained = run_until(&mut sim, 60_000, |s| {
        !find_all(s, OWNER_PLAYER, EntityKind::Militia).is_empty()
    });
    assert!(trained, "the militia never trained");
    let militia = find_id(&sim, OWNER_PLAYER, EntityKind::Militia);

    // Ride to the house and raze it.
    let (hx, hy) = {
        let b = sim.buildings().iter().find(|b| b.id == house).unwrap();
        (b.pos_x, b.pos_y)
    };
    sim.push_command(Command::Move {
        entity: militia,
        x: hx,
        y: hy,
    });
    let near = run_until(&mut sim, 120_000, |s| {
        let e = s.entities().iter().find(|e| e.id == militia).unwrap();
        let dx = (e.pos_x - hx).to_num::<f64>();
        let dy = (e.pos_y - hy).to_num::<f64>();
        dx * dx + dy * dy < 100.0
    });
    assert!(near, "the militia never reached the house");
    sim.push_command(Command::AttackBuilding {
        entity: militia,
        building: house,
    });
    let razed = run_until(&mut sim, 180_000, |s| {
        s.buildings().iter().all(|b| b.id != house)
    });
    assert!(razed, "the house was never razed");
    // A House costs 25 wood: destroyed value 25, Military 5.
    assert_eq!(sim.destroyed_value_of(OWNER_PLAYER), 25);
    assert_eq!(sim.score_for(OWNER_PLAYER).military, 5);
    assert_eq!(sim.score_for(OWNER_OPPONENT).military, 0);
}
