//! The engineer class (feature 74): `crate::class` and `crate::deploy`.
//! Experience and levels, the picks, the class chosen at the start; the
//! kits laid as sandbags and sentries, what a deploy is refused for, the
//! cover on both rooms, a sentry's fire and its end; and each of the ten
//! levels' talents doing what it says.

use bims::combat::{Item, Tier, WeaponKind};
use economy::trade_price;
use physics::ResourceId;
use shipdesign::fixture::flyer;
use shipdesign::parts::PartKind;
use shipdesign::Rotation;

use crate::class::{self, Class, LEVEL_XP, Progress, Side, Talent};
use crate::deploy::{self, Deck, DeployKind, Kit};
use crate::event::{Refusal, WorldEvent};
use crate::fixture::{REFERENCE_MONEY, simulation_world};
use crate::world::{Command, ShipState, World, Workbench};
use crate::world_checksum;
use crate::Target;

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
            class::pick_at(l).and_then(|(left, right)| {
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
        events
            .iter()
            .any(|e| matches!(e, WorldEvent::TalentPicked { talent: t, .. } if *t == talent.code())),
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
    let (cx, cy) = ((here.x / TILE).floor() as i32, (here.y / TILE).floor() as i32);
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
        !events.iter().any(|e| matches!(e, WorldEvent::Refused { .. })),
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

/// The hostile dock with resident 0 stood a few tiles down the corridor
/// from James, the rest of the garrison down (`stage_fight_for_probe`).
fn fight() -> World {
    let mut world = engineer();
    assert!(world.stage_fight_for_probe());
    let ashore = world.residents.as_mut().unwrap();
    for other in 1..ashore.aboard.count() as usize {
        ashore.aboard.room.kill_for_probe(other);
    }
    world
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
    assert_eq!(events, vec![WorldEvent::LevelUp { who: 0, level: 2 }]);
    events.clear();
    world.award(0, 1, &mut events);
    assert!(events.is_empty(), "a level is said once");
    for level in 3..=class::LEVELS {
        let got = level_up(&mut world, 0, level);
        assert_eq!(
            got,
            vec![WorldEvent::LevelUp {
                who: 0,
                level: level as u32
            }]
        );
    }
    assert_eq!(world.progress_of(0).to_next(), 0, "the top");
    // Slot 1 has no class and learns nothing.
    world.award(1, 1_000, &mut events);
    assert_eq!(world.progress_of(1), Progress::default());
    assert!(!world.has_talent(1, Talent::QuickHands));
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
    world.aboard.room.put_for_probe(1, here + bims::math::vec2(TILE, 0.0));
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
    // And a dead crew member's progress died with it.
    assert_eq!(world.progress_of(1), Progress::default());
}

#[test]
fn an_enemy_down_on_an_unjoined_deck_and_one_seen_by_a_classless_crew_is_nothing() {
    let mut world = fight();
    // Kate, no class, beside James.
    let here = world.aboard.room.bim_pos(0);
    world.aboard.room.put_for_probe(1, here + bims::math::vec2(TILE, 0.0));
    world.step(&[]);
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
    assert_eq!(world.progress_of(0).xp, 0, "unjoined: nobody is in range");
    assert_eq!(world.progress_of(1).xp, 0, "and Kate has no class");
}

#[test]
fn a_site_finished_and_a_kit_laid_are_the_engineer_s_and_a_re_used_kit_is_not() {
    let mut world = engineer();
    let before = world.progress_of(0).xp;
    // A site of James's own, and one Kate finishes beside him.
    let here = world.aboard.room.bim_pos(1);
    world.aboard.room.put_for_probe(1, here);
    let (tx, ty) = tile_near(&world, 0, Kit::Sandbag);
    let (_, p) = world.aboard.design_of(worldgen::math::dvec2(
        (tx as f64 + 0.5) * shipdesign::TILE as f64,
        (ty as f64 + 0.5) * shipdesign::TILE as f64,
    ));
    let origin = (
        (p.x / shipdesign::TILE as f64).floor() as u32,
        (p.y / shipdesign::TILE as f64).floor() as u32,
    );
    world.ship.design.cargo[ResourceId::Metal as usize] += 10;
    let events = world.step(&[Command::PlaceSite {
        slot: 0,
        kind: PartKind::Sandbags,
        origin,
        rotation: Rotation::R0,
    }]);
    assert!(
        events.iter().any(|e| matches!(e, WorldEvent::SitePlaced { .. })),
        "{events:?}"
    );
    let site = world.builds[0].id;
    let mut events = Vec::new();
    world.finish_build(site, 0, &mut events);
    assert!(events.iter().any(|e| matches!(e, WorldEvent::Built { .. })));
    assert_eq!(world.progress_of(0).xp, before + class::XP_BUILT, "his own");
    // Kate's, within his vicinity.
    let (tx2, ty2) = tile_near(&world, 0, Kit::Sandbag);
    let (_, p) = world.aboard.design_of(worldgen::math::dvec2(
        (tx2 as f64 + 0.5) * shipdesign::TILE as f64,
        (ty2 as f64 + 0.5) * shipdesign::TILE as f64,
    ));
    let origin = (
        (p.x / shipdesign::TILE as f64).floor() as u32,
        (p.y / shipdesign::TILE as f64).floor() as u32,
    );
    world.step(&[Command::PlaceSite {
        slot: 0,
        kind: PartKind::Sandbags,
        origin,
        rotation: Rotation::R0,
    }]);
    let site = world.builds[0].id;
    world.finish_build(site, 1, &mut events);
    assert_eq!(world.progress_of(0).xp, before + 2 * class::XP_BUILT, "a crewmate's");
    assert_eq!(world.progress_of(1).xp, 0, "Kate has no class");
    // A kit laid is two more; the same kit packed up and laid again is
    // nothing, since it was re-used.
    let tile = tile_near(&world, 0, Kit::Sandbag);
    let (_, id) = deploy_now(&mut world, 0, Kit::Sandbag, tile);
    assert_eq!(world.progress_of(0).xp, before + 3 * class::XP_BUILT, "laid");
    let events = world.step(&[Command::PackUp { slot: 0, id }]);
    assert!(
        events.iter().any(|e| matches!(e, WorldEvent::PackedUp { who: 0, .. })),
        "{events:?}"
    );
    assert_eq!(world.reused_kits[0], 1);
    let tile = tile_near(&world, 0, Kit::Sandbag);
    deploy_now(&mut world, 0, Kit::Sandbag, tile);
    assert_eq!(world.reused_kits[0], 0);
    assert_eq!(world.progress_of(0).xp, before + 3 * class::XP_BUILT, "re-used");
}

#[test]
fn a_pick_is_refused_for_the_wrong_slot_level_or_a_second_time_and_moves_the_checksum() {
    let mut world = engineer();
    let events = world.step(&[Command::PickTalent {
        slot: 1,
        level: 2,
        side: Side::Left,
    }]);
    assert!(refused_with(&events, Refusal::NotAnEngineer), "no class");
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
    assert!(refused_with(&events, Refusal::NotAPickLevel), "a fixed level");
    assert_eq!(world.progress_of(0).pending_pick(), Some(2));
    let before = world_checksum(&world);
    let events = world.step(&[Command::PickTalent {
        slot: 0,
        level: 2,
        side: Side::Left,
    }]);
    assert!(
        events.iter().any(|e| matches!(
            e,
            WorldEvent::TalentPicked { who: 0, talent } if *talent == Talent::QuickHands.code()
        )),
        "{events:?}"
    );
    assert_ne!(world_checksum(&world), before, "a pick is in the checksum");
    assert_eq!(world.progress_of(0).pending_pick(), None);
    let events = world.step(&[Command::PickTalent {
        slot: 0,
        level: 2,
        side: Side::Right,
    }]);
    assert!(refused_with(&events, Refusal::AlreadyPicked), "never changed");
    assert!(world.has_talent(0, Talent::QuickHands) && !world.has_talent(0, Talent::SiteForeman));
}

// --- B: the class at the start ----------------------------------------------

#[test]
fn an_engineer_sets_out_with_four_kits_and_the_class_locks_at_the_first_undock() {
    let mut world = basic();
    assert_eq!(world.class_of(0), Class::None);
    assert_eq!(kits_in_pack(&world, 0, Kit::Sandbag), 0);
    let events = world.step(&[Command::SetClass {
        slot: 0,
        class: Class::Engineer,
    }]);
    assert!(!events.iter().any(|e| matches!(e, WorldEvent::Refused { .. })));
    assert_eq!(world.class_of(0), Class::Engineer);
    assert_eq!(
        kits_in_pack(&world, 0, Kit::Sandbag),
        deploy::ENGINEER_START_KITS as usize
    );
    assert_eq!(kits_in_pack(&world, 0, Kit::Sentry), 0);
    // Put back to none, the kits come out again.
    world.step(&[Command::SetClass {
        slot: 0,
        class: Class::None,
    }]);
    assert_eq!(kits_in_pack(&world, 0, Kit::Sandbag), 0);
    world.step(&[Command::SetClass {
        slot: 0,
        class: Class::Engineer,
    }]);
    // The kits are priced by the labour rule.
    assert_eq!(trade_price(ResourceId::SandbagKit), 61);
    assert_eq!(trade_price(ResourceId::SentryKit), 885);
    // Off the berth, and the class is fixed.
    world.man_the_helm_for_probe(0);
    let target = Target::Point(world.ship.position().add(worldgen::math::dvec2(5_000.0, 0.0)));
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

// --- C: sandbags --------------------------------------------------------------

#[test]
fn every_reason_a_deploy_is_refused() {
    let mut world = engineer();
    let tile = tile_near(&world, 0, Kit::Sandbag);
    // Slot 1 is nobody's engineer.
    give_kit(&mut world, 1, Kit::Sandbag);
    assert_eq!(world.can_deploy(1, Kit::Sandbag, tile), Err(Refusal::NotAnEngineer));
    // No sentry kit in the pack.
    assert_eq!(world.can_deploy(0, Kit::Sentry, tile), Err(Refusal::NoKit));
    give_kit(&mut world, 0, Kit::Sentry);
    // A sentry before the third level.
    assert_eq!(world.can_deploy(0, Kit::Sentry, tile), Err(Refusal::NoSentryYet));
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
    // The sentry's limit: one standing, and a second refused.
    let tile = tile_near(&world, 0, Kit::Sentry);
    deploy_now(&mut world, 0, Kit::Sentry, tile);
    give_kit(&mut world, 0, Kit::Sentry);
    let tile = tile_near(&world, 0, Kit::Sandbag);
    assert_eq!(world.can_deploy(0, Kit::Sentry, tile), Err(Refusal::SentryLimit));
    // And the command says the same through the seam.
    let events = world.step(&[Command::Deploy {
        slot: 0,
        kit: Kit::Sentry,
        x: tile.0,
        y: tile.1,
    }]);
    assert!(refused_with(&events, Refusal::SentryLimit));
}

#[test]
fn laying_sandbags_takes_its_minutes_and_a_hit_drops_it_with_the_kit_kept() {
    let mut world = engineer();
    let tile = tile_near(&world, 0, Kit::Sandbag);
    let kits = kits_in_pack(&world, 0, Kit::Sandbag);
    let (steps, id) = deploy_now(&mut world, 0, Kit::Sandbag, tile);
    let work = (deploy::DEPLOY_SANDBAG_MINUTES / crate::data::STEP_MINUTES) as u32;
    assert!(steps >= work, "{steps} steps for {work} of work");
    assert!(steps < work + 60 * 20, "{steps} steps: a walk of under twenty minutes");
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
    assert!(!events.iter().any(|e| matches!(e, WorldEvent::Refused { .. })));
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
    assert_eq!(kits_in_pack(&world, 0, Kit::Sandbag), kits - 1, "the kit kept");
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
    assert!(!crew_room.covered_for_probe(north, far_north), "not from behind");
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
    assert_eq!(world.aboard.room.laid_cover().len(), 1, "on the ship's own room");
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
    assert!(!events.iter().any(|e| matches!(e, WorldEvent::Refused { .. })));
    for _ in 0..20_000 {
        if events.iter().any(|e| matches!(e, WorldEvent::Deployed { .. })) {
            let id = world.deployables.iter().map(|d| d.id).max().unwrap();
            return (world, id);
        }
        world.aboard.room.patch_up_for_probe(0);
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

#[test]
fn a_sentry_fires_at_the_enemy_it_sees_runs_dry_and_is_refilled() {
    let (mut world, id) = sentry_fight();
    assert_eq!(world.deployable(id).unwrap().kind, DeployKind::Sentry);
    assert_eq!(world.deployable(id).unwrap().shots, deploy::SENTRY_SHOTS);
    assert_eq!(world.deployable(id).unwrap().health, deploy::SENTRY_HEALTH);
    assert_eq!(world.sentry_weapon(0), WeaponKind::AutoRifle.basic());
    // James out of the fight: only the sentry is left to shoot, and to be
    // shot at.
    world.aboard.room.knock_out_for_probe(0);
    let before = world.residents.as_ref().unwrap().aboard.room.health(0);
    let mut fired = false;
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
        if world.deployable(id).unwrap().shots < deploy::SENTRY_SHOTS {
            fired = true;
        }
        if world.residents.as_ref().unwrap().aboard.room.health(0) < before {
            break;
        }
    }
    assert!(fired, "the sentry pulled its trigger");
    assert!(
        world.residents.as_ref().unwrap().aboard.room.health(0) < before,
        "and the resident was hit"
    );
    // Dry: a shot left, fired, and then nothing.
    world.deployables.iter_mut().find(|d| d.id == id).unwrap().shots = 1;
    for _ in 0..600 {
        world
            .residents
            .as_mut()
            .unwrap()
            .aboard
            .room
            .patch_up_for_probe(0);
        world.step(&[]);
        if world.deployable(id).unwrap().shots == 0 {
            break;
        }
    }
    assert_eq!(world.deployable(id).unwrap().shots, 0, "dry");
    let quiet = world.aboard.room.sentries()[0].trigger;
    for _ in 0..120 {
        world.step(&[]);
    }
    assert_eq!(world.deployable(id).unwrap().shots, 0);
    assert!(
        world.aboard.room.sentries()[0].trigger.burst_left == 0 && quiet.burst_left == 0,
        "holds"
    );
    // Refilled by James beside it, for a metal.
    world.aboard.room.patch_up_for_probe(0);
    let metal = world.free(ResourceId::Metal);
    assert!(metal >= deploy::SENTRY_REFILL_METAL);
    let events = world.step(&[Command::Refill { slot: 0, id }]);
    assert!(
        events.iter().any(|e| matches!(e, WorldEvent::Refilled { who: 0 })),
        "{events:?}"
    );
    assert_eq!(world.deployable(id).unwrap().shots, deploy::SENTRY_SHOTS);
    assert_eq!(world.free(ResourceId::Metal), metal - deploy::SENTRY_REFILL_METAL);
}

#[test]
fn a_sentry_is_the_enemy_s_target_and_is_destroyed_at_nothing() {
    let (mut world, id) = sentry_fight();
    // James out of the fight: the sentry is what the resident sees.
    world.aboard.room.knock_out_for_probe(0);
    world.deployables.iter_mut().find(|d| d.id == id).unwrap().shots = 0;
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
    assert_eq!(world.can_deploy(0, Kit::Sentry, tile), Err(Refusal::NoSentryYet));
    level_up(&mut world, 0, 3);
    assert_eq!(world.can_deploy(0, Kit::Sentry, tile), Ok(()), "level three");
    assert_eq!(world.sentry_weapon(0).tier, Tier::One);
    level_up(&mut world, 0, 7);
    assert_eq!(world.sentry_weapon(0).tier, Tier::Two, "level seven");
    assert_eq!(world.sentry_weapon(0).kind, WeaponKind::AutoRifle);
}

#[test]
fn quick_hands_and_site_foreman_are_the_work_factors() {
    let mut world = engineer();
    world.step(&[]);
    assert_eq!(world.aboard.room.work_factors_for_probe(0), (1.0, 1.0));
    pick(&mut world, 0, Talent::QuickHands);
    world.step(&[]);
    assert_eq!(
        world.aboard.room.work_factors_for_probe(0),
        (class::QUICK_HANDS_EFFORT, 1.0)
    );
    let mut other = engineer();
    pick(&mut other, 0, Talent::SiteForeman);
    other.step(&[]);
    assert_eq!(
        other.aboard.room.work_factors_for_probe(0),
        (1.0, class::SITE_FOREMAN_EFFORT)
    );
    assert_eq!(other.aboard.room.work_factors_for_probe(1), (1.0, 1.0));
}

#[test]
fn sandbagger_halves_the_time_and_bulk_bags_lays_two() {
    let mut world = engineer();
    assert_eq!(world.deploy_minutes(0, Kit::Sandbag), deploy::DEPLOY_SANDBAG_MINUTES);
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

#[test]
fn armoured_sentry_and_deep_magazine_are_the_sentry_s_numbers() {
    let mut world = engineer();
    pick(&mut world, 0, Talent::ArmouredSentry);
    give_kit(&mut world, 0, Kit::Sentry);
    let tile = tile_near(&world, 0, Kit::Sentry);
    let (_, id) = deploy_now(&mut world, 0, Kit::Sentry, tile);
    let laid = world.deployable(id).unwrap();
    assert_eq!(laid.health, deploy::SENTRY_HEALTH * class::ARMOURED_SENTRY_HEALTH);
    assert_eq!(laid.shots, deploy::SENTRY_SHOTS);
    let mut other = engineer();
    pick(&mut other, 0, Talent::DeepMagazine);
    give_kit(&mut other, 0, Kit::Sentry);
    let tile = tile_near(&other, 0, Kit::Sentry);
    let (_, id) = deploy_now(&mut other, 0, Kit::Sentry, tile);
    let laid = other.deployable(id).unwrap();
    assert_eq!(laid.health, deploy::SENTRY_HEALTH);
    assert_eq!(
        laid.shots,
        (deploy::SENTRY_SHOTS as f32 * class::DEEP_MAGAZINE_SHOTS) as u32
    );
}

#[test]
fn the_armourer_repairs_a_piece_at_the_workbench_for_a_metal() {
    use bims::combat::{ArmourKind, Piece};
    let mut world = engineer();
    // A damaged helm on the bench, put there straight.
    let mut piece = Piece::new(900, ArmourKind::BasicHelm, Tier::One);
    let full = piece.stats().health;
    piece.health = full - 25.0;
    let bench = world.workbench().expect("the flyer has a workbench");
    assert_eq!(world.bench.takes(Item::Armour(piece)), Ok(0), "a damaged piece goes on");
    world.bench.slots[0] = Some(Item::Armour(piece));
    // Beside it, since the repair is begun from beside the bench.
    let spot = world.aboard.room.bench_spot_for_probe(bench);
    world.aboard.room.put_for_probe(0, spot);
    let events = world.step(&[Command::Repair { slot: 0 }]);
    assert!(refused_with(&events, Refusal::NoTalent), "without the talent");
    pick(&mut world, 0, Talent::Armourer);
    let metal = world.free(ResourceId::Metal);
    let events = world.step(&[Command::Repair { slot: 0 }]);
    assert!(
        !events.iter().any(|e| matches!(e, WorldEvent::Refused { .. })),
        "{events:?}"
    );
    assert_eq!(world.bench.repair, Some(0));
    assert_eq!(world.free(ResourceId::Metal), metal - deploy::ARMOUR_REPAIR_METAL);
    // Only the engineer works it; Kate is never sent.
    let mut done = false;
    for _ in 0..60 * 60 * 6 {
        let events = world.step(&[]);
        assert!(!world.aboard.room.is_at_work_for_probe(1, deploy::REPAIR_ORDER));
        if events.iter().any(|e| matches!(e, WorldEvent::Repaired { .. })) {
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
    assert_eq!(out.health, full - 25.0 + class::ARMOUR_REPAIR_PER_METAL);
}

#[test]
fn field_refit_refills_for_nothing() {
    let mut world = engineer();
    pick(&mut world, 0, Talent::FieldRefit);
    give_kit(&mut world, 0, Kit::Sentry);
    let tile = tile_near(&world, 0, Kit::Sentry);
    let (_, id) = deploy_now(&mut world, 0, Kit::Sentry, tile);
    world.deployables.iter_mut().find(|d| d.id == id).unwrap().shots = 3;
    world.ship.design.cargo[ResourceId::Metal as usize] = 0;
    world.on_ship_changed();
    let events = world.step(&[Command::Refill { slot: 0, id }]);
    assert!(
        events.iter().any(|e| matches!(e, WorldEvent::Refilled { who: 0 })),
        "{events:?}"
    );
    assert_eq!(world.deployable(id).unwrap().shots, deploy::SENTRY_SHOTS);
    assert_eq!(world.free(ResourceId::Metal), 0);
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
    assert_eq!(other.deploy_minutes(0, Kit::Sentry), deploy::DEPLOY_SENTRY_MINUTES);
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

#[test]
fn salvage_returns_a_destroyed_sentry_s_kit_and_steady_hands_keep_at_a_deploy() {
    let mut world = engineer();
    pick(&mut world, 0, Talent::Salvage);
    give_kit(&mut world, 0, Kit::Sentry);
    let tile = tile_near(&world, 0, Kit::Sentry);
    let (_, id) = deploy_now(&mut world, 0, Kit::Sentry, tile);
    assert_eq!(kits_in_pack(&world, 0, Kit::Sentry), 0);
    world.aboard.room.sentry_hit_for_probe(id, 1_000.0);
    let mut events = Vec::new();
    world.settle_deployables(&mut events);
    assert!(events.iter().any(|e| matches!(e, WorldEvent::DeployableLost { .. })));
    assert_eq!(kits_in_pack(&world, 0, Kit::Sentry), 1, "the kit came back");
    assert_eq!(world.reused_kits[0], 1, "as a re-used one");

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

#[test]
fn second_sentry_is_two_and_mark_three_is_the_tier_three_rifle() {
    let mut world = engineer();
    assert_eq!(world.sentry_limit(0), 1);
    pick(&mut world, 0, Talent::SecondSentry);
    assert_eq!(world.sentry_limit(0), 2);
    assert_eq!(world.sentry_weapon(0).tier, Tier::Two, "mark II at the tenth");
    give_kit(&mut world, 0, Kit::Sentry);
    give_kit(&mut world, 0, Kit::Sentry);
    give_kit(&mut world, 0, Kit::Sentry);
    let tile = tile_near(&world, 0, Kit::Sentry);
    deploy_now(&mut world, 0, Kit::Sentry, tile);
    let tile = tile_near(&world, 0, Kit::Sentry);
    deploy_now(&mut world, 0, Kit::Sentry, tile);
    let tile = tile_near(&world, 0, Kit::Sentry);
    assert_eq!(world.can_deploy(0, Kit::Sentry, tile), Err(Refusal::SentryLimit));
    let mut other = engineer();
    pick(&mut other, 0, Talent::SentryMarkThree);
    assert_eq!(other.sentry_weapon(0), WeaponKind::AutoRifle.at(Tier::Three));
    assert_eq!(other.sentry_limit(0), 1);
}
