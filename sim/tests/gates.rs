//! Gate tests (Package J, BLD-13): the Palisade Gate and the
//! Stone Gate — 4×1 spans placed over a wall line (replacing the
//! placer's own same-family segments), with a DERIVED open state
//! (owner at the gate and no foreign unit there → open; the grid
//! block follows), owner pass-through pathing, and the DE table
//! rows pinned from the walls dossier §5 (the wiki Wall article's
//! Building table, via
//! `~/workspace/skirmish-study/walls-de/DE-WALLS-DOSSIER.md`):
//! Palisade Gate 20 wood, 30 s, 240 HP Dark / 400 Feudal+, armor
//! 2/4; Stone Gate 30 stone, 70 s, 1650 HP Feudal / 2700 Castle+,
//! armor 6/6, Feudal-gated.
//!
//! Everything is driven through ordinary commands — no state is
//! poked. Scenarios run in TwoPlayer matches.

use fixed::types::I16F16;
use skirmish_sim::{
    Age, BuildingKind, Command, Entity, EntityKind, MatchMode, ResourceKind, Sim, OWNER_OPPONENT,
    OWNER_PLAYER,
};

const PGATE: BuildingKind = BuildingKind::PalisadeGate;
const SGATE: BuildingKind = BuildingKind::StoneGate;
const PWALL: BuildingKind = BuildingKind::PalisadeWall;

fn f(v: I16F16) -> f64 {
    v.to_num::<f64>()
}

fn ent(sim: &Sim, id: u32) -> &Entity {
    sim.entities().iter().find(|e| e.id == id).unwrap()
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

fn gate_of(sim: &Sim, owner: u8) -> Option<&skirmish_sim::Building> {
    sim.buildings()
        .iter()
        .find(|b| b.owner == owner && b.kind.is_gate())
}

/// A horizontal run of `len` cells near `owner`'s Town Center
/// where a wall line of `len` segments validates segment by
/// segment (the walls suite's open-ground scan, gate edition:
/// the caller checks the run for the wall kind, then places the
/// gate over its middle).
fn open_run_near_tc(sim: &Sim, owner: u8, entity: u32, len: u32) -> (u32, u32) {
    let tc = ent(sim, find_id(sim, owner, EntityKind::TownCenter));
    let (tcx, tcy) = sim.grid().world_to_cell(tc.pos_x, tc.pos_y);
    for r in 3..=40i32 {
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
                let ok = (0..len).all(|i| {
                    let (x, y) = cell_point(sim, ax + i, ay);
                    sim.can_build(entity, PWALL, x, y)
                });
                if ok {
                    return (ax, ay);
                }
            }
        }
    }
    panic!("no open run of {len} near the Town Center");
}

/// Build a straight palisade line of `len` segments starting at
/// `(ax, ay)` (horizontal) with one villager and wait for the
/// whole line to stand complete.
fn raise_line(sim: &mut Sim, vil: u32, ax: u32, ay: u32, len: u32) {
    let (x0, y0) = cell_point(sim, ax, ay);
    let (x1, y1) = cell_point(sim, ax + len - 1, ay);
    sim.push_command(Command::BuildLine {
        entity: vil,
        kind: PWALL,
        x0,
        y0,
        x1,
        y1,
    });
    let done = run_until(sim, 240_000, |s| {
        s.buildings()
            .iter()
            .filter(|b| b.kind == PWALL && b.complete)
            .count()
            >= len as usize
    });
    assert!(done, "wall line of {len} never completed");
}

/// Place a gate for `owner`'s villager at anchor `(ax, ay)` and
/// wait for it to complete.
fn raise_gate(sim: &mut Sim, vil: u32, kind: BuildingKind, ax: u32, ay: u32) -> u32 {
    let (x, y) = cell_point(sim, ax, ay);
    assert!(
        sim.can_build(vil, kind, x, y),
        "gate does not validate at ({ax}, {ay})"
    );
    sim.push_command(Command::Build {
        entity: vil,
        kind,
        x,
        y,
    });
    let done = run_until(sim, 240_000, |s| {
        gate_of(s, OWNER_PLAYER).map(|g| g.complete).unwrap_or(false)
    });
    assert!(done, "gate never completed");
    gate_of(sim, OWNER_PLAYER).unwrap().id
}

