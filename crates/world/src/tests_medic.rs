//! The medic class (feature 76; its ranked kit task 130, reworked by task
//! 153): `crate::class`'s third class. The sixteen levels and a skill
//! point a level; the two base traits — a revive in four seconds, and up
//! at two fifths of the bar; the four abilities a rank at a time — Heal
//! Drone, Triage, Heal Beam and Healing Circle — each doing what its rank
//! says, and the Override Core's half again; and carrying a downed
//! crewmate out of the fire.

use bims::health::MAX_HEALTH;
use bims::math::{Vec2, vec2};
use bims::order::CrewOrder;
use shipdesign::fixture::combat_ship;

use crate::class::{self, Class};
use crate::event::{Refusal, WorldEvent};
use crate::fixture::{REFERENCE_MONEY, simulation_world};
use crate::world::{Command, World};
use crate::world_checksum;

const TILE: f32 = shipdesign::TILE as f32;

/// Steps of the world a minute of its clock is — a second at 1×.
const STEPS_A_MINUTE: u32 = 60;

/// Seconds of the mission clock a world step is.
const SECONDS_A_STEP: f64 = crate::data::STEP_MINUTES / time::MINUTES_PER_SECOND;

/// The combat ship, bunks for two and more, with two players' crew.
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

/// [`medic`] with exactly these ranks bought, Q C E R.
fn medic_at(want: [u8; 4]) -> World {
    let mut world = medic();
    ranks(&mut world, 0, want);
    beside(&mut world, 1, 0);
    world
}

/// The crew held where they stand: their own errands off, and everybody
/// recruited, so nobody wanders off from a beam six tiles long. What the
/// player orders still goes.
fn hold_still(world: &mut World) {
    world.aboard.room.set_autonomous(false);
    for who in 0..world.aboard.crew_count() as usize {
        // On a cell a body fits in first: where the crew wake up is
        // against the furniture, and the push-out walks one off it.
        let at = world.aboard.room.bim_pos(who);
        world.aboard.room.put_for_probe(who, at);
        world.aboard.room.recruit_for_probe(who, true);
    }
    // And nobody revives of their own accord: these tests want the hands
    // they name.
    world
        .aboard
        .room
        .set_work_priority(bims::work::Job::Medical as u32, bims::work::NEVER);
}

/// Crew member `who` stood a tile from `of`.
fn beside(world: &mut World, who: usize, of: usize) -> Vec2 {
    stand_off(world, who, of, 1.0)
}

/// Crew member `who` stood `tiles` tiles east of `of`.
fn stand_off(world: &mut World, who: usize, of: usize, tiles: f32) -> Vec2 {
    let at = world.aboard.room.bim_pos(of) + vec2(tiles * TILE, 0.0);
    let spot = world.aboard.room.put_for_probe(who, at);
    world.step(&[]);
    spot
}

/// The medic and crew member 1 stood `tiles` apart on open deck with
/// nothing between them, near where the medic is — for a circle's reach,
/// which a corner clipped on a diagonal stops.
fn in_the_open(world: &mut World, tiles: f32) {
    let here = world.aboard.room.bim_pos(0);
    let snap = |p: Vec2| {
        vec2(
            ((p.x / TILE).floor() + 0.5) * TILE,
            ((p.y / TILE).floor() + 0.5) * TILE,
        )
    };
    for r in 0..20i32 {
        for mx in -r..=r {
            for my in -r..=r {
                let m = snap(here + vec2(mx as f32 * TILE, my as f32 * TILE));
                let p = m + vec2(tiles * TILE, 0.0);
                let room = &world.aboard.room;
                if !room.is_deck_tile(m) || !room.is_deck_tile(p) || !room.line_clear(m, p) {
                    continue;
                }
                let a = world.aboard.room.put_for_probe(0, m);
                let b = world.aboard.room.put_for_probe(1, p);
                if ((b - a).len() - tiles * TILE).abs() < 0.3 * TILE
                    && world.aboard.room.line_clear(a, b)
                {
                    world.step(&[]);
                    return;
                }
            }
        }
    }
    panic!("no open deck {tiles} tiles long");
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
    assert!(world.level_of(who as u32) >= level);
}

/// Ranks bought one at a time through `Command::RankUp`, Q C E R, the
/// level raised first to what the highest wants.
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

fn beam(world: &mut World, slot: u32, patient: Option<u32>) -> Vec<WorldEvent> {
    world.step(&[Command::Beam { slot, patient }])
}

fn linked(events: &[WorldEvent], who: u32, patient: Option<u32>) -> bool {
    events
        .iter()
        .any(|e| *e == WorldEvent::Beamed { who, patient })
}

/// `who` at `points` of its bar: what a heal is measured against, since a
/// whole bar gains nothing whatever the rate.
fn hurt(world: &mut World, who: usize, points: f32) {
    world.aboard.room.set_health_for_probe(who, points);
    world.step(&[]);
}

/// Steps enough for `seconds` of the mission clock.
fn run_for(world: &mut World, seconds: f64) {
    for _ in 0..(seconds / SECONDS_A_STEP).ceil() as u32 {
        world.step(&[]);
    }
}

/// What `minutes` of the clock of a first-rank beam put back.
fn beam_gain(minutes: f32) -> f32 {
    class::HEAL_BEAM_HP / 60.0 * minutes
}

/// `who`'s whole bar over a hundred: what a heal's per cents are
/// multiplied by (October 2026, every heal a share of the bar).
fn bar(world: &World, who: usize) -> f32 {
    world.aboard.room.max_health(who) / 100.0
}

/// The mission left — if one is under way — and another begun somewhere
/// else in the system, the way the crew do it. The events of the step
/// the trip was taken.
fn next_mission(world: &mut World) -> Vec<WorldEvent> {
    if world.run.phase == crate::run::Phase::Mission {
        world.leave_for_probe();
    }
    let here = world.current_site();
    let site = world
        .sites_at(world.star_id)
        .into_iter()
        .find(|&s| Some(s) != here && world.travel_quote(s).is_ok_and(|q| !q.trader))
        .expect("somewhere else to go");
    let mut events = world.step(&[Command::Propose {
        slot: 0,
        star: site.star,
        station: site.station,
    }]);
    for slot in 1..world.players() {
        events.extend(world.step(&[Command::Accept { slot, yes: true }]));
    }
    assert_eq!(world.run.phase, crate::run::Phase::Mission);
    events
}

