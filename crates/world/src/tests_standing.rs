//! The two standing orders every player has (feature 84,
//! `crate::orders`): **attack** and **retreat**, the following they
//! start on and go back to, what each does to the bots that follow the
//! player, and the ship being the last stand nobody leaves.

use bims::math::vec2;
use shipdesign::fixture::combat_ship;

use crate::event::{Refusal, WorldEvent};
use crate::fixture::{REFERENCE_MONEY, crewed_world};
use crate::orders::Standing;
use crate::world::{Command, World};
use crate::world_checksum;

const TILE: f32 = shipdesign::TILE as f32;

/// The combat ship — bunks and chairs for five — with one player and
/// four crew nobody steers, docked at the spawn. The four are the bots
/// the orders are about.
fn crewed() -> World {
    crewed_world(combat_ship(), REFERENCE_MONEY, 1, 5)
}

/// A handful of tiles of the crew's deck a banner may go on, in the
/// room's own tile frame — found by asking the room rather than written
/// down, since the deck is the ship's design's.
fn banner_tiles(world: &World, want: usize) -> Vec<(i32, i32)> {
    let room = &world.aboard.room;
    let at = room.bim_pos(0);
    let (cx, cy) = ((at.x / TILE) as i32, (at.y / TILE) as i32);
    let mut out = Vec::new();
    for r in 0..12 {
        for dy in -r..=r {
            for dx in -r..=r {
                let tile = (cx + dx, cy + dy);
                let middle = vec2((tile.0 as f32 + 0.5) * TILE, (tile.1 as f32 + 0.5) * TILE);
                if room.is_banner_tile(middle) && !out.contains(&tile) {
                    out.push(tile);
                    if out.len() == want {
                        return out;
                    }
                }
            }
        }
    }
    out
}

fn refused_with(events: &[WorldEvent], want: Refusal) -> bool {
    events
        .iter()
        .any(|e| matches!(e, WorldEvent::Refused { why, .. } if *why == want))
}

/// The order is given, said, and the same order again lets the crew go
/// — the commander's squad rule, and for the same reason: one key does
/// both.
#[test]
fn an_order_is_given_said_and_released_by_the_same_key_again() {
    let mut world = crewed();
    world.step(&[]);
    assert_eq!(world.standing_of(0), Standing::Follow);

    let events = world.step(&[Command::Orders {
        slot: 0,
        order: Standing::Retreat,
    }]);
    assert!(
        events
            .iter()
            .any(|e| matches!(e, WorldEvent::Ordered { who: 0, kind: 2 })),
        "the retreat is said: {events:?}"
    );
    assert_eq!(world.standing_of(0), Standing::Retreat);

    // Again, and they are following.
    let events = world.step(&[Command::Orders {
        slot: 0,
        order: Standing::Retreat,
    }]);
    assert!(
        events
            .iter()
            .any(|e| matches!(e, WorldEvent::Ordered { who: 0, kind: 0 })),
        "the release is said: {events:?}"
    );
    assert_eq!(world.standing_of(0), Standing::Follow);

    // An attack wants ground: a tile far outside the deck is refused,
    // and one under the crew member's own feet is taken.
    let events = world.step(&[Command::Orders {
        slot: 0,
        order: Standing::Attack {
            tile: (-9_000, -9_000),
        },
    }]);
    assert!(
        refused_with(&events, Refusal::NoGroundThere),
        "no ground there: {events:?}"
    );
    assert_eq!(world.standing_of(0), Standing::Follow);

    let tiles = banner_tiles(&world, 2);
    assert_eq!(tiles.len(), 2, "the deck has two tiles to stand on");
    let (tile, other) = (tiles[0], tiles[1]);
    world.step(&[Command::Orders {
        slot: 0,
        order: Standing::Attack { tile },
    }]);
    assert_eq!(world.standing_of(0), Standing::Attack { tile });
    // And a banner on another tile replaces it rather than releasing.
    world.step(&[Command::Orders {
        slot: 0,
        order: Standing::Attack { tile: other },
    }]);
    assert_eq!(world.standing_of(0), Standing::Attack { tile: other });
}

