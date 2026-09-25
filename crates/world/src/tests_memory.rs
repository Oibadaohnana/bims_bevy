//! What a system has to remember: `crate::memory`. A station's dead stay
//! dead when its room is closed and opened again, and a mercenary hired
//! is not there to hire twice; a jump away and back finds the system as
//! it was left — the keys, the lamps, the chart, the losses and the
//! dead — and the checksum knows the difference.

use shipdesign::fixture::flyer;

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

/// The ship, holding, jumped to `star` — what a trip across a hyperlane
/// does on the way (`World::jump`) — and a step after it.
fn jump_to(world: &mut World, star: u32) {
    assert_eq!(
        world.ship.state,
        ShipState::Holding,
        "a jump wants a holding ship"
    );
    let mut events = Vec::new();
    assert!(world.jump(star, &mut events), "the ship never jumped");
    assert!(
        events
            .iter()
            .any(|e| matches!(e, WorldEvent::Jumped { star: s } if *s == star)),
        "{events:?}"
    );
    assert_eq!(world.star_id, star);
    world.step(&[]);
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

/// A station's people shot down stay down: the room closes with two of
/// its own dead and opens again with two fewer on their feet, closes
/// with the rest dead and opens again empty — and the count is in the
/// checksum.
#[test]
fn a_station_s_dead_stay_dead_when_its_room_is_closed_and_opened_again() {
    let mut world = basic();
    let station = world.ship.state.station().expect("docked at the spawn");
    let (people, mercs) = {
        let s = world.station(station).unwrap();
        (world.people_of(s), world.mercenaries_of(s))
    };
    let crowd = world.residents.as_ref().unwrap().aboard.count();
    assert_eq!(crowd, people + mercs, "its people, then its hands for hire");
    assert!(people >= 2, "two of its own to shoot: {people}");
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
    assert_eq!(world.stance(station), Stance::Friendly, "home still");

    // Back: the survivors, and the two dead lying where they fell
    // (feature 85) — two fewer on their feet, the same number of bodies
    // on the deck.
    back_to(&mut world, station);
    let ashore = world.residents.as_ref().unwrap();
    assert_eq!(ashore.aboard.count(), crowd, "the dead are still here");
    assert_eq!(standing(&world), crowd - 2, "two fewer stand up");

    // The rest dead too: the station is emptied, and stays so.
    {
        let ashore = world.residents.as_mut().unwrap();
        for who in 0..ashore.aboard.count() as usize {
            ashore.aboard.room.kill_for_probe(who);
        }
    }
    world.step(&[]);
    out_of_range(&mut world);
    assert_eq!(
        world.losses_at(station),
        Losses {
            station,
            dead: people,
            mercenaries: mercs
        }
    );
    back_to(&mut world, station);
    assert_eq!(standing(&world), 0, "an emptied station opens empty");
    assert_eq!(
        world.residents.as_ref().unwrap().aboard.count(),
        crowd,
        "and the dead are all still on its deck"
    );
    // Docked there, the same: nobody comes to the airlock.
    world.dock_for_probe(station);
    assert_eq!(standing(&world), 0);
    assert!(world.people_of(world.station(station).unwrap()) == 0);
}

/// How many of the station's people are on their feet: the bodies the
/// room holds less the dead lying about it.
fn standing(world: &World) -> u32 {
    let ashore = world.residents.as_ref().expect("a room ashore");
    (0..ashore.aboard.count())
        .filter(|&who| ashore.aboard.room.is_alive(who as usize))
        .count() as u32
}

/// Where each body lying on the station's deck is, and what is left on
/// it, in the room as it stands now.
fn bodies(world: &World) -> Vec<(worldgen::math::DVec2, bims::combat::Gear)> {
    let ashore = world.residents.as_ref().expect("a room ashore");
    (0..ashore.aboard.count())
        .filter(|&who| !ashore.aboard.room.is_alive(who as usize))
        .map(|who| {
            (
                ashore.aboard.position(who),
                ashore.aboard.room.gear(who as usize),
            )
        })
        .collect()
}

/// A body shot on a station's deck is still lying there when the crew
/// come back to it (feature 85): the room closes, the world keeps the
/// grave — where it fell and what is still on it — and the room that
/// opens next lays it out again, dead, in the same place, with the same
/// gun in its hand. Its pack is not restocked: a body looted stays
/// looted.
#[test]
fn a_station_s_dead_lie_where_they_fell_when_its_room_opens_again() {
    let mut world = basic();
    let station = world.ship.state.station().expect("docked at the spawn");
    let crowd = world.residents.as_ref().unwrap().aboard.count();
    assert!(crowd >= 2, "people to shoot: {crowd}");

    // One of them shot where it stands, and its pack emptied the way a
    // looting empties one.
    let shot = 1;
    {
        let ashore = world.residents.as_mut().unwrap();
        let mut gear = ashore.aboard.room.gear(shot);
        gear.pack = [None; bims::combat::PACK_CELLS];
        ashore.aboard.room.issue(shot, gear);
        ashore.aboard.room.kill_for_probe(shot);
    }
    world.step(&[]);
    let fell = {
        let ashore = world.residents.as_ref().unwrap();
        assert!(!ashore.aboard.room.is_alive(shot));
        (
            ashore.aboard.position(shot as u32),
            ashore.aboard.room.gear(shot),
        )
    };

    // The room closed: the world has the grave.
    world.undock_for_probe();
    out_of_range(&mut world);
    let graves = world.graves_at(station);
    assert_eq!(graves.len(), 1, "one body on its deck");
    assert_eq!(graves[0].station, station);
    assert_eq!(graves[0].gear, fell.1, "what was left on it");
    assert!(!graves[0].hired, "one of the station's own");

    // And the room that opens next has it lying there: one body, dead,
    // within a tile of where it fell, with what was on it still on it.
    back_to(&mut world, station);
    assert_eq!(standing(&world), crowd - 1);
    let lying = bodies(&world);
    assert_eq!(lying.len(), 1, "one body, not two and not none");
    let (at, gear) = lying[0];
    let tile = shipdesign::parts::TILE as f64;
    assert!(
        at.sub(fell.0).length() <= tile,
        "the body moved: {at:?} was {:?}",
        fell.0
    );
    assert_eq!(gear, fell.1, "and nothing was put back in its pack");

    // Closed and opened again, it is still the one body: a grave laid
    // out is not a fresh death, so nothing is counted twice.
    out_of_range(&mut world);
    assert_eq!(world.graves_at(station).len(), 1);
    assert_eq!(world.losses_at(station).dead, 1, "counted once");
    back_to(&mut world, station);
    assert_eq!(bodies(&world).len(), 1);
    assert_eq!(standing(&world), crowd - 1);
}

/// A planet's settlement is a station like any other, so its dead lie in
/// its street the same way (feature 85): laid where its people stood,
/// the room built again over them, and still there after the ship has
/// lifted off and come back down.
#[test]
fn a_settlement_s_dead_lie_in_its_street_too() {
    let mut world = basic();
    assert!(world.land_for_probe(), "somewhere to land");
    let town = world.ship.state.alongside().expect("set down at the town");
    let living = standing(&world);
    assert!(living >= 2, "somebody lives there: {living}");

    // Two of them dead where they stand, the town's room built over
    // them: two fewer on their feet, two bodies on its deck.
    assert!(world.lay_graves_for_probe(2), "two of the town dead");
    let laid = world.graves_at(town).to_vec();
    assert_eq!(laid.len(), 2);
    assert_eq!(standing(&world), living - 2);
    assert_eq!(bodies(&world).len(), 2);
    assert_eq!(
        world.losses_at(town).dead + world.losses_at(town).mercenaries,
        2
    );

    // Up and away, and down again: the same two, in the same places.
    world.undock_for_probe();
    out_of_range(&mut world);
    let kept = world.graves_at(town).to_vec();
    assert_eq!(kept.len(), 2);
    for (was, is) in laid.iter().zip(&kept) {
        // The spot goes through the room's own f32 on the way, so it
        // comes back within a ten-thousandth rather than exactly — far
        // inside the thousandth the checksum rounds to, and it settles
        // after the first round trip rather than drifting.
        assert!((was.x - is.x).abs() < 0.001 && (was.y - is.y).abs() < 0.001);
        assert_eq!((was.gear, was.hired), (is.gear, is.hired));
    }
    assert!(world.land_for_probe(), "down at the town again");
    world.step(&[]);
    let again = bodies(&world);
    assert_eq!(again.len(), 2);
    assert_eq!(standing(&world), living - 2);
    let tile = shipdesign::parts::TILE as f64;
    for grave in &laid {
        assert!(
            again
                .iter()
                .any(|(at, _)| at.sub(worldgen::math::dvec2(grave.x, grave.y)).length() <= tile),
            "no body where one fell: {grave:?} of {again:?}"
        );
    }
}

/// A jump away and back: the system is met as it was left — the key off
/// its desk, the lamp shot out, the chart, the dead — where a system
/// never visited is as the generator rolled it, and every system left is
/// in the checksum.
#[test]
fn a_jump_away_and_back_finds_the_system_as_it_was_left() {
    let mut world = basic();
    let from = world.star_id;
    let to = crate::tests::laned_star(&world);
    let station = world.ship.state.station().expect("docked at the spawn");

    // What happened here: one of the spawn's people shot.
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
    // And held at the belt, which closes the station's room.
    assert!(
        world.hold_at_belt_for_probe(),
        "the spawn system has a belt"
    );
    world.step(&[]);
    // The room closed by the flight to the belt counted the dead.
    assert!(world.residents.is_none());
    assert_eq!(world.losses_at(station).dead, 1);

    let left = (
        world.station_keys.clone(),
        world.lamps.clone(),
        world.discovered.clone(),
        world.losses.clone(),
    );
    assert!(!left.2.is_empty(), "the chart has something on it");

    // Away: the other system as the generator rolled it, and this one
    // filed under its star.
    jump_to(&mut world, to);
    assert_eq!(world.memories.len(), 1);
    assert_eq!(world.memories[0].star, from);
    assert_eq!(world.memories[0].losses, left.3);
    assert_eq!(
        world.station_keys,
        world.stations.iter().map(|s| s.key).collect::<Vec<_>>()
    );
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
    assert_eq!(world.station_keys, left.0);
    assert_eq!(world.lamps, left.1);
    assert_eq!(world.losses, left.3);
    for node in &left.2 {
        assert!(world.discovered.contains(node), "{node:?} forgotten");
    }
    assert_eq!(world.stance(station), Stance::Friendly, "home again");
    assert_eq!(world.station_keys[with_key], 0, "the key stays taken");
    // Its people: one fewer, still.
    let s = world.station(station).unwrap();
    assert_eq!(world.people_of(s), s.residents() - 1);

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

/// The dead go with the system (feature 85): a jump away files the
/// graves under the star with everything else the crew changed, a system
/// never visited has none, and the jump back finds the body still lying
/// on the deck it fell on.
#[test]
fn the_dead_stay_on_the_deck_across_a_jump_away_and_back() {
    let mut world = basic();
    let from = world.star_id;
    let to = crate::tests::laned_star(&world);
    let station = world.ship.state.station().expect("docked at the spawn");
    world
        .residents
        .as_mut()
        .unwrap()
        .aboard
        .room
        .kill_for_probe(0);
    world.step(&[]);
    world.undock_for_probe();
    out_of_range(&mut world);
    let laid = world.graves.clone();
    assert_eq!(laid.len(), 1);

    // Away: this system's dead are filed with it, and the one arrived
    // at has none of its own.
    jump_to(&mut world, to);
    assert!(world.graves.is_empty(), "another system's deck is clean");
    assert_eq!(
        crate::memory::memory_of(&world.memories, from)
            .unwrap()
            .graves,
        laid
    );
    let away = world_checksum(&world);

    // And back: the body is where it was, and the room opens over it.
    jump_to(&mut world, from);
    assert_eq!(world.graves, laid);
    assert_ne!(
        world_checksum(&world),
        away,
        "the graves are in the checksum"
    );
    back_to(&mut world, station);
    assert_eq!(bodies(&world).len(), 1, "the body is still on the deck");
    let mut forgetful = world;
    forgetful.graves.clear();
    forgetful.memories.iter_mut().for_each(|m| m.graves.clear());
    let bare = world_checksum(&forgetful);
    forgetful.graves = laid;
    assert_ne!(world_checksum(&forgetful), bare);
}

/// The map marks where the crew have been (feature 85): the station the
/// ship is docked at from the first step, the belt it holds at once it
/// is there, and nowhere it has only flown past. A jump files the list
/// with the system and starts the new one empty, and the stars are the
/// systems the ship has a memory of plus the one it is at.
#[test]
fn the_map_marks_where_the_ship_has_already_been() {
    use worldgen::Node;
    let mut world = basic();
    let from = world.star_id;
    let to = crate::tests::laned_star(&world);
    let station = world.ship.state.station().expect("docked at the spawn");
    world.step(&[]);
    assert!(
        world.visited.contains(&Node::Station(station)),
        "docked is been there: {:?}",
        world.visited
    );
    assert_eq!(world.stars_visited(), vec![from]);

    // The belt it goes and holds at, once it is holding there.
    assert!(
        world.hold_at_belt_for_probe(),
        "the spawn system has a belt"
    );
    world.step(&[]);
    world.step(&[]);
    let belt = world
        .visited
        .iter()
        .find(|n| matches!(n, Node::Body(_)))
        .copied()
        .expect("the belt it is holding at");
    let here = world.visited.clone();
    assert!(here.len() >= 2);

    // Away: the new system is a blank chart, and this one's is filed.
    jump_to(&mut world, to);
    assert!(!world.visited.contains(&Node::Station(station)));
    assert_eq!(
        crate::memory::memory_of(&world.memories, from)
            .unwrap()
            .visited,
        here
    );
    assert_eq!(world.stars_visited(), {
        let mut both = vec![from, to];
        both.sort_unstable();
        both
    });

    // And back: the marks are on it again.
    jump_to(&mut world, from);
    assert!(world.visited.contains(&Node::Station(station)));
    assert!(world.visited.contains(&belt));
    assert!(
        world
            .visited
            .windows(2)
            .all(|w| crate::world::node_key(&w[0]) < crate::world::node_key(&w[1])),
        "sorted, for the checksum: {:?}",
        world.visited
    );
}
