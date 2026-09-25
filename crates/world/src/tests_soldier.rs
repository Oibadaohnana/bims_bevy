//! The soldier class (feature 75): `crate::class`'s second class. A
//! class owns abilities and nothing else; the brace; the grenades; and
//! each of the ten levels' talents doing what it says, to the soldier
//! who holds it alone.

use bims::combat::{Gear, Item, WeaponKind};
use bims::droid::{DroidKind, DroidPart};
use bims::health::Part;
use bims::math::{Vec2, vec2};
use economy::trade_price;
use physics::ResourceId;
use shipdesign::Rotation;
use shipdesign::fixture::flyer;
use shipdesign::parts::PartKind;

use crate::class::{self, Ability, Class, LEVEL_XP, Side, Talent};
use crate::deploy::{Deck, DeployKind, Deployable};
use crate::event::{Refusal, WorldEvent};
use crate::fixture::{REFERENCE_MONEY, simulation_world};
use crate::world::{Command, World};
use crate::world_checksum;

const TILE: f32 = shipdesign::TILE as f32;

/// Seconds of the room's clock a world step is: a game minute is a real
/// second at 1×, and a step is a sixtieth of one.
const SECONDS_A_STEP: f64 = crate::data::STEP_MINUTES / time::MINUTES_PER_SECOND;

/// Whether `took` off a body is what `dealt` comes to on one of its
/// parts: the whole of it, or the part's own total when that is less —
/// a burst on the head takes the head's five and no more.
fn plausible(took: f32, dealt: f32) -> bool {
    Part::ALL
        .iter()
        .any(|p| (took - dealt.min(p.max())).abs() < 1.0)
}

/// [`plausible`] for the staged machine: one of its four parts took the
/// whole of it, or the part's own total when that is less.
fn plausible_on_machine(world: &World, took: f32, dealt: f32) -> bool {
    let body = machine(world).body;
    DroidPart::ALL
        .iter()
        .any(|&p| (took - dealt.min(body.max(p))).abs() < 1.0)
}

/// The soldier's weapon out of its hand, its pack kept: nothing but the
/// grenade lands on anybody.
fn disarm(world: &mut World, who: usize) {
    let mut gear = world.aboard.room.gear(who);
    gear.weapon = None;
    world.aboard.room.issue(who, gear);
}

fn basic() -> World {
    simulation_world(flyer(2), REFERENCE_MONEY, 2)
}

/// [`basic`] with slot 0 a soldier.
fn soldier() -> World {
    let mut world = basic();
    assert_eq!(world.set_class(0, Class::Soldier), Ok(()));
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
    assert_eq!(world.progress_of(who as u32).level(), level);
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

fn grenades(world: &World, who: u32) -> u32 {
    world.grenades_of(who)
}

fn give_grenade(world: &mut World, who: usize) {
    assert!(
        world
            .aboard
            .room
            .give(who, None, Item::Stack(ResourceId::Grenade as u32)),
        "room in the pack"
    );
}

fn tile_of(p: Vec2) -> (i32, i32) {
    ((p.x / TILE).floor() as i32, (p.y / TILE).floor() as i32)
}

fn middle(tile: (i32, i32)) -> Vec2 {
    vec2((tile.0 as f32 + 0.5) * TILE, (tile.1 as f32 + 0.5) * TILE)
}

/// The machines' dock with one of them stood four tiles down the
/// corridor from the soldier, held where it is put and firing nothing
/// (`stage_droid_fight_for_probe`): a target and nothing else.
fn fight() -> World {
    let mut world = soldier();
    level_up(&mut world, 0, class::GRENADE_LEVEL);
    assert!(world.stage_droid_fight_for_probe(DroidKind::Trooper, None));
    for _ in 0..3 {
        world.step(&[]);
    }
    world
}

/// The staged machine.
fn machine(world: &World) -> &bims::droid::Droid {
    world
        .residents
        .as_ref()
        .unwrap()
        .aboard
        .room
        .droid(0)
        .expect("the staged machine")
}

/// Where the machine stands, in the crew's room.
fn machine_at(world: &World) -> Vec2 {
    let residents = world.residents.as_ref().unwrap();
    let p = world
        .aboard
        .from_station(residents.aboard.position(0))
        .expect("on the joined deck");
    vec2(p.x as f32, p.y as f32)
}

/// What is left of the machine, its four parts added up.
fn machine_health(world: &World) -> f32 {
    let body = machine(world).body;
    DroidPart::ALL.iter().map(|&p| body.health(p)).sum()
}

fn throw(world: &mut World, slot: u32, tile: (i32, i32)) -> Vec<WorldEvent> {
    world.step(&[Command::Throw {
        slot,
        x: tile.0,
        y: tile.1,
    }])
}

/// Step until no grenade is in the room: how many steps it took.
fn run_until_burst(world: &mut World) -> u32 {
    for step in 1..2_000 {
        world.step(&[]);
        if world.aboard.room.grenades().is_empty() {
            return step;
        }
    }
    panic!("the grenade never burst");
}

/// Step to the burst and no further: what the machine lost to it, and
/// where it stood as it went off — the step's own drop, so nothing that
/// landed before or after is counted.
fn machine_burst(world: &mut World) -> (f32, Vec2) {
    for _ in 1..2_000 {
        let health = machine_health(world);
        // Where the crew's room has it as a target, which is what the
        // burst reaches for.
        let at = world
            .aboard
            .room
            .combat_targets_for_probe()
            .first()
            .copied()
            .flatten()
            .unwrap_or_else(|| machine_at(world));
        world.step(&[]);
        if world.aboard.room.grenades().is_empty() {
            return (health - machine_health(world), at);
        }
    }
    panic!("the grenade never burst");
}

/// A run of `n` tiles of deck from the soldier's own tile in one of the
/// four directions, every one thrown at with nothing in the way: the
/// tiles, nearest first.
fn open_run(world: &World, who: u32, n: i32) -> Vec<(i32, i32)> {
    let here = tile_of(world.aboard.room.bim_pos(who as usize));
    let from = world.aboard.room.bim_pos(who as usize);
    for (dx, dy) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
        let run: Vec<(i32, i32)> = (1..=n)
            .map(|k| (here.0 + dx * k, here.1 + dy * k))
            .collect();
        if run.iter().all(|&t| {
            let at = middle(t);
            world.aboard.room.is_deck_tile(at) && world.aboard.room.line_clear(from, at)
        }) {
            return run;
        }
    }
    panic!("no open run of {n} tiles from the soldier");
}

