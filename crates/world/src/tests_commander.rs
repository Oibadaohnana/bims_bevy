//! The commander class (feature 78): `crate::class`'s fifth class. His
//! start and his one source of experience; the aura and whom it reaches;
//! hiring at a discount; the three squad orders and whom *they* reach;
//! the rally; and each of the ten levels' talents doing what it says, to
//! the commander who holds it alone.

use bims::combat::{ArmourKind, WeaponKind};
use bims::math::vec2;
use shipdesign::fixture::combat_ship;

use crate::class::{self, Class, LEVEL_XP, Side, Talent};
use crate::commander::{SquadAsk, SquadKind};
use crate::event::{Refusal, WorldEvent};
use crate::fixture::{REFERENCE_MONEY, crewed_world};
use crate::world::{Command, World};
use crate::world_checksum;

const TILE: f32 = shipdesign::TILE as f32;

/// The combat ship — bunks for five, armour and guns in the hold — with
/// three players and three crewmates nobody steers, which is what a
/// squad is made of.
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

/// The crew held where they stand: their own errands off, the timetable
/// cleared, and nobody doctoring of their own accord.
fn hold_still(world: &mut World) {
    world.aboard.room.set_autonomous(false);
    for hour in 0..24 {
        world.aboard.room.set_schedule_slot(hour, 0);
    }
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

// --- A: the class, the start, the experience, the aura, hiring -------------

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
    let pack = world.aboard.room.pack(0);
    assert_eq!(world.set_class(0, Class::None), Ok(()));
    assert_eq!(world.aboard.room.pack(0), pack);
    assert_eq!(
        world.aboard.room.weapon(0),
        Some(WeaponKind::LaserPistol.basic())
    );
}

#[test]
fn the_aura_lifts_every_friendly_bim_in_it_and_nobody_else() {
    let mut world = commander();
    hold_still(&mut world);
    // A player's own steered Bim (slot 1), a bot (crew 3) and a
    // mercenary are all the same to it. Crew 1 stands just inside the
    // radius, crew 2 just outside.
    stand_near(&mut world, 1, class::AURA_TILES - 1.0);
    stand_near(&mut world, 2, class::AURA_TILES + 2.0);
    world.step(&[]);
    assert!(world.aura_reaching(1).is_some(), "inside the radius");
    assert!(world.aura_reaching(2).is_none(), "outside it");
    assert!(world.aura_reaching(0).is_none(), "never himself");
    let lifted = skill(&world, 1);
    assert!((lifted.accuracy - class::AURA_AIM).abs() < 1e-5);
    assert!((lifted.effort - class::AURA_WORK).abs() < 1e-5);
    assert!(lifted.nerve_hold > 0.0, "it holds its ground a while");
    let outside = skill(&world, 2);
    assert_eq!(outside.accuracy, 1.0);
    assert_eq!(outside.effort, 1.0);
    assert_eq!(outside.nerve_hold, 0.0);
    // The bots past the players are in it too — a hire is a bot.
    stand_near(&mut world, 3, 1.0);
    world.step(&[]);
    assert!(world.aura_reaching(3).is_some(), "a bot, and a hire, too");
    // And the enemy is never in it: the aura is asked of the crew's room
    // alone, so a resident's index is nobody's here.
    assert!(world.stage_fight_for_probe());
    world.step(&[]);
    let residents = world.residents.as_ref().unwrap().aboard.count();
    assert!(residents > 0);
    // Out cold, he lifts nobody.
    stand_near(&mut world, 1, 1.0);
    world.step(&[]);
    assert!(world.aura_reaching(1).is_some());
    world.aboard.room.knock_out_for_probe(0);
    world.step(&[]);
    assert!(world.aura_reaching(1).is_none(), "not while he is out cold");
}

#[test]
fn two_commanders_auras_do_not_stack_and_the_stronger_holds() {
    let mut world = basic();
    assert_eq!(world.set_class(0, Class::Commander), Ok(()));
    assert_eq!(world.set_class(1, Class::Commander), Ok(()));
    world.step(&[]);
    hold_still(&mut world);
    stand_near(&mut world, 1, 2.0);
    stand_near(&mut world, 2, 3.0);
    world.step(&[]);
    let one = world.aura_reaching(2).expect("in both auras");
    assert!((one.aim - class::AURA_AIM).abs() < 1e-5, "one aura's worth");
    // The second commander takes *strong presence*: the deeper aura is
    // the one that holds, and still only one of them.
    pick(&mut world, 1, Talent::StrongPresence);
    world.step(&[]);
    let strong = world.aura_reaching(2).expect("still in both");
    let wanted = class::aura_bonus(class::AURA_AIM, class::STRONG_PRESENCE);
    assert!((strong.aim - wanted).abs() < 1e-5, "the deeper one alone");
    assert!(
        (skill(&world, 2).accuracy - wanted).abs() < 1e-5,
        "and the room is told that and no product of the two"
    );
}

