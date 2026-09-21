//! A storage class as a grid: where each thing in it lies, which way
//! round, and how many are in the stack.
//!
//! The hold's counts say *how much* of each resource is aboard; the
//! grids say *where*. Every class but the research desk is one grid
//! [`GRID_COLS`] across — a shelf, a cold store, a suit locker, an
//! armoury or a drug lab is so many **cells** of its class's grid
//! (`PartDef::capacity`) — and every thing kept there covers its
//! `economy::footprint` of them, laid out the way a survival game's
//! inventory is: nothing goes in that has no run of cells to lie on, and a
//! thing can be turned a quarter round to make it fit. Goods that stack
//! (`economy::stack_size`) cover one footprint a **stack**: ten ore to a
//! cell, a crate of ten vegetables one by two, a block of ten tofu four
//! by four — so what a shelf holds is its cells times the stacks, which is
//! what bounds it. A piece of armour and a gun are one each, since each
//! is an instance. So beside the hold's counts the world keeps a [`Grid`]
//! a class: one [`Slot`] per stack, piece or gun, with an id that only
//! climbs, what it holds, how many, where it lies and whether it is
//! turned.
//!
//! **The invariant**: a class's slots are exactly what the class holds —
//! one per piece of armour at `Where::Hold`, one per gun on
//! `World::guns`, and for every other resource stacks whose counts add
//! up to the hold's count, none over the stack size — as far as they fit.
//! `World::settle_grids` holds it, inside `on_ship_changed` like the
//! pieces' and the guns' settles: a count that grew tops up the stacks
//! with room first (the lowest id first) and lays new ones in the first
//! place they fit, row by row from the top left, unturned anywhere before
//! turned anywhere; a count that shrank empties the last stack first (the
//! highest id) and drops what is empty; a piece or a gun gone loses its
//! slot. Whether a thing *may* arrive is asked first, by
//! `World::has_room`, so the settle finds room for everything that came
//! through a command. A count poked past the grid — a test, a design
//! bought full in the yard — leaves the overflow **unplaced**: still
//! counted, in no slot, and laid in the step room is made. It is never
//! lost and never forced in.
//!
//! The grids, their slots and their next ids are in `world_checksum`
//! whole: two crews whose armouries are laid out differently have
//! different armouries, and where a rifle lies is what a
//! `Command::Arrange` changes.

use bims::combat::{Tier, WeaponKind};
use economy::{Footprint, stack_size};
use physics::ResourceId;
use shipdesign::GRID_COLS;

/// What one slot of a grid holds.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Kept {
    /// A piece of armour, by its id — one of `World::pieces`.
    Piece(u32),
    /// A weapon of a kind at a tier — one off `World::guns`. Two guns of
    /// a kind and tier are alike, so a slot names the kind and not the
    /// gun.
    Gun(WeaponKind, Tier),
    /// A stack of a resource that is neither: ore, a crate of vegetables,
    /// a suit — `Slot::count` says how many, up to its stack size.
    Stack(ResourceId),
}

impl Kept {
    /// The numbers that go in the checksum: which of the three, then what.
    pub fn codes(self) -> (u64, u64, u64) {
        match self {
            Kept::Piece(id) => (0, id as u64, 0),
            Kept::Gun(kind, tier) => (1, kind.code() as u64, tier.code() as u64),
            Kept::Stack(id) => (2, id as u64, 0),
        }
    }
}

/// One thing on a grid: what, how many, where, and which way round.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Slot {
    /// Only ever climbs — `Grid::next` — so a slot is the same slot
    /// across steps and two clients name the next one alike.
    pub id: u32,
    pub kept: Kept,
    /// How many are in the stack: one for a piece or a gun, up to the
    /// resource's stack size for a stack.
    pub count: u32,
    /// Its footprint unturned, as `economy::footprint` gives it: kept
    /// here so the grid can be asked about itself without the hold.
    pub foot: Footprint,
    /// The column and row of its top-left cell.
    pub x: u8,
    pub y: u8,
    /// Turned a quarter round: it covers `foot.turned()`.
    pub turned: bool,
}

impl Slot {
    /// The cells it covers: columns and rows, as laid.
    pub fn laid(&self) -> Footprint {
        if self.turned {
            self.foot.turned()
        } else {
            self.foot
        }
    }

