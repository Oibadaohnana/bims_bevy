//! What a system has to remember: `crate::memory`. A station's dead stay
//! dead when its room is closed and opened again, and a mercenary hired
//! is not there to hire twice; a jump away and back finds the system as
//! it was left — the stance, the keys, the sites, the shelves, the lamps,
//! the chart, the losses — and the checksum knows the difference.

use shipdesign::fixture::flyer;
use shipdesign::parts::PartKind;
use shipdesign::{Budget, Edit, Rotation, ShipDesign, apply};

use crate::Losses;
use crate::data;
use crate::event::WorldEvent;
use crate::fixture::{REFERENCE_MONEY, simulation_world};
use crate::world::{Command, LampDamage, ShipState, World};
use crate::world_checksum;
use bims::sight::Stance;

fn basic() -> World {
    simulation_world(flyer(2), REFERENCE_MONEY, 2)
}

/// The flyer with a drive, as `tests::jumper` builds it.
fn jumper() -> ShipDesign {
    let budget = Budget::new(10_000_000);
    let mut design = flyer(2);
    for (kind, origin) in [
        (PartKind::PowerConduit, (7, 16)),
        (PartKind::PowerConduit, (6, 16)),
        (PartKind::Hyperdrive, (5, 16)),
    ] {
        design = apply(
            &design,
            &budget,
            Edit::Place {
                kind,
                origin,
                rotation: Rotation::R0,
            },
        )
        .unwrap_or_else(|e| panic!("{kind:?} at {origin:?}: {e:?}"));
    }
    design
}

/// The ship, holding, charged and jumped to `star`.
fn jump_to(world: &mut World, star: u32) {
    assert_eq!(
        world.ship.state,
        ShipState::Holding,
        "a jump wants a holding ship"
    );
    world.man_the_helm_for_probe(0);
    let events = world.step(&[Command::Jump { slot: 0, star }]);
    assert!(
        events
            .iter()
            .any(|e| matches!(e, WorldEvent::Charging { star: s, .. } if *s == star)),
        "{events:?}"
    );
    let steps = (data::JUMP_CHARGE_MINUTES / data::STEP_MINUTES).ceil() as u32 + 5;
    for _ in 0..steps {
        let events = world.step(&[]);
        if events
            .iter()
            .any(|e| matches!(e, WorldEvent::Jumped { star: s } if *s == star))
        {
            assert_eq!(world.star_id, star);
            return;
        }
    }
    panic!("the ship never jumped");
}

/// The ship put well out of every station's range and stepped, so the
/// residents' room is closed.
fn out_of_range(world: &mut World) {
    let far = world
        .stations
        .iter()
        .map(|s| s.centre().x + s.radius())
        .fold(0.0, f64::max)
        + data::RESIDENTS_RANGE * 10.0;
    world.put_for_probe(worldgen::math::dvec2(far, 0.0));
    world.step(&[]);
    assert!(
        world.residents.is_none(),
        "out of range, the room is closed"
    );
}

/// Back within range of `station`'s hull and stepped, so its room opens.
fn back_to(world: &mut World, station: u32) {
    let s = world.station(station).unwrap();
    let at = s.centre().add(worldgen::math::dvec2(
        s.radius() + data::RESIDENTS_RANGE * 0.5,
        0.0,
    ));
    world.put_for_probe(at);
    world.step(&[]);
    assert_eq!(
        world.residents.as_ref().map(|r| r.station),
        Some(station),
        "back within range, the room is open"
    );
}

