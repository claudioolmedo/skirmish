//! Garrison core (Castle P3b, GAR-01..08): garrison state and
//! orders for defensive buildings (Town Center, towers, Castle),
//! production garrison (GAR-08), ejection (GAR-03), shelter
//! healing (GAR-04 base), arrow scaling (GAR-05), the Town
//! Center's garrison-only weapon (GAR-06 — its firing step lives
//! in `combat.rs` beside the placed buildings'), and the Town
//! Bell (GAR-07).
//!
//! Eligibility is the resolved per-building data column (scope
//! note 2026-10-29): Town Center 15 foot-only; towers 5 foot-only;
//! Castle 20 foot + mounted; production buildings 10, entered ONLY
//! as the rally/newly-trained case and never re-entered; siege
//! never; the Petard (a siege/suicide unit) never.

use fixed::types::I16F16;

use crate::build::BuildingKind;
use crate::entity::{EntityKind, GarrisonRef, Task};
use crate::{Sim, TC_GARRISON_CAPACITY, TC_GARRISON_HEAL_HP_PER_MIN, TOWN_BELL_RADIUS};

/// Distance from a host's footprint edge at which a walking unit
/// is counted as arrived and enters (world units): a unit steered
/// to a ring standing point stands ~2 units off the edge, so 3.0
/// certifies arrival without letting a passer-by slip inside.
const GARRISON_ARRIVE: I16F16 = I16F16::from_bits(196_608); // 3.0

impl Sim {
    // ---- host bookkeeping -------------------------------------

    fn tc_state_index(&self, tc: u32) -> Option<usize> {
        self.tc_states.iter().position(|s| s.tc_id == tc)
    }

    /// (owner, center x, center y, footprint edge in cells) of a
    /// host, if it still stands: a placed building must exist; a
    /// Town Center entity must be alive.
    fn host_geometry(&self, host: GarrisonRef) -> Option<(u8, I16F16, I16F16, u32)> {
        match host {
            GarrisonRef::Building(bid) => {
                let bi = self.building_index(bid)?;
                let b = &self.buildings[bi];
                Some((b.owner, b.pos_x, b.pos_y, b.kind.footprint_size()))
            }
            GarrisonRef::TownCenter(tc) => {
                let ei = self.entity_index(tc)?;
                let e = &self.entities[ei];
                if e.dead || e.kind != EntityKind::TownCenter {
                    return None;
                }
                Some((e.owner, e.pos_x, e.pos_y, 4))
            }
        }
    }

    fn host_complete(&self, host: GarrisonRef) -> bool {
        match host {
            GarrisonRef::Building(bid) => match self.building_index(bid) {
                Some(bi) => self.buildings[bi].complete,
                None => false,
            },
            GarrisonRef::TownCenter(_) => self.host_geometry(host).is_some(),
        }
    }

    /// Current and maximum HP of a host, in raw fixed-point bits.
    fn host_hp(&self, host: GarrisonRef) -> Option<(i64, i64)> {
        match host {
            GarrisonRef::Building(bid) => {
                let bi = self.building_index(bid)?;
                let b = &self.buildings[bi];
                let max = self.building_max_hp(b.kind, b.owner) as i64;
                Some((b.hp.to_bits() as i64, max << 16))
            }
            GarrisonRef::TownCenter(tc) => {
                let ei = self.entity_index(tc)?;
                let e = &self.entities[ei];
                let max = self.stats_for(e.owner, EntityKind::TownCenter).max_hp as i64;
                Some((e.hp.to_bits() as i64, max << 16))
            }
        }
    }

    /// GAR-03's threshold: a host at ≤20% HP ejects its garrison
    /// and refuses (re-)entry until it is above the threshold.
    fn host_at_or_below_fifth(&self, host: GarrisonRef) -> bool {
        match self.host_hp(host) {
            Some((hp, max)) => hp * 5 <= max,
            None => true,
        }
    }

    pub(crate) fn host_capacity(&self, host: GarrisonRef) -> Option<u32> {
        match host {
            GarrisonRef::Building(bid) => {
                let bi = self.building_index(bid)?;
                self.buildings[bi].kind.garrison_capacity()
            }
            GarrisonRef::TownCenter(_) => Some(TC_GARRISON_CAPACITY),
        }
    }

