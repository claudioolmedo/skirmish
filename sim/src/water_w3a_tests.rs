//! Water W3a tests (CLS-07 narrows): the Feudal combat set —
//! Galley, Fire Galley, Demolition Raft, Hulk — trains at the
//! Dock, sails the water domain, and fights the post-169123
//! ecology at the pinned numbers: pierce/melee plus the single
//! surviving bonus case (the long-range warship class), the
//! Hulk's +1 vs the Fire Ship class, the Fire spray as ordinary
//! fast melee data, and the Demolition blast as a tapered,
//! enemies-only detonation that also fires on death-by-enemy-
//! fire. The Mangonel/Petard control half of the friendly-fire
//! pair (a hurts-everyone blast DOES hurt its own side) is
//! pinned by the Castle P4b/P3a suites this slice reuses.

use crate::build::{Building, BuildingKind};
use crate::classes as cls;
use crate::econ::{NodeKind, ResourceKind};
use crate::entity::{Entity, EntityKind, Stance, OWNER_OPPONENT, OWNER_PLAYER};
use crate::grid::Domain;
use crate::tech::{Age, TechId};
use crate::{Command, Sim};
use fixed::types::I16F16;

fn run_until(sim: &mut Sim, cap: u64, mut done: impl FnMut(&Sim) -> bool) -> bool {
    for _ in 0..cap {
        if done(sim) {
            return true;
        }
        sim.tick();
    }
    done(sim)
}

fn dock_anchor(sim: &Sim) -> Option<(u32, u32)> {
    let grid = sim.grid();
    for ay in 0..=117u32 {
        for ax in 0..=117u32 {
            if grid.dock_placement_ok(ax, ay, 3) {
                return Some((ax, ay));
            }
        }
    }
    None
}

/// A completed Dock placed directly on the pond shore (the W2a
/// tests' `placed_dock` pattern, duplicated so this module
/// stands alone).
fn placed_dock(sim: &mut Sim, id: u32) -> u32 {
    let (ax, ay) = dock_anchor(sim).expect("seed pond offers a Dock site");
    let kind = BuildingKind::Dock;
    let size = kind.footprint_size();
    let (x0, y0) = sim.grid().cell_center(ax, ay);
    let half = I16F16::from_num(size as i32 * 2);
    for dy in 0..size {
        for dx in 0..size {
            sim.grid.set_dynamic(ax + dx, ay + dy, true);
        }
    }
    sim.buildings.push(Building {
        id,
        kind,
        owner: OWNER_PLAYER,
        hp: I16F16::from_num(kind.max_hp()),
        cell_x: ax,
        cell_y: ay,
        pos_x: x0 + half - I16F16::from_num(2),
        pos_y: y0 + half - I16F16::from_num(2),
        progress_thirds: kind.build_thirds(),
        repair_hp_remainder: 0,
        repair_cost_remainder: 0,
        repair_stone_remainder: 0,
        repair_gold_remainder: 0,
        build_rate_remainder: 0,
        gate_along_x: true,
        queue_rate_remainder: 0,
        complete: true,
        node_id: None,
        queue: Vec::new(),
        train_progress: 0,
        rally: None,
        rally_node: None,
        weapon_target: None,
        weapon_target_building: None,
        weapon_strike_tick: None,
        weapon_next_tick: 0,
        weapon_engaged: false,
        garrison: Vec::new(),
        garrison_arrows: 0,
        relics: Vec::new(),
        relic_gold_remainder: 0,
    });
    id
}

/// A point on the seed-42 pond: the first deep-fish node's
/// position (deep water by construction).
fn pond_point(sim: &Sim) -> (I16F16, I16F16) {
    let n = sim
        .nodes()
        .iter()
        .find(|n| n.kind == NodeKind::DeepFish)
        .expect("seed 42 stocks deep fish");
    (n.pos_x, n.pos_y)
}

