//! MOV-05 tests (2026-11-16): the per-unit collision-radius table
//! converges on the documented DE values, replacing the adopted
//! constants the rows carried until now.
//!
//! Sources (public documentation only): the central DE data package
//! (Claudio's central research, 2026-11-16) — villager/foot 0.20
//! tiles, Karambit 0.15, cavalry 0.25, war ships 0.50, Fishing Ship
//! 0.40 and Trade Cog 0.30 (official update 81058 notes: both
//! "reduced from 0.5", which also pins the ship default the
//! Transport Ship keeps — it was never carved out) — and the
//! Jineapple Infosheet legend's collision lines, extracted
//! 2026-11-16 with Claudio's approval: "Other foot units,
//! Missionary: 0.2×0.2 tiles; Cavalry units, Trade Cart:
//! 0.25×0.25; War Wagon, Hussite Wagon, rams: 0.45×0.45; Other
//! siege units, incl. Trebuchets: 0.5×0.5". The legend presents
//! class lines, so the Monk (the Missionary's line), the King and
//! the Petard (this table's own INFANTRY row) take the foot line.
//! Scale: 1 tile = 4 world units; bits = tiles × 262 144.
//!
//! The behavioural probes spawn orderless pairs 1.0 world unit
//! apart and let the mutual shove (steer.rs, M2) settle them:
//! a pair comes to rest at exactly the sum of its two radii, so
//! the resting distance IS the table value, observed through
//! movement rather than read off the row.

use crate::entity::{Entity, EntityKind, OWNER_PLAYER};
use crate::steer::vec_len;
use crate::units::spec;
use crate::Sim;
use fixed::types::I16F16;

fn bits(kind: EntityKind) -> i32 {
    spec(kind).radius.to_bits()
}

#[test]
fn mov05_radius_table_matches_the_documented_values() {
    // Foot units: 0.20 tiles = 0.8 world units (package + legend
    // foot line; the Monk is named there through the Missionary,
    // the King and the Petard ride the same foot line).
    for kind in [
        EntityKind::Villager,
        EntityKind::Militia,
        EntityKind::ManAtArms,
        EntityKind::Archer,
        EntityKind::Spearman,
        EntityKind::Skirmisher,
        EntityKind::Monk,
        EntityKind::King,
        EntityKind::Petard,
    ] {
        assert_eq!(bits(kind), 52_429, "{kind:?}: foot radius 0.20 tiles");
    }
    // Cavalry and the Trade Cart: 0.25 tiles = 1.0 world unit
    // (package; the legend names the Trade Cart on this line).
    for kind in [
        EntityKind::ScoutCavalry,
        EntityKind::LightCavalry,
        EntityKind::Knight,
        EntityKind::CavalryArcher,
        EntityKind::TradeCart,
    ] {
        assert_eq!(bits(kind), 65_536, "{kind:?}: cavalry/cart radius 0.25 tiles");
    }
    // Rams: 0.45 tiles = 1.8 world units (legend rams line).
    assert_eq!(bits(EntityKind::BatteringRam), 117_965, "ram radius 0.45 tiles");
    // Other siege: 0.5 tiles = 2.0 world units (legend siege line).
    for kind in [EntityKind::Mangonel, EntityKind::Scorpion] {
        assert_eq!(bits(kind), 131_072, "{kind:?}: siege radius 0.5 tiles");
    }
    // War ships and the Transport Ship: 0.5 tiles = 2.0 world
    // units (package; update 81058's from-values pin the ship
    // default at 0.5 and carve out only the two rows below).
    for kind in [
        EntityKind::Galley,
        EntityKind::WarGalley,
        EntityKind::FireGalley,
        EntityKind::FireShip,
        EntityKind::Hulk,
        EntityKind::WarHulk,
        EntityKind::DemolitionRaft,
        EntityKind::DemolitionShip,
        EntityKind::TransportShip,
    ] {
        assert_eq!(bits(kind), 131_072, "{kind:?}: war-ship radius 0.5 tiles");
    }
    // Fishing Ship: 0.4 tiles = 1.6 world units (update 81058).
    assert_eq!(bits(EntityKind::FishingShip), 104_858, "fishing radius 0.4 tiles");
    // Trade Cog: 0.3 tiles = 1.2 world units (update 81058).
    assert_eq!(bits(EntityKind::TradeCog), 78_643, "trade cog radius 0.3 tiles");
    // Rows the DE sources do not cover keep their adopted values,
    // asserted so a silent drift shows up here first: the Gaia
    // animals, the Relic object, and the Town Center building row.
    assert_eq!(bits(EntityKind::Sheep), 65_536, "sheep: adopted, unchanged");
    assert_eq!(bits(EntityKind::Boar), 78_643, "boar: adopted, unchanged");
    assert_eq!(bits(EntityKind::Deer), 65_536, "deer: adopted, unchanged");
    assert_eq!(bits(EntityKind::Wolf), 65_536, "wolf: adopted, unchanged");
    assert_eq!(bits(EntityKind::Relic), 32_768, "relic: adopted, unchanged");
    assert_eq!(bits(EntityKind::TownCenter), 235_930, "TC: building row, out of scope");
}

fn ent(sim: &Sim, id: u32) -> &Entity {
    sim.entities().iter().find(|e| e.id == id).unwrap()
}

