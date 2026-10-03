//! The tank class (feature 77; a ranked kit since task 139, reworked by
//! task 155): `crate::class`'s fourth class. The rank system and his two
//! base traits — the half-rate armour and the start — then Q Riot Shield,
//! C Plated, E Reflect Barrier and R Bastion, each doing what its rank
//! says.

use bims::combat::{ArmourKind, Item, Piece, Tier, WeaponKind};
use bims::droid::{DroidKind, DroidPart};
use bims::math::vec2;
use physics::ResourceId;
use shipdesign::fixture::combat_ship;

use crate::class::{self, Class, LEVEL_XP};
use crate::event::{Refusal, WorldEvent};
use crate::fixture::{REFERENCE_MONEY, simulation_world};
use crate::world::{Command, World};
use crate::world_checksum;

const TILE: f32 = shipdesign::TILE as f32;

/// Seconds of the mission clock a step of the world is.
const SECONDS_A_STEP: f64 = crate::data::STEP_MINUTES / time::MINUTES_PER_SECOND;

/// The combat ship — bunks for five, armour and guns in the hold — with
/// three players' crew.
fn basic() -> World {
    simulation_world(combat_ship(), REFERENCE_MONEY, 3)
}

/// [`basic`] with slot 0 a tank, one step taken so the room has been
/// handed its skills.
fn tank() -> World {
    let mut world = basic();
    assert_eq!(world.set_class(0, Class::Tank), Ok(()));
    world.step(&[]);
    world
}

/// [`tank`] with slot 0's ranks bought, Q C E R, and a step taken so the
/// room has them.
fn tank_at(want: [u8; 4]) -> World {
    let mut world = tank();
    ranks(&mut world, 0, want);
    world.step(&[]);
    world
}

fn refused_with(events: &[WorldEvent], want: Refusal) -> bool {
    events
        .iter()
        .any(|e| matches!(e, WorldEvent::Refused { why, .. } if *why == want))
}

/// Straight to a level, off the table.
fn level_up(world: &mut World, who: usize, level: u8) {
    let mut events = Vec::new();
    let want = LEVEL_XP[level as usize - 1];
    let have = world.progress_of(who as u32).xp;
    world.award(who, want.saturating_sub(have), &mut events);
    assert!(world.level_of(who as u32) >= level);
}

/// Ranks bought one at a time through `Command::RankUp`, Q C E R, the
/// level raised first to what the highest wants — for the class the crew
/// member is.
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

/// Steps for `seconds` of the mission clock, slot 0 patched up each.
fn run_for(world: &mut World, seconds: f64) {
    for _ in 0..(seconds / SECONDS_A_STEP).ceil() as u32 {
        world.aboard.room.patch_up_for_probe(0);
        world.step(&[]);
    }
}

/// The mission left — if one is under way — and another begun somewhere
/// else in the system, the way the crew do it.
fn next_mission(world: &mut World) {
    if world.run.phase == crate::run::Phase::Mission {
        world.leave_for_probe();
    }
    let here = world.current_site();
    let site = world
        .sites_at(world.star_id)
        .into_iter()
        .find(|&s| Some(s) != here && world.travel_quote(s).is_ok_and(|q| !q.trader))
        .expect("somewhere else to go");
    world.step(&[Command::Propose {
        slot: 0,
        star: site.star,
        station: site.station,
    }]);
    for slot in 1..world.players() {
        world.step(&[Command::Accept { slot, yes: true }]);
    }
    assert_eq!(world.run.phase, crate::run::Phase::Mission);
}

/// The crew held where they stand: their own errands off, and nobody
/// doctoring of their own accord.
fn hold_still(world: &mut World) {
    world.aboard.room.set_autonomous(false);
    for who in 0..world.aboard.crew_count() as usize {
        let at = world.aboard.room.bim_pos(who);
        world.aboard.room.put_for_probe(who, at);
        world.aboard.room.recruit_for_probe(who, true);
    }
    world
        .aboard
        .room
        .set_work_priority(bims::work::Job::Medical as u32, bims::work::NEVER);
}

/// Every enemy hit this crew member has taken since the world opened.
fn hits_taken(world: &World, who: u32) -> u32 {
    world.aboard.room.hits_taken(who as usize)
}

/// A piece of armour worn by `who`, put on by hand over whatever was
/// there, with `health` left on it.
fn wear(world: &mut World, who: usize, kind: ArmourKind, health: f32) -> u32 {
    let mut piece = world.holdings.new_piece(kind, Tier::One);
    let id = piece.id;
    piece.health = health;
    let mut gear = world.aboard.room.gear(who);
    *gear.worn_mut() = Some(piece);
    world.aboard.room.issue(who, gear);
    id
}

/// The health left on the armour worn.
fn worn_health(world: &World, who: usize) -> f32 {
    world.aboard.room.gear(who).worn().map_or(0.0, |p| p.health)
}

