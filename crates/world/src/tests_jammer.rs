//! Jumping along lanes, and the jammer (feature 93).
//!
//! Three things, and they lean on one another. A charge takes the ship
//! **one hop and only down a lane** ([`Refusal::NoLane`]); an infested
//! system's jammer holds the lanes **inward** shut while it stands
//! ([`Refusal::Jammed`]); and what tier the machines come at is how far
//! the system is from where they began. The lane graph and the route are
//! `worldgen::galaxy`'s and pinned there; what is here is what the
//! *world* makes of them.

use shipdesign::fixture::flyer;
use shipdesign::parts::PartKind;
use shipdesign::{Budget, Edit, Rotation, ShipDesign, apply};
use worldgen::{Galaxy, GalaxyType, Node};

use crate::data;
use crate::event::{Refusal, WorldEvent};
use crate::fixture::{REFERENCE_MONEY, simulation_world};
use crate::jammer;
use crate::world::{Command, ShipState, World};
use crate::world_checksum;

/// The flyer with a hyperdrive, as `tests_crisis::jumper` builds it.
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

fn jumper_world() -> World {
    let mut world = simulation_world(jumper(), REFERENCE_MONEY, 2);
    world.undock_for_probe();
    world.man_the_helm_for_probe(0);
    quiet_skies(&mut world);
    world
}

/// The next raid pushed a year off. These tests wind the clock on by
/// months (`set_day_for_probe`) to reach a day the crisis has come, and
/// the raid schedule is on that same clock: without this a raider ties up
/// to the ship mid-test and the jump under test is refused because the
/// ship is docked. Say it again after every winding.
fn quiet_skies(world: &mut World) {
    world.raid_due_for_probe(time::DAY as u64 * 365);
}

/// The clock wound to the start of `day`, with the skies kept quiet.
fn wind_to(world: &mut World, day: u32) {
    world.set_day_for_probe(day);
    quiet_skies(world);
}

fn refused_with(events: &[WorldEvent], why: Refusal) -> bool {
    events
        .iter()
        .any(|e| matches!(e, WorldEvent::Refused { why: w, .. } if *w == why))
}

fn charging(events: &[WorldEvent], star: u32) -> bool {
    events
        .iter()
        .any(|e| matches!(e, WorldEvent::Charging { star: s, .. } if *s == star))
}

/// An origin that gives the star the ship is at a lane **each way** —
/// inward (fewer hops from the origin), sideways (the same) and outward
/// (more) — and one neighbour of each. Searched rather than written down,
/// since which star the fixture spawns at is the generator's business.
fn three_ways(world: &World) -> (u32, u32, u32, u32) {
    let galaxy = world.galaxy();
    let here = world.star_id;
    for origin in 0..galaxy.stars.len() as u32 {
        let hops = galaxy.hops_from(origin);
        let h = hops[here as usize];
        if h == 0 || h == u16::MAX {
            continue;
        }
        let (mut inward, mut level, mut outward) = (None, None, None);
        for &next in galaxy.lanes(here) {
            let there = hops[next as usize];
            let slot = match there.cmp(&h) {
                std::cmp::Ordering::Less => &mut inward,
                std::cmp::Ordering::Equal => &mut level,
                std::cmp::Ordering::Greater => &mut outward,
            };
            slot.get_or_insert(next);
        }
        if let (Some(i), Some(l), Some(o)) = (inward, level, outward) {
            return (origin, i, l, o);
        }
    }
    panic!("no origin gives the crew's star a lane each way");
}

/// A star the lanes do **not** join to this one.
fn unlaned_star(world: &World) -> u32 {
    let galaxy = world.galaxy();
    let here = world.star_id;
    (0..galaxy.stars.len() as u32)
        .find(|&s| s != here && !galaxy.lanes(here).contains(&s))
        .expect("a galaxy of a thousand stars is not one clique")
}

// --- 1: a jump follows the lanes ------------------------------------------

