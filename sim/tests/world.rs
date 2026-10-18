//! World tests (M7): the seeded Arabia generator, start fairness,
//! connectivity validation, fog of war semantics, terrain and
//! water — the world the older suites used to take as a fixed
//! arena. Everything here drives the public Sim API only.

use fixed::types::I16F16;
use skirmish_sim::{
    BuildingKind, Command, EntityKind, MatchMode, NodeKind, Sim, GRID_DIM, OWNER_OPPONENT,
    OWNER_PLAYER,
};

fn cell_of(sim: &Sim, x: I16F16, y: I16F16) -> (u32, u32) {
    sim.grid().world_to_cell(x, y)
}

fn tc_of(sim: &Sim, owner: u8) -> (I16F16, I16F16) {
    let tc = sim
        .entities()
        .iter()
        .find(|e| e.kind == EntityKind::TownCenter && e.owner == owner)
        .expect("a Town Center");
    (tc.pos_x, tc.pos_y)
}

fn rot(cell: (u32, u32)) -> (u32, u32) {
    (GRID_DIM - 1 - cell.0, GRID_DIM - 1 - cell.1)
}

/// Squared cell distance between two cells (exact integer math).
fn d2(a: (u32, u32), b: (u32, u32)) -> i64 {
    let dx = a.0 as i64 - b.0 as i64;
    let dy = a.1 as i64 - b.1 as i64;
    dx * dx + dy * dy
}

#[test]
fn generated_maps_are_deterministic_and_seed_varied() {
    // Same seed ⇒ identical layout and identical hash stream.
    for seed in [1u64, 7, 42] {
        let mut a = Sim::new_match(seed, MatchMode::AiNormal);
        let mut b = Sim::new_match(seed, MatchMode::AiNormal);
        assert_eq!(a.state_hash(), b.state_hash(), "seed {seed} tick 0");
        assert_eq!(
            a.grid().terrain_bytes(),
            b.grid().terrain_bytes(),
            "seed {seed} terrain"
        );
        assert_eq!(
            a.grid().height_bytes(),
            b.grid().height_bytes(),
            "seed {seed} heights"
        );
        assert_eq!(a.nodes().len(), b.nodes().len());
        for _ in 0..300 {
            assert_eq!(a.tick(), b.tick(), "seed {seed} replay diverged");
        }
    }
    // Different seeds ⇒ different worlds (terrain or heights or
    // layout — at least the state hash at tick 0 must differ).
    let hashes: Vec<u64> = [1u64, 2, 3, 4, 5]
        .iter()
        .map(|&s| Sim::new_match(s, MatchMode::AiNormal).state_hash())
        .collect();
    let mut sorted = hashes.clone();
    sorted.sort_unstable();
    sorted.dedup();
    assert_eq!(sorted.len(), hashes.len(), "seeds produced identical maps");
}

#[test]
fn starts_are_mirrored_fair_and_complete() {
    for seed in [1u64, 2, 3, 7, 42, 99, 1234] {
        let sim = Sim::new_match(seed, MatchMode::AiNormal);
        let (hx, hy) = tc_of(&sim, OWNER_PLAYER);
        let (ox, oy) = tc_of(&sim, OWNER_OPPONENT);
        let htc = cell_of(&sim, hx, hy);
        let otc = cell_of(&sim, ox, oy);
        // Exact rotational symmetry: the opponent's start is the
        // human's mirrored through the map center.
        assert_eq!(otc, rot(htc), "seed {seed}: TC cells not mirrored");

        // Node fairness: group by nearer Town Center; per kind the
        // counts match exactly, and every start-A node has a
        // same-kind mirror node in start B's set.
        let mut count = |owner_cell: (u32, u32), kind: NodeKind| {
            sim.nodes()
                .iter()
                .filter(|n| {
                    let c = cell_of(&sim, n.pos_x, n.pos_y);
                    let mine = d2(c, owner_cell);
                    let other = d2(c, if owner_cell == htc { otc } else { htc });
                    mine < other && n.kind == kind
                })
                .count()
        };
        for kind in [
            NodeKind::Tree,
            NodeKind::BerryBush,
            NodeKind::GoldMine,
            NodeKind::StoneMine,
        ] {
            assert_eq!(
                count(htc, kind),
                count(otc, kind),
                "seed {seed}: unfair {kind:?} counts"
            );
        }
        // The OPENING-SPEC §2 package minimums, per start.
        assert!(count(htc, NodeKind::BerryBush) >= 6, "seed {seed} berries");
        assert!(count(htc, NodeKind::GoldMine) >= 11, "seed {seed} gold");
        assert!(count(htc, NodeKind::StoneMine) >= 9, "seed {seed} stone");
        assert!(count(htc, NodeKind::Tree) >= 30, "seed {seed} trees");

        // Animal fairness: same multiset per kind per start, and
        // mirrored cells.
        for kind in [EntityKind::Sheep, EntityKind::Boar, EntityKind::Deer] {
            let cells: Vec<(u32, u32)> = sim
                .entities()
                .iter()
                .filter(|e| e.kind == kind)
                .map(|e| cell_of(&sim, e.pos_x, e.pos_y))
                .collect();
            assert_eq!(cells.len() % 2, 0, "seed {seed}: odd {kind:?} count");
            for c in &cells {
                assert!(
                    cells.contains(&rot(*c)),
                    "seed {seed}: {kind:?} at {c:?} has no mirror"
                );
            }
        }
        // Package sizes: 8 sheep, 2 boars, 6 deer per start.
        let n = |kind: EntityKind| {
            sim.entities().iter().filter(|e| e.kind == kind).count()
        };
        assert_eq!(n(EntityKind::Sheep), 16, "seed {seed}");
        assert_eq!(n(EntityKind::Boar), 4, "seed {seed}");
        assert_eq!(n(EntityKind::Deer), 12, "seed {seed}");
    }
}