/// The design tile of the ship a room point is on.
fn design_tile(world: &World, at: Vec2) -> (u32, u32) {
    let (_, p) = world
        .aboard
        .design_of(worldgen::math::dvec2(at.x as f64, at.y as f64));
    let t = shipdesign::TILE as f64;
    (
        (p.x / t).floor().max(0.0) as u32,
        (p.y / t).floor().max(0.0) as u32,
    )
}

/// Sandbags laid on a room tile of the ship, the way an engineer's
/// finish would lay them, without the engineer.
fn lay_bags(world: &mut World, at: Vec2) -> u32 {
    let id = world.next_deployable;
    world.next_deployable += 1;
    world.deployables.push(Deployable {
        id,
        kind: DeployKind::Sandbags,
        owner_slot: 1,
        deck: Deck::Ship,
        tile: design_tile(world, at),
        health: crate::deploy::SANDBAG_HEALTH,
    });
    id
}

// --- A: a class owns abilities, never jobs or money --------------------------

#[test]
fn every_class_equips_every_weapon_takes_every_errand_and_places_every_site() {
    // Three crew, one of each class; a fresh world for each, since an
    // errand put down stays on the queue.
    let classed = || {
        let mut world = simulation_world(flyer(3), REFERENCE_MONEY, 3);
        assert_eq!(world.set_class(0, Class::Engineer), Ok(()));
        assert_eq!(world.set_class(1, Class::Soldier), Ok(()));
        assert_eq!(world.class_of(2), Class::None);
        world
    };
    for who in 0..3u32 {
        let mut world = classed();
        // Every weapon into the hand, whatever the class.
        for kind in WeaponKind::ALL {
            let item = Item::Weapon(kind.basic());
            assert!(world.aboard.room.give(who as usize, None, item));
            let cell = world
                .aboard
                .room
                .pack(who as usize)
                .iter()
                .position(|i| *i == Some(item))
                .unwrap();
            let events = world.step(&[Command::Equip {
                slot: who,
                who,
                cell: cell as u32,
            }]);
            assert!(
                !events
                    .iter()
                    .any(|e| matches!(e, WorldEvent::Refused { .. })),
                "{who} equips {kind:?}: {events:?}"
            );
            assert_eq!(world.aboard.room.weapon(who as usize), Some(kind.basic()));
            // What came off into the pack goes, so the pack has room for
            // the next.
            let off: Vec<usize> = world
                .aboard
                .room
                .pack(who as usize)
                .iter()
                .enumerate()
                .filter(|(_, i)| matches!(i, Some(Item::Weapon(_))))
                .map(|(c, _)| c)
                .collect();
            for cell in off {
                world.aboard.room.take(who as usize, cell);
            }
        }
        // Every errand: a gun picked up off the deck and its own wound
        // dressed are each an order the room takes for anybody. The
        // order's answer is on the deck, never a refusal; what says it was
        // taken is the errand on hand. Stood on a cell a body fits in
        // first: where the crew wake up is against the furniture.
        let at = world.aboard.room.bim_pos(who as usize);
        let here = world.aboard.room.put_for_probe(who as usize, at);
        let gun =
            world
                .aboard
                .room
                .drop_for_probe(here, WeaponKind::LaserPistol.basic(), who as usize);
        let order = bims::order::CrewOrder::PickUp { who, item: gun };
        world.step(&[Command::Crew { slot: who, order }]);
        assert_eq!(
            world.aboard.room.task_kind_for_probe(who as usize),
            Some(bims::game::JOB_FETCH),
            "{who} takes {order:?}"
        );
        world.aboard.room.set_bandages_for_probe(who as usize, 1);
        world
            .aboard
            .room
            .wound(who as usize, bims::health::Part::Legs, 1.0);
        let order = bims::order::CrewOrder::Bandage {
            who,
            patient: who,
            part: bims::health::Part::Legs,
        };
        world.step(&[Command::Crew { slot: who, order }]);
        assert_eq!(
            world.aboard.room.task_kind_for_probe(who as usize),
            Some(bims::game::JOB_BANDAGE),
            "{who} takes {order:?}"
        );
    }
    // Every site: the rule never asks the class, so it answers the same
    // for every part whoever asks — there is no slot in the question.
    let world = classed();
    for kind in [
        PartKind::Sandbags,
        PartKind::Wall,
        PartKind::Table,
        PartKind::WallLight,
    ] {
        let answer = world.can_place_site(kind, (3, 3), Rotation::R0);
        assert_eq!(world.can_place_site(kind, (3, 3), Rotation::R0), answer);
    }
    // `class::can` answers for an ability and nothing else.
    for ability in Ability::ALL {
        assert!(!class::can(Class::None, ability));
    }
    assert!(class::can(Class::Engineer, Ability::Deploy));
    assert!(!class::can(Class::Soldier, Ability::Deploy));
    assert!(class::can(Class::Soldier, Ability::Brace));
    assert!(class::can(Class::Soldier, Ability::Throw));
    assert!(!class::can(Class::Engineer, Ability::Brace));
    assert!(!class::can(Class::Engineer, Ability::Throw));
    // A deploy is refused a soldier and a throw an engineer, and that is
    // the whole of what a class refuses.
    let tile = tile_of(world.aboard.room.bim_pos(1));
    assert_eq!(
        world.can_deploy(1, crate::deploy::Kit::Sandbag, tile),
        Err(Refusal::NotAnEngineer)
    );
    assert_eq!(world.can_throw(0, tile), Err(Refusal::NotASoldier));
    assert_eq!(world.can_brace(0), Err(Refusal::NotASoldier));
    assert_eq!(world.can_brace(2), Err(Refusal::NotASoldier));
}

