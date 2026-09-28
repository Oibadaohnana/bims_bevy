//! The medic class (feature 76): `crate::class`'s third class. The heal
//! beam — which puts hit points back since task 120, and may be turned on
//! the medic itself — the surge, the talents that still do something,
//! carrying a downed crewmate out of the fire, and how fast a medic
//! revives.

use bims::health::MAX_HEALTH;
use bims::math::{Vec2, vec2};
use bims::order::CrewOrder;
use shipdesign::fixture::combat_ship;

use crate::class::{self, Class, LEVEL_XP, Side, Talent};
use crate::event::{Refusal, WorldEvent};
use crate::fixture::{REFERENCE_MONEY, simulation_world};
use crate::world::{Command, World};
use crate::world_checksum;

const TILE: f32 = shipdesign::TILE as f32;

/// Steps of the world a minute of its clock is.
const STEPS_A_MINUTE: u32 = 60;

/// The combat ship, bunks for two and more, with two players' crew.
fn basic() -> World {
    simulation_world(combat_ship(), REFERENCE_MONEY, 2)
}

/// [`basic`] with slot 0 a medic, and crew member 1 stood a tile beside
/// it, within the beam's reach and sight — and the crew held still.
fn medic() -> World {
    let mut world = basic();
    assert_eq!(world.set_class(0, Class::Medic), Ok(()));
    hold_still(&mut world);
    beside(&mut world, 1, 0);
    world
}

/// The crew held where they stand: their own errands off, and everybody
/// recruited, so nobody wanders off from a beam six tiles long. What the
/// player orders still goes.
fn hold_still(world: &mut World) {
    world.aboard.room.set_autonomous(false);
    for who in 0..world.aboard.crew_count() as usize {
        // On a cell a body fits in first: where the crew wake up is
        // against the furniture, and the push-out walks one off it.
        let at = world.aboard.room.bim_pos(who);
        world.aboard.room.put_for_probe(who, at);
        world.aboard.room.recruit_for_probe(who, true);
    }
    // And nobody revives of their own accord: these tests want the hands
    // they name.
    world
        .aboard
        .room
        .set_work_priority(bims::work::Job::Medical as u32, bims::work::NEVER);
}

/// Crew member `who` stood a tile from `of`.
fn beside(world: &mut World, who: usize, of: usize) -> Vec2 {
    let at = world.aboard.room.bim_pos(of) + vec2(TILE, 0.0);
    let spot = world.aboard.room.put_for_probe(who, at);
    world.step(&[]);
    spot
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
    assert!(world.progress_of(who as u32).level() >= level);
}

fn pick(world: &mut World, who: u32, talent: Talent) {
    let (level, side) = (1..=class::LEVELS)
        .find_map(|l| {
            class::pick_at(talent.class(), l).and_then(|(left, right)| {
                if left == talent {
                    Some((l, Side::Left))
                } else if right == talent {
                    Some((l, Side::Right))
                } else {
                    None
                }
            })
        })
        .expect("a talent is on a pick level");
    if world.progress_of(who).level() < level {
        level_up(world, who as usize, level);
    }
    let events = world.step(&[Command::PickTalent {
        slot: who,
        level: level as u32,
        side,
    }]);
    assert!(
        events.iter().any(
            |e| matches!(e, WorldEvent::TalentPicked { talent: t, .. } if *t == talent.code())
        ),
        "{talent:?} picked: {events:?}"
    );
    assert!(world.has_talent(who, talent));
}

fn beam(world: &mut World, slot: u32, patient: Option<u32>) -> Vec<WorldEvent> {
    world.step(&[Command::Beam { slot, patient }])
}

fn linked(events: &[WorldEvent], who: u32, patient: Option<u32>) -> bool {
    events
        .iter()
        .any(|e| *e == WorldEvent::Beamed { who, patient })
}

/// `who` at `points` of its bar: what the beam is measured against,
/// since a whole bar gains nothing whatever the rate.
fn hurt(world: &mut World, who: usize, points: f32) {
    world.aboard.room.set_health_for_probe(who, points);
    world.step(&[]);
}

/// What `minutes` of the clock of a beam at the plain rate put back.
fn beam_gain(minutes: f32) -> f32 {
    class::HEAL_BEAM_HP / 60.0 * minutes
}

// --- A: the heal beam ------------------------------------------------------------