/// A hostile station's garrison shot down stays down: the room closes
/// with two dead and opens again with two fewer, closes with the rest
/// dead and opens again empty — and the count is in the checksum.
#[test]
fn a_station_s_dead_stay_dead_when_its_room_is_closed_and_opened_again() {
    let mut world = basic();
    let station = world.ship.state.station().expect("docked at the spawn");
    world.set_hostile(station, true);
    let garrison = world.residents.as_ref().unwrap().aboard.count();
    assert!(garrison >= 3, "a garrison to shoot: {garrison}");
    assert_eq!(world.losses_at(station), Losses::none(station));
    let before = world_checksum(&world);

    // Two of them dead, in their own room.
    {
        let ashore = world.residents.as_mut().unwrap();
        ashore.aboard.room.kill_for_probe(0);
        ashore.aboard.room.kill_for_probe(1);
    }
    world.step(&[]);
    {
        let ashore = world.residents.as_ref().unwrap();
        assert!(!ashore.aboard.room.is_alive(0) && !ashore.aboard.room.is_alive(1));
    }

    // Off the berth and away: the room closes, and the dead are counted.
    world.undock_for_probe();
    out_of_range(&mut world);
    assert_eq!(
        world.losses_at(station),
        Losses {
            station,
            dead: 2,
            mercenaries: 0
        }
    );
    assert_ne!(
        world_checksum(&world),
        before,
        "the losses are in the checksum"
    );
    assert_eq!(world.stance(station), Stance::Hostile);

    // Back: the survivors, every one alive.
    back_to(&mut world, station);
    let ashore = world.residents.as_ref().unwrap();
    assert_eq!(ashore.aboard.count(), garrison - 2, "two fewer stand up");
    for who in 0..ashore.aboard.count() as usize {
        assert!(ashore.aboard.room.is_alive(who));
    }

    // The rest dead too: the station is emptied, and stays so.
    {
        let ashore = world.residents.as_mut().unwrap();
        for who in 0..ashore.aboard.count() as usize {
            ashore.aboard.room.kill_for_probe(who);
        }
    }
    world.step(&[]);
    out_of_range(&mut world);
    assert_eq!(world.losses_at(station).dead, garrison);
    back_to(&mut world, station);
    assert_eq!(
        world.residents.as_ref().unwrap().aboard.count(),
        0,
        "a raided station opens empty"
    );
    // Docked there, the same: nobody comes to the airlock.
    world.dock_for_probe(station);
    assert_eq!(world.residents.as_ref().unwrap().aboard.count(), 0);
    assert!(world.people_of(world.station(station).unwrap()) == 0);
}

/// A jump away and back: the system is met as it was left — the stance
/// the crew gave a station, the key off its desk, the rocks mined and
/// marked at the belt, the enemy's shelf as it was plundered, the lamp
/// shot out, the chart, the dead — where a system never visited is as
/// the generator rolled it, and every system left is in the checksum.
#[test]
fn a_jump_away_and_back_finds_the_system_as_it_was_left() {
    let mut world = simulation_world(jumper(), REFERENCE_MONEY, 2);
    let from = world.star_id;
    let to = (from + 1) % world.galaxy().stars.len() as u32;
    let station = world.ship.state.station().expect("docked at the spawn");

    // What the crew did here: turned the spawn's people against them,
    // which lays their shelf out as loot, and shot one of the garrison.
    world.set_hostile(station, true);
    assert!(!world.plunder.is_empty(), "an enemy's shelf is laid out");
    world
        .residents
        .as_mut()
        .unwrap()
        .aboard
        .room
        .kill_for_probe(0);
    world.step(&[]);
    // Took the key off a desk.
    let with_key = world
        .station_keys
        .iter()
        .position(|&k| k > 0)
        .expect("a key on some desk");
    world.station_keys[with_key] = 0;
    // Shot a lamp of the station's out.
    world.lamps.push(LampDamage {
        station: Some(station),
        tile: (3, 3),
        health: 0.0,
    });
    // And mined at the belt: a rock gone and another marked.
    assert!(
        world.hold_at_belt_for_probe(),
        "the spawn system has a belt"
    );
    assert!(!world.sites.is_empty());
    let gone = world.sites[0].tiles.pop().expect("a rock");
    let marked = world.sites[0].tiles[0];
    world.step(&[Command::MarkRock {
        slot: 0,
        x: marked.x,
        y: marked.y,
    }]);
    assert_eq!(world.sites[0].marked, vec![(marked.x, marked.y)]);
    // The room closed by the flight to the belt counted the dead.
    assert!(world.residents.is_none());
    assert_eq!(world.losses_at(station).dead, 1);

    let left = (
        world.hostile.clone(),
        world.station_keys.clone(),
        world.sites.clone(),
        world.plunder.clone(),
        world.lamps.clone(),
        world.discovered.clone(),
        world.losses.clone(),
    );
    assert!(!left.5.is_empty(), "the chart has something on it");

    // Away: the other system as the generator rolled it, and this one
    // filed under its star.
    jump_to(&mut world, to);
    assert_eq!(world.memories.len(), 1);
    assert_eq!(world.memories[0].star, from);
    assert_eq!(world.memories[0].sites, left.2);
    assert_eq!(world.memories[0].losses, left.6);
    for s in &world.stations {
        assert_eq!(world.hostile.binary_search(&s.id).is_ok(), s.hostile);
    }
    assert_eq!(
        world.station_keys,
        world.stations.iter().map(|s| s.key).collect::<Vec<_>>()
    );
    assert!(world.sites.is_empty());
    assert!(world.plunder.is_empty());
    assert!(world.losses.is_empty());
    assert!(
        world.lamps.iter().all(|d| d.station.is_none()),
        "the station's lamps stayed behind: {:?}",
        world.lamps
    );
    let away = world_checksum(&world);

    // And back: as it was left. The chart may have grown by what the
    // sensors reached on the landing, never shrunk.
    jump_to(&mut world, from);
    assert_eq!(world.memories.len(), 2);
    assert_eq!(world.hostile, left.0);
    assert_eq!(world.station_keys, left.1);
    assert_eq!(world.sites, left.2);
    assert_eq!(world.plunder, left.3);
    assert_eq!(world.lamps, left.4);
    assert_eq!(world.losses, left.6);
    for node in &left.5 {
        assert!(world.discovered.contains(node), "{node:?} forgotten");
    }
    assert!(
        world.sites[0].tiles.iter().all(|t| *t != gone),
        "the rock stays mined"
    );
    assert_eq!(world.stance(station), Stance::Hostile);
    assert_eq!(world.station_keys[with_key], 0, "the key stays taken");
    // Its people: one fewer, still.
    let s = world.station(station).unwrap();
    let full = crate::station::enemies_of(
        world.aboard.crew_count(),
        world.worth(),
        world.start_worth,
        world.days_gone(),
    )
    .min(data::ENEMIES_MAX);
    assert_eq!(world.people_of(s), full - 1);

    // Every system left is in the checksum.
    let back = world_checksum(&world);
    assert_ne!(back, away);
    let mut forgetful = world;
    forgetful.memories.clear();
    assert_ne!(
        world_checksum(&forgetful),
        back,
        "the memories are in the checksum"
    );
}

