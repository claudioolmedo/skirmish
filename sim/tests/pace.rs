//! Pace regression suite (post-M7 pace correction).
//!
//! Until the correction, the unit table preserved only the study's
//! speed RATIOS — the villager row kept M1/M2's 1.0 units/tick and
//! every other row was scaled × 1.25 off it. At M7's tile scale
//! (1 tile = 1 cell = 4 world units) that made absolute travel
//! ~12× too fast: a villager crossed ~9.8 tiles/s against the
//! reference 0.8. The table now carries absolute tiles/s values
//! (units.rs header has the formula); these tests pin the pace so
//! it cannot drift back.
//!
//! Measurement note: cruise is read as the best SUSTAINED window
//! (the minimum step inside the best 45-tick window of a long leg),
//! not a median. Separation can only subtract from a unit's table
//! speed — a traveler bulldozing a standing flock-mate or sheep
//! settles at the displacement equilibrium (half speed while the
//! push lasts, the M2 separation model) — and nothing in the sim
//! can push a unit past its table speed, so the fastest sustained
//! pace on an unimpeded stretch IS the table pace. A global pace
//! error (the regression this suite guards) moves that number by
//! the full error factor.

use fixed::types::I16F16;
use skirmish_sim::{BuildingKind, Command, EntityKind, Sim};

fn f(v: I16F16) -> f64 {
    v.to_num::<f64>()
}

fn ent(sim: &Sim, id: u32) -> &skirmish_sim::Entity {
    sim.entities().iter().find(|e| e.id == id).unwrap()
}

fn kind_id(sim: &Sim, kind: EntityKind) -> u32 {
    sim.entities()
        .iter()
        .find(|e| e.kind == kind)
        .unwrap_or_else(|| panic!("no {kind:?} in the start"))
        .id
}

/// Order `entity` on a long leg toward the map center and return
/// its sustained cruise speed in tiles/second (see the module
/// docs for why the best-window minimum is the right estimator).
fn cruise_tiles_per_sec(sim: &mut Sim, entity: u32, leg_units: f64, cap: u64) -> f64 {
    let e = ent(sim, entity);
    let (sx, sy) = (f(e.pos_x), f(e.pos_y));
    let tx = if sx <= 0.0 { sx + leg_units } else { sx - leg_units };
    sim.push_command(Command::Move {
        entity,
        x: I16F16::from_num(tx),
        y: I16F16::from_num(sy),
    });
    let mut steps = Vec::new();
    let mut prev = (sx, sy);
    for _ in 0..cap {
        sim.tick();
        let e = ent(sim, entity);
        let (x, y) = (f(e.pos_x), f(e.pos_y));
        steps.push(((x - prev.0).powi(2) + (y - prev.1).powi(2)).sqrt());
        prev = (x, y);
        if e.order.is_none() {
            break;
        }
    }
    assert!(
        ent(sim, entity).order.is_none(),
        "entity {entity} never arrived on its pace leg"
    );
    const WINDOW: usize = 45;
    assert!(
        steps.len() > WINDOW * 4,
        "pace leg too short for a sustained window: {} steps",
        steps.len()
    );
    let sustained = steps
        .windows(WINDOW)
        .map(|w| w.iter().copied().fold(f64::INFINITY, f64::min))
        .fold(0.0f64, f64::max);
    sustained * 60.0 / 4.0 // world units/tick -> tiles/second
}

fn assert_pace(sim: &mut Sim, entity: u32, want_tiles_per_sec: f64, cap: u64) {
    let got = cruise_tiles_per_sec(sim, entity, 120.0, cap);
    let err = (got - want_tiles_per_sec).abs() / want_tiles_per_sec;
    assert!(
        err < 0.015,
        "entity {entity} cruises at {got:.4} tiles/s, want {want_tiles_per_sec} (±1.5%)"
    );
}

#[test]
fn villager_cruise_is_point_eight_tiles_per_sec() {
    let mut sim = Sim::new(42);
    assert_pace(&mut sim, 1, 0.80, 20_000);
}

#[test]
fn scout_cruise_is_one_point_two_tiles_per_sec() {
    let mut sim = Sim::new(42);
    let scout = kind_id(&sim, EntityKind::ScoutCavalry);
    assert_pace(&mut sim, scout, 1.20, 20_000);
}

