//! Task 118's relics: five patches of five — *Dismantler*, *Lifeline*,
//! *Flanker*, *Command Net* and *Supply Line* — and *Kill Relay*'s three
//! seconds. Each is tested where its hook is read: a crew hit landing on a
//! machine, the skill, the blood, a kill, a crewmate down, the clear and
//! the trader.

use bims::combat::{Gear, Hit, WeaponKind};
use bims::droid::DroidPart;
use bims::health::Part;
use bims::math::{Vec2, vec2};
use shipdesign::fixture::{COMBAT_CREW, combat_ship, flyer};
use worldgen::GalaxyType;

use crate::checksum::world_checksum;
use crate::class::{Charge, Class};
use crate::data;
use crate::event::{Refusal, WorldEvent};
use crate::fixture::{REFERENCE_MONEY, crewed_world, simulation_world};
use crate::relic::{self, Relic, Trigger};
use crate::run::Phase;
use crate::world::{Command, MachineKill, World};

// --- the arena --------------------------------------------------------------------

/// The `droids` arena with `players` players, a gun in every hand, one
/// wave of three standing and every machine held where it stands.
fn arena(players: u32) -> World {
    let galaxy = worldgen::Galaxy::new(data::DEFAULT_SEED, GalaxyType::SpiralTwoArm);
    let (star, station) = crate::spawn(&galaxy).unwrap();
    let mut world = World::start_with_crew(
        combat_ship(),
        REFERENCE_MONEY,
        players,
        COMBAT_CREW,
        data::DEFAULT_SEED,
        GalaxyType::SpiralTwoArm,
        star,
        station,
    )
    .unwrap();
    world.arena_dock_for_probe();
    let crew = world.aboard.room.crew_count() as usize;
    for who in 0..crew {
        let gear = world.aboard.room.gear(who);
        world.aboard.room.issue(
            who,
            Gear {
                weapon: Some(WeaponKind::LaserPistol.basic()),
                ..gear
            },
        );
    }
    world.infest(station);
    world.set_droid_reinforce_minutes_for_probe(1.0);
    world.set_droid_waves_for_probe(1);
    world.set_droid_wave_for_probe(3);
    for _ in 0..40 {
        world.step(&[]);
        if world.droids_standing() > 0 {
            break;
        }
    }
    assert!(world.droids_standing() > 0, "the wave stood up");
    let room = &mut world.residents.as_mut().unwrap().aboard.room;
    for i in 0..room.droid_count() as usize {
        room.droid_mut_for_probe(i).unwrap().posing = true;
    }
    world
}

/// The first machine standing, by droid index.
fn a_machine(world: &World) -> usize {
    let room = &world.residents.as_ref().unwrap().aboard.room;
    (0..room.droid_count() as usize)
        .find(|&i| room.droid(i).is_some_and(|d| !d.destroyed))
        .expect("a machine standing")
}

/// A roll that lands on `part` of a machine.
fn roll_for(part: DroidPart) -> f32 {
    (0..1000)
        .map(|k| k as f32 / 1000.0 + 0.0005)
        .find(|&r| DroidPart::hit_by(r) == part)
        .unwrap()
}

/// A hit of `damage` by crew member `by` on machine `i`, rolled onto
/// `part`, landed the way `visit` lands it.
fn shoot(world: &mut World, by: usize, i: usize, part: DroidPart, damage: f32) {
    let bims = world.residents.as_ref().unwrap().aboard.room.crew_count() as usize;
    let hit = Hit {
        who: bims + i,
        part: Part::Body,
        damage,
        cut: false,
        by: Some(by),
        blast: false,
        roll: roll_for(part),
        strips: 0.0,
        flat: 0.0,
        crit: false,
    };
    let rest = world.land_on_machines(vec![hit]);
    assert!(rest.is_empty(), "a machine's hit is the machine's");
}

fn health(world: &World, i: usize, part: DroidPart) -> f32 {
    let room = &world.residents.as_ref().unwrap().aboard.room;
    room.droid(i).unwrap().body.health(part)
}

/// What a hit by `by` on machine `i`, rolled onto `part`, took off
/// `lands_on`, as if it had been a hit of ten: a hit of one is landed, so a
/// test's many hits never run a machine's part down to nothing.
fn taken(world: &mut World, by: usize, i: usize, part: DroidPart, lands_on: DroidPart) -> f32 {
    let before = health(world, i, lands_on);
    shoot(world, by, i, part, 1.0);
    (before - health(world, i, lands_on)) * 10.0
}

fn close(a: f32, b: f32) -> bool {
    (a - b).abs() < 1e-3
}

