//! The game: one world, one clock, and the screen turning the crank on it.
//!
//! The design phase is over by the time any of this runs: the ship is
//! settled, it is docked at the spawn station, and what was left of the pool
//! is in the crew's hands. The crew are the room — the same simulation as
//! the behaviour test room, laid out on the ship — so the crew's panels are
//! `crew.rs`, and what this screen adds is the coordinates: a window point
//! read back through the ship's camera and heading into the room's own
//! units.
//!
//! The world advances in fixed steps and never in stretched ones: a 24x
//! step would move the ship several times its own length and a trip would
//! skip straight past its own braking phase. Speed is more steps, never
//! bigger ones.

use bevy::prelude::*;
use bevy_egui::{EguiContexts, EguiPrimaryContextPass, egui};
use bims::order::CrewOrder;
use bims::room::{HIT_BIM, HIT_DOOR, HIT_DROPPED, HIT_SHIP_DOOR};
use flight::Target;
use physics::{Facing, ResourceId};
use ship::Session;
use ship::game::ViewMode;
use shipdesign::Storage;
use shipdesign::parts::Rotation;
use wire::{PeerId, To};
use world::{Refusal, ShipState, Speed, Where, WorldEvent};
use worldgen::Node;

use super::designer::{Cart, Net, Order, ShipSession, trade_rows};
use crate::canvas::{Pointer, canvas_painter, rect_of, root_ui, zoom_factor};
use crate::crew::{
    Actions, Body, CLICK_SLOP, Craft, CrewPanels, GearOrder, Hold, Near, Open, ResearchView, Tool,
    UpgradeView,
};
use crate::format::{euros, grouped, roman, spell};
use crate::keys::{Action, Keys};
use crate::names::*;
use crate::net::{CHECK_EVERY, Event, Online, Packet};
use crate::save::{Beginning, Request};
use crate::scene::WorldCanvas;
use crate::screens::room::{panel_frame, tray_frame};
use crate::settings::{Allowed, Sheet, settings_sheet};
use crate::shapes::View;
use crate::sound::{Bed, Sounds};
use crate::{Launch, Screen, icons, theme};
use bims::game::Container;
use shipdesign::parts::PartKind;

/// Ceiling on world steps per frame. It has to be at least `TOP_SPEED * 60
/// / 30`, or the top of the speed range stops being reachable on a display
/// that is keeping up at 30fps and the world quietly runs slower than the
/// button says. 48x at 30fps is 96 a frame.
const MAX_STEPS_PER_FRAME: u32 = 128;

/// How many lines of what-just-happened stay on screen.
const LOG_LINES: usize = 4;

/// How long the window stays black once the ship is down, in seconds,
/// and how much of that is the fade back in. The landing fades to black
/// on the way down (`ship::world_paint`); this is the world loading —
/// the settlement laid out, the rooms joined — and a beat on it.
const BLACKOUT_HOLD: f32 = 1.4;
const BLACKOUT_FADE: f32 = 0.6;

/// How far into the `raid` command the raid makes contact, in minutes of
/// the world's clock — ten seconds at 1×, since a minute of the clock is
/// a real second (`time::MINUTES_PER_SECOND`).
const RAID_IN_MINUTES: u64 = 10;

/// How long the `droids` probes wait between waves, in minutes of the
/// world's clock: a minute, a second at 1x, where the game's own is
/// `data::DROID_REINFORCE_MINUTES` (two hours). The shortcut feature 83
/// asks for, so a wave landing can be watched rather than waited for.
const DROID_REINFORCE_IN_PROBE: f64 = 1.0;

/// How many waves of machines the `droids` probes give a held station,
/// the one aboard counted — where the game's own is
/// `droid::wave_count`'s sum, which at day nought is
/// `data::DROID_WAVES_BASE` (two). **Three**, because these commands
/// exist to look at what a wave *after* the first does: with two, one
/// landed and the station was cleared. `BIMS_DROID_WAVES=n` says
/// otherwise.
const DROID_WAVES_IN_PROBE: u32 = 3;

/// How long the `defense` command waits between the crew setting down at
/// a threatened town and the first wave, in minutes of the world's clock
/// (feature 94): a minute, where the game's own is
/// `data::DEFENSE_DELAY_MINUTES` (an hour) — the same shortcut as the
/// machines' reinforcement clock, and for the same reason.
/// `BIMS_DEFENSE_DELAY=n` says otherwise.
const DEFENSE_DELAY_IN_PROBE: f64 = 1.0;

/// What the map writes over the ship, before where it is.
const HERE_TAG: &str = "You";
/// How far above the ship's mark on the map its words sit: clear of the
/// reticle `ship::world_paint` draws round it, ring and ticks.
const HERE_LIFT: f32 = 36.0;
/// What the map writes after a landable planet's name, and how far
/// **below** its icon the words' baseline sits: under it rather than
/// over, because the ship's own words go over, and a ship docked at a
/// planet's station is drawn on the planet. Clear of the stance ring
/// (`26 * 1.3 / 2`, about 17) and the reticle's south tick when the ship is
/// docked at the planet's station, with a line of type to spare.
const LAND_TAG: &str = "land";
/// And what a town on the front is tagged instead (feature 94): one the
/// machines are a hop from, and one the crew held against them.
const TOWN_THREATENED_TAG: &str = "threatened";
const TOWN_HELD_TAG: &str = "held";
const LAND_DROP: f32 = 38.0;

/// How near a click has to come to a map icon to count as picking it, in
/// points. Measured on screen rather than in world units: the thing being
/// aimed at is an icon.
const MAP_PICK_SLOP: f32 = 14.0;

/// What the helm is aimed at, for the local player and nobody else.
#[derive(Clone, Copy)]
enum Aim {
    Node(Node),
    Point(f64, f64),
}

/// A drag with the Mine tool: a press on a rock marks it — or unmarks a
/// marked one — and the drag that follows does the same to every rock the
/// pointer is pulled over, so a whole face is marked in one stroke.
///
/// An order to the crew's room as the seam carries it: given plain, or —
/// with Shift held — to wait its turn behind what the crew member is on
/// (feature 69, `Order::CrewLater`).
fn crew_order(order: CrewOrder, later: bool) -> Order {
    if later {
        Order::CrewLater(order)
    } else {
        Order::Crew(order)
    }
}

impl Aim {
    fn target(self) -> Target {
        match self {
            Aim::Node(Node::Body(id)) => Target::Body(id),
            Aim::Node(Node::Station(id)) => Target::Station(id),
            Aim::Point(x, y) => Target::Point(worldgen::math::dvec2(x, y)),
        }
    }
}

/// An order that wants somebody at the helm, from the strip at the top.
/// The world refuses one from anywhere else, so the screen walks the local
/// player's crew member to the seat first and gives the order the frame it
/// gets there.
#[derive(Clone, Copy)]
enum HelmOrder {
    Fly(Aim),
    Stop,
    /// Charge the hyperdrive for the star picked on the galaxy chart.
    Jump(u32),
    /// Come down onto the planet the ship is holding over.
    Land,
}

#[derive(Resource)]
pub struct GameScreen {
    net: Net,
    /// The crew's panels: `None` until the world opens, because everything
    /// they read is the room aboard, which does not exist before there is
    /// a world.
    panels: Option<CrewPanels>,
    aimed: Option<Aim>,
    /// An order on its way to the helm: the crew member is walking there,
    /// and it goes through the seam the frame they arrive.
    pending: Option<HelmOrder>,
    /// The order went through: the post at the helm is lifted once the
    /// world has stepped with it, so the crew member goes back to its
    /// errands and the helm is the room's job to fill.
    relieve: bool,
    /// The room tile a soldier is aiming a grenade at while the Q key is
    /// held (feature 75), for the burst's ring on the deck.
    throw_aim: Option<(i32, i32)>,
    /// The **attack** key has armed the pointer (feature 84): the system
    /// cursor is off, a red crosshair is drawn in its place, and the next
    /// left click on the deck puts the banner down there. Esc, a
    /// right-click or the key again puts it away.
    aiming_attack: bool,
    /// Whether the station's trade window is up.
    trading: bool,
    /// `BIMS_ARMOURY=1`: the armoury window is to be opened on the first
    /// frame there is a room to open it in.
    armoury_wanted: Option<String>,
    /// Tab went down last frame with the keys ours: the focus egui gave a
    /// widget for it is to be surrendered (`keys::release_tab_focus`).
    tab_took_focus: bool,
    /// What is in the trade window's cart, not yet bought or sold.
    cart: Cart,
    backlog: f64,
    log: Vec<String>,
    /// Where the pointer is over the canvas, for the readout.
    hover_at: Option<Vec2>,
    /// A marquee under way on the deck: where the press landed.
    marquee_from: Option<Vec2>,
    /// Where a right-drag on the deck began, on the glass: an order in the
    /// making — a line for the selected crew, or a point if it never moves
    /// further than a click.
    order_from: Option<Vec2>,
    pan_from: Option<Vec2>,
    /// The Esc sheet, if it is up, and which page.
    sheet: Option<Sheet>,
    /// The sheet's save and load pages' state — `crate::save`.
    saves: crate::save::Saves,
    size: Vec2,
    /// The speed Space pauses from, for Space to go back to.
    resume: Speed,
    /// The smooth fog over the deck, as a texture — see `fogmap`.
    fog: crate::fogmap::FogTexture,
    /// The same over the plain beyond the box, on a planet: a texture a
    /// chunk of the room, kept while the room composes the chunk.
    plain_fog: std::collections::BTreeMap<(i32, i32), crate::fogmap::FogTexture>,
    /// The galaxy chart, up over the system map: the strip's `Galaxy view`.
    /// The chart itself is the lobby's, made the first time it is asked
    /// for — it generates every system once — and kept for the game.
    galaxy_up: bool,
    galaxy: Option<lobby::Lobby>,
    galaxy_list: lobby::draw::DrawList,
    galaxy_size: Vec2,
    /// The star picked on the chart: what its system holds is in the
    /// strip, and it is where Jump goes.
    picked_star: Option<u32>,
    /// The slots whose players have left the game: said once each; their
    /// crew members carry on unsteered.
    gone: Vec<u32>,
    /// A guest whose checksum has parted from the host's and has asked
    /// for its world (`Packet::Resync`, feature 67): said once, and not
    /// asked again until the world arrives. The wrong world keeps
    /// stepping meanwhile — stopping would make it wronger under the
    /// pointer.
    resyncing: bool,
    /// The host's side of it: the step each peer was last answered a
    /// `World` at, so a guest that keeps asking gets one an
    /// `CHECK_EVERY` at most — the world is megabytes to write.
    answered: Vec<(PeerId, u64)>,
    /// `BIMS_DESYNC_AT`: on a guest, the step at which one order is
    /// applied without asking the host — a divergence on purpose, for
    /// looking at the resync. Taken once it has fired.
    desync_at: Option<u64>,
    /// `BIMS_FREEZE` (feature 98): the shots still to be heard in the
    /// crew's room and the frames after the last of them before the game
    /// pauses itself. Taken once it has fired.
    freeze: Option<crate::dev::Freeze>,
    /// Seconds of black left over the canvas: a landing ends in it — the
    /// planet has filled the window and the settlement is being laid out —
    /// and it lifts once the ground is there. Nought nearly always.
    blackout: f32,
    /// Whether the tray is still to be opened on the Skills tab for a
    /// point waiting to be spent (features 80 and 83). Set at every
    /// `WorldEvent::LevelUp` of the local slot — and at an open, so a run
    /// that starts part-way up the tree shows the tree at once — and
    /// taken the first frame the player's own crew member actually has a
    /// pick waiting. Nothing is forced after that: the tray is the
    /// player's, and the Skills tab is where the point is spent whenever
    /// they get to it.
    skills_prompt: bool,
    /// The green numbers a heal beam puts up over the body it holds
    /// (feature 91). Kept by the screen and nowhere else: what a beam put
    /// back is the body's own count going up, and neither the room nor
    /// the world records it as an event to be read.
    heals: Heals,
}

/// How long one of those numbers is in the air, and the shortest gap
/// between two over one body — at 24× a beam puts back a dozen points a
/// second, and a number a frame would be a green smear rather than a
/// figure anybody could read.
const HEAL_FLOAT_SECONDS: f64 = 1.3;
const HEAL_GAP: f64 = 0.45;

/// What a body a beam holds had last frame, and what has gathered since
/// the last number went up over it.
#[derive(Clone, Copy)]
struct Watched {
    /// Blood and the three parts added: everything a beam puts back.
    points: f32,
    /// Points gathered and not yet shown — a number is whole, and a beam
    /// puts its points back a fraction at a time.
    gathered: f32,
    /// When the last number over this body went up.
    said: f64,
}

/// One number in the air: over whom, how much, and when it appeared.
#[derive(Clone, Copy)]
struct Floater {
    who: u32,
    points: u32,
    born: f64,
}

#[derive(Default)]
struct Heals {
    /// One entry per crew member some medic's beam holds; dropped the
    /// frame the beam lets go, so a body picked up again starts afresh
    /// rather than showing what it mended on its own in between.
    watched: std::collections::BTreeMap<u32, Watched>,
    floating: Vec<Floater>,
}

/// What every beam on the deck has put back since last frame, gathered
/// into whole numbers to float over each patient (feature 91). Called
/// every frame the world is stepping, map up or not, so a number does
/// not appear for a minute's worth of healing the moment the map closes.
fn note_heals(heals: &mut Heals, game: &ship::game::Game, now: f64) {
    let world = &game.world;
    let crew = world.aboard.crew_count();
    let mut beamed: Vec<u32> = Vec::new();
    for medic in 0..crew {
        for patient in world.patients_of(medic) {
            if !beamed.contains(&patient) {
                beamed.push(patient);
            }
        }
    }
    heals.watched.retain(|who, _| beamed.contains(who));
    let room = &world.aboard.room;
    for who in beamed {
        let points = room.blood(who as usize) + room.health(who as usize);
        let watched = heals.watched.entry(who).or_insert(Watched {
            points,
            gathered: 0.0,
            said: now,
        });
        // Only what came *back*: a patient taking a bolt while it is
        // beamed loses points, and a red number is the health bar's job.
        watched.gathered += (points - watched.points).max(0.0);
        watched.points = points;
        if watched.gathered >= 1.0 && now - watched.said >= HEAL_GAP {
            let whole = watched.gathered.floor();
            watched.gathered -= whole;
            watched.said = now;
            heals.floating.push(Floater {
                who,
                points: whole as u32,
                born: now,
            });
        }
    }
    heals.floating.retain(|f| now - f.born < HEAL_FLOAT_SECONDS);
}

pub struct GamePlugin;

impl Plugin for GamePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(Screen::Game), open)
            .add_systems(EguiPrimaryContextPass, frame.run_if(in_state(Screen::Game)))
            .add_systems(EguiPrimaryContextPass, over.run_if(in_state(Screen::Over)));
    }
}

/// The end of the run: nobody of the crew standing — `World::lost`, said
/// by `WorldEvent::CrewLost` — and the game screen hands over to this,
/// a screen that says so and a way back to the menu. The world is left
/// as it was, so a save made before the fight is still there to load.
fn over(
    mut contexts: EguiContexts,
    mut session: ResMut<ShipSession>,
    mut next: ResMut<NextState<Screen>>,
    window: Single<&Window>,
    online: Res<Online>,
    beginning: Option<Res<Beginning>>,
) -> Result {
    let ctx = contexts.ctx_mut()?.clone();
    let mut root = root_ui(&ctx);
    let when = session
        .0
        .game
        .as_ref()
        .map(|game| {
            let minutes = game.world.minutes_into_day();
            format!(
                "Day {}, {:02}:{:02}.",
                game.world.day(),
                (minutes / 60.0).floor() as u32,
                (minutes % 60.0).floor() as u32
            )
        })
        .unwrap_or_default();
    // The run again from where it opened, the Esc sheet's Restart from a
    // screen that has no Esc sheet (feature 79): the fight is the likeliest
    // place to want it, and it is the likeliest place to end up. The
    // host's alone, as a restart is anywhere.
    let mut again = false;
    let restartable = beginning.is_some() && !online.is_guest();
    egui::CentralPanel::default().show(&mut root, |ui| {
        ui.vertical_centered(|ui| {
            ui.add_space(ui.available_height() * 0.3);
            ui.label(egui::RichText::new(OVER_TITLE).size(28.0).strong());
            ui.label(egui::RichText::new(OVER_LINE).color(theme::MUTED));
            ui.label(egui::RichText::new(when).color(theme::MUTED));
            ui.add_space(12.0);
            if restartable && ui.link(RESTART_AGAIN).clicked() {
                again = true;
            }
            if ui.link(OVER_BACK).clicked() {
                next.set(Screen::Menu);
            }
        });
    });
    // The session is put back to the beginning and the screen entered
    // again, so `open` stands the panels up round it as it would round
    // any other open — the beginning it then keeps is the one just read.
    if again
        && let Some(text) = beginning.as_ref()
        && let Ok(loaded) =
            Session::restore(&text.0, window.width().max(64.0), window.height().max(64.0))
    {
        // With company the world goes round as it does on a load, so
        // everybody's crew stand up again and not just the host's.
        if online.is_host() {
            let at = loaded.game.as_ref().map_or(0, |g| g.world.steps);
            online.send(
                To::All,
                &Packet::World {
                    save: text.0.clone(),
                    at,
                },
            );
        }
        crate::names::set_crew_names(&loaded.crew_names);
        session.0 = loaded;
        next.set(Screen::Game);
    }
    Ok(())
}

fn open(
    mut commands: Commands,
    session: Option<Res<ShipSession>>,
    launch: Res<Launch>,
    window: Single<&Window>,
    online: Res<Online>,
) {
    let size = Vec2::new(window.width().max(64.0), window.height().max(64.0));
    let (slot, players) = match session {
        Some(session) => {
            // The crew's names, as the lobby dealt them or a save kept them.
            crate::names::set_crew_names(&session.0.crew_names);
            remember_beginning(&mut commands, &session.0);
            (session.0.editor.local, session.0.editor.players)
        }
        None => {
            // No design phase in front of this: the simulation, on the
            // playtest ship. The `test` command is the same somewhere else
            // each time — a random seed, and a dock somebody lives on picked
            // at random across that galaxy.
            // `test_planet` is the same roll made among the systems with
            // friendly ground, since the ship is then set down on it.
            let (seed, spawn) = match *launch {
                Launch::Test
                | Launch::TestPlanet
                | Launch::DroidsPlanet
                | Launch::Defense
                | Launch::Crisis
                | Launch::Jammer => {
                    let seed = super::room::rand_seed();
                    let roll = super::room::rand_seed();
                    let pick = match *launch {
                        Launch::TestPlanet | Launch::DroidsPlanet | Launch::Defense => {
                            ship::session::pick_ground
                        }
                        // `crisis` is `test`: a dock somebody lives on,
                        // so there are people for the machines to take.
                        _ => ship::session::pick_dock,
                    };
                    (seed, pick(seed, 0, roll))
                }
                _ => (world::data::DEFAULT_SEED, None),
            };
            // The `combat` command is the fight: the combat ship's fourteen
            // crew, a gun in every hand, docked at the spawn rebuilt as the
            // arena and turned against them — its people enemies, fifteen
            // of them — so a recruited crew member has somebody to
            // shoot at and somewhere to do it. `Session::combat` is all of
            // that; `BIMS_FIGHT` stages the two a few tiles apart on top.
            // The `test` command is on the combat ship too, with one crew
            // member — four bunks to spare — and a mercenary for hire at
            // the dock whatever the roll said, so a hire can be looked at.
            // The `test_planet` command is that landed: the ship set down
            // on the system's first planet with ground, the way
            // `BIMS_LANDED=1` sets the simulation down — the mercenary asked
            // for first, since the ask holds for every friendly room opened
            // after it, the settlement's included.
            // The `raid` command is the simulation off its berth, holding
            // in open space with a raid on its way: contact `RAID_IN_MINUTES`
            // of the clock in — ten seconds at 1× — and the raider then
            // closing at its own pace, so the warning, the map and the
            // boarding are watched from the start rather than staged.
            // `tier2_test` and `tier3_test` are `combat` with everybody's
            // guns and armour at that tier, crew and garrison alike
            // (`Session::combat_at_tier`).
            // A `combat_<class>` command is the fight itself, with the
            // class below put on the crew member you steer: the ship,
            // the arena and the garrison are `combat`'s, so two of those
            // runs differ by the class and nothing else (feature 79).
            let mut session = match *launch {
                Launch::Combat | Launch::CombatAs(_) => Session::combat(seed, size.x, size.y),
                Launch::CombatAtTier(tier) => Session::combat_at_tier(seed, tier, size.x, size.y),
                // The `droids` command is the fight with the arena held
                // by the machines (feature 83): the same ship, the same
                // crew and the same guns, and a wave of droids about the
                // arena instead of its garrison. `BIMS_DROID_TIER` is
                // what tier they come at and `BIMS_DROID_WAVE` how big a
                // wave may be, for the measurements.
                // `combat_droids_<class>` is that same wave with the
                // class below in hand, exactly as `combat_<class>` is
                // `combat`'s own fight with one: the ship, the arena,
                // the wave and both dials are `droids`'.
                Launch::Droids | Launch::DroidsAs(_) => Session::droids(
                    seed,
                    crate::dev::droid_tier(),
                    crate::dev::droid_reinforce(DROID_REINFORCE_IN_PROBE),
                    crate::dev::droid_wave_max(),
                    crate::dev::droid_waves(DROID_WAVES_IN_PROBE),
                    size.x,
                    size.y,
                ),
                Launch::Test
                | Launch::TestPlanet
                | Launch::DroidsPlanet
                | Launch::Defense
                | Launch::Crisis
                | Launch::Jammer => {
                    let design = shipdesign::fixture::combat_ship();
                    let mut session =
                        Session::simulate_on(design, 1, seed, 0, spawn, size.x, size.y);
                    session.mercenary_for_probe();
                    // `crisis` is that with the clock a day short of the
                    // machines appearing and their origin two hyperlane
                    // hops off, so the chart turns red while you watch
                    // (feature 92).
                    if *launch == Launch::Crisis {
                        session.crisis_for_probe(crate::dev::crisis_day());
                    }
                    // `jammer` is one step on from that (feature 93): the
                    // clock wound past the day this system falls, every
                    // station of it in the machines' hands, and the crew
                    // tied up at one of them with a wave aboard — so the
                    // jam on the chart and the wave on the deck are the
                    // same picture.
                    if *launch == Launch::Jammer {
                        session.jammer_for_probe(
                            crate::dev::droid_tier(),
                            crate::dev::droid_reinforce(DROID_REINFORCE_IN_PROBE),
                            crate::dev::droid_waves(DROID_WAVES_IN_PROBE),
                        );
                    }
                    if matches!(
                        *launch,
                        Launch::TestPlanet | Launch::DroidsPlanet | Launch::Defense
                    ) {
                        session.land_for_probe();
                    }
                    // `defense` is that with the town **threatened**
                    // (feature 94): the machines' origin one hop off, so
                    // a wave lands outside a gate a minute after the
                    // landing and the town's own people fight beside the
                    // crew. After the landing, since what starts an
                    // attack is the crew being on the pad.
                    if *launch == Launch::Defense {
                        if let (Some(n), Some(game)) =
                            (crate::dev::droid_wave_max(), session.game.as_mut())
                        {
                            game.world.set_droid_wave_for_probe(n);
                        }
                        session.defense_for_probe(
                            crate::dev::defense_delay(DEFENSE_DELAY_IN_PROBE),
                            crate::dev::droid_reinforce(DROID_REINFORCE_IN_PROBE),
                            crate::dev::droid_waves(DROID_WAVES_IN_PROBE),
                        );
                    }
                    // `droids_planet` is that with the town held: the
                    // settlement's own people gone and the machines in
                    // their place, the same minute's reinforcements.
                    if *launch == Launch::DroidsPlanet {
                        if let (Some(n), Some(game)) =
                            (crate::dev::droid_wave_max(), session.game.as_mut())
                        {
                            game.world.set_droid_wave_for_probe(n);
                        }
                        session.infest_the_dock_for_probe(
                            crate::dev::droid_tier(),
                            crate::dev::droid_reinforce(DROID_REINFORCE_IN_PROBE),
                            crate::dev::droid_waves(DROID_WAVES_IN_PROBE),
                        );
                    }
                    session
                }
                Launch::Raid => {
                    let mut session = Session::simulate(seed, 0, spawn, size.x, size.y);
                    session.raid_coming_for_probe(RAID_IN_MINUTES);
                    session
                }
                _ => Session::simulate(seed, 0, spawn, size.x, size.y),
            };
            // The class onto slot 0 first — the command's own, or
            // `BIMS_CLASS` over it — before anything that leaves the
            // berth, since the class locks at the first undock.
            let asked = match *launch {
                Launch::CombatAs(class) | Launch::DroidsAs(class) => class,
                _ => world::Class::None,
            };
            crate::dev::class_crew(&mut session, asked);
            // And `BIMS_BEAM` after it: the class has to be on before a
            // medic can hold anybody (feature 76).
            crate::dev::beam_crew(&mut session);
            if crate::dev::landed() || crate::dev::afield() {
                session.land_for_probe();
            }
            if crate::dev::afield() {
                session.walk_afield_for_probe();
            }
            if let Some(done) = crate::dev::landing() {
                session.landing_for_probe(done);
            }
            if crate::dev::fight() {
                session.stage_fight_for_probe();
            }
            // The dead of a fight nobody watched, lying where they stood
            // (feature 85): after the fight above, so a staged fight and
            // its aftermath can be asked for together.
            if let Some(n) = crate::dev::graves() {
                session.lay_graves_for_probe(n);
            }
            // Every state a machine can be drawn in, laid out on the
            // arena's deck for one picture (feature 83). The wave the
            // probe laid out is replaced by the showcase.
            if crate::dev::droid_showcase() {
                session.stage_droids_for_probe();
            }
            if let Some(dock) = crate::dev::raid() {
                session.raid_for_probe(dock);
            }
            if crate::dev::lost() {
                session.lose_for_probe();
            }
            if let Some(n) = crate::dev::lamps_out() {
                session.shoot_lamps_for_probe(n);
            }
            // And the wounded: after the fight and the lamps, so a dying
            // Bim can be asked for on a deck already staged.
            if let Some(n) = crate::dev::dying() {
                session.maim_for_probe(n);
            }
            // The hired field medics (feature 86), and a body in one's
            // arms: after the wounded, so a medic can be asked for on a
            // deck that already has somebody to fetch.
            if let Some(n) = crate::dev::field_medics() {
                session.field_medics_for_probe(n);
            }
            if crate::dev::carry() {
                session.carry_for_probe();
            }
            // And the dressings and medkits each of them carries — the
            // medicine charges: after the wounded, so a Bim asked for one
            // with a wound on it has something to bind it with, and a
            // nought is the empty box with its sweep running.
            if let Some(n) = crate::dev::bandages() {
                session.bandages_for_probe(n);
            }
            if let Some(n) = crate::dev::medkits() {
                session.medkits_for_probe(n);
            }
            // And the engineer's charges (feature 88): nought is the
            // empty pack with both cooldowns running, which is what the
            // seconds in the two boxes are looked at with.
            if let Some(n) = crate::dev::kits() {
                session.kits_for_probe(n);
            }
            // And the soldier's grenade charges (feature 90), the same
            // way: nought is an empty pack with the thirty seconds
            // running.
            if let Some(n) = crate::dev::grenades() {
                session.grenades_for_probe(n);
            }
            // A weapon asked for by name goes into the hand in place of
            // whatever was issued, the rest of the gear kept: the crew
            // member's, or every resident's; and armour asked for goes on
            // the crew member, pieces with ids the hold does not have.
            // `Game::issue` redraws the picture, so the swing or the
            // barrel is on screen from the first frame.
            let worn = crate::dev::armoured();
            if (worn || crate::dev::weapon().is_some())
                && let Some(game) = session.game.as_mut()
            {
                use bims::combat::{ArmourKind, Piece};
                let room = &mut game.world.aboard.room;
                let gear = room.gear(0);
                let piece = |kind: ArmourKind| {
                    worn.then(|| Piece::new(u32::MAX - kind.code(), kind, bims::combat::Tier::One))
                        .or(gear.worn(kind.slot()))
                };
                room.issue(
                    0,
                    bims::combat::Gear {
                        weapon: crate::dev::weapon().or(gear.weapon),
                        head: piece(ArmourKind::BasicHelm),
                        body: piece(ArmourKind::BasicKevlar),
                        legs: piece(ArmourKind::BasicLegs),
                        ..gear
                    },
                );
            }
            if let Some(kind) = crate::dev::enemy_weapon()
                && let Some(residents) = session
                    .game
                    .as_mut()
                    .and_then(|g| g.world.residents.as_mut())
            {
                let room = &mut residents.aboard.room;
                for who in 0..room.crew_count() as usize {
                    let gear = room.gear(who);
                    room.issue(
                        who,
                        bims::combat::Gear {
                            weapon: Some(kind),
                            ..gear
                        },
                    );
                }
            }
            let out = (session.editor.local, session.editor.players);
            crate::names::set_crew_names(&session.crew_names);
            remember_beginning(&mut commands, &session);
            commands.insert_resource(ShipSession(session));
            out
        }
    };
    let mut screen = GameScreen::fresh(slot, players);
    screen.net.wire = online.wire();
    commands.insert_resource(screen);
}