    pub(crate) fn host_is_production(&self, host: GarrisonRef) -> bool {
        match host {
            GarrisonRef::Building(bid) => match self.building_index(bid) {
                Some(bi) => self.buildings[bi].kind.is_production_garrison(),
                None => false,
            },
            GarrisonRef::TownCenter(_) => false,
        }
    }

    fn host_garrison(&self, host: GarrisonRef) -> Vec<u32> {
        match host {
            GarrisonRef::Building(bid) => match self.building_index(bid) {
                Some(bi) => self.buildings[bi].garrison.clone(),
                None => Vec::new(),
            },
            GarrisonRef::TownCenter(tc) => match self.tc_state_index(tc) {
                Some(si) => self.tc_states[si].garrison.clone(),
                None => Vec::new(),
            },
        }
    }

    fn host_heal_rate(&self, host: GarrisonRef) -> i64 {
        match host {
            GarrisonRef::Building(bid) => match self.building_index(bid) {
                Some(bi) => self.buildings[bi].kind.garrison_heal_hp_per_min(),
                None => 0,
            },
            GarrisonRef::TownCenter(_) => TC_GARRISON_HEAL_HP_PER_MIN,
        }
    }

    /// Distance from a point to the host's footprint edge (0 when
    /// beside or inside it) — PRJ-08's building geometry.
    fn host_edge_dist(&self, host: GarrisonRef, px: I16F16, py: I16F16) -> Option<I16F16> {
        let (_, bx, by, size) = self.host_geometry(host)?;
        let half = I16F16::from_num(size as i32 * 2);
        let qx = (px - bx).clamp(-half, half) + bx;
        let qy = (py - by).clamp(-half, half) + by;
        Some(crate::steer::vec_len(px - qx, py - qy))
    }

    /// Standing points around a host (ring cells, fixed order,
    /// in-bounds and unblocked), then edge offsets as a fallback —
    /// the exit/spawn placement discipline of `spawn_trained_unit`.
    fn host_exit_points(&self, host: GarrisonRef) -> Vec<(I16F16, I16F16)> {
        let (anchor, size, cx0, cy0) = match host {
            GarrisonRef::Building(bid) => match self.building_index(bid) {
                Some(bi) => {
                    let b = &self.buildings[bi];
                    (
                        (b.cell_x, b.cell_y),
                        b.kind.footprint_size(),
                        b.pos_x,
                        b.pos_y,
                    )
                }
                None => return Vec::new(),
            },
            GarrisonRef::TownCenter(tc) => match self.entity_index(tc) {
                Some(ei) => {
                    let e = &self.entities[ei];
                    let (tx, ty) = self.grid.world_to_cell(e.pos_x, e.pos_y);
                    (
                        (tx.saturating_sub(1), ty.saturating_sub(1)),
                        4,
                        e.pos_x,
                        e.pos_y,
                    )
                }
                None => return Vec::new(),
            },
        };
        let mut out = Vec::new();
        for (cx, cy) in crate::build::ring_cells(anchor.0, anchor.1, size) {
            if cx >= crate::GRID_DIM || cy >= crate::GRID_DIM {
                continue;
            }
            if !self.grid.is_blocked(cx, cy) {
                out.push(self.grid.cell_center(cx, cy));
            }
        }
        if out.is_empty() {
            let half = I16F16::from_num(size as i32 * 2) + I16F16::from_num(2);
            for (ox, oy) in [
                (half, I16F16::ZERO),
                (-half, I16F16::ZERO),
                (I16F16::ZERO, half),
                (I16F16::ZERO, -half),
            ] {
                out.push((cx0 + ox, cy0 + oy));
            }
        }
        out
    }

    // ---- arrow scaling (GAR-05) --------------------------------