/// Where crew member `who` stands in the residents' room, as the hits
/// read it.
fn crew_at(world: &World, who: usize) -> Vec2 {
    let shift = world.residents.as_ref().unwrap().aboard.offset;
    let at = world.aboard.crew_ashore()[who].unwrap().add(shift);
    vec2(at.x as f32, at.y as f32)
}

/// Machine `i` turned to face `dir`.
fn face(world: &mut World, i: usize, dir: Vec2) {
    let room = &mut world.residents.as_mut().unwrap().aboard.room;
    room.droid_mut_for_probe(i).unwrap().face_for_probe(dir);
}

fn machine_pos(world: &World, i: usize) -> Vec2 {
    world
        .residents
        .as_ref()
        .unwrap()
        .aboard
        .room
        .droid(i)
        .unwrap()
        .pos
}

// --- Dismantler -----------------------------------------------------------------------

/// **With no relic a hit lands where it was rolled, at its damage**; and
/// **Marksman's Habit** puts its holder's first hit on each machine on a
/// limb, and only the first.
#[test]
fn marksman_s_habit_puts_the_first_hit_on_a_limb() {
    let mut world = arena(2);
    let i = a_machine(&world);
    assert!(close(
        taken(&mut world, 1, i, DroidPart::Chassis, DroidPart::Chassis),
        10.0
    ));
    world.give_relic_for_probe(0, Relic::MarksmansHabit);
    let limbs = |w: &World| health(w, i, DroidPart::Arms) + health(w, i, DroidPart::Legs);
    let (chassis, before) = (health(&world, i, DroidPart::Chassis), limbs(&world));
    shoot(&mut world, 0, i, DroidPart::Chassis, 10.0);
    assert_eq!(
        health(&world, i, DroidPart::Chassis),
        chassis,
        "not the chassis"
    );
    assert!(close(before - limbs(&world), 10.0), "a limb");
    assert!(close(
        taken(&mut world, 0, i, DroidPart::Chassis, DroidPart::Chassis),
        10.0
    ));
    // Another player's first hit on it is its own affair.
    assert!(close(
        taken(&mut world, 1, i, DroidPart::Chassis, DroidPart::Chassis),
        10.0
    ));
}

/// **Servo Cutter** on a limb it still has, **Crippler's Mark** on a
/// machine missing one, and **Total Teardown** on a limb already gone.
#[test]
fn servo_cutter_crippler_s_mark_and_total_teardown() {
    let mut world = arena(1);
    let i = a_machine(&world);
    world.give_relic_for_probe(0, Relic::ServoCutter);
    let f = |p: i32| relic::factor(p) as f32;
    assert!(close(
        taken(&mut world, 0, i, DroidPart::Legs, DroidPart::Legs),
        10.0 * f(data::SERVO_CUTTER_DAMAGE_PERCENT)
    ));
    assert!(close(
        taken(&mut world, 0, i, DroidPart::Chassis, DroidPart::Chassis),
        10.0
    ));
    // The arms gone: crippled — their own health and no more, since
    // every hit comes off the machine's one health (task 137).
    let room = &mut world.residents.as_mut().unwrap().aboard.room;
    let arms = room.droid(i).unwrap().body.max(DroidPart::Arms);
    room.strike_droid(i, DroidPart::Arms, arms);
    world.give_relic_for_probe(0, Relic::CripplersMark);
    assert!(close(
        taken(&mut world, 0, i, DroidPart::Chassis, DroidPart::Chassis),
        10.0 * f(data::CRIPPLERS_MARK_DAMAGE_PERCENT)
    ));
    // A hit on the arms gone lands on the chassis: crippled, never a limb.
    assert!(close(
        taken(&mut world, 0, i, DroidPart::Arms, DroidPart::Chassis),
        10.0 * f(data::CRIPPLERS_MARK_DAMAGE_PERCENT)
    ));
    world.give_relic_for_probe(0, Relic::TotalTeardown);
    assert!(close(
        taken(&mut world, 0, i, DroidPart::Arms, DroidPart::Chassis),
        10.0 * f(data::CRIPPLERS_MARK_DAMAGE_PERCENT) * f(data::TOTAL_TEARDOWN_DAMAGE_PERCENT)
    ));
}

