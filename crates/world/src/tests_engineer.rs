//! The engineer class (feature 74, reworked in task 127): `crate::class`,
//! `crate::deploy` and `crate::engineer`. The ranked kit — sixteen
//! levels, a skill point a level, Q EMP, C Healing Sentry, E Sandbags and
//! R the sentry — the charges that are counters and never kits, what a
//! deploy and a throw are refused for, the cover on both rooms, the
//! machines stunned, the crew healed, the sentry's fire, its time and its
//! end; and experience, which is every class's alike (task 119).

use bims::combat::{Hit, Tier, WeaponKind};
use bims::droid::{Droid, DroidBody, DroidKind, DroidPart};
use bims::health::{MAX_HEALTH, Part};
use physics::ResourceId;
use shipdesign::fixture::flyer;

use crate::armour::LootSource;
use crate::class::{self, Charge, Class, Progress};
use crate::data;
use crate::deploy::{Deck, DeployKind};
use crate::event::{Refusal, WorldEvent};
use crate::fixture::{REFERENCE_MONEY, simulation_world};
use crate::run::Phase;
use crate::world::{Command, ShipState, World};
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

fn refused(events: &[WorldEvent]) -> bool {
    events
        .iter()
        .any(|e| matches!(e, WorldEvent::Refused { .. }))
}

/// Straight to a level, off the engineer's own table.
fn level_up(world: &mut World, who: usize, level: u8) -> Vec<WorldEvent> {
    let mut events = Vec::new();
    let want = class::LEVEL_XP[level as usize - 1];
    let have = world.progress_of(who as u32).xp;
    world.award(who, want.saturating_sub(have), &mut events);
    assert_eq!(world.level_of(who as u32), level);
    events
}

/// The top level and exactly these ranks — Q, C, E and R — with the
/// charges they give in hand.
fn ranks(world: &mut World, who: u32, want: [u8; 4]) {
    if world.level_of(who) < class::LEVELS {
        level_up(world, who as usize, class::LEVELS);
    }
    world.set_ranks_for_probe(who, want);
    for (slot, &rank) in want.iter().enumerate() {
        assert_eq!(world.rank_of(who, slot as u8), rank);
    }
}

/// [`engineer`] at the top level with every slot at `rank`.
fn engineer_at(rank: u8) -> World {
    let mut world = engineer();
    ranks(&mut world, 0, [rank; 4]);
    world
}

/// The nearest tile to `who` that `kind` could be laid on, in room tiles.
fn tile_near(world: &World, who: u32, kind: DeployKind) -> (i32, i32) {
    tiles_near(world, who, kind)
        .into_iter()
        .next()
        .expect("a free tile near the Bim")
}

/// Every tile within five of `who` that `kind` could be laid on, nearest
/// ring first.
fn tiles_near(world: &World, who: u32, kind: DeployKind) -> Vec<(i32, i32)> {
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
        .filter(|&t| world.can_deploy(who, kind, t).is_ok())
        .collect()
}

fn centre(tile: (i32, i32)) -> bims::math::Vec2 {
    bims::math::vec2((tile.0 as f32 + 0.5) * TILE, (tile.1 as f32 + 0.5) * TILE)
}

/// The command that lays `kind` on `tile`.
fn lay(who: u32, kind: DeployKind, tile: (i32, i32)) -> Command {
    match kind {
        DeployKind::Sentry => Command::Sentry { slot: who, tile },
        kind => Command::Deploy {
            slot: who,
            kind,
            x: tile.0,
            y: tile.1,
        },
    }
}

/// Lay `kind` and run until it is down. The steps it took, and the id.
fn deploy_now(world: &mut World, who: u32, kind: DeployKind, tile: (i32, i32)) -> (u32, u32) {
    deploy_now_keeping(world, who, kind, tile, |_| {})
}

/// [`deploy_now`] with something done to the world every step of it.
fn deploy_now_keeping(
    world: &mut World,
    who: u32,
    kind: DeployKind,
    tile: (i32, i32),
    mut each: impl FnMut(&mut World),
) -> (u32, u32) {
    let mut events = world.step(&[lay(who, kind, tile)]);
    assert!(!refused(&events), "{events:?}");
    for step in 1..20_000u32 {
        if events
            .iter()
            .any(|e| matches!(e, WorldEvent::Deployed { who: w, .. } if *w == who))
        {
            let id = world.deployables.iter().map(|d| d.id).max().unwrap();
            return (step, id);
        }
        each(world);
        events = world.step(&[]);
    }
    panic!("{kind:?} was never laid");
}

/// The machines' dock with one `kind` of them stood a few tiles down the
/// corridor from James (`stage_droid_fight_for_probe`), held where it is
/// put and firing nothing: a hit drops a deploy.
fn fight_with(kind: DroidKind) -> World {
    let mut world = engineer();
    // A wave still to come, so the staged machine going down is not the
    // site cleared and the deck frozen under a test that is not done.
    world.set_droid_waves_for_probe(2);
    assert!(world.stage_droid_fight_for_probe(kind, None));
    for _ in 0..3 {
        world.step(&[]);
    }
    world
}

fn fight() -> World {
    fight_with(DroidKind::Trooper)
}

/// The staged machine let go and a pistol in its arm: it fights.
fn arm_machine(world: &mut World) {
    let droid = machine_mut(world, 0);
    droid.posing = false;
    droid.weapon = WeaponKind::LaserPistol.basic();
}

/// Machine `i` of the station's room, to change by hand.
fn machine_mut(world: &mut World, i: usize) -> &mut Droid {
    world
        .residents
        .as_mut()
        .unwrap()
        .aboard
        .room
        .droid_mut_for_probe(i)
        .expect("the staged machine")
}

fn machine(world: &World, i: usize) -> &Droid {
    world
        .residents
        .as_ref()
        .unwrap()
        .aboard
        .room
        .droid(i)
        .expect("the staged machine")
}

/// What is left of machine `i`, its four parts added up.
fn machine_health(world: &World, i: usize) -> f32 {
    let droid = machine(world, i);
    DroidPart::ALL.iter().map(|&p| droid.body.health(p)).sum()
}

/// The staged machine whole again — a wreck stood up again with it: the
/// test wants a target, not a fight won.
fn mend_machine(world: &mut World) {
    let droid = machine_mut(world, 0);
    droid.body = DroidBody::new(droid.kind, droid.tier);
    droid.destroyed = false;
}

/// Where machine `i` stands in the crew's room.
fn machine_on_deck(world: &World, i: usize) -> bims::math::Vec2 {
    let bims = world.residents.as_ref().unwrap().aboard.room.crew_count();
    world
        .body_position(LootSource::Resident(bims + i as u32))
        .expect("the machine is on the joined deck")
}

/// Another machine of `kind` stood `offset` from the first in the
/// station's own units, posing, and the index it has.
fn another_machine(world: &mut World, kind: DroidKind, offset: bims::math::Vec2) -> usize {
    let residents = world.residents.as_mut().unwrap();
    let room = &mut residents.aboard.room;
    let first = room.droid(0).expect("the staged machine");
    let (at, tier) = (first.pos + offset, first.tier);
    let mut droid = Droid::new(kind, tier, 1, 1, at, 0.0, 7);
    droid.posing = true;
    let i = room.droid_count() as usize;
    room.adopt_droids(vec![droid], bims::math::Vec2::ZERO);
    residents.aboard.crew = residents.aboard.room.body_count();
    i
}

/// Steps the world forward that many seconds of the clock, which is that
/// many game minutes (`time::MINUTES_PER_SECOND`).
fn run_for_seconds(world: &mut World, seconds: f64) {
    let until = world.mission_minutes() + seconds * time::MINUTES_PER_SECOND;
    while world.mission_minutes() < until {
        world.step(&[]);
    }
}

