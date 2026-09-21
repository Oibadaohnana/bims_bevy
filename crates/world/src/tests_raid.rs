//! What a raid has to be true of: `crate::raid`. The raider's hull, the
//! schedule, the contact and the warning, the arrival and the boarding,
//! the two ways a raid is called off, that two runs on one seed are
//! raided alike, and the end of the run. The retreat the whole thing
//! rests on — the enemy following the crew aboard and forcing the ship's
//! locked airlock — is pinned next door in `tests.rs`
//! (`enemies_follow_a_crew_that_retreats_aboard_and_force_the_ship_s_locked_airlock`).

use physics::ResourceId;
use shipdesign::fixture::flyer;
use shipdesign::{Budget, Edit, PartKind, Rotation, ShipDesign, apply, has_errors, validate};
use worldgen::math::dvec2;

use crate::data;
use crate::event::WorldEvent;
use crate::fixture::{REFERENCE_MONEY, simulation_world};
use crate::raid::{Raid, Raids, boarders_of, raider_id, raider_index};
use crate::speed::Speed;
use crate::station::{Plan, layout_raider};
use crate::world::{Command, ShipState, World};
use crate::world_checksum;
use crate::{Target, surface_body};

fn basic() -> World {
    simulation_world(flyer(2), REFERENCE_MONEY, 2)
}

/// How many steps a departure from the berth is allowed: an hour, the
/// casting-off limit, and a little over.
const LEAVING: u32 = 62 * 60;

/// Confirm a trip from wherever the ship is with `slot` at the helm and
/// run until it is under way.
fn set_off(world: &mut World, slot: u32, target: Target) -> Vec<WorldEvent> {
    world.man_the_helm_for_probe(slot);
    let mut seen = world.step(&[Command::Confirm { slot, target }]);
    for _ in 0..LEAVING {
        if matches!(world.ship.state, ShipState::Travelling { .. }) {
            return seen;
        }
        seen.extend(world.step(&[]));
    }
    panic!("the ship never set off: {:?}", world.ship.state);
}

/// Run until the ship is at rest, or give up.
fn until_stopped(world: &mut World, limit: u32) -> Vec<WorldEvent> {
    let mut seen = Vec::new();
    for _ in 0..limit {
        seen.extend(world.step(&[]));
        if matches!(
            world.ship.state,
            ShipState::Holding | ShipState::Docked { .. }
        ) {
            return seen;
        }
    }
    panic!("the trip never ended");
}

/// The ship off its berth and holding a short way out: where a raid can
/// find it. A short trip rather than `undock_for_probe`, so the crew have
/// left home the way the schedule counts it.
fn holding_out(world: &mut World) {
    let there = Target::Point(world.ship.position().add(dvec2(40_000.0, 15_000.0)));
    set_off(world, 0, there);
    until_stopped(world, 200_000);
    assert!(matches!(world.ship.state, ShipState::Holding));
}

/// Step until the raider makes contact; the events of that step.
fn until_contact(world: &mut World, limit: u32) -> Vec<WorldEvent> {
    for _ in 0..limit {
        let events = world.step(&[]);
        if events
            .iter()
            .any(|e| matches!(e, WorldEvent::RaidContact { .. }))
        {
            return events;
        }
    }
    panic!("no contact: {:?}", world.raid());
}

/// The minutes out the contact said.
fn minutes_out(events: &[WorldEvent]) -> u32 {
    events
        .iter()
        .find_map(|e| match e {
            WorldEvent::RaidContact { minutes, .. } => Some(*minutes),
            _ => None,
        })
        .expect("a contact")
}

