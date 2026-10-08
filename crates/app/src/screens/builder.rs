//! The front of the game: the start menu, the setup screen, and the lobby.
//!
//! The first screens a player meets, and deliberately separate from the
//! room: the builder deals in settings and has no `Game`,
//! and the room deals in a `Game` and has no menus. The galaxy is never
//! shown (October 2026: the map is the floor): `crates/lobby` only rolls
//! the start in it.
//!
//! The multiplayer half (feature 59) is `crate::net`: the lobby is a room
//! at the relay, the host's settings go to everybody in it, and the
//! host's Start takes the whole room into the run. The screen reads the
//! room — who is in it, whose it is — off the `Online` resource and never
//! past it, which is what made the wire a change to that one object rather
//! than to the screens.
//!
//! What the settings come out as, and the only things that leave this
//! screen, are plain numbers: a count of euros, a tile count, a seed, a
//! type and two ids. Nothing but numbers cross into the simulation.

use bevy::prelude::*;
use bevy_egui::{EguiContexts, EguiPrimaryContextPass, egui};
use bims::character::{Hair, Look, Shade, Tint};
use lobby::Lobby;
use wire::To;
use world::Class;
use world::droid::{Difficulty, WaveScaling};

use super::backdrop::{Backdrop, Backdrops};
use crate::canvas::{paint_shapes, rect_of, root_ui};
use crate::format::euros;
use crate::keys::Keys;
use crate::names::*;
use crate::net::{Event, Online, Packet, SettingsWire};
use crate::settings::{Allowed, Sheet, settings_sheet};
use crate::shapes::View;
use crate::sound::Sounds;
use crate::wavecfg::Watched;
use crate::{Screen, theme};

/// What each of the crew brings, in whole euros. It all goes into one pool.
/// A run sets out on the default ship with nothing else to spend it on but
/// gear and time (feature 102), so the standard is
/// `world::data::START_MONEY_PER_BIM` and the other two are half and
/// double it.
const MONEY: [(&str, u64); 3] = [
    ("Lean", world::data::START_MONEY_PER_BIM / 2),
    ("Standard", world::data::START_MONEY_PER_BIM),
    ("Full", world::data::START_MONEY_PER_BIM * 2),
];

/// The shape of the galaxy, by `worldgen::GalaxyType`.
const GALAXIES: [(&str, &str); 4] = [
    ("Two-arm spiral", "Two arms, easy to read"),
    ("Spiral", "Four arms, busier"),
    ("Elliptical", "A squashed cloud"),
    ("Round", "Dense in the middle"),
];

const DEFAULT_MONEY: u64 = world::data::START_MONEY_PER_BIM;
const DEFAULT_GALAXY: u32 = 0;

/// Berths in a lobby. Four is a guess at the eventual crew ceiling.
const LOBBY_SLOTS: usize = 4;

/// Room codes are read out loud down a phone line, so the alphabet leaves
/// out the pairs that are heard wrong: O/0, I/1. The relay deals them
/// (`wire::CODE_ALPHABET`); the field takes as many letters as one is.
const CODE_LENGTH: usize = wire::CODE_LENGTH;

/// How long a passing remark stays on screen, in seconds.
const NOTE_SECONDS: f64 = 4.0;

/// The start menu's card over its picture: how wide, in points, and how
/// far down the window its top is.
const MENU_CARD_WIDTH: f32 = 470.0;
const MENU_CARD_DOWN: f32 = 0.22;
/// The setup's card over its picture: as wide as this at most, in points,
/// and the whole height between the bars.
const SETUP_CARD_WIDTH: f32 = 880.0;
/// The setup's bars and card: the panels' grey, dark enough to read on
/// and thin enough that the cockpit shows through.
const SETUP_BAR: egui::Color32 = egui::Color32::from_rgba_premultiplied(17, 18, 20, 215);
const SETUP_CARD: egui::Color32 = egui::Color32::from_rgba_premultiplied(12, 13, 15, 238);

/// Where the start menu's card goes in a panel `full` big: centred across
/// it, `MENU_CARD_DOWN` of the way down, narrowed to fit a small window.
fn menu_card_slot(full: egui::Rect) -> egui::Rect {
    let width = MENU_CARD_WIDTH.min(full.width());
    egui::Rect::from_min_max(
        egui::pos2(
            full.center().x - width / 2.0,
            full.top() + full.height() * MENU_CARD_DOWN,
        ),
        egui::pos2(full.center().x + width / 2.0, full.bottom()),
    )
}

/// The start menu's card: dark glass over the picture.
fn menu_card() -> egui::Frame {
    egui::Frame::new()
        .fill(egui::Color32::from_rgba_premultiplied(10, 10, 12, 228))
        .stroke(egui::Stroke::new(1.0, theme::LINE))
        .corner_radius(10.0)
        .inner_margin(24.0)
}

/// Where the setup's card goes in its panel: centred, the full height.
fn setup_card_slot(full: egui::Rect) -> egui::Rect {
    let width = SETUP_CARD_WIDTH.min(full.width());
    egui::Rect::from_min_max(
        egui::pos2(full.center().x - width / 2.0, full.top()),
        egui::pos2(full.center().x + width / 2.0, full.bottom()),
    )
}

/// The setup's card, the menu's glass a shade lighter.
fn setup_card() -> egui::Frame {
    menu_card().fill(SETUP_CARD).inner_margin(16.0)
}

/// What the lobby's footer says when nothing has happened lately: where
/// the room is.
fn lobby_said() -> String {
    format!("Through {}.", crate::net::server_url())
}

/// What a game would be started with. One object, shared by every settings
/// tool on the screen: the solo screen and the lobby cannot disagree about
/// what was picked. `spawn` is `None` until a station has been picked, and
/// its absence is what disables Start.
#[derive(Resource, Clone, Debug)]
pub struct Settings {
    pub money_per_bim: u64,
    pub seed: u64,
    pub galaxy: u32,
    pub spawn: Option<(u32, u32)>,
    /// How many players, and which slot is this one — the lobby's, or one
    /// and nought.
    pub players: u32,
    pub slot: u32,
    /// What the players called their crew, in slot order — empty at a slot
    /// is the crew's own name (`names::crew_name`). Dealt at Start with the
    /// slots, so every machine calls each Bim the same.
    pub names: Vec<String>,
    /// How each player wears their Bim's hair, in slot order, dealt at
    /// Start like the names (feature 62). Empty is every slot's dealt look.
    pub hair: Vec<(Hair, Shade)>,
    /// Each player's class for their Bim, in slot order, dealt at Start
    /// like the hair (feature 74). Empty is every slot with none.
    pub classes: Vec<Class>,
    /// Which colour each player's own Bim is ringed in, in slot order,
    /// dealt at Start like the hair (feature 84). Empty is every slot
    /// its own of `Tint::ALL`.
    pub tints: Vec<Tint>,
    /// What the **host's** profile opened for the run (feature 106): the
    /// classes that may be picked. This machine's own
    /// until a host's settings arrive, and the host's after.
    pub unlocks: crate::profile::RunUnlocks,
    /// The run's difficulty (every dial of the wave formula, task 147),
    /// the host's to pick and dealt with the rest; `None`
    /// until one is picked, which is the tuning file's (`scaling.ron`).
    pub difficulty: Option<world::droid::Difficulty>,
    /// The host's `rewards.ron` and `weapons.ron`, dealt at a Start with
    /// company (`wavecfg::Dealt`): every machine plays them for the run,
    /// whatever its own files say. `None` in a game of one, which plays
    /// its files and is tuned by them as it runs.
    pub rewards: Option<world::rewards::Rewards>,
    pub weapons: Option<bims::balance::WeaponDamage>,
    /// The `end` command's run (`crate::Launch::End`): the lobby's run
    /// opened at the Machine Heart with ten classed bots at the top level
    /// in tier-three kit (`run::build_run`). Dealt with the rest.
    pub end: bool,
    /// The world clock's day the `end` run opens on, so its waves are a
    /// run's that far on (`dev::end_day`, the host's, dealt with the rest).
    pub end_day: u32,
    /// The setup's auto-shoot (October 2026): this player's own Bim fires
    /// at the nearest enemy in reach, or the one the crosshair picked, on
    /// top of the keys and the trigger (`screens::game`). Each player's
    /// own choice and nobody else's: it never crosses the wire, and the
    /// room is told only the aim and the trigger a click would tell it.
    pub auto_shoot: bool,
    /// The run's ascension (`world::ascension`), the host's to pick and
    /// dealt with the rest.
    pub ascension: u32,
    /// The highest ascension this machine's profile has opened
    /// (`profile::ascension_open`): how far the picker goes. Never
    /// crosses the wire — a guest only watches the host's pick.
    pub ascension_open: u32,
    /// The Dev tab's picks (`super::devtab`), the host's, dealt with the
    /// rest; the default is the run as it plays.
    pub dev: super::devtab::DevSetup,
}

impl Default for Settings {
    fn default() -> Settings {
        Settings {
            money_per_bim: DEFAULT_MONEY,
            seed: crate::screens::rand_seed(),
            galaxy: DEFAULT_GALAXY,
            spawn: None,
            players: 1,
            slot: 0,
            names: Vec::new(),
            hair: Vec::new(),
            classes: Vec::new(),
            tints: Vec::new(),
            unlocks: crate::profile::RunUnlocks::of(&crate::profile::load()),
            difficulty: None,
            rewards: None,
            weapons: None,
            end: false,
            end_day: crate::dev::end_day(),
            auto_shoot: crate::dev::auto_shoot(),
            ascension: crate::profile::ascension_reached(),
            ascension_open: crate::profile::ascension_open(),
            dev: Default::default(),
        }
    }
}