#[test]
fn starts_connect_and_every_resource_is_reachable() {
    for seed in [1u64, 2, 3, 7, 42, 99] {
        let sim = Sim::new_match(seed, MatchMode::AiNormal);
        let (hx, hy) = tc_of(&sim, OWNER_PLAYER);
        let (ox, oy) = tc_of(&sim, OWNER_OPPONENT);
        // A land path exists between the starts (MAP-09).
        assert!(
            sim.grid().find_path((hx, hy), (ox, oy)).is_some(),
            "seed {seed}: no path between the Town Centers"
        );
        // Every node of the human's package is workable from the
        // Town Center. Non-tree nodes stand on open cells: a real
        // A* path to the cell (or a standing cell beside it) must
        // exist. A tree is its own cell's block (MAP-04) and a
        // forest's interior trees have no standing cell beside them
        // at all — they are workable the AoE2 way, by chopping in
        // from the edge — so for trees the criterion is the
        // generator's own: the cell stands on the shared landmass
        // (a flood fill over terrain, where trees do not block).
        let htc = cell_of(&sim, hx, hy);
        let otc = cell_of(&sim, ox, oy);
        let mut flood = vec![false; (GRID_DIM * GRID_DIM) as usize];
        {
            let mut stack = vec![htc];
            flood[(htc.1 * GRID_DIM + htc.0) as usize] = true;
            while let Some((cx, cy)) = stack.pop() {
                for (dx, dy) in [(1i32, 0i32), (-1, 0), (0, 1), (0, -1)] {
                    let (nx, ny) = (cx as i32 + dx, cy as i32 + dy);
                    if nx < 0 || ny < 0 || nx >= GRID_DIM as i32 || ny >= GRID_DIM as i32 {
                        continue;
                    }
                    let (nx, ny) = (nx as u32, ny as u32);
                    let i = (ny * GRID_DIM + nx) as usize;
                    if !flood[i] && !sim.grid().is_water(nx, ny) {
                        flood[i] = true;
                        stack.push((nx, ny));
                    }
                }
            }
        }
        for n in sim.nodes() {
            let c = cell_of(&sim, n.pos_x, n.pos_y);
            if d2(c, htc) >= d2(c, otc) {
                continue; // the opponent's package — symmetric by construction
            }
            assert!(
                flood[(c.1 * GRID_DIM + c.0) as usize],
                "seed {seed}: node {} ({:?}) off the shared landmass",
                n.id,
                n.kind
            );
            if n.kind == NodeKind::Tree {
                continue;
            }
            let direct = sim.grid().find_path((hx, hy), (n.pos_x, n.pos_y));
            let beside = [-1i32, 0, 1].iter().any(|&dx| {
                [-1i32, 0, 1].iter().any(|&dy| {
                    let (nx, ny) = (c.0 as i32 + dx, c.1 as i32 + dy);
                    if nx < 0 || ny < 0 || nx >= GRID_DIM as i32 || ny >= GRID_DIM as i32 {
                        return false;
                    }
                    let center = sim.grid().cell_center(nx as u32, ny as u32);
                    sim.grid().find_path((hx, hy), center).is_some()
                })
            });
            assert!(
                direct.is_some() || beside,
                "seed {seed}: node {} ({:?}) unreachable from the Town Center",
                n.id,
                n.kind
            );
        }
        // Neither start stands in water or on a blocked cell.
        assert!(!sim.grid().is_blocked(htc.0, htc.1), "seed {seed}");
        assert!(!sim.grid().is_blocked(otc.0, otc.1), "seed {seed}");
    }
}