/// A raider is a hull the room can live in: valid as a station is, a port
/// in its west skin, a bunk a boarder, a reactor, a shelf, cover, light —
/// and every use spot and every tile of deck reachable from inside the
/// port, the walkability contract every station keeps. Its plan is not
/// rolled and not on `Plan::ALL`, and its id is nobody else's.
#[test]
fn a_raider_is_a_place_the_room_can_live_in_and_can_be_walked() {
    use shipdesign::validate::walkable;
    let tile = shipdesign::TILE as f32;
    let middle =
        |(x, y): (u32, u32)| bims::math::vec2((x as f32 + 0.5) * tile, (y as f32 + 0.5) * tile);
    assert!(!Plan::ALL.contains(&Plan::Raider));
    for seed in [1, 7, 0x_5749_4e44_4f57_0001] {
        let design = layout_raider(seed);
        assert_eq!(design.build_area, data::RAIDER_SIDE);
        // Valid bar what it has not got on purpose: a raider has no galley
        // and no heads (codes 2 to 10), and every other error is a fault.
        let report = validate(&design, 1);
        let faults: Vec<_> = report
            .iter()
            .filter(|i| i.severity == shipdesign::validate::Severity::Error)
            .filter(|i| !(2..=10).contains(&(i.code as u32)))
            .collect();
        assert!(faults.is_empty(), "seed {seed}: {faults:?}");
        assert!(has_errors(&report), "no galley, no heads: not a ship");
        let count = |kind| design.parts.iter().filter(|p| p.kind == kind).count() as u32;
        assert_eq!(
            count(PartKind::Bunk),
            data::BOARDERS_MAX,
            "a bunk a boarder"
        );
        assert_eq!(count(PartKind::Reactor), 1);
        assert!(count(PartKind::Shelf) >= 1, "a shelf with loot on it");
        assert!(count(PartKind::Sandbags) >= 1, "somewhere to duck");
        assert!(count(PartKind::WallLight) >= 4, "lit");
        assert_eq!(count(PartKind::Airlock), 1, "the port and nothing else");
        let port = shipdesign::port(&design).expect("a port");
        assert_eq!(port.outward, (-1, 0), "in the west skin");

        let grid = design.grid();
        let room = bims::room::Room::from_layout(bims::aboard::layout_of(&design));
        let nav = bims::nav::Nav::tiled(
            room.interior,
            &room.solids(),
            bims::character::BODY_MARGIN,
            tile,
        );
        let inside = (
            (port.centre.0 / tile as f64) as u32 + 1,
            (port.centre.1 / tile as f64) as u32,
        );
        let from = nav.nearest_free(middle(inside));
        let mut cut_off = Vec::new();
        for part in &design.parts {
            for spot in part.use_spots() {
                let spot = (spot.0 as u32, spot.1 as u32);
                if nav.path(from, middle(spot)).is_empty() {
                    cut_off.push((part.kind, spot));
                }
            }
        }
        assert!(cut_off.is_empty(), "seed {seed}: no route to {cut_off:?}");
        let mut pockets = Vec::new();
        for y in 0..design.build_area as i32 {
            for x in 0..design.build_area as i32 {
                if walkable(&design, &grid, (x, y))
                    && nav.path(from, middle((x as u32, y as u32))).is_empty()
                {
                    pockets.push((x, y));
                }
            }
        }
        assert!(pockets.is_empty(), "seed {seed}: pockets at {pockets:?}");
    }
    // The ids: clear of the generator's, and of a planet's surface.
    assert_eq!(raider_index(raider_id(3)), Some(3));
    assert_eq!(raider_index(3), None);
    assert_eq!(raider_index(crate::surface_id(3)), None);
    assert_eq!(surface_body(raider_id(3)), None);
}