/// The run as it began, kept for the Esc sheet's Restart (feature 79):
/// the world written out the moment the screen opened, whatever opened
/// it — a command of its own (`combat_medic`, `raid`, `test_planet`) or
/// the last Accept in the yard. A restart stands a session up from it
/// again the way a load does, so what is kept here is exactly the
/// situation the run started in. Nothing is kept where there is no world
/// to write, which is the design phase alone.
fn remember_beginning(commands: &mut Commands, session: &Session) {
    if let Some(text) = session.save() {
        commands.insert_resource(crate::save::Beginning(text));
    }
}

impl GameScreen {
    /// The screen as it is at an open, and again round a loaded game:
    /// nothing aimed, no panels yet, the canvas unmeasured so the first
    /// frame fits the world to it.
    fn fresh(slot: u32, players: u32) -> GameScreen {
        GameScreen {
            net: Net {
                slot,
                players,
                wire: None,
            },
            panels: None,
            aimed: None,
            pending: None,
            relieve: false,
            throw_aim: None,
            aiming_attack: false,
            trading: crate::dev::trade(),
            armoury_wanted: crate::dev::armoury(),
            tab_took_focus: false,
            cart: Cart::new(),
            backlog: 0.0,
            log: Vec::new(),
            hover_at: None,
            marquee_from: None,
            order_from: None,
            pan_from: None,
            sheet: None,
            saves: crate::save::Saves::default(),
            size: Vec2::ZERO,
            resume: Speed::Real,
            fog: crate::fogmap::FogTexture::default(),
            plain_fog: std::collections::BTreeMap::new(),
            galaxy_up: false,
            galaxy: None,
            galaxy_list: lobby::draw::DrawList::new(),
            galaxy_size: Vec2::ZERO,
            picked_star: None,
            gone: Vec::new(),
            resyncing: false,
            answered: Vec::new(),
            desync_at: crate::dev::desync_at(),
            freeze: crate::dev::freeze_at_shot(),
            blackout: 0.0,
            skills_prompt: true,
            heals: Heals::default(),
        }
    }

    /// The screen as `fresh` makes it, round a world that took this
    /// one's place — a load, or the host's world arrived (feature 67) —
    /// with the wire and the log carried across: the company is the
    /// same company, and what was said is still worth reading. The
    /// probe's divergence is not carried: a world replaced has had it.
    fn again(&mut self, slot: u32, players: u32) -> GameScreen {
        let mut next = GameScreen::fresh(slot, players);
        next.net.wire = self.net.wire.take();
        next.log = std::mem::take(&mut self.log);
        next.desync_at = None;
        next.freeze = None;
        next
    }
}

/// What a thing on the map is called. A kind and a number, because a kind
/// is a fixed table and an identity is a number.
fn node_name(session: &Session, node: Node) -> String {
    // A settlement is named for the planet it stands on.
    if let Node::Station(id) = node
        && let Some(body) = world::surface_body(id)
    {
        return format!("{} settlement", node_name(session, Node::Body(body)));
    }
    // A raider is nobody's station: the one tied to the ship.
    if let Node::Station(id) = node
        && world::raider_index(id).is_some()
    {
        return "Raider".into();
    }
    let kind = session.map_type(node);
    match node {
        Node::Station(id) => format!(
            "{} {id}",
            STATION_KIND_NAMES
                .get(kind as usize)
                .copied()
                .unwrap_or("Station")
        ),
        Node::Body(id) => format!(
            "{} {id}",
            BODY_KIND_NAMES
                .get(kind as usize)
                .copied()
                .unwrap_or("Body")
        ),
    }
}