// --- the ranked kit ---------------------------------------------------------------

/// **The engineer joins the rank system** (task 127): sixteen levels on
/// the soldier's table, a point a level, and the same gates — Q, C and E
/// rank `n` at level `2n − 1`, R at 6, 9, 12 and 15.
#[test]
fn the_engineer_climbs_sixteen_levels_and_buys_ranks_as_the_soldier_does() {
    let e = Class::Engineer;
    assert!(class::ranked(e));
    assert_eq!(class::LEVELS, 16);
    assert_eq!(class::level_of(3_199), 15);
    assert_eq!(class::level_of(3_200), 16, "level 16 at 3 200");
    for slot in [class::SLOT_Q, class::SLOT_C, class::SLOT_E] {
        for rank in 1..=4 {
            assert_eq!(class::rank_level(e, slot, rank), Some(2 * rank - 1));
        }
    }
    for (rank, want) in [(1, 6), (2, 9), (3, 12), (4, 15)] {
        assert_eq!(class::rank_level(e, class::SLOT_R, rank), Some(want));
    }
    let mut world = engineer();
    assert_eq!(world.points_of(0), 1, "a point at the first level");
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
    assert!(
        refused_with(&rank_up(&mut world, class::SLOT_Q), Refusal::RankLocked),
        "Q's second rank wants the third level"
    );
    assert!(!refused(&rank_up(&mut world, class::SLOT_E)));
    level_up(&mut world, 0, 16);
    for _ in 1..4 {
        assert!(!refused(&rank_up(&mut world, class::SLOT_Q)));
    }
    assert!(refused_with(
        &rank_up(&mut world, class::SLOT_Q),
        Refusal::TopRank
    ));
    assert_eq!(world.rank_of(0, class::SLOT_Q), 4);
    // Every other class buys a rank the same way (the tank since task
    // 139), and a classless crew member none.
    for (other, why) in [
        (Class::Tank, None),
        (Class::Medic, None),
        (Class::None, Some(Refusal::NoClass)),
    ] {
        let mut world = basic();
        assert_eq!(world.set_class(0, other), Ok(()));
        let events = world.step(&[Command::RankUp {
            slot: 0,
            ability_slot: 0,
        }]);
        match why {
            Some(why) => assert!(refused_with(&events, why), "{other:?}"),
            None => assert!(!refused(&events), "{other:?}: {events:?}"),
        }
    }
}

/// The class is chosen before the first undock and sets out with the
/// charges its ranks give it — none at rank nought — as counters.
#[test]
fn an_engineer_sets_out_with_its_ranks_charges_and_the_class_locks_at_the_first_undock() {
    let mut world = basic();
    let events = world.step(&[Command::SetClass {
        slot: 0,
        class: Class::Engineer,
    }]);
    assert!(!refused(&events));
    for charge in [Charge::Sandbag, Charge::HealingSentry, Charge::Emp] {
        assert_eq!(world.charges_of(0, charge), 0, "{charge:?} at rank nought");
    }
    world.step(&[Command::RankUp {
        slot: 0,
        ability_slot: class::SLOT_E as u32,
    }]);
    assert_eq!(
        world.charges_of(0, Charge::Sandbag),
        class::SANDBAG_CHARGES[0],
        "a rank bought puts its charges in hand"
    );
    world.step(&[Command::SetClass {
        slot: 0,
        class: Class::None,
    }]);
    assert_eq!(world.charges_of(0, Charge::Sandbag), 0);
    world.step(&[Command::SetClass {
        slot: 0,
        class: Class::Engineer,
    }]);
    world.undock_for_probe();
    assert!(!matches!(world.ship.state, ShipState::Docked { .. }));
    world.step(&[]);
    assert!(world.undocked_once);
    let events = world.step(&[Command::SetClass {
        slot: 0,
        class: Class::None,
    }]);
    assert!(refused_with(&events, Refusal::ClassLocked));
}

// --- no kits ------------------------------------------------------------------

/// **No kit is left in any crate** (task 127): the sandbag kit, the
/// sentry kit and the grenade are gone from `physics`, their codes left
/// free and nothing renumbered, and nothing the trade prices or lays out
/// names them.
#[test]
fn no_kit_is_a_resource_any_more_and_nothing_was_renumbered() {
    for code in 15..=17 {
        assert_eq!(ResourceId::from_code(code), None, "{code} is left free");
    }
    assert!(
        ResourceId::ALL
            .iter()
            .all(|&id| !(15..=17).contains(&(id as u32)))
    );
    assert_eq!(ResourceId::ALL.len(), 19);
    assert_eq!(ResourceId::CODES, 22);
    assert_eq!(shipdesign::CARGO_SLOTS, ResourceId::CODES);
    assert_eq!(ResourceId::Minigun as u32, 18);
    assert_eq!(ResourceId::ReflectivePlate as u32, 21);
    for id in ResourceId::ALL {
        assert_eq!(ResourceId::from_code(id as u32), Some(id));
    }
    // A charge is a counter and nothing else: no charge is a thing.
    for charge in Charge::ALL {
        assert_eq!(Charge::from_code(charge.code()), Some(charge));
    }
    assert_eq!(Charge::from_code(1), None, "the sentry's charge went");
}

/// **Charges restock as counters on the cooldown**, one at a time, to
/// the rank's charges and no further.
#[test]
fn charges_restock_as_counters_on_their_cooldown() {
    let mut world = engineer_at(1);
    assert_eq!(world.charges_of(0, Charge::Sandbag), 2);
    world.set_charges_for_probe(Charge::Sandbag, 0);
    assert_eq!(world.charges_of(0, Charge::Sandbag), 0);
    let cooldown = world.charge_cooldown(0, Charge::Sandbag);
    assert_eq!(cooldown, class::SANDBAG_COOLDOWN[0]);
    run_for_seconds(&mut world, cooldown - 1.0);
    assert_eq!(world.charges_of(0, Charge::Sandbag), 0, "not yet");
    run_for_seconds(&mut world, 2.0);
    assert_eq!(world.charges_of(0, Charge::Sandbag), 1, "one back");
    run_for_seconds(&mut world, cooldown);
    assert_eq!(world.charges_of(0, Charge::Sandbag), 2);
    run_for_seconds(&mut world, cooldown * 2.0);
    assert_eq!(
        world.charges_of(0, Charge::Sandbag),
        2,
        "never past its charges"
    );
    // The counter is in the checksum.
    let before = world_checksum(&world);
    world.set_charges_held(0, Charge::Sandbag, 1);
    assert_ne!(world_checksum(&world), before);
}

/// **Laying or throwing lowers the counter**: sandbags, a Healing Sentry
/// and an EMP one each.
#[test]
fn laying_or_throwing_lowers_the_counter() {
    let mut world = fight();
    ranks(&mut world, 0, [1, 1, 1, 0]);
    let tile = tile_near(&world, 0, DeployKind::Sandbags);
    deploy_now(&mut world, 0, DeployKind::Sandbags, tile);
    assert_eq!(
        world.charges_of(0, Charge::Sandbag),
        class::SANDBAG_CHARGES[0] - 1
    );
    let tile = tile_near(&world, 0, DeployKind::HealingSentry);
    deploy_now(&mut world, 0, DeployKind::HealingSentry, tile);
    assert_eq!(world.charges_of(0, Charge::HealingSentry), 0);
    let at = machine_on_deck(&world, 0);
    let events = world.step(&[Command::Emp {
        slot: 0,
        x: (at.x / TILE).floor() as i32,
        y: (at.y / TILE).floor() as i32,
    }]);
    assert!(!refused(&events), "{events:?}");
    assert_eq!(world.charges_of(0, Charge::Emp), 0);
}