#[test]
fn the_starting_pool_is_the_same_for_any_mix_of_classes_and_each_has_its_kit() {
    let mut world = basic();
    let money = world.money;
    for (a, b) in [
        (Class::Engineer, Class::None),
        (Class::Soldier, Class::None),
        (Class::Soldier, Class::Engineer),
        (Class::Engineer, Class::Soldier),
        (Class::Soldier, Class::Soldier),
        (Class::None, Class::None),
    ] {
        assert_eq!(world.set_class(0, a), Ok(()));
        assert_eq!(world.set_class(1, b), Ok(()));
        assert_eq!(world.money, money, "{a:?} and {b:?}: the same pool");
    }
    // The soldier's kit: a basic auto rifle in hand, the pistol in the
    // pack, two grenades; the engineer keeps its own sandbag kits; and
    // a class put back to none is the plain start again.
    assert_eq!(world.set_class(0, Class::Soldier), Ok(()));
    assert_eq!(
        world.aboard.room.weapon(0),
        Some(WeaponKind::AutoRifle.basic())
    );
    let pistol = Item::Weapon(WeaponKind::LaserPistol.basic());
    assert!(world.aboard.room.pack(0).contains(&Some(pistol)));
    assert_eq!(grenades(&world, 0), class::GRENADE_CHARGES);
    assert_eq!(world.set_class(1, Class::Engineer), Ok(()));
    let kit = Item::Stack(ResourceId::SandbagKit as u32);
    assert_eq!(
        world
            .aboard
            .room
            .pack(1)
            .iter()
            .filter(|i| **i == Some(kit))
            .count(),
        crate::deploy::SANDBAG_CHARGES as usize
    );
    assert_eq!(grenades(&world, 1), 0);
    assert_eq!(world.set_class(0, Class::None), Ok(()));
    assert_eq!(
        world.aboard.room.weapon(0),
        Some(WeaponKind::LaserPistol.basic())
    );
    assert!(!world.aboard.room.pack(0).contains(&Some(pistol)));
    assert!(
        !world
            .aboard
            .room
            .pack(0)
            .contains(&Some(Item::Weapon(WeaponKind::AutoRifle.basic())))
    );
    assert_eq!(grenades(&world, 0), 0);
    // And straight from one class to the other swaps the kits.
    assert_eq!(world.set_class(1, Class::Soldier), Ok(()));
    assert_eq!(
        world
            .aboard
            .room
            .pack(1)
            .iter()
            .filter(|i| **i == Some(kit))
            .count(),
        0
    );
    assert_eq!(grenades(&world, 1), 2);
    assert_eq!(
        world.aboard.room.weapon(1),
        Some(WeaponKind::AutoRifle.basic())
    );
}

// --- B: the brace ---------------------------------------------------------------

#[test]
fn a_braced_soldier_holds_its_ground_takes_no_errand_never_runs_and_shoots_steadier() {
    let mut world = fight();
    let mut events = world.step(&[Command::Brace { slot: 0, on: true }]);
    assert!(
        events
            .iter()
            .any(|e| matches!(e, WorldEvent::Braced { who: 0, on: true })),
        "{events:?}"
    );
    assert!(world.is_braced(0));
    assert!(world.aboard.room.is_braced(0));
    let here = world.aboard.room.bim_pos(0);
    // It stands there, on no errand, under arms, shooting at the braced
    // odds — the room's skill is the world's.
    for _ in 0..300 {
        events = world.step(&[]);
    }
    assert!(world.is_braced(0));
    assert!(
        (world.aboard.room.bim_pos(0) - here).len() < 1.0,
        "stood still"
    );
    assert!(
        world.aboard.room.task_kind_for_probe(0).is_none(),
        "no errand"
    );
    assert!(world.aboard.room.is_armed(0));
    let skill = world.skill_of(0);
    assert_eq!(skill.accuracy, class::BRACE_ACCURACY);
    assert_eq!(world.aboard.room.skill_for_probe(0), skill);
    assert!(
        events
            .iter()
            .all(|e| !matches!(e, WorldEvent::Braced { .. })),
        "said once"
    );
    // Dying with an enemy about, it does not run.
    world.aboard.room.wound(0, Part::Body, Part::Body.max());
    assert!(world.aboard.room.is_dying(0));
    for _ in 0..120 {
        world.aboard.room.patch_up_for_probe(0);
        world.aboard.room.wound(0, Part::Body, Part::Body.max());
        world.step(&[]);
        assert!(!world.aboard.room.is_fleeing(0), "braced: never flees");
    }
    assert!((world.aboard.room.bim_pos(0) - here).len() < 1.0);
    world.aboard.room.patch_up_for_probe(0);
    // Toggled off: standing easy again, the errands open to it.
    let events = world.step(&[Command::Brace { slot: 0, on: false }]);
    assert!(
        events
            .iter()
            .any(|e| matches!(e, WorldEvent::Braced { who: 0, on: false }))
    );
    assert!(!world.is_braced(0));
    assert_eq!(world.skill_of(0).accuracy, 1.0);
}

#[test]
fn a_brace_ends_on_an_order_to_move_and_on_going_down_and_is_refused_the_others() {
    let mut world = soldier();
    // Refused for anybody but a soldier, and for one not fit to act.
    let events = world.step(&[Command::Brace { slot: 1, on: true }]);
    assert!(refused_with(&events, Refusal::NotASoldier));
    assert_eq!(world.can_brace(1), Err(Refusal::NotASoldier));
    world.aboard.room.knock_out_for_probe(0);
    world.step(&[]);
    assert_eq!(world.can_brace(0), Err(Refusal::OutOfReach));
    let events = world.step(&[Command::Brace { slot: 0, on: true }]);
    assert!(refused_with(&events, Refusal::OutOfReach));
    world.aboard.room.patch_up_for_probe(0);
    world.step(&[]);
    assert_eq!(world.can_brace(0), Ok(()));
    // Braced, then ordered across the deck: the brace is over.
    world.step(&[Command::Brace { slot: 0, on: true }]);
    assert!(world.is_braced(0));
    let here = world.aboard.room.bim_pos(0);
    let there = here + vec2(3.0 * TILE, 0.0);
    world.step(&[Command::Crew {
        slot: 0,
        order: bims::order::CrewOrder::SendTo {
            who: 0,
            x: there.x,
            y: there.y,
        },
    }]);
    assert!(!world.is_braced(0), "an order to move ends it");
    // Braced, then down: over too.
    world.step(&[Command::Brace { slot: 0, on: true }]);
    assert!(world.is_braced(0));
    world.aboard.room.knock_out_for_probe(0);
    world.step(&[]);
    assert!(!world.is_braced(0), "going down ends it");
    // And it is in the checksum: a soldier braced is a different world.
    world.aboard.room.patch_up_for_probe(0);
    world.step(&[]);
    let before = world_checksum(&world);
    world.step(&[Command::Brace { slot: 0, on: true }]);
    assert_ne!(world_checksum(&world), before);
}

// --- C: grenades ----------------------------------------------------------------