#[allow(clippy::too_many_arguments)]
fn frame(
    mut contexts: EguiContexts,
    mut screen: ResMut<GameScreen>,
    mut session: ResMut<ShipSession>,
    time: Res<Time>,
    mut sounds: ResMut<Sounds>,
    mut bindings: ResMut<Keys>,
    mut commands: Commands,
    mut next: ResMut<NextState<Screen>>,
    mut online: ResMut<Online>,
    // The run as it opened, for the Esc sheet's Restart (feature 79).
    // None where there was no world to keep, which is nowhere this
    // screen runs — held as an option rather than assumed.
    beginning: Option<Res<Beginning>>,
    // The canvas between the panels, which Bevy draws (feature 97).
    mut world_canvas: WorldCanvas,
) -> Result {
    // `BIMS_PERF`: where the frame goes (feature 96). Nothing at all
    // without it.
    let _timed = crate::perf::scope(crate::perf::Phase::Frame);
    let ctx = contexts.ctx_mut()?.clone();
    let screen = &mut *screen;
    let session = &mut session.0;
    let keys_now = *bindings;
    let now = ctx.input(|i| i.time);
    let dt = time.delta_secs().min(super::designer::MAX_FRAME_DT) as f64;
    let mut root = root_ui(&ctx);

    // --- the wire ------------------------------------------------------------
    // What the others said since last frame, in order. On the host a
    // guest's ask is applied and goes round; on a guest the host's applied
    // orders and its steps are what the world is made of. The host gone
    // is the end of company: the clock is this window's from here.
    let wire_timed = crate::perf::scope(crate::perf::Phase::Wire);
    for event in online.drain(now) {
        match event {
            Event::Packet { from, packet } => match packet {
                Packet::Ask { at, message } => {
                    if let Some(slot) = online.slot_of(from) {
                        screen.net.asked(session, slot, at, message, from);
                    }
                }
                Packet::Applied { from, at, message } => {
                    screen.net.applied(session, from, at, message);
                }
                Packet::Refused { why } => {
                    let line = if why == 0 {
                        ACCEPT_STALE
                    } else {
                        edit_line(why)
                    };
                    screen.log.push(line.to_string());
                }
                Packet::Steps { n, checksum } if !screen.net.is_clock() => {
                    for _ in 0..n {
                        session.world_step();
                    }
                    if let (Some(theirs), Some(game)) = (checksum, &session.game) {
                        let mine = world::world_checksum(&game.world);
                        if theirs != mine && !screen.resyncing {
                            // Parted from the host: say so, and ask for
                            // its world. Once — the steps keep coming
                            // and keep being applied to the wrong world
                            // until it arrives (`Packet::World`).
                            screen.resyncing = true;
                            screen.log.push(DESYNC.into());
                            screen.log.push(RESYNC_ASKED.into());
                            if let Some(wire) = &screen.net.wire {
                                wire.send(To::Host, &Packet::Resync { reason: 0 });
                            }
                            if crate::dev::auto().is_some() {
                                println!("desync: {} {theirs:#x} {mine:#x}", game.world.steps);
                            }
                        } else if theirs == mine && crate::dev::auto().is_some() {
                            println!("checksum: {} {mine:#x}", game.world.steps);
                        }
                    }
                }
                // A guest asking for the world: the host writes it out
                // and sends it to that guest alone — once an
                // `CHECK_EVERY` a peer at most, since the writing is the
                // hitch a save is. A guest asked is nobody's to answer.
                Packet::Resync { .. } if screen.net.wire.as_ref().is_some_and(|w| w.host) => {
                    let at = session.game.as_ref().map_or(0, |g| g.world.steps);
                    let recently = screen
                        .answered
                        .iter()
                        .any(|&(p, last)| p == from && at < last + CHECK_EVERY);
                    if !recently && let Some(text) = session.save() {
                        screen.answered.retain(|&(p, _)| p != from);
                        screen.answered.push((from, at));
                        if let Some(wire) = &screen.net.wire {
                            wire.send(To::Peer(from), &Packet::World { save: text, at });
                        }
                        if crate::dev::auto().is_some() {
                            println!("world sent: {at} asked");
                        }
                    }
                }
                // The host's world, whole: this one's replaced with it,
                // as this player's own slot, in place — the packets
                // behind it in this same drain are steps of the new
                // world. The screen starts over round it as at a load,
                // the wire and the log kept; the canvas is fitted again
                // this frame, since it is unmeasured. A host applies
                // none, and nobody applies one from anybody but the host.
                Packet::World { save, at }
                    if Some(from) == online.host
                        && screen.net.wire.as_ref().is_some_and(|w| !w.host) =>
                {
                    let size = screen.size.max(Vec2::splat(64.0));
                    match Session::restore_as(&save, online.my_slot(), size.x, size.y) {
                        Ok(loaded) => {
                            let (slot, players) = (loaded.editor.local, loaded.editor.players);
                            crate::names::set_crew_names(&loaded.crew_names);
                            *session = loaded;
                            *screen = screen.again(slot, players);
                            screen.log.push(RESYNC_DONE.into());
                            if crate::dev::auto().is_some() {
                                println!("resync: {at}");
                            }
                        }
                        // Not asked again: the answer would be the same
                        // text, and it is megabytes a time.
                        Err(why) => {
                            screen
                                .log
                                .push(world_refused(&crate::save::load_error(why)));
                        }
                    }
                }
                _ => {}
            },
            Event::Roster => {
                for slot in 0..screen.net.players {
                    let there = online
                        .slots
                        .get(slot as usize)
                        .is_some_and(|id| online.peers.iter().any(|p| p.id == *id));
                    if !there && !screen.gone.contains(&slot) {
                        screen.gone.push(slot);
                        screen.log.push(player_left(&crew_name(slot)));
                    }
                }
            }
            Event::Closed(_) | Event::Lost(_) if screen.net.wire.is_some() => {
                screen.net.wire = None;
                screen.log.push(HOST_GONE.into());
            }
            _ => {}
        }
    }

    // A Bim's name said late — after the host's Start, or typed since —
    // lands on the crew here, and on every word about them.
    if online.names_said(&mut session.crew_names) {
        crate::names::set_crew_names(&session.crew_names);
    }
    if online.hair_said(&mut session.crew_hair) || online.tint_said(&mut session.crew_tints) {
        session.dress_crew();
    }

    drop(wire_timed);

    if session.game.is_none() {
        // A world that never opened — a spawn the galaxy has not got.
        egui::CentralPanel::default().show(&mut root, |ui| {
            ui.vertical_centered(|ui| {
                ui.add_space(ui.available_height() * 0.3);
                ui.label(egui::RichText::new("Nowhere to start").size(28.0).strong());
                ui.label(
                    egui::RichText::new("The galaxy has no station to dock at.")
                        .color(theme::MUTED),
                );
            });
        });
        return Ok(());
    }

    // --- the world, stepped -------------------------------------------------
    // The clock is this window's alone in a game of one, and the host's
    // with company: a guest's steps arrived above, and the host says
    // below how many it took, with its checksum every `CHECK_EVERY` so a
    // guest can tell it is still the same world.
    let mut steps = 0;
    if screen.net.is_clock() {
        let times = session
            .game
            .as_ref()
            .map(|g| g.world.effective_speed().multiplier())
            .unwrap_or(0) as f64;
        screen.backlog += dt * session.steps_per_second() * times;
        let before = session.game.as_ref().map_or(0, |g| g.world.steps);
        {
            let _timed = crate::perf::scope(crate::perf::Phase::Step);
            while screen.backlog >= 1.0 && steps < MAX_STEPS_PER_FRAME {
                session.world_step();
                screen.backlog -= 1.0;
                steps += 1;
            }
        }
        crate::perf::tally(crate::perf::Count::Steps, steps as u64);
        if screen.backlog > MAX_STEPS_PER_FRAME as f64 {
            screen.backlog = 0.0; // gave up catching up
        }
        if steps > 0
            && let Some(wire) = &screen.net.wire
            && let Some(game) = &session.game
        {
            let after = game.world.steps;
            let checksum = (before / CHECK_EVERY != after / CHECK_EVERY)
                .then(|| world::world_checksum(&game.world));
            if let Some(mine) = checksum
                && crate::dev::auto().is_some()
            {
                println!("checksum: {after} {mine:#x}");
            }
            wire.send(To::All, &Packet::Steps { n: steps, checksum });
        }
    } else if let Some(at) = screen.desync_at
        && let Some(room) = session.room_ref()
        && session.game.as_ref().is_some_and(|g| g.world.steps >= at)
    {
        // `BIMS_DESYNC_AT`: the guest's own crew member sent three tiles
        // over without the host hearing of it — a world of its own from
        // here, for the resync to mend.
        screen.desync_at = None;
        let slot = screen.net.slot;
        let from = room.bim_pos(slot as usize);
        use bims::order::CrewOrder;
        screen
            .net
            .apply_unasked(session, Order::Crew(CrewOrder::SelectOwn));
        screen.net.apply_unasked(
            session,
            Order::Crew(CrewOrder::Move {
                x: from.x + 3.0 * shipdesign::parts::TILE as f32,
                y: from.y,
            }),
        );
        if crate::dev::auto().is_some() {
            println!("diverged: {at}");
        }
    }
    // The fight's passing lights age on this window's own clock — real
    // seconds at any speed, held still while the game is paused (feature
    // 98) — on the host and a guest alike.
    let running = session
        .game
        .as_ref()
        .is_some_and(|g| g.world.effective_speed().multiplier() > 0);
    session.age_effects(if running { dt as f32 } else { 0.0 });
    // What just happened. An event is a thing that happened once, so the
    // list is drained after it is read.
    if let Some(game) = &mut session.game {
        for event in game.events.drain(..) {
            if let Some(line) = event_line(event) {
                screen.log.push(line);
            }
            if let Some(freeze) = screen.freeze.as_mut()
                && freeze.downs
                && matches!(event, WorldEvent::DroidDown { .. })
            {
                freeze.left = freeze.left.saturating_sub(1);
            }
            // The engines catch as the ship pushes off its berth, or as a
            // trip begins from a hold; `Sounds` plays one ignition for
            // the undocking and the departure that follows it.
            if matches!(
                event,
                WorldEvent::Undocking { .. }
                    | WorldEvent::LiftedOff { .. }
                    | WorldEvent::Departed { .. }
            ) {
                sounds.engine_start(&mut commands);
            }
            // Down: the window stays black a moment while the settlement
            // is laid out, then the ground is there.
            if matches!(event, WorldEvent::Landed { .. }) {
                screen.blackout = BLACKOUT_HOLD;
            }
            // A level of your own may be a choice to make: the tray comes
            // up on the Skills tab for it (features 80 and 83). Somebody
            // else's slot is their own screen's.
            if matches!(event, WorldEvent::LevelUp { who, .. } if who == screen.net.slot) {
                screen.skills_prompt = true;
            }
        }
        while screen.log.len() > LOG_LINES {
            screen.log.remove(0);
        }
        // Nobody standing: the run is over, and the screen that says so
        // takes over from this one on the next frame.
        if game.world.lost {
            next.set(Screen::Over);
        }
        // What the steps sounded like: the crew's room, and the station's
        // beside it while the decks are joined — its doors and its galley
        // are on the same picture.
        for cued in game.world.aboard.room.take_cues() {
            if let Some(freeze) = screen.freeze.as_mut()
                && !freeze.downs
                && matches!(
                    cued.cue,
                    bims::cue::Cue::Shot { .. } | bims::cue::Cue::Blow { .. }
                )
            {
                freeze.left = freeze.left.saturating_sub(1);
            }
            sounds.play(&mut commands, cued);
        }
        if let Some(residents) = game.world.residents.as_mut() {
            let joined = game.world.ship.state.alongside().is_some();
            for cued in residents.aboard.room.take_cues() {
                if joined {
                    sounds.play(&mut commands, cued);
                }
            }
        }
        // The beds: a planet's air while set down at its settlement — the
        // biome's own — a station's hum while tied up with the rooms
        // joined, the ship's own otherwise, and the engines while they
        // burn — and the world is moving, since a paused burn is silent.
        let ground = game
            .world
            .landed()
            .and_then(|body| game.world.surface(body))
            .map(|surface| surface.biome);
        match (ground, &game.world.ship.state) {
            (Some(biome), _) => sounds.want(Bed::of_biome(biome)),
            (None, ShipState::Docked { .. } | ShipState::CastingOff { .. }) => {
                sounds.want(Bed::Station)
            }
            _ => sounds.want(Bed::Ship),
        }
        if game.world.effort().engines > 0 && game.world.effective_speed().multiplier() > 0 {
            sounds.want(Bed::Engine);
        }
    }
    let prep_timed = crate::perf::scope(crate::perf::Phase::Prep);
    let local = screen.net.slot;
    // Everything that changes the ship or the crew goes through the seam
    // as an order, gathered here and sent below.
    let mut orders: Vec<Order> = Vec::new();
    // `BIMS_FREEZE`: the pause the `||` button gives, so many frames
    // after the shot that was asked for.
    match screen.freeze {
        Some(freeze) if freeze.left == 0 && freeze.frames == 0 => {
            orders.push(Order::Speed(Speed::Paused));
            screen.freeze = None;
        }
        Some(freeze) if freeze.left == 0 => {
            screen.freeze = Some(crate::dev::Freeze {
                frames: freeze.frames - 1,
                ..freeze
            });
        }
        _ => {}
    }

    // The crew's panels, built once there is a room to read, and rebuilt
    // for whoever is there now: a docking brings the station's residents
    // into the ship's room and leaving takes them out.
    let crew_now = session.room_ref().map(|r| r.crew_count()).unwrap_or(0);
    match &mut screen.panels {
        None => {
            let mut panels = CrewPanels::new(local as usize, crew_now);
            orders.push(Order::Crew(CrewOrder::SelectOwn));
            panels.begin_frame();
            screen.panels = Some(panels);
        }
        Some(panels) => {
            panels.rebuild_crew(crew_now);
            panels.begin_frame();
        }
    }
    // The hold, for the pack's rows and the container windows: what is
    // in it, and whether the Bim whose inventory is shown can reach into
    // something that takes each thing. Fresh every frame — the Bim is
    // walking.
    if let Some(panels) = &mut screen.panels {
        let who = session.room_ref().map(|r| panels.inventory_who(r));
        panels.hold = who.map(|who| hold_of(session, who));
        panels.keys = keys_now;
    }
    let crew_count = session
        .game
        .as_ref()
        .map(|g| g.world.aboard.crew_count())
        .unwrap_or(0);
    let resident_station = session.resident_station();
    // The room is the crew's; a second crew — another player's, one day —
    // would come after them and be named for where it is from.
    let name = move |who: u32| -> String {
        if who < crew_count {
            crew_name(who)
        } else {
            resident_name(resident_station.unwrap_or(0), who - crew_count)
        }
    };
    // And what is within reach of the Bim shown, for the nearby strip
    // and the Inventory key.
    if let Some(panels) = &mut screen.panels {
        let who = session.room_ref().map(|r| panels.inventory_who(r));
        panels.nearby = who.map_or_else(Vec::new, |who| nearby_of(session, who, &name));
        // And the player's own class, for the section under the health
        // (feature 74).
        panels.class_view = session.game.as_ref().map(|game| {
            let slot = screen.net.slot;
            let world = &game.world;
            let progress = world.progress_of(slot);
            let class = world.class_of(slot);
            crate::crew::ClassView {
                class,
                level: progress.level(),
                to_next: progress.to_next(),
                pending: progress
                    .pending_pick(class)
                    .and_then(|l| world::class::pick_at(class, l).map(|(a, b)| (l, a, b))),
                picks: progress.picks.clone(),
                talents: progress.talents(class),
                can_change: !world.undocked_once,
                repair: (class == world::Class::Engineer).then(|| world.can_repair(slot)),
                soldier: (class == world::Class::Soldier).then(|| crate::crew::SoldierView {
                    grenades: world.grenades_of(slot),
                    cooldown: world.grenade_cooldown_left(slot),
                    braced: world.is_braced(slot),
                }),
                medic: (class == world::Class::Medic).then(|| crate::crew::MedicView {
                    patients: world.patients_of(slot).iter().map(|&p| name(p)).collect(),
                    charge: world.surge_charge(slot),
                    can_surge: progress.level() >= world::class::SURGE_LEVEL,
                    surging: world.is_surging(slot),
                }),
                tank: (class == world::Class::Tank).then(|| crate::crew::TankView {
                    bulwark: world.is_bulwark(slot),
                    taunt_left: world.taunt_left(slot),
                    cooldown: world.taunt_cooldown_left(slot),
                    can_taunt: progress.level() >= world::class::TAUNT_LEVEL,
                }),
                commander: (class == world::Class::Commander).then(|| {
                    let order = world.squad.as_ref().filter(|o| o.by_slot == slot);
                    crate::crew::CommanderView {
                        squad: order.map(|o| o.kind.code()),
                        members: order.map_or(0, |o| o.members.len()),
                        rally_left: world.rally_left(slot),
                        cooldown: world.rally_cooldown_left(slot),
                        can_rally: progress.level() >= world::class::RALLY_LEVEL,
                    }
                }),
            }
        });
        // A point waiting to be spent opens the tray on the Skills tab,
        // once (feature 83): where the level-up window used to stand over
        // the deck, the tree in the tray is what the player is shown.
        // The flag waits for a pick to actually be there, so an open with
        // no class — or a level with nothing to choose — moves nothing.
        if panels
            .class_view
            .as_ref()
            .is_some_and(|view| view.pending.is_some())
            && std::mem::take(&mut screen.skills_prompt)
        {
            panels.show_skills();
        }
    }

    drop(prep_timed);

    // --- an order on its way to the helm ---------------------------------------
    // The strip at the top walked the crew member to the seat; the order
    // goes through the frame they get there, and the post is lifted once
    // the world has stepped with it — a step later, so they are still at
    // the seat when the command lands.
    if let Some(game) = &mut session.game {
        if screen.relieve && steps > 0 {
            orders.push(Order::Crew(CrewOrder::StandDown { who: local }));
            screen.relieve = false;
        }
        if let Some(order) = screen.pending
            && game.world.at_the_helm(local)
        {
            orders.push(match order {
                HelmOrder::Fly(aim) => Order::Fly(aim.target()),
                HelmOrder::Stop => Order::Stop,
                HelmOrder::Jump(star) => Order::Jump(star),
                HelmOrder::Land => Order::Land,
            });
            if matches!(order, HelmOrder::Fly(_)) {
                screen.aimed = None;
            }
            screen.pending = None;
            screen.relieve = true;
        }
    }

    let canvas_timed = crate::perf::scope(crate::perf::Phase::Canvas);
    // --- the canvas ------------------------------------------------------------
    let canvas = rect_of(root.available_rect_before_wrap());
    let size = canvas.size();
    if size != screen.size && size.x > 0.0 && size.y > 0.0 {
        // The first size the canvas is actually seen at is the one the
        // world should have opened on: the whole hull, in view.
        if screen.size == Vec2::ZERO {
            session.fit(size.x, size.y);
            if let Some(factor) = crate::dev::zoom() {
                session.zoom(size.x / 2.0, size.y / 2.0, factor);
            }
        } else {
            session.resize(size.x, size.y);
        }
        screen.size = size;
    }
    let pointer = Pointer::read(&ctx);
    let on_canvas = pointer.on(canvas);
    let here = pointer.pos.map(|p| p - canvas.min);
    crate::keys::release_tab_focus(
        &ctx,
        &mut screen.tab_took_focus,
        !ctx.egui_wants_keyboard_input() && screen.sheet.is_none(),
    );
    let keys = !ctx.egui_wants_keyboard_input() && screen.sheet.is_none();
    let map_up = session
        .game
        .as_ref()
        .is_some_and(|g| g.mode == ViewMode::Map);
    // The galaxy chart over the system map, once it has been made. Its
    // marks are the world's: the star the ship is at, and the one picked.
    let galaxy_up = map_up && screen.galaxy_up && screen.galaxy.is_some();
    if galaxy_up && let Some(chart) = &mut screen.galaxy {
        chart.here = session.game.as_ref().map(|g| g.world.star_id);
        chart.target = screen.picked_star;
        // And every star the crew have been to, ringed (feature 85).
        chart.visited = session
            .game
            .as_ref()
            .map(|g| g.world.stars_visited())
            .unwrap_or_default();
        // And every star the machines hold, crossed in red (feature 92) —
        // charted or not: the crisis is not a secret.
        chart.infested = session
            .game
            .as_ref()
            .map(|g| g.world.infested_stars())
            .unwrap_or_default();
        // And where a charge could take the ship (feature 93): the lanes
        // out of its own star lit, and the route to the picked one drawn
        // step by step with any jammed step barred.
        chart.reachable = session
            .game
            .as_ref()
            .map(|g| g.world.reachable_stars())
            .unwrap_or_default();
        let plotted = session
            .game
            .as_ref()
            .zip(screen.picked_star)
            .and_then(|(g, star)| g.world.route_to(star).map(|route| (g, route)));
        (chart.route, chart.jammed) = match plotted {
            Some((g, route)) => {
                let jammed = route
                    .windows(2)
                    .map(|pair| g.world.jammed_step(pair[0], pair[1]))
                    .collect();
                (route, jammed)
            }
            None => (Vec::new(), Vec::new()),
        };
        if canvas.size() != screen.galaxy_size {
            screen.galaxy_size = canvas.size();
            chart.preview.resize(canvas.size().x, canvas.size().y);
        }
    }
    let panels = screen.panels.as_mut().unwrap();

    // Where this pointer is over the deck, to the room, as a design point
    // — nothing over a panel, and nothing with the map up, where a tile
    // means nothing.
    online.point(
        now,
        on_canvas
            .filter(|_| !map_up)
            .map(|p| session.design_point(p.x, p.y)),
    );
    // The blueprint in hand — the Build tab's tool — is the ship view's:
    // the painter draws it under the pointer there, and on the map a tile
    // means nothing. The turn it has is kept across a change of part; the
    // world's answer about the tile is kept until the part or the tile
    // moves, so the tool is only set when it changes.
    let building = match panels.tool {
        Some(Tool::Build(kind)) if !map_up => Some(kind),
        _ => None,
    };
    if let Some(game) = &mut session.game
        && game.placing.map(|(kind, _)| kind) != building
    {
        let rotation = game
            .placing
            .map(|(_, rotation)| rotation)
            .unwrap_or(Rotation::R0);
        game.set_placing(building.map(|kind| (kind, rotation)));
    }
    let mut actions = Actions {
        overlay: session.game.as_ref().map(|g| g.overlay).unwrap_or_default(),
        crafts: crafts(session),
        keep: Vec::new(),
        at_rest: session.game.as_ref().is_some_and(|g| g.world.at_rest()),
        free_money: session
            .game
            .as_ref()
            .map(|g| g.world.free_money())
            .unwrap_or(0),
        sites: sites(session),
        cancel: Vec::new(),
        head_up: session.game.as_ref().is_some_and(|g| g.head_up),
        follow: session.game.as_ref().is_some_and(|g| g.follow),
        docked: session.docked_at().is_some(),
        station: false,
        research: research_view(session),
        research_orders: Vec::new(),
        auto_upgrade: session
            .game
            .as_ref()
            .is_some_and(|g| g.world.auto_upgrade()),
        set_auto_upgrade: None,
        upgrade: upgrade_view(session),
    };

    // Middle drags pan, in either view.
    if let Some(p) = on_canvas
        && pointer.middle_pressed
    {
        screen.pan_from = Some(p);
    }
    if let Some(from) = screen.pan_from {
        match here {
            Some(p) if pointer.middle_down => {
                match &mut screen.galaxy {
                    Some(chart) if galaxy_up => chart.preview.pan(p.x - from.x, p.y - from.y),
                    _ => session.pan(p.x - from.x, p.y - from.y),
                }
                screen.pan_from = Some(p);
            }
            _ => screen.pan_from = None,
        }
    }
    if let Some(p) = on_canvas
        && pointer.scroll != 0.0
    {
        match &mut screen.galaxy {
            Some(chart) if galaxy_up => chart.preview.zoom(p.x, p.y, zoom_factor(pointer.scroll)),
            _ => session.zoom(p.x, p.y, zoom_factor(pointer.scroll)),
        }
    }

    if map_up {
        screen.hover_at = None;
        if let Some(game) = &mut session.game {
            game.hover = None;
        }
        // On the chart, the pointer is over stars: the one under it is
        // rung, and a click picks it — its system goes into the strip,
        // and it is where Jump goes. Picking is looking, like aiming.
        if galaxy_up {
            if let Some(chart) = &mut screen.galaxy {
                match here.filter(|_| on_canvas.is_some()) {
                    Some(p) => chart.hover(p.x, p.y),
                    None => chart.hovered = None,
                }
                if on_canvas.is_some()
                    && pointer.primary_pressed
                    && let Some(star) = chart.hovered
                {
                    chart.inspect(star);
                    screen.picked_star = Some(star);
                }
            }
        }
        // Plot a trip to whatever a click on the map landed on — a thing,
        // or the empty space beside it, which is a perfectly good place to
        // go. Aiming is looking; it is Confirm that wants the helm.
        else if let Some(p) = on_canvas
            && pointer.primary_pressed
        {
            let game = session.game.as_mut().unwrap();
            screen.aimed = Some(match game.pick(p.x, p.y, MAP_PICK_SLOP) {
                Some(node) => Aim::Node(node),
                None => {
                    let at = game.point_at(p.x, p.y);
                    Aim::Point(at.x, at.y)
                }
            });
        }
    } else {
        // In the ship view the pointer is over the room aboard, and it does
        // what it does on the room's own screen: a left click or a marquee
        // selects a Bim, a right click on the deck sends the one that takes
        // orders there, and a click on a fixture opens its menu. The
        // marquee is drawn on the deck rather than on the glass — it turns
        // with the ship — which is the box the room tests the crew against.
        screen.hover_at = pointer
            .pos
            .filter(|p| canvas.contains(*p))
            .map(|p| p - canvas.min);
        if let Some(game) = &mut session.game {
            game.hover = screen.hover_at.map(|p| game.tile_at(p.x, p.y));
        }
        // Nothing on the deck is lit until the pointer is found over it.
        if let Some(room) = session.room() {
            room.set_hover_dropped(None);
        }
        // With the attack key pressed the pointer is about one thing and
        // nothing else (feature 84): the system's cursor goes and a red
        // crosshair is drawn in its place, below, a left click puts the
        // banner down where it points, and a right-click thinks better
        // of it. The Mine tool's shape, and for the same reason — a
        // click that both selected a Bim and sent the crew somewhere
        // would be a click nobody could undo.
        if screen.aiming_attack {
            if let Some(p) = on_canvas {
                ctx.set_cursor_icon(egui::CursorIcon::None);
                if pointer.primary_pressed {
                    let (rx, ry) = session.room_point(p.x, p.y);
                    if let Some(game) = &session.game {
                        let t = shipdesign::TILE as f32;
                        let tile = ((rx / t).floor() as i32, (ry / t).floor() as i32);
                        let (order, line) = orders_key(
                            &game.world,
                            screen.net.slot,
                            world::Standing::Attack { tile },
                        );
                        orders.extend(order);
                        screen.log.extend(line);
                    }
                    screen.aiming_attack = false;
                }
                if pointer.secondary_pressed {
                    screen.aiming_attack = false;
                }
            }
        // With a blueprint in hand the pointer is about laying it out: a
        // click on a tile it would go on sends the site through the seam,
        // one it would not says why in the log — the readout at the top
        // left says it already — and a right-click puts the tool down.
        } else if let Some(kind) = building {
            if let Some(p) = on_canvas {
                if pointer.primary_pressed
                    && let Some(game) = &mut session.game
                {
                    let (x, y) = game.tile_at(p.x, p.y);
                    match game.ghost_check() {
                        Some(Ok(())) if x >= 0 && y >= 0 => orders.push(Order::Build {
                            kind,
                            x: x as u32,
                            y: y as u32,
                            rotation: game
                                .placing
                                .map(|(_, rotation)| rotation)
                                .unwrap_or(Rotation::R0),
                        }),
                        Some(Err(why)) => screen.log.push(site_refusal_line(why)),
                        _ => {}
                    }
                }
                if pointer.secondary_pressed {
                    panels.tool = None;
                }
            }
        } else if let Some(p) = on_canvas {
            let (rx, ry) = session.room_point(p.x, p.y);
            // The gun under the pointer, ringed — worked out afresh every
            // frame, like the highlight, so one walked off with is not
            // left lit.
            let who = session.room().map(|room| panels.inventory_who(room));
            if let Some(room) = session.room() {
                let under = room.dropped_at(rx, ry);
                room.set_hover_dropped(under);
            }
            if pointer.secondary_pressed {
                panels.close_menu();
                if let Some(room) = session.room() {
                    let fixture = room.hit_at(rx, ry);
                    // A gun on the deck is picked up by the right-click
                    // itself — no menu — into the pack of the Bim shown.
                    if fixture == HIT_DROPPED {
                        let id = room.hit_dropped();
                        if let Some(who) = who {
                            if room.can_fetch(who, id) {
                                orders.push(crew_order(
                                    CrewOrder::PickUp {
                                        who: who as u32,
                                        item: id,
                                    },
                                    pointer.shift,
                                ));
                            } else {
                                screen.log.push(PICK_UP_REFUSED.into());
                            }
                        }
                    } else if fixture != 0 {
                        let at = pointer.pos.unwrap();
                        panels.open_menu(fixture, egui::pos2(at.x, at.y), room);
                    }
                    // A doorway is somewhere to stand as well as something
                    // to work: the door's menu opens *and* the order goes
                    // through, so a right-click on one walks the Bim into
                    // it. Every other fixture keeps the click for its menu
                    // — a body included, living (`HIT_BIM`, the bandage
                    // menu), dead (`HIT_BODY`) or one of the station's
                    // people down in its own room (`HIT_VISITOR`, the Loot
                    // row): never the deck it lies on.
                    // The order is given when the button comes up: held and
                    // dragged, it is a line the crew form along.
                    if fixture == 0 || fixture == HIT_SHIP_DOOR || fixture == HIT_DOOR {
                        screen.order_from = Some(p);
                        room.order_drag_begin(rx, ry);
                    }
                }
            }
            if pointer.primary_pressed {
                panels.close_menu();
                screen.marquee_from = Some(p);
                if let Some(room) = session.room() {
                    room.drag_begin(rx, ry);
                }
            }
        }
        if let Some(from) = screen.marquee_from {
            match here {
                Some(p) => {
                    let (rx, ry) = session.room_point(p.x, p.y);
                    if let Some(room) = session.room() {
                        room.drag_update(rx, ry);
                        if pointer.primary_released {
                            // The picture of the marquee is this window's;
                            // the pick itself is an order, the same on
                            // every player's copy of the room. A click on
                            // a fixture is that fixture's menu here, and
                            // nobody's pick there.
                            room.drag_cancel();
                            let (fx, fy) = session.room_point(from.x, from.y);
                            orders.push(Order::Crew(CrewOrder::Select {
                                x0: fx,
                                y0: fy,
                                x1: rx,
                                y1: ry,
                            }));
                            let room = session.room().unwrap();
                            let moved = (p - from).length() > CLICK_SLOP;
                            let mut fixture = if moved { 0 } else { room.hit_at(rx, ry) };
                            // A living crew member under a left click is
                            // picked, not menued: only a body down is a
                            // window.
                            if fixture == HIT_BIM && !room.is_down(room.hit_bim()) {
                                fixture = 0;
                            }
                            if fixture != 0 {
                                // A body — dead, out cold, or one of the
                                // station's people down — opens its inventory
                                // straight off; anything else its menu.
                                if let Some(source) = panels.body_under_click(room, fixture) {
                                    panels.open_loot(source);
                                } else {
                                    let at = pointer.pos.unwrap();
                                    panels.open_menu(fixture, egui::pos2(at.x, at.y), room);
                                }
                            }
                            screen.marquee_from = None;
                        }
                    }
                }
                None => {
                    screen.marquee_from = None;
                    if let Some(room) = session.room() {
                        room.drag_cancel();
                    }
                }
            }
        }
        if let Some(from) = screen.order_from {
            match here {
                Some(p) => {
                    let (rx, ry) = session.room_point(p.x, p.y);
                    if let Some(room) = session.room() {
                        room.order_drag_update(rx, ry);
                        if pointer.secondary_released {
                            // The line drawn is this window's; the order
                            // is everybody's. A walk with no way there
                            // comes back as a `Refused` event.
                            room.order_drag_cancel();
                            let dragged = (p - from).length() > CLICK_SLOP;
                            let (fx, fy) = session.room_point(from.x, from.y);
                            let walk = if dragged {
                                CrewOrder::Line {
                                    x0: fx,
                                    y0: fy,
                                    x1: rx,
                                    y1: ry,
                                }
                            } else {
                                CrewOrder::Move { x: rx, y: ry }
                            };
                            // With Shift held the walk waits its turn
                            // behind what the crew are on (feature 69).
                            orders.push(crew_order(walk, pointer.shift));
                            screen.order_from = None;
                        }
                    }
                }
                None => {
                    screen.order_from = None;
                    if let Some(room) = session.room() {
                        room.order_drag_cancel();
                    }
                }
            }
        }
    }

    if keys {
        let mut d = Vec2::ZERO;
        ctx.input(|i| {
            if let Some(game) = &mut session.game {
                if keys_now.pressed(i, Action::Map) {
                    game.set_mode(if game.mode == ViewMode::Map {
                        ViewMode::Ship
                    } else {
                        ViewMode::Map
                    });
                }
                if keys_now.pressed(i, Action::NorthUp) {
                    game.head_up = !game.head_up;
                }
                if keys_now.pressed(i, Action::Follow) {
                    let on = !game.follow;
                    game.set_follow(on);
                }
            }
            if !i.modifiers.any() {
                // The speed keys: Space pauses and goes back to what it paused
                // from, the digits pick a speed. Orders, like the buttons.
                if keys_now.pressed(i, Action::Pause)
                    && let Some(game) = &session.game
                {
                    let mine = game.requested(screen.net.slot);
                    if mine == Speed::Paused {
                        orders.push(Order::Speed(screen.resume));
                    } else {
                        screen.resume = mine;
                        orders.push(Order::Speed(Speed::Paused));
                    }
                }
                for (action, speed) in [
                    (Action::Speed1, Speed::Real),
                    (Action::Speed3, Speed::Triple),
                    (Action::Speed10, Speed::Ten),
                    (Action::Speed24, Speed::Day),
                    (Action::SpeedTop, Speed::Top),
                ] {
                    if keys_now.pressed(i, action) {
                        orders.push(Order::Speed(speed));
                    }
                }
                // Select: the crew member you steer, selected and in the middle.
                if keys_now.pressed(i, Action::Select) {
                    orders.push(Order::Crew(CrewOrder::SelectOwn));
                    if let Some(game) = &mut session.game {
                        game.centre_on_player();
                    }
                }
                // Turn turns the blueprint in hand; with none, Recruit recruits —
                // the two are one key by default, told apart by the hand.
                if building.is_some() {
                    if keys_now.pressed(i, Action::Turn)
                        && let Some(game) = &mut session.game
                    {
                        game.rotate_placing();
                    }
                } else if keys_now.pressed(i, Action::Recruit) {
                    orders.push(Order::Crew(CrewOrder::Recruit));
                }
                // Tab: the inventory of the crew member you steer.
                if keys_now.pressed(i, Action::Inventory) {
                    panels.toggle_inventory();
                }
                // Q and E: the steered crew member's class's two actions
                // (features 74 and 75) — an engineer's sentry and sandbags
                // on the deck tile under the pointer, a soldier's grenade
                // at it and its brace. The world says why not, into the
                // log; with a classless crew member steered, nothing at
                // all.
                // While Q is held with a soldier steered, the burst's ring
                // is drawn on the tile under the pointer.
                screen.throw_aim = None;
                if keys_now.down(i, Action::ClassPrimary)
                    && let Some(game) = &session.game
                    && game.world.class_of(screen.net.slot) == world::Class::Soldier
                    && let Some(p) = on_canvas.filter(|_| !map_up)
                {
                    let (rx, ry) = session.room_point(p.x, p.y);
                    let t = shipdesign::TILE as f32;
                    screen.throw_aim = Some(((rx / t).floor() as i32, (ry / t).floor() as i32));
                }
                for action in [Action::ClassPrimary, Action::ClassSecondary] {
                    if !keys_now.pressed(i, action) {
                        continue;
                    }
                    let slot = screen.net.slot;
                    let Some(game) = &session.game else {
                        continue;
                    };
                    let room = on_canvas
                        .filter(|_| !map_up)
                        .map(|p| session.room_point(p.x, p.y));
                    let tile = room.map(|(rx, ry)| {
                        let t = shipdesign::TILE as f32;
                        ((rx / t).floor() as i32, (ry / t).floor() as i32)
                    });
                    // And the crew member under the pointer, for a
                    // medic's beam (feature 76).
                    let under = room
                        .and_then(|(rx, ry)| game.world.aboard.room.crew_at(rx, ry))
                        .map(|who| who as u32);
                    // And the enemy under it, for a commander's attack
                    // (feature 78).
                    let enemy = room.and_then(|(rx, ry)| game.world.resident_at(rx, ry));
                    let (order, line) = class_key(
                        &game.world,
                        slot,
                        action == Action::ClassPrimary,
                        tile,
                        under,
                        enemy,
                    );
                    orders.extend(order);
                    screen.log.extend(line);
                }
                // The commander's other two squad keys (feature 78): X
                // calls the squad back to the tile under the pointer —
                // to him with the pointer on nothing — and Z has it
                // hold where it stands. They do nothing for any other
                // class.
                for action in [Action::SquadFallBack, Action::SquadStandGround] {
                    if !keys_now.pressed(i, action) {
                        continue;
                    }
                    let slot = screen.net.slot;
                    let Some(game) = &session.game else {
                        continue;
                    };
                    if game.world.class_of(slot) != world::Class::Commander {
                        continue;
                    }
                    let tile = on_canvas.filter(|_| !map_up).map(|p| {
                        let (rx, ry) = session.room_point(p.x, p.y);
                        let t = shipdesign::TILE as f32;
                        ((rx / t).floor() as i32, (ry / t).floor() as i32)
                    });
                    let ask = if action == Action::SquadFallBack {
                        world::SquadAsk::FallBack { tile }
                    } else {
                        world::SquadAsk::StandGround
                    };
                    let (order, line) = squad_key(&game.world, slot, ask);
                    orders.extend(order);
                    screen.log.extend(line);
                }
                // The medic's carry (feature 86): the crewmate under the
                // pointer up into its arms, or — with its arms already
                // full, or with the pointer on nobody — set down. With
                // nobody under the pointer and nobody in its arms it
                // takes up the nearest it could, so the key is worth
                // pressing without aiming it in the middle of a fight.
                if keys_now.pressed(i, Action::Carry)
                    && let Some(game) = &session.game
                {
                    let slot = screen.net.slot;
                    let under = on_canvas
                        .filter(|_| !map_up)
                        .map(|p| session.room_point(p.x, p.y))
                        .and_then(|(rx, ry)| game.world.aboard.room.crew_at(rx, ry))
                        .map(|who| who as u32);
                    let (order, line) = carry_key(&game.world, slot, under);
                    orders.extend(order);
                    screen.log.extend(line);
                }
                // And every player's own two (feature 84). **Attack**
                // arms the pointer rather than doing anything: the
                // banner goes down on the click after it, below, so
                // that the player picks the ground. Pressed while the
                // pointer is already armed, it is thought better of.
                //
                // **With a banner already down it is the banner taken
                // up**, which is the key's second press and the only
                // way there is to it: the world reads the *same* order
                // given again as a release (`Standing::same_as`), and
                // the same order means the same *tile* — a second
                // banner anywhere else is a fresh attack, so a crew
                // left under one after a fight would stand under arms
                // at it for ever, taking no errand.
                if keys_now.pressed(i, Action::Attack) {
                    let standing = session
                        .game
                        .as_ref()
                        .map(|game| game.world.standing_of(screen.net.slot))
                        .unwrap_or_default();
                    let (release, armed) = attack_key(standing, screen.aiming_attack, map_up);
                    screen.aiming_attack = armed;
                    if let (Some(order), Some(game)) = (release, &session.game) {
                        let (order, line) = orders_key(&game.world, screen.net.slot, order);
                        orders.extend(order);
                        screen.log.extend(line);
                    }
                }
                // **Retreat** is the order itself: there is nothing to
                // point at, since the ship is where they go.
                if keys_now.pressed(i, Action::Retreat)
                    && let Some(game) = &session.game
                {
                    screen.aiming_attack = false;
                    let (order, line) =
                        orders_key(&game.world, screen.net.slot, world::Standing::Retreat);
                    orders.extend(order);
                    screen.log.extend(line);
                }
            }
            if i.key_pressed(egui::Key::Escape) {
                if screen.aiming_attack {
                    // The armed pointer is put away first, and nothing
                    // else happens — the Mine tool's rule.
                    screen.aiming_attack = false;
                } else if panels.tool.is_some() {
                    // A tool in hand is put down first, and nothing else
                    // happens.
                    panels.tool = None;
                } else if panels.escape() {
                    // A menu, a container window or the trade window is
                    // shut without opening the sheet.
                } else if screen.trading {
                    screen.trading = false;
                } else {
                    screen.aimed = None;
                    screen.sheet = Some(Sheet::Menu);
                }
            }
            let step = super::designer::PAN_SPEED * dt as f32;
            if keys_now.down(i, Action::PanLeft) {
                d.x += step;
            }
            if keys_now.down(i, Action::PanRight) {
                d.x -= step;
            }
            if keys_now.down(i, Action::PanUp) {
                d.y += step;
            }
            if keys_now.down(i, Action::PanDown) {
                d.y -= step;
            }
        });
        if d != Vec2::ZERO {
            session.pan(d.x, d.y);
        }
    } else if screen.sheet.is_some()
        && keys_now.listening.is_none()
        && ctx.input(|i| i.key_pressed(egui::Key::Escape))
    {
        // Esc while the controls page waits on a key is that page's.
        screen.sheet = None;
    }
    if screen.aimed.is_none() {
        session.clear_preview();
    }

    // Everything that changes the ship goes through the seam.
    for order in orders.drain(..) {
        screen.net.order(session, order);
    }

    drop(canvas_timed);
    let panels_timed = crate::perf::scope(crate::perf::Phase::Panels);
    // --- the left stack: items, the readout, the pointer, the agendas ----------
    let game = session.game.as_ref().unwrap();
    let view_name = match game.mode {
        ViewMode::Ship => "Ship",
        ViewMode::Map => "System map",
    };
    let state = game.world.ship.state.code();
    let phase = if state == 2 {
        if game.world.plan().is_some_and(|p| p.aborting) {
            "Stopping".to_string()
        } else {
            PHASE_NAMES
                .get(ship::world_paint::phase_code(game) as usize)
                .copied()
                .unwrap_or("Under way")
                .to_string()
        }
    } else {
        state_name(game)
    };
    let speed = game.world.trip_state().map(|s| s.speed).unwrap_or(0.0);
    let degrees = (game.world.ship.heading.to_degrees() + 360.0) % 360.0;
    // What the pointer is over, in the ship view: the part under it and
    // the tile, and what the room aboard makes of the same point.
    // A blueprint in hand names itself and says whether it would go; a
    // site under the pointer names the part and what has reached it.
    let ghost = building.and_then(|kind| Some((kind, game.ghost_answer()?)));
    let site_under = game.hover.and_then(|tile| game.site_at(tile));
    let (hover_thing, hover_on_it) = match screen.hover_at {
        Some(_) if !map_up && ghost.is_some() => {
            let (kind, answer) = ghost.unwrap();
            let tile = game.hover.unwrap_or((0, 0));
            (
                format!("{} · {}, {}", part_name(kind), tile.0, tile.1),
                match answer {
                    Ok(()) => "Click to lay it out".to_string(),
                    Err(why) => site_refusal_line(why),
                },
            )
        }
        Some(_) if !map_up && site_under.is_some() => {
            let site = site_under.unwrap();
            (
                format!("Blueprint · {}", part_name(site.kind)),
                site_progress(site, &game.world.ship.design),
            )
        }
        Some(p) if !map_up && session.game_tile_inside() => {
            let part = session.game_hovered_part();
            let mut thing = part
                .and_then(|id| session.part_kind(id))
                .map(part_name)
                .unwrap_or("—")
                .to_string();
            let (rx, ry) = session.room_point(p.x, p.y);
            let room = session.room_ref().unwrap();
            let (spot, room_thing, on_it) = CrewPanels::spot_readout(room, rx, ry);
            if !PLAIN_SPOTS.contains(&spot) {
                thing = room_thing;
            }
            let tile = game.hover.unwrap_or((0, 0));
            (format!("{thing} · {}, {}", tile.0, tile.1), on_it)
        }
        _ => (String::new(), String::new()),
    };

    // The name of the game, the clock and the speed, each in a little
    // frame of its own along the top. An area of its own rather than the
    // first row of the stack under it: egui counts the whole of an area's
    // bounding box as its own for the pointer, so a row this wide over a
    // stack this tall would make a dead rectangle of the ship's left half
    // — a click there would be nobody's.
    let top = egui::Area::new(egui::Id::new("game-top-left"))
        .fixed_pos(egui::pos2(canvas.min.x + 10.0, canvas.min.y + 10.0))
        .order(egui::Order::Middle)
        .show(&ctx, |ui| {
            ui.horizontal(|ui| {
                panel_frame().show(ui, |ui| {
                    ui.heading("Bims");
                });
                panel_frame().show(ui, |ui| {
                    clock_panel(ui, session, &screen.net, &mut orders);
                });
                // The machines, in the raid's red (feature 83): which
                // wave holds the station alongside, and how long until
                // the next lands. **Before the alarm and the rest**,
                // which are up for the whole of a droid fight and would
                // push this line under the crew's panels — this is the
                // one of them the player has no other way of knowing.
                if let Some(game) = &session.game {
                    droid_warning(ui, &game.world);
                }
                // The recruited panel says why the gun has gone quiet
                // while a blade is at the player's crew member: the same
                // panel, since nothing on the screen may grow — the left
                // stack already fills a short window, and a panel added
                // to it shoves the whole stack up over this row.
                if let Some(room) = session.room_ref()
                    && room.is_recruited(local)
                {
                    let locked =
                        room.is_alive(local as usize) && room.is_locked(local as usize).is_some();
                    panel_frame().show(ui, |ui| {
                        ui.horizontal(|ui| {
                            if locked {
                                ui.label(
                                    egui::RichText::new(format!("Recruited — {LOCKED_STATUS}"))
                                        .color(theme::CAUTION),
                                );
                                theme::question_mark(ui, &locked_tip());
                            } else {
                                ui.label(
                                    egui::RichText::new("Recruited — the crew follow you")
                                        .color(theme::ACCENT),
                                );
                                // And what that now does to everybody
                                // else (feature 84): the crew come with
                                // you, under arms, until you holster.
                                theme::question_mark(ui, ORDERS_TIP);
                            }
                        });
                    });
                }
                // And the alarm: the rest of the crew under arms of their own
                // accord, an enemy near or one of them hit.
                if let Some(room) = session.room_ref()
                    && room.is_alarmed()
                {
                    panel_frame().show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.label(egui::RichText::new(ALARM_STATUS).color(theme::CAUTION));
                            theme::question_mark(ui, ALARM_TIP);
                        });
                    });
                }
                // And what this player's own standing order to the crew
                // is, while it is anything but following (feature 84):
                // the banner or the fall back, in the banner's red.
                if let Some(game) = &session.game
                    && let Some(line) = orders_line(game.world.standing_of(screen.net.slot).code())
                {
                    panel_frame().show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.label(egui::RichText::new(line).color(theme::ATTACK));
                            theme::question_mark(ui, ORDERS_TIP);
                        });
                    });
                }
                // And the raid, in red: the raider closing with the
                // time it has left counted down, then its boarders at
                // the locked airlock, forcing it, and through it.
                if let Some(game) = &session.game {
                    raid_warning(ui, &game.world);
                }
            });
        })
        .response
        .rect;
    egui::Area::new(egui::Id::new("game-left"))
        .fixed_pos(egui::pos2(canvas.min.x + 10.0, top.max.y + 4.0))
        .order(egui::Order::Middle)
        .show(&ctx, |ui| {
            panel_frame().show(ui, |ui| {
                ui.set_max_width(260.0);
                items_panel(ui, session, panels);
            });
            panel_frame().show(ui, |ui| {
                ui.set_max_width(260.0);
                ui.set_min_width(200.0);
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new(view_name).strong());
                    ui.label(egui::RichText::new(&phase).color(theme::MUTED));
                });
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new(format!("{speed:.1} u/min")).color(theme::MUTED));
                    ui.label(egui::RichText::new(format!("{degrees:.0}°")).color(theme::MUTED));
                });
                if !hover_thing.is_empty() {
                    ui.label(&hover_thing);
                    if !hover_on_it.is_empty() {
                        ui.label(
                            egui::RichText::new(&hover_on_it)
                                .small()
                                .color(theme::MUTED),
                        );
                    }
                }
            });
            if let Some(room) = session.room_ref() {
                let has_agenda = (0..panels.crew_count).any(|w| room.agenda_len(w as usize) > 0);
                if has_agenda {
                    panel_frame().show(ui, |ui| {
                        ui.set_min_width(200.0);
                        panels.agendas(ui, room, &name);
                    });
                }
            }
        });
    for order in orders.drain(..) {
        screen.net.order(session, order);
    }

    // --- the strip across the top: where the ship is, and where it is going ----
    // Centred on the canvas, but never over the row of frames at the top
    // left: that row grows a "Recruited" panel in combat, and at the
    // default window width the two met in the middle. Its width is last
    // frame's (the first frame guesses the minimum), which is what egui's
    // own anchoring reads too.
    let trip_id = egui::Id::new("game-trip");
    let trip_width = ctx
        .memory(|m| m.area_rect(trip_id).map(|r| r.width()))
        .unwrap_or(300.0);
    let trip_x = ((canvas.min.x + canvas.max.x - trip_width) / 2.0).max(top.max.x + 8.0);
    egui::Area::new(trip_id)
        .fixed_pos(egui::pos2(trip_x, canvas.min.y + 10.0))
        .order(egui::Order::Middle)
        .show(&ctx, |ui| {
            panel_frame().show(ui, |ui| {
                ui.set_min_width(300.0);
                trip_panel(
                    ui,
                    session,
                    local,
                    map_up,
                    &mut screen.aimed,
                    &mut screen.pending,
                    &mut screen.relieve,
                    &mut screen.log,
                    &mut Chart {
                        up: &mut screen.galaxy_up,
                        lobby: &mut screen.galaxy,
                        picked: &mut screen.picked_star,
                    },
                    &mut orders,
                );
            });
        });

    egui::Area::new(egui::Id::new("game-tray"))
        .anchor(
            egui::Align2::LEFT_BOTTOM,
            egui::vec2(canvas.min.x + 10.0, -10.0),
        )
        .order(egui::Order::Middle)
        .show(&ctx, |ui| {
            tray_frame().show(ui, |ui| {
                let ship_tab = match session.room() {
                    Some(room) => panels.tray(ui, room, Some(&mut actions), &name),
                    None => false,
                };
                if ship_tab {
                    ship_panel(ui, session, &screen.net, local, screen.pending.is_some());
                }
            });
        });
    if let Some(game) = &mut session.game {
        game.head_up = actions.head_up;
        if game.follow != actions.follow {
            game.set_follow(actions.follow);
        }
    }
    // The Station button opens the trade window and shuts it again; so
    // does leaving the berth, since there is nobody to trade with then.
    if actions.station {
        screen.trading = !screen.trading;
    }
    // So does the Trade row on the station's desk, which walked the Bim
    // over as well.
    if std::mem::take(&mut panels.trade_requested) {
        screen.trading = true;
    }
    if !actions.docked {
        screen.trading = false;
    }
    if screen.trading {
        trade_window(
            &ctx,
            session,
            local,
            &mut screen.trading,
            &mut screen.cart,
            &mut orders,
        );
    }
    // A cart is this window's, at this berth: shutting the window, or
    // casting off with it up, is walking away from the desk with nothing
    // agreed.
    if !screen.trading {
        screen.cart.clear();
    }
    for (resource, units) in actions.keep.drain(..) {
        orders.push(Order::Keep { resource, units });
    }
    for site in actions.cancel.drain(..) {
        orders.push(Order::Cancel { site });
    }
    if let Some(on) = actions.set_auto_upgrade.take() {
        orders.push(Order::AutoUpgrade(on));
    }
    // The workbench window's button.
    if std::mem::take(&mut panels.upgrade_requested) {
        orders.push(Order::Upgrade);
    }
    // The class section's and the deployable rows' (feature 74).
    for order in panels.deploy_orders.drain(..) {
        orders.push(match order {
            crate::crew::DeployOrder::PackUp(id) => Order::PackUp(id),
            crate::crew::DeployOrder::Pick { level, side } => Order::PickTalent { level, side },
            crate::crew::DeployOrder::SetClass(class) => Order::SetClass(class),
            crate::crew::DeployOrder::Repair => Order::Repair,
        });
    }
    for order in actions.research_orders.drain(..) {
        orders.push(Order::Research(order));
    }
    if let Some(game) = &mut session.game {
        game.overlay = actions.overlay;
    }
    for order in orders.drain(..) {
        screen.net.order(session, order);
    }

    // Clear of the left stack on a narrow window, rather than over it. The
    // strip is as wide as the health block wants: a part's bar, its number
    // and the trauma holding it at nothing, in one row. And under the row
    // of frames along the top rather than beside it — a panel this wide
    // reaches the middle of the window, where the trip strip is.
    let side_x = (canvas.max.x - (crate::crew::SIDE_W + 30.0)).max(canvas.min.x + 290.0);
    egui::Area::new(egui::Id::new("game-side"))
        .fixed_pos(egui::pos2(side_x, top.max.y + 4.0))
        .order(egui::Order::Middle)
        .show(&ctx, |ui| {
            if let Some(room) = session.room() {
                let mut drawn = false;
                let frame = panel_frame();
                frame.show(ui, |ui| {
                    drawn = panels.side(ui, room, &name);
                    if !drawn {
                        ui.label(
                            egui::RichText::new("Click a Bim to look at it.").color(theme::MUTED),
                        );
                    }
                });
            }
        });

    // The two boxes at the foot of the canvas: what the class's own keys
    // do, how many are left and what each of them is (feature 80).
    // Nothing for a classless crew member, which has no keys — but
    // everybody has the medicine's two past them, the medkit and the
    // bandages it carries, with their cooldowns swept the way Dota 2
    // sweeps a skill's.
    // And whom the box the pointer rests on would reach (feature 86),
    // for the ring on the deck below — worked out afresh every frame off
    // what is hovered, the way the panels' highlight is, so a bar that
    // folds away under the pointer cannot leave one lit.
    let mut cast_reaches: Vec<u32> = Vec::new();
    if let Some(game) = &session.game {
        let boxes = ability_boxes(&game.world, local, &keys_now);
        let medicine = medicine_boxes(&game.world, local);
        if let Some(action) = ability_bar(&ctx, canvas, &boxes, &medicine) {
            cast_reaches = affected_by(&game.world, local, action);
        }
    }

    if !screen.log.is_empty() {
        egui::Area::new(egui::Id::new("game-log"))
            .anchor(
                egui::Align2::RIGHT_BOTTOM,
                egui::vec2(
                    -(size.x * 0.0) - 10.0 - (ctx.viewport_rect().max.x - canvas.max.x),
                    -10.0,
                ),
            )
            .order(egui::Order::Middle)
            .show(&ctx, |ui| {
                panel_frame().show(ui, |ui| {
                    for line in &screen.log {
                        ui.label(egui::RichText::new(line).small());
                    }
                });
            });
    }

    if let Some(room) = session.room() {
        if let Some(what) = screen.armoury_wanted.take() {
            panels.open_named(room, &what);
        }
        panels.menu(&ctx, room, &name);
        panels.container_window(&ctx, room, &name);
    }
    // The body under the Loot window, after the menu — which is what
    // opens it — and fresh every frame: the looter is walking, and a
    // crewmate out cold may come round. The walk over is the world's to
    // start, since where one of the station's people lies is a point of
    // its own room put through the station's frame.
    if let Some(game) = &mut session.game {
        let world = &mut game.world;
        let who = panels.inventory_who(&world.aboard.room);
        if let Some(source) = panels.walk.take()
            && let Some(at) = world.body_position(source)
        {
            orders.push(Order::Crew(CrewOrder::SendTo {
                who: who as u32,
                x: at.x,
                y: at.y,
            }));
        }
        // Whose the station alongside is, for the Kill row on a body among
        // its people: an enemy's, or not.
        panels.enemies_alongside = world
            .residents
            .as_ref()
            .is_some_and(|r| world.stance(r.station) == bims::sight::Stance::Hostile);
        panels.body = panels
            .loot_source()
            .and_then(|source| body_of(world, who, source));
        // And the enemy's shelf under the Plunder window — and under a
        // click on one of the station's shelves, which is what tells the
        // panels a station shelf is loot rather than a friend's: laid out
        // by the world, read back every frame with whether the Bim shown
        // is within reach of one.
        panels.shelf = world.plunder_alongside().map(|p| crate::crew::Shelf {
            grid: p.grid.clone(),
            capacity: p.capacity,
            reach: world.shelf_ashore_in_reach(who as u32),
        });
        // And the mercenary under the Hire window, the same way: for hire
        // still, and what it asks, read off the world every frame.
        panels.terms = panels.hire_source().and_then(|resident| {
            let offer = world.hire_offer(who as u32, resident)?;
            let gear = world
                .residents
                .as_ref()?
                .aboard
                .room
                .gear(resident as usize);
            Some(crate::crew::Terms {
                fee: offer.fee,
                gear,
                in_reach: offer.in_reach,
                affordable: offer.affordable,
                bunk: offer.bunk,
                medic: offer.medic,
            })
        });
        // The station's key: the desk's row walked the Bim over, and the
        // take goes through the seam the frame they are within reach —
        // or is forgotten if the key goes or the ship does.
        if let Some(who) = panels.key_requested {
            if world.key_at_the_dock() == 0 {
                panels.key_requested = None;
            } else if world.key_in_reach(who) {
                orders.push(Order::Gear(GearOrder::TakeKey { who }));
                panels.key_requested = None;
            }
        }
        let room = &mut world.aboard.room;
        panels.loot_window(&ctx, room, &name);
        panels.plunder_window(&ctx, room, &name);
        panels.hire_window(&ctx, room, &name);
        panels.inventory_window(&ctx, room, &name);
        panels.cell_menu(&ctx, room, &name);
        panels.end_frame(room);
    }
    // What the grids asked for: gear moves through the seam like every
    // other change to the hold.
    for order in panels.orders.drain(..) {
        orders.push(Order::Gear(order));
    }
    for order in panels.crew_orders.drain(..) {
        orders.push(Order::Crew(order));
    }
    for order in panels.later_orders.drain(..) {
        orders.push(Order::CrewLater(order));
    }
    for order in orders.drain(..) {
        screen.net.order(session, order);
    }
    let asked = settings_sheet(
        &ctx,
        &mut screen.sheet,
        &mut sounds.mix,
        &mut bindings,
        &mut screen.saves,
        Allowed::of(session.playing(), online.is_guest()),
    );
    match asked {
        Some(Request::Save(name)) => match session.save() {
            Some(text) => match crate::save::write(&name, &text) {
                Ok(_) => screen.saves.saved(&name),
                Err(why) => screen.saves.failed(why),
            },
            None => screen.saves.failed("Nothing to save.".to_string()),
        },
        // A loaded game is a new session and a new screen round it —
        // the panels, the log, the aim and the sheet all start over, and
        // the first frame fits the canvas to the world as an open does.
        // Both land after this frame, which has already drawn the old
        // one; the world under the pointer next frame is the loaded one.
        // With company — the host's, since a guest's Load is greyed —
        // the file has to fit the room, and the loaded world goes to
        // everybody (`Packet::World`, feature 67): each guest replaces
        // its own with it the way this end does here. The wire is kept
        // across the new screen for that.
        // A restart is a load of the world kept at the open (feature 79):
        // the same text, the same `Session::restore`, the same new screen
        // round it — and with company the same world sent on, so the
        // whole room goes back to the beginning together rather than one
        // end alone. What it reads is the only difference.
        Some(request @ (Request::Load(_) | Request::Restart)) => {
            let text = match &request {
                Request::Load(path) => crate::save::read(path),
                _ => match &beginning {
                    Some(beginning) => Ok(beginning.0.clone()),
                    None => Err(RESTART_NONE.to_string()),
                },
            };
            let read = text.and_then(|text| {
                if let Some(here) = online.room_size()
                    && ship::save::players_of(&text) != Some(here)
                {
                    let saved = ship::save::players_of(&text).unwrap_or(0);
                    return Err(load_players(saved, here));
                }
                Session::restore(&text, screen.size.x, screen.size.y)
                    .map(|loaded| (loaded, text))
                    .map_err(crate::save::load_error)
            });
            match read {
                Ok((loaded, text)) => {
                    let (slot, players) = (loaded.editor.local, loaded.editor.players);
                    if let Some(wire) = &screen.net.wire
                        && wire.host
                    {
                        let at = loaded.game.as_ref().map_or(0, |g| g.world.steps);
                        wire.send(To::All, &Packet::World { save: text, at });
                        if crate::dev::auto().is_some() {
                            let now = session.game.as_ref().map_or(0, |g| g.world.steps);
                            println!("world sent: {at} loaded at {now}");
                        }
                    }
                    crate::names::set_crew_names(&loaded.crew_names);
                    let loaded_steps = loaded.game.as_ref().map_or(0, |g| g.world.steps);
                    commands.insert_resource(ShipSession(loaded));
                    let mut next = screen.again(slot, players);
                    if request == Request::Restart {
                        // A run with nobody at the keyboard says so, the
                        // way a `BIMS_AUTO` one says a world was sent:
                        // a picture cannot tell a restart from a world
                        // that never moved, and `./check` reads this.
                        if crate::dev::smoke_frames().is_some() {
                            let was = session.game.as_ref().map_or(0, |g| g.world.steps);
                            let back = loaded_steps;
                            println!("restart: back at {back} steps, from {was}");
                        }
                        // The log is carried across a world replaced, so
                        // it is where a restart says what it did: what is
                        // above the line happened in the run before it.
                        next.log.push(RESTART_DONE.into());
                    }
                    commands.insert_resource(next);
                }
                Err(why) => screen.saves.failed(why),
            }
        }
        None => {}
    }

    drop(panels_timed);

    // --- painting ------------------------------------------------------------------
    let view = View {
        scale: session.view_scale(),
        offset: {
            let (x, y) = session.view_offset();
            Vec2::new(x, y)
        },
    };
    let painter = canvas_painter(&ctx, canvas);
    // Everything painted over the shapes — names, marks, the numbers —
    // is timed as one (feature 96); it starts once the buffer is down.
    let mut overlay_timed = None;
    if galaxy_up && let Some(chart) = &screen.galaxy {
        // The chart in place of the map: the lobby's picture, in pixels,
        // and the names of the star the ship is at and the one picked over
        // them, since the buffer holds no words.
        {
            let _timed = crate::perf::scope(crate::perf::Phase::Render);
            chart.paint(&mut screen.galaxy_list);
        }
        world_canvas.shapes(&ctx, canvas, View::PIXELS, screen.galaxy_list.shapes());
        for (star, color, tag) in [
            (chart.here, theme::YOURS, "here"),
            (screen.picked_star, theme::HYPER, "picked"),
        ] {
            if let Some(s) = star.and_then(|id| chart.galaxy.star(id)) {
                let (x, y) = chart.preview.to_screen(s.position.x, s.position.y);
                let at = egui::pos2(canvas.min.x + x, canvas.min.y + y - 16.0);
                theme::name_over(
                    &painter,
                    at,
                    &format!("{} · {tag}", star_name(s.name)),
                    color,
                );
            }
        }
    } else {
        {
            let _timed = crate::perf::scope(crate::perf::Phase::Render);
            session.render();
        }
        // The world under the fog now; what goes over the fog — the
        // shots, the rings — once the fog is down (feature 97).
        world_canvas.shapes(&ctx, canvas, view, session.fog_split().0);
        overlay_timed = Some(crate::perf::scope(crate::perf::Phase::Overlay));
        // Where you are, in words, over the reticle the map draws round the
        // ship — `You`, and the berth or the place — in the colour the
        // player's own things are, the way the chart tags the star the ship
        // is at. The ship is the map's origin, wherever it has been panned
        // to; off the canvas the words go with it and the strip still says.
        if map_up && session.game.is_some() {
            // Every planet the ship can land on, named and tagged over
            // its icon — `Rocky planet 1 · land` — in the side's colour,
            // so that it can be landed on is said in words as well as by
            // the pad the map draws at its shoulder. Under the icon, where
            // the ship's own words — over it — cannot land on them.
            for site in session.landing_sites() {
                let (x, y) = site.at;
                let at = view.to_canvas(Vec2::new(x, y)) + canvas.min;
                let at = egui::pos2(at.x, at.y + LAND_DROP);
                // A town the machines are one hop from is said in the
                // enemy's red with a word of its own (feature 94), since
                // "land" is not what the player wants to read about it;
                // one the crew held says so instead.
                let (colour, tag) = if site.hostile {
                    (theme::BAD, LAND_TAG)
                } else if site.threatened {
                    (theme::BAD, TOWN_THREATENED_TAG)
                } else if site.held {
                    (theme::ACCENT, TOWN_HELD_TAG)
                } else {
                    (theme::LAND, LAND_TAG)
                };
                theme::name_over(
                    &painter,
                    at,
                    &format!("{} · {tag}", node_name(session, site.node)),
                    colour,
                );
            }
            let at = view.to_canvas(Vec2::ZERO) + canvas.min;
            let at = egui::pos2(at.x, at.y - HERE_LIFT);
            theme::name_over(
                &painter,
                at,
                &format!("{HERE_TAG} · {}", whereabouts(session)),
                theme::YOURS,
            );
        }
        // The smooth fog over the deck — what the crew do not see, and
        // the dark where no light reaches — as the room's light map,
        // through the ship's camera and heading like the crew's names.
        if !map_up {
            // The plain's fog first, a chunk a texture: the pieces of
            // each leave the room's box to the light map, so the two
            // never lie over one another. A chunk the room let go of
            // takes its texture with it.
            let fogs = session.plain_fog();
            screen
                .plain_fog
                .retain(|key, _| fogs.iter().any(|(k, _, _)| k == key));
            for (key, map, pieces) in fogs {
                let pieces: Vec<crate::fogmap::Piece> = pieces
                    .iter()
                    .map(|p| crate::fogmap::Piece {
                        uv0: p.uv0,
                        uv1: p.uv1,
                        corners: p.corners.map(|(x, y)| {
                            let at = view.to_canvas(Vec2::new(x, y)) + canvas.min;
                            egui::pos2(at.x, at.y)
                        }),
                    })
                    .collect();
                screen.plain_fog.entry(key).or_default().paint_pieces(
                    &mut world_canvas,
                    &ctx,
                    canvas,
                    map,
                    &pieces,
                );
            }
            if let Some((map, corners)) = session.light_map() {
                let corners = corners.map(|(x, y)| {
                    let at = view.to_canvas(Vec2::new(x, y)) + canvas.min;
                    egui::pos2(at.x, at.y)
                });
                screen
                    .fog
                    .paint(&mut world_canvas, &ctx, canvas, map, corners);
            }
        }
        // And over the fog: a bolt is always seen, whatever it flies
        // through.
        world_canvas.shapes(&ctx, canvas, view, session.fog_split().1);
    }

    // And the red crosshair while the attack key has the pointer armed
    // (feature 84), in the same place for the same reason.
    if screen.aiming_attack
        && let Some(p) = on_canvas
    {
        attack_cursor(&painter, egui::pos2(p.x + canvas.min.x, p.y + canvas.min.y));
    }
    // The others' pointers over the deck, each in its player's colour
    // with their Bim's name, through the ship's camera and heading like
    // the names: over the tile they are over, whichever way each has
    // turned the ship.
    if !map_up {
        for (slot, (x, y)) in online.others_pointing() {
            let (x, y) = session.design_point_on_screen(x, y);
            let at = view.to_canvas(Vec2::new(x, y)) + canvas.min;
            theme::ghost_pointer(
                &painter,
                egui::pos2(at.x, at.y),
                theme::ship_color32(ship::world_paint::player_color(slot)),
                &crew_name(slot),
            );
        }
    }

    // What every beam put back since last frame (feature 91), gathered
    // whether or not the deck is being looked at: with the map up the
    // numbers are simply not drawn, rather than saved up for the frame
    // it closes.
    if let Some(game) = &session.game {
        note_heals(&mut screen.heals, game, now);
    }

    // The red cross over every crew member in a dying state: a part of
    // it at nothing with the trauma untreated, which is the one thing on
    // a body only a crewmate with a medkit ends. The living only — a
    // trauma stays on a corpse, and a cross over one would be asking for
    // a medkit nothing can be done with. Drawn first, under the beams
    // and the banners, so a medic already working on one shows through.
    if !map_up && let Some(game) = &session.game {
        for who in 0..game.world.aboard.crew_count() {
            let room = &game.world.aboard.room;
            if !room.is_alive(who as usize) || !room.is_dying(who as usize) {
                continue;
            }
            let Some((x, y)) = session.crew_on_screen(who) else {
                continue;
            };
            let at = view.to_canvas(Vec2::new(x, y)) + canvas.min;
            theme::dying_cross(&painter, egui::pos2(at.x, at.y), view.scale);
        }
    }

    // Every heal beam on the deck (feature 76): a line from the medic to
    // each crew member it holds, and a mark on a body a surge is running
    // on, in the beam's own green.
    if !map_up && let Some(game) = &session.game {
        let crew = game.world.aboard.crew_count();
        for medic in 0..crew {
            let Some(from) = session.crew_on_screen(medic) else {
                continue;
            };
            for patient in game.world.patients_of(medic) {
                let Some(to) = session.crew_on_screen(patient) else {
                    continue;
                };
                let a = view.to_canvas(Vec2::new(from.0, from.1)) + canvas.min;
                let b = view.to_canvas(Vec2::new(to.0, to.1)) + canvas.min;
                theme::heal_beam(
                    &painter,
                    egui::pos2(a.x, a.y),
                    egui::pos2(b.x, b.y),
                    view.scale,
                );
            }
        }
        for who in 0..crew {
            if !game.world.is_surging(who) {
                continue;
            }
            let Some((x, y)) = session.crew_on_screen(who) else {
                continue;
            };
            let at = view.to_canvas(Vec2::new(x, y)) + canvas.min;
            theme::surge_mark(&painter, egui::pos2(at.x, at.y), view.scale);
        }
        // And what each beam is putting back, in green over the patient
        // (feature 91): the line says a medic is working and the numbers
        // say how well it is going, which is the half another player
        // could not see before.
        for floater in &screen.heals.floating {
            let Some((x, y)) = session.crew_on_screen(floater.who) else {
                continue;
            };
            let at = view.to_canvas(Vec2::new(x, y)) + canvas.min;
            let rise = ((now - floater.born) / HEAL_FLOAT_SECONDS) as f32;
            theme::heal_number(
                &painter,
                egui::pos2(at.x, at.y),
                view.scale,
                &crate::names::heal_gain(floater.points),
                rise,
            );
        }
        // The charge bar over every tile being worked (feature 91): an
        // engineer laying a kit, or anybody putting a site together. It
        // stands on the tile rather than over the builder, because what
        // the player wants to see is how far *that* tile has got —
        // which is also why the walk to it counts towards the bar.
        {
            let (ox, oy) = (
                game.world.aboard.offset.x as f32,
                game.world.aboard.offset.y as f32,
            );
            let room = &game.world.aboard.room;
            for who in 0..crew {
                let Some((at, progress)) = room.working_at(who as usize) else {
                    continue;
                };
                let (x, y) = session.design_point_on_screen(at.x - ox, at.y - oy);
                let p = view.to_canvas(Vec2::new(x, y)) + canvas.min;
                theme::work_bar(&painter, egui::pos2(p.x, p.y), view.scale, progress);
            }
        }
        // And the tanks (feature 77): a wall up is a ring of shield at
        // the bulwark's reach, and a taunt the radius it is drawing fire
        // from, dashed. Each also wears its own mark on the **body**
        // (feature 91) — a shield over the head, and rings thrown off
        // him — since a ring drawn at a radius says how far the ability
        // reaches and not which of two tanks standing together is
        // holding it.
        let t = shipdesign::TILE as f32;
        for who in 0..crew {
            let wall = game.world.is_bulwark(who);
            let taunting = game.world.is_taunting(who);
            if !wall && !taunting {
                continue;
            }
            let Some((x, y)) = session.crew_on_screen(who) else {
                continue;
            };
            let at = view.to_canvas(Vec2::new(x, y)) + canvas.min;
            let at = egui::pos2(at.x, at.y);
            if wall {
                let radius = game.world.bulwark_reach(who) * t * view.scale;
                theme::wall_mark(&painter, at, radius, view.scale);
                theme::bulwark_shield(&painter, at, view.scale);
            }
            if taunting {
                theme::taunt_ring(&painter, at, game.world.taunt_radius(who) * t * view.scale);
                theme::taunt_shout(&painter, at, view.scale, now as f32);
            }
        }
        // And the commander (feature 78): the aura's radius round him,
        // a ring under every Bim it lifts — a player's own included —
        // and a bracket over every squad member under his order, with a
        // thread to the enemy it was sent at or the tile it was called
        // back to.
        let on_screen = |who: u32| {
            session.crew_on_screen(who).map(|(x, y)| {
                let p = view.to_canvas(Vec2::new(x, y)) + canvas.min;
                egui::pos2(p.x, p.y)
            })
        };
        for who in 0..crew {
            if game.world.class_of(who) != world::Class::Commander {
                continue;
            }
            let Some(at) = on_screen(who) else {
                continue;
            };
            theme::aura_ring(&painter, at, game.world.aura_radius(who) * t * view.scale);
        }
        // A Bim the aura lifts, and — while a rally runs over it
        // (feature 86) — the rally's own mark in its place, so a rally
        // called is told from the aura standing there all along.
        let rallying = (0..crew).any(|who| {
            game.world.class_of(who) == world::Class::Commander && game.world.rally_left(who) > 0.0
        });
        for who in 0..crew {
            if game.world.aura_reaching(who).is_none() {
                continue;
            }
            if let Some(at) = on_screen(who) {
                if rallying {
                    theme::rallied_mark(&painter, at, view.scale);
                } else {
                    theme::lifted_mark(&painter, at, view.scale);
                }
            }
        }
        // And the caller of it (feature 91). `aura_reaching` answers
        // `None` for a commander asked about his own aura — a commander
        // is not in his own — so without this the one Bim on the deck
        // that is certainly rallying was the one with nothing on it to
        // say so. Two chevrons over the head rather than the rallied
        // ring: his own rig and his selection ring are in the way of
        // anything small drawn at the body.
        for who in 0..crew {
            if game.world.class_of(who) != world::Class::Commander
                || game.world.rally_left(who) <= 0.0
            {
                continue;
            }
            if let Some(at) = on_screen(who) {
                theme::rally_call(&painter, at, view.scale);
            }
        }
        // And whom the ability box under the pointer would reach: the
        // ring is the answer to "who does this cast take in".
        for &who in &cast_reaches {
            if let Some(at) = on_screen(who) {
                theme::affected_ring(&painter, at, view.scale);
            }
        }
        if let Some(order) = &game.world.squad {
            for &who in &order.members {
                let Some(at) = on_screen(who) else {
                    continue;
                };
                let to = match &order.kind {
                    world::SquadKind::Attack { .. } => order
                        .mark_for(who)
                        .and_then(|e| session.resident_on_screen(e))
                        .map(|(x, y)| {
                            let p = view.to_canvas(Vec2::new(x, y)) + canvas.min;
                            egui::pos2(p.x, p.y)
                        }),
                    world::SquadKind::FallBack { tile } => {
                        let (ox, oy) = (
                            game.world.aboard.offset.x as f32,
                            game.world.aboard.offset.y as f32,
                        );
                        let (x, y) = session.design_point_on_screen(
                            (tile.0 as f32 + 0.5) * t - ox,
                            (tile.1 as f32 + 0.5) * t - oy,
                        );
                        let p = view.to_canvas(Vec2::new(x, y)) + canvas.min;
                        Some(egui::pos2(p.x, p.y))
                    }
                    world::SquadKind::StandGround => None,
                };
                theme::squad_mark(&painter, at, to, view.scale);
            }
        }
        // And every player's attack banner (feature 84), on the tile
        // they put it down on — everybody's, since a banner is what the
        // crew round that player are walking into, and a second player
        // ought to see where the first has sent them.
        let (ox, oy) = (
            game.world.aboard.offset.x as f32,
            game.world.aboard.offset.y as f32,
        );
        for slot in 0..game.world.players() {
            let world::Standing::Attack { tile } = game.world.standing_of(slot) else {
                continue;
            };
            let (x, y) = session.design_point_on_screen(
                (tile.0 as f32 + 0.5) * t - ox,
                (tile.1 as f32 + 0.5) * t - oy,
            );
            let p = view.to_canvas(Vec2::new(x, y)) + canvas.min;
            theme::attack_banner(&painter, egui::pos2(p.x, p.y), view.scale);
        }
        // And the **defend sign** over the ship while anybody has called
        // a retreat (feature 84): one sign, on the spot the fall back
        // gathers on, since every player's crew fall back to the one
        // ship. It is what says the crew are coming home, and where to
        // — a retreat has no banner to put down, so without it the only
        // word for it was the line in the log.
        if (0..game.world.players())
            .any(|slot| game.world.standing_of(slot) == world::Standing::Retreat)
        {
            let (rx, ry) = game.world.fall_back_point();
            let (x, y) = session.design_point_on_screen(rx - ox, ry - oy);
            let p = view.to_canvas(Vec2::new(x, y)) + canvas.min;
            theme::defend_banner(&painter, egui::pos2(p.x, p.y), view.scale);
        }
    }

    // A grenade being aimed (feature 75): the burst's radius round the
    // tile under the pointer while Q is held, in the throw's colour when
    // the world would take it and the refusal's when not.
    if !map_up
        && let Some(tile) = screen.throw_aim
        && let Some(game) = &session.game
    {
        let t = shipdesign::TILE as f32;
        let slot = screen.net.slot;
        let (ox, oy) = (
            game.world.aboard.offset.x as f32,
            game.world.aboard.offset.y as f32,
        );
        let (x, y) = session.design_point_on_screen(
            (tile.0 as f32 + 0.5) * t - ox,
            (tile.1 as f32 + 0.5) * t - oy,
        );
        let at = view.to_canvas(Vec2::new(x, y)) + canvas.min;
        let radius = game.world.grenade_radius(slot) * t * view.scale;
        let ok = game.world.can_throw(slot, tile).is_ok();
        theme::burst_ring(&painter, egui::pos2(at.x, at.y), radius, ok);
    }

    // The bunks' tags, across the middle of each: whose it is, or that it
    // is nobody's (feature 61). Under the names, so a sleeper's own stays
    // on top; the station's bunks on a joined deck wear none.
    if !map_up {
        for label in session.bunk_labels() {
            let at = view.to_canvas(Vec2::new(label.x, label.y)) + canvas.min;
            let (words, color) = match label.owner {
                Some(o) if o == local => (crew_name(o), theme::YOURS),
                Some(o) => (crew_name(o), theme::THEIRS),
                None => (BED_TAG_UNASSIGNED.to_string(), theme::MUTED),
            };
            theme::bunk_tag(&painter, egui::pos2(at.x, at.y), &words, color);
        }
    }

    // The crew's names, over their heads, where the ship says each Bim
    // landed — the same camera the shapes went through, so a name stays over
    // its head as the ship turns.
    if !map_up {
        for who in 0..crew_count {
            if let Some((x, y)) = session.crew_on_screen(who) {
                let at = view.to_canvas(Vec2::new(x, y)) + canvas.min;
                let at = egui::pos2(at.x, at.y - theme::NAME_LIFT * view.scale);
                let color = if who == local {
                    theme::YOURS
                } else {
                    theme::THEIRS
                };
                theme::name_over(&painter, at, &crew_name(who), color);
                // Locked in a melee, it says so over its head, where the
                // eye is during a fight: the deck shows the brawl and not
                // why the shooting stopped.
                if session.room_ref().is_some_and(|r| {
                    r.is_alive(who as usize) && r.is_locked(who as usize).is_some()
                }) {
                    let above = egui::pos2(at.x, at.y - theme::NAME_SIZE - 2.0);
                    theme::name_over(&painter, above, LOCKED_STATUS, theme::CAUTION);
                }
            }
        }
        let station = session.resident_station().unwrap_or(0);
        for who in 0..session.resident_count() {
            if let Some((x, y)) = session.resident_on_screen(who) {
                let at = view.to_canvas(Vec2::new(x, y)) + canvas.min;
                let at = egui::pos2(at.x, at.y - theme::NAME_LIFT * view.scale);
                // A machine wears its kind, not a name: it is not
                // somebody (feature 83).
                let label = match session.resident_droid(who) {
                    Some(kind) => crate::names::droid_name(kind).to_string(),
                    None => resident_name(station, who),
                };
                theme::name_over(&painter, at, &label, theme::THEIRS);
                // A mercenary for hire wears a `?` over its name: somebody
                // to right-click and talk to, which nobody else ashore is.
                if session.mercenary_fee(who).is_some() {
                    let above = egui::pos2(at.x, at.y - theme::NAME_SIZE - 4.0);
                    theme::badge_over(&painter, above, MERCENARY_MARK, theme::ACCENT);
                }
            }
        }
    }
    // The electricity view's numbers: what every drainer draws, in yellow
    // over the top of it — the engines with what they draw at this moment,
    // against what they would flat out, so a burn can be read at the deck
    // as well as at the reactor. A dark consumer is written muted, with the
    // draw it would have if it were wired.
    if !map_up
        && session
            .game
            .as_ref()
            .is_some_and(|g| g.overlay == ship::game::Overlay::Electricity)
    {
        for label in session.power_labels() {
            let at = view.to_canvas(Vec2::new(label.x, label.y)) + canvas.min;
            let at = egui::pos2(at.x, at.y - 3.0);
            let words = if label.now == label.full {
                format!("{}", label.now.round())
            } else {
                format!("{} / {}", label.now.round(), label.full.round())
            };
            let color = if label.live {
                theme::DRAW
            } else {
                theme::MUTED
            };
            theme::name_over(&painter, at, &words, color);
        }
    }
    // The black after a landing, over everything on the canvas: held
    // while the ground is laid out, then lifted. Real seconds, not the
    // world's: a pause is not a longer night.
    if screen.blackout > 0.0 {
        screen.blackout -= dt as f32;
        let alpha = (screen.blackout / BLACKOUT_FADE).clamp(0.0, 1.0);
        painter.rect_filled(
            crate::canvas::egui_rect(canvas),
            0.0,
            egui::Color32::from_black_alpha((alpha * 255.0) as u8),
        );
    }
    drop(overlay_timed);
    let _ = now;
    Ok(())
}