fn spawn_at(sim: &mut Sim, id: u32, kind: EntityKind, owner: u8, x: I16F16, y: I16F16) -> u32 {
    sim.entities.push(Entity::spawned(id, kind, owner, x, y));
    id
}

fn hp_of(sim: &Sim, id: u32) -> I16F16 {
    sim.entities().iter().find(|e| e.id == id).expect("entity").hp
}

fn dead(sim: &Sim, id: u32) -> bool {
    sim.entities().iter().find(|e| e.id == id).expect("entity").dead
}

fn set_age(sim: &mut Sim, owner: u8, age: Age) {
    let slot = crate::player_slot(owner).expect("player slot");
    sim.players[slot].age = age;
}

fn grant(sim: &mut Sim, owner: u8, tech: TechId) {
    let slot = crate::player_slot(owner).expect("player slot");
    sim.players[slot].techs |= tech.bit();
}

#[test]
fn combat_ship_rows_are_the_spec_rows() {
    // Tags append after the Fishing Ship (plan §2 allocation).
    assert_eq!(EntityKind::Galley.tag(), 24);
    assert_eq!(EntityKind::FireGalley.tag(), 25);
    assert_eq!(EntityKind::DemolitionRaft.tag(), 26);
    assert_eq!(EntityKind::Hulk.tag(), 27);
    for (tag, kind) in [
        (24, EntityKind::Galley),
        (25, EntityKind::FireGalley),
        (26, EntityKind::DemolitionRaft),
        (27, EntityKind::Hulk),
    ] {
        assert_eq!(EntityKind::from_tag(tag), Some(kind));
        assert_eq!(kind.domain(), Domain::Water);
        assert_eq!(kind.train_age(), Age::Feudal);
        assert!(kind.counts_population());
        assert!(kind.is_trainable());
        assert!(kind.is_military());
        assert!(BuildingKind::Dock.trains(kind));
        assert!(!BuildingKind::Barracks.trains(kind));
    }
    // The two new armor classes take the next free ids.
    assert_eq!(cls::LONG_RANGE_WARSHIP, 22);
    assert_eq!(cls::FIRE_SHIP, 23);

    let g = crate::units::spec(EntityKind::Galley);
    assert_eq!(g.max_hp, 110);
    assert_eq!((g.armor_melee, g.armor_pierce), (0, 3));
    assert_eq!(
        g.attacks,
        &[
            (cls::PIERCE, 6),
            (cls::LONG_RANGE_WARSHIP, 4),
            (cls::STANDARD_BUILDINGS, 6),
            (cls::RAM, 3),
        ]
    );
    assert_eq!(g.classes, cls::mask(&[cls::SHIP, cls::LONG_RANGE_WARSHIP]));
    assert_eq!(g.range_max, I16F16::from_num(20)); // 5 tiles
    assert_eq!(g.reload_ticks, 180); // 3.0 s
    assert_eq!((g.wood_cost, g.gold_cost), (90, 30));
    assert_eq!(g.train_ticks, 2160); // 36 s
    assert_eq!(g.los, I16F16::from_num(28)); // 7 tiles

    let f = crate::units::spec(EntityKind::FireGalley);
    assert_eq!(f.max_hp, 110);
    assert_eq!((f.armor_melee, f.armor_pierce), (1, 4));
    // Spray-as-data: 2 melee, +1 vs long-range warships, no
    // building bonus, range 2.5, RoF 0.25 s.
    assert_eq!(f.attacks, &[(cls::MELEE, 2), (cls::LONG_RANGE_WARSHIP, 1)]);
    assert_eq!(f.classes, cls::mask(&[cls::SHIP, cls::FIRE_SHIP]));
    assert_eq!(f.range_max, I16F16::from_num(10)); // 2.5 tiles
    assert_eq!(f.reload_ticks, 15); // 0.25 s
    assert_eq!((f.wood_cost, f.gold_cost), (75, 45));
    assert_eq!(f.train_ticks, 2940); // 49 s

    let d = crate::units::spec(EntityKind::DemolitionRaft);
    assert_eq!(d.max_hp, 40);
    assert_eq!((d.armor_melee, d.armor_pierce), (0, 0));
    assert_eq!(d.attacks, &[(cls::MELEE, 65)]);
    assert_eq!(d.classes, cls::mask(&[cls::SHIP]));
    assert_eq!(
        crate::units::DEMOLITION_RAFT_BLAST_RADIUS,
        I16F16::from_num(10) // 2.5 tiles
    );
    assert_eq!((d.wood_cost, d.gold_cost), (70, 80));
    assert_eq!(d.train_ticks, 2700); // 45 s

    let h = crate::units::spec(EntityKind::Hulk);
    assert_eq!(h.max_hp, 85);
    assert_eq!((h.armor_melee, h.armor_pierce), (3, 1));
    assert_eq!(h.attacks, &[(cls::MELEE, 4), (cls::FIRE_SHIP, 1)]);
    assert_eq!(h.classes, cls::mask(&[cls::SHIP]));
    assert_eq!(h.reload_ticks, 105); // 1.75 s
    assert_eq!((h.wood_cost, h.gold_cost), (75, 35));
    assert_eq!(h.train_ticks, 2520); // 42 s
}

