//! Castle P5b tests: the conversion core — faith (MNK-02), the
//! CI model for ranged units (MNK-03), resistance (MNK-05),
//! group conversion (MNK-06), sticky CI progress (MNK-07),
//! eligibility (MNK-08), the ownership transfer + frozen patch
//! snapshot (MNK-12), and the MNK-13 acceptance distribution
//! (lone monk vs one Knight).
//!
//! Values: RULEBOOK §12; the P5 execution plan §3.3/§3.4/§7.
//! Fixture style follows `monastery_p5a_tests` (direct records;
//! ages/techs set on the player slots). Where a fixture boosts a
//! monk's HP so a fighting target cannot kill it mid-window,
//! that is scene-setting only — every conversion asserted on
//! runs through the ordinary command path and monk phase.

use crate::entity::{Entity, EntityKind, GarrisonRef, OWNER_OPPONENT, OWNER_PLAYER};
use crate::tech::TechId;
use crate::{Command, Sim};
use fixed::types::I16F16;

fn ent(sim: &Sim, id: u32) -> &Entity {
    sim.entities().iter().find(|e| e.id == id).unwrap()
}

fn spawn(sim: &mut Sim, id: u32, kind: EntityKind, owner: u8, cell: (u32, u32)) {
    let (x, y) = sim.grid().cell_center(cell.0, cell.1);
    sim.entities.push(Entity::spawned(id, kind, owner, x, y));
    sim.entities.sort_by_key(|e| e.id);
}

/// A monk (player) and a target (opponent) two tiles apart —
/// well inside conversion range 9. The monk's HP is boosted so
/// a fighting target cannot end the experiment early.
fn monk_and_target(seed: u64, target: EntityKind) -> (Sim, u32, u32) {
    let mut sim = Sim::new(seed);
    spawn(&mut sim, 9001, EntityKind::Monk, OWNER_PLAYER, (60, 60));
    spawn(&mut sim, 9002, target, OWNER_OPPONENT, (62, 60));
    sim.entities
        .iter_mut()
        .find(|e| e.id == 9001)
        .unwrap()
        .hp = I16F16::from_num(30_000);
    (sim, 9001, 9002)
}

/// Tick until `target` belongs to the player (or the cap hits);
/// returns the tick count at which the flip was first observed.
fn ticks_until_converted(sim: &mut Sim, target: u32, cap: u64) -> Option<u64> {
    for n in 1..=cap {
        sim.tick();
        if ent(sim, target).owner == OWNER_PLAYER {
            return Some(n);
        }
    }
    None
}

// ------------------------------------------------------------------
// MNK-02 — faith gates, drains, regenerates exactly.

#[test]
fn faith_gates_conversion_and_regenerates_in_exactly_62s() {
    let (mut sim, monk, knight) = monk_and_target(7, EntityKind::Knight);
    // Partial faith cannot start a conversion.
    sim.entities.iter_mut().find(|e| e.id == monk).unwrap().faith = 50_000;
    assert!(!sim.can_convert(monk, knight), "50% faith must refuse");
    assert!(sim.can_convert_target(monk, knight), "eligibility is faith-free");
    sim.push_command(Command::Convert { entity: monk, target: knight });
    sim.tick();
    assert_eq!(ent(&sim, monk).conversion_target, None);
    // From zero, full returns at exactly 3 720 ticks (62 s).
    sim.entities.iter_mut().find(|e| e.id == monk).unwrap().faith = 0;
    for _ in 0..3719 {
        sim.tick();
    }
    assert!(ent(&sim, monk).faith < 100_000, "not full one tick early");
    sim.tick();
    assert_eq!(ent(&sim, monk).faith, 100_000, "full at exactly 62 s");
    assert!(sim.can_convert(monk, knight));
}

#[test]
fn success_drains_faith_to_zero() {
    let (mut sim, monk, knight) = monk_and_target(7, EntityKind::Knight);
    sim.push_command(Command::Convert { entity: monk, target: knight });
    let at = ticks_until_converted(&mut sim, knight, 700).expect("knight converts");
    assert!(at <= 675);
    assert_eq!(ent(&sim, monk).faith, 0, "success drains faith to 0");
}

