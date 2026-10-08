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

// --- The budget (October 2026) ------------------------------------------------

/// The objective's clock made due this very step, for a test that wants
/// its landings without the wait.
fn due_now(world: &mut World, station: u32) {
    let o = world
        .infested
        .iter_mut()
        .find(|it| it.station == station)
        .and_then(|it| it.objective.as_mut());
    match o {
        Some(Objective::Overseer(o)) => o.next_wave = 0,
        Some(Objective::Heist(h)) => h.next_wave = 0,
        Some(Objective::FuelRun(f)) => f.next_wave = 0,
        Some(Objective::Prison(p)) if p.next_wave.is_some() => p.next_wave = Some(0),
        _ => {}
    }
}

/// How many bodies the residents' room holds.
fn bodies(world: &World) -> u32 {
    world.residents.as_ref().unwrap().aboard.room.body_count()
}

fn landed(events: &[WorldEvent]) -> bool {
    events
        .iter()
        .any(|e| matches!(e, WorldEvent::DroidReinforcements { .. }))
}

/// The clocked attacks spend their budget and then trickle, unpaid
/// (October 2026, `run::Budget`): the garrison is the first slice, the
/// clock's landings take the rest to the body over the par waves, and
/// after it every landing is one a player, every body of it unpaid.
#[test]
fn the_clocked_attacks_spend_their_budget_and_then_trickle_unpaid() {
    for mission in [Mission::Overseer, Mission::Heist, Mission::FuelRun] {
        let (mut world, station) = mission_world(mission);
        let budget = world.run.budget.expect("a clocked attack has its budget");
        assert_eq!(budget.laid, budget.garrison, "{mission:?}: the garrison");
        let players = world.players();
        let mut slices: Vec<u32> = Vec::new();
        let mut trickles = 0;
        for _ in 0..40 {
            if trickles == 2 {
                break;
            }
            wreck_them_all(&mut world);
            due_now(&mut world, station);
            let (before, laid) = (bodies(&world), world.run.budget.unwrap().laid);
            if !landed(&world.step(&[])) {
                continue;
            }
            let after = bodies(&world);
            let room = &world.residents.as_ref().unwrap().aboard.room;
            let unpaid: Vec<bool> = (before..after)
                .map(|i| room.is_unpaid(i as usize))
                .collect();
            let now = world.run.budget.unwrap();
            if now.laid == laid {
                assert_eq!(laid, now.planned, "{mission:?}: a trickle once spent");
                assert_eq!(after - before, players, "{mission:?}: one a player");
                assert!(unpaid.iter().all(|&u| u), "{mission:?}: unpaid");
                trickles += 1;
            } else {
                assert_eq!(trickles, 0, "{mission:?}: nothing paid after the trickle");
                assert!(unpaid.iter().all(|&u| !u), "{mission:?}: the budget's pay");
                slices.push(now.laid - laid);
            }
        }
        assert_eq!(
            trickles, 2,
            "{mission:?}: the trickle came after {slices:?}"
        );
        assert!(
            slices.len() < data::OBJECTIVE_WAVES as usize,
            "{mission:?}: the garrison and three at the most: {slices:?}"
        );
        assert_eq!(
            budget.garrison + slices.iter().sum::<u32>(),
            budget.planned,
            "{mission:?}: the slices spend the budget to the body: {slices:?}"
        );
    }
}

/// The Overseer is no wave: on a wave of one, him standing does not hold
/// his own clock's wave off (he did, for good).
#[test]
fn the_overseer_does_not_hold_his_own_clock_wave_off_on_a_wave_of_one() {
    let (mut world, station) = mission_world(Mission::Overseer);
    world.set_droid_wave_for_probe(1);
    let mut came = false;
    for _ in 0..5 {
        wreck_them_all(&mut world);
        due_now(&mut world, station);
        if landed(&world.step(&[])) {
            came = true;
            break;
        }
    }
    let Some(Objective::Overseer(o)) = objective(&world, station) else {
        panic!("an Overseer");
    };
    assert_eq!(o.phase, OverseerPhase::Office, "he stands");
    assert!(came, "his clock's wave landed with only him standing");
}

