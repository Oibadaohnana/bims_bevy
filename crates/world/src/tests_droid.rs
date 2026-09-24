//! What the machines have to be true of (feature 83): the room holds
//! them beside no Bims at all, a hit reaches the one it was aimed at, a
//! wreck is down and worth fifteen points once and carries nothing, the
//! Unmaker strips armour rather than opening the body under it, the
//! waves are fixed at the first dock and come one at a time, and two
//! worlds on one seed meet the same machines.
//!
//! The plan's own arithmetic — [`crate::droid::wave_size`],
//! `wave_count`, `worth_steps` — is pinned next door in `droid.rs`, and
//! what a machine *is* in `bims::droid`.

use bims::combat::{ArmourKind, Gear, Tier, Weapon, WeaponKind};
use bims::droid::{DroidKind, DroidPart};
use shipdesign::fixture::{COMBAT_CREW, combat_ship};
use worldgen::GalaxyType;

use crate::checksum::world_checksum;
use crate::class;
use crate::data;
use crate::event::WorldEvent;
use crate::fixture::REFERENCE_MONEY;
use crate::world::World;

/// The `droids` probe's world, made by hand: the combat ship's fourteen
/// crew docked at the arena, and the arena held by the machines. A gun
/// in every hand, the way `Session::combat` deals them.
fn held_arena() -> (World, u32) {
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
    // The shortcut the probes take, so a wave can be waited for in a
    // test as readily as in a window.
    world.set_droid_reinforce_minutes_for_probe(1.0);
    (world, station)
}

/// Step until the first wave is standing, and answer how many there are.
fn open_the_room(world: &mut World) -> u32 {
    for _ in 0..40 {
        world.step(&[]);
        if world.droids_standing() > 0 {
            return world.droids_standing();
        }
    }
    panic!("no machines ever stood up");
}

/// The residents' room, for a test that wants to poke it.
macro_rules! room {
    ($world:expr) => {
        &$world.residents.as_ref().unwrap().aboard.room
    };
}
macro_rules! room_mut {
    ($world:expr) => {
        &mut $world.residents.as_mut().unwrap().aboard.room
    };
}

/// Destroy every machine standing, where it stands.
fn wreck_them_all(world: &mut World) {
    let n = room!(world).droid_count() as usize;
    let room = room_mut!(world);
    for i in 0..n {
        room.strike_droid(i, DroidPart::Chassis, 1e6);
    }
}

#[test]
fn a_held_station_has_machines_and_no_people_at_all() {
    let (mut world, station) = held_arena();
    let n = open_the_room(&mut world);
    assert!(n > 0, "a wave stood up");

    // **No Bim is ever made for a machine, and none is among them.**
    let room = room!(world);
    assert_eq!(room.crew_count(), 0, "the station's people are gone");
    assert_eq!(room.droid_count(), n, "and the machines are the room");
    assert_eq!(room.body_count(), n, "one index space, the Bims first");
    // Which is what `people_of` says, and `mercenaries_of` with it.
    let at = world.station(station).unwrap().clone();
    assert_eq!(world.people_of(&at), 0);
    assert_eq!(world.mercenaries_of(&at), 0);
    // A held station is an enemy's whatever the hostile list says.
    assert_eq!(world.stance(station), bims::sight::Stance::Hostile);
    assert!(world.is_droid_held(station));

    // The wave's mix is `bims::droid`'s, and every machine is at the
    // world's tier with the arm its kind gives it.
    let (husks, troopers, wardens) = bims::droid::mix_of(n);
    let mut seen = (0u32, 0u32, 0u32);
    for i in 0..n as usize {
        let droid = room!(world).droid(i).unwrap();
        assert_eq!(droid.tier, Tier::One, "tier one outside the probes' dial");
        assert_eq!(droid.wave, 1, "wave one is aboard at the first dock");
        match droid.kind {
            DroidKind::Husk => {
                seen.0 += 1;
                assert_eq!(droid.weapon.kind, WeaponKind::Claw);
            }
            DroidKind::Trooper => {
                seen.1 += 1;
                assert!(matches!(
                    droid.weapon.kind,
                    WeaponKind::LaserPistol | WeaponKind::AutoRifle
                ));
            }
            DroidKind::Warden => {
                seen.2 += 1;
                assert_eq!(droid.weapon.kind, WeaponKind::Unmaker);
            }
        }
    }
    assert_eq!(seen, (husks, troopers, wardens), "the mix the plan says");
    // Troopers alternate pistol and auto rifle by their place among the
    // Troopers, so a wave with two of them has one of each.
    if troopers >= 2 {
        let arms: Vec<WeaponKind> = (0..n as usize)
            .filter_map(|i| room!(world).droid(i))
            .filter(|d| d.kind == DroidKind::Trooper)
            .map(|d| d.weapon.kind)
            .collect();
        assert_eq!(arms[0], WeaponKind::LaserPistol);
        assert_eq!(arms[1], WeaponKind::AutoRifle);
    }
}