/// The lobby's end of the seam. The room itself — the socket, the code,
/// who is in it — is `crate::net::Online`, one for the whole game; what
/// the lobby keeps of its own is what it last told the others, so the
/// settings go out when they change and not every frame the tab is
/// drawn.
#[derive(Default)]
struct Net {
    pushed: Option<SettingsWire>,
}

impl Net {
    /// The host's settings, to everybody — on a change, or when `again`
    /// says so (somebody joined and has nothing yet). A guest pushes
    /// nothing: the host decides the settings.
    fn push(&mut self, online: &Online, settings: &Settings, again: bool) {
        if !online.is_host() {
            return;
        }
        let now = SettingsWire::of(settings);
        if again || self.pushed != Some(now) {
            self.pushed = Some(now);
            online.send(To::All, &Packet::Settings(Box::new(now)));
        }
    }
}

/// A passing remark on a line, and when it goes.
struct Remark {
    text: String,
    warn: bool,
    until: f64,
}

impl Remark {
    fn say(text: impl Into<String>, warn: bool, now: f64) -> Option<Remark> {
        Some(Remark {
            text: text.into(),
            warn,
            until: now + NOTE_SECONDS,
        })
    }

    fn show(slot: &mut Option<Remark>, ui: &mut egui::Ui, now: f64, back: &str) {
        if let Some(r) = slot
            && now > r.until
        {
            *slot = None;
        }
        match slot {
            Some(r) => {
                ui.label(egui::RichText::new(&r.text).color(if r.warn {
                    theme::WARN
                } else {
                    theme::MUTED
                }));
            }
            None => {
                ui.label(egui::RichText::new(back).color(theme::MUTED));
            }
        }
    }
}

#[derive(Resource)]
pub struct BuilderScreen {
    net: Net,
    join_code: String,
    /// What this player calls their Bim, as typed: kept across lobbies,
    /// said to the room on every change (`Online::say_bim_name`) and
    /// dealt with the slots at Start.
    bim_name: String,
    /// The name as last said to the room, so it goes out on a change and
    /// again for a joiner, not every frame.
    said_bim_name: Option<String>,
    /// How this player's Bim wears its hair, as picked: kept like the
    /// name, said to the room the same way (`Online::say_bim_hair`).
    bim_hair: (Hair, Shade),
    said_bim_hair: Option<(Hair, Shade)>,
    /// Which colour this player's own Bim is ringed in, as picked
    /// (feature 84): kept like the hair, said the same way
    /// (`Online::say_bim_tint`), and a colour another player in the
    /// lobby has taken is not offered.
    bim_tint: Tint,
    said_bim_tint: Option<Tint>,
    /// What class this player's Bim is, as picked: kept like the hair,
    /// said to the room the same way (`Online::say_bim_class`).
    bim_class: Class,
    said_bim_class: Option<Class>,
    /// The chooser's picture of it, drawn afresh each frame it is shown.
    portrait: bims::draw::DrawList,
    join_note: Option<Remark>,
    lobby_said: Option<Remark>,
    /// Who was in the lobby last frame, so a roster change can say who
    /// came or went.
    seen: Vec<wire::PeerId>,
    /// `BIMS_AUTO` has pressed Start; once.
    auto_done: bool,
    /// The `end` command has asked the relay for its room; once, so a
    /// refusal leaves the menu up rather than asking every frame.
    end_asked: bool,
    /// The galaxy the run is rolled on: never shown, only what the start
    /// is picked in (`Lobby::random_start`).
    lobby: Lobby,
    /// The Load window over the menu, if it is up, and its page's state —
    /// `crate::save`. A saved game is picked up from here without the
    /// setup: the session is stood up round it and the game screen opens.
    loading: bool,
    saves: crate::save::Saves,
    /// The Esc sheet, if it is up: the settings alone, as the game's
    /// Esc brings them up, with no game here yet to save or load.
    sheet: Option<Sheet>,
    /// The wave formula as the tuning file says it (`crate::wavecfg`),
    /// this frame: what the difficulty shows until the host picks one.
    file_scaling: WaveScaling,
    /// What Save as default said, for a few seconds under the difficulty.
    difficulty_note: Option<Remark>,
    /// Which of the setup's two tabs is up.
    tab: SetupTab,
}

/// The setup's tabs: the crew and the run, and the wave formula's dials.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
enum SetupTab {
    #[default]
    Crew,
    Scaling,
    /// What a playtest run sets out with (`super::devtab`).
    Dev,
}

pub struct BuilderPlugin;

impl Plugin for BuilderPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Settings>()
            .add_plugins(super::backdrop::BackdropPlugin)
            .add_systems(
                Startup,
                open.run_if(|l: Res<crate::Launch>| {
                    matches!(
                        *l,
                        crate::Launch::Game | crate::Launch::End | crate::Launch::EndOffline
                    )
                }),
            )
            .add_systems(
                EguiPrimaryContextPass,
                // Before the loading screen's: the frame the run's build
                // comes back in, this still sees it busy and drains
                // nothing, where after it the state is still the lobby's
                // and the host's first steps would be drained and dropped.
                frame
                    .run_if(|s: Res<State<Screen>>| {
                        matches!(s.get(), Screen::Menu | Screen::Setup | Screen::Lobby)
                    })
                    .before(super::loading::show),
            );
    }
}

fn open(mut commands: Commands, mut settings: ResMut<Settings>) {
    // The seed, the galaxy's shape and the start are never chosen: a game
    // opens on all three at random, so Start can be pressed at once.
    roll_galaxy(&mut settings);
    let lobby = Lobby::new(
        settings.seed,
        ship::session::galaxy_type(settings.galaxy),
        560.0,
        400.0,
    );
    let mut screen = BuilderScreen {
        net: Net::default(),
        join_code: String::new(),
        bim_name: crate::dev::bim_name(),
        said_bim_name: None,
        bim_hair: crate::dev::bim_hair(),
        bim_tint: crate::dev::bim_tint(),
        said_bim_tint: None,
        said_bim_hair: None,
        bim_class: setup_class(),
        said_bim_class: None,
        portrait: bims::draw::DrawList::new(),
        join_note: None,
        lobby_said: None,
        seen: Vec::new(),
        auto_done: false,
        end_asked: false,
        lobby,
        loading: false,
        saves: crate::save::Saves::default(),
        sheet: None,
        file_scaling: WaveScaling::DEFAULT,
        difficulty_note: None,
        tab: SetupTab::Crew,
    };
    roll_start(&mut screen, &mut settings);
    commands.insert_resource(screen);
}