/// A spot for the medic and one for crew member 1 within `tiles` of it
/// with a wall between them, stood there — for what walls stop. `None`
/// if the deck round the medic has no such pair.
fn put_behind_a_wall(world: &mut World, tiles: f32) -> Option<()> {
    let here = world.aboard.room.bim_pos(0);
    let snap = |p: Vec2| {
        vec2(
            ((p.x / TILE).floor() + 0.5) * TILE,
            ((p.y / TILE).floor() + 0.5) * TILE,
        )
    };
    let reach = tiles as i32;
    for mx in -20..=20 {
        for my in -20..=20 {
            let m = snap(here + vec2(mx as f32 * TILE, my as f32 * TILE));
            if !world.aboard.room.is_deck_tile(m) {
                continue;
            }
            for dx in -reach..=reach {
                for dy in -reach..=reach {
                    let p = m + vec2(dx as f32 * TILE, dy as f32 * TILE);
                    if (p - m).len() > (tiles - 0.5) * TILE
                        || !world.aboard.room.is_deck_tile(p)
                        || world.aboard.room.line_clear(m, p)
                    {
                        continue;
                    }
                    let a = world.aboard.room.put_for_probe(0, m);
                    let b = world.aboard.room.put_for_probe(1, p);
                    if (b - a).len() <= (tiles - 0.2) * TILE
                        && !world.aboard.room.line_clear(a, b)
                        && !world.aboard.room.sees_for_probe(0, b)
                    {
                        world.step(&[]);
                        return Some(());
                    }
                }
            }
        }
    }
    None
}

// --- A: the rank system and the base traits ---------------------------------------

/// **The medic climbs sixteen levels and buys a rank a point** (task
/// 130), through the same gates as the soldier; the tank is still
/// refused.
#[test]
fn the_medic_climbs_sixteen_levels_and_buys_ranks_the_tank_is_refused() {
    let mut world = medic();
    assert!(class::ranked(Class::Medic));
    level_up(&mut world, 0, 16);
    assert_eq!(world.progress_of(0).xp, 3_200);
    assert_eq!(world.level_of(0), 16);
    assert_eq!(world.points_of(0), 16);
    // Every gate: a fresh medic at the first level buys Q, C or E's first
    // rank and nothing of R's, and one point is one rank.
    let mut world = medic();
    let events = world.step(&[Command::RankUp {
        slot: 0,
        ability_slot: class::SLOT_R as u32,
    }]);
    assert!(refused_with(&events, Refusal::RankLocked), "{events:?}");
    let events = world.step(&[Command::RankUp {
        slot: 0,
        ability_slot: class::SLOT_Q as u32,
    }]);
    assert!(events.iter().any(|e| matches!(
        e,
        WorldEvent::RankedUp {
            who: 0,
            ability_slot: 0,
            rank: 1,
            ..
        }
    )));
    let events = world.step(&[Command::RankUp {
        slot: 0,
        ability_slot: class::SLOT_C as u32,
    }]);
    assert!(refused_with(&events, Refusal::NoSkillPoint), "{events:?}");
    ranks(&mut world, 0, [4, 4, 4, 4]);
    assert_eq!(world.level_of(0), 16);
    assert_eq!(world.points_of(0), 0);
    let events = world.step(&[Command::RankUp {
        slot: 0,
        ability_slot: class::SLOT_Q as u32,
    }]);
    assert!(
        refused_with(&events, Refusal::TopRank) || refused_with(&events, Refusal::NoSkillPoint),
        "{events:?}"
    );
    // The tank buys his the same way since task 139.
    let mut world = basic();
    assert_eq!(world.set_class(1, Class::Tank), Ok(()));
    let events = world.step(&[Command::RankUp {
        slot: 1,
        ability_slot: class::SLOT_Q as u32,
    }]);
    assert!(
        events
            .iter()
            .any(|e| matches!(e, WorldEvent::RankedUp { who: 1, .. })),
        "{events:?}"
    );
}

/// **The surge is gone** (task 130), and so are the burst, the aura and
/// the cloak (task 153): the medic's abilities are the beam, the drone
/// and the circle, and its Q is the drone.
#[test]
fn the_medic_s_abilities_are_the_beam_the_drone_and_the_circle() {
    let own: Vec<class::Ability> = class::Ability::ALL
        .into_iter()
        .filter(|&a| class::can(Class::Medic, a))
        .collect();
    assert_eq!(
        own,
        vec![
            class::Ability::Beam,
            class::Ability::HealDrone,
            class::Ability::HealingCircle
        ]
    );
    let mut world = medic_at([1, 0, 0, 0]);
    let events = world.step(&[Command::HealDrone { slot: 0 }]);
    assert!(
        events
            .iter()
            .any(|e| *e == WorldEvent::DroneLaunched { who: 0 }),
        "Q is the drone: {events:?}"
    );
}

/// Crew member `helper` ordered to revive `patient`, run until it is up.
/// How much of its bar it got up at.
fn revived_by(world: &mut World, helper: u32, patient: usize) -> f32 {
    world.aboard.room.knock_out_for_probe(patient);
    world.step(&[]);
    assert!(world.aboard.room.is_downed(patient), "downed");
    world.step(&[Command::Crew {
        slot: helper,
        order: CrewOrder::Revive {
            who: helper,
            patient: patient as u32,
        },
    }]);
    for _ in 0..(20 * STEPS_A_MINUTE) {
        world.step(&[]);
        if !world.aboard.room.is_downed(patient) {
            return world.aboard.room.health(patient);
        }
    }
    panic!("{helper} never got {patient} up");
}

/// **A medic revives in four seconds and gets the crewmate up at two
/// fifths of its bar; anybody else in ten, at three tenths** (task 130's
/// base traits). A hired field medic is as quick as a medic, but its
/// patient gets up at anybody's share. Triage does not touch a
/// revive: it is not a heal.
#[test]
fn a_medic_revives_in_four_seconds_to_two_fifths_and_anybody_else_to_three_tenths() {
    let mut world = simulation_world(combat_ship(), REFERENCE_MONEY, 3);
    assert_eq!(world.revive_seconds(0), bims::health::REVIVE_SECONDS);
    assert_eq!(world.set_class(0, Class::Medic), Ok(()));
    hold_still(&mut world);
    // Triage at its top: still two fifths, a revive being no heal.
    ranks(&mut world, 0, [0, 4, 0, 0]);
    assert_eq!(world.revive_seconds(0), class::MEDIC_REVIVE_SECONDS);
    assert_eq!(world.skill_of(0).revive, class::MEDIC_REVIVE_SECONDS);
    assert_eq!(world.skill_of(0).revived_to, class::MEDIC_REVIVED_TO);
    assert_eq!(world.revive_seconds(1), bims::health::REVIVE_SECONDS);
    assert_eq!(world.skill_of(1).revived_to, bims::health::REVIVED_TO);
    beside(&mut world, 2, 0);
    let up = revived_by(&mut world, 0, 2);
    assert_eq!(up, MAX_HEALTH * class::MEDIC_REVIVED_TO, "a medic's");
    beside(&mut world, 1, 2);
    let up = revived_by(&mut world, 1, 2);
    assert_eq!(up, MAX_HEALTH * bims::health::REVIVED_TO, "anybody else's");
    // A field medic: a medic's time, anybody's share.
    assert!(world.field_medic_for_probe(2));
    assert_eq!(world.revive_seconds(2), class::MEDIC_REVIVE_SECONDS);
    assert_eq!(world.skill_of(2).revived_to, bims::health::REVIVED_TO);
    world.step(&[]);
    assert_eq!(
        world.aboard.room.skill_for_probe(0).revived_to,
        class::MEDIC_REVIVED_TO,
        "the room is told"
    );
}