/// **The beam puts hit points back** (task 120): the rate the blood came
/// back at, read as hit points an hour of the clock — never past the whole
/// bar, and nothing to a downed body — and the medic holds its fire.
#[test]
fn a_linked_patient_gains_hit_points_and_the_medic_holds_fire() {
    let mut world = medic();
    hurt(&mut world, 1, 40.0);
    let events = beam(&mut world, 0, Some(1));
    assert!(linked(&events, 0, Some(1)), "{events:?}");
    assert_eq!(world.patients_of(0), vec![1]);
    assert!(world.aboard.room.is_beaming(0));
    let before = world.aboard.room.health(1);
    for _ in 0..(10 * STEPS_A_MINUTE) {
        world.step(&[]);
    }
    let gained = world.aboard.room.health(1) - before;
    let want = beam_gain(10.0);
    assert!(
        (gained - want).abs() < 0.2,
        "ten minutes of the beam: {gained} of {want}"
    );
    assert!(world.aboard.room.is_beaming(0), "still linked");
    // The medic under arms holds its fire while it beams, and shoots
    // again unlinked.
    world.aboard.room.recruit_for_probe(0, true);
    world.step(&[]);
    assert!(!world.aboard.room.is_armed(0), "holds its fire");
    assert!(world.skill_of(0).holds_fire);
    assert!(linked(&beam(&mut world, 0, None), 0, None));
    world.step(&[]);
    assert!(world.aboard.room.is_armed(0), "armed again");
    assert!(!world.skill_of(0).holds_fire);
    // Downed, the beam gets nobody up: only a revive does.
    world.aboard.room.knock_out_for_probe(1);
    world.step(&[]);
    assert!(world.aboard.room.is_downed(1));
    assert!(linked(&beam(&mut world, 0, Some(1)), 0, Some(1)));
    for _ in 0..(5 * STEPS_A_MINUTE) {
        world.step(&[]);
    }
    assert!(world.aboard.room.is_downed(1), "still downed");
    assert_eq!(world.aboard.room.health(1), 0.0);
    // Never above a whole bar.
    let mut world = medic();
    hurt(&mut world, 1, MAX_HEALTH - 1.0);
    assert!(linked(&beam(&mut world, 0, Some(1)), 0, Some(1)));
    for _ in 0..(10 * STEPS_A_MINUTE) {
        world.step(&[]);
    }
    assert_eq!(world.aboard.room.health(1), MAX_HEALTH);
}

/// **A medic may beam itself** (task 120): the link is its own, it gains
/// the beam's hit points, and its fire is held as for anybody else.
#[test]
fn a_medic_may_beam_itself() {
    let mut world = medic();
    assert_eq!(world.can_beam(0, 0), Ok(()));
    hurt(&mut world, 0, 50.0);
    let events = beam(&mut world, 0, Some(0));
    assert!(linked(&events, 0, Some(0)), "{events:?}");
    assert_eq!(world.patients_of(0), vec![0]);
    let before = world.aboard.room.health(0);
    for _ in 0..(10 * STEPS_A_MINUTE) {
        world.step(&[]);
    }
    let gained = world.aboard.room.health(0) - before;
    assert!(
        (gained - beam_gain(10.0)).abs() < 0.2,
        "its own bar: {gained}"
    );
    assert!(world.skill_of(0).holds_fire);
    // And it charges the surge like any patient short of its bar.
    assert!(world.surge_charge(0) > 0.0);
}