    /// One garrisoned unit's contributed ranged DPS, in raw
    /// fixed-point bits: a villager counts 2.5 DPS; a foot archer
    /// contributes its effective ranged DPS (pierce attack ÷
    /// reload); everything else — infantry, monks, and ALL mounted
    /// units (GAR-05: mounted grant no arrow boost) — contributes 0.
    fn contributed_dps_bits(&self, owner: u8, kind: EntityKind) -> i64 {
        if kind == EntityKind::Villager {
            return 163_840; // 2.5 × 65536
        }
        if !matches!(kind, EntityKind::Archer | EntityKind::Skirmisher) {
            return 0;
        }
        let spec = self.stats_for(owner, kind);
        let pierce = spec
            .attacks
            .iter()
            .find(|(c, _)| *c == crate::classes::PIERCE)
            .map(|(_, v)| *v)
            .unwrap_or(0);
        if pierce <= 0 || spec.reload_ticks == 0 {
            return 0;
        }
        (pierce as i64 * 65_536 * 60) / spec.reload_ticks as i64
    }

    /// Recompute a host's stored arrow count (GAR-05): event-driven
    /// (on every garrison change), never per tick. Extra arrows =
    /// floor(contributed DPS ÷ the building's own DPS). Caps:
    /// tower +4 over its base 1; Castle +16 over its base volley 5;
    /// Town Center 10 arrows TOTAL, minimum 1 while garrisoned
    /// (GAR-06). Unarmed and production hosts store 0.
    pub(crate) fn recompute_garrison_arrows(&mut self, host: GarrisonRef) {
        let (owner, kind_tag) = match host {
            GarrisonRef::Building(bid) => match self.building_index(bid) {
                Some(bi) => (self.buildings[bi].owner, Some(self.buildings[bi].kind)),
                None => return,
            },
            GarrisonRef::TownCenter(tc) => match self.entity_index(tc) {
                Some(ei) => (self.entities[ei].owner, None),
                None => return,
            },
        };
        let ids = self.host_garrison(host);
        let mut contributed: i64 = 0;
        for id in &ids {
            if let Some(ei) = self.entity_index(*id) {
                let k = self.entities[ei].kind;
                contributed += self.contributed_dps_bits(owner, k);
            }
        }
        let weapon = match kind_tag {
            Some(k) => k.weapon(),
            None => Some(crate::TOWN_CENTER_WEAPON),
        };
        let total: u32 = match weapon {
            None => 0,
            Some(w) => {
                let pierce = w
                    .attacks
                    .iter()
                    .find(|(c, _)| *c == crate::classes::PIERCE)
                    .map(|(_, v)| *v)
                    .unwrap_or(0);
                if pierce <= 0 || w.reload_ticks == 0 {
                    0
                } else {
                    let own_dps = (pierce as i64 * 65_536 * 60) / w.reload_ticks as i64;
                    let extra = if own_dps > 0 {
                        (contributed / own_dps).max(0) as u32
                    } else {
                        0
                    };
                    match kind_tag {
                        // Town Center: total count, min 1 while held.
                        None => {
                            if ids.is_empty() {
                                0
                            } else {
                                extra.max(1).min(10)
                            }
                        }
                        Some(BuildingKind::WatchTower) => 1 + extra.min(4),
                        Some(BuildingKind::Castle) => 5 + extra.min(16),
                        _ => 0,
                    }
                }
            }
        };
        match host {
            GarrisonRef::Building(bid) => {
                if let Some(bi) = self.building_index(bid) {
                    self.buildings[bi].garrison_arrows = total;
                }
            }
            GarrisonRef::TownCenter(tc) => {
                if let Some(si) = self.tc_state_index(tc) {
                    self.tc_states[si].garrison_arrows = total;
                }
            }
        }
    }

    // ---- validation & orders -----------------------------------