/// A salvage sweep counts crates, not pick-ups (October 2026): a crate
/// dropped and taken again brings nothing, and the crates between them
/// spend what the garrison left of the budget to the body.
#[test]
fn a_salvage_crate_taken_twice_brings_one_wave_and_the_crates_spend_the_budget() {
    let (mut world, station) = mission_world(Mission::Salvage);
    let budget = world.run.budget.expect("a salvage sweep has its budget");
    let crates = match objective(&world, station) {
        Some(Objective::Salvage(s)) => s.crates.len(),
        _ => panic!("a salvage sweep"),
    };
    assert!(crates > 1, "crates to take: {crates}");
    let crate_at = |world: &World, i: usize| match objective(world, station) {
        Some(Objective::Salvage(s)) => {
            let at = s.crates[i].at;
            bims::math::vec2(at.0 as f32, at.1 as f32)
        }
        _ => panic!("a salvage sweep"),
    };
    // The first crate: taken, dropped, taken again — one wave.
    world.aboard.room.stand_at(0, crate_at(&world, 0));
    let laid = world.run.budget.unwrap().laid;
    assert!(landed(&world.step(&[Command::Interact { slot: 0 }])));
    let first = world.run.budget.unwrap().laid;
    assert!(first > laid, "the first crate's wave is the budget's");
    wreck_them_all(&mut world);
    world.step(&[Command::Interact { slot: 0 }]);
    assert!(!world.carries_a_load(0), "put down");
    wreck_them_all(&mut world);
    let again = world.step(&[Command::Interact { slot: 0 }]);
    assert!(world.carries_a_load(0), "taken up again");
    assert!(
        !landed(&again),
        "a crate taken again brings no wave: {again:?}"
    );
    assert_eq!(world.run.budget.unwrap().laid, first, "and takes nothing");
    // Home with it, and every other crate taken up in turn.
    let home = |world: &mut World, i: usize| {
        if let Some(Objective::Salvage(s)) = world
            .infested
            .iter_mut()
            .find(|it| it.station == station)
            .and_then(|it| it.objective.as_mut())
        {
            s.crates[i].state = LoadState::Home;
            s.crates[i].carrier = None;
        }
    };
    home(&mut world, 0);
    for i in 1..crates {
        wreck_them_all(&mut world);
        world.aboard.room.restore_health(0);
        world.aboard.room.stand_at(0, crate_at(&world, i));
        let events = world.step(&[Command::Interact { slot: 0 }]);
        assert!(landed(&events), "crate {i} brings its wave");
        home(&mut world, i);
    }
    let spent = world.run.budget.unwrap();
    assert_eq!(spent.laid, budget.planned, "the crates spend the budget");
    assert_eq!(
        spent.landings,
        1 + crates as u32,
        "the garrison and a crate each"
    );
}

