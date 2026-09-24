//! Bims, on a desk.
//!
//! The one binary. It opens a window, and every frame it asks whichever
//! screen is up to step its simulation, lay out its panels and paint its
//! shapes; the world's shapes go into Bevy meshes under a bloom
//! (`scene.rs`) and the panels and the words are egui.
//! Nothing in here decides anything about the game — the room, the rules,
//! the world and the galaxy are the crates beside this one, and this crate
//! is the window, the pointer and the words.
//!
//! Eighteen things to run, and each is a name rather than a flag:
//!
//! ```text
//! bims               the whole game in order — menu, setup or lobby, world
//!                    and station, then the run: docked where you said on
//!                    the default ship, five thousand a Bim in the pool
//! bims simulation    straight into the world on the playtest ship
//! bims design        straight into the yard, the playtest ship given, docked
//!                    where the simulation docks
//! bims test          the simulation somewhere else each time — docked at a
//!                    random station somebody lives on, in a random galaxy
//! bims droids        the fight: the combat ship — sixteen crew, a gun in
//!                    every hand — docked at the arena, which the machines
//!                    hold, a wave of droids about it and the next a
//!                    minute behind (every enemy is a machine since
//!                    feature 102, so `combat` is gone: it was this)
//! bims combat_droids_<class>
//!                    that fight with a class on the crew member you steer:
//!                    `combat_droids_engineer` … `combat_droids_commander`
//!                    — one a class, at the tenth level of it with every
//!                    talent still to choose; `BIMS_LEVEL` says otherwise
//! bims tier2_test    `droids` with everybody's kit at tier two: every
//!                    crew member's gun and a full set of armour, and the
//!                    machines at it too
//! bims tier3_test    the same at tier three
//! bims crisis        the simulation a day before the machines' first
//!                    spread, the origin two hyperlane hops off and already
//!                    theirs, so the next stars turn red on the chart while
//!                    you watch
//! bims jammer        the crew in an infested system two hops from the
//!                    machines' origin: the jammer standing, a wave aboard,
//!                    and the lanes inward shut
//! bims defense       a town on a planet with the machines one hop away: the
//!                    crew set down at its pad, a wave landing outside a gate
//!                    a minute later, and the town's own guard fighting beside
//!                    them
//! bims stationbuilder [name]
//!                    a grid to sketch a station's rough shape on, saved as
//!                    text to `stations/<name>.txt` for a plan to be
//!                    written from
//! bims list          every one of them printed, and what it opens, rather
//!                    than a window: `--list`, `--help` and `-h` are it too
//! bims --self-check  the pinned constants, checked, and the verdict printed
//! ```

mod canvas;
mod crew;
mod dev;
mod fogmap;
mod format;
mod grid;
mod icons;
mod keys;
mod names;
mod net;
mod perf;
mod save;
mod scene;
mod screens;
mod settings;
mod shapes;
mod sound;
mod theme;

use bevy::prelude::*;
use bevy_egui::EguiPlugin;

/// Which of the eighteen things this process is.
#[derive(Resource, Clone, Copy, PartialEq, Eq, Debug)]
pub enum Launch {
    Game,
    Simulation,
    Design,
    Test,
    /// `Test` set down on a planet: the same roll, made in a system with
    /// friendly ground, and the ship landed at the settlement.
    TestPlanet,
    /// `Droids` with everybody's guns and armour at one tier, crew and
    /// machines alike: the `tier2_test` and `tier3_test` commands. It was
    /// the human garrison's fight until every enemy was a machine
    /// (feature 102).
    DroidsAtTier(bims::combat::Tier),
    /// The fight (feature 83): the combat ship and its sixteen crew, a
    /// gun in every hand, at an arena the **machines** hold — a wave of
    /// them about it — with `DROID_REINFORCE_STEPS` a minute here, so
    /// the next wave can be watched arriving. Since every enemy is a
    /// machine (feature 102) it is *the* fight: the `combat` command that
    /// turned the arena's people against the crew would have been this
    /// exactly, and is gone.
    Droids,
    /// `Droids` with a class already on the crew member you steer: the
    /// `combat_droids_engineer` … `combat_droids_commander` commands,
    /// one a class (features 79 and 83) — the same ship, the same arena,
    /// the same wave and the same two dials (`BIMS_DROID_TIER`,
    /// `BIMS_DROID_WAVE`), so what a class does against the machines is
    /// the only thing that differs between two of these runs.
    /// `BIMS_CLASS` still wins over it. It opens at
    /// `dev::COMBAT_CLASS_LEVEL` — the tenth, the top of the tree, with
    /// every one of the seven talents still to choose (feature 80) — and
    /// `BIMS_LEVEL` says otherwise.
    DroidsAs(world::Class),
    /// `TestPlanet` with the town droid-held, the same shortcut.
    DroidsPlanet,
    /// `TestPlanet` with the town **threatened** (feature 94): the
    /// machines' origin one hyperlane hop off, so the town is next and a
    /// wave lands outside a gate `DEFENSE_DELAY_STEPS` after the crew
    /// set down. That wait and `DROID_REINFORCE_STEPS` are both a
    /// minute of the mission clock here, so the whole fight is watched rather than waited for.
    Defense,
    /// `Test` a day before the crisis first spreads (feature 92): the
    /// same random galaxy and roll, the crisis's origin forced two
    /// hyperlane hops from the crew's own star — theirs from day nought,
    /// as in every run since feature 102 — and the clock wound on to the
    /// day before the stars next to it turn, so they go red on the galaxy
    /// chart within a day of the clock rather than five.
    /// `BIMS_CRISIS_DAY` moves the day the origin turns, and the rest
    /// with it.
    Crisis,
    /// `Crisis` one step on (feature 93): the crew **in** an infested
    /// system two hyperlane hops from the machines' origin, its jammer
    /// standing and one wave of machines aboard the station they are
    /// tied up at, with the reinforcement clock a minute. What a jam
    /// does to the chart and to a jump is looked at from here.
    Jammer,
    StationBuilder,
}