    /// Whether `entity` may garrison in `host` right now.
    /// `prod_entry` is the GAR-08 rally/newly-trained case — the
    /// only entry a production building ever accepts.
    pub(crate) fn can_garrison(
        &self,
        entity: u32,
        host: GarrisonRef,
        prod_entry: bool,
    ) -> bool {
        let idx = match self.entity_index(entity) {
            Some(i) => i,
            None => return false,
        };
        let e = &self.entities[idx];
        if e.dead || e.garrisoned_in.is_some() || !crate::entity::is_player_owner(e.owner) {
            return false;
        }
        if !e.kind.is_mobile() {
            return false;
        }
        // Eligibility (GAR-01's data column).
        if prod_entry {
            // Production garrison: the units a production building
            // trains — foot and mounted alike; never the Petard
            // (siege/suicide family), never animals or villagers.
            if !self.host_is_production(host) && !self.host_is_defensive(host) {
                return false;
            }
            if !(e.kind.is_foot_garrisonable() || e.kind.is_mounted_garrisonable())
                || e.kind == EntityKind::Villager
            {
                return false;
            }
        } else {
            if self.host_is_production(host) {
                return false; // production garrison: no manual entry, no re-entry
            }
            let admits = match host {
                GarrisonRef::Building(bid) => match self.building_index(bid) {
                    Some(bi) => self.buildings[bi].kind.admits_defensive(e.kind),
                    None => return false,
                },
                GarrisonRef::TownCenter(_) => e.kind.is_foot_garrisonable(),
            };
            if !admits {
                return false;
            }
        }
        // Host side: standing, complete, own, above the ≤20% HP
        // re-garrison block (GAR-03), and with room inside.
        let (howner, _, _, _) = match self.host_geometry(host) {
            Some(g) => g,
            None => return false,
        };
        if howner != e.owner || !self.host_complete(host) {
            return false;
        }
        if self.host_at_or_below_fifth(host) {
            return false;
        }
        match self.host_capacity(host) {
            Some(cap) => (self.host_garrison(host).len() as u32) < cap,
            None => false,
        }
    }

    fn host_is_defensive(&self, host: GarrisonRef) -> bool {
        match host {
            GarrisonRef::Building(bid) => match self.building_index(bid) {
                Some(bi) => self.buildings[bi].kind.is_defensive_garrison(),
                None => false,
            },
            GarrisonRef::TownCenter(_) => true,
        }
    }

    /// The mechanics of a garrison order, shared by the commands,
    /// the Town Bell and the rally spawn path: cancel the unit's
    /// current work and walk it to the host's standing point; the
    /// garrison tick enters it on arrival.
    pub(crate) fn apply_garrison_order(
        &mut self,
        idx: usize,
        host: GarrisonRef,
        prod_entry: bool,
    ) -> bool {
        let entity = self.entities[idx].id;
        if !self.can_garrison(entity, host, prod_entry) {
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
            e.garrison_target = Some(host);
            e.garrison_prod_entry = prod_entry;
        }
        // Walk to a standing point beside the host.
        let approach = match host {
            GarrisonRef::Building(bid) => match self.building_index(bid) {
                Some(bi) => {
                    let b = &self.buildings[bi];
                    self.approach_point(idx, b.cell_x, b.cell_y, b.kind.footprint_size())
                }
                None => None,
            },
            GarrisonRef::TownCenter(_) => self
                .host_exit_points(host)
                .into_iter()
                .find(|p| {
                    let (px, py) = (self.entities[idx].pos_x, self.entities[idx].pos_y);
                    self.grid.find_path((px, py), *p).is_some()
                }),
        };
        match approach {
            Some((px, py)) => {
                self.set_move_order(idx, px, py);
            }
            None => {
                // Crowded site: aim at the host's east edge point;
                // the arrival check in the garrison tick certifies
                // entry from beside the footprint.
                if let Some((_, bx, by, size)) = self.host_geometry(host) {
                    let half = I16F16::from_num(size as i32 * 2) + I16F16::from_num(2);
                    self.set_move_order(idx, bx + half, by);
                }
            }
        }
        true
    }

    pub(crate) fn issue_garrison_building(&mut self, entity: u32, building: u32) {
        if let Some(idx) = self.entity_index(entity) {
            self.apply_garrison_order(idx, GarrisonRef::Building(building), false);
        }
    }

    pub(crate) fn issue_garrison_tc(&mut self, entity: u32, tc: u32) {
        if let Some(idx) = self.entity_index(entity) {
            self.apply_garrison_order(idx, GarrisonRef::TownCenter(tc), false);
        }
    }

    // ---- ejection (GAR-03) --------------------------------------