/// A prison break's alarm (October 2026): the cut brings a wave at once
/// and more on the clock while a prisoner alive is not aboard; every one
/// of them aboard, it is over, and only then does the deck clear the site.
#[test]
fn a_prison_s_alarm_comes_on_the_cut_and_stops_once_the_prisoners_are_aboard() {
    let (mut world, station) = mission_world(Mission::Prison);
    let Some(Objective::Prison(p)) = objective(&world, station) else {
        panic!("a prison");
    };
    assert_eq!(p.next_wave, None, "no alarm before the cut");
    let at = mark_at(&world, |k| {
        matches!(k, crate::MarkKind::CellDoor { open: false })
    });
    world.aboard.room.stand_at(0, at);
    world.step(&[Command::Interact { slot: 0 }]);
    let mut alarm = None;
    for _ in 0..(data::CUT_SECONDS * 60 + 120) {
        wreck_them_all(&mut world);
        let events = world.step(&[]);
        if events
            .iter()
            .any(|e| matches!(e, WorldEvent::Objective { what: 9, .. }))
        {
            alarm = Some(landed(&events));
            break;
        }
    }
    assert_eq!(alarm, Some(true), "the cut, and a wave with it");
    // The alarm's clock brings another while they are out.
    let mut again = false;
    for _ in 0..5 {
        wreck_them_all(&mut world);
        due_now(&mut world, station);
        if landed(&world.step(&[])) {
            again = true;
            break;
        }
    }
    assert!(again, "the alarm's next wave");
    for _ in 0..300 {
        wreck_them_all(&mut world);
        world.step(&[]);
    }
    assert!(
        !world.infestation(station).unwrap().cleared,
        "not while they are out"
    );
    // Every one aboard: the alarm is over, and the deck clear is the
    // site clear.
    let gangway = world.aboard.gangway.unwrap();
    let gangway = bims::math::vec2(gangway.x as f32, gangway.y as f32);
    let Some(Objective::Prison(p)) = objective(&world, station) else {
        panic!();
    };
    world.aboard.room.stand_at(0, gangway);
    for &w in &p.prisoners {
        world.aboard.room.stand_at(w as usize, gangway);
    }
    wreck_them_all(&mut world);
    world.step(&[]);
    let Some(Objective::Prison(p)) = objective(&world, station) else {
        panic!();
    };
    assert_eq!(p.next_wave, None, "the alarm is over");
    let mut cleared = false;
    for _ in 0..600 {
        wreck_them_all(&mut world);
        world.step(&[]);
        if world.infestation(station).unwrap().cleared {
            cleared = true;
            break;
        }
    }
    assert!(cleared, "the site clears once they are aboard");
}

// --- every mission an end (October 2026) -------------------------------------
//
// The player's: "prison for example just sets the prisoners free and after
// that nothing ever happens. All missions need a lost state and a won
// state". The objective done is the mission won the step it is, whatever
// still stands; one that can no longer be done is the mission failed.

/// A prison break's cell cut open by crew member 0, the machines wrecked
/// meanwhile; the prisoners.
fn cut_the_cell(world: &mut World, station: u32) -> Vec<u32> {
    let at = mark_at(world, |k| {
        matches!(k, crate::MarkKind::CellDoor { open: false })
    });
    world.aboard.room.stand_at(0, at);
    world.step(&[Command::Interact { slot: 0 }]);
    for _ in 0..(data::CUT_SECONDS * 60 + 120) {
        wreck_them_all(world);
        if world
            .step(&[])
            .iter()
            .any(|e| matches!(e, WorldEvent::Objective { what: 9, .. }))
        {
            let Some(Objective::Prison(p)) = objective(world, station) else {
                panic!("a prison");
            };
            return p.prisoners;
        }
    }
    panic!("the cell never opened");
}

/// The ship's gangway on the crew's deck.
fn gangway(world: &World) -> bims::math::Vec2 {
    let at = world.aboard.gangway.expect("the ship's gangway");
    bims::math::vec2(at.x as f32, at.y as f32)
}

/// Every prisoner alive aboard is the prison break won that very step —
/// the alarm's wave still standing, frozen where it stands — and *Back to
/// ship* pressed with them walking aboard takes the crew home won, not
/// from a site left uncleared.
#[test]
fn a_prison_break_is_won_the_step_its_prisoners_are_aboard_whatever_stands() {
    let (mut world, station) = mission_world(Mission::Prison);
    let prisoners = cut_the_cell(&mut world, station);
    // The alarm's wave, at once.
    world.step(&[]);
    assert!(world.droids_standing() > 0, "the alarm's wave stands");
    let look = world.objective_look().unwrap();
    assert_eq!(
        (look.done, look.total),
        (0, data::PRISONERS),
        "0 of 3 aboard"
    );
    let home = gangway(&world);
    world.aboard.room.stand_at(0, home);
    for &w in &prisoners[1..] {
        world.aboard.room.stand_at(w as usize, home);
    }
    world.step(&[]);
    assert!(!world.infestation(station).unwrap().cleared, "one is out");
    assert_eq!(world.objective_look().unwrap().done, data::PRISONERS - 1);
    // The last one walks aboard with *Back to ship* pressed.
    world.aboard.room.stand_at(prisoners[0] as usize, home);
    let events = world.step(&[Command::Return { slot: 0 }]);
    assert!(
        events.iter().any(
            |e| matches!(e, WorldEvent::LeftSite { station: s, cleared: true } if *s == station)
        ),
        "left won: {events:?}"
    );
    assert!(world.infestation(station).unwrap().cleared);
    assert!(!world.in_mission());
    // The freed prisoners alive stay on as crew.
    assert_eq!(world.crew_bots() as usize >= prisoners.len(), true);
}