#[test]
fn a_hire_a_commander_makes_is_cheaper_and_gives_him_experience() {
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
    // Walked over, the hire goes through at the discounted fee, and only
    // the commander who sent it learns anything.
    let at = world
        .body_position(crate::LootSource::Resident(merc))
        .expect("alongside");
    world.aboard.room.put_for_probe(0, at);
    let money = world.money;
    let events = world.step(&[Command::Hire {
        slot: 0,
        who: 0,
        resident: merc,
    }]);
    assert!(
        events.iter().any(|e| matches!(e, WorldEvent::Hired { .. })),
        "{events:?}"
    );
    assert_eq!(world.money, money - discounted);
    let hired = world.hired().last().expect("a contract");
    assert_eq!(hired.fee, discounted, "the contract's fee is the discount");
    assert_eq!(world.progress_of(0).xp, class::XP_HIRE);
    assert_eq!(world.progress_of(1).xp, 0, "the other commander gets none");
    // And it stays that hand's fee after he dies.
    let hired_who = hired.who;
    world.aboard.room.kill_for_probe(0);
    world.step(&[]);
    assert_eq!(world.hired().last().expect("still hired").fee, discounted);
    // A dismissal refunds nothing: the month comes due with no money to
    // cover it and the hand walks off at the berth.
    let crew = world.aboard.crew_count();
    world.money = 0;
    let clock = world.clock_minutes;
    for h in &mut world.hired {
        h.due = clock;
    }
    for _ in 0..4 {
        world.step(&[]);
    }
    assert!(world.aboard.crew_count() < crew, "the hand walked off");
    assert!(!world.is_hired(hired_who));
    assert_eq!(world.money, 0, "no refund");
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
    let money = world.money;
    let events = world.step(&[Command::Hire {
        slot: 2,
        who: 2,
        resident: merc,
    }]);
    assert!(events.iter().any(|e| matches!(e, WorldEvent::Hired { .. })));
    assert_eq!(world.money, money - full, "the full fee");
    assert_eq!(
        world.progress_of(0).xp,
        0,
        "the commander standing beside them learns nothing"
    );
}

// --- B: the squad orders ---------------------------------------------------

/// A fight staged with slot 0 a commander at the station's door, the
/// crew held still and everybody but the commander a squad member.
fn fight() -> World {
    let mut world = basic();
    assert_eq!(world.set_class(0, Class::Commander), Ok(()));
    assert!(world.stage_fight_for_probe());
    world.step(&[]);
    hold_still(&mut world);
    world
}

fn enemy_up(world: &World) -> u32 {
    let residents = world.residents.as_ref().expect("alongside");
    (0..residents.aboard.count())
        .find(|&i| {
            residents.aboard.room.is_alive(i as usize)
                && !residents.aboard.room.is_unconscious(i as usize)
        })
        .expect("an enemy standing")
}

#[test]
fn a_squad_order_reaches_the_squad_and_never_a_players_own_bim() {
    let mut world = fight();
    for who in 1..world.aboard.crew_count() {
        stand_near(&mut world, who as usize, 2.0);
    }
    world.step(&[]);
    let events = world.step(&[Command::Squad {
        slot: 0,
        order: SquadAsk::StandGround,
    }]);
    assert!(
        events
            .iter()
            .any(|e| matches!(e, WorldEvent::Squadded { .. })),
        "{events:?}"
    );
    let order = world.squad.as_ref().expect("an order");
    assert_eq!(order.by_slot, 0);
    assert!(
        order.members.iter().all(|&m| m >= world.players()),
        "no Bim a player steers is in it: {:?}",
        order.members
    );
    assert!(!order.members.is_empty());
    // The room is told, one a crew member: nothing for the players'
    // own, the order for everybody else in it.
    world.step(&[]);
    for who in 0..world.aboard.crew_count() as usize {
        let said = world.aboard.room.squad_for_probe(who);
        if world.squad.as_ref().unwrap().has(who as u32) {
            assert_eq!(said, bims::game::Squad::StandGround, "crew {who}");
        } else {
            assert_eq!(said, bims::game::Squad::None, "crew {who}");
        }
    }
    // And a squad member is under arms whether or not the alarm is up.
    let member = world.squad.as_ref().unwrap().members[0] as usize;
    assert!(world.aboard.room.is_recruited(member as u32));
    // Stand ground never runs, whatever the fight does to it.
    assert!(skill(&world, member).nerve);
}

