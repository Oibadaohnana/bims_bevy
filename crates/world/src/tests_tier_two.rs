//! The tier-two machines (task 157), as far as the world is concerned:
//! which waves they come in — the Bombers and the Lancers on top of every
//! wave from the floor's tier-two zone on, the Conductor in an elite's
//! Guardian wave there — and a Bomber's bomb and a Lancer's slug crossing
//! the seam to land in the crew's room. The machines themselves are
//! `bims`'s (`crates/game/src/tests_tier_two.rs`).

use bims::combat::{Gear, Tier, WeaponKind};
use bims::droid::DroidKind;
use shipdesign::fixture::{COMBAT_CREW, combat_ship, flyer};
use worldgen::GalaxyType;

use crate::data;
use crate::fixture::{REFERENCE_MONEY, simulation_world};
use crate::world::World;

/// The `droids` probe's world, made by hand — the combat ship's crew, a gun
/// in every hand, docked at the arena — with the wave size forced and the
/// arena then handed to the machines; the zone is the run day's unless
/// `tier` forces it.
fn held_arena(tier: Option<Tier>, wave: u32) -> (World, u32) {
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
    let crew = world.aboard.room.crew_count() as usize;
    for who in 0..crew {
        let gear = world.aboard.room.gear(who);
        world.aboard.room.issue(
            who,
            Gear {
                weapon: Some(WeaponKind::LaserPistol.basic()),
                ..gear
            },
        );
    }
    world.set_droid_tier_for_probe(tier);
    world.set_droid_wave_for_probe(wave);
    let id = world.ship.state.alongside().expect("docked at the arena");
    (world, id)
}

/// The kinds standing in the residents' room once the first wave is up.
fn first_wave(world: &mut World, station: u32) -> Vec<DroidKind> {
    world.infest(station);
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

fn count(kinds: &[DroidKind], kind: DroidKind) -> u32 {
    kinds.iter().filter(|&&k| k == kind).count() as u32
}

#[test]
fn a_tier_one_zone_s_wave_has_none_and_a_tier_two_zone_s_has_them_on_top() {
    for (tier, wave) in [
        (Tier::One, 12),
        (Tier::Two, 12),
        (Tier::Two, 4),
        (Tier::Three, 16),
    ] {
        let (mut world, station) = held_arena(Some(tier), wave);
        let kinds = first_wave(&mut world, station);
        let (husks, troopers, wardens) = bims::droid::mix_of(wave);
        // The wave's own mix stands under them, whatever the zone.
        assert_eq!(count(&kinds, DroidKind::Husk), husks, "{tier:?} {wave}");
        assert_eq!(count(&kinds, DroidKind::Trooper), troopers);
        assert_eq!(count(&kinds, DroidKind::Warden), wardens);
        let (bombers, lancers) = if tier >= Tier::Two {
            ((wave / 6).max(1), (wave / 8).max(1))
        } else {
            (0, 0)
        };
        assert_eq!(count(&kinds, DroidKind::Bomber), bombers, "{tier:?} {wave}");
        assert_eq!(count(&kinds, DroidKind::Lancer), lancers, "{tier:?} {wave}");
        assert_eq!(kinds.len() as u32, wave + bombers + lancers, "on top");
        assert_eq!(count(&kinds, DroidKind::Conductor), 0, "not an elite");
    }
}

#[test]
fn the_zone_is_the_floor_s_tier_two_rows_by_the_run_day() {
    let (mut world, station) = held_arena(None, 8);
    let tier2 = world.scaling().tier2_days;
    assert!(tier2 > 2, "a tier-two zone some way in");
    assert_eq!(world.zone_tier(), Tier::One);
    world.set_day_for_probe(tier2);
    assert!(world.run_day() >= tier2);
    assert_eq!(world.zone_tier(), Tier::Two);
    let kinds = first_wave(&mut world, station);
    assert_eq!(count(&kinds, DroidKind::Bomber), 1);
    assert_eq!(count(&kinds, DroidKind::Lancer), 1);
    // A dial at nought brings none of that kind.
    let mut scaling = world.scaling();
    scaling.lancer_every = 0;
    world.set_wave_scaling(scaling);
    assert_eq!(world.scaling().tier_two_extras(8), (1, 0));
}

#[test]
fn a_tier_two_elite_s_guardian_wave_has_its_conductor() {
    for (tier, conductors) in [(Tier::One, 0), (Tier::Two, 1), (Tier::Three, 1)] {
        let (mut world, station) = held_arena(Some(tier), 12);
        world.set_elite_for_probe(station);
        first_wave(&mut world, station);
        let first = world.wave_kinds_for(12, 1);
        assert_eq!(count(&first, DroidKind::Conductor), 0, "not in wave one");
        let second = world.wave_kinds_for(12, data::ELITE_GUARDIAN_WAVE);
        assert_eq!(
            count(&second, DroidKind::Conductor),
            conductors,
            "the Guardian wave at {tier:?}"
        );
        assert!(
            count(&second, DroidKind::Guardian) >= 1,
            "and the Guardians still come"
        );
    }
}

#[test]
fn a_bomber_s_bomb_and_a_lancer_s_slug_cross_the_seam_and_land_on_the_crew() {
    // The Lancer first: the Bomber's half ends the test once its bomb lies.
    for kind in [DroidKind::Lancer, DroidKind::Bomber] {
        let mut world = simulation_world(flyer(2), REFERENCE_MONEY, 2);
        world.set_droid_tier_for_probe(Some(Tier::Two));
        let arm = kind.arm(0).at(Tier::Two);
        assert!(world.stage_droid_fight_for_probe(kind, Some(arm)));
        let whole = world.aboard.room.health(0);
        for _ in 0..(60 * 8) {
            world.step(&[]);
            let crew = world.aboard.room.bim_pos(0);
            if kind == DroidKind::Bomber
                && let Some(bomb) = world.aboard.room.grenades().iter().find(|g| g.bomb)
            {
                // Laid on the joined deck where the crew member stands — it
                // bursts on them unless they move.
                assert!(
                    (bomb.at - crew).len() <= bomb.radius,
                    "the bomb's circle is over the crew member: {:?} against {crew:?}",
                    bomb.at
                );
                return;
            }
            if kind == DroidKind::Lancer && world.aboard.room.health(0) < whole {
                break;
            }
        }
        assert!(
            kind == DroidKind::Lancer && world.aboard.room.health(0) < whole,
            "the {kind:?} reached the crew member"
        );
    }
}
