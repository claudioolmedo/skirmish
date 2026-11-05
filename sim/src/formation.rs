//! Formations (MOV-07) — destination slots by class layer.
//!
//! A group ordered in formation (Line, Box, Staggered, Flank)
//! receives destination slots: melee units in the front layers,
//! ranged and siege behind, monks/siege at the center-back of a
//! box. Units path to their own slot and hold the shape on
//! arrival (docs/MECHANICS-RULEBOOK.md MOV-07; GAP 2.10; wiki,
//! "Unit formation" / "Battle Formations").
//!
//! Slot assignment is a PURE function of (group composition in
//! id order, formation kind, destination, facing) computed at
//! order time — deterministic and hash-safe, since slots are
//! just per-unit move targets: a formation order installs
//! ordinary `MoveOrder`s whose targets are the slots, so the
//! state hash covers them through the fields it already covers
//! and no new hashed state exists. A match that never orders a
//! formation hashes byte-identically to before MOV-07.
//!
//! All math is fixed-point / integer: facing is quantized to
//! one of 8 directions (MOV-09's facing model) from a small
//! constant table, and slot offsets are integer multiples of
//! a half-spacing step along the facing's forward/right unit
//! vectors, accumulated in i64 raw bits — no floats, no RNG,
//! no iteration over unordered containers anywhere.
//!
//! MOV-08 lives here too: on a move longer than ~30 tiles a
//! formed group narrows into a file/column for travel and
//! re-forms near the destination. The column is the same
//! assignment machinery with a one-wide layout
//! (`assign_column_slots`), and the travel/re-form anchors
//! are pure geometry (`column_anchor_points`) — the sim
//! routes each member's ordinary move order through its
//! column slots at both anchors on the way to its formation
//! slot, so the column, too, is just per-unit move targets
//! and no new hashed state exists.

use fixed::types::I16F16;

use crate::entity::EntityKind;
use crate::units;

/// The four documented battle formations (wiki, "Battle
/// Formations"): Line is the reference default — ranks by
/// class layer facing the enemy; Box packs the group into a
/// block with its vulnerable units sheltered center-back;
/// Staggered offsets alternate ranks by half a step so the
/// ranks behind can act between the front rank's members;
/// Flank splits the melee into two wings with the shooters
/// and support held in the center between/behind them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FormationKind {
    Line,
    Box,
    Staggered,
    Flank,
}

impl FormationKind {
    /// Stable tag for the wire vocabulary, the save codecs and
    /// the state hash: 0 line, 1 box, 2 staggered, 3 flank.
    pub fn tag(self) -> u8 {
        match self {
            FormationKind::Line => 0,
            FormationKind::Box => 1,
            FormationKind::Staggered => 2,
            FormationKind::Flank => 3,
        }
    }

    /// Inverse of `tag`; unknown values map to `None`.
    pub fn from_tag(tag: u8) -> Option<FormationKind> {
        match tag {
            0 => Some(FormationKind::Line),
            1 => Some(FormationKind::Box),
            2 => Some(FormationKind::Staggered),
            3 => Some(FormationKind::Flank),
            _ => None,
        }
    }
}

/// Slot spacing between neighbouring units, in world units
/// (2.0 = half a tile): units carry a 1.0-world-unit collision
/// radius (MOV-05's table), so adjacent slots sit exactly one
/// diameter apart — the shape holds without separation having
/// to shove members off their slots (MOV-04 tolerates the
/// residual crowding). Offsets are computed in HALF-spacing
/// steps (1.0 world unit) so a rank can center exactly on the
/// destination with integer math.
const HALF_SPACING_BITS: i64 = 65_536; // 1.0 world unit

/// MOV-08's documented threshold: a formed move longer than
/// ~30 tiles collapses into column. One tile is 4 world
/// units, so the switch sits at 120.0 world units from the
/// group's centroid to the destination anchor. "Longer
/// than" is strict: a move of exactly 30 tiles stays formed.
pub const LONG_MOVE_DISTANCE: I16F16 = I16F16::from_bits(7_864_320); // 120.0

