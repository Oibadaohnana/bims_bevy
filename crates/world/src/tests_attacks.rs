//! The second set of attack missions (October 2026, `attacks.rs`): each
//! laid on a station built for it, and each played through by hand —
//! the Overseer running from the crew, hurt into fleeing and out of his
//! airlock with the bounty,
//! a heist's terminal taken and the site held until all are, a prison's
//! cell cut open and its prisoners armed, a fuel run's drum gone up in
//! the arms and another put in the reactor, a salvage crate's wave and its
//! pay aboard the ship.

use bims::combat::{Gear, WeaponKind};
use bims::droid::DroidPart;
use shipdesign::fixture::{COMBAT_CREW, combat_ship};
use worldgen::GalaxyType;

use crate::data;
use crate::event::WorldEvent;
use crate::fixture::REFERENCE_MONEY;
use crate::objective::{LoadState, Objective, OverseerPhase};
use crate::run::Mission;
use crate::world::{Command, World};

/// The `droids` probe's world made a mission of the second set: the
/// combat ship's crew docked at the arena held by the machines, the
/// mission forced, the dock laid out for it, and the room opened with its
/// objective laid. The station.
fn mission_world(mission: Mission) -> (World, u32) {
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
    world.set_machines_only_for_probe();
    world.arena_dock_for_probe();
    let kinds = WeaponKind::ALL.iter().copied().cycle();
    let crew = world.aboard.room.crew_count() as usize;
    for (who, kind) in kinds.take(crew).enumerate() {
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
    world.set_mission_for_probe(Some(mission));
    assert!(world.fit_dock_for_probe(), "{mission:?}: laid out for it");
    for _ in 0..60 {
        world.step(&[]);
        if objective(&world, station).is_some() {
            return (world, station);
        }
    }
    panic!("{mission:?}: no objective laid");
}

fn objective(world: &World, station: u32) -> Option<Objective> {
    world.infestation(station)?.objective.clone()
}

/// Every machine on the residents' deck destroyed where it stands.
fn wreck_them_all(world: &mut World) {
    if let Some(residents) = world.residents.as_mut() {
        let room = &mut residents.aboard.room;
        for i in 0..room.droid_count() as usize {
            room.strike_droid(i, DroidPart::Chassis, 1e6);
        }
    }
}

/// A design point of the site on the crew's deck.
fn on_deck(world: &World, p: worldgen::math::DVec2) -> bims::math::Vec2 {
    let q = world.aboard.from_station(p).unwrap();
    bims::math::vec2(q.x as f32, q.y as f32)
}

fn mark_at(world: &World, pick: impl Fn(crate::MarkKind) -> bool) -> bims::math::Vec2 {
    let look = world.objective_look().expect("a look");
    let m = look.marks.iter().find(|m| pick(m.kind)).expect("the mark");
    on_deck(world, m.at)
}

#[test]
fn every_mission_of_the_second_set_is_laid_on_a_station_built_for_it() {
    for (mission, feature) in [
        (Mission::Overseer, crate::stationgen::Feature::Office),
        (Mission::Heist, crate::stationgen::Feature::Servers),
        (Mission::Prison, crate::stationgen::Feature::Brig),
        (Mission::FuelRun, crate::stationgen::Feature::FuelRun),
        (Mission::Salvage, crate::stationgen::Feature::Cargo),
    ] {
        let (world, station) = mission_world(mission);
        let site = world.station(station).unwrap();
        assert_eq!(site.fitted.as_ref().map(|f| f.feature), Some(feature));
        let o = objective(&world, station).unwrap();
        let kind_ok = matches!(
            (mission, &o),
            (Mission::Overseer, Objective::Overseer(_))
                | (Mission::Heist, Objective::Heist(_))
                | (Mission::Prison, Objective::Prison(_))
                | (Mission::FuelRun, Objective::FuelRun(_))
                | (Mission::Salvage, Objective::Salvage(_))
        );
        assert!(kind_ok, "{mission:?}: {o:?}");
        assert_eq!(world.objective_look().unwrap().mission, mission);
    }
}

#[test]
fn every_station_attack_is_one_of_the_seven_each_as_likely() {
    let (world, _) = mission_world(Mission::Heist);
    let mut world = world;
    world.set_mission_for_probe(None);
    // A roll off the floor is plain: the floor is what deals them, so the
    // roll is asked straight, over many stations of a galaxy.
    let mut counts = [0u32; 7];
    for star in 0..400u32 {
        for station in 0..3u32 {
            let seed = worldgen::rng::mix(data::DEFAULT_SEED ^ 0x_4D49_5353_494F_4E)
                ^ worldgen::rng::mix(u64::from(star))
                ^ worldgen::rng::mix(u64::from(station).wrapping_add(0x_5354));
            let roll = worldgen::rng::Rng::new(seed).below(Mission::ATTACKS.len() as u32);
            counts[roll as usize] += 1;
        }
    }
    let total: u32 = counts.iter().sum();
    for (i, &c) in counts.iter().enumerate() {
        let share = c as f64 / total as f64;
        assert!(
            (0.10..0.19).contains(&share),
            "{:?}: {share:.3}",
            Mission::ATTACKS[i]
        );
    }
}

#[test]
fn the_overseer_flees_hurt_and_out_of_his_airlock_takes_the_bounty() {
    let (mut world, station) = mission_world(Mission::Overseer);
    let Some(Objective::Overseer(o)) = objective(&world, station) else {
        panic!("an Overseer");
    };
    let body = o.body.expect("he is laid") as usize;
    let area = world.area_now().0.min(3);
    let max = world
        .residents
        .as_ref()
        .unwrap()
        .aboard
        .room
        .max_health(body);
    assert!(
        (max - data::OVERSEER_HEALTH[area]).abs() < 1.0,
        "his health is the area's: {max}"
    );
    // The clear waits on him.
    for _ in 0..300 {
        wreck_them_all(&mut world);
        world.step(&[]);
    }
    assert!(
        !world.infestation(station).unwrap().cleared,
        "not while he lives"
    );
    world.run.pending_bounty = 5_000;
    world.hurt_the_overseer_for_probe();
    let mut fled = false;
    for _ in 0..10 {
        if world
            .step(&[])
            .iter()
            .any(|e| matches!(e, WorldEvent::Objective { what: 10, .. }))
        {
            fled = true;
            break;
        }
    }
    assert!(fled, "hurt, he runs");
    // Put at his airlock: out, and the bounty with him.
    let exit = {
        let site = world.station(station).unwrap();
        let port = crate::droid::airlocks(&site.design)[o.airlock as usize];
        let residents = world.residents.as_ref().unwrap();
        residents.aboard.to_room(
            worldgen::math::dvec2(port.centre.0, port.centre.1).sub(
                worldgen::math::dvec2(port.outward.0 as f64, port.outward.1 as f64)
                    .scale(shipdesign::TILE as f64),
            ),
        )
    };
    world
        .residents
        .as_mut()
        .unwrap()
        .aboard
        .room
        .put_for_probe(body, exit);
    let mut out = false;
    for _ in 0..10 {
        if world
            .step(&[])
            .iter()
            .any(|e| matches!(e, WorldEvent::Objective { what: 11, .. }))
        {
            out = true;
            break;
        }
    }
    assert!(out, "out of his airlock");
    let Some(Objective::Overseer(o)) = objective(&world, station) else {
        panic!();
    };
    assert_eq!(o.phase, OverseerPhase::Escaped);
    assert_eq!(world.run.pending_bounty, 0, "the bounty went with him");
}

/// The player's: "the overseer should run away from the players" — the
/// crew stood two tiles off him (unarmed, and he kept whole, so he never
/// flees for his airlock), he picks a tile farther from them and walks
/// off at his running pace; the crew gone back aboard, he has none.
#[test]
fn the_overseer_runs_from_the_crew_and_stops_once_they_are_gone() {
    let (mut world, station) = mission_world(Mission::Overseer);
    let Some(Objective::Overseer(o)) = objective(&world, station) else {
        panic!("an Overseer");
    };
    assert_eq!(o.away, None, "nobody near him yet");
    let body = o.body.expect("he is laid") as usize;
    let his = |world: &World| {
        let r = world.residents.as_ref().unwrap();
        r.aboard.to_design(r.aboard.room.body_pos(body))
    };
    let t = shipdesign::TILE as f64;
    let start = his(&world);
    let crew_at = start.add(worldgen::math::dvec2(2.0 * t, 0.0));
    let crew = world.aboard.room.crew_count() as usize;
    let aboard: Vec<_> = (0..crew).map(|w| world.aboard.room.bim_pos(w)).collect();
    for w in 0..crew {
        let gear = world.aboard.room.gear(w);
        world.aboard.room.issue(
            w,
            Gear {
                weapon: None,
                ..gear
            },
        );
    }
    let mut away = None;
    for _ in 0..600 {
        let at = on_deck(&world, crew_at);
        for w in 0..crew {
            world.aboard.room.put_for_probe(w, at);
        }
        wreck_them_all(&mut world);
        world
            .residents
            .as_mut()
            .unwrap()
            .aboard
            .room
            .heal_percent(body, 100.0);
        world.step(&[]);
        let Some(Objective::Overseer(o)) = objective(&world, station) else {
            panic!();
        };
        assert_eq!(o.phase, OverseerPhase::Office);
        away = away.or(o.away);
    }
    let away = away.expect("a tile to run to");
    let mid = worldgen::math::dvec2((away.0 as f64 + 0.5) * t, (away.1 as f64 + 0.5) * t);
    let was = start.sub(crew_at).length();
    assert!(
        mid.sub(crew_at).length() > was,
        "away from them: {away:?} {mid:?} from {start:?}"
    );
    let now = his(&world).sub(crew_at).length();
    assert!(now > was + t, "he ran: {was:.0} then {now:.0}");
    // The crew put back where they stood: no tile to run to after his
    // next think.
    for _ in 0..=data::OVERSEER_RETHINK_STEPS {
        for (w, &at) in aboard.iter().enumerate() {
            world.aboard.room.put_for_probe(w, at);
        }
        world.step(&[]);
    }
    let Some(Objective::Overseer(o)) = objective(&world, station) else {
        panic!();
    };
    assert_eq!(o.away, None, "nobody near: back to his desk");
}

#[test]
fn a_heist_s_terminal_is_taken_by_hand_and_the_site_held_until_all_are() {
    let (mut world, station) = mission_world(Mission::Heist);
    let Some(Objective::Heist(h)) = objective(&world, station) else {
        panic!("a heist");
    };
    assert_eq!(h.terminals.len(), data::HEIST_TERMINALS as usize);
    let at = mark_at(&world, |k| {
        matches!(k, crate::MarkKind::Terminal { taken: false })
    });
    let t = shipdesign::TILE as f32;
    world.aboard.room.stand_at(0, at + bims::math::vec2(t, 0.0));
    world.step(&[Command::Interact { slot: 0 }]);
    let mut taken = false;
    for _ in 0..(data::HACK_SECONDS * 60 + 120) {
        wreck_them_all(&mut world);
        if world
            .step(&[])
            .iter()
            .any(|e| matches!(e, WorldEvent::Objective { what: 1, .. }))
        {
            taken = true;
            break;
        }
    }
    assert!(taken, "a terminal taken by hand");
    for _ in 0..300 {
        wreck_them_all(&mut world);
        world.step(&[]);
    }
    assert!(
        !world.infestation(station).unwrap().cleared,
        "not while a terminal is left"
    );
    // Every one taken: the deck cleared is the site cleared.
    if let Some(Objective::Heist(h)) = world
        .infested
        .iter_mut()
        .find(|it| it.station == station)
        .and_then(|it| it.objective.as_mut())
    {
        h.taken.iter_mut().for_each(|t| *t = true);
    }
    for _ in 0..600 {
        wreck_them_all(&mut world);
        world.step(&[]);
        if world.infestation(station).unwrap().cleared {
            break;
        }
    }
    assert!(world.infestation(station).unwrap().cleared);
}

#[test]
fn a_prison_s_cell_is_cut_open_and_its_prisoners_armed() {
    let (mut world, station) = mission_world(Mission::Prison);
    let Some(Objective::Prison(p)) = objective(&world, station) else {
        panic!("a prison");
    };
    assert_eq!(p.prisoners.len(), data::PRISONERS as usize);
    for &w in &p.prisoners {
        assert!(world.is_captive(w), "held");
        assert!(
            world.aboard.room.gear(w as usize).weapon.is_none(),
            "unarmed"
        );
    }
    let bots_before = world.crew_bots();
    assert!(p.door.is_some(), "a cell with a door");
    let at = mark_at(&world, |k| {
        matches!(k, crate::MarkKind::CellDoor { open: false })
    });
    world.aboard.room.stand_at(0, at);
    world.step(&[Command::Interact { slot: 0 }]);
    let mut open = false;
    for _ in 0..(data::CUT_SECONDS * 60 + 120) {
        wreck_them_all(&mut world);
        if world
            .step(&[])
            .iter()
            .any(|e| matches!(e, WorldEvent::Objective { what: 9, .. }))
        {
            open = true;
            break;
        }
    }
    assert!(open, "cut open");
    for &w in &p.prisoners {
        assert!(!world.is_captive(w));
        assert_eq!(
            world.aboard.room.gear(w as usize).weapon.map(|w| w.kind),
            Some(WeaponKind::LaserPistol)
        );
    }
    assert_eq!(world.crew_bots(), bots_before + data::PRISONERS);
}

#[test]
fn a_fuel_run_s_drum_goes_up_in_the_arms_and_another_goes_in() {
    let (mut world, station) = mission_world(Mission::FuelRun);
    let drum = mark_at(&world, |k| {
        matches!(k, crate::MarkKind::Drum { carried: false })
    });
    world.aboard.room.stand_at(0, drum);
    world.step(&[Command::Interact { slot: 0 }]);
    assert!(world.carries_a_load(0), "taken up");
    assert!(world.skill_of(0).holds_fire, "no shooting with it");
    // A hit, and it goes up.
    world.aboard.room.note_hit_for_probe(0);
    let blew = world
        .step(&[])
        .iter()
        .any(|e| matches!(e, WorldEvent::Objective { what: 6, .. }));
    assert!(blew, "a hit in the arms");
    let Some(Objective::FuelRun(f)) = objective(&world, station) else {
        panic!();
    };
    assert!(f.drums.iter().any(|d| d.state == LoadState::Lost));
    assert!(f.restock_at.is_some(), "a fresh drum is coming");
    // Another, carried to the reactor.
    world.aboard.room.restore_health(0);
    let drum = mark_at(&world, |k| {
        matches!(k, crate::MarkKind::Drum { carried: false })
    });
    world.aboard.room.stand_at(0, drum);
    world.step(&[Command::Interact { slot: 0 }]);
    let reactor = mark_at(&world, |k| matches!(k, crate::MarkKind::Reactor { .. }));
    world.aboard.room.stand_at(
        0,
        reactor + bims::math::vec2(2.0 * shipdesign::TILE as f32, 0.0),
    );
    let mut put_in = false;
    for _ in 0..5 {
        if world
            .step(&[])
            .iter()
            .any(|e| matches!(e, WorldEvent::Objective { what: 7, n: 1, .. }))
        {
            put_in = true;
            break;
        }
    }
    assert!(put_in, "in the reactor");
}

#[test]
fn a_salvage_crate_taken_brings_a_wave_and_aboard_pays() {
    let (mut world, station) = mission_world(Mission::Salvage);
    let crate_at = mark_at(&world, |k| {
        matches!(k, crate::MarkKind::Crate { carried: false })
    });
    world.aboard.room.stand_at(0, crate_at);
    let events = world.step(&[Command::Interact { slot: 0 }]);
    assert!(
        events
            .iter()
            .any(|e| matches!(e, WorldEvent::DroidReinforcements { .. })),
        "a wave for it: {events:?}"
    );
    let money = world.money;
    let gangway = world.aboard.gangway.unwrap();
    world
        .aboard
        .room
        .stand_at(0, bims::math::vec2(gangway.x as f32, gangway.y as f32));
    let mut home = false;
    for _ in 0..5 {
        if world
            .step(&[])
            .iter()
            .any(|e| matches!(e, WorldEvent::Objective { what: 4, n: 1, .. }))
        {
            home = true;
            break;
        }
    }
    assert!(home, "aboard");
    assert!(world.money > money, "paid into the takings");
    let Some(Objective::Salvage(s)) = objective(&world, station) else {
        panic!();
    };
    assert_eq!(s.home, 1);
}

/// What the game setup's Dev tab does (`set_mission_for_probe`), in a run
/// on the floor: every mission forced is laid at the first site of its
/// kind the crew travel to, the station laid out for it on arrival.
#[test]
fn a_mission_forced_as_the_dev_tab_does_is_laid_where_the_floor_takes_the_crew() {
    use crate::run::{Site, SiteKind};
    for mission in [
        Mission::Overseer,
        Mission::Heist,
        Mission::Prison,
        Mission::FuelRun,
        Mission::Salvage,
        Mission::Sabotage,
        Mission::Nests,
        Mission::Bombs,
        Mission::Doors,
        Mission::Chief,
        Mission::Breaches,
        Mission::Evacuation,
    ] {
        let want = if mission.is_attack() {
            SiteKind::Attack
        } else {
            SiteKind::Defend
        };
        let galaxy = worldgen::Galaxy::new(data::DEFAULT_SEED, GalaxyType::SpiralTwoArm);
        let (star, station) = crate::spawn(&galaxy).unwrap();
        let mut world = World::start(
            shipdesign::fixture::flyer(2),
            REFERENCE_MONEY,
            1,
            data::DEFAULT_SEED,
            GalaxyType::SpiralTwoArm,
            star,
            station,
        )
        .unwrap();
        world.set_machines_only_for_probe();
        world.set_floor(true);
        world.set_mission_for_probe(Some(mission));
        let mut laid = false;
        'climb: for _ in 0..12 {
            if world.in_mission() {
                world.leave_for_probe();
            }
            let next: Vec<Site> = world.floor_next();
            let fits = |w: &World, s: &Site| {
                w.travel_quote(*s).is_ok_and(|q| {
                    q.kind == want
                        && !q.elite
                        && (crate::surface_body(s.station).is_none() || mission == Mission::Chief)
                })
            };
            let Some(&site) = next.iter().find(|s| fits(&world, s)).or(next.first()) else {
                break;
            };
            let found = fits(&world, &site);
            world.step(&[Command::Propose {
                slot: 0,
                star: site.star,
                station: site.station,
            }]);
            if !found {
                continue;
            }
            assert_eq!(world.mission_here(site.station), mission, "{mission:?}");
            for _ in 0..120 {
                world.step(&[]);
                let begun = world.objective_look().is_some()
                    || world.sabotage_look().is_some()
                    || world.nests_standing().is_some()
                    || world.breaches_open().is_some()
                    || world.evacuation_look().is_some();
                if begun {
                    laid = true;
                    break 'climb;
                }
            }
            panic!("{mission:?}: nothing laid at {site:?}");
        }
        assert!(laid, "{mission:?}: no site of its kind on the way up");
        if let Some(feature) = crate::world::feature_of(mission) {
            let id = world.ship.state.station().unwrap();
            assert_eq!(
                world
                    .station(id)
                    .unwrap()
                    .fitted
                    .as_ref()
                    .map(|f| f.feature),
                Some(feature),
                "{mission:?}: laid out for it"
            );
        }
    }
}

