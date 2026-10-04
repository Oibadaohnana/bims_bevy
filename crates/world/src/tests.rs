//! What the world has to be true of.
//!
//! Most of these run the real fixture in the real spawn system, because
//! almost everything here is about how the parts fit together rather than
//! about arithmetic. Where a scenario would otherwise depend on where the
//! generator happened to put a planet, the probe seams on [`World`] are
//! used instead: see `discover_for_probe` and `put_for_probe`.

use bims::combat::Tier;
use economy::Money;
use physics::ResourceId;
use shipdesign::fixture::flyer;
use shipdesign::parts::PartKind;
use shipdesign::{Budget, Edit, Rotation, ShipDesign, apply};
use worldgen::math::{DVec2, dvec2};
use worldgen::{GalaxyType, StationKind};

use crate::data;
use crate::event::{Refusal, WorldEvent};
use crate::fixture::{REFERENCE_MONEY, simulation_world};
use crate::frame::Frame;
use crate::speed::Speed;
use crate::station::Station;
use crate::world::{Command, ShipState, World};
use crate::world_checksum;

fn close(a: f64, b: f64) -> bool {
    (a - b).abs() <= 1e-6 * a.abs().max(b.abs()).max(1.0)
}

/// A world with a named amount of money, for the trading tests.
fn world_with(design: ShipDesign, money: Money, players: u32) -> World {
    simulation_world(design, money, players)
}

pub(crate) fn basic() -> World {
    world_with(flyer(2), REFERENCE_MONEY, 2)
}

/// The machine a fight was staged with (`stage_droid_fight_for_probe`):
/// the one body of a held station's room, to read.
fn machine(world: &World) -> &bims::droid::Droid {
    world
        .residents
        .as_ref()
        .expect("alongside")
        .aboard
        .room
        .droid(0)
        .expect("the staged machine")
}

/// The same machine, to change by hand.
fn machine_mut(world: &mut World) -> &mut bims::droid::Droid {
    world
        .residents
        .as_mut()
        .expect("alongside")
        .aboard
        .room
        .droid_mut_for_probe(0)
        .expect("the staged machine")
}

/// What is left of the staged machine, its four parts added up.
fn machine_health(world: &World) -> f32 {
    let body = machine(world).body;
    bims::droid::DroidPart::ALL
        .iter()
        .map(|&p| body.health(p))
        .sum()
}

/// The staged machine whole again, the way a crew member is patched up —
/// a wreck stood up again with it: the test wants a target, not a fight
/// won.
fn mend_machine(world: &mut World) {
    let droid = machine_mut(world);
    droid.body = bims::droid::DroidBody::new(droid.kind, droid.tier);
    droid.destroyed = false;
}

/// The staged machine stood at a point of its own room and whole again:
/// what a test that wants the distance it asked for does before every
/// step, the way it puts a crew member back.
fn hold_machine(world: &mut World, at: bims::math::Vec2) {
    mend_machine(world);
    let droid = machine_mut(world);
    droid.halt();
    droid.pos = at;
}

/// How long a walk across the joined deck is given, in steps: an hour and
/// a little over, the length of the biggest station there and back.
const LEAVING: u32 = 62 * 60;

// --- starting ---------------------------------------------------------------

#[test]
fn a_world_opens_docked_at_the_spawn_station_with_the_money_left_over() {
    let world = basic();
    let ShipState::Docked { station } = world.ship.state else {
        panic!("it should start docked, not {:?}", world.ship.state);
    };
    assert_eq!(world.crew_money(), REFERENCE_MONEY);
    assert_eq!(world.clock_minutes, 0.0);
    assert_eq!(world.steps, 0);

    // Docked means alongside: the ship is at its berth, which is beside
    // the station rather than on top of it.
    let berth = world
        .berth_at(station)
        .expect("the spawn station has a door");
    assert!(world.ship.position().distance(berth.position) < 1e-9);
    assert_eq!(world.ship.heading, berth.heading);
    let at = world
        .system
        .absolute_position(worldgen::Node::Station(station))
        .unwrap();
    // Half the side, not the half-diagonal: the berth is at the middle of
    // a face, inside the circle round a big station's corners; tile for
    // tile is `the_ship_docks_airlock_to_airlock_outside_the_station`.
    let half =
        world.station(station).unwrap().design.build_area as f64 * shipdesign::TILE as f64 / 2.0;
    assert!(
        world.ship.position().distance(at) > half,
        "the ship is inside the station"
    );

    // And the dock the ship is tied to is something the crew have seen —
    // as is everything else in the system, off the lobby's chart.
    assert!(world.discovered.contains(&worldgen::Node::Station(station)));
    assert_eq!(world.discovered.len(), world.system.nodes().len());
    assert_eq!(
        world.ship.frame,
        Frame::Local(worldgen::Node::Station(station))
    );
}

/// The spawn is the **lowest** star with an orbital nobody shoots from and
/// a belt in the system, and the lowest such orbital in it, so two clients
/// that generated the same galaxy start in the same place — and not
/// beside a wreck, not under fire, and with somewhere to mine.
#[test]
fn the_spawn_is_the_first_lived_in_dock_in_the_galaxy() {
    use crate::station::residents_of;
    let galaxy = worldgen::Galaxy::new(data::DEFAULT_SEED, GalaxyType::SpiralTwoArm);
    let (star, station) = crate::spawn(&galaxy).expect("a galaxy has a station somewhere");
    let a_start =
        |s: &worldgen::system::StationBlueprint| s.kind == StationKind::Orbital && !s.hostile;
    let a_belt = |system: &worldgen::StarSystem| {
        system
            .bodies
            .iter()
            .any(|b| b.kind == worldgen::BodyKind::AsteroidBelt)
    };
    for earlier in 0..star {
        let system = galaxy.system(earlier).unwrap();
        assert!(
            !(a_belt(&system) && system.stations.iter().any(a_start)),
            "star {earlier} had an orbital nobody shoots from and a belt"
        );
    }
    let system = galaxy.system(star).unwrap();
    assert!(a_belt(&system), "the spawn system has no belt to mine");
    assert!(residents_of(system.station(station).unwrap().kind) > 0);
    assert_eq!(
        system
            .stations
            .iter()
            .filter(|s| a_start(s))
            .map(|s| s.id)
            .min(),
        Some(station)
    );
    assert!(a_start(system.station(station).unwrap()));

    let world = basic();
    assert_eq!(world.star_id, star);
}

/// The reference galaxy has enemies in it, and neither spawn ever puts a
/// crew at one of theirs: `spawn` never, and `spawn_anywhere` for no roll
/// — every dock it can answer with is somebody's and nobody's enemy.
#[test]
fn the_galaxy_has_hostile_stations_and_the_spawn_is_never_one() {
    use crate::station::residents_of;
    let galaxy = worldgen::Galaxy::new(data::DEFAULT_SEED, GalaxyType::SpiralTwoArm);
    let systems = galaxy.every_system();
    let hostile = systems
        .iter()
        .flat_map(|s| s.stations.iter())
        .filter(|s| s.hostile)
        .count();
    assert!(hostile > 0, "the reference galaxy has enemies");
    let blueprint = |star: u32, station: u32| {
        systems
            .iter()
            .find(|s| s.star_id == star)
            .and_then(|s| s.station(station))
            .cloned()
            .expect("the spawn is a real station")
    };
    let (star, station) = crate::spawn(&galaxy).unwrap();
    let b = blueprint(star, station);
    assert!(!b.hostile && residents_of(b.kind) > 0);
    // Every roll lands somewhere different or the same, never at an
    // enemy's: enough rolls to go round every dock at least once.
    let docks = systems
        .iter()
        .flat_map(|s| s.stations.iter())
        .filter(|s| residents_of(s.kind) > 0 && !s.hostile)
        .count() as u64;
    for pick in 0..docks + 3 {
        let (star, station) = crate::spawn_anywhere(&galaxy, pick).unwrap();
        let b = blueprint(star, station);
        assert!(!b.hostile, "roll {pick} landed at an enemy's");
        assert!(residents_of(b.kind) > 0);
    }
}

// --- one clock ---------------------------------------------------------------

/// A step is a step whoever calls it. Grouping them the way a frame does
/// changes nothing at all, which is the whole promise the fixed step is for.
#[test]
fn a_step_is_always_the_same_length_however_the_steps_are_grouped() {
    // --- how_the_steps_are_grouped_into_frames_makes_no_difference ---
    {
        let order_at = 37u32;
        let total = 400u32;

        let run = |group: u32| {
            let mut world = basic();
            let mut taken = 0;
            while taken < total {
                let batch = group.min(total - taken);
                for _ in 0..batch {
                    // A walk to the station's desk partway: an order that
                    // moves somebody, in the middle of a frame or not.
                    let commands: Vec<Command> = if taken == order_at {
                        let at = world.aboard.room.desk_spot(0).expect("a desk");
                        vec![Command::Crew {
                            slot: 0,
                            order: bims::order::CrewOrder::SendTo {
                                who: 0,
                                x: at.x,
                                y: at.y,
                            },
                        }]
                    } else {
                        Vec::new()
                    };
                    world.step(&commands);
                    taken += 1;
                }
            }
            world.checksum()
        };

        let one_at_a_time = run(1);
        for group in [2u32, 7, 24, 60, 400] {
            assert_eq!(
                run(group),
                one_at_a_time,
                "running {group} steps to a frame gave a different world",
            );
        }
    }

    // --- a_step_is_always_the_same_length ---
    {
        let mut world = basic();
        for i in 1..=10u32 {
            world.step(&[]);
            assert!(close(
                world.mission_minutes(),
                data::STEP_MINUTES * i as f64
            ));
            assert_eq!(world.steps, i as u64);
            // The world clock is travel's alone (feature 103).
            assert_eq!(world.clock_minutes, 0.0);
        }
        // And the day shown is the crew's: the room counts from day 1 and
        // opens at the waking hour, and stands there through a mission,
        // since only a trip moves the day.
        assert_eq!(world.day(), 1);
        let expected = 8.0 * time::HOUR;
        assert!(
            (world.minutes_into_day() - expected).abs() < 1e-3,
            "{} vs {expected}",
            world.minutes_into_day()
        );
    }
}

// --- what a ship change does, and does not do -------------------------------

/// A part built adds its own mass and **does not move the hull a
/// millimetre**: the anchor is stored and the position derived, so
/// bolting a wall to the stern drags the centre of mass aft and leaves
/// every tile of the ship exactly where it was.
#[test]
fn building_a_part_moves_the_centre_of_mass_and_not_the_hull() {
    // The flyer has no shelf, so there is nowhere to keep the materials. One
    // shelf and a hundred units of metal is what a construction step would
    // actually be working from.
    let budget = Budget::new(10_000_000);
    let mut design = flyer(2);
    design = apply(
        &design,
        &budget,
        Edit::Place {
            kind: PartKind::Shelf,
            origin: (7, 9),
            rotation: Rotation::R0,
        },
    )
    .expect("a shelf on bare deck");
    design = apply(
        &design,
        &budget,
        Edit::Buy {
            resource: ResourceId::Vegetable,
            units: 60,
        },
    )
    .expect("metal onto the shelf");

    let mut world = world_with(design, 10_000, 2);
    let before_mass = world.ship.dynamics.mass.get();
    let before_anchor = world.ship.anchor;
    let before_centre = world.ship.dynamics.centre_of_mass;
    let hull_before: Vec<DVec2> = hull_positions(&world);

    // A wall at the far end of the ship, which is where the centre of mass
    // will be dragged.
    world.ship.design = apply(
        &world.ship.design,
        &Budget::new(economy::Money::MAX),
        Edit::Place {
            kind: PartKind::Wall,
            origin: (16, 16),
            rotation: Rotation::R0,
        },
    )
    .expect("a wall at the stern");
    world.on_ship_changed();

    // A part is **bought** since the money rework (feature 95), so the
    // ship is heavier by exactly the wall — the hull and the hold were
    // one stock of materials before, and are not now.
    assert!(
        close(
            world.ship.dynamics.mass.get(),
            before_mass + shipdesign::part_mass(PartKind::Wall)
        ),
        "mass went from {before_mass} to {}",
        world.ship.dynamics.mass.get(),
    );
    assert_eq!(world.ship.anchor, before_anchor, "the anchor moved");
    assert_eq!(hull_positions(&world), hull_before, "the hull moved");
    assert_ne!(
        world.ship.dynamics.centre_of_mass, before_centre,
        "a wall at one end should have pulled the weight towards it",
    );
    // And it moved *towards the wall*, which is down and to the right of
    // where it was in design coordinates.
    assert!(world.ship.dynamics.centre_of_mass.x > before_centre.x);
    assert!(world.ship.dynamics.centre_of_mass.y > before_centre.y);
}

/// Every hull tile, in system coordinates. What must not move when the ship
/// changes shape.
fn hull_positions(world: &World) -> Vec<DVec2> {
    use flight::angle;
    use shipdesign::parts::TILE;
    let mut out = Vec::new();
    for part in &world.ship.design.parts {
        if part.kind != PartKind::OutsideWall {
            continue;
        }
        for (x, y) in part.tiles() {
            let design = dvec2(
                (x as f64 + 0.5) * TILE as f64,
                (y as f64 + 0.5) * TILE as f64,
            );
            out.push(
                world
                    .ship
                    .anchor
                    .add(angle::rotate_design(design, world.ship.heading)),
            );
        }
    }
    out
}

// --- power under way ------------------------------------------------------------

// --- the jump -------------------------------------------------------------------

/// A star a jump from here can actually reach: the lowest-numbered lane
/// out of the star the ship is at. Since feature 93 a charge is **one hop
/// and only down a lane**, so every test that jumps picks its target
/// through this rather than by counting ids.
pub(crate) fn laned_star(world: &World) -> u32 {
    world
        .galaxy()
        .lanes(world.star_id)
        .first()
        .copied()
        .expect("the lane graph is one connected web")
}

/// A jump is another system altogether — what a trip across a hyperlane
/// does on the way (`World::travel`): the ship holding in empty space
/// round another star, with the old system's stations, people and chart
/// left behind and everything of the ship's own brought along. A star the
/// galaxy has not got is no jump at all.
#[test]
fn a_jump_puts_the_ship_in_another_system() {
    let mut world = basic();
    let from = world.star_id;
    let to = laned_star(&world);
    let money = world.money;
    let cargo = world.ship.design.cargo;
    let heading = world.ship.heading;

    let mut events = Vec::new();
    assert!(!world.jump(1_000_000, &mut events), "no such star");
    assert!(events.is_empty());
    assert_eq!(world.star_id, from);

    assert!(world.jump(to, &mut events));
    assert!(
        events
            .iter()
            .any(|e| matches!(e, WorldEvent::Jumped { star } if *star == to)),
        "{events:?}"
    );
    assert_eq!(world.star_id, to);
    assert_eq!(world.ship.state, ShipState::Holding);
    assert!(world.residents.is_none());
    assert_eq!(world.ship.frame, Frame::Space);
    // In empty space: clear of everything the new system holds.
    let here = world.ship.position();
    for node in world.system.nodes() {
        let there = world.system.absolute_position(node).unwrap();
        assert!(there.distance(here) >= data::JUMP_CLEARANCE, "{node:?}");
    }
    // The new system's stations, nobody on them an enemy — every human is
    // friendly (feature 104) — and nobody's home, and only what the
    // sensors reach on the chart.
    assert_eq!(world.stations.len(), Station::all_of(&world.system).len());
    // A station the machines already hold is theirs (at 240 stars the
    // crisis can be next door from the start).
    for s in &world.stations {
        let stance = if world.is_droid_held(s.id) {
            bims::sight::Stance::Hostile
        } else {
            bims::sight::Stance::Neutral
        };
        assert_eq!(world.stance(s.id), stance);
    }
    assert!(world.discovered.len() <= world.system.nodes().len());
    // What is the ship's came along.
    assert_eq!(world.money, money);
    assert_eq!(world.ship.design.cargo, cargo);
    assert_eq!(world.ship.crew_count, 2);
    assert_eq!(world.aboard.crew_count(), 2);
    assert!((world.ship.heading - heading).abs() < 1e-9);

    // And a trip on from there is an ordinary trip: every site of the new
    // system has a quote.
    let sites = world.sites_at(to);
    assert!(!sites.is_empty(), "the system has somewhere to go");
    for site in sites {
        assert!(world.travel_quote(site).is_ok(), "{site:?}");
    }
}