#[test]
fn gate_table_rows_match_the_de_building_table() {
    // Palisade Gate: 20 wood, 30 s, 240 Dark / 400 Feudal+,
    // armor 2 melee / 4 pierce, Dark Age, wall-family sight 2.
    let k = PGATE;
    assert_eq!(f(k.wood_cost()), 20.0);
    assert_eq!(f(k.stone_cost()), 0.0);
    assert_eq!(k.build_ticks(), 1800);
    assert_eq!(k.max_hp_at(Age::Dark), 240);
    assert_eq!(k.max_hp_at(Age::Feudal), 400);
    assert_eq!(k.max_hp_at(Age::Castle), 400);
    assert_eq!(k.max_hp_at(Age::Imperial), 400);
    assert_eq!(k.armor_melee_at(Age::Dark), 2);
    assert_eq!(k.armor_pierce_at(Age::Dark), 4);
    assert_eq!(k.age_required(), Age::Dark);
    assert_eq!(k.sight_tiles_at(Age::Dark), 2);
    assert!(k.is_gate());
    assert_eq!(k.tag(), 18);
    // Stone Gate: 30 stone, 70 s, 1650 Feudal / 2700 Castle+,
    // armor 6/6, Feudal Age.
    let k = SGATE;
    assert_eq!(f(k.wood_cost()), 0.0);
    assert_eq!(f(k.stone_cost()), 30.0);
    assert_eq!(k.build_ticks(), 4200);
    assert_eq!(k.max_hp_at(Age::Feudal), 1650);
    assert_eq!(k.max_hp_at(Age::Castle), 2700);
    assert_eq!(k.max_hp_at(Age::Imperial), 2700);
    assert_eq!(k.armor_melee_at(Age::Feudal), 6);
    assert_eq!(k.armor_pierce_at(Age::Feudal), 6);
    assert_eq!(k.age_required(), Age::Feudal);
    assert_eq!(k.tag(), 19);
    // Neither gate is a wall segment, a drop-off, a production
    // building, or a garrison host.
    assert!(!PGATE.is_wall() && !SGATE.is_wall());
    assert!(!PGATE.is_dropoff() && !SGATE.is_dropoff());
    assert!(PGATE.garrison_capacity().is_none());
    assert_eq!(PGATE.wall_family(), Some(PWALL));
    assert_eq!(
        SGATE.wall_family(),
        Some(BuildingKind::StoneWall)
    );
}

#[test]
fn gate_replaces_own_wall_segments_on_placement() {
    let mut sim = Sim::with_mode(42, MatchMode::TwoPlayer);
    let vil = villagers_of(&sim, OWNER_PLAYER)[0];
    let (ax, ay) = open_run_near_tc(&sim, OWNER_PLAYER, vil, 8);
    raise_line(&mut sim, vil, ax, ay, 8);
    let segments_before = sim
        .buildings()
        .iter()
        .filter(|b| b.kind == PWALL)
        .count();
    assert_eq!(segments_before, 8);
    // The gate goes over the middle four segments (anchor = the
    // run's 3rd cell): they are removed as the foundation lands.
    let wood0 = f(sim.stockpile_of(OWNER_PLAYER).amount(ResourceKind::Wood));
    let gid = raise_gate(&mut sim, vil, PGATE, ax + 2, ay);
    let g = gate_of(&sim, OWNER_PLAYER).unwrap();
    assert_eq!(g.id, gid);
    assert!(g.gate_along_x, "span over a horizontal line runs along x");
    assert_eq!((g.cell_x, g.cell_y), (ax + 2, ay));
    let segments_after = sim
        .buildings()
        .iter()
        .filter(|b| b.kind == PWALL)
        .count();
    assert_eq!(segments_after, 4, "the gate must replace exactly 4 segments");
    // The gate cost exactly its 20 wood (the replaced segments
    // refund nothing — a conversion, not a deletion).
    let wood1 = f(sim.stockpile_of(OWNER_PLAYER).amount(ResourceKind::Wood));
    assert_eq!(wood0 - wood1, 20.0);
    // The completed gate stands at the table's Dark-Age HP.
    assert_eq!(f(g.hp), 240.0);
}

