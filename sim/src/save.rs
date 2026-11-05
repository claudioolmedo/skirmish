//! NET-09 — Save/restore: a saved game is a full serialization
//! of sim state.
//!
//! The body follows the canonical inventory of `Sim::hash_parts`
//! (SIM-05): every field the state hash covers, written in the
//! hash's own order, plus the few behavioral fields the hash
//! deliberately omits — `next_building_id` (id allocation must
//! continue where the match left off), the AutoExplore pair
//! (SEL-17's walker state), and the pending command queue (a
//! save taken between frames must not eat orders already
//! given). No renderer state is saved: fog *visibility lists*
//! are bookkeeping derivable from the fog grids, and everything
//! visual is a function of sim state.
//!
//! Framing: a short magic, a format version, the body, and the
//! state's own SIM-05 hash as the trailer. Loading verifies the
//! trailer against the restored state before returning it, so a
//! corrupt, truncated or foreign file is refused whole — a
//! restore either re-enters lockstep from the saved tick as if
//! the match had never paused, or does not happen.
//!
//! All integers are little-endian; fixed-point values travel as
//! their raw bits, exactly as the hash reads them. Tags reuse
//! the enums' existing stable tag spaces wherever one exists.

use fixed::types::I16F16;

use crate::build::{Building, BuildingKind};
use crate::combat::Projectile;
use crate::econ::{NodeKind, ResourceKind, ResourceNode, Stockpile};
use crate::entity::{
    BellSnapshot, Dropoff, Entity, EntityKind, GarrisonRef, MoveOrder, QueuedOrder, Stance, Task,
};
use crate::grid::{Grid, GRID_DIM};
use crate::market::Market;
use crate::rng::Pcg32;
use crate::tech::{Age, ProdEntry, TechId};
use crate::formation::FormationKind;
use crate::{Command, MatchMode, PlayerState, Sim, TcState, TeamSettings, VictorySettings};

/// File magic: "SKSV" (Skirmish Save).
const MAGIC: &[u8; 4] = b"SKSV";
/// Save format version. Bump on any layout change; loaders
/// refuse versions they do not know. Version 2: the Town Bell
/// snapshot carries its ringing TC (`bell_tc`, GAR-07 fix).
/// Version 3: Package J — building records carry `gate_along_x`,
/// TC records carry the repair accumulators, and the pending-
/// command codec gains `Command::RepairTc` (tag 38).
/// Version 4: Package M — the building record gains the
/// Conscription queue-rate remainder.
/// Version 5: Castle P4b landing — the entity record gains
/// `attack_point` (CMB-16), and the command / queued-order
/// codecs gain their AttackGround tags, renumbered on landing
/// (Command 39, QueuedOrder 8 — the builder's 38/7 were already
/// RepairTc here).
/// Version 6: wave-1 leftovers landing — the players section
/// carries the INF-11 counters, the gathered totals and the
/// ECO-14 reseed state, node records carry the node's stamped
/// full amount, and the command codec gains the reseed commands
/// at tags 40-42 (the builder's 38-40 were taken here).
/// Version 7: Castle P5a landing — the entity record gains the
/// monk heal fields, and the command codec gains `Heal` at
/// tag 43 (the builder's 38 was taken here).
/// Version 8: Castle P5b — the entity record gains the monk
/// faith/conversion block and the frozen patch snapshot
/// (MNK-12), and the codecs gain `Convert` (Command tag 44,
/// QueuedOrder tag 9).
/// Version 9: Castle P5c — the entity record gains the monk's
/// building conversion target (MNK-04), and the codecs gain
/// `ConvertBuilding` (Command tag 45, QueuedOrder tag 10).
/// Version 10: Castle P5d — the entity record gains the relic
/// fields (the monk's carry/pickup/deposit state and the
/// relic's carried-by/garrisoned-in pointers, MNK-09), the
/// building record gains the Monastery relic store and its
/// ECO-16 trickle remainder, and the command codec gains
/// `PickupRelic` (tag 46) and `DepositRelic` (tag 47).
/// Version 11: Package C landing combined with P5d — this
/// version carries BOTH deltas on one layout: all of v10's P5d
/// fields exactly as above, PLUS the world section carries the
/// victory settings (Package C: VIC-05/06/07) after the mode
/// tag, and each player record carries the `mode_out` flag; a
/// non-default settings block is hashed, so without it a
/// Regicide / Sudden Death / timed save could never pass the
/// load-time hash check. (The builder tree numbered its own
/// victory-only delta v10 as well; the combined format is v11.)
/// Version 12: VIC-03 (relic victory) — the victory flags gain
/// bit 2 (relic victory), and a match under that setting carries
/// its relic countdown state (holder + accrued hold ticks) right
/// after the victory settings in the world section; the block is
/// present only while the flag is on, which the flags byte
/// itself tells the reader, so a non-relic v12 save is laid out
/// exactly as v11 was apart from the version word.
/// Version 13: MOV-06 — the buildings section carries
/// `next_group_id` right after `next_building_id` (the group-id
/// allocator must continue where the match left off, like its
/// building twin), and each move-order record carries its group
/// stamp as a flag byte plus, while stamped, the group id and
/// speed cap — a save taken mid-group-move restores the clamp,
/// and its hash (the stamp is hashed state, marker 0xC2) still
/// verifies at load.
/// Version 14: Package I — the victory flags gain bits 3/4/5
/// (Death Match / Empire Wars / King of the Hill, shifted at
/// this landing from the builder tree's 2/3/4 because VIC-03
/// took bit 2), and a match under the King of the Hill flag
/// carries its countdown state (ticks left + decided winner)
/// right after VIC-03's block in the world section; the block
/// is present only while the flag is on, which the flags byte
/// itself tells the reader, so a non-KotH v14 save is laid out
/// exactly as v13 was apart from the version word. (Package
/// I's builder tree wrote no save delta at all — its base
/// predated the v11 settings round-trip; without this block a
/// mid-KotH save could never pass the load-time hash check,
/// the countdown being hashed state in KotH matches.)
/// Version 15: VIC-11 + MOV-07 (landed together) — each
/// player record carries the `resigned` flag right after
/// `mode_out` (hashed while set, so a post-resignation save
/// could never pass the load-time hash check without it), and
/// the command codec gains `Command::Resign` at tag 48 and
/// `Command::FormationMove` at tag 49 (append-only, like every
/// tag before it; MOV-07 was renumbered 48 -> 49 at landing
/// because both builder packages had taken 48). The
/// queued-order codec gains its queued form of FormationMove
/// (QueuedOrder tag 11). No other record layout changes:
/// formation slots live in ordinary move orders, so a v15
/// save with no formation order pending is laid out exactly
/// as v14 apart from the version word and the resigned flag.
/// Version 16: VIC-02 (wonder victory) — the victory flags
/// gain bit 6 (wonder victory), and a match under that setting
/// carries its wonder countdown state (holder + accrued hold
/// ticks) right after VIC-03's block in the world section; the
/// block is present only while the flag is on, which the flags
/// byte itself tells the reader, so a non-wonder v16 save is
/// laid out exactly as v15 was apart from the version word and
/// the next delta: the building record gains the repair gold
/// accumulator (VIC-02's gold cost leg), written for every
/// building like its wood/stone twins — 0 for every kind with
/// no gold cost, so it round-trips the hash unchanged.
/// Version 17: Teams T1 (VIS-08's foundation) — the world
/// section carries the team settings (two team bytes, in slot
/// order) right after the victory settings, the block present
/// only while the settings are non-default (distinct teams).
/// Its presence cannot be inferred from any byte already
/// written — the victory flags byte's one free bit (bit 7) is
/// reserved for allied victory (T2) — so an explicit one-byte
/// presence flag is written in every v17 save (0 = default
/// teams, no block follows; 1 = the two team bytes follow),
/// which the reader never has to guess. A default-teams v17
/// save is therefore laid out exactly as v16 apart from the
/// version word and that one presence byte.
/// Teams T2 (VIC-12) note, recorded on this version — NO new
/// version: the victory flags byte above gains bit 7 (allied
/// victory), which the existing flags write/parse carries
/// with no layout change, and while that flag is on the
/// relic/wonder countdown holders stored by the v12/v16
/// blocks are scope keys (team ids; see `Sim`'s holder
/// fields) instead of owner tags — a team id and an owner
/// tag are both one byte, so the existing (holder, ticks)
/// pairs express the re-keyed state exactly, and the flag
/// itself round-trips in the same block that tells the
/// reader which meaning the byte carries.
const SAVE_VERSION: u32 = 17;

