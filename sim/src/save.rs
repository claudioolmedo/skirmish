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
use crate::{Command, MatchMode, PlayerState, Sim, TcState};

/// File magic: "SKSV" (Skirmish Save).
const MAGIC: &[u8; 4] = b"SKSV";
/// Save format version. Bump on any layout change; loaders
/// refuse versions they do not know.
const SAVE_VERSION: u32 = 1;

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
    }
    fn queued_order(&mut self, v: &QueuedOrder) {
        self.u8(v.tag());
        match *v {
            QueuedOrder::Move { x, y }
            | QueuedOrder::AttackMove { x, y }
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
        }
        // --- buildings ---
        w.u32(self.next_building_id);
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
            w.i64(b.build_rate_remainder);
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
            w.usize(b.garrison.len());
            for id in &b.garrison {
                w.u32(*id);
            }
            w.u32(b.garrison_arrows);
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
            w.usize(e.garrison.len());
            for id in &e.garrison {
                w.u32(*id);
            }
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
                Some(Some(MoveOrder {
                    target_x,
                    target_y,
                    waypoints,
                    wp_index,
                    arrive,
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
        let n_nodes = r.count(21)?;
        let mut nodes = Vec::with_capacity(n_nodes);
        for _ in 0..n_nodes {
            nodes.push(ResourceNode {
                id: r.u32()?,
                kind: node_kind_from_tag(r.u8()?)?,
                pos_x: r.fx()?,
                pos_y: r.fx()?,
                amount_left: r.fx()?,
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
            });
        }
        let next_building_id = r.u32()?;
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
            let build_rate_remainder = r.i64()?;
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
            let n_g = r.count(4)?;
            let mut garrison = Vec::with_capacity(n_g);
            for _ in 0..n_g {
                garrison.push(r.u32()?);
            }
            let garrison_arrows = r.u32()?;
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
                build_rate_remainder,
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
                garrison,
                garrison_arrows,
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
            let n_g = r.count(4)?;
            let mut garrison = Vec::with_capacity(n_g);
            for _ in 0..n_g {
                garrison.push(r.u32()?);
            }
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
            entities,
            commands,
            stockpiles,
            market,
            players,
            nodes,
            tc_states,
            buildings,
            next_building_id,
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