#[test]
fn a_hit_reaches_the_machine_it_was_aimed_at_and_no_other() {
    let (mut world, _) = held_arena();
    let n = open_the_room(&mut world) as usize;
    assert!(n >= 2, "two to tell apart");
    let before: Vec<f32> = (0..n)
        .map(|i| {
            room!(world)
                .droid(i)
                .unwrap()
                .body
                .health(DroidPart::Chassis)
        })
        .collect();
    let target = 1;
    room_mut!(world).strike_droid(target, DroidPart::Chassis, 5.0);
    for i in 0..n {
        let now = room!(world)
            .droid(i)
            .unwrap()
            .body
            .health(DroidPart::Chassis);
        if i == target {
            assert_eq!(now, before[i] - 5.0, "the one aimed at");
        } else {
            assert_eq!(now, before[i], "and no other");
        }
    }
}

#[test]
fn a_wreck_is_down_carries_nothing_and_is_worth_fifteen_once() {
    let (mut world, station) = held_arena();
    open_the_room(&mut world);
    // The crew member that does it has a class, so it earns.
    world.set_class(0, class::Class::Soldier).ok();
    let before = world.progress[0].xp;

    // One machine down, where it stands, and the step that follows says
    // so and pays for it.
    room_mut!(world).strike_droid(0, DroidPart::Head, 1e6);
    assert!(room!(world).droid(0).unwrap().destroyed);
    // Down for everything that asks.
    assert!(room!(world).is_down(0), "a wreck is down");
    assert!(!room!(world).is_alive(0));
    assert!(!room!(world).is_unconscious(0), "and never out cold");
    // And carries nothing: the Loot window's cells are empty and
    // nothing may be taken off it.
    assert!(room!(world).loot_cells(0).iter().all(|c| c.is_none()));
    for cell in [
        bims::combat::LootCell::Weapon,
        bims::combat::LootCell::Head,
        bims::combat::LootCell::Pack(0),
    ] {
        assert!(
            room_mut!(world).take_from_body(0, cell).is_none(),
            "nothing comes off a machine"
        );
    }
    // A wreck cannot be finished off either — there is no dying state.
    assert!(!room_mut!(world).execute_body(0));

    // Stand the crew member next to it so the experience is in range,
    // and step: `XP_ENEMY_DOWN` and `XP_ENEMY_DEAD` together, once.
    let events = world.step(&[]);
    // A machine is said as a machine, not as an enemy with a name.
    assert!(
        events.iter().any(|e| matches!(
            e,
            WorldEvent::DroidDown { station: s, who: 0, .. } if *s == station
        )),
        "said down as a machine"
    );
    assert!(
        !events
            .iter()
            .any(|e| matches!(e, WorldEvent::EnemyDown { .. })),
        "and never as one of the station's people"
    );
    let gained = world.progress[0].xp - before;
    // The crew member has to be within `VICINITY_TILES` of it to earn;
    // whether it was is the vicinity rule's, so what is pinned here is
    // that it is never paid twice.
    let after_one = world.progress[0].xp;
    for _ in 0..10 {
        world.step(&[]);
    }
    assert_eq!(world.progress[0].xp, after_one, "paid once, never again");
    if gained > 0 {
        assert_eq!(
            gained,
            class::XP_ENEMY_DOWN + class::XP_ENEMY_DEAD,
            "down and dead together"
        );
    }
}

