//! The front of the game: the start menu, the setup screen, and the lobby.
//!
//! The first screens a player meets, and deliberately separate from the
//! room and the designer: the builder deals in settings and has no `Game`,
//! and the room deals in a `Game` and has no menus. Only the World tab
//! touches the galaxy, through `crates/lobby`.
//!
//! The multiplayer half (feature 59) is `crate::net`: the lobby is a room
//! at the relay, the host's settings go to everybody in it, a guest can
//! point at a star, and the host's Start takes the whole room into the
//! yard. The screen reads the room — who is in it, whose it is — off the
//! `Online` resource and never past it, which is what made the wire a
//! change to that one object rather than to the screens.
//!
//! What the settings come out as, and the only things that leave this
//! screen, are plain numbers: a count of euros, a tile count, a seed, a
//! type and two ids. Nothing but numbers cross into the simulation.

use bevy::prelude::*;
use bevy_egui::{EguiContexts, EguiPrimaryContextPass, egui};
use bims::character::{Hair, Look, Shade, Tint};
use lobby::{Lobby, NONE};
use wire::To;
use world::Class;

use crate::canvas::{paint_shapes, rect_of, root_ui};
use crate::format::{euros, roman};
use crate::names::*;
use crate::net::{Event, Online, Packet, SettingsWire};
use crate::shapes::View;
use crate::{Screen, theme};

/// What each of the crew brings, in whole euros. It all goes into one pool.
const MONEY: [(&str, u64); 3] = [("Lean", 50_000), ("Standard", 100_000), ("Full", 200_000)];

/// Starting ship, in tiles a side.
const SHIPS: [(&str, u32, &str); 3] = [
    ("Small", 30, "30 × 30"),
    ("Standard", 40, "40 × 40"),
    ("Large", 60, "60 × 60"),
];

/// The shape of the galaxy, by `worldgen::GalaxyType`.
const GALAXIES: [(&str, &str); 4] = [
    ("Two-arm spiral", "Two arms, easy to read"),
    ("Spiral", "Four arms, busier"),
    ("Elliptical", "A squashed cloud"),
    ("Round", "Dense in the middle"),
];

const DEFAULT_MONEY: u64 = 100_000;
const DEFAULT_SHIP: u32 = 40;
const DEFAULT_GALAXY: u32 = 0;

/// Berths in a lobby. Four is a guess at the eventual crew ceiling.
const LOBBY_SLOTS: usize = 4;

/// Room codes are read out loud down a phone line, so the alphabet leaves
/// out the pairs that are heard wrong: O/0, I/1. The relay deals them
/// (`wire::CODE_ALPHABET`); the field takes as many letters as one is.
const CODE_LENGTH: usize = wire::CODE_LENGTH;

/// How long a passing remark stays on screen, in seconds.
const NOTE_SECONDS: f64 = 4.0;

/// A press that moves less than this before it lets go is a click on a
/// star, not a drag of the map.
const DRAG_SLOP: f32 = 4.0;

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
    pub ship: u32,
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
}

impl Default for Settings {
    fn default() -> Settings {
        Settings {
            money_per_bim: DEFAULT_MONEY,
            ship: DEFAULT_SHIP,
            seed: crate::screens::room::rand_seed(),
            galaxy: DEFAULT_GALAXY,
            spawn: None,
            players: 1,
            slot: 0,
            names: Vec::new(),
            hair: Vec::new(),
            classes: Vec::new(),
            tints: Vec::new(),
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
            online.send(To::All, &Packet::Settings(now));
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Tab {
    Setup,
    World,
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
    tab: Tab,
    seed_text: String,
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
    world_note: Option<Remark>,
    lobby_said: Option<Remark>,
    /// Who was in the lobby last frame, so a roster change can say who
    /// came or went.
    seen: Vec<wire::PeerId>,
    /// `BIMS_AUTO` has pressed Start; once.
    auto_done: bool,
    /// The galaxy: built once, moved between the two tools. There is one
    /// galaxy, one camera over it and one canvas.
    lobby: Lobby,
    inspected: Option<u32>,
    /// A press on the preview: where it started, and whether it has become
    /// a drag.
    pressed: Option<egui::Pos2>,
    dragging: bool,
    preview_size: Vec2,
    system_list: lobby::draw::DrawList,
    galaxy_list: lobby::draw::DrawList,
    /// The Load window over the menu, if it is up, and its page's state —
    /// `crate::save`. A saved game is picked up from here without the
    /// setup: the session is stood up round it and the game screen opens.
    loading: bool,
    saves: crate::save::Saves,
}

pub struct BuilderPlugin;

impl Plugin for BuilderPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Settings>()
            .add_systems(
                Startup,
                open.run_if(|l: Res<crate::Launch>| *l == crate::Launch::Game),
            )
            .add_systems(
                EguiPrimaryContextPass,
                frame.run_if(|s: Res<State<Screen>>| {
                    matches!(s.get(), Screen::Menu | Screen::Setup | Screen::Lobby)
                }),
            );
    }
}

fn open(mut commands: Commands, settings: Res<Settings>) {
    let lobby = Lobby::new(
        settings.seed,
        ship::session::galaxy_type(settings.galaxy),
        560.0,
        400.0,
    );
    commands.insert_resource(BuilderScreen {
        net: Net::default(),
        tab: Tab::Setup,
        seed_text: settings.seed.to_string(),
        join_code: String::new(),
        bim_name: crate::dev::bim_name(),
        said_bim_name: None,
        bim_hair: crate::dev::bim_hair(),
        bim_tint: crate::dev::bim_tint(),
        said_bim_tint: None,
        said_bim_hair: None,
        bim_class: crate::dev::bim_class(),
        said_bim_class: None,
        portrait: bims::draw::DrawList::new(),
        join_note: None,
        world_note: None,
        lobby_said: None,
        seen: Vec::new(),
        auto_done: false,
        lobby,
        inspected: None,
        pressed: None,
        dragging: false,
        preview_size: Vec2::new(560.0, 400.0),
        system_list: lobby::draw::DrawList::new(),
        galaxy_list: lobby::draw::DrawList::new(),
        loading: false,
        saves: crate::save::Saves::default(),
    });
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
) -> Result {
    let ctx = contexts.ctx_mut()?.clone();
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
        {
            if settings.spawn.is_none() {
                pick_random_start(screen, settings, online);
            }
            start = true;
            screen.auto_done = true;
        }
    }