#[test]
fn starts_are_flat_and_hills_exist() {
    // MAP-05 + the generator's fairness rule: the ground a start
    // plays its opening on is one level (the TC's), while the map
    // as a whole has real elevation variety.
    for seed in [1u64, 2, 3, 7, 42] {
        let sim = Sim::new_match(seed, MatchMode::AiNormal);
        let levels: std::collections::HashSet<u8> =
            sim.grid().height_bytes().iter().copied().collect();
        assert!(levels.len() >= 3, "seed {seed}: hills too flat: {levels:?}");
        for owner in [OWNER_PLAYER, OWNER_OPPONENT] {
            let (tx, ty) = tc_of(&sim, owner);
            let tc_cell = cell_of(&sim, tx, ty);
            let level = sim.grid().height_at(tc_cell.0, tc_cell.1);
            for dy in -3i32..=3 {
                for dx in -3i32..=3 {
                    let (cx, cy) = (tc_cell.0 as i32 + dx, tc_cell.1 as i32 + dy);
                    if cx < 0 || cy < 0 || cx >= GRID_DIM as i32 || cy >= GRID_DIM as i32 {
                        continue;
                    }
                    assert_eq!(
                        sim.grid().height_at(cx as u32, cy as u32),
                        level,
                        "seed {seed}: start not flat at ({cx}, {cy})"
                    );
                }
            }
        }
    }
}

#[test]
fn fog_starts_blind_and_transitions_with_the_scout() {
    let mut sim = Sim::new_match(7, MatchMode::AiNormal);
    let (hx, hy) = tc_of(&sim, OWNER_PLAYER);
    let (ox, oy) = tc_of(&sim, OWNER_OPPONENT);
    let htc = cell_of(&sim, hx, hy);
    let otc = cell_of(&sim, ox, oy);
    // VIS-07: own surroundings visible; the enemy start unknown.
    assert_eq!(sim.cell_visibility(OWNER_PLAYER, htc.0, htc.1), 2);
    assert_eq!(sim.cell_visibility(OWNER_PLAYER, otc.0, otc.1), 0);
    assert_eq!(sim.cell_visibility(OWNER_OPPONENT, htc.0, htc.1), 0);
    // The map's far corner is unexplored for both players.
    assert_eq!(sim.cell_visibility(OWNER_PLAYER, 0, GRID_DIM - 1), 0);

    // Ride the Scout to the enemy Town Center: its ground becomes
    // explored, then visible while the Scout stands there.
    let scout = sim
        .entities()
        .iter()
        .find(|e| e.owner == OWNER_PLAYER && e.kind == EntityKind::ScoutCavalry)
        .map(|e| e.id)
        .expect("a Scout");
    sim.push_command(Command::Move {
        entity: scout,
        x: ox,
        y: oy,
    });
    let mut seen_visible = false;
    for _ in 0..12_000 {
        sim.tick();
        if sim.cell_visibility(OWNER_PLAYER, otc.0, otc.1) == 2 {
            seen_visible = true;
            break;
        }
    }
    assert!(seen_visible, "the Scout never revealed the enemy start");
    assert!(sim.cell_explored_by(OWNER_PLAYER, otc.0, otc.1));

    // Ride home: the enemy start drops to explored memory (1) —
    // remembered, no longer visible (VIS-01/06).
    sim.push_command(Command::Move {
        entity: scout,
        x: hx,
        y: hy,
    });
    let mut demoted = false;
    for _ in 0..12_000 {
        sim.tick();
        if sim.cell_visibility(OWNER_PLAYER, otc.0, otc.1) == 1 {
            demoted = true;
            break;
        }
    }
    assert!(demoted, "the enemy start never dropped to explored memory");
}