/// The pointer while the attack key has it armed (feature 84): a red
/// crosshair where the system's cursor was, each stroke over a dark one
/// so it reads on the deck and on the void alike. Red because what the
/// next click does is send people into a fight.
fn attack_cursor(painter: &egui::Painter, at: egui::Pos2) {
    let arms = [
        [at + egui::vec2(-13.0, 0.0), at + egui::vec2(-4.0, 0.0)],
        [at + egui::vec2(4.0, 0.0), at + egui::vec2(13.0, 0.0)],
        [at + egui::vec2(0.0, -13.0), at + egui::vec2(0.0, -4.0)],
        [at + egui::vec2(0.0, 4.0), at + egui::vec2(0.0, 13.0)],
    ];
    for (width, color) in [
        (4.5, egui::Color32::from_black_alpha(200)),
        (2.0, theme::ATTACK),
    ] {
        for arm in arms {
            painter.line_segment(arm, egui::Stroke::new(width, color));
        }
        painter.circle_stroke(at, 7.0, egui::Stroke::new(width * 0.6, color));
    }
    painter.circle_filled(at, 1.5, theme::ATTACK);
}

/// What the ship is doing, in a word: `STATE_NAMES` by the state's code —
/// except at a planet, where a docking is a landing, a push-off a
/// lift-off and a berth the ground, since the picture says so too.
fn state_name(game: &ship::game::Game) -> String {
    let state = game.world.ship.state.code();
    let on_a_planet = game
        .world
        .ship
        .state
        .station()
        .is_some_and(|id| world::surface_body(id).is_some());
    let word = match (state, on_a_planet) {
        (0, true) => "Landed",
        (4, true) => "Lifting off",
        (5, true) => "Landing",
        _ => STATE_NAMES
            .get(state as usize)
            .copied()
            .unwrap_or("Holding"),
    };
    word.to_string()
}

