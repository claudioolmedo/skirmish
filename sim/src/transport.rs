//! Transport Ship core (Water W4, GAR-09): land units load
//! onto an own Transport Ship at a shoreline, ride immune and
//! inert, and unload at another shoreline — and if the ship
//! sinks, everything aboard dies with it.
//!
//! The cargo model is deliberately NOT the garrison core's
//! fourth host kind (WATER-EXECUTION-PLAN §2/§3.4): reusing
//! `GarrisonRef::Unit` (the Ram) would import ejection-on-death
//! — the one behavior GAR-09 forbids. Instead the ship entity
//! carries its own cargo list (`Entity::transport_cargo`), the
//! cargo unit carries the back-pointer (`transported_in`), and
//! containment is the generalized predicate
//! `Entity::is_contained` (garrisoned OR transported), so every
//! spatial iteration, targeting scan, heal/convert eligibility
//! check and idle count treats cargo exactly as it treats a
//! garrison (GAR-02 by construction): cargo cannot act, cannot
//! be damaged, and is NOT healed aboard (no shelter-healing
//! path reads the transport fields at all — the rate is zero
//! by absence, not by a borrowed constant).
//!
//! Death routing is inverted (the anti-GAR-03): the death
//! sweep in `combat.rs`, on a Transport Ship's death, zeroes
//! every cargo unit's HP in cargo-list order instead of
//! ejecting anyone; the sweep then processes each cargo death
//! through the ordinary path. There is no ≤20%-HP ejection
//! and no destruction ejection for transports, ever.

use fixed::types::I16F16;

use crate::entity::{EntityKind, Task};
use crate::grid::{Domain, GRID_DIM};
use crate::Sim;

/// GAR-09: a Transport Ship carries at most 5 land units
/// (WATER-SPEC §6 — wiki, Transport Ship changelog: "Transport
/// Ships have a garrison capacity of 5"; the "5 → 20"
/// progression is Chronicles-mode content, not standard DE).
pub const TRANSPORT_CAPACITY: u32 = 5;

/// Boarding geometry (implementation constant, recorded as
/// such): a unit boards when its own cell is a land-side cell
/// (land or shallows — never deep water) at Chebyshev distance
/// ≤ 1 from the ship's cell, the ship's cell being water-side.
/// Both tests read LIVE positions every tick (the Ram's
/// moving-host lesson, Castle P4a): a ship that moved between
/// the load order and the arrival still loads.

impl Sim {
    /// Whether `entity` may load onto Transport Ship `ship`
    /// right now (GAR-09 eligibility-as-data): the unit is a
    /// living, player-owned, mobile LAND-domain unit, not
    /// already contained; the ship is a living own Transport
    /// Ship with room aboard. Domain is the eligibility test —
    /// ships cannot board (a Transport cannot carry a Galley),
    /// buildings cannot board (the Town Center is not mobile),
    /// and any land kind may: infantry, cavalry, siege,
    /// villagers, monks ("transports carry anyone", GAR-09).
    /// The Relic/King edge cases follow this generic rule —
    /// no fetched source singles them out (UNVERIFIED, recorded
    /// in the W4 handoff): a King is a mobile land unit and
    /// boards; a Relic is not mobile and never does.
    pub fn can_load_transport(&self, entity: u32, ship: u32) -> bool {
        let ei = match self.entity_index(entity) {
            Some(i) => i,
            None => return false,
        };
        let si = match self.entity_index(ship) {
            Some(i) => i,
            None => return false,
        };
        if entity == ship {
            return false;
        }
        let e = &self.entities[ei];
        let s = &self.entities[si];
        if e.dead || s.dead || s.kind != EntityKind::TransportShip {
            return false;
        }
        if e.owner != s.owner || !crate::entity::is_player_owner(e.owner) {
            return false;
        }
        if !e.kind.is_mobile() || e.kind.domain() != Domain::Land {
            return false;
        }
        if e.is_contained() {
            return false;
        }
        (s.transport_cargo.len() as u32) < TRANSPORT_CAPACITY
    }

    /// The number of units currently aboard Transport Ship
    /// `ship` (0 for any other entity / unknown id).
    pub fn transport_cargo_count(&self, ship: u32) -> u32 {
        match self.entity_index(ship) {
            Some(si) if self.entities[si].kind == EntityKind::TransportShip => {
                self.entities[si].transport_cargo.len() as u32
            }
            _ => 0,
        }
    }

    /// Whether Transport Ship `ship` is beside land right now:
    /// its cell is water-side and at least one Chebyshev-adjacent
    /// cell is land-side standable ground (land or shallows,
    /// not blocked for the land domain) — the shoreline an
    /// unload needs.
    fn transport_at_shore(&self, ship: u32) -> bool {
        !self.transport_exit_points(ship).is_empty()
    }

