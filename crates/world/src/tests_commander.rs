//! The commander class (feature 78, its ranked kit task 129):
//! `crate::class`'s fifth class. His start; the sixteen levels and a skill
//! point a level; the base trait — hiring at a discount; and the four
//! abilities a rank at a time — Battle Cry, Medivac, Rally and
//! Reinforcements — each doing what its rank says. (His squad orders were
//! removed.)

use bims::combat::{Gear, Tier, WeaponKind};
use bims::droid::{DroidKind, DroidPart};
use bims::math::vec2;
use shipdesign::fixture::combat_ship;

use crate::class::{self, Class};
use crate::event::{Refusal, WorldEvent};
use crate::fixture::{REFERENCE_MONEY, crewed_world};
use crate::world::{Command, World};
use crate::world_checksum;

const TILE: f32 = shipdesign::TILE as f32;

/// Seconds of the room's clock a world step is.
const SECONDS_A_STEP: f64 = crate::data::STEP_MINUTES / time::MINUTES_PER_SECOND;

/// The combat ship — bunks for five, armour and guns in the hold — with
/// three players and three crewmates nobody steers.
fn basic() -> World {
    crewed_world(combat_ship(), REFERENCE_MONEY, 3, 6)
}

/// [`basic`] with slot 0 a commander, one step taken so the room has
/// been handed its skills.
fn commander() -> World {
    let mut world = basic();
    assert_eq!(world.set_class(0, Class::Commander), Ok(()));
    world.step(&[]);
    world
}

fn refused_with(events: &[WorldEvent], want: Refusal) -> bool {
    events
        .iter()
        .any(|e| matches!(e, WorldEvent::Refused { why, .. } if *why == want))
}

/// Straight to a level, off the class's own table.
fn level_up(world: &mut World, who: usize, level: u8) {
    let mut events = Vec::new();
    let want = class::LEVEL_XP[level as usize - 1];
    let have = world.progress_of(who as u32).xp;
    world.award(who, want.saturating_sub(have), &mut events);
    assert!(world.level_of(who as u32) >= level);
}

/// Ranks bought one at a time through `Command::RankUp`, Q C E R, the
/// level raised first to what the highest wants.
fn ranks(world: &mut World, who: u32, want: [u8; 4]) {
    let class = world.class_of(who);
    let need = (0..4u8)
        .filter_map(|slot| class::rank_level(class, slot, want[slot as usize]))
        .chain(std::iter::once(want.iter().sum::<u8>().max(1)))
        .max()
        .unwrap_or(1);
    if world.level_of(who) < need {
        level_up(world, who as usize, need);
    }
    for (slot, &rank) in want.iter().enumerate() {
        while world.rank_of(who, slot as u8) < rank {
            let events = world.step(&[Command::RankUp {
                slot: who,
                ability_slot: slot as u32,
            }]);
            assert!(
                events
                    .iter()
                    .any(|e| matches!(e, WorldEvent::RankedUp { .. })),
                "rank {rank} of slot {slot}: {events:?}"
            );
        }
    }
}

/// The crew held where they stand: their own errands off, and nobody
/// doctoring of their own accord.
fn hold_still(world: &mut World) {
    world.aboard.room.set_autonomous(false);
    for who in 0..world.aboard.crew_count() as usize {
        let at = world.aboard.room.bim_pos(who);
        world.aboard.room.put_for_probe(who, at);
    }
    world
        .aboard
        .room
        .set_work_priority(bims::work::Job::Medical as u32, bims::work::NEVER);
}

/// Stand crew member `who` `tiles` tiles from the commander at slot 0,
/// on the deck.
fn stand_near(world: &mut World, who: usize, tiles: f32) {
    let at = world.aboard.room.bim_pos(0) + vec2(tiles * TILE, 0.0);
    world.aboard.room.put_for_probe(who, at);
}

/// The skill the world last handed the room for a crew member.
fn skill(world: &World, who: usize) -> bims::combat::Skill {
    world.aboard.room.skill_for_probe(who)
}

/// Steps enough for `seconds` of the mission clock, the commander kept
/// whole.
fn run_for(world: &mut World, seconds: f64) {
    for _ in 0..(seconds / SECONDS_A_STEP).ceil() as u32 {
        world.aboard.room.patch_up_for_probe(0);
        world.step(&[]);
    }
}

/// The mission left — if one is under way — and another begun somewhere
/// else in the system, the way the crew do it: back to the map, a
/// proposal and every yes. The events of the step the trip was taken.
fn next_mission(world: &mut World) -> Vec<WorldEvent> {
    if world.run.phase == crate::run::Phase::Mission {
        world.leave_for_probe();
    }
    let here = world.current_site();
    let site = world
        .sites_at(world.star_id)
        .into_iter()
        .find(|&s| Some(s) != here && world.travel_quote(s).is_ok_and(|q| !q.trader))
        .expect("somewhere else to go");
    let mut events = world.step(&[Command::Propose {
        slot: 0,
        star: site.star,
        station: site.station,
    }]);
    for slot in 1..world.players() {
        events.extend(world.step(&[Command::Accept { slot, yes: true }]));
    }
    assert_eq!(world.run.phase, crate::run::Phase::Mission);
    events
}

// --- A: the class, the start, the levels, hiring ---------------------------

