//! The hyperdrive's one rule: it is bolted to an engine, or it is furniture.
//!
//! A [`PartKind::Hyperdrive`] jumps the ship to another star, and what it
//! throws through the jump is the engines' push — so it has to be
//! **connected** to a main engine: a tile of its footprint four-neighbour
//! to a tile of a part that [`PartDef::pushes`](crate::parts::PartDef::pushes).
//! Not the conduit — that is power, and the drive wants it too, like any
//! consumer — but the block itself against the engine's block, the way a
//! reactor is the wire between two runs under it. One engine is enough,
//! and which size does not matter.
//!
//! [`connected`] is the question, asked here by the validator (a warning,
//! `IssueCode::HyperdriveUnconnected`, never an error — a ship that cannot
//! jump is still a ship) and by `world` before a charge is begun, so the
//! two cannot disagree about which drive works. Nothing here knows what a
//! jump is.

use crate::design::{PlacedPart, ShipDesign};
use crate::parts::PartKind;

/// Whether `drive` — a placed hyperdrive — touches a main engine: some tile
/// of its footprint is four-neighbour to some tile of an engine's.
pub fn connected(design: &ShipDesign, drive: &PlacedPart) -> bool {
    let own = drive.tiles();
    design
        .parts
        .iter()
        .filter(|p| p.kind.def().pushes())
        .any(|engine| {
            engine.tiles().iter().any(|&(ex, ey)| {
                own.iter().any(|&(x, y)| {
                    (x == ex && (y as i32 - ey as i32).abs() == 1)
                        || (y == ey && (x as i32 - ex as i32).abs() == 1)
                })
            })
        })
}

/// Every hyperdrive aboard that is not connected, by id, ascending. What
/// the validator points at.
pub fn unconnected(design: &ShipDesign) -> Vec<u32> {
    let mut out: Vec<u32> = design
        .parts
        .iter()
        .filter(|p| p.kind == PartKind::Hyperdrive && !connected(design, p))
        .map(|p| p.id)
        .collect();
    out.sort_unstable();
    out
}

/// Whether the ship has a hyperdrive that works: connected to an engine
/// **and** on a live network. What `world` asks before a charge; the
/// lowest-id such drive is the one that fires, though nothing reads which.
pub fn ready(design: &ShipDesign) -> bool {
    design
        .parts
        .iter()
        .filter(|p| p.kind == PartKind::Hyperdrive)
        .any(|p| connected(design, p) && crate::power::is_powered(design, p.id))
}