/// The station's doors are in two rooms while the ship is docked, and a
/// lock is the same door's in both: one the crew set on the joined deck
/// stops the station's people in their own room, and one lifted there is
/// lifted on the deck.
/// A lamp shot out on the joined deck is out in the residents' room too
/// — the same lamp, found by its tile of the station's design through
/// the frame — and remembered by the world, so it is out again on every
/// room built afresh: the ship's own after an undock, the station's
/// after a dock. The checksum sees it.
#[test]
fn a_lock_on_a_station_door_is_the_same_lock_in_both_rooms_and_a_lamp_shot_out_is_out_in_both() {
    // --- a_lock_on_a_station_door_is_the_same_lock_in_both_rooms ---
    {
        use bims::door::{Locker, Order};
        let mut world = basic();
        assert!(world.aboard.is_joined());
        let residents = world.residents.as_ref().expect("people at the spawn");
        // A station door: one of the residents' room's, found on the deck by
        // its middle through the station frame.
        let theirs = residents.aboard.room.door_states();
        assert!(!theirs.is_empty(), "the station has doors");
        let (j, on_deck) = theirs
            .iter()
            .enumerate()
            .find_map(|(j, s)| {
                let design =
                    dvec2(s.centre.x as f64, s.centre.y as f64).sub(residents.aboard.offset);
                let at = world.aboard.from_station(design)?;
                let i = world
                    .aboard
                    .room
                    .door_index_at(bims::math::vec2(at.x as f32, at.y as f32))?;
                Some((j, i))
            })
            .expect("a station door on the joined deck");
        assert!(!world.aboard.room.door_states()[on_deck].locked);

        // Locked from the deck: their door is locked, by the crew.
        world.aboard.room.order_door_now(on_deck, Order::Lock);
        world.step(&[]);
        let theirs = world.residents.as_ref().unwrap().aboard.room.door_states();
        assert!(theirs[j].locked, "the lock reached the station's room");
        assert!(theirs[j].by_crew);
        assert!(!theirs[j].changed);

        // Lifted in theirs — a smash, or one of them at the panel — it lifts
        // on the deck.
        world
            .residents
            .as_mut()
            .unwrap()
            .aboard
            .room
            .set_door_locked_for_probe(j, None);
        world.step(&[]);
        assert!(!world.aboard.room.door_states()[on_deck].locked);
        assert!(!world.aboard.room.ship_door_is_locked(on_deck));

        // And a lock of theirs — a body sealing itself in — is a lock on the
        // deck that is nobody's on this side, which the panel can still lift.
        world
            .residents
            .as_mut()
            .unwrap()
            .aboard
            .room
            .set_door_locked_for_probe(j, Some(Locker::Body(0)));
        world.step(&[]);
        assert!(world.aboard.room.ship_door_is_locked(on_deck));
        assert_ne!(
            world.aboard.room.door_locked_by(on_deck),
            Some(Locker::Crew)
        );
        world.aboard.room.order_door_now(on_deck, Order::Unlock);
        world.step(&[]);
        assert!(
            !world
                .residents
                .as_ref()
                .unwrap()
                .aboard
                .room
                .ship_door_is_locked(j)
        );
    }

    // --- a_lamp_shot_out_is_out_in_both_rooms_and_stays_out_across_a_dock ---
    {
        use bims::combat::WeaponKind;
        use bims::math::vec2;
        let mut world = basic();
        let station_id = world.residents.as_ref().unwrap().station;
        let before = world_checksum(&world);
        let t = shipdesign::TILE as f32;

        // Shoot the nearest lamp James sees from where he stands, until it
        // is out, and say which design it hangs in and where.
        let shoot_out = |world: &mut World, foreign: bool| -> (u32, u32) {
            let james = world.aboard.room.bim_pos(0);
            let lamps: Vec<_> = world.aboard.room.lamps().to_vec();
            let (i, lamp) = lamps
                .iter()
                .enumerate()
                .filter(|(_, l)| {
                    world
                        .aboard
                        .design_of(dvec2(l.at.x as f64, l.at.y as f64))
                        .0
                        == foreign
                        && (l.at - james).len() < 6.0 * t
                        && world.aboard.room.sees_for_probe(0, l.at)
                })
                .min_by(|a, b| (a.1.at - james).len().total_cmp(&(b.1.at - james).len()))
                .expect("a lamp in sight");
            let (which, tile) = {
                let (f, p) = world
                    .aboard
                    .design_of(dvec2(lamp.at.x as f64, lamp.at.y as f64));
                assert_eq!(f, foreign);
                let t = shipdesign::TILE as f64;
                (
                    if f { Some(station_id) } else { None },
                    ((p.x / t).floor() as u32, (p.y / t).floor() as u32),
                )
            };
            for _ in 0..60 {
                if world.aboard.room.lamps()[i].is_out() {
                    break;
                }
                world.aboard.room.enemy_fire(
                    james,
                    lamp.at,
                    WeaponKind::LaserPistol.basic(),
                    false,
                );
                for _ in 0..30 {
                    world.step(&[]);
                    // Kept where he stands: an errand would walk him off.
                    world.aboard.room.put_for_probe(0, james);
                }
            }
            assert!(world.aboard.room.lamps()[i].is_out(), "shot out");
            assert!(
                world
                    .lamps
                    .iter()
                    .any(|d| d.station == which && d.tile == tile && d.health == 0.0),
                "remembered as {which:?} {tile:?}: {:?}",
                world.lamps
            );
            tile
        };
        // The residents' lamp at a tile of the station's design.
        let residents_lamp = |world: &World, tile: (u32, u32)| -> Option<bims::sight::Lamp> {
            let r = world.residents.as_ref().unwrap();
            let t = shipdesign::TILE as f64;
            let p = r
                .aboard
                .room_of(
                    false,
                    dvec2((tile.0 as f64 + 0.5) * t, (tile.1 as f64 + 0.5) * t),
                )
                .unwrap();
            r.aboard
                .room
                .lamp_at(vec2(p.x as f32, p.y as f32))
                .map(|(_, l)| *l)
        };

        // James inside the station's door: a station lamp.
        let ashore = world.aboard.ashore.unwrap();
        world
            .aboard
            .room
            .put_for_probe(0, vec2(ashore.x as f32, ashore.y as f32));
        world.step(&[]);
        let station_tile = shoot_out(&mut world, true);
        let theirs = residents_lamp(&world, station_tile).expect("the same lamp in their room");
        assert!(theirs.is_out(), "out in the residents' room too");
        assert_ne!(world_checksum(&world), before, "the checksum sees it");

        // And back on the ship: one of its own.
        let gangway = world.aboard.gangway.unwrap();
        world
            .aboard
            .room
            .put_for_probe(0, vec2(gangway.x as f32, gangway.y as f32));
        world.step(&[]);
        let ship_tile = shoot_out(&mut world, false);
        assert_eq!(world.lamps.len(), 2);

        // Undocked, the ship's room is built afresh — and its lamp is out.
        world.undock_for_probe();
        let t64 = shipdesign::TILE as f64;
        let p = world
            .aboard
            .room_of(
                false,
                dvec2(
                    (ship_tile.0 as f64 + 0.5) * t64,
                    (ship_tile.1 as f64 + 0.5) * t64,
                ),
            )
            .unwrap();
        let (_, lamp) = world
            .aboard
            .room
            .lamp_at(vec2(p.x as f32, p.y as f32))
            .expect("the ship's lamp");
        assert!(lamp.is_out(), "out on the fresh room");
        // The station's own room, the ship gone, keeps its lamp out.
        assert!(residents_lamp(&world, station_tile).unwrap().is_out());
        // Every other lamp is whole.
        assert_eq!(
            world
                .aboard
                .room
                .lamps()
                .iter()
                .filter(|l| l.is_out())
                .count(),
            1
        );

        // Docked again, both rooms are built afresh once more, and both
        // lamps are out on the joined deck.
        world.dock_for_probe(station_id);
        world.step(&[]);
        let out: Vec<(bool, (u32, u32))> = world
            .aboard
            .room
            .lamps()
            .iter()
            .filter(|l| l.is_out())
            .map(|l| {
                let (f, p) = world.aboard.design_of(dvec2(l.at.x as f64, l.at.y as f64));
                (f, ((p.x / t64).floor() as u32, (p.y / t64).floor() as u32))
            })
            .collect();
        assert_eq!(out.len(), 2, "{out:?}");
        assert!(out.contains(&(true, station_tile)) && out.contains(&(false, ship_tile)));
        assert!(residents_lamp(&world, station_tile).unwrap().is_out());
    }
}

// --- redirecting ----------------------------------------------------------------

// --- trading ---------------------------------------------------------------------

/// A derelict keeps no desk: nothing to buy, since it stocks nothing, and
/// nobody to sell to either. Every lived-on kind quotes; a settlement
/// quotes as a settlement whatever kind it is laid out as; and the same
/// seed twice is the same quote.
#[test]
fn a_derelict_has_no_market_and_every_other_station_quotes() {
    use crate::station::{Plan, market_kind};
    use economy::market::MarketKind;
    let world = world_with(flyer(2), 100_000, 2);
    for station in &world.stations {
        let desk = station.market();
        if station.kind == StationKind::Derelict {
            assert!(desk.is_none(), "a derelict with a desk");
        } else {
            let desk = desk.expect("a lived-on station keeps a desk");
            for &resource in ResourceId::ALL.iter() {
                let q = desk.quote(resource);
                assert!(q.bid < q.ask, "{:?} {resource:?}", station.kind);
            }
        }
    }
    assert_eq!(
        market_kind(crate::surface::SURFACE_KIND, Plan::Surface),
        Some(MarketKind::Settlement)
    );
    assert_eq!(market_kind(StationKind::Derelict, Plan::Hub), None);
    assert_eq!(
        market_kind(StationKind::Relay, Plan::Pod),
        Some(MarketKind::Relay)
    );
    let again = world_with(flyer(2), 100_000, 2);
    for (a, b) in world.stations.iter().zip(&again.stations) {
        assert_eq!(a.bias, b.bias);
        assert_eq!(a.market(), b.market());
    }
    // Only the spawn has its lean forced off: somewhere else in the
    // galaxy the roll reaches the desk.
    let leaning = world
        .stations
        .iter()
        .chain(again.stations.iter())
        .any(|s| s.bias != economy::market::Bias::NONE);
    assert!(leaning, "no station in the spawn system leans on anything");
}

// --- looking out of the window ----------------------------------------------------

/// Something that comes within range of the stretch just travelled is seen;
/// something that never does is not. The **stretch**, not the endpoints: at
/// the top speed a step is a long way, and a station passed in the middle of
/// one would otherwise be missed entirely.
#[test]
fn what_comes_within_range_of_the_track_is_seen_and_a_sensor_array_makes_the_system_visible() {
    // --- what_comes_within_range_of_the_track_is_seen ---
    {
        let world = basic();
        let range = world.detection_range();
        assert!(range > 0.0);

        let here = world.ship.position();
        let out = here.add(dvec2(range * 20.0, 0.0));

        // A point half way along the track and just inside the range, and another
        // beside the same track and well outside it.
        let seen_from = here.add(dvec2(range * 10.0, range * 0.5));
        let unseen_from = here.add(dvec2(range * 10.0, range * 3.0));
        assert!(crate::world::segment_distance_for_probe(here, out, seen_from) <= range);
        assert!(crate::world::segment_distance_for_probe(here, out, unseen_from) > range);

        // Neither endpoint is anywhere near the near one, which is the half that
        // an endpoint-only test would get wrong.
        assert!(here.distance(seen_from) > range);
        assert!(out.distance(seen_from) > range);
    }

    // --- sensor_arrays_are_what_make_a_system_visible ---
    {
        let world = basic();
        let with = world.detection_range();

        let mut blind = flyer(2);
        blind.parts.retain(|p| p.kind != PartKind::SensorArray);
        let world = world_with(blind, REFERENCE_MONEY, 2);
        assert_eq!(world.detection_range(), data::VISION_RANGE);
        assert!(with > world.detection_range());
    }

    // --- flying_past_something_is_what_discovers_it ---
    {
        let mut world = basic();
        world.uncharted_for_probe();
        let unseen: Vec<worldgen::Node> = world
            .system
            .nodes()
            .into_iter()
            .filter(|n| !world.discovered.contains(n))
            .collect();
        let Some(&node) = unseen.first() else {
            return; // everything was in range from the dock
        };
        let at = world.system.absolute_position(node).unwrap();

        // Standing still beside it is enough; the track is a point.
        let events = world.discover_for_probe(at, at);
        assert!(
            events
                .iter()
                .any(|e| matches!(e, WorldEvent::Discovered { node: n } if *n == node)),
            "{events:?}",
        );
        assert!(world.discovered.contains(&node));

        // And it is never forgotten, and never listed twice.
        let again = world.discover_for_probe(at, at);
        assert!(again.is_empty());
        assert_eq!(world.discovered.iter().filter(|&&n| n == node).count(), 1,);
    }
}

// --- the local frame -----------------------------------------------------------

