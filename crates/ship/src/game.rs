//! The game, as the browser sees it: a world, two cameras, and the pointer.
//!
//! Every rule is next door in `crates/world`, which renders nothing and
//! knows nothing about a canvas. Nothing in here decides what a step does;
//! it asks, it draws the answer, and it passes the player's orders along.
//!
//! # The page drives the clock, and that is deliberate
//!
//! [`Game::step`] advances the world by exactly one step and nothing else
//! decides how many of those happen. `crates/app/src/screens/game.rs` keeps an accumulator, works
//! out how many steps a frame is worth at the effective speed, and calls this
//! that many times. Putting the accumulator in here would mean the wasm had an
//! opinion about real time, which is the one thing it has no way to measure.
//!
//! # Commands are queued, not applied
//!
//! A command from the page is put on a list and handed to the **next** step.
//! That is what makes the stamp `crates/app/src/screens/game.rs` puts on it mean something: a
//! command applies at a step, the same step for everybody, and a transport
//! that arrives late has something to compare against. Applying one the
//! instant a button is pressed would work perfectly and would be impossible to
//! wire a network into later.

use flight::{Target, angle};
use shipdesign::parts::{PartKind, Rotation, TILE};
use shipdesign::{Money, ShipDesign};
use world::world::Command;
use world::{SiteRefusal, Speed, World, WorldEvent};
use worldgen::GalaxyType;
use worldgen::math::{DVec2, dvec2};

use crate::camera::Camera;

#[path = "map_layout.rs"]
mod map_layout;
pub use map_layout::{MAP_SEPARATION, MapSite};

/// Which picture is being drawn.
///
/// The discriminants cross the wasm boundary. Two of them, and there is no
/// third: a "ship view that is also a map" is how you end up with a map you
/// cannot read and a ship you cannot see.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u32)]
pub enum ViewMode {
    /// The ship itself, at tile scale, turned to its heading.
    Ship = 0,
    /// The system: what the crew have found, and where the ship is in it.
    Map = 1,
}

/// The picture behind a station's deck ([`Game::backdrop`]).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Backdrop {
    /// One of the plain pictures: a number the app takes modulo them
    /// (`backdrop::STATION`).
    Station(u64),
    /// The machine-infested system's picture: the machines have the
    /// system and it is not liberated yet.
    Infested,
}

/// A world and the orders queued for its next step, without the cameras
/// and the tools round it: the host's timeline as a guest keeps it beside
/// the one it shows, which runs ahead on its own orders (task 156,
/// `crates/app/src/rollback.rs`). Traded into a [`Game`] with
/// [`Game::swap`] to be stepped and told things the way the game is.
#[derive(Clone)]
pub struct Timeline {
    pub world: World,
    queued: Vec<Command>,
}

/// Where what a [`Game`] puts out stood: its events, the crew's room's
/// cues and particles, and the station's room's with the station it is.
#[derive(Clone, Copy, Debug)]
pub struct OutMark {
    events: usize,
    aboard: (usize, usize, usize),
    residents: Option<(u32, (usize, usize, usize))>,
}

/// The residents' room's place in a mark: its own while it is the room
/// the mark was taken in, the start of everything for one opened since.
fn residents_mark(mark: &OutMark, station: u32) -> (usize, usize, usize) {
    match mark.residents {
        Some((at, m)) if at == station => m,
        _ => (0, 0, 0),
    }
}

/// What a [`Game`] put out for the picture and the speakers over a step,
/// or an order: its events, and each room's — the crew's, the station's —
/// cues and particles.
#[derive(Clone, Debug, Default)]
pub struct Output {
    pub events: Vec<WorldEvent>,
    pub rooms: [(Vec<bims::cue::Cued>, Vec<bims::fx::Spray>); 2],
}

impl Output {
    /// Everything in `more` this has not got, added, one for one: what was
    /// heard at a step, the first time and since.
    pub fn merge(&mut self, more: Output) {
        fn add<T>(have: &mut Vec<T>, more: Vec<T>, same: impl Fn(&T, &T) -> bool) {
            // Matched against what was there before: one pushed here is
            // `more`'s own, and `unmatched` is only as long as `have` was.
            let had = have.len();
            let mut unmatched: Vec<bool> = vec![true; had];
            for item in more {
                match (0..had).find(|&i| unmatched[i] && same(&have[i], &item)) {
                    Some(i) => unmatched[i] = false,
                    None => have.push(item),
                }
            }
        }
        add(&mut self.events, more.events, |a, b| a == b);
        for (mine, theirs) in self.rooms.iter_mut().zip(more.rooms) {
            add(&mut mine.0, theirs.0, |a, b| a.alike(b));
            add(&mut mine.1, theirs.1, |a, b| a.same(b));
        }
    }

