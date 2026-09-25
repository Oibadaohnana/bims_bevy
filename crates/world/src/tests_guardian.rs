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
fn guardians_come_in_a_tier_three_wave_and_in_no_other() {
    for (tier, wave, want) in [
        (Tier::Three, 8, 1),
        (Tier::Three, 16, 2),
        (Tier::Three, 3, 0),
        (Tier::Two, 16, 0),
        (Tier::One, 16, 0),
    ] {
        let mut world = held_arena(tier, wave);
        let kinds = first_wave(&mut world);
        assert_eq!(kinds.len(), wave as usize);
        let guardians = kinds.iter().filter(|&&k| k == DroidKind::Guardian).count();
        assert_eq!(guardians, want, "a wave of {wave} at tier {tier:?}");
        // Out of the Troopers' share, and the rest of the mix untouched.
        let (husks, troopers, wardens) = bims::droid::mix_of(wave);
        let count = |k: DroidKind| kinds.iter().filter(|&&x| x == k).count() as u32;
        assert_eq!(count(DroidKind::Husk), husks);
        assert_eq!(count(DroidKind::Warden), wardens);
        assert_eq!(count(DroidKind::Trooper), troopers - want as u32);
        let room = &world.residents.as_ref().unwrap().aboard.room;
        for i in 0..room.droid_count() as usize {
            let d = room.droid(i).unwrap();
            if d.kind == DroidKind::Guardian {
                assert_eq!(d.weapon.kind, WeaponKind::Sweeper);
                assert_eq!(d.tier, Tier::Three);
                assert!(d.shield().is_some(), "it stands behind its shield");
            }
        }
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