#[test]
fn the_commander_sets_out_with_the_pistol_and_the_pool_is_unchanged() {
    let mut world = basic();
    let money = world.money;
    for (a, b) in [
        (Class::Commander, Class::None),
        (Class::Commander, Class::Commander),
        (Class::Commander, Class::Tank),
        (Class::None, Class::None),
    ] {
        assert_eq!(world.set_class(0, a), Ok(()));
        assert_eq!(world.set_class(1, b), Ok(()));
        assert_eq!(world.money, money, "{a:?} and {b:?}: the same pool");
    }
    assert_eq!(world.set_class(0, Class::Commander), Ok(()));
    assert_eq!(
        world.aboard.room.weapon(0),
        Some(WeaponKind::LaserPistol.basic()),
        "the laser pistol he had in hand"
    );
    // He brings nothing else, and a class put back to none takes
    // nothing off him.
    let gear = world.aboard.room.gear(0);
    assert_eq!(world.set_class(0, Class::None), Ok(()));
    assert_eq!(world.aboard.room.gear(0), gear);
    assert_eq!(
        world.aboard.room.weapon(0),
        Some(WeaponKind::LaserPistol.basic())
    );
}

#[test]
fn a_hire_a_commander_makes_is_cheaper_and_nobody_s_experience() {
    let mut world = basic();
    assert_eq!(world.set_class(0, Class::Commander), Ok(()));
    assert_eq!(world.set_class(1, Class::Commander), Ok(()));
    world.step(&[]);
    if !world.mercenary_for_probe() {
        return;
    }
    let merc = world.residents.as_ref().unwrap().aboard.count() - 1;
    let full = world.mercenary_fee(merc).expect("a fee");
    let discounted = world.hire_fee(0, merc).expect("a fee");
    assert_eq!(
        discounted,
        full - full * u64::from(class::HIRE_DISCOUNT_PERCENT) / 100,
        "a quarter off, rounded down"
    );
    assert!(discounted < full);
    // Anybody else pays in full.
    assert_eq!(world.hire_fee(2, merc), Some(full));
    // A refused hire gives nothing: nobody is within reach yet.
    let before = world.progress_of(0).xp;
    let events = world.step(&[Command::Hire {
        slot: 0,
        who: 0,
        resident: merc,
    }]);
    assert!(refused_with(&events, Refusal::OutOfReach));
    assert_eq!(world.progress_of(0).xp, before, "a refused hire is nothing");
    // Walked over, the hire goes through at the discounted fee, and
    // nobody learns anything by it (task 119).
    let at = world
        .body_position(crate::LootSource::Resident(merc))
        .expect("alongside");
    world.aboard.room.put_for_probe(0, at);
    let money = world.wallet(0);
    let events = world.step(&[Command::Hire {
        slot: 0,
        who: 0,
        resident: merc,
    }]);
    assert!(
        events.iter().any(|e| matches!(e, WorldEvent::Hired { .. })),
        "{events:?}"
    );
    assert_eq!(world.wallet(0), money - discounted);
    let hired = world.hired().last().expect("a contract");
    assert_eq!(hired.fee, discounted, "the contract's fee is the discount");
    assert_eq!(world.progress_of(0).xp, before, "the commander who hired");
    assert_eq!(world.progress_of(1).xp, 0, "the other commander");
    // And it stays that hand's fee after he dies.
    let hired_who = hired.who;
    world.aboard.room.kill_for_probe(0);
    world.step(&[]);
    assert_eq!(world.hired().last().expect("still hired").fee, discounted);
    assert!(world.is_hired(hired_who));
}

#[test]
fn a_hire_by_anybody_else_pays_in_full_and_gives_no_experience() {
    let mut world = basic();
    assert_eq!(world.set_class(0, Class::Commander), Ok(()));
    world.step(&[]);
    if !world.mercenary_for_probe() {
        return;
    }
    let merc = world.residents.as_ref().unwrap().aboard.count() - 1;
    let full = world.mercenary_fee(merc).expect("a fee");
    let at = world
        .body_position(crate::LootSource::Resident(merc))
        .expect("alongside");
    // Crew member 2 does the hiring, standing beside the commander.
    world.aboard.room.put_for_probe(2, at);
    world.aboard.room.put_for_probe(0, at);
    let money = world.wallet(2);
    let events = world.step(&[Command::Hire {
        slot: 2,
        who: 2,
        resident: merc,
    }]);
    assert!(events.iter().any(|e| matches!(e, WorldEvent::Hired { .. })));
    assert_eq!(world.wallet(2), money - full, "the full fee");
    assert_eq!(
        world.progress_of(0).xp,
        0,
        "the commander standing beside them learns nothing"
    );
}

// --- B: the ranked kit (task 129) ------------------------------------------