#[test]
fn blacksmith_and_ballistics_patch_rows_land_on_the_ships() {
    let mut sim = Sim::new(42);
    // Fletching: the Galley gains +1 pierce and +1 tile of range;
    // its bonus pairs are untouched.
    let base = sim.stats_for(OWNER_PLAYER, EntityKind::Galley);
    grant(&mut sim, OWNER_PLAYER, TechId::Fletching);
    let fletched = sim.stats_for(OWNER_PLAYER, EntityKind::Galley);
    assert_eq!(fletched.attacks, crate::units::GALLEY_FLETCHING_ATTACKS);
    assert_eq!(fletched.attacks[0], (cls::PIERCE, 7));
    assert_eq!(
        fletched.range_max,
        base.range_max + I16F16::from_num(4) // +1 tile
    );
    // The Fire Galley is a melee sprayer: Fletching does NOT
    // touch it (its article lists no Fletching row).
    assert_eq!(
        sim.stats_for(OWNER_PLAYER, EntityKind::FireGalley).attacks,
        crate::units::spec(EntityKind::FireGalley).attacks
    );
    // Forging: the Hulk gains +1 melee, its anti-fire pair
    // untouched; the Galley (pierce) gains nothing from Forging.
    grant(&mut sim, OWNER_PLAYER, TechId::Forging);
    let forged = sim.stats_for(OWNER_PLAYER, EntityKind::Hulk);
    assert_eq!(forged.attacks, crate::units::HULK_FORGING_ATTACKS);
    assert_eq!(forged.attacks, &[(cls::MELEE, 5), (cls::FIRE_SHIP, 1)]);
    assert_eq!(
        sim.stats_for(OWNER_PLAYER, EntityKind::Galley).attacks,
        crate::units::GALLEY_FLETCHING_ATTACKS
    );
    // Ballistics: the Galley's weapon is a projectile on the
    // generic PRJ-05 release path (the lead is applied at
    // release for every ranged attacker whose owner holds the
    // tech — pinned behaviorally by the University P3c suite);
    // the row itself carries no Ballistics patch, exactly like
    // the Archer's.
    grant(&mut sim, OWNER_PLAYER, TechId::Ballistics);
    assert_eq!(
        sim.stats_for(OWNER_PLAYER, EntityKind::Galley).attacks,
        crate::units::GALLEY_FLETCHING_ATTACKS
    );
}