/// A clear land cell near the map centre: land domain unblocked
/// in the whole 3×3, so a spawned pair shoves in the open.
fn clear_land_spot(sim: &Sim) -> (I16F16, I16F16) {
    let g = sim.grid();
    for cy in 40..80 {
        for cx in 40..80 {
            let ok = (0..3).all(|dy| {
                (0..3).all(|dx| {
                    let (x, y) = (cx + dx, cy + dy);
                    !g.is_blocked(x, y) && !g.is_water(x, y) && !g.is_shallows(x, y)
                })
            });
            if ok {
                return g.cell_center(cx + 1, cy + 1);
            }
        }
    }
    panic!("seed 42: no clear land spot near the centre");
}

/// The centre cell of a generated pond: deep water, with
/// water-side cells (deep or shallows — both are ship domain)
/// on both horizontal neighbours, so a spawned pair settling
/// along x stays in the water the whole way. (Seed 42's ponds
/// are plus-shaped: a deep cross in a shallows ring.)
fn open_water_spot(sim: &Sim) -> (I16F16, I16F16) {
    let g = sim.grid();
    for cy in 1..g.height() - 1 {
        for cx in 1..g.width() - 1 {
            let wet = |x: u32, y: u32| g.is_water(x, y) || g.is_shallows(x, y);
            if g.is_water(cx, cy) && wet(cx - 1, cy) && wet(cx + 1, cy) {
                return g.cell_center(cx, cy);
            }
        }
    }
    panic!("seed 42: no open water spot");
}

/// Spawn two orderless units of `kind` 1.0 world unit apart
/// around `centre`, run the sim, and return the pair's resting
/// distance in raw bits.
fn settled_pair_distance(kind: EntityKind, centre: (I16F16, I16F16), ticks: u64) -> i32 {
    let mut sim = Sim::new(42);
    let half = I16F16::from_bits(32_768); // 0.5 world units
    sim.entities.push(Entity::spawned(
        9_001,
        kind,
        OWNER_PLAYER,
        centre.0 - half,
        centre.1,
    ));
    sim.entities.push(Entity::spawned(
        9_002,
        kind,
        OWNER_PLAYER,
        centre.0 + half,
        centre.1,
    ));
    for _ in 0..ticks {
        sim.tick();
    }
    let (a, b) = (ent(&sim, 9_001), ent(&sim, 9_002));
    vec_len(b.pos_x - a.pos_x, b.pos_y - a.pos_y).to_bits()
}

fn assert_settles(kind: EntityKind, centre: (I16F16, I16F16), expected_bits: i32) {
    let measured = settled_pair_distance(kind, centre, 240);
    assert!(
        (measured - expected_bits).abs() <= 8,
        "{kind:?} pair settled at {measured} bits, expected {expected_bits} (sum of radii)",
    );
}

#[test]
fn foot_pairs_settle_at_the_documented_foot_diameter() {
    let mut sim = Sim::new(42);
    let spot = clear_land_spot(&sim);
    drop(sim);
    // Two villagers: 2 × 0.8 = 1.6 world units (was 2.0).
    assert_settles(EntityKind::Villager, spot, 104_858);
}

#[test]
fn cavalry_pairs_keep_the_documented_cavalry_diameter() {
    let sim = Sim::new(42);
    let spot = clear_land_spot(&sim);
    drop(sim);
    // Two knights: 2 × 1.0 = 2.0 world units — the control family,
    // unchanged by MOV-05.
    assert_settles(EntityKind::Knight, spot, 131_072);
}

#[test]
fn siege_pairs_settle_at_the_documented_siege_diameters() {
    let sim = Sim::new(42);
    let spot = clear_land_spot(&sim);
    drop(sim);
    // Two rams: 2 × 1.8 = 3.6 world units (was 2.4).
    assert_settles(EntityKind::BatteringRam, spot, 235_930);
    // Two scorpions: 2 × 2.0 = 4.0 world units (was 2.4).
    assert_settles(EntityKind::Scorpion, spot, 262_144);
}

#[test]
fn warship_pairs_settle_at_the_documented_ship_diameter() {
    let sim = Sim::new(42);
    let spot = open_water_spot(&sim);
    drop(sim);
    // Two galleys: 2 × 2.0 = 4.0 world units (was 3.2).
    assert_settles(EntityKind::Galley, spot, 262_144);
}

#[test]
fn villager_clusters_pack_to_foot_spacing_not_tighter() {
    // Four villagers spawned in a tight knot (0.4 world units
    // from the centre) must end no closer than the foot diameter
    // allows: the woodline bunch, in miniature.
    let mut sim = Sim::new(42);
    let spot = clear_land_spot(&sim);
    let off = I16F16::from_bits(26_214); // 0.4 world units
    let mut id = 9_100;
    for (dx, dy) in [(-1, -1), (-1, 1), (1, -1), (1, 1)] {
        sim.entities.push(Entity::spawned(
            id,
            EntityKind::Villager,
            OWNER_PLAYER,
            spot.0 + I16F16::from_bits(off.to_bits() * dx),
            spot.1 + I16F16::from_bits(off.to_bits() * dy),
        ));
        id += 1;
    }
    for _ in 0..240 {
        sim.tick();
    }
    let pts: Vec<(i32, i32)> = (9_100..9_104)
        .map(|i| {
            let e = ent(&sim, i);
            (e.pos_x.to_bits(), e.pos_y.to_bits())
        })
        .collect();
    for a in 0..4 {
        for b in (a + 1)..4 {
            let d = vec_len(
                I16F16::from_bits(pts[b].0 - pts[a].0),
                I16F16::from_bits(pts[b].1 - pts[a].1),
            )
            .to_bits();
            assert!(
                d >= 104_858 - 8,
                "villagers {a} and {b} ended {d} bits apart, inside the 1.6-unit foot diameter",
            );
        }
    }
}