/// Which screen is up. One at a time, and the whole game is a walk through
/// them in order: the menu, then setup or a lobby, then the game its Start
/// opens (feature 102: the designer is the `design` command's alone now).
/// The simulation and the fights start further along.
#[derive(States, Clone, Copy, PartialEq, Eq, Hash, Debug, Default)]
pub enum Screen {
    #[default]
    Menu,
    Setup,
    Lobby,
    /// Nowhere to start: the lobby named no station, or one the galaxy has
    /// not got. A screen that says so and a way back, never a different
    /// dock.
    Lost,
    Design,
    Game,
    /// The run is over: nobody of the crew standing. A screen that says
    /// so and a way back to the menu (`screens::game::over`).
    Over,
    StationBuilder,
}

fn usage() -> ! {
    eprintln!(
        "usage: bims [game|simulation|design|test|test_planet|droids|combat_droids_<class>|tier2_test|tier3_test|droids_planet|crisis|jammer|defense|stationbuilder [name]|list|--self-check]"
    );
    eprintln!("       a class is one of: {}", class_words().join(", "));
    eprintln!("       `bims list` says what each of them opens");
    std::process::exit(2)
}

/// Every command but the `combat_droids_<class>` ones, and what it opens
/// — the one list, printed by [`list`] and nothing else. A new command is
/// a row here and an arm in `main`; the classes' commands are not written
/// out, since [`class_words`] reads them off `Class::ALL`.
const COMMANDS: [(&str, &str); 15] = [
    (
        "game",
        "The whole game in order: menu, setup or lobby, world and station, then the run: a mission where you docked, on the default ship, 5 000 a Bim in the pool",
    ),
    ("simulation", "Straight into the world on the playtest ship"),
    (
        "design",
        "Straight into the yard, the playtest ship given, docked where the simulation docks",
    ),
    (
        "test",
        "The simulation somewhere else each time: a random galaxy, docked at a station somebody lives on, a mercenary for hire at the dock",
    ),
    (
        "test_planet",
        "That set down on a planet: the pad, the ground and the settlement beside it",
    ),
    (
        "droids",
        "The fight: the combat ship's sixteen crew, a gun in every hand, at an arena the machines hold, reinforcements a minute apart",
    ),
    (
        "tier2_test",
        "The fight with everybody's guns and armour at tier two, and the machines at it too",
    ),
    ("tier3_test", "The same at tier three"),
    (
        "droids_planet",
        "That on a planet: a town held by the machines, the ship set down at its pad",
    ),
    (
        "crisis",
        "The simulation a day before the crisis next spreads, its origin two hyperlane hops off: travel a day on the world map and the next stars are red",
    ),
    (
        "jammer",
        "The crew in an infested system two hops from the machines' origin: the jammer standing, a wave aboard, the lanes inward shut",
    ),
    (
        "defense",
        "A town on a planet with the machines one hop away: the crew set down at its pad, and a wave landing outside a gate a minute later",
    ),
    (
        "stationbuilder [name]",
        "The station builder: a grid to sketch a station's rough shape on, saved as text to stations/<name>.txt",
    ),
    (
        "list",
        "This list, rather than a window: `--list`, `--help` and `-h` are it too",
    ),
    (
        "--self-check",
        "The pinned constants, checked, and the verdict printed; non-zero if the build disagrees with them",
    ),
];

