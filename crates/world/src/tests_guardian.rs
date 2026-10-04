//! The Guardian (feature 100), as far as the world is concerned: which
//! waves it comes in, and its shield handed across the seam so a crew
//! member's bolt is stopped in the crew's room where it would have
//! landed. The machine itself — the shield's rule, the turn, the beam —
//! is `bims`'s (`crates/game/src/tests_guardian.rs`).

use bims::combat::{Gear, Tier, WeaponKind};
use bims::droid::{DroidKind, DroidPart};
use shipdesign::fixture::{COMBAT_CREW, combat_ship, flyer};
use worldgen::GalaxyType;

use crate::data;
use crate::fixture::{REFERENCE_MONEY, simulation_world};
use crate::world::World;

/// The `droids` probe's world, made by hand — the combat ship's crew,
/// a gun in every hand, docked at the arena — with the machines' tier and
/// wave size forced, and the arena then handed to them.
fn held_arena(tier: Tier, wave: u32) -> World {
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
    world.set_droid_tier_for_probe(Some(tier));
    world.set_droid_wave_for_probe(wave);
    world.infest(station);
    world
}

/// The kinds standing in the residents' room once the first wave is up.
fn first_wave(world: &mut World) -> Vec<DroidKind> {
    for _ in 0..40 {
        world.step(&[]);
        if world.droids_standing() > 0 {
            let room = &world.residents.as_ref().unwrap().aboard.room;
            return (0..room.droid_count() as usize)
                .filter_map(|i| room.droid(i).map(|d| d.kind))
                .collect();
        }
    }
    panic!("no machines ever stood up");
}

#[test]
fn no_guardian_comes_in_a_wave_that_is_not_an_elite_s() {
    // October 2026, the player's word: Guardians spawn only at elite
    // sites (`tests_elite.rs`); a plain site's wave has none at any tier
    // or size, and the Troopers keep their whole share.
    for (tier, wave) in [
        (Tier::Three, 8),
        (Tier::Three, 16),
        (Tier::Two, 16),
        (Tier::One, 16),
    ] {
        let mut world = held_arena(tier, wave);
        let kinds = first_wave(&mut world);
        // From the tier-two zone on, the Bombers and the Lancers come on
        // top of the wave (task 157).
        let (bombers, lancers) = if tier >= Tier::Two {
            world.scaling().tier_two_extras(wave)
        } else {
            (0, 0)
        };
        assert_eq!(kinds.len(), (wave + bombers + lancers) as usize);
        let count = |k: DroidKind| kinds.iter().filter(|&&x| x == k).count() as u32;
        assert_eq!(
            count(DroidKind::Guardian),
            0,
            "a wave of {wave} at tier {tier:?}"
        );
        let (husks, troopers, wardens) = bims::droid::mix_of(wave);
        assert_eq!(count(DroidKind::Husk), husks);
        assert_eq!(count(DroidKind::Warden), wardens);
        assert_eq!(count(DroidKind::Trooper), troopers);
    }
}

/// The staged machine, and what is left of it.
fn guardian(world: &World) -> &bims::droid::Droid {
    world
        .residents
        .as_ref()
        .unwrap()
        .aboard
        .room
        .droid(0)
        .expect("the staged machine")
}

fn guardian_health(world: &World) -> f32 {
    let body = guardian(world).body;
    DroidPart::ALL.iter().map(|&p| body.health(p)).sum()
}