/// Fire `shots` pistol bolts at a crew member from six tiles off, one a
/// step, and say how many landed on it — off the world's own `CrewHit`.
/// Whoever is shot at is patched up and stood still between shots. The
/// bolts come from the west, or the first of the other three ways with a
/// clear line.
fn shoot_at(world: &mut World, who: u32, shots: u32) -> u32 {
    let at = world.aboard.room.bim_pos(who as usize);
    let from = [(-1.0, 0.0), (1.0, 0.0), (0.0, -1.0), (0.0, 1.0)]
        .into_iter()
        .map(|(x, y)| at + vec2(x, y) * (6.0 * TILE))
        .find(|&from| world.aboard.room.line_clear(from, at))
        .unwrap_or(at - vec2(6.0 * TILE, 0.0));
    let mut landed = 0;
    for _ in 0..shots {
        world.aboard.room.put_for_probe(who as usize, at);
        world
            .aboard
            .room
            .enemy_fire(from, at, WeaponKind::LaserPistol.basic(), false);
        // Long enough for the bolt to fly its six tiles and land.
        for _ in 0..40 {
            let events = world.step(&[]);
            landed += events
                .iter()
                .filter(|e| matches!(e, WorldEvent::CrewHit { who: w, .. } if *w == who))
                .count() as u32;
            world.aboard.room.patch_up_for_probe(who as usize);
            if world.aboard.room.combat_quiet_for_probe() {
                break;
            }
        }
    }
    landed
}

/// A tank of `want` ranks at a held station's door with a Trooper four
/// tiles in, and the rest of the crew as `others` say.
fn tank_in_a_fight(want: [u8; 4], others: [Class; 2]) -> World {
    let mut world = basic();
    assert_eq!(world.set_class(0, Class::Tank), Ok(()));
    assert_eq!(world.set_class(1, others[0]), Ok(()));
    assert_eq!(world.set_class(2, others[1]), Ok(()));
    assert!(world.stage_droid_fight_for_probe(DroidKind::Trooper, None));
    ranks(&mut world, 0, want);
    world.step(&[]);
    world
}

// --- A: the rank system (task 139) -------------------------------------------

#[test]
fn the_tank_climbs_sixteen_levels_and_ranks_up_as_the_soldier_does() {
    let mut world = tank();
    assert!(class::ranked(Class::Tank));
    assert_eq!(world.points_of(0), 1, "a point at the first level");
    level_up(&mut world, 0, 16);
    assert_eq!(world.progress_of(0).xp, 3_200, "level sixteen at 3 200");
    assert_eq!(world.level_of(0), 16);
    assert_eq!(world.points_of(0), 16);
    // The gates: Q, C and E rank n at 2n − 1, R at 6, 9, 12 and 15.
    for slot in [class::SLOT_Q, class::SLOT_C, class::SLOT_E] {
        for rank in 1..=4 {
            assert_eq!(
                class::rank_level(Class::Tank, slot, rank),
                Some(2 * rank - 1)
            );
        }
    }
    for (rank, want) in [(1, 6), (2, 9), (3, 12), (4, 15)] {
        assert_eq!(
            class::rank_level(Class::Tank, class::SLOT_R, rank),
            Some(want)
        );
    }
    // The refusals, as the soldier's.
    let mut world = tank();
    let rank_up = |world: &mut World, ability_slot: u8| {
        world.step(&[Command::RankUp {
            slot: 0,
            ability_slot: ability_slot as u32,
        }])
    };
    assert!(refused_with(
        &rank_up(&mut world, class::SLOT_R),
        Refusal::RankLocked
    ));
    assert!(refused_with(&rank_up(&mut world, 4), Refusal::NoRankedKit));
    let events = rank_up(&mut world, class::SLOT_Q);
    assert!(
        events.iter().any(|e| matches!(
            e,
            WorldEvent::RankedUp {
                who: 0,
                ability_slot: 0,
                rank: 1,
                ..
            }
        )),
        "{events:?}"
    );
    assert!(refused_with(
        &rank_up(&mut world, class::SLOT_C),
        Refusal::NoSkillPoint
    ));
    level_up(&mut world, 0, 2);
    assert!(refused_with(
        &rank_up(&mut world, class::SLOT_Q),
        Refusal::RankLocked
    ));
    level_up(&mut world, 0, 16);
    for _ in 1..4 {
        rank_up(&mut world, class::SLOT_Q);
    }
    assert_eq!(world.rank_of(0, class::SLOT_Q), 4);
    assert!(refused_with(
        &rank_up(&mut world, class::SLOT_Q),
        Refusal::TopRank
    ));
    // Between missions as well.
    world.leave_for_probe();
    rank_up(&mut world, class::SLOT_E);
    assert_eq!(world.rank_of(0, class::SLOT_E), 1);
}

/// **The old system is gone**: sixteen levels for everybody, the last at
/// 3 200, and every class buys a rank.
#[test]
fn every_class_accepts_a_rank_and_the_levels_are_sixteen() {
    assert_eq!(class::LEVELS, 16);
    assert_eq!(*LEVEL_XP.last().unwrap(), 3_200);
    for class in Class::ALL {
        let mut world = basic();
        assert_eq!(world.set_class(0, class), Ok(()));
        let events = world.step(&[Command::RankUp {
            slot: 0,
            ability_slot: class::SLOT_Q as u32,
        }]);
        if class == Class::None {
            assert!(refused_with(&events, Refusal::NoClass), "{events:?}");
        } else {
            assert!(
                events
                    .iter()
                    .any(|e| matches!(e, WorldEvent::RankedUp { who: 0, .. })),
                "{class:?}: {events:?}"
            );
        }
    }
}

// --- B: the base traits --------------------------------------------------------