/// **Parts Broker** raises the bounty for a crippled kill of its
/// holder's, and nothing else.
#[test]
fn parts_broker_pays_for_a_crippled_machine() {
    let mut world = crewed_world(flyer(2), REFERENCE_MONEY, 2, 2);
    world.give_relic_for_probe(0, Relic::PartsBroker);
    let bounty = crate::world::bounty_for(1);
    let kill = |by, crippled| MachineKill {
        by: Some(by),
        bounty,
        crippled,
        flanked: false,
    };
    let mut events = Vec::new();
    assert_eq!(
        world.machine_kills_noted(&[kill(0, false)], &mut events),
        bounty
    );
    assert_eq!(
        world.machine_kills_noted(&[kill(1, true)], &mut events),
        bounty
    );
    let raised = bounty + bounty * data::PARTS_BROKER_BOUNTY_PERCENT as u64 / 100;
    assert_eq!(
        world.machine_kills_noted(&[kill(0, true)], &mut events),
        raised
    );
}

// --- Flanker -------------------------------------------------------------------------------

/// **Blind Spot** is more damage from the side or behind; **Wide Angle
/// Optics** makes the side wider.
#[test]
fn blind_spot_and_wide_angle_optics_read_the_machine_s_front() {
    let mut world = arena(1);
    let i = a_machine(&world);
    world.give_relic_for_probe(0, Relic::BlindSpot);
    let toward = (crew_at(&world, 0) - machine_pos(&world, i)).normalize_or_zero();
    let f = relic::factor(data::BLIND_SPOT_DAMAGE_PERCENT) as f32;
    face(&mut world, i, toward);
    assert!(
        close(
            taken(&mut world, 0, i, DroidPart::Chassis, DroidPart::Chassis),
            10.0
        ),
        "front"
    );
    face(&mut world, i, -toward);
    assert!(
        close(
            taken(&mut world, 0, i, DroidPart::Chassis, DroidPart::Chassis),
            10.0 * f
        ),
        "back"
    );
    face(&mut world, i, toward.perp());
    assert!(
        close(
            taken(&mut world, 0, i, DroidPart::Chassis, DroidPart::Chassis),
            10.0 * f
        ),
        "side"
    );
    // Forty-five degrees off: the front for everybody, the side with the
    // optics.
    let (c, s) = (
        std::f32::consts::FRAC_1_SQRT_2,
        std::f32::consts::FRAC_1_SQRT_2,
    );
    face(&mut world, i, toward.rotate_by(c, s));
    assert!(close(
        taken(&mut world, 0, i, DroidPart::Chassis, DroidPart::Chassis),
        10.0
    ));
    world.give_relic_for_probe(0, Relic::WideAngleOptics);
    assert!(close(
        taken(&mut world, 0, i, DroidPart::Chassis, DroidPart::Chassis),
        10.0 * f
    ));
    assert_eq!(world.front_cos_of(0), data::WIDE_ANGLE_OPTICS_FRONT_COS);
    assert_eq!(world.front_cos_of(1), bims::balance::GUARDIAN_SHIELD_COS);
}

/// **Wide Angle Optics** narrows a Guardian's shield against its holder's
/// shots alone: a bolt coming in forty-five degrees off the shield's
/// middle is stopped for everybody else and not for it.
#[test]
fn wide_angle_optics_narrows_the_guardian_s_shield_for_its_holder() {
    use bims::combat::shield_stops_within;
    let heading = vec2(1.0, 0.0);
    let (c, s) = (
        std::f32::consts::FRAC_1_SQRT_2,
        std::f32::consts::FRAC_1_SQRT_2,
    );
    let toward = heading.rotate_by(c, s);
    assert!(shield_stops_within(
        heading,
        toward,
        bims::balance::GUARDIAN_SHIELD_COS
    ));
    assert!(!shield_stops_within(
        heading,
        toward,
        data::WIDE_ANGLE_OPTICS_FRONT_COS
    ));
    assert!(shield_stops_within(
        heading,
        heading,
        data::WIDE_ANGLE_OPTICS_FRONT_COS
    ));
    // And the world hands the room the holder's front, nobody else's.
    let mut world = crewed_world(flyer(2), REFERENCE_MONEY, 2, 2);
    world.give_relic_for_probe(1, Relic::WideAngleOptics);
    assert_eq!(world.front_cos_of(1), data::WIDE_ANGLE_OPTICS_FRONT_COS);
    assert_eq!(world.front_cos_of(0), bims::balance::GUARDIAN_SHIELD_COS);
}