    /// What of this `heard` has not got, one for one: the news in it.
    pub fn unheard(self, heard: &Output) -> Output {
        fn sift<T: Clone>(mine: Vec<T>, heard: &[T], same: impl Fn(&T, &T) -> bool) -> Vec<T> {
            let mut unmatched: Vec<bool> = vec![true; heard.len()];
            mine.into_iter()
                .filter(|item| {
                    match (0..heard.len()).find(|&i| unmatched[i] && same(&heard[i], item)) {
                        Some(i) => {
                            unmatched[i] = false;
                            false
                        }
                        None => true,
                    }
                })
                .collect()
        }
        let [aboard, residents] = self.rooms;
        let [heard_aboard, heard_residents] = &heard.rooms;
        let room = |(cues, sprays): (Vec<_>, Vec<_>), heard: &(Vec<_>, Vec<bims::fx::Spray>)| {
            (
                sift(cues, &heard.0, |a: &bims::cue::Cued, b| a.alike(b)),
                sift(sprays, &heard.1, |a: &bims::fx::Spray, b| a.same(b)),
            )
        };
        Output {
            events: sift(self.events, &heard.events, |a, b| a == b),
            rooms: [room(aboard, heard_aboard), room(residents, heard_residents)],
        }
    }

    /// `more` after this, all of it.
    pub fn extend(&mut self, more: Output) {
        self.events.extend(more.events);
        for (mine, theirs) in self.rooms.iter_mut().zip(more.rooms) {
            mine.0.extend(theirs.0);
            mine.1.extend(theirs.1);
        }
    }
}

/// Furthest in and out the ship view will go. The same three-times-life-size
/// ceiling the design phase has, so the two look like one game.
const SHIP_MIN_SCALE: f32 = 0.02;
const SHIP_MAX_SCALE: f32 = 3.0;

/// And for the map, where the units are whole systems. A hundred million
/// units across a canvas is about `1e-5`, so this brackets it either side by a
/// couple of orders of magnitude.
const MAP_MIN_SCALE: f32 = 1e-8;
const MAP_MAX_SCALE: f32 = 1e-2;

/// How far the ship view reaches from its middle to the canvas's nearer
/// edge, in tiles, at its widest and at its start: the reach of the
/// furthest-shooting weapon (`bims::balance::MAX_RANGE`), so nothing
/// fires from off the screen. The player counted it on a 2560×1440
/// screen: 14 tiles up and down, 25 to either side; then zoomed in a
/// wheel notch and asked for that as the view: 12.5 up and down, 22 to
/// either side, a tile 58 pixels where it had been 51.
pub const VIEW_REACH: f32 = bims::balance::MAX_RANGE;

/// How near a body has to be to the passage for the airlock doors to open,
/// in design units: a tile and a half, which is about a body's walk before
/// it reaches the door.
const AIRLOCK_HAIL: f64 = 1.5 * TILE as f64;

/// How much of the way to open or shut the door moves each frame.
const AIRLOCK_EASE: f32 = 0.18;

/// How much of the canvas the whole of the discovered system should fill when
/// the map is first opened.
const MAP_FIT: f32 = 0.8;

pub struct Game {
    pub world: World,
    pub mode: ViewMode,
    pub ship_view: Camera,
    pub map_view: Camera,
    /// Which player this browser is.
    pub local: u32,
    /// What the last batch of steps threw up, waiting for the page to read it.
    /// Drained rather than kept: an event is a thing that happened once.
    pub events: Vec<WorldEvent>,
    /// Orders waiting for the next step — see the module note.
    pub queued: Vec<Command>,
    /// What the map rings: the site picked on the world map's list, set by
    /// the app every frame (feature 103). The local player's own, never a
    /// command, and read by nothing that decides anything.
    pub aimed: Option<Target>,
    /// The design tile under the pointer, in the ship view. Signed: a pointer
    /// off the hull is off it rather than on the nearest edge.
    pub hover: Option<(i32, i32)>,
    /// Head up rather than north up: the ship is drawn the way it was laid
    /// out and the sky, the station alongside and the map turn round it
    /// instead. A view setting and nothing else — the heading is what it is,
    /// and nothing that decides anything reads this. On by default (feature
    /// 65): the ship square to the window is the view the game opens on,
    /// and North up, `N`, is the fixed sky that makes a
    /// flip legible; see `camera.rs`.
    pub head_up: bool,
    /// Whether the player is marking rocks: the Actions tab's Mine tool is
    /// on, so a rock under the pointer is rung. A tool setting, this
    /// window's own; a mark itself is a command and crosses the seam.
    pub marking: bool,
    /// The part the player is about to lay out, and which way round: the
    /// Build tab's tool, drawn as a blueprint under the pointer. A tool
    /// setting, this window's own, like `marking`; the site itself is a
    /// command and crosses the seam.
    pub placing: Option<(PartKind, Rotation)>,
    /// Whether the blueprint under the pointer would go, worked out once
    /// per tile it is over rather than every frame — `validate` walks the
    /// whole ship. `(kind, tile, rotation)` it was asked for, and the
    /// answer.
    ghost_check: Option<((PartKind, (i32, i32), Rotation), Result<(), SiteRefusal>)>,
    /// Whether the cameras follow their subject — the ship view the crew
    /// member the player steers, the map the ship — or have been let go to
    /// be dragged anywhere (`Camera::set_loose`). Off by default — the game
    /// opens on the whole ship, free to be dragged, and `F` or the View
    /// panel tethers both; a view setting like `head_up`, this window's
    /// own, and nothing that decides anything reads it.
    pub follow: bool,
    /// Whom the cameras follow in place of the crew member the player
    /// steers: a crewmate, while the player's own Bim is out and waiting
    /// to be bought back (feature 107). A view setting like `follow`,
    /// this window's own, and nothing that decides anything reads it.
    pub spectate: Option<u32>,
    /// Whether the ship view may zoom out past [`VIEW_REACH`]: off in a
    /// game, where the view the fight is balanced around is the widest
    /// there is; on for a terminal looking at a whole town (`BIMS_ZOOM`
    /// under one). A view setting, this window's own.
    pub wide: bool,
    /// Where in the system the middle of a map let go is held: the map's
    /// camera is measured from the ship, so a focus left alone would fly
    /// with it, and a planet zoomed in on would slide off as the ship set
    /// out. Set from the camera after every pan and zoom, and put back into
    /// it every frame (`follow_player`). Meaningless while `follow` is on.
    map_anchor: DVec2,
    /// Frames drawn. What the exhaust flickers and the running lights blink
    /// off — a picture clock, counted by the render and by nothing that
    /// decides anything. It does not stop at a pause, which is right: a
    /// paused flame still burns.
    pub frame: u32,
    /// How far the mated airlocks stand open, 0 shut to 1 wide. A picture
    /// clock like `frame`: it eases towards open while somebody is at the
    /// door and shut when nobody is, and nothing that decides anything
    /// reads it — the passage is walkable whatever the door looks like.
    pub airlock_ajar: f32,
    /// How many of the running abilities' particles are owed, a beat
    /// at a time (`crate::sprays::running`): a picture clock on real
    /// seconds, never saved.
    pub(crate) spray_due: f32,
    /// The stations' own pictures as last drawn, kept for the next frame
    /// and used again while each is still the picture of its station
    /// (`world_paint::KeptStation`, task 122). The picture's and nothing
    /// else's: never saved, and a game read back starts with none.
    pub(crate) kept_stations: Vec<crate::world_paint::KeptStation>,
    /// The system the map shows when it is not the ship's own (the map rework,
    /// `map_layout`): a star picked on the galaxy chart or off the list.
    /// A view setting, this window's own.
    shown: Option<map_layout::Shown>,
    /// The star the ship was in when the map was last fitted, so a jump
    /// fits it again.
    shown_own: Option<u32>,
}