/// **Pack-up returns a charge, capped at the charges**; a Healing Sentry
/// is packed up the same way, and **laying gives no experience**, fresh
/// charge or reused (task 119's rule, kept).
#[test]
fn pack_up_returns_a_charge_capped_and_laying_is_nobody_s_experience() {
    let mut world = engineer_at(1);
    let xp = world.progress_of(0).xp;
    let full = class::SANDBAG_CHARGES[0];
    let tile = tile_near(&world, 0, DeployKind::Sandbags);
    let (_, id) = deploy_now(&mut world, 0, DeployKind::Sandbags, tile);
    assert_eq!(world.charges_of(0, Charge::Sandbag), full - 1);
    let events = world.step(&[Command::PackUp { slot: 0, id }]);
    assert!(
        events
            .iter()
            .any(|e| matches!(e, WorldEvent::PackedUp { who: 0, .. })),
        "{events:?}"
    );
    assert!(world.deployable(id).is_none());
    assert_eq!(
        world.charges_of(0, Charge::Sandbag),
        full,
        "the charge back"
    );
    // Laid again from the charge got back, and packed up with the counter
    // full: the bags come up, the counter stays at its charges.
    let (_, id) = deploy_now(&mut world, 0, DeployKind::Sandbags, tile);
    world.set_charges_held(0, Charge::Sandbag, full);
    world.step(&[Command::PackUp { slot: 0, id }]);
    assert!(world.deployable(id).is_none());
    assert_eq!(world.charges_of(0, Charge::Sandbag), full, "capped");
    // A Healing Sentry comes back up the same way.
    let tile = tile_near(&world, 0, DeployKind::HealingSentry);
    let (_, id) = deploy_now(&mut world, 0, DeployKind::HealingSentry, tile);
    assert_eq!(world.charges_of(0, Charge::HealingSentry), 0);
    world.step(&[Command::PackUp { slot: 0, id }]);
    assert!(world.deployable(id).is_none());
    assert_eq!(world.charges_of(0, Charge::HealingSentry), 1);
    assert_eq!(world.progress_of(0).xp, xp, "nothing laid is experience");
}

// --- the refusals -------------------------------------------------------------

#[test]
fn every_reason_a_deploy_is_refused() {
    let mut world = engineer();
    let tile = tiles_near(&world, 0, DeployKind::Sandbags)
        .into_iter()
        .next();
    // At rank nought nothing is learnt, so nothing can be laid anywhere.
    assert!(tile.is_none());
    ranks(&mut world, 0, [0, 0, 1, 0]);
    let free = tiles_near(&world, 0, DeployKind::Sandbags);
    let tile = free[0];
    assert_eq!(
        world.can_deploy(0, DeployKind::HealingSentry, tile),
        Err(Refusal::NotLearnt)
    );
    assert_eq!(world.can_lay_sentry(0, tile), Err(Refusal::NotLearnt));
    // Slot 1 is nobody's engineer.
    assert_eq!(
        world.can_deploy(1, DeployKind::Sandbags, tile),
        Err(Refusal::NotAnEngineer)
    );
    // No charge.
    world.set_charges_held(0, Charge::Sandbag, 0);
    assert_eq!(
        world.can_deploy(0, DeployKind::Sandbags, tile),
        Err(Refusal::NoKit)
    );
    world.set_charges_held(0, Charge::Sandbag, 2);
    // A tile that will not take it: off the deck, and one laid on.
    assert_eq!(
        world.can_deploy(0, DeployKind::Sandbags, (-1, -1)),
        Err(Refusal::CantDeployThere)
    );
    deploy_now(&mut world, 0, DeployKind::Sandbags, tile);
    assert_eq!(
        world.can_deploy(0, DeployKind::Sandbags, tile),
        Err(Refusal::CantDeployThere)
    );
    // Downed, nothing at all.
    world.aboard.room.knock_out_for_probe(0);
    world.step(&[]);
    let tile = free[1];
    assert_eq!(
        world.can_deploy(0, DeployKind::Sandbags, tile),
        Err(Refusal::OutOfReach)
    );
}

// --- Q: the EMP -----------------------------------------------------------------

/// **An EMP is thrown the grenade's way** — its range and its fuse — and
/// refused with no charge, while downed, and beyond range. It harms
/// nothing.
#[test]
fn an_emp_has_the_grenade_s_range_and_fuse_and_its_refusals() {
    let mut world = fight();
    let at = machine_on_deck(&world, 0);
    let tile = ((at.x / TILE).floor() as i32, (at.y / TILE).floor() as i32);
    assert_eq!(world.can_throw_emp(0, tile), Err(Refusal::NotLearnt));
    assert_eq!(world.can_throw_emp(1, tile), Err(Refusal::NotAnEngineer));
    ranks(&mut world, 0, [1, 0, 0, 0]);
    assert_eq!(world.can_throw_emp(0, tile), Ok(()));
    world.set_charges_held(0, Charge::Emp, 0);
    assert_eq!(world.can_throw_emp(0, tile), Err(Refusal::NoKit));
    world.set_charges_held(0, Charge::Emp, 1);
    // Beyond the grenade's range, along the corridor from James.
    let here = world.aboard.room.bim_pos(0);
    let far = here + (at - here).normalize_or_zero() * ((class::GRENADE_RANGE + 2.0) * TILE);
    let far = ((far.x / TILE).floor() as i32, (far.y / TILE).floor() as i32);
    assert!(matches!(
        world.can_throw_emp(0, far),
        Err(Refusal::OutOfThrowRange | Refusal::CantThrowThere | Refusal::NoLineToTile)
    ));
    assert_eq!(
        world.can_throw_emp(0, (-50, -50)),
        Err(Refusal::CantThrowThere)
    );
    // Thrown: on the grenade's fuse, with no damage in it (that a burst
    // lands no hit is the room's test, `droid::tests`).
    let events = world.step(&[Command::Emp {
        slot: 0,
        x: tile.0,
        y: tile.1,
    }]);
    assert!(
        events
            .iter()
            .any(|e| matches!(e, WorldEvent::EmpThrown { who: 0 }))
    );
    let thrown = world.aboard.room.grenades()[0];
    assert_eq!(thrown.fuse, class::GRENADE_FUSE);
    assert_eq!(thrown.stun, class::EMP_STUN[0]);
    assert_eq!(thrown.damage, 0.0);
    while !world.aboard.room.grenades().is_empty() {
        world.step(&[]);
    }
    world.step(&[]);
    assert!(machine(&world, 0).is_stunned());
    // Downed, nothing.
    world.set_charges_held(0, Charge::Emp, 1);
    world.aboard.room.knock_out_for_probe(0);
    world.step(&[]);
    assert_eq!(world.can_throw_emp(0, tile), Err(Refusal::OutOfReach));
}

/// **The EMP's numbers by rank**: radius, stun, charges and cooldown.
#[test]
fn an_emp_s_radius_stun_charges_and_cooldown_are_its_rank_s() {
    let want = [
        (2.0, 1.5, 1, 30.0),
        (2.5, 2.0, 2, 30.0),
        (2.5, 2.5, 2, 25.0),
        (3.0, 3.0, 2, 20.0),
    ];
    for (i, (radius, stun, charges, cooldown)) in want.into_iter().enumerate() {
        let rank = i as u8 + 1;
        let mut world = engineer();
        ranks(&mut world, 0, [rank, 0, 0, 0]);
        assert_eq!(world.emp_radius(0), radius, "rank {rank}");
        assert_eq!(world.emp_stun(0), stun, "rank {rank}");
        assert_eq!(world.charges(0, Charge::Emp), charges, "rank {rank}");
        assert_eq!(world.charges_of(0, Charge::Emp), charges, "in hand");
        assert_eq!(
            world.charge_cooldown(0, Charge::Emp),
            cooldown,
            "rank {rank}"
        );
    }
}

