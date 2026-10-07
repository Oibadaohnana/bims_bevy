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
//! Twenty-one things to run, and each is a name rather than a flag:
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
//!                    — one a class, at the top (twentieth) level with every
//!                    skill point still to spend; `BIMS_LEVEL` says
//!                    otherwise, `BIMS_RANKS=q,c,e,r` buys ranks
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
//! bims guardian      the fight at tier three against a Guardian and two
//!                    Troopers a wave: the shield, the turn and the beam
//! bims bomber        the fight at tier two against a Bomber and two
//!                    Troopers a wave (also `lancer` and `conductor`)
//! bims relics        the droids arena with one short wave: clear it, go
//!                    back to the ship, and the reward screen offers relics
//! bims heart         the Machine Heart: the crew docked at its fortress at
//!                    the machines' origin, everybody in tier-three kit;
//!                    `BIMS_HEART_PHASE=2` or `3` opens the fight past its
//!                    seal or in its overload
//! bims end           the Machine Heart with company: a lobby at code THEEND,
//!                    Start pressed once a second player joins, and the
//!                    ready check at the fortress with ten bots — two of
//!                    every class at the top level, every rank bought —
//!                    everybody in tier-three kit, the waves the lobby's
//!                    difficulty on the Heart's day 32 (`BIMS_END_DAY`), the first
//!                    mission not eased; `bims end offline` is that alone,
//!                    from the game setup, with no relay
//! bims manufacturers the combat crew at the nearest site of the
//!                    Manufacturers' on day `BIMS_MANUFACTURER_DAY` (eight
//!                    unless it says: their people with Troopers beside them)
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

mod ability_icons;
mod canvas;
mod crew;
mod dev;
mod fogmap;
mod format;
mod healthbars;
mod icons;
mod keys;
mod lightmap;
mod names;
mod net;
mod offscreen;
mod particles;
mod perf;
mod playout;
mod profile;
mod rollback;
mod save;
mod scene;
mod screens;
mod settings;
mod shapes;
mod sound;
mod surfaces;
mod theme;
mod wavecfg;

use bevy::prelude::*;
use bevy_egui::EguiPlugin;

/// Which of the twenty-one things this process is.
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
    /// `dev::combat_class_level` — the twentieth, with sixteen skill
    /// points to spend (every class a ranked kit since task 139) — and
    /// `BIMS_LEVEL` says otherwise; `BIMS_RANKS=q,c,e,r` sets the ranks.
    DroidsAs(world::Class),
    /// A mission the map shapes, for a playtest (October 2026): the
    /// `breaches`, `sabotage`, `evacuation` and `nests` commands
    /// (`Session::mission`).
    Mission(world::run::Mission),
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
    /// The Guardian looked at (feature 100): the `droids` fight at tier
    /// three with every wave one Guardian and two Troopers, and the
    /// reinforcement clock a minute.
    Guardian,
    /// A tier-two machine looked at (task 157): the `droids` fight at tier
    /// two with every wave that machine and two Troopers — a Husk beside
    /// them for the Conductor, so it has somebody to link — the `bomber`,
    /// `lancer` and `conductor` commands.
    TierTwo(bims::droid::DroidKind),
    /// The relics looked at (feature 106): the `droids` arena with one
    /// wave short enough to finish, so clearing it and going back to the
    /// ship opens the reward screen.
    Relics,
    /// The Machine Heart looked at (feature 108): the crew docked at its
    /// fortress at the machines' origin, everybody in tier-three kit.
    /// `BIMS_HEART_PHASE` opens the fight in its second or third phase.
    Heart,
    /// A site of the Manufacturers' looked at (feature 109): the combat
    /// ship's crew at the nearest site of theirs on the day
    /// `BIMS_MANUFACTURER_DAY` says (eight unless it does).
    Manufacturers,
    /// The end fight with company: the `game` run's lobby opened at the
    /// menu on its own at `dev::END_CODE` for others to join, Start pressed
    /// once `dev::END_PLAYERS` are in, and the run opened at the Machine
    /// Heart's fortress with `dev::END_BOTS` bots aboard — plain
    /// classless Bims — everybody in
    /// tier-three kit and the waves the lobby's difficulty (the
    /// scaling file unless the host moved it). `builder::Settings::end`.
    End,
    /// `End` with nobody else and no relay (`end offline`): the solo game
    /// setup opened with a start picked, for a class and a Start, and the
    /// same run — the Machine Heart, the ten bots, the day, the waves
    /// uneased — for one player.
    EndOffline,
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
        "usage: bims [game|simulation|design|test|test_planet|droids|combat_droids_<class>|tier2_test|tier3_test|droids_planet|crisis|jammer|defense|guardian|relics|heart|manufacturers|end [offline]|stationbuilder [name]|list|--self-check]"
    );
    eprintln!("       a class is one of: {}", class_words().join(", "));
    eprintln!("       `bims list` says what each of them opens");
    std::process::exit(2)
}