/// What the Dev tab does at Start with a mission picked (October 2026,
/// the player's: "if i pick anything in the dev it should do that
/// mission"): the run's opening dock — a defence — taken by the machines
/// for an attack's, the mission forced, the dock laid out for it; and the
/// first steps begin that very mission there.
#[test]
fn a_mission_picked_in_the_dev_tab_is_the_run_s_first_fight() {
    for mission in [
        Mission::Overseer,
        Mission::Heist,
        Mission::Prison,
        Mission::FuelRun,
        Mission::Salvage,
        Mission::Sabotage,
        Mission::Nests,
        Mission::Bombs,
        Mission::Doors,
        Mission::Chief,
        Mission::Breaches,
        Mission::Evacuation,
    ] {
        let galaxy = worldgen::Galaxy::new(data::DEFAULT_SEED, GalaxyType::SpiralTwoArm);
        let (star, station) = crate::spawn(&galaxy).unwrap();
        let mut world = World::start(
            shipdesign::fixture::flyer(2),
            REFERENCE_MONEY,
            1,
            data::DEFAULT_SEED,
            GalaxyType::SpiralTwoArm,
            star,
            station,
        )
        .unwrap();
        world.set_floor(true);
        let id = world.ship.state.station().unwrap();
        // The Dev tab's apply.
        if mission.is_attack() {
            world.infest(id);
        }
        world.set_mission_for_probe(Some(mission));
        world.fit_dock_for_probe();
        assert_eq!(world.mission_here(id), mission, "{mission:?}");
        let mut begun = false;
        for _ in 0..400 {
            world.step(&[]);
            begun = world.objective_look().is_some()
                || world.sabotage_look().is_some()
                || world.nests_standing().is_some()
                || world.breaches_open().is_some()
                || world.evacuation_look().is_some();
            if begun {
                break;
            }
        }
        assert!(begun, "{mission:?}: the first fight is the mission");
        if let Some(feature) = crate::world::feature_of(mission) {
            assert_eq!(
                world
                    .station(id)
                    .unwrap()
                    .fitted
                    .as_ref()
                    .map(|f| f.feature),
                Some(feature),
                "{mission:?}: laid out for it"
            );
        }
    }
}