// ------------------------------------------------------------------
// MNK-03 + MNK-13 — the CI window and the acceptance distribution.

#[test]
fn conversion_lands_only_in_ci_5_to_9() {
    for seed in 1..=24u64 {
        let (mut sim, monk, knight) = monk_and_target(seed, EntityKind::Knight);
        sim.push_command(Command::Convert { entity: monk, target: knight });
        let at = ticks_until_converted(&mut sim, knight, 700)
            .unwrap_or_else(|| panic!("seed {seed}: knight never converted"));
        assert!(
            (375..=675).contains(&at),
            "seed {seed}: converted at tick {at}, outside CIs 5–9 (375–675)"
        );
    }
}

/// MNK-13 acceptance: a lone monk vs one Knight reproduces the
/// documented outcome distribution from the CI model — every
/// outcome inside CIs 5–9, the guarantee at CI 9 the ceiling,
/// and rolled successes before the guarantee actually occurring
/// (the window is a distribution, not a fixed timer).
#[test]
fn mnk13_lone_monk_vs_knight_distribution() {
    let mut before_guarantee = 0;
    let mut total_ci = 0u64;
    for seed in 100..160u64 {
        let (mut sim, monk, knight) = monk_and_target(seed, EntityKind::Knight);
        sim.push_command(Command::Convert { entity: monk, target: knight });
        let at = ticks_until_converted(&mut sim, knight, 700)
            .unwrap_or_else(|| panic!("seed {seed}: no conversion"));
        let ci = at.div_ceil(75);
        assert!((5..=9).contains(&ci), "seed {seed}: CI {ci} out of window");
        if ci < 9 {
            before_guarantee += 1;
        }
        total_ci += ci;
    }
    assert!(
        before_guarantee >= 20,
        "only {before_guarantee}/60 converted before the guarantee CI"
    );
    let mean = total_ci as f64 / 60.0;
    assert!((5.0..8.0).contains(&mean), "mean CI {mean} implausible for 38%/CI");
}

#[test]
fn warmup_draws_no_rng_and_guarantee_is_a_counter() {
    // A passive target (Trade Cart: no attack, no orders) keeps
    // every other RNG consumer out of the stream.
    let (mut a, monk_a, cart_a) = monk_and_target(42, EntityKind::TradeCart);
    let (mut b, _monk_b, _cart_b) = monk_and_target(42, EntityKind::TradeCart);
    a.push_command(Command::Convert { entity: monk_a, target: cart_a });
    // Sim B's monk stands idle. Through the 4 warm-up CIs the
    // RNG streams must agree bit-for-bit (zero draws).
    for _ in 0..300 {
        a.tick();
        b.tick();
    }
    assert_eq!(
        a.rng.state_words(),
        b.rng.state_words(),
        "warm-up CIs must draw no RNG"
    );
    // Find a seed whose rolls fail through CI 8, so CI 9 is the
    // guarantee: the guarantee must convert WITHOUT a draw.
    let mut done = false;
    for seed in 200..260u64 {
        let (mut s, m, t) = monk_and_target(seed, EntityKind::TradeCart);
        s.push_command(Command::Convert { entity: m, target: t });
        let mut converted_at = None;
        for n in 1..=675u64 {
            s.tick();
            if ent(&s, t).owner == OWNER_PLAYER {
                converted_at = Some(n);
                break;
            }
        }
        if converted_at == Some(675) {
            // Replay, but lift the conversion one tick before the
            // guarantee; the streams must still agree at 675.
            let (mut s2, m2, t2) = monk_and_target(seed, EntityKind::TradeCart);
            s2.push_command(Command::Convert { entity: m2, target: t2 });
            for _ in 0..674 {
                s2.tick();
            }
            s2.entities
                .iter_mut()
                .find(|e| e.id == m2)
                .unwrap()
                .conversion_target = None;
            s2.tick();
            assert_eq!(ent(&s2, t2).owner, OWNER_OPPONENT, "lifted: no conversion");
            assert_eq!(
                s.rng.state_words(),
                s2.rng.state_words(),
                "the guarantee CI must be a counter check, not a roll"
            );
            done = true;
            break;
        }
    }
    assert!(done, "no seed in 200..260 reached the guarantee CI");
}