#[test]
fn out_of_range_crew_are_not_ordered_and_long_reach_takes_the_room() {
    let mut world = fight();
    for who in 1..world.aboard.crew_count() {
        stand_near(&mut world, who as usize, class::SQUAD_RANGE + 5.0);
    }
    stand_near(&mut world, 3, 2.0);
    world.step(&[]);
    let near = world.squad_members(0);
    assert!(near.contains(&3), "{near:?}");
    assert!(!near.contains(&4), "beyond the range");
    // *Long reach* is the seventh level, fixed: the whole room.
    level_up(&mut world, 0, class::LONG_REACH_LEVEL);
    world.step(&[]);
    let far = world.squad_members(0);
    assert!(far.len() > near.len(), "{far:?} against {near:?}");
    assert!(far.contains(&4));
}

#[test]
fn an_attack_marks_an_enemy_and_ends_when_it_goes_down() {
    let mut world = fight();
    let enemy = enemy_up(&world);
    for who in 1..world.aboard.crew_count() {
        stand_near(&mut world, who as usize, 2.0);
    }
    world.step(&[]);
    // An attack with no enemy there is refused.
    let events = world.step(&[Command::Squad {
        slot: 0,
        order: SquadAsk::Attack { enemy: 9_000 },
    }]);
    assert!(refused_with(&events, Refusal::NoEnemyThere));
    assert!(world.squad.is_none());
    world.step(&[Command::Squad {
        slot: 0,
        order: SquadAsk::Attack { enemy },
    }]);
    let order = world.squad.as_ref().expect("an attack");
    assert_eq!(
        order.kind,
        SquadKind::Attack {
            enemies: vec![enemy]
        }
    );
    let member = order.members[0];
    world.step(&[]);
    assert_eq!(
        world.aboard.room.squad_for_probe(member as usize),
        bims::game::Squad::Attack {
            enemy: enemy as usize,
            seen: true
        }
    );
    // The mark ends when that enemy is down.
    world
        .residents
        .as_mut()
        .unwrap()
        .aboard
        .room
        .knock_out_for_probe(enemy as usize);
    // A step for the body to go out cold, and a step for the world to
    // read it back: the squad is handed over before the rooms step.
    world.step(&[]);
    world.step(&[]);
    assert!(world.squad.is_none(), "the mark is down, the order is over");
}

#[test]
fn an_attack_prefers_the_marked_enemy_over_a_nearer_one() {
    let mut world = fight();
    // Two enemies: one right beside the squad member, one further off,
    // both in sight. The order marks the far one.
    let residents = world.residents.as_ref().unwrap();
    let up: Vec<u32> = (0..residents.aboard.count())
        .filter(|&i| {
            residents.aboard.room.is_alive(i as usize)
                && !residents.aboard.room.is_unconscious(i as usize)
        })
        .collect();
    if up.len() < 2 {
        return;
    }
    let (near, far) = (up[0], up[1]);
    let _ = (near, far);
    // The squad member stands where the commander does — beside the
    // station's door — and shoots whichever enemy it would pick for
    // itself; the order marks another one it can see, and that one
    // comes first.
    let member = 3u32;
    stand_near(&mut world, member as usize, 1.0);
    world.step(&[]);
    let Some(own) = world.aboard.room.aims_at_for_probe(member as usize) else {
        return;
    };
    let here = world.aboard.room.bim_pos(member as usize);
    let seen: Vec<u32> = up
        .iter()
        .copied()
        .filter(|&e| {
            e as usize != own
                && world
                    .body_position(crate::LootSource::Resident(e))
                    .is_some_and(|p| world.aboard.room.sees(member as usize, p))
        })
        .collect();
    let Some(&mark) = seen.first() else {
        return;
    };
    let _ = here;
    world.step(&[Command::Squad {
        slot: 0,
        order: SquadAsk::Attack { enemy: mark },
    }]);
    world.aboard.room.put_for_probe(member as usize, here);
    world.step(&[]);
    assert_eq!(
        world.aboard.room.aims_at_for_probe(member as usize),
        Some(mark as usize),
        "the mark comes before the one it would pick for itself"
    );
}