#[allow(clippy::too_many_arguments)]
fn frame(
    mut contexts: EguiContexts,
    mut screen: ResMut<BuilderScreen>,
    mut settings: ResMut<Settings>,
    state: Res<State<Screen>>,
    mut next: ResMut<NextState<Screen>>,
    mut commands: Commands,
    time: Res<Time>,
    window: Single<&Window>,
    mut online: ResMut<Online>,
    backdrops: Res<Backdrops>,
    mut sounds: ResMut<Sounds>,
    mut bindings: ResMut<Keys>,
    scaling_file: Res<Watched<WaveScaling>>,
    mut loading: ResMut<super::loading::Loading>,
    launch: Res<crate::Launch>,
) -> Result {
    let ctx = contexts.ctx_mut()?.clone();
    // The run being built behind the loading screen (`screens::loading`):
    // nothing here moves, and the room's words wait for the game.
    if loading.busy() {
        return Ok(());
    }
    screen.file_scaling = scaling_file.dials();
    // Read before anything is laid out: an Esc that shuts the Load window
    // or leaves a text field is theirs, not the sheet's.
    let esc_taken = screen.loading || ctx.egui_wants_keyboard_input();
    let screen = &mut *screen;
    let settings = &mut *settings;
    let online = &mut *online;
    let now = ctx.input(|i| i.time);
    screen.lobby.advance(time.delta_secs().min(0.1));
    let mut root = root_ui(&ctx);
    let mut go: Option<Screen> = None;
    let mut start = false;

    // `BIMS_AUTO`: the lobby played with nobody at the keyboard (`dev.rs`).
    if let Some(auto) = crate::dev::auto()
        && !screen.auto_done
    {
        if *state.get() == Screen::Menu && online.link.state() == &crate::net::LinkState::Offline {
            match auto {
                crate::dev::Auto::Create => {
                    online.create();
                    go = Some(Screen::Lobby);
                }
                crate::dev::Auto::Join(code) => online.join(&code),
            }
        }
        if *state.get() == Screen::Lobby
            && online.is_host()
            && online.peers.len() >= crate::dev::auto_players()
            && online.builds_differ().is_none()
        {
            if settings.spawn.is_none() {
                pick_random_start(screen, settings, online);
            }
            start = true;
            screen.auto_done = true;
        }
    }

    // The `end` command (`crate::Launch::End`): a lobby opened at the
    // menu at `dev::END_CODE` for the others to join, and Start pressed on
    // its own once `dev::END_PLAYERS` are in it — straight to the ready
    // check at the Machine Heart. Every run it starts is the `end` run,
    // a Start pressed by hand too.
    if *launch == crate::Launch::End {
        settings.end = true;
        if !screen.end_asked
            && *state.get() == Screen::Menu
            && online.link.state() == &crate::net::LinkState::Offline
        {
            online.create_at(crate::dev::END_CODE);
            screen.end_asked = true;
            go = Some(Screen::Lobby);
        }
        if !screen.auto_done
            && *state.get() == Screen::Lobby
            && online.is_host()
            && online.peers.len() >= crate::dev::END_PLAYERS
            && online.builds_differ().is_none()
        {
            if settings.spawn.is_none() {
                pick_random_start(screen, settings, online);
            }
            start = true;
            screen.auto_done = true;
        }
    }
    // `end offline` (`crate::Launch::EndOffline`): the same run alone, no
    // relay — the solo game setup opened once with a start picked, for a
    // class and a Start.
    if *launch == crate::Launch::EndOffline {
        settings.end = true;
        if !screen.end_asked && *state.get() == Screen::Menu {
            if settings.spawn.is_none() {
                pick_random_start(screen, settings, online);
            }
            screen.end_asked = true;
            go = Some(Screen::Setup);
        }
    }

    // What the room said since last frame: the relay, and the others in
    // it. A guest's settings and its Start come from the host this way;
    // a refusal or a closed room is a line and the menu again.
    let mut events = online.drain(now).into_iter();
    while let Some(event) = events.next() {
        match event {
            // Only a game under way is dialled again, which is not here.
            Event::Connected | Event::Reconnecting | Event::Rejoined => {}
            Event::Joined => {
                screen.seen = online.peers.iter().map(|p| p.id).collect();
                if (crate::dev::auto().is_some() || *launch == crate::Launch::End)
                    && let Some(code) = &online.code
                {
                    println!("lobby: {code}");
                }
                if online.is_host() {
                    screen.net.push(online, settings, true);
                } else {
                    screen.join_note = None;
                    go = Some(Screen::Lobby);
                }
                // And what this player calls their Bim, to whoever is
                // already in.
                screen.said_bim_name = None;
                screen.said_bim_hair = None;
                screen.said_bim_tint = None;
                screen.said_bim_class = None;
            }
            Event::Roster => {
                // A joiner has no settings yet; the host says them again.
                screen.net.push(online, settings, true);
                // Nor anybody's Bim's name: everybody says theirs again.
                screen.said_bim_name = None;
                screen.said_bim_hair = None;
                screen.said_bim_tint = None;
                screen.said_bim_class = None;
                // And who came or went, by name.
                let now_here: Vec<wire::PeerId> = online.peers.iter().map(|p| p.id).collect();
                for p in &online.peers {
                    if !screen.seen.contains(&p.id) {
                        screen.lobby_said = Remark::say(player_joined(&p.name), false, now);
                    }
                }
                if screen.seen.iter().any(|id| !now_here.contains(id)) {
                    screen.lobby_said = Remark::say(SOMEBODY_LEFT, false, now);
                }
                screen.seen = now_here;
            }
            Event::Rejected(why) => {
                let line = relay_refusal(why);
                screen.join_note = Remark::say(line.clone(), true, now);
                screen.lobby_said = Remark::say(line, true, now);
                if *state.get() == Screen::Lobby && online.code.is_none() {
                    go = Some(Screen::Menu);
                }
            }
            Event::Lost(why) => {
                let line = format!("{LINK_LOST} {why}");
                screen.join_note = Remark::say(line.clone(), true, now);
                if *state.get() == Screen::Lobby {
                    go = Some(Screen::Menu);
                }
            }
            Event::Closed(why) => {
                screen.join_note = Remark::say(room_closed(why), true, now);
                if *state.get() == Screen::Lobby {
                    go = Some(Screen::Menu);
                }
            }
            Event::Packet { from, packet } => match packet {
                Packet::Settings(wire) if Some(from) == online.host => {
                    let galaxy_moved = wire.seed != settings.seed || wire.galaxy != settings.galaxy;
                    wire.onto(settings);
                    if galaxy_moved {
                        screen
                            .lobby
                            .set_world(settings.seed, ship::session::galaxy_type(settings.galaxy));
                    }
                    screen.lobby.spawn = settings.spawn;
                }
                Packet::Suggest { star } => {
                    screen.lobby.ping(star);
                    screen.lobby_said = Remark::say(
                        format!("{} points at a star.", crate::net::peer_name(online, from)),
                        false,
                        now,
                    );
                }
                Packet::Start {
                    settings: wire,
                    slots,
                    names,
                    hair,
                    classes,
                    tints,
                } if Some(from) == online.host => {
                    wire.onto(settings);
                    online.slots = slots;
                    settings.players = online.slots.len().max(1) as u32;
                    settings.slot = online.my_slot();
                    settings.names = names;
                    // This player's own Bim is called what its field says,
                    // whether or not that reached the host before Start;
                    // the others learn it off the wire (`Online::names_said`).
                    let mine = settings.slot as usize;
                    if settings.names.len() <= mine {
                        settings.names.resize(mine + 1, String::new());
                    }
                    settings.names[mine] = wire::tidy_name(&screen.bim_name);
                    // And its hair is what the chooser says, the same way.
                    settings.hair = hair;
                    if settings.hair.len() <= mine {
                        for s in settings.hair.len()..=mine {
                            let look = Look::of(s);
                            settings.hair.push((look.hair, look.shade));
                        }
                    }
                    settings.hair[mine] = screen.bim_hair;
                    // But its class is what the host dealt, even its own:
                    // the class is the world's, so a pick that missed
                    // Start would part the guest's world from the host's.
                    settings.classes = classes
                        .into_iter()
                        .map(|c| Class::from_code(c).unwrap_or_default())
                        .collect();
                    if settings.classes.len() <= mine {
                        settings.classes.resize(mine + 1, Class::None);
                    }
                    // And its colour, the same way (feature 84).
                    settings.tints = tints.into_iter().map(Tint::from_code).collect();
                    if settings.tints.len() <= mine {
                        for s in settings.tints.len()..=mine {
                            settings.tints.push(Tint::ALL[s % Tint::ALL.len()]);
                        }
                    }
                    settings.tints[mine] = screen.bim_tint;
                    // Straight into the run, as the host does at its own
                    // Start (feature 102): the same numbers, the same world.
                    let size = Vec2::new(window.width().max(64.0), window.height().max(64.0));
                    loading.start_run(&mut commands, settings, size);
                    // Whatever came behind the Start is the run's — the
                    // host's first steps and orders, should its build be
                    // done first — kept for the game screen in order, as
                    // a trip's leftovers are. Dropped here, this world
                    // would run behind the host's for good.
                    loading.stash(events.by_ref());
                    break;
                }
                // The host loaded a game: its world, whole, is this end's
                // now, and the game screen opens round it as it does for
                // a load here (feature 67). Nothing before Start deals
                // the slots, so the seat is the one `Online::deal` would
                // deal: this player's place in join order.
                Packet::World { save, dealt, .. }
                    if Some(from) == online.host && !online.is_host() =>
                {
                    let size = Vec2::new(window.width().max(64.0), window.height().max(64.0));
                    let seat = if online.slots.is_empty() {
                        online
                            .peers
                            .iter()
                            .position(|p| Some(p.id) == online.me)
                            .unwrap_or(0) as u32
                    } else {
                        online.my_slot()
                    };
                    match ship::Session::restore_as(&save, seat, size.x, size.y) {
                        Ok(mut loaded) => {
                            // The host's tuning, for the run (`wavecfg::Dealt`).
                            let dealt = dealt.map(|d| *d);
                            crate::wavecfg::deal(dealt);
                            if let (Some(d), Some(game)) = (dealt, &mut loaded.game) {
                                d.onto(&mut game.world);
                            }
                            commands.insert_resource(crate::screens::run::ShipSession(loaded));
                            screen.loading = false;
                            go = Some(Screen::Game);
                            // What came behind it is that world's.
                            loading.stash(events.by_ref());
                            break;
                        }
                        Err(why) => {
                            let line = world_refused(&crate::save::load_error(why));
                            screen.lobby_said = Remark::say(line, true, now);
                        }
                    }
                }
                _ => {}
            },
        }
    }
    // What this player calls their Bim, to the room: when it changes, and
    // again after a join — the field is read here rather than where it is
    // drawn, so a name typed on the Setup tab still goes out with the
    // World tab up.
    if online.is_online() {
        let name = wire::tidy_name(&screen.bim_name);
        if screen.said_bim_name.as_deref() != Some(name.as_str()) {
            online.say_bim_name(&name);
            screen.said_bim_name = Some(name);
        }
        if screen.said_bim_hair != Some(screen.bim_hair) {
            online.say_bim_hair(screen.bim_hair.0, screen.bim_hair.1);
            screen.said_bim_hair = Some(screen.bim_hair);
        }
        // And the colour (feature 84). One somebody else took while this
        // player was looking elsewhere is stepped off first, so no two
        // Bims are ringed alike whoever said last.
        let taken = online.tints_taken();
        if taken.contains(&screen.bim_tint)
            && let Some(free) = Tint::ALL.iter().copied().find(|t| !taken.contains(t))
        {
            screen.bim_tint = free;
        }
        if screen.said_bim_tint != Some(screen.bim_tint) {
            online.say_bim_tint(screen.bim_tint);
            screen.said_bim_tint = Some(screen.bim_tint);
        }
        if screen.said_bim_class != Some(screen.bim_class) {
            online.say_bim_class(screen.bim_class);
            // And which build this game is, with it: on joining and after
            // every join, so nobody starts a run on two different games.
            online.say_build();
            screen.said_bim_class = Some(screen.bim_class);
        }
    }

    match state.get() {
        Screen::Menu => {
            // The start picture behind it all, and the menu on a dark card
            // over it so the words read against the nebula.
            super::backdrop::paint(&ctx, &backdrops, Backdrop::Start);
            egui::CentralPanel::default()
                .frame(egui::Frame::NONE)
                .show(&mut root, |ui| {
                    let slot = menu_card_slot(ui.max_rect());
                    ui.scope_builder(egui::UiBuilder::new().max_rect(slot), |ui| {
                        menu_card().show(ui, |ui| {
                            ui.vertical_centered(|ui| {
                                ui.label(egui::RichText::new("Bims").size(40.0).strong());
                                ui.label(
                                    egui::RichText::new(
                                        "A ship, a crew, and whatever they can grow.",
                                    )
                                    .color(theme::MUTED),
                                );
                                ui.add_space(20.0);
                                ui.horizontal(|ui| {
                                    ui.add_space((ui.available_width() - 390.0).max(0.0) / 2.0);
                                    if theme::big(ui, "Play", true).clicked() {
                                        roll_everything(screen, settings);
                                        go = Some(Screen::Setup);
                                    }
                                    if theme::big(ui, "Create lobby", true).clicked() {
                                        roll_everything(screen, settings);
                                        online.create();
                                        screen.net.pushed = None;
                                        screen.lobby_said = Remark::say(CONNECTING, false, now);
                                        go = Some(Screen::Lobby);
                                    }
                                    // A saved game, picked up where it was left: the
                                    // window lists what is on disk, read afresh each
                                    // time it opens. Not for a guest, whose world is
                                    // the host's (feature 67).
                                    let load = theme::big(ui, "Load", !online.is_guest())
                                        .on_disabled_hover_text(LOAD_GUEST);
                                    if load.clicked() {
                                        screen.saves.note = None;
                                        screen.saves.refresh();
                                        screen.loading = true;
                                    }
                                });
                                ui.add_space(20.0);
                                ui.horizontal(|ui| {
                                    ui.add_space((ui.available_width() - 300.0).max(0.0) / 2.0);
                                    ui.label(
                                        egui::RichText::new("Join with a code").color(theme::MUTED),
                                    );
                                    let field = ui.add(
                                        egui::TextEdit::singleline(&mut screen.join_code)
                                            .char_limit(CODE_LENGTH)
                                            .hint_text("——————")
                                            .desired_width(90.0),
                                    );
                                    let submitted = field.lost_focus()
                                        && ui.input(|i| i.key_pressed(egui::Key::Enter));
                                    if ui.button("Join").clicked() || submitted {
                                        let typed = wire::normalise_code(&screen.join_code);
                                        if wire::is_code(&typed) {
                                            online.join(&typed);
                                            screen.join_note = Remark::say(CONNECTING, false, now);
                                        } else {
                                            screen.join_note = Remark::say(NOT_A_CODE, true, now);
                                        }
                                    }
                                });
                                Remark::show(&mut screen.join_note, ui, now, "");
                            });
                        });
                    });
                });
            if screen.loading {
                let mut open = true;
                let mut asked = None;
                egui::Window::new("Load")
                    .collapsible(false)
                    .resizable(false)
                    .open(&mut open)
                    .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
                    .show(&ctx, |ui| {
                        asked = crate::save::load_page(ui, &mut screen.saves);
                    });
                if !open || ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
                    screen.loading = false;
                }
                if let Some(crate::save::Request::Load(path)) = asked {
                    let size = Vec2::new(window.width().max(64.0), window.height().max(64.0));
                    let read = crate::save::read(&path).and_then(|text| {
                        ship::Session::restore(&text, size.x, size.y)
                            .map_err(crate::save::load_error)
                    });
                    match read {
                        Ok(loaded) => {
                            commands.insert_resource(crate::screens::run::ShipSession(loaded));
                            screen.loading = false;
                            go = Some(Screen::Game);
                        }
                        Err(why) => screen.saves.failed(why),
                    }
                }
            }
        }
        Screen::Setup => {
            // The cockpit behind it, moving; the bars and the settings'
            // card see-through over it.
            super::backdrop::paint(&ctx, &backdrops, Backdrop::Setup);
            let bar = egui::Frame::side_top_panel(&ctx.global_style())
                .fill(SETUP_BAR)
                .inner_margin(egui::Margin::symmetric(12, 8));
            egui::Panel::top("setup-head")
                .frame(bar)
                .show(&mut root, |ui| {
                    ui.horizontal(|ui| {
                        if ui.button("< Back").clicked() {
                            go = Some(Screen::Menu);
                        }
                        ui.add_space(8.0);
                        ui.label(title_text(SETUP_TITLE));
                    });
                });
            egui::Panel::bottom("setup-foot")
                .frame(bar)
                .show(&mut root, |ui| {
                    ui.horizontal(|ui| {
                        let why = start_refusal(settings);
                        match why {
                            Some(why) => {
                                ui.label(egui::RichText::new(why).color(theme::CAUTION));
                            }
                            None => run_summary(ui, screen, settings),
                        }
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if start_button(ui, why.is_none()).clicked() {
                                start = true;
                            }
                        });
                    });
                });
            egui::CentralPanel::default()
                .frame(egui::Frame::NONE.inner_margin(12.0))
                .show(&mut root, |ui| {
                    let slot = setup_card_slot(ui.max_rect());
                    ui.scope_builder(egui::UiBuilder::new().max_rect(slot), |ui| {
                        setup_card().show(ui, |ui| {
                            ui.set_min_size(ui.available_size());
                            tool(ui, screen, settings, online, true, now);
                        });
                    });
                });
        }
        Screen::Lobby => {
            egui::Panel::top("lobby-head").show(&mut root, |ui| {
                ui.horizontal(|ui| {
                    if ui.button("< Leave").clicked() {
                        online.leave();
                        go = Some(Screen::Menu);
                    }
                    ui.label(title_text(LOBBY_TITLE));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button("Copy").clicked()
                            && let Some(code) = &online.code
                        {
                            ui.ctx().copy_text(code.clone());
                            screen.lobby_said = Remark::say(format!("Copied {code}."), false, now);
                        }
                        ui.label(
                            egui::RichText::new(
                                online.code.clone().unwrap_or_else(|| "——————".into()),
                            )
                            .size(18.0)
                            .strong(),
                        );
                        ui.label(egui::RichText::new("Room code").color(theme::MUTED));
                    });
                });
            });
            egui::Panel::bottom("lobby-foot").show(&mut root, |ui| {
                ui.horizontal(|ui| {
                    Remark::show(&mut screen.lobby_said, ui, now, &lobby_said());
                    let why = lobby_start_refusal(settings, online);
                    ui.label(egui::RichText::new(why.unwrap_or("")).color(theme::CAUTION));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if theme::big(ui, "Start", why.is_none()).clicked() {
                            start = true;
                        }
                    });
                });
            });
            let my_bim = wire::tidy_name(&screen.bim_name);
            egui::Panel::left("lobby-crew")
                .default_size(220.0)
                .show(&mut root, |ui| {
                    theme::heading(ui, "Crew");
                    for i in 0..LOBBY_SLOTS {
                        ui.horizontal(|ui| match online.peers.get(i) {
                            Some(p) => {
                                // In the colour their pointer and their
                                // route will be drawn in: the crew are
                                // dealt in this order.
                                theme::swatch(
                                    ui,
                                    theme::ship_color32(ship::world_paint::player_color(i as u32)),
                                );
                                ui.label(if Some(p.id) == online.me {
                                    format!("{} (you)", p.name)
                                } else {
                                    p.name.clone()
                                });
                                ui.label(
                                    egui::RichText::new(if Some(p.id) == online.host {
                                        "Host"
                                    } else {
                                        "Crew"
                                    })
                                    .small()
                                    .color(theme::MUTED),
                                );
                                // And what their Bim is called: what they
                                // typed, else the berth's own name.
                                let bim = if Some(p.id) == online.me {
                                    Some(my_bim.as_str())
                                } else {
                                    online.bim_name(p.id)
                                }
                                .filter(|n| !n.is_empty())
                                .map(str::to_string)
                                .unwrap_or_else(|| {
                                    CREW_NAMES.get(i).map(|s| s.to_string()).unwrap_or_default()
                                });
                                ui.label(egui::RichText::new(bim).color(theme::ACCENT));
                            }
                            None => {
                                theme::swatch(ui, theme::RAISED);
                                ui.label(egui::RichText::new("Open").color(theme::MUTED));
                                ui.label(
                                    egui::RichText::new("Waiting").small().color(theme::MUTED),
                                );
                            }
                        });
                    }
                    ui.label(
                        egui::RichText::new(if online.is_host() {
                            "Read the code out to whoever is joining."
                        } else {
                            "The host decides the settings."
                        })
                        .small()
                        .color(theme::MUTED),
                    );
                });
            let editable = online.is_host();
            egui::CentralPanel::default().show(&mut root, |ui| {
                tool(ui, screen, settings, online, editable, now);
            });
        }
        _ => {}
    }

    // Esc brings the settings up over any of the three and puts them
    // away again, as in a game; while the controls page waits on a key
    // the Esc is that page's.
    if !esc_taken && bindings.listening.is_none() && ctx.input(|i| i.key_pressed(egui::Key::Escape))
    {
        screen.sheet = match screen.sheet {
            None => Some(Sheet::Menu),
            Some(_) => None,
        };
    }
    settings_sheet(
        &ctx,
        &mut screen.sheet,
        &mut sounds.mix,
        &mut bindings,
        &mut screen.saves,
        Allowed::SETTINGS_ONLY,
    );

    if start && lobby_start_refusal(settings, online).is_none() {
        // Start opens the run. What crosses is the numbers and nothing
        // else: the money each Bim brings, how many players there are,
        // which slot you are, the seed, the galaxy type, and the star and
        // station the game starts at.
        // With company, the crew are dealt in join order — the host slot 0
        // — and everybody is told: the same Start on every machine.
        if online.is_online() {
            online.begin();
            // The wave numbers too, though nobody moved a dial: left to
            // each machine they are its own `scaling.ron`, and a game
            // started away from the tree has none (the constants) — two
            // machines laying different waves part at the first, and the
            // guest is sent the whole world mid-fight. The host's file,
            // dealt with the rest; it no longer moves this run.
            if settings.difficulty.is_none() {
                settings.difficulty = Some(crate::wavecfg::file_dials());
            }
            // And the rest of the tuning that moves the world, the same
            // way: what an enemy pays and what things cost, and every
            // weapon's damage.
            settings.rewards = Some(crate::wavecfg::file_dials());
            settings.weapons = Some(crate::wavecfg::file_dials());
            let slots = online.deal();
            settings.players = slots.len().max(1) as u32;
            settings.slot = online.my_slot();
            // The names go with the slots: what each said, and this
            // player's own off the field, since nobody is told their own.
            let mut names = online.deal_names(&slots);
            names[settings.slot as usize] = wire::tidy_name(&screen.bim_name);
            settings.names = names.clone();
            let mut hair = online.deal_hair(&slots);
            hair[settings.slot as usize] = screen.bim_hair;
            settings.hair = hair.clone();
            let mut classes = online.deal_classes(&slots);
            classes[settings.slot as usize] = screen.bim_class;
            settings.classes = classes.clone();
            let mut tints = online.deal_tints(&slots);
            tints[settings.slot as usize] = screen.bim_tint;
            settings.tints = tints.clone();
            online.send(
                To::All,
                &Packet::Start {
                    settings: Box::new(SettingsWire::of(settings)),
                    slots,
                    names,
                    hair,
                    classes: classes.iter().map(|c| c.code()).collect(),
                    tints: tints.iter().map(|t| t.code()).collect(),
                },
            );
        } else {
            settings.players = 1;
            settings.slot = 0;
            settings.rewards = None;
            settings.weapons = None;
            settings.names = vec![wire::tidy_name(&screen.bim_name)];
            settings.hair = vec![screen.bim_hair];
            settings.classes = vec![screen.bim_class];
            settings.tints = vec![screen.bim_tint];
        }
        // Straight into the run (feature 102): the default ship docked at
        // the station picked.
        let size = Vec2::new(window.width().max(64.0), window.height().max(64.0));
        loading.start_run(&mut commands, settings, size);
    }
    if go.is_some() {
        screen.sheet = None;
    }
    if let Some(screen) = go {
        next.set(screen);
    }
    Ok(())
}

