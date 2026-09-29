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
///
/// **`droids`, `tier2_test` and `combat_droids_medic` moved once more, on
/// purpose**: the minigun
/// and the rail lance (task 115). `Session::combat` deals
/// `WeaponKind::ALL` down its sixteen, and the two new kinds are in it, so
/// four of the crew carry them now. They were `0x_58e5_da1b_7aeb_4c13`,
/// `0x_faf7_7de2_1506_a29b` and `0x_8897_a1fc_b6fc_683e`; with the deal cut
/// back to the first five kinds all eleven came back, so the deal is the
/// whole of the move. Under the old layouts the three are the same
/// numbers, the arena not being generated.
///
/// **All eleven moved once more, on purpose**: every site an attack, a
/// defence or a trader (task 111), a change meant to alter how a run
/// plays. Every site that is neither a trader nor an enemy's is a defence
/// from the first day, so `simulation`, `game`, `test`, `crisis` and the
/// towns meet the machines twenty seconds in with their crew stood ashore
/// and armed defenders on the site's deck; and the reading hashes whether
/// each site of the system is threatened, which every one of them now is,
/// so the held arena's commands moved by that alone. And no mining outpost
/// is built, so the galaxy's stations are others. They were
/// `0x_37d6_6b39_8298_9325`, `0x_123b_cbc1_3bd0_f5e2`,
/// `0x_4b40_a4f0_a589_d45f`, `0x_4a8f_5a2b_e8a8_e253`,
/// `0x_a6c8_5835_4a8f_84e2`, `0x_551a_3ee8_1e3a_44cb`,
/// `0x_c01a_a1aa_1b48_feb5`, `0x_2b9c_d125_981c_4147`,
/// `0x_f7d9_18b0_5265_5f26`, `0x_9ecf_6122_fd41_05b8` and
/// `0x_26e0_b44c_8492_25f3`.
///
/// **All eleven moved once more, on purpose**: worldgen's
/// `GENERATOR_VERSION` 8 — six hundred stars, every system a station and a
/// planet to land on — with a trader in one system in ten. Every seed is
/// another galaxy, so every command opens at another dock in another
/// system; no rule of the room or the world's fight moved. They were
/// `0x_3a78_fda9_a405_e7d3`, `0x_074c_fece_8e0c_0155`,
/// `0x_fb46_6db9_8b79_539f`, `0x_603a_ae26_b955_6193`,
/// `0x_f9ff_1d94_1432_08a2`, `0x_ee69_befd_0b9f_7707`,
/// `0x_ff5f_353f_2c2b_41e4`, `0x_eb66_c363_5af3_b9a2`,
/// `0x_da90_308b_cddb_d152`, `0x_809f_8d4d_c22a_3798` and
/// `0x_a4e9_578d_80d0_3102`.
///
/// **All eleven moved again, on purpose**: task 120's health rework. A
/// body is one bar of hit points now — no blood, no wounds, no traumas, no
/// bandage or medkit — downed at nought and dead thirty seconds later
/// unless revived, and the reading takes a body's health as that bar and
/// its countdown where it took the blood and the parts. So every fight
/// is fought on other numbers, the drug lab is off the playtest ship, and
/// the medicine is out of every pack. They were `0x_97f1_bc58_be4b_3934`,
/// `0x_0342_a7aa_dddd_ee12`, `0x_b1eb_4f60_28a6_a0ab`,
/// `0x_cf1d_e29c_baeb_ef27`, `0x_498b_db72_80bb_60ea`,
/// `0x_cdcd_f1e3_0d39_a470`, `0x_c492_6d86_74bd_bb05`,
/// `0x_5b55_867b_fa3d_10a4`, `0x_d1c5_f4e7_b7d6_820f`,
/// `0x_f4ba_4e20_6eee_6acb` and `0x_b77c_ecb6_e0f6_2c3a`.
///
/// **`game` moved alone, on purpose**: task 124's soldier (slot 0 of the
/// run, beside a medic) climbs its ranked kit's sixteen levels and starts
/// with no grenade at rank nought, where it had two in its pack; task 127
/// made every class charge a counter on the world, so no charge is in any
/// gear the reading takes. No other command has a soldier or an engineer
/// in it, and none of the other ten moved. It was `0x_d30e_877d_b1e4_5020`.
///
/// **All eleven moved again, on purpose**, re-pinned once for the five
/// changes that landed together on 28 September 2026 and were tested as
/// one tree: nobody walks about at random any more and the player's own
/// Bim is moved by nobody but its player — it takes up no errand of its
/// own, a bot shoved off it under arms gives the whole of the ground, and
/// it takes arms with the alarm (the room's wander drew on the room's
/// stream every step, so every roll after moved); task 131 (#16) —
/// Manufacturers attack a defence before day ten, station waves thirty
/// seconds apart, a field medic sets a carried body down, arrival spots
/// on reachable deck; task 131 (#20) — every generated station another
/// building, the waves in at every airlock but the port in turn; task
/// 130 — the medic's ranked kit (up at two fifths, a beam four times the
/// heal); and task 132's uncapped wave. Their shares were not taken apart.
/// They were `0x_99c9_324d_6ab6_208a`, `0x_48d6_94af_8626_5246`,
/// `0x_7122_55ef_2011_e57f`, `0x_09bb_2ad9_b012_ca13`,
/// `0x_3cce_d174_8375_71d0`, `0x_a47e_3196_aa47_069a`,
/// `0x_c6ba_960c_ef87_9b8b`, `0x_7086_fe1a_a578_b006`,
/// `0x_a471_03e9_1bfb_6f17`, `0x_0050_4e32_409e_6da9` and
/// `0x_acb8_1d88_be11_b1a0`.
///
/// **Three moved, on purpose**: every wave of a run's first mission is
/// one machine fewer than the formula (`data::FIRST_MISSION_WAVE_EASE`),
/// unless a probe forced its size. `droids_planet`, `defense` and
/// `jammer` meet unforced waves in their first mission; the other eight
/// force theirs or meet none, and did not move. They were
/// `0x_a052_db80_4b5e_a436`, `0x_f913_4921_8ef6_d21b` and
/// `0x_5c5f_3a45_26c7_942e`.
///
/// **All eleven moved, on purpose**, for the tasks of 29 September 2026
/// in the tree together: task 135 (a system offers one station and one
/// town — the home station and a town at home — so every command's world
/// holds fewer sites, and one fight a system), task 136 (the machines hold
/// every other site from the first day, a defence pays no money) and
/// task 133 (a fight won freezes the deck: `test_planet`'s). The
/// sessions use no dial, so they are the game as it plays; the shares
/// were not taken apart. They were `0x_f6c6_0d7c_076a_1bcf`,
/// `0x_ef2f_154c_5bef_a8ba`, `0x_ed09_7fbe_b2fa_4fa1`,
/// `0x_14ad_eda0_295a_98d2`, `0x_a988_b611_0ad1_42b5`,
/// `0x_606f_c276_801f_6d7e`, `0x_9012_786f_22c6_0c02`,
/// `0x_281f_b990_f3c9_e671`, `0x_5376_eb05_b524_e099`,
/// `0x_c5f1_4fd2_88f6_6279` and `0x_6507_468b_fa27_bde8`.
///
/// **Six moved, on purpose**, for three changes of 29 September 2026 in
/// the tree together: task 137's follow-up (a machine has one health that
/// every hit comes off), the map rework (a jump is a day, a trip within a
/// system nothing) and the bots' deaths (a bot lost costs nothing) — the
/// six commands that fight machines. Their shares were not taken apart;
/// the empty-handed crewmate's pistol moves none of them (checked with
/// that rule switched off). They were `0x_e1f2_77b7_3739_64aa`,
/// `0x_7840_b21d_4488_7899`, `0x_6f28_4ad3_5486_cfbe`,
/// `0x_5379_eedc_d57e_9612`, `0x_bb40_ce94_1cfa_873c` and
/// `0x_0d7f_2c9a_94c7_19e8`.
///
/// **`jammer` moved, on purpose**: the machines grow a step every five
/// days of the world clock (`data::ENEMIES_HOURS`), not every three
/// weeks, and the `jammer` probe's clock is past the first five. With
/// the three weeks put back it read its old number again. It was
/// `0x_a253_e753_3a70_d468`.
const PINNED: [(&str, u64); 11] = [
    ("simulation", 0x_73b2_eae7_e74e_9684),
    ("game", 0x_2ba5_0425_4c88_c171),
    ("droids", 0x_6bc6_e563_4740_f31b),
    ("tier2_test", 0x_7f3f_61cd_1727_d600),
    ("combat_droids_medic", 0x_6900_5ef7_e616_5287),
    ("test", 0x_b8c7_dab8_5450_1537),
    ("test_planet", 0x_9e0b_4049_7a5b_e561),
    ("droids_planet", 0x_01a1_86b4_7407_4d8a),
    ("defense", 0x_0cab_dd44_d055_bfbc),
    ("crisis", 0x_c257_27a9_1785_d070),
    ("jammer", 0x_390f_0810_a0f4_7aae),
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

/// `droids_deck` moved once, on purpose, for the minigun and the rail lance
/// (task 115): the combat crew deals them, so four of its sixteen hold a
/// minigun or a lance, drawn as such, and the fight six hundred steps on
/// is another fight. It was `0x_549f_77b7_8357_3bf8`, and came back with
/// the deal cut back to the first five kinds.
///
/// `simulation_deck` moved once, on purpose, for task 111 (every site an
/// attack, a defence or a trader): the simulation's spawn is a defence from
/// the first step, so three hundred steps in the crew stand ashore and the
/// station's deck has its armed defenders on it — another picture of the
/// same drawing. Nothing is drawn another way: with the sites quiet
/// (`World::set_quiet_sites_for_probe`) the old number,
/// `0x_234f_3e03_869d_4c31`, came back bit for bit. `droids_deck` did not
/// move — the arena is the machines', an attack.
///
/// Both decks moved once more, on purpose, for worldgen's
/// `GENERATOR_VERSION` 8: another galaxy, so the simulation's spawn is
/// another station and the arena is rebuilt where another spawn stood —
/// another place, drawn by the same drawing (the designer's two pictures,
/// which no galaxy reaches, did not move). They were
/// `0x_189b_b6fc_1f93_b38b` and `0x_c4ed_495a_3d48_134b`.
///
/// Both decks moved again for task 120's health rework: the playtest ship
/// has no drug lab (a part off the hull), and the fight on `droids` is
/// fought on one bar of hit points, with the blood on the deck only where
/// a hit took some and a body under twenty drips — another fight, drawn by
/// the same drawing. The designer's two pictures did not move. They were
/// `0x_3247_8062_1db9_2394` and `0x_e514_6179_ca55_7e04`.
///
/// Both decks moved again for the same five changes `PINNED`'s last note
/// names (no wander and the player's Bim moved by its player alone, both
/// tasks 131, task 130 and task 132), tested and re-pinned as one tree:
/// the crew stand elsewhere, the stations are other buildings and the
/// fight is another fight — the same drawing. The designer's two
/// pictures did not move. They were `0x_6b2e_903e_8707_0b74` and
/// `0x_51f1_540c_3428_328a`.
///
/// `simulation_deck` moved alone for the first mission's eased waves
/// (`PINNED`'s last note): a machine fewer on the deck. It was
/// `0x_dbcf_396c_2e7e_20a2`.
const PICTURES: [(&str, u64); 4] = [
    ("designer_playtest", 0x_9b08_8f44_06ad_4414),
    ("designer_combat", 0x_157a_1c34_2e97_18e0),
    ("simulation_deck", 0x_68e5_dd88_f023_77e4),
    ("droids_deck", 0x_df15_3380_ad75_d243),
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
/// And again under task 111, which moved every one on every layout for
/// the reason `PINNED`'s note gives, and the same eight are its own.
/// And again under worldgen's `GENERATOR_VERSION` 8 (another galaxy on
/// every layout), the same eight `PINNED`'s own again; the three towns
/// were `0x_a7cb_07a5_991b_0776`, `0x_8841_e762_737f_2e95` and
/// `0x_6c59_a77a_0351_e662`.
/// And again for tasks 135, 136 and 133 (`PINNED`'s last note), the same
/// eight `PINNED`'s own; the three towns were `0x_a68e_2287_bf07_7dbd`,
/// `0x_6401_fed0_37d3_fa63` and `0x_6619_454b_19a3_2754`.
/// And again for the three changes of 29 September 2026 in `PINNED`'s last
/// note, the same eight `PINNED`'s own; the two towns that moved were
/// `0x_f28b_ae2e_c456_dfa2` and `0x_eb85_850a_64dc_1c74`.
const PINNED_BEFORE_112: [(&str, u64); 11] = [
    ("simulation", 0x_73b2_eae7_e74e_9684),
    ("game", 0x_2ba5_0425_4c88_c171),
    ("droids", 0x_6bc6_e563_4740_f31b),
    ("tier2_test", 0x_7f3f_61cd_1727_d600),
    ("combat_droids_medic", 0x_6900_5ef7_e616_5287),
    ("test", 0x_b8c7_dab8_5450_1537),
    ("test_planet", 0x_dc0b_b3bb_316f_2cb2),
    ("droids_planet", 0x_ecce_0859_25c9_4ece),
    ("defense", 0x_8ff4_9ad5_ae66_5b74),
    ("crisis", 0x_c257_27a9_1785_d070),
    ("jammer", 0x_a253_e753_3a70_d468),
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
