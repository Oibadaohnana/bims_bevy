//! The game world: a star system, the ship in it, and everything aboard, all
//! advancing on **one clock**.
//!
//! The design phase is the only phase before this one. After Accept, a
//! [`World`] opens with the accepted ship docked at the spawn station and what
//! was left of the pool in the crew's hands, and from then on there is one
//! simulation and one loop: [`World::step`].
//!
//! It renders nothing and exports nothing to wasm. `crates/ship` draws it and
//! `crates/app` steps it, the same way `crates/shipdesign` is the rules and
//! `crates/ship` is the pointer. The split is not tidiness — a native server
//! has to run exactly this loop and reach exactly the same world, and two
//! implementations of it would be two different games.
//!
//! # The contract for the steps that come after this one
//!
//! Written down here because the whole point of the loop's shape is that the
//! things which do not exist yet have somewhere to go that is already decided.
//!
//! - **Bims live in ship-design tile coordinates.** The ship's position, its
//!   rotation and its acceleration do not reach them: a Bim walking to the
//!   galley is walking across a grid, and whether that grid happens to be
//!   pointing north or east is the renderer's business and nobody else's.
//! - **Every ship change goes through [`World::on_ship_changed`].** It is the
//!   one place the dynamics are recomputed, and what it promises — the anchor
//!   and the hull do not move, the centre of mass does — is what stops a
//!   welded wall shoving the ship sideways through space.
//! - **Construction and deconstruction follow the mass conservation
//!   contract** in `shipdesign::materials`, use only what is aboard, and ask
//!   [`World::can_modify_part`] first.
//! - **Money is used only while docked.** Away from a station there is nobody
//!   to buy from, and what gets built comes out of the hold or does not get
//!   built.
//! - **Crew, construction and health run inside [`World::step`]**, at the
//!   numbered extension points, on this clock. Not on a second one.
//! - **The design's exposure map is the radiation input.**
//!   `shipdesign::exposure` says which tiles the outside can see into, and
//!   `crates/health` says what standing in one does to a body.
//!
//! # What is deliberately absent
//!
//! Networking, interstellar travel, moving bodies, gravity, oxygen,
//! prices that differ by where you are, manual flight, and any speed above
//! [`data::TOP_SPEED`]. Stations have interiors now — [`station`] — and the
//! ship docks beside one rather than inside it and, docked, shares a room
//! with it ([`docking`]); but a station is still not a solid a trip has to
//! fly round.

pub mod armour;
pub mod build;
pub mod checksum;
pub mod class;
pub mod crew;
pub mod data;
pub mod deploy;
pub mod docking;
pub mod event;
pub mod fixture;
pub mod frame;
pub mod grid;
pub mod jump;
pub mod memory;
pub mod mercenary;
pub mod mining;
pub mod plunder;
pub mod raid;
pub mod speed;
pub mod station;
pub mod surface;
pub mod world;

pub use armour::{FetchKind, LootSource, Piece, Where};
pub use build::{BuildSite, SiteRefusal};
pub use checksum::world_checksum;
pub use class::{Class, Progress, Side, Talent};
pub use deploy::{Deck, DeployKind, Deployable, Kit};
pub use event::{Refusal, WorldEvent};
pub use frame::Frame;
pub use grid::{Grid, Kept, Slot, Wanted};
pub use memory::{Losses, SystemMemory};
pub use mining::{MiningSite, Rock, RockTile};
pub use plunder::Plunder;
pub use raid::{Raid, Raids, boarders_of, raider_id, raider_index};
pub use speed::Speed;
pub use station::{Berth, Plan, Station, layout_surface};
pub use surface::{Biome, Surface, landable, surface_body, surface_id};
pub use world::{
    Command, Power, Preview, Ship, ShipState, StartError, UPGRADE_ORDER, Upgrade, Workbench, World,
    spawn, spawn_anywhere, spawn_with_ground,
};

// The three things a caller of this crate wants from the ones underneath it,
// re-exported so it does not have to depend on all four for the sake of a
// type: a target to fly to, a reason it could not be, and where a trip has
// got to.
pub use flight::{Phase, PlanError, Target};

#[cfg(test)]
mod tests;
#[cfg(test)]
mod tests_engineer;
#[cfg(test)]
mod tests_memory;
#[cfg(test)]
mod tests_orders;
#[cfg(test)]
mod tests_plunder;
#[cfg(test)]
mod tests_raid;
#[cfg(test)]
mod tests_surface;
