//! Bims, on a desk.
//!
//! The one binary. It opens a window, and every frame it asks whichever
//! screen is up to step its simulation, lay out its panels and paint its
//! shapes; the shapes go into one mesh (`canvas.rs`) and the panels are egui.
//! Nothing in here decides anything about the game — the room, the rules,
//! the world and the galaxy are the crates beside this one, and this crate
//! is the window, the pointer and the words.
//!
//! Eleven things to run, and each is a name rather than a flag:
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
//! bims tier2_test    `combat` with everybody's kit at tier two: every gun and
//!                    a full set of armour, crew and garrison alike
//! bims tier3_test    the same at tier three
//! bims raid          the simulation off its berth, holding in open space,
//!                    with a raid on its way: contact ten seconds in, the
//!                    raider then closing at its own pace
//! bims stationbuilder [name]
//!                    a grid to sketch a station's rough shape on, saved as
//!                    text to `stations/<name>.txt` for a plan to be
//!                    written from
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
mod save;
mod screens;
mod settings;
mod shapes;
mod sound;
mod theme;

use bevy::prelude::*;
use bevy_egui::EguiPlugin;

/// Which of the eleven things this process is.
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
    /// `Combat` with everybody's guns and armour at one tier, both
    /// sides: the `tier2_test` and `tier3_test` commands.
    CombatAtTier(bims::combat::Tier),
    /// The simulation with a raid on its way: off the berth and holding,
    /// contact ten seconds in.
    Raid,
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
        "usage: bims [game|simulation|design|room|test|test_planet|combat|tier2_test|tier3_test|raid|stationbuilder [name]|--self-check]"
    );
    std::process::exit(2)
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
        Some("tier2_test") => Launch::CombatAtTier(bims::combat::Tier::Two),
        Some("tier3_test") => Launch::CombatAtTier(bims::combat::Tier::Three),
        Some("raid") => Launch::Raid,
        Some("stationbuilder") => Launch::StationBuilder,
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
        canvas::CanvasPlugin,
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
        | Launch::CombatAtTier(_)
        | Launch::Raid => next.set(Screen::Game),
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