/// `bims list`: every launch option there is, and what it opens. The
/// classes' fights are made from `Class::ALL` rather than written down,
/// so a class added later is listed the day it exists.
fn list() {
    println!("bims <what>, and each of these is a what:\n");
    println!(
        "Every one of them bar the yard and the station builder is a run (feature 103):\na mission at the site it opens at, Back to ship at the bottom right, and the world map\nbetween missions, where a trip is chosen, accepted by every player and resolved in days.\n"
    );
    // Wide enough for `combat_droids_commander`, the longest of them,
    // and a space after it.
    let row = |name: &str, what: &str| println!("  {name:<26}{what}");
    for (name, what) in COMMANDS {
        row(name, what);
        // The classes' commands go under the fight they each are.
        let (prefix, fight) = match name {
            "droids" => (DROIDS_AS, "That same fight"),
            _ => continue,
        };
        for class in world::Class::ALL {
            if class == world::Class::None {
                continue;
            }
            let word = names::class_name(class).to_ascii_lowercase();
            let a = if word.starts_with(['a', 'e', 'i', 'o', 'u']) {
                "an"
            } else {
                "a"
            };
            row(
                &format!("{prefix}{word}"),
                &format!(
                    "{fight}, the crew member you steer {a} {word} at the tenth level (BIMS_LEVEL says otherwise)"
                ),
            );
        }
    }
    println!(
        "\nNothing named is `game`. The environment says the rest: BIMS_CLASS, BIMS_LEVEL,\nBIMS_DROID_TIER and the others are in crates/app/src/dev.rs."
    );
}

/// What comes before a class's name in a `combat_droids_<class>`
/// command: the machines' fight with that class in hand. The
/// `combat_<class>` commands that stood beside them were the human
/// garrison's fight, and went with it (feature 102).
const DROIDS_AS: &str = "combat_droids_";

/// The class a `combat_droids_<class>` command names, by the name
/// `names.rs` gives it in any case — `None` for a word no class is called.
/// The classless `Class::None` is not one of them: there would be nothing
/// to tell the run apart from plain `droids`.
fn class_named(word: &str) -> Option<world::Class> {
    world::Class::ALL
        .into_iter()
        .filter(|class| *class != world::Class::None)
        .find(|class| names::class_name(*class).eq_ignore_ascii_case(word))
}

/// Every class a `combat_droids_<class>` command takes, in the words it takes
/// them in — read off `Class::ALL` rather than written out, so a class
/// added later is offered by the usage line as well as accepted.
fn class_words() -> Vec<String> {
    world::Class::ALL
        .into_iter()
        .filter(|class| *class != world::Class::None)
        .map(|class| names::class_name(class).to_ascii_lowercase())
        .collect()
}