// ------------------------------------------------------------------
// MNK-05 — resistance: Scouts level 2; Devotion shifts the victim.

#[test]
fn scout_resistance_window_is_ci_8_to_10() {
    for seed in 1..=12u64 {
        let (mut sim, monk, scout) = monk_and_target(seed, EntityKind::ScoutCavalry);
        sim.push_command(Command::Convert { entity: monk, target: scout });
        let at = ticks_until_converted(&mut sim, scout, 800)
            .unwrap_or_else(|| panic!("seed {seed}: scout never converted"));
        assert!(
            (600..=750).contains(&at),
            "seed {seed}: scout converted at {at}, outside CIs 8–10 (600–750)"
        );
    }
}

#[test]
fn devotion_shifts_the_victims_window_by_one_ci() {
    let (mut sim, monk, knight) = monk_and_target(7, EntityKind::Knight);
    // The TARGET's owner holds Devotion: +1/+1.
    sim.players[1].techs |= TechId::Devotion.bit();
    sim.push_command(Command::Convert { entity: monk, target: knight });
    sim.tick();
    let m = ent(&sim, monk);
    assert_eq!(m.conversion_start_shift, 1);
    assert_eq!(m.conversion_guarantee_shift, 1);
    let at = ticks_until_converted(&mut sim, knight, 800).expect("converts");
    assert!(
        (450..=750).contains(&(at + 1)),
        "devotion window should be CIs 6–10, landed at {at}"
    );
}

// ------------------------------------------------------------------
// MNK-06 — group conversion: independent records, shared rest.

#[test]
fn group_conversion_drains_every_participant() {
    let mut sim = Sim::new(7);
    spawn(&mut sim, 9001, EntityKind::Monk, OWNER_PLAYER, (60, 60));
    spawn(&mut sim, 9002, EntityKind::Monk, OWNER_PLAYER, (60, 62));
    spawn(&mut sim, 9003, EntityKind::Monk, OWNER_PLAYER, (62, 62));
    spawn(&mut sim, 9010, EntityKind::Knight, OWNER_OPPONENT, (61, 61));
    for id in [9001u32, 9002, 9003] {
        sim.entities.iter_mut().find(|e| e.id == id).unwrap().hp =
            I16F16::from_num(30_000);
        sim.push_command(Command::Convert { entity: id, target: 9010 });
    }
    let at = ticks_until_converted(&mut sim, 9010, 700).expect("knight converts");
    assert!(at <= 675, "group conversion still lands inside one window");
    for id in [9001u32, 9002, 9003] {
        // Drained to 0 by the success; a participant processed
        // AFTER the converter in the same tick has already
        // regenned one tick of faith (≤26 milli-percent) by the
        // time the tick ends — the drain, not the exact zero, is
        // the pin (the single-monk drain test pins the zero).
        assert!(
            ent(&sim, id).faith <= 26,
            "participant {id} must rest after the group success, faith {}",
            ent(&sim, id).faith
        );
        assert_eq!(ent(&sim, id).conversion_target, None);
    }
}

// ------------------------------------------------------------------
// MNK-07 — sticky progress.