/// A mercenary hired is off the station's offer: closed and opened again,
/// the room has one fewer for hire — and one dismissed back ashore is on
/// it again.
#[test]
fn a_mercenary_hired_is_not_there_to_hire_twice() {
    // The combat ship with one aboard: a bunk to spare for the hire.
    let galaxy = worldgen::Galaxy::new(data::DEFAULT_SEED, worldgen::GalaxyType::SpiralTwoArm);
    let (star, station) = crate::spawn(&galaxy).unwrap();
    let mut world = World::start_with_crew(
        shipdesign::fixture::combat_ship(),
        REFERENCE_MONEY,
        1,
        1,
        data::DEFAULT_SEED,
        worldgen::GalaxyType::SpiralTwoArm,
        star,
        station,
    )
    .unwrap();
    assert!(world.mercenary_for_probe(), "a mercenary at the dock");
    let s = world.station(station).unwrap().clone();
    let offer = world.mercenaries_of(&s);
    assert!(offer >= 1);
    let residents = world.residents.as_ref().unwrap();
    let ashore = residents.aboard.count();
    let merc = residents
        .fee
        .iter()
        .position(|f| f.is_some())
        .expect("one priced") as u32;
    let fee = residents.fee[merc as usize].unwrap();

    // Hired: the world's count says one fewer for hire here from now on.
    world.money = fee * 10;
    let at = world
        .body_position(crate::LootSource::Resident(merc))
        .expect("stands on the deck");
    world
        .aboard
        .room
        .put_for_probe(0, at + bims::math::vec2(30.0, 0.0));
    let events = world.step(&[Command::Hire {
        slot: 0,
        who: 0,
        resident: merc,
    }]);
    assert!(
        events.iter().any(|e| matches!(e, WorldEvent::Hired { .. })),
        "{events:?}"
    );
    assert_eq!(world.losses_at(station).mercenaries, 1);
    assert_eq!(world.mercenaries_of(&s), offer - 1);

    // Away and back: the room opens with the one fewer.
    world.undock_for_probe();
    out_of_range(&mut world);
    back_to(&mut world, station);
    assert_eq!(
        world.residents.as_ref().unwrap().aboard.count(),
        ashore - 1,
        "the hired hand is not for hire again"
    );
}