    // What the room said since last frame: the relay, and the others in
    // it. A guest's settings and its Start come from the host this way;
    // a refusal or a closed room is a line and the menu again.
    for event in online.drain(now) {
        match event {
            Event::Connected => {}
            Event::Joined => {
                screen.seen = online.peers.iter().map(|p| p.id).collect();
                if crate::dev::auto().is_some()
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
            }
            Event::Roster => {
                // A joiner has no settings yet; the host says them again.
                screen.net.push(online, settings, true);
                // Nor anybody's Bim's name: everybody says theirs again.
                screen.said_bim_name = None;
                screen.said_bim_hair = None;
                screen.said_bim_tint = None;
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
                        screen.seed_text = settings.seed.to_string();
                        screen.inspected = None;
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
                    // And its class, the same way.
                    settings.classes = classes
                        .into_iter()
                        .map(|c| Class::from_code(c).unwrap_or_default())
                        .collect();
                    if settings.classes.len() <= mine {
                        settings.classes.resize(mine + 1, Class::None);
                    }
                    settings.classes[mine] = screen.bim_class;
                    // And its colour, the same way (feature 84).
                    settings.tints = tints.into_iter().map(Tint::from_code).collect();
                    if settings.tints.len() <= mine {
                        for s in settings.tints.len()..=mine {
                            settings.tints.push(Tint::ALL[s % Tint::ALL.len()]);
                        }
                    }
                    settings.tints[mine] = screen.bim_tint;
                    commands.insert_resource(crate::screens::designer::Start(settings.clone()));
                    go = Some(Screen::Design);
                }
                // The host loaded a game: its world, whole, is this end's
                // now, and the game screen opens round it as it does for
                // a load here (feature 67). Nothing before Start deals
                // the slots, so the seat is the one `Online::deal` would
                // deal: this player's place in join order.
                Packet::World { save, .. } if Some(from) == online.host && !online.is_host() => {
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
                        Ok(loaded) => {
                            commands.insert_resource(crate::screens::designer::ShipSession(loaded));
                            screen.loading = false;
                            go = Some(Screen::Game);
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
            screen.said_bim_class = Some(screen.bim_class);
        }
    }

    match state.get() {
        Screen::Menu => {
            egui::CentralPanel::default().show(&mut root, |ui| {
                ui.vertical_centered(|ui| {
                    ui.add_space(ui.available_height() * 0.25);
                    ui.label(egui::RichText::new("Bims").size(40.0).strong());
                    ui.label(
                        egui::RichText::new("A ship, a crew, and whatever they can grow.")
                            .color(theme::MUTED),
                    );
                    ui.add_space(20.0);
                    ui.horizontal(|ui| {
                        ui.add_space((ui.available_width() - 390.0).max(0.0) / 2.0);
                        if theme::big(ui, "Play", true).clicked() {
                            go = Some(Screen::Setup);
                        }
                        if theme::big(ui, "Create lobby", true).clicked() {
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
                        ui.label(egui::RichText::new("Join with a code").color(theme::MUTED));
                        let field = ui.add(
                            egui::TextEdit::singleline(&mut screen.join_code)
                                .char_limit(CODE_LENGTH)
                                .hint_text("——————")
                                .desired_width(90.0),
                        );
                        let submitted =
                            field.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
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
                            commands.insert_resource(crate::screens::designer::ShipSession(loaded));
                            screen.loading = false;
                            go = Some(Screen::Game);
                        }
                        Err(why) => screen.saves.failed(why),
                    }
                }
            }
        }
        Screen::Setup => {
            egui::Panel::top("setup-head").show(&mut root, |ui| {
                ui.horizontal(|ui| {
                    if ui.button("< Back").clicked() {
                        go = Some(Screen::Menu);
                    }
                    ui.heading("Game setup");
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(egui::RichText::new("One crew, no lobby").color(theme::MUTED));
                    });
                });
            });
            egui::Panel::bottom("setup-foot").show(&mut root, |ui| {
                ui.horizontal(|ui| {
                    let why = start_refusal(settings);
                    ui.label(egui::RichText::new(why.unwrap_or("")).color(theme::CAUTION));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if theme::big(ui, "Start", why.is_none()).clicked() {
                            start = true;
                        }
                    });
                });
            });
            egui::CentralPanel::default().show(&mut root, |ui| {
                tool(ui, screen, settings, online, true, now);
            });
        }
        Screen::Lobby => {
            egui::Panel::top("lobby-head").show(&mut root, |ui| {
                ui.horizontal(|ui| {
                    if ui.button("< Leave").clicked() {
                        online.leave();
                        go = Some(Screen::Menu);
                    }
                    ui.heading("Lobby");
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

    if start && lobby_start_refusal(settings, online).is_none() {
        // Start hands the game over to the designer. What crosses is the
        // numbers and nothing else: the money each Bim brings, the build
        // area in tiles, how many players there are, which slot you are,
        // the seed, the galaxy type, and the star and station the game
        // starts at.
        // With company, the crew are dealt in join order — the host slot 0
        // — and everybody is told: the same Start on every machine.
        if online.is_online() {
            online.begin();
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
                    settings: SettingsWire::of(settings),
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
            settings.names = vec![wire::tidy_name(&screen.bim_name)];
            settings.hair = vec![screen.bim_hair];
            settings.classes = vec![screen.bim_class];
            settings.tints = vec![screen.bim_tint];
        }
        commands.insert_resource(crate::screens::designer::Start(settings.clone()));
        go = Some(Screen::Design);
    }
    if let Some(screen) = go {
        next.set(screen);
    }
    Ok(())
}

/// Why Start cannot be pressed, or `None` if it can.
fn start_refusal(settings: &Settings) -> Option<&'static str> {
    if settings.spawn.is_none() {
        return Some("Pick a station to start at on the World tab.");
    }
    None
}

/// The same in a lobby: a station picked, the room reached, and the host
/// the one pressing — a guest waits for the host's Start.
fn lobby_start_refusal(settings: &Settings, online: &Online) -> Option<&'static str> {
    if online.connecting() {
        return Some(CONNECTING);
    }
    if online.is_online() && !online.is_host() {
        return Some("The host starts the game.");
    }
    start_refusal(settings)
}

/// The settings tool: one tabbed panel, shown on its own for a solo game
/// and inside the lobby. Both write to the same `Settings`.
fn tool(
    ui: &mut egui::Ui,
    screen: &mut BuilderScreen,
    settings: &mut Settings,
    online: &Online,
    editable: bool,
    now: f64,
) {
    ui.horizontal(|ui| {
        for (tab, label) in [(Tab::Setup, "Game setup"), (Tab::World, "World")] {
            if theme::toggle(ui, screen.tab == tab, label).clicked() {
                screen.tab = tab;
            }
        }
    });
    ui.separator();
    match screen.tab {
        Tab::Setup => {
            choice_row(
                ui,
                "Money per Bim",
                "What each of you brings; it all goes into one pool",
                editable,
                &MONEY.map(|(label, amount)| (label, euros(amount), amount)),
                &mut settings.money_per_bim,
            );
            choice_row(
                ui,
                "Ship size",
                "Tiles a side",
                editable,
                &SHIPS.map(|(label, tiles, sub)| (label, sub.to_string(), tiles)),
                &mut settings.ship,
            );
            // The player's own crew member's name: everybody's to type,
            // host or guest, since each names their own.
            ui.add_space(6.0);
            ui.label(egui::RichText::new(BIM_NAME).strong());
            ui.label(
                egui::RichText::new(BIM_NAME_NOTE)
                    .small()
                    .color(theme::MUTED),
            );
            ui.add(
                egui::TextEdit::singleline(&mut screen.bim_name)
                    .char_limit(wire::MAX_NAME)
                    .hint_text(BIM_NAME_HINT)
                    .desired_width(180.0),
            );
            hair_chooser(ui, screen);
            tint_chooser(ui, screen, &online.tints_taken());
            class_chooser(ui, screen);
            screen.net.push(online, settings, false);
        }
        Tab::World => world(ui, screen, settings, online, editable, now),
    }
}

/// How big the chooser's portrait is, in points a side, and how far the
/// figure is zoomed: a body is some ninety units across at the room's
/// scale, so nine tenths fills the box with it, hair and all.
const PORTRAIT_SIDE: f32 = 96.0;
const PORTRAIT_ZOOM: f32 = 0.9;

/// The hair chooser (feature 62): the figure as it will stand on the
/// deck, a button a style and a swatch a colour, everybody's to pick like
/// the name. The portrait is the room's own drawing of the Bim
/// (`character::portrait`), facing up the screen, so what is chosen is
/// what is seen — in the class's own kit (feature 81), since the class
/// chosen below it is worn on the deck.
fn hair_chooser(ui: &mut egui::Ui, screen: &mut BuilderScreen) {
    ui.add_space(6.0);
    ui.label(egui::RichText::new(BIM_HAIR).strong());
    ui.label(
        egui::RichText::new(BIM_HAIR_NOTE)
            .small()
            .color(theme::MUTED),
    );
    ui.horizontal(|ui| {
        let (rect, _) = ui.allocate_exact_size(
            egui::vec2(PORTRAIT_SIDE, PORTRAIT_SIDE),
            egui::Sense::hover(),
        );
        ui.painter().rect_filled(rect, 4.0, theme::RAISED);
        let (hair, shade) = screen.bim_hair;
        screen.portrait.clear();
        bims::character::portrait(
            Look::of(0).with_hair(hair, shade),
            screen.bim_class.outfit(),
            -std::f32::consts::FRAC_PI_2,
            &mut screen.portrait,
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
            4.0,
            egui::Stroke::new(1.0, theme::LINE),
            egui::StrokeKind::Outside,
        );
        ui.vertical(|ui| {
            // The styles, four to a row.
            for row in Hair::ALL.chunks(4) {
                ui.horizontal(|ui| {
                    for &style in row {
                        let on = style == hair;
                        let b =
                            egui::Button::new(hair_name(style)).min_size(egui::vec2(76.0, 22.0));
                        let b = if on { b.fill(theme::RAISED_ON) } else { b };
                        if ui.add(b).clicked() {
                            screen.bim_hair.0 = style;
                        }
                    }
                });
            }
            // The colours: a swatch each, the picked one ringed.
            ui.horizontal(|ui| {
                for &tone in &Shade::ALL {
                    let (r, g, b) = tone.rgb();
                    let colour = egui::Color32::from_rgb(
                        (r * 255.0) as u8,
                        (g * 255.0) as u8,
                        (b * 255.0) as u8,
                    );
                    let (swatch, response) =
                        ui.allocate_exact_size(egui::vec2(22.0, 22.0), egui::Sense::click());
                    ui.painter().rect_filled(swatch, 3.0, colour);
                    if tone == shade {
                        ui.painter().rect_stroke(
                            swatch,
                            3.0,
                            egui::Stroke::new(2.0, theme::ACCENT),
                            egui::StrokeKind::Outside,
                        );
                    }
                    if response.clicked() {
                        screen.bim_hair.1 = tone;
                    }
                }
                ui.label(egui::RichText::new(shade_name(shade)).color(theme::MUTED));
            });
        });
    });
}

/// The colour chooser (feature 84): a swatch a colour, the picked one
/// ringed, and one another player in the lobby has taken greyed and
/// dead — a colour is one player's, which is what makes it worth
/// drawing under their Bim at all. Solo there is nobody to clash with
/// and every swatch is live.
fn tint_chooser(ui: &mut egui::Ui, screen: &mut BuilderScreen, taken: &[Tint]) {
    ui.add_space(6.0);
    ui.label(egui::RichText::new(BIM_TINT).strong());
    ui.label(
        egui::RichText::new(BIM_TINT_NOTE)
            .small()
            .color(theme::MUTED),
    );
    ui.horizontal(|ui| {
        for &tint in &Tint::ALL {
            let gone = taken.contains(&tint);
            let (r, g, b) = tint.rgb();
            let colour =
                egui::Color32::from_rgb((r * 255.0) as u8, (g * 255.0) as u8, (b * 255.0) as u8);
            let colour = if gone {
                colour.gamma_multiply(0.25)
            } else {
                colour
            };
            let (swatch, response) = ui.allocate_exact_size(
                egui::vec2(24.0, 24.0),
                if gone {
                    egui::Sense::hover()
                } else {
                    egui::Sense::click()
                },
            );
            ui.painter().circle_filled(swatch.center(), 10.0, colour);
            if tint == screen.bim_tint {
                ui.painter().circle_stroke(
                    swatch.center(),
                    11.5,
                    egui::Stroke::new(2.0, theme::INK),
                );
            }
            if gone {
                response.on_hover_text(BIM_TINT_TAKEN);
            } else if response.clicked() {
                screen.bim_tint = tint;
            }
        }
        ui.label(egui::RichText::new(tint_name(screen.bim_tint)).color(theme::MUTED));
    });
}

/// The class chooser (feature 74): a button a class, everybody's to pick
/// like the hair, with a line under it saying what the class does and
/// what it brings to the pool. What is picked goes with the slot at
/// Start and onto the world as it opens; playing, the crew panel's
/// own picker changes it through `Command::SetClass` until the first
/// undock.
fn class_chooser(ui: &mut egui::Ui, screen: &mut BuilderScreen) {
    ui.add_space(6.0);
    ui.label(egui::RichText::new(BIM_CLASS).strong());
    ui.label(
        egui::RichText::new(BIM_CLASS_NOTE)
            .small()
            .color(theme::MUTED),
    );
    ui.horizontal(|ui| {
        for class in Class::ALL {
            let on = class == screen.bim_class;
            let b = egui::Button::new(class_name(class)).min_size(egui::vec2(96.0, 22.0));
            let b = if on { b.fill(theme::RAISED_ON) } else { b };
            if ui.add(b).clicked() {
                screen.bim_class = class;
            }
        }
    });
    ui.label(
        egui::RichText::new(class_tip(screen.bim_class))
            .small()
            .color(theme::MUTED),
    );
}

/// A row of mutually exclusive choices, each one a number written into the
/// setting. A guest in somebody else's lobby watches the settings rather
/// than setting them.
fn choice_row<T: PartialEq + Copy>(
    ui: &mut egui::Ui,
    name: &str,
    note: &str,
    editable: bool,
    options: &[(&str, String, T)],
    value: &mut T,
) {
    ui.add_space(6.0);
    ui.label(egui::RichText::new(name).strong());
    ui.label(egui::RichText::new(note).small().color(theme::MUTED));
    ui.horizontal(|ui| {
        for (label, sub, v) in options {
            let on = *value == *v;
            let text = format!("{label}\n{sub}");
            let response = ui.add_enabled(editable, {
                let b = egui::Button::new(text).min_size(egui::vec2(120.0, 40.0));
                if on { b.fill(theme::RAISED_ON) } else { b }
            });
            if response.clicked() && editable {
                *value = *v;
            }
        }
    });
}

/// The World tab: the seed, the galaxy type, the galaxy itself, and the
/// system beside it. A start is chosen here, not explored: no distances,
/// no travel times, nothing about what a station is like.
fn world(
    ui: &mut egui::Ui,
    screen: &mut BuilderScreen,
    settings: &mut Settings,
    online: &Online,
    editable: bool,
    now: f64,
) {
    // The header names the start, or says there is none yet.
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new("Start at").strong());
        match settings.spawn.and_then(|(star, station)| {
            place_name(&mut screen.lobby, screen.inspected, star, station)
        }) {
            Some(name) => ui.label(egui::RichText::new(name).color(theme::ACCENT)),
            None => {
                ui.label(egui::RichText::new("nowhere yet — pick a station").color(theme::MUTED))
            }
        };
    });

    // The seed: a decimal field for a u64.
    ui.add_space(4.0);
    ui.label(egui::RichText::new("Seed").strong());
    ui.label(
        egui::RichText::new("A whole number; the same one is the same galaxy")
            .small()
            .color(theme::MUTED),
    );
    let mut new_seed: Option<u64> = None;
    ui.horizontal(|ui| {
        let field = ui.add_enabled(
            editable,
            egui::TextEdit::singleline(&mut screen.seed_text).desired_width(220.0),
        );
        let submitted = field.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
        if ui.add_enabled(editable, egui::Button::new("Set")).clicked() || submitted {
            match parse_seed(&screen.seed_text) {
                Some(seed) => new_seed = Some(seed),
                None => {
                    screen.world_note = Remark::say(
                        "That is not a seed: a whole number, up to twenty digits.",
                        true,
                        now,
                    );
                    screen.seed_text = settings.seed.to_string();
                }
            }
        }
        if ui
            .add_enabled(editable, egui::Button::new("New seed"))
            .clicked()
        {
            new_seed = Some(crate::screens::room::rand_seed());
        }
    });

    // The galaxy type.
    ui.add_space(4.0);
    ui.label(egui::RichText::new("Galaxy").strong());
    ui.label(
        egui::RichText::new("Its shape; a thousand stars either way")
            .small()
            .color(theme::MUTED),
    );
    let mut new_galaxy: Option<u32> = None;
    ui.horizontal(|ui| {
        for (i, (label, sub)) in GALAXIES.iter().enumerate() {
            let on = settings.galaxy == i as u32;
            let response = ui.add_enabled(editable, {
                let b =
                    egui::Button::new(format!("{label}\n{sub}")).min_size(egui::vec2(120.0, 40.0));
                if on { b.fill(theme::RAISED_ON) } else { b }
            });
            if response.clicked() && !on {
                new_galaxy = Some(i as u32);
            }
        }
    });

    // The seed or the type moved: a different galaxy, and nothing chosen
    // in the old one means anything in it.
    let mut changed = false;
    if let Some(seed) = new_seed
        && seed != settings.seed
    {
        settings.seed = seed;
        changed = true;
    }
    if let Some(galaxy) = new_galaxy {
        settings.galaxy = galaxy;
        changed = true;
    }
    if changed {
        screen.seed_text = settings.seed.to_string();
        settings.spawn = None;
        screen.inspected = None;
        screen
            .lobby
            .set_world(settings.seed, ship::session::galaxy_type(settings.galaxy));
        screen.lobby.spawn = None;
        screen.net.push(online, settings, false);
    }

    ui.label(
        egui::RichText::new("Drag to pan, scroll to zoom, click a star to look at its system. Dim stars have no station.")
            .small()
            .color(theme::MUTED),
    );

    // The preview on the left, the system on the right.
    ui.add_space(4.0);
    let total = ui.available_size();
    let card_w = 300.0_f32.min(total.x * 0.4);
    let preview_w = (total.x - card_w - 12.0).max(200.0);
    let preview_h = (total.y - 50.0).clamp(200.0, 600.0);
    ui.horizontal_top(|ui| {
        ui.vertical(|ui| {
            ui.set_width(preview_w);
            let (rect, response) = ui.allocate_exact_size(
                egui::vec2(preview_w, preview_h),
                egui::Sense::click_and_drag(),
            );
            preview(ui, screen, settings, rect, &response);
            ui.horizontal(|ui| {
                let said = match screen.lobby.hovered {
                    Some(star) => {
                        let s = screen.lobby.galaxy.star(star);
                        s.map(|s| {
                            format!(
                                "{} · class {} · {}",
                                star_name(s.name),
                                STAR_CLASS_NAMES
                                    .get(s.star_class as usize)
                                    .copied()
                                    .unwrap_or("?"),
                                if screen
                                    .lobby
                                    .has_station
                                    .get(star as usize)
                                    .copied()
                                    .unwrap_or(false)
                                {
                                    "has a station"
                                } else {
                                    "no station"
                                }
                            )
                        })
                        .unwrap_or_default()
                    }
                    None => String::new(),
                };
                ui.label(egui::RichText::new(said).small().color(theme::MUTED));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui
                        .add_enabled(editable, egui::Button::new("Random start"))
                        .clicked()
                    {
                        pick_random_start(screen, settings, online);
                    }
                });
            });
            Remark::show(&mut screen.world_note, ui, now, "");
        });
        ui.vertical(|ui| {
            ui.set_width(card_w);
            system_card(ui, screen, settings, online, editable, card_w);
        });
    });
}