/// Little-endian byte writer over a growable buffer.
struct W {
    buf: Vec<u8>,
}

impl W {
    fn new() -> Self {
        W { buf: Vec::new() }
    }
    fn u8(&mut self, v: u8) {
        self.buf.push(v);
    }
    fn bool(&mut self, v: bool) {
        self.u8(u8::from(v));
    }
    fn u32(&mut self, v: u32) {
        self.buf.extend_from_slice(&v.to_le_bytes());
    }
    fn i32(&mut self, v: i32) {
        self.buf.extend_from_slice(&v.to_le_bytes());
    }
    fn u64(&mut self, v: u64) {
        self.buf.extend_from_slice(&v.to_le_bytes());
    }
    fn i64(&mut self, v: i64) {
        self.buf.extend_from_slice(&v.to_le_bytes());
    }
    fn usize(&mut self, v: usize) {
        self.u64(v as u64);
    }
    fn fx(&mut self, v: I16F16) {
        self.i32(v.to_bits());
    }
    fn bytes(&mut self, v: &[u8]) {
        self.buf.extend_from_slice(v);
    }
    fn opt_u32(&mut self, v: Option<u32>) {
        match v {
            None => self.u8(0),
            Some(x) => {
                self.u8(1);
                self.u32(x);
            }
        }
    }
    fn opt_u64(&mut self, v: Option<u64>) {
        match v {
            None => self.u8(0),
            Some(x) => {
                self.u8(1);
                self.u64(x);
            }
        }
    }
    fn opt_u8(&mut self, v: Option<u8>) {
        match v {
            None => self.u8(0),
            Some(x) => {
                self.u8(1);
                self.u8(x);
            }
        }
    }
    fn opt_point(&mut self, v: Option<(I16F16, I16F16)>) {
        match v {
            None => self.u8(0),
            Some((x, y)) => {
                self.u8(1);
                self.fx(x);
                self.fx(y);
            }
        }
    }
    fn opt_cell(&mut self, v: Option<(u32, u32)>) {
        match v {
            None => self.u8(0),
            Some((x, y)) => {
                self.u8(1);
                self.u32(x);
                self.u32(y);
            }
        }
    }
    fn garrison_ref(&mut self, v: Option<GarrisonRef>) {
        match v {
            None => self.u8(0),
            Some(r) => {
                self.u8(r.tag());
                self.u32(r.id());
            }
        }
    }
    fn dropoff(&mut self, v: Option<Dropoff>) {
        match v {
            None => self.u8(0),
            Some(d) => {
                self.u8(d.tag());
                if let Dropoff::Building(id) = d {
                    self.u32(id);
                }
            }
        }
    }
    fn task(&mut self, v: Task) {
        self.u8(v.tag());
        if let Task::Repair { building } = v {
            self.u32(building);
        }
        // Package J landing: Task::RepairTc (tag 7) carries its
        // Town Center id the same way.
        if let Task::RepairTc { tc } = v {
            self.u32(tc);
        }
    }
    fn queued_order(&mut self, v: &QueuedOrder) {
        self.u8(v.tag());
        match *v {
            QueuedOrder::Move { x, y }
            | QueuedOrder::AttackMove { x, y }
            | QueuedOrder::AttackGround { x, y }
            | QueuedOrder::Patrol { x, y } => {
                self.fx(x);
                self.fx(y);
            }
            QueuedOrder::Gather { node } => self.u32(node),
            QueuedOrder::Attack { target } => self.u32(target),
            QueuedOrder::Build { kind, x, y } => {
                self.u8(kind.tag());
                self.fx(x);
                self.fx(y);
            }
            QueuedOrder::Repair { building } => self.u32(building),
            // Package J landing: QueuedOrder::RepairTc (tag 7).
            QueuedOrder::RepairTc { tc } => self.u32(tc),
            // Castle P5b: QueuedOrder::Convert (tag 9).
            QueuedOrder::Convert { target } => self.u32(target),
            // Castle P5c: QueuedOrder::ConvertBuilding (tag 10).
            QueuedOrder::ConvertBuilding { building } => self.u32(building),
            // MOV-07: QueuedOrder::FormationMove (tag 11).
            QueuedOrder::FormationMove { x, y, formation } => {
                self.fx(x);
                self.fx(y);
                self.u8(formation.tag());
            }
        }
    }
    fn bell_snapshot(&mut self, v: Option<BellSnapshot>) {
        match v {
            None => self.u8(0),
            Some(s) => {
                self.u8(1);
                self.fx(s.pos_x);
                self.fx(s.pos_y);
                self.task(s.task);
                self.opt_u32(s.task_node);
                self.opt_u32(s.task_building);
                self.u32(s.bell_tc);
            }
        }
    }
    fn move_order(&mut self, v: Option<&MoveOrder>) {
        match v {
            None => self.u8(0),
            Some(o) => {
                self.u8(1);
                self.fx(o.target_x);
                self.fx(o.target_y);
                self.fx(o.arrive);
                self.usize(o.wp_index);
                self.usize(o.waypoints.len());
                for &(x, y) in &o.waypoints {
                    self.fx(x);
                    self.fx(y);
                }
                // MOV-06 (v13): the group stamp, flagged.
                if o.group_id != 0 {
                    self.u8(1);
                    self.u32(o.group_id);
                    self.fx(o.group_cap);
                } else {
                    self.u8(0);
                }
            }
        }
    }
    fn prod_queue(&mut self, q: &[ProdEntry]) {
        self.usize(q.len());
        for e in q {
            self.u32(e.code());
        }
    }
    fn command(&mut self, c: &Command) {
        match *c {
            Command::Noop => self.u8(0),
            Command::Move { entity, x, y } => {
                self.u8(1);
                self.u32(entity);
                self.fx(x);
                self.fx(y);
            }
            Command::Gather { entity, node } => {
                self.u8(2);
                self.u32(entity);
                self.u32(node);
            }
            Command::TrainVillager => self.u8(3),
            Command::TrainVillagerAt { tc } => {
                self.u8(4);
                self.u32(tc);
            }
            Command::Build {
                entity,
                kind,
                x,
                y,
            } => {
                self.u8(5);
                self.u32(entity);
                self.u8(kind.tag());
                self.fx(x);
                self.fx(y);
            }
            Command::BuildLine {
                entity,
                kind,
                x0,
                y0,
                x1,
                y1,
            } => {
                self.u8(6);
                self.u32(entity);
                self.u8(kind.tag());
                self.fx(x0);
                self.fx(y0);
                self.fx(x1);
                self.fx(y1);
            }
            Command::Attack { entity, target } => {
                self.u8(7);
                self.u32(entity);
                self.u32(target);
            }
            Command::AttackBuilding { entity, building } => {
                self.u8(8);
                self.u32(entity);
                self.u32(building);
            }
            Command::TrainAt { building, kind } => {
                self.u8(9);
                self.u32(building);
                self.u8(kind.tag());
            }
            Command::Queue { entity, order } => {
                self.u8(10);
                self.u32(entity);
                self.queued_order(&order);
            }
            Command::SetRally { building, x, y } => {
                self.u8(11);
                self.opt_u32(building);
                self.fx(x);
                self.fx(y);
            }
            Command::SetRallyTc { tc, x, y } => {
                self.u8(12);
                self.u32(tc);
                self.fx(x);
                self.fx(y);
            }
            Command::ResearchTc { tech } => {
                self.u8(13);
                self.u8(tech.tag());
            }
            Command::ResearchTcAt { tc, tech } => {
                self.u8(14);
                self.u32(tc);
                self.u8(tech.tag());
            }
            Command::ResearchAt { building, tech } => {
                self.u8(15);
                self.u32(building);
                self.u8(tech.tag());
            }
            Command::Repair { entity, building } => {
                self.u8(16);
                self.u32(entity);
                self.u32(building);
            }
            Command::Trade { entity, building } => {
                self.u8(17);
                self.u32(entity);
                self.u32(building);
            }
            Command::MarketBuy { building, resource } => {
                self.u8(18);
                self.u32(building);
                self.u8(resource.tag());
            }
            Command::MarketSell { building, resource } => {
                self.u8(19);
                self.u32(building);
                self.u8(resource.tag());
            }
            Command::ForceDeposit { entity } => {
                self.u8(20);
                self.u32(entity);
            }
            Command::SetAutoExplore { entity, enabled } => {
                self.u8(21);
                self.u32(entity);
                self.bool(enabled);
            }
            Command::AttackMove { entity, x, y } => {
                self.u8(22);
                self.u32(entity);
                self.fx(x);
                self.fx(y);
            }
            Command::Patrol { entity, x, y } => {
                self.u8(23);
                self.u32(entity);
                self.fx(x);
                self.fx(y);
            }
            Command::SetStance { entity, stance } => {
                self.u8(24);
                self.u32(entity);
                self.u8(stance.tag());
            }
            Command::Stop { entity } => {
                self.u8(25);
                self.u32(entity);
            }
            Command::DeleteEntity { entity } => {
                self.u8(26);
                self.u32(entity);
            }
            Command::DeleteBuilding { building } => {
                self.u8(27);
                self.u32(building);
            }
            Command::CancelQueueTc { tc, index } => {
                self.u8(28);
                self.u32(tc);
                self.u32(index);
            }
            Command::CancelQueueBuilding { building, index } => {
                self.u8(29);
                self.u32(building);
                self.u32(index);
            }
            Command::GarrisonBuilding { entity, building } => {
                self.u8(30);
                self.u32(entity);
                self.u32(building);
            }
            Command::GarrisonTc { entity, tc } => {
                self.u8(31);
                self.u32(entity);
                self.u32(tc);
            }
            Command::UngarrisonBuilding { building } => {
                self.u8(32);
                self.u32(building);
            }
            Command::UngarrisonTc { tc } => {
                self.u8(33);
                self.u32(tc);
            }
            Command::TownBell { tc } => {
                self.u8(34);
                self.u32(tc);
            }
            // Combo landing (bot-loop 2026-10-30): the save codec
            // predates Castle P4a and Package K — their commands
            // append here, tags 35-37, append-only like the enum.
            Command::GarrisonUnit { entity, host } => {
                self.u8(35);
                self.u32(entity);
                self.u32(host);
            }
            Command::UngarrisonUnit { host } => {
                self.u8(36);
                self.u32(host);
            }
            Command::Tribute {
                sender,
                recipient,
                resource,
                amount,
            } => {
                self.u8(37);
                self.u8(sender);
                self.u8(recipient);
                self.u8(resource.tag());
                self.i32(amount);
            }
            // Package J landing: Command::RepairTc joins the
            // pending-command codec append-only at tag 38.
            Command::RepairTc { entity, tc } => {
                self.u8(38);
                self.u32(entity);
                self.u32(tc);
            }
            // Castle P4b landing (CMB-16): tag 39, append-only
            // like the enum (the builder's 38 was RepairTc here).
            Command::AttackGround { entity, x, y } => {
                self.u8(39);
                self.u32(entity);
                self.fx(x);
                self.fx(y);
            }
            // Wave-1 leftovers landing (ECO-14): tags 40-42,
            // append-only (the builder's 38-40 were taken here).
            Command::QueueReseed { building } => {
                self.u8(40);
                self.u32(building);
            }
            Command::DequeueReseed { building } => {
                self.u8(41);
                self.u32(building);
            }
            Command::SetAutoReseed { building, on } => {
                self.u8(42);
                self.u32(building);
                self.bool(on);
            }
            // Castle P5a landing: the Monk's heal order
            // (MNK-10) at tag 43, append-only (the builder's
            // 38 was taken here).
            Command::Heal { entity, target } => {
                self.u8(43);
                self.u32(entity);
                self.u32(target);
            }
            // Castle P5b: the Monk's convert order (MNK-03) at
            // tag 44, append-only.
            Command::Convert { entity, target } => {
                self.u8(44);
                self.u32(entity);
                self.u32(target);
            }
            // Castle P5c: the Monk's building convert order
            // (MNK-04) at tag 45, append-only.
            Command::ConvertBuilding { entity, building } => {
                self.u8(45);
                self.u32(entity);
                self.u32(building);
            }
            // Castle P5d: the Monk's relic orders (MNK-09) at
            // tags 46/47, append-only.
            Command::PickupRelic { entity, relic } => {
                self.u8(46);
                self.u32(entity);
                self.u32(relic);
            }
            Command::DepositRelic { entity, building } => {
                self.u8(47);
                self.u32(entity);
                self.u32(building);
            }
            // VIC-11: the resignation command (tag 48,
            // append-only) — the resigning seat's owner byte.
            Command::Resign { player } => {
                self.u8(48);
                self.u8(player);
            }
            // MOV-07: Command::FormationMove (tag 49 at
            // landing; see the version-15 note above).
            Command::FormationMove {
                entity,
                x,
                y,
                formation,
            } => {
                self.u8(49);
                self.u32(entity);
                self.fx(x);
                self.fx(y);
                self.u8(formation.tag());
            }
        }
    }
}

