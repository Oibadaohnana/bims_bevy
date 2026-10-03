//! The game: one world, one clock, and the screen turning the crank on it.
//!
//! The run is under way by the time any of this runs: the ship is docked
//! or landed at the site the crew are at, and the pool is in the crew's
//! hands. The crew are the room laid out on the ship, so the crew's panels
//! are `crew.rs`, and what this screen adds is the coordinates: a window
//! point read back through the ship's camera and heading into the room's
//! own units.
//!
//! The world advances in fixed steps and never in stretched ones, and
//! since task 119 at one speed: 1×, sixty steps a second, or paused.

use bevy::prelude::*;
use bevy_egui::{EguiContexts, EguiPrimaryContextPass, egui};
use bims::order::{CrewOrder, angle_code};
use bims::room::HIT_BIM;
use ship::Session;
use ship::game::ViewMode;

use wire::{PeerId, To};
use world::{Refusal, ShipState, Speed, WorldEvent};

use super::designer::{Net, Order, ShipSession};
use super::hud::{self, GAP, MARGIN};
use super::loading::{Apply, Loading};
use crate::ability_icons::{self, Glyph};
use crate::canvas::{
    Pointer, canvas_painter, edge_pan_now, egui_rect, rect_of, root_ui, zoom_factor,
};
use crate::crew::{CLICK_SLOP, CrewPanels, Near, Open, TrayAsk, TrayView};
use crate::keys::{Action, Keys};
use crate::names::*;
use crate::net::{CHECK_EVERY, Choice, Event, Online, PING_SECONDS, Packet, Spot};
use crate::save::{Beginning, Request};
use crate::scene::WorldCanvas;
use crate::settings::{Allowed, Sheet, settings_sheet};
use crate::shapes::View;
use crate::sound::{Bed, Sounds};
use crate::theme::{panel_frame, tray_frame};
use crate::{Launch, Screen, theme};

/// Ceiling on world steps per frame. The world runs at 1× or not at all
/// (task 119), so a frame wants one or two; this is how far a long frame —
/// a load, a hitch — may catch up before the backlog is given up.
const MAX_STEPS_PER_FRAME: u32 = 128;

/// How long the pointer has to rest on the deck before the small readout
/// of what is under it comes up beside it (feature 107), in seconds:
/// egui's own tooltip delay and a little more, so crossing the deck on
/// the way somewhere says nothing.
const HOVER_REST: f64 = 0.4;

/// How long the window stays black once a trip is over, in seconds, and
/// how much of that is the fade back in: the place the crew arrived at
/// being laid out — the station or the settlement, the rooms joined — and
/// a beat on it.
const BLACKOUT_HOLD: f32 = 1.4;
const BLACKOUT_FADE: f32 = 0.6;

/// How long the `droids` probes wait between waves, in minutes of the
/// mission clock (feature 103): a minute, a second at 1x, where the
/// game's own is `data::DROID_REINFORCE_STEPS` (fifteen). The shortcut feature 83
/// asks for, so a wave landing can be watched rather than waited for.
const DROID_REINFORCE_IN_PROBE: f64 = 1.0;

/// How many waves of machines the `droids` probes give a held station,
/// the one aboard counted — where the game's own is the tier's
/// (`droid::wave_count`: one at tier one, two at two, four at three).
/// **Three**, because these commands exist to look at what a wave
/// *after* the first does: with one, it landed and the station was
/// cleared. `BIMS_DROID_WAVES=n` says otherwise.
const DROID_WAVES_IN_PROBE: u32 = 3;

/// How long the `defense` command waits between the crew setting down at
/// a threatened town and the first wave, in minutes of the mission clock
/// (features 94 and 103): a minute, where the game's own is
/// `data::DEFENSE_DELAY_STEPS` (twenty, task 111) — the same shortcut as the
/// machines' reinforcement clock, and for the same reason.
/// `BIMS_DEFENSE_DELAY=n` says otherwise.
const DEFENSE_DELAY_IN_PROBE: f64 = 1.0;

/// How far in the galaxy chart has to be (`Preview::zoom_level`, nought at
/// the fit and one at the closest) before every star's tier is written
/// under it; short of that only the stars near the ship's are.
const CHART_TIERS_ZOOM: f32 = 0.45;
/// The tier's numeral under a star on the chart: its size, and how far
/// below the star's middle it hangs, in points.
const CHART_TIER_TEXT: f32 = 10.0;
const CHART_TIER_DROP: f32 = 7.0;

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

/// A player's colour: the one they picked for their Bim's ring (feature
/// 84), off the session's list by slot, else the slot's place in the
/// list of colours. Their pointer, their pings and what they have their
/// eye on are all drawn in it.
fn slot_colour(tints: &[bims::character::Tint], slot: u32) -> egui::Color32 {
    let tint = tints
        .get(slot as usize)
        .copied()
        .unwrap_or_else(|| bims::character::Tint::from_code(slot as u8));
    let (red, green, blue) = tint.rgb();
    egui::Color32::from_rgb(
        (red * 255.0) as u8,
        (green * 255.0) as u8,
        (blue * 255.0) as u8,
    )
}

#[derive(Resource)]
pub struct GameScreen {
    net: Net,
    /// The crew's panels: `None` until the world opens, because everything
    /// they read is the room aboard, which does not exist before there is
    /// a world.
    panels: Option<CrewPanels>,
    /// The room tile a soldier is aiming a grenade at while the Q key is
    /// held (feature 75), for the burst's ring on the deck.
    throw_aim: Option<(i32, i32)>,
    /// The **attack** key has armed the pointer (feature 84): the system
    /// cursor is off, a red crosshair is drawn in its place, and the next
    /// left click on the deck puts the banner down there. Esc, a
    /// right-click or the key again puts it away.
    aiming_attack: bool,
    /// The **attack-move** key has armed the pointer: the same red
    /// crosshair, and the next left click on the deck sends the player's
    /// own Bim there under arms, shooting what it meets on the way.
    aiming_move: bool,
    /// The throw key (a soldier's Q, a grenade, or E, a Stun Shot, or an
    /// engineer's E, a satchel charge) has armed the pointer: the throw's reach is drawn
    /// round the player's own Bim, and the next left click on the deck
    /// throws there — walking out to it first where it must
    /// (`Order::ThrowAt`) — and so does letting the key go: a quick
    /// throw. Esc or a right-click puts it away, and so does letting the
    /// key go with the pointer off the deck. A Stun Shot charges at the
    /// tile rather than walking out to it.
    aiming_throw: Option<Throw>,
    /// The reach of a key held that aims at somebody rather than at a
    /// tile — a medic's beam (E) or his circle (R) — in tiles, and the
    /// ability, drawn round the player's own Bim while the key is down.
    held_reach: Option<(f32, Glyph)>,
    /// The downed crewmate the held revive key (G) sent the player's own
    /// Bim to get up: let go of when the key comes up before they are.
    held_revive: Option<u32>,
    /// The downed crewmate the menu's Carry row was for, and the frames
    /// since the walk over was sent: the carry goes the frame the player's
    /// own Bim is within reach, and is given up once it stops short.
    carry_walk: Option<(u32, u32)>,
    /// The keys, the pointer and the fire button as last sent to the
    /// player's own Bim (task 144), and when: a `CrewOrder::Control`
    /// goes whenever the walk or the trigger changes, and for the aim
    /// alone no more than [`CONTROL_EVERY`] and only past [`AIM_STEP`].
    control: Option<(CrewOrderControl, f64)>,
    /// The follow key has let the camera go (task 144): until it does,
    /// the ship view follows the player's own Bim from the first frame
    /// of a run, and of a world a resync stood up.
    free_camera: bool,
    /// The pointer is the crosshair this frame (task 144): the system's
    /// cursor hidden (`hide_the_cursor`) and the reticle drawn over
    /// everything — over the deck, unless a window wants the pointer: the
    /// trader, a relic to choose, the Esc sheet, the map, the Armory and
    /// the character sheet.
    crosshair: bool,
    /// The left press was an armed pointer's — a throw, an attack-move, the
    /// banner — and fires nothing until the button comes up (task 144).
    trigger_spent: bool,
    /// The dodge roll's key (Alt, task 150) was down last frame: a roll
    /// goes on its way down, once.
    dodge_was: bool,
    /// Tab went down last frame with the keys ours: the focus egui gave a
    /// widget for it is to be surrendered (`keys::release_tab_focus`).
    tab_took_focus: bool,
    backlog: f64,
    /// What just happened, over *Back to ship* (feature 107): four lines
    /// at most, each fading a few seconds after it came.
    log: hud::Log,
    /// Where the pointer is over the canvas, for the readout.
    hover_at: Option<Vec2>,
    /// Where the pointer came to rest on the deck and when, on the
    /// window's clock: the readout at the pointer waits for it to rest.
    rest: Option<(Vec2, f64)>,
    /// The player's own experience last frame, so what came in since can
    /// go into the log (feature 107): the world keeps no event of it.
    last_xp: Option<u32>,
    /// A marquee under way on the deck: where the press landed.
    marquee_from: Option<Vec2>,
    pan_from: Option<Vec2>,
    /// Whether the middle drag began on the galaxy chart rather than the
    /// deck.
    pan_galaxy: bool,
    /// A left press on the galaxy chart (the map rework): where it landed, and
    /// whether it has moved far enough to be a drag of the chart rather
    /// than a click on a star, which is said at the release.
    chart_press: Option<(Vec2, bool)>,
    /// The Esc sheet, if it is up, and which page.
    sheet: Option<Sheet>,
    /// The sheet's save and load pages' state — `crate::save`.
    saves: crate::save::Saves,
    size: Vec2,
    /// The smooth fog over the deck, as a texture — see `fogmap`.
    fog: crate::fogmap::FogTexture,
    /// The same over the plain beyond the box, on a planet: a texture a
    /// chunk of the room, kept while the room composes the chunk.
    plain_fog: std::collections::BTreeMap<(i32, i32), crate::fogmap::FogTexture>,
    /// The galaxy chart, on the left of the world map (task 135): the
    /// lobby's, made the first time the map is up — it generates every
    /// system once — and kept for the game.
    galaxy: Option<lobby::Lobby>,
    galaxy_list: lobby::draw::DrawList,
    galaxy_size: Vec2,
    /// Every star whose system has a trader (`World::trader_stars`),
    /// worked out when the chart is made: it generates the systems, and
    /// it is a function of the galaxy and the crew's home alone.
    chart_traders: Vec<u32>,
    /// The star picked on the chart: the crisis's word on it is in the
    /// strip, and the route to it is drawn.
    picked_star: Option<u32>,
    /// The slots whose players have left the game: said once each; their
    /// crew members carry on unsteered.
    gone: Vec<u32>,
    /// What this player last told the room its own Bim is called
    /// (`Online::say_bim_name`, task 145): said again on the deck at the
    /// open and whenever somebody comes or goes, not only in the lobby,
    /// so a name the host missed before its Start still lands on every
    /// machine (`Online::names_said`). `None` until said.
    said_name: Option<String>,
    /// A guest whose checksum has parted from the host's and has asked
    /// for its world (`Packet::Resync`, feature 67): said once, and not
    /// asked again until the world arrives. The wrong world keeps
    /// stepping meanwhile — stopping would make it wronger under the
    /// pointer.
    resyncing: bool,
    /// A guest's playout buffer (task 148): the host's steps, orders and
    /// world, held in their order and played out at the world's pace so
    /// a shaky line does not stop and lurch the world. Idle on the clock.
    playout: crate::playout::Playout,
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
    /// Seconds of black left over the canvas: a trip ends in it — the
    /// place arrived at is being laid out — and it lifts once it is there.
    /// Nought nearly always.
    blackout: f32,
    /// The green numbers a heal beam puts up over the body it holds
    /// (feature 91). Kept by the screen and nowhere else: what a beam put
    /// back is the body's own count going up, and neither the room nor
    /// the world records it as an event to be read.
    heals: Heals,
    /// What the enemies down lately paid, floating over where they fell.
    rewards: Vec<Reward>,
    /// The numbers of the hits landed lately, over whoever took them.
    hits: Vec<HitNumber>,
    /// The health bar over every body on the deck and what each lost or
    /// got back a moment ago (task 137, `crate::healthbars`).
    bars: crate::healthbars::HealthBars,
    /// The world map's list of destinations and the one picked on it
    /// (feature 103, `super::worldmap`): worked out when the world it
    /// reads has moved, not every frame.
    world_map: super::worldmap::WorldMap,
    /// What this mission has earned, for the end of a fight
    /// (`super::fightwon`): the screen that comes up when the site is
    /// cleared.
    fight: super::fightwon::FightTally,
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
    /// The hit points: everything a beam puts back (task 120).
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

/// An enemy down and what it paid, rising off where it fell: which body
/// of the station's room, the money and the experience, when it went up,
/// and where it was last on the screen — kept, so the numbers finish
/// where the body lay if it is lost from sight.
#[derive(Clone, Copy)]
struct Reward {
    who: u32,
    money: u64,
    xp: u32,
    born: f64,
    at: Option<(f32, f32)>,
}

/// A hit's number in the air (`WorldEvent::Hit`): on one of the station's
/// bodies or one of the crew, how much, whether critical, when, where it
/// was last on the screen, and a nudge sideways so a burst's numbers do
/// not stand on one another.
#[derive(Clone, Copy)]
struct HitNumber {
    resident: bool,
    who: u32,
    damage: u32,
    crit: bool,
    born: f64,
    at: Option<(f32, f32)>,
    nudge: f32,
}

/// How long a hit's number is in the air, and the most in the air at
/// once: a minigun lands a dozen a second, and past that they are noise.
const HIT_SECONDS: f64 = 0.7;
const HITS_SHOWN: usize = 48;

/// How long an enemy's pay is in the air: short, since a fight downs
/// many, and a deck of old numbers would hide the next.
const REWARD_SECONDS: f64 = 0.9;

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
    for who in beamed {
        // A site's defender too, in its own room (`World::patient_bar`).
        let Some((points, _)) = world.patient_bar(who) else {
            continue;
        };
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
            .add_systems(EguiPrimaryContextPass, over.run_if(in_state(Screen::Over)))
            .add_systems(Update, hide_the_cursor);
    }
}

/// The system's cursor hidden while the pointer is the crosshair (task
/// 144, `GameScreen::crosshair`) and shown again everywhere else: another
/// screen, or a window on this one that wants the pointer. Through the
/// window's own `CursorOptions`, since bevy_egui turns `CursorIcon::None`
/// into the arrow.
fn hide_the_cursor(
    state: Res<State<Screen>>,
    screen: Option<Res<GameScreen>>,
    mut cursor: Single<&mut bevy::window::CursorOptions, With<bevy::window::PrimaryWindow>>,
) {
    let hide = *state.get() == Screen::Game && screen.is_some_and(|s| s.crosshair);
    if cursor.visible == hide {
        cursor.visible = !hide;
    }
}