    /// Whether cell `(x, y)` is one of its.
    pub fn covers(&self, x: u32, y: u32) -> bool {
        covers_cell(self.x as u32, self.y as u32, self.laid(), x, y)
    }

    /// How many more the stack takes: nought for a piece or a gun.
    pub fn room(&self) -> u32 {
        match self.kept {
            Kept::Stack(id) => stack_size(id).saturating_sub(self.count),
            Kept::Piece(_) | Kept::Gun(..) => 0,
        }
    }
}

/// Whether a footprint laid at `(sx, sy)` covers cell `(x, y)`.
fn covers_cell(sx: u32, sy: u32, laid: Footprint, x: u32, y: u32) -> bool {
    x >= sx && x < sx + laid.cols as u32 && y >= sy && y < sy + laid.rows as u32
}

/// One class's grid: every slot, and the next id.
#[derive(Clone, PartialEq, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Grid {
    /// In id order — a slot is pushed with `next` and never reordered —
    /// so two worlds that laid the same things hash the same list.
    pub slots: Vec<Slot>,
    pub next: u32,
}

impl Default for Grid {
    fn default() -> Grid {
        Grid {
            slots: Vec::new(),
            next: 1,
        }
    }
}

/// What a class holds, for a settle: a piece, a gun, or so many units
/// of a resource to be kept in stacks.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Wanted {
    Piece(u32, Footprint),
    Gun(WeaponKind, Tier, Footprint),
    Units(ResourceId, u32, Footprint),
}

impl Grid {
    /// How many rows a capacity in cells is: the last one ragged if the
    /// capacity is not a multiple of the width, its cells past the
    /// capacity blocked.
    pub fn rows(capacity: u32) -> u32 {
        capacity.div_ceil(GRID_COLS)
    }

    /// Whether cell `(x, y)` is on a grid of `capacity` cells at all.
    pub fn on_grid(capacity: u32, x: u32, y: u32) -> bool {
        x < GRID_COLS && y < Grid::rows(capacity) && y * GRID_COLS + x < capacity
    }

    pub fn slot(&self, id: u32) -> Option<&Slot> {
        self.slots.iter().find(|s| s.id == id)
    }

    /// The slot covering cell `(x, y)`, if one does.
    pub fn at(&self, x: u32, y: u32) -> Option<&Slot> {
        self.slots.iter().find(|s| s.covers(x, y))
    }

    /// Whether `foot` — turned, if `turned` — would lie at `(x, y)` on a
    /// grid of `capacity` cells: every cell of it on the grid and covered
    /// by no slot, bar the one `ignoring`, which is the slot being moved.
    pub fn fits(
        &self,
        capacity: u32,
        foot: Footprint,
        x: u32,
        y: u32,
        turned: bool,
        ignoring: Option<u32>,
    ) -> bool {
        let laid = if turned { foot.turned() } else { foot };
        for cy in y..y + laid.rows as u32 {
            for cx in x..x + laid.cols as u32 {
                if !Grid::on_grid(capacity, cx, cy) {
                    return false;
                }
                if self
                    .slots
                    .iter()
                    .any(|s| Some(s.id) != ignoring && s.covers(cx, cy))
                {
                    return false;
                }
            }
        }
        true
    }

    /// The first place `foot` fits: row by row from the top left, the
    /// whole grid unturned before any of it turned, so a rifle lies along
    /// a row while any row has room for it and stands up only when none
    /// has.
    pub fn first_fit(&self, capacity: u32, foot: Footprint) -> Option<(u8, u8, bool)> {
        for turned in [false, true] {
            if turned && foot.rows == foot.cols {
                break;
            }
            for y in 0..Grid::rows(capacity) {
                for x in 0..GRID_COLS {
                    if self.fits(capacity, foot, x, y, turned, None) {
                        return Some((x as u8, y as u8, turned));
                    }
                }
            }
        }
        None
    }

    /// Lay `kept` in the first place it fits, `count` in the slot, and
    /// say which slot it got; `None`, and nothing changed, when nowhere
    /// fits.
    pub fn place(&mut self, capacity: u32, kept: Kept, count: u32, foot: Footprint) -> Option<u32> {
        let (x, y, turned) = self.first_fit(capacity, foot)?;
        let id = self.next;
        self.next += 1;
        self.slots.push(Slot {
            id,
            kept,
            count,
            foot,
            x,
            y,
            turned,
        });
        Some(id)
    }