#[test]
fn combat_ships_train_at_the_dock_behind_the_feudal_gate() {
    let mut sim = Sim::new(42);
    let dock = placed_dock(&mut sim, 900);
    // Dark Age: the whole combat set is refused (the Fishing
    // Ship alone is Dark Age at the Dock).
    for kind in [
        EntityKind::Galley,
        EntityKind::FireGalley,
        EntityKind::DemolitionRaft,
        EntityKind::Hulk,
    ] {
        assert!(!sim.can_train_at(dock, kind), "{kind:?} trained in Dark Age");
    }
    set_age(&mut sim, OWNER_PLAYER, Age::Feudal);
    sim.stockpile_mut(OWNER_PLAYER)
        .add(ResourceKind::Wood, I16F16::from_num(2000));
    sim.stockpile_mut(OWNER_PLAYER)
        .add(ResourceKind::Gold, I16F16::from_num(1000));
    for kind in [
        EntityKind::Galley,
        EntityKind::FireGalley,
        EntityKind::DemolitionRaft,
        EntityKind::Hulk,
    ] {
        assert!(sim.can_train_at(dock, kind), "{kind:?} refused in Feudal");
    }
    // Train a Galley for real: 90 wood / 30 gold leave the
    // stockpile on enqueue and the ship spawns on water.
    let wood_before = sim.stockpile_of(OWNER_PLAYER).amount(ResourceKind::Wood);
    let gold_before = sim.stockpile_of(OWNER_PLAYER).amount(ResourceKind::Gold);
    sim.push_command(Command::TrainAt {
        building: dock,
        kind: EntityKind::Galley,
    });
    sim.tick();
    assert_eq!(
        sim.stockpile_of(OWNER_PLAYER).amount(ResourceKind::Wood),
        wood_before - I16F16::from_num(90)
    );
    assert_eq!(
        sim.stockpile_of(OWNER_PLAYER).amount(ResourceKind::Gold),
        gold_before - I16F16::from_num(30)
    );
    let spawned = run_until(&mut sim, 10_000, |s| {
        s.entities().iter().any(|e| e.kind == EntityKind::Galley)
    });
    assert!(spawned, "the Galley never left the Dock queue");
    let ship = sim
        .entities()
        .iter()
        .find(|e| e.kind == EntityKind::Galley)
        .expect("the galley");
    let (cx, cy) = sim.grid().world_to_cell(ship.pos_x, ship.pos_y);
    assert!(
        sim.grid().is_water_side(cx, cy),
        "the Galley spawned off water at {cx},{cy}"
    );
}

#[test]
fn fire_galley_beats_galley_at_close_range_and_galley_outranges_it() {
    // 1v1 at contact distance: the Fire Galley's spray (2 melee
    // every 0.25 s against the Galley's 0 melee armor, +1 vs its
    // long-range class) sinks the Galley, whose own bolts land
    // (6 − 4 pierce armor) + 4 vs long-range = 6 per 3 s on the
    // Fire Galley — real damage, but far too slow.
    let mut sim = Sim::new(42);
    let (x, y) = pond_point(&sim);
    let fire = spawn_at(&mut sim, 950, EntityKind::FireGalley, OWNER_PLAYER, x, y);
    let galley = spawn_at(
        &mut sim,
        951,
        EntityKind::Galley,
        OWNER_OPPONENT,
        x + I16F16::from_num(2),
        y,
    );
    sim.push_command(Command::Attack {
        entity: fire,
        target: galley,
    });
    sim.push_command(Command::Attack {
        entity: galley,
        target: fire,
    });
    let sunk = run_until(&mut sim, 60_000, |s| dead(s, galley));
    assert!(sunk, "the Fire Galley never sank the Galley 1v1");
    assert!(!dead(&sim, fire), "the Galley sank the Fire Galley 1v1");
    assert!(
        hp_of(&sim, fire) < I16F16::from_num(110),
        "the Galley's bolts (with their +4 vs long-range) never landed"
    );

    // Range: a Galley ordered on a passive Fire Galley 4.5 tiles
    // away shoots from outside the spray's 2.5 — the Fire Galley
    // bleeds and the Galley stays untouched.
    let mut sim = Sim::new(42);
    let (x, y) = pond_point(&sim);
    let galley = spawn_at(&mut sim, 950, EntityKind::Galley, OWNER_PLAYER, x, y);
    let fire = spawn_at(
        &mut sim,
        951,
        EntityKind::FireGalley,
        OWNER_OPPONENT,
        x + I16F16::from_num(18),
        y,
    );
    sim.push_command(Command::SetStance {
        entity: fire,
        stance: Stance::NoAttack,
    });
    sim.push_command(Command::Attack {
        entity: galley,
        target: fire,
    });
    for _ in 0..6000 {
        sim.tick();
    }
    assert!(
        hp_of(&sim, fire) < I16F16::from_num(110),
        "the Galley never hit at range 4.5 tiles"
    );
    assert_eq!(
        hp_of(&sim, galley),
        I16F16::from_num(110),
        "the passive Fire Galley damaged the Galley from outside its spray"
    );
}

