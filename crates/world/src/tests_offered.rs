//! What a system offers (task 135): one station and one planet's town —
//! the home station in the crew's own system — its trader beside them
//! where it has one, and one fight a system. The shared fixtures keep
//! whole systems (`World::set_whole_systems_for_probe`); these worlds are
//! opened without that dial.

use shipdesign::fixture::flyer;
use worldgen::GalaxyType;

use crate::checksum::world_checksum;
use crate::data;
use crate::event::{Refusal, WorldEvent};
use crate::fixture::REFERENCE_MONEY;
use crate::run::{Phase, Site};
use crate::world::{Command, World, offered};
use crate::{heart, jammer, surface};

/// The game as it opens at the default seed's spawn, the sites quiet so
/// the fights a trip lands in are nobody's subject here.
fn offered_world() -> World {
    let galaxy = worldgen::Galaxy::new(data::DEFAULT_SEED, GalaxyType::SpiralTwoArm);
    let (star, station) = crate::spawn(&galaxy).expect("the default seed has a dock");
    let mut world = World::start(
        flyer(2),
        REFERENCE_MONEY,
        1,
        data::DEFAULT_SEED,
        GalaxyType::SpiralTwoArm,
        star,
        station,
    )
    .expect("the default seed has somewhere to spawn");
    world.set_quiet_sites_for_probe(true);
    world
}

/// The stations of a list that the generator made — no derived jammer and
/// no fortress — and the towns.
fn split(sites: &[Site]) -> (Vec<u32>, Vec<u32>) {
    let made = |id: u32| !jammer::is_derived(id) && !heart::is_heart(id);
    let stations = sites
        .iter()
        .map(|s| s.station)
        .filter(|&id| made(id) && surface::surface_body(id).is_none())
        .collect();
    let towns = sites
        .iter()
        .map(|s| s.station)
        .filter(|&id| surface::surface_body(id).is_some())
        .collect();
    (stations, towns)
}

/// The trip there: the one player's yes is all of it.
fn travel_to(world: &mut World, site: Site) -> Vec<WorldEvent> {
    world.step(&[Command::Propose {
        slot: 0,
        star: site.star,
        station: site.station,
    }])
}

#[test]
fn every_system_offers_one_station_and_one_town_and_home_is_its_own() {
    let world = offered_world();
    let galaxy = world.galaxy();

    // --- the home system: the home station and one town ---
    let whole = galaxy.system(world.home_star).expect("the spawn's system");
    assert!(
        whole.stations.len() > 1,
        "the generator put more than one station here, or this proves nothing"
    );
    let (stations, towns) = split(&world.sites_at(world.home_star));
    assert_eq!(stations, vec![world.home], "home alone");
    assert_eq!(towns.len(), 1, "one town: {towns:?}");
    assert_eq!(world.surfaces.len(), 1);
    assert_eq!(
        Some(surface::surface_body(towns[0]).unwrap()),
        offered::town_body(&whole),
        "the lowest landable body's"
    );
    assert!(
        world
            .stations
            .iter()
            .all(|s| s.id == world.home || jammer::is_derived(s.id) || heart::is_heart(s.id)),
        "the world holds nothing else"
    );
    assert!(
        world
            .trader_sites()
            .iter()
            .all(|s| s.star != world.home_star),
        "no trader at home"
    );

    // --- every system next door: its station, a trader maybe, a town ---
    let mut next_door = world.reachable_stars();
    next_door.dedup();
    assert!(!next_door.is_empty());
    for &star in &next_door {
        let sites = world.sites_at(star);
        let (stations, towns) = split(&sites);
        assert_eq!(towns.len(), 1, "star {star}: {sites:?}");
        let traders: Vec<u32> = stations
            .iter()
            .copied()
            .filter(|&id| world.is_trader(Site { star, station: id }))
            .collect();
        assert_eq!(
            stations.len(),
            1 + traders.len(),
            "star {star}: one station besides its trader: {sites:?}"
        );
        assert!(traders.len() <= 1);
        let whole = galaxy.system(star).unwrap();
        assert!(
            stations.contains(&offered::primary(&whole.stations).unwrap()),
            "star {star}: the lowest the generator made"
        );
    }

    // --- and every system of the galaxy trims to the same shape ---
    for star in 0..galaxy.stars.len() as u32 {
        if star == world.home_star {
            continue;
        }
        let mut system = galaxy.system(star).unwrap();
        world.trim_system(star, &mut system);
        let made = system
            .stations
            .iter()
            .filter(|s| !jammer::is_derived(s.id) && !heart::is_heart(s.id))
            .count();
        assert!((1..=2).contains(&made), "star {star}: {made} stations");
        assert_eq!(world.offered_surfaces(&system).len(), 1, "star {star}");
        assert_eq!(world.offered_fights(star, &system).len(), 2, "star {star}");
    }
}

#[test]
fn one_fight_a_system_and_the_other_is_refused_for_the_run() {
    let mut world = offered_world();
    // The first mission, at home, ended: the map is up.
    world.leave_for_probe();
    // A system next door with its station and its town both a trip.
    let (station, town) = world
        .reachable_stars()
        .into_iter()
        .find_map(|star| {
            let sites = world.sites_at(star);
            let fights: Vec<Site> = sites
                .iter()
                .copied()
                .filter(|&s| !world.is_trader(s) && !jammer::is_derived(s.station))
                .filter(|&s| !heart::is_heart(s.station))
                .collect();
            let station = fights
                .iter()
                .copied()
                .find(|s| surface::surface_body(s.station).is_none())?;
            let town = fights
                .iter()
                .copied()
                .find(|s| surface::surface_body(s.station).is_some())?;
            (world.travel_quote(station).is_ok() && world.travel_quote(town).is_ok())
                .then_some((station, town))
        })
        .expect("a system next door with both fights a trip");
    assert!(world.run.chosen.is_empty(), "nothing chosen at the start");
    let before = world_checksum(&world);

    let events = travel_to(&mut world, station);
    assert!(
        events
            .iter()
            .any(|e| matches!(e, WorldEvent::Travelled { .. })),
        "{events:?}"
    );
    assert_eq!(world.run.chosen, vec![station], "the station is chosen");

    world.leave_for_probe();
    assert_eq!(world.run.phase, Phase::Map);
    assert_eq!(
        world.travel_quote(town),
        Err(Refusal::OtherSiteChosen),
        "the town is the other fight"
    );
    let listed = world
        .travel_quotes()
        .into_iter()
        .find(|(s, _)| *s == town)
        .expect("still listed");
    assert_eq!(listed.1, Err(Refusal::OtherSiteChosen), "greyed on the map");
    let events = travel_to(&mut world, town);
    assert!(
        events.iter().any(|e| matches!(
            e,
            WorldEvent::Refused {
                why: Refusal::OtherSiteChosen,
                ..
            }
        )),
        "a vote for it is refused: {events:?}"
    );
    assert!(
        !events
            .iter()
            .any(|e| matches!(e, WorldEvent::Travelled { .. }))
    );

    // Home's town is another system's fight, and still a trip.
    let home_town = world
        .sites_at(world.home_star)
        .into_iter()
        .find(|s| surface::surface_body(s.station).is_some());
    if let Some(home_town) = home_town {
        assert_ne!(world.travel_quote(home_town), Err(Refusal::OtherSiteChosen));
    }

    // Whoever reads the world reads the choice: saved and hashed.
    assert_ne!(world_checksum(&world), before);

    // The tests' dial lifts the rule.
    world.set_whole_systems_for_probe(true);
    assert_ne!(world.travel_quote(town), Err(Refusal::OtherSiteChosen));
}