/// MOV-08 geometry margin, in half-spacing steps (1 step =
/// 1.0 world unit): the column assembles this far ahead of
/// the group's centroid and re-forms this far short of the
/// destination — 16 world units = 4 tiles at each end. No DE
/// source pins the re-form distance; this is the smallest
/// margin that gives the file a full formation-depth of
/// travel to spread back into shape on the final leg, and
/// it is a GEOMETRY constant, not a game value (it touches
/// no cost, stat or balance number).
const COLUMN_MARGIN_STEPS: i64 = 16;

/// Whether a formed move of this centroid-to-destination
/// distance is a MOV-08 long move (see `LONG_MOVE_DISTANCE`).
pub fn is_long_move(distance: I16F16) -> bool {
    distance > LONG_MOVE_DISTANCE
}

/// MOV-08's two travel anchors for a long move, as
/// (assembly, re-form): the assembly anchor stands
/// `COLUMN_MARGIN_STEPS` ahead of the group centroid along
/// the (quantized) facing, the re-form anchor the same
/// margin short of the destination. Column slots computed
/// at the two anchors carry identical offsets, so the leg
/// between them translates the file without reshaping it.
/// Pure integer geometry, like the rest of this module.
pub fn column_anchor_points(
    centroid: (I16F16, I16F16),
    dest: (I16F16, I16F16),
    facing: (I16F16, I16F16),
) -> ((I16F16, I16F16), (I16F16, I16F16)) {
    let (fwd, right) = facing_vectors(facing.0, facing.1);
    let c = (centroid.0.to_bits() as i64, centroid.1.to_bits() as i64);
    let d = (dest.0.to_bits() as i64, dest.1.to_bits() as i64);
    let a = cell_point(
        c,
        fwd,
        right,
        Cell {
            lat: 0,
            depth: COLUMN_MARGIN_STEPS,
        },
    );
    let r = cell_point(
        d,
        fwd,
        right,
        Cell {
            lat: 0,
            depth: -COLUMN_MARGIN_STEPS,
        },
    );
    (
        (I16F16::from_bits(a.0 as i32), I16F16::from_bits(a.1 as i32)),
        (I16F16::from_bits(r.0 as i32), I16F16::from_bits(r.1 as i32)),
    )
}

/// The class layer a kind fights from (MOV-07's layering):
/// 0 = melee front (range-0 military: infantry, cavalry, the
/// Petard), 1 = civilians and other non-combatants (villagers,
/// trade carts, the King — they travel sheltered behind the
/// front), 2 = ranged (foot and mounted archers — anything
/// whose table row carries a real attack range), 3 = siege
/// and monks (the back layer; in a Box, its center-back).
/// Keyed on the unit TABLE (range + kind families), never on
/// per-order state, so the layer is a pure function of kind.
pub fn layer_of(kind: EntityKind) -> u8 {
    // Siege first: the Mangonel and Scorpion carry attack
    // ranges but are siege, not archers; the Ram carries none
    // but is siege too. The Monk has no attack at all (CLS-08)
    // and shelters with the siege at the back.
    if matches!(
        kind,
        EntityKind::BatteringRam | EntityKind::Mangonel | EntityKind::Scorpion | EntityKind::Monk
    ) {
        return 3;
    }
    if units::spec(kind).range_max > I16F16::ZERO {
        return 2;
    }
    if kind.is_military() {
        return 0;
    }
    1
}

/// One group member for slot assignment: its entity id and
/// kind. Assignment consumes members in ANY order and sorts
/// by id itself — the result depends only on the composition,
/// never on the caller's iteration order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FormationMember {
    pub id: u32,
    pub kind: EntityKind,
}

/// The 8 facing unit vectors (MOV-09), as (forward, right)
/// raw-bit pairs. Diagonal components are 46341 bits
/// (0.70711 × 2^16, the rounded 1/√2). The right vector is
/// the forward rotated a quarter turn clockwise in world
/// space (x east, y south on screen): right = (-fy, fx) in
/// the sim's y-down world reads as the facing's right hand —
/// the only property the layout relies on is that `right`
/// is `forward`'s fixed perpendicular, consistently.
const OCTANTS: [((i64, i64), (i64, i64)); 8] = [
    ((65_536, 0), (0, 65_536)),       // E
    ((46_341, 46_341), (-46_341, 46_341)), // SE
    ((0, 65_536), (-65_536, 0)),      // S
    ((-46_341, 46_341), (-46_341, -46_341)), // SW
    ((-65_536, 0), (0, -65_536)),    // W
    ((-46_341, -46_341), (46_341, -46_341)), // NW
    ((0, -65_536), (65_536, 0)),      // N
    ((46_341, -46_341), (46_341, 46_341)), // NE
];

