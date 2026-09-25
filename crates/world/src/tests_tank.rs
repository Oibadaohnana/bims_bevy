//! The tank class (feature 77): `crate::class`'s fourth class. Its
//! start and its one source of experience; the armour passive; the
//! bulwark; the taunt; and each of the ten levels' talents doing what it
//! says, to the tank who holds it alone.

use bims::combat::{ArmourKind, Item, Piece, Tier, WeaponKind};
use bims::droid::{DroidKind, DroidPart};
use bims::health::Part;
use bims::math::vec2;
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

/// The crew held where they stand: their own errands off, and nobody
/// doctoring of their own accord, so a test that opens a wound keeps it
/// open.
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

/// Every enemy hit this crew member has taken since the world opened:
/// the whole points its count has made, and what is left over.
fn hits_taken(world: &World, who: u32) -> u32 {
    world.progress_of(who).xp * class::TANK_HITS_PER_XP + world.aboard.room.hits_taken(who as usize)
}

/// A piece of armour worn by `who`, put on by hand over whatever was
/// there, with `health` left on it.
fn wear(world: &mut World, who: usize, kind: ArmourKind, health: f32) -> u32 {
    let id = world.next_piece;
    world.next_piece += 1;
    let mut piece = Piece::new(id, kind, Tier::One);
    piece.health = health;
    world.pieces.push(crate::armour::Piece {
        health,
        at: crate::Where::Worn { who: who as u32 },
        ..crate::armour::Piece::new(id, kind, Tier::One)
    });
    let mut gear = world.aboard.room.gear(who);
    *gear.worn_mut(kind.slot()) = Some(piece);
    world.aboard.room.issue(who, gear);
    id
}

/// The health left on a worn piece.
fn worn_health(world: &World, who: usize, kind: ArmourKind) -> f32 {
    world
        .aboard
        .room
        .gear(who)
        .worn(kind.slot())
        .map_or(0.0, |p| p.health)
}

/// Fire `shots` pistol bolts at a crew member from six tiles off, one a
/// step, and say how many landed on it — off the world's own `CrewHit`,
/// which is every hostile bolt that reached a body. Whoever is shot at
/// is patched up and stood still between shots, so a run is the bolts
/// and nothing else.
fn shoot_at(world: &mut World, who: u32, shots: u32) -> u32 {
    let at = world.aboard.room.bim_pos(who as usize);
    let from = at - vec2(6.0 * TILE, 0.0);
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

// --- A: the class, the start, the experience and the armour passive ---------

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
    for kind in ArmourKind::ALL {
        let piece = world
            .aboard
            .room
            .gear(0)
            .worn(kind.slot())
            .expect("a basic piece on every part");
        assert_eq!((piece.kind, piece.tier), (kind, Tier::One));
        assert_eq!(piece.health, kind.stats().health, "fresh");
        assert!(
            world.pieces.iter().any(|p| p.id == piece.id),
            "the world knows the piece"
        );
    }
    // And a class put back to none takes its start off again.
    assert_eq!(world.set_class(0, Class::None), Ok(()));
    for kind in ArmourKind::ALL {
        assert!(world.aboard.room.gear(0).worn(kind.slot()).is_none());
    }
    // Every crew member wears every piece of armour, whatever its class:
    // the tank's kit is a start, not a monopoly.
    assert_eq!(world.set_class(1, Class::Medic), Ok(()));
    let helm = Item::Armour(Piece::new(9_001, ArmourKind::BasicHelm, Tier::One));
    assert!(world.aboard.room.give(1, None, helm));
    let cell = world
        .aboard
        .room
        .pack(1)
        .iter()
        .position(|i| *i == Some(helm))
        .expect("in the pack");
    world.aboard.room.equip(1, cell);
    assert!(world.aboard.room.gear(1).worn(Part::Head).is_some());
}