impl Game {
    /// Open the game with the accepted design.
    ///
    /// Returns `None` when the world will not start, which in practice means
    /// a galaxy with nowhere to spawn or a design that does not weigh enough
    /// to be a ship. A cdylib that panicked here would abort, and an abort
    /// tells the player nothing at all.
    pub fn start(
        design: ShipDesign,
        money: Money,
        players: u32,
        local: u32,
        seed: u64,
        galaxy_type: GalaxyType,
        star: u32,
        station: u32,
        width: f32,
        height: f32,
    ) -> Option<Game> {
        Game::start_with_crew(
            design,
            money,
            players,
            players,
            local,
            seed,
            galaxy_type,
            star,
            station,
            width,
            height,
        )
    }

    /// [`Game::start`] with `crew` aboard, of whom `players` are players —
    /// see `World::start_with_crew`. The `combat` command's sixteen.
    #[allow(clippy::too_many_arguments)]
    pub fn start_with_crew(
        design: ShipDesign,
        money: Money,
        players: u32,
        crew: u32,
        local: u32,
        seed: u64,
        galaxy_type: GalaxyType,
        star: u32,
        station: u32,
        width: f32,
        height: f32,
    ) -> Option<Game> {
        let world = World::start_with_crew(
            design,
            money,
            players,
            crew,
            seed,
            galaxy_type,
            star,
            station,
        )
        .ok()?;
        let mut game = Game {
            world,
            mode: ViewMode::Ship,
            ship_view: Camera::new(width, height, 1.0, SHIP_MIN_SCALE, SHIP_MAX_SCALE),
            map_view: Camera::new(width, height, 1e-5, MAP_MIN_SCALE, MAP_MAX_SCALE),
            local: local.min(players.saturating_sub(1)),
            events: Vec::new(),
            queued: Vec::new(),
            aimed: None,
            hover: None,
            head_up: true,
            follow: false,
            spectate: None,
            wide: false,
            map_anchor: DVec2::ZERO,
            marking: false,
            placing: None,
            ghost_check: None,
            frame: 0,
            airlock_ajar: 0.0,
            spray_due: 0.0,
            kept_stations: Vec::new(),
            shown: None,
            shown_own: None,
        };
        game.fit_ship();
        game.fit_map();
        game.ship_view.set_loose(true);
        game.map_view.set_loose(true);
        game.map_anchor = game.world.ship.position();
        Some(game)
    }