/// A charge is **one hop, and only down a lane**. A star the lanes do not
/// reach from here is [`Refusal::NoLane`], whatever else is right about
/// the jump; one they do is accepted and charges as it always did.
#[test]
fn a_jump_wants_a_lane_out_of_the_star_the_ship_is_at() {
    let mut world = jumper_world();
    let here = world.star_id;
    let lane = world.galaxy().lanes(here)[0];
    let away = unlaned_star(&world);

    assert!(world.laned_to(lane));
    assert!(!world.laned_to(away));
    assert!(!world.laned_to(here), "a star is never laned to itself");

    let events = world.step(&[Command::Jump {
        slot: 0,
        star: away,
    }]);
    assert!(refused_with(&events, Refusal::NoLane), "{events:?}");
    assert_eq!(world.ship.state, ShipState::Holding);

    // The star the ship is at and a star the galaxy has not got are still
    // said before the lanes are looked at: those are better answers.
    let events = world.step(&[Command::Jump {
        slot: 0,
        star: here,
    }]);
    assert!(refused_with(&events, Refusal::SameStar), "{events:?}");
    let events = world.step(&[Command::Jump {
        slot: 0,
        star: 1_000_000,
    }]);
    assert!(refused_with(&events, Refusal::NoSuchStar), "{events:?}");

    // And down a lane it goes.
    let events = world.step(&[Command::Jump {
        slot: 0,
        star: lane,
    }]);
    assert!(charging(&events, lane), "{events:?}");
}

/// The route the chart draws and the Jump button charges the first step
/// of: the shortest chain of lanes, both ends in it, the same on every
/// build. `World::route_to` is `Galaxy::route` asked from where the ship
/// is, and the star the ship is at is a route of one.
#[test]
fn the_route_is_the_shortest_chain_of_lanes_from_here() {
    let world = jumper_world();
    let here = world.star_id;
    let galaxy = world.galaxy();
    let hops = galaxy.hops_from(here);

    assert_eq!(world.route_to(here), Some(vec![here]));
    // Every star the lanes reach, and there is no other kind.
    for star in [0u32, 3, 77, 400, 999] {
        let route = world.route_to(star).expect("one connected web");
        assert_eq!(route.first().copied(), Some(here));
        assert_eq!(route.last().copied(), Some(star));
        assert_eq!(route.len() as u16, hops[star as usize] + 1, "star {star}");
        for pair in route.windows(2) {
            assert!(galaxy.lanes(pair[0]).contains(&pair[1]));
        }
        // The first step is a lane out of here, which is what the Jump
        // button charges for.
        if let Some(&next) = route.get(1) {
            assert!(world.laned_to(next));
            assert!(world.reachable_stars().contains(&next));
        }
        // And the same answer twice, whoever asks.
        assert_eq!(world.route_to(star), Some(route));
    }
    // The stars a charge could reach are exactly the lanes out of here.
    assert_eq!(world.reachable_stars(), galaxy.lanes(here).to_vec());
    assert!(!world.reachable_stars().is_empty());
}

// --- 3: the jammer --------------------------------------------------------

/// In an infested system whose jammer still stands, a jump **inward** —
/// to a star fewer hops from the machines' origin — is
/// [`Refusal::Jammed`]; sideways and outward are accepted. And flying
/// *into* an infested system is never refused, which is the other half of
/// the trap: getting in is free, getting back out the way you came is
/// not.
#[test]
fn a_standing_jammer_shuts_the_lanes_inward_and_no_others() {
    let mut world = jumper_world();
    let here = world.star_id;
    let (origin, inward, level, outward) = three_ways(&world);
    world.set_droid_origin_for_probe(origin);
    world.set_crisis_first_day_for_probe(0);
    let h = world.hops_from_origin(here);

    // A day short of this system's own: the star inward has fallen and
    // this one has not, so a jump *into* an infested system is what is
    // being asked for.
    wind_to(&mut world, u32::from(h) * data::DROID_SPREAD_DAYS - 1);
    assert!(!world.infested(here), "not yet theirs");
    assert!(world.infested(inward), "the star inward is");
    assert_eq!(world.jammer_station(), None, "and no jammer here");
    assert!(!world.jammed());
    let events = world.step(&[Command::Jump {
        slot: 0,
        star: inward,
    }]);
    assert!(
        charging(&events, inward),
        "a jump into an infested system is never refused: {events:?}"
    );
    let events = world.step(&[Command::Abort { slot: 0 }]);
    assert!(
        events
            .iter()
            .any(|e| matches!(e, WorldEvent::Aborted { .. }))
    );

    // And now this system's own day: the jammer stands.
    wind_to(&mut world, u32::from(h) * data::DROID_SPREAD_DAYS);
    assert!(world.infested(here));
    assert!(world.jammed(), "the jammer is standing");
    assert!(world.jammer_station().is_some());

    world.man_the_helm_for_probe(0);
    let events = world.step(&[Command::Jump {
        slot: 0,
        star: inward,
    }]);
    assert!(refused_with(&events, Refusal::Jammed), "{events:?}");
    assert_eq!(world.ship.state, ShipState::Holding);

    // Sideways is open.
    let events = world.step(&[Command::Jump {
        slot: 0,
        star: level,
    }]);
    assert!(charging(&events, level), "sideways is open: {events:?}");
    world.step(&[Command::Abort { slot: 0 }]);

    // And so is outward.
    world.man_the_helm_for_probe(0);
    let events = world.step(&[Command::Jump {
        slot: 0,
        star: outward,
    }]);
    assert!(charging(&events, outward), "outward is open: {events:?}");

    // The same rule read off the chart, step by step.
    assert!(world.jammed_step(here, inward));
    assert!(!world.jammed_step(here, level));
    assert!(!world.jammed_step(here, outward));
}

