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
use bims::room::{HIT_DOOR, HIT_DROPPED, HIT_SHIP_DOOR};
use flight::Target;
use physics::{Facing, ResourceId};
use ship::Session;
use ship::game::ViewMode;
use shipdesign::Storage;
use shipdesign::parts::Rotation;
use world::{ShipState, Speed, Where, WorldEvent};
use worldgen::Node;

use super::designer::{Cart, Net, Order, ShipSession, trade_rows};
use crate::canvas::{Pointer, canvas_painter, paint_shapes, rect_of, root_ui, zoom_factor};
use crate::crew::{
    Actions, Body, CLICK_SLOP, Craft, CrewPanels, GearOrder, Hold, Near, Open, ResearchView, Tool,
    UpgradeView,
};
use crate::format::{euros, grouped, roman, spell};
use crate::keys::{Action, Keys};
use crate::names::*;
use crate::screens::room::{panel_frame, tray_frame};
use crate::settings::{Sheet, settings_sheet};
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

/// What the map writes over the ship, before where it is.
const HERE_TAG: &str = "You";
/// How far above the ship's mark on the map its words sit: clear of the
/// reticle `ship::world_paint` draws round it, ring and ticks.
const HERE_LIFT: f32 = 36.0;

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
/// The press decides which way the stroke goes, from the first rock: a
/// stroke that began on a bare rock marks and skips the marked, one that
/// began on a marked rock unmarks and skips the bare. `touched` is every
/// tile the stroke has already sent an order for, kept here because the
/// order lands on a step and a paused world would otherwise be asked to
/// toggle the same rock every frame the pointer rests on it.
struct MarkDrag {
    marking: bool,
    /// The tile the pointer was over last frame, to walk the tiles between
    /// it and this one: a fast pull skips tiles between frames.
    last: (i32, i32),
    touched: Vec<(i32, i32)>,
}

/// The tiles a straight pull of the pointer from `from` to `to` crosses,
/// both ends included: one a step along the longer axis, the other axis
/// rounded to keep beside the line. What a stroke marks, so a rock
/// between two frames' positions is not skipped.
fn tiles_between(from: (i32, i32), to: (i32, i32)) -> Vec<(i32, i32)> {
    let (dx, dy) = (to.0 - from.0, to.1 - from.1);
    let steps = dx.abs().max(dy.abs());
    (0..=steps)
        .map(|i| {
            if steps == 0 {
                return from;
            }
            let t = i as f32 / steps as f32;
            (
                from.0 + (dx as f32 * t).round() as i32,
                from.1 + (dy as f32 * t).round() as i32,
            )
        })
        .collect()
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
    /// A drag with the Mine tool in hand: the rocks are marked, or
    /// unmarked, as the pointer is dragged over them.
    mark_drag: Option<MarkDrag>,
    pan_from: Option<Vec2>,
    /// The Esc sheet, if it is up, and which page.
    sheet: Option<Sheet>,
    size: Vec2,
    /// The speed Space pauses from, for Space to go back to.
    resume: Speed,
    /// The smooth fog over the deck, as a texture — see `fogmap`.
    fog: crate::fogmap::FogTexture,
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
}

pub struct GamePlugin;

impl Plugin for GamePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(Screen::Game), open)
            .add_systems(EguiPrimaryContextPass, frame.run_if(in_state(Screen::Game)));
    }
}