// --- B: Q, the Heal Drone (task 153) ------------------------------------------------

fn drone(world: &mut World) -> Vec<WorldEvent> {
    world.step(&[Command::HealDrone { slot: 0 }])
}

/// **The drone's heal, time and cooldown by rank**; it flies to the Bim
/// lowest on its bar, heals it its rank's points a second once over it,
/// is gone when its time is up, and the next waits the cooldown.
#[test]
fn the_heal_drone_flies_to_the_lowest_and_heals_it_by_rank() {
    let mut world = medic();
    assert!(refused_with(&drone(&mut world), Refusal::NotLearnt));
    let events = world.step(&[Command::HealDrone { slot: 1 }]);
    assert!(refused_with(&events, Refusal::NotAMedic), "{events:?}");
    let want = [
        (1.8, 8.0, 25.0),
        (2.0, 10.0, 22.0),
        (2.5, 12.0, 20.0),
        (3.0, 14.0, 18.0),
    ];
    for (rank, &(heal, seconds, cooldown)) in (1..=4u8).zip(&want) {
        let mut world = medic_at([rank, 0, 0, 0]);
        assert_eq!(world.heal_drone_heal(0), heal, "rank {rank}");
        assert_eq!(world.heal_drone_seconds(0), seconds, "rank {rank}");
        assert_eq!(world.heal_drone_cooldown(0), cooldown, "rank {rank}");
        stand_off(&mut world, 1, 0, 3.0);
        hurt(&mut world, 1, 30.0);
        assert!(
            drone(&mut world).contains(&WorldEvent::DroneLaunched { who: 0 }),
            "rank {rank}"
        );
        assert!(refused_with(&drone(&mut world), Refusal::CoolingDown));
        // A second to get there — three tiles at six a second — and the
        // patient is the one hurt.
        run_for(&mut world, 1.0);
        assert_eq!(world.drone_of(0).and_then(|d| d.patient), Some(1));
        let before = world.aboard.room.health(1);
        run_for(&mut world, 4.0);
        let gained = world.aboard.room.health(1) - before;
        assert!(
            (gained - heal * 4.0).abs() < 0.2,
            "rank {rank}: four seconds at {heal}: {gained}"
        );
        // Gone when its time is up, and ready again after the cooldown.
        run_for(&mut world, seconds - 5.0 + 0.1);
        assert!(world.drone_of(0).is_none(), "rank {rank}: gone");
        run_for(&mut world, cooldown - seconds);
        assert_eq!(world.can_heal_drone(0), Ok(()), "rank {rank}");
    }
}

/// **The lowest by share, the medic included; over walls; and by him
/// with nobody hurt.**
#[test]
fn the_drone_picks_the_lowest_share_flies_over_walls_and_keeps_by_the_medic() {
    // The medic lower than his crewmate: the drone is his.
    let mut world = medic_at([4, 0, 0, 0]);
    hurt(&mut world, 1, 60.0);
    hurt(&mut world, 0, 20.0);
    drone(&mut world);
    run_for(&mut world, 0.5);
    assert_eq!(world.drone_of(0).and_then(|d| d.patient), Some(0));
    // Nobody hurt: no patient, and it hangs about him.
    let mut world = medic_at([4, 0, 0, 0]);
    drone(&mut world);
    run_for(&mut world, 1.0);
    let d = world.drone_of(0).expect("in the air");
    assert_eq!(d.patient, None);
    assert!((vec2(d.x, d.y) - world.aboard.room.bim_pos(0)).len() < TILE);
    // A downed crewmate is no patient: only a revive gets it up.
    world.aboard.room.knock_out_for_probe(1);
    world.step(&[]);
    run_for(&mut world, 1.0);
    assert_eq!(world.drone_of(0).and_then(|d| d.patient), None);
    // Behind a wall, it flies over and heals.
    let mut world = medic_at([4, 0, 0, 0]);
    put_behind_a_wall(&mut world, 4.0).expect("a wall with deck both sides on the ship");
    hurt(&mut world, 1, 30.0);
    drone(&mut world);
    run_for(&mut world, 2.0);
    let before = world.aboard.room.health(1);
    run_for(&mut world, 2.0);
    assert!(
        world.aboard.room.health(1) > before + 5.0,
        "healed over the wall"
    );
}

// --- C: C, Triage, and the Override Core (task 153) -------------------------------

/// **Triage lifts a heal by how much of its bar the Bim is missing**: the
/// rank's share at an empty bar, half of it at half a bar, none whole.
#[test]
fn triage_lifts_a_heal_by_how_much_of_the_bar_is_missing() {
    assert_eq!(class::TRIAGE, [0.25, 0.40, 0.55, 0.70]);
    let world = medic();
    assert_eq!(world.medic_heal_factor(0, 1), 1.0, "no rank, no lift");
    for rank in 1..=4u8 {
        let mut world = medic_at([0, rank, 0, 0]);
        let max = world.aboard.room.max_health(1);
        assert_eq!(world.medic_heal_factor(0, 1), 1.0, "rank {rank}: whole");
        hurt(&mut world, 1, max / 2.0);
        let want = 1.0 + class::TRIAGE[rank as usize - 1] * 0.5;
        assert!(
            (world.medic_heal_factor(0, 1) - want).abs() < 1e-4,
            "rank {rank}: half a bar"
        );
        // A crewmate's heals are not his: no lift for a heal of a bot's.
        assert_eq!(world.medic_heal_factor(1, 1), 1.0);
    }
    // Measured on the beam: the badly hurt gain more.
    let gain = |triage: u8| {
        let mut world = medic_at([0, triage, 1, 0]);
        hurt(&mut world, 1, 10.0);
        beam(&mut world, 0, Some(1));
        let before = world.aboard.room.health(1);
        run_for(&mut world, 3.0);
        world.aboard.room.health(1) - before
    };
    let (plain, lifted) = (gain(0), gain(4));
    assert!(
        lifted > plain * 1.5,
        "Triage on a Bim near nothing: {lifted} against {plain}"
    );
}

/// **An Override Core gives a medic half as much healing again** — the
/// beam, the drone and the circle — and nobody else's heals.
#[test]
fn an_override_core_gives_a_medic_half_as_much_healing_again() {
    let core = bims::module::ModuleKind::OverrideCore.at(bims::combat::Tier::One);
    let mut world = medic_at([0, 0, 1, 0]);
    assert_eq!(world.override_heal(0), 1.0);
    carry(&mut world, 0, core);
    assert_eq!(world.override_heal(0), class::OVERRIDE_HEAL);
    assert_eq!(class::OVERRIDE_HEAL, 1.5);
    // Not a medic: nothing.
    carry(&mut world, 1, core);
    assert_eq!(world.override_heal(1), 1.0);
    let gain = |core_on: bool| {
        let mut world = medic_at([0, 0, 1, 0]);
        if core_on {
            carry(&mut world, 0, core);
        }
        hurt(&mut world, 1, 40.0);
        beam(&mut world, 0, Some(1));
        let before = world.aboard.room.health(1);
        run_for(&mut world, 4.0);
        world.aboard.room.health(1) - before
    };
    let (plain, lifted) = (gain(false), gain(true));
    assert!(
        (lifted / plain - 1.5).abs() < 0.02,
        "{lifted} against {plain}"
    );
}