#[test]
fn every_reason_a_throw_is_refused() {
    let mut world = soldier();
    let run = open_run(&world, 0, 2);
    let tile = run[1];
    // Level one: not yet. A classless crewmate holding grenades: never.
    assert_eq!(world.can_throw(0, tile), Err(Refusal::NoGrenadesYet));
    give_grenade(&mut world, 1);
    assert_eq!(world.can_throw(1, tile), Err(Refusal::NotASoldier));
    let events = throw(&mut world, 1, tile);
    assert!(refused_with(&events, Refusal::NotASoldier));
    assert_eq!(grenades(&world, 1), 1, "kept");
    level_up(&mut world, 0, class::GRENADE_LEVEL);
    assert_eq!(world.can_throw(0, tile), Ok(()));
    // Not fit to act.
    world.aboard.room.knock_out_for_probe(0);
    world.step(&[]);
    assert_eq!(world.can_throw(0, tile), Err(Refusal::OutOfReach));
    world.aboard.room.patch_up_for_probe(0);
    world.step(&[]);
    // No grenade in the pack.
    let grenade = Item::Stack(ResourceId::Grenade as u32);
    let cells: Vec<usize> = world
        .aboard
        .room
        .pack(0)
        .iter()
        .enumerate()
        .filter(|(_, i)| **i == Some(grenade))
        .map(|(c, _)| c)
        .collect();
    for cell in cells {
        world.aboard.room.take(0, cell);
    }
    assert_eq!(world.can_throw(0, tile), Err(Refusal::NoGrenade));
    let events = throw(&mut world, 0, tile);
    assert!(refused_with(&events, Refusal::NoGrenade));
    give_grenade(&mut world, 0);
    give_grenade(&mut world, 0);
    // Out of range, and no line: a tile past the range, and one the
    // other side of a wall.
    let from = world.aboard.room.bim_pos(0);
    let far = tile_of(from + vec2((class::GRENADE_RANGE + 1.0) * TILE, 0.0));
    let far = if world.aboard.room.is_deck_tile(middle(far)) {
        far
    } else {
        // Whichever deck tile beyond the range the room has.
        let here = tile_of(from);
        (-12..=12)
            .flat_map(|dx| (-12..=12).map(move |dy| (here.0 + dx, here.1 + dy)))
            .find(|&t| {
                world.aboard.room.is_deck_tile(middle(t))
                    && (middle(t) - from).len() > class::GRENADE_RANGE * TILE
            })
            .expect("a deck tile beyond the range")
    };
    assert_eq!(world.can_throw(0, far), Err(Refusal::OutOfThrowRange));
    let here = tile_of(from);
    let behind = (-8..=8)
        .flat_map(|dx| (-8..=8).map(move |dy| (here.0 + dx, here.1 + dy)))
        .find(|&t| {
            let at = middle(t);
            world.aboard.room.is_deck_tile(at)
                && (at - from).len() <= class::GRENADE_RANGE * TILE
                && !world.aboard.room.line_clear(from, at)
        })
        .expect("a deck tile in range behind a wall");
    assert_eq!(world.can_throw(0, behind), Err(Refusal::NoLineToTile));
    // Not deck at all.
    let void = (-40..=40)
        .flat_map(|dx| (-40..=40).map(move |dy| (here.0 + dx, here.1 + dy)))
        .find(|&t| !world.aboard.room.is_deck_tile(middle(t)))
        .expect("a tile that is not deck");
    assert_eq!(world.can_throw(0, void), Err(Refusal::CantThrowThere));
    // A throw, and then the cooldown.
    let events = throw(&mut world, 0, tile);
    assert!(
        events
            .iter()
            .any(|e| matches!(e, WorldEvent::Thrown { who: 0 })),
        "{events:?}"
    );
    assert_eq!(grenades(&world, 0), 1, "the grenade left the pack at once");
    // The charge is the whole of the gate (feature 90): the second one
    // goes straight after the first, and the wait is for the charges
    // coming back rather than between throws.
    assert_eq!(world.can_throw(0, tile), Ok(()));
    let left = world.grenade_cooldown_left(0);
    assert!(left > 0.0 && left <= class::GRENADE_COOLDOWN, "{left}");
    throw(&mut world, 0, tile);
    assert_eq!(grenades(&world, 0), 0, "both charges thrown");
    let events = throw(&mut world, 0, tile);
    assert!(refused_with(&events, Refusal::NoGrenade));
    assert_eq!(world.can_throw(0, tile), Err(Refusal::NoGrenade));
    // Thirty seconds of the clock on, one charge is back; thirty more
    // and it is at its two again.
    let steps = (class::GRENADE_COOLDOWN / SECONDS_A_STEP).ceil() as u32;
    for _ in 0..steps {
        world.step(&[]);
    }
    assert_eq!(grenades(&world, 0), 1, "one charge back");
    assert_eq!(world.can_throw(0, tile), Ok(()));
    for _ in 0..steps {
        world.step(&[]);
    }
    assert_eq!(grenades(&world, 0), class::GRENADE_CHARGES);
    assert_eq!(
        world.grenade_cooldown_left(0),
        0.0,
        "nothing running at its charges"
    );
}

#[test]
fn the_fuse_burns_its_seconds_and_the_throw_moves_the_checksum() {
    let mut world = soldier();
    level_up(&mut world, 0, class::GRENADE_LEVEL);
    let tile = open_run(&world, 0, 3)[2];
    let before = world_checksum(&world);
    throw(&mut world, 0, tile);
    assert_ne!(world_checksum(&world), before, "a throw is in the checksum");
    assert_eq!(world.aboard.room.grenades().len(), 1);
    let g = world.aboard.room.grenades()[0];
    assert_eq!(g.by, 0);
    assert_eq!(g.at, middle(tile));
    assert_eq!(g.fuse, class::GRENADE_FUSE);
    assert_eq!(g.radius, class::GRENADE_RADIUS * TILE);
    assert_eq!(g.damage, class::GRENADE_DAMAGE);
    let steps = run_until_burst(&mut world);
    let want = (class::GRENADE_FUSE / SECONDS_A_STEP as f32).round() as u32;
    assert!(
        (steps as i64 - want as i64).abs() <= 2,
        "burst after {steps} steps, the fuse is {want}"
    );
}

#[test]
fn the_burst_hurts_the_enemy_at_the_centre_and_less_at_the_edge() {
    // The centre: the machine on its tile takes the whole of it, on a
    // part rolled off the stream, with no armour to take any — a machine
    // wears none. The soldier's rifle out of its hand, so nothing else
    // lands.
    let mut world = fight();
    disarm(&mut world, 0);
    let at = machine_at(&world);
    let tile = tile_of(at);
    assert_eq!(world.can_throw(0, tile), Ok(()));
    throw(&mut world, 0, tile);
    let (took, now) = machine_burst(&mut world);
    let d = (now - middle(tile)).len() / TILE;
    let want = class::GRENADE_DAMAGE * (1.0 - 0.5 * d / class::GRENADE_RADIUS);
    assert!(
        plausible_on_machine(&world, took, want),
        "took {took} at {d:.2} tiles from the burst, {want} dealt"
    );
    assert!(took > 0.0);
    // The edge: a tile two tiles short of the machine deals less, in the
    // straight line to half. A limb is smaller than the burst, so a limb
    // rolled says nothing; the burst is thrown until the chassis is hit.
    for _ in 0..6 {
        let mut world = fight();
        disarm(&mut world, 0);
        let at = machine_at(&world);
        let from = world.aboard.room.bim_pos(0);
        let towards = (from - at).normalize_or_zero();
        let edge = tile_of(at + towards * (2.0 * TILE));
        assert_eq!(world.can_throw(0, edge), Ok(()));
        throw(&mut world, 0, edge);
        let (took, now) = machine_burst(&mut world);
        let d = (now - middle(edge)).len() / TILE;
        assert!(d > 0.5 && d < class::GRENADE_RADIUS, "{d}");
        let want = class::GRENADE_DAMAGE * (1.0 - 0.5 * d / class::GRENADE_RADIUS);
        assert!(
            plausible_on_machine(&world, took, want),
            "took {took} at {d:.2} tiles from the burst, {want} dealt"
        );
        let body = machine(&world).body;
        if took > body.max(DroidPart::Legs) + 1.0 {
            assert!(
                (took - want).abs() < 1.0,
                "took {took} at {d:.2} tiles, wanted {want}"
            );
            assert!(
                took < class::GRENADE_DAMAGE * 0.85,
                "less than the centre's"
            );
            return;
        }
    }
    panic!("six bursts, and every one landed on a limb");
}