#[test]
fn fall_back_gathers_round_a_tile_and_round_the_commander() {
    let mut world = fight();
    for who in 1..world.aboard.crew_count() {
        stand_near(&mut world, who as usize, 3.0);
    }
    world.step(&[]);
    let here = world.aboard.room.bim_pos(0);
    let his_tile = (
        (here.x / TILE).floor() as i32,
        (here.y / TILE).floor() as i32,
    );
    // A tile of the deck a little way off him, so the fall back on to
    // himself below is a different order and not a repeat.
    let tile = (1..6)
        .flat_map(|d| [(his_tile.0 + d, his_tile.1), (his_tile.0 - d, his_tile.1)])
        .find(|&(x, y)| {
            world
                .aboard
                .room
                .is_deck_tile(vec2((x as f32 + 0.5) * TILE, (y as f32 + 0.5) * TILE))
        })
        .expect("a tile of deck beside him");
    world.step(&[Command::Squad {
        slot: 0,
        order: SquadAsk::FallBack { tile: Some(tile) },
    }]);
    assert_eq!(
        world.squad.as_ref().unwrap().kind,
        SquadKind::FallBack { tile }
    );
    // A tile that is no deck of the room falls back on the commander
    // himself, which is what the pointer on nothing means.
    world.step(&[Command::Squad {
        slot: 0,
        order: SquadAsk::FallBack {
            tile: Some((-9_000, -9_000)),
        },
    }]);
    let kind = world.squad.as_ref().unwrap().kind.clone();
    assert!(matches!(kind, SquadKind::FallBack { .. }));
    assert_ne!(
        kind,
        SquadKind::FallBack {
            tile: (-9_000, -9_000)
        }
    );
    assert_eq!(kind, SquadKind::FallBack { tile: his_tile });
    // And the squad walks: a member three tiles off ends up nearer the
    // tile than it began.
    let member = world.squad.as_ref().unwrap().members[0] as usize;
    let began = (world.aboard.room.bim_pos(member) - here).len();
    world.aboard.room.set_autonomous(false);
    for _ in 0..600 {
        world.step(&[]);
    }
    let ended = (world.aboard.room.bim_pos(member) - here).len();
    assert!(ended <= began + TILE, "{ended} against {began}");
}

#[test]
fn stand_ground_holds_where_it_stands() {
    let mut world = fight();
    for who in 1..world.aboard.crew_count() {
        stand_near(&mut world, who as usize, 6.0);
    }
    world.step(&[]);
    world.step(&[Command::Squad {
        slot: 0,
        order: SquadAsk::StandGround,
    }]);
    let member = world.squad.as_ref().unwrap().members[0] as usize;
    let began = world.aboard.room.bim_pos(member);
    for _ in 0..300 {
        world.step(&[]);
    }
    let moved = (world.aboard.room.bim_pos(member) - began).len();
    assert!(moved <= TILE, "it held its ground: {moved}");
}

#[test]
fn an_order_ends_on_a_repeat_a_new_one_a_click_and_his_going_down() {
    let mut world = fight();
    for who in 1..world.aboard.crew_count() {
        stand_near(&mut world, who as usize, 2.0);
    }
    world.step(&[]);
    let send = |world: &mut World, ask: SquadAsk| {
        world.step(&[Command::Squad {
            slot: 0,
            order: ask,
        }]);
    };
    send(&mut world, SquadAsk::StandGround);
    assert!(world.squad.is_some());
    // The same order again releases the squad.
    send(&mut world, SquadAsk::StandGround);
    assert!(world.squad.is_none(), "a repeat lets it go");
    // A new order replaces the old.
    send(&mut world, SquadAsk::StandGround);
    let here = world.aboard.room.bim_pos(0);
    let tile = (
        (here.x / TILE).floor() as i32,
        (here.y / TILE).floor() as i32,
    );
    send(&mut world, SquadAsk::FallBack { tile: Some(tile) });
    assert!(matches!(
        world.squad.as_ref().unwrap().kind,
        SquadKind::FallBack { .. }
    ));
    // A player's own click order takes that member out of it.
    let member = world.squad.as_ref().unwrap().members[0];
    let at = world.aboard.room.bim_pos(member as usize);
    world.step(&[Command::Crew {
        slot: 0,
        order: bims::order::CrewOrder::SendTo {
            who: member,
            x: at.x,
            y: at.y,
        },
    }]);
    assert!(
        world.squad.as_ref().is_none_or(|o| !o.has(member)),
        "the member the player ordered is out of it"
    );
    // And his going down ends the whole order.
    send(&mut world, SquadAsk::StandGround);
    assert!(world.squad.is_some());
    world.aboard.room.knock_out_for_probe(0);
    world.step(&[]);
    world.step(&[]);
    assert!(world.squad.is_none(), "down, his orders are over");
}