/// Throw an EMP at machine 0 and run until it has burst.
fn emp_on_the_machine(world: &mut World) {
    let at = machine_on_deck(world, 0);
    let events = world.step(&[Command::Emp {
        slot: 0,
        x: (at.x / TILE).floor() as i32,
        y: (at.y / TILE).floor() as i32,
    }]);
    assert!(!refused(&events), "{events:?}");
    while !world.aboard.room.grenades().is_empty() {
        world.step(&[]);
    }
    world.step(&[]);
}

/// **Every droid kind in the radius is stunned, and none outside it.**
#[test]
fn every_machine_kind_in_the_radius_is_stunned_and_none_outside_it() {
    for kind in DroidKind::ALL {
        let mut world = fight_with(kind);
        ranks(&mut world, 0, [4, 0, 0, 0]);
        let far = another_machine(
            &mut world,
            DroidKind::Trooper,
            bims::math::vec2(0.0, 12.0 * TILE),
        );
        let apart = (machine_on_deck(&world, far) - machine_on_deck(&world, 0)).len();
        assert!(
            apart > world.emp_radius(0) * TILE + TILE,
            "{kind:?}: {apart}"
        );
        emp_on_the_machine(&mut world);
        assert!(machine(&world, 0).is_stunned(), "{kind:?} in the radius");
        assert!(
            !machine(&world, far).is_stunned(),
            "{kind:?}: the one outside"
        );
    }
}

/// **The Machine Heart, enemy Bims and the crew's own machines are never
/// stunned.** The Heart's machines refuse a stun outright
/// (`Droid::stun`); a Bim has no stun to have; and a sentry is no target
/// of the crew's, so an EMP reaches nothing of it.
#[test]
fn the_heart_enemy_bims_and_the_crew_s_machines_are_never_stunned() {
    for kind in DroidKind::HEART {
        let mut d = Droid::structure(
            kind,
            Tier::Three,
            100.0,
            bims::math::Vec2::ZERO,
            bims::math::vec2(1.0, 0.0),
            3,
        );
        assert!(!d.stun(3.0, true), "{kind:?}");
        assert!(!d.is_stunned());
    }
    // A sentry beside the machine: the EMP neither stuns nor harms it.
    let mut world = fight();
    ranks(&mut world, 0, [4, 0, 0, 1]);
    let tile = tile_near(&world, 0, DeployKind::Sentry);
    let (_, id) = deploy_now_keeping(&mut world, 0, DeployKind::Sentry, tile, mend_machine);
    let health = world.deployable(id).unwrap().health;
    // Thrown at the sentry's own tile.
    let events = world.step(&[Command::Emp {
        slot: 0,
        x: tile.0,
        y: tile.1,
    }]);
    assert!(!refused(&events), "{events:?}");
    while !world.aboard.room.grenades().is_empty() {
        world.step(&[]);
    }
    world.step(&[]);
    assert_eq!(world.deployable(id).unwrap().health, health);
}

/// **A stunned machine neither moves, turns, fires nor finishes a started
/// attack**, a Guardian's shield blocks nothing while it lasts, and a
/// second stun takes the longer timer rather than adding.
#[test]
fn a_stunned_machine_does_nothing_and_a_second_stun_takes_the_longer() {
    let mut d = Droid::new(
        DroidKind::Husk,
        Tier::One,
        0,
        1,
        bims::math::Vec2::ZERO,
        0.0,
        5,
    );
    d.blow = Some(bims::combat::Blow {
        target: 0,
        left: 0.2,
        damage: 30.0,
        cut: false,
        flat: 30.0,
    });
    d.follow_path(vec![bims::math::vec2(500.0, 0.0)]);
    assert!(d.stun(2.0, false));
    assert!(d.blow.is_none(), "the blow on its way is dropped");
    assert!(!d.is_walking(), "the route dropped");
    assert!(d.stun(1.0, false));
    assert_eq!(d.stunned, 2.0, "the longer, never added");
    assert!(d.stun(2.5, true));
    assert_eq!(d.stunned, 2.5);
    assert!(d.is_exposed());
    d.wear_off_stun(2.5);
    assert!(!d.is_stunned() && !d.is_exposed());
    // A Guardian: its shield blocks nothing while stunned, and a wind-up
    // is dropped to its cooldown.
    let mut g = Droid::new(
        DroidKind::Guardian,
        Tier::Three,
        0,
        1,
        bims::math::Vec2::ZERO,
        0.0,
        5,
    );
    assert!(g.shield().is_some());
    g.beam = bims::droid::Beam::WindUp {
        left: 0.5,
        aim: bims::math::vec2(1.0, 0.0),
        at: bims::math::vec2(100.0, 0.0),
        mark: 0,
    };
    assert!(g.stun(1.0, false));
    assert!(g.shield().is_none(), "the shield stops nothing");
    assert!(!g.beam.holds_heading(), "the wind-up let go of");
    g.wear_off_stun(1.0);
    assert!(g.shield().is_some(), "and is back when the stun ends");

    // In the room: an armed machine stunned stands still and fires at
    // nobody for as long as the stun lasts, then fights again.
    let mut world = fight();
    arm_machine(&mut world);
    world.aboard.room.set_health_for_probe(0, MAX_HEALTH);
    machine_mut(&mut world, 0).stun(3.0, false);
    let (pos, heading) = (machine(&world, 0).pos, machine(&world, 0).heading);
    let health = world.aboard.room.health(0);
    for _ in 0..150 {
        world.step(&[]);
        assert_eq!(machine(&world, 0).pos, pos, "it does not move");
        assert_eq!(machine(&world, 0).heading, heading, "nor turn");
    }
    assert_eq!(world.aboard.room.health(0), health, "nor fire");
    assert!(!machine(&world, 0).is_stunned() || machine(&world, 0).stunned < 0.6);
}

/// **Rank four's +25% applies only while the machine is stunned**, from
/// everyone, on the relics' own sum.
#[test]
fn rank_four_s_extra_damage_is_only_while_stunned() {
    let hit = |world: &World| Hit {
        who: world.residents.as_ref().unwrap().aboard.room.crew_count() as usize,
        part: Part::Body,
        damage: 8.0,
        cut: false,
        by: None,
        blast: false,
        roll: 0.4,
        strips: 0.0,
        flat: 0.0,
        crit: false,
    };
    let taken = |world: &mut World| {
        mend_machine(world);
        let before = machine_health(world, 0);
        let h = hit(world);
        let rest = world.land_on_machines(vec![h]);
        assert!(rest.is_empty());
        before - machine_health(world, 0)
    };
    let mut world = fight();
    let plain = taken(&mut world);
    assert!(plain > 0.0);
    machine_mut(&mut world, 0).stun(2.0, false);
    assert_eq!(taken(&mut world), plain, "a lower rank's stun adds nothing");
    machine_mut(&mut world, 0).stun(2.0, true);
    let exposed = taken(&mut world);
    assert!(
        (exposed - plain * 1.25).abs() < 1e-3,
        "{exposed} against {plain}"
    );
    let d = machine_mut(&mut world, 0);
    d.wear_off_stun(5.0);
    assert_eq!(taken(&mut world), plain, "and none once the stun is over");
}

// --- C: the Healing Sentry --------------------------------------------------------