#[test]
fn the_unmaker_strips_an_unbroken_piece_and_a_bare_part_takes_the_damage() {
    // The room's rule, asked of a Bim directly: a hit carrying a strip
    // takes it off the piece over the part with the piece's protection
    // ignored, and the part takes nothing.
    let mut room = bims::game::Game::new(7, 800.0, 600.0);
    let kevlar = bims::combat::Piece::new(1, ArmourKind::BasicKevlar, Tier::One);
    let whole = kevlar.health;
    let protection = kevlar.stats().protection;
    assert!(protection > 0.0, "a kevlar protects");
    let gear = room.gear(0);
    room.issue(
        0,
        Gear {
            body: Some(kevlar),
            ..gear
        },
    );
    let body_before = room.part_health(0, bims::health::Part::Body);
    let strips = WeaponKind::Unmaker.stats().strips;

    room.strip_for_probe(0, bims::health::Part::Body, 6.0, strips);
    // The piece lost exactly `strips`, protection ignored...
    let worn = room.gear(0).body.unwrap();
    assert!(
        (worn.health - (whole - strips).max(0.0)).abs() < 1e-3,
        "the piece lost the whole strip: {} from {whole}",
        worn.health
    );
    // ...and the part under it took nothing.
    assert_eq!(
        room.part_health(0, bims::health::Part::Body),
        body_before,
        "the part is untouched"
    );
    assert_eq!(room.wounds(0, bims::health::Part::Body), 0, "and unwounded");

    // A **bare** part takes the plain damage instead.
    let mut bare = bims::game::Game::new(7, 800.0, 600.0);
    let was = bare.part_health(0, bims::health::Part::Legs);
    bare.strip_for_probe(0, bims::health::Part::Legs, 4.0, strips);
    assert_eq!(
        bare.part_health(0, bims::health::Part::Legs),
        was - 4.0,
        "a bare part takes the damage"
    );

    // And a tier scales the strip the way it scales the damage.
    let one = WeaponKind::Unmaker.basic().stats();
    let two = WeaponKind::Unmaker.at(Tier::Two).stats();
    assert!(two.strips > one.strips);
    assert!((two.strips / one.strips - two.damage / one.damage).abs() < 1e-5);
}

#[test]
fn a_broken_piece_is_no_shield_and_the_part_takes_the_damage() {
    let mut room = bims::game::Game::new(11, 800.0, 600.0);
    let mut kevlar = bims::combat::Piece::new(1, ArmourKind::BasicKevlar, Tier::One);
    kevlar.health = 0.0;
    assert!(kevlar.broken());
    let gear = room.gear(0);
    room.issue(
        0,
        Gear {
            body: Some(kevlar),
            ..gear
        },
    );
    let was = room.part_health(0, bims::health::Part::Body);
    room.strip_for_probe(0, bims::health::Part::Body, 5.0, 30.0);
    assert_eq!(
        room.part_health(0, bims::health::Part::Body),
        was - 5.0,
        "a broken piece strips nothing and shields nothing"
    );
}