/// Why Start cannot be pressed, or `None` if it can.
fn start_refusal(settings: &Settings) -> Option<&'static str> {
    if settings.spawn.is_none() {
        return Some("This galaxy has nowhere to start; go back and play again.");
    }
    None
}

/// The same in a lobby: a station picked, the room reached, and the host
/// the one pressing — a guest waits for the host's Start.
fn lobby_start_refusal(settings: &Settings, online: &Online) -> Option<&'static str> {
    if online.connecting() {
        return Some(CONNECTING);
    }
    if online.is_online() {
        match online.builds_differ() {
            Some(true) => return Some(BUILDS_DIFFER),
            Some(false) => return Some(BUILDS_CHECKING),
            None => {}
        }
    }
    if online.is_online() && !online.is_host() {
        return Some("The host starts the game.");
    }
    start_refusal(settings)
}

/// The setup's gold: the ascension picked, the Start button, a section's
/// mark — the tier-three colour, so the screen's one warm accent is the
/// one the run climbs to.
const GOLD: egui::Color32 = theme::TIER_THREE;
/// A section of the setup's crew page: a card a shade lighter than the
/// page, so the page reads as panels and not one list.
const SECTION_FILL: egui::Color32 = egui::Color32::from_rgba_premultiplied(26, 28, 31, 235);
/// Under this width the crew page's two columns stack.
const TWO_COLUMNS_FROM: f32 = 660.0;