    /// Put `units` more of a resource on the grid as it stands: into the
    /// stacks with room first, the lowest id first, then new stacks laid
    /// where they fit. What could not be put anywhere is handed back.
    pub fn add(&mut self, capacity: u32, resource: ResourceId, units: u32, foot: Footprint) -> u32 {
        let mut left = units;
        for slot in self.slots.iter_mut() {
            if left == 0 {
                break;
            }
            if slot.kept == Kept::Stack(resource) {
                let take = slot.room().min(left);
                slot.count += take;
                left -= take;
            }
        }
        let size = stack_size(resource).max(1);
        while left > 0 {
            let count = left.min(size);
            if self
                .place(capacity, Kept::Stack(resource), count, foot)
                .is_none()
            {
                break;
            }
            left -= count;
        }
        left
    }

    /// Whether `units` more of a resource could be put on the grid as it
    /// is now — what a purchase or a stow asks before the count moves.
    /// The grid is not changed.
    pub fn can_take(
        &self,
        capacity: u32,
        resource: ResourceId,
        units: u32,
        foot: Footprint,
    ) -> bool {
        let mut trial = self.clone();
        trial.add(capacity, resource, units, foot) == 0
    }

    /// Take `units` of a resource off the grid — off the stack `from` if
    /// one is named, else the last stack first — and say how many came
    /// off. An emptied stack loses its slot.
    pub fn remove(&mut self, resource: ResourceId, units: u32, from: Option<u32>) -> u32 {
        let mut left = units;
        let mut order: Vec<usize> = self
            .slots
            .iter()
            .enumerate()
            .filter(|(_, s)| s.kept == Kept::Stack(resource))
            .map(|(i, _)| i)
            .collect();
        order.reverse();
        if let Some(id) = from
            && let Some(i) = self.slots.iter().position(|s| s.id == id)
        {
            order.retain(|&j| j != i);
            order.insert(0, i);
        }
        let mut emptied = Vec::new();
        for i in order {
            if left == 0 {
                break;
            }
            let take = self.slots[i].count.min(left);
            self.slots[i].count -= take;
            left -= take;
            if self.slots[i].count == 0 {
                emptied.push(self.slots[i].id);
            }
        }
        self.slots.retain(|s| !emptied.contains(&s.id));
        units - left
    }

    /// Take a slot off the grid whole: what it held, for the caller that
    /// is moving the thing.
    pub fn take(&mut self, id: u32) -> Option<Slot> {
        let i = self.slots.iter().position(|s| s.id == id)?;
        Some(self.slots.remove(i))
    }

    /// Move a slot to `(x, y)`, turned or not — `Command::Arrange`. `false`,
    /// and nothing changed, when it would not lie there or there is no
    /// such slot.
    pub fn arrange(&mut self, capacity: u32, id: u32, x: u32, y: u32, turned: bool) -> bool {
        let Some(i) = self.slots.iter().position(|s| s.id == id) else {
            return false;
        };
        let foot = self.slots[i].foot;
        if !self.fits(capacity, foot, x, y, turned, Some(id)) {
            return false;
        }
        let slot = &mut self.slots[i];
        slot.x = x as u8;
        slot.y = y as u8;
        slot.turned = turned;
        true
    }

    /// Lay everything out again from scratch, the biggest things first —
    /// which packs tighter than the order they happened to arrive in.
    /// Ids and the order of the list are kept; a slot that fits nowhere
    /// even so is dropped, and the settle lays it again when room is made.
    pub fn repack(&mut self, capacity: u32) {
        let mut order: Vec<usize> = (0..self.slots.len()).collect();
        order.sort_by_key(|&i| {
            let s = &self.slots[i];
            (u32::MAX - s.foot.cells(), s.id)
        });
        let mut laid = Grid {
            slots: Vec::new(),
            next: self.next,
        };
        for i in order {
            let slot = self.slots[i];
            if let Some((x, y, turned)) = laid.first_fit(capacity, slot.foot) {
                laid.slots.push(Slot {
                    x,
                    y,
                    turned,
                    ..slot
                });
            }
        }
        laid.slots.sort_by_key(|s| s.id);
        *self = laid;
    }