#[test]
fn armour_on_a_tank_drains_at_half_rate_and_the_overflow_reaches_the_body() {
    let mut world = tank();
    assert_eq!(world.set_class(1, Class::None), Ok(()));
    world.step(&[]);
    let kevlar = ArmourKind::BasicKevlar;
    let protection = kevlar.stats().protection;
    assert_eq!(kevlar.stats().health, 20.0);
    // The same piece, at twenty, on the tank: forty past its protection
    // is exactly what it can take, and nothing reaches the body.
    wear(&mut world, 0, kevlar, 20.0);
    let out = world.aboard.room.wound(0, Part::Body, 40.0 + protection);
    assert_eq!(out.through, 0.0, "the tank's kevlar took all forty");
    assert_eq!(out.absorbed, 40.0 + protection);
    assert_eq!(worn_health(&world, 0, kevlar), 0.0, "and is spent");
    assert!(out.piece_broke);
    // And on anybody else it takes twenty, the other twenty reaching the
    // body.
    wear(&mut world, 1, kevlar, 20.0);
    let out = world.aboard.room.wound(1, Part::Body, 40.0 + protection);
    assert_eq!(out.through, 20.0, "half of it through");
    assert_eq!(out.absorbed, 20.0 + protection);
    assert_eq!(worn_health(&world, 1, kevlar), 0.0);
    // The overflow on a tank is exact too: sixty past the protection is
    // forty taken and twenty through.
    wear(&mut world, 0, kevlar, 20.0);
    let out = world.aboard.room.wound(0, Part::Body, 60.0 + protection);
    assert_eq!(out.through, 20.0);
    assert_eq!(out.absorbed, 40.0 + protection);
    // A hit the protection swallows whole is swallowed for everybody,
    // and the piece's stored health is never doubled.
    wear(&mut world, 0, kevlar, 20.0);
    let out = world.aboard.room.wound(0, Part::Body, protection);
    assert_eq!((out.through, out.absorbed), (0.0, protection));
    assert_eq!(worn_health(&world, 0, kevlar), 20.0, "not a point off");
    assert_eq!(world.armour_drain(0), class::TANK_DRAIN);
    assert_eq!(world.armour_drain(1), 1.0);
}

#[test]
fn five_enemy_hits_are_one_point_of_experience_and_the_count_starts_again() {
    let mut world = tank();
    assert_eq!(world.set_class(1, Class::None), Ok(()));
    world.step(&[]);
    // Shot at until five have landed on him: one point, nothing left
    // over. Every kind of landing counts — his armour is on, and what it
    // does not take his body does.
    let mut landed = 0;
    while landed < class::TANK_HITS_PER_XP {
        landed += shoot_at(&mut world, 0, 1);
    }
    assert_eq!(hits_taken(&world, 0), landed, "every landing counted");
    assert_eq!(world.progress_of(0).xp, landed / class::TANK_HITS_PER_XP);
    assert_eq!(
        world.aboard.room.hits_taken(0),
        landed % class::TANK_HITS_PER_XP,
        "the remainder counts on"
    );
    // A miss counts for nothing: more bolts than landings, every time.
    let before = hits_taken(&world, 0);
    let landed = shoot_at(&mut world, 0, 30);
    assert!(landed < 30, "{landed} of thirty bolts landed");
    assert_eq!(hits_taken(&world, 0) - before, landed);
    // A hit a surge takes is still a hit that landed on him.
    let before = hits_taken(&world, 0);
    world.aboard.room.set_surge(0, 60.0, false);
    let health = world.aboard.room.health(0);
    let landed = shoot_at(&mut world, 0, 6);
    assert!(landed > 0);
    assert_eq!(hits_taken(&world, 0) - before, landed);
    assert_eq!(world.aboard.room.health(0), health, "the surge took it");
    // A hit from his own side is not one: his crewmate's grenade.
    let before = hits_taken(&world, 0);
    let at = world.aboard.room.bim_pos(0);
    world.aboard.room.put_for_probe(1, at + vec2(TILE, 0.0));
    world
        .aboard
        .room
        .throw_grenade(1, at, 0.2, 2.0 * TILE, 40.0);
    for _ in 0..120 {
        world.step(&[]);
        if world.aboard.room.grenades().is_empty() {
            break;
        }
    }
    assert_eq!(hits_taken(&world, 0), before, "his own soldier's burst");
    // And a crew member that is not a tank gains nothing from being hit.
    // The crew held still for it and the tank stood off the line: a
    // crewmate lying out of harm is dressed in a fight now rather than
    // twenty seconds after it, so the burst's wounds have the three of
    // them walking over to one another, and a body between the gun and
    // the crewmate takes its bolts.
    hold_still(&mut world);
    world
        .aboard
        .room
        .put_for_probe(0, at + vec2(0.0, 3.0 * TILE));
    let landed = shoot_at(&mut world, 1, 20);
    assert!(landed > 0, "the bolts landed on the crewmate");
    assert!(world.aboard.room.hits_taken(1) > 0, "the count is kept");
    assert_eq!(world.progress_of(1).xp, 0, "and makes nothing");
}