/// The boarders: one, one a month gone and one a crewmate, doubled with
/// the worth, capped at six — where a station's garrison goes to sixteen.
/// A raid rolled thirty days in carries the month's extra boarder.
#[test]
fn boarders_are_counted_like_a_garrison_to_a_lower_cap() {
    assert_eq!(boarders_of(2, 1_000, 1_000, 0), 3);
    assert_eq!(boarders_of(2, 1_500, 1_000, 0), 6, "one doubling, capped");
    assert_eq!(boarders_of(1, 1_499, 1_000, 0), 2, "not yet a half");
    assert_eq!(boarders_of(14, 0, 0, 0), data::BOARDERS_MAX);
    assert_eq!(boarders_of(1, 100_000, 1_000, 0), data::BOARDERS_MAX);
    assert_eq!(
        crate::station::enemies_of(20, 0, 0, 0),
        data::ENEMIES_MAX,
        "a garrison's cap is its own"
    );
    // The calendar under the doublings, to the same cap.
    let month = data::ENEMIES_DAYS;
    assert_eq!(boarders_of(2, 1_000, 1_000, month - 1), 3);
    assert_eq!(boarders_of(2, 1_000, 1_000, month), 4, "a month: one more");
    assert_eq!(boarders_of(2, 1_000, 1_000, 2 * month), 5);
    assert_eq!(boarders_of(1, 1_500, 1_000, month), 6, "three doubled, capped");
    assert_eq!(boarders_of(2, 1_000, 1_000, 10 * month), data::BOARDERS_MAX);

    // And a raid is rolled at the clock's day: the same crew, thirty
    // days on, are boarded by four rather than three.
    let mut world = basic();
    holding_out(&mut world);
    world.clock_minutes = time::minutes(30.0);
    assert_eq!(world.days_gone(), 30);
    world.raid_now_for_probe();
    until_contact(&mut world, 120);
    let Raid::Closing { boarders, .. } = world.raid().clone() else {
        panic!("closing: {:?}", world.raid());
    };
    assert_eq!(boarders, 4, "four against two, a month in");
}

/// The schedule is the seed's: whole minutes, a day to three days apart,
/// and none before the crew have left home. Docked at the spawn the due
/// minute goes by and nothing comes.
#[test]
fn a_raid_waits_for_the_crew_to_leave_home_and_for_a_hold() {
    for n in 0..8 {
        let gap = Raids::gap(data::DEFAULT_SEED, n);
        assert!(gap >= data::RAID_GAP_MIN, "raid {n}: {gap}");
        assert!(
            gap < data::RAID_GAP_MIN + data::RAID_GAP_SPREAD,
            "raid {n}: {gap}"
        );
    }
    let mut world = basic();
    assert!(!world.raids.left_home);
    // Brought forward to now, and still nothing: the ship is tied up at
    // home and the crew have never left it.
    world.raids.due = 0;
    for _ in 0..600 {
        let events = world.step(&[]);
        assert!(
            !events
                .iter()
                .any(|e| matches!(e, WorldEvent::RaidContact { .. })),
            "raided at the berth"
        );
    }
    assert!(!world.raids.left_home);
    assert_eq!(*world.raid(), Raid::Quiet);
}

/// A raid staged to come in ten minutes — the `raid` command — puts the
/// ship off its berth holding in open space with nothing on the radar,
/// and contact is made at the first step of the tenth minute, not
/// before: ten seconds at 1×, the raider then closing at its own pace.
#[test]
fn a_raid_staged_to_come_in_ten_minutes_makes_contact_on_the_tenth() {
    let mut world = basic();
    let berth = world.ship.position();
    assert!(world.raid_coming_for_probe(10));
    assert!(matches!(world.ship.state, ShipState::Holding));
    assert!(world.ship.position().distance(berth) > 1_000.0, "off the berth");
    assert_eq!(*world.raid(), Raid::Quiet);
    assert!(world.raid_contact().is_none(), "nothing on the radar yet");
    let began = world.clock_minutes;
    // Nine minutes and every step short of the tenth: quiet.
    let steps_a_minute = (1.0 / data::STEP_MINUTES).round() as u32;
    for _ in 0..(10 * steps_a_minute - 1) {
        let events = world.step(&[]);
        assert!(
            !events
                .iter()
                .any(|e| matches!(e, WorldEvent::RaidContact { .. })),
            "contact early, at {:.2} minutes",
            world.clock_minutes - began
        );
        assert_eq!(*world.raid(), Raid::Quiet);
    }
    let events = until_contact(&mut world, 2);
    assert!((world.clock_minutes - began - 10.0).abs() < data::STEP_MINUTES * 1.5);
    assert!(matches!(world.raid(), Raid::Closing { .. }));
    assert!(world.raid_contact().is_some(), "the raider on the radar");
    assert_eq!(minutes_out(&events) as f64, (world.detection_range() / data::RAIDER_SPEED).ceil());
}