impl Sim {
    /// NET-09: serialize the full sim state into a save file
    /// (magic + version + body + the state's SIM-05 hash). The
    /// bytes are self-verifying: `load_game` recomputes the hash
    /// over the restored state and refuses any mismatch.
    pub fn save_game(&self) -> Vec<u8> {
        let mut w = W::new();
        w.bytes(MAGIC);
        w.u32(SAVE_VERSION);
        // --- tick + RNG (the hash's first part) ---
        w.u64(self.tick);
        let (rng_state, rng_inc) = self.rng.state_words();
        w.u64(rng_state);
        w.u64(rng_inc);
        // --- world: mode + the grid's four layers ---
        w.u8(self.mode.tag());
        // Format v11: the victory settings (Package C) — hashed
        // while non-default, so they must round-trip exactly.
        w.u64(self.victory.time_limit_ticks);
        w.u64(self.victory.score_target as u64);
        w.u8(self.victory.flags());
        // Format v17: Teams T1 — the explicit presence byte,
        // then the team block (two team bytes, slot order) only
        // while the settings are non-default; the settings are
        // hashed while non-default, so a non-default save must
        // carry them for the load-time hash check to pass (see
        // the version-17 note at SAVE_VERSION).
        if !self.teams.is_default() {
            w.bool(true);
            w.u8(self.teams.teams[0]);
            w.u8(self.teams.teams[1]);
        } else {
            w.bool(false);
        }
        // Format v12: VIC-03's countdown state, present only
        // while the relic-victory flag (bit 2 above) is on —
        // hashed while a hold stands, so a save taken
        // mid-countdown must carry it for the load-time hash
        // check to pass.
        if self.victory.relic_victory {
            w.bool(self.relic_holder.is_some());
            if let Some(holder) = self.relic_holder {
                w.u8(holder);
            }
            w.u64(self.relic_hold_ticks);
        }
        // Format v16: VIC-02's countdown state, present only
        // while the wonder-victory flag (bit 6 above) is on —
        // hashed while a hold stands, so a save taken
        // mid-countdown must carry it for the load-time hash
        // check to pass.
        if self.victory.wonder_victory {
            w.bool(self.wonder_holder.is_some());
            if let Some(holder) = self.wonder_holder {
                w.u8(holder);
            }
            w.u64(self.wonder_hold_ticks);
        }
        // Format v14: Package I's King of the Hill countdown
        // state, present only while the KotH flag (bit 5 above)
        // is on — hashed in KotH matches, so a save taken
        // mid-countdown must carry it for the load-time hash
        // check to pass.
        if self.victory.king_of_the_hill {
            w.u64(self.koth_ticks_left);
            w.bool(self.koth_winner.is_some());
            if let Some(winner) = self.koth_winner {
                w.u8(winner);
            }
        }
        w.u32(self.grid.width());
        w.u32(self.grid.height());
        w.bytes(self.grid.terrain_bytes());
        w.bytes(self.grid.height_bytes());
        w.bytes(self.grid.node_block_bytes());
        w.bytes(self.grid.dynamic_bytes());
        // --- players: progression, stockpiles, market ---
        for p in &self.players {
            w.u8(p.age.tag());
            w.u64(p.techs);
            w.u64(p.destroyed_value);
            // Format v2: INF-11 counters, gathered totals,
            // ECO-14 reseed state.
            w.u32(p.units_killed);
            w.u32(p.units_lost);
            w.u32(p.buildings_razed);
            w.u32(p.buildings_lost);
            for b in p.gathered_bits {
                w.u64(b as u64);
            }
            w.u32(p.reseed_queue);
            w.bool(p.auto_reseed);
            // Format v11: the mode-elimination flag (Package C).
            w.bool(p.mode_out);
            // Format v15: the resignation flag (VIC-11).
            w.bool(p.resigned);
        }
        for s in &self.stockpiles {
            for a in s.amounts() {
                w.fx(a);
            }
        }
        for r in self.market.rates() {
            w.i32(r);
        }
        // --- nodes ---
        w.usize(self.nodes.len());
        for n in &self.nodes {
            w.u32(n.id);
            w.u8(n.kind.tag());
            w.fx(n.pos_x);
            w.fx(n.pos_y);
            w.fx(n.amount_left);
            // Format v2: the node's stamped full amount (ECO-14's
            // top-up divisor).
            w.fx(n.full_amount);
            w.fx(n.decay_per_tick);
        }
        // --- Town Center states ---
        w.usize(self.tc_states.len());
        for t in &self.tc_states {
            w.u32(t.tc_id);
            w.prod_queue(&t.queue);
            w.u64(t.progress);
            w.opt_point(t.rally);
            w.opt_u32(t.rally_node);
            w.usize(t.garrison.len());
            for id in &t.garrison {
                w.u32(*id);
            }
            w.u32(t.garrison_arrows);
            w.bool(t.bell_active);
            w.opt_u32(t.weapon_target);
            w.opt_u32(t.weapon_target_building);
            w.opt_u64(t.weapon_strike_tick);
            w.u64(t.weapon_next_tick);
            w.bool(t.weapon_engaged);
            // Package J landing: the TC repair accumulators
            // (BLD-07) — hashed while nonzero, so a save taken
            // mid-repair must carry them for the load-time hash
            // check to pass.
            w.i64(t.repair_hp_remainder);
            w.i64(t.repair_cost_remainder);
        }
        // --- buildings ---
        w.u32(self.next_building_id);
        // MOV-06 (v13): the group-id allocator, beside its
        // building twin.
        w.u32(self.next_group_id);
        w.usize(self.buildings.len());
        for b in &self.buildings {
            w.u32(b.id);
            w.u8(b.kind.tag());
            w.u8(b.owner);
            w.fx(b.hp);
            w.u32(b.cell_x);
            w.u32(b.cell_y);
            w.fx(b.pos_x);
            w.fx(b.pos_y);
            w.u64(b.progress_thirds);
            w.i64(b.repair_hp_remainder);
            w.i64(b.repair_cost_remainder);
            w.i64(b.repair_stone_remainder);
            // Format v16: VIC-02's repair gold accumulator.
            w.i64(b.repair_gold_remainder);
            w.i64(b.build_rate_remainder);
            w.i64(b.queue_rate_remainder);
            w.bool(b.complete);
            w.opt_u32(b.node_id);
            w.prod_queue(&b.queue);
            w.u64(b.train_progress);
            w.opt_point(b.rally);
            w.opt_u32(b.rally_node);
            w.opt_u32(b.weapon_target);
            w.opt_u32(b.weapon_target_building);
            w.opt_u64(b.weapon_strike_tick);
            w.u64(b.weapon_next_tick);
            w.bool(b.weapon_engaged);
            // Package J landing: the gate span orientation
            // (hashed for gates), in struct order.
            w.bool(b.gate_along_x);
            w.usize(b.garrison.len());
            for id in &b.garrison {
                w.u32(*id);
            }
            w.u32(b.garrison_arrows);
            // Castle P5d (v10): the Monastery relic store and
            // its ECO-16 trickle remainder (MNK-09/ECO-16) —
            // hashed state, so a save taken mid-trickle must
            // carry it for the load-time hash check to pass.
            w.usize(b.relics.len());
            for id in &b.relics {
                w.u32(*id);
            }
            w.i64(b.relic_gold_remainder);
        }
        // --- entities ---
        w.usize(self.entities.len());
        for e in &self.entities {
            w.u32(e.id);
            w.u8(e.kind.tag());
            w.u8(e.owner);
            w.fx(e.pos_x);
            w.fx(e.pos_y);
            w.fx(e.vel_x);
            w.fx(e.vel_y);
            w.fx(e.hp);
            w.bool(e.dead);
            w.move_order(e.order.as_ref());
            w.usize(e.order_queue.len());
            for o in &e.order_queue {
                w.queued_order(o);
            }
            w.bool(e.order_from_queue);
            w.task(e.task);
            w.opt_u32(e.task_node);
            w.opt_u32(e.task_building);
            w.dropoff(e.task_dropoff);
            w.fx(e.carry);
            match e.carry_kind {
                None => w.u8(0),
                Some(k) => {
                    w.u8(1);
                    w.u8(k.tag());
                }
            }
            w.opt_u32(e.attack_target);
            w.opt_u32(e.attack_building);
            w.opt_point(e.attack_point);
            w.opt_u64(e.strike_tick);
            w.u64(e.next_attack_tick);
            w.bool(e.attack_engaged);
            w.opt_u32(e.last_hit_by);
            w.opt_u8(e.last_hit_owner);
            w.bool(e.suppress_separation);
            w.fx(e.home_x);
            w.fx(e.home_y);
            w.opt_u32(e.aggro_target);
            w.u32(e.hits_taken);
            w.bool(e.aggro_locked);
            w.bool(e.returning_home);
            w.u64(e.flee_until);
            w.fx(e.flee_from_x);
            w.fx(e.flee_from_y);
            w.bool(e.auto_explore);
            w.opt_cell(e.auto_target);
            w.u64(e.wander_until);
            w.u8(e.stance.tag());
            w.fx(e.stance_anchor_x);
            w.fx(e.stance_anchor_y);
            w.bool(e.attack_auto);
            w.opt_point(e.attack_move);
            w.usize(e.patrol.len());
            for &(x, y) in &e.patrol {
                w.fx(x);
                w.fx(y);
            }
            w.usize(e.patrol_next);
            w.garrison_ref(e.garrisoned_in);
            w.garrison_ref(e.garrison_target);
            w.bool(e.garrison_prod_entry);
            w.bell_snapshot(e.bell_snapshot);
            w.i64(e.garrison_heal_remainder);
            w.opt_u32(e.trade_home);
            w.opt_u32(e.trade_target);
            w.bool(e.trade_loaded);
            w.opt_u32(e.heal_target);
            w.i64(e.heal_remainder);
            // Castle P5b (v8): the monk conversion block (MNK-02/
            // 03/05/07) and the frozen patch snapshot (MNK-12),
            // in blank-default form on every non-monk entity.
            w.i32(e.faith);
            w.i64(e.faith_remainder);
            w.opt_u32(e.conversion_target);
            w.u32(e.ci_progress);
            w.u32(e.ci_ticks);
            w.u8(e.conversion_level);
            w.i32(e.conversion_start_shift);
            w.i32(e.conversion_guarantee_shift);
            match e.frozen_patches {
                None => w.u8(0),
                Some(m) => {
                    w.u8(1);
                    w.u64(m);
                }
            }
            // Castle P5c (v9): the building half of the monk
            // conversion record (MNK-04).
            w.opt_u32(e.conversion_target_building);
            w.usize(e.garrison.len());
            for id in &e.garrison {
                w.u32(*id);
            }
            // Castle P5d (v10): the relic fields (MNK-09) —
            // the monk's carry/pickup/deposit state and the
            // relic's carried-by/garrisoned-in pointers.
            w.opt_u32(e.carrying_relic);
            w.opt_u32(e.relic_pickup_target);
            w.opt_u32(e.relic_deposit_target);
            w.opt_u32(e.relic_carried_by);
            w.opt_u32(e.relic_garrisoned_in);
        }
        // --- projectiles ---
        w.usize(self.projectiles.len());
        for p in &self.projectiles {
            w.fx(p.spawn_x);
            w.fx(p.spawn_y);
            w.fx(p.impact_x);
            w.fx(p.impact_y);
            w.u64(p.spawn_tick);
            w.u64(p.impact_tick);
            w.opt_u32(p.intended);
            w.opt_u32(p.intended_building);
            w.u32(p.attacker);
            w.opt_u32(p.attacker_building);
            w.u8(p.owner);
            w.u8(p.attacker_elev);
            w.usize(p.attacks.len());
            for &(class, value) in &p.attacks {
                w.u8(class);
                w.i32(value);
            }
        }
        // --- AI clock, fog, pending commands ---
        w.u64(self.ai.next_wave_tick);
        for slot in &self.fog {
            w.bytes(slot);
        }
        w.usize(self.commands.len());
        for c in &self.commands {
            w.command(c);
        }
        // --- trailer: the state's own hash (SIM-05) ---
        let hash = self.state_hash();
        w.u64(hash);
        w.buf
    }
}