    /// The slots against what the class holds — see the module note for
    /// the rule. Anything not wanted at all goes first, so its cells are
    /// free for what is; a piece or a gun short is laid, one over is
    /// dropped (the highest id first); a resource's stacks are topped up
    /// or emptied to its count; and if something fits nowhere the grid is
    /// laid again from scratch once and it is tried again. Anything that
    /// still fits nowhere stays unplaced, for the next settle. Idempotent.
    pub fn settle(&mut self, capacity: u32, wanted: &[Wanted]) {
        let is_wanted = |s: &Slot| {
            wanted.iter().any(|w| match (w, s.kept) {
                (Wanted::Piece(id, _), Kept::Piece(kid)) => *id == kid,
                (Wanted::Gun(kind, tier, _), Kept::Gun(k, t)) => *kind == k && *tier == t,
                (Wanted::Units(id, units, _), Kept::Stack(rid)) => *id == rid && *units > 0,
                _ => false,
            })
        };
        self.slots.retain(is_wanted);
        // What is over, first: a gun's extra slot, a stack's extra units.
        for w in wanted {
            match *w {
                Wanted::Piece(..) => {}
                Wanted::Gun(kind, tier, _) => loop {
                    let have: Vec<u32> = self
                        .slots
                        .iter()
                        .filter(|s| s.kept == Kept::Gun(kind, tier))
                        .map(|s| s.id)
                        .collect();
                    let want = wanted
                        .iter()
                        .filter(|w| matches!(w, Wanted::Gun(k, t, _) if *k == kind && *t == tier))
                        .count();
                    if have.len() > want {
                        let last = *have.iter().max().expect("have is not empty");
                        self.take(last);
                    } else {
                        break;
                    }
                },
                Wanted::Units(id, units, _) => {
                    let have: u32 = self
                        .slots
                        .iter()
                        .filter(|s| s.kept == Kept::Stack(id))
                        .map(|s| s.count)
                        .sum();
                    if have > units {
                        self.remove(id, have - units, None);
                    }
                }
            }
        }
        // Then what is short, laid where it fits.
        let mut repacked = false;
        for w in wanted {
            let short = |grid: &Grid| -> u32 {
                match *w {
                    Wanted::Piece(id, _) => {
                        u32::from(!grid.slots.iter().any(|s| s.kept == Kept::Piece(id)))
                    }
                    Wanted::Gun(kind, tier, _) => {
                        let want = wanted
                            .iter()
                            .filter(
                                |w| matches!(w, Wanted::Gun(k, t, _) if *k == kind && *t == tier),
                            )
                            .count() as u32;
                        let have = grid
                            .slots
                            .iter()
                            .filter(|s| s.kept == Kept::Gun(kind, tier))
                            .count() as u32;
                        want.saturating_sub(have)
                    }
                    Wanted::Units(id, units, _) => {
                        let have: u32 = grid
                            .slots
                            .iter()
                            .filter(|s| s.kept == Kept::Stack(id))
                            .map(|s| s.count)
                            .sum();
                        units.saturating_sub(have)
                    }
                }
            };
            let lay = |grid: &mut Grid, n: u32| -> bool {
                match *w {
                    Wanted::Piece(id, foot) => {
                        grid.place(capacity, Kept::Piece(id), 1, foot).is_some()
                    }
                    Wanted::Gun(kind, tier, foot) => grid
                        .place(capacity, Kept::Gun(kind, tier), 1, foot)
                        .is_some(),
                    Wanted::Units(id, _, foot) => grid.add(capacity, id, n, foot) == 0,
                }
            };
            let n = short(self);
            if n == 0 {
                continue;
            }
            // A gun wanted twice over is one entry twice, so one at a time.
            let n = if matches!(w, Wanted::Gun(..)) { 1 } else { n };
            if lay(self, n) {
                continue;
            }
            if !repacked {
                repacked = true;
                self.repack(capacity);
                let n = short(self);
                if n > 0 {
                    let n = if matches!(w, Wanted::Gun(..)) { 1 } else { n };
                    lay(self, n);
                }
            }
        }
    }

    /// How many cells the slots cover.
    pub fn covered(&self) -> u32 {
        self.slots.iter().map(|s| s.foot.cells()).sum()
    }

    /// How many of a resource the slots hold between them.
    pub fn units_of(&self, resource: ResourceId) -> u32 {
        self.slots
            .iter()
            .filter(|s| s.kept == Kept::Stack(resource))
            .map(|s| s.count)
            .sum()
    }
}