#[test]
fn every_way_the_link_breaks_and_every_reason_a_beam_is_refused() {
    let mut world = medic();
    // Refused: not a medic, an enemy (nobody of the crew), out of range,
    // out of sight.
    assert_eq!(world.can_beam(1, 0), Err(Refusal::NotAMedic));
    assert!(refused_with(
        &beam(&mut world, 1, Some(0)),
        Refusal::NotAMedic
    ));
    assert_eq!(world.can_beam(0, 7), Err(Refusal::NotACrewmate));
    assert!(refused_with(
        &beam(&mut world, 0, Some(7)),
        Refusal::NotACrewmate
    ));
    let here = world.aboard.room.bim_pos(0);
    let range = world.beam_range(0) * TILE;
    let far = world
        .aboard
        .room
        .put_for_probe(1, here + vec2(range + 2.0 * TILE, 0.0));
    if (far - here).len() > range {
        assert_eq!(world.can_beam(0, 1), Err(Refusal::OutOfBeamRange));
        assert!(refused_with(
            &beam(&mut world, 0, Some(1)),
            Refusal::OutOfBeamRange
        ));
    }
    // A tile within range the medic cannot see, if the deck has one
    // here: behind a bulkhead.
    let hidden = (-8..=8)
        .flat_map(|dx| (-8..=8).map(move |dy| (dx, dy)))
        .map(|(dx, dy)| here + vec2(dx as f32 * TILE, dy as f32 * TILE))
        .filter(|&p| (p - here).len() <= range && world.aboard.room.is_deck_tile(p))
        .find(|&p| !world.aboard.room.sees_for_probe(0, p));
    if let Some(hidden) = hidden {
        let at = world.aboard.room.put_for_probe(1, hidden);
        if !world.aboard.room.sees_for_probe(0, at) && (at - here).len() <= range {
            assert_eq!(world.can_beam(0, 1), Err(Refusal::NoSightOfPatient));
        }
    }
    // Linked, then the patient walks out of range: broken, said once.
    beside(&mut world, 1, 0);
    assert!(linked(&beam(&mut world, 0, Some(1)), 0, Some(1)));
    world
        .aboard
        .room
        .put_for_probe(1, here + vec2(range + 2.0 * TILE, 0.0));
    let events = world.step(&[]);
    assert!(linked(&events, 0, None), "{events:?}");
    assert!(!world.is_beaming(0));
    assert!(!world.aboard.room.is_beaming(0));
    // The patient dies: broken.
    beside(&mut world, 1, 0);
    assert!(linked(&beam(&mut world, 0, Some(1)), 0, Some(1)));
    world.aboard.room.kill_for_probe(1);
    world.step(&[]);
    let events = world.step(&[]);
    assert!(!world.is_beaming(0), "{events:?}");
    // A fresh world for the rest: a dead crew member stays dead.
    let mut world = medic();
    // The medic goes down: broken.
    assert!(linked(&beam(&mut world, 0, Some(1)), 0, Some(1)));
    world.aboard.room.knock_out_for_probe(0);
    // Downed at the top of its next tick; the world reads it the step
    // after.
    world.step(&[]);
    world.step(&[]);
    assert!(!world.is_beaming(0), "the medic down");
    world.aboard.room.patch_up_for_probe(0);
    world.step(&[]);
    // Ordered to walk: kept. Ordered to an errand — a revive: broken.
    assert!(linked(&beam(&mut world, 0, Some(1)), 0, Some(1)));
    let there = world.aboard.room.bim_pos(0) + vec2(2.0 * TILE, 0.0);
    world.step(&[Command::Crew {
        slot: 0,
        order: CrewOrder::SendTo {
            who: 0,
            x: there.x,
            y: there.y,
        },
    }]);
    world.step(&[]);
    assert!(world.is_beaming(0), "a walk keeps the beam");
    world.aboard.room.knock_out_for_probe(1);
    world.step(&[]);
    world.step(&[Command::Crew {
        slot: 0,
        order: CrewOrder::Revive { who: 0, patient: 1 },
    }]);
    assert!(!world.is_beaming(0), "an errand ends it");
    // Unlinked by the medic: broken, and unlinking is never refused a
    // medic.
    world.aboard.room.patch_up_for_probe(1);
    beside(&mut world, 1, 0);
    assert!(linked(&beam(&mut world, 0, Some(1)), 0, Some(1)));
    assert!(linked(&beam(&mut world, 0, None), 0, None));
    assert!(!world.is_beaming(0));
    // And the link is in the checksum.
    let before = world_checksum(&world);
    beam(&mut world, 0, Some(1));
    assert_ne!(world_checksum(&world), before);
}

// --- B: the surge ----------------------------------------------------------------

/// Beaming a hurt crewmate until the charge is full.
fn charge_up(world: &mut World) {
    world.aboard.room.set_health_for_probe(1, 10.0);
    if !world.is_beaming(0) {
        assert!(linked(&beam(world, 0, Some(1)), 0, Some(1)));
    }
    for _ in 0..(60 * STEPS_A_MINUTE) {
        if world.surge_charge(0) >= 1.0 {
            return;
        }
        world.step(&[]);
    }
    panic!("never charged");
}

