//! A site's experience (October 2026, `site_xp.rs`): a site is worth its
//! day's budget to every player whatever its waves are — one player or
//! two — an elite's twice that; the bonus wave a cleared site may call,
//! bigger, for half the budget again; *Clean Sweep*; the *Training Log*
//! and the catch-up.

use bims::combat::{Gear, Tier, WeaponKind};
use bims::droid::DroidPart;
use bims::module::{ModuleKind, TRAINING_LOG_XP};
use shipdesign::fixture::{COMBAT_CREW, combat_ship};
use worldgen::GalaxyType;

use crate::armour::LootSource;
use crate::class::Class;
use crate::data;
use crate::event::{Refusal, WorldEvent};
use crate::fixture::REFERENCE_MONEY;
use crate::relic::Relic;
use crate::run::BonusWave;
use crate::world::{Command, World};

/// The combat ship's crew docked at the arena, `players` of them players
/// and every one a soldier, the arena held by the machines — the `droids`
/// probe made by hand, as `tests_droid` makes it.
fn held_arena(players: u32) -> (World, u32) {
    let galaxy = worldgen::Galaxy::new(data::DEFAULT_SEED, GalaxyType::SpiralTwoArm);
    let (star, station) = crate::spawn(&galaxy).unwrap();
    let mut world = World::start_with_crew(
        combat_ship(),
        REFERENCE_MONEY,
        players,
        COMBAT_CREW,
        data::DEFAULT_SEED,
        GalaxyType::SpiralTwoArm,
        star,
        station,
    )
    .unwrap();
    for slot in 0..players {
        world.set_class(slot, Class::Soldier).unwrap();
    }
    world.arena_dock_for_probe();
    let crew = world.aboard.room.crew_count() as usize;
    for (who, kind) in WeaponKind::ALL
        .iter()
        .copied()
        .cycle()
        .take(crew)
        .enumerate()
    {
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
    world.set_droid_reinforce_minutes_for_probe(1.0);
    (world, station)
}

/// Step until machines stand, and answer how many.
fn wait_for_a_wave(world: &mut World) -> u32 {
    for _ in 0..400 {
        world.step(&[]);
        if world.droids_standing() > 0 {
            return world.droids_standing();
        }
    }
    panic!("no machines ever stood up");
}

/// Every machine standing destroyed one by one with every player stood
/// beside it, a step each, so each is in everybody's range; the events.
fn wreck_them_one_by_one(world: &mut World, players: u32) -> Vec<WorldEvent> {
    let mut events = Vec::new();
    let n = world.residents.as_ref().unwrap().aboard.room.droid_count() as usize;
    for i in 0..n {
        let room = &world.residents.as_ref().unwrap().aboard.room;
        if room.droid(i).is_none_or(|d| d.destroyed) {
            continue;
        }
        let at = world
            .body_position(LootSource::Resident(i as u32))
            .expect("the rooms joined");
        for slot in 0..players {
            world.aboard.room.put_for_probe(slot as usize, at);
        }
        let room = &mut world.residents.as_mut().unwrap().aboard.room;
        room.strike_droid(i, DroidPart::Chassis, 1e6);
        events.extend(world.step(&[]));
    }
    for _ in 0..3 {
        events.extend(world.step(&[]));
    }
    events
}

fn xp(world: &World, slot: u32) -> u32 {
    world.progress_of(slot).xp
}

#[test]
fn the_budget_grows_a_twelfth_or_so_a_day() {
    let r = crate::rewards::Rewards::DEFAULT;
    assert_eq!(r.site_xp_on(1), 60);
    assert_eq!(r.site_xp_on(10), 166);
    assert_eq!(r.site_xp_on(20), 516);
    assert_eq!(r.site_xp_on(31), 1_797);
    assert_eq!(r.site_xp_on(0), 60, "day nought is the first");
}

/// **A site pays its budget, whatever its waves**: one player alone and
/// each of two meet waves of different sizes and are paid the same —
/// the day's budget, give or take the rounding of a share a body.
#[test]
fn a_site_pays_every_player_its_budget_alone_or_two() {
    for players in [1, 2] {
        let (mut world, station) = held_arena(players);
        let n = wait_for_a_wave(&mut world);
        let budget = world.site_budget(station) as u32;
        assert_eq!(budget, world.site_xp_on(world.run_day()));
        let each = world.xp_per_down();
        assert_eq!(each, (budget + n / 2) / n, "a body's share of the wave");
        let before: Vec<u32> = (0..players).map(|s| xp(&world, s)).collect();
        wreck_them_one_by_one(&mut world, players);
        assert!(world.droid_station_cleared(station), "one wave on day one");
        for slot in 0..players {
            let got = xp(&world, slot) - before[slot as usize];
            assert_eq!(got, each * n, "{players} players: every body once");
            assert!(got.abs_diff(budget) <= n / 2 + 1, "{got} of {budget}");
        }
    }
}

#[test]
fn an_elite_s_site_is_worth_twice_an_attack_s() {
    let (mut world, station) = held_arena(1);
    let plain = world.site_budget(station);
    world.set_elite_for_probe(station);
    assert_eq!(
        world.site_budget(station),
        plain * u64::from(data::ELITE_XP_PERCENT) / 100
    );
}

/// **The bonus wave**: refused until the site is cleared, then called
/// once — the deck thaws, a wave half as big again lands, it pays half
/// the budget again and its bounty, and once it is down the site is won
/// as it was and no second is called.
#[test]
fn a_cleared_site_calls_one_bonus_wave_half_as_big_again() {
    let (mut world, station) = held_arena(1);
    let n = wait_for_a_wave(&mut world);
    let call = Command::CallBonusWave { slot: 0 };
    assert_eq!(world.can_call_bonus_wave(0), Err(Refusal::NoBonusWave));
    wreck_them_one_by_one(&mut world, 1);
    assert!(world.fight_over(), "the deck frozen");
    assert_eq!(world.can_call_bonus_wave(0), Ok(()));
    let budget = world.site_budget(station) as u32;
    let before = xp(&world, 0);
    let money = world.money;

    let events = world.step(&[call]);
    assert!(events.contains(&WorldEvent::BonusWaveCalled { slot: 0 }));
    assert!(!world.fight_over(), "the deck thawed");
    assert_eq!(world.bonus_wave_here(), BonusWave::Called);
    let big = wait_for_a_wave(&mut world);
    assert_eq!(big, (n * data::BONUS_WAVE_SIZE_PERCENT).div_ceil(100));
    assert_eq!(world.bonus_wave_here(), BonusWave::Landed);
    let each = world.xp_per_down();
    let half = budget * data::BONUS_WAVE_XP_PERCENT / 100;
    assert_eq!(each, (half + big / 2) / big);

    let events = wreck_them_one_by_one(&mut world, 1);
    assert!(events.contains(&WorldEvent::BonusWaveCleared { station }));
    assert_eq!(xp(&world, 0) - before, each * big, "half the budget again");
    assert!(world.money > money, "and its bounty, paid at once");
    assert_eq!(world.bonus_wave_here(), BonusWave::Done);
    assert!(world.fight_over(), "frozen again");
    assert!(world.droid_station_cleared(station));
    let events = world.step(&[call]);
    assert!(events.contains(&WorldEvent::Refused {
        slot: 0,
        why: Refusal::NoBonusWave
    }));
}

/// ***Clean Sweep***: a site cleared with no player down pays every
/// player a quarter of what it earned there again; a player down since is
/// none.
#[test]
fn clean_sweep_pays_a_quarter_again_for_a_clear_with_nobody_down() {
    for spoil in [false, true] {
        let (mut world, _) = held_arena(1);
        world.give_relic_for_probe(Relic::CleanSweep);
        wait_for_a_wave(&mut world);
        if spoil {
            world.aboard.room.wound(0, 1e6);
            world.step(&[]);
            assert!(world.run.clean_spoiled, "a player's Bim down");
        }
        let before = xp(&world, 0);
        let events = wreck_them_one_by_one(&mut world, 1);
        let sweep: Vec<_> = events
            .iter()
            .filter_map(|e| match *e {
                WorldEvent::CleanSweep { who: 0, xp } => Some(xp),
                _ => None,
            })
            .collect();
        if spoil {
            assert!(sweep.is_empty(), "nothing for a clear with a down");
            continue;
        }
        let earned = xp(&world, 0) - before - sweep[0];
        assert_eq!(
            sweep,
            vec![earned * data::CLEAN_SWEEP_EXPERIENCE as u32 / 100]
        );
        assert!(world.run.clean_xp.is_empty(), "the book starts again");
    }
}

/// The ***Training Log*** puts its per cent on its carrier's share, the
/// best of two carried counting; a player below the best level among the
/// players catches up by a quarter; a bot gets the plain share.
#[test]
fn a_training_log_and_the_catch_up_lift_a_player_s_share() {
    let (mut world, _) = held_arena(2);
    let best = world.best_player_level();
    assert_eq!(world.xp_for(0, 100, best), 100);
    let mut gear = world.aboard.room.gear(0);
    gear.items[0] = Some(ModuleKind::TrainingLog.at(Tier::One));
    gear.items[1] = Some(ModuleKind::TrainingLog.at(Tier::Three));
    world.aboard.room.issue(0, gear);
    assert_eq!(world.xp_for(0, 100, best), 100 + TRAINING_LOG_XP[2]);

    let mut events = Vec::new();
    world.award(1, 1_000, &mut events);
    let best = world.best_player_level();
    assert!(world.level_of(0) < best);
    assert_eq!(
        world.xp_for(0, 100, best),
        100 + TRAINING_LOG_XP[2] + data::CATCH_UP_XP_PERCENT
    );
    assert_eq!(
        world.xp_for(1, 100, best),
        100,
        "the best catches nobody up"
    );
    assert_eq!(
        world.xp_for(5, 100, best),
        100,
        "a bot gets the plain share"
    );
}