/// In at the radius, out at a quarter again, and nothing at all in between.
/// A ship sitting exactly on the line must not strobe.
/// The frame is about the view. It must not reach the clock or the speed —
/// which is easy to say and easy to break, so it is checked.
#[test]
fn the_local_frame_has_a_hysteresis_and_changing_it_touches_neither_the_clock_nor_the_speed() {
    // --- the_local_frame_has_a_hysteresis_and_uses_it ---
    {
        let mut world = basic();
        let ShipState::Docked { station } = world.ship.state else {
            unreachable!()
        };
        let node = worldgen::Node::Station(station);
        let at = world.system.absolute_position(node).unwrap();
        let radius = data::local_radius(node);
        // Straight away from whatever else is nearest, so that drifting out from
        // the station does not drift *into* its parent body's frame instead —
        // which is a fact about where the generator put the planet, not about
        // the hysteresis.
        let away = world
            .system
            .nodes()
            .into_iter()
            .filter(|&other| other != node)
            .filter_map(|other| world.system.absolute_position(other))
            .min_by(|a, b| {
                a.distance(at)
                    .partial_cmp(&b.distance(at))
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .map(|other| at.sub(other))
            .map(|d| d.scale(1.0 / d.length().max(1e-9)))
            .unwrap_or(dvec2(1.0, 0.0));
        let out_at = |by: f64| at.add(away.scale(by));

        // Undock, so the ship is holding rather than tied on.
        world.ship.state = ShipState::Holding;
        world.put_for_probe(at);
        world.settle_frame_for_probe();
        assert_eq!(world.ship.frame, Frame::Local(node));

        // Drifting between the two radii changes nothing, however often.
        for i in 0..10 {
            let out = radius * (1.0 + 0.2 * (i % 2) as f64);
            world.put_for_probe(out_at(out));
            assert!(
                world.settle_frame_for_probe().is_empty(),
                "the frame flipped at {out}",
            );
            assert_eq!(world.ship.frame, Frame::Local(node));
        }

        // Past the outer radius, once.
        world.put_for_probe(out_at(radius * data::LOCAL_HYSTERESIS * 1.01));
        let events = world.settle_frame_for_probe();
        assert_eq!(events.len(), 1, "{events:?}");
        assert_eq!(world.ship.frame, Frame::Space);

        // And back in takes the inner one: the outer radius is not enough.
        world.put_for_probe(out_at(radius * 1.1));
        assert!(world.settle_frame_for_probe().is_empty());
        assert_eq!(world.ship.frame, Frame::Space);
        world.put_for_probe(out_at(radius * 0.9));
        let events = world.settle_frame_for_probe();
        assert_eq!(events.len(), 1);
        assert_eq!(world.ship.frame, Frame::Local(node));
    }

    // --- the_frame_changing_does_not_touch_the_clock_or_the_speed ---
    {
        let mut world = basic();
        world.step(&[Command::SetSpeed {
            slot: 0,
            speed: Speed::Real,
        }]);
        let speed = world.effective_speed();
        world.undock_for_probe();
        let berth = world.ship.position();
        let far = berth.add(dvec2(10.0 * data::LOCAL_RADIUS_BODY, 0.0));
        let clock = world.clock_minutes;

        // Out of the dock's frame and back into it, a few times over: the
        // step settles the frame, and the frame changing must not reach the
        // clocks or the speed.
        let mut changes = 0;
        for i in 0..400u32 {
            world.put_for_probe(if (i / 100) % 2 == 0 { berth } else { far });
            let steps = world.mission_steps();
            let events = world.step(&[]);
            assert_eq!(world.mission_steps(), steps + 1, "a step was not a step");
            assert_eq!(world.clock_minutes, clock, "the world clock moved");
            assert_eq!(world.effective_speed(), speed);
            changes += events
                .iter()
                .filter(|e| matches!(e, WorldEvent::FrameChanged { .. }))
                .count();
        }
        assert!(changes > 1, "the frame should have changed: {changes}");
    }
}

// --- speed ---------------------------------------------------------------------

/// The world opens at 1× — the only speed there is (task 119) — and runs
/// at it until somebody pauses.
#[test]
fn the_world_runs_at_one_times_or_not_at_all() {
    let mut world = basic();
    assert_eq!(world.effective_speed(), Speed::Real);
    assert_eq!(world.effective_speed().multiplier(), 1);
    for request in &world.speed_requests {
        assert!(Speed::ALL.contains(request));
    }
    world.step(&[Command::SetSpeed {
        slot: 0,
        speed: Speed::Paused,
    }]);
    assert_eq!(world.effective_speed().multiplier(), 0);
    world.step(&[Command::SetSpeed {
        slot: 0,
        speed: Speed::Real,
    }]);
    assert_eq!(world.effective_speed(), Speed::Real);
}

/// **Any player's pause pauses everyone** (task 119): the world is paused
/// while any one of them asks for it, whoever it is, and runs again only
/// when every pause is lifted.
#[test]
fn any_player_s_pause_pauses_everyone() {
    let mut world = basic();
    let players = world.players();
    assert!(players >= 2, "a world of company: {players}");
    for slot in 0..players {
        world.step(&[Command::SetSpeed {
            slot,
            speed: Speed::Paused,
        }]);
        assert_eq!(world.effective_speed(), Speed::Paused, "slot {slot}");
        world.step(&[Command::SetSpeed {
            slot,
            speed: Speed::Real,
        }]);
        assert_eq!(world.effective_speed(), Speed::Real, "slot {slot}");
    }
    // Two pausing: one lifting it is not enough.
    world.step(&[
        Command::SetSpeed {
            slot: 0,
            speed: Speed::Paused,
        },
        Command::SetSpeed {
            slot: 1,
            speed: Speed::Paused,
        },
    ]);
    world.step(&[Command::SetSpeed {
        slot: 0,
        speed: Speed::Real,
    }]);
    assert_eq!(world.effective_speed(), Speed::Paused);

    // A slot nobody is in cannot change it.
    world.step(&[Command::SetSpeed {
        slot: 9,
        speed: Speed::Real,
    }]);
    assert_eq!(world.effective_speed(), Speed::Paused);
    world.step(&[Command::SetSpeed {
        slot: 1,
        speed: Speed::Real,
    }]);
    assert_eq!(world.effective_speed(), Speed::Real);
}

// --- permissions ------------------------------------------------------------------

/// Walk one of the crew off the ship — out through the airlocks to the
/// corridor just inside the station's door — so there is somebody to
/// call back aboard. Who it was: the second crew member, since the first
/// is about to take the helm.
fn send_a_crew_member_ashore(world: &mut World) -> u32 {
    let who = 1;
    assert!(who < world.aboard.crew_count(), "nobody to send");
    let to = world.aboard.ashore.expect("the station has a door");
    assert!(
        world
            .aboard
            .room
            .send_for_probe(who as usize, bims::math::vec2(to.x as f32, to.y as f32)),
        "no route ashore"
    );
    for _ in 0..LEAVING {
        world.step(&[]);
        if !world.aboard.on_ship(who, &world.ship.design) {
            return who;
        }
    }
    panic!("the crew member never went ashore");
}

// --- the lockers' grid ---------------------------------------------------------

// --- the cross-target number -------------------------------------------------------

/// The scenario `ship_self_check` runs in wasm, run here in native. Both
/// compare against the same written-down constant; a target whose arithmetic
/// drifted fails exactly one of the two.
#[test]
fn the_reference_run_comes_out_at_the_number_it_is_pinned_to() {
    assert_eq!(
        crate::fixture::reference_run(),
        crate::fixture::REFERENCE_CHECKSUM,
    );
    // And it is the same every time it is asked for, which is the other half
    // of what a checksum is worth.
    assert_eq!(
        crate::fixture::reference_run(),
        crate::fixture::reference_run()
    );
}

/// A checksum that did not notice anything would pass every test above.
/// Whose side a station is on is in the checksum: two worlds that
/// disagree about it are two different fights.
/// A worn piece is in the checksum, where it is and what it has left:
/// two worlds that disagree about who is wearing what are two different
/// fights.
/// The checksum sees what the crew know and which desks still have their
/// key: two worlds alike but for a node begun, or a key taken, differ.
/// Two worlds alike differ the moment one has a tier-two gun in the
/// armory the other has not, a key picked up or a bandage spent: the
/// holdings and the loadouts are in it (task 113).
#[test]
fn the_checksum_notices_every_kind_of_change() {
    // --- the_checksum_notices_a_world_that_has_moved ---
    {
        let mut world = basic();
        let was = world.checksum();
        world.step(&[]);
        assert_ne!(world.checksum(), was, "a step should show");

        let mut spent = basic();
        spent.wallets[0] -= 1;
        assert_ne!(spent.checksum(), basic().checksum(), "a euro should show");

        let mut travelled = basic();
        travelled.leave_for_probe();
        let mut stayed = basic();
        stayed.leave_for_probe();
        assert_eq!(travelled.checksum(), stayed.checksum());
        let site = travelled
            .travel_quotes()
            .into_iter()
            .find(|(site, quote)| quote.is_ok() && Some(*site) != travelled.current_site())
            .map(|(site, _)| site)
            .expect("somewhere to go");
        travelled.step(&[
            Command::Propose {
                slot: 0,
                star: site.star,
                station: site.station,
            },
            Command::Accept { slot: 1, yes: true },
        ]);
        stayed.step(&[]);
        assert_ne!(
            travelled.checksum(),
            stayed.checksum(),
            "a trip should show"
        );
    }

    // --- the_checksum_notices_a_stance_change ---
    {
        use bims::sight::Stance;
        let mut world = basic();
        // Every human is friendly (feature 104): home is, and every other
        // station is a stranger's, whatever the generator rolled it.
        assert_eq!(world.stance(world.home), Stance::Friendly);
        for s in &world.stations {
            if s.id != world.home {
                assert_eq!(world.stance(s.id), Stance::Neutral, "station {}", s.id);
            }
        }
        // The one stance that changes is the machines taking a station.
        let was = world.checksum();
        let station_id = world.residents.as_ref().unwrap().station;
        world.infest(station_id);
        assert_eq!(world.stance(station_id), Stance::Hostile);
        assert_ne!(world.checksum(), was, "the machines should show");
    }

    // --- the_checksum_notices_a_worn_piece (task 113: between missions) ---
    {
        use shipdesign::fixture::playtest_ship;
        let mut world = simulation_world(playtest_ship(), data::SIMULATION_MONEY, 1);
        let mut twin = simulation_world(playtest_ship(), data::SIMULATION_MONEY, 1);
        world.leave_for_probe();
        twin.leave_for_probe();
        assert_eq!(world.checksum(), twin.checksum());
        let helm = armory_id(&world, ResourceId::Armour).expect("a helm in the armory");
        assert_eq!(armory_id(&twin, ResourceId::Armour), Some(helm));
        let equip = Command::Equip {
            slot: 0,
            who: 0,
            from: crate::GearSource::Armory { id: helm },
        };
        world.step(&[equip]);
        twin.step(&[]);
        assert_ne!(
            world.checksum(),
            twin.checksum(),
            "one wears it, one does not"
        );
        twin.step(&[equip]);
        world.step(&[]);
        assert_eq!(world.checksum(), twin.checksum(), "both do");
        // And a dent shows.
        world.aboard.room.wound(0, 5.0);
        assert_ne!(world.checksum(), twin.checksum(), "one helm is dented");
    }

    // --- the_checksum_notices_a_relic (feature 106) ---
    {
        use shipdesign::fixture::playtest_ship;
        let mut world = simulation_world(playtest_ship(), data::SIMULATION_MONEY, 1);
        let twin = simulation_world(playtest_ship(), data::SIMULATION_MONEY, 1);
        assert_eq!(world_checksum(&world), world_checksum(&twin));
        world.give_relic_for_probe(crate::relic::Relic::GlassCannon);
        assert_ne!(
            world_checksum(&world),
            world_checksum(&twin),
            "a relic held"
        );
    }

    // --- the_checksum_notices_the_holdings (task 113) ---
    {
        use bims::combat::{Item, Tier, WeaponKind};
        use shipdesign::fixture::playtest_ship;
        let mut world = simulation_world(playtest_ship(), data::SIMULATION_MONEY, 1);
        let mut twin = simulation_world(playtest_ship(), data::SIMULATION_MONEY, 1);
        assert_eq!(world.checksum(), twin.checksum());
        world
            .holdings
            .put(0, Item::Weapon(WeaponKind::LaserPistol.at(Tier::Two)));
        assert_ne!(world.checksum(), twin.checksum(), "a pistol in the armory");
        twin.holdings
            .put(0, Item::Weapon(WeaponKind::LaserPistol.at(Tier::Two)));
        assert_eq!(world.checksum(), twin.checksum());
        // The same count, a different tier.
        let mut other = simulation_world(playtest_ship(), data::SIMULATION_MONEY, 1);
        other
            .holdings
            .put(0, Item::Weapon(WeaponKind::LaserPistol.at(Tier::One)));
        assert_ne!(world.checksum(), other.checksum(), "one pistol is tier two");
        // A charge changed: a grenade held (task 120 took the bandage this
        // used to spend, and task 127 made a charge a counter).
        world.set_charges_held(0, crate::class::Charge::Grenade, 1);
        assert_ne!(world.checksum(), twin.checksum(), "a grenade given");
    }
}

/// The armory id of the first thing of `resource` it holds.
fn armory_id(world: &World, resource: ResourceId) -> Option<u32> {
    world
        .holdings
        .armory
        .iter()
        .find(|s| crate::armour::resource_of_item(s.item) == Some(resource))
        .map(|s| s.id)
}

// --- where a world starts ----------------------------------------------------

/// The lobby names the spawn, and a world starts exactly there — any star
/// with a station, any station in it — docked, with that dock discovered.
/// A spawn that is not there is an error and never a different dock.
#[test]
fn a_world_starts_at_the_station_it_was_told_to_and_a_spawn_that_does_not_exist_is_refused() {
    // --- a_world_starts_at_the_station_it_was_told_to ---
    {
        let galaxy = worldgen::Galaxy::new(data::DEFAULT_SEED, GalaxyType::SpiralTwoArm);
        // Not the simulation's spawn: the *last* dock in the galaxy, so a world
        // that quietly fell back to `spawn` would show up.
        let (star, station) = galaxy
            .stars
            .iter()
            .rev()
            .find_map(|s| {
                let system = galaxy.system(s.id)?;
                let last = system.stations.iter().map(|st| st.id).max()?;
                Some((s.id, last))
            })
            .expect("a galaxy has a station somewhere");
        assert_ne!((star, station), crate::spawn(&galaxy).unwrap());

        let world = World::start(
            flyer(2),
            REFERENCE_MONEY,
            2,
            data::DEFAULT_SEED,
            GalaxyType::SpiralTwoArm,
            star,
            station,
        )
        .expect("a real station is somewhere to start");
        assert_eq!(world.star_id, star);
        assert_eq!(world.ship.state, ShipState::Docked { station });
        assert!(world.discovered.contains(&worldgen::Node::Station(station)));
    }

    // --- a_spawn_that_does_not_exist_is_refused_rather_than_replaced ---
    {
        let galaxy = worldgen::Galaxy::new(data::DEFAULT_SEED, GalaxyType::SpiralTwoArm);
        // Every system has a station since worldgen's version 8, so the
        // station asked for is one past the first star's last.
        let empty = galaxy.stars[0].id;
        let beyond = galaxy.system(empty).unwrap().stations.len() as u32;
        let (star, _) = crate::spawn(&galaxy).unwrap();
        let start = |star, station| {
            World::start(
                flyer(2),
                REFERENCE_MONEY,
                2,
                data::DEFAULT_SEED,
                GalaxyType::SpiralTwoArm,
                star,
                station,
            )
        };
        assert_eq!(
            start(empty, beyond).err(),
            Some(crate::StartError::NoSuchStation)
        );
        assert_eq!(
            start(star, 999).err(),
            Some(crate::StartError::NoSuchStation)
        );
        assert_eq!(
            start(u32::MAX, 0).err(),
            Some(crate::StartError::NoSuchStation)
        );
    }
}

/// The simulation's ship, at the simulation's dock, with the simulation's
/// purse: a trip to every other site of the spawn system and of the
/// systems a lane away can be quoted on its engines, or the
/// playtest is a ship that cannot leave the dock.
#[test]
fn the_playtest_ship_can_travel_somewhere_from_the_simulation_spawn() {
    use shipdesign::fixture::playtest_ship;
    let world = simulation_world(playtest_ship(), data::SIMULATION_MONEY, 1);
    assert_eq!(world.crew_money(), data::SIMULATION_MONEY);
    assert_eq!(world.ship.crew_count, 1);
    assert!(matches!(world.ship.state, ShipState::Docked { .. }));
    let quotes = world.travel_quotes();
    let here = world.current_site();
    let mut near = 0;
    let mut hops = 0;
    for (site, quote) in quotes {
        if Some(site) == here {
            continue;
        }
        let Ok(quote) = quote else {
            continue;
        };
        // A day a lane, nothing within the system (the map rework, task
        // 139).
        let minutes = data::JUMP_MINUTES * u64::from(quote.hops);
        assert_eq!(quote.jump, quote.hops > 0, "{site:?}");
        assert_eq!(quote.minutes, minutes, "{site:?}");
        if quote.jump {
            hops += 1;
        } else {
            near += 1;
        }
    }
    assert!(near > 0, "nowhere in the spawn system to go");
    assert!(hops > 0, "nowhere a lane away to go");
}

// --- the crew ----------------------------------------------------------------

/// The crew are aboard from the first step, and they are the room's Bims:
/// one per player, each at their own bunk — Bim *i* at bunk *i* in id
/// order — standing on its use spot, which is deck.
#[test]
fn everybody_spawns_at_their_own_bunk() {
    let world = basic();
    // Two of the crew; the station's residents are in the same room, after.
    assert_eq!(world.aboard.crew_count(), 2);
    let mut bunks: Vec<_> = world
        .ship
        .design
        .parts
        .iter()
        .filter(|p| p.kind == PartKind::Bunk)
        .collect();
    bunks.sort_by_key(|p| p.id);
    let grid = world.ship.design.grid();
    let mut seen = Vec::new();
    for who in 0..world.aboard.crew_count() {
        let spot = bunks[who as usize].use_spots()[0];
        let at = world.aboard.position(who);
        let want = bims::aboard::tile_middle(spot.0, spot.1);
        // To a navigation cell: the world opens docked, and joining the
        // ship's room to the station's stands everybody on the nearest
        // cell a body fits in, which beside a bunk is a few units off the
        // spot itself.
        assert!(
            (at.x - want.x as f64).abs() <= 10.0 && (at.y - want.y as f64).abs() <= 10.0,
            "bim {who} at {at:?}, bunk {who} is used from {want:?}"
        );
        assert_ne!(
            grid.get(shipdesign::Layer::Floor, spot),
            0,
            "bim {who} is not on deck"
        );
        assert_eq!(
            grid.get(shipdesign::Layer::Object, spot),
            0,
            "bim {who} is inside something"
        );
        assert!(!seen.contains(&spot), "two Bims in one place");
        seen.push(spot);
    }
}

/// Stage 5 runs on the world's clock and nobody else's: the room's clock
/// aboard reads exactly the world's, step for step, and a Bim that was put
/// somewhere else shows in the checksum.
#[test]
fn the_crew_keep_the_world_s_clock() {
    let mut world = basic();
    let mut other = basic();
    // The room's day starts at eight in the morning, so its clock reads
    // ahead of the world's by a fixed offset; what has to agree is how far
    // each has moved.
    let dawn = world.aboard.minutes();
    for _ in 0..10 {
        world.step(&[]);
        other.step(&[]);
    }
    // To a thousandth of a minute: the room keeps its clock in `f32`, and
    // at eight in the morning an `f32` minute is good to about that.
    assert!(
        ((world.aboard.minutes() - dawn) - world.clock_minutes).abs() < 1e-3,
        "room {} vs world {}",
        world.aboard.minutes() - dawn,
        world.clock_minutes
    );
    assert_eq!(world.checksum(), other.checksum());
    let was = other.aboard.position(0);
    other
        .aboard
        .room
        .put_for_probe(0, bims::math::vec2(was.x as f32 + 60.0, was.y as f32));
    assert_ne!(
        world.checksum(),
        other.checksum(),
        "a Bim that moved should show"
    );
}

/// The room aboard is the room, and a player's own Bim left to itself
/// stays where it is (September 2026, the user's word: a Bim never walks
/// about at random, and only the player's input moves the player's Bim) —
/// standing on the design's deck, never in a wall or off the ship.
#[test]
fn the_crew_live_aboard() {
    use shipdesign::fixture::playtest_ship;
    let mut world = simulation_world(playtest_ship(), data::SIMULATION_MONEY, 1);
    assert_eq!(world.aboard.crew_count(), 1);
    let start = world.aboard.position(0);
    let mut farthest = 0.0f64;
    // An hour of steps with nobody at the keyboard.
    for _ in 0..(60 * 60) {
        world.step(&[]);
        let at = world.aboard.position(0);
        farthest = farthest.max(at.distance(start));
        assert!(
            world.aboard.on_deck(0),
            "the Bim is off the deck at {at:?} after {} minutes",
            world.clock_minutes
        );
    }
    assert!(
        farthest < 1.0,
        "the Bim walked off on its own; farthest {farthest}"
    );
}

// --- stations ---------------------------------------------------------------

/// Every kind of station on every plan, at a handful of seeds, is a place the room can
/// live in: it has a door that opens onto space, the designer's rules find
/// nothing wrong with it for the people who live there, its hull keeps the
/// radiation out unless it is a derelict, and the same seed builds the same
/// station — which is what a native server and a browser agreeing depends
/// on.
#[test]
fn a_station_is_a_place_the_room_can_live_in() {
    use crate::station::{Plan, layout, resolve};
    use shipdesign::{design_hash, exposure, has_errors, validate};
    for plan in Plan::ALL {
        for kind in worldgen::StationKind::ALL {
            for seed in [1u64, 7, 0x_5749_4e44_4f57_0001, u64::MAX] {
                let design = layout(kind, plan, seed);
                // A generated station is as big as it rolled, within its
                // kind's range; one the generator fell back from is its
                // drawn plan's size.
                let (built, _) = resolve(kind, plan, seed);
                if built == Plan::Generated {
                    let (least, most) = crate::stationgen::side_range(kind);
                    assert!(
                        (least..=most).contains(&design.build_area),
                        "{kind:?} {seed}"
                    );
                } else {
                    assert_eq!(design.build_area, built.side(kind));
                }
                assert_eq!(
                    design_hash(&design),
                    design_hash(&layout(kind, plan, seed)),
                    "{plan:?} {kind:?}"
                );
                let port = shipdesign::port(&design)
                    .unwrap_or_else(|| panic!("{plan:?} {kind:?} has no port"));
                assert_eq!(
                    port.outward,
                    (-1, 0),
                    "{plan:?} {kind:?}: the port is in the west skin"
                );
                let issues = validate(&design, plan.residents(kind));
                assert!(
                    !has_errors(&issues),
                    "{plan:?} {kind:?} at seed {seed}: {:?}",
                    issues.iter().map(|i| i.code).collect::<Vec<_>>()
                );
                assert!(
                    exposure(&design).is_empty(),
                    "{plan:?} {kind:?} lets the radiation in"
                );
                // The one desk the key sits on, and — on the five newer
                // plans; a hub outpost's two bunks are its two residents'
                // — two beds to spare beyond the
                // residents.
                assert_eq!(design.count(PartKind::ResearchDesk), 1, "{plan:?} {kind:?}");
                assert_eq!(design.count(PartKind::TradingDesk), 1, "{plan:?} {kind:?}");
                let spare = if plan == Plan::Hub { 0 } else { 2 };
                assert!(
                    design.count(PartKind::Bunk) >= plan.residents(kind) + spare,
                    "{plan:?} {kind:?}: {} bunks for {} residents",
                    design.count(PartKind::Bunk),
                    plan.residents(kind)
                );
            }
        }
    }
    // And two seeds are two stations, not one station twice.
    assert_ne!(
        design_hash(&layout(worldgen::StationKind::Orbital, Plan::Hub, 1)),
        design_hash(&layout(worldgen::StationKind::Orbital, Plan::Hub, 2)),
    );
}

/// Every station's seed rolls the generated plan (feature 112), and over
/// a galaxy's worth of stations the generator draws all but a few — at
/// most two in a hundred — which are the drawn plan their seed rolled
/// before, as big as that plan is. The six drawn plans still differ in
/// what the user asked them to differ in — size, corridors and how many
/// live there. The spawn is a hub whatever it rolled, and every other
/// station of its system is generated or its fallback.
#[test]
fn a_station_s_plan_is_rolled_off_its_seed_and_the_spawn_is_a_hub() {
    use crate::station::{Plan, Station};
    use worldgen::{Galaxy, StationKind};
    let galaxy = Galaxy::new(data::DEFAULT_SEED, GalaxyType::SpiralTwoArm);
    let (mut generated, mut fell) = (0u32, 0u32);
    for star in 0..(galaxy.stars.len() as u32).min(40) {
        let Some(system) = galaxy.system(star) else {
            continue;
        };
        for station in Station::all_of(&system) {
            assert_eq!(Plan::rolled(station.map_seed), Plan::Generated);
            if station.plan == Plan::Generated {
                let (least, most) = crate::stationgen::side_range(station.kind);
                assert!((least..=most).contains(&station.design.build_area));
                generated += 1;
            } else {
                assert_eq!(station.plan, Plan::hand_rolled(station.map_seed));
                assert_eq!(station.design.build_area, station.plan.side(station.kind));
                fell += 1;
            }
        }
    }
    assert!(
        generated > 50 && fell * 50 <= generated + fell,
        "{generated} generated, {fell} fell back"
    );
    // Size, corridors and crew: no two drawn plans agree on all three,
    // and the pod is the smallest with the fewest, the hub the widest.
    let kind = StationKind::Orbital;
    let signature = |p: Plan| (p.side(kind), p.corridor(), p.residents(kind));
    for a in Plan::HAND {
        for b in Plan::HAND {
            assert!(a == b || signature(a) != signature(b), "{a:?} and {b:?}");
        }
    }
    assert!(
        Plan::HAND
            .iter()
            .all(|&p| p.side(kind) >= Plan::Pod.side(kind))
    );
    assert!(
        Plan::HAND
            .iter()
            .all(|&p| p.residents(kind) >= Plan::Pod.residents(kind))
    );
    assert!(
        Plan::HAND
            .iter()
            .all(|&p| p.corridor() <= Plan::Hub.corridor())
    );
    // Nobody lives on a derelict on any plan; a relay houses one.
    for plan in Plan::ALL {
        assert_eq!(plan.residents(StationKind::Derelict), 0);
        assert_eq!(plan.residents(StationKind::Relay), 1);
    }
    // The spawn is the hub whatever it rolled; the rest of its system is
    // generated, or the drawn plan the generator fell back on.
    let world = basic();
    let home = world.station(world.home).unwrap();
    assert_eq!(home.plan, Plan::Hub);
    assert_eq!(home.design.build_area, crate::station::side_of(home.kind));
    for station in &world.stations {
        if station.id != world.home && !crate::heart::is_heart(station.id) {
            assert!(
                station.plan == Plan::Generated
                    || station.plan == Plan::hand_rolled(station.map_seed),
                "{:?}",
                station.plan
            );
        }
    }
    assert!(
        world.stations.iter().any(|s| s.plan == Plan::Generated),
        "the spawn system should have a generated station"
    );
}

/// A station's rooms are rooms the crew can actually walk: from the deck
/// inside the port, the room's own navigation — the inflated grid that
/// cannot pass a one-tile gap — finds a route to every tile a fixture is
/// worked from and to every open tile of deck. Asked of the navigation
/// rather than of `validate`, because the designer's reachability is
/// four-neighbour over tiles and passes doorways the body cannot fit
/// through; a room the residents cannot get into is a room whose fixtures
/// they starve in front of, and it is cheaper to find out here than by
/// watching one stand at a doorway for a game day.
/// A one-tile corridor is walkable, straight or with a corner in it. The
/// grid aboard is phased to the tiles — five cells a tile, so a tile's
/// middle is a cell's middle — which is what puts a cell in the six-unit
/// strip a body's margin leaves down the middle of a one-tile gap. The
/// classic grid, started from the walkable area's edge, had one there or
/// not tile by tile, and a corridor the designer admits could cut a room
/// off. Built by hand: a deck split by a wall with a one-tile gap, and an
/// L of one-tile corridor through a block of wall.
///
/// The whole matrix — every plan on every kind at every seed — is asked
/// for with `BIMS_SWEEP=1` (`./check full`); a plain run walks a slice of
/// it, since a path search per tile of deck over sixty-five layouts costs
/// more than the rest of the workspace's tests together.
#[test]
fn a_station_s_rooms_and_a_one_tile_corridor_can_be_walked() {
    // --- a_station_s_rooms_can_all_be_walked_from_its_door ---
    {
        use crate::station::{Plan, layout};
        use shipdesign::validate::walkable;
        let tile = shipdesign::TILE as f32;
        let middle =
            |(x, y): (u32, u32)| bims::math::vec2((x as f32 + 0.5) * tile, (y as f32 + 0.5) * tile);
        // The scan below is a path search per tile of deck, so the matrix
        // is what this test costs: every plan on every kind at two seeds
        // (three for the hub) is sixty-five layouts and longer than every
        // other test in the workspace put together, which is why it is the
        // full tier's — `BIMS_SWEEP=1`, set by `./check full`. Without it,
        // one seed and one kind a plan, cycled so that every plan and
        // every kind is walked: the same assertions over a slice, which is
        // what catches a plan that stopped being walkable at all.
        let sweep = std::env::var("BIMS_SWEEP").is_ok();
        let kinds = worldgen::StationKind::ALL;
        let plans: Vec<(Plan, worldgen::StationKind)> = if sweep {
            Plan::ALL
                .iter()
                .flat_map(|&plan| kinds.map(|kind| (plan, kind)))
                .collect()
        } else {
            // The generated plan on every kind, since it is what every
            // station but the spawn is (feature 112); the drawn ones one
            // kind each, cycled.
            kinds
                .iter()
                .map(|&kind| (Plan::Generated, kind))
                .chain(
                    Plan::HAND
                        .iter()
                        .enumerate()
                        .map(|(i, &plan)| (plan, kinds[i % kinds.len()])),
                )
                .collect()
        };
        for (plan, kind) in plans {
            // The seed decides how many bays, shelves and batteries and the
            // holes in a derelict — and, generated, the whole building,
            // so the sweep walks eight of those a kind.
            let seeds: &[u64] = match (sweep, plan) {
                (false, _) => &[1],
                (true, Plan::Hub) => &[1, 7, 0x_5749_4e44_4f57_0001],
                (true, Plan::Generated) => &[1, 7, 11, 13, 17, 19, 23, 0x_5749_4e44_4f57_0001],
                (true, _) => &[1, 7],
            };
            for &seed in seeds {
                let design = layout(kind, plan, seed);
                let grid = design.grid();
                let room = bims::room::Room::from_layout(bims::aboard::layout_of(&design));
                let nav = bims::nav::Nav::tiled(
                    room.interior,
                    &room.solids(),
                    bims::character::BODY_MARGIN,
                    tile as f32,
                );
                let port = shipdesign::port(&design).unwrap();
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
                assert!(
                    cut_off.is_empty(),
                    "{plan:?} {kind:?} at seed {seed}: no route from the door to {cut_off:?}"
                );
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
                assert!(
                    pockets.is_empty(),
                    "{plan:?} {kind:?} at seed {seed}: deck nobody can get to at {pockets:?}"
                );
            }
        }
    }

    // --- a_one_tile_corridor_can_be_walked ---
    {
        use bims::aboard::layout_of;
        use bims::character::BODY_MARGIN;
        use bims::math::vec2;
        use bims::nav::Nav;
        use bims::room::Room;
        let tile = shipdesign::TILE as f32;
        let budget = Budget::new(1_000_000);
        let mut design = ShipDesign::new(14);
        let put = |design: &mut ShipDesign, kind: PartKind, origin: (u32, u32)| {
            *design = apply(
                design,
                &budget,
                Edit::Place {
                    kind,
                    origin,
                    rotation: Rotation::R0,
                },
            )
            .unwrap_or_else(|e| panic!("{kind:?} at {origin:?}: {e:?}"));
        };
        for y in 1..13 {
            for x in 1..13 {
                put(&mut design, PartKind::Structure, (x, y));
                put(&mut design, PartKind::Floor, (x, y));
            }
        }
        // A wall down x = 6 with one tile open at y = 6.
        for y in 1..13 {
            if y != 6 {
                put(&mut design, PartKind::Wall, (6, y));
            }
        }
        // And on the far side a block of wall with an L-shaped one-tile
        // corridor cut through it: in from the west along y = 10, then north
        // up x = 10 to the open deck at y = 7.
        for y in 8..13 {
            for x in 8..13 {
                if !((y == 10 && x <= 10) || (x == 10 && y <= 10)) {
                    put(&mut design, PartKind::Wall, (x, y));
                }
            }
        }
        let room = Room::from_layout(layout_of(&design));
        let nav = Nav::tiled(room.interior, &room.solids(), BODY_MARGIN, tile);
        let middle = |x: u32, y: u32| vec2((x as f32 + 0.5) * tile, (y as f32 + 0.5) * tile);
        let walk = |from: (u32, u32), to: (u32, u32)| {
            let a = nav.nearest_free(middle(from.0, from.1));
            let b = nav.nearest_free(middle(to.0, to.1));
            !nav.path(a, b).is_empty()
        };
        assert!(walk((3, 3), (9, 3)), "through the one-tile gap in the wall");
        assert!(walk((3, 3), (10, 10)), "into the L corridor's corner");
        assert!(walk((10, 8), (8, 10)), "round the corner of the L");
        // And the wall is still a wall: a tile of it is nowhere to stand.
        assert!(!nav.is_free(middle(6, 3)), "a wall tile reads as free");
    }
}

/// The berth is airlock to airlock: the outer faces of the two doors are on
/// the same point, the ship's opens the way the station's does not, and the
/// hull is outside the station's hull — every tile of it.
#[test]
fn the_ship_docks_airlock_to_airlock_outside_the_station() {
    let world = basic();
    let ShipState::Docked { station } = world.ship.state else {
        panic!("not docked");
    };
    let station = world.station(station).unwrap();
    let tile = shipdesign::TILE as f64;

    // The ship's door, in the system.
    let port = shipdesign::port(&world.ship.design).unwrap();
    let (fx, fy) = port.face();
    let ship_face = world.ship.anchor.add(flight::angle::rotate_design(
        dvec2(fx, fy),
        world.ship.heading,
    ));
    let station_port = station.port().unwrap();
    let (sx, sy) = station_port.face();
    let station_face = station.to_system(dvec2(sx, sy));
    assert!(
        ship_face.distance(station_face) < 1e-6,
        "the doors are {} apart",
        ship_face.distance(station_face)
    );
    // The ship's door opens towards the station: its outward step, turned
    // through the heading, is the station's outward step reversed.
    let ship_out = flight::angle::rotate_design(
        dvec2(port.outward.0 as f64, port.outward.1 as f64),
        world.ship.heading,
    );
    let station_out = flight::angle::rotate_design(
        dvec2(station_port.outward.0 as f64, station_port.outward.1 as f64),
        0.0,
    );
    assert!(
        ship_out.add(station_out).length() < 1e-9,
        "the doors do not face each other"
    );

    // No tile of the ship's frame is over a tile of the station's.
    let grid = station.design.grid();
    for part in &world.ship.design.parts {
        for (x, y) in part.tiles() {
            let middle = dvec2((x as f64 + 0.5) * tile, (y as f64 + 0.5) * tile);
            let at = world
                .ship
                .anchor
                .add(flight::angle::rotate_design(middle, world.ship.heading));
            let on_station = flight::angle::unrotate_design(at.sub(station.anchor), 0.0);
            let t = (
                (on_station.x / tile).floor() as i32,
                (on_station.y / tile).floor() as i32,
            );
            assert_eq!(
                grid.get(shipdesign::Layer::Structure, t),
                0,
                "ship tile ({x}, {y}) is over station tile {t:?}"
            );
        }
    }
}

/// A trip to a station ends at its berth, alongside, with the doors mated
/// — the same arithmetic as the spawn, reached by a trip rather than by
/// being put there.
#[test]
fn arriving_at_a_station_docks_at_its_berth() {
    let mut world = basic();
    let here = match world.ship.state {
        ShipState::Docked { station } => station,
        _ => panic!(),
    };
    let there = world.stations.iter().map(|s| s.id).find(|&id| id != here);
    let Some(there) = there else {
        return; // a system with one station has nowhere else to dock
    };
    world.leave_for_probe();
    assert_eq!(world.ship.state, ShipState::Holding);
    let events = world.step(&[
        Command::Propose {
            slot: 0,
            star: world.star_id,
            station: there,
        },
        Command::Accept { slot: 1, yes: true },
    ]);
    assert!(
        events
            .iter()
            .any(|e| matches!(e, WorldEvent::Travelled { station, .. } if *station == there)),
        "{events:?}"
    );
    assert_eq!(world.ship.state, ShipState::Docked { station: there });
    let berth = world.berth_at(there).unwrap();
    assert!(world.ship.position().distance(berth.position) < 1e-6);
    assert!((world.ship.heading - berth.heading).abs() < 1e-9);
    assert!(world.aboard.is_joined(), "tied up, but the rooms are apart");
}

/// The people on a station are simulated only while the ship is near: within
/// fifty tiles of the hull they are there, further out they are not, and a
/// derelict has nobody at any range.
#[test]
fn residents_are_there_within_fifty_tiles_and_not_beyond() {
    let mut world = basic();
    // Home, the station the ship is docked at: whatever else lives in the
    // spawn system, this one's people are known.
    let lived_in = world
        .stations
        .iter()
        .find(|s| s.id == world.home)
        .map(|s| (s.id, s.centre(), s.radius(), s.residents()))
        .expect("the spawn is a station somebody lives on");
    // The count is the residents.
    let (id, centre, radius, count) = lived_in;
    assert!(count > 0);
    // Docked, the station is in the ship's room; this is about the range, so
    // the ship is undocked first.
    world.undock_for_probe();

    // Far away: nobody.
    world.put_for_probe(centre.add(dvec2(radius + data::RESIDENTS_RANGE * 3.0, 0.0)));
    world.step(&[]);
    assert!(
        world.residents.is_none(),
        "residents at three times the range"
    );
    // Docked, the residents' room is open from the first step: the spawn is
    // a station somebody lives on.
    assert_eq!(
        basic().residents.as_ref().map(|r| r.aboard.count()),
        Some(count)
    );

    // Inside the range: the room opens, with the residents at their bunks
    // and the day the world's.
    world.put_for_probe(centre.add(dvec2(radius + data::RESIDENTS_RANGE * 0.5, 0.0)));
    world.step(&[]);
    let residents = world.residents.as_ref().expect("residents within range");
    assert_eq!(residents.station, id);
    assert_eq!(residents.aboard.count(), count);
    // Between the ranges: kept.
    world.put_for_probe(centre.add(dvec2(radius + data::RESIDENTS_RANGE * 1.1, 0.0)));
    world.step(&[]);
    assert!(world.residents.is_some(), "dropped inside the hysteresis");
    // And they live: a resident goes somewhere in a couple of hours, and
    // never off the station's deck.
    let grid = world.station(id).unwrap().design.grid();
    let tile = shipdesign::TILE as f64;
    let start = world.residents.as_ref().unwrap().aboard.position(0);
    let mut moved = false;
    for _ in 0..(2 * 60 * 60) {
        world.step(&[]);
        let residents = world.residents.as_ref().unwrap();
        for who in 0..residents.aboard.count() {
            let at = residents.aboard.position(who);
            let t = ((at.x / tile).floor() as i32, (at.y / tile).floor() as i32);
            assert_ne!(
                grid.get(shipdesign::Layer::Floor, t),
                0,
                "resident {who} off the deck at {t:?}"
            );
        }
        if residents.aboard.position(0).distance(start) > tile {
            moved = true;
        }
    }
    assert!(moved, "the residents never went anywhere");

    // Past the way out: gone.
    world.put_for_probe(centre.add(dvec2(radius + data::RESIDENTS_RANGE * 1.5, 0.0)));
    world.step(&[]);
    assert!(world.residents.is_none(), "still there past the hysteresis");

    // A derelict has nobody, however close — but its room opens all the
    // same, because the room is what draws the fixtures, and it runs.
    if let Some(derelict) = world
        .stations
        .iter()
        .find(|s| s.kind == worldgen::StationKind::Derelict)
        .map(|s| s.centre())
    {
        world.put_for_probe(derelict);
        world.step(&[]);
        let room = world
            .residents
            .as_ref()
            .expect("the derelict's room is open");
        assert_eq!(room.aboard.count(), 0, "somebody lives on the derelict");
        for _ in 0..600 {
            world.step(&[]);
        }
        assert!(world.residents.is_some());
    }
}

// --- one room while docked -----------------------------------------------------

/// Docked, the ship and the station are one deck: the crew are on it and
/// can walk through the airlocks onto the station; the station's people
/// are in a room of their own, with their own fixtures, and never on the
/// ship. Off the berth again, the ship's room is the ship's alone, with
/// the crew in it.
#[test]
fn docked_the_ship_and_the_station_are_one_room_and_the_crew_can_cross() {
    let mut world = basic();
    let ShipState::Docked { station } = world.ship.state else {
        panic!("not docked");
    };
    let residents = world.station(station).unwrap().residents();
    assert!(residents > 0, "the spawn station has nobody to meet");
    let ashore_count = residents;
    assert!(world.aboard.is_joined(), "the rooms were not joined");
    assert_eq!(world.aboard.crew_count(), 2);
    assert_eq!(world.aboard.count(), 2);
    for who in 0..world.aboard.count() {
        assert!(world.aboard.on_deck(who), "{who} is not on the deck");
    }
    // The station's people are in the station's room, all of them.
    let ashore = world.residents.as_ref().expect("the station's room");
    assert_eq!(ashore.aboard.count(), ashore_count);

    // Somewhere on the station's deck: a floor tile of the joined design
    // that is not inside the ship's own square, well away from the join.
    let tile = shipdesign::TILE as f64;
    let ship_side = world.ship.design.build_area as f64 * tile;
    let offset = world.aboard.offset;
    let on_ship = |p: DVec2| {
        p.x >= offset.x
            && p.y >= offset.y
            && p.x < offset.x + ship_side
            && p.y < offset.y + ship_side
    };
    let there = world
        .aboard
        .design
        .parts
        .iter()
        .filter(|p| p.kind == PartKind::Floor)
        .map(|p| {
            dvec2(
                (p.origin.0 as f64 + 0.5) * tile,
                (p.origin.1 as f64 + 0.5) * tile,
            )
        })
        .filter(|&p| !on_ship(p))
        .max_by(|a, b| a.x.partial_cmp(&b.x).unwrap())
        .expect("the station has deck");
    let sent = world
        .aboard
        .room
        .send_for_probe(0, bims::math::vec2(there.x as f32, there.y as f32));
    assert!(sent, "no route from the ship onto the station");
    let mut arrived = false;
    for _ in 0..(20 * 60 * 60) {
        world.step(&[]);
        let p = world.aboard.room.bim_pos(0);
        if !world.aboard.room.is_walking(0) && (p.x as f64 - there.x).abs() < 2.0 * tile {
            arrived = true;
            break;
        }
    }
    assert!(arrived, "the crew member never got across");
    let p = world.aboard.room.bim_pos(0);
    assert!(
        !on_ship(dvec2(p.x as f64, p.y as f64)),
        "arrived, but still on the ship"
    );

    // Leaving: whoever was over there walks back aboard, through the
    // airlocks the other way, and then the ship lets go — one room again,
    // the ship's alone, with the crew in it.
    let gangway = world.aboard.gangway.expect("joined, the ship's side");
    let sent = world
        .aboard
        .room
        .send_for_probe(0, bims::math::vec2(gangway.x as f32, gangway.y as f32));
    assert!(sent, "no route back onto the ship");
    let mut back = false;
    for _ in 0..LEAVING {
        world.step(&[]);
        if world.inside_ship(0) {
            back = true;
            break;
        }
    }
    assert!(back, "the crew member was never seen back on the ship");
    world.undock_for_probe();
    assert!(!world.aboard.is_joined());
    assert_eq!(world.aboard.count(), 2);
    assert_eq!(world.aboard.offset, DVec2::ZERO);
    for who in 0..2 {
        assert!(world.aboard.on_deck(who), "{who} is off the ship");
    }
    // And the station's room reopens with its people, since the ship is
    // still within range.
    world.step(&[]);
    assert_eq!(
        world.residents.as_ref().map(|r| r.aboard.count()),
        Some(ashore_count)
    );
}

/// A ship whose airlock is in its bow docks turned a quarter, and the
/// station is turned the other way into the ship's frame: every part of
/// both comes across, plus the two decked tiles of the passage, and the
/// station's door lands beyond the ship's.
#[test]
fn a_station_is_turned_into_the_frame_of_a_ship_docked_side_on() {
    use shipdesign::{Budget, Edit, Rotation, apply};
    let budget = Budget::new(Money::MAX);
    let mut design = flyer(2);
    // The starboard airlock off, and one across the bow instead: two tiles
    // of the north skin peeled, decked, and the airlock turned to lie along
    // it.
    let starboard = shipdesign::port(&design).unwrap().part_id;
    design = apply(&design, &budget, Edit::Remove { part_id: starboard }).unwrap();
    for x in [11u32, 12] {
        let wall = design.grid().get(shipdesign::Layer::Object, (x as i32, 1));
        design = apply(&design, &budget, Edit::Remove { part_id: wall }).unwrap();
        design = apply(
            &design,
            &budget,
            Edit::Place {
                kind: PartKind::Floor,
                origin: (x, 1),
                rotation: Rotation::R0,
            },
        )
        .unwrap();
    }
    design = apply(
        &design,
        &budget,
        Edit::Place {
            kind: PartKind::Airlock,
            origin: (11, 1),
            rotation: Rotation::R90,
        },
    )
    .unwrap();
    let port = shipdesign::port(&design).expect("a port in the bow");
    assert_eq!(port.outward, (0, -1));

    let world = world_with(design.clone(), REFERENCE_MONEY, 2);
    let ShipState::Docked { station } = world.ship.state else {
        panic!("not docked");
    };
    assert!(
        (world.ship.heading.abs() - std::f64::consts::FRAC_PI_2).abs() < 1e-9,
        "docked at {} rather than a quarter turn",
        world.ship.heading
    );
    let station = world.station(station).unwrap();
    let joined = &world.aboard.design;
    assert_eq!(
        joined.parts.len(),
        design.parts.len() + station.design.parts.len() + 4,
        "not every part came across"
    );
    // The passage is floor, and it is where the collars meet.
    let grid = joined.grid();
    let shift = world.aboard.offset;
    let t = shipdesign::TILE as f64;
    for (x, y) in design.part(port.part_id).unwrap().tiles() {
        let gap = (
            (x as f64 + (shift.x / t)) as i32 + port.outward.0,
            (y as f64 + (shift.y / t)) as i32 + port.outward.1,
        );
        assert_ne!(
            grid.get(shipdesign::Layer::Floor, gap),
            0,
            "no deck at {gap:?}"
        );
    }
    assert!(world.aboard.is_joined());
    for who in 0..world.aboard.count() {
        assert!(world.aboard.on_deck(who), "{who} is off the deck");
    }
}

/// `spawn_anywhere` is what `nix run .#test` lands on: some dock somebody
/// lives on, anywhere in the galaxy, different rolls giving different
/// places and the same roll the same place — and never a derelict.
/// `spawn_with_ground` is what `nix run .#test_planet` lands on: a dock
/// like `spawn_anywhere`'s, but in a system whose first planet with ground
/// has friendly people on it — so that a world opened there can be set
/// down with `land_for_probe` and not open under the guard's fire.
#[test]
fn a_roll_picks_a_lived_in_dock_anywhere_and_one_with_friendly_ground_when_asked() {
    // --- a_roll_picks_a_lived_in_dock_anywhere_in_the_galaxy ---
    {
        use crate::station::residents_of;
        let galaxy = worldgen::Galaxy::new(data::DEFAULT_SEED, GalaxyType::SpiralTwoArm);
        let mut seen = std::collections::BTreeSet::new();
        for roll in 0..12u64 {
            let (star, station) =
                crate::spawn_anywhere(&galaxy, roll).expect("somebody lives somewhere");
            assert_eq!(crate::spawn_anywhere(&galaxy, roll), Some((star, station)));
            let system = galaxy.system(star).unwrap();
            let blueprint = system.station(station).unwrap();
            assert!(
                residents_of(blueprint.kind) > 0,
                "roll {roll} landed on a {:?}",
                blueprint.kind
            );
            seen.insert((star, station));
        }
        assert!(seen.len() > 1, "every roll landed on the same dock");
        // Roll 0 is the first lived-in dock in the galaxy. The simulation's own
        // spawn is choosier — an orbital, with a belt in the system — and is
        // among the rolls all the same.
        let spawn = crate::spawn(&galaxy).unwrap();
        let docks: Vec<(u32, u32)> = (0..2000u64)
            .filter_map(|roll| crate::spawn_anywhere(&galaxy, roll))
            .collect();
        assert!(
            docks.contains(&spawn),
            "the simulation's spawn is nobody's roll"
        );
        assert!(docks[0] <= spawn);
        // And a world opens there, docked, with the residents in their room.
        let (star, station) = crate::spawn_anywhere(&galaxy, 5).unwrap();
        let world = World::start(
            flyer(2),
            REFERENCE_MONEY,
            2,
            data::DEFAULT_SEED,
            GalaxyType::SpiralTwoArm,
            star,
            station,
        )
        .expect("a world opens at the picked dock");
        assert_eq!(world.ship.state, ShipState::Docked { station });
        assert!(
            world
                .residents
                .as_ref()
                .is_some_and(|r| r.aboard.count() > 0)
        );
    }

    // --- a_roll_picks_a_dock_in_a_system_with_friendly_ground ---
    {
        use crate::station::residents_of;
        let galaxy = worldgen::Galaxy::new(data::DEFAULT_SEED, GalaxyType::SpiralTwoArm);
        let mut seen = std::collections::BTreeSet::new();
        for roll in 0..12u64 {
            let (star, station) =
                crate::spawn_with_ground(&galaxy, roll).expect("ground somewhere");
            assert_eq!(
                crate::spawn_with_ground(&galaxy, roll),
                Some((star, station))
            );
            let system = galaxy.system(star).unwrap();
            let blueprint = system.station(station).unwrap();
            assert!(residents_of(blueprint.kind) > 0 && !blueprint.hostile);
            let surfaces = crate::surface::Surface::all_of(&system, data::DEFAULT_SEED);
            let first = surfaces
                .first()
                .unwrap_or_else(|| panic!("roll {roll} picked a system with no ground"));
            assert!(!first.hostile, "roll {roll} picked hostile ground");
            seen.insert((star, station));
        }
        assert!(seen.len() > 1, "every roll landed on the same dock");
        // Every pick is one of `spawn_anywhere`'s, and some of those are not
        // picks here — a system with no ground, or an enemy's. Both lists are
        // read off the systems once: a roll generates the galaxy every time.
        let systems = galaxy.every_system();
        let docks = |with_ground: bool| -> Vec<(u32, u32)> {
            systems
                .iter()
                .filter(|system| {
                    !with_ground
                        || crate::surface::Surface::all_of(system, data::DEFAULT_SEED)
                            .first()
                            .is_some_and(|s| !s.hostile)
                })
                .flat_map(|system| {
                    system
                        .stations
                        .iter()
                        .filter(|s| residents_of(s.kind) > 0 && !s.hostile)
                        .map(move |s| (system.star_id, s.id))
                })
                .collect()
        };
        let (anywhere, grounded) = (docks(false), docks(true));
        assert!(
            grounded.len() < anywhere.len(),
            "every dock has friendly ground"
        );
        for roll in 0..12u64 {
            let pick = grounded[roll as usize % grounded.len()];
            assert_eq!(crate::spawn_with_ground(&galaxy, roll), Some(pick));
        }
        assert!(grounded.iter().all(|dock| anywhere.contains(dock)));
        // And a world opened at the pick sets down on the planet.
        let (star, station) = crate::spawn_with_ground(&galaxy, 5).unwrap();
        let mut world = World::start(
            flyer(2),
            REFERENCE_MONEY,
            2,
            data::DEFAULT_SEED,
            GalaxyType::SpiralTwoArm,
            star,
            station,
        )
        .expect("a world opens at the picked dock");
        assert!(world.land_for_probe(), "nowhere to land");
        let body = world.landed().expect("on the ground");
        assert_eq!(
            world.stance(crate::surface::surface_id(body)),
            bims::sight::Stance::Neutral
        );
        assert!(world.aboard.is_joined(), "tied up at the settlement");
    }
}

/// A ship's doors are powered: they open for whoever walks up, and the
/// pathfinder plans straight through them. Locked, one is a wall — the
/// bridge is behind a two-tile doorway on the playtest ship, and with both
/// tiles locked there is no route to the helm; James works the panel by
/// hand, as with the bathroom door, and the lock is undone the same way.
/// A doorway is somewhere to stand: a move order onto a door's tile walks
/// the Bim into the opening rather than snapping it to the deck beside,
/// and a click there is still the door's (`hit_at` says so), which is
/// what lets the screen open the door's menu *and* give the order.
#[test]
fn a_bim_can_be_sent_to_stand_in_a_doorway_and_a_locked_door_is_a_wall() {
    // --- a_locked_door_is_a_wall_and_an_unlocked_one_is_not ---
    {
        use shipdesign::fixture::playtest_ship;
        let mut world = simulation_world(playtest_ship(), data::SIMULATION_MONEY, 1);
        let tile = shipdesign::TILE as f32;
        let offset = world.aboard.offset;
        assert!(
            world.aboard.room.ship_door_count() >= 2,
            "the playtest ship has two doors, one a doorway"
        );
        // The pilot's spot, in the joined room's coordinates.
        let helm = bims::math::vec2(offset.x as f32 + 9.5 * tile, offset.y as f32 + 4.5 * tile);
        world.aboard.room.recruit_for_probe(0, true);
        let start = world.aboard.room.bim_pos(0);
        assert!(
            world.aboard.room.send_for_probe(0, helm),
            "no route to the helm to begin with"
        );
        // Door 0 is the bridge bulkhead's, the first the design lays, and it is
        // the whole two-tile doorway: locked, the bridge is sealed.
        assert!(!world.aboard.room.ship_door_is_locked(0));
        // Nobody locks a door from its panel since task 138 — no class's
        // skill says it may — so the order is refused and the door is
        // locked outright for the rest of the test.
        world.aboard.room.order_door(0, 0, bims::door::Order::Lock);
        for _ in 0..60 * 5 {
            world.step(&[]);
        }
        assert!(
            !world.aboard.room.ship_door_is_locked(0),
            "a crew member locked a door from its panel"
        );
        // The refused lock left the walk to the helm standing, so the Bim
        // is on the bridge now: walk it back out before sealing it.
        assert!(world.aboard.room.send_for_probe(0, start));
        let mut budget = 60 * 60;
        while budget > 0 && world.aboard.room.is_walking(0) {
            world.step(&[]);
            budget -= 1;
        }
        assert!(world.aboard.room.lock_door_for_probe(0, true));
        let mut budget = 60 * 5;
        while budget > 0 && world.aboard.room.is_walking(0) {
            world.step(&[]);
            budget -= 1;
        }
        assert!(
            !world.aboard.room.send_for_probe(0, helm),
            "a route through a locked door"
        );
        world
            .aboard
            .room
            .order_door(0, 0, bims::door::Order::Unlock);
        let mut budget = 60 * 60 * 5;
        while budget > 0 && world.aboard.room.ship_door_is_locked(0) {
            world.step(&[]);
            budget -= 1;
        }
        assert!(
            !world.aboard.room.ship_door_is_locked(0),
            "the door never got unlocked"
        );
        assert!(
            world.aboard.room.send_for_probe(0, helm),
            "no route through an unlocked door"
        );
    }

    // --- a_bim_can_be_sent_to_stand_in_a_doorway ---
    {
        use bims::room::HIT_SHIP_DOOR;
        use shipdesign::fixture::playtest_ship;
        let mut world = simulation_world(playtest_ship(), data::SIMULATION_MONEY, 1);
        let room = &mut world.aboard.room;
        assert!(room.ship_door_count() >= 1);
        let opening = room.ship_door_opening_for_probe(0);
        let middle = opening.center();
        assert!(!room.ship_door_is_locked(0));
        room.select_group(0, 1);
        assert_eq!(room.hit_at(middle.x, middle.y), HIT_SHIP_DOOR);
        assert_eq!(
            room.order_move(0, middle.x, middle.y),
            bims::game::ORDER_MOVING
        );
        let mut budget = 60 * 60 * 5;
        while budget > 0 && world.aboard.room.destination_for_probe(0).is_some() {
            world.step(&[]);
            budget -= 1;
        }
        let at = world.aboard.room.bim_pos(0);
        assert!(
            opening.expand(shipdesign::TILE as f32 * 0.5).contains(at),
            "James stopped at {at:?}, not in the doorway {opening:?}"
        );
        assert!(
            world.aboard.room.ship_door_is_open(0),
            "and the door stands open round him"
        );
    }
}

/// The bay aboard is drawn as six trays, one a tile along the run, laid
/// the way its use spots face: a tray a tile across for a bay lying
/// east–west and a tray a tile down for one standing north–south. The
/// side used to be read off the part's centre, and the first spot — at the
/// *end* of the run — then read as off the end rather than off the side,
/// which laid the trays across the bay in six strips.
#[test]
fn the_bay_aboard_is_a_tray_a_tile_along_its_run() {
    use bims::aboard::layout_of;
    use bims::fixtures::Bay;
    use shipdesign::parts::TILE;
    let tile = TILE as f32;

    // A bay on a square of deck, turned `rotation`. The playtest ship's
    // lay east–west until the bay went off it.
    let budget = Budget::new(1_000_000);
    let bay_on_deck = |rotation| {
        let mut design = ShipDesign::new(12);
        for y in 1..11 {
            for x in 1..11 {
                for kind in [PartKind::Structure, PartKind::Floor] {
                    design = apply(
                        &design,
                        &budget,
                        Edit::Place {
                            kind,
                            origin: (x, y),
                            rotation: Rotation::R0,
                        },
                    )
                    .unwrap();
                }
            }
        }
        apply(
            &design,
            &budget,
            Edit::Place {
                kind: PartKind::HydroBay,
                origin: (2, 2),
                rotation,
            },
        )
        .expect("a bay")
    };

    // One lying east–west is worked from the north.
    let layout = layout_of(&bay_on_deck(Rotation::R0));
    assert_eq!((layout.bay_side.x, layout.bay_side.y), (0.0, -1.0));
    let bay = Bay::at(layout.bay, layout.bay_side);
    for (i, tray) in bay.trays().iter().enumerate() {
        let want = layout.bay.min.x + (i as f32 + 0.5) * tile;
        assert!(
            (tray.center().x - want).abs() < tile / 3.0,
            "tray {i} is at {tray:?}, not a tile along the run"
        );
    }

    // One standing north–south, worked from the east: the same spots turned.
    let layout = layout_of(&bay_on_deck(Rotation::R90));
    assert_eq!((layout.bay_side.x, layout.bay_side.y), (1.0, 0.0));
    let bay = Bay::at(layout.bay, layout.bay_side);
    for (i, tray) in bay.trays().iter().enumerate() {
        let want = layout.bay.min.y + (i as f32 + 0.5) * tile;
        assert!(
            (tray.center().y - want).abs() < tile / 3.0,
            "tray {i} is at {tray:?}, not a tile down the run"
        );
    }
}

/// Not a test: a picture of a station's navigation grid, for when
/// `a_station_s_rooms_can_all_be_walked_from_its_door` says a tile is cut
/// off and the layout looks fine. Every deck tile is a digit for how much
/// of it a body can stand on, `.` for all and `x` for none, and every part
/// is its first letter. `cargo test -p world nav_map -- --ignored
/// --nocapture`. `BIMS_NAV_MAP=Ring` picks the plan by name (the hub
/// without it); `BIMS_STATION_KIND=Orbital` the kind (a relay without it)
/// and `BIMS_STATION_SEED=<n>` the seed (one) — so `BIMS_NAV_MAP=Generated
/// BIMS_STATION_SEED=42 BIMS_STATION_KIND=Orbital` is the orbital seed 42
/// generates (feature 112). `BIMS_NAV_MAP=Surface` is a town:
/// `BIMS_TOWN_SEED`, `BIMS_TOWN_BIOME` (desert, temperate, arctic) and
/// `BIMS_TOWN_POPULATION` say which, the biggest temperate town at seed one
/// without them. And `BIMS_SKETCH=<name>` writes the layout out in the
/// station builder's format to `stations/<name>.txt` (or
/// ``), so `nix run .#stationbuilder <name>` opens it.
#[test]
#[ignore]
fn nav_map_of_a_station() {
    use crate::station::Plan;
    let var = |name: &str| std::env::var(name).ok();
    let kind = var("BIMS_STATION_KIND")
        .and_then(|k| {
            worldgen::StationKind::ALL
                .into_iter()
                .find(|s| format!("{s:?}").eq_ignore_ascii_case(&k))
        })
        .unwrap_or(worldgen::StationKind::Relay);
    let seed: u64 = var("BIMS_STATION_SEED")
        .and_then(|s| s.parse().ok())
        .unwrap_or(1);
    let plan = var("BIMS_NAV_MAP")
        .and_then(|name| {
            Plan::ALL
                .into_iter()
                .chain([Plan::Surface])
                .find(|p| format!("{p:?}").eq_ignore_ascii_case(&name))
        })
        .unwrap_or(Plan::Hub);
    let design = if plan == Plan::Surface {
        let town_seed: u64 = var("BIMS_TOWN_SEED")
            .and_then(|s| s.parse().ok())
            .unwrap_or(1);
        let biome = var("BIMS_TOWN_BIOME")
            .and_then(|b| {
                crate::surface::Biome::ALL
                    .into_iter()
                    .find(|x| format!("{x:?}").eq_ignore_ascii_case(&b))
            })
            .unwrap_or(crate::surface::Biome::Temperate);
        let population: u32 = var("BIMS_TOWN_POPULATION")
            .and_then(|s| s.parse().ok())
            .unwrap_or(data::SURFACE_POPULATION.1);
        crate::station::layout_surface(town_seed, biome, population)
    } else {
        let (built, design) = crate::station::resolve(kind, plan, seed);
        println!("{kind:?} at seed {seed}: built as {built:?}");
        design
    };
    if let Some(name) = var("BIMS_SKETCH") {
        let dir = var("BIMS_STATIONS_DIR")
            .unwrap_or_else(|| format!("{}/../../stations", env!("CARGO_MANIFEST_DIR")));
        let _ = std::fs::create_dir_all(&dir);
        let path = format!("{dir}/{name}.txt");
        std::fs::write(&path, crate::station::sketch_text(&design, &name))
            .unwrap_or_else(|e| panic!("writing {path}: {e}"));
        println!("wrote {path}");
    }
    let tile = shipdesign::TILE as f32;
    let room = bims::room::Room::from_layout(bims::aboard::layout_of(&design));
    let nav = bims::nav::Nav::tiled(
        room.interior,
        &room.solids(),
        bims::character::BODY_MARGIN,
        tile,
    );
    let grid = design.grid();
    for y in 0..design.build_area as i32 {
        let row: String = (0..design.build_area as i32)
            .map(|x| {
                let object = grid.get(shipdesign::Layer::Object, (x, y));
                if object != 0 {
                    let kind = design.part(object).unwrap().kind;
                    return format!("{kind:?}").chars().next().unwrap();
                }
                if grid.get(shipdesign::Layer::Floor, (x, y)) == 0 {
                    return ' ';
                }
                let (mut free, mut all) = (0u32, 0u32);
                for i in 0..5 {
                    for j in 0..5 {
                        let p = bims::math::vec2(
                            x as f32 * tile + 6.0 + i as f32 * 10.0,
                            y as f32 * tile + 6.0 + j as f32 * 10.0,
                        );
                        all += 1;
                        free += nav.is_free(p) as u32;
                    }
                }
                match free {
                    0 => 'x',
                    f if f == all => '.',
                    f => char::from_digit(f * 9 / all, 10).unwrap(),
                }
            })
            .collect();
        println!("{y:2} {row}");
    }
}

// --- making things -----------------------------------------------------------

// --- building ------------------------------------------------------------------

/// A site laid out on the deck is walked to and built, and **its price
/// leaves the pool the moment the part goes down** (feature 95): nothing
/// is carried to it, since there are no materials left to carry. The ship
/// gets *heavier* by the part's own mass — a part is bought, brought and
/// bolted on — and the room goes on under the crew with the wall in it:
/// nobody's errand is lost, and the new wall is a solid the deck's grid
/// goes round.
#[test]
fn a_site_on_the_deck_is_paid_for_and_built_by_the_crew() {
    use bims::game::JOB_BUILD;
    use shipdesign::fixture::playtest_ship;
    use shipdesign::parts::Layer;
    // A bot to build it: a player's own Bim takes no errand of its own
    // accord (September 2026), and the site is nobody's order.
    let mut world = crate::fixture::crewed_world(playtest_ship(), data::SIMULATION_MONEY, 1, 2);
    world.set_shipyard_enabled(true);
    let money = world.crew_money();
    let price = PartKind::Wall.def().price;
    let mass = world.ship.dynamics.mass.get();
    let walls = world.ship.design.count(PartKind::Wall);

    // A wall on an open tile of the main deck.
    let at = (9, 12);
    assert!(
        world
            .ship
            .design
            .grid()
            .get(Layer::Object, (at.0 as i32, at.1 as i32))
            == 0,
        "clear"
    );
    let events = world.step(&[Command::PlaceSite {
        slot: 0,
        kind: PartKind::Wall,
        origin: at,
        rotation: Rotation::R0,
    }]);
    assert!(
        events
            .iter()
            .any(|e| matches!(e, WorldEvent::SitePlaced { .. })),
        "{events:?}"
    );
    assert_eq!(world.builds.len(), 1);
    // Nothing is spent until the part goes down: a site is a plan.
    assert_eq!(world.crew_money(), money, "nothing spent yet");

    // The crew walk over and put it together. Nothing is hauled — the
    // one errand is the build.
    let mut built = false;
    let mut stood = false;
    for _ in 0..(6 * 60 * 60) {
        let events = world.step(&[]);
        stood |= world.aboard.room.activity(1) == JOB_BUILD;
        if events.iter().any(|e| matches!(e, WorldEvent::Built { .. })) {
            built = true;
            break;
        }
    }
    assert!(built, "the wall was never built");
    assert!(stood, "nobody stood at the site");
    assert!(world.builds.is_empty(), "the site is finished with");
    assert_eq!(world.ship.design.count(PartKind::Wall), walls + 1);
    assert_ne!(
        world
            .ship
            .design
            .grid()
            .get(Layer::Object, (at.0 as i32, at.1 as i32)),
        0
    );
    // Paid for, and the ship is heavier by exactly the wall.
    assert_eq!(world.crew_money(), money - price);
    let want = mass + shipdesign::part_mass(PartKind::Wall);
    assert!(
        (world.ship.dynamics.mass.get() - want).abs() < 1e-6,
        "{} is not {want}",
        world.ship.dynamics.mass.get()
    );
    // The room came with it: the crew are still aboard and still going
    // about their day.
    assert_eq!(world.aboard.count(), 2);
}

/// A site the crew cannot pay for **waits**, with
/// [`Refusal::NotEnoughMoney`] where the part would otherwise go down,
/// and the money earned since lets it go on (feature 95). And two sites
/// the pool covers only one of are one at a time: `free_money` is the
/// pool less the sites already begun.
#[test]
fn a_site_waits_while_the_pool_cannot_cover_it() {
    use shipdesign::fixture::playtest_ship;
    let mut world = crate::fixture::crewed_world(playtest_ship(), data::SIMULATION_MONEY, 1, 2);
    world.set_shipyard_enabled(true);
    let price = PartKind::Wall.def().price;
    // Just short of one wall.
    world.set_money_for_probe(price - 1);
    world.step(&[Command::PlaceSite {
        slot: 0,
        kind: PartKind::Wall,
        origin: (9, 12),
        rotation: Rotation::R0,
    }]);
    assert_eq!(world.builds.len(), 1);
    let site = world.builds[0].clone();
    assert_eq!(site.price(&world.ship.design), price);
    assert!(!world.affordable_site(&site), "a euro short");
    // Nothing is offered to be built at it: the order is on the list, and
    // it has no minutes on it.
    let orders = world.build_orders();
    assert_eq!(orders.len(), 1);
    assert_eq!(orders[0].minutes, 0.0, "nothing to start at it");

    // A day goes by and nothing is built.
    for _ in 0..(6 * 60 * 60) {
        world.step(&[]);
    }
    assert_eq!(world.builds.len(), 1, "the site waits");
    assert_eq!(world.ship.design.count(PartKind::Wall), {
        let design = playtest_ship();
        design.count(PartKind::Wall)
    });

    // The Republic pays, and it goes on.
    world.set_money_for_probe(price * 4);
    let site = world.builds[0].clone();
    assert!(world.affordable_site(&site));
    let mut built = false;
    for _ in 0..(6 * 60 * 60) {
        if world
            .step(&[])
            .iter()
            .any(|e| matches!(e, WorldEvent::Built { .. }))
        {
            built = true;
            break;
        }
    }
    assert!(built, "the wall was never built once there was money");
    assert_eq!(world.crew_money(), price * 3);
}

/// Deck plating outside the hull is built from outside: laid against the
/// skin it has no tile beside it a body can stand on from the deck, so
/// the Bim takes the suit out through the airlock, walks round to it and
/// builds it out there — and the frame goes down with the deck, since
/// plating a bare tile lays both and is charged for both.
#[test]
fn a_site_beyond_the_hull_is_built_in_a_suit() {
    use shipdesign::fixture::playtest_ship;
    use shipdesign::parts::Layer;
    let mut world = crate::fixture::crewed_world(playtest_ship(), data::SIMULATION_MONEY, 1, 2);
    world.set_shipyard_enabled(true);
    world.undock_for_probe();
    let money = world.crew_money();
    let price = PartKind::Floor.def().price + PartKind::Structure.def().price;

    // The tile west of the west wall, amidships: nothing there at all.
    let grid = world.ship.design.grid();
    let y = 9;
    let west = (0..20)
        .find(|&x| grid.get(Layer::Structure, (x, y)) != 0)
        .expect("a hull tile on the row");
    let at = ((west - 1) as u32, y as u32);
    assert!(!grid.occupied((at.0 as i32, at.1 as i32)));
    assert_eq!(
        world.can_place_site(PartKind::Floor, at, Rotation::R0),
        Ok(()),
        "plating against the skin should go"
    );
    let events = world.step(&[Command::PlaceSite {
        slot: 0,
        kind: PartKind::Floor,
        origin: at,
        rotation: Rotation::R0,
    }]);
    assert!(
        events
            .iter()
            .any(|e| matches!(e, WorldEvent::SitePlaced { .. })),
        "{events:?}"
    );
    assert_eq!(world.builds[0].price(&world.ship.design), price);

    let mut built = false;
    let mut outside = false;
    for _ in 0..(8 * 60 * 60) {
        let events = world.step(&[]);
        outside |= world.aboard.room.is_outside(1);
        if events.iter().any(|e| matches!(e, WorldEvent::Built { .. })) {
            built = true;
            break;
        }
    }
    assert!(built, "the plating was never laid");
    assert!(outside, "nobody went outside for it");
    assert!(
        world
            .ship
            .design
            .grid()
            .has_structure((at.0 as i32, at.1 as i32))
    );
    assert_ne!(
        world
            .ship
            .design
            .grid()
            .get(Layer::Floor, (at.0 as i32, at.1 as i32)),
        0
    );
    assert_eq!(world.crew_money(), money - price, "the deck and its frame");
}

/// A blueprint nobody has walked to holds nothing: it is not under
/// construction and its price is nobody's yet. Once somebody is on the
/// way to it, it is — and its price is spoken for (feature 95) — and a
/// cancelled site frees what it had claimed, the Bim on the way turning
/// back.
#[test]
fn a_site_is_begun_once_somebody_is_on_the_way_and_a_cancel_frees_its_price() {
    use shipdesign::fixture::playtest_ship;
    let mut world = crate::fixture::crewed_world(playtest_ship(), data::SIMULATION_MONEY, 1, 2);
    world.set_shipyard_enabled(true);
    let money = world.crew_money();
    let place = |slot: u32| Command::PlaceSite {
        slot,
        kind: PartKind::Wall,
        origin: (9, 12),
        rotation: Rotation::R0,
    };

    // A blueprint with nothing done at it. The Bim is under orders so it
    // does not take the site up the moment it is laid — which it
    // otherwise would, in the same step.
    world.aboard.room.recruit_for_probe(0, true);
    world.step(&[place(0)]);
    assert!(!world.under_construction());
    assert_eq!(world.free_money(), money, "nothing spoken for yet");
    assert_eq!(world.build_orders().len(), 1);
    world.aboard.room.recruit_for_probe(0, false);

    // Once somebody is on the way to it, it is under construction — and
    // its price is spoken for.
    let mut begun = false;
    for _ in 0..(60 * 60) {
        world.step(&[]);
        if world.aboard.room.building_at(world.builds[0].id) {
            begun = true;
            break;
        }
    }
    assert!(begun, "nobody set out for it in an hour");
    assert!(world.under_construction());
    assert_eq!(
        world.free_money(),
        money - PartKind::Wall.def().price,
        "the site's price is spoken for"
    );

    // Cancelled, the claim is gone and nothing has left the pool. The
    // Bim on the way finds the site gone and turns back.
    let events = world.step(&[Command::CancelSite { slot: 0, site: 1 }]);
    assert!(events.iter().any(|e| matches!(
        e,
        WorldEvent::SiteCancelled {
            kind: PartKind::Wall
        }
    )));
    assert!(world.builds.is_empty());
    assert_eq!(world.free_money(), money);
    assert_eq!(world.crew_money(), money);
    for _ in 0..(30 * 60) {
        world.step(&[]);
    }
    assert!(
        !world.under_construction(),
        "somebody is still on the errand"
    );
    let events = world.step(&[Command::CancelSite { slot: 0, site: 1 }]);
    assert!(events.iter().any(|e| matches!(
        e,
        WorldEvent::Refused {
            why: Refusal::NoSuchSite,
            ..
        }
    )));
}

/// A site is refused where the part would not go, and where it would
/// leave the ship with a fault it has not got — a wall on the spot the
/// hob is worked from is a crew that cannot cook. And a site laid out on
/// deck that is itself only laid out goes, because the crew take the
/// sites in order.
#[test]
fn a_site_is_refused_where_the_designer_would_have_refused_it() {
    use shipdesign::EditError;
    use shipdesign::fixture::playtest_ship;
    let mut world = simulation_world(playtest_ship(), data::SIMULATION_MONEY, 1);
    world.set_shipyard_enabled(true);
    // On the hob itself: something is standing there.
    let hob = world
        .ship
        .design
        .parts
        .iter()
        .find(|p| p.kind == PartKind::Hob)
        .unwrap()
        .origin;
    assert_eq!(
        world.can_place_site(PartKind::Wall, hob, Rotation::R0),
        Err(crate::SiteRefusal::WontFit(EditError::ObjectOverlap.code()))
    );
    // On the spot it is worked from: it would go, and the crew could not
    // cook.
    let spot = (hob.0, hob.1 + 1);
    assert!(matches!(
        world.can_place_site(PartKind::Wall, spot, Rotation::R0),
        Err(crate::SiteRefusal::Fault(_))
    ));
    let events = world.step(&[Command::PlaceSite {
        slot: 0,
        kind: PartKind::Wall,
        origin: spot,
        rotation: Rotation::R0,
    }]);
    assert!(events.iter().any(|e| matches!(
        e,
        WorldEvent::Refused {
            why: Refusal::WontFit,
            ..
        }
    )));
    assert!(world.builds.is_empty());

    // Plating beyond the skin, and a wall on that plating: the second
    // stands on the first's deck, which is not there yet.
    let west = (1, 9);
    assert_eq!(
        world.can_place_site(PartKind::Wall, west, Rotation::R0),
        Err(crate::SiteRefusal::WontFit(
            EditError::MissingStructure.code()
        ))
    );
    world.step(&[Command::PlaceSite {
        slot: 0,
        kind: PartKind::Floor,
        origin: west,
        rotation: Rotation::R0,
    }]);
    assert_eq!(
        world.can_place_site(PartKind::Wall, west, Rotation::R0),
        Ok(())
    );
    world.step(&[Command::PlaceSite {
        slot: 0,
        kind: PartKind::Wall,
        origin: west,
        rotation: Rotation::R0,
    }]);
    assert_eq!(world.builds.len(), 2);
    // Both are on the list; the wall is not built until its deck is
    // there, and the room only puts together a site the world gives
    // minutes for.
    let orders = world.build_orders();
    assert_eq!(orders.len(), 2);
    assert_eq!(orders[0].site, 1);
    assert_eq!(orders[1].site, 2);
    assert_eq!(orders[1].minutes, 0.0);
    // A site shows in the checksum.
    assert_ne!(
        world.checksum(),
        simulation_world(playtest_ship(), data::SIMULATION_MONEY, 1).checksum()
    );
}

/// Sight is traced, all the way round, and stops at what is in the way: a
/// wall is seen from the room it walls and nothing past it is; a door is
/// a wall shut and nothing open; and what one eye sees, the crew see.
/// A shut door stops a line of sight in a room nobody has traced — a
/// station's room under a joined deck, whose people aim through its sight
/// while the crew's trace is the joined room's — and a trace after the
/// doors moved is not skipped as unchanged.
#[test]
fn sight_is_traced_and_a_shut_door_stops_it_without_a_trace() {
    // --- sight_is_traced_and_stops_at_walls_and_shut_doors ---
    {
        use bims::math::{Rect, vec2};
        use bims::sight::Sight;
        let t = 52.0;
        let bounds = Rect::from_min_size(vec2(0.0, 0.0), vec2(7.0 * t, 5.0 * t));
        // A wall down column 3, with a doorway at row 2.
        let wall: Vec<Rect> = [0, 1, 3, 4]
            .iter()
            .map(|&y| Rect::from_min_size(vec2(3.0 * t, y as f32 * t), vec2(t, t)))
            .collect();
        let door = Rect::from_min_size(vec2(3.0 * t, 2.0 * t), vec2(t, t));
        let mut sight = Sight::new(bounds, bounds, t, &wall, &[]);
        let middle = |x: i32, y: i32| vec2((x as f32 + 0.5) * t, (y as f32 + 0.5) * t);

        // Nothing is seen before anybody has looked.
        assert!(!sight.seen_at(middle(1, 2)));

        // One eye west of the wall, the door open.
        assert!(sight.observe(&[middle(1, 2)], &[]));
        assert!(sight.seen_at(middle(1, 2)), "its own tile");
        assert!(sight.seen_at(middle(0, 0)), "all the way round");
        assert!(sight.seen_at(middle(3, 0)), "the wall itself");
        assert!(sight.seen_at(middle(6, 2)), "straight through the doorway");
        assert!(!sight.seen_at(middle(6, 0)), "behind the wall");
        assert!(!sight.seen_at(middle(4, 4)), "behind the wall, near");
        // Nothing moved: no new trace.
        assert!(!sight.observe(&[middle(1, 2) + vec2(3.0, -4.0)], &[]));

        // The door shut: the far side goes.
        assert!(sight.observe(&[middle(1, 2)], &[door]));
        assert!(sight.seen_at(middle(3, 2)), "the shut door is seen");
        assert!(!sight.seen_at(middle(6, 2)), "and nothing past it");

        // A second eye east of the wall: the crew see both sides.
        assert!(sight.observe(&[middle(1, 2), middle(5, 3)], &[door]));
        assert!(sight.seen_at(middle(6, 0)));
        assert!(sight.seen_at(middle(0, 0)));
    }

    // --- a_shut_door_stops_a_line_of_sight_without_a_trace ---
    {
        use bims::math::{Rect, vec2};
        use bims::sight::Sight;
        let t = 52.0;
        let bounds = Rect::from_min_size(vec2(0.0, 0.0), vec2(7.0 * t, 5.0 * t));
        let wall: Vec<Rect> = [0, 1, 3, 4]
            .iter()
            .map(|&y| Rect::from_min_size(vec2(3.0 * t, y as f32 * t), vec2(t, t)))
            .collect();
        let door = Rect::from_min_size(vec2(3.0 * t, 2.0 * t), vec2(t, t));
        let mut sight = Sight::new(bounds, bounds, t, &wall, &[]);
        let middle = |x: i32, y: i32| vec2((x as f32 + 0.5) * t, (y as f32 + 0.5) * t);

        // Nothing traced: the doorway is open and the far side is in sight.
        assert!(sight.sees_from(middle(1, 2), middle(6, 2)).is_some());
        assert!(sight.clear_line(middle(1, 2), (6, 2)));
        // The door shut, still with no trace: it is in the way.
        assert!(sight.set_shut(&[door]));
        assert!(!sight.set_shut(&[door]), "the same doors again");
        assert!(sight.sees_from(middle(1, 2), middle(6, 2)).is_none());
        assert!(!sight.clear_line(middle(1, 2), (6, 2)));
        assert!(sight.clear_line(middle(1, 2), (3, 2)), "up to the door");

        // The trace reads the same doors, and does not take the doors it
        // was handed already for a mask it has yet to work out.
        assert!(sight.observe(&[middle(1, 2)], &[door]));
        assert!(sight.seen_at(middle(3, 2)));
        assert!(!sight.seen_at(middle(6, 2)));
        // The door opened between the picture and the step: the next trace
        // is not skipped as unchanged.
        assert!(sight.set_shut(&[]));
        assert!(sight.observe(&[middle(1, 2)], &[]));
        assert!(sight.seen_at(middle(6, 2)));
    }
}

/// Docked, the crew see the compartment they stand in and not the
/// station's rooms beyond its bulkheads, and of the station's bodies its
/// friendly people are always drawn and the rest only where
/// the crew can see them. Away from the berth the station's room shows
/// nobody at all.
#[test]
fn docked_the_crew_see_what_is_in_view_and_the_station_s_people_only_there() {
    let mut world = basic();
    assert!(world.aboard.is_joined());
    world.aboard.room.observe();
    let james = world.aboard.room.bim_pos(0);
    assert!(world.aboard.room.seen_at(james.x, james.y));

    // Somewhere on the station's deck, well inside its hull: its middle,
    // which is a bulkhead or two from the airlock either way.
    let station_id = world.residents.as_ref().unwrap().station;
    let station = world.station(station_id).unwrap().clone();
    let side = station.design.build_area as f64 * shipdesign::TILE as f64;
    let (origin, ex, ey) = world.aboard.station_frame.unwrap();
    let at = origin.add(ex.scale(side / 2.0)).add(ey.scale(side / 2.0));
    assert!(
        !world.aboard.room.seen_at(at.x as f32, at.y as f32),
        "the middle of the station is in view from the ship's deck"
    );

    // The residents: a friendly one drawn wherever it stands, anybody
    // else exactly where the crew's trace says they are seen — and, with
    // the crew on their own deck, not all of them seen.
    world.step(&[]);
    let ashore = world.residents.as_ref().unwrap();
    let positions: Vec<DVec2> = (0..ashore.aboard.count())
        .map(|who| ashore.aboard.position(who))
        .collect();
    let seen = world.aboard.seen(&positions);
    let room = &ashore.aboard.room;
    for (who, &s) in seen.iter().enumerate() {
        let friendly = room.is_friendly_body(who);
        assert_eq!(room.body_seen(who), s || friendly, "resident {who}");
    }
    assert!(
        seen.iter().any(|&s| !s),
        "every resident is in view from the ship"
    );

    // Off the berth, the station's room is nobody's to look into.
    world.undock_for_probe();
    world.step(&[]);
    let ashore = world.residents.as_ref().unwrap();
    for who in 0..ashore.aboard.count() {
        assert!(!ashore.aboard.room.body_seen(who as usize));
    }
}

/// A Bim standing against a wall peeks round it: pressed to the corner of
/// a room, it sees the whole of the room on the other side of the corner,
/// where one standing a tile back sees only the wedge the corner leaves.
/// The layout is the one the rule was asked for with: a wall down column
/// 9 with a doorway at row 3, a wall along row 7 from column 9 east, and
/// the Bim at (9, 8), under the corner.
#[test]
fn a_bim_against_a_wall_peeks_round_it() {
    use bims::math::{Rect, vec2};
    use bims::sight::Sight;
    let t = 52.0;
    let bounds = Rect::from_min_size(vec2(0.0, 0.0), vec2(22.0 * t, 11.0 * t));
    let tile = |x: i32, y: i32| Rect::from_min_size(vec2(x as f32 * t, y as f32 * t), vec2(t, t));
    let middle = |x: i32, y: i32| vec2((x as f32 + 0.5) * t, (y as f32 + 0.5) * t);
    let mut wall: Vec<Rect> = (0..7).filter(|&y| y != 3).map(|y| tile(9, y)).collect();
    wall.extend((9..22).map(|x| tile(x, 7)));
    let mut sight = Sight::new(bounds, bounds, t, &wall, &[]);

    // Against the wall, under its corner: the peek to the west sees up
    // the whole of the west side, row 7's wall included and the far
    // corner of it; nothing east of the wall column is seen past the wall.
    assert!(sight.observe(&[middle(9, 8)], &[]));
    assert!(
        sight.seen_at(middle(0, 0)),
        "the far corner, round the wall"
    );
    assert!(
        sight.seen_at(middle(6, 0)),
        "up the west side, round the wall"
    );
    assert!(
        sight.seen_at(middle(8, 0)),
        "straight up the wall's west face"
    );
    assert!(sight.seen_at(middle(4, 7)), "the row beside the corner");
    assert!(sight.seen_at(middle(9, 7)), "the corner itself");
    assert!(!sight.seen_at(middle(12, 6)), "behind the east wall");
    assert!(!sight.seen_at(middle(10, 0)), "the wall column's far side");
    assert!(
        sight.seen_at(middle(15, 9)),
        "the open deck to the south-east"
    );
    // The shot is fired from the peek: the eye that saw up the wall's
    // west face is the tile west of the body, not the body itself. (The
    // far corner the body sees for itself, past the corner's edge.)
    let eye = sight.sees_from(middle(9, 8), middle(8, 0)).expect("seen");
    assert_eq!(eye, middle(8, 8));
    assert_eq!(
        sight.sees_from(middle(9, 8), middle(15, 9)),
        Some(middle(9, 8))
    );
    assert_eq!(sight.sees_from(middle(9, 8), middle(12, 6)), None);

    // A tile back from the wall: no peek, and the trace stops at the
    // corner's edge — the top of the west side is out of view.
    assert!(sight.observe(&[middle(9, 9)], &[]));
    assert!(
        !sight.seen_at(middle(6, 0)),
        "up the west side is round the wall"
    );
    assert!(!sight.seen_at(middle(8, 0)));
    assert!(sight.seen_at(middle(4, 8)), "the open deck is seen as ever");
    assert_eq!(sight.eyes_from(middle(9, 9)).len(), 1);
    assert_eq!(sight.eyes_from(middle(9, 8)).len(), 3);
}

/// One fog over the whole map (task 128): a hostile station's deck is
/// under the same fog as the crew's own ship wherever nobody sees it,
/// whether anybody has ever looked there or not — nothing black, nothing
/// grey — and what the fog hides is who is standing there. Its machines
/// are drawn only in sight and a moment after; the tile a line of sight
/// reached goes back under the fog the moment none does. Docked at a
/// station that is not home — the spawn is, so it is told otherwise.
#[test]
fn a_hostile_station_is_under_the_ship_s_own_fog_and_its_machines_only_in_sight() {
    use bims::math::vec2;
    use bims::sight::Stance;
    let mut world = basic();
    let station_id = world.residents.as_ref().unwrap().station;
    assert_eq!(
        world.stance(station_id),
        Stance::Friendly,
        "the spawn is home"
    );
    // Home no longer: the machines have it.
    world.infest(station_id);
    assert_eq!(world.stance(station_id), Stance::Hostile);
    world.step(&[]);
    world.aboard.room.observe();

    // The middle of the station, well out of view and never looked at:
    // the fog, the one veil there is — the veil over a tile of the ship
    // the crew do not see.
    let station = world.station(station_id).unwrap().clone();
    let side = station.design.build_area as f64 * shipdesign::TILE as f64;
    let (origin, ex, ey) = world.aboard.station_frame.unwrap();
    let at = origin.add(ex.scale(side / 2.0)).add(ey.scale(side / 2.0));
    let middle = (at.x as f32, at.y as f32);
    let ship_tiles: Vec<(f32, f32)> = world
        .ship
        .design
        .parts
        .iter()
        .flat_map(|p| p.tiles())
        .map(|(x, y)| {
            let p = world.aboard.offset;
            (
                (x as f32 + 0.5) * shipdesign::TILE as f32 + p.x as f32,
                (y as f32 + 0.5) * shipdesign::TILE as f32 + p.y as f32,
            )
        })
        .collect();
    let unseen_aboard = ship_tiles
        .iter()
        .find(|&&(x, y)| world.aboard.room.veil_at(x, y) != 0)
        .copied()
        .expect("a tile of the ship nobody sees");
    let fog = world.aboard.room.veil_at(unseen_aboard.0, unseen_aboard.1);
    assert_eq!(fog, 1, "the fog");
    assert_eq!(
        world.aboard.room.veil_at(middle.0, middle.1),
        fog,
        "the station's middle under the ship's own fog"
    );
    // Nothing on the way from there to the ship is anything but seen or
    // the fog, and nothing aboard either.
    let james = world.aboard.room.bim_pos(0);
    for i in 0..200 {
        let t = i as f32 / 200.0;
        let x = middle.0 + (james.x - middle.0) * t;
        let y = middle.1 + (james.y - middle.1) * t;
        assert!(world.aboard.room.veil_at(x, y) <= 1, "no black, no grey");
    }
    assert!(
        ship_tiles
            .iter()
            .all(|&(x, y)| world.aboard.room.veil_at(x, y) <= 1)
    );

    // Its machines where nobody sees them are not drawn: the world hands
    // their room the crew's trace every step.
    let deck = |world: &World, i: usize| {
        let (origin, ex, ey) = world.aboard.station_frame.unwrap();
        let p = world.residents.as_ref().unwrap().aboard.position(i as u32);
        let q = origin.add(ex.scale(p.x)).add(ey.scale(p.y));
        vec2(q.x as f32, q.y as f32)
    };
    world.step(&[]);
    let crowd = world.residents.as_ref().unwrap().aboard.count() as usize;
    assert!(crowd >= 2, "a wave of them");
    assert_eq!(
        world.residents.as_ref().unwrap().aboard.room.crew_count(),
        0,
        "and no people"
    );
    let hidden: Vec<usize> = (0..crowd)
        .filter(|&i| {
            !world
                .aboard
                .room
                .seen_at(deck(&world, i).x, deck(&world, i).y)
        })
        .collect();
    assert!(!hidden.is_empty(), "a machine out of view");
    for &i in &hidden {
        assert!(
            !world.residents.as_ref().unwrap().aboard.room.body_seen(i),
            "machine {i} drawn under the fog"
        );
    }

    // An eye stood beside the first of them: its tile is seen and it is
    // drawn.
    let there = deck(&world, hidden[0]);
    world.aboard.room.observe_from_for_probe(&[there]);
    assert_eq!(world.aboard.room.veil_at(there.x, there.y), 0, "seen");
    world.step(&[]);
    assert!(
        world
            .residents
            .as_ref()
            .unwrap()
            .aboard
            .room
            .body_seen(hidden[0]),
        "drawn in sight"
    );

    // Nobody looking: the tile is back under the fog at once, and the
    // machine stays drawn for a moment after it is out of view and no
    // longer.
    world.aboard.room.observe_from_for_probe(&[]);
    assert_eq!(
        world.aboard.room.veil_at(there.x, there.y),
        fog,
        "fog again"
    );
    world.step(&[]);
    assert!(
        world
            .residents
            .as_ref()
            .unwrap()
            .aboard
            .room
            .body_seen(hidden[0]),
        "just out of view: still drawn"
    );
    let steps = (bims::game::SEEN_FOR / (data::STEP_MINUTES / time::MINUTES_PER_SECOND) as f32)
        .ceil() as u32
        + 2;
    for _ in 0..steps {
        world.step(&[]);
    }
    assert!(
        !world
            .residents
            .as_ref()
            .unwrap()
            .aboard
            .room
            .body_seen(hidden[0])
    );
    // And the crew's own eyes back: the middle is still the fog, the same
    // as before anybody looked.
    world.aboard.room.observe();
    assert_eq!(world.aboard.room.veil_at(middle.0, middle.1), fog);
}

/// A recruited crew member at a station the machines hold draws its laser
/// and shoots the machine it can see, and what lands comes off the
/// machine's parts — in its own room, since the shot is fired in the
/// crew's. The bolts fly rather than land at once.
///
/// The machine shoots back, and a one-in-twenty head shot would end the
/// run before James's fire has, so he is patched up before every step:
/// what this pins is his fire landing and ending it. Destroyed, the
/// machine is said once, and a wreck is nobody's target.
#[test]
fn a_recruited_bim_shoots_the_machines_it_can_see_and_they_are_hurt() {
    use bims::combat::WeaponKind;
    use bims::droid::DroidKind;
    use bims::sight::Stance;
    let mut world = basic();

    // Everybody carries the issued pistol, holstered until recruited.
    let gear = world.aboard.room.gear(0);
    assert_eq!(gear.weapon, Some(WeaponKind::LaserPistol.basic()));
    assert!(gear.armour.is_none());
    let stats = world.aboard.room.weapon_stats(0).unwrap();
    // A second's damage is over the magazine and its reload (October
    // 2026), a little under the trigger rate's.
    assert_eq!(stats.dps(), stats.pulls() * stats.damage);
    assert!(stats.pulls() < stats.fire_rate);
    assert!(
        (stats.hit_chance(10.0) - 0.680).abs() < 0.01,
        "the pistol at ten tiles (0.732 before October 2026 cut its reach)"
    );
    world.step(&[]);
    assert!(!world.aboard.room.is_armed(0), "holstered");

    // The machines have the station, and one of them is down the
    // corridor with a pistol of its own.
    assert!(
        world
            .stage_droid_fight_for_probe(DroidKind::Trooper, Some(WeaponKind::LaserPistol.basic()))
    );
    let station_id = world.residents.as_ref().unwrap().station;
    assert_eq!(world.stance(station_id), Stance::Hostile);

    // James stood a tile or two from the machine in the machine's own
    // room — the station frame put through the join.
    let (origin, ex, ey) = world.aboard.station_frame.unwrap();
    let there = world.residents.as_ref().unwrap().aboard.position(0);
    let on_deck = origin.add(ex.scale(there.x)).add(ey.scale(there.y));
    let stood = world.aboard.room.put_for_probe(
        0,
        bims::math::vec2(
            on_deck.x as f32 + 1.5 * shipdesign::TILE as f32,
            on_deck.y as f32,
        ),
    );
    assert!((stood.x - on_deck.x as f32).abs() < 3.0 * shipdesign::TILE as f32);
    // And the frame goes back the way it came.
    let back = world
        .aboard
        .to_station(dvec2(on_deck.x, on_deck.y))
        .unwrap();
    assert!(close(back.x, there.x) && close(back.y, there.y));
    world.aboard.room.observe();

    let before = machine_health(&world);
    let mut flew = false;
    let mut said = 0;
    for _ in 0..3_000 {
        world.aboard.room.patch_up_for_probe(0);
        let events = world.step(&[]);
        world.aboard.room.observe();
        assert!(world.aboard.room.is_armed(0), "weapon drawn");
        if world.aboard.room.bolts_in_flight() > 0 {
            flew = true;
        }
        said += events
            .iter()
            .filter(|e| matches!(e, WorldEvent::DroidDown { who: 0, .. }))
            .count();
        if machine(&world).destroyed {
            break;
        }
    }
    assert!(flew, "a bolt was in the air at some point");
    assert!(
        machine_health(&world) < before,
        "the machine was hit: {before} -> {}",
        machine_health(&world)
    );
    assert!(machine(&world).destroyed, "and destroyed within the run");
    // Said once, the step the world reads it, and a wreck is nobody's
    // target: James holds his fire.
    for _ in 0..3 {
        let events = world.step(&[]);
        said += events
            .iter()
            .filter(|e| matches!(e, WorldEvent::DroidDown { who: 0, .. }))
            .count();
    }
    assert_eq!(said, 1, "the wreck is said once");
    assert!(
        world.aboard.room.combat_targets_for_probe()[0].is_none(),
        "a wreck is not a target"
    );
    // The event names the machine: its kind in the hundreds, the body in
    // the units.
    let value = WorldEvent::DroidDown {
        station: station_id,
        who: 0,
        kind: DroidKind::Trooper.code(),
    }
    .value();
    assert_eq!(value, (100 * DroidKind::Trooper.code()) as i64);

    // Let go, the weapon is holstered again and nothing more is fired.
    world.aboard.room.recruit_for_probe(0, false);
    world.step(&[]);
    assert!(!world.aboard.room.is_armed(0));
}

/// At a station the machines hold the fight goes both ways: the machines
/// know where the crew are, shoot at them, and what lands opens a wound
/// on the crew member — in the joined room, where the bolt flew — and
/// the world says so. `stage_droid_fight_for_probe` is the start of it.
#[test]
fn the_machines_shoot_back_and_a_crew_member_hit_loses_hit_points() {
    use bims::combat::WeaponKind;
    let mut world = basic();
    assert!(world.stage_droid_fight_for_probe(
        bims::droid::DroidKind::Trooper,
        Some(WeaponKind::LaserPistol.basic())
    ));
    let station_id = world.residents.as_ref().unwrap().station;
    assert_eq!(world.stance(station_id), bims::sight::Stance::Hostile);
    assert_eq!(world.aboard.room.health(0), bims::health::MAX_HEALTH);

    let mut hit = None;
    for _ in 0..3_000 {
        // The machine whole again every step: James shoots back, and a
        // wreck shoots nobody.
        mend_machine(&mut world);
        let events = world.step(&[]);
        world.aboard.room.observe();
        if let Some(e) = events
            .iter()
            .find(|e| matches!(e, WorldEvent::CrewHit { who: 0, .. }))
        {
            hit = Some(*e);
            break;
        }
    }
    let hit = hit.expect("the machine hit James within the run");
    let WorldEvent::CrewHit { who } = hit else {
        unreachable!()
    };
    assert_eq!(who, 0);
    // The hit is on the bar already (task 120: one bar, no wounds).
    assert!(world.aboard.room.health(0) < bims::health::MAX_HEALTH);
    // The event carries the crew member, and nowhere on it (October
    // 2026: a hit lands on no part).
    assert_eq!(hit.code(), 32);
    assert_eq!(hit.value(), 0);
    assert_eq!(WorldEvent::CrewDown { who: 1 }.code(), 33);
    assert_eq!(WorldEvent::CrewDown { who: 1 }.value(), 1);
}

/// A station's people are armed off its own seed and their seat, so the
/// same station arms the same people the same way every time it is
/// reached and the checksum has nothing new to hash.
#[test]
fn a_station_s_people_are_armed_off_its_seed() {
    use bims::combat::Gear;
    let world = basic();
    let ashore = world.residents.as_ref().unwrap();
    let station = world.station(ashore.station).unwrap();
    let seed = station.map_seed;
    // The residents, and nobody else.
    let residents = station.residents();
    assert!(residents > 0);
    assert_eq!(ashore.aboard.count(), residents);
    for who in 0..residents {
        let issued = Gear::issued_for(seed ^ who as u64);
        assert!(issued.weapon.is_some(), "every resident carries something");
        assert_eq!(ashore.aboard.room.weapon(who as usize), issued.weapon);
        assert_eq!(ashore.aboard.room.gear(who as usize), issued);
    }
}

/// Where a body of the station's room is put on the joined deck, in the
/// crew's room's units — the peek while it peeks, since that is where a
/// shot at it is aimed.
fn on_deck(world: &World, who_ashore: u32) -> bims::math::Vec2 {
    let (origin, ex, ey) = world.aboard.station_frame.unwrap();
    let p = world.residents.as_ref().unwrap().aboard.exposed(who_ashore);
    let at = origin.add(ex.scale(p.x)).add(ey.scale(p.y));
    bims::math::vec2(at.x as f32, at.y as f32)
}

/// A machine with a claw charges the crew member rather than standing
/// off, and within reach the two are locked in a melee: the world says
/// so once, the crew member fires nothing more while it holds and lands
/// fists instead — a blow the world carries to the machine's parts — and
/// the claw's blow comes back the other way. James wears the armour,
/// and the machine is made whole before every step, so the lock
/// is what is looked at rather than who wins — a Warden at tier two,
/// whose head takes a fist and a bolt with some to spare, so no one blow
/// ends it between two mendings.
#[test]
fn a_claw_charges_and_locks_the_crew_member_who_fights_with_its_fists() {
    use bims::combat::{ArmourKind, Gear, MELEE_RANGE, Piece, WeaponKind};
    use bims::droid::DroidKind;
    let mut world = basic();
    world.set_droid_tier_for_probe(Some(Tier::Two));
    assert!(
        world.stage_droid_fight_for_probe(DroidKind::Warden, Some(WeaponKind::Claw.at(Tier::Two)))
    );
    world.aboard.room.issue(
        0,
        Gear {
            armour: Some(Piece::new(99, ArmourKind::Armour, Tier::One)),
            ..Gear::issued()
        },
    );
    // Kate unarmed: from the ship, a pistol reaching twenty-two tiles has
    // her firing at the machine too, and a bolt of hers in the air would
    // read as James firing while locked.
    world.aboard.room.issue(1, Gear::default());
    assert_eq!(
        world.residents.as_ref().unwrap().aboard.room.weapon(0),
        Some(WeaponKind::Claw.at(Tier::Two))
    );
    assert_eq!(WorldEvent::Locked { who: 0 }.code(), 37);
    assert_eq!(WorldEvent::Locked { who: 1 }.value(), 1);

    // A blow on James — read across the one step it landed in.
    let units = |world: &World| world.aboard.room.health(0);
    let blow = |events: &[WorldEvent], _was: f32, _world: &World| {
        events.iter().find_map(|e| match e {
            WorldEvent::CrewHit { who: 0 } => Some(()),
            _ => None,
        })
    };
    let mut locked = false;
    let mut hit = None;
    for _ in 0..3_000 {
        mend_machine(&mut world);
        let was = units(&world);
        let events = world.step(&[]);
        world.aboard.room.observe();
        if let Some(landed) = blow(&events, was, &world) {
            hit = Some(landed);
        }
        if events.contains(&WorldEvent::Locked { who: 0 }) {
            locked = true;
            break;
        }
    }
    assert!(locked, "the claw closed and locked James within the run");
    assert_eq!(world.aboard.room.is_locked(0), Some(0));
    assert!(world.aboard.room.is_armed(0), "still under arms");
    // Within reach of each other, as the world put them to each other.
    let gap = (on_deck(&world, 0) - world.aboard.room.exposed_at(0)).len();
    assert!(
        gap <= (MELEE_RANGE + 0.5) * shipdesign::TILE as f32,
        "within reach: {gap}"
    );
    // The first swing starts the step the lock forms and lands when it
    // has been swung, `SWING_TIME` on: something comes off the machine.
    let step = (data::STEP_MINUTES / time::MINUTES_PER_SECOND) as f32;
    let swing = (bims::character::SWING_TIME / step).ceil() as usize + 2;
    let mut landed = false;
    for _ in 0..swing {
        mend_machine(&mut world);
        let before = machine_health(&world);
        world.step(&[]);
        world.aboard.room.observe();
        if machine_health(&world) < before {
            landed = true;
            break;
        }
    }
    assert!(landed, "a fist landed on the machine");

    // While the lock holds nothing more is fired, and the world does not
    // say the lock again. The claw's own blow comes the other way — said
    // as a hit — before or after James's lock, since the machine reads its
    // reach a step ahead of him; the armour takes the first.
    let mut bolts = world.aboard.room.bolts_in_flight();
    let mut held = 0;
    for _ in 0..1_200 {
        mend_machine(&mut world);
        let was = units(&world);
        let events = world.step(&[]);
        world.aboard.room.observe();
        if world.aboard.room.is_locked(0).is_none() {
            break;
        }
        held += 1;
        assert!(
            !events.contains(&WorldEvent::Locked { who: 0 }),
            "said once"
        );
        let now = world.aboard.room.bolts_in_flight();
        assert!(now <= bolts, "no new bolt while locked");
        bolts = now;
        if let Some(landed) = blow(&events, was, &world) {
            hit = Some(landed);
        }
        if hit.is_some() && held > 10 {
            break;
        }
    }
    assert!(held > 0, "the lock held for a step at least");
    hit.expect("the claw landed on James");
    assert!(
        world.aboard.room.health(0) < bims::health::MAX_HEALTH,
        "and it hurt"
    );
}

/// What the station's people are handed as the crew's positions is where
/// a shot at each is aimed: the peek a crew member leans out to while it
/// peeks, else where it stands — and a word that it peeks, index for
/// index, since a bolt reaching a body in cover is dodged half the time.
/// James is stood just off the corridor with a wall on his east side —
/// the first such tile east of the port, found by asking the deck — and
/// the resident down the corridor east of him, so that his body sees
/// nothing and his aim is the peek.
#[test]
fn the_crew_are_handed_over_at_the_peek_while_peeking() {
    let mut world = basic();
    // A machine held where it is put, firing nothing: something for James
    // to aim at from the peek.
    assert!(world.stage_droid_fight_for_probe(bims::droid::DroidKind::Trooper, None));
    // The station's frame: a station point onto the joined deck, and the
    // rows of the corridor the port opens onto, in the station's units.
    let (origin, ex, ey) = world.aboard.station_frame.unwrap();
    let deck = |p: bims::math::Vec2| {
        let at = origin.add(ex.scale(p.x as f64)).add(ey.scale(p.y as f64));
        bims::math::vec2(at.x as f32, at.y as f32)
    };
    let tile = shipdesign::TILE as f32;
    let station = world.ship.state.station().unwrap();
    let side = world.station(station).unwrap().design.build_area;
    // The corridor is five tiles about the port's centre line, which
    // sits on the boundary between its middle row and the one above.
    let ashore = world
        .aboard
        .to_station(world.aboard.ashore.unwrap())
        .unwrap();
    let corridor_row = (ashore.y as f32 / tile + 0.5).floor();
    // A tile a body can stand on, or `None`: the deck snaps a body to the
    // nearest free spot, so a spot that moved is not one.
    let stands = |world: &mut World, col: f32, row: f32| -> Option<bims::math::Vec2> {
        let want = deck(bims::math::vec2((col + 0.5) * tile, (row + 0.5) * tile));
        let got = world.aboard.room.put_for_probe(0, want);
        ((got - want).len() < tile / 4.0).then_some(got)
    };
    // In the corridor's north wall — a doorway — the first free tile with
    // a wall to its east: the wall is what a peek needs, and its side is
    // what the peek looks past.
    let row = corridor_row - 3.0;
    let mut spot = None;
    for col in 3..(side as i32 - 3) {
        let col = col as f32;
        if let Some(got) = stands(&mut world, col, row)
            && stands(&mut world, col + 1.0, row).is_none()
        {
            spot = Some((col, got));
            break;
        }
    }
    let (col, spot) = spot.expect("a tile off the corridor with a wall east of it");
    world.aboard.room.put_for_probe(0, spot);
    // The machine six tiles east of it, in the corridor's middle row,
    // held there.
    let there = bims::math::vec2((col + 6.5) * tile, (corridor_row + 0.5) * tile);

    let mut peeked = false;
    for _ in 0..600 {
        let there_in_room = world
            .residents
            .as_ref()
            .unwrap()
            .aboard
            .to_room(dvec2(there.x as f64, there.y as f64));
        hold_machine(&mut world, there_in_room);
        world.step(&[]);
        world.aboard.room.observe();
        let room = &world.aboard.room;
        let handed = world.aboard.crew_ashore();
        let peeking = world.aboard.crew_peeking();
        assert_eq!(handed.len(), peeking.len());
        assert_eq!(peeking[0], room.peek(0).is_some());
        let Some(at) = handed[0] else {
            continue;
        };
        let exposed = room.exposed_at(0);
        let want = world
            .aboard
            .to_station(dvec2(exposed.x as f64, exposed.y as f64))
            .unwrap();
        assert!(
            close(at.x, want.x) && close(at.y, want.y),
            "handed over where a shot is aimed: {at:?} vs {want:?}"
        );
        if let Some(peek) = room.peek(0) {
            peeked = true;
            assert_ne!(peek, room.bim_pos(0), "the peek is beside the body");
            assert_eq!(exposed, peek);
        }
    }
    assert!(peeked, "James peeked at some point in the run");
}

/// A sniper rifle reaches past the pistol, out to the view's fourteen
/// tiles (`MAX_RANGE`; twenty and more before it), both ways round. A **machine** built with the rifle — a Warden,
/// the kind that takes cover — walks off down the corridor to its range
/// rather than closing, never to a doorway, which opens for whoever
/// stands in it (`bims::combat::Tactics::stand`), and its shot lands on
/// James from past the pistol's reach, said as a `CrewHit`. Then the crew
/// member's: the bolt flies the whole corridor of the joined deck and
/// lands with the rifle's damage at that distance, and the world carries
/// the hit to the machine — held down the corridor, put back where it
/// was before every step, so the distance is the one asked for.
/// The crew's shotgun does more up close than down the corridor: the
/// damage is the weapon's at the distance the bolt flew, read when it
/// lands, and the world carries that number to the machine's chassis.
/// Both stood still — put back where they were before every step — so
/// the distance is the one asked for.
#[test]
fn a_sniper_rifle_reaches_from_twenty_tiles_and_a_shotgun_does_as_much_at_nine_as_at_three() {
    use bims::droid::{DroidKind, DroidPart};
    // --- a_sniper_rifle_reaches_from_twenty_tiles ---
    {
        use bims::combat::{Gear, WeaponKind};
        let stats = WeaponKind::SniperRifle.stats();
        let rifle = Gear {
            weapon: Some(WeaponKind::SniperRifle.basic()),
            ..Gear::default()
        };

        // A machine with the rifle walks off to its range rather than
        // closing, stands where it can shoot from, and hits from there.
        // A rifle at four tiles is the end of an unarmoured James, so he
        // is patched up before every step and the hit counted is the
        // first landed from a stand: what this pins is where the machine
        // goes to shoot from, not what it does on the way. The crew are
        // unarmed for it, and the machine is made whole every step
        // besides.
        let mut world = basic();
        assert!(
            world.stage_droid_fight_for_probe(
                DroidKind::Warden,
                Some(WeaponKind::SniperRifle.basic())
            )
        );
        for who in 0..world.aboard.crew_count() as usize {
            world.aboard.room.issue(who, Gear::default());
        }
        let ashore = world.aboard.ashore.unwrap();
        let ashore = bims::math::vec2(ashore.x as f32, ashore.y as f32);
        let mut hit_at = None;
        for _ in 0..3_000 {
            world.aboard.room.put_for_probe(0, ashore);
            world.aboard.room.patch_up_for_probe(0);
            mend_machine(&mut world);
            let gap = (on_deck(&world, 0) - world.aboard.room.exposed_at(0)).len()
                / shipdesign::TILE as f32;
            let events = world.step(&[]);
            let standing = !machine(&world).is_walking();
            if standing
                && events
                    .iter()
                    .any(|e| matches!(e, WorldEvent::CrewHit { who: 0, .. }))
            {
                hit_at = Some(gap);
                break;
            }
        }
        let gap = hit_at.expect("the machine's rifle hit James within the run");
        // Out of the pistol's reach and inside the rifle's.
        let pistol = WeaponKind::LaserPistol.stats().range;
        assert!(gap > pistol, "from its range, not up close: {gap:.1} tiles");
        assert!(gap * shipdesign::TILE as f32 <= stats.reach());
        assert!(!machine(&world).is_walking(), "stood still to shoot");
        assert!(
            world.aboard.room.health(0) < bims::health::MAX_HEALTH,
            "and James is hurt for it"
        );

        // The long shot: James with the rifle, the machine thirteen tiles
        // down the corridor — past the pistol's 11.2, inside the rifle's
        // fourteen — held there and firing nothing.
        let mut world = basic();
        assert!(world.stage_droid_fight_for_probe(DroidKind::Warden, None));
        let station = world.ship.state.station().unwrap();
        let port = world.station(station).unwrap().port().unwrap();
        let tiles = 13.0;
        let reach = (data::ASHORE_TILES + tiles) * shipdesign::TILE as f64;
        // A row up from the port's centre line: the corridor's barricade of
        // sandbags stands on the middle row and the two below it, and the
        // gap past it is the two rows above.
        let there = bims::math::vec2(
            (port.centre.0 - port.outward.0 as f64 * reach) as f32,
            (port.centre.1 - port.outward.1 as f64 * reach) as f32 - shipdesign::TILE as f32,
        );
        world.aboard.room.issue(0, rifle);
        // Kate unarmed: under the alarm she would come and shoot too, and the
        // first hit counted would be her pistol's.
        world.aboard.room.issue(1, Gear::default());
        let mut hit_from = None;
        for _ in 0..3_000 {
            let there_in_room = world
                .residents
                .as_ref()
                .unwrap()
                .aboard
                .to_room(dvec2(there.x as f64, there.y as f64));
            hold_machine(&mut world, there_in_room);
            world.aboard.room.put_for_probe(0, ashore);
            let gap = (on_deck(&world, 0) - world.aboard.room.exposed_at(0)).len();
            world.step(&[]);
            world.aboard.room.observe();
            let body = machine(&world).body;
            if let Some(part) = DroidPart::ALL
                .into_iter()
                .find(|&p| body.health(p) < body.max(p))
            {
                hit_from = Some((gap / shipdesign::TILE as f32, part));
                break;
            }
        }
        let (hit_from, part) = hit_from.expect("the rifle hit the machine within the run");
        assert!(
            (12.0..=tiles as f32 + 1.0).contains(&hit_from),
            "from past the pistol's reach: {hit_from:.1} tiles"
        );
        assert!(hit_from * shipdesign::TILE as f32 <= stats.reach());
        // And it was the rifle's damage at that distance that landed, as
        // much of it as the part had.
        let body = machine(&world).body;
        let dealt = stats.damage_at(hit_from).min(body.max(part));
        let took = body.max(part) - body.health(part);
        assert!(
            (took - dealt).abs() < 4.0,
            "the rifle's damage came off the part: {took} of {}, {dealt} dealt",
            body.max(part)
        );
    }

    // --- the_crew_s_shotgun_does_as_much_at_nine_tiles_as_at_three ---
    {
        use bims::combat::{Gear, WeaponKind};
        let stats = WeaponKind::Shotgun.stats();
        // The first hit on the chassis, on a machine still whole: what
        // came off it — all of a Trooper's sixty up close, and as much down
        // the corridor. A hit elsewhere is mended away and waited past.
        let chassis_drop_at = |tiles: f64| -> (f32, f32) {
            let mut world = basic();
            assert!(world.stage_droid_fight_for_probe(DroidKind::Trooper, None));
            let station = world.ship.state.station().unwrap();
            let port = world.station(station).unwrap().port().unwrap();
            let ashore = world.aboard.ashore.unwrap();
            let ashore = bims::math::vec2(ashore.x as f32, ashore.y as f32);
            let reach = (data::ASHORE_TILES + tiles) * shipdesign::TILE as f64;
            let there = bims::math::vec2(
                (port.centre.0 - port.outward.0 as f64 * reach) as f32,
                (port.centre.1 - port.outward.1 as f64 * reach) as f32,
            );
            world.aboard.room.issue(
                0,
                Gear {
                    weapon: Some(WeaponKind::Shotgun.basic()),
                    ..Gear::default()
                },
            );
            world.aboard.room.issue(1, Gear::default());
            for _ in 0..3_000 {
                let there_in_room = world
                    .residents
                    .as_ref()
                    .unwrap()
                    .aboard
                    .to_room(dvec2(there.x as f64, there.y as f64));
                hold_machine(&mut world, there_in_room);
                world.aboard.room.put_for_probe(0, ashore);
                let flown = (on_deck(&world, 0) - world.aboard.room.exposed_at(0)).len()
                    / shipdesign::TILE as f32;
                world.step(&[]);
                world.aboard.room.observe();
                let body = machine(&world).body;
                let chassis = DroidPart::Chassis;
                if body.health(chassis) < body.max(chassis) {
                    return (body.max(chassis) - body.health(chassis), flown);
                }
            }
            panic!("no hit on the chassis within the run at {tiles} tiles");
        };
        let (near, near_from) = chassis_drop_at(3.0);
        let (far, far_from) = chassis_drop_at(9.0);
        assert!(
            (near_from - 3.0).abs() < 1.0 && (far_from - 9.0).abs() < 1.0,
            "stood where asked: {near_from:.1} and {far_from:.1} tiles"
        );
        // The bolt leaves the muzzle, under a tile ahead of the body
        // (feature 84), and lands a body's radius short of the middle: it
        // flies up to a tile and a bit less than the two stand apart, and
        // the damage is the curve's at the distance flown.
        let flown = |took: f32, apart: f32| {
            took >= stats.damage_at(apart) - 1.0
                && took <= stats.damage_at((apart - 1.2).max(0.0)) + 1.0
        };
        // Up close the curve's full number.
        assert!(
            flown(near, near_from),
            "the weapon's damage up close: {near} at {near_from:.1} tiles"
        );
        // Down the corridor the same: no weapon loses damage over
        // distance since October 2026 (it was less at nine).
        assert!(
            (far - near).abs() < 1e-3,
            "the same at nine tiles: {far} and {near}"
        );
        assert!(
            flown(far, far_from),
            "the weapon's damage at the distance flown: {far} at {far_from:.1} tiles"
        );
    }
}

/// The `droids` command's dock: the combat ship with its sixteen crew —
/// five at their bunks and eleven on the deck, each of them on a tile of
/// its own — every one with a gun, the five kinds dealt down the crew and
/// round again, tied up at the spawn rebuilt as the arena — bigger than
/// any kind of station, with its quarters' bunks in more columns — and,
/// once the machines have it, a wave of them and none of its people; and
/// the world steps with the crowd in it.
/// The arena and the combat ship can be walked like any station: from the
/// deck inside the port to every use spot of every part, and the arena
/// has no deck nobody can get to. Three seeds for every kind, since the
/// seed dresses the arena as it dresses a station.
#[test]
fn the_arena_is_the_combat_dock_and_it_and_the_combat_ship_can_be_walked() {
    // --- the_droids_dock_is_the_arena_with_sixteen_crew_and_a_wave ---
    {
        use crate::checksum::world_checksum;
        use bims::combat::{Gear, WeaponKind};
        use shipdesign::fixture::{COMBAT_BERTHS, COMBAT_CREW, combat_ship};
        let tile = shipdesign::parts::TILE as f32;
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
        assert_eq!(world.aboard.crew_count(), COMBAT_CREW, "fourteen crew");
        assert_eq!(world.speed_requests.len(), 1, "one player");
        // Nobody shares a tile: the five at their bunks, the rest on deck.
        assert!(COMBAT_CREW > COMBAT_BERTHS);
        let mut tiles: Vec<(i32, i32)> = (0..COMBAT_CREW as usize)
            .map(|who| {
                let at = world.aboard.room.bim_pos(who);
                ((at.x / tile).floor() as i32, (at.y / tile).floor() as i32)
            })
            .collect();
        tiles.sort_unstable();
        tiles.dedup();
        assert_eq!(tiles.len(), COMBAT_CREW as usize, "a tile each");
        let before = world_checksum(&world);
        let small = world.station(station).unwrap().design.clone();

        assert!(world.arena_dock_for_probe());
        let arena = world.station(station).unwrap().clone();
        assert_eq!(arena.design.build_area, data::ARENA_SIDE);
        assert!(
            arena.design.build_area > small.build_area,
            "bigger than it was"
        );
        assert!(
            arena.design.count(PartKind::Bunk) > small.count(PartKind::Bunk),
            "more bunks in its quarters: {}",
            arena.design.count(PartKind::Bunk)
        );
        assert!(
            shipdesign::validate(&arena.design, 1)
                .iter()
                .all(|i| i.severity != shipdesign::Severity::Error)
        );
        assert_eq!(world.ship.state, ShipState::Docked { station });
        assert!(world.aboard.is_joined(), "docked at it");
        assert_eq!(world.residents.as_ref().unwrap().station, station);
        assert_ne!(
            world_checksum(&world),
            before,
            "the crew stand on a new deck"
        );

        // A gun each, as the command issues them: the kinds round and round.
        let kinds: Vec<WeaponKind> = WeaponKind::ALL
            .iter()
            .copied()
            .cycle()
            .take(COMBAT_CREW as usize)
            .collect();
        for (who, &kind) in kinds.iter().enumerate() {
            let gear = world.aboard.room.gear(who);
            world.aboard.room.issue(
                who,
                Gear {
                    weapon: Some(kind.basic()),
                    ..gear
                },
            );
        }
        world.infest(station);
        assert_eq!(world.people_of(&arena), 0, "none of its people");
        world.step(&[]);
        let wave = world.droids_standing();
        assert!(wave > 0, "a wave of machines stands about it");
        let ashore = world.residents.as_ref().unwrap();
        assert_eq!(ashore.aboard.room.crew_count(), 0);
        assert_eq!(ashore.aboard.count(), wave, "the wave is the room");
        for (who, &kind) in kinds.iter().enumerate() {
            assert_eq!(world.aboard.room.weapon(who), Some(kind.basic()));
        }
        for _ in 0..10 {
            world.step(&[]);
        }
        assert_eq!(world.residents.as_ref().unwrap().aboard.count(), wave);
    }

    // --- the_arena_and_the_combat_ship_can_be_walked ---
    {
        use crate::station::arena;
        use shipdesign::fixture::combat_ship;
        use shipdesign::validate::walkable;
        let tile = shipdesign::TILE as f32;
        let middle =
            |(x, y): (u32, u32)| bims::math::vec2((x as f32 + 0.5) * tile, (y as f32 + 0.5) * tile);
        let walk = |design: &ShipDesign, name: &str, pockets_too: bool| {
            let grid = design.grid();
            let room = bims::room::Room::from_layout(bims::aboard::layout_of(design));
            let nav = bims::nav::Nav::tiled(
                room.interior,
                &room.solids(),
                bims::character::BODY_MARGIN,
                tile,
            );
            let port = shipdesign::port(design).unwrap();
            let inside = (
                ((port.centre.0 - port.outward.0 as f64 * tile as f64) / tile as f64) as u32,
                ((port.centre.1 - port.outward.1 as f64 * tile as f64) / tile as f64) as u32,
            );
            let from = nav.nearest_free(middle(inside));
            let mut cut_off = Vec::new();
            for part in &design.parts {
                for spot in part.use_spots() {
                    let spot = (spot.0 as u32, spot.1 as u32);
                    if !nav.can_reach(from, middle(spot)) {
                        cut_off.push((part.kind, spot));
                    }
                }
            }
            assert!(
                cut_off.is_empty(),
                "{name}: no route from the door to {cut_off:?}"
            );
            if !pockets_too {
                return;
            }
            let mut pockets = Vec::new();
            for y in 0..design.build_area as i32 {
                for x in 0..design.build_area as i32 {
                    if walkable(design, &grid, (x, y))
                        && !nav.can_reach(from, middle((x as u32, y as u32)))
                    {
                        pockets.push((x, y));
                    }
                }
            }
            assert!(
                pockets.is_empty(),
                "{name}: deck nobody can get to at {pockets:?}"
            );
        };
        for kind in worldgen::StationKind::ALL {
            for seed in [1u64, 7, 0x_5749_4e44_4f57_0001] {
                walk(
                    &arena(kind, seed),
                    &format!("arena {kind:?} at seed {seed}"),
                    true,
                );
            }
        }
        // The playtest ship has a pocket or two of deck the bow's cut leaves
        // behind the helm; the combat ship inherits them, so only its use
        // spots — every bunk's, every chair's — are asked.
        walk(&combat_ship(), "the combat ship", false);
    }
}

// --- armour and the pack ------------------------------------------------------

// --- looting a body ------------------------------------------------------------

/// **A body downed is nobody's target** (task 120; it was a body under
/// half its blood, feature 89). A crew member ashore at a station the
/// machines hold is believed where it stands; at a point of its bar it is
/// still on its feet and still believed; at nought it is downed, and the
/// machines forget it the same step — the slot goes `None`, which is what
/// `aim`, `melee_with` and the tactics all read.
#[test]
fn a_downed_crew_member_is_nobody_s_target() {
    use bims::health::DOWNED_SECONDS;
    let mut world = basic();
    // The machines have the station: one of them down the corridor from
    // the door, held where it is put and firing nothing — its eyes are
    // what is asked of.
    assert!(world.stage_droid_fight_for_probe(bims::droid::DroidKind::Trooper, None));
    world.aboard.room.recruit_for_probe(0, false);
    world.step(&[]);
    let who = send_a_crew_member_ashore(&mut world);
    let believed = |world: &World| {
        world
            .residents
            .as_ref()
            .unwrap()
            .aboard
            .room
            .believed_for_probe()[who as usize]
    };
    let mut seen = false;
    for _ in 0..LEAVING {
        world.step(&[]);
        if believed(&world).is_some() {
            seen = true;
            break;
        }
    }
    assert!(seen, "the machine has it in its sight");

    // A point of its bar is still up.
    let body = who as usize;
    world.aboard.room.set_health_for_probe(body, 1.0);
    world.step(&[]);
    assert!(!world.aboard.room.is_downed(body), "a point is still up");
    assert!(believed(&world).is_some(), "and still somebody's target");

    // And nought: downed, and gone from the list the machines shoot at.
    world.aboard.room.set_health_for_probe(body, 0.0);
    world.step(&[]);
    assert!(world.aboard.room.is_downed(body), "nought is downed");
    assert!(
        world.aboard.room.down_left(body).unwrap() > DOWNED_SECONDS - 1.0,
        "and nowhere near dead"
    );
    assert_eq!(believed(&world), None, "a body down is nobody's target");
}

// --- tiers, and the workbench's upgrade -------------------------------------

// --- the surface -------------------------------------------------------------

/// Every rocky planet and ice world of a system has a settlement, rolled
/// off the galaxy seed, the star and the body — the same for two players,
/// its side at the generator's share over a galaxy's worth of them — and
/// nothing else has one. A settlement is a station to the world, found by
/// its own id, built the first time it is asked for and not before, its
/// grid centred on the planet; whatever side it rolled, its people are a
/// stranger's (feature 104). The generator itself never saw any of it:
/// the galaxy checksum is what it was.
#[test]
fn a_landable_body_has_a_settlement_and_the_world_finds_it_as_a_station() {
    use crate::station::Plan;
    use crate::surface::{Surface, landable, surface_body, surface_id};
    use worldgen::{Galaxy, Node};
    let world = basic();
    let landable_bodies: Vec<u32> = world
        .system
        .bodies
        .iter()
        .filter(|b| landable(b.kind))
        .map(|b| b.id)
        .collect();
    assert_eq!(
        world.surfaces.iter().map(|s| s.body).collect::<Vec<_>>(),
        landable_bodies,
        "one settlement a landable body, in body order"
    );
    assert!(
        !world.surfaces.is_empty(),
        "the spawn system should have a planet to land on"
    );
    for surface in &world.surfaces {
        assert_eq!(surface.id, surface_id(surface.body));
        assert_eq!(surface_body(surface.id), Some(surface.body));
        assert!(
            !surface.is_built(),
            "a world opens without building its towns"
        );
        // Whatever the town's own roll, its people are nobody's enemy.
        if !world.is_droid_held(surface.id) {
            assert_eq!(world.stance(surface.id), bims::sight::Stance::Neutral);
        }
    }
    // A gas giant or a belt has none, and no station's id reads as one.
    for body in &world.system.bodies {
        assert_eq!(world.surface(body.id).is_some(), landable(body.kind));
    }
    for station in &world.stations {
        assert_eq!(surface_body(station.id), None);
    }
    // Asked for, it is a station: the surface plan, centred on the body.
    let surface = &world.surfaces[0];
    let station = world
        .station(surface.id)
        .expect("the settlement is a station");
    assert!(surface.is_built());
    assert_eq!(station.plan, Plan::Surface);
    assert_eq!(station.id, surface.id);
    let at = world
        .system
        .absolute_position(Node::Body(surface.body))
        .unwrap();
    assert!(station.centre().distance(at) < 1e-6);

    // The same galaxy rolls the same towns, and a different seed others.
    let again = Surface::all_of(&world.system, world.galaxy_seed);
    for (a, b) in world.surfaces.iter().zip(&again) {
        assert_eq!(
            (a.map_seed, a.hostile, a.stock),
            (b.map_seed, b.hostile, b.stock)
        );
    }
    let other = Surface::all_of(&world.system, world.galaxy_seed ^ 1);
    assert!(
        world
            .surfaces
            .iter()
            .zip(&other)
            .any(|(a, b)| a.map_seed != b.map_seed)
    );
    // The share of enemies among them, over the reference galaxy.
    let galaxy = Galaxy::new(data::DEFAULT_SEED, GalaxyType::SpiralTwoArm);
    let (mut all, mut hostile) = (0u32, 0u32);
    for star in 0..(galaxy.stars.len() as u32).min(120) {
        let Some(system) = galaxy.system(star) else {
            continue;
        };
        for surface in Surface::all_of(&system, data::DEFAULT_SEED) {
            all += 1;
            hostile += surface.hostile as u32;
        }
    }
    assert!(all > 50, "{all} settlements");
    let share = hostile as f64 / all as f64;
    assert!(
        (0.18..0.42).contains(&share),
        "{share} of settlements hostile"
    );
}

/// Feature 81: a class is worn on the deck. The world hands the room the
/// kit every step off its own `classes` — every class its own, and a
/// crew member with no class the plain body every Bim had before — and
/// `Class::outfit` is the one table between the two.
#[test]
fn a_class_is_worn_on_the_deck_and_a_crew_member_with_no_class_wears_none() {
    use crate::class::Class;
    use bims::character::Outfit;

    let mut world = basic();
    // Nobody has chosen yet, so nobody is in anything.
    world.step(&[]);
    assert_eq!(world.aboard.room.outfit(0), Outfit::Plain);
    assert_eq!(world.aboard.room.outfit(1), Outfit::Plain);

    // Every class in turn on slot 0, and slot 1 left alone throughout.
    let mut worn = Vec::new();
    for class in Class::ALL {
        assert_eq!(world.set_class(0, class), Ok(()));
        world.step(&[]);
        let outfit = world.aboard.room.outfit(0);
        assert_eq!(
            outfit,
            class.outfit(),
            "{class:?} is worn as {:?}",
            class.outfit()
        );
        assert_eq!(world.aboard.room.outfit(1), Outfit::Plain);
        worn.push(outfit);
    }
    // Six classes, six different kits: nobody looks like anybody else.
    for (i, a) in worn.iter().enumerate() {
        for b in &worn[i + 1..] {
            assert_ne!(a, b, "two classes share a kit");
        }
    }
}