/// Where the ship is, in words: the berth it is tied up at, the place it is
/// alongside, or open space. The trip strip's first words, and what the
/// map writes over the ship.
fn whereabouts(session: &Session) -> String {
    match session.docked_at() {
        Some(station) if world::surface_body(station).is_some() => {
            format!("Landed · {}", node_name(session, Node::Station(station)))
        }
        Some(station) => format!("Docked · {}", node_name(session, Node::Station(station))),
        None => match session.game.as_ref().unwrap().world.ship.frame.node() {
            Some(node) => format!("Alongside {}", node_name(session, node)),
            None => "Open space".into(),
        },
    }
}

/// The strip across the top of the canvas: where the ship is, and where it
/// is going. With the map up it is the helm — what is aimed at, the quote
/// for it, and Confirm; with the ship view up it is the trip under way, as
/// a bar and the time left, or the berth. Brake and Abort sit on it either
/// way. The preview is worked out here and is **never** a command: every
/// order here wants the helm, so a press walks the crew member there and
/// the frame reads it through the seam when they arrive.
#[allow(clippy::too_many_arguments)]
fn trip_panel(
    ui: &mut egui::Ui,
    session: &mut Session,
    local: u32,
    map_up: bool,
    aimed: &mut Option<Aim>,
    pending: &mut Option<HelmOrder>,
    relieve: &mut bool,
    log: &mut Vec<String>,
    chart: &mut Chart,
    orders: &mut Vec<Order>,
) {
    // Re-quoted every frame while the player is aiming at something. A
    // quote goes stale the moment the ship moves.
    match *aimed {
        None => session.clear_preview(),
        Some(aim) => session.preview(aim.target()),
    }
    let whereabouts = whereabouts(session);
    let game = session.game.as_ref().unwrap();
    let state = game.world.ship.state.code();
    let aborting = game.world.plan().is_some_and(|p| p.aborting);
    let walking = pending.is_some();
    let mut press: Option<HelmOrder> = None;
    let mut cancel = false;

    // The chart's toggle, on the map: the same button either way round.
    // The chart is made the first time it is asked for — every system of
    // the galaxy, once — and the star the ship is at is the one open in it
    // until another is picked.
    let charging = game.world.jump_charge();
    if map_up {
        ui.horizontal(|ui| {
            let label = if *chart.up {
                "System view"
            } else {
                "Galaxy view"
            };
            if ui.button(label).clicked() {
                *chart.up = !*chart.up;
                if *chart.up {
                    if chart.lobby.is_none() {
                        *chart.lobby = Some(lobby::Lobby::new(
                            game.world.galaxy_seed,
                            game.world.galaxy_type,
                            800.0,
                            600.0,
                        ));
                    }
                    if let Some(lobby) = chart.lobby.as_mut() {
                        lobby.inspect(chart.picked.unwrap_or(game.world.star_id));
                    }
                }
            }
            if let Some((star, done)) = charging {
                ui.label(
                    egui::RichText::new(format!("Charging for star {star}")).color(theme::HYPER),
                );
                theme::bar(ui, 120.0, done as f32, theme::HYPER);
            }
        });
    }
    if map_up && *chart.up {
        if let Some(order) = chart_panel(ui, session, chart, walking) {
            press = Some(order);
        }
        ui.horizontal(|ui| {
            brake_buttons(ui, state, aborting, walking, &mut press);
        });
    } else if map_up {
        let quoted = match game.preview.as_ref() {
            Some(Err(why)) => {
                ui.label(egui::RichText::new(plan_error(why.code())).color(theme::WARN));
                false
            }
            Some(Ok(p)) => {
                let mut rows = vec![
                    ("Going to", describe_aim(session, *aimed)),
                    ("Arrives in", spell(p.minutes)),
                ];
                if p.stopping > 0.0 {
                    rows.push(("Stopping first", spell(p.stopping)));
                }
                // What the burn costs the ship: the engines' draw off the
                // reactor, and how much of their push that buys.
                rows.push((
                    "Engines",
                    if p.throttle >= 1.0 {
                        format!("{} a minute, flat out", p.power.round())
                    } else {
                        format!(
                            "{} a minute, throttled to {}%",
                            p.power.round(),
                            (p.throttle * 100.0).round()
                        )
                    },
                ));
                rows.push(("Ends", if p.docks { "Docked" } else { "Holding" }.into()));
                egui::Grid::new("quote")
                    .num_columns(2)
                    .spacing([10.0, 1.0])
                    .show(ui, |ui| {
                        for (label, value) in rows {
                            ui.label(egui::RichText::new(label).small().color(theme::MUTED));
                            ui.label(value);
                            ui.end_row();
                        }
                    });
                true
            }
            None => {
                ui.label(
                    egui::RichText::new(if state == 2 {
                        "Under way. Click somewhere on the map to plot a new trip."
                    } else {
                        "Click somewhere on the map to plot a trip."
                    })
                    .color(theme::MUTED),
                );
                false
            }
        };
        ui.horizontal(|ui| {
            if ui
                .add_enabled(quoted && !walking, egui::Button::new("Confirm"))
                .clicked()
                && let Some(aim) = *aimed
            {
                press = Some(HelmOrder::Fly(aim));
            }
            if ui
                .add_enabled(aimed.is_some(), egui::Button::new("Clear"))
                .clicked()
            {
                *aimed = None;
            }
            land_button(ui, game, walking, &mut press);
            brake_buttons(ui, state, aborting, walking, &mut press);
        });
    } else {
        match game.world.trip_progress() {
            Some((done, total)) => {
                let going = match game.world.plan().map(|p| p.target) {
                    Some(Target::Point(_)) | None => "A point in space".to_string(),
                    Some(target) => target
                        .node()
                        .map(|node| node_name(session, node))
                        .unwrap_or_else(|| "A point in space".into()),
                };
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new(if aborting { "Stopping" } else { "To" }).strong(),
                    );
                    ui.label(going);
                });
                ui.horizontal(|ui| {
                    theme::bar(ui, 220.0, (done / total.max(1e-9)) as f32, theme::ACCENT);
                    ui.label(
                        egui::RichText::new(format!("{} left", spell(total - done)))
                            .color(theme::MUTED),
                    );
                });
            }
            None => {
                let phase = if (3..=5).contains(&state) {
                    state_name(game)
                } else {
                    whereabouts.clone()
                };
                ui.label(egui::RichText::new(phase).strong());
            }
        }
        if state >= 2 || over_a_planet(game) {
            ui.horizontal(|ui| {
                land_button(ui, game, walking, &mut press);
                brake_buttons(ui, state, aborting, walking, &mut press);
            });
        }
    }
    if walking {
        ui.horizontal(|ui| {
            ui.label(
                egui::RichText::new(format!("{} is on the way to the helm.", crew_name(local)))
                    .color(theme::ACCENT),
            );
            if ui.button("Cancel").clicked() {
                cancel = true;
            }
        });
    }

    // A press is a walk first. The order waits in `pending` for the crew
    // member to reach the seat; with no helm to walk to it says so and
    // orders nothing. The walk itself is `Order::ToHelm`, through the
    // seam like every order: a way there or not is the world's to say.
    let game = session.game.as_mut().unwrap();
    if let Some(order) = press {
        if game.world.helm_spot().is_some() {
            orders.push(Order::ToHelm);
            *pending = Some(order);
            *relieve = false;
        } else {
            log.push(format!("{} cannot reach the helm.", crew_name(local)));
        }
    }
    if cancel {
        *pending = None;
        orders.push(Order::Crew(CrewOrder::StandDown { who: local }));
    }
}

/// The galaxy chart's state, as the strip sees it: whether it is up, the
/// chart itself, and the star picked on it.
struct Chart<'a> {
    up: &'a mut bool,
    lobby: &'a mut Option<lobby::Lobby>,
    picked: &'a mut Option<u32>,
}

/// What the crisis has to say about a star (feature 92): that the machines
/// hold it and what day it fell, or the day it is due to. One line, under
/// the star's name on the chart's panel, and nothing at all for a star the
/// lanes do not reach — which, the graph being one piece, is no star at all.
fn crisis_line(ui: &mut egui::Ui, world: &world::World, star: u32) {
    let day = world.infested_on(star);
    if day == u32::MAX {
        return;
    }
    // The day the crisis counts by is `World::days_gone` — days the *world*
    // has run — and the strip at the top reads the crew's own calendar,
    // which starts at the waking hour and is a day ahead for part of every
    // day. So the day is said with how far off it is beside it, and the two
    // readings cannot be mistaken for one another.
    let now = world.days_gone();
    let (words, colour) = if world.infested(star) {
        let since = match now - day {
            0 => "today".to_string(),
            1 => "yesterday".to_string(),
            n => format!("{n} days ago"),
        };
        (
            format!("Held by the machines · day {day}, {since}"),
            theme::BAD,
        )
    } else {
        let off = match day - now {
            0 => "today".to_string(),
            1 => "tomorrow".to_string(),
            n => format!("{n} days off"),
        };
        (
            format!("The machines reach it on day {day} · {off}"),
            theme::WARN,
        )
    };
    ui.label(egui::RichText::new(words).small().color(colour));
}

/// Whether the machines' jammer holds this system's lanes shut, and which
/// station it stands on (feature 93). One line, under the crisis's own on
/// the chart's panel and on the helm's strip, and nothing at all in a
/// system the machines have not got.
fn jammer_line(ui: &mut egui::Ui, world: &world::World) {
    let Some(station) = world.jammer_station() else {
        return;
    };
    let name = world
        .system
        .station(station)
        .map(|s| station_name(s.name))
        .unwrap_or_else(|| "a station".to_string());
    let (words, colour) = if world.jammed() {
        (
            format!("Jammed · the lanes inward are shut from {name}"),
            theme::BAD,
        )
    } else {
        (format!("Jammer down · {name} is cleared"), theme::ACCENT)
    };
    ui.label(egui::RichText::new(words).small().color(colour));
}

/// One system's contents, as rows: each body by its numeral and kind, each
/// station by its name, kind and side. What the chart says a star holds —
/// the star the ship is at, or the one picked — off the generator, the way
/// the lobby lists a system before the game opens.
fn system_contents(ui: &mut egui::Ui, base: &str, system: &worldgen::StarSystem) {
    for body in &system.bodies {
        ui.horizontal(|ui| {
            ui.label(format!("{base} {}", roman(body.name.part as u32)));
            ui.label(
                egui::RichText::new(
                    BODY_KIND_NAMES
                        .get(body.kind as usize)
                        .copied()
                        .unwrap_or("Body"),
                )
                .small()
                .color(theme::MUTED),
            );
        });
    }
    for station in &system.stations {
        ui.horizontal(|ui| {
            ui.label(
                egui::RichText::new(station_name(station.name)).color(if station.hostile {
                    theme::BAD
                } else {
                    theme::INK
                }),
            );
            // And which gear trades it has (feature 95), since that is
            // the one thing about a station worth flying to it for.
            let trades = gear_trades(
                station.stock.sells(ResourceId::Handgun),
                station.stock.sells(ResourceId::Helm),
            );
            ui.label(
                egui::RichText::new(format!(
                    "{}{}{}",
                    STATION_KIND_NAMES
                        .get(station.kind as usize)
                        .copied()
                        .unwrap_or("Station"),
                    if station.hostile { " · hostile" } else { "" },
                    trades.map(|t| format!(" · {t}")).unwrap_or_default()
                ))
                .small()
                .color(theme::MUTED),
            );
        });
    }
    if system.bodies.is_empty() && system.stations.is_empty() {
        ui.label(egui::RichText::new("Nothing there.").color(theme::MUTED));
    }
}

/// The chart's half of the strip: where the ship is and what is round it,
/// what the picked star holds, and Jump. `Some(order)` is a press.
fn chart_panel(
    ui: &mut egui::Ui,
    session: &Session,
    chart: &mut Chart,
    walking: bool,
) -> Option<HelmOrder> {
    let game = session.game.as_ref().unwrap();
    let lobby = chart.lobby.as_mut()?;
    let here = game.world.star_id;
    let name_of = |star: u32| {
        lobby
            .galaxy
            .star(star)
            .map(|s| {
                format!(
                    "{} · class {}",
                    star_name(s.name),
                    STAR_CLASS_NAMES
                        .get(s.star_class as usize)
                        .copied()
                        .unwrap_or("?")
                )
            })
            .unwrap_or_default()
    };
    let base_of = |star: u32| {
        lobby
            .galaxy
            .star(star)
            .map(|s| star_name(s.name))
            .unwrap_or_default()
    };

    ui.label(egui::RichText::new("Here").small().color(theme::MUTED));
    ui.label(egui::RichText::new(name_of(here)).strong());
    crisis_line(ui, &game.world, here);
    jammer_line(ui, &game.world);
    system_contents(ui, &base_of(here), &game.world.system);

    ui.add_space(4.0);
    let mut press = None;
    match *chart.picked {
        Some(star) if star != here => {
            ui.label(egui::RichText::new("Picked").small().color(theme::MUTED));
            ui.label(egui::RichText::new(name_of(star)).strong());
            crisis_line(ui, &game.world, star);
            match lobby.inspected.as_ref().filter(|(id, _)| *id == star) {
                Some((_, system)) => system_contents(ui, &base_of(star), system),
                None => {
                    ui.label(egui::RichText::new("Looking…").color(theme::MUTED));
                }
            }
            // The route along the lanes, which is what a jump follows now
            // (feature 93): how many hops, the next star down it, and
            // whether a jammer shuts the first step.
            let route = game.world.route_to(star);
            let next = route.as_ref().and_then(|r| r.get(1).copied());
            ui.add_space(2.0);
            match &route {
                Some(r) if r.len() > 1 => {
                    let hops = r.len() - 1;
                    let word = if hops == 1 { "hop" } else { "hops" };
                    ui.label(
                        egui::RichText::new(format!("{hops} {word} along the lanes"))
                            .small()
                            .color(theme::MUTED),
                    );
                    if let Some(next) = next {
                        ui.label(
                            egui::RichText::new(format!("Next: {}", base_of(next)))
                                .small()
                                .color(theme::HYPER),
                        );
                    }
                    let shut = r
                        .windows(2)
                        .filter(|pair| game.world.jammed_step(pair[0], pair[1]))
                        .count();
                    if shut > 0 {
                        ui.label(
                            egui::RichText::new(format!(
                                "{shut} of the route's steps {} shut by a jammer",
                                if shut == 1 { "is" } else { "are" }
                            ))
                            .small()
                            .color(theme::BAD),
                        );
                    }
                }
                _ => {
                    ui.label(
                        egui::RichText::new("No lane route to that star.")
                            .small()
                            .color(theme::BAD),
                    );
                }
            }
            let state = game.world.ship.state.code();
            let ready = game.world.hyperdrive_ready();
            let jammed = next.is_some_and(|next| game.world.jammed_step(here, next));
            let why = if !ready {
                Some("No working hyperdrive: one bolted to an engine, on a live cable.")
            } else if state != 1 {
                Some("A jump wants the ship holding on its own, away from any berth.")
            } else if next.is_none() {
                Some("The lanes do not reach that star.")
            } else if jammed {
                Some("The machines' jammer holds this system's lanes inward shut.")
            } else {
                None
            };
            if let Some(why) = why {
                ui.label(egui::RichText::new(why).small().color(theme::WARN));
            }
            ui.horizontal(|ui| {
                // The charge is for the route's **first** star, not the
                // one picked: a jump is one hop.
                let button = ui.add_enabled(
                    why.is_none() && !walking,
                    egui::Button::new(match next {
                        Some(next) if next != star => format!("Jump to {}", base_of(next)),
                        _ => "Jump".to_string(),
                    }),
                );
                if button.clicked()
                    && let Some(next) = next
                {
                    press = Some(HelmOrder::Jump(next));
                }
                if ui.button("Clear").clicked() {
                    *chart.picked = None;
                }
            });
        }
        Some(_) => {
            ui.label(
                egui::RichText::new(
                    "That is the star the ship is at. Click another to see what it holds.",
                )
                .small()
                .color(theme::MUTED),
            );
        }
        None => {
            ui.label(
                egui::RichText::new(
                    "Click a star to see what it holds. The hyperdrive jumps there.",
                )
                .small()
                .color(theme::MUTED),
            );
        }
    }
    press
}

/// Whether the ship is in the frame of a planet it could come down onto —
/// one with a settlement (`World::surface`) — holding or not.
fn over_a_planet(game: &ship::game::Game) -> bool {
    match game.world.ship.frame {
        world::Frame::Local(Node::Body(body)) => game.world.surface(body).is_some(),
        _ => false,
    }
}

/// Land, shown while the ship is in a landable planet's frame and pressed
/// from a hold: the descent onto the settlement's pad. Greyed with why
/// when the ship is docked, under way or built on (`World::can_land`).
fn land_button(
    ui: &mut egui::Ui,
    game: &ship::game::Game,
    walking: bool,
    press: &mut Option<HelmOrder>,
) {
    if !over_a_planet(game) {
        return;
    }
    let why = game.world.can_land().err();
    let button = ui.add_enabled(why.is_none() && !walking, egui::Button::new("Land"));
    let button = match why {
        Some(why) => button.on_disabled_hover_text(format!("Not now: {}.", refusal(why))),
        None => button.on_hover_text("Come down onto the planet: the settlement's landing pad."),
    };
    if button.clicked() {
        *press = Some(HelmOrder::Land);
    }
}

