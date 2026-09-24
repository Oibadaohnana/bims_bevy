//! The commands the app launches, played as they played before the old
//! game was deleted (feature 104).
//!
//! The world's own `tests_survivors.rs` pins a seeded run built out of
//! world calls. This is the other half: every session the app stands up
//! for a command — `droids`, a tier test, a class fight, `test`,
//! `test_planet`, `droids_planet`, `defense`, `crisis`, `jammer`, the
//! simulation and the game's own run — built here the way
//! `screens::game::open` builds it, with the dials at the commands' own
//! values and the random commands' seed and roll written down, stepped a
//! while, and read with `world::fixture::Survivors`: only the state that
//! outlived the deletion. Each number was taken off the tree before
//! anything was deleted (the `needs-sim-final` tag).

use bims::combat::Tier;
use shipdesign::fixture::combat_ship;
use world::Class;
use world::fixture::Survivors;

use crate::Session;
use crate::session::{pick_dock, pick_ground};

const W: f32 = 1400.0;
const H: f32 = 900.0;
/// The seed and the roll the random commands are read at.
const SEED: u64 = 12_345;
const ROLL: u64 = 678;
/// The commands' own dials (`screens::game`): a minute between waves,
/// three waves, a minute before a town's first.
const REINFORCE: f64 = 1.0;
const WAVES: u32 = 3;
const DEFENSE_DELAY: f64 = 1.0;

fn read(mut session: Session, steps: u32) -> u64 {
    for _ in 0..steps {
        session.world_step();
    }
    let mut hash = Survivors::new();
    hash.eat_world(&session.game.as_ref().expect("a game").world);
    hash.value()
}

/// `test` and the commands built on it: the combat ship with one crew
/// member, docked where the roll says, a mercenary for hire.
fn tested(pick: fn(u64, u32, u64) -> Option<(u32, u32)>) -> Session {
    let spawn = pick(SEED, 0, ROLL);
    assert!(spawn.is_some(), "the roll finds somewhere");
    let mut session = Session::simulate_on(combat_ship(), 1, SEED, 0, spawn, W, H);
    session.mercenary_for_probe();
    session
}

fn commands() -> Vec<(&'static str, u64)> {
    let seed = world::data::DEFAULT_SEED;
    let mut out = Vec::new();

    out.push((
        "simulation",
        read(Session::simulate(seed, 0, None, W, H), 600),
    ));

    let spawn = world::spawn(&worldgen::Galaxy::new(seed, crate::session::galaxy_type(0)));
    out.push((
        "game",
        read(
            Session::run(
                world::data::START_MONEY_PER_BIM,
                2,
                0,
                seed,
                0,
                spawn,
                &[Class::Soldier, Class::Medic],
                W,
                H,
            ),
            600,
        ),
    ));

    out.push((
        "droids",
        read(
            Session::droids(seed, None, REINFORCE, None, WAVES, W, H),
            1500,
        ),
    ));

    out.push((
        "tier2_test",
        read(
            Session::droids_at_tier(seed, Tier::Two, REINFORCE, None, WAVES, W, H),
            900,
        ),
    ));

    let mut medic = Session::droids(seed, None, REINFORCE, None, WAVES, W, H);
    if let Some(game) = medic.game.as_mut() {
        let _ = game.world.set_class(0, Class::Medic);
    }
    out.push(("combat_droids_medic", read(medic, 900)));

    out.push(("test", read(tested(pick_dock), 600)));

    let mut planet = tested(pick_ground);
    planet.land_for_probe();
    out.push(("test_planet", read(planet, 900)));

    let mut held = tested(pick_ground);
    held.land_for_probe();
    held.infest_the_dock_for_probe(None, REINFORCE, WAVES);
    out.push(("droids_planet", read(held, 900)));

    let mut defense = tested(pick_ground);
    defense.land_for_probe();
    defense.defense_for_probe(DEFENSE_DELAY, REINFORCE, WAVES);
    out.push(("defense", read(defense, 1500)));

    let mut crisis = tested(pick_dock);
    crisis.crisis_for_probe(0);
    out.push(("crisis", read(crisis, 600)));

    let mut jammer = tested(pick_dock);
    jammer.jammer_for_probe(None, REINFORCE, WAVES);
    out.push(("jammer", read(jammer, 900)));

    out
}

