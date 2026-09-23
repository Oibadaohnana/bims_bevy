//! An enemy's shelf, laid out as loot.
//!
//! A friendly station's shelf is reached across its desk — `World::buy` —
//! and a stranger's the same; the crew never take from either. An
//! **enemy's** shelf is a different thing: a raider's, or a hostile
//! station's the crew fought their way into. Nobody there sells, and the
//! game had no way to take from a shelf but the desk, so what a raid left
//! the crew was the boarders' bodies and nothing else. This is the other
//! half: the moment the rooms are joined at a station whose people are
//! enemies ([`World::stance`] hostile — the raider is on the list from
//! the moment it exists), its shelf is laid out **once** as a [`Grid`] of
//! its own — one stack of everything the station's kind stocks
//! (`Station::stock`, the same bits the desk sells by) to
//! [`data::PLUNDER_STACKS_MAX`], rolled off the station's own seed on a
//! branch of its own — and kept on the world by the station's id
//! ([`World::plunder`]). Taken from, it stays taken from: come back and
//! the shelf is as it was left. Cast off from a raider and its shelf goes
//! with it; a jump leaves every one behind with the system.
//!
//! **Taking is [`Command::Plunder`]**: crew member `who` within
//! [`data::REACH`] of one of the *station's* shelves on the joined deck —
//! the ones standing in the station's box, told apart from the ship's the
//! way its research desk is ([`World::station_shelves`]) — takes a stack
//! by its slot id, as many of it as the pack has cells for, one to a cell
//! the way a fetch lands one. The station's shelves are **not** containers
//! of the ship's hold: `World::container_takes` says no for them, so a
//! stow from beside one is refused `OutOfReach` and nothing of the crew's
//! goes onto an enemy's shelf.
//!
//! The grid is in `world_checksum` whole, like the hold's: two crews who
//! plundered a raider differently have different worlds, and a save
//! carries it.

use economy::{footprint, stack_size};
use physics::ResourceId;
use worldgen::rng::Rng;

use crate::data;
use crate::grid::Grid;
use crate::station::Station;

/// One enemy's shelf as the crew found it and have left it.
#[derive(Clone, PartialEq, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Plunder {
    /// Whose: a station's id, a raider's included.
    pub station: u32,
    /// What lies on it — stacks only: nobody's shelf stocks armour or
    /// guns, which are made and never sold (`StationKind::sells`).
    pub grid: Grid,
    /// The grid's size in cells: the station's shelves' capacity, the
    /// same rule as the ship's (`ShipDesign::capacity`).
    pub capacity: u32,
}

/// The branch of a station's stream its shelf's contents are rolled off:
/// "LOOT".
const BRANCH: u64 = 0x_4c4f_4f54_0000_0000;

/// Lay a station's shelf out as loot: for every good its kind stocks, in
/// `ResourceId` order, one to [`data::PLUNDER_STACKS_MAX`] full stacks,
/// rolled off `Station::map_seed` — so two worlds lay the same raider's
/// shelf alike, and a change to one good's roll moves no other's. What
/// would not fit the shelves is left off; a derelict, which stocks
/// nothing, has an empty grid.
pub fn lay_out(station: &Station) -> Plunder {
    let capacity = station.design.capacity(economy::Storage::Locker);
    let mut grid = Grid::default();
    let mut roll = Rng::new(station.map_seed).branch(BRANCH);
    for &resource in ResourceId::ALL.iter() {
        // Drawn for every good whether stocked or not, so a good that
        // comes into stock later moves nothing after it.
        let stacks = 1 + roll.below(data::PLUNDER_STACKS_MAX);
        if !station.stock.sells(resource) {
            continue;
        }
        let units = stacks * stack_size(resource).max(1);
        grid.add(capacity, resource, units, footprint(resource));
    }
    Plunder {
        station: station.id,
        grid,
        capacity,
    }
}