/// Bounds-checked little-endian reader. Every read returns
/// `Option`; any shortfall, unknown tag or implausible count
/// fails the whole load (the caller maps `None` to "not a valid
/// save"), never a partial state.
struct R<'a> {
    bytes: &'a [u8],
    pos: usize,
}

impl<'a> R<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        R { bytes, pos: 0 }
    }
    fn take(&mut self, n: usize) -> Option<&'a [u8]> {
        let end = self.pos.checked_add(n)?;
        let s = self.bytes.get(self.pos..end)?;
        self.pos = end;
        Some(s)
    }
    fn u8(&mut self) -> Option<u8> {
        Some(self.take(1)?[0])
    }
    fn bool(&mut self) -> Option<bool> {
        match self.u8()? {
            0 => Some(false),
            1 => Some(true),
            _ => None,
        }
    }
    fn u32(&mut self) -> Option<u32> {
        Some(u32::from_le_bytes(self.take(4)?.try_into().ok()?))
    }
    fn i32(&mut self) -> Option<i32> {
        Some(i32::from_le_bytes(self.take(4)?.try_into().ok()?))
    }
    fn u64(&mut self) -> Option<u64> {
        Some(u64::from_le_bytes(self.take(8)?.try_into().ok()?))
    }
    fn i64(&mut self) -> Option<i64> {
        Some(i64::from_le_bytes(self.take(8)?.try_into().ok()?))
    }
    /// A collection count, plausible against the bytes remaining
    /// (every element costs at least `min_each` bytes on the
    /// wire) — a corrupt count dies here instead of allocating
    /// against a fantasy length.
    fn count(&mut self, min_each: usize) -> Option<usize> {
        let n = self.u64()?;
        if n > (self.bytes.len() - self.pos) as u64 / min_each.max(1) as u64 {
            return None;
        }
        Some(n as usize)
    }
    fn fx(&mut self) -> Option<I16F16> {
        Some(I16F16::from_bits(self.i32()?))
    }
    fn opt_u32(&mut self) -> Option<Option<u32>> {
        match self.u8()? {
            0 => Some(None),
            1 => Some(Some(self.u32()?)),
            _ => None,
        }
    }
    fn opt_u64(&mut self) -> Option<Option<u64>> {
        match self.u8()? {
            0 => Some(None),
            1 => Some(Some(self.u64()?)),
            _ => None,
        }
    }
    fn opt_u8(&mut self) -> Option<Option<u8>> {
        match self.u8()? {
            0 => Some(None),
            1 => Some(Some(self.u8()?)),
            _ => None,
        }
    }
    fn opt_point(&mut self) -> Option<Option<(I16F16, I16F16)>> {
        match self.u8()? {
            0 => Some(None),
            1 => Some(Some((self.fx()?, self.fx()?))),
            _ => None,
        }
    }
    fn opt_cell(&mut self) -> Option<Option<(u32, u32)>> {
        match self.u8()? {
            0 => Some(None),
            1 => Some(Some((self.u32()?, self.u32()?))),
            _ => None,
        }
    }
    fn garrison_ref(&mut self) -> Option<Option<GarrisonRef>> {
        match self.u8()? {
            0 => Some(None),
            1 => Some(Some(GarrisonRef::Building(self.u32()?))),
            2 => Some(Some(GarrisonRef::TownCenter(self.u32()?))),
            3 => Some(Some(GarrisonRef::Unit(self.u32()?))),
            _ => None,
        }
    }
    fn dropoff(&mut self) -> Option<Option<Dropoff>> {
        match self.u8()? {
            0 => Some(None),
            1 => Some(Some(Dropoff::TownCenter)),
            2 => Some(Some(Dropoff::Building(self.u32()?))),
            _ => None,
        }
    }
    fn task(&mut self) -> Option<Task> {
        match self.u8()? {
            0 => Some(Task::Idle),
            1 => Some(Task::ToNode),
            2 => Some(Task::Gathering),
            3 => Some(Task::Returning),
            4 => Some(Task::ToBuild),
            5 => Some(Task::Building),
            6 => Some(Task::Repair {
                building: self.u32()?,
            }),
            7 => Some(Task::RepairTc { tc: self.u32()? }),
            _ => None,
        }
    }
    fn queued_order(&mut self) -> Option<QueuedOrder> {
        match self.u8()? {
            0 => Some(QueuedOrder::Move {
                x: self.fx()?,
                y: self.fx()?,
            }),
            1 => Some(QueuedOrder::Gather { node: self.u32()? }),
            2 => Some(QueuedOrder::Attack {
                target: self.u32()?,
            }),
            3 => Some(QueuedOrder::Build {
                kind: BuildingKind::from_tag(self.u8()?)?,
                x: self.fx()?,
                y: self.fx()?,
            }),
            4 => Some(QueuedOrder::Repair {
                building: self.u32()?,
            }),
            5 => Some(QueuedOrder::AttackMove {
                x: self.fx()?,
                y: self.fx()?,
            }),
            6 => Some(QueuedOrder::Patrol {
                x: self.fx()?,
                y: self.fx()?,
            }),
            7 => Some(QueuedOrder::RepairTc { tc: self.u32()? }),
            8 => Some(QueuedOrder::AttackGround {
                x: self.fx()?,
                y: self.fx()?,
            }),
            9 => Some(QueuedOrder::Convert { target: self.u32()? }),
            10 => Some(QueuedOrder::ConvertBuilding {
                building: self.u32()?,
            }),
            // MOV-07: QueuedOrder::FormationMove (tag 11).
            11 => Some(QueuedOrder::FormationMove {
                x: self.fx()?,
                y: self.fx()?,
                formation: FormationKind::from_tag(self.u8()?)?,
            }),
            _ => None,
        }
    }
    fn bell_snapshot(&mut self) -> Option<Option<BellSnapshot>> {
        match self.u8()? {
            0 => Some(None),
            1 => Some(Some(BellSnapshot {
                pos_x: self.fx()?,
                pos_y: self.fx()?,
                task: self.task()?,
                task_node: self.opt_u32()?,
                task_building: self.opt_u32()?,
                bell_tc: self.u32()?,
            })),
            _ => None,
        }
    }
    fn move_order(&mut self) -> Option<Option<MoveOrder>> {
        match self.u8()? {
            0 => Some(None),
            1 => {
                let target_x = self.fx()?;
                let target_y = self.fx()?;
                let arrive = self.fx()?;
                let wp_index = self.u64()? as usize;
                let n = self.count(8)?;
                let mut waypoints = Vec::with_capacity(n);
                for _ in 0..n {
                    waypoints.push((self.fx()?, self.fx()?));
                }
                // MOV-06 (v13): the group stamp, flagged.
                let (group_id, group_cap) = match self.u8()? {
                    0 => (0, I16F16::ZERO),
                    1 => (self.u32()?, self.fx()?),
                    _ => return None,
                };
                Some(Some(MoveOrder {
                    target_x,
                    target_y,
                    waypoints,
                    wp_index,
                    arrive,
                    group_id,
                    group_cap,
                }))
            }
            _ => None,
        }
    }
    fn prod_queue(&mut self) -> Option<Vec<ProdEntry>> {
        let n = self.count(4)?;
        let mut q = Vec::with_capacity(n);
        for _ in 0..n {
            q.push(prod_entry_from_code(self.u32()?)?);
        }
        Some(q)
    }
    fn command(&mut self) -> Option<Command> {
        match self.u8()? {
            0 => Some(Command::Noop),
            1 => Some(Command::Move {
                entity: self.u32()?,
                x: self.fx()?,
                y: self.fx()?,
            }),
            2 => Some(Command::Gather {
                entity: self.u32()?,
                node: self.u32()?,
            }),
            3 => Some(Command::TrainVillager),
            4 => Some(Command::TrainVillagerAt { tc: self.u32()? }),
            5 => Some(Command::Build {
                entity: self.u32()?,
                kind: BuildingKind::from_tag(self.u8()?)?,
                x: self.fx()?,
                y: self.fx()?,
            }),
            6 => Some(Command::BuildLine {
                entity: self.u32()?,
                kind: BuildingKind::from_tag(self.u8()?)?,
                x0: self.fx()?,
                y0: self.fx()?,
                x1: self.fx()?,
                y1: self.fx()?,
            }),
            7 => Some(Command::Attack {
                entity: self.u32()?,
                target: self.u32()?,
            }),
            8 => Some(Command::AttackBuilding {
                entity: self.u32()?,
                building: self.u32()?,
            }),
            9 => Some(Command::TrainAt {
                building: self.u32()?,
                kind: EntityKind::from_tag(self.u8()?)?,
            }),
            10 => Some(Command::Queue {
                entity: self.u32()?,
                order: self.queued_order()?,
            }),
            11 => Some(Command::SetRally {
                building: self.opt_u32()?,
                x: self.fx()?,
                y: self.fx()?,
            }),
            12 => Some(Command::SetRallyTc {
                tc: self.u32()?,
                x: self.fx()?,
                y: self.fx()?,
            }),
            13 => Some(Command::ResearchTc {
                tech: TechId::from_tag(self.u8()?)?,
            }),
            14 => Some(Command::ResearchTcAt {
                tc: self.u32()?,
                tech: TechId::from_tag(self.u8()?)?,
            }),
            15 => Some(Command::ResearchAt {
                building: self.u32()?,
                tech: TechId::from_tag(self.u8()?)?,
            }),
            16 => Some(Command::Repair {
                entity: self.u32()?,
                building: self.u32()?,
            }),
            17 => Some(Command::Trade {
                entity: self.u32()?,
                building: self.u32()?,
            }),
            18 => Some(Command::MarketBuy {
                building: self.u32()?,
                resource: ResourceKind::from_tag(self.u8()?)?,
            }),
            19 => Some(Command::MarketSell {
                building: self.u32()?,
                resource: ResourceKind::from_tag(self.u8()?)?,
            }),
            20 => Some(Command::ForceDeposit {
                entity: self.u32()?,
            }),
            21 => Some(Command::SetAutoExplore {
                entity: self.u32()?,
                enabled: self.bool()?,
            }),
            22 => Some(Command::AttackMove {
                entity: self.u32()?,
                x: self.fx()?,
                y: self.fx()?,
            }),
            23 => Some(Command::Patrol {
                entity: self.u32()?,
                x: self.fx()?,
                y: self.fx()?,
            }),
            24 => Some(Command::SetStance {
                entity: self.u32()?,
                stance: Stance::from_tag(self.u8()?)?,
            }),
            25 => Some(Command::Stop {
                entity: self.u32()?,
            }),
            26 => Some(Command::DeleteEntity {
                entity: self.u32()?,
            }),
            27 => Some(Command::DeleteBuilding {
                building: self.u32()?,
            }),
            28 => Some(Command::CancelQueueTc {
                tc: self.u32()?,
                index: self.u32()?,
            }),
            29 => Some(Command::CancelQueueBuilding {
                building: self.u32()?,
                index: self.u32()?,
            }),
            30 => Some(Command::GarrisonBuilding {
                entity: self.u32()?,
                building: self.u32()?,
            }),
            31 => Some(Command::GarrisonTc {
                entity: self.u32()?,
                tc: self.u32()?,
            }),
            32 => Some(Command::UngarrisonBuilding {
                building: self.u32()?,
            }),
            33 => Some(Command::UngarrisonTc { tc: self.u32()? }),
            34 => Some(Command::TownBell { tc: self.u32()? }),
            35 => Some(Command::GarrisonUnit {
                entity: self.u32()?,
                host: self.u32()?,
            }),
            36 => Some(Command::UngarrisonUnit { host: self.u32()? }),
            37 => Some(Command::Tribute {
                sender: self.u8()?,
                recipient: self.u8()?,
                resource: ResourceKind::from_tag(self.u8()?)?,
                amount: self.i32()?,
            }),
            38 => Some(Command::RepairTc {
                entity: self.u32()?,
                tc: self.u32()?,
            }),
            39 => Some(Command::AttackGround {
                entity: self.u32()?,
                x: self.fx()?,
                y: self.fx()?,
            }),
            40 => Some(Command::QueueReseed {
                building: self.u32()?,
            }),
            41 => Some(Command::DequeueReseed {
                building: self.u32()?,
            }),
            42 => Some(Command::SetAutoReseed {
                building: self.u32()?,
                on: self.bool()?,
            }),
            43 => Some(Command::Heal {
                entity: self.u32()?,
                target: self.u32()?,
            }),
            44 => Some(Command::Convert {
                entity: self.u32()?,
                target: self.u32()?,
            }),
            45 => Some(Command::ConvertBuilding {
                entity: self.u32()?,
                building: self.u32()?,
            }),
            46 => Some(Command::PickupRelic {
                entity: self.u32()?,
                relic: self.u32()?,
            }),
            47 => Some(Command::DepositRelic {
                entity: self.u32()?,
                building: self.u32()?,
            }),
            48 => Some(Command::Resign {
                player: self.u8()?,
            }),
            // MOV-07: Command::FormationMove (tag 49 at landing).
            49 => Some(Command::FormationMove {
                entity: self.u32()?,
                x: self.fx()?,
                y: self.fx()?,
                formation: FormationKind::from_tag(self.u8()?)?,
            }),
            _ => None,
        }
    }
}