fn main() {
    let launch = match std::env::args().nth(1).as_deref() {
        None | Some("game") => Launch::Game,
        Some("simulation") => Launch::Simulation,
        Some("design") => Launch::Design,
        Some("test") => Launch::Test,
        Some("test_planet") => Launch::TestPlanet,
        // `combat_droids_medic` and the rest: the machines' fight, that
        // class in hand.
        Some(word) if word.starts_with(DROIDS_AS) => match class_named(&word[DROIDS_AS.len()..]) {
            Some(class) => Launch::DroidsAs(class),
            None => usage(),
        },
        Some("droids") => Launch::Droids,
        Some("droids_planet") => Launch::DroidsPlanet,
        Some("tier2_test") => Launch::DroidsAtTier(bims::combat::Tier::Two),
        Some("tier3_test") => Launch::DroidsAtTier(bims::combat::Tier::Three),
        Some("crisis") => Launch::Crisis,
        Some("jammer") => Launch::Jammer,
        Some("defense") => Launch::Defense,
        Some("stationbuilder") => Launch::StationBuilder,
        // What there is to run, printed rather than opened.
        Some("list") | Some("--list") | Some("--help") | Some("-h") => {
            list();
            std::process::exit(0);
        }
        Some("--self-check") => {
            let bits = ship::session::self_check();
            let all = ship::session::SELF_CHECK_ALL;
            println!("self check: {bits:#010b} of {all:#010b}");
            std::process::exit(if bits == all { 0 } else { 1 });
        }
        Some(_) => usage(),
    };
    // The sketch's name, after `stationbuilder`: letters, digits, `-` and
    // `_`, so it names a file and not a path.
    let sketch = match (launch, std::env::args().nth(2)) {
        (Launch::StationBuilder, Some(name)) => {
            if !screens::station::valid_name(&name) {
                eprintln!("a sketch's name is letters, digits, '-' and '_': {name:?}");
                std::process::exit(2);
            }
            name
        }
        _ => screens::station::DEFAULT_NAME.to_string(),
    };

    let mut app = App::new();
    app.add_plugins(DefaultPlugins.set(WindowPlugin {
        primary_window: Some(Window {
            title: "Bims".into(),
            name: Some(dev::window_name()),
            present_mode: dev::present_mode(),
            resolution: dev::window_resolution(),
            mode: dev::window_mode(),
            // The panels want their room: the designer's palette and
            // checks, the game's controls, and the floating panels over
            // the deck between them.
            resize_constraints: bevy::window::WindowResizeConstraints {
                min_width: 1100.0,
                min_height: 700.0,
                ..default()
            },
            visible: dev::smoke_frames().is_none(),
            ..default()
        }),
        ..default()
    }))
    .add_plugins(EguiPlugin::default())
    .insert_resource(ClearColor(theme::VOID))
    .insert_resource(launch)
    .insert_resource(screens::station::SketchName(sketch))
    .insert_resource(keys::Keys::load())
    .init_resource::<net::Online>()
    .init_state::<Screen>()
    .add_plugins((
        scene::ScenePlugin,
        dev::DevPlugin,
        sound::SoundPlugin,
        theme::ThemePlugin,
        screens::builder::BuilderPlugin,
        screens::designer::DesignerPlugin,
        screens::game::GamePlugin,
        screens::station::StationBuilderPlugin,
    ))
    .add_systems(Startup, open);
    app.run();
}

/// Where to start, by what was asked for. The menu is the default state;
/// the others skip straight to their screen. The yard is handed what the
/// setup screen would have handed it: the defaults, and the simulation's
/// dock for a spawn.
fn open(launch: Res<Launch>, mut commands: Commands, mut next: ResMut<NextState<Screen>>) {
    match *launch {
        Launch::Game => {}
        Launch::Simulation
        | Launch::Test
        | Launch::TestPlanet
        | Launch::DroidsAtTier(_)
        | Launch::Droids
        | Launch::DroidsAs(_)
        | Launch::DroidsPlanet
        | Launch::Crisis
        | Launch::Jammer
        | Launch::Defense => next.set(Screen::Game),
        Launch::Design => {
            let mut settings = screens::builder::Settings::default();
            settings.seed = world::data::DEFAULT_SEED;
            settings.spawn = ship::session::pick_dock(settings.seed, settings.galaxy, 0);
            commands.insert_resource(screens::designer::Start(settings));
            next.set(Screen::Design);
        }
        Launch::StationBuilder => next.set(Screen::StationBuilder),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every class but the classless one has a `combat_droids_<class>`
    /// command, spelled as the usage line offers it, and a word no class
    /// is called is nobody's command — a new class is a new command with
    /// nothing written here, since both sides read `Class::ALL`.
    #[test]
    fn every_class_has_a_fight_of_its_own() {
        let words = class_words();
        assert_eq!(words.len(), world::Class::ALL.len() - 1);
        for (class, word) in world::Class::ALL
            .into_iter()
            .filter(|class| *class != world::Class::None)
            .zip(&words)
        {
            assert_eq!(class_named(word), Some(class));
            // The case a shell happens to type it in is not the point.
            assert_eq!(class_named(&word.to_ascii_uppercase()), Some(class));
            let command = format!("{DROIDS_AS}{word}");
            assert_eq!(class_named(&command[DROIDS_AS.len()..]), Some(class));
        }
        // `list` hangs the classes' rows under `droids`', so the row it
        // hangs them under has to be there.
        assert!(COMMANDS.iter().any(|(name, _)| *name == "droids"));
        assert_eq!(class_named("none"), None);
        assert_eq!(class_named("cook"), None);
        assert_eq!(class_named(""), None);
        assert_eq!(class_named("droids_cook"), None);
    }

    /// Every enemy is a machine (feature 102): the human garrison's
    /// fight and the raid are no commands any more, and `combat` would
    /// have been `droids` exactly. The behaviour test room went with the
    /// needs it was there to watch (feature 104).
    #[test]
    fn the_human_fights_are_no_commands_any_more() {
        for gone in ["combat", "raid", "room"] {
            assert!(
                !COMMANDS.iter().any(|(name, _)| *name == gone),
                "{gone} is still listed"
            );
        }
    }
}
