//! What the world has to be true of.
//!
//! Most of these fly the real fixture in the real spawn system, because
//! almost everything here is about how the parts fit together rather than
//! about arithmetic — the arithmetic has its own tests next door in `flight`.
//! Where a scenario would otherwise depend on where the generator happened to
//! put a planet, the probe seams on [`World`] are used instead: see
//! `discover_for_probe` and `put_for_probe`.

use bims::combat::Tier;
use economy::{Money, Storage, trade_price};
use physics::ResourceId;
use shipdesign::fixture::flyer;
use shipdesign::parts::PartKind;
use shipdesign::{Budget, Edit, Rotation, ShipDesign, apply, build_from_cargo};
use worldgen::math::{DVec2, dvec2};
use worldgen::{GalaxyType, StationKind};

use crate::data;
use crate::event::{Refusal, WorldEvent};
use crate::fixture::{REFERENCE_MONEY, crewed_world, reference_target, simulation_world};
use crate::frame::Frame;
use crate::mining::{self, MiningSite, Rock, RockTile};
use crate::speed::Speed;
use crate::station::Station;
use crate::world::{Command, ShipState, World};
use crate::world_checksum;
use crate::{PlanError, Target};

fn close(a: f64, b: f64) -> bool {
    (a - b).abs() <= 1e-6 * a.abs().max(b.abs()).max(1.0)
}

/// A world with a named amount of money, for the trading tests.
fn world_with(design: ShipDesign, money: Money, players: u32) -> World {
    simulation_world(design, money, players)
}

fn basic() -> World {
    world_with(flyer(2), REFERENCE_MONEY, 2)
}

/// **No dressings on anybody, and none asked for** (feature 87): every
/// crew member starts with a box in the first cells of its pack that
/// fits one, and fills back up out of the hold out of combat. That is
/// exactly what a test which lays a pack out by hand, counts the
/// lockers or pins the hold's `Bandage` count does not want in the way,
/// so it says so at the top and the restock leaves everybody alone.
pub(crate) fn without_dressings(world: &mut World) {
    world
        .aboard
        .room
        .set_target(bims::manager::Stock::Bandages, 0);
    for who in 0..world.aboard.room.crew_count() as usize {
        world.aboard.room.set_bandages_for_probe(who, 0);
    }
    // And none in the hold either: a dock or an undock builds the room
    // afresh with the manager's own numbers back — the food's targets
    // reset the same way — so an empty hold is what actually holds the
    // packs empty across one. A test that wants dressings aboard puts
    // them back after this and does not dock.
    world.ship.design.cargo[ResourceId::Bandage as usize] = 0;
    world.on_ship_changed();
}

/// [`basic`] with Kate a crewmate nobody steers rather than a second
/// player's own: one player, two crew. What a test of the crew under the
/// alarm wants — a player's own is never mustered (`bims::order`).
fn with_a_crewmate() -> World {
    crewed_world(flyer(2), REFERENCE_MONEY, 1, 2)
}

/// Somewhere near enough for a trip to finish inside a test.
///
/// A real node in the spawn system is days away, and a test that stepped
/// through one would be stepping five million times. A point target is the
/// same code path with a shorter line.
fn nearby(world: &World, distance: f64) -> Target {
    Target::Point(world.ship.position().add(dvec2(distance, distance / 3.0)))
}

/// Run until the ship is at rest — holding or tied up — or give up. Coming
/// alongside at the end of a trip is part of the trip here.
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

/// Take every one of a station's people but `who` out of the fight — a
/// shot to the head each — so a run is one on one. An enemy dock arms
/// `enemies_of` the crew (four against `basic()`'s two), and a test that
/// pins one shot or one blade wants the rest of the garrison down first.
fn one_on_one(room: &mut bims::game::Game, who: usize) {
    for other in 0..room.crew_count() as usize {
        if other != who {
            room.kill_for_probe(other);
        }
    }
}

/// How many steps a departure from the berth is allowed: the station's
/// people walking ashore, the crew back, and the push-off. An hour, the
/// casting-off limit, and a little over.
const LEAVING: u32 = 62 * 60;

/// Confirm a trip from wherever the ship is, with that player's crew member
/// at the helm, and run until it is under way. From a berth that is the
/// whole of casting off — everybody to their own side of the airlock, the
/// push-off — which is what every trip in here begins with; from a hold it
/// is one step. Everything seen on the way comes back.
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
    assert_eq!(world.power().engines, 0.0);

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
        let target = |w: &World| nearby(w, 20_000.0);
        let confirm_at = 37u32;
        let total = 400u32;

        let run = |group: u32| {
            let mut world = basic();
            let target = target(&world);
            let mut taken = 0;
            while taken < total {
                let batch = group.min(total - taken);
                for _ in 0..batch {
                    let commands: Vec<Command> = if taken == confirm_at {
                        vec![Command::Confirm { slot: 0, target }]
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
            assert!(close(world.clock_minutes, data::STEP_MINUTES * i as f64));
            assert_eq!(world.steps, i as u64);
        }
        // And the day shown is the crew's: the room counts from day 1 and
        // opens at the waking hour, so the clock panel and the schedule strip
        // agree on what hour it is.
        assert_eq!(world.day(), 1);
        let expected = 8.0 * time::HOUR + data::STEP_MINUTES * 10.0;
        assert!(
            (world.minutes_into_day() - expected).abs() < 1e-3,
            "{} vs {expected}",
            world.minutes_into_day()
        );
    }
}

/// Stepping the ship through a whole trip puts it where the closed-form plan
/// said it would be, at the time the plan said. If those two ever disagree,
/// every promise the helm panel makes is a lie.
#[test]
fn stepping_a_trip_lands_where_the_plan_said_it_would() {
    let mut world = basic();
    let target = nearby(&world, 4_000.0);
    set_off(&mut world, 0, target);

    // The departure is the clock reading the step the plan was made in
    // began at — the push-off ending and the trip beginning are the same
    // instant, and commands are applied before the clock advances.
    let ShipState::Travelling { plan, departed } = &world.ship.state else {
        panic!("the trip should have been planned");
    };
    let (arrival, duration, departed) = (plan.arrival, plan.duration(), *departed);

    until_stopped(&mut world, 200_000);
    assert!(
        world.ship.position().distance(arrival) < 1.0,
        "ended {} units from the arrival point",
        world.ship.position().distance(arrival),
    );
    // Within one step of the quoted time: the loop cannot stop mid-step, so
    // the arrival lands on the first step at or past it.
    let took = world.clock_minutes - departed;
    assert!(
        took >= duration && took < duration + data::STEP_MINUTES * 2.0,
        "took {took} minutes against a quote of {duration}",
    );
    assert_eq!(world.ship.state, ShipState::Holding);
}

// --- what a ship change does, and does not do -------------------------------

/// The conservation contract, in the world rather than in `shipdesign`:
/// welding a wall out of the hold changes where the weight is and not how
/// much of it there is, and it does not move the hull a millimetre.
#[test]
fn building_a_part_out_of_the_hold_moves_no_mass_and_no_hull() {
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
            resource: ResourceId::Metal,
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
    world.ship.design = build_from_cargo(
        &world.ship.design,
        Edit::Place {
            kind: PartKind::Wall,
            origin: (16, 16),
            rotation: Rotation::R0,
        },
    )
    .expect("the hold had the metal for a wall");
    world.on_ship_changed();

    assert!(
        close(world.ship.dynamics.mass.get(), before_mass),
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

/// A design change mid-flight does not move an arrival that has already been
/// promised. The trip keeps the dynamics it was planned with; the *next* one
/// gets the new ones.
#[test]
fn changing_the_ship_under_way_does_not_move_the_arrival() {
    let mut world = basic();
    let target = nearby(&world, 12_000.0);
    set_off(&mut world, 0, target);
    let (arrival, duration) = {
        let plan = world.plan().unwrap();
        (plan.arrival, plan.duration())
    };
    let was = world.ship.dynamics;

    for _ in 0..500 {
        world.step(&[]);
    }
    // A wall, welded on out of nowhere — the point is the dynamics changing,
    // not where the metal came from.
    world.ship.design = apply(
        &world.ship.design,
        &Budget::new(10_000_000),
        Edit::Place {
            kind: PartKind::Wall,
            origin: (16, 16),
            rotation: Rotation::R0,
        },
    )
    .unwrap();
    world.on_ship_changed();
    assert_ne!(
        world.ship.dynamics, was,
        "the ship should fly differently now"
    );

    let plan = world.plan().unwrap();
    assert_eq!(
        plan.dynamics, was,
        "the trip kept the ship it was planned for"
    );
    assert!(close(plan.duration(), duration));
    assert!(plan.arrival.distance(arrival) < 1e-9);
}

/// The other half of the same rule: the reactor is what the engines run
/// on, so it cannot come off in flight either.
#[test]
fn engines_thrusters_and_the_reactor_are_not_to_be_touched_in_flight() {
    // --- engines_and_thrusters_are_not_to_be_touched_in_flight ---
    {
        let mut world = basic();
        for kind in [PartKind::Engine, PartKind::HeavyEngine, PartKind::Thruster] {
            assert!(world.can_modify_part(kind), "{kind:?} while docked");
        }

        let target = nearby(&world, 12_000.0);
        set_off(&mut world, 0, target);
        for kind in [PartKind::Engine, PartKind::HeavyEngine, PartKind::Thruster] {
            assert!(!world.can_modify_part(kind), "{kind:?} while travelling");
        }
        // Everything else is still fair game — a bunk does not change a trip.
        assert!(world.can_modify_part(PartKind::Bunk));

        until_stopped(&mut world, 200_000);
        assert!(
            world.can_modify_part(PartKind::Engine),
            "holding, not flying"
        );
    }

    // --- a_reactor_cannot_be_taken_off_in_flight ---
    {
        let mut world = basic();
        assert!(
            world.can_modify_part(PartKind::Reactor),
            "docked, nothing depends on it yet",
        );

        let target = nearby(&world, 12_000.0);
        set_off(&mut world, 0, target);
        assert!(
            !world.can_modify_part(PartKind::Reactor),
            "the engines are running on it",
        );
    }
}

// --- power under way ------------------------------------------------------------

/// There is no fuel. A burn draws the engines' throttled power off the
/// reactor for as long as the engines are lit — the plan's own figure, so
/// it agrees with the exhaust — and a turn, a hold and a berth draw nothing.
/// Nothing leaves the hold for it.
#[test]
fn a_burn_draws_on_the_reactor_and_a_turn_does_not() {
    let mut world = basic();
    let cargo = world.ship.design.cargo;
    let idle = world.power();
    assert_eq!(idle.engines, 0.0);
    assert!(idle.load() > 0.0 && idle.load() < 1.0, "{}", idle.load());
    let wanted = world.ship.dynamics.forward_power;
    assert!(wanted > 0.0);
    assert_eq!(world.ship.dynamics.forward_throttle, 1.0);

    let target = nearby(&world, 40_000.0);
    set_off(&mut world, 0, target);
    let mut burning = false;
    let mut turning = false;
    for _ in 0..20_000 {
        world.step(&[]);
        let ShipState::Travelling { plan, departed } = &world.ship.state else {
            break;
        };
        let effort = flight::effort_at(plan, world.clock_minutes - departed);
        let power = world.power();
        if effort.engines > 0 {
            assert_eq!(power.engines, wanted);
            assert!(power.load() > idle.load());
            burning = true;
        } else {
            assert_eq!(power.engines, 0.0);
            turning = true;
        }
    }
    assert!(burning && turning, "the trip should have burnt and turned");
    until_stopped(&mut world, 200_000);
    assert_eq!(world.power().engines, 0.0);
    assert_eq!(
        world.ship.design.cargo, cargo,
        "nothing was burnt out of the hold"
    );
}

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
        let ShipState::Docked { station } = world.ship.state else {
            panic!("a world opens docked");
        };
        let kind = world.station(station).unwrap().kind;
        assert!(kind.sells(ResourceId::Metal));
        assert!(world.man_the_desk_for_probe(0), "a desk to trade at");

        let events = world.step(&[Command::Buy {
            slot: 0,
            resource: ResourceId::Emitter,
            units: 1,
        }]);
        assert!(refused_with(&events, Refusal::NotSoldHere), "{events:?}");
        assert_eq!(world.ship.design.carrying(ResourceId::Emitter), 0);

        let events = world.step(&[Command::Buy {
            slot: 0,
            resource: ResourceId::Galvum,
            units: 1,
        }]);
        if kind == worldgen::StationKind::MiningOutpost {
            assert_eq!(world.ship.design.carrying(ResourceId::Galvum), 1);
        } else {
            assert!(refused_with(&events, Refusal::NotSoldHere), "{events:?}");
            assert_eq!(world.ship.design.carrying(ResourceId::Galvum), 0);
        }

        // Metal is on every shelf, and what is aboard sells anywhere with
        // somebody to buy it.
        let events = world.step(&[Command::Buy {
            slot: 0,
            resource: ResourceId::Metal,
            units: 1,
        }]);
        assert!(!refused_with(&events, Refusal::NotSoldHere), "{events:?}");
        assert_eq!(world.ship.design.carrying(ResourceId::Metal), 1);
        let money = world.money;
        world.step(&[Command::Sell {
            slot: 0,
            resource: ResourceId::Metal,
            units: 1,
        }]);
        assert_eq!(world.ship.design.carrying(ResourceId::Metal), 0);
        assert!(world.money > money);
    }
}

/// Under way there is nobody to trade with, and trading is refused for
/// that reason and no other.
/// Money works at a dock and nowhere else. That is `shipdesign::materials`'
/// rule, and this is the world keeping it.
#[test]
fn nothing_is_bought_or_sold_under_way_or_away_from_a_station() {
    // --- nothing_is_sold_under_way ---
    {
        let mut world = basic();
        let target = nearby(&world, 4_000.0);
        set_off(&mut world, 0, target);
        let tofu = world.ship.design.carrying(ResourceId::Tofu);
        let events = world.step(&[Command::Sell {
            slot: 0,
            resource: ResourceId::Tofu,
            units: 1,
        }]);
        assert!(refused_with(&events, Refusal::NotDocked));
        assert_eq!(world.ship.design.carrying(ResourceId::Tofu), tofu);
    }

    // --- nothing_is_bought_or_sold_away_from_a_station ---
    {
        let mut world = world_with(flyer(2), 1_000_000, 2);
        let target = nearby(&world, 4_000.0);
        set_off(&mut world, 0, target);

        for command in [
            Command::Buy {
                slot: 0,
                resource: ResourceId::Vegetable,
                units: 1,
            },
            Command::Sell {
                slot: 0,
                resource: ResourceId::Vegetable,
                units: 1,
            },
        ] {
            let events = world.step(&[command]);
            assert!(refused_with(&events, Refusal::NotDocked), "{command:?}");
        }

        until_stopped(&mut world, 200_000);
        assert_eq!(world.ship.state, ShipState::Holding);
        let events = world.step(&[Command::Buy {
            slot: 0,
            resource: ResourceId::Vegetable,
            units: 1,
        }]);
        assert!(
            refused_with(&events, Refusal::NotDocked),
            "holding is not docked either",
        );
    }
}

/// An abort brakes on the engines, drawing what a burn draws, and once the
/// ship is at rest the draw stops with it.
#[test]
fn an_abort_stops_the_draw_when_the_ship_stops() {
    let mut world = basic();
    let target = nearby(&world, 40_000.0);
    set_off(&mut world, 0, target);

    // Into the burn, so there is speed to take off.
    for _ in 0..200_000 {
        world.step(&[]);
        if world.effort().engines > 0 {
            break;
        }
    }
    assert!(world.effort().engines > 0, "never got to the burn");
    world.man_the_helm_for_probe(1);
    let events = world.step(&[Command::Abort { slot: 1 }]);
    assert!(
        events
            .iter()
            .any(|e| matches!(e, WorldEvent::Aborted { slot: 1 })),
        "{events:?}",
    );
    let ShipState::Travelling { plan, .. } = &world.ship.state else {
        panic!("stopping is still travelling");
    };
    assert!(plan.aborting);
    assert!(
        plan.segments
            .iter()
            .any(|s| s.engines > 0 && s.power == world.ship.dynamics.forward_power),
        "the stop brakes on the engines"
    );

    until_stopped(&mut world, 200_000);
    assert_eq!(world.ship.state, ShipState::Holding);
    assert_eq!(world.power().engines, 0.0);
    assert_eq!(world.ship.destination_set_by, None);
}

/// An engine with no reactor behind it pushes nothing, so a ship whose
/// reactor has gone has, as far as a trip is concerned, no engine.
#[test]
fn a_ship_with_a_dark_engine_is_not_going_anywhere() {
    let mut dark = flyer(2);
    dark.parts.retain(|p| p.kind != PartKind::Reactor);
    let mut world = world_with(dark, REFERENCE_MONEY, 2);
    assert_eq!(world.ship.dynamics.a_forward, 0.0);
    assert_eq!(world.ship.dynamics.forward_engines, 0);
    let target = nearby(&world, 4_000.0);
    assert_eq!(world.preview(target), Err(PlanError::NoForwardEngine));

    // Refused at the berth, before anybody is sent ashore for nothing.
    world.man_the_helm_for_probe(0);
    let events = world.step(&[Command::Confirm { slot: 0, target }]);
    assert!(
        events.iter().any(|e| matches!(
            e,
            WorldEvent::PlanFailed {
                error: PlanError::NoForwardEngine,
                ..
            }
        )),
        "{events:?}",
    );
    assert!(matches!(world.ship.state, ShipState::Docked { .. }));
}

// --- the jump -------------------------------------------------------------------

/// The flyer with a hyperdrive against its engine's port side, on the run
/// that feeds the engine: a ship that can jump.
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
    assert!(shipdesign::hyperdrive::ready(&design));
    design
}

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

/// A jump is another system altogether: the drive charges for its twenty
/// seconds and the ship is then holding in empty space round another star,
/// with the old system's stations, people, chart and site left behind and
/// everything of the ship's own brought along. Docked it is refused, and
/// so is the star the ship is at; a flyer with no drive cannot.
#[test]
fn a_charged_hyperdrive_puts_the_ship_in_another_system() {
    let mut world = world_with(jumper(), REFERENCE_MONEY, 2);
    let from = world.star_id;
    let to = laned_star(&world);
    let money = world.money;
    let cargo = world.ship.design.cargo;
    let heading = world.ship.heading;
    assert!(world.hyperdrive_ready());

    // Docked: not from here.
    world.man_the_helm_for_probe(0);
    let events = world.step(&[Command::Jump { slot: 0, star: to }]);
    assert!(refused_with(&events, Refusal::NotHolding), "{events:?}");

    world.undock_for_probe();
    world.man_the_helm_for_probe(0);
    let events = world.step(&[Command::Jump {
        slot: 0,
        star: from,
    }]);
    assert!(refused_with(&events, Refusal::SameStar), "{events:?}");
    let events = world.step(&[Command::Jump {
        slot: 0,
        star: 1_000_000,
    }]);
    assert!(refused_with(&events, Refusal::NoSuchStar), "{events:?}");
    let events = world.step(&[Command::Jump { slot: 1, star: to }]);
    assert!(refused_with(&events, Refusal::NotAtTheHelm), "{events:?}");

    let events = world.step(&[Command::Jump { slot: 0, star: to }]);
    assert!(
        events
            .iter()
            .any(|e| matches!(e, WorldEvent::Charging { slot: 0, star } if *star == to)),
        "{events:?}"
    );
    assert!(matches!(world.ship.state, ShipState::Charging { .. }));
    let (star, progress) = world.jump_charge().expect("charging");
    assert_eq!(star, to);
    assert!(progress < 0.01);

    // An abort calls the charge off, and the ship is where it was.
    let at = world.ship.position();
    let events = world.step(&[Command::Abort { slot: 0 }]);
    assert!(
        events
            .iter()
            .any(|e| matches!(e, WorldEvent::Aborted { slot: 0 }))
    );
    assert_eq!(world.ship.state, ShipState::Holding);
    assert_eq!(world.star_id, from);
    assert!(world.ship.position().distance(at) < 1e-6);

    // Charged through: the clock, and then somewhere else.
    world.step(&[Command::Jump { slot: 0, star: to }]);
    let steps = (data::JUMP_CHARGE_MINUTES / data::STEP_MINUTES).ceil() as u32;
    let mut jumped = None;
    for i in 0..steps + 5 {
        let events = world.step(&[]);
        if events
            .iter()
            .any(|e| matches!(e, WorldEvent::Jumped { star } if *star == to))
        {
            jumped = Some(i);
            break;
        }
        assert!(
            world.jump_charge().is_some(),
            "step {i}: the charge stopped early"
        );
    }
    let when = jumped.expect("the ship should have jumped");
    assert!(when + 2 >= steps, "jumped at step {when} of {steps}");

    assert_eq!(world.star_id, to);
    assert_eq!(world.ship.state, ShipState::Holding);
    assert!(world.jump_charge().is_none());
    assert_eq!(world.ship.destination_set_by, None);
    assert!(world.residents.is_none());
    assert!(world.sites.is_empty());
    assert_eq!(world.ship.frame, Frame::Space);
    // In empty space: clear of everything the new system holds.
    let here = world.ship.position();
    for node in world.system.nodes() {
        let there = world.system.absolute_position(node).unwrap();
        assert!(there.distance(here) >= data::JUMP_CLEARANCE, "{node:?}");
    }
    // The new system's stations, its hostile ones on the list without the
    // spawn's exemption, and only what the sensors reach on the chart.
    assert_eq!(world.stations.len(), Station::all_of(&world.system).len());
    for s in &world.stations {
        assert_eq!(
            world.hostile.binary_search(&s.id).is_ok(),
            s.hostile,
            "{}",
            s.id
        );
        assert_ne!(world.stance(s.id), bims::sight::Stance::Friendly);
    }
    assert!(world.discovered.len() <= world.system.nodes().len());
    // What is the ship's came along.
    assert_eq!(world.money, money);
    assert_eq!(world.ship.design.cargo, cargo);
    assert_eq!(world.ship.crew_count, 2);
    assert_eq!(world.aboard.crew_count(), 2);
    assert!((world.ship.heading - heading).abs() < 1e-9);
    assert!(world.hyperdrive_ready());

    // And a trip from there is an ordinary trip.
    world.man_the_helm_for_probe(0);
    let target = nearby(&world, 4_000.0);
    assert!(world.preview(target).is_ok());
    set_off(&mut world, 0, target);
    until_stopped(&mut world, 200_000);
    assert_eq!(world.ship.state, ShipState::Holding);
}