#[test]
fn the_tank_sets_out_in_basic_armour_with_the_pistol_and_the_pool_is_unchanged() {
    let mut world = basic();
    let money = world.money;
    // The pool is the same whatever mix of classes the crew are: a class
    // owns abilities, never money.
    for (a, b) in [
        (Class::Tank, Class::None),
        (Class::Tank, Class::Tank),
        (Class::Tank, Class::Medic),
        (Class::None, Class::None),
    ] {
        assert_eq!(world.set_class(0, a), Ok(()));
        assert_eq!(world.set_class(1, b), Ok(()));
        assert_eq!(world.money, money, "{a:?} and {b:?}: the same pool");
    }
    assert_eq!(world.set_class(0, Class::Tank), Ok(()));
    assert_eq!(
        world.aboard.room.weapon(0),
        Some(WeaponKind::LaserPistol.basic()),
        "the pistol he had in hand"
    );
    let piece = world.aboard.room.gear(0).worn().expect("the armour on");
    assert_eq!((piece.kind, piece.tier), (ArmourKind::Armour, Tier::One));
    assert_eq!(piece.health, ArmourKind::Armour.stats().health, "fresh");
    assert!(
        piece.id < world.holdings.next_id,
        "numbered off the holdings"
    );
    // And a class put back to none takes its start off again.
    assert_eq!(world.set_class(0, Class::None), Ok(()));
    assert!(world.aboard.room.gear(0).worn().is_none());
    // Every crew member wears every piece of armour, whatever its class:
    // the tank's kit is a start, not a monopoly.
    assert_eq!(world.set_class(1, Class::Medic), Ok(()));
    let helm = Item::Armour(Piece::new(9_001, ArmourKind::Armour, Tier::One));
    world.leave_for_probe();
    let id = world.holdings.put(helm).unwrap();
    world.step(&[Command::Equip {
        slot: 1,
        who: 1,
        from: crate::GearSource::Armory { id },
    }]);
    assert!(world.aboard.room.gear(1).worn().is_some());
}

#[test]
fn armour_on_a_tank_drains_at_half_rate_and_the_overflow_reaches_the_body() {
    let mut world = tank();
    assert_eq!(world.set_class(1, Class::None), Ok(()));
    world.step(&[]);
    let kevlar = ArmourKind::Armour;
    let protection = kevlar.stats().protection;
    assert_eq!(kevlar.stats().health, 45.0);
    // The same piece, at twenty, on the tank: forty past its protection
    // is exactly what it can take, and nothing reaches the body.
    wear(&mut world, 0, kevlar, 20.0);
    let out = world.aboard.room.wound(0, 40.0 + protection);
    assert_eq!(out.through, 0.0, "the tank's kevlar took all forty");
    assert_eq!(out.absorbed, 40.0 + protection);
    assert_eq!(worn_health(&world, 0), 0.0, "and is spent");
    assert!(out.piece_broke);
    // And on anybody else it takes twenty, the other twenty reaching the
    // body.
    wear(&mut world, 1, kevlar, 20.0);
    let out = world.aboard.room.wound(1, 40.0 + protection);
    assert_eq!(out.through, 20.0, "half of it through");
    assert_eq!(out.absorbed, 20.0 + protection);
    assert_eq!(worn_health(&world, 1), 0.0);
    // The overflow on a tank is exact too: sixty past the protection is
    // forty taken and twenty through.
    wear(&mut world, 0, kevlar, 20.0);
    let out = world.aboard.room.wound(0, 60.0 + protection);
    assert_eq!(out.through, 20.0);
    assert_eq!(out.absorbed, 40.0 + protection);
    // A hit the protection swallows whole is swallowed for everybody,
    // and the piece's stored health is never doubled.
    wear(&mut world, 0, kevlar, 20.0);
    let out = world.aboard.room.wound(0, protection);
    assert_eq!((out.through, out.absorbed), (0.0, protection));
    assert_eq!(worn_health(&world, 0), 20.0, "not a point off");
    assert_eq!(world.armour_drain(0), class::TANK_DRAIN);
    assert_eq!(world.armour_drain(1), 1.0);
}

#[test]
fn enemy_hits_on_a_tank_are_counted_and_are_no_experience() {
    let mut world = tank();
    assert_eq!(world.set_class(1, Class::None), Ok(()));
    world.step(&[]);
    let mut landed = 0;
    while landed < 5 {
        landed += shoot_at(&mut world, 0, 1);
    }
    assert_eq!(hits_taken(&world, 0), landed, "every landing counted");
    assert_eq!(world.progress_of(0).xp, 0, "no experience for it");
    let before = hits_taken(&world, 0);
    let landed = shoot_at(&mut world, 0, 30);
    assert!(landed < 30, "{landed} of thirty bolts landed");
    assert_eq!(hits_taken(&world, 0) - before, landed);
}

// --- D: C, Plated ---------------------------------------------------------------