    /// [`Game::start_with_crew`] on a world already under way — one read
    /// back from a save (`crate::save`). Everything that is not the world
    /// starts as it does at an open: the cameras fitted, nothing aimed,
    /// no tool in hand.
    pub fn resume(world: World, local: u32, width: f32, height: f32) -> Game {
        let players = world.players();
        let mut world = world;
        // The crisis's hop table is derived from the galaxy and the
        // origin, and a save carries only the origin (feature 92): every
        // world read back — a load, a restart, a guest handed the host's
        // — wants it worked out again before anything asks which stars
        // the machines have.
        world.settle_crisis();
        // And what everybody wears, which a save leaves out: the first
        // frame is drawn before the first step hands it over.
        world.dress_the_rooms();
        let mut game = Game {
            world,
            mode: ViewMode::Ship,
            ship_view: Camera::new(width, height, 1.0, SHIP_MIN_SCALE, SHIP_MAX_SCALE),
            map_view: Camera::new(width, height, 1e-5, MAP_MIN_SCALE, MAP_MAX_SCALE),
            local: local.min(players.saturating_sub(1)),
            events: Vec::new(),
            queued: Vec::new(),
            aimed: None,
            hover: None,
            head_up: true,
            follow: false,
            spectate: None,
            wide: false,
            map_anchor: DVec2::ZERO,
            marking: false,
            placing: None,
            ghost_check: None,
            frame: 0,
            airlock_ajar: 0.0,
            spray_due: 0.0,
            kept_stations: Vec::new(),
            shown: None,
            shown_own: None,
        };
        game.fit_ship();
        game.fit_map();
        game.ship_view.set_loose(true);
        game.map_view.set_loose(true);
        game.map_anchor = game.world.ship.position();
        game
    }

    pub fn resize(&mut self, width: f32, height: f32) {
        self.ship_view.resize(width, height);
        self.map_view.resize(width, height);
    }

    /// The map's camera alone (task 135, `Session::resize_map`), fitted
    /// again to everything found: the part of the canvas it is given is
    /// not the one it was first fitted to.
    pub fn resize_map(&mut self, width: f32, height: f32) {
        self.map_view.resize(width, height);
        self.fit_map();
    }

    /// The canvas is this big, and the views are to start again from it:
    /// the whole hull in the ship view, everything found in the map. For a
    /// host that only learns the canvas's size after the game has opened —
    /// the desktop lays its panels out around the window it was given —
    /// and wants the opening view the game would have had at that size.
    pub fn fit(&mut self, width: f32, height: f32) {
        self.resize(width, height);
        self.fit_ship();
        self.fit_map();
    }

    /// The camera the page should be painting through.
    pub fn camera(&self) -> &Camera {
        match self.mode {
            ViewMode::Ship => &self.ship_view,
            ViewMode::Map => &self.map_view,
        }
    }

    pub fn camera_mut(&mut self) -> &mut Camera {
        match self.mode {
            ViewMode::Ship => &mut self.ship_view,
            ViewMode::Map => &mut self.map_view,
        }
    }

    /// How far the camera is turned, in the screen's sense — the angle
    /// everything that is *out there* is drawn through, and the one the ship's
    /// own heading is drawn on top of.
    ///
    /// Nothing, north up: the camera never turns and the ship does. Head up,
    /// it is the heading undone, so the ship comes out at `heading +
    /// camera_turn() == 0` — square to the window — and the world turns the
    /// other way by exactly as much. Everything that draws or reads back the
    /// ship goes through [`Game::ship_turn`], everything that draws or reads
    /// back the world goes through this alone, and there is no third thing.
    pub fn camera_turn(&self) -> f64 {
        if self.head_up {
            -self.world.ship.heading
        } else {
            0.0
        }
    }

    /// Which picture is behind the deck: `None` on a planet, where the
    /// ground is the surroundings; at a station in a system the machines
    /// have (and the crew have not liberated) the infested one; at any
    /// other a number the app takes modulo its pictures
    /// (`backdrop::STATION`). **Cosmetic**: off the galaxy's seed, the
    /// system and the station, so one station has one picture on every
    /// end and every visit, and nothing reads it but the painter.
    pub fn backdrop(&self) -> Option<Backdrop> {
        let world = &self.world;
        if world.landed().and_then(|b| world.surface(b)).is_some() {
            return None;
        }
        if world.infested(world.star_id) && !world.liberated(world.star_id) {
            return Some(Backdrop::Infested);
        }
        let station = world.ship.state.alongside().map_or(u64::MAX, u64::from);
        // splitmix64's finish over the three, so neighbouring stations
        // do not all fall on one picture.
        let mut z = world.galaxy_seed
            ^ (u64::from(world.system.star_id) << 32)
            ^ station.wrapping_mul(0x9e37_79b9_7f4a_7c15);
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        Some(Backdrop::Station(z ^ (z >> 31)))
    }

    /// The angle the ship is drawn through: its heading, less however far the
    /// camera has turned to keep it upright.
    pub fn ship_turn(&self) -> f64 {
        self.world.ship.heading + self.camera_turn()
    }

    /// The ship's airlock while it is mated to a station's: docked, and the
    /// ship has a port. What the painter draws open. `None` otherwise.
    pub fn mated_airlock(&self) -> Option<u32> {
        let station = self.world.ship.state.alongside()?;
        self.world.station(station)?.port()?;
        shipdesign::port(&self.world.ship.design).map(|p| p.part_id)
    }