/// The galaxy, painted into `rect`: pan by dragging, zoom with the wheel,
/// click a star to open it.
fn preview(
    ui: &egui::Ui,
    screen: &mut BuilderScreen,
    settings: &Settings,
    rect: egui::Rect,
    response: &egui::Response,
) {
    let size = Vec2::new(rect.width(), rect.height());
    if size != screen.preview_size {
        screen.preview_size = size;
        screen.lobby.preview.resize(size.x, size.y);
    }
    let local = |p: egui::Pos2| (p.x - rect.min.x, p.y - rect.min.y);
    if response.drag_started_by(egui::PointerButton::Primary)
        || response.drag_started_by(egui::PointerButton::Middle)
    {
        screen.pressed = response.interact_pointer_pos();
        screen.dragging = response.drag_started_by(egui::PointerButton::Middle);
    }
    if response.dragged() {
        let delta = response.drag_delta();
        let moved = screen
            .pressed
            .zip(response.interact_pointer_pos())
            .is_some_and(|(a, b)| (a - b).length() > DRAG_SLOP);
        if screen.dragging || moved {
            screen.dragging = true;
            screen.lobby.preview.pan(delta.x, delta.y);
        }
    }
    if let Some(p) = response.hover_pos() {
        let (x, y) = local(p);
        screen.lobby.hover(x, y);
        let scroll = response.ctx.input(|i| i.smooth_scroll_delta.y);
        if scroll != 0.0 {
            screen
                .lobby
                .preview
                .zoom(x, y, crate::canvas::zoom_factor(scroll));
            screen.lobby.hover(x, y);
        }
    } else if !response.dragged() {
        screen.lobby.hovered = None;
    }
    let was_click = response.clicked() || (response.drag_stopped() && !screen.dragging);
    if response.drag_stopped() || response.clicked() {
        if was_click && let Some(p) = response.interact_pointer_pos() {
            let (x, y) = local(p);
            screen.lobby.hover(x, y);
            if let Some(star) = screen.lobby.hovered {
                inspect(screen, star);
            }
        }
        screen.pressed = None;
        screen.dragging = false;
    }
    screen.lobby.spawn = settings.spawn;
    screen.lobby.paint(&mut screen.galaxy_list);
    paint_shapes(
        ui.painter(),
        rect_of(rect),
        View::PIXELS,
        screen.galaxy_list.shapes(),
    );
    // The frame round it.
    ui.painter().rect_stroke(
        rect,
        4.0,
        egui::Stroke::new(1.0, theme::LINE),
        egui::StrokeKind::Outside,
    );
}