    /// Free land-side cells beside Transport Ship `ship`, in a
    /// deterministic expanding-ring order (Chebyshev radius 1,
    /// then 2, then 3; row-major inside each ring): land or
    /// shallows cells the land domain does not read as blocked.
    /// Unload places cargo on these in cargo-list order; an
    /// empty result means the cargo holds (no teleport, no
    /// loss — the plan §5 pin).
    fn transport_exit_points(&self, ship: u32) -> Vec<(u32, u32)> {
        let si = match self.entity_index(ship) {
            Some(i) => i,
            None => return Vec::new(),
        };
        let s = &self.entities[si];
        if s.dead || s.kind != EntityKind::TransportShip {
            return Vec::new();
        }
        let (scx, scy) = self.grid.world_to_cell(s.pos_x, s.pos_y);
        if !self.grid.is_water_side(scx, scy) {
            return Vec::new();
        }
        let mut out = Vec::new();
        for r in 1..=3i32 {
            for dy in -r..=r {
                for dx in -r..=r {
                    if dx.abs() != r && dy.abs() != r {
                        continue; // this ring only
                    }
                    let (cx, cy) = (scx as i32 + dx, scy as i32 + dy);
                    if cx < 0 || cy < 0 || cx >= GRID_DIM as i32 || cy >= GRID_DIM as i32 {
                        continue;
                    }
                    let (cx, cy) = (cx as u32, cy as u32);
                    // Land-side standable: not deep water (land
                    // units may stand in shallows — the
                    // amphibious reading, MAP-08 as amended),
                    // and not blocked for the land domain
                    // (buildings, trees and cliffs refuse).
                    if self.grid.is_water(cx, cy) {
                        continue;
                    }
                    if self.grid.is_blocked_for(Domain::Land, cx, cy) {
                        continue;
                    }
                    out.push((cx, cy));
                }
            }
        }
        out
    }

    /// The shore standing point a loading unit walks to: a
    /// land-side cell Chebyshev-adjacent to the ship's CURRENT
    /// cell, reachable on the land domain; failing adjacency
    /// (the ship is mid-water), the nearest reachable shore cell
    /// within an expanding search (the unit walks to the shore
    /// and waits — the transport tick re-approaches as the ship
    /// moves, exactly the Ram's live-geometry discipline).
    fn transport_approach_point(
        &self,
        idx: usize,
        ship: u32,
    ) -> Option<(I16F16, I16F16)> {
        let si = self.entity_index(ship)?;
        let s = &self.entities[si];
        let (scx, scy) = self.grid.world_to_cell(s.pos_x, s.pos_y);
        let (px, py) = (self.entities[idx].pos_x, self.entities[idx].pos_y);
        let standable = |cx: u32, cy: u32| {
            !self.grid.is_water(cx, cy)
                && !self.grid.is_blocked_for(Domain::Land, cx, cy)
        };
        // Adjacent cells first, fixed order.
        for dy in -1..=1i32 {
            for dx in -1..=1i32 {
                if dx == 0 && dy == 0 {
                    continue;
                }
                let (cx, cy) = (scx as i32 + dx, scy as i32 + dy);
                if cx < 0 || cy < 0 || cx >= GRID_DIM as i32 || cy >= GRID_DIM as i32 {
                    continue;
                }
                let (cx, cy) = (cx as u32, cy as u32);
                if standable(cx, cy) {
                    let p = self.grid.cell_center(cx, cy);
                    if self.grid.find_path_for(Domain::Land, (px, py), p).is_some() {
                        return Some(p);
                    }
                }
            }
        }
        // Wider search: the nearest shore cell (land-side,
        // touching water) reachable from the unit.
        for r in 2..=8i32 {
            for dy in -r..=r {
                for dx in -r..=r {
                    if dx.abs() != r && dy.abs() != r {
                        continue;
                    }
                    let (cx, cy) = (scx as i32 + dx, scy as i32 + dy);
                    if cx < 0 || cy < 0 || cx >= GRID_DIM as i32 || cy >= GRID_DIM as i32 {
                        continue;
                    }
                    let (cx, cy) = (cx as u32, cy as u32);
                    if standable(cx, cy) && self.grid.is_shore(cx, cy) {
                        let p = self.grid.cell_center(cx, cy);
                        if self.grid.find_path_for(Domain::Land, (px, py), p).is_some() {
                            return Some(p);
                        }
                    }
                }
            }
        }
        None
    }