    /// Put the crew member the player steers in the middle of the ship view:
    /// the camera's focus is where they stand, in the camera's units about
    /// the ship, so the view follows them off the ship and into a station
    /// and can never be panned until they are off the edge. Once a frame,
    /// from `ship_render`. Let go (`follow` off), the ship view is left
    /// wherever it was dragged. The map is held on `map_anchor`, a place in
    /// the system it shows, either way.
    pub fn follow_player(&mut self) {
        // The map is held where it was fitted or dragged to, following or
        // not (the map rework): it shows a system's sites, not the ship.
        let (x, y) = self.map_focus_of(self.map_anchor);
        self.map_view.set_focus(x, y);
        if !self.follow {
            return;
        }
        let who = self.spectate.unwrap_or(self.local);
        if who >= self.world.aboard.count() {
            return;
        }
        let (x, y) = crate::world_paint::crew_on_screen(self, who);
        self.ship_view.set_focus(x, y);
    }

    /// The ship view reaches no further than [`VIEW_REACH`] (October 2026,
    /// the player's word): its scale is held so the canvas's nearer edge is
    /// that far from its middle, whatever the window's size — the widest
    /// zoom, and the one it opens at. [`Game::wide`], it is held only on a
    /// planet, where its far corner stays within `bims::terrain::VIEW`
    /// tiles of its middle, the ground that is loaded. Once a frame,
    /// before the picture.
    pub fn hold_view_to_the_ground(&mut self) {
        let floor = if !self.wide {
            self.ship_view.scale_for_reach(VIEW_REACH * TILE as f32)
        } else if self.world.aboard.room.plane().is_some() {
            let reach = bims::terrain::VIEW as f32 * TILE as f32;
            self.ship_view.scale_for_reach(reach)
        } else {
            0.0
        };
        self.ship_view.set_floor(floor);
    }

    /// On a planet, the plain's picture for this frame — the fog beyond
    /// the box, marched like the room's light map, a chunk a texture —
    /// asked of the room over the window the plain is about to be drawn
    /// on (`world_paint::plain_window`), which is why it is asked here
    /// and not in the room's own `render`: the room does not know the
    /// camera. Once a frame, after the view is held to the ground and
    /// before the picture.
    pub fn picture_the_plain(&mut self) {
        if let Some(window) = crate::world_paint::plain_window(self) {
            self.world.aboard.room.picture_plain(window);
        }
    }

    /// A place in the system as the map camera's focus: from the ship, y
    /// flipped, turned with the camera — the way `pick` lays the nodes out.
    fn map_focus_of(&self, at: DVec2) -> (f32, f32) {
        let offset = at.sub(self.map_origin());
        turned(offset.x as f32, -offset.y as f32, self.camera_turn() as f32)
    }

    /// Note where a map let go is now looking, after a pan or a zoom has
    /// moved it: the middle of the canvas, as a place in the system. A
    /// loose camera keeps no shove, so the middle is the focus.
    fn hold_map(&mut self) {
        if self.mode == ViewMode::Map && self.map_view.is_loose() {
            self.map_anchor = self.point_at(self.map_view.width / 2.0, self.map_view.height / 2.0);
        }
    }

    /// Shove the view that is up by a screen-pixel delta.
    pub fn pan(&mut self, dx: f32, dy: f32) {
        self.camera_mut().pan(dx, dy);
        self.hold_map();
    }

    /// Zoom the view that is up about a point on the canvas.
    pub fn zoom(&mut self, at_x: f32, at_y: f32, factor: f32) {
        self.camera_mut().zoom(at_x, at_y, factor);
        self.hold_map();
    }

    /// Put the crew member the player steers in the middle of the ship view
    /// now, once, whatever the camera is doing: a tethered view has its
    /// shove undone, a loose one is brought back to them and left loose.
    /// The map is brought back to the ship the same way.
    pub fn centre_on_player(&mut self) {
        self.map_view.recentre(0.0, 0.0);
        self.map_anchor = self.world.ship.position();
        let who = self.spectate.unwrap_or(self.local);
        if who >= self.world.aboard.count() {
            return;
        }
        let (x, y) = crate::world_paint::crew_on_screen(self, who);
        self.ship_view.recentre(x, y);
    }

    /// Follow the crew member the player steers, or stop: the ship view is
    /// let loose so a drag takes it anywhere, and tethered again it snaps
    /// back to them on the next frame. The map is not the crew's and stays
    /// where it is.
    pub fn set_follow(&mut self, on: bool) {
        self.follow = on;
        self.ship_view.set_loose(!on);
    }

    /// Move the airlock door on one frame: open while anybody in the room
    /// is within [`AIRLOCK_HAIL`] of the passage, shut otherwise, easing
    /// either way so it reads as a door and not a switch. Docked with the
    /// rooms joined, or it stays shut.
    pub fn tick_airlock(&mut self) {
        let want = self.someone_at_the_door();
        let target = if want { 1.0 } else { 0.0 };
        self.airlock_ajar += (target - self.airlock_ajar) * AIRLOCK_EASE;
        if (self.airlock_ajar - target).abs() < 0.005 {
            self.airlock_ajar = target;
        }
    }

    fn someone_at_the_door(&self) -> bool {
        if self.mated_airlock().is_none() || !self.world.aboard.is_joined() {
            return false;
        }
        let Some(port) = shipdesign::port(&self.world.ship.design) else {
            return false;
        };
        // The passage, in the room's units: the ship's door face, plus the
        // ship's offset into the joined room.
        let (fx, fy) = port.face();
        let door = dvec2(fx, fy).add(self.world.aboard.offset);
        (0..self.world.aboard.count()).any(|who| {
            let at = self
                .world
                .aboard
                .position(who)
                .add(self.world.aboard.offset);
            at.distance(door) <= AIRLOCK_HAIL
        })
    }

