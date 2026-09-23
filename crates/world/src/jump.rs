//! The hyperdrive: a jump from one star to another.
//!
//! A ship with a working hyperdrive — `shipdesign::hyperdrive::ready`: a
//! drive bolted to a main engine and on a live network — can be charged
//! from the helm at any star it can see on the galaxy chart, and
//! [`crate::data::JUMP_CHARGE_MINUTES`] later it is **somewhere else**: the
//! same ship, the same crew, the same hold, in a system generated afresh
//! from the galaxy seed and the star's id the way the lobby generates one
//! to look at, standing still in **empty space**. Not at a station: a jump
//! lands wherever [`landing_point`] says, which is a point well clear of
//! everything the system holds, and the crew fly in from there like
//! anybody arriving.
//!
//! Only the ship makes the trip. What belonged to the system stays behind:
//! its stations and their people, the chart of it, the mining site, the
//! hostile list. `World::jump` is the one place that replaces them, so
//! there is one list of what a system is.
//!
//! The charge is the world's clock and nothing else — [`ShipState::Charging`]
//! (`crate::ShipState`) holds the star and when it began, the same shape
//! as a docking — so a browser at 24x and a server catching up land on the
//! same step. A charge is called off like a trip, with Abort, and the ship
//! is left holding where it was. There is no fuel and no charge to pay:
//! the drive draws its keep all day like a system, and the jump is free.

use worldgen::StarSystem;
use worldgen::math::{DVec2, dvec2};

use crate::data;

/// Where a jump into `system` puts the ship: somewhere empty.
///
/// Rings out from the system's origin, sixteen bearings a ring, the first
/// point at least [`data::JUMP_CLEARANCE`] from every node — deterministic
/// off the system alone, so two clients land the same ship in the same
/// place. The rings start at the clearance and step out by it, so a
/// crowded system lands the ship further out and an empty one close in;
/// past sixty-four rings the last candidate is taken whatever is near it,
/// which no generated system comes anywhere near needing.
pub fn landing_point(system: &StarSystem) -> DVec2 {
    clear_point(system, 1, 0.0)
}

/// The same search from a stated first ring and with every bearing turned
/// by `turn` radians: what [`landing_point`] is, and what the machines'
/// derived jammer station stands on (`crate::jammer`), which wants a point
/// as clear as a landing's but **not the landing's own**. Sixty-four rings
/// out from `first_ring`, sixteen bearings a ring.
pub fn clear_point(system: &StarSystem, first_ring: u32, turn: f64) -> DVec2 {
    let nodes: Vec<DVec2> = system
        .nodes()
        .into_iter()
        .filter_map(|n| system.absolute_position(n))
        .collect();
    let mut last = DVec2::ZERO;
    for ring in first_ring..first_ring.saturating_add(64) {
        let radius = data::JUMP_CLEARANCE * ring as f64;
        for i in 0..16u32 {
            // Off the bearing a little each ring, so the candidates do not
            // line up along sixteen spokes.
            let a = (i as f64 + 0.5 * (ring % 2) as f64) * std::f64::consts::TAU / 16.0 + turn;
            let at = dvec2(radius * a.sin(), radius * a.cos());
            last = at;
            if nodes.iter().all(|n| n.distance(at) >= data::JUMP_CLEARANCE) {
                return at;
            }
        }
    }
    last
}

#[cfg(test)]
mod tests {
    use super::*;
    use worldgen::{Galaxy, GalaxyType};

    #[test]
    fn a_landing_is_clear_of_everything_and_the_same_every_time() {
        let galaxy = Galaxy::new(0x1234_5678, GalaxyType::Round);
        for star in [0u32, 17, 400, 999] {
            let system = galaxy.system(star).unwrap();
            let at = landing_point(&system);
            assert_eq!(at, landing_point(&system));
            for node in system.nodes() {
                let there = system.absolute_position(node).unwrap();
                assert!(
                    there.distance(at) >= data::JUMP_CLEARANCE,
                    "star {star}: {node:?} at {there:?} is within {} of {at:?}",
                    data::JUMP_CLEARANCE
                );
            }
        }
    }
}
