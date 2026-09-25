//! A run (feature 102): what the roguelike's first step switched off, and
//! the footing a run stands on now — no needs, no human enemies, the
//! crisis there from the start, gear the one thing a desk sells, nothing
//! built onto the ship, and a station's people walking a scripted round.

use bims::routine::Role;
use bims::sight::Stance;
use physics::ResourceId;
use shipdesign::Rotation;
use shipdesign::fixture::flyer;
use shipdesign::parts::PartKind;
use worldgen::{Galaxy, GalaxyType};

use crate::crew::Residents;
use crate::data;
use crate::event::{Refusal, WorldEvent};
use crate::fixture::{REFERENCE_MONEY, simulation_world};
use crate::station::Station;
use crate::surface::Surface;
use crate::world::{Command, ShipState, World};

fn basic() -> World {
    simulation_world(flyer(2), REFERENCE_MONEY, 2)
}

/// No human is ever the crew's enemy in a generated galaxy: wherever a
/// crew starts, nothing in the system is hostile but what the machines
/// hold — the stations the generator rolled hostile among them — and
/// nothing after a jump either; and a station's dead are nobody's to
/// strip.
#[test]
fn no_human_is_ever_hostile_in_a_generated_galaxy() {
    let mut rolled_hostile = 0;
    for (seed, galaxy_type) in [
        (data::DEFAULT_SEED, GalaxyType::SpiralTwoArm),
        (0x_0102_0304, GalaxyType::Spiral),
        (0x_7777_1234, GalaxyType::Round),
    ] {
        let galaxy = Galaxy::new(seed, galaxy_type);
        for pick in 0..6u64 {
            let Some((star, station)) = crate::spawn_anywhere(&galaxy, pick * 7_919) else {
                continue;
            };
            let world = World::start(
                flyer(2),
                REFERENCE_MONEY,
                2,
                seed,
                galaxy_type,
                star,
                station,
            )
            .expect("a dock to start at");
            for s in &world.stations {
                rolled_hostile += usize::from(s.hostile);
                if !world.is_droid_held(s.id) {
                    assert_ne!(world.stance(s.id), Stance::Hostile, "station {}", s.id);
                    assert_eq!(world.people_of(s), s.residents(), "station {}", s.id);
                }
            }
            for s in &world.surfaces {
                rolled_hostile += usize::from(s.hostile);
                if !world.is_droid_held(s.id) {
                    assert_ne!(world.stance(s.id), Stance::Hostile, "town {}", s.id);
                }
            }
        }
    }
    assert!(
        rolled_hostile > 0,
        "the sample never met a station the generator rolled hostile"
    );

    // And a jump: the system arrived at has nobody hostile in it either.
    let mut world = basic();
    out_in_the_open(&mut world);
    let next = world.reachable_stars()[0];
    jump_to(&mut world, next);
    for s in &world.stations {
        if !world.is_droid_held(s.id) {
            assert_ne!(world.stance(s.id), Stance::Hostile);
        }
    }

    // And looting one of a station's people is refused: the dead are
    // nobody's to strip, and only a crewmate's body is the crew's.
    let mut world = basic();
    world
        .residents
        .as_mut()
        .unwrap()
        .aboard
        .room
        .kill_for_probe(0);
    world.step(&[]);
    let events = world.step(&[Command::Loot {
        slot: 0,
        who: 0,
        source: crate::LootSource::Resident(0),
        cell: 0,
    }]);
    assert!(
        events.contains(&WorldEvent::Refused {
            slot: 0,
            why: Refusal::NotACrewmate
        }),
        "{events:?}"
    );
}

/// The crisis is there from day nought: the origin is the machines' the
/// moment the world opens, and so is every star due by then — and a
/// system due is theirs whole the moment the crew are in it, its jammer
/// standing and a station's people a wave, exactly as if the spread
/// had run to it.
#[test]
fn at_day_nought_the_systems_due_to_have_fallen_are_infested() {
    let world = basic();
    assert_eq!(world.days_gone(), 0);
    assert_eq!(world.crisis_first_day(), 0);
    let origin = world.droid_origin();
    assert!(world.infested(origin), "the origin is theirs on day nought");
    let galaxy = world.galaxy();
    let due: Vec<u32> = (0..galaxy.stars.len() as u32)
        .filter(|&s| world.infested_on(s) == 0)
        .collect();
    assert_eq!(world.infested_stars(), due);
    assert_eq!(due, vec![origin], "one star is due on day nought");
    assert_eq!(world.crisis_radius(), Some(0));
    // The front is there from the start: a star next to the origin is on it.
    let hops = galaxy.hops_from(origin);
    let neighbour = (0..galaxy.stars.len() as u32)
        .find(|&s| hops[s as usize] == 1)
        .expect("the origin has a lane");
    assert_eq!(world.front(neighbour), Some(1));

    // Move the origin next door and jump there: the system arrived at is
    // the machines' already, every station of it and its jammer.
    let mut world = basic();
    out_in_the_open(&mut world);
    let next = world.reachable_stars()[0];
    world.set_droid_origin_for_probe(next);
    assert!(world.infested(next));
    jump_to(&mut world, next);
    world.step(&[]);
    assert!(world.infested(world.star_id));
    let jammer = world.jammer_station().expect("a jammer stands");
    assert!(world.stations.iter().any(|s| s.id == jammer));
    for s in &world.stations {
        assert!(world.is_droid_held(s.id), "station {} is theirs", s.id);
        assert_eq!(world.stance(s.id), Stance::Hostile);
        assert_eq!(world.people_of(s), 0, "its people are gone");
    }
    for s in &world.surfaces {
        assert!(world.is_droid_held(s.id), "town {} is theirs", s.id);
    }
}