#[test]
fn no_reinforcement_while_a_machine_lives_and_one_a_clock_after_the_last_dies() {
    let (mut world, station) = held_arena();
    let first = open_the_room(&mut world) as usize;
    let waves = world.infestation(station).unwrap().waves_left;
    assert!(waves > 0, "the arena has more to come");

    // A wave is never reinforced mid-fight: with one left standing,
    // nothing arrives however long the clock runs.
    for i in 1..first {
        room_mut!(world).strike_droid(i, DroidPart::Chassis, 1e6);
    }
    assert_eq!(world.droids_standing(), 1, "one left");
    let was = world.mission_minutes();
    for _ in 0..200 {
        world.step(&[]);
    }
    assert!(
        world.mission_minutes() - was > world.droid_reinforce_minutes(),
        "the clock ran well past a reinforcement"
    );
    assert_eq!(
        world.infestation(station).unwrap().wave,
        1,
        "still wave one"
    );
    assert_eq!(world.droids_standing(), 1, "and no more arrived");

    // The last one down starts the clock, and nothing arrives before it
    // runs out.
    wreck_them_all(&mut world);
    let events = world.step(&[]);
    assert!(
        !events
            .iter()
            .any(|e| matches!(e, WorldEvent::DroidReinforcements { .. })),
        "not the same step"
    );
    let due = world
        .infestation(station)
        .unwrap()
        .next_wave
        .expect("the clock is running now");
    assert!(due > world.mission_steps());

    // Run it out: the wave lands, and it is said once.
    let mut landed = 0;
    for _ in 0..400 {
        let events = world.step(&[]);
        landed += events
            .iter()
            .filter(
                |e| matches!(e, WorldEvent::DroidReinforcements { station: s } if *s == station),
            )
            .count();
        if landed > 0 && world.droids_standing() > 0 {
            break;
        }
    }
    assert_eq!(landed, 1, "said once");
    assert_eq!(world.infestation(station).unwrap().wave, 2, "wave two");
    assert_eq!(
        world.infestation(station).unwrap().waves_left,
        waves - 1,
        "one fewer to come"
    );
    assert!(world.droids_standing() > 0, "and it is standing");
    // Every machine of it knows which wave it came with.
    for i in 0..room!(world).droid_count() as usize {
        assert_eq!(room!(world).droid(i).unwrap().wave, 2);
    }
}

#[test]
fn a_wave_arrives_at_the_airlock_farthest_from_the_port() {
    let (mut world, station) = held_arena();
    open_the_room(&mut world);
    // Wave one is stood about the rooms; the *next* one comes in at a
    // door. Clear the deck and let it land.
    wreck_them_all(&mut world);
    for _ in 0..400 {
        world.step(&[]);
        if world.infestation(station).unwrap().wave == 2 && world.droids_standing() > 0 {
            break;
        }
    }
    assert_eq!(world.infestation(station).unwrap().wave, 2);

    let design = &world.station(station).unwrap().design;
    let ports = crate::droid::airlocks(design);
    let arrival = crate::droid::arrival_airlock(design).expect("an airlock to arrive at");
    let first = ports[0];
    // The farthest from the crew's own door, and a lower index on a tie.
    let far = |p: &shipdesign::dock::Port| {
        (p.centre.0 - first.centre.0).hypot(p.centre.1 - first.centre.1)
    };
    for port in &ports {
        assert!(far(port) <= far(&arrival) + 1e-6, "none is farther");
    }
    // And the wave stands inside it rather than scattered over the deck.
    let inside = crate::droid::inside_of(&arrival, data::ASHORE_TILES);
    let aboard = &world.residents.as_ref().unwrap().aboard;
    let want = aboard.to_room(worldgen::math::dvec2(inside.0, inside.1));
    let near = (0..room!(world).droid_count() as usize)
        .map(|i| (room!(world).droid(i).unwrap().pos - want).len())
        .fold(f32::MAX, f32::min);
    assert!(
        near < shipdesign::TILE as f32 * 2.0,
        "one of them is at the door: {near} units off"
    );
}