/// The end of the run: nobody of the crew standing — `World::lost`, said
/// by `WorldEvent::CrewLost` — and the game screen hands over to this,
/// a screen that says so and a way back to the menu. The world is left
/// as it was, so a save made before the fight is still there to load.
#[allow(clippy::too_many_arguments)]
fn over(
    mut contexts: EguiContexts,
    mut session: ResMut<ShipSession>,
    mut next: ResMut<NextState<Screen>>,
    window: Single<&Window>,
    online: Res<Online>,
    beginning: Option<Res<Beginning>>,
    mut commands: Commands,
    victory: Option<Res<crate::profile::Victory>>,
) -> Result {
    let ctx = contexts.ctx_mut()?.clone();
    let mut root = root_ui(&ctx);
    // A run **won** (feature 106, `World::run_won`) ends here too, and is
    // written into this machine's profile the first frame it is shown:
    // every player's own, on their own disk.
    let won = session.0.game.as_ref().is_some_and(|g| g.world.is_won());
    if won && victory.is_none() {
        crate::profile::record_win();
        commands.insert_resource(crate::profile::Victory);
    }
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
            if won {
                ui.label(egui::RichText::new(VICTORY_TITLE).size(28.0).strong());
                ui.label(egui::RichText::new(when).color(theme::MUTED));
                // The run in numbers (feature 108), and the crew's relics.
                if let Some(game) = session.0.game.as_ref() {
                    let summary = game.world.run_summary();
                    ui.add_space(8.0);
                    for line in victory_summary(&summary) {
                        ui.label(line);
                    }
                    ui.add_space(6.0);
                    ui.label(egui::RichText::new(VICTORY_RELICS).strong());
                    ui.label(egui::RichText::new(victory_relics(&summary.relics)).small());
                }
            } else {
                ui.label(egui::RichText::new(OVER_TITLE).size(28.0).strong());
                ui.label(egui::RichText::new(OVER_LINE).color(theme::MUTED));
                ui.label(egui::RichText::new(when).color(theme::MUTED));
            }
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
        commands.remove_resource::<crate::profile::Victory>();
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
    // A run opening has won nothing yet (feature 106).
    commands.remove_resource::<crate::profile::Victory>();
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
                    let seed = super::rand_seed();
                    let roll = super::rand_seed();
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
            // The `test` command is on the combat ship, with one crew
            // member and a mercenary for hire at the dock whatever the
            // roll said, so a hire can be looked at.
            // The `test_planet` command is that landed: the ship set down
            // at the settlement of the system's first planet with ground —
            // the mercenary asked for first, since the ask holds for every
            // friendly room opened after it, the settlement's included.
            // `tier2_test` and `tier3_test` are `droids` with everybody's
            // guns and armour at that tier, and the machines at it too
            // (`Session::droids_at_tier`): every enemy is a machine since
            // feature 102, so the human garrison they used to dress is
            // gone, and with it the `combat`, `combat_<class>` and `raid`
            // commands.
            let mut session = match *launch {
                Launch::DroidsAtTier(tier) => Session::droids_at_tier(
                    seed,
                    tier,
                    crate::dev::droid_reinforce(DROID_REINFORCE_IN_PROBE),
                    crate::dev::droid_wave_max(),
                    crate::dev::droid_waves(DROID_WAVES_IN_PROBE),
                    size.x,
                    size.y,
                ),
                // The `droids` command is the fight (feature 83): the
                // combat ship, its crew and its guns, and a wave of droids
                // about the arena. `BIMS_DROID_TIER` is what tier they
                // come at and `BIMS_DROID_WAVE` how big a wave may be, for
                // the measurements. `combat_droids_<class>` is that same
                // wave with the class below in hand: the ship, the arena,
                // the wave and both dials are `droids`', so two of those
                // runs differ by the class and nothing else (feature 79).
                // The Guardian looked at (feature 100): `droids` at tier
                // three with every wave one Guardian and two Troopers.
                Launch::Guardian => Session::guardian(
                    seed,
                    crate::dev::droid_reinforce(DROID_REINFORCE_IN_PROBE),
                    crate::dev::droid_waves(DROID_WAVES_IN_PROBE),
                    size.x,
                    size.y,
                ),
                // The Machine Heart (feature 108): the crew at its fortress
                // in tier-three kit, the waves the game's own unless
                // `BIMS_DROID_WAVES` says, and `BIMS_HEART_PHASE` the phase
                // the fight opens in.
                Launch::Heart => Session::heart(
                    seed,
                    crate::dev::droid_reinforce(DROID_REINFORCE_IN_PROBE),
                    crate::dev::droid_waves_dial(),
                    crate::dev::heart_phase(),
                    size.x,
                    size.y,
                ),
                // A site of the Manufacturers' (feature 109): the combat
                // crew there on `BIMS_MANUFACTURER_DAY`.
                Launch::Manufacturers => Session::manufacturers(
                    seed,
                    crate::dev::manufacturer_day(),
                    crate::dev::droid_wave_max(),
                    crate::dev::droid_reinforce(DROID_REINFORCE_IN_PROBE),
                    size.x,
                    size.y,
                ),
                // The relics looked at (feature 106): the arena with one
                // wave short enough to clear, and the reward screen after.
                Launch::Relics => Session::relics(
                    seed,
                    crate::dev::droid_reinforce(DROID_REINFORCE_IN_PROBE),
                    size.x,
                    size.y,
                ),
                Launch::Droids | Launch::DroidsAs(_) => {
                    let mut session = Session::droids(
                        seed,
                        crate::dev::droid_tier(),
                        crate::dev::droid_reinforce(DROID_REINFORCE_IN_PROBE),
                        crate::dev::droid_wave_max(),
                        crate::dev::droid_waves(DROID_WAVES_IN_PROBE),
                        size.x,
                        size.y,
                    );
                    // The fight in a generated station rather than the
                    // arena, when a seed is asked for (feature 112).
                    if let Some(station) = crate::dev::station_seed() {
                        session.regenerate_dock_for_probe(station, crate::dev::station_kind());
                    }
                    say_where(&session);
                    session
                }
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
                    // `BIMS_STATION_SEED` (feature 112): the dock, or the
                    // town set down at, drawn from that seed instead.
                    let station_seed = crate::dev::station_seed();
                    if let (Launch::Test, Some(station)) = (*launch, station_seed) {
                        session.regenerate_dock_for_probe(station, crate::dev::station_kind());
                    }
                    if let (
                        Launch::TestPlanet | Launch::DroidsPlanet | Launch::Defense,
                        Some(town),
                    ) = (*launch, station_seed)
                    {
                        session.reseed_ground_for_probe(town);
                    }
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
                        // `BIMS_NIGHT` (task 152): night or day at the town,
                        // said before the landing opens its room.
                        if let (Some(night), Some(game)) =
                            (crate::dev::night(), session.game.as_mut())
                        {
                            game.world.set_night_for_probe(Some(night));
                        }
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
                    say_where(&session);
                    session
                }
                _ => Session::simulate(seed, 0, spawn, size.x, size.y),
            };
            // `BIMS_DARK` (task 152): every enemy's station dark, or lit,
            // whatever the roll — the dock the command opens at too.
            if let (Some(dark), Some(game)) = (crate::dev::dark(), session.game.as_mut()) {
                game.world.set_dark_for_probe(Some(dark));
            }
            // Every run on the combat ship opens with everything there is
            // in the armory — every weapon and piece at every tier it is
            // made at — for trying any kit on aboard before stepping off.
            if !matches!(
                *launch,
                Launch::Game | Launch::Simulation | Launch::Design | Launch::StationBuilder
            ) {
                session.stock_the_armory_for_probe();
            }
            // The class onto slot 0 first — the command's own, or
            // `BIMS_CLASS` over it — before anything that leaves the
            // berth, since the class locks at the first undock.
            let asked = match *launch {
                Launch::DroidsAs(class) => class,
                _ => world::Class::None,
            };
            crate::dev::class_crew(&mut session, asked);
            // The run's open classes (feature 106): with nobody else in
            // it, this machine's own profile's, as the lobby's host's would be.
            let unlocks = crate::profile::RunUnlocks::of(&crate::profile::load());
            commands.insert_resource(unlocks);
            // The relics asked for, and the probes' win.
            crate::dev::relic_dials(&mut session);
            // And `BIMS_BEAM` after it: the class has to be on before a
            // medic can hold anybody (feature 76).
            crate::dev::beam_crew(&mut session);
            // `BIMS_AFIELD`: set down at the settlement and walked out onto
            // the plain.
            if crate::dev::afield() {
                session.land_for_probe();
                session.walk_afield_for_probe();
            }
            // The dead of a fight nobody watched, lying where they stood
            // (feature 85).
            if let Some(n) = crate::dev::graves() {
                session.lay_graves_for_probe(n);
            }
            // A townsperson down beside the player's Bim, to pick up.
            if crate::dev::down_resident() && !session.down_resident_for_probe() {
                eprintln!("BIMS_DOWN_RESIDENT: nobody of the station to down");
            }
            // A site's defenders in a row beside the player's Bim.
            if crate::dev::stand_defenders() && !session.stand_defenders_for_probe() {
                eprintln!("BIMS_DEFENDERS: no defenders at the site");
            }
            // Every state a machine can be drawn in, laid out on the
            // arena's deck for one picture (feature 83). The wave the
            // probe laid out is replaced by the showcase.
            if crate::dev::droid_showcase() {
                session.stage_droids_for_probe();
            }
            if crate::dev::lost() {
                session.lose_for_probe();
            }
            // At the nearest elite, its mission begun.
            if crate::dev::elite() && session.elite_for_probe().is_none() {
                eprintln!("BIMS_ELITE: no elite in reach");
            }
            // The run's loop (feature 103): between missions with the map
            // up, or the departure check asking.
            if crate::dev::map() {
                session.map_for_probe();
            }
            if crate::dev::depart() {
                session.depart_for_probe();
            }
            // And at a trader (task 114), the Trader panel up on the map.
            if crate::dev::trader() {
                match session.trader_for_probe() {
                    Some(site) => println!("trader: star {} station {}", site.star, site.station),
                    None => println!("trader: none in reach"),
                }
            }
            // And the relics' screen (feature 106): the reward after an
            // elite's site cleared.
            if crate::dev::reward() && !session.reward_for_probe() {
                eprintln!("BIMS_REWARD: no held site to clear, or nothing left to offer");
            }
            if crate::dev::fight_won() && !session.fight_won_for_probe() {
                eprintln!("BIMS_FIGHT_WON: no held site to clear");
            }
            // A fight staged at the dock, one machine and the steered Bim.
            if let Some(kind) = crate::dev::duel()
                && let Some(game) = session.game.as_mut()
            {
                let arm = kind.arm(0).at(bims::combat::Tier::One);
                game.world.stage_droid_fight_for_probe(kind, Some(arm));
            }
            if let Some(n) = crate::dev::lamps_out() {
                session.shoot_lamps_for_probe(n);
            }
            // And the downed: after the lamps, so a downed Bim can be
            // asked for on a deck already staged.
            if let Some(n) = crate::dev::dying() {
                session.maim_for_probe(n);
            }
            // The hired field medics (feature 86), and a body in one's
            // arms: after the downed, so a medic can be asked for on a
            // deck that already has somebody to fetch.
            if let Some(n) = crate::dev::field_medics() {
                session.field_medics_for_probe(n);
            }
            if crate::dev::carry() {
                session.carry_for_probe();
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
            // member's, or every resident's — a town's guard, say; and
            // armour asked for goes on the crew member, pieces with ids the
            // hold does not have.
            // `Game::issue` redraws the picture, so the swing or the
            // barrel is on screen from the first frame.
            let worn = crate::dev::armoured();
            if (worn.is_some() || crate::dev::weapon().is_some())
                && let Some(game) = session.game.as_mut()
            {
                use bims::combat::Piece;
                let room = &mut game.world.aboard.room;
                let gear = room.gear(0);
                // The armour at the tier `BIMS_ARMOURED` says, else what
                // is worn.
                let kind = bims::combat::ArmourKind::Armour;
                let armour = worn
                    .map(|tier| Piece::new(u32::MAX - kind.code(), kind, tier))
                    .or(gear.worn());
                room.issue(
                    0,
                    bims::combat::Gear {
                        weapon: crate::dev::weapon().or(gear.weapon),
                        armour,
                        ..gear
                    },
                );
                // And the rest of the crew too, `BIMS_WEAPON_ALL` saying so.
                if let Some(weapon) = crate::dev::weapon().filter(|_| crate::dev::weapon_all()) {
                    for who in 1..room.crew_count() as usize {
                        let gear = room.gear(who);
                        room.issue(
                            who,
                            bims::combat::Gear {
                                weapon: Some(weapon),
                                ..gear
                            },
                        );
                    }
                }
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
            // The ready check on a command's own run only when asked
            // (`BIMS_READY=1`): the `game` run has it from the lobby.
            if crate::dev::ready_check() == Some(true)
                && let Some(game) = session.game.as_mut()
            {
                game.world.set_ready_check(true);
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
/// it — a command of its own (`droids`, `combat_droids_medic`,
/// `test_planet`), the lobby's Start or the yard's last Accept. A restart stands a session up from it
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
    /// nothing picked, no panels yet, the canvas unmeasured so the first
    /// frame fits the world to it.
    fn fresh(slot: u32, players: u32) -> GameScreen {
        GameScreen {
            net: Net {
                slot,
                players,
                wire: None,
            },
            panels: None,
            throw_aim: None,
            aiming_attack: false,
            aiming_move: false,
            aiming_throw: None,
            held_reach: None,
            held_revive: None,
            carry_walk: None,
            control: None,
            free_camera: false,
            crosshair: false,
            trigger_spent: false,
            dodge_was: false,
            tab_took_focus: false,
            backlog: 0.0,
            log: hud::Log::default(),
            hover_at: None,
            rest: None,
            last_xp: None,
            marquee_from: None,
            pan_from: None,
            sheet: None,
            saves: crate::save::Saves::default(),
            size: Vec2::ZERO,
            fog: crate::fogmap::FogTexture::default(),
            plain_fog: std::collections::BTreeMap::new(),
            pan_galaxy: false,
            chart_press: None,
            galaxy: None,
            galaxy_list: lobby::draw::DrawList::new(),
            galaxy_size: Vec2::ZERO,
            chart_traders: Vec::new(),
            picked_star: None,
            gone: Vec::new(),
            said_name: None,
            resyncing: false,
            playout: crate::playout::Playout::default(),
            answered: Vec::new(),
            desync_at: crate::dev::desync_at(),
            freeze: crate::dev::freeze_at_shot(),
            blackout: 0.0,
            heals: Heals::default(),
            rewards: Vec::new(),
            hits: Vec::new(),
            bars: crate::healthbars::HealthBars::default(),
            world_map: super::worldmap::WorldMap::default(),
            fight: super::fightwon::FightTally::default(),
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
        // What is still queued behind the world that arrived is of that
        // world: it plays on as it would have.
        next.playout = std::mem::take(&mut self.playout);
        next.freeze = None;
        next
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
    // What the host's profile opened for the run (feature 106): the
    // classes the sheet offers. Absent on a command's own run, which
    // opens every class.
    unlocks: Option<Res<crate::profile::RunUnlocks>>,
    // Whether the window has the focus, for the edge scroll (task 123).
    window: Single<&Window>,
    // A mission being built off this thread (`screens::loading`).
    mut loading: ResMut<super::loading::Loading>,
    // The picture behind a station's deck.
    station_backdrops: Res<super::backdrop::StationBackdrops>,
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
    // A trip being built behind the loading screen, or one this end's own
    // order set off last frame (`screens::loading`): the frame is the
    // loading screen's, and the session here a stand-in.
    if loading.busy() || loading.begin_deferred(&screen.net, session) {
        return Ok(());
    }
    // The log's clock first: a line said this frame is stamped with it.
    screen.log.tick(now);

    // --- the wire ------------------------------------------------------------
    // What the others said since last frame, in order. On the host a
    // guest's ask is applied and goes round; on a guest the host's applied
    // orders and its steps are what the world is made of. The host gone
    // is the end of company: the clock is this window's from here.
    let wire_timed = crate::perf::scope(crate::perf::Phase::Wire);
    // What a trip left waiting goes first. A message that is the trip is
    // applied on a thread (`screens::loading`), and whatever came behind
    // it waits for it: the order everything is applied in holds.
    let mut drained = loading.take_stash();
    let fresh = online.drain(now);
    if screen.net.is_clock() {
        drained.extend(screen.playout.flush());
        drained.extend(fresh);
    } else {
        // A guest's timeline goes through the playout buffer (task 148):
        // the host's steps, orders and world in their order, the steps
        // at the world's pace. The host gone, all of it at once, before
        // the word that it went: the clock is this end's after that.
        let gone = fresh
            .iter()
            .any(|e| matches!(e, Event::Closed(_) | Event::Lost(_)));
        let mut rest = Vec::new();
        for event in fresh {
            if crate::playout::Playout::holds(&event) {
                screen.playout.push(event);
            } else {
                rest.push(event);
            }
        }
        if gone {
            drained.extend(screen.playout.flush());
        } else {
            let nominal = session.steps_per_second();
            let times = session
                .game
                .as_ref()
                .map_or(0, |g| g.world.effective_speed().multiplier());
            let cap = keys_now.net_buffer_seconds();
            drained.extend(
                screen
                    .playout
                    .release(dt, nominal * f64::from(times), nominal, cap),
            );
        }
        drained.extend(rest);
    }
    let mut drained = drained.into_iter();
    while let Some(event) = drained.next() {
        match event {
            Event::Packet { from, packet } => match packet {
                Packet::Ask { at, message } => {
                    if let Some(slot) = online.slot_of(from) {
                        if screen.net.wire.as_ref().is_some_and(|w| w.host)
                            && Loading::travels(session, slot, &message)
                        {
                            let apply = Apply::Asked {
                                from: slot,
                                at,
                                message,
                                peer: from,
                            };
                            loading.trip(&screen.net, session, apply);
                            loading.stash(drained.by_ref());
                            break;
                        }
                        screen.net.asked(session, slot, at, message, from);
                    }
                }
                Packet::Applied { from, at, message } => {
                    if !screen.net.is_clock() && Loading::travels(session, from, &message) {
                        loading.trip(&screen.net, session, Apply::Applied { from, at, message });
                        loading.stash(drained.by_ref());
                        break;
                    }
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
                        if crate::dev::auto().is_some() {
                            let p = &screen.playout;
                            println!(
                                "playout: target {:.3} gaps {} frames {:?}",
                                p.target(),
                                p.tally.gaps,
                                p.tally.frames
                            );
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
                        Ok(mut loaded) => {
                            let (slot, players) = (loaded.editor.local, loaded.editor.players);
                            // This player's own Bim keeps what its
                            // player called it, whatever the host's
                            // copy says (task 145).
                            let mine = session.crew_names.get(slot as usize).cloned();
                            if let Some(mine) = mine.filter(|n| !n.is_empty()) {
                                if loaded.crew_names.len() <= slot as usize {
                                    loaded.crew_names.resize(slot as usize + 1, String::new());
                                }
                                loaded.crew_names[slot as usize] = mine;
                            }
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
                // Whoever came hears this player's Bim's name again.
                screen.said_name = None;
                for slot in 0..screen.net.players {
                    let there = online
                        .slots
                        .get(slot as usize)
                        .is_some_and(|id| online.peers.iter().any(|p| p.id == *id));
                    if !there && !screen.gone.contains(&slot) {
                        screen.gone.push(slot);
                        screen.log.push(player_left(&crew_name(slot)));
                        // And the world told, by the host alone, so a
                        // vote or a departure does not wait on somebody
                        // who is not there (feature 103).
                        if screen.net.wire.as_ref().is_some_and(|w| w.host) {
                            loading.order(&screen.net, session, Order::PlayerGone(slot));
                        }
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

    if loading.busy() {
        return Ok(());
    }

    // A Bim's name said late — after the host's Start, or typed since —
    // lands on the crew here, and on every word about them.
    if online.names_said(&mut session.crew_names) {
        crate::names::set_crew_names(&session.crew_names);
    }
    // And this player's own said again (task 145): in the lobby it went
    // out only while the field changed, so a name the host had not heard
    // by its Start — or a peer's world dealt without it — stayed the
    // table's on the others' decks. Here it goes out at the open and
    // after every join, and `names_said` puts it on everybody's crew.
    if screen.net.wire.is_some() {
        let mine = session
            .crew_names
            .get(screen.net.slot as usize)
            .map(|n| wire::tidy_name(n))
            .unwrap_or_default();
        if screen.said_name.as_deref() != Some(mine.as_str()) {
            online.say_bim_name(&mine);
            screen.said_name = Some(mine);
        }
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
    // And the white and the light on the health bars (task 137), on the
    // same clock.
    let bars_dt = if running { dt as f32 } else { 0.0 };
    // What just happened. An event is a thing that happened once, so the
    // list is drained after it is read.
    if let Some(game) = &mut session.game {
        let mut arrived = false;
        // Whether a machine went down this frame: what experience that
        // came in with it was for (feature 107).
        let mut machines_down = false;
        // The fight's tally, started afresh with a new mission.
        screen.fight.follow(&game.world);
        for event in game.events.drain(..) {
            machines_down |= matches!(event, WorldEvent::DroidDown { .. });
            // Anyone's purchase at the trader rings the till in every
            // window: the event is the world's, so a guest's buy is
            // heard by the host and the other way round.
            if matches!(
                event,
                WorldEvent::ShelfBought { .. } | WorldEvent::ItemBought { .. }
            ) {
                sounds.bought(&mut commands);
            }
            // And a sale to the trader, the coins, heard the same way.
            if matches!(event, WorldEvent::Sold { .. }) {
                sounds.sold(&mut commands);
            }
            // A class's ability, whoever in the crew used it, heard the
            // same way.
            sounds.ability(&mut commands, event);
            // A player downed, this one or another: every window steps
            // the world and hears it, so every player knows to come and
            // pick them up. Not a bot of the crew.
            if let WorldEvent::CrewDowned { who } = event
                && !game.world.aboard.room.is_bot(who as usize)
            {
                sounds.downed(&mut commands, who);
            }
            // Somebody brought round, heard where a player had a hand in
            // it: a player up again, or a player's Bim the one reviving
            // (a crewmate or a townsperson). Two bots between themselves
            // are left to the picture, or a fight would be all chimes.
            let player = |w: u32| !game.world.aboard.room.is_bot(w as usize);
            match event {
                WorldEvent::CrewRevived { who, by } if player(who) || player(by) => {
                    sounds.revived(&mut commands, who);
                }
                WorldEvent::ResidentRevived { who, by, .. } if player(by) => {
                    sounds.revived(&mut commands, bims::game::GUEST as u32 + who);
                }
                _ => {}
            }
            // An enemy down: its pay floats up over it, with a soft chime.
            // A hit: its number over whoever took it.
            if let WorldEvent::Hit {
                resident,
                who,
                damage,
                crit,
            } = event
                && screen.hits.len() < HITS_SHOWN
            {
                let nudge = (screen.hits.len() % 5) as f32 - 2.0;
                screen.hits.push(HitNumber {
                    resident,
                    who,
                    damage,
                    crit,
                    born: now,
                    at: None,
                    nudge,
                });
            }
            if let WorldEvent::EnemyRewarded { who, xp, money, .. } = event {
                screen.rewards.push(Reward {
                    who,
                    money,
                    xp,
                    born: now,
                    at: None,
                });
                sounds.reward(&mut commands);
            }
            // One of the Manufacturers dead is said as one (feature 109):
            // they have no names the crew know.
            let theirs = match event {
                WorldEvent::EnemyDown { who, .. } => game
                    .world
                    .residents
                    .as_ref()
                    .is_some_and(|r| r.aboard.room.is_manufacturer(who as usize)),
                _ => false,
            };
            screen.fight.note(&event, theirs);
            if theirs {
                screen.log.push(crate::names::MANUFACTURER_DOWN.to_string());
            } else if let Some(line) = event_line(event) {
                screen.log.push(line);
            }
            if let Some(freeze) = screen.freeze.as_mut()
                && freeze.counts == crate::dev::Counted::Downs
                && matches!(event, WorldEvent::DroidDown { .. })
            {
                freeze.left = freeze.left.saturating_sub(1);
            }
            // Arrived (feature 103): a mission begins, and the ship view
            // is where it is played — the map put away, nothing picked,
            // and the window black a moment while the place is laid out.
            // Said on stdout in a scripted run, for the two-window pair
            // (`scratchpad/duo_resync.sh travel`).
            if crate::dev::auto().is_some() {
                match event {
                    WorldEvent::LeftSite { station, cleared } => {
                        println!("left: {station} {cleared}")
                    }
                    WorldEvent::Travelled {
                        star,
                        station,
                        minutes,
                    } => println!("travelled: {star} {station} {minutes}"),
                    WorldEvent::Proposed {
                        slot,
                        star,
                        station,
                    } => println!("proposed: {slot} {star} {station}"),
                    WorldEvent::ProposalAccepted { slot, yes } => {
                        println!("accepted: {slot} {yes}")
                    }
                    WorldEvent::PlayerGone { slot } => println!("gone: {slot}"),
                    WorldEvent::Refused { slot, why } => println!("refused: {slot} {why:?}"),
                    _ => {}
                }
            }
            if matches!(event, WorldEvent::Travelled { .. }) {
                arrived = true;
                screen.world_map.picked = None;
                screen.blackout = BLACKOUT_HOLD;
            }
        }
        // The experience the player's own Bim gained since last frame, a
        // line of the log gathered a second at a time by what it was for
        // (feature 107). The world says a level and not the points, so
        // the points are read off the count going up; a machine down the
        // same frame is what they were for, and anything else is said
        // as experience.
        let xp = game.world.progress_of(screen.net.slot).xp;
        if let Some(before) = screen.last_xp
            && xp > before
        {
            let source = if machines_down {
                XP_FROM_MACHINES
            } else {
                XP_FROM_WORK
            };
            screen.log.xp(source, xp - before);
        }
        screen.last_xp = Some(xp);
        if arrived {
            game.set_mode(ViewMode::Ship);
        }
        // Between missions the map is up for everybody, and stays up
        // (feature 103): it is the one thing there is to do.
        if !game.world.in_mission() && game.mode != ViewMode::Map {
            game.set_mode(ViewMode::Map);
        }
        // Nobody standing: the run is over, and the screen that says so
        // takes over from this one on the next frame. A run won ends on
        // the same screen, saying so (feature 106).
        if game.world.lost || game.world.is_won() {
            next.set(Screen::Over);
        }
        // What the steps sounded like: the crew's room, and the station's
        // beside it while the decks are joined — its doors and its galley
        // are on the same picture.
        // The player's own Bim is the crew's room's by its slot; the
        // station's room has nobody of ours in it.
        let own = Some(screen.net.slot as usize);
        for cued in game.world.aboard.room.take_cues() {
            if let Some(freeze) = screen.freeze.as_mut() {
                use crate::dev::Counted;
                use bims::cue::Cue;
                let counted = match (freeze.counts, cued.cue) {
                    (Counted::Shots, Cue::Shot { .. } | Cue::Blow { .. }) => true,
                    // A Guardian's beam is said as a shot of the Sweeper
                    // where it is laid (feature 100).
                    (
                        Counted::Sweeps,
                        Cue::Shot {
                            weapon: bims::combat::WeaponKind::Sweeper,
                            ..
                        },
                    ) => true,
                    (Counted::Shields, Cue::Shielded) => true,
                    _ => false,
                };
                if counted {
                    freeze.left = freeze.left.saturating_sub(1);
                }
            }
            // A door the crew do not see slides unheard, as it slides
            // undrawn (`Game::door_unseen_at`); a forcing is still heard.
            if is_door_slide(cued.cue)
                && game.world.aboard.room.door_unseen_at(cued.at.x, cued.at.y)
            {
                continue;
            }
            sounds.play(&mut commands, cued, own);
        }
        if let Some(residents) = game.world.residents.as_mut() {
            let joined = game.world.ship.state.alongside().is_some();
            for cued in residents.aboard.room.take_cues() {
                // The station's doors are the joined room's, which slide
                // for the residents too and are heard (or not) above.
                if joined && !is_door_slide(cued.cue) {
                    sounds.play(&mut commands, cued, None);
                }
            }
        }
        // The beds: a planet's air while set down at its settlement — the
        // biome's own — a station's hum while tied up with the rooms
        // joined, and the ship's own otherwise.
        let ground = game
            .world
            .landed()
            .and_then(|body| game.world.surface(body))
            .map(|surface| surface.biome);
        match (ground, &game.world.ship.state) {
            (Some(biome), _) => sounds.want(Bed::of_biome(biome)),
            (None, ShipState::Docked { .. }) => sounds.want(Bed::Station),
            _ => sounds.want(Bed::Ship),
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
    // The keys, for the panels' hints. Fresh every frame.
    if let Some(panels) = &mut screen.panels {
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
        if let Some(i) = world::medic::guest_of(who) {
            // A site's defender a medic's beam holds.
            resident_name(resident_station.unwrap_or(0), i)
        } else if who < crew_count {
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
        // How long the player's own Bim takes over a revive (task 120),
        // for the row on a downed crewmate's menu.
        if let Some(game) = session.game.as_ref() {
            panels.revive_seconds = game.world.revive_seconds(panels.player as u32);
            panels.may_lift = game.world.can_lift(panels.player as u32);
            // The crew's relics, for the side panel of a crewmate picked
            // (task 136).
            panels.crew_relics = game.world.relics().to_vec();
        }
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
                xp: progress.xp,
                ranks: progress.ranks,
                points: progress.points(class),
                can_change: !world.undocked_once,
                soldier: (class == world::Class::Soldier).then(|| crate::crew::SoldierView {
                    grenades: world.grenades_of(slot),
                    cooldown: world.grenade_cooldown_left(slot),
                    charging: world.is_charging(slot),
                    shot_cooldown: world.stun_shot_cooldown_left(slot),
                }),
                medic: (class == world::Class::Medic).then(|| crate::crew::MedicView {
                    patients: world.patients_of(slot).iter().map(|&p| name(p)).collect(),
                    drone_learnt: world.rank_of(slot, world::class::SLOT_Q) > 0,
                    drone_cooldown: world.heal_drone_cooldown_left(slot),
                    drone_left: world.drone_left(slot),
                    circle_learnt: world.rank_of(slot, world::class::SLOT_R) > 0,
                    circling: world.is_circling(slot),
                }),
                tank: (class == world::Class::Tank).then(|| crate::crew::TankView {
                    shield_up: world.is_shielding(slot),
                    shield_left: world.riot_shield_left(slot),
                    shield_whole: world.riot_shield_hp(slot),
                    shield_cooldown: world.riot_shield_cooldown_left(slot),
                    reflect_left: world.reflect_left(slot),
                    reflect_cooldown: world.reflect_cooldown_left(slot),
                    reflect_learnt: world.rank_of(slot, world::class::SLOT_E) > 0,
                    bastion_cooldown: world.bastion_cooldown_left(slot),
                    bastion_learnt: world.rank_of(slot, world::class::SLOT_R) > 0,
                }),
                commander: (class == world::Class::Commander).then(|| crate::crew::CommanderView {
                    rally_left: world.rally_left(slot),
                    cooldown: world.rally_cooldown_left(slot),
                    can_rally: progress.rank(world::class::SLOT_E) > 0,
                    cry_left: world.battle_cry_left(slot),
                    cry_cooldown: world.battle_cry_cooldown_left(slot),
                    can_cry: progress.rank(world::class::SLOT_Q) > 0,
                }),
                relics: world.relics().to_vec(),
                open_classes: world::Class::ALL
                    .into_iter()
                    .filter(|&c| unlocks.as_ref().is_none_or(|u| u.class_open(c)))
                    .collect(),
            }
        });
    }

    drop(prep_timed);

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
    let map_up = session
        .game
        .as_ref()
        .is_some_and(|g| g.mode == ViewMode::Map);
    // The world map is the galaxy chart alone (the galaxy-only map): the
    // whole canvas but the list's column while it is popped out. Every
    // trip is picked and put to the crew off the chart, a star its one
    // mission; there is no system view any more. While it is up `canvas`
    // — what the deck reads and draws in — is empty, and `full` the whole,
    // which the HUD lays itself out on.
    let full = canvas;
    let (galaxy_rect, canvas) = if map_up {
        (
            chart_rect(full, screen.world_map.column_w()),
            crate::shapes::Rect::new(full.max, full.max),
        )
    } else {
        (crate::shapes::Rect::new(full.min, full.min), full)
    };
    // The ready check (a mission with a fight in it, held for every
    // player's Ready) shows nothing of the site: the deck, the bodies and
    // everything over them wait for the mission to start, and only the
    // site's backdrop stands behind the check. The panels — the loadout,
    // the skill points — stay in reach.
    let veiled = !map_up
        && session
            .game
            .as_ref()
            .is_some_and(|g| g.world.in_mission() && g.world.awaiting_ready());
    let deck_hidden = map_up || veiled;
    let mut pointer = Pointer::read(&ctx);
    let on_canvas = pointer.on(canvas).filter(|_| !deck_hidden);
    let on_galaxy = pointer.on(galaxy_rect);
    let here = pointer.pos.map(|p| p - canvas.min);
    crate::keys::release_tab_focus(
        &ctx,
        &mut screen.tab_took_focus,
        !ctx.egui_wants_keyboard_input() && screen.sheet.is_none(),
    );
    let keys = !ctx.egui_wants_keyboard_input() && screen.sheet.is_none();
    // The galaxy chart beside the system map (task 135): the lobby's own
    // picture, made the first time the map is up. Its marks are the
    // world's: the star the ship is at, and the one picked.
    if map_up
        && screen.galaxy.is_none()
        && let Some(game) = &session.game
    {
        let world = &game.world;
        let mut chart = lobby::Lobby::new(world.galaxy_seed, world.galaxy_type, 800.0, 600.0);
        screen.chart_traders = world.trader_stars(&chart.galaxy);
        chart.inspect(screen.picked_star.unwrap_or(world.star_id));
        screen.galaxy = Some(chart);
    }
    let galaxy_up = map_up && screen.galaxy.is_some();
    if galaxy_up && let Some(chart) = &mut screen.galaxy {
        chart.here = session.game.as_ref().map(|g| g.world.star_id);
        // Where the crew are heading (the second map rework): the star picked, else the
        // one on the table.
        let heading = screen
            .picked_star
            .or_else(|| {
                session
                    .game
                    .as_ref()
                    .and_then(|g| g.world.run.proposal.as_ref())
                    .map(|p| p.site.star)
            })
            .filter(|&star| Some(star) != chart.here);
        chart.target = heading;
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
        // And the stars two lanes off a trip still reaches (the second map rework), off
        // the chart's own galaxy rather than a fresh one a frame.
        chart.far = session
            .game
            .as_ref()
            .map(|g| g.world.two_lanes_off(&chart.galaxy))
            .unwrap_or_default();
        // And the machines' origin, where the Machine Heart stands
        // (feature 108) — always, so the crew know where the run ends.
        chart.heart = session.game.as_ref().map(|g| g.world.droid_origin());
        // And every system with a trader, faded where it is closed today.
        chart.traders = session
            .game
            .as_ref()
            .map(|g| {
                let day = g.world.days_gone();
                screen
                    .chart_traders
                    .iter()
                    .map(|&star| (star, !g.world.trader_closed_on(star, day)))
                    .collect()
            })
            .unwrap_or_default();
        // And every elite's system, crowned.
        chart.elites = session
            .game
            .as_ref()
            .map(|g| g.world.elite_stars(chart.galaxy.stars.len() as u32))
            .unwrap_or_default();
        // And every star's tier today, for the rings round the stars past
        // tier one.
        chart.tiers = session
            .game
            .as_ref()
            .map(|g| {
                (0..chart.galaxy.stars.len() as u32)
                    .map(|star| g.world.system_tiers(star, g.world.clock_minutes).1.code() as u8)
                    .collect()
            })
            .unwrap_or_default();
        // And every star's one mission (the galaxy-only map): an attack's
        // blades or a defence's shield, faded where the fight is over —
        // worked out with the list, only when the run has moved on.
        if let Some(game) = session.game.as_ref() {
            screen.world_map.refresh(&game.world);
        }
        chart.missions = screen
            .world_map
            .missions
            .iter()
            .filter_map(|m| {
                let mark = match m.kind {
                    world::SiteKind::Attack => lobby::preview::Mission::Attack,
                    world::SiteKind::Defend => lobby::preview::Mission::Defend,
                    world::SiteKind::Trader => return None,
                };
                Some((m.site.star, mark, m.cleared))
            })
            .collect();
        // The way a trip goes where it can (two lanes at most, round a
        // jammer where there is a way round, the second map rework), else the shortest.
        let plotted = session.game.as_ref().zip(heading).and_then(|(g, star)| {
            g.world
                .trip_route_in(&chart.galaxy, star)
                .map(|(route, _)| route)
                .or_else(|| g.world.route_to(star))
                .map(|route| (g, route))
        });
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
        if galaxy_rect.size() != screen.galaxy_size {
            screen.galaxy_size = galaxy_rect.size();
            chart
                .preview
                .resize(galaxy_rect.size().x, galaxy_rect.size().y);
        }
    }
    // Where the pointer is in the world's views, the same place on every
    // machine: on the galaxy chart or on the deck. Nothing over a panel.
    let in_view = if let Some(p) = on_galaxy.filter(|_| galaxy_up) {
        screen.galaxy.as_ref().map(|chart| {
            let (x, y) = chart.preview.to_galaxy(p.x, p.y);
            Spot::Galaxy(x, y)
        })
    } else if let Some(p) = on_canvas {
        let (x, y) = session.design_point(p.x, p.y);
        Some(Spot::Deck(x, y))
    } else {
        None
    };
    // Ctrl and a left click is a ping, on the deck or the galaxy chart: a
    // mark in this player's colour on everybody's screen where it was
    // put. The press is the ping's and nothing else's — it is taken off
    // the pointer, so no pick, marquee or drag follows it. It was Alt
    // until Alt became the dodge roll (task 150).
    if pointer.primary_pressed
        && ctx.input(crate::keys::ping_held)
        && let Some(at) = in_view
    {
        online.ping(now, at);
        pointer.primary_pressed = false;
    }
    let panels = screen.panels.as_mut().unwrap();

    // Where this pointer is, to the room: over the trader's window or the
    // relic choice (where they stood last frame, since they are laid out
    // below), else wherever it is in the views.
    let over_window = pointer.pos.and_then(|p| {
        let p = egui::pos2(p.x, p.y);
        let from = |r: egui::Rect| (p.x - r.min.x, p.y - r.min.y);
        if let Some(r) = super::worldmap::relic_rect(&ctx).filter(|r| r.contains(p)) {
            let (x, y) = from(r);
            Some(Spot::Relic(x, y))
        } else if let Some(r) = super::worldmap::trader_rect(&ctx).filter(|r| r.contains(p)) {
            let (x, y) = from(r);
            Some(Spot::Trader(x, y))
        } else {
            None
        }
    });
    online.point(now, over_window.or(in_view));

    // Middle drags pan, in either view: the deck, or the galaxy chart
    // with the map up.
    let at_galaxy = pointer.pos.map(|p| p - galaxy_rect.min);
    if pointer.middle_pressed {
        if let Some(p) = on_galaxy.filter(|_| galaxy_up) {
            screen.pan_from = Some(p);
            screen.pan_galaxy = true;
        } else if let Some(p) = on_canvas {
            screen.pan_from = Some(p);
            screen.pan_galaxy = false;
        }
    }
    // A left press on the galaxy chart (the map rework) is a click on a star or
    // a drag of the chart, whichever it turns out to be: past the click's
    // slop it pans, and a release short of it picks (below, with the
    // hover).
    if !galaxy_up {
        screen.chart_press = None;
    }
    if let Some(p) = on_galaxy.filter(|_| galaxy_up && pointer.primary_pressed) {
        screen.chart_press = Some((p, false));
    }
    if let Some((from, moved)) = screen.chart_press
        && pointer.primary_down
        && let Some(p) = at_galaxy
        && let Some(chart) = &mut screen.galaxy
        && (moved || (p - from).length() > CLICK_SLOP)
    {
        chart.preview.pan(p.x - from.x, p.y - from.y);
        screen.chart_press = Some((p, true));
    }
    if let Some(from) = screen.pan_from {
        let now_at = if screen.pan_galaxy { at_galaxy } else { here };
        match now_at {
            Some(p) if pointer.middle_down => {
                match &mut screen.galaxy {
                    Some(chart) if galaxy_up && screen.pan_galaxy => {
                        chart.preview.pan(p.x - from.x, p.y - from.y)
                    }
                    _ => session.pan(p.x - from.x, p.y - from.y),
                }
                screen.pan_from = Some(p);
            }
            _ => screen.pan_from = None,
        }
    }
    if pointer.scroll != 0.0 {
        match &mut screen.galaxy {
            Some(chart) if galaxy_up && on_galaxy.is_some() => {
                let p = on_galaxy.unwrap_or_default();
                chart.preview.zoom(p.x, p.y, zoom_factor(pointer.scroll))
            }
            _ => {
                if let Some(p) = on_canvas {
                    // Following the player's own Bim the ship view zooms about
                    // the middle, where the Bim is, so it stays there (task
                    // 144); the map and a free camera about the pointer.
                    let follows = !map_up && session.game.as_ref().is_some_and(|g| g.follow);
                    let at = if follows { canvas.size() / 2.0 } else { p };
                    session.zoom(at.x, at.y, zoom_factor(pointer.scroll))
                }
            }
        }
    }

    if map_up {
        screen.hover_at = None;
        if let Some(game) = &mut session.game {
            game.hover = None;
        }
        // On the chart, the pointer is over stars: the one under it is
        // rung and its mission is what the column's card shows (feature
        // 107), and a click picks it — the crisis's word on it goes into
        // the strip, the route to it is drawn and the bar offers the trip.
        if galaxy_up && let Some(chart) = &mut screen.galaxy {
            match on_galaxy {
                Some(p) => chart.hover(p.x, p.y),
                None => chart.hovered = None,
            }
            screen.world_map.hovered = chart
                .hovered
                .and_then(|star| screen.world_map.star_site(star));
            // A left press let go before it became a drag is the click:
            // the star picked, and its mission on the list (the galaxy-only
            // map: a star is gone to by its one mission).
            if let Some((_, moved)) = screen.chart_press
                && !pointer.primary_down
            {
                screen.chart_press = None;
                if !moved
                    && on_galaxy.is_some()
                    && let Some(star) = chart.hovered
                {
                    chart.inspect(star);
                    screen.picked_star = Some(star);
                    screen.world_map.pick_star(star);
                }
            }
        }
    } else {
        // In the ship view the pointer is over the room aboard: a left
        // click or a marquee selects a Bim — never a bot under arms — a
        // right click on the deck sends the player's own Bim there, and a
        // click on a fixture opens its menu. The
        // marquee is drawn on the deck rather than on the glass — it turns
        // with the ship — which is the box the room tests the crew against.
        screen.hover_at = pointer
            .pos
            .filter(|p| canvas.contains(*p))
            .map(|p| p - canvas.min);
        if let Some(game) = &mut session.game {
            game.hover = screen.hover_at.map(|p| game.tile_at(p.x, p.y));
        }
        // With the attack key pressed the pointer is about one thing and
        // nothing else (feature 84): the system's cursor goes and a red
        // crosshair is drawn in its place, below, a left click puts the
        // banner down where it points, and a right-click thinks better
        // of it. The Mine tool's shape, and for the same reason — a
        // click that both selected a Bim and sent the crew somewhere
        // would be a click nobody could undo.
        if let Some(throw) = screen.aiming_throw {
            // The throw's armed pointer: a left click throws at the tile
            // under it, walking out first where it must (a Stun Shot
            // charges at it); a right-click thinks better of it.
            if let Some(p) = on_canvas {
                ctx.set_cursor_icon(egui::CursorIcon::Crosshair);
                if pointer.primary_pressed {
                    let (rx, ry) = session.room_point(p.x, p.y);
                    let t = shipdesign::TILE as f32;
                    let tile = ((rx / t).floor() as i32, (ry / t).floor() as i32);
                    let world = session.game.as_ref().map(|g| &g.world);
                    let (order, line) = throw_order(world, screen.net.slot, throw, tile);
                    orders.extend(order);
                    screen.log.extend(line);
                    screen.aiming_throw = None;
                    screen.trigger_spent = true;
                }
                if pointer.secondary_pressed {
                    screen.aiming_throw = None;
                }
            }
        } else if screen.aiming_move {
            // The attack-move's armed pointer: a left click sends the
            // player's own Bim there under arms, a right-click thinks
            // better of it — the banner's shape, for the same reason.
            if let Some(p) = on_canvas {
                ctx.set_cursor_icon(egui::CursorIcon::None);
                if pointer.primary_pressed {
                    let (rx, ry) = session.room_point(p.x, p.y);
                    // On an enemy the attack-move is the attack on it
                    // (task 126), as Dota's A-click on a unit is.
                    let order = match session.room().and_then(|room| room.enemy_at(rx, ry)) {
                        Some(enemy) => CrewOrder::Attack {
                            enemy: enemy as u32,
                        },
                        None => CrewOrder::AttackMove { x: rx, y: ry },
                    };
                    orders.push(crew_order(order, pointer.shift));
                    screen.aiming_move = false;
                    screen.trigger_spent = true;
                }
                if pointer.secondary_pressed {
                    screen.aiming_move = false;
                }
            }
        } else if screen.aiming_attack {
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
                    screen.trigger_spent = true;
                }
                if pointer.secondary_pressed {
                    screen.aiming_attack = false;
                }
            }
        } else if let Some(p) = on_canvas {
            let (rx, ry) = session.room_point(p.x, p.y);
            // The pointer is the aim (task 144): the system's cursor goes
            // and the aim's reticle is drawn in its place, below.
            ctx.set_cursor_icon(egui::CursorIcon::None);
            // The left button is the trigger (task 144), which the control
            // order below carries, and a left press closes any menu open.
            if pointer.primary_pressed {
                panels.close_menu();
            }
            // A right-click walks nobody anywhere and attacks nobody (task
            // 144). A downed crewmate or townsperson under it is the
            // revive, the medkit in hand or not — the walk over and the
            // hands on it (task 138); with Shift held it waits its turn
            // (feature 69). Otherwise it
            // is what a left click was until the left button became the
            // trigger: the one under it picked, and the menu of what is
            // there opened — a door, a body down, a mercenary for
            // hire. A living crew member is picked, not menued.
            if pointer.secondary_pressed {
                panels.close_menu();
                if let Some(room) = session.room() {
                    match downed_patient(room, panels.player, rx, ry, &crew_name) {
                        Some(Ok(patient)) => orders.push(crew_order(
                            CrewOrder::Revive {
                                who: panels.player as u32,
                                patient,
                            },
                            pointer.shift,
                        )),
                        Some(Err(why)) => screen.log.push(why),
                        None => {
                            orders.push(Order::Crew(CrewOrder::Select {
                                x0: rx,
                                y0: ry,
                                x1: rx,
                                y1: ry,
                            }));
                            let mut fixture = room.hit_at(rx, ry);
                            if fixture == HIT_BIM && !room.is_down(room.hit_bim()) {
                                fixture = 0;
                            }
                            if fixture != 0
                                && let Some(at) = pointer.pos
                            {
                                panels.open_menu(fixture, egui::pos2(at.x, at.y), room);
                            }
                        }
                    }
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
                                let at = pointer.pos.unwrap();
                                panels.open_menu(fixture, egui::pos2(at.x, at.y), room);
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
    }

    // The player's own Bim under the keys and the pointer (task 144):
    // WASD walk it up, left, down and right on the screen, it turns to
    // the pointer at once, and the left button held fires
    // along its facing as fast as the weapon goes. Said to the room as a
    // `CrewOrder::Control` whenever it changes (`control_due`); with the
    // pointer off the deck the aim is the last one said.
    if !map_up
        && screen.sheet.is_none()
        && let Some(at) = session
            .room_ref()
            .filter(|room| (screen.net.slot as usize) < room.crew_count() as usize)
            .map(|room| room.bim_pos(screen.net.slot as usize))
    {
        let (mut wx, mut wy) = (0.0f32, 0.0f32);
        // Shift sprints and Alt dodge-rolls (task 150).
        let (mut sprint, mut dodge_down) = (false, false);
        // And the reload key (October 2026), a press.
        let mut reload = false;
        if keys {
            ctx.input(|i| {
                for (action, dx, dy) in [
                    (Action::WalkUp, 0.0, -1.0),
                    (Action::WalkDown, 0.0, 1.0),
                    (Action::WalkLeft, -1.0, 0.0),
                    (Action::WalkRight, 1.0, 0.0),
                ] {
                    if keys_now.down(i, action) {
                        wx += dx;
                        wy += dy;
                    }
                }
                sprint = crate::keys::sprint_held(i);
                dodge_down = crate::keys::dodge_held(i);
                reload = keys_now.pressed(i, Action::Reload)
                    && crate::keys::plain_or_sprinting(i.modifiers);
            });
        }
        let dodge = dodge_down && !screen.dodge_was;
        screen.dodge_was = dodge_down;
        // Up the screen is whichever way the room lies under the camera,
        // head up or north up: two points of the canvas, through it.
        let middle = canvas.size() / 2.0;
        let walk = (wx != 0.0 || wy != 0.0).then(|| {
            let (x0, y0) = session.room_point(middle.x, middle.y);
            let (x1, y1) = session.room_point(middle.x + wx * 100.0, middle.y + wy * 100.0);
            angle_code((y1 - y0).atan2(x1 - x0))
        });
        // Walking brings the Bim back to the middle: a middle drag's shove
        // on the camera following it eases away while the keys are down.
        if walk.is_some()
            && let Some(game) = session.game.as_mut()
            && game.follow
        {
            game.ship_view
                .ease_pan((-(dt as f32) * RECENTRE_RATE).exp());
        }
        let aim = on_canvas
            .map(|p| {
                let (rx, ry) = session.room_point(p.x, p.y);
                angle_code((ry - at.y).atan2(rx - at.x))
            })
            .or(screen.control.map(|((_, aim, _, _), _)| aim));
        // The left button. A press counts as well as a button held: a quick
        // click goes down and up within one frame, and the room owes it its
        // shot (`character::TRIGGER_OWED`). A press an armed pointer took —
        // a throw, an attack-move, the banner — fires nothing until the
        // button has come up.
        if !pointer.primary_down && !pointer.primary_pressed {
            screen.trigger_spent = false;
        }
        let fire = (pointer.primary_down || pointer.primary_pressed)
            && !screen.trigger_spent
            && on_canvas.is_some()
            && !screen.aiming_attack
            && !screen.aiming_move
            && screen.aiming_throw.is_none();
        // A sprint is said only while the keys walk it: Shift alone is
        // also an order's wait-its-turn, and nothing to send then.
        let sprint = sprint && walk.is_some();
        if let Some(aim) = aim
            && control_due(screen.control, (walk, aim, fire, sprint), now)
        {
            screen.control = Some(((walk, aim, fire, sprint), now));
            orders.push(Order::Crew(CrewOrder::Control {
                walk,
                aim,
                fire,
                sprint,
            }));
        }
        // The roll after the keys, so it goes the way they walk it now.
        if dodge {
            orders.push(Order::Crew(CrewOrder::Dodge));
        }
        if reload {
            orders.push(Order::Crew(CrewOrder::Reload));
        }
    }

    // A rank-up asked for this frame (task 123): Ctrl and a slot's key
    // here, or Ctrl and a click on the slot's box in the hero panel below.
    let mut rank_up_asked: Option<RankUp> = None;
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
                    screen.free_camera = !on;
                }
            }
            // With no modifier held, or Shift alone: sprinting, every key
            // below still does what it does — an ability, an item, the
            // medkit — mid-run.
            if crate::keys::plain_or_sprinting(i.modifiers) {
                // The engineer's remote trigger (task 154): Space sets off
                // every satchel of his. Space paused the world until then;
                // the pause is the Esc sheet's now.
                if keys_now.pressed(i, Action::Detonate)
                    && let Some(game) = &session.game
                {
                    let (order, line) = detonate_key(&game.world, screen.net.slot);
                    orders.extend(order);
                    screen.log.extend(line);
                }
                // The 1× key (task 119: 1× or paused, nothing else) sets
                // going: an order, so a pause by anybody is a pause for
                // everybody.
                if keys_now.pressed(i, Action::Speed1) {
                    orders.push(Order::Speed(Speed::Real));
                }
                // The medkit (task 138; one key, H, since October 2026):
                // into the player's own Bim's hands, or the weapon back
                // with it there already. An order, so every copy of the
                // room sees the same hands.
                if keys_now.pressed(i, Action::Medkit)
                    && let Some(game) = &session.game
                {
                    let held = game.world.aboard.room.hand(screen.net.slot as usize);
                    let hand = if held == bims::bim::Hand::Medkit {
                        bims::bim::Hand::Weapon
                    } else {
                        bims::bim::Hand::Medkit
                    };
                    orders.push(Order::Crew(CrewOrder::Hand { hand }));
                }
                // The four item slots, 1 to 4 (October 2026): the item in
                // that slot used at the pointer — a Blink Drive blinks
                // there. The world says why not, into the log. Read
                // through Shift, which turns 1 into `!`.
                if let Some(p) = on_canvas {
                    for (index, action) in Action::ITEMS.into_iter().enumerate() {
                        if keys_now.pressed_through_shift(&i.events, action) {
                            let (rx, ry) = session.room_point(p.x, p.y);
                            orders.push(Order::UseItem {
                                item: index as u32,
                                x: rx.round() as i32,
                                y: ry.round() as i32,
                            });
                        }
                    }
                }
                // Select: the crew member you steer, selected and in the middle.
                if keys_now.pressed(i, Action::Select) {
                    orders.push(Order::Crew(CrewOrder::SelectOwn));
                    if let Some(game) = &mut session.game {
                        game.centre_on_player();
                    }
                }
                // Recruit recruits (on L since task 123 gave R to the
                // fourth ability slot).
                if keys_now.pressed(i, Action::Recruit) {
                    orders.push(Order::Crew(CrewOrder::Recruit));
                }
                // Tab: the Armory panel (task 113) — every loadout, the
                // armory, the money and the keys; read-only in a mission.
                if keys_now.pressed(i, Action::Inventory) {
                    panels.toggle_armory();
                }
                // K: the character sheet (feature 107).
                if keys_now.pressed(i, Action::CharacterSheet) {
                    panels.toggle_sheet();
                }
                // The four ability slots, Q, C, E and R (task 123): the
                // first and third are the steered crew member's class's
                // two actions (features 74 and 75) — an engineer's sentry
                // and sandbags on the deck tile under the pointer, a
                // soldier's grenade at it and its Stun Shot — and the second
                // and fourth are empty for every class, so pressing one
                // does nothing. The world says why not, into the log;
                // with a classless crew member steered, nothing at all.
                // Read with Ctrl up (`Keys::used`): with it held the key
                // is the slot's rank-up, below.
                // While a medic's beam or circle key is held, its reach is
                // drawn round the player's own Bim.
                screen.held_reach = session
                    .game
                    .as_ref()
                    .filter(|_| !map_up)
                    .and_then(|game| held_reach(&game.world, screen.net.slot, &keys_now, i));
                // While a throw has the pointer armed, the burst's ring is
                // drawn on the tile under the pointer.
                screen.throw_aim = None;
                if screen.aiming_throw.is_some()
                    && let Some(p) = on_canvas.filter(|_| !map_up)
                {
                    let (rx, ry) = session.room_point(p.x, p.y);
                    let t = shipdesign::TILE as f32;
                    screen.throw_aim = Some(((rx / t).floor() as i32, (ry / t).floor() as i32));
                }
                for action in Action::ABILITIES {
                    let slot = screen.net.slot;
                    // A ranked kit's four slots are its four abilities
                    // (task 124); every other class has two keys.
                    let ranked = session
                        .game
                        .as_ref()
                        .is_some_and(|g| world::class::ranked(g.world.class_of(slot)));
                    let primary = slot_action(action);
                    if !ranked && primary.is_none() {
                        continue;
                    }
                    if !keys_now.used(&i.events, i.modifiers, action) {
                        continue;
                    }
                    let Some(game) = &session.game else {
                        continue;
                    };
                    // A throw — a soldier's grenade (Q), an engineer's
                    // satchel charge (E) — is a quick throw: the press arms
                    // the pointer and draws the reach, and letting the key
                    // go throws at the tile under it (below; a click
                    // before then throws too, a right-click or Esc thinks
                    // better of it).
                    if let Some(throw) = throw_key(game.world.class_of(slot), action) {
                        match can_arm_throw(&game.world, slot, throw) {
                            Ok(()) => {
                                screen.aiming_throw = (!map_up).then_some(throw);
                                screen.aiming_attack = false;
                                screen.aiming_move = false;
                            }
                            Err(line) => screen.log.push(line),
                        }
                        continue;
                    }
                    let room = on_canvas
                        .filter(|_| !map_up)
                        .map(|p| session.room_point(p.x, p.y));
                    let tile = room.map(|(rx, ry)| {
                        let t = shipdesign::TILE as f32;
                        ((rx / t).floor() as i32, (ry / t).floor() as i32)
                    });
                    // And the crew member under the pointer, for a
                    // medic's beam (feature 76) — its own Bim among them,
                    // since a medic may beam itself (task 120) — or, with
                    // no crewmate there, a site's defender; off everybody,
                    // the nearest friendly the beam reaches near it.
                    let under =
                        room.and_then(|(rx, ry)| game.world.beam_patient_near(slot, rx, ry));
                    let (order, line) = match primary {
                        _ if ranked => ranked_key(&game.world, slot, action, tile, under),
                        Some(primary) => class_key(&game.world, slot, primary, tile, under),
                        None => (None, None),
                    };
                    orders.extend(order);
                    screen.log.extend(line);
                }
                // The quick throw's other half: the throw key let go with
                // the pointer still armed throws at the tile under it —
                // walking out first where it must, as the click does —
                // or, the pointer off the deck, puts the throw away.
                if let Some(throw) = screen.aiming_throw
                    && !keys_now.down(i, throw_action(throw))
                {
                    screen.aiming_throw = None;
                    if let Some(p) = on_canvas.filter(|_| !map_up) {
                        let (rx, ry) = session.room_point(p.x, p.y);
                        let t = shipdesign::TILE as f32;
                        let tile = ((rx / t).floor() as i32, (ry / t).floor() as i32);
                        let world = session.game.as_ref().map(|g| &g.world);
                        let (order, line) = throw_order(world, screen.net.slot, throw, tile);
                        orders.extend(order);
                        screen.log.extend(line);
                    }
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
                // The held revive (G): standing close to a downed crewmate,
                // the player's own Bim gets the nearest back up while the
                // key is held, and lets go when it comes up first.
                if let Some(game) = &session.game {
                    let (order, line) = held_revive(
                        &game.world.aboard.room,
                        panels.player,
                        &mut screen.held_revive,
                        !map_up && keys_now.down(i, Action::Revive),
                        keys_now.pressed(i, Action::Revive),
                        &crew_name,
                    );
                    orders.extend(order);
                    screen.log.extend(line);
                }
                // And every player's own two (feature 84). **Attack**
                // arms the pointer rather than doing anything — on the
                // first press, banner down or not: the banner goes down
                // on the click after it, below, so that the player
                // picks the ground. Pressed while the pointer is
                // already armed, it is thought better of, and **a
                // banner already down is taken up** with it (the key's
                // second press, `attack_key`).
                // **Attack-move** arms the pointer for the player's own
                // Bim: the next click on the deck is where it walks
                // under arms. Pressed again, it is thought better of.
                if keys_now.pressed(i, Action::AttackMove) {
                    screen.aiming_move = !screen.aiming_move && !map_up;
                    screen.aiming_attack = false;
                    screen.aiming_throw = None;
                }
                if keys_now.pressed(i, Action::Attack) {
                    screen.aiming_move = false;
                    screen.aiming_throw = None;
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
                    screen.aiming_move = false;
                    let (order, line) =
                        orders_key(&game.world, screen.net.slot, world::Standing::Retreat);
                    orders.extend(order);
                    screen.log.extend(line);
                }
            }
            // Ctrl and a slot's key: that slot ranked up (task 123),
            // never the ability used — the rows above are read with no
            // modifier held but Shift. Handed on after the hero panel, with a
            // Ctrl-click on its box, through the one `rank_up`.
            rank_up_asked = rank_up_by_key(&keys_now, &i.events, i.modifiers).or(rank_up_asked);
            if i.key_pressed(egui::Key::Escape) {
                if screen.aiming_attack || screen.aiming_move || screen.aiming_throw.is_some() {
                    // The armed pointer is put away first, and nothing
                    // else happens — the Mine tool's rule.
                    screen.aiming_attack = false;
                    screen.aiming_move = false;
                    screen.aiming_throw = None;
                } else if panels.escape() {
                    // A menu, a container window or the character sheet
                    // is shut without opening the sheet.
                } else {
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
        // The keys pan the map alone — the galaxy chart, which is all of
        // it: over the deck WASD walk the player's own Bim (task 144), and
        // the camera follows it.
        if d != Vec2::ZERO && map_up {
            match &mut screen.galaxy {
                Some(chart) => chart.preview.pan(d.x, d.y),
                None => session.pan(d.x, d.y),
            }
        }
    } else if screen.sheet.is_some()
        && keys_now.listening.is_none()
        && ctx.input(|i| i.key_pressed(egui::Key::Escape))
    {
        // Esc while the controls page waits on a key is that page's.
        screen.sheet = None;
    }
    // The pointer against the window's edge pans (task 123) whatever a
    // middle drag pans — the deck, or the galaxy chart — by the same
    // calls, so Follow takes it as it takes a drag; beside WASD, and
    // paused as well, since it is the camera and not the world.
    // Not over the deck while the camera follows the player's own Bim
    // (task 144): it would only shove the view off it.
    let camera_pans = map_up || session.game.as_ref().is_some_and(|g| !g.follow);
    let edge = edge_pan_now(&ctx, &pointer, &keys_now, window.focused, dt as f32).filter(|_| {
        screen.sheet.is_none()
            && screen.pan_from.is_none()
            && screen.chart_press.is_none()
            && camera_pans
    });
    if let Some(d) = edge {
        match &mut screen.galaxy {
            Some(chart) if galaxy_up => chart.preview.pan(d.x, d.y),
            _ => session.pan(d.x, d.y),
        }
    }

    // Everything that changes the ship goes through the seam.
    for order in orders.drain(..) {
        loading.order(&screen.net, session, order);
    }

    drop(canvas_timed);
    let panels_timed = crate::perf::scope(crate::perf::Phase::Panels);
    // --- the HUD (feature 107) -----------------------------------------------------
    // Minimal and about the player's own Bim: the portraits and one frame
    // along the top, the hero panel at the foot, the tray at the bottom
    // left, *Back to ship* and the log at the bottom right, and the
    // inspect panel on the right while somebody is picked — everything
    // else a key or a button away. `hud`'s module note says where each
    // piece sits and why none of them lands on another. With the map up
    // the canvas is the chart and a column down its right, and nothing
    // else.
    let area = egui_rect(full);
    let out = session
        .game
        .as_ref()
        .is_some_and(|g| g.world.run.is_out(local))
        || crate::dev::out();
    // A player whose Bim is out watches a crewmate's, which the portraits
    // pick: the camera follows the one watched, tethered to it the first
    // frame out. Back in, the camera follows the player's own again — as
    // it does from the first frame of a run, since the keys walk it and
    // the pointer aims it (task 144); the follow key lets it go.
    if let Some(game) = session.game.as_mut() {
        if !screen.free_camera && !game.follow {
            game.set_follow(true);
        }
        let crew = game.world.aboard.crew_count();
        let room = &game.world.aboard.room;
        let watchable = |w: u32| w != local && w < crew && room.is_alive(w as usize);
        let next = if out {
            game.spectate
                .filter(|&w| watchable(w))
                .or_else(|| (0..crew).find(|&w| watchable(w)))
        } else {
            None
        };
        let first = next.is_some() && game.spectate.is_none();
        let back = !out && game.spectate.is_some();
        game.spectate = next;
        if first || back {
            game.set_follow(true);
        }
    }

    // What is under the pointer on the deck — the part, or the fixture and
    // its state, and the tile — as a small readout at the pointer once it
    // has rested there; it was a panel of the left stack.
    let readout = match screen.hover_at {
        Some(p) if !deck_hidden && session.game_tile_inside() => {
            let game = session.game.as_ref().unwrap();
            let part = session.game_hovered_part();
            let mut thing = part
                .and_then(|id| session.part_kind(id))
                .map(part_name)
                .unwrap_or("—")
                .to_string();
            let (rx, ry) = session.room_point(p.x, p.y);
            let room = session.room_ref().unwrap();
            let (spot, room_thing) = CrewPanels::spot_readout(room, rx, ry);
            if !PLAIN_SPOTS.contains(&spot) {
                thing = room_thing;
            }
            let tile = game.hover.unwrap_or((0, 0));
            Some(format!("{thing} · {}, {}", tile.0, tile.1))
        }
        _ => None,
    };
    screen.rest = match (screen.rest, on_canvas.filter(|_| !deck_hidden)) {
        (Some((at, since)), Some(p)) if (p - at).length() <= 2.0 => Some((at, since)),
        (_, Some(p)) => Some((p, now)),
        _ => None,
    };
    if let (Some(words), Some((_, since)), Some(at)) = (readout, screen.rest, pointer.pos)
        && now - since >= HOVER_REST
    {
        hud::readout(&ctx, egui::pos2(at.x, at.y), &words);
    }

    let game = session.game.as_ref().unwrap();
    let world = &game.world;
    let mut asks: Vec<TrayAsk> = Vec::new();
    let mut cast_reaches: Vec<u32> = Vec::new();
    let mut portrait_press = None;
    let mut column = None;
    // Whether the side panel is to be drawn, and where it may reach: its
    // top under the top frame, its foot over whatever shares the right of
    // the canvas with it.
    let mut side_at: Option<(egui::Pos2, f32)> = None;
    let mut focus_here = false;
    if map_up {
        column = Some(super::worldmap::map_column(
            &ctx,
            area,
            &mut screen.world_map,
            world,
            local,
            &crew_name,
        ));
        // The galaxy chart's word on the star looked at, where the strip
        // used to say it.
        if galaxy_up {
            let star = screen.picked_star.unwrap_or(world.star_id);
            egui::Area::new(egui::Id::new("hud-crisis"))
                .fixed_pos(area.min + egui::vec2(MARGIN, MARGIN))
                .order(egui::Order::Middle)
                .show(&ctx, |ui| {
                    panel_frame().show(ui, |ui| {
                        ui.set_max_width(360.0);
                        // Its one mission and where it is fought, and
                        // nothing else: the chart's marks say the rest.
                        let mission = screen
                            .world_map
                            .missions
                            .iter()
                            .find(|m| m.site.star == star);
                        if let Some(mission) = mission {
                            chart_mission_line(ui, mission);
                        }
                        // The chart's own control: bring the ship's system back to
                        // the middle of the chart after a drag has lost it. In
                        // this panel, since the chart's top right corner is the
                        // list's tab now the chart is the whole map.
                        let button = egui::Button::new("Focus current system")
                            .wrap_mode(egui::TextWrapMode::Extend);
                        if ui
                            .add(button)
                            .on_hover_text("Centre the galaxy chart on the system the ship is in")
                            .clicked()
                        {
                            focus_here = true;
                        }
                    });
                });
        }
        // The bar that puts a trip to the crew, at the foot of the map
        // in the middle of the chart (the second map rework), and the log over it.
        let map_right = area.max.x - MARGIN - screen.world_map.column_w();
        let bar = super::worldmap::propose_bar(
            &ctx,
            (area.min.x + map_right) / 2.0,
            area.max.y - MARGIN,
            &screen.world_map,
            world,
            local,
            &mut orders,
            &crew_name,
        );
        hud::log_area(
            &ctx,
            area,
            map_right - GAP,
            bar.map_or(0.0, |r| r.height() + GAP),
            &screen.log,
        );
        // This player's money at the top middle of the chart, where the
        // ship's top frame says it, and the crew's relics under it.
        let money = egui::Area::new(egui::Id::new("map-money"))
            .fixed_pos(egui::pos2(
                (area.min.x + map_right) / 2.0,
                area.min.y + MARGIN,
            ))
            .pivot(egui::Align2::CENTER_TOP)
            .order(egui::Order::Middle)
            .show(&ctx, |ui| {
                panel_frame().show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new(MAP_MONEY).color(theme::MUTED));
                        ui.label(
                            egui::RichText::new(crate::format::euros(world.share_of(local)))
                                .strong()
                                .size(16.0)
                                .color(theme::ACCENT),
                        );
                    })
                    .response
                    .on_hover_text(MAP_MONEY_TIP);
                });
            })
            .response
            .rect;
        hud::relic_bar(&ctx, money.center().x, money.max.y + 6.0, world.relics());
    } else {
        let threats = hud::threats(world, local);
        let paused = world.effective_speed().multiplier() == 0;
        let cells = hud::portraits_of(world, local, game.spectate);
        let (faces, press) = hud::portraits(&ctx, area.min + egui::vec2(MARGIN, MARGIN), &cells);
        portrait_press = press;
        let top = hud::top_frame(&ctx, area, faces.max.x, world, local, &threats, paused);
        let mut top_foot = top.max.y;
        if out {
            top_foot = hud::out_banner(
                &ctx,
                top.center().x,
                top.max.y + 6.0,
                world.crew_money(),
                world.rewards().buyback,
            )
            .max
            .y;
        }
        // The crew's relics under it all, always in sight.
        if let Some(bar) = hud::relic_bar(&ctx, top.center().x, top_foot + 6.0, world.relics()) {
            top_foot = bar.max.y;
        }

        // The tray, anchored at the bottom left and growing upwards.
        let tray_view = TrayView {
            out,
            bots: cells.iter().filter(|c| !c.player).cloned().collect(),
            standing: world.standing_of(local).code(),
        };
        let tray = egui::Area::new(egui::Id::new("game-tray"))
            .anchor(
                egui::Align2::LEFT_BOTTOM,
                egui::vec2(area.min.x + MARGIN, -MARGIN),
            )
            .order(egui::Order::Middle)
            .show(&ctx, |ui| {
                tray_frame().show(ui, |ui| {
                    if let Some(room) = session.room_ref() {
                        asks = panels.tray(ui, room, &tray_view, &name);
                    }
                });
            })
            .response
            .rect;

        // The character sheet, under the portraits and over the tray.
        let sheet = if out {
            None
        } else {
            session.room_ref().and_then(|room| {
                panels.character_sheet(
                    &ctx,
                    room,
                    egui::pos2(area.min.x + MARGIN, faces.max.y + GAP),
                    tray.min.y - GAP,
                )
            })
        };

        // *Back to ship* at the bottom right, during a mission, and the log
        // directly over it.
        let back = super::worldmap::back_to_ship(&ctx, area.max.x, world, local, out, &mut orders);
        let lift = back.map_or(0.0, |r| r.height() + 6.0);
        let log = hud::log_area(&ctx, area, area.max.x - MARGIN, lift, &screen.log);

        // The hero panel, the player's own Bim, at the foot: clear of
        // whatever on the left or the right reaches down into its row.
        let mut hero_rect = None;
        if !out && local < world.aboard.crew_count() {
            let room = &world.aboard.room;
            let w = local as usize;
            let alive = room.is_alive(w);
            let hero = hud::Hero {
                class: world.class_of(local),
                xp: world.progress_of(local).xp,
                points: room.health(w),
                max: room.max_health(w),
                armour: if alive { room.armour_health(w) } else { 0.0 },
                hurt: crate::crew::is_hurt(room, w),
                downed: alive && room.is_down(w),
                down_left: room.down_left(w),
                peril: crate::crew::peril_summary(room, w),
                points_waiting: world.points_of(local),
                magazine: room.magazine(w),
            };
            let band = ctx
                .memory(|m| m.area_rect(egui::Id::new("hud-hero")))
                .map_or(area.max.y - 100.0, |r| r.min.y);
            let clear = [Some(tray), sheet]
                .into_iter()
                .flatten()
                .filter(|r| r.max.y > band)
                .fold(area.min.x, |x, r| x.max(r.max.x));
            let right = [back, log]
                .into_iter()
                .flatten()
                .filter(|r| r.max.y > band)
                .fold(area.max.x, |x, r| x.min(r.min.x));
            let boxes = ability_boxes(world, local, &keys_now);
            // A Ctrl-click on a slot's box is the rank-up Ctrl and its
            // key are (task 123) — not with the Esc sheet up, nor a
            // text field holding the keyboard, the key's own rule.
            let mut clicked = None;
            // The quickselect (task 138) before the slots: what is in the
            // hands, and a click the key's own order.
            let hand = room.hand(w);
            let mut hand_picked = None;
            let got = hud::hero_panel(&ctx, area, clear, right, &hero, |ui| {
                hand_picked = quickselect(ui, hand, &keys_now);
                let row = ability_row(ui, &boxes);
                clicked = row.rank_up;
                // The four items beside the abilities, Dota's way
                // (October 2026): a player's own Bim's alone.
                ui.separator();
                item_grid(ui, world, local, &keys_now);
                row.hovered
            });
            if keys {
                rank_up_asked = rank_up_asked.or(clicked);
            }
            if let Some(hand) = hand_picked.filter(|&h| h != hand) {
                orders.push(Order::Crew(CrewOrder::Hand { hand }));
            }
            // Whom the box the pointer rests on would reach (feature 86),
            // for the ring on the deck below.
            if let Some(action) = got.hovered {
                cast_reaches = affected_by(world, local, action);
            }
            hero_rect = Some(got.rect);
        }

        // The inspect panel, on the right while somebody is picked: under
        // the top frame, and over the log, *Back to ship* and the hero
        // panel wherever they reach its column.
        let side_x = (area.max.x - (crate::crew::SIDE_W + 30.0)).max(area.min.x + MARGIN);
        let floor = [back, log, hero_rect]
            .into_iter()
            .flatten()
            .filter(|r| r.max.x > side_x)
            .fold(area.max.y - MARGIN, |y, r| y.min(r.min.y))
            - GAP;
        side_at = Some((egui::pos2(side_x, top_foot + GAP), floor));
    }
    // The departure check, over everything while it is asking — map up or
    // not, since it is everybody's question.
    super::worldmap::departure_window(&ctx, world, local, &mut orders, &crew_name);
    // And the ready check, while a mission with a fight in it waits for
    // every player's *Ready*.
    super::worldmap::ready_window(&ctx, world, local, &mut orders, &crew_name, &|slot| {
        slot_colour(&session.crew_tints, slot)
    });
    // And a relic being chosen (feature 106): the reward screen after an
    // elite's site cleared, over the map.
    // The others as the two windows over the map show them: their
    // pointers over this player's copy of each, and what each has their
    // eye on outlined, in their colours; and what this player has, said
    // to them once both are laid out.
    let mates = super::worldmap::Mates {
        pointers: online
            .others_pointing()
            .into_iter()
            .map(|(slot, spot)| {
                (
                    slot_colour(&session.crew_tints, slot),
                    crew_name(slot),
                    spot,
                )
            })
            .collect(),
        choices: online
            .others_choosing()
            .into_iter()
            .map(|(slot, choice)| (slot_colour(&session.crew_tints, slot), choice))
            .collect(),
    };
    let mut eyed = Choice::default();
    super::worldmap::relic_window(
        &ctx,
        world,
        local,
        &mut orders,
        &crew_name,
        &mates,
        &mut eyed,
    );
    // A rank-up asked for this frame, by Ctrl and a slot's key or by a
    // Ctrl-click on its box (task 123), for the player's own Bim — the
    // one place both go through.
    if let Some(asked) = rank_up_asked {
        let (order, line) = rank_up(world, local, asked);
        orders.extend(order);
        screen.log.extend(line);
    }
    // And the end of a fight: the site of this mission just cleared, with
    // what the fight earned and the way back to the ship.
    super::fightwon::fight_won_window(
        &ctx,
        &mut screen.fight,
        world,
        local,
        &mut orders,
        &crew_name,
    );
    // And the trader the crew are at (task 114): the whole visit is on the
    // map, and the Armory panel may be up beside it.
    let mut on_line = Choice::default();
    super::worldmap::trader_window(
        &ctx,
        world,
        local,
        &mut orders,
        &crew_name,
        &mates,
        &mut on_line,
    );
    eyed.line = on_line.line;
    online.say_choice(eyed);

    // What the HUD asked for: the orders now, off the world as it stands,
    // and the windows once it is let go of.
    let standing = world.standing_of(local);
    let map_asked = asks.contains(&TrayAsk::Map);
    let mut arm = None;
    for ask in asks {
        let (order, line) = match ask {
            TrayAsk::Map => (None, None),
            // The X key's own rule: the pointer armed, or a banner taken
            // up again.
            TrayAsk::Attack => {
                let (release, armed) = attack_key(standing, screen.aiming_attack, map_up);
                arm = Some(armed);
                match release {
                    Some(order) => orders_key(world, local, order),
                    None => (None, None),
                }
            }
            TrayAsk::Retreat => orders_key(world, local, world::Standing::Retreat),
            // The order the crew are under given again, which the world
            // reads as letting them go (`Standing::same_as`).
            TrayAsk::Follow if standing != world::Standing::Follow => {
                orders_key(world, local, standing)
            }
            TrayAsk::Follow => (None, None),
        };
        orders.extend(order);
        screen.log.extend(line);
    }
    if let Some(armed) = arm {
        screen.aiming_attack = armed;
        screen.aiming_move = false;
        screen.aiming_throw = None;
    }
    if map_asked && let Some(game) = &mut session.game {
        game.set_mode(if game.mode == ViewMode::Map {
            ViewMode::Ship
        } else {
            ViewMode::Map
        });
    }
    // A place picked on the list (the map rework): its star picked on the chart
    // — brought into view there if it was off it — and its system in the
    // system view.
    if let Some(star) = column.as_ref().and_then(|ask| ask.show)
        && let Some(chart) = &mut screen.galaxy
    {
        chart.inspect(star);
        screen.picked_star = Some(star);
        if let Some(s) = chart.galaxy.star(star) {
            let (x, y) = chart.preview.to_screen(s.position.x, s.position.y);
            let (w, h) = (chart.preview.width, chart.preview.height);
            let edge = 0.1 * w.min(h);
            if x < edge || y < edge || x > w - edge || y > h - edge {
                chart.preview.pan(w / 2.0 - x, h / 2.0 - y);
            }
        }
    }
    if focus_here
        && let Some(chart) = &mut screen.galaxy
        && let Some(s) = chart.here.and_then(|id| chart.galaxy.star(id))
    {
        let (x, y) = chart.preview.to_screen(s.position.x, s.position.y);
        let (w, h) = (chart.preview.width, chart.preview.height);
        chart.preview.pan(w / 2.0 - x, h / 2.0 - y);
    }
    if let Some(ask) = column
        && ask.close
        && let Some(game) = &mut session.game
    {
        game.set_mode(ViewMode::Ship);
    }
    // A portrait clicked picks that crew member as a click on it on the
    // deck would — a click at where it stands, through the seam — or, for
    // a player who is out, is the one watched; the player's own twice is
    // the character sheet.
    match portrait_press {
        Some(hud::PortraitPress::Sheet) => panels.sheet_open = true,
        Some(hud::PortraitPress::Pick(who)) if out => {
            if let Some(game) = session.game.as_mut()
                && who != local
                && game.world.aboard.room.is_alive(who as usize)
            {
                game.spectate = Some(who);
                game.set_follow(true);
            }
        }
        Some(hud::PortraitPress::Pick(who)) => {
            if let Some(room) = session.room_ref() {
                let at = room.bim_pos(who as usize);
                orders.push(Order::Crew(CrewOrder::Select {
                    x0: at.x,
                    y0: at.y,
                    x1: at.x,
                    y1: at.y,
                }));
            }
        }
        None => {}
    }
    // The inspect panel, now the room may be written through: the side
    // panel's own, scrolled where the column is shorter than it.
    if let Some((at, floor)) = side_at
        && let Some(room) = session.room()
        && panels.inspected(room).is_some()
    {
        egui::Area::new(egui::Id::new("game-side"))
            .fixed_pos(at)
            .order(egui::Order::Middle)
            .show(&ctx, |ui| {
                panel_frame().show(ui, |ui| {
                    egui::ScrollArea::vertical()
                        .id_salt("side")
                        .max_height((floor - at.y - 16.0).max(80.0))
                        .min_scrolled_height((floor - at.y - 16.0).max(80.0))
                        .show(ui, |ui| {
                            panels.side(ui, room, &name);
                        });
                });
            });
    }
    for order in orders.drain(..) {
        loading.order(&screen.net, session, order);
    }

    // The class section's, the sheet's and the deployable rows' (feature 74).
    for order in panels.deploy_orders.drain(..) {
        orders.push(match order {
            crate::crew::DeployOrder::PackUp(id) => Order::PackUp(id),
            crate::crew::DeployOrder::SetClass(class) => Order::SetClass(class),
            crate::crew::DeployOrder::RankUp { ability_slot } => Order::RankUp { ability_slot },
        });
    }
    for order in orders.drain(..) {
        loading.order(&screen.net, session, order);
    }

    if let Some(room) = session.room() {
        panels.menu(&ctx, room, &name);
    }
    // The mercenary the Hire row opened on: the walk over is the world's
    // to start, since where one of the station's people stands is a point
    // of its own room put through the station's frame.
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
                medic: offer.medic,
            })
        });
        // The menu's Carry row on a downed crewmate: the player's own Bim
        // walks over, and the carry goes the frame it is within reach —
        // or is given up with the world's reason once the walk stops
        // short, or the body no longer wants carrying.
        let own = panels.player as u32;
        if let Some(patient) = panels.carry_requested.take() {
            screen.carry_walk = Some((patient, 0));
            let at = world.aboard.room.bim_pos(patient as usize);
            orders.push(Order::Crew(CrewOrder::SendTo {
                who: own,
                x: at.x,
                y: at.y,
            }));
        }
        if let Some((patient, frames)) = screen.carry_walk {
            screen.carry_walk = None;
            match world.can_carry(own, patient) {
                Ok(()) => orders.push(Order::Carry(Some(patient))),
                Err(Refusal::OutOfReach)
                    if frames < CARRY_WALK_GRACE || world.aboard.room.is_walking(own as usize) =>
                {
                    screen.carry_walk = Some((patient, frames + 1));
                }
                Err(why) => screen.log.push(carry_refused(why)),
            }
        }
        // The Armory panel (task 113): on every screen of a run — the deck,
        // the map, the galaxy chart and the reward screen — read-only in a
        // mission.
        let armory = armory_of(world, local);
        panels.armory_window(&ctx, &armory);
        let room = &mut world.aboard.room;
        panels.hire_window(&ctx, room, &name);
        panels.end_frame(room);
    }
    // What the Armory panel and the rows asked for: gear moves through the
    // seam like every other change to the world.
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
        loading.order(&screen.net, session, order);
    }
    let mut allowed = Allowed::of(session.playing(), online.is_guest());
    allowed.paused = session
        .game
        .as_ref()
        .map(|game| game.requested(screen.net.slot) == Speed::Paused);
    let asked = settings_sheet(
        &ctx,
        &mut screen.sheet,
        &mut sounds.mix,
        &mut bindings,
        &mut screen.saves,
        allowed,
    );
    match asked {
        // Back to the start menu: out of the room with company, and the
        // world left as it is, unsaved.
        Some(Request::ToMenu) => {
            online.leave();
            next.set(Screen::Menu);
        }
        // The sheet's pause (task 154): an order, so a pause by anybody
        // is a pause for everybody, as Space's was.
        Some(Request::Pause(on)) => {
            let speed = if on { Speed::Paused } else { Speed::Real };
            loading.order(&screen.net, session, Order::Speed(speed));
        }
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
    {
        let _timed = crate::perf::scope(crate::perf::Phase::Render);
        session.render();
    }
    // The view is read only after `render`, which is what puts the camera on
    // the player's own Bim where this frame's steps left it: a view read
    // before it drew the Bim through last frame's camera — a step off,
    // every frame it walked, which shook it (task 144).
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
    let overlay_timed;
    if galaxy_up && let Some(chart) = &screen.galaxy {
        // The chart on the left of the canvas (task 135), beside the
        // system map, with a painter of its own clipped to it.
        let chart_painter = canvas_painter(&ctx, galaxy_rect);
        // The chart in place of the map: the lobby's picture, in pixels,
        // and the names of the star the ship is at and the one picked over
        // them, since the buffer holds no words.
        {
            let _timed = crate::perf::scope(crate::perf::Phase::Render);
            chart.paint(&mut screen.galaxy_list);
        }
        world_canvas.shapes(&ctx, galaxy_rect, View::PIXELS, screen.galaxy_list.shapes());
        // The ship's star named over its reticle, and the star it is heading
        // for under its own mark and tier tag (the second map rework), so two stars a
        // lane apart never write over each other. The one heading for says
        // what the trip there costs — `costs 2 days`.
        for (star, color, tag, lift) in [
            (chart.here, theme::YOURS, "here", -24.0),
            (chart.target, theme::HYPER, "heading", 36.0),
        ] {
            if let Some(s) = star.and_then(|id| chart.galaxy.star(id)) {
                let (x, y) = chart.preview.to_screen(s.position.x, s.position.y);
                let at = egui::pos2(galaxy_rect.min.x + x, galaxy_rect.min.y + y + lift);
                let mut words = format!("{} · {tag}", star_name(s.name));
                if star != chart.here
                    && let Some(game) = session.game.as_ref()
                {
                    let days = game.world.trip_days_in(&chart.galaxy, s.id);
                    words = format!("{words} · {}", trip_cost(days));
                }
                theme::name_over(&chart_painter, at, &words, color);
            }
        }
        // The Machine Heart named over its mark, or — off the chart — an
        // arrow at the chart's edge pointing the way to it.
        if let Some(s) = chart.heart.and_then(|id| chart.galaxy.star(id)) {
            let (x, y) = chart.preview.to_screen(s.position.x, s.position.y);
            heart_tag(&chart_painter, egui_rect(galaxy_rect), egui::pos2(x, y));
        }
        // Every star's tier under it (`World::system_tiers`), on a
        // small dark tag in the tier's colour: every star on the galaxy_rect
        // once the chart is zoomed in far enough that the tags do not
        // crowd, and before that the ship's own, the picked one and the
        // stars a lane away — the rings the chart draws round every star
        // past tier one say the rest at any zoom.
        if let Some(game) = session.game.as_ref() {
            let world = &game.world;
            let all = chart.preview.zoom_level() >= CHART_TIERS_ZOOM;
            let font = egui::FontId::proportional(CHART_TIER_TEXT);
            let bounds = egui_rect(galaxy_rect).expand(8.0);
            for s in &chart.galaxy.stars {
                let (low, high) = world.system_tiers(s.id, world.clock_minutes);
                let near = chart.here == Some(s.id)
                    || screen.picked_star == Some(s.id)
                    || chart.reachable.contains(&s.id);
                if !all && !near {
                    continue;
                }
                let (x, y) = chart.preview.to_screen(s.position.x, s.position.y);
                let at = egui::pos2(
                    galaxy_rect.min.x + x,
                    galaxy_rect.min.y + y + CHART_TIER_DROP,
                );
                if !bounds.contains(at) {
                    continue;
                }
                let colour = tier_colour(high);
                let galley =
                    chart_painter.layout_no_wrap(system_tier(low, high), font.clone(), colour);
                let tag = egui::Rect::from_center_size(
                    at + egui::vec2(0.0, galley.size().y / 2.0),
                    galley.size() + egui::vec2(6.0, 1.0),
                );
                chart_painter.rect_filled(tag, 3.0, theme::PANEL_DEEP.gamma_multiply(0.85));
                chart_painter.rect_stroke(
                    tag,
                    3.0,
                    egui::Stroke::new(1.0, colour.gamma_multiply(0.6)),
                    egui::StrokeKind::Inside,
                );
                chart_painter.galley(tag.center() - galley.size() / 2.0, galley, colour);
            }
        }
    }
    {
        // The world under the fog now; what goes over the fog — the
        // shots, the rings — once the fog is down (feature 97). Nothing
        // under the map, which is the galaxy chart alone.
        if !map_up {
            // At a station, its picture under everything (the planet's
            // ground is the painter's own) — behind the ready check too,
            // with nothing of the deck over it.
            if let Some(key) = session.game.as_ref().and_then(|g| g.backdrop()) {
                station_backdrops.paint(&mut world_canvas, &ctx, canvas, key);
            }
        }
        if !deck_hidden {
            world_canvas.shapes(&ctx, canvas, view, session.fog_split().0);
        }
        overlay_timed = crate::perf::scope(crate::perf::Phase::Overlay);
        // The smooth fog over the deck — what the crew do not see, and
        // the dark where no light reaches — as the room's light map,
        // through the ship's camera and heading like the crew's names.
        if !deck_hidden {
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
        if !deck_hidden {
            world_canvas.shapes(&ctx, canvas, view, session.fog_split().1);
        }
        // And the particles over the shots, simulated on the GPU: what the
        // rooms and the abilities spawned this frame goes on the ring
        // whether or not the deck is shown, so none pile up behind the map.
        let sprays = session.take_sprays();
        if !deck_hidden {
            world_canvas.particles(&ctx, canvas, view, bars_dt, &sprays);
        }
    }

    // And the red crosshair while the attack key has the pointer armed
    // (feature 84), in the same place for the same reason.
    // Over the deck the pointer is the crosshair and nothing else (task
    // 144): the system's cursor is hidden (`hide_the_cursor`) and the
    // reticle drawn over everything, the panels too — unless a window
    // wants the pointer: the trader, a relic to choose, the Esc sheet, the
    // map, the Armory or the character sheet.
    let windowed = deck_hidden
        || screen.sheet.is_some()
        || super::worldmap::trader_rect(&ctx).is_some()
        || super::worldmap::relic_rect(&ctx).is_some()
        || screen
            .panels
            .as_ref()
            .is_some_and(|p| p.armory_open || p.sheet_open);
    let inside = pointer
        .pos
        .is_some_and(|p| egui_rect(full).contains(egui::pos2(p.x, p.y)));
    screen.crosshair = !windowed && inside && session.game.is_some();
    if screen.crosshair
        && let Some(p) = pointer.pos
    {
        let top = ctx.layer_painter(egui::LayerId::new(
            egui::Order::Tooltip,
            egui::Id::new("crosshair"),
        ));
        let at = egui::pos2(p.x, p.y);
        if screen.aiming_attack || screen.aiming_move {
            // The red one while the attack key has the pointer armed
            // (feature 84).
            attack_cursor(&top, at);
        } else {
            let firing = screen.control.is_some_and(|((_, _, fire, _), _)| fire);
            // Grey past where a shot of the player's own Bim reaches
            // (`Game::shot_reach`): the reticle shows where it can hit.
            let beyond = here.is_some_and(|h| {
                let me = screen.net.slot as usize;
                session.room_ref().is_some_and(|room| {
                    me < room.crew_count() as usize
                        && room.shot_reach(me).is_some_and(|reach| {
                            let (rx, ry) = session.room_point(h.x, h.y);
                            let at = room.bim_pos(me);
                            (rx - at.x).hypot(ry - at.y) > reach
                        })
                })
            });
            let colour = if beyond { theme::AIM_OUT } else { theme::AIM };
            aim_cursor(&top, at, firing, colour, 1.0);
        }
    } else if (screen.aiming_attack || screen.aiming_move)
        && let Some(p) = on_canvas
    {
        attack_cursor(&painter, egui::pos2(p.x + canvas.min.x, p.y + canvas.min.y));
    }
    // The others' pointers and everybody's pings (Ctrl and a left click),
    // each in its player's colour, wherever it is in whichever view this
    // player has up: over the tile it is over on the deck, through the
    // ship's camera and heading like the names; the place in the system
    // shown (the same star only); the place on the galaxy chart. A
    // pointer over the trader's window or the relic choice is drawn by
    // the window (`worldmap::Mates`).
    {
        let chart_painter = canvas_painter(&ctx, galaxy_rect);
        let place = |spot: Spot| -> Option<(&egui::Painter, egui::Pos2)> {
            match spot {
                Spot::Deck(x, y) if !deck_hidden => {
                    let (x, y) = session.design_point_on_screen(x, y);
                    let at = view.to_canvas(Vec2::new(x, y)) + canvas.min;
                    Some((&painter, egui::pos2(at.x, at.y)))
                }
                Spot::Galaxy(x, y) if galaxy_up => screen.galaxy.as_ref().map(|chart| {
                    let (sx, sy) = chart.preview.to_screen(x, y);
                    (
                        &chart_painter,
                        egui::pos2(galaxy_rect.min.x + sx, galaxy_rect.min.y + sy),
                    )
                }),
                _ => None,
            }
        };
        // Over the deck another player's pointer is their crosshair, as
        // it is on their own screen, in their colour and closing in while
        // their trigger is held; on the chart it is the arrow.
        for (slot, spot) in online.others_pointing() {
            if let Some((on, at)) = place(spot) {
                let colour = slot_colour(&session.crew_tints, slot);
                if matches!(spot, Spot::Deck(..)) {
                    let firing = session
                        .game
                        .as_ref()
                        .is_some_and(|g| g.world.aboard.room.trigger_held(slot as usize));
                    aim_cursor(on, at, firing, colour, OTHERS_AIM_ALPHA);
                    theme::ghost_label(on, at + egui::vec2(12.0, 8.0), colour, &crew_name(slot));
                } else {
                    theme::ghost_pointer(on, at, colour, &crew_name(slot));
                }
            }
        }
        let me = online.my_slot();
        for ping in online.pings_now(now).to_vec() {
            let Some((on, at)) = place(ping.at) else {
                continue;
            };
            let name = if ping.slot == me {
                String::new()
            } else {
                crew_name(ping.slot)
            };
            theme::ping_mark(
                on,
                at,
                slot_colour(&session.crew_tints, ping.slot),
                &name,
                (now - ping.born) as f32,
                PING_SECONDS as f32,
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

    // The countdown over every downed body (task 120): a red ring over
    // the head emptying as its thirty seconds run out, the crew's and the
    // station's people's alike — a townsperson or a Manufacturer down is
    // on the same clock, and a player deciding whether to cross the deck
    // for a crewmate wants to see the one beside it too. The downed only:
    // nothing counts on a corpse. Drawn first, under the beams and the
    // banners, so a medic already working on one shows through.
    if !deck_hidden && let Some(game) = &session.game {
        let ring = |left: f32, (x, y): (f32, f32)| {
            let at = view.to_canvas(Vec2::new(x, y)) + canvas.min;
            theme::downed_ring(
                &painter,
                egui::pos2(at.x, at.y),
                view.scale,
                left / bims::health::DOWNED_SECONDS,
                &downed_seconds(left),
            );
        };
        let room = &game.world.aboard.room;
        for who in 0..game.world.aboard.crew_count() {
            if let Some(left) = room.down_left(who as usize)
                && let Some(at) = session.crew_on_screen(who)
            {
                ring(left, at);
                // And over the ring, while somebody's hands are on it, how
                // far the revive has got.
                if let Some((_, share)) = room.revive_share(who as usize) {
                    let p = view.to_canvas(Vec2::new(at.0, at.1)) + canvas.min;
                    theme::revive_bar(&painter, egui::pos2(p.x, p.y), view.scale, share);
                }
            }
        }
        // Who is in cover (`Game::cover_of`): a curved wall on the side
        // the cover is against and a shield at the shoulder — for the
        // crew, and for a station's people the crew can see.
        let (ox, oy) = (
            game.world.aboard.offset.x as f32,
            game.world.aboard.offset.y as f32,
        );
        for who in 0..game.world.aboard.crew_count() {
            let Some(threat) = room.cover_of(who as usize) else {
                continue;
            };
            let Some(at) = session.crew_on_screen(who) else {
                continue;
            };
            let a = view.to_canvas(Vec2::new(at.0, at.1)) + canvas.min;
            let (tx, ty) = session.design_point_on_screen(threat.x - ox, threat.y - oy);
            let t = view.to_canvas(Vec2::new(tx, ty)) + canvas.min;
            theme::cover_mark(
                &painter,
                egui::pos2(a.x, a.y),
                view.scale,
                egui::vec2(t.x - a.x, t.y - a.y),
            );
        }
        if let Some(residents) = &game.world.residents {
            let room = &residents.aboard.room;
            for who in 0..session.resident_count() {
                if room.cover_of(who as usize).is_none() {
                    continue;
                }
                if let Some(at) = session.resident_on_screen(who) {
                    let a = view.to_canvas(Vec2::new(at.0, at.1)) + canvas.min;
                    theme::cover_mark(&painter, egui::pos2(a.x, a.y), view.scale, egui::Vec2::ZERO);
                }
            }
        }
        if let Some(residents) = &game.world.residents {
            let room = &residents.aboard.room;
            let crew_room = &game.world.aboard.room;
            for who in 0..session.resident_count() {
                if let Some(left) = room.down_left(who as usize)
                    && let Some(at) = session.resident_on_screen(who)
                {
                    ring(left, at);
                    // A townsperson a crew member is bringing round with
                    // the medkit: the crew's room has the hands on it.
                    if let Some((_, share)) =
                        crew_room.revive_share(bims::game::GUEST + who as usize)
                    {
                        let p = view.to_canvas(Vec2::new(at.0, at.1)) + canvas.min;
                        theme::revive_bar(&painter, egui::pos2(p.x, p.y), view.scale, share);
                    }
                }
            }
        }
    }

    // Every heal beam on the deck (feature 76): a line from the medic to
    // each crew member it holds, and a mark on a body a surge is running
    // on, in the beam's own green.
    if !deck_hidden && let Some(game) = &session.game {
        let crew = game.world.aboard.crew_count();
        for medic in 0..crew {
            let Some(from) = session.crew_on_screen(medic) else {
                continue;
            };
            for patient in game.world.patients_of(medic) {
                let Some(to) = session.patient_on_screen(patient) else {
                    continue;
                };
                let a = view.to_canvas(Vec2::new(from.0, from.1)) + canvas.min;
                if patient == medic {
                    theme::self_beam(&painter, egui::pos2(a.x, a.y), view.scale);
                    continue;
                }
                let b = view.to_canvas(Vec2::new(to.0, to.1)) + canvas.min;
                theme::heal_beam(
                    &painter,
                    egui::pos2(a.x, a.y),
                    egui::pos2(b.x, b.y),
                    view.scale,
                );
            }
        }
        // And every Healing Sentry's (task 127): a thin line in the beam's
        // green from the sentry to each crew member it heals.
        for (from, to) in session.healing_lines_on_screen() {
            let a = view.to_canvas(Vec2::new(from.0, from.1)) + canvas.min;
            let b = view.to_canvas(Vec2::new(to.0, to.1)) + canvas.min;
            theme::healing_line(
                &painter,
                egui::pos2(a.x, a.y),
                egui::pos2(b.x, b.y),
                view.scale,
            );
        }
        // And a surge on a body (nothing sets one since the relics were
        // rebuilt; the medic's own went in task 130).
        for who in 0..crew {
            if !game.world.aboard.room.is_surging(who as usize) {
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
            let Some((x, y)) = session.patient_on_screen(floater.who) else {
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
        // And every hit landed, a small red number rising off whoever
        // took it — enemy or crew — a critical one bigger, in gold, with
        // a mark; gone in well under a second.
        screen.hits.retain(|h| now - h.born < HIT_SECONDS);
        for hit in &mut screen.hits {
            let on = if hit.resident {
                session.resident_on_screen(hit.who)
            } else {
                session.crew_on_screen(hit.who)
            };
            if let Some(p) = on {
                hit.at = Some(p);
            }
            let Some((x, y)) = hit.at else {
                continue;
            };
            let at = view.to_canvas(Vec2::new(x, y)) + canvas.min;
            theme::hit_number(
                &painter,
                egui::pos2(at.x + hit.nudge * 9.0, at.y),
                view.scale,
                &crate::names::hit_damage(hit.damage, hit.crit),
                hit.crit,
                ((now - hit.born) / HIT_SECONDS) as f32,
            );
        }
        // And what each enemy down paid, rising off where it fell: the
        // money in gold over the experience in blue, gone within a
        // second.
        screen.rewards.retain(|r| now - r.born < REWARD_SECONDS);
        for reward in &mut screen.rewards {
            if let Some(p) = session.resident_on_screen(reward.who) {
                reward.at = Some(p);
            }
            let Some((x, y)) = reward.at else {
                continue;
            };
            let at = view.to_canvas(Vec2::new(x, y)) + canvas.min;
            theme::reward_numbers(
                &painter,
                egui::pos2(at.x, at.y),
                view.scale,
                &crate::names::reward_money(reward.money),
                &crate::names::reward_xp(reward.xp),
                ((now - reward.born) / REWARD_SECONDS) as f32,
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
        // The tank's (task 155) are drawn on the body by the room — the
        // Riot Shield's plate before him, the Reflect Barrier's ring of
        // thorns, and the Bastion's shield on everybody it reached.
        let t = shipdesign::TILE as f32;
        // And the commander (feature 78): the aura's radius round him,
        // a ring under every Bim it lifts — a player's own included.
        // A crew member, or a site's defender a medic's key reaches.
        let on_screen = |who: u32| {
            session.patient_on_screen(who).map(|(x, y)| {
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
            // A Battle Cry called: a short gold ring running out to its
            // reach over its first moments.
            if game.world.is_crying(who) {
                let run = game.world.battle_cry_seconds(who) - game.world.battle_cry_left(who);
                let shown = (run / BATTLE_CRY_RING_SECONDS) as f32;
                if shown < 1.0 {
                    theme::battle_cry_ring(
                        &painter,
                        at,
                        world::class::BATTLE_CRY_TILES * t * view.scale,
                        shown,
                    );
                }
            }
        }
        // And the medic (task 153): his Healing Circle round him while it
        // is on — a soft green floor, its rim breathing — and every Heal
        // Drone in the air, with its thin green line down to the Bim it
        // hovers over.
        for who in 0..crew {
            if !game.world.is_circling(who) {
                continue;
            }
            let Some(at) = on_screen(who) else {
                continue;
            };
            let radius = game.world.healing_circle_radius(who) * t * view.scale;
            theme::healing_circle(&painter, at, radius, now as f32);
        }
        for (from, patient, share) in session.drones_on_screen() {
            let p = view.to_canvas(Vec2::new(from.0, from.1)) + canvas.min;
            let at = egui::pos2(p.x, p.y);
            if let Some((x, y)) = patient {
                let q = view.to_canvas(Vec2::new(x, y)) + canvas.min;
                theme::healing_line(&painter, at, egui::pos2(q.x, q.y), view.scale);
            }
            theme::heal_drone(&painter, at, view.scale, now as f32, share);
        }
        // A Bim a rally runs over (feature 86), the commander himself
        // included (task 129).
        for who in 0..crew {
            if game.world.rally_reaching(who).is_none() {
                continue;
            }
            if let Some(at) = on_screen(who) {
                theme::rallied_mark(&painter, at, view.scale);
            }
        }
        // And the caller of it (feature 91): two chevrons over his head,
        // since his own rig and his selection ring are in the way of
        // anything small drawn at the body.
        for who in 0..crew {
            if !game.world.is_rallying(who) {
                continue;
            }
            if let Some(at) = on_screen(who) {
                theme::rally_call(&painter, at, view.scale);
            }
        }
        // And every reinforcement (task 129): a small chevron over its
        // head in the colour of the commander who brought it.
        for r in &game.world.reinforcements {
            if !game.world.aboard.room.is_alive(r.who as usize) {
                continue;
            }
            let Some(at) = on_screen(r.who) else {
                continue;
            };
            let tint = session
                .crew_tints
                .get(r.by as usize)
                .copied()
                .unwrap_or_else(|| bims::character::Tint::from_code(r.by as u8));
            let (red, green, blue) = tint.rgb();
            let colour = egui::Color32::from_rgb(
                (red * 255.0) as u8,
                (green * 255.0) as u8,
                (blue * 255.0) as u8,
            );
            theme::reinforcement_mark(&painter, at, view.scale, colour);
        }
        // And whom the ability box under the pointer would reach: the
        // ring is the answer to "who does this cast take in".
        for &who in &cast_reaches {
            if let Some(at) = on_screen(who) {
                theme::affected_ring(&painter, at, view.scale);
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

    // A throw being aimed (feature 75; its reach since the ability range
    // indicators): the throw's reach round the player's own Bim, very
    // faint, in its class's colour — a click past it or behind a wall
    // walks out to throw — and the burst's radius round the tile under
    // the pointer, in the throw's colour where it would be thrown,
    // walked out to or not, and the refusal's where it would not.
    if !deck_hidden
        && let Some(throw) = screen.aiming_throw
        && let Some(game) = &session.game
        && let Some((x, y)) = session.crew_on_screen(screen.net.slot)
    {
        let t = shipdesign::TILE as f32;
        let at = view.to_canvas(Vec2::new(x, y)) + canvas.min;
        let slot = screen.net.slot;
        let range = match throw {
            Throw::StunShot => game.world.stun_shot_range(slot).unwrap_or(0.0),
            _ => game.world.throw_range(slot, throw.satchel()),
        };
        theme::reach_ring(
            &painter,
            egui::pos2(at.x, at.y),
            range * t * view.scale,
            throw.glyph().colour(),
        );
    }
    if !deck_hidden
        && let Some((tiles, glyph)) = screen.held_reach
        && let Some((x, y)) = session.crew_on_screen(screen.net.slot)
    {
        let at = view.to_canvas(Vec2::new(x, y)) + canvas.min;
        let radius = tiles * shipdesign::TILE as f32 * view.scale;
        theme::reach_ring(&painter, egui::pos2(at.x, at.y), radius, glyph.colour());
    }
    // A Stun Shot's ring sits where it would come down with nobody in its
    // way — short of a wall, within the weapon's reach — while it is
    // aimed and on through its charge, so the player sees where it goes.
    let stun_aim = session.game.as_ref().and_then(|game| {
        let slot = screen.net.slot;
        match (screen.aiming_throw, screen.throw_aim) {
            (Some(Throw::StunShot), aim) => aim,
            (None, _) => game.world.stun_shot_aim(slot),
            _ => None,
        }
        .map(|tile| (Throw::StunShot, tile))
    });
    if !deck_hidden
        && let Some((throw, tile)) = stun_aim.or(screen.aiming_throw.zip(screen.throw_aim))
        && let Some(game) = &session.game
    {
        let t = shipdesign::TILE as f32;
        let slot = screen.net.slot;
        let (ox, oy) = (
            game.world.aboard.offset.x as f32,
            game.world.aboard.offset.y as f32,
        );
        let centre = Vec2::new((tile.0 as f32 + 0.5) * t, (tile.1 as f32 + 0.5) * t);
        let (centre, radius, ok) = match throw {
            Throw::StunShot => match game.world.stun_shot_landing(slot, tile) {
                Some(at) => (
                    Vec2::new(at.x, at.y),
                    game.world.stun_shot_radius(slot),
                    true,
                ),
                None => (centre, game.world.stun_shot_radius(slot), false),
            },
            _ => (
                centre,
                if throw.satchel() {
                    game.world.satchel_blast(slot).1
                } else {
                    game.world.grenade_radius(slot)
                },
                matches!(
                    game.world.can_throw_now(slot, throw.satchel(), tile),
                    Ok(()) | Err(Refusal::OutOfThrowRange | Refusal::NoLineToTile)
                ),
            ),
        };
        let (x, y) = session.design_point_on_screen(centre.x - ox, centre.y - oy);
        let at = view.to_canvas(Vec2::new(x, y)) + canvas.min;
        theme::burst_ring(
            &painter,
            egui::pos2(at.x, at.y),
            radius * t * view.scale,
            ok,
        );
    }

    // The health bar over every body standing (task 137), under the names:
    // read off the rooms here, after the step, so a bar stays over its body
    // as it walks.
    if !deck_hidden && let Some(game) = &session.game {
        screen.bars.update(game, bars_dt);
        screen.bars.paint(&painter, view.scale, |(x, y)| {
            let at = view.to_canvas(Vec2::new(x, y)) + canvas.min;
            egui::pos2(at.x, at.y)
        });
    }

    // The crew's names, over their heads, where the ship says each Bim
    // landed — the same camera the shapes went through, so a name stays over
    // its head as the ship turns.
    if !deck_hidden {
        for who in 0..crew_count {
            if let Some((x, y)) = session.crew_on_screen(who) {
                let at = view.to_canvas(Vec2::new(x, y)) + canvas.min;
                let at = egui::pos2(
                    at.x,
                    at.y - theme::NAME_LIFT * view.scale - crate::healthbars::name_room(view.scale),
                );
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
                let at = egui::pos2(
                    at.x,
                    at.y - theme::NAME_LIFT * view.scale - crate::healthbars::name_room(view.scale),
                );
                // A machine wears its kind, not a name: it is not
                // somebody (feature 83).
                let label = match session.resident_droid(who) {
                    Some(kind) => crate::names::droid_name(kind).to_string(),
                    // And a Manufacturer its faction (feature 109).
                    None if session.resident_manufacturer(who) => {
                        crate::names::MANUFACTURER_NAME.to_string()
                    }
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
    // The black after a trip, over everything on the canvas: held while
    // the place is laid out, then lifted. Real seconds, not the world's: a
    // pause is not a longer night.
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
/// The world map's galaxy chart (the galaxy-only map): the whole canvas
/// but what the list's column takes while it is popped out — and all of
/// it while the column is retracted, its tab lying over the chart.
fn chart_rect(full: crate::shapes::Rect, column_w: f32) -> crate::shapes::Rect {
    let right = if column_w > 0.0 {
        full.max.x - column_w - 2.0 * MARGIN
    } else {
        full.max.x
    };
    crate::shapes::Rect::new(full.min, Vec2::new(right.max(full.min.x + 1.0), full.max.y))
}

/// What a `CrewOrder::Control` says (task 144): the walk's angle code,
/// `None` with no key down, the aim's, the trigger, and the sprint (task
/// 150).
type CrewOrderControl = (Option<u16>, u16, bool, bool);

/// Seconds at the least between two control orders that change the aim
/// alone (task 144): every order is a command every player's copy of the
/// world applies, and a pointer moves every frame.
const CONTROL_EVERY: f64 = 0.05;

/// How quickly walking eases a middle drag's shove off the camera, a
/// second (task 144): the shove falls to `e^(-rate t)` of itself, so a
/// third of a second takes most of it.
const RECENTRE_RATE: f32 = 8.0;

/// How far the aim has to have turned, in angle codes, to be said again
/// on its own: a fifth of a degree.
const AIM_STEP: u16 = 36;

/// Whether the keys, the pointer and the trigger are worth a control
/// order now (task 144): the first, a walk, a trigger or a sprint changed — at
/// once, since those are what a body does — or the aim turned past
/// [`AIM_STEP`] at least [`CONTROL_EVERY`] after the last.
fn control_due(last: Option<(CrewOrderControl, f64)>, now: CrewOrderControl, time: f64) -> bool {
    let Some(((walk, aim, fire, sprint), at)) = last else {
        return true;
    };
    if walk != now.0 || fire != now.2 || sprint != now.3 {
        return true;
    }
    let turned = aim.wrapping_sub(now.1).min(now.1.wrapping_sub(aim));
    turned >= AIM_STEP && time - at >= CONTROL_EVERY
}

/// How see-through another player's crosshair is: less than their arrow
/// on the chart, since it is small and wants to read on a dark deck.
const OTHERS_AIM_ALPHA: f32 = 0.85;

/// The pointer over the deck (task 144): the aim's reticle where the
/// system's cursor was — a ring broken into four with a dot in the
/// middle, each stroke over a dark one so it reads on the deck and the
/// void alike, in the crew's cyan, and closing in while the trigger is
/// held. Another player's is the same in their `colour`, as see-through
/// as `alpha` says.
fn aim_cursor(
    painter: &egui::Painter,
    at: egui::Pos2,
    firing: bool,
    colour: egui::Color32,
    alpha: f32,
) {
    let r = if firing { 8.0 } else { 10.0 };
    let gap = 0.42;
    for (width, color) in [
        (
            4.0,
            egui::Color32::from_black_alpha(200).gamma_multiply(alpha),
        ),
        (1.8, colour.gamma_multiply(alpha)),
    ] {
        for quarter in 0..4 {
            let from = quarter as f32 * std::f32::consts::FRAC_PI_2 + gap;
            let to = from + std::f32::consts::FRAC_PI_2 - 2.0 * gap;
            let points: Vec<egui::Pos2> = (0..=6)
                .map(|k| {
                    let a = from + (to - from) * k as f32 / 6.0;
                    at + egui::vec2(a.cos(), a.sin()) * r
                })
                .collect();
            painter.add(egui::Shape::line(points, egui::Stroke::new(width, color)));
            let a = from - gap;
            let (c, s) = (a.cos(), a.sin());
            painter.line_segment(
                [
                    at + egui::vec2(c, s) * (r - 3.0),
                    at + egui::vec2(c, s) * (r + 5.0),
                ],
                egui::Stroke::new(width, color),
            );
        }
    }
    painter.circle_filled(at, 1.6, colour.gamma_multiply(alpha));
}

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

/// A door's leaves starting to slide, open or shut: what is heard only of
/// a door the crew see.
fn is_door_slide(cue: bims::cue::Cue) -> bool {
    matches!(cue, bims::cue::Cue::DoorOpens | bims::cue::Cue::DoorShuts)
}

/// Where the Machine Heart is on the galaxy chart: its name in the enemy's
/// red over its mark when the star is on the chart, and when it is not, an
/// arrow just inside the chart's edge on the line from the middle to it,
/// with the name beside it — so a pan or a zoom never hides the end of
/// the run. `at` is the star in the chart's own pixels.
fn heart_tag(painter: &egui::Painter, chart: egui::Rect, at: egui::Pos2) {
    let star = chart.min + at.to_vec2();
    if chart.shrink(4.0).contains(star) {
        heart_label(painter, star - egui::vec2(0.0, 32.0));
        return;
    }
    let inner = chart.shrink(34.0);
    if inner.width() < 100.0 || inner.height() < 20.0 {
        return;
    }
    let from = chart.center();
    let way = star - from;
    // How far along the line from the middle the inner edge is.
    let reach = |d: f32, half: f32| {
        if d.abs() < 1e-3 {
            f32::MAX
        } else {
            half / d.abs()
        }
    };
    let t = reach(way.x, inner.width() / 2.0).min(reach(way.y, inner.height() / 2.0));
    let tip = from + way * t.min(1.0);
    let dir = way.normalized();
    let side = egui::vec2(-dir.y, dir.x);
    let back = tip - dir * 16.0;
    painter.add(egui::Shape::convex_polygon(
        vec![tip, back + side * 9.0, back - side * 9.0],
        theme::ATTACK,
        egui::Stroke::new(1.5, theme::PANEL_DEEP),
    ));
    // The name behind the arrow, kept inside the chart.
    let label = back - dir * 8.0 + egui::vec2(0.0, -8.0);
    let label = egui::pos2(
        label.x.clamp(inner.min.x + 50.0, inner.max.x - 50.0),
        label.y.clamp(inner.min.y + 14.0, inner.max.y),
    );
    heart_label(painter, label);
}

/// The Machine Heart's name on a dark tag ringed in the enemy's red, its
/// bottom middle at `at` — a tag rather than the stars' outlined names, so
/// it reads over the red crosses and blades crowding round the origin.
fn heart_label(painter: &egui::Painter, at: egui::Pos2) {
    let font = egui::FontId::proportional(theme::NAME_SIZE + 1.0);
    let galley = painter.layout_no_wrap(HEART_NAME.to_string(), font, theme::ATTACK);
    let tag = egui::Rect::from_center_size(
        at - egui::vec2(0.0, galley.size().y / 2.0 + 3.0),
        galley.size() + egui::vec2(12.0, 4.0),
    );
    painter.rect_filled(tag, 4.0, theme::PANEL_DEEP.gamma_multiply(0.94));
    painter.rect_stroke(
        tag,
        4.0,
        egui::Stroke::new(1.5, theme::ATTACK),
        egui::StrokeKind::Inside,
    );
    painter.galley(tag.center() - galley.size() / 2.0, galley, theme::ATTACK);
}

/// The galaxy chart's word on the star looked at: its one mission as two
/// pictures — crossed blades or a shield, a ringed planet or a wheel of a
/// station — and its kind and place, `DEFEND station`. Faded once the
/// fight there is over; nothing for a trader's star.
fn chart_mission_line(ui: &mut egui::Ui, mission: &world::run::StarMission) {
    if mission.kind == world::SiteKind::Trader {
        return;
    }
    let colour = if mission.cleared {
        theme::MUTED
    } else {
        theme::site_kind_colour(mission.kind)
    };
    let planet = world::surface_body(mission.site.station).is_some();
    let place = if mission.cleared {
        theme::MUTED
    } else {
        ui.visuals().strong_text_color()
    };
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 6.0;
        let size = egui::vec2(20.0, 20.0);
        let (rect, _) = ui.allocate_exact_size(size, egui::Sense::hover());
        let painter = ui.painter();
        match mission.kind {
            world::SiteKind::Attack => blades_icon(painter, rect.center(), 15.0, colour),
            _ => shield_icon(painter, rect.center(), 16.0, colour),
        }
        let (rect, _) = ui.allocate_exact_size(size, egui::Sense::hover());
        let painter = ui.painter();
        if planet {
            planet_icon(painter, rect.center(), 19.0, place);
        } else {
            station_icon(painter, rect.center(), 16.0, place);
        }
        let word = format!(
            "{} {}",
            site_kind_word(mission.kind),
            site_place_word(mission.site.station)
        );
        ui.label(egui::RichText::new(word).strong().color(colour));
    });
}

/// A shield, `size` tall, centred on `at`: a flat top, straight sides
/// meeting in a point below and a bar down its middle — the chart's mark
/// for a defence (`lobby::preview`'s `shield`), drawn again in egui.
fn shield_icon(painter: &egui::Painter, at: egui::Pos2, size: f32, colour: egui::Color32) {
    let (w, h) = (size * 0.42, size / 2.0);
    let points = vec![
        at + egui::vec2(-w, -h),
        at + egui::vec2(w, -h),
        at + egui::vec2(w, h * 0.15),
        at + egui::vec2(0.0, h),
        at + egui::vec2(-w, h * 0.15),
    ];
    painter.add(egui::Shape::closed_line(
        points,
        egui::Stroke::new(1.8, colour),
    ));
    painter.line_segment(
        [at + egui::vec2(0.0, -h), at + egui::vec2(0.0, h)],
        egui::Stroke::new(1.4, colour),
    );
}

/// Two blades crossed, `size` across, centred on `at`, a guard across each
/// near its hilt — the chart's mark for an attack (`lobby::preview`'s
/// `blades`).
fn blades_icon(painter: &egui::Painter, at: egui::Pos2, size: f32, colour: egui::Color32) {
    let r = size / 2.0;
    let stroke = egui::Stroke::new(1.8, colour);
    for s in [1.0f32, -1.0] {
        let hilt = at + egui::vec2(-s * r, r);
        let point = at + egui::vec2(s * r, -r);
        painter.line_segment([hilt, point], stroke);
        let guard = hilt + (point - hilt) * 0.27;
        let g = size * 0.2;
        painter.line_segment(
            [guard + egui::vec2(-g, -s * g), guard + egui::vec2(g, s * g)],
            stroke,
        );
    }
}

/// A planet, `size` across with its ring, centred on `at`: a disc with a
/// tilted ring, the ring's far half behind the disc and its near half
/// across it.
fn planet_icon(painter: &egui::Painter, at: egui::Pos2, size: f32, colour: egui::Color32) {
    let (rx, ry) = (size / 2.0, size * 0.18);
    let tilt = -0.35f32;
    let (sin, cos) = tilt.sin_cos();
    let ring = |from: f32, to: f32| -> Vec<egui::Pos2> {
        (0..=12)
            .map(|i| {
                let t = from + (to - from) * i as f32 / 12.0;
                let (x, y) = (rx * t.cos(), ry * t.sin());
                at + egui::vec2(x * cos - y * sin, x * sin + y * cos)
            })
            .collect()
    };
    let stroke = egui::Stroke::new(1.4, colour);
    let pi = std::f32::consts::PI;
    painter.add(egui::Shape::line(ring(pi, 2.0 * pi), stroke));
    painter.circle_filled(at, size * 0.29, colour);
    // A gap of the panel's dark (`theme::panel_frame`) round the near
    // half, so it reads in front.
    painter.add(egui::Shape::line(
        ring(0.0, pi),
        egui::Stroke::new(2.6, egui::Color32::from_rgb(20, 29, 25)),
    ));
    painter.add(egui::Shape::line(ring(0.0, pi), stroke));
}

/// A wheel of a station, `size` across, centred on `at`: a rim, a hub and
/// four spokes.
fn station_icon(painter: &egui::Painter, at: egui::Pos2, size: f32, colour: egui::Color32) {
    let r = size / 2.0 - 1.0;
    let stroke = egui::Stroke::new(1.6, colour);
    painter.circle_stroke(at, r, stroke);
    painter.circle_filled(at, size * 0.14, colour);
    for (x, y) in [(1.0f32, 0.0f32), (0.0, 1.0), (-1.0, 0.0), (0.0, -1.0)] {
        let d = egui::vec2(x, y);
        painter.line_segment(
            [at + d * size * 0.14, at + d * r],
            egui::Stroke::new(1.2, colour),
        );
    }
    // The docking arm: a short bar off the rim's top right.
    let arm = egui::vec2(1.0, -1.0) * std::f32::consts::FRAC_1_SQRT_2;
    painter.line_segment([at + arm * r, at + arm * (r + 2.5)], stroke);
}

/// A tier's colour on the galaxy chart: the muted grey for one, the
/// caution colour for two, the attack red for three.
fn tier_colour(tier: bims::combat::Tier) -> egui::Color32 {
    match tier {
        bims::combat::Tier::One => theme::MUTED,
        bims::combat::Tier::Two => theme::CAUTION,
        bims::combat::Tier::Three => theme::ATTACK,
    }
}

/// The Armory panel's reading (task 113), off the world as it stands: a
/// column a crew member — its portrait as the HUD draws it, its
/// loadout, whether the player looking may change it, and the offers
/// standing to it and from it — the armory, the money, the keys, and
/// whether a mission is running, when only a Bim inside the ship may be
/// changed and no offer is made.
fn armory_of(world: &world::World, local: u32) -> crate::crew::ArmoryView {
    let portraits = hud::portraits_of(world, local, None);
    let room = &world.aboard.room;
    let offers = &world.holdings.offers;
    let columns = portraits
        .into_iter()
        .map(|portrait| {
            let who = portrait.who;
            let offers_in = offers
                .iter()
                .filter(|o| o.to == who)
                .filter_map(|o| {
                    world
                        .worn_on(o.from, o.slot)
                        .map(|item| (o.from, o.slot, item, crew_name(o.from)))
                })
                .collect();
            let offers_out = offers
                .iter()
                .filter(|o| o.from == who)
                .map(|o| (o.slot, o.to, crew_name(o.to)))
                .collect();
            crate::crew::ArmoryColumn {
                who,
                gear: room.gear(who as usize),
                may_change: world.may_change_now(local, who),
                offers_in,
                offers_out,
                portrait,
            }
        })
        .collect();
    crate::crew::ArmoryView {
        local,
        columns,
        armory: world.holdings.armory.clone(),
        money: world.share_of(local),
        locked: world.in_mission(),
    }
}

/// What is within reach of crew member `who`, nearest first, for the
/// panels' nearby strip: the engineer's deployables to pack up. Nothing
/// is a container and nothing is looted since task 113.
fn nearby_of(session: &Session, who: usize, _name: &dyn Fn(u32) -> String) -> Vec<Near> {
    let Some(game) = session.game.as_ref() else {
        return Vec::new();
    };
    let world = &game.world;
    let room = &world.aboard.room;
    let at = room.bim_pos(who);
    let mut found: Vec<(f32, Near)> = Vec::new();
    // The engineer's deployables within reach (feature 74): a row to
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

/// The picture in an ability box (feature 80): the ability's own, in its
/// class's colours (`ability_icons`), or none for a slot the class has
/// nothing on, an empty frame.
type Mark = Option<Glyph>;

/// One of the two boxes at the foot of the screen (feature 80): what one
/// of the class's own keys does, how many uses are left, and whether it
/// could be pressed now. Read off the world every frame — nothing here
/// is kept between frames, the way the crew panel's rows are not.
struct AbilityBox {
    /// The key bound to it, spelt as the Controls page spells it.
    key: String,
    name: &'static str,
    /// What it does, in a line (Dota 2's way).
    tip: String,
    /// Its numbers under that line, a row each, every rank's at once
    /// for a ranked ability.
    stats: Vec<crate::names::Stat>,
    /// The line at the foot: the rank it is at and the level the next
    /// wants. Empty for a box of no ranks.
    foot: String,
    mark: Mark,
    /// How many are left: kits or grenades in the pack, beams free to
    /// link, how near to pick up. `None` where nothing is counted — a wall
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
    /// How charged it is, nought to one: a bar along the foot. Nothing has
    /// one since the medic's surge went (task 130); kept for a charge to come.
    charge: Option<f32>,
    /// Whether it is running now: a shot charging, the shield up, a beam held, a
    /// barrier, a rally, a body in the arms.
    on: bool,
    /// Out of stock — no charge left. Told from
    /// a count of nought that is not a stock (no beam free to link, an
    /// nobody near to carry), which does not stop the key.
    short: bool,
    /// The level it is learnt at, where the crew member is not there yet.
    locked: Option<u8>,
    /// The key this box is for (feature 86), so the frame can ask the
    /// world **who the cast would reach** while the pointer rests on it.
    /// `None` for a box no key casts, which since task 120 took the
    /// medicine's two away is none.
    action: Option<Action>,
    /// The rank bought of a ranked ability and its top (task 124): the pips
    /// under the box. `None` for a box past the four slots.
    rank: Option<(u8, u8)>,
    /// Whether a skill point could buy its next rank now: the "+" in the
    /// corner.
    plus: bool,
    /// Not learnt: a ranked ability at rank nought, which the key is not
    /// taken for.
    unlearnt: bool,
}

/// What one box shows, short of its key and its words: what
/// `ability_boxes` works out for each key. See [`AbilityBox`] for each
/// field.
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

impl AbilityBox {
    /// An empty ability slot (task 123): its key and nothing else. What
    /// the tests lay a row of boxes out with; every class fills its four
    /// slots since task 139.
    #[cfg(test)]
    fn empty(key: String, action: Action) -> AbilityBox {
        AbilityBox {
            key,
            name: "",
            tip: String::new(),
            stats: Vec::new(),
            foot: String::new(),
            mark: None,
            count: None,
            cooldown: 0.0,
            cooldown_whole: 0.0,
            recharge: None,
            charge: None,
            on: false,
            short: false,
            locked: None,
            action: Some(action),
            rank: None,
            plus: false,
            unlearnt: false,
        }
    }

    /// Whether the key would be taken now, as far as the box can tell:
    /// the level reached, out of the cooldown, something left to spend.
    /// What the world says when the key is actually pressed is
    /// `class_key`'s, and it knows about the pointer as well.
    fn ready(&self) -> bool {
        self.locked.is_none() && !self.unlearnt && self.cooldown <= 0.0 && !self.short
    }
}

/// Which of the class's two actions an ability slot does (task 123):
/// the first slot the primary, the third the secondary, and the second
/// and fourth nothing, being empty for every class so far.
fn slot_action(action: Action) -> Option<bool> {
    match action {
        Action::Ability1 => Some(true),
        Action::Ability3 => Some(false),
        _ => None,
    }
}

/// A rank-up asked for (task 123): the ability slot, nought to three,
/// whose rank the player's own Bim is to raise. Ctrl and the slot's key
/// and Ctrl and a click on its box are the same request, and both go
/// through [`rank_up`].
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
struct RankUp {
    slot: usize,
}

/// The rank-up Ctrl and a slot's key asked for among `events`.
fn rank_up_by_key(
    keys: &Keys,
    events: &[egui::Event],
    modifiers: egui::Modifiers,
) -> Option<RankUp> {
    keys.rank_up_asked(events, modifiers)
        .and_then(Action::ability_slot)
        .map(|slot| RankUp { slot })
}

/// The rank-up a click on the box for `action` asked for: a slot's box
/// clicked with Ctrl held, and nothing for a plain click or for any other
/// box — the carry.
fn rank_up_by_click(action: Option<Action>, clicked: bool, ctrl: bool) -> Option<RankUp> {
    action
        .filter(|_| clicked && ctrl)
        .and_then(Action::ability_slot)
        .map(|slot| RankUp { slot })
}

/// What a rank-up of the player's own Bim's ability does: the order to
/// send and the log's line, `class_key`'s shape. The one place a rank-up
/// asked by key or by click goes (task 123), and what it asks is the
/// world's own rule (task 124): the rank bought, or the log says why not.
fn rank_up(world: &world::World, slot: u32, asked: RankUp) -> (Option<Order>, Option<String>) {
    let ability_slot = u8::try_from(asked.slot).unwrap_or(u8::MAX);
    match world.can_rank_up(slot, ability_slot) {
        Ok(_) => (
            Some(Order::RankUp {
                ability_slot: u32::from(ability_slot),
            }),
            None,
        ),
        Err(why) => (None, Some(crate::names::rank_refused(why))),
    }
}

/// A key of a ranked kit (task 124), by slot: the soldier's Q throws a
/// grenade and E charges a Stun Shot (`class_key`), C is Weak
/// Spot and does nothing when pressed, and R goes on a Rampage. `under`
/// is the crew member under the pointer, for the medic's beam.
fn ranked_key(
    world: &world::World,
    slot: u32,
    action: Action,
    tile: Option<(i32, i32)>,
    under: Option<u32>,
) -> (Option<Order>, Option<String>) {
    match (world.class_of(slot), action) {
        (world::Class::Soldier, Action::Ability1) => class_key(world, slot, true, tile, None),
        (world::Class::Soldier, Action::Ability3) => class_key(world, slot, false, tile, None),
        (world::Class::Soldier, Action::Ability4) => match world.can_rampage(slot) {
            Ok(()) => (Some(Order::Rampage), None),
            Err(why) => (None, Some(crate::names::rampage_refused(why))),
        },
        (world::Class::Engineer, action) => match action.ability_slot() {
            Some(ability) => engineer_key(world, slot, ability as u8, tile),
            None => (None, None),
        },
        // The commander's (task 129): Q calls a Battle Cry, C a medic in,
        // E a Rally and R his reinforcements in.
        (world::Class::Commander, Action::Ability1) => match world.can_battle_cry(slot) {
            Ok(()) => (Some(Order::BattleCry), None),
            Err(why) => (None, Some(crate::names::battle_cry_refused(why))),
        },
        (world::Class::Commander, Action::Ability2) => match world.can_medivac(slot) {
            Ok(()) => (Some(Order::Medivac), None),
            Err(why) => (None, Some(crate::names::medivac_refused(why))),
        },
        (world::Class::Commander, Action::Ability3) => match world.can_rally(slot) {
            Ok(()) => (Some(Order::Rally), None),
            Err(why) => (None, Some(rally_refused(why))),
        },
        (world::Class::Commander, Action::Ability4) => match world.can_reinforce(slot) {
            Ok(()) => (Some(Order::Reinforce), None),
            Err(why) => (None, Some(crate::names::reinforce_refused(why))),
        },
        // The medic's (task 153): Q drops a Heal Drone; C, Triage, is
        // passive; E beams the crew member under the pointer — on the
        // one already held, or on nobody while one is held, it unlinks;
        // R switches his Healing Circle on or off.
        (world::Class::Medic, Action::Ability1) => match world.can_heal_drone(slot) {
            Ok(()) => (Some(Order::HealDrone), None),
            Err(why) => (None, Some(crate::names::heal_drone_refused(why))),
        },
        (world::Class::Medic, Action::Ability3) => match under {
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
        (world::Class::Medic, Action::Ability4) => {
            let on = !world.is_circling(slot);
            match world.can_healing_circle(slot, on) {
                Ok(()) => (Some(Order::HealingCircle(on)), None),
                Err(why) => (None, Some(crate::names::healing_circle_refused(why))),
            }
        }
        // The tank's (task 155): Q raises the Riot Shield and puts it down
        // and E raises the Reflect Barrier (`class_key`); C, Plated, is
        // passive; R throws the Bastion.
        (world::Class::Tank, Action::Ability1) => class_key(world, slot, true, tile, None),
        (world::Class::Tank, Action::Ability3) => class_key(world, slot, false, tile, None),
        (world::Class::Tank, Action::Ability4) => match world.can_bastion(slot) {
            Ok(()) => (Some(Order::Bastion), None),
            Err(why) => (None, Some(crate::names::bastion_refused(why))),
        },
        _ => (None, None),
    }
}

/// The reach of a medic's key held down — the beam's (E) or his circle's
/// (R) — once it has a rank, in tiles, with the ability's glyph for its
/// colour. (A soldier's Stun Shot arms the pointer as a throw does.)
fn held_reach(
    world: &world::World,
    slot: u32,
    keys: &Keys,
    input: &egui::InputState,
) -> Option<(f32, Glyph)> {
    if world.class_of(slot) != world::Class::Medic {
        return None;
    }
    if keys.down(input, Action::Ability3) && world.rank_of(slot, world::class::SLOT_E) > 0 {
        return Some((world.beam_range(slot), Glyph::HealBeam));
    }
    if keys.down(input, Action::Ability4) && world.rank_of(slot, world::class::SLOT_R) > 0 {
        return Some((world.healing_circle_radius(slot), Glyph::HealingCircle));
    }
    None
}

/// What a key that arms the pointer throws: the soldier's grenade (Q),
/// the engineer's satchel charge (E, task 154) or the soldier's Stun Shot
/// (E, aimed as the grenade is since October 2026, the player's word).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Throw {
    Grenade,
    Satchel,
    StunShot,
}

impl Throw {
    /// The engineer's satchel charge, the throw that is not a grenade.
    fn satchel(self) -> bool {
        self == Throw::Satchel
    }

    /// The ability's glyph, for the rings' colour.
    fn glyph(self) -> Glyph {
        match self {
            Throw::Grenade => Glyph::FragGrenade,
            Throw::Satchel => Glyph::Satchel,
            Throw::StunShot => Glyph::StunShot,
        }
    }
}

/// The key of a throw: held, the throw's reach is drawn; let go, it is
/// thrown at the pointer. The soldier's grenade is Q, the engineer's
/// satchel charge and the soldier's Stun Shot E.
fn throw_action(throw: Throw) -> Action {
    match throw {
        Throw::Grenade => Action::Ability1,
        Throw::Satchel | Throw::StunShot => Action::Ability3,
    }
}

/// Whether an ability key is a throw that arms the pointer: the soldier's
/// Q, a grenade, and E, a Stun Shot, and the engineer's E, a satchel
/// charge.
fn throw_key(class: world::Class, action: Action) -> Option<Throw> {
    match class {
        world::Class::Soldier if action == throw_action(Throw::Grenade) => Some(Throw::Grenade),
        world::Class::Soldier if action == throw_action(Throw::StunShot) => Some(Throw::StunShot),
        world::Class::Engineer if action == throw_action(Throw::Satchel) => Some(Throw::Satchel),
        _ => None,
    }
}

/// Whether the throw key may arm the pointer: everything the throw asks
/// but the tile — which the click picks, and which a walk out may yet
/// make good — asked with the Bim's own tile; the log's line otherwise.
/// A Stun Shot is never refused its tile.
fn can_arm_throw(world: &world::World, slot: u32, throw: Throw) -> Result<(), String> {
    if throw == Throw::StunShot {
        return world.can_stun_shot(slot).map_err(stun_shot_refused);
    }
    let t = shipdesign::TILE as f32;
    let at = world.aboard.room.bim_pos(slot as usize);
    let tile = ((at.x / t).floor() as i32, (at.y / t).floor() as i32);
    match world.can_throw_now(slot, throw.satchel(), tile) {
        Ok(())
        | Err(Refusal::CantThrowThere | Refusal::OutOfThrowRange | Refusal::NoLineToTile) => Ok(()),
        Err(why) => Err(throw_refused(why)),
    }
}

/// The order an armed pointer's click — or its key let go — gives at
/// `tile`: a throw, walking out first where it must, or a Stun Shot
/// charged at it (`can_stun_shot` asked again; the log's line if refused).
fn throw_order(
    world: Option<&world::World>,
    slot: u32,
    throw: Throw,
    (x, y): (i32, i32),
) -> (Option<Order>, Option<String>) {
    match throw {
        Throw::Grenade | Throw::Satchel => (
            Some(Order::ThrowAt {
                satchel: throw.satchel(),
                x,
                y,
            }),
            None,
        ),
        Throw::StunShot => match world.map(|w| w.can_stun_shot(slot)) {
            Some(Ok(())) => (Some(Order::StunShot { x, y }), None),
            Some(Err(why)) => (None, Some(stun_shot_refused(why))),
            None => (None, None),
        },
    }
}

/// The engineer's keys (task 127; task 154), by ability slot: Q lays a
/// mine on the tile under the pointer, C a Healing Sentry there, E throws
/// a satchel charge at it and R lays the sentry — the order, or the log's
/// line off the world's own refusal. The screen takes E as a quick throw
/// ([`throw_key`]) before it gets here; Space sets the satchels off
/// ([`detonate_key`]).
fn engineer_key(
    world: &world::World,
    slot: u32,
    ability: u8,
    tile: Option<(i32, i32)>,
) -> (Option<Order>, Option<String>) {
    use world::DeployKind;
    let Some(tile) = tile else {
        return (None, Some(deploy_refused(Refusal::CantDeployThere)));
    };
    let (x, y) = tile;
    match ability {
        world::class::SLOT_R => match world.can_lay_sentry(slot, tile) {
            Ok(()) => (Some(Order::Sentry { x, y }), None),
            Err(why) => (None, Some(deploy_refused(why))),
        },
        world::class::SLOT_Q | world::class::SLOT_C => {
            let kind = if ability == world::class::SLOT_C {
                DeployKind::HealingSentry
            } else {
                DeployKind::Mine
            };
            match world.can_deploy(slot, kind, tile) {
                Ok(()) => (Some(Order::Deploy { kind, x, y }), None),
                Err(why) => (None, Some(deploy_refused(why))),
            }
        }
        // The satchel thrown at once, walking out first where it must.
        world::class::SLOT_E => match world.can_throw_satchel(slot, tile) {
            Ok(()) | Err(Refusal::OutOfThrowRange | Refusal::NoLineToTile) => (
                Some(Order::ThrowAt {
                    satchel: true,
                    x,
                    y,
                }),
                None,
            ),
            Err(why) => (None, Some(throw_refused(why))),
        },
        _ => (None, None),
    }
}

/// The engineer's remote trigger (task 154, Space): every satchel of his
/// set off, or the log's line off the world's refusal. Nothing for any
/// other class — the key is the engineer's alone.
fn detonate_key(world: &world::World, slot: u32) -> (Option<Order>, Option<String>) {
    if world.class_of(slot) != world::Class::Engineer {
        return (None, None);
    }
    match world.can_detonate(slot) {
        Ok(()) => (Some(Order::Detonate), None),
        Err(why) => (None, Some(crate::names::detonate_refused(why))),
    }
}

/// One ability box of a ranked kit (task 124): what the ability is at
/// its rank, the pips, and the "+" while a point could buy the next.
fn ranked_box(world: &world::World, slot: u32, action: Action, keys: &Keys) -> AbilityBox {
    let class = world.class_of(slot);
    let ability = action.ability_slot().unwrap_or(0) as u8;
    let rank = world.rank_of(slot, ability);
    let face = match (class, ability) {
        (world::Class::Soldier, 0) => Face::charges(
            world,
            slot,
            world::Charge::Grenade,
            Some(Glyph::FragGrenade),
        ),
        (world::Class::Soldier, 1) => Face::of(Some(Glyph::WeakSpot)),
        (world::Class::Soldier, 2) => Face {
            cooldown: world.stun_shot_cooldown_left(slot),
            cooldown_whole: world.stun_shot_cooldown(slot),
            on: world.is_charging(slot),
            ..Face::of(Some(Glyph::StunShot))
        },
        (world::Class::Soldier, 3) => Face {
            cooldown: world.rampage_cooldown_left(slot),
            cooldown_whole: world.rampage_cooldown(slot),
            on: world.is_rampaging(slot),
            ..Face::of(Some(Glyph::Rampage))
        },
        // The engineer's (task 127): three stocks of charges, and the
        // ultimate on its own cooldown, lit while its sentry stands.
        (world::Class::Engineer, 0) => {
            Face::charges(world, slot, world::Charge::Mine, Some(Glyph::Mine))
        }
        (world::Class::Engineer, 1) => Face::charges(
            world,
            slot,
            world::Charge::HealingSentry,
            Some(Glyph::HealingSentry),
        ),
        // The satchels: lit while any lies out for the trigger.
        (world::Class::Engineer, 2) => Face {
            on: world.satchels_out(slot) > 0,
            ..Face::charges(world, slot, world::Charge::Satchel, Some(Glyph::Satchel))
        },
        (world::Class::Engineer, 3) => Face {
            cooldown: world.sentry_cooldown_left(slot),
            cooldown_whole: world.sentry_cooldown(slot),
            on: world.sentry_standing(slot),
            ..Face::of(Some(Glyph::Sentry))
        },
        // The commander's (task 129): two shouts on their cooldowns, lit
        // while they run; his medics and his reinforcements on their
        // cooldowns, counted, those still standing this mission.
        (world::Class::Commander, 0) => Face {
            cooldown: world.battle_cry_cooldown_left(slot),
            cooldown_whole: world.battle_cry_cooldown(slot),
            on: world.is_crying(slot),
            ..Face::of(Some(Glyph::BattleCry))
        },
        (world::Class::Commander, 1) => Face {
            cooldown: world.medivac_cooldown_left(slot),
            cooldown_whole: world.medivac_cooldown(slot),
            count: Some(world.medivacs_of(slot).len() as u32),
            ..Face::of(Some(Glyph::Medivac))
        },
        (world::Class::Commander, 2) => Face {
            cooldown: world.rally_cooldown_left(slot),
            cooldown_whole: world.rally_cooldown(slot),
            on: world.is_rallying(slot),
            ..Face::of(Some(Glyph::Rally))
        },
        (world::Class::Commander, 3) => Face {
            cooldown: world.reinforcement_cooldown_left(slot),
            cooldown_whole: world.reinforcement_cooldown(slot),
            count: Some(world.reinforcements_of(slot).len() as u32),
            ..Face::of(Some(Glyph::Reinforcements))
        },
        // The medic's (task 153): the drone on its cooldown, lit while one
        // is in the air; Triage lit once learnt; the beam counting the
        // patients it could still take, lit while it holds anybody; the
        // circle lit while it is on.
        (world::Class::Medic, 0) => Face {
            cooldown: world.heal_drone_cooldown_left(slot),
            cooldown_whole: world.heal_drone_cooldown(slot),
            on: world.drone_of(slot).is_some(),
            ..Face::of(Some(Glyph::HealDrone))
        },
        (world::Class::Medic, 1) => Face {
            on: world.rank_of(slot, world::class::SLOT_C) > 0,
            ..Face::of(Some(Glyph::Triage))
        },
        (world::Class::Medic, 2) => {
            let held = world.patients_of(slot).len();
            Face {
                count: Some(world.beam_patients(slot).saturating_sub(held) as u32),
                on: held > 0,
                ..Face::of(Some(Glyph::HealBeam))
            }
        }
        (world::Class::Medic, 3) => Face {
            on: world.is_circling(slot),
            ..Face::of(Some(Glyph::HealingCircle))
        },
        // The tank's (task 155): the Riot Shield lit while it is up,
        // counting the hit points it has left, and broken on its cooldown;
        // Plated always on; the Reflect Barrier and the Bastion on their
        // cooldowns, the barrier lit while it runs.
        (world::Class::Tank, 0) => Face {
            count: (world.riot_shield_hp(slot) > 0.0)
                .then(|| world.riot_shield_left(slot).ceil() as u32),
            cooldown: world.riot_shield_cooldown_left(slot),
            cooldown_whole: world.riot_shield_cooldown(slot),
            on: world.is_shielding(slot),
            ..Face::of(Some(Glyph::RiotShield))
        },
        (world::Class::Tank, 1) => Face::of(Some(Glyph::Plated)),
        (world::Class::Tank, 2) => Face {
            cooldown: world.reflect_cooldown_left(slot),
            cooldown_whole: world.reflect_cooldown(slot),
            on: world.is_reflecting(slot),
            ..Face::of(Some(Glyph::Reflect))
        },
        (world::Class::Tank, 3) => Face {
            cooldown: world.bastion_cooldown_left(slot),
            cooldown_whole: world.bastion_cooldown(slot),
            ..Face::of(Some(Glyph::Bastion))
        },
        _ => Face::of(None),
    };
    // Not learnt, the box waits on the level its first rank wants — the
    // ultimate's sixth — and says it; learnt, nothing is locked.
    let first = world::class::rank_level(class, ability, 1).unwrap_or(1);
    let locked = (rank == 0 && world.level_of(slot) < first).then_some(first);
    AbilityBox {
        key: keys.key(action).name().to_string(),
        name: crate::names::ranked_ability(class, ability),
        tip: crate::names::ranked_what(class, ability).to_string(),
        stats: crate::names::ranked_stats(class, ability),
        foot: crate::names::ranked_foot(class, ability, rank),
        mark: face.mark,
        // A stock at rank nought counts nothing: there is none to have.
        count: face.count.filter(|_| rank > 0),
        cooldown: face.cooldown,
        cooldown_whole: face.cooldown_whole,
        recharge: face.recharge,
        charge: face.charge,
        on: face.on,
        short: face.short && rank > 0,
        locked,
        action: Some(action),
        rank: Some((rank, world::class::MAX_RANK)),
        plus: world.can_rank_up(slot, ability).is_ok(),
        unlearnt: rank == 0,
    }
}

/// Every key the class `slot` steers has a box for, in the order they
/// are laid out: the four ability slots, Q, C, E and R, for everybody
/// (task 123; the second and fourth empty for now), and past them
/// whatever else that class has a key of its own for (feature 86) — the
/// medic's carry. A crew member with no class has no keys and no
/// boxes.
fn ability_keys(world: &world::World, slot: u32) -> Vec<Action> {
    use world::Class;
    let class = world.class_of(slot);
    let mut keys = match class {
        Class::None => return Vec::new(),
        _ => Action::ABILITIES.to_vec(),
    };
    if world.can_lift(slot) {
        keys.push(Action::Carry);
    }
    keys
}

/// The boxes for the class `slot` steers, the four slots first, in Q C
/// E R order. None for a classless crew member, which has no keys.
fn ability_boxes(world: &world::World, slot: u32, keys: &Keys) -> Vec<AbilityBox> {
    if world.class_of(slot) == world::Class::None {
        return Vec::new();
    }
    ability_keys(world, slot)
        .into_iter()
        .map(|action| {
            // Every class's four slots are its ranked kit's four abilities
            // (task 124; the tank's since task 139).
            if action.ability_slot().is_some() {
                return ranked_box(world, slot, action, keys);
            }
            // The keys past the slots, one class's each and read off the
            // world: the medic's carry.
            let face = match action {
                Action::Carry => {
                    // Carrying, the box counts nothing: a nought in the
                    // corner reads as "nothing to do", and what the key
                    // does now is set this one down. Empty-handed it is
                    // how many are near enough to pick up.
                    let carrying = world.carrying_of(slot).is_some();
                    let near = world.carryable_near(slot).len() as u32;
                    Face {
                        count: (!carrying).then_some(near),
                        on: carrying,
                        short: !carrying && near == 0,
                        ..Face::of(Some(Glyph::Carry))
                    }
                }
                _ => Face::of(None),
            };
            let (name, tip) = match action {
                Action::Carry => (crate::names::CARRY, crate::names::CARRY_TIP),
                _ => ("", ""),
            };
            AbilityBox {
                key: keys.key(action).name().to_string(),
                name,
                tip: tip.to_string(),
                stats: Vec::new(),
                foot: String::new(),
                mark: face.mark,
                count: face.count,
                cooldown: face.cooldown,
                cooldown_whole: face.cooldown_whole,
                recharge: face.recharge,
                charge: face.charge,
                on: face.on,
                short: face.short,
                locked: None,
                action: Some(action),
                rank: None,
                plus: false,
                unlearnt: false,
            }
        })
        .collect()
}

/// Seconds of the mission clock a Battle Cry's gold ring takes to run out
/// to its reach on the deck (task 129): a short ring, not the cry's whole
/// length.
const BATTLE_CRY_RING_SECONDS: f64 = 0.6;

/// How big one box is: a key's on the hero panel (feature 107), its name
/// in its tooltip rather than under it, so the panel stays one row.
const ABILITY_SIDE: f32 = 44.0;

/// Whom a cast of `slot`'s would reach, by crew index (feature 86) —
/// what the deck rings while the pointer rests on that key's box, which
/// is the panels' own rule (resting on a row rings what it names) said
/// about an ability instead of a fixture.
///
/// The commander's are the point of it: his **rally** lifts every
/// friendly Bim in his aura. The medic's drone names whom it is over,
/// his beam its patients and his circle whom it heals (task 153), and the carry names everybody near enough to pick up. The rest reach
/// enemies or nobody, and ring nothing.
fn affected_by(world: &world::World, slot: u32, action: Action) -> Vec<u32> {
    use world::Class;
    let class = world.class_of(slot);
    match (class, action) {
        // The commander's (task 129): a Battle Cry or a Rally called now
        // reaches everybody on the deck within its tiles of him, himself
        // included; his medics and his reinforcements are the ones he
        // brought.
        (Class::Commander, Action::Ability1) => {
            world.crew_within(slot, world::class::BATTLE_CRY_TILES)
        }
        (Class::Commander, Action::Ability2) => world.medivacs_of(slot),
        (Class::Commander, Action::Ability3) => world.crew_within(slot, world::class::RALLY_TILES),
        (Class::Commander, Action::Ability4) => world.reinforcements_of(slot),
        // The medic's (task 153): the drone names whom it is over; the
        // beam its patients; the circle whom it heals now.
        (Class::Medic, Action::Ability1) => world
            .drone_of(slot)
            .and_then(|d| d.patient)
            .into_iter()
            .collect(),
        (Class::Medic, Action::Ability3) => world.patients_of(slot),
        (Class::Medic, Action::Ability4) => world.healing_circle_reaching(slot),
        (_, Action::Carry) => match world.carrying_of(slot) {
            Some(patient) => vec![patient],
            None => world.carryable_near(slot),
        },
        _ => Vec::new(),
    }
}

/// The class's keys, a box each in a row inside the hero panel (feature
/// 107; they were a bar of their own, `game-abilities`, since feature
/// 80). The medicine's two boxes stood past a rule at the right-hand end
/// until task 120 took the medkits and the bandages away. The key whose
/// box the pointer rests on, for the ring round whom its cast would
/// reach, and the rank-up a Ctrl-click on a slot's box asked for (task
/// 123).
/// The quickselect in the hero panel (task 138): the weapon over the
/// medkit, each with its key, the one in hand lit. The one clicked, if
/// any.
fn quickselect(ui: &mut egui::Ui, hand: bims::bim::Hand, keys: &Keys) -> Option<bims::bim::Hand> {
    use bims::bim::Hand;
    let mut picked = None;
    ui.vertical(|ui| {
        ui.spacing_mut().item_spacing.y = 3.0;
        // One key swaps the two (October 2026): the one in hand is lit,
        // and the key is said under them.
        for (one, word, tip) in [
            (Hand::Weapon, HAND_WEAPON, HAND_WEAPON_TIP),
            (Hand::Medkit, HAND_MEDKIT, HAND_MEDKIT_TIP),
        ] {
            let text = egui::RichText::new(word).small();
            let button = theme::toggle_button(one == hand, text).min_size(egui::vec2(66.0, 18.0));
            if ui.add(button).on_hover_text(tip).clicked() {
                picked = Some(one);
            }
        }
        ui.label(
            egui::RichText::new(crate::names::hand_swap_key(keys.key(Action::Medkit).name()))
                .small()
                .color(theme::MUTED),
        );
    });
    picked
}

/// How big one item box is: Dota's inventory, two by two beside the
/// abilities, the two rows as tall as the hero panel inside its frame
/// (the player's word) — about an ability's box.
const ITEM_SIDE: f32 = 41.0;

/// The gap between two item boxes, across and down.
const ITEM_GAP: f32 = 3.0;

/// The four item slots of the player's own Bim, two by two (October
/// 2026): each its picture, its key in the corner, an active one's
/// cooldown swept back as an ability's is, and greyed while a hit locks
/// a blink. Empty, a dark frame with the key. Resting on one names it and
/// says what it does. Laid from the top of the panel's row: a grid put
/// straight into the row stood a line's height below it.
fn item_grid(ui: &mut egui::Ui, world: &world::World, who: u32, keys: &Keys) {
    let items = world.items_of(who);
    let locked = world.blink_locked_left(who) > 0.0;
    ui.vertical(|ui| {
        egui::Grid::new("hud-items")
            .spacing(egui::vec2(ITEM_GAP, ITEM_GAP))
            .min_col_width(ITEM_SIDE)
            .min_row_height(ITEM_SIDE)
            .show(ui, |ui| {
                for (index, action) in Action::ITEMS.into_iter().enumerate() {
                    let (rect, response) = ui.allocate_exact_size(
                        egui::vec2(ITEM_SIDE, ITEM_SIDE),
                        egui::Sense::hover(),
                    );
                    let painter = ui.painter();
                    let key = keys.key(action).name();
                    match items[index] {
                        None => {
                            painter.rect_filled(rect, 4.0, theme::RAISED.gamma_multiply(0.4));
                            painter.rect_stroke(
                                rect,
                                4.0,
                                egui::Stroke::new(1.0, theme::LINE),
                                egui::StrokeKind::Inside,
                            );
                        }
                        Some(item) => {
                            crate::icons::module(painter, rect, item);
                            let left = world.item_cooldown_left(who, index);
                            let whole = world.item_cooldown(who, index);
                            if left > 0.0 && whole > 0.0 {
                                cooldown_sweep(
                                    painter,
                                    rect,
                                    (left / whole).clamp(0.0, 1.0) as f32,
                                    theme::PANEL_DEEP.gamma_multiply(0.85),
                                );
                                painter.text(
                                    rect.center(),
                                    egui::Align2::CENTER_CENTER,
                                    format!("{}", left.ceil()),
                                    egui::FontId::proportional(13.0),
                                    theme::INK,
                                );
                            } else if item.is_blink() && locked {
                                painter.rect_filled(
                                    rect,
                                    4.0,
                                    theme::PANEL_DEEP.gamma_multiply(0.6),
                                );
                            }
                            // An Ablative Shell on: its box lit, as an ability
                            // running is.
                            if item.kind == bims::module::ModuleKind::AblativeShell
                                && world.shell_left(who) > 0.0
                            {
                                painter.rect_stroke(
                                    rect,
                                    4.0,
                                    egui::Stroke::new(2.0, theme::CAUTION),
                                    egui::StrokeKind::Inside,
                                );
                            }
                            let passive = !item.kind.active();
                            response.on_hover_ui(|ui| {
                                ui.label(crate::names::module_tip(item, passive));
                                crate::crew::item_tiers(ui, item.kind, item.tier.code());
                            });
                        }
                    }
                    painter.text(
                        rect.min + egui::vec2(4.0, 2.0),
                        egui::Align2::LEFT_TOP,
                        key,
                        egui::FontId::proportional(10.0),
                        theme::MUTED,
                    );
                    if index % 2 == 1 {
                        ui.end_row();
                    }
                }
            });
    });
}

fn ability_row(ui: &mut egui::Ui, boxes: &[AbilityBox]) -> RowOut {
    let mut out = RowOut {
        hovered: None,
        rank_up: None,
    };
    ui.spacing_mut().item_spacing.x = 4.0;
    for one in boxes {
        let (resting, clicked) = ability_box(ui, one);
        if resting {
            out.hovered = one.action;
        }
        let ctrl = ui.input(|i| i.modifiers.ctrl);
        out.rank_up = out.rank_up.or(rank_up_by_click(one.action, clicked, ctrl));
    }
    out
}

/// What the row of boxes was asked this frame.
struct RowOut {
    /// The key whose box the pointer rests on.
    hovered: Option<Action>,
    /// A slot's box clicked with Ctrl held (task 123).
    rank_up: Option<RankUp>,
}

/// One box: the key in the corner, the picture in the middle, and what is
/// left in the other corner. Resting on it names it and says what the key
/// does — a box is a control, so it carries its own words rather than an
/// underlined one beside it; the name was under the box until the hero
/// panel took the boxes in (feature 107).
/// Whether the pointer is resting on it — what the deck rings the cast's
/// own Bims by (feature 86) — and whether it was clicked, which only the
/// four slots' boxes hear (task 123: Ctrl and a click ranks one up). An
/// empty slot is an empty frame with its key, and says nothing on a
/// hover.
fn ability_box(ui: &mut egui::Ui, one: &AbilityBox) -> (bool, bool) {
    ui.vertical(|ui| {
        ui.spacing_mut().item_spacing.y = 2.0;
        let sense = if one.action.and_then(Action::ability_slot).is_some() {
            egui::Sense::click()
        } else {
            egui::Sense::hover()
        };
        let (rect, response) =
            ui.allocate_exact_size(egui::vec2(ABILITY_SIDE, ABILITY_SIDE), sense);
        let Some(glyph) = one.mark else {
            let painter = ui.painter();
            painter.rect_filled(rect, 4.0, theme::RAISED.gamma_multiply(0.5));
            painter.rect_stroke(
                rect,
                4.0,
                egui::Stroke::new(1.0, theme::LINE),
                egui::StrokeKind::Inside,
            );
            painter.text(
                rect.min + egui::vec2(4.0, 3.0),
                egui::Align2::LEFT_TOP,
                &one.key,
                egui::FontId::proportional(11.0),
                theme::MUTED,
            );
            return (false, response.clicked());
        };
        let ready = one.ready();
        let painter = ui.painter();
        // The frame: lit while the ability runs, dull while it waits, and
        // ready it is the picture's own rim in its class's colour.
        let edge = if one.on {
            Some(theme::CAUTION)
        } else if ready {
            None
        } else {
            Some(theme::LINE)
        };
        // The picture fills the box, in its class's colours, its glow
        // brighter while the ability runs (`ability_icons`); dimmed below
        // while the key would not be taken, so the whole box reads as off
        // rather than the number alone.
        ability_icons::paint(painter, rect, glyph, one.on, 4.0);
        let middle = rect.center();
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
        if let Some(edge) = edge {
            painter.rect_stroke(
                rect,
                4.0,
                egui::Stroke::new(1.0, edge),
                egui::StrokeKind::Inside,
            );
        }
        // The key, in the top left, over a shadow so it reads on the
        // picture.
        painter.text(
            rect.min + egui::vec2(5.0, 4.0),
            egui::Align2::LEFT_TOP,
            &one.key,
            egui::FontId::proportional(11.0),
            theme::PANEL_DEEP,
        );
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
            if glyph.is_stock() {
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
        // A charge, as a bar along the foot.
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
            // A level to wait for is two words in a box the size of a
            // key: a size smaller than a cooldown's seconds.
            let size = if one.locked.is_some() { 10.5 } else { 13.0 };
            // A shadow under the words, since they sit over the picture.
            painter.text(
                middle + egui::vec2(1.0, 1.0),
                egui::Align2::CENTER_CENTER,
                &words,
                egui::FontId::proportional(size),
                theme::PANEL_DEEP,
            );
            painter.text(
                middle,
                egui::Align2::CENTER_CENTER,
                words,
                egui::FontId::proportional(size),
                color,
            );
        }
        // A skill point could buy its next rank now (task 124): a "+" in
        // the top right corner.
        if one.plus {
            let at = egui::pos2(rect.max.x - 7.0, rect.min.y + 7.0);
            painter.circle_filled(at, 6.0, theme::ACCENT);
            painter.text(
                at,
                egui::Align2::CENTER_CENTER,
                "+",
                egui::FontId::proportional(12.0),
                theme::PANEL_DEEP,
            );
        }
        let resting = response.hovered();
        let clicked = response.clicked();
        response.on_hover_ui(|ui| {
            ui.label(egui::RichText::new(one.name).strong());
            ui.label(&one.tip);
            if !one.stats.is_empty() {
                ui.add_space(2.0);
                theme::stat_rows(ui, &one.stats, one.rank.map(|(rank, _)| rank), 13.0);
            }
            if !one.foot.is_empty() {
                ui.add_space(2.0);
                ui.label(egui::RichText::new(&one.foot).small().color(theme::MUTED));
            }
        });
        // And the ranks under a ranked ability's box: a pip a rank, filled
        // for one bought and hollow for one not.
        if let Some((rank, top)) = one.rank {
            rank_pips(ui, rank, top);
        }
        (resting, clicked)
    })
    .inner
}

/// The pips under a ranked ability's box (task 124): `top` of them in a
/// row as wide as the box, the first `rank` filled.
fn rank_pips(ui: &mut egui::Ui, rank: u8, top: u8) {
    let (rect, _) = ui.allocate_exact_size(egui::vec2(ABILITY_SIDE, PIP_ROW), egui::Sense::hover());
    let painter = ui.painter();
    let gap = ABILITY_SIDE / f32::from(top.max(1));
    for k in 0..top {
        let at = egui::pos2(rect.min.x + gap * (f32::from(k) + 0.5), rect.center().y);
        if k < rank {
            painter.circle_filled(at, PIP_RADIUS, theme::CAUTION);
        } else {
            painter.circle_stroke(at, PIP_RADIUS, egui::Stroke::new(1.0, theme::MUTED));
        }
    }
}

/// How tall the row of rank pips is, and how big a pip.
const PIP_ROW: f32 = 7.0;
const PIP_RADIUS: f32 = 2.5;

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
    painter.add(egui::Shape::line(
        arc,
        egui::Stroke::new(2.0, theme::ACCENT),
    ));
}

/// What the class's two keys do for the crew member `slot` steers
/// (features 74, 75 and 76): `primary` is Q, the other E, `tile` the
/// room tile under the pointer if it is over the deck, and `under` the
/// crew member under the pointer if there is one. An engineer's Q sets
/// a sentry up on the tile and its E lays sandbags there; a soldier's Q
/// throws a grenade at it and its E charges a Stun Shot at it; a medic's Q
/// drops a Heal Drone and its E beams `under` (task 153) — the medic's own Bim
/// included, since task 120 lets a medic beam itself — and pressed on the
/// one it already holds, or on nobody while one is held, it unlinks, and
/// on nobody with no beam on it says so; a tank's Q raises his Riot Shield or puts
/// it down and his E raises his Reflect Barrier; a commander's are his Battle Cry and
/// Rally (task 129). The order to send, if the world would take it, and
/// the log's line saying why not if it would not — both off the world's
/// own check, so the key and the command agree. A classless crew
/// member's keys do nothing at all.
fn class_key(
    world: &world::World,
    slot: u32,
    primary: bool,
    tile: Option<(i32, i32)>,
    under: Option<u32>,
) -> (Option<Order>, Option<String>) {
    match world.class_of(slot) {
        world::Class::None => (None, None),
        // The engineer's Q and E are its ranked kit's mine and satchel
        // charge (task 154).
        world::Class::Engineer => {
            let ability = if primary {
                world::class::SLOT_Q
            } else {
                world::class::SLOT_E
            };
            engineer_key(world, slot, ability, tile)
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
        // The soldier's E (October 2026): a Stun Shot charged at the tile
        // under the pointer — the world keeps it within the weapon's reach.
        world::Class::Soldier => match (world.can_stun_shot(slot), tile) {
            (Ok(()), Some((x, y))) => (Some(Order::StunShot { x, y }), None),
            (Ok(()), None) => (None, Some(stun_shot_refused(Refusal::CantThrowThere))),
            (Err(why), _) => (None, Some(stun_shot_refused(why))),
        },
        // The medic (task 153): his ranked kit's Q and E, a Heal Drone
        // and the beam on the crew member under the pointer —
        // `ranked_key`'s answer, since his slots are ranked.
        world::Class::Medic => ranked_key(
            world,
            slot,
            if primary {
                Action::Ability1
            } else {
                Action::Ability3
            },
            tile,
            under,
        ),
        // The tank (task 155): Q raises the Riot Shield or puts it down,
        // E raises the Reflect Barrier.
        world::Class::Tank if primary => {
            let on = !world.is_shielding(slot);
            match world.can_riot_shield(slot, on) {
                Ok(()) => (Some(Order::RiotShield(on)), None),
                Err(why) => (None, Some(riot_shield_refused(why))),
            }
        }
        world::Class::Tank => match world.can_reflect(slot) {
            Ok(()) => (Some(Order::Reflect), None),
            Err(why) => (None, Some(reflect_refused(why))),
        },
        // The commander (task 129): his ranked kit's Q and E, a Battle
        // Cry and a Rally — `ranked_key`'s answer, since his slots are
        // ranked.
        world::Class::Commander => ranked_key(
            world,
            slot,
            if primary {
                Action::Ability1
            } else {
                Action::Ability3
            },
            tile,
            under,
        ),
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
/// The pointer at rest: **armed**, on the first press whatever is down,
/// so the click after it picks the ground (a fresh tile moves the
/// banner there, its own tile takes it up) — unless the map is up, which
/// has no deck to put a banner on. The pointer already armed: thought
/// better of, and a banner already down is **taken up** with it, since
/// the world reads the same order given again as a release
/// ([`world::Standing::same_as`]) and *the same order* means the same
/// **tile** — a second banner anywhere else is a fresh attack, so
/// without this there is no key that ever lets the crew go and they
/// stand under arms at the banner for ever, taking no errand. Esc or a
/// right-click puts the armed pointer away and leaves the banner be.
fn attack_key(
    standing: world::Standing,
    armed: bool,
    map_up: bool,
) -> (Option<world::Standing>, bool) {
    match standing {
        world::Standing::Attack { .. } if armed => (Some(standing), false),
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

/// How near, in tiles, a downed crewmate has to lie for the held revive
/// key to reach it: standing close, a step or two over at most.
const REVIVE_HOLD_REACH: f32 = 2.5;

/// Frames the menu's carry waits for the walk over to begin before a
/// Bim standing still out of reach is taken as having stopped short:
/// the walk goes through the seam and starts a frame or two later.
const CARRY_WALK_GRACE: u32 = 30;

/// What a right-click at room point `(x, y)` revives, the medkit in
/// `own`'s hand or not (task 138; any hand since October 2026): `None`
/// when no downed crewmate or townsperson (`bims::game::GUEST`) is under
/// the pointer — the click is a pick or a menu then — else the patient,
/// or the reason `own` cannot revive it, for the log.
fn downed_patient(
    room: &bims::game::Game,
    own: usize,
    x: f32,
    y: f32,
    name: &dyn Fn(u32) -> String,
) -> Option<Result<u32, String>> {
    // A downed crewmate, else a townsperson down on the joined deck — a
    // fighting townsperson is picked up the same way.
    let patient = room
        .crew_at(x, y)
        .filter(|&p| p != own && room.is_downed(p))
        .or_else(|| room.guest_at(x, y))?;
    Some(
        match crate::crew::revive_refused(room, own, patient, name) {
            Some(why) => Err(why),
            None => Ok(patient as u32),
        },
    )
}

/// The held revive key (G), a frame at a time: the order if there is one
/// to send, and the log's line. `held` is the patient the key sent the
/// player's own Bim `own` to, kept on the screen between frames.
///
/// * the key goes down beside a downed crewmate or townsperson — the
///   nearest within [`REVIVE_HOLD_REACH`] that `crew::revive_refused`
///   passes — and it is the revive (`CrewOrder::Revive`), the walk over
///   and the hands on;
/// * held, nothing more is sent while that one is still down; once it is
///   up the next one near is taken, the key still held;
/// * let go with the revive still in hand, and it is stopped
///   (`CrewOrder::StandDown`, which lets a revive go);
/// * pressed with nobody near, the log says so.
fn held_revive(
    room: &bims::game::Game,
    own: usize,
    held: &mut Option<u32>,
    down: bool,
    pressed: bool,
    name: &dyn Fn(u32) -> String,
) -> (Option<Order>, Option<String>) {
    if !down {
        if let Some(patient) = held.take()
            && room.reviving(own) == Some(patient as usize)
        {
            return (
                Some(Order::Crew(CrewOrder::StandDown { who: own as u32 })),
                None,
            );
        }
        return (None, None);
    }
    // A crewmate down, or a townsperson down on the joined deck
    // (`bims::game::GUEST`), whose body is in its own room.
    let still_down = |p: usize| {
        if p >= bims::game::GUEST {
            room.is_revivable_guest(p)
        } else {
            room.is_downed(p)
        }
    };
    let lies_at = |p: usize| {
        if p >= bims::game::GUEST {
            room.guest_pos(p)
        } else {
            Some(room.bim_pos(p))
        }
    };
    if let Some(patient) = *held {
        if still_down(patient as usize) {
            return (None, None);
        }
        *held = None;
    }
    if (own as u32) >= room.crew_count() || !room.is_alive(own) {
        return (None, None);
    }
    let at = room.bim_pos(own);
    let reach = REVIVE_HOLD_REACH * shipdesign::TILE as f32;
    let away = |p: usize| lies_at(p).map_or(f32::MAX, |q| (q - at).len());
    let nearest = (0..room.crew_count() as usize)
        .filter(|&p| p != own && room.is_downed(p))
        .chain(room.revivable_guests())
        .filter(|&p| away(p) <= reach && crate::crew::revive_refused(room, own, p, name).is_none())
        .min_by(|&a, &b| away(a).total_cmp(&away(b)));
    match nearest {
        Some(patient) => {
            *held = Some(patient as u32);
            (
                Some(Order::Crew(CrewOrder::Revive {
                    who: own as u32,
                    patient: patient as u32,
                })),
                None,
            )
        }
        None => (None, pressed.then(|| REVIVE_NOBODY_NEAR.to_string())),
    }
}

#[cfg(test)]
mod class_key_tests {
    use super::*;
    use crate::names;
    use shipdesign::fixture::flyer;
    use world::fixture::{REFERENCE_MONEY, simulation_world};

    /// Q and E dispatch by the steered crew member's class: an engineer's
    /// deploys, a soldier's throws and Stun Shots, a classless one's do
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
        assert_eq!(class_key(&world, 2, true, None, None), (None, None));
        assert_eq!(
            class_key(&world, 2, false, Some(tile_of(&world, 2)), Some(1)),
            (None, None)
        );
        // The engineer (task 154): its Q, a mine, learnt, lays on a tile
        // beside it; its E, the satchel, not learnt yet, is refused.
        world.set_ranks_for_probe(0, [1, 0, 0, 0]);
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
            .find(|&tile| world.can_deploy(0, world::DeployKind::Mine, tile).is_ok())
            .expect("a free tile beside the engineer");
        assert_eq!(
            class_key(&world, 0, true, Some(beside), None),
            (
                Some(Order::Deploy {
                    kind: world::DeployKind::Mine,
                    x: beside.0,
                    y: beside.1
                }),
                None
            )
        );
        let (order, line) = class_key(&world, 0, false, Some(beside), None);
        assert_eq!(order, None);
        assert_eq!(
            line,
            Some(throw_refused(Refusal::NotLearnt)),
            "the satchel not learnt yet"
        );
        assert_eq!(
            class_key(&world, 0, true, None, None),
            (None, Some(deploy_refused(Refusal::CantDeployThere))),
            "no tile under the pointer"
        );
        // Space with no satchel out says so; nothing for another class.
        world.set_ranks_for_probe(0, [1, 0, 1, 0]);
        assert_eq!(
            detonate_key(&world, 0),
            (
                None,
                Some(crate::names::detonate_refused(Refusal::NoSatchels))
            )
        );
        assert_eq!(detonate_key(&world, 1), (None, None));
        // The soldier (task 124): E wants a rank of Stun Shot, then charges
        // one at the tile under the pointer, and E again while it charges
        // is refused; Q wants a rank of Frag Grenade, then throws at a
        // tile within range.
        assert_eq!(
            class_key(&world, 1, false, None, None),
            (None, Some(stun_shot_refused(Refusal::NotLearnt)))
        );
        world.set_ranks_for_probe(1, [0, 0, 1, 0]);
        let (x, y) = tile_of(&world, 1);
        assert_eq!(
            class_key(&world, 1, false, Some((x, y)), None),
            (Some(Order::StunShot { x, y }), None)
        );
        world.step(&[world::Command::StunShot { slot: 1, x, y }]);
        assert_eq!(
            class_key(&world, 1, false, Some((x, y)), None),
            (None, Some(stun_shot_refused(Refusal::AlreadyActive)))
        );
        // Called off again, for the keys below.
        world.soldiers[1] = Default::default();
        world.aboard.room.set_braced(1, false);
        let target = tile_of(&world, 1);
        assert_eq!(
            class_key(&world, 1, true, Some(target), None),
            (None, Some(throw_refused(Refusal::NoGrenadesYet)))
        );
        let mut events = Vec::new();
        world.set_ranks_for_probe(1, [1, 0, 1, 0]);
        assert_eq!(
            class_key(&world, 1, true, Some(target), None),
            (
                Some(Order::Throw {
                    x: target.0,
                    y: target.1
                }),
                None
            )
        );
        assert_eq!(
            class_key(&world, 1, true, None, None),
            (None, Some(throw_refused(Refusal::CantThrowThere)))
        );
        // The medic (task 153): Q drops a Heal Drone — refused
        // unlearnt — and E beams whoever is under the pointer, unlinks on
        // the one already held and on nobody.
        assert_eq!(world.set_class(2, world::Class::Medic), Ok(()));
        assert_eq!(
            class_key(&world, 2, true, None, None),
            (
                None,
                Some(crate::names::heal_drone_refused(Refusal::NotLearnt))
            )
        );
        world.set_ranks_for_probe(2, [1, 0, 1, 0]);
        assert_eq!(
            class_key(&world, 2, true, None, None),
            (Some(Order::HealDrone), None)
        );
        assert_eq!(
            class_key(&world, 2, false, None, None),
            (None, Some(beam_refused(Refusal::NoPatient))),
            "E on nobody with no beam on says so"
        );
        // A medic may beam itself (task 120): E over its own Bim links
        // the beam to it.
        assert_eq!(
            class_key(&world, 2, false, None, Some(2)),
            (Some(Order::Beam(Some(2))), None),
            "itself, since task 120"
        );
        // Crew member 0 beside it, and beamed.
        let at = world.aboard.room.bim_pos(2) + bims::math::vec2(t, 0.0);
        world.aboard.room.put_for_probe(0, at);
        world.step(&[]);
        assert_eq!(
            class_key(&world, 2, false, None, Some(0)),
            (Some(Order::Beam(Some(0))), None)
        );
        world.step(&[world::Command::Beam {
            slot: 2,
            patient: Some(0),
        }]);
        assert!(world.is_beaming(2));
        assert_eq!(
            class_key(&world, 2, false, None, Some(0)),
            (Some(Order::Beam(None)), None),
            "E on the one it holds unlinks"
        );
        assert_eq!(
            class_key(&world, 2, false, None, None),
            (Some(Order::Beam(None)), None),
            "and E on nobody unlinks while one is held"
        );
        // R switches his circle on, and off again once it is on, once its
        // rank is bought at the sixth level.
        world.award(2, world::class::LEVEL_XP[5], &mut events);
        world.set_ranks_for_probe(2, [1, 0, 1, 1]);
        assert_eq!(
            ranked_key(&world, 2, Action::Ability4, None, None),
            (Some(Order::HealingCircle(true)), None),
            "on"
        );
        world.step(&[world::Command::HealingCircle { slot: 2, on: true }]);
        assert_eq!(
            ranked_key(&world, 2, Action::Ability4, None, Some(0)),
            (Some(Order::HealingCircle(false)), None),
            "and off again, whoever is under the pointer"
        );
        assert_eq!(
            ranked_key(&world, 2, Action::Ability2, None, None),
            (None, None),
            "Triage is passive"
        );
        // And the engineer's keys are never a soldier's, nor the other
        // way about: an engineer pressing E with a tile throws a satchel
        // (task 154), not a Stun Shot, and a soldier's E is a Stun Shot.
        assert!(matches!(
            class_key(&world, 0, false, Some(beside), None).0,
            Some(Order::ThrowAt { satchel: true, .. })
        ));
        assert!(matches!(
            class_key(&world, 1, false, Some(beside), None).0,
            Some(Order::StunShot { .. })
        ));
    }

    /// And the tank's (task 155): Q raises the Riot Shield and puts it
    /// down, E raises the Reflect Barrier, R throws the Bastion, each from
    /// its first rank — none wants a tile or a crew member under the
    /// pointer, and none is anybody else's.
    #[test]
    fn the_tank_s_keys_raise_the_shield_and_the_barrier_and_throw_the_bastion() {
        let mut world = simulation_world(flyer(4), REFERENCE_MONEY, 4);
        assert_eq!(world.set_class(0, world::Class::Tank), Ok(()));
        assert_eq!(world.set_class(1, world::Class::Engineer), Ok(()));
        assert_eq!(world.set_class(2, world::Class::Soldier), Ok(()));
        assert_eq!(world.set_class(3, world::Class::Medic), Ok(()));
        // The soldier's Stun Shot wants its rank (task 124).
        world.set_ranks_for_probe(2, [0, 0, 1, 0]);
        // The shield wants its rank.
        assert_eq!(
            class_key(&world, 0, true, None, None),
            (None, Some(riot_shield_refused(Refusal::NotLearnt)))
        );
        world.set_ranks_for_probe(0, [1, 0, 0, 0]);
        let t = shipdesign::TILE as f32;
        let tile_of = |world: &world::World, who: u32| {
            let p = world.aboard.room.bim_pos(who as usize);
            ((p.x / t).floor() as i32, (p.y / t).floor() as i32)
        };
        // Q raises it wherever the pointer is, and puts it down again.
        assert_eq!(
            class_key(&world, 0, true, None, None),
            (Some(Order::RiotShield(true)), None)
        );
        assert_eq!(
            ranked_key(&world, 0, Action::Ability1, None, None),
            (Some(Order::RiotShield(true)), None),
            "Q is the same key through the ranked kit"
        );
        world.step(&[world::Command::RiotShield { slot: 0, on: true }]);
        assert_eq!(
            class_key(&world, 0, true, Some(tile_of(&world, 0)), Some(1)),
            (Some(Order::RiotShield(false)), None),
            "and down again, whatever is under the pointer"
        );
        // Broken, it waits its cooldown, and the Q box sweeps it.
        let now = world.mission_minutes();
        world.tanks[0].shield_up = false;
        world.tanks[0].shield_broke = Some(now);
        assert_eq!(
            class_key(&world, 0, true, None, None),
            (None, Some(riot_shield_refused(Refusal::ShieldRecharging)))
        );
        let q = &ability_boxes(&world, 0, &Keys::default())[0];
        assert!(q.cooldown > 0.0 && q.cooldown_whole == world.riot_shield_cooldown(0));
        world.tanks[0].shield_broke = None;
        // E wants its rank, and then cools down.
        assert_eq!(
            class_key(&world, 0, false, None, None),
            (None, Some(reflect_refused(Refusal::NotLearnt)))
        );
        let mut events = Vec::new();
        world.award(0, world::class::LEVEL_XP[5], &mut events);
        world.set_ranks_for_probe(0, [1, 0, 1, 0]);
        assert_eq!(
            class_key(&world, 0, false, None, None),
            (Some(Order::Reflect), None)
        );
        world.step(&[world::Command::Reflect { slot: 0 }]);
        assert_eq!(
            class_key(&world, 0, false, None, None),
            (None, Some(reflect_refused(Refusal::AlreadyActive)))
        );
        // R throws the Bastion once its rank is bought at the sixth level;
        // C, Plated, is passive.
        assert_eq!(
            ranked_key(&world, 0, Action::Ability4, None, None),
            (
                None,
                Some(crate::names::bastion_refused(Refusal::NotLearnt))
            )
        );
        world.set_ranks_for_probe(0, [1, 0, 1, 1]);
        assert_eq!(
            ranked_key(&world, 0, Action::Ability4, None, None),
            (Some(Order::Bastion), None)
        );
        assert_eq!(
            ranked_key(&world, 0, Action::Ability2, None, None),
            (None, None),
            "Plated is passive"
        );
        // And nobody else's keys are the tank's: the engineer lays a mine
        // (task 154), the soldier charges a Stun Shot, the medic beams.
        let t = shipdesign::TILE as f32;
        let p = world.aboard.room.bim_pos(2);
        let own = ((p.x / t).floor() as i32, (p.y / t).floor() as i32);
        assert!(matches!(
            class_key(&world, 2, false, Some(own), None).0,
            Some(Order::StunShot { .. })
        ));
        assert_eq!(
            class_key(&world, 3, false, None, None),
            (None, Some(beam_refused(Refusal::NoPatient)))
        );
        assert_eq!(
            class_key(&world, 1, true, None, None),
            (None, Some(deploy_refused(Refusal::CantDeployThere)))
        );
        // And a tank is refused a medic's and a soldier's rules.
        assert_eq!(world.can_reflect(1), Err(Refusal::NotATank));
        assert_eq!(world.can_riot_shield(3, true), Err(Refusal::NotATank));
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

    /// The commander's keys (task 129): Q a Battle Cry, E a Rally and R
    /// his reinforcements, each wanting its first rank; a classless crew
    /// member's do nothing.
    #[test]
    fn the_commanders_keys_are_his_ranked_kit_s() {
        let mut world = simulation_world(flyer(3), REFERENCE_MONEY, 3);
        assert_eq!(world.set_class(0, world::Class::Commander), Ok(()));
        assert_eq!(world.set_class(1, world::Class::Tank), Ok(()));
        world.step(&[]);
        // Q and E want their first rank.
        assert_eq!(
            class_key(&world, 0, true, None, None),
            (
                None,
                Some(crate::names::battle_cry_refused(Refusal::NotLearnt))
            )
        );
        assert_eq!(
            class_key(&world, 0, false, None, None),
            (None, Some(rally_refused(Refusal::NotLearnt)))
        );
        world.set_ranks_for_probe(0, [1, 0, 1, 0]);
        assert_eq!(
            ranked_key(&world, 0, Action::Ability1, None, None),
            (Some(Order::BattleCry), None)
        );
        assert_eq!(
            ranked_key(&world, 0, Action::Ability3, None, None),
            (Some(Order::Rally), None)
        );
        // C and R want their ranks.
        assert_eq!(
            ranked_key(&world, 0, Action::Ability2, None, None),
            (
                None,
                Some(crate::names::medivac_refused(Refusal::NotLearnt))
            )
        );
        assert_eq!(
            ranked_key(&world, 0, Action::Ability4, None, None),
            (
                None,
                Some(crate::names::reinforce_refused(Refusal::NotLearnt))
            )
        );
        // R, the ultimate, is bought from the sixth level.
        let mut events = Vec::new();
        world.award(0, world::class::LEVEL_XP[5], &mut events);
        world.set_ranks_for_probe(0, [1, 0, 1, 1]);
        assert_eq!(
            ranked_key(&world, 0, Action::Ability4, None, None),
            (Some(Order::Reinforce), None)
        );
        // And C at its first rank calls the medic.
        world.set_ranks_for_probe(0, [1, 1, 1, 1]);
        assert_eq!(
            ranked_key(&world, 0, Action::Ability2, None, None),
            (Some(Order::Medivac), None)
        );
        // A classless crew member's Q and E do nothing at all.
        assert_eq!(class_key(&world, 2, true, None, None), (None, None));
        assert_eq!(class_key(&world, 2, false, None, None), (None, None));
    }

    /// A stock of charges on a box is told the way Dota 2 tells one
    /// (`Face::charges`): with none left the whole box swept over the
    /// whole of the cooldown, and with some left and the next on its way
    /// the ring round the count part way. The engineer's satchels, a
    /// stock a first-level class has, where the medicine's two boxes
    /// were until task 120.
    #[test]
    fn a_spent_charge_sweeps_and_a_coming_one_rings() {
        use world::Charge;
        let (charges, cooldown) = (
            world::class::SATCHEL_CHARGES[0],
            world::class::SATCHEL_COOLDOWN[0],
        );
        let keys = Keys::default();
        let mut world = simulation_world(flyer(2), REFERENCE_MONEY, 2);
        assert_eq!(world.set_class(0, world::Class::Engineer), Ok(()));
        world.set_ranks_for_probe(0, [0, 0, 1, 0]);
        let bags = |world: &world::World| ability_boxes(world, 0, &keys).remove(2);
        let full = bags(&world);
        assert_eq!(full.count, Some(charges));
        assert!(full.cooldown == 0.0 && full.recharge.is_none());
        // None left: the whole box swept, over the whole of the cooldown.
        world.set_charges_for_probe(Charge::Satchel, 0);
        world.step(&[]);
        let empty = bags(&world);
        assert!(empty.short && !empty.ready());
        assert_eq!(empty.cooldown_whole, cooldown);
        assert!(empty.cooldown > 0.0 && empty.cooldown <= empty.cooldown_whole);
        assert!(empty.recharge.is_none(), "the sweep says it, not the ring");
        // Some left and the next on its way: ready, no seconds, and the
        // ring round the count part way.
        world.set_charges_for_probe(Charge::Satchel, charges - 1);
        for _ in 0..600 {
            world.step(&[]);
        }
        let some = bags(&world);
        assert_eq!(some.count, Some(charges - 1));
        assert!(some.ready());
        assert_eq!(some.cooldown, 0.0);
        let share = some.recharge.expect("the ring");
        assert!(share > 0.0 && share < 1.0, "{share}");
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

        // The engineer (task 127): four ranked abilities on Q C E R, the
        // key each is bound to, nothing learnt at rank nought — the
        // ultimate waiting on the sixth level — and a stock counted once a
        // rank is bought.
        let boxes = ability_boxes(&world, 0, &keys);
        assert_eq!(boxes.len(), 4);
        let keys_named: Vec<&str> = boxes.iter().map(|b| b.key.as_str()).collect();
        assert_eq!(keys_named, vec!["Q", "C", "E", "R"]);
        let names: Vec<&str> = boxes.iter().map(|b| b.name).collect();
        assert_eq!(
            names,
            vec!["Mine", "Healing Sentry", "Satchel Charge", "Sentry"]
        );
        assert!(
            boxes
                .iter()
                .all(|b| b.unlearnt && !b.ready() && b.count.is_none())
        );
        assert_eq!(boxes[3].locked, Some(6), "the ultimate's first rank");
        assert_eq!(boxes[0].mark, Some(Glyph::Mine));
        assert_eq!(boxes[3].mark, Some(Glyph::Sentry));
        world.set_ranks_for_probe(0, [1, 1, 1, 0]);
        let boxes = ability_boxes(&world, 0, &keys);
        assert_eq!(boxes[0].count, Some(world::class::MINE_CHARGES[0]));
        assert_eq!(
            boxes[1].count,
            Some(world::class::HEALING_SENTRY_CHARGES[0])
        );
        assert_eq!(boxes[2].count, Some(world::class::SATCHEL_CHARGES[0]));
        assert!(boxes[..3].iter().all(|b| b.ready()));

        // The soldier (task 124): its four ranked abilities, nothing
        // learnt at rank nought, a "+" on each a point could buy now —
        // the first level's point on Q, C and E, not the ultimate's — and
        // the pips its rank.
        let boxes = ability_boxes(&world, 1, &keys);
        let names: Vec<&str> = boxes.iter().map(|b| b.name).collect();
        assert_eq!(
            names,
            vec!["Frag Grenade", "Weak Spot", "Stun Shot", "Rampage"]
        );
        assert!(boxes.iter().all(|b| b.unlearnt && !b.ready()));
        let plus: Vec<bool> = boxes.iter().map(|b| b.plus).collect();
        assert_eq!(plus, vec![true, true, true, false]);
        assert_eq!(boxes[3].locked, Some(6));
        assert_eq!(boxes[0].rank, Some((0, 4)));
        world.set_ranks_for_probe(1, [1, 0, 1, 0]);
        let boxes = ability_boxes(&world, 1, &keys);
        assert_eq!(boxes[0].rank, Some((1, 4)));
        assert_eq!(boxes[0].count, Some(world::class::GRENADE_CHARGES[0]));
        assert_eq!(boxes[0].mark, Some(Glyph::FragGrenade));
        assert!(boxes[0].ready() && boxes[2].ready() && !boxes[2].on);
        assert!(boxes.iter().all(|b| !b.plus), "no point left");
        assert!(boxes[0].foot.contains("Next, rank 2"));
        let p = world.aboard.room.bim_pos(1);
        let t = shipdesign::TILE as f32;
        world.step(&[world::Command::StunShot {
            slot: 1,
            x: (p.x / t).floor() as i32,
            y: (p.y / t).floor() as i32,
        }]);
        assert!(ability_boxes(&world, 1, &keys)[2].on, "charging now");
        // The key's own rank-up: the order, and the world's refusal said.
        assert_eq!(
            rank_up(&world, 1, RankUp { slot: 1 }),
            (
                None,
                Some(crate::names::rank_refused(Refusal::NoSkillPoint))
            )
        );
        let mut events = Vec::new();
        world.award(1, world::class::LEVEL_XP[2], &mut events);
        assert_eq!(
            rank_up(&world, 1, RankUp { slot: 1 }),
            (Some(Order::RankUp { ability_slot: 1 }), None)
        );
        assert_eq!(
            ranked_key(&world, 1, Action::Ability2, None, None),
            (None, None),
            "Weak Spot is passive"
        );
        assert_eq!(
            ranked_key(&world, 1, Action::Ability4, None, None),
            (
                None,
                Some(crate::names::rampage_refused(Refusal::NotLearnt))
            )
        );

        // Every class has a name and a tip on every box, and the
        // primary one is the level-three key for all of them. A class
        // with keys of its own past Q and E has a box for each of
        // them (feature 86): the medic's carry.
        for class in world::Class::ALL {
            if class == world::Class::None {
                continue;
            }
            // A ranked kit's four are pinned above.
            if world::class::ranked(class) {
                continue;
            }
            let mut world = simulation_world(flyer(1), REFERENCE_MONEY, 1);
            assert_eq!(world.set_class(0, class), Ok(()));
            let boxes = ability_boxes(&world, 0, &keys);
            let wanted = match class {
                world::Class::Medic => 5,
                _ => 4,
            };
            assert_eq!(boxes.len(), wanted, "{class:?}");
            // The four slots first, Q C E R, the second and fourth
            // empty for every class (task 123).
            let slots: Vec<Option<Action>> = boxes[..4].iter().map(|b| b.action).collect();
            assert_eq!(slots, Action::ABILITIES.map(Some).to_vec(), "{class:?}");
            assert!(boxes[1].mark == None && boxes[3].mark == None);
            assert!(
                boxes
                    .iter()
                    .filter(|b| b.mark != None)
                    .all(|b| !b.name.is_empty() && !b.tip.is_empty())
            );
            assert_eq!(boxes[0].locked, Some(3), "{class:?}'s Q is its third");
            assert_eq!(boxes[2].locked, None, "{class:?}'s E is its first");
            // And the keys are the Controls page's own, in the order
            // the bar lays them out.
            assert!(
                boxes
                    .iter()
                    .filter(|b| b.mark != None)
                    .all(|b| !b.key.is_empty())
            );
        }

        // The medic (task 153): his four ranked abilities, the ultimate
        // waiting on the sixth level, and his carry after them, which
        // wants no level at all.
        let mut world = simulation_world(flyer(1), REFERENCE_MONEY, 1);
        assert_eq!(world.set_class(0, world::Class::Medic), Ok(()));
        let boxes = ability_boxes(&world, 0, &keys);
        let named: Vec<&str> = boxes.iter().map(|b| b.name).collect();
        assert_eq!(
            named,
            vec![
                "Heal Drone",
                "Triage",
                "Heal Beam",
                "Healing Circle",
                names::CARRY
            ]
        );
        assert_eq!(boxes[0].mark, Some(Glyph::HealDrone));
        assert_eq!(boxes[1].mark, Some(Glyph::Triage));
        assert_eq!(boxes[3].mark, Some(Glyph::HealingCircle));
        assert_eq!(boxes[3].locked, Some(6), "the ultimate's first rank");
        assert_eq!(boxes[4].locked, None, "the carry wants no level");
        assert!(boxes[..4].iter().all(|b| b.unlearnt && !b.ready()));
        world.set_ranks_for_probe(0, [1, 1, 1, 0]);
        let boxes = ability_boxes(&world, 0, &keys);
        assert!(boxes[0].ready(), "the drone learnt and ready");
        assert_eq!(boxes[2].count, Some(1), "one patient the beam could take");
    }

    /// Resting on a box says whom the cast would reach (feature 86):
    /// a commander's Battle Cry the crew about him, and an engineer's
    /// nobody at all.
    #[test]
    fn a_box_says_which_bims_its_cast_reaches() {
        let mut world = simulation_world(flyer(3), REFERENCE_MONEY, 1);
        assert_eq!(world.set_class(0, world::Class::Commander), Ok(()));
        world.step(&[]);
        // The Battle Cry reaches whoever stands in the aura, the commander
        // among them — the whole of a small room, which is what a test
        // room is.
        let lifted = affected_by(&world, 0, Action::Ability1);
        assert!(lifted.contains(&0), "the aura covers him: {lifted:?}");
        // An engineer's keys reach nobody: they are laid on the deck.
        let mut world = simulation_world(flyer(3), REFERENCE_MONEY, 1);
        assert_eq!(world.set_class(0, world::Class::Engineer), Ok(()));
        assert!(affected_by(&world, 0, Action::Ability1).is_empty());
        assert!(affected_by(&world, 0, Action::Ability3).is_empty());
    }
}

#[cfg(test)]
mod rank_up_tests {
    use super::*;

    /// The four slots' boxes and a carry box after them, the way the hero
    /// panel lays a medic's out: Q, C, E, R and G.
    fn boxes() -> Vec<AbilityBox> {
        let mut row: Vec<AbilityBox> = Action::ABILITIES
            .iter()
            .zip(["Q", "C", "E", "R"])
            .map(|(&action, key)| AbilityBox::empty(key.to_string(), action))
            .collect();
        row[0] = AbilityBox {
            name: "Heal Drone",
            tip: "tip".to_string(),
            mark: Some(Glyph::HealDrone),
            ..AbilityBox::empty("Q".to_string(), Action::Ability1)
        };
        row[2] = AbilityBox {
            name: "Heal Beam",
            tip: "tip".to_string(),
            mark: Some(Glyph::HealBeam),
            ..AbilityBox::empty("E".to_string(), Action::Ability3)
        };
        row.push(AbilityBox {
            name: "Carry",
            tip: "tip".to_string(),
            mark: Some(Glyph::Carry),
            ..AbilityBox::empty("G".to_string(), Action::Carry)
        });
        row
    }

    /// A left click on box `which` of the row, with `modifiers` held
    /// throughout, through a real egui context and no window: the
    /// rank-up the row said, and whether the deck would have taken the
    /// click as its own — the game screen's own test, `Pointer::on`
    /// over the whole window, read before the frame's panels as the
    /// screen reads it.
    fn click_box(which: usize, modifiers: egui::Modifiers) -> (Option<RankUp>, bool) {
        let at = ROW
            + egui::vec2(
                which as f32 * (ABILITY_SIDE + 4.0) + ABILITY_SIDE / 2.0,
                ABILITY_SIDE / 2.0,
            );
        click_at(at, modifiers)
    }

    /// Where the row stands.
    const ROW: egui::Pos2 = egui::pos2(100.0, 100.0);

    /// [`click_box`] at any point of the window.
    fn click_at(at: egui::Pos2, modifiers: egui::Modifiers) -> (Option<RankUp>, bool) {
        let ctx = egui::Context::default();
        let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(800.0, 600.0));
        let origin = ROW;
        let button = |pressed| egui::Event::PointerButton {
            pos: at,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers,
        };
        let row = boxes();
        let mut asked = None;
        let mut deck = false;
        // egui 0.36 hears the modifiers as an event, and keeps them.
        for events in [
            vec![egui::Event::ModifiersChanged(modifiers)],
            vec![egui::Event::PointerMoved(at)],
            vec![button(true)],
            vec![button(false)],
            vec![],
        ] {
            // `run_ui`, so the whole window is the root's to hand out and
            // a pointer over no panel is the canvas's, as in the game.
            let input = egui::RawInput {
                screen_rect: Some(screen),
                events,
                ..Default::default()
            };
            let mut output = ctx.run_ui(input, |root| {
                let ctx = root.ctx().clone();
                let pointer = Pointer::read(&ctx);
                deck |= (pointer.primary_pressed || pointer.primary_released)
                    && pointer.on(rect_of(screen)).is_some();
                egui::Area::new(egui::Id::new("row"))
                    .fixed_pos(origin)
                    .show(&ctx, |ui| {
                        ui.horizontal(|ui| {
                            asked = asked.or(ability_row(ui, &row).rank_up);
                        });
                    });
            });
            output.textures_delta.clear();
        }
        (asked, deck)
    }

    /// Ctrl+E and a Ctrl-click on the third slot's box are one rank-up
    /// request (task 123), the same value handed to the one `rank_up`.
    #[test]
    fn ctrl_e_and_a_ctrl_click_on_the_third_box_are_the_same_rank_up() {
        let keys = Keys::default();
        let ctrl = egui::Modifiers::CTRL;
        let e = [egui::Event::Key {
            key: egui::Key::E,
            physical_key: Some(egui::Key::E),
            pressed: true,
            repeat: false,
            modifiers: ctrl,
        }];
        let by_key = rank_up_by_key(&keys, &e, ctrl);
        assert_eq!(by_key, Some(RankUp { slot: 2 }));
        let (by_click, _) = click_box(2, ctrl);
        assert_eq!(by_click, by_key);
        // Every slot's box, the empty ones as well.
        for which in 0..4 {
            assert_eq!(click_box(which, ctrl).0, Some(RankUp { slot: which }));
        }
        // And the one place both go asks the world (task 124): a crew
        // member with no class is told so.
        let world = world::fixture::simulation_world(
            shipdesign::fixture::flyer(1),
            world::fixture::REFERENCE_MONEY,
            1,
        );
        assert_eq!(
            rank_up(&world, 0, RankUp { slot: 2 }),
            (None, Some(crate::names::rank_refused(Refusal::NoClass)))
        );
    }

    /// A plain click on a slot's box asks nothing, and nor does a
    /// Ctrl-click on a box that is not a slot's — the carry here.
    #[test]
    fn a_plain_click_or_a_box_that_is_no_slot_asks_no_rank_up() {
        assert_eq!(click_box(2, egui::Modifiers::NONE).0, None);
        assert_eq!(click_box(0, egui::Modifiers::NONE).0, None);
        assert_eq!(click_box(4, egui::Modifiers::CTRL).0, None);
        assert_eq!(rank_up_by_click(Some(Action::Carry), true, true), None);
        assert_eq!(rank_up_by_click(Some(Action::Ability4), true, false), None);
        assert_eq!(
            rank_up_by_click(Some(Action::Ability4), true, true),
            Some(RankUp { slot: 3 })
        );
    }

    /// A Ctrl-click on a box never reaches the deck: the pointer is over
    /// the hero panel, so the canvas is not handed the press and gives
    /// no order and picks nothing up. A click beside the row is the
    /// deck's, which is what says the test can tell.
    #[test]
    fn a_ctrl_click_on_a_box_is_no_deck_order() {
        for which in 0..5 {
            let (_, deck) = click_box(which, egui::Modifiers::CTRL);
            assert!(!deck, "box {which}");
        }
        let (asked, deck) = click_at(egui::pos2(50.0, 50.0), egui::Modifiers::CTRL);
        assert!(deck && asked.is_none(), "beside the row is the deck's");
    }
}

#[cfg(test)]
mod attack_key_tests {
    use super::*;

    /// The Attack key arms the pointer on its first press, banner down
    /// or not, thinks better of it while armed, and **takes the banner
    /// up** on that second press when one is down, which is the key's
    /// way of letting the crew go back to their errands.
    #[test]
    fn the_attack_key_arms_the_pointer_and_takes_a_banner_back_up() {
        use world::Standing;
        // Nothing down: armed, so the click after it picks the ground.
        assert_eq!(attack_key(Standing::Follow, false, false), (None, true));
        // Armed already: thought better of, and nothing given.
        assert_eq!(attack_key(Standing::Follow, true, false), (None, false));
        // The map is up, which has no deck to put a banner on.
        assert_eq!(attack_key(Standing::Follow, false, true), (None, false));
        // A banner down: still armed at once, with nothing given — the
        // click after it moves the banner (or, on its own tile, takes
        // it up). One press, the crosshair.
        let order = Standing::Attack { tile: (7, 9) };
        assert_eq!(attack_key(order, false, false), (None, true));
        // Armed over a banner: the same order back, which the world
        // reads as a release, and the pointer put away.
        assert_eq!(attack_key(order, true, false), (Some(order), false));
        // A retreat is the Retreat key's to call off, not this one's.
        assert_eq!(attack_key(Standing::Retreat, false, false), (None, true));
    }
}

/// Where a probe opened, printed, so a layout on its screenshot can be
/// asked for again (feature 112): the station alongside — its kind, its
/// seed and the plan it was built on — or the town set down at, with its
/// seed, biome and population. What `BIMS_STATION_SEED` and the
/// `nav_map_of_a_station` probe take.
fn say_where(session: &Session) {
    let Some(game) = session.game.as_ref() else {
        return;
    };
    let world = &game.world;
    let Some(id) = world.ship.state.station() else {
        return;
    };
    match world::surface_body(id) {
        Some(body) => {
            if let Some(surface) = world.surfaces.iter().find(|s| s.body == body) {
                println!(
                    "town: seed {} biome {:?} population {}",
                    surface.map_seed, surface.biome, surface.population
                );
            }
        }
        None => {
            if let Some(station) = world.station(id) {
                println!(
                    "station: {:?} seed {} plan {:?}",
                    station.kind, station.map_seed, station.plan
                );
            }
        }
    }
}

#[cfg(test)]
mod control_tests {
    use super::*;

    /// A control order goes at once for a walk or a trigger changed, and
    /// for the aim alone only once it has turned past `AIM_STEP` and
    /// `CONTROL_EVERY` has gone by since the last (task 144).
    #[test]
    fn a_control_order_goes_for_a_walk_or_a_trigger_at_once_and_for_the_aim_now_and_then() {
        let last = Some(((None, 1000, false, false), 10.0));
        assert!(control_due(None, (None, 0, false, false), 0.0), "the first");
        assert!(
            control_due(last, (Some(0), 1000, false, false), 10.001),
            "a key down"
        );
        assert!(
            control_due(last, (None, 1000, true, false), 10.001),
            "the trigger"
        );
        assert!(
            control_due(last, (None, 1000, false, true), 10.001),
            "the sprint (task 150)"
        );
        assert!(
            !control_due(last, (None, 1000, false, false), 11.0),
            "nothing changed"
        );
        assert!(
            !control_due(last, (None, 1000 + AIM_STEP - 1, false, false), 11.0),
            "a hair"
        );
        assert!(
            !control_due(last, (None, 2000, false, false), 10.01),
            "too soon"
        );
        assert!(
            control_due(last, (None, 2000, false, false), 10.0 + CONTROL_EVERY),
            "turned, in time"
        );
        // Across nought the short way round.
        let north = Some(((None, 65530, false, false), 0.0));
        assert!(
            !control_due(north, (None, 10, false, false), 1.0),
            "16 codes apart"
        );
        assert!(
            control_due(north, (None, 40, false, false), 1.0),
            "42 codes apart"
        );
    }
}
