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
///
/// **Three moved once since, on purpose**: procedural towns (feature
/// 112). `test_planet`, `droids_planet` and `defense` set the ship down at
/// a town, and a town's streets, gates, hall and lots are drawn from its
/// seed now, so everybody on it stands somewhere else. They were
/// `0x_8f91_50e0_31b0_10ec`, `0x_e0b9_f2bd_f4b4_10db` and
/// `0x_0e7f_1c4d_4f87_9ace` ([`PINNED_BEFORE_112`]); with every town on
/// the template and every station on its drawn plan
/// (`world::station::set_legacy_layouts`) all eleven came back
/// (`the_commands_come_back_under_the_old_layouts`). The rest did not move:
/// every other command docks at the spawn, a hub still, or the arena.
///
/// **All eleven moved once more, on purpose**: nothing stored (task 113).
/// The reading takes every body's gear by its `Debug`, and `Gear` lost
/// the pack and gained the charges; and the sessions play differently by
/// the task's own rules — the design's gear in the armory rather than
/// the hold, a body out cold keeping its gun (`world`'s `SURVIVORS` note
/// has the list). `PICTURES` did not move.
///
/// **`jammer` moved once more, on purpose**: the trader (task 114). A
/// trader is never the machines' — the crisis passes it by and it is
/// closed instead — and `World::infest_here_for_probe`, which is the
/// crisis at once, passes it by too, so a trader of the command's system
/// stands untaken. It was `0x_3bb8_9b7e_8ffe_057f`; with the probe
/// infesting the trader as before it came back, and it is the same number
/// under the old layouts (`PINNED_BEFORE_112`). Nothing else moved.
const PINNED: [(&str, u64); 11] = [
    ("simulation", 0x_37d6_6b39_8298_9325),
    ("game", 0x_123b_cbc1_3bd0_f5e2),
    ("droids", 0x_58e5_da1b_7aeb_4c13),
    ("tier2_test", 0x_faf7_7de2_1506_a29b),
    ("combat_droids_medic", 0x_8897_a1fc_b6fc_683e),
    ("test", 0x_551a_3ee8_1e3a_44cb),
    ("test_planet", 0x_c01a_a1aa_1b48_feb5),
    ("droids_planet", 0x_2b9c_d125_981c_4147),
    ("defense", 0x_f7d9_18b0_5265_5f26),
    ("crisis", 0x_9ecf_6122_fd41_05b8),
    ("jammer", 0x_26e0_b44c_8492_25f3),
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
///
/// The two decks were re-pinned once on purpose (feature 106): research
/// left the game, and the gold ring of lights round a station's research
/// desk that held a key went with it — the spawn's desk, lit on both
/// decks. Drawing that ring by the old key rule again gave back the old
/// two numbers bit for bit, so the ring is the whole of the move. The
/// same ring now lights a desk with a relic cache on it.
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
    ("simulation_deck", 0x_234f_3e03_869d_4c31),
    ("droids_deck", 0x_549f_77b7_8357_3bf8),
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

/// What `PINNED` comes to under the layouts before feature 112, for the
/// check below. Taken again under task 113, which moved every one on
/// every layout (`PINNED`'s note): the eight that never stand on a town
/// are `PINNED`'s own, and the three that do differ by the town alone.
const PINNED_BEFORE_112: [(&str, u64); 11] = [
    ("simulation", 0x_37d6_6b39_8298_9325),
    ("game", 0x_123b_cbc1_3bd0_f5e2),
    ("droids", 0x_58e5_da1b_7aeb_4c13),
    ("tier2_test", 0x_faf7_7de2_1506_a29b),
    ("combat_droids_medic", 0x_8897_a1fc_b6fc_683e),
    ("test", 0x_551a_3ee8_1e3a_44cb),
    ("test_planet", 0x_1670_f153_e5c8_e2fd),
    ("droids_planet", 0x_1545_0c14_606b_a1dc),
    ("defense", 0x_3ff6_9a47_94c1_b073),
    ("crisis", 0x_9ecf_6122_fd41_05b8),
    ("jammer", 0x_26e0_b44c_8492_25f3),
];

/// Not a test of its own, and **run alone** (`--exact`), since it flips
/// the process-wide switch to the layouts before feature 112: with every
/// station on the drawn plan its seed rolled and every town on the
/// template, every command and every picture comes back to what it was —
/// so the layouts are the whole of what moved them.
/// `cargo test -p ship -- --ignored --exact --nocapture
/// tests_survivors::the_commands_come_back_under_the_old_layouts`.
#[test]
#[ignore]
fn the_commands_come_back_under_the_old_layouts() {
    world::station::set_legacy_layouts(true);
    let got = commands();
    let pictures = pictures();
    world::station::set_legacy_layouts(false);
    for ((name, g), (_, p)) in got.iter().zip(PINNED_BEFORE_112.iter()) {
        println!("{name}: {g:#018x} under the old layouts, {p:#018x} before");
    }
    assert_eq!(got.to_vec(), PINNED_BEFORE_112.to_vec());
    assert_eq!(pictures.to_vec(), PICTURES.to_vec());
}