// --- B: the bulwark ----------------------------------------------------------

#[test]
fn the_wall_halves_the_pace_is_handed_to_the_room_and_ends_on_a_toggle_or_going_down() {
    let mut world = tank();
    assert_eq!(world.set_class(1, Class::None), Ok(()));
    assert_eq!(world.set_class(2, Class::Medic), Ok(()));
    world.step(&[]);
    assert_eq!(world.skill_of(0).walk, 1.0, "no wall, no cost");
    let events = world.step(&[Command::Bulwark { slot: 0, on: true }]);
    assert!(
        events
            .iter()
            .any(|e| matches!(e, WorldEvent::Bulwarked { who: 0, on: true })),
        "{events:?}"
    );
    assert!(world.is_bulwark(0));
    assert_eq!(world.skill_of(0).walk, class::BULWARK_PACE);
    // The room has the wall, with the tank's own reach on it, and only
    // the tank's.
    let walls = world.aboard.room.bulwarks_for_probe();
    assert_eq!(walls.len(), 1);
    assert_eq!(walls[0].who, 0);
    assert_eq!(walls[0].reach, class::BULWARK_REACH);
    assert!(!walls[0].interpose);
    // Nobody else may put one up.
    for slot in [1, 2] {
        assert_eq!(world.can_bulwark(slot), Err(Refusal::NotATank));
        let events = world.step(&[Command::Bulwark { slot, on: true }]);
        assert!(refused_with(&events, Refusal::NotATank));
        assert!(!world.is_bulwark(slot));
    }
    // Toggled off it is down, and the room has no wall.
    let events = world.step(&[Command::Bulwark { slot: 0, on: false }]);
    assert!(
        events
            .iter()
            .any(|e| matches!(e, WorldEvent::Bulwarked { who: 0, on: false })),
        "{events:?}"
    );
    assert!(!world.is_bulwark(0));
    assert!(world.aboard.room.bulwarks_for_probe().is_empty());
    assert_eq!(world.skill_of(0).walk, 1.0);
    // And going down takes it down.
    world.step(&[Command::Bulwark { slot: 0, on: true }]);
    assert!(world.is_bulwark(0));
    world.aboard.room.kill_for_probe(0);
    world.step(&[]);
    assert!(!world.is_bulwark(0), "the wall goes down with him");
    world.step(&[]);
    assert!(
        world.aboard.room.bulwarks_for_probe().is_empty(),
        "and shelters nobody after it"
    );
    // A tank that cannot act cannot put one up either.
    assert_eq!(world.can_bulwark(0), Err(Refusal::OutOfReach));
}

// --- C: the taunt ------------------------------------------------------------

#[test]
fn a_taunt_wants_the_third_level_runs_its_minutes_and_waits_its_cooldown() {
    let mut world = tank();
    assert_eq!(world.set_class(1, Class::None), Ok(()));
    world.step(&[]);
    assert_eq!(world.can_taunt(0), Err(Refusal::NoTauntYet));
    let events = world.step(&[Command::Taunt { slot: 0 }]);
    assert!(refused_with(&events, Refusal::NoTauntYet));
    assert_eq!(world.can_taunt(1), Err(Refusal::NotATank));
    assert!(refused_with(
        &world.step(&[Command::Taunt { slot: 1 }]),
        Refusal::NotATank
    ));
    level_up(&mut world, 0, class::TAUNT_LEVEL);
    assert_eq!(world.can_taunt(0), Ok(()));
    assert!(!world.is_taunting(0));
    let events = world.step(&[Command::Taunt { slot: 0 }]);
    assert!(
        events
            .iter()
            .any(|e| matches!(e, WorldEvent::Taunted { who: 0 })),
        "{events:?}"
    );
    assert!(world.is_taunting(0));
    assert!((world.taunt_left(0) - class::TAUNT_MINUTES).abs() < 0.1);
    assert_eq!(world.can_taunt(0), Err(Refusal::CoolingDown));
    // It runs its minutes and no longer.
    for _ in 0..(class::TAUNT_MINUTES as u32 * STEPS_A_MINUTE) {
        world.step(&[]);
    }
    assert!(!world.is_taunting(0), "six minutes and it is over");
    assert_eq!(world.taunt_left(0), 0.0);
    assert_eq!(world.can_taunt(0), Err(Refusal::CoolingDown));
    assert!(world.taunt_cooldown_left(0) > 0.0);
    // And the cooldown runs from the taunt, not from its end.
    let left = world.taunt_cooldown_left(0);
    for _ in 0..((left / time::MINUTES_PER_SECOND) as u32 * STEPS_A_MINUTE + STEPS_A_MINUTE) {
        world.step(&[]);
    }
    assert_eq!(world.taunt_cooldown_left(0), 0.0);
    assert_eq!(world.can_taunt(0), Ok(()));
}