#[test]
fn plated_cuts_the_damage_taken_by_its_rank_before_the_armour() {
    let world = tank();
    assert_eq!(
        world.skill_of(0).damage_taken,
        1.0,
        "nothing at rank nought"
    );
    let kevlar = ArmourKind::Armour;
    let protection = kevlar.stats().protection;
    for (rank, taken) in (1..=4u8).zip(class::PLATED_DAMAGE_TAKEN) {
        let mut world = tank_at([0, rank, 0, 0]);
        assert_eq!(world.skill_of(0).damage_taken, taken, "rank {rank}");
        assert_eq!(world.aboard.room.skill_for_probe(0).damage_taken, taken);
        // Twenty on the kevlar: the hit cut first, the protection off
        // what is left, and the rest drains the piece at his rate.
        wear(&mut world, 0, kevlar, 20.0);
        world.aboard.room.wound(0, 10.0);
        let drain = world.armour_drain(0);
        let want = (10.0 * taken - protection) * drain;
        assert!(
            (20.0 - worn_health(&world, 0) - want).abs() < 1e-4,
            "rank {rank}: {} off, {want} wanted",
            20.0 - worn_health(&world, 0)
        );
    }
    // His alone.
    let mut world = tank_at([0, 4, 0, 0]);
    assert_eq!(world.set_class(1, Class::None), Ok(()));
    world.step(&[]);
    assert_eq!(world.skill_of(1).damage_taken, 1.0);
}

#[test]
fn plated_s_fourth_rank_drains_the_armour_at_a_quarter() {
    for (rank, drain) in [
        (0, class::TANK_DRAIN),
        (3, class::TANK_DRAIN),
        (4, class::TANK_DRAIN * class::FORTRESS_DRAIN),
    ] {
        let world = tank_at([0, rank, 0, 0]);
        assert_eq!(world.armour_drain(0), drain, "rank {rank}");
        assert_eq!(world.skill_of(0).armour_drain, drain);
    }
    assert_eq!(class::TANK_DRAIN * class::FORTRESS_DRAIN, 0.25);
}

/// **Every factor multiplies**: Plated and a commander's Rally on one
/// tank — Brace was the soldier's, and a Rampage multiplies into the same
/// `Skill::damage_taken` the same way (`tests_soldier`).
#[test]
fn plated_multiplies_with_a_rally() {
    let mut world = basic();
    assert_eq!(world.set_class(0, Class::Tank), Ok(()));
    assert_eq!(world.set_class(1, Class::Commander), Ok(()));
    ranks(&mut world, 0, [0, 1, 0, 0]);
    ranks(&mut world, 1, [0, 0, 1, 0]);
    let at = world.aboard.room.bim_pos(0);
    world.aboard.room.put_for_probe(1, at + vec2(TILE, 0.0));
    world.step(&[Command::Rally { slot: 1 }]);
    assert_eq!(world.rally_reaching(0), Some(1));
    let want = class::PLATED_DAMAGE_TAKEN[0] * class::RALLY_DAMAGE_TAKEN[0];
    assert!(
        (world.skill_of(0).damage_taken - want).abs() < 1e-6,
        "{} against {want}",
        world.skill_of(0).damage_taken
    );
}

/// **Plated mends** (task 155): his rank's hit points a second, on his
/// feet, a hurt tank's bar climbing by exactly that; nought before the
/// first rank, and nobody else's.
#[test]
fn plated_mends_its_rank_s_hit_points_a_second() {
    for (rank, regen) in (0..=4u8).zip([0.0, 0.2, 0.8, 1.4, 2.0]) {
        let mut world = tank_at([0, rank, 0, 0]);
        hold_still(&mut world);
        assert_eq!(world.plated_regen(0), regen, "rank {rank}");
        assert_eq!(world.plated_regen(1), 0.0, "his alone");
        // His armour broken, so the hit reaches the bar.
        wear(&mut world, 0, ArmourKind::Armour, 0.0);
        world.aboard.room.wound(0, 50.0);
        let before = world.aboard.room.health(0);
        for _ in 0..(10.0 / SECONDS_A_STEP).round() as u32 {
            world.step(&[]);
        }
        let mended = world.aboard.room.health(0) - before;
        assert!(
            (mended - regen * 10.0).abs() < 0.05,
            "rank {rank}: {mended} in ten seconds"
        );
    }
}

// --- C: Q, Riot Shield (task 155) -------------------------------------------------

/// The tank facing `towards` — his player's pointer on it — and every
/// player's Bim steered with the trigger up, so nobody of the crew fires
/// and the only bolts flying are the ones a test fires.
fn facing(world: &mut World, towards: bims::math::Vec2) {
    assert_eq!(world.aboard.crew_count(), world.players(), "players alone");
    let at = world.aboard.room.bim_pos(0);
    let aim = bims::order::angle_code((towards - at).angle());
    let steer = |slot: u32| Command::Crew {
        slot,
        order: bims::order::CrewOrder::Control {
            walk: None,
            aim,
            fire: false,
            sprint: false,
        },
    };
    let commands: Vec<Command> = (0..world.players()).map(steer).collect();
    world.step(&commands);
    // Whatever the crew fired before they were steered flies out.
    for _ in 0..240 {
        if world.aboard.room.combat_quiet_for_probe() {
            break;
        }
        world.step(&[]);
    }
}

/// Pistol bolts fired at the tank from `from`, one at a time, each flown
/// out, the tank held where it is: the `CrewHit`s on him.
fn bolts_at_the_tank(world: &mut World, from: bims::math::Vec2, n: u32, by: Option<usize>) -> u32 {
    let at = world.aboard.room.bim_pos(0);
    let mut landed = 0;
    for _ in 0..n {
        world
            .aboard
            .room
            .enemy_fire_by(from, at, WeaponKind::LaserPistol.basic(), false, by);
        for _ in 0..60 {
            world.aboard.room.put_for_probe(0, at);
            let events = world.step(&[]);
            landed += events
                .iter()
                .filter(|e| matches!(e, WorldEvent::CrewHit { who: 0 }))
                .count() as u32;
            if world.aboard.room.combat_quiet_for_probe() {
                break;
            }
        }
    }
    landed
}