/// **The surge charges while the patient is short of its bar** (task 120:
/// it was short of blood or bleeding), and a surge takes every hit whole.
#[test]
fn the_charge_fills_only_while_the_patient_is_hurt_and_a_surge_absorbs_every_hit() {
    let run = |seed_steps: u32| {
        let mut world = medic();
        // A whole patient: nothing to heal, nothing charged.
        assert!(linked(&beam(&mut world, 0, Some(1)), 0, Some(1)));
        for _ in 0..(5 * STEPS_A_MINUTE) {
            world.step(&[]);
        }
        assert_eq!(
            world.surge_charge(0),
            0.0,
            "a whole patient charges nothing"
        );
        // Refused before the third level, unlinked, uncharged.
        assert_eq!(world.can_surge(0), Err(Refusal::NoSurgeYet));
        assert!(refused_with(
            &world.step(&[Command::Surge { slot: 0 }]),
            Refusal::NoSurgeYet
        ));
        level_up(&mut world, 0, class::SURGE_LEVEL);
        assert_eq!(world.can_surge(0), Err(Refusal::NotCharged));
        beam(&mut world, 0, None);
        assert_eq!(world.can_surge(0), Err(Refusal::NotLinked));
        // A hurt patient — far enough down that forty minutes of the
        // beam do not fill it — and the charge fills, full after forty
        // minutes.
        hurt(&mut world, 1, 10.0);
        assert!(linked(&beam(&mut world, 0, Some(1)), 0, Some(1)));
        for _ in 0..(20 * STEPS_A_MINUTE) {
            world.step(&[]);
        }
        assert!(
            (world.surge_charge(0) - 0.5).abs() < 0.02,
            "half way: {}",
            world.surge_charge(0)
        );
        assert_eq!(world.can_surge(0), Err(Refusal::NotCharged));
        for _ in 0..(20 * STEPS_A_MINUTE) {
            world.step(&[]);
        }
        assert_eq!(world.surge_charge(0), 1.0);
        assert_eq!(world.can_surge(0), Ok(()));
        for _ in 0..seed_steps {
            world.step(&[]);
        }
        // Armour on the patient, so a drained piece would show.
        let mut gear = world.aboard.room.gear(1);
        gear.basic_armour(9_000);
        world.aboard.room.issue(1, gear);
        let armour = world.aboard.room.armour_health(1);
        let events = world.step(&[Command::Surge { slot: 0 }]);
        assert!(
            events.contains(&WorldEvent::Surged { who: 0 }),
            "{events:?}"
        );
        assert!(world.surge_charge(0) < 0.01, "{}", world.surge_charge(0));
        assert!(world.is_surging(0) && world.is_surging(1));
        // Neither loses a hit point nor a drained piece.
        let (hp0, hp1) = (world.aboard.room.health(0), world.aboard.room.health(1));
        let out = world.aboard.room.wound(0, bims::health::Part::Body, 80.0);
        assert_eq!(out.absorbed, 80.0);
        assert_eq!(out.through, 0.0);
        assert!(!out.downed);
        let out = world
            .aboard
            .room
            .strike(1, bims::health::Part::Body, 80.0, true);
        assert_eq!(out.absorbed, 80.0);
        assert!(!out.piece_broke);
        assert_eq!(
            world.aboard.room.armour_health(1),
            armour,
            "no armour drained"
        );
        assert_eq!(world.aboard.room.health(0), hp0);
        assert_eq!(world.aboard.room.health(1), hp1);
        // Unlinking does not end the surge on the patient.
        beam(&mut world, 0, None);
        assert!(world.is_surging(1));
        // It ends on time: eight minutes of the clock.
        let mut ended = None;
        for step in 1..(20 * STEPS_A_MINUTE) {
            world.step(&[]);
            if !world.is_surging(1) {
                ended = Some(step);
                break;
            }
        }
        let ended = ended.expect("ended");
        let want = (class::SURGE_MINUTES * STEPS_A_MINUTE as f64) as u32;
        assert!(
            ended.abs_diff(want) <= 3,
            "ended after {ended} steps, wanted about {want}"
        );
        assert!(!world.is_surging(0));
        // A hit lands again after.
        let out = world.aboard.room.wound(0, bims::health::Part::Legs, 1.0);
        assert_eq!(out.through, 1.0);
        world_checksum(&world)
    };
    // Two runs on one seed are one fight.
    assert_eq!(run(7), run(7));
}

// --- C: the ten levels -----------------------------------------------------------

#[test]
fn the_fixed_levels_are_the_beam_and_the_surge() {
    // Level one: the beam is a medic's, and a medic's from the first.
    let mut world = medic();
    assert_eq!(world.progress_of(0).level(), 1);
    assert_eq!(world.can_beam(0, 1), Ok(()));
    assert_eq!(world.set_class(1, Class::Soldier), Ok(()));
    assert_eq!(world.can_beam(1, 0), Err(Refusal::NotAMedic));
    // Level three: the surge — pinned above in `can_surge`'s order.
    assert_eq!(class::SURGE_LEVEL, 3);
    // Level seven is *mender*, a no-op since task 120: nothing mends, so
    // the beam puts back the same at the sixth and the seventh.
    let healed_by = |level: u8| {
        let mut world = medic();
        level_up(&mut world, 0, level);
        hurt(&mut world, 1, 20.0);
        assert!(linked(&beam(&mut world, 0, Some(1)), 0, Some(1)));
        let before = world.aboard.room.health(1);
        for _ in 0..(10 * STEPS_A_MINUTE) {
            world.step(&[]);
        }
        world.aboard.room.health(1) - before
    };
    let (plain, mender) = (healed_by(6), healed_by(class::MENDER_LEVEL));
    assert!((plain - mender).abs() < 1e-3, "{plain} against {mender}");
}