/// Quantize a direction vector to the nearest of the 8
/// facings, returning its (forward, right) unit vectors as
/// raw-bit pairs. A zero vector faces East (octant 0) — a
/// group ordered onto its own centroid keeps a defined
/// shape. Selection is by maximum dot product over i64 bit
/// arithmetic (ties keep the lower octant: the strict `>`),
/// so every machine quantizes identically.
pub fn facing_vectors(dx: I16F16, dy: I16F16) -> ((i64, i64), (i64, i64)) {
    let (dx, dy) = (dx.to_bits() as i64, dy.to_bits() as i64);
    if dx == 0 && dy == 0 {
        return OCTANTS[0];
    }
    let mut best = 0usize;
    let mut best_dot = i64::MIN;
    for (i, ((fx, fy), _)) in OCTANTS.iter().enumerate() {
        let dot = fx * dx + fy * dy;
        if dot > best_dot {
            best_dot = dot;
            best = i;
        }
    }
    OCTANTS[best]
}

/// Smallest `c` with `c * c >= n` (the formation's column
/// count: the squarest rank width for `n` units).
fn ceil_sqrt(n: usize) -> usize {
    let mut c = 1;
    while c * c < n {
        c += 1;
    }
    c
}

/// One slot's offset from the destination anchor, in
/// half-spacing steps along (right, forward): positive
/// `depth` is AHEAD of the anchor (toward the facing),
/// positive `lat` is to the facing's right.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Cell {
    lat: i64,
    depth: i64,
}

/// Convert a cell offset into a world point: anchor +
/// right * lat + forward * depth, in i64 raw bits (the unit
/// vectors' bits times the half-spacing bits, shifted back
/// by the fixed-point fraction — exact for axis facings,
/// deterministically rounded for diagonals).
fn cell_point(anchor: (i64, i64), fwd: (i64, i64), right: (i64, i64), cell: Cell) -> (i64, i64) {
    let lat_bits = cell.lat * HALF_SPACING_BITS;
    let depth_bits = cell.depth * HALF_SPACING_BITS;
    let x = anchor.0 + (right.0 * lat_bits >> 16) + (fwd.0 * depth_bits >> 16);
    let y = anchor.1 + (right.1 * lat_bits >> 16) + (fwd.1 * depth_bits >> 16);
    (x, y)
}

/// Ranked cells for Line / Staggered / Box, in PLACEMENT
/// order for members already sorted by (layer, id):
/// `sorted_layers[i]` is the layer of the i-th placed member.
/// Each LAYER starts a fresh row — layers never share a row,
/// so the front rows are purely melee and every later layer
/// stands wholly behind the one before it (MOV-07's "melee
/// in the front layers, ranged and siege behind"). Rows hold
/// up to `cols` members and are centered on the anchor;
/// depths are centered too (the front row is as far ahead
/// of the anchor as the back row is behind it). Staggered
/// shifts odd rows half a step (1 half-spacing) to the
/// facing's right.
fn layered_cells(sorted_layers: &[u8], cols: usize, stagger: bool) -> Vec<Cell> {
    // First pass: row breaks — a new row when the layer
    // changes or the row fills.
    let mut rows_of: Vec<usize> = Vec::with_capacity(sorted_layers.len());
    let mut row = 0usize;
    let mut in_row = 0usize;
    let mut prev_layer: Option<u8> = None;
    for &layer in sorted_layers {
        if prev_layer == Some(layer) && in_row < cols {
            in_row += 1;
        } else {
            if prev_layer.is_some() {
                row += 1;
            }
            prev_layer = Some(layer);
            in_row = 1;
        }
        rows_of.push(row);
    }
    let total_rows = if sorted_layers.is_empty() { 0 } else { row + 1 };
    // Row lengths, for centering each row on the anchor.
    let mut row_len = vec![0usize; total_rows];
    for &r in &rows_of {
        row_len[r] += 1;
    }
    let mut col_in_row = vec![0usize; total_rows];
    let mut cells = Vec::with_capacity(sorted_layers.len());
    for &r in rows_of.iter() {
        let c = col_in_row[r];
        col_in_row[r] += 1;
        let mut lat = 2 * c as i64 - (row_len[r] as i64 - 1);
        if stagger && r % 2 == 1 {
            lat += 1;
        }
        let depth = (total_rows as i64 - 1) - 2 * r as i64;
        cells.push(Cell { lat, depth });
    }
    cells
}