/// **Crossfire**: the holder and a crewmate on opposite sides of a
/// machine both do more to it, and nobody does off to one side.
#[test]
fn crossfire_is_both_sides_of_a_machine() {
    let mut world = arena(1);
    let i = a_machine(&world);
    world.give_relic_for_probe(0, Relic::Crossfire);
    let me = crew_at(&world, 0);
    let reach = data::CROSSFIRE_TILES * shipdesign::TILE as f32;
    let crew = world.aboard.crew_count() as usize;
    let mate = (1..crew)
        .find(|&c| {
            world.aboard.crew_ashore()[c].is_some() && {
                let gap = (crew_at(&world, c) - me).len();
                gap > shipdesign::TILE as f32 && gap < reach
            }
        })
        .expect("a crewmate a few tiles off");
    let between = (me + crew_at(&world, mate)) * 0.5;
    world
        .residents
        .as_mut()
        .unwrap()
        .aboard
        .room
        .droid_mut_for_probe(i)
        .unwrap()
        .pos = between;
    let f = relic::factor(data::CROSSFIRE_DAMAGE_PERCENT) as f32;
    assert!(
        close(
            taken(&mut world, 0, i, DroidPart::Chassis, DroidPart::Chassis),
            10.0 * f
        ),
        "the holder"
    );
    assert!(
        close(
            taken(&mut world, mate, i, DroidPart::Chassis, DroidPart::Chassis),
            10.0 * f
        ),
        "the crewmate"
    );
    // Off to one side of both: no crossfire.
    let side = (crew_at(&world, mate) - me).perp().normalize_or_zero();
    world
        .residents
        .as_mut()
        .unwrap()
        .aboard
        .room
        .droid_mut_for_probe(i)
        .unwrap()
        .pos = me + side * shipdesign::TILE as f32;
    assert!(close(
        taken(&mut world, 0, i, DroidPart::Chassis, DroidPart::Chassis),
        10.0
    ));
}

/// **Signal Scrambler**: a kill from behind hides its holder from every
/// machine for its seconds, and not again inside its cooldown.
#[test]
fn signal_scrambler_hides_its_holder_after_a_kill_from_behind() {
    let mut world = crewed_world(flyer(2), REFERENCE_MONEY, 1, 2);
    world.give_relic_for_probe(0, Relic::SignalScrambler);
    let bounty = crate::world::bounty_for(1);
    let kill = |flanked| MachineKill {
        by: Some(0),
        bounty,
        crippled: false,
        flanked,
    };
    let mut events = Vec::new();
    world.machine_kills_noted(&[kill(false)], &mut events);
    assert!(!world.unseen_by_machines()[0], "from the front: nothing");
    world.machine_kills_noted(&[kill(true)], &mut events);
    assert!(world.unseen_by_machines()[0], "from behind: hidden");
    assert!(!world.unseen_by_machines()[1], "the holder alone");
    run_seconds(&mut world, data::SIGNAL_SCRAMBLER_SECONDS + 0.5);
    assert!(!world.unseen_by_machines()[0], "for its seconds");
    let mut events = Vec::new();
    world.machine_kills_noted(&[kill(true)], &mut events);
    assert!(
        !world.unseen_by_machines()[0],
        "not again inside its cooldown"
    );
    run_seconds(&mut world, data::SIGNAL_SCRAMBLER_COOLDOWN);
    world.machine_kills_noted(&[kill(true)], &mut events);
    assert!(world.unseen_by_machines()[0], "and again after it");
}

/// **Sprint Coil**: its pace up at a mission's start and after an ability,
/// for its seconds.
#[test]
fn sprint_coil_is_a_burst_of_pace() {
    let mut world = crewed_world(flyer(2), REFERENCE_MONEY, 1, 1);
    let walk = world.skill_of(0).walk;
    world.give_relic_for_probe(0, Relic::SprintCoil);
    assert!(
        close(world.skill_of(0).walk, walk),
        "nothing until it fires"
    );
    let mut events = Vec::new();
    world.relic_trigger_on(0, Trigger::AbilityUse, None, &mut events);
    let f = relic::factor(data::SPRINT_COIL_SPEED_PERCENT) as f32;
    assert!(close(world.skill_of(0).walk, walk * f));
    run_seconds(&mut world, data::SPRINT_COIL_SECONDS + 0.5);
    assert!(close(world.skill_of(0).walk, walk), "for its seconds");
    world.relic_trigger_on(0, Trigger::MissionStart, None, &mut events);
    assert!(close(world.skill_of(0).walk, walk * f));
}

// --- Command Net ------------------------------------------------------------------------

/// Crew member `who` stood a tile or `tiles` from crew member 0.
fn stand_near(world: &mut World, who: usize, tiles: f32) {
    let at = world.aboard.room.bim_pos(0);
    let step = vec2(tiles * shipdesign::TILE as f32, 0.0);
    for dir in [step, -step, step.perp(), -step.perp()] {
        world.aboard.room.put_for_probe(who, at + dir);
        let gap = (world.aboard.room.bim_pos(who) - at).len();
        if (gap - tiles * shipdesign::TILE as f32).abs() < shipdesign::TILE as f32 * 0.6 {
            return;
        }
    }
    panic!("nowhere {tiles} tiles from crew member 0");
}