/// It is not a class's, and it is not free: the player has to be fit to
/// act, the same gate a trip and a squad order stand behind.
#[test]
fn an_order_wants_the_player_on_its_feet_and_no_class_at_all() {
    let mut world = crewed();
    world.step(&[]);
    // No class chosen, and the order goes.
    assert_eq!(world.class_of(0), crate::class::Class::None);
    world.step(&[Command::Orders {
        slot: 0,
        order: Standing::Retreat,
    }]);
    assert_eq!(world.standing_of(0), Standing::Retreat);

    // Knocked out, the order it had is dropped and a fresh one refused.
    // The blood is set low and the room takes a step to notice it.
    world.aboard.room.knock_out_for_probe(0);
    world.step(&[]);
    assert!(
        world.aboard.room.is_unconscious(0),
        "the player's own Bim is out cold"
    );
    let events = world.step(&[Command::Orders {
        slot: 0,
        order: Standing::Retreat,
    }]);
    assert!(
        refused_with(&events, Refusal::OutOfReach),
        "out of reach while down: {events:?}"
    );
    assert_eq!(
        world.standing_of(0),
        Standing::Follow,
        "and the one it had is let go"
    );
}

/// The order is the world's: it is in `world_checksum`, so two clients
/// that disagree about it disagree about where the crew are standing.
#[test]
fn the_checksum_notices_a_standing_order() {
    let mut world = crewed();
    world.step(&[]);
    let before = world_checksum(&world);
    let tile = banner_tiles(&world, 1)[0];
    world.step(&[Command::Orders {
        slot: 0,
        order: Standing::Attack { tile },
    }]);
    assert_ne!(before, world_checksum(&world), "the banner is hashed");

    // And two worlds fed the same stream stay one world.
    let mut twin = crewed();
    twin.step(&[]);
    twin.step(&[Command::Orders {
        slot: 0,
        order: Standing::Attack { tile },
    }]);
    assert_eq!(world_checksum(&world), world_checksum(&twin));
}

/// The bots take the order, and the room reads it by the player they
/// are nearest. Nobody a player steers is ever moved by one.
#[test]
fn the_bots_take_the_order_and_the_players_own_bim_never_does() {
    let mut world = crewed();
    world.step(&[]);
    let tile = banner_tiles(&world, 1)[0];
    world.step(&[Command::Orders {
        slot: 0,
        order: Standing::Attack { tile },
    }]);
    let room = &world.aboard.room;
    let at = vec2((tile.0 as f32 + 0.5) * TILE, (tile.1 as f32 + 0.5) * TILE);
    for who in 1..room.crew_count() as usize {
        assert_eq!(
            room.standing_for_probe(who),
            bims::game::Standing::Attack { at },
            "crew {who} is under the banner"
        );
    }
    // And the crew are under arms of their own accord for it: an order
    // is a player leading them, which musters them off their errands.
    assert!(world.aboard.room.is_mustered(), "under arms for the order");
}

/// A retreat walks the bots back towards the ship's own gangway, and
/// they are still the player's to order one by one while it stands.
#[test]
fn a_retreat_walks_the_bots_back_to_the_ship() {
    let mut world = crewed();
    world.step(&[]);
    // Stand one of the bots well out along the station's deck, past the
    // ship's box, and call the crew back.
    let ashore = world.aboard.ashore.expect("docked, so there is an ashore");
    let there = bims::math::vec2(ashore.x as f32, ashore.y as f32);
    world.aboard.room.put_for_probe(1, there);
    let away = (world.aboard.room.bim_pos(1) - there).len();
    assert!(away < TILE, "stood where it was put");
    world.step(&[Command::Orders {
        slot: 0,
        order: Standing::Retreat,
    }]);
    // A minute of the clock is plenty to be walking.
    let start = world.aboard.room.bim_pos(1);
    for _ in 0..(60 * 60) {
        world.step(&[]);
        if !world.aboard.room.is_aboard(world.aboard.room.bim_pos(1)) {
            continue;
        }
        break;
    }
    let now = world.aboard.room.bim_pos(1);
    assert!(
        world.aboard.room.is_aboard(now),
        "it went home: {start:?} to {now:?}"
    );
}