/// The Healing Sentry laid beside James at `rank` of C, and Kate brought
/// beside it hurt: the world, the sentry's id and its position.
fn healing(rank: u8) -> (World, u32, bims::math::Vec2) {
    let mut world = engineer();
    ranks(&mut world, 0, [0, rank, 0, 0]);
    let tile = tile_near(&world, 0, DeployKind::HealingSentry);
    let (_, id) = deploy_now(&mut world, 0, DeployKind::HealingSentry, tile);
    let at = centre(tile);
    let beside = world
        .aboard
        .room
        .put_for_probe(1, at + bims::math::vec2(TILE, 0.0));
    assert!((beside - at).len() < 3.0 * TILE);
    world.aboard.room.set_health_for_probe(1, 40.0);
    (world, id, at)
}

/// **It heals at its rank's share of `HEAL_BEAM_HP`**, and never past the
/// full bar.
#[test]
fn a_healing_sentry_heals_at_its_rank_s_share_of_the_beam() {
    for (i, share) in class::HEALING_SENTRY_RATE.into_iter().enumerate() {
        let (mut world, _, _) = healing(i as u8 + 1);
        let before = world.aboard.room.health(1);
        let steps = 120;
        for _ in 0..steps {
            world
                .aboard
                .room
                .put_for_probe(1, world.aboard.room.bim_pos(1));
            world.step(&[]);
        }
        let got = world.aboard.room.health(1) - before;
        let want = share * class::HEAL_BEAM_HP * (data::STEP_MINUTES / 60.0) as f32 * steps as f32;
        assert!(
            (got - want).abs() < 0.05,
            "rank {}: {got} against {want}",
            i + 1
        );
    }
    // Never above full.
    let (mut world, _, _) = healing(4);
    world.aboard.room.set_health_for_probe(1, MAX_HEALTH - 0.01);
    for _ in 0..60 {
        world.step(&[]);
    }
    assert_eq!(world.aboard.room.health(1), MAX_HEALTH);
    assert!(
        world.healing_links().iter().all(|&(_, who, _)| who != 1),
        "nobody full is healed"
    );
}

/// **Not through a wall, not a downed Bim, not an enemy, not above full.**
#[test]
fn a_healing_sentry_heals_nobody_behind_a_wall_down_or_an_enemy() {
    let (mut world, id, _) = healing(4);
    assert!(
        world
            .healing_links()
            .iter()
            .any(|&(s, who, _)| s == id && who == 1)
    );
    // Downed: nothing.
    world.aboard.room.knock_out_for_probe(1);
    world.step(&[]);
    assert!(world.aboard.room.is_downed(1));
    assert!(world.healing_links().iter().all(|&(_, who, _)| who != 1));
    // Behind a wall within its radius: nothing — on a station's deck,
    // which has walls.
    let mut world = fight();
    ranks(&mut world, 0, [0, 4, 0, 0]);
    let tile = tile_near(&world, 0, DeployKind::HealingSentry);
    deploy_now_keeping(&mut world, 0, DeployKind::HealingSentry, tile, mend_machine);
    let at = centre(tile);
    let room = &world.aboard.room;
    let radius = class::HEALING_SENTRY_RADIUS[3] * TILE;
    let mut hidden = None;
    'search: for dy in -5i32..=5 {
        for dx in -5i32..=5 {
            let p = at + bims::math::vec2(dx as f32 * TILE, dy as f32 * TILE);
            if (p - at).len() <= radius && room.is_deck_tile(p) && !room.line_clear(at, p) {
                hidden = Some(p);
                break 'search;
            }
        }
    }
    let hidden = hidden.expect("a tile behind a wall within the radius");
    let put = world.aboard.room.put_for_probe(1, hidden);
    assert!(
        !world.aboard.room.line_clear(at, put),
        "Kate out of its sight"
    );
    world.aboard.room.set_health_for_probe(1, 40.0);
    assert!(world.healing_links().iter().all(|&(_, who, _)| who != 1));
    world.aboard.room.put_for_probe(1, hidden);
    world.step(&[]);
    assert_eq!(world.aboard.room.health(1), 40.0, "not through a wall");
    // And in its sight, the same distance off, she is healed.
    let seen = world.aboard.room.put_for_probe(
        1,
        at + bims::math::vec2(0.0, 0.0) + (hidden - at).normalize_or_zero() * TILE,
    );
    if world.aboard.room.line_clear(at, seen) {
        assert!(world.healing_links().iter().any(|&(_, who, _)| who == 1));
    }
    // An enemy is never on the list: it is the crew it heals.
    let crew = world.aboard.crew_count();
    assert!(world.healing_links().iter().all(|&(_, who, _)| who < crew));
}

/// **Two Healing Sentries reaching one Bim do not stack**: the highest
/// rate is what it gets.
#[test]
fn two_healing_sentries_do_not_stack() {
    let mut world = basic();
    assert_eq!(world.set_class(0, Class::Engineer), Ok(()));
    assert_eq!(world.set_class(1, Class::Engineer), Ok(()));
    ranks(&mut world, 0, [0, 1, 0, 0]);
    ranks(&mut world, 1, [0, 2, 0, 0]);
    let tile = tile_near(&world, 0, DeployKind::HealingSentry);
    deploy_now(&mut world, 0, DeployKind::HealingSentry, tile);
    world
        .aboard
        .room
        .put_for_probe(1, world.aboard.room.bim_pos(0));
    world.step(&[]);
    let tile = tile_near(&world, 1, DeployKind::HealingSentry);
    deploy_now(&mut world, 1, DeployKind::HealingSentry, tile);
    // Kate the patient now, both sentries reaching her.
    let here = world.aboard.room.bim_pos(1);
    world.aboard.room.set_health_for_probe(1, 40.0);
    let links: Vec<_> = world
        .healing_links()
        .into_iter()
        .filter(|&(_, who, _)| who == 1)
        .collect();
    assert_eq!(links.len(), 2, "both reach her: {links:?}");
    let before = world.aboard.room.health(1);
    let steps = 60;
    for _ in 0..steps {
        world.aboard.room.put_for_probe(1, here);
        world.step(&[]);
    }
    let got = world.aboard.room.health(1) - before;
    let best = class::HEALING_SENTRY_RATE[1] * class::HEAL_BEAM_HP;
    let want = best * (data::STEP_MINUTES / 60.0) as f32 * steps as f32;
    assert!(
        (got - want).abs() < 0.05,
        "{got} against the best alone, {want}"
    );
}

/// **Its charges are the standing limit**, and **enemies destroy it**
/// under the gun sentry's rule.
#[test]
fn a_healing_sentry_s_charges_are_the_limit_and_the_enemy_destroys_it() {
    let (mut world, first, _) = healing(1);
    world.set_charges_held(0, Charge::HealingSentry, 1);
    let tile = tile_near(&world, 0, DeployKind::HealingSentry);
    let (_, second) = deploy_now(&mut world, 0, DeployKind::HealingSentry, tile);
    assert!(world.deployable(first).is_none(), "the oldest destroyed");
    assert!(world.deployable(second).is_some());
    assert_eq!(world.laid_of(0, DeployKind::HealingSentry), 1);

    // In the fight, with James out of it, the machine shoots it down.
    let mut world = fight();
    ranks(&mut world, 0, [0, 1, 0, 0]);
    let tile = tile_near(&world, 0, DeployKind::HealingSentry);
    let (_, id) = deploy_now_keeping(&mut world, 0, DeployKind::HealingSentry, tile, mend_machine);
    assert_eq!(world.aboard.room.sentries().len(), 1);
    assert!(world.aboard.room.sentries()[0].heals);
    arm_machine(&mut world);
    world.aboard.room.knock_out_for_probe(0);
    let mut lost = false;
    for _ in 0..6_000 {
        mend_machine(&mut world);
        let events = world.step(&[]);
        if events.iter().any(|e| {
            matches!(e, WorldEvent::DeployableLost { kind } if *kind == DeployKind::HealingSentry.code())
        }) {
            lost = true;
            break;
        }
    }
    assert!(lost, "the machine shot the Healing Sentry to nothing");
    assert!(world.deployable(id).is_none());
}