/// Every command but the `combat_droids_<class>` ones, and what it opens
/// — the one list, printed by [`list`] and nothing else. A new command is
/// a row here and an arm in `main`; the classes' commands are not written
/// out, since [`class_words`] reads them off `Class::ALL`.
const COMMANDS: [(&str, &str); 36] = [
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
        "The simulation somewhere else each time: a random galaxy, docked at a station somebody lives on; BIMS_STATION_SEED (and BIMS_STATION_KIND) rebuilds the dock as that seed generates it",
    ),
    (
        "test_planet",
        "That set down on a planet: the pad, the ground and the settlement beside it; BIMS_STATION_SEED draws the town from that seed",
    ),
    (
        "droids",
        "The fight: the combat ship's sixteen crew, a gun in every hand, at an arena the machines hold, reinforcements a minute apart; BIMS_STATION_SEED fights it in a generated station instead",
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
        "A town on a planet with the machines one hop away: the crew set down at its pad, and a wave landing outside a gate a minute later; BIMS_STATION_SEED draws the town from that seed",
    ),
    (
        "guardian",
        "The fight at tier three with every wave one Guardian and two Troopers: its shield, its turn and its beam, reinforcements a minute apart",
    ),
    (
        "bomber",
        "The fight at tier two with every wave one Bomber and two Troopers: its rolling bomb, the circle it bursts over and the machines it catches",
    ),
    (
        "lancer",
        "The fight at tier two with every wave one Lancer and two Troopers: its charged rail, the line it follows and the one it locks",
    ),
    (
        "conductor",
        "The fight at tier two with every wave a Conductor, two Troopers and a Husk: its link, its mark, its blink and its strike call",
    ),
    (
        "breaches",
        "Seal the breaches: the combat crew at the spawn's arena under attack, every way in but the port a breach the machines come by every 30 s until all are welded shut (V at one, 15 s, faster with two)",
    ),
    (
        "sabotage",
        "Sabotage: the droids arena held by the machines; plant the charge at the amber mark (V, 12 s), hold it 45 s against the machines making to disarm it, then get to the green way out before it blows",
    ),
    (
        "evacuation",
        "Evacuation: the combat crew at the spawn's arena under attack; take up the flag (V) and lead the site's people and its refugees aboard the ship, put it down to make them hold",
    ),
    (
        "nests",
        "A nest hunt: the droids arena held by the machines, nests in its walls (ringed red) building a machine each every 20 s until they are destroyed",
    ),
    (
        "overseer",
        "Kill the Overseer: the dock laid out with his office deep inside; waves while he lives, and hurt he walks for a far airlock with the bounty",
    ),
    (
        "heist",
        "A data heist: the dock laid out with three server rooms; hold V at each terminal, every one taken bringing the waves sooner and bigger",
    ),
    (
        "prison",
        "A prison break: the dock laid out with a cell; cut its door open (V) and the prisoners follow the crew as bots",
    ),
    (
        "fuelrun",
        "A fuel run: the dock laid out with a depot and a reactor; carry drums (V) to the reactor, the carrier unable to shoot and a hit blowing the drum",
    ),
    (
        "salvage",
        "A salvage sweep: the dock laid out with cargo holds; carry crates (V) aboard the ship for money, each taken bringing a bigger wave",
    ),
    (
        "bombs",
        "Bomb disposal: the combat crew defending the spawn; defuse every charge (V, 8 s) inside 3:00 or the station goes up with the run",
    ),
    (
        "doors",
        "Hold the doors: the spawn laid out with a vault; the machines break each door in 20 s at it, the commander inside down loses the run; hold 3:00 and the Republic comes",
    ),
    (
        "chief",
        "Protect the commander: the spawn's commander walks his rounds, every machine hunting him; keep him alive 2:00",
    ),
    (
        "relics",
        "The droids arena with one short wave: clear it, go back to the ship, and the reward screen offers the site's relics",
    ),
    (
        "heart",
        "The Machine Heart: the crew (one player) in tier-three kit at the trader under the Heart on scaling.ron's floor with BIMS_HEART_MONEY (800 000) to spend, the Heart the next trip; BIMS_HEART_PHASE=1, 2 or 3 docks at the fortress at once (sealed, past its seal, overloading) on the Heart's day (BIMS_HEART_DAY, days gone)",
    ),
    (
        "manufacturers",
        "A site of the Manufacturers': the combat crew at the nearest one on day BIMS_MANUFACTURER_DAY (scaling.ron's areas: their people alone in area 0, machines beside them through tier one, machines alone from tier two)",
    ),
    (
        "end",
        "The end fight with company: a lobby at code THEEND for others to join, Start pressed once a second player is in, then the ready check at the Machine Heart with ten plain classless bots and everybody in tier-three kit; the waves are the setup's difficulty on the Heart's day 32 (BIMS_END_DAY, days gone) with no first-mission ease",
    ),
    (
        "end offline",
        "That run alone, no relay: the game setup opens with a start picked; choose your class and press Start",
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
                    "{fight}, the crew member you steer {a} {word} at the top level (the twentieth), every skill point unspent (BIMS_LEVEL says otherwise, BIMS_RANKS=q,c,e,r buys ranks)"
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
        Some("guardian") => Launch::Guardian,
        Some("bomber") => Launch::TierTwo(bims::droid::DroidKind::Bomber),
        Some("lancer") => Launch::TierTwo(bims::droid::DroidKind::Lancer),
        Some("conductor") => Launch::TierTwo(bims::droid::DroidKind::Conductor),
        Some("relics") => Launch::Relics,
        Some("breaches") => Launch::Mission(world::run::Mission::Breaches),
        Some("sabotage") => Launch::Mission(world::run::Mission::Sabotage),
        Some("evacuation") => Launch::Mission(world::run::Mission::Evacuation),
        Some("nests") => Launch::Mission(world::run::Mission::Nests),
        Some("overseer") => Launch::Mission(world::run::Mission::Overseer),
        Some("heist") => Launch::Mission(world::run::Mission::Heist),
        Some("prison") => Launch::Mission(world::run::Mission::Prison),
        Some("fuelrun") => Launch::Mission(world::run::Mission::FuelRun),
        Some("salvage") => Launch::Mission(world::run::Mission::Salvage),
        Some("bombs") => Launch::Mission(world::run::Mission::Bombs),
        Some("doors") => Launch::Mission(world::run::Mission::Doors),
        Some("chief") => Launch::Mission(world::run::Mission::Chief),
        Some("heart") => Launch::Heart,
        Some("manufacturers") => Launch::Manufacturers,
        Some("end") => match std::env::args().nth(2).as_deref() {
            None => Launch::End,
            Some("offline") => Launch::EndOffline,
            Some(_) => usage(),
        },
        Some("end_offline") => Launch::EndOffline,
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
    // Which plan the stations are built on, before anything is built.
    dev::apply_station_plan();
    // How the shape buffer's two jobs are run (task 122).
    install_render_join();

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
        lightmap::LightMapPlugin,
        dev::DevPlugin,
        sound::SoundPlugin,
        theme::ThemePlugin,
        screens::builder::BuilderPlugin,
        screens::designer::DesignerPlugin,
        screens::game::GamePlugin,
        screens::loading::LoadingPlugin,
        screens::station::StationBuilderPlugin,
        wavecfg::WaveConfigPlugin,
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
        Launch::Game | Launch::End | Launch::EndOffline => {}
        Launch::Simulation
        | Launch::Test
        | Launch::TestPlanet
        | Launch::DroidsAtTier(_)
        | Launch::Droids
        | Launch::DroidsAs(_)
        | Launch::DroidsPlanet
        | Launch::Crisis
        | Launch::Jammer
        | Launch::Guardian
        | Launch::TierTwo(_)
        | Launch::Relics
        | Launch::Mission(_)
        | Launch::Heart
        | Launch::Manufacturers
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

/// Tell `ship::Session::render` how to run its two jobs — the crew's room
/// drawing itself, and the stations' own pictures being built — side by
/// side (task 122): on Bevy's compute pool, so no second pool competes
/// with Bevy's. The picture is the same bit for bit however they are run.
/// `BIMS_RENDER_JOIN=serial` runs them one after the other, as a build
/// with no threads does, and `=thread` on a thread of their own a frame,
/// which is how the three were measured against each other.
fn install_render_join() {
    if cfg!(target_arch = "wasm32") {
        return;
    }
    match std::env::var("BIMS_RENDER_JOIN").as_deref() {
        Ok("serial") => ship::fork::set_join(ship::fork::serial),
        Ok("thread") => ship::fork::set_join(ship::fork::scoped),
        _ => ship::fork::set_join(pool_join),
    }
}

/// The first job on Bevy's compute pool and the second on this thread,
/// back when both are done. With no pool up — nothing in a run, since the
/// shape buffer is only built inside a system — the two run in turn.
fn pool_join(a: &mut (dyn FnMut() + Send), b: &mut (dyn FnMut() + Send)) {
    match bevy::tasks::ComputeTaskPool::try_get() {
        Some(pool) => {
            pool.scope(|s| {
                s.spawn(async move { a() });
                b();
            });
        }
        None => ship::fork::serial(a, b),
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
