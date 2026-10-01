//! The tank class (feature 77; a ranked kit since task 139):
//! `crate::class`'s fourth class. The rank system and his two base traits
//! — the half-rate armour and the start — then Q Taunt, C Plated,
//! E Bulwark and R Juggernaut, each doing what its rank says, to the tank
//! who holds it alone.

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

/// What the enemy's room was handed of the crew's taunts: a radius in
/// room units and whether it pulls a blade, a target each — nought for
/// none, and for a target the enemy may not pick at all.
fn handed(world: &World) -> Vec<(f32, bool)> {
    world
        .residents
        .as_ref()
        .unwrap()
        .aboard
        .room
        .hostiles_taunting_for_probe()
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

// --- C: Q, Taunt --------------------------------------------------------------

#[test]
fn the_taunt_s_radius_time_and_cooldown_go_by_its_rank() {
    let want = [
        (6.0, 3.0, 20.0),
        (8.0, 4.0, 18.0),
        (10.0, 5.0, 16.0),
        (12.0, 6.0, 14.0),
    ];
    for (rank, &(radius, seconds, cooldown)) in (1..=4u8).zip(&want) {
        let mut world = tank_at([rank, 0, 0, 0]);
        assert_eq!(world.taunt_radius(0), radius, "rank {rank}");
        assert_eq!(world.taunt_seconds(0), seconds, "rank {rank}");
        assert_eq!(world.taunt_cooldown(0), cooldown, "rank {rank}");
        assert_eq!(world.can_taunt(0), Ok(()));
        let events = world.step(&[Command::Taunt { slot: 0 }]);
        assert!(
            events
                .iter()
                .any(|e| matches!(e, WorldEvent::Taunted { who: 0 })),
            "{events:?}"
        );
        assert!(world.is_taunting(0));
        assert!((world.taunt_left(0) - seconds).abs() < 0.1, "rank {rank}");
        assert_eq!(world.can_taunt(0), Err(Refusal::CoolingDown));
        run_for(&mut world, seconds - 0.5);
        assert!(world.is_taunting(0), "rank {rank}: still on");
        run_for(&mut world, 1.0);
        assert!(!world.is_taunting(0), "rank {rank}: over");
        assert_eq!(world.taunt_left(0), 0.0);
        // The cooldown runs from the taunt, not from its end.
        let left = world.taunt_cooldown_left(0);
        assert!(
            left > 0.0 && left <= cooldown - seconds + 1.0,
            "rank {rank}: {left}"
        );
        run_for(&mut world, left + 0.1);
        assert_eq!(world.can_taunt(0), Ok(()), "rank {rank}");
    }
}

#[test]
fn a_taunt_is_refused_unlearnt_downed_on_its_cooldown_and_to_the_others() {
    let mut world = tank();
    assert_eq!(world.set_class(1, Class::None), Ok(()));
    world.step(&[]);
    assert_eq!(world.can_taunt(0), Err(Refusal::NotLearnt));
    assert!(refused_with(
        &world.step(&[Command::Taunt { slot: 0 }]),
        Refusal::NotLearnt
    ));
    assert_eq!(world.can_taunt(1), Err(Refusal::NotATank));
    assert!(refused_with(
        &world.step(&[Command::Taunt { slot: 1 }]),
        Refusal::NotATank
    ));
    ranks(&mut world, 0, [1, 0, 0, 0]);
    world.aboard.room.knock_out_for_probe(0);
    world.step(&[]);
    assert_eq!(world.can_taunt(0), Err(Refusal::OutOfReach), "downed");
    world.aboard.room.patch_up_for_probe(0);
    world.step(&[]);
    assert_eq!(world.can_taunt(0), Ok(()));
    world.step(&[Command::Taunt { slot: 0 }]);
    assert!(refused_with(
        &world.step(&[Command::Taunt { slot: 0 }]),
        Refusal::CoolingDown
    ));
}

/// Who the enemy shot, over the steps a forcing runs: the tank and the
/// crewmate nearer the machine, each held where it was put with nothing
/// in its hands so only the machine fires.
fn who_is_shot(force: Option<Command>) -> (u32, u32, u64) {
    let mut world = basic();
    assert_eq!(world.set_class(0, Class::Tank), Ok(()));
    assert!(
        world
            .stage_droid_fight_for_probe(DroidKind::Trooper, Some(WeaponKind::LaserPistol.basic()))
    );
    ranks(&mut world, 0, [4, 0, 0, 4]);
    // The crew member the station's door put inside is the tank; the
    // crewmate stands two tiles further in and a tile to one side —
    // nearer the machine, which is four tiles in, and off the line
    // between the two.
    let tank_at = world.aboard.room.bim_pos(0);
    let resident = world
        .aboard
        .from_station(world.residents.as_ref().unwrap().aboard.position(0))
        .expect("on the joined deck");
    let towards = (vec2(resident.x as f32, resident.y as f32) - tank_at).normalize_or_zero();
    let mate_at = tank_at + towards * (2.0 * TILE) + towards.perp() * TILE;
    for who in [0usize, 1] {
        let mut gear = world.aboard.room.gear(who);
        gear.weapon = None;
        world.aboard.room.issue(who, gear);
        world.aboard.room.recruit_for_probe(who, true);
    }
    world.aboard.room.put_for_probe(1, mate_at);
    if let Some(command) = force {
        world.step(&[command]);
        assert!(world.is_taunting(0) || world.is_juggernaut(0));
    }
    // The machine is held where it was staged — its legs shot off — so
    // the test measures a choice rather than a walk.
    if let Some(residents) = &mut world.residents {
        let room = &mut residents.aboard.room;
        let legs = room.droid(0).unwrap().body.max(DroidPart::Legs);
        room.strike_droid(0, DroidPart::Legs, legs);
    }
    let (mut on_tank, mut on_mate) = (0, 0);
    // Five seconds: inside the fourth rank's taunt and the first rank's
    // Juggernaut alike.
    for _ in 0..(5.0 / SECONDS_A_STEP) as u32 {
        world.aboard.room.put_for_probe(0, tank_at);
        world.aboard.room.put_for_probe(1, mate_at);
        world.aboard.room.patch_up_for_probe(0);
        world.aboard.room.patch_up_for_probe(1);
        for hit in world.step(&[]) {
            if let WorldEvent::CrewHit { who, .. } = hit {
                match who {
                    0 => on_tank += 1,
                    1 => on_mate += 1,
                    _ => {}
                }
            }
        }
    }
    (on_tank, on_mate, world_checksum(&world))
}

#[test]
fn a_taunting_tank_is_shot_at_and_the_nearer_crewmate_is_not_and_two_runs_agree() {
    let (quiet_tank, quiet_mate, _) = who_is_shot(None);
    assert!(
        quiet_mate > quiet_tank,
        "the nearer crewmate takes the fire: {quiet_mate} against {quiet_tank}"
    );
    let (taunt_tank, taunt_mate, checksum) = who_is_shot(Some(Command::Taunt { slot: 0 }));
    assert!(taunt_tank > 0, "the taunt draws the fire");
    assert_eq!(taunt_mate, 0, "and nobody else is shot at");
    // Two runs on one seed are the same fight, to the checksum.
    let again = who_is_shot(Some(Command::Taunt { slot: 0 }));
    assert_eq!((taunt_tank, taunt_mate, checksum), again);
}

#[test]
fn the_taunt_the_enemies_are_handed_is_the_rank_s_radius_and_a_magnet_at_the_fourth() {
    let mut world = tank_in_a_fight([3, 0, 0, 0], [Class::None, Class::None]);
    assert!(
        handed(&world).iter().all(|&(r, m)| r == 0.0 && !m),
        "nobody is taunting"
    );
    world.step(&[Command::Taunt { slot: 0 }]);
    let flags = handed(&world);
    assert_eq!(
        flags[0],
        (class::TAUNT_RADIUS[2] * TILE, false),
        "no magnet"
    );
    assert!(flags[1..].iter().all(|&(r, _)| r == 0.0));
    // The fourth rank turns every charging blade within it.
    let mut world = tank_in_a_fight([4, 0, 0, 0], [Class::None, Class::None]);
    world.step(&[Command::Taunt { slot: 0 }]);
    assert_eq!(handed(&world)[0], (class::TAUNT_RADIUS[3] * TILE, true));
}

/// **An enemy taunted by two tanks follows the most recent taunt**: the
/// later taunt is handed the higher order, whichever tank is nearer.
#[test]
fn of_two_tanks_taunting_the_most_recent_is_followed() {
    let mut world = tank_in_a_fight([1, 0, 0, 0], [Class::Tank, Class::None]);
    ranks(&mut world, 1, [1, 0, 0, 0]);
    // The second tank stands in the doorway beside the first, where the
    // machine sees them both.
    let at = world.aboard.room.bim_pos(0);
    let resident = world
        .aboard
        .from_station(world.residents.as_ref().unwrap().aboard.position(0))
        .expect("on the joined deck");
    let towards = (vec2(resident.x as f32, resident.y as f32) - at).normalize_or_zero();
    world.aboard.room.put_for_probe(1, at + towards * TILE);
    world.step(&[]);
    let order = |world: &World| {
        world
            .residents
            .as_ref()
            .unwrap()
            .aboard
            .room
            .hostiles_taunt_order_for_probe()
    };
    world.step(&[Command::Taunt { slot: 0 }]);
    world.step(&[]);
    world.step(&[Command::Taunt { slot: 1 }]);
    let now = order(&world);
    assert!(now[1] > now[0] && now[0] > 0, "{now:?}");
    // Once the first has run out, the second alone.
    let left = world.taunt_left(0);
    // A step past the first's end, and still a step short of the
    // second's: they began two steps apart.
    run_for(&mut world, left + SECONDS_A_STEP);
    assert!(!world.is_taunting(0) && world.is_taunting(1));
    let now = order(&world);
    assert_eq!(now[0], 0, "{now:?}");
    assert!(now[1] > 0, "{now:?}");
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

/// **Every factor multiplies**: Plated, a Juggernaut and a commander's
/// Rally on one tank — Brace is the soldier's, and multiplies into the
/// same `Skill::damage_taken` the same way (`tests_soldier`).
#[test]
fn plated_multiplies_with_a_juggernaut_and_a_rally() {
    let mut world = basic();
    assert_eq!(world.set_class(0, Class::Tank), Ok(()));
    assert_eq!(world.set_class(1, Class::Commander), Ok(()));
    ranks(&mut world, 0, [0, 1, 0, 1]);
    ranks(&mut world, 1, [0, 0, 1, 0]);
    let at = world.aboard.room.bim_pos(0);
    world.aboard.room.put_for_probe(1, at + vec2(TILE, 0.0));
    world.step(&[Command::Juggernaut { slot: 0 }, Command::Rally { slot: 1 }]);
    assert!(world.is_juggernaut(0));
    assert_eq!(world.rally_reaching(0), Some(1));
    let want = class::PLATED_DAMAGE_TAKEN[0]
        * class::JUGGERNAUT_DAMAGE_TAKEN[0]
        * class::RALLY_DAMAGE_TAKEN[0];
    assert!(
        (world.skill_of(0).damage_taken - want).abs() < 1e-6,
        "{} against {want}",
        world.skill_of(0).damage_taken
    );
}

// --- E: E, Bulwark ---------------------------------------------------------------

#[test]
fn the_wall_s_reach_and_pace_go_by_its_rank_and_it_ends_on_a_toggle_or_going_down() {
    let mut world = tank();
    assert_eq!(world.set_class(1, Class::None), Ok(()));
    world.step(&[]);
    assert_eq!(world.can_bulwark(0), Err(Refusal::NotLearnt));
    assert!(refused_with(
        &world.step(&[Command::Bulwark { slot: 0, on: true }]),
        Refusal::NotLearnt
    ));
    assert_eq!(world.can_bulwark(1), Err(Refusal::NotATank));
    for (rank, (&reach, &pace)) in
        (1..=4u8).zip(class::BULWARK_REACH.iter().zip(&class::BULWARK_PACE))
    {
        let mut world = tank_at([0, 0, rank, 0]);
        assert_eq!(world.bulwark_reach(0), reach, "rank {rank}");
        assert_eq!(world.bulwark_pace(0), pace, "rank {rank}");
        assert_eq!(world.skill_of(0).walk, 1.0, "no wall, no cost");
        let events = world.step(&[Command::Bulwark { slot: 0, on: true }]);
        assert!(
            events
                .iter()
                .any(|e| matches!(e, WorldEvent::Bulwarked { who: 0, on: true })),
            "{events:?}"
        );
        assert_eq!(world.skill_of(0).walk, pace, "rank {rank}");
        let walls = world.aboard.room.bulwarks_for_probe();
        assert_eq!(walls.len(), 1);
        assert_eq!((walls[0].who, walls[0].reach), (0, reach), "rank {rank}");
        world.step(&[Command::Bulwark { slot: 0, on: false }]);
        assert!(!world.is_bulwark(0));
        assert!(world.aboard.room.bulwarks_for_probe().is_empty());
        assert_eq!(world.skill_of(0).walk, 1.0);
    }
    // And going down takes it down.
    let mut world = tank_at([0, 0, 1, 0]);
    world.step(&[Command::Bulwark { slot: 0, on: true }]);
    world.aboard.room.kill_for_probe(0);
    world.step(&[]);
    assert!(!world.is_bulwark(0), "the wall goes down with him");
    world.step(&[]);
    assert!(world.aboard.room.bulwarks_for_probe().is_empty());
    assert_eq!(world.can_bulwark(0), Err(Refusal::OutOfReach));
}

#[test]
fn the_wall_adds_dodge_from_its_third_rank_and_interposes_at_its_fourth() {
    for (rank, dodge, interpose) in [
        (1, 0.0, false),
        (2, 0.0, false),
        (3, class::GUARDED_DODGE, false),
        (4, class::GUARDED_DODGE, true),
    ] {
        let mut world = tank_at([0, 0, rank, 0]);
        assert_eq!(world.skill_of(0).dodge, 0.0, "not with the wall down");
        world.step(&[Command::Bulwark { slot: 0, on: true }]);
        assert_eq!(world.skill_of(0).dodge, dodge, "rank {rank}");
        let walls = world.aboard.room.bulwarks_for_probe();
        assert_eq!(walls[0].interpose, interpose, "rank {rank}");
    }
    // What *interpose* does to a bolt — onto him in place of the Bim he
    // shields — is the room's one shooter, pinned there (`combat::tests`,
    // `a_bulwark_shelters_the_body_behind_it_and_interposes_for_it`).
}

// --- F: R, Juggernaut -------------------------------------------------------------

#[test]
fn the_juggernaut_s_time_damage_taken_and_cooldown_go_by_its_rank() {
    let want = [
        (6.0, 0.50, 70.0),
        (7.0, 0.40, 60.0),
        (8.0, 0.35, 50.0),
        (10.0, 0.30, 40.0),
    ];
    for (rank, &(seconds, taken, cooldown)) in (1..=4u8).zip(&want) {
        let mut world = tank_at([0, 0, 0, rank]);
        assert_eq!(world.juggernaut_seconds(0), seconds, "rank {rank}");
        assert_eq!(world.juggernaut_cooldown(0), cooldown, "rank {rank}");
        let events = world.step(&[Command::Juggernaut { slot: 0 }]);
        assert!(
            events
                .iter()
                .any(|e| matches!(e, WorldEvent::Juggernaut { who: 0 })),
            "{events:?}"
        );
        assert!(world.is_juggernaut(0));
        assert_eq!(world.skill_of(0).damage_taken, taken, "rank {rank}");
        assert_eq!(world.aboard.room.skill_for_probe(0).damage_taken, taken);
        assert_eq!(world.skill_of(0).walk, 1.0, "his own pace");
        assert!(refused_with(
            &world.step(&[Command::Juggernaut { slot: 0 }]),
            Refusal::AlreadyActive
        ));
        run_for(&mut world, seconds - 0.5);
        assert!(world.is_juggernaut(0), "rank {rank}: still on");
        run_for(&mut world, 1.0);
        assert!(!world.is_juggernaut(0), "rank {rank}: over");
        assert_eq!(world.skill_of(0).damage_taken, 1.0);
        assert_eq!(world.can_juggernaut(0), Err(Refusal::CoolingDown));
        let left = world.juggernaut_cooldown_left(0);
        assert!(
            left > 0.0 && left <= cooldown - seconds + 1.0,
            "rank {rank}: {left}"
        );
    }
}

#[test]
fn the_juggernaut_is_refused_unlearnt_downed_and_to_the_others() {
    let mut world = tank();
    assert_eq!(world.set_class(1, Class::Soldier), Ok(()));
    world.step(&[]);
    assert!(refused_with(
        &world.step(&[Command::Juggernaut { slot: 0 }]),
        Refusal::NotLearnt
    ));
    assert!(refused_with(
        &world.step(&[Command::Juggernaut { slot: 1 }]),
        Refusal::NotATank
    ));
    ranks(&mut world, 0, [0, 0, 0, 1]);
    world.aboard.room.knock_out_for_probe(0);
    world.step(&[]);
    assert_eq!(world.can_juggernaut(0), Err(Refusal::OutOfReach), "downed");
    world.aboard.room.patch_up_for_probe(0);
    world.step(&[]);
    assert_eq!(world.can_juggernaut(0), Ok(()));
}

#[test]
fn every_enemy_that_sees_a_juggernaut_shoots_him_at_any_distance() {
    let mut world = tank_in_a_fight([0, 0, 0, 1], [Class::None, Class::None]);
    world.step(&[Command::Juggernaut { slot: 0 }]);
    let flags = handed(&world);
    assert_eq!(flags[0], (f32::INFINITY, false), "any distance, no magnet");
    assert!(flags[1..].iter().all(|&(r, _)| r == 0.0));
    // And in the fight the nearer crewmate is shot at by nobody.
    let (on_tank, on_mate, _) = who_is_shot(Some(Command::Juggernaut { slot: 0 }));
    assert!(on_tank > 0);
    assert_eq!(on_mate, 0, "him and nobody else");
}

#[test]
fn a_juggernaut_walks_at_his_pace_or_the_wall_s_and_runs_beside_a_taunt() {
    let mut world = tank_at([1, 0, 2, 1]);
    world.step(&[Command::Juggernaut { slot: 0 }]);
    assert_eq!(world.skill_of(0).walk, 1.0, "his own pace");
    world.step(&[Command::Bulwark { slot: 0, on: true }]);
    assert_eq!(
        world.skill_of(0).walk,
        class::BULWARK_PACE[1],
        "the wall's pace"
    );
    // A taunt beside it, each on its own timer.
    assert_eq!(world.can_taunt(0), Ok(()), "no bar to a taunt");
    world.step(&[Command::Taunt { slot: 0 }]);
    assert!(world.is_taunting(0) && world.is_juggernaut(0));
    run_for(&mut world, class::TAUNT_SECONDS[0] + 0.5);
    assert!(!world.is_taunting(0), "the taunt's three seconds are over");
    assert!(world.is_juggernaut(0), "the Juggernaut's six are not");
    run_for(
        &mut world,
        class::JUGGERNAUT_SECONDS[0] - class::TAUNT_SECONDS[0],
    );
    assert!(!world.is_juggernaut(0));
}

/// **A cloaked tank forces nothing**: a medic's cloak takes him off the
/// enemy's list, and his taunt with him, until it runs out.
#[test]
fn a_taunt_or_a_juggernaut_on_a_cloaked_tank_forces_nothing() {
    let mut world = tank_in_a_fight([1, 0, 0, 1], [Class::Medic, Class::None]);
    ranks(&mut world, 1, [0, 0, 0, 1]);
    let at = world.aboard.room.bim_pos(0);
    world.aboard.room.put_for_probe(1, at + vec2(TILE, 0.0));
    world.step(&[Command::Juggernaut { slot: 0 }, Command::Taunt { slot: 0 }]);
    assert_eq!(handed(&world)[0].0, f32::INFINITY);
    let events = world.step(&[Command::Cloak { slot: 1, target: 0 }]);
    assert!(
        events
            .iter()
            .any(|e| matches!(e, WorldEvent::Cloaked { who: 1, target: 0 })),
        "{events:?}"
    );
    world.step(&[]);
    assert!(world.is_cloaked(0) && world.is_juggernaut(0));
    assert_eq!(handed(&world)[0], (0.0, false), "no target, no forcing");
    // And a cloaked tank starts nothing new (task 130's rule).
    let mut other = tank_in_a_fight([0, 0, 0, 1], [Class::Medic, Class::None]);
    ranks(&mut other, 1, [0, 0, 0, 1]);
    let at = other.aboard.room.bim_pos(0);
    other.aboard.room.put_for_probe(1, at + vec2(TILE, 0.0));
    other.step(&[Command::Cloak { slot: 1, target: 0 }]);
    assert!(refused_with(
        &other.step(&[Command::Juggernaut { slot: 0 }]),
        Refusal::Cloaked
    ));
}

// --- G: the clocks ------------------------------------------------------------------

#[test]
fn the_taunt_and_the_juggernaut_are_ready_at_every_mission_and_shortened_by_the_relics() {
    let mut world = tank_at([1, 0, 0, 1]);
    let (taunt, jug) = (world.taunt_cooldown(0), world.juggernaut_cooldown(0));
    world.give_relic_for_probe(0, crate::relic::Relic::CoolantLoop);
    assert!(world.taunt_cooldown(0) < taunt, "*Coolant Loop*");
    assert!(world.juggernaut_cooldown(0) < jug, "*Coolant Loop*");
    world.step(&[Command::Taunt { slot: 0 }, Command::Juggernaut { slot: 0 }]);
    run_for(&mut world, 10.0);
    let (taunt_left, jug_left) = (
        world.taunt_cooldown_left(0),
        world.juggernaut_cooldown_left(0),
    );
    assert!(taunt_left > 0.0 && jug_left > 0.0);
    // *Kill Relay*: a kill takes seconds off both.
    world.give_relic_for_probe(0, crate::relic::Relic::KillRelay);
    let mut events = Vec::new();
    world.machine_kills(&[(Some(0), 0)], &mut events);
    assert!(world.taunt_cooldown_left(0) < taunt_left, "*Kill Relay*");
    assert!(world.juggernaut_cooldown_left(0) < jug_left, "*Kill Relay*");
    // Ready at every mission's start, whatever was left.
    next_mission(&mut world);
    assert_eq!(world.taunt_cooldown_left(0), 0.0);
    assert_eq!(world.juggernaut_cooldown_left(0), 0.0);
    assert_eq!(world.can_taunt(0), Ok(()));
    assert_eq!(world.can_juggernaut(0), Ok(()));
}

// --- the seam ------------------------------------------------------------------------

#[test]
fn the_checksum_notices_a_wall_a_taunt_a_juggernaut_and_a_hit_taken() {
    let base = || tank_at([1, 0, 1, 1]);
    let plain = world_checksum(&base());
    let mut walled = base();
    walled.step(&[Command::Bulwark { slot: 0, on: true }]);
    let mut quiet = base();
    quiet.step(&[]);
    assert_ne!(world_checksum(&walled), world_checksum(&quiet), "a wall");
    let mut taunting = base();
    taunting.step(&[Command::Taunt { slot: 0 }]);
    assert_ne!(world_checksum(&taunting), world_checksum(&quiet), "a taunt");
    let mut jug = base();
    jug.step(&[Command::Juggernaut { slot: 0 }]);
    assert_ne!(world_checksum(&jug), world_checksum(&quiet), "a Juggernaut");
    assert_ne!(world_checksum(&jug), world_checksum(&taunting));
    let mut hit = base();
    let before = world_checksum(&hit);
    assert!(shoot_at(&mut hit, 0, 8) > 0);
    assert_ne!(world_checksum(&hit), before);
    let _ = plain;
    // And a tank's kit is a resource nobody's hold moved.
    let mut none = basic();
    assert_eq!(none.set_class(0, Class::None), Ok(()));
    none.step(&[]);
    let mut tanked = basic();
    assert_eq!(tanked.set_class(0, Class::Tank), Ok(()));
    tanked.step(&[]);
    for resource in [ResourceId::Armour, ResourceId::Armour, ResourceId::Armour] {
        assert_eq!(
            none.ship.design.carrying(resource),
            tanked.ship.design.carrying(resource),
            "the tank's kit comes with him, not out of the hold"
        );
    }
    // Two tanks' worth of the crew held still and shot at: the same
    // fight twice.
    let run = || {
        let mut world = tank_at([2, 2, 2, 1]);
        hold_still(&mut world);
        world.step(&[Command::Taunt { slot: 0 }, Command::Juggernaut { slot: 0 }]);
        shoot_at(&mut world, 0, 4);
        world_checksum(&world)
    };
    assert_eq!(run(), run());
}
