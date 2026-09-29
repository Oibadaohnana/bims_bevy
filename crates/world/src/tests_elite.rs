//! Elites (`crate::elite`): one system in ten holds one — its station the
//! machines' from the first day, an attack, at least two waves with a
//! Guardian in the second — and only an elite drops relics.

use bims::combat::{Gear, WeaponKind};
use bims::droid::{DroidKind, DroidPart};
use shipdesign::fixture::{COMBAT_CREW, combat_ship};
use worldgen::GalaxyType;

use crate::data;
use crate::fixture::{REFERENCE_MONEY, open_crewed_world};
use crate::run::{Phase, Site, SiteKind};
use crate::world::World;

/// The combat ship's crew, a gun in every hand.
fn armed(world: &mut World) {
    let kinds = WeaponKind::ALL.iter().copied().cycle();
    let crew = world.aboard.room.crew_count() as usize;
    for (who, kind) in kinds.take(crew).enumerate() {
        let gear = world.aboard.room.gear(who);
        world.aboard.room.issue(
            who,
            Gear {
                weapon: Some(kind.basic()),
                ..gear
            },
        );
    }
}

/// Steps until a wave stands in the residents' room: its kinds.
fn wave_up(world: &mut World) -> Vec<DroidKind> {
    for _ in 0..600 {
        world.step(&[]);
        if world.droids_standing() > 0 {
            let room = &world.residents.as_ref().unwrap().aboard.room;
            return (0..room.droid_count() as usize)
                .filter_map(|i| room.droid(i).filter(|d| !d.destroyed).map(|d| d.kind))
                .collect();
        }
    }
    panic!("no machines ever stood up");
}

/// Every machine standing wrecked where it stands.
fn wreck_them_all(world: &mut World) {
    let residents = world.residents.as_mut().unwrap();
    let n = residents.aboard.room.droid_count() as usize;
    for i in 0..n {
        residents
            .aboard
            .room
            .strike_droid(i, DroidPart::Chassis, 1e6);
    }
}

#[test]
fn about_one_system_in_ten_holds_an_elite_and_never_the_manufacturers() {
    let world = open_crewed_world(combat_ship(), REFERENCE_MONEY, 1, COMBAT_CREW);
    let galaxy = world.galaxy();
    let stars = galaxy.stars.len() as u32;
    let elites = world.elite_stars(stars);
    let share = elites.len() as u32 * 100 / stars;
    assert!((6..=14).contains(&share), "{share}% of systems");
    assert!(!elites.contains(&world.home_star), "never home");
    for &star in &elites {
        let system = galaxy.system(star).unwrap();
        let station = world
            .elite_station(star, &system.stations)
            .expect("an elite's station");
        assert_eq!(
            Some(station),
            crate::world::offered::primary(&system.stations)
        );
        let blueprint = system.station(station).unwrap();
        assert!(
            !world.is_manufacturer_site(star, blueprint),
            "star {star}: never the Manufacturers'"
        );
        assert!(
            !world.outposts_of(star, &system).contains(&station),
            "star {star}: never an outpost's coin"
        );
        assert!(world.is_elite(Site { star, station }));
    }
}

#[test]
fn an_elite_is_the_machines_two_waves_a_guardian_in_the_second_and_relics() {
    let mut world = open_crewed_world(combat_ship(), REFERENCE_MONEY, 1, COMBAT_CREW);
    armed(&mut world);
    let station = world.elite_dock_for_probe().expect("an elite in reach");
    assert!(world.is_elite_here(station));
    assert!(
        world.is_droid_held(station),
        "the machines' from the first day"
    );
    assert_eq!(world.site_kind(station), SiteKind::Attack);
    world.set_droid_reinforce_minutes_for_probe(1.0);

    let first = wave_up(&mut world);
    assert!(!first.is_empty());
    let it = world.infestation(station).unwrap();
    assert!(
        it.wave + it.waves_left >= data::ELITE_WAVES,
        "at least two waves: {} and {} to come",
        it.wave,
        it.waves_left
    );
    // The first down, the second lands with its Guardian.
    wreck_them_all(&mut world);
    let mut second = Vec::new();
    for _ in 0..20 {
        let kinds = wave_up(&mut world);
        if world.infestation(station).unwrap().wave == data::ELITE_GUARDIAN_WAVE {
            second = kinds;
            break;
        }
    }
    assert!(
        second.contains(&DroidKind::Guardian),
        "a Guardian in the second wave: {second:?}"
    );
    // Every wave down, the site cleared: the reward screen on leaving.
    for _ in 0..40 {
        if world.droid_station_cleared(station) {
            break;
        }
        wreck_them_all(&mut world);
        for _ in 0..200 {
            world.step(&[]);
            if world.droid_station_cleared(station) || world.droids_standing() > 0 {
                break;
            }
        }
    }
    assert!(world.droid_station_cleared(station), "the elite cleared");
    world.leave_for_probe();
    assert_eq!(world.run.phase, Phase::Reward, "an elite drops relics");
    assert!(world.relic_choice().is_some());
}

#[test]
fn a_fight_that_is_no_elite_drops_no_relic() {
    let galaxy = worldgen::Galaxy::new(data::DEFAULT_SEED, GalaxyType::SpiralTwoArm);
    let (star, station) = crate::spawn(&galaxy).unwrap();
    let mut world = World::start_with_crew(
        combat_ship(),
        REFERENCE_MONEY,
        1,
        COMBAT_CREW,
        data::DEFAULT_SEED,
        GalaxyType::SpiralTwoArm,
        star,
        station,
    )
    .unwrap();
    world.arena_dock_for_probe();
    armed(&mut world);
    world.infest(station);
    world.set_droid_reinforce_minutes_for_probe(1.0);
    world.set_droid_waves_for_probe(1);
    world.set_droid_wave_for_probe(3);
    assert!(!world.is_elite_here(station), "home is never an elite");
    assert!(!world.infestation(station).unwrap().cache, "no cache");
    wave_up(&mut world);
    wreck_them_all(&mut world);
    for _ in 0..200 {
        world.step(&[]);
        if world.droid_station_cleared(station) {
            break;
        }
    }
    assert!(world.droid_station_cleared(station));
    world.leave_for_probe();
    assert_eq!(world.run.phase, Phase::Map, "straight to the map");
    assert!(world.relic_choice().is_none(), "no relic offered");
}