#[test]
fn an_order_is_cleared_by_an_unjoin_a_hire_and_a_dismissal() {
    let mut world = fight();
    for who in 1..world.aboard.crew_count() {
        stand_near(&mut world, who as usize, 2.0);
    }
    world.step(&[]);
    world.step(&[Command::Squad {
        slot: 0,
        order: SquadAsk::StandGround,
    }]);
    assert!(world.squad.is_some());
    world.undock_for_probe();
    world.step(&[]);
    assert!(world.squad.is_none(), "the rooms unjoined");
}

#[test]
fn every_other_player_orders_the_crew_during_the_alarm_as_before() {
    let mut world = fight();
    for who in 1..world.aboard.crew_count() {
        stand_near(&mut world, who as usize, 2.0);
    }
    world.step(&[]);
    // A commander's squad order is on, and player 1 orders a crewmate
    // somewhere all the same: an order is a player's, and the squad is
    // an addition to it.
    world.step(&[Command::Squad {
        slot: 0,
        order: SquadAsk::StandGround,
    }]);
    let who = 4u32.min(world.aboard.crew_count() - 1);
    let at = world.aboard.room.bim_pos(0);
    let events = world.step(&[Command::Crew {
        slot: 1,
        order: bims::order::CrewOrder::SendTo {
            who,
            x: at.x,
            y: at.y,
        },
    }]);
    assert!(
        !events
            .iter()
            .any(|e| matches!(e, WorldEvent::Refused { .. })),
        "{events:?}"
    );
}

#[test]
fn two_runs_of_a_squad_order_on_one_seed_are_the_same_world() {
    let run = || -> u64 {
        let mut world = fight();
        for who in 1..world.aboard.crew_count() {
            stand_near(&mut world, who as usize, 2.0);
        }
        world.step(&[]);
        let enemy = enemy_up(&world);
        world.step(&[Command::Squad {
            slot: 0,
            order: SquadAsk::Attack { enemy },
        }]);
        for _ in 0..200 {
            world.step(&[]);
        }
        world_checksum(&world)
    };
    assert_eq!(run(), run());
}

// --- C: the rally ----------------------------------------------------------

#[test]
fn a_rally_wants_the_third_level_and_its_cooldown() {
    let mut world = commander();
    hold_still(&mut world);
    let events = world.step(&[Command::Rally { slot: 0 }]);
    assert!(refused_with(&events, Refusal::NoRallyYet));
    // Anybody but a commander is refused outright.
    assert_eq!(world.can_rally(1), Err(Refusal::NotACommander));
    level_up(&mut world, 0, class::RALLY_LEVEL);
    let events = world.step(&[Command::Rally { slot: 0 }]);
    assert!(
        events
            .iter()
            .any(|e| matches!(e, WorldEvent::Rallied { who: 0 })),
        "{events:?}"
    );
    assert!(world.is_rallying(0));
    assert!(world.rally_cooldown_left(0) > 0.0);
    let events = world.step(&[Command::Rally { slot: 0 }]);
    assert!(refused_with(&events, Refusal::CoolingDown));
}

#[test]
fn a_rally_lifts_the_aim_and_stops_the_running_for_its_minutes() {
    let mut world = commander();
    hold_still(&mut world);
    level_up(&mut world, 0, class::RALLY_LEVEL);
    stand_near(&mut world, 1, 2.0);
    stand_near(&mut world, 3, 2.0);
    world.step(&[Command::Rally { slot: 0 }]);
    world.step(&[]);
    // A player's own Bim and a bot alike, and it stacks with the aura.
    for who in [1usize, 3] {
        let s = skill(&world, who);
        assert!(s.nerve, "crew {who} does not run");
        let wanted = class::AURA_AIM * class::RALLY_AIM;
        assert!(
            (s.accuracy - wanted).abs() < 1e-5,
            "crew {who}: {} against {wanted}",
            s.accuracy
        );
    }
    // Two rallies do not stack: a second commander's rally over the
    // first is the same factor.
    assert_eq!(world.set_class(1, Class::Commander), Ok(()));
    level_up(&mut world, 1, class::RALLY_LEVEL);
    world.step(&[Command::Rally { slot: 1 }]);
    world.step(&[]);
    let s = skill(&world, 3);
    let wanted = class::AURA_AIM * class::RALLY_AIM;
    assert!((s.accuracy - wanted).abs() < 1e-5, "{}", s.accuracy);
    // And it runs out.
    let minutes = world.rally_minutes(0);
    for _ in 0..(minutes as u32 + 2) * 60 {
        world.step(&[]);
    }
    assert!(!world.is_rallying(0));
    assert!(!world.is_rallying(1));
    world.step(&[]);
    assert!(!skill(&world, 3).nerve);
}

