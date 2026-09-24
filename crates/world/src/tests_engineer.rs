//! The engineer class (feature 74): `crate::class` and `crate::deploy`.
//! Experience and levels, the picks, the class chosen at the start; the
//! kits laid as sandbags and sentries, what a deploy is refused for, the
//! cover on both rooms, a sentry's fire and its end; and each of the ten
//! levels' talents doing what it says.

use bims::combat::{Gear, Item, Tier, WeaponKind};
use economy::trade_price;
use physics::ResourceId;
use shipdesign::Rotation;
use shipdesign::fixture::flyer;
use shipdesign::parts::PartKind;

use crate::Target;
use crate::class::{self, Class, LEVEL_XP, Progress, Side, Talent};
use crate::deploy::{self, Deck, DeployKind, Kit};
use crate::event::{Refusal, WorldEvent};
use crate::fixture::{REFERENCE_MONEY, simulation_world};
use crate::world::{Command, ShipState, Workbench, World};
use crate::world_checksum;

const TILE: f32 = shipdesign::TILE as f32;

fn basic() -> World {
    simulation_world(flyer(2), REFERENCE_MONEY, 2)
}

/// [`basic`] with slot 0 an engineer.
fn engineer() -> World {
    let mut world = basic();
    assert_eq!(world.set_class(0, Class::Engineer), Ok(()));
    world
}

fn refused_with(events: &[WorldEvent], want: Refusal) -> bool {
    events
        .iter()
        .any(|e| matches!(e, WorldEvent::Refused { why, .. } if *why == want))
}