#[test]
fn the_commander_climbs_sixteen_levels_and_buys_his_ranks_as_the_soldier_does() {
    let mut world = basic();
    assert_eq!(world.set_class(0, Class::Commander), Ok(()));
    assert_eq!(world.set_class(1, Class::None), Ok(()));
    assert_eq!(world.set_class(2, Class::Tank), Ok(()));
    let c = Class::Commander;
    assert!(class::ranked(c));
    assert_eq!(class::LEVELS, 20);
    assert_eq!(class::level_of(3_199), 15);
    assert_eq!(class::level_of(3_200), 16, "the sixteenth at 3 200");
    let rank_up = |world: &mut World, slot: u32, ability_slot: u32| {
        world.step(&[Command::RankUp { slot, ability_slot }])
    };
    // No class is refused; the tank's kit is ranked since task 139.
    assert!(refused_with(&rank_up(&mut world, 1, 0), Refusal::NoClass));
    assert!(
        rank_up(&mut world, 2, 0)
            .iter()
            .any(|e| matches!(e, WorldEvent::RankedUp { who: 2, .. }))
    );
    // The ultimate wants the sixth level; the first level's point buys Q.
    assert!(refused_with(
        &rank_up(&mut world, 0, 3),
        Refusal::RankLocked
    ));
    let events = rank_up(&mut world, 0, 0);
    assert!(
        events.iter().any(|e| matches!(
            e,
            WorldEvent::RankedUp {
                who: 0,
                class: 5,
                ability_slot: 0,
                rank: 1
            }
        )),
        "{events:?}"
    );
    assert!(refused_with(
        &rank_up(&mut world, 0, 1),
        Refusal::NoSkillPoint
    ));
    // The soldier's gates: Q, C and E rank n at 2n − 1, R at 6, 9, 12, 15.
    for slot in 0..4u8 {
        for rank in 1..=4u8 {
            let want = if slot == class::SLOT_R {
                [6, 9, 12, 15][rank as usize - 1]
            } else {
                2 * rank - 1
            };
            assert_eq!(class::rank_level(c, slot, rank), Some(want));
        }
    }
    level_up(&mut world, 0, 2);
    assert!(refused_with(
        &rank_up(&mut world, 0, 0),
        Refusal::RankLocked
    ));
    level_up(&mut world, 0, 16);
    assert_eq!(world.points_of(0), 15);
    for _ in 0..4 {
        rank_up(&mut world, 0, 3);
    }
    assert_eq!(world.rank_of(0, 3), 4);
    assert!(refused_with(&rank_up(&mut world, 0, 3), Refusal::TopRank));
}

#[test]
fn battle_cry_fires_faster_for_its_seconds_by_rank_and_waits_its_cooldown() {
    let mut world = commander();
    let events = world.step(&[Command::BattleCry { slot: 0 }]);
    assert!(refused_with(&events, Refusal::NotLearnt), "{events:?}");
    let events = world.step(&[Command::BattleCry { slot: 1 }]);
    assert!(refused_with(&events, Refusal::NotACommander));
    let want = [
        (1.25, 3.0, 20.0),
        (1.30, 4.0, 18.0),
        (1.35, 5.0, 16.0),
        (1.40, 6.0, 14.0),
    ];
    for (rank, &(rate, seconds, cooldown)) in (1..=4u8).zip(&want) {
        let mut world = commander();
        hold_still(&mut world);
        ranks(&mut world, 0, [rank, 0, 0, 0]);
        stand_near(&mut world, 1, 2.0);
        world.step(&[]);
        assert_eq!(world.battle_cry_seconds(0), seconds, "rank {rank}");
        assert_eq!(world.battle_cry_cooldown(0), cooldown, "rank {rank}");
        let before = world.skill_of(1).fire_rate;
        let events = world.step(&[Command::BattleCry { slot: 0 }]);
        assert!(
            events
                .iter()
                .any(|e| matches!(e, WorldEvent::BattleCried { who: 0 })),
            "{events:?}"
        );
        assert!(world.is_crying(0));
        assert!((world.skill_of(1).fire_rate - before * rate).abs() < 1e-5);
        assert!(
            (world.skill_of(0).fire_rate - rate).abs() < 1e-5,
            "himself too"
        );
        world.step(&[]);
        assert_eq!(skill(&world, 1), world.skill_of(1), "the room is told");
        let events = world.step(&[Command::BattleCry { slot: 0 }]);
        assert!(refused_with(&events, Refusal::CoolingDown));
        run_for(&mut world, seconds - 0.5);
        assert!(world.is_crying(0), "rank {rank}: still on");
        run_for(&mut world, 1.0);
        assert!(!world.is_crying(0), "rank {rank}: over");
        assert_eq!(world.skill_of(1).fire_rate, before);
        let left = world.battle_cry_cooldown_left(0);
        assert!(
            left > 0.0 && left <= cooldown - seconds + 1.0,
            "rank {rank}: {left}"
        );
    }
}

#[test]
fn battle_cry_reaches_those_near_at_the_call_and_keeps_to_them() {
    let mut world = commander();
    hold_still(&mut world);
    ranks(&mut world, 0, [1, 0, 0, 0]);
    stand_near(&mut world, 1, 3.0);
    stand_near(&mut world, 2, class::BATTLE_CRY_TILES + 4.0);
    world.step(&[]);
    world.step(&[Command::BattleCry { slot: 0 }]);
    assert_eq!(world.battle_cry_reaching(0), Some(0));
    assert_eq!(world.battle_cry_reaching(1), Some(0), "within eight");
    assert_eq!(world.battle_cry_reaching(2), None, "none outside");
    // The one within walks out and keeps it; the one outside walks in and
    // is given nothing.
    stand_near(&mut world, 1, class::BATTLE_CRY_TILES + 4.0);
    stand_near(&mut world, 2, 1.0);
    world.step(&[]);
    assert_eq!(world.battle_cry_reaching(1), Some(0), "walked out, kept");
    assert_eq!(world.battle_cry_reaching(2), None, "walked in, nothing");
    assert!(world.skill_of(1).fire_rate > 1.0);
    assert_eq!(world.skill_of(2).fire_rate, 1.0);
}