/// The passage between the two collars on the crew's deck: inside the
/// ship by the departure's rule ([`World::inside_ship`]), on no tile of
/// the ship's own frame.
fn passage(world: &World) -> bims::math::Vec2 {
    let ship = &world.ship.design;
    let port = shipdesign::dock::port(ship).expect("a port");
    let (x, y) = ship.part(port.part_id).expect("the airlock").tiles()[0];
    let tile = (x as i32 + port.outward.0, y as i32 + port.outward.1);
    assert_eq!(
        ship.grid().get(shipdesign::Layer::Structure, tile),
        0,
        "beyond the hull"
    );
    let t = shipdesign::TILE as f64;
    let at = |i: i32, o: f64| (i as f64 + 0.5) * t + o;
    bims::math::vec2(
        at(tile.0, world.aboard.offset.x) as f32,
        at(tile.1, world.aboard.offset.y) as f32,
    )
}

/// A prisoner in the passage is aboard as the departure counts it: the
/// last one stepping into it with *Back to ship* pressed is the mission
/// won, not the ship leaving a step before the win.
#[test]
fn a_prisoner_in_the_passage_is_aboard_for_the_win() {
    let (mut world, station) = mission_world(Mission::Prison);
    let prisoners = cut_the_cell(&mut world, station);
    let home = gangway(&world);
    world.aboard.room.stand_at(0, home);
    for &w in &prisoners[1..] {
        world.aboard.room.stand_at(w as usize, home);
    }
    world.step(&[]);
    let last = prisoners[0];
    world.aboard.room.stand_at(last as usize, passage(&world));
    assert!(world.inside_ship(last), "the passage is inside the ship");
    let events = world.step(&[Command::Return { slot: 0 }]);
    assert!(
        events.iter().any(
            |e| matches!(e, WorldEvent::LeftSite { station: s, cleared: true } if *s == station)
        ),
        "left won: {events:?}"
    );
}

/// Won on the deck, *Back to ship* not pressed: the site cleared with the
/// machines still standing, and the deck frozen on them.
#[test]
fn a_prison_break_won_freezes_the_deck_on_what_still_stands() {
    let (mut world, station) = mission_world(Mission::Prison);
    let prisoners = cut_the_cell(&mut world, station);
    world.step(&[]);
    let standing = world.droids_standing();
    assert!(standing > 0);
    let home = gangway(&world);
    for &w in &prisoners {
        world.aboard.room.stand_at(w as usize, home);
    }
    world.step(&[]);
    assert!(world.infestation(station).unwrap().cleared);
    assert!(world.fight_over());
    assert_eq!(world.mission_failed(), None);
    world.step(&[]);
    assert_eq!(world.droids_standing(), standing, "frozen, not destroyed");
}

/// Every prisoner dead is the prison break failed: no win however clear
/// the deck, the deck frozen, and the crew go home from a site uncleared.
#[test]
fn a_prison_break_is_failed_with_every_prisoner_dead() {
    let (mut world, station) = mission_world(Mission::Prison);
    let prisoners = cut_the_cell(&mut world, station);
    for &w in &prisoners {
        world.aboard.room.kill_now(w as usize);
    }
    wreck_them_all(&mut world);
    world.step(&[]);
    assert_eq!(
        world.mission_failed(),
        Some(crate::run::MissionFailure::Prisoners)
    );
    assert!(world.fight_over(), "the deck is frozen");
    for _ in 0..120 {
        wreck_them_all(&mut world);
        world.step(&[]);
    }
    assert!(!world.infestation(station).unwrap().cleared, "never won");
    assert!(!world.lost, "the run goes on");
    let events = world.step(&[Command::Return { slot: 0 }]);
    assert!(
        events
            .iter()
            .any(|e| matches!(e, WorldEvent::LeftSite { cleared: false, .. })),
        "{events:?}"
    );
}

