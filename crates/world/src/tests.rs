//! What the world has to be true of.
//!
//! Most of these run the real fixture in the real spawn system, because
//! almost everything here is about how the parts fit together rather than
//! about arithmetic. Where a scenario would otherwise depend on where the
//! generator happened to put a planet, the probe seams on [`World`] are
//! used instead: see `discover_for_probe` and `put_for_probe`.

use bims::combat::Tier;
use economy::{Money, Storage, trade_price};
use physics::ResourceId;
use shipdesign::fixture::flyer;
use shipdesign::parts::PartKind;
use shipdesign::{Budget, Edit, Rotation, ShipDesign, apply};
use worldgen::math::{DVec2, dvec2};
use worldgen::{GalaxyType, StationKind};

use crate::data;
use crate::event::{Refusal, WorldEvent};
use crate::fixture::{REFERENCE_MONEY, crewed_world, simulation_world};
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

/// **No medicine on anybody, and none coming back**: every crew member
/// starts with a box of dressings and a medkit in the first cells of its
/// pack that fit them — everybody's charges — and the cooldowns fill it
/// back up. That is exactly what a test which lays a pack out by hand,
/// counts the lockers or pins the hold's counts does not want in the
/// way, so it says so at the top and the charges leave everybody alone.
pub(crate) fn without_dressings(world: &mut World) {
    // The medicine is everybody's charge — a box of dressings and a
    // medkit in every pack, come back on a cooldown — so it is switched
    // off before the packs are emptied of it, or it would be back in the
    // cell the test wants half a minute later.
    world.medicine_off_for_probe();
    let kit = bims::combat::Item::Stack(ResourceId::Medkit as u32);
    for who in 0..world.aboard.room.crew_count() as usize {
        world.aboard.room.set_bandages_for_probe(who, 0);
        world.aboard.room.take_stack(who, kit, u32::MAX);
    }
    // And none in the hold either: a dock or an undock builds the room
    // afresh with the manager's own numbers back — the food's targets
    // reset the same way — so an empty hold is what actually holds the
    // packs empty across one. A test that wants dressings aboard puts
    // them back after this and does not dock.
    world.ship.design.cargo[ResourceId::Bandage as usize] = 0;
    world.on_ship_changed();
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
    assert_eq!(world.money, REFERENCE_MONEY);
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
                        vec![Command::ToDesk { slot: 0 }]
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

/// A station is traded with across its trading desk: docked, a buy or a
/// sell by a player whose crew member is not within reach of one is
/// refused, and from the desk it goes, the goods straight into the hold.
/// Every station lays a desk just inside its port; a ship has none, so
/// cast off there is no desk to stand at.
/// A dock sells what its kind sells and nothing else: an emitter is never
/// on the shelf, galvum only at an outpost, and selling is open either way.
/// The spawn is whatever kind it is, so the test reads the kind and expects
/// accordingly — the rule itself is pinned in `worldgen`.
#[test]
fn trading_wants_somebody_at_the_desk_and_a_station_only_sells_what_its_kind_sells() {
    // --- trading_wants_somebody_at_the_station_s_desk ---
    {
        let mut world = basic();
        world.set_shipyard_enabled(true);
        assert!(
            !world.aboard.room.desks().is_empty(),
            "a desk on the joined deck"
        );
        assert!(!world.at_the_desk(0));
        // The flyer's cold store is stocked: a sale is the trade that would go.
        let tofu = world.ship.design.carrying(ResourceId::Tofu);
        assert!(tofu > 0);
        let events = world.step(&[Command::Sell {
            slot: 0,
            resource: ResourceId::Tofu,
            units: 1,
        }]);
        assert!(refused_with(&events, Refusal::NotAtTheDesk), "{events:?}");
        assert_eq!(
            world.ship.design.carrying(ResourceId::Tofu),
            tofu,
            "nothing sold from across the room"
        );

        assert!(world.man_the_desk_for_probe(0));
        assert!(world.at_the_desk(0));
        assert!(!world.at_the_desk(1), "the other one is not");
        let money = world.money;
        let events = world.step(&[Command::Sell {
            slot: 0,
            resource: ResourceId::Tofu,
            units: 1,
        }]);
        assert!(
            events
                .iter()
                .any(|e| matches!(e, WorldEvent::Traded { .. })),
            "{events:?}"
        );
        assert!(world.money > money);
        assert_eq!(world.ship.design.carrying(ResourceId::Tofu), tofu - 1);

        world.undock_for_probe();
        assert!(
            world.aboard.room.desks().is_empty(),
            "no desk on a ship of its own"
        );
        assert!(world.desk_spot().is_none());
        assert!(!world.man_the_desk_for_probe(0));
    }
    // --- a_station_only_sells_what_its_kind_sells ---
    {
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
        let mut world = world_with(design, 1_000_000, 2);
        world.set_shipyard_enabled(true);
        let ShipState::Docked { station } = world.ship.state else {
            panic!("a world opens docked");
        };
        let kind = world.station(station).unwrap().kind;
        assert!(kind.sells(ResourceId::Vegetable));
        assert!(world.man_the_desk_for_probe(0), "a desk to trade at");

        // A research key is found on a desk, never on a shelf, and no
        // kind stocks one.
        let events = world.step(&[Command::Buy {
            slot: 0,
            resource: ResourceId::ResearchKey,
            units: 1,
            tier: 1,
        }]);
        assert!(refused_with(&events, Refusal::NotSoldHere), "{events:?}");
        assert_eq!(world.ship.design.carrying(ResourceId::ResearchKey), 0);

        // A pressure suit hangs where people work outside, and nowhere
        // else (feature 95).
        let events = world.step(&[Command::Buy {
            slot: 0,
            resource: ResourceId::Suit,
            units: 1,
            tier: 1,
        }]);
        let outside_work = matches!(
            kind,
            worldgen::StationKind::Refinery | worldgen::StationKind::MiningOutpost
        );
        if outside_work
            && world
                .station(station)
                .unwrap()
                .stock
                .sells(ResourceId::Suit)
        {
            assert_eq!(world.ship.design.carrying(ResourceId::Suit), 1);
        } else {
            assert!(refused_with(&events, Refusal::NotSoldHere), "{events:?}");
        }

        // Vegetables are on every shelf, and what is aboard sells
        // anywhere with somebody to buy it.
        let events = world.step(&[Command::Buy {
            slot: 0,
            resource: ResourceId::Vegetable,
            units: 1,
            tier: 1,
        }]);
        assert!(!refused_with(&events, Refusal::NotSoldHere), "{events:?}");
        let veg = world.ship.design.carrying(ResourceId::Vegetable);
        assert!(veg > 0);
        let money = world.money;
        world.step(&[Command::Sell {
            slot: 0,
            resource: ResourceId::Vegetable,
            units: 1,
        }]);
        assert_eq!(world.ship.design.carrying(ResourceId::Vegetable), veg - 1);
        assert!(world.money > money);
    }
}

/// Money works at a dock and nowhere else: off the berth there is nobody
/// to trade with, and trading is refused for that reason and no other.
/// That is `shipdesign::materials`' rule, and this is the world keeping it.
#[test]
fn nothing_is_bought_or_sold_away_from_a_station() {
    let mut world = world_with(flyer(2), 1_000_000, 2);
    world.undock_for_probe();
    world.step(&[]);
    assert_eq!(world.ship.state, ShipState::Holding);
    let tofu = world.ship.design.carrying(ResourceId::Tofu);
    for command in [
        Command::Buy {
            slot: 0,
            resource: ResourceId::Vegetable,
            units: 1,
            tier: 1,
        },
        Command::Sell {
            slot: 0,
            resource: ResourceId::Vegetable,
            units: 1,
        },
        Command::Sell {
            slot: 0,
            resource: ResourceId::Tofu,
            units: 1,
        },
    ] {
        let events = world.step(&[command]);
        assert!(
            refused_with(&events, Refusal::NotDocked),
            "holding is not docked: {command:?}"
        );
    }
    assert_eq!(world.ship.design.carrying(ResourceId::Tofu), tofu);
}

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
    for s in &world.stations {
        assert_eq!(world.stance(s.id), bims::sight::Stance::Neutral);
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

#[test]
fn buying_costs_money_and_makes_the_ship_heavier() {
    let mut world = world_with(flyer(2), 100_000, 2);
    world.set_shipyard_enabled(true);
    // Somewhere to put it first: the flyer has a cold store and no racking
    // at all.
    world.ship.design = apply(
        &world.ship.design,
        &Budget::new(10_000_000),
        Edit::Place {
            kind: PartKind::Shelf,
            origin: (7, 9),
            rotation: Rotation::R0,
        },
    )
    .unwrap();
    world.on_ship_changed();
    assert!(world.man_the_desk_for_probe(1), "a desk to trade at");

    let money = world.money;
    let mass = world.ship.dynamics.mass.get();
    let aboard = world.ship.design.carrying(ResourceId::Vegetable);
    let events = world.step(&[Command::Buy {
        slot: 1,
        resource: ResourceId::Vegetable,
        units: 10,
        tier: 1,
    }]);

    assert!(events.iter().any(|e| matches!(
        e,
        WorldEvent::Traded {
            resource: ResourceId::Vegetable,
            units: 10,
            ..
        }
    )));
    // At the desk's ask — the station's kind's lean on the book with the
    // spawn's own lean forced to nothing, then half the spread over it —
    // and never at the book itself.
    let desk = world.station(world.home).unwrap().market().unwrap();
    assert_eq!(desk.bias, economy::market::Bias::NONE, "the spawn leans");
    let quote = desk.quote(ResourceId::Vegetable);
    assert_ne!(quote.ask, trade_price(ResourceId::Vegetable));
    assert_eq!(world.money, money - 10 * quote.ask);
    assert_eq!(
        world.ship.design.carrying(ResourceId::Vegetable),
        aboard + 10
    );
    assert!(close(
        world.ship.dynamics.mass.get(),
        mass + 10.0 * ResourceId::Vegetable.mass_per_unit(),
    ));

    // And selling puts it back at the bid — by whoever is at the desk —
    // which is under the ask: a buy and a sell at one desk lose money.
    assert!(world.man_the_desk_for_probe(0));
    world.step(&[Command::Sell {
        slot: 0,
        resource: ResourceId::Vegetable,
        units: 10,
    }]);
    assert_eq!(world.money, money - 10 * (quote.ask - quote.bid));
    assert!(world.money < money);
    assert!(close(world.ship.dynamics.mass.get(), mass));
}

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

    // And a sale at a derelict is refused for want of anybody to sell to.
    let mut world = world_with(flyer(2), 100_000, 2);
    let derelict = world
        .stations
        .iter()
        .find(|s| s.kind == StationKind::Derelict)
        .map(|s| s.id);
    if let Some(id) = derelict {
        world.ship.state = ShipState::Docked { station: id };
        world.ship.design.cargo[ResourceId::Vegetable as usize] = 5;
        let events = world.step(&[Command::Sell {
            slot: 0,
            resource: ResourceId::Vegetable,
            units: 1,
        }]);
        assert!(refused_with(&events, Refusal::NoMarket));
        let events = world.step(&[Command::Buy {
            slot: 0,
            resource: ResourceId::Vegetable,
            units: 1,
            tier: 1,
        }]);
        assert!(refused_with(&events, Refusal::NotSoldHere));
    }
}

#[test]
fn what_cannot_be_paid_for_or_stowed_is_refused() {
    let mut world = world_with(flyer(2), 100, 2);
    world.set_shipyard_enabled(true);
    assert!(world.man_the_desk_for_probe(0), "a desk to trade at");
    // No money. A staple, which every shelf carries: whether the spawn
    // rolled anything else is the generator's business, not this
    // test's.
    let events = world.step(&[Command::Buy {
        slot: 0,
        resource: ResourceId::Vegetable,
        units: 100,
        tier: 1,
    }]);
    assert!(refused_with(&events, Refusal::Unaffordable));

    // Money, but nowhere to put it: the flyer has no locker room.
    world.money = 1_000_000;
    assert_eq!(world.ship.design.capacity(Storage::Locker), 0);
    let events = world.step(&[Command::Buy {
        slot: 0,
        resource: ResourceId::Medkit,
        units: 1,
        tier: 1,
    }]);
    assert!(refused_with(&events, Refusal::NoRoomAboard), "{events:?}");

    // And selling what is not there.
    let events = world.step(&[Command::Sell {
        slot: 0,
        resource: ResourceId::Schword,
        units: 1,
    }]);
    assert!(refused_with(&events, Refusal::NotAboard), "{events:?}");
}

fn refused_with(events: &[WorldEvent], want: Refusal) -> bool {
    events
        .iter()
        .any(|e| matches!(e, WorldEvent::Refused { why, .. } if *why == want))
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
            speed: Speed::Ten,
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

#[test]
fn the_world_runs_at_the_slowest_request() {
    let mut world = basic();
    world.step(&[
        Command::SetSpeed {
            slot: 0,
            speed: Speed::Top,
        },
        Command::SetSpeed {
            slot: 1,
            speed: Speed::Triple,
        },
    ]);
    assert_eq!(world.effective_speed(), Speed::Triple);

    world.step(&[Command::SetSpeed {
        slot: 1,
        speed: Speed::Paused,
    }]);
    assert_eq!(world.effective_speed(), Speed::Paused);
    assert_eq!(world.effective_speed().multiplier(), 0);

    // A slot nobody is in cannot change it.
    world.step(&[Command::SetSpeed {
        slot: 9,
        speed: Speed::Top,
    }]);
    assert_eq!(world.effective_speed(), Speed::Paused);
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

/// The invariant of `crate::locker`, asked whole: every slot lies on the
/// grid, no two overlap, the cells covered are what the class stores,
/// and each slot's thing is aboard — a piece in the hold, a gun on the
/// list, a unit counted.
fn lockers_agree(world: &World) {
    use crate::Kept;
    use shipdesign::GRID_COLS;
    let capacity = world.grid_capacity(Storage::Locker);
    let lockers = world.grid(Storage::Locker).unwrap();
    for slot in &lockers.slots {
        let laid = slot.laid();
        for y in slot.y as u32..slot.y as u32 + laid.rows as u32 {
            for x in slot.x as u32..slot.x as u32 + laid.cols as u32 {
                assert!(
                    x < GRID_COLS && y * GRID_COLS + x < capacity,
                    "{slot:?} off the grid"
                );
                let over: Vec<u32> = lockers
                    .slots
                    .iter()
                    .filter(|s| s.covers(x, y))
                    .map(|s| s.id)
                    .collect();
                assert_eq!(over, vec![slot.id], "two slots on ({x}, {y})");
            }
        }
        match slot.kept {
            Kept::Piece(id) => assert!(
                world
                    .pieces
                    .iter()
                    .any(|p| p.id == id && p.at == crate::Where::Hold),
                "{slot:?}"
            ),
            Kept::Gun(kind, tier) => assert!(world.guns_at(kind, tier) > 0, "{slot:?}"),
            Kept::Stack(id) => assert!(world.ship.design.carrying(id) > 0, "{slot:?}"),
        }
        assert!(slot.id < lockers.next);
    }
    assert!(lockers.covered() <= world.ship.design.stored(Storage::Locker));
}

/// The playtest ship's gear is laid out on the lockers' grid from the
/// first step — the sniper rifle along a row, the whole class placed,
/// nothing overlapping — and `Command::Arrange` moves a thing and turns
/// it, refuses a place it would not lie on, and is seen by the checksum.
#[test]
fn the_lockers_lay_the_gear_out_and_a_thing_can_be_moved_and_turned() {
    use crate::Kept;
    use bims::combat::WeaponKind;
    use shipdesign::GRID_COLS;
    use shipdesign::fixture::playtest_ship;
    let mut world = simulation_world(playtest_ship(), data::SIMULATION_MONEY, 1);
    let mut twin = simulation_world(playtest_ship(), data::SIMULATION_MONEY, 1);
    lockers_agree(&world);
    // Thirty-six rows of ten: two for the suit locker, eight for the
    // armoury, six for the drug lab and ten for each of the two shelves,
    // which are locker room since the money rework (feature 95).
    // Everything aboard is laid, and covers what the class counts.
    assert_eq!(world.grid_capacity(Storage::Locker), 36 * GRID_COLS);
    assert_eq!(crate::Grid::rows(world.grid_capacity(Storage::Locker)), 36);
    assert_eq!(
        world.grid(Storage::Locker).unwrap().covered(),
        world.ship.design.stored(Storage::Locker)
    );
    let sniper = *world
        .grid(Storage::Locker)
        .unwrap()
        .slots
        .iter()
        .find(|s| s.kept == Kept::Gun(WeaponKind::SniperRifle, Tier::One))
        .expect("the sniper rifle is aboard");
    assert_eq!((sniper.foot.rows, sniper.foot.cols), (1, 10));
    assert!(!sniper.turned, "a row was free, so it lies along one");
    assert_eq!(sniper.x, 0);

    // The bottom rows are empty: the rifle stood on end fits there, and
    // the twin that did the same lands on the same checksum.
    let arrange = Command::Arrange {
        slot: 0,
        class: Storage::Locker.code(),
        id: sniper.id,
        x: 9,
        y: 6,
        turned: true,
    };
    let before = world.checksum();
    assert_eq!(before, twin.checksum());
    let events = world.step(&[arrange]);
    assert!(!refused_with(&events, Refusal::NoRoom), "{events:?}");
    let moved = world
        .grid(Storage::Locker)
        .unwrap()
        .slot(sniper.id)
        .expect("still there");
    assert_eq!((moved.x, moved.y, moved.turned), (9, 6, true));
    assert_eq!((moved.laid().rows, moved.laid().cols), (10, 1));
    lockers_agree(&world);
    assert_ne!(world.checksum(), before, "the grid is in the checksum");
    twin.step(&[arrange]);
    assert_eq!(world.checksum(), twin.checksum());

    // Off the grid, over another slot, or a slot that is not there: refused
    // and nothing moved.
    for bad in [
        // Off the bottom: the lockers are thirty-six rows since a shelf
        // became locker room (feature 95), and a rifle stood on end at
        // row 30 reaches past them.
        Command::Arrange {
            slot: 0,
            class: Storage::Locker.code(),
            id: sniper.id,
            x: 9,
            y: 30,
            turned: true,
        },
        Command::Arrange {
            slot: 0,
            class: Storage::Locker.code(),
            id: sniper.id,
            x: 0,
            y: 0,
            turned: false,
        },
        Command::Arrange {
            slot: 0,
            class: Storage::Locker.code(),
            id: 9_999,
            x: 0,
            y: 15,
            turned: false,
        },
    ] {
        let events = world.step(&[bad]);
        assert!(
            refused_with(&events, Refusal::NoRoom),
            "{bad:?}: {events:?}"
        );
        let still = world
            .grid(Storage::Locker)
            .unwrap()
            .slot(sniper.id)
            .unwrap();
        assert_eq!((still.x, still.y, still.turned), (9, 6, true));
    }
    lockers_agree(&world);
}

/// A fetch of a slot takes the thing in *that* slot — two rifles alike,
/// and the one clicked is the one that goes — and a stow wants a run of
/// cells for the thing, not just the area: the lockers full but for
/// seven scattered cells refuse a rifle the count would take.
#[test]
fn a_fetch_takes_the_slot_asked_for_and_a_stow_wants_a_run_of_cells() {
    use crate::{FetchKind, Kept};
    use bims::combat::{Item, WeaponKind};
    use shipdesign::fixture::playtest_ship;
    let mut world = simulation_world(playtest_ship(), data::SIMULATION_MONEY, 1);
    // The dressings every Bim carries are out of the way (feature 87).
    without_dressings(&mut world);
    world.ship.design.cargo[ResourceId::AutoRifle as usize] += 1;
    world.on_ship_changed();
    lockers_agree(&world);
    let rifles: Vec<crate::Slot> = world
        .grid(Storage::Locker)
        .unwrap()
        .slots
        .iter()
        .filter(|s| s.kept == Kept::Gun(WeaponKind::AutoRifle, Tier::One))
        .copied()
        .collect();
    assert_eq!(rifles.len(), 2);
    let (first, second) = (rifles[0], rifles[1]);
    at_the_armoury(&mut world, 0);
    let events = world.step(&[Command::Fetch {
        slot: 0,
        who: 0,
        kind: FetchKind::Slot {
            class: Storage::Locker.code(),
            id: second.id,
        },
    }]);
    assert!(!refused_with(&events, Refusal::NotAboard), "{events:?}");
    assert!(
        world
            .grid(Storage::Locker)
            .unwrap()
            .slot(second.id)
            .is_none(),
        "that one went"
    );
    assert_eq!(
        world
            .grid(Storage::Locker)
            .unwrap()
            .slot(first.id)
            .map(|s| (s.x, s.y)),
        Some((first.x, first.y)),
        "the other stayed put"
    );
    assert_eq!(world.ship.design.carrying(ResourceId::AutoRifle), 1);
    lockers_agree(&world);
    guns_agree(&world);
    let pack = world.aboard.room.pack(0);
    assert!(pack.contains(&Some(Item::Weapon(WeaponKind::AutoRifle.basic()))));

    // Fill the grid to the cell with grenades — one to a cell and no
    // stacking, where a box of dressings is four cells and five of them
    // since feature 87 — then take seven out of slots on seven
    // different rows and columns: seven cells free, and no run of seven
    // among them.
    let spare = world.ship.design.spare(Storage::Locker);
    world.ship.design.cargo[ResourceId::Grenade as usize] += spare;
    world.on_ship_changed();
    assert_eq!(world.ship.design.spare(Storage::Locker), 0);
    lockers_agree(&world);
    let mut taken: Vec<(u8, u8)> = Vec::new();
    let mut scattered = Vec::new();
    for s in world
        .grid(Storage::Locker)
        .unwrap()
        .slots
        .iter()
        .filter(|s| s.kept == Kept::Stack(ResourceId::Grenade))
    {
        if taken.iter().all(|&(x, y)| x != s.x && y != s.y) {
            taken.push((s.x, s.y));
            scattered.push(s.id);
        }
        if scattered.len() == 7 {
            break;
        }
    }
    assert_eq!(scattered.len(), 7, "{taken:?}");
    for id in scattered {
        at_the_armoury(&mut world, 0);
        let events = world.step(&[Command::Fetch {
            slot: 0,
            who: 0,
            kind: FetchKind::Slot {
                class: Storage::Locker.code(),
                id: id,
            },
        }]);
        assert!(!refused_with(&events, Refusal::PackFull), "{events:?}");
    }
    assert_eq!(world.ship.design.spare(Storage::Locker), 7);
    assert!(
        world.ship.design.has_room(ResourceId::AutoRifle, 1),
        "by area"
    );
    assert!(!world.has_room(ResourceId::AutoRifle, 1), "not by the grid");
    let cell = world
        .aboard
        .room
        .pack(0)
        .iter()
        .position(|c| matches!(c, Some(Item::Weapon(_))))
        .expect("the rifle is in the pack");
    at_the_armoury(&mut world, 0);
    let events = world.step(&[Command::Stow {
        slot: 0,
        who: 0,
        cell: cell as u32,
    }]);
    assert!(refused_with(&events, Refusal::NoRoom), "{events:?}");
    assert_eq!(world.ship.design.carrying(ResourceId::AutoRifle), 1);
    lockers_agree(&world);
}

/// The grid on its own: a thing is laid unturned where that fits and
/// turned where only that does; stacks fill to their size before a new
/// one is laid and empty from the last; a count past the grid leaves the
/// overflow unplaced, never forced, and laid the moment room is made; and
/// a thing that fits nowhere as the grid stands has the grid laid again,
/// biggest first, before it is given up on.
#[test]
fn the_grid_turns_a_thing_to_fit_and_keeps_an_overflow_unplaced() {
    use crate::{Grid, Kept, Wanted};
    use economy::Footprint;
    // Two rows of ten. A rifle does not stack; ore goes ten to a cell.
    let capacity = 20;
    let rifle = Footprint::new(1, 7);
    let one = Footprint::new(1, 1);
    let rifles = |n: u32| Wanted::Units(ResourceId::AutoRifle, n, rifle);
    let mut grid = Grid::default();
    // A rifle along each row; a third fits neither way — three cells
    // left in a row, and two rows to stand in.
    for _ in 0..2 {
        assert!(
            grid.place(capacity, Kept::Stack(ResourceId::AutoRifle), 1, rifle)
                .is_some()
        );
    }
    assert!(grid.slots.iter().all(|s| !s.turned));
    assert!(
        grid.place(capacity, Kept::Stack(ResourceId::AutoRifle), 1, rifle)
            .is_none()
    );
    // Leg guards, three rows by two, stand in no gap two rows tall — but
    // lie in one, turned.
    let legs = Footprint::new(3, 2);
    let id = grid
        .place(capacity, Kept::Stack(ResourceId::LegGuard), 1, legs)
        .expect("turned to fit");
    let slot = *grid.slot(id).unwrap();
    assert_eq!((slot.x, slot.y, slot.turned), (7, 0, true));
    assert_eq!((slot.laid().rows, slot.laid().cols), (2, 3));

    // Settled against the two rifles, the guards and a medkit: the grid
    // is full, so the medkit stays off it — counted, in no slot; once
    // the guards are not wanted it lands where they lay.
    let medkit = Wanted::Units(ResourceId::Medkit, 1, Footprint::new(2, 2));
    let wanted = [
        rifles(2),
        Wanted::Units(ResourceId::LegGuard, 1, legs),
        medkit,
    ];
    grid.settle(capacity, &wanted);
    assert_eq!(grid.slots.len(), 3, "{:?}", grid.slots);
    assert_eq!(grid.units_of(ResourceId::Medkit), 0);
    grid.settle(capacity, &wanted);
    assert_eq!(grid.slots.len(), 3, "idempotent");
    grid.settle(capacity, &[rifles(2), medkit]);
    assert_eq!(grid.slots.len(), 3);
    assert!(grid.slot(id).is_none(), "the guards' slot went");
    let landed = grid
        .slots
        .iter()
        .find(|s| s.kept == Kept::Stack(ResourceId::Medkit))
        .expect("laid");
    assert_eq!((landed.x, landed.y, landed.turned), (7, 0, false));

    // Stacks: twenty-five ore is three cells, ten, ten and five; five
    // more top the last up and take no cell; seven off come off the last
    // stack first, and a stack emptied loses its slot.
    let mut grid = Grid::default();
    grid.settle(capacity, &[Wanted::Units(ResourceId::Tofu, 25, one)]);
    let counts = |grid: &Grid| -> Vec<u32> { grid.slots.iter().map(|s| s.count).collect() };
    assert_eq!(counts(&grid), vec![10, 10, 5]);
    assert_eq!(grid.units_of(ResourceId::Tofu), 25);
    assert!(grid.can_take(capacity, ResourceId::Tofu, 175, one));
    assert!(
        !grid.can_take(capacity, ResourceId::Tofu, 176, one),
        "twenty cells"
    );
    grid.settle(capacity, &[Wanted::Units(ResourceId::Tofu, 30, one)]);
    assert_eq!(counts(&grid), vec![10, 10, 10]);
    grid.settle(capacity, &[Wanted::Units(ResourceId::Tofu, 23, one)]);
    assert_eq!(counts(&grid), vec![10, 10, 3]);
    grid.settle(capacity, &[Wanted::Units(ResourceId::Tofu, 12, one)]);
    assert_eq!(counts(&grid), vec![10, 2]);
    let first = grid.slots[0].id;
    assert_eq!(
        grid.remove(ResourceId::Tofu, 4, Some(first)),
        4,
        "off the stack asked for"
    );
    assert_eq!(counts(&grid), vec![6, 2]);
    grid.settle(capacity, &[Wanted::Units(ResourceId::Tofu, 0, one)]);
    assert!(grid.slots.is_empty());

    // A bandage moved to the middle of a row leaves no run of seven in it:
    // a second rifle fits nowhere as the grid stands, and the settle lays
    // the grid again — rifles first, the bandage after — rather than
    // leave it off. The ids survive the repack.
    let mut grid = Grid::default();
    let bandage = grid
        .place(capacity, Kept::Stack(ResourceId::Bandage), 1, one)
        .unwrap();
    assert!(grid.arrange(capacity, bandage, 5, 0, false));
    let first = grid
        .place(capacity, Kept::Stack(ResourceId::AutoRifle), 1, rifle)
        .unwrap();
    assert_eq!(grid.slot(first).map(|s| (s.x, s.y)), Some((0, 1)));
    assert!(grid.first_fit(capacity, rifle).is_none());
    grid.settle(
        capacity,
        &[rifles(2), Wanted::Units(ResourceId::Bandage, 1, one)],
    );
    assert_eq!(grid.slots.len(), 3, "{:?}", grid.slots);
    assert!(grid.slot(bandage).is_some() && grid.slot(first).is_some());
    let ys: Vec<u8> = grid
        .slots
        .iter()
        .filter(|s| s.kept == Kept::Stack(ResourceId::AutoRifle))
        .map(|s| s.y)
        .collect();
    assert_eq!(ys, vec![0, 1]);
    assert_eq!(grid.slot(bandage).map(|s| (s.x, s.y)), Some((7, 0)));
}

/// The cold store and the lockers are grids of stacks: the playtest
/// ship's forty vegetables are four crates of ten on two cells each, the
/// twenty tofu two blocks of four by four; a fetch of a slot takes one
/// off *that* stack; a sale empties the last stack first; and a block the
/// cold store has area for but no four-by-four run of cells for is
/// refused.
#[test]
fn the_shelves_hold_stacks_and_a_fetch_takes_one_off_the_stack_asked_for() {
    use crate::{FetchKind, Kept};
    use shipdesign::fixture::playtest_ship;
    let mut world = simulation_world(playtest_ship(), data::SIMULATION_MONEY, 1);
    without_dressings(&mut world);
    let cold = world.grid(Storage::ColdStore).unwrap();
    let veg: Vec<crate::Slot> = cold
        .slots
        .iter()
        .filter(|s| s.kept == Kept::Stack(ResourceId::Vegetable))
        .copied()
        .collect();
    assert_eq!(
        veg.iter().map(|s| s.count).collect::<Vec<u32>>(),
        vec![10; 4],
        "ten to a crate"
    );
    assert_eq!(cold.units_of(ResourceId::Vegetable), 40);
    assert_eq!(
        cold.slots
            .iter()
            .filter(|s| s.kept == Kept::Stack(ResourceId::Tofu))
            .count(),
        2,
        "ten to a block"
    );
    // Four crates at one by two and two blocks at four by four.
    assert_eq!(world.ship.design.stored(Storage::ColdStore), 4 * 2 + 2 * 16);
    assert_eq!(cold.covered(), world.ship.design.stored(Storage::ColdStore));
    // Area for three more blocks of tofu, cells in a run for two.
    assert!(world.ship.design.has_room(ResourceId::Tofu, 30), "by area");
    assert!(world.has_room(ResourceId::Tofu, 20));
    assert!(!world.has_room(ResourceId::Tofu, 30), "not by the grid");
    assert_eq!(world.room_for(ResourceId::Tofu, 30), 20);

    // One off the second crate, by its slot, standing at the cold store:
    // that crate is nine, the first still ten.
    let second = veg[1].id;
    let store = world
        .aboard
        .room
        .container_spot(bims::game::Container::Fridge(0))
        .expect("a cold store with a use spot");
    world.aboard.room.put_for_probe(0, store);
    let events = world.step(&[Command::Fetch {
        slot: 0,
        who: 0,
        kind: FetchKind::Slot {
            class: Storage::ColdStore.code(),
            id: second,
        },
    }]);
    assert!(
        !refused_with(&events, Refusal::OutOfReach) && !refused_with(&events, Refusal::NotAboard),
        "{events:?}"
    );
    assert_eq!(world.ship.design.carrying(ResourceId::Vegetable), 39);
    let cold = world.grid(Storage::ColdStore).unwrap();
    assert_eq!(cold.slot(second).map(|s| s.count), Some(9));
    assert_eq!(cold.slot(veg[0].id).map(|s| s.count), Some(10));
    assert_eq!(cold.units_of(ResourceId::Vegetable), 39);
    // A sale of nine comes off the last stack.
    assert!(world.man_the_desk_for_probe(0));
    let events = world.step(&[Command::Sell {
        slot: 0,
        resource: ResourceId::Vegetable,
        units: 9,
    }]);
    assert!(
        events.contains(&WorldEvent::Traded {
            slot: 0,
            resource: ResourceId::Vegetable,
            units: -9
        }),
        "{events:?}"
    );
    let cold = world.grid(Storage::ColdStore).unwrap();
    let counts: Vec<u32> = veg
        .iter()
        .filter_map(|s| cold.slot(s.id).map(|s| s.count))
        .collect();
    assert_eq!(counts, vec![10, 9, 10, 1]);
    assert_eq!(cold.units_of(ResourceId::Vegetable), 30);
}

/// The room's footprint table is the world's said again by code — this
/// crate does not know `physics` — and the two agree for every resource,
/// as a piece, a gun, a unit or a key; and a thing moved across a pack
/// (`Command::Repack`) is seen by the checksum, since a piece's cell is
/// in it.
#[test]
fn the_pack_lays_things_by_the_same_footprints_as_the_lockers() {
    use bims::combat::{Item, PACK_COLS};
    use shipdesign::fixture::playtest_ship;
    // Bar the keys, which are one cell on a desk and two tall in a pack.
    for &id in ResourceId::ALL
        .iter()
        .filter(|&&id| crate::armour::key_tier_of(id).is_none())
    {
        let foot = economy::footprint(id);
        let item = match crate::armour::kind_of(id) {
            Some(kind) => crate::Piece::new(1, kind, Tier::One).item(),
            None => crate::armour::item_of(id),
        };
        assert_eq!(item.footprint(), (foot.rows, foot.cols), "{id:?}");
    }
    assert_eq!(Item::Key(1).footprint(), (2, 1));
    assert_eq!(Item::Key(2).footprint(), (2, 1));
    assert_eq!(
        crate::armour::item_of(ResourceId::ResearchKeyTwo),
        Item::Key(2)
    );
    let mut world = simulation_world(playtest_ship(), data::SIMULATION_MONEY, 1);
    let mut twin = simulation_world(playtest_ship(), data::SIMULATION_MONEY, 1);
    // An empty pack, so the helm lies at the top left.
    without_dressings(&mut world);
    without_dressings(&mut twin);
    at_the_armoury(&mut world, 0);
    at_the_armoury(&mut twin, 0);
    let fetch = Command::Fetch {
        slot: 0,
        who: 0,
        kind: crate::FetchKind::Piece(1),
    };
    world.step(&[fetch]);
    twin.step(&[fetch]);
    assert_eq!(world.checksum(), twin.checksum());
    // The helm lies two by four at the top left; moved to the bottom rows
    // the checksum moves with it, the twin agreeing; over the edge it is
    // refused and nothing moves.
    let to = (3 * PACK_COLS) as u32;
    let moved = Command::Repack {
        slot: 0,
        who: 0,
        cell: 1,
        to,
        turned: false,
    };
    let before = world.checksum();
    let events = world.step(&[moved]);
    assert!(!refused_with(&events, Refusal::NoRoom), "{events:?}");
    assert_ne!(world.checksum(), before);
    twin.step(&[moved]);
    assert_eq!(world.checksum(), twin.checksum());
    let piece = world.pieces.iter().find(|p| p.id == 1).unwrap();
    assert_eq!(
        piece.at,
        crate::Where::Pack {
            who: 0,
            cell: to as u8
        }
    );
    let events = world.step(&[Command::Repack {
        slot: 0,
        who: 0,
        cell: to,
        to: (PACK_COLS - 3) as u32,
        turned: false,
    }]);
    assert!(refused_with(&events, Refusal::NoRoom), "{events:?}");
    let events = world.step(&[Command::Repack {
        slot: 0,
        who: 0,
        cell: 0,
        to: 0,
        turned: false,
    }]);
    assert!(refused_with(&events, Refusal::NotAboard), "{events:?}");
}

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
/// Two worlds alike differ the moment one ticks the box, and the moment
/// one has a tier-two gun in the hold the other has not.
#[test]
fn the_checksum_notices_every_kind_of_change() {
    // --- the_checksum_notices_a_world_that_has_moved ---
    {
        let mut world = basic();
        let was = world.checksum();
        world.step(&[]);
        assert_ne!(world.checksum(), was, "a step should show");

        let mut spent = basic();
        spent.money -= 1;
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

    // --- the_checksum_notices_a_worn_piece ---
    {
        use bims::health::Part;
        use shipdesign::fixture::playtest_ship;
        let mut world = simulation_world(playtest_ship(), data::SIMULATION_MONEY, 1);
        let mut twin = simulation_world(playtest_ship(), data::SIMULATION_MONEY, 1);
        // Empty packs, so the helm fetched lies in cell 0.
        without_dressings(&mut world);
        without_dressings(&mut twin);
        assert_eq!(world.checksum(), twin.checksum());
        at_the_armoury(&mut world, 0);
        at_the_armoury(&mut twin, 0);
        let fetch = Command::Fetch {
            slot: 0,
            who: 0,
            kind: crate::FetchKind::Piece(1),
        };
        world.step(&[fetch]);
        twin.step(&[fetch]);
        assert_eq!(world.checksum(), twin.checksum(), "the same fetch");
        let equip = Command::Equip {
            slot: 0,
            who: 0,
            cell: 0,
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
        world.aboard.room.wound(0, Part::Head, 5.0);
        world.step(&[]);
        twin.step(&[]);
        assert_ne!(world.checksum(), twin.checksum(), "one helm is dented");
    }

    // --- the_checksum_notices_a_relic (feature 106) ---
    {
        use shipdesign::fixture::playtest_ship;
        let mut world = simulation_world(playtest_ship(), data::SIMULATION_MONEY, 1);
        let twin = simulation_world(playtest_ship(), data::SIMULATION_MONEY, 1);
        assert_eq!(world_checksum(&world), world_checksum(&twin));
        world.give_relic_for_probe(0, crate::relic::Relic::FocusingLens);
        assert_ne!(world_checksum(&world), world_checksum(&twin), "a relic held");
    }

    // --- the_checksum_notices_a_tier ---
    {
        use bims::combat::{Tier, WeaponKind};
        use shipdesign::fixture::playtest_ship;
        let mut world = simulation_world(playtest_ship(), data::SIMULATION_MONEY, 1);
        let mut twin = simulation_world(playtest_ship(), data::SIMULATION_MONEY, 1);
        assert_eq!(world.checksum(), twin.checksum());
        world.step(&[Command::SetAutoUpgrade { slot: 0, on: true }]);
        twin.step(&[]);
        assert_ne!(world.checksum(), twin.checksum(), "one ticked the box");
        twin.step(&[Command::SetAutoUpgrade { slot: 0, on: true }]);
        world.step(&[]);
        assert_eq!(world.checksum(), twin.checksum());
        // The same count, a different tier.
        for w in [&mut world, &mut twin] {
            w.ship.design.cargo[ResourceId::Handgun as usize] += 1;
        }
        world.guns.push(WeaponKind::LaserPistol.at(Tier::Two));
        world.on_ship_changed();
        twin.on_ship_changed();
        assert_eq!(
            world.ship.design.carrying(ResourceId::Handgun),
            twin.ship.design.carrying(ResourceId::Handgun)
        );
        assert_ne!(world.checksum(), twin.checksum(), "one pistol is tier two");
    }
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
        let empty = galaxy
            .stars
            .iter()
            .find(|s| galaxy.system(s.id).unwrap().stations.is_empty())
            .expect("most stars have no station")
            .id;
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
            start(empty, 0).err(),
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
/// systems a lane away can be quoted on its engines, fed flat out, or the
/// playtest is a ship that cannot leave the dock.
#[test]
fn the_playtest_ship_can_travel_somewhere_from_the_simulation_spawn() {
    use shipdesign::fixture::playtest_ship;
    let world = simulation_world(playtest_ship(), data::SIMULATION_MONEY, 1);
    assert_eq!(world.money, data::SIMULATION_MONEY);
    assert_eq!(world.ship.crew_count, 1);
    assert!(matches!(world.ship.state, ShipState::Docked { .. }));
    assert_eq!(world.ship.dynamics.forward_throttle, 1.0);
    assert_eq!(
        world.ship.dynamics.forward_power,
        shipdesign::parts::ENGINE_POWER
    );
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
        assert!(quote.minutes > 0, "{site:?} is no trip at all");
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

/// The room aboard is the room: left to themselves for a game day the
/// crew walk about, and the deck they walk is the design's — nobody ends
/// up standing in a wall or off the ship.
#[test]
fn the_crew_live_aboard() {
    use shipdesign::fixture::playtest_ship;
    let mut world = simulation_world(playtest_ship(), data::SIMULATION_MONEY, 1);
    assert_eq!(world.aboard.crew_count(), 1);
    let start = world.aboard.position(0);
    let mut moved = false;
    let mut farthest = 0.0f64;
    let tile = shipdesign::parts::TILE as f64;
    // Six game hours: long enough to be hungry, cook and eat. The deck is
    // the room's — docked, the ship's and the station's together — and
    // the Bim may well wander across to the station.
    for _ in 0..(6 * 60 * 60) {
        world.step(&[]);
        let at = world.aboard.position(0);
        let d = at.distance(start);
        farthest = farthest.max(d);
        if d > tile {
            moved = true;
        }
        assert!(
            world.aboard.on_deck(0),
            "the Bim is off the deck at {at:?} after {} minutes",
            world.clock_minutes
        );
    }
    assert!(moved, "the Bim never went anywhere; farthest {farthest}");
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
    use crate::station::{Plan, layout};
    use shipdesign::{design_hash, exposure, has_errors, validate};
    for plan in Plan::ALL {
        for kind in worldgen::StationKind::ALL {
            for seed in [1u64, 7, 0x_5749_4e44_4f57_0001, u64::MAX] {
                let design = layout(kind, plan, seed);
                assert_eq!(design.build_area, plan.side(kind));
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
                if kind != worldgen::StationKind::Derelict {
                    assert!(
                        exposure(&design).is_empty(),
                        "{plan:?} {kind:?} lets the radiation in"
                    );
                }
                // The one desk the key sits on, and — on the five newer
                // plans; a hub outpost's two bunks are its two residents'
                // — two beds to spare for mercenaries for hire beyond the
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

/// The plan is the seed's, evenly and fixed: over a galaxy's worth of
/// stations every one of the six comes up, the same seed rolls the same
/// plan, and the plans differ in what the user asked them to differ in —
/// size, corridors and how many live there. The spawn is a hub whatever
/// it rolled, and every other station of its system is what it rolled.
#[test]
fn a_station_s_plan_is_rolled_off_its_seed_and_the_spawn_is_a_hub() {
    use crate::station::{Plan, Station};
    use worldgen::{Galaxy, StationKind};
    let galaxy = Galaxy::new(data::DEFAULT_SEED, GalaxyType::SpiralTwoArm);
    let mut seen = std::collections::BTreeMap::new();
    for star in 0..(galaxy.stars.len() as u32).min(40) {
        let Some(system) = galaxy.system(star) else {
            continue;
        };
        for station in Station::all_of(&system) {
            assert_eq!(station.plan, Plan::rolled(station.map_seed));
            assert_eq!(station.design.build_area, station.plan.side(station.kind));
            *seen.entry(station.plan).or_insert(0u32) += 1;
        }
    }
    for plan in Plan::ALL {
        assert!(
            seen.get(&plan).copied().unwrap_or(0) > 0,
            "{plan:?} never comes up"
        );
    }
    // Size, corridors and crew: no two plans agree on all three, and the
    // pod is the smallest with the fewest, the hub the widest.
    let kind = StationKind::Orbital;
    let signature = |p: Plan| (p.side(kind), p.corridor(), p.residents(kind));
    for a in Plan::ALL {
        for b in Plan::ALL {
            assert!(a == b || signature(a) != signature(b), "{a:?} and {b:?}");
        }
    }
    assert!(
        Plan::ALL
            .iter()
            .all(|&p| p.side(kind) >= Plan::Pod.side(kind))
    );
    assert!(
        Plan::ALL
            .iter()
            .all(|&p| p.residents(kind) >= Plan::Pod.residents(kind))
    );
    assert!(
        Plan::ALL
            .iter()
            .all(|&p| p.corridor() <= Plan::Hub.corridor())
    );
    // Nobody lives on a derelict on any plan; a relay houses one.
    for plan in Plan::ALL {
        assert_eq!(plan.residents(StationKind::Derelict), 0);
        assert_eq!(plan.residents(StationKind::Relay), 1);
    }
    // The spawn is the hub whatever it rolled; the rest of its system is
    // what it rolled.
    let world = basic();
    let home = world.station(world.home).unwrap();
    assert_eq!(home.plan, Plan::Hub);
    assert_eq!(home.design.build_area, crate::station::side_of(home.kind));
    for station in &world.stations {
        if station.id != world.home {
            assert_eq!(station.plan, Plan::rolled(station.map_seed));
        }
    }
    assert!(
        world.stations.iter().any(|s| s.plan != Plan::Hub),
        "the spawn system should have a station on another plan"
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
            Plan::ALL
                .iter()
                .enumerate()
                .map(|(i, &plan)| (plan, kinds[i % kinds.len()]))
                .collect()
        };
        for (plan, kind) in plans {
            // The seed decides how many bays, shelves and batteries and the
            // holes in a derelict.
            let seeds: &[u64] = match (sweep, plan) {
                (false, _) => &[1],
                (true, Plan::Hub) => &[1, 7, 0x_5749_4e44_4f57_0001],
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
        .map(|s| {
            (
                s.id,
                s.centre(),
                s.radius(),
                s.residents() + world.mercenaries_of(s),
            )
        })
        .expect("the spawn is a station somebody lives on");
    // The count is the residents and whoever is for hire beside them.
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
    // And whoever is for hire lives in the same room.
    let ashore_count = residents + world.mercenaries_of(world.station(station).unwrap());
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
        assert!(
            world.aboard.room.send_for_probe(0, helm),
            "no route to the helm to begin with"
        );
        // Door 0 is the bridge bulkhead's, the first the design lays, and it is
        // the whole two-tile doorway: locked, the bridge is sealed.
        assert!(!world.aboard.room.ship_door_is_locked(0));
        world.aboard.room.order_door(0, 0, bims::door::Order::Lock);
        let mut budget = 60 * 60 * 5;
        while budget > 0 && !world.aboard.room.ship_door_is_locked(0) {
            world.step(&[]);
            budget -= 1;
        }
        assert!(
            world.aboard.room.ship_door_is_locked(0),
            "the door never got locked"
        );
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
    use shipdesign::fixture::playtest_ship;
    use shipdesign::parts::TILE;
    let tile = TILE as f32;

    // The playtest ship's bay lies east–west and is worked from the north.
    let layout = layout_of(&playtest_ship());
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
    let budget = Budget::new(1_000_000);
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
    design = apply(
        &design,
        &budget,
        Edit::Place {
            kind: PartKind::HydroBay,
            origin: (2, 2),
            rotation: Rotation::R90,
        },
    )
    .expect("a turned bay");
    let layout = layout_of(&design);
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
/// --nocapture`; the kind, the plan and the seed are the lines below, or
/// `BIMS_NAV_MAP=Ring` picks the plan by name.
#[test]
#[ignore]
fn nav_map_of_a_station() {
    use crate::station::Plan;
    let (kind, seed) = (worldgen::StationKind::Relay, 1u64);
    let plan = std::env::var("BIMS_NAV_MAP")
        .ok()
        .and_then(|name| {
            Plan::ALL
                .into_iter()
                .chain([Plan::Surface])
                .find(|p| format!("{p:?}") == name)
        })
        .unwrap_or(Plan::Hub);
    let design = crate::station::layout(kind, plan, seed);
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

// --- power -------------------------------------------------------------------

/// The flyer has no battery, so its charge is nought and stays there; it
/// makes more than it draws, so nothing is browned out. Put a battery on a
/// wired tile and it arrives empty and fills at the surplus; put enough
/// on the run to draw more than the reactor makes and it drains at the
/// deficit, and when it is flat the bay stops and life support does not.
/// Every figure is closed form off the step count, which is what a server
/// catching up relies on.
#[test]
fn the_batteries_fill_at_the_surplus_drain_at_the_deficit_and_flat_is_a_brownout() {
    use shipdesign::{BATTERY_CHARGE, REACTOR_OUTPUT};
    let budget = Budget::new(10_000_000);
    let mut world = basic();
    let power = world.power();
    assert_eq!(power.supply, REACTOR_OUTPUT);
    // The flyer's cold store, helm, bay and array — thirty-five — and its
    // four wall lights and standing light, a hundred and forty.
    assert_eq!(power.draw, 175.0);
    assert_eq!(power.storage, 0.0);
    assert_eq!(power.charge, 0.0);
    assert!(!power.brownout());
    assert!(world.powered(PartKind::HydroBay));
    assert!(
        !world.powered(PartKind::LifeSupport),
        "the flyer has none, so none is running"
    );
    world.step(&[]);
    assert_eq!(world.power().charge, 0.0);

    // A battery on the spine, which runs down column 8: it arrives empty.
    let with_battery = apply(
        &world.ship.design,
        &budget,
        Edit::Place {
            kind: PartKind::Battery,
            origin: (8, 7),
            rotation: Rotation::R0,
        },
    )
    .expect("a battery on the spine");
    world.ship.design = with_battery;
    world.on_ship_changed();
    assert_eq!(world.power().storage, BATTERY_CHARGE);
    assert_eq!(world.power().charge, 0.0);

    // The surplus — the reactor less the flyer's hundred and seventy-five
    // — for ten steps.
    let surplus = REACTOR_OUTPUT - 175.0;
    for _ in 0..10 {
        world.step(&[]);
    }
    let expected = surplus * 10.0 * data::STEP_MINUTES;
    assert!(
        close(world.power().charge, expected),
        "{} vs {expected}",
        world.power().charge
    );

    // Run it full, and it stops at full.
    let steps_to_full = (BATTERY_CHARGE / (surplus * data::STEP_MINUTES)).ceil() as u32 + 5;
    for _ in 0..steps_to_full {
        world.step(&[]);
    }
    assert_eq!(world.power().charge, BATTERY_CHARGE);

    // A world opened on this design opens full, as one docked would be.
    let fresh = world_with(world.ship.design.clone(), REFERENCE_MONEY, 2);
    assert_eq!(fresh.power().charge, BATTERY_CHARGE);

    // Four life supports along the spine and a second array on it draw
    // ninety more: two hundred and sixty-five against the reactor, and
    // the battery drains at the difference.
    let mut design = world.ship.design.clone();
    design = apply(
        &design,
        &budget,
        Edit::Place {
            kind: PartKind::SensorArray,
            origin: (9, 13),
            rotation: Rotation::R0,
        },
    )
    .expect("an array on the spine");
    for y in [4u32, 8, 11, 13] {
        design = apply(
            &design,
            &budget,
            Edit::Place {
                kind: PartKind::LifeSupport,
                origin: (7, y),
                rotation: Rotation::R0,
            },
        )
        .unwrap_or_else(|e| panic!("life support at (7, {y}): {e:?}"));
    }
    world.ship.design = design;
    world.on_ship_changed();
    assert_eq!(world.power().draw, 265.0);
    // A basic reactor makes two and a half thousand, which nothing on a
    // twenty-tile ship out-draws, so the reactor is held down to five
    // under the draw for the deficit to exist.
    let reactor = 260.0;
    world.throttle_reactors_for_probe(reactor);
    let deficit = 265.0 - reactor;
    assert!(deficit > 0.0);
    assert!(!world.power().brownout());
    assert!(world.powered(PartKind::HydroBay));
    for _ in 0..10 {
        world.step(&[]);
    }
    let expected = BATTERY_CHARGE - deficit * 10.0 * data::STEP_MINUTES;
    assert!(
        close(world.power().charge, expected),
        "{} vs {expected}",
        world.power().charge
    );

    // Flat: the bay and the cold store stop, life support carries on. The
    // last of the charge is skipped rather than drained — at five a minute
    // that is twelve game hours — and three steps' worth is left to go.
    world.ship.charge = deficit * data::STEP_MINUTES * 3.0;
    for _ in 0..10 {
        world.step(&[]);
    }
    assert_eq!(world.power().charge, 0.0);
    assert!(world.power().brownout());
    assert!(!world.powered(PartKind::HydroBay));
    assert!(!world.powered(PartKind::ColdStore));
    assert!(world.powered(PartKind::LifeSupport));
    assert!(
        world.powered(PartKind::Table),
        "a table draws nothing and is always running"
    );

    // Taking the battery off takes what is in it — here nothing — and the
    // charge cannot exceed what is left to hold it.
    let battery = world
        .ship
        .design
        .parts
        .iter()
        .find(|p| p.kind == PartKind::Battery)
        .unwrap()
        .id;
    world.ship.charge = 100.0;
    world.ship.design = apply(
        &world.ship.design,
        &budget,
        Edit::Remove { part_id: battery },
    )
    .unwrap();
    world.on_ship_changed();
    assert_eq!(world.power().charge, 0.0);
}

/// A consumer standing off the run is not running, brownout or no.
#[test]
fn a_consumer_off_the_run_is_not_running() {
    let budget = Budget::new(10_000_000);
    let mut world = basic();
    // Life support on bare deck to port, between the bay and the aft
    // lamps' run, nowhere near the conduit.
    world.ship.design = apply(
        &world.ship.design,
        &budget,
        Edit::Place {
            kind: PartKind::LifeSupport,
            origin: (4, 12),
            rotation: Rotation::R0,
        },
    )
    .expect("life support on bare deck");
    world.on_ship_changed();
    // The only life support aboard is off the run, so none is running.
    assert!(!world.powered(PartKind::LifeSupport));
    assert!(world.powered(PartKind::HydroBay));
    assert_eq!(
        world.power().draw,
        175.0,
        "an unwired consumer draws nothing"
    );
}

/// The ship's lamps on the crew's deck, index for index with the light
/// parts of the design, by tile.
fn ship_lamps(world: &World) -> Vec<((u32, u32), bims::sight::Lamp)> {
    world
        .ship
        .design
        .parts
        .iter()
        .filter(|p| shipdesign::is_light(p.kind))
        .map(|p| {
            let t = shipdesign::TILE as f32;
            let at = bims::math::vec2(
                (p.origin.0 as f32 + 0.5) * t + world.aboard.offset.x as f32,
                (p.origin.1 as f32 + 0.5) * t + world.aboard.offset.y as f32,
            );
            let (_, lamp) = world
                .aboard
                .room
                .lamp_at(at)
                .unwrap_or_else(|| panic!("no lamp at {:?}", p.origin));
            (p.origin, *lamp)
        })
        .collect()
}

/// The whole of what a brownout does, on the playtest ship with its
/// battery emptied under an overdraw: the lamps go dark and whole and the
/// benches stop, while the doors and life support run on. Said once each
/// way. Then the reactors cover the draw again, and the lamps come back.
#[test]
fn a_brownout_darkens_the_ship_and_stops_the_benches_until_the_power_is_back() {
    use shipdesign::fixture::playtest_ship;
    let mut world = simulation_world(playtest_ship(), data::SIMULATION_MONEY, 1);
    let lamps = ship_lamps(&world);
    assert_eq!(lamps.len(), 7);
    assert!(lamps.iter().all(|(_, l)| l.powered && !l.is_dark()));
    assert!(world.powered(PartKind::DrugLab));
    assert!(world.powered(PartKind::ColdStore));

    // The reactors held under the draw: the battery drains, and the ship
    // is not browned out.
    let draw = world.power().draw;
    assert_eq!(draw, 287.0);
    world.throttle_reactors_for_probe(draw - 27.0);
    let events = world.step(&[]);
    assert!(!events.iter().any(|e| matches!(e, WorldEvent::Brownout)));
    assert!(!world.power().brownout());

    // Flat. The step says so once, and everything optional stops.
    world.ship.charge = 0.0;
    let events = world.step(&[]);
    assert!(
        events.iter().any(|e| matches!(e, WorldEvent::Brownout)),
        "{events:?}"
    );
    assert!(world.power().brownout());
    let events = world.step(&[]);
    assert!(!events.iter().any(|e| matches!(e, WorldEvent::Brownout)));
    let lamps = ship_lamps(&world);
    assert!(
        lamps
            .iter()
            .all(|(_, l)| !l.powered && l.is_dark() && !l.is_out()),
        "dark and whole: {lamps:?}"
    );
    assert!(!world.powered(PartKind::HydroBay));
    assert!(!world.powered(PartKind::ColdStore));
    assert!(!world.powered(PartKind::DrugLab));
    assert!(!world.powered(PartKind::Workbench));
    assert!(world.powered(PartKind::Door));
    assert!(world.powered(PartKind::LifeSupport));
    // A target for medkits and the vegetables for one aboard, and still
    // no order: the drug lab is dark.
    world.set_craft_target(ResourceId::Medkit, 999);
    assert!(world.craft_orders().is_empty());

    // The reactors back over the draw: said once, and the lamps lit.
    world.throttle_reactors_for_probe(2.0 * shipdesign::REACTOR_OUTPUT);
    let events = world.step(&[]);
    assert!(
        events
            .iter()
            .any(|e| matches!(e, WorldEvent::PowerRestored)),
        "{events:?}"
    );
    assert!(!world.power().brownout());
    assert!(
        ship_lamps(&world)
            .iter()
            .all(|(_, l)| l.powered && !l.is_dark())
    );
    assert!(world.powered(PartKind::ColdStore));
    assert!(world.powered(PartKind::DrugLab));
}

/// An overdraw the battery covers costs nothing: an hour and more of the
/// reactors under the draw with charge to spare is no brownout and no
/// dark — only a battery that much emptier.
/// A lamp on no live network is dark from the first step — whole, and
/// nothing to do with any brownout — while the wired lamps beside it are
/// lit.
#[test]
fn a_short_overdraw_a_battery_covers_costs_nothing_and_an_unwired_lamp_is_dark() {
    // --- a_short_overdraw_a_battery_covers_costs_nothing ---
    {
        use shipdesign::fixture::playtest_ship;
        let mut world = simulation_world(playtest_ship(), data::SIMULATION_MONEY, 1);
        let charge = world.power().charge;
        assert!(charge > 0.0);
        let deficit = 27.0;
        world.throttle_reactors_for_probe(world.power().draw - deficit);
        // An hour and a half.
        let steps = (1.5 * time::HOUR / data::STEP_MINUTES) as u64;
        for _ in 0..steps {
            let events = world.step(&[]);
            assert!(
                !events
                    .iter()
                    .any(|e| matches!(e, WorldEvent::Brownout | WorldEvent::PowerRestored)),
                "{events:?}"
            );
        }
        assert!(!world.power().brownout());
        assert!(
            close(
                world.power().charge,
                charge - deficit * steps as f64 * data::STEP_MINUTES
            ),
            "{} of {charge}",
            world.power().charge
        );
        assert!(
            ship_lamps(&world)
                .iter()
                .all(|(_, l)| l.powered && !l.is_dark())
        );
    }

    // --- an_unwired_lamp_is_dark ---
    {
        use shipdesign::fixture::playtest_ship;
        let budget = Budget::new(10_000_000);
        // A standing light on the main deck's bare tiles, away from every run.
        let design = apply(
            &playtest_ship(),
            &budget,
            Edit::Place {
                kind: PartKind::StandingLight,
                origin: (12, 9),
                rotation: Rotation::R0,
            },
        )
        .expect("a standing light on bare deck");
        assert!(!shipdesign::is_powered(
            &design,
            design.parts.last().unwrap().id
        ));
        let world = simulation_world(design, data::SIMULATION_MONEY, 1);
        assert!(!world.power().brownout());
        let lamps = ship_lamps(&world);
        assert_eq!(lamps.len(), 8);
        for (tile, lamp) in &lamps {
            if *tile == (12, 9) {
                assert!(
                    !lamp.powered && lamp.is_dark() && !lamp.is_out(),
                    "{lamp:?}"
                );
            } else {
                assert!(lamp.powered && !lamp.is_dark(), "{tile:?} {lamp:?}");
            }
        }
    }
}

// --- making things -----------------------------------------------------------

/// The whole seam, on the playtest ship: a target for medkits,
/// vegetables aboard, the drug lab wired — the room is handed an order, a
/// Bim walks to the bench and stands at it for the recipe's length, and
/// the world moves two vegetables out of the hold and one medkit in, with
/// the mass moving with them. Then the target is met and nothing more is
/// made.
///
/// It was ore into metal at the smelter until the money rework (feature
/// 95) left one recipe in the table.
#[test]
fn a_target_for_a_medkit_has_a_bim_make_one_at_the_drug_lab() {
    use bims::game::JOB_CRAFT;
    use shipdesign::fixture::playtest_ship;
    let mut world = simulation_world(playtest_ship(), data::SIMULATION_MONEY, 1);
    // The dressings every Bim carries are out of the way (feature 87).
    without_dressings(&mut world);
    let veg = world.ship.design.carrying(ResourceId::Vegetable);
    let kits = world.ship.design.carrying(ResourceId::Medkit);
    assert!(veg >= 2, "the playtest ship carries vegetables");
    let mass = world.ship.design.manifest();
    let before: f64 = mass
        .iter()
        .map(|&(id, units)| units as f64 * id.mass_per_unit())
        .sum();

    world.set_craft_target(ResourceId::Medkit, kits + 1);
    assert_eq!(world.craft_orders().len(), 1, "the drug lab is asked for");
    let mut made = false;
    let mut stood = false;
    for _ in 0..(4 * 60 * 60) {
        let events = world.step(&[]);
        stood |= world.aboard.room.activity(0) == JOB_CRAFT;
        if events
            .iter()
            .any(|e| matches!(e, WorldEvent::Crafted { recipe: 0 }))
        {
            made = true;
            break;
        }
    }
    assert!(made, "no medkit was made in four hours");
    assert!(stood, "nobody stood at the bench");
    assert_eq!(world.ship.design.carrying(ResourceId::Medkit), kits + 1);
    assert_eq!(world.ship.design.carrying(ResourceId::Vegetable), veg - 2);
    // Crafting conserves mass: two vegetables at a half weigh what one
    // medkit does.
    let after: f64 = world
        .ship
        .design
        .manifest()
        .iter()
        .map(|&(id, units)| units as f64 * id.mass_per_unit())
        .sum();
    assert!((after - before).abs() < 1e-9, "{before} became {after}");
    // The target met, nothing more is asked for.
    assert!(world.craft_orders().is_empty());
}

/// A recipe wants its inputs and its station's power. With no vegetables
/// aboard there is no order for a medkit however high the target; with
/// the drug lab unpowered, none either.
#[test]
fn an_order_wants_the_inputs_aboard_and_the_bench_powered() {
    use shipdesign::fixture::playtest_ship;
    let budget = Budget::new(10_000_000);
    let mut world = simulation_world(playtest_ship(), data::SIMULATION_MONEY, 1);
    let veg = world.ship.design.carrying(ResourceId::Vegetable);
    world.ship.design = apply(
        &world.ship.design,
        &budget,
        Edit::Sell {
            resource: ResourceId::Vegetable,
            units: veg,
        },
    )
    .expect("the food sold");
    world.on_ship_changed();
    world.set_craft_target(ResourceId::Medkit, 10);
    assert!(
        world.craft_orders().is_empty(),
        "nothing to make a medkit of"
    );

    // The vegetables back, and the order is there.
    world.ship.design.cargo[ResourceId::Vegetable as usize] = 10;
    world.on_ship_changed();
    assert_eq!(world.craft_orders().len(), 1);

    // Both reactors off the run and the lab is dark: no order.
    let reactors: Vec<u32> = world
        .ship
        .design
        .parts
        .iter()
        .filter(|p| p.kind == PartKind::Reactor)
        .map(|p| p.id)
        .collect();
    for id in reactors {
        world.ship.design = apply(&world.ship.design, &budget, Edit::Remove { part_id: id })
            .expect("the reactor came off");
    }
    world.on_ship_changed();
    assert!(
        world.craft_orders().is_empty(),
        "a dark bench is no bench at all"
    );
}

#[test]
fn a_bot_never_stands_at_a_bench_and_a_player_s_bim_does() {
    use bims::game::JOB_CRAFT;
    use bims::work::Job;
    use shipdesign::fixture::playtest_ship;
    let mut world = crewed_world(playtest_ship(), data::SIMULATION_MONEY, 1, 2);
    without_dressings(&mut world);
    let kits = world.ship.design.carrying(ResourceId::Medkit);
    world.set_craft_target(ResourceId::Medkit, kits + 1);
    assert_eq!(world.craft_orders().len(), 1, "the drug lab is asked for");
    // One step to hand the order over, and the room says who it is for.
    world.step(&[]);
    let room = &world.aboard.room;
    assert_eq!(room.crew_count(), 2);
    assert!(
        room.work_on_offer_for_probe(0).contains(&Job::Craft.code()),
        "the player's own is offered the bench"
    );
    assert!(
        !room.work_on_offer_for_probe(1).contains(&Job::Craft.code()),
        "and the bot is not"
    );
    // It is the bench and nothing else: the crewmate is still offered
    // everything it was, which on a ship at its berth is the deck, the
    // bay and the galley as they come round.
    let bot_rows = room.work_on_offer_for_probe(1);
    assert!(
        bot_rows.iter().all(|&job| job != Job::Craft.code()),
        "{bot_rows:?}"
    );

    // And over a morning the making is slot 0's from end to end.
    let mut crafted = false;
    for _ in 0..(4 * 60 * 60) {
        let events = world.step(&[]);
        assert_ne!(
            world.aboard.room.activity(1),
            JOB_CRAFT,
            "the crewmate never stood at a bench"
        );
        if events
            .iter()
            .any(|e| matches!(e, WorldEvent::Crafted { recipe: 0 }))
        {
            crafted = true;
            break;
        }
    }
    assert!(crafted, "no medkit was made in four hours");
    assert_eq!(world.ship.design.carrying(ResourceId::Medkit), kits + 1);
}

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
    let mut world = simulation_world(playtest_ship(), data::SIMULATION_MONEY, 1);
    world.set_shipyard_enabled(true);
    // The dressings every Bim carries are out of the way (feature 87).
    without_dressings(&mut world);
    let money = world.money;
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
    assert_eq!(world.money, money, "nothing spent yet");

    // The crew walk over and put it together. Nothing is hauled — the
    // one errand is the build.
    let mut built = false;
    let mut stood = false;
    for _ in 0..(6 * 60 * 60) {
        let events = world.step(&[]);
        stood |= world.aboard.room.activity(0) == JOB_BUILD;
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
    assert_eq!(world.money, money - price);
    let want = mass + shipdesign::part_mass(PartKind::Wall);
    assert!(
        (world.ship.dynamics.mass.get() - want).abs() < 1e-6,
        "{} is not {want}",
        world.ship.dynamics.mass.get()
    );
    // The room came with it: the crew are still aboard and still going
    // about their day.
    assert_eq!(world.aboard.count(), 1);
}

/// A site the crew cannot pay for **waits**, with
/// [`Refusal::NotEnoughMoney`] where the part would otherwise go down,
/// and the money earned since lets it go on (feature 95). And two sites
/// the pool covers only one of are one at a time: `free_money` is the
/// pool less the sites already begun.
#[test]
fn a_site_waits_while_the_pool_cannot_cover_it() {
    use shipdesign::fixture::playtest_ship;
    let mut world = simulation_world(playtest_ship(), data::SIMULATION_MONEY, 1);
    world.set_shipyard_enabled(true);
    without_dressings(&mut world);
    let price = PartKind::Wall.def().price;
    // Just short of one wall.
    world.money = price - 1;
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
    world.money = price * 4;
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
    assert_eq!(world.money, price * 3);
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
    let mut world = simulation_world(playtest_ship(), data::SIMULATION_MONEY, 1);
    world.set_shipyard_enabled(true);
    world.undock_for_probe();
    without_dressings(&mut world);
    let money = world.money;
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
        outside |= world.aboard.room.is_outside(0);
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
    assert_eq!(world.money, money - price, "the deck and its frame");
}

/// A blueprint nobody has walked to holds nothing: it is not under
/// construction and its price is nobody's yet. Once somebody is on the
/// way to it, it is — and its price is spoken for (feature 95) — and a
/// cancelled site frees what it had claimed, the Bim on the way turning
/// back.
#[test]
fn a_site_is_begun_once_somebody_is_on_the_way_and_a_cancel_frees_its_price() {
    use shipdesign::fixture::playtest_ship;
    let mut world = simulation_world(playtest_ship(), data::SIMULATION_MONEY, 1);
    world.set_shipyard_enabled(true);
    let money = world.money;
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
    assert_eq!(world.money, money);
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
/// station's rooms beyond its bulkheads, and the station's people are
/// drawn only where the crew can see them. Away from the berth the
/// station's room shows nobody at all.
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

    // The residents: drawn exactly where the crew's trace says they are
    // seen, and — with the crew on their own deck — not all of them.
    world.step(&[]);
    let ashore = world.residents.as_ref().unwrap();
    let positions: Vec<DVec2> = (0..ashore.aboard.count())
        .map(|who| ashore.aboard.position(who))
        .collect();
    let seen = world.aboard.seen(&positions);
    for (who, &s) in seen.iter().enumerate() {
        assert_eq!(ashore.aboard.room.body_seen(who), s, "resident {who}");
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

/// A stranger's structure is black where the crew have never looked,
/// grey where they have and see nothing now, and its people are drawn
/// only in sight and a moment after; the crew's own ship stays under the
/// dim fog it always had. Docked at a station that is not home — the
/// spawn is, so it is told otherwise — the joined deck has both.
#[test]
fn a_stranger_s_deck_is_black_where_nobody_has_looked_and_grey_where_they_have() {
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
    world.aboard.room.observe();

    // The middle of the station, well out of view: black. Somewhere on
    // the ship the crew do not see: the dim fog, or nothing at all.
    let station = world.station(station_id).unwrap().clone();
    let side = station.design.build_area as f64 * shipdesign::TILE as f64;
    let (origin, ex, ey) = world.aboard.station_frame.unwrap();
    let at = origin.add(ex.scale(side / 2.0)).add(ey.scale(side / 2.0));
    assert_eq!(
        world.aboard.room.veil_at(at.x as f32, at.y as f32),
        3,
        "black"
    );
    // Nothing is grey on the first look: what is not black is in view,
    // and there is no ring round it — walk in from the station's middle
    // towards the ship's port, and every tile on the way is black, seen,
    // or the ship's own.
    let james = world.aboard.room.bim_pos(0);
    for i in 0..200 {
        let t = i as f32 / 200.0;
        let x = at.x as f32 + (james.x - at.x as f32) * t;
        let y = at.y as f32 + (james.y - at.y as f32) * t;
        let veil = world.aboard.room.veil_at(x, y);
        assert_ne!(veil, 2, "grey before anything has been looked at");
    }
    // And the ship's own tiles are never black or grey.
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
    assert!(
        ship_tiles
            .iter()
            .all(|&(x, y)| world.aboard.room.veil_at(x, y) <= 1)
    );
    // Once looked at, a tile is grey when nobody sees it any more: an eye
    // stood at the station's middle sees the tile under it, and with
    // every line of sight shut again — a trace from nowhere, then the
    // crew's own from the ship — it is grey, remembered, not black.
    let middle = (at.x as f32, at.y as f32);
    world
        .aboard
        .room
        .observe_from_for_probe(&[bims::math::vec2(middle.0, middle.1)]);
    assert_eq!(world.aboard.room.veil_at(middle.0, middle.1), 0, "seen");
    world.aboard.room.observe_from_for_probe(&[]);
    assert_eq!(
        world.aboard.room.veil_at(middle.0, middle.1),
        2,
        "remembered: grey"
    );
    world.aboard.room.observe();
    assert_eq!(world.aboard.room.veil_at(middle.0, middle.1), 2);

    // The machines: one seen stays drawn for a moment after it is out of
    // view, and no longer.
    world.step(&[]);
    let ashore = world.residents.as_mut().unwrap();
    // A wave of them, not the two residents.
    let crowd = ashore.aboard.count() as usize;
    assert!(crowd >= 2);
    assert_eq!(ashore.aboard.room.crew_count(), 0, "and no people");
    let mut seen = vec![false; crowd];
    seen[0] = true;
    ashore.aboard.room.set_seen(&seen);
    assert!(ashore.aboard.room.body_seen(0));
    assert!(!ashore.aboard.room.body_seen(1));
    ashore.aboard.room.set_seen(&vec![false; crowd]);
    assert!(
        ashore.aboard.room.body_seen(0),
        "just out of view: still drawn"
    );
    let steps = (bims::game::SEEN_FOR / (data::STEP_MINUTES / time::MINUTES_PER_SECOND) as f32)
        .ceil() as u32
        + 2;
    for _ in 0..steps {
        world.step(&[]);
        let ashore = world.residents.as_mut().unwrap();
        ashore.aboard.room.set_seen(&vec![false; crowd]);
    }
    assert!(!world.residents.as_ref().unwrap().aboard.room.body_seen(0));
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
    assert!(gear.head.is_none() && gear.body.is_none() && gear.legs.is_none());
    let stats = world.aboard.room.weapon_stats(0).unwrap();
    assert_eq!(stats.dps(), stats.fire_rate * stats.damage);
    assert!(
        (stats.hit_chance(10.0) - 0.732).abs() < 0.01,
        "the pistol at ten tiles"
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
fn the_machines_shoot_back_and_a_crew_member_hit_bleeds() {
    use bims::combat::WeaponKind;
    let mut world = basic();
    assert!(world.stage_droid_fight_for_probe(
        bims::droid::DroidKind::Trooper,
        Some(WeaponKind::LaserPistol.basic())
    ));
    let station_id = world.residents.as_ref().unwrap().station;
    assert_eq!(world.stance(station_id), bims::sight::Stance::Hostile);
    assert_eq!(world.aboard.room.bleeding(0), 0);
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
    let WorldEvent::CrewHit { who, part } = hit else {
        unreachable!()
    };
    assert_eq!(who, 0);
    let part = bims::health::Part::from_code(part).expect("a real part");
    // The wound is on the body already, and it bleeds.
    assert!(world.aboard.room.bleeding(0) > 0, "bleeding");
    assert!(world.aboard.room.wounds(0, part) > 0);
    assert!(world.aboard.room.part_health(0, part) < part.max());
    assert!(world.aboard.room.health(0) < bims::health::MAX_HEALTH);
    // The event carries the part in the tens.
    assert_eq!(hit.code(), 32);
    assert_eq!(hit.value(), (10 * part.code()) as i64);
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
    // The residents, seated first; a mercenary for hire after them is
    // kitted off its own seed, and is the other test's.
    let residents = station.residents();
    assert!(residents > 0);
    assert_eq!(
        ashore.aboard.count(),
        residents + world.mercenaries_of(station)
    );
    for who in 0..residents {
        let mut issued = Gear::issued_for(seed ^ who as u64);
        assert!(issued.weapon.is_some(), "every resident carries something");
        assert_eq!(ashore.aboard.room.weapon(who as usize), issued.weapon);
        // And a couple of dressings in the pack beside it since
        // feature 87 — the roll is the weapon's and the armour's.
        let cell = issued
            .free_cell_for(bims::game::BANDAGE)
            .expect("a fresh pack has room for a box");
        issued.put_many(cell, bims::game::BANDAGE, data::RESIDENT_BANDAGES);
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
/// the claw's blow comes back the other way, a crush of one wound unit on
/// the part it landed on where a blade's cut is three. James wears the
/// kevlar, and the machine is made whole before every step, so the lock
/// is what is looked at rather than who wins — a Warden at tier two,
/// whose head takes a fist and a bolt with some to spare, so no one blow
/// ends it between two mendings.
#[test]
fn a_claw_charges_and_locks_the_crew_member_who_fights_with_its_fists() {
    use bims::combat::{ArmourKind, Gear, MELEE_RANGE, Piece, WeaponKind};
    use bims::droid::DroidKind;
    use bims::health::{CUT_WOUND, Part};
    let mut world = basic();
    world.set_droid_tier_for_probe(Some(Tier::Two));
    assert!(
        world.stage_droid_fight_for_probe(DroidKind::Warden, Some(WeaponKind::Claw.at(Tier::Two)))
    );
    world.aboard.room.issue(
        0,
        Gear {
            body: Some(Piece::new(99, ArmourKind::BasicKevlar, Tier::One)),
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

    // A blow on James: the part it landed on, and how many wound units it
    // opened there — read across the one step it landed in.
    let units = |world: &World| Part::ALL.map(|p| world.aboard.room.wounds(0, p));
    let blow = |events: &[WorldEvent], was: [u32; 3], world: &World| {
        events.iter().find_map(|e| match e {
            WorldEvent::CrewHit { who: 0, part } => {
                let part = Part::from_code(*part).unwrap();
                let i = Part::ALL.iter().position(|&p| p == part).unwrap();
                Some((part, world.aboard.room.wounds(0, part) - was[i]))
            }
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
    // say the lock again. The claw's own blow comes the other way — a
    // crush, one unit on the part it landed on, said as a hit — before or
    // after James's lock, since the machine reads its reach a step ahead
    // of him; the kevlar takes the first on the body.
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
    let (part, opened) = hit.expect("the claw landed on James");
    assert!(world.aboard.room.wounds(0, part) >= 1);
    assert!(
        opened == 1 && opened < CUT_WOUND,
        "a crush is one unit, not a cut's three: {opened}"
    );
    assert!(world.aboard.room.bleeding(0) >= 1);
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

/// A sniper rifle reaches from twenty tiles and more, where nothing else
/// does, both ways round. A **machine** built with the rifle — a Warden,
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
fn a_sniper_rifle_reaches_from_twenty_tiles_and_a_shotgun_does_more_at_three_than_at_nine() {
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
        assert!(world.aboard.room.bleeding(0) > 0, "and James bleeds for it");

        // The long shot: James with the rifle, the machine twenty-one
        // tiles down the corridor, held there and firing nothing.
        let mut world = basic();
        assert!(world.stage_droid_fight_for_probe(DroidKind::Warden, None));
        let station = world.ship.state.station().unwrap();
        let port = world.station(station).unwrap().port().unwrap();
        let tiles = 21.0;
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
            (20.0..=tiles as f32 + 1.0).contains(&hit_from),
            "from twenty tiles and more: {hit_from:.1} tiles"
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

    // --- the_crew_s_shotgun_does_more_at_three_tiles_than_at_nine ---
    {
        use bims::combat::{Gear, WeaponKind};
        let stats = WeaponKind::Shotgun.stats();
        // The first hit on the chassis, on a machine still whole: what
        // came off it — all of a Trooper's sixty up close, and less down
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
        // Down the corridor, the curve's number, which is less.
        assert!(far < near, "less at nine tiles: {far} < {near}");
        assert!(
            flown(far, far_from),
            "the weapon's damage at the distance flown: {far} at {far_from:.1} tiles"
        );
    }
}

/// A bandage is a thing in a pack since feature 87, and **a charge**
/// since the medicine became everybody's: the crew member sets out with
/// `class::BANDAGE_CHARGES` of them, dresses a wound of its own with one,
/// and the one spent comes back on the cooldown — the hold never pays
/// and never fills a pack. A treatment aboard is the same with the
/// helper's own medkit: opened where the helper stands, taken out of its
/// pack when the hands come off, and back a minute of the clock later —
/// there is no cabinet to fetch one from and the room has no shelf.
#[test]
fn a_wound_is_dressed_and_a_trauma_treated_out_of_the_helper_s_own_charges() {
    use crate::class::{self, Charge};
    // --- a_crew_member_dresses_a_wound_with_a_bandage_of_its_own ---
    {
        use bims::health::Part;
        use shipdesign::fixture::playtest_ship;
        let mut world = simulation_world(playtest_ship(), data::SIMULATION_MONEY, 1);
        let bandages = world.ship.design.carrying(ResourceId::Bandage);
        assert!(bandages > 0, "the playtest ship carries bandages");
        for _ in 0..3 {
            world.step(&[]);
        }
        assert_eq!(
            world.aboard.room.bandages_of(0),
            class::BANDAGE_CHARGES,
            "the crew member carries its charges"
        );
        assert_eq!(
            world.ship.design.carrying(ResourceId::Bandage),
            bandages,
            "and none of them came out of the hold"
        );

        assert!(
            !world.aboard.room.wound(0, Part::Body, 12.0).leg_lost,
            "no leg lost"
        );
        assert_eq!(world.aboard.room.bleeding(0), 1);
        assert!(world.aboard.room.bandage(0, 0, Part::Body));
        let mut budget = 60 * 60;
        while budget > 0 && world.aboard.room.bleeding(0) > 0 {
            world.step(&[]);
            budget -= 1;
        }
        assert_eq!(world.aboard.room.bleeding(0), 0, "the wound is closed");
        // The dressing is out of the pack, the hold as it was, and the
        // cooldown running to bring it back.
        world.step(&[]);
        assert_eq!(world.aboard.room.bandages_of(0), class::BANDAGE_CHARGES - 1);
        assert_eq!(world.ship.design.carrying(ResourceId::Bandage), bandages);
        assert!(world.charge_cooldown_left(0, Charge::Bandage) > 0.0);
        for _ in 0..(class::BANDAGE_COOLDOWN as u32 * 60 + 2) {
            world.step(&[]);
        }
        assert_eq!(world.aboard.room.bandages_of(0), class::BANDAGE_CHARGES);
        assert_eq!(world.ship.design.carrying(ResourceId::Bandage), bandages);
        // A part with nothing open on it is not worth a bandage.
        assert!(!world.aboard.room.bandage(0, 0, Part::Body));
    }

    // --- a_treatment_aboard_is_the_helper_s_own_kit_and_the_hold_never_pays ---
    {
        use bims::health::Part;
        // The combat ship: the playtest ship with bunks for two and more, and
        // the same cargo, kits included.
        use shipdesign::fixture::combat_ship;
        let mut world = simulation_world(combat_ship(), data::SIMULATION_MONEY, 2);
        assert_eq!(world.aboard.crew_count(), 2);
        let kits = world.ship.design.carrying(ResourceId::Medkit);
        assert!(kits > 0, "the ship carries medkits");
        world.step(&[]);
        assert_eq!(
            world.aboard.room.medkits(),
            0,
            "the hold's are nobody's to hand"
        );
        assert_eq!(world.charges_of(0, Charge::Medkit), class::MEDKIT_CHARGES);

        // Kate dying of a body at nothing; James, free, treats her with
        // the kit in his own pack.
        let out = world.aboard.room.wound(1, Part::Body, Part::Body.max());
        assert!(out.trauma.is_some(), "dying");
        let mut budget = 60 * 60 * 3;
        while budget > 0 && world.aboard.room.is_dying(1) {
            world.step(&[]);
            budget -= 1;
        }
        assert!(!world.aboard.room.is_dying(1), "treated within the run");
        world.step(&[]);
        assert_eq!(world.charges_of(0, Charge::Medkit), 0, "his own kit spent");
        assert_eq!(
            world.ship.design.carrying(ResourceId::Medkit),
            kits,
            "and the hold is untouched"
        );
        assert!(world.charge_cooldown_left(0, Charge::Medkit) > 0.0);
        for _ in 0..(class::MEDKIT_COOLDOWN as u32 * 60 + 2) {
            world.step(&[]);
        }
        assert_eq!(
            world.charges_of(0, Charge::Medkit),
            class::MEDKIT_CHARGES,
            "back a minute of the clock later"
        );
        assert_eq!(world.ship.design.carrying(ResourceId::Medkit), kits);
    }
}

/// A mercenary for hire at the dock: an extra body in the station's room
/// in the olive coverall, priced by its kit; hired from within reach with
/// the money in hand, it walks out of the station's room into the crew's,
/// the first month paid and the next due a month on; the month is paid
/// when a trip puts the world clock past it, and one the money will not
/// cover is owed, said once. Out of reach, broke, or one of the station's
/// own, the hire is refused.
#[test]
fn a_mercenary_is_hired_from_the_station_and_paid_by_the_month() {
    use crate::armour::{LootSource, Where};
    use crate::mercenary::MONTH;
    use bims::character::Uniform;
    use shipdesign::fixture::combat_ship;
    let galaxy = worldgen::Galaxy::new(data::DEFAULT_SEED, GalaxyType::SpiralTwoArm);
    let (star, station) = crate::spawn(&galaxy).unwrap();
    let mut world = World::start_with_crew(
        combat_ship(),
        REFERENCE_MONEY,
        1,
        1,
        data::DEFAULT_SEED,
        GalaxyType::SpiralTwoArm,
        star,
        station,
    )
    .unwrap();
    assert_eq!(world.aboard.crew_count(), 1);
    assert!(world.mercenary_for_probe());
    let residents = world.residents.as_ref().unwrap();
    let people = world.people_of(world.station(station).unwrap());
    let count = residents.aboard.count();
    assert!(count > people, "somebody extra: {count} of {people}");
    let merc = count - 1;
    let fee = world.mercenary_fee(merc).expect("the last is for hire");
    assert!(world.mercenary_fee(0).is_none(), "the first is not");
    assert!((1_700..=25_000).contains(&fee), "{fee}");
    assert_eq!(
        residents.aboard.room.uniform(merc as usize),
        Uniform::Mercenary
    );
    assert!(residents.hailable()[merc as usize]);
    assert!(!residents.hailable()[0]);
    let before = world_checksum(&world);

    // Out of reach: refused, and nothing moved.
    let hire = Command::Hire {
        slot: 0,
        who: 0,
        resident: merc,
    };
    let offer = world.hire_offer(0, merc).unwrap();
    assert!(offer.docked && offer.affordable && !offer.in_reach);
    let events = world.step(&[hire]);
    assert!(events.contains(&WorldEvent::Refused {
        slot: 0,
        why: Refusal::OutOfReach
    }));
    assert_eq!(world.aboard.crew_count(), 1);

    // Beside it: hired. Stood there every step, since it walks about.
    let at = world.body_position(LootSource::Resident(merc)).unwrap();
    world
        .aboard
        .room
        .put_for_probe(0, at + bims::math::vec2(30.0, 0.0));
    assert!(world.hire_offer(0, merc).unwrap().in_reach);

    // With nothing to spend it is refused, and one of the station's own
    // is nobody's to hire.
    let money = world.money;
    world.money = 0;
    let events = world.step(&[hire]);
    assert!(events.contains(&WorldEvent::Refused {
        slot: 0,
        why: Refusal::Unaffordable
    }));
    world.money = money;
    let plain = Command::Hire {
        slot: 0,
        who: 0,
        resident: 0,
    };
    let events = world.step(&[plain]);
    assert!(events.contains(&WorldEvent::Refused {
        slot: 0,
        why: Refusal::NotForHire
    }));
    assert_eq!(world.aboard.crew_count(), 1);

    let at = world.body_position(LootSource::Resident(merc)).unwrap();
    world
        .aboard
        .room
        .put_for_probe(0, at + bims::math::vec2(30.0, 0.0));
    let events = world.step(&[hire]);
    assert!(events.contains(&WorldEvent::Hired { who: 1 }), "{events:?}");
    assert_eq!(world.aboard.crew_count(), 2, "one more aboard");
    assert_eq!(world.ship.crew_count, 2);
    assert_eq!(world.money, money - fee, "the first month paid");
    assert_eq!(world.residents.as_ref().unwrap().aboard.count(), count - 1);
    assert_eq!(world.hired().len(), 1);
    assert_eq!((world.hired()[0].who, world.hired()[0].fee), (1, fee));
    assert!(world.is_hired(1) && !world.is_hired(0));
    assert_eq!(world.aboard.room.uniform(1), Uniform::Mercenary);
    assert_ne!(world_checksum(&world), before);
    // Its armour is the world's now, worn, under the world's ids.
    let worn: Vec<_> = world
        .pieces
        .iter()
        .filter(|p| matches!(p.at, Where::Worn { who: 1 }))
        .collect();
    let gear = world.aboard.room.gear(1);
    let pieces_on = [gear.head, gear.body, gear.legs].iter().flatten().count();
    assert_eq!(worn.len(), pieces_on, "every worn piece is on the list");
    for piece in worn {
        assert!(piece.id < world.next_piece);
    }
    assert!(
        world.mercenary_fee(merc).is_none(),
        "gone from the station's list"
    );

    // A month on, paid: the months fall due as a trip puts the world
    // clock past them (feature 103), the one time it moves. Both walk
    // back aboard first — whoever is outside the ship when it leaves is
    // left behind — and the ship goes somewhere else in the system.
    let trip = |world: &mut World| -> Vec<WorldEvent> {
        let gangway = world.aboard.gangway.expect("joined, the ship's side");
        let gangway = bims::math::vec2(gangway.x as f32, gangway.y as f32);
        for who in 0..world.aboard.crew_count() as usize {
            world.aboard.room.put_for_probe(who, gangway);
        }
        let left = world.leave_for_probe();
        assert!(
            !left
                .iter()
                .any(|e| matches!(e, WorldEvent::LeftBehind { .. })),
            "{left:?}"
        );
        world.clock_minutes += MONTH;
        let site = world
            .travel_quotes()
            .into_iter()
            .find(|(site, quote)| quote.is_ok() && Some(*site) != world.current_site())
            .map(|(site, _)| site)
            .expect("somewhere to go");
        world.step(&[Command::Propose {
            slot: 0,
            star: site.star,
            station: site.station,
        }])
    };
    let money = world.money;
    let events = trip(&mut world);
    assert!(
        events.contains(&WorldEvent::MercenaryPaid { who: 1, fee }),
        "{events:?}"
    );
    assert_eq!(world.money, money - fee);
    assert!(world.hired()[0].due > world.clock_minutes);
    assert_eq!(world.aboard.crew_count(), 2);

    // Broke a month later: owed, and said once. The hand sails on owed —
    // the ship is holding when a trip pays, never at a berth.
    world.money = fee - 1;
    let events = trip(&mut world);
    assert!(
        events.contains(&WorldEvent::MercenaryLeft { who: 1 }),
        "{events:?}"
    );
    assert!(world.hired()[0].owed);
    assert_eq!(world.aboard.crew_count(), 2);
    let events = trip(&mut world);
    assert!(
        !events
            .iter()
            .any(|e| matches!(e, WorldEvent::MercenaryLeft { .. })),
        "said once: {events:?}"
    );
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

/// The armoury aboard as the room's bench — told by its part code — and
/// where a Bim stands to reach into it, in the room's units.
fn armoury(world: &World) -> (usize, bims::math::Vec2) {
    use bims::game::Container;
    let room = &world.aboard.room;
    let i = (0..room.benches().len())
        .find(|&i| room.bench_part(i) == PartKind::Armoury.code())
        .expect("the playtest ship has an armoury");
    let spot = room
        .container_spot(Container::Bench(i))
        .expect("and it has a use spot");
    (i, spot)
}

/// Feature 87 had the crew fill their packs with dressings out of the
/// hold to the Management tab's number. **Nobody does any more**: a
/// bandage is everybody's charge, and an emptied pack is filled by the
/// cooldown — the player's own and the bot's alike, a dressing every
/// thirty seconds of the clock — with the hold's box left where it lies.
#[test]
fn the_packs_fill_on_the_cooldown_and_never_out_of_the_hold() {
    use shipdesign::fixture::playtest_ship;
    let mut world = crate::fixture::crewed_world(playtest_ship(), data::SIMULATION_MONEY, 1, 2);
    let aboard = world.ship.design.carrying(ResourceId::Bandage);
    assert!(aboard >= 5, "the playtest ship carries a few: {aboard}");
    for who in 0..2 {
        world.aboard.room.set_bandages_for_probe(who, 0);
    }
    for _ in 0..(crate::class::BANDAGE_COOLDOWN as u32 * 60 + 2) {
        world.step(&[]);
    }
    assert_eq!(world.aboard.room.bandages_of(0), 1, "the player's own");
    assert_eq!(world.aboard.room.bandages_of(1), 1, "and the bot");
    assert_eq!(
        world.ship.design.carrying(ResourceId::Bandage),
        aboard,
        "nothing out of the hold"
    );
    lockers_agree(&world);
}

/// Stand a crew member at the armoury this instant, for the reach check.
/// Before every command that wants it: the Bim goes off about its errands
/// between steps, and a command lands before the room moves.
/// The helm's seat on the crew's deck, in the room's units: the middle of
/// the first helm's use spot. Forward of everything aboard that holds
/// anything, which is what a test of "out of reach" wants.
fn helm_seat(world: &World) -> bims::math::Vec2 {
    let helm = world
        .ship
        .design
        .parts
        .iter()
        .filter(|p| p.kind == PartKind::Helm)
        .min_by_key(|p| p.id)
        .expect("a helm aboard");
    let (x, y) = helm.use_spots()[0];
    let t = shipdesign::TILE as f64;
    let at = world
        .aboard
        .offset
        .add(dvec2((x as f64 + 0.5) * t, (y as f64 + 0.5) * t));
    bims::math::vec2(at.x as f32, at.y as f32)
}

fn at_the_armoury(world: &mut World, who: usize) {
    let (_, spot) = armoury(world);
    world.aboard.room.put_for_probe(who, spot);
}

/// The pieces of one kind in the hold, ids ascending.
fn in_hold(world: &World, kind: bims::combat::ArmourKind) -> Vec<&crate::Piece> {
    world
        .pieces
        .iter()
        .filter(|p| p.kind == kind && p.at == crate::Where::Hold)
        .collect()
}

/// The invariant of `crate::armour`, asked of every kind: the hold's
/// count of each armour resource is the number of pieces there of that
/// kind, and no piece there is broken.
fn pieces_agree(world: &World) {
    for kind in bims::combat::ArmourKind::ALL {
        let held = in_hold(world, kind);
        assert_eq!(
            held.len() as u32,
            world.ship.design.carrying(crate::armour::resource_of(kind)),
            "{kind:?}: {:?}",
            world.pieces
        );
        assert!(
            held.iter().all(|p| !p.broken()),
            "a broken piece in the hold"
        );
    }
    // Ids are the world's and never reissued.
    let mut ids: Vec<u32> = world.pieces.iter().map(|p| p.id).collect();
    ids.dedup();
    assert_eq!(ids.len(), world.pieces.len());
    assert!(ids.iter().all(|&id| id < world.next_piece));
}

/// A piece in a container is a resource and a piece anywhere else is an
/// instance, and the two agree: the playtest ship's helm, kevlar and leg
/// guards are three whole pieces in the hold from the first step, a
/// fetch takes one out of the count and into the pack, and putting it on
/// moves it onto the body — the count and the pieces never disagreeing.
/// The room's weapon codes name the world's resources, both ways: the
/// handgun is the pistol, and each of the four after it its own — so a
/// weapon in a hand and one in the hold are one thing, and every weapon
/// is locker class, made and never sold.
/// The weapons in the hold are a list with a tier each, and the count is
/// its length: the playtest ship's four — the pistol is in the hand — are
/// tier one, sorted; a fetch by resource takes the best tier, one by tier
/// takes exactly that tier and
/// is refused for a tier the hold has not got, a stow puts the tier back,
/// and a count poked down loses the lowest tier first.
#[test]
fn pieces_and_guns_agree_with_the_hold_and_every_weapon_is_a_locker_resource() {
    // --- pieces_agree_with_the_hold ---
    {
        use bims::combat::{ArmourKind, Item};
        use bims::health::Part;
        use shipdesign::fixture::playtest_ship;
        let mut world = simulation_world(playtest_ship(), data::SIMULATION_MONEY, 1);
        // The dressings every Bim carries are out of the way (feature 87).
        without_dressings(&mut world);
        assert_eq!(world.ship.design.carrying(ResourceId::Helm), 1);
        assert_eq!(world.pieces.len(), 3);
        assert_eq!(world.next_piece, 4);
        for (piece, kind) in world.pieces.iter().zip(ArmourKind::ALL) {
            assert_eq!(piece.kind, kind);
            assert_eq!(piece.health, kind.stats().health, "whole");
            assert_eq!(piece.at, crate::Where::Hold);
        }
        pieces_agree(&world);
        assert!(world.aboard.room.pack(0).iter().all(|c| c.is_none()));

        // Out of the armoury and into the pack: the hold's count is down by
        // one and the piece knows where it is.
        at_the_armoury(&mut world, 0);
        let events = world.step(&[Command::Fetch {
            slot: 0,
            who: 0,
            kind: crate::FetchKind::Resource(ResourceId::Helm as u32),
        }]);
        assert!(
            !events
                .iter()
                .any(|e| matches!(e, WorldEvent::Refused { .. })),
            "{events:?}"
        );
        assert_eq!(world.ship.design.carrying(ResourceId::Helm), 0);
        let Some(Item::Armour(helm)) = world.aboard.room.pack(0)[0] else {
            panic!(
                "the helm is in the first cell: {:?}",
                world.aboard.room.pack(0)
            );
        };
        assert_eq!(helm.id, 1);
        assert_eq!(world.pieces[0].at, crate::Where::Pack { who: 0, cell: 0 });
        pieces_agree(&world);

        // On the head: the piece is worn, the count is still nought.
        let events = world.step(&[Command::Equip {
            slot: 0,
            who: 0,
            cell: 0,
        }]);
        assert!(events.contains(&WorldEvent::Equipped {
            who: 0,
            kind: ArmourKind::BasicHelm
        }));
        assert_eq!(world.aboard.room.worn(0, Part::Head).map(|p| p.id), Some(1));
        assert_eq!(world.pieces[0].at, crate::Where::Worn { who: 0 });
        assert!(world.aboard.room.pack(0)[0].is_none());
        pieces_agree(&world);
        // The event names the kind in the tens.
        assert_eq!(
            WorldEvent::Equipped {
                who: 1,
                kind: ArmourKind::BasicLegs
            }
            .value(),
            31
        );
        assert_eq!(
            WorldEvent::Equipped {
                who: 0,
                kind: ArmourKind::BasicHelm
            }
            .code(),
            34
        );
        assert_eq!(WorldEvent::Stowed { who: 0 }.code(), 35);
        assert_eq!(
            WorldEvent::PieceBroke {
                who: 0,
                kind: ArmourKind::BasicHelm
            }
            .code(),
            36
        );
    }

    // --- every_weapon_is_a_locker_resource_and_the_two_tables_agree ---
    {
        use crate::armour::{item_of, resource_of_item, weapon_of, weapon_resource};
        use bims::combat::{Item, WeaponKind};
        assert_eq!(
            weapon_resource(WeaponKind::LaserPistol),
            ResourceId::Handgun
        );
        assert_eq!(weapon_resource(WeaponKind::Shotgun), ResourceId::Shotgun);
        assert_eq!(
            weapon_resource(WeaponKind::AutoRifle),
            ResourceId::AutoRifle
        );
        assert_eq!(
            weapon_resource(WeaponKind::SniperRifle),
            ResourceId::SniperRifle
        );
        assert_eq!(weapon_resource(WeaponKind::Schword), ResourceId::Schword);
        for kind in WeaponKind::ALL {
            let resource = weapon_resource(kind);
            assert_eq!(Some(resource as u32), kind.resource());
            assert_eq!(weapon_of(resource), Some(kind));
            assert_eq!(item_of(resource), Item::Weapon(kind.basic()));
            assert_eq!(resource_of_item(Item::Weapon(kind.basic())), Some(resource));
            assert_eq!(economy::storage(resource), Storage::Locker);
            // Whether a weapon is on a shelf is the **station's** roll
            // since the money rework (feature 95), not the kind's: the
            // kind lets all five through and `Stock` decides.
        }
        assert_eq!(weapon_of(ResourceId::Helm), None);
    }

    // --- guns_agree_with_the_hold ---
    {
        use bims::combat::{Item, Tier, WeaponKind};
        use shipdesign::fixture::playtest_ship;
        let mut world = simulation_world(playtest_ship(), data::SIMULATION_MONEY, 1);
        without_dressings(&mut world);
        assert_eq!(world.guns.len(), 4, "{:?}", world.guns);
        assert!(world.guns.iter().all(|g| g.tier == Tier::One));
        guns_agree(&world);

        // A tier-two pistol beside a tier-one one: the resource fetch takes
        // the better, and the list knows what is left.
        world.guns.push(WeaponKind::LaserPistol.at(Tier::Two));
        world.ship.design.cargo[ResourceId::Handgun as usize] += 2;
        world.on_ship_changed();
        guns_agree(&world);
        assert_eq!(world.guns_at(WeaponKind::LaserPistol, Tier::Two), 1);
        at_the_armoury(&mut world, 0);
        let events = world.step(&[Command::Fetch {
            slot: 0,
            who: 0,
            kind: crate::FetchKind::Resource(ResourceId::Handgun as u32),
        }]);
        unrefused(&events);
        assert_eq!(
            world.aboard.room.pack(0)[0],
            Some(Item::Weapon(WeaponKind::LaserPistol.at(Tier::Two)))
        );
        assert_eq!(world.guns_at(WeaponKind::LaserPistol, Tier::Two), 0);
        assert_eq!(world.guns_at(WeaponKind::LaserPistol, Tier::One), 1);
        guns_agree(&world);

        // A tier the hold has not got is refused; the tier it has is taken.
        at_the_armoury(&mut world, 0);
        let events = world.step(&[
            Command::Fetch {
                slot: 0,
                who: 0,
                kind: crate::FetchKind::Tiered {
                    resource: ResourceId::Shotgun as u32,
                    tier: 3,
                },
            },
            Command::Fetch {
                slot: 0,
                who: 0,
                kind: crate::FetchKind::Tiered {
                    resource: ResourceId::Shotgun as u32,
                    tier: 1,
                },
            },
        ]);
        assert_eq!(
            events
                .iter()
                .filter(|e| matches!(
                    e,
                    WorldEvent::Refused {
                        why: Refusal::NotAboard,
                        ..
                    }
                ))
                .count(),
            1,
            "{events:?}"
        );
        // The pistol lies along two cells; the shotgun goes beside it.
        assert_eq!(
            world.aboard.room.pack(0)[2],
            Some(Item::Weapon(WeaponKind::Shotgun.basic()))
        );
        assert_eq!(world.ship.design.carrying(ResourceId::Shotgun), 0);
        guns_agree(&world);

        // Stowed, the tier-two pistol is back on the list at tier two.
        at_the_armoury(&mut world, 0);
        let events = world.step(&[Command::Stow {
            slot: 0,
            who: 0,
            cell: 0,
        }]);
        unrefused(&events);
        assert_eq!(world.guns_at(WeaponKind::LaserPistol, Tier::Two), 1);
        guns_agree(&world);

        // A count that shrinks — the sell rule — loses the worst first.
        world.ship.design.cargo[ResourceId::Handgun as usize] -= 1;
        world.on_ship_changed();
        assert_eq!(world.guns_at(WeaponKind::LaserPistol, Tier::One), 0);
        assert_eq!(world.guns_at(WeaponKind::LaserPistol, Tier::Two), 1);
        guns_agree(&world);
    }
}

/// A piece worn is worn against the shots: the helm adds its health to
/// the head's, a hit on the head comes off the helm first with its
/// protection taken off the damage, and the head is untouched. Broken —
/// at nought — it is still worn and does nothing, the world says so once,
/// and it can be taken off and discarded but not stowed.
/// A damaged piece keeps its damage wherever it goes: stowed, it is in the
/// hold at the health it had, the count up by one; a sale takes the most
/// damaged piece first; and a bench pushes a fresh one. The workbench
/// makes a helm out of two metal, the way the smelter makes metal.
#[test]
fn a_shot_on_the_head_is_taken_by_the_helm_first_and_a_stowed_piece_keeps_its_health() {
    // --- a_shot_on_the_head_is_taken_by_the_helm_first ---
    {
        use bims::combat::ArmourKind;
        use bims::health::Part;
        use shipdesign::fixture::playtest_ship;
        let mut world = simulation_world(playtest_ship(), data::SIMULATION_MONEY, 1);
        // The dressings every Bim carries are out of the way (feature 87).
        without_dressings(&mut world);
        at_the_armoury(&mut world, 0);
        world.step(&[Command::Fetch {
            slot: 0,
            who: 0,
            kind: crate::FetchKind::Piece(1),
        }]);
        world.step(&[Command::Equip {
            slot: 0,
            who: 0,
            cell: 0,
        }]);
        let room = &world.aboard.room;
        assert_eq!(room.armour_health(0), 15.0);
        assert_eq!(room.part_bonus(0, Part::Head), 15.0);
        assert_eq!(room.part_bonus(0, Part::Body), 0.0);

        // Five on the head: two taken off by the protection, three off the
        // helm, nothing through.
        let head = world.aboard.room.part_health(0, Part::Head);
        let out = world.aboard.room.wound(0, Part::Head, 5.0);
        assert_eq!(out.through, 0.0);
        assert_eq!(out.absorbed, 5.0);
        assert!(!out.piece_broke);
        assert_eq!(
            world.aboard.room.part_health(0, Part::Head),
            head,
            "untouched"
        );
        assert_eq!(world.aboard.room.wounds(0, Part::Head), 0);
        assert_eq!(world.aboard.room.armour_health(0), 12.0);
        // The world's copy is read back from the room at the step.
        let events = world.step(&[]);
        assert_eq!(world.pieces[0].health, 12.0);
        assert_eq!(world.pieces[0].at, crate::Where::Worn { who: 0 });
        assert!(
            !events
                .iter()
                .any(|e| matches!(e, WorldEvent::PieceBroke { .. }))
        );

        // Fourteen more: twelve off the helm, which is broken, nothing
        // through — and the world says so, once.
        let out = world.aboard.room.wound(0, Part::Head, 14.0);
        assert!(out.piece_broke);
        assert_eq!(out.through, 0.0);
        assert_eq!(world.aboard.room.armour_health(0), 0.0, "does nothing now");
        assert!(
            world.aboard.room.worn(0, Part::Head).is_some(),
            "still worn"
        );
        let events = world.step(&[]);
        assert!(events.contains(&WorldEvent::PieceBroke {
            who: 0,
            kind: ArmourKind::BasicHelm
        }));
        let events = world.step(&[]);
        assert!(
            !events
                .iter()
                .any(|e| matches!(e, WorldEvent::PieceBroke { .. })),
            "said once"
        );
        // And with nothing on it, the head takes the next one whole: the
        // protection is lifted with the piece.
        let out = world.aboard.room.wound(0, Part::Head, 1.0);
        assert_eq!(out.through, 1.0);
        assert_eq!(world.aboard.room.part_health(0, Part::Head), head - 1.0);

        // Off, into the pack; a stow is refused — a broken piece is worth
        // nothing in the hold — and the count does not move; a discard is
        // the end of it.
        world.step(&[Command::Unequip {
            slot: 0,
            who: 0,
            part: Part::Head,
        }]);
        assert!(world.aboard.room.worn(0, Part::Head).is_none());
        assert_eq!(world.pieces[0].at, crate::Where::Pack { who: 0, cell: 0 });
        at_the_armoury(&mut world, 0);
        let events = world.step(&[Command::Stow {
            slot: 0,
            who: 0,
            cell: 0,
        }]);
        assert!(events.contains(&WorldEvent::Refused {
            slot: 0,
            why: Refusal::Broken
        }));
        assert_eq!(world.ship.design.carrying(ResourceId::Helm), 0);
        assert_eq!(world.pieces.len(), 3);
        world.step(&[Command::Discard {
            slot: 0,
            who: 0,
            cell: 0,
        }]);
        assert!(world.aboard.room.pack(0)[0].is_none());
        assert_eq!(world.pieces.len(), 2, "gone for good");
        assert!(world.pieces.iter().all(|p| p.id != 1));
        assert_eq!(world.ship.design.carrying(ResourceId::Helm), 0);
        pieces_agree(&world);
    }

    // --- a_stowed_piece_keeps_its_health_and_a_sale_takes_the_worst ---
    {
        use bims::combat::ArmourKind;
        use bims::health::Part;
        use shipdesign::fixture::playtest_ship;
        let mut world = simulation_world(playtest_ship(), data::SIMULATION_MONEY, 1);
        without_dressings(&mut world);

        // A second helm: bought rather than made since the money rework
        // (feature 95) — the hold's count is poked the way a purchase
        // moves it, and `settle_pieces` puts a whole piece behind it.
        world.ship.design.cargo[ResourceId::Helm as usize] += 1;
        world.on_ship_changed();
        assert_eq!(world.ship.design.carrying(ResourceId::Helm), 2);
        let helms = in_hold(&world, ArmourKind::BasicHelm);
        assert_eq!(helms.len(), 2);
        assert_eq!(helms[1].health, 15.0, "whole");
        pieces_agree(&world);

        // The first helm, worn, dented, and put back: the hold has two helms
        // and one of them has twelve left.
        at_the_armoury(&mut world, 0);
        world.step(&[Command::Fetch {
            slot: 0,
            who: 0,
            kind: crate::FetchKind::Piece(1),
        }]);
        world.step(&[Command::Equip {
            slot: 0,
            who: 0,
            cell: 0,
        }]);
        world.aboard.room.wound(0, Part::Head, 5.0);
        world.step(&[Command::Unequip {
            slot: 0,
            who: 0,
            part: Part::Head,
        }]);
        at_the_armoury(&mut world, 0);
        let events = world.step(&[Command::Stow {
            slot: 0,
            who: 0,
            cell: 0,
        }]);
        assert!(
            events.contains(&WorldEvent::Stowed { who: 0 }),
            "{events:?}"
        );
        assert_eq!(world.ship.design.carrying(ResourceId::Helm), 2);
        let dented = world.pieces.iter().find(|p| p.id == 1).unwrap();
        assert_eq!(dented.at, crate::Where::Hold);
        assert_eq!(dented.health, 12.0, "kept its damage");
        pieces_agree(&world);

        // A fetch by resource takes the best one; back it goes.
        at_the_armoury(&mut world, 0);
        world.step(&[Command::Fetch {
            slot: 0,
            who: 0,
            kind: crate::FetchKind::Resource(ResourceId::Helm as u32),
        }]);
        let Some(bims::combat::Item::Armour(best)) = world.aboard.room.pack(0)[0] else {
            panic!("a helm in the pack");
        };
        assert_eq!(best.id, 4, "the whole one");
        at_the_armoury(&mut world, 0);
        world.step(&[Command::Stow {
            slot: 0,
            who: 0,
            cell: 0,
        }]);
        pieces_agree(&world);

        // Sold, one goes: the dented one, and the whole one stays. Sold from
        // the desk, where a sale is made.
        assert!(world.man_the_desk_for_probe(0));
        let money = world.money;
        let events = world.step(&[Command::Sell {
            slot: 0,
            resource: ResourceId::Helm,
            units: 1,
        }]);
        world.aboard.room.stand_down(0);
        assert!(events.iter().any(|e| matches!(
            e,
            WorldEvent::Traded {
                resource: ResourceId::Helm,
                units: -1,
                ..
            }
        )));
        assert!(world.money > money);
        assert_eq!(world.ship.design.carrying(ResourceId::Helm), 1);
        let helms = in_hold(&world, ArmourKind::BasicHelm);
        assert_eq!(helms.len(), 1);
        assert_eq!(helms[0].id, 4);
        assert_eq!(helms[0].health, 15.0);
        pieces_agree(&world);
    }
}

/// Reach and room: a stow or a fetch wants the crew member within `REACH`
/// of a container that takes the thing — the armoury for a piece of
/// armour, or a shelf, which takes armour and weapons as well as
/// materials, and not the cold store — a full pack refuses a fetch, and
/// a full class refuses a stow. Putting on what is in the pack wants no
/// container at all.
#[test]
fn a_fetch_or_a_stow_wants_the_bim_in_reach_and_room_to_put_it() {
    use bims::game::Container;
    use shipdesign::fixture::playtest_ship;
    let mut world = simulation_world(playtest_ship(), data::SIMULATION_MONEY, 1);
    // The dressings every Bim carries are out of the way (feature 87).
    without_dressings(&mut world);
    let (bench, _) = armoury(&world);
    assert!(world.container_takes(Container::Bench(bench), ResourceId::Helm));
    assert!(world.container_takes(Container::Bench(bench), ResourceId::Bandage));
    assert!(!world.container_takes(Container::Bench(bench), ResourceId::Vegetable));
    // A shelf is locker room since the money rework (feature 95): it
    // takes what a locker takes and nothing the cold store does.
    assert!(world.container_takes(Container::Shelf(0), ResourceId::Helm));
    assert!(world.container_takes(Container::Shelf(0), ResourceId::Handgun));
    assert!(world.container_takes(Container::Shelf(0), ResourceId::Bandage));
    assert!(!world.container_takes(Container::Shelf(0), ResourceId::Vegetable));
    assert!(!world.container_takes(Container::Shelf(0), ResourceId::Tofu));
    assert!(world.container_takes(Container::Fridge(0), ResourceId::Tofu));
    assert!(!world.container_takes(Container::Fridge(0), ResourceId::Helm));
    // The workbench is a bench that holds nothing: it is worked at, not
    // stowed into (`shipdesign::recipes::is_workstation`).
    let bench = (0..world.aboard.room.benches().len())
        .find(|&i| world.aboard.room.bench_part(i) == PartKind::Workbench.code())
        .unwrap();
    assert!(!world.container_takes(Container::Bench(bench), ResourceId::Helm));
    assert!(!world.container_takes(Container::Bench(bench), ResourceId::Vegetable));

    // At the helm, forward of everything that holds anything — the bunk
    // is right beside the armoury — refused, and nothing moved.
    let helm = helm_seat(&world);
    world.aboard.room.put_for_probe(0, helm);
    assert!(!world.in_reach(0, ResourceId::Helm));
    let fetch = Command::Fetch {
        slot: 0,
        who: 0,
        kind: crate::FetchKind::Piece(1),
    };
    let events = world.step(&[fetch]);
    assert!(events.contains(&WorldEvent::Refused {
        slot: 0,
        why: Refusal::OutOfReach
    }));
    assert_eq!(world.ship.design.carrying(ResourceId::Helm), 1);
    assert!(world.aboard.room.pack(0).iter().all(|c| c.is_none()));

    // At the armoury: in reach, and the cells fill from the hold — the
    // three pieces, the suit, a box of dressings and then medkits until
    // the pack has no square left for another.
    at_the_armoury(&mut world, 0);
    assert!(world.in_reach(0, ResourceId::Helm));
    // The dressings `without_dressings` took out of the hold, put back:
    // five of them, which is one box since feature 87 and not five cells.
    world.ship.design.cargo[ResourceId::Bandage as usize] = 5;
    world.on_ship_changed();
    let bandages = world.ship.design.carrying(ResourceId::Bandage);
    assert_eq!(bandages, 5);
    assert_eq!(world.ship.design.carrying(ResourceId::Suit), 1);
    let mut commands = vec![
        Command::Fetch {
            slot: 0,
            who: 0,
            kind: crate::FetchKind::Resource(ResourceId::Kevlar as u32),
        },
        Command::Fetch {
            slot: 0,
            who: 0,
            kind: crate::FetchKind::Resource(ResourceId::Helm as u32),
        },
        Command::Fetch {
            slot: 0,
            who: 0,
            kind: crate::FetchKind::Resource(ResourceId::Suit as u32),
        },
        Command::Fetch {
            slot: 0,
            who: 0,
            kind: crate::FetchKind::Resource(ResourceId::LegGuard as u32),
        },
    ];
    for _ in 0..5 {
        commands.push(Command::Fetch {
            slot: 0,
            who: 0,
            kind: crate::FetchKind::Resource(ResourceId::Bandage as u32),
        });
    }
    // And medkits after them — two by two and nothing stacks — put
    // aboard by hand, until the pack has no square left: whatever does
    // not fit is `PackFull`.
    world.ship.design.cargo[ResourceId::Medkit as usize] += 6;
    world.on_ship_changed();
    for _ in 0..6 {
        commands.push(Command::Fetch {
            slot: 0,
            who: 0,
            kind: crate::FetchKind::Resource(ResourceId::Medkit as u32),
        });
    }
    let events = world.step(&commands);
    let refused: Vec<&WorldEvent> = events
        .iter()
        .filter(|e| matches!(e, WorldEvent::Refused { .. }))
        .collect();
    assert!(!refused.is_empty(), "the pack fills up: {events:?}");
    assert!(
        refused.iter().all(|e| matches!(
            e,
            WorldEvent::Refused {
                why: Refusal::PackFull,
                ..
            }
        )),
        "{refused:?}"
    );
    // The kevlar four square at the top left, the helm beside it, the
    // suit under the helm, the leg guards down the last two columns, one
    // box of dressings and as many medkits as would lie — and no
    // two-by-two hole left for another.
    let pack = world.aboard.room.pack(0);
    let medkit = bims::combat::Item::Stack(ResourceId::Medkit as u32);
    assert!(world.aboard.room.gear(0).free_cell_for(medkit).is_none());
    let boxes: Vec<usize> = pack
        .iter()
        .enumerate()
        .filter(|(_, c)| **c == Some(bims::combat::Item::Stack(ResourceId::Bandage as u32)))
        .map(|(i, _)| i)
        .collect();
    assert_eq!(boxes.len(), 1, "five to a box: {pack:?}");
    assert_eq!(world.aboard.room.gear(0).units(boxes[0]), 5);
    let kevlar_cell = 0;
    assert!(matches!(
        pack[kevlar_cell],
        Some(bims::combat::Item::Armour(p)) if p.kind == bims::combat::ArmourKind::BasicKevlar
    ));
    assert_eq!(world.ship.design.carrying(ResourceId::Bandage), 0);
    assert_eq!(world.ship.design.carrying(ResourceId::Suit), 0);
    assert_eq!(world.ship.design.carrying(ResourceId::Helm), 0);
    pieces_agree(&world);
    // An armour resource the hold has none of is nothing to fetch.
    let events = world.step(&[Command::Fetch {
        slot: 0,
        who: 0,
        kind: crate::FetchKind::Piece(1),
    }]);
    assert!(events.contains(&WorldEvent::Refused {
        slot: 0,
        why: Refusal::NotAboard
    }));

    // Putting the kevlar on wants no container: from the helm, it goes.
    world.aboard.room.put_for_probe(0, helm);
    assert!(!world.in_reach(0, ResourceId::Kevlar));
    let events = world.step(&[Command::Equip {
        slot: 0,
        who: 0,
        cell: kevlar_cell as u32,
    }]);
    assert!(events.contains(&WorldEvent::Equipped {
        who: 0,
        kind: bims::combat::ArmourKind::BasicKevlar
    }));
    assert_eq!(
        world.aboard.room.part_bonus(0, bims::health::Part::Body),
        20.0
    );
    assert_eq!(world.aboard.room.armour_health(0), 20.0);
    assert!(world.aboard.room.pack(0)[kevlar_cell].is_none());
    // A stack is not something to put on.
    let events = world.step(&[Command::Equip {
        slot: 0,
        who: 0,
        cell: boxes[0] as u32,
    }]);
    assert!(events.contains(&WorldEvent::Refused {
        slot: 0,
        why: Refusal::NotAboard
    }));

    // The box of dressings stays where it is, at the armoury or not: a
    // bandage is everybody's charge, which the cooldown would fill the
    // pack back up with, so none is ever stowed.
    at_the_armoury(&mut world, 0);
    let events = world.step(&[Command::Stow {
        slot: 0,
        who: 0,
        cell: boxes[0] as u32,
    }]);
    assert!(events.contains(&WorldEvent::Refused {
        slot: 0,
        why: Refusal::ChargeKept
    }));
    assert_eq!(
        world.aboard.room.gear(0).units(boxes[0]),
        5,
        "all five kept"
    );
    assert_eq!(world.ship.design.carrying(ResourceId::Bandage), 0);
    // A stow from the helm is out of reach; at the armoury the leg
    // guards go back, and once the lockers are full the next thing is
    // refused.
    let legs = world
        .aboard
        .room
        .pack(0)
        .iter()
        .position(|c| {
            matches!(c, Some(bims::combat::Item::Armour(p))
                if p.kind == bims::combat::ArmourKind::BasicLegs)
        })
        .expect("the leg guards in the pack");
    let legs_held = world.ship.design.carrying(ResourceId::LegGuard);
    world.aboard.room.put_for_probe(0, helm);
    let events = world.step(&[Command::Stow {
        slot: 0,
        who: 0,
        cell: legs as u32,
    }]);
    assert!(events.contains(&WorldEvent::Refused {
        slot: 0,
        why: Refusal::OutOfReach
    }));
    at_the_armoury(&mut world, 0);
    let events = world.step(&[Command::Stow {
        slot: 0,
        who: 0,
        cell: legs as u32,
    }]);
    assert!(events.contains(&WorldEvent::Stowed { who: 0 }));
    assert_eq!(
        world.ship.design.carrying(ResourceId::LegGuard),
        legs_held + 1
    );
    // The suit is the next thing in the pack to try to put away.
    let suit = bims::combat::Item::Stack(ResourceId::Suit as u32);
    let kit_cell = world
        .aboard
        .room
        .pack(0)
        .iter()
        .position(|c| *c == Some(suit))
        .expect("the suit is still on its back");
    let lockers = world.ship.design.capacity(Storage::Locker);
    let stored = world.ship.design.stored(Storage::Locker);
    world.ship.design.cargo[ResourceId::Grenade as usize] += lockers - stored;
    world.on_ship_changed();
    at_the_armoury(&mut world, 0);
    let events = world.step(&[Command::Stow {
        slot: 0,
        who: 0,
        cell: kit_cell as u32,
    }]);
    assert!(events.contains(&WorldEvent::Refused {
        slot: 0,
        why: Refusal::NoRoom
    }));
    assert!(
        world.aboard.room.pack(0)[kit_cell].is_some(),
        "left in the pack"
    );
    pieces_agree(&world);
}

// --- looting a body ------------------------------------------------------------

/// A piece of armour into a crew member's pack the way a fetch puts it
/// there — on the world's list under the next id, `at: Pack`, and in the
/// room's cell — without the container: the flyer has no shelf or
/// locker to reach into, and a test of looting is not a test of the walk
/// to one.
/// The room's copy of a piece of the world's: the same three numbers.
fn room_piece(piece: crate::Piece) -> bims::combat::Piece {
    bims::combat::Piece {
        id: piece.id,
        kind: piece.kind,
        tier: piece.tier,
        health: piece.health,
    }
}

fn hand_a_piece(
    world: &mut World,
    who: u32,
    kind: bims::combat::ArmourKind,
    health: f32,
) -> crate::Piece {
    let id = world.next_piece;
    world.next_piece += 1;
    let piece = crate::Piece {
        id,
        kind,
        tier: Tier::One,
        health,
        at: crate::Where::Pack { who, cell: 0 },
    };
    world.pieces.push(piece);
    assert!(world.aboard.room.give(who as usize, Some(0), piece.item()));
    piece
}

/// A crewmate who is out cold can be looted, and one who is awake cannot:
/// the helm comes off the body with the health it had, into the looter's
/// pack, the world's copy moves with it, and the checksum notices — two
/// worlds that disagree about who has the helm are two different worlds.
/// Reach is asked when the command lands.
#[test]
fn an_unconscious_crewmate_is_looted_and_an_awake_one_is_refused() {
    use bims::combat::{ArmourKind, Item, LootCell};
    use bims::health::Part;
    let tile = shipdesign::TILE as f32;
    let mut world = basic();
    let mut twin = basic();
    // The packs empty of their medicine, so the helm goes in cell 0.
    without_dressings(&mut world);
    without_dressings(&mut twin);
    // James under orders in both, so he stands where the test puts him
    // rather than going to see to Kate when she is down — which he would
    // do a step apart in the two worlds, one of them having sent him
    // across the deck first.
    world.aboard.room.recruit_for_probe(0, true);
    twin.aboard.room.recruit_for_probe(0, true);
    let source = crate::LootSource::Crew(1);
    let head = LootCell::Head.code();

    // Kate wears a dented helm, in both worlds.
    let piece = hand_a_piece(&mut world, 1, ArmourKind::BasicHelm, 9.0);
    assert_eq!(
        hand_a_piece(&mut twin, 1, ArmourKind::BasicHelm, 9.0),
        piece
    );
    let equip = Command::Equip {
        slot: 1,
        who: 1,
        cell: 0,
    };
    world.step(&[equip]);
    twin.step(&[equip]);
    assert_eq!(
        world.aboard.room.worn(1, Part::Head),
        Some(room_piece(piece))
    );
    assert_eq!(world.pieces[0].at, crate::Where::Worn { who: 1 });
    assert_eq!(world.checksum(), twin.checksum());
    let cells = world.loot_cells(source).expect("a crewmate shows");
    assert_eq!(cells[head as usize], Some(piece.item()));
    assert_eq!(
        cells[LootCell::Weapon.code() as usize],
        Some(Item::Weapon(bims::combat::WeaponKind::LaserPistol.basic()))
    );
    assert_eq!(world.loot_cells(crate::LootSource::Crew(9)), None);

    // Awake, she is nobody's to loot, however close James stands.
    let beside = |world: &mut World| {
        let at = world.aboard.room.bim_pos(1);
        world
            .aboard
            .room
            .put_for_probe(0, bims::math::vec2(at.x + tile, at.y));
    };
    beside(&mut world);
    beside(&mut twin);
    assert!(!world.is_down(source));
    let loot = Command::Loot {
        slot: 0,
        who: 0,
        source,
        cell: head,
    };
    let events = world.step(&[loot]);
    twin.step(&[loot]);
    assert!(
        events.contains(&WorldEvent::Refused {
            slot: 0,
            why: Refusal::NotDown
        }),
        "{events:?}"
    );
    assert_eq!(
        world.aboard.room.worn(1, Part::Head),
        Some(room_piece(piece))
    );
    assert_eq!(Refusal::NotDown.code(), 18);

    // Bled to under the line, she is out cold on the deck.
    for _ in 0..10 {
        world.aboard.room.wound(1, Part::Body, 1.0);
        twin.aboard.room.wound(1, Part::Body, 1.0);
    }
    let mut budget = 60 * 60;
    while budget > 0 && !world.aboard.room.is_unconscious(1) {
        world.step(&[]);
        twin.step(&[]);
        budget -= 1;
    }
    assert!(world.aboard.room.is_unconscious(1), "out cold");
    assert!(world.aboard.room.is_alive(1), "not dead");
    assert!(world.is_down(source));
    assert_eq!(world.checksum(), twin.checksum());

    // From across the deck the command is refused; beside her it lands.
    let far = world.aboard.room.bim_pos(1);
    world
        .aboard
        .room
        .put_for_probe(0, bims::math::vec2(far.x - 6.0 * tile, far.y));
    assert!(!world.in_reach_of_body(0, source));
    let events = world.step(&[loot]);
    twin.step(&[]);
    assert!(
        events.contains(&WorldEvent::Refused {
            slot: 0,
            why: Refusal::OutOfReach
        }),
        "{events:?}"
    );
    beside(&mut world);
    beside(&mut twin);
    assert!(world.in_reach_of_body(0, source));
    let events = world.step(&[loot]);
    twin.step(&[]);
    assert!(
        events.contains(&WorldEvent::Looted {
            who: 0,
            source_kind: 0
        }),
        "{events:?}"
    );
    assert_eq!(world.aboard.room.worn(1, Part::Head), None, "stripped");
    assert_eq!(world.aboard.room.pack(0)[0], Some(piece.item()));
    assert_eq!(world.pieces.len(), 1, "the same piece, moved");
    assert_eq!(world.pieces[0].at, crate::Where::Pack { who: 0, cell: 0 });
    assert_eq!(world.pieces[0].health, 9.0);
    assert_ne!(world.checksum(), twin.checksum(), "one has looted");
    // The twin catches up and the two agree again; and a bare head has
    // nothing more to give.
    let events = twin.step(&[loot]);
    world.step(&[loot]);
    assert!(events.contains(&WorldEvent::Looted {
        who: 0,
        source_kind: 0
    }));
    assert_eq!(world.checksum(), twin.checksum(), "both have");
    // The event names the looter, the source's kind in the tens.
    let looted = WorldEvent::Looted {
        who: 1,
        source_kind: 1,
    };
    assert_eq!(looted.code(), 38);
    assert_eq!(looted.value(), 11);
}

/// **Half its blood is where a crew member goes out, and a body that far
/// gone is nobody's target** (feature 89). A crew member ashore at a
/// station the machines hold is believed where it stands; bled to just
/// over half it is still on its feet and still believed; bled a drop
/// under half it is out cold, and the machines forget it the same step —
/// the slot goes `None`, which is what `aim`, `melee_with` and the
/// tactics all read. The band above it — half pace under three quarters — is
/// pinned in `bims::health`'s own tests.
#[test]
fn a_crew_member_under_half_its_blood_is_out_cold_and_nobody_s_target() {
    use bims::health::{MAX_BLOOD, OUT_AT};
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

    // Half its blood is the line: a drop above it and it is still up.
    let body = who as usize;
    world.aboard.room.set_blood_for_probe(body, OUT_AT + 0.01);
    world.step(&[]);
    assert!(
        !world.aboard.room.is_unconscious(body),
        "just over half is still up at {}",
        world.aboard.room.blood(body)
    );
    assert!(believed(&world).is_some(), "and still somebody's target");

    // And a drop under it: out cold, and gone from the list the machines
    // shoot at.
    world.aboard.room.set_blood_for_probe(body, OUT_AT - 0.01);
    world.step(&[]);
    assert!(
        world.aboard.room.is_unconscious(body),
        "under half its blood is out cold at {}",
        world.aboard.room.blood(body)
    );
    assert!(
        world.aboard.room.blood(body) > MAX_BLOOD * 0.45,
        "and nowhere near dead"
    );
    assert_eq!(believed(&world), None, "a body down is nobody's target");
}


/// Upgrades at the workbench are **always allowed** (feature 106): with
/// research gone from the game there is no node to wait on. The button
/// finds no refusal but the bench's own, and with Combine matching gear
/// on and two matching tier-one pairs in the hold, the helms go first and
/// the day's work begins the step the pair is on the bench.
#[test]
fn a_workbench_upgrade_wants_no_research() {
    use bims::combat::{Tier, WeaponKind};
    use shipdesign::fixture::playtest_ship;
    let mut world = simulation_world(playtest_ship(), data::SIMULATION_MONEY, 1);
    world.undock_for_probe();
    world.ship.design.cargo[ResourceId::Handgun as usize] += 2;
    world.ship.design.cargo[ResourceId::Helm as usize] += 2;
    world.on_ship_changed();
    // The playtest ship carries a helm of its own: three, two of them a pair.
    assert_eq!(world.ship.design.carrying(ResourceId::Helm), 3);
    assert_eq!(world.guns_at(WeaponKind::LaserPistol, Tier::One), 2);
    // Nothing on the bench yet: the bench's own refusal, and no other.
    assert_eq!(world.can_upgrade(), Err(Refusal::NoPair));
    let events = world.step(&[Command::SetAutoUpgrade { slot: 0, on: true }]);
    unrefused(&events);
    let (said, begun) = run_until(&mut world, 8, &|w| w.bench.work.is_some());
    assert!(begun, "{said:?}");
    assert_eq!(
        said,
        vec![WorldEvent::UpgradeBegun {
            resource: ResourceId::Helm as u32,
            tier: 2,
        }]
    );
    guns_agree(&world);
}

// --- tiers, and the workbench's upgrade -------------------------------------

/// The invariant of `World::guns`, asked of every kind: the hold's count of
/// each weapon resource is the number of guns on the list of that kind,
/// and the list is sorted by kind and tier.
fn guns_agree(world: &World) {
    use bims::combat::WeaponKind;
    for kind in WeaponKind::ALL {
        assert_eq!(
            world.guns.iter().filter(|g| g.kind == kind).count() as u32,
            world
                .ship
                .design
                .carrying(crate::armour::weapon_resource(kind)),
            "{kind:?}: {:?}",
            world.guns
        );
    }
    let keys: Vec<(u32, u32)> = world
        .guns
        .iter()
        .map(|g| (g.kind.code(), g.tier.code()))
        .collect();
    assert!(keys.windows(2).all(|w| w[0] <= w[1]), "sorted: {keys:?}");
}

/// No refusal among the events.
fn unrefused(events: &[WorldEvent]) {
    assert!(
        !events
            .iter()
            .any(|e| matches!(e, WorldEvent::Refused { .. })),
        "{events:?}"
    );
}

/// The workbench with the slots, and where a Bim stands to reach onto it.
fn workbench(world: &World) -> (usize, bims::math::Vec2) {
    use bims::game::Container;
    let i = world
        .workbench()
        .expect("the playtest ship has a workbench");
    let spot = world
        .aboard
        .room
        .container_spot(Container::Bench(i))
        .expect("and it has a use spot");
    (i, spot)
}

/// Stand a crew member at the workbench this instant, like `at_the_armoury`.
fn at_the_workbench(world: &mut World, who: usize) {
    let (_, spot) = workbench(world);
    world.aboard.room.put_for_probe(who, spot);
}

/// Run a world until `done` says so, at most `hours` of it, gathering the
/// events that named the workbench's work; whether it said so.
fn run_until(
    world: &mut World,
    hours: u32,
    done: &dyn Fn(&World) -> bool,
) -> (Vec<WorldEvent>, bool) {
    let mut said = Vec::new();
    for _ in 0..(hours * 60 * 60) {
        for e in world.step(&[]) {
            if matches!(
                e,
                WorldEvent::UpgradeBegun { .. } | WorldEvent::Upgraded { .. }
            ) {
                said.push(e);
            }
        }
        if done(world) {
            return (said, true);
        }
    }
    (said, false)
}

/// The tick box on and two pistols in the hold: a Bim carries them to
/// the workbench one at a time — each out of the hold and off the list as
/// it is picked up, on the bench as it is put down — the day's work
/// begins the step the pair is there, a day of hour-long sessions later
/// the two are gone from the bench and one pistol is in its output slot
/// at tier two, and the Bim carries that back to the lockers: on the
/// list and in the count, and a fetch hands over the better gun. Off, a
/// pair sits there.
/// Three helms, one dented: armour goes before weapons, the dented helm
/// and one whole one go onto the bench, the other whole one stays, and the
/// piece that comes off is fresh at tier two with half again the health —
/// waiting on the bench while the lockers are full, since nothing is
/// carried back to a locker with no room for it — and the checksum sees
/// every step of it.
/// The constants agree with what they were asked for: a day of hour-long
/// sessions, an order code past every recipe, and an event value that
/// carries the resource and the tier apart.
#[test]
fn two_pistols_or_two_helms_are_combined_at_the_workbench_over_a_day() {
    // --- two_pistols_are_combined_at_the_workbench_over_a_day ---
    {
        use bims::combat::{Item, Tier, WeaponKind};
        use shipdesign::fixture::playtest_ship;
        let mut world = simulation_world(playtest_ship(), data::SIMULATION_MONEY, 1);
        // The dressings every Bim carries are out of the way (feature 87).
        without_dressings(&mut world);
        world.undock_for_probe();
        world.ship.design.cargo[ResourceId::Handgun as usize] += 2;
        world.on_ship_changed();
        assert_eq!(world.guns_at(WeaponKind::LaserPistol, Tier::One), 2);
        assert!(world.powered(PartKind::Workbench));
        assert!(world.workbench().is_some());
        assert_eq!(world.store_bench(), Some(armoury(&world).0));

        // Off, nothing happens: nobody carries anything, and the bench
        // stays empty.
        for _ in 0..600 {
            let events = world.step(&[]);
            assert!(
                !events
                    .iter()
                    .any(|e| matches!(e, WorldEvent::UpgradeBegun { .. }))
            );
            assert!(world.aboard.room.ferries().is_empty());
        }
        assert!(world.bench.is_empty());
        assert_eq!(world.guns_at(WeaponKind::LaserPistol, Tier::One), 2);

        // On: the first pistol is asked for at once, and a Bim carries it
        // over — out of the hold as it is picked up.
        let events = world.step(&[Command::SetAutoUpgrade { slot: 0, on: true }]);
        assert!(world.auto_upgrade());
        unrefused(&events);
        assert_eq!(
            world.aboard.room.ferries(),
            vec![bims::game::Ferry {
                from: armoury(&world).0,
                to: world.workbench().unwrap(),
            }]
        );
        let (_, picked) = run_until(&mut world, 2, &|w| w.bench.carrying.is_some());
        assert!(picked, "somebody picked a pistol up");
        assert_eq!(
            world.bench.carrying,
            Some(Item::Weapon(WeaponKind::LaserPistol.at(Tier::One)))
        );
        assert!(!world.bench.back);
        assert_eq!(world.guns_at(WeaponKind::LaserPistol, Tier::One), 1);
        assert_eq!(world.ship.design.carrying(ResourceId::Handgun), 1);
        guns_agree(&world);
        // The carry stays on the room's list until it lands.
        assert_eq!(world.aboard.room.ferries().len(), 1);

        // Both on the bench, and the work begun the same step.
        let (said, begun) = run_until(&mut world, 4, &|w| w.bench.work.is_some());
        assert!(begun, "{said:?}");
        assert_eq!(
            said,
            vec![WorldEvent::UpgradeBegun {
                resource: ResourceId::Handgun as u32,
                tier: 2,
            }]
        );
        assert_eq!(world.ship.design.carrying(ResourceId::Handgun), 0);
        assert_eq!(world.guns_at(WeaponKind::LaserPistol, Tier::One), 0);
        assert_eq!(
            world.bench.slots,
            [
                Some(Item::Weapon(WeaponKind::LaserPistol.basic())),
                Some(Item::Weapon(WeaponKind::LaserPistol.basic())),
                None
            ]
        );
        assert_eq!(world.bench.carrying, None);
        guns_agree(&world);
        let upgrade = world.bench.work.expect("under way");
        assert_eq!(upgrade.resource, ResourceId::Handgun);
        assert_eq!(upgrade.to, Tier::Two);
        assert_eq!(upgrade.done, 0);
        // The Bim takes the session as soon as its hands are off the
        // second pistol — the order is posted before the room moves — and
        // while it is at the bench no second order is offered: one pair
        // of hands a day.
        let (_, at_it) = run_until(&mut world, 1, &|w| {
            w.aboard.room.crafts_under_way(crate::UPGRADE_ORDER) == 1
        });
        assert!(at_it, "somebody at the bench");
        assert!(
            !world
                .craft_orders()
                .iter()
                .any(|o| o.recipe == crate::UPGRADE_ORDER),
            "nobody else is offered it"
        );
        // Nothing comes off the bench while the work is on it.
        at_the_workbench(&mut world, 0);
        let events = world.step(&[Command::Fetch {
            slot: 0,
            who: 0,
            kind: crate::FetchKind::Bench { slot: 0 },
        }]);
        assert!(
            events.contains(&WorldEvent::Refused {
                slot: 0,
                why: Refusal::BenchBusy
            }),
            "{events:?}"
        );
        // Timed by the mission clock, the one that runs with the step.
        let clock_at_start = world.mission_minutes();

        // A day's sessions, with the Bim's own day round them, and it is
        // done: the pair gone, the tier-two pistol in the output slot.
        let (said, upgraded) = run_until(&mut world, 4 * 24, &|w| w.bench.work.is_none());
        assert!(upgraded, "{said:?}");
        assert_eq!(
            said,
            vec![WorldEvent::Upgraded {
                resource: ResourceId::Handgun as u32,
                tier: 2,
            }]
        );
        assert!(
            world.mission_minutes() - clock_at_start
                >= data::UPGRADE_SESSIONS as f64 * data::UPGRADE_SESSION_MINUTES,
            "a day of work at least: {} minutes",
            world.mission_minutes() - clock_at_start
        );
        assert_eq!(
            world.bench.slots,
            [
                None,
                None,
                Some(Item::Weapon(WeaponKind::LaserPistol.at(Tier::Two)))
            ]
        );
        assert!(
            !world
                .craft_orders()
                .iter()
                .any(|o| o.recipe == crate::UPGRADE_ORDER),
            "nothing more to do at the bench"
        );

        // And carried back: on the list and in the count.
        let (_, landed) = run_until(&mut world, 4, &|w| {
            w.guns_at(WeaponKind::LaserPistol, Tier::Two) == 1
        });
        assert!(landed, "the pistol came back to the lockers");
        assert!(world.bench.is_empty(), "{:?}", world.bench);
        assert_eq!(world.ship.design.carrying(ResourceId::Handgun), 1);
        guns_agree(&world);
        assert!(world.aboard.room.ferries().is_empty());

        // And it is the gun a fetch hands over.
        at_the_armoury(&mut world, 0);
        let events = world.step(&[Command::Fetch {
            slot: 0,
            who: 0,
            kind: crate::FetchKind::Resource(ResourceId::Handgun as u32),
        }]);
        unrefused(&events);
        let pack = world.aboard.room.pack(0);
        assert!(
            pack.contains(&Some(Item::Weapon(WeaponKind::LaserPistol.at(Tier::Two)))),
            "{pack:?}"
        );
        guns_agree(&world);
    }

    // --- two_helms_are_combined_and_the_worse_two_go_in ---
    {
        use bims::combat::{ArmourKind, Item, Tier};
        use shipdesign::fixture::playtest_ship;
        let mut world = simulation_world(playtest_ship(), data::SIMULATION_MONEY, 1);
        without_dressings(&mut world);
        world.undock_for_probe();
        world.ship.design.cargo[ResourceId::Handgun as usize] += 2;
        world.ship.design.cargo[ResourceId::Helm as usize] += 2;
        world.on_ship_changed();
        pieces_agree(&world);
        let helms: Vec<u32> = in_hold(&world, ArmourKind::BasicHelm)
            .iter()
            .map(|p| p.id)
            .collect();
        assert_eq!(helms.len(), 3);
        let dented = helms[1];
        world
            .pieces
            .iter_mut()
            .find(|p| p.id == dented)
            .unwrap()
            .health = 5.0;
        let twin_before = world.checksum();

        world.step(&[Command::SetAutoUpgrade { slot: 0, on: true }]);
        let (said, begun) = run_until(&mut world, 6, &|w| w.bench.work.is_some());
        assert!(begun, "{said:?}");
        assert_eq!(
            said,
            vec![WorldEvent::UpgradeBegun {
                resource: ResourceId::Helm as u32,
                tier: 2,
            }],
            "armour first"
        );
        assert_ne!(world.checksum(), twin_before);
        let left = in_hold(&world, ArmourKind::BasicHelm);
        assert_eq!(left.len(), 1);
        assert_eq!(
            left[0].id, helms[2],
            "the dented one and the first whole one went in"
        );
        // The two on the bench are the world's no longer: whole, health and
        // all, in the slots.
        assert!(
            world
                .pieces
                .iter()
                .all(|p| p.id != dented && p.id != helms[0])
        );
        let on_bench: Vec<(u32, f32)> = world.bench.slots[..2]
            .iter()
            .map(|s| match s {
                Some(Item::Armour(p)) => (p.id, p.health),
                other => panic!("{other:?}"),
            })
            .collect();
        assert_eq!(on_bench, vec![(dented, 5.0), (helms[0], 15.0)]);
        assert_eq!(world.ship.design.carrying(ResourceId::Helm), 1);
        pieces_agree(&world);
        // The pistols are still a pair, waiting their turn.
        assert_eq!(world.ship.design.carrying(ResourceId::Handgun), 2);

        // The lockers filled to the last cell with grenades — a grenade
        // is one cell and does not stack, where a box of dressings is
        // four cells and five of them (feature 87), so the count fills
        // the grid exactly — so once the day of work is done the helm
        // waits on the bench: nothing is carried to a locker with no
        // room for it, and nothing is lost.
        let spare = world.ship.design.spare(Storage::Locker);
        world.ship.design.cargo[ResourceId::Grenade as usize] += spare;
        world.on_ship_changed();
        assert_eq!(world.ship.design.spare(Storage::Locker), 0);
        let (said, upgraded) = run_until(&mut world, 4 * 24, &|w| w.bench.work.is_none());
        assert!(upgraded, "{said:?}");
        assert_eq!(
            said,
            vec![WorldEvent::Upgraded {
                resource: ResourceId::Helm as u32,
                tier: 2,
            }]
        );
        let Some(Item::Armour(gold)) = world.bench.slots[2] else {
            panic!("{:?}", world.bench);
        };
        assert_eq!(gold.tier, Tier::Two);
        assert_eq!(gold.health, 15.0 * 1.5);
        assert_eq!(gold.id, world.next_piece - 1, "a fresh id");
        for _ in 0..(2 * 60 * 60) {
            world.step(&[]);
        }
        assert!(
            world.bench.slots[2].is_some() && world.bench.carrying.is_none(),
            "waiting for room: {:?}",
            world.bench
        );
        assert_eq!(in_hold(&world, ArmourKind::BasicHelm).len(), 1);
        pieces_agree(&world);

        // Room made, it is carried back, and the pistols follow.
        world.ship.design.cargo[ResourceId::Grenade as usize] = 0;
        world.on_ship_changed();
        let (_, landed) = run_until(&mut world, 4, &|w| {
            in_hold(w, ArmourKind::BasicHelm).len() == 2
        });
        assert!(landed, "{:?}", world.bench);
        pieces_agree(&world);
        let helms = in_hold(&world, ArmourKind::BasicHelm);
        let back = helms
            .iter()
            .find(|p| p.tier == Tier::Two)
            .expect("the new one");
        assert_eq!(back.id, gold.id);
        assert_eq!(back.health, 15.0 * 1.5);
        let (said, begun) = run_until(&mut world, 6, &|w| w.bench.work.is_some());
        assert!(begun, "{said:?}");
        assert_eq!(
            world.bench.work.map(|u| u.resource),
            Some(ResourceId::Handgun),
            "{:?}",
            world.bench
        );
        assert_eq!(world.ship.design.carrying(ResourceId::Handgun), 0);
        pieces_agree(&world);
        guns_agree(&world);
    }

    // --- upgrade_sessions_make_a_day ---
    {
        assert_eq!(
            data::UPGRADE_SESSIONS as f64 * data::UPGRADE_SESSION_MINUTES,
            time::DAY
        );
        assert!(crate::UPGRADE_ORDER as usize > shipdesign::RECIPES.len());
        assert!(ResourceId::ALL.len() < 100);
        let begun = WorldEvent::UpgradeBegun {
            resource: ResourceId::Helm as u32,
            tier: 3,
        };
        assert_eq!(begun.code(), 48);
        assert_eq!(begun.value(), ResourceId::Helm as i64 + 300);
        let done = WorldEvent::Upgraded {
            resource: ResourceId::Schword as u32,
            tier: 2,
        };
        assert_eq!(done.code(), 49);
        assert_eq!(done.value(), ResourceId::Schword as i64 + 200);
        assert_eq!(Refusal::NoWorkbench.code(), 34);
        assert_eq!(Refusal::NoPair.code(), 35);
        assert_eq!(Refusal::BenchBusy.code(), 36);
    }
}

/// The tick box off, the player does it by hand: two pistols fetched
/// into the pack at the armoury and put on the bench at the workbench,
/// one an input slot each — a third refused for want of a slot, a
/// helm refused because it would not pair, a tier-three thing refused
/// because there is nowhere for it to go — the button pressed with the
/// pair there and refused without it, the day worked with nobody
/// carrying anything, and the tier-two pistol taken off the output slot
/// into the pack, the checksum seeing the bench the whole way. A piece
/// put on the bench leaves the world's list and comes back to it, health
/// and all, when it is taken off.
#[test]
fn a_pair_is_put_on_the_bench_by_hand_and_the_button_pressed() {
    use bims::combat::{ArmourKind, Item, Tier, WeaponKind};
    use shipdesign::fixture::playtest_ship;
    let mut world = simulation_world(playtest_ship(), data::SIMULATION_MONEY, 1);
    world.undock_for_probe();
    world.ship.design.cargo[ResourceId::Handgun as usize] += 2;
    world.on_ship_changed();
    assert!(!world.auto_upgrade());

    // Nothing to press the button on.
    assert_eq!(world.can_upgrade(), Err(Refusal::NoPair));
    let events = world.step(&[Command::Upgrade { slot: 0 }]);
    assert!(events.contains(&WorldEvent::Refused {
        slot: 0,
        why: Refusal::NoPair
    }));

    // Two pistols and a helm into the pack.
    at_the_armoury(&mut world, 0);
    let events = world.step(&[
        Command::Fetch {
            slot: 0,
            who: 0,
            kind: crate::FetchKind::Resource(ResourceId::Handgun as u32),
        },
        Command::Fetch {
            slot: 0,
            who: 0,
            kind: crate::FetchKind::Resource(ResourceId::Handgun as u32),
        },
        Command::Fetch {
            slot: 0,
            who: 0,
            kind: crate::FetchKind::Resource(ResourceId::Helm as u32),
        },
    ]);
    unrefused(&events);
    let pack = world.aboard.room.pack(0);
    let pistols: Vec<usize> = (0..pack.len())
        .filter(|&i| pack[i] == Some(Item::Weapon(WeaponKind::LaserPistol.basic())))
        .collect();
    assert_eq!(pistols.len(), 2, "{pack:?}");
    let helm_cell = (0..pack.len())
        .find(|&i| matches!(pack[i], Some(Item::Armour(p)) if p.kind == ArmourKind::BasicHelm))
        .expect("the helm");
    let Some(Item::Armour(helm)) = pack[helm_cell] else {
        unreachable!()
    };
    assert!(world.pieces.iter().any(|p| p.id == helm.id));

    // Out of reach of the bench from the armoury.
    let events = world.step(&[Command::StowOnBench {
        slot: 0,
        who: 0,
        cell: pistols[0] as u32,
    }]);
    assert!(
        events.contains(&WorldEvent::Refused {
            slot: 0,
            why: Refusal::OutOfReach
        }),
        "{events:?}"
    );

    // At the bench: the first pistol on, then the helm refused — it would
    // not pair — then the second pistol on, and a helm still refused, for
    // want of a slot now.
    at_the_workbench(&mut world, 0);
    assert!(world.in_reach_of_bench(0));
    let before = world.checksum();
    let events = world.step(&[Command::StowOnBench {
        slot: 0,
        who: 0,
        cell: pistols[0] as u32,
    }]);
    assert!(
        events.contains(&WorldEvent::Stowed { who: 0 }),
        "{events:?}"
    );
    assert_ne!(world.checksum(), before, "a thing on the bench is seen");
    assert_eq!(
        world.bench.slots[0],
        Some(Item::Weapon(WeaponKind::LaserPistol.basic()))
    );
    assert_eq!(world.can_upgrade(), Err(Refusal::NoPair));
    at_the_workbench(&mut world, 0);
    let events = world.step(&[Command::StowOnBench {
        slot: 0,
        who: 0,
        cell: helm_cell as u32,
    }]);
    assert!(
        events.contains(&WorldEvent::Refused {
            slot: 0,
            why: Refusal::NoPair
        }),
        "{events:?}"
    );
    at_the_workbench(&mut world, 0);
    let events = world.step(&[Command::StowOnBench {
        slot: 0,
        who: 0,
        cell: pistols[1] as u32,
    }]);
    assert!(
        events.contains(&WorldEvent::Stowed { who: 0 }),
        "{events:?}"
    );
    assert_eq!(world.can_upgrade(), Ok(()));
    at_the_workbench(&mut world, 0);
    let events = world.step(&[Command::StowOnBench {
        slot: 0,
        who: 0,
        cell: helm_cell as u32,
    }]);
    assert!(
        events.contains(&WorldEvent::Refused {
            slot: 0,
            why: Refusal::NoRoom
        }),
        "{events:?}"
    );
    // The pistols are nowhere in the hold, and nothing is carried: the
    // tick box is off.
    assert_eq!(world.ship.design.carrying(ResourceId::Handgun), 0);
    assert!(world.aboard.room.ferries().is_empty());
    guns_agree(&world);

    // A piece on the bench leaves the list, and comes back off it with
    // whatever it has left: one pistol off, the helm would go on beside
    // the other pistol — refused, since it is not a pistol — the other
    // pistol off, the helm on alone, dented where it lies, and off again
    // with its dent, back on the list and in the pack.
    at_the_workbench(&mut world, 0);
    let events = world.step(&[Command::Fetch {
        slot: 0,
        who: 0,
        kind: crate::FetchKind::Bench { slot: 1 },
    }]);
    unrefused(&events);
    assert_eq!(world.bench.slots[1], None);
    at_the_workbench(&mut world, 0);
    let events = world.step(&[Command::StowOnBench {
        slot: 0,
        who: 0,
        cell: helm_cell as u32,
    }]);
    assert!(events.contains(&WorldEvent::Refused {
        slot: 0,
        why: Refusal::NoPair
    }));
    at_the_workbench(&mut world, 0);
    let events = world.step(&[Command::Fetch {
        slot: 0,
        who: 0,
        kind: crate::FetchKind::Bench { slot: 0 },
    }]);
    unrefused(&events);
    assert!(world.bench.is_empty());
    at_the_workbench(&mut world, 0);
    let events = world.step(&[Command::StowOnBench {
        slot: 0,
        who: 0,
        cell: helm_cell as u32,
    }]);
    unrefused(&events);
    assert!(world.pieces.iter().all(|p| p.id != helm.id), "off the list");
    match world.bench.slots[0].as_mut() {
        Some(Item::Armour(p)) if p.id == helm.id => p.health = 7.0,
        other => panic!("{other:?}"),
    }
    at_the_workbench(&mut world, 0);
    let events = world.step(&[Command::Fetch {
        slot: 0,
        who: 0,
        kind: crate::FetchKind::Bench { slot: 0 },
    }]);
    unrefused(&events);
    let back = world
        .pieces
        .iter()
        .find(|p| p.id == helm.id)
        .expect("back on the list");
    assert_eq!(back.health, 7.0);
    assert!(matches!(back.at, crate::Where::Pack { who: 0, .. }));
    let dented = world.aboard.room.pack(0)[helm_cell];
    assert!(
        matches!(dented, Some(Item::Armour(p)) if p.health == 7.0),
        "{dented:?}"
    );
    pieces_agree(&world);

    // The pair on again, and the button.
    let pack = world.aboard.room.pack(0);
    let pistols: Vec<usize> = (0..pack.len())
        .filter(|&i| pack[i] == Some(Item::Weapon(WeaponKind::LaserPistol.basic())))
        .collect();
    assert_eq!(pistols.len(), 2);
    for cell in pistols {
        at_the_workbench(&mut world, 0);
        let events = world.step(&[Command::StowOnBench {
            slot: 0,
            who: 0,
            cell: cell as u32,
        }]);
        unrefused(&events);
    }
    let events = world.step(&[Command::Upgrade { slot: 0 }]);
    assert!(
        events.contains(&WorldEvent::UpgradeBegun {
            resource: ResourceId::Handgun as u32,
            tier: 2,
        }),
        "{events:?}"
    );
    assert_eq!(world.can_upgrade(), Err(Refusal::BenchBusy));
    let events = world.step(&[Command::Upgrade { slot: 0 }]);
    assert!(events.contains(&WorldEvent::Refused {
        slot: 0,
        why: Refusal::BenchBusy
    }));
    let (said, done) = run_until(&mut world, 4 * 24, &|w| w.bench.work.is_none());
    assert!(done, "{said:?}");
    assert_eq!(
        world.bench.slots[2],
        Some(Item::Weapon(WeaponKind::LaserPistol.at(Tier::Two)))
    );
    // It waits there — the box is off — and comes off into the pack.
    for _ in 0..600 {
        world.step(&[]);
    }
    assert!(world.aboard.room.ferries().is_empty());
    assert!(world.bench.slots[2].is_some());
    at_the_workbench(&mut world, 0);
    let events = world.step(&[Command::Fetch {
        slot: 0,
        who: 0,
        kind: crate::FetchKind::Bench { slot: 2 },
    }]);
    unrefused(&events);
    assert!(world.bench.is_empty());
    assert!(
        world
            .aboard
            .room
            .pack(0)
            .contains(&Some(Item::Weapon(WeaponKind::LaserPistol.at(Tier::Two))))
    );
    assert_eq!(world.ship.design.carrying(ResourceId::Handgun), 0);
    guns_agree(&world);
}

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
