//! The ready check: a mission with a fight in it opens held, nothing
//! moving, until every connected player has pressed *Ready*.

use shipdesign::fixture::flyer;

use crate::event::{Refusal, WorldEvent};
use crate::fixture::{REFERENCE_MONEY, open_simulation_world, simulation_world};
use crate::run::Phase;
use crate::world::{Command, World};
use crate::world_checksum;

/// The game as it plays at the spawn, two players, the check switched on
/// the way the `game` run does before its first step.
fn held() -> World {
    let mut world = open_simulation_world(flyer(2), REFERENCE_MONEY, 2);
    let id = world.ship.state.alongside().expect("docked");
    assert!(!world.site_cleared(id), "the spawn has a fight in it");
    world.set_ready_check(true);
    assert!(world.awaiting_ready(), "held for the ready check");
    world
}

/// Where every body of the crew's room stands.
fn bodies(world: &World) -> Vec<bims::math::Vec2> {
    let room = &world.aboard.room;
    (0..room.body_count() as usize)
        .map(|i| room.body_pos(i))
        .collect()
}

fn refused_with(events: &[WorldEvent], why: Refusal) -> bool {
    events
        .iter()
        .any(|e| matches!(e, WorldEvent::Refused { why: w, .. } if *w == why))
}

#[test]
fn a_held_mission_moves_nothing_until_every_player_is_ready() {
    let mut world = held();
    let before = bodies(&world);
    for _ in 0..200 {
        world.step(&[]);
    }
    assert_eq!(world.mission_steps(), 0, "the mission clock waits");
    assert_eq!(bodies(&world), before, "nobody moved");
    // Nothing that would start anything is heard.
    let events = world.step(&[Command::Return { slot: 0 }]);
    assert!(refused_with(&events, Refusal::AwaitingReady));
    assert!(!world.run.is_returning(0));

    // One yes is not enough, and a yes can be taken back.
    let events = world.apply_now(Command::Ready { slot: 0, yes: true });
    assert!(events.contains(&WorldEvent::Readied { slot: 0, yes: true }));
    assert_eq!(world.ready_count(), (1, 2));
    world.apply_now(Command::Ready {
        slot: 0,
        yes: false,
    });
    assert_eq!(world.ready_count(), (0, 2));
    world.step(&[]);
    assert!(world.awaiting_ready());
    assert_eq!(world.mission_steps(), 0);

    // Every yes, and the mission is under way.
    world.apply_now(Command::Ready { slot: 0, yes: true });
    let events = world.apply_now(Command::Ready { slot: 1, yes: true });
    assert!(events.contains(&WorldEvent::AllReady));
    assert!(!world.awaiting_ready());
    world.step(&[]);
    assert_eq!(world.mission_steps(), 1, "the clock runs");
    // And *Ready* now is a press with nothing to answer.
    let events = world.apply_now(Command::Ready { slot: 0, yes: true });
    assert!(refused_with(&events, Refusal::NoReadyCheck));
}

#[test]
fn a_player_gone_is_not_waited_for() {
    let mut world = held();
    world.apply_now(Command::Ready { slot: 0, yes: true });
    assert!(world.awaiting_ready());
    let events = world.apply_now(Command::PlayerGone { slot: 1 });
    assert!(events.contains(&WorldEvent::AllReady));
    assert!(!world.awaiting_ready());
}

/// A player whose line dropped and came back (`Command::PlayerBack`) is
/// waited for again.
#[test]
fn a_player_back_is_waited_for_again() {
    let mut world = held();
    world.apply_now(Command::PlayerGone { slot: 1 });
    assert!(!world.run.is_connected(1));
    world.apply_now(Command::PlayerBack { slot: 1 });
    assert!(world.run.is_connected(1));
    let events = world.apply_now(Command::Ready { slot: 0, yes: true });
    assert!(!events.contains(&WorldEvent::AllReady));
    assert!(world.awaiting_ready(), "the one back is waited for");
    let events = world.apply_now(Command::Ready { slot: 1, yes: true });
    assert!(events.contains(&WorldEvent::AllReady));
}

#[test]
fn a_peaceful_stop_and_a_world_without_the_switch_start_at_once() {
    // The tests' quiet dial: nothing to fight, nothing to wait for.
    let mut quiet = simulation_world(flyer(2), REFERENCE_MONEY, 2);
    quiet.set_ready_check(true);
    assert!(!quiet.awaiting_ready());
    // The switch off, as in every test and staged command: the game as
    // it was, to the checksum.
    let mut off = open_simulation_world(flyer(2), REFERENCE_MONEY, 2);
    let mut again = open_simulation_world(flyer(2), REFERENCE_MONEY, 2);
    again.set_ready_check(false);
    assert!(!off.awaiting_ready());
    off.step(&[]);
    again.step(&[]);
    assert_eq!(off.mission_steps(), 1);
    assert_eq!(world_checksum(&off), world_checksum(&again));
}

#[test]
fn every_mission_with_a_fight_is_held_again_after_travel() {
    let mut world = held();
    world.apply_now(Command::Ready { slot: 0, yes: true });
    world.apply_now(Command::Ready { slot: 1, yes: true });
    world.leave_for_probe();
    assert_eq!(world.run.phase, Phase::Map);
    assert!(!world.awaiting_ready());
    let here = world.current_site();
    let sites: Vec<_> = world
        .sites_at(world.star_id)
        .into_iter()
        .filter(|&s| Some(s) != here && world.travel_quote(s).is_ok())
        .collect();
    let mut seen_held = false;
    for site in sites {
        let quote = world.travel_quote(site).expect("quoted");
        if quote.trader {
            continue;
        }
        world.step(&[Command::Propose {
            slot: 0,
            star: site.star,
            station: site.station,
        }]);
        world.step(&[Command::Accept { slot: 1, yes: true }]);
        assert_eq!(world.run.phase, Phase::Mission, "arrived");
        let id = world.ship.state.alongside().expect("tied up");
        assert_eq!(
            world.awaiting_ready(),
            !world.site_cleared(id),
            "held exactly where there is a fight"
        );
        assert_eq!(world.ready_count(), (0, 2), "nobody ready on arrival");
        seen_held |= world.awaiting_ready();
        break;
    }
    assert!(seen_held, "the next site had a fight to hold for");
}