#[test]
fn long_beam_and_strong_beam_are_the_beam_s_numbers() {
    let mut world = medic();
    assert_eq!(world.beam_range(0), class::HEAL_BEAM_RANGE);
    assert_eq!(world.beam_rate(0), class::HEAL_BEAM_HP);
    pick(&mut world, 0, Talent::LongBeam);
    assert_eq!(
        world.beam_range(0),
        class::HEAL_BEAM_RANGE * class::LONG_BEAM_RANGE
    );
    assert_eq!(world.beam_rate(0), class::HEAL_BEAM_HP);
    assert_eq!(
        world.beam_range(1),
        class::HEAL_BEAM_RANGE,
        "the medic's alone"
    );
    // A patient at seven and a half tiles: out of the plain beam's reach,
    // within the long one's.
    let here = world.aboard.room.bim_pos(0);
    let at = world
        .aboard
        .room
        .put_for_probe(1, here + vec2(7.5 * TILE, 0.0));
    let apart = (at - here).len();
    if apart > class::HEAL_BEAM_RANGE * TILE && apart <= world.beam_range(0) * TILE {
        assert_ne!(world.can_beam(0, 1), Err(Refusal::OutOfBeamRange));
    }
    // Strong beam: half again the hit points an hour, and the patient
    // gains them.
    let gained_with = |talent: Option<Talent>| {
        let mut world = medic();
        if let Some(t) = talent {
            pick(&mut world, 0, t);
        }
        hurt(&mut world, 1, 20.0);
        assert!(linked(&beam(&mut world, 0, Some(1)), 0, Some(1)));
        let before = world.aboard.room.health(1);
        for _ in 0..(10 * STEPS_A_MINUTE) {
            world.step(&[]);
        }
        (world.beam_rate(0), world.aboard.room.health(1) - before)
    };
    let (plain_rate, plain) = gained_with(None);
    let (strong_rate, strong) = gained_with(Some(Talent::StrongBeam));
    assert_eq!(strong_rate, plain_rate * class::STRONG_BEAM_RATE);
    assert!(
        (strong / plain - class::STRONG_BEAM_RATE).abs() < 0.05,
        "{strong} against {plain}"
    );
}

#[test]
fn quick_charge_and_long_surge_are_the_surge_s_numbers() {
    let mut world = medic();
    assert_eq!(world.surge_charge_wanted(0), class::SURGE_CHARGE_MINUTES);
    assert_eq!(world.surge_minutes(0), class::SURGE_MINUTES);
    pick(&mut world, 0, Talent::QuickCharge);
    assert_eq!(
        world.surge_charge_wanted(0),
        class::SURGE_CHARGE_MINUTES / class::QUICK_CHARGE_RATE
    );
    // Full in forty minutes over one and a half.
    hurt(&mut world, 1, 10.0);
    assert!(linked(&beam(&mut world, 0, Some(1)), 0, Some(1)));
    let want =
        (class::SURGE_CHARGE_MINUTES / class::QUICK_CHARGE_RATE * STEPS_A_MINUTE as f64) as u32;
    for _ in 0..(want - 5) {
        world.step(&[]);
    }
    assert!(world.surge_charge(0) < 1.0);
    for _ in 0..10 {
        world.step(&[]);
    }
    assert_eq!(world.surge_charge(0), 1.0);
    // Long surge: half again the minutes, and it runs them.
    let mut world = medic();
    pick(&mut world, 0, Talent::LongSurge);
    assert_eq!(
        world.surge_minutes(0),
        class::SURGE_MINUTES * class::LONG_SURGE_TIME
    );
    assert_eq!(
        world.surge_minutes(1),
        class::SURGE_MINUTES,
        "the medic's alone"
    );
    level_up(&mut world, 0, class::SURGE_LEVEL);
    charge_up(&mut world);
    assert!(
        world
            .step(&[Command::Surge { slot: 0 }])
            .contains(&WorldEvent::Surged { who: 0 })
    );
    let want = (world.surge_minutes(0) * STEPS_A_MINUTE as f64) as u32;
    for _ in 0..(want - 5) {
        world.step(&[]);
    }
    assert!(world.is_surging(1), "still running");
    for _ in 0..10 {
        world.step(&[]);
    }
    assert!(!world.is_surging(1));
}

#[test]
fn double_link_holds_two_patients_at_the_full_rate() {
    let mut world = simulation_world(combat_ship(), REFERENCE_MONEY, 3);
    assert_eq!(world.set_class(0, Class::Medic), Ok(()));
    hold_still(&mut world);
    beside(&mut world, 1, 0);
    let at = world.aboard.room.bim_pos(0) - vec2(TILE, 0.0);
    world.aboard.room.put_for_probe(2, at);
    world.step(&[]);
    // Without it, the second link replaces the first.
    assert!(linked(&beam(&mut world, 0, Some(1)), 0, Some(1)));
    assert!(linked(&beam(&mut world, 0, Some(2)), 0, Some(2)));
    assert_eq!(world.patients_of(0), vec![2]);
    pick(&mut world, 0, Talent::DoubleLink);
    assert_eq!(world.beam_patients(0), class::DOUBLE_LINK_PATIENTS);
    assert!(linked(&beam(&mut world, 0, None), 0, None));
    for who in [1, 2] {
        world.aboard.room.set_health_for_probe(who, 30.0);
    }
    world.step(&[]);
    assert!(linked(&beam(&mut world, 0, Some(2)), 0, Some(2)));
    assert!(linked(&beam(&mut world, 0, Some(1)), 0, Some(1)));
    assert_eq!(world.patients_of(0), vec![2, 1]);
    let before = [world.aboard.room.health(1), world.aboard.room.health(2)];
    for _ in 0..(10 * STEPS_A_MINUTE) {
        world.step(&[]);
    }
    let want = beam_gain(10.0);
    for who in [1, 2] {
        let gained = world.aboard.room.health(who) - before[who - 1];
        assert!(
            (gained - want).abs() < 0.2,
            "{who} gained {gained} of {want}"
        );
    }
    assert!(world.surge_charge(0) > 0.0, "the charge fills off either");
    // Unlinked, both go.
    assert!(linked(&beam(&mut world, 0, None), 0, None));
    assert!(world.patients_of(0).is_empty());
}