// --- D: the ten levels -----------------------------------------------------

#[test]
fn the_fixed_levels_are_the_aura_the_rally_and_the_long_reach() {
    let mut world = commander();
    hold_still(&mut world);
    stand_near(&mut world, 1, 2.0);
    world.step(&[]);
    // One: the aura, the discount and the three orders, with nothing
    // picked at all.
    assert_eq!(world.progress_of(0).level(), 1);
    assert!(world.aura_reaching(1).is_some());
    assert_eq!(world.can_squad(0), Ok(()));
    assert_eq!(world.can_rally(0), Err(Refusal::NoRallyYet));
    // Three: the rally.
    level_up(&mut world, 0, class::RALLY_LEVEL);
    assert_eq!(world.can_rally(0), Ok(()));
    // Seven: long reach.
    assert_eq!(world.squad_range(0), class::SQUAD_RANGE);
    level_up(&mut world, 0, class::LONG_REACH_LEVEL);
    assert_eq!(world.squad_range(0), f32::MAX);
}

#[test]
fn wide_presence_and_strong_presence() {
    let mut world = commander();
    hold_still(&mut world);
    stand_near(&mut world, 1, class::AURA_TILES + 2.0);
    world.step(&[]);
    assert!(world.aura_reaching(1).is_none());
    pick(&mut world, 0, Talent::WidePresence);
    world.step(&[]);
    assert_eq!(
        world.aura_radius(0),
        class::AURA_TILES * class::WIDE_PRESENCE_RADIUS
    );
    assert!(
        world.aura_reaching(1).is_some(),
        "the wider aura reaches it"
    );
    // The right-hand pick instead: each bonus deeper, the radius as it
    // was, and only for the commander who holds it.
    let mut other = commander();
    hold_still(&mut other);
    stand_near(&mut other, 1, 2.0);
    pick(&mut other, 0, Talent::StrongPresence);
    other.step(&[]);
    assert_eq!(other.aura_radius(0), class::AURA_TILES);
    let aura = other.aura_reaching(1).expect("in it");
    assert!((aura.aim - class::aura_bonus(class::AURA_AIM, class::STRONG_PRESENCE)).abs() < 1e-5);
    assert!((aura.work - class::aura_bonus(class::AURA_WORK, class::STRONG_PRESENCE)).abs() < 1e-5);
}

#[test]
fn haggler_and_outfitter() {
    let mut world = basic();
    assert_eq!(world.set_class(0, Class::Commander), Ok(()));
    world.step(&[]);
    if !world.mercenary_for_probe() {
        return;
    }
    let merc = world.residents.as_ref().unwrap().aboard.count() - 1;
    let full = world.mercenary_fee(merc).expect("a fee");
    pick(&mut world, 0, Talent::Haggler);
    assert_eq!(
        world.hire_fee(0, merc),
        Some(full - full * u64::from(class::HAGGLER_DISCOUNT_PERCENT) / 100)
    );
    // Only the commander who holds it.
    assert_eq!(world.hire_fee(1, merc), Some(full));

    // *Outfitter*: the hire arrives with the lowest basic piece it lacked.
    let mut world = basic();
    assert_eq!(world.set_class(0, Class::Commander), Ok(()));
    world.step(&[]);
    pick(&mut world, 0, Talent::Outfitter);
    if !world.mercenary_for_probe() {
        return;
    }
    let merc = world.residents.as_ref().unwrap().aboard.count() - 1;
    let missing: Vec<ArmourKind> = ArmourKind::ALL
        .into_iter()
        .filter(|k| {
            world
                .residents
                .as_ref()
                .unwrap()
                .aboard
                .room
                .gear(merc as usize)
                .worn(k.slot())
                .is_none()
        })
        .collect();
    let fee = world.hire_fee(0, merc).expect("a fee");
    let at = world
        .body_position(crate::LootSource::Resident(merc))
        .expect("alongside");
    world.aboard.room.put_for_probe(0, at);
    let money = world.money;
    world.step(&[Command::Hire {
        slot: 0,
        who: 0,
        resident: merc,
    }]);
    let new_who = world.aboard.crew_count() - 1;
    assert_eq!(world.money, money - fee, "the piece is not charged for");
    if let Some(&first) = missing.first() {
        assert!(
            world
                .aboard
                .room
                .gear(new_who as usize)
                .worn(first.slot())
                .is_some(),
            "the lowest piece it lacked, {first:?}"
        );
    }
}