/// MOV-07's slot assignment: a pure function of (group
/// composition in id order, formation kind, destination,
/// facing). `members` may arrive in any order; they are
/// sorted by id, layered by `layer_of`, and filled into the
/// formation's cells front-to-back in (layer, id) order —
/// within a layer the lowest id takes the leftmost-front
/// slot, as the reference's deterministic fill does. The
/// destination is the formation's anchor (its center); the
/// facing is the direction the front rank faces.
///
/// Returns one (id, slot) pair per member, in the INPUT
/// order, so callers can zip the result back onto their own
/// member list.
pub fn assign_slots(
    members: &[FormationMember],
    formation: FormationKind,
    dest: (I16F16, I16F16),
    facing: (I16F16, I16F16),
) -> Vec<(u32, (I16F16, I16F16))> {
    let n = members.len();
    if n == 0 {
        return Vec::new();
    }
    if n == 1 {
        // A lone unit has no shape to hold: its slot is the
        // destination itself, in every formation.
        return vec![(members[0].id, dest)];
    }
    let (fwd, right) = facing_vectors(facing.0, facing.1);
    let anchor = (dest.0.to_bits() as i64, dest.1.to_bits() as i64);

    let order = placement_order(members);
    let sorted_layers: Vec<u8> = order.iter().map(|&i| layer_of(members[i].kind)).collect();
    let cols = ceil_sqrt(n);
    let cells = match formation {
        FormationKind::Line => layered_cells(&sorted_layers, cols, false),
        FormationKind::Staggered => layered_cells(&sorted_layers, cols, true),
        FormationKind::Box => {
            let mut cells = layered_cells(&sorted_layers, cols, false);
            // Center-back for the back layer (siege/monks):
            // the cells the back layer will occupy are the
            // TAIL of the placement list (it sorts last);
            // reorder that tail center-first — smallest
            // |lat|, then deepest (most negative depth),
            // then leftmost — so the layer's first members
            // (lowest ids) take the sheltered center slots
            // and any overflow wraps around them.
            let back_count = sorted_layers.iter().filter(|&&l| l == 3).count();
            if back_count > 0 {
                let tail_start = n - back_count;
                let mut tail: Vec<Cell> = cells[tail_start..].to_vec();
                tail.sort_by(|a, b| {
                    a.lat
                        .abs()
                        .cmp(&b.lat.abs())
                        .then(a.depth.cmp(&b.depth))
                        .then(a.lat.cmp(&b.lat))
                });
                cells[tail_start..].copy_from_slice(&tail);
            }
            cells
        }
        FormationKind::Flank => flank_cells(members, &order),
    };

    slots_from_cells(members, &order, &cells, anchor, fwd, right)
}

/// The canonical placement order shared by every layout:
/// by id first (the composition's canonical order), then
/// stably by layer — within a layer, id order is preserved.
/// Placement position `i` in the returned order takes cell
/// `i` of the layout's cell list.
fn placement_order(members: &[FormationMember]) -> Vec<usize> {
    let mut order: Vec<usize> = (0..members.len()).collect();
    order.sort_by_key(|&i| members[i].id);
    order.sort_by_key(|&i| layer_of(members[i].kind));
    order
}

/// Turn a layout's cells (in placement order) into slots:
/// one (id, slot) pair per member, in the INPUT order.
fn slots_from_cells(
    members: &[FormationMember],
    order: &[usize],
    cells: &[Cell],
    anchor: (i64, i64),
    fwd: (i64, i64),
    right: (i64, i64),
) -> Vec<(u32, (I16F16, I16F16))> {
    let n = members.len();
    let mut out: Vec<(u32, (I16F16, I16F16))> = vec![(0, (I16F16::ZERO, I16F16::ZERO)); n];
    for (slot_idx, &member_idx) in order.iter().enumerate() {
        let (x, y) = cell_point(anchor, fwd, right, cells[slot_idx]);
        out[member_idx] = (
            members[member_idx].id,
            (
                I16F16::from_bits(x as i32),
                I16F16::from_bits(y as i32),
            ),
        );
    }
    out
}