/// Brake stops a ship under way — once. Abort calls off a departure while
/// the ship is still casting off or pushing off the berth, or a hyperdrive
/// charging. The two are the
/// same command at the seam and never both live.
fn brake_buttons(
    ui: &mut egui::Ui,
    state: u32,
    aborting: bool,
    walking: bool,
    press: &mut Option<HelmOrder>,
) {
    if ui
        .add_enabled(
            state == 2 && !aborting && !walking,
            egui::Button::new("Brake"),
        )
        .clicked()
    {
        *press = Some(HelmOrder::Stop);
    }
    if ui
        .add_enabled(
            (state == 3 || state == 4 || state == 6) && !walking,
            egui::Button::new("Abort"),
        )
        .clicked()
    {
        *press = Some(HelmOrder::Stop);
    }
}

/// What the helm is pointed at, and how far off it is. The distance comes
/// from the map rather than from the plan: a plan is a route, and "how far
/// away is it" is a question about the thing.
fn describe_aim(session: &Session, aimed: Option<Aim>) -> String {
    let Some(aim) = aimed else {
        return "Nowhere".into();
    };
    let here = session.game.as_ref().unwrap().world.ship.position();
    match aim {
        Aim::Node(node) => {
            let at = session
                .map_index_of(node)
                .and_then(|i| session.map_position(i))
                .unwrap_or(here);
            let away = ((at.x - here.x).powi(2) + (at.y - here.y).powi(2)).sqrt();
            format!(
                "{} · {} units",
                node_name(session, node),
                grouped(away.round() as u64)
            )
        }
        Aim::Point(x, y) => {
            let away = ((x - here.x).powi(2) + (y - here.y).powi(2)).sqrt();
            format!("A point in space · {} units", grouped(away.round() as u64))
        }
    }
}

/// The red warning along the top while the station alongside is held by
/// the machines (feature 83), in the raid's own frame since it is the
/// same kind of thing: which wave is on the deck and how many of it are
/// standing, and — the moment the last of them is down — **the
/// countdown to the next one landing**, which is the one number the
/// player has no other way of knowing (a wave is never reinforced
/// mid-fight, so until then there is nothing to count). Nothing at a
/// station nobody holds, and nothing once the last wave is spent bar the
/// one line saying so.
fn droid_warning(ui: &mut egui::Ui, world: &world::World) {
    // A **town under attack** (feature 94) says the same three things in
    // the same frame: the fight is the same fight, and the player wants
    // the same number out of it. Before the first wave has landed there
    // is a wave on its way and none on the ground, which is the one
    // reading a held station never has.
    if let Some(defending) = world.defense_here() {
        let waves = defending.wave + defending.waves_left;
        let standing = world.droids_standing();
        let words = if standing > 0 {
            droids_standing(defending.wave, waves, standing)
        } else if let Some(due) = defending.next_in {
            droids_next_wave(&crate::format::in_words(due), defending.wave + 1, waves)
        } else if defending.wave > 0 && defending.waves_left == 0 {
            DROIDS_CLEARED.into()
        } else {
            droids_standing(defending.wave.max(1), waves.max(1), 0)
        };
        raid_frame().show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new(words).strong().color(theme::BAD));
                theme::question_mark(ui, DEFENSE_TIP);
            });
        });
        return;
    }
    let Some((wave, left)) = world.droid_wave_standing() else {
        return;
    };
    let waves = wave + left;
    let standing = world.droids_standing();
    let words = if standing > 0 {
        droids_standing(wave, waves, standing)
    } else if let Some(due) = world.droid_wave_due() {
        droids_next_wave(&crate::format::in_words(due), wave + 1, waves)
    } else if left == 0 {
        DROIDS_CLEARED.into()
    } else {
        // The last machine went down this very step: the clock is set
        // at the top of the next one. Say the wave rather than a
        // countdown of nothing.
        droids_standing(wave, waves, 0)
    };
    raid_frame().show(ui, |ui| {
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new(words).strong().color(theme::BAD));
            theme::question_mark(ui, DROIDS_TIP);
        });
    });
}

/// The frame the raid warning sits in: the panel's, filled and edged in
/// red, so it is the one red thing on the screen.
fn raid_frame() -> egui::Frame {
    panel_frame()
        .fill(egui::Color32::from_rgba_unmultiplied(64, 14, 10, 235))
        .stroke(egui::Stroke::new(1.0, theme::WARN))
}

/// The red warning while a raid is on (feature 68): the raider closing,
/// with what is left of its run counted down in words every frame; its
/// boarders at the ship's airlock, locked in their face the step it tied
/// up; the bar as they force it; and the airlock given. Nothing once the
/// boarders are all down — the log said so — or with no raid on.
fn raid_warning(ui: &mut egui::Ui, world: &world::World) {
    let words = match world.raid() {
        world::Raid::Quiet | world::Raid::Docked { repelled: true, .. } => return,
        world::Raid::Closing { boarders, .. } => {
            let left = world.raid_minutes_left().unwrap_or(0.0);
            raid_incoming(&crate::format::in_words(left), *boarders)
        }
        world::Raid::Docked {
            boarders, breached, ..
        } => {
            if *breached {
                raid_aboard(*boarders)
            } else if world.raid_forcing().is_some() {
                raid_forcing(*boarders)
            } else {
                raid_at_the_airlock(*boarders)
            }
        }
    };
    let forcing = world.raid_forcing();
    raid_frame().show(ui, |ui| {
        ui.vertical(|ui| {
            let row = ui
                .horizontal(|ui| {
                    ui.label(egui::RichText::new(words).strong().color(theme::BAD));
                    theme::question_mark(ui, RAID_TIP);
                })
                .response
                .rect;
            // The bar under the words, the width of them: how far the
            // heaving has got, the same as the bar over the door.
            if let Some(progress) = forcing {
                theme::thin_bar(ui, row.width(), progress, theme::WARN);
            }
        });
    });
}

/// The day and the clock, with the speed beside them: what you asked for
/// is the button held down, and what the world is running at is the one
/// coloured — a player held at 1x by somebody else needs to be able to see
/// that is what happened, so with more than one player each one's request
/// is listed under.
fn clock_panel(ui: &mut egui::Ui, session: &Session, net: &Net, orders: &mut Vec<Order>) {
    let game = session.game.as_ref().unwrap();
    let minutes = game.world.minutes_into_day();
    let mine = game.requested(net.slot);
    let effective = game.world.effective_speed();
    ui.horizontal(|ui| {
        ui.label(
            egui::RichText::new(format!(
                "Day {} · {:02}:{:02}",
                game.world.day(),
                (minutes / 60.0).floor() as u32,
                (minutes % 60.0).floor() as u32
            ))
            .strong(),
        );
        ui.add_space(6.0);
        for speed in Speed::ALL {
            let times = speed.multiplier();
            let label = if times == 0 {
                "||".to_string()
            } else {
                format!("{times}×")
            };
            let text = egui::RichText::new(label).color(if speed == effective {
                theme::ACCENT
            } else {
                theme::INK
            });
            if theme::toggle(ui, speed == mine, text).clicked() {
                orders.push(Order::Speed(speed));
            }
        }
    });
    if net.players > 1 {
        ui.horizontal(|ui| {
            for slot in 0..net.players {
                let times = game.requested(slot).multiplier();
                ui.label(
                    egui::RichText::new(format!(
                        "Player {}{} {}",
                        slot + 1,
                        if slot == net.slot { " (you)" } else { "" },
                        if times == 0 {
                            "paused".into()
                        } else {
                            format!("{times}×")
                        }
                    ))
                    .small()
                    .color(theme::MUTED),
                );
            }
        });
    }
}

/// The station's shelf, in a window in the middle of the screen: what it
/// trades, a row a resource with its icon, the ones it does not stock
/// dimmed, and under them the cart — what the lot would cost or fetch, the
/// hold's room after it, and Confirm, which is when anything is bought or
/// sold at all. Only while docked — the button that opens it is
/// only there then — and shut by its own cross, by Escape, or by leaving.
fn trade_window(
    ctx: &egui::Context,
    session: &mut Session,
    local: u32,
    open: &mut bool,
    cart: &mut Cart,
    orders: &mut Vec<Order>,
) {
    let Some(station) = session.docked_at() else {
        *open = false;
        return;
    };
    let title = node_name(session, Node::Station(station));
    // The station is traded with across its desk: the cart is filled from
    // anywhere, but Confirm goes only while the crew member steered stands
    // at it, and the button walks it over. The world refuses a deal from
    // across the room either way.
    let at_desk = session.at_the_desk(local);
    let mut walk = false;
    egui::Window::new(title)
        .id(egui::Id::new("trade"))
        .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
        .collapsible(false)
        .resizable(false)
        .open(open)
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new("Crew's money")
                        .small()
                        .color(theme::MUTED),
                );
                ui.label(egui::RichText::new(euros(session.remaining())).strong());
            });
            if !at_desk {
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new(NOT_AT_DESK)
                            .small()
                            .color(theme::CAUTION),
                    );
                    if ui.small_button(WALK_TO_DESK).clicked() {
                        walk = true;
                    }
                });
            }
            // A desk inside the front charges over the odds for what a
            // fight is fought with (feature 94). Said outright, with the
            // underlined word carrying what it is charged on, since a
            // price that has moved and no word for why is a bug report.
            if let Some(hops) = session.front_premium() {
                theme::asks(ui, &front_premium(hops), FRONT_PREMIUM_TIP);
            }
            // And which gear this desk deals in (feature 95): the two
            // trades are rolled off the place's own seed, so a station
            // that sells no guns at all is the ordinary case rather than
            // a fault, and the line says so before the rows are read.
            let trades = gear_trades(
                session.sold_here(ResourceId::Handgun),
                session.sold_here(ResourceId::Helm),
            );
            theme::asks(
                ui,
                &trades
                    .map(|t| format!("Gear traded here: {t}."))
                    .unwrap_or_else(|| NO_GEAR_TRADE.to_string()),
                GEAR_TRADE_TIP,
            );
            ui.add_space(4.0);
            // What is aboard to sell is what no construction site has
            // claimed, which is the rule `Sell` is judged by.
            let free = |s: &Session, id: ResourceId| {
                s.game
                    .as_ref()
                    .map_or(0, |g| g.world.ship.design.carrying(id))
            };
            for (resource, units, buying, tier) in
                trade_rows(ui, session, cart, true, at_desk, free)
            {
                orders.push(Order::Deal {
                    resource: ResourceId::ALL[resource as usize],
                    units,
                    buying,
                    // The tier the row's chooser named (feature 95): a buy
                    // is at that tier, a sale gives up the lowest first.
                    tier,
                });
            }
        });
    if walk {
        orders.push(Order::ToDesk);
    }
}

/// The Ship tab: the helm, and the ship's facts.
fn ship_panel(ui: &mut egui::Ui, session: &Session, net: &Net, local: u32, walking: bool) {
    let game = session.game.as_ref().unwrap();
    theme::heading(ui, "Helm");
    let manned = game.world.at_the_helm(local);
    ui.label(if walking {
        egui::RichText::new(format!("{} is on the way to the helm.", crew_name(local)))
            .color(theme::ACCENT)
    } else if manned {
        egui::RichText::new(format!("{} is at the helm.", crew_name(local))).color(theme::ACCENT)
    } else {
        egui::RichText::new("Nobody of yours is at the helm.").color(theme::MUTED)
    });
    ui.label(
        egui::RichText::new(
            "Open the map (M), click somewhere, and Confirm at the top: a crew member walks to the helm and sets off.",
        )
        .small()
        .color(theme::MUTED),
    );
    facts_panel(ui, session, net);
}

fn facts_panel(ui: &mut egui::Ui, session: &Session, net: &Net) {
    theme::heading(ui, "Ship");
    let game = session.game.as_ref().unwrap();
    let power = game.world.power();
    let batteries = if power.storage > 0.0 {
        format!(
            ", {} of {} stored",
            power.charge.round(),
            power.storage.round()
        )
    } else {
        String::new()
    };
    let engines = if power.engines > 0.0 {
        format!(" + {} to the engines", power.engines.round())
    } else {
        String::new()
    };
    let power_line = format!(
        "{} drawn{engines} of {} made{batteries}{}",
        power.draw.round(),
        power.supply.round(),
        if power.brownout() {
            " — brownout"
        } else {
            ""
        }
    );
    let dosed: Vec<String> = (0..net.players)
        .filter_map(|who| {
            let dose = game.world.health.get(who as usize)?.dose;
            (dose > 0.5).then(|| format!("{} {}", crew_name(who), dose.round()))
        })
        .collect();
    let by = game.world.ship.destination_set_by;
    let at = game.world.ship.position();
    let rows = [
        ("Power", power_line),
        (
            "Dose",
            if dosed.is_empty() {
                "none".into()
            } else {
                dosed.join(", ")
            },
        ),
        ("Mass", format!("{:.0}", session.mass())),
        (
            "Acceleration",
            format!("{:.4}", session.acceleration(Facing::Forward)),
        ),
        ("Parts", session.design_ref().parts.len().to_string()),
        ("Exposed tiles", session.editor.exposed().len().to_string()),
        (
            "Position",
            format!(
                "{}, {}",
                grouped(at.x.round().abs() as u64),
                grouped(at.y.round().abs() as u64)
            ),
        ),
        (
            "Found",
            format!("{} in this system", game.world.discovered.len()),
        ),
        (
            "Scanner",
            format!(
                "{} units",
                grouped(game.world.detection_range().round() as u64)
            ),
        ),
        (
            "Route set by",
            by.map(|s| format!("Player {}", s + 1))
                .unwrap_or_else(|| "Nobody".into()),
        ),
        ("Crew", net.players.to_string()),
    ];
    egui::Grid::new("facts")
        .num_columns(2)
        .spacing([10.0, 1.0])
        .show(ui, |ui| {
            for (label, value) in rows {
                ui.label(egui::RichText::new(label).small().color(theme::MUTED));
                ui.label(egui::RichText::new(value).small());
                ui.end_row();
            }
        });
}

/// What is aboard, a row a resource, grouped by where it is stowed. The
/// counts are not all the hold's: the cold store aboard is the room's,
/// stocked off the manifest when the world opens and at every dock, and
/// what is eaten and grown in between never goes back on the manifest —
/// so the two food rows read the room, because that is what the crew can
/// eat. A readout and nothing else: the standing orders for what the
/// benches make are set on the tray's Management tab, beside the cold
/// store's.
fn items_panel(ui: &mut egui::Ui, session: &Session, panels: &mut CrewPanels) {
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new("Inventory").strong());
        theme::question_mark(ui, ITEMS_TIP);
    });
    let room = session.room_ref();
    egui::Grid::new("items")
        .num_columns(3)
        .min_col_width(icons::INLINE)
        .spacing([8.0, 1.0])
        .show(ui, |ui| {
            // The crew's money first: it is what everything under it was
            // bought with, and what the rest of it sells for.
            ui.label("");
            ui.label("Money");
            ui.label(egui::RichText::new(euros(session.remaining())).color(theme::ACCENT));
            ui.end_row();
            for &class in Storage::ALL.iter() {
                let mut first = true;
                for &id in ResourceId::ALL.iter() {
                    if Session::storage_of(id) != class {
                        continue;
                    }
                    let held = match (id, room) {
                        (ResourceId::Vegetable, Some(r)) => r.store_veg(),
                        (ResourceId::Tofu, Some(r)) => r.store_tofu(),
                        _ => session.cargo(id),
                    };
                    if first {
                        ui.label("");
                        ui.label(
                            egui::RichText::new(
                                STORAGE_NAMES.get(class as usize).copied().unwrap_or(""),
                            )
                            .small()
                            .color(theme::MUTED),
                        );
                        ui.end_row();
                        first = false;
                    }
                    icons::resource_cell(ui, id);
                    let row = ui.label(egui::RichText::new(resource_name(id)).color(if held > 0 {
                        theme::INK
                    } else {
                        theme::MUTED
                    }));
                    let count =
                        ui.label(egui::RichText::new(held.to_string()).color(if held > 0 {
                            theme::ACCENT
                        } else {
                            theme::MUTED
                        }));
                    if class == Storage::ColdStore {
                        panels.points(&row, bims::room::SPOT_FRIDGE);
                        panels.points(&count, bims::room::SPOT_FRIDGE);
                    }
                    ui.end_row();
                }
                // Then what is made aboard and kept in the same hold: stew, the
                // first thing made rather than bought.
                if class == Storage::ColdStore
                    && let Some(r) = room
                {
                    let held = r.store_stew();
                    icons::cell(ui, icons::stew);
                    let row = ui.label(egui::RichText::new("Stew, ready").color(if held > 0 {
                        theme::INK
                    } else {
                        theme::MUTED
                    }));
                    let count =
                        ui.label(egui::RichText::new(held.to_string()).color(if held > 0 {
                            theme::ACCENT
                        } else {
                            theme::MUTED
                        }));
                    panels.points(&row, bims::room::SPOT_FRIDGE);
                    panels.points(&count, bims::room::SPOT_FRIDGE);
                    ui.end_row();
                }
            }
            // And the plates, which are the galley's rather than any hold's:
            // clean and in the chopping board's drawer, out of the most it
            // holds. The rest are on the table or in the rack.
            if let Some(r) = room {
                ui.label("");
                ui.label(egui::RichText::new("Drawer").small().color(theme::MUTED));
                ui.end_row();
                let held = r.plates();
                icons::cell(ui, icons::plate);
                let row = ui.label(egui::RichText::new("Plates").color(if held > 0 {
                    theme::INK
                } else {
                    theme::MUTED
                }));
                let count = ui.label(
                    egui::RichText::new(format!("{held} / {}", r.plate_drawer_capacity())).color(
                        if held > 0 {
                            theme::ACCENT
                        } else {
                            theme::MUTED
                        },
                    ),
                );
                panels.points(&row, bims::room::SPOT_BOARD);
                panels.points(&count, bims::room::SPOT_BOARD);
                ui.end_row();
            }
        });
}

/// The hold as the crew's panels want it: what is aboard and not spoken
/// for, the pieces of armour in it, how full each class is, and whether
/// crew member `who` stands within reach of a container that takes each
/// resource — the world's own `in_reach`, asked once a resource, so the
/// rows can say "walk over first" before a command is sent and refused.
fn hold_of(session: &Session, who: usize) -> Hold {
    let mut hold = Hold::default();
    let Some(game) = session.game.as_ref() else {
        return hold;
    };
    let world = &game.world;
    for &id in ResourceId::ALL.iter() {
        hold.counts[id as usize] = world.ship.design.carrying(id);
        hold.reach[id as usize] = world.in_reach(who as u32, id);
    }
    hold.guns = world.guns.clone();
    hold.grids = world.grids.clone();
    for (i, &class) in world::World::GRID_CLASSES.iter().enumerate() {
        hold.grid_capacity[i] = world.grid_capacity(class);
    }
    for piece in world.pieces.iter().filter(|p| p.at == Where::Hold) {
        if let bims::combat::Item::Armour(piece) = piece.item() {
            hold.pieces.push(piece);
        }
    }
    for &class in Storage::ALL.iter() {
        hold.used[class as usize] = session.storage_used(class);
        hold.capacity[class as usize] = session.storage_capacity(class);
    }
    hold.station_desk = world.station_desk();
    hold.station_key = world.key_at_the_dock();
    hold.station_shelves = world.station_shelves();
    hold.bench = world.workbench().map(|index| crate::crew::BenchView {
        index,
        bench: world.bench,
        reach: world.in_reach_of_bench(who as u32),
        upgrade: world.can_upgrade(),
    });
    hold
}

/// What is on the workbench being upgraded, for the Management tab's line
/// under its tick box, off `World::bench`: the work under way, or the
/// output waiting in its slot.
fn upgrade_view(session: &Session) -> Option<UpgradeView> {
    let bench = session.game.as_ref()?.world.bench;
    if let Some(upgrade) = bench.work {
        return Some(UpgradeView {
            resource: upgrade.resource,
            tier: upgrade.to.code(),
            done: upgrade.done.min(world::data::UPGRADE_SESSIONS),
            of: world::data::UPGRADE_SESSIONS,
            waiting: false,
        });
    }
    let out = bench.slots[world::Workbench::OUT]?;
    let tier = match out {
        bims::combat::Item::Weapon(w) => w.tier,
        bims::combat::Item::Armour(p) => p.tier,
        _ => return None,
    };
    Some(UpgradeView {
        resource: world::armour::resource_of_item(out)?,
        tier: tier.code(),
        done: world::data::UPGRADE_SESSIONS,
        of: world::data::UPGRADE_SESSIONS,
        waiting: true,
    })
}

/// The research tree as the crew stand in it, for the Research tab, off
/// `World::research`; and which parts may be laid out, by kind, for the
/// Build tab to leave the rest out.
fn research_view(session: &Session) -> ResearchView {
    let Some(game) = session.game.as_ref() else {
        return ResearchView::default();
    };
    let world = &game.world;
    let research = &world.research;
    let mut view = ResearchView {
        unlocked: research.unlocked,
        current: research.current.map(|n| n.code()),
        fraction: research.fraction(),
        queue: research.queue.iter().map(|n| n.code()).collect(),
        desk: world.research_desk_aboard(),
        powered: world.research_desk_powered(),
        keys: [world.keys_in_desk(1), world.keys_in_desk(2)],
        parts: shipdesign::PartKind::ALL
            .iter()
            .map(|&kind| research.part_allowed(kind))
            .collect(),
        ..ResearchView::default()
    };
    for node in shipdesign::research::Node::ALL {
        let i = node as usize;
        view.done[i] = research.is_done(node);
        view.available[i] = research.available(node);
        view.needs_key[i] = research.needs_key(node);
        view.queueable[i] = research.queueable(node);
    }
    view
}

/// What is within reach of crew member `who`, nearest first, for the
/// panels' nearby strip and the Inventory key: every container that
/// keeps something — the armoury and the drug lab, the shelves, the cold
/// stores, the desks — by the room's own reach (`Game::within_reach`,
/// `data::REACH`), and every body down within reach — a crewmate, or one
/// of the station's people while the rooms are joined — by the world's
/// (`in_reach_of_body`). Named the way their windows are titled.
fn nearby_of(session: &Session, who: usize, name: &dyn Fn(u32) -> String) -> Vec<Near> {
    let Some(game) = session.game.as_ref() else {
        return Vec::new();
    };
    let world = &game.world;
    let room = &world.aboard.room;
    let at = room.bim_pos(who);
    let reach = world::data::REACH;
    let mut found: Vec<(f32, Near)> = Vec::new();
    let workbench = world.workbench().map(Container::Bench);
    let ashore = world.station_shelves();
    let plunder = world.plunder_alongside().is_some();
    for container in world.aboard.containers() {
        // The station's shelves are not the hold's: an enemy's is the
        // Plunder window, a friend's nothing at all.
        if let Container::Shelf(i) = container
            && ashore.contains(&i)
        {
            if plunder
                && room.within_reach(who, container, reach)
                && let Some(frame) = room.container_frame(container)
            {
                found.push((
                    (at - frame.center()).len(),
                    Near {
                        open: Open::Plunder(i),
                        label: PLUNDER_WINDOW.to_string(),
                    },
                ));
            }
            continue;
        }
        // The workbench keeps no class of goods, but it has its slots.
        let keeps =
            crate::crew::container_class(room, container).is_some() || Some(container) == workbench;
        if !keeps || !room.within_reach(who, container, reach) {
            continue;
        }
        let Some(frame) = room.container_frame(container) else {
            continue;
        };
        let label = match container {
            Container::Bench(i) => PartKind::from_code(room.bench_part(i))
                .map(part_name)
                .unwrap_or("Container")
                .to_string(),
            Container::Shelf(_) => STORAGE_WINDOW.to_string(),
            Container::Fridge(_) => COLD_STORE_WINDOW.to_string(),
            Container::Desk(_) => RESEARCH_WINDOW.to_string(),
        };
        found.push((
            (at - frame.center()).len(),
            Near {
                open: Open::Container(container),
                label,
            },
        ));
    }
    let crew = world.aboard.crew_count();
    let residents = world.residents.as_ref().map_or(0, |r| r.aboard.count());
    let bodies = (0..crew)
        .map(world::LootSource::Crew)
        .chain((0..residents).map(world::LootSource::Resident));
    for source in bodies {
        if source == world::LootSource::Crew(who as u32)
            || !world.is_down(source)
            || !world.in_reach_of_body(who as u32, source)
        {
            continue;
        }
        let Some(lies) = world.body_position(source) else {
            continue;
        };
        let whose = match source {
            world::LootSource::Crew(body) => name(body),
            world::LootSource::Resident(body) => name(crew + body),
        };
        found.push((
            (at - lies).len(),
            Near {
                open: Open::Loot(source),
                label: format!("{LOOT_WINDOW} — {whose}"),
            },
        ));
    }
    // And the engineer's deployables within reach (feature 74): a row to
    // pack each up, and nothing else — a sentry never runs out of shots
    // and is never refilled (feature 88). Only an engineer's rows —
    // nobody else can, and the world would only say so.
    if world.class_of(who as u32) == world::Class::Engineer {
        for (d, lies) in world.deployables_in_reach(who as u32) {
            let what = deployable_line(&d);
            found.push((
                (at - lies).len(),
                Near {
                    open: Open::PackUp(d.id),
                    label: format!("{PACK_UP} — {what}"),
                },
            ));
        }
    }
    found.sort_by(|a, b| a.0.total_cmp(&b.0));
    found.into_iter().map(|(_, near)| near).collect()
}

/// A body as the Loot window wants it: what it has on it, whether it is
/// still down, and whether crew member `who` stands within reach of it —
/// the world's own `in_reach_of_body`, so the window can say "walk over
/// first" before a command is sent and refused. `None` for a Bim the
/// world no longer has — a resident once the rooms have parted.
fn body_of(world: &world::World, who: usize, source: world::LootSource) -> Option<Body> {
    Some(Body {
        cells: world.loot_cells(source)?,
        turned: world.loot_turned(source)?,
        counts: world.loot_counts(source)?,
        down: world.is_down(source),
        reach: world.in_reach_of_body(who as u32, source),
    })
}

/// What the benches make, a row each for the Management tab: one per
/// resource a recipe outputs, in `ResourceId` order, with what is aboard,
/// where it is kept, the standing order and the most the hold would take.
fn crafts(session: &Session) -> Vec<Craft> {
    let Some(game) = session.game.as_ref() else {
        return Vec::new();
    };
    ResourceId::ALL
        .iter()
        .copied()
        .filter(|&id| shipdesign::RECIPES.iter().any(|r| r.output.0 == id))
        .map(|id| {
            let class = Session::storage_of(id);
            Craft {
                resource: id,
                held: session.cargo(id),
                target: game.world.craft_target(id),
                most: game.world.ship.design.most_of(id),
                kept_in: STORAGE_NAMES.get(class as usize).copied().unwrap_or(""),
                recipe: recipe_lines(id),
                // The first recipe for it that is not researched, if none
                // is: a thing two benches make is made at whichever is known.
                needs: {
                    let rows: Vec<usize> = shipdesign::RECIPES
                        .iter()
                        .enumerate()
                        .filter(|(_, r)| r.output.0 == id)
                        .map(|(i, _)| i)
                        .collect();
                    if rows.iter().any(|&i| game.world.research.recipe_allowed(i)) {
                        None
                    } else {
                        rows.first()
                            .map(|&i| shipdesign::research::node_of_recipe(i).code())
                    }
                },
            }
        })
        .collect()
}

