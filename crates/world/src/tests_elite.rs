//! Elites (`crate::elite`): one system in ten holds one — its station the
//! machines' from the first day, an attack, at least two waves with a
//! Guardian a tier in the second — and only an elite drops relics.

use bims::combat::{Gear, Tier, WeaponKind};
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

/// **No elite on the floor's first five fights** (October 2026, the
/// player's: "they shouldn't spawn the first 5 fights"): an elite the
/// galaxy rolls on rows one to [`data::FLOOR_NO_ELITE_ROWS`] is a plain
/// fight; above them every one stays, tier one included, and off the
/// floor every one.
#[test]
fn the_floor_s_first_five_fights_hold_no_elite() {
    let mut world = open_crewed_world(combat_ship(), REFERENCE_MONEY, 1, COMBAT_CREW);
    let stars = world.galaxy().stars.len() as u32;
    // The fixture keeps whole systems: the machines' origin may roll one.
    let rolled =
        |world: &World, star: u32| crate::elite::holds(world.galaxy_seed, world.home_star, star);
    let off_floor: Vec<u32> = (0..stars).filter(|&s| rolled(&world, s)).collect();
    assert_eq!(world.elite_stars(stars), off_floor, "off the floor, all");

    // The default seed's first rows roll none, so a few more galaxies
    // under the same crew until an early one does.
    let (mut early, mut later) = (0, 0);
    for seed in data::DEFAULT_SEED..data::DEFAULT_SEED + 16 {
        world.galaxy_seed = seed;
        world.set_floor(true);
        let rows: Vec<(u32, u32)> = {
            let floor = world.floor().unwrap();
            let mut rows = Vec::new();
            for (row, nodes) in floor.rows.iter().enumerate() {
                for node in nodes {
                    rows.push((node.star, row as u32));
                }
            }
            rows
        };
        for &(star, row) in &rows {
            // A star on two rows is asked of the one nearest today.
            if rows.iter().any(|&(s, r)| s == star && r != row) {
                continue;
            }
            if !rolled(&world, star) {
                assert!(!world.holds_elite(star), "star {star}: never rolled");
                continue;
            }
            let keep = row > data::FLOOR_NO_ELITE_ROWS;
            assert_eq!(
                world.holds_elite(star),
                keep,
                "seed {seed}: star {star} on row {row}"
            );
            if keep {
                later += 1;
            } else {
                early += 1;
            }
        }
        if early > 0 && later > 0 {
            return;
        }
    }
    panic!("{early} early, {later} later");
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
fn an_elite_s_second_wave_has_two_guardians_at_tier_two_and_three_at_tier_three() {
    for (tier, want) in [(Tier::Two, 2), (Tier::Three, 3)] {
        let mut world = open_crewed_world(combat_ship(), REFERENCE_MONEY, 1, COMBAT_CREW);
        armed(&mut world);
        world.set_droid_tier_for_probe(Some(tier));
        let station = world.elite_dock_for_probe().expect("an elite in reach");
        world.set_droid_reinforce_minutes_for_probe(1.0);
        wave_up(&mut world);
        wreck_them_all(&mut world);
        let mut second = Vec::new();
        for _ in 0..20 {
            let kinds = wave_up(&mut world);
            if world.infestation(station).unwrap().wave == data::ELITE_GUARDIAN_WAVE {
                second = kinds;
                break;
            }
        }
        let guardians = second.iter().filter(|&&k| k == DroidKind::Guardian).count();
        assert!(
            guardians >= want,
            "{tier:?}: {guardians} Guardians in {second:?}"
        );
    }
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

/// **An elite pays more money an enemy down** (October 2026, the
/// player's word, beside its experience; twice, then half as much again
/// once the money was a site's budget): the bounty at an elite is
/// [`data::ELITE_BOUNTY_PERCENT`] of the same site's as a plain attack.
#[test]
fn an_elite_pays_its_bounty_twice_over() {
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
    world.infest(station);
    assert!(!world.is_elite_fight(station));
    let plain = world.bounty_here(1_000);
    assert_eq!(plain, 1_000, "an attack pays the bounty whole");
    world.set_elite_for_probe(station);
    assert!(world.is_elite_fight(station));
    assert_eq!(
        world.bounty_here(1_000),
        plain * u64::from(data::ELITE_BOUNTY_PERCENT) / 100
    );
}