/// A raid arrives while holding: contact at the edge of the radar with
/// the warning the sensors buy, every player back to 1× once, the raider
/// drawn closing on a straight line, and on arrival the ship docked to
/// it — its people enemies, as many as the formula says, posted at the
/// ship's gangway and coming through the passage onto its deck.
#[test]
fn a_raid_arrives_while_holding_and_the_boarders_come_for_the_ship() {
    let mut world = basic();
    holding_out(&mut world);
    assert!(world.raids.left_home, "the crew have left home");
    world.request_speed(0, Speed::Top);
    world.request_speed(1, Speed::Day);
    world.raid_now_for_probe();
    let here = world.ship.position();
    let events = until_contact(&mut world, 120);
    let minutes = minutes_out(&events);
    // The flyer has one sensor array: half an hour's warning.
    let range = world.detection_range();
    assert_eq!(range, data::RADAR_RANGE_PER_SENSOR);
    assert_eq!(minutes as f64, (range / data::RAIDER_SPEED).ceil());
    assert_eq!(
        world.speed_requests,
        vec![Speed::Real, Speed::Real],
        "everybody back to real time"
    );
    // And it is a reset, not a veto.
    world.request_speed(0, Speed::Top);
    assert_eq!(world.speed_requests[0], Speed::Top);
    world.request_speed(0, Speed::Real);
    let Raid::Closing {
        boarders, from, at, ..
    } = world.raid().clone()
    else {
        panic!("closing: {:?}", world.raid());
    };
    assert_eq!(
        boarders,
        boarders_of(2, world.worth(), world.start_worth, 0),
        "three against two"
    );
    assert_eq!(boarders, 3);
    assert!(
        (from.distance(here) - range).abs() < 1.0,
        "at the edge of the radar"
    );
    assert_eq!(at, here);
    let first = world.raid_contact().expect("on the radar");
    assert!(first.distance(from) < 1.0);

    // Half way there, half way along the line.
    let steps_out = minutes * 60;
    for _ in 0..steps_out / 2 {
        world.step(&[]);
    }
    let mid = world.raid_contact().expect("still closing");
    let along = mid.sub(from).length() / at.sub(from).length();
    assert!((along - 0.5).abs() < 0.02, "half way: {along}");

    // Arrived: tied up, and the rooms joined.
    let mut boarded = None;
    for _ in 0..steps_out / 2 + 120 {
        let events = world.step(&[]);
        if let Some(e) = events
            .iter()
            .find(|e| matches!(e, WorldEvent::RaidBoarded { .. }))
        {
            boarded = Some(e.clone());
            break;
        }
    }
    assert_eq!(boarded, Some(WorldEvent::RaidBoarded { boarders }));
    let ShipState::Docked { station } = world.ship.state else {
        panic!("docked, not {:?}", world.ship.state);
    };
    assert_eq!(station, raider_id(0));
    assert!(world.raided());
    assert_eq!(world.stance(station), bims::sight::Stance::Hostile);
    assert!(world.aboard.is_joined(), "one deck");
    assert!(
        world.ship.position().distance(here) < 1e-6,
        "the ship did not move: the raider came to it"
    );
    let raider = world.station(station).expect("the raider is a station");
    assert_eq!(raider.plan, Plan::Raider);
    assert!(raider.market().is_none(), "no desk");
    let residents = world.residents.as_ref().expect("the boarders' room");
    assert_eq!(residents.station, station);
    assert_eq!(residents.aboard.count(), boarders);
    assert!(
        (0..boarders as usize).all(|who| residents.aboard.room.has_post(who)),
        "posted at the gangway"
    );
    assert!(
        (0..boarders as usize).all(|who| residents.aboard.room.weapon(who).is_some()),
        "armed"
    );
    // The next raid is on the schedule after this one.
    assert_eq!(world.raids.next, 1);
    assert!(world.raids.due > world.clock_minutes as u64);

    // They come: within a couple of minutes at least one boarder is on the
    // ship's own deck, through the passage.
    let ship = world.ship.design.clone();
    let on_ship = |world: &World| {
        let (origin, ex, ey) = world.aboard.station_frame.unwrap();
        let residents = world.residents.as_ref().unwrap();
        (0..residents.aboard.count()).any(|who| {
            let p = residents.aboard.exposed(who);
            let at = origin
                .add(ex.scale(p.x))
                .add(ey.scale(p.y))
                .sub(world.aboard.offset);
            let t = shipdesign::TILE as f64;
            let tile = ((at.x / t).floor() as i32, (at.y / t).floor() as i32);
            ship.grid().get(shipdesign::Layer::Structure, tile) != 0
        })
    };
    let mut aboard = false;
    for _ in 0..3_000 {
        world.step(&[]);
        if on_ship(&world) {
            aboard = true;
            break;
        }
    }
    assert!(aboard, "a boarder came onto the ship's deck");
}