fn inspect(screen: &mut BuilderScreen, star: u32) {
    screen.lobby.inspect(star);
    screen.inspected = screen.lobby.inspected.as_ref().map(|(id, _)| *id);
}

/// The side panel: the star that is open, its bodies and its stations,
/// with a diagram of the system.
fn system_card(
    ui: &mut egui::Ui,
    screen: &mut BuilderScreen,
    settings: &mut Settings,
    online: &Online,
    editable: bool,
    width: f32,
) {
    let Some(star) = screen.inspected else {
        ui.label(egui::RichText::new("Pick a star").strong());
        return;
    };
    let base = screen
        .lobby
        .galaxy
        .star(star)
        .map(|s| star_name(s.name))
        .unwrap_or_default();
    let class = screen
        .lobby
        .galaxy
        .star(star)
        .map(|s| {
            STAR_CLASS_NAMES
                .get(s.star_class as usize)
                .copied()
                .unwrap_or("?")
        })
        .unwrap_or("?");
    ui.label(egui::RichText::new(format!("{base} · class {class}")).strong());
    let diagram_h = 180.0;
    let (rect, _) = ui.allocate_exact_size(egui::vec2(width, diagram_h), egui::Sense::hover());
    screen.lobby.spawn = settings.spawn;
    screen
        .lobby
        .paint_system(rect.width(), rect.height(), &mut screen.system_list);
    paint_shapes(
        ui.painter(),
        rect_of(rect),
        View::PIXELS,
        screen.system_list.shapes(),
    );

    let Some((_, system)) = screen.lobby.inspected.as_ref() else {
        return;
    };
    // The labels on the diagram. Text is the app's — the buffer holds
    // rectangles and ellipses and nothing else — so it goes on after the
    // shapes, where the lobby says each thing landed.
    let painter = ui.painter().with_clip_rect(rect);
    for (i, body) in system.bodies.iter().enumerate() {
        if let Some(&(x, y)) = screen.lobby.placed.bodies.get(i) {
            painter.text(
                rect.min + egui::vec2(x + 9.0, y - 9.0),
                egui::Align2::LEFT_CENTER,
                roman(body.name.part as u32),
                egui::FontId::proportional(11.0),
                theme::MUTED,
            );
        }
    }
    for (i, station) in system.stations.iter().enumerate() {
        if let Some(&(x, y)) = screen.lobby.placed.stations.get(i) {
            painter.text(
                rect.min + egui::vec2(x + 9.0, y + 9.0),
                egui::Align2::LEFT_CENTER,
                STATION_KIND_NAMES
                    .get(station.kind as usize)
                    .copied()
                    .unwrap_or("Station"),
                egui::FontId::proportional(11.0),
                theme::INK,
            );
        }
    }

    let bodies: Vec<(String, &'static str)> = system
        .bodies
        .iter()
        .map(|b| {
            (
                format!("{base} {}", roman(b.name.part as u32)),
                BODY_KIND_NAMES
                    .get(b.kind as usize)
                    .copied()
                    .unwrap_or("Body"),
            )
        })
        .collect();
    let stations: Vec<(String, String)> = system
        .stations
        .iter()
        .map(|s| {
            let parent = match s.parent_body {
                Some(p) if p != NONE => system
                    .body(p)
                    .map(|b| format!("{base} {}", roman(b.name.part as u32)))
                    .unwrap_or_else(|| "deep space".into()),
                _ => "deep space".into(),
            };
            // Which gear trades it has (feature 95) goes on the same
            // line: a crew choosing where to start are choosing a desk
            // as much as a system.
            let trades = gear_trades(
                s.stock.sells(physics::ResourceId::Handgun),
                s.stock.sells(physics::ResourceId::Helm),
            );
            (
                station_name(s.name),
                format!(
                    "{} · {parent}{}",
                    STATION_KIND_NAMES
                        .get(s.kind as usize)
                        .copied()
                        .unwrap_or("Station"),
                    trades.map(|t| format!(" · {t}")).unwrap_or_default()
                ),
            )
        })
        .collect();
    // Whatever is left under the diagram, and the list scrolls in it: the
    // preview beside it decides how tall the tab is.
    let left = ui.available_height().max(80.0);
    egui::ScrollArea::vertical()
        .max_height(left)
        .show(ui, |ui| {
            for (name, kind) in bodies {
                ui.horizontal(|ui| {
                    ui.label(name);
                    ui.label(egui::RichText::new(kind).small().color(theme::MUTED));
                });
            }
            ui.separator();
            if stations.is_empty() {
                ui.label(
                    egui::RichText::new("No station here — nowhere to start from.")
                        .color(theme::MUTED),
                );
            }
            for (i, (name, whereabouts)) in stations.into_iter().enumerate() {
                let on = settings.spawn == Some((star, i as u32));
                // Somebody else's station: named in the enemy's red, and
                // not somewhere to start — the button is dead and says why.
                let hostile = screen.lobby.station_hostile(i as u32);
                ui.horizontal(|ui| {
                    ui.vertical(|ui| {
                        ui.label(egui::RichText::new(name).color(if on {
                            theme::ACCENT
                        } else if hostile {
                            theme::BAD
                        } else {
                            theme::INK
                        }));
                        ui.label(
                            egui::RichText::new(if hostile {
                                format!("{whereabouts} · hostile")
                            } else {
                                whereabouts
                            })
                            .small()
                            .color(theme::MUTED),
                        );
                    });
                    let label = if editable {
                        if on { "Starting here" } else { "Start here" }
                    } else {
                        "Suggest"
                    };
                    let can_start = !editable || screen.lobby.can_start_at(star, i as u32);
                    let mut button =
                        ui.add_enabled(!(editable && on) && can_start, egui::Button::new(label));
                    if hostile {
                        button = button.on_disabled_hover_text(
                            "Hostile — the people living there are enemies, and a crew cannot start at an enemy's.",
                        );
                    }
                    if button.clicked() {
                        if editable {
                            settings.spawn = Some((star, i as u32));
                            screen.lobby.spawn = settings.spawn;
                            screen.net.push(online, settings, false);
                        } else {
                            // A guest pointing at a station: a ring on the map
                            // for everybody, and a word about who.
                            screen.lobby.ping(star);
                            online.send(To::All, &Packet::Suggest { star });
                        }
                    }
                });
            }
        });
}

/// A random station among every star that has one a crew can start at:
/// the lobby's rule (`Lobby::random_start`), which skips the hostile
/// ones — a crew cannot start at an enemy's — off this page's roll.
fn pick_random_start(screen: &mut BuilderScreen, settings: &mut Settings, online: &Online) {
    let roll = crate::screens::room::rand_seed();
    let Some((star, station)) = screen.lobby.random_start(roll) else {
        return;
    };
    inspect(screen, star);
    settings.spawn = Some((star, station));
    screen.lobby.spawn = settings.spawn;
    screen.net.push(online, settings, false);
}

/// What a station at a star is called, "Cordell Yard 7 at Tanis-284". Reads
/// the inspected system if it is the right one and opens the other for a
/// moment otherwise — the name is the galaxy's to give, and the screen
/// keeps no copy of any.
fn place_name(
    lobby: &mut Lobby,
    inspected: Option<u32>,
    star: u32,
    station: u32,
) -> Option<String> {
    let name = |lobby: &Lobby| -> Option<String> {
        let (_, system) = lobby.inspected.as_ref()?;
        let s = system.station(station)?;
        let star = lobby.galaxy.star(star)?;
        Some(format!(
            "{} at {}",
            station_name(s.name),
            star_name(star.name)
        ))
    };
    if lobby.inspected.as_ref().map(|(id, _)| *id) == Some(star) {
        return name(lobby);
    }
    lobby.inspect(star);
    let found = name(lobby);
    lobby.inspect(inspected.unwrap_or(NONE));
    found
}

/// What was typed, as a seed — or `None` if it is not one. Every digit and
/// nothing else, and no bigger than a u64 holds.
fn parse_seed(text: &str) -> Option<u64> {
    let trimmed: String = text
        .chars()
        .filter(|c| !c.is_whitespace() && *c != '_')
        .collect();
    if trimmed.is_empty() || !trimmed.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    trimmed.parse().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_seed_is_digits_and_a_room_code_is_six_readable_letters() {
        // --- a_seed_is_digits_and_nothing_else ---
        {
            assert_eq!(parse_seed("42"), Some(42));
            assert_eq!(parse_seed(" 1 000 "), Some(1000));
            assert_eq!(parse_seed("18446744073709551615"), Some(u64::MAX));
            assert_eq!(parse_seed("18446744073709551616"), None);
            assert_eq!(parse_seed("abc"), None);
            assert_eq!(parse_seed(""), None);
        }

        // --- a_room_code_is_six_of_the_readable_letters ---
        // The relay deals them (`wire`); the field takes exactly that many.
        {
            assert_eq!(CODE_LENGTH, 6);
            assert!(wire::is_code("Q7FKAB"));
            assert!(!wire::is_code("O0I1AB"));
            assert!(!wire::is_code("ABCDE"));
        }
    }
}