/// The Overseer out of his airlock is the mission failed (it was the site
/// cleared once the deck was, the bounty gone).
#[test]
fn the_overseer_out_of_his_airlock_is_the_mission_failed() {
    let (mut world, station) = mission_world(Mission::Overseer);
    let Some(Objective::Overseer(mut o)) = objective(&world, station) else {
        panic!("an Overseer");
    };
    o.phase = OverseerPhase::Escaped;
    if let Some(it) = world.infested.iter_mut().find(|it| it.station == station) {
        it.objective = Some(Objective::Overseer(o));
    }
    wreck_them_all(&mut world);
    world.step(&[]);
    assert_eq!(
        world.mission_failed(),
        Some(crate::run::MissionFailure::Overseer)
    );
    assert!(world.fight_over());
    assert!(!world.infestation(station).unwrap().cleared);
}

/// The Overseer down is the mission won at once, his waves standing.
#[test]
fn the_overseer_down_is_the_mission_won() {
    let (mut world, station) = mission_world(Mission::Overseer);
    let Some(Objective::Overseer(o)) = objective(&world, station) else {
        panic!("an Overseer");
    };
    let body = o.body.expect("he is laid") as usize;
    world.residents.as_mut().unwrap().aboard.room.kill_now(body);
    let events = world.step(&[]);
    assert!(
        events
            .iter()
            .any(|e| matches!(e, WorldEvent::Objective { what: 12, .. }))
    );
    assert!(world.infestation(station).unwrap().cleared);
    assert!(world.fight_over() && world.mission_failed().is_none());
}

/// Every terminal taken is the heist won that step, the garrison still
/// standing wherever it waits.
#[test]
fn a_heist_is_won_the_step_its_last_terminal_is_taken() {
    let (mut world, station) = mission_world(Mission::Heist);
    assert!(world.droids_standing() > 0, "the garrison");
    if let Some(Objective::Heist(h)) = world
        .infested
        .iter_mut()
        .find(|it| it.station == station)
        .and_then(|it| it.objective.as_mut())
    {
        h.taken.iter_mut().for_each(|t| *t = true);
    }
    world.step(&[]);
    assert!(world.infestation(station).unwrap().cleared);
    assert!(world.droids_standing() > 0, "won with them standing");
}

/// Every crate home is the salvage sweep won.
#[test]
fn a_salvage_sweep_is_won_with_every_crate_home() {
    let (mut world, station) = mission_world(Mission::Salvage);
    if let Some(Objective::Salvage(s)) = world
        .infested
        .iter_mut()
        .find(|it| it.station == station)
        .and_then(|it| it.objective.as_mut())
    {
        for c in &mut s.crates {
            c.state = LoadState::Home;
        }
    }
    world.step(&[]);
    assert!(world.infestation(station).unwrap().cleared);
}