#[test]
fn ci_progress_is_sticky_across_targets_and_chase_but_not_manual_move() {
    let mut sim = Sim::new(7);
    spawn(&mut sim, 9001, EntityKind::Monk, OWNER_PLAYER, (60, 60));
    spawn(&mut sim, 9002, EntityKind::Knight, OWNER_OPPONENT, (62, 60));
    spawn(&mut sim, 9003, EntityKind::Knight, OWNER_OPPONENT, (60, 63));
    sim.entities.iter_mut().find(|e| e.id == 9001).unwrap().hp =
        I16F16::from_num(30_000);
    sim.push_command(Command::Convert { entity: 9001, target: 9002 });
    for _ in 0..150 {
        sim.tick();
    }
    assert_eq!(ent(&sim, 9001).ci_progress, 2, "two CIs banked");
    // Unit→unit switch: progress survives.
    sim.push_command(Command::Convert { entity: 9001, target: 9003 });
    sim.tick();
    assert_eq!(ent(&sim, 9001).conversion_target, Some(9003));
    assert_eq!(ent(&sim, 9001).ci_progress, 2, "switch must not reset");
    // Automatic chase: the target teleports out of range; the
    // monk walks after it and the counter does not move or reset.
    {
        let (x, y) = sim.grid().cell_center(90, 90);
        let t = sim.entities.iter_mut().find(|e| e.id == 9003).unwrap();
        t.pos_x = x;
        t.pos_y = y;
    }
    for _ in 0..30 {
        sim.tick();
    }
    assert_eq!(ent(&sim, 9001).ci_progress, 2, "chase must not reset");
    assert_eq!(ent(&sim, 9001).conversion_target, Some(9003));
    // Manual move: the one reset.
    let (x, y) = sim.grid().cell_center(55, 55);
    sim.push_command(Command::Move { entity: 9001, x, y });
    sim.tick();
    assert_eq!(ent(&sim, 9001).ci_progress, 0, "manual move resets");
    assert_eq!(ent(&sim, 9001).conversion_target, None);
}

// ------------------------------------------------------------------
// MNK-08 — eligibility matrix.

#[test]
fn eligibility_matrix() {
    let mut sim = Sim::new(7);
    spawn(&mut sim, 9001, EntityKind::Monk, OWNER_PLAYER, (60, 60));
    spawn(&mut sim, 9002, EntityKind::Monk, OWNER_OPPONENT, (62, 60));
    spawn(&mut sim, 9003, EntityKind::BatteringRam, OWNER_OPPONENT, (62, 62));
    spawn(&mut sim, 9004, EntityKind::Villager, OWNER_OPPONENT, (58, 60));
    spawn(&mut sim, 9005, EntityKind::Knight, OWNER_PLAYER, (58, 62));
    // Enemy monk: refused without Atonement, admitted with it.
    assert!(!sim.can_convert_target(9001, 9002));
    sim.players[0].techs |= TechId::Atonement.bit();
    assert!(sim.can_convert_target(9001, 9002));
    // Siege (the Ram): refused — Redemption is a P5c tech, so
    // the gate is closed for everyone in this slice.
    assert!(!sim.can_convert_target(9001, 9003), "siege needs Redemption (P5c)");
    // Villagers: always eligible. Own units: never.
    assert!(sim.can_convert_target(9001, 9004));
    assert!(!sim.can_convert_target(9001, 9005));
    // A garrisoned unit is never offered as a target (GAR-02).
    sim.entities.iter_mut().find(|e| e.id == 9004).unwrap().garrisoned_in =
        Some(GarrisonRef::Building(999));
    assert!(!sim.can_convert_target(9001, 9004));
}

// ------------------------------------------------------------------
// MNK-12 — transfer + frozen snapshot.

