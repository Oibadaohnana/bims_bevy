//! The machines' jammer (feature 93): the one station in an infested
//! system that holds the hyperlanes shut.
//!
//! The crisis spreads along the lanes (`crate::droid`, feature 92) and a
//! jump follows them too — one hop a charge, and only down a lane
//! (`World::begin_jump`, [`crate::event::Refusal::NoLane`]). The jammer is
//! what makes that a corner rather than a map: while it stands, a ship in
//! an infested system may go **sideways or outward** — to a star the same
//! number of hops from the machines' origin, or further off — and may not
//! go **inward**, towards where they began
//! ([`crate::event::Refusal::Jammed`]). Flying *into* an infested system is
//! never refused: the trap is getting back out the way you came.
//!
//! # Which station holds it
//!
//! One a system, and the rule is written down once, in
//! `World::jammer_station`:
//!
//! - **the system's orbital station with the lowest id** — a town on a
//!   planet's surface is never it, since a system may have no orbit worth
//!   the name and every system has to have exactly one jammer;
//! - and where a system has no orbital station at all, a **derived** one:
//!   a station the machines put there themselves, built here rather than
//!   by the generator.
//!
//! A derived jammer is rolled off the star's own stream
//! ([`Purpose::Jammer`]) — its plan, its map seed, its shelf and the point
//! it stands on, which is clear of every node the way a jump's landing
//! point is ([`crate::jump::clear_point`]) — so two clients put the same
//! station in the same place without exchanging a word. Its id is
//! [`jammer_id`], in a range of its own well clear of the generator's, of
//! a raider's and of a surface's.
//!
//! **It is never saved.** `World::settle_jammer` strips whatever a save
//! carried and rolls it again, at the start, at a jump, at a load and the
//! step a system falls — so the station is always the seed's rather than
//! an old build's. What *is* saved is what happened to it: its waves and
//! whether it was cleared, which are an `Infestation` filed by station id
//! like any other station's.
//!
//! # Down for good
//!
//! Clearing the jammer station — the last machine of its last wave
//! destroyed, `World::droid_station_cleared` — lifts the jam and lifts it
//! for ever: the crisis step's record is what keeps it down through a
//! save, a load, and every later spread.

use worldgen::rng::{Purpose, Rng, seed_for};
use worldgen::{Name, StarSystem, StationBlueprint, StationKind, Stock};

use crate::jump;

/// The bit that marks a station id as a derived jammer's. Clear of the
/// generator's ids, of [`crate::raid::RAIDER_BASE`] and of
/// [`crate::surface::SURFACE_BASE`].
pub const JAMMER_BASE: u32 = 0x1000_0000;

/// The station id of the derived jammer in a star's system.
pub fn jammer_id(star: u32) -> u32 {
    JAMMER_BASE | star
}

/// Which star's derived jammer a station id names, if it names one.
pub fn jammer_star(id: u32) -> Option<u32> {
    (id & JAMMER_BASE != 0 && id & (crate::raid::RAIDER_BASE | crate::surface::SURFACE_BASE) == 0)
        .then_some(id & !JAMMER_BASE)
}

/// Whether this station id is a derived jammer's rather than a station the
/// generator numbered.
pub fn is_derived(id: u32) -> bool {
    jammer_star(id).is_some()
}

/// The kind a derived jammer is built as: a relay's. "Deep space, on its
/// own, listening" is what a relay is for, and it is what the machines
/// have made of this one.
pub const JAMMER_KIND: StationKind = StationKind::Relay;

/// The first ring of [`jump::clear_point`] a derived jammer is looked for
/// on: well outside the ring a jump lands on, so the two are never within
/// sight of one another however empty the system.
const JAMMER_FIRST_RING: u32 = 3;

/// How many rings out from [`JAMMER_FIRST_RING`] the roll may push it.
const JAMMER_RINGS: u32 = 4;