/// What each command came to before the deletion.
const PINNED: [(&str, u64); 11] = [
    ("simulation", 0x_1253_d38e_edbd_1192),
    ("game", 0x_b817_4a5d_28b1_de71),
    ("droids", 0x_df45_19b3_735c_4a33),
    ("tier2_test", 0x_2f3e_804a_355f_9cf3),
    ("combat_droids_medic", 0x_cc8b_5e2f_2320_a411),
    ("test", 0x_4d32_ad42_ac1c_4591),
    ("test_planet", 0x_8f91_50e0_31b0_10ec),
    ("droids_planet", 0x_e0b9_f2bd_f4b4_10db),
    ("defense", 0x_0e7f_1c4d_4f87_9ace),
    ("crisis", 0x_0876_98de_a683_2bfa),
    ("jammer", 0x_ea1e_1f88_d357_2888),
];

/// **Every command plays as it did**: each session the app builds, read
/// over what survived the deletion, comes out at the number it came to
/// before anything was deleted.
#[test]
fn every_command_plays_as_it_did_before_the_old_game_was_deleted() {
    let got = commands();
    let moved: Vec<String> = got
        .iter()
        .zip(PINNED.iter())
        .filter(|((_, g), (_, p))| g != p)
        .map(|((name, g), (_, p))| format!("{name}: {g:#018x} where {p:#018x} was pinned"))
        .collect();
    assert!(
        moved.is_empty(),
        "commands moved:\n{}\nall: {got:#x?}",
        moved.join("\n")
    );
}

/// FNV-1a over a picture's floats, bit for bit.
fn picture_hash(shapes: &[f32]) -> u64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for value in shapes {
        for byte in value.to_bits().to_le_bytes() {
            hash ^= byte as u64;
            hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
        }
    }
    hash
}

/// The pictures, beside the state (feature 104): the fixtures that now
/// do nothing stayed in the part list **as visuals**, so the designer's
/// picture of them (`paint::fixtures`, the room laid out from the design
/// and asked for its fixtures' pictures) and a run's own picture of its
/// deck — `Session::render`, the shape buffer the canvas draws, after a
/// while of the simulation and of the fight — have to be what they were.
/// These are floats bit for bit, so a change that draws the same thing
/// another way moves them too: re-pin one only for a change that says
/// in this note why the picture is the same picture.
fn pictures() -> Vec<(&'static str, u64)> {
    use shipdesign::fixture::{combat_ship, playtest_ship};
    let seed = world::data::DEFAULT_SEED;
    let mut out = vec![
        (
            "designer_playtest",
            picture_hash(crate::paint::fixtures(&playtest_ship()).shapes()),
        ),
        (
            "designer_combat",
            picture_hash(crate::paint::fixtures(&combat_ship()).shapes()),
        ),
    ];
    let mut simulation = Session::simulate(seed, 0, None, W, H);
    for _ in 0..300 {
        simulation.world_step();
    }
    out.push(("simulation_deck", picture_hash(simulation.render())));
    let mut droids = Session::droids(seed, None, REINFORCE, None, WAVES, W, H);
    for _ in 0..600 {
        droids.world_step();
    }
    out.push(("droids_deck", picture_hash(droids.render())));
    out
}

const PICTURES: [(&str, u64); 4] = [
    ("designer_playtest", 0x_9b08_8f44_06ad_4414),
    ("designer_combat", 0x_157a_1c34_2e97_18e0),
    ("simulation_deck", 0x_870e_965b_f9ec_1ca4),
    ("droids_deck", 0x_2bd2_5009_73a3_8c09),
];

#[test]
fn the_fixtures_and_a_run_s_deck_are_drawn_as_they_were() {
    let got = pictures();
    let moved: Vec<String> = got
        .iter()
        .zip(PICTURES.iter())
        .filter(|((_, g), (_, p))| g != p)
        .map(|((name, g), (_, p))| format!("{name}: {g:#018x} where {p:#018x} was pinned"))
        .collect();
    assert!(moved.is_empty(), "pictures moved:\n{}", moved.join("\n"));
}