#[test]
fn the_riot_shield_s_hit_points_go_by_its_rank_and_it_is_raised_and_put_down() {
    let mut world = tank();
    assert_eq!(world.set_class(1, Class::None), Ok(()));
    world.step(&[]);
    assert_eq!(world.can_riot_shield(0, true), Err(Refusal::NotLearnt));
    assert!(refused_with(
        &world.step(&[Command::RiotShield { slot: 0, on: true }]),
        Refusal::NotLearnt
    ));
    assert_eq!(world.can_riot_shield(1, true), Err(Refusal::NotATank));
    assert_eq!(
        world.can_riot_shield(0, false),
        Ok(()),
        "down is never refused"
    );
    for (rank, hp) in (1..=4u8).zip(class::RIOT_SHIELD_HP) {
        let mut world = tank_at([rank, 0, 0, 0]);
        assert_eq!(world.riot_shield_hp(0), hp, "rank {rank}");
        assert_eq!(world.riot_shield_left(0), hp, "a fresh tank's is whole");
        let events = world.step(&[Command::RiotShield { slot: 0, on: true }]);
        assert!(
            events
                .iter()
                .any(|e| matches!(e, WorldEvent::ShieldRaised { who: 0, on: true })),
            "{events:?}"
        );
        assert!(world.is_shielding(0));
        world.step(&[]);
        let plates = world.aboard.room.plates_for_probe();
        assert_eq!(plates.len(), 1);
        assert_eq!((plates[0].who, plates[0].hp), (0, hp), "rank {rank}");
        // Raised again is nothing said.
        let events = world.step(&[Command::RiotShield { slot: 0, on: true }]);
        assert!(
            !events
                .iter()
                .any(|e| matches!(e, WorldEvent::ShieldRaised { .. }))
        );
        world.step(&[Command::RiotShield { slot: 0, on: false }]);
        assert!(!world.is_shielding(0));
        world.step(&[]);
        assert!(world.aboard.room.plates_for_probe().is_empty());
    }
    // And going down takes it down.
    let mut world = tank_at([1, 0, 0, 0]);
    world.step(&[Command::RiotShield { slot: 0, on: true }]);
    world.aboard.room.kill_for_probe(0);
    world.step(&[]);
    world.step(&[]);
    assert!(!world.is_shielding(0), "the shield goes down with him");
    assert!(world.aboard.room.plates_for_probe().is_empty());
    assert_eq!(world.can_riot_shield(0, true), Err(Refusal::OutOfReach));
}

/// **A bolt from the front is stopped and bounced**: held up and facing
/// the shooter, nothing lands on him and the shield takes it; from
/// behind the shield is nothing; and the bolt bounced off it lands on
/// the machine that fired it, as the tank's.
#[test]
fn a_bolt_from_the_front_is_stopped_on_the_shield_and_bounced_back_at_the_shooter() {
    let mut world = tank_in_a_fight([4, 0, 0, 0], [Class::None, Class::None]);
    hold_still(&mut world);
    let residents = world.residents.as_ref().unwrap();
    let shooter = residents.aboard.room.crew_count() as usize;
    let machine = world
        .aboard
        .from_station(residents.aboard.position(shooter as u32))
        .expect("on the joined deck");
    let machine = vec2(machine.x as f32, machine.y as f32);
    // The machine held where it was staged, its legs shot off.
    {
        let room = &mut world.residents.as_mut().unwrap().aboard.room;
        let legs = room.droid(0).unwrap().body.max(DroidPart::Legs);
        room.strike_droid(0, DroidPart::Legs, legs);
    }
    let life = |world: &World| {
        world
            .residents
            .as_ref()
            .unwrap()
            .aboard
            .room
            .droid(0)
            .unwrap()
            .body
            .life_share()
    };
    facing(&mut world, machine);
    world.step(&[Command::RiotShield { slot: 0, on: true }]);
    let before = life(&world);
    let landed = bolts_at_the_tank(&mut world, machine, 8, Some(shooter));
    assert_eq!(landed, 0, "nothing reaches him through the shield");
    assert!(world.tank_of(0).shield_spent > 0.0, "the shield took it");
    assert!(life(&world) < before, "and the bolt went home");
    // From behind, the shield is nothing.
    let at = world.aboard.room.bim_pos(0);
    let behind = at + (at - machine).normalize_or_zero() * (5.0 * TILE);
    if world.aboard.room.line_clear(behind, at) {
        let spent = world.tank_of(0).shield_spent;
        assert!(
            bolts_at_the_tank(&mut world, behind, 8, None) > 0,
            "from behind"
        );
        assert!(world.tank_of(0).shield_spent <= spent + 1e-4);
    }
}