/// The station the machines put in a system with no orbit of its own to
/// take the jammer, rolled off the star's own stream.
///
/// The position is a clear point like a jump's landing, further out and on
/// a bearing of its own — half a step off every bearing
/// [`jump::landing_point`] tries, so the two can never come out the same
/// point whatever the system holds.
pub fn blueprint(system: &StarSystem, galaxy_seed: u64, star: u32) -> StationBlueprint {
    let base = seed_for(
        galaxy_seed,
        star,
        worldgen::GENERATOR_VERSION,
        Purpose::Jammer,
    );
    let stream = Rng::new(base);
    // Each off its own branch, the way a settlement's are, so reworking
    // one of them does not move the others.
    let map_seed = stream.branch(0x_4d41_5000_0000_0000).next_u64();
    let stock = Stock::roll(JAMMER_KIND, &mut stream.branch(0x_5354_4f43_4b00_0000));
    let bias = worldgen::data::price_bias(&mut stream.branch(0x_4249_4153_0000_0000));
    let mut place = stream.branch(0x_504c_4143_4500_0000);
    let ring = JAMMER_FIRST_RING + place.below(JAMMER_RINGS);
    // A sixteenth of the way round is the gap between two of the search's
    // bearings; a half-step inside one of sixteen sub-steps is never one
    // of them, so a landing and a jammer never share a candidate.
    let turn = (place.below(16) as f64 + 0.5) * core::f64::consts::TAU / 256.0;
    let position = jump::clear_point(system, ring, turn);
    // The name is the star's own catalogue number with a mark of its own,
    // so the row on the chart reads as a station rather than as a blank.
    let name = Name::station(
        (map_seed % worldgen::name::STATION_WORDS as u64) as u16,
        ((map_seed >> 16) % 1000) as u16,
        0,
    );
    StationBlueprint {
        id: jammer_id(star),
        kind: JAMMER_KIND,
        parent_body: None,
        position,
        name,
        salvage_sites: 0,
        hazard_sites: Vec::new(),
        map_seed,
        stock,
        bias,
        // The machines' own: nobody friendly ever stood on it. A held
        // station has no people at all (`World::people_of`), so this is
        // what it reads as the moment it is won back.
        hostile: true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data;
    use worldgen::{Galaxy, GalaxyType, Node};

    /// A derived jammer's id is its own: never a generated station's,
    /// never a raider's and never a surface's.
    #[test]
    fn a_derived_jammer_s_id_collides_with_nothing() {
        for star in [0u32, 1, 7, 512, 999] {
            let id = jammer_id(star);
            assert_eq!(jammer_star(id), Some(star));
            assert!(is_derived(id));
            assert!(crate::surface::surface_body(id).is_none());
            assert!(crate::raid::raider_index(id).is_none());
            // And the generator's own ids, which are small, are not it.
            assert!(!is_derived(star));
            assert!(!is_derived(crate::surface::surface_id(star)));
            assert!(!is_derived(crate::raid::raider_id(star)));
        }
    }

    /// It stands clear of everything the system holds, it is the same
    /// station every time, and it is never where a jump would land.
    #[test]
    fn a_derived_jammer_stands_clear_and_is_the_same_every_time() {
        let galaxy = Galaxy::new(0x1234_5678, GalaxyType::Round);
        for star in [0u32, 17, 400, 999] {
            let system = galaxy.system(star).unwrap();
            let one = blueprint(&system, galaxy.seed, star);
            let two = blueprint(&system, galaxy.seed, star);
            assert_eq!(one.position, two.position);
            assert_eq!(one.map_seed, two.map_seed);
            assert_eq!(one.id, jammer_id(star));
            assert!(one.hostile);
            for node in system.nodes() {
                let there = system.absolute_position(node).unwrap();
                assert!(
                    there.distance(one.position) >= data::JUMP_CLEARANCE,
                    "star {star}: {node:?} is on top of the jammer"
                );
            }
            assert_ne!(
                one.position,
                jump::landing_point(&system),
                "star {star}: a jump would land inside it"
            );
            // And once it is in the system, a landing is clear of it too.
            let mut with = system.clone();
            with.stations.push(one.clone());
            let at = jump::landing_point(&with);
            assert!(
                with.absolute_position(Node::Station(one.id))
                    .unwrap()
                    .distance(at)
                    >= data::JUMP_CLEARANCE
            );
        }
    }
}