/// Armour takes its share of a burst — the crew's own, since a machine
/// wears none — and nothing of the ship or the station moves for it.
#[test]
fn armour_takes_its_share_of_a_burst_and_the_parts_are_as_they_were() {
    let mut world = fight();
    disarm(&mut world, 0);
    // Kate in the basic armour, every part covered so whichever the
    // burst lands on takes its protection off first, stood beside the
    // machine and held there: the grenade at her feet.
    let mut gear = Gear::default();
    gear.basic_armour(1_000);
    world.aboard.room.issue(1, gear);
    let at = machine_at(&world);
    let kate = world
        .aboard
        .room
        .put_for_probe(1, at + vec2(0.0, 1.5 * TILE));
    world.aboard.room.recruit_for_probe(1, true);
    let tile = tile_of(kate);
    let hull = shipdesign::design_hash(&world.ship.design);
    let station = world.residents.as_ref().unwrap().station;
    let deck = shipdesign::design_hash(&world.station(station).unwrap().design);
    assert_eq!(world.can_throw(0, tile), Ok(()));
    throw(&mut world, 0, tile);
    let (mut took, mut armour_took, mut now) = (0.0, 0.0, kate);
    for _ in 1..2_000 {
        world.aboard.room.put_for_probe(1, kate);
        let health = world.aboard.room.health(1);
        let armour = world.aboard.room.armour_health(1);
        now = world.aboard.room.bim_pos(1);
        world.step(&[]);
        if world.aboard.room.grenades().is_empty() {
            took = health - world.aboard.room.health(1);
            armour_took = armour - world.aboard.room.armour_health(1);
            break;
        }
    }
    assert!(world.aboard.room.grenades().is_empty(), "it burst");
    let d = (now - middle(tile)).len() / TILE;
    let dealt = class::GRENADE_DAMAGE * (1.0 - 0.5 * d / class::GRENADE_RADIUS);
    assert!(armour_took > 0.0, "the armour took some");
    assert!(
        took < dealt,
        "and the body less than was dealt: {took} of {dealt}"
    );
    // The piece's protection comes off the top, then the piece drains,
    // then the body: the two together are short of what was dealt by
    // the protection.
    assert!(
        took + armour_took < dealt,
        "body {took} + armour {armour_took} against {dealt}: the protection is the rest"
    );
    // Nothing of the ship or the station moved for it.
    assert_eq!(shipdesign::design_hash(&world.ship.design), hull);
    assert_eq!(
        shipdesign::design_hash(&world.station(station).unwrap().design),
        deck
    );
}

#[test]
fn a_burst_hurts_the_thrower_a_crewmate_and_a_sentry_and_blows_the_sandbags_up() {
    let mut world = soldier();
    level_up(&mut world, 0, class::GRENADE_LEVEL);
    world.aboard.room.issue(1, Gear::default());
    let run = open_run(&world, 0, 3);
    // The crewmate on the second tile of the run, held there, a sentry
    // on the third, sandbags on the first, and the grenade at the
    // crewmate's feet: all of it within the radius.
    let mate = world.aboard.room.put_for_probe(1, middle(run[1]));
    world.aboard.room.recruit_for_probe(1, true);
    let bags = lay_bags(&mut world, middle(run[0]));
    let sentry = world.next_deployable;
    world.next_deployable += 1;
    world.deployables.push(Deployable {
        id: sentry,
        kind: DeployKind::Sentry,
        owner_slot: 1,
        deck: Deck::Ship,
        tile: design_tile(&world, middle(run[2])),
        health: crate::deploy::SENTRY_HEALTH,
    });
    world.step(&[]);
    assert_eq!(world.aboard.room.sentries().len(), 1);
    assert_eq!(world.aboard.room.laid_cover().len(), 1);
    let my_health = world.aboard.room.health(0);
    let mate_health = world.aboard.room.health(1);
    let hull = shipdesign::design_hash(&world.ship.design);
    let events = throw(&mut world, 0, run[1]);
    assert!(
        events
            .iter()
            .any(|e| matches!(e, WorldEvent::Thrown { who: 0 }))
    );
    let mut lost = false;
    for _ in 0..2_000 {
        let events = world.step(&[]);
        if events.iter().any(|e| {
            matches!(e, WorldEvent::DeployableLost { kind } if *kind == DeployKind::Sandbags.code())
        }) {
            lost = true;
        }
        if world.aboard.room.grenades().is_empty() {
            break;
        }
    }
    assert!(lost, "the sandbags were blown up");
    assert!(world.deployable(bags).is_none());
    assert!(world.aboard.room.laid_cover().is_empty());
    // The thrower, a tile off, and the crewmate at the centre — each
    // where it stood when the grenade went off.
    let burst = middle(run[1]);
    let dealt = |at: Vec2| {
        let d = (at - burst).len() / TILE;
        class::GRENADE_DAMAGE * (1.0 - 0.5 * d / class::GRENADE_RADIUS)
    };
    let me = world.aboard.room.bim_pos(0);
    let mine = my_health - world.aboard.room.health(0);
    assert!(mine > 0.0, "friendly fire, the thrower included");
    assert!(
        plausible(mine, dealt(me)),
        "the thrower took {mine}, {} was dealt",
        dealt(me)
    );
    let theirs = mate_health - world.aboard.room.health(1);
    assert!(
        plausible(theirs, dealt(mate)),
        "the crewmate took {theirs}, {} was dealt",
        dealt(mate)
    );
    // The sentry, a tile off the other way, on its health.
    let s = world.deployable(sentry).unwrap();
    assert!(
        (crate::deploy::SENTRY_HEALTH - s.health - dealt(middle(run[2]))).abs() < 1.0,
        "the sentry took {}",
        crate::deploy::SENTRY_HEALTH - s.health
    );
    // And the blood: the crew's deck round the crewmate.
    assert!(world.aboard.room.bloody_tiles() > 0);
    assert_eq!(
        shipdesign::design_hash(&world.ship.design),
        hull,
        "the parts are untouched"
    );
}