/// Inverse of `ProdEntry::code` (unit kinds keep their entity
/// tag below 100; technologies ride at 100 + tech tag).
fn prod_entry_from_code(code: u32) -> Option<ProdEntry> {
    if code < 100 {
        EntityKind::from_tag(code as u8).map(ProdEntry::Unit)
    } else {
        TechId::from_tag((code - 100) as u8).map(ProdEntry::Tech)
    }
}

/// Inverse of `NodeKind::tag` (econ gives the kind a tag but no
/// from-tag; the mapping is the tag's own table).
fn node_kind_from_tag(tag: u8) -> Option<NodeKind> {
    match tag {
        0 => Some(NodeKind::Tree),
        1 => Some(NodeKind::BerryBush),
        2 => Some(NodeKind::GoldMine),
        3 => Some(NodeKind::StoneMine),
        4 => Some(NodeKind::Farm),
        5 => Some(NodeKind::CarcassHunt),
        6 => Some(NodeKind::CarcassHerd),
        // Water W2a (ECO-15): the fish kinds (tags 7/8).
        7 => Some(NodeKind::ShoreFish),
        8 => Some(NodeKind::DeepFish),
        _ => None,
    }
}

/// Inverse of `Age::tag`.
fn age_from_tag(tag: u8) -> Option<Age> {
    match tag {
        0 => Some(Age::Dark),
        1 => Some(Age::Feudal),
        2 => Some(Age::Castle),
        3 => Some(Age::Imperial),
        _ => None,
    }
}