#[test]
fn gunner_medic_fires_while_beaming() {
    let mut world = medic();
    pick(&mut world, 0, Talent::GunnerMedic);
    assert!(linked(&beam(&mut world, 0, Some(1)), 0, Some(1)));
    world.aboard.room.recruit_for_probe(0, true);
    world.step(&[]);
    assert!(world.aboard.room.is_armed(0), "fires while beaming");
    let skill = world.skill_of(0);
    assert!(!skill.holds_fire);
    assert_eq!(skill.fire_rate, class::GUNNER_MEDIC_FIRE_RATE);
    assert_eq!(world.aboard.room.skill_for_probe(0), skill);
    beam(&mut world, 0, None);
    assert_eq!(world.skill_of(0).fire_rate, 1.0, "the whole rate unlinked");
}

#[test]
fn mass_surge_covers_the_crew_round_the_patient() {
    // A crew member within three tiles of the patient is covered, one
    // further off is not.
    let mut world = simulation_world(combat_ship(), REFERENCE_MONEY, 4);
    assert_eq!(world.set_class(0, Class::Medic), Ok(()));
    hold_still(&mut world);
    pick(&mut world, 0, Talent::MassSurge);
    beside(&mut world, 1, 0);
    let patient_at = world.aboard.room.bim_pos(1);
    let near = world
        .aboard
        .room
        .put_for_probe(2, patient_at + vec2(0.0, 1.5 * TILE));
    let far = world
        .aboard
        .room
        .put_for_probe(3, patient_at + vec2(0.0, 6.0 * TILE));
    world.step(&[]);
    assert!((near - patient_at).len() <= class::MASS_SURGE_TILES * TILE);
    assert!((far - patient_at).len() > class::MASS_SURGE_TILES * TILE);
    charge_up(&mut world);
    assert!(
        world
            .step(&[Command::Surge { slot: 0 }])
            .contains(&WorldEvent::Surged { who: 0 })
    );
    assert!(world.is_surging(0) && world.is_surging(1));
    assert!(world.is_surging(2), "within three tiles");
    assert!(!world.is_surging(3), "six tiles off");
}

#[test]
fn a_medic_s_state_dies_with_it_and_a_game_with_a_medic_reads_the_same_twice() {
    let mut world = medic();
    level_up(&mut world, 0, class::SURGE_LEVEL);
    charge_up(&mut world);
    assert_eq!(world.surge_charge(0), 1.0);
    world.aboard.room.kill_for_probe(0);
    world.step(&[]);
    world.step(&[]);
    assert_eq!(
        world.medic_of(0),
        crate::medic::Medic::default(),
        "gone with it"
    );
    // The level is the player's, kept for its buyback (feature 103).
    assert_eq!(world.progress_of(0).level(), class::SURGE_LEVEL as u8);
    // Two runs of a beam on one seed are one world.
    let run = || {
        let mut world = medic();
        hurt(&mut world, 1, 60.0);
        beam(&mut world, 0, Some(1));
        for _ in 0..(3 * STEPS_A_MINUTE) {
            world.step(&[]);
        }
        world_checksum(&world)
    };
    assert_eq!(run(), run());
}

// --- D: carrying a downed body out of the fire, and the field medic ---