#[test]
fn a_crew_member_s_bolts_are_stopped_by_the_shield_across_the_seam_and_land_from_behind() {
    let mut world = simulation_world(flyer(2), REFERENCE_MONEY, 2);
    world.set_droid_tier_for_probe(Some(Tier::Three));
    // One Guardian four tiles down the corridor from the crew member,
    // posing — firing nothing, turning nowhere — facing the port, which
    // is to say facing the crew member.
    assert!(world.stage_droid_fight_for_probe(DroidKind::Guardian, None));
    let whole = guardian_health(&world);
    let mut fired = 0;
    for _ in 0..(60 * 20) {
        world.aboard.room.patch_up_for_probe(0);
        world.step(&[]);
        fired += world.aboard.room.bolts_in_flight().min(1);
    }
    assert!(fired > 0, "the crew member was shooting");
    assert_eq!(
        guardian_health(&world),
        whole,
        "every bolt from the front stopped at the plate"
    );
    // Each of them worn off the plate, across the seam.
    let worn = guardian(&world).plate_taken;
    assert!(worn > 0.0, "the plate took what it stopped");
    assert!(
        worn < bims::balance::GUARDIAN_SHIELD_HP,
        "and is not broken: {worn}"
    );
    // And the shield handed across is the machine's own, turned onto the
    // joined deck: facing back down the corridor at the crew.
    let facing = guardian(&world).facing();
    let room = &mut world.residents.as_mut().unwrap().aboard.room;
    // Turned about: the crew member is behind it now.
    room.droid_mut_for_probe(0)
        .unwrap()
        .face_for_probe(facing * -1.0);
    let mut hurt = false;
    for _ in 0..(60 * 20) {
        world.aboard.room.patch_up_for_probe(0);
        world.step(&[]);
        if guardian_health(&world) < whole {
            hurt = true;
            break;
        }
    }
    assert!(hurt, "from behind the bolts land");
}

/// The plate breaks when it has stopped its hit points' worth, and from
/// then on the crew member's bolts land from the front.
#[test]
fn a_plate_worn_through_across_the_seam_breaks_and_the_front_is_open() {
    let mut world = simulation_world(flyer(2), REFERENCE_MONEY, 2);
    world.set_droid_tier_for_probe(Some(Tier::Three));
    assert!(world.stage_droid_fight_for_probe(DroidKind::Guardian, None));
    let whole = guardian_health(&world);
    // A hair short of broken.
    world
        .residents
        .as_mut()
        .unwrap()
        .aboard
        .room
        .droid_mut_for_probe(0)
        .unwrap()
        .plate_taken = bims::balance::GUARDIAN_SHIELD_HP - 1.0;
    assert!(guardian(&world).shield().is_some());
    let mut broke = None;
    let mut hurt = false;
    for frame in 0..(60 * 20) {
        world.aboard.room.patch_up_for_probe(0);
        world.step(&[]);
        if broke.is_none() && guardian(&world).shield().is_none() {
            // The bolt that broke it was stopped: nothing on the body yet.
            assert_eq!(guardian_health(&world), whole);
            broke = Some(frame);
        }
        if guardian_health(&world) < whole {
            hurt = true;
            break;
        }
    }
    assert!(broke.is_some(), "the next bolt broke the plate");
    assert!(guardian(&world).plate_broken());
    assert!(hurt, "and the bolts after it land from the front");
}

/// A Guardian armed and awake four tiles down the corridor from the
/// crew member, at tier three.
fn guardian_fight() -> World {
    let mut world = simulation_world(flyer(2), REFERENCE_MONEY, 2);
    world.set_droid_tier_for_probe(Some(Tier::Three));
    assert!(world.stage_droid_fight_for_probe(
        DroidKind::Guardian,
        Some(WeaponKind::Sweeper.at(Tier::Three))
    ));
    world
}

#[test]
fn the_beam_crosses_the_seam_and_two_worlds_agree_through_it() {
    use crate::checksum::world_checksum;
    use crate::event::WorldEvent;
    let mut one = guardian_fight();
    let mut two = guardian_fight();
    assert_eq!(world_checksum(&one), world_checksum(&two));
    let mut swept = false;
    let mut hit = false;
    for step in 0..(60 * 20) {
        // Nobody dies of it: the fight is watched, not decided.
        one.aboard.room.patch_up_for_probe(0);
        two.aboard.room.patch_up_for_probe(0);
        let a = one.step(&[]);
        let b = two.step(&[]);
        assert_eq!(a, b, "the same events at step {step}");
        assert_eq!(
            world_checksum(&one),
            world_checksum(&two),
            "the two worlds parted at step {step}"
        );
        // The beam is laid in the crew's room, off the recorded shot,
        // and draws there.
        swept |= one.aboard.room.sweeps().iter().any(|s| s.drawn);
        hit |= a
            .iter()
            .any(|e| matches!(e, WorldEvent::CrewHit { who: 0, .. }));
        if swept && hit {
            break;
        }
    }
    assert!(swept, "the Guardian swept its beam across the seam");
    assert!(hit, "and it reached the crew member");
}