/// **It breaks at nought** — put down, `ShieldBroken` said, and refused
/// `ShieldRecharging` until a quarter is back — and **restores two a
/// second**: at once while stowed, and up only five seconds after the
/// last hit.
#[test]
fn the_shield_breaks_at_nought_and_restores_two_a_second_stowed_or_unstruck() {
    let mut world = tank_in_a_fight([1, 0, 0, 0], [Class::None, Class::None]);
    hold_still(&mut world);
    let at = world.aboard.room.bim_pos(0);
    let from = [(-1.0, 0.0), (1.0, 0.0), (0.0, -1.0), (0.0, 1.0)]
        .into_iter()
        .map(|(x, y)| at + vec2(x, y) * (4.0 * TILE))
        .find(|&from| world.aboard.room.line_clear(from, at))
        .expect("a clear way in");
    facing(&mut world, from);
    world.step(&[Command::RiotShield { slot: 0, on: true }]);
    // One hit point left: the next bolt breaks it.
    world.tanks[0].shield_spent = class::RIOT_SHIELD_HP[0] - 1.0;
    let mut broke = false;
    for _ in 0..20 {
        world
            .aboard
            .room
            .enemy_fire(from, at, WeaponKind::LaserPistol.basic(), false);
        for _ in 0..60 {
            world.aboard.room.put_for_probe(0, at);
            let events = world.step(&[]);
            broke |= events
                .iter()
                .any(|e| matches!(e, WorldEvent::ShieldBroken { who: 0 }));
            if world.aboard.room.combat_quiet_for_probe() {
                break;
            }
        }
        if broke {
            break;
        }
    }
    assert!(broke, "the shield broke");
    assert!(!world.is_shielding(0), "and went down");
    // Already restoring, stowed, over the steps the bolt took to fly out.
    let left = world.riot_shield_left(0);
    assert!(left < 1.0, "{left} left");
    assert_eq!(
        world.can_riot_shield(0, true),
        Err(Refusal::ShieldRecharging)
    );
    // Stowed, two a second at once: a quarter of twenty is five.
    run_for(&mut world, 2.0);
    assert!((world.riot_shield_left(0) - left - 4.0).abs() < 0.1);
    assert_eq!(
        world.can_riot_shield(0, true),
        Err(Refusal::ShieldRecharging)
    );
    run_for(&mut world, 1.0);
    assert_eq!(world.can_riot_shield(0, true), Ok(()), "a quarter back");
    // Up and struck, nothing for five seconds; then two a second.
    world.step(&[Command::RiotShield { slot: 0, on: true }]);
    let now = world.mission_minutes();
    world.tanks[0].shield_struck = Some(now);
    let left = world.riot_shield_left(0);
    run_for(&mut world, 4.5);
    assert!(
        (world.riot_shield_left(0) - left).abs() < 1e-4,
        "held while struck lately"
    );
    run_for(&mut world, 2.5);
    assert!(
        world.riot_shield_left(0) > left + 2.0,
        "restoring after five seconds"
    );
}

// --- E: E, Reflect Barrier (task 155) ----------------------------------------------

#[test]
fn the_barrier_s_time_and_cooldown_go_by_its_rank_and_it_is_refused_as_it_should() {
    let mut world = tank();
    assert_eq!(world.set_class(1, Class::None), Ok(()));
    world.step(&[]);
    assert_eq!(world.can_reflect(0), Err(Refusal::NotLearnt));
    assert_eq!(world.can_reflect(1), Err(Refusal::NotATank));
    for (rank, (&seconds, &cooldown)) in
        (1..=4u8).zip(class::REFLECT_SECONDS.iter().zip(&class::REFLECT_COOLDOWN))
    {
        let mut world = tank_at([0, 0, rank, 0]);
        assert_eq!(world.reflect_seconds(0), seconds, "rank {rank}");
        assert_eq!(world.reflect_cooldown(0), cooldown, "rank {rank}");
        assert_eq!(world.skill_of(0).reflect, 0.0, "none until raised");
        let events = world.step(&[Command::Reflect { slot: 0 }]);
        assert!(
            events
                .iter()
                .any(|e| matches!(e, WorldEvent::Reflecting { who: 0 })),
            "{events:?}"
        );
        assert!(world.is_reflecting(0));
        assert_eq!(world.skill_of(0).reflect, class::REFLECT_SHARE);
        assert_eq!(
            world.aboard.room.skill_for_probe(0).reflect,
            class::REFLECT_SHARE
        );
        assert_eq!(world.can_reflect(0), Err(Refusal::AlreadyActive));
        run_for(&mut world, seconds + 0.1);
        assert!(!world.is_reflecting(0), "rank {rank}: over");
        assert_eq!(world.skill_of(0).reflect, 0.0);
        assert_eq!(world.can_reflect(0), Err(Refusal::CoolingDown));
        run_for(&mut world, cooldown - seconds);
        assert_eq!(world.can_reflect(0), Ok(()), "rank {rank}: ready again");
    }
    let mut world = tank_at([0, 0, 1, 0]);
    world.aboard.room.kill_for_probe(0);
    world.step(&[]);
    assert_eq!(world.can_reflect(0), Err(Refusal::OutOfReach));
}