/// Clearing the station the jammer stands on lifts it, and lifts it for
/// good: a load works the hop table and the station out again
/// (`World::settle_crisis`) and the crisis spreading further never
/// re-arms it.
#[test]
fn a_cleared_jammer_station_stays_down() {
    let mut world = jumper_world();
    let here = world.star_id;
    let (origin, ..) = three_ways(&world);
    world.set_droid_origin_for_probe(origin);
    world.set_crisis_first_day_for_probe(0);
    let h = world.hops_from_origin(here);
    wind_to(&mut world, u32::from(h) * data::DROID_SPREAD_DAYS);
    assert!(world.jammed());

    let station = world.jammer_station().expect("infested, so there is one");
    world.infest(station);
    // The last machine of the last wave destroyed, without the fight:
    // `Infestation::cleared` is what `droid_station_cleared` reads and
    // what the crisis step keeps through a save.
    world
        .infested
        .iter_mut()
        .find(|it| it.station == station)
        .expect("just infested")
        .cleared = true;
    assert!(world.droid_station_cleared(station));
    assert!(!world.jammed(), "the jammer is down");
    assert_eq!(
        world.jammer_station(),
        Some(station),
        "and still the station that held it"
    );

    // A load: the hop table and the derived station worked out again.
    world.settle_crisis();
    assert!(!world.jammed(), "still down after a load");

    // And the crisis running on for another month changes nothing here.
    wind_to(&mut world, u32::from(h) * data::DROID_SPREAD_DAYS + 60);
    assert!(world.infested(here));
    assert!(!world.jammed(), "still down after the spread went on");

    // The way inward is open again.
    let (_, inward, ..) = three_ways(&world);
    assert!(!world.jammed_step(here, inward));
    world.man_the_helm_for_probe(0);
    let events = world.step(&[Command::Jump {
        slot: 0,
        star: inward,
    }]);
    assert!(charging(&events, inward), "{events:?}");
}

/// Every infested system has **exactly one** jammer station: the orbital
/// station with the lowest id, and a derived one where the system has
/// none. A town on a planet's surface is never it.
#[test]
fn an_infested_system_has_exactly_one_jammer_station() {
    let mut world = jumper_world();
    let here = world.star_id;
    world.set_droid_origin_for_probe(here);
    world.set_crisis_first_day_for_probe(0);
    assert!(world.infested(here));

    let lowest = world.stations.iter().map(|s| s.id).min().unwrap();
    assert_eq!(world.jammer_station(), Some(lowest));
    assert!(
        !jammer::is_derived(lowest),
        "this system has orbital stations of its own"
    );
    // A settlement is never it, however low its id sorts.
    assert!(!world.surfaces.is_empty(), "the spawn system has ground");
    for surface in &world.surfaces {
        assert_ne!(world.jammer_station(), Some(surface.id));
    }
    // And nothing at all in a system the machines have not got.
    // A crisis ten days off, so day nought has nothing of it.
    world.set_crisis_first_day_for_probe(10);
    wind_to(&mut world, 0);
    assert!(!world.infested(here));
    assert_eq!(world.jammer_station(), None);
    assert!(!world.jammed());
}

