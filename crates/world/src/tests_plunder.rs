//! What an enemy's shelf has to be true of: `crate::plunder`. Laid out
//! once at a hostile dock from the station's stock, not at a friend's;
//! the station's shelves told from the ship's and kept out of the hold;
//! a stack taken into the pack as far as the pack goes, and the shelf
//! left as it was left; the refusals in order; two worlds on one seed
//! laid out alike. A raider's shelf is pinned next door in
//! `tests_raid.rs`, with the derelict it becomes.

use bims::combat::Item;
use bims::game::Container;
use physics::ResourceId;
use shipdesign::fixture::playtest_ship;

use crate::data;
use crate::event::{Refusal, WorldEvent};
use crate::fixture::simulation_world;
use crate::grid::Kept;
use crate::world::{Command, World};
use crate::world_checksum;

/// The playtest ship, with a crew of two: it has shelves of its own, which
/// the station's have to be told from.
fn basic() -> World {
    simulation_world(playtest_ship(), data::SIMULATION_MONEY, 2)
}

fn refused(events: &[WorldEvent], why: Refusal) -> bool {
    events
        .iter()
        .any(|e| matches!(e, WorldEvent::Refused { why: w, .. } if *w == why))
}

#[test]
fn an_enemy_s_shelf_is_laid_out_once_and_a_stack_comes_off_it_into_the_pack() {
    let mut world = basic();
    let station = world.ship.state.station().expect("docked at the spawn");
    let shelves = world.station_shelves();
    assert!(
        !shelves.is_empty(),
        "the station has shelves on the joined deck"
    );

    // A friend's shelf is the desk's to sell from: nothing to plunder, and
    // the command says why.
    assert!(world.plunder_alongside().is_none());
    assert!(world.plunder.is_empty());
    let events = world.step(&[Command::Plunder {
        slot: 0,
        who: 0,
        id: 1,
    }]);
    assert!(refused(&events, Refusal::NotHostile), "{events:?}");

    // Turned hostile under the crew, the shelf is laid out: a stack of
    // everything the station's kind stocks, to the cap, and nothing else.
    world.set_hostile(station, true);
    let stock = world.station(station).unwrap().stock;
    let laid = world.plunder_alongside().expect("an enemy's shelf").clone();
    assert_eq!(laid.station, station);
    assert!(!laid.grid.slots.is_empty());
    for slot in &laid.grid.slots {
        let Kept::Stack(resource) = slot.kept else {
            panic!("nobody's shelf stocks armour or guns: {slot:?}");
        };
        assert!(stock.sells(resource), "{resource:?} is not in stock");
    }
    for &resource in ResourceId::ALL.iter() {
        let units = laid.grid.units_of(resource);
        if stock.sells(resource) {
            let size = economy::stack_size(resource);
            assert!(
                units >= size && units <= size * data::PLUNDER_STACKS_MAX,
                "{resource:?}: {units}"
            );
        } else {
            assert_eq!(units, 0, "{resource:?}");
        }
    }
    // The station's shelves are not containers of the hold: standing at
    // one reaches nothing of the crew's.
    for &i in &shelves {
        assert!(!world.container_takes(Container::Shelf(i), ResourceId::Medkit));
    }
    let ships_own = (0..)
        .take_while(|&i| {
            world
                .aboard
                .room
                .container_frame(Container::Shelf(i))
                .is_some()
        })
        .find(|i| !shelves.contains(i))
        .expect("the ship has a shelf of its own");
    assert!(world.container_takes(Container::Shelf(ships_own), ResourceId::Medkit));

    // Out of reach from the ship's deck; in reach at the shelf.
    let ore = laid
        .grid
        .slots
        .iter()
        .find(|s| s.kept == Kept::Stack(ResourceId::Tofu))
        .expect("ore is a staple");
    assert!(!world.shelf_ashore_in_reach(0));
    let events = world.step(&[Command::Plunder {
        slot: 0,
        who: 0,
        id: ore.id,
    }]);
    assert!(refused(&events, Refusal::OutOfReach), "{events:?}");
    let spot = world.plunder_spot(0).expect("a shelf to walk to");
    world.aboard.room.put_for_probe(0, spot);
    assert!(world.shelf_ashore_in_reach(0));
    assert!(!world.shelf_ashore_in_reach(1), "the other is still aboard");

    // No such slot.
    let events = world.step(&[Command::Plunder {
        slot: 0,
        who: 0,
        id: 9_999,
    }]);
    assert!(refused(&events, Refusal::NotAboard), "{events:?}");

    // The stack comes off, one to a cell, as far as the pack goes. How
    // far that is is the **footprint's** to say rather than the free
    // cells': a block of tofu is four by four and only a bandage stacks
    // in a pack, so a pack with cells to spare still takes only the
    // blocks that lie in it.
    let before = world_checksum(&world);
    let events = world.step(&[Command::Plunder {
        slot: 0,
        who: 0,
        id: ore.id,
    }]);
    let expect = events
        .iter()
        .find_map(|e| match e {
            WorldEvent::Plundered { who: 0, units } => Some(*units),
            _ => None,
        })
        .unwrap_or_else(|| panic!("{events:?}"));
    assert!(
        expect > 0 && expect <= ore.count,
        "{expect} of {}",
        ore.count
    );
    let in_pack = world
        .aboard
        .room
        .gear(0)
        .units_of(Item::Stack(ResourceId::Tofu as u32));
    assert_eq!(in_pack, expect);
    let after = world.plunder_alongside().unwrap();
    assert_eq!(
        after.grid.units_of(ResourceId::Tofu),
        laid.grid.units_of(ResourceId::Tofu) - expect,
        "the shelf gave up what the pack took"
    );
    assert_ne!(
        world_checksum(&world),
        before,
        "the shelf is in the checksum"
    );
    assert_eq!(WorldEvent::Plundered { who: 0, units: 3 }.code(), 64);

    // Once the pack is full, nothing more.
    let mut left = laid.grid.slots.clone();
    left.retain(|s| world.plunder_alongside().unwrap().grid.slot(s.id).is_some());
    let mut full = false;
    for _ in 0..200 {
        let Some(slot) = left.first() else { break };
        let events = world.step(&[Command::Plunder {
            slot: 0,
            who: 0,
            id: slot.id,
        }]);
        if refused(&events, Refusal::PackFull) {
            full = true;
            break;
        }
        if world
            .plunder_alongside()
            .unwrap()
            .grid
            .slot(slot.id)
            .is_none()
        {
            left.remove(0);
        }
    }
    assert!(full, "the pack fills before the shelf empties");

    // A shelf plundered stays plundered: away and back, it is as it was
    // left.
    let left_as = world.plunder_alongside().unwrap().grid.clone();
    world.undock_for_probe();
    assert!(world.plunder_alongside().is_none(), "away from the berth");
    assert_eq!(world.plunder.len(), 1, "but remembered");
    world.dock_for_probe(station);
    assert_eq!(world.plunder_alongside().unwrap().grid, left_as);
    assert_eq!(world.plunder.len(), 1, "laid out once");
}

/// Two worlds on one seed lay the same enemy's shelf out alike, since it
/// is rolled off the station's own seed and nothing else.
#[test]
fn two_worlds_on_one_seed_find_the_same_shelf() {
    let mut a = basic();
    let mut b = basic();
    let station = a.ship.state.station().unwrap();
    a.set_hostile(station, true);
    b.set_hostile(station, true);
    assert_eq!(a.plunder, b.plunder);
    assert_eq!(world_checksum(&a), world_checksum(&b));
    let laid = crate::plunder::lay_out(a.station(station).unwrap());
    assert_eq!(laid, a.plunder[0]);
    assert_eq!(
        laid.capacity,
        a.station(station)
            .unwrap()
            .design
            .capacity(economy::Storage::Locker)
    );
}
