//! The crew's orders as commands (feature 59): a click, a walk, a menu row
//! and a Management box all cross the seam as `Command::Crew`, every
//! player has a selection of their own, and two worlds fed one stream of
//! commands stay one world.

use bims::game::Container;
use bims::order::CrewOrder;
use shipdesign::fixture::playtest_ship;
use shipdesign::parts::TILE;

use crate::checksum::world_checksum;
use crate::data;
use crate::event::WorldEvent;
use crate::fixture::simulation_world;
use crate::world::{Command, World};

fn two_players() -> World {
    simulation_world(playtest_ship(), data::SIMULATION_MONEY, 2)
}

/// Where the crew stand, in the ship's frame, as the checksum reads them.
fn positions(world: &World) -> Vec<(f64, f64)> {
    (0..world.aboard.count())
        .map(|who| {
            let at = world.aboard.position(who);
            (at.x, at.y)
        })
        .collect()
}

#[test]
fn a_walk_is_a_command_and_every_player_selects_for_themselves() {
    let mut world = two_players();
    let tile = TILE as f32;

    // Player 1 selects their own; player 0's selection is untouched.
    world.step(&[Command::Crew {
        slot: 1,
        order: CrewOrder::SelectOwn,
    }]);
    let room = &world.aboard.room;
    assert!(room.is_selected(1, 1), "player 1 has their own selected");
    assert!(!room.is_selected(1, 0), "and player 0 has not");
    assert!(
        !room.is_selected(0, 1),
        "player 1 has not picked player 0's"
    );

    // A click on player 0's own by player 1 selects it for them — looking
    // at is allowed — but an order to it is ignored: it is not theirs.
    let james = world.aboard.room.bim_pos(0);
    world.step(&[Command::Crew {
        slot: 1,
        order: CrewOrder::Select {
            x0: james.x,
            y0: james.y,
            x1: james.x,
            y1: james.y,
        },
    }]);
    assert!(world.aboard.room.is_selected(0, 1));
    let events = world.step(&[Command::Crew {
        slot: 1,
        order: CrewOrder::Move {
            x: james.x + 3.0 * tile,
            y: james.y,
        },
    }]);
    assert!(
        !events
            .iter()
            .any(|e| matches!(e, WorldEvent::Refused { .. })),
        "an ignored order is not a refusal"
    );
    assert!(
        world.aboard.room.post_of(0).is_none(),
        "James takes no order from player 1"
    );

    // Player 0 selects their own and walks it: the post is set and the
    // body moves over the steps that follow.
    let from = world.aboard.room.bim_pos(0);
    world.step(&[
        Command::Crew {
            slot: 0,
            order: CrewOrder::SelectOwn,
        },
        Command::Crew {
            slot: 0,
            order: CrewOrder::Move {
                x: from.x + 3.0 * tile,
                y: from.y,
            },
        },
    ]);
    // A player's own gets no post from a walk — a post is a crewmate's,
    // under the alarm — so the walk shows in where it gets to.
    for _ in 0..600 {
        world.step(&[]);
    }
    let now = world.aboard.room.bim_pos(0);
    assert!(
        (now - from).len() > tile,
        "James walked: {from:?} -> {now:?}"
    );
}

#[test]
fn the_helm_the_desk_and_a_menu_row_are_commands_too() {
    let mut world = two_players();
    // To the helm, and stood down again.
    world.step(&[Command::ToHelm { slot: 1 }]);
    assert!(world.aboard.room.post_of(1).is_some(), "posted at the helm");
    world.step(&[Command::Crew {
        slot: 1,
        order: CrewOrder::StandDown { who: 1 },
    }]);
    assert!(world.aboard.room.post_of(1).is_none());
    // To the desk: docked, there is one.
    world.step(&[Command::ToDesk { slot: 0 }]);
    assert!(world.aboard.room.post_of(0).is_some(), "posted at the desk");
    // A container's use spot, the way a window's click walks a Bim.
    let spot = world
        .aboard
        .room
        .container_spot(Container::Shelf(0))
        .expect("the playtest ship has a shelf");
    world.step(&[Command::Crew {
        slot: 0,
        order: CrewOrder::SendTo {
            who: 0,
            x: spot.x,
            y: spot.y,
        },
    }]);
    let post = world.aboard.room.post_of(0).expect("posted at the shelf");
    assert!((post - spot).len() < TILE as f32);
    // A Management box: autonomy off is read straight back.
    world.step(&[Command::Crew {
        slot: 1,
        order: CrewOrder::Autonomous { on: false },
    }]);
    assert!(!world.aboard.room.is_autonomous());
    world.step(&[Command::Crew {
        slot: 1,
        order: CrewOrder::Autonomous { on: true },
    }]);
    assert!(world.aboard.room.is_autonomous());
}