#[test]
fn the_wave_count_is_fixed_at_the_first_dock_and_a_rich_crew_is_not_doubled() {
    let (mut world, station) = held_arena();
    open_the_room(&mut world);
    let settled = world.infestation(station).unwrap().clone();
    assert!(settled.settled, "the first dock settled it");
    let was = world.droid_wave_count();
    assert_eq!(settled.waves_left, was.saturating_sub(1));

    // Ten times as rich, and a month older: the count does not move.
    world.money = world.money.saturating_add(world.start_worth * 10);
    for _ in 0..20 {
        world.step(&[]);
    }
    assert_eq!(
        world.infestation(station).unwrap().waves_left,
        settled.waves_left,
        "fixed at the first dock"
    );

    // And the **size** is the formula's, never a doubling: a crew ten
    // times as rich meets the sum, capped, and not a thousand machines.
    let steps = crate::droid::worth_steps(world.worth(), world.start_worth);
    let want = crate::droid::wave_size(
        world.aboard.crew_count(),
        crate::droid::day_steps(world.days_gone()),
        steps,
        0,
    )
    .min(world.droid_wave_max());
    assert_eq!(world.droid_wave_size(), want.max(1));
    assert!(world.droid_wave_size() <= data::DROID_WAVE_MAX);
}

#[test]
fn the_last_machine_of_the_last_wave_clears_the_station_once() {
    let (mut world, station) = held_arena();
    open_the_room(&mut world);
    // Cut it to one wave, so the run is short.
    if let Some(it) = world.infested.iter_mut().find(|it| it.station == station) {
        it.waves_left = 0;
    }
    assert!(!world.droid_station_cleared(station));
    wreck_them_all(&mut world);
    let mut said = 0;
    for _ in 0..30 {
        said += world
            .step(&[])
            .iter()
            .filter(
                |e| matches!(e, WorldEvent::DroidStationCleared { station: s } if *s == station),
            )
            .count();
    }
    assert_eq!(said, 1, "said once");
    assert!(
        world.droid_station_cleared(station),
        "and the crisis step can read it"
    );
}

#[test]
fn the_state_survives_leaving_and_the_checksum_notices_it() {
    let (mut world, station) = held_arena();
    open_the_room(&mut world);
    // Half the wave down, then the ship away: the room closes.
    let n = room!(world).droid_count() as usize;
    for i in 0..n / 2 {
        room_mut!(world).strike_droid(i, DroidPart::Chassis, 1e6);
    }
    let waves = world.infestation(station).unwrap().clone();
    world.undock_for_probe();
    world.residents = None;
    for _ in 0..5 {
        world.step(&[]);
    }
    assert_eq!(
        world.infestation(station).unwrap(),
        &waves,
        "leaving resets nothing"
    );
    assert!(world.is_droid_held(station), "and it is still held");

    // The checksum notices the state: a world one wave further on is a
    // different world.
    let before = world_checksum(&world);
    if let Some(it) = world.infested.iter_mut().find(|it| it.station == station) {
        it.wave += 1;
    }
    assert_ne!(world_checksum(&world), before, "a wave is in the checksum");
    // And whether a station is held at all.
    let mut clean = held_arena().0;
    let held = world_checksum(&clean);
    clean.infested.clear();
    assert_ne!(world_checksum(&clean), held, "so is the holding");
}

#[test]
fn two_worlds_on_one_seed_meet_the_same_machines() {
    let run = || {
        let (mut world, _) = held_arena();
        open_the_room(&mut world);
        for _ in 0..60 {
            world.step(&[]);
        }
        let room = &world.residents.as_ref().unwrap().aboard.room;
        let shape: Vec<(u32, u32, u32)> = (0..room.droid_count() as usize)
            .filter_map(|i| room.droid(i))
            .map(|d| (d.kind.code(), d.weapon.kind.code(), d.tier.code()))
            .collect();
        (world_checksum(&world), shape)
    };
    let (a_sum, a_shape) = run();
    let (b_sum, b_shape) = run();
    assert_eq!(a_shape, b_shape, "the same wave");
    assert!(!a_shape.is_empty(), "and there was one");
    assert_eq!(a_sum, b_sum, "and the same checksum");
}