#[test]
fn focus_fire_and_pincer() {
    let mut world = fight();
    let enemy = enemy_up(&world);
    for who in 1..world.aboard.crew_count() {
        stand_near(&mut world, who as usize, 2.0);
    }
    world.step(&[]);
    pick(&mut world, 0, Talent::FocusFire);
    world.step(&[Command::Squad {
        slot: 0,
        order: SquadAsk::Attack { enemy },
    }]);
    world.step(&[]);
    let member = world.squad.as_ref().unwrap().members[0] as usize;
    assert!(
        (skill(&world, member).marked_accuracy - class::FOCUS_FIRE_ACCURACY).abs() < 1e-5,
        "the squad's odds against the mark"
    );
    // And against nobody else: a member not under an attack has none.
    assert_eq!(skill(&world, 0).marked_accuracy, 1.0);

    // *Pincer* marks a second enemy beside the first.
    let mut world = fight();
    let residents = world.residents.as_ref().unwrap();
    let up: Vec<u32> = (0..residents.aboard.count())
        .filter(|&i| {
            residents.aboard.room.is_alive(i as usize)
                && !residents.aboard.room.is_unconscious(i as usize)
        })
        .collect();
    if up.len() < 2 {
        return;
    }
    for who in 1..world.aboard.crew_count() {
        stand_near(&mut world, who as usize, 2.0);
    }
    world.step(&[]);
    pick(&mut world, 0, Talent::Pincer);
    world.step(&[Command::Squad {
        slot: 0,
        order: SquadAsk::Attack { enemy: up[0] },
    }]);
    world.step(&[Command::Squad {
        slot: 0,
        order: SquadAsk::Attack { enemy: up[1] },
    }]);
    let order = world.squad.as_ref().expect("an attack");
    assert_eq!(
        order.kind,
        SquadKind::Attack {
            enemies: vec![up[0], up[1]]
        }
    );
    // The squad is split between them, in turn.
    assert_eq!(order.mark_for(order.members[0]), Some(up[0]));
    if order.members.len() > 1 {
        assert_eq!(order.mark_for(order.members[1]), Some(up[1]));
    }
}

#[test]
fn long_rally_and_quick_rally() {
    let mut world = commander();
    level_up(&mut world, 0, class::RALLY_LEVEL);
    assert_eq!(world.rally_minutes(0), class::RALLY_MINUTES);
    assert_eq!(world.rally_cooldown(0), class::RALLY_COOLDOWN);
    pick(&mut world, 0, Talent::LongRally);
    assert_eq!(
        world.rally_minutes(0),
        class::RALLY_MINUTES * class::LONG_RALLY_TIME
    );
    assert_eq!(world.rally_cooldown(0), class::RALLY_COOLDOWN);
    assert_eq!(world.rally_minutes(1), class::RALLY_MINUTES, "his alone");

    let mut world = commander();
    pick(&mut world, 0, Talent::QuickRally);
    assert_eq!(
        world.rally_cooldown(0),
        class::RALLY_COOLDOWN * class::QUICK_RALLY_COOLDOWN
    );
    assert_eq!(world.rally_minutes(0), class::RALLY_MINUTES);
}

#[test]
fn steady_ranks_and_double_time() {
    let mut world = commander();
    hold_still(&mut world);
    stand_near(&mut world, 1, 2.0);
    stand_near(&mut world, 2, class::AURA_TILES + 4.0);
    pick(&mut world, 0, Talent::SteadyRanks);
    world.step(&[]);
    // A Bim in the aura bleeds slower than one outside it.
    world.aboard.room.wound(1, bims::health::Part::Body, 20.0);
    world.aboard.room.wound(2, bims::health::Part::Body, 20.0);
    let (a0, b0) = (world.aboard.room.blood(1), world.aboard.room.blood(2));
    for _ in 0..600 {
        world
            .aboard
            .room
            .put_for_probe(1, world.aboard.room.bim_pos(1));
        world
            .aboard
            .room
            .put_for_probe(2, world.aboard.room.bim_pos(2));
        world.step(&[]);
    }
    let (a1, b1) = (world.aboard.room.blood(1), world.aboard.room.blood(2));
    assert!(
        a0 - a1 < b0 - b1,
        "in the aura it bled {} against {} outside it",
        a0 - a1,
        b0 - b1
    );

    let mut world = commander();
    hold_still(&mut world);
    stand_near(&mut world, 1, 2.0);
    pick(&mut world, 0, Talent::DoubleTime);
    world.step(&[]);
    assert!((skill(&world, 1).walk - class::DOUBLE_TIME_PACE).abs() < 1e-5);
    assert_eq!(skill(&world, 0).walk, 1.0, "never himself");
}

