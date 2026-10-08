//! Ascensions (October 2026, `crate::ascension`): each level on top of
//! the ones under it, set on a run's world as the game setup deals it.

use shipdesign::fixture::flyer;
use worldgen::GalaxyType;

use crate::data;
use crate::droid::WaveScaling;
use crate::fixture::REFERENCE_MONEY;
use crate::relic::Profile;
use crate::world::World;

/// A run opened at the default seed's spawn with the floor on and the
/// sites as the game has them, so the elites stand where they roll.
fn floor_world() -> World {
    let seed = data::DEFAULT_SEED;
    let galaxy_type = GalaxyType::SpiralTwoArm;
    let galaxy = worldgen::Galaxy::new(seed, galaxy_type);
    let (star, station) = crate::spawn(&galaxy).expect("the default seed has a dock");
    let mut world = World::start(
        flyer(2),
        REFERENCE_MONEY,
        1,
        seed,
        galaxy_type,
        star,
        station,
    )
    .expect("the world opens");
    world.set_floor(true);
    world
}

/// The fights on the floor that are a system's elite (not an Area
/// defend's, which are elite fights by their own rule), and the fights.
fn elites_and_fights(world: &World) -> (usize, usize) {
    let marks = world.floor_marks();
    let fights = marks
        .iter()
        .filter(|m| !m.heart && m.kind != crate::run::SiteKind::Trader && m.row > 0)
        .count();
    let elites = marks.iter().filter(|m| m.elite && !m.area).count();
    (elites, fights)
}

#[test]
fn swarming_elites_lays_more_elites_on_the_floor_and_none_in_area_0() {
    let mut world = floor_world();
    let (before, fights) = elites_and_fights(&world);
    let laid = world.floor().expect("the floor is on").clone();
    world.set_ascension(1);
    let (after, _) = elites_and_fights(&world);
    // A galaxy has some twenty elite systems, so the floor runs out of
    // them before a fifth of its fights is one: the default seed's goes
    // from seven to thirteen.
    assert!(
        after >= before + fights / 20,
        "{before} elites of {fights} fights, {after} at ascension one"
    );
    let tier_one = world.scaling().tier_one_day();
    for mark in world.floor_marks() {
        if mark.elite && !mark.area {
            assert!(crate::floor::row_day(mark.row) >= tier_one, "{mark:?}");
        }
    }
    // Nought again is the floor it was.
    world.set_ascension(0);
    assert_eq!(*world.floor().expect("the floor is on"), laid);
}

#[test]
fn tougher_enemies_and_harder_hitters_come_at_two_and_three() {
    let mut world = floor_world();
    assert_eq!(world.enemy_health_factor(), None);
    let taken = world.skill_of(0).damage_taken;
    world.set_ascension(1);
    assert_eq!(world.enemy_health_factor(), None);
    world.set_ascension(2);
    assert_eq!(world.enemy_health_factor(), Some(1.1));
    assert_eq!(world.skill_of(0).damage_taken, taken);
    world.set_ascension(3);
    assert!((world.skill_of(0).damage_taken - taken * 1.1).abs() < 1e-6);
    // Past the highest is the highest.
    world.set_ascension(99);
    assert_eq!(world.ascension(), crate::ascension::MOST);
}

#[test]
fn more_enemies_and_one_more_wave_are_the_scaling_s() {
    let mut world = floor_world();
    world.set_wave_scaling(WaveScaling::DEFAULT);
    let heart = world.scaling().heart_day();
    let was = world.scaling().per_player_on(heart);
    let waves = world.scaling().waves(1);
    let budget = world.scaling().area_on(1).budget_hundredths();
    world.set_ascension(4);
    assert!(world.scaling().per_player_on(heart) * 100 >= was * 109);
    assert_eq!(world.scaling().waves(1), waves);
    assert_eq!(world.scaling().area_on(1).budget_hundredths(), budget);
    world.set_ascension(5);
    assert_eq!(world.scaling().waves(1), waves + 1);
    // And every mission's budget a wave more.
    assert_eq!(
        world.scaling().area_on(1).budget_hundredths(),
        budget + 100
    );
    // The tuning file's own dials are untouched.
    assert_eq!(world.wave_scaling(), WaveScaling::DEFAULT);
}

#[test]
fn a_win_opens_the_next_ascension_and_never_closes_one() {
    let mut profile = Profile::new();
    assert_eq!(profile.ascension, 0);
    profile.record_win(0);
    assert_eq!((profile.wins, profile.ascension), (1, 1));
    profile.record_win(0);
    assert_eq!(profile.ascension, 1);
    profile.record_win(3);
    assert_eq!(profile.ascension, 4);
    profile.record_win(crate::ascension::MOST);
    assert_eq!(profile.ascension, crate::ascension::MOST);
}