    /// Switch views. The ship view keeps its scale and its place between
    /// visits; the map opens fitted to every site of the system it shows
    /// (the map rework), and keeps a pan or a zoom only while it is up.
    pub fn set_mode(&mut self, mode: ViewMode) {
        if mode == ViewMode::Map && self.mode != ViewMode::Map {
            self.fit_map();
        }
        self.mode = mode;
    }

    /// Start the ship view at [`VIEW_REACH`] — or, [`Game::wide`], showing
    /// the whole hull.
    fn fit_ship(&mut self) {
        if !self.wide {
            let scale = self.ship_view.scale_for_reach(VIEW_REACH * TILE as f32);
            self.ship_view.set_floor(scale);
            self.ship_view.set_scale(scale);
            return;
        }
        // The cap a narrow view was opened with lifted; a planet's is
        // set again at the next frame.
        self.ship_view.set_floor(0.0);
        let span = self.world.ship.design.build_area as f32 * TILE as f32;
        let fit = (self.ship_view.width / span).min(self.ship_view.height / span);
        self.ship_view.set_scale(fit * 0.9);
    }

    // --- the clock ----------------------------------------------------------

    /// One step of the world, with whatever the page has queued.
    ///
    /// Events pile up across a frame's worth of steps and the page drains them
    /// once; a step that produced nothing adds nothing.
    pub fn step(&mut self) {
        let commands = std::mem::take(&mut self.queued);
        let (hash, sites, state) = (
            self.world.design_hash(),
            self.world.builds.len(),
            self.world.ship.state.code(),
        );
        let events = self.world.step(&commands);
        crate::sprays::abilities(self, &events);
        self.events.extend(events);
        // The ship may have changed under a still pointer — a part built, a
        // site laid out, the ship set off — and then the blueprint's answer
        // is asked again. Only then: the answer is a validation of the
        // whole ship, and at 24x this runs many times a frame.
        if hash != self.world.design_hash()
            || sites != self.world.builds.len()
            || state != self.world.ship.state.code()
        {
            self.ghost_check = None;
        }
    }

    // --- a second timeline (task 156) ---------------------------------------

    /// This game's world and its queue, copied: what a guest keeps of the
    /// host's timeline beside the one it shows (`crates/app/src/rollback.rs`).
    pub fn fork(&self) -> Timeline {
        Timeline {
            world: self.world.clone(),
            queued: self.queued.clone(),
        }
    }

    /// Trade this game's world and queue for `other`'s: the cameras, the
    /// tools and the events already thrown up stay this game's. The
    /// blueprint's answer is asked again, the ship under it being another.
    pub fn swap(&mut self, other: &mut Timeline) {
        std::mem::swap(&mut self.world, &mut other.world);
        std::mem::swap(&mut self.queued, &mut other.queued);
        self.ghost_check = None;
    }

    /// Put `other`'s world and queue in this game's place, copied, and
    /// drop what this world was: the guest's rollback. The pictures carry
    /// on from the world they replace (`World::adopt_picture`), so the
    /// host's light maps take the new one for a change of the old.
    pub fn restore(&mut self, other: &Timeline) {
        let mut world = other.world.clone();
        world.adopt_picture(&mut self.world);
        self.world = world;
        self.queued.clone_from(&other.queued);
        self.ghost_check = None;
    }

    /// Where what the game puts out for the picture and the speakers
    /// stands now — its events, both rooms' cues and particles — to read
    /// what a step or an order adds ([`Game::out_since`]).
    pub fn out_mark(&mut self) -> OutMark {
        OutMark {
            events: self.events.len(),
            aboard: self.world.aboard.room.out_mark(),
            residents: self
                .world
                .residents
                .as_mut()
                .map(|r| (r.station, r.aboard.room.out_mark())),
        }
    }

    /// What the game put out since `mark`, copied. A station's room
    /// opened since is read whole; one closed since put out nothing here.
    pub fn out_since(&mut self, mark: OutMark) -> Output {
        let events = self.events.get(mark.events..).unwrap_or_default().to_vec();
        let aboard = self.world.aboard.room.out_since(mark.aboard);
        let residents = match self.world.residents.as_mut() {
            Some(r) => r.aboard.room.out_since(residents_mark(&mark, r.station)),
            None => Default::default(),
        };
        Output {
            events,
            rooms: [aboard, residents],
        }
    }

    /// Of what the game put out since `mark`, each thing `heard` holds
    /// taken back out, one for one: a guest's rollback playing a step
    /// again shows and sounds only what the first time did not.
    pub fn drop_heard(&mut self, mark: OutMark, heard: &Output) {
        let mut left = heard.events.clone();
        let mut i = mark.events.min(self.events.len());
        while i < self.events.len() {
            if let Some(j) = left.iter().position(|e| *e == self.events[i]) {
                left.swap_remove(j);
                self.events.remove(i);
            } else {
                i += 1;
            }
        }
        let [aboard, residents] = &heard.rooms;
        self.world
            .aboard
            .room
            .drop_heard(mark.aboard, &aboard.0, &aboard.1);
        if let Some(r) = self.world.residents.as_mut() {
            let from = residents_mark(&mark, r.station);
            r.aboard.room.drop_heard(from, &residents.0, &residents.1);
        }
    }