    /// Whether the unit at `idx` stands in boarding position for
    /// Transport Ship `ship` (live geometry, read every call):
    /// its cell is land-side and Chebyshev-adjacent to the
    /// ship's cell, and the ship's cell is water-side.
    fn transport_board_ready(&self, idx: usize, ship: u32) -> bool {
        let si = match self.entity_index(ship) {
            Some(i) => i,
            None => return false,
        };
        let s = &self.entities[si];
        if s.dead || s.kind != EntityKind::TransportShip {
            return false;
        }
        let e = &self.entities[idx];
        let (ux, uy) = self.grid.world_to_cell(e.pos_x, e.pos_y);
        let (sx, sy) = self.grid.world_to_cell(s.pos_x, s.pos_y);
        if self.grid.is_water(ux, uy) {
            return false; // a land unit never boards from deep water
        }
        if !self.grid.is_water_side(sx, sy) {
            return false;
        }
        (ux as i32 - sx as i32).abs() <= 1 && (uy as i32 - sy as i32).abs() <= 1
    }

    /// The mechanics of a load order (the GarrisonUnit-shaped
    /// pair's first half): cancel the unit's current work and
    /// walk it to the shore beside the ship; the transport tick
    /// boards it on arrival.
    pub(crate) fn apply_load_transport(&mut self, idx: usize, ship: u32) -> bool {
        let entity = self.entities[idx].id;
        if !self.can_load_transport(entity, ship) {
            return false;
        }
        {
            let e = &mut self.entities[idx];
            e.order_queue.clear();
            e.order_from_queue = false;
            e.task = Task::Idle;
            e.task_node = None;
            e.task_building = None;
            e.task_dropoff = None;
            e.clear_combat();
            e.auto_explore = false;
            e.auto_target = None;
            e.attack_move = None;
            e.patrol.clear();
            e.patrol_next = 0;
            e.garrison_target = None;
            e.garrison_prod_entry = false;
            e.transport_target = Some(ship);
        }
        if !self.transport_board_ready(idx, ship) {
            if let Some((gx, gy)) = self.transport_approach_point(idx, ship) {
                self.set_move_order(idx, gx, gy);
            }
        }
        true
    }

    pub(crate) fn issue_load_transport(&mut self, entity: u32, ship: u32) {
        if let Some(idx) = self.entity_index(entity) {
            self.apply_load_transport(idx, ship);
        }
    }

    /// Order Transport Ship `ship` to unload (GAR-09): if it
    /// stands beside a shore it unloads on this tick's transport
    /// phase; otherwise the pending flag sails it to the nearest
    /// water-side shore cell first (the ship ordered to a shore
    /// point paths on the water domain — plan §3.4). An empty
    /// ship ignores the order.
    pub(crate) fn issue_unload_transport(&mut self, ship: u32) {
        let si = match self.entity_index(ship) {
            Some(i) => i,
            None => return,
        };
        let s = &self.entities[si];
        if s.dead || s.kind != EntityKind::TransportShip || s.transport_cargo.is_empty() {
            return;
        }
        self.entities[si].transport_unload_pending = true;
    }

    /// Place every unit aboard `ship` on free shore cells beside
    /// it, in cargo-list order, idle (determinism). Returns how
    /// many units were placed; when no free shore cell exists,
    /// nothing is placed and the cargo holds whole.
    fn unload_transport_now(&mut self, ship: u32) -> usize {
        let points = self.transport_exit_points(ship);
        if points.is_empty() {
            return 0;
        }
        let si = match self.entity_index(ship) {
            Some(i) => i,
            None => return 0,
        };
        let cargo = std::mem::take(&mut self.entities[si].transport_cargo);
        let placed = cargo.len();
        for (slot, id) in cargo.iter().enumerate() {
            if let Some(idx) = self.entity_index(*id) {
                if self.entities[idx].dead {
                    self.entities[idx].transported_in = None;
                    continue;
                }
                let (cx, cy) = points[slot % points.len()];
                let (px, py) = self.grid.cell_center(cx, cy);
                let e = &mut self.entities[idx];
                e.pos_x = px;
                e.pos_y = py;
                e.transported_in = None;
                e.transport_target = None;
                e.order = None;
                e.vel_x = I16F16::ZERO;
                e.vel_y = I16F16::ZERO;
                e.task = Task::Idle;
                e.task_node = None;
                e.task_building = None;
                e.task_dropoff = None;
                e.clear_combat();
            }
        }
        placed
    }