/// The construction sites laid out, for the Build tab's list, in the order
/// the crew take them.
fn sites(session: &Session) -> Vec<crate::crew::Site> {
    let Some(game) = session.game.as_ref() else {
        return Vec::new();
    };
    let design = &game.world.ship.design;
    game.world
        .builds
        .iter()
        .map(|site| crate::crew::Site {
            id: site.id,
            kind: site.kind,
            at: site.origin,
            progress: site_progress(site, design),
            affordable: game.world.affordable_site(site),
        })
        .collect()
}

/// Every recipe that makes `resource`, in words: "2 ore → 1 metal at the
/// smelter, 30 min". Off the table, so the words cannot drift from what the
/// benches do.
fn recipe_lines(resource: ResourceId) -> String {
    let lower = |id: ResourceId| resource_name(id).to_lowercase();
    shipdesign::RECIPES
        .iter()
        .filter(|r| r.output.0 == resource)
        .map(|r| {
            let inputs: Vec<String> = r
                .inputs
                .iter()
                .map(|&(id, units)| format!("{units} {}", lower(id)))
                .collect();
            format!(
                "{} → {} {} at the {}, {} min.",
                inputs.join(" + "),
                r.output.1,
                lower(resource),
                part_name(r.station).to_lowercase(),
                r.minutes
            )
        })
        .collect::<Vec<_>>()
        .join(" ")
}

#[allow(dead_code)]
fn unused(_: ShipState) {}

/// The picture in an ability box (feature 80). A kit or a grenade is the
/// thing itself, out of `icons.rs`; everything else is the mark the deck
/// already draws for it, so a box and what happens when it is pressed
/// are one picture.
#[derive(Clone, Copy, PartialEq, Debug)]
enum Mark {
    Thing(ResourceId),
    Brace,
    Surge,
    Beam,
    Taunt,
    Wall,
    Rally,
    Squad,
    /// The commander's other two squad orders (feature 86), which had
    /// keys and no box until the user asked for all of his abilities to
    /// be shown.
    FallBack,
    StandGround,
    /// The medic's carry (feature 86).
    Carry,
}

/// One of the two boxes at the foot of the screen (feature 80): what one
/// of the class's own keys does, how many uses are left, and whether it
/// could be pressed now. Read off the world every frame — nothing here
/// is kept between frames, the way the crew panel's rows are not.
struct AbilityBox {
    /// The key bound to it, spelt as the Controls page spells it.
    key: String,
    name: &'static str,
    tip: &'static str,
    mark: Mark,
    /// How many are left: kits or grenades in the pack, beams free to
    /// link, the squad's size. `None` where nothing is counted — a wall
    /// is a switch, not a stock.
    count: Option<u32>,
    /// Seconds of the clock until it may be used again; nought when it
    /// may.
    cooldown: f64,
    /// The whole of that cooldown, in the same seconds: what the sweep
    /// over the picture is a share of, the way Dota 2 draws a skill
    /// coming back. Nought where there is nothing to sweep.
    cooldown_whole: f64,
    /// For a stock of charges that is neither full nor empty, how far
    /// the next one has come back, nought to one: the ring round the
    /// count in the corner. `None` when it is full, when it is empty —
    /// the sweep over the whole box says it then — and for anything that
    /// is not a stock of charges.
    recharge: Option<f32>,
    /// How charged it is, nought to one — the medic's surge alone.
    charge: Option<f32>,
    /// Whether it is running now: braced, the wall up, a beam held, a
    /// taunt, a rally, the squad under an order.
    on: bool,
    /// Out of stock — no kit, no grenade, a surge not charged. Told from
    /// a count of nought that is not a stock (no beam free to link, an
    /// empty squad), which does not stop the key.
    short: bool,
    /// The level it is learnt at, where the crew member is not there yet.
    locked: Option<u8>,
    /// The key this box is for (feature 86), so the frame can ask the
    /// world **who the cast would reach** while the pointer rests on it.
    /// `None` for the medicine's two, which no key casts.
    action: Option<Action>,
}

/// What one box shows, short of its key and its words: what
/// `ability_boxes` works out for each key and `medicine_boxes` for the
/// two stocks of medicine. See [`AbilityBox`] for each field.
struct Face {
    mark: Mark,
    count: Option<u32>,
    cooldown: f64,
    cooldown_whole: f64,
    recharge: Option<f32>,
    charge: Option<f32>,
    on: bool,
    short: bool,
}

impl Face {
    /// The picture and nothing else about it.
    fn of(mark: Mark) -> Face {
        Face {
            mark,
            count: None,
            cooldown: 0.0,
            cooldown_whole: 0.0,
            recharge: None,
            charge: None,
            on: false,
            short: false,
        }
    }

    /// A stock of charges: how many are in the pack, and the cooldown
    /// told the way Dota 2 tells one — **with none left**, the seconds
    /// until the next lands swept over the whole box; with some left and
    /// the next on its way, how far it has come back, for the ring round
    /// the count. A box with a charge in it is ready whatever the
    /// cooldown is doing, so the seconds are shown only when they are
    /// what is actually in the way.
    fn charges(world: &world::World, slot: u32, charge: world::Charge, mark: Mark) -> Face {
        let held = world.charges_of(slot, charge);
        let whole = world.charge_cooldown(slot, charge);
        let left = world.charge_cooldown_left(slot, charge);
        let mut face = Face {
            count: Some(held),
            short: held == 0,
            ..Face::of(mark)
        };
        if held == 0 {
            face.cooldown = left;
            face.cooldown_whole = whole;
        } else if held < world.charges(slot, charge) && left > 0.0 && whole > 0.0 {
            face.recharge = Some((1.0 - left / whole).clamp(0.0, 1.0) as f32);
        }
        face
    }
}

/// The medicine's two boxes (a medkit and the bandages), which every
/// crew member has whatever its class, beside the class's own: the
/// charges in the pack, the sweep while none is left, and the ring while
/// the next is coming back. No key casts either, so neither is `ready`
/// for anything but the look of it.
fn medicine_boxes(world: &world::World, slot: u32) -> Vec<AbilityBox> {
    if slot >= world.aboard.crew_count() {
        return Vec::new();
    }
    [
        (
            world::Charge::Medkit,
            crate::names::MEDKIT_BOX,
            crate::names::MEDKIT_BOX_TIP,
        ),
        (
            world::Charge::Bandage,
            crate::names::BANDAGE_BOX,
            crate::names::BANDAGE_BOX_TIP,
        ),
    ]
    .into_iter()
    .map(|(charge, name, tip)| {
        let face = Face::charges(world, slot, charge, Mark::Thing(charge.resource()));
        AbilityBox {
            key: String::new(),
            name,
            tip,
            mark: face.mark,
            count: face.count,
            cooldown: face.cooldown,
            cooldown_whole: face.cooldown_whole,
            recharge: face.recharge,
            charge: face.charge,
            on: face.on,
            short: face.short,
            locked: None,
            action: None,
        }
    })
    .collect()
}

impl AbilityBox {
    /// Whether the key would be taken now, as far as the box can tell:
    /// the level reached, out of the cooldown, something left to spend.
    /// What the world says when the key is actually pressed is
    /// `class_key`'s, and it knows about the pointer as well.
    fn ready(&self) -> bool {
        self.locked.is_none() && self.cooldown <= 0.0 && !self.short
    }
}

/// Every key the class `slot` steers has a box for, in the order they
/// are laid out: Q and E for everybody, and past them whatever else that
/// class has a key of its own for (feature 86) — the commander's fall
/// back and stand ground, which had keys and no boxes, and the medic's
/// carry. A crew member with no class has no keys and no boxes.
fn ability_keys(world: &world::World, slot: u32) -> Vec<Action> {
    use world::Class;
    let class = world.class_of(slot);
    let mut keys = match class {
        Class::None => return Vec::new(),
        _ => vec![Action::ClassPrimary, Action::ClassSecondary],
    };
    if class == Class::Commander {
        keys.push(Action::SquadFallBack);
        keys.push(Action::SquadStandGround);
    }
    if world.can_lift(slot) {
        keys.push(Action::Carry);
    }
    keys
}

/// The boxes for the class `slot` steers, primary (Q) first. Empty
/// for a classless crew member, which has no keys.
fn ability_boxes(world: &world::World, slot: u32, keys: &Keys) -> Vec<AbilityBox> {
    use world::Class;
    let class = world.class_of(slot);
    if class == Class::None {
        return Vec::new();
    }
    let level = world.progress_of(slot).level();
    ability_keys(world, slot)
        .into_iter()
        .map(|action| {
            let primary = action == Action::ClassPrimary;
            // The keys past Q and E are gated by the same level their
            // own class's E is, being the same order sent by another
            // name; the carry is nobody's level at all.
            let wants = match action {
                Action::Carry => 1,
                Action::ClassPrimary => world::class::key_level(class, true).unwrap_or(1),
                _ => world::class::key_level(class, false).unwrap_or(1),
            };
            // The three keys of their own first, since they are one
            // class's each and read off the world rather than off the
            // (class, primary) pair below.
            let extra = match action {
                Action::SquadFallBack | Action::SquadStandGround => {
                    let running = world.squad.as_ref().is_some_and(|o| {
                        o.by_slot == slot
                            && o.kind.code() == u32::from(action == Action::SquadStandGround) + 1
                    });
                    Some(Face {
                        count: Some(world.squad_members(slot).len() as u32),
                        on: running,
                        ..Face::of(if action == Action::SquadFallBack {
                            Mark::FallBack
                        } else {
                            Mark::StandGround
                        })
                    })
                }
                Action::Carry => {
                    // Carrying, the box counts nothing: a nought in the
                    // corner reads as "nothing to do", and what the key
                    // does now is set this one down. Empty-handed it is
                    // how many are near enough to pick up.
                    let carrying = world.carrying_of(slot).is_some();
                    let near = world.carryable_near(slot).len() as u32;
                    Some(Face {
                        count: (!carrying).then_some(near),
                        on: carrying,
                        short: !carrying && near == 0,
                        ..Face::of(Mark::Carry)
                    })
                }
                _ => None,
            };
            let face = if let Some(extra) = extra {
                extra
            } else {
                match (class, primary) {
                    // The engineer's two boxes and the soldier's grenade
                    // count the charges in the pack (features 88 and 90)
                    // and, **with none left**, sweep the seconds until
                    // the next one lands; with some left and the next
                    // on its way, the ring round the count fills instead.
                    (Class::Engineer, true) => Face::charges(
                        world,
                        slot,
                        world::Charge::Sentry,
                        Mark::Thing(ResourceId::SentryKit),
                    ),
                    (Class::Engineer, false) => Face::charges(
                        world,
                        slot,
                        world::Charge::Sandbag,
                        Mark::Thing(ResourceId::SandbagKit),
                    ),
                    (Class::Soldier, true) => Face::charges(
                        world,
                        slot,
                        world::Charge::Grenade,
                        Mark::Thing(ResourceId::Grenade),
                    ),
                    (Class::Soldier, false) => Face {
                        on: world.is_braced(slot),
                        ..Face::of(Mark::Brace)
                    },
                    (Class::Medic, true) => {
                        let charge = world.surge_charge(slot);
                        Face {
                            charge: Some(charge),
                            on: world.is_surging(slot),
                            short: charge < 1.0,
                            ..Face::of(Mark::Surge)
                        }
                    }
                    (Class::Medic, false) => {
                        let held = world.patients_of(slot).len();
                        Face {
                            count: Some(world.beam_patients(slot).saturating_sub(held) as u32),
                            on: held > 0,
                            ..Face::of(Mark::Beam)
                        }
                    }
                    // The two cooldowns that are not charges sweep the
                    // same way, over the whole of their own length.
                    (Class::Tank, true) => Face {
                        cooldown: world.taunt_cooldown_left(slot),
                        cooldown_whole: world::class::TAUNT_COOLDOWN,
                        on: world.taunt_left(slot) > 0.0,
                        ..Face::of(Mark::Taunt)
                    },
                    (Class::Tank, false) => Face {
                        on: world.is_bulwark(slot),
                        ..Face::of(Mark::Wall)
                    },
                    (Class::Commander, true) => Face {
                        cooldown: world.rally_cooldown_left(slot),
                        cooldown_whole: world.rally_cooldown(slot),
                        on: world.rally_left(slot) > 0.0,
                        ..Face::of(Mark::Rally)
                    },
                    (Class::Commander, false) => Face {
                        count: Some(world.squad_members(slot).len() as u32),
                        on: world.squad.as_ref().is_some_and(|o| o.by_slot == slot),
                        ..Face::of(Mark::Squad)
                    },
                    (Class::None, _) => Face::of(Mark::Brace),
                }
            };
            let (name, tip) = match action {
                Action::SquadFallBack => (crate::names::FALL_BACK, crate::names::FALL_BACK_TIP),
                Action::SquadStandGround => {
                    (crate::names::STAND_GROUND, crate::names::STAND_GROUND_TIP)
                }
                Action::Carry => (crate::names::CARRY, crate::names::CARRY_TIP),
                _ => (ability_name(class, primary), ability_tip(class, primary)),
            };
            AbilityBox {
                key: keys.key(action).name().to_string(),
                name,
                tip,
                mark: face.mark,
                count: face.count,
                cooldown: face.cooldown,
                cooldown_whole: face.cooldown_whole,
                recharge: face.recharge,
                charge: face.charge,
                on: face.on,
                short: face.short,
                locked: (level < wants).then_some(wants),
                action: Some(action),
            }
        })
        .collect()
}

/// How big one box is, and how much room its name wants under it.
const ABILITY_SIDE: f32 = 54.0;

/// The bar itself: the boxes in a frame at the foot of the canvas,
/// centred on it and never over the tray at its left, which grows a
/// whole panel when a tab is open. Its own width and the tray's are
/// last frame's, the way the trip strip's is — egui's own anchoring
/// reads the same memory — so the first frame guesses and every frame
/// after is exact.
/// Whom a cast of `slot`'s would reach, by crew index (feature 86) —
/// what the deck rings while the pointer rests on that key's box, which
/// is the panels' own rule (resting on a row rings what it names) said
/// about an ability instead of a fixture.
///
/// The commander's are the point of it: his **rally** lifts every
/// friendly Bim in his aura, and each of his three **squad** keys
/// commands the same squad — every crew member nobody is steering,
/// within his range. The medic's beam and surge name their patients,
/// and the carry names everybody near enough to pick up. The rest reach
/// enemies or nobody, and ring nothing.
fn affected_by(world: &world::World, slot: u32, action: Action) -> Vec<u32> {
    use world::Class;
    let class = world.class_of(slot);
    match (class, action) {
        // The rally: every friendly Bim in his aura, and **himself** —
        // `aura_reaching` leaves a commander out of his own aura, and
        // the rally is the one thing that covers him.
        (Class::Commander, Action::ClassPrimary) => (0..world.aboard.crew_count())
            .filter(|&who| who == slot || world.aura_reaching(who).is_some())
            .collect(),
        (
            Class::Commander,
            Action::ClassSecondary | Action::SquadFallBack | Action::SquadStandGround,
        ) => world.squad_members(slot),
        (Class::Medic, Action::ClassPrimary) => {
            let mut held = world.patients_of(slot);
            held.push(slot);
            held.sort_unstable();
            held
        }
        (Class::Medic, Action::ClassSecondary) => world.patients_of(slot),
        (_, Action::Carry) => match world.carrying_of(slot) {
            Some(patient) => vec![patient],
            None => world.carryable_near(slot),
        },
        _ => Vec::new(),
    }
}

/// The medicine's two boxes go at the right-hand end, past a rule, so
/// the class's keys are one group and what everybody carries another.
fn ability_bar(
    ctx: &egui::Context,
    canvas: crate::shapes::Rect,
    boxes: &[AbilityBox],
    medicine: &[AbilityBox],
) -> Option<Action> {
    if boxes.is_empty() && medicine.is_empty() {
        return None;
    }
    let id = egui::Id::new("game-abilities");
    let rect_of = |id| ctx.memory(|m| m.area_rect(id));
    let size = rect_of(id).map_or(egui::vec2(160.0, 96.0), |r| r.size());
    let tray = rect_of(egui::Id::new("game-tray")).map_or(canvas.min.x, |r| r.max.x);
    let x = ((canvas.min.x + canvas.max.x - size.x) / 2.0)
        .max(tray + 8.0)
        .min(canvas.max.x - 10.0 - size.x);
    let y = canvas.max.y - 10.0 - size.y;
    let mut hovered = None;
    egui::Area::new(id)
        .fixed_pos(egui::pos2(x, y))
        .order(egui::Order::Middle)
        .show(ctx, |ui| {
            panel_frame().show(ui, |ui| {
                ui.horizontal(|ui| {
                    for one in boxes {
                        if ability_box(ui, one) {
                            hovered = one.action;
                        }
                    }
                    if !boxes.is_empty() && !medicine.is_empty() {
                        ui.separator();
                    }
                    for one in medicine {
                        ability_box(ui, one);
                    }
                });
            });
        });
    hovered
}

/// One box: the key in the corner, the picture in the middle, what is
/// left in the other corner, and the name under it. Resting on it says
/// what the key does — a box is a control, so it carries its own words
/// rather than an underlined one beside it.
/// Whether the pointer is resting on it — what the deck rings the cast's
/// own Bims by (feature 86).
fn ability_box(ui: &mut egui::Ui, one: &AbilityBox) -> bool {
    ui.vertical(|ui| {
        ui.spacing_mut().item_spacing.y = 2.0;
        let (rect, response) =
            ui.allocate_exact_size(egui::vec2(ABILITY_SIDE, ABILITY_SIDE), egui::Sense::hover());
        let ready = one.ready();
        let painter = ui.painter();
        let edge = if one.on {
            theme::CAUTION
        } else if ready {
            theme::ACCENT
        } else {
            theme::LINE
        };
        painter.rect_filled(
            rect,
            4.0,
            if one.on {
                theme::RAISED_ON
            } else {
                theme::RAISED
            },
        );
        // The picture, dimmed while the key would not be taken — the
        // whole box reads as off rather than the number alone.
        let inner = rect.shrink(11.0);
        let middle = inner.center();
        let radius = inner.width() / 2.0;
        match one.mark {
            Mark::Thing(id) => icons::resource(painter, inner, id),
            Mark::Brace => theme::brace_mark(painter, middle, radius),
            Mark::Surge => theme::surge_mark(painter, middle, radius / 34.0),
            Mark::Beam => theme::heal_beam(
                painter,
                egui::pos2(inner.min.x, inner.max.y),
                egui::pos2(inner.max.x, inner.min.y),
                1.4,
            ),
            Mark::Taunt => theme::taunt_ring(painter, middle, radius),
            Mark::Wall => theme::wall_mark(painter, middle, radius, 1.2),
            Mark::Rally => {
                theme::aura_ring(painter, middle, radius);
                theme::lifted_mark(painter, middle, radius / 10.0);
            }
            Mark::Squad => theme::squad_mark(
                painter,
                egui::pos2(middle.x, middle.y + radius * 0.4),
                Some(egui::pos2(middle.x, inner.min.y)),
                radius / 9.0,
            ),
            Mark::FallBack => theme::fall_back_mark(painter, middle, radius),
            Mark::StandGround => theme::stand_ground_mark(painter, middle, radius),
            Mark::Carry => theme::carry_mark(painter, middle, radius),
        }
        // Off, the box goes dark — and a cooldown goes dark the way Dota
        // 2 draws one: only the share still to come, swept back
        // clockwise from twelve o'clock as it runs out, so how long is
        // left is read off the picture before the seconds.
        let shade = theme::PANEL_DEEP.gamma_multiply(0.62);
        if one.locked.is_none() && one.cooldown > 0.0 && one.cooldown_whole > 0.0 {
            let left = (one.cooldown / one.cooldown_whole).clamp(0.0, 1.0) as f32;
            cooldown_sweep(painter, rect, left, theme::PANEL_DEEP.gamma_multiply(0.85));
        } else if !ready {
            painter.rect_filled(rect, 4.0, shade);
        }
        painter.rect_stroke(
            rect,
            4.0,
            egui::Stroke::new(1.0, edge),
            egui::StrokeKind::Inside,
        );
        // The key, in the top left.
        painter.text(
            rect.min + egui::vec2(4.0, 3.0),
            egui::Align2::LEFT_TOP,
            &one.key,
            egui::FontId::proportional(11.0),
            if ready { theme::INK } else { theme::MUTED },
        );
        // What is left, in the bottom right — nothing where nothing is
        // counted. A stock of charges (a thing out of the pack: a kit, a
        // grenade, the medicine) has its count on a disc of its own, and
        // the ring round the disc fills clockwise while the next charge
        // comes back — Dota 2's charge counter.
        if let Some(count) = one.count {
            let ink = if count == 0 { theme::MUTED } else { theme::INK };
            if matches!(one.mark, Mark::Thing(_)) {
                let at = rect.max - egui::vec2(CHARGE_BADGE + 2.0, CHARGE_BADGE + 2.0);
                recharge_badge(painter, at, one.recharge);
                painter.text(
                    at,
                    egui::Align2::CENTER_CENTER,
                    format!("{count}"),
                    egui::FontId::proportional(12.0),
                    ink,
                );
            } else {
                painter.text(
                    rect.max - egui::vec2(4.0, 3.0),
                    egui::Align2::RIGHT_BOTTOM,
                    format!("{count}"),
                    egui::FontId::proportional(15.0),
                    ink,
                );
            }
        }
        // The surge's charge, as a bar along the foot.
        if let Some(charge) = one.charge {
            let bar = egui::Rect::from_min_max(
                egui::pos2(rect.min.x + 4.0, rect.max.y - 7.0),
                egui::pos2(rect.max.x - 4.0, rect.max.y - 4.0),
            );
            theme::bar_in(painter, bar, charge, theme::HEAL);
        }
        // The cooldown, over the picture, and the level it is learnt at
        // where it is not learnt yet — one or the other, never both.
        let over = if let Some(level) = one.locked {
            Some((ability_locked(level), theme::MUTED))
        } else if one.cooldown > 0.0 {
            Some((format!("{:.0}s", one.cooldown), theme::CAUTION))
        } else {
            None
        };
        if let Some((words, color)) = over {
            // A shadow under the words, since they sit over the picture.
            painter.text(
                middle + egui::vec2(1.0, 1.0),
                egui::Align2::CENTER_CENTER,
                &words,
                egui::FontId::proportional(13.0),
                theme::PANEL_DEEP,
            );
            painter.text(
                middle,
                egui::Align2::CENTER_CENTER,
                words,
                egui::FontId::proportional(13.0),
                color,
            );
        }
        ui.label(egui::RichText::new(one.name).small().color(if ready {
            theme::INK
        } else {
            theme::MUTED
        }));
        let resting = response.hovered();
        response.on_hover_text(one.tip);
        resting
    })
    .inner
}

/// The radius of the disc a stock of charges is counted on, in the
/// bottom right of its box.
const CHARGE_BADGE: f32 = 8.0;

/// Dota 2's clock over a box on cooldown: `left` of a whole turn still
/// to come, laid dark from where the sweep has got to round clockwise
/// to twelve o'clock, so the lit part grows clockwise from the top as
/// the cooldown runs out. A fan from the middle of the box out to its
/// edge — through the corners the sweep has not passed yet, since
/// between two of them the edge is straight and the fan is exact — so
/// the corners go dark with the rest. A thin line marks the hand.
fn cooldown_sweep(painter: &egui::Painter, rect: egui::Rect, left: f32, shade: egui::Color32) {
    use std::f32::consts::{FRAC_PI_4, TAU};
    if left <= 0.0 {
        return;
    }
    let middle = rect.center();
    let half = rect.size() / 2.0;
    // Where a ray from the middle at `a` (clockwise from straight up)
    // leaves the box.
    let edge = |a: f32| {
        let way = egui::vec2(a.sin(), -a.cos());
        let reach = (half.x / way.x.abs().max(1e-6)).min(half.y / way.y.abs().max(1e-6));
        middle + way * reach
    };
    let from = (1.0 - left.min(1.0)) * TAU;
    let mut mesh = egui::Mesh::default();
    mesh.colored_vertex(middle, shade);
    mesh.colored_vertex(edge(from), shade);
    for corner in [1.0, 3.0, 5.0, 7.0].map(|k| k * FRAC_PI_4) {
        if corner > from {
            mesh.colored_vertex(edge(corner), shade);
        }
    }
    mesh.colored_vertex(edge(TAU), shade);
    for i in 1..mesh.vertices.len() as u32 - 1 {
        mesh.add_triangle(0, i, i + 1);
    }
    painter.add(egui::Shape::mesh(mesh));
    if left < 1.0 {
        painter.line_segment(
            [middle, edge(from)],
            egui::Stroke::new(1.0, theme::INK.gamma_multiply(0.45)),
        );
    }
}

/// The disc a stock of charges is counted on, and — while the next
/// charge is coming back — the ring round it filling clockwise from
/// twelve o'clock by `recharge`, over a faint track of the whole ring.
fn recharge_badge(painter: &egui::Painter, at: egui::Pos2, recharge: Option<f32>) {
    use std::f32::consts::TAU;
    painter.circle_filled(at, CHARGE_BADGE, theme::PANEL_DEEP.gamma_multiply(0.9));
    let Some(share) = recharge else {
        return;
    };
    let ring = CHARGE_BADGE + 1.5;
    painter.circle_stroke(at, ring, egui::Stroke::new(2.0, theme::LINE));
    let steps = ((share * 32.0).ceil() as usize).max(1);
    let arc: Vec<egui::Pos2> = (0..=steps)
        .map(|i| {
            let a = share * TAU * i as f32 / steps as f32;
            at + egui::vec2(a.sin(), -a.cos()) * ring
        })
        .collect();
    painter.add(egui::Shape::line(arc, egui::Stroke::new(2.0, theme::ACCENT)));
}

