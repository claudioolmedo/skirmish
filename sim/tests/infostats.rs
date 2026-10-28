//! INF-05 display-stats tests (bot-loop, 2026-10-28): the
//! selection panel's attack/armor numbers must be the sim's own
//! effective rows — the same rows combat reads — so the getters
//! are pinned against the documented base values here, and the
//! House row proves the building form reads the kind's armor at
//! the owner's Age (Dark: House 1 melee / 8 pierce is NOT assumed;
//! the values asserted are the build.rs rows for Dark Age).

use skirmish_sim::{BuildingKind, Command, EntityKind, MatchMode, Sim, OWNER_PLAYER};

fn run_until(sim: &mut Sim, cap: u64, mut done: impl FnMut(&Sim) -> bool) -> bool {
    for _ in 0..cap {
        if done(sim) {
            return true;
        }
        sim.tick();
    }
    done(sim)
}

fn entity(sim: &Sim, owner: u8, kind: EntityKind) -> u32 {
    sim.entities()
        .iter()
        .find(|e| e.owner == owner && e.kind == kind && !e.dead)
        .unwrap()
        .id
}

#[test]
fn entity_display_stats_are_the_effective_rows() {
    let sim = Sim::with_mode(7, MatchMode::TwoPlayer);
    // Villager: documented attack 3, armor 0/0.
    let vil = entity(&sim, OWNER_PLAYER, EntityKind::Villager);
    assert_eq!(sim.entity_display_stats(vil), Some((3, 0, 0)));
    // Town Center (an entity): unarmed in our roster, armor 3/5
    // in the Dark Age (BLD-11's per-Age TC row).
    let tc = entity(&sim, OWNER_PLAYER, EntityKind::TownCenter);
    assert_eq!(sim.entity_display_stats(tc), Some((-1, 3, 5)));
    // Scout Cavalry, Dark Age row: attack 3, armor 0 melee / 2 pierce.
    let scout = entity(&sim, OWNER_PLAYER, EntityKind::ScoutCavalry);
    assert_eq!(sim.entity_display_stats(scout), Some((3, 0, 2)));
    // Unknown ids have no stats.
    assert_eq!(sim.entity_display_stats(999_999), None);
    assert_eq!(sim.building_display_stats(999_999), None);
}

#[test]
fn building_display_stats_read_the_kind_rows() {
    let mut sim = Sim::with_mode(7, MatchMode::TwoPlayer);
    let vil = entity(&sim, OWNER_PLAYER, EntityKind::Villager);
    let tc = sim.town_center();
    let (tcx, tcy) = sim.grid().world_to_cell(tc.pos_x, tc.pos_y);
    // Find a buildable House anchor near the TC, as towers.rs does.
    let mut anchor = None;
    'search: for r in 1..=14i32 {
        for dy in -r..=r {
            for dx in -r..=r {
                if dx.abs() != r && dy.abs() != r {
                    continue;
                }
                let (ax, ay) = (tcx as i32 + dx, tcy as i32 + dy);
                if ax < 0 || ay < 0 {
                    continue;
                }
                let (x, y) = sim.grid().cell_center(ax as u32, ay as u32);
                if sim.can_build(vil, BuildingKind::House, x, y) {
                    anchor = Some((x, y));
                    break 'search;
                }
            }
        }
    }
    let (x, y) = anchor.expect("a House anchor near the TC");
    sim.push_command(Command::Build { entity: vil, kind: BuildingKind::House, x, y });
    let done = run_until(&mut sim, 120_000, |s| {
        s.buildings().iter().any(|b| b.kind == BuildingKind::House && b.complete)
    });
    assert!(done, "the House never completed");
    let house = sim
        .buildings()
        .iter()
        .find(|b| b.kind == BuildingKind::House)
        .unwrap();
    // House is unarmed; its Dark-Age armor row is build.rs's table.
    let expect = (
        -1,
        BuildingKind::House.armor_melee_at(sim.age_of(OWNER_PLAYER)),
        BuildingKind::House.armor_pierce_at(sim.age_of(OWNER_PLAYER)),
    );
    assert_eq!(sim.building_display_stats(house.id), Some(expect));
}