/// A screen's title along its top bar: big, spaced, upper case.
fn title_text(text: &str) -> egui::RichText {
    egui::RichText::new(text.to_uppercase())
        .size(22.0)
        .strong()
        .extra_letter_spacing(3.0)
        .color(theme::INK)
}

/// The setup's Start: gold, and bigger than anything else on the screen.
fn start_button(ui: &mut egui::Ui, enabled: bool) -> egui::Response {
    let text = egui::RichText::new("START")
        .size(20.0)
        .strong()
        .extra_letter_spacing(2.0)
        .color(if enabled {
            theme::PANEL_DEEP
        } else {
            theme::MUTED
        });
    let button = egui::Button::new(text).min_size(egui::vec2(170.0, 40.0));
    let button = if enabled { button.fill(GOLD) } else { button };
    ui.add_enabled(enabled, button)
}

/// The footer's line beside Start: the class, the ascension and the
/// money the run sets out with.
fn run_summary(ui: &mut egui::Ui, screen: &BuilderScreen, settings: &Settings) {
    ui.spacing_mut().item_spacing.x = 14.0;
    ui.label(
        egui::RichText::new(class_name(screen.bim_class))
            .size(16.0)
            .strong()
            .color(class_colour(screen.bim_class)),
    );
    ui.label(
        egui::RichText::new(format!(
            "{ASCENSION} {} · {}",
            settings.ascension,
            ascension_name(settings.ascension)
        ))
        .size(16.0)
        .color(if settings.ascension > 0 {
            GOLD
        } else {
            theme::MUTED
        }),
    );
    ui.label(
        egui::RichText::new(euros(settings.money_per_bim))
            .size(16.0)
            .color(theme::MONEY),
    );
    if settings.dev.any() {
        ui.label(
            egui::RichText::new(DEV_ON)
                .size(16.0)
                .strong()
                .color(theme::WARN),
        );
    }
}

/// A section's title: small, upper case, spaced, with a gold tick before
/// it.
fn section_title(ui: &mut egui::Ui, text: &str) -> egui::Response {
    ui.horizontal(|ui| {
        let (mark, _) = ui.allocate_exact_size(egui::vec2(3.0, 14.0), egui::Sense::hover());
        ui.painter().rect_filled(mark, 1.0, GOLD);
        ui.label(
            egui::RichText::new(text.to_uppercase())
                .size(13.0)
                .strong()
                .extra_letter_spacing(1.5)
                .color(theme::ACCENT),
        )
    })
    .inner
}

/// A card of the crew page, its title along the top.
pub(super) fn section(
    ui: &mut egui::Ui,
    title: &str,
    tip: Option<&str>,
    body: impl FnOnce(&mut egui::Ui),
) {
    egui::Frame::new()
        .fill(SECTION_FILL)
        .stroke(egui::Stroke::new(1.0, theme::LINE))
        .corner_radius(8.0)
        .inner_margin(12.0)
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            let title = section_title(ui, title);
            if let Some(tip) = tip {
                title.on_hover_text(tip);
            }
            ui.add_space(6.0);
            body(ui);
        });
    ui.add_space(8.0);
}

/// The settings tool: two tabs — the crew and the run, and the wave
/// formula's dials — shown on its own for a solo game and inside the
/// lobby. Both write to the same `Settings`.
fn tool(
    ui: &mut egui::Ui,
    screen: &mut BuilderScreen,
    settings: &mut Settings,
    online: &Online,
    editable: bool,
    now: f64,
) {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 22.0;
        for (tab, name) in [
            (SetupTab::Crew, SETUP_TAB_CREW),
            (SetupTab::Scaling, SETUP_TAB_SCALING),
            (SetupTab::Dev, SETUP_TAB_DEV),
        ] {
            if tab_button(ui, name, screen.tab == tab).clicked() {
                screen.tab = tab;
            }
        }
    });
    let line = ui.available_rect_before_wrap();
    ui.painter().hline(
        line.x_range(),
        line.top(),
        egui::Stroke::new(1.0, theme::LINE),
    );
    ui.add_space(10.0);
    // The rows run past a short window's foot, so they scroll.
    egui::ScrollArea::vertical()
        .id_salt("setup-rows")
        .auto_shrink([false, false])
        .show(ui, |ui| match screen.tab {
            SetupTab::Crew => crew_page(ui, screen, settings, online, editable),
            SetupTab::Dev => super::devtab::dev_page(ui, &mut settings.dev, editable),
            SetupTab::Scaling => {
                let players = (online.peers.len() as u32).max(1);
                difficulty_rows(
                    ui,
                    settings,
                    screen.file_scaling,
                    players,
                    editable,
                    &mut screen.difficulty_note,
                    now,
                );
            }
        });
    screen.net.push(online, settings, false);
}