    /// Place one garrisoned unit of `host` at an exit point, idle
    /// and unharmed. `slot` spreads simultaneous ejections over the
    /// exit points in order.
    fn place_at_exit(&mut self, idx: usize, host: GarrisonRef, slot: usize) {
        let points = self.host_exit_points(host);
        if !points.is_empty() {
            let (px, py) = points[slot % points.len()];
            let e = &mut self.entities[idx];
            e.pos_x = px;
            e.pos_y = py;
        }
        let e = &mut self.entities[idx];
        e.garrisoned_in = None;
        e.garrison_target = None;
        e.garrison_prod_entry = false;
        e.garrison_heal_remainder = 0;
        e.order = None;
        e.vel_x = I16F16::ZERO;
        e.vel_y = I16F16::ZERO;
        e.task = Task::Idle;
        e.task_node = None;
        e.task_building = None;
        e.task_dropoff = None;
        e.clear_combat();
    }

    /// Eject a host's whole garrison, unharmed, beside it (GAR-03):
    /// used by destruction, by the ≤20% HP rule, and by voluntary
    /// ungarrisoning alike.
    pub(crate) fn eject_host(&mut self, host: GarrisonRef) {
        let ids = self.host_garrison(host);
        if ids.is_empty() {
            return;
        }
        match host {
            GarrisonRef::Building(bid) => {
                if let Some(bi) = self.building_index(bid) {
                    self.buildings[bi].garrison.clear();
                }
            }
            GarrisonRef::TownCenter(tc) => {
                if let Some(si) = self.tc_state_index(tc) {
                    self.tc_states[si].garrison.clear();
                }
            }
        }
        for (slot, id) in ids.iter().enumerate() {
            if let Some(idx) = self.entity_index(*id) {
                if !self.entities[idx].dead {
                    self.place_at_exit(idx, host, slot);
                } else {
                    self.entities[idx].garrisoned_in = None;
                }
            }
        }
        self.recompute_garrison_arrows(host);
    }

    /// Remove ONE unit from its host (the Town Bell's restore path
    /// ejects bell villagers individually), placing it at the exit.
    fn eject_one(&mut self, idx: usize) {
        let host = match self.entities[idx].garrisoned_in {
            Some(h) => h,
            None => return,
        };
        let id = self.entities[idx].id;
        match host {
            GarrisonRef::Building(bid) => {
                if let Some(bi) = self.building_index(bid) {
                    self.buildings[bi].garrison.retain(|&g| g != id);
                }
            }
            GarrisonRef::TownCenter(tc) => {
                if let Some(si) = self.tc_state_index(tc) {
                    self.tc_states[si].garrison.retain(|&g| g != id);
                }
            }
        }
        self.place_at_exit(idx, host, 0);
        self.recompute_garrison_arrows(host);
    }

    pub(crate) fn issue_ungarrison_building(&mut self, building: u32) {
        self.eject_host(GarrisonRef::Building(building));
    }

    pub(crate) fn issue_ungarrison_tc(&mut self, tc: u32) {
        self.eject_host(GarrisonRef::TownCenter(tc));
    }

    // ---- the garrison tick ---------------------------------------