#[test]
fn battle_cry_is_refused_downed_multiplies_with_a_rampage_and_lifts_no_sentry() {
    let mut world = basic();
    assert_eq!(world.set_class(0, Class::Commander), Ok(()));
    assert_eq!(world.set_class(1, Class::Soldier), Ok(()));
    assert_eq!(world.set_class(2, Class::Engineer), Ok(()));
    world.step(&[]);
    hold_still(&mut world);
    ranks(&mut world, 0, [1, 0, 1, 0]);
    ranks(&mut world, 1, [0, 0, 0, 1]);
    ranks(&mut world, 2, [0, 0, 0, 2]);
    stand_near(&mut world, 1, 1.0);
    stand_near(&mut world, 2, 2.0);
    // Downed, he cries nothing and rallies nothing.
    world.aboard.room.knock_out_for_probe(0);
    world.step(&[]);
    assert_eq!(world.can_battle_cry(0), Err(Refusal::OutOfReach));
    assert_eq!(world.can_rally(0), Err(Refusal::OutOfReach));
    world.aboard.room.patch_up_for_probe(0);
    world.step(&[]);
    world.step(&[]);
    assert_eq!(world.can_battle_cry(0), Ok(()));
    // The soldier on a Rampage and in the cry: both factors.
    world.step(&[Command::Rampage { slot: 1 }]);
    world.step(&[Command::BattleCry { slot: 0 }]);
    let s = world.skill_of(1);
    let want = class::RAMPAGE_FIRE_RATE[0] * class::BATTLE_CRY_FIRE_RATE[0];
    assert!((s.fire_rate - want).abs() < 1e-5, "{}", s.fire_rate);
    // The engineer is lifted and his sentry is not: a sentry's skill is
    // its own.
    assert_eq!(world.battle_cry_reaching(2), Some(0));
    let sentry = world.sentry_skill(2);
    assert_eq!(sentry.fire_rate, class::SENTRY_FIRE_RATE[1]);
    assert!(world.skill_of(2).fire_rate > 1.0, "the engineer's own is");
}

#[test]
fn battle_cry_and_rally_are_ready_at_every_mission_and_shortened_by_the_relics() {
    let mut world = commander();
    ranks(&mut world, 0, [1, 0, 1, 0]);
    let cry = world.battle_cry_cooldown(0);
    let rally = world.rally_cooldown(0);
    world.give_relic_for_probe(crate::relic::Relic::OverclockedCores);
    assert!(world.battle_cry_cooldown(0) < cry, "*Overclocked Cores*");
    assert!(world.rally_cooldown(0) < rally, "*Overclocked Cores*");
    world.step(&[Command::BattleCry { slot: 0 }]);
    world.step(&[Command::Rally { slot: 0 }]);
    run_for(&mut world, 12.0);
    assert!(world.battle_cry_cooldown_left(0) > 0.0);
    assert!(world.rally_cooldown_left(0) > 0.0);
    // Seconds taken off (a *Reset Capacitor*'s way) both once their shouts are
    // over.
    let (cry_left, rally_left) = (
        world.battle_cry_cooldown_left(0),
        world.rally_cooldown_left(0),
    );
    world.cooldowns_less(0, 3.0);
    assert!(world.battle_cry_cooldown_left(0) < cry_left, "seconds off");
    assert!(world.rally_cooldown_left(0) < rally_left, "seconds off");
    next_mission(&mut world);
    assert_eq!(world.battle_cry_cooldown_left(0), 0.0);
    assert_eq!(world.rally_cooldown_left(0), 0.0);
    assert_eq!(world.can_battle_cry(0), Ok(()));
    assert_eq!(world.can_rally(0), Ok(()));
    assert!(world.commander_of(0).cried.is_empty());
}

// --- C: the Medivac -----------------------------------------------------------

/// Every medic called in, off a step's events: who called and who came.
fn medivacs(events: &[WorldEvent]) -> Vec<(u32, u32)> {
    events
        .iter()
        .filter_map(|e| match *e {
            WorldEvent::Medivac { who, medic } => Some((who, medic)),
            _ => None,
        })
        .collect()
}

/// C calls one medic of the Republic's in: the pistol, the rank's armour,
/// a medic's revive, the Republic's medic's look, and none of the R's.
#[test]
fn the_medivac_calls_a_medic_in_armoured_by_rank() {
    use bims::character::{Outfit, Tint};
    use bims::combat::ArmourKind;
    let vests = [None, Some(Tier::One), Some(Tier::Three), Some(Tier::Three)];
    for (rank, &vest) in (1..=4u8).zip(&vests) {
        let mut world = commander();
        ranks(&mut world, 0, [0, rank, 0, 0]);
        next_mission(&mut world);
        let crew = world.aboard.crew_count();
        let events = world.step(&[Command::Medivac { slot: 0 }]);
        assert_eq!(medivacs(&events), vec![(0, crew)], "rank {rank}");
        assert_eq!(world.medivacs_of(0), vec![crew], "rank {rank}");
        assert!(world.reinforcements_of(0).is_empty(), "no soldier of R's");
        assert!(world.is_medivac(crew) && world.is_reinforcement(crew));
        assert_eq!(world.class_of(crew), Class::None);
        let gear = world.aboard.room.gear(crew as usize);
        assert_eq!(gear.weapon, Some(WeaponKind::LaserPistol.basic()));
        let armour = gear.armour.map(|p| (p.kind, p.tier));
        assert_eq!(armour, vest.map(|t| (ArmourKind::Armour, t)), "rank {rank}");
        let skill = world.skill_of(crew);
        assert!(skill.medic, "a medic to the room");
        assert_eq!(skill.revive, class::MEDIC_REVIVE_SECONDS);
        assert_eq!(
            world.medivac_cooldown(0),
            class::MEDIVAC_COOLDOWN[rank as usize - 1]
        );
        world.step(&[]);
        assert_eq!(
            world.aboard.room.outfit(crew as usize),
            Outfit::RepublicMedic(Tint::ALL[0])
        );
        assert!(world.aboard.room.is_medivac(crew as usize));
    }
}