/// A run starts with gear and time: a desk sells the crew guns and armour
/// and nothing the ship lives on, nothing is built onto the ship, and a
/// bunk puts no cap on the crew.
#[test]
fn a_run_buys_gear_builds_nothing_and_has_no_bunk_cap() {
    let mut world = basic();
    let station = world.ship.state.station().unwrap();
    assert!(world.buyable(ResourceId::Handgun));
    assert!(world.buyable(ResourceId::Kevlar));
    for goods in [
        ResourceId::Vegetable,
        ResourceId::Tofu,
        ResourceId::Suit,
        ResourceId::Medkit,
        ResourceId::Bandage,
    ] {
        assert!(!world.buyable(goods), "{goods:?}");
    }
    world.man_the_desk_for_probe(0);
    let sold = world
        .station(station)
        .is_some_and(|s| s.stock.sells(ResourceId::Vegetable));
    assert!(sold, "the spawn's shelf has vegetables on it");
    let events = world.step(&[Command::Buy {
        slot: 0,
        resource: ResourceId::Vegetable,
        units: 1,
        tier: 1,
    }]);
    assert!(
        events.contains(&WorldEvent::Refused {
            slot: 0,
            why: Refusal::NotSoldHere
        }),
        "{events:?}"
    );

    // Nothing built onto the ship.
    assert_eq!(
        world.can_place_site(PartKind::Wall, (1, 1), Rotation::R0),
        Err(crate::SiteRefusal::NoShipyard)
    );
    let events = world.step(&[Command::PlaceSite {
        slot: 0,
        kind: PartKind::Wall,
        origin: (1, 1),
        rotation: Rotation::R0,
    }]);
    assert!(
        events.contains(&WorldEvent::Refused {
            slot: 0,
            why: Refusal::NoShipyard
        }),
        "{events:?}"
    );
    assert!(world.builds.is_empty());

    // And a bunk is furniture: a crew of twenty on the playtest ship is
    // more than every bunk on the joined deck, and a mercenary is offered
    // all the same.
    let mut world =
        crate::fixture::crewed_world(shipdesign::fixture::playtest_ship(), REFERENCE_MONEY, 1, 20);
    assert!(world.aboard.room.bed_count() < world.aboard.crew_count() as usize);
    assert!(world.mercenary_for_probe());
    let merc = world.residents.as_ref().unwrap().aboard.count() - 1;
    let offer = world.hire_offer(0, merc).expect("a mercenary for hire");
    assert!(offer.docked && offer.affordable, "{offer:?}");
}