/// A raid is cancelled by leaving: the raider arrives to find the ship
/// under way, says so, and there is no raider.
#[test]
fn a_raid_is_cancelled_by_leaving() {
    let mut world = basic();
    holding_out(&mut world);
    world.raid_now_for_probe();
    let events = until_contact(&mut world, 120);
    let minutes = minutes_out(&events);
    // Off again, a long way, before it gets here.
    let far = Target::Point(world.ship.position().add(dvec2(2_000_000.0, 0.0)));
    set_off(&mut world, 0, far);
    assert!(matches!(world.raid(), Raid::Closing { .. }));
    let mut cancelled = false;
    for _ in 0..minutes * 60 + 120 {
        let events = world.step(&[]);
        if events.contains(&WorldEvent::RaidCancelled) {
            cancelled = true;
            break;
        }
        assert!(
            !events
                .iter()
                .any(|e| matches!(e, WorldEvent::RaidBoarded { .. })),
            "boarded under way"
        );
    }
    assert!(cancelled);
    assert_eq!(*world.raid(), Raid::Quiet);
    assert!(matches!(world.ship.state, ShipState::Travelling { .. }));
    assert!(!world.raided());
    assert_eq!(
        world
            .stations
            .iter()
            .filter(|s| s.plan == Plan::Raider)
            .count(),
        0
    );
}

/// A raid is cancelled by a completed jump: the raider is left behind
/// with the system.
#[test]
fn a_raid_is_cancelled_by_a_completed_jump() {
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
    let mut world = simulation_world(design, REFERENCE_MONEY, 2);
    holding_out(&mut world);
    world.raid_now_for_probe();
    let events = until_contact(&mut world, 120);
    let minutes = minutes_out(&events);
    // The charge is twenty minutes, the raider half an hour out.
    assert!((data::JUMP_CHARGE_MINUTES as u32) < minutes);
    let from = world.star_id;
    let to = (from + 1) % world.galaxy().stars.len() as u32;
    world.man_the_helm_for_probe(0);
    let events = world.step(&[Command::Jump { slot: 0, star: to }]);
    assert!(
        events
            .iter()
            .any(|e| matches!(e, WorldEvent::Charging { .. })),
        "{events:?}"
    );
    let mut seen = Vec::new();
    for _ in 0..minutes * 60 + 120 {
        seen.extend(world.step(&[]));
        if seen.contains(&WorldEvent::RaidCancelled) {
            break;
        }
    }
    let jumped = seen
        .iter()
        .position(|e| matches!(e, WorldEvent::Jumped { .. }));
    let cancelled = seen.iter().position(|e| *e == WorldEvent::RaidCancelled);
    assert!(jumped.is_some(), "{seen:?}");
    assert!(cancelled.is_some(), "{seen:?}");
    // Said with the jump, ahead of it: the raider is left with the system.
    assert!(cancelled < jumped, "cancelled with the jump: {seen:?}");
    assert_eq!(world.star_id, to);
    assert_eq!(*world.raid(), Raid::Quiet);
    assert!(
        !seen
            .iter()
            .any(|e| matches!(e, WorldEvent::RaidBoarded { .. }))
    );
}