    /// Both rooms' cues and particles not yet taken, taken: what a world
    /// about to be thrown away had put out, for [`Game::put_back`].
    pub fn take_unread(&mut self) -> Output {
        let take = |room: &mut bims::game::Game| (room.take_cues(), room.take_sprays());
        let aboard = take(&mut self.world.aboard.room);
        let residents = self
            .world
            .residents
            .as_mut()
            .map(|r| take(&mut r.aboard.room))
            .unwrap_or_default();
        Output {
            events: Vec::new(),
            rooms: [aboard, residents],
        }
    }

    /// What [`Game::take_unread`] took, back in front of whatever the
    /// world in its place has put out since. The events are the caller's.
    pub fn put_back(&mut self, unread: Output) {
        let [aboard, residents] = unread.rooms;
        self.world.aboard.room.put_back_unread(aboard.0, aboard.1);
        if let Some(r) = self.world.residents.as_mut() {
            r.aboard.room.put_back_unread(residents.0, residents.1);
        }
    }

    /// Everything the world threw up for the picture and the speakers
    /// since it was last read — its events, both rooms' cues and their
    /// particles — dropped unread: steps nobody is to hear again.
    pub fn silence(&mut self) {
        self.events.clear();
        drop(self.world.aboard.room.take_cues());
        if let Some(residents) = self.world.residents.as_mut() {
            drop(residents.aboard.room.take_cues());
        }
        drop(crate::sprays::take(self));
    }

    /// Queue an order for the next step.
    ///
    /// With exceptions, and they are `world`'s rather than this module's:
    /// a speed request is applied straight away, because at a pause there
    /// are no steps for a queued one to land on and the pause could never
    /// be lifted; and so is an order to the crew's room, so a click on the
    /// deck at a pause is a click that shows. See
    /// [`World::applies_at_once`].
    pub fn send(&mut self, command: Command) {
        if World::applies_at_once(&command) {
            let events = self.world.apply_now(command);
            crate::sprays::abilities(self, &events);
            self.events.extend(events);
            return;
        }
        self.queued.push(command);
    }

    /// What step a command queued now will apply at. The stamp `crates/app/src/screens/game.rs`
    /// puts on every message, and the thing a transport would compare.
    pub fn next_step(&self) -> u64 {
        self.world.steps + 1
    }

    // --- the pointer ---------------------------------------------------------

    /// A point on the canvas, as a design tile.
    ///
    /// The ship is drawn turned, so this is the turn read backwards: screen
    /// offset from the middle, back through the heading, back into the grid.
    /// It is the same arithmetic `world_paint` uses forwards, which is what
    /// makes a click land on the tile it looks like it landed on at every
    /// heading rather than only at zero.
    pub fn tile_at(&self, x: f32, y: f32) -> (i32, i32) {
        let design = self.design_point_at(x, y);
        (
            (design.x / TILE as f64).floor() as i32,
            (design.y / TILE as f64).floor() as i32,
        )
    }

    /// A point on the canvas, in design world units — the tile arithmetic
    /// above before the floor, and the room's own coordinates aboard, since a
    /// design unit is a room unit (`bims::aboard`). What a click on the deck
    /// is handed to the room as: a marquee corner, an order, a fixture.
    pub fn design_point_at(&self, x: f32, y: f32) -> DVec2 {
        let (vx, vy) = self.ship_view.to_view(x, y);
        // Screen is y-down and the system is y-up, so the screen offset is
        // turned back into system space before it is turned back into the
        // grid. Through the angle the ship was *drawn* at, which is the
        // heading less the camera's turn — head up, that is nothing at all.
        let system = dvec2(vx as f64, -(vy as f64));
        angle::unrotate_design(system, self.ship_turn())
            .add(self.world.ship.dynamics.centre_of_mass)
    }

    /// A point on the canvas, as a position in the system: the place the
    /// map is held on as it is panned and zoomed (`hold_map`).
    pub fn point_at(&self, x: f32, y: f32) -> DVec2 {
        let (vx, vy) = self.map_view.to_view(x, y);
        // Back through the camera's turn before the y-flip, since the turn
        // was made on screen, after it.
        self.map_point_of((vx, vy))
    }

    /// Where the map puts a node, in the camera's units about the ship:
    /// its offset from the ship with y flipped, then turned with the
    /// camera — the arithmetic `world_paint::paint_map` draws by, read the
    /// same way by `pick` and by the words the app writes over an icon.
    /// `None` for a node the system cannot place.
    pub fn map_spot(&self, node: worldgen::Node) -> Option<(f32, f32)> {
        self.map_spots()
            .into_iter()
            .find(|(n, _)| *n == node)
            .map(|(_, spot)| spot)
    }

