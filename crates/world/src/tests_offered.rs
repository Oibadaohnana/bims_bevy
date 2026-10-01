//! What a system offers (the galaxy-only map): one mission — its station
//! or its town, the home station in the crew's own system — or its trader
//! alone, and every star of the galaxy marked with it. The shared fixtures
//! keep whole systems (`World::set_whole_systems_for_probe`); these worlds
//! are opened without that dial.

use shipdesign::fixture::flyer;
use worldgen::GalaxyType;

use crate::data;
use crate::fixture::REFERENCE_MONEY;
use crate::run::{Site, SiteKind};
use crate::world::{World, offered};
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

#[test]
fn every_system_offers_one_mission_or_its_trader_and_home_is_its_own() {
    let world = offered_world();
    let galaxy = world.galaxy();

    // --- the home system: the home station alone ---
    let whole = galaxy.system(world.home_star).expect("the spawn's system");
    assert!(
        whole.stations.len() > 1,
        "the generator put more than one station here, or this proves nothing"
    );
    let (stations, towns) = split(&world.sites_at(world.home_star));
    assert_eq!(stations, vec![world.home], "home alone");
    assert!(towns.is_empty(), "no town beside home: {towns:?}");
    assert!(world.surfaces.is_empty());
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

    // --- every system of the galaxy: one site, its mission or its trader ---
    let (mut on_stations, mut in_towns, mut traders) = (0, 0, 0);
    for star in 0..galaxy.stars.len() as u32 {
        if star == world.home_star {
            continue;
        }
        let whole = galaxy.system(star).unwrap();
        let mut system = whole.clone();
        world.trim_system(star, &mut system);
        let made: Vec<u32> = system
            .stations
            .iter()
            .map(|s| s.id)
            .filter(|&id| !jammer::is_derived(id) && !heart::is_heart(id))
            .collect();
        let surfaces = world.offered_surfaces(&system);
        let fight = world.offered_fight(star, &system);
        assert_eq!(
            fight,
            world.offered_fight(star, &whole),
            "star {star}: the same whole or trimmed"
        );
        let mut again = system.clone();
        world.trim_system(star, &mut again);
        assert_eq!(again.stations.len(), system.stations.len(), "star {star}");
        assert_eq!(made.len() + surfaces.len(), 1, "star {star}: one site");
        let trader = made
            .first()
            .is_some_and(|&id| world.is_trader(Site { star, station: id }));
        match fight {
            None => {
                assert!(trader, "star {star}: no mission is a trader's");
                assert_eq!(made, vec![offered::primary(&whole.stations).unwrap()]);
                traders += 1;
            }
            Some(id) if surface::surface_body(id).is_some() => {
                assert_eq!(
                    surface::surface_body(id),
                    offered::town_body(&whole),
                    "star {star}: the lowest landable body's"
                );
                in_towns += 1;
            }
            Some(id) => {
                assert!(!trader);
                assert_eq!(made, vec![id]);
                assert_eq!(Some(id), offered::primary(&whole.stations), "star {star}");
                on_stations += 1;
            }
        }
    }
    // The coin falls both ways, and a trader is about one system in ten.
    let all = on_stations + in_towns + traders;
    assert!(
        on_stations * 4 > all && in_towns * 4 > all,
        "{on_stations} / {in_towns}"
    );
    assert!(
        traders * 25 > all && traders * 5 < all,
        "{traders} of {all}"
    );
}

/// The galaxy chart's marks: one a star, its mission's kind or the
/// trader, the same answer the list's quote gives — and both an attack and
/// a defence among them.
#[test]
fn every_star_is_marked_with_its_one_mission() {
    let mut world = offered_world();
    world.set_quiet_sites_for_probe(false);
    let galaxy = world.galaxy();
    let started = std::time::Instant::now();
    let marks = world.star_missions(&galaxy);
    eprintln!(
        "star_missions: {} stars in {:.1} ms",
        marks.len(),
        started.elapsed().as_secs_f64() * 1000.0
    );
    assert_eq!(marks.len(), galaxy.stars.len(), "a mark a star");
    let kinds = |kind| marks.iter().filter(|m| m.kind == kind).count();
    assert!(kinds(SiteKind::Attack) > 0 && kinds(SiteKind::Defend) > 0);
    assert!(kinds(SiteKind::Trader) > 0);
    // Home is the defence the run opens on.
    let home = marks
        .iter()
        .find(|m| m.site.star == world.home_star)
        .unwrap();
    assert_eq!(home.site.station, world.home);
    assert_eq!(home.kind, SiteKind::Defend);
    // Every mark a trip goes to is the site the list quotes, as the list
    // says.
    let quotes = world.travel_quotes();
    for mark in &marks {
        let Some((_, quote)) = quotes.iter().find(|(s, _)| *s == mark.site) else {
            continue;
        };
        if let Ok(quote) = quote {
            assert_eq!(quote.kind, mark.kind, "{mark:?}");
        }
    }
}