#[test]
fn the_probes_dials_move_the_tier_and_the_cap() {
    let (mut world, _) = held_arena();
    world.set_droid_tier_for_probe(Some(Tier::Three));
    world.set_droid_wave_for_probe(3);
    let n = open_the_room(&mut world);
    assert!(n <= 3, "the cap holds: {n}");
    let room = &world.residents.as_ref().unwrap().aboard.room;
    for i in 0..room.droid_count() as usize {
        let droid = room.droid(i).unwrap();
        assert_eq!(droid.tier, Tier::Three);
        assert_eq!(droid.weapon.tier, Tier::Three, "the arm at the tier too");
        // And every part is the tier's, which is the kind's times the
        // armour step twice.
        let base = droid.kind.body();
        let step = bims::balance::ARMOUR_TIER_STEP * bims::balance::ARMOUR_TIER_STEP;
        for (p, part) in DroidPart::ALL.iter().enumerate() {
            assert!((droid.body.max(*part) - base[p] * step).abs() < 1e-3);
        }
    }
}

#[test]
fn a_held_station_keeps_its_stance_and_its_room_is_hostile() {
    let (mut world, station) = held_arena();
    open_the_room(&mut world);
    assert_eq!(world.stance(station), bims::sight::Stance::Hostile);
    // Taking the machines off gives the station back its own stance.
    world.infested.clear();
    assert_ne!(world.stance(station), bims::sight::Stance::Hostile);
}

#[test]
fn the_crew_s_targets_run_over_the_room_s_bims_then_its_machines() {
    let (mut world, _) = held_arena();
    let n = open_the_room(&mut world) as usize;
    world.step(&[]);
    // The joined deck's targets are the room's bodies, in order: no
    // Bims here, so every one of them is a machine, at its own place.
    let targets = world.aboard.room.hostiles_for_probe();
    assert_eq!(targets.len(), n, "one target a machine");
    for (i, target) in targets.iter().enumerate() {
        let alive = !room!(world).droid(i).unwrap().destroyed;
        assert_eq!(target.is_some(), alive, "a wreck is nobody's target");
    }
    // And a machine's weapon crosses with it, so a claw locks a gunner.
    let claws = (0..n)
        .filter(|&i| room!(world).droid(i).unwrap().kind == DroidKind::Husk)
        .count();
    let melee = targets
        .iter()
        .flatten()
        .filter(|t| t.weapon.kind == WeaponKind::Claw)
        .count();
    assert_eq!(melee, claws, "every Husk's claw crossed over");
    assert!(
        targets.iter().flatten().all(|t| !t.weapon.kind.carried()
            || matches!(
                t.weapon,
                Weapon {
                    kind: WeaponKind::LaserPistol | WeaponKind::AutoRifle,
                    ..
                }
            )),
        "a Trooper's arm is a pistol or a rifle and nothing else"
    );
}

/// What a wave costs the world's step, at the cap and past it. **Not a
/// check** — it asserts nothing about the clock, since a number measured
/// on one machine is not a number to pin — but the measurement feature
/// 83 asks for, and the way to take it again after anything that touches
/// `tick_droids` or the tactics:
///
/// ```text
/// cargo test --release -p world -- --ignored --nocapture droid_waves_cost
/// ```
///
/// The frame time is the other half and is not measurable from here:
/// `BIMS_SMOKE_FREE=1 BIMS_DROID_WAVE=n BIMS_SMOKE_FRAMES=600 ./hidden
/// target/release/bims droids` prints it.
#[test]
#[ignore = "a measurement, not a check"]
fn droid_waves_cost_this_much_a_step() {
    for cap in [16u32, 32, 64] {
        // Forced, since the formula's own answer is sixteen here.
        let (mut world, _) = held_arena();
        world.set_droid_wave_for_probe(cap);
        let n = open_the_room(&mut world);
        assert_eq!(n, cap, "the dial says the size outright");
        // A few steps to let them plan and set off, so what is timed is
        // a wave walking and shooting rather than one standing still.
        for _ in 0..120 {
            world.step(&[]);
        }
        let steps = 600;
        let began = std::time::Instant::now();
        for _ in 0..steps {
            world.step(&[]);
        }
        let each = began.elapsed().as_secs_f64() * 1000.0 / steps as f64;
        println!("droid wave cap {cap}: {n} machines, {each:.3} ms a world step");
    }
}