/// **What strikes him goes back**: a bolt and a blow landing on a tank
/// behind his barrier take as much off the machine that struck as they
/// took off him — and none without it.
#[test]
fn a_hit_on_a_reflecting_tank_goes_back_on_the_machine_that_struck_it() {
    let run = |reflecting: bool| {
        let mut world = tank_in_a_fight([0, 0, 1, 0], [Class::None, Class::None]);
        hold_still(&mut world);
        let residents = world.residents.as_ref().unwrap();
        let shooter = residents.aboard.room.crew_count() as usize;
        let machine = world
            .aboard
            .from_station(residents.aboard.position(shooter as u32))
            .expect("on the joined deck");
        let machine = vec2(machine.x as f32, machine.y as f32);
        let life = |world: &World| {
            let room = &world.residents.as_ref().unwrap().aboard.room;
            room.droid(0).unwrap().body.life_share()
        };
        facing(&mut world, machine);
        if reflecting {
            world.step(&[Command::Reflect { slot: 0 }]);
        }
        let before = life(&world);
        let landed = bolts_at_the_tank(&mut world, machine, 6, Some(shooter));
        assert!(landed > 0, "the bolts land on him");
        let after_bolts = life(&world);
        // A blow, carried in from the machine's side.
        let at = world.aboard.room.bim_pos(0);
        let beside = at + (machine - at).normalize_or_zero() * TILE;
        assert!(
            world
                .aboard
                .room
                .enemy_strike_by(beside, 0, 10.0, false, Some(shooter))
        );
        world.step(&[]);
        (before, after_bolts, life(&world))
    };
    let (before, bolts, blow) = run(false);
    assert_eq!(
        (before, bolts, blow),
        (before, before, before),
        "none without it"
    );
    let (before, bolts, blow) = run(true);
    assert!(bolts < before, "the bolts went back");
    assert!(blow < bolts, "and the blow");
}

// --- F: R, Bastion (task 155) -------------------------------------------------------

#[test]
fn the_bastion_s_radius_and_cooldown_go_by_its_rank_and_it_is_refused_as_it_should() {
    let mut world = tank();
    assert_eq!(world.set_class(1, Class::None), Ok(()));
    world.step(&[]);
    assert_eq!(world.can_bastion(0), Err(Refusal::NotLearnt));
    assert_eq!(world.can_bastion(1), Err(Refusal::NotATank));
    for (rank, (&radius, &cooldown)) in
        (1..=4u8).zip(class::BASTION_RADIUS.iter().zip(&class::BASTION_COOLDOWN))
    {
        let mut world = tank_at([0, 0, 0, rank]);
        assert_eq!(world.bastion_radius(0), radius, "rank {rank}");
        assert_eq!(world.bastion_cooldown(0), cooldown, "rank {rank}");
        world.step(&[Command::Bastion { slot: 0 }]);
        assert_eq!(
            world.can_bastion(0),
            Err(Refusal::CoolingDown),
            "rank {rank}"
        );
    }
    assert_eq!(class::BASTION_RADIUS[0], 6.0, "six tiles at the first");
}

/// **Every friend in reach gets a thousand that drains a hundred a
/// second**: the tank and a crewmate beside him, not one twenty tiles
/// off; a hit comes off it first; ten seconds and it is gone.
#[test]
fn the_bastion_shields_every_friend_in_reach_and_drains_in_ten_seconds() {
    let mut world = tank_at([0, 0, 0, 1]);
    hold_still(&mut world);
    let at = world.aboard.room.bim_pos(0);
    let near = world.aboard.room.free_tiles_near(at, 3.0 * TILE);
    let near = near.first().copied().expect("a tile beside him");
    world.aboard.room.put_for_probe(1, near);
    let far = (0..60)
        .map(|i| at + vec2((i % 8) as f32 - 4.0, (i / 8) as f32 - 4.0) * (3.0 * TILE))
        .find(|&p| (p - at).len() > 8.0 * TILE && world.aboard.room.is_deck_tile(p))
        .expect("a deck tile far off");
    world.aboard.room.put_for_probe(2, far);
    let reached = world.bastion_reaching(0);
    assert!(reached.contains(&0) && reached.contains(&1), "{reached:?}");
    assert!(!reached.contains(&2), "{reached:?}");
    let events = world.step(&[Command::Bastion { slot: 0 }]);
    assert!(
        events.iter().any(|e| matches!(
            e,
            WorldEvent::Bastion { who: 0, reached } if *reached as usize == 2
        )),
        "{events:?}"
    );
    let shield = |world: &World, who: usize| world.aboard.room.shield_hp(who);
    assert!(shield(&world, 0) > 990.0 && shield(&world, 1) > 990.0);
    assert_eq!(shield(&world, 2), 0.0, "out of reach");
    // A hit comes off the shield, not the body.
    let health = world.aboard.room.health(1);
    let left = shield(&world, 1);
    world.aboard.room.wound(1, 50.0);
    assert_eq!(world.aboard.room.health(1), health, "the body untouched");
    assert!((left - shield(&world, 1) - 50.0).abs() < 1e-3);
    // Five seconds: half of it drained.
    for _ in 0..(5.0 / SECONDS_A_STEP).round() as u32 {
        world.step(&[]);
    }
    assert!(
        (shield(&world, 0) - 500.0).abs() < 5.0,
        "{}",
        shield(&world, 0)
    );
    for _ in 0..(5.5 / SECONDS_A_STEP).round() as u32 {
        world.step(&[]);
    }
    assert_eq!(shield(&world, 0), 0.0, "gone in ten seconds");
    assert_eq!(shield(&world, 1), 0.0);
}