/// Its numbers by rank: health, deploy time, charges and cooldown.
#[test]
fn a_healing_sentry_s_numbers_are_its_rank_s() {
    for i in 0..4 {
        let rank = i as u8 + 1;
        let mut world = engineer();
        ranks(&mut world, 0, [0, rank, 0, 0]);
        assert_eq!(
            world.laid_health(DeployKind::HealingSentry, 0),
            class::HEALING_SENTRY_HEALTH[i]
        );
        assert_eq!(
            world.deploy_minutes(0, DeployKind::HealingSentry),
            class::HEALING_SENTRY_MINUTES[i]
        );
        assert_eq!(world.charges(0, Charge::HealingSentry), 1);
        assert_eq!(
            world.charge_cooldown(0, Charge::HealingSentry),
            class::HEALING_SENTRY_COOLDOWN[i]
        );
    }
}

// --- E: sandbags -------------------------------------------------------------------

/// **Charges, health, deploy time and cooldown per rank**, and a laying
/// that takes its minutes.
#[test]
fn sandbags_charges_health_time_and_cooldown_are_their_rank_s() {
    let want = [
        (2, 150.0, 4.0, 45.0),
        (3, 200.0, 4.0, 45.0),
        (3, 250.0, 2.0, 40.0),
        (4, 250.0, 2.0, 35.0),
    ];
    for (i, (charges, health, minutes, cooldown)) in want.into_iter().enumerate() {
        let rank = i as u8 + 1;
        let mut world = engineer();
        ranks(&mut world, 0, [0, 0, rank, 0]);
        assert_eq!(world.charges(0, Charge::Sandbag), charges, "rank {rank}");
        assert_eq!(world.charges_of(0, Charge::Sandbag), charges);
        assert_eq!(world.deploy_minutes(0, DeployKind::Sandbags), minutes);
        assert_eq!(world.charge_cooldown(0, Charge::Sandbag), cooldown);
        let tile = tile_near(&world, 0, DeployKind::Sandbags);
        let (steps, id) = deploy_now(&mut world, 0, DeployKind::Sandbags, tile);
        let work = (minutes / data::STEP_MINUTES) as u32;
        assert!(
            steps >= work,
            "rank {rank}: {steps} steps for {work} of work"
        );
        let laid = world.deployable(id).unwrap();
        assert_eq!(laid.health, health, "rank {rank}");
        assert_eq!(laid.deck, Deck::Ship);
    }
}

/// A hit on the engineer does not interrupt laying sandbags or a Healing
/// Sentry: it keeps at the work, and the kit is laid.
#[test]
fn a_hit_does_not_drop_the_sandbags_or_the_healing_sentry() {
    for kind in [DeployKind::Sandbags, DeployKind::HealingSentry] {
        let mut world = engineer_at(1);
        let charge = kind.charge().unwrap();
        let held = world.charges_of(0, charge);
        let tile = tile_near(&world, 0, kind);
        let events = world.step(&[lay(0, kind, tile)]);
        assert!(!refused(&events));
        assert!(world.aboard.room.is_deploying(0));
        world.step(&[]);
        world.aboard.room.wound(0, Part::Body, 5.0);
        assert!(world.aboard.room.is_deploying(0), "{kind:?} kept at");
        run_for_seconds(&mut world, 30.0);
        assert!(
            world.deployable_under(centre(tile)).is_some(),
            "{kind:?} laid"
        );
        assert_eq!(world.charges_of(0, charge), held - 1, "the charge spent");
    }
}

/// A kit placed somewhere else while the first is on its way replaces
/// it: the first is dropped, never queued to be laid after.
#[test]
fn a_second_placement_replaces_the_first() {
    let mut world = engineer_at(1);
    let kind = DeployKind::Sandbags;
    let tiles = tiles_near(&world, 0, kind);
    let (first, second) = (tiles[0], *tiles.last().unwrap());
    assert!(!refused(&world.step(&[lay(0, kind, first)])));
    world.step(&[]);
    assert!(!refused(&world.step(&[lay(0, kind, second)])));
    run_for_seconds(&mut world, 60.0);
    assert!(
        world.deployable_under(centre(second)).is_some(),
        "the second laid"
    );
    assert!(
        world.deployable_under(centre(first)).is_none(),
        "the first dropped"
    );
    assert_eq!(world.deployables.len(), 1);
    assert!(!world.aboard.room.is_deploying(0), "nothing left queued");
}

/// The work is done within two tiles of the kit's tile, its bar empty
/// until the engineer is there: the walk counts nothing.
#[test]
fn a_kit_is_worked_within_two_tiles_and_the_walk_fills_no_bar() {
    let mut world = engineer_at(1);
    let kind = DeployKind::Sandbags;
    let from = world.aboard.room.bim_pos(0);
    let tile = tiles_near(&world, 0, kind)
        .into_iter()
        .rev()
        .find(|&t| (centre(t) - from).len() > 4.0 * TILE)
        .expect("a tile four away");
    assert!(!refused(&world.step(&[lay(0, kind, tile)])));
    let reach = bims::task::DEPLOY_REACH * TILE + 1e-3;
    let mut worked = false;
    for _ in 0..20_000 {
        if !world.aboard.room.is_deploying(0) {
            break;
        }
        if let Some((_, progress)) = world.aboard.room.working_at(0)
            && progress > 0.0
        {
            let off = (world.aboard.room.bim_pos(0) - centre(tile)).len();
            assert!(off <= reach, "worked {} tiles off", off / TILE);
            worked = true;
        }
        world.step(&[]);
    }
    assert!(worked, "the kit was worked");
    assert!(world.deployable_under(centre(tile)).is_some(), "and laid");
}

/// Under arms the engineer holsters while it lays a kit, and draws again
/// once it is down.
#[test]
fn an_engineer_fires_nothing_while_it_lays_a_kit() {
    let mut world = fight();
    for _ in 0..200 {
        if world.aboard.room.is_armed(0) {
            break;
        }
        world.step(&[]);
    }
    assert!(world.aboard.room.is_armed(0), "armed in the fight");
    ranks(&mut world, 0, [1; 4]);
    let kind = DeployKind::Sandbags;
    let tile = tile_near(&world, 0, kind);
    let (_, id) = deploy_now_keeping(&mut world, 0, kind, tile, |world| {
        if world
            .aboard
            .room
            .working_at(0)
            .is_some_and(|(_, p)| p > 0.0)
        {
            assert!(!world.aboard.room.is_armed(0), "holstered while laying");
        }
    });
    assert!(world.deployable(id).is_some());
    world.step(&[]);
    assert!(world.aboard.room.is_armed(0), "armed again after");
}

/// A tile with all four neighbours free to lay on, near James.
fn tile_with_room_round(world: &World) -> (i32, i32) {
    tiles_near(world, 0, DeployKind::Sandbags)
        .into_iter()
        .find(|&(x, y)| {
            [(0, -1), (1, 0), (0, 1), (-1, 0)].iter().all(|&(dx, dy)| {
                world
                    .can_deploy(0, DeployKind::Sandbags, (x + dx, y + dy))
                    .is_ok()
            })
        })
        .expect("a tile with room round it")
}