/// A medic takes a downed crewmate up into its arms, walks it somewhere
/// else and sets it down there: the body goes where the arms go, walks
/// nowhere of its own, and is left on the deck where it was put down.
#[test]
fn a_medic_carries_a_downed_crewmate_and_sets_it_down_again() {
    let mut world = medic();
    world.aboard.room.knock_out_for_probe(1);
    world.step(&[]);
    assert!(world.aboard.room.is_downed(1), "downed");
    assert_eq!(world.can_carry(0, 1), Ok(()));

    let events = world.step(&[Command::Carry {
        slot: 0,
        who: Some(1),
    }]);
    assert!(
        events.iter().any(|e| matches!(
            e,
            WorldEvent::Carried {
                who: 0,
                patient: Some(1)
            }
        )),
        "{events:?}"
    );
    assert_eq!(world.carrying_of(0), Some(1));

    // The arms go, and the body goes with them: the medic is walked a
    // few tiles off and the two are still within a body's width.
    let from = world.aboard.room.bim_pos(0);
    let to = from + vec2(4.0 * TILE, 0.0);
    world.aboard.room.put_for_probe(0, to);
    world.step(&[]);
    let gap = (world.aboard.room.bim_pos(1) - world.aboard.room.bim_pos(0)).len();
    assert!(gap <= TILE, "carried along: {gap}");

    // And set down, where the medic stands and not back where it fell.
    let events = world.step(&[Command::Carry { slot: 0, who: None }]);
    assert!(
        events.iter().any(|e| matches!(
            e,
            WorldEvent::Carried {
                who: 0,
                patient: None
            }
        )),
        "{events:?}"
    );
    assert_eq!(world.carrying_of(0), None);
    let left = world.aboard.room.bim_pos(1);
    assert!(
        (left - from).len() > 3.0 * TILE,
        "set down where it was carried to, not where it fell"
    );
    assert!(world.aboard.room.is_downed(1), "and still downed");
    // A second set down with empty arms says so rather than doing
    // nothing quietly.
    let events = world.step(&[Command::Carry { slot: 0, who: None }]);
    assert!(refused_with(&events, Refusal::NotCarrying), "{events:?}");
}

/// Who may carry whom: a medic and a hired field medic, a downed body,
/// and one pair of arms to a body.
#[test]
fn only_a_medic_carries_and_only_somebody_downed() {
    let mut world = simulation_world(combat_ship(), REFERENCE_MONEY, 3);
    hold_still(&mut world);
    beside(&mut world, 1, 0);
    // Nobody is a medic yet: the key does nothing for anybody.
    assert_eq!(world.can_carry(0, 1), Err(Refusal::NotCarrying));
    assert_eq!(world.set_class(0, Class::Medic), Ok(()));
    // A crewmate on its feet is nobody's to carry, hurt or whole.
    assert_eq!(world.can_carry(0, 1), Err(Refusal::NotHurt));
    hurt(&mut world, 1, 10.0);
    assert_eq!(world.can_carry(0, 1), Err(Refusal::NotHurt));
    // Itself, never.
    assert_eq!(world.can_carry(0, 0), Err(Refusal::NotACrewmate));
    // Downed is what a carry is for.
    world.aboard.room.knock_out_for_probe(1);
    world.step(&[]);
    assert_eq!(world.can_carry(0, 1), Ok(()));
    // Too far off, and it is out of reach.
    let far = world.aboard.room.bim_pos(0) + vec2(6.0 * TILE, 0.0);
    world.aboard.room.put_for_probe(1, far);
    world.step(&[]);
    assert_eq!(world.can_carry(0, 1), Err(Refusal::OutOfReach));
    beside(&mut world, 1, 0);
    // And a body already in somebody's arms is not picked up twice: a
    // second medic stood on the patient's other side.
    assert_eq!(world.set_class(2, Class::Medic), Ok(()));
    let other = world.aboard.room.bim_pos(1) + vec2(TILE * 0.5, 0.0);
    world.aboard.room.put_for_probe(2, other);
    world.step(&[Command::Carry {
        slot: 0,
        who: Some(1),
    }]);
    assert_eq!(world.carrying_of(0), Some(1));
    assert_eq!(world.can_carry(2, 1), Err(Refusal::AlreadyCarried));
}

/// A field medic under arms goes for a crewmate that is down, picks it
/// up and carries it away from the fight.
#[test]
fn a_field_medic_fetches_a_crewmate_that_is_down_out_of_the_fire() {
    // One player and three aboard, so the medic is a **bot**: the
    // rescue is `Game::bot_stand`'s branch, and a Bim a player steers
    // never runs it.
    let mut world = crate::fixture::crewed_world(combat_ship(), REFERENCE_MONEY, 1, 3);
    // The staged fight puts a machine on the deck a few tiles inside the
    // station's door with a crew member recruited against it, which is
    // what makes the body worth fetching rather than already clear. It
    // is held where it is put and fires nothing, so the fetch is the
    // only thing being measured.
    assert!(world.stage_droid_fight_for_probe(bims::droid::DroidKind::Trooper, None));
    world.step(&[]);
    let medic = 2;
    assert!(world.field_medic_for_probe(medic));
    assert!(world.is_field_medic(medic));
    assert!(world.can_lift(medic), "and may carry");
    assert_eq!(
        world.class_of(medic),
        Class::None,
        "none of the class's own"
    );
    // Crew member 1 down beside the fight, and the medic a few tiles
    // off it rather than back aboard the ship — a field medic looks
    // `RESCUE_LOOK` tiles for somebody, not across the whole station.
    let fight = world.aboard.room.bim_pos(0);
    world.aboard.room.put_for_probe(1, fight + vec2(TILE, 0.0));
    world
        .aboard
        .room
        .put_for_probe(2, fight + vec2(-3.0 * TILE, 0.0));
    world.aboard.room.knock_out_for_probe(1);
    world.step(&[]);
    assert!(world.aboard.room.is_downed(1), "down");

    // A few seconds of the clock: long enough to walk over and reach,
    // well inside the countdown.
    let mut fetched = false;
    for _ in 0..(20 * STEPS_A_MINUTE) {
        world.step(&[]);
        if world.carrying_of(medic) == Some(1) {
            fetched = true;
            break;
        }
    }
    assert!(fetched, "the field medic went and got it");
}