/// The warning grows with the sensors: a ship with none sees the raider
/// a minute out, the flyer with its one array half an hour, and two
/// arrays an hour.
#[test]
fn warning_time_grows_with_sensors() {
    let warning = |design: ShipDesign| {
        let mut world = simulation_world(design, REFERENCE_MONEY, 2);
        holding_out(&mut world);
        world.raid_now_for_probe();
        let events = until_contact(&mut world, 120);
        (world.detection_range(), minutes_out(&events))
    };
    let budget = Budget::new(10_000_000);
    let one = flyer(2);
    let array = one
        .parts
        .iter()
        .find(|p| p.kind == PartKind::SensorArray)
        .expect("the flyer has an array")
        .id;
    let none = apply(&one, &budget, Edit::Remove { part_id: array }).expect("taken off");
    // A second array in the north skin, on the plating's tile.
    let plating = one
        .parts
        .iter()
        .find(|p| p.kind == PartKind::OutsideWall && p.origin == (12, 1))
        .expect("plating at (12, 1)")
        .id;
    let two = apply(&one, &budget, Edit::Remove { part_id: plating }).expect("opened");
    let two = apply(
        &two,
        &budget,
        Edit::Place {
            kind: PartKind::SensorArray,
            origin: (12, 1),
            rotation: Rotation::R0,
        },
    )
    .expect("a second array");
    let (r0, m0) = warning(none);
    let (r1, m1) = warning(one);
    let (r2, m2) = warning(two);
    assert_eq!(r0, data::VISION_RANGE);
    assert_eq!(r1, data::RADAR_RANGE_PER_SENSOR);
    assert_eq!(r2, 2.0 * data::RADAR_RANGE_PER_SENSOR);
    assert_eq!(m0, 1);
    assert_eq!(m1, 30);
    assert_eq!(m2, 60);
    assert!(m0 < m1 && m1 < m2);
}

/// Two runs on one seed raid at the same minutes with the same boarders,
/// from the same bearing, and agree to the checksum through the whole of
/// it — the contact, the arrival, the boarding and the fight.
#[test]
fn two_runs_on_one_seed_raid_at_the_same_minutes_with_the_same_boarders() {
    let mut a = basic();
    let mut b = basic();
    let there = Target::Point(a.ship.position().add(dvec2(40_000.0, 15_000.0)));
    for world in [&mut a, &mut b] {
        set_off(world, 0, there);
        until_stopped(world, 200_000);
        world.raid_now_for_probe();
    }
    assert_eq!(world_checksum(&a), world_checksum(&b));
    let mut contact = (None, None);
    let mut boarded = (None, None);
    for step in 0..40 * 60 {
        let ea = a.step(&[]);
        let eb = b.step(&[]);
        assert_eq!(ea, eb, "step {step}");
        assert_eq!(world_checksum(&a), world_checksum(&b), "step {step}");
        if ea
            .iter()
            .any(|e| matches!(e, WorldEvent::RaidContact { .. }))
        {
            contact = (Some(step), Some(a.raid().clone()));
        }
        if ea
            .iter()
            .any(|e| matches!(e, WorldEvent::RaidBoarded { .. }))
        {
            boarded = (Some(step), Some(b.raid().clone()));
            break;
        }
    }
    assert!(contact.0.is_some(), "contact");
    assert!(boarded.0.is_some(), "boarded");
    assert_eq!(a.raid(), b.raid());
    let Raid::Docked { boarders, .. } = a.raid() else {
        panic!()
    };
    assert_eq!(*boarders, 3);
    // And the schedule agrees on the next one.
    assert_eq!(a.raids.due, b.raids.due);
    assert_eq!(a.raids.next, b.raids.next);
}

