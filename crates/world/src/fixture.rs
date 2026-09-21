//! One world, opened the same way every time.
//!
//! Here rather than in the tests for the reason `shipdesign::fixture` is:
//! **two targets have to agree about it.** The browser runs
//! [`World::step`](crate::World::step) in wasm and the native server that will
//! one day be authoritative runs the same loop for x86, and the only way to
//! find out whether they agree is to run a fixed scenario on both and compare
//! each against the same written-down number.
//!
//! The native end is `tests.rs`; the wasm end is `ship_self_check` in
//! `crates/ship`, which the node harness reads. Both compare against
//! [`REFERENCE_CHECKSUM`]. If one target's arithmetic ever drifts from the
//! other's, exactly one of those two fails.

use shipdesign::fixture::flyer;
use worldgen::GalaxyType;

use crate::data;
use crate::world::{Command, World};
use crate::{Speed, Target};

/// What the reference crew have left over from the design phase.
pub const REFERENCE_MONEY: economy::Money = 40_000;

/// How many steps [`reference_run`] takes. Ten game minutes at 1x, which is
/// long enough to get a trip planned, confirmed and turning and short enough
/// that the wasm half of the check does not hold up a page load.
pub const REFERENCE_STEPS: u32 = 600;

/// What [`reference_run`] comes out at.
///
/// Pinned rather than computed, for the same reason `REFERENCE_HASH` is: a
/// test comparing two computed values would pass happily while both were
/// wrong. Update it only when the scenario below is meant to change — or
/// the checksum's shape does: it moved when the raids went in (September
/// 2026), since the schedule is hashed from the first step, and again
/// when an enemy's shelf became loot (`crate::plunder`), since the list
/// of them is hashed whole.
pub const REFERENCE_CHECKSUM: u64 = 0x_9937_b08f_c1b9_0777;

/// A world with the flyable fixture docked at the simulation's spawn: the
/// default seed's first dock, which is where every fixture world starts.
pub fn reference_world() -> World {
    simulation_world(flyer(2), REFERENCE_MONEY, 2)
}

/// A world opened the way the simulation opens one: the default seed, a
/// two-arm spiral, and [`crate::spawn`]'s dock. What `nix run .#simulation`
/// does with [`shipdesign::fixture::playtest_ship`] and
/// [`data::SIMULATION_MONEY`], and what every fixture here does with its own
/// ship and purse.
pub fn simulation_world(
    design: shipdesign::ShipDesign,
    money: economy::Money,
    players: u32,
) -> World {
    let galaxy = worldgen::Galaxy::new(data::DEFAULT_SEED, GalaxyType::SpiralTwoArm);
    let (star, station) = crate::spawn(&galaxy).expect("the default seed has a dock somewhere");
    World::start(
        design,
        money,
        players,
        data::DEFAULT_SEED,
        GalaxyType::SpiralTwoArm,
        star,
        station,
    )
    .expect("the default seed should have somewhere to spawn")
}

/// Somewhere in the spawn system that is not where the ship is standing.
///
/// The lowest-numbered node that is not the dock, not the dock's own
/// parent body — which body that is depends on the generator, and a trip
/// to it from its orbit is a hop to the point over it rather than the
/// burn the checksum was pinned on — and not already there, so the
/// scenario does not depend on which of them the generator happened to
/// put nearest.
pub fn reference_target(world: &World) -> Target {
    let docked = match &world.ship.state {
        crate::ShipState::Docked { station } => Some(*station),
        _ => None,
    };
    let parent = docked.and_then(|id| world.system.station(id)?.parent_body);
    for node in world.system.nodes() {
        let target = match node {
            worldgen::Node::Station(id) if Some(id) == docked => continue,
            worldgen::Node::Body(id) if Some(id) == parent => continue,
            worldgen::Node::Body(id) => Target::Body(id),
            worldgen::Node::Station(id) => Target::Station(id),
        };
        if world.preview(target) != Err(flight::PlanError::AlreadyThere) {
            return target;
        }
    }
    Target::Point(worldgen::math::dvec2(0.0, 0.0))
}

/// The scenario: open a world, confirm a trip, and run for
/// [`REFERENCE_STEPS`].
///
/// Everything a checksum is meant to catch is in it — a plan made, a burn
/// drawing on the reactor, a heading turning through a trigonometric function, the local
/// frame changing as the ship leaves the dock.
pub fn reference_run() -> u64 {
    let mut world = reference_world();
    let target = reference_target(&world);

    // The second player at the helm, since the ship is flown from there;
    // both ask for a day a minute, so the effective speed is a decision that
    // was actually taken rather than the default. `Day` rather than `Top`:
    // the request's code is in the checksum, and code 4 is what the
    // reference was pinned with before 48x went in above it.
    world.man_the_helm_for_probe(1);
    world.step(&[
        Command::SetSpeed {
            slot: 0,
            speed: Speed::Day,
        },
        Command::SetSpeed {
            slot: 1,
            speed: Speed::Day,
        },
        Command::Confirm { slot: 1, target },
    ]);
    for _ in 1..REFERENCE_STEPS {
        world.step(&[]);
    }
    world.checksum()
}