/// C is pressed as often as the rank's cooldown allows, the medics called
/// before stay, and it is ready again at every mission's start, the
/// medics gone.
#[test]
fn the_medivac_is_on_its_rank_s_cooldown() {
    let mut world = commander();
    let events = world.step(&[Command::Medivac { slot: 0 }]);
    assert!(refused_with(&events, Refusal::NotLearnt), "{events:?}");
    ranks(&mut world, 0, [0, 4, 0, 0]);
    assert_eq!(world.medivac_cooldown(0), 110.0);
    assert_eq!(
        medivacs(&world.step(&[Command::Medivac { slot: 0 }])).len(),
        1
    );
    run_for(&mut world, 60.0);
    let events = world.step(&[Command::Medivac { slot: 0 }]);
    assert!(refused_with(&events, Refusal::CoolingDown), "{events:?}");
    run_for(&mut world, 51.0);
    assert_eq!(world.medivac_cooldown_left(0), 0.0);
    assert_eq!(
        medivacs(&world.step(&[Command::Medivac { slot: 0 }])).len(),
        1
    );
    assert_eq!(world.medivacs_of(0).len(), 2);
    // Its own cooldown: R is untouched by it.
    assert_eq!(world.reinforcement_cooldown_left(0), 0.0);
    next_mission(&mut world);
    assert!(world.medivacs_of(0).is_empty());
    assert_eq!(world.medivac_cooldown_left(0), 0.0);
    assert_eq!(world.can_medivac(0), Ok(()));
    // Not between missions, and nobody's but a commander's.
    world.leave_for_probe();
    assert_eq!(world.can_medivac(0), Err(Refusal::OutOfReach));
    assert_eq!(world.can_medivac(1), Err(Refusal::NotACommander));
}

/// The medic's kit is the Republic's as a soldier's is: nothing put on
/// him, taken off him or moved off him onto anybody else.
#[test]
fn a_medivac_medic_s_kit_is_nobody_s_to_change() {
    use crate::holdings::{GearSlot, GearSource};
    use bims::combat::Item;
    let mut world = commander();
    ranks(&mut world, 0, [0, 2, 0, 0]);
    next_mission(&mut world);
    world.step(&[Command::Medivac { slot: 0 }]);
    let medic = world.medivacs_of(0)[0];
    let gear = world.aboard.room.gear(medic as usize);
    assert!(!world.may_change(0, medic) && !world.may_change_now(0, medic));
    let helm = world
        .holdings
        .put(Item::Armour(bims::combat::Piece::new(
            9_000,
            bims::combat::ArmourKind::Armour,
            Tier::One,
        )))
        .unwrap();
    for command in [
        Command::Equip {
            slot: 0,
            who: medic,
            from: GearSource::Armory { id: helm },
        },
        Command::Unequip {
            slot: 0,
            who: medic,
            part: GearSlot::Armour,
        },
        Command::Equip {
            slot: 0,
            who: 0,
            from: GearSource::Worn {
                who: medic,
                slot: GearSlot::Armour,
            },
        },
    ] {
        let events = world.step(&[command]);
        assert!(
            refused_with(&events, Refusal::NotYours),
            "{command:?}: {events:?}"
        );
        let now = world.aboard.room.gear(medic as usize);
        assert_eq!(now.weapon, gear.weapon);
        assert_eq!(now.armour.map(|p| p.id), gear.armour.map(|p| p.id));
    }
}

/// A critical hit by crew member `by` on the staged machine's chassis,
/// landed the way a bolt lands it with `damage` and `flat`, and what the
/// chassis lost.
fn crit_on_machine(world: &mut World, by: usize, damage: f32, flat: f32) -> f32 {
    let residents = world.residents.as_ref().unwrap();
    let bims = residents.aboard.room.crew_count() as usize;
    let chassis = |world: &World| {
        world
            .residents
            .as_ref()
            .unwrap()
            .aboard
            .room
            .droid(0)
            .unwrap()
            .body
            .health(DroidPart::Chassis)
    };
    let before = chassis(world);
    let roll = (0..1000)
        .map(|k| k as f32 / 1000.0 + 0.0005)
        .find(|&r| DroidPart::hit_by(r) == DroidPart::Chassis)
        .unwrap();
    world.aboard.room.land_hit_for_probe(bims::combat::Hit {
        who: bims,
        damage,
        cut: false,
        by: Some(by),
        blast: false,
        roll,
        strips: 0.0,
        flat,
        crit: true,
    });
    world.step(&[]);
    before - chassis(world)
}

#[test]
fn weak_spot_s_crit_on_a_lifted_hit_is_the_flat_damage_s_share_alone() {
    let mut world = basic();
    assert_eq!(world.set_class(0, Class::Commander), Ok(()));
    assert_eq!(world.set_class(1, Class::Soldier), Ok(()));
    assert!(world.stage_droid_fight_for_probe(DroidKind::Warden, None));
    world.step(&[]);
    hold_still(&mut world);
    // Nobody fires but the hit landed by hand.
    for who in 0..world.aboard.crew_count() as usize {
        world.aboard.room.issue(who, Gear::default());
    }
    ranks(&mut world, 0, [0, 4, 0, 0]);
    ranks(&mut world, 1, [0, 4, 0, 0]);
    stand_near(&mut world, 1, 1.0);
    world.step(&[]);
    // A bolt landed at half again its flat damage (a relic, a cry): the
    // crit adds its share of the flat damage, and never a share of the
    // lift.
    let aura = 1.5;
    let took = crit_on_machine(&mut world, 1, 10.0 * aura, 10.0);
    let bonus = took - 10.0 * aura;
    assert!(
        (bonus - 10.0 * (class::WEAK_SPOT_DAMAGE[3] - 1.0)).abs() < 1e-3,
        "the crit's share {bonus}"
    );
    // A commander's own slot C is his Medivac, not a Weak Spot: a hit said
    // critical by him adds nothing.
    let took = crit_on_machine(&mut world, 0, 10.0, 10.0);
    assert!((took - 10.0).abs() < 1e-3, "{took}");
}