/// An item in crew member `who`'s first item slot.
fn carry(world: &mut World, who: usize, item: bims::module::Module) {
    let mut gear = world.aboard.room.gear(who);
    gear.items[0] = Some(item);
    world.aboard.room.issue(who, gear);
}

// --- D: E, the Heal Beam ------------------------------------------------------------

/// **The beam's rate, range and patients by rank**, and
/// [`class::HEAL_BEAM_HP`] a hundred and twenty an hour — two a second at
/// 1×.
#[test]
fn the_heal_beam_s_rate_range_and_patients_by_rank() {
    assert_eq!(class::HEAL_BEAM_HP, 120.0);
    assert_eq!(class::HEAL_BEAM_RANGE, 6.0);
    let world = medic();
    assert_eq!(world.can_beam(0, 1), Err(Refusal::NotLearnt), "rank nought");
    let want = [(2.0, 6.0, 1), (3.0, 7.0, 1), (4.0, 8.0, 1), (5.0, 9.0, 2)];
    for (rank, &(per_second, range, patients)) in (1..=4u8).zip(&want) {
        let world = medic_at([0, 0, rank, 0]);
        assert_eq!(world.beam_rate(0), per_second * 60.0, "rank {rank}");
        assert_eq!(world.beam_range(0), range, "rank {rank}");
        assert_eq!(world.beam_patients(0), patients, "rank {rank}");
    }
    // The gain, measured: rank three's four a second.
    let mut world = medic_at([0, 0, 3, 0]);
    hurt(&mut world, 1, 20.0);
    assert!(linked(&beam(&mut world, 0, Some(1)), 0, Some(1)));
    let before = world.aboard.room.health(1);
    for _ in 0..(10 * STEPS_A_MINUTE) {
        world.step(&[]);
    }
    let gained = world.aboard.room.health(1) - before;
    assert!((gained - 40.0).abs() < 0.3, "ten seconds at four: {gained}");
    // Its range: seven tiles and a half is out at rank two, in at three.
    let mut world = medic_at([0, 0, 2, 0]);
    stand_off(&mut world, 1, 0, 7.5);
    assert_eq!(world.can_beam(0, 1), Err(Refusal::OutOfBeamRange));
    let mut world = medic_at([0, 0, 3, 0]);
    stand_off(&mut world, 1, 0, 7.5);
    if world
        .aboard
        .room
        .sees_for_probe(0, world.aboard.room.bim_pos(1))
    {
        assert_eq!(world.can_beam(0, 1), Ok(()));
    }
}

/// **Two patients at the fourth rank, each at the full rate**, a third
/// taking the first's place.
#[test]
fn the_fourth_rank_holds_two_patients_at_the_full_rate() {
    let mut world = simulation_world(combat_ship(), REFERENCE_MONEY, 3);
    assert_eq!(world.set_class(0, Class::Medic), Ok(()));
    hold_still(&mut world);
    ranks(&mut world, 0, [0, 0, 4, 0]);
    beside(&mut world, 1, 0);
    stand_off(&mut world, 2, 0, -1.0);
    hurt(&mut world, 1, 20.0);
    hurt(&mut world, 2, 20.0);
    beam(&mut world, 0, Some(1));
    beam(&mut world, 0, Some(2));
    assert_eq!(world.patients_of(0), vec![1, 2]);
    let (a, b) = (world.aboard.room.health(1), world.aboard.room.health(2));
    for _ in 0..(5 * STEPS_A_MINUTE) {
        world.step(&[]);
    }
    let rate = class::HEAL_BEAM_HP * class::HEAL_BEAM_RATE[3] / 60.0 * 5.0;
    assert!((world.aboard.room.health(1) - a - rate).abs() < 0.3);
    assert!((world.aboard.room.health(2) - b - rate).abs() < 0.3);
    beam(&mut world, 0, Some(0));
    assert_eq!(world.patients_of(0), vec![2, 0], "the oldest let go");
}

/// **The fourth rank's link heals what the medic's items heal him by on
/// top**: a *Pressure Seal*'s regeneration added to each patient's beam,
/// never below the fourth rank and never twice on himself.
#[test]
fn the_fourth_rank_s_link_adds_the_medic_s_item_healing() {
    let seal = bims::module::ModuleKind::PressureSeal.at(bims::combat::Tier::Three);
    let regen = bims::module::PRESSURE_SEAL_REGEN[2];
    let gain = |rank: u8, item: bool| {
        let mut world = medic_at([0, 0, rank, 0]);
        if item {
            carry(&mut world, 0, seal);
        }
        hurt(&mut world, 1, 20.0);
        assert!(linked(&beam(&mut world, 0, Some(1)), 0, Some(1)));
        let added = world.beam_item_rate(0);
        let before = world.aboard.room.health(1);
        for _ in 0..(5 * STEPS_A_MINUTE) {
            world.step(&[]);
        }
        (added, world.aboard.room.health(1) - before)
    };
    let (none, plain) = gain(4, false);
    assert_eq!(none, 0.0, "no items, nothing added");
    let (added, lifted) = gain(4, true);
    assert_eq!(added, regen * 60.0);
    assert!(
        (lifted - plain - regen * 5.0).abs() < 0.3,
        "{lifted} against {plain}"
    );
    let (below, _) = gain(3, true);
    assert_eq!(below, 0.0, "not below the fourth rank");
}

/// **The beam's key forgives a near miss**: off everybody, it takes the
/// nearest friendly the beam reaches within
/// [`class::HEAL_BEAM_PICK_REACH`] tiles of the pointer — never the medic
/// himself, never one already held — and nobody past that.
#[test]
fn the_beam_s_key_takes_the_nearest_friendly_beside_the_pointer() {
    let mut world = medic_at([0, 0, 1, 0]);
    let at = stand_off(&mut world, 1, 0, 2.0);
    let medic = world.aboard.room.bim_pos(0);
    // On it: the patient under the pointer, as before.
    assert_eq!(world.beam_patient_near(0, at.x, at.y), Some(1));
    // A tile and a half past it, on nobody: still crew member 1.
    let miss = at + (at - medic).normalize_or_zero() * (1.5 * TILE);
    assert_eq!(world.patient_at(miss.x, miss.y), None, "nobody under it");
    assert_eq!(world.beam_patient_near(0, miss.x, miss.y), Some(1));
    // Past the reach: nobody.
    let far = at + (at - medic).normalize_or_zero() * ((class::HEAL_BEAM_PICK_REACH + 0.5) * TILE);
    assert_eq!(world.beam_patient_near(0, far.x, far.y), None);
    // Held already, it is not picked again by a near miss (the key on
    // nobody unlinks), and the medic beside the pointer is not picked.
    assert!(linked(&beam(&mut world, 0, Some(1)), 0, Some(1)));
    assert_eq!(world.beam_patient_near(0, miss.x, miss.y), None);
    let by_medic = medic - (at - medic).normalize_or_zero() * TILE;
    assert_eq!(world.patient_at(by_medic.x, by_medic.y), None);
    assert_eq!(world.beam_patient_near(0, by_medic.x, by_medic.y), None);
}

