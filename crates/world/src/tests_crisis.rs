//! The crisis (feature 92): the lanes, the origin, the spread and the flip.
//!
//! The lane graph itself is `worldgen::galaxy`'s — one connected web, the
//! same for a seed and different for another, and in the galaxy checksum.
//! What is here is what the *world* makes of it: where the machines began,
//! which day each star turns, and what happens to a system the day comes
//! for while the crew are standing in it.

use shipdesign::fixture::flyer;
use shipdesign::parts::PartKind;
use shipdesign::{Budget, Edit, Rotation, ShipDesign, apply};
use worldgen::{Galaxy, GalaxyType};

use crate::data;
use crate::droid;
use crate::event::WorldEvent;
use crate::fixture::{REFERENCE_MONEY, simulation_world};
use crate::world::{Command, ShipState, World};
use crate::world_checksum;

fn basic() -> World {
    simulation_world(flyer(2), REFERENCE_MONEY, 2)
}

/// The flyer with a hyperdrive, as `tests_memory::jumper` builds it.
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

/// The ship off its berth and holding in open space, well out of every
/// station's range, and stepped once: the rooms are not joined, so nothing
/// holds the flip off, and no station's room is open either.
fn out_in_the_open(world: &mut World) {
    world.undock_for_probe();
    let far = world
        .stations
        .iter()
        .map(|s| s.centre().x + s.radius())
        .fold(0.0, f64::max)
        + data::RESIDENTS_RANGE * 10.0;
    world.put_for_probe(worldgen::math::dvec2(far, 0.0));
    world.step(&[]);
    assert!(world.residents.is_none(), "no station's room is open");
}