/// **Field Radio** lifts a crewmate's odds within its tiles, never its
/// holder's; **Cover Formation** takes a share off every hit on a bot
/// within its tiles, never a player's.
#[test]
fn field_radio_and_cover_formation_are_auras() {
    let mut world = crewed_world(flyer(2), REFERENCE_MONEY, 2, 3);
    let aim = world.skill_of(1).accuracy;
    let own = world.skill_of(0).accuracy;
    world.give_relic_for_probe(0, Relic::FieldRadio);
    world.give_relic_for_probe(0, Relic::CoverFormation);
    stand_near(&mut world, 1, 1.0);
    stand_near(&mut world, 2, 1.0);
    let radio = relic::factor(data::FIELD_RADIO_ACCURACY_PERCENT) as f32;
    let cover = relic::factor(-data::COVER_FORMATION_PERCENT) as f32;
    assert!(
        close(world.skill_of(1).accuracy, aim * radio),
        "a player beside it"
    );
    assert!(close(world.skill_of(0).accuracy, own), "never its own");
    assert!(
        close(world.skill_of(2).damage_taken, cover),
        "a bot beside it"
    );
    assert!(close(world.skill_of(1).damage_taken, 1.0), "a bot's alone");
    stand_near(&mut world, 1, 8.0);
    stand_near(&mut world, 2, 8.0);
    assert!(close(world.skill_of(1).accuracy, aim), "out of its tiles");
    assert!(close(world.skill_of(2).damage_taken, 1.0));
}

/// **Spotter**: the machine its holder hit last takes more from every
/// crewmate for its seconds.
#[test]
fn spotter_marks_the_machine_it_hit_last() {
    let mut world = arena(1);
    let i = a_machine(&world);
    world.give_relic_for_probe(0, Relic::Spotter);
    let f = relic::factor(data::SPOTTER_DAMAGE_PERCENT) as f32;
    assert!(
        close(
            taken(&mut world, 3, i, DroidPart::Chassis, DroidPart::Chassis),
            10.0
        ),
        "unmarked"
    );
    assert!(
        close(
            taken(&mut world, 0, i, DroidPart::Chassis, DroidPart::Chassis),
            10.0
        ),
        "the mark"
    );
    assert!(
        close(
            taken(&mut world, 3, i, DroidPart::Chassis, DroidPart::Chassis),
            10.0 * f
        ),
        "marked"
    );
    world.run.mission_steps +=
        ((data::SPOTTER_SECONDS + 0.5) * time::MINUTES_PER_SECOND / data::STEP_MINUTES) as u64;
    assert!(
        close(
            taken(&mut world, 3, i, DroidPart::Chassis, DroidPart::Chassis),
            10.0
        ),
        "gone"
    );
}

/// **Squad Morale**: a second off its holder's class cooldowns for its own
/// kill and a bot's; with **Kill Relay** four for its own. Kill Relay is
/// three seconds now.
#[test]
fn squad_morale_and_kill_relay_take_seconds_off_for_kills() {
    let mut world = crewed_world(flyer(2), REFERENCE_MONEY, 2, 3);
    world.set_class(0, Class::Soldier).unwrap();
    world.give_relic_for_probe(0, Relic::SquadMorale);
    world.give_relic_for_probe(0, Relic::KillRelay);
    let bounty = crate::world::bounty_for(1);
    let kill = |by| MachineKill {
        by: Some(by),
        bounty,
        crippled: false,
        flanked: false,
    };
    let seconds_off = |world: &mut World, by| {
        let now = world.mission_minutes();
        world.charge_timers[0][Charge::Grenade.code() as usize] = Some(now);
        let left = world.charge_cooldown_left(0, Charge::Grenade);
        let mut events = Vec::new();
        world.machine_kills_noted(&[kill(by)], &mut events);
        left - world.charge_cooldown_left(0, Charge::Grenade)
    };
    let own = seconds_off(&mut world, 0);
    assert!((own - 4.0).abs() < 1e-6, "{own}");
    let bot = seconds_off(&mut world, 2);
    assert!((bot - data::SQUAD_MORALE_SECONDS).abs() < 1e-6, "{bot}");
    let player = seconds_off(&mut world, 1);
    assert!(player.abs() < 1e-6, "another player's kill is not a bot's");
}

