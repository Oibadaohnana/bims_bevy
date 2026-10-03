//! The soldier class (feature 75, its ranked kit task 124): `crate::class`'s
//! second class. A class owns abilities and nothing else; the sixteen
//! levels and a skill point a level; the four abilities a rank at a time
//! — Frag Grenade, Weak Spot, Stun Shot and Rampage — each doing what its rank
//! says, to the soldier who holds it alone.

use bims::combat::{Gear, Item, WeaponKind};
use bims::droid::{DroidKind, DroidPart};
use bims::math::{Vec2, vec2};
use shipdesign::Rotation;
use shipdesign::fixture::flyer;
use shipdesign::parts::PartKind;

use crate::class::{self, Ability, Charge, Class};
use crate::deploy::{Deck, DeployKind, Deployable};
use crate::event::{Refusal, WorldEvent};
use crate::fixture::{REFERENCE_MONEY, simulation_world};
use crate::world::{Command, World};
use crate::world_checksum;

const TILE: f32 = shipdesign::TILE as f32;

/// Frag Grenade at the second rank, which the grenade tests throw at:
/// its burst, its radius in tiles and its cooldown a charge.
const DAMAGE: f32 = class::GRENADE_DAMAGE[1];
const RADIUS: f32 = class::GRENADE_RADIUS[1];
const COOLDOWN: f64 = class::GRENADE_COOLDOWN[1];
/// A sentry's health as laid at the engineer's first rank.
const SENTRY_HEALTH: f32 = class::SENTRY_HEALTH[0];

/// Seconds of the room's clock a world step is: a game minute is a real
/// second at 1×, and a step is a sixtieth of one.
const SECONDS_A_STEP: f64 = crate::data::STEP_MINUTES / time::MINUTES_PER_SECOND;

