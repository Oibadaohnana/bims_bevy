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
//! Twenty-six things to run, and each is a name rather than a flag:
//!
//! ```text
//! bims               the whole game in order — menu, setup or lobby, world
//!                    and station, ship design, then the world docked where
//!                    you said
//! bims simulation    straight into the world on the playtest ship
//! bims design        straight into the yard, the playtest ship given, docked
//!                    where the simulation docks
//! bims room          the behaviour test room — Bims on a deck
//! bims test          the simulation somewhere else each time — docked at a
//!                    random station somebody lives on, in a random galaxy
//! bims combat        the fight: the combat ship — fourteen crew, a gun in
//!                    every hand — docked at the spawn rebuilt as the arena
//!                    and made hostile, its people enemies, fifteen of
//!                    them; a recruited crew member shoots at any it can
//!                    see
//! bims combat_<class>
//!                    that fight with a class on the crew member you steer:
//!                    `combat_engineer`, `combat_soldier`, `combat_medic`,
//!                    `combat_tank`, `combat_commander` — one a class, at
//!                    the tenth level of it with every talent still to
//!                    choose; `BIMS_LEVEL` says otherwise
//! bims combat_droids_<class>
//!                    the machines' fight with that class in hand:
//!                    `combat_droids_engineer` … `combat_droids_commander`,
//!                    `droids` as `combat_<class>` is `combat`
//! bims tier2_test    `combat` with everybody's kit at tier two: every gun and
//!                    a full set of armour, crew and garrison alike
//! bims tier3_test    the same at tier three
//! bims raid          the simulation off its berth, holding in open space,
//!                    with a raid on its way: contact ten seconds in, the
//!                    raider then closing at its own pace
//! bims crisis        the simulation a day before the machines appear, with
//!                    the origin two hyperlane hops off, so the first star
//!                    turns red on the chart while you watch
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

/// Which of the twenty-six things this process is.
#[derive(Resource, Clone, Copy, PartialEq, Eq, Debug)]
pub enum Launch {
    Game,
    Simulation,
    Design,
    Room,
    Test,
    /// `Test` set down on a planet: the same roll, made in a system with
    /// friendly ground, and the ship landed at the settlement.
    TestPlanet,
    Combat,
    /// `Combat` with a class already on the crew member you steer: the
    /// `combat_engineer`, `combat_soldier`, `combat_medic`, `combat_tank`
    /// and `combat_commander` commands, one a class (feature 79). The
    /// fight is `Combat`'s own — the same ship, the same arena, the same
    /// garrison — so what a class does in it is the only thing that
    /// differs between two of these runs. `BIMS_CLASS` still wins over it.
    /// It opens at `dev::COMBAT_CLASS_LEVEL` — the tenth, the top of the
    /// tree, with every one of the seven talents still to choose
    /// (feature 80) — and `BIMS_LEVEL` says otherwise.
    CombatAs(world::Class),
    /// `Combat` with everybody's guns and armour at one tier, both
    /// sides: the `tier2_test` and `tier3_test` commands.
    CombatAtTier(bims::combat::Tier),
    /// `Combat` with the arena **droid-held** instead of garrisoned
    /// (feature 83): the same ship and the same crew, and a wave of
    /// machines about the arena instead of its people.
    /// `DROID_REINFORCE_MINUTES` is a minute here, so the next wave can
    /// be watched arriving.
    Droids,
    /// `Droids` with a class already on the crew member you steer: the
    /// `combat_droids_engineer` … `combat_droids_commander` commands,
    /// one a class. It is to `Droids` exactly what [`Launch::CombatAs`]
    /// is to `Combat` — the same ship, the same arena, the same wave and
    /// the same two dials (`BIMS_DROID_TIER`, `BIMS_DROID_WAVE`) — so
    /// what a class does **against the machines** is the only thing that
    /// differs between two of these runs. The level and the engineer's
    /// sentry kits are `CombatAs`'s as well, since both go through
    /// `dev::class_crew`.
    DroidsAs(world::Class),
    /// `TestPlanet` with the town droid-held, the same shortcut.
    DroidsPlanet,
    /// `TestPlanet` with the town **threatened** (feature 94): the
    /// machines' origin one hyperlane hop off, so the town is next and a
    /// wave lands outside a gate `DEFENSE_DELAY_MINUTES` after the crew
    /// set down. That wait and `DROID_REINFORCE_MINUTES` are both a
    /// minute here, so the whole fight is watched rather than waited for.
    Defense,
    /// The simulation with a raid on its way: off the berth and holding,
    /// contact ten seconds in.
    Raid,
    /// `Test` a day before the machines appear (feature 92): the same
    /// random galaxy and roll, the clock wound on to the day before
    /// `DROID_FIRST_DAY` and the crisis's origin forced two hyperlane
    /// hops from the crew's own star, so the first star turns red on the
    /// galaxy chart within a day of the clock rather than ten.
    /// `BIMS_CRISIS_DAY` moves the day the first one turns; the clock
    /// opens a day short of whatever it says.
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
/// them in order: the menu, then setup or a lobby, then the designer, then
/// the game the last Accept opens. The room and the simulation start
/// further along.
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
    Room,
    StationBuilder,
}