/// **Rally Point**: once a mission, an ability used gets every crewmate
/// down within its tiles up again.
#[test]
fn rally_point_gets_the_downed_up_once_a_mission() {
    let mut world = crewed_world(flyer(2), REFERENCE_MONEY, 2, 2);
    world.give_relic_for_probe(0, Relic::RallyPoint);
    stand_near(&mut world, 1, 2.0);
    world.aboard.room.knock_out_for_probe(1);
    world.step(&[]);
    assert!(world.aboard.room.is_down(1));
    let mut events = Vec::new();
    world.relic_trigger_on(0, Trigger::AbilityUse, None, &mut events);
    assert!(!world.aboard.room.is_down(1), "up again");
    assert!(world.health_share(1) >= data::RALLY_POINT_HEALTH_PERCENT as f32 / 100.0 - 1e-3);
    assert!(events.contains(&WorldEvent::RelicFired {
        who: 0,
        relic: Relic::RallyPoint.code()
    }));
    world.aboard.room.knock_out_for_probe(1);
    world.step(&[]);
    world.relic_trigger_on(0, Trigger::AbilityUse, None, &mut events);
    assert!(world.aboard.room.is_down(1), "once a mission");
}

// --- Lifeline ---------------------------------------------------------------------------

/// Steps a world `seconds` of the mission clock on.
fn run_seconds(world: &mut World, seconds: f64) -> Vec<WorldEvent> {
    let steps = (seconds * time::MINUTES_PER_SECOND / data::STEP_MINUTES).ceil() as u32;
    let mut events = Vec::new();
    for _ in 0..steps {
        events.extend(world.step(&[]));
    }
    events
}

/// Crew member `who` bared of its armour and hit for `damage` on the body:
/// short of health, and nothing between it and the next hit.
fn hurt(world: &mut World, who: usize, damage: f32) {
    world.aboard.room.issue(who, Gear::issued());
    world.aboard.room.wound(who, Part::Body, damage);
}

/// **Pressure Seal** puts health back all the time, **Clot Booster** for
/// its seconds after going down — once it is revived, a downed body being
/// healed by nothing else — and **Quick Wrap** with every revive (task
/// 120: it was every dressing).
#[test]
fn pressure_seal_clot_booster_and_quick_wrap_heal_hit_points() {
    // What each of two Bims hurt alike gained over `seconds`: the one
    // without a relic is the room's own slow mending, which both have.
    let gains = |world: &mut World, seconds: f64| {
        let before = [0, 1].map(|who| world.aboard.room.health(who));
        run_seconds(world, seconds);
        [0, 1].map(|who| world.aboard.room.health(who) - before[who])
    };
    let mut world = crewed_world(flyer(2), REFERENCE_MONEY, 2, 2);
    world.give_relic_for_probe(0, Relic::PressureSeal);
    hurt(&mut world, 0, 30.0);
    hurt(&mut world, 1, 30.0);
    let [holder, other] = gains(&mut world, 4.0);
    let want = data::PRESSURE_SEAL_HP_PER_SECOND * 4.0;
    assert!(
        (holder - other - want).abs() < 0.05,
        "{holder} {other} against {want}"
    );
    assert!(other < want / 2.0, "the holder alone");

    // Quick Wrap: a crewmate it revives is health back on top.
    world.give_relic_for_probe(1, Relic::QuickWrap);
    let mut events = Vec::new();
    let before = world.aboard.room.health(0);
    world.relic_trigger_on(1, Trigger::Revived, Some(0), &mut events);
    let gained = world.aboard.room.health(0) - before;
    assert!(close(gained, data::QUICK_WRAP_HEAL), "{gained}");

    // Clot Booster: health back once revived, for its seconds after going
    // down and no longer.
    let mut world = crewed_world(flyer(2), REFERENCE_MONEY, 2, 2);
    world.give_relic_for_probe(1, Relic::ClotBooster);
    hurt(&mut world, 0, 60.0);
    hurt(&mut world, 1, 60.0);
    let [plain, holder] = gains(&mut world, 2.0);
    assert!((holder - plain).abs() < 0.01, "nothing before it went down");
    for who in [0, 1] {
        world.aboard.room.knock_out_for_probe(who);
    }
    world.step(&[]);
    assert!(world.aboard.room.is_down(0) && world.aboard.room.is_down(1));
    let [plain, holder] = gains(&mut world, 1.0);
    assert!((holder - plain).abs() < 0.01, "nothing heals a downed body");
    for who in [0, 1] {
        world.aboard.room.bring_round(who, bims::health::REVIVED_TO);
    }
    world.step(&[]);
    let [plain, holder] = gains(&mut world, 5.0);
    let want = data::CLOT_BOOSTER_HP_PER_SECOND * 5.0;
    assert!(
        (holder - plain - want).abs() < 0.5,
        "{holder} {plain} against {want}"
    );
    run_seconds(&mut world, data::CLOT_BOOSTER_SECONDS);
    let [plain, holder] = gains(&mut world, 3.0);
    assert!((holder - plain).abs() < 0.01, "its seconds are up");
}

