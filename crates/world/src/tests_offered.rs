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
        if star == world.home_star || star == world.droid_origin() {
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

/// No exceptions: every star within a trip's reach lists exactly one site,
/// the machines' origin its Heart's fortress and nothing else — and in a
/// system the machines have, that one site is the jammer: no derived
/// station is ever laid beside it.
#[test]
fn every_system_is_one_site_and_the_jammer_is_that_site() {
    let mut world = offered_world();
    let galaxy = world.galaxy();
    let mut stars = world.reachable_stars();
    stars.extend(world.stars_two_lanes_off());
    stars.push(world.home_star);
    for &star in &stars {
        let sites = world.sites_at(star);
        assert_eq!(sites.len(), 1, "star {star}: {sites:?}");
    }
    // The origin, wherever it is: the fortress alone.
    let origin = world.droid_origin();
    assert_eq!(
        world.sites_at(origin),
        vec![Site {
            star: origin,
            station: heart::heart_id(origin)
        }]
    );
    assert!(
        !world.holds_elite(origin),
        "the origin is the Heart's, not an elite's"
    );
    // The machines in this system: the jammer is home's one site, and no
    // derived jammer stands beside it.
    world.set_droid_origin_for_probe(galaxy.lanes(world.home_star)[0]);
    world.set_crisis_first_day_for_probe(0);
    world.set_day_for_probe(world.infested_on(world.home_star));
    world.settle_crisis();
    assert!(world.infested(world.star_id));
    assert_eq!(world.jammer_station(), Some(world.home));
    assert!(
        world.stations.iter().all(|s| !jammer::is_derived(s.id)),
        "no derived jammer"
    );
    assert_eq!(world.sites_at(world.home_star).len(), 1);
}

/// A trader in a system the machines have is their jammer: an attack, a
/// mission when the crew go there, and once it is cleared the trader is
/// open the moment the crew are back aboard.
#[test]
fn a_fallen_trader_is_fought_for_and_trades_once_won() {
    let mut world = offered_world();
    world.leave_for_probe();
    let galaxy = world.galaxy();
    let from_home = galaxy.hops_from(world.home_star);
    // A trader a trip reaches, and an origin next to it further from home,
    // so its system falls before the crew's own does.
    let (trader, origin) = world
        .trader_sites()
        .into_iter()
        .filter(|&s| world.travel_quote(s).is_ok())
        .find_map(|s| {
            let origin = galaxy.lanes(s.star).iter().copied().find(|&o| {
                o != world.home_star && from_home[o as usize] > from_home[s.star as usize]
            })?;
            Some((s, origin))
        })
        .expect("a trader in reach with a star beyond it");
    world.set_droid_origin_for_probe(origin);
    world.set_crisis_first_day_for_probe(0);
    world.set_day_for_probe(world.infested_on(trader.star));
    assert!(!world.infested(world.star_id), "home still the crew's");
    let quote = world
        .travel_quote(trader)
        .expect("a fallen trader is a trip");
    assert!(quote.trader && quote.infested && quote.jammer);
    assert_eq!(quote.kind, SiteKind::Attack, "an attack, not a shop");

    world.step(&[crate::world::Command::Propose {
        slot: 0,
        star: trader.star,
        station: trader.station,
    }]);
    assert_eq!(world.run.phase, crate::run::Phase::Mission, "a fight there");
    assert!(world.is_droid_held(trader.station));
    assert_eq!(world.jammer_station(), Some(trader.station));
    assert_eq!(world.site_kind(trader.station), SiteKind::Attack);
    assert!(!world.at_trader());

    // Won: back aboard, and the trader's purchase order is up.
    world
        .infestation_mut_for_probe(trader.station)
        .expect("held")
        .cleared = true;
    world.leave_for_probe();
    assert!(world.at_trader(), "trading where the fight ended");
    assert!(world.trader_here(0).is_some());
    assert_eq!(world.site_kind(trader.station), SiteKind::Trader);
    assert!(!world.trader_closed_on(trader.star, world.days_gone()));
}