/// Straight to a level, off the table.
fn level_up(world: &mut World, who: usize, level: u8) -> Vec<WorldEvent> {
    let mut events = Vec::new();
    let want = LEVEL_XP[level as usize - 1];
    let have = world.progress_of(who as u32).xp;
    world.award(who, want.saturating_sub(have), &mut events);
    assert_eq!(world.progress_of(who as u32).level(), level);
    events
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

fn kits_in_pack(world: &World, who: usize, kit: Kit) -> usize {
    let wanted = Item::Stack(kit.resource() as u32);
    world
        .aboard
        .room
        .pack(who)
        .iter()
        .filter(|i| **i == Some(wanted))
        .count()
}

/// Every one of that kit out of the pack again, for a test that wants
/// the engineer without the ones its class dealt it.
fn drop_kits(world: &mut World, who: usize, kit: Kit) {
    let wanted = Item::Stack(kit.resource() as u32);
    let pack = world.aboard.room.pack(who);
    for (cell, item) in pack.iter().enumerate() {
        if *item == Some(wanted) {
            world.aboard.room.take(who, cell);
        }
    }
    assert_eq!(kits_in_pack(world, who, kit), 0);
}

fn give_kit(world: &mut World, who: usize, kit: Kit) {
    assert!(
        world
            .aboard
            .room
            .give(who, None, Item::Stack(kit.resource() as u32)),
        "room in the pack"
    );
}

/// The nearest tile to `who` that a kit could be laid on, in room tiles.
fn tile_near(world: &World, who: u32, kit: Kit) -> (i32, i32) {
    let here = world.aboard.room.bim_pos(who as usize);
    let (cx, cy) = (
        (here.x / TILE).floor() as i32,
        (here.y / TILE).floor() as i32,
    );
    let mut ring: Vec<(i32, i32)> = Vec::new();
    for r in 1i32..6 {
        for dx in -r..=r {
            for dy in -r..=r {
                if dx.abs().max(dy.abs()) == r {
                    ring.push((cx + dx, cy + dy));
                }
            }
        }
    }
    ring.into_iter()
        .find(|&t| world.can_deploy(who, kit, t).is_ok())
        .expect("a free tile near the Bim")
}

/// Lay a kit and run until it is down. The steps it took, and the id.
fn deploy_now(world: &mut World, who: u32, kit: Kit, tile: (i32, i32)) -> (u32, u32) {
    let mut events = world.step(&[Command::Deploy {
        slot: who,
        kit,
        x: tile.0,
        y: tile.1,
    }]);
    assert!(
        !events
            .iter()
            .any(|e| matches!(e, WorldEvent::Refused { .. })),
        "{events:?}"
    );
    for step in 1..20_000u32 {
        if events
            .iter()
            .any(|e| matches!(e, WorldEvent::Deployed { who: w, .. } if *w == who))
        {
            let id = world.deployables.iter().map(|d| d.id).max().unwrap();
            return (step, id);
        }
        events = world.step(&[]);
    }
    panic!("the kit was never laid");
}

/// A design tile of the ship near `who` where the rules would take a
/// sandbag part: what a site is laid on.
fn site_near(world: &World, who: u32) -> (u32, u32) {
    let here = world.aboard.room.bim_pos(who as usize);
    let (_, p) = world
        .aboard
        .design_of(worldgen::math::dvec2(here.x as f64, here.y as f64));
    let t = shipdesign::TILE as f64;
    let (cx, cy) = ((p.x / t).floor() as i32, (p.y / t).floor() as i32);
    for r in 1i32..8 {
        for dx in -r..=r {
            for dy in -r..=r {
                if dx.abs().max(dy.abs()) != r || cx + dx < 0 || cy + dy < 0 {
                    continue;
                }
                let origin = ((cx + dx) as u32, (cy + dy) as u32);
                if world
                    .can_place_site(PartKind::Sandbags, origin, Rotation::R0)
                    .is_ok()
                {
                    return origin;
                }
            }
        }
    }
    panic!("nowhere near the Bim for a site");
}

/// The hostile dock with resident 0 stood a few tiles down the corridor
/// from James, the rest of the garrison down (`stage_fight_for_probe`).
fn fight() -> World {
    let mut world = engineer();
    assert!(world.stage_fight_for_probe());
    let ashore = world.residents.as_mut().unwrap();
    for other in 1..ashore.aboard.count() as usize {
        ashore.aboard.room.kill_for_probe(other);
    }
    // Disarmed while the kits are laid: a hit drops a deploy, and a
    // resident four tiles off with a pistol lands one every few seconds.
    // `arm_resident` gives the pistol back for the fight.
    ashore.aboard.room.issue(0, Gear::default());
    // And the garrison's deaths taken before anything is measured.
    for _ in 0..3 {
        world.step(&[]);
    }
    world
}

/// The staged resident's pistol back in its hand.
fn arm_resident(world: &mut World) {
    let mut gear = Gear::default();
    gear.weapon = Some(WeaponKind::LaserPistol.basic());
    world.residents.as_mut().unwrap().aboard.room.issue(0, gear);
}

// --- A: experience, levels and picks -----------------------------------------

#[test]
fn experience_climbs_the_levels_and_a_level_up_is_said_once() {
    let mut world = engineer();
    assert_eq!(world.progress_of(0).level(), 1);
    let mut events = Vec::new();
    world.award(0, 99, &mut events);
    assert!(events.is_empty());
    world.award(0, 1, &mut events);
    assert_eq!(
        events,
        vec![WorldEvent::LevelUp {
            who: 0,
            class: Class::Engineer.code(),
            level: 2
        }]
    );
    events.clear();
    world.award(0, 1, &mut events);
    assert!(events.is_empty(), "a level is said once");
    for level in 3..=class::LEVELS {
        let got = level_up(&mut world, 0, level);
        assert_eq!(
            got,
            vec![WorldEvent::LevelUp {
                who: 0,
                class: Class::Engineer.code(),
                level: level as u32
            }]
        );
    }
    assert_eq!(world.progress_of(0).to_next(), 0, "the top");
    // Slot 1 has no class and learns nothing.
    world.award(1, 1_000, &mut events);
    assert_eq!(world.progress_of(1), Progress::default());
    assert!(!world.has_talent(1, Talent::ReinforcedSand));
}

#[test]
fn an_enemy_going_down_and_dying_is_experience_once_each_to_the_classed_crew_in_range() {
    let mut world = fight();
    assert_eq!(world.set_class(1, Class::Engineer), Ok(()));
    // James is in the vicinity of himself, and of a point fifty tiles off,
    // and not of one fifty-one off.
    let here = world.aboard.room.bim_pos(0);
    assert!(world.in_vicinity(0, here + bims::math::vec2(50.0 * TILE, 0.0)));
    assert!(!world.in_vicinity(0, here + bims::math::vec2(51.0 * TILE, 0.0)));
    // Kate beside James, so both are in range of the resident; the
    // resident is a few tiles from James.
    world
        .aboard
        .room
        .put_for_probe(1, here + bims::math::vec2(TILE, 0.0));
    world.step(&[]);
    let (a, b) = (world.progress_of(0).xp, world.progress_of(1).xp);
    world
        .residents
        .as_mut()
        .unwrap()
        .aboard
        .room
        .knock_out_for_probe(0);
    for _ in 0..3 {
        world.step(&[]);
    }
    assert_eq!(world.progress_of(0).xp, a + class::XP_ENEMY_DOWN, "James");
    assert_eq!(world.progress_of(1).xp, b + class::XP_ENEMY_DOWN, "Kate");
    world.step(&[]);
    assert_eq!(world.progress_of(0).xp, a + class::XP_ENEMY_DOWN, "once");
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
    assert_eq!(
        world.progress_of(0).xp,
        a + class::XP_ENEMY_DOWN + class::XP_ENEMY_DEAD,
        "and dying"
    );
    world.step(&[]);
    assert_eq!(
        world.progress_of(1).xp,
        b + class::XP_ENEMY_DOWN + class::XP_ENEMY_DEAD,
        "once each"
    );
    // A crewmate going down or dying is nobody's experience.
    let a = world.progress_of(0).xp;
    world.aboard.room.knock_out_for_probe(1);
    for _ in 0..3 {
        world.step(&[]);
    }
    world.aboard.room.kill_for_probe(1);
    for _ in 0..3 {
        world.step(&[]);
    }
    assert_eq!(world.progress_of(0).xp, a);
    // And a dead player's progress is kept, for the buyback (feature
    // 103): the level, the experience and the talents come back with it.
    assert_eq!(
        world.progress_of(1).xp,
        b + class::XP_ENEMY_DOWN + class::XP_ENEMY_DEAD
    );
    assert_ne!(world.progress_of(1), Progress::default());
}

#[test]
fn an_enemy_down_on_an_unjoined_deck_and_one_seen_by_a_classless_crew_is_nothing() {
    let mut world = fight();
    // Kate, no class, beside James.
    let here = world.aboard.room.bim_pos(0);
    world
        .aboard
        .room
        .put_for_probe(1, here + bims::math::vec2(TILE, 0.0));
    world.step(&[]);
    let before = world.progress_of(0).xp;
    world.undock_for_probe();
    assert!(!world.aboard.is_joined());
    world
        .residents
        .as_mut()
        .unwrap()
        .aboard
        .room
        .knock_out_for_probe(0);
    for _ in 0..3 {
        world.step(&[]);
    }
    assert_eq!(
        world.progress_of(0).xp,
        before,
        "unjoined: nobody is in range"
    );
    assert_eq!(world.progress_of(1).xp, 0, "and Kate has no class");
}

#[test]
fn a_site_finished_and_a_kit_laid_are_the_engineer_s_and_a_re_used_kit_is_not() {
    let mut world = engineer();
    // A site is a part built onto the ship: the old game's (feature 102).
    world.set_shipyard_enabled(true);
    let before = world.progress_of(0).xp;
    // A site of James's own, and one Kate finishes beside him.
    let here = world.aboard.room.bim_pos(1);
    world.aboard.room.put_for_probe(1, here);
    let origin = site_near(&world, 0);
    world.ship.design.cargo[ResourceId::Vegetable as usize] += 10;
    let events = world.step(&[Command::PlaceSite {
        slot: 0,
        kind: PartKind::Sandbags,
        origin,
        rotation: Rotation::R0,
    }]);
    assert!(
        events
            .iter()
            .any(|e| matches!(e, WorldEvent::SitePlaced { .. })),
        "{events:?}"
    );
    let site = world.builds[0].id;
    let mut events = Vec::new();
    world.finish_build(site, 0, &mut events);
    assert!(events.iter().any(|e| matches!(e, WorldEvent::Built { .. })));
    assert_eq!(world.progress_of(0).xp, before + class::XP_BUILT, "his own");
    // Kate's, within his vicinity.
    let origin = site_near(&world, 0);
    let events2 = world.step(&[Command::PlaceSite {
        slot: 0,
        kind: PartKind::Sandbags,
        origin,
        rotation: Rotation::R0,
    }]);
    assert!(
        events2
            .iter()
            .any(|e| matches!(e, WorldEvent::SitePlaced { .. })),
        "{events2:?}"
    );
    let site = world.builds[0].id;
    world.finish_build(site, 1, &mut events);
    assert_eq!(
        world.progress_of(0).xp,
        before + 2 * class::XP_BUILT,
        "a crewmate's"
    );
    assert_eq!(world.progress_of(1).xp, 0, "Kate has no class");
    // A kit laid is two more; the same kit packed up and laid again is
    // nothing, since it was re-used.
    let tile = tile_near(&world, 0, Kit::Sandbag);
    let (_, id) = deploy_now(&mut world, 0, Kit::Sandbag, tile);
    assert_eq!(
        world.progress_of(0).xp,
        before + 3 * class::XP_BUILT,
        "laid"
    );
    let events = world.step(&[Command::PackUp { slot: 0, id }]);
    assert!(
        events
            .iter()
            .any(|e| matches!(e, WorldEvent::PackedUp { who: 0, .. })),
        "{events:?}"
    );
    assert_eq!(world.reused_kits[0], 1);
    let tile = tile_near(&world, 0, Kit::Sandbag);
    deploy_now(&mut world, 0, Kit::Sandbag, tile);
    assert_eq!(world.reused_kits[0], 0);
    assert_eq!(
        world.progress_of(0).xp,
        before + 3 * class::XP_BUILT,
        "re-used"
    );
}

#[test]
fn a_pick_is_refused_for_the_wrong_slot_level_or_a_second_time_and_moves_the_checksum() {
    let mut world = engineer();
    let events = world.step(&[Command::PickTalent {
        slot: 1,
        level: 2,
        side: Side::Left,
    }]);
    assert!(refused_with(&events, Refusal::NoClass), "no class");
    let events = world.step(&[Command::PickTalent {
        slot: 0,
        level: 2,
        side: Side::Left,
    }]);
    assert!(refused_with(&events, Refusal::LevelNotReached));
    level_up(&mut world, 0, 3);
    let events = world.step(&[Command::PickTalent {
        slot: 0,
        level: 3,
        side: Side::Left,
    }]);
    assert!(
        refused_with(&events, Refusal::NotAPickLevel),
        "a fixed level"
    );
    assert_eq!(world.progress_of(0).pending_pick(Class::Engineer), Some(2));
    let before = world_checksum(&world);
    let events = world.step(&[Command::PickTalent {
        slot: 0,
        level: 2,
        side: Side::Left,
    }]);
    assert!(
        events.iter().any(|e| matches!(
            e,
            WorldEvent::TalentPicked { who: 0, talent } if *talent == Talent::ReinforcedSand.code()
        )),
        "{events:?}"
    );
    assert_ne!(world_checksum(&world), before, "a pick is in the checksum");
    assert_eq!(world.progress_of(0).pending_pick(Class::Engineer), None);
    let events = world.step(&[Command::PickTalent {
        slot: 0,
        level: 2,
        side: Side::Right,
    }]);
    assert!(
        refused_with(&events, Refusal::AlreadyPicked),
        "never changed"
    );
    assert!(
        world.has_talent(0, Talent::ReinforcedSand) && !world.has_talent(0, Talent::SiteForeman)
    );
}

// --- B: the class at the start ----------------------------------------------

#[test]
fn an_engineer_sets_out_with_its_kits_and_the_class_locks_at_the_first_undock() {
    let mut world = basic();
    // The old game's clock, running with the step (feature 103).
    world.set_free_clock(true);
    assert_eq!(world.class_of(0), Class::None);
    assert_eq!(kits_in_pack(&world, 0, Kit::Sandbag), 0);
    let events = world.step(&[Command::SetClass {
        slot: 0,
        class: Class::Engineer,
    }]);
    assert!(
        !events
            .iter()
            .any(|e| matches!(e, WorldEvent::Refused { .. }))
    );
    assert_eq!(world.class_of(0), Class::Engineer);
    assert_eq!(
        kits_in_pack(&world, 0, Kit::Sandbag),
        deploy::SANDBAG_CHARGES as usize
    );
    assert_eq!(
        kits_in_pack(&world, 0, Kit::Sentry),
        deploy::SENTRY_CHARGES as usize,
        "and the sentry kit its Q is"
    );
    // Put back to none, the kits come out again.
    world.step(&[Command::SetClass {
        slot: 0,
        class: Class::None,
    }]);
    assert_eq!(kits_in_pack(&world, 0, Kit::Sandbag), 0);
    assert_eq!(kits_in_pack(&world, 0, Kit::Sentry), 0);
    world.step(&[Command::SetClass {
        slot: 0,
        class: Class::Engineer,
    }]);
    // The kits are priced by hand since the money rework took the labour
    // rule away with the production chains it was written to keep honest.
    assert_eq!(trade_price(ResourceId::SandbagKit), 60);
    assert_eq!(trade_price(ResourceId::SentryKit), 880);
    // Off the berth, and the class is fixed.
    world.man_the_helm_for_probe(0);
    let target = Target::Point(
        world
            .ship
            .position()
            .add(worldgen::math::dvec2(5_000.0, 0.0)),
    );
    world.step(&[Command::Confirm { slot: 0, target }]);
    for _ in 0..62 * 60 {
        if !matches!(world.ship.state, ShipState::Docked { .. }) {
            break;
        }
        world.step(&[]);
    }
    assert!(!matches!(world.ship.state, ShipState::Docked { .. }));
    world.step(&[]);
    assert!(world.undocked_once);
    let events = world.step(&[Command::SetClass {
        slot: 0,
        class: Class::None,
    }]);
    assert!(refused_with(&events, Refusal::ClassLocked));
    assert_eq!(world.class_of(0), Class::Engineer);
}

#[test]
fn kits_go_into_the_pack_for_a_probe() {
    let mut world = engineer();
    let own = deploy::SENTRY_CHARGES as usize;
    assert_eq!(kits_in_pack(&world, 0, Kit::Sentry), own, "its class's own");
    assert_eq!(world.give_kits_for_probe(0, Kit::Sentry, 2), 2);
    assert_eq!(kits_in_pack(&world, 0, Kit::Sentry), own + 2);
    assert_eq!(world.kits_of(0, Kit::Sentry), own as u32 + 2);
    assert_eq!(
        kits_in_pack(&world, 0, Kit::Sandbag),
        deploy::SANDBAG_CHARGES as usize,
        "the sandbags it set out with are untouched"
    );
    // Nobody aboard gets nothing, and neither does a pack with no room.
    assert_eq!(world.give_kits_for_probe(99, Kit::Sentry, 2), 0);
    let fitted = world.give_kits_for_probe(0, Kit::Sentry, 40);
    assert!(fitted < 40, "the pack fills up: {fitted}");
}

// --- C: sandbags --------------------------------------------------------------

#[test]
fn every_reason_a_deploy_is_refused() {
    let mut world = engineer();
    let tile = tile_near(&world, 0, Kit::Sandbag);
    // Slot 1 is nobody's engineer.
    give_kit(&mut world, 1, Kit::Sandbag);
    assert_eq!(
        world.can_deploy(1, Kit::Sandbag, tile),
        Err(Refusal::NotAnEngineer)
    );
    // No sentry kit in the pack, once its class's own is out of it.
    drop_kits(&mut world, 0, Kit::Sentry);
    assert_eq!(world.can_deploy(0, Kit::Sentry, tile), Err(Refusal::NoKit));
    give_kit(&mut world, 0, Kit::Sentry);
    // A sentry before the third level.
    assert_eq!(
        world.can_deploy(0, Kit::Sentry, tile),
        Err(Refusal::NoSentryYet)
    );
    level_up(&mut world, 0, class::SENTRY_LEVEL);
    assert_eq!(world.can_deploy(0, Kit::Sentry, tile), Ok(()));
    // A tile that will not take it: a wall, and one with a deployable on it.
    assert_eq!(
        world.can_deploy(0, Kit::Sandbag, (-1, -1)),
        Err(Refusal::CantDeployThere)
    );
    let (_, _) = deploy_now(&mut world, 0, Kit::Sandbag, tile);
    assert_eq!(
        world.can_deploy(0, Kit::Sandbag, tile),
        Err(Refusal::CantDeployThere)
    );
    // A second sentry is **not** refused any more (feature 88): the
    // charges are the world limit and one over it destroys the oldest.
    let tile = tile_near(&world, 0, Kit::Sentry);
    let (_, first) = deploy_now(&mut world, 0, Kit::Sentry, tile);
    give_kit(&mut world, 0, Kit::Sentry);
    let tile = tile_near(&world, 0, Kit::Sentry);
    assert_eq!(world.can_deploy(0, Kit::Sentry, tile), Ok(()));
    let (_, second) = deploy_now(&mut world, 0, Kit::Sentry, tile);
    assert_eq!(world.sentries_of(0), 1, "one standing still");
    assert!(world.deployable(first).is_none());
    assert!(world.deployable(second).is_some());
}

#[test]
fn laying_sandbags_takes_its_minutes_and_a_hit_drops_it_with_the_kit_kept() {
    let mut world = engineer();
    let tile = tile_near(&world, 0, Kit::Sandbag);
    let kits = kits_in_pack(&world, 0, Kit::Sandbag);
    let (steps, id) = deploy_now(&mut world, 0, Kit::Sandbag, tile);
    let work = (deploy::DEPLOY_SANDBAG_MINUTES / crate::data::STEP_MINUTES) as u32;
    assert!(steps >= work, "{steps} steps for {work} of work");
    assert!(
        steps < work + 60 * 20,
        "{steps} steps: a walk of under twenty minutes"
    );
    assert_eq!(kits_in_pack(&world, 0, Kit::Sandbag), kits - 1);
    let laid = world.deployable(id).unwrap();
    assert_eq!(laid.kind, DeployKind::Sandbags);
    assert_eq!(laid.deck, Deck::Ship);
    assert_eq!(laid.health, deploy::SANDBAG_HEALTH);
    assert_eq!(laid.owner_slot, 0);
    // A second one, hit half-way: the errand dropped, the kit still there.
    let tile = tile_near(&world, 0, Kit::Sandbag);
    let events = world.step(&[Command::Deploy {
        slot: 0,
        kit: Kit::Sandbag,
        x: tile.0,
        y: tile.1,
    }]);
    assert!(
        !events
            .iter()
            .any(|e| matches!(e, WorldEvent::Refused { .. }))
    );
    assert!(world.aboard.room.is_deploying(0));
    for _ in 0..work / 2 {
        world.step(&[]);
    }
    assert!(world.aboard.room.is_deploying(0));
    world.aboard.room.wound(0, bims::health::Part::Body, 5.0);
    assert!(!world.aboard.room.is_deploying(0), "dropped");
    for _ in 0..work * 2 {
        world.step(&[]);
    }
    assert_eq!(world.deployables.len(), 1, "nothing more laid");
    assert_eq!(
        kits_in_pack(&world, 0, Kit::Sandbag),
        kits - 1,
        "the kit kept"
    );
}

#[test]
fn laid_sandbags_are_cover_in_both_rooms_both_ways_and_survive_a_relayout() {
    let mut world = fight();
    let tile = tile_near(&world, 0, Kit::Sandbag);
    let (_, id) = deploy_now(&mut world, 0, Kit::Sandbag, tile);
    let laid = *world.deployable(id).unwrap();
    let station = world.residents.as_ref().unwrap().station;
    assert_eq!(laid.deck, Deck::Station(station), "on the station's deck");
    let at = bims::math::vec2((tile.0 as f32 + 0.5) * TILE, (tile.1 as f32 + 0.5) * TILE);
    assert_eq!(world.aboard.room.laid_cover().len(), 1);
    let crew_room = &world.aboard.room;
    // A body close behind the bags is covered from across them, and
    // the body on the other side from across them the other way.
    let north = at - bims::math::vec2(0.0, TILE);
    let south = at + bims::math::vec2(0.0, TILE);
    let far_north = at - bims::math::vec2(0.0, 6.0 * TILE);
    let far_south = at + bims::math::vec2(0.0, 6.0 * TILE);
    assert!(crew_room.covered_for_probe(north, far_south));
    assert!(crew_room.covered_for_probe(south, far_north));
    assert!(
        !crew_room.covered_for_probe(north, far_north),
        "not from behind"
    );
    // The residents' room has the same tile marked, in its own units.
    let theirs = &world.residents.as_ref().unwrap().aboard.room;
    assert_eq!(theirs.laid_cover().len(), 1);
    let rect = theirs.laid_cover()[0];
    let mid = rect.center();
    let t = TILE;
    assert!(theirs.covered_for_probe(
        mid - bims::math::vec2(0.0, t),
        mid + bims::math::vec2(0.0, 6.0 * t)
    ));
    assert!(theirs.covered_for_probe(
        mid + bims::math::vec2(0.0, t),
        mid - bims::math::vec2(0.0, 6.0 * t)
    ));
    // A relayout is a fresh `Sight`, and the cover is said again.
    let before = world.aboard.room.laid_cover().to_vec();
    world.relayout_room_for_probe();
    assert_eq!(world.aboard.room.laid_cover(), &before[..]);
    assert!(world.aboard.room.covered_for_probe(north, far_south));
    // And the station's deck's sandbags are lost when the rooms unjoin;
    // the ship's own keep.
    let ship_tile = {
        let here = world.aboard.gangway.unwrap();
        let here = bims::math::vec2(here.x as f32, here.y as f32);
        world.aboard.room.put_for_probe(0, here);
        world.step(&[]);
        tile_near(&world, 0, Kit::Sandbag)
    };
    let (_, ship_id) = deploy_now(&mut world, 0, Kit::Sandbag, ship_tile);
    assert_eq!(world.deployable(ship_id).unwrap().deck, Deck::Ship);
    assert_eq!(world.deployables.len(), 2);
    world.undock_for_probe();
    assert_eq!(world.deployables.len(), 1);
    assert_eq!(world.deployables[0].id, ship_id);
    assert_eq!(
        world.aboard.room.laid_cover().len(),
        1,
        "on the ship's own room"
    );
    let station = world.residents.as_ref().unwrap().station;
    world.dock_at_for_probe(station);
    assert_eq!(world.deployables.len(), 1, "and across a docking");
    assert_eq!(world.aboard.room.laid_cover().len(), 1);
    assert_eq!(
        world
            .residents
            .as_ref()
            .unwrap()
            .aboard
            .room
            .laid_cover()
            .len(),
        1,
        "mirrored onto the residents' deck"
    );
}

#[test]
fn sandbags_take_the_bolts_they_stop_and_are_gone_at_nothing() {
    let mut world = fight();
    let tile = tile_near(&world, 0, Kit::Sandbag);
    let (_, id) = deploy_now(&mut world, 0, Kit::Sandbag, tile);
    assert_eq!(world.deployable(id).unwrap().health, deploy::SANDBAG_HEALTH);
    let mut events = Vec::new();
    for _ in 0..3 {
        // Two bolts ducked behind the bags, said by the room: the bags
        // took them.
        world
            .aboard
            .room
            .cover_hit_for_probe((tile.0, tile.1), 90.0);
        world.settle_deployables(&mut events);
    }
    assert!(
        events
            .iter()
            .any(|e| matches!(e, WorldEvent::DeployableLost { kind } if *kind == DeployKind::Sandbags.code())),
        "{events:?}"
    );
    assert!(world.deployable(id).is_none());
    assert!(world.aboard.room.laid_cover().is_empty());
}

// --- D: the sentry ------------------------------------------------------------

/// A sentry laid beside James in the staged fight, at the third level,
/// with resident 0 alone a few tiles down the corridor.
fn sentry_fight() -> (World, u32) {
    let mut world = fight();
    level_up(&mut world, 0, class::SENTRY_LEVEL);
    give_kit(&mut world, 0, Kit::Sentry);
    let tile = tile_near(&world, 0, Kit::Sentry);
    // Patched up through the laying, since the resident shoots.
    let mut events = world.step(&[Command::Deploy {
        slot: 0,
        kit: Kit::Sentry,
        x: tile.0,
        y: tile.1,
    }]);
    assert!(
        !events
            .iter()
            .any(|e| matches!(e, WorldEvent::Refused { .. }))
    );
    for _ in 0..20_000 {
        if events
            .iter()
            .any(|e| matches!(e, WorldEvent::Deployed { .. }))
        {
            let id = world.deployables.iter().map(|d| d.id).max().unwrap();
            arm_resident(&mut world);
            return (world, id);
        }
        world
            .residents
            .as_mut()
            .unwrap()
            .aboard
            .room
            .patch_up_for_probe(0);
        events = world.step(&[]);
    }
    panic!("the sentry was never laid");
}

/// A sentry fires at what it sees and **never runs out** (feature 88):
/// nothing in the game carries ammunition, so there are no shots to
/// count and no refill to walk over for.
#[test]
fn a_sentry_fires_at_the_enemy_it_sees_and_never_runs_out() {
    let (mut world, id) = sentry_fight();
    assert_eq!(world.deployable(id).unwrap().kind, DeployKind::Sentry);
    assert_eq!(world.deployable(id).unwrap().health, deploy::SENTRY_HEALTH);
    assert_eq!(world.sentry_weapon(0), WeaponKind::AutoRifle.basic());
    // James out of the fight: only the sentry is left to shoot, and to be
    // shot at.
    world.aboard.room.knock_out_for_probe(0);
    let before = world.residents.as_ref().unwrap().aboard.room.health(0);
    for _ in 0..3_000 {
        world
            .residents
            .as_mut()
            .unwrap()
            .aboard
            .room
            .patch_up_for_probe(0);
        world.step(&[]);
        assert_eq!(world.aboard.room.sentries().len(), 1);
        if world.residents.as_ref().unwrap().aboard.room.health(0) < before {
            break;
        }
    }
    assert!(
        world.residents.as_ref().unwrap().aboard.room.health(0) < before,
        "the sentry pulled its trigger and the resident was hit"
    );
    // And it is still firing a long while later: there is nothing to run
    // out of. The resident is patched up every step, so a hit landing
    // after a thousand more steps is the sentry still shooting.
    let mut hit_again = false;
    for _ in 0..3_000 {
        let whole = world.residents.as_ref().unwrap().aboard.room.health(0);
        world.step(&[]);
        if world.residents.as_ref().unwrap().aboard.room.health(0) < whole {
            hit_again = true;
            break;
        }
        world
            .residents
            .as_mut()
            .unwrap()
            .aboard
            .room
            .patch_up_for_probe(0);
    }
    assert!(hit_again, "a sentry never runs dry");
}

#[test]
fn a_sentry_is_the_enemy_s_target_and_is_destroyed_at_nothing() {
    let (mut world, id) = sentry_fight();
    // James out of the fight: the sentry is what the resident sees.
    world.aboard.room.knock_out_for_probe(0);
    let mut lost = false;
    for _ in 0..6_000 {
        world
            .residents
            .as_mut()
            .unwrap()
            .aboard
            .room
            .patch_up_for_probe(0);
        let events = world.step(&[]);
        if events
            .iter()
            .any(|e| matches!(e, WorldEvent::DeployableLost { kind } if *kind == DeployKind::Sentry.code()))
        {
            lost = true;
            break;
        }
    }
    assert!(lost, "the resident shot the sentry to nothing");
    assert!(world.deployable(id).is_none());
    assert!(world.aboard.room.sentries().is_empty());
}

#[test]
fn two_runs_of_a_sentry_fight_on_one_seed_are_the_same_fight() {
    let run = || {
        let (mut world, _) = sentry_fight();
        world.aboard.room.knock_out_for_probe(0);
        for _ in 0..600 {
            world.step(&[]);
        }
        world_checksum(&world)
    };
    assert_eq!(run(), run());
}

// --- E: the ten levels --------------------------------------------------------

#[test]
fn the_fixed_levels_lay_sandbags_a_sentry_and_a_mark_two_rifle() {
    let mut world = engineer();
    let tile = tile_near(&world, 0, Kit::Sandbag);
    assert_eq!(world.can_deploy(0, Kit::Sandbag, tile), Ok(()), "level one");
    give_kit(&mut world, 0, Kit::Sentry);
    assert_eq!(
        world.can_deploy(0, Kit::Sentry, tile),
        Err(Refusal::NoSentryYet)
    );
    level_up(&mut world, 0, 3);
    assert_eq!(
        world.can_deploy(0, Kit::Sentry, tile),
        Ok(()),
        "level three"
    );
    assert_eq!(world.sentry_weapon(0).tier, Tier::One);
    level_up(&mut world, 0, 7);
    assert_eq!(world.sentry_weapon(0).tier, Tier::Two, "level seven");
    assert_eq!(world.sentry_weapon(0).kind, WeaponKind::AutoRifle);
}

/// *Reinforced sand* puts fifty on every bag laid, and *site foreman* is
/// the one work factor an engineer has left: feature 88 took *quick
/// hands* off the tree, so the craft factor is one for everybody.
#[test]
fn reinforced_sand_and_site_foreman_are_the_level_two_pick() {
    let mut world = engineer();
    world.step(&[]);
    assert_eq!(world.aboard.room.work_factors_for_probe(0), (1.0, 1.0));
    pick(&mut world, 0, Talent::ReinforcedSand);
    world.step(&[]);
    assert_eq!(
        world.aboard.room.work_factors_for_probe(0),
        (1.0, 1.0),
        "no working step is any faster for it"
    );
    let tile = tile_near(&world, 0, Kit::Sandbag);
    let (_, id) = deploy_now(&mut world, 0, Kit::Sandbag, tile);
    assert_eq!(
        world.deployable(id).unwrap().health,
        deploy::SANDBAG_HEALTH + class::REINFORCED_SAND_HEALTH
    );
    let mut other = engineer();
    pick(&mut other, 0, Talent::SiteForeman);
    other.step(&[]);
    assert_eq!(
        other.aboard.room.work_factors_for_probe(0),
        (1.0, class::SITE_FOREMAN_EFFORT)
    );
    assert_eq!(other.aboard.room.work_factors_for_probe(1), (1.0, 1.0));
    let tile = tile_near(&other, 0, Kit::Sandbag);
    let (_, id) = deploy_now(&mut other, 0, Kit::Sandbag, tile);
    assert_eq!(
        other.deployable(id).unwrap().health,
        deploy::SANDBAG_HEALTH,
        "the other side lays a plain bag"
    );
}

#[test]
fn sandbagger_halves_the_time_and_bulk_bags_lays_two() {
    let mut world = engineer();
    assert_eq!(
        world.deploy_minutes(0, Kit::Sandbag),
        deploy::DEPLOY_SANDBAG_MINUTES
    );
    pick(&mut world, 0, Talent::Sandbagger);
    assert_eq!(
        world.deploy_minutes(0, Kit::Sandbag),
        deploy::DEPLOY_SANDBAG_MINUTES * class::SANDBAGGER_TIME
    );
    let mut other = engineer();
    pick(&mut other, 0, Talent::BulkBags);
    let kits = kits_in_pack(&other, 0, Kit::Sandbag);
    let tile = tile_near(&other, 0, Kit::Sandbag);
    deploy_now(&mut other, 0, Kit::Sandbag, tile);
    assert_eq!(other.deployables.len(), 2, "two tiles from one kit");
    assert_eq!(kits_in_pack(&other, 0, Kit::Sandbag), kits - 1);
    let (a, b) = (other.deployables[0].tile, other.deployables[1].tile);
    assert_eq!(
        (a.0 as i32 - b.0 as i32).abs() + (a.1 as i32 - b.1 as i32).abs(),
        1,
        "adjacent"
    );
}

/// *Armoured sentry* is half again the health and *enhanced optics* ten
/// tiles of range (feature 88), which is the sentry's own `Skill` and not
/// its weapon: the gun in the turret is the same gun.
#[test]
fn armoured_sentry_and_enhanced_optics_are_the_sentry_s_numbers() {
    let mut world = engineer();
    pick(&mut world, 0, Talent::ArmouredSentry);
    give_kit(&mut world, 0, Kit::Sentry);
    let tile = tile_near(&world, 0, Kit::Sentry);
    let (_, id) = deploy_now(&mut world, 0, Kit::Sentry, tile);
    let laid = world.deployable(id).unwrap();
    assert_eq!(
        laid.health,
        deploy::SENTRY_HEALTH * class::ARMOURED_SENTRY_HEALTH
    );
    assert_eq!(
        world.sentry_skill(0),
        bims::combat::Skill::NONE,
        "no optics"
    );
    let mut other = engineer();
    pick(&mut other, 0, Talent::EnhancedOptics);
    give_kit(&mut other, 0, Kit::Sentry);
    let tile = tile_near(&other, 0, Kit::Sentry);
    let (_, id) = deploy_now(&mut other, 0, Kit::Sentry, tile);
    let laid = other.deployable(id).unwrap();
    assert_eq!(
        laid.health,
        deploy::SENTRY_HEALTH,
        "the health is untouched"
    );
    let skill = other.sentry_skill(0);
    assert_eq!(skill.range, class::ENHANCED_OPTICS_RANGE);
    assert_eq!(skill.fire_rate, 1.0);
    assert_eq!(skill.damage, 1.0);
    // And the room is handed it, so the reach the turret aims with is ten
    // tiles longer than the gun's own.
    other.step(&[]);
    let sentry = other.aboard.room.sentries()[0];
    assert_eq!(
        sentry.skill.stats(sentry.weapon).range,
        sentry.weapon.stats().range + class::ENHANCED_OPTICS_RANGE
    );
}

#[test]
fn the_armourer_repairs_a_piece_at_the_workbench_for_a_metal() {
    use bims::combat::{ArmourKind, Piece};
    let mut world = simulation_world(
        shipdesign::fixture::playtest_ship(),
        crate::data::SIMULATION_MONEY,
        1,
    );
    assert_eq!(world.set_class(0, Class::Engineer), Ok(()));
    // A damaged helm on the bench, put there straight.
    let mut piece = Piece::new(900, ArmourKind::BasicHelm, Tier::One);
    let full = piece.stats().health;
    piece.health = full - 25.0;
    let bench = world.workbench().expect("the flyer has a workbench");
    assert_eq!(
        world.bench.takes(Item::Armour(piece)),
        Ok(0),
        "a damaged piece goes on"
    );
    world.bench.slots[0] = Some(Item::Armour(piece));
    // Beside it, since the repair is begun from beside the bench.
    let spot = world.aboard.room.bench_spot_for_probe(bench);
    world.aboard.room.put_for_probe(0, spot);
    let events = world.step(&[Command::Repair { slot: 0 }]);
    assert!(
        refused_with(&events, Refusal::NoTalent),
        "without the talent"
    );
    pick(&mut world, 0, Talent::Armourer);
    world.ship.design.cargo[ResourceId::Vegetable as usize] += 5;
    world.on_ship_changed();
    let money = world.money;
    let events = world.step(&[Command::Repair { slot: 0 }]);
    assert!(
        !events
            .iter()
            .any(|e| matches!(e, WorldEvent::Refused { .. })),
        "{events:?}"
    );
    assert_eq!(world.bench.repair, Some(0));
    assert_eq!(world.money, money - deploy::ARMOUR_REPAIR_COST);
    // Only the engineer works it; Kate is never sent.
    let mut done = false;
    for _ in 0..60 * 60 * 6 {
        let events = world.step(&[]);
        if events
            .iter()
            .any(|e| matches!(e, WorldEvent::Repaired { .. }))
        {
            done = true;
            break;
        }
    }
    assert!(done, "the session was worked");
    assert_eq!(world.bench.repair, None);
    assert!(world.bench.slots[0].is_none());
    let Some(Item::Armour(out)) = world.bench.slots[Workbench::OUT] else {
        panic!("the piece is in the output slot");
    };
    assert_eq!(out.id, 900);
    assert_eq!(out.health, full - 25.0 + class::ARMOUR_REPAIR_HEALTH);
}

/// *Higher quality armour* (feature 88) is the engineer's own worn
/// pieces': a point added to what each stops, and five per cent more
/// health said as the drain's reciprocal, which is the tank's own
/// mechanism.
#[test]
fn higher_quality_armour_adds_protection_and_health_to_what_he_wears() {
    let mut world = engineer();
    let plain = world.skill_of(0);
    assert_eq!(plain.armour_protection_add, 0.0);
    assert_eq!(plain.armour_drain, 1.0);
    pick(&mut world, 0, Talent::BetterArmour);
    let skill = world.skill_of(0);
    assert_eq!(
        skill.armour_protection_add,
        class::BETTER_ARMOUR_PROTECTION,
        "a point on the protection"
    );
    assert!(
        (skill.armour_drain - 1.0 / class::BETTER_ARMOUR_HEALTH).abs() < 1e-6,
        "five per cent more health, as the drain: {}",
        skill.armour_drain
    );
    // The multiplier is untouched, so it stacks with a tank's *plated*
    // rather than replacing it.
    assert_eq!(skill.armour_protection, 1.0);
    // And nobody else's armour is any better for it.
    assert_eq!(world.skill_of(1).armour_protection_add, 0.0);
}

#[test]
fn dug_in_is_handed_to_the_room_and_quick_build_halves_the_sentry_s_time() {
    let mut world = engineer();
    pick(&mut world, 0, Talent::DugIn);
    give_kit(&mut world, 0, Kit::Sentry);
    let tile = tile_near(&world, 0, Kit::Sentry);
    deploy_now(&mut world, 0, Kit::Sentry, tile);
    world.step(&[]);
    assert!(world.aboard.room.sentries()[0].dug_in);
    let mut other = engineer();
    assert_eq!(
        other.deploy_minutes(0, Kit::Sentry),
        deploy::DEPLOY_SENTRY_MINUTES
    );
    pick(&mut other, 0, Talent::QuickBuild);
    assert_eq!(
        other.deploy_minutes(0, Kit::Sentry),
        deploy::DEPLOY_SENTRY_MINUTES * class::QUICK_BUILD_TIME
    );
    give_kit(&mut other, 0, Kit::Sentry);
    let tile = tile_near(&other, 0, Kit::Sentry);
    deploy_now(&mut other, 0, Kit::Sentry, tile);
    other.step(&[]);
    assert!(!other.aboard.room.sentries()[0].dug_in);
}

/// *Extra bags* is one more sandbag charge (feature 88) — so the pack
/// fills back up to four rather than three — and nothing comes back off a
/// destroyed sentry: its charge returns on the cooldown like any other.
#[test]
fn extra_bags_is_a_fourth_charge_and_steady_hands_keep_at_a_deploy() {
    let mut world = engineer();
    assert_eq!(
        world.kit_charges(0, Kit::Sandbag),
        deploy::SANDBAG_CHARGES,
        "three to start"
    );
    pick(&mut world, 0, Talent::ExtraBags);
    assert_eq!(
        world.kit_charges(0, Kit::Sandbag),
        deploy::SANDBAG_CHARGES + class::EXTRA_BAGS_CHARGES
    );
    // *Extra bags* is the ninth level's pick, so the sentry's own charge
    // is long since learnt by the time it is taken.
    assert_eq!(world.kit_charges(0, Kit::Sentry), deploy::SENTRY_CHARGES);
    // The pack fills back up to the fourth, one cooldown at a time.
    assert_eq!(kits_in_pack(&world, 0, Kit::Sandbag), 3);
    run_for_seconds(&mut world, deploy::SANDBAG_COOLDOWN + 1.0);
    assert_eq!(kits_in_pack(&world, 0, Kit::Sandbag), 4);
    // A destroyed sentry gives nothing back.
    drop_kits(&mut world, 0, Kit::Sentry);
    give_kit(&mut world, 0, Kit::Sentry);
    let tile = tile_near(&world, 0, Kit::Sentry);
    let (_, id) = deploy_now(&mut world, 0, Kit::Sentry, tile);
    assert_eq!(kits_in_pack(&world, 0, Kit::Sentry), 0);
    world.aboard.room.sentry_hit_for_probe(id, 1_000.0);
    let mut events = Vec::new();
    world.settle_deployables(&mut events);
    assert!(
        events
            .iter()
            .any(|e| matches!(e, WorldEvent::DeployableLost { .. }))
    );
    assert_eq!(
        kits_in_pack(&world, 0, Kit::Sentry),
        0,
        "nothing comes back off the wreck"
    );

    let mut other = engineer();
    pick(&mut other, 0, Talent::SteadyHands);
    let tile = tile_near(&other, 0, Kit::Sandbag);
    other.step(&[Command::Deploy {
        slot: 0,
        kit: Kit::Sandbag,
        x: tile.0,
        y: tile.1,
    }]);
    other.step(&[]);
    assert!(other.aboard.room.is_deploying(0));
    other.aboard.room.wound(0, bims::health::Part::Body, 5.0);
    assert!(other.aboard.room.is_deploying(0), "kept at it");
}

/// The tenth level is two sentry charges or one tier-three sniper
/// (feature 88), and the charges are the world limit: one laid over it
/// destroys the engineer's oldest rather than being refused.
#[test]
fn second_sentry_is_two_charges_and_mark_three_is_a_tier_three_sniper() {
    let mut world = engineer();
    assert_eq!(
        world.sentry_limit(0),
        0,
        "the charge itself is the third level's"
    );
    level_up(&mut world, 0, class::SENTRY_LEVEL);
    assert_eq!(world.sentry_limit(0), deploy::SENTRY_CHARGES);
    pick(&mut world, 0, Talent::SecondSentry);
    assert_eq!(world.sentry_limit(0), class::SECOND_SENTRY_CHARGES);
    assert_eq!(
        world.kit_charges(0, Kit::Sentry),
        class::SECOND_SENTRY_CHARGES,
        "the limit is the charges"
    );
    assert_eq!(
        world.sentry_weapon(0),
        WeaponKind::AutoRifle.at(Tier::Two),
        "mark II at the tenth"
    );
    drop_kits(&mut world, 0, Kit::Sentry);
    give_kit(&mut world, 0, Kit::Sentry);
    give_kit(&mut world, 0, Kit::Sentry);
    give_kit(&mut world, 0, Kit::Sentry);
    // What the box at the foot of the screen counts (feature 80): the
    // kits in the pack, whatever is standing.
    assert_eq!(world.kits_of(0, Kit::Sentry), 3);
    assert_eq!(world.sentries_left(0), 3);
    let tile = tile_near(&world, 0, Kit::Sentry);
    let (_, first) = deploy_now(&mut world, 0, Kit::Sentry, tile);
    let tile = tile_near(&world, 0, Kit::Sentry);
    let (_, second) = deploy_now(&mut world, 0, Kit::Sentry, tile);
    assert_eq!(world.sentries_of(0), 2, "both standing");
    // The third is allowed and the oldest goes.
    let tile = tile_near(&world, 0, Kit::Sentry);
    assert_eq!(world.can_deploy(0, Kit::Sentry, tile), Ok(()));
    let (_, third) = deploy_now(&mut world, 0, Kit::Sentry, tile);
    assert_eq!(world.sentries_of(0), 2, "still two");
    assert!(
        world.deployable(first).is_none(),
        "the oldest was destroyed"
    );
    assert!(world.deployable(second).is_some());
    assert!(world.deployable(third).is_some());

    let mut other = engineer();
    pick(&mut other, 0, Talent::SentryMarkThree);
    assert_eq!(
        other.sentry_weapon(0),
        WeaponKind::SniperRifle.at(Tier::Three)
    );
    assert_eq!(other.sentry_limit(0), deploy::SENTRY_CHARGES, "one charge");
    let skill = other.sentry_skill(0);
    assert_eq!(skill.fire_rate, class::SENTRY_MARK_THREE_FIRE_RATE);
    assert_eq!(skill.damage, class::SENTRY_MARK_THREE_DAMAGE);
    assert_eq!(skill.range, 0.0, "no optics of its own");
    // Double the rate and a fifth more damage than the tier-three sniper.
    let plain = WeaponKind::SniperRifle.at(Tier::Three).stats();
    let theirs = skill.stats(other.sentry_weapon(0));
    assert!((theirs.fire_rate - plain.fire_rate * 2.0).abs() < 1e-4);
    assert!((theirs.damage - plain.damage * 1.2).abs() < 1e-3);
    assert!((theirs.damage_far - plain.damage_far * 1.2).abs() < 1e-3);
}

/// The charges and their cooldowns (feature 88): an engineer that spends
/// a bag gets it back three quarters of a minute later, and its sentry
/// after a minute, out of nothing and with nobody walking for it.
#[test]
fn a_spent_charge_comes_back_on_its_cooldown() {
    let mut world = engineer();
    assert_eq!(world.kit_charges(0, Kit::Sandbag), deploy::SANDBAG_CHARGES);
    assert_eq!(
        world.kit_charges(0, Kit::Sentry),
        0,
        "the third level first"
    );
    assert_eq!(
        world.kit_charges(1, Kit::Sandbag),
        0,
        "and nobody else has charges at all"
    );
    // At its charges nothing is running.
    world.step(&[]);
    assert_eq!(world.kit_cooldown_left(0, Kit::Sandbag), 0.0);
    // One bag laid: the cooldown starts and the kit is back when it ends.
    let tile = tile_near(&world, 0, Kit::Sandbag);
    deploy_now(&mut world, 0, Kit::Sandbag, tile);
    assert_eq!(
        kits_in_pack(&world, 0, Kit::Sandbag),
        deploy::SANDBAG_CHARGES as usize - 1
    );
    world.step(&[]);
    let left = world.kit_cooldown_left(0, Kit::Sandbag);
    assert!(
        left > 0.0 && left <= deploy::SANDBAG_COOLDOWN,
        "running: {left}"
    );
    run_for_seconds(&mut world, deploy::SANDBAG_COOLDOWN - 2.0);
    assert!(
        kits_in_pack(&world, 0, Kit::Sandbag) < deploy::SANDBAG_CHARGES as usize,
        "not yet"
    );
    run_for_seconds(&mut world, 3.0);
    assert_eq!(
        kits_in_pack(&world, 0, Kit::Sandbag),
        deploy::SANDBAG_CHARGES as usize,
        "back"
    );
    assert_eq!(world.kit_cooldown_left(0, Kit::Sandbag), 0.0, "and at rest");
    // The sentry's charge only exists from the third level, and it is
    // the slower of the two.
    level_up(&mut world, 0, class::SENTRY_LEVEL);
    drop_kits(&mut world, 0, Kit::Sentry);
    world.step(&[]);
    assert_eq!(world.kit_charges(0, Kit::Sentry), deploy::SENTRY_CHARGES);
    run_for_seconds(&mut world, deploy::SENTRY_COOLDOWN - 3.0);
    assert_eq!(kits_in_pack(&world, 0, Kit::Sentry), 0, "slower than a bag");
    run_for_seconds(&mut world, 4.0);
    assert_eq!(kits_in_pack(&world, 0, Kit::Sentry), 1);
    // And it is in the checksum: a charge waiting is a different fight.
    let mut twin = engineer();
    level_up(&mut twin, 0, class::SENTRY_LEVEL);
    twin.step(&[]);
    let tile = tile_near(&twin, 0, Kit::Sandbag);
    deploy_now(&mut twin, 0, Kit::Sandbag, tile);
    let before = world_checksum(&twin);
    run_for_seconds(&mut twin, 5.0);
    assert_ne!(world_checksum(&twin), before, "the timer is hashed");
}

/// Steps the world forward that many seconds of the clock, which is that
/// many game minutes (`time::MINUTES_PER_SECOND`).
fn run_for_seconds(world: &mut World, seconds: f64) {
    let until = world.mission_minutes() + seconds * time::MINUTES_PER_SECOND;
    while world.mission_minutes() < until {
        world.step(&[]);
    }
}