/// **Tether Field**: a crewmate its holder revives takes less of every hit
/// for its seconds — in the room, where the hit lands (task 120: it was a
/// crewmate dressed).
#[test]
fn tether_field_shelters_the_crewmate_revived() {
    let mut world = crewed_world(flyer(2), REFERENCE_MONEY, 2, 2);
    world.give_relic_for_probe(0, Relic::TetherField);
    let mut events = Vec::new();
    world.relic_trigger_on(0, Trigger::Revived, Some(1), &mut events);
    let f = relic::factor(-data::TETHER_FIELD_PERCENT) as f32;
    assert!(close(world.skill_of(1).damage_taken, f));
    assert!(close(world.skill_of(0).damage_taken, 1.0), "the crewmate's");
    world.aboard.room.issue(1, Gear::issued());
    world.step(&[]);
    let before = world.aboard.room.health(1);
    world.aboard.room.wound(1, Part::Legs, 8.0);
    assert!(close(before - world.aboard.room.health(1), 8.0 * f));
    run_seconds(&mut world, data::TETHER_FIELD_SECONDS + 0.5);
    assert!(
        close(world.skill_of(1).damage_taken, 1.0),
        "for its seconds"
    );
}

/// **Lifeline**: once a mission, a crewmate going down near its holder
/// makes the two of them untouchable.
#[test]
fn lifeline_shelters_a_crewmate_going_down_once_a_mission() {
    let mut world = crewed_world(flyer(2), REFERENCE_MONEY, 2, 2);
    world.give_relic_for_probe(0, Relic::Lifeline);
    stand_near(&mut world, 1, 2.0);
    world.aboard.room.knock_out_for_probe(1);
    let events = world.step(&[]);
    assert!(world.aboard.room.is_down(1));
    assert!(events.contains(&WorldEvent::RelicFired {
        who: 0,
        relic: Relic::Lifeline.code()
    }));
    assert!(world.aboard.room.is_surging(0) && world.aboard.room.is_surging(1));
    run_seconds(&mut world, f64::from(data::LIFELINE_SECONDS) + 0.5);
    assert!(!world.aboard.room.is_surging(0), "for its seconds");
    // Up and down again the same mission: nothing.
    world.aboard.room.bring_round(1, 0.5);
    world.step(&[]);
    world.aboard.room.knock_out_for_probe(1);
    world.step(&[]);
    assert!(world.aboard.room.is_down(1));
    assert!(!world.aboard.room.is_surging(0), "once a mission");
}

// --- Supply Line ---------------------------------------------------------------------------

/// **Scrap Collector** is money on its holder's own kills, pending with
/// the bounty.
#[test]
fn scrap_collector_pays_for_its_holder_s_kills() {
    let mut world = crewed_world(flyer(2), REFERENCE_MONEY, 2, 2);
    world.give_relic_for_probe(0, Relic::ScrapCollector);
    let bounty = crate::world::bounty_for(1);
    let kill = |by| MachineKill {
        by: Some(by),
        bounty,
        crippled: false,
        flanked: false,
    };
    let mut events = Vec::new();
    assert_eq!(
        world.machine_kills_noted(&[kill(0)], &mut events),
        bounty + data::SCRAP_COLLECTOR_PAY
    );
    assert_eq!(world.machine_kills_noted(&[kill(1)], &mut events), bounty);
}

/// **Hazard Pay**: a site cleared pays the crew.
#[test]
fn hazard_pay_pays_a_site_cleared() {
    let mut world = arena(1);
    world.give_relic_for_probe(0, Relic::HazardPay);
    let room = &mut world.residents.as_mut().unwrap().aboard.room;
    for i in 0..room.droid_count() as usize {
        room.strike_droid(i, DroidPart::Chassis, 1e6);
    }
    let money = world.money;
    let mut events = Vec::new();
    for _ in 0..200 {
        events.extend(world.step(&[]));
        if world.run.cleared_here {
            break;
        }
    }
    assert!(events.contains(&WorldEvent::RelicFired {
        who: 0,
        relic: Relic::HazardPay.code()
    }));
    let bounty: u64 = events
        .iter()
        .map(|e| match e {
            WorldEvent::Bounty { amount } => *amount,
            _ => 0,
        })
        .sum();
    assert_eq!(world.money, money + bounty + data::HAZARD_PAY);
}

