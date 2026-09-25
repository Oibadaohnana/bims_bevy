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
//!   contract** in `shipdesign::materials`: a part is paid for out of the
//!   pool, and the one thing that changes the hull under the crew.
//! - **Goods change hands only while docked.** Away from a station there is
//!   nobody to buy from.
//! - **Crew and construction run inside [`World::step`]**, at the numbered
//!   extension points, on the mission clock. Not on a second one.
//! - **Nothing is flown.** A trip is chosen on the world map between
//!   missions and resolved in one go (`crate::run`): the world clock is put
//!   on by its length, and the crew arrive docked.
//!
//! # What is deliberately absent
//!
//! Moving bodies, gravity, oxygen, flight, radiation, and any speed above
//! [`data::TOP_SPEED`]. Stations have interiors — [`station`] — and the
//! ship docks beside one rather than inside it and, docked, shares a room
//! with it ([`docking`]).

pub mod armour;
pub mod build;
pub mod checksum;
pub mod class;
pub mod commander;
pub mod crew;
pub mod data;
pub mod defense;
pub mod deploy;
pub mod docking;
pub mod droid;
pub mod event;
pub mod fixture;
pub mod frame;
pub mod grid;
pub mod jammer;
pub mod jump;
pub mod medic;
pub mod memory;
pub mod mercenary;
pub mod orders;
pub mod run;
pub mod speed;
pub mod station;
pub mod surface;
pub mod tank;
pub mod world;

pub use armour::{FetchKind, LootSource, Piece, Where};
pub use build::{BuildSite, SiteRefusal};
pub use checksum::world_checksum;
pub use class::{Charge, Class, Progress, Side, Talent};
pub use commander::{Aura, Commander, SquadAsk, SquadKind, SquadOrder};
pub use defense::Defense;
pub use deploy::{Deck, DeployKind, Deployable, Kit};
pub use event::{Refusal, WorldEvent};
pub use frame::Frame;
pub use grid::{Grid, Kept, Slot, Wanted};
pub use jammer::{JAMMER_BASE, jammer_id, jammer_star};
pub use medic::Medic;
pub use memory::{Losses, SystemMemory};
pub use orders::Standing;
pub use run::{Departure, Fallen, Proposal, Run, Site, SiteSnapshot, TravelQuote};
pub use speed::Speed;
pub use station::{Berth, Plan, Station, layout_surface};
pub use surface::{Biome, Surface, landable, surface_body, surface_id};
pub use tank::Tank;
pub use world::{
    Command, Power, Ship, ShipState, StartError, UPGRADE_ORDER, Upgrade, Workbench, World, spawn,
    spawn_anywhere, spawn_with_ground,
};

#[cfg(test)]
mod tests;
#[cfg(test)]
mod tests_commander;
#[cfg(test)]
mod tests_crisis;
#[cfg(test)]
mod tests_defense;
#[cfg(test)]
mod tests_droid;
#[cfg(test)]
mod tests_engineer;
#[cfg(test)]
mod tests_front;
#[cfg(test)]
mod tests_guardian;
#[cfg(test)]
mod tests_jammer;
#[cfg(test)]
mod tests_medic;
#[cfg(test)]
mod tests_memory;
#[cfg(test)]
mod tests_mission;
#[cfg(test)]
mod tests_money;
#[cfg(test)]
mod tests_orders;
#[cfg(test)]
mod tests_run;
#[cfg(test)]
mod tests_soldier;
#[cfg(test)]
mod tests_standing;
#[cfg(test)]
mod tests_surface;
#[cfg(test)]
mod tests_survivors;
#[cfg(test)]
mod tests_tank;