/// The last crate carried into the ship with *Back to ship* pressed is
/// the sweep won the step its carrier is inside — the step's own salvage
/// pass read where the carrier stood before the rooms stepped, and the
/// ship left before the crate was home.
#[test]
fn the_last_crate_carried_aboard_wins_the_sweep_the_step_it_is_inside() {
    let (mut world, station) = mission_world(Mission::Salvage);
    let crates = match objective(&world, station) {
        Some(Objective::Salvage(s)) => s.crates.len() as u32,
        _ => panic!("a salvage sweep"),
    };
    if let Some(Objective::Salvage(s)) = world
        .infested
        .iter_mut()
        .find(|it| it.station == station)
        .and_then(|it| it.objective.as_mut())
    {
        for c in &mut s.crates[1..] {
            c.state = LoadState::Home;
        }
        s.home = crates - 1;
        s.crates[0].state = LoadState::Carried;
        s.crates[0].carrier = Some(0);
        s.crates[0].picked = true;
    }
    // Two tiles out from the passage, on the station's side.
    let ship = &world.ship.design;
    let out = shipdesign::dock::port(ship).expect("a port").outward;
    let t = shipdesign::TILE as f32;
    let p = passage(&world);
    let start = bims::math::vec2(p.x + 2.0 * t * out.0 as f32, p.y + 2.0 * t * out.1 as f32);
    world.aboard.room.stand_at(0, start);
    assert!(!world.inside_ship(0));
    let mut commands = vec![Command::Return { slot: 0 }];
    for _ in 0..600 {
        wreck_them_all(&mut world);
        let events = world.step(&commands);
        commands.clear();
        if world.in_mission() && !world.inside_ship(0) {
            continue;
        }
        assert!(
            events.iter().any(
                |e| matches!(e, WorldEvent::LeftSite { station: s, cleared: true } if *s == station)
            ),
            "left won the step the carrier was inside: {events:?}"
        );
        return;
    }
    panic!("the carrier never came aboard");
}

/// The reactor critical is the fuel run won; the drums lost while the
/// depot's clock runs all stood again (it was one, the rest never), while
/// its stock lasts; and too few drums left for the reactor is the mission
/// failed.
#[test]
fn a_fuel_run_s_lost_drums_come_back_while_the_stock_lasts_and_then_it_fails() {
    let (mut world, station) = mission_world(Mission::FuelRun);
    let edit = |world: &mut World, f: &dyn Fn(&mut crate::objective::FuelRun)| {
        if let Some(Objective::FuelRun(r)) = world
            .infested
            .iter_mut()
            .find(|it| it.station == station)
            .and_then(|it| it.objective.as_mut())
        {
            f(r);
        }
    };
    let fuel = |world: &World| match objective(world, station) {
        Some(Objective::FuelRun(f)) => f,
        _ => panic!("a fuel run"),
    };
    assert_eq!(fuel(&world).drums.len(), data::FUEL_DRUMS as usize);
    // Two lost on one clock: both stood again.
    let now = world.run.mission_steps;
    edit(&mut world, &|f| {
        f.drums[0].state = LoadState::Lost;
        f.drums[1].state = LoadState::Lost;
        f.restock_at = Some(now + 1);
    });
    for _ in 0..3 {
        wreck_them_all(&mut world);
        world.step(&[]);
    }
    let f = fuel(&world);
    let lying = f
        .drums
        .iter()
        .filter(|d| d.state == LoadState::Lying)
        .count();
    assert_eq!(lying, data::FUEL_DRUMS as usize, "both stood again");
    assert_eq!(f.spares, data::FUEL_SPARES - 2);
    assert_eq!(world.mission_failed(), None);
    // The stock spent, two more lost: two left of the three wanted.
    edit(&mut world, &|f| {
        f.drums[2].state = LoadState::Lost;
        f.drums[3].state = LoadState::Lost;
    });
    wreck_them_all(&mut world);
    world.step(&[]);
    assert_eq!(
        world.mission_failed(),
        Some(crate::run::MissionFailure::Fuel)
    );
    assert!(world.fight_over());
    assert!(!world.infestation(station).unwrap().cleared);
}

/// The reactor critical is the fuel run won at once.
#[test]
fn a_fuel_run_is_won_the_step_the_reactor_goes_critical() {
    let (mut world, station) = mission_world(Mission::FuelRun);
    if let Some(Objective::FuelRun(f)) = world
        .infested
        .iter_mut()
        .find(|it| it.station == station)
        .and_then(|it| it.objective.as_mut())
    {
        f.delivered = data::FUEL_NEEDED;
    }
    let events = world.step(&[]);
    assert!(
        events
            .iter()
            .any(|e| matches!(e, WorldEvent::Objective { what: 8, .. }))
    );
    assert!(world.infestation(station).unwrap().cleared);
}