#[test]
fn fog_never_exceeds_the_los_cap() {
    // VIS-04: no visible cell may sit farther than the 20-tile cap
    // (in cell distance from the nearest sight source's cell).
    // Checked on the construction-time grids, where the sight
    // sources stand exactly where the stamps were made.
    let sim = Sim::new_match(3, MatchMode::AiNormal);
    for owner in [OWNER_PLAYER, OWNER_OPPONENT] {
        let mut sources: Vec<(u32, u32)> = sim
            .entities()
            .iter()
            .filter(|e| e.owner == owner && !e.dead)
            .map(|e| cell_of(&sim, e.pos_x, e.pos_y))
            .collect();
        sources.extend(
            sim.buildings()
                .iter()
                .filter(|b| b.owner == owner)
                .map(|b| cell_of(&sim, b.pos_x, b.pos_y)),
        );
        let fog = sim.visibility(owner);
        for gy in 0..GRID_DIM {
            for gx in 0..GRID_DIM {
                if fog[(gy * GRID_DIM + gx) as usize] != 2 {
                    continue;
                }
                let nearest = sources
                    .iter()
                    .map(|&s| d2((gx, gy), s))
                    .min()
                    .unwrap_or(i64::MAX);
                assert!(
                    nearest <= 20 * 20,
                    "owner {owner}: cell ({gx}, {gy}) visible beyond the LoS cap"
                );
            }
        }
    }
}

#[test]
fn enemy_units_are_known_only_while_visible() {
    // The client rule (VIS-01/06): enemy/Gaia units draw iff their
    // cell is currently visible (fog == 2). At match start the
    // whole enemy start is unexplored — every enemy unit hidden.
    let sim = Sim::new_match(11, MatchMode::AiNormal);
    for e in sim.entities() {
        if e.owner != OWNER_OPPONENT {
            continue;
        }
        let c = cell_of(&sim, e.pos_x, e.pos_y);
        assert_ne!(
            sim.cell_visibility(OWNER_PLAYER, c.0, c.1),
            2,
            "enemy {:?} visible at match start",
            e.kind
        );
    }
    // The AI's own knowledge is symmetric: it cannot see us either.
    for e in sim.entities() {
        if e.owner != OWNER_PLAYER || e.kind == EntityKind::TownCenter {
            continue;
        }
        let c = cell_of(&sim, e.pos_x, e.pos_y);
        assert_ne!(sim.cell_visibility(OWNER_OPPONENT, c.0, c.1), 2);
    }
}

#[test]
fn water_blocks_land_pathing_and_placement() {
    // Find a seed whose Arabia carries a pond (the generator uses
    // water sparingly — most seeds in this range have one).
    let mut found = None;
    for seed in 1..=60u64 {
        let sim = Sim::new(seed);
        if sim.grid().terrain_bytes().iter().any(|&t| t == 1) {
            found = Some((seed, sim));
            break;
        }
    }
    let (_seed, sim) = found.expect("no pond in 60 seeds — generator lost its water");
    let grid = sim.grid();
    // A water cell is blocked and cannot be pathed onto.
    let water = (0..GRID_DIM)
        .flat_map(|y| (0..GRID_DIM).map(move |x| (x, y)))
        .find(|&(x, y)| grid.is_water(x, y))
        .expect("a water cell");
    assert!(grid.is_blocked(water.0, water.1));
    let center = grid.cell_center(water.0, water.1);
    let (hx, hy) = tc_of(&sim, OWNER_PLAYER);
    assert!(
        grid.find_path((hx, hy), center).is_none(),
        "a land path entered water"
    );
    // And a House cannot be placed on it (the sim's own
    // validation, via the public probe).
    assert!(
        !sim.can_build(1, BuildingKind::House, center.0, center.1),
        "a House was placeable on water"
    );
}

#[test]
fn ai_match_on_a_generated_map_replays_bit_identically() {
    // A match on a generated map (fog, hills and all) is still just
    // (seed, mode, commands): two runs agree on every tick's hash.
    let run = |seed: u64| -> Vec<u64> {
        let mut sim = Sim::new_match(seed, MatchMode::AiNormal);
        // A fixed human script: three villagers to the nearest tree.
        let (hx, hy) = tc_of(&sim, OWNER_PLAYER);
        // (f64 key: fixed-point squares overflow I16F16 at map
        // scale — the sim itself compares lengths via `vec_len`.)
        let tree = sim
            .nodes()
            .iter()
            .filter(|n| n.kind == NodeKind::Tree)
            .min_by_key(|n| {
                let dx = n.pos_x.to_num::<f64>() - hx.to_num::<f64>();
                let dy = n.pos_y.to_num::<f64>() - hy.to_num::<f64>();
                (dx * dx + dy * dy).to_bits()
            })
            .map(|n| n.id)
            .expect("a tree");
        for v in 1..=3u32 {
            sim.push_command(Command::Gather {
                entity: v,
                node: tree,
            });
        }
        (0..5_400).map(|_| sim.tick()).collect()
    };
    assert_eq!(run(42), run(42));
    assert_ne!(run(42), run(43));
}