/// One of the setup's tabs: its name, lit and underlined in gold while it
/// is up.
fn tab_button(ui: &mut egui::Ui, name: &str, on: bool) -> egui::Response {
    let colour = if on { theme::INK } else { theme::MUTED };
    let galley = ui.painter().layout_no_wrap(
        name.to_uppercase(),
        egui::FontId::proportional(17.0),
        colour,
    );
    let size = galley.size() + egui::vec2(4.0, 10.0);
    let (rect, response) = ui.allocate_exact_size(size, egui::Sense::click());
    let colour = if response.hovered() && !on {
        theme::ACCENT
    } else {
        colour
    };
    ui.painter()
        .galley(rect.min + egui::vec2(2.0, 2.0), galley, colour);
    if on {
        let bar = egui::Rect::from_min_max(
            egui::pos2(rect.left(), rect.bottom() - 3.0),
            rect.right_bottom(),
        );
        ui.painter().rect_filled(bar, 1.0, GOLD);
    }
    response.on_hover_cursor(egui::CursorIcon::PointingHand)
}

/// The Crew tab: the player's own Bim on the left — how it looks, what it
/// is called, its class — and the run on the right: the ascension, the
/// money, the auto-shoot. One column under each other where the card is
/// narrow.
fn crew_page(
    ui: &mut egui::Ui,
    screen: &mut BuilderScreen,
    settings: &mut Settings,
    online: &Online,
    editable: bool,
) {
    let bim = |ui: &mut egui::Ui, screen: &mut BuilderScreen, settings: &mut Settings| {
        section(ui, BIM_NAME, None, |ui| {
            bim_card(ui, screen, &online.tints_taken());
        });
        section(ui, BIM_CLASS, Some(BIM_CLASS_NOTE), |ui| {
            class_chooser(ui, screen, &settings.unlocks);
        });
    };
    let run = |ui: &mut egui::Ui, settings: &mut Settings| {
        section(ui, ASCENSION, None, |ui| {
            ascension_picker(ui, settings, editable);
        });
        section(ui, MONEY_PER_BIM, Some(MONEY_PER_BIM_TIP), |ui| {
            choice_row(
                ui,
                editable,
                &MONEY.map(|(label, amount)| (label, euros(amount), amount)),
                &mut settings.money_per_bim,
            );
        });
        // Auto-shoot: everybody's own, host or guest, like the name.
        section(ui, AUTO_SHOOT, Some(AUTO_SHOOT_NOTE), |ui| {
            choice_row(
                ui,
                true,
                &[
                    ("Off", AUTO_SHOOT_OFF.to_string(), false),
                    ("On", AUTO_SHOOT_ON.to_string(), true),
                ],
                &mut settings.auto_shoot,
            );
        });
    };
    if ui.available_width() >= TWO_COLUMNS_FROM {
        ui.columns(2, |columns| {
            let (left, right) = columns.split_at_mut(1);
            bim(&mut left[0], screen, settings);
            run(&mut right[0], settings);
        });
    } else {
        bim(ui, screen, settings);
        run(ui, settings);
    }
}

/// How big the chooser's portrait is, in points a side, and how far the
/// figure is zoomed: a body is some ninety units across at the room's
/// scale, and the room draws it at a fraction of that here, so the zoom
/// is what fills the box with it, hair and all.
const PORTRAIT_SIDE: f32 = 120.0;
const PORTRAIT_ZOOM: f32 = 1.8 * PORTRAIT_SIDE / 96.0;

/// The player's Bim (features 62 and 84): the figure as it will stand on
/// the deck beside its name, its hair's style and colour, and the colour
/// of the ring under it — everybody's to pick. The portrait is the room's
/// own drawing of the Bim (`character::portrait`), facing up the screen,
/// so what is chosen is what is seen — in the class's own kit (feature
/// 81), since the class chosen below it is worn on the deck. A colour
/// another player in the lobby has taken is greyed and dead: a colour is
/// one player's.
fn bim_card(ui: &mut egui::Ui, screen: &mut BuilderScreen, taken: &[Tint]) {
    ui.horizontal(|ui| {
        let (rect, _) = ui.allocate_exact_size(
            egui::vec2(PORTRAIT_SIDE, PORTRAIT_SIDE),
            egui::Sense::hover(),
        );
        ui.painter().rect_filled(rect, 8.0, theme::PANEL_DEEP);
        let (hair, shade) = screen.bim_hair;
        screen.portrait.clear();
        bims::character::portrait(
            Look::of(0).with_hair(hair, shade),
            screen.bim_class.outfit(),
            -std::f32::consts::FRAC_PI_2,
            &mut screen.portrait,
        );
        // The ring it will stand in on the deck, under the figure.
        let (r, g, b) = screen.bim_tint.rgb();
        let ring = egui::Color32::from_rgb((r * 255.0) as u8, (g * 255.0) as u8, (b * 255.0) as u8);
        ui.painter().circle_stroke(
            rect.center(),
            PORTRAIT_SIDE * 0.36,
            egui::Stroke::new(2.0, ring.gamma_multiply(0.8)),
        );
        paint_shapes(
            ui.painter(),
            rect_of(rect),
            View {
                scale: PORTRAIT_ZOOM,
                // A view is measured from the canvas, not the window.
                offset: Vec2::splat(PORTRAIT_SIDE / 2.0),
            },
            screen.portrait.data(),
        );
        ui.painter().rect_stroke(
            rect,
            8.0,
            egui::Stroke::new(1.0, class_colour(screen.bim_class).gamma_multiply(0.7)),
            egui::StrokeKind::Outside,
        );
        ui.add_space(6.0);
        ui.vertical(|ui| {
            ui.add(
                egui::TextEdit::singleline(&mut screen.bim_name)
                    .char_limit(wire::MAX_NAME)
                    .hint_text(BIM_NAME_HINT)
                    .font(egui::FontId::proportional(17.0))
                    .desired_width(ui.available_width().min(220.0)),
            )
            .on_hover_text(BIM_NAME_NOTE);
            ui.add_space(4.0);
            // The styles, a wrapping row of small buttons.
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing = egui::vec2(4.0, 4.0);
                for &style in &Hair::ALL {
                    let b = egui::Button::new(egui::RichText::new(hair_name(style)).small())
                        .min_size(egui::vec2(58.0, 20.0));
                    let b = if style == hair {
                        b.fill(theme::RAISED_ON)
                    } else {
                        b
                    };
                    if ui.add(b).on_hover_text(BIM_HAIR_NOTE).clicked() {
                        screen.bim_hair.0 = style;
                    }
                }
            });
            ui.add_space(2.0);
            // The hair's colours: a square swatch each, the picked one
            // ringed.
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing = egui::vec2(4.0, 4.0);
                for &tone in &Shade::ALL {
                    let (r, g, b) = tone.rgb();
                    let colour = egui::Color32::from_rgb(
                        (r * 255.0) as u8,
                        (g * 255.0) as u8,
                        (b * 255.0) as u8,
                    );
                    let (swatch, response) =
                        ui.allocate_exact_size(egui::vec2(18.0, 18.0), egui::Sense::click());
                    ui.painter().rect_filled(swatch, 3.0, colour);
                    ui.painter().rect_stroke(
                        swatch,
                        3.0,
                        egui::Stroke::new(1.0, theme::LINE),
                        egui::StrokeKind::Inside,
                    );
                    if tone == shade {
                        ui.painter().rect_stroke(
                            swatch,
                            3.0,
                            egui::Stroke::new(2.0, theme::INK),
                            egui::StrokeKind::Outside,
                        );
                    }
                    if response.on_hover_text(shade_name(tone)).clicked() {
                        screen.bim_hair.1 = tone;
                    }
                }
            });
            ui.add_space(2.0);
            // The ring's colours: a round swatch each.
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing = egui::vec2(4.0, 4.0);
                for &tint in &Tint::ALL {
                    let gone = taken.contains(&tint);
                    let (r, g, b) = tint.rgb();
                    let colour = egui::Color32::from_rgb(
                        (r * 255.0) as u8,
                        (g * 255.0) as u8,
                        (b * 255.0) as u8,
                    );
                    let colour = if gone {
                        colour.gamma_multiply(0.25)
                    } else {
                        colour
                    };
                    let (swatch, response) = ui.allocate_exact_size(
                        egui::vec2(20.0, 20.0),
                        if gone {
                            egui::Sense::hover()
                        } else {
                            egui::Sense::click()
                        },
                    );
                    ui.painter().circle_filled(swatch.center(), 8.0, colour);
                    if tint == screen.bim_tint {
                        ui.painter().circle_stroke(
                            swatch.center(),
                            9.5,
                            egui::Stroke::new(2.0, theme::INK),
                        );
                    }
                    if gone {
                        response.on_hover_text(BIM_TINT_TAKEN);
                    } else if response
                        .on_hover_text(format!("{BIM_TINT}: {}\n{BIM_TINT_NOTE}", tint_name(tint)))
                        .clicked()
                    {
                        screen.bim_tint = tint;
                    }
                }
            });
        });
    });
}

/// A class's own colour: its abilities' family (`ability_icons`), off its
/// ultimate's picture; the muted grey for none.
fn class_colour(class: Class) -> egui::Color32 {
    crate::ability_icons::Glyph::of(class, 3).map_or(theme::MUTED, |g| g.colour())
}