#[test]
fn cover_halves_a_burst_sandbags_do_not_stop_it_and_a_wall_does() {
    // In cover: the crewmate close behind laid sandbags on the burst's
    // side takes half. Sandbags at the run's second tile, the crewmate on
    // the first, held there, the burst on the third — two tiles from the
    // body. The head is five, so a head rolled says nothing; the burst
    // is thrown until a body or a leg is hit.
    for _ in 0..6 {
        let mut world = soldier();
        level_up(&mut world, 0, class::GRENADE_LEVEL);
        world.aboard.room.issue(1, Gear::default());
        let run = open_run(&world, 0, 3);
        let mate = world.aboard.room.put_for_probe(1, middle(run[0]));
        world.aboard.room.recruit_for_probe(1, true);
        lay_bags(&mut world, middle(run[1]));
        world.step(&[]);
        assert!(
            world.aboard.room.covered_for_probe(mate, middle(run[2])),
            "in cover from the burst's side"
        );
        let before = world.aboard.room.health(1);
        throw(&mut world, 0, run[2]);
        run_until_burst(&mut world);
        let d = (mate - middle(run[2])).len() / TILE;
        let full = class::GRENADE_DAMAGE * (1.0 - 0.5 * d / class::GRENADE_RADIUS);
        let took = before - world.aboard.room.health(1);
        assert!(took > 0.0, "sandbags do not stop a burst");
        assert!(
            plausible(took, full * 0.5),
            "in cover it took {took}, half of {full}"
        );
        if took > Part::Head.max() + 1.0 {
            assert!(
                (took - full * 0.5).abs() < 1.0,
                "in cover it took {took}, half of {full}"
            );
            break;
        }
    }
    // A wall does: a deck tile in the radius of the crewmate with no line
    // to it, and the crewmate is untouched.
    let mut world = soldier();
    level_up(&mut world, 0, class::GRENADE_LEVEL);
    world.aboard.room.issue(1, Gear::default());
    let from = world.aboard.room.bim_pos(0);
    let here = tile_of(from);
    let (burst, mate) = (-8..=8)
        .flat_map(|dx| (-8..=8).map(move |dy| (here.0 + dx, here.1 + dy)))
        .filter(|&t| world.can_throw(0, t).is_ok())
        .find_map(|t| {
            let b = middle(t);
            (-2..=2)
                .flat_map(|dx| (-2..=2).map(move |dy| (t.0 + dx, t.1 + dy)))
                .map(middle)
                .find(|&m| {
                    world.aboard.room.is_deck_tile(m)
                        && (m - b).len() <= class::GRENADE_RADIUS * TILE
                        && !world.aboard.room.line_clear(b, m)
                })
                .map(|m| (t, m))
        })
        .expect("a burst tile with a deck tile in its radius behind a wall");
    let mate = world.aboard.room.put_for_probe(1, mate);
    world.aboard.room.recruit_for_probe(1, true);
    assert!(!world.aboard.room.line_clear(middle(burst), mate));
    let before = world.aboard.room.health(1);
    throw(&mut world, 0, burst);
    run_until_burst(&mut world);
    assert_eq!(world.aboard.room.health(1), before, "a wall stops it");
}

#[test]
fn a_shut_door_stops_a_burst() {
    // The ship's doors shut when nobody is near them; a burst on one side
    // does not reach a body on the other. Frag, so the radius spans the
    // door with both bodies clear of its reach: the soldier three tiles
    // off on one side throwing at the tile a tile short of the door, the
    // crewmate two and a half tiles off on the other.
    let mut world = soldier();
    level_up(&mut world, 0, class::GRENADE_LEVEL);
    pick(&mut world, 0, Talent::Frag);
    world.aboard.room.issue(1, Gear::default());
    let doors = world.aboard.room.ship_doors_for_probe();
    assert!(!doors.is_empty());
    let radius = world.grenade_radius(0) * TILE;
    let mut staged = None;
    'doors: for (centre, through) in doors {
        for side in [1.0f32, -1.0] {
            let me = centre + through * side * (3.0 * TILE);
            let burst = tile_of(centre + through * side * (1.0 * TILE));
            let mate = centre - through * side * (2.5 * TILE);
            let room = &world.aboard.room;
            if room.is_deck_tile(me)
                && room.is_deck_tile(middle(burst))
                && room.is_deck_tile(mate)
                && (mate - middle(burst)).len() <= radius
                && room.line_clear(me, middle(burst))
            {
                staged = Some((me, burst, mate));
                break 'doors;
            }
        }
    }
    let Some((me, burst, mate)) = staged else {
        // No door of this layout has three tiles of deck one side and
        // two and a half the other: nothing to test here.
        return;
    };
    let mate = world.aboard.room.put_for_probe(1, mate);
    world.aboard.room.recruit_for_probe(1, true);
    world.aboard.room.put_for_probe(0, me);
    world.aboard.room.recruit_for_probe(0, true);
    for _ in 0..300 {
        world.step(&[]);
    }
    if world.aboard.room.line_clear(middle(burst), mate) {
        // The door did not shut — a doorway the room keeps open, or a
        // body within its reach after all. Nothing to test on this layout.
        return;
    }
    let before = world.aboard.room.health(1);
    assert_eq!(world.can_throw(0, burst), Ok(()));
    throw(&mut world, 0, burst);
    run_until_burst(&mut world);
    assert_eq!(world.aboard.room.health(1), before, "a shut door stops it");
}

#[test]
fn two_runs_of_a_grenade_fight_on_one_seed_are_the_same_fight_and_the_book_is_the_labour_rule() {
    let run = || {
        let mut world = fight();
        let at = machine_at(&world);
        throw(&mut world, 0, tile_of(at));
        for _ in 0..600 {
            world.step(&[]);
        }
        world_checksum(&world)
    };
    assert_eq!(run(), run());
    // A grenade is a charge on a cooldown since feature 90 and is made
    // nowhere since feature 95; a desk still buys one off a pack, and
    // its book value is a hand-written number like every other.
    assert_eq!(trade_price(ResourceId::Grenade), 80);
}

// --- D: the ten levels ----------------------------------------------------------