/// **War Chest**: its damage up with the pool a player, to its cap.
#[test]
fn war_chest_reads_the_pool_a_player() {
    let mut world = crewed_world(flyer(2), REFERENCE_MONEY, 2, 2);
    let base = world.skill_of(0).damage;
    world.give_relic_for_probe(0, Relic::WarChest);
    world.money = 10_999;
    // Five thousand and a bit a player: five steps.
    assert_eq!(
        world.war_chest_percent(0),
        5 * data::WAR_CHEST_PERCENT_PER_THOUSAND
    );
    let f = relic::factor(world.war_chest_percent(0)) as f32;
    assert!(close(world.skill_of(0).damage, base * f));
    world.money = 100_000_000;
    assert_eq!(world.war_chest_percent(0), data::WAR_CHEST_CAP_PERCENT);
    world.money = 0;
    assert!(close(world.skill_of(0).damage, base));
    assert_eq!(world.war_chest_percent(1), 0, "the holder's");
}

/// A world at a trader.
fn at_a_trader() -> World {
    let mut world = simulation_world(flyer(2), 10_000_000, 1);
    world.leave_for_probe();
    assert_eq!(world.run.phase, Phase::Map);
    let site = world
        .trader_sites()
        .into_iter()
        .find(|&s| world.travel_quote(s).is_ok())
        .expect("a trader near home");
    world.step(&[Command::Propose {
        slot: 0,
        star: site.star,
        station: site.station,
    }]);
    assert_eq!(world.run.phase, Phase::Trade);
    world
}

/// **Trade License** takes its share off the shelf and the relic.
#[test]
fn trade_license_is_cheaper_at_the_trader() {
    let mut world = at_a_trader();
    let item = world.trader_here().unwrap().shelf[0].unwrap();
    let price = world.shelf_price(item);
    let relic_price = world.trader_relic_price(Relic::FocusingLens);
    world.give_relic_for_probe(0, Relic::TradeLicense);
    let off = |p: u64| p - p * data::TRADE_LICENSE_PERCENT as u64 / 100;
    assert_eq!(world.shelf_price(item), off(price));
    assert_eq!(
        world.trader_relic_price(Relic::FocusingLens),
        off(relic_price)
    );
}

/// **Restock Codes**: the shelf rolled again once a visit — the relic kept
/// — and refused without it or twice.
#[test]
fn restock_codes_rolls_the_shelf_again_once_a_visit() {
    let mut world = at_a_trader();
    let events = world.step(&[Command::Restock { slot: 0 }]);
    assert!(events.iter().any(|e| matches!(
        e,
        WorldEvent::Refused {
            why: Refusal::NoRestock,
            ..
        }
    )));
    world.give_relic_for_probe(0, Relic::RestockCodes);
    let shelf = world.trader_here().unwrap().shelf.clone();
    let relic = world.trader_here().unwrap().relic;
    let before = world_checksum(&world);
    let events = world.step(&[Command::Restock { slot: 0 }]);
    assert!(
        events.contains(&WorldEvent::Restocked { slot: 0 }),
        "{events:?}"
    );
    let after = world.trader_here().unwrap();
    assert_ne!(after.shelf, shelf, "another shelf");
    assert_eq!(after.shelf.len(), shelf.len());
    assert_eq!(after.relic, relic, "the relic is never rolled");
    assert_ne!(world_checksum(&world), before);
    let events = world.step(&[Command::Restock { slot: 0 }]);
    assert!(events.iter().any(|e| matches!(
        e,
        WorldEvent::Refused {
            why: Refusal::Restocked,
            ..
        }
    )));
}

// --- two clients --------------------------------------------------------------------------

/// **Two worlds holding the same relics fight the same fight**: every
/// relic of the five patches on the two players, the arena stepped, and
/// the checksum alike step for step.
#[test]
fn two_worlds_with_every_new_relic_agree_step_for_step() {
    let build = || {
        let mut world = arena(2);
        let room = &mut world.residents.as_mut().unwrap().aboard.room;
        for i in 0..room.droid_count() as usize {
            room.droid_mut_for_probe(i).unwrap().posing = false;
        }
        for (k, &relic) in Relic::ALL[12..].iter().enumerate() {
            world.give_relic_for_probe(k as u32 % 2, relic);
        }
        world
    };
    let (mut a, mut b) = (build(), build());
    for step in 0..600 {
        let (ea, eb) = (a.step(&[]), b.step(&[]));
        assert_eq!(ea, eb, "step {step}");
        assert_eq!(world_checksum(&a), world_checksum(&b), "step {step}");
    }
}