/// The ship, holding, charged and jumped to `star` — `tests_memory`'s own.
fn jump_to(world: &mut World, star: u32) {
    assert_eq!(world.ship.state, ShipState::Holding);
    world.man_the_helm_for_probe(0);
    world.step(&[Command::Jump { slot: 0, star }]);
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

/// The origin is rolled far from the crew, it is the same star twice, and
/// the day a star turns is the line of arithmetic the spread rule is.
#[test]
fn the_origin_is_far_off_and_the_day_a_star_turns_is_its_hops() {
    let world = basic();
    let galaxy = world.galaxy();
    let hops = galaxy.hops_from(world.star_id);
    let origin = world.droid_origin();
    let out = hops[origin as usize];
    assert_ne!(out, u16::MAX, "the origin is reachable by lane");
    // Far off, or as far as this galaxy allows.
    let furthest = hops
        .iter()
        .copied()
        .filter(|&h| h != u16::MAX)
        .max()
        .unwrap();
    assert!(
        out >= data::DROID_ORIGIN_MIN_HOPS || out == furthest,
        "the origin is {out} hops off and the furthest star is {furthest}"
    );
    // The same galaxy started from the same dock puts them in the same place.
    assert_eq!(basic().droid_origin(), origin);

    // The origin is the machines' from day nought (feature 102), and
    // nothing else is.
    let mut world = world;
    assert_eq!(world.crisis_first_day(), 0);
    assert_eq!(world.days_gone(), 0);
    assert!(world.infested(origin));
    assert_eq!(world.infested_on(origin), 0);
    let origin_hops = galaxy.hops_from(origin);
    for star in 0..galaxy.stars.len() as u32 {
        assert_eq!(world.infested(star), origin_hops[star as usize] == 0);
    }
    // And a star n hops out turns on 5n, and not the day before.
    for star in 0..galaxy.stars.len() as u32 {
        let n = origin_hops[star as usize];
        let day = data::DROID_SPREAD_DAYS * u32::from(n);
        assert_eq!(world.infested_on(star), day, "star {star}, {n} hops out");
        if day > 0 {
            world.set_day_for_probe(day - 1);
            assert!(!world.infested(star), "star {star} the day before");
        }
        world.set_day_for_probe(day);
        assert!(world.infested(star), "star {star} on its day");
        // Cheap enough for a handful; the arithmetic is the same for all.
        if star > 40 {
            break;
        }
    }
    // A star the galaxy has not got never turns.
    assert_eq!(world.infested_on(galaxy.stars.len() as u32), u32::MAX);
    assert!(!world.infested(galaxy.stars.len() as u32));

    // The probes' dial moves the whole of it: a first day of ten, and
    // nothing is theirs the day before, whatever the graph says.
    world.set_crisis_first_day_for_probe(10);
    world.set_day_for_probe(9);
    assert!(
        (0..galaxy.stars.len() as u32).all(|s| !world.infested(s)),
        "nothing turns before the day the dial names"
    );
    world.set_day_for_probe(10);
    assert!(world.infested(origin));
    assert_eq!(world.infested_on(origin), 10);
}

/// Two builds of one galaxy count the same hops, and the day the whole of
/// it is infested is what the furthest star's hop count says.
#[test]
fn two_builds_of_a_galaxy_agree_about_every_hop() {
    for &t in &[GalaxyType::SpiralTwoArm, GalaxyType::Round] {
        let a = Galaxy::new(data::DEFAULT_SEED, t);
        let b = Galaxy::new(data::DEFAULT_SEED, t);
        for star in [0, 1, 250, 999] {
            assert_eq!(a.hops_from(star), b.hops_from(star), "{t:?} from {star}");
        }
    }
    // And `turns_on` is the whole rule, unreachable stars included.
    assert_eq!(droid::turns_on(10, 0), 10);
    assert_eq!(droid::turns_on(10, 3), 10 + 3 * data::DROID_SPREAD_DAYS);
    assert_eq!(droid::turns_on(10, u16::MAX), u32::MAX);
}

/// The flip waits for the crew to leave, and when it comes every station
/// of the system is the machines' — no people, no hire, no shelf.
#[test]
fn a_system_does_not_flip_under_the_crew_and_flips_the_moment_they_are_off_it() {
    let mut world = basic();
    // The machines began here, and they began this morning.
    world.set_droid_origin_for_probe(world.star_id);
    world.set_crisis_first_day_for_probe(0);
    assert!(world.infested(world.star_id), "its day has come");

    // Docked, with the rooms joined, nothing moves.
    let home = world.home;
    let before = world.station(home).map(|s| world.people_of(s)).unwrap();
    assert!(before > 0, "somebody lives at the spawn");
    for _ in 0..4 {
        let events = world.step(&[]);
        assert!(
            !events
                .iter()
                .any(|e| matches!(e, WorldEvent::Infested { .. })),
            "never with a room joined"
        );
    }
    assert!(
        !world.is_droid_held(home),
        "still the system they arrived in"
    );
    assert_eq!(
        world.station(home).map(|s| world.people_of(s)),
        Some(before)
    );

    // Off the berth, and it turns on the very next step — the station's
    // own room still open alongside, since that is the crew's to watch
    // empty rather than a room of theirs to be standing in.
    world.undock_for_probe();
    let events = world.step(&[]);
    assert!(
        events
            .iter()
            .any(|e| matches!(e, WorldEvent::Infested { star } if *star == world.star_id)),
        "{events:?}"
    );
    for id in world
        .stations
        .iter()
        .map(|s| s.id)
        .chain(world.surfaces.iter().map(|s| s.id))
        .collect::<Vec<_>>()
    {
        assert!(world.is_droid_held(id), "station {id} is the machines'");
        let station = world.station(id).unwrap();
        assert_eq!(world.people_of(&station), 0, "station {id} has nobody");
        assert_eq!(world.mercenaries_of(&station), 0, "and nobody for hire");
        assert_eq!(world.stance(id), bims::sight::Stance::Hostile);
    }
    // Said once: the second step has nothing left to take.
    let events = world.step(&[]);
    assert!(
        !events
            .iter()
            .any(|e| matches!(e, WorldEvent::Infested { .. }))
    );
}

/// A station the crew cleared stays cleared: the crisis never re-arms it,
/// whatever day it is.
#[test]
fn a_station_cleared_before_its_day_stays_cleared_after_it() {
    let mut world = basic();
    world.set_droid_origin_for_probe(world.star_id);
    // A station taken and won back long before the crisis reaches here.
    let held = world.stations[0].id;
    let other = world.stations.iter().map(|s| s.id).nth(1);
    world.infest(held);
    let it = world
        .infested
        .iter_mut()
        .find(|it| it.station == held)
        .unwrap();
    it.settled = true;
    it.wave = 3;
    it.waves_left = 0;
    it.cleared = true;
    assert!(world.droid_station_cleared(held));

    // The day comes, and the crew are nowhere near.
    world.set_crisis_first_day_for_probe(0);
    out_in_the_open(&mut world);

    assert!(
        world.droid_station_cleared(held),
        "the crisis never re-arms a station the crew won back"
    );
    let it = world.infestation(held).unwrap();
    assert_eq!((it.wave, it.waves_left), (3, 0), "and never re-settles it");
    if let Some(other) = other {
        assert!(world.is_droid_held(other), "its neighbours did fall");
        assert!(!world.droid_station_cleared(other));
    }
}

/// A system the crisis took while the crew were away gives back no people,
/// no stances and no losses when they come back to it.
#[test]
fn a_system_overrun_while_the_crew_were_away_remembers_none_of_its_people() {
    let mut world = simulation_world(jumper(), REFERENCE_MONEY, 2);
    // The old game's clock, running with the step (feature 103).
    world.set_free_clock(true);
    let home_star = world.star_id;
    let home = world.home;
    // A neighbour's people are enemies, and the crew have killed two.
    let enemy = world.stations.iter().map(|s| s.id).nth(1).unwrap();
    world.set_hostile(enemy, true);
    crate::memory::amend_losses(&mut world.losses, enemy, |l| l.dead += 2);
    assert!(!world.hostile.is_empty());

    // Away to another star, and the crisis takes the one behind them.
    world.undock_for_probe();
    // A star a lane joins to this one: a jump goes one hop (feature 93),
    // and the lanes are symmetric, so the way back is a jump too.
    let elsewhere = crate::tests::laned_star(&world);
    jump_to(&mut world, elsewhere);
    world.set_droid_origin_for_probe(home_star);
    world.set_crisis_first_day_for_probe(0);
    assert!(world.infested(home_star));

    // Back, and the memory of that system is the chart and nothing else.
    jump_to(&mut world, home_star);
    assert!(
        world.hostile.is_empty(),
        "no stances come back: {:?}",
        world.hostile
    );
    assert!(world.losses.is_empty(), "and no losses");
    world.step(&[]);
    assert!(
        world.is_droid_held(home),
        "the flip is what stands there now"
    );
    assert_eq!(world.station(home).map(|s| world.people_of(s)), Some(0));
}

/// Two clients of the same galaxy at the same day agree about the whole of
/// the crisis, and the checksum says so.
#[test]
fn two_worlds_on_one_seed_fall_to_the_machines_alike() {
    let mut a = basic();
    let mut b = basic();
    for world in [&mut a, &mut b] {
        world.set_crisis_first_day_for_probe(0);
        world.set_day_for_probe(data::DROID_SPREAD_DAYS * 4);
        out_in_the_open(world);
    }
    assert_eq!(a.infested_stars(), b.infested_stars());
    assert!(!a.infested_stars().is_empty(), "somewhere has fallen");
    assert_eq!(world_checksum(&a), world_checksum(&b));

    // And the checksum notices the origin moving, which is the whole map
    // of who falls when.
    let before = world_checksum(&a);
    let moved = (a.droid_origin() + 1) % 1000;
    a.set_droid_origin_for_probe(moved);
    assert_ne!(before, world_checksum(&a));
    // As it notices the day the first star turns.
    let before = world_checksum(&b);
    b.set_crisis_first_day_for_probe(3);
    assert_ne!(before, world_checksum(&b));
}

/// What the crisis costs, over ten galaxy seeds: how many lanes a star
/// has, how far the origin is from everywhere, and from that the day the
/// last star in the galaxy turns. Printed rather than asserted — the
/// numbers are in the root `CLAUDE.md`.
#[test]
#[ignore]
fn crisis_spread_over_ten_seeds() {
    println!("seed  lanes(min/med/max)  hops(med/max)  whole galaxy by day");
    let (mut worst_day, mut worst_hops) = (0, 0u16);
    for n in 0..10u64 {
        let seed = data::DEFAULT_SEED.wrapping_add(n.wrapping_mul(0x9e37_79b9));
        let galaxy = Galaxy::new(seed, GalaxyType::SpiralTwoArm);
        let mut lanes: Vec<usize> = galaxy.lanes.iter().map(Vec::len).collect();
        lanes.sort_unstable();
        // The origin as the world rolls it, from the same dock the
        // simulation starts at.
        let (star, _) = crate::spawn_anywhere(&galaxy, n).expect("a dock somewhere");
        let origin = droid::origin(&galaxy, star);
        let mut hops: Vec<u16> = galaxy
            .hops_from(origin)
            .into_iter()
            .filter(|&h| h != u16::MAX)
            .collect();
        hops.sort_unstable();
        let furthest = *hops.last().unwrap();
        // From day nought, as every run since feature 102.
        let day = droid::turns_on(0, furthest);
        worst_day = worst_day.max(day);
        worst_hops = worst_hops.max(furthest);
        println!(
            "{n:>4}  {:>3}/{:>3}/{:>3}        {:>3}/{:>3}        {day}",
            lanes[0],
            lanes[lanes.len() / 2],
            lanes[lanes.len() - 1],
            hops[hops.len() / 2],
            furthest,
        );
    }
    println!("worst: {worst_hops} hops, the whole galaxy by day {worst_day}");
}

/// An infested station is no shop and no larder: nobody keeps the desk,
/// and there is no shelf to loot — the machines carry nothing and the
/// people who stocked it are gone.
#[test]
fn an_infested_station_has_no_desk_and_nothing_on_a_shelf() {
    use crate::event::Refusal;
    use physics::ResourceId;

    let mut world = basic();
    world.set_droid_origin_for_probe(world.star_id);
    world.set_crisis_first_day_for_probe(0);
    out_in_the_open(&mut world);
    let home = world.home;
    assert!(world.is_droid_held(home), "the crisis took the spawn");

    // Docked at it, with somebody at the desk: metal is on every shelf,
    // and both halves of a trade are refused all the same.
    world.dock_for_probe(home);
    world.step(&[]);
    assert!(world.man_the_desk_for_probe(0), "a desk to stand at");
    let events = world.step(&[Command::Buy {
        slot: 0,
        resource: ResourceId::Vegetable,
        units: 1,
        tier: 1,
    }]);
    assert!(
        events
            .iter()
            .any(|e| matches!(e, WorldEvent::Refused { why, .. } if *why == Refusal::NoMarket)),
        "{events:?}"
    );
    let money = world.money;
    let events = world.step(&[Command::Sell {
        slot: 0,
        resource: ResourceId::Vegetable,
        units: 1,
    }]);
    assert!(
        events
            .iter()
            .any(|e| matches!(e, WorldEvent::Refused { why, .. } if *why == Refusal::NoMarket)),
        "{events:?}"
    );
    assert_eq!(world.money, money, "nothing was bought or sold");
    // And no shelf was laid out, hostile though it is.
    assert!(world.plunder_alongside().is_none());
    assert!(world.plunder.iter().all(|p| p.station != home));
    // Nobody to hire, and nobody aboard.
    let station = world.station(home).unwrap();
    assert_eq!(world.people_of(&station), 0);
    assert_eq!(world.mercenaries_of(&station), 0);
}