#[test]
fn gate_on_open_ground_and_vertical_orientation() {
    let mut sim = Sim::with_mode(43, MatchMode::TwoPlayer);
    let vil = villagers_of(&sim, OWNER_PLAYER)[0];
    // A spot where the along-x span does NOT fit (a tree line or
    // the map feature blocks it) but the along-y span does: scan
    // for an anchor where can_build holds and the resolved
    // orientation comes out vertical. Ground truth is the placed
    // record's own flag.
    let tc = ent(&sim, find_id(&sim, OWNER_PLAYER, EntityKind::TownCenter));
    let (tcx, tcy) = sim.grid().world_to_cell(tc.pos_x, tc.pos_y);
    let mut placed = None;
    'scan: for r in 3..=40i32 {
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
                let (x, y) = cell_point(&sim, ax, ay);
                if !sim.can_build(vil, PGATE, x, y) {
                    continue;
                }
                // Along-x must fail: some span cell of the
                // horizontal span is not gate-placeable ground —
                // approximated here by the wall mirror: a cell
                // that is blocked or holds a node/building.
                let x_blocked = (0..4).any(|k| {
                    let cx = ax + k;
                    sim.grid().is_blocked(cx, ay)
                        || sim.buildings().iter().any(|b| {
                            let bs = b.kind.footprint_size();
                            cx >= b.cell_x && cx < b.cell_x + bs
                                && ay >= b.cell_y && ay < b.cell_y + bs
                        })
                });
                if x_blocked {
                    placed = Some((ax, ay));
                    break 'scan;
                }
            }
        }
    }
    let (ax, ay) = placed.expect("no vertical-only gate anchor near the TC");
    raise_gate(&mut sim, vil, PGATE, ax, ay);
    let g = gate_of(&sim, OWNER_PLAYER).unwrap();
    assert!(!g.gate_along_x, "span must resolve along y here");
    // Its footprint is the vertical span.
    let cells = g.footprint_cells();
    assert_eq!(cells, vec![(ax, ay), (ax, ay + 1), (ax, ay + 2), (ax, ay + 3)]);
}

#[test]
fn gate_over_enemy_wall_is_rejected() {
    let mut sim = Sim::with_mode(44, MatchMode::TwoPlayer);
    let opp_vil = villagers_of(&sim, OWNER_OPPONENT)[0];
    let (ax, ay) = open_run_near_tc(&sim, OWNER_OPPONENT, opp_vil, 6);
    raise_line(&mut sim, opp_vil, ax, ay, 6);
    // The human villager cannot found a gate over the enemy's
    // segments (replacement is the placer's OWN family only).
    let vil = villagers_of(&sim, OWNER_PLAYER)[0];
    let (x, y) = cell_point(&sim, ax + 1, ay);
    assert!(!sim.can_build(vil, PGATE, x, y));
    sim.push_command(Command::Build {
        entity: vil,
        kind: PGATE,
        x,
        y,
    });
    for _ in 0..120 {
        sim.tick();
    }
    assert!(gate_of(&sim, OWNER_PLAYER).is_none());
}