/// The class chooser (feature 74): a card a class — its ultimate's
/// picture and its name, ringed in its colour once picked — everybody's
/// to pick like the hair, with what the class does under the row. What is
/// picked goes with the slot at Start and onto the world as it opens;
/// playing, the crew panel's own picker changes it through
/// `Command::SetClass` until the first undock. Only the classes the
/// host's profile opened are offered (feature 106,
/// `profile::RunUnlocks`), and never the classless `Class::None`
/// (October 2026): a player always plays a class.
fn class_chooser(
    ui: &mut egui::Ui,
    screen: &mut BuilderScreen,
    unlocks: &crate::profile::RunUnlocks,
) {
    const CARD: egui::Vec2 = egui::vec2(74.0, 82.0);
    const ICON: f32 = 44.0;
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = egui::vec2(6.0, 6.0);
        for class in Class::ALL
            .into_iter()
            .filter(|&c| c != Class::None && unlocks.class_open(c))
        {
            let on = class == screen.bim_class;
            let colour = class_colour(class);
            let (rect, response) = ui.allocate_exact_size(CARD, egui::Sense::click());
            let hovered = response.hovered();
            let fill = if on {
                colour.gamma_multiply(0.22)
            } else if hovered {
                theme::RAISED_ON
            } else {
                theme::RAISED
            };
            ui.painter().rect_filled(rect, 6.0, fill);
            ui.painter().rect_stroke(
                rect,
                6.0,
                egui::Stroke::new(
                    if on { 2.0 } else { 1.0 },
                    if on { colour } else { theme::LINE },
                ),
                egui::StrokeKind::Inside,
            );
            if let Some(glyph) = crate::ability_icons::Glyph::of(class, 3) {
                let icon = egui::Rect::from_center_size(
                    egui::pos2(rect.center().x, rect.top() + 8.0 + ICON / 2.0),
                    egui::vec2(ICON, ICON),
                );
                crate::ability_icons::paint(ui.painter(), icon, glyph, on || hovered, 6.0);
            }
            ui.painter().text(
                egui::pos2(rect.center().x, rect.bottom() - 8.0),
                egui::Align2::CENTER_BOTTOM,
                class_name(class),
                egui::FontId::proportional(12.5),
                if on { theme::INK } else { theme::MUTED },
            );
            if response
                .on_hover_text(class_tip(class))
                .on_hover_cursor(egui::CursorIcon::PointingHand)
                .clicked()
            {
                screen.bim_class = class;
            }
        }
    });
    ui.add_space(6.0);
    ui.label(
        egui::RichText::new(class_tip(screen.bim_class))
            .small()
            .color(theme::MUTED),
    );
}

/// What the setup's class chooser starts on: `BIMS_CLASS`'s class
/// (`dev::bim_class`), else the soldier, since the chooser offers no
/// `Class::None` (October 2026).
fn setup_class() -> Class {
    match crate::dev::bim_class() {
        Class::None => Class::Soldier,
        class => class,
    }
}

/// How big an ascension's medallion is, a side.
const MEDALLION: f32 = 46.0;

/// The ascensions (October 2026, `world::ascension`): a medallion a
/// level, nought to the highest — the ones up to the pick lit gold, since
/// each holds every one under it, the ones the profile has not opened
/// dark and dead — and under them what the pick adds, a line a level. A
/// guest sees the host's pick and picks nothing.
fn ascension_picker(ui: &mut egui::Ui, settings: &mut Settings, editable: bool) {
    let most = world::ascension::MOST;
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = egui::vec2(6.0, 6.0);
        for level in 0..=most {
            let locked = editable && level > settings.ascension_open;
            let picked = level == settings.ascension;
            let lit = level <= settings.ascension;
            let (rect, response) = ui.allocate_exact_size(
                egui::vec2(MEDALLION, MEDALLION),
                if editable && !locked {
                    egui::Sense::click()
                } else {
                    egui::Sense::hover()
                },
            );
            let painter = ui.painter();
            let centre = rect.center();
            let radius = MEDALLION / 2.0 - 2.0;
            // A diamond: the four points of the compass round the centre.
            let diamond = |r: f32| {
                vec![
                    centre + egui::vec2(0.0, -r),
                    centre + egui::vec2(r, 0.0),
                    centre + egui::vec2(0.0, r),
                    centre + egui::vec2(-r, 0.0),
                ]
            };
            let (fill, edge, ink) = if locked {
                (theme::PANEL_DEEP, theme::LINE, theme::LINE)
            } else if picked {
                (
                    GOLD,
                    GOLD.lerp_to_gamma(egui::Color32::WHITE, 0.4),
                    theme::PANEL_DEEP,
                )
            } else if lit {
                (GOLD.gamma_multiply(0.25), GOLD, GOLD)
            } else if response.hovered() {
                (theme::RAISED_ON, theme::ACCENT, theme::INK)
            } else {
                (theme::RAISED, theme::LINE, theme::MUTED)
            };
            painter.add(egui::Shape::convex_polygon(
                diamond(radius),
                fill,
                egui::Stroke::new(if picked { 2.0 } else { 1.0 }, edge),
            ));
            if picked {
                painter.add(egui::Shape::closed_line(
                    diamond(radius + 3.0),
                    egui::Stroke::new(1.0, GOLD.gamma_multiply(0.6)),
                ));
            }
            painter.text(
                centre,
                egui::Align2::CENTER_CENTER,
                level.to_string(),
                egui::FontId::proportional(if picked { 20.0 } else { 17.0 }),
                ink,
            );
            let tip = if locked {
                format!("{}\n{}", ascension_name(level), ascension_locked(level))
            } else {
                format!("{}\n{}", ascension_name(level), ascension_line(level))
            };
            let response = response.on_hover_text(tip);
            if response.clicked() && editable && !locked {
                settings.ascension = level;
            }
        }
    });
    ui.add_space(8.0);
    let level = settings.ascension;
    ui.label(
        egui::RichText::new(ascension_name(level))
            .size(18.0)
            .strong()
            .color(if level > 0 { GOLD } else { theme::INK }),
    );
    if level == 0 {
        ui.label(egui::RichText::new(ascension_line(0)).color(theme::MUTED));
    }
    for each in 1..=level {
        ui.horizontal(|ui| {
            ui.label(
                egui::RichText::new(each.to_string())
                    .small()
                    .strong()
                    .color(GOLD),
            );
            ui.label(egui::RichText::new(ascension_line(each)).color(theme::INK));
        });
    }
    if editable && settings.ascension_open < most {
        ui.add_space(2.0);
        ui.label(
            egui::RichText::new(ascension_locked(settings.ascension_open + 1))
                .small()
                .color(theme::MUTED),
        );
    }
}

/// The most machines a count dial of the difficulty goes to.
const DIFFICULTY_MOST: u32 = 99;
/// The most days a day dial of the difficulty goes to.
const DIFFICULTY_DAYS_MOST: u32 = 365;
/// How far one press of − or + moves the enemies per bot.
const PER_BOT_STEP: f32 = 0.5;

/// The Scaling tab (task 147; the areas since October 2026): every dial
/// of the wave formula — enemies per player and per bot, then the four
/// areas' table — the host's to move. They show the tuning file's
/// (`file`) until one is moved; Default puts them back to it. Under them,
/// what the first wave comes to for the `players` here. A dial's note is
/// its name's tooltip.
fn difficulty_rows(
    ui: &mut egui::Ui,
    settings: &mut Settings,
    file: WaveScaling,
    players: u32,
    editable: bool,
    note: &mut Option<Remark>,
    now: f64,
) {
    // Both buttons only when the numbers are not the file's already.
    let apart = settings.difficulty.is_some_and(|d| d != file);
    ui.horizontal(|ui| {
        if ui
            .add_enabled(
                editable && apart,
                egui::Button::new(DIFFICULTY_RESET).small(),
            )
            .on_hover_text(DIFFICULTY_RESET_HOVER)
            .clicked()
        {
            settings.difficulty = None;
        }
        if ui
            .add_enabled(
                editable && apart,
                egui::Button::new(DIFFICULTY_SAVE).small(),
            )
            .on_hover_text(DIFFICULTY_SAVE_HOVER)
            .clicked()
            && let Some(d) = settings.difficulty
        {
            // The file is read again by its watcher, and the buttons go
            // grey once it says these numbers.
            *note = match crate::wavecfg::save_difficulty(d) {
                Ok(()) => Remark::say(DIFFICULTY_SAVED, false, now),
                Err(why) => Remark::say(difficulty_not_saved(&why), true, now),
            };
        }
        theme::question_mark(ui, DIFFICULTY_NOTE);
    });
    ui.add_space(6.0);
    let mut d: Difficulty = settings.difficulty.unwrap_or(file);
    egui::Grid::new("difficulty")
        .num_columns(2)
        .spacing(egui::vec2(10.0, 4.0))
        .show(ui, |ui| {
            count_row(
                ui,
                WAVE_PER_PLAYER,
                WAVE_PER_PLAYER_NOTE,
                &mut d.enemies_per_player,
                DIFFICULTY_MOST,
                editable,
            );
            bot_row(ui, &mut d.enemies_per_bot, editable);
        });
    ui.add_space(8.0);
    area_table(ui, &mut d, editable);
    ui.add_space(6.0);
    ui.label(
        egui::RichText::new(area_rows_line(
            d.tier_one_day(),
            d.tier_two_day(),
            d.tier_three_day(),
            d.heart_day(),
        ))
        .small()
        .color(theme::MUTED),
    );
    if editable && d != settings.difficulty.unwrap_or(file) {
        settings.difficulty = Some(d);
    }
    // The first wave as the world works it: day one's formula for the
    // players here, nobody defending.
    let first = d.size(players, 0, 1).max(1);
    ui.label(
        egui::RichText::new(first_wave_line(first, players))
            .small()
            .color(theme::ACCENT),
    );
    if note.is_some() {
        Remark::show(note, ui, now, "");
    }
}

/// A dial's name, its note on hover.
fn dial_name(ui: &mut egui::Ui, name: &str, note: &str) {
    ui.add(egui::Label::new(name).sense(egui::Sense::hover()))
        .on_hover_cursor(egui::CursorIcon::Help)
        .on_hover_text(note);
}