/// A system with no orbital station of its own gets one the machines put
/// there: rolled off the star's own stream, standing clear of everything,
/// the same on two clients and after a reload, and with an id that
/// collides with nothing the generator, a raider or a surface numbers.
///
/// The stations are taken out of the system by hand rather than by flying
/// to a starless one: what `settle_jammer` sees is "no station that is not
/// derived", and that is exactly what this arranges.
#[test]
fn a_system_with_no_station_gets_the_machines_own() {
    let mut world = jumper_world();
    let here = world.star_id;
    world.set_droid_origin_for_probe(here);
    world.set_crisis_first_day_for_probe(0);

    world.system.stations.clear();
    world.stations.clear();
    world.station_keys.clear();
    world.settle_jammer();

    let id = jammer::jammer_id(here);
    assert_eq!(world.jammer_station(), Some(id));
    assert!(jammer::is_derived(id));
    assert!(world.jammed());
    // It is a station like any other: found by id, laid out, on the
    // chart, and on the system so a trip can be planned to it.
    let station = world.station(id).expect("a station of the system");
    assert_eq!(station.id, id);
    assert!(station.hostile);
    assert!(station.design.parts.len() > 100, "a real hull");
    assert!(world.discovered.contains(&Node::Station(id)));
    assert!(world.system.station(id).is_some());
    assert_eq!(world.station_keys.len(), world.stations.len());
    assert_eq!(world.station_key(id), 0, "no key on the machines' desk");

    // Clear of everything else the system holds, and clear of where a
    // jump would put the ship.
    let at = world
        .system
        .absolute_position(Node::Station(id))
        .expect("placed");
    for node in world.system.nodes() {
        if node == Node::Station(id) {
            continue;
        }
        let there = world.system.absolute_position(node).unwrap();
        assert!(there.distance(at) >= data::JUMP_CLEARANCE, "{node:?}");
    }
    assert!(crate::jump::landing_point(&world.system).distance(at) >= data::JUMP_CLEARANCE);

    // Its id is nobody else's.
    assert!(crate::surface_body(id).is_none());
    assert!(crate::raider_index(id).is_none());
    assert!(world.surfaces.iter().all(|s| s.id != id));

    // The same station on another client of the same galaxy, and after a
    // reload — which is `settle_crisis`, the call `Game::resume` makes.
    let mut twin = jumper_world();
    twin.set_droid_origin_for_probe(here);
    twin.set_crisis_first_day_for_probe(0);
    twin.system.stations.clear();
    twin.stations.clear();
    twin.station_keys.clear();
    twin.settle_jammer();
    assert_eq!(twin.station(id), world.station(id));
    let was = world.station(id).cloned();
    world.settle_crisis();
    assert_eq!(world.station(id).cloned(), was, "a reload rolls the same");
    assert_eq!(
        world.stations.iter().filter(|s| s.id == id).count(),
        1,
        "and lays exactly one of it"
    );

    // And it goes away again the moment the system is not theirs.
    // A crisis ten days off, so day nought has nothing of it.
    world.set_crisis_first_day_for_probe(10);
    wind_to(&mut world, 0);
    world.settle_jammer();
    assert!(world.station(id).is_none());
    assert!(!world.discovered.contains(&Node::Station(id)));
    assert_eq!(world.station_keys.len(), world.stations.len());
}

// --- 4: the tier by distance ---------------------------------------------

/// The machines come at **tier three within
/// [`data::DROID_TIER_THREE_HOPS`] hops of their origin** and tier one
/// anywhere else. The probes' dial still wins over it.
#[test]
fn the_machines_come_at_tier_three_near_their_origin() {
    use bims::combat::Tier;
    let mut world = jumper_world();
    let here = world.star_id;
    let galaxy = world.galaxy();

    for hops in 0..=data::DROID_TIER_THREE_HOPS {
        let origin = (0..galaxy.stars.len() as u32)
            .find(|&o| galaxy.hops_from(o)[here as usize] == hops)
            .unwrap_or_else(|| panic!("no star {hops} hops off"));
        world.set_droid_origin_for_probe(origin);
        assert_eq!(world.hops_from_origin(here), hops);
        assert_eq!(world.droid_tier(), Tier::Three, "{hops} hops off");
    }
    for hops in [data::DROID_TIER_THREE_HOPS + 1, 6, 12] {
        let origin = (0..galaxy.stars.len() as u32)
            .find(|&o| galaxy.hops_from(o)[here as usize] == hops)
            .unwrap_or_else(|| panic!("no star {hops} hops off"));
        world.set_droid_origin_for_probe(origin);
        assert_eq!(world.droid_tier(), Tier::One, "{hops} hops off");
    }

    // And `BIMS_DROID_TIER` over the lot, either way.
    world.set_droid_tier_for_probe(Some(Tier::Two));
    assert_eq!(world.droid_tier(), Tier::Two);
    world.set_droid_tier_for_probe(None);
    assert_eq!(world.droid_tier(), Tier::One, "the distance rule is back");
}