    /// Per-tick garrison work, in fixed order: arrivals enter,
    /// the ≤20% HP rule ejects (GAR-03), shelter healing accrues
    /// (GAR-04). Deterministic by construction (entity-id and
    /// building-id sweeps only).
    pub(crate) fn garrison_tick(&mut self) {
        // Arrivals.
        for idx in 0..self.entities.len() {
            let host = match self.entities[idx].garrison_target {
                Some(h) => h,
                None => continue,
            };
            if self.entities[idx].dead || self.entities[idx].garrisoned_in.is_some() {
                self.entities[idx].garrison_target = None;
                continue;
            }
            let entity = self.entities[idx].id;
            let prod = self.entities[idx].garrison_prod_entry;
            if !self.can_garrison(entity, host, prod) {
                // Host fell, filled, or dropped to ≤20% HP on the
                // way: the order ends and the unit stands idle.
                let e = &mut self.entities[idx];
                e.garrison_target = None;
                e.garrison_prod_entry = false;
                e.order = None;
                e.vel_x = I16F16::ZERO;
                e.vel_y = I16F16::ZERO;
                continue;
            }
            let (px, py) = (self.entities[idx].pos_x, self.entities[idx].pos_y);
            let arrived = self
                .host_edge_dist(host, px, py)
                .map(|d| d <= GARRISON_ARRIVE)
                .unwrap_or(false);
            if arrived {
                self.enter_garrison(idx, host);
            }
        }
        // ≤20% HP ejection (GAR-03).
        let mut hosts: Vec<GarrisonRef> = Vec::new();
        for b in &self.buildings {
            if !b.garrison.is_empty() {
                hosts.push(GarrisonRef::Building(b.id));
            }
        }
        for s in &self.tc_states {
            if !s.garrison.is_empty() {
                hosts.push(GarrisonRef::TownCenter(s.tc_id));
            }
        }
        for host in hosts {
            if self.host_at_or_below_fifth(host) {
                self.eject_host(host);
            }
        }
        // Shelter healing (GAR-04 base rates).
        for idx in 0..self.entities.len() {
            let host = match self.entities[idx].garrisoned_in {
                Some(h) => h,
                None => continue,
            };
            if self.entities[idx].dead {
                continue;
            }
            let rate = self.host_heal_rate(host);
            if rate <= 0 {
                continue;
            }
            let max_hp = I16F16::from_num(self.unit_stats(idx).max_hp);
            let e = &mut self.entities[idx];
            if e.hp >= max_hp {
                e.garrison_heal_remainder = 0;
                continue;
            }
            let total = e.garrison_heal_remainder + rate * 65_536;
            let heal_bits = total / 3600;
            e.garrison_heal_remainder = total % 3600;
            e.hp += I16F16::from_bits(heal_bits as i32);
            if e.hp > max_hp {
                e.hp = max_hp;
            }
        }
    }

    fn enter_garrison(&mut self, idx: usize, host: GarrisonRef) {
        let id = self.entities[idx].id;
        match host {
            GarrisonRef::Building(bid) => {
                if let Some(bi) = self.building_index(bid) {
                    self.buildings[bi].garrison.push(id);
                }
            }
            GarrisonRef::TownCenter(tc) => {
                if let Some(si) = self.tc_state_index(tc) {
                    self.tc_states[si].garrison.push(id);
                }
            }
        }
        {
            let e = &mut self.entities[idx];
            e.garrisoned_in = Some(host);
            e.garrison_target = None;
            e.garrison_prod_entry = false;
            e.garrison_heal_remainder = 0;
            e.order = None;
            e.vel_x = I16F16::ZERO;
            e.vel_y = I16F16::ZERO;
            e.task = Task::Idle;
            e.task_node = None;
            e.task_building = None;
            e.task_dropoff = None;
            e.clear_combat();
        }
        self.recompute_garrison_arrows(host);
    }

    // ---- Town Bell (GAR-07) --------------------------------------