/// MOV-08's column assignment: the same placement order as
/// `assign_slots` (melee front, then civilians, ranged,
/// siege/monks at the back) laid out one-wide — a file
/// along the facing, centered on the anchor, members one
/// spacing apart. This is `layered_cells` with a column
/// count of 1: every member is its own row, so the file's
/// head is the front layer and its tail the back layer,
/// exactly the order the group marches in. A single
/// member's column slot is the anchor itself, as in every
/// formation.
pub fn assign_column_slots(
    members: &[FormationMember],
    anchor_point: (I16F16, I16F16),
    facing: (I16F16, I16F16),
) -> Vec<(u32, (I16F16, I16F16))> {
    let n = members.len();
    if n == 0 {
        return Vec::new();
    }
    if n == 1 {
        return vec![(members[0].id, anchor_point)];
    }
    let (fwd, right) = facing_vectors(facing.0, facing.1);
    let anchor = (
        anchor_point.0.to_bits() as i64,
        anchor_point.1.to_bits() as i64,
    );
    let order = placement_order(members);
    let sorted_layers: Vec<u8> = order.iter().map(|&i| layer_of(members[i].kind)).collect();
    let cells = layered_cells(&sorted_layers, 1, false);
    slots_from_cells(members, &order, &cells, anchor, fwd, right)
}

/// Flank cells, in placement order for the sorted members:
/// the melee layer splits into two wings at the front —
/// the first half (id order) to the left wing, the rest to
/// the right — extending outward from the center block; the
/// remaining layers form a Line block in the center behind
/// the wings' front line. With no melee, Flank degenerates
/// to Line; with only melee, the wings flank an empty
/// center at the anchor's depth.
fn flank_cells(members: &[FormationMember], order: &[usize]) -> Vec<Cell> {
    let n = members.len();
    let melee_count = order
        .iter()
        .filter(|&&i| layer_of(members[i].kind) == 0)
        .count();
    let center_count = n - melee_count;
    // The center block is a layered Line layout of its own;
    // its front depth and widest half-width frame the wings.
    let (center_depth_front, center_half_lat) = if center_count > 0 {
        let center_layers: Vec<u8> = order[melee_count..]
            .iter()
            .map(|&i| layer_of(members[i].kind))
            .collect();
        let ccells = layered_cells(&center_layers, ceil_sqrt(center_count), false);
        let front = ccells.iter().map(|c| c.depth).max().unwrap_or(0);
        let half = ccells.iter().map(|c| c.lat.abs()).max().unwrap_or(0);
        (front, half)
    } else {
        (0, 0)
    };
    let mut cells = vec![Cell { lat: 0, depth: 0 }; n];
    // Center members occupy placement positions melee_count..
    // with their layered cells set BEHIND the front line by
    // one spacing, so the wings stand ahead of them.
    if center_count > 0 {
        let center_layers: Vec<u8> = order[melee_count..]
            .iter()
            .map(|&i| layer_of(members[i].kind))
            .collect();
        let center_cells = layered_cells(&center_layers, ceil_sqrt(center_count), false);
        for (j, cell) in center_cells.iter().enumerate() {
            cells[melee_count + j] = Cell {
                lat: cell.lat,
                depth: cell.depth - 2,
            };
        }
    }
    // Wings: placement positions 0..melee_count are the melee
    // in (layer, id) order — the first half forms the LEFT
    // wing (negative lat), the rest the RIGHT wing, each
    // extending outward from just beside the center block at
    // the center's front depth. Wing member i stands
    // (i+1) spacings out from the block's edge.
    let left_count = (melee_count + 1) / 2;
    for i in 0..melee_count {
        let (side, out_idx) = if i < left_count {
            (-1i64, i as i64)
        } else {
            (1i64, (i - left_count) as i64)
        };
        cells[i] = Cell {
            lat: side * (center_half_lat + 2 + 2 * out_idx),
            depth: center_depth_front,
        };
    }
    cells
}