#[test]
fn militia_cruise_is_point_nine_tiles_per_sec() {
    let mut sim = Sim::new(42);
    // Produce a real Militia the way the game does: Barracks
    // (175 W — the start stockpile covers it) near the Town Center,
    // then TrainAt (60 F / 20 G, also covered).
    let tc = sim.town_center();
    let tc_cell = sim.grid().world_to_cell(tc.pos_x, tc.pos_y);
    let anchor = anchor_near(&sim, 1, BuildingKind::Barracks, tc_cell);
    let (x, y) = sim.grid().cell_center(anchor.0, anchor.1);
    sim.push_command(Command::Build {
        entity: 1,
        kind: BuildingKind::Barracks,
        x,
        y,
    });
    let up = run_until(&mut sim, 60_000, |s| {
        s.buildings()
            .iter()
            .any(|b| b.kind == BuildingKind::Barracks && b.complete)
    });
    assert!(up, "barracks never completed");
    let barracks = sim
        .buildings()
        .iter()
        .find(|b| b.kind == BuildingKind::Barracks && b.complete)
        .unwrap()
        .id;
    sim.push_command(Command::TrainAt {
        building: barracks,
        kind: EntityKind::Militia,
    });
    let trained = run_until(&mut sim, 10_000, |s| {
        s.entities().iter().any(|e| e.kind == EntityKind::Militia)
    });
    assert!(trained, "militia never trained");
    let militia = kind_id(&sim, EntityKind::Militia);
    // Lane discipline (see the module docs): the pace leg needs an
    // unimpeded stretch. On seed 42's generated layout the near
    // flock stands exactly on the Barracks-exit lane east, and a
    // traveler herding a sheep sits at the half-speed displacement
    // equilibrium for as long as the shove lasts (measured: the
    // whole 120-unit leg at 0.45) — that measures the flock, not
    // the table. Step the Militia one lane north first, off the
    // flock line; the leg itself is unchanged. (Lane moved by the
    // 2026-10-20 forest contiguity pass, which regenerated the
    // layout; on the clear lane the same estimator reads 0.9000.)
    let m = ent(&sim, militia);
    let (mx, my) = (m.pos_x, m.pos_y);
    sim.push_command(Command::Move {
        entity: militia,
        x: mx,
        y: my - I16F16::from_num(16),
    });
    sim.tick();
    let stepped = run_until(&mut sim, 10_000, |s| {
        s.entities()
            .iter()
            .find(|e| e.id == militia)
            .unwrap()
            .order
            .is_none()
    });
    assert!(stepped, "militia never reached its pace lane");
    assert_pace(&mut sim, militia, 0.90, 20_000);
}

#[test]
fn villager_ten_tile_leg_takes_about_twelve_and_a_half_seconds() {
    let mut sim = Sim::new(42);
    let e = ent(&sim, 1);
    let (sx, sy) = (f(e.pos_x), f(e.pos_y));
    // The lane: due south on this seed — the one direction out of
    // the start with no flock-mate or sheep standing in it (the
    // flock is east, the other villagers west/north), so the leg
    // measures travel time, not bulldozing (see module docs).
    sim.push_command(Command::Move {
        entity: 1,
        x: I16F16::from_num(sx),
        y: I16F16::from_num(sy - 40.0),
    });
    let mut ticks = 0u64;
    loop {
        sim.tick();
        ticks += 1;
        if ent(&sim, 1).order.is_none() || ticks >= 3_600 {
            break;
        }
    }
    assert!(ent(&sim, 1).order.is_none(), "villager never arrived");
    // 10 tiles at 0.8 tiles/s = 12.5 s = 750 ticks at cruise; the
    // arrival slowdown (SLOW_RADIUS) adds its share on top. ±10%.
    assert!(
        (675..=825).contains(&ticks),
        "10-tile leg took {ticks} ticks ({:.2} s), want ~750 (12.5 s ±10%)",
        ticks as f64 / 60.0
    );
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

/// The first anchor cell for `kind` that `entity` may build, in
/// expanding rings around a center cell (the combat suite's helper,
/// same reasoning: on a generated map a test picks ground the way a
/// player does — near the thing the building serves).
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
                let (x, y) = sim.grid().cell_center(ax, ay);
                if sim.can_build(entity, kind, x, y) {
                    return (ax, ay);
                }
            }
        }
    }
    panic!("no buildable anchor near {center:?} for {kind:?}");
}