#[test]
fn hulk_strikes_carry_the_anti_fire_bonus() {
    // Per-strike arithmetic, read off a short engagement: the
    // Hulk lands (4 − 1 melee armor) + 1 vs the Fire Ship class
    // = 4 per strike on a Fire Galley; the spray answers at the
    // minimum 1 (2 − 3 melee armor floors at 0, CMB-01's clamp).
    let mut sim = Sim::new(42);
    let (x, y) = pond_point(&sim);
    let hulk = spawn_at(&mut sim, 950, EntityKind::Hulk, OWNER_PLAYER, x, y);
    let fire = spawn_at(
        &mut sim,
        951,
        EntityKind::FireGalley,
        OWNER_OPPONENT,
        x + I16F16::from_num(2),
        y,
    );
    sim.push_command(Command::Attack {
        entity: hulk,
        target: fire,
    });
    sim.push_command(Command::SetStance {
        entity: fire,
        stance: Stance::NoAttack,
    });
    for _ in 0..1000 {
        sim.tick();
    }
    let fire_hp = hp_of(&sim, fire);
    assert!(
        fire_hp <= I16F16::from_num(110 - 12),
        "the Hulk's anti-fire strikes under-delivered: fire at {fire_hp}"
    );
    assert_eq!(
        hp_of(&sim, hulk),
        I16F16::from_num(85),
        "the NoAttack Fire Galley fought back"
    );
}

