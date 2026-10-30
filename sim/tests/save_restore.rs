//! NET-09 — save/restore round-trip tests.
//!
//! A saved game is a full serialization of sim state: restoring
//! it mid-match must reproduce the state hash exactly, and the
//! restored match must then continue bit-identically to the
//! match that was never paused (same commands ⇒ same hashes,
//! tick for tick). Corrupt or foreign bytes are refused whole.

use fixed::types::I16F16;
use skirmish_sim::{Command, MatchMode, Sim};

/// Drive a match into a genuinely mid-game state: villagers
/// working, a queue running, units moving — then save it.
fn mid_match() -> Sim {
    let mut sim = Sim::new_match(42, MatchMode::AiNormal);
    // Send two villagers to work and queue another at the TC.
    let villagers: Vec<u32> = sim
        .entities()
        .iter()
        .filter(|e| e.kind == skirmish_sim::EntityKind::Villager && e.owner == 0)
        .map(|e| e.id)
        .collect();
    let tree = sim
        .nodes()
        .iter()
        .find(|n| n.kind == skirmish_sim::NodeKind::Tree)
        .map(|n| n.id)
        .expect("generated map has trees");
    for (i, v) in villagers.iter().enumerate() {
        if i % 2 == 0 {
            sim.push_command(Command::Gather {
                entity: *v,
                node: tree,
            });
        } else {
            sim.push_command(Command::Move {
                entity: *v,
                x: I16F16::from_num(30),
                y: I16F16::from_num(-20),
            });
        }
    }
    sim.push_command(Command::TrainVillager);
    for _ in 0..900 {
        sim.tick();
    }
    sim
}

#[test]
fn save_restore_round_trip_preserves_hash() {
    let sim = mid_match();
    let bytes = sim.save_game();
    let restored = Sim::load_game(&bytes).expect("a save we wrote must load");
    assert_eq!(restored.tick_count(), sim.tick_count());
    assert_eq!(
        restored.state_hash(),
        sim.state_hash(),
        "restored state must hash identically to the saved state"
    );
    // And a second save of the restored state is byte-identical:
    // the format is canonical, not just equivalent.
    assert_eq!(restored.save_game(), bytes);
}

#[test]
fn restored_match_continues_deterministically() {
    let mut live = mid_match();
    let bytes = live.save_game();
    let mut restored = Sim::load_game(&bytes).expect("save loads");
    // Feed both the same fresh orders and run: the hash streams
    // must agree tick for tick — the restore re-entered lockstep
    // as if the match had never paused.
    let scout = live
        .entities()
        .iter()
        .find(|e| e.kind == skirmish_sim::EntityKind::ScoutCavalry)
        .map(|e| e.id)
        .expect("opening package has a scout");
    for sim in [&mut live, &mut restored] {
        sim.push_command(Command::Move {
            entity: scout,
            x: I16F16::from_num(-40),
            y: I16F16::from_num(40),
        });
        sim.push_command(Command::TrainVillager);
    }
    for tick in 0..1200 {
        let ha = live.tick();
        let hb = restored.tick();
        assert_eq!(ha, hb, "restored match diverged at +{tick} ticks");
    }
}

#[test]
fn save_at_tick_zero_round_trips() {
    let sim = Sim::new(7);
    let restored = Sim::load_game(&sim.save_game()).expect("fresh save loads");
    assert_eq!(restored.state_hash(), sim.state_hash());
}

#[test]
fn corrupt_and_foreign_bytes_are_refused() {
    let sim = mid_match();
    let bytes = sim.save_game();
    // Not a save at all.
    assert!(Sim::load_game(b"").is_none());
    assert!(Sim::load_game(b"not a save file").is_none());
    assert!(Sim::load_game(&bytes[..bytes.len() / 2]).is_none(), "truncated");
    // One flipped byte in the body: the hash trailer refuses it.
    let mut tampered = bytes.clone();
    let mid = tampered.len() / 2;
    tampered[mid] ^= 0xFF;
    assert!(Sim::load_game(&tampered).is_none(), "tampered body");
    // One flipped byte in the trailer itself.
    let mut tampered = bytes.clone();
    let last = tampered.len() - 1;
    tampered[last] ^= 0x01;
    assert!(Sim::load_game(&tampered).is_none(), "tampered trailer");
    // Trailing garbage after a valid save.
    let mut extended = bytes.clone();
    extended.push(0);
    assert!(Sim::load_game(&extended).is_none(), "trailing bytes");
    // An unknown version is refused, never guessed at.
    let mut future = bytes.clone();
    future[4] = 99;
    assert!(Sim::load_game(&future).is_none(), "unknown version");
}