/// **A link is broken by the range, never by sight**: a wall coming
/// between the medic and his patient keeps the beam on — though a new
/// link still wants sight — and the patient walking out of range breaks
/// it.
#[test]
fn a_link_holds_out_of_sight_and_breaks_out_of_range() {
    let mut world = medic_at([0, 0, 1, 0]);
    hurt(&mut world, 1, 20.0);
    assert!(linked(&beam(&mut world, 0, Some(1)), 0, Some(1)));
    put_behind_a_wall(&mut world, 4.0).expect("a wall with deck both sides on the ship");
    let before = world.aboard.room.health(1);
    run_for(&mut world, 2.0);
    assert_eq!(world.patients_of(0), vec![1], "held behind the wall");
    assert!(world.aboard.room.health(1) > before + 1.0, "and healing");
    assert_eq!(
        world.can_beam(0, 1),
        Err(Refusal::NoSightOfPatient),
        "a new link wants sight"
    );
    // Out of range: broken.
    let medic = world.aboard.room.bim_pos(0);
    let out = world.beam_range(0) + 1.5;
    let spot = stand_off(&mut world, 1, 0, out);
    assert!(
        (spot - medic).len() > world.beam_range(0) * TILE,
        "stood out of range"
    );
    assert!(world.patients_of(0).is_empty(), "broken by the range");
    assert!(!world.is_beaming(0));
}

/// **A medic linked fires at his full rate at every rank, and the link
/// heals him as much as his patient** (task 153) — once, two patients or
/// himself.
#[test]
fn a_medic_linked_fires_at_his_full_rate_and_heals_himself_as_much() {
    for rank in 1..=4u8 {
        let mut world = medic_at([0, 0, rank, 0]);
        hurt(&mut world, 1, 40.0);
        assert!(linked(&beam(&mut world, 0, Some(1)), 0, Some(1)));
        let skill = world.skill_of(0);
        assert!(!skill.holds_fire, "rank {rank}: fires");
        assert_eq!(skill.fire_rate, 1.0, "rank {rank}: at his own rate");
    }
    let mut world = medic_at([0, 0, 1, 0]);
    hurt(&mut world, 1, 40.0);
    hurt(&mut world, 0, 40.0);
    beam(&mut world, 0, Some(1));
    let (a, m) = (world.aboard.room.health(1), world.aboard.room.health(0));
    for _ in 0..(5 * STEPS_A_MINUTE) {
        world.step(&[]);
    }
    let patient = world.aboard.room.health(1) - a;
    let himself = world.aboard.room.health(0) - m;
    let (pb, hb) = (bar(&world, 1), bar(&world, 0));
    assert!((patient - beam_gain(5.0) * pb).abs() < 0.2, "{patient}");
    assert!(
        (himself - beam_gain(5.0) * hb).abs() < 0.2,
        "himself: {himself}"
    );
    // Two patients at the fourth rank: once on him.
    let mut world = simulation_world(combat_ship(), REFERENCE_MONEY, 3);
    assert_eq!(world.set_class(0, Class::Medic), Ok(()));
    hold_still(&mut world);
    ranks(&mut world, 0, [0, 0, 4, 0]);
    beside(&mut world, 1, 0);
    stand_off(&mut world, 2, 0, -1.0);
    for who in 0..3 {
        hurt(&mut world, who, 40.0);
    }
    beam(&mut world, 0, Some(1));
    beam(&mut world, 0, Some(2));
    let m = world.aboard.room.health(0);
    for _ in 0..(2 * STEPS_A_MINUTE) {
        world.step(&[]);
    }
    let rate = class::HEAL_BEAM_HP * class::HEAL_BEAM_RATE[3] / 60.0 * 2.0 * bar(&world, 0);
    let himself = world.aboard.room.health(0) - m;
    assert!((himself - rate).abs() < 0.2, "once on him: {himself}");
}

/// **The beam puts hit points back** — never past the whole bar, nothing
/// to a downed body — and a medic may beam himself.
#[test]
fn a_linked_patient_gains_hit_points_and_a_medic_may_beam_himself() {
    let mut world = medic_at([0, 0, 1, 0]);
    hurt(&mut world, 1, 40.0);
    assert!(linked(&beam(&mut world, 0, Some(1)), 0, Some(1)));
    let before = world.aboard.room.health(1);
    for _ in 0..(10 * STEPS_A_MINUTE) {
        world.step(&[]);
    }
    let gained = world.aboard.room.health(1) - before;
    assert!((gained - beam_gain(10.0)).abs() < 0.2, "{gained}");
    // Downed, the beam gets nobody up: only a revive does.
    world.aboard.room.knock_out_for_probe(1);
    world.step(&[]);
    assert!(world.aboard.room.is_downed(1));
    assert!(linked(&beam(&mut world, 0, Some(1)), 0, Some(1)));
    for _ in 0..(5 * STEPS_A_MINUTE) {
        world.step(&[]);
    }
    assert!(world.aboard.room.is_downed(1), "still downed");
    assert_eq!(world.aboard.room.health(1), 0.0);
    // Never above a whole bar.
    let mut world = medic_at([0, 0, 1, 0]);
    hurt(&mut world, 1, MAX_HEALTH - 1.0);
    assert!(linked(&beam(&mut world, 0, Some(1)), 0, Some(1)));
    for _ in 0..(10 * STEPS_A_MINUTE) {
        world.step(&[]);
    }
    assert_eq!(world.aboard.room.health(1), MAX_HEALTH);
    // Himself.
    let mut world = medic_at([0, 0, 1, 0]);
    assert_eq!(world.can_beam(0, 0), Ok(()));
    hurt(&mut world, 0, 50.0);
    assert!(linked(&beam(&mut world, 0, Some(0)), 0, Some(0)));
    let before = world.aboard.room.health(0);
    for _ in 0..(10 * STEPS_A_MINUTE) {
        world.step(&[]);
    }
    let gained = world.aboard.room.health(0) - before;
    assert!(
        (gained - beam_gain(10.0) * bar(&world, 0)).abs() < 0.2,
        "its own bar: {gained}"
    );
}