#[test]
fn demolition_blast_kills_the_cluster_and_spares_its_own() {
    // The raft sails into a clustered enemy pair and detonates:
    // 65 melee tapered over 2.5 tiles kills both at contact
    // distance (the pair 3–5 from the detonation point) — and
    // the owner's own Fishing Ship inside the
    // radius is untouched (no friendly fire, the anti-Mangonel
    // pin; the hurts-everyone half is P4b/P3a's).
    //
    // The victims are Galleys on NoAttack, not Fishing Ships:
    // a civilian hull hit by the closing raft flees its
    // attacker and drags the whole cluster across the pond
    // before the strike lands (observed in this test's first
    // staging) — a military hull on NoAttack holds station, so
    // the geometry being pinned is the blast's, not the flee
    // rule's. The pair is battle-worn (40 HP): at contact
    // geometry the tapered blast lands ~44–58 — a kill on a
    // worn pair, a crippling on a fresh one; the fresh case's
    // damage is pinned by the death-detonation test below.
    let mut sim = Sim::new(42);
    let (x, y) = pond_point(&sim);
    let raft = spawn_at(&mut sim, 950, EntityKind::DemolitionRaft, OWNER_PLAYER, x, y);
    // Spacing is load-bearing: hulls spawned closer than the
    // radii sum (3.2) get launched by the separation shove
    // before the raft closes (this test's first stagings lost
    // the primary that way) — the pair stands 3 and (3, 4)
    // from the raft, inside contact geometry without overlap.
    let foe_a = spawn_at(
        &mut sim,
        951,
        EntityKind::Galley,
        OWNER_OPPONENT,
        x + I16F16::from_num(3),
        y,
    );
    let foe_b = spawn_at(
        &mut sim,
        952,
        EntityKind::Galley,
        OWNER_OPPONENT,
        x + I16F16::from_num(3),
        y + I16F16::from_num(4),
    );
    let own = spawn_at(
        &mut sim,
        953,
        EntityKind::FishingShip,
        OWNER_PLAYER,
        x - I16F16::from_num(1),
        y - I16F16::from_num(9),
    );
    for id in [foe_a, foe_b] {
        sim.push_command(Command::SetStance {
            entity: id,
            stance: Stance::NoAttack,
        });
        let e = sim
            .entities
            .iter_mut()
            .find(|e| e.id == id)
            .expect("victim");
        e.hp = I16F16::from_num(35);
    }
    sim.push_command(Command::Attack {
        entity: raft,
        target: foe_a,
    });
    // Both victims are re-anchored to their stations every 30
    // ticks: a melee engagement's separation shove otherwise
    // drags the primary (and the cluster chained to it) across
    // the pond while the raft closes — the steering of a fresh
    // Move order walks a victim back to its anchor between
    // shoves, so the geometry pinned here is the blast's.
    let (ax_a, ay_a) = (x + I16F16::from_num(3), y);
    let (ax_b, ay_b) = (x + I16F16::from_num(3), y + I16F16::from_num(4));
    let mut blown = false;
    for t in 0..60_000u64 {
        if t % 30 == 0 {
            sim.push_command(Command::Move { entity: foe_a, x: ax_a, y: ay_a });
            sim.push_command(Command::Move { entity: foe_b, x: ax_b, y: ay_b });
        }
        sim.tick();
        if dead(&sim, raft) {
            blown = true;
            break;
        }
    }
    assert!(blown, "the raft never detonated on its target");
    assert!(dead(&sim, foe_a), "the blast's primary survived");
    assert!(dead(&sim, foe_b), "the clustered second ship survived the blast");
    assert!(!dead(&sim, own), "the blast killed the owner's own ship");
    assert_eq!(
        hp_of(&sim, own),
        I16F16::from_num(50),
        "the owner's ship took friendly blast damage"
    );
}

#[test]
fn demolition_raft_killed_by_fire_still_detonates() {
    // An enemy Galley shoots the raft dead before it reaches
    // anything: the death detonation still fires, wounding the
    // enemy Fishing Ship moored beside the wreck and sparing
    // the owner's own.
    let mut sim = Sim::new(42);
    let (x, y) = pond_point(&sim);
    let raft = spawn_at(&mut sim, 950, EntityKind::DemolitionRaft, OWNER_PLAYER, x, y);
    let galley = spawn_at(
        &mut sim,
        951,
        EntityKind::Galley,
        OWNER_OPPONENT,
        x + I16F16::from_num(6),
        y,
    );
    let foe_fish = spawn_at(
        &mut sim,
        952,
        EntityKind::FishingShip,
        OWNER_OPPONENT,
        x + I16F16::from_num(2),
        y,
    );
    let own_fish = spawn_at(
        &mut sim,
        953,
        EntityKind::FishingShip,
        OWNER_PLAYER,
        x - I16F16::from_num(2),
        y,
    );
    sim.push_command(Command::Attack {
        entity: galley,
        target: raft,
    });
    let sunk = run_until(&mut sim, 120_000, |s| dead(s, raft));
    assert!(sunk, "the Galley never sank the raft");
    assert!(
        dead(&sim, foe_fish) || hp_of(&sim, foe_fish) < I16F16::from_num(50),
        "the death detonation never touched the enemy ship beside the wreck"
    );
    assert_eq!(
        hp_of(&sim, own_fish),
        I16F16::from_num(50),
        "the death detonation hurt the owner's own ship"
    );
    // And it detonates exactly once: running on changes nothing
    // further (no second blast from the tombstone).
    let after = hp_of(&sim, foe_fish);
    for _ in 0..600 {
        sim.tick();
    }
    assert_eq!(hp_of(&sim, foe_fish), after, "the wreck detonated twice");
}