/// No drive, no jump — and a drive on a dark run is no drive.
#[test]
fn a_jump_wants_a_working_hyperdrive() {
    let mut world = basic();
    world.undock_for_probe();
    world.man_the_helm_for_probe(0);
    assert!(!world.hyperdrive_ready());
    let to = (world.star_id + 1) % world.galaxy().stars.len() as u32;
    let events = world.step(&[Command::Jump { slot: 0, star: to }]);
    assert!(refused_with(&events, Refusal::NoHyperdrive), "{events:?}");
    assert_eq!(world.ship.state, ShipState::Holding);

    let mut dark = jumper();
    dark.parts
        .retain(|p| !(p.kind == PartKind::PowerConduit && p.origin == (6, 16)));
    assert!(!shipdesign::hyperdrive::ready(&dark));
    let mut world = world_with(dark, REFERENCE_MONEY, 2);
    world.undock_for_probe();
    world.man_the_helm_for_probe(0);
    let events = world.step(&[Command::Jump { slot: 0, star: to }]);
    assert!(refused_with(&events, Refusal::NoHyperdrive), "{events:?}");
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

/// A Confirm while the ship is under way stops it first and then sets off
/// again. Two separate plans, one after the other, and the ship is at rest in
/// between — which is the only shape a trip has, so a redirect does not need
/// a mechanism of its own.
/// Only the latest confirmed target is kept. Two in one step is one trip, and
/// it is the second one's.
/// A redirect whose second leg cannot be flown leaves the ship holding where
/// it stopped, and says why.
#[test]
fn a_confirm_under_way_is_a_redirect_the_later_one_wins_and_one_that_cannot_be_flown_holds() {
    // --- a_confirm_under_way_stops_first_and_then_goes ---
    {
        let mut world = basic();
        let first = nearby(&world, 40_000.0);
        set_off(&mut world, 0, first);
        for _ in 0..4_000 {
            world.step(&[]);
        }
        assert_eq!(world.plan().unwrap().target, first);

        let second = Target::Point(world.ship.position().add(dvec2(-8_000.0, 2_000.0)));
        world.man_the_helm_for_probe(1);
        world.step(&[Command::Confirm {
            slot: 1,
            target: second,
        }]);
        assert!(world.plan().unwrap().aborting, "it should be stopping");
        assert_eq!(world.ship.pending, Some((1, second)));
        assert_eq!(
            world.ship.destination_set_by,
            Some(1),
            "the route is theirs now"
        );

        // It comes to rest, and the moment it does the second trip begins — with
        // no gap, and without the player having to ask twice.
        let mut set_off = false;
        for _ in 0..200_000 {
            let events = world.step(&[]);
            if events
                .iter()
                .any(|e| matches!(e, WorldEvent::Departed { .. }))
            {
                set_off = true;
                break;
            }
        }
        assert!(set_off, "the redirect never set off");
        assert_eq!(world.plan().unwrap().target, second);
        assert!(!world.plan().unwrap().aborting);
        assert_eq!(world.ship.pending, None);

        until_stopped(&mut world, 400_000);
        let want = match second {
            Target::Point(at) => at,
            _ => unreachable!(),
        };
        assert!(world.ship.position().distance(want) < 1.0);
    }

    // --- two_confirms_in_one_step_keep_the_later_one ---
    {
        let mut world = basic();
        let first = nearby(&world, 40_000.0);
        let second = Target::Point(world.ship.position().add(dvec2(-30_000.0, 0.0)));
        world.man_the_helm_for_probe(0);
        world.man_the_helm_for_probe(1);
        world.step(&[
            Command::Confirm {
                slot: 0,
                target: first,
            },
            Command::Confirm {
                slot: 1,
                target: second,
            },
        ]);

        // The first began the departure and the second changed where it is
        // going: the ship is still casting off, and the target it will set off
        // for once it is clear is the second one's.
        assert_eq!(world.ship.destination_set_by, Some(1));
        let going_to = world.ship.pending.map(|(_, t)| t).unwrap_or_else(|| {
            let plan = world.plan().expect("it should be flying somewhere");
            assert!(!plan.aborting, "an abort with nothing pending goes nowhere");
            plan.target
        });
        assert_eq!(going_to, second);
        assert_ne!(going_to, first);
    }

    // --- a_redirect_that_cannot_be_flown_holds_and_says_so ---
    {
        let mut world = basic();
        let first = nearby(&world, 40_000.0);
        set_off(&mut world, 0, first);
        for _ in 0..4_000 {
            world.step(&[]);
        }
        // Somewhere nobody has seen. A node the generator put in this system but
        // that is beyond the ship's sensors — with the chart forgotten first,
        // because a system opens fully charted.
        world.uncharted_for_probe();
        let unseen = world
            .system
            .nodes()
            .into_iter()
            .find(|n| !world.discovered.contains(n));
        let Some(unseen) = unseen else {
            return; // this seed's system is small enough to see all at once
        };
        let target = match unseen {
            worldgen::Node::Body(id) => Target::Body(id),
            worldgen::Node::Station(id) => Target::Station(id),
        };
        let events = world.step(&[Command::Confirm { slot: 0, target }]);
        assert!(
            events.iter().any(|e| matches!(
                e,
                WorldEvent::PlanFailed {
                    error: PlanError::TargetUndiscovered,
                    ..
                }
            )),
            "{events:?}",
        );
        // And the first trip carries on, untouched.
        assert_eq!(world.plan().unwrap().target, first);
        assert!(!world.plan().unwrap().aborting);
    }
}

// --- trading ---------------------------------------------------------------------

#[test]
fn buying_costs_money_and_makes_the_ship_heavier() {
    let mut world = world_with(flyer(2), 100_000, 2);
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
    let events = world.step(&[Command::Buy {
        slot: 1,
        resource: ResourceId::Metal,
        units: 10,
    }]);

    assert!(events.iter().any(|e| matches!(
        e,
        WorldEvent::Traded {
            resource: ResourceId::Metal,
            units: 10,
            ..
        }
    )));
    // At the desk's ask — the station's kind's lean on the book with the
    // spawn's own lean forced to nothing, then half the spread over it —
    // and never at the book itself.
    let desk = world.station(world.home).unwrap().market().unwrap();
    assert_eq!(desk.bias, economy::market::Bias::NONE, "the spawn leans");
    let quote = desk.quote(ResourceId::Metal);
    assert_ne!(quote.ask, trade_price(ResourceId::Metal));
    assert_eq!(world.money, money - 10 * quote.ask);
    assert_eq!(world.ship.design.carrying(ResourceId::Metal), 10);
    assert!(close(
        world.ship.dynamics.mass.get(),
        mass + 10.0 * ResourceId::Metal.mass_per_unit(),
    ));

    // And selling puts it back at the bid — by whoever is at the desk —
    // which is under the ask: a buy and a sell at one desk lose money.
    assert!(world.man_the_desk_for_probe(0));
    world.step(&[Command::Sell {
        slot: 0,
        resource: ResourceId::Metal,
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
        world.ship.design.cargo[ResourceId::Metal as usize] = 5;
        let events = world.step(&[Command::Sell {
            slot: 0,
            resource: ResourceId::Metal,
            units: 1,
        }]);
        assert!(refused_with(&events, Refusal::NoMarket));
        let events = world.step(&[Command::Buy {
            slot: 0,
            resource: ResourceId::Metal,
            units: 1,
        }]);
        assert!(refused_with(&events, Refusal::NotSoldHere));
    }
}

#[test]
fn what_cannot_be_paid_for_or_stowed_is_refused() {
    let mut world = world_with(flyer(2), 100, 2);
    assert!(world.man_the_desk_for_probe(0), "a desk to trade at");
    // No money. A staple, which every shelf carries: whether the spawn
    // rolled components is the generator's business, not this test's.
    let events = world.step(&[Command::Buy {
        slot: 0,
        resource: ResourceId::Metal,
        units: 10,
    }]);
    assert!(refused_with(&events, Refusal::Unaffordable));

    // Money, but nowhere to put it: the flyer has no racking.
    world.money = 1_000_000;
    assert_eq!(world.ship.design.capacity(Storage::Shelf), 0);
    let events = world.step(&[Command::Buy {
        slot: 0,
        resource: ResourceId::Metal,
        units: 1,
    }]);
    assert!(refused_with(&events, Refusal::NoRoomAboard));

    // And selling what is not there.
    let events = world.step(&[Command::Sell {
        slot: 0,
        resource: ResourceId::Ore,
        units: 1,
    }]);
    assert!(refused_with(&events, Refusal::NotAboard));
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
        let target = nearby(&world, 40_000.0);
        world.man_the_helm_for_probe(0);

        // The Confirm is inside the loop, because the frame change it causes —
        // leaving the dock's local frame — happens during it. Counting from
        // afterwards would count nothing and the test would be asserting that
        // nothing happened.
        let mut changes = 0;
        let mut last = world.clock_minutes;
        for i in 0..8_000 {
            let commands: Vec<Command> = if i == 0 {
                vec![Command::Confirm { slot: 0, target }]
            } else {
                Vec::new()
            };
            let events = world.step(&commands);
            assert!(
                close(world.clock_minutes - last, data::STEP_MINUTES),
                "a step was not a step",
            );
            last = world.clock_minutes;
            assert_eq!(world.effective_speed(), speed);
            changes += events
                .iter()
                .filter(|e| matches!(e, WorldEvent::FrameChanged { .. }))
                .count();
        }
        assert!(
            changes > 0,
            "leaving the dock should have changed the frame"
        );
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

/// The ship is flown from the helm: a Confirm from a player whose crew
/// member is standing anywhere else is refused, and one whose crew member
/// is at the seat begins the departure. Any of the crew, once there.
#[test]
fn the_ship_is_flown_from_the_helm() {
    let mut world = basic();
    let seat = world.helm_spot().expect("the flyer has a helm");
    assert!(!world.can_command(0), "nobody starts at the helm");
    assert!(!world.can_command(1));
    let target = nearby(&world, 12_000.0);
    let events = world.step(&[Command::Confirm { slot: 1, target }]);
    assert!(refused_with(&events, Refusal::NotAtTheHelm), "{events:?}");
    assert!(matches!(world.ship.state, ShipState::Docked { .. }));

    // Walking there — the order the page's button gives — puts the crew
    // member within reach of the seat and leaves them standing there.
    assert!(world.order_to_helm(1), "no route to the helm");
    let mut there = false;
    for _ in 0..LEAVING {
        world.step(&[]);
        if world.at_the_helm(1) {
            there = true;
            break;
        }
    }
    assert!(there, "never reached the helm");
    assert!(world.aboard.position(1).distance(seat) <= data::HELM_REACH);
    assert!(world.can_command(1));
    assert!(!world.can_command(0), "the other one is still elsewhere");
    assert!(!world.can_command(5), "there are only two of them");

    let events = world.step(&[Command::Confirm { slot: 1, target }]);
    assert!(
        events
            .iter()
            .any(|e| matches!(e, WorldEvent::CastingOff { slot: 1 })),
        "{events:?}"
    );
    assert_eq!(world.ship.destination_set_by, Some(1));
    // And they stay at the helm rather than wandering off it.
    for _ in 0..600 {
        world.step(&[]);
    }
    assert!(world.at_the_helm(1), "wandered off the helm");
}

/// Only a living, waking crew member flies the ship. A body that died
/// beside the seat is within reach of it and at the helm for nothing; so
/// is one that dropped off there, until it comes round. And the check is
/// made when a command lands, not on the way: one who walks off the helm
/// under way is refused the *next* order, and the trip carries on.
#[test]
fn only_a_living_waking_crew_member_is_at_the_helm() {
    // --- a_dead_body_at_the_seat_is_not_at_the_helm ---
    {
        let mut world = basic();
        let seat = world.helm_spot().expect("the flyer has a helm");
        world.man_the_helm_for_probe(1);
        assert!(world.at_the_helm(1));
        world.aboard.room.kill_for_probe(1);
        world.step(&[]);
        assert!(!world.aboard.room.is_alive(1), "dead on the next tick");
        assert!(
            world.aboard.position(1).distance(seat) <= data::HELM_REACH,
            "the body lies where it stood"
        );
        assert!(!world.at_the_helm(1), "a body is not at the helm");
        assert!(!world.can_command(1));
        let target = nearby(&world, 12_000.0);
        let events = world.step(&[Command::Confirm { slot: 1, target }]);
        assert!(refused_with(&events, Refusal::NotAtTheHelm), "{events:?}");
        assert!(matches!(world.ship.state, ShipState::Docked { .. }));
    }
    // --- a_sleeping_one_is_not_either_until_it_wakes ---
    {
        let mut world = basic();
        world.man_the_helm_for_probe(0);
        assert!(world.can_command(0));
        world.aboard.room.nod_off_for_probe(0, 15.0);
        assert!(world.aboard.room.is_asleep(0));
        assert!(!world.at_the_helm(0), "asleep at the seat");
        let target = nearby(&world, 12_000.0);
        let events = world.step(&[Command::Confirm { slot: 0, target }]);
        assert!(refused_with(&events, Refusal::NotAtTheHelm), "{events:?}");
        assert!(matches!(world.ship.state, ShipState::Docked { .. }));
        // A quarter of an hour on it has come round, still posted there.
        let mut awake = false;
        for _ in 0..(16.0 / data::STEP_MINUTES) as usize {
            world.step(&[]);
            if !world.aboard.room.is_asleep(0) {
                awake = true;
                break;
            }
        }
        assert!(awake, "never came round");
        assert!(world.can_command(0), "awake at the seat again");
    }
    // --- one_who_walks_away_is_refused_the_next_order_not_mid_trip ---
    {
        let mut world = basic();
        let target = nearby(&world, 12_000.0);
        set_off(&mut world, 0, target);
        assert!(matches!(world.ship.state, ShipState::Travelling { .. }));
        assert!(world.at_the_helm(0));
        // Off the seat: stood down and put at the other crew member's
        // bunk, which is nowhere near the helm.
        world.stand_down(0);
        let away = world.aboard.room.bed_station_for_probe(1);
        world.aboard.room.put_for_probe(0, away);
        assert!(!world.at_the_helm(0), "put clear of the helm");
        // The trip is not the helm's to keep: it carries on.
        for _ in 0..60 {
            world.step(&[]);
        }
        assert!(
            matches!(world.ship.state, ShipState::Travelling { .. }),
            "the trip went on without anybody at the helm: {:?}",
            world.ship.state
        );
        // But the next order from that player is refused — put there again
        // first, since a Bim under way walks back to the seat on its own.
        world.aboard.room.put_for_probe(0, away);
        let events = world.step(&[Command::Abort { slot: 0 }]);
        assert!(refused_with(&events, Refusal::NotAtTheHelm), "{events:?}");
        assert!(matches!(world.ship.state, ShipState::Travelling { .. }));
        // And back at the seat it goes through.
        world.man_the_helm_for_probe(0);
        let events = world.step(&[Command::Abort { slot: 0 }]);
        assert!(!refused_with(&events, Refusal::NotAtTheHelm), "{events:?}");
    }
}

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

/// Leaving a station is three things in order: the crew come back aboard
/// (the station's people were never on the ship — they keep their own
/// room), the rooms come apart and the ship pushes straight off the berth, and only then is the trip planned — from
/// where the push-off ended, so the turn towards the target is the plan's
/// own align phase. Nobody is teleported off the ship.
#[test]
fn leaving_a_station_sends_everybody_home_and_pushes_off_before_the_trip() {
    let mut world = basic();
    let ShipState::Docked { station } = world.ship.state else {
        panic!("not docked");
    };
    let berth = world.berth_at(station).unwrap();
    let outward = world.station(station).unwrap().face().unwrap().1;
    let target = nearby(&world, 40_000.0);
    let visitor = send_a_crew_member_ashore(&mut world);

    world.man_the_helm_for_probe(0);
    let events = world.step(&[Command::Confirm { slot: 0, target }]);
    assert!(
        events
            .iter()
            .any(|e| matches!(e, WorldEvent::CastingOff { slot: 0 })),
        "{events:?}"
    );
    assert!(matches!(world.ship.state, ShipState::CastingOff { .. }));
    assert!(world.aboard.is_joined(), "the rooms came apart too soon");
    assert_eq!(world.ship.pending, Some((0, target)));

    // The crew member ashore walks back; the ship waits at the berth until
    // they are on it, and then it is a room of its own.
    let mut cast_off = None;
    for i in 0..LEAVING {
        assert!(
            world.ship.position().distance(berth.position) < 1e-6,
            "moved while still casting off"
        );
        if i == 0 {
            assert!(!world.aboard.on_ship(visitor, &world.ship.design));
        }
        let events = world.step(&[]);
        if events
            .iter()
            .any(|e| matches!(e, WorldEvent::Undocking { .. }))
        {
            cast_off = Some(i);
            break;
        }
    }
    let cast_off = cast_off.expect("never cast off");
    assert!(cast_off > 0, "the walk back takes time");
    assert!(
        world.aboard.on_ship(visitor, &world.ship.design),
        "cast off with a crew member still ashore"
    );
    assert!(matches!(world.ship.state, ShipState::Undocking { .. }));
    assert!(!world.aboard.is_joined());
    assert_eq!(world.aboard.count(), 2, "somebody came along");
    assert!(world.aboard.everybody_home(&world.ship.design));

    // The push-off: straight out the way the door opens, further every
    // step, heading untouched, and the trip planned from where it ends.
    let mut last = 0.0;
    for _ in 0..LEAVING {
        world.step(&[]);
        if matches!(world.ship.state, ShipState::Travelling { .. }) {
            break;
        }
        let away = world.ship.position().sub(berth.position);
        let along = away.x * outward.x + away.y * outward.y;
        assert!(along >= last - 1e-9, "the ship came back");
        assert!((away.length() - along).abs() < 1e-6, "not straight out");
        assert_eq!(world.ship.heading, berth.heading, "turned before clear");
        last = along;
    }
    let ShipState::Travelling { plan, .. } = &world.ship.state else {
        panic!("never set off: {:?}", world.ship.state);
    };
    assert!(last > 0.0, "never pushed off");
    assert!(plan.start.distance(world.ship.position()) < 1e-6);
    assert!(
        plan.start.distance(berth.position) > 5.0 * shipdesign::TILE as f64,
        "planned from the berth"
    );
    assert_eq!(world.ship.pending, None);
}

/// Called off while somebody is still walking back aboard, the ship stays tied
/// up; called off during the push-off, it holds where the push-off ends.
#[test]
fn a_departure_can_be_called_off() {
    let mut world = basic();
    let ShipState::Docked { station } = world.ship.state else {
        panic!("not docked");
    };
    let target = nearby(&world, 40_000.0);
    send_a_crew_member_ashore(&mut world);
    world.man_the_helm_for_probe(0);
    world.step(&[Command::Confirm { slot: 0, target }]);
    assert!(matches!(world.ship.state, ShipState::CastingOff { .. }));
    let events = world.step(&[Command::Abort { slot: 0 }]);
    assert!(
        events
            .iter()
            .any(|e| matches!(e, WorldEvent::Aborted { slot: 0 }))
    );
    assert_eq!(world.ship.state, ShipState::Docked { station });
    assert!(world.aboard.is_joined());
    assert_eq!(world.ship.pending, None);
    assert_eq!(world.ship.destination_set_by, None);

    world.step(&[Command::Confirm { slot: 0, target }]);
    for _ in 0..LEAVING {
        world.step(&[]);
        if matches!(world.ship.state, ShipState::Undocking { .. }) {
            break;
        }
    }
    assert!(matches!(world.ship.state, ShipState::Undocking { .. }));
    world.step(&[Command::Abort { slot: 0 }]);
    until_stopped(&mut world, LEAVING);
    assert_eq!(world.ship.state, ShipState::Holding);
    assert_eq!(world.ship.destination_set_by, None);
    assert!(world.ship.pending.is_none());
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
    // Sixteen rows of ten: two for the suit locker, eight for the armoury,
    // six for the drug lab. Everything aboard is laid, and covers what the
    // class counts.
    assert_eq!(world.grid_capacity(Storage::Locker), 16 * GRID_COLS);
    assert_eq!(crate::Grid::rows(world.grid_capacity(Storage::Locker)), 16);
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
        Command::Arrange {
            slot: 0,
            class: Storage::Locker.code(),
            id: sniper.id,
            x: 9,
            y: 7,
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
    grid.settle(capacity, &[Wanted::Units(ResourceId::Ore, 25, one)]);
    let counts = |grid: &Grid| -> Vec<u32> { grid.slots.iter().map(|s| s.count).collect() };
    assert_eq!(counts(&grid), vec![10, 10, 5]);
    assert_eq!(grid.units_of(ResourceId::Ore), 25);
    assert!(grid.can_take(capacity, ResourceId::Ore, 175, one));
    assert!(
        !grid.can_take(capacity, ResourceId::Ore, 176, one),
        "twenty cells"
    );
    grid.settle(capacity, &[Wanted::Units(ResourceId::Ore, 30, one)]);
    assert_eq!(counts(&grid), vec![10, 10, 10]);
    grid.settle(capacity, &[Wanted::Units(ResourceId::Ore, 23, one)]);
    assert_eq!(counts(&grid), vec![10, 10, 3]);
    grid.settle(capacity, &[Wanted::Units(ResourceId::Ore, 12, one)]);
    assert_eq!(counts(&grid), vec![10, 2]);
    let first = grid.slots[0].id;
    assert_eq!(
        grid.remove(ResourceId::Ore, 4, Some(first)),
        4,
        "off the stack asked for"
    );
    assert_eq!(counts(&grid), vec![6, 2]);
    grid.settle(capacity, &[Wanted::Units(ResourceId::Ore, 0, one)]);
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

/// The shelves and the cold store are grids of stacks: the playtest ore
/// is four stacks of ten on four cells, the tofu two blocks of four by
/// four; a fetch of a slot takes one off *that* stack; a sale empties the
/// last stack first; and a block that the cold store has area for but no
/// four-by-four run of cells for is refused.
#[test]
fn the_shelves_hold_stacks_and_a_fetch_takes_one_off_the_stack_asked_for() {
    use crate::{FetchKind, Kept};
    use shipdesign::fixture::playtest_ship;
    let mut world = simulation_world(playtest_ship(), data::SIMULATION_MONEY, 1);
    let shelves = world.grid(Storage::Shelf).unwrap();
    let ore: Vec<crate::Slot> = shelves
        .slots
        .iter()
        .filter(|s| s.kept == Kept::Stack(ResourceId::Ore))
        .copied()
        .collect();
    assert_eq!(
        ore.iter().map(|s| s.count).collect::<Vec<u32>>(),
        vec![10; 4]
    );
    assert_eq!(shelves.units_of(ResourceId::Components), 40);
    assert_eq!(
        shelves
            .slots
            .iter()
            .filter(|s| s.kept == Kept::Stack(ResourceId::Components))
            .count(),
        2,
        "twenty to a stack"
    );
    // Four ore, six metal, two components: twelve cells of two hundred.
    assert_eq!(world.ship.design.stored(Storage::Shelf), 12);
    assert_eq!(shelves.covered(), 12);
    let cold = world.grid(Storage::ColdStore).unwrap();
    assert_eq!(
        world.ship.design.stored(Storage::ColdStore),
        4 * 2 + 2 * 16 + 1
    );
    assert_eq!(cold.covered(), world.ship.design.stored(Storage::ColdStore));
    // Area for three more blocks of tofu, cells in a run for two.
    assert!(world.ship.design.has_room(ResourceId::Tofu, 30), "by area");
    assert!(world.has_room(ResourceId::Tofu, 20));
    assert!(!world.has_room(ResourceId::Tofu, 30), "not by the grid");
    assert_eq!(world.room_for(ResourceId::Tofu, 30), 20);

    // One off the second stack, by its slot, standing at a shelf: that
    // stack is nine, the first still ten.
    let second = ore[1].id;
    let shelf = world
        .aboard
        .room
        .container_spot(bims::game::Container::Shelf(0))
        .expect("a shelf with a use spot");
    world.aboard.room.put_for_probe(0, shelf);
    let events = world.step(&[Command::Fetch {
        slot: 0,
        who: 0,
        kind: FetchKind::Slot {
            class: Storage::Shelf.code(),
            id: second,
        },
    }]);
    assert!(
        !refused_with(&events, Refusal::OutOfReach) && !refused_with(&events, Refusal::NotAboard),
        "{events:?}"
    );
    assert_eq!(world.ship.design.carrying(ResourceId::Ore), 39);
    let shelves = world.grid(Storage::Shelf).unwrap();
    assert_eq!(shelves.slot(second).map(|s| s.count), Some(9));
    assert_eq!(shelves.slot(ore[0].id).map(|s| s.count), Some(10));
    assert_eq!(shelves.units_of(ResourceId::Ore), 39);
    // A sale of nine comes off the last stack.
    assert!(world.man_the_desk_for_probe(0));
    let events = world.step(&[Command::Sell {
        slot: 0,
        resource: ResourceId::Ore,
        units: 9,
    }]);
    assert!(
        events.contains(&WorldEvent::Traded {
            slot: 0,
            resource: ResourceId::Ore,
            units: -9
        }),
        "{events:?}"
    );
    let shelves = world.grid(Storage::Shelf).unwrap();
    let counts: Vec<u32> = ore
        .iter()
        .filter_map(|s| shelves.slot(s.id).map(|s| s.count))
        .collect();
    assert_eq!(counts, vec![10, 9, 10, 1]);
    assert_eq!(shelves.units_of(ResourceId::Ore), 30);
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

        let mut confirmed = basic();
        let target = nearby(&confirmed, 12_000.0);
        confirmed.man_the_helm_for_probe(0);
        confirmed.step(&[Command::Confirm { slot: 0, target }]);
        let mut idle = basic();
        idle.man_the_helm_for_probe(0);
        idle.step(&[]);
        assert_ne!(confirmed.checksum(), idle.checksum(), "a trip should show");
    }

    // --- the_checksum_notices_a_stance_change ---
    {
        let mut world = basic();
        let was = world.checksum();
        let station_id = world.residents.as_ref().unwrap().station;
        world.set_hostile(station_id, true);
        assert_ne!(world.checksum(), was, "an enemy should show");
        world.set_hostile(station_id, false);
        assert_eq!(world.checksum(), was, "and peace again is the same world");
        // And the stations the generator rolled hostile are on the list from
        // the start, home excepted.
        let rolled: Vec<u32> = world
            .stations
            .iter()
            .filter(|s| s.hostile && s.id != world.home)
            .map(|s| s.id)
            .collect();
        assert_eq!(world.hostile, rolled);
        assert_eq!(world.stance(world.home), bims::sight::Stance::Friendly);
    }

    // --- the_checksum_notices_a_worn_piece ---
    {
        use bims::health::Part;
        use shipdesign::fixture::playtest_ship;
        let mut world = simulation_world(playtest_ship(), data::SIMULATION_MONEY, 1);
        let mut twin = simulation_world(playtest_ship(), data::SIMULATION_MONEY, 1);
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

    // --- the_checksum_notices_research_and_a_key_taken ---
    {
        use shipdesign::fixture::playtest_ship;
        use shipdesign::research::Node;
        let mut world = simulation_world(playtest_ship(), data::SIMULATION_MONEY, 1);
        let twin = simulation_world(playtest_ship(), data::SIMULATION_MONEY, 1);
        assert_eq!(world_checksum(&world), world_checksum(&twin));
        assert!(world.research.enqueue(Node::Smelting));
        assert_ne!(world_checksum(&world), world_checksum(&twin), "queued");
        assert_eq!(world.research.next(), Some(Node::Smelting));
        assert_ne!(world_checksum(&world), world_checksum(&twin), "on the AI");
        world.research.cancel();
        assert_eq!(world_checksum(&world), world_checksum(&twin));
        let home = world.home;
        let at = world.stations.iter().position(|s| s.id == home).unwrap();
        world.station_keys[at] = 0;
        assert_ne!(world_checksum(&world), world_checksum(&twin));
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
/// purse: a trip to the nearest thing the crew can see can be planned on
/// the reactor, fed flat out, or the playtest is a ship that cannot leave
/// the dock.
#[test]
fn the_playtest_ship_can_fly_somewhere_from_the_simulation_spawn() {
    use shipdesign::fixture::playtest_ship;
    let world = simulation_world(playtest_ship(), data::SIMULATION_MONEY, 1);
    assert_eq!(world.money, data::SIMULATION_MONEY);
    assert_eq!(world.ship.crew_count, 1);
    let ShipState::Docked { station: dock } = world.ship.state else {
        panic!("not docked");
    };
    assert_eq!(world.ship.dynamics.forward_throttle, 1.0);
    assert_eq!(
        world.ship.dynamics.forward_power,
        shipdesign::parts::ENGINE_POWER
    );

    // The nearest node that is somewhere to *go*. At this spawn nothing but
    // the dock's own parent body is in sight, and that is where the ship
    // already is — the planner says `AlreadyThere` — so the trip is to the
    // next thing out, revealed through the probe seam the way a sensor sweep
    // would reveal it.
    let here = world.ship.position();
    let mut nodes: Vec<worldgen::Node> = world
        .system
        .nodes()
        .into_iter()
        .filter(|&n| n != worldgen::Node::Station(dock))
        .collect();
    nodes.sort_by(|a, b| {
        let da = world.system.absolute_position(*a).unwrap().distance(here);
        let db = world.system.absolute_position(*b).unwrap().distance(here);
        da.partial_cmp(&db).unwrap()
    });
    let mut world = world;
    let mut quote = None;
    for node in nodes {
        let at = world.system.absolute_position(node).unwrap();
        world.discover_for_probe(at, at);
        let target = match node {
            worldgen::Node::Body(id) => Target::Body(id),
            worldgen::Node::Station(id) => Target::Station(id),
        };
        match world.preview(target) {
            Err(PlanError::AlreadyThere) => continue,
            other => {
                quote = Some(other.expect("the trip should plan"));
                break;
            }
        }
    }
    let quote = quote.expect("the spawn system has somewhere to fly to");
    assert!(quote.minutes > 0.0);
    assert_eq!(quote.throttle, 1.0);
    assert_eq!(quote.power, shipdesign::parts::ENGINE_POWER);
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

/// A day's grime is put right under the shower: the playtest ship has one,
/// and a Bim whose washing need is emptied goes and stands under it, comes
/// out with the bar full and its own filth washed off. Emptied by hand
/// rather than waited for, because on the clock the first shower is at the
/// end of the waking day and a test that long is a test of everything.
/// A Bim that soils itself goes and showers: the accident empties the
/// washing need as it covers the Bim, so the shower is its next errand
/// rather than the end of the day's. Forced — poisoned, at the extreme
/// urge — rather than waited an hour for.
#[test]
fn a_bim_aboard_showers_for_a_day_s_grime_or_for_soiling_itself() {
    // --- a_bim_aboard_takes_a_shower_when_a_day_s_grime_has_caught_up_with_it ---
    {
        use bims::game::JOB_SHOWER;
        use shipdesign::fixture::playtest_ship;
        let mut world = simulation_world(playtest_ship(), data::SIMULATION_MONEY, 1);
        // `Need::ALL`'s index of the washing need, as `spend_for_probe` counts.
        const WASHING: u32 = 5;
        let room = &mut world.aboard.room;
        eprintln!("fog {:?} bims {}", room.fog(), room.crew_count());
        assert!(
            room.need_count() > WASHING,
            "the washing need is on the list"
        );
        room.spend_for_probe(0, WASHING, 1.0);
        assert_eq!(room.need_level(0, WASHING), 0.0);
        let mut showered = false;
        for _ in 0..(3 * 60 * 60) {
            world.step(&[]);
            if world.aboard.room.activity(0) == JOB_SHOWER {
                showered = true;
            }
            if showered && world.aboard.room.activity(0) != JOB_SHOWER {
                break;
            }
        }
        assert!(showered, "the Bim never went for its shower");
        let level = world.aboard.room.need_level(0, WASHING);
        assert!(level > 0.9, "out of the shower the bar reads {level}");
    }

    // --- a_bim_that_soils_itself_goes_for_a_shower ---
    {
        use bims::game::JOB_SHOWER;
        use shipdesign::fixture::playtest_ship;
        let mut world = simulation_world(playtest_ship(), data::SIMULATION_MONEY, 1);
        const RESTROOM: u32 = 2;
        const WASHING: u32 = 5;
        let room = &mut world.aboard.room;
        room.poison_for_probe(0);
        room.spend_for_probe(0, RESTROOM, 1.0);
        world.step(&[]);
        let room = &world.aboard.room;
        assert_eq!(
            room.need_level(0, WASHING),
            0.0,
            "covered, and wants a wash"
        );
        assert!(room.need_level(0, RESTROOM) > 0.0, "relieved");
        let mut showered = false;
        for _ in 0..(3 * 60 * 60) {
            world.step(&[]);
            if world.aboard.room.activity(0) == JOB_SHOWER {
                showered = true;
            }
            if showered && world.aboard.room.activity(0) != JOB_SHOWER {
                break;
            }
        }
        assert!(showered, "the Bim never went for its shower");
        let level = world.aboard.room.need_level(0, WASHING);
        assert!(level > 0.9, "out of the shower the bar reads {level}");
    }
}

/// Under way, the helm is a job on the work list: nobody posted there, and
/// the room posts whoever is free. Here James is stood at the helm for the
/// departure and then ordered elsewhere, which takes the post away — and
/// the job puts somebody back. Tied up again, the post the job set is
/// lifted, and only that one.
#[test]
fn under_way_the_helm_is_a_job_and_somebody_takes_it() {
    use bims::work::Job;
    let mut world = basic();
    let target = nearby(&world, 40_000.0);
    set_off(&mut world, 0, target);
    let room = &mut world.aboard.room;
    assert!(
        !room.work_on_offer_for_probe(1).contains(&Job::Helm.code()),
        "with James posted there the helm is not on offer"
    );
    // Off the helm: an order anywhere else ends the post. Two tiles aft.
    // Selected first, as the page has him from the first paint.
    room.select_group(0, 1);
    let at = room.bim_pos(0);
    let tile = shipdesign::parts::TILE as f32;
    assert!(
        room.order_move(0, at.x, at.y + 2.0 * tile) != 0,
        "the order was refused"
    );
    assert!(room.post_of(0).is_none());
    let mut posted = None;
    for _ in 0..(2 * 60 * 60) {
        world.step(&[]);
        if let Some(who) = (0..2).find(|&who| world.aboard.room.post_of(who).is_some()) {
            posted = Some(who);
            break;
        }
    }
    let who = posted.expect("nobody took the helm");
    for _ in 0..(60 * 60) {
        if world.at_the_helm(who as u32) {
            break;
        }
        world.step(&[]);
    }
    assert!(world.at_the_helm(who as u32), "posted but never got there");
    // And once the ship no longer wants anybody there, the post goes.
    world.aboard.room.set_helm(None);
    assert!(
        world.aboard.room.post_of(who).is_none(),
        "the job's post outlived the trip"
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

/// A trip to a station ends at its berth, alongside, with the doors mated —
/// the same arithmetic as the spawn, reached by flying rather than by being
/// put there.
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
    // Stand the ship near the other station rather than flying for days.
    world.undock_for_probe();
    let berth = world.berth_at(there).unwrap();
    world.put_for_probe(berth.position.add(dvec2(40_000.0, 10_000.0)));
    set_off(&mut world, 0, Target::Station(there));
    let ShipState::Travelling { plan, .. } = &world.ship.state else {
        panic!()
    };
    assert!(plan.docks);
    let arrival = plan.arrival;

    // The trip ends short of the berth, and the ship then comes alongside:
    // sliding and turning onto the berth over `DOCK_MINUTES`, never
    // jumping, and tied up at the end of it.
    let mut docking = false;
    let mut began = None;
    let mut last = None;
    for _ in 0..400_000 {
        let events = world.step(&[]);
        if events
            .iter()
            .any(|e| matches!(e, WorldEvent::Docking { station } if *station == there))
        {
            docking = true;
            assert!(world.ship.position().distance(arrival) < 1.0);
            began = Some(world.clock_minutes);
        }
        if let ShipState::Docking { .. } = world.ship.state {
            if let Some(was) = last {
                let moved = world.ship.position().distance(was);
                assert!(moved < 200.0, "jumped {moved} in one step");
            }
            last = Some(world.ship.position());
        }
        if !matches!(
            world.ship.state,
            ShipState::Travelling { .. } | ShipState::Docking { .. }
        ) {
            break;
        }
    }
    assert!(docking, "never came alongside");
    let took = world.clock_minutes - began.unwrap();
    assert!(
        took >= data::DOCK_MINUTES && took < data::DOCK_MINUTES + 2.0 * data::STEP_MINUTES,
        "took {took} to come alongside"
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
    // Home, not the first station somebody lives on: the spawn system has
    // an enemy's orbital too, and that one's room opens with a garrison.
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
/// ship. Under way again, the ship's room is the ship's alone and anybody
/// of the crew who was over there is back aboard.
#[test]
fn docked_the_ship_and_the_station_are_one_room_and_the_crew_can_cross() {
    let mut world = basic();
    let ShipState::Docked { station } = world.ship.state else {
        panic!("not docked");
    };
    let residents = world.station(station).unwrap().residents();
    assert!(residents > 0, "the spawn station has nobody to meet");
    // And whoever is for hire lives in the same room, fed the same.
    let ashore_count = residents + world.mercenaries_of(world.station(station).unwrap());
    assert!(world.aboard.is_joined(), "the rooms were not joined");
    assert_eq!(world.aboard.crew_count(), 2);
    assert_eq!(world.aboard.count(), 2);
    for who in 0..world.aboard.count() {
        assert!(world.aboard.on_deck(who), "{who} is not on the deck");
    }
    // The station's people are in the station's room, all of them, with
    // its galley for a galley and their own goals to keep.
    let ashore = world.residents.as_ref().expect("the station's room");
    assert_eq!(ashore.aboard.count(), ashore_count);
    assert_eq!(
        ashore.aboard.room.target(bims::manager::Stock::Veg),
        data::RESIDENT_VEG_EACH * ashore_count
    );

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

    // Leaving: whoever was over there walks back aboard — the ship waits
    // for them — and then it is one room again with the crew in it.
    let target = nearby(&world, 40_000.0);
    world.man_the_helm_for_probe(1);
    world.step(&[Command::Confirm { slot: 1, target }]);
    assert!(world.aboard.is_joined(), "left without them");
    let mut back = false;
    for _ in 0..LEAVING {
        world.step(&[]);
        if !world.aboard.is_joined() {
            break;
        }
        let p = world.aboard.room.bim_pos(0);
        back = back || on_ship(dvec2(p.x as f64, p.y as f64));
    }
    assert!(back, "the crew member was never seen back on the ship");
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

/// The bay aboard is six trays, one a tile along the run, worked from the
/// side its use spots are on: a tray a tile across for a bay lying
/// east–west and a tray a tile down for one standing north–south. The
/// side used to be read off the part's centre, and the first spot — at the
/// *end* of the run — then read as off the end rather than off the side,
/// which laid the trays across the bay in six strips.
#[test]
fn the_bay_aboard_is_a_tray_a_tile_worked_from_the_spots_side() {
    use bims::aboard::layout_of;
    use bims::hydro::Bay;
    use shipdesign::fixture::playtest_ship;
    use shipdesign::parts::TILE;
    let tile = TILE as f32;

    // The playtest ship's bay lies east–west and is worked from the north.
    let layout = layout_of(&playtest_ship());
    assert_eq!((layout.bay_side.x, layout.bay_side.y), (0.0, -1.0));
    let bay = Bay::at(layout.bay, layout.bay_side);
    for i in 0..6 {
        let stand = bay.station(i);
        let want = layout.bay.min.x + (i as f32 + 0.5) * tile;
        assert!(
            (stand.x - want).abs() < tile / 3.0 && stand.y < layout.bay.min.y,
            "tray {i} is worked from {stand:?}, not a tile along from the north"
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
    for i in 0..6 {
        let stand = bay.station(i);
        let want = layout.bay.min.y + (i as f32 + 0.5) * tile;
        assert!(
            (stand.y - want).abs() < tile / 3.0 && stand.x > layout.bay.max.x,
            "tray {i} is worked from {stand:?}, not a tile down from the east"
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
/// battery emptied under an overdraw: the lamps go dark and whole, the
/// bay hibernates, the benches stop, the cold store stops and the food in
/// it loses a share an hour — off the shelf and off the hold alike — while
/// the doors and life support run on. Said once each way. Then the
/// reactors cover the draw again: the lamps come back, the bay wakes, and
/// what is left in the cold store stays exactly as it is.
#[test]
fn a_brownout_darkens_the_ship_stops_the_bay_and_spoils_the_food_until_the_power_is_back() {
    use shipdesign::fixture::playtest_ship;
    let mut world = simulation_world(playtest_ship(), data::SIMULATION_MONEY, 1);
    world.know_everything_for_probe();
    let lamps = ship_lamps(&world);
    assert_eq!(lamps.len(), 7);
    assert!(lamps.iter().all(|(_, l)| l.powered && !l.is_dark()));
    assert!(world.aboard.room.hydro_powered(0));
    assert!(!world.aboard.room.hydro_hibernating(0));
    assert!(world.powered(PartKind::Smelter));
    assert!(world.powered(PartKind::ColdStore));
    assert_eq!(world.cold_store_out, 0);

    // The reactors held under the draw: the battery drains, the ship is
    // not browned out, and the cold store is not on the clock.
    let draw = world.power().draw;
    assert_eq!(draw, 327.0);
    world.throttle_reactors_for_probe(draw - 27.0);
    let events = world.step(&[]);
    assert!(!events.iter().any(|e| matches!(e, WorldEvent::Brownout)));
    assert!(!world.power().brownout());
    assert_eq!(world.cold_store_out, 0);

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
    assert!(!world.aboard.room.hydro_powered(0));
    assert!(world.aboard.room.hydro_hibernating(0));
    assert!(!world.powered(PartKind::HydroBay));
    assert!(!world.powered(PartKind::ColdStore));
    assert!(!world.powered(PartKind::Smelter));
    assert!(!world.powered(PartKind::Workbench));
    assert!(world.powered(PartKind::Door));
    assert!(world.powered(PartKind::LifeSupport));
    // Ore aboard and a target for metal, and still no order: the smelter
    // is dark.
    world.set_craft_target(ResourceId::Metal, 999);
    assert!(world.craft_orders().is_empty());

    // An hour without the cold store: a share of the food goes, off the
    // shelf and off the hold, and the log says how much. The crew may
    // have eaten off the shelf meanwhile, so the shelf is only asked to
    // have fallen; the hold is exact.
    let shelf_before = (
        world.aboard.room.store_veg(),
        world.aboard.room.store_tofu(),
    );
    let hold_veg = world.ship.design.carrying(ResourceId::Vegetable);
    let hold_tofu = world.ship.design.carrying(ResourceId::Tofu);
    assert!(hold_veg > 0 && hold_tofu > 0);
    let mut spoiled = None;
    for _ in 0..data::SPOIL_STEPS {
        for event in world.step(&[]) {
            if let WorldEvent::FoodSpoiled { units } = event {
                spoiled = Some(units);
            }
        }
    }
    let units = spoiled.expect("an hour unpowered spoils a share");
    assert!(units > 0);
    // The clock ran two steps before the hour's loop, so the hour was up
    // two steps before its end and the next has two on it.
    assert_eq!(world.cold_store_out, 2, "the clock starts the next hour");
    assert_eq!(
        world.ship.design.carrying(ResourceId::Vegetable),
        hold_veg - hold_veg.div_ceil(data::SPOIL_DIVISOR)
    );
    assert_eq!(
        world.ship.design.carrying(ResourceId::Tofu),
        hold_tofu - hold_tofu.div_ceil(data::SPOIL_DIVISOR)
    );
    assert!(world.aboard.room.store_veg() < shelf_before.0);
    assert!(world.aboard.room.store_tofu() < shelf_before.1);

    // The reactors back over the draw: said once, the lamps lit, the bay
    // awake, and the food that is left stays put through the next hour.
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
    assert!(world.aboard.room.hydro_powered(0));
    assert!(world.powered(PartKind::ColdStore));
    assert!(world.powered(PartKind::Smelter));
    let hold_veg = world.ship.design.carrying(ResourceId::Vegetable);
    let hold_tofu = world.ship.design.carrying(ResourceId::Tofu);
    for _ in 0..data::SPOIL_STEPS + 10 {
        let events = world.step(&[]);
        assert!(
            !events
                .iter()
                .any(|e| matches!(e, WorldEvent::FoodSpoiled { .. } | WorldEvent::Brownout)),
            "{events:?}"
        );
    }
    assert_eq!(world.cold_store_out, 0);
    assert_eq!(world.ship.design.carrying(ResourceId::Vegetable), hold_veg);
    assert_eq!(world.ship.design.carrying(ResourceId::Tofu), hold_tofu);
}

/// An overdraw the battery covers costs nothing: an hour and more of the
/// reactors under the draw with charge to spare is no brownout, no dark,
/// no sleeping bay and no spoilage — only a battery that much emptier.
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
        let hold_veg = world.ship.design.carrying(ResourceId::Vegetable);
        let hold_tofu = world.ship.design.carrying(ResourceId::Tofu);
        let steps = data::SPOIL_STEPS + data::SPOIL_STEPS / 2;
        for _ in 0..steps {
            let events = world.step(&[]);
            assert!(
                !events.iter().any(|e| matches!(
                    e,
                    WorldEvent::Brownout
                        | WorldEvent::PowerRestored
                        | WorldEvent::FoodSpoiled { .. }
                )),
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
        assert_eq!(world.cold_store_out, 0);
        assert!(
            ship_lamps(&world)
                .iter()
                .all(|(_, l)| l.powered && !l.is_dark())
        );
        assert!(world.aboard.room.hydro_powered(0));
        assert_eq!(world.ship.design.carrying(ResourceId::Vegetable), hold_veg);
        assert_eq!(world.ship.design.carrying(ResourceId::Tofu), hold_tofu);
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

/// Two worlds on one seed brown out together and lose the same food on
/// the same step: the spoil clock is an integer and the share is a
/// division, so the checksums agree throughout.
#[test]
fn two_runs_on_one_seed_spoil_the_same_amount() {
    use shipdesign::fixture::playtest_ship;
    let mut worlds = [
        simulation_world(playtest_ship(), data::SIMULATION_MONEY, 1),
        simulation_world(playtest_ship(), data::SIMULATION_MONEY, 1),
    ];
    for world in &mut worlds {
        world.throttle_reactors_for_probe(300.0);
        world.ship.charge = 0.0;
    }
    let mut spoiled = [0u32; 2];
    for _ in 0..data::SPOIL_STEPS + 5 {
        for (i, world) in worlds.iter_mut().enumerate() {
            for event in world.step(&[]) {
                if let WorldEvent::FoodSpoiled { units } = event {
                    spoiled[i] += units;
                }
            }
        }
        assert_eq!(world_checksum(&worlds[0]), world_checksum(&worlds[1]));
    }
    assert!(spoiled[0] > 0);
    assert_eq!(spoiled, [spoiled[0]; 2]);
    assert_eq!(
        worlds[0].ship.design.carrying(ResourceId::Vegetable),
        worlds[1].ship.design.carrying(ResourceId::Vegetable)
    );
    assert_eq!(worlds[0].cold_store_out, worlds[1].cold_store_out);
}

// --- making things -----------------------------------------------------------

/// The whole seam, on the playtest ship: a target for metal, ore aboard,
/// the smelter wired — the room is handed an order, a Bim walks to the
/// bench and stands at it for the recipe's length, and the world moves two
/// ore out of the hold and one metal in, with the mass moving with them.
/// Then the target is met and nothing more is made.
#[test]
fn a_target_for_metal_has_a_bim_smelt_ore_at_the_bench() {
    use bims::game::JOB_CRAFT;
    use shipdesign::fixture::playtest_ship;
    let mut world = simulation_world(playtest_ship(), data::SIMULATION_MONEY, 1);
    // The dressings every Bim carries are out of the way (feature 87).
    without_dressings(&mut world);
    world.know_everything_for_probe();
    let ore = world.ship.design.carrying(ResourceId::Ore);
    let metal = world.ship.design.carrying(ResourceId::Metal);
    assert!(ore >= 2);
    assert_eq!(world.craft_target(ResourceId::Metal), 0);
    assert!(world.craft_orders().is_empty(), "nothing is asked for yet");
    // The smelter, the workbench, the drug lab and the armoury.
    assert_eq!(world.aboard.room.benches().len(), 4);
    assert!(world.powered(PartKind::Smelter));

    // One more metal than there is: one smelt's worth.
    world.set_craft_target(ResourceId::Metal, metal + 1);
    let orders = world.craft_orders();
    assert_eq!(orders.len(), 1, "{}", orders.len());
    assert_eq!(orders[0].recipe, 0);
    assert_eq!(orders[0].minutes, 30.0);
    let mass_before = world.ship.dynamics.mass.get();

    // The recipe is half an hour at the bench plus the walk there; give it
    // the morning. The agenda shows the craft errand on the way.
    let mut crafted = false;
    let mut seen_at_bench = false;
    for _ in 0..(4 * 60 * 60) {
        let events = world.step(&[]);
        if world.aboard.room.agenda_len(0) > 0 && world.aboard.room.agenda_job(0, 0) == JOB_CRAFT {
            seen_at_bench = true;
        }
        if events
            .iter()
            .any(|e| matches!(e, WorldEvent::Crafted { recipe: 0 }))
        {
            crafted = true;
            break;
        }
        assert!(
            !events
                .iter()
                .any(|e| matches!(e, WorldEvent::CraftLost { .. })),
            "the ore was there the whole time"
        );
    }
    assert!(crafted, "no metal was made in four hours");
    assert!(seen_at_bench, "the errand never showed on the agenda");
    assert_eq!(world.ship.design.carrying(ResourceId::Ore), ore - 2);
    assert_eq!(world.ship.design.carrying(ResourceId::Metal), metal + 1);
    // Two ore at ten is twenty; one metal is eight. The slag went.
    let mass_after = world.ship.dynamics.mass.get();
    assert!(
        close(mass_before - mass_after, 12.0),
        "mass went from {mass_before} to {mass_after}"
    );

    // Met: no order, and an hour later still one metal more.
    assert!(world.craft_orders().is_empty());
    for _ in 0..(60 * 60) {
        world.step(&[]);
    }
    assert_eq!(world.ship.design.carrying(ResourceId::Metal), metal + 1);
}

/// A recipe wants its inputs and its station's power. With no ore aboard
/// there is no order for metal however high the target; with the smelter
/// unpowered, none either; and an order for components at the workbench
/// is a different bench and is still made.
#[test]
fn an_order_wants_the_inputs_aboard_and_the_bench_powered() {
    use shipdesign::fixture::playtest_ship;
    let budget = Budget::new(10_000_000);
    let mut world = simulation_world(playtest_ship(), data::SIMULATION_MONEY, 1);
    world.know_everything_for_probe();
    let ore = world.ship.design.carrying(ResourceId::Ore);
    world.ship.design = apply(
        &world.ship.design,
        &budget,
        Edit::Sell {
            resource: ResourceId::Ore,
            units: ore,
        },
    )
    .unwrap();
    world.on_ship_changed();
    world.set_craft_target(ResourceId::Metal, 999);
    assert!(world.craft_orders().is_empty(), "no ore, no smelting");

    // Components out of metal want only the workbench, which has power.
    let components = world.ship.design.carrying(ResourceId::Components);
    world.set_craft_target(ResourceId::Components, components + 4);
    let orders = world.craft_orders();
    assert_eq!(orders.len(), 1);
    assert_eq!(orders[0].recipe, 1);

    // Take the reactors off — the playtest ship carries two now, and one
    // left would run the workbench: nothing is powered, nothing is on
    // offer.
    let reactors: Vec<u32> = world
        .ship
        .design
        .parts
        .iter()
        .filter(|p| p.kind == PartKind::Reactor)
        .map(|p| p.id)
        .collect();
    assert!(!reactors.is_empty());
    for reactor in reactors {
        world.ship.design = apply(
            &world.ship.design,
            &budget,
            Edit::Remove { part_id: reactor },
        )
        .unwrap();
    }
    world.on_ship_changed();
    assert!(!world.powered(PartKind::Workbench));
    assert!(world.craft_orders().is_empty(), "no power, no work");

    // The target is clamped to what the shelves could hold of it alone:
    // a stack of twenty a cell.
    world.set_craft_target(ResourceId::Components, 10_000);
    assert_eq!(
        world.craft_target(ResourceId::Components),
        world.ship.design.capacity(Storage::Shelf) * 20
    );
}

/// Standing at a bench is the player's own Bim's work and nobody else's
/// (feature 89). With a crew of two and one player, an order for metal
/// is on offer to slot 0 and never to the crewmate behind it, whatever
/// the Craft row says; and the smelt that gets done is done by slot 0.
/// The rest of the work list is untouched — the crewmate still has the
/// cooking, the bay and the deck on offer.
#[test]
fn a_bot_never_stands_at_a_bench_and_a_player_s_bim_does() {
    use bims::game::JOB_CRAFT;
    use bims::work::Job;
    use shipdesign::fixture::playtest_ship;
    let mut world = crewed_world(playtest_ship(), data::SIMULATION_MONEY, 1, 2);
    without_dressings(&mut world);
    world.know_everything_for_probe();
    let metal = world.ship.design.carrying(ResourceId::Metal);
    world.set_craft_target(ResourceId::Metal, metal + 1);
    assert_eq!(world.craft_orders().len(), 1, "the smelter is asked for");
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

    // And over a morning the smelt is slot 0's from end to end.
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
    assert!(crafted, "no metal was made in four hours");
    assert_eq!(world.ship.design.carrying(ResourceId::Metal), metal + 1);
}

// --- a walk outside ----------------------------------------------------------

/// A belt of the spawn system, or of a system that has one: what a walk
/// outside is measured at. The simulation's spawn system is whatever the
/// generator made it, so this looks rather than assumes.
fn a_belt(world: &World) -> Option<worldgen::Body> {
    world
        .system
        .bodies
        .iter()
        .find(|b| b.kind == worldgen::BodyKind::AsteroidBelt)
        .cloned()
}

/// The ship at a belt of the spawn system, holding, with the site laid out
/// — or `None` when the spawn system has no belt, which a test says so
/// about and gives up on.
fn at_a_belt(world: &mut World) -> Option<worldgen::Body> {
    let belt = a_belt(world)?;
    world.undock_for_probe();
    world.put_for_probe(belt.position);
    world.settle_frame_for_probe();
    assert_eq!(
        world.ship.frame,
        Frame::Local(worldgen::Node::Body(belt.id))
    );
    world.step(&[]);
    Some(belt)
}

/// The asteroids of a site, each as its tiles: four-way connected pieces.
fn asteroids_of(site: &MiningSite) -> Vec<Vec<RockTile>> {
    let mut left: Vec<RockTile> = site.tiles.clone();
    let mut out = Vec::new();
    while let Some(seed) = left.pop() {
        let mut piece = vec![seed];
        let mut i = 0;
        while i < piece.len() {
            let (x, y) = (piece[i].x, piece[i].y);
            let mut j = 0;
            while j < left.len() {
                let t = left[j];
                if (t.x - x).abs() + (t.y - y).abs() == 1 {
                    piece.push(left.remove(j));
                } else {
                    j += 1;
                }
            }
            i += 1;
        }
        out.push(piece);
    }
    out
}

/// A trip to a belt ends `ARRIVAL_RADIUS_BODY` short of it, from whichever
/// side the ship came, and the site is laid out wherever that was: at rest
/// there the belt is the nearest thing in the system, so the view settles
/// on the belt and not on something beside it. It used not to be — a
/// mining outpost or a derelict sat in orbit of a belt, nearer the ship
/// than the belt was on the near side, the view settled on *that*, and no
/// site was laid out — which is why nothing stands at a belt any more
/// (`worldgen::data::parent_suits`). Every belt of the spawn system, from
/// twelve directions.
/// Holding at a belt lays a field of asteroids out about the ship: clear of
/// the hull, skinned in stone with the ore three tiles down, the same
/// field every time, and galvum in about one asteroid in ten.
#[test]
fn a_mining_site_is_laid_out_about_the_ship_at_a_belt_from_any_side() {
    // --- coming_to_rest_at_a_belt_from_any_side_lays_the_site_out ---
    {
        let mut world = basic();
        let belts: Vec<worldgen::Body> = world
            .system
            .bodies
            .iter()
            .filter(|b| b.kind == worldgen::BodyKind::AsteroidBelt)
            .cloned()
            .collect();
        assert!(!belts.is_empty(), "the spawn system has a belt to mine");
        world.undock_for_probe();
        for belt in &belts {
            for i in 0..12 {
                let angle = std::f64::consts::TAU * i as f64 / 12.0;
                let short = DVec2::polar(angle, flight::data::ARRIVAL_RADIUS_BODY);
                world.put_for_probe(belt.position.add(short));
                world.settle_frame_for_probe();
                world.step(&[]);
                assert_eq!(
                    world.ship.frame,
                    Frame::Local(worldgen::Node::Body(belt.id)),
                    "belt {} from {i}/12 round: the view is about {:?}",
                    belt.id,
                    world.ship.frame
                );
                assert_eq!(
                    world.site_here().map(|s| s.belt),
                    Some(belt.id),
                    "belt {} from {i}/12 round: no site",
                    belt.id
                );
                // Away again, so the next approach enters the frame afresh.
                world.put_for_probe(belt.position.add(DVec2::polar(
                    angle,
                    data::LOCAL_RADIUS_BODY * data::LOCAL_HYSTERESIS * 2.0,
                )));
                world.settle_frame_for_probe();
                assert_eq!(world.ship.frame, Frame::Space);
            }
        }
    }

    // --- a_mining_site_is_laid_out_about_the_ship_at_a_belt ---
    {
        use shipdesign::fixture::playtest_ship;
        let mut world = simulation_world(playtest_ship(), data::SIMULATION_MONEY, 1);
        assert!(world.site_here().is_none(), "docked is not at a site");
        let Some(belt) = at_a_belt(&mut world) else {
            eprintln!("the spawn system has no belt; nothing to lay out");
            return;
        };
        let site = world.site_here().expect("a site at a belt").clone();
        assert_eq!(site.belt, belt.id);
        assert!(!site.tiles.is_empty());
        assert!(site.marked.is_empty());

        // Clear of the hull, every tile of it, with room to walk between.
        let hull: Vec<(i32, i32)> = world
            .ship
            .design
            .parts
            .iter()
            .flat_map(|p| p.tiles())
            .map(|(x, y)| (x as i32, y as i32))
            .collect();
        for tile in &site.tiles {
            for &(hx, hy) in &hull {
                assert!(
                    (tile.x - hx).abs() > 2 || (tile.y - hy).abs() > 2,
                    "rock at ({}, {}) against the hull at ({hx}, {hy})",
                    tile.x,
                    tile.y
                );
            }
        }
        // Skin and core: an ore tile has rock two tiles deep every way.
        let stands = |x: i32, y: i32| site.at(x, y).is_some();
        let mut cores = 0;
        for tile in &site.tiles {
            if tile.kind == Rock::Stone {
                continue;
            }
            cores += 1;
            for dx in -2..=2i32 {
                for dy in -2..=2i32 {
                    if dx.abs() + dy.abs() <= 2 {
                        assert!(
                            stands(tile.x + dx, tile.y + dy),
                            "ore at ({}, {}) with nothing at ({dx}, {dy}) off it",
                            tile.x,
                            tile.y
                        );
                    }
                }
            }
        }
        assert!(cores > 0, "no ore anywhere in the site");
        let pieces = asteroids_of(&site);
        assert!(
            pieces.len() >= mining::ASTEROIDS.0 as usize / 2,
            "{} asteroids",
            pieces.len()
        );

        // The same field every time.
        let again = MiningSite::generate(world.galaxy_seed, world.star_id, belt.id, (0, 0, 19, 19));
        let twice = MiningSite::generate(world.galaxy_seed, world.star_id, belt.id, (0, 0, 19, 19));
        assert_eq!(again, twice);

        // About one asteroid in ten carries galvum, over many belts.
        let (mut rich, mut all) = (0, 0);
        for belt in 0..200u32 {
            let site = MiningSite::generate(world.galaxy_seed, world.star_id, belt, (0, 0, 19, 19));
            for piece in asteroids_of(&site) {
                all += 1;
                if piece.iter().any(|t| t.kind == Rock::Galvum) {
                    rich += 1;
                }
            }
        }
        let share = rich as f64 / all as f64;
        assert!(
            (0.06..=0.14).contains(&share),
            "{rich} of {all} asteroids carry galvum"
        );
    }
}

/// The whole seam: holding at a belt with rocks marked and a suit in the
/// locker, the room is told a walk is on, a Bim takes the suit, goes out
/// through the airlock — off the deck, in the suit, dosed while it is out —
/// walks to the marked rocks, digs through the stone to the ore, and comes
/// back in with the rock and the ore on the shelf and the tiles gone from
/// the site. Nothing marked, no walk; no suit, no walk.
#[test]
fn marked_rocks_are_mined_on_foot_and_what_they_yield_lands_on_the_shelf() {
    use bims::game::JOB_EVA;
    use shipdesign::fixture::playtest_ship;
    let mut world = simulation_world(playtest_ship(), data::SIMULATION_MONEY, 1);
    assert_eq!(world.ship.design.carrying(ResourceId::Suit), 1);
    assert!(!world.aboard.room.is_outside(0));
    assert!(world.eva_offer().is_none(), "docked is not at a site");
    let Some(_) = at_a_belt(&mut world) else {
        eprintln!("the spawn system has no belt; nothing to walk out to");
        return;
    };
    let offer = world.eva_offer().expect("a walk is on at a site");
    assert_eq!(offer.allowed, vec![true]);
    assert!(offer.targets.is_empty(), "nothing is marked yet");
    assert!(!offer.rocks.is_empty());
    assert_eq!(offer.tile_minutes, data::MINE_TILE_MINUTES as f32);

    // Nothing marked: the room is told there is nothing to go out for.
    for _ in 0..600 {
        world.step(&[]);
        assert!(!world.aboard.room.is_outside(0), "out with nothing marked");
    }

    // Sell the suit and there is no walk; buy it back — not docked, so by
    // hand — and there is.
    world.ship.design.cargo[ResourceId::Suit as usize] = 0;
    assert!(world.eva_offer().is_none(), "no suit, no walk");
    world.ship.design.cargo[ResourceId::Suit as usize] = 1;

    // A dig: from an ore tile of the nearest asteroid straight out to the
    // skin, every tile on the line marked. The Bim takes them from the
    // outside in, since the one behind is only reachable once the one in
    // front is gone.
    let site = world.site_here().unwrap().clone();
    let port = shipdesign::dock::port(&world.ship.design).unwrap();
    let door = (port.centre.0 as i32, port.centre.1 as i32);
    let nearest = site
        .tiles
        .iter()
        .min_by_key(|t| (t.x - door.0).abs() + (t.y - door.1).abs())
        .unwrap();
    let piece = asteroids_of(&site)
        .into_iter()
        .find(|p| p.contains(nearest))
        .unwrap();
    let core = piece
        .iter()
        .find(|t| t.kind != Rock::Stone)
        .copied()
        .expect("an asteroid with a core");
    let (mx, my) = (
        piece.iter().map(|t| t.x).sum::<i32>() as f64 / piece.len() as f64,
        piece.iter().map(|t| t.y).sum::<i32>() as f64 / piece.len() as f64,
    );
    // Out along whichever axis the core is further from the middle on, or
    // +x for one dead centre.
    let (dx, dy) = if (core.x as f64 - mx).abs() >= (core.y as f64 - my).abs() {
        (if core.x as f64 >= mx { 1 } else { -1 }, 0)
    } else {
        (0, if core.y as f64 >= my { 1 } else { -1 })
    };
    let mut marks = Vec::new();
    let (mut x, mut y) = (core.x, core.y);
    while site.at(x, y).is_some() {
        marks.push((x, y));
        x += dx;
        y += dy;
    }
    assert!(marks.len() >= 3, "a dig of {} tiles", marks.len());
    let commands: Vec<Command> = marks
        .iter()
        .map(|&(x, y)| Command::MarkRock { slot: 0, x, y })
        .collect();
    world.step(&commands);
    assert_eq!(world.site_here().unwrap().marked, marks);
    let offer = world.eva_offer().unwrap();
    assert_eq!(offer.targets.len(), marks.len());

    let rock = world.ship.design.carrying(ResourceId::Rock);
    let ore = world.ship.design.carrying(ResourceId::Ore);
    let tiles = world.site_here().unwrap().tiles.len();
    let expect_rock: u32 = marks
        .iter()
        .filter(|&&(x, y)| site.at(x, y) == Some(Rock::Stone))
        .count() as u32
        * mining::yield_of(Rock::Stone).1;
    let expect_ore: u32 = marks
        .iter()
        .filter(|&&(x, y)| site.at(x, y) == Some(Rock::Iron))
        .count() as u32
        * mining::yield_of(Rock::Iron).1;

    let mut went_out = false;
    let mut walked = false;
    let mut out_at: Option<DVec2> = None;
    let mut peak_dose = 0.0f64;
    let mut mined: Vec<(u32, u32, u32)> = Vec::new();
    let mut on_agenda = false;
    // A dozen minutes a rock and the walks between, plus the errand either
    // side and whatever else the day asks; give it the day.
    for _ in 0..(24 * 60 * 60) {
        let events = world.step(&[]);
        if world.aboard.room.agenda_len(0) > 0 && world.aboard.room.agenda_job(0, 0) == JOB_EVA {
            on_agenda = true;
        }
        if world.aboard.room.is_outside(0) {
            went_out = true;
            assert!(!world.aboard.on_deck(0), "outside is off the deck");
            // Somewhere well away from where it stepped out: a walk, not a
            // body held at the door.
            let at = world.aboard.position(0);
            let first = *out_at.get_or_insert(at);
            if at.sub(first).length() > 3.0 * shipdesign::TILE as f64 {
                walked = true;
            }
        }
        peak_dose = peak_dose.max(world.health[0].dose);
        for e in &events {
            if let WorldEvent::Mined { rock, ore, galvum } = e {
                mined.push((*rock, *ore, *galvum));
            }
        }
        if world.site_here().unwrap().marked.is_empty() && !world.aboard.room.is_outside(0) {
            break;
        }
    }
    assert!(on_agenda, "the walk never showed on the agenda");
    assert!(went_out, "the Bim never went outside");
    assert!(walked, "the Bim never walked anywhere out there");
    assert!(!mined.is_empty(), "no walk ever came back");
    assert!(
        world.site_here().unwrap().marked.is_empty(),
        "marks left: {:?}",
        world.site_here().unwrap().marked
    );
    assert_eq!(world.site_here().unwrap().tiles.len(), tiles - marks.len());
    assert_eq!(
        world.ship.design.carrying(ResourceId::Rock),
        rock + expect_rock
    );
    assert_eq!(
        world.ship.design.carrying(ResourceId::Ore),
        ore + expect_ore
    );
    let said: (u32, u32, u32) = mined
        .iter()
        .fold((0, 0, 0), |a, m| (a.0 + m.0, a.1 + m.1, a.2 + m.2));
    assert_eq!(said.0, expect_rock, "the log's rock");
    assert_eq!(said.1, expect_ore, "the log's ore");
    // Dosed out there, and never near the critical line.
    assert!(
        peak_dose > 0.0 && peak_dose < health::CRITICAL,
        "peak dose {peak_dose}"
    );
    // Back in: on the deck, in the coverall, and the dose coming off.
    assert!(!world.aboard.room.is_outside(0));
    let dose_in = world.health[0].dose;
    for _ in 0..(60 * 60) {
        world.step(&[]);
        assert!(!world.aboard.room.is_outside(0));
    }
    assert!(
        world.health[0].dose < dose_in,
        "the dose should come off inside"
    );
    assert!(!world.health[0].dead);
}

/// A mark is a command: it toggles, it refuses a tile that is not a rock,
/// `ClearMarks` takes them all off, and leaving the site takes them off
/// too and brings whoever is out there in. Two crew, since the one out
/// on the rocks is not aboard and cannot cast off: the other takes the
/// helm.
#[test]
fn a_mark_is_a_command_and_the_marks_come_off_with_the_ship() {
    use shipdesign::fixture::playtest_ship;
    let mut world = simulation_world(playtest_ship(), data::SIMULATION_MONEY, 2);
    let Some(_) = at_a_belt(&mut world) else {
        return;
    };
    let site = world.site_here().unwrap().clone();
    let rock = site.tiles[0];
    let (x, y) = (rock.x, rock.y);
    world.step(&[Command::MarkRock { slot: 0, x, y }]);
    assert!(world.site_here().unwrap().is_marked(x, y));
    world.step(&[Command::MarkRock { slot: 0, x, y }]);
    assert!(!world.site_here().unwrap().is_marked(x, y));
    // Off a rock: nothing.
    world.step(&[Command::MarkRock {
        slot: 0,
        x: 10_000,
        y: 10_000,
    }]);
    assert!(world.site_here().unwrap().marked.is_empty());
    world.step(&[
        Command::MarkRock { slot: 0, x, y },
        Command::MarkRock {
            slot: 0,
            x: site.tiles[1].x,
            y: site.tiles[1].y,
        },
    ]);
    assert_eq!(world.site_here().unwrap().marked.len(), 2);
    let before = world_checksum(&world);
    world.step(&[Command::ClearMarks { slot: 0 }]);
    assert!(world.site_here().unwrap().marked.is_empty());
    assert_ne!(
        world_checksum(&world),
        before,
        "the marks are in the checksum"
    );

    // Marked and out there; then the ship leaves, and the marks and the
    // walk go with it.
    let door = shipdesign::dock::port(&world.ship.design).unwrap().centre;
    let nearest = site
        .tiles
        .iter()
        .min_by_key(|t| (t.x - door.0 as i32).abs() + (t.y - door.1 as i32).abs())
        .unwrap();
    world.step(&[Command::MarkRock {
        slot: 0,
        x: nearest.x,
        y: nearest.y,
    }]);
    let mut out = None;
    for _ in 0..(3 * 60 * 60) {
        world.step(&[]);
        if let Some(who) = (0..2).find(|&who| world.aboard.room.is_outside(who)) {
            out = Some(who);
            break;
        }
    }
    let out = out.expect("nobody went out");
    let inside = 1 - out as u32;
    let target = nearby(&world, 200_000.0);
    let events = set_off(&mut world, inside, target);
    assert!(
        events
            .iter()
            .any(|e| matches!(e, WorldEvent::Departed { .. }))
    );
    assert!(!world.aboard.room.is_outside(out), "left outside");
    let belt_site = world.sites.iter().find(|s| s.belt == site.belt).unwrap();
    assert!(belt_site.marked.is_empty(), "marks survived the departure");
    assert!(world.site_here().is_none());
}

/// A Bim past the dose limit is not sent out; below it, it is.
#[test]
fn a_dosed_bim_is_kept_in() {
    use shipdesign::fixture::playtest_ship;
    let mut world = simulation_world(playtest_ship(), data::SIMULATION_MONEY, 1);
    let Some(_) = at_a_belt(&mut world) else {
        return;
    };
    world.health[0].dose = data::EVA_DOSE_LIMIT + 1.0;
    assert_eq!(world.eva_offer().unwrap().allowed, vec![false]);
    world.health[0].dose = data::EVA_DOSE_LIMIT - 1.0;
    assert_eq!(world.eva_offer().unwrap().allowed, vec![true]);
}

/// The chain the whole thing was asked for: ore to metal, metal to
/// components, components and galvum to an emitter, and the emitter into a
/// laser handgun at the armoury. Set the targets and the benches do it in
/// order, because a handgun is not on offer until there is an emitter and
/// an emitter is not until there are components — each recipe waits on the
/// one before it without anybody sequencing them.
#[test]
fn a_target_for_a_handgun_runs_the_whole_chain_from_the_hold() {
    use shipdesign::fixture::playtest_ship;
    let budget = Budget::new(10_000_000);
    let mut world = simulation_world(playtest_ship(), data::SIMULATION_MONEY, 1);
    // The dressings every Bim carries are out of the way (feature 87).
    without_dressings(&mut world);
    world.know_everything_for_probe();
    // An armoury amidships on a branch of its own off the spine.
    let mut design = world.ship.design.clone();
    for (kind, origin, rotation) in [
        (PartKind::PowerConduit, (9, 11), Rotation::R0),
        (PartKind::PowerConduit, (10, 11), Rotation::R0),
        (PartKind::PowerConduit, (11, 11), Rotation::R0),
        (PartKind::Armoury, (11, 11), Rotation::R0),
    ] {
        design = apply(
            &design,
            &budget,
            Edit::Place {
                kind,
                origin,
                rotation,
            },
        )
        .unwrap_or_else(|e| panic!("{kind:?} at {origin:?}: {e:?}"));
    }
    // One galvum, by hand: the spawn is not an outpost.
    design.cargo[ResourceId::Galvum as usize] = 1;
    world.ship.design = design;
    world.on_ship_changed();
    assert!(world.powered(PartKind::Armoury));
    assert_eq!(
        world.aboard.room.benches().len(),
        4,
        "the room was laid out before"
    );
    // The room is laid out again when the rooms come apart, so the second
    // armoury is a bench once the ship lets go of the dock.
    world.undock_for_probe();
    assert_eq!(world.aboard.room.benches().len(), 5);

    world.set_craft_target(ResourceId::Emitter, 1);
    world.set_craft_target(ResourceId::Handgun, 1);
    // A handgun wants an emitter, so only the emitter is on offer.
    let orders = world.craft_orders();
    assert_eq!(orders.len(), 1, "{orders:?}");
    assert_eq!(orders[0].recipe, 2);

    let components = world.ship.design.carrying(ResourceId::Components);
    let metal = world.ship.design.carrying(ResourceId::Metal);
    let mut made = Vec::new();
    for _ in 0..(8 * 60 * 60) {
        let events = world.step(&[]);
        for e in &events {
            if let WorldEvent::Crafted { recipe } = e {
                made.push(*recipe);
            }
        }
        if world.ship.design.carrying(ResourceId::Handgun) >= 1 {
            break;
        }
    }
    assert_eq!(made, vec![2, 3], "the emitter and then the handgun");
    assert_eq!(world.ship.design.carrying(ResourceId::Handgun), 1);
    assert_eq!(
        world.ship.design.carrying(ResourceId::Emitter),
        0,
        "used up"
    );
    assert_eq!(world.ship.design.carrying(ResourceId::Galvum), 0);
    assert_eq!(
        world.ship.design.carrying(ResourceId::Components),
        components - 4
    );
    assert_eq!(world.ship.design.carrying(ResourceId::Metal), metal - 1);
    // A handgun goes in the locker class, beside the suit, the bandages,
    // the medkits, the three pieces of armour and the four other weapons
    // the playtest ship carries.
    let bandages = world.ship.design.carrying(ResourceId::Bandage);
    let medkits = world.ship.design.carrying(ResourceId::Medkit);
    assert_eq!(world.ship.design.carrying(ResourceId::Suit), 1);
    // In cells, since the lockers are a grid: the suit's nine, the
    // helm's eight, the kevlar's sixteen and the leg guards' six, the
    // shotgun's ten, the rifle's seven, the sniper's ten and the
    // schword's five, the handgun's two — seventy-three — four a box of
    // dressings, five to a box (feature 87), and four a medkit. And
    // every one of them lies in a slot.
    assert_eq!(
        world.ship.design.stored(Storage::Locker),
        73 + 4 * bandages.div_ceil(5) + 4 * medkits
    );
    assert_eq!(
        world.grid(Storage::Locker).unwrap().covered(),
        world.ship.design.stored(Storage::Locker)
    );
}

/// Docked, the station's people live on the station: over a day in their
/// own room they cook and eat at their own galley (a larder stocked to
/// their goals means stew warmed up rather than a pot from scratch), go to their own heads,
/// tend their own bay — and are never on the ship's deck, which has only
/// the crew on it. Their goals are their own too: the crew's targets are
/// untouched by theirs.
#[test]
fn docked_the_station_s_people_keep_to_the_station_and_their_own_agenda() {
    use bims::game::{
        JOB_BOWL, JOB_HEADS, JOB_LEFTOVERS, JOB_MEAL, JOB_REHEAT, JOB_STEW, JOB_TEND,
    };
    use bims::manager::Stock;
    let mut world = basic();
    assert!(world.aboard.is_joined());
    let ashore = world.residents.as_ref().expect("the station's room");
    let residents = ashore.aboard.count();
    assert!(residents > 0, "the spawn station has nobody living on it");
    assert_eq!(
        ashore.aboard.room.target(Stock::Stew),
        data::RESIDENT_STEW_EACH * residents
    );
    assert_eq!(
        world.aboard.room.target(Stock::Stew),
        0,
        "the crew's own target moved"
    );

    let mut cooked = false;
    let mut heads = false;
    let mut tended = false;
    let mut on_deck = false;
    for _ in 0..(24 * 60 * 60) {
        world.step(&[]);
        let ashore = world.residents.as_ref().unwrap();
        for who in 0..ashore.aboard.count() {
            match ashore.aboard.room.activity(who as usize) {
                JOB_MEAL | JOB_BOWL | JOB_STEW | JOB_REHEAT | JOB_LEFTOVERS => cooked = true,
                JOB_HEADS => heads = true,
                JOB_TEND => tended = true,
                _ => {}
            }
        }
        // The ship's room holds the crew and nobody else.
        assert_eq!(world.aboard.count(), world.aboard.crew_count());
        on_deck |= (0..world.aboard.count()).any(|who| !world.aboard.on_deck(who));
    }
    assert!(cooked, "nobody ashore cooked or ate in a day");
    assert!(heads, "nobody ashore went to the heads in a day");
    assert!(tended, "nobody ashore tended the bay in a day");
    assert!(!on_deck, "somebody of the crew was off the deck");
}

// --- building ------------------------------------------------------------------

/// A site laid out on the deck is carried to and built: the crew fetch
/// the wall's metal off a shelf a load at a time, put it down at the site,
/// then stand beside it and put it together, and the wall is on the ship
/// with its recipe out of the hold. The ship weighs the same throughout —
/// the materials were only ever spoken for, never in transit — and the
/// room goes on under the crew with the wall in it: nobody's errand is
/// lost, and the new wall is a solid the deck's grid goes round.
#[test]
fn a_site_on_the_deck_is_hauled_to_and_built_by_the_crew() {
    use bims::game::{JOB_BUILD, JOB_HAUL};
    use shipdesign::fixture::playtest_ship;
    use shipdesign::parts::Layer;
    let mut world = simulation_world(playtest_ship(), data::SIMULATION_MONEY, 1);
    // The dressings every Bim carries are out of the way (feature 87).
    without_dressings(&mut world);
    let metal = world.ship.design.carrying(ResourceId::Metal);
    let parts = world.ship.design.parts.len();
    let mass = world.ship.dynamics.mass.get();
    let at = (9, 12);
    assert!(
        world
            .ship
            .design
            .grid()
            .has_floor((at.0 as i32, at.1 as i32)),
        "the site is meant to go on deck"
    );
    assert_eq!(
        world.can_place_site(PartKind::Wall, at, Rotation::R0),
        Ok(())
    );
    assert!(world.build_orders().is_empty(), "nothing to build yet");

    let events = world.step(&[Command::PlaceSite {
        slot: 0,
        kind: PartKind::Wall,
        origin: at,
        rotation: Rotation::R0,
    }]);
    assert!(
        events.iter().any(|e| matches!(
            e,
            WorldEvent::SitePlaced {
                site: 1,
                kind: PartKind::Wall
            }
        )),
        "{events:?}"
    );
    assert_eq!(world.builds.len(), 1);
    assert!(!world.builds[0].begun(), "nothing has been carried yet");
    // The first order is the wall's metal, a load of it.
    let orders = world.build_orders();
    assert_eq!(orders.len(), 1);
    assert_eq!(orders[0].site, 1);
    assert_eq!(orders[0].haul, Some((ResourceId::Metal as u32, 2)));
    assert_eq!(orders[0].tiles.len(), 1);

    let mut hauled = false;
    let mut built_seen = false;
    let mut held_the_ship = false;
    let mut sale_refused = false;
    let mut built = false;
    for _ in 0..(4 * 60 * 60) {
        // Once the load is claimed, the metal cannot be sold from under
        // the site — docked, where a sale would otherwise go.
        let sell = if world.builds.first().is_some_and(|s| s.begun()) && !sale_refused {
            vec![Command::Sell {
                slot: 0,
                resource: ResourceId::Metal,
                units: metal,
            }]
        } else {
            Vec::new()
        };
        let events = world.step(&sell);
        if !sell.is_empty() {
            assert!(
                events.iter().any(|e| matches!(
                    e,
                    WorldEvent::Refused {
                        why: Refusal::NotAboard,
                        ..
                    }
                )),
                "{events:?}"
            );
            assert_eq!(world.ship.design.carrying(ResourceId::Metal), metal);
            sale_refused = true;
        }
        let room = &world.aboard.room;
        if room.agenda_len(0) > 0 {
            match room.agenda_job(0, 0) {
                JOB_HAUL => hauled = true,
                JOB_BUILD => built_seen = true,
                _ => {}
            }
        }
        held_the_ship |= world.under_construction();
        assert!(
            close(world.ship.dynamics.mass.get(), mass),
            "the mass moved while the wall was being built"
        );
        if events.iter().any(|e| {
            matches!(
                e,
                WorldEvent::Built {
                    kind: PartKind::Wall
                }
            )
        }) {
            built = true;
            break;
        }
        assert!(
            !events
                .iter()
                .any(|e| matches!(e, WorldEvent::BuildLost { .. })),
            "the metal was there the whole time"
        );
    }
    assert!(built, "the wall was not built in four hours");
    assert!(hauled, "nobody was seen hauling");
    assert!(built_seen, "nobody was seen building");
    assert!(held_the_ship, "the build never held the ship");
    assert!(sale_refused, "the sale was never tried");
    assert!(world.builds.is_empty());
    assert!(!world.under_construction());
    assert_eq!(world.ship.design.carrying(ResourceId::Metal), metal - 2);
    assert_eq!(world.ship.design.parts.len(), parts + 1);
    let grid = world.ship.design.grid();
    let wall = grid.get(Layer::Object, (at.0 as i32, at.1 as i32));
    assert_eq!(
        world.ship.design.part(wall).map(|p| p.kind),
        Some(PartKind::Wall)
    );
    assert!(close(world.ship.dynamics.mass.get(), mass));
    // The room aboard has the wall as a solid now: it is among what the
    // crew are kept out of.
    // In the joined room's units: the ship sits its offset into the deck.
    let middle = bims::aboard::tile_middle(at.0 as i32, at.1 as i32)
        + bims::math::vec2(world.aboard.offset.x as f32, world.aboard.offset.y as f32);
    let (_, solids) = world.aboard.room.route_for_probe(0);
    assert!(
        solids.iter().any(|s| s.contains(middle)),
        "the new wall is not a solid"
    );
    // And the crew are still the crew, with their needs where they were.
    assert_eq!(world.aboard.count(), 1);
}

/// A site beyond the hull is built from outside: deck plating laid out
/// against the skin has no tile beside it a body can stand on from the
/// deck, so the Bim takes the suit out through the airlock, carries the
/// load round to it and builds it out there — and the frame goes down
/// with the deck, since plating a bare tile lays both. The dose is the
/// walk's, as for mining.
#[test]
fn a_site_beyond_the_hull_is_built_in_a_suit() {
    use shipdesign::fixture::playtest_ship;
    use shipdesign::parts::Layer;
    let mut world = simulation_world(playtest_ship(), data::SIMULATION_MONEY, 1);
    world.undock_for_probe();
    let metal = world.ship.design.carrying(ResourceId::Metal);
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
    world.step(&[Command::PlaceSite {
        slot: 0,
        kind: PartKind::Floor,
        origin: at,
        rotation: Rotation::R0,
    }]);
    // Frame and deck: three metal.
    assert_eq!(
        world.build_orders()[0].haul,
        Some((ResourceId::Metal as u32, 3))
    );

    let mut went_out = false;
    let mut built = false;
    for _ in 0..(6 * 60 * 60) {
        let events = world.step(&[]);
        went_out |= world.aboard.room.is_outside(0);
        if events.iter().any(|e| {
            matches!(
                e,
                WorldEvent::Built {
                    kind: PartKind::Floor
                }
            )
        }) {
            built = true;
            break;
        }
    }
    assert!(built, "the plating was not laid in six hours");
    assert!(went_out, "the Bim never went outside for it");
    // And comes back in through the door when it is done.
    for _ in 0..(60 * 60) {
        if !world.aboard.room.is_outside(0) {
            break;
        }
        world.step(&[]);
    }
    assert!(
        !world.aboard.room.is_outside(0),
        "the Bim is still out there"
    );
    assert_eq!(world.ship.design.carrying(ResourceId::Metal), metal - 3);
    let grid = world.ship.design.grid();
    assert!(grid.has_structure((at.0 as i32, at.1 as i32)));
    assert!(grid.has_floor((at.0 as i32, at.1 as i32)));
    assert!(world.health[0].dose > 0.0, "the walk cost no dose");
}

/// The ship and the building keep off each other: nothing is laid out or
/// worked while the ship is not at rest, and a Confirm is refused while a
/// site has anything carried to it or anybody on the way — a bare
/// blueprint holds nothing, and a cancelled site frees what it had claimed.
#[test]
fn the_ship_does_not_move_while_built_on_and_is_not_built_on_while_moving() {
    use shipdesign::fixture::playtest_ship;
    let mut world = simulation_world(playtest_ship(), data::SIMULATION_MONEY, 1);
    let metal = world.ship.design.carrying(ResourceId::Metal);
    let place = |slot: u32| Command::PlaceSite {
        slot,
        kind: PartKind::Wall,
        origin: (9, 12),
        rotation: Rotation::R0,
    };

    // A blueprint with nothing done at it does not hold the ship. The Bim
    // is under orders so it does not take the site up the moment it is
    // laid — which it otherwise would, in the same step.
    world.aboard.room.recruit_for_probe(0, true);
    world.step(&[place(0)]);
    assert!(!world.under_construction());
    world.man_the_helm_for_probe(0);
    let target = reference_target(&world);
    let events = world.step(&[Command::Confirm { slot: 0, target }]);
    assert!(
        events
            .iter()
            .any(|e| matches!(e, WorldEvent::CastingOff { .. })),
        "{events:?}"
    );
    // Under way — the one crew member was aboard, so the ship is pushing
    // off already — nothing is worked and nothing is laid out.
    assert!(!world.at_rest());
    assert!(world.build_orders().is_empty());
    assert_eq!(
        world.can_place_site(PartKind::Wall, (9, 11), Rotation::R0),
        Err(crate::SiteRefusal::UnderWay)
    );
    let events = world.step(&[place(0)]);
    assert!(events.iter().any(|e| matches!(
        e,
        WorldEvent::Refused {
            why: Refusal::UnderWay,
            ..
        }
    )));
    assert_eq!(world.builds.len(), 1, "the refused site was placed");
    // Called off: the push-off finishes and the ship holds there. The
    // blueprint is still there, and now it is worked.
    world.step(&[Command::Abort { slot: 0 }]);
    until_stopped(&mut world, 60 * 60);
    assert_eq!(world.ship.state, ShipState::Holding);
    assert_eq!(world.build_orders().len(), 1);
    world.aboard.room.recruit_for_probe(0, false);

    // Once a load has been taken for it, the ship stays.
    let mut begun = false;
    for _ in 0..(60 * 60) {
        world.step(&[]);
        if world.builds[0].begun() {
            begun = true;
            break;
        }
    }
    assert!(begun, "nobody took a load in an hour");
    assert!(world.under_construction());
    assert_eq!(world.reserved(ResourceId::Metal), 2);
    assert_eq!(world.free(ResourceId::Metal), metal - 2);
    world.man_the_helm_for_probe(0);
    let events = world.step(&[Command::Confirm { slot: 0, target }]);
    assert!(
        events.iter().any(|e| matches!(
            e,
            WorldEvent::Refused {
                why: Refusal::UnderConstruction,
                ..
            }
        )),
        "{events:?}"
    );
    assert_eq!(world.ship.state, ShipState::Holding);

    // Cancelled, the claim is gone and nothing has left the hold. The Bim
    // on the way with the load finds the site gone and turns back.
    let events = world.step(&[Command::CancelSite { slot: 0, site: 1 }]);
    assert!(events.iter().any(|e| matches!(
        e,
        WorldEvent::SiteCancelled {
            kind: PartKind::Wall
        }
    )));
    assert!(world.builds.is_empty());
    assert_eq!(world.reserved(ResourceId::Metal), 0);
    assert_eq!(world.ship.design.carrying(ResourceId::Metal), metal);
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
    // And with nothing holding it, the ship goes.
    world.man_the_helm_for_probe(0);
    let events = world.step(&[Command::Confirm { slot: 0, target }]);
    assert!(
        events
            .iter()
            .any(|e| matches!(e, WorldEvent::Departed { .. })),
        "{events:?}"
    );
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
    // Both are carried to; neither is built yet, and the wall will not be
    // until its deck is there: what it wants is on the order, and the
    // room only puts together a site the world gives minutes for.
    let orders = world.build_orders();
    assert_eq!(orders.len(), 2);
    assert_eq!(orders[0].site, 1);
    assert_eq!(orders[1].site, 2);
    assert!(orders[1].haul.is_some());
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

    // Under way, the station's room is nobody's to look into.
    let target = nearby(&world, 4_000.0);
    set_off(&mut world, 0, target);
    let ashore = world.residents.as_ref().unwrap();
    for who in 0..ashore.aboard.count() {
        assert!(!ashore.aboard.room.body_seen(who as usize));
    }
}

/// A bay the crew built — or any bay past the first — is a bay like the
/// first: it is tended, a click on it is a click on a bay, and it has a
/// switch and a standing order of its own.
/// The same, the way the player does it: a bay laid out as a site, its
/// materials carried to it and put together by the crew — and once it is
/// built, the room has two bays and both are tended.
#[test]
fn every_bay_aboard_is_worked_and_one_the_crew_build_is_a_bay_like_the_first() {
    // --- every_bay_aboard_is_worked_and_has_its_own_menu ---
    {
        use bims::room::HIT_HYDRO;
        use shipdesign::fixture::playtest_ship;
        // The playtest ship with a second bay wherever one fits.
        let ship = playtest_ship();
        let budget = Budget::new(10_000_000);
        let area = ship.build_area;
        let mut with_two = None;
        'find: for y in 0..area {
            for x in 0..area {
                let edit = Edit::Place {
                    kind: PartKind::HydroBay,
                    origin: (x, y),
                    rotation: Rotation::R0,
                };
                if let Ok(next) = apply(&ship, &budget, edit) {
                    // Placed and sound, as the designer would insist on: every
                    // tray reachable, nothing walled off.
                    let sound = shipdesign::validate(&next, 1)
                        .iter()
                        .all(|i| i.severity != shipdesign::validate::Severity::Error);
                    if !sound {
                        continue;
                    }
                    with_two = Some(next);
                    break 'find;
                }
            }
        }
        let design = with_two.expect("somewhere on the playtest ship a second bay fits");
        let mut world = simulation_world(design, data::SIMULATION_MONEY, 1);
        let room = &mut world.aboard.room;
        assert_eq!(room.hydro_bays(), 2);

        // A click on the second bay is a click on a bay — that bay.
        let frame = room.bay_frame_for_probe(1);
        let at = frame.center();
        assert_eq!(room.hit_at(at.x, at.y), HIT_HYDRO);
        assert_eq!(room.hit_bay(), 1);
        // And its own settings: a standing order on one is not one on the other.
        room.set_hydro_forced(1, 1);
        assert_eq!(room.hydro_forced(1), 1);
        assert_eq!(room.hydro_forced(0), 0);
        room.set_hydro_forced(0, 1);

        // Both bays are worked: over a few days, a tray of each changes hands.
        let spots = room.hydro_spots();
        let snapshot = |room: &bims::game::Game, bay: usize| -> Vec<u32> {
            (0..spots).map(|i| room.hydro_crop(bay, i)).collect()
        };
        let mut before = [snapshot(room, 0), snapshot(room, 1)];
        let mut worked = [false, false];
        for _ in 0..(3 * 24 * 60 * 60) {
            world.step(&[]);
            for bay in 0..2 {
                let now = snapshot(&world.aboard.room, bay);
                if now != before[bay] {
                    worked[bay] = true;
                    before[bay] = now;
                }
            }
            if worked.iter().all(|&w| w) {
                break;
            }
        }
        assert!(worked[0], "the first bay was never tended");
        assert!(worked[1], "the second bay was never tended");
    }

    // --- a_bay_the_crew_build_is_a_bay_like_the_first ---
    {
        use bims::room::HIT_HYDRO;
        use shipdesign::fixture::playtest_ship;
        let mut world = simulation_world(playtest_ship(), data::SIMULATION_MONEY, 1);
        assert_eq!(world.aboard.room.hydro_bays(), 1, "the ship's own, docked");
        // Somewhere on the deck the site is allowed.
        let area = world.ship.design.build_area;
        let mut at = None;
        'find: for y in 0..area {
            for x in 0..area {
                if world
                    .can_place_site(PartKind::HydroBay, (x, y), Rotation::R0)
                    .is_ok()
                {
                    at = Some((x, y));
                    break 'find;
                }
            }
        }
        let at = at.expect("somewhere on the playtest ship a bay can be laid out");
        let events = world.step(&[Command::PlaceSite {
            slot: 0,
            kind: PartKind::HydroBay,
            origin: at,
            rotation: Rotation::R0,
        }]);
        assert!(
            events
                .iter()
                .any(|e| matches!(e, WorldEvent::SitePlaced { .. })),
            "{events:?}"
        );

        // Built: carried to and put together, within a few days of the crew's
        // own time, at the berth.
        let mut built = false;
        for _ in 0..(3 * 24 * 60 * 60) {
            let events = world.step(&[]);
            if events.iter().any(|e| matches!(e, WorldEvent::Built { .. })) {
                built = true;
                break;
            }
        }
        assert!(built, "the bay was never built");
        let room = &mut world.aboard.room;
        assert_eq!(
            room.hydro_bays(),
            2,
            "the built bay is not a bay of the room"
        );
        let frame = room.bay_frame_for_probe(1);
        let m = frame.center();
        assert_eq!(room.hit_at(m.x, m.y), HIT_HYDRO);
        assert_eq!(room.hit_bay(), 1);
        room.set_hydro_forced(1, 2);
        assert_eq!(room.hydro_forced(1), 2);

        // And it is worked: a tray of it changes hands.
        let spots = room.hydro_spots();
        let snapshot = |room: &bims::game::Game| -> Vec<u32> {
            (0..spots).map(|i| room.hydro_crop(1, i)).collect()
        };
        let before = snapshot(room);
        let mut worked = false;
        for _ in 0..(3 * 24 * 60 * 60) {
            world.step(&[]);
            if snapshot(&world.aboard.room) != before {
                worked = true;
                break;
            }
        }
        assert!(worked, "the built bay was never tended");
    }
}

/// Two galleys, two cooks: with a second hob, worktop and cold store
/// aboard, two hungry Bims cook at the same time, each at fixtures of its
/// own — the closest free — rather than one waiting for the other to
/// finish. And with one galley, they still take turns.
#[test]
fn a_second_galley_is_cooked_in_at_the_same_time() {
    use bims::game::JOB_MEAL;
    use shipdesign::fixture::playtest_ship;
    let budget = Budget::new(10_000_000);
    // A second bunk and chair for the second Bim, then a second hob,
    // worktop and cold store, each wherever one fits without making the
    // ship any worse than it was — the designer's errors for a crew of
    // two are what they are until the bunk and the chair are in.
    let mut design = playtest_ship();
    let errors = |d: &ShipDesign| {
        shipdesign::validate(d, 2)
            .iter()
            .filter(|i| i.severity == shipdesign::validate::Severity::Error)
            .count()
    };
    for kind in [
        PartKind::Bunk,
        PartKind::Chair,
        PartKind::Hob,
        PartKind::Worktop,
        PartKind::ColdStore,
    ] {
        let before = errors(&design);
        let area = design.build_area;
        let mut placed = None;
        'find: for y in 0..area {
            for x in 0..area {
                for rotation in [Rotation::R0, Rotation::R90] {
                    let edit = Edit::Place {
                        kind,
                        origin: (x, y),
                        rotation,
                    };
                    if let Ok(next) = apply(&design, &budget, edit)
                        && errors(&next) <= before
                    {
                        placed = Some(next);
                        break 'find;
                    }
                }
            }
        }
        design = placed.unwrap_or_else(|| panic!("nowhere on the playtest ship for a {kind:?}"));
    }
    let mut world = simulation_world(design, data::SIMULATION_MONEY, 2);
    let room = &mut world.aboard.room;
    room.select_group(0, 1);
    // Both starving this instant: the food need spent to nothing.
    room.spend_for_probe(0, 1, 1.0);
    room.spend_for_probe(1, 1, 1.0);

    let mut together = false;
    for _ in 0..(2 * 60 * 60) {
        world.step(&[]);
        let room = &world.aboard.room;
        if room.activity(0) == JOB_MEAL && room.activity(1) == JOB_MEAL {
            let (a, b) = (
                room.picks_for_probe(0).unwrap(),
                room.picks_for_probe(1).unwrap(),
            );
            // Whatever each has picked so far, it is not the other's.
            let apart = |x: Option<usize>, y: Option<usize>| x.is_none() || y.is_none() || x != y;
            assert!(apart(a.fridge, b.fridge), "{a:?} {b:?}");
            assert!(apart(a.worktop, b.worktop), "{a:?} {b:?}");
            assert!(apart(a.hob, b.hob), "{a:?} {b:?}");
            if a.hob.is_some() && b.hob.is_some() {
                together = true;
                break;
            }
        }
    }
    assert!(together, "the two never cooked at once");
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
    world.set_hostile(station_id, true);
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

    // The residents: an enemy seen stays drawn for a moment after it is
    // out of view, and no longer.
    world.step(&[]);
    let ashore = world.residents.as_mut().unwrap();
    // The garrison, not the two residents: `enemies_of` the crew.
    let crowd = ashore.aboard.count() as usize;
    assert!(crowd > 2);
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

/// A recruited crew member at a hostile station draws its laser and
/// shoots at any of the station's people it can see, and what lands comes
/// off their health — in their own room, since the shot is fired in the
/// crew's. The bolts fly rather than land at once, and are blue.
///
/// The station's people shoot back now, and two of them against one is
/// not a fight the crew member wins — `the_residents_shoot_back…` is
/// that half — so resident 1 is taken out of it first, by a probe's shot
/// to the head, and the run is one on one: what this pins is that the
/// crew member's fire lands and ends it.
#[test]
fn a_recruited_bim_shoots_the_enemies_it_can_see_and_they_are_hurt() {
    use bims::sight::Stance;
    let mut world = basic();
    let station_id = world.residents.as_ref().unwrap().station;
    world.set_hostile(station_id, true);
    assert_eq!(world.stance(station_id), Stance::Hostile);

    // Everybody carries the issued pistol, holstered until recruited.
    let gear = world.aboard.room.gear(0);
    assert_eq!(
        gear.weapon,
        Some(bims::combat::WeaponKind::LaserPistol.basic())
    );
    assert!(gear.head.is_none() && gear.body.is_none() && gear.legs.is_none());
    let stats = world.aboard.room.weapon_stats(0).unwrap();
    assert_eq!(stats.dps(), stats.fire_rate * stats.damage);
    assert!(
        (stats.hit_chance(10.0) - 0.732).abs() < 0.01,
        "the pistol at ten tiles"
    );
    world.step(&[]);
    assert!(!world.aboard.room.is_armed(0), "holstered");

    // James, recruited, stood a tile or two from resident 0 in the
    // resident's own room — the station frame put through the join.
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
    // The rest of the garrison out of the fight, so it is one on one —
    // and James patched up before every step, since resident 0 shoots
    // back at a tile and a half and a one-in-twenty head shot would end
    // the run before his fire has: what this pins is his fire landing.
    let ashore = world.residents.as_mut().unwrap();
    one_on_one(&mut ashore.aboard.room, 0);
    world.aboard.room.recruit_for_probe(0, true);
    world.aboard.room.observe();

    let before = world.residents.as_ref().unwrap().aboard.room.health(0);
    let mut flew = false;
    let mut hurt = false;
    for _ in 0..3_000 {
        world.aboard.room.patch_up_for_probe(0);
        let events = world.step(&[]);
        world.aboard.room.observe();
        assert!(world.aboard.room.is_armed(0), "weapon drawn");
        if world.aboard.room.bolts_in_flight() > 0 {
            flew = true;
        }
        let ashore = world.residents.as_ref().unwrap();
        if ashore.aboard.room.is_dying(0) || ashore.aboard.room.is_down(0) {
            hurt = true;
            break;
        }
        assert!(
            !events
                .iter()
                .any(|e| matches!(e, WorldEvent::EnemyDown { who: 0, .. })),
            "not down yet"
        );
    }
    assert!(flew, "a bolt was in the air at some point");
    let ashore = world.residents.as_ref().unwrap();
    let after = ashore.aboard.room.health(0);
    assert!(after < before, "resident 0 was hit: {before} -> {after}");
    assert!(
        hurt,
        "and was shot to a dying state, or out cold, within the run"
    );
    // A part at nothing is a dying state, not a death: it is alive, it is
    // running from James, and nothing says `EnemyDown` until it has bled
    // out or been treated — of which the garrison's one medkit is the
    // only chance. Out cold, it is nobody's target: James holds his fire.
    if ashore.aboard.room.is_alive(0) {
        let events = world.step(&[]);
        assert!(
            !events
                .iter()
                .any(|e| matches!(e, WorldEvent::EnemyDown { who: 0, .. }))
        );
        let ashore = world.residents.as_ref().unwrap();
        if ashore.aboard.room.is_unconscious(0) {
            assert!(
                world.aboard.room.combat_targets_for_probe()[0].is_none(),
                "out cold is not a target"
            );
        } else {
            assert!(ashore.aboard.room.is_dying(0));
            assert!(ashore.aboard.room.is_fleeing(0), "and runs from him");
            assert!(!ashore.aboard.room.is_armed(0), "without shooting back");
        }
    }
    // The event names the person: station in the thousands, seat in the
    // units.
    let value = WorldEvent::EnemyDown {
        station: station_id,
        who: 0,
    }
    .value();
    assert_eq!(value, (station_id * 1_000) as i64);

    // Let go, the weapon is holstered again and nothing more is fired.
    world.aboard.room.recruit_for_probe(0, false);
    world.step(&[]);
    assert!(!world.aboard.room.is_armed(0));
}

/// At a hostile station the fight goes both ways: the station's people
/// know where the crew are, shoot at them, and what lands opens a wound
/// on the crew member — in the joined room, where the bolt flew — and
/// the world says so. `stage_fight_for_probe` is the start of it.
#[test]
fn the_residents_shoot_back_and_a_crew_member_hit_bleeds() {
    let mut world = basic();
    assert!(world.stage_fight_for_probe());
    let station_id = world.residents.as_ref().unwrap().station;
    assert_eq!(world.stance(station_id), bims::sight::Stance::Hostile);
    assert_eq!(world.aboard.room.bleeding(0), 0);
    assert_eq!(world.aboard.room.health(0), bims::health::MAX_HEALTH);

    let mut hit = None;
    for _ in 0..3_000 {
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
    let hit = hit.expect("the residents hit James within the run");
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

/// Casting off from a hostile station ends the fight for the crew too:
/// the undock builds the crew a fresh room, at peace, and the crewmates
/// the alarm put under arms come into it stood down — holstered and back
/// to their errands, unless the hold on a last hit is still running.
/// They used to come in still recruited: the fresh room musters on the
/// alarm's edge and its alarm had never been up, so nothing let them go
/// and they stood in combat mode the whole flight.
#[test]
fn casting_off_from_a_hostile_station_stands_the_crew_down() {
    let mut world = with_a_crewmate();
    assert!(world.stage_fight_for_probe());
    // The targets are handed over after the room has stepped, so the
    // alarm is a step behind them.
    for _ in 0..5 {
        world.step(&[]);
    }
    assert!(
        world.aboard.room.is_alarmed(),
        "the alarm is up at the dock"
    );
    assert!(
        world
            .aboard
            .room
            .combat_targets_for_probe()
            .iter()
            .any(|t| t.is_some()),
        "the garrison are targets"
    );
    assert!(world.aboard.room.is_armed(1), "Kate is under arms");

    world.undock_for_probe();
    world.step(&[]);
    assert!(
        world
            .aboard
            .room
            .combat_targets_for_probe()
            .iter()
            .all(|t| t.is_none()),
        "nobody is a target once the rooms are apart"
    );
    // Down at once unless somebody was hit in that one step, in which
    // case the hold on it runs out and nothing re-arms it out here.
    let hold = (bims::game::ALARM_HOLD * 60.0) as usize + 60;
    let mut down = false;
    for _ in 0..hold {
        if !world.aboard.room.is_alarmed() {
            down = true;
            break;
        }
        world.step(&[]);
    }
    assert!(down, "the alarm comes down in flight");
    // James still has his own weapon out — `stage_fight_for_probe`
    // recruited him — and since feature 84 that is a player leading the
    // crew, so Kate keeps hers out and stays at his side. He holsters,
    // and she stands down to her errands.
    assert!(
        world.aboard.room.is_armed(1),
        "Kate follows James under arms"
    );
    world.aboard.room.recruit_for_probe(0, false);
    world.step(&[]);
    assert!(!world.aboard.room.is_armed(1), "and Kate holsters");
}

/// At war a resident is recruited and armed, and — with James in its
/// sight four tiles off and no cover worth the walk on this seed — it
/// stands where it is and shoots, since walking would halve its odds
/// (`bims::game::plan_stand`); its bolts fly in the crew's room.
/// A station's people are armed off its own seed and their seat, so the
/// same station arms the same people the same way every time it is
/// reached and the checksum has nothing new to hash.
#[test]
fn a_hostile_station_s_people_take_arms_and_are_armed_off_its_seed() {
    // --- a_hostile_station_s_people_take_arms_and_shoot_from_where_they_stand ---
    {
        let mut world = basic();
        assert!(world.stage_fight_for_probe());
        let was = world.residents.as_ref().unwrap().aboard.position(0);
        let stood = world.residents.as_ref().unwrap().aboard.room.is_armed(0);
        assert!(!stood, "at peace a resident carries nothing drawn");
        // James unarmed, so the resident is not shot to a dying run before it
        // has fired; patched up every step against its pistol.
        world.aboard.room.issue(0, bims::combat::Gear::default());
        world.aboard.room.issue(1, bims::combat::Gear::default());
        let mut fired = false;
        for _ in 0..300 {
            world.aboard.room.patch_up_for_probe(0);
            world.step(&[]);
            fired |= world.aboard.room.bolts_in_flight() > 0;
            if fired {
                break;
            }
        }
        let ashore = world.residents.as_ref().unwrap();
        assert!(ashore.aboard.room.is_armed(0), "under arms");
        assert!(fired, "and shooting, a hostile bolt in the crew's room");
        let now = ashore.aboard.position(0);
        assert!(
            was.distance(now) <= shipdesign::TILE as f64,
            "stood still to shoot: {was:?} -> {now:?}"
        );
        // Home again, nobody is anybody's target and the arms go away.
        world.set_hostile(world.residents.as_ref().unwrap().station, false);
        for _ in 0..3 {
            world.step(&[]);
        }
        let ashore = world.residents.as_ref().unwrap();
        assert!(!ashore.aboard.room.is_armed(0), "stood down");
    }

    // --- a_station_s_people_are_armed_off_its_seed ---
    {
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
}

/// Where a resident is put on the joined deck, and where a crew member
/// is, in the room's units — the peek while either peeks, since that is
/// where a shot at it is aimed.
fn on_deck(world: &World, who_ashore: u32) -> bims::math::Vec2 {
    let (origin, ex, ey) = world.aboard.station_frame.unwrap();
    let p = world.residents.as_ref().unwrap().aboard.exposed(who_ashore);
    let at = origin.add(ex.scale(p.x)).add(ey.scale(p.y));
    bims::math::vec2(at.x as f32, at.y as f32)
}

/// A resident with a schword charges the crew member rather than
/// standing off, and within reach the two are locked in a melee: the
/// world says so once, the crew member fires nothing more while it holds
/// and lands fists instead — a blow the world carries to the resident's
/// body — and the blade's cut comes back the other way, three wound units
/// on the part it landed on. James wears the kevlar so the first cut is
/// one he lives through: seventy on a body of seventy-five is otherwise
/// the whole fight, over the step the blade arrives.
#[test]
fn a_blade_charges_and_locks_the_crew_member_who_fights_with_its_fists() {
    use bims::combat::{ArmourKind, FIST_DAMAGE, Gear, MELEE_RANGE, Piece, WeaponKind};
    use bims::health::{CUT_WOUND, Part};
    let mut world = basic();
    assert!(world.stage_fight_for_probe());
    world.aboard.room.issue(
        0,
        Gear {
            body: Some(Piece::new(99, ArmourKind::BasicKevlar, Tier::One)),
            ..Gear::issued()
        },
    );
    // Kate unarmed: from the ship, a pistol reaching twenty-two tiles has
    // her firing at the resident too, and a bolt of hers in the air would
    // read as James firing while locked.
    world.aboard.room.issue(1, Gear::default());
    // Resident 0 with a blade, resident 1 out of it, so it is one on one.
    {
        let ashore = world.residents.as_mut().unwrap();
        ashore.aboard.room.issue(
            0,
            Gear {
                weapon: Some(WeaponKind::Schword.basic()),
                ..Gear::default()
            },
        );
        assert_eq!(
            ashore.aboard.room.weapon(0),
            Some(WeaponKind::Schword.basic())
        );
        one_on_one(&mut ashore.aboard.room, 0);
    }
    assert_eq!(WorldEvent::Locked { who: 0 }.code(), 37);
    assert_eq!(WorldEvent::Locked { who: 1 }.value(), 1);

    let mut locked = false;
    let mut cut = None;
    let (mut before, mut units_before) = (0.0, 0);
    for _ in 0..3_000 {
        // The resident good as new before every step: a pistol reaching
        // twenty-two tiles, or a fist on the legs — a fifth of them, and
        // the legs are twenty — would otherwise have it dying and running
        // before the lock is read, and what this pins is the lock.
        world
            .residents
            .as_mut()
            .unwrap()
            .aboard
            .room
            .patch_up_for_probe(0);
        let ashore = &world.residents.as_ref().unwrap().aboard.room;
        before = ashore.health(0);
        units_before = Part::ALL.iter().map(|&p| ashore.wounds(0, p)).sum::<u32>();
        let events = world.step(&[]);
        world.aboard.room.observe();
        if let Some(WorldEvent::CrewHit { part, .. }) = events
            .iter()
            .find(|e| matches!(e, WorldEvent::CrewHit { who: 0, .. }))
        {
            cut = Some(Part::from_code(*part).unwrap());
        }
        if events.contains(&WorldEvent::Locked { who: 0 }) {
            locked = true;
            break;
        }
    }
    assert!(locked, "the blade closed and locked James within the run");
    assert_eq!(world.aboard.room.is_locked(0), Some(0));
    assert!(world.aboard.room.is_armed(0), "still under arms");
    // Within reach of each other, as the world put them to each other.
    let gap = (on_deck(&world, 0) - world.aboard.room.exposed_at(0)).len();
    assert!(
        gap <= (MELEE_RANGE + 0.5) * shipdesign::TILE as f32,
        "within reach: {gap}"
    );
    // The first swing starts the step the lock forms and lands when it
    // has been swung, `SWING_TIME` on, and it is a fist: twenty off the
    // resident and one wound unit, not a cut's three — give or take a
    // last pistol bolt landing in the meantime.
    // Read step by step, since a fed body mends a little every step.
    let step = (data::STEP_MINUTES / time::MINUTES_PER_SECOND) as f32;
    let swing = (bims::character::SWING_TIME / step).ceil() as usize + 2;
    let health_ashore = |world: &World| world.residents.as_ref().unwrap().aboard.room.health(0);
    let units_ashore = |world: &World| {
        let ashore = &world.residents.as_ref().unwrap().aboard.room;
        Part::ALL.iter().map(|&p| ashore.wounds(0, p)).sum::<u32>()
    };
    let (mut before, mut after) = (before, health_ashore(&world));
    let mut units_before = units_before;
    let mut units = units_ashore(&world);
    for _ in 0..swing {
        if before - after >= FIST_DAMAGE - 1e-3 {
            break;
        }
        world
            .residents
            .as_mut()
            .unwrap()
            .aboard
            .room
            .patch_up_for_probe(0);
        after = health_ashore(&world);
        units = units_ashore(&world);
        before = after;
        units_before = units;
        world.step(&[]);
        world.aboard.room.observe();
        after = health_ashore(&world);
        units = units_ashore(&world);
    }
    assert!(
        before - after >= FIST_DAMAGE - 1e-3,
        "a fist landed: {before} -> {after}"
    );
    assert!(
        units > units_before && units - units_before < CUT_WOUND,
        "a fist's wound, not a cut's: {units_before} -> {units}"
    );

    // While the lock holds nothing more is fired, and the world does not
    // say the lock again. The blade's own blow comes the other way — a
    // cut, three units on the part it landed on, said as a hit — before
    // or after James's lock, since the blade reads its reach a step
    // ahead of him.
    let mut bolts = world.aboard.room.bolts_in_flight();
    let mut held = 0;
    for _ in 0..600 {
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
        if let Some(WorldEvent::CrewHit { part, .. }) = events
            .iter()
            .find(|e| matches!(e, WorldEvent::CrewHit { who: 0, .. }))
        {
            cut = Some(Part::from_code(*part).unwrap());
        }
        if cut.is_some() && held > 10 {
            break;
        }
    }
    assert!(held > 0, "the lock held for a step at least");
    let part = cut.expect("the blade landed on James");
    let units = world.aboard.room.wounds(0, part);
    assert!(
        units >= CUT_WOUND && units % CUT_WOUND == 0,
        "a cut is three units: {units}"
    );
    assert!(world.aboard.room.bleeding(0) >= CUT_WOUND);
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
    use bims::combat::{Gear, WeaponKind};
    let mut world = basic();
    assert!(world.stage_fight_for_probe());
    {
        let ashore = world.residents.as_mut().unwrap();
        ashore.aboard.room.issue(
            0,
            Gear {
                weapon: Some(WeaponKind::Schword.basic()),
                ..Gear::default()
            },
        );
        one_on_one(&mut ashore.aboard.room, 0);
    }
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
    // The resident six tiles east of it, in the corridor's middle row,
    // held there.
    let there = bims::math::vec2((col + 6.5) * tile, (corridor_row + 0.5) * tile);

    let mut peeked = false;
    for _ in 0..600 {
        let ashore_room = world.residents.as_mut().unwrap();

        let there_in_room = ashore_room
            .aboard
            .to_room(dvec2(there.x as f64, there.y as f64));

        ashore_room.aboard.room.put_for_probe(0, there_in_room);
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
/// does, both ways round. A *resident* issued the rifle walks off down
/// the corridor to the open at its range — never a doorway, which opens
/// for whoever stands in it (`bims::combat::Tactics::stand`) — and its
/// shot lands on James from twenty tiles and more, said as a `CrewHit`.
/// Then the crew member's: the bolt flies the whole corridor of the
/// joined deck and lands with the rifle's damage at that distance, and
/// the world carries the hit to the body — the resident stood down the
/// corridor with a blade, put back where it was before every step, so
/// the distance is the one asked for.
/// The crew's shotgun does more up close than down the corridor: the
/// damage is the weapon's at the distance the bolt flew, read when it
/// lands, and the world carries that number to the resident's body.
/// Both stood still — put back where they were before every step — so
/// the distance is the one asked for.
#[test]
fn a_sniper_rifle_reaches_from_twenty_tiles_and_a_shotgun_does_more_at_three_than_at_nine() {
    // --- a_sniper_rifle_reaches_from_twenty_tiles ---
    {
        use bims::combat::{Gear, WeaponKind};
        use bims::health::Part;
        let stats = WeaponKind::SniperRifle.stats();
        let rifle = Gear {
            weapon: Some(WeaponKind::SniperRifle.basic()),
            ..Gear::default()
        };

        // A resident with the rifle walks off to its range rather than
        // closing, stands where it can shoot from, and hits from there. An
        // enemy shoots on the move too — at half the odds — and a rifle at
        // four tiles is the end of an unarmoured James, so he is patched up
        // before every step and the hit counted is the first landed from a
        // stand: what this pins is where the resident goes to shoot from,
        // not what it does on the way. James is unarmed for it: a pistol
        // reaching twenty-two tiles had the resident shot to a dying state
        // — and running — before it ever reached its stand.
        let mut world = basic();
        assert!(world.stage_fight_for_probe());
        // And so is the rest of the crew: under the alarm a crewmate with a
        // pistol shoots what she sees, and the lobby's layout is not the
        // test's to lean on.
        for who in 0..world.aboard.crew_count() as usize {
            world.aboard.room.issue(who, Gear::default());
        }
        {
            let ashore = world.residents.as_mut().unwrap();
            ashore.aboard.room.issue(0, rifle);
            one_on_one(&mut ashore.aboard.room, 0);
        }
        let ashore = world.aboard.ashore.unwrap();
        let ashore = bims::math::vec2(ashore.x as f32, ashore.y as f32);
        let mut hit_at = None;
        for _ in 0..3_000 {
            world.aboard.room.put_for_probe(0, ashore);
            world.aboard.room.patch_up_for_probe(0);
            let gap = (on_deck(&world, 0) - world.aboard.room.exposed_at(0)).len()
                / shipdesign::TILE as f32;
            let events = world.step(&[]);
            let standing = !world.residents.as_ref().unwrap().aboard.room.is_walking(0);
            if standing
                && events
                    .iter()
                    .any(|e| matches!(e, WorldEvent::CrewHit { who: 0, .. }))
            {
                hit_at = Some(gap);
                break;
            }
        }
        let gap = hit_at.expect("the resident's rifle hit James within the run");
        // Out of the pistol's reach and inside the rifle's: on the default
        // seed it is the cover at the corridor's crossing, some seventeen
        // tiles off, and cover near it beats the open further on.
        let pistol = WeaponKind::LaserPistol.stats().range;
        assert!(gap > pistol, "from its range, not up close: {gap:.1} tiles");
        assert!(gap * shipdesign::TILE as f32 <= stats.reach());
        let room = &world.residents.as_ref().unwrap().aboard.room;
        assert!(!room.is_walking(0), "stood still to shoot");
        assert!(world.aboard.room.bleeding(0) > 0, "and James bleeds for it");

        // The long shot: James with the rifle, the resident twenty-one tiles
        // down the corridor.
        let mut world = basic();
        assert!(world.stage_fight_for_probe());
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
        // first wound counted would be her pistol's.
        world.aboard.room.issue(1, Gear::default());
        {
            let ashore = world.residents.as_mut().unwrap();
            ashore.aboard.room.issue(
                0,
                Gear {
                    weapon: Some(WeaponKind::Schword.basic()),
                    ..Gear::default()
                },
            );
            one_on_one(&mut ashore.aboard.room, 0);
        }
        let mut hit_from = None;
        for _ in 0..3_000 {
            let ashore_room = world.residents.as_mut().unwrap();

            let there_in_room = ashore_room
                .aboard
                .to_room(dvec2(there.x as f64, there.y as f64));

            ashore_room.aboard.room.put_for_probe(0, there_in_room);
            world.aboard.room.put_for_probe(0, ashore);
            let gap = (on_deck(&world, 0) - world.aboard.room.exposed_at(0)).len();
            world.step(&[]);
            world.aboard.room.observe();
            let room = &world.residents.as_ref().unwrap().aboard.room;
            if let Some(part) = Part::ALL.into_iter().find(|&p| room.wounds(0, p) > 0) {
                hit_from = Some((gap / shipdesign::TILE as f32, part));
                break;
            }
        }
        let (hit_from, part) = hit_from.expect("the rifle hit the resident within the run");
        assert!(
            (20.0..=tiles as f32 + 1.0).contains(&hit_from),
            "from twenty tiles and more: {hit_from:.1} tiles"
        );
        assert!(hit_from * shipdesign::TILE as f32 <= stats.reach());
        // And it was the rifle's damage at that distance that landed: forty-five
        // at twenty tiles, which is all a head or a leg has and most of a body.
        let dealt = stats.damage_at(hit_from).min(part.max());
        let room = &world.residents.as_ref().unwrap().aboard.room;
        let left = room.part_health(0, part);
        assert!(
            (part.max() - left - dealt).abs() < 4.0
                || (part == Part::Legs && room.legs_lost(0) == 1),
            "the rifle's damage came off the part: {left} left of {}, {dealt} dealt",
            part.max()
        );
        assert_eq!(room.wounds(0, part), 1, "a shot, not a cut");
    }

    // --- the_crew_s_shotgun_does_more_at_three_tiles_than_at_nine ---
    {
        use bims::combat::{Gear, WeaponKind};
        use bims::health::Part;
        let stats = WeaponKind::Shotgun.stats();
        // The first hit on the body, on a body still whole: what came off it.
        let body_drop_at = |tiles: f64| -> (f32, f32) {
            let mut world = basic();
            assert!(world.stage_fight_for_probe());
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
            {
                // A blade in the resident's hand, so nothing is fired back,
                // and resident 1 out of it.
                let ashore = world.residents.as_mut().unwrap();
                ashore.aboard.room.issue(
                    0,
                    Gear {
                        weapon: Some(WeaponKind::Schword.basic()),
                        ..Gear::default()
                    },
                );
                one_on_one(&mut ashore.aboard.room, 0);
            }
            for _ in 0..3_000 {
                let ashore_room = world.residents.as_mut().unwrap();

                let there_in_room = ashore_room
                    .aboard
                    .to_room(dvec2(there.x as f64, there.y as f64));

                ashore_room.aboard.room.put_for_probe(0, there_in_room);
                world.aboard.room.put_for_probe(0, ashore);
                let flown = (on_deck(&world, 0) - world.aboard.room.exposed_at(0)).len()
                    / shipdesign::TILE as f32;
                world.step(&[]);
                world.aboard.room.observe();
                let ashore = world.residents.as_ref().unwrap();
                if ashore.aboard.room.wounds(0, Part::Body) > 0 {
                    return (
                        Part::Body.max() - ashore.aboard.room.part_health(0, Part::Body),
                        flown,
                    );
                }
                assert!(
                    ashore.aboard.room.is_alive(0) && ashore.aboard.room.health(0) > 0.0,
                    "the body is the part to wait for"
                );
            }
            panic!("no hit on the body within the run at {tiles} tiles");
        };
        let (near, near_from) = body_drop_at(3.0);
        let (far, far_from) = body_drop_at(9.0);
        assert!(
            (near_from - 3.0).abs() < 1.0 && (far_from - 9.0).abs() < 1.0,
            "stood where asked: {near_from:.1} and {far_from:.1} tiles"
        );
        // Up close the curve's full number, fifty: most of a body.
        assert!(
            (near - stats.damage_at(near_from)).abs() < 4.0,
            "the weapon's damage up close: {near} at {near_from:.1} tiles"
        );
        // Down the corridor, the curve's number, which is less.
        assert!(far < near, "less at nine tiles: {far} < {near}");
        assert!(
            (far - stats.damage_at(far_from)).abs() < 4.0,
            "the weapon's damage at the distance flown: {far} at {far_from:.1} tiles"
        );
    }
}

/// A bandage is a thing in a pack since feature 87: the crew member
/// fills up out of the hold out of combat, dresses a wound of their own
/// with one of its own dressings, and the hold pays for it — the pack is
/// topped up again next step, so what falls by one is the hold and the
/// packs added up. The playtest ship carries a few from the first minute.
/// A treatment aboard fetches its kit from a cabinet that holds one — the
/// armoury or the drug lab on the combat ship, whose spots the world
/// hands the room every step (`Game::set_kit_stands`) — and the hold pays
/// for it when the hands come off: two kits aboard, one after.
#[test]
fn a_wound_is_dressed_with_a_bandage_from_the_hold_and_a_treatment_fetches_the_kit() {
    // --- a_crew_member_dresses_a_wound_with_a_bandage_from_the_hold ---
    {
        use bims::health::Part;
        use shipdesign::fixture::playtest_ship;
        let mut world = simulation_world(playtest_ship(), data::SIMULATION_MONEY, 1);
        let bandages = world.ship.design.carrying(ResourceId::Bandage);
        assert!(bandages > 0, "the playtest ship carries bandages");
        // The restock fills the pack out of the hold, one a step, out of
        // combat: the two added up never move on their own.
        let wanted = world
            .aboard
            .room
            .target(bims::manager::Stock::Bandages)
            .min(bandages);
        for _ in 0..(wanted + 1) {
            world.step(&[]);
        }
        assert_eq!(
            world.aboard.room.bandages_of(0),
            wanted,
            "the crew member carries what the Management tab asks for"
        );
        assert_eq!(
            world.ship.design.carrying(ResourceId::Bandage) + wanted,
            bandages,
            "and they came out of the hold"
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
        // A step or two later the pack is full again and the hold is one
        // lighter: the dressing was spent, whichever pocket it came from.
        for _ in 0..3 {
            world.step(&[]);
        }
        assert_eq!(
            world.ship.design.carrying(ResourceId::Bandage) + world.aboard.room.bandages_of(0),
            bandages - 1,
            "and the dressing is gone for good"
        );
        // A part with nothing open on it is not worth a bandage.
        assert!(!world.aboard.room.bandage(0, 0, Part::Body));
    }

    // --- a_treatment_aboard_fetches_the_kit_from_a_cabinet_and_the_hold_pays_for_it ---
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
        assert_eq!(world.aboard.room.medkits(), kits, "the room is told");
        // Where a kit is fetched from: every container that takes one, and
        // there is at least one such cabinet aboard.
        let stands: Vec<bims::math::Vec2> = world
            .aboard
            .containers()
            .into_iter()
            .filter(|&c| world.container_takes(c, ResourceId::Medkit))
            .filter_map(|c| world.aboard.room.container_spot(c))
            .collect();
        assert!(!stands.is_empty(), "somewhere aboard takes a medkit");

        // Kate dying of a body at nothing; James, free, goes for a kit first.
        let out = world.aboard.room.wound(1, Part::Body, Part::Body.max());
        assert!(out.trauma.is_some(), "dying");
        let mut budget = 60 * 60 * 3;
        let mut went_to_a_cabinet = false;
        while budget > 0 && world.aboard.room.is_dying(1) {
            world.step(&[]);
            budget -= 1;
            if world.aboard.room.activity(0) == bims::game::JOB_TREAT
                && let Some(to) = world.aboard.room.destination_for_probe(0)
                && stands
                    .iter()
                    .any(|&s| (s - to).len() < 2.0 * shipdesign::TILE as f32)
            {
                went_to_a_cabinet = true;
            }
        }
        assert!(!world.aboard.room.is_dying(1), "treated within the run");
        assert!(went_to_a_cabinet, "by way of a cabinet with a kit in it");
        world.step(&[]);
        assert_eq!(
            world.ship.design.carrying(ResourceId::Medkit),
            kits - 1,
            "and the kit came off the hold"
        );
        assert_eq!(world.aboard.room.medkits(), kits - 1);
    }
}

/// The garrison an enemy station arms is the crew's number and how much
/// richer they have got: `ENEMIES_BASE` and one a crewmate, doubled for
/// every half of the starting worth the worth has grown by, and never
/// past `ENEMIES_MAX`. Whole euros in, so the steps are exact.
/// A hostile dock's room is opened with the garrison, not the residents:
/// turning the spawn hostile with its two residents' room already open
/// (what `combat` does) reopens it at `enemies_of`, and peace again
/// puts the residents back. The ship's own crew and the checksum do not
/// notice either way — the residents' room is outside it.
#[test]
fn enemies_of_grows_with_the_crew_s_worth_and_a_hostile_dock_opens_with_a_garrison() {
    // --- enemies_of_grows_with_the_crew_s_worth_and_caps ---
    {
        use crate::station::enemies_of;
        let (base, max) = (data::ENEMIES_BASE, data::ENEMIES_MAX);
        // The baseline: one more an enemy for one more a crewmate.
        assert_eq!(enemies_of(1, 1_000, 1_000, 0), base + 1);
        assert_eq!(enemies_of(2, 1_000, 1_000, 0), base + 2);
        // Poorer than at the start is still the baseline, never fewer.
        assert_eq!(enemies_of(2, 10, 1_000, 0), base + 2);
        // Half the starting worth on top doubles it; a euro short does not.
        assert_eq!(enemies_of(2, 1_499, 1_000, 0), base + 2);
        assert_eq!(enemies_of(2, 1_500, 1_000, 0), (base + 2) * 2);
        // Every further half doubles again.
        assert_eq!(enemies_of(2, 2_000, 1_000, 0), (base + 2) * 4);
        assert_eq!(enemies_of(1, 2_499, 1_000, 0), (base + 1) * 4);
        // And it stops at the cap however rich the crew.
        assert_eq!(enemies_of(2, 10_000, 1_000, 0), max);
        assert_eq!(enemies_of(2, Money::MAX, 1_000, 0), max);
        assert_eq!(enemies_of(max + 5, 1_000, 1_000, 0), max);
        // A ship worth nothing at the start has no half to grow by.
        assert_eq!(enemies_of(2, 1_000_000, 0, 0), base + 2);
        assert_eq!(enemies_of(2, 1_000_000, 1, 0), base + 2);
        // The calendar: nothing more for the first month, one more an
        // enemy every thirty days from then on — a day short is not a
        // month — under the worth's doublings and the cap alike.
        let month = data::ENEMIES_DAYS;
        assert_eq!(month, 30);
        assert_eq!(enemies_of(2, 1_000, 1_000, month - 1), base + 2);
        assert_eq!(enemies_of(2, 1_000, 1_000, month), base + 3);
        assert_eq!(enemies_of(2, 1_000, 1_000, 2 * month - 1), base + 3);
        assert_eq!(enemies_of(2, 1_000, 1_000, 2 * month), base + 4);
        assert_eq!(enemies_of(2, 1_500, 1_000, month), (base + 3) * 2);
        assert_eq!(enemies_of(1, 2_000, 1_000, month), (base + 2) * 4);
        assert_eq!(
            enemies_of(2, 1_000_000, 0, 3 * month),
            base + 5,
            "no worth to grow by, and a season gone"
        );
        assert_eq!(enemies_of(2, 1_000, 1_000, 100 * month), max);
        assert_eq!(enemies_of(2, 1_000, 1_000, u32::MAX), max);
        assert_eq!(crate::station::base_by_day(0), base);
        assert_eq!(crate::station::base_by_day(month), base + 1);
        assert_eq!(
            crate::station::base_by_day(u32::MAX),
            base + u32::MAX / month
        );
    }

    // --- the_days_gone_are_the_world_s_clock_in_whole_days ---
    {
        let mut world = basic();
        assert_eq!(world.days_gone(), 0);
        world.clock_minutes = time::minutes(1.0) - data::STEP_MINUTES;
        assert_eq!(world.days_gone(), 0, "a step short of a day");
        world.clock_minutes = time::minutes(1.0);
        assert_eq!(world.days_gone(), 1);
        world.clock_minutes = time::minutes(30.0) - 1.0;
        assert_eq!(world.days_gone(), 29);
        world.clock_minutes = time::minutes(30.0);
        assert_eq!(world.days_gone(), 30);
        world.clock_minutes = time::minutes(75.5);
        assert_eq!(world.days_gone(), 75);
    }

    // --- a_hostile_dock_opens_with_a_garrison_not_its_residents ---
    {
        use crate::station::enemies_of;
        let mut world = basic();
        let station_id = world.residents.as_ref().unwrap().station;
        let station = world.station(station_id).unwrap().clone();
        let residents = station.residents();
        // Peace: the residents, and whoever is for hire beside them — the
        // reference spawn rolls one. Hostile, the mercenary goes with the
        // residents: an enemy hires nobody out.
        let at_peace = residents + world.mercenaries_of(&station);
        assert_eq!(world.residents.as_ref().unwrap().aboard.count(), at_peace);
        assert_eq!(world.people_of(&station), residents, "home: its residents");
        assert_eq!(world.start_worth, Budget::spent(&world.ship.design));
        assert_eq!(world.worth(), world.start_worth, "nothing bought yet");

        world.set_hostile(station_id, true);
        let garrison = enemies_of(
            world.aboard.crew_count(),
            world.worth(),
            world.start_worth,
            0,
        );
        assert_eq!(garrison, data::ENEMIES_BASE + 2, "two crew: the baseline");
        assert_ne!(garrison, residents);
        assert_eq!(world.people_of(&station), garrison);
        let ashore = world.residents.as_ref().unwrap();
        assert_eq!(ashore.station, station_id);
        assert_eq!(ashore.aboard.count(), garrison, "reopened at the garrison");
        // Every one of them in the station's coverall and armed.
        for who in 0..garrison as usize {
            assert!(ashore.aboard.room.gear(who).weapon.is_some());
        }
        assert_eq!(world.aboard.crew_count(), 2, "the crew are untouched");
        // Stepping with the crowd in the room is fine.
        for _ in 0..10 {
            world.step(&[]);
        }
        assert_eq!(world.residents.as_ref().unwrap().aboard.count(), garrison);

        world.set_hostile(station_id, false);
        assert_eq!(world.people_of(&station), residents);
        assert_eq!(
            world.residents.as_ref().unwrap().aboard.count(),
            at_peace,
            "peace: the residents again"
        );

        // --- a_month_in_the_garrison_is_one_bigger ---
        // Thirty days on the world's clock, nothing bought: the same dock
        // turned hostile again arms one more, and a day short of it does
        // not. The people are asked as the room opens, so the count is
        // the clock's at that moment.
        world.clock_minutes = time::minutes(30.0) - 1.0;
        world.set_hostile(station_id, true);
        assert_eq!(
            world.people_of(&station),
            garrison,
            "a day short: no bigger"
        );
        world.set_hostile(station_id, false);
        world.clock_minutes = time::minutes(30.0);
        world.set_hostile(station_id, true);
        let older = enemies_of(2, world.worth(), world.start_worth, 30);
        assert_eq!(older, garrison + 1, "a month gone: one more");
        assert_eq!(world.people_of(&station), older);
        assert_eq!(
            world.residents.as_ref().unwrap().aboard.count(),
            older,
            "reopened at the bigger garrison"
        );
        world.clock_minutes = time::minutes(90.0);
        world.set_hostile(station_id, false);
        world.set_hostile(station_id, true);
        assert_eq!(
            world.people_of(&station),
            garrison + 3,
            "a season: three more"
        );
    }
}

/// A mercenary for hire at the dock: an extra body in the station's room
/// in the olive coverall, priced by its kit; hired from within reach with
/// the money in hand and a bunk aboard, it walks out of the station's
/// room into the crew's, the first month paid and the next due a month
/// on; the month is paid when it comes round, and one the money will not
/// cover has the hand walk off at the berth — back into the station's
/// room, for hire again. Out of reach, or broke, the hire is refused.
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
    assert!(world.aboard.room.bed_count() >= 2, "a bunk to spare");
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
    assert!(offer.docked && offer.bunk && offer.affordable && !offer.in_reach);
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
    let money = world.money;
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

    // A month on, paid.
    world.clock_minutes += MONTH;
    let money = world.money;
    let events = world.step(&[]);
    assert!(
        events.contains(&WorldEvent::MercenaryPaid { who: 1, fee }),
        "{events:?}"
    );
    assert_eq!(world.money, money - fee);
    assert_eq!(
        world.hired()[0].due,
        world.hired()[0].due.max(world.clock_minutes)
    );

    // Broke a month later: owed, said once, and off at the berth.
    world.money = fee - 1;
    world.clock_minutes += MONTH;
    let events = world.step(&[]);
    assert!(
        events.contains(&WorldEvent::MercenaryLeft { who: 1 }),
        "{events:?}"
    );
    assert_eq!(world.aboard.crew_count(), 1, "walked off");
    assert_eq!(world.ship.crew_count, 1);
    assert!(world.hired().is_empty());
    assert!(
        !world
            .pieces
            .iter()
            .any(|p| matches!(p.at, Where::Worn { .. }))
    );
    let ashore = world.residents.as_ref().unwrap();
    assert_eq!(ashore.aboard.count(), count, "back in the station's room");
    assert_eq!(
        ashore.fee.last().copied().flatten(),
        Some(fee),
        "for hire again"
    );
    assert_eq!(world.mercenary_fee(count - 1), Some(fee));
    let events = world.step(&[]);
    assert!(
        !events
            .iter()
            .any(|e| matches!(e, WorldEvent::MercenaryLeft { .. }))
    );

    // With nothing to spend, a hire is refused; with no bunk, likewise.
    world.aboard.room.put_for_probe(
        0,
        world
            .body_position(LootSource::Resident(count - 1))
            .unwrap()
            + bims::math::vec2(30.0, 0.0),
    );
    let hire = Command::Hire {
        slot: 0,
        who: 0,
        resident: count - 1,
    };
    let events = world.step(&[hire]);
    assert!(events.contains(&WorldEvent::Refused {
        slot: 0,
        why: Refusal::Unaffordable
    }));
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
}

/// The `combat` command's dock: the combat ship with its fourteen crew —
/// five at their bunks and nine on the deck, each of them on a tile of
/// its own — every one with a gun, the five kinds dealt down the crew and
/// round again, tied up at the spawn rebuilt as the arena — bigger than
/// any kind of station, with a bunk for every one of a garrison — and,
/// once hostile, a garrison of `ARENA_GARRISON` whatever the crew's worth
/// would have put up: fifteen for fourteen, every one of them in the
/// room. The reinforcements that make the number up are in the checksum,
/// since they are the size of the fight; and the world steps with the
/// crowd in it.
/// The arena and the combat ship can be walked like any station: from the
/// deck inside the port to every use spot of every part, and the arena
/// has no deck nobody can get to. Three seeds for every kind, since the
/// seed dresses the arena as it dresses a station.
#[test]
fn the_arena_is_the_combat_dock_and_it_and_the_combat_ship_can_be_walked() {
    // --- the_combat_dock_is_the_arena_with_fourteen_crew_and_a_garrison_of_fifteen ---
    {
        use crate::checksum::world_checksum;
        use crate::station::enemies_of;
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
        let small = world.station(station).unwrap().design.build_area;

        assert!(world.arena_dock_for_probe());
        let arena = world.station(station).unwrap().clone();
        assert_eq!(arena.design.build_area, data::ARENA_SIDE);
        assert!(arena.design.build_area > small, "bigger than it was");
        assert!(
            arena.design.count(PartKind::Bunk) >= data::ENEMIES_MAX,
            "a bunk for every one of a garrison: {}",
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
        world.set_hostile(station, true);
        let garrison =
            enemies_of(COMBAT_CREW, world.worth(), world.start_worth, 0) + world.reinforcements;
        assert_eq!(garrison, data::ARENA_GARRISON);
        assert_eq!(garrison, 15);
        assert!(garrison <= data::ENEMIES_MAX);
        assert_eq!(world.people_of(&arena), garrison);
        let ashore = world.residents.as_ref().unwrap();
        assert_eq!(ashore.aboard.count(), garrison, "fifteen in the room");
        for who in 0..garrison as usize {
            assert!(ashore.aboard.room.gear(who).weapon.is_some());
        }
        for (who, &kind) in kinds.iter().enumerate() {
            assert_eq!(world.aboard.room.weapon(who), Some(kind.basic()));
        }
        for _ in 0..10 {
            world.step(&[]);
        }
        assert_eq!(world.residents.as_ref().unwrap().aboard.count(), garrison);
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

/// Feature 87: a dressing is a thing in a pack and the Management tab
/// says how many each of the crew is to carry. Out of combat they fill
/// themselves up out of the hold, one a step, whoever they are — a
/// player's own and the bots alike — and a box stowed back into the
/// lockers puts the whole five in at once.
#[test]
fn the_crew_fill_their_packs_with_dressings_out_of_the_hold() {
    use bims::manager::Stock;
    use shipdesign::fixture::playtest_ship;
    let mut world = crate::fixture::crewed_world(playtest_ship(), data::SIMULATION_MONEY, 1, 2);
    let aboard = world.ship.design.carrying(ResourceId::Bandage);
    assert!(aboard >= 5, "the playtest ship carries a few: {aboard}");
    // Nobody carrying and nobody asking, then the hold's own back: this
    // test is about the filling, so it starts from empty packs.
    without_dressings(&mut world);
    world.ship.design.cargo[ResourceId::Bandage as usize] = aboard;
    world.on_ship_changed();
    // Two apiece: the hold pays for all four, one a step.
    world.aboard.room.set_target(Stock::Bandages, 2);
    for _ in 0..8 {
        world.step(&[]);
    }
    assert_eq!(world.aboard.room.bandages_of(0), 2, "the player's own");
    assert_eq!(world.aboard.room.bandages_of(1), 2, "and the bot");
    assert_eq!(
        world.ship.design.carrying(ResourceId::Bandage),
        aboard - 4,
        "out of the hold"
    );
    lockers_agree(&world);
    // Asked for more than there is: the hold empties and nobody is given
    // what is not there.
    world.aboard.room.set_target(Stock::Bandages, 9);
    for _ in 0..40 {
        world.step(&[]);
    }
    assert_eq!(world.ship.design.carrying(ResourceId::Bandage), 0);
    assert_eq!(
        world.aboard.room.bandages_of(0) + world.aboard.room.bandages_of(1),
        aboard,
        "every dressing aboard is on somebody"
    );
    lockers_agree(&world);
    // A box stowed back is five at once, not one — and the restock does
    // not take it straight out again while the target is unmet, so the
    // target comes down first.
    world.aboard.room.set_target(Stock::Bandages, 0);
    let carried = world.aboard.room.bandages_of(0);
    let cell = world
        .aboard
        .room
        .gear(0)
        .stack_to_spend(bims::game::BANDAGE)
        .expect("a box on it");
    let units = world.aboard.room.gear(0).units(cell);
    at_the_armoury(&mut world, 0);
    world.step(&[Command::Stow {
        slot: 0,
        who: 0,
        cell: cell as u32,
    }]);
    assert_eq!(world.ship.design.carrying(ResourceId::Bandage), units);
    assert_eq!(world.aboard.room.bandages_of(0), carried - units);
    lockers_agree(&world);
}

/// Stand a crew member at the armoury this instant, for the reach check.
/// Before every command that wants it: the Bim goes off about its errands
/// between steps, and a command lands before the room moves.
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
            assert!(!worldgen::StationKind::Orbital.sells(resource));
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
        world.know_everything_for_probe();
        assert!(world.ship.design.carrying(ResourceId::Metal) >= 2);
        assert!(world.powered(PartKind::Workbench));

        // A second helm off the bench: recipe 7, half an hour at the
        // workbench plus the walk there.
        world.set_craft_target(ResourceId::Helm, 2);
        let orders = world.craft_orders();
        assert_eq!(orders.len(), 1, "{orders:?}");
        assert_eq!(orders[0].recipe, 7);
        let mut made = false;
        for _ in 0..(4 * 60 * 60) {
            let events = world.step(&[]);
            if events.contains(&WorldEvent::Crafted { recipe: 7 }) {
                made = true;
                break;
            }
        }
        assert!(made, "the helm was made");
        assert_eq!(world.ship.design.carrying(ResourceId::Helm), 2);
        let helms = in_hold(&world, ArmourKind::BasicHelm);
        assert_eq!(helms.len(), 2);
        assert_eq!(helms[1].id, 4, "the next id");
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
        world.stand_down(0);
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
    assert!(!world.container_takes(Container::Bench(bench), ResourceId::Metal));
    assert!(world.container_takes(Container::Shelf(0), ResourceId::Metal));
    assert!(world.container_takes(Container::Shelf(0), ResourceId::Helm));
    assert!(world.container_takes(Container::Shelf(0), ResourceId::Handgun));
    assert!(!world.container_takes(Container::Shelf(0), ResourceId::Bandage));
    assert!(!world.container_takes(Container::Shelf(0), ResourceId::Tofu));
    assert!(world.container_takes(Container::Fridge(0), ResourceId::Tofu));
    assert!(!world.container_takes(Container::Fridge(0), ResourceId::Helm));
    // The smelter is a bench that holds nothing.
    let smelter = (0..world.aboard.room.benches().len())
        .find(|&i| world.aboard.room.bench_part(i) == PartKind::Smelter.code())
        .unwrap();
    assert!(!world.container_takes(Container::Bench(smelter), ResourceId::Helm));

    // At the helm, forward of everything that holds anything — the bunk
    // is right beside the armoury — refused, and nothing moved.
    let helm = world.helm_spot().unwrap().add(world.aboard.offset);
    let helm = bims::math::vec2(helm.x as f32, helm.y as f32);
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

    // A stow from the helm is out of reach; at the armoury the box of
    // dressings goes back — **all five of it** (feature 87) — and once
    // the lockers are full the next thing is refused.
    let events = world.step(&[Command::Stow {
        slot: 0,
        who: 0,
        cell: boxes[0] as u32,
    }]);
    assert!(events.contains(&WorldEvent::Refused {
        slot: 0,
        why: Refusal::OutOfReach
    }));
    at_the_armoury(&mut world, 0);
    let events = world.step(&[Command::Stow {
        slot: 0,
        who: 0,
        cell: boxes[0] as u32,
    }]);
    assert!(events.contains(&WorldEvent::Stowed { who: 0 }));
    assert_eq!(world.ship.design.carrying(ResourceId::Bandage), 5);
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

/// At a hostile station the fight ends with a body in the corridor, and
/// James walks over and goes through its pockets: the weapon out of its
/// hand into his pack, and the helm it was issued off its head as a piece
/// of the world's — a fresh id, since the station's room numbered it —
/// with the health the fight left it.
/// A hostile station's people know only what they have seen, and they
/// chase it: a resident that saw James at the station's door follows him
/// through the passage and onto the ship — its own room holds the ship
/// as well, turned into the station's frame (`docking::join_mirror`,
/// `Residents::join`). Then the ship leaves with the resident still
/// aboard, and it is stood at its bunk in a room of the station alone.
#[test]
fn an_enemy_follows_the_crew_member_it_saw_onto_the_ship_and_is_put_ashore_when_it_leaves() {
    use bims::combat::{Gear, WeaponKind};
    use shipdesign::parts::TILE;
    let mut world = basic();
    assert!(world.stage_fight_for_probe());
    let station = world.ship.state.station().unwrap();
    // The residents' room holds the ship: its deck is bigger than the
    // station's own, and shifted.
    let ashore = world.residents.as_ref().unwrap();
    assert!(
        ashore.aboard.station_frame.is_some(),
        "the ship is on their deck"
    );
    let shift = ashore.aboard.offset;
    let side = world.station(station).unwrap().design.build_area as f64 * TILE as f64;
    assert!(
        ashore.aboard.design.build_area as f64 * TILE as f64 > side,
        "bigger than the station alone"
    );
    // A blade on the resident so it charges and a gun on nobody, so the
    // chase is what this measures; James patched every step against the
    // blade.
    {
        let ashore = world.residents.as_mut().unwrap();
        ashore.aboard.room.issue(
            0,
            Gear {
                weapon: Some(WeaponKind::Schword.basic()),
                ..Gear::default()
            },
        );
        one_on_one(&mut ashore.aboard.room, 0);
    }
    world.aboard.room.issue(0, Gear::default());
    world.aboard.room.issue(1, Gear::default());
    // Whether the resident stands on the ship's own deck: its spot on the
    // joined deck, less the ship's shift, against the ship's grid.
    let ship = world.ship.design.clone();
    let on_ship = |world: &World| {
        let at = on_deck(world, 0);
        let p = dvec2(at.x as f64, at.y as f64).sub(world.aboard.offset);
        let t = TILE as f64;
        let tile = ((p.x / t).floor() as i32, (p.y / t).floor() as i32);
        ship.grid().get(shipdesign::Layer::Structure, tile) != 0
    };
    assert!(!on_ship(&world));

    // Seen at the door: believed where it stands, and the room at war.
    world.step(&[]);
    let believed = world
        .residents
        .as_ref()
        .unwrap()
        .aboard
        .room
        .believed_for_probe();
    assert!(believed[0].is_some(), "James in sight is known");

    // James walks home through the passage; the resident comes after him
    // and is on the ship within the run.
    let gangway = world.aboard.gangway.unwrap();
    let gangway = bims::math::vec2(gangway.x as f32, gangway.y as f32);
    assert!(world.aboard.room.walk_to(0, gangway));
    let mut followed = false;
    for _ in 0..1_500 {
        world.aboard.room.patch_up_for_probe(0);
        world.step(&[]);
        if world.aboard.on_ship(0, &world.ship.design) && on_ship(&world) {
            followed = true;
            break;
        }
    }
    assert!(followed, "the resident followed James aboard");
    assert!(
        world
            .residents
            .as_ref()
            .unwrap()
            .aboard
            .room
            .is_recruited(0),
        "still under arms"
    );

    // (What it believes once James is out of sight, and for how long, is
    // the room's own rule: `a_hostile_room_chases_what_it_last_saw_and_forgets_it_after_a_minute`
    // in `bims::game::tests`.)

    // The ship casts off with the resident still on its deck: the room of
    // the station alone has no floor there, so it is stood at its bunk.
    let was_aboard = on_ship(&world);
    world.undock_for_probe();
    assert!(!world.aboard.is_joined(), "cast off");
    let ashore = world.residents.as_ref().unwrap();
    assert!(
        ashore.aboard.station_frame.is_none(),
        "the station alone again"
    );
    assert_eq!(ashore.aboard.offset, DVec2::ZERO);
    let p = ashore.aboard.position(0);
    assert!(
        p.x >= 0.0 && p.y >= 0.0 && p.x <= side && p.y <= side,
        "on the station: {p:?}"
    );
    let _ = (was_aboard, shift);
}

/// What a raid rests on: the crew retreat aboard from a hostile dock and
/// lock the ship's airlock behind them, and the station's people come
/// after them — to the ship's door, which is locked against them and
/// which they force (`Game::breach`, `SMASH_AIRLOCK`), and through it
/// onto the ship's own deck, where they stand and fight. Every one of
/// those is the room's own rule; this pins that the world carries them
/// across the two rooms and the mated airlocks, since a raider docked to
/// the ship is exactly a hostile station docked to it. A blade charges;
/// a gunner with nobody in sight hunts to where it last saw its quarry
/// (`Game::plan_stand`, September 2026) — before that it took a stand
/// with a view of the door at the far end of its range and waited, and
/// nine boarders in ten would never have boarded.
#[test]
fn enemies_follow_a_crew_that_retreats_aboard_and_force_the_ship_s_locked_airlock() {
    use bims::combat::WeaponKind;
    for weapon in [WeaponKind::Schword, WeaponKind::LaserPistol] {
        retreat_aboard_and_lock_the_door(weapon);
    }
}

fn retreat_aboard_and_lock_the_door(weapon: bims::combat::WeaponKind) {
    use bims::combat::Gear;
    use bims::door::Order;
    use shipdesign::parts::TILE;
    let mut world = basic();
    assert!(world.stage_fight_for_probe());
    let t = TILE as f64;
    // One enemy, with the weapon under test; James and the other crew
    // member unarmed, so the chase is what this measures.
    {
        let ashore = world.residents.as_mut().unwrap();
        ashore.aboard.room.issue(
            0,
            Gear {
                weapon: Some(weapon.basic()),
                ..Gear::default()
            },
        );
        one_on_one(&mut ashore.aboard.room, 0);
    }
    world.aboard.room.issue(0, Gear::default());
    world.aboard.room.issue(1, Gear::default());
    let ship = world.ship.design.clone();
    let on_ship = |world: &World| {
        let at = on_deck(world, 0);
        let p = dvec2(at.x as f64, at.y as f64).sub(world.aboard.offset);
        let tile = ((p.x / t).floor() as i32, (p.y / t).floor() as i32);
        ship.grid().get(shipdesign::Layer::Structure, tile) != 0
    };
    // The ship's airlock on the joined deck: its opening, through the
    // ship's shift.
    let port = shipdesign::dock::port(&ship).expect("the flyer has a port");
    let (px, py) = port.centre;
    let airlock = world
        .aboard
        .room
        .door_index_at(bims::math::vec2(
            (px + world.aboard.offset.x) as f32,
            (py + world.aboard.offset.y) as f32,
        ))
        .expect("the ship's airlock is a door of the joined room");
    assert!(!world.aboard.room.ship_door_is_locked(airlock));

    // Seen at the door, then James runs for the ship: well inside, the
    // length of the deck from the airlock.
    world.step(&[]);
    let inside = world.aboard.offset.add(dvec2(4.5 * t, 12.5 * t));
    assert!(
        world
            .aboard
            .room
            .walk_to(0, bims::math::vec2(inside.x as f32, inside.y as f32))
    );
    let mut locked_at = None;
    for step in 0..1_200 {
        world.aboard.room.patch_up_for_probe(0);
        world.step(&[]);
        // Home — a deck tile inside the skin, not the passage — and seen
        // there by the enemy at the door, the door is locked in its face
        // from the panel, which is the crew's to do from anywhere. (Locked
        // before it saw him inside, it would believe him where it last saw
        // him, in the passage, and stand off with a shot at the door rather
        // than force it: the room's rule, `Game::breach`.)
        let p = world.aboard.position(0);
        let past_the_skin = (p.x / t).floor() < 17.0;
        let seen_inside = {
            let r = world.residents.as_ref().unwrap();
            let (o, ex, ey) = r.aboard.station_frame.unwrap();
            let jr = o.add(ex.scale(p.x)).add(ey.scale(p.y));
            r.aboard
                .room
                .sees_for_probe(0, bims::math::vec2(jr.x as f32, jr.y as f32))
        };
        if locked_at.is_none() && past_the_skin && seen_inside {
            world.aboard.room.order_door_now(airlock, Order::Lock);
            locked_at = Some(step);
        }
        if locked_at.is_some_and(|at| step > at + 2) {
            break;
        }
    }
    assert!(locked_at.is_some(), "James got aboard");
    assert!(world.aboard.room.ship_door_is_locked(airlock), "locked");
    assert!(!on_ship(&world), "the enemy is still ashore");

    // The enemy comes to the locked door and heaves at it: the smashing
    // in its own room is a bar on the deck's door, and then the lock
    // gives — nobody's order on this side — and it is through, on the
    // ship's deck, still at war.
    let mut smashing = false;
    let mut forced = false;
    let mut aboard = false;
    for _ in 0..3_600 {
        world.aboard.room.patch_up_for_probe(0);
        world.step(&[]);
        let state = &world.aboard.room.door_states()[airlock];
        if state.smash.is_some() {
            smashing = true;
        }
        if smashing && !state.locked {
            forced = true;
        }
        if forced && on_ship(&world) {
            aboard = true;
            break;
        }
    }
    assert!(smashing, "the enemy heaved at the ship's airlock");
    assert!(forced, "and the lock gave");
    assert!(aboard, "and it came onto the ship's deck");
    assert!(
        world
            .residents
            .as_ref()
            .unwrap()
            .aboard
            .room
            .is_recruited(0),
        "still under arms"
    );
    // And it stays there — on James, round his own deck — rather than
    // wandering off, while it has him to fight.
    let mut held = 0;
    for _ in 0..600 {
        world.aboard.room.patch_up_for_probe(0);
        world.step(&[]);
        if on_ship(&world) {
            held += 1;
        }
    }
    assert!(
        held > 300,
        "it stayed on the ship's deck: {held} of 600 steps"
    );
}

/// A hostile station's people watch their own airlock: a crew member
/// walking in through it is seen at the door with nobody looking —
/// every one of them dead here, so no eye but the airlock's — where one
/// still aboard the ship is nobody to them. `Residents::join` puts the
/// watch on the tile just inside the station's door
/// (`Game::set_watched`); the room's own rule for the watch is
/// `a_hostile_room_s_airlock_is_watched…` in `bims::game::tests`.
#[test]
fn a_hostile_station_notices_a_boarding_at_its_airlock_with_nobody_looking() {
    let mut world = basic();
    let station = world.ship.state.station().unwrap();
    world.set_hostile(station, true);
    {
        let ashore = world.residents.as_mut().unwrap();
        for who in 0..ashore.aboard.count() as usize {
            ashore.aboard.room.kill_for_probe(who);
        }
    }
    world.step(&[]);
    let believed = |world: &World| {
        world
            .residents
            .as_ref()
            .unwrap()
            .aboard
            .room
            .believed_for_probe()
    };
    assert_eq!(
        believed(&world),
        vec![None, None],
        "the crew aboard their own ship are nobody to the station"
    );
    let who = send_a_crew_member_ashore(&mut world);
    let mut seen = false;
    for _ in 0..LEAVING {
        world.step(&[]);
        if believed(&world)[who as usize].is_some() {
            seen = true;
            break;
        }
    }
    assert!(seen, "seen at the door, with nobody's eyes");
    assert_eq!(believed(&world)[0], None, "the one still aboard is not");
}

/// **Half its blood is where a crew member goes out, and a body that far
/// gone is nobody's target** (feature 89). A crew member ashore at a
/// hostile station is believed where it stands; bled to just over half
/// it is still on its feet and still believed; bled a drop under half it
/// is out cold, and the station's people forget it the same step — the
/// slot goes `None`, which is what `aim`, `melee_with` and the tactics
/// all read. The band above it — half pace under three quarters — is
/// pinned in `bims::health`'s own tests.
#[test]
fn a_crew_member_under_half_its_blood_is_out_cold_and_nobody_s_target() {
    use bims::health::{MAX_BLOOD, OUT_AT};
    let mut world = basic();
    let station = world.ship.state.station().unwrap();
    world.set_hostile(station, true);
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
    assert!(seen, "the station's people have it in their sight");

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

    // And a drop under it: out cold, and gone from the list the station's
    // people shoot at.
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

/// A hostile station's person lying out cold is finished off on the
/// player's word — `Command::Execute`: James walks to within a few tiles
/// with a clear line and shoots it where it lies, and it is dead in its
/// own room, `Executed` said. Refused for one on its feet (`NotDown`),
/// with nothing in hand (`Unarmed`), and at a station that is not an
/// enemy's (`NotHostile`).
#[test]
fn a_downed_enemy_is_finished_off_where_it_lies_and_the_refusals_are_said() {
    use bims::combat::Gear;
    let mut world = basic();
    assert!(world.stage_fight_for_probe());
    let station_id = world.residents.as_ref().unwrap().station;
    let order = |slot| Command::Execute {
        slot,
        who: 0,
        resident: 0,
    };
    // At peace: not an enemy's, whatever state the body is in. Asked
    // first, since a change of stance opens the station's room afresh —
    // its mercenaries come and go with it — and everything below is
    // about one resident's body.
    world.set_hostile(station_id, false);
    let events = world.step(&[order(3)]);
    assert!(refused_with(&events, Refusal::NotHostile), "{events:?}");
    world.set_hostile(station_id, true);
    assert_eq!(Refusal::NotHostile.code(), 26);
    assert_eq!(Refusal::Unarmed.code(), 27);
    {
        let ashore = world.residents.as_mut().unwrap();
        one_on_one(&mut ashore.aboard.room, 0);
        // The resident unarmed too, so nobody shoots anybody meanwhile.
        ashore.aboard.room.issue(0, Gear::default());
    }
    let pistol = world.aboard.room.gear(0);
    world.aboard.room.issue(0, Gear::default());
    world.aboard.room.issue(1, Gear::default());

    // On its feet: nobody to finish.
    let events = world.step(&[order(1)]);
    assert!(
        refused_with(&events, Refusal::NotDown),
        "on its feet: {events:?}"
    );

    // Out cold in its own room: the blood put under the line with nothing
    // open, so it lies there rather than bleeding out under the test.
    world
        .residents
        .as_mut()
        .unwrap()
        .aboard
        .room
        .knock_out_for_probe(0);
    let mut budget = 60 * 60 * 2;
    while budget > 0
        && !world
            .residents
            .as_ref()
            .unwrap()
            .aboard
            .room
            .is_unconscious(0)
    {
        world.step(&[]);
        budget -= 1;
    }
    let source = crate::LootSource::Resident(0);
    assert!(world.is_down(source), "out cold");
    assert!(
        world.residents.as_ref().unwrap().aboard.room.is_alive(0),
        "and alive"
    );

    // Unarmed: nothing to do it with.
    let events = world.step(&[order(2)]);
    assert!(refused_with(&events, Refusal::Unarmed), "{events:?}");
    world.aboard.room.issue(0, pistol);

    // The order: James walks over, shoots it where it lies, and it is
    // dead in its own room; the world says so.
    let events = world.step(&[order(4)]);
    assert!(
        !events
            .iter()
            .any(|e| matches!(e, WorldEvent::Refused { slot: 4, .. })),
        "{events:?}"
    );
    assert_eq!(world.aboard.room.activity(0), bims::game::JOB_EXECUTE);
    let mut executed = false;
    let mut budget = 60 * 60;
    while budget > 0 && !executed {
        let events = world.step(&[]);
        executed = events.contains(&WorldEvent::Executed {
            who: 0,
            resident: 0,
        });
        budget -= 1;
    }
    assert!(executed, "finished within the minute");
    // The body follows at the top of its next tick, as any death does.
    world.step(&[]);
    assert!(
        !world.residents.as_ref().unwrap().aboard.room.is_alive(0),
        "dead in its own room"
    );
    assert_eq!(
        WorldEvent::Executed {
            who: 0,
            resident: 0
        }
        .code(),
        48
    );
    // Dead is not down to finish: refused again.
    let events = world.step(&[order(5)]);
    assert!(refused_with(&events, Refusal::NotDown), "{events:?}");
}

#[test]
fn a_resident_down_in_the_fight_is_looted_of_its_weapon_and_its_helm() {
    use bims::combat::{ArmourKind, Item, LootCell, Piece};
    use bims::health::Part;
    let mut world = with_a_crewmate();
    assert!(world.stage_fight_for_probe());
    let station_id = world.residents.as_ref().unwrap().station;
    let source = crate::LootSource::Resident(0);

    // Resident 0 wears a helm the station numbered as it liked; the rest
    // of the garrison is out of it, so the fight is one on one — and James
    // is patched up before every step, so it ends with the resident down
    // and not on the coin of a head shot the other way.
    let issued = {
        let ashore = world.residents.as_mut().unwrap();
        let mut gear = ashore.aboard.room.gear(0);
        gear.head = Some(Piece::new(1, ArmourKind::BasicHelm, Tier::One));
        ashore.aboard.room.issue(0, gear);
        one_on_one(&mut ashore.aboard.room, 0);
        gear
    };
    let weapon = issued.weapon.expect("armed");
    assert!(!world.is_down(source), "on its feet");
    assert!(world.body_position(source).is_some(), "on the joined deck");

    let mut down = false;
    for _ in 0..3_000 {
        world.aboard.room.patch_up_for_probe(0);
        let events = world.step(&[]);
        world.aboard.room.observe();
        if events.iter().any(
            |e| matches!(e, WorldEvent::EnemyDown { station, who: 0 } if *station == station_id),
        ) {
            down = true;
            break;
        }
    }
    assert!(down, "resident 0 went down within the run");
    // Said the step its total reached nought; the body follows at the
    // top of its next tick.
    for _ in 0..3 {
        if world.is_down(source) {
            break;
        }
        world.step(&[]);
    }
    assert!(world.is_down(source));
    // And the joined room is told: the visitor is a body under a click.
    let body = world.body_position(source).expect("a body on the deck");
    assert_eq!(
        world.aboard.room.hit_at(body.x, body.y),
        bims::room::HIT_VISITOR
    );
    assert_eq!(world.aboard.room.hit_visitor(), 0);
    let cells = world.loot_cells(source).expect("the body shows");
    assert_eq!(
        cells[LootCell::Weapon.code() as usize],
        Some(Item::Weapon(weapon))
    );
    let Some(Item::Armour(helm)) = cells[LootCell::Head.code() as usize] else {
        panic!("the helm is still on it: {cells:?}");
    };
    assert_eq!(helm.kind, ArmourKind::BasicHelm);

    // James, let go, walks over: `send_to` beside the body, as the Loot
    // row does, and the command lands once he is within reach.
    world.aboard.room.recruit_for_probe(0, false);
    assert!(world.aboard.room.send_to(0, body), "a route to the body");
    let mut budget = 60 * 60;
    while budget > 0 && !world.in_reach_of_body(0, source) {
        world.step(&[]);
        budget -= 1;
    }
    assert!(world.in_reach_of_body(0, source), "James got there");
    let next = world.next_piece;
    let pieces = world.pieces.len();
    let loot = |cell: LootCell| Command::Loot {
        slot: 0,
        who: 0,
        source,
        cell: cell.code(),
    };
    let events = world.step(&[loot(LootCell::Weapon), loot(LootCell::Head)]);
    assert_eq!(
        events
            .iter()
            .filter(|e| matches!(
                e,
                WorldEvent::Looted {
                    who: 0,
                    source_kind: 1
                }
            ))
            .count(),
        2,
        "{events:?}"
    );
    // The pistol lies along two cells, so the helm is at the third.
    let pack = world.aboard.room.pack(0);
    assert_eq!(pack[0], Some(Item::Weapon(weapon)));
    let Some(Item::Armour(taken)) = pack[2] else {
        panic!("the helm in the pack: {pack:?}");
    };
    assert_eq!(taken.id, next, "renumbered by the world");
    assert_eq!(taken.kind, ArmourKind::BasicHelm);
    assert_eq!(taken.health, helm.health, "with what the fight left it");
    assert_eq!(world.next_piece, next + 1);
    assert_eq!(world.pieces.len(), pieces + 1);
    let last = world.pieces.last().unwrap();
    assert_eq!(last.id, next);
    assert_eq!(last.at, crate::Where::Pack { who: 0, cell: 2 });
    assert_eq!(last.health, helm.health);
    pieces_agree(&world);
    // The body is bare-handed and bare-headed now, and gives nothing twice.
    let ashore = world.residents.as_ref().unwrap();
    assert_eq!(ashore.aboard.room.weapon(0), None);
    assert_eq!(ashore.aboard.room.worn(0, Part::Head), None);
    let events = world.step(&[loot(LootCell::Weapon)]);
    assert!(
        events.contains(&WorldEvent::Refused {
            slot: 0,
            why: Refusal::NotAboard
        }),
        "{events:?}"
    );
}

// --- research ----------------------------------------------------------------

/// The whole loop, on the playtest ship at its spawn: the home station has
/// a key on its research desk whatever it rolled; a crew member within
/// reach takes it into their pack, where it is two cells tall; puts it
/// into the ship's own desk, whose slot it fills; the desk consumes it to
/// open the armoury — that node and no other; and the AI, asked for it,
/// queues what it needs ahead of it and goes onto the head — a node off
/// the queue takes what needed it, and a cancel the AI off — and finishes
/// a node on the desk's power, going straight onto the next queued.
/// What research gates, on a fresh crew: the smelter is aboard the
/// playtest ship and the ore for it, and nothing is smelted until
/// smelting is known; a site for a smelter is refused the same way, with
/// the node it waits on; and the drug lab's medicine wants nothing.
#[test]
fn a_key_is_taken_ashore_and_consumed_and_the_benches_wait_on_research() {
    // --- a_key_is_taken_ashore_put_in_the_desk_and_consumed_to_open_a_node ---
    {
        use bims::combat::Item;
        use shipdesign::fixture::playtest_ship;
        use shipdesign::research::Node;
        let mut world = simulation_world(playtest_ship(), data::SIMULATION_MONEY, 1);
        // The dressings every Bim carries are out of the way (feature 87).
        without_dressings(&mut world);
        assert_eq!(world.key_at_the_dock(), 1, "the spawn always has a key");
        assert!(world.research_desk_aboard());
        assert!(world.research_desk_powered());
        assert_eq!(world.keys_in_desk(1), 0);
        // Two desks on the joined deck: the ship's first, the station's after.
        assert_eq!(world.aboard.room.research_desks().len(), 2);
        assert_eq!(world.station_desk(), Some(1));
        assert!(!world.key_in_reach(0), "James woke at his bunk");

        // From the bunk the desk is out of reach; from its spot it is not.
        let events = world.step(&[Command::TakeKey { slot: 0, who: 0 }]);
        assert!(events.contains(&WorldEvent::Refused {
            slot: 0,
            why: Refusal::OutOfReach
        }));
        let spot = world.key_desk_spot().expect("the station's desk");
        world.aboard.room.put_for_probe(0, spot);
        assert!(world.key_in_reach(0));
        let events = world.step(&[Command::TakeKey { slot: 0, who: 0 }]);
        assert!(
            events.contains(&WorldEvent::KeyTaken { who: 0 }),
            "{events:?}"
        );
        assert_eq!(world.key_at_the_dock(), 0, "the desk is bare");
        let pack = world.aboard.room.pack(0);
        let under = bims::combat::PACK_COLS;
        assert_eq!(pack[0], Some(Item::Key(1)));
        assert!(
            pack[under].is_none(),
            "the tail cell holds nothing of its own"
        );
        let gear = world.aboard.room.gear(0);
        assert!(gear.occupied(under), "but it is taken");
        assert_eq!(gear.free_cell(), Some(1));
        assert_eq!(gear.head_of(under), 0);
        // A second take finds nothing there.
        world.aboard.room.put_for_probe(0, spot);
        let events = world.step(&[Command::TakeKey { slot: 0, who: 0 }]);
        assert!(events.contains(&WorldEvent::Refused {
            slot: 0,
            why: Refusal::NoKey
        }));
        assert_eq!(WorldEvent::KeyTaken { who: 0 }.code(), 44);

        // Into the ship's own desk: out of reach from the station's, in reach
        // from its own spot, and the hold counts it in the research class.
        let events = world.step(&[Command::Stow {
            slot: 0,
            who: 0,
            cell: under as u32,
        }]);
        assert!(events.contains(&WorldEvent::Refused {
            slot: 0,
            why: Refusal::OutOfReach
        }));
        let own = world.aboard.room.research_spot(0).expect("the ship's desk");
        world.aboard.room.put_for_probe(0, own);
        assert!(world.in_reach(0, ResourceId::ResearchKey));
        // Stowed by its tail cell, which is the same key.
        let events = world.step(&[Command::Stow {
            slot: 0,
            who: 0,
            cell: under as u32,
        }]);
        assert!(
            events.contains(&WorldEvent::Stowed { who: 0 }),
            "{events:?}"
        );
        assert!(world.aboard.room.pack(0)[0].is_none());
        assert_eq!(world.keys_in_desk(1), 1);
        assert_eq!(world.ship.design.stored(Storage::Research), 1);
        assert_eq!(world.ship.design.capacity(Storage::Research), 1);

        // Back out and in again: a fetch puts it in the first two free cells.
        world.aboard.room.put_for_probe(0, own);
        world.step(&[Command::Fetch {
            slot: 0,
            who: 0,
            kind: crate::FetchKind::Resource(ResourceId::ResearchKey as u32),
        }]);
        assert_eq!(world.aboard.room.pack(0)[0], Some(Item::Key(1)));
        assert_eq!(world.keys_in_desk(1), 0);
        // An unlock with the desk empty is refused, and spends nothing.
        let events = world.step(&[Command::Unlock {
            slot: 0,
            node: Node::Armoury.code(),
        }]);
        assert!(events.contains(&WorldEvent::Refused {
            slot: 0,
            why: Refusal::NoKey
        }));
        world.aboard.room.put_for_probe(0, own);
        world.step(&[Command::Stow {
            slot: 0,
            who: 0,
            cell: 0,
        }]);
        assert_eq!(world.keys_in_desk(1), 1);

        // The key consumed: the armoury open and the emitters still shut, the
        // desk empty, and a second key on the armoury — or one on a node
        // with no lock — refused rather than spent.
        assert!(!world.research.is_unlocked(Node::Armoury));
        let events = world.step(&[Command::Unlock {
            slot: 0,
            node: Node::Armoury.code(),
        }]);
        assert!(
            events.contains(&WorldEvent::Unlocked {
                node: Node::Armoury.code()
            }),
            "{events:?}"
        );
        assert!(world.research.is_unlocked(Node::Armoury));
        assert!(!world.research.is_unlocked(Node::Emitters));
        assert!(world.research.needs_key(Node::Emitters));
        assert_eq!(world.keys_in_desk(1), 0);
        world.ship.design.cargo[ResourceId::ResearchKey as usize] = 1;
        world.on_ship_changed();
        for node in [Node::Armoury, Node::Smelting] {
            let events = world.step(&[Command::Unlock {
                slot: 0,
                node: node.code(),
            }]);
            assert!(events.contains(&WorldEvent::Refused {
                slot: 0,
                why: Refusal::NotResearchable
            }));
        }
        assert_eq!(world.keys_in_desk(1), 1);

        // The armoury wants the workshop first, and the workshop smelting:
        // asked for, the whole chain goes onto the queue ahead of it, and
        // the AI goes onto smelting the same step.
        let events = world.step(&[Command::Research {
            slot: 0,
            node: Node::Armoury.code(),
        }]);
        for node in [Node::Smelting, Node::Workshop, Node::Armoury] {
            assert!(
                events.contains(&WorldEvent::ResearchQueued { node: node.code() }),
                "{events:?}"
            );
        }
        assert!(events.contains(&WorldEvent::ResearchBegun {
            node: Node::Smelting.code()
        }));
        assert_eq!(world.research.current, Some(Node::Smelting));
        assert_eq!(world.research.queue, vec![Node::Workshop, Node::Armoury]);
        // The emitters, still behind their key, are refused whole; so is
        // the armoury again, being queued already.
        for node in [Node::Emitters, Node::Armoury] {
            let events = world.step(&[Command::Research {
                slot: 0,
                node: node.code(),
            }]);
            assert!(events.contains(&WorldEvent::Refused {
                slot: 0,
                why: Refusal::NotResearchable
            }));
        }
        // The workshop taken off the queue takes the armoury with it, and
        // there is nothing to take a second time.
        let events = world.step(&[Command::Dequeue {
            slot: 0,
            node: Node::Workshop.code(),
        }]);
        for node in [Node::Workshop, Node::Armoury] {
            assert!(
                events.contains(&WorldEvent::ResearchDropped { node: node.code() }),
                "{events:?}"
            );
        }
        assert!(world.research.queue.is_empty());
        let events = world.step(&[Command::Dequeue {
            slot: 0,
            node: Node::Workshop.code(),
        }]);
        assert!(events.contains(&WorldEvent::Refused {
            slot: 0,
            why: Refusal::NotQueued
        }));
        // The AI taken off smelting stands idle; the workshop learnt
        // without its time, the armoury is one node's wait, and queued it
        // is begun the same step.
        world.step(&[Command::CancelResearch { slot: 0 }]);
        assert_eq!(world.research.current, None);
        world.research_for_probe(Node::Workshop);
        let events = world.step(&[Command::Research {
            slot: 0,
            node: Node::Armoury.code(),
        }]);
        assert!(events.contains(&WorldEvent::ResearchQueued {
            node: Node::Armoury.code()
        }));
        assert!(events.contains(&WorldEvent::ResearchBegun {
            node: Node::Armoury.code()
        }));
        assert_eq!(world.research.current, Some(Node::Armoury));
        // The AI runs on the world's clock: a minute of steps is a minute —
        // and the step the order landed in counted too.
        for _ in 0..59 {
            world.step(&[]);
        }
        assert!(
            (world.research.progress - 1.0).abs() < 1e-6,
            "{}",
            world.research.progress
        );
        // Fusion power queued behind it waits its turn; nearly there, then
        // the word — and the AI onto fusion power the same step.
        let events = world.step(&[Command::Research {
            slot: 0,
            node: Node::FusionPower.code(),
        }]);
        assert!(events.contains(&WorldEvent::ResearchQueued {
            node: Node::FusionPower.code()
        }));
        assert!(!events.contains(&WorldEvent::ResearchBegun {
            node: Node::FusionPower.code()
        }));
        assert_eq!(world.research.current, Some(Node::Armoury));
        world.research.progress = Node::Armoury.def().minutes as f64 - 0.5;
        let mut said = false;
        for _ in 0..60 {
            let events = world.step(&[]);
            if events.contains(&WorldEvent::Researched {
                node: Node::Armoury.code(),
            }) {
                assert!(
                    events.contains(&WorldEvent::ResearchBegun {
                        node: Node::FusionPower.code()
                    }),
                    "{events:?}"
                );
                said = true;
                break;
            }
        }
        assert!(said);
        assert!(world.research.is_done(Node::Armoury));
        assert_eq!(world.research.current, Some(Node::FusionPower));
        assert!(world.research.queue.is_empty());
        assert!(world.research.part_allowed(PartKind::Armoury));
    }

    // --- the_benches_and_the_build_tab_wait_on_research ---
    {
        use shipdesign::fixture::playtest_ship;
        use shipdesign::research::Node;
        let mut world = simulation_world(playtest_ship(), data::SIMULATION_MONEY, 1);
        assert!(world.powered(PartKind::Smelter));
        let metal = world.ship.design.carrying(ResourceId::Metal);
        world.set_craft_target(ResourceId::Metal, metal + 1);
        assert!(world.craft_orders().is_empty(), "smelting is not known");
        // Medicine is: a medkit out of the vegetables at the drug lab.
        let medkits = world.ship.design.carrying(ResourceId::Medkit);
        world.set_craft_target(ResourceId::Medkit, medkits + 1);
        let orders = world.craft_orders();
        assert_eq!(orders.len(), 1, "{orders:?}");
        assert_eq!(orders[0].recipe, 5);
        world.set_craft_target(ResourceId::Medkit, 0);

        // A smelter cannot be laid out, and the refusal names the node; a
        // suit locker can, since mining is known from the first day.
        let refusal = world.can_place_site(PartKind::Smelter, (5, 15), Rotation::R0);
        assert_eq!(
            refusal,
            Err(crate::SiteRefusal::NotResearched(Node::Smelting.code()))
        );
        let events = world.step(&[Command::PlaceSite {
            slot: 0,
            kind: PartKind::Smelter,
            origin: (5, 15),
            rotation: Rotation::R0,
        }]);
        assert!(events.contains(&WorldEvent::Refused {
            slot: 0,
            why: Refusal::NotResearched
        }));
        assert!(world.builds.is_empty());
        assert_ne!(
            world.can_place_site(PartKind::SuitLocker, (5, 15), Rotation::R0),
            Err(crate::SiteRefusal::NotResearched(Node::Mining.code()))
        );

        // Known, the smelter is offered the ore.
        world.research_for_probe(Node::Smelting);
        let orders = world.craft_orders();
        assert_eq!(orders.len(), 1, "{orders:?}");
        assert_eq!(orders[0].recipe, 0);
        assert_ne!(
            world.can_place_site(PartKind::Smelter, (5, 15), Rotation::R0),
            Err(crate::SiteRefusal::NotResearched(Node::Smelting.code()))
        );
    }
}

/// Where the keys are: four friendly stations in five have a tier-one key
/// on their desk, an enemy's and a derelict never, and the spawn always —
/// and every station has a desk to keep one on. Which friendly stations
/// hold one is pinned over a seed set across every galaxy type — a count
/// and a hash of `(type, seed, star, station)` — so the tier-two keys
/// going in moved no tier-one key: the set was captured before they did.
#[test]
fn keys_are_on_four_friendly_desks_in_five_and_always_at_the_spawn() {
    use crate::station::key_rolled;
    use worldgen::StationKind;
    let mut with = 0;
    let mut without = 0;
    let mut desks = 0;
    let (count, hash) = tier_one_keys(|station| {
        assert_eq!(station.design.count(PartKind::ResearchDesk), 1);
        desks += 1;
        if station.hostile || station.kind == StationKind::Derelict {
            assert_ne!(station.key, 1, "{:?}", station.kind);
        } else if station.key == 1 {
            with += 1;
        } else {
            without += 1;
        }
        assert_eq!(
            station.key == 1,
            !station.hostile
                && station.kind != StationKind::Derelict
                && key_rolled(station.map_seed)
        );
    });
    assert!(desks > 30, "{desks}");
    assert!(with > 0 && without > 0, "{with} with, {without} without");
    assert_eq!(
        (count, hash),
        (156, 0x7d98_7bd4_6aa9_4ea5),
        "{count} {hash:#x}"
    );
    // The odds themselves, over enough seeds to mean something.
    let rolled = (0..2_000u64)
        .filter(|&seed| key_rolled(seed * 7_919))
        .count();
    assert!((1_500..=1_700).contains(&rolled), "{rolled} of 2000");
    // The spawn: a key whatever it rolled.
    let world = simulation_world(flyer(2), REFERENCE_MONEY, 2);
    assert!(world.station_has_key(world.home));
    assert_eq!(world.station_key(world.home), 1);
    let rolled = world
        .stations
        .iter()
        .zip(&world.station_keys)
        .all(|(s, &key)| key == if s.id == world.home { 1 } else { s.key });
    assert!(rolled);
}

/// The seed set the keys are pinned over: every galaxy type, eight seeds,
/// the first six stars of each — every station of those, in order, to
/// `each`; and how many hold a tier-one key, with a hash of which.
fn tier_one_keys(mut each: impl FnMut(&crate::station::Station)) -> (u64, u64) {
    use crate::station::Station;
    use worldgen::{Galaxy, GalaxyType};
    let mut count = 0u64;
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    let mut eat = |value: u64| {
        for byte in value.to_le_bytes() {
            hash ^= byte as u64;
            hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
        }
    };
    for kind in GalaxyType::ALL {
        for seed in 0..8u64 {
            let galaxy = Galaxy::new(seed, kind);
            for star in 0..(galaxy.stars.len() as u32).min(6) {
                let Some(system) = galaxy.system(star) else {
                    continue;
                };
                for station in Station::all_of(&system) {
                    each(&station);
                    if station.key == 1 {
                        count += 1;
                        eat(kind as u64);
                        eat(seed);
                        eat(star as u64);
                        eat(station.id as u64);
                    }
                }
            }
        }
    }
    (count, hash)
}

/// Where the tier-two keys are: on the desk of every hostile station that
/// is not a derelict, and nowhere else — no friendly station holds one, a
/// derelict holds nothing, the spawn holds tier one whatever it is (the
/// combat arena, hostile, included), and a raider's desk is bare.
#[test]
fn tier_two_keys_lie_only_on_hostile_desks() {
    use crate::station::key_tier;
    use shipdesign::fixture::combat_ship;
    use worldgen::StationKind;
    let mut hostile = 0;
    tier_one_keys(|station| {
        let derelict = station.kind == StationKind::Derelict;
        if derelict {
            assert_eq!(station.key, 0, "a derelict holds nothing");
        } else if station.hostile {
            assert_eq!(station.key, 2, "an enemy's desk holds tier two");
            hostile += 1;
        } else {
            assert_ne!(station.key, 2, "no friendly desk holds tier two");
        }
        assert_eq!(
            station.key,
            key_tier(station.kind, station.hostile, station.map_seed)
        );
    });
    assert!(hostile > 0, "no hostile station in the seed set");
    assert_eq!(key_tier(StationKind::Derelict, true, 7), 0);
    assert_eq!(key_tier(StationKind::Orbital, true, 7), 2);
    // The spawn: tier one, and still tier one made hostile as the arena.
    let world = simulation_world(flyer(2), REFERENCE_MONEY, 2);
    assert_eq!(world.station_key(world.home), 1);
    let mut arena = simulation_world(combat_ship(), REFERENCE_MONEY, 1);
    let home = arena.home;
    arena.set_hostile(home, true);
    assert_eq!(arena.station_key(arena.home), 1);
    for (station, &key) in world.stations.iter().zip(&world.station_keys) {
        if station.id != world.home {
            assert_eq!(key, station.key);
        }
    }
    // A raider: none.
    let mut world = simulation_world(flyer(2), REFERENCE_MONEY, 2);
    world.raid_for_probe(true).expect("a raider tied up");
    let raider = world.ship.state.station().expect("tied to it");
    assert!(crate::raid::raider_index(raider).is_some());
    assert_eq!(world.station(raider).expect("built").key, 0);
    assert!(!world.station_has_key(raider));
    assert_eq!(world.key_at_the_dock(), 0);
}

/// A tier-two key is taken the way a tier-one key is, and no rule stops
/// it: the ship tied up at a hostile station, its people up and at war,
/// a crew member within reach of its desk takes `Item::Key(2)` into the
/// pack; the desk is bare after, and the checksum tells the world from a
/// twin that left the key where it lay.
#[test]
fn a_key_is_taken_at_a_hostile_dock_while_they_stand() {
    use bims::combat::Item;
    use bims::sight::Stance;
    use shipdesign::fixture::playtest_ship;
    use worldgen::StationKind;
    let mut world = simulation_world(playtest_ship(), data::SIMULATION_MONEY, 1);
    // The dressings every Bim carries are out of the way (feature 87).
    without_dressings(&mut world);
    let mut twin = simulation_world(playtest_ship(), data::SIMULATION_MONEY, 1);
    without_dressings(&mut twin);
    let enemy = world
        .stations
        .iter()
        .find(|s| s.hostile && s.kind != StationKind::Derelict)
        .map(|s| s.id)
        .expect("the default seed's spawn system has an enemy in it");
    assert_eq!(world.station_key(enemy), 2);
    for w in [&mut world, &mut twin] {
        w.undock_for_probe();
        w.dock_for_probe(enemy);
    }
    assert_eq!(world.stance(enemy), Stance::Hostile);
    let residents = world.residents.as_ref().expect("their room is open");
    assert_eq!(residents.station, enemy);
    assert!(residents.aboard.count() > 0, "enemies up");
    assert!(
        (0..residents.aboard.count() as usize).all(|i| residents.aboard.room.is_alive(i)),
        "and standing"
    );
    assert_eq!(world.key_at_the_dock(), 2);
    assert_eq!(world.checksum(), twin.checksum());

    // From the ship, out of reach; at the desk, taken — tier two.
    let events = world.step(&[Command::TakeKey { slot: 0, who: 0 }]);
    assert!(
        events.contains(&WorldEvent::Refused {
            slot: 0,
            why: Refusal::OutOfReach
        }),
        "{events:?}"
    );
    let spot = world.key_desk_spot().expect("the enemy's desk");
    world.aboard.room.put_for_probe(0, spot);
    assert!(world.key_in_reach(0));
    let events = world.step(&[Command::TakeKey { slot: 0, who: 0 }]);
    assert!(
        events.contains(&WorldEvent::KeyTaken { who: 0 }),
        "{events:?}"
    );
    assert_eq!(world.aboard.room.pack(0)[0], Some(Item::Key(2)));
    assert_eq!(world.key_at_the_dock(), 0, "the desk is bare");
    assert!(!world.station_has_key(enemy));
    twin.aboard.room.put_for_probe(0, spot);
    twin.step(&[]);
    assert_ne!(world.checksum(), twin.checksum());
}

/// An unlock wants a key of the node's own tier: a tier-one key alone in
/// the desk is refused for the upgrades node and stays; a tier-two key
/// alone is refused for the armoury and stays; and a tier-two key opens
/// the upgrades node and is gone. The desk holds one of either, never
/// two.
#[test]
fn unlock_wants_the_nodes_tier() {
    use shipdesign::fixture::playtest_ship;
    use shipdesign::research::Node;
    let mut world = simulation_world(playtest_ship(), data::SIMULATION_MONEY, 1);
    assert!(world.research_desk_powered());
    let refused = |events: &[WorldEvent], why: Refusal| {
        events.contains(&WorldEvent::Refused { slot: 0, why })
    };

    // A tier-one key in the desk: the upgrades node is refused, and the
    // key stays where it is.
    world.ship.design.cargo[ResourceId::ResearchKey as usize] = 1;
    world.on_ship_changed();
    assert_eq!(world.keys_in_desk(1), 1);
    assert_eq!(world.keys_in_desk(2), 0);
    assert!(!world.ship.design.has_room(ResourceId::ResearchKeyTwo, 1));
    let events = world.step(&[Command::Unlock {
        slot: 0,
        node: Node::Upgrades.code(),
    }]);
    assert!(refused(&events, Refusal::NoKey), "{events:?}");
    assert_eq!(world.keys_in_desk(1), 1);
    assert!(!world.research.is_unlocked(Node::Upgrades));

    // A tier-two key in the desk instead: the armoury is refused and the
    // key stays.
    world.ship.design.cargo[ResourceId::ResearchKey as usize] = 0;
    world.ship.design.cargo[ResourceId::ResearchKeyTwo as usize] = 1;
    world.on_ship_changed();
    assert_eq!(world.keys_in_desk(1), 0);
    assert_eq!(world.keys_in_desk(2), 1);
    assert_eq!(world.ship.design.stored(Storage::Research), 1);
    let events = world.step(&[Command::Unlock {
        slot: 0,
        node: Node::Armoury.code(),
    }]);
    assert!(refused(&events, Refusal::NoKey), "{events:?}");
    assert_eq!(world.keys_in_desk(2), 1);
    assert!(!world.research.is_unlocked(Node::Armoury));
    // Nor a node with no lock, whatever is in the desk.
    let events = world.step(&[Command::Unlock {
        slot: 0,
        node: Node::Smelting.code(),
    }]);
    assert!(refused(&events, Refusal::NotResearchable), "{events:?}");
    assert_eq!(world.keys_in_desk(2), 1);

    // And it opens the upgrades node, and is gone.
    let events = world.step(&[Command::Unlock {
        slot: 0,
        node: Node::Upgrades.code(),
    }]);
    assert!(
        events.contains(&WorldEvent::Unlocked {
            node: Node::Upgrades.code()
        }),
        "{events:?}"
    );
    assert!(world.research.is_unlocked(Node::Upgrades));
    assert_eq!(world.keys_in_desk(2), 0);
    assert_eq!(world.ship.design.carrying(ResourceId::ResearchKeyTwo), 0);
    assert!(!world.research.is_done(Node::Upgrades), "open, not known");
    assert!(!world.research.upgrades_allowed());
}

/// No upgrade before the node: the button is refused `NoUpgrades`, and
/// with Combine matching gear on and two matching tier-one pairs in the
/// hold, a day of simulation begins no upgrade and carries nothing to
/// the bench. The node done, the same world upgrades as before.
#[test]
fn no_upgrade_before_the_node() {
    use bims::combat::{Tier, WeaponKind};
    use shipdesign::fixture::playtest_ship;
    use shipdesign::research::Node;
    let mut world = simulation_world(playtest_ship(), data::SIMULATION_MONEY, 1);
    world.undock_for_probe();
    world.ship.design.cargo[ResourceId::Handgun as usize] += 2;
    world.ship.design.cargo[ResourceId::Helm as usize] += 2;
    world.on_ship_changed();
    // The playtest ship carries a helm of its own: three, two of them a pair.
    let helms = world.ship.design.carrying(ResourceId::Helm);
    assert_eq!(helms, 3);
    assert_eq!(world.guns_at(WeaponKind::LaserPistol, Tier::One), 2);
    assert!(!world.research.is_done(Node::Upgrades));
    assert_eq!(world.can_upgrade(), Err(Refusal::NoUpgrades));
    let events = world.step(&[Command::Upgrade { slot: 0 }]);
    assert!(
        events.contains(&WorldEvent::Refused {
            slot: 0,
            why: Refusal::NoUpgrades
        }),
        "{events:?}"
    );
    assert_eq!(Refusal::NoUpgrades.code(), 40);

    // The tick box on, and a day: nothing begun, nothing carried.
    let events = world.step(&[Command::SetAutoUpgrade { slot: 0, on: true }]);
    unrefused(&events);
    assert!(world.auto_upgrade());
    let (said, _) = run_until(&mut world, 24, &|w| {
        w.bench.carrying.is_some() || !w.bench.is_empty() || !w.aboard.room.ferries().is_empty()
    });
    assert!(said.is_empty(), "{said:?}");
    assert!(world.bench.is_empty(), "{:?}", world.bench);
    assert!(world.bench.carrying.is_none());
    assert!(world.aboard.room.ferries().is_empty());
    assert_eq!(world.guns_at(WeaponKind::LaserPistol, Tier::One), 2);
    assert_eq!(world.ship.design.carrying(ResourceId::Helm), helms);
    guns_agree(&world);

    // The node done: the helms go first, as before, and the day's work
    // begins the step the pair is on the bench.
    upgrades_known(&mut world);
    assert_eq!(world.can_upgrade(), Err(Refusal::NoPair));
    let (said, begun) = run_until(&mut world, 8, &|w| w.bench.work.is_some());
    assert!(begun, "{said:?}");
    assert_eq!(
        said,
        vec![WorldEvent::UpgradeBegun {
            resource: ResourceId::Helm as u32,
            tier: 2,
        }]
    );
    // And the work is worked: a session of it within a few hours.
    let (said, worked) = run_until(&mut world, 6, &|w| {
        w.bench.work.is_some_and(|u| u.done >= 1)
    });
    assert!(worked, "{said:?} {:?}", world.bench);
    assert!(said.is_empty(), "{said:?}");
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

/// The upgrades node researched this instant, and nothing else of the
/// tree: what the workbench asks before it takes two of a kind up a tier
/// (`World::can_upgrade`, `Refusal::NoUpgrades`).
fn upgrades_known(world: &mut World) {
    use shipdesign::research::Node;
    world.research.done[Node::Upgrades as usize] = true;
    assert!(world.research.upgrades_allowed());
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
        upgrades_known(&mut world);
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
        let clock_at_start = world.clock_minutes;

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
            world.clock_minutes - clock_at_start
                >= data::UPGRADE_SESSIONS as f64 * data::UPGRADE_SESSION_MINUTES,
            "a day of work at least: {} minutes",
            world.clock_minutes - clock_at_start
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
        upgrades_known(&mut world);
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
    upgrades_known(&mut world);
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

/// A body of the spawn system the ship can land on, and the ship holding
/// at its arrival radius from the east of it — the side, so a landing has
/// to slide over the planet before it comes down — in its frame, with
/// somebody at the helm. `None` when the spawn system has no such body,
/// which a test says so about and gives up on.
fn over_a_planet(world: &mut World) -> Option<worldgen::Body> {
    let planet = world
        .system
        .bodies
        .iter()
        .find(|b| crate::surface::landable(b.kind))
        .cloned()?;
    world.undock_for_probe();
    world.put_for_probe(
        planet
            .position
            .add(dvec2(flight::data::ARRIVAL_RADIUS_BODY, 0.0)),
    );
    world.settle_frame_for_probe();
    assert_eq!(
        world.ship.frame,
        Frame::Local(worldgen::Node::Body(planet.id))
    );
    world.step(&[]);
    world.man_the_helm_for_probe(0);
    Some(planet)
}

/// Every rocky planet and ice world of a system has a settlement, rolled
/// off the galaxy seed, the star and the body — the same for two players,
/// its side at the generator's share over a galaxy's worth of them — and
/// nothing else has one. A settlement is a station to the world, found by
/// its own id, built the first time it is asked for and not before, its
/// grid centred on the planet; its side is on the hostile list, so
/// `stance` reads it and the checksum notices it. The generator itself
/// never saw any of it: the galaxy checksum is what it was.
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
        assert_eq!(
            world.hostile.binary_search(&surface.id).is_ok(),
            surface.hostile
        );
        assert_eq!(
            world.stance(surface.id),
            if surface.hostile {
                bims::sight::Stance::Hostile
            } else {
                bims::sight::Stance::Neutral
            }
        );
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
    assert_eq!(station.key, 0, "no key on a settlement's desk");
    let at = world
        .system
        .absolute_position(Node::Body(surface.body))
        .unwrap();
    assert!(station.centre().distance(at) < 1e-6);
    assert!(!world.station_has_key(surface.id));

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

/// A station orbits its parent body nearer than a trip to the body ends,
/// so from its berth the planet used to be "already there" — and a ship
/// docked at a derelict in a planet's orbit could never land on it: the
/// trip was refused, and Land wants the planet's frame, which the dock
/// never gives. Now a trip to a body from inside its arrival circle is a
/// hop to the point straight over it — where a landing starts from — and
/// a ship holding there keeps the planet's frame although the station in
/// its orbit is nearer than the planet's centre, so Land is on offer.
#[test]
fn from_a_dock_in_a_planet_s_orbit_a_trip_to_the_planet_ends_over_it_in_its_frame() {
    let mut world = basic();
    let ShipState::Docked { station } = world.ship.state else {
        unreachable!()
    };
    let planet = world
        .system
        .station(station)
        .and_then(|s| s.parent_body)
        .expect("the spawn orbits a body");
    let at = world
        .system
        .absolute_position(worldgen::Node::Body(planet))
        .unwrap();
    assert!(
        at.distance(world.ship.position()) < flight::data::ARRIVAL_RADIUS_BODY,
        "the berth is inside the planet's arrival circle"
    );
    let target = Target::Body(planet);
    let quote = world.preview(target).expect("the planet can be flown to");
    assert!(quote.minutes > 0.0);
    assert!(!quote.docks);

    set_off(&mut world, 0, target);
    until_stopped(&mut world, 200_000);
    assert_eq!(world.ship.state, ShipState::Holding);
    let over = at.add(dvec2(0.0, data::LANDING_HEIGHT));
    assert!(
        world.ship.position().distance(over) < 1.0,
        "ended {:?} rather than over the planet at {over:?}",
        world.ship.position()
    );
    assert_eq!(
        world.ship.frame,
        Frame::Local(worldgen::Node::Body(planet)),
        "there for the planet, not for the station beside it"
    );
    // And it stays so: the station is nearer than the planet's centre
    // from most of the orbit, and the frame is not up for grabs.
    for _ in 0..600 {
        world.step(&[]);
    }
    assert_eq!(world.ship.frame, Frame::Local(worldgen::Node::Body(planet)));
    // Land is on offer if the planet has ground; else it is only the
    // planet that is missing, not the hold over it.
    let landable = world
        .system
        .bodies
        .iter()
        .any(|b| b.id == planet && crate::surface::landable(b.kind));
    match world.can_land() {
        Ok(body) => assert!(landable && body == planet),
        Err(why) => assert!(!landable && why == Refusal::NoPlanetHere, "{why:?}"),
    }
    // Standing on that point, the planet is where the ship already is.
    assert_eq!(world.preview(target), Err(PlanError::AlreadyThere));
}

/// A Land from a hold over a planet is a landing: the same docking state
/// as a berth's, over `LAND_MINUTES`, sliding to the point straight over
/// the planet and coming down from there onto the pad — and at the end
/// the ship is tied up at the settlement with the rooms joined, on the
/// ground, in the planet's frame, with the settlement's people in their
/// own room, its guard on the way to its post, and its desk open for
/// trade. A Confirm from the pad is a lift-off: straight up to where a
/// trip to the planet would have ended, and the trip from there.
#[test]
fn a_ship_holding_over_a_planet_lands_on_the_pad_and_lifts_off_straight_up() {
    use crate::surface::{GUARD, guard_post, surface_id};
    use worldgen::Node;
    let mut world = basic();
    let Some(planet) = over_a_planet(&mut world) else {
        eprintln!("the spawn system has no planet to land on");
        return;
    };
    let id = surface_id(planet.id);
    assert_eq!(world.can_land(), Ok(planet.id));
    let money = world.money;

    let events = world.step(&[Command::Land { slot: 0 }]);
    assert!(
        events.contains(&WorldEvent::Landing { body: planet.id }),
        "{events:?}"
    );
    assert!(
        matches!(world.ship.state, ShipState::Docking { station, .. } if station == id),
        "{:?}",
        world.ship.state
    );
    // Halfway through, the ship is straight over the planet, up: the
    // slide is done and the descent about to begin.
    let half = (data::LAND_MINUTES / 2.0 / data::STEP_MINUTES).ceil() as u32;
    for _ in 0..half {
        world.step(&[]);
    }
    let over = planet.position.add(dvec2(0.0, data::LANDING_HEIGHT));
    assert!(
        world.ship.position().distance(over) < 1.0,
        "{:?} is not over the planet at {:?}",
        world.ship.position(),
        over
    );
    assert!(
        world
            .landing()
            .is_some_and(|(body, done)| body == planet.id && done > 0.4)
    );
    let events = until_stopped(&mut world, 2 * half + 10);
    assert!(
        events.contains(&WorldEvent::Landed { body: planet.id }),
        "{events:?}"
    );
    assert_eq!(world.ship.state, ShipState::Docked { station: id });
    assert_eq!(world.landed(), Some(planet.id));
    assert_eq!(world.landing(), None);
    assert_eq!(world.ship.frame, Frame::Local(Node::Body(planet.id)));
    // On the ground: within the settlement's grid of the planet's middle.
    let ground = data::SURFACE_SIDE as f64 * shipdesign::TILE as f64;
    assert!(world.ship.position().distance(planet.position) < ground);
    assert!(world.aboard.is_joined(), "the rooms are one deck");
    let residents = world.residents.as_ref().expect("the settlement's people");
    assert_eq!(residents.station, id);
    assert!(residents.aboard.count() >= data::SURFACE_POPULATION.0);
    // The guard on its way to the post outside the watch house, and there
    // within the hour.
    let post = guard_post();
    let mut arrived = false;
    for _ in 0..3_600 {
        world.step(&[]);
        let residents = world.residents.as_ref().unwrap();
        let at = residents.aboard.position(GUARD);
        if at.distance(post) < 1.5 * shipdesign::TILE as f64 {
            arrived = true;
            break;
        }
    }
    assert!(arrived, "the guard never reached its post");
    // Trade, at the settlement's desk.
    assert!(world.man_the_desk_for_probe(0));
    let events = world.step(&[Command::Buy {
        slot: 0,
        resource: ResourceId::Vegetable,
        units: 1,
    }]);
    assert!(
        events
            .iter()
            .any(|e| matches!(e, WorldEvent::Traded { .. })),
        "{events:?}"
    );
    assert!(world.money < money);

    // And off again: a Confirm is a cast-off, then a climb straight up.
    world.man_the_helm_for_probe(0);
    let target = Target::Point(planet.position.add(dvec2(0.0, 4.0 * data::LANDING_HEIGHT)));
    let events = world.step(&[Command::Confirm { slot: 0, target }]);
    assert!(
        events.contains(&WorldEvent::CastingOff { slot: 0 }),
        "{events:?}"
    );
    // Everybody is home already, so the cast-off is over in the same
    // step and the climb begins.
    let mut lifted = events.contains(&WorldEvent::LiftedOff { body: planet.id });
    let mut planned = None;
    for _ in 0..(data::CASTING_OFF_LIMIT + data::LIFT_MINUTES + 5.0) as u32 * 60 {
        let events = world.step(&[]);
        if events.contains(&WorldEvent::LiftedOff { body: planet.id }) {
            lifted = true;
            assert!(world.lifting().is_some_and(|(body, _)| body == planet.id));
            assert_eq!(world.landed(), None, "off the ground");
        }
        if let ShipState::Travelling { .. } = world.ship.state {
            planned = Some(world.ship.position());
            break;
        }
    }
    assert!(lifted, "the ship never lifted off");
    let from = planned.expect("the trip never began");
    assert!(
        from.distance(over) < 1.0,
        "the trip begins at {from:?}, not over the planet at {over:?}"
    );
    assert!(world.residents.is_none(), "the settlement is behind");
}

/// A Land is refused everywhere but a hold over a planet with ground: at
/// a berth, at a belt, and under way.
#[test]
fn a_land_wants_a_hold_over_a_planet_with_ground() {
    let mut world = basic();
    assert_eq!(world.can_land(), Err(Refusal::NotHolding));
    world.man_the_helm_for_probe(0);
    let events = world.step(&[Command::Land { slot: 0 }]);
    assert!(events.contains(&WorldEvent::Refused {
        slot: 0,
        why: Refusal::NotHolding
    }));
    if at_a_belt(&mut world).is_some() {
        assert_eq!(world.can_land(), Err(Refusal::NoPlanetHere));
        world.man_the_helm_for_probe(0);
        let events = world.step(&[Command::Land { slot: 0 }]);
        assert!(events.contains(&WorldEvent::Refused {
            slot: 0,
            why: Refusal::NoPlanetHere
        }));
    }
    if over_a_planet(&mut world).is_some() {
        let target = nearby(&world, 200_000.0);
        world.step(&[Command::Confirm { slot: 0, target }]);
        assert!(matches!(world.ship.state, ShipState::Travelling { .. }));
        assert_eq!(world.can_land(), Err(Refusal::NotHolding));
    }
}

/// After a fight at a hostile dock the crew go back to their day — and to
/// bed. The alarm is a fight, not a berth: with nobody hit for the hold,
/// nobody in anybody's sight for as long and none of the station's people
/// within `ALARM_RANGE`, it comes down though the station is still an
/// enemy's and some of its people still alive on it, and Kate — who took
/// arms at the alarm — is let go to her errands and turns in that night
/// like any other. It used to hold for any enemy within a hundred tiles,
/// which was anybody left alive on the station, and a crew docked at an
/// enemy's neither ate nor slept for as long as they were tied up there.
#[test]
fn after_a_fight_at_a_hostile_dock_the_alarm_comes_down_and_the_crew_sleep() {
    use bims::combat::{Gear, WeaponKind};
    use bims::game::{ALARM_HOLD, JOB_SLEEP};
    let mut world = with_a_crewmate();
    assert!(world.stage_fight_for_probe());
    world.aboard.room.issue(
        0,
        Gear {
            weapon: Some(WeaponKind::AutoRifle.basic()),
            ..Gear::issued()
        },
    );
    // The fight: until the resident James was put in front of is down, or
    // an hour has gone by. Both crew patched up every step so neither is
    // bled out cold on the deck for days, which is a different story —
    // **and patched through the loops after it as well**, since the
    // station is still hostile and its people still shoot: whether the
    // crew come out of the last exchange with an open wound is the fight
    // re-rolled (feature 84 moved the shots to the muzzles and re-rolled
    // it), and a crew bled out over the following day says nothing about
    // the alarm coming down or about anybody's night.
    let mut steps = 0;
    let mut alarmed = false;
    while steps < 3600 {
        world.aboard.room.patch_up_for_probe(0);
        world.aboard.room.patch_up_for_probe(1);
        world.step(&[]);
        steps += 1;
        alarmed |= world.aboard.room.is_alarmed();
        let ashore = world.residents.as_ref().unwrap();
        if !ashore.aboard.room.is_alive(0) {
            break;
        }
    }
    assert!(alarmed, "the alarm went up for the fight");
    assert!(
        !world.residents.as_ref().unwrap().aboard.room.is_alive(0),
        "and the resident is down"
    );
    // The player stands its own down; the rest is the room's.
    world.aboard.room.recruit_for_probe(0, false);
    // Within the hold plus a little for the last bolt to land, the alarm
    // is down and Kate is hers again — the station still hostile, with
    // people left alive on it, if out of sight and out of range.
    let hold = (ALARM_HOLD * 60.0) as usize + 60 * 60;
    let mut down = false;
    for _ in 0..hold {
        world.aboard.room.patch_up_for_probe(0);
        world.aboard.room.patch_up_for_probe(1);
        world.step(&[]);
        if !world.aboard.room.is_alarmed() {
            down = true;
            break;
        }
    }
    assert!(down, "the alarm comes down after the fight");
    assert!(
        world
            .hostile
            .contains(&world.residents.as_ref().unwrap().station)
    );
    for _ in 0..60 {
        world.aboard.room.patch_up_for_probe(0);
        world.aboard.room.patch_up_for_probe(1);
        world.step(&[]);
    }
    assert!(!world.aboard.room.is_alarmed(), "and stays down");
    assert!(!world.aboard.room.is_armed(1), "Kate holsters");
    // And within the day both have been to bed.
    let mut slept = [false, false];
    for _ in 0..(24 * 3600) {
        world.step(&[]);
        for who in 0..2 {
            slept[who] |= world.aboard.room.activity(who) == JOB_SLEEP;
        }
        if slept[0] && slept[1] {
            break;
        }
    }
    assert!(slept[0] && slept[1], "both slept: {slept:?}");
}

/// A probe, not a test — `cargo test -p world -- --ignored --nocapture
/// tier_two_probe` prints its numbers and asserts nothing worth the name.
/// The three questions the tier-two key raised: how many tier-one keys a
/// crew's start system holds and how often an enemy's desk is in it, over
/// every galaxy type and fifty seeds; how far the walk is from a station's
/// port to its research desk, plan by plan; and what becomes of a key in
/// the pack of a crew member left down aboard a station when the ship
/// casts off. Reported so the numbers can be read, and nothing changed
/// because of them.
#[test]
#[ignore]
fn tier_two_probe() {
    use crate::station::{Plan, key_tier};
    use bims::combat::Item;
    use shipdesign::fixture::playtest_ship;
    use worldgen::{Galaxy, GalaxyType, StationKind};

    // 1. The start system, per galaxy type over fifty seeds.
    println!("--- start systems: every galaxy type x 50 seeds ---");
    for kind in GalaxyType::ALL {
        let mut starts = 0u32;
        let mut keys = 0u32;
        let mut keys_max = 0u32;
        let mut keys_none = 0u32;
        let mut hostile_starts = 0u32;
        let mut bearing = 0u32;
        let mut bearing_hostile = 0u32;
        for seed in 0..50u64 {
            let galaxy = Galaxy::new(seed, kind);
            let Some((star, dock)) = crate::spawn(&galaxy) else {
                continue;
            };
            let system = galaxy.system(star).expect("the spawn's system");
            starts += 1;
            let tier_one = system
                .stations
                .iter()
                .filter(|s| s.id == dock || key_tier(s.kind, s.hostile, s.map_seed) == 1)
                .count() as u32;
            keys += tier_one;
            keys_max = keys_max.max(tier_one);
            if tier_one == 0 {
                keys_none += 1;
            }
            if system
                .stations
                .iter()
                .any(|s| s.hostile && s.kind != StationKind::Derelict)
            {
                hostile_starts += 1;
            }
            for system in galaxy.every_system() {
                if system.stations.is_empty() {
                    continue;
                }
                bearing += 1;
                if system
                    .stations
                    .iter()
                    .any(|s| s.hostile && s.kind != StationKind::Derelict)
                {
                    bearing_hostile += 1;
                }
            }
        }
        println!(
            "{kind:?}: {starts} starts; tier-one keys in the start system {:.2} on average (spawn included), most {keys_max}, none {keys_none}; start systems with an enemy's desk {hostile_starts}/{starts} ({:.0}%); station-bearing systems with one {bearing_hostile}/{bearing} ({:.0}%)",
            keys as f64 / starts.max(1) as f64,
            100.0 * hostile_starts as f64 / starts.max(1) as f64,
            100.0 * bearing_hostile as f64 / bearing.max(1) as f64,
        );
    }

    // 2. The walk from the port to the research desk, per plan and kind.
    println!("--- port to research desk, in tiles walked ---");
    let galaxy = Galaxy::new(data::DEFAULT_SEED, GalaxyType::SpiralTwoArm);
    let (star, _) = crate::spawn(&galaxy).unwrap();
    let system = galaxy.system(star).unwrap();
    for plan in Plan::ALL {
        for kind in [
            StationKind::Orbital,
            StationKind::Refinery,
            StationKind::MiningOutpost,
            StationKind::Relay,
        ] {
            let blueprint = system
                .stations
                .iter()
                .find(|s| s.kind == kind)
                .cloned()
                .unwrap_or_else(|| {
                    let mut b = system.stations[0].clone();
                    b.kind = kind;
                    b
                });
            let station = Station::build_as(&blueprint, DVec2::ZERO, plan);
            let Some(port) = station.port() else {
                println!("{plan:?} {kind:?}: no port");
                continue;
            };
            let mut residents =
                crate::crew::Residents::open(station.id, &station.design, 1, 0, 7, 0.0, &[]);
            let inside = data::ASHORE_TILES * shipdesign::TILE as f64;
            let at = dvec2(
                port.centre.0 - port.outward.0 as f64 * inside,
                port.centre.1 - port.outward.1 as f64 * inside,
            );
            let at = residents.aboard.to_room(at);
            let room = &mut residents.aboard.room;
            let from = room.put_for_probe(0, at);
            let Some(desk) = room.research_spot(0) else {
                println!("{plan:?} {kind:?}: no research desk");
                continue;
            };
            let reached = room.send_for_probe(0, desk);
            let (path, _) = room.route_for_probe(0);
            let mut walked = 0.0f32;
            let mut last = from;
            for p in &path {
                walked += (*p - last).len();
                last = *p;
            }
            println!(
                "{plan:?} {kind:?} ({} tiles a side): {:.1} tiles{}",
                station.design.build_area,
                walked / bims::room::TILE,
                if reached { "" } else { " (no way there)" }
            );
        }
    }

    // 3. A crew member left down at the enemy's desk with the key in
    // their pack, and the ship casting off.
    println!("--- a key in the pack of a crew member left down ashore ---");
    let mut world = simulation_world(playtest_ship(), data::SIMULATION_MONEY, 2);
    let enemy = world
        .stations
        .iter()
        .find(|s| s.hostile && s.kind != StationKind::Derelict)
        .map(|s| s.id)
        .expect("an enemy in the spawn system");
    world.undock_for_probe();
    world.dock_for_probe(enemy);
    let spot = world.key_desk_spot().unwrap();
    world.aboard.room.put_for_probe(0, spot);
    let events = world.step(&[Command::TakeKey { slot: 0, who: 0 }]);
    println!(
        "taken: {}",
        events.contains(&WorldEvent::KeyTaken { who: 0 })
    );
    world.aboard.room.knock_out_for_probe(0);
    world.step(&[]);
    println!(
        "down at the desk: unconscious {}, alive {}, pack[0] {:?}",
        world.aboard.room.is_unconscious(0),
        world.aboard.room.is_alive(0),
        world.aboard.room.pack(0)[0]
    );
    world.undock_for_probe();
    let room = &world.aboard.room;
    println!(
        "cast off: crew aboard {}, crew member 0 alive {}, unconscious {}, at {:?} (bunk {:?}), pack[0] {:?}, hold's tier-two keys {}",
        room.crew_count(),
        room.is_alive(0),
        room.is_unconscious(0),
        room.bim_pos(0),
        room.bed_of(0),
        room.pack(0)[0],
        world.ship.design.carrying(ResourceId::ResearchKeyTwo),
    );
    assert!(matches!(room.pack(0)[0], Some(Item::Key(2)) | None));
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