/// **A medic revives in four seconds, anybody else in ten**, a hired
/// field medic as a medic — and the room is handed it on the helper's
/// skill (task 120).
#[test]
fn a_medic_revives_in_four_seconds_and_anybody_else_in_ten() {
    let mut world = simulation_world(combat_ship(), REFERENCE_MONEY, 3);
    assert_eq!(world.revive_seconds(0), bims::health::REVIVE_SECONDS);
    assert_eq!(world.set_class(0, Class::Medic), Ok(()));
    assert_eq!(world.revive_seconds(0), class::MEDIC_REVIVE_SECONDS);
    assert_eq!(world.skill_of(0).revive, class::MEDIC_REVIVE_SECONDS);
    assert_eq!(world.revive_seconds(1), bims::health::REVIVE_SECONDS);
    assert!(world.field_medic_for_probe(2));
    assert_eq!(world.revive_seconds(2), class::MEDIC_REVIVE_SECONDS);
    world.step(&[]);
    assert_eq!(
        world.aboard.room.skill_for_probe(0).revive,
        class::MEDIC_REVIVE_SECONDS,
        "the room is told"
    );
}

/// A hire breaks every beam, and so does a bot lost off the crew — an
/// index is all a link is.
#[test]
fn beams_are_cleared_by_a_hire_and_a_bot_lost() {
    use crate::armour::LootSource;
    use crate::data;
    use worldgen::GalaxyType;
    let galaxy = worldgen::Galaxy::new(data::DEFAULT_SEED, GalaxyType::SpiralTwoArm);
    let (star, station) = crate::spawn(&galaxy).unwrap();
    let mut world = World::start_with_crew(
        combat_ship(),
        REFERENCE_MONEY,
        2,
        2,
        data::DEFAULT_SEED,
        GalaxyType::SpiralTwoArm,
        star,
        station,
    )
    .unwrap();
    // A hire, not a fight: the spawn a peaceful stop (task 111).
    world.set_quiet_sites_for_probe(true);
    assert_eq!(world.set_class(0, Class::Medic), Ok(()));
    hold_still(&mut world);
    assert!(world.mercenary_for_probe());
    let merc = world.residents.as_ref().unwrap().aboard.count() - 1;
    assert!(world.mercenary_fee(merc).is_some());
    // Linked to the crewmate; the hire breaks every beam.
    beside(&mut world, 1, 0);
    assert!(linked(&beam(&mut world, 0, Some(1)), 0, Some(1)));
    assert_eq!(world.patients_of(0), vec![1]);
    let at = world.body_position(LootSource::Resident(merc)).unwrap();
    world.aboard.room.put_for_probe(0, at + vec2(30.0, 0.0));
    let events = world.step(&[Command::Hire {
        slot: 0,
        who: 0,
        resident: merc,
    }]);
    assert!(events.contains(&WorldEvent::Hired { who: 2 }), "{events:?}");
    assert_eq!(world.aboard.crew_count(), 3);
    assert!(world.patients_of(0).is_empty(), "cleared by the hire");
    assert!(!world.is_beaming(0));
    // Linked to the mercenary; the hand dead and gone with the site —
    // a bot is dropped off the crew when the ship leaves — breaks it, and
    // the state list shrinks with the crew.
    hold_still(&mut world);
    beside(&mut world, 2, 0);
    assert!(linked(&beam(&mut world, 0, Some(2)), 0, Some(2)));
    assert_eq!(world.medics.len(), 3);
    world.aboard.room.kill_for_probe(2);
    let events = world.step(&[]);
    assert!(
        events
            .iter()
            .any(|e| matches!(e, WorldEvent::BotLost { who: 2, .. })),
        "{events:?}"
    );
    let gangway = world.aboard.gangway.expect("joined, the ship's side");
    for who in 0..2 {
        world
            .aboard
            .room
            .put_for_probe(who, vec2(gangway.x as f32, gangway.y as f32));
    }
    world.leave_for_probe();
    assert_eq!(world.aboard.crew_count(), 2);
    assert!(world.patients_of(0).is_empty(), "cleared by the bot lost");
    assert_eq!(world.medics.len(), 2);
}