// --- 7: two clients ------------------------------------------------------

/// Two clients of the same galaxy agree about a jam, about a jammer
/// brought down and about where a jump puts them, and the checksum says
/// so.
#[test]
fn two_worlds_on_one_seed_are_jammed_and_freed_alike() {
    let mut a = jumper_world();
    let mut b = jumper_world();
    let here = a.star_id;
    let (origin, inward, _, outward) = three_ways(&a);
    let h = a.galaxy().hops_from(origin)[here as usize];
    for world in [&mut a, &mut b] {
        world.set_droid_origin_for_probe(origin);
        world.set_crisis_first_day_for_probe(0);
        wind_to(world, u32::from(h) * data::DROID_SPREAD_DAYS);
        world.man_the_helm_for_probe(0);
    }
    assert!(a.jammed() && b.jammed());
    assert_eq!(a.jammer_station(), b.jammer_station());
    assert_eq!(world_checksum(&a), world_checksum(&b));

    // A jump inward, refused on both.
    for world in [&mut a, &mut b] {
        world.step(&[Command::Jump {
            slot: 0,
            star: inward,
        }]);
    }
    assert_eq!(world_checksum(&a), world_checksum(&b));
    assert_eq!(a.ship.state, ShipState::Holding);

    // The jammer station cleared on both, and the checksum notices — the
    // infestation it is recorded in is hashed whole.
    let station = a.jammer_station().unwrap();
    let before = world_checksum(&a);
    for world in [&mut a, &mut b] {
        world.infest(station);
        world
            .infested
            .iter_mut()
            .find(|it| it.station == station)
            .unwrap()
            .cleared = true;
    }
    assert_ne!(before, world_checksum(&a), "a cleared station is hashed");
    assert_eq!(world_checksum(&a), world_checksum(&b));
    assert!(!a.jammed() && !b.jammed());

    // And a jump outward carried through on both lands the same world.
    let steps = (data::JUMP_CHARGE_MINUTES / data::STEP_MINUTES).ceil() as u32 + 5;
    for world in [&mut a, &mut b] {
        world.man_the_helm_for_probe(0);
        world.step(&[Command::Jump {
            slot: 0,
            star: outward,
        }]);
        for _ in 0..steps {
            world.step(&[]);
        }
    }
    assert_eq!(a.star_id, outward);
    assert_eq!(b.star_id, outward);
    assert_eq!(world_checksum(&a), world_checksum(&b));
}

// --- 5: the measurements -------------------------------------------------

/// What the jammer costs a galaxy, over ten seeds: how many systems have
/// no orbital station of their own and so need one of the machines' own,
/// and how far the starting star is from the origin. Printed rather than
/// asserted — the numbers are in the root `CLAUDE.md`.
#[test]
#[ignore]
fn jammers_over_ten_seeds() {
    println!("seed  systems  derived jammers  share  hops start->origin");
    let (mut worst, mut best) = (0u16, u16::MAX);
    for n in 0..10u64 {
        let seed = data::DEFAULT_SEED.wrapping_add(n.wrapping_mul(0x9e37_79b9));
        let galaxy = Galaxy::new(seed, GalaxyType::SpiralTwoArm);
        let systems = galaxy.every_system();
        let total = systems.len();
        let derived = systems.iter().filter(|s| s.stations.is_empty()).count();
        // The origin the way `World::start` takes it: off the dock
        // `spawn_anywhere` picks.
        let (start, _) = crate::spawn_anywhere(&galaxy, n).expect("a dock somewhere");
        let origin = crate::droid::origin(&galaxy, start);
        let hops = galaxy.hops_from(start)[origin as usize];
        worst = worst.max(hops);
        best = best.min(hops);
        println!(
            "{seed:>20}  {total:>7}  {derived:>15}  {:>4.0}%  {hops:>3}",
            100.0 * derived as f64 / total as f64
        );
    }
    println!("hops from the start to the origin: {best} to {worst}");
}