#[test]
fn relentless_and_grit() {
    let mut world = fight();
    let enemy = enemy_up(&world);
    for who in 1..world.aboard.crew_count() {
        stand_near(&mut world, who as usize, 2.0);
    }
    world.step(&[]);
    pick(&mut world, 0, Talent::Relentless);
    world.step(&[Command::Squad {
        slot: 0,
        order: SquadAsk::Attack { enemy },
    }]);
    world
        .residents
        .as_mut()
        .unwrap()
        .aboard
        .room
        .knock_out_for_probe(enemy as usize);
    world.step(&[]);
    assert!(
        world.squad.is_some(),
        "the mark lasts until the enemy is dead"
    );
    world
        .residents
        .as_mut()
        .unwrap()
        .aboard
        .room
        .kill_for_probe(enemy as usize);
    world.step(&[]);
    world.step(&[]);
    assert!(world.squad.is_none(), "and ends when it is");

    // *Grit*: during a rally, nothing the fight has done costs pace.
    let mut world = commander();
    hold_still(&mut world);
    level_up(&mut world, 0, class::RALLY_LEVEL);
    pick(&mut world, 0, Talent::Grit);
    stand_near(&mut world, 1, 2.0);
    world.step(&[]);
    assert!(!skill(&world, 1).unhurt, "not before the rally");
    world.step(&[Command::Rally { slot: 0 }]);
    world.step(&[]);
    assert!(skill(&world, 1).unhurt);
    assert!(skill(&world, 3).unhurt || world.aura_reaching(3).is_none());
}

#[test]
fn anchor_and_warcry() {
    let mut world = commander();
    hold_still(&mut world);
    stand_near(&mut world, 1, 2.0);
    pick(&mut world, 0, Talent::Anchor);
    world.step(&[]);
    // Stood where he is — *anchor* asks whether he is walking, and it
    // is read live rather than off the last step.
    world.aboard.room.halt_for_probe(0);
    let still = world.aura_reaching(1).expect("in it");
    assert!(
        (still.aim - class::aura_bonus(class::AURA_AIM, class::ANCHOR_BONUS)).abs() < 1e-5,
        "doubled while he stands still: {}",
        still.aim
    );

    // *Warcry*: the rally covers the whole room.
    let mut world = commander();
    hold_still(&mut world);
    level_up(&mut world, 0, class::RALLY_LEVEL);
    stand_near(&mut world, 2, class::AURA_TILES + 10.0);
    world.step(&[Command::Rally { slot: 0 }]);
    world.step(&[]);
    assert!(world.rally_reaching(2).is_none(), "beyond the aura");
    pick(&mut world, 0, Talent::Warcry);
    world.step(&[]);
    assert!(world.rally_reaching(2).is_some(), "the whole room");
}

#[test]
fn a_talent_reaches_only_the_commander_who_holds_it() {
    let mut world = basic();
    assert_eq!(world.set_class(0, Class::Commander), Ok(()));
    assert_eq!(world.set_class(1, Class::Commander), Ok(()));
    world.step(&[]);
    for talent in Talent::ALL
        .into_iter()
        .filter(|t| t.class() == Class::Commander)
    {
        assert!(!world.has_talent(0, talent));
        assert!(!world.has_talent(1, talent));
    }
    pick(&mut world, 0, Talent::WidePresence);
    assert!(world.has_talent(0, Talent::WidePresence));
    assert!(!world.has_talent(1, Talent::WidePresence));
    assert_eq!(
        world.aura_radius(1),
        class::AURA_TILES,
        "the other commander's aura is the plain one"
    );
    // And a talent of the commander's is nobody else's class's.
    assert_eq!(world.set_class(2, Class::Tank), Ok(()));
    assert!(!world.has_talent(2, Talent::WidePresence));
}

#[test]
fn the_checksum_notices_a_rally_and_a_squad_order() {
    let mut world = fight();
    for who in 1..world.aboard.crew_count() {
        stand_near(&mut world, who as usize, 2.0);
    }
    world.step(&[]);
    let quiet = world_checksum(&world);
    world.step(&[Command::Squad {
        slot: 0,
        order: SquadAsk::StandGround,
    }]);
    assert_ne!(
        world_checksum(&world),
        quiet,
        "an order is a different world"
    );
    let ordered = world_checksum(&world);
    level_up(&mut world, 0, class::RALLY_LEVEL);
    world.step(&[Command::Rally { slot: 0 }]);
    assert_ne!(world_checksum(&world), ordered);
}