#[test]
fn converted_knight_keeps_its_snapshot_against_both_owners_later_techs() {
    let (mut sim, monk, knight) = monk_and_target(7, EntityKind::Knight);
    // The old owner holds Bloodlines at conversion time: the
    // snapshot carries it (+20 max HP).
    sim.players[1].techs |= TechId::Bloodlines.bit();
    // Wound the knight and give it an order: HP must survive the
    // transfer, the order must not.
    sim.entities.iter_mut().find(|e| e.id == knight).unwrap().hp = I16F16::from_num(50);
    sim.entities.iter_mut().find(|e| e.id == knight).unwrap().attack_target = Some(monk);
    let pop_before = (sim.population_of(OWNER_PLAYER), sim.population_of(OWNER_OPPONENT));
    let score_before = sim.destroyed_value_of(OWNER_PLAYER);
    sim.push_command(Command::Convert { entity: monk, target: knight });
    ticks_until_converted(&mut sim, knight, 700).expect("knight converts");
    let k = ent(&sim, knight);
    assert_eq!(k.owner, OWNER_PLAYER);
    assert_eq!(k.hp, I16F16::from_num(50), "current HP untouched by transfer");
    assert_eq!(k.attack_target, None, "orders cleared on transfer");
    assert!(k.frozen_patches.is_some(), "snapshot stamped");
    let ki = sim.entities().iter().position(|e| e.id == knight).unwrap();
    let at_conversion = sim.unit_stats(ki);
    assert_eq!(at_conversion.max_hp, 120, "snapshot carries Bloodlines' +20");
    // Population moved seats the same tick; VIC-04 credited the
    // converted value (Knight: 60 F + 75 G = 135) like a kill.
    assert_eq!(sim.population_of(OWNER_PLAYER), pop_before.0 + 1);
    assert_eq!(sim.population_of(OWNER_OPPONENT), pop_before.1 - 1);
    assert_eq!(sim.destroyed_value_of(OWNER_PLAYER), score_before + 135);
    // Neither owner's LATER research changes the converted unit.
    sim.players[0].techs |= TechId::Husbandry.bit() | TechId::Forging.bit();
    sim.players[1].techs |= TechId::Forging.bit() | TechId::ScaleBarding.bit();
    let after = sim.unit_stats(ki);
    assert_eq!(after.max_hp, at_conversion.max_hp);
    assert_eq!(after.speed, at_conversion.speed);
    assert_eq!(after.attacks, at_conversion.attacks);
    assert_eq!(after.armor_melee, at_conversion.armor_melee);
    // A converted villager builds with the NEW owner's set: build
    // gates read the owner (age/techs), never the snapshot — the
    // transferred villager's owner IS the player now.
    let (mut sim2, monk2, vil) = monk_and_target(7, EntityKind::Villager);
    let _ = monk2;
    sim2.push_command(Command::Convert { entity: monk2, target: vil });
    ticks_until_converted(&mut sim2, vil, 700).expect("villager converts");
    assert_eq!(ent(&sim2, vil).owner, OWNER_PLAYER);
    assert!(ent(&sim2, vil).frozen_patches.is_some());
}

#[test]
fn converted_monk_loses_its_own_in_flight_conversion() {
    let mut sim = Sim::new(7);
    spawn(&mut sim, 9001, EntityKind::Monk, OWNER_PLAYER, (60, 60));
    spawn(&mut sim, 9002, EntityKind::Monk, OWNER_OPPONENT, (62, 60));
    spawn(&mut sim, 9003, EntityKind::Knight, OWNER_PLAYER, (64, 60));
    // Player holds Atonement; the enemy monk is mid-conversion
    // on the player's knight (staged directly — scene-setting).
    sim.players[0].techs |= TechId::Atonement.bit();
    {
        let e = sim.entities.iter_mut().find(|e| e.id == 9002).unwrap();
        e.conversion_target = Some(9003);
        e.ci_progress = 3;
        e.hp = I16F16::from_num(30_000);
    }
    sim.entities.iter_mut().find(|e| e.id == 9001).unwrap().hp =
        I16F16::from_num(30_000);
    sim.push_command(Command::Convert { entity: 9001, target: 9002 });
    ticks_until_converted(&mut sim, 9002, 700).expect("enemy monk converts");
    let m = ent(&sim, 9002);
    assert_eq!(m.owner, OWNER_PLAYER);
    assert_eq!(m.conversion_target, None, "in-flight conversion discarded");
    assert_eq!(m.ci_progress, 0, "allegiance change resets CI progress");
}

// ------------------------------------------------------------------
// Tech rows (TEC-10 P5b arm).

#[test]
fn atonement_and_devotion_rows() {
    assert_eq!(TechId::Atonement.tag(), 35);
    assert_eq!(TechId::Devotion.tag(), 36);
    assert_eq!(TechId::Atonement.cost(), (0, 0, 325, 0));
    assert_eq!(TechId::Devotion.cost(), (100, 0, 200, 0));
    assert_eq!(TechId::Atonement.ticks(), 2400);
    assert_eq!(TechId::Devotion.ticks(), 2400);
    assert_eq!(
        TechId::Atonement.host(),
        crate::TechHost::Building(crate::build::BuildingKind::Monastery)
    );
    assert_eq!(TechId::Devotion.age_required(), crate::tech::Age::Castle);
}