/// Boarders all down: the raider is a derelict tied to the ship, said
/// once; a body on it can be looted as at any hostile dock; and casting
/// off removes it — no raider in the world, its people gone, the frame
/// what it was.
#[test]
fn a_repelled_raider_is_a_derelict_until_the_ship_casts_off() {
    let mut world = basic();
    holding_out(&mut world);
    let frame = world.ship.frame;
    world.raid_now_for_probe();
    let events = until_contact(&mut world, 120);
    let minutes = minutes_out(&events);
    for _ in 0..minutes * 60 + 120 {
        if world.raided() {
            break;
        }
        world.step(&[]);
    }
    assert!(world.raided());
    assert_eq!(world.ship.frame, frame, "the view is about what it was");
    let station = world.ship.state.station().unwrap();
    // Every boarder shot where it stands.
    {
        let residents = world.residents.as_mut().unwrap();
        for who in 0..residents.aboard.count() as usize {
            residents.aboard.room.kill_for_probe(who);
        }
    }
    let events = world.step(&[]);
    assert!(events.contains(&WorldEvent::RaidRepelled), "{events:?}");
    let events = world.step(&[]);
    assert!(!events.contains(&WorldEvent::RaidRepelled), "said once");
    assert!(matches!(world.raid(), Raid::Docked { repelled: true, .. }));
    assert!(world.is_down(crate::LootSource::Resident(0)), "lootable");
    assert!(world.station(station).is_some(), "still there to loot");
    // And its shelf is loot too: an orbital's stock, laid out the moment
    // the rooms joined.
    let shelf = world.plunder_alongside().expect("a raider's shelf");
    assert_eq!(shelf.station, station);
    assert!(shelf.grid.units_of(ResourceId::Ore) > 0, "{shelf:?}");
    assert!(!world.station_shelves().is_empty());

    // Cast off: the raider is gone with the push-off.
    let away = Target::Point(world.ship.position().add(dvec2(60_000.0, 0.0)));
    set_off(&mut world, 0, away);
    assert_eq!(*world.raid(), Raid::Quiet);
    assert!(world.station(station).is_none(), "no derelict left behind");
    assert!(!world.hostile.contains(&station));
    assert!(!world.raided());
    assert!(world.plunder.is_empty(), "its shelf went with it");
    assert!(
        world
            .residents
            .as_ref()
            .is_none_or(|r| r.station != station),
        "its people gone"
    );
    // The next raid is on the schedule, and comes in its turn.
    assert_eq!(world.raids.next, 1);
    until_stopped(&mut world, 400_000);
    world.raid_now_for_probe();
    let _ = until_contact(&mut world, 120);
    assert!(matches!(world.raid(), Raid::Closing { n: 1, .. }));
}

/// A lost fight reaches the end: no crew member standing — dead, or out
/// cold — and the world says the run is over, once, and keeps saying it
/// is (`World::lost`), which is what the app's screen reads. One of them
/// on their feet, and it is not over.
#[test]
fn a_lost_fight_reaches_the_end_screen() {
    let mut world = basic();
    assert!(!world.lost);
    world.aboard.room.kill_for_probe(0);
    let events = world.step(&[]);
    assert!(
        !events.contains(&WorldEvent::CrewLost),
        "one still standing"
    );
    assert!(!world.lost);
    world.aboard.room.knock_out_for_probe(1);
    let events = world.step(&[]);
    assert!(events.contains(&WorldEvent::CrewLost), "{events:?}");
    assert!(world.lost);
    let events = world.step(&[]);
    assert!(!events.contains(&WorldEvent::CrewLost), "said once");
    assert!(world.lost);
    assert_eq!(WorldEvent::CrewLost.code(), 63);
    // And a world with everybody down from a raid says the same.
    let mut raided = basic();
    holding_out(&mut raided);
    raided.raid_now_for_probe();
    let events = until_contact(&mut raided, 120);
    let minutes = minutes_out(&events);
    for _ in 0..minutes * 60 + 120 {
        if raided.raided() {
            break;
        }
        raided.step(&[]);
    }
    assert!(raided.raided());
    raided.aboard.room.kill_for_probe(0);
    raided.aboard.room.kill_for_probe(1);
    let events = raided.step(&[]);
    assert!(events.contains(&WorldEvent::CrewLost));
}