#[test]
fn rally_takes_less_and_walks_faster_for_its_seconds_by_rank() {
    let want = [
        (0.85, 1.10, 6.0, 45.0),
        (0.80, 1.15, 7.0, 40.0),
        (0.75, 1.20, 8.0, 35.0),
        (0.70, 1.20, 9.0, 30.0),
    ];
    for (rank, &(taken, pace, seconds, cooldown)) in (1..=4u8).zip(&want) {
        let mut world = commander();
        hold_still(&mut world);
        ranks(&mut world, 0, [0, 0, rank, 0]);
        stand_near(&mut world, 1, 2.0);
        world.step(&[]);
        assert_eq!(world.rally_seconds(0), seconds, "rank {rank}");
        assert_eq!(world.rally_cooldown(0), cooldown, "rank {rank}");
        let events = world.step(&[Command::Rally { slot: 0 }]);
        assert!(
            events
                .iter()
                .any(|e| matches!(e, WorldEvent::Rallied { who: 0 })),
            "{events:?}"
        );
        for who in [0, 1] {
            let s = world.skill_of(who);
            assert!((s.damage_taken - taken).abs() < 1e-5, "rank {rank}");
            assert!((s.walk - pace).abs() < 1e-5, "rank {rank}");
            assert_eq!(s.accuracy, 1.0, "no aim: that went");
        }
        let events = world.step(&[Command::Rally { slot: 0 }]);
        assert!(refused_with(&events, Refusal::CoolingDown));
        run_for(&mut world, seconds - 0.5);
        assert!(world.is_rallying(0), "rank {rank}: still on");
        run_for(&mut world, 1.0);
        assert!(!world.is_rallying(0), "rank {rank}: over");
        assert_eq!(world.skill_of(1).damage_taken, 1.0);
        assert_eq!(world.skill_of(1).walk, 1.0);
        assert!(world.rally_cooldown_left(0) > 0.0);
    }
    // Refused before its first rank.
    let mut world = commander();
    let events = world.step(&[Command::Rally { slot: 0 }]);
    assert!(refused_with(&events, Refusal::NotLearnt), "{events:?}");
}

#[test]
fn rally_reaches_those_near_at_the_call_and_keeps_to_them() {
    let mut world = commander();
    hold_still(&mut world);
    ranks(&mut world, 0, [0, 0, 1, 0]);
    stand_near(&mut world, 1, 3.0);
    stand_near(&mut world, 2, class::RALLY_TILES + 4.0);
    world.step(&[]);
    world.step(&[Command::Rally { slot: 0 }]);
    let rallied = world.commander_of(0).rallied;
    assert!(rallied.contains(&0) && rallied.contains(&1), "{rallied:?}");
    assert!(!rallied.contains(&2), "{rallied:?}");
    stand_near(&mut world, 1, class::RALLY_TILES + 4.0);
    stand_near(&mut world, 2, 1.0);
    world.step(&[]);
    assert_eq!(world.rally_reaching(1), Some(0), "walked out, kept");
    assert_eq!(world.rally_reaching(2), None, "walked in, nothing");
    assert!(world.skill_of(1).damage_taken < 1.0);
    assert_eq!(world.skill_of(2).damage_taken, 1.0);
}

// --- C: the Reinforcements -------------------------------------------------

/// Every reinforcement's events said in the steps a mission's start took.
fn reinforced(events: &[WorldEvent]) -> Vec<(u32, u32)> {
    events
        .iter()
        .filter_map(|e| match *e {
            WorldEvent::Reinforced { who, count } => Some((who, count)),
            _ => None,
        })
        .collect()
}

#[test]
fn reinforcements_are_called_in_with_r_by_rank() {
    let tiers = [Tier::One, Tier::One, Tier::Two, Tier::Three];
    for (rank, (&count, &tier)) in (1..=4u8).zip(class::REINFORCEMENTS.iter().zip(&tiers)) {
        let mut world = commander();
        ranks(&mut world, 0, [0, 0, 0, rank]);
        // Nobody comes of their own accord, now or at a mission's start.
        world.step(&[]);
        assert!(world.reinforcements_of(0).is_empty(), "rank {rank}");
        next_mission(&mut world);
        assert!(world.reinforcements_of(0).is_empty(), "rank {rank}");
        let crew = world.aboard.crew_count();
        let events = world.step(&[Command::Reinforce { slot: 0 }]);
        assert_eq!(reinforced(&events), vec![(0, count)], "rank {rank}");
        let brought = world.reinforcements_of(0);
        assert_eq!(brought.len() as u32, count, "rank {rank}");
        assert_eq!(world.aboard.crew_count(), crew + count);
        let at = world.aboard.room.bim_pos(0);
        for &who in &brought {
            assert!(who >= crew, "on the end of the crew");
            assert_eq!(world.reinforcement_of(who), Some(0));
            assert_eq!(world.class_of(who), Class::None);
            let gear = world.aboard.room.gear(who as usize);
            assert_eq!(gear.weapon, Some(WeaponKind::AutoRifle.at(tier)));
            assert!(gear.armour.is_none());
            assert_eq!(
                world.aboard.room.health(who as usize),
                bims::health::MAX_HEALTH
            );
            let far = (world.aboard.room.bim_pos(who as usize) - at).len() / TILE;
            assert!(far <= class::REINFORCEMENT_REACH_TILES + 0.5, "{far}");
        }
    }
}