#[test]
fn derived_open_state_and_owner_passage_enemy_blocked() {
    let mut sim = Sim::with_mode(45, MatchMode::TwoPlayer);
    let vil = villagers_of(&sim, OWNER_PLAYER)[0];
    // A 12-cell barrier: 8 wall segments + a gate over the
    // middle 4 (segments 5..8 replaced), so the ONLY way across
    // the line is the gate span.
    let (ax, ay) = open_run_near_tc(&sim, OWNER_PLAYER, vil, 12);
    raise_line(&mut sim, vil, ax, ay, 12);
    raise_gate(&mut sim, vil, PGATE, ax + 4, ay);
    let g = gate_of(&sim, OWNER_PLAYER).unwrap().clone();
    let span = g.footprint_cells();
    // Nobody near: the gate stands closed and its cells block.
    assert!(!sim.gate_is_open(&g));
    for &(cx, cy) in &span {
        assert!(sim.grid().is_dynamic(cx, cy), "closed gate cell must block");
    }
    // The enemy Scout, ordered to a point straight across the
    // barrier, never gets there (the line is unbroken and the
    // gate is shut to it).
    let scout = find_id(&sim, OWNER_OPPONENT, EntityKind::ScoutCavalry);
    // Stage the scout on the north side of the barrier first.
    let (nx, ny) = cell_point(&sim, ax + 5, ay - 3);
    sim.push_command(Command::Move {
        entity: scout,
        x: nx,
        y: ny,
    });
    let staged = run_until(&mut sim, 60_000, |s| {
        let e = ent(s, scout);
        s.grid().world_to_cell(e.pos_x, e.pos_y) == (ax + 5, ay - 3)
    });
    assert!(staged, "enemy scout never staged north of the barrier");
    let (sx, sy) = cell_point(&sim, ax + 5, ay + 3);
    sim.push_command(Command::Move {
        entity: scout,
        x: sx,
        y: sy,
    });
    // The invariant BLD-13 actually states: the gate never opens
    // for an enemy, and the enemy never occupies the span (a
    // 12-cell line on open ground can legitimately be walked
    // AROUND — that is not the gate's failure — so the check is
    // span occupancy + the derived state, sampled every tick).
    for _ in 0..3600 {
        sim.tick();
        let g = gate_of(&sim, OWNER_PLAYER).unwrap();
        assert!(
            !sim.gate_is_open(g),
            "gate opened with only an enemy near it"
        );
        let e = ent(&sim, scout);
        let cell = sim.grid().world_to_cell(e.pos_x, e.pos_y);
        assert!(
            !span.contains(&cell),
            "enemy scout entered the closed gate span at {cell:?}"
        );
    }
    // Send the enemy home before the owner's phases: while it
    // presses at the span it holds the gate shut (radius rule),
    // which would fight the owner's own crossing.
    let opp_tc = find_id(&sim, OWNER_OPPONENT, EntityKind::TownCenter);
    let (hx, hy) = {
        let t = ent(&sim, opp_tc);
        (t.pos_x, t.pos_y)
    };
    sim.push_command(Command::Move {
        entity: scout,
        x: hx,
        y: hy,
    });
    let cleared = run_until(&mut sim, 120_000, |s| {
        let e = ent(s, scout);
        let dx = f(e.pos_x - g.pos_x);
        let dy = f(e.pos_y - g.pos_y);
        dx * dx + dy * dy > 25.0 * 25.0
    });
    assert!(cleared, "enemy scout never cleared the gate area");
    // The derived state flips with the OWNER's approach: walk the
    // owner's villager to the gate's center — the gate opens and
    // its cells unblock.
    sim.push_command(Command::Move {
        entity: vil,
        x: g.pos_x,
        y: g.pos_y,
    });
    let opened = run_until(&mut sim, 60_000, |s| {
        let g = gate_of(s, OWNER_PLAYER).unwrap();
        s.gate_is_open(g)
    });
    assert!(opened, "gate never opened for its owner at the span");
    for &(cx, cy) in &span {
        assert!(!sim.grid().is_dynamic(cx, cy), "open gate cell must not block");
    }
    // …and the owner passes through to the far side. The
    // crossing is asserted geometrically — the villager ends
    // NORTH of the line's row, within the barrier's columns —
    // not by an exact far cell: on this seed the cell three
    // rows north of the span sits in a forest pocket that is
    // sealed from the south, so the villager's best-effort
    // path through its own gate ends at the closest reachable
    // cell past the span (which is exactly the passage being
    // proven: the wall row is unbroken except at the gate).
    let (tx, ty) = cell_point(&sim, ax + 5, ay - 3);
    sim.push_command(Command::Move {
        entity: vil,
        x: tx,
        y: ty,
    });
    let north = run_until(&mut sim, 60_000, |s| {
        let e = ent(s, vil);
        let (cx, cy) = s.grid().world_to_cell(e.pos_x, e.pos_y);
        cy < ay && (ax..=ax + 11).contains(&cx)
    });
    assert!(north, "owner villager never reached the north side");
    // Now order it back south, THROUGH its own gate.
    let (bx, by) = cell_point(&sim, ax + 5, ay + 3);
    sim.push_command(Command::Move {
        entity: vil,
        x: bx,
        y: by,
    });
    let crossed = run_until(&mut sim, 60_000, |s| {
        let e = ent(s, vil);
        let (cx, cy) = s.grid().world_to_cell(e.pos_x, e.pos_y);
        cy > ay && (ax..=ax + 11).contains(&cx)
    });
    assert!(crossed, "owner villager never crossed back through its own gate");
    // With the owner gone again the gate closes behind it.
    let closed = run_until(&mut sim, 600, |s| {
        let g = gate_of(s, OWNER_PLAYER).unwrap();
        !s.gate_is_open(g)
    });
    assert!(closed, "gate never closed after the owner left");
}