/// **The Override Core's gift**: at the fifth rank everybody the Bastion
/// reached walks half again as fast for its ten seconds; without the core
/// nobody does.
#[test]
fn with_an_override_core_the_bastion_hastes_everybody_it_reached_for_ten_seconds() {
    for core in [false, true] {
        let mut world = tank_at([0, 0, 0, 4]);
        hold_still(&mut world);
        if core {
            let mut gear = world.aboard.room.gear(0);
            gear.items[0] = Some(bims::module::ModuleKind::OverrideCore.at(Tier::One));
            world.aboard.room.issue(0, gear);
        }
        assert_eq!(
            world.rank_of(0, class::SLOT_R),
            if core { class::OVERRIDE_RANK } else { 4 }
        );
        let at = world.aboard.room.bim_pos(0);
        world.aboard.room.put_for_probe(1, at + vec2(TILE, 0.0));
        world.step(&[Command::Bastion { slot: 0 }]);
        let want = if core { class::BASTION_HASTE } else { 1.0 };
        assert_eq!(world.skill_of(0).walk, want, "core {core}: him");
        assert_eq!(world.skill_of(1).walk, want, "core {core}: a crewmate");
        run_for(&mut world, class::BASTION_SECONDS + 0.1);
        assert_eq!(world.skill_of(1).walk, 1.0, "core {core}: over");
    }
}

// --- G: the clocks ------------------------------------------------------------------

#[test]
fn the_barrier_and_the_bastion_are_ready_at_every_mission_and_shortened_by_the_relics() {
    let mut world = tank_at([1, 0, 1, 1]);
    let (reflect, bastion) = (world.reflect_cooldown(0), world.bastion_cooldown(0));
    world.give_relic_for_probe(crate::relic::Relic::OverclockedCores);
    assert!(world.reflect_cooldown(0) < reflect, "*Overclocked Cores*");
    assert!(world.bastion_cooldown(0) < bastion, "*Overclocked Cores*");
    world.step(&[
        Command::Reflect { slot: 0 },
        Command::Bastion { slot: 0 },
        Command::RiotShield { slot: 0, on: true },
    ]);
    world.tanks[0].shield_spent = 5.0;
    run_for(&mut world, 10.0);
    let (reflect_left, bastion_left) = (
        world.reflect_cooldown_left(0),
        world.bastion_cooldown_left(0),
    );
    assert!(reflect_left > 0.0 && bastion_left > 0.0);
    // Seconds taken off (a *Reset Capacitor*'s way) both.
    world.cooldowns_less(0, 3.0);
    assert!(world.reflect_cooldown_left(0) < reflect_left, "seconds off");
    assert!(world.bastion_cooldown_left(0) < bastion_left, "seconds off");
    // Ready at every mission's start, whatever was left, the shield whole
    // and down.
    next_mission(&mut world);
    assert_eq!(world.reflect_cooldown_left(0), 0.0);
    assert_eq!(world.bastion_cooldown_left(0), 0.0);
    assert_eq!(world.can_reflect(0), Ok(()));
    assert_eq!(world.can_bastion(0), Ok(()));
    assert!(!world.is_shielding(0));
    assert_eq!(world.riot_shield_left(0), world.riot_shield_hp(0));
}

// --- the seam ------------------------------------------------------------------------

#[test]
fn the_checksum_notices_a_shield_a_barrier_a_bastion_and_a_hit_taken() {
    let base = || tank_at([1, 0, 1, 1]);
    let mut quiet = base();
    quiet.step(&[]);
    let mut shielded = base();
    shielded.step(&[Command::RiotShield { slot: 0, on: true }]);
    assert_ne!(
        world_checksum(&shielded),
        world_checksum(&quiet),
        "a shield"
    );
    let mut spent = base();
    spent.tanks[0].shield_spent = 3.0;
    spent.step(&[]);
    assert_ne!(
        world_checksum(&spent),
        world_checksum(&quiet),
        "a shield struck"
    );
    let mut reflecting = base();
    reflecting.step(&[Command::Reflect { slot: 0 }]);
    assert_ne!(
        world_checksum(&reflecting),
        world_checksum(&quiet),
        "a barrier"
    );
    let mut bastion = base();
    bastion.step(&[Command::Bastion { slot: 0 }]);
    assert_ne!(
        world_checksum(&bastion),
        world_checksum(&quiet),
        "a Bastion"
    );
    assert_ne!(world_checksum(&bastion), world_checksum(&reflecting));
    let mut hit = base();
    let before = world_checksum(&hit);
    assert!(shoot_at(&mut hit, 0, 8) > 0);
    assert_ne!(world_checksum(&hit), before);
    // And a tank's kit is a resource nobody's hold moved.
    let mut none = basic();
    assert_eq!(none.set_class(0, Class::None), Ok(()));
    none.step(&[]);
    let mut tanked = basic();
    assert_eq!(tanked.set_class(0, Class::Tank), Ok(()));
    tanked.step(&[]);
    assert_eq!(
        none.ship.design.carrying(ResourceId::Armour),
        tanked.ship.design.carrying(ResourceId::Armour),
        "the tank's kit comes with him, not out of the hold"
    );
    // A tank shielded, reflecting and his Bastion thrown, shot at: the
    // same fight twice.
    let run = || {
        let mut world = tank_at([2, 2, 2, 1]);
        hold_still(&mut world);
        world.step(&[
            Command::RiotShield { slot: 0, on: true },
            Command::Reflect { slot: 0 },
            Command::Bastion { slot: 0 },
        ]);
        shoot_at(&mut world, 0, 4);
        world_checksum(&world)
    };
    assert_eq!(run(), run());
}