/// **Rank four lays the second bag on the first free neighbour, north,
/// east, south, west**, and only one when none is free.
#[test]
fn rank_four_lays_a_second_bag_north_east_south_west_and_one_when_none_is_free() {
    let mut world = engineer_at(4);
    let (x, y) = tile_with_room_round(&world);
    let tile_of = |world: &World, id: u32| world.deployable(id).map(|d| d.tile);
    // North free: the second goes north.
    let (_, id) = deploy_now(&mut world, 0, DeployKind::Sandbags, (x, y));
    let first = tile_of(&world, id - 1).unwrap();
    let second = tile_of(&world, id).unwrap();
    assert_eq!(world.deployables.len(), 2, "two tiles from one charge");
    assert_eq!(world.charges_of(0, Charge::Sandbag), 3, "one charge spent");
    // In the ship's design the tiles are the room's less a shift, the same
    // for both.
    assert_eq!(
        (
            second.0 as i64 - first.0 as i64,
            second.1 as i64 - first.1 as i64
        ),
        (0, -1)
    );
    // North taken: the next laid east of its tile.
    let mut world = engineer_at(4);
    ranks(&mut world, 0, [4, 4, 1, 4]);
    deploy_now(&mut world, 0, DeployKind::Sandbags, (x, y - 1));
    ranks(&mut world, 0, [4, 4, 4, 4]);
    world.set_charges_held(0, Charge::Sandbag, 4);
    let (_, id) = deploy_now(&mut world, 0, DeployKind::Sandbags, (x, y));
    let (a, b) = (
        tile_of(&world, id - 1).unwrap(),
        tile_of(&world, id).unwrap(),
    );
    assert_eq!(
        (b.0 as i64 - a.0 as i64, b.1 as i64 - a.1 as i64),
        (1, 0),
        "east"
    );
    // Every neighbour taken: one bag.
    let mut world = engineer_at(4);
    ranks(&mut world, 0, [4, 4, 1, 4]);
    for (dx, dy) in [(0, -1), (1, 0), (0, 1), (-1, 0)] {
        world.set_charges_held(0, Charge::Sandbag, 2);
        deploy_now(&mut world, 0, DeployKind::Sandbags, (x + dx, y + dy));
    }
    ranks(&mut world, 0, [4, 4, 4, 4]);
    world.set_charges_held(0, Charge::Sandbag, 4);
    let before = world.deployables.len();
    deploy_now(&mut world, 0, DeployKind::Sandbags, (x, y));
    assert_eq!(world.deployables.len(), before + 1, "only one");
}

#[test]
fn laid_sandbags_are_cover_in_both_rooms_and_survive_a_relayout() {
    let mut world = fight();
    ranks(&mut world, 0, [0, 0, 1, 0]);
    let tile = tile_near(&world, 0, DeployKind::Sandbags);
    let (_, id) = deploy_now(&mut world, 0, DeployKind::Sandbags, tile);
    let laid = *world.deployable(id).unwrap();
    let station = world.residents.as_ref().unwrap().station;
    assert_eq!(laid.deck, Deck::Station(station), "on the station's deck");
    let at = centre(tile);
    assert_eq!(world.aboard.room.laid_cover().len(), 1);
    let north = at - bims::math::vec2(0.0, TILE);
    let far_south = at + bims::math::vec2(0.0, 6.0 * TILE);
    assert!(world.aboard.room.covered_for_probe(north, far_south));
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
    let before = world.aboard.room.laid_cover().to_vec();
    world.relayout_room_for_probe();
    assert_eq!(world.aboard.room.laid_cover(), &before[..]);
    // The station's deck's sandbags are lost when the rooms unjoin.
    world.undock_for_probe();
    assert!(world.deployables.is_empty());
}

#[test]
fn sandbags_take_the_bolts_they_stop_and_are_gone_at_nothing() {
    let mut world = fight();
    ranks(&mut world, 0, [0, 0, 1, 0]);
    let tile = tile_near(&world, 0, DeployKind::Sandbags);
    let (_, id) = deploy_now(&mut world, 0, DeployKind::Sandbags, tile);
    let mut events = Vec::new();
    for _ in 0..3 {
        world
            .aboard
            .room
            .cover_hit_for_probe((tile.0, tile.1), 60.0);
        world.settle_deployables(&mut events);
    }
    assert!(
        events.iter().any(|e| matches!(
            e,
            WorldEvent::DeployableLost { kind } if *kind == DeployKind::Sandbags.code()
        )),
        "{events:?}"
    );
    assert!(world.deployable(id).is_none());
    assert!(world.aboard.room.laid_cover().is_empty());
}

// --- R: the sentry ------------------------------------------------------------------

/// **Weapon, fire rate, health and the range it adds per rank** — five
/// tiles from the first.
#[test]
fn the_sentry_s_weapon_rate_health_and_range_are_its_rank_s() {
    let want = [
        (Tier::Two, 1.0, 200.0, 5.0, 70.0),
        (Tier::Three, 1.0, 250.0, 5.0, 60.0),
        (Tier::Three, 1.5, 300.0, 5.0, 50.0),
        (Tier::Three, 2.0, 400.0, 5.0, 40.0),
    ];
    for (i, (tier, rate, health, range, cooldown)) in want.into_iter().enumerate() {
        let rank = i as u8 + 1;
        let mut world = engineer();
        ranks(&mut world, 0, [0, 0, 0, rank]);
        assert_eq!(
            world.sentry_weapon(0),
            WeaponKind::Minigun.at(tier),
            "rank {rank}"
        );
        assert_eq!(world.sentry_skill(0).fire_rate, rate);
        assert_eq!(world.laid_health(DeployKind::Sentry, 0), health);
        assert_eq!(world.sentry_skill(0).range, range);
        assert_eq!(world.sentry_cooldown(0), cooldown);
    }
}

/// **Its cooldown starts when it is laid, it stands until it is
/// destroyed — no timer —, one stands at a time, a hit does not interrupt
/// the laying, and it can't be packed up.**
#[test]
fn the_sentry_is_laid_through_a_hit_stands_until_destroyed_and_is_never_packed_up() {
    let mut world = engineer();
    ranks(&mut world, 0, [0, 0, 0, 1]);
    let free = tiles_near(&world, 0, DeployKind::Sentry);
    let (tile, tile2) = (free[0], free[1]);
    // Ready at the start, and the cooldown waits for the laying.
    assert_eq!(world.sentry_cooldown_left(0), 0.0);
    let events = world.step(&[lay(0, DeployKind::Sentry, tile)]);
    assert!(!refused(&events), "{events:?}");
    world.step(&[]);
    assert!(world.aboard.room.is_deploying(0));
    world.aboard.room.wound(0, Part::Body, 5.0);
    assert!(
        world.aboard.room.is_deploying(0),
        "a hit does not interrupt it"
    );
    assert_eq!(world.sentry_cooldown_left(0), 0.0, "not laid yet");
    let mut laid = None;
    for _ in 0..20_000 {
        let events = world.step(&[]);
        if events.iter().any(|e| {
            matches!(e, WorldEvent::Deployed { who: 0, kind } if *kind == DeployKind::Sentry.code())
        }) {
            laid = world.deployables.iter().map(|d| d.id).max();
            break;
        }
    }
    let id = laid.expect("the sentry laid");
    let left = world.sentry_cooldown_left(0);
    assert!(
        (left - world.sentry_cooldown(0)).abs() < 0.1,
        "the cooldown runs from the laying: {left}"
    );
    assert_eq!(world.aboard.room.sentries().len(), 1);
    assert!(!world.aboard.room.sentries()[0].heals);
    // Refused while it cools.
    assert_eq!(world.can_lay_sentry(0, tile2), Err(Refusal::CoolingDown));
    // Never packed up.
    let events = world.step(&[Command::PackUp { slot: 0, id }]);
    assert!(refused(&events), "{events:?}");
    assert!(world.deployable(id).is_some());
    // One at a time: the cooldown forgotten by hand, a second laid, and
    // the first is gone.
    world.engineers[0].sentry_laid = None;
    let (_, second) = deploy_now(&mut world, 0, DeployKind::Sentry, tile2);
    assert!(world.deployable(id).is_none(), "one stands");
    assert!(world.deployable(second).is_some());
    // No time runs out: a minute and more on, it still stands.
    assert!(world.sentry_standing(0));
    run_for_seconds(&mut world, 90.0);
    assert!(world.deployable(second).is_some(), "still standing");
    assert!(world.sentry_standing(0));
    assert_eq!(world.aboard.room.sentries().len(), 1);
}