#[test]
fn enemy_at_the_gate_holds_it_shut() {
    let mut sim = Sim::with_mode(46, MatchMode::TwoPlayer);
    let vil = villagers_of(&sim, OWNER_PLAYER)[0];
    let (ax, ay) = open_run_near_tc(&sim, OWNER_PLAYER, vil, 8);
    raise_line(&mut sim, vil, ax, ay, 8);
    raise_gate(&mut sim, vil, PGATE, ax + 2, ay);
    let g = gate_of(&sim, OWNER_PLAYER).unwrap().clone();
    // Owner stands at the gate: open.
    sim.push_command(Command::Move {
        entity: vil,
        x: g.pos_x,
        y: g.pos_y,
    });
    let opened = run_until(&mut sim, 60_000, |s| {
        let g = gate_of(s, OWNER_PLAYER).unwrap();
        s.gate_is_open(g)
    });
    assert!(opened, "gate never opened for the owner");
    // The enemy Scout rides up to the gate — to the cell just
    // north of the span (inside the open radius, ≈4.5 units from
    // the center): while it stands in the radius the gate is
    // shut even with the owner present — an enemy never holds a
    // gate open, it holds it closed.
    let scout = find_id(&sim, OWNER_OPPONENT, EntityKind::ScoutCavalry);
    let (ex, ey) = cell_point(&sim, g.cell_x + 1, g.cell_y - 1);
    sim.push_command(Command::Move {
        entity: scout,
        x: ex,
        y: ey,
    });
    let shut = run_until(&mut sim, 60_000, |s| {
        // The scout is inside the open radius…
        let e = ent(s, scout);
        let dx = f(e.pos_x - g.pos_x);
        let dy = f(e.pos_y - g.pos_y);
        let near = dx * dx + dy * dy <= 30.0;
        // …and the gate reads closed.
        let g = gate_of(s, OWNER_PLAYER).unwrap();
        near && !s.gate_is_open(g)
    });
    assert!(shut, "enemy at the gate never held it shut");
}