impl Sim {
    /// NET-09: restore a saved game. Returns `None` for anything
    /// that is not an intact save of a known version — wrong
    /// magic, truncation, an unknown tag anywhere in the body,
    /// trailing bytes, or (the decisive check) a restored state
    /// whose SIM-05 hash differs from the file's trailer.
    pub fn load_game(bytes: &[u8]) -> Option<Sim> {
        let mut r = R::new(bytes);
        if r.take(4)? != MAGIC {
            return None;
        }
        if r.u32()? != SAVE_VERSION {
            return None;
        }
        let tick = r.u64()?;
        let rng = Pcg32::from_words(r.u64()?, r.u64()?);
        let mode = MatchMode::from_tag(r.u8()?)?;
        // Format v11: the victory settings (Package C), packed
        // exactly as `save_game` wrote them (flags: bit 0
        // Regicide, bit 1 Sudden Death — the WASM ABI packing).
        // Format v12: bit 2 is VIC-03's relic victory, and its
        // countdown state follows when the bit is on.
        // Format v14: bits 3/4/5 are Package I's Death Match /
        // Empire Wars / King of the Hill, and the KotH countdown
        // state follows VIC-03's block when bit 5 is on.
        // Format v16: bit 6 is VIC-02's wonder victory, and its
        // countdown state follows VIC-03's block when bit 6 is
        // on (before the KotH block, exactly as written).
        let victory = {
            let time_limit_ticks = r.u64()?;
            let score_target = r.u64()? as i64;
            let flags = r.u8()?;
            VictorySettings {
                time_limit_ticks,
                score_target,
                regicide: flags & 1 != 0,
                sudden_death: flags & 2 != 0,
                relic_victory: flags & 4 != 0,
                death_match: flags & 8 != 0,
                empire_wars: flags & 16 != 0,
                king_of_the_hill: flags & 32 != 0,
                wonder_victory: flags & 64 != 0,
                // Format v17 (Teams T2): bit 7 is VIC-12's
                // allied victory; no layout change (see the
                // version-17 note at SAVE_VERSION).
                allied_victory: flags & 128 != 0,
            }
        };
        // Format v17: Teams T1 — the presence byte written in
        // every v17 save, then the team block while present
        // (see the version-17 note at SAVE_VERSION).
        let teams = if r.bool()? {
            TeamSettings {
                teams: [r.u8()?, r.u8()?],
            }
        } else {
            TeamSettings::default()
        };
        let (relic_holder, relic_hold_ticks) = if victory.relic_victory {
            let holder = if r.bool()? { Some(r.u8()?) } else { None };
            (holder, r.u64()?)
        } else {
            (None, 0)
        };
        let (wonder_holder, wonder_hold_ticks) = if victory.wonder_victory {
            let holder = if r.bool()? { Some(r.u8()?) } else { None };
            (holder, r.u64()?)
        } else {
            (None, 0)
        };
        let (koth_ticks_left, koth_winner) = if victory.king_of_the_hill {
            let left = r.u64()?;
            let winner = if r.bool()? { Some(r.u8()?) } else { None };
            (left, winner)
        } else {
            (crate::KOTH_COUNTDOWN_TICKS, None)
        };
        // World: the grid is the fixed rulebook size; the stored
        // dimensions must agree before any layer is read.
        if r.u32()? != GRID_DIM || r.u32()? != GRID_DIM {
            return None;
        }
        let cells = (GRID_DIM * GRID_DIM) as usize;
        let terrain = r.take(cells)?.to_vec();
        let heights = r.take(cells)?.to_vec();
        let node_block = r.take(cells)?.to_vec();
        let dynamic = r.take(cells)?.to_vec();
        let mut grid = Grid::new();
        for y in 0..GRID_DIM {
            for x in 0..GRID_DIM {
                let i = (y * GRID_DIM + x) as usize;
                grid.set_terrain(x, y, terrain[i]);
                grid.set_height(x, y, heights[i]);
                grid.set_node_block(x, y, node_block[i] != 0);
                grid.set_dynamic(x, y, dynamic[i] != 0);
            }
        }
        let mut players = [PlayerState::default(), PlayerState::default()];
        for p in &mut players {
            p.age = age_from_tag(r.u8()?)?;
            p.techs = r.u64()?;
            p.destroyed_value = r.u64()?;
            p.units_killed = r.u32()?;
            p.units_lost = r.u32()?;
            p.buildings_razed = r.u32()?;
            p.buildings_lost = r.u32()?;
            for b in &mut p.gathered_bits {
                *b = r.u64()? as i64;
            }
            p.reseed_queue = r.u32()?;
            p.auto_reseed = r.bool()?;
            p.mode_out = r.bool()?;
            p.resigned = r.bool()?;
        }
        let mut stockpiles = [Stockpile::starting(), Stockpile::starting()];
        for s in &mut stockpiles {
            let mut amounts = [I16F16::ZERO; 4];
            for a in &mut amounts {
                *a = r.fx()?;
            }
            *s = Stockpile::from_amounts(amounts);
        }
        let market = Market::from_rates([r.i32()?, r.i32()?, r.i32()?]);
        let n_nodes = r.count(25)?;
        let mut nodes = Vec::with_capacity(n_nodes);
        for _ in 0..n_nodes {
            nodes.push(ResourceNode {
                id: r.u32()?,
                kind: node_kind_from_tag(r.u8()?)?,
                pos_x: r.fx()?,
                pos_y: r.fx()?,
                amount_left: r.fx()?,
                full_amount: r.fx()?,
                decay_per_tick: r.fx()?,
            });
        }
        let n_tc = r.count(30)?;
        let mut tc_states = Vec::with_capacity(n_tc);
        for _ in 0..n_tc {
            let tc_id = r.u32()?;
            let queue = r.prod_queue()?;
            let progress = r.u64()?;
            let rally = r.opt_point()?;
            let rally_node = r.opt_u32()?;
            let n_g = r.count(4)?;
            let mut garrison = Vec::with_capacity(n_g);
            for _ in 0..n_g {
                garrison.push(r.u32()?);
            }
            tc_states.push(TcState {
                tc_id,
                queue,
                progress,
                rally,
                rally_node,
                garrison,
                garrison_arrows: r.u32()?,
                bell_active: r.bool()?,
                weapon_target: r.opt_u32()?,
                weapon_target_building: r.opt_u32()?,
                weapon_strike_tick: r.opt_u64()?,
                weapon_next_tick: r.u64()?,
                weapon_engaged: r.bool()?,
                repair_hp_remainder: r.i64()?,
                repair_cost_remainder: r.i64()?,
            });
        }
        let next_building_id = r.u32()?;
        // MOV-06 (v13): the group-id allocator.
        let next_group_id = r.u32()?;
        let n_b = r.count(60)?;
        let mut buildings = Vec::with_capacity(n_b);
        for _ in 0..n_b {
            let id = r.u32()?;
            let kind = BuildingKind::from_tag(r.u8()?)?;
            let owner = r.u8()?;
            let hp = r.fx()?;
            let cell_x = r.u32()?;
            let cell_y = r.u32()?;
            let pos_x = r.fx()?;
            let pos_y = r.fx()?;
            let progress_thirds = r.u64()?;
            let repair_hp_remainder = r.i64()?;
            let repair_cost_remainder = r.i64()?;
            let repair_stone_remainder = r.i64()?;
            let repair_gold_remainder = r.i64()?;
            let build_rate_remainder = r.i64()?;
            let queue_rate_remainder = r.i64()?;
            let complete = r.bool()?;
            let node_id = r.opt_u32()?;
            let queue = r.prod_queue()?;
            let train_progress = r.u64()?;
            let rally = r.opt_point()?;
            let rally_node = r.opt_u32()?;
            let weapon_target = r.opt_u32()?;
            let weapon_target_building = r.opt_u32()?;
            let weapon_strike_tick = r.opt_u64()?;
            let weapon_next_tick = r.u64()?;
            let weapon_engaged = r.bool()?;
            let gate_along_x = r.bool()?;
            let n_g = r.count(4)?;
            let mut garrison = Vec::with_capacity(n_g);
            for _ in 0..n_g {
                garrison.push(r.u32()?);
            }
            let garrison_arrows = r.u32()?;
            let n_r = r.count(8)?;
            let mut relics = Vec::with_capacity(n_r);
            for _ in 0..n_r {
                relics.push(r.u32()?);
            }
            let relic_gold_remainder = r.i64()?;
            buildings.push(Building {
                id,
                kind,
                owner,
                hp,
                cell_x,
                cell_y,
                pos_x,
                pos_y,
                progress_thirds,
                repair_hp_remainder,
                repair_cost_remainder,
                repair_stone_remainder,
                repair_gold_remainder,
                build_rate_remainder,
                queue_rate_remainder,
                complete,
                node_id,
                queue,
                train_progress,
                rally,
                rally_node,
                weapon_target,
                weapon_target_building,
                weapon_strike_tick,
                weapon_next_tick,
                weapon_engaged,
                gate_along_x,
                garrison,
                garrison_arrows,
                relics,
                relic_gold_remainder,
            });
        }
        let n_e = r.count(80)?;
        let mut entities = Vec::with_capacity(n_e);
        for _ in 0..n_e {
            let id = r.u32()?;
            let kind = EntityKind::from_tag(r.u8()?)?;
            let owner = r.u8()?;
            let pos_x = r.fx()?;
            let pos_y = r.fx()?;
            let vel_x = r.fx()?;
            let vel_y = r.fx()?;
            let hp = r.fx()?;
            let dead = r.bool()?;
            let order = r.move_order()?;
            let n_q = r.count(9)?;
            let mut order_queue = Vec::with_capacity(n_q);
            for _ in 0..n_q {
                order_queue.push(r.queued_order()?);
            }
            let order_from_queue = r.bool()?;
            let task = r.task()?;
            let task_node = r.opt_u32()?;
            let task_building = r.opt_u32()?;
            let task_dropoff = r.dropoff()?;
            let carry = r.fx()?;
            let carry_kind = match r.u8()? {
                0 => None,
                1 => Some(ResourceKind::from_tag(r.u8()?)?),
                _ => return None,
            };
            let attack_target = r.opt_u32()?;
            let attack_building = r.opt_u32()?;
            let attack_point = r.opt_point()?;
            let strike_tick = r.opt_u64()?;
            let next_attack_tick = r.u64()?;
            let attack_engaged = r.bool()?;
            let last_hit_by = r.opt_u32()?;
            let last_hit_owner = r.opt_u8()?;
            let suppress_separation = r.bool()?;
            let home_x = r.fx()?;
            let home_y = r.fx()?;
            let aggro_target = r.opt_u32()?;
            let hits_taken = r.u32()?;
            let aggro_locked = r.bool()?;
            let returning_home = r.bool()?;
            let flee_until = r.u64()?;
            let flee_from_x = r.fx()?;
            let flee_from_y = r.fx()?;
            let auto_explore = r.bool()?;
            let auto_target = r.opt_cell()?;
            let wander_until = r.u64()?;
            let stance = Stance::from_tag(r.u8()?)?;
            let stance_anchor_x = r.fx()?;
            let stance_anchor_y = r.fx()?;
            let attack_auto = r.bool()?;
            let attack_move = r.opt_point()?;
            let n_p = r.count(8)?;
            let mut patrol = Vec::with_capacity(n_p);
            for _ in 0..n_p {
                patrol.push((r.fx()?, r.fx()?));
            }
            let patrol_next = r.u64()? as usize;
            let garrisoned_in = r.garrison_ref()?;
            let garrison_target = r.garrison_ref()?;
            let garrison_prod_entry = r.bool()?;
            let bell_snapshot = r.bell_snapshot()?;
            let garrison_heal_remainder = r.i64()?;
            let trade_home = r.opt_u32()?;
            let trade_target = r.opt_u32()?;
            let trade_loaded = r.bool()?;
            let heal_target = r.opt_u32()?;
            let heal_remainder = r.i64()?;
            let faith = r.i32()?;
            let faith_remainder = r.i64()?;
            let conversion_target = r.opt_u32()?;
            let ci_progress = r.u32()?;
            let ci_ticks = r.u32()?;
            let conversion_level = r.u8()?;
            let conversion_start_shift = r.i32()?;
            let conversion_guarantee_shift = r.i32()?;
            let frozen_patches = match r.u8()? {
                0 => None,
                1 => Some(r.u64()?),
                _ => return None,
            };
            let conversion_target_building = r.opt_u32()?;
            let n_g = r.count(4)?;
            let mut garrison = Vec::with_capacity(n_g);
            for _ in 0..n_g {
                garrison.push(r.u32()?);
            }
            let carrying_relic = r.opt_u32()?;
            let relic_pickup_target = r.opt_u32()?;
            let relic_deposit_target = r.opt_u32()?;
            let relic_carried_by = r.opt_u32()?;
            let relic_garrisoned_in = r.opt_u32()?;
            entities.push(Entity {
                id,
                kind,
                pos_x,
                pos_y,
                vel_x,
                vel_y,
                order,
                order_queue,
                order_from_queue,
                task,
                task_node,
                task_building,
                task_dropoff,
                carry,
                carry_kind,
                owner,
                hp,
                dead,
                attack_target,
                attack_building,
                attack_point,
                strike_tick,
                next_attack_tick,
                attack_engaged,
                last_hit_by,
                last_hit_owner,
                suppress_separation,
                home_x,
                home_y,
                aggro_target,
                hits_taken,
                aggro_locked,
                returning_home,
                flee_until,
                flee_from_x,
                flee_from_y,
                auto_explore,
                auto_target,
                wander_until,
                stance,
                stance_anchor_x,
                stance_anchor_y,
                attack_auto,
                attack_move,
                patrol,
                patrol_next,
                garrisoned_in,
                garrison_target,
                garrison_prod_entry,
                bell_snapshot,
                garrison_heal_remainder,
                trade_home,
                trade_target,
                trade_loaded,
                heal_target,
                heal_remainder,
                faith,
                faith_remainder,
                conversion_target,
                conversion_target_building,
                ci_progress,
                ci_ticks,
                conversion_level,
                conversion_start_shift,
                conversion_guarantee_shift,
                frozen_patches,
                carrying_relic,
                relic_pickup_target,
                relic_deposit_target,
                relic_carried_by,
                relic_garrisoned_in,
                garrison,
            });
        }
        let n_p = r.count(50)?;
        let mut projectiles = Vec::with_capacity(n_p);
        for _ in 0..n_p {
            let spawn_x = r.fx()?;
            let spawn_y = r.fx()?;
            let impact_x = r.fx()?;
            let impact_y = r.fx()?;
            let spawn_tick = r.u64()?;
            let impact_tick = r.u64()?;
            let intended = r.opt_u32()?;
            let intended_building = r.opt_u32()?;
            let attacker = r.u32()?;
            let attacker_building = r.opt_u32()?;
            let owner = r.u8()?;
            let attacker_elev = r.u8()?;
            let n_a = r.count(5)?;
            let mut attacks = Vec::with_capacity(n_a);
            for _ in 0..n_a {
                attacks.push((r.u8()?, r.i32()?));
            }
            projectiles.push(Projectile {
                spawn_x,
                spawn_y,
                impact_x,
                impact_y,
                spawn_tick,
                impact_tick,
                intended,
                intended_building,
                attacker,
                attacker_building,
                owner,
                attacker_elev,
                attacks,
            });
        }
        let next_wave_tick = r.u64()?;
        let fog0 = r.take(cells)?.to_vec();
        let fog1 = r.take(cells)?.to_vec();
        let fog = [fog0, fog1];
        // The visible-cell lists are bookkeeping for the fog
        // grids: rebuild them from the grids themselves (cells
        // currently at visibility 2), exactly as the field's own
        // contract states.
        let fog_visible: [Vec<u32>; 2] = [
            fog[0]
                .iter()
                .enumerate()
                .filter(|(_, &v)| v == 2)
                .map(|(i, _)| i as u32)
                .collect(),
            fog[1]
                .iter()
                .enumerate()
                .filter(|(_, &v)| v == 2)
                .map(|(i, _)| i as u32)
                .collect(),
        ];
        let n_c = r.count(5)?;
        let mut commands = Vec::with_capacity(n_c);
        for _ in 0..n_c {
            commands.push(r.command()?);
        }
        let stored_hash = r.u64()?;
        if r.pos != r.bytes.len() {
            return None; // trailing bytes: not one of our saves
        }
        let sim = Sim {
            tick,
            rng,
            grid,
            mode,
            victory,
            teams,
            relic_holder,
            relic_hold_ticks,
            wonder_holder,
            wonder_hold_ticks,
            koth_ticks_left,
            koth_winner,
            entities,
            commands,
            stockpiles,
            market,
            players,
            nodes,
            tc_states,
            buildings,
            next_building_id,
            next_group_id,
            group_session: Vec::new(),
            formation_session: Vec::new(),
            projectiles,
            ai: crate::ai::AiState { next_wave_tick },
            fog,
            fog_visible,
        };
        // The decisive verification (NET-09): the restored state
        // must hash to the trailer — the snapshot verifies itself.
        if sim.state_hash() != stored_hash {
            return None;
        }
        Some(sim)
    }
}