/// **And home is the ship's own airlock, not the station's far one.**
/// The room cannot work out where the ship is from a joined deck —
/// `Room::gangway` there is the joined design's first *free* airlock,
/// and the ship's is mated to the station, so the room's own answer is
/// the door at the other end of the building. The world says it instead
/// (`Game::set_home`), and without that a retreat walked the crew out
/// through the station rather than back aboard, which is what the key
/// looked like doing nothing at all.
#[test]
fn the_fall_back_point_is_the_ships_own_gangway_and_not_the_stations() {
    let mut world = crewed();
    world.step(&[]);
    let (x, y) = world.fall_back_point();
    let at = vec2(x, y);
    assert!(
        world.aboard.room.is_aboard(at),
        "the spot they gather on is aboard the ship: {at:?}"
    );
    // It is the ship's gangway to the tile, and the station's own door
    // — where the crew step ashore — is somewhere else entirely.
    let gangway = world.aboard.gangway.expect("docked, so the ship has one");
    assert!(
        (at - vec2(gangway.x as f32, gangway.y as f32)).len() < TILE,
        "and it is the ship's gangway: {at:?} against {gangway:?}"
    );
    let ashore = world.aboard.ashore.expect("docked, so there is an ashore");
    assert!(
        (at - vec2(ashore.x as f32, ashore.y as f32)).len() > TILE,
        "and not the spot ashore"
    );

    // Undocked, the ship is the whole room and the room's own gangway
    // is the right answer, so nothing is said and nothing changes.
    world.undock_for_probe();
    world.step(&[]);
    let (x, y) = world.fall_back_point();
    assert!(
        world.aboard.room.is_aboard(vec2(x, y)),
        "a ship flying alone is aboard everywhere"
    );
}

/// The crew fall back **fighting**: a bot with an enemy in front of it
/// gives ground backwards with its gun up, and one with nothing to
/// shoot at turns round and runs for it. Both walk the same route home;
/// what differs is which way the body faces and how fast it goes.
#[test]
fn a_bot_falling_back_gives_ground_backwards_and_sprints_with_nothing_in_sight() {
    let mut world = crewed();
    world.step(&[]);
    assert!(world.stage_fight_for_probe(), "a fight is staged");
    // The player and a bot out along the station's deck, under arms,
    // with the station's one resident down the corridor from them.
    let ashore = world.aboard.ashore.expect("docked, so there is an ashore");
    let there = bims::math::vec2(ashore.x as f32, ashore.y as f32);
    for who in [0usize, 1] {
        world
            .aboard
            .room
            .put_for_probe(who, there + vec2(who as f32 * TILE, 0.0));
        world.aboard.room.recruit_for_probe(who, true);
    }
    world.step(&[Command::Orders {
        slot: 0,
        order: Standing::Retreat,
    }]);
    // Somewhere in the first seconds of the walk home it is backing off
    // — walking, with the enemy in its sights.
    let mut backed = false;
    for _ in 0..(20 * 60) {
        world.step(&[]);
        if world.aboard.room.is_backing_for_probe(1) {
            backed = true;
            break;
        }
    }
    assert!(backed, "it gives ground with its gun on the enemy");

    // With nobody left to shoot at it runs instead: the same order, the
    // same walk, and never backing.
    let mut quiet = crewed();
    quiet.step(&[]);
    let ashore = quiet.aboard.ashore.expect("docked, so there is an ashore");
    let there = bims::math::vec2(ashore.x as f32, ashore.y as f32);
    quiet.aboard.room.put_for_probe(1, there);
    quiet.step(&[Command::Orders {
        slot: 0,
        order: Standing::Retreat,
    }]);
    let mut walked = false;
    for _ in 0..(60 * 60) {
        quiet.step(&[]);
        assert!(
            !quiet.aboard.room.is_backing_for_probe(1),
            "nothing to shoot at, so nothing to back away from"
        );
        if quiet.aboard.room.is_aboard(quiet.aboard.room.bim_pos(1)) {
            walked = true;
            break;
        }
    }
    assert!(walked, "and it got home");
}