/// Whether `took` off a body is what `dealt` comes to on its one bar:
/// the whole of it, or the whole bar when that is less (task 120).
fn plausible(took: f32, dealt: f32) -> bool {
    (took - dealt.min(bims::health::MAX_HEALTH)).abs() < 1.0
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

/// Straight to a level, off the class's own table.
fn level_up(world: &mut World, who: usize, level: u8) {
    let mut events = Vec::new();
    let want = class::LEVEL_XP[level as usize - 1];
    let have = world.progress_of(who as u32).xp;
    world.award(who, want.saturating_sub(have), &mut events);
    assert_eq!(world.level_of(who as u32), level);
}

/// Ranks bought one at a time through `Command::RankUp`, Q C E R, the
/// level raised first to what the highest wants.
fn ranks(world: &mut World, who: u32, want: [u8; 4]) {
    let need = (0..4u8)
        .filter_map(|slot| class::rank_level(Class::Soldier, slot, want[slot as usize]))
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

fn grenades(world: &World, who: u32) -> u32 {
    world.grenades_of(who)
}

fn give_grenade(world: &mut World, who: usize) {
    let held = world.grenades_of(who as u32);
    world.set_charges_held(who as u32, Charge::Grenade, held + 1);
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
    ranks(&mut world, 0, [2, 0, 1, 0]);
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
        // Whole before every step, so the burst meets a body nothing else
        // has taken from first — a crewmate's bolt would leave the
        // chassis short of the doubled burst (feature 110), and the burst
        // would read as a wreck rather than as its damage.
        let droid = world
            .residents
            .as_mut()
            .unwrap()
            .aboard
            .room
            .droid_mut_for_probe(0)
            .expect("the staged machine");
        droid.body = bims::droid::DroidBody::new(droid.kind, droid.tier);
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

/// Low cover laid on the room tile a point is the middle of: the room's
/// own laid cover, which an engineer's sandbags were until task 154 took
/// them away and nothing of the world's lays now.
fn lay_bags(world: &mut World, at: Vec2) {
    let mut laid = world.aboard.room.laid_cover().to_vec();
    laid.push(bims::math::Rect::from_center_size(at, vec2(TILE, TILE)));
    world.aboard.room.set_laid_cover(&laid);
}

// --- A: a class owns abilities, never jobs or money --------------------------

#[test]
fn every_class_equips_every_weapon_takes_every_errand_and_places_every_site() {
    // Three crew, one of each class; a fresh world for each, since an
    // errand put down stays on the queue. Three players, so each changes
    // its own.
    let classed = || {
        let mut world = simulation_world(flyer(3), REFERENCE_MONEY, 3);
        assert_eq!(world.set_class(0, Class::Engineer), Ok(()));
        assert_eq!(world.set_class(1, Class::Soldier), Ok(()));
        assert_eq!(world.class_of(2), Class::None);
        world
    };
    for who in 0..3u32 {
        let mut world = classed();
        // Every weapon into the hand, whatever the class — out of the
        // armory between missions (task 113).
        world.leave_for_probe();
        for kind in WeaponKind::ALL {
            let item = Item::Weapon(kind.basic());
            let id = world.holdings.put(item).unwrap();
            let events = world.step(&[Command::Equip {
                slot: who,
                who,
                from: crate::GearSource::Armory { id },
            }]);
            assert!(
                !events
                    .iter()
                    .any(|e| matches!(e, WorldEvent::Refused { .. })),
                "{who} equips {kind:?}: {events:?}"
            );
            assert_eq!(world.aboard.room.weapon(who as usize), Some(kind.basic()));
        }
        world.restart_mission_for_probe();
        // Every errand: a crewmate downed revived is an order the room
        // takes for anybody. The order's answer is on the deck, never a
        // refusal; what says it was taken is the errand on hand. Stood on
        // a cell a body fits in first: where the crew wake up is against
        // the furniture.
        let at = world.aboard.room.bim_pos(who as usize);
        world.aboard.room.put_for_probe(who as usize, at);
        let patient = (who + 1) % 3;
        world
            .aboard
            .room
            .put_for_probe(patient as usize, at + bims::math::vec2(TILE, 0.0));
        world.aboard.room.knock_out_for_probe(patient as usize);
        world.step(&[]);
        let order = bims::order::CrewOrder::Revive { who, patient };
        world.step(&[Command::Crew { slot: who, order }]);
        assert_eq!(
            world.aboard.room.task_kind_for_probe(who as usize),
            Some(bims::game::JOB_REVIVE),
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
    assert!(class::can(Class::Soldier, Ability::StunShot));
    assert!(class::can(Class::Soldier, Ability::Throw));
    assert!(!class::can(Class::Engineer, Ability::StunShot));
    assert!(!class::can(Class::Engineer, Ability::Throw));
    // A deploy is refused a soldier and a throw an engineer, and that is
    // the whole of what a class refuses.
    let tile = tile_of(world.aboard.room.bim_pos(1));
    assert_eq!(
        world.can_deploy(1, DeployKind::Mine, tile),
        Err(Refusal::NotAnEngineer)
    );
    assert_eq!(world.can_throw(0, tile), Err(Refusal::NotASoldier));
    assert_eq!(world.can_stun_shot(0), Err(Refusal::NotASoldier));
    assert_eq!(world.can_stun_shot(2), Err(Refusal::NotASoldier));
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
    // The soldier's kit: a basic auto rifle in hand, the pistol into
    // the armory (task 113), and grenades by its rank; the engineer keeps its own mines; and
    // a class put back to none is the plain start again.
    assert_eq!(world.set_class(0, Class::Soldier), Ok(()));
    assert_eq!(
        world.aboard.room.weapon(0),
        Some(WeaponKind::AutoRifle.basic())
    );
    let pistol = Item::Weapon(WeaponKind::LaserPistol.basic());
    let in_armory = |world: &World, item: Item| {
        world
            .holdings
            .armory
            .iter()
            .filter(|s| s.item == item)
            .count()
    };
    assert_eq!(in_armory(&world, pistol), 1);
    // No grenade before a rank of Frag Grenade (task 124): one at the first.
    assert_eq!(grenades(&world, 0), 0);
    ranks(&mut world, 0, [1, 0, 0, 0]);
    assert_eq!(grenades(&world, 0), 1, "put in hand with the rank");
    assert_eq!(world.set_class(1, Class::Engineer), Ok(()));
    assert_eq!(
        world.charges_of(1, Charge::Mine),
        world.charges(1, Charge::Mine)
    );
    assert_eq!(grenades(&world, 1), 0);
    assert_eq!(world.set_class(0, Class::None), Ok(()));
    assert_eq!(
        world.aboard.room.weapon(0),
        Some(WeaponKind::LaserPistol.basic())
    );
    assert_eq!(in_armory(&world, pistol), 0, "taken back out");
    assert_eq!(
        in_armory(&world, Item::Weapon(WeaponKind::AutoRifle.basic())),
        0
    );
    assert_eq!(grenades(&world, 0), 0);
    // And straight from one class to the other swaps the kits.
    assert_eq!(world.set_class(1, Class::Soldier), Ok(()));
    assert_eq!(world.charges_of(1, Charge::Mine), 0);
    assert_eq!(grenades(&world, 1), 0, "not before a rank");
    assert_eq!(
        world.aboard.room.weapon(1),
        Some(WeaponKind::AutoRifle.basic())
    );
}

// --- B: the Stun Shot (October 2026; the brace until then) ---------------------

/// Step until the Stun Shot fired by slot 0 has burst, the machine made
/// whole before every step so nothing that landed before counts: what
/// it lost to the burst, and the room's stun on it after.
fn machine_shot(world: &mut World) -> (f32, f32) {
    let mut fired = false;
    for _ in 1..2_000 {
        let droid = world
            .residents
            .as_mut()
            .unwrap()
            .aboard
            .room
            .droid_mut_for_probe(0)
            .expect("the staged machine");
        droid.body = bims::droid::DroidBody::new(droid.kind, droid.tier);
        let health = machine_health(world);
        let events = world.step(&[]);
        if events
            .iter()
            .any(|e| matches!(e, WorldEvent::StunShotFired { who: 0 }))
        {
            fired = true;
            // Nothing but the shot lands on it from here: the gun goes.
            disarm(world, 0);
        }
        if fired && world.aboard.room.grenades().is_empty() {
            return (health - machine_health(world), machine(world).stunned);
        }
    }
    panic!("the shot never burst");
}

#[test]
fn a_stun_shot_charges_two_seconds_then_hurts_and_stuns_what_it_hits() {
    let mut world = fight();
    disarm(&mut world, 1);
    let tile = tile_of(machine_at(&world));
    let events = world.step(&[Command::StunShot {
        slot: 0,
        x: tile.0,
        y: tile.1,
    }]);
    assert!(
        events
            .iter()
            .any(|e| matches!(e, WorldEvent::ShotCharging { who: 0 })),
        "{events:?}"
    );
    assert!(world.is_charging(0));
    assert!(world.aboard.room.is_braced(0), "holding its fire");
    assert_eq!(world.can_stun_shot(0), Err(Refusal::AlreadyActive));
    // Charging: it stands where it is and nothing has gone yet.
    let here = world.aboard.room.bim_pos(0);
    for _ in 0..((1.8 / SECONDS_A_STEP) as u32) {
        let events = world.step(&[]);
        assert!(
            events
                .iter()
                .all(|e| !matches!(e, WorldEvent::StunShotFired { .. })),
            "not before its two seconds"
        );
    }
    assert!(world.is_charging(0));
    assert!(world.charge_share(0) > 0.85 && world.charge_share(0) < 1.0);
    assert!(
        (world.aboard.room.bim_pos(0) - here).len() < 1.0,
        "stood still"
    );
    // Fired: fifteen on the machine at the first rank, and a stun of
    // three seconds, less the flight's few steps of it worn off.
    let (took, stunned) = machine_shot(&mut world);
    assert_eq!(class::STUN_SHOT_DAMAGE[0], 15.0);
    assert!(
        plausible_on_machine(&world, took, class::STUN_SHOT_DAMAGE[0]),
        "took {took}"
    );
    assert!(
        stunned > 2.8 && stunned <= class::STUN_SHOT_STUN[0],
        "{stunned}"
    );
    assert!(machine(&world).is_stunned());
    // Over: its fire no longer held, and cooling down from the shot.
    assert!(!world.is_charging(0));
    assert!(!world.aboard.room.is_braced(0));
    assert_eq!(world.can_stun_shot(0), Err(Refusal::CoolingDown));
    let left = world.stun_shot_cooldown_left(0);
    assert!(
        left > 29.0 && left <= class::STUN_SHOT_COOLDOWN[0],
        "{left}"
    );
}

#[test]
fn a_stun_shot_never_reaches_past_the_weapon_and_never_hurts_the_crew() {
    let mut world = fight();
    disarm(&mut world, 1);
    let from = world.aboard.room.bim_pos(0);
    let range = world.stun_shot_range(0).expect("a rifle in hand");
    // Aimed sixty tiles off, whichever way: it bursts within the reach.
    let far = tile_of(from + vec2(60.0 * TILE, 0.0));
    world.step(&[Command::StunShot {
        slot: 0,
        x: far.0,
        y: far.1,
    }]);
    for _ in 0..2_000 {
        world.step(&[]);
        if let Some(g) = world.aboard.room.grenades().first() {
            assert!(g.shot);
            assert!(
                (g.at - from).len() <= range * TILE + 1.0,
                "{} past {}",
                (g.at - from).len() / TILE,
                range
            );
            break;
        }
    }
    // And at its own feet: the crew are untouched by the burst.
    let mut world = fight();
    disarm(&mut world, 1);
    let own = tile_of(world.aboard.room.bim_pos(0));
    world.step(&[Command::StunShot {
        slot: 0,
        x: own.0,
        y: own.1,
    }]);
    let crew: Vec<f32> = (0..2).map(|w| world.aboard.room.health(w)).collect();
    let mut burst = false;
    for _ in 0..((class::STUN_SHOT_CHARGE + 1.0) / SECONDS_A_STEP) as u32 {
        world.step(&[]);
        burst |= world.aboard.room.grenades().iter().any(|g| g.shot);
    }
    assert!(burst, "fired");
    for (w, &before) in crew.iter().enumerate() {
        assert!(world.aboard.room.health(w) >= before - 1e-3, "crew {w}");
    }
}

#[test]
fn a_stun_shot_is_refused_goes_on_as_he_walks_and_is_called_off_by_a_down() {
    let mut world = soldier();
    let own = tile_of(world.aboard.room.bim_pos(0));
    let shot = |slot| Command::StunShot {
        slot,
        x: own.0,
        y: own.1,
    };
    // Not before a rank of it (task 124).
    assert_eq!(world.can_stun_shot(0), Err(Refusal::NotLearnt));
    let events = world.step(&[shot(0)]);
    assert!(refused_with(&events, Refusal::NotLearnt));
    ranks(&mut world, 0, [0, 0, 1, 0]);
    // Refused for anybody but a soldier, and for one not fit to act.
    let events = world.step(&[shot(1)]);
    assert!(refused_with(&events, Refusal::NotASoldier));
    assert_eq!(world.can_stun_shot(1), Err(Refusal::NotASoldier));
    world.aboard.room.knock_out_for_probe(0);
    world.step(&[]);
    assert_eq!(world.can_stun_shot(0), Err(Refusal::OutOfReach));
    world.aboard.room.patch_up_for_probe(0);
    world.step(&[]);
    // Nothing in the hand to fire it from.
    let gear = world.aboard.room.gear(0);
    disarm(&mut world, 0);
    assert_eq!(world.can_stun_shot(0), Err(Refusal::NoWeaponInHand));
    world.aboard.room.issue(0, gear);
    assert_eq!(world.can_stun_shot(0), Ok(()));
    // Charging, then down: called off, and no cooldown for a shot never
    // fired.
    world.step(&[shot(0)]);
    assert!(world.is_charging(0));
    world.aboard.room.knock_out_for_probe(0);
    world.step(&[]);
    world.step(&[]);
    assert!(!world.is_charging(0), "going down calls it off");
    world.aboard.room.patch_up_for_probe(0);
    world.step(&[]);
    assert_eq!(world.can_stun_shot(0), Ok(()), "no cooldown");
    // Charging, then walked by the keys, rolled and ordered across the
    // deck (October 2026, the player's word): the charge goes on as he
    // walks, and fires at its two seconds.
    world.step(&[shot(0)]);
    assert!(world.is_charging(0));
    // Towards the farthest free deck tile within four tiles of him.
    let there = *world
        .aboard
        .room
        .free_tiles_near(world.aboard.room.bim_pos(0), 4.0 * TILE)
        .last()
        .expect("free deck about him");
    let towards = bims::order::angle_code((there - world.aboard.room.bim_pos(0)).angle());
    let control = |walk: Option<u16>| Command::Crew {
        slot: 0,
        order: bims::order::CrewOrder::Control {
            walk,
            aim: towards,
            fire: false,
            sprint: false,
        },
    };
    world.step(&[control(Some(towards))]);
    world.step(&[Command::Crew {
        slot: 0,
        order: bims::order::CrewOrder::Dodge,
    }]);
    world.step(&[control(None)]);
    assert!(
        world.is_charging(0),
        "the keys and a roll leave it charging"
    );
    let here = world.aboard.room.bim_pos(0);
    assert!((there - here).len() > 2.0 * TILE, "room to walk");
    world.step(&[Command::Crew {
        slot: 0,
        order: bims::order::CrewOrder::SendTo {
            who: 0,
            x: there.x,
            y: there.y,
        },
    }]);
    for _ in 0..60 {
        world.step(&[]);
    }
    assert!(world.is_charging(0), "an order to move leaves it charging");
    assert!(
        (world.aboard.room.bim_pos(0) - here).len() > 20.0,
        "walked on while it charged"
    );
    let mut fired = false;
    for _ in 0..((class::STUN_SHOT_CHARGE + 0.5) / SECONDS_A_STEP) as u32 {
        let events = world.step(&[]);
        fired |= events
            .iter()
            .any(|e| matches!(e, WorldEvent::StunShotFired { who: 0 }));
    }
    assert!(fired, "fired at the end of its charge");
    assert_eq!(world.can_stun_shot(0), Err(Refusal::CoolingDown));
    // And it is in the checksum: a soldier charging is a different world.
    let before = world_checksum(&world);
    world.soldiers[0].charging = Some(crate::soldier::Charging {
        until: world.mission_minutes() + 1.0,
        tile: own,
    });
    assert_ne!(world_checksum(&world), before);
}

#[test]
fn the_stun_shot_s_numbers_are_its_rank_s() {
    let mut world = soldier();
    for rank in 1..=4u8 {
        ranks(&mut world, 0, [0, 0, rank, 0]);
        let r = rank as usize - 1;
        assert_eq!(world.stun_shot_damage(0), class::STUN_SHOT_DAMAGE[r]);
        assert_eq!(world.stun_shot_radius(0), class::GRENADE_RADIUS[r]);
        assert_eq!(world.stun_shot_stun(0), 3.0);
        assert_eq!(world.stun_shot_cooldown(0), class::STUN_SHOT_COOLDOWN[r]);
    }
    assert_eq!(class::STUN_SHOT_CHARGE, 2.0);
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
    ranks(&mut world, 0, [2, 0, 1, 0]);
    assert_eq!(world.can_throw(0, tile), Ok(()));
    // Not fit to act.
    world.aboard.room.knock_out_for_probe(0);
    world.step(&[]);
    assert_eq!(world.can_throw(0, tile), Err(Refusal::OutOfReach));
    world.aboard.room.patch_up_for_probe(0);
    world.step(&[]);
    // No grenade left.
    world.set_charges_held(0, Charge::Grenade, 0);
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
    assert!(left > 0.0 && left <= COOLDOWN, "{left}");
    throw(&mut world, 0, tile);
    assert_eq!(grenades(&world, 0), 0, "both charges thrown");
    let events = throw(&mut world, 0, tile);
    assert!(refused_with(&events, Refusal::NoGrenade));
    assert_eq!(world.can_throw(0, tile), Err(Refusal::NoGrenade));
    // Thirty seconds of the clock on, one charge is back; thirty more
    // and it is at its two again. Two bursts two tiles off take the whole
    // of one bar (task 120), and a soldier dead of them gets nothing back,
    // so it is kept on its feet: the charges are what is asked here.
    // Held where it threw from: an idle crew member wanders.
    let spot = world.aboard.room.bim_pos(0);
    let steps = (COOLDOWN / SECONDS_A_STEP).ceil() as u32;
    for _ in 0..steps {
        world.aboard.room.put_for_probe(0, spot);
        world.aboard.room.patch_up_for_probe(0);
        world.step(&[]);
    }
    assert_eq!(grenades(&world, 0), 1, "one charge back");
    assert_eq!(world.can_throw(0, tile), Ok(()));
    for _ in 0..steps {
        world.aboard.room.put_for_probe(0, spot);
        world.aboard.room.patch_up_for_probe(0);
        world.step(&[]);
    }
    assert_eq!(grenades(&world, 0), 2);
    assert_eq!(
        world.grenade_cooldown_left(0),
        0.0,
        "nothing running at its charges"
    );
}

#[test]
fn the_fuse_burns_its_seconds_and_the_throw_moves_the_checksum() {
    let mut world = soldier();
    ranks(&mut world, 0, [2, 0, 1, 0]);
    let tile = open_run(&world, 0, 3)[2];
    let before = world_checksum(&world);
    throw(&mut world, 0, tile);
    assert_ne!(world_checksum(&world), before, "a throw is in the checksum");
    assert_eq!(world.aboard.room.grenades().len(), 1);
    let g = world.aboard.room.grenades()[0];
    assert_eq!(g.by, 0);
    assert_eq!(g.at, middle(tile));
    assert_eq!(g.fuse, class::GRENADE_FUSE);
    assert_eq!(g.radius, RADIUS * TILE);
    assert_eq!(g.damage, DAMAGE);
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
    let want = DAMAGE * (1.0 - 0.5 * d / RADIUS);
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
        assert!(d > 0.5 && d < RADIUS, "{d}");
        let want = DAMAGE * (1.0 - 0.5 * d / RADIUS);
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
            assert!(took < DAMAGE * 0.85, "less than the centre's");
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
    let dealt = DAMAGE * (1.0 - 0.5 * d / RADIUS);
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
fn a_burst_hurts_the_thrower_a_crewmate_and_a_sentry_and_blows_the_cover_up() {
    let mut world = soldier();
    ranks(&mut world, 0, [2, 0, 1, 0]);
    world.aboard.room.issue(1, Gear::default());
    let run = open_run(&world, 0, 3);
    // The crewmate on the second tile of the run, held there, a sentry
    // on the third, laid cover on the first, and the grenade at the
    // crewmate's feet: all of it within the radius.
    let mate = world.aboard.room.put_for_probe(1, middle(run[1]));
    world.aboard.room.recruit_for_probe(1, true);
    lay_bags(&mut world, middle(run[0]));
    let sentry = world.next_deployable;
    world.next_deployable += 1;
    world.deployables.push(Deployable {
        id: sentry,
        kind: DeployKind::Sentry,
        owner_slot: 1,
        deck: Deck::Ship,
        tile: design_tile(&world, middle(run[2])),
        health: SENTRY_HEALTH,
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
    for _ in 0..2_000 {
        world.step(&[]);
        if world.aboard.room.grenades().is_empty() {
            break;
        }
    }
    // The laid cover the burst reached is said to be blown (the room's
    // own word; nothing of the world's is laid there since task 154).
    assert_eq!(
        world.aboard.room.take_bags_blown(),
        vec![tile_of(middle(run[0]))],
        "the cover was blown up"
    );
    // The thrower, a tile off, and the crewmate at the centre — each
    // where it stood when the grenade went off.
    let burst = middle(run[1]);
    let dealt = |at: Vec2| {
        let d = (at - burst).len() / TILE;
        DAMAGE * (1.0 - 0.5 * d / RADIUS)
    };
    let me = world.aboard.room.bim_pos(0);
    let mine = my_health - world.aboard.room.health(0);
    assert!(mine > 0.0, "friendly fire, the thrower included");
    // The thrower stands where it threw from, a tile behind the bags on
    // the first tile of the run, so the burst reaches it over them and
    // is halved like any burst over cover. (It used to wander off before
    // the fuse ran out, which is what this read until September 2026
    // took the idle wander away.)
    assert!(
        plausible(mine, dealt(me) * 0.5),
        "the thrower took {mine}, {} was dealt over the bags",
        dealt(me) * 0.5
    );
    let theirs = mate_health - world.aboard.room.health(1);
    assert!(
        plausible(theirs, dealt(mate)),
        "the crewmate took {theirs}, {} was dealt",
        dealt(mate)
    );
    // The sentry, a tile off the other way, on its health — or gone, where
    // the burst there is more than a sentry has (the doubled burst,
    // feature 110, is).
    let at_sentry = dealt(middle(run[2]));
    match world.deployable(sentry) {
        Some(s) => assert!(
            (SENTRY_HEALTH - s.health - at_sentry).abs() < 1.0,
            "the sentry took {}",
            SENTRY_HEALTH - s.health
        ),
        None => assert!(
            at_sentry >= SENTRY_HEALTH,
            "the sentry went to a burst of {at_sentry}"
        ),
    }
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
        ranks(&mut world, 0, [2, 0, 1, 0]);
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
        let full = DAMAGE * (1.0 - 0.5 * d / RADIUS);
        let took = before - world.aboard.room.health(1);
        assert!(took > 0.0, "sandbags do not stop a burst");
        assert!(
            plausible(took, full * 0.5),
            "in cover it took {took}, half of {full}"
        );
        if took > 0.0 {
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
    ranks(&mut world, 0, [2, 0, 1, 0]);
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
                        && (m - b).len() <= RADIUS * TILE
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
    ranks(&mut world, 0, [4, 0, 1, 0]);
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
fn two_runs_of_a_grenade_fight_on_one_seed_are_the_same_fight() {
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
}

// --- D: twenty levels, a point a level to sixteen, a rank a point (task 124) ---

#[test]
fn the_soldier_climbs_sixteen_levels_as_every_class_does() {
    let mut world = simulation_world(flyer(3), REFERENCE_MONEY, 3);
    assert_eq!(world.set_class(0, Class::Soldier), Ok(()));
    assert_eq!(world.set_class(1, Class::Tank), Ok(()));
    assert_eq!(world.set_class(2, Class::None), Ok(()));
    // The tank climbs the same table since task 139.
    let mut events = Vec::new();
    world.award(0, 3_199, &mut events);
    world.award(1, 3_199, &mut events);
    world.award(2, 3_199, &mut events);
    assert_eq!((world.level_of(0), world.level_of(1)), (15, 15));
    world.award(0, 1, &mut events);
    world.award(1, 1, &mut events);
    assert_eq!(world.level_of(0), 16, "the sixteenth at 3 200");
    assert_eq!(world.level_of(1), 16, "the tank's too");
    world.award(0, 10_000, &mut events);
    world.award(1, 10_000, &mut events);
    assert_eq!((world.level_of(0), world.level_of(1)), (20, 20));
    // Every level said once, twenty each; a classless crew member learns
    // nothing.
    let said = |who: u32| {
        events
            .iter()
            .filter(|e| matches!(e, WorldEvent::LevelUp { who: w, .. } if *w == who))
            .count()
    };
    assert_eq!((said(0), said(1), said(2)), (19, 19, 0));
    // A point a level up to the sixteenth, the first included; none
    // without a class.
    assert_eq!(world.points_of(0), 16);
    assert_eq!(world.points_of(1), 16);
    assert_eq!(world.points_of(2), 0);
}

#[test]
fn a_rank_up_is_refused_without_a_kit_a_point_a_level_or_room_at_the_top() {
    let mut world = simulation_world(flyer(3), REFERENCE_MONEY, 3);
    assert_eq!(world.set_class(0, Class::Soldier), Ok(()));
    assert_eq!(world.set_class(1, Class::Tank), Ok(()));
    let rank_up = |world: &mut World, slot: u32, ability_slot: u32| {
        world.step(&[Command::RankUp { slot, ability_slot }])
    };
    // No class at all has no ranked kit; the tank has one (task 139).
    assert!(refused_with(&rank_up(&mut world, 2, 0), Refusal::NoClass));
    assert!(
        rank_up(&mut world, 1, 0)
            .iter()
            .any(|e| matches!(e, WorldEvent::RankedUp { who: 1, .. }))
    );
    // Nor is there a fifth slot.
    assert!(refused_with(
        &rank_up(&mut world, 0, 4),
        Refusal::NoRankedKit
    ));
    // The ultimate wants the sixth level.
    assert!(refused_with(
        &rank_up(&mut world, 0, 3),
        Refusal::RankLocked
    ));
    // The first level's point.
    let events = rank_up(&mut world, 0, 0);
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
        &rank_up(&mut world, 0, 1),
        Refusal::NoSkillPoint
    ));
    // A point to spend, and Q's second rank still a level off.
    level_up(&mut world, 0, 2);
    assert!(refused_with(
        &rank_up(&mut world, 0, 0),
        Refusal::RankLocked
    ));
    level_up(&mut world, 0, 16);
    for _ in 0..3 {
        rank_up(&mut world, 0, 0);
    }
    assert_eq!(world.rank_of(0, 0), 4);
    assert!(refused_with(&rank_up(&mut world, 0, 0), Refusal::TopRank));
    // A rank bought is in the checksum.
    let before = world_checksum(&world);
    rank_up(&mut world, 0, 1);
    assert_ne!(world_checksum(&world), before);
    // A point is spent between missions as well.
    world.leave_for_probe();
    rank_up(&mut world, 0, 2);
    assert_eq!(world.rank_of(0, 2), 1);
}

#[test]
fn frag_grenade_s_charges_cooldown_burst_and_radius_go_by_its_rank() {
    let mut world = soldier();
    let tile = open_run(&world, 0, 2)[1];
    assert_eq!(world.charges(0, Charge::Grenade), 0);
    assert_eq!(world.can_throw(0, tile), Err(Refusal::NoGrenadesYet));
    assert_eq!(
        (world.grenade_damage(0), world.grenade_radius(0)),
        (0.0, 0.0)
    );
    let want = [
        (60.0, 2.0, 1, 30.0),
        (75.0, 2.5, 2, 30.0),
        (90.0, 2.5, 2, 24.0),
        (110.0, 3.0, 2, 20.0),
    ];
    for (rank, &(damage, radius, charges, cooldown)) in (1..=4u8).zip(&want) {
        ranks(&mut world, 0, [rank, 0, 0, 0]);
        assert_eq!(world.grenade_damage(0), damage, "rank {rank}");
        assert_eq!(world.grenade_radius(0), radius, "rank {rank}");
        assert_eq!(world.charges(0, Charge::Grenade), charges);
        assert_eq!(world.grenades_of(0), charges, "put in hand with the rank");
        assert_eq!(world.grenade_cooldown(0), cooldown, "rank {rank}");
        assert_eq!(world.grenade_range(0), 8.0);
        assert_eq!(world.grenade_fuse(0), 2.0);
    }
    // And the fourth rank's burst in the air is its numbers.
    throw(&mut world, 0, tile);
    let g = world.aboard.room.grenades()[0];
    assert_eq!((g.damage, g.radius), (110.0, 3.0 * TILE));
}

/// A crit hit by the soldier on the staged machine's chassis, landed the
/// way a bolt lands it, and what the chassis lost.
fn crit_on_machine(world: &mut World, damage: f32, flat: f32) -> f32 {
    let bims = world.residents.as_ref().unwrap().aboard.room.crew_count() as usize;
    let before = machine(world).body.health(DroidPart::Chassis);
    let roll = (0..1000)
        .map(|k| k as f32 / 1000.0 + 0.0005)
        .find(|&r| DroidPart::hit_by(r) == DroidPart::Chassis)
        .unwrap();
    world.aboard.room.land_hit_for_probe(bims::combat::Hit {
        who: bims,
        damage,
        cut: false,
        by: Some(0),
        blast: false,
        roll,
        strips: 0.0,
        flat,
        crit: true,
    });
    world.step(&[]);
    before - machine(world).body.health(DroidPart::Chassis)
}

#[test]
fn a_critical_hit_adds_its_share_of_the_flat_damage_after_every_factor() {
    // Weak Spot's four ranks: a crit with no relic is the flat damage times
    // the crit damage, exactly.
    for (rank, crit) in (1..=4u8).zip(class::WEAK_SPOT_DAMAGE) {
        let mut world = fight();
        disarm(&mut world, 0);
        world.aboard.room.issue(1, Gear::default());
        ranks(&mut world, 0, [2, rank, 1, 0]);
        let chance = class::WEAK_SPOT_CHANCE[rank as usize - 1];
        assert_eq!(world.skill_of(0).crit_chance, chance);
        let took = crit_on_machine(&mut world, 10.0, 10.0);
        assert!(
            (took - 10.0 * crit).abs() < 1e-3,
            "rank {rank}: took {took}"
        );
    }
    // A *Glass Cannon*: the share is on the bolt's own damage — what the
    // room lands — and the crit adds its share of the flat damage alone,
    // so the lens never multiplies the crit.
    let mut world = fight();
    disarm(&mut world, 0);
    world.aboard.room.issue(1, Gear::default());
    ranks(&mut world, 0, [2, 4, 1, 0]);
    world.give_relic_for_probe(crate::relic::Relic::GlassCannon);
    let lens = world.skill_of(0).damage;
    assert!(lens > 1.0);
    let took = crit_on_machine(&mut world, 10.0 * lens, 10.0);
    let bonus = took - 10.0 * lens;
    assert!(
        (bonus - 10.0 * (class::WEAK_SPOT_DAMAGE[3] - 1.0)).abs() < 1e-3,
        "the crit's share {bonus}"
    );
    // No rank of Weak Spot, no crit's worth, whatever the hit says.
    let mut world = fight();
    disarm(&mut world, 0);
    world.aboard.room.issue(1, Gear::default());
    assert_eq!(world.skill_of(0).crit_chance, 0.0);
    assert!((crit_on_machine(&mut world, 10.0, 10.0) - 10.0).abs() < 1e-3);
}

#[test]
fn a_critical_hit_is_added_before_the_armour_takes_its_share() {
    // A Manufacturer in its armour (day six): a crit of a flat point is
    // 2.25 at the fourth rank, all of it past the armour's protection of
    // 1.8 — so the armour drains 0.45. Were the
    // crit added after the armour, the flat point would be stopped whole
    // and the crit's share land on the body.
    let mut world = simulation_world(flyer(2), REFERENCE_MONEY, 2);
    assert_eq!(world.set_class(0, Class::Soldier), Ok(()));
    ranks(&mut world, 0, [0, 4, 0, 0]);
    world
        .manufacturer_dock_for_probe(6)
        .expect("a Manufacturers' site");
    for _ in 0..3 {
        world.step(&[]);
    }
    disarm(&mut world, 0);
    world.aboard.room.issue(1, Gear::default());
    // Everybody held, so nothing lands but the hit.
    let found = {
        let room = &world.residents.as_ref().unwrap().aboard.room;
        (0..room.crew_count() as usize).find_map(|who| {
            let gear = room.gear(who);
            gear.worn()
                .filter(|p| !p.broken() && p.tier == bims::combat::Tier::One)
                .map(|p| (who, p.kind))
        })
    };
    let (who, _) = found.expect("a Manufacturer in armour on day six");
    let piece = |world: &World| {
        let room = &world.residents.as_ref().unwrap().aboard.room;
        (room.gear(who).worn().unwrap().health, room.health(who))
    };
    let (armour, body) = piece(&world);
    let roll = 0.5;
    world.aboard.room.land_hit_for_probe(bims::combat::Hit {
        who,
        damage: 1.0,
        cut: false,
        by: Some(0),
        blast: false,
        roll,
        strips: 0.0,
        flat: 1.0,
        crit: true,
    });
    world.step(&[]);
    let (armour_now, body_now) = piece(&world);
    assert!(
        (armour - armour_now - 0.45).abs() < 1e-3,
        "the piece drained {}",
        armour - armour_now
    );
    assert!(body_now >= body - 1e-3, "the body took nothing");
}

/// Steps enough for `seconds` of the mission clock.
fn run_for(world: &mut World, seconds: f64) {
    for _ in 0..(seconds / SECONDS_A_STEP).ceil() as u32 {
        world.aboard.room.patch_up_for_probe(0);
        world.step(&[]);
    }
}

#[test]
fn rampage_fires_faster_takes_less_and_aims_on_the_move_for_its_seconds() {
    let mut world = soldier();
    let events = world.step(&[Command::Rampage { slot: 0 }]);
    assert!(refused_with(&events, Refusal::NotLearnt), "{events:?}");
    let want = [
        (8.0, 1.5, 0.80, 35.0),
        (10.0, 1.75, 0.75, 30.0),
        (12.0, 2.0, 0.70, 25.0),
        (12.0, 2.0, 0.70, 20.0),
    ];
    for (rank, &(seconds, rate, taken, cooldown)) in (1..=4u8).zip(&want) {
        let mut world = soldier();
        ranks(&mut world, 0, [0, 0, 0, rank]);
        assert_eq!(world.rampage_seconds(0), seconds, "rank {rank}");
        assert_eq!(world.rampage_cooldown(0), cooldown, "rank {rank}");
        let before = world.skill_of(0);
        let events = world.step(&[Command::Rampage { slot: 0 }]);
        assert!(
            events
                .iter()
                .any(|e| matches!(e, WorldEvent::Rampaged { who: 0 })),
            "{events:?}"
        );
        assert!(world.is_rampaging(0));
        let s = world.skill_of(0);
        assert_eq!(s.fire_rate, before.fire_rate * rate, "rank {rank}");
        assert_eq!(s.damage_taken, taken, "rank {rank}");
        assert_eq!(s.walking, 1.0, "full aim on the move");
        assert_eq!(world.aboard.room.skill_for_probe(0), s);
        // Refused while it runs, and on its cooldown after.
        let events = world.step(&[Command::Rampage { slot: 0 }]);
        assert!(refused_with(&events, Refusal::AlreadyActive));
        // It ends on time.
        run_for(&mut world, seconds - 0.5);
        assert!(world.is_rampaging(0), "rank {rank}: still on");
        run_for(&mut world, 1.0);
        assert!(!world.is_rampaging(0), "rank {rank}: over");
        assert_eq!(world.skill_of(0), before);
        assert_eq!(world.can_rampage(0), Err(Refusal::CoolingDown));
        let left = world.rampage_cooldown_left(0);
        assert!(
            left > 0.0 && left <= cooldown - seconds + 1.0,
            "rank {rank}: {left}"
        );
    }
}

#[test]
fn rampage_is_refused_downed_ready_at_every_mission_and_goes_on_with_a_charge() {
    let mut world = soldier();
    ranks(&mut world, 0, [0, 0, 4, 4]);
    world.aboard.room.knock_out_for_probe(0);
    world.step(&[]);
    assert_eq!(world.can_rampage(0), Err(Refusal::OutOfReach), "downed");
    world.aboard.room.patch_up_for_probe(0);
    world.step(&[]);
    // A Stun Shot charging and on a Rampage: both at once.
    let own = tile_of(world.aboard.room.bim_pos(0));
    world.step(&[Command::StunShot {
        slot: 0,
        x: own.0,
        y: own.1,
    }]);
    assert_eq!(world.can_rampage(0), Ok(()), "may go on one charging");
    world.step(&[Command::Rampage { slot: 0 }]);
    assert!(world.is_charging(0) && world.is_rampaging(0));
    let s = world.skill_of(0);
    assert!((s.damage_taken - 0.70).abs() < 1e-6, "{}", s.damage_taken);
    assert_eq!(s.fire_rate, 2.0);
    let here = world.aboard.room.bim_pos(0);
    run_for(&mut world, 2.0);
    assert!(
        (world.aboard.room.bim_pos(0) - here).len() < 1.0,
        "still held where it stands"
    );
    // The fourth rank's extension: a second a machine it downs, six at
    // the most.
    let until = world.soldier_of(0).until;
    for _ in 0..10 {
        world.rampage_kill(0);
    }
    let s = world.soldier_of(0);
    assert_eq!(s.extended, class::RAMPAGE_EXTEND_MAX);
    assert!(
        (s.until - until - 6.0 * time::MINUTES_PER_SECOND).abs() < 1e-9,
        "{} {}",
        s.until,
        until
    );
    // And a kill credited through the bounty's own door counts.
    let mut other = soldier();
    ranks(&mut other, 0, [0, 0, 0, 4]);
    other.step(&[Command::Rampage { slot: 0 }]);
    other.machine_kills_noted(&[(Some(0), 0)]);
    assert_eq!(other.soldier_of(0).extended, 1.0);
    // Not below the fourth rank.
    let mut third = soldier();
    ranks(&mut third, 0, [0, 0, 0, 3]);
    third.step(&[Command::Rampage { slot: 0 }]);
    third.rampage_kill(0);
    assert_eq!(third.soldier_of(0).extended, 0.0);
    // Ready at every mission's start, whatever its cooldown.
    run_for(&mut world, 5.0);
    assert!(world.rampage_cooldown_left(0) > 0.0);
    world.leave_for_probe();
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
    assert_eq!(world.rampage_cooldown_left(0), 0.0);
    assert_eq!(world.can_rampage(0), Ok(()));
}

#[test]
fn the_cooldown_relics_shorten_the_rampage_s_cooldown() {
    let mut world = soldier();
    ranks(&mut world, 0, [0, 0, 0, 1]);
    let plain = world.rampage_cooldown(0);
    world.give_relic_for_probe(crate::relic::Relic::OverclockedCores);
    assert!(world.rampage_cooldown(0) < plain, "*Overclocked Cores*");
    world.step(&[Command::Rampage { slot: 0 }]);
    run_for(&mut world, 10.0);
    let left = world.rampage_cooldown_left(0);
    // Seconds taken off (a *Reset Capacitor*'s way) every class cooldown running.
    world.cooldowns_less(0, 3.0);
    assert!(world.rampage_cooldown_left(0) < left, "seconds off");
    // And it leaves the Rampage running's end alone.
    assert!(!world.is_rampaging(0));
}

#[test]
fn a_soldier_s_ranks_and_rampage_are_saved_and_hashed() {
    let run = || {
        let mut world = fight();
        ranks(&mut world, 0, [2, 4, 1, 4]);
        world.step(&[Command::Rampage { slot: 0 }]);
        for _ in 0..600 {
            world.step(&[]);
        }
        world_checksum(&world)
    };
    assert_eq!(run(), run(), "one seed, one fight, crits and all");
    let mut world = soldier();
    ranks(&mut world, 0, [0, 0, 0, 1]);
    let before = world_checksum(&world);
    world.step(&[Command::Rampage { slot: 0 }]);
    assert_ne!(
        world_checksum(&world),
        before,
        "a Rampage is in the checksum"
    );
}

/// A deck tile `lo` to `hi` tiles off the soldier that it could throw at
/// from somewhere it can walk to, and that it cannot throw at from where
/// it stands for `why`: the nearest such, by row then column on a tie.
fn throw_target(world: &World, lo: f32, hi: f32, why: Refusal) -> (i32, i32) {
    let from = world.aboard.room.bim_pos(0);
    let here = tile_of(from);
    let span = hi.ceil() as i32;
    let mut found: Vec<(f32, i32, i32)> = (-span..=span)
        .flat_map(|dy| (-span..=span).map(move |dx| (here.0 + dx, here.1 + dy)))
        .filter(|&t| {
            let d = (middle(t) - from).len() / TILE;
            (lo..=hi).contains(&d)
                && world.aboard.room.is_deck_tile(middle(t))
                && world.can_throw(0, t) == Err(why)
                && world.throw_stand(0, false, t).is_some()
        })
        .map(|t| ((middle(t) - from).len(), t.1, t.0))
        .collect();
    found.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)).then(a.2.cmp(&b.2)));
    let (_, y, x) = *found.first().expect("a tile to walk out and throw at");
    (x, y)
}

fn throw_at(world: &mut World, tile: (i32, i32)) -> Vec<WorldEvent> {
    world.step(&[Command::ThrowAt {
        slot: 0,
        satchel: false,
        x: tile.0,
        y: tile.1,
    }])
}

/// Step until the soldier throws, at most `seconds` of the clock: the
/// step it did, or `None`.
fn steps_to_the_throw(world: &mut World, seconds: u32) -> Option<u32> {
    for step in 1..=seconds * 60 {
        let events = world.step(&[]);
        if events
            .iter()
            .any(|e| matches!(e, WorldEvent::Thrown { who: 0 }))
        {
            return Some(step);
        }
    }
    None
}

/// **A throw out of reach is walked out to** (the ability range
/// indicators): the soldier is not refused, walks to a spot within reach
/// of the tile and throws from there, the grenade in the room and the
/// pending throw gone.
#[test]
fn a_throw_out_of_reach_walks_to_a_spot_in_reach_and_lands() {
    let mut world = soldier();
    ranks(&mut world, 0, [1, 0, 0, 0]);
    let range = world.grenade_range(0);
    let tile = throw_target(&world, range + 2.0, range + 8.0, Refusal::OutOfThrowRange);
    let start = world.aboard.room.bim_pos(0);
    let held = grenades(&world, 0);
    let events = throw_at(&mut world, tile);
    assert!(
        !events
            .iter()
            .any(|e| matches!(e, WorldEvent::Refused { .. })),
        "{events:?}"
    );
    assert_eq!(world.throws.len(), 1, "walking out to throw");
    assert_eq!(grenades(&world, 0), held, "nothing thrown yet");
    let step = steps_to_the_throw(&mut world, 30).expect("thrown within thirty seconds");
    assert!(step > 1, "it walked first");
    let at = world.aboard.room.bim_pos(0);
    assert!((at - start).len() > TILE, "the soldier moved");
    assert!(
        (middle(tile) - at).len() <= range * TILE,
        "thrown from within reach"
    );
    assert_eq!(grenades(&world, 0), held - 1);
    assert!(world.throws.is_empty());
    assert!(
        !world.aboard.room.grenades().is_empty(),
        "a grenade in the air"
    );
}

/// **A throw behind a wall walks round to a line**: in reach but with a
/// wall between, the soldier walks to where it can see the tile and
/// throws from there.
#[test]
fn a_throw_behind_a_wall_walks_round_to_a_line_and_lands() {
    let mut world = soldier();
    ranks(&mut world, 0, [1, 0, 0, 0]);
    let range = world.grenade_range(0);
    let tile = throw_target(&world, 1.0, range, Refusal::NoLineToTile);
    let events = throw_at(&mut world, tile);
    assert!(
        !events
            .iter()
            .any(|e| matches!(e, WorldEvent::Refused { .. })),
        "{events:?}"
    );
    steps_to_the_throw(&mut world, 30).expect("thrown within thirty seconds");
    let at = world.aboard.room.bim_pos(0);
    assert!(
        world.aboard.room.line_clear(at, middle(tile)),
        "a line from where it threw"
    );
}

/// **Another order calls a throw walked out to off**: a move given while
/// the soldier walks out means the throw is not made, the grenade kept;
/// in reach it is thrown at once, and a tile no spot reaches is refused.
#[test]
fn another_order_calls_off_a_throw_walked_out_to() {
    let mut world = soldier();
    ranks(&mut world, 0, [1, 0, 0, 0]);
    let range = world.grenade_range(0);
    let far = throw_target(&world, range + 2.0, range + 8.0, Refusal::OutOfThrowRange);
    let held = grenades(&world, 0);
    assert!(held > 0, "a grenade to throw");
    throw_at(&mut world, far);
    assert_eq!(world.throws.len(), 1);
    let here = world.aboard.room.bim_pos(0);
    world.step(&[Command::Crew {
        slot: 0,
        order: bims::order::CrewOrder::Move {
            x: here.x,
            y: here.y,
        },
    }]);
    assert!(world.throws.is_empty(), "called off");
    assert_eq!(steps_to_the_throw(&mut world, 10), None, "and never made");
    assert_eq!(grenades(&world, 0), held);

    // In reach with a line: thrown the step it is ordered.
    let near = open_run(&world, 0, 3)[2];
    let events = throw_at(&mut world, near);
    assert!(
        events
            .iter()
            .any(|e| matches!(e, WorldEvent::Thrown { who: 0 })),
        "{events:?}"
    );
    assert!(world.throws.is_empty());

    // Not deck at all: nowhere to throw from, refused at once.
    give_grenade(&mut world, 0);
    let at = tile_of(world.aboard.room.bim_pos(0));
    let void = (-40..=40)
        .flat_map(|dx| (-40..=40).map(move |dy| (at.0 + dx, at.1 + dy)))
        .find(|&t| !world.aboard.room.is_deck_tile(middle(t)))
        .expect("a tile that is not deck");
    let events = throw_at(&mut world, void);
    assert!(refused_with(&events, Refusal::CantThrowThere), "{events:?}");
    assert!(world.throws.is_empty());
}
