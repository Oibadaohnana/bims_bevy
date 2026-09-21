//! The native half of the cross-target check.
//!
//! [`crate::fixture`] pins what the reference galaxy's checksum comes out at.
//! This compares the native build against it; `lobby_galaxy_checksum_hi`/`_lo`
//! in `crates/lobby` is the wasm build's answer to the same question, read by
//! `scratchpad/builder-check.mjs` against the same constants. A target whose
//! arithmetic drifted fails exactly one of the two.

use crate::fixture::{REFERENCE_CHECKSUMS, REFERENCE_SEED, reference};
use crate::galaxy::GalaxyType;

#[test]
fn the_reference_galaxy_comes_out_at_the_numbers_it_is_pinned_to() {
    for &t in &GalaxyType::ALL {
        let got = reference(t).checksum();
        assert_eq!(
            got, REFERENCE_CHECKSUMS[t as usize],
            "{t:?} under seed {REFERENCE_SEED:#x} came out at {got:#018x}. If the \
             generator was meant to change, this is a GENERATOR_VERSION bump and \
             the pinned checksums in fixture.rs move with it."
        );
    }
}

/// A checksum that did not notice anything would pass the test above with
/// the wrong numbers written down.
/// The side a station is on is in the number: the same galaxy with one
/// station turned is a different checksum.
/// The desk's lean is in the number too: the same galaxy with one
/// station's ore a point dearer is a different checksum.
#[test]
fn the_checksum_notices_a_different_galaxy_a_side_change_and_a_lean() {
    // --- the_checksum_notices_a_different_galaxy ---
    {
        let a = reference(GalaxyType::Round).checksum();
        let b = crate::Galaxy::new(REFERENCE_SEED + 1, GalaxyType::Round).checksum();
        let c = reference(GalaxyType::Spiral).checksum();
        assert_ne!(a, b, "a different seed should show");
        assert_ne!(a, c, "a different type should show");
    }

    // --- the_checksum_notices_a_station_changing_sides ---
    {
        let galaxy = reference(GalaxyType::Round);
        let mut systems = galaxy.every_system();
        let pinned = crate::galaxy_checksum(&galaxy, &systems);
        let station = systems
            .iter_mut()
            .flat_map(|s| s.stations.iter_mut())
            .find(|s| s.kind != crate::StationKind::Derelict)
            .expect("a station somebody lives on");
        station.hostile = !station.hostile;
        assert_ne!(pinned, crate::galaxy_checksum(&galaxy, &systems));
    }

    // --- the_checksum_notices_a_station_leaning_on_a_price ---
    {
        let galaxy = reference(GalaxyType::Round);
        let mut systems = galaxy.every_system();
        let pinned = crate::galaxy_checksum(&galaxy, &systems);
        let station = systems
            .iter_mut()
            .flat_map(|s| s.stations.iter_mut())
            .find(|s| s.kind != crate::StationKind::Derelict)
            .expect("a station somebody lives on");
        station.bias.0[0] = station.bias.0[0].wrapping_add(1);
        assert_ne!(pinned, crate::galaxy_checksum(&galaxy, &systems));
    }
}

/// Every station's lean is inside the range the generator rolls, a
/// derelict's is nothing, and across the reference galaxy the roll is
/// reaching the desk: two stations lean different ways on something,
/// and every entry of the range turns up somewhere. The same seed twice
/// is the same lean.
#[test]
fn a_station_leans_on_every_price_inside_the_range() {
    use economy::market::{Bias, MAX_BIAS};
    let mut seen = std::collections::HashSet::new();
    let mut leaned = std::collections::HashSet::new();
    for &t in &GalaxyType::ALL {
        for system in reference(t).every_system() {
            for station in &system.stations {
                assert!(station.bias.in_range(), "{:?}", station.bias);
                if station.kind == crate::StationKind::Derelict {
                    assert_eq!(station.bias, Bias::NONE, "a derelict with a desk");
                } else {
                    seen.extend(station.bias.0.iter().copied());
                    leaned.insert(station.bias);
                }
            }
        }
    }
    assert_eq!(seen.len(), (2 * MAX_BIAS + 1) as usize, "{seen:?}");
    assert!(leaned.len() > 1, "every desk leans the same way");
    let twice = |t| {
        reference(t)
            .every_system()
            .iter()
            .flat_map(|s| s.stations.iter().map(|st| st.bias))
            .collect::<Vec<_>>()
    };
    assert_eq!(twice(GalaxyType::Round), twice(GalaxyType::Round));
}

/// The four pinned numbers are four *different* numbers, or a type is not
/// reaching the generator.
#[test]
fn every_type_is_pinned_to_its_own_number() {
    for (i, a) in REFERENCE_CHECKSUMS.iter().enumerate() {
        for b in &REFERENCE_CHECKSUMS[i + 1..] {
            assert_ne!(a, b);
        }
    }
}

/// A station's shelf is the kind's rule with a roll under it: nothing the
/// kind never sells, every staple the kind does, and — across the reference
/// galaxy — at least two stations of the same kind that stock different
/// things, or the roll is not reaching the shelf.
#[test]
fn a_station_stocks_the_staples_and_rolls_the_rest() {
    use crate::data::{STAPLES, StationKind, Stock};
    use physics::ResourceId;

    let galaxy = reference(GalaxyType::Round);
    let mut shelves: Vec<(StationKind, Stock)> = Vec::new();
    for star in &galaxy.stars {
        let system = galaxy.system(star.id).unwrap();
        for station in &system.stations {
            for &resource in ResourceId::ALL.iter() {
                if !station.kind.sells(resource) {
                    assert!(
                        !station.stock.sells(resource),
                        "{:?} {resource:?}",
                        station.kind
                    );
                }
                if STAPLES.contains(&resource) && station.kind.sells(resource) {
                    assert!(
                        station.stock.sells(resource),
                        "{:?} {resource:?}",
                        station.kind
                    );
                }
            }
            shelves.push((station.kind, station.stock));
        }
    }
    let differ = shelves
        .iter()
        .any(|&(kind, stock)| shelves.iter().any(|&(k, s)| k == kind && s != stock));
    assert!(differ, "every shelf of a kind came out the same");
    // And the same station again is the same shelf.
    let again = reference(GalaxyType::Round);
    for star in &galaxy.stars {
        let a = galaxy.system(star.id).unwrap();
        let b = again.system(star.id).unwrap();
        assert_eq!(a.stations, b.stations);
    }
}