#[test]
fn two_worlds_fed_one_stream_of_orders_stay_one_world() {
    let mut a = two_players();
    let mut b = two_players();
    let tile = TILE as f32;
    let from = a.aboard.room.bim_pos(1);
    let stream: Vec<(u32, Vec<Command>)> = vec![
        (
            5,
            vec![Command::Crew {
                slot: 1,
                order: CrewOrder::SelectOwn,
            }],
        ),
        (
            20,
            vec![Command::Crew {
                slot: 1,
                order: CrewOrder::Move {
                    x: from.x + 2.0 * tile,
                    y: from.y + tile,
                },
            }],
        ),
        (
            400,
            vec![
                Command::Crew {
                    slot: 0,
                    order: CrewOrder::Recruit,
                },
                Command::ToHelm { slot: 0 },
            ],
        ),
        (
            700,
            vec![Command::Crew {
                slot: 0,
                order: CrewOrder::Recruit,
            }],
        ),
    ];
    let mut step = 0;
    for (at, commands) in stream {
        while step < at {
            a.step(&[]);
            b.step(&[]);
            step += 1;
        }
        a.step(&commands);
        b.step(&commands);
        step += 1;
    }
    for _ in 0..300 {
        a.step(&[]);
        b.step(&[]);
    }
    assert_eq!(positions(&a), positions(&b));
    assert_eq!(world_checksum(&a), world_checksum(&b));
    assert!(
        (a.aboard.room.bim_pos(1) - from).len() > tile,
        "the walk was taken on both"
    );
}

/// Feature 69: an order given with Shift crosses the seam as
/// `Command::CrewLater` and waits its turn — two walks one after the
/// other, read off the room until walked — and a plain order afterwards
/// calls what is queued off.
#[test]
fn a_shift_order_is_a_command_that_waits_its_turn() {
    let mut world = two_players();
    let tile = TILE as f32;
    let from = world.aboard.room.bim_pos(0);
    let (a, b) = (
        from + bims::math::vec2(3.0 * tile, 0.0),
        from + bims::math::vec2(3.0 * tile, 2.0 * tile),
    );
    let events = world.step(&[
        // Autonomy off, so where it walks is where it stays.
        Command::Crew {
            slot: 0,
            order: CrewOrder::Autonomous { on: false },
        },
        Command::Crew {
            slot: 0,
            order: CrewOrder::SelectOwn,
        },
        Command::CrewLater {
            slot: 0,
            order: CrewOrder::Move { x: a.x, y: a.y },
        },
        Command::CrewLater {
            slot: 0,
            order: CrewOrder::Move { x: b.x, y: b.y },
        },
    ]);
    assert!(
        !events
            .iter()
            .any(|e| matches!(e, WorldEvent::Refused { .. })),
        "both queued: {events:?}"
    );
    // The first is on at once (there was nothing to wait behind), the
    // second waits, and the room says so.
    let walks = world.aboard.room.queued_walks(0);
    assert_eq!(walks.len(), 1, "the second waits: {walks:?}");
    // Snapped to somewhere a body can stand, as a right-click is.
    let goal = walks[0];
    assert!(
        (goal - b).len() < 3.0 * tile,
        "near enough: {goal:?} for {b:?}"
    );
    // Walked to the first, the second is taken up, and walked there
    // too: the walk ends within reach of the spot.
    let mut steps = 0;
    while !world.aboard.room.queued_walks(0).is_empty() && steps < 900 {
        world.step(&[]);
        steps += 1;
    }
    assert!(
        world.aboard.room.queued_walks(0).is_empty(),
        "the second was taken up"
    );
    while !world.aboard.room.arrived_for_probe(0) && steps < 1500 {
        world.step(&[]);
        steps += 1;
    }
    let now = world.aboard.room.bim_pos(0);
    assert!(
        (now - goal).len() < 1.5 * tile,
        "walked both: {from:?} -> {now:?}, for {goal:?}"
    );
    assert!(world.aboard.room.queued_walks(0).is_empty());
    // Queued behind a plain walk, then called off by another.
    world.step(&[
        Command::Crew {
            slot: 0,
            order: CrewOrder::Move {
                x: from.x,
                y: from.y,
            },
        },
        Command::CrewLater {
            slot: 0,
            order: CrewOrder::Move { x: a.x, y: a.y },
        },
    ]);
    assert_eq!(world.aboard.room.ordered_count(0), 1);
    world.step(&[Command::Crew {
        slot: 0,
        order: CrewOrder::Move {
            x: from.x,
            y: from.y,
        },
    }]);
    assert_eq!(
        world.aboard.room.ordered_count(0),
        0,
        "a plain order drops the queue"
    );
}