fn open(
    mut commands: Commands,
    session: Option<Res<ShipSession>>,
    launch: Res<Launch>,
    window: Single<&Window>,
) {
    let size = Vec2::new(window.width().max(64.0), window.height().max(64.0));
    let (slot, players) = match session {
        Some(session) => (session.0.editor.local, session.0.editor.players),
        None => {
            // No design phase in front of this: the simulation, on the
            // playtest ship. The `test` command is the same somewhere else
            // each time — a random seed, and a dock somebody lives on picked
            // at random across that galaxy.
            let (seed, spawn) = match *launch {
                Launch::Test => {
                    let seed = super::room::rand_seed();
                    let roll = super::room::rand_seed();
                    (seed, ship::session::pick_dock(seed, 0, roll))
                }
                _ => (world::data::DEFAULT_SEED, None),
            };
            // The `combat` command is the fight: the combat ship's five crew,
            // a different gun in each hand, docked at the spawn rebuilt as
            // the arena and turned against them — its people enemies, and
            // more of them — so a recruited crew member has somebody to
            // shoot at and somewhere to do it. `Session::combat` is all of
            // that; `BIMS_FIGHT` stages the two a few tiles apart on top.
            // The `test` command is on the combat ship too, with one crew
            // member — four bunks to spare — and a mercenary for hire at
            // the dock whatever the roll said, so a hire can be looked at.
            let mut session = match *launch {
                Launch::Combat => Session::combat(seed, size.x, size.y),
                Launch::Test => {
                    let design = shipdesign::fixture::combat_ship();
                    let mut session =
                        Session::simulate_on(design, 1, seed, 0, spawn, size.x, size.y);
                    session.mercenary_for_probe();
                    session
                }
                _ => Session::simulate(seed, 0, spawn, size.x, size.y),
            };
            if crate::dev::at_belt() {
                session.hold_at_belt_for_probe();
            }
            if crate::dev::fight() {
                session.stage_fight_for_probe();
            }
            if let Some(n) = crate::dev::lamps_out() {
                session.shoot_lamps_for_probe(n);
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
            commands.insert_resource(ShipSession(session));
            out
        }
    };
    commands.insert_resource(GameScreen {
        net: Net { slot, players },
        panels: None,
        aimed: None,
        pending: None,
        relieve: false,
        trading: crate::dev::trade(),
        armoury_wanted: crate::dev::armoury(),
        tab_took_focus: false,
        cart: Cart::new(),
        backlog: 0.0,
        log: Vec::new(),
        hover_at: None,
        marquee_from: None,
        order_from: None,
        mark_drag: None,
        pan_from: None,
        sheet: None,
        size: Vec2::ZERO,
        resume: Speed::Real,
        fog: crate::fogmap::FogTexture::default(),
        galaxy_up: false,
        galaxy: None,
        galaxy_list: lobby::draw::DrawList::new(),
        galaxy_size: Vec2::ZERO,
        picked_star: None,
    });
}

/// What a thing on the map is called. A kind and a number, because a kind
/// is a fixed table and an identity is a number.
fn node_name(session: &Session, node: Node) -> String {
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

fn frame(
    mut contexts: EguiContexts,
    mut screen: ResMut<GameScreen>,
    mut session: ResMut<ShipSession>,
    time: Res<Time>,
    mut sounds: ResMut<Sounds>,
    mut bindings: ResMut<Keys>,
    mut commands: Commands,
) -> Result {
    let ctx = contexts.ctx_mut()?.clone();
    let screen = &mut *screen;
    let session = &mut session.0;
    let keys_now = *bindings;
    let now = ctx.input(|i| i.time);
    let dt = time.delta_secs().min(super::designer::MAX_FRAME_DT) as f64;
    let mut root = root_ui(&ctx);

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
    let times = session
        .game
        .as_ref()
        .map(|g| g.world.effective_speed().multiplier())
        .unwrap_or(0) as f64;
    screen.backlog += dt * session.steps_per_second() * times;
    let mut steps = 0;
    while screen.backlog >= 1.0 && steps < MAX_STEPS_PER_FRAME {
        session.world_step();
        screen.backlog -= 1.0;
        steps += 1;
    }
    if screen.backlog > MAX_STEPS_PER_FRAME as f64 {
        screen.backlog = 0.0; // gave up catching up
    }
    // What just happened. An event is a thing that happened once, so the
    // list is drained after it is read.
    if let Some(game) = &mut session.game {
        for event in game.events.drain(..) {
            if let Some(line) = event_line(event) {
                screen.log.push(line);
            }
            // The engines catch as the ship pushes off its berth, or as a
            // trip begins from a hold; `Sounds` plays one ignition for
            // the undocking and the departure that follows it.
            if matches!(
                event,
                WorldEvent::Undocking { .. } | WorldEvent::Departed { .. }
            ) {
                sounds.engine_start(&mut commands);
            }
        }
        while screen.log.len() > LOG_LINES {
            screen.log.remove(0);
        }
        // What the steps sounded like: the crew's room, and the station's
        // beside it while the decks are joined — its doors and its galley
        // are on the same picture.
        for cued in game.world.aboard.room.take_cues() {
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
        // The beds: a station's hum while tied up with the rooms joined,
        // the ship's own otherwise, and the engines while they burn —
        // and the world is moving, since a paused burn is silent.
        match game.world.ship.state {
            ShipState::Docked { .. } | ShipState::CastingOff { .. } => sounds.want(Bed::Station),
            _ => sounds.want(Bed::Ship),
        }
        if game.world.effort().engines > 0 && game.world.effective_speed().multiplier() > 0 {
            sounds.want(Bed::Engine);
        }
    }

    // The crew's panels, built once there is a room to read, and rebuilt
    // for whoever is there now: a docking brings the station's residents
    // into the ship's room and leaving takes them out.
    let crew_now = session.room_ref().map(|r| r.crew_count()).unwrap_or(0);
    match &mut screen.panels {
        None => {
            let mut panels = CrewPanels::new(crate::crew::player(), crew_now);
            if let Some(room) = session.room() {
                room.select_group(1);
            }
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
    }
    let local = screen.net.slot;

    // --- an order on its way to the helm ---------------------------------------
    // The strip at the top walked the crew member to the seat; the order
    // goes through the frame they get there, and the post is lifted once
    // the world has stepped with it — a step later, so they are still at
    // the seat when the command lands.
    let mut orders: Vec<Order> = Vec::new();
    if let Some(game) = &mut session.game {
        if screen.relieve && steps > 0 {
            game.world.stand_down(local);
            screen.relieve = false;
        }
        if let Some(order) = screen.pending
            && game.world.at_the_helm(local)
        {
            orders.push(match order {
                HelmOrder::Fly(aim) => Order::Fly(aim.target()),
                HelmOrder::Stop => Order::Stop,
                HelmOrder::Jump(star) => Order::Jump(star),
            });
            if matches!(order, HelmOrder::Fly(_)) {
                screen.aimed = None;
            }
            screen.pending = None;
            screen.relieve = true;
        }
    }

    // --- the canvas ------------------------------------------------------------
    let canvas = rect_of(root.available_rect_before_wrap());
    let size = canvas.size();
    if size != screen.size && size.x > 0.0 && size.y > 0.0 {
        // The first size the canvas is actually seen at is the one the
        // world should have opened on: the whole hull, in view.
        if screen.size == Vec2::ZERO {
            session.fit(size.x, size.y);
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
        if canvas.size() != screen.galaxy_size {
            screen.galaxy_size = canvas.size();
            chart.preview.resize(canvas.size().x, canvas.size().y);
        }
    }
    let panels = screen.panels.as_mut().unwrap();

    // The tool in the pointer's hand. Mine only means anything at a
    // mining site, in the ship view: away from one it is put down.
    let (at_site, marked) = session
        .game
        .as_ref()
        .and_then(|g| g.world.site_here())
        .map(|s| (true, s.marked.len()))
        .unwrap_or((false, 0));
    if panels.tool == Some(Tool::Mine) && !at_site {
        panels.tool = None;
    }
    let marking = panels.tool == Some(Tool::Mine) && !map_up;
    if let Some(game) = &mut session.game {
        game.marking = marking;
    }
    if !marking {
        screen.mark_drag = None;
    }
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
        at_site,
        marked,
        reachable: session.room_ref().map(|r| r.rocks_reachable()).unwrap_or(0),
        clear: false,
        overlay: session.game.as_ref().map(|g| g.overlay).unwrap_or_default(),
        crafts: crafts(session),
        keep: Vec::new(),
        at_rest: session.game.as_ref().is_some_and(|g| g.world.at_rest()),
        free: {
            let mut free = [0u32; shipdesign::CARGO_SLOTS];
            if let Some(game) = &session.game {
                for &id in ResourceId::ALL.iter() {
                    free[id as usize] = game.world.free(id);
                }
            }
            free
        },
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
        // With a blueprint in hand the pointer is about laying it out: a
        // click on a tile it would go on sends the site through the seam,
        // one it would not says why in the log — the readout at the top
        // left says it already — and a right-click puts the tool down.
        if let Some(kind) = building {
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
        } else if marking {
            // With the Mine tool in hand the pointer is about the rocks and
            // nothing else: a press on one marks it, or unmarks it, through
            // the seam like any order, and dragging on from there does
            // the same to every rock the pointer crosses (`MarkDrag`); a
            // right-click puts the tool down. The system's cursor goes
            // and a pick is drawn in its place, below.
            if let Some(p) = on_canvas {
                ctx.set_cursor_icon(egui::CursorIcon::None);
                if let Some(game) = &session.game
                    && let Some(site) = game.world.site_here()
                {
                    let here = game.tile_at(p.x, p.y);
                    if pointer.primary_pressed && site.at(here.0, here.1).is_some() {
                        screen.mark_drag = Some(MarkDrag {
                            marking: !site.is_marked(here.0, here.1),
                            last: here,
                            touched: Vec::new(),
                        });
                    }
                    if let Some(drag) = &mut screen.mark_drag
                        && pointer.primary_down
                    {
                        for (x, y) in tiles_between(drag.last, here) {
                            let rock = site.at(x, y).is_some();
                            let wants = site.is_marked(x, y) != drag.marking;
                            if rock && wants && !drag.touched.contains(&(x, y)) {
                                drag.touched.push((x, y));
                                orders.push(Order::Mark { x, y });
                            }
                        }
                        drag.last = here;
                    }
                }
                if pointer.secondary_pressed {
                    panels.tool = None;
                }
            }
            if !pointer.primary_down {
                screen.mark_drag = None;
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
                        if let Some(who) = who
                            && !room.fetch(who, id)
                        {
                            screen.log.push(PICK_UP_REFUSED.into());
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
                            let fixture = room.drag_end(rx, ry);
                            let moved = (p - from).length() > CLICK_SLOP;
                            if fixture != 0 && !moved {
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
                            let dragged = (p - from).length() > CLICK_SLOP;
                            let code = room.order_drag_end(rx, ry, dragged);
                            if let Some(refused) = order_refused(code) {
                                screen.log.push(refused.into());
                            }
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
                    if let Some(room) = session.room() {
                        room.select_group(1);
                    }
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
                } else if keys_now.pressed(i, Action::Recruit)
                    && let Some(room) = session.room()
                {
                    room.toggle_recruited();
                }
                // Tab: the inventory of the crew member you steer.
                if keys_now.pressed(i, Action::Inventory) {
                    panels.toggle_inventory();
                }
            }
            if i.key_pressed(egui::Key::Escape) {
                if panels.tool.is_some() {
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
        STATE_NAMES
            .get(state as usize)
            .copied()
            .unwrap_or("Holding")
            .to_string()
    };
    let speed = game.world.trip_state().map(|s| s.speed).unwrap_or(0.0);
    let degrees = (game.world.ship.heading.to_degrees() + 360.0) % 360.0;
    // What the pointer is over, in the ship view: the part under it and the
    // tile, and what the room aboard makes of the same point.
    let rock_under = game.hover.and_then(|(x, y)| {
        let site = game.world.site_here()?;
        Some((site.at(x, y)?, site.is_marked(x, y), x, y))
    });
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
        Some(_) if !map_up && rock_under.is_some() => {
            let (kind, marked, x, y) = rock_under.unwrap();
            let name = ROCK_NAMES
                .get(kind.code() as usize)
                .copied()
                .unwrap_or("Rock");
            (
                format!("{name} · {x}, {y}"),
                if marked { "Marked to be mined" } else { "" }.to_string(),
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
                // The recruited panel says why the gun has gone quiet
                // while a blade is at the player's crew member: the same
                // panel, since nothing on the screen may grow — the left
                // stack already fills a short window, and a panel added
                // to it shoves the whole stack up over this row.
                if let Some(room) = session.room_ref()
                    && room.is_recruited()
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
                                    egui::RichText::new("Recruited — orders only")
                                        .color(theme::ACCENT),
                                );
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
    if actions.clear {
        orders.push(Order::ClearMarks);
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
    for order in actions.research_orders.drain(..) {
        orders.push(Order::Research(order));
    }
    if let Some(game) = &mut session.game {
        game.overlay = actions.overlay;
    }
    for order in orders.drain(..) {
        screen.net.order(session, order);
    }

    // Clear of the left stack on a narrow window, rather than over it.
    let side_x = (canvas.max.x - 270.0).max(canvas.min.x + 290.0);
    egui::Area::new(egui::Id::new("game-side"))
        .fixed_pos(egui::pos2(side_x, canvas.min.y + 10.0))
        .order(egui::Order::Middle)
        .show(&ctx, |ui| {
            if let Some(room) = session.room() {
                let mut drawn = false;
                let frame = panel_frame();
                frame.show(ui, |ui| {
                    ui.set_min_width(240.0);
                    drawn = panels.side(ui, room, &name);
                    if !drawn {
                        ui.label(
                            egui::RichText::new("Click a Bim to look at it.").color(theme::MUTED),
                        );
                    }
                });
            }
        });

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
            world.aboard.room.send_to(who, at);
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
            })
        });
        // The station's key: the desk's row walked the Bim over, and the
        // take goes through the seam the frame they are within reach —
        // or is forgotten if the key goes or the ship does.
        if let Some(who) = panels.key_requested {
            if !world.key_at_the_dock() {
                panels.key_requested = None;
            } else if world.key_in_reach(who) {
                orders.push(Order::Gear(GearOrder::TakeKey { who }));
                panels.key_requested = None;
            }
        }
        let room = &mut world.aboard.room;
        panels.loot_window(&ctx, room, &name);
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
    for order in orders.drain(..) {
        screen.net.order(session, order);
    }
    settings_sheet(&ctx, &mut screen.sheet, &mut sounds.mix, &mut bindings);

    // --- painting ------------------------------------------------------------------
    let view = View {
        scale: session.view_scale(),
        offset: {
            let (x, y) = session.view_offset();
            Vec2::new(x, y)
        },
    };
    let painter = canvas_painter(&ctx, canvas);
    if galaxy_up && let Some(chart) = &screen.galaxy {
        // The chart in place of the map: the lobby's picture, in pixels,
        // and the names of the star the ship is at and the one picked over
        // them, since the buffer holds no words.
        chart.paint(&mut screen.galaxy_list);
        paint_shapes(&painter, canvas, View::PIXELS, screen.galaxy_list.shapes());
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
        paint_shapes(&painter, canvas, view, session.render());
        // Where you are, in words, over the reticle the map draws round the
        // ship — `You`, and the berth or the place — in the colour the
        // player's own things are, the way the chart tags the star the ship
        // is at. The ship is the map's origin, wherever it has been panned
        // to; off the canvas the words go with it and the strip still says.
        if map_up && session.game.is_some() {
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
        if !map_up && let Some((map, corners)) = session.light_map() {
            let corners = corners.map(|(x, y)| {
                let at = view.to_canvas(Vec2::new(x, y)) + canvas.min;
                egui::pos2(at.x, at.y)
            });
            screen.fog.paint(&ctx, &painter, map, corners);
        }
    }

    // The pick in the pointer's hand, where the system's cursor was.
    if marking && let Some(p) = on_canvas {
        pick_cursor(&painter, egui::pos2(p.x + canvas.min.x, p.y + canvas.min.y));
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
                theme::name_over(&painter, at, &resident_name(station, who), theme::THEIRS);
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
    let _ = now;
    Ok(())
}

/// A pick, drawn at the pointer: the handle running down and to the left
/// of the point, the head across its top, each stroke over a dark one so
/// it reads on the deck and on the void alike.
fn pick_cursor(painter: &egui::Painter, at: egui::Pos2) {
    let handle = [at + egui::vec2(-11.0, 11.0), at + egui::vec2(2.0, -2.0)];
    let head = [at + egui::vec2(-5.0, -7.0), at + egui::vec2(9.0, 5.0)];
    for (width, color) in [
        (5.0, egui::Color32::from_black_alpha(200)),
        (2.5, theme::INK),
    ] {
        painter.line_segment(handle, egui::Stroke::new(width, color));
        painter.line_segment(head, egui::Stroke::new(width + 1.0, color));
    }
    painter.circle_filled(at, 1.5, theme::ACCENT);
}

/// Where the ship is, in words: the berth it is tied up at, the place it is
/// alongside, or open space. The trip strip's first words, and what the
/// map writes over the ship.
fn whereabouts(session: &Session) -> String {
    match session.docked_at() {
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
                    STATE_NAMES
                        .get(state as usize)
                        .copied()
                        .unwrap_or("Under way")
                        .to_string()
                } else {
                    whereabouts.clone()
                };
                ui.label(egui::RichText::new(phase).strong());
            }
        }
        if state >= 2 {
            ui.horizontal(|ui| {
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
    // member to reach the seat; a walk that cannot start — no helm, or no
    // way to it — says so and orders nothing.
    let game = session.game.as_mut().unwrap();
    if let Some(order) = press {
        if game.world.order_to_helm(local) {
            *pending = Some(order);
            *relieve = false;
        } else {
            log.push(format!("{} cannot reach the helm.", crew_name(local)));
        }
    }
    if cancel {
        *pending = None;
        game.world.stand_down(local);
    }
}

/// The galaxy chart's state, as the strip sees it: whether it is up, the
/// chart itself, and the star picked on it.
struct Chart<'a> {
    up: &'a mut bool,
    lobby: &'a mut Option<lobby::Lobby>,
    picked: &'a mut Option<u32>,
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
            ui.label(
                egui::RichText::new(format!(
                    "{}{}",
                    STATION_KIND_NAMES
                        .get(station.kind as usize)
                        .copied()
                        .unwrap_or("Station"),
                    if station.hostile { " · hostile" } else { "" }
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
    system_contents(ui, &base_of(here), &game.world.system);

    ui.add_space(4.0);
    let mut press = None;
    match *chart.picked {
        Some(star) if star != here => {
            ui.label(egui::RichText::new("Picked").small().color(theme::MUTED));
            ui.label(egui::RichText::new(name_of(star)).strong());
            match lobby.inspected.as_ref().filter(|(id, _)| *id == star) {
                Some((_, system)) => system_contents(ui, &base_of(star), system),
                None => {
                    ui.label(egui::RichText::new("Looking…").color(theme::MUTED));
                }
            }
            let state = game.world.ship.state.code();
            let ready = game.world.hyperdrive_ready();
            let why = if !ready {
                Some("No working hyperdrive: one bolted to an engine, on a live cable.")
            } else if state != 1 {
                Some("A jump wants the ship holding on its own, away from any berth.")
            } else {
                None
            };
            if let Some(why) = why {
                ui.label(egui::RichText::new(why).small().color(theme::WARN));
            }
            ui.horizontal(|ui| {
                if ui
                    .add_enabled(why.is_none() && !walking, egui::Button::new("Jump"))
                    .clicked()
                {
                    press = Some(HelmOrder::Jump(star));
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
            ui.add_space(4.0);
            // What is aboard to sell is what no construction site has
            // claimed, which is the rule `Sell` is judged by.
            let free =
                |s: &Session, id: ResourceId| s.game.as_ref().map_or(0, |g| g.world.free(id));
            for (resource, units, buying) in trade_rows(ui, session, cart, true, at_desk, free) {
                orders.push(Order::Deal {
                    resource: ResourceId::ALL[resource as usize],
                    units,
                    buying,
                });
            }
        });
    if walk {
        session.walk_to_desk(local);
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
        hold.counts[id as usize] = world.free(id);
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
    hold
}

/// What is on the workbench being upgraded, for the Management tab's line
/// under its tick box, off `World::upgrade`.
fn upgrade_view(session: &Session) -> Option<UpgradeView> {
    let upgrade = session.game.as_ref()?.world.upgrade?;
    Some(UpgradeView {
        resource: upgrade.resource,
        tier: upgrade.to.code(),
        done: upgrade.done.min(world::data::UPGRADE_SESSIONS),
        of: world::data::UPGRADE_SESSIONS,
        waiting: upgrade.complete(),
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
        desk: world.research_desk_aboard(),
        powered: world.research_desk_powered(),
        keys: world.keys_in_desk(),
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
    for container in world.aboard.containers() {
        if crate::crew::container_class(room, container).is_none()
            || !room.within_reach(who, container, reach)
        {
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
            stocked: site.stocked(design),
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