    pub(crate) fn issue_town_bell(&mut self, tc: u32) {
        let si = match self.tc_state_index(tc) {
            Some(si) => si,
            None => return,
        };
        let owner = match self.entity_index(tc) {
            Some(ei) if !self.entities[ei].dead => self.entities[ei].owner,
            _ => return,
        };
        if !self.tc_states[si].bell_active {
            // Ring: send every own villager within 23 tiles to the
            // nearest shelter, snapshotting task + position. The
            // sweep runs in entity-id order (determinism).
            let (tcx, tcy) = match self.entity_index(tc) {
                Some(ei) => (self.entities[ei].pos_x, self.entities[ei].pos_y),
                None => return,
            };
            self.tc_states[si].bell_active = true;
            let mut assigned: Vec<(GarrisonRef, u32)> = Vec::new();
            let villagers: Vec<usize> = (0..self.entities.len())
                .filter(|&idx| {
                    let e = &self.entities[idx];
                    e.kind == EntityKind::Villager
                        && e.owner == owner
                        && !e.dead
                        && e.garrisoned_in.is_none()
                        && crate::steer::vec_len(e.pos_x - tcx, e.pos_y - tcy)
                            <= TOWN_BELL_RADIUS
                })
                .collect();
            for idx in villagers {
                // Nearest shelter with room: own Town Centers
                // (tc-state order), then own defensive buildings
                // (id order); strictly-nearer replaces.
                let (px, py) = (self.entities[idx].pos_x, self.entities[idx].pos_y);
                let mut best: Option<(i64, GarrisonRef)> = None;
                let mut shelters: Vec<GarrisonRef> = Vec::new();
                for s in &self.tc_states {
                    shelters.push(GarrisonRef::TownCenter(s.tc_id));
                }
                for b in &self.buildings {
                    if b.kind.is_defensive_garrison() {
                        shelters.push(GarrisonRef::Building(b.id));
                    }
                }
                for host in shelters {
                    let (howner, _, _, _) = match self.host_geometry(host) {
                        Some(g) => g,
                        None => continue,
                    };
                    if howner != owner || !self.host_complete(host) {
                        continue;
                    }
                    if self.host_at_or_below_fifth(host) {
                        continue;
                    }
                    let cap = match self.host_capacity(host) {
                        Some(c) => c,
                        None => continue,
                    };
                    let used = self.host_garrison(host).len() as u32
                        + assigned
                            .iter()
                            .find(|(h, _)| *h == host)
                            .map(|(_, n)| *n)
                            .unwrap_or(0);
                    if used >= cap {
                        continue;
                    }
                    let d = match self.host_edge_dist(host, px, py) {
                        Some(d) => d,
                        None => continue,
                    };
                    let db = d.to_bits() as i64;
                    let key = db * db;
                    if best.map(|(bk, _)| key < bk).unwrap_or(true) {
                        best = Some((key, host));
                    }
                }
                let host = match best {
                    Some((_, h)) => h,
                    None => continue, // no shelter with room: stays at work
                };
                {
                    let e = &mut self.entities[idx];
                    e.bell_snapshot = Some(crate::entity::BellSnapshot {
                        pos_x: e.pos_x,
                        pos_y: e.pos_y,
                        task: e.task,
                        task_node: e.task_node,
                        task_building: e.task_building,
                    });
                }
                if self.apply_garrison_order(idx, host, false) {
                    match assigned.iter_mut().find(|(h, _)| *h == host) {
                        Some((_, n)) => *n += 1,
                        None => assigned.push((host, 1)),
                    }
                } else {
                    self.entities[idx].bell_snapshot = None;
                }
            }
        } else {
            // Ring again — "back to work": ungarrison the bell
            // villagers and restore each snapshot (id order).
            self.tc_states[si].bell_active = false;
            let held: Vec<usize> = (0..self.entities.len())
                .filter(|&idx| {
                    self.entities[idx].owner == owner
                        && self.entities[idx].bell_snapshot.is_some()
                        && !self.entities[idx].dead
                })
                .collect();
            for idx in held {
                let snap = self.entities[idx].bell_snapshot.take().expect("filtered");
                if self.entities[idx].garrisoned_in.is_some() {
                    self.eject_one(idx);
                } else {
                    // Still walking to shelter when the second ring
                    // landed: cancel the walk.
                    let e = &mut self.entities[idx];
                    e.garrison_target = None;
                    e.garrison_prod_entry = false;
                    e.order = None;
                    e.vel_x = I16F16::ZERO;
                    e.vel_y = I16F16::ZERO;
                }
                // Restore the snapshot task where it still stands;
                // otherwise walk back to the snapshot position.
                let restored = match snap.task {
                    Task::ToNode | Task::Gathering | Task::Returning => match snap.task_node {
                        Some(nid) if self.live_node_pos(nid).is_some() => {
                            self.begin_gather(idx, nid);
                            true
                        }
                        _ => false,
                    },
                    Task::ToBuild | Task::Building => match snap.task_building {
                        Some(bid) => match self.building_index(bid) {
                            Some(bi) if !self.buildings[bi].complete => {
                                self.assign_builder(idx, bid);
                                true
                            }
                            _ => false,
                        },
                        None => false,
                    },
                    Task::Repair { building } => self.apply_repair(idx, building),
                    Task::Idle => false,
                };
                if !restored {
                    self.set_move_order(idx, snap.pos_x, snap.pos_y);
                }
            }
        }
    }