/// R is pressed as often as its 140 seconds allow, those called before
/// stay, and it is ready again at every mission's start.
#[test]
fn reinforcements_are_on_a_hundred_and_forty_second_cooldown() {
    let mut world = commander();
    // Not learnt: refused.
    let events = world.step(&[Command::Reinforce { slot: 0 }]);
    assert!(refused_with(&events, Refusal::NotLearnt), "{events:?}");
    ranks(&mut world, 0, [0, 0, 0, 1]);
    let events = world.step(&[Command::Reinforce { slot: 0 }]);
    assert_eq!(reinforced(&events), vec![(0, 2)]);
    assert_eq!(
        world.reinforcement_cooldown(0),
        class::REINFORCEMENT_COOLDOWN
    );
    assert!(world.reinforcement_cooldown_left(0) > 139.0);
    // Not a second time within the cooldown.
    run_for(&mut world, 60.0);
    let events = world.step(&[Command::Reinforce { slot: 0 }]);
    assert!(refused_with(&events, Refusal::CoolingDown), "{events:?}");
    assert_eq!(world.reinforcements_of(0).len(), 2);
    // Past it: two more, and the first two still there.
    run_for(&mut world, 81.0);
    assert_eq!(world.reinforcement_cooldown_left(0), 0.0);
    let events = world.step(&[Command::Reinforce { slot: 0 }]);
    assert_eq!(reinforced(&events), vec![(0, 2)], "{events:?}");
    assert_eq!(world.reinforcements_of(0).len(), 4);
    // A mission's start: they are gone, and R is ready again.
    next_mission(&mut world);
    assert!(world.reinforcements_of(0).is_empty());
    assert_eq!(world.reinforcement_cooldown_left(0), 0.0);
    let events = world.step(&[Command::Reinforce { slot: 0 }]);
    assert_eq!(reinforced(&events), vec![(0, 2)]);
    // Not between missions.
    world.leave_for_probe();
    assert_eq!(world.can_reinforce(0), Err(Refusal::OutOfReach));
    let events = world.step(&[Command::Reinforce { slot: 0 }]);
    assert!(
        refused_with(&events, Refusal::BetweenMissions),
        "{events:?}"
    );
}

/// A reinforcement keeps the Republic's rifle it came with: nothing is
/// put on it, taken off it or moved off it onto anybody else, and it wears
/// the Republic's armour in its caller's colour.
#[test]
fn a_reinforcement_keeps_its_rifle_and_wears_the_republic_s_armour() {
    use crate::holdings::{GearSlot, GearSource};
    use bims::character::{Outfit, Tint};
    use bims::combat::Item;
    let mut world = commander();
    ranks(&mut world, 0, [0, 0, 0, 1]);
    next_mission(&mut world);
    world.step(&[Command::Reinforce { slot: 0 }]);
    let soldier = world.reinforcements_of(0)[0];
    let rifle = world.aboard.room.weapon(soldier as usize);
    assert!(rifle.is_some());
    assert_eq!(
        world.aboard.room.outfit(soldier as usize),
        Outfit::Republic(Tint::ALL[0])
    );
    assert!(!world.may_change(0, soldier) && !world.may_change_now(0, soldier));
    let sniper = world
        .holdings
        .put(Item::Weapon(WeaponKind::SniperRifle.basic()))
        .unwrap();
    let armory = world.holdings.armory.len();
    for command in [
        Command::Equip {
            slot: 0,
            who: soldier,
            from: GearSource::Armory { id: sniper },
        },
        Command::Unequip {
            slot: 0,
            who: soldier,
            part: GearSlot::Weapon,
        },
        Command::Equip {
            slot: 0,
            who: 0,
            from: GearSource::Worn {
                who: soldier,
                slot: GearSlot::Weapon,
            },
        },
    ] {
        let events = world.step(&[command]);
        assert!(
            events
                .iter()
                .any(|e| matches!(e, WorldEvent::Refused { .. })),
            "{command:?}: {events:?}"
        );
        assert_eq!(world.aboard.room.weapon(soldier as usize), rifle);
        assert_eq!(world.holdings.armory.len(), armory);
    }
    // The Republic's rifle is never the crew's.
    let events = world.step(&[Command::Unequip {
        slot: 0,
        who: soldier,
        part: GearSlot::Weapon,
    }]);
    assert!(refused_with(&events, Refusal::NotYours), "{events:?}");
}

#[test]
fn fewer_arrive_where_the_free_deck_round_him_is_short() {
    let mut world = commander();
    ranks(&mut world, 0, [0, 0, 0, 4]);
    // Crowd the tiles round him with the crew: what is left is what comes.
    let at = world.aboard.room.bim_pos(0);
    let free = world
        .aboard
        .room
        .free_tiles_near(at, class::REINFORCEMENT_REACH_TILES * TILE);
    assert!(!free.is_empty());
    // Stand the crew on all but two of them, so two are left.
    let keep = 2usize;
    let crowd = free.len().saturating_sub(keep);
    let mut others: Vec<usize> = (1..world.aboard.crew_count() as usize).collect();
    let mut spots = free.clone().into_iter();
    let mut stood = 0;
    while stood < crowd {
        let Some(spot) = spots.next() else { break };
        match others.pop() {
            Some(who) => {
                world.aboard.room.put_for_probe(who, spot);
            }
            None => {
                // More tiles than crew: fill the rest with fresh bodies.
                let gear = Gear::default();
                world
                    .aboard
                    .room
                    .enlist_reinforcement(spot, gear, 7 + stood as u64);
            }
        }
        stood += 1;
    }
    world.aboard.crew = world.aboard.room.crew_count();
    let left = world
        .aboard
        .room
        .free_tiles_near(at, class::REINFORCEMENT_REACH_TILES * TILE);
    assert!(left.len() < 4, "{} free tiles left", left.len());
    let events = world.reinforce_for_probe();
    let came = world.reinforcements_of(0).len();
    assert_eq!(came, left.len(), "as many as the tiles found");
    assert_eq!(reinforced(&events), vec![(0, came as u32)]);
}

