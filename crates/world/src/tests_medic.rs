//! The medic class (feature 76): `crate::class`'s third class. Its
//! start and its one source of experience; the heal beam; the surge; and
//! each of the ten levels' talents doing what it says, to the medic who
//! holds it alone.

use bims::combat::{Item, WeaponKind};
use bims::health::{MAX_BLOOD, OUT_AT, Part, TREATED_TO};
use bims::math::{Vec2, vec2};
use bims::order::CrewOrder;
use physics::ResourceId;
use shipdesign::fixture::combat_ship;

use crate::class::{self, Class, LEVEL_XP, Side, Talent};
use crate::event::{Refusal, WorldEvent};
use crate::fixture::{REFERENCE_MONEY, simulation_world};
use crate::world::{Command, World};
use crate::world_checksum;

const TILE: f32 = shipdesign::TILE as f32;

/// Steps of the world a minute of its clock is.
const STEPS_A_MINUTE: u32 = 60;

/// The combat ship — bandages and medkits in the hold, bunks for two and
/// more — with two players' crew.
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

/// The crew held where they stand: their own errands off, the timetable
/// cleared, and everybody recruited, which is the one thing that stops
/// a Bim walking off to talk to another (`Game::free_to_talk`) — a beam
/// six tiles long is broken by a chat across the deck otherwise. What
/// the player orders still goes.
fn hold_still(world: &mut World) {
    world.aboard.room.set_autonomous(false);
    for hour in 0..24 {
        world.aboard.room.set_schedule_slot(hour, 0);
    }
    for who in 0..world.aboard.crew_count() as usize {
        // On a cell a body fits in first: where the crew wake up is
        // against the furniture, and the push-out walks one off it.
        let at = world.aboard.room.bim_pos(who);
        world.aboard.room.put_for_probe(who, at);
        world.aboard.room.recruit_for_probe(who, true);
    }
    // And nobody doctors of their own accord: a recruited bot with
    // nothing in sight dresses a crewmate's wound, and these tests want
    // the hands they name.
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

/// Every medkit out of `who`'s pack and nowhere: what "no kit anywhere"
/// takes, now that a helper treats with its own before a shelf's.
fn empty_pack_of_kits(world: &mut World, who: usize) {
    let wanted = Item::Stack(ResourceId::Medkit as u32);
    while let Some(cell) = world
        .aboard
        .room
        .pack(who)
        .iter()
        .position(|i| *i == Some(wanted))
    {
        assert!(world.aboard.room.take(who, cell).is_some());
    }
    world.step(&[]);
}

fn count_in_pack(world: &World, who: usize, resource: ResourceId) -> usize {
    let wanted = Item::Stack(resource as u32);
    world
        .aboard
        .room
        .pack(who)
        .iter()
        .filter(|i| **i == Some(wanted))
        .count()
}

/// A bandage ordered on `patient`'s part by `who`, and the steps until
/// nothing on it bleeds; `None` if it never closed within the budget.
fn bandage_and_wait(world: &mut World, who: u32, patient: u32, part: Part) -> Option<u32> {
    world.step(&[Command::Crew {
        slot: 0,
        order: CrewOrder::Bandage { who, patient, part },
    }]);
    for step in 1..(60 * STEPS_A_MINUTE) {
        world.step(&[]);
        if world.aboard.room.wounds(patient as usize, part) == 0 {
            return Some(step);
        }
    }
    None
}

/// A treatment ordered the same way, and the steps until the patient is
/// dying no more.
fn treat_and_wait(world: &mut World, who: u32, patient: u32, part: Part) -> Option<u32> {
    world.step(&[Command::Crew {
        slot: 0,
        order: CrewOrder::Treat { who, patient, part },
    }]);
    for step in 1..(180 * STEPS_A_MINUTE) {
        world.step(&[]);
        if !world.aboard.room.is_dying(patient as usize) {
            return Some(step);
        }
    }
    None
}

/// `who` cut about — ten shallow wounds on the body, no trauma — and
/// left to bleed `minutes` of the clock, so its blood is well under
/// full and the wounds are open: what the beam is measured against,
/// since a body at `MAX_BLOOD` gains nothing whatever the rate.
fn bleed_down(world: &mut World, who: usize, minutes: u32) {
    for _ in 0..10 {
        world.aboard.room.wound(who, Part::Body, 1.0);
    }
    assert!(!world.aboard.room.is_dying(who));
    for _ in 0..(minutes * STEPS_A_MINUTE) {
        world.step(&[]);
    }
    assert!(
        world.aboard.room.blood(who) < MAX_BLOOD - 5.0,
        "bled: {}",
        world.aboard.room.blood(who)
    );
}

/// The body of `who` shot to nothing: dying, with a trauma on it.
fn make_dying(world: &mut World, who: usize) {
    let out = world.aboard.room.wound(who, Part::Body, Part::Body.max());
    assert!(out.trauma.is_some(), "dying");
    assert!(world.aboard.room.is_dying(who));
}

// --- A: the class, the start, experience ---------------------------------------

#[test]
fn a_medic_sets_out_with_its_kit_the_pool_is_unchanged_and_anyone_still_doctors() {
    let mut world = basic();
    let money = world.money;
    assert_eq!(world.set_class(0, Class::Medic), Ok(()));
    assert_eq!(world.money, money, "a class brings the same money");
    assert_eq!(
        world.aboard.room.weapon(0),
        Some(WeaponKind::LaserPistol.basic()),
        "the pistol stays in hand"
    );
    assert_eq!(
        count_in_pack(&world, 0, ResourceId::Medkit),
        class::MEDIC_START_MEDKITS as usize
    );
    assert_eq!(
        count_in_pack(&world, 0, ResourceId::Bandage),
        class::MEDIC_START_BANDAGES as usize
    );
    assert_eq!(count_in_pack(&world, 1, ResourceId::Medkit), 0);
    // Back to none takes them out; on to a soldier swaps the kits.
    assert_eq!(world.set_class(0, Class::None), Ok(()));
    assert_eq!(count_in_pack(&world, 0, ResourceId::Medkit), 0);
    assert_eq!(count_in_pack(&world, 0, ResourceId::Bandage), 0);
    assert_eq!(world.set_class(0, Class::Medic), Ok(()));
    assert_eq!(world.set_class(0, Class::Soldier), Ok(()));
    assert_eq!(count_in_pack(&world, 0, ResourceId::Medkit), 0);
    assert_eq!(world.grenades_of(0), class::SOLDIER_START_GRENADES);
    assert_eq!(world.money, money);
    // Anybody bandages and treats: a soldier and a classless Bim alike.
    assert_eq!(world.set_class(1, Class::None), Ok(()));
    world.step(&[]);
    world.aboard.room.wound(1, Part::Body, 10.0);
    assert!(world.aboard.room.bandage(0, 1, Part::Body), "a soldier");
    world.aboard.room.wound(0, Part::Body, 10.0);
    assert!(
        world.aboard.room.bandage(1, 0, Part::Body),
        "a classless Bim"
    );
    make_dying(&mut world, 1);
    assert!(world.aboard.room.treat(0, 1, Part::Body));
    // And the abilities are the medic's alone.
    assert_eq!(world.can_beam(0, 1), Err(Refusal::NotAMedic));
    assert_eq!(world.can_surge(1), Err(Refusal::NotAMedic));
    for class in Class::ALL {
        assert_eq!(
            class::can(class, class::Ability::Beam),
            class == Class::Medic
        );
    }
}

#[test]
fn a_medic_earns_five_for_a_finished_bandage_or_treatment_on_a_crewmate_and_nothing_otherwise() {
    let mut world = medic();
    assert_eq!(world.set_class(1, Class::Soldier), Ok(()));
    world.step(&[]);
    // A bandage on a crewmate, finished: five.
    world.aboard.room.wound(1, Part::Body, 10.0);
    assert!(bandage_and_wait(&mut world, 0, 1, Part::Body).is_some());
    assert_eq!(world.progress_of(0).xp, class::XP_HEALED);
    // A treatment with a medkit, finished: five more.
    beside(&mut world, 1, 0);
    make_dying(&mut world, 1);
    assert!(treat_and_wait(&mut world, 0, 1, Part::Body).is_some());
    assert_eq!(world.progress_of(0).xp, 2 * class::XP_HEALED);
    world.aboard.room.patch_up_for_probe(1);
    // On itself: nothing.
    world.aboard.room.wound(0, Part::Body, 10.0);
    assert!(bandage_and_wait(&mut world, 0, 0, Part::Body).is_some());
    assert_eq!(
        world.progress_of(0).xp,
        2 * class::XP_HEALED,
        "not for itself"
    );
    // A soldier doing the same on the medic: nothing for the soldier.
    world.aboard.room.wound(0, Part::Body, 10.0);
    beside(&mut world, 1, 0);
    assert!(bandage_and_wait(&mut world, 1, 0, Part::Body).is_some());
    assert_eq!(world.progress_of(1).xp, 0, "not a medic");
    assert_eq!(world.progress_of(0).xp, 2 * class::XP_HEALED);
    // An interrupted bandage gives nothing until it is finished.
    world.aboard.room.wound(1, Part::Body, 10.0);
    beside(&mut world, 1, 0);
    world.step(&[Command::Crew {
        slot: 0,
        order: CrewOrder::Bandage {
            who: 0,
            patient: 1,
            part: Part::Body,
        },
    }]);
    for _ in 0..30 {
        world.step(&[]);
    }
    assert!(world.aboard.room.bleeding(1) > 0, "not done yet");
    let far = world.aboard.room.bim_pos(0) + vec2(4.0 * TILE, 0.0);
    world.step(&[Command::Crew {
        slot: 0,
        order: CrewOrder::SendTo {
            who: 0,
            x: far.x,
            y: far.y,
        },
    }]);
    for _ in 0..60 {
        world.step(&[]);
    }
    assert_eq!(world.progress_of(0).xp, 2 * class::XP_HEALED, "interrupted");
}

#[test]
fn a_helper_treats_with_the_kit_in_its_own_pack_before_fetching_one_off_a_shelf() {
    // Its own kit: the pack is one down, the hold is untouched, and the
    // medic never leaves the patient's side for a cabinet.
    let mut world = medic();
    let hold = world.ship.design.carrying(ResourceId::Medkit);
    let carried = count_in_pack(&world, 0, ResourceId::Medkit);
    assert!(hold > 0 && carried > 0, "kits both places");
    make_dying(&mut world, 1);
    let stood = world.aboard.room.bim_pos(0);
    let own = treat_and_wait(&mut world, 0, 1, Part::Body).expect("treated");
    let went = (world.aboard.room.bim_pos(0) - stood).len();
    assert_eq!(count_in_pack(&world, 0, ResourceId::Medkit), carried - 1);
    assert_eq!(world.ship.design.carrying(ResourceId::Medkit), hold);
    assert!(went <= 2.0 * TILE, "stayed put: {went}");
    // The same medic with an empty pack walks to a cabinet for one of
    // the hold's, which takes longer and costs the hold a kit.
    let mut world = medic();
    empty_pack_of_kits(&mut world, 0);
    make_dying(&mut world, 1);
    let from_shelf = treat_and_wait(&mut world, 0, 1, Part::Body).expect("treated");
    assert_eq!(count_in_pack(&world, 0, ResourceId::Medkit), 0);
    assert_eq!(world.ship.design.carrying(ResourceId::Medkit), hold - 1);
    assert!(
        own < from_shelf,
        "its own kit is the quicker: {own} against {from_shelf}"
    );
    // And a pack kit is a kit when the hold has none: no field surgery
    // asked for, nothing off the hold, and the pack one down.
    let mut world = medic();
    world.ship.design.cargo[ResourceId::Medkit as usize] = 0;
    world.step(&[]);
    assert_eq!(world.aboard.room.medkits(), 0, "no shelf to walk to");
    assert!(!world.has_talent(0, Talent::FieldSurgeon));
    make_dying(&mut world, 1);
    assert!(treat_and_wait(&mut world, 0, 1, Part::Body).is_some());
    assert_eq!(count_in_pack(&world, 0, ResourceId::Medkit), carried - 1);
    assert_eq!(world.ship.design.carrying(ResourceId::Medkit), 0);
}

#[test]
fn a_medic_earns_five_on_a_mercenary_and_beams_are_cleared_by_a_hire_and_a_dismissal() {
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
    // A bandage on the mercenary: five. The hire is held still like the
    // rest — it arrives on its own errands.
    hold_still(&mut world);
    beside(&mut world, 2, 0);
    // Hard enough to get through the kevlar a mercenary is rolled: a
    // shot the armour takes whole opens no wound.
    world.aboard.room.wound(2, Part::Body, 40.0);
    assert!(world.aboard.room.bleeding(2) > 0, "a wound to dress");
    assert!(bandage_and_wait(&mut world, 0, 2, Part::Body).is_some());
    assert_eq!(world.progress_of(0).xp, class::XP_HEALED);
    // Linked to the mercenary; the dismissal breaks it, and the state
    // list shrinks with the crew.
    beside(&mut world, 2, 0);
    assert!(linked(&beam(&mut world, 0, Some(2)), 0, Some(2)));
    assert_eq!(world.medics.len(), 3);
    // Broke a month on, the hand walks off at the berth: a dismissal.
    let fee = world.mercenary_fee(merc).unwrap_or(world.hired()[0].fee);
    world.money = fee - 1;
    world.clock_minutes += crate::mercenary::MONTH;
    let events = world.step(&[]);
    assert!(
        events.contains(&WorldEvent::MercenaryLeft { who: 2 }),
        "{events:?}"
    );
    assert_eq!(world.aboard.crew_count(), 2);
    assert!(world.patients_of(0).is_empty(), "cleared by the dismissal");
    assert_eq!(world.medics.len(), 2);
}

// --- B: the heal beam ------------------------------------------------------------

#[test]
fn a_linked_patient_does_not_bleed_gains_blood_wakes_past_the_line_and_the_medic_holds_fire() {
    let mut world = medic();
    // Ten wounds open: bleeding a hundred an hour unlinked.
    let before = world.aboard.room.blood(1);
    bleed_down(&mut world, 1, 10);
    assert!(
        world.aboard.room.blood(1) < before - 5.0,
        "bleeding unlinked"
    );
    let events = beam(&mut world, 0, Some(1));
    assert!(linked(&events, 0, Some(1)), "{events:?}");
    assert_eq!(world.patients_of(0), vec![1]);
    assert!(world.aboard.room.is_beaming(0));
    let before = world.aboard.room.blood(1);
    for _ in 0..(10 * STEPS_A_MINUTE) {
        world.step(&[]);
    }
    let gained = world.aboard.room.blood(1) - before;
    let want = class::HEAL_BEAM_BLOOD / 6.0;
    assert!(
        (gained - want).abs() < 0.2,
        "ten minutes of the beam: {gained} of {want}"
    );
    assert_eq!(world.aboard.room.bleeding(1), 10, "the wounds stay open");
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
    // Out cold under the line, the blood comes back over it and the
    // patient wakes — by the ordinary rule.
    world.aboard.room.patch_up_for_probe(1);
    world.aboard.room.knock_out_for_probe(1);
    world.step(&[]);
    assert!(world.aboard.room.is_unconscious(1));
    assert!(linked(&beam(&mut world, 0, Some(1)), 0, Some(1)));
    let mut woke = None;
    for step in 1..(20 * STEPS_A_MINUTE) {
        world.step(&[]);
        if !world.aboard.room.is_unconscious(1) {
            woke = Some(step);
            break;
        }
    }
    assert!(woke.is_some(), "woke");
    assert!(world.aboard.room.blood(1) >= MAX_BLOOD * OUT_AT);
    // A trauma stays, and the part at nothing stays there.
    make_dying(&mut world, 1);
    for _ in 0..(5 * STEPS_A_MINUTE) {
        world.step(&[]);
    }
    assert!(world.aboard.room.is_dying(1), "held, not cured");
    assert_eq!(world.aboard.room.part_health(1, Part::Body), 0.0);
    assert!(world.aboard.room.is_beaming(0));
    // Never above full.
    world.aboard.room.patch_up_for_probe(1);
    for _ in 0..(5 * STEPS_A_MINUTE) {
        world.step(&[]);
    }
    assert_eq!(world.aboard.room.blood(1), MAX_BLOOD);
}

#[test]
fn every_way_the_link_breaks_and_every_reason_a_beam_is_refused() {
    let mut world = medic();
    // Refused: not a medic, itself, an enemy (nobody of the crew), out
    // of range, out of sight.
    assert_eq!(world.can_beam(1, 0), Err(Refusal::NotAMedic));
    assert!(refused_with(
        &beam(&mut world, 1, Some(0)),
        Refusal::NotAMedic
    ));
    assert_eq!(world.can_beam(0, 0), Err(Refusal::NotACrewmate));
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
    world.aboard.room.patch_up_for_probe(1);
    // A fresh world for the rest: a dead crew member stays dead.
    let mut world = medic();
    // The medic goes down: broken.
    assert!(linked(&beam(&mut world, 0, Some(1)), 0, Some(1)));
    world.aboard.room.knock_out_for_probe(0);
    // Out cold at the top of its next tick; the world reads it the step
    // after.
    world.step(&[]);
    world.step(&[]);
    assert!(!world.is_beaming(0), "the medic down");
    world.aboard.room.patch_up_for_probe(0);
    world.step(&[]);
    // Ordered to an errand: broken. Ordered to walk: kept.
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
    world.aboard.room.wound(1, Part::Body, 5.0);
    world.step(&[Command::Crew {
        slot: 0,
        order: CrewOrder::Bandage {
            who: 0,
            patient: 1,
            part: Part::Body,
        },
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

// --- C: the surge ----------------------------------------------------------------

/// Beaming a wounded crewmate until the charge is full.
fn charge_up(world: &mut World) {
    world.aboard.room.wound(1, Part::Legs, 2.0);
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

#[test]
fn the_charge_fills_only_while_the_patient_qualifies_and_a_surge_absorbs_every_hit() {
    let run = |seed_steps: u32| {
        let mut world = medic();
        // A whole patient: nothing to hold, nothing charged.
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
        // A wounded patient: the charge fills, full after forty minutes.
        world.aboard.room.wound(1, Part::Legs, 2.0);
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
        // Emptied — but the beam is still on a patient that wants
        // holding, so the same step puts a step's worth back.
        assert!(world.surge_charge(0) < 0.01, "{}", world.surge_charge(0));
        assert!(world.is_surging(0) && world.is_surging(1));
        // Neither takes a wound, a drained piece, nor a trauma.
        let (blood0, blood1) = (world.aboard.room.blood(0), world.aboard.room.blood(1));
        let wounds1 = world.aboard.room.bleeding(1);
        let out = world.aboard.room.wound(0, Part::Body, Part::Body.max());
        assert_eq!(out.absorbed, Part::Body.max());
        assert_eq!(out.through, 0.0);
        assert!(out.trauma.is_none());
        let out = world
            .aboard
            .room
            .strike(1, Part::Body, Part::Body.max(), true);
        assert_eq!(out.absorbed, Part::Body.max());
        assert!(out.trauma.is_none() && !out.piece_broke);
        assert_eq!(
            world.aboard.room.armour_health(1),
            armour,
            "no armour drained"
        );
        assert_eq!(world.aboard.room.bleeding(1), wounds1, "no wound opened");
        assert!(!world.aboard.room.is_dying(0) && !world.aboard.room.is_dying(1));
        assert_eq!(
            world.aboard.room.part_health(0, Part::Body),
            Part::Body.max()
        );
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
        assert_eq!(world.aboard.room.blood(0), blood0.min(MAX_BLOOD));
        // The patient's own wound bled through the surge — a surge is
        // not a beam.
        assert!(world.aboard.room.blood(1) >= blood1 - 3.0);
        // A hit lands again after.
        let out = world.aboard.room.wound(0, Part::Legs, 1.0);
        assert_eq!(out.through, 1.0);
        world_checksum(&world)
    };
    // Two runs on one seed are one fight.
    assert_eq!(run(7), run(7));
}

// --- D: the ten levels -----------------------------------------------------------

#[test]
fn the_fixed_levels_are_the_beam_the_surge_and_the_mender() {
    // Level one: the beam is a medic's, and a medic's from the first.
    let mut world = medic();
    assert_eq!(world.progress_of(0).level(), 1);
    assert_eq!(world.can_beam(0, 1), Ok(()));
    assert_eq!(world.set_class(1, Class::Soldier), Ok(()));
    assert_eq!(world.can_beam(1, 0), Err(Refusal::NotAMedic));
    // Level three: the surge — pinned above in `can_surge`'s order.
    assert_eq!(class::SURGE_LEVEL, 3);
    // Level seven: *mender* — a beamed patient's parts mend ten times as
    // fast. The same wound, held by a medic at the sixth level and one at
    // the seventh, over an hour.
    let mended_by = |level: u8| {
        let mut world = medic();
        level_up(&mut world, 0, level);
        world.aboard.room.wound(1, Part::Body, 30.0);
        world.aboard.room.bandage(1, 1, Part::Body);
        for _ in 0..(20 * STEPS_A_MINUTE) {
            world.step(&[]);
            if world.aboard.room.bleeding(1) == 0 {
                break;
            }
        }
        assert_eq!(world.aboard.room.bleeding(1), 0);
        beside(&mut world, 1, 0);
        assert!(linked(&beam(&mut world, 0, Some(1)), 0, Some(1)));
        let before = world.aboard.room.part_health(1, Part::Body);
        for _ in 0..(60 * STEPS_A_MINUTE) {
            world.step(&[]);
        }
        assert!(world.is_beaming(0));
        world.aboard.room.part_health(1, Part::Body) - before
    };
    let (plain, mender) = (mended_by(6), mended_by(class::MENDER_LEVEL));
    assert!(plain > 0.0, "a part mends on its own: {plain}");
    let ratio = mender / plain;
    assert!(
        (ratio - class::MENDER_RECOVER).abs() < 0.5,
        "{mender} against {plain}: {ratio}"
    );
    // And nothing mends a part at nothing.
    let mut world = medic();
    level_up(&mut world, 0, class::MENDER_LEVEL);
    make_dying(&mut world, 1);
    assert!(linked(&beam(&mut world, 0, Some(1)), 0, Some(1)));
    for _ in 0..(10 * STEPS_A_MINUTE) {
        world.step(&[]);
    }
    assert_eq!(world.aboard.room.part_health(1, Part::Body), 0.0);
}

#[test]
fn field_dressing_and_surgeon_halve_the_medic_s_own_doctoring() {
    let dressing = |talent: Option<Talent>| {
        let mut world = medic();
        if let Some(t) = talent {
            pick(&mut world, 0, t);
        }
        world.aboard.room.wound(1, Part::Body, 10.0);
        bandage_and_wait(&mut world, 0, 1, Part::Body).expect("dressed")
    };
    let (plain, quick) = (dressing(None), dressing(Some(Talent::FieldDressing)));
    assert!(
        quick < plain * 3 / 4 && quick > plain / 3,
        "a field dressing: {quick} steps against {plain}"
    );
    let treating = |talent: Option<Talent>, who: u32| {
        let mut world = medic();
        if let Some(t) = talent {
            pick(&mut world, 0, t);
        }
        let patient = 1 - who;
        make_dying(&mut world, patient as usize);
        treat_and_wait(&mut world, who, patient, Part::Body).expect("treated")
    };
    let (plain, quick) = (treating(None, 0), treating(Some(Talent::Surgeon), 0));
    assert!(quick < plain, "a surgeon: {quick} steps against {plain}");
    // The talents are the medic's alone: a crewmate treating the medic
    // takes the ordinary time whatever the medic picked.
    let other = treating(Some(Talent::Surgeon), 1);
    assert!(
        other > quick,
        "the crewmate: {other} against the medic's {quick}"
    );
}

#[test]
fn long_beam_and_strong_beam_are_the_beam_s_numbers() {
    let mut world = medic();
    assert_eq!(world.beam_range(0), class::HEAL_BEAM_RANGE);
    assert_eq!(world.beam_blood(0), class::HEAL_BEAM_BLOOD);
    pick(&mut world, 0, Talent::LongBeam);
    assert_eq!(
        world.beam_range(0),
        class::HEAL_BEAM_RANGE * class::LONG_BEAM_RANGE
    );
    assert_eq!(world.beam_blood(0), class::HEAL_BEAM_BLOOD);
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
    // Strong beam: half again the blood an hour, and the patient gains it.
    let gained_with = |talent: Option<Talent>| {
        let mut world = medic();
        if let Some(t) = talent {
            pick(&mut world, 0, t);
        }
        bleed_down(&mut world, 1, 20);
        assert!(linked(&beam(&mut world, 0, Some(1)), 0, Some(1)));
        let before = world.aboard.room.blood(1);
        for _ in 0..(10 * STEPS_A_MINUTE) {
            world.step(&[]);
        }
        (world.beam_blood(0), world.aboard.room.blood(1) - before)
    };
    let (plain_rate, plain) = gained_with(None);
    let (strong_rate, strong) = gained_with(Some(Talent::StrongBeam));
    assert_eq!(strong_rate, plain_rate * class::STRONG_BEAM_BLOOD);
    assert!(
        (strong / plain - class::STRONG_BEAM_BLOOD).abs() < 0.05,
        "{strong} against {plain}"
    );
}

/// The body shot to nothing until the trauma rolled is one that leaves
/// something lasting after treatment.
fn make_dying_with_aftermath(world: &mut World, who: usize) {
    for _ in 0..200 {
        world.aboard.room.patch_up_for_probe(who);
        let out = world.aboard.room.wound(who, Part::Body, Part::Body.max());
        if out.trauma.is_some_and(|t| t.after().is_some()) {
            return;
        }
    }
    panic!("no trauma with an aftermath rolled");
}

#[test]
fn clean_hands_and_steady_hands_are_the_medic_s_treatment() {
    let treated_by = |talent: Option<Talent>, who: u32| {
        let mut world = medic();
        if let Some(t) = talent {
            pick(&mut world, 0, t);
        }
        let patient = 1 - who;
        make_dying_with_aftermath(&mut world, patient as usize);
        assert!(treat_and_wait(&mut world, who, patient, Part::Body).is_some());
        (
            world.aboard.room.lasting(patient as usize).len(),
            world.aboard.room.part_health(patient as usize, Part::Body),
        )
    };
    let (lasting, part) = treated_by(None, 0);
    assert_eq!(lasting, 1, "an ordinary treatment leaves its aftermath");
    assert!((part - Part::Body.max() * TREATED_TO).abs() < 0.5, "{part}");
    let (lasting, part) = treated_by(Some(Talent::CleanHands), 0);
    assert_eq!(lasting, 0, "clean hands leave nothing");
    assert!((part - Part::Body.max() * TREATED_TO).abs() < 0.5);
    let (lasting, part) = treated_by(Some(Talent::SteadyHandsMedic), 0);
    assert_eq!(lasting, 1);
    let want = Part::Body.max() * TREATED_TO * class::STEADY_HANDS_TREATED;
    assert!((part - want).abs() < 0.5, "{part} against {want}");
    // The crewmate treating the medic gets neither.
    let (lasting, part) = treated_by(Some(Talent::CleanHands), 1);
    assert_eq!(lasting, 1);
    assert!((part - Part::Body.max() * TREATED_TO).abs() < 0.5);
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
    world.aboard.room.wound(1, Part::Legs, 2.0);
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
fn self_care_and_double_link_are_the_beam_s_reach() {
    // Self-care: the medic's own wounds do not bleed while it beams.
    let medic_lost = |talent: Option<Talent>| {
        let mut world = medic();
        if let Some(t) = talent {
            pick(&mut world, 0, t);
        }
        world.aboard.room.wound(0, Part::Body, 5.0);
        world.aboard.room.wound(1, Part::Legs, 2.0);
        assert!(linked(&beam(&mut world, 0, Some(1)), 0, Some(1)));
        let before = world.aboard.room.blood(0);
        for _ in 0..(10 * STEPS_A_MINUTE) {
            world.step(&[]);
        }
        assert!(world.is_beaming(0));
        before - world.aboard.room.blood(0)
    };
    assert!(medic_lost(None) > 1.0, "bleeds without it");
    assert!(
        medic_lost(Some(Talent::SelfCare)) <= 0.0,
        "bleeds nothing with it"
    );
    // Double link: two patients, each at the full rate; a third takes the
    // first's place.
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
        bleed_down(&mut world, who, 10);
    }
    assert!(linked(&beam(&mut world, 0, Some(2)), 0, Some(2)));
    assert!(linked(&beam(&mut world, 0, Some(1)), 0, Some(1)));
    assert_eq!(world.patients_of(0), vec![2, 1]);
    let before = [world.aboard.room.blood(1), world.aboard.room.blood(2)];
    for _ in 0..(10 * STEPS_A_MINUTE) {
        world.step(&[]);
    }
    let want = class::HEAL_BEAM_BLOOD / 6.0;
    for who in [1, 2] {
        let gained = world.aboard.room.blood(who) - before[who - 1];
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
fn gunner_medic_fires_while_beaming_and_closing_surge_closes_the_wounds() {
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
    // Closing surge: the patient's open wounds close as the surge ends;
    // the medic's own do not.
    let mut world = medic();
    pick(&mut world, 0, Talent::ClosingSurge);
    level_up(&mut world, 0, class::SURGE_LEVEL);
    for _ in 0..3 {
        world.aboard.room.wound(1, Part::Body, 3.0);
    }
    world.aboard.room.wound(0, Part::Body, 3.0);
    charge_up(&mut world);
    assert!(
        world
            .step(&[Command::Surge { slot: 0 }])
            .contains(&WorldEvent::Surged { who: 0 })
    );
    assert!(world.aboard.room.bleeding(1) > 0, "open while it runs");
    let xp = world.progress_of(0).xp;
    for _ in 0..(20 * STEPS_A_MINUTE) {
        world.step(&[]);
        if !world.is_surging(1) {
            break;
        }
    }
    assert!(!world.is_surging(1));
    assert_eq!(world.aboard.room.bleeding(1), 0, "closed as it ended");
    assert!(
        world.aboard.room.bleeding(0) > 0,
        "the medic's own stay open"
    );
    assert_eq!(world.progress_of(0).xp, xp, "no experience for it");
}

#[test]
fn mass_surge_covers_the_crew_round_the_patient_and_field_surgeon_treats_without_a_kit() {
    // Mass surge: a crew member within three tiles of the patient is
    // covered, one further off is not.
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
    // Field surgeon: with no medkit anywhere, a trauma is treated with
    // nothing, in half the time, for the medic's five; once a fight.
    let mut world = medic();
    let kits = world.ship.design.carrying(ResourceId::Medkit);
    assert!(kits > 0);
    world.ship.design.cargo[ResourceId::Medkit as usize] = 0;
    // The medic's own kit is a kit: no medkit anywhere means its pack
    // emptied of them as well as the hold.
    empty_pack_of_kits(&mut world, 0);
    world.step(&[]);
    assert_eq!(world.aboard.room.medkits(), 0);
    make_dying(&mut world, 1);
    assert!(
        !world.aboard.room.treat(0, 1, Part::Body),
        "no kit, no talent"
    );
    pick(&mut world, 0, Talent::FieldSurgeon);
    world.step(&[]);
    let steps = treat_and_wait(&mut world, 0, 1, Part::Body).expect("treated bare-handed");
    assert_eq!(world.progress_of(0).xp, LEVEL_XP[9] + class::XP_HEALED);
    let mut world = medic();
    make_dying(&mut world, 1);
    let carried = count_in_pack(&world, 0, ResourceId::Medkit);
    let with_kit = treat_and_wait(&mut world, 0, 1, Part::Body).expect("treated");
    assert!(
        steps < with_kit,
        "bare in half the time: {steps} against {with_kit} with a kit"
    );
    assert_eq!(
        count_in_pack(&world, 0, ResourceId::Medkit),
        carried - 1,
        "its own kit is the one spent"
    );
    assert_eq!(
        world.ship.design.carrying(ResourceId::Medkit),
        kits,
        "and the hold is untouched"
    );
    // And the crewmate cannot do it whatever the medic knows.
    let mut world = medic();
    pick(&mut world, 0, Talent::FieldSurgeon);
    world.ship.design.cargo[ResourceId::Medkit as usize] = 0;
    world.step(&[]);
    make_dying(&mut world, 0);
    // The crewmate carries none of its own either: a helper's pack is
    // the first place a kit is looked for.
    assert_eq!(count_in_pack(&world, 1, ResourceId::Medkit), 0);
    assert!(!world.aboard.room.treat(1, 0, Part::Body));
}

#[test]
fn field_surgeon_is_once_a_fight_and_comes_back_when_it_ends() {
    let mut world = medic();
    pick(&mut world, 0, Talent::FieldSurgeon);
    world.ship.design.cargo[ResourceId::Medkit as usize] = 0;
    // A fight: the dock hostile with one of its people standing.
    assert!(world.stage_fight_for_probe());
    let ashore = world.residents.as_mut().unwrap();
    for other in 1..ashore.aboard.count() as usize {
        ashore.aboard.room.kill_for_probe(other);
    }
    ashore.aboard.room.issue(0, bims::combat::Gear::default());
    world.aboard.room.issue(0, bims::combat::Gear::default());
    world.aboard.room.issue(1, bims::combat::Gear::default());
    for _ in 0..3 {
        world.step(&[]);
    }
    assert_eq!(world.aboard.room.medkits(), 0);
    // The medic's crewmate beside it, dying: treated bare once.
    beside(&mut world, 1, 0);
    make_dying(&mut world, 1);
    assert!(treat_and_wait(&mut world, 0, 1, Part::Body).is_some());
    assert!(world.medic_of(0).field_surgery_used, "used this fight");
    beside(&mut world, 1, 0);
    make_dying(&mut world, 1);
    assert!(
        !world.aboard.room.treat(0, 1, Part::Body),
        "not twice in one fight"
    );
    // The fight over — nobody standing — it comes back.
    world
        .residents
        .as_mut()
        .unwrap()
        .aboard
        .room
        .kill_for_probe(0);
    for _ in 0..3 {
        world.step(&[]);
    }
    assert!(
        !world.medic_of(0).field_surgery_used,
        "back after the fight"
    );
    assert!(world.aboard.room.treat(0, 1, Part::Body));
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
    assert_eq!(world.progress_of(0).level(), 1);
    // Two runs of a beam on one seed are one world.
    let run = || {
        let mut world = medic();
        world.aboard.room.wound(1, Part::Body, 5.0);
        beam(&mut world, 0, Some(1));
        for _ in 0..(3 * STEPS_A_MINUTE) {
            world.step(&[]);
        }
        world_checksum(&world)
    };
    assert_eq!(run(), run());
}

// --- carrying a body out of the fire, and the field medic (feature 86) ---

/// A medic takes a crewmate that is out cold up into its arms, walks it
/// somewhere else and sets it down there: the body goes where the arms
/// go, walks nowhere of its own, and is left on the deck where it was
/// put down.
#[test]
fn a_medic_carries_a_crewmate_that_is_out_cold_and_sets_it_down_again() {
    let mut world = medic();
    // Crew member 1 bled past the line: out cold, and worth fetching.
    world.aboard.room.wound(1, Part::Legs, 1000.0);
    world.aboard.room.set_blood_for_probe(1, OUT_AT * 0.5);
    world.step(&[]);
    assert!(world.aboard.room.is_unconscious(1), "out cold");
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
    // A second set down with empty arms says so rather than doing
    // nothing quietly.
    let events = world.step(&[Command::Carry { slot: 0, who: None }]);
    assert!(refused_with(&events, Refusal::NotCarrying), "{events:?}");
}

/// Who may carry whom: a medic and a hired field medic, a body that
/// wants fetching, and one pair of arms to a body.
#[test]
fn only_a_medic_carries_and_only_somebody_worth_fetching() {
    let mut world = simulation_world(combat_ship(), REFERENCE_MONEY, 3);
    hold_still(&mut world);
    beside(&mut world, 1, 0);
    // Nobody is a medic yet: the key does nothing for anybody.
    assert_eq!(world.can_carry(0, 1), Err(Refusal::NotCarrying));
    assert_eq!(world.set_class(0, Class::Medic), Ok(()));
    // A crewmate on its feet and whole is nobody's to carry.
    assert_eq!(world.can_carry(0, 1), Err(Refusal::NotHurt));
    // Itself, never.
    assert_eq!(world.can_carry(0, 0), Err(Refusal::NotACrewmate));
    // A wound is enough — the user asked for the hurt as well as the
    // unconscious.
    world.aboard.room.wound(1, Part::Body, 4.0);
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

/// A hired field medic turns up with two medkits in its pack, is not a
/// medic of the class, and fills its pack back up out of the hold once
/// it has spent one.
#[test]
fn a_field_medic_brings_two_medkits_and_fills_up_again_out_of_combat() {
    let mut world = basic();
    hold_still(&mut world);
    assert!(world.field_medic_for_probe(1), "hired");
    assert!(world.is_field_medic(1));
    assert!(world.can_lift(1), "and may carry");
    // None of the class's own: no talents, and no class.
    assert_eq!(world.class_of(1), Class::None);

    let kits = |world: &World| {
        let wanted = Item::Stack(ResourceId::Medkit as u32);
        world
            .aboard
            .room
            .pack(1)
            .iter()
            .filter(|i| **i == Some(wanted))
            .count() as u32
    };
    assert_eq!(kits(&world), crate::mercenary::MEDIC_MEDKITS);

    // One spent: the room is quiet, so the next step puts one back and
    // the hold is one down.
    let held = world.ship.design.carrying(ResourceId::Medkit);
    assert!(held > 0, "the combat ship carries medkits");
    let cell = world
        .aboard
        .room
        .pack(1)
        .iter()
        .position(|i| *i == Some(Item::Stack(ResourceId::Medkit as u32)))
        .expect("a kit in the pack");
    world.aboard.room.take(1, cell);
    assert_eq!(kits(&world), crate::mercenary::MEDIC_MEDKITS - 1);
    world.step(&[]);
    assert_eq!(kits(&world), crate::mercenary::MEDIC_MEDKITS, "filled up");
    assert_eq!(
        world.ship.design.carrying(ResourceId::Medkit),
        held - 1,
        "out of the hold"
    );
}

/// A field medic under arms goes for a crewmate that is down, picks it
/// up and carries it away from the fight.
#[test]
fn a_field_medic_fetches_a_crewmate_that_is_down_out_of_the_fire() {
    // One player and three aboard, so the medic is a **bot**: the
    // rescue is `Game::bot_stand`'s branch, and a Bim a player steers
    // never runs it.
    let mut world = crate::fixture::crewed_world(combat_ship(), REFERENCE_MONEY, 1, 3);
    let station = world.ship.state.station().expect("docked");
    world.set_hostile(station, true);
    // The staged fight puts an enemy on the deck a few tiles inside the
    // station's door with a crew member recruited against it, which is
    // what makes the body worth fetching rather than already clear.
    world.stage_fight_for_probe();
    world.step(&[]);
    let medic = 2;
    assert!(world.field_medic_for_probe(medic));
    // Crew member 1 down beside the fight, and the medic a few tiles
    // off it rather than back aboard the ship — a field medic looks
    // `RESCUE_LOOK` tiles for somebody, not across the whole station.
    let fight = world.aboard.room.bim_pos(0);
    world.aboard.room.put_for_probe(1, fight + vec2(TILE, 0.0));
    world
        .aboard
        .room
        .put_for_probe(2, fight + vec2(-3.0 * TILE, 0.0));
    world.aboard.room.wound(1, Part::Legs, 1000.0);
    world.aboard.room.set_blood_for_probe(1, OUT_AT * 0.5);
    world.step(&[]);
    assert!(world.aboard.room.is_unconscious(1), "down");

    // A few minutes of the clock: long enough to walk over and reach.
    let mut fetched = false;
    for _ in 0..(5 * STEPS_A_MINUTE) {
        world.step(&[]);
        if world.carrying_of(medic) == Some(1) {
            fetched = true;
            break;
        }
    }
    assert!(fetched, "the field medic went and got it");
}