/// **Ready at every mission's start**: the cooldown left in the last
/// mission is forgotten, and a sentry left standing goes with it.
#[test]
fn the_sentry_is_ready_at_the_start_of_every_mission() {
    let mut world = engineer();
    ranks(&mut world, 0, [0, 0, 0, 1]);
    let tile = tile_near(&world, 0, DeployKind::Sentry);
    deploy_now(&mut world, 0, DeployKind::Sentry, tile);
    assert!(world.sentry_cooldown_left(0) > 0.0);
    world.leave_for_probe();
    assert_eq!(world.run.phase, Phase::Map);
    let here = world.current_site();
    let site = world
        .sites_at(world.star_id)
        .into_iter()
        .find(|&s| Some(s) != here && world.travel_quote(s).is_ok())
        .expect("somewhere else to go");
    world.step(&[Command::Propose {
        slot: 0,
        star: site.star,
        station: site.station,
    }]);
    for slot in 1..world.players() {
        world.step(&[Command::Accept { slot, yes: true }]);
    }
    assert_eq!(world.run.phase, Phase::Mission);
    assert_eq!(world.sentry_cooldown_left(0), 0.0, "ready");
    assert!(
        world
            .deployables
            .iter()
            .all(|d| d.kind != DeployKind::Sentry),
        "the last mission's sentry gone"
    );
}

/// The sentry laid in the staged fight and the machine armed once it is.
fn sentry_fight() -> (World, u32) {
    let mut world = fight();
    ranks(&mut world, 0, [0, 0, 0, 1]);
    let tile = tile_near(&world, 0, DeployKind::Sentry);
    let (_, id) = deploy_now_keeping(&mut world, 0, DeployKind::Sentry, tile, mend_machine);
    mend_machine(&mut world);
    arm_machine(&mut world);
    (world, id)
}

/// The sentry fires at what it sees and **never runs out**; the enemy
/// aims at it and destroys it.
#[test]
fn the_sentry_fires_at_the_enemy_and_is_the_enemy_s_target() {
    let (mut world, id) = sentry_fight();
    assert_eq!(world.deployable(id).unwrap().kind, DeployKind::Sentry);
    world.aboard.room.knock_out_for_probe(0);
    let before = machine_health(&world, 0);
    let mut hit = false;
    for _ in 0..600 {
        mend_machine(&mut world);
        let whole = machine_health(&world, 0);
        world.step(&[]);
        if machine_health(&world, 0) < whole {
            hit = true;
            break;
        }
    }
    assert!(hit, "the sentry hit the machine ({before})");
    let mut lost = false;
    for _ in 0..(5 * 60 * 60) {
        mend_machine(&mut world);
        let events = world.step(&[]);
        if events.iter().any(|e| {
            matches!(e, WorldEvent::DeployableLost { kind } if *kind == DeployKind::Sentry.code())
        }) {
            lost = true;
            break;
        }
    }
    assert!(lost, "shot to nothing");
    assert!(world.deployable(id).is_none());
}

/// **It turns to fire**: laid facing east, once it has fired its barrel
/// points at the machine it fires at, and the flash is at its muzzle.
#[test]
fn the_sentry_turns_to_face_what_it_fires_at() {
    let (mut world, id) = sentry_fight();
    world.aboard.room.knock_out_for_probe(0);
    let sentry = |world: &World| {
        *world
            .aboard
            .room
            .sentries()
            .iter()
            .find(|s| s.id == id)
            .expect("the sentry stands")
    };
    let mut fired = false;
    for _ in 0..600 {
        mend_machine(&mut world);
        world.step(&[]);
        if sentry(&world).flash > 0.0 {
            fired = true;
            break;
        }
    }
    assert!(fired, "the sentry fired");
    for _ in 0..30 {
        mend_machine(&mut world);
        world.step(&[]);
    }
    let s = sentry(&world);
    let target = world
        .aboard
        .room
        .combat_targets_for_probe()
        .into_iter()
        .flatten()
        .next()
        .expect("the machine");
    let to = target - s.at;
    let tau = std::f32::consts::TAU;
    let off = (to.y.atan2(to.x) - s.facing).rem_euclid(tau);
    let off = off.min(tau - off);
    assert!(off < 0.2, "facing {} is {off} off the machine", s.facing);
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

// --- experience -----------------------------------------------------------------------

/// A machine destroyed is experience to every classed crew member in
/// range, once — the down and the death together (task 119).
#[test]
fn a_machine_destroyed_is_experience_once_each_to_the_classed_crew_in_range() {
    let mut world = fight();
    assert_eq!(world.set_class(1, Class::Engineer), Ok(()));
    let here = world.aboard.room.bim_pos(0);
    assert!(world.in_vicinity(0, here + bims::math::vec2(50.0 * TILE, 0.0)));
    assert!(!world.in_vicinity(0, here + bims::math::vec2(51.0 * TILE, 0.0)));
    world
        .aboard
        .room
        .put_for_probe(1, here + bims::math::vec2(TILE, 0.0));
    world.step(&[]);
    let (a, b) = (world.progress_of(0).xp, world.progress_of(1).xp);
    machine_mut(&mut world, 0).destroy();
    for _ in 0..3 {
        world.step(&[]);
    }
    let paid = class::XP_ENEMY_DOWN;
    assert_eq!(world.progress_of(0).xp, a + paid, "James");
    assert_eq!(world.progress_of(1).xp, b + paid, "Kate");
    world.step(&[]);
    assert_eq!(world.progress_of(0).xp, a + paid, "once");
    // Slot 1 put back to none learns nothing.
    let mut events = Vec::new();
    let mut plain = basic();
    plain.award(1, 1_000, &mut events);
    assert_eq!(plain.progress_of(1), Progress::default());
}

/// **Every class earns alike** (task 119): the same machine destroyed
/// beside crew member 0 and a crewmate of another class is the same
/// experience whatever class either is.
#[test]
fn each_class_gets_identical_experience_for_the_same_kills() {
    for class in Class::ALL.into_iter().filter(|&c| c != Class::None) {
        let mut world = basic();
        assert_eq!(world.set_class(0, class), Ok(()));
        let other = Class::ALL[Class::ALL.len() - class as usize];
        assert_eq!(world.set_class(1, other), Ok(()));
        assert!(world.stage_droid_fight_for_probe(DroidKind::Trooper, None));
        for _ in 0..3 {
            world.step(&[]);
        }
        let here = world.aboard.room.bim_pos(0);
        world
            .aboard
            .room
            .put_for_probe(1, here + bims::math::vec2(TILE, 0.0));
        world.step(&[]);
        let before = (world.progress_of(0).xp, world.progress_of(1).xp);
        machine_mut(&mut world, 0).destroy();
        for _ in 0..10 {
            world.step(&[]);
        }
        let gained = (
            world.progress_of(0).xp - before.0,
            world.progress_of(1).xp - before.1,
        );
        assert_eq!(gained.0, gained.1, "{class:?} beside {other:?}");
        assert_eq!(gained.0, class::XP_ENEMY_DOWN);
    }
}