    /// The discovered node a click on the map is near enough to count as
    /// picking, if any.
    ///
    /// Measured in **screen pixels** rather than in world units, because the
    /// thing being picked is an icon: at a map scale where a system fits on
    /// a laptop, a world-unit tolerance is either the whole screen or a
    /// thousandth of a pixel.
    pub fn pick(&self, x: f32, y: f32, slop: f32) -> Option<worldgen::Node> {
        let mut best: Option<(f32, worldgen::Node)> = None;
        for (node, (ox, oy)) in self.map_spots() {
            let sx = self.map_view.offset_x() + ox * self.map_view.scale();
            let sy = self.map_view.offset_y() + oy * self.map_view.scale();
            let away = ((sx - x).powi(2) + (sy - y).powi(2)).sqrt();
            if away <= slop && best.is_none_or(|(near, _)| away < near) {
                best = Some((away, node));
            }
        }
        best.map(|(_, node)| node)
    }

    /// Whether the blueprint under the pointer would go where it is, and
    /// why not if not — the world's `can_place_site`, asked once per tile
    /// the pointer crosses and remembered, since it validates the whole
    /// ship. `None` with no tool in hand or the pointer off the hull's
    /// grid. The answer goes stale when the ship changes under a still
    /// pointer, which `step` clears for.
    pub fn ghost_check(&mut self) -> Option<Result<(), SiteRefusal>> {
        let (kind, rotation) = self.placing?;
        let tile = self.hover?;
        let key = (kind, tile, rotation);
        if let Some((asked, answer)) = self.ghost_check
            && asked == key
        {
            return Some(answer);
        }
        let answer = if tile.0 < 0 || tile.1 < 0 {
            Err(SiteRefusal::WontFit(
                shipdesign::EditError::OutOfBounds.code(),
            ))
        } else {
            self.world
                .can_place_site(kind, (tile.0 as u32, tile.1 as u32), rotation)
        };
        self.ghost_check = Some((key, answer));
        Some(answer)
    }

    /// The last answer, if it is about the tile the pointer is on now —
    /// for a readout with no hand to ask with. A frame behind the pointer
    /// at most.
    pub fn ghost_answer(&self) -> Option<Result<(), SiteRefusal>> {
        let (kind, rotation) = self.placing?;
        let tile = self.hover?;
        self.ghost_check
            .as_ref()
            .filter(|(asked, _)| *asked == (kind, tile, rotation))
            .map(|(_, answer)| *answer)
    }

    /// Whether the last answer was that the blueprint would go — for the
    /// painter, which has no hand to ask with; `Session::render` asks
    /// first.
    pub fn ghost_ok(&self) -> bool {
        self.ghost_check
            .as_ref()
            .is_some_and(|(_, answer)| answer.is_ok())
    }

    /// Put the blueprint's tool in hand, or down, and turn it. The check
    /// is forgotten with the tool.
    pub fn set_placing(&mut self, placing: Option<(PartKind, Rotation)>) {
        self.placing = placing;
        self.ghost_check = None;
    }

    pub fn rotate_placing(&mut self) {
        if let Some((kind, rotation)) = self.placing {
            self.set_placing(Some((kind, rotation.next())));
        }
    }

    /// The construction site under `tile`, if one is laid out over it.
    pub fn site_at(&self, tile: (i32, i32)) -> Option<&world::BuildSite> {
        if tile.0 < 0 || tile.1 < 0 {
            return None;
        }
        let tile = (tile.0 as u32, tile.1 as u32);
        self.world.builds.iter().find(|s| s.tiles().contains(&tile))
    }

    /// The speed this player has asked for.
    pub fn requested(&self, slot: u32) -> Speed {
        self.world
            .speed_requests
            .get(slot as usize)
            .copied()
            .unwrap_or(Speed::Paused)
    }
}

/// A screen offset turned about the origin by `angle`, in the screen's own
/// sense — the same matrix `world_paint` turns a tile with and the canvas's
/// `rotate()` applies, so a positive angle is clockwise on a y-down screen.
pub fn turned(x: f32, y: f32, angle: f32) -> (f32, f32) {
    let (s, c) = (angle.sin(), angle.cos());
    (x * c - y * s, x * s + y * c)
}

#[cfg(test)]
mod tests {
    use super::Output;
    use world::WorldEvent;

    fn of(events: &[WorldEvent]) -> Output {
        Output {
            events: events.to_vec(),
            ..Output::default()
        }
    }

    /// Two or more new things merged into a list that had fewer: the
    /// guest's rollback hearing a step again with more in it (the panic
    /// "the len is 0 but the index is 0" joining a mission).
    #[test]
    fn a_merge_brings_several_new_things_at_once() {
        let mut have = of(&[]);
        have.merge(of(&[
            WorldEvent::CrewHit { who: 1 },
            WorldEvent::CrewHit { who: 1 },
        ]));
        assert_eq!(have.events.len(), 2, "both, one for one");

        let mut have = of(&[WorldEvent::CrewHit { who: 1 }]);
        have.merge(of(&[
            WorldEvent::CrewHit { who: 2 },
            WorldEvent::CrewHit { who: 1 },
            WorldEvent::CrewHit { who: 2 },
            WorldEvent::CrewHit { who: 1 },
        ]));
        let who: Vec<u32> = have
            .events
            .iter()
            .map(|e| match e {
                WorldEvent::CrewHit { who } => *who,
                _ => unreachable!(),
            })
            .collect();
        assert_eq!(
            who,
            [1, 2, 2, 1],
            "the one already heard kept, the rest added"
        );
    }
}