/// The rounds: dealt to a station's people when the room opens, one role
/// each, and derived from the site's own fixtures — so every role has a
/// round on every kind of generated site, stations of every plan and
/// towns alike, and where a site has not got what the role wants the
/// fallback's wander still gives it somewhere to go.
#[test]
fn every_role_has_a_round_on_a_sample_of_stations_and_towns() {
    let galaxy = Galaxy::new(data::DEFAULT_SEED, GalaxyType::SpiralTwoArm);
    let mut sites: Vec<Station> = Vec::new();
    let mut plans = Vec::new();
    for system in galaxy.every_system().iter().take(80) {
        for station in Station::all_of(system) {
            if crate::station::residents_of(station.kind) == 0 || plans.contains(&station.plan) {
                continue;
            }
            plans.push(station.plan);
            sites.push(station);
        }
    }
    let mut towns = 0;
    for system in galaxy.every_system().iter().take(80) {
        if towns >= 2 {
            break;
        }
        for surface in Surface::all_of(system, galaxy.seed).iter().take(1) {
            sites.push(surface.station().clone());
            towns += 1;
        }
    }
    assert!(plans.len() >= 5, "only {} plans in the sample", plans.len());
    assert!(towns >= 1, "no town in the sample");

    let mut own = [0usize; 4];
    for site in &sites {
        let count = site.residents().max(1);
        let mut residents =
            Residents::open(site.id, &site.design, count, 0, site.map_seed, 0.0, &[]);
        residents.deal_roles(site);
        let room = &mut residents.aboard.room;
        // What the world dealt: everybody a round with somewhere on it.
        for who in 0..room.crew_count() as usize {
            let routine = room.routine(who).expect("everybody is dealt a round");
            assert!(!routine.stops.is_empty(), "{:?} {who}", site.plan);
        }
        // And every role, on this site: its own round or the fallback's,
        // and every stop somewhere the body can walk to.
        let gates: Vec<bims::math::Vec2> = if site.plan == crate::station::Plan::Surface {
            crate::surface::gates()
                .into_iter()
                .map(|g| residents.aboard.to_room(g))
                .collect()
        } else {
            Vec::new()
        };
        let room = &mut residents.aboard.room;
        for role in Role::ALL {
            assert!(room.set_role(0, role, &gates, 7));
            let routine = room.routine(0).unwrap().clone();
            assert!(
                !routine.stops.is_empty(),
                "{role:?} on a {:?} has nowhere to go",
                site.plan
            );
            assert!(
                room.reachable_for_probe(
                    0,
                    &routine.stops.iter().map(|s| s.at).collect::<Vec<_>>()
                ),
                "{role:?} on a {:?} has a stop it cannot walk to",
                site.plan
            );
            if !routine.fallback {
                own[role.code() as usize] += 1;
            }
        }
    }
    for role in Role::ALL {
        assert!(
            own[role.code() as usize] > 0,
            "{role:?} never had a round of its own"
        );
    }

    // A site with none of what the roles want — no desk, no benches, no
    // research, no shelves, no bunks, no showers, one airlock — still gives
    // every role a round: the fallback's, over the open deck.
    let site = &sites[0];
    let mut bare = site.design.clone();
    let mut airlocks = 0;
    bare.parts.retain(|p| match p.kind {
        PartKind::TradingDesk
        | PartKind::ResearchDesk
        | PartKind::Shelf
        | PartKind::Bunk
        | PartKind::Shower
        | PartKind::Workbench
        | PartKind::DrugLab => false,
        PartKind::Airlock => {
            airlocks += 1;
            airlocks == 1
        }
        _ => true,
    });
    let mut room = bims::aboard::game_aboard(&bare, 1, 3);
    for role in Role::ALL {
        assert!(room.set_role(0, role, &[], 11));
        let routine = room.routine(0).unwrap();
        assert!(
            routine.fallback,
            "{role:?} found its fixtures on a bare site"
        );
        assert!(routine.stops.len() >= 2, "{role:?}: {:?}", routine.stops);
    }
}

/// Two runs on one seed walk the same rounds step for step, and the
/// rounds are walked: a station's people get somewhere in an afternoon.
#[test]
fn two_runs_on_one_seed_walk_the_same_rounds() {
    let mut a = basic();
    let mut b = basic();
    let positions = |w: &World| -> Vec<(i64, i64)> {
        let residents = w.residents.as_ref().unwrap();
        (0..residents.aboard.room.crew_count())
            .map(|who| {
                let at = residents.aboard.position(who);
                ((at.x * 1_000.0) as i64, (at.y * 1_000.0) as i64)
            })
            .collect()
    };
    let start = positions(&a);
    let mut moved = false;
    let mut advanced = false;
    for step in 0..(4 * 60 * 60) {
        a.step(&[]);
        b.step(&[]);
        if step % 300 == 0 {
            assert_eq!(positions(&a), positions(&b), "step {step}");
            assert_eq!(a.checksum(), b.checksum(), "step {step}");
        }
        let now = positions(&a);
        moved |= now.iter().zip(&start).any(|(p, q)| {
            ((p.0 - q.0).pow(2) + (p.1 - q.1).pow(2)) as f64
                > (2.0 * 1_000.0 * shipdesign::parts::TILE as f64).powi(2)
        });
        let room = &a.residents.as_ref().unwrap().aboard.room;
        advanced |= (0..room.crew_count() as usize)
            .any(|who| room.routine(who).is_some_and(|r| r.next > 0));
    }
    assert_eq!(positions(&a), positions(&b));
    assert!(moved, "nobody of the station's people walked anywhere");
    assert!(advanced, "nobody got on to a second stop of a round");
}

// --- the jump helpers, as `tests_crisis` has them ----------------------------

/// Off the berth and holding far from every station.
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
}

/// Jumped to `star` — what a trip across a hyperlane does on the way
/// (`World::jump`) — and a step after it.
fn jump_to(world: &mut World, star: u32) {
    assert_eq!(world.ship.state, ShipState::Holding);
    let mut events = Vec::new();
    assert!(world.jump(star, &mut events), "the ship never jumped");
    assert_eq!(world.star_id, star);
    world.step(&[]);
}
