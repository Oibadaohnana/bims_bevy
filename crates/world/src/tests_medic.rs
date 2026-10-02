//! The medic class (feature 76; its ranked kit task 130): `crate::class`'s
//! third class. The sixteen levels and a skill point a level; the two
//! base traits — a revive in four seconds, and up at two fifths of the
//! bar; the four abilities a rank at a time — Nanite Burst, Healing Aura,
//! Heal Beam and Cloak — each doing what its rank says; that the surge is
//! gone; and carrying a downed crewmate out of the fire.

use bims::combat::WeaponKind;
use bims::health::MAX_HEALTH;
use bims::math::{Vec2, vec2};
use bims::order::CrewOrder;
use shipdesign::fixture::combat_ship;

use crate::class::{self, Class};
use crate::deploy::DeployKind;
use crate::event::{Refusal, WorldEvent};
use crate::fixture::{REFERENCE_MONEY, crewed_world, simulation_world};
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

/// The way from `of` — east first, then west, south and north — that has
/// open deck `tiles` tiles off with a clear line to it. Nobody walks about
/// on their own (September 2026), so `of` stands where the crew woke up,
/// and what is east of it is whatever the ship has there: a test that
/// wants open ground asks for it rather than assuming it.
fn open_way(world: &World, of: usize, tiles: f32) -> Vec2 {
    let from = world.aboard.room.bim_pos(of);
    [(1.0, 0.0), (-1.0, 0.0), (0.0, 1.0), (0.0, -1.0)]
        .into_iter()
        .map(|(x, y)| vec2(x, y))
        .find(|&way| {
            let p = from + way * (tiles * TILE);
            world.aboard.room.is_deck_tile(p) && world.aboard.room.line_clear(from, p)
        })
        .unwrap_or(vec2(1.0, 0.0))
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

/// **The surge is gone** (task 130): the medic's abilities are the beam,
/// the burst and the cloak, and its Q is the Nanite Burst.
#[test]
fn the_surge_is_gone() {
    let own: Vec<class::Ability> = class::Ability::ALL
        .into_iter()
        .filter(|&a| class::can(Class::Medic, a))
        .collect();
    assert_eq!(
        own,
        vec![
            class::Ability::Beam,
            class::Ability::NaniteBurst,
            class::Ability::Cloak
        ]
    );
    let mut world = medic_at([1, 0, 0, 0]);
    let events = world.step(&[Command::NaniteBurst { slot: 0 }]);
    assert!(
        events
            .iter()
            .any(|e| matches!(e, WorldEvent::NaniteBurst { who: 0, .. })),
        "Q is the burst: {events:?}"
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
/// patient gets up at anybody's share. The Healing Aura does not touch a
/// revive: it is not a heal.
#[test]
fn a_medic_revives_in_four_seconds_to_two_fifths_and_anybody_else_to_three_tenths() {
    let mut world = simulation_world(combat_ship(), REFERENCE_MONEY, 3);
    assert_eq!(world.revive_seconds(0), bims::health::REVIVE_SECONDS);
    assert_eq!(world.set_class(0, Class::Medic), Ok(()));
    hold_still(&mut world);
    // The aura at its top, and the medic within it: still two fifths.
    ranks(&mut world, 0, [0, 4, 0, 0]);
    assert_eq!(world.revive_seconds(0), class::MEDIC_REVIVE_SECONDS);
    assert_eq!(world.skill_of(0).revive, class::MEDIC_REVIVE_SECONDS);
    assert_eq!(world.skill_of(0).revived_to, class::MEDIC_REVIVED_TO);
    assert_eq!(world.revive_seconds(1), bims::health::REVIVE_SECONDS);
    assert_eq!(world.skill_of(1).revived_to, bims::health::REVIVED_TO);
    beside(&mut world, 2, 0);
    let up = revived_by(&mut world, 0, 2);
    assert_eq!(up, MAX_HEALTH * class::MEDIC_REVIVED_TO, "a medic's");
    assert!(world.heal_factor(2) > 1.0, "and in the aura all the same");
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

// --- B: Q, the Nanite Burst -------------------------------------------------------

/// **The burst heals at its rank, reaches its radius and waits its
/// cooldown**, the medic among those it heals.
#[test]
fn nanite_burst_heals_its_rank_s_points_within_its_radius_and_waits_its_cooldown() {
    let mut world = medic();
    let events = world.step(&[Command::NaniteBurst { slot: 0 }]);
    assert!(refused_with(&events, Refusal::NotLearnt), "{events:?}");
    let events = world.step(&[Command::NaniteBurst { slot: 1 }]);
    assert!(refused_with(&events, Refusal::NotAMedic), "{events:?}");
    let want = [
        (30.0, 4.0, 25.0),
        (40.0, 4.0, 22.0),
        (50.0, 5.0, 20.0),
        (60.0, 6.0, 18.0),
    ];
    for (rank, &(heal, radius, cooldown)) in (1..=4u8).zip(&want) {
        let mut world = medic_at([rank, 0, 0, 0]);
        assert_eq!(world.nanite_burst_heal(0), heal, "rank {rank}");
        assert_eq!(world.nanite_burst_radius(0), radius, "rank {rank}");
        assert_eq!(world.nanite_burst_cooldown(0), cooldown, "rank {rank}");
        hurt(&mut world, 1, 10.0);
        hurt(&mut world, 0, 20.0);
        let events = world.step(&[Command::NaniteBurst { slot: 0 }]);
        assert!(
            events
                .iter()
                .any(|e| *e == WorldEvent::NaniteBurst { who: 0, healed: 2 }),
            "rank {rank}: {events:?}"
        );
        assert_eq!(world.aboard.room.health(1), 10.0 + heal, "rank {rank}");
        assert_eq!(world.aboard.room.health(0), 20.0 + heal, "the medic too");
        let events = world.step(&[Command::NaniteBurst { slot: 0 }]);
        assert!(refused_with(&events, Refusal::CoolingDown), "{events:?}");
        run_for(&mut world, cooldown - 1.0);
        assert!(world.nanite_burst_cooldown_left(0) > 0.0, "rank {rank}");
        run_for(&mut world, 1.5);
        assert_eq!(world.nanite_burst_cooldown_left(0), 0.0, "rank {rank}");
        assert_eq!(world.can_nanite_burst(0), Ok(()));
        // Out past the radius: nothing.
        stand_off(&mut world, 1, 0, radius + 1.5);
        hurt(&mut world, 1, 10.0);
        world.step(&[Command::NaniteBurst { slot: 0 }]);
        assert_eq!(
            world.aboard.room.health(1),
            10.0,
            "rank {rank}: out of reach"
        );
    }
}

/// **Not through a wall, not a downed Bim**, and not by a medic downed.
#[test]
fn nanite_burst_heals_nobody_behind_a_wall_and_nobody_downed() {
    let mut world = medic_at([4, 0, 0, 0]);
    // A downed crewmate beside him: still downed, still at nought.
    world.aboard.room.knock_out_for_probe(1);
    world.step(&[]);
    assert!(world.aboard.room.is_downed(1));
    assert_eq!(world.nanite_burst_reaching(0), vec![0], "himself alone");
    world.step(&[Command::NaniteBurst { slot: 0 }]);
    assert!(world.aboard.room.is_downed(1), "it revives nobody");
    assert_eq!(world.aboard.room.health(1), 0.0);
    // The medic downed: refused.
    let mut world = medic_at([4, 0, 0, 0]);
    world.aboard.room.knock_out_for_probe(0);
    world.step(&[]);
    world.step(&[]);
    assert_eq!(world.can_nanite_burst(0), Err(Refusal::OutOfReach));
    // Behind a wall, within the radius: nothing.
    let mut world = medic_at([4, 0, 0, 0]);
    let radius = world.nanite_burst_radius(0);
    put_behind_a_wall(&mut world, radius).expect("a wall with deck both sides on the ship");
    hurt(&mut world, 1, 10.0);
    assert!(!world.nanite_burst_reaching(0).contains(&1));
    world.step(&[Command::NaniteBurst { slot: 0 }]);
    assert_eq!(world.aboard.room.health(1), 10.0, "the wall took it");
}

/// **The Healing Aura multiplies the burst, and it is ready at every
/// mission's start**, and shorter with the cooldown relics.
#[test]
fn nanite_burst_is_multiplied_by_the_aura_and_ready_at_every_mission() {
    let mut world = medic_at([1, 1, 0, 0]);
    hurt(&mut world, 1, 10.0);
    world.step(&[Command::NaniteBurst { slot: 0 }]);
    let want = 10.0 + class::NANITE_BURST_HEAL[0] * class::HEALING_AURA_FACTOR[0];
    assert!((world.aboard.room.health(1) - want).abs() < 1e-3);
    // Ready at every mission's start; the relics shorten it.
    let mut world = crewed_world(combat_ship(), REFERENCE_MONEY, 3, 3);
    assert_eq!(world.set_class(0, Class::Medic), Ok(()));
    world.step(&[]);
    ranks(&mut world, 0, [1, 0, 0, 1]);
    let (burst, cloak) = (world.nanite_burst_cooldown(0), world.cloak_cooldown(0));
    world.give_relic_for_probe(crate::relic::Relic::OverclockedCores);
    assert!(
        world.nanite_burst_cooldown(0) < burst,
        "*Overclocked Cores*"
    );
    assert!(world.cloak_cooldown(0) < cloak, "*Overclocked Cores*");
    world.step(&[Command::NaniteBurst { slot: 0 }]);
    world.step(&[Command::Cloak { slot: 0, target: 0 }]);
    assert!(world.nanite_burst_cooldown_left(0) > 0.0);
    assert!(world.cloak_cooldown_left(0) > 0.0);
    // Seconds taken off both (a *Reset Capacitor*'s way).
    let (burst_left, cloak_left) = (
        world.nanite_burst_cooldown_left(0),
        world.cloak_cooldown_left(0),
    );
    world.cooldowns_less(0, 3.0);
    assert!(
        world.nanite_burst_cooldown_left(0) < burst_left,
        "seconds off"
    );
    assert!(world.cloak_cooldown_left(0) < cloak_left, "seconds off");
    next_mission(&mut world);
    assert_eq!(world.nanite_burst_cooldown_left(0), 0.0);
    assert_eq!(world.cloak_cooldown_left(0), 0.0);
    assert!(!world.is_cloaked(0), "every cloak off");
    assert_eq!(world.can_nanite_burst(0), Ok(()));
}

// --- C: the Healing Aura ----------------------------------------------------------

/// **The factor and the radius by rank**, the medic himself included,
/// never while he is downed, and the higher of two medics — never both.
#[test]
fn the_healing_aura_s_factor_and_radius_by_rank_and_the_higher_of_two() {
    let want = [(1.15, 5.0), (1.20, 6.0), (1.25, 7.0), (1.30, 8.0)];
    for (rank, &(factor, radius)) in (1..=4u8).zip(&want) {
        let mut world = medic_at([0, rank, 0, 0]);
        assert_eq!(world.healing_aura_radius(0), radius, "rank {rank}");
        assert_eq!(world.heal_factor(0), factor, "rank {rank}: himself");
        stand_off(&mut world, 1, 0, radius - 0.5);
        assert_eq!(world.heal_factor(1), factor, "rank {rank}: within");
        stand_off(&mut world, 1, 0, radius + 1.0);
        assert_eq!(world.heal_factor(1), 1.0, "rank {rank}: beyond");
    }
    // Downed, the aura is off.
    let mut world = medic_at([0, 4, 0, 0]);
    assert!(world.heal_factor(1) > 1.0);
    world.aboard.room.knock_out_for_probe(0);
    world.step(&[]);
    world.step(&[]);
    assert_eq!(world.heal_factor(1), 1.0, "the medic downed");
    // Two medics reaching one Bim: the higher, never the two multiplied.
    let mut world = simulation_world(combat_ship(), REFERENCE_MONEY, 3);
    assert_eq!(world.set_class(0, Class::Medic), Ok(()));
    assert_eq!(world.set_class(1, Class::Medic), Ok(()));
    hold_still(&mut world);
    ranks(&mut world, 0, [0, 1, 0, 0]);
    ranks(&mut world, 1, [0, 4, 0, 0]);
    beside(&mut world, 1, 0);
    stand_off(&mut world, 2, 0, 2.0);
    assert_eq!(world.heal_factor(2), class::HEALING_AURA_FACTOR[3]);
    assert_eq!(world.healing_aura_reaching(2), Some(1));
}

/// **The aura multiplies the beam where the patient stands**: a patient
/// in it gains the beam times the factor, one the beam reaches outside it
/// the beam alone — wherever the medic is.
#[test]
fn the_healing_aura_multiplies_the_beam_and_not_outside_its_radius() {
    let gain = |tiles: f32| {
        let mut world = medic_at([0, 1, 1, 0]);
        let way = open_way(&world, 0, tiles);
        let at = world.aboard.room.bim_pos(0) + way * (tiles * TILE);
        world.aboard.room.put_for_probe(1, at);
        world.step(&[]);
        hurt(&mut world, 1, 20.0);
        assert!(linked(&beam(&mut world, 0, Some(1)), 0, Some(1)));
        let before = world.aboard.room.health(1);
        for _ in 0..(10 * STEPS_A_MINUTE) {
            world.step(&[]);
        }
        world.aboard.room.health(1) - before
    };
    let within = gain(2.0);
    let want = beam_gain(10.0) * class::HEALING_AURA_FACTOR[0];
    assert!(
        (within - want).abs() < 0.3,
        "in the aura: {within} of {want}"
    );
    // Five and a half tiles: the beam's six reach, the aura's five do not.
    let outside = gain(class::HEALING_AURA_RADIUS[0] + 0.5);
    assert!(
        (outside - beam_gain(10.0)).abs() < 0.3,
        "outside the aura: {outside}"
    );
}

/// **A Healing Sentry's heal and a relic's are multiplied too**:
/// everything that goes through `Health::heal`.
#[test]
fn the_healing_aura_multiplies_a_healing_sentry_and_a_relic() {
    // A Healing Sentry, laid by the engineer at slot 1, healing the
    // engineer, with the medic's aura at its top or not bought.
    let sentry_gain = |aura: u8| {
        let mut world = basic();
        assert_eq!(world.set_class(0, Class::Medic), Ok(()));
        assert_eq!(world.set_class(1, Class::Engineer), Ok(()));
        hold_still(&mut world);
        if aura > 0 {
            ranks(&mut world, 0, [0, aura, 0, 0]);
        }
        ranks(&mut world, 1, [0, 1, 0, 0]);
        beside(&mut world, 1, 0);
        let here = world.aboard.room.bim_pos(1);
        let (cx, cy) = (
            (here.x / TILE).floor() as i32,
            (here.y / TILE).floor() as i32,
        );
        let tile = (1i32..4)
            .flat_map(|r| (-r..=r).flat_map(move |dx| (-r..=r).map(move |dy| (cx + dx, cy + dy))))
            .find(|&t| world.can_deploy(1, DeployKind::HealingSentry, t).is_ok())
            .expect("a free tile by the engineer");
        let mut events = world.step(&[Command::Deploy {
            slot: 1,
            kind: DeployKind::HealingSentry,
            x: tile.0,
            y: tile.1,
        }]);
        let mut laid = false;
        for _ in 0..20_000 {
            if events
                .iter()
                .any(|e| matches!(e, WorldEvent::Deployed { who: 1, .. }))
            {
                laid = true;
                break;
            }
            events = world.step(&[]);
        }
        assert!(laid, "the Healing Sentry laid");
        beside(&mut world, 0, 1);
        hurt(&mut world, 1, 40.0);
        let before = world.aboard.room.health(1);
        for _ in 0..120 {
            world
                .aboard
                .room
                .put_for_probe(1, world.aboard.room.bim_pos(1));
            world.step(&[]);
        }
        world.aboard.room.health(1) - before
    };
    let plain = sentry_gain(0);
    let lifted = sentry_gain(4);
    assert!(plain > 0.0, "the sentry heals");
    assert!(
        (lifted / plain - class::HEALING_AURA_FACTOR[3]).abs() < 0.02,
        "the sentry's heal lifted: {lifted} against {plain}"
    );
    // *Nanite Mesh*'s regeneration on slot 1, beside the medic.
    let relic_gain = |aura: u8| {
        let mut world = medic();
        if aura > 0 {
            ranks(&mut world, 0, [0, aura, 0, 0]);
        }
        beside(&mut world, 1, 0);
        world.give_relic_for_probe(crate::relic::Relic::NaniteMesh);
        hurt(&mut world, 1, 20.0);
        let before = world.aboard.room.health(1);
        for _ in 0..(5 * STEPS_A_MINUTE) {
            world.step(&[]);
        }
        world.aboard.room.health(1) - before
    };
    let plain = relic_gain(0);
    let lifted = relic_gain(1);
    assert!(plain > 0.0, "the relic heals");
    assert!(
        (lifted / plain - class::HEALING_AURA_FACTOR[0]).abs() < 0.02,
        "the relic's heal lifted: {lifted} against {plain}"
    );
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

/// **No fire while beaming at ranks one and two; at half the rate from
/// the third.**
#[test]
fn a_medic_beaming_holds_fire_until_the_third_rank_and_then_fires_at_half() {
    for rank in 1..=4u8 {
        let mut world = medic_at([0, 0, rank, 0]);
        hurt(&mut world, 1, 40.0);
        assert!(linked(&beam(&mut world, 0, Some(1)), 0, Some(1)));
        let skill = world.skill_of(0);
        if rank < class::HEAL_BEAM_FIRE_RANK {
            assert!(skill.holds_fire, "rank {rank}: holds its fire");
            world.aboard.room.recruit_for_probe(0, true);
            world.step(&[]);
            assert!(!world.aboard.room.is_armed(0), "rank {rank}: holstered");
        } else {
            assert!(!skill.holds_fire, "rank {rank}: fires");
            assert_eq!(skill.fire_rate, class::HEAL_BEAM_FIRE_RATE, "rank {rank}");
        }
        // Unlinked, it fires at its own rate at every rank.
        beam(&mut world, 0, None);
        assert!(!world.skill_of(0).holds_fire);
        assert_eq!(world.skill_of(0).fire_rate, 1.0);
    }
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
        (gained - beam_gain(10.0)).abs() < 0.2,
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

// --- E: R, the Cloak -------------------------------------------------------------------

fn cloaked(events: &[WorldEvent], who: u32, target: u32) -> bool {
    events
        .iter()
        .any(|e| *e == WorldEvent::Cloaked { who, target })
}

/// **Its time, its pace and its cooldown by rank**, on the crewmate
/// under the pointer or on the medic himself.
#[test]
fn the_cloak_s_time_pace_and_cooldown_by_rank() {
    let mut world = medic();
    let events = world.step(&[Command::Cloak { slot: 0, target: 0 }]);
    assert!(refused_with(&events, Refusal::NotLearnt), "{events:?}");
    let want = [
        (6.0, 1.10, 60.0),
        (7.0, 1.15, 55.0),
        (8.0, 1.20, 50.0),
        (10.0, 1.25, 45.0),
    ];
    for (rank, &(seconds, pace, cooldown)) in (1..=4u8).zip(&want) {
        let mut world = medic_at([0, 0, 0, rank]);
        assert_eq!(world.cloak_seconds(0), seconds, "rank {rank}");
        assert_eq!(world.cloak_cooldown(0), cooldown, "rank {rank}");
        let walk = world.skill_of(1).walk;
        let events = world.step(&[Command::Cloak { slot: 0, target: 1 }]);
        assert!(cloaked(&events, 0, 1), "rank {rank}: {events:?}");
        assert!(world.is_cloaked(1) && !world.is_cloaked(0));
        assert!(
            (world.skill_of(1).walk - walk * pace).abs() < 1e-5,
            "rank {rank}"
        );
        assert!(world.skill_of(1).holds_fire, "rank {rank}");
        assert!(world.aboard.room.is_cloaked(1), "the room draws it");
        let events = world.step(&[Command::Cloak { slot: 0, target: 0 }]);
        assert!(refused_with(&events, Refusal::CoolingDown), "{events:?}");
        run_for(&mut world, seconds - 0.5);
        assert!(world.is_cloaked(1), "rank {rank}: still on");
        run_for(&mut world, 1.0);
        assert!(!world.is_cloaked(1), "rank {rank}: over");
        assert!(!world.aboard.room.is_cloaked(1));
        assert!(!world.skill_of(1).holds_fire);
        assert_eq!(world.skill_of(1).walk, walk);
        let left = world.cloak_cooldown_left(0);
        assert!(
            left > 0.0 && left <= cooldown - seconds + 1.0,
            "rank {rank}: {left}"
        );
    }
    // Himself, with nobody under the pointer.
    let mut world = medic_at([0, 0, 0, 1]);
    assert!(cloaked(
        &world.step(&[Command::Cloak { slot: 0, target: 0 }]),
        0,
        0
    ));
    assert!(world.is_cloaked(0));
}

/// **Refused out of range and out of sight**, and while the medic is
/// downed or not a medic.
#[test]
fn the_cloak_is_refused_out_of_range_and_out_of_sight() {
    let mut world = medic_at([0, 0, 0, 1]);
    assert_eq!(world.can_cloak(1, 0), Err(Refusal::NotAMedic));
    assert_eq!(world.can_cloak(0, 9), Err(Refusal::NotACrewmate));
    stand_off(&mut world, 1, 0, class::CLOAK_RANGE + 1.0);
    let far = world.aboard.room.bim_pos(1) - world.aboard.room.bim_pos(0);
    if far.len() > class::CLOAK_RANGE * TILE {
        let events = world.step(&[Command::Cloak { slot: 0, target: 1 }]);
        assert!(
            refused_with(&events, Refusal::OutOfCloakRange),
            "{events:?}"
        );
    }
    let mut world = medic_at([0, 0, 0, 1]);
    put_behind_a_wall(&mut world, class::CLOAK_RANGE).expect("a wall on the ship");
    let events = world.step(&[Command::Cloak { slot: 0, target: 1 }]);
    assert!(
        refused_with(&events, Refusal::NoSightOfTarget),
        "{events:?}"
    );
    let mut world = medic_at([0, 0, 0, 1]);
    world.aboard.room.knock_out_for_probe(0);
    world.step(&[]);
    world.step(&[]);
    assert_eq!(world.can_cloak(0, 1), Err(Refusal::OutOfReach));
}

/// **A cloaked Bim fires nothing and uses no ability, but walks — faster
/// — and revives**; a medic who cloaks himself lets his beam go.
#[test]
fn a_cloaked_bim_holds_fire_and_its_abilities_but_walks_and_revives() {
    // The medic cloaked: his beam let go, and his keys refused.
    let mut world = medic_at([1, 0, 1, 1]);
    assert!(linked(&beam(&mut world, 0, Some(1)), 0, Some(1)));
    world.step(&[Command::Cloak { slot: 0, target: 0 }]);
    assert!(world.patients_of(0).is_empty(), "the beam let go");
    assert!(!world.aboard.room.is_beaming(0));
    let events = world.step(&[Command::NaniteBurst { slot: 0 }]);
    assert!(refused_with(&events, Refusal::Cloaked), "{events:?}");
    let events = beam(&mut world, 0, Some(1));
    assert!(refused_with(&events, Refusal::Cloaked), "{events:?}");
    // Under arms, holstered.
    world.aboard.room.recruit_for_probe(0, true);
    world.step(&[]);
    assert!(!world.aboard.room.is_armed(0), "holds its fire");
    // It walks.
    let from = world.aboard.room.bim_pos(0);
    let there = from + open_way(&world, 0, 2.0) * (2.0 * TILE);
    world.step(&[Command::Crew {
        slot: 0,
        order: CrewOrder::SendTo {
            who: 0,
            x: there.x,
            y: there.y,
        },
    }]);
    run_for(&mut world, 2.0);
    assert!(
        (world.aboard.room.bim_pos(0) - from).len() > TILE,
        "walked while cloaked"
    );
    // A crewmate cloaked revives a downed one, at anybody's share.
    let mut world = simulation_world(combat_ship(), REFERENCE_MONEY, 3);
    assert_eq!(world.set_class(0, Class::Medic), Ok(()));
    hold_still(&mut world);
    ranks(&mut world, 0, [0, 0, 0, 4]);
    beside(&mut world, 1, 0);
    beside(&mut world, 2, 1);
    world.step(&[Command::Cloak { slot: 0, target: 1 }]);
    assert!(world.is_cloaked(1));
    let up = revived_by(&mut world, 1, 2);
    assert_eq!(
        up,
        MAX_HEALTH * bims::health::REVIVED_TO,
        "revived while cloaked"
    );
}

/// **A downed Bim can be cloaked, and its countdown keeps running**; a
/// second cloak takes the longer time and never adds them.
#[test]
fn a_downed_bim_can_be_cloaked_and_a_second_cloak_takes_the_longer() {
    let mut world = medic_at([0, 0, 0, 1]);
    world.aboard.room.knock_out_for_probe(1);
    world.step(&[]);
    assert!(world.aboard.room.is_downed(1));
    assert_eq!(world.can_cloak(0, 1), Ok(()));
    world.step(&[Command::Cloak { slot: 0, target: 1 }]);
    assert!(world.is_cloaked(1));
    let left = world.aboard.room.down_left(1).expect("a countdown");
    run_for(&mut world, 2.0);
    let later = world.aboard.room.down_left(1).expect("still down");
    assert!(
        later < left - 1.5,
        "the countdown runs: {left} then {later}"
    );
    // Two medics: a long cloak then a short one keeps the long; a short
    // then a long takes the long.
    for long_first in [true, false] {
        let mut world = crewed_world(combat_ship(), REFERENCE_MONEY, 2, 3);
        assert_eq!(world.set_class(0, Class::Medic), Ok(()));
        assert_eq!(world.set_class(1, Class::Medic), Ok(()));
        hold_still(&mut world);
        ranks(&mut world, 0, [0, 0, 0, 1]);
        ranks(&mut world, 1, [0, 0, 0, 4]);
        beside(&mut world, 1, 0);
        // The target on the medic's other side, in sight of both: packed
        // in beside the two of them, a bot is shoved off a player's own
        // Bim by the whole of the overlap (September 2026), and out of the
        // second medic's sight round the corner of its tile.
        stand_off(&mut world, 2, 0, -1.0);
        let (first, second) = if long_first { (1, 0) } else { (0, 1) };
        world.step(&[Command::Cloak {
            slot: first,
            target: 2,
        }]);
        world.step(&[Command::Cloak {
            slot: second,
            target: 2,
        }]);
        let left = world.cloak_left(2);
        let long = class::CLOAK_SECONDS[3];
        assert!(
            left <= long && left > long - 0.2,
            "long first {long_first}: {left}"
        );
        assert_eq!(world.cloak_of(2).pace, class::CLOAK_PACE[3]);
    }
}

/// **A Guardian's sweep still hits a cloaked Bim**: it targets nobody.
#[test]
fn a_sweep_still_hits_a_cloaked_bim() {
    let mut world = medic_at([0, 0, 0, 1]);
    world.step(&[Command::Cloak { slot: 0, target: 1 }]);
    assert!(world.is_cloaked(1));
    let at = world.aboard.room.bim_pos(1);
    let way = open_way(&world, 1, 5.0);
    let across = vec2(-way.y, way.x) * (2.0 * TILE);
    let from = at + way * (5.0 * TILE);
    world.aboard.room.enemy_sweep(
        from,
        at - across,
        at + across,
        WeaponKind::Sweeper.basic(),
        30.0,
        1.0,
    );
    for _ in 0..40 {
        world.step(&[]);
    }
    assert!(world.aboard.room.health(1) < MAX_HEALTH, "the sweep landed");
}

/// **No enemy picks a cloaked Bim, one aiming at it drops it the same
/// step, and an enemy with nobody it may pick holds where it stands and
/// fires nothing** — picking again the step the cloak is off.
#[test]
fn no_enemy_picks_a_cloaked_bim_and_one_with_nobody_to_pick_holds() {
    // One crew member, the medic, inside the door with a Trooper armed
    // four tiles down the corridor.
    let mut world = crewed_world(combat_ship(), REFERENCE_MONEY, 1, 1);
    assert_eq!(world.set_class(0, Class::Medic), Ok(()));
    ranks(&mut world, 0, [0, 0, 0, 1]);
    assert!(world.stage_droid_fight_for_probe(
        bims::droid::DroidKind::Trooper,
        Some(WeaponKind::LaserPistol.basic())
    ));
    let theirs = |world: &World| {
        world
            .residents
            .as_ref()
            .unwrap()
            .aboard
            .room
            .combat_targets_for_probe()
            .first()
            .copied()
            .flatten()
    };
    let mut seen = false;
    for _ in 0..(3 * STEPS_A_MINUTE) {
        world.aboard.room.patch_up_for_probe(0);
        world.step(&[]);
        if theirs(&world).is_some() {
            seen = true;
            break;
        }
    }
    assert!(seen, "the machine has the medic on its list");
    let events = world.step(&[Command::Cloak { slot: 0, target: 0 }]);
    assert!(cloaked(&events, 0, 0), "{events:?}");
    world.step(&[]);
    assert_eq!(theirs(&world), None, "dropped the same step");
    // Nobody it may pick: it holds, facing as it was, and fires nothing —
    // once a turn under way has come round.
    run_for(&mut world, 1.5);
    let droid = |world: &World| {
        let d = world
            .residents
            .as_ref()
            .unwrap()
            .aboard
            .room
            .droid(0)
            .unwrap();
        (d.pos, d.facing())
    };
    let (pos, facing) = droid(&world);
    let health = world.aboard.room.health(0);
    run_for(&mut world, class::CLOAK_SECONDS[0] - 2.5);
    assert!(world.is_cloaked(0));
    let (now, turned) = droid(&world);
    assert!(
        (now - pos).len() < 1.0,
        "held where it stood: {pos:?} {now:?}"
    );
    assert!((turned - facing).len() < 1e-2, "facing kept");
    assert_eq!(world.aboard.room.health(0), health, "nothing fired at it");
    // The cloak off, it picks again.
    let mut back = false;
    for _ in 0..(3 * STEPS_A_MINUTE) {
        world.aboard.room.patch_up_for_probe(0);
        world.step(&[]);
        if theirs(&world).is_some() {
            back = true;
            break;
        }
    }
    assert!(!world.is_cloaked(0));
    assert!(back, "picked again once allowed");
}

/// The medic's state goes with its body, and a game with a medic reads
/// the same twice.
#[test]
fn a_medic_s_state_dies_with_it_and_a_game_with_a_medic_reads_the_same_twice() {
    let mut world = medic_at([1, 1, 1, 1]);
    world.step(&[Command::NaniteBurst { slot: 0 }]);
    world.step(&[Command::Cloak { slot: 0, target: 0 }]);
    assert!(world.medic_of(0).last_burst.is_some());
    world.aboard.room.kill_for_probe(0);
    world.step(&[]);
    world.step(&[]);
    assert_eq!(
        world.medic_of(0),
        crate::medic::Medic::default(),
        "gone with it"
    );
    assert!(!world.is_cloaked(0));
    // The level and the ranks are the player's, kept for its buyback.
    assert_eq!(world.rank_of(0, class::SLOT_R), 1);
    // Two runs of the kit on one seed are one world, and the cloak and the
    // burst are in the checksum.
    let run = || {
        let mut world = medic_at([1, 1, 1, 1]);
        hurt(&mut world, 1, 60.0);
        beam(&mut world, 0, Some(1));
        world.step(&[Command::NaniteBurst { slot: 0 }]);
        world.step(&[Command::Cloak { slot: 0, target: 1 }]);
        for _ in 0..(3 * STEPS_A_MINUTE) {
            world.step(&[]);
        }
        world_checksum(&world)
    };
    assert_eq!(run(), run());
    let mut a = medic_at([0, 0, 0, 1]);
    let before = world_checksum(&a);
    a.step(&[Command::Cloak { slot: 0, target: 1 }]);
    let mut b = medic_at([0, 0, 0, 1]);
    b.step(&[]);
    assert_ne!(
        world_checksum(&a),
        world_checksum(&b),
        "the cloak is hashed"
    );
    assert_ne!(world_checksum(&a), before);
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

/// A hire breaks every beam, and so does a bot lost off the crew — an
/// index is all a link is.
#[test]
fn beams_are_cleared_by_a_hire_and_a_bot_lost() {
    use crate::armour::LootSource;
    use crate::data;
    use worldgen::GalaxyType;
    let galaxy = worldgen::Galaxy::new(data::DEFAULT_SEED, GalaxyType::SpiralTwoArm);
    let (star, station) = crate::spawn(&galaxy).unwrap();
    let mut world = World::start_with_crew(
        combat_ship(),
        REFERENCE_MONEY,
        2,
        2,
        data::DEFAULT_SEED,
        GalaxyType::SpiralTwoArm,
        star,
        station,
    )
    .unwrap();
    // A hire, not a fight: the spawn a peaceful stop (task 111).
    world.set_quiet_sites_for_probe(true);
    assert_eq!(world.set_class(0, Class::Medic), Ok(()));
    hold_still(&mut world);
    // The beam's first rank, since a medic of nought ranks has none.
    ranks(&mut world, 0, [0, 0, 1, 0]);
    assert!(world.mercenary_for_probe());
    let merc = world.residents.as_ref().unwrap().aboard.count() - 1;
    assert!(world.mercenary_fee(merc).is_some());
    // Linked to the crewmate; the hire breaks every beam.
    beside(&mut world, 1, 0);
    assert!(linked(&beam(&mut world, 0, Some(1)), 0, Some(1)));
    assert_eq!(world.patients_of(0), vec![1]);
    let at = world.body_position(LootSource::Resident(merc)).unwrap();
    world.aboard.room.put_for_probe(0, at + vec2(30.0, 0.0));
    let events = world.step(&[Command::Hire {
        slot: 0,
        who: 0,
        resident: merc,
    }]);
    assert!(events.contains(&WorldEvent::Hired { who: 2 }), "{events:?}");
    assert_eq!(world.aboard.crew_count(), 3);
    assert!(world.patients_of(0).is_empty(), "cleared by the hire");
    assert!(!world.is_beaming(0));
    // Linked to the mercenary; the hand dead and gone with the site —
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