#[test]
fn a_reinforcement_is_downed_and_revived_under_the_crew_s_rules() {
    let mut world = commander();
    ranks(&mut world, 0, [0, 0, 0, 1]);
    world.reinforce_for_probe();
    let r = world.reinforcements_of(0)[0];
    world.aboard.room.set_autonomous(false);
    world.aboard.room.knock_out_for_probe(r as usize);
    world.step(&[]);
    world.step(&[]);
    assert!(world.aboard.room.is_downed(r as usize));
    // The player's own Bim, ordered to it, revives it.
    let at = world.aboard.room.bim_pos(r as usize) + vec2(TILE, 0.0);
    world.aboard.room.put_for_probe(0, at);
    world.step(&[Command::Crew {
        slot: 0,
        order: bims::order::CrewOrder::Revive { who: 0, patient: r },
    }]);
    run_for(&mut world, bims::health::REVIVE_SECONDS as f64 + 3.0);
    assert!(!world.aboard.room.is_downed(r as usize), "up again");
    assert!(world.aboard.room.is_alive(r as usize));
}

#[test]
fn a_dead_reinforcement_is_gone_at_once_and_all_go_at_the_mission_s_end() {
    let mut world = commander();
    ranks(&mut world, 0, [0, 0, 0, 4]);
    next_mission(&mut world);
    world.step(&[Command::Reinforce { slot: 0 }]);
    let brought = world.reinforcements_of(0);
    assert_eq!(brought.len(), 4);
    let crew = world.aboard.crew_count();
    let money = world.money;
    let armory = world.holdings.armory.len();
    let dead = brought[0];
    world.aboard.room.kill_for_probe(dead as usize);
    let mut events = world.step(&[]);
    events.extend(world.step(&[]));
    assert!(
        world.aboard.room.is_gone(dead as usize),
        "gone from the deck"
    );
    assert!(!world.aboard.room.body_seen(dead as usize), "drawn nowhere");
    assert!(
        !events
            .iter()
            .any(|e| matches!(e, WorldEvent::BotLost { .. })),
        "{events:?}"
    );
    assert_eq!(world.money, money, "no penalty");
    assert_eq!(world.aboard.crew_count(), crew, "nobody else's index moved");
    assert_eq!(world.reinforcements_of(0).len(), 3);
    run_for(&mut world, 5.0);
    assert!(world.aboard.room.is_gone(dead as usize), "and stays gone");
    assert!(!world.aboard.room.is_alive(dead as usize));
    // The mission's end: every one off the crew, alive or not, and their
    // rifles with them.
    world.leave_for_probe();
    assert_eq!(world.aboard.crew_count(), crew - 4);
    assert!(world.reinforcements.is_empty());
    assert_eq!(world.holdings.armory.len(), armory, "nothing dropped");
    assert_eq!(world.money, money, "nothing paid");
    // And the next mission they come again, fresh, all four, when he
    // calls them.
    next_mission(&mut world);
    assert!(world.reinforcements_of(0).is_empty());
    world.step(&[Command::Reinforce { slot: 0 }]);
    let again = world.reinforcements_of(0);
    assert_eq!(again.len(), 4);
    for &who in &again {
        assert!(!world.aboard.room.is_gone(who as usize));
        assert_eq!(
            world.aboard.room.health(who as usize),
            bims::health::MAX_HEALTH
        );
    }
}

#[test]
fn reinforcements_count_for_nothing_the_crew_is_counted_by() {
    let mut world = commander();
    ranks(&mut world, 0, [0, 0, 0, 4]);
    let worth = world.worth();
    world.reinforce_for_probe();
    let brought = world.reinforcements_of(0);
    assert_eq!(brought.len(), 4);
    assert_eq!(world.worth(), worth, "their rifles are not the crew's");
    // No experience: they have no class.
    let mut events = Vec::new();
    world.award(brought[0] as usize, 500, &mut events);
    assert_eq!(world.progress_of(brought[0]).xp, 0);
    // No departure waits for them and none asks about them.
    for &who in &brought {
        assert!(!world.left_behind().contains(&who));
    }
    // The run is lost when every player's Bim is dead, whoever stands.
    for slot in 0..world.players() {
        world.aboard.room.kill_for_probe(slot as usize);
    }
    let events = world.step(&[]);
    assert!(
        events.iter().any(|e| matches!(e, WorldEvent::CrewLost)),
        "{events:?}"
    );
    assert!(
        brought
            .iter()
            .all(|&w| world.aboard.room.is_alive(w as usize))
    );
}

#[test]
fn a_cry_a_rally_and_the_reinforcements_are_hashed_and_two_runs_agree() {
    let run = || {
        let mut world = commander();
        ranks(&mut world, 0, [1, 1, 1, 1]);
        let before = world_checksum(&world);
        world.step(&[Command::BattleCry { slot: 0 }]);
        let cried = world_checksum(&world);
        assert_ne!(cried, before, "a cry");
        world.step(&[Command::Rally { slot: 0 }]);
        world.reinforce_for_probe();
        let reinforced = world_checksum(&world);
        assert_ne!(reinforced, cried, "a rally and the reinforcements");
        for _ in 0..120 {
            world.step(&[]);
        }
        world_checksum(&world)
    };
    assert_eq!(run(), run());
}