#[test]
fn the_fixed_levels_are_the_brace_the_grenades_and_the_drill() {
    let mut world = soldier();
    assert_eq!(world.can_brace(0), Ok(()), "level one");
    let tile = open_run(&world, 0, 2)[1];
    assert_eq!(world.can_throw(0, tile), Err(Refusal::NoGrenadesYet));
    level_up(&mut world, 0, 3);
    assert_eq!(world.can_throw(0, tile), Ok(()), "level three");
    assert_eq!(world.skill_of(0).fire_rate, 1.0);
    level_up(&mut world, 0, 6);
    assert_eq!(world.skill_of(0).fire_rate, 1.0);
    level_up(&mut world, 0, 7);
    assert_eq!(
        world.skill_of(0).fire_rate,
        class::DRILL_FIRE_RATE,
        "level seven"
    );
    let stats = world.skill_of(0).stats(WeaponKind::AutoRifle.basic());
    assert_eq!(
        stats.fire_rate,
        WeaponKind::AutoRifle.basic().stats().fire_rate * class::DRILL_FIRE_RATE
    );
    // The crewmate has none of it.
    assert_eq!(world.skill_of(1), bims::combat::Skill::NONE);
}

#[test]
fn marksman_and_point_blank() {
    let mut world = soldier();
    pick(&mut world, 0, Talent::Marksman);
    let skill = world.skill_of(0);
    assert_eq!(skill.accuracy, class::MARKSMAN_ACCURACY);
    let base = WeaponKind::LaserPistol.basic().stats();
    let stats = skill.stats(WeaponKind::LaserPistol.basic());
    assert_eq!(stats.accuracy, base.accuracy * class::MARKSMAN_ACCURACY);
    assert_eq!(
        stats.accuracy_far,
        base.accuracy_far * class::MARKSMAN_ACCURACY
    );
    assert_eq!(skill.point_blank, 1.0);
    let mut world = soldier();
    pick(&mut world, 0, Talent::PointBlank);
    assert_eq!(world.skill_of(0).point_blank, class::POINT_BLANK_DAMAGE);
    assert_eq!(world.skill_of(0).accuracy, 1.0);
    assert_eq!(world.skill_of(1), bims::combat::Skill::NONE);
    // The same pick on an engineer is the engineer's talent, not this.
    let mut world = basic();
    assert_eq!(world.set_class(0, Class::Engineer), Ok(()));
    level_up(&mut world, 0, 2);
    world.step(&[Command::PickTalent {
        slot: 0,
        level: 2,
        side: Side::Left,
    }]);
    assert!(world.has_talent(0, Talent::ReinforcedSand));
    assert!(!world.has_talent(0, Talent::Marksman));
    assert_eq!(world.skill_of(0), bims::combat::Skill::NONE);
}

#[test]
fn runner_and_steady_aim() {
    let mut world = soldier();
    pick(&mut world, 0, Talent::Runner);
    assert_eq!(world.skill_of(0).pace, class::RUNNER_PACE);
    assert_eq!(world.skill_of(0).walking, bims::combat::WALKING_ACCURACY);
    world.step(&[]);
    assert_eq!(
        world.aboard.room.skill_for_probe(0).pace,
        class::RUNNER_PACE
    );
    assert_eq!(world.aboard.room.skill_for_probe(1).pace, 1.0);
    let mut world = soldier();
    pick(&mut world, 0, Talent::SteadyAim);
    assert_eq!(world.skill_of(0).walking, class::steady_aim_walking());
    assert_eq!(world.skill_of(0).walking, 0.75);
    assert_eq!(world.skill_of(0).pace, 1.0);
}

#[test]
fn iron_nerve_never_flees_and_cover_master_dodges_more() {
    let mut world = fight();
    pick(&mut world, 0, Talent::IronNerve);
    assert!(world.skill_of(0).nerve);
    world.step(&[]);
    let here = world.aboard.room.bim_pos(0);
    for _ in 0..120 {
        world.aboard.room.patch_up_for_probe(0);
        world.aboard.room.wound(0, Part::Body, Part::Body.max());
        world.step(&[]);
        assert!(!world.aboard.room.is_fleeing(0), "iron nerve: never flees");
    }
    assert!((world.aboard.room.bim_pos(0) - here).len() < TILE);
    // Without it, the same soldier runs.
    let mut world = fight();
    world.step(&[]);
    world.aboard.room.wound(0, Part::Body, Part::Body.max());
    world.step(&[]);
    assert!(world.aboard.room.is_fleeing(0));
    let mut world = soldier();
    pick(&mut world, 0, Talent::CoverMaster);
    assert_eq!(
        world.skill_of(0).cover_dodge,
        bims::combat::DODGE_IN_COVER * class::COVER_MASTER_DODGE
    );
    assert!(!world.skill_of(0).nerve);
    assert_eq!(world.skill_of(1).cover_dodge, bims::combat::DODGE_IN_COVER);
}

#[test]
fn long_throw_short_fuse_frag_and_quick_draw_are_the_grenade_s_numbers() {
    let mut world = soldier();
    assert_eq!(world.grenade_range(0), class::GRENADE_RANGE);
    assert_eq!(world.grenade_fuse(0), class::GRENADE_FUSE);
    assert_eq!(world.grenade_radius(0), class::GRENADE_RADIUS);
    assert_eq!(world.grenade_cooldown(0), class::GRENADE_COOLDOWN);
    pick(&mut world, 0, Talent::LongThrow);
    assert_eq!(
        world.grenade_range(0),
        class::GRENADE_RANGE * class::LONG_THROW_RANGE
    );
    assert_eq!(world.grenade_fuse(0), class::GRENADE_FUSE);
    pick(&mut world, 0, Talent::Frag);
    assert_eq!(
        world.grenade_radius(0),
        class::GRENADE_RADIUS * class::FRAG_RADIUS
    );
    assert_eq!(world.grenade_cooldown(0), class::GRENADE_COOLDOWN);
    let mut world = soldier();
    pick(&mut world, 0, Talent::ShortFuse);
    assert_eq!(
        world.grenade_fuse(0),
        class::GRENADE_FUSE * class::SHORT_FUSE_TIME
    );
    assert_eq!(world.grenade_range(0), class::GRENADE_RANGE);
    pick(&mut world, 0, Talent::QuickDraw);
    assert_eq!(
        world.grenade_cooldown(0),
        class::GRENADE_COOLDOWN * class::QUICK_DRAW_COOLDOWN
    );
    assert_eq!(world.grenade_radius(0), class::GRENADE_RADIUS);
    // And the throw carries them: a short fuse bursts in half the steps,
    // and a charge thrown comes back in half the time.
    let tile = open_run(&world, 0, 3)[2];
    throw(&mut world, 0, tile);
    let g = world.aboard.room.grenades()[0];
    assert_eq!(g.fuse, class::GRENADE_FUSE * class::SHORT_FUSE_TIME);
    let steps = run_until_burst(&mut world);
    let want = (g.fuse / SECONDS_A_STEP as f32).round() as u32;
    assert!((steps as i64 - want as i64).abs() <= 2, "{steps} vs {want}");
    throw(&mut world, 0, tile);
    assert_eq!(grenades(&world, 0), 0, "both charges thrown");
    let half =
        (class::GRENADE_COOLDOWN * class::QUICK_DRAW_COOLDOWN / SECONDS_A_STEP).ceil() as u32;
    for _ in 0..=half {
        world.step(&[]);
    }
    assert!(grenades(&world, 0) >= 1, "a charge back in half the time");
    assert_eq!(world.can_throw(0, tile), Ok(()));
    // Nothing of it for the crewmate.
    assert_eq!(world.grenade_range(1), class::GRENADE_RANGE);
    assert_eq!(world.grenade_fuse(1), class::GRENADE_FUSE);
}