    // ---- rally-garrison (GAR-08) ---------------------------------

    /// The garrison host a rally point at `(rx, ry)` sits on, for a
    /// unit owned by the rallying player: the first standing,
    /// complete building (id order) whose footprint contains the
    /// point and that would take this unit, then Town Centers the
    /// same way. `None` when the point is on open ground, on a
    /// non-garrisonable building, or on a full/damaged host.
    pub(crate) fn rally_garrison_host(
        &self,
        idx: usize,
        rx: I16F16,
        ry: I16F16,
    ) -> Option<GarrisonRef> {
        let (owner, kind) = (self.entities[idx].owner, self.entities[idx].kind);
        let id = self.entities[idx].id;
        for b in &self.buildings {
            if b.owner != owner || !b.complete || b.kind.garrison_capacity().is_none() {
                continue;
            }
            let half = I16F16::from_num(b.kind.footprint_size() as i32 * 2);
            // Boundary is NOT inside: a rally point exactly on the
            // footprint edge is open ground (a walk, SEL-13) — the
            // pre-P3b TC rally test rallies exactly 8 units east of
            // the TC, on its footprint edge, and must keep walking.
            if (rx - b.pos_x).abs() >= half || (ry - b.pos_y).abs() >= half {
                continue;
            }
            let host = GarrisonRef::Building(b.id);
            let prod = b.kind.is_production_garrison();
            if self.can_garrison(id, host, prod) {
                return Some(host);
            }
        }
        for s in &self.tc_states {
            let host = GarrisonRef::TownCenter(s.tc_id);
            let (_, bx, by, _) = match self.host_geometry(host) {
                Some(g) => g,
                None => continue,
            };
            let half = I16F16::from_num(8);
            if (rx - bx).abs() >= half || (ry - by).abs() >= half {
                continue;
            }
            if self.can_garrison(id, host, false) {
                return Some(host);
            }
        }
        let _ = kind;
        None
    }

    // ---- client queries ------------------------------------------

    /// Entities garrisoned in a placed building (0 if unknown).
    pub fn garrison_count_building(&self, building: u32) -> u32 {
        self.host_garrison(GarrisonRef::Building(building)).len() as u32
    }

    /// Entities garrisoned in a Town Center (0 if unknown).
    pub fn garrison_count_tc(&self, tc: u32) -> u32 {
        self.host_garrison(GarrisonRef::TownCenter(tc)).len() as u32
    }

    /// A placed building's stored GAR-05 arrow total.
    pub fn garrison_arrows_building(&self, building: u32) -> u32 {
        match self.building_index(building) {
            Some(bi) => self.buildings[bi].garrison_arrows,
            None => 0,
        }
    }

    /// A Town Center's stored GAR-05/06 arrow total.
    pub fn garrison_arrows_tc(&self, tc: u32) -> u32 {
        match self.tc_state_index(tc) {
            Some(si) => self.tc_states[si].garrison_arrows,
            None => 0,
        }
    }

    /// Whether a Town Center's bell is currently rung (GAR-07).
    pub fn tc_bell_active(&self, tc: u32) -> bool {
        match self.tc_state_index(tc) {
            Some(si) => self.tc_states[si].bell_active,
            None => false,
        }
    }

    /// Whether the unit may be ordered to garrison in the building
    /// (the client's garrison-order gate, SEL-11).
    pub fn can_garrison_building(&self, entity: u32, building: u32) -> bool {
        self.can_garrison(entity, GarrisonRef::Building(building), false)
    }

    /// Whether the unit may be ordered to garrison in the Town
    /// Center (the client's garrison-order gate, SEL-11).
    pub fn can_garrison_tc(&self, entity: u32, tc: u32) -> bool {
        self.can_garrison(entity, GarrisonRef::TownCenter(tc), false)
    }

    /// Whether a Town Bell ring would take effect at this TC.
    pub fn can_town_bell(&self, tc: u32) -> bool {
        self.tc_state_index(tc).is_some()
            && matches!(self.entity_index(tc), Some(ei) if !self.entities[ei].dead)
    }
}