#[test]
fn every_way_the_link_breaks_and_every_reason_a_beam_is_refused() {
    let mut world = medic_at([0, 0, 1, 0]);
    assert_eq!(world.can_beam(1, 0), Err(Refusal::NotAMedic));
    assert!(refused_with(
        &beam(&mut world, 1, Some(0)),
        Refusal::NotAMedic
    ));
    assert_eq!(world.can_beam(0, 7), Err(Refusal::NotACrewmate));
    let here = world.aboard.room.bim_pos(0);
    let range = world.beam_range(0) * TILE;
    let far = world
        .aboard
        .room
        .put_for_probe(1, here + vec2(range + 2.0 * TILE, 0.0));
    if (far - here).len() > range {
        assert_eq!(world.can_beam(0, 1), Err(Refusal::OutOfBeamRange));
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
    // The medic goes down: broken.
    let mut world = medic_at([0, 0, 1, 0]);
    assert!(linked(&beam(&mut world, 0, Some(1)), 0, Some(1)));
    world.aboard.room.knock_out_for_probe(0);
    world.step(&[]);
    world.step(&[]);
    assert!(!world.is_beaming(0), "the medic down");
    // Unlinked by the medic, and the link in the checksum.
    let mut world = medic_at([0, 0, 1, 0]);
    let before = world_checksum(&world);
    assert!(linked(&beam(&mut world, 0, Some(1)), 0, Some(1)));
    assert_ne!(world_checksum(&world), before);
    assert!(linked(&beam(&mut world, 0, None), 0, None));
    assert!(!world.is_beaming(0));
}

// --- E: R, the Healing Circle (task 153) ------------------------------------------

fn circle(world: &mut World, on: bool) -> Vec<WorldEvent> {
    world.step(&[Command::HealingCircle { slot: 0, on }])
}

/// **On, it heals every Bim round him at the link's rate and drains him
/// as much**; its radius by rank; nobody past it or behind a wall; off,
/// nothing more.
#[test]
fn the_healing_circle_heals_round_him_at_the_link_s_rate_and_drains_him_as_much() {
    let mut world = medic();
    assert!(refused_with(&circle(&mut world, true), Refusal::NotLearnt));
    let events = world.step(&[Command::HealingCircle { slot: 1, on: true }]);
    assert!(refused_with(&events, Refusal::NotAMedic), "{events:?}");
    for (rank, radius) in (1..=4u8).zip([3.0, 3.5, 4.0, 4.5]) {
        let world = medic_at([0, 0, 0, rank]);
        assert_eq!(world.healing_circle_radius(0), radius, "rank {rank}");
    }
    // The link's rate is the E rank's: three a second at its second.
    let mut world = medic_at([0, 0, 2, 1]);
    in_the_open(&mut world, 1.0);
    assert_eq!(world.healing_circle_rate(0), 3.0);
    hurt(&mut world, 1, 40.0);
    assert!(circle(&mut world, true).contains(&WorldEvent::Circled { who: 0, on: true }));
    assert!(world.is_circling(0));
    assert_eq!(world.healing_circle_reaching(0), vec![1], "not himself");
    let (a, m) = (world.aboard.room.health(1), world.aboard.room.health(0));
    run_for(&mut world, 5.0);
    let healed = world.aboard.room.health(1) - a;
    let drained = m - world.aboard.room.health(0);
    // Shares of each one's own bar (October 2026).
    assert!(
        (healed - 15.0 * bar(&world, 1)).abs() < 0.3,
        "five seconds at three: {healed}"
    );
    assert!(
        (drained - 15.0 * bar(&world, 0)).abs() < 0.3,
        "as much off him: {drained}"
    );
    // Off: nothing more either way.
    assert!(circle(&mut world, false).contains(&WorldEvent::Circled { who: 0, on: false }));
    let (a, m) = (world.aboard.room.health(1), world.aboard.room.health(0));
    run_for(&mut world, 2.0);
    assert_eq!(world.aboard.room.health(1), a);
    assert_eq!(world.aboard.room.health(0), m);
    // Past its radius: no heal, and the drain all the same.
    let mut world = medic_at([0, 0, 1, 1]);
    in_the_open(&mut world, 4.0);
    hurt(&mut world, 1, 40.0);
    circle(&mut world, true);
    let (a, m) = (world.aboard.room.health(1), world.aboard.room.health(0));
    run_for(&mut world, 2.0);
    assert_eq!(world.aboard.room.health(1), a, "out of reach");
    assert!(
        world.aboard.room.health(0) < m - 3.5,
        "drained all the same"
    );
    // Behind a wall: nothing.
    let mut world = medic_at([0, 0, 1, 4]);
    put_behind_a_wall(&mut world, 4.0).expect("a wall with deck both sides on the ship");
    hurt(&mut world, 1, 40.0);
    circle(&mut world, true);
    assert!(world.healing_circle_reaching(0).is_empty());
    run_for(&mut world, 1.0);
    assert_eq!(world.aboard.room.health(1), 40.0, "the wall took it");
}

/// **Its drain downs him**, and that switches it off, said; a medic down
/// cannot switch it on, and off is never refused.
#[test]
fn the_circle_s_drain_downs_him_and_that_switches_it_off() {
    let mut world = medic_at([0, 0, 1, 1]);
    hurt(&mut world, 0, 3.0);
    circle(&mut world, true);
    let mut off = false;
    for _ in 0..(4 * STEPS_A_MINUTE) {
        let events = world.step(&[]);
        off |= events.contains(&WorldEvent::Circled { who: 0, on: false });
    }
    assert!(world.aboard.room.is_downed(0), "drained down");
    assert!(off && !world.is_circling(0), "and it went off, said");
    assert_eq!(world.can_healing_circle(0, true), Err(Refusal::OutOfReach));
    assert_eq!(world.can_healing_circle(0, false), Ok(()));
    // The link on himself pays for it: two in, two out.
    let mut world = medic_at([0, 0, 1, 1]);
    hurt(&mut world, 0, 50.0);
    beam(&mut world, 0, Some(0));
    circle(&mut world, true);
    run_for(&mut world, 5.0);
    assert!(
        (world.aboard.room.health(0) - 50.0).abs() < 0.3,
        "{}",
        world.aboard.room.health(0)
    );
}

/// The machines' dock with one of them stood down the corridor, held
/// where it is put and firing nothing, and the medic three tiles from it
/// with his crewmate downed in his arms — so neither fires, and only the
/// circle can touch it — a medic with empty hands two tiles off would
/// punch it.
fn circle_fight(ranks_of: [u8; 4]) -> World {
    let mut world = simulation_world(shipdesign::fixture::flyer(2), REFERENCE_MONEY, 2);
    assert_eq!(world.set_class(0, Class::Medic), Ok(()));
    ranks(&mut world, 0, ranks_of);
    assert!(world.stage_droid_fight_for_probe(bims::droid::DroidKind::Trooper, None));
    for _ in 0..3 {
        world.step(&[]);
    }
    assert!(world.carry_for_probe(0, 1), "the crewmate in his arms");
    let target = world
        .aboard
        .room
        .combat_targets_for_probe()
        .into_iter()
        .flatten()
        .next()
        .expect("the staged machine a target");
    let from = world.aboard.room.bim_pos(0);
    let way = (from - target) * (1.0 / (from - target).len());
    world
        .aboard
        .room
        .post_for_probe(0, target + way * (3.0 * TILE));
    // A bolt fired before the carry lands first.
    run_for(&mut world, 1.0);
    world
}

/// What is left of the staged machine, its parts added up.
fn machine_health(world: &World) -> f32 {
    let droid = world
        .residents
        .as_ref()
        .unwrap()
        .aboard
        .room
        .droid(0)
        .expect("the staged machine");
    bims::droid::DroidPart::ALL
        .iter()
        .map(|&p| droid.body.health(p))
        .sum()
}

/// **The enemy in it burn at half its heal**, a pulse every half second;
/// nothing with it off.
#[test]
fn the_circle_burns_the_enemy_in_it_at_half_its_heal() {
    let burned = |e: u8, on: bool| {
        let mut world = circle_fight([0, 0, e, 4]);
        if on {
            circle(&mut world, true);
        }
        let before = machine_health(&world);
        run_for(&mut world, 4.0);
        before - machine_health(&world)
    };
    assert_eq!(burned(1, false), 0.0, "nothing with it off");
    let one = burned(1, true);
    let four = burned(4, true);
    assert!(one > 0.0, "it burns");
    // Two a second's heal is one a second's burn, four seconds of pulses.
    assert!(one <= 4.0 + 1e-3, "at most half its heal: {one}");
    assert!(
        (four / one - 2.5).abs() < 0.05,
        "five a second's heal burns two and a half times two's: {four} against {one}"
    );
}

/// The medic's state goes with its body, and a game with a medic reads
/// the same twice.
#[test]
fn a_medic_s_state_dies_with_it_and_a_game_with_a_medic_reads_the_same_twice() {
    let mut world = medic_at([1, 1, 1, 1]);
    drone(&mut world);
    circle(&mut world, true);
    assert!(world.medic_of(0).last_drone.is_some());
    world.aboard.room.kill_for_probe(0);
    world.step(&[]);
    world.step(&[]);
    assert_eq!(
        world.medic_of(0),
        crate::medic::Medic::default(),
        "gone with it"
    );
    // The level and the ranks are the player's, kept for its buyback.
    assert_eq!(world.rank_of(0, class::SLOT_R), 1);
    // Two runs of the kit on one seed are one world, and the drone and the
    // circle are in the checksum.
    let run = || {
        let mut world = medic_at([1, 1, 1, 1]);
        hurt(&mut world, 1, 60.0);
        beam(&mut world, 0, Some(1));
        drone(&mut world);
        circle(&mut world, true);
        for _ in 0..(3 * STEPS_A_MINUTE) {
            world.step(&[]);
        }
        world_checksum(&world)
    };
    assert_eq!(run(), run());
    for command in [
        Command::HealDrone { slot: 0 },
        Command::HealingCircle { slot: 0, on: true },
    ] {
        let mut a = medic_at([1, 0, 0, 1]);
        a.step(&[command]);
        let mut b = medic_at([1, 0, 0, 1]);
        b.step(&[]);
        assert_ne!(
            world_checksum(&a),
            world_checksum(&b),
            "{command:?} is hashed"
        );
    }
    // Every mission starts with the drone ready and the circle off.
    let mut world = medic_at([1, 0, 0, 1]);
    drone(&mut world);
    circle(&mut world, true);
    next_mission(&mut world);
    assert_eq!(world.heal_drone_cooldown_left(0), 0.0);
    assert!(world.drone_of(0).is_none() && !world.is_circling(0));
}

// --- D: carrying a downed body out of the fire, and the field medic ---

/// A medic takes a downed crewmate up into its arms, walks it somewhere
/// else and sets it down there: the body goes where the arms go, walks
/// nowhere of its own, and is left on the deck where it was put down.
#[test]
fn a_medic_carries_a_downed_crewmate_and_sets_it_down_again() {
    let mut world = medic();
    world.aboard.room.knock_out_for_probe(1);
    world.step(&[]);
    assert!(world.aboard.room.is_downed(1), "downed");
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
    assert!(world.aboard.room.is_downed(1), "and still downed");
    // A second set down with empty arms says so rather than doing
    // nothing quietly.
    let events = world.step(&[Command::Carry { slot: 0, who: None }]);
    assert!(refused_with(&events, Refusal::NotCarrying), "{events:?}");
}

/// Who may carry whom: a medic and a hired field medic, a downed body,
/// and one pair of arms to a body.
#[test]
fn only_a_medic_carries_and_only_somebody_downed() {
    let mut world = simulation_world(combat_ship(), REFERENCE_MONEY, 3);
    hold_still(&mut world);
    beside(&mut world, 1, 0);
    // Nobody is a medic yet: the key does nothing for anybody.
    assert_eq!(world.can_carry(0, 1), Err(Refusal::NotCarrying));
    assert_eq!(world.set_class(0, Class::Medic), Ok(()));
    // A crewmate on its feet is nobody's to carry, hurt or whole.
    assert_eq!(world.can_carry(0, 1), Err(Refusal::NotHurt));
    hurt(&mut world, 1, 10.0);
    assert_eq!(world.can_carry(0, 1), Err(Refusal::NotHurt));
    // Itself, never.
    assert_eq!(world.can_carry(0, 0), Err(Refusal::NotACrewmate));
    // Downed is what a carry is for.
    world.aboard.room.knock_out_for_probe(1);
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

/// A field medic under arms goes for a crewmate that is down, picks it
/// up and carries it away from the fight.
#[test]
fn a_field_medic_fetches_a_crewmate_that_is_down_out_of_the_fire() {
    // One player and three aboard, so the medic is a **bot**: the
    // rescue is `Game::bot_stand`'s branch, and a Bim a player steers
    // never runs it.
    let mut world = crate::fixture::crewed_world(combat_ship(), REFERENCE_MONEY, 1, 3);
    // The staged fight puts a machine on the deck a few tiles inside the
    // station's door with a crew member recruited against it, which is
    // what makes the body worth fetching rather than already clear. It
    // is held where it is put and fires nothing, so the fetch is the
    // only thing being measured.
    assert!(world.stage_droid_fight_for_probe(bims::droid::DroidKind::Trooper, None));
    world.step(&[]);
    let medic = 2;
    assert!(world.field_medic_for_probe(medic));
    assert!(world.is_field_medic(medic));
    assert!(world.can_lift(medic), "and may carry");
    assert_eq!(
        world.class_of(medic),
        Class::None,
        "none of the class's own"
    );
    // Crew member 1 down beside the fight, and the medic a few tiles
    // off it rather than back aboard the ship — a field medic looks
    // `RESCUE_LOOK` tiles for somebody, not across the whole station.
    let fight = world.aboard.room.bim_pos(0);
    world.aboard.room.put_for_probe(1, fight + vec2(TILE, 0.0));
    world
        .aboard
        .room
        .put_for_probe(2, fight + vec2(-3.0 * TILE, 0.0));
    world.aboard.room.knock_out_for_probe(1);
    world.step(&[]);
    assert!(world.aboard.room.is_downed(1), "down");

    // A few seconds of the clock: long enough to walk over and reach,
    // well inside the countdown.
    let mut fetched = false;
    for _ in 0..(20 * STEPS_A_MINUTE) {
        world.step(&[]);
        if world.carrying_of(medic) == Some(1) {
            fetched = true;
            break;
        }
    }
    assert!(fetched, "the field medic went and got it");
}

/// A bot lost off the crew breaks every beam — an index is all a link
/// is.
#[test]
fn beams_are_cleared_by_a_bot_lost() {
    use crate::data;
    use worldgen::GalaxyType;
    let galaxy = worldgen::Galaxy::new(data::DEFAULT_SEED, GalaxyType::SpiralTwoArm);
    let (star, station) = crate::spawn(&galaxy).unwrap();
    let mut world = World::start_with_crew(
        combat_ship(),
        REFERENCE_MONEY,
        2,
        3,
        data::DEFAULT_SEED,
        GalaxyType::SpiralTwoArm,
        star,
        station,
    )
    .unwrap();
    // No fight: the spawn a peaceful stop (task 111).
    world.set_quiet_sites_for_probe(true);
    assert_eq!(world.set_class(0, Class::Medic), Ok(()));
    hold_still(&mut world);
    // The beam's first rank, since a medic of nought ranks has none.
    ranks(&mut world, 0, [0, 0, 1, 0]);
    // Linked to the bot; the bot dead and gone with the site —
    // a bot is dropped off the crew when the ship leaves — breaks it, and
    // the state list shrinks with the crew.
    hold_still(&mut world);
    beside(&mut world, 2, 0);
    assert!(linked(&beam(&mut world, 0, Some(2)), 0, Some(2)));
    assert_eq!(world.medics.len(), 3);
    world.aboard.room.kill_for_probe(2);
    let events = world.step(&[]);
    assert!(
        events
            .iter()
            .any(|e| matches!(e, WorldEvent::BotLost { who: 2, .. })),
        "{events:?}"
    );
    let gangway = world.aboard.gangway.expect("joined, the ship's side");
    for who in 0..2 {
        world
            .aboard
            .room
            .put_for_probe(who, vec2(gangway.x as f32, gangway.y as f32));
    }
    world.leave_for_probe();
    assert_eq!(world.aboard.crew_count(), 2);
    assert!(world.patients_of(0).is_empty(), "cleared by the bot lost");
    assert_eq!(world.medics.len(), 2);
}

// --- F: a site's defenders (the beam and the drone reach them too) ------------------

/// A station the machines are coming for, docked and joined, with slot 0
/// a medic — the drone and the beam at their fourth rank — the crew held
/// still, and the first of its defenders: its patient index
/// (`medic::GUEST + i`) and its body in the residents' room.
fn defended() -> (World, u32, usize) {
    let mut world =
        crate::fixture::open_simulation_world(shipdesign::fixture::flyer(2), REFERENCE_MONEY, 2);
    world.set_defense_by_machines_for_probe();
    world.step(&[]);
    assert_eq!(world.set_class(0, Class::Medic), Ok(()));
    ranks(&mut world, 0, [4, 0, 4, 0]);
    hold_still(&mut world);
    let residents = world.residents.as_ref().expect("the station's room");
    let i = (0..residents.defender.len())
        .find(|&i| residents.is_defender(i))
        .expect("a defender fielded");
    (world, crate::medic::GUEST + i as u32, i)
}

fn defender_health(world: &World, i: usize) -> f32 {
    world.residents.as_ref().unwrap().aboard.room.health(i)
}

fn hurt_defender(world: &mut World, i: usize, points: f32) {
    let room = &mut world.residents.as_mut().unwrap().aboard.room;
    room.set_health_for_probe(i, points);
}

/// **The beam links a defender** and heals it in its own room: the
/// medic stood beside it, the pointer on it names it (a crewmate under
/// the pointer would come first), and the link holds and heals.
#[test]
fn the_beam_links_a_site_s_defender_and_heals_it() {
    let (mut world, g, i) = defended();
    let at = world
        .patient_pos(g)
        .expect("the defender on the joined deck");
    world.aboard.room.put_for_probe(0, at + vec2(TILE, 0.0));
    world
        .aboard
        .room
        .put_for_probe(1, at + vec2(-4.0 * TILE, 0.0));
    world.step(&[]);
    let at = world.patient_pos(g).unwrap();
    assert_eq!(world.patient_at(at.x, at.y), Some(g), "the pointer on it");
    let medic = world.aboard.room.bim_pos(0);
    assert_eq!(
        world.patient_at(medic.x, medic.y),
        Some(0),
        "a crewmate first"
    );
    hurt_defender(&mut world, i, 30.0);
    assert_eq!(world.can_beam(0, g), Ok(()));
    assert!(linked(&beam(&mut world, 0, Some(g)), 0, Some(g)));
    assert_eq!(world.patients_of(0), vec![g]);
    let before = defender_health(&world, i);
    run_for(&mut world, 2.0);
    assert_eq!(world.patients_of(0), vec![g], "held");
    assert!(
        defender_health(&world, i) > before + 1.0,
        "healed: {before} -> {}",
        defender_health(&world, i)
    );
    // Nobody but a defender: a guest index naming no defender is refused.
    let residents = world.residents.as_ref().unwrap();
    if let Some(own) = (0..residents.defender.len()).find(|&j| !residents.is_defender(j)) {
        let other = crate::medic::GUEST + own as u32;
        assert_eq!(world.can_beam(0, other), Err(Refusal::NotACrewmate));
    }
}

/// **The drone goes to a defender only with no crewmate hurt**, and
/// leaves it for a crewmate the moment one is.
#[test]
fn the_drone_heals_a_defender_but_prefers_a_crewmate() {
    let (mut world, g, i) = defended();
    for who in 0..world.aboard.crew_count() as usize {
        let max = world.aboard.room.max_health(who);
        world.aboard.room.set_health_for_probe(who, max);
    }
    hurt_defender(&mut world, i, 20.0);
    world.step(&[]);
    assert!(drone(&mut world).contains(&WorldEvent::DroneLaunched { who: 0 }));
    run_for(&mut world, 0.2);
    assert_eq!(world.drone_of(0).and_then(|d| d.patient), Some(g));
    // Over it, it heals it.
    run_for(&mut world, 3.0);
    let before = defender_health(&world, i);
    run_for(&mut world, 1.0);
    assert!(
        defender_health(&world, i) > before + 1.0,
        "healed: {before} -> {}",
        defender_health(&world, i)
    );
    // A crewmate hurt, though higher on his bar than the defender: the
    // drone is his.
    hurt(&mut world, 1, 60.0);
    assert_eq!(world.drone_of(0).and_then(|d| d.patient), Some(1));
}