/// *Bruiser*, *dug in* and *deadeye* are the soldier's numbers. The last
/// pick's other side, *rampage*, is the test after this one.
#[test]
fn bruiser_dug_in_and_deadeye() {
    let mut world = soldier();
    pick(&mut world, 0, Talent::Bruiser);
    assert_eq!(world.skill_of(0).melee, class::BRUISER_MELEE);
    assert_eq!(world.skill_of(0).dodge, 0.0);
    let mut world = soldier();
    pick(&mut world, 0, Talent::DugInBraced);
    assert_eq!(world.skill_of(0).dodge, 0.0, "not braced: nothing");
    world.step(&[Command::Brace { slot: 0, on: true }]);
    assert_eq!(world.skill_of(0).dodge, class::DUG_IN_DODGE);
    assert_eq!(world.skill_of(0).melee, 1.0);
    world.step(&[Command::Brace { slot: 0, on: false }]);
    assert_eq!(world.skill_of(0).dodge, 0.0);
    let mut world = soldier();
    pick(&mut world, 0, Talent::Deadeye);
    let skill = world.skill_of(0);
    assert!(skill.deadeye);
    for kind in WeaponKind::ALL {
        let stats = skill.stats(kind.basic());
        assert_eq!(stats.accuracy_far, stats.accuracy, "{kind:?}");
        assert_eq!(stats.accuracy, kind.basic().stats().accuracy);
    }
    assert!(!world.skill_of(1).deadeye);
}

/// A second machine stood on the station's deck `tiles` tiles off the
/// staged one, held where it is put the same way: far enough that a
/// burst on the first never reaches it.
fn machine_beyond(world: &mut World, tiles: f32) {
    let residents = world.residents.as_mut().expect("alongside");
    let room = &mut residents.aboard.room;
    let first = room.droid(0).expect("the staged machine");
    let (kind, tier, wave, from, heading) =
        (first.kind, first.tier, first.wave, first.pos, first.heading);
    let at = [
        vec2(1.0, 0.0),
        vec2(-1.0, 0.0),
        vec2(0.0, 1.0),
        vec2(0.0, -1.0),
    ]
    .into_iter()
    .map(|d| from + d * (tiles * TILE))
    .find(|&p| room.is_deck_tile(p))
    .expect("deck that far off the machine");
    let mut other = bims::droid::Droid::new(kind, tier, 1, wave, at, heading, 0x5EC0_4D);
    other.posing = true;
    room.adopt_droids(vec![other], Vec2::ZERO);
    residents.aboard.crew = residents.aboard.room.body_count();
    let stood = residents
        .aboard
        .room
        .droid(1)
        .expect("the second machine")
        .pos;
    assert!(
        (stood - from).len() > (class::GRENADE_RADIUS + 1.0) * TILE,
        "out of the burst's reach"
    );
}

/// A machine destroyed where it stands, by its body index.
fn wreck(world: &mut World, who: usize) {
    world
        .residents
        .as_mut()
        .unwrap()
        .aboard
        .room
        .strike_droid(who, DroidPart::Chassis, 1e6);
}

/// *Rampage*: a stack for each machine the soldier downs while another
/// still stands, up to three, each one the fire rate ×1.1 — and every
/// stack gone once none stands, the fight over. The soldier's grenade is
/// the last thing to land on the first machine, and a second stands out
/// of the burst's reach so the fight goes on past it. (Until the fight
/// counted the machines as enemies standing, the stack was cleared the
/// step it was earned.)
#[test]
fn rampage_is_a_stack_a_machine_downed_until_none_stands() {
    let mut world = fight();
    disarm(&mut world, 0);
    disarm(&mut world, 1);
    machine_beyond(&mut world, 6.0);
    pick(&mut world, 0, Talent::Rampage);
    // The tenth level has the seventh's drill under it.
    let drill = class::DRILL_FIRE_RATE;
    assert_eq!(world.skill_of(0).fire_rate, drill);
    assert_eq!(world.aboard.room.rampage(0), 0);
    let tile = tile_of(machine_at(&world));
    assert_eq!(world.can_throw(0, tile), Ok(()));
    throw(&mut world, 0, tile);
    let (took, _) = machine_burst(&mut world);
    assert!(took > 0.0, "the burst landed");
    // Whatever the burst left of it destroyed: the grenade was the last
    // thing to land on it, so it is the soldier's.
    wreck(&mut world, 0);
    for _ in 0..3 {
        if world.aboard.room.rampage(0) > 0 {
            break;
        }
        world.step(&[]);
    }
    assert!(machine(&world).destroyed);
    assert_eq!(
        world.aboard.room.rampage(0),
        1,
        "a stack while the second machine stands"
    );
    assert_eq!(
        world.skill_of(0).fire_rate,
        drill * class::RAMPAGE_FIRE_RATE
    );
    for _ in 0..3 {
        world.step(&[]);
    }
    assert_eq!(
        world.aboard.room.rampage(0),
        1,
        "and it holds while one does"
    );
    // Capped at three, and the checksum knows the stacks.
    let before = world_checksum(&world);
    world.aboard.room.set_rampage(0, 3);
    assert_ne!(world_checksum(&world), before);
    assert_eq!(
        world.skill_of(0).fire_rate,
        drill * class::RAMPAGE_FIRE_RATE.powi(class::RAMPAGE_STACKS as i32)
    );
    // The last machine down: the fight is over, and the stacks with it.
    wreck(&mut world, 1);
    for _ in 0..3 {
        world.step(&[]);
    }
    assert_eq!(world.aboard.room.rampage(0), 0);
    assert_eq!(world.skill_of(0).fire_rate, drill);
    // Nothing for the crewmate, ever.
    assert_eq!(world.aboard.room.rampage(1), 0);
}