    /// Per-tick transport work, in fixed order (entity-id sweep):
    /// pending unloads (sail to shore, then unload), then load
    /// arrivals board, re-approaching a moved ship when a
    /// walker's path has run out. Deterministic by construction.
    pub(crate) fn transport_tick(&mut self) {
        // Unloads first: a ship that unloads frees its capacity
        // before this tick's boarding checks read it.
        let pending: Vec<u32> = self
            .entities
            .iter()
            .filter(|e| {
                e.kind == EntityKind::TransportShip && !e.dead && e.transport_unload_pending
            })
            .map(|e| e.id)
            .collect();
        for ship in pending {
            if self.transport_at_shore(ship) {
                let n = self.unload_transport_now(ship);
                if let Some(si) = self.entity_index(ship) {
                    if n > 0 || self.entities[si].transport_cargo.is_empty() {
                        self.entities[si].transport_unload_pending = false;
                    }
                }
            } else {
                // Sail to the nearest water-side shore cell: a
                // water cell Chebyshev-adjacent to land, nearest
                // by cell distance, fixed order on ties. Only
                // re-issue when the ship stands idle (a player
                // move in progress owns the helm; the unload
                // completes when that move brings it to a shore).
                let si = match self.entity_index(ship) {
                    Some(i) => i,
                    None => continue,
                };
                if self.entities[si].order.is_none() {
                    if let Some(p) = self.nearest_shore_water_point(ship) {
                        self.set_move_order(si, p.0, p.1);
                    } else if let Some(si2) = self.entity_index(ship) {
                        // No shore exists on this water at all:
                        // the cargo holds; the order ends.
                        self.entities[si2].transport_unload_pending = false;
                    }
                }
            }
        }
        // Load arrivals.
        for idx in 0..self.entities.len() {
            let ship = match self.entities[idx].transport_target {
                Some(s) => s,
                None => continue,
            };
            if self.entities[idx].dead || self.entities[idx].is_contained() {
                self.entities[idx].transport_target = None;
                continue;
            }
            let entity = self.entities[idx].id;
            if !self.can_load_transport(entity, ship) {
                // Ship sank, filled, or was deleted on the way:
                // the order ends and the unit stands idle.
                let e = &mut self.entities[idx];
                e.transport_target = None;
                e.order = None;
                e.vel_x = I16F16::ZERO;
                e.vel_y = I16F16::ZERO;
                continue;
            }
            if self.transport_board_ready(idx, ship) {
                // Board: the unit leaves the field into the
                // ship's cargo list (boarding order preserved).
                if let Some(si) = self.entity_index(ship) {
                    self.entities[si].transport_cargo.push(entity);
                }
                let e = &mut self.entities[idx];
                e.transported_in = Some(ship);
                e.transport_target = None;
                e.order = None;
                e.vel_x = I16F16::ZERO;
                e.vel_y = I16F16::ZERO;
                e.task = Task::Idle;
                e.task_node = None;
                e.task_building = None;
                e.task_dropoff = None;
                e.clear_combat();
            } else if self.entities[idx].order.is_none() {
                // The ship moved while the unit walked (or its
                // path failed): re-approach from the ship's
                // CURRENT position instead of standing forever
                // beside where it used to be (the Ram rule).
                if let Some((gx, gy)) = self.transport_approach_point(idx, ship) {
                    self.set_move_order(idx, gx, gy);
                }
            }
        }
    }

    /// The nearest water-side cell adjacent to land, as a world
    /// point, for Transport Ship `ship`'s sail-to-shore leg:
    /// expanding Chebyshev rings around the ship's cell, the
    /// first cell that is water-side, touches land, and is
    /// reachable on the water domain.
    fn nearest_shore_water_point(&self, ship: u32) -> Option<(I16F16, I16F16)> {
        let si = self.entity_index(ship)?;
        let s = &self.entities[si];
        let (scx, scy) = self.grid.world_to_cell(s.pos_x, s.pos_y);
        let (px, py) = (s.pos_x, s.pos_y);
        for r in 1..=16i32 {
            for dy in -r..=r {
                for dx in -r..=r {
                    if dx.abs() != r && dy.abs() != r {
                        continue;
                    }
                    let (cx, cy) = (scx as i32 + dx, scy as i32 + dy);
                    if cx < 0 || cy < 0 || cx >= GRID_DIM as i32 || cy >= GRID_DIM as i32 {
                        continue;
                    }
                    let (cx, cy) = (cx as u32, cy as u32);
                    if !self.grid.is_water_side(cx, cy) || !self.grid.is_shore(cx, cy) {
                        continue;
                    }
                    if self.grid.is_blocked_for(Domain::Water, cx, cy) {
                        continue;
                    }
                    let p = self.grid.cell_center(cx, cy);
                    if self.grid.find_path_for(Domain::Water, (px, py), p).is_some() {
                        return Some(p);
                    }
                }
            }
        }
        None
    }
}