/// What the class's two keys do for the crew member `slot` steers
/// (features 74, 75 and 76): `primary` is Q, the other E, `tile` the
/// room tile under the pointer if it is over the deck, and `under` the
/// crew member under the pointer if there is one. An engineer's Q sets
/// a sentry up on the tile and its E lays sandbags there; a soldier's Q
/// throws a grenade at it and its E braces or stands easy; a medic's Q
/// triggers its surge and its E beams `under` — pressed on the one it
/// already holds, or on nobody while one is held, it unlinks, and on
/// nobody with no beam on it says so; a tank's Q taunts and its E puts
/// the wall up or takes it down. The order to send, if the
/// world would take it, and the log's line saying why not if it would
/// not — both off the world's own check, so the key and the command
/// agree. A classless crew member's keys do nothing at all.
fn class_key(
    world: &world::World,
    slot: u32,
    primary: bool,
    tile: Option<(i32, i32)>,
    under: Option<u32>,
    enemy: Option<u32>,
) -> (Option<Order>, Option<String>) {
    match world.class_of(slot) {
        world::Class::None => (None, None),
        world::Class::Engineer => {
            let kit = if primary {
                world::Kit::Sentry
            } else {
                world::Kit::Sandbag
            };
            let Some(tile) = tile else {
                return (None, Some(deploy_refused(Refusal::CantDeployThere)));
            };
            match world.can_deploy(slot, kit, tile) {
                Ok(()) => (
                    Some(Order::Deploy {
                        kit,
                        x: tile.0,
                        y: tile.1,
                    }),
                    None,
                ),
                Err(why) => (None, Some(deploy_refused(why))),
            }
        }
        world::Class::Soldier if primary => {
            let Some(tile) = tile else {
                return (None, Some(throw_refused(Refusal::CantThrowThere)));
            };
            match world.can_throw(slot, tile) {
                Ok(()) => (
                    Some(Order::Throw {
                        x: tile.0,
                        y: tile.1,
                    }),
                    None,
                ),
                Err(why) => (None, Some(throw_refused(why))),
            }
        }
        world::Class::Soldier => match world.can_brace(slot) {
            Ok(()) => (Some(Order::Brace(!world.is_braced(slot))), None),
            Err(why) => (None, Some(brace_refused(why))),
        },
        // The medic (feature 76): Q surges, E beams the crew member
        // under the pointer — on the one already held, or on nobody
        // while one is held, it unlinks.
        world::Class::Medic if primary => match world.can_surge(slot) {
            Ok(()) => (Some(Order::Surge), None),
            Err(why) => (None, Some(surge_refused(why))),
        },
        world::Class::Medic => match under {
            None if world.is_beaming(slot) => (Some(Order::Beam(None)), None),
            None => (None, Some(beam_refused(Refusal::NoPatient))),
            Some(patient) if world.patients_of(slot).contains(&patient) => {
                (Some(Order::Beam(None)), None)
            }
            Some(patient) => match world.can_beam(slot, patient) {
                Ok(()) => (Some(Order::Beam(Some(patient))), None),
                Err(why) => (None, Some(beam_refused(why))),
            },
        },
        // The tank (feature 77): Q taunts, E puts the wall up and down.
        world::Class::Tank if primary => match world.can_taunt(slot) {
            Ok(()) => (Some(Order::Taunt), None),
            Err(why) => (None, Some(taunt_refused(why))),
        },
        world::Class::Tank => match world.can_bulwark(slot) {
            Ok(()) => (Some(Order::Bulwark(!world.is_bulwark(slot))), None),
            Err(why) => (None, Some(bulwark_refused(why))),
        },
        // The commander (feature 78): Q rallies, E sends the squad at
        // the enemy under the pointer.
        world::Class::Commander if primary => match world.can_rally(slot) {
            Ok(()) => (Some(Order::Rally), None),
            Err(why) => (None, Some(rally_refused(why))),
        },
        world::Class::Commander => {
            let Some(enemy) = enemy else {
                return (None, Some(squad_refused(Refusal::NoEnemyThere)));
            };
            squad_key(world, slot, world::SquadAsk::Attack { enemy })
        }
    }
}

/// A commander's squad key: the order if the world would take it, and
/// the log's line saying why not if it would not — the same shape as
/// `class_key`'s, and the same check the command makes. X and Z go
/// through this straight; E goes through `class_key` first, which finds
/// the enemy under the pointer.
fn squad_key(
    world: &world::World,
    slot: u32,
    ask: world::SquadAsk,
) -> (Option<Order>, Option<String>) {
    match world.can_squad(slot) {
        Ok(()) => (Some(Order::Squad(ask)), None),
        Err(why) => (None, Some(squad_refused(why))),
    }
}

/// Every player's own standing order (feature 84), the same shape: the
/// order if the world would take it, and the log's line if it would
/// not. F comes through here with the tile it was clicked on, T with a
/// retreat and nothing to point at. It is not a class's key — every
/// player has these two — so the only refusal is being unfit to act,
/// and an attack's on ground that is not there.
fn orders_key(
    world: &world::World,
    slot: u32,
    order: world::Standing,
) -> (Option<Order>, Option<String>) {
    match world.can_order(slot) {
        Ok(()) => (Some(Order::Orders(order)), None),
        Err(why) => (None, Some(orders_refused(why))),
    }
}

/// What the **Attack** key does (feature 84), given the player's
/// standing order and whether the pointer is already armed: the order to
/// give — always a release, never a fresh banner — and what the pointer
/// is armed to afterwards.
///
/// Three cases. A banner already down and the pointer at rest: the
/// banner is **taken up**, since the world reads the same order given
/// again as a release ([`world::Standing::same_as`]) and *the same
/// order* means the same **tile** — a second banner anywhere else is a
/// fresh attack, so without this there is no press that ever lets the
/// crew go and they stand under arms at the banner for ever, taking no
/// errand. The pointer already armed: thought better of. Anything else:
/// armed, so the click after it picks the ground — unless the map is up,
/// which has no deck to put a banner on.
fn attack_key(
    standing: world::Standing,
    armed: bool,
    map_up: bool,
) -> (Option<world::Standing>, bool) {
    match standing {
        world::Standing::Attack { .. } if !armed => (Some(standing), false),
        _ => (None, !armed && !map_up),
    }
}

/// The medic's carry key (feature 86), the same shape as the rest: the
/// order if the world would take it, and the log's line if it would not.
/// `under` is the crew member under the pointer, if any.
///
/// * arms full, and it is a set down, wherever the pointer is — which
///   is the key's second press and the whole of how a body is put back
///   on the deck;
/// * arms free and somebody under the pointer, and it is that one;
/// * arms free and nobody under the pointer, and it is the nearest
///   worth fetching (`World::carryable_near`), so the key is worth
///   pressing in a fight without aiming it — and the world's own reason
///   when there is nobody at all.
fn carry_key(
    world: &world::World,
    slot: u32,
    under: Option<u32>,
) -> (Option<Order>, Option<String>) {
    if !world.can_lift(slot) {
        return (None, Some(carry_refused(Refusal::NotCarrying)));
    }
    if world.carrying_of(slot).is_some() {
        return (Some(Order::Carry(None)), None);
    }
    let patient = under.or_else(|| world.carryable_near(slot).first().copied());
    let Some(patient) = patient else {
        return (None, Some(carry_refused(Refusal::NotHurt)));
    };
    match world.can_carry(slot, patient) {
        Ok(()) => (Some(Order::Carry(Some(patient))), None),
        Err(why) => (None, Some(carry_refused(why))),
    }
}

#[cfg(test)]
mod class_key_tests {
    use super::*;
    use crate::names;
    use shipdesign::fixture::flyer;
    use world::fixture::{REFERENCE_MONEY, simulation_world};

    /// Q and E dispatch by the steered crew member's class: an engineer's
    /// deploys, a soldier's throws and braces, a classless one's do
    /// nothing — each with the world's own reason when refused.
    #[test]
    fn the_class_keys_dispatch_by_the_class_steered() {
        let mut world = simulation_world(flyer(3), REFERENCE_MONEY, 3);
        assert_eq!(world.set_class(0, world::Class::Engineer), Ok(()));
        assert_eq!(world.set_class(1, world::Class::Soldier), Ok(()));
        let t = shipdesign::TILE as f32;
        let tile_of = |world: &world::World, who: u32| {
            let p = world.aboard.room.bim_pos(who as usize);
            ((p.x / t).floor() as i32, (p.y / t).floor() as i32)
        };
        // Nothing for a classless crew member, key or no key, tile or none.
        assert_eq!(class_key(&world, 2, true, None, None, None), (None, None));
        assert_eq!(
            class_key(&world, 2, false, Some(tile_of(&world, 2)), Some(1), None),
            (None, None)
        );
        // The engineer: a sandbag kit on its own tile is refused (a tile
        // with a body on it, and its own at that), a tile beside it is a
        // deploy; a sentry wants the third level.
        let here = tile_of(&world, 0);
        let beside = (1..6)
            .flat_map(|r| {
                [
                    (here.0 + r, here.1),
                    (here.0 - r, here.1),
                    (here.0, here.1 + r),
                    (here.0, here.1 - r),
                ]
            })
            .find(|&tile| world.can_deploy(0, world::Kit::Sandbag, tile).is_ok())
            .expect("a free tile beside the engineer");
        assert_eq!(
            class_key(&world, 0, false, Some(beside), None, None),
            (
                Some(Order::Deploy {
                    kit: world::Kit::Sandbag,
                    x: beside.0,
                    y: beside.1
                }),
                None
            )
        );
        let (order, line) = class_key(&world, 0, true, Some(beside), None, None);
        assert_eq!(order, None);
        assert_eq!(
            line,
            Some(deploy_refused(Refusal::NoSentryYet)),
            "the kit is its class's, the level is not reached"
        );
        assert_eq!(
            class_key(&world, 0, false, None, None, None),
            (None, Some(deploy_refused(Refusal::CantDeployThere))),
            "no tile under the pointer"
        );
        // The soldier: E braces, and E again stands easy; Q wants the
        // third level, then throws at a tile within range.
        assert_eq!(
            class_key(&world, 1, false, None, None, None),
            (Some(Order::Brace(true)), None)
        );
        world.step(&[world::Command::Brace { slot: 1, on: true }]);
        assert_eq!(
            class_key(&world, 1, false, Some(tile_of(&world, 1)), None, None),
            (Some(Order::Brace(false)), None)
        );
        let target = tile_of(&world, 1);
        assert_eq!(
            class_key(&world, 1, true, Some(target), None, None),
            (None, Some(throw_refused(Refusal::NoGrenadesYet)))
        );
        let mut events = Vec::new();
        world.award(1, world::class::LEVEL_XP[2], &mut events);
        assert_eq!(
            class_key(&world, 1, true, Some(target), None, None),
            (
                Some(Order::Throw {
                    x: target.0,
                    y: target.1
                }),
                None
            )
        );
        assert_eq!(
            class_key(&world, 1, true, None, None, None),
            (None, Some(throw_refused(Refusal::CantThrowThere)))
        );
        // The medic: Q surges — refused before the third level and
        // unlinked — and E beams whoever is under the pointer, unlinks
        // on the one already held and on nobody.
        assert_eq!(world.set_class(2, world::Class::Medic), Ok(()));
        assert_eq!(
            class_key(&world, 2, true, None, None, None),
            (None, Some(surge_refused(Refusal::NoSurgeYet)))
        );
        world.award(2, world::class::LEVEL_XP[2], &mut events);
        assert_eq!(
            class_key(&world, 2, true, None, None, None),
            (None, Some(surge_refused(Refusal::NotLinked)))
        );
        assert_eq!(
            class_key(&world, 2, false, None, None, None),
            (None, Some(beam_refused(Refusal::NoPatient))),
            "E on nobody with no beam on says so"
        );
        assert_eq!(
            class_key(&world, 2, false, None, Some(2), None),
            (None, Some(beam_refused(Refusal::NotACrewmate))),
            "never itself"
        );
        // Crew member 0 beside it, and beamed.
        let at = world.aboard.room.bim_pos(2) + bims::math::vec2(t, 0.0);
        world.aboard.room.put_for_probe(0, at);
        world.step(&[]);
        assert_eq!(
            class_key(&world, 2, false, None, Some(0), None),
            (Some(Order::Beam(Some(0))), None)
        );
        world.step(&[world::Command::Beam {
            slot: 2,
            patient: Some(0),
        }]);
        assert!(world.is_beaming(2));
        assert_eq!(
            class_key(&world, 2, false, None, Some(0), None),
            (Some(Order::Beam(None)), None),
            "E on the one it holds unlinks"
        );
        assert_eq!(
            class_key(&world, 2, false, None, None, None),
            (Some(Order::Beam(None)), None),
            "and E on nobody unlinks while one is held"
        );
        // Linked but not charged: Q says so.
        assert_eq!(
            class_key(&world, 2, true, None, None, None),
            (None, Some(surge_refused(Refusal::NotCharged)))
        );
        // And the engineer's keys are never a soldier's, nor the other
        // way about: an engineer pressing E with a tile is a deploy, not
        // a brace, and a soldier's E with a kit in its pack is a brace.
        assert!(matches!(
            class_key(&world, 0, false, Some(beside), None, None).0,
            Some(Order::Deploy { .. })
        ));
        assert!(matches!(
            class_key(&world, 1, false, Some(beside), None, None).0,
            Some(Order::Brace(_))
        ));
    }

    /// And the tank's (feature 77): Q taunts from the third level, E
    /// puts the wall up and takes it down — neither wants a tile or a
    /// crew member under the pointer, and neither is anybody else's.
    #[test]
    fn the_tank_s_keys_taunt_and_raise_the_wall() {
        let mut world = simulation_world(flyer(4), REFERENCE_MONEY, 4);
        assert_eq!(world.set_class(0, world::Class::Tank), Ok(()));
        assert_eq!(world.set_class(1, world::Class::Engineer), Ok(()));
        assert_eq!(world.set_class(2, world::Class::Soldier), Ok(()));
        assert_eq!(world.set_class(3, world::Class::Medic), Ok(()));
        let t = shipdesign::TILE as f32;
        let tile_of = |world: &world::World, who: u32| {
            let p = world.aboard.room.bim_pos(who as usize);
            ((p.x / t).floor() as i32, (p.y / t).floor() as i32)
        };
        // E puts the wall up wherever the pointer is, and down again.
        assert_eq!(
            class_key(&world, 0, false, None, None, None),
            (Some(Order::Bulwark(true)), None)
        );
        world.step(&[world::Command::Bulwark { slot: 0, on: true }]);
        assert_eq!(
            class_key(&world, 0, false, Some(tile_of(&world, 0)), Some(1), None),
            (Some(Order::Bulwark(false)), None),
            "and down again, whatever is under the pointer"
        );
        // Q wants the third level, and then cools down.
        assert_eq!(
            class_key(&world, 0, true, None, None, None),
            (None, Some(taunt_refused(Refusal::NoTauntYet)))
        );
        let mut events = Vec::new();
        world.award(0, world::class::LEVEL_XP[2], &mut events);
        assert_eq!(
            class_key(&world, 0, true, None, None, None),
            (Some(Order::Taunt), None)
        );
        world.step(&[world::Command::Taunt { slot: 0 }]);
        assert_eq!(
            class_key(&world, 0, true, None, None, None),
            (None, Some(taunt_refused(Refusal::CoolingDown)))
        );
        // And nobody else's keys are the tank's: the engineer deploys,
        // the soldier braces, the medic beams.
        assert!(matches!(
            class_key(&world, 2, false, None, None, None).0,
            Some(Order::Brace(_))
        ));
        assert_eq!(
            class_key(&world, 3, false, None, None, None),
            (None, Some(beam_refused(Refusal::NoPatient)))
        );
        assert_eq!(
            class_key(&world, 1, true, None, None, None),
            (None, Some(deploy_refused(Refusal::CantDeployThere)))
        );
        // And a tank is refused a medic's and a soldier's rules.
        assert_eq!(world.can_bulwark(1), Err(Refusal::NotATank));
        assert_eq!(world.can_taunt(3), Err(Refusal::NotATank));
    }

    /// The two every player has (feature 84): F and T are nobody's class
    /// in particular — a classless crew member gives them as readily as a
    /// commander — and the line they refuse with is the standing order's,
    /// not a class's.
    #[test]
    fn the_attack_and_retreat_keys_are_every_player_s() {
        let mut world = simulation_world(flyer(2), REFERENCE_MONEY, 2);
        assert_eq!(world.set_class(1, world::Class::Tank), Ok(()));
        world.step(&[]);
        // A retreat, with no class at all.
        assert_eq!(world.class_of(0), world::Class::None);
        assert_eq!(
            orders_key(&world, 0, world::Standing::Retreat),
            (Some(Order::Orders(world::Standing::Retreat)), None)
        );
        // And with one: a class changes nothing about these two.
        assert_eq!(
            orders_key(&world, 1, world::Standing::Retreat),
            (Some(Order::Orders(world::Standing::Retreat)), None)
        );
        // A banner on the tile the crew member stands on goes; one off
        // the deck altogether is refused, and the line is the order's.
        let t = shipdesign::TILE as f32;
        let p = world.aboard.room.bim_pos(0);
        let here = ((p.x / t).floor() as i32, (p.y / t).floor() as i32);
        assert_eq!(
            orders_key(&world, 0, world::Standing::Attack { tile: here }),
            (
                Some(Order::Orders(world::Standing::Attack { tile: here })),
                None
            )
        );
        // The world is what refuses the ground, when the order lands.
        let events = world.step(&[world::Command::Orders {
            slot: 0,
            order: world::Standing::Attack {
                tile: (-9_000, -9_000),
            },
        }]);
        assert!(
            events.iter().any(
                |e| matches!(e, world::WorldEvent::Refused { why, .. } if *why == Refusal::NoGroundThere)
            ),
            "no ground there: {events:?}"
        );
        // Unfit to act, the key itself says so before anything is sent.
        world.aboard.room.knock_out_for_probe(0);
        world.step(&[]);
        assert_eq!(
            orders_key(&world, 0, world::Standing::Retreat),
            (None, Some(orders_refused(Refusal::OutOfReach)))
        );
    }

    /// The commander's four (feature 78): Q rallies, E sends the squad
    /// at the enemy under the pointer, X calls it back and Z has it
    /// hold — and X and Z do nothing for any other class or for a
    /// classless crew member, which is the key handler's own guard.
    #[test]
    fn the_commanders_keys_are_the_squads_and_nobody_elses() {
        let mut world = simulation_world(flyer(3), REFERENCE_MONEY, 3);
        assert_eq!(world.set_class(0, world::Class::Commander), Ok(()));
        assert_eq!(world.set_class(1, world::Class::Tank), Ok(()));
        world.step(&[]);
        // Q wants the third level.
        assert_eq!(
            class_key(&world, 0, true, None, None, None),
            (None, Some(rally_refused(Refusal::NoRallyYet)))
        );
        // E with no enemy under the pointer says so.
        assert_eq!(
            class_key(&world, 0, false, None, None, None),
            (None, Some(squad_refused(Refusal::NoEnemyThere)))
        );
        // E over an enemy is an attack the world would take.
        assert_eq!(
            class_key(&world, 0, false, None, None, Some(2)),
            (
                Some(Order::Squad(world::SquadAsk::Attack { enemy: 2 })),
                None
            )
        );
        // X and Z go straight through, tile or none.
        assert_eq!(
            squad_key(&world, 0, world::SquadAsk::FallBack { tile: None }),
            (
                Some(Order::Squad(world::SquadAsk::FallBack { tile: None })),
                None
            )
        );
        assert_eq!(
            squad_key(&world, 0, world::SquadAsk::StandGround),
            (Some(Order::Squad(world::SquadAsk::StandGround)), None)
        );
        // And for anybody else they are refused by the same rule the
        // key handler skips them with.
        for slot in [1, 2] {
            assert_eq!(
                squad_key(&world, slot, world::SquadAsk::StandGround),
                (None, Some(squad_refused(Refusal::NotACommander)))
            );
            assert_ne!(world.class_of(slot), world::Class::Commander);
        }
        // A classless crew member's Q and E do nothing at all.
        assert_eq!(class_key(&world, 2, true, None, None, None), (None, None));
        assert_eq!(
            class_key(&world, 2, false, None, None, Some(1)),
            (None, None)
        );
    }

    /// The two boxes at the foot of the screen (feature 80): the
    /// class's own keys named, the key each is bound to, how many are
    /// left, and the level the locked one wants — all read off the world
    /// rather than kept, and the same pairing `class_key` dispatches by.
    #[test]
    fn the_two_boxes_say_what_the_keys_do_and_how_many_are_left() {
        let keys = Keys::default();
        let mut world = simulation_world(flyer(3), REFERENCE_MONEY, 3);
        assert_eq!(world.set_class(0, world::Class::Engineer), Ok(()));
        assert_eq!(world.set_class(1, world::Class::Soldier), Ok(()));
        // A classless crew member has no keys, so it has no boxes.
        assert!(ability_boxes(&world, 2, &keys).is_empty());

        // The engineer: the sentry waits for its level with the kit its
        // class dealt it in the pack; the sandbags are the ones it set
        // out with, and Q and E are what the bindings say.
        let boxes = ability_boxes(&world, 0, &keys);
        assert_eq!(boxes.len(), 2);
        assert_eq!((boxes[0].key.as_str(), boxes[1].key.as_str()), ("Q", "E"));
        assert_eq!(boxes[0].name, "Sentry");
        assert_eq!(boxes[0].locked, Some(world::class::SENTRY_LEVEL));
        assert_eq!(
            boxes[0].count,
            Some(world::deploy::SENTRY_CHARGES),
            "the sentry kit it set out with"
        );
        assert!(!boxes[0].ready(), "the level, not the kit");
        assert_eq!(boxes[1].name, "Sandbags");
        assert_eq!(boxes[1].locked, None);
        assert_eq!(
            boxes[1].count,
            Some(world::deploy::SANDBAG_CHARGES),
            "the kits it set out with"
        );
        assert!(boxes[1].ready());

        // The soldier: the grenade waits for its level with two in the
        // pack; the brace is there from the first and says when it is on.
        let boxes = ability_boxes(&world, 1, &keys);
        assert_eq!(boxes[0].name, "Grenade");
        assert_eq!(boxes[0].count, Some(world::class::GRENADE_CHARGES));
        assert_eq!(boxes[0].locked, Some(world::class::GRENADE_LEVEL));
        assert_eq!(boxes[1].name, "Brace");
        assert!(boxes[1].ready() && !boxes[1].on);
        world.step(&[world::Command::Brace { slot: 1, on: true }]);
        assert!(ability_boxes(&world, 1, &keys)[1].on, "braced now");

        // The level opens the locked one, and the charges thrown put it
        // out — which is the box saying no, not the level (feature 90:
        // the seconds show only with none in the pack).
        let mut events = Vec::new();
        world.award(1, world::class::LEVEL_XP[2], &mut events);
        let boxes = ability_boxes(&world, 1, &keys);
        assert_eq!(boxes[0].locked, None);
        assert!(boxes[0].ready());
        assert_eq!(boxes[0].cooldown, 0.0, "a charge in hand says no seconds");

        // Every class has a name and a tip on every box, and the
        // primary one is the level-three key for all of them. A class
        // with keys of its own past Q and E has a box for each of
        // them (feature 86): the commander's two other squad orders,
        // and the medic's carry.
        for class in world::Class::ALL {
            if class == world::Class::None {
                continue;
            }
            let mut world = simulation_world(flyer(1), REFERENCE_MONEY, 1);
            assert_eq!(world.set_class(0, class), Ok(()));
            let boxes = ability_boxes(&world, 0, &keys);
            let wanted = match class {
                world::Class::Commander => 4,
                world::Class::Medic => 3,
                _ => 2,
            };
            assert_eq!(boxes.len(), wanted, "{class:?}");
            assert!(
                boxes
                    .iter()
                    .all(|b| !b.name.is_empty() && !b.tip.is_empty())
            );
            assert_eq!(boxes[0].locked, Some(3), "{class:?}'s Q is its third");
            assert_eq!(boxes[1].locked, None, "{class:?}'s E is its first");
            // And the keys are the Controls page's own, in the order
            // the bar lays them out.
            let named: Vec<&str> = boxes.iter().map(|b| b.name).collect();
            if class == world::Class::Commander {
                assert_eq!(
                    named,
                    vec!["Rally", "Squad", names::FALL_BACK, names::STAND_GROUND]
                );
                assert_eq!(boxes[2].key, "X");
                assert_eq!(boxes[3].key, "Z");
                // All four say how many of the squad they reach.
                assert!(boxes[1..].iter().all(|b| b.count.is_some()));
            }
            if class == world::Class::Medic {
                assert_eq!(named, vec!["Surge", "Heal beam", names::CARRY]);
                assert_eq!(boxes[2].key, "G");
                assert_eq!(boxes[2].locked, None, "the carry wants no level");
            }
        }
    }

    /// Resting on a box says whom the cast would reach (feature 86):
    /// a commander's rally the crew in his aura, each of his three
    /// squad keys the squad, and nobody else's anybody at all.
    #[test]
    fn a_box_says_which_bims_its_cast_reaches() {
        let mut world = simulation_world(flyer(3), REFERENCE_MONEY, 1);
        assert_eq!(world.set_class(0, world::Class::Commander), Ok(()));
        // The squad is every crew member nobody steers within his
        // range, and all three of his squad keys reach exactly it —
        // which is the point of the ring: the three are one order under
        // three names.
        world.step(&[]);
        let squad = world.squad_members(0);
        assert!(squad.iter().all(|&w| w != 0), "never a steered Bim");
        for action in [
            Action::ClassSecondary,
            Action::SquadFallBack,
            Action::SquadStandGround,
        ] {
            assert_eq!(affected_by(&world, 0, action), squad, "{action:?}");
        }
        // The rally reaches whoever stands in the aura, the commander
        // among them — the whole of a small room, which is what a test
        // room is.
        let lifted = affected_by(&world, 0, Action::ClassPrimary);
        assert!(lifted.contains(&0), "the aura covers him: {lifted:?}");
        // An engineer's keys reach nobody: they are laid on the deck.
        let mut world = simulation_world(flyer(3), REFERENCE_MONEY, 1);
        assert_eq!(world.set_class(0, world::Class::Engineer), Ok(()));
        assert!(affected_by(&world, 0, Action::ClassPrimary).is_empty());
        assert!(affected_by(&world, 0, Action::ClassSecondary).is_empty());
    }
}

#[cfg(test)]
mod attack_key_tests {
    use super::*;

    /// The Attack key arms the pointer with nothing down, thinks better
    /// of it while armed, and — the half that was missing — **takes the
    /// banner up** when one is already down, which is the only press
    /// there is that lets the crew go back to their errands.
    #[test]
    fn the_attack_key_arms_the_pointer_and_takes_a_banner_back_up() {
        use world::Standing;
        // Nothing down: armed, so the click after it picks the ground.
        assert_eq!(attack_key(Standing::Follow, false, false), (None, true));
        // Armed already: thought better of, and nothing given.
        assert_eq!(attack_key(Standing::Follow, true, false), (None, false));
        // The map is up, which has no deck to put a banner on.
        assert_eq!(attack_key(Standing::Follow, false, true), (None, false));
        // A banner down: the same order back, which the world reads as
        // a release — and the pointer is left at rest rather than armed
        // for a second banner nobody could take up.
        let order = Standing::Attack { tile: (7, 9) };
        assert_eq!(attack_key(order, false, false), (Some(order), false));
        // Armed over a banner, it is still only thought better of: the
        // click is already on its way to a fresh tile.
        assert_eq!(attack_key(order, true, false), (None, false));
        // A retreat is the Retreat key's to call off, not this one's.
        assert_eq!(attack_key(Standing::Retreat, false, false), (None, true));
    }
}