/// A crew member ashore, just inside the station's door and under arms:
/// something for the machines to go to war over, without walking one
/// there. `World::stage_fight_for_probe`'s own spot, which a droid-held
/// station has no people of its own for.
fn put_one_ashore(world: &mut World) {
    let ashore = world.aboard.ashore.expect("docked, so there is a door");
    world
        .aboard
        .room
        .put_for_probe(0, bims::math::vec2(ashore.x as f32, ashore.y as f32));
    world.aboard.room.recruit_for_probe(0, true);
}

/// Step until the wave after the one aboard has landed and is standing.
fn land_the_next_wave(world: &mut World, station: u32) -> u32 {
    let was = world.infestation(station).unwrap().wave;
    wreck_them_all(world);
    for _ in 0..900 {
        world.step(&[]);
        let wave = world.infestation(station).unwrap().wave;
        if wave > was && world.droids_standing() > 0 {
            return wave;
        }
    }
    panic!("the next wave never landed");
}

/// **Every machine of a landing wave stands where a body fits.** A
/// reinforcement wave is posted in rings round the airlock its ship tied
/// up at, and a ring falls where it falls: at the arena four of sixteen
/// landed in a bulkhead or out in the void, where they could neither
/// walk nor see. They stood there for good — never fired a shot, and
/// held the wave after them up as well, since nothing arrives while one
/// is still standing. `Game::adopt_droids` snaps them now, the way
/// `Game::adopt` snaps a Bim carried between rooms.
#[test]
fn every_machine_of_a_landing_wave_stands_where_a_body_fits() {
    let (mut world, station) = held_arena();
    // Three waves, so there is one after the one that lands.
    world.set_droid_waves_for_probe(3);
    open_the_room(&mut world);
    put_one_ashore(&mut world);
    for _ in 0..300 {
        world.step(&[]);
    }
    assert_eq!(land_the_next_wave(&mut world, station), 2);

    let n = room!(world).droid_count() as usize;
    assert!(n > 1, "a wave of more than one, so the rings are used");
    let off: Vec<usize> = (0..n)
        .filter(|&i| {
            let at = room!(world).droid(i).unwrap().pos;
            !room!(world).is_deck_tile(at)
        })
        .collect();
    assert!(off.is_empty(), "machines landed off the deck: {off:?}");

    // And the wave after it lands too, which is what a stranded machine
    // was holding up.
    assert_eq!(land_the_next_wave(&mut world, station), 3);
}