fn usage() -> ! {
    eprintln!(
        "usage: bims [game|simulation|design|room|test|test_planet|combat|combat_<class>|tier2_test|tier3_test|droids|combat_droids_<class>|droids_planet|raid|crisis|jammer|defense|stationbuilder [name]|list|--self-check]"
    );
    eprintln!("       a class is one of: {}", class_words().join(", "));
    eprintln!("       `bims list` says what each of them opens");
    std::process::exit(2)
}

/// Every command but the `combat_<class>` and `combat_droids_<class>`
/// ones, and what it opens — the one list, printed by [`list`] and
/// nothing else. A new command is a row here and an arm in `main`; the
/// classes' commands are not written out, since [`class_words`] reads
/// them off `Class::ALL`.
const COMMANDS: [(&str, &str); 18] = [
    (
        "game",
        "The whole game in order: menu, setup or lobby, world and station, ship design, then the world docked where you said",
    ),
    ("simulation", "Straight into the world on the playtest ship"),
    (
        "design",
        "Straight into the yard, the playtest ship given, docked where the simulation docks",
    ),
    ("room", "The behaviour test room: Bims on a deck"),
    (
        "test",
        "The simulation somewhere else each time: a random galaxy, docked at a station somebody lives on, a mercenary for hire at the dock",
    ),
    (
        "test_planet",
        "That set down on a planet: the pad, the ground and the settlement beside it",
    ),
    (
        "combat",
        "The fight: the combat ship's fourteen crew, a gun in every hand, docked at the arena and its fifteen people turned against them",
    ),
    (
        "tier2_test",
        "The fight with everybody's guns and armour at tier two, crew and garrison alike",
    ),
    ("tier3_test", "The same at tier three"),
    (
        "droids",
        "The fight with the arena held by the machines: a wave of droids instead of its people, reinforcements a minute apart",
    ),
    (
        "droids_planet",
        "That on a planet: a town held by the machines, the ship set down at its pad",
    ),
    (
        "raid",
        "The simulation off its berth, holding in open space, with a raid on its way: contact ten seconds in",
    ),
    (
        "crisis",
        "The simulation a day before the machines appear, the origin two hyperlane hops off: the first star turns red on the chart while you watch",
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
/// classes' fights — both sets, the garrison's and the machines' — are
/// made from `Class::ALL` rather than written down, so a class added
/// later is listed the day it exists.
fn list() {
    println!("bims <what>, and each of these is a what:\n");
    // Wide enough for `combat_droids_commander`, the longest of them,
    // and a space after it.
    let row = |name: &str, what: &str| println!("  {name:<26}{what}");
    for (name, what) in COMMANDS {
        row(name, what);
        // The classes' commands go under the fight they each are: the
        // garrison's under `combat`, the machines' under `droids`.
        let (prefix, fight) = match name {
            "combat" => (COMBAT_AS, "That same fight"),
            "droids" => (DROIDS_AS, "That same wave"),
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
        "\nNothing named is `game`. The environment says the rest: BIMS_CLASS, BIMS_LEVEL,\nBIMS_FIGHT, BIMS_RAID and the others are in crates/app/src/dev.rs."
    );
}

/// What comes before a class's name in a `combat_<class>` command.
const COMBAT_AS: &str = "combat_";

/// What comes before a class's name in a `combat_droids_<class>`
/// command: the machines' fight with that class in hand. It starts with
/// [`COMBAT_AS`], so it has to be tried **first** — `combat_droids_medic`
/// read as a `combat_<class>` would be a class called `droids_medic`,
/// which is nobody, and the run would be refused rather than opened.
const DROIDS_AS: &str = "combat_droids_";

/// The class a `combat_<class>` command names, by the name `names.rs`
/// gives it in any case — `None` for a word no class is called. The
/// classless `Class::None` is not one of them: there would be nothing to
/// tell the run apart from plain `combat`.
fn class_named(word: &str) -> Option<world::Class> {
    world::Class::ALL
        .into_iter()
        .filter(|class| *class != world::Class::None)
        .find(|class| names::class_name(*class).eq_ignore_ascii_case(word))
}

/// Every class a `combat_<class>` command takes, in the words it takes
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
        Some("room") => Launch::Room,
        Some("test") => Launch::Test,
        Some("test_planet") => Launch::TestPlanet,
        // Accepted as a flag too, since that is how it was first asked for.
        Some("combat") | Some("--combat") => Launch::Combat,
        // `combat_droids_medic` and the rest: the machines' fight, that
        // class in hand. Before `combat_<class>`, which its name starts
        // with.
        Some(word) if word.starts_with(DROIDS_AS) => match class_named(&word[DROIDS_AS.len()..]) {
            Some(class) => Launch::DroidsAs(class),
            None => usage(),
        },
        // `combat_medic` and the rest: the same fight, that class in hand.
        Some(word) if word.strip_prefix(COMBAT_AS).is_some() => {
            match class_named(&word[COMBAT_AS.len()..]) {
                Some(class) => Launch::CombatAs(class),
                None => usage(),
            }
        }
        Some("droids") => Launch::Droids,
        Some("droids_planet") => Launch::DroidsPlanet,
        Some("tier2_test") => Launch::CombatAtTier(bims::combat::Tier::Two),
        Some("tier3_test") => Launch::CombatAtTier(bims::combat::Tier::Three),
        Some("raid") => Launch::Raid,
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
        screens::room::RoomPlugin,
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
        | Launch::Combat
        | Launch::CombatAs(_)
        | Launch::CombatAtTier(_)
        | Launch::Droids
        | Launch::DroidsAs(_)
        | Launch::DroidsPlanet
        | Launch::Raid
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
        Launch::Room => next.set(Screen::Room),
        Launch::StationBuilder => next.set(Screen::StationBuilder),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every class but the classless one has a `combat_<class>` command,
    /// spelled as the usage line offers it, and a word no class is called
    /// is nobody's command — a new class is a new command with nothing
    /// written here, since both sides read `Class::ALL`.
    #[test]
    fn every_class_has_a_combat_command_of_its_own() {
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
        }
        // `list` hangs the classes' rows under `combat`'s and under
        // `droids`', so both rows it hangs them under have to be there.
        assert!(COMMANDS.iter().any(|(name, _)| *name == "combat"));
        assert!(COMMANDS.iter().any(|(name, _)| *name == "droids"));
        assert_eq!(class_named("none"), None);
        assert_eq!(class_named("cook"), None);
        assert_eq!(class_named(""), None);
    }

    /// The machines' fight takes the same five classes, and the trap in
    /// it is that `combat_droids_` starts with `combat_`: read as a
    /// `combat_<class>` the word names no class at all, so `main` has to
    /// try the longer prefix first or every one of these runs is refused.
    #[test]
    fn the_machines_fight_takes_a_class_too() {
        assert!(DROIDS_AS.starts_with(COMBAT_AS));
        for word in class_words() {
            let command = format!("{DROIDS_AS}{word}");
            assert_eq!(
                class_named(&command[DROIDS_AS.len()..]),
                class_named(&word),
                "{command} names its class",
            );
            // And what the shorter prefix would have made of it: nobody.
            assert_eq!(class_named(&command[COMBAT_AS.len()..]), None);
        }
        assert_eq!(class_named("droids_cook"), None);
    }
}