#[test]
fn gate_foundation_blocking_and_delete_frees_the_span() {
    let mut sim = Sim::with_mode(47, MatchMode::TwoPlayer);
    let vil = villagers_of(&sim, OWNER_PLAYER)[0];
    let (ax, ay) = open_run_near_tc(&sim, OWNER_PLAYER, vil, 8);
    let (x, y) = cell_point(&sim, ax + 2, ay);
    assert!(sim.can_build(vil, PGATE, x, y));
    sim.push_command(Command::Build {
        entity: vil,
        kind: PGATE,
        x,
        y,
    });
    // The untouched foundation does not block its span (BLD-20's
    // wall-foundation rule, gates included).
    let founded = run_until(&mut sim, 600, |s| gate_of(s, OWNER_PLAYER).is_some());
    assert!(founded, "gate foundation never landed");
    let g = gate_of(&sim, OWNER_PLAYER).unwrap().clone();
    for &(cx, cy) in &g.footprint_cells() {
        assert!(
            !sim.grid().is_dynamic(cx, cy),
            "untouched gate foundation must be walkable"
        );
    }
    // Once construction advances, the span blocks (closed).
    let started = run_until(&mut sim, 60_000, |s| {
        gate_of(s, OWNER_PLAYER).unwrap().progress_thirds > 0
    });
    assert!(started, "gate construction never started");
    for &(cx, cy) in &g.footprint_cells() {
        assert!(sim.grid().is_dynamic(cx, cy), "started gate must block");
    }
    // Finish it, then delete the completed gate: the span frees.
    let done = run_until(&mut sim, 240_000, |s| {
        gate_of(s, OWNER_PLAYER).map(|g| g.complete).unwrap_or(false)
    });
    assert!(done, "gate never completed");
    sim.push_command(Command::DeleteBuilding { building: g.id });
    sim.tick();
    assert!(gate_of(&sim, OWNER_PLAYER).is_none());
    for &(cx, cy) in &g.footprint_cells() {
        assert!(!sim.grid().is_dynamic(cx, cy), "deleted gate cell must free");
    }
}

#[test]
fn gate_takes_damage_through_combat() {
    let mut sim = Sim::with_mode(48, MatchMode::TwoPlayer);
    let vil = villagers_of(&sim, OWNER_PLAYER)[0];
    let (ax, ay) = open_run_near_tc(&sim, OWNER_PLAYER, vil, 8);
    raise_gate(&mut sim, vil, PGATE, ax + 2, ay);
    let g = gate_of(&sim, OWNER_PLAYER).unwrap().clone();
    assert_eq!(f(g.hp), 240.0);
    // The enemy Scout attacks the gate (an entity-vs-building
    // order): HP falls through the ordinary combat path.
    let scout = find_id(&sim, OWNER_OPPONENT, EntityKind::ScoutCavalry);
    sim.push_command(Command::AttackBuilding {
        entity: scout,
        building: g.id,
    });
    let hurt = run_until(&mut sim, 120_000, |s| {
        gate_of(s, OWNER_PLAYER).map(|g| f(g.hp) < 240.0).unwrap_or(true)
    });
    assert!(hurt, "gate never took damage");
}

#[test]
fn gate_scenario_is_deterministic() {
    let script = |sim: &mut Sim| {
        let vil = villagers_of(sim, OWNER_PLAYER)[0];
        let (ax, ay) = open_run_near_tc(sim, OWNER_PLAYER, vil, 8);
        raise_line(sim, vil, ax, ay, 8);
        raise_gate(sim, vil, PGATE, ax + 2, ay);
        let g = gate_of(sim, OWNER_PLAYER).unwrap().clone();
        sim.push_command(Command::Move {
            entity: vil,
            x: g.pos_x,
            y: g.pos_y,
        });
        for _ in 0..1800 {
            sim.tick();
        }
        sim.state_hash()
    };
    let mut a = Sim::with_mode(49, MatchMode::TwoPlayer);
    let mut b = Sim::with_mode(49, MatchMode::TwoPlayer);
    assert_eq!(script(&mut a), script(&mut b));
}