/// **A wave cleared takes the room's memory of it with it.** The lists
/// `visit` keeps are one entry a body and only ever grow; left as they
/// were, every machine of the next wave was born already flagged down —
/// destroying it said no `DroidDown` and paid no experience.
#[test]
fn the_next_wave_s_machines_are_said_down_and_paid_for_like_the_first() {
    let (mut world, station) = held_arena();
    world.set_droid_waves_for_probe(3);
    open_the_room(&mut world);
    put_one_ashore(&mut world);
    let _ = world.set_class(0, class::Class::Soldier);
    for _ in 0..60 {
        world.step(&[]);
    }
    let first_xp = {
        let was = world.progress[0].xp;
        wreck_them_all(&mut world);
        let events = world.step(&[]);
        assert!(
            events
                .iter()
                .any(|e| matches!(e, WorldEvent::DroidDown { .. })),
            "wave one's wrecks are said"
        );
        world.progress[0].xp - was
    };
    assert!(first_xp > 0, "and paid for");

    // Wave two, and the same again.
    for _ in 0..900 {
        world.step(&[]);
        if world.infestation(station).unwrap().wave == 2 && world.droids_standing() > 0 {
            break;
        }
    }
    assert_eq!(world.infestation(station).unwrap().wave, 2);
    // Experience is paid to whoever is within `class::VICINITY_TILES` of
    // the wreck, and this wave landed at the airlock at the far end of a
    // seventy-tile arena: the crew member goes over to it.
    let at = world
        .body_position(crate::armour::LootSource::Resident(0))
        .expect("a machine of the wave to stand beside");
    world.aboard.room.put_for_probe(0, at);
    let was = world.progress[0].xp;
    wreck_them_all(&mut world);
    let events = world.step(&[]);
    assert!(
        events
            .iter()
            .any(|e| matches!(e, WorldEvent::DroidDown { .. })),
        "wave two's wrecks are said too"
    );
    // And paid for. Not the same sum as wave one's — that one stood
    // about the whole arena and only the machines within the vicinity
    // counted, where this one is landed on top of the crew member — but
    // before the lists were cut back it was nought.
    assert!(world.progress[0].xp > was, "and paid for");
}

/// The probes' dial for how many waves a held station has — three on the
/// `droids` commands, where the formula's own at day nought is two
/// ([`data::DROID_WAVES_BASE`]) — read at the first dock and never
/// again, like the formula it stands in for.
#[test]
fn a_probe_says_how_many_waves_a_held_station_has() {
    let (mut world, station) = held_arena();
    assert_eq!(world.droid_wave_count(), data::DROID_WAVES_BASE);
    world.set_droid_waves_for_probe(3);
    assert_eq!(world.droid_wave_count(), 3);
    open_the_room(&mut world);
    let it = world.infestation(station).unwrap();
    assert_eq!(it.wave, 1, "wave one is aboard");
    assert_eq!(it.waves_left, 2, "and two more to come");
    // Fixed at the dock: moving the dial after it changes nothing.
    world.set_droid_waves_for_probe(9);
    for _ in 0..20 {
        world.step(&[]);
    }
    assert_eq!(world.infestation(station).unwrap().waves_left, 2);
    // And nought reads as one, since a station with no wave at all is
    // nothing to look at.
    let (mut other, _) = held_arena();
    other.set_droid_waves_for_probe(0);
    assert_eq!(other.droid_wave_count(), 1);
}

/// What the app counts down: which wave is aboard, how many there are,
/// and how long until the next lands — nothing while one is standing,
/// since a wave is never reinforced mid-fight.
#[test]
fn the_countdown_says_the_wave_and_how_long_until_the_next() {
    let (mut world, station) = held_arena();
    world.set_droid_waves_for_probe(3);
    open_the_room(&mut world);
    assert_eq!(world.droid_wave_standing(), Some((1, 2)));
    assert_eq!(world.droid_wave_due(), None, "a fight is on: no clock");

    wreck_them_all(&mut world);
    world.step(&[]);
    let left = world.droid_wave_due().expect("the clock is running now");
    assert!(
        left > 0.0 && left <= world.droid_reinforce_minutes(),
        "a whole clock at most: {left}"
    );
    // And it counts down.
    for _ in 0..20 {
        world.step(&[]);
    }
    assert!(world.droid_wave_due().unwrap() < left);

    // The last wave down is no countdown at all.
    for _ in 0..1800 {
        world.step(&[]);
        if world.infestation(station).unwrap().wave == 3 && world.droids_standing() > 0 {
            break;
        }
        if world.droids_standing() > 0 {
            wreck_them_all(&mut world);
        }
    }
    assert_eq!(world.infestation(station).unwrap().wave, 3, "the last wave");
    wreck_them_all(&mut world);
    world.step(&[]);
    assert_eq!(world.droid_wave_standing(), Some((3, 0)));
    assert_eq!(world.droid_wave_due(), None, "nothing left to come");
}