#[test]
fn a_taunting_tank_is_shot_at_before_a_nearer_crewmate_and_two_runs_agree() {
    // Who the enemy shot, over a hundred steps: the tank and the
    // crewmate, each held where it was put with nothing in its hands so
    // only the machine fires.
    let run = |taunt: bool| -> (u32, u32, u64) {
        let mut world = basic();
        assert_eq!(world.set_class(0, Class::Tank), Ok(()));
        assert!(world.stage_droid_fight_for_probe(
            DroidKind::Trooper,
            Some(WeaponKind::LaserPistol.basic())
        ));
        level_up(&mut world, 0, class::TAUNT_LEVEL);
        // The crew member the station's door put inside is the tank; the
        // crewmate stands two tiles further in and a tile to one side —
        // nearer the machine, which is four tiles in, and off the line
        // between the two, so a bolt aimed at the tank does not have to
        // go through it.
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
        if taunt {
            world.step(&[Command::Taunt { slot: 0 }]);
            assert!(world.is_taunting(0));
        }
        // The machine is held where it was staged too — its legs shot
        // off, and a machine with no legs fights where it stands — or its
        // tactics walk it out of the taunt's reach and the test measures
        // a walk rather than a choice. Nobody of the crew is armed, so
        // nothing else of it is ever hit.
        if let Some(residents) = &mut world.residents {
            residents.aboard.room.strike_droid(0, DroidPart::Legs, 1e6);
        }
        let (mut on_tank, mut on_mate) = (0, 0);
        for _ in 0..400 {
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
    };
    let (quiet_tank, quiet_mate, _) = run(false);
    assert!(
        quiet_mate > quiet_tank,
        "the nearer crewmate takes the fire: {quiet_mate} against {quiet_tank}"
    );
    let (taunt_tank, taunt_mate, checksum) = run(true);
    assert!(
        taunt_tank > taunt_mate,
        "the taunt takes it instead: {taunt_tank} against {taunt_mate}"
    );
    // Two runs on one seed are the same fight, to the checksum.
    let (again_tank, again_mate, again) = run(true);
    assert_eq!((taunt_tank, taunt_mate), (again_tank, again_mate));
    assert_eq!(checksum, again);
}

#[test]
fn the_taunt_the_enemies_are_handed_is_the_tank_s_radius_and_nobody_else_s() {
    let mut world = basic();
    assert_eq!(world.set_class(0, Class::Tank), Ok(()));
    assert!(world.stage_droid_fight_for_probe(DroidKind::Trooper, None));
    level_up(&mut world, 0, class::TAUNT_LEVEL);
    world.step(&[]);
    let handed = |world: &World| {
        world
            .residents
            .as_ref()
            .unwrap()
            .aboard
            .room
            .hostiles_taunting_for_probe()
    };
    assert!(
        handed(&world).iter().all(|&(r, m)| r == 0.0 && !m),
        "nobody is taunting"
    );
    world.step(&[Command::Taunt { slot: 0 }]);
    let flags = handed(&world);
    assert_eq!(flags[0], (class::TAUNT_RADIUS * TILE, false));
    assert!(flags[1..].iter().all(|&(r, _)| r == 0.0));
    // A melee charger is unmoved by a plain taunt: *magnet* is what
    // turns one, and it is a talent.
    pick(&mut world, 0, Talent::Magnet);
    world.step(&[]);
    assert_eq!(handed(&world)[0].1, true);
}

// --- D: the ten levels -------------------------------------------------------

#[test]
fn the_fixed_levels_are_the_wall_the_taunt_and_the_iron_frame() {
    let mut world = tank();
    assert_eq!(world.set_class(1, Class::None), Ok(()));
    world.step(&[]);
    // One: the wall, and the armour draining at half rate.
    assert_eq!(world.progress_of(0).level(), 1);
    assert_eq!(world.can_bulwark(0), Ok(()));
    assert_eq!(world.armour_drain(0), class::TANK_DRAIN);
    // Three: the taunt.
    assert_eq!(world.can_taunt(0), Err(Refusal::NoTauntYet));
    level_up(&mut world, 0, class::TAUNT_LEVEL);
    assert_eq!(world.can_taunt(0), Ok(()));
    // Seven: a hit rolled on the head lands on the body.
    assert!(!world.skill_of(0).iron_frame);
    level_up(&mut world, 0, class::IRON_FRAME_LEVEL);
    world.step(&[]);
    assert!(world.skill_of(0).iron_frame);
    assert!(!world.skill_of(1).iron_frame, "his alone");
    // The helm is untouched and the kevlar takes it.
    let helm = worn_health(&world, 0, ArmourKind::BasicHelm);
    wear(&mut world, 0, ArmourKind::BasicKevlar, 20.0);
    let out = world.aboard.room.wound(0, Part::Head, 6.0);
    assert!(out.absorbed > 0.0);
    assert_eq!(worn_health(&world, 0, ArmourKind::BasicHelm), helm);
    assert!(worn_health(&world, 0, ArmourKind::BasicKevlar) < 20.0);
}

#[test]
fn plated_stands_alone_at_the_second_level() {
    // Since the money rework (feature 95) the tank's second level is a
    // **fixed** level: *pack mule* went with the hauling, and *plated* is
    // given outright rather than chosen at.
    assert!(!class::is_pick_level(Class::Tank, 2));
    assert_eq!(class::fixed_at(Class::Tank, 2), Some(Talent::Plated));
    assert_eq!(class::pick_at(Class::Tank, 2), None);
    // *Plated*: half again the protection, on him, from the second level
    // with nothing to pick.
    let mut world = tank();
    assert_eq!(world.set_class(1, Class::Tank), Ok(()));
    // Slot 0 to the second level, where *plated* is given; slot 1 left
    // at the first, where it is not.
    level_up(&mut world, 0, 2);
    world.step(&[]);
    assert!(world.has_talent(0, Talent::Plated));
    assert!(!world.has_talent(1, Talent::Plated));
    assert_eq!(
        world.skill_of(0).armour_protection,
        class::PLATED_PROTECTION
    );
    assert_eq!(world.skill_of(1).armour_protection, 1.0);
    let kevlar = ArmourKind::BasicKevlar;
    let protection = kevlar.stats().protection;
    for who in [0, 1] {
        wear(&mut world, who, kevlar, 20.0);
    }
    let plated = world.aboard.room.wound(0, Part::Body, 10.0);
    let plain = world.aboard.room.wound(1, Part::Body, 10.0);
    assert_eq!(
        20.0 - worn_health(&world, 0, kevlar),
        (10.0 - protection * class::PLATED_PROTECTION) * class::TANK_DRAIN
    );
    assert_eq!(
        20.0 - worn_health(&world, 1, kevlar),
        (10.0 - protection) * class::TANK_DRAIN
    );
    assert_eq!((plated.through, plain.through), (0.0, 0.0));
}

#[test]
fn breacher_and_unmovable() {
    // *Breacher*: a locked door forced in half the time.
    let mut world = tank();
    assert_eq!(world.skill_of(0).smash_rate, 1.0);
    pick(&mut world, 0, Talent::Breacher);
    world.step(&[]);
    assert_eq!(world.skill_of(0).smash_rate, 1.0 / class::BREACHER_TIME);
    assert_eq!(world.skill_of(1).smash_rate, 1.0, "his alone");
    // *Unmovable*: never flees, and no pace lost to low blood while the
    // kevlar holds.
    let mut world = tank();
    assert_eq!(world.set_class(1, Class::Tank), Ok(()));
    world.step(&[]);
    assert!(!world.skill_of(0).nerve && !world.skill_of(0).steady_pace);
    pick(&mut world, 0, Talent::Unmovable);
    world.step(&[]);
    assert!(world.skill_of(0).nerve && world.skill_of(0).steady_pace);
    assert!(!world.skill_of(1).nerve && !world.skill_of(1).steady_pace);
    // Bled to half, the crewmate walks at half pace and the tank does
    // not — his kevlar is on and unbroken. Stood apart, so neither is
    // slowed for crowding the other.
    hold_still(&mut world);
    let at = world.aboard.room.bim_pos(0);
    world
        .aboard
        .room
        .put_for_probe(1, at + vec2(5.0 * TILE, 0.0));
    for who in [0, 1] {
        world.aboard.room.bleed_for_probe(who, 0.45);
    }
    world.step(&[]);
    let pace = |world: &World, who: usize| world.aboard.room.pace_for_probe(who);
    assert!(
        pace(&world, 0) > pace(&world, 1) * 1.9,
        "{} against {}",
        pace(&world, 0),
        pace(&world, 1)
    );
    // With the kevlar broken it is a pace like anybody else's.
    wear(&mut world, 0, ArmourKind::BasicKevlar, 0.0);
    world.step(&[]);
    assert!((pace(&world, 0) - pace(&world, 1)).abs() < 1e-3);
}

#[test]
fn wide_wall_and_fast_wall() {
    let mut world = tank();
    assert_eq!(world.bulwark_reach(0), class::BULWARK_REACH);
    assert_eq!(world.bulwark_pace(0), class::BULWARK_PACE);
    pick(&mut world, 0, Talent::WideWall);
    assert_eq!(
        world.bulwark_reach(0),
        class::BULWARK_REACH * class::WIDE_WALL_REACH
    );
    assert_eq!(world.bulwark_pace(0), class::BULWARK_PACE);
    world.step(&[Command::Bulwark { slot: 0, on: true }]);
    assert_eq!(
        world.aboard.room.bulwarks_for_probe()[0].reach,
        class::BULWARK_REACH * class::WIDE_WALL_REACH
    );
    let mut world = tank();
    assert_eq!(world.set_class(1, Class::Tank), Ok(()));
    pick(&mut world, 0, Talent::FastWall);
    world.step(&[
        Command::Bulwark { slot: 0, on: true },
        Command::Bulwark { slot: 1, on: true },
    ]);
    assert_eq!(
        world.skill_of(0).walk,
        class::BULWARK_PACE * class::FAST_WALL_PACE
    );
    assert_eq!(world.skill_of(1).walk, class::BULWARK_PACE, "his alone");
    assert_eq!(world.bulwark_reach(0), class::BULWARK_REACH);
}

#[test]
fn loud_taunt_and_long_taunt() {
    let mut world = tank();
    assert_eq!(world.set_class(1, Class::Tank), Ok(()));
    for who in [0, 1] {
        level_up(&mut world, who, class::TAUNT_LEVEL);
    }
    assert_eq!(world.taunt_radius(0), class::TAUNT_RADIUS);
    assert_eq!(world.taunt_minutes(0), class::TAUNT_MINUTES);
    pick(&mut world, 0, Talent::LoudTaunt);
    assert_eq!(
        world.taunt_radius(0),
        class::TAUNT_RADIUS * class::LOUD_TAUNT_RADIUS
    );
    assert_eq!(world.taunt_minutes(0), class::TAUNT_MINUTES, "not longer");
    assert_eq!(world.taunt_radius(1), class::TAUNT_RADIUS, "his alone");
    let mut world = tank();
    assert_eq!(world.set_class(1, Class::Tank), Ok(()));
    for who in [0, 1] {
        level_up(&mut world, who, class::TAUNT_LEVEL);
    }
    pick(&mut world, 0, Talent::LongTaunt);
    assert_eq!(
        world.taunt_minutes(0),
        class::TAUNT_MINUTES * class::LONG_TAUNT_TIME
    );
    assert_eq!(world.taunt_radius(0), class::TAUNT_RADIUS, "not wider");
    world.step(&[Command::Taunt { slot: 0 }, Command::Taunt { slot: 1 }]);
    // Six minutes on, the plain taunt is over and the long one runs on.
    for _ in 0..(class::TAUNT_MINUTES as u32 * STEPS_A_MINUTE) {
        world.step(&[]);
    }
    assert!(world.is_taunting(0));
    assert!(!world.is_taunting(1));
}

#[test]
fn hold_fast_and_guarded() {
    // *Hold fast*: his wounds do not bleed while he taunts.
    let mut world = tank();
    assert_eq!(world.set_class(1, Class::Tank), Ok(()));
    for who in [0, 1] {
        level_up(&mut world, who, class::TAUNT_LEVEL);
    }
    pick(&mut world, 0, Talent::HoldFast);
    hold_still(&mut world);
    // Past the leg guards, so a wound is actually opened on each.
    for who in [0, 1] {
        world.aboard.room.wound(who, Part::Legs, 40.0);
        assert!(world.aboard.room.bleeding(who) > 0);
    }
    world.step(&[Command::Taunt { slot: 0 }, Command::Taunt { slot: 1 }]);
    let blood = |world: &World, who: usize| world.aboard.room.blood(who);
    let (before_0, before_1) = (blood(&world, 0), blood(&world, 1));
    for _ in 0..(2 * STEPS_A_MINUTE) {
        world.step(&[]);
    }
    assert_eq!(blood(&world, 0), before_0, "nothing bleeds while he taunts");
    assert!(blood(&world, 1) < before_1, "the other one bleeds");
    // The taunt over, he bleeds like anybody else.
    for _ in 0..(class::TAUNT_MINUTES as u32 * STEPS_A_MINUTE) {
        world.step(&[]);
    }
    assert!(!world.is_taunting(0));
    let before = blood(&world, 0);
    for _ in 0..(2 * STEPS_A_MINUTE) {
        world.step(&[]);
    }
    assert!(blood(&world, 0) < before);
    // *Guarded*: ten per cent more dodge while the wall is up.
    let mut world = tank();
    assert_eq!(world.set_class(1, Class::Tank), Ok(()));
    pick(&mut world, 0, Talent::Guarded);
    world.step(&[]);
    assert_eq!(world.skill_of(0).dodge, 0.0, "not with the wall down");
    world.step(&[
        Command::Bulwark { slot: 0, on: true },
        Command::Bulwark { slot: 1, on: true },
    ]);
    assert_eq!(world.skill_of(0).dodge, class::GUARDED_DODGE);
    assert_eq!(world.skill_of(1).dodge, 0.0, "his alone");
}

#[test]
fn interpose_and_magnet() {
    // Both are flags the room's shooter reads; what they do to a bolt
    // and to a charge is pinned in `bims::combat`.
    let mut world = tank();
    assert_eq!(world.set_class(1, Class::Tank), Ok(()));
    world.step(&[
        Command::Bulwark { slot: 0, on: true },
        Command::Bulwark { slot: 1, on: true },
    ]);
    let walls = world.aboard.room.bulwarks_for_probe();
    assert!(walls.iter().all(|w| !w.interpose));
    pick(&mut world, 0, Talent::Interpose);
    world.step(&[]);
    let walls = world.aboard.room.bulwarks_for_probe();
    assert!(walls.iter().any(|w| w.who == 0 && w.interpose));
    assert!(
        walls.iter().any(|w| w.who == 1 && !w.interpose),
        "his alone"
    );
    let mut world = tank();
    assert_eq!(world.set_class(1, Class::Tank), Ok(()));
    for who in [0, 1] {
        level_up(&mut world, who, class::TAUNT_LEVEL);
    }
    pick(&mut world, 0, Talent::Magnet);
    assert!(world.has_talent(0, Talent::Magnet));
    assert!(!world.has_talent(1, Talent::Magnet));
}

#[test]
fn fortress_and_rallying_wall() {
    // *Fortress*: a quarter of anybody else's drain, and his alone.
    let mut world = tank();
    assert_eq!(world.set_class(1, Class::Tank), Ok(()));
    pick(&mut world, 0, Talent::Fortress);
    world.step(&[]);
    assert_eq!(
        world.armour_drain(0),
        class::TANK_DRAIN * class::FORTRESS_DRAIN
    );
    assert_eq!(world.armour_drain(1), class::TANK_DRAIN);
    // Both are tanks at the second level, so both wear *plated* — a fixed
    // level since the money rework rather than a pick — and the piece
    // stops half again what it says it does. What comes through drains it
    // at the tank's own rate, and his is a quarter where hers is a half.
    level_up(&mut world, 1, 2);
    world.step(&[]);
    let kevlar = ArmourKind::BasicKevlar;
    let protection = kevlar.stats().protection;
    let through = 8.0 + protection - protection * class::PLATED_PROTECTION;
    for who in [0, 1] {
        wear(&mut world, who, kevlar, 20.0);
    }
    world.aboard.room.wound(0, Part::Body, 8.0 + protection);
    world.aboard.room.wound(1, Part::Body, 8.0 + protection);
    assert_eq!(
        20.0 - worn_health(&world, 0, kevlar),
        through * class::TANK_DRAIN * class::FORTRESS_DRAIN
    );
    assert_eq!(
        20.0 - worn_health(&world, 1, kevlar),
        through * class::TANK_DRAIN
    );
    // *Rallying wall*: the crew within three tiles of him drain at half
    // rate too, while he taunts.
    let mut world = tank();
    assert_eq!(world.set_class(1, Class::None), Ok(()));
    assert_eq!(world.set_class(2, Class::None), Ok(()));
    level_up(&mut world, 0, class::TAUNT_LEVEL);
    pick(&mut world, 0, Talent::RallyingWall);
    let at = world.aboard.room.bim_pos(0);
    world
        .aboard
        .room
        .put_for_probe(1, at + vec2(2.0 * TILE, 0.0));
    let far = world
        .aboard
        .room
        .put_for_probe(2, at + vec2(6.0 * TILE, 0.0));
    assert!((far - at).len() > class::RALLYING_WALL_TILES * TILE);
    world.step(&[]);
    assert_eq!(world.armour_drain(1), 1.0, "not until he taunts");
    world.step(&[Command::Taunt { slot: 0 }]);
    world
        .aboard
        .room
        .put_for_probe(1, at + vec2(2.0 * TILE, 0.0));
    world.aboard.room.put_for_probe(2, far);
    assert_eq!(world.armour_drain(1), class::RALLYING_WALL_DRAIN);
    assert_eq!(world.armour_drain(2), 1.0, "three tiles and no further");
    for _ in 0..(class::TAUNT_MINUTES as u32 * STEPS_A_MINUTE) {
        world
            .aboard
            .room
            .put_for_probe(1, at + vec2(2.0 * TILE, 0.0));
        world.step(&[]);
    }
    assert!(!world.is_taunting(0));
    assert_eq!(world.armour_drain(1), 1.0, "and it ends with the taunt");
}

// --- the seam ----------------------------------------------------------------

#[test]
fn the_checksum_notices_a_wall_a_taunt_and_a_hit_taken() {
    let base = || {
        let mut world = basic();
        assert_eq!(world.set_class(0, Class::Tank), Ok(()));
        world.step(&[]);
        world
    };
    let plain = world_checksum(&base());
    let mut walled = base();
    walled.step(&[Command::Bulwark { slot: 0, on: true }]);
    assert_ne!(world_checksum(&walled), plain, "a wall up is a fight");
    let mut taunting = base();
    level_up(&mut taunting, 0, class::TAUNT_LEVEL);
    let level = world_checksum(&taunting);
    taunting.step(&[Command::Taunt { slot: 0 }]);
    assert_ne!(world_checksum(&taunting), level, "a taunt is too");
    let mut hit = base();
    let before = world_checksum(&hit);
    assert!(shoot_at(&mut hit, 0, 8) > 0);
    assert_ne!(world_checksum(&hit), before);
    // And a tank's kit is a resource nobody's hold moved: the classes
    // and the pieces are hashed, the money is not touched.
    let mut none = basic();
    assert_eq!(none.set_class(0, Class::None), Ok(()));
    none.step(&[]);
    assert_ne!(world_checksum(&none), plain);
    let mut tanked = basic();
    assert_eq!(tanked.set_class(0, Class::Tank), Ok(()));
    tanked.step(&[]);
    for resource in [ResourceId::Helm, ResourceId::Kevlar, ResourceId::LegGuard] {
        assert_eq!(
            none.ship.design.carrying(resource),
            tanked.ship.design.carrying(resource),
            "the tank's kit comes with him, not out of the hold"
        );
    }
}