/// The four areas' dials (October 2026, `world::droid::Area`): a row a
/// dial, a column an area, a drag box a cell; area 0 has no machines and
/// no elite, so its cells for those are a dash.
fn area_table(ui: &mut egui::Ui, d: &mut Difficulty, editable: bool) {
    #[derive(Clone, Copy)]
    enum Dial {
        Days,
        Growth,
        Waves,
        Bombers,
        Lancers,
        Guardians,
        Elites,
        Defenders,
    }
    let rows = [
        (Dial::Days, AREA_DAYS, AREA_DAYS_NOTE),
        (Dial::Growth, AREA_GROWTH, AREA_GROWTH_NOTE),
        (Dial::Waves, AREA_WAVES, AREA_WAVES_NOTE),
        (Dial::Bombers, AREA_BOMBERS, AREA_EXTRAS_NOTE),
        (Dial::Lancers, AREA_LANCERS, AREA_EXTRAS_NOTE),
        (Dial::Guardians, AREA_GUARDIANS, AREA_GUARDIANS_NOTE),
        (Dial::Elites, AREA_ELITES, AREA_ELITES_NOTE),
        (Dial::Defenders, AREA_DEFENDERS, AREA_DEFENDERS_NOTE),
    ];
    egui::Grid::new("difficulty_areas")
        .num_columns(5)
        .spacing(egui::vec2(14.0, 4.0))
        .show(ui, |ui| {
            ui.label("");
            for name in AREA_NAMES {
                ui.label(egui::RichText::new(name).strong());
            }
            ui.end_row();
            for (dial, name, note) in rows {
                dial_name(ui, name, note);
                let areas = [
                    &mut d.area_0,
                    &mut d.tier_1_area,
                    &mut d.tier_2_area,
                    &mut d.tier_3_area,
                ];
                for (index, area) in areas.into_iter().enumerate() {
                    let count = |ui: &mut egui::Ui, value: &mut u32, most: u32| {
                        ui.add_enabled(
                            editable,
                            egui::DragValue::new(value).range(0..=most).speed(0.1),
                        );
                    };
                    let machines_or_elite = matches!(
                        dial,
                        Dial::Bombers | Dial::Lancers | Dial::Guardians | Dial::Elites
                    );
                    if index == 0 && machines_or_elite {
                        ui.label(egui::RichText::new(AREA_NONE).color(theme::MUTED));
                        continue;
                    }
                    match dial {
                        Dial::Days => count(ui, &mut area.days, DIFFICULTY_DAYS_MOST),
                        Dial::Growth => {
                            ui.add_enabled(
                                editable,
                                egui::DragValue::new(&mut area.growth_per_day)
                                    .range(0.0..=DIFFICULTY_MOST as f32)
                                    .speed(0.05)
                                    .max_decimals(2),
                            );
                        }
                        Dial::Waves => count(ui, &mut area.waves, DIFFICULTY_MOST),
                        Dial::Bombers => count(ui, &mut area.bombers, DIFFICULTY_MOST),
                        Dial::Lancers => count(ui, &mut area.lancers, DIFFICULTY_MOST),
                        Dial::Guardians => count(ui, &mut area.guardians, DIFFICULTY_MOST),
                        Dial::Elites => count(ui, &mut area.elites, DIFFICULTY_MOST),
                        Dial::Defenders => count(ui, &mut area.defenders, DIFFICULTY_MOST),
                    }
                }
                ui.end_row();
            }
        });
}

/// One whole-number dial of the difficulty: its name (its note on hover),
/// and a − and a + beside a drag box held to `0..=most`.
fn count_row(
    ui: &mut egui::Ui,
    name: &str,
    note: &str,
    value: &mut u32,
    most: u32,
    editable: bool,
) {
    dial_name(ui, name, note);
    ui.horizontal(|ui| {
        let less = ui.add_enabled(
            editable && *value > 0,
            egui::Button::new("-").min_size(egui::vec2(22.0, 22.0)),
        );
        if less.clicked() {
            *value -= 1;
        }
        ui.add_enabled(
            editable,
            egui::DragValue::new(value).range(0..=most).speed(0.1),
        );
        let more = ui.add_enabled(
            editable && *value < most,
            egui::Button::new("+").min_size(egui::vec2(22.0, 22.0)),
        );
        if more.clicked() {
            *value += 1;
        }
    });
    ui.end_row();
}

/// The enemies per bot, the one dial with decimals: − and + move it
/// by [`PER_BOT_STEP`], the drag box by hundredths.
fn bot_row(ui: &mut egui::Ui, value: &mut f32, editable: bool) {
    let most = DIFFICULTY_MOST as f32;
    dial_name(ui, WAVE_PER_BOT, WAVE_PER_BOT_NOTE);
    ui.horizontal(|ui| {
        let less = ui.add_enabled(
            editable && *value > 0.0,
            egui::Button::new("-").min_size(egui::vec2(22.0, 22.0)),
        );
        if less.clicked() {
            *value = (*value - PER_BOT_STEP).max(0.0);
        }
        ui.add_enabled(
            editable,
            egui::DragValue::new(value)
                .range(0.0..=most)
                .speed(0.05)
                .max_decimals(2),
        );
        let more = ui.add_enabled(
            editable && *value < most,
            egui::Button::new("+").min_size(egui::vec2(22.0, 22.0)),
        );
        if more.clicked() {
            *value = (*value + PER_BOT_STEP).min(most);
        }
    });
    ui.end_row();
}

/// A row of mutually exclusive choices, each one a number written into the
/// setting. A guest in somebody else's lobby watches the settings rather
/// than setting them.
fn choice_row<T: PartialEq + Copy>(
    ui: &mut egui::Ui,
    editable: bool,
    options: &[(&str, String, T)],
    value: &mut T,
) {
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = egui::vec2(6.0, 6.0);
        let width = ((ui.available_width() - 10.0 * options.len() as f32) / options.len() as f32)
            .clamp(80.0, 130.0);
        for (label, sub, v) in options {
            let on = *value == *v;
            let mut job = egui::text::LayoutJob::default();
            let line = |size: f32, colour: egui::Color32| egui::TextFormat {
                font_id: egui::FontId::proportional(size),
                color: colour,
                ..Default::default()
            };
            job.append(
                label,
                0.0,
                line(15.0, if on { theme::INK } else { theme::ACCENT }),
            );
            job.append("\n", 0.0, line(6.0, theme::MUTED));
            // A sum among the options (Starting money) in the money's gold.
            let sub_colour = if sub.contains('€') {
                theme::MONEY
            } else {
                theme::MUTED
            };
            job.append(sub, 0.0, line(12.5, sub_colour));
            let response = ui.add_enabled(editable, {
                let b = egui::Button::new(job)
                    .min_size(egui::vec2(width, 44.0))
                    .corner_radius(6.0);
                if on {
                    b.fill(theme::RAISED_ON)
                        .stroke(egui::Stroke::new(1.5, GOLD))
                } else {
                    b
                }
            });
            if response.clicked() && editable {
                *value = *v;
            }
        }
    });
}

/// The settings' seed or galaxy type moved: a different galaxy, and
/// nothing chosen in the old one means anything in it. The caller pushes.
fn new_world(screen: &mut BuilderScreen, settings: &mut Settings) {
    settings.spawn = None;
    screen
        .lobby
        .set_world(settings.seed, ship::session::galaxy_type(settings.galaxy));
    screen.lobby.spawn = None;
}

/// Everything at random: a new seed, a galaxy of any shape, and a station
/// anywhere in it. Every game is opened so — the player picks none of the
/// three — and the caller pushes, where there is a room to push to.
fn roll_everything(screen: &mut BuilderScreen, settings: &mut Settings) {
    // The profile read again, since a run won since the last setup may
    // have opened an ascension: the highest reached is the one picked.
    let profile = crate::profile::load();
    settings.unlocks = crate::profile::RunUnlocks::of(&profile);
    settings.ascension_open = crate::profile::ascension_open();
    settings.ascension = crate::profile::ascension_reached();
    screen.tab = SetupTab::Crew;
    roll_galaxy(settings);
    new_world(screen, settings);
    roll_start(screen, settings);
}

/// A new seed and a galaxy of any shape, on the settings alone.
fn roll_galaxy(settings: &mut Settings) {
    let roll = crate::screens::rand_seed();
    settings.seed = crate::screens::rand_seed();
    settings.galaxy = (roll % GALAXIES.len() as u64) as u32;
}

/// A random station among every star that has one a crew can start at:
/// the lobby's rule (`Lobby::random_start`), which skips the hostile
/// ones — a crew cannot start at an enemy's — off this page's roll.
fn roll_start(screen: &mut BuilderScreen, settings: &mut Settings) {
    let roll = crate::screens::rand_seed();
    let Some((star, station)) = screen.lobby.random_start(roll) else {
        return;
    };
    settings.spawn = Some((star, station));
    screen.lobby.spawn = settings.spawn;
}

/// [`roll_start`], and the room told.
fn pick_random_start(screen: &mut BuilderScreen, settings: &mut Settings, online: &Online) {
    roll_start(screen, settings);
    screen.net.push(online, settings, false);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_room_code_is_six_readable_letters() {
        // The relay deals them (`wire`); the field takes exactly that many.
        {
            assert_eq!(CODE_LENGTH, 6);
            assert!(wire::is_code("Q7FKAB"));
            assert!(!wire::is_code("O0I1AB"));
            assert!(!wire::is_code("ABCDE"));
        }
    }
}
