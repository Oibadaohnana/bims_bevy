//! The crew's panels: everything on a screen that is about the Bims rather
//! than about the deck they are standing on. Shared by the room and the
//! ship, which run the same room — aboard, stepped by the world.
//!
//! One copy on purpose: the selected crew member's needs and health and
//! diary, the agendas, the tray with the timetable, the work list and the
//! management row, the fixture menus, and the tooltips everything hangs
//! off. What is *not* here is the canvas and the pointer, because those are
//! each screen's own — the room fits the deck to a window and the ship turns
//! it with the hull — and the screen hands the room its coordinates.
//!
//! # Tooltips are asked for, never stumbled into
//!
//! Every tooltip hangs off an affordance: a word underlined with
//! [`theme::asks`], or a `?` from [`theme::question_mark`]. Never a row, a
//! bar or a panel — a player crossing the needs panel on the way to the deck
//! should get nothing.
//!
//! # A highlight is not a tooltip
//!
//! Resting on a management row rings the fixture it names on the deck.
//! Nothing pops up, nothing is said, and the ring is gone the moment the
//! pointer moves — [`CrewPanels::points`] is the mechanism, and it hangs off
//! a whole row on purpose, because every row is about exactly one place.
//! The ring is worked out afresh every frame from what is hovered, so a
//! panel that folds away under the pointer cannot leave a fixture lit.
//!
//! # Gear moves by order, and the hold is handed in
//!
//! The pack, the worn slots and the container windows are grids
//! (`crate::grid`), and what a click on a cell asks for — put this on, put
//! it away, take that out — is a [`GearOrder`] left on
//! [`CrewPanels::orders`] for the screen to send: on the ship through the
//! seam as a `world::Command`, since the hold is the world's and every
//! player's ship has to agree about what is in it; in the test room, which
//! has no hold, straight to the room. The hold itself comes the other way
//! as a [`Hold`] snapshot the ship's screen sets every frame, and it is
//! `None` in the room, which is how the container windows know they have
//! nothing to show there.
//!
//! A body is handed in the same way. The Loot window is a grid over what
//! a dead or unconscious Bim has on it, and what it shows, whether the Bim
//! is still down and whether the looter is within reach come as a [`Body`]
//! snapshot the screen sets every frame *after* the fixture menu has run,
//! since the menu is what opens the window — on the ship off the world,
//! which is the only thing that can see one of a hostile station's people
//! lying in its own room; in the test room off the room itself. Taking is
//! an order like the rest, `GearOrder::Loot`, and the walk over to the
//! body is the screen's too (`walk`), for the same reason: where a resident
//! lies is the world's to say.

use bevy_egui::egui;
use bims::combat::{Item as PackItem, LOOT_CELLS, PACK_CELLS, PACK_COLS, PACK_ROWS, Piece};
use bims::game::{Container, Game};
use bims::manager::Stock;
use bims::room::*;
use bims::{bim, door, health, manager, schedule, task};
use physics::ResourceId;
use ship::game::Overlay;
use shipdesign::parts::PartKind;
use shipdesign::research::{KEY_CELLS, NODES, Node};
use shipdesign::{CARGO_SLOTS, Storage};
use world::{FetchKind, Grid, Kept, LootSource};

use crate::format::{clock_text, date_text, span_text};
use crate::grid::{self, Cell};
use crate::icons;
use crate::keys::{Action, Keys};
use crate::names::*;
use crate::theme;

/// Below this the pointer moved so little that it counts as a click, not a
/// sweep, in points.
pub const CLICK_SLOP: f32 = 4.0;

/// The side of a pack cell, in points; the container windows' cells are
/// smaller, by how many there are across — see [`container_cell`].
const PACK_CELL: f32 = 34.0;

/// The side of a container window's cells: the grids' ten across by
/// however many rows the ship has, big enough for the picture of a rifle
/// lying across seven of them and a count in a stack's corner.
fn container_cell(class: Storage) -> f32 {
    match class {
        Storage::Shelf | Storage::Locker | Storage::ColdStore => 24.0,
        // The desk's slot is the key's size: a pack cell, two down.
        Storage::Research => PACK_CELL,
    }
}

/// How many cells a container window has across and down, for a class
/// that is no grid: the research desk's one slot. The grids are the
/// ship's, `GRID_COLS` across by as many rows as the parts aboard add up
/// to, and their windows lay the hold out on them (`grid_things`).
fn container_dims(class: Storage) -> (usize, usize) {
    match class {
        Storage::Shelf | Storage::ColdStore | Storage::Locker => {
            (shipdesign::GRID_COLS as usize, 0)
        }
        Storage::Research => (KEY_CELLS.0 as usize, KEY_CELLS.1 as usize),
    }
}

/// Where a container window sits: to the right of the left-hand stack,
/// under the strip along the top. The inventory pop-up goes beside it
/// while one is open.
const CONTAINER_AT: egui::Vec2 = egui::vec2(290.0, 60.0);

/// The hold, as the panels see it: a snapshot the ship's screen hands
/// over every frame, and `None` in the test room, which has no hold.
/// Counts and reach are by `ResourceId`; the pieces are the armour in the
/// hold, one entry a piece; the class fills are by `Storage`.
#[derive(Clone, Default)]
pub struct Hold {
    /// How many of each resource are aboard and not spoken for.
    pub counts: [u32; CARGO_SLOTS],
    /// Every piece of armour in the hold, as the room would carry it.
    pub pieces: Vec<Piece>,
    /// Every weapon in the hold with its tier — `World::guns`.
    pub guns: Vec<bims::combat::Weapon>,
    /// The grids — `World::grids`, the shelves', the cold stores' and the
    /// lockers' in `World::GRID_CLASSES` order — where every stack, piece
    /// and gun lies and which way round, and each grid's size in cells;
    /// what the container windows are pictures of.
    pub grids: [Grid; 3],
    pub grid_capacity: [u32; 3],
    pub used: [u32; 4],
    pub capacity: [u32; 4],
    /// Whether the Bim whose inventory is shown stands within reach of a
    /// container that takes each resource — `World::in_reach`.
    pub reach: [bool; CARGO_SLOTS],
    /// Which research desk on the deck is the station's, while docked —
    /// `World::station_desk` — and whether its key is still on it. The
    /// desk's row reads both.
    pub station_desk: Option<usize>,
    pub station_key: bool,
}

impl Hold {
    /// A class's grid and its size in cells, if the class is one: the
    /// research desk is not.
    pub fn grid(&self, class: Storage) -> Option<(&Grid, u32)> {
        world::World::GRID_CLASSES
            .iter()
            .position(|&c| c == class)
            .map(|i| (&self.grids[i], self.grid_capacity[i]))
    }
}

/// A mercenary's terms, as the panels see them: a snapshot the screen
/// hands over every frame for the resident the Hire window is open on
/// (`World::hire_offer`), and `None` once it is not for hire — hired, or
/// the rooms parted — which shuts the window.
#[derive(Clone, Copy)]
pub struct Terms {
    pub fee: economy::Money,
    pub gear: bims::combat::Gear,
    pub in_reach: bool,
    pub affordable: bool,
    pub bunk: bool,
}

/// The body the Loot window is over, as the panels see it: a snapshot the
/// screen hands over every frame for the source that is open, and `None`
/// when there is no such Bim any more — the rooms parted — which shuts
/// the window. Cells in `bims::combat::LootCell` order: the pack's nine,
/// then the head, the body, the legs and the weapon in hand.
#[derive(Clone)]
pub struct Body {
    pub cells: [Option<PackItem>; LOOT_CELLS],
    /// Which way round the thing kept in each pack cell lies — the
    /// body's `Gear::turned`.
    pub turned: [bool; PACK_CELLS],
    /// Still dead or out cold. A crewmate that came round is no longer a
    /// body, and the window shuts on it.
    pub down: bool,
    /// Whether the Bim whose inventory is shown stands within reach of
    /// it, alive and awake — `World::in_reach_of_body`.
    pub reach: bool,
}

/// What a row or a ctrl-click asked for, about somebody's gear. The
/// screen sends it: on the ship as the matching `world::Command`, in the
/// room straight to the `Game`.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum GearOrder {
    /// Put what is in a pack cell into a container.
    Stow { who: u32, cell: u32 },
    /// Take a piece by its id, or one unit of a resource, into the pack.
    Fetch { who: u32, kind: FetchKind },
    /// Put on what is in a pack cell.
    Equip { who: u32, cell: u32 },
    /// Take off what is worn on a part, into the pack.
    Unequip { who: u32, part: health::Part },
    /// Throw away what is in a pack cell.
    Discard { who: u32, cell: u32 },
    /// Move the thing kept in `cell` of `who`'s pack so its corner is in
    /// `to`, turned or not — a drag across the pack, `Command::Repack`.
    Repack {
        who: u32,
        cell: u32,
        to: u32,
        turned: bool,
    },
    /// Move a slot of a class's grid to a cell, turned or not — a drag in
    /// a container window, or `R` over a thing there — `Command::Arrange`.
    Arrange {
        class: Storage,
        id: u32,
        x: u32,
        y: u32,
        turned: bool,
    },
    /// Take one cell off a body — `cell` a `bims::combat::LootCell` code
    /// — into the pack.
    Loot {
        who: u32,
        source: LootSource,
        cell: u32,
    },
    /// Hire the mercenary that is that resident of the station, `who`
    /// doing the hiring — `Command::Hire`.
    Hire { who: u32, resident: u32 },
    /// Finish off the resident lying out cold, `who` doing it —
    /// `Command::Execute`.
    Execute { who: u32, resident: u32 },
    /// Take the research key off the station's desk into `who`'s pack —
    /// `Command::TakeKey`. Sent by the screen once `who` is within reach
    /// of the desk, after the desk's row walked them there.
    TakeKey { who: u32 },
}

/// Something within reach of the Bim shown, for the nearby strip: the
/// window it opens and what to call it.
#[derive(Clone, PartialEq, Debug)]
pub struct Near {
    pub open: Open,
    pub label: String,
}

/// What is up in the window beside the inventory: a container's grid, a
/// body's, or a mercenary's terms.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Open {
    Container(Container),
    Loot(LootSource),
    Hire(u32),
    /// Not a window either: the Kill row on one of the station's people
    /// lying out cold — `Command::Execute` through `GearOrder::Execute`,
    /// the Bim shown walking over to shoot it where it lies, or to cut it
    /// from beside it with a blade.
    Kill(u32),
    /// Not a window of the panels' own: the Trade row walks the Bim to
    /// the desk and asks the screen for the trade window
    /// (`CrewPanels::trade_requested`).
    Trade(usize),
    /// Not a window either: the station's research desk's row walks
    /// the Bim shown to it and asks the screen for the key
    /// (`CrewPanels::key_requested`).
    Key(usize),
}

/// A cell's pop-up: where it was asked for, which cell, whose gear, and
/// the same freshness guard as a fixture menu's.
struct CellMenu {
    at: egui::Pos2,
    from: Source,
    who: usize,
    fresh: bool,
}

/// Which grid a cell pop-up is about.
#[derive(Clone, Copy, PartialEq, Debug)]
enum Source {
    /// A cell of the pack.
    Pack(usize),
    /// A cell of the open container window.
    Hold(HoldCell),
    /// A worn slot.
    Worn(health::Part),
    /// A cell of the open Loot window, by its `LootCell` code; whose body
    /// is the window's.
    Loot(u32),
}

/// A cell of a container window: one slot of a class's grid by its id —
/// a piece of armour, a gun at its tier, or a stack of anything else kept
/// there — or, on the research desk, which is no grid, the key.
#[derive(Clone, Copy, PartialEq, Debug)]
enum HoldCell {
    Slot(Storage, u32),
    Stack(ResourceId),
}

/// Which fixture answers each of the four targets: the bay grows the
/// first two and the last, the hob makes the third. Indexed by
/// `manager::Stock`.
const KEEP_SPOTS: [u32; 4] = [SPOT_BAY, SPOT_BAY, SPOT_HOB, SPOT_BAY];

/// Which fixture each job on the work list is about, so resting on a row
/// rings the place it happens — every fixture of that kind, since a row
/// names a kind: both hobs for cooking, every bay for the bay jobs. Hauling
/// is the crop carry, and where a haul *ends* is the thing worth pointing
/// at. Indexed by `work::Job` code; the test below pins the length.
const WORK_SPOTS: [u32; 10] = [
    SPOT_LOCKER,
    SPOT_BAY,
    SPOT_BAY,
    SPOT_FRIDGE,
    SPOT_HOB,
    SPOT_HELM,
    SPOT_BENCH,
    SPOT_SUIT_LOCKER,
    // A site is wherever it was laid out; nothing fixed to ring.
    SPOT_NOTHING,
    // A patient is wherever it fell, and a ring round a body would be a
    // selection; nothing fixed to ring.
    SPOT_NOTHING,
];

#[derive(Clone, Copy, PartialEq, Eq)]
enum Tab {
    Schedule,
    Work,
    Management,
    /// What the selected crew member has on it: the armour slots and
    /// the weapon, with its numbers. On both screens.
    Inventory,
    /// What the ship view is drawn to show over the ship: the plain deck,
    /// or the electricity. Only on the ship's screen, like Actions.
    View,
    /// Things the player does with the pointer on the crew's behalf. Only
    /// on the ship's screen: the room has no outside.
    Actions,
    /// Parts to lay out for the crew to build, by category, with a search
    /// box over them; and the sites laid out so far. Only on the ship's
    /// screen: the room is not a ship.
    Build,
    /// The research tree: what the crew know, what the AI is on, and what
    /// a key would open. Only on the ship's screen: research is the
    /// world's.
    Research,
    /// The helm and the ship's facts. Only on the ship's screen, and drawn
    /// by it: the tray lays out the tabs and leaves the body to the
    /// caller, since everything on it is the world's rather than the
    /// room's.
    Ship,
}

/// A tool the pointer is holding, picked on the Actions tab. One at a time,
/// and none is the ordinary pointer.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Tool {
    /// Marking rocks outside to be mined: a click on a rock marks it, a
    /// second click unmarks it.
    Mine,
    /// Laying out a part of this kind: a blueprint follows the pointer, `R`
    /// turns it, and a click lays it down as a construction site for the
    /// crew to carry to and build.
    Build(PartKind),
}

/// One thing the benches make, for the Management tab's second table: what
/// it is, how many are aboard, where they are kept, the standing order to
/// keep that many made, and the most the hold could take.
pub struct Craft {
    pub resource: ResourceId,
    pub held: u32,
    pub target: u32,
    pub most: u32,
    pub kept_in: &'static str,
    /// The recipe in words, for the tooltip.
    pub recipe: String,
    /// The node of the research tree the recipe waits on, while it is not
    /// researched: the row is greyed and says so.
    pub needs: Option<u32>,
}

/// What the Actions tab has to know that the room does not: whether the
/// ship is at a mining site and how many rocks are marked, and — going the
/// other way — that the marks are to be cleared.
pub struct Actions {
    pub at_site: bool,
    pub marked: usize,
    /// How many of those a walk could get to: the rest wait on a rock in
    /// front of them.
    pub reachable: usize,
    pub clear: bool,
    /// And what the View tab shows over the ship. Read in and written
    /// back, like `clear`.
    pub overlay: Overlay,
    /// What the benches make, for the Management tab, and — going the
    /// other way — the targets changed there this frame, as `Order::Keep`s
    /// to be.
    pub crafts: Vec<Craft>,
    pub keep: Vec<(ResourceId, u32)>,
    /// The Build tab: whether the ship is at rest, which is the only time
    /// anything is laid out or built; how much of each material is aboard
    /// and not spoken for by a site, by `ResourceId`; the sites laid out;
    /// and — going the other way — the sites to be called off.
    pub at_rest: bool,
    pub free: [u32; CARGO_SLOTS],
    pub sites: Vec<Site>,
    pub cancel: Vec<u32>,
    /// The camera, for the View tab: which way is up, and whether it
    /// follows the crew member you steer. Read in and written back.
    pub head_up: bool,
    pub follow: bool,
    /// Whether the ship is docked, which is when there is a station to
    /// trade with; and — going the other way — that the Station button
    /// was pressed this frame.
    pub docked: bool,
    pub station: bool,
    /// The Research tab: the tree as the crew stand in it; and — going
    /// the other way — what was asked of the AI this frame.
    pub research: ResearchView,
    pub research_orders: Vec<ResearchOrder>,
    /// The workbench's upgrade: whether the crew combine matching gear —
    /// `World::auto_upgrade` — and, going the other way, the box ticked or
    /// unticked this frame; and what is on the bench, for the line under it.
    pub auto_upgrade: bool,
    pub set_auto_upgrade: Option<bool>,
    pub upgrade: Option<UpgradeView>,
}

/// What is on the workbench being upgraded, as the Management tab says it:
/// a snapshot off `World::upgrade`.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct UpgradeView {
    pub resource: ResourceId,
    /// The tier it comes out at, as `Tier::code`.
    pub tier: u32,
    /// Sessions done, of `of` — hours of a day.
    pub done: u32,
    pub of: u32,
    /// Done, and waiting for room in the lockers.
    pub waiting: bool,
}

/// The research tree, as the panels see it: a snapshot the screen hands
/// over every frame off `World::research`.
#[derive(Clone, Default)]
pub struct ResearchView {
    /// By `Node` code.
    pub done: [bool; NODES],
    /// By `Node` code: could be begun now.
    pub available: [bool; NODES],
    /// By `Node` code: waiting on its tier's key and nothing else.
    pub needs_key: [bool; NODES],
    /// By `Node` code: a locked node whose key has been consumed.
    pub unlocked: [bool; NODES],
    /// What the AI is on, and how far, nought to one.
    pub current: Option<u32>,
    pub fraction: f64,
    /// A research desk aboard, and running.
    pub desk: bool,
    pub powered: bool,
    /// Keys in the crew's own desk.
    pub keys: u32,
    /// Whether each part may be laid out, by `PartKind` code.
    pub parts: Vec<bool>,
}

/// What the Research tab asked for.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum ResearchOrder {
    /// Put the AI onto a node — `Command::Research`.
    Begin(u32),
    /// Take it off — `Command::CancelResearch`.
    Cancel,
    /// Consume the key in the desk to open a locked node — `Command::Unlock`.
    Unlock(u32),
}

/// One construction site, for the Build tab's list: what and where, how
/// much of what it is made of has been carried to it in words, and whether
/// all of it has.
pub struct Site {
    pub id: u32,
    pub kind: PartKind,
    pub at: (u32, u32),
    pub progress: String,
    pub stocked: bool,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Sort {
    Up,
    Down,
    Name,
}

/// An open fixture menu: which fixture, and where on the window it was
/// asked for.
pub struct Menu {
    fixture: u32,
    at: egui::Pos2,
    /// Opened this frame: the press that opened it must not shut it.
    fresh: bool,
}

/// What a menu row does when it is clicked: walks the player's Bim over
/// to do it.
type Errand = Box<dyn FnOnce(&mut Game)>;

/// One row of a fixture's menu: an errand for the Bim, or a window to
/// open — the cold store's "Open", a body's "Loot" — which is the panels'
/// to do rather than the room's.
struct Item {
    label: String,
    hint: String,
    disabled: bool,
    run: Option<Errand>,
    opens: Option<Open>,
}

impl Item {
    fn note(label: &str, hint: String) -> Item {
        Item {
            label: label.into(),
            hint,
            disabled: true,
            run: None,
            opens: None,
        }
    }

    fn run(
        label: impl Into<String>,
        hint: impl Into<String>,
        disabled: bool,
        f: impl FnOnce(&mut Game) + 'static,
    ) -> Item {
        Item {
            label: label.into(),
            hint: hint.into(),
            disabled,
            run: Some(Box::new(f)),
            opens: None,
        }
    }

    fn opens(label: impl Into<String>, hint: impl Into<String>, what: Open) -> Item {
        Item {
            label: label.into(),
            hint: hint.into(),
            disabled: false,
            run: None,
            opens: Some(what),
        }
    }
}

pub struct CrewPanels {
    /// The one the mouse steers.
    pub player: usize,
    pub crew_count: u32,
    tray_open: bool,
    tab: Tab,
    /// The tool the pointer is holding, if any. See [`Tool`].
    pub tool: Option<Tool>,
    brush: u32,
    /// Whether a drag across the timetable is under way.
    painting: bool,
    /// The character sheet's page, per crew member: `true` for the diary.
    diary_open: Vec<bool>,
    sort: Option<Sort>,
    /// The spot the pointer's row rang last frame, and the one the rows
    /// hovered this frame want.
    ringed: u32,
    wanted: u32,
    menu: Option<Menu>,
    /// The Build tab: what is typed in its search box, and which category
    /// is open — `None` for the list of categories.
    search: String,
    open_group: Option<usize>,
    /// The Research tab: the node picked, whose details are under the tree.
    research_pick: Option<Node>,
    /// The inventory pop-up: up from the moment the player's crew member
    /// is recruited, or a container is opened, until it is shut or they
    /// are let go; and whether they were recruited last frame, which is
    /// how the moment is noticed.
    inventory_open: bool,
    was_recruited: bool,
    /// The hold, as the ship's screen last handed it over; `None` in the
    /// room. See the module note.
    pub hold: Option<Hold>,
    /// The window that is up beside the inventory, if one is — a
    /// container's or a body's — and the rect it took this frame, which is
    /// where the inventory pop-up goes beside it.
    open: Option<Open>,
    container_rect: Option<egui::Rect>,
    /// The body the Loot window is over, as the screen last handed it
    /// over; `None` while none is open, or once the Bim is gone. See the
    /// module note.
    pub body: Option<Body>,
    /// Whether the station alongside is an enemy's, the world's word every
    /// frame: what puts the Kill row on a downed visitor's menu. Only an
    /// enemy's people are finished off; the command refuses the rest.
    pub enemies_alongside: bool,
    /// The body the Loot row just opened the window on: the screen walks
    /// the Bim shown over to it and takes this. See the module note.
    pub walk: Option<LootSource>,
    /// The mercenary's terms the Hire window is over, as the screen last
    /// handed them; `None` while none is open, or once the body is no
    /// longer for hire — hired, or the rooms parted — which shuts it.
    pub terms: Option<Terms>,
    /// The Trade row was picked: the screen opens the trade window and
    /// takes this.
    pub trade_requested: bool,
    /// The station desk's Take row was picked for this Bim: the screen
    /// sends the take once they are within reach, and takes this.
    pub key_requested: Option<u32>,
    /// A cell's pop-up, if one is up.
    cell_menu: Option<CellMenu>,
    /// What is within reach of the Bim shown, nearest first — a container
    /// or a body down, with a name for the strip — as the screen last
    /// handed it over (`Near`). Fresh every frame: the Bim is walking.
    pub nearby: Vec<Near>,
    /// A thing being carried across the armoury window, between frames,
    /// and one across the pack.
    locker_drag: Option<grid::Drag>,
    pack_drag: Option<grid::Drag>,
    /// The player's keys, as the screen last handed them over: what turns
    /// a thing in the armoury is the Turn binding.
    pub keys: Keys,
    /// What the rows and the ctrl-clicks asked for this frame, for the
    /// screen to send. Drained by it.
    pub orders: Vec<GearOrder>,
}

impl CrewPanels {
    pub fn new(player: usize, crew_count: u32) -> CrewPanels {
        CrewPanels {
            player,
            crew_count,
            tray_open: true,
            tab: Tab::Schedule,
            tool: None,
            brush: 1,
            painting: false,
            diary_open: vec![false; crew_count as usize],
            sort: Some(Sort::Up),
            ringed: SPOT_NOTHING,
            wanted: SPOT_NOTHING,
            menu: None,
            search: String::new(),
            open_group: None,
            research_pick: None,
            inventory_open: false,
            was_recruited: false,
            hold: None,
            open: None,
            container_rect: None,
            body: None,
            enemies_alongside: false,
            walk: None,
            terms: None,
            trade_requested: false,
            key_requested: None,
            cell_menu: None,
            nearby: Vec::new(),
            locker_drag: None,
            pack_drag: None,
            keys: Keys::default(),
            orders: Vec::new(),
        }
    }

    /// The room's crew has changed size — a ship docking brings a station's
    /// residents into its room, and leaving takes them out again.
    pub fn rebuild_crew(&mut self, count: u32) {
        if count == self.crew_count {
            return;
        }
        self.crew_count = count;
        self.diary_open = vec![false; count as usize];
        self.menu = None;
        self.cell_menu = None;
        // The room was rebuilt with it, and its benches and shelves are
        // numbered afresh: the window would be over somebody else's — and
        // a body being looted has gone with the room it lay in.
        self.open = None;
        self.body = None;
        self.walk = None;
        self.terms = None;
    }

    // --- pointing at the thing itself ---------------------------------------

    /// Ring `spot` on the deck while the pointer is on `response`. Rows are
    /// drawn outer first, so a cell inside a row that points somewhere else
    /// wins by coming later.
    pub fn points(&mut self, response: &egui::Response, spot: u32) {
        if response.hovered() && spot != SPOT_NOTHING {
            self.wanted = spot;
        }
    }

    /// Call once a frame, before any panel: nothing wants a ring yet.
    pub fn begin_frame(&mut self) {
        self.wanted = SPOT_NOTHING;
    }

    /// Call once a frame, after every panel: the ring follows the pointer.
    pub fn end_frame(&mut self, game: &mut Game) {
        if self.wanted != self.ringed {
            self.ringed = self.wanted;
            game.set_highlight(self.ringed);
        }
    }

    // --- fixture menus ------------------------------------------------------

    /// A click landed on a fixture: its menu opens at the pointer — or,
    /// for a container on the ship, its window. A workstation is a
    /// container when its part keeps a class of goods (the armoury and
    /// the drug lab are lockers; the smelter keeps nothing and has no
    /// menu either), a shelf always is; the cold store keeps its menu,
    /// which has an "Open" row. Nothing is a container in the test room,
    /// which has no hold.
    pub fn open_menu(&mut self, fixture: u32, at: egui::Pos2, game: &mut Game) {
        let container = match fixture {
            HIT_BENCH if self.hold.is_some() => {
                let bench = game.hit_bench();
                PartKind::from_code(game.bench_part(bench))
                    .and_then(|kind| kind.def().capacity)
                    .map(|_| Container::Bench(bench))
            }
            HIT_SHELF if self.hold.is_some() => Some(Container::Shelf(game.hit_shelf())),
            // The ship's own research desk is a container — the key's slot;
            // the station's has a row instead.
            HIT_RESEARCH
                if self
                    .hold
                    .as_ref()
                    .is_some_and(|h| h.station_desk != Some(game.hit_research())) =>
            {
                Some(Container::Desk(game.hit_research()))
            }
            _ => None,
        };
        if let Some(container) = container {
            self.open_container(game, container);
            return;
        }
        self.cell_menu = None;
        self.menu = Some(Menu {
            fixture,
            at,
            fresh: true,
        });
    }

    /// Open a container's window, and the inventory pop-up of the Bim
    /// shown beside it, and walk that Bim to the container's use spot:
    /// nothing moves until it is within reach, and the click is the
    /// natural place to start it walking.
    fn open_container(&mut self, game: &mut Game, container: Container) {
        let who = self.inventory_who(game);
        if let Some(spot) = game.container_spot(container)
            && game.is_alive(who)
        {
            game.send_to(who, spot);
        }
        self.open = Some(Open::Container(container));
        self.inventory_open = true;
        self.menu = None;
        self.cell_menu = None;
    }

    /// Open a container's window by name, the way a click on it would:
    /// `BIMS_ARMOURY=1` (or `armoury`) the first bench aboard whose part
    /// is an armoury, `storage` the first shelf, `fridge` the first cold
    /// store. Nothing, on a ship without one.
    pub fn open_named(&mut self, game: &mut Game, what: &str) {
        let container = match what {
            "storage" => game
                .container_frame(Container::Shelf(0))
                .map(|_| Container::Shelf(0)),
            "fridge" => game
                .container_frame(Container::Fridge(0))
                .map(|_| Container::Fridge(0)),
            _ => (0..game.benches().len())
                .find(|&i| game.bench_part(i) == PartKind::Armoury.code())
                .map(Container::Bench),
        };
        if let Some(container) = container {
            self.open_container(game, container);
        }
    }

    /// Open the Loot window on a body, and the inventory pop-up beside it,
    /// the way a container's opens — except that the walk over is left
    /// to the screen (`walk`): where one of the station's people lies is
    /// the world's to say, not the room's.
    pub fn open_loot(&mut self, source: LootSource) {
        self.open = Some(Open::Loot(source));
        self.walk = Some(source);
        self.body = None;
        self.inventory_open = true;
        self.menu = None;
        self.cell_menu = None;
    }

    /// The body under a plain left click, if a body is what was clicked:
    /// a dead crewmate (`HIT_BODY`), one out cold (`HIT_BIM` with the Bim
    /// down), or one of the station's people the world marked down
    /// (`HIT_VISITOR`, down). A click on one opens its inventory straight
    /// off — the Loot window, the way the row would — rather than a menu;
    /// the right-click keeps the rows. `None` for anything else.
    pub fn body_under_click(&self, game: &Game, fixture: u32) -> Option<LootSource> {
        match fixture {
            HIT_BODY => Some(LootSource::Crew(game.hit_body() as u32)),
            HIT_BIM => {
                let who = game.hit_bim();
                game.is_down(who).then_some(LootSource::Crew(who as u32))
            }
            HIT_VISITOR => {
                let body = game.hit_visitor();
                game.visitor_down(body)
                    .then_some(LootSource::Resident(body as u32))
            }
            _ => None,
        }
    }

    /// The body the Loot window is up on, if it is: what the screen hands
    /// a [`Body`] for.
    pub fn loot_source(&self) -> Option<LootSource> {
        match self.open {
            Some(Open::Loot(source)) => Some(source),
            _ => None,
        }
    }

    /// Open the Hire window on one of the station's people, the way the
    /// Loot window opens on a body: the walk over is the screen's, and
    /// the terms are handed in every frame ([`Terms`]).
    fn open_hire(&mut self, resident: u32) {
        self.open = Some(Open::Hire(resident));
        self.walk = Some(LootSource::Resident(resident));
        self.terms = None;
        self.menu = None;
        self.cell_menu = None;
    }

    /// The resident the Hire window is up on, if it is: what the screen
    /// hands [`Terms`] for.
    pub fn hire_source(&self) -> Option<u32> {
        match self.open {
            Some(Open::Hire(resident)) => Some(resident),
            _ => None,
        }
    }

    /// Shut the menus — a fixture's, a cell's — the way a click away
    /// does. A container window stays: a click on the deck beside it is
    /// how a Bim is picked to use it.
    pub fn close_menu(&mut self) {
        self.menu = None;
        self.cell_menu = None;
    }

    /// Escape: the innermost thing up goes first — a cell's pop-up, then a
    /// fixture's menu, then the container or the Loot window. `true` when something
    /// was shut, so the screen knows the key is spent.
    pub fn escape(&mut self) -> bool {
        if self.cell_menu.is_some() {
            self.cell_menu = None;
        } else if self.menu.is_some() {
            self.menu = None;
        } else if self.open.is_some() {
            self.open = None;
        } else {
            return false;
        }
        true
    }

    /// Who has the run of a shared part of the ship, or `None` for nobody.
    /// The room counts from 1 so that 0 can mean "free"; this turns that
    /// back into a name, and into `None` when it is the player's own Bim —
    /// being told you cannot cook because you are already cooking is no
    /// help.
    fn held_by(&self, code: u32, name: &dyn Fn(u32) -> String) -> Option<String> {
        if code == 0 || code as usize - 1 == self.player {
            return None;
        }
        Some(name(code - 1))
    }

    /// Build the rows for the fixture that was clicked. Nothing here is
    /// remote: every item walks the Bim over to do it by hand. Nor does
    /// anything wait for the Bim to be free — a new errand takes over, and
    /// what it displaced goes on the agenda to be finished afterwards.
    /// Every menu acts on the player's Bim; the other crew take no orders.
    fn items(&self, fixture: u32, game: &Game, name: &dyn Fn(u32) -> String) -> Vec<Item> {
        let who = self.player;
        let busy = game.is_busy(who);
        let takes_over: Option<&str> = busy.then_some("takes over — the rest waits its turn");
        // The fixture the click landed on, of each kind — there may be
        // several of any of them — and who has it. A meal is kept from
        // starting only when no galley at all is free.
        let (hob, fridge, washer) = (game.hit_hob(), game.hit_fridge(), game.hit_dishwasher());
        let (bath, shower_i, locker) = (game.hit_bath(), game.hit_shower(), game.hit_locker());
        let galley = self.held_by(game.galley_busy_by(who), name);
        let hob_held = self.held_by(game.hob_held_by(hob), name);
        let fridge_held = self.held_by(game.fridge_held_by(fridge), name);
        let washer_held = self.held_by(game.dishwasher_held_by(washer), name);
        let heads = self.held_by(game.heads_held_by(bath), name);
        let shower = self.held_by(game.shower_held_by(shower_i), name);
        let in_galley = |g: &Option<String>| g.as_ref().map(|n| format!("{n} is in the galley"));
        let mut items = Vec::new();
        match fixture {
            HIT_FRIDGE => {
                let veg = game.store_veg();
                let tofu = game.store_tofu();
                let no_stew = veg < 2;
                let no_bowl = tofu < 1 || veg < 1;
                items.push(Item::note(
                    "Cold store",
                    format!(
                        "{veg} veg, {tofu} tofu, {} stew ready — keeping {}, {} and {}",
                        game.store_stew(),
                        game.target(Stock::Veg),
                        game.target(Stock::Tofu),
                        game.target(Stock::Stew)
                    ),
                ));
                items.push(Item::run(
                    "Make a stew",
                    in_galley(&galley).unwrap_or_else(|| {
                        if no_stew {
                            "needs two vegetables".into()
                        } else {
                            takes_over
                                .unwrap_or("two vegetables, chopped and cooked")
                                .into()
                        }
                    }),
                    no_stew || galley.is_some(),
                    move |g| {
                        g.cook(who, Dish::Stew);
                    },
                ));
                items.push(Item::run(
                    "Make a bowl",
                    in_galley(&galley).unwrap_or_else(|| {
                        if no_bowl {
                            "needs a block of tofu and a salad".into()
                        } else {
                            takes_over
                                .unwrap_or("tofu chopped in with the salad, no cooking")
                                .into()
                        }
                    }),
                    no_bowl || galley.is_some(),
                    move |g| {
                        g.cook(who, Dish::Bowl);
                    },
                ));
                items.push(Item::run(
                    if game.fridge_is_open(fridge) {
                        "Close door"
                    } else {
                        "Open door"
                    },
                    in_galley(&fridge_held)
                        .unwrap_or_else(|| takes_over.unwrap_or("the Bim walks over to it").into()),
                    fridge_held.is_some(),
                    move |g| g.toggle_fridge(who, fridge),
                ));
                // On the ship the cold store is a container as well: its
                // window is the hold's cold class, the way the armoury's
                // is the lockers.
                if self.hold.is_some() {
                    items.push(Item::opens(
                        "Open",
                        "what is in it, a cell a thing — the Bim walks over to reach in",
                        Open::Container(Container::Fridge(fridge)),
                    ));
                }
            }
            HIT_STOVE => {
                let left = game.pot_servings(hob);
                if left > 0 {
                    let capacity = game.pot_capacity();
                    items.push(Item::run(
                        "Eat from the pot",
                        in_galley(&galley).unwrap_or_else(|| {
                            takes_over.map(|s| s.to_string()).unwrap_or_else(|| {
                                format!("{left} of {capacity} helpings left — no cooking")
                            })
                        }),
                        galley.is_some(),
                        move |g| {
                            g.eat_leftovers(who);
                        },
                    ));
                }
                let no_stock = game.store_veg() < 1 || game.store_tofu() < 1;
                let ready = game.store_stew();
                let keeping = game.target(Stock::Stew);
                items.push(Item::run(
                    "Cook a stew for the store",
                    in_galley(&galley).unwrap_or_else(|| {
                        if no_stock {
                            "needs a vegetable and a block of tofu".into()
                        } else {
                            takes_over
                                .map(|s| s.to_string())
                                .unwrap_or_else(|| format!("{ready} ready — keeping {keeping}"))
                        }
                    }),
                    no_stock || galley.is_some(),
                    move |g| {
                        g.make_stew(who);
                    },
                ));
                let idle_left = game.stove_idle_left(hob);
                items.push(Item::run(
                    if game.stove_is_on(hob) {
                        "Turn off"
                    } else {
                        "Turn on"
                    },
                    in_galley(&hob_held).unwrap_or_else(|| {
                        takes_over.map(|s| s.to_string()).unwrap_or_else(|| {
                            if idle_left > 0.0 {
                                format!("left on — cuts out in {}", span_text(idle_left))
                            } else {
                                "the Bim walks over to it".into()
                            }
                        })
                    }),
                    hob_held.is_some(),
                    move |g| g.toggle_stove(who, hob),
                ));
            }
            HIT_DISHWASHER => {
                let loaded = game.dishwasher_loaded(washer);
                let capacity = game.dishwasher_capacity();
                let left = game.dishwasher_cycle_left(washer);
                let now = game.clock_minutes();
                if left > 0.0 {
                    items.push(Item::note(
                        "Running",
                        format!(
                            "{} left — done at {}",
                            span_text(left),
                            clock_text(now + left)
                        ),
                    ));
                }
                items.push(Item::run(
                    "Run now",
                    in_galley(&washer_held).unwrap_or_else(|| {
                        if left > 0.0 {
                            "already running".into()
                        } else if loaded == 0 {
                            "nothing in it".into()
                        } else {
                            takes_over.map(|s| s.to_string()).unwrap_or_else(|| {
                                format!(
                                    "{loaded} of {capacity} stowed — the Bim goes and presses it"
                                )
                            })
                        }
                    }),
                    left > 0.0 || loaded == 0 || washer_held.is_some(),
                    move |g| g.run_dishwasher(who, washer),
                ));
            }
            HIT_SHOWER => {
                let can = game.can_shower(who);
                items.push(Item::run(
                    "Take a shower",
                    shower
                        .as_ref()
                        .map(|n| format!("{n} is in it"))
                        .unwrap_or_else(|| {
                            if can {
                                takes_over.unwrap_or("and come out clean").into()
                            } else {
                                "can't get to it".into()
                            }
                        }),
                    !can || shower.is_some(),
                    move |g| {
                        g.take_shower(who);
                    },
                ));
            }
            HIT_TOILET => {
                let can = game.can_use_toilet(who);
                items.push(Item::run(
                    "Use",
                    heads
                        .as_ref()
                        .map(|n| format!("{n} is in there"))
                        .unwrap_or_else(|| {
                            if can {
                                takes_over.unwrap_or("and wash at the basin after").into()
                            } else {
                                "can't get to it — the door is locked".into()
                            }
                        }),
                    !can || heads.is_some(),
                    move |g| {
                        g.use_toilet(who);
                    },
                ));
            }
            HIT_DOOR => {
                let open = game.door_is_open();
                let locked = game.door_is_locked();
                let in_there = heads.as_ref().map(|n| format!("{n} is in there"));
                items.push(Item::run(
                    if open { "Close door" } else { "Open door" },
                    in_there.clone().unwrap_or_else(|| {
                        if locked {
                            "unlock it first".into()
                        } else {
                            takes_over.unwrap_or("the Bim walks over to it").into()
                        }
                    }),
                    locked || heads.is_some(),
                    move |g| g.toggle_door(who),
                ));
                items.push(Item::run(
                    if locked { "Unlock" } else { "Lock" },
                    in_there.unwrap_or_else(|| {
                        takes_over
                            .unwrap_or(if locked {
                                "at the panel"
                            } else {
                                "shuts it as well"
                            })
                            .into()
                    }),
                    heads.is_some(),
                    move |g| g.toggle_door_lock(who),
                ));
            }
            HIT_SHIP_DOOR => {
                // A powered door in a bulkhead: "Open" holds it open,
                // "Close" hands it back to itself, and a locked door is a
                // wall until it is unlocked.
                let door = game.hit_door();
                let held = game.ship_door_is_held(door);
                let locked = game.ship_door_is_locked(door);
                items.push(Item::run(
                    if held { "Close door" } else { "Hold door open" },
                    if locked {
                        "unlock it first".to_string()
                    } else {
                        takes_over
                            .unwrap_or(if held {
                                "and let it shut behind people again"
                            } else {
                                "so it stops shutting itself"
                            })
                            .into()
                    },
                    locked,
                    move |g| {
                        g.order_door(
                            who,
                            door,
                            if held {
                                door::Order::Close
                            } else {
                                door::Order::Open
                            },
                        )
                    },
                ));
                items.push(Item::run(
                    if locked { "Unlock" } else { "Lock" },
                    takes_over
                        .unwrap_or(if locked {
                            "at the panel"
                        } else {
                            "shuts it, and nobody gets through"
                        })
                        .to_string(),
                    false,
                    move |g| {
                        g.order_door(
                            who,
                            door,
                            if locked {
                                door::Order::Unlock
                            } else {
                                door::Order::Lock
                            },
                        )
                    },
                ));
            }
            HIT_LOCKER => {
                let dirty = game.dirty_tiles();
                let broom = self.held_by(game.broom_held_by(locker), name);
                items.push(Item::run(
                    "Sweep up",
                    broom
                        .as_ref()
                        .map(|n| format!("{n} has the broom"))
                        .unwrap_or_else(|| {
                            if dirty == 0 {
                                "the deck is clean".into()
                            } else {
                                takes_over.map(|s| s.to_string()).unwrap_or_else(|| {
                                    format!(
                                        "{dirty} patch{} of deck want it",
                                        if dirty == 1 { "" } else { "es" }
                                    )
                                })
                            }
                        }),
                    dirty == 0 || broom.is_some(),
                    move |g| {
                        g.sweep_up(who);
                    },
                ));
            }
            HIT_HYDRO => {
                // The bay the click landed on: each has its own trays, its
                // own switch and its own standing order.
                let bay = game.hit_bay();
                let spots = game.hydro_spots();
                let automated = game.hydro_automated(bay);
                let asleep = game.hydro_hibernating(bay);
                let forced = game.hydro_forced(bay);
                let ripe = game.hydro_ripe(bay);
                let mut growing = 0;
                let mut furthest: f32 = 0.0;
                for i in 0..spots {
                    if game.hydro_crop(bay, i) == 0 {
                        continue;
                    }
                    growing += 1;
                    furthest = furthest.max(game.hydro_growth(bay, i));
                }
                let along = if ripe > 0 || growing == 0 {
                    String::new()
                } else {
                    format!(", furthest {}% grown", (furthest * 100.0).round())
                };
                items.push(Item::note(
                    "Trays",
                    format!(
                        "{growing} of {spots} planted{}",
                        if ripe > 0 {
                            format!(", {ripe} ready to lift")
                        } else {
                            along
                        }
                    ),
                ));
                items.push(Item::note(
                    "Store",
                    format!(
                        "{} veg, {} tofu, {} fibre — keeping {}, {} and {}",
                        game.store_veg(),
                        game.store_tofu(),
                        game.store_fibre(),
                        game.target(Stock::Veg),
                        game.target(Stock::Tofu),
                        game.target(Stock::Fibre)
                    ),
                ));
                items.push(Item::run(
                    if automated {
                        "Stop automating"
                    } else {
                        "Automate"
                    },
                    if automated {
                        if asleep {
                            "at target — holding what is planted"
                        } else {
                            "following the manager's target"
                        }
                    } else {
                        "grow whatever the store is short of"
                    },
                    false,
                    move |g| g.set_hydro_automated(bay, !automated),
                ));
                for (code, label, what) in [
                    (1, "Plant greens in every tray", "two of these in a stew"),
                    (
                        2,
                        "Plant soy in every tray",
                        "a day and a half, and it presses into tofu",
                    ),
                    (
                        3,
                        "Plant fibre in every tray",
                        "a day, and two of it make a bandage at the drug lab",
                    ),
                ] {
                    let on = forced == code;
                    items.push(Item::run(
                        if on {
                            format!("{label} ✓")
                        } else {
                            label.to_string()
                        },
                        if on {
                            "standing order — click to lift it".to_string()
                        } else {
                            format!("no matter the target · {what}")
                        },
                        false,
                        move |g| g.set_hydro_forced(bay, if on { 0 } else { code }),
                    ));
                }
            }
            HIT_BIM => {
                // A body on the deck — the player's own, or a crewmate: a
                // row a part of it, saying what is open there, and the
                // player's Bim walks over and dresses the one picked. The
                // patient may be anybody alive; the hands are always the
                // player's.
                let patient = game.hit_bim();
                let bandages = game.bandages();
                let out = game.is_unconscious(who) || game.is_outside(who);
                let patient_out = game.is_outside(patient);
                for (i, part) in health::Part::ALL.into_iter().enumerate() {
                    let wounds = game.wounds(patient, part);
                    let (count, hint) = bandage_words(wounds, bandages);
                    let can = wounds > 0 && bandages > 0 && !out && !patient_out;
                    items.push(Item::run(
                        format!("Bandage the {} · {count}", SLOT_NAMES[i].to_lowercase()),
                        if out {
                            HELPER_OUT.to_string()
                        } else if patient_out {
                            PATIENT_OUT.to_string()
                        } else if can {
                            takes_over.unwrap_or(hint).to_string()
                        } else {
                            hint.to_string()
                        },
                        !can,
                        move |g| {
                            g.bandage(who, patient, part);
                        },
                    ));
                }
                items.push(Item::note("Bandages", format!("{bandages} to hand")));
                // A part at nothing: the dying state on it, and a medkit
                // in somebody else's hands the only way out. The player's
                // Bim treats a crewmate; for the player's own, the nearest
                // crewmate that is free is sent, since nobody treats
                // their own.
                let medkits = game.medkits();
                let dying: Vec<(usize, health::Part, bims::health::Trauma)> = health::Part::ALL
                    .into_iter()
                    .enumerate()
                    .filter_map(|(i, part)| game.trauma(patient, part).map(|t| (i, part, t)))
                    .collect();
                if !dying.is_empty() {
                    let helper = treat_helper(game, who, patient);
                    for (i, part, trauma) in dying {
                        let can = helper.is_some() && medkits > 0 && !patient_out;
                        let hint = if medkits == 0 {
                            "no medkits — the armoury makes them".to_string()
                        } else if patient_out {
                            PATIENT_OUT.to_string()
                        } else if let Some(h) = helper {
                            if h == who {
                                takes_over.unwrap_or(trauma_line(trauma.code())).to_string()
                            } else {
                                format!("{} walks over — nobody treats their own", name(h as u32))
                            }
                        } else if patient == who {
                            "nobody free to do it — nobody treats their own".to_string()
                        } else {
                            HELPER_OUT.to_string()
                        };
                        items.push(Item::run(
                            format!(
                                "Treat the {} · {}",
                                SLOT_NAMES[i].to_lowercase(),
                                trauma_name(trauma.code()).to_lowercase()
                            ),
                            hint,
                            !can,
                            move |g| {
                                if let Some(h) = helper {
                                    g.treat(h, patient, part);
                                }
                            },
                        ));
                    }
                    items.push(Item::note("Medkits", format!("{medkits} to hand")));
                }
                // Out cold, a crewmate is a body as well as a patient: the
                // Loot row sits beside the bandages, and which the player
                // means is theirs to say.
                if game.is_unconscious(patient) {
                    items.push(Item::opens(
                        LOOT_ROW,
                        format!(
                            "{} is out cold — everything on the body",
                            name(patient as u32)
                        ),
                        Open::Loot(LootSource::Crew(patient as u32)),
                    ));
                }
            }
            // A dead crew member, or one of a hostile station's people
            // lying in its own room: nothing to dress, and the one row is
            // the Loot window. A resident is named as the world's names
            // run — the crew first, then the station's people.
            HIT_BODY => {
                let body = game.hit_body() as u32;
                items.push(Item::opens(
                    LOOT_ROW,
                    format!("{} — everything on the body", name(body)),
                    Open::Loot(LootSource::Crew(body)),
                ));
            }
            HIT_VISITOR => {
                let body = game.hit_visitor() as u32;
                if game.visitor_down(body as usize) {
                    items.push(Item::opens(
                        LOOT_ROW,
                        format!("{} — everything on the body", name(self.crew_count + body)),
                        Open::Loot(LootSource::Resident(body)),
                    ));
                    // And, at an enemy's station, the end of it: the Bim
                    // shown walks over and shoots it where it lies, or cuts
                    // it from beside it with a blade. Greyed with nothing in
                    // hand; the command checks the rest.
                    if self.enemies_alongside {
                        let armed = game.weapon(who).is_some();
                        items.push(if armed {
                            Item::opens(
                                KILL_ROW,
                                format!(
                                    "{} — {}",
                                    name(self.crew_count + body),
                                    kill_hint(game.weapon(who).map(|w| w.kind))
                                ),
                                Open::Kill(body),
                            )
                        } else {
                            Item::note(KILL_ROW, KILL_UNARMED.to_string())
                        });
                    }
                } else {
                    // On its feet and hailable: a mercenary for hire. What
                    // it asks is the world's to say — the window reads it.
                    items.push(Item::opens(
                        HIRE_ROW,
                        format!(
                            "{} — a mercenary, paid by the month",
                            name(self.crew_count + body)
                        ),
                        Open::Hire(body),
                    ));
                }
            }
            // A weapon on the deck has no menu: the right-click itself picks
            // it up — the screen calls `Game::fetch` — into the pack of the
            // Bim shown, and the gun under the pointer is ringed on the deck
            // (`Game::set_hover_dropped`).
            HIT_DROPPED => {}
            HIT_DESK => {
                // A station's trading desk: the one row walks the Bim shown
                // over and puts the trade window up.
                let desk = game.hit_desk();
                items.push(Item::opens(
                    TRADE_ROW,
                    "walk to the desk and trade with the station",
                    Open::Trade(desk),
                ));
            }
            HIT_RESEARCH => {
                // A station's research desk: the one row walks the Bim
                // shown over and takes the key, if there is one.
                let desk = game.hit_research();
                let key = self.hold.as_ref().is_some_and(|h| h.station_key);
                if key {
                    items.push(Item::opens(KEY_ROW, KEY_ROW_HINT, Open::Key(desk)));
                } else {
                    items.push(Item::note(KEY_ROW, NO_KEY_ROW_HINT.into()));
                }
            }
            HIT_BED => {
                let now = game.clock_minutes();
                for (minutes, label) in [(task::NAP_MINUTES, "Nap"), (task::SLEEP_MINUTES, "Sleep")]
                {
                    items.push(Item::run(
                        format!("{label} — {}", span_text(minutes)),
                        takes_over
                            .map(|s| s.to_string())
                            .unwrap_or_else(|| format!("up around {}", clock_text(now + minutes))),
                        false,
                        move |g| {
                            g.rest(who, minutes);
                        },
                    ));
                }
            }
            _ => {}
        }
        items
    }

    /// Draw the open menu, if there is one, at the cursor it was opened
    /// at, and shut it on a click away.
    pub fn menu(&mut self, ctx: &egui::Context, game: &mut Game, name: &dyn Fn(u32) -> String) {
        let Some(menu) = &self.menu else { return };
        if !game.is_alive(self.player) {
            self.menu = None;
            return;
        }
        let items = self.items(menu.fixture, game, name);
        if items.is_empty() {
            self.menu = None;
            return;
        }
        let at = menu.at;
        let fresh = menu.fresh;
        let rows: Vec<theme::Row> = items
            .iter()
            .map(|item| theme::Row::new(item.label.clone(), item.hint.clone(), item.disabled))
            .collect();
        let (chosen, rect) = theme::popup(ctx, "fixture-menu", at, &rows);
        if let Some(i) = chosen {
            let item = items.into_iter().nth(i).unwrap();
            self.menu = None;
            match item.opens {
                Some(Open::Container(container)) => self.open_container(game, container),
                Some(Open::Loot(source)) => self.open_loot(source),
                Some(Open::Hire(resident)) => self.open_hire(resident),
                Some(Open::Kill(resident)) => {
                    let who = self.inventory_who(game) as u32;
                    self.orders.push(GearOrder::Execute { who, resident });
                }
                Some(Open::Trade(desk)) => {
                    if let Some(spot) = game.desk_spot(desk) {
                        game.send_to(self.inventory_who(game), spot);
                    }
                    self.trade_requested = true;
                }
                Some(Open::Key(desk)) => {
                    let who = self.inventory_who(game);
                    if let Some(spot) = game.research_spot(desk) {
                        game.send_to(who, spot);
                    }
                    self.key_requested = Some(who as u32);
                }
                None => {
                    if let Some(run) = item.run {
                        run(game);
                    }
                }
            }
            return;
        }
        // Clicking away dismisses it — but not the press that opened it.
        let pressed = ctx.input(|i| i.pointer.any_pressed());
        let pos = ctx.input(|i| i.pointer.interact_pos());
        if let Some(menu) = &mut self.menu {
            if fresh {
                menu.fresh = false;
            } else if pressed && !pos.is_some_and(|p| rect.contains(p)) {
                self.menu = None;
            }
        }
    }

    // --- what the crew want -------------------------------------------------

    /// The header that says whose panel this is.
    fn who_header(&self, ui: &mut egui::Ui, who: u32, name: &dyn Fn(u32) -> String) {
        ui.horizontal(|ui| {
            let yours = who as usize == self.player;
            ui.label(egui::RichText::new(name(who)).strong().color(if yours {
                theme::YOURS
            } else {
                theme::INK
            }));
            ui.label(
                egui::RichText::new(if yours { "yours" } else { "her own" })
                    .small()
                    .color(theme::MUTED),
            );
        });
    }

    /// The selected crew member's panels: the bars, the health lines and
    /// the character sheet. One at a time, and only when somebody is
    /// picked: the right-hand side answers "who am I looking at", not
    /// "what is everybody up to". `true` when something was drawn.
    pub fn side(
        &mut self,
        ui: &mut egui::Ui,
        game: &mut Game,
        name: &dyn Fn(u32) -> String,
    ) -> bool {
        let Some(who) = (0..self.crew_count).find(|&w| game.is_selected(w as usize)) else {
            return false;
        };
        let w = who as usize;
        let alive = game.is_alive(w);
        self.who_header(ui, who, name);

        // The needs. Only the first crew member's words are affordances:
        // the explanation is the same for everybody and two sets of
        // underlines down one edge of the screen is noise, not help.
        egui::Grid::new(("needs", who))
            .num_columns(3)
            .spacing([8.0, 3.0])
            .show(ui, |ui| {
                for i in 0..game.need_count() {
                    let label = NEED_NAMES.get(i as usize).copied().unwrap_or("Need");
                    let level = game.need_level(w, i);
                    let urgent = game.need_trigger_on(i) && level < game.need_trigger(i);
                    let color = if urgent { theme::WARN } else { theme::INK };
                    if who == 0 {
                        theme::asks(ui, label, need_tip(i as usize));
                    } else {
                        ui.label(egui::RichText::new(label).color(color));
                    }
                    theme::bar(
                        ui,
                        110.0,
                        level,
                        if urgent { theme::WARN } else { theme::ACCENT },
                    );
                    ui.label(
                        egui::RichText::new(format!("{}%", (level * 100.0).round()))
                            .small()
                            .color(theme::MUTED),
                    );
                    ui.end_row();
                }
            });

        // How it is bearing up. The armour worn adds its health to the
        // body's: the blue on the end of the green is what the pieces
        // still have, and the number reads the two apart.
        ui.add_space(4.0);
        let points = game.health(w);
        let armour = if alive { game.armour_health(w) } else { 0.0 };
        let stage = game.malnutrition(w);
        let hurt = stage >= 3 || !alive;
        ui.horizontal(|ui| {
            if who == 0 {
                theme::asks(ui, "Health", HEALTH_TIP);
            } else {
                ui.label("Health");
            }
            let total = health::MAX_HEALTH + armour;
            theme::two_tone_bar(
                ui,
                110.0,
                points / total,
                armour / total,
                if hurt { theme::BAD } else { theme::ACCENT },
                theme::ARMOUR,
            );
            ui.label(
                egui::RichText::new(if armour > 0.0 {
                    format!("{} hp + {} hp", points.round(), armour.round())
                } else {
                    format!("{}", points.round())
                })
                .small()
                .color(theme::MUTED),
            );
        });
        // What the bar is made of: the head, the body and the legs, a thin
        // bar each, and the blood under them. Drawn for the dead too — a
        // body with its head at nothing says how it died.
        egui::Grid::new(("body", who))
            .num_columns(3)
            .spacing([8.0, 1.0])
            .show(ui, |ui| {
                for (i, part) in health::Part::ALL.into_iter().enumerate() {
                    let left = game.part_health(w, part);
                    let bonus = if alive { game.part_bonus(w, part) } else { 0.0 };
                    let bleeding = alive && game.wounds(w, part) > 0;
                    ui.label(
                        egui::RichText::new(SLOT_NAMES[i])
                            .small()
                            .color(if bleeding {
                                theme::CAUTION
                            } else {
                                theme::MUTED
                            }),
                    );
                    let total = part.max() + bonus;
                    theme::thin_two_tone_bar(
                        ui,
                        110.0,
                        left / total,
                        bonus / total,
                        if bleeding { theme::BAD } else { theme::ACCENT },
                        theme::ARMOUR,
                    );
                    ui.label(
                        egui::RichText::new(if bonus > 0.0 {
                            format!("{} + {}", left.round(), bonus.round())
                        } else {
                            format!("{}", left.round())
                        })
                        .small()
                        .color(theme::MUTED),
                    );
                    ui.end_row();
                }
                // Red when there is less than half left, which is when the
                // Bim starts to slow — the number alone does not say that.
                let blood = game.blood(w) / health::MAX_BLOOD;
                let low = blood < health::SLOWED_AT;
                ui.label(egui::RichText::new("Blood").small().color(if low {
                    theme::BAD
                } else {
                    theme::MUTED
                }));
                theme::thin_bar(
                    ui,
                    110.0,
                    blood,
                    if low { theme::BAD } else { theme::ACCENT },
                );
                ui.label(
                    egui::RichText::new(format!("{}%", (blood * 100.0).round()))
                        .small()
                        .color(theme::MUTED),
                );
                ui.end_row();
            });
        let line = |ui: &mut egui::Ui, text: String, warn: bool, gone: bool| {
            if text.is_empty() {
                return;
            }
            let color = if gone {
                theme::MUTED
            } else if warn {
                theme::CAUTION
            } else {
                theme::INK
            };
            ui.label(egui::RichText::new(text).small().color(color));
        };
        // The wounds first: they are the thing that is killing it fastest.
        let open = if alive { game.bleeding(w) } else { 0 };
        line(
            ui,
            match open {
                0 => String::new(),
                1 => "Bleeding · 1 open wound".into(),
                n => format!("Bleeding · {n} open wounds"),
            },
            true,
            false,
        );
        if alive && game.is_unconscious(w) {
            ui.label(egui::RichText::new("Out cold").small().color(theme::BAD));
        }
        // A part at nothing: the dying state on it, in red, with what it
        // is doing under it — the line is what sends a player to the
        // medkits. Then what treated traumas have left behind, and for
        // how long.
        if alive {
            for part in health::Part::ALL {
                if let Some(trauma) = game.trauma(w, part) {
                    ui.label(
                        egui::RichText::new(format!(
                            "Dying · {}",
                            trauma_name(trauma.code()).to_lowercase()
                        ))
                        .small()
                        .color(theme::BAD),
                    );
                    ui.label(
                        egui::RichText::new(format!(
                            "{} Needs a medkit from a crewmate.",
                            trauma_line(trauma.code())
                        ))
                        .small()
                        .color(theme::MUTED),
                    );
                }
            }
            for l in game.lasting(w) {
                line(
                    ui,
                    format!(
                        "{} · {} · {} left",
                        trauma_name(l.trauma.code()),
                        trauma_lasting(l.trauma.code()),
                        span_text(l.left)
                    ),
                    true,
                    false,
                );
            }
        }
        let legs = if alive { game.legs_lost(w) } else { 0 };
        line(
            ui,
            match legs {
                0 => String::new(),
                1 => "One leg lost".into(),
                _ => "No legs".into(),
            },
            true,
            false,
        );
        line(
            ui,
            if alive {
                CONDITIONS[stage as usize].to_string()
            } else {
                format!("{} has died.", name(who))
            },
            stage > 0,
            !alive,
        );
        let tired = game.drowsiness(w);
        line(
            ui,
            if alive {
                DROWSINESS[tired as usize].into()
            } else {
                String::new()
            },
            tired > 0,
            false,
        );
        // Both of these are stages reached by a clock rather than levels,
        // so the bars above cannot show them.
        let urge = if alive { game.urge(w) } else { 0 };
        line(ui, URGES[urge as usize].into(), urge >= 2, false);
        let mess = if alive { game.discomfort(w) } else { 0 };
        line(ui, DISCOMFORTS[mess as usize].into(), mess >= 2, false);
        let ill = if alive { game.poisoning(w) } else { 0.0 };
        line(
            ui,
            if ill > 0.0 {
                format!("Food poisoning · {} hours to go", ill.ceil())
            } else {
                String::new()
            },
            ill > 0.0,
            false,
        );
        let alone = if alive { game.loneliness(w) } else { 0 };
        let days = game.days_alone(w).floor();
        line(
            ui,
            if alone > 0 {
                format!("{} · {days} days", LONELINESS[alone as usize])
            } else {
                String::new()
            },
            alone >= 2,
            false,
        );

        // The character sheet: two pages under the bars.
        ui.add_space(6.0);
        let diary = self.diary_open.get(w).copied().unwrap_or(false);
        ui.horizontal(|ui| {
            if theme::toggle(ui, !diary, "About").clicked() {
                self.diary_open[w] = false;
            }
            if theme::toggle(ui, diary, "Memory").clicked() {
                self.diary_open[w] = true;
            }
        });
        if !diary {
            egui::Grid::new(("about", who))
                .num_columns(2)
                .show(ui, |ui| {
                    ui.label(egui::RichText::new("Name").color(theme::MUTED));
                    ui.label(name(who));
                    ui.end_row();
                    ui.label(egui::RichText::new("Age").color(theme::MUTED));
                    ui.label(format!("{}", game.age(w)));
                    ui.end_row();
                    ui.label(egui::RichText::new("Born").color(theme::MUTED));
                    ui.label(date_text(
                        game.born_date(w),
                        game.born_month(w),
                        game.born_year(w),
                    ));
                    ui.end_row();
                });
        } else {
            self.diary(ui, game, w);
        }
        true
    }

    /// The Bim's own account of its days: newest day first, and within a
    /// day in the order it happened. An empty page is the *good* outcome:
    /// the diary keeps only what went wrong.
    fn diary(&self, ui: &mut egui::Ui, game: &Game, who: usize) {
        let count = game.memory_len(who);
        egui::ScrollArea::vertical()
            .max_height(220.0)
            .show(ui, |ui| {
                if count == 0 {
                    ui.label(egui::RichText::new("Nothing has gone wrong.").color(theme::MUTED));
                    return;
                }
                let mut days: Vec<(u32, Vec<(f32, String)>)> = Vec::new();
                for i in 0..count {
                    let day = game.memory_day(who, i);
                    let Some(words) =
                        memory_line(game.memory_what(who, i), game.memory_detail(who, i))
                    else {
                        continue;
                    };
                    let at = game.memory_at(who, i);
                    match days.iter_mut().find(|(d, _)| *d == day) {
                        Some((_, lines)) => lines.push((at, words)),
                        None => days.push((day, vec![(at, words)])),
                    }
                }
                days.sort_by_key(|a| std::cmp::Reverse(a.0));
                for (day, lines) in days {
                    ui.label(
                        egui::RichText::new(format!("Day {day}"))
                            .strong()
                            .color(theme::MUTED),
                    );
                    for (at, words) in lines {
                        ui.horizontal_wrapped(|ui| {
                            ui.label(
                                egui::RichText::new(clock_text(at))
                                    .small()
                                    .color(theme::MUTED),
                            );
                            ui.label(words);
                        });
                    }
                }
            });
    }

    // --- the agenda -----------------------------------------------------------

    /// One list per crew member with something on: the chain running, then
    /// the ones waiting behind it. `true` when anything was drawn.
    pub fn agendas(
        &mut self,
        ui: &mut egui::Ui,
        game: &Game,
        name: &dyn Fn(u32) -> String,
    ) -> bool {
        let mut drawn = false;
        for who in 0..self.crew_count {
            let w = who as usize;
            let count = game.agenda_len(w);
            if count == 0 {
                continue;
            }
            drawn = true;
            self.who_header(ui, who, name);
            for i in 0..count {
                let active = game.agenda_active(w, i) != 0;
                let done = game.agenda_progress(w, i);
                ui.horizontal(|ui| {
                    theme::bar(
                        ui,
                        70.0,
                        done,
                        if active { theme::ACCENT } else { theme::MUTED },
                    );
                    ui.label(
                        egui::RichText::new(job_name(game.agenda_job(w, i))).color(if active {
                            theme::INK
                        } else {
                            theme::MUTED
                        }),
                    );
                    ui.label(
                        egui::RichText::new(format!("{}%", (done * 100.0).round()))
                            .small()
                            .color(theme::MUTED),
                    );
                });
            }
        }
        drawn
    }

    // --- the tray ---------------------------------------------------------------

    /// Bottom-left, three tabs — seven on the ship, where there is a view
    /// to pick, actions to take outside, parts to build and a helm — and it
    /// folds away. Docked, a Station button sits with the tabs: not a tab
    /// but a press, which the screen answers with the trade window. True
    /// when the Ship tab is open, whose body the caller draws under the
    /// tabs itself.
    pub fn tray(
        &mut self,
        ui: &mut egui::Ui,
        game: &mut Game,
        mut actions: Option<&mut Actions>,
        name: &dyn Fn(u32) -> String,
    ) -> bool {
        let mut tabs = vec![
            (Tab::Schedule, "Schedule"),
            (Tab::Work, "Work"),
            (Tab::Management, "Management"),
            (Tab::Inventory, "Inventory"),
        ];
        if actions.is_some() {
            tabs.push((Tab::View, "View"));
            tabs.push((Tab::Actions, "Actions"));
            tabs.push((Tab::Build, "Build"));
            tabs.push((Tab::Research, "Research"));
            tabs.push((Tab::Ship, "Ship"));
        } else if matches!(
            self.tab,
            Tab::Actions | Tab::View | Tab::Build | Tab::Research | Tab::Ship
        ) {
            self.tab = Tab::Schedule;
        }
        let docked = actions.as_ref().is_some_and(|a| a.docked);
        let mut station = false;
        ui.horizontal(|ui| {
            for (tab, label) in tabs {
                if theme::toggle(ui, self.tab == tab && self.tray_open, label).clicked() {
                    self.tab = tab;
                    self.tray_open = true;
                }
            }
            if docked
                && ui
                    .button("Station")
                    .on_hover_text("Trade with the station")
                    .clicked()
            {
                station = true;
            }
            let fold = if self.tray_open { "Hide" } else { "Show" };
            if ui
                .button(fold)
                .on_hover_text(if self.tray_open {
                    "Hide the panel"
                } else {
                    "Show the panel"
                })
                .clicked()
            {
                self.tray_open = !self.tray_open;
            }
        });
        if let Some(actions) = actions.as_deref_mut() {
            actions.station = station;
        }
        if !self.tray_open {
            return false;
        }
        ui.separator();
        match self.tab {
            Tab::Ship => return true,
            Tab::Schedule => self.schedule(ui, game),
            Tab::Work => self.work(ui, game),
            Tab::Management => self.management(ui, game, actions),
            Tab::Inventory => {
                let who = self.inventory_who(game);
                self.inventory(ui, game, who, name);
            }
            Tab::Actions => {
                if let Some(actions) = actions {
                    self.actions(ui, actions);
                }
            }
            Tab::View => {
                if let Some(actions) = actions {
                    self.view(ui, actions);
                }
            }
            Tab::Build => {
                if let Some(actions) = actions {
                    self.build(ui, actions);
                }
            }
            Tab::Research => {
                if let Some(actions) = actions {
                    self.research(ui, actions);
                }
            }
        }
        false
    }

    // --- the inventory --------------------------------------------------------

    /// Whose inventory the tab, the pop-up and the container windows are
    /// about: the selected crew member, or the one the player steers when
    /// nobody is picked.
    pub fn inventory_who(&self, game: &Game) -> usize {
        (0..self.crew_count)
            .map(|w| w as usize)
            .find(|&w| game.is_selected(w))
            .unwrap_or(self.player)
    }

    /// What a Bim has on it: head, body and leg protection down the left,
    /// top to bottom, each slot with the piece's icon, its health and its
    /// numbers; the weapon beside them with its numbers, and under the
    /// weapon the pack on its back, three by three. Right-click a worn
    /// slot to take the piece off, a pack cell for what can be done with
    /// the thing in it; ctrl-click a pack cell to put the thing straight
    /// into a container within reach. Beside each armour slot, the wounds
    /// open on that part of the body and a Bandage button that sends the
    /// player's Bim to dress them — and, for a part at nothing, the dying
    /// state on it and a Treat button that sends a crewmate with a medkit;
    /// under the lot, how many bandages there are to do it with.
    fn inventory(
        &mut self,
        ui: &mut egui::Ui,
        game: &mut Game,
        who: usize,
        name: &dyn Fn(u32) -> String,
    ) {
        let gear = game.gear(who);
        let alive = game.is_alive(who);
        let bandages = game.bandages();
        // The hands are the player's whoever is shown, so the button is
        // greyed for the same reasons the menu on a body is: the player's
        // Bim dead, out cold or outside, or the patient outside. The room
        // refuses the order silently otherwise, and the tab — unlike the
        // pop-up — stays open past the player's death.
        let helper_out = !game.is_alive(self.player)
            || game.is_unconscious(self.player)
            || game.is_outside(self.player);
        let patient_out = alive && game.is_outside(who);
        let mut dress: Option<health::Part> = None;
        let mut treat: Option<(usize, health::Part)> = None;
        let medkits = game.medkits();
        // A blade within reach is the state that changes what the gun in
        // the slot is worth, so it is said in the header rather than
        // beside the numbers: the numbers do not apply while it lasts.
        let locked = alive && game.is_locked(who).is_some();
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new(name(who as u32)).strong());
            ui.label(
                egui::RichText::new(if locked {
                    format!("combat mode — {LOCKED_STATUS}")
                } else if game.is_armed(who) {
                    "combat mode — weapon drawn".into()
                } else if game.is_recruited() && who == self.player {
                    "combat mode".into()
                } else {
                    "weapon holstered".into()
                })
                .small()
                .color(if locked {
                    theme::CAUTION
                } else if game.is_armed(who) {
                    theme::ACCENT
                } else {
                    theme::MUTED
                }),
            );
            if locked {
                theme::question_mark(ui, &locked_tip());
            }
            theme::question_mark(ui, INVENTORY_TIP);
        });
        ui.add_space(4.0);
        ui.horizontal_top(|ui| {
            // The three armour slots, one above the other, each with the
            // bleeding on that part beside it. The button is live only
            // when there is something to dress and something to dress it
            // with; the hint says which is missing. A dead Bim is past it.
            ui.vertical(|ui| {
                for (i, part) in health::Part::ALL.into_iter().enumerate() {
                    let worn = gear.worn(part);
                    let wounds = if alive { game.wounds(who, part) } else { 0 };
                    ui.horizontal(|ui| {
                        let line = worn.map(worn_line).unwrap_or_default();
                        let response = slot(
                            ui,
                            SLOT_NAMES[i],
                            worn.map(PackItem::Armour),
                            armour_name(worn.map(|p| p.kind)),
                            &line,
                        );
                        if let Some(piece) = worn {
                            let response =
                                response.on_hover_text(tip_of(PackItem::Armour(piece), 1));
                            if response.secondary_clicked()
                                && let Some(at) = response.interact_pointer_pos()
                            {
                                self.cell_menu = Some(CellMenu {
                                    at,
                                    from: Source::Worn(part),
                                    who,
                                    fresh: true,
                                });
                            }
                        }
                        ui.vertical(|ui| {
                            let (count, hint) = bandage_words(wounds, bandages);
                            ui.label(egui::RichText::new(count).small().color(if wounds > 0 {
                                theme::CAUTION
                            } else {
                                theme::MUTED
                            }));
                            let can = wounds > 0 && bandages > 0 && !helper_out && !patient_out;
                            let hint = if wounds > 0 && bandages > 0 && helper_out {
                                HELPER_OUT
                            } else if wounds > 0 && bandages > 0 && patient_out {
                                PATIENT_OUT
                            } else {
                                hint
                            };
                            if ui
                                .add_enabled(can, egui::Button::new("Bandage"))
                                .on_hover_text(hint)
                                .on_disabled_hover_text(hint)
                                .clicked()
                            {
                                dress = Some(part);
                            }
                            // A part at nothing: the dying state on it,
                            // and a Treat button that sends the helper
                            // `treat_helper` picks with a medkit.
                            if let Some(trauma) = alive.then(|| game.trauma(who, part)).flatten() {
                                ui.label(
                                    egui::RichText::new(format!(
                                        "Dying · {}",
                                        trauma_name(trauma.code()).to_lowercase()
                                    ))
                                    .small()
                                    .color(theme::BAD),
                                );
                                let helper = treat_helper(game, self.player, who);
                                let can = helper.is_some() && medkits > 0 && !patient_out;
                                let hint = if medkits == 0 {
                                    "no medkits — the armoury makes them".to_string()
                                } else if patient_out {
                                    PATIENT_OUT.to_string()
                                } else if let Some(h) = helper {
                                    if h == self.player {
                                        trauma_line(trauma.code()).to_string()
                                    } else {
                                        format!(
                                            "{} walks over — nobody treats their own",
                                            name(h as u32)
                                        )
                                    }
                                } else if who == self.player {
                                    "nobody free to do it — nobody treats their own".to_string()
                                } else {
                                    HELPER_OUT.to_string()
                                };
                                if ui
                                    .add_enabled(can, egui::Button::new("Treat"))
                                    .on_hover_text(&hint)
                                    .on_disabled_hover_text(&hint)
                                    .clicked()
                                    && let Some(h) = helper
                                {
                                    treat = Some((h, part));
                                }
                            }
                        });
                    });
                }
            });
            ui.add_space(8.0);
            // The weapon slot, and the pack under it.
            ui.vertical(|ui| {
                // The tier under the name, when it is above one.
                let weapon_line = gear
                    .weapon
                    .and_then(|w| tier_word(w.tier))
                    .unwrap_or_default();
                slot(
                    ui,
                    SLOT_NAMES[3],
                    gear.weapon.map(PackItem::Weapon),
                    weapon_name(gear.weapon.map(|w| w.kind)),
                    &weapon_line,
                );
                ui.label(egui::RichText::new("Pack").small().color(theme::MUTED));
                // Seven by seven, laid out like the lockers: a drag moves a
                // thing, Turn turns it, through the seam as a repack.
                let (things, heads) = pack_things(&gear);
                let mut drag = self.pack_drag;
                let fits = |i: usize, x: usize, y: usize, turned: bool| -> bool {
                    heads.get(i).is_some_and(|&head| {
                        gear.pack[head].is_some_and(|item| {
                            gear.fits_turned(y * PACK_COLS + x, item, turned, Some(head))
                        })
                    })
                };
                let moved = grid::lockers(
                    ui,
                    PACK_COLS,
                    PACK_ROWS,
                    0,
                    PACK_CELL,
                    &things,
                    &mut drag,
                    &fits,
                    self.keys.key(Action::Turn),
                    true,
                );
                self.pack_drag = drag;
                self.pack_moved(who, &gear, &heads, moved);
            });
            ui.add_space(8.0);
            if let Some(stats) = game.weapon_stats(who) {
                // The numbers, off the two-point curves: a gun's odds and
                // damage each as "its best to here, this much at the
                // range"; a burst weapon's fire rate as the burst and the
                // recharge; a blade in one line, since its range is an
                // arm's length and nothing flies.
                egui::Grid::new(("weapon-stats", who))
                    .num_columns(2)
                    .spacing([10.0, 2.0])
                    .show(ui, |ui| {
                        let row = |ui: &mut egui::Ui, label: &str, value: String| {
                            ui.label(egui::RichText::new(label).small().color(theme::MUTED));
                            ui.label(value);
                            ui.end_row();
                        };
                        let asks = |ui: &mut egui::Ui, label: &str, tip: &str, value: String| {
                            theme::asks(ui, label, tip);
                            ui.label(value);
                            ui.end_row();
                        };
                        if stats.melee {
                            row(ui, "Weapon", melee_text(&stats));
                            row(ui, "Reach", format!("{} tiles", tidy(stats.range)));
                        } else {
                            row(ui, "Range", format!("{} tiles", tidy(stats.range)));
                            asks(ui, "Accuracy", ACCURACY_TIP, accuracy_text(&stats));
                            asks(ui, "Damage", DAMAGE_TIP, damage_text(&stats));
                            row(ui, "Shot speed", format!("{} tiles/s", stats.speed));
                            asks(
                                ui,
                                if stats.burst > 1 {
                                    "Burst"
                                } else {
                                    "Fire rate"
                                },
                                FIRE_RATE_TIP,
                                fire_rate_text(&stats),
                            );
                        }
                        theme::asks(ui, "DPS", DPS_TIP);
                        ui.label(egui::RichText::new(format!("{}", stats.dps())).strong());
                        ui.end_row();
                    });
            } else {
                ui.label(egui::RichText::new("Nothing in hand.").color(theme::MUTED));
            }
        });
        ui.horizontal(|ui| {
            ui.label(
                egui::RichText::new(format!("Bandages: {bandages}"))
                    .small()
                    .color(if bandages > 0 {
                        theme::INK
                    } else {
                        theme::MUTED
                    }),
            );
            theme::question_mark(ui, BANDAGE_TIP);
        });
        // The order goes through the player's Bim, whoever is shown: the
        // other crew take no orders, and one of them is dressed by being
        // walked over to.
        if let Some(part) = dress {
            game.bandage(self.player, who, part);
        }
        if let Some((helper, part)) = treat {
            game.treat(helper, who, part);
        }
    }

    /// What the pointer did to the pack: a thing dropped or turned is a
    /// repack through the seam; a right-click on a thing is its pop-up; a
    /// ctrl-click is the quick move into a container within reach — and
    /// one that cannot go opens the pop-up instead, whose Store row says
    /// why. `heads` is the cell each thing in the grid is kept in.
    fn pack_moved(
        &mut self,
        who: usize,
        gear: &bims::combat::Gear,
        heads: &[usize],
        moved: grid::Moved,
    ) {
        let pack = &gear.pack;
        if let Some((i, x, y, turned)) = moved.dropped
            && let Some(&cell) = heads.get(i)
        {
            self.orders.push(GearOrder::Repack {
                who: who as u32,
                cell: cell as u32,
                to: (y * PACK_COLS + x) as u32,
                turned,
            });
        }
        if let Some(i) = moved.turn
            && let Some(&cell) = heads.get(i)
            && let Some(item) = pack[cell]
            && gear.fits_turned(cell, item, !gear.turned[cell], Some(cell))
        {
            self.orders.push(GearOrder::Repack {
                who: who as u32,
                cell: cell as u32,
                to: cell as u32,
                turned: !gear.turned[cell],
            });
        }
        if let Some((i, at)) = moved.right_clicked
            && let Some(&cell) = heads.get(i)
            && pack[cell].is_some()
        {
            self.cell_menu = Some(CellMenu {
                at,
                from: Source::Pack(cell),
                who,
                fresh: true,
            });
        }
        if let Some((i, at)) = moved.ctrl_clicked
            && let Some(&cell) = heads.get(i)
            && let Some(item) = pack[cell]
        {
            let i = cell;
            if self.can_stow(item).is_ok() {
                self.orders.push(GearOrder::Stow {
                    who: who as u32,
                    cell: i as u32,
                });
            } else {
                self.cell_menu = Some(CellMenu {
                    at,
                    from: Source::Pack(i),
                    who,
                    fresh: true,
                });
            }
        }
    }

    /// Whether a thing in the pack can go into a container now, or why
    /// not, in the words the Store row shows. The world checks the same
    /// things again when the command lands; this is so the row can say so
    /// first.
    fn can_stow(&self, item: PackItem) -> Result<(), String> {
        let Some(hold) = &self.hold else {
            return Err("there is no hold to put it in".into());
        };
        if let PackItem::Armour(piece) = item
            && piece.broken()
        {
            return Err("broken — worth nothing put away; discard it".into());
        }
        let Some(resource) = world::armour::resource_of_item(item) else {
            return Err("nothing aboard takes it".into());
        };
        if !hold.reach[resource as usize] {
            return Err(REACH_HINT.into());
        }
        let class = economy::storage(resource);
        if hold.used[class as usize] >= hold.capacity[class as usize] {
            return Err(format!(
                "no room left in the {}",
                STORAGE_NAMES[class as usize].to_lowercase()
            ));
        }
        Ok(())
    }

    /// Whether a thing in a container can come into `who`'s pack now, or
    /// why not: reach, and a free cell.
    fn can_fetch(&self, game: &Game, who: usize, resource: ResourceId) -> Result<(), String> {
        let Some(hold) = &self.hold else {
            return Err("there is no hold to take it from".into());
        };
        if !hold.reach[resource as usize] {
            return Err(REACH_HINT.into());
        }
        if game.gear(who).free_cell().is_none() {
            return Err("the pack is full".into());
        }
        Ok(())
    }

    /// Open the inventory pop-up, or shut it — the Inventory key (Tab).
    /// Opening it opens the nearest thing within reach as well — a
    /// container, or a body down — the way a survival game shows what is
    /// to hand, with the rest of what is near a click away on the strip
    /// over the window (`nearby`); shutting it shuts that window too.
    pub fn toggle_inventory(&mut self) {
        self.cell_menu = None;
        if self.inventory_open {
            self.inventory_open = false;
            self.open = None;
            return;
        }
        self.inventory_open = true;
        if self.open.is_none()
            && let Some(near) = self.nearby.first()
        {
            self.show(near.open);
        }
    }

    /// Put up the window for a thing already within reach — off the
    /// nearby strip, or the Inventory key — without the walk over.
    fn show(&mut self, open: Open) {
        self.open = Some(open);
        self.body = None;
        self.inventory_open = true;
        self.menu = None;
        self.cell_menu = None;
    }

    /// Whatever the nearby strip was clicked on this frame, put up.
    fn follow_strip(&mut self, pick: Option<Open>) {
        if let Some(open) = pick {
            self.show(open);
        }
    }

    /// The pop-up that opens the moment the crew member the player steers
    /// is recruited, or a container window opens, or the Inventory key is
    /// pressed: the inventory of the Bim shown, in a window of its own,
    /// until it is shut or the Bim is let go. Beside the container window while one is up, else at the
    /// top of the screen. Call once a frame after the tray and after
    /// [`CrewPanels::container_window`].
    pub fn inventory_window(
        &mut self,
        ctx: &egui::Context,
        game: &mut Game,
        name: &dyn Fn(u32) -> String,
    ) {
        let recruited = game.is_recruited() && game.is_alive(self.player);
        if recruited && !self.was_recruited {
            self.inventory_open = true;
        }
        // Being let go shuts it — unless a container is open, in which
        // case the pack is still what the container is being used with.
        if !recruited && self.was_recruited && self.open.is_none() {
            self.inventory_open = false;
        }
        self.was_recruited = recruited;
        if !self.inventory_open {
            return;
        }
        let who = self.inventory_who(game);
        let mut open = true;
        let window = egui::Window::new("Inventory")
            .id(egui::Id::new("inventory-window"))
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .frame(crate::screens::room::panel_frame());
        let window = match self.container_rect {
            Some(rect) => window.fixed_pos(egui::pos2(rect.max.x + 10.0, rect.min.y)),
            None => window.anchor(egui::Align2::CENTER_TOP, egui::vec2(0.0, 60.0)),
        };
        let mut strip = None;
        window.show(ctx, |ui| {
            // With nothing else up, what is near is a click away from here.
            if self.open.is_none() {
                strip = nearby_strip(ui, &self.nearby.clone(), None);
            }
            self.inventory(ui, game, who, name);
        });
        self.inventory_open = open;
        self.follow_strip(strip);
    }

    // --- the containers -------------------------------------------------------

    /// The open container's window, if one is: the hold's class the
    /// container keeps. The shelves and the cold store are a grid of
    /// stacks, a resource a cell with its count. The lockers are the
    /// ship's grid itself, `GRID_COLS` across: every thing in the
    /// class laid over its footprint where the world has it, a piece of
    /// armour with its health under it, turned if it is turned — and
    /// the pointer moves them: a drag carries a thing, `R` on the way
    /// turns it, and letting go where it would lie sends
    /// `GearOrder::Arrange`; `R` over a thing at rest turns it where it
    /// is. Over the grid, how full the class is; under it, what a click
    /// does. Ctrl-click a thing to take it into the pack of the Bim
    /// shown; right-click for the row. Shut by its cross, by Escape, or
    /// by the container going away under it. Call once a frame after
    /// the tray and before [`CrewPanels::inventory_window`], which sits
    /// beside it.
    pub fn container_window(
        &mut self,
        ctx: &egui::Context,
        game: &Game,
        name: &dyn Fn(u32) -> String,
    ) {
        self.container_rect = None;
        let Some(Open::Container(container)) = self.open else {
            self.locker_drag = None;
            return;
        };
        let (Some(hold), Some(class)) = (self.hold.as_ref(), class_of(game, container)) else {
            self.open = None;
            self.locker_drag = None;
            return;
        };
        let who = self.inventory_who(game);
        let title = match container {
            Container::Bench(i) => PartKind::from_code(game.bench_part(i))
                .map(part_name)
                .unwrap_or("Container")
                .to_string(),
            Container::Shelf(_) => STORAGE_WINDOW.to_string(),
            Container::Fridge(_) => COLD_STORE_WINDOW.to_string(),
            Container::Desk(_) => RESEARCH_WINDOW.to_string(),
        };
        // A class with a grid is drawn as one; the desk, which is a count of
        // one, as a cell.
        let laid = hold.grid(class);
        let lockers = laid.is_some();
        let (cols, rows) = match laid {
            Some((_, capacity)) => (
                shipdesign::GRID_COLS as usize,
                Grid::rows(capacity) as usize,
            ),
            None => container_dims(class),
        };
        let (cells, mut what): (Vec<Option<Cell>>, Vec<HoldCell>) = if lockers {
            (Vec::new(), Vec::new())
        } else {
            container_cells(class, hold)
        };
        let things: Vec<grid::Laid> = match laid {
            Some((grid, _)) => {
                let (things, slots) = grid_things(grid, hold);
                what = slots
                    .into_iter()
                    .map(|id| HoldCell::Slot(class, id))
                    .collect();
                things
            }
            None => Vec::new(),
        };
        let near = ResourceId::ALL
            .iter()
            .any(|&id| economy::storage(id) == class && hold.reach[id as usize]);
        let used = hold.used[class as usize];
        let capacity = hold.capacity[class as usize];
        let mut open = true;
        let mut picked = grid::Picked::default();
        let mut moved = grid::Moved::default();
        let mut drag = self.locker_drag;
        let mut strip = None;
        let (nearby, showing) = (self.nearby.clone(), self.open);
        let fits = |i: usize, x: usize, y: usize, turned: bool| -> bool {
            what.get(i).is_some_and(|&cell| match cell {
                HoldCell::Slot(class, id) => hold.grid(class).is_some_and(|(grid, capacity)| {
                    grid.slot(id).is_some_and(|s| {
                        grid.fits(capacity, s.foot, x as u32, y as u32, turned, Some(id))
                    })
                }),
                HoldCell::Stack(_) => false,
            })
        };
        let response = egui::Window::new(title)
            .id(egui::Id::new("container-window"))
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::LEFT_TOP, CONTAINER_AT)
            .frame(crate::screens::room::panel_frame())
            .show(ctx, |ui| {
                strip = nearby_strip(ui, &nearby, showing);
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new(if lockers {
                            format!(
                                "{used} of {capacity} cells in the {}",
                                STORAGE_NAMES[class as usize].to_lowercase()
                            )
                        } else {
                            format!(
                                "{used} of {capacity} in the {}",
                                STORAGE_NAMES[class as usize].to_lowercase()
                            )
                        })
                        .small()
                        .color(theme::MUTED),
                    );
                    theme::question_mark(ui, CONTAINER_TIP);
                });
                if let Some((_, capacity)) = laid {
                    let blocked = rows * cols - capacity as usize;
                    moved = grid::lockers(
                        ui,
                        cols,
                        rows,
                        blocked,
                        container_cell(class),
                        &things,
                        &mut drag,
                        &fits,
                        self.keys.key(Action::Turn),
                        true,
                    );
                } else {
                    picked = grid::grid(ui, cols, rows, container_cell(class), &cells);
                }
                let hint = if !near {
                    format!(
                        "{} is not within reach — walk over first; clicking the container sends the Bim",
                        name(who as u32)
                    )
                } else if lockers {
                    format!(
                        "Drag a thing to move it, {} turns it · Ctrl-click takes it into {}'s pack · right-click for the rows",
                        self.keys.key(Action::Turn).symbol_or_name(),
                        name(who as u32)
                    )
                } else {
                    format!(
                        "Ctrl-click takes one into {}'s pack · right-click for the rows",
                        name(who as u32)
                    )
                };
                ui.add(egui::Label::new(egui::RichText::new(hint).small().color(theme::MUTED)).wrap());
            });
        self.locker_drag = drag;
        if let Some(response) = response {
            self.container_rect = Some(response.response.rect);
        }
        if !open {
            self.open = None;
            self.locker_drag = None;
        }
        // A thing moved or turned on the lockers' grid: through the seam,
        // since the grid is the world's.
        if let Some((i, x, y, turned)) = moved.dropped
            && let Some(&HoldCell::Slot(class, id)) = what.get(i)
        {
            self.orders.push(GearOrder::Arrange {
                class,
                id,
                x: x as u32,
                y: y as u32,
                turned,
            });
        }
        if let Some(i) = moved.turn
            && let Some(&HoldCell::Slot(class, id)) = what.get(i)
            && let Some(slot) = hold.grid(class).and_then(|(g, _)| g.slot(id))
            && fits(i, slot.x as usize, slot.y as usize, !slot.turned)
        {
            self.orders.push(GearOrder::Arrange {
                class,
                id,
                x: slot.x as u32,
                y: slot.y as u32,
                turned: !slot.turned,
            });
        }
        // The pointer on the grid: a right-click is the row, a ctrl-click
        // the quick take — or the row, when the take cannot go, so the
        // reason is read rather than guessed at.
        let right = picked.right_clicked.or(moved.right_clicked);
        let quick = picked.ctrl_clicked.or(moved.ctrl_clicked);
        if let Some((i, at)) = right
            && let Some(&cell) = what.get(i)
        {
            self.cell_menu = Some(CellMenu {
                at,
                from: Source::Hold(cell),
                who,
                fresh: true,
            });
        }
        if let Some((i, at)) = quick
            && let Some(&cell) = what.get(i)
        {
            let resource = match cell {
                HoldCell::Slot(class, id) => hold
                    .grid(class)
                    .and_then(|(g, _)| g.slot(id))
                    .and_then(|s| kept_resource(s.kept, hold)),
                HoldCell::Stack(id) => Some(id),
            };
            match resource.map(|r| self.can_fetch(game, who, r)) {
                Some(Ok(())) => self.orders.push(GearOrder::Fetch {
                    who: who as u32,
                    kind: fetch_kind(cell),
                }),
                _ => {
                    self.cell_menu = Some(CellMenu {
                        at,
                        from: Source::Hold(cell),
                        who,
                        fresh: true,
                    });
                }
            }
        }
        self.follow_strip(strip);
    }

    /// The Hire window, if a mercenary is open: whose, what it carries —
    /// the weapon and every piece worn, which is what the fee is — and a
    /// month's fee, with the button that sends the hire through the seam
    /// (`GearOrder::Hire`). Greyed, with the reason, while the Bim shown
    /// is out of reach (the row walked it over), the money is short, or
    /// there is no bunk aboard. Shut by its cross, by Escape, by the hire
    /// going through, or by the body no longer being for hire — the rooms
    /// parted. Call once a frame after the screen has set
    /// [`CrewPanels::terms`], in the Loot window's place.
    pub fn hire_window(&mut self, ctx: &egui::Context, game: &Game, name: &dyn Fn(u32) -> String) {
        let Some(resident) = self.hire_source() else {
            return;
        };
        let Some(terms) = self.terms else {
            self.open = None;
            return;
        };
        let who = self.inventory_who(game);
        let whose = name(self.crew_count + resident);
        let mut open = true;
        let mut hired = false;
        egui::Window::new(format!("{HIRE_WINDOW} — {whose}"))
            .id(egui::Id::new("hire-window"))
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::LEFT_TOP, CONTAINER_AT)
            .frame(crate::screens::room::panel_frame())
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new("Carries").small().color(theme::MUTED));
                    theme::question_mark(ui, HIRE_TIP);
                });
                ui.label(weapon_name(terms.gear.weapon.map(|w| w.kind)));
                for part in health::Part::ALL {
                    if let Some(piece) = terms.gear.worn(part) {
                        ui.label(armour_name(Some(piece.kind)));
                    }
                }
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new("A month").small().color(theme::MUTED));
                    ui.label(egui::RichText::new(crate::format::euros(terms.fee)).strong());
                });
                let hint = if !terms.in_reach {
                    Some(format!("{} — {REACH_HINT}", name(who as u32)))
                } else if !terms.bunk {
                    Some(NO_BUNK_HINT.to_string())
                } else if !terms.affordable {
                    Some(BROKE_HINT.to_string())
                } else {
                    None
                };
                let can = hint.is_none();
                if ui
                    .add_enabled(can, egui::Button::new(HIRE_BUTTON))
                    .clicked()
                {
                    hired = true;
                }
                if let Some(hint) = hint {
                    ui.add(
                        egui::Label::new(egui::RichText::new(hint).small().color(theme::MUTED))
                            .wrap(),
                    );
                }
            });
        if hired {
            self.orders.push(GearOrder::Hire {
                who: who as u32,
                resident,
            });
            open = false;
        }
        if !open || ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            self.open = None;
            self.terms = None;
        }
    }

    /// The Loot window, if a body is open: the body's pack three by three,
    /// and under it a row of four — the head, the body, the legs and the
    /// weapon in hand, labelled — every cell drawn the way the containers'
    /// are, a piece with its health under it. Ctrl-click a cell to take
    /// it into the pack of the Bim shown; right-click for the row. Shut
    /// by its cross, by Escape, by the Bim getting up (a crewmate that
    /// came round is no longer a body), or by the body going away under
    /// it — the rooms parting. Call once a frame after the tray, after
    /// [`CrewPanels::container_window`] and after the screen has set
    /// [`CrewPanels::body`], and before [`CrewPanels::inventory_window`],
    /// which sits beside it.
    pub fn loot_window(&mut self, ctx: &egui::Context, game: &Game, name: &dyn Fn(u32) -> String) {
        let Some(source) = self.loot_source() else {
            return;
        };
        let Some(body) = self.body.as_ref() else {
            self.open = None;
            return;
        };
        if !body.down {
            self.open = None;
            return;
        }
        let who = self.inventory_who(game);
        let whose = match source {
            LootSource::Crew(body) => name(body),
            LootSource::Resident(body) => name(self.crew_count + body),
        };
        let cells: Vec<Option<Cell>> = body.cells[PACK_CELLS..]
            .iter()
            .map(|slot| slot.map(|item| cell_of(item, 1)))
            .collect();
        let worn_cells = cells.as_slice();
        let (things, heads) = laid_things(&body.cells[..PACK_CELLS], &body.turned);
        let reach = body.reach;
        let mut open = true;
        let mut pack = grid::Moved::default();
        let mut worn = grid::Picked::default();
        let mut no_drag = None;
        let mut strip = None;
        let (nearby, showing) = (self.nearby.clone(), self.open);
        let never = |_: usize, _: usize, _: usize, _: bool| false;
        let response = egui::Window::new(format!("{LOOT_WINDOW} — {whose}"))
            .id(egui::Id::new("loot-window"))
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::LEFT_TOP, CONTAINER_AT)
            .frame(crate::screens::room::panel_frame())
            .show(ctx, |ui| {
                strip = nearby_strip(ui, &nearby, showing);
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new("Pack").small().color(theme::MUTED));
                    theme::question_mark(ui, LOOT_TIP);
                });
                pack = grid::lockers(
                    ui,
                    PACK_COLS,
                    PACK_ROWS,
                    0,
                    PACK_CELL,
                    &things,
                    &mut no_drag,
                    &never,
                    self.keys.key(Action::Turn),
                    false,
                );
                ui.add_space(6.0);
                ui.label(
                    egui::RichText::new("Worn, and in hand")
                        .small()
                        .color(theme::MUTED),
                );
                worn = grid::grid(ui, SLOT_NAMES.len(), 1, PACK_CELL, worn_cells);
                // A label under each of the four, on the grid's own pitch,
                // so the word sits under its cell.
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = grid::GAP;
                    for label in SLOT_NAMES {
                        ui.add_sized(
                            [PACK_CELL, 14.0],
                            egui::Label::new(
                                egui::RichText::new(label).small().color(theme::MUTED),
                            ),
                        );
                    }
                });
                let hint = if reach {
                    format!(
                        "Ctrl-click takes it into {}'s pack · right-click for the row",
                        name(who as u32)
                    )
                } else {
                    format!(
                        "{} is not within reach — walk over first; the Loot row sends the Bim",
                        name(who as u32)
                    )
                };
                ui.add(
                    egui::Label::new(egui::RichText::new(hint).small().color(theme::MUTED)).wrap(),
                );
            });
        if let Some(response) = response {
            self.container_rect = Some(response.response.rect);
        }
        if !open {
            self.open = None;
        }
        self.follow_strip(strip);
        // The two grids as one list of cells, in `LootCell` order: a thing
        // in the pack is the cell it is kept in, the row's is its code less
        // the pack's cells.
        let right_clicked = pack
            .right_clicked
            .and_then(|(i, at)| heads.get(i).map(|&c| (c, at)))
            .or(worn.right_clicked.map(|(i, at)| (i + PACK_CELLS, at)));
        let ctrl_clicked = pack
            .ctrl_clicked
            .and_then(|(i, at)| heads.get(i).map(|&c| (c, at)))
            .or(worn.ctrl_clicked.map(|(i, at)| (i + PACK_CELLS, at)));
        // The pointer on a cell: a right-click is the row, a ctrl-click
        // the quick take — or the row, when the take cannot go, so the
        // reason is read rather than guessed at.
        if let Some((i, at)) = right_clicked {
            self.cell_menu = Some(CellMenu {
                at,
                from: Source::Loot(i as u32),
                who,
                fresh: true,
            });
        }
        if let Some((i, at)) = ctrl_clicked {
            if self.can_loot(game, who).is_ok() {
                self.orders.push(GearOrder::Loot {
                    who: who as u32,
                    source,
                    cell: i as u32,
                });
            } else {
                self.cell_menu = Some(CellMenu {
                    at,
                    from: Source::Loot(i as u32),
                    who,
                    fresh: true,
                });
            }
        }
    }

    /// Whether a thing on the open body can come into `who`'s pack now, or
    /// why not: the body still down, reach, and a free cell. The world
    /// checks the same things again when the command lands; this is so
    /// the row can say so first.
    fn can_loot(&self, game: &Game, who: usize) -> Result<(), String> {
        let Some(body) = &self.body else {
            return Err("the body is gone".into());
        };
        if !body.down {
            return Err("not down any more".into());
        }
        if !body.reach {
            return Err(REACH_HINT.into());
        }
        if game.gear(who).free_cell().is_none() {
            return Err("the pack is full".into());
        }
        Ok(())
    }

    /// The rows of a cell's pop-up, each with the order it sends. Empty
    /// when the cell is empty now — the thing moved while the pop-up was
    /// up — which shuts it.
    fn cell_rows(
        &self,
        game: &Game,
        menu: &CellMenu,
        name: &dyn Fn(u32) -> String,
    ) -> Vec<(theme::Row, GearOrder)> {
        let who = menu.who;
        let alive = game.is_alive(who);
        let pack_full = game.gear(who).free_cell().is_none();
        let mut rows = Vec::new();
        match menu.from {
            Source::Pack(cell) => {
                let Some(item) = game.pack(who).get(cell).copied().flatten() else {
                    return rows;
                };
                let (who32, cell32) = (who as u32, cell as u32);
                match item {
                    PackItem::Armour(piece) => {
                        let slot = SLOT_NAMES[piece.kind.slot() as usize].to_lowercase();
                        rows.push((
                            theme::Row::new(
                                "Equip",
                                if !alive {
                                    "not any more".to_string()
                                } else if piece.broken() {
                                    "broken — it goes on, and does nothing".to_string()
                                } else {
                                    format!("on the {slot}; whatever is worn there comes off into this cell")
                                },
                                !alive,
                            ),
                            GearOrder::Equip {
                                who: who32,
                                cell: cell32,
                            },
                        ));
                    }
                    PackItem::Weapon(_) => {
                        rows.push((
                            theme::Row::new(
                                "Equip",
                                if alive {
                                    "swaps with the one in hand"
                                } else {
                                    "not any more"
                                },
                                !alive,
                            ),
                            GearOrder::Equip {
                                who: who32,
                                cell: cell32,
                            },
                        ));
                    }
                    PackItem::Stack(_) | PackItem::Key(_) => {}
                }
                // Only where there is a hold: the room has nowhere to
                // put a thing away.
                if self.hold.is_some() {
                    let (hint, disabled) = match self.can_stow(item) {
                        Ok(()) => {
                            let class = world::armour::resource_of_item(item)
                                .map(economy::storage)
                                .unwrap_or(Storage::Locker);
                            (
                                format!(
                                    "into the {}",
                                    STORAGE_NAMES[class as usize].to_lowercase()
                                ),
                                false,
                            )
                        }
                        Err(why) => (why, true),
                    };
                    rows.push((
                        theme::Row::new("Store", hint, disabled),
                        GearOrder::Stow {
                            who: who32,
                            cell: cell32,
                        },
                    ));
                }
                rows.push((
                    theme::Row::new("Discard", "thrown out, for good", false),
                    GearOrder::Discard {
                        who: who32,
                        cell: cell32,
                    },
                ));
            }
            Source::Hold(cell) => {
                let Some(hold) = &self.hold else {
                    return rows;
                };
                let (label, resource) = match cell {
                    HoldCell::Slot(class, id) => {
                        let Some(slot) = hold.grid(class).and_then(|(g, _)| g.slot(id)) else {
                            return rows;
                        };
                        let Some(resource) = kept_resource(slot.kept, hold) else {
                            return rows;
                        };
                        (if slot.count > 1 { "Take one" } else { "Take" }, resource)
                    }
                    HoldCell::Stack(id) => {
                        if hold.counts[id as usize] == 0 {
                            return rows;
                        }
                        ("Take one", id)
                    }
                };
                let (hint, disabled) = match self.can_fetch(game, who, resource) {
                    Ok(()) => (format!("into {}'s pack", name(who as u32)), false),
                    Err(why) => (why, true),
                };
                rows.push((
                    theme::Row::new(label, hint, disabled),
                    GearOrder::Fetch {
                        who: who as u32,
                        kind: fetch_kind(cell),
                    },
                ));
                // A thing on the lockers' grid turns where it lies, if it
                // can: the same as R over it.
                if let HoldCell::Slot(class, id) = cell
                    && let Some((grid, capacity)) = hold.grid(class)
                    && let Some(slot) = grid.slot(id)
                    && slot.foot.rows != slot.foot.cols
                {
                    let room = grid.fits(
                        capacity,
                        slot.foot,
                        slot.x as u32,
                        slot.y as u32,
                        !slot.turned,
                        Some(id),
                    );
                    rows.push((
                        theme::Row::new(
                            "Turn",
                            if room {
                                format!(
                                    "a quarter round, where it lies — or drag it and press {}",
                                    self.keys.key(Action::Turn).symbol_or_name()
                                )
                            } else {
                                "no room to turn it where it lies — drag it somewhere with more"
                                    .to_string()
                            },
                            !room,
                        ),
                        GearOrder::Arrange {
                            class,
                            id,
                            x: slot.x as u32,
                            y: slot.y as u32,
                            turned: !slot.turned,
                        },
                    ));
                }
            }
            Source::Worn(part) => {
                if game.worn(who, part).is_none() {
                    return rows;
                }
                let (hint, disabled) = if !alive {
                    ("not any more", true)
                } else if pack_full {
                    ("the pack is full", true)
                } else {
                    ("into the pack", false)
                };
                rows.push((
                    theme::Row::new("Unequip", hint, disabled),
                    GearOrder::Unequip {
                        who: who as u32,
                        part,
                    },
                ));
            }
            Source::Loot(cell) => {
                let (Some(source), Some(body)) = (self.loot_source(), self.body.as_ref()) else {
                    return rows;
                };
                if body.cells.get(cell as usize).copied().flatten().is_none() {
                    return rows;
                }
                let (hint, disabled) = match self.can_loot(game, who) {
                    Ok(()) => (format!("into {}'s pack", name(who as u32)), false),
                    Err(why) => (why, true),
                };
                rows.push((
                    theme::Row::new("Take", hint, disabled),
                    GearOrder::Loot {
                        who: who as u32,
                        source,
                        cell,
                    },
                ));
            }
        }
        rows
    }

    /// Draw the open cell pop-up, if there is one, and shut it on a click
    /// away or once its row is pressed. Call once a frame after the
    /// windows.
    pub fn cell_menu(&mut self, ctx: &egui::Context, game: &Game, name: &dyn Fn(u32) -> String) {
        let Some(menu) = &self.cell_menu else { return };
        let rows = self.cell_rows(game, menu, name);
        if rows.is_empty() {
            self.cell_menu = None;
            return;
        }
        let at = menu.at;
        let fresh = menu.fresh;
        let (words, orders): (Vec<theme::Row>, Vec<GearOrder>) = rows.into_iter().unzip();
        let (chosen, rect) = theme::popup(ctx, "cell-menu", at, &words);
        if let Some(i) = chosen {
            self.orders.push(orders[i]);
            self.cell_menu = None;
            return;
        }
        let pressed = ctx.input(|i| i.pointer.any_pressed());
        let pos = ctx.input(|i| i.pointer.interact_pos());
        if let Some(menu) = &mut self.cell_menu {
            if fresh {
                menu.fresh = false;
            } else if pressed && !pos.is_some_and(|p| rect.contains(p)) {
                self.cell_menu = None;
            }
        }
    }

    /// The parts, by category — a list of headings, one open at a time
    /// with a way back — or every part that matches what is typed in the
    /// search box; a row each with its picture's colour, its size and what
    /// it is made of, dimmed where the hold has not got it. Picking one
    /// puts the blueprint in the pointer's hand. Under the parts, the sites
    /// laid out so far, each with what has reached it and a way to call it
    /// off.
    fn build(&mut self, ui: &mut egui::Ui, actions: &mut Actions) {
        ui.set_max_width(380.0);
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("Build").small().color(theme::MUTED));
            theme::question_mark(ui, BUILD_TIP);
            ui.add_space(8.0);
            ui.label(egui::RichText::new("Search").small().color(theme::MUTED));
            let box_ = ui.add(
                egui::TextEdit::singleline(&mut self.search)
                    .desired_width(120.0)
                    .hint_text("table, wall, …"),
            );
            if !self.search.is_empty() && ui.small_button("×").on_hover_text("Clear").clicked() {
                self.search.clear();
                box_.request_focus();
            }
        });
        if !actions.at_rest {
            ui.label(
                egui::RichText::new("Nothing is built while the ship is moving. Sites can be laid out once it is at rest.")
                    .small()
                    .color(theme::WARN),
            );
        }

        // Which rows to show: a category's, or the search's matches from
        // every category — and only the parts the crew know how to build;
        // the rest are the Research tab's to name.
        let known = |code: &u32| {
            actions
                .research
                .parts
                .get(*code as usize)
                .copied()
                .unwrap_or(true)
        };
        let needle = self.search.trim().to_lowercase();
        let rows: Vec<u32> = if !needle.is_empty() {
            BUILD_GROUPS
                .iter()
                .flat_map(|(_, _, kinds)| kinds.iter().copied())
                .filter(|&code| {
                    PartKind::from_code(code)
                        .is_some_and(|k| part_name(k).to_lowercase().contains(&needle))
                })
                .filter(known)
                .collect()
        } else if let Some(open) = self.open_group {
            BUILD_GROUPS
                .get(open)
                .map(|(_, _, kinds)| kinds.iter().copied().filter(known).collect())
                .unwrap_or_default()
        } else {
            Vec::new()
        };

        egui::ScrollArea::vertical()
            .max_height(260.0)
            .show(ui, |ui| {
                if needle.is_empty() {
                    match self.open_group {
                        None => {
                            // The categories, a button each; what is
                            // inside is the button's tooltip, so the list
                            // reads as a list rather than a page of notes.
                            for (i, (name, hint, _)) in BUILD_GROUPS.iter().enumerate() {
                                if ui
                                    .add(egui::Button::new(*name).min_size(egui::vec2(120.0, 0.0)))
                                    .on_hover_text(*hint)
                                    .clicked()
                                {
                                    self.open_group = Some(i);
                                }
                            }
                            return;
                        }
                        Some(open) => {
                            ui.horizontal(|ui| {
                                if ui.button("< Back").clicked() {
                                    self.open_group = None;
                                }
                                if let Some((name, _, _)) = BUILD_GROUPS.get(open) {
                                    ui.label(egui::RichText::new(*name).strong());
                                }
                            });
                        }
                    }
                } else if rows.is_empty() {
                    ui.label(egui::RichText::new("Nothing by that name.").color(theme::MUTED));
                }
                for code in rows {
                    let Some(kind) = PartKind::from_code(code) else {
                        continue;
                    };
                    self.part_row(ui, kind, actions);
                }
            });

        // The sites laid out, and a way to call each off.
        if !actions.sites.is_empty() {
            ui.separator();
            ui.label(egui::RichText::new("Laid out").small().color(theme::MUTED));
            let mut cancel = None;
            for site in &actions.sites {
                ui.horizontal(|ui| {
                    ui.label(part_name(site.kind));
                    ui.label(
                        egui::RichText::new(format!("{}, {}", site.at.0, site.at.1))
                            .small()
                            .color(theme::MUTED),
                    );
                    ui.label(
                        egui::RichText::new(&site.progress)
                            .small()
                            .color(if site.stocked {
                                theme::ACCENT
                            } else {
                                theme::MUTED
                            }),
                    );
                    if ui.small_button("Cancel").clicked() {
                        cancel = Some(site.id);
                    }
                });
            }
            if let Some(id) = cancel {
                actions.cancel.push(id);
            }
        }
        let hint = match self.tool {
            Some(Tool::Build(kind)) => format!(
                "{} in hand: click the deck — or the space beside it — to lay it out; {} turns it; right-click or Esc puts it down.",
                part_name(kind),
                self.keys.key(Action::Turn).symbol_or_name()
            ),
            _ => "Pick a part and click where it is to go. The crew carry what it is made of from the shelves and build it; a site beyond the hull is built in a suit.".to_string(),
        };
        ui.add(egui::Label::new(egui::RichText::new(hint).small().color(theme::MUTED)).wrap());
    }

    /// The research tree: the nodes as boxes in columns by how deep they
    /// sit — what is known from the start on the left, what waits on it
    /// to the right — with a line from each to what it needs, coloured
    /// for their state: known, being researched (and how far), open to
    /// begin, waiting on a key, or waiting on something else. A click on
    /// a box picks it, and under the tree the picked node says what it
    /// opens, what it wants and what it is waiting on, with the button
    /// that puts the AI onto it. Over the tree, the desk: whether there
    /// is one running, and the key in it with the button that consumes
    /// it.
    fn research(&mut self, ui: &mut egui::Ui, actions: &mut Actions) {
        let view = &actions.research;
        ui.set_max_width(400.0);
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("Research").small().color(theme::MUTED));
            theme::question_mark(ui, RESEARCH_TIP);
        });
        // The desk and the key.
        if !view.desk {
            ui.label(egui::RichText::new(NO_DESK_HINT).small().color(theme::WARN));
        } else if !view.powered {
            ui.label(
                egui::RichText::new(DESK_DARK_HINT)
                    .small()
                    .color(theme::WARN),
            );
        }
        ui.label(
            egui::RichText::new(format!(
                "Keys in the desk: {} — a locked node wants one consumed for it",
                view.keys
            ))
            .small()
            .color(theme::MUTED),
        );
        ui.add_space(4.0);

        // The tree. Depth is one past the deepest prerequisite; the boxes
        // of a depth are stacked in table order.
        let view = &actions.research;
        let mut depth = [0usize; NODES];
        for node in Node::ALL {
            depth[node as usize] = node
                .def()
                .requires
                .iter()
                .map(|r| depth[*r as usize] + 1)
                .max()
                .unwrap_or(0);
        }
        let cols = depth.iter().max().copied().unwrap_or(0) + 1;
        let mut row = [0usize; NODES];
        let mut per_col = vec![0usize; cols];
        for node in Node::ALL {
            let d = depth[node as usize];
            row[node as usize] = per_col[d];
            per_col[d] += 1;
        }
        let rows = per_col.iter().max().copied().unwrap_or(1);
        let (bw, bh, gx, gy) = (88.0f32, 30.0f32, 22.0f32, 10.0f32);
        let size = egui::vec2(
            cols as f32 * bw + (cols as f32 - 1.0) * gx,
            rows as f32 * bh + (rows as f32 - 1.0) * gy,
        );
        let (rect, response) = ui.allocate_exact_size(size, egui::Sense::click());
        let painter = ui.painter_at(rect);
        let box_of = |node: Node| {
            let (d, r) = (depth[node as usize] as f32, row[node as usize] as f32);
            egui::Rect::from_min_size(
                egui::pos2(rect.min.x + d * (bw + gx), rect.min.y + r * (bh + gy)),
                egui::vec2(bw, bh),
            )
        };
        // The lines first, under the boxes.
        for node in Node::ALL {
            let to = box_of(node);
            for r in node.def().requires {
                let from = box_of(*r);
                let a = egui::pos2(from.max.x, from.center().y);
                let b = egui::pos2(to.min.x, to.center().y);
                let mid = egui::pos2((a.x + b.x) / 2.0, a.y);
                let mid2 = egui::pos2(mid.x, b.y);
                let ink = if view.done[node as usize] {
                    theme::ACCENT
                } else {
                    theme::LINE
                };
                painter.line_segment([a, mid], egui::Stroke::new(1.5, ink));
                painter.line_segment([mid, mid2], egui::Stroke::new(1.5, ink));
                painter.line_segment([mid2, b], egui::Stroke::new(1.5, ink));
            }
        }
        let hovered = response
            .hover_pos()
            .and_then(|p| Node::ALL.into_iter().find(|n| box_of(*n).contains(p)));
        if let Some(p) = response.interact_pointer_pos()
            && response.clicked()
            && let Some(node) = Node::ALL.into_iter().find(|n| box_of(*n).contains(p))
        {
            self.research_pick = Some(node);
        }
        let picked = self.research_pick;
        for node in Node::ALL {
            let b = box_of(node);
            let i = node as usize;
            let current = view.current == Some(node.code());
            let (fill, edge, ink) = if view.done[i] {
                (theme::RAISED_ON, theme::ACCENT, theme::INK)
            } else if current {
                (theme::RAISED, theme::ACCENT, theme::INK)
            } else if view.available[i] {
                (theme::RAISED, theme::MUTED, theme::INK)
            } else if view.needs_key[i] {
                (theme::PANEL_DEEP, theme::GRAVE, theme::CAUTION)
            } else {
                (theme::PANEL_DEEP, theme::LINE, theme::MUTED)
            };
            let lit = hovered == Some(node) || picked == Some(node);
            painter.rect(
                b,
                4.0,
                fill,
                egui::Stroke::new(
                    if lit { 2.0 } else { 1.0 },
                    if lit { theme::INK } else { edge },
                ),
                egui::StrokeKind::Inside,
            );
            if current {
                // How far the AI has got, as a wash along the bottom.
                let bar = egui::Rect::from_min_max(
                    egui::pos2(b.min.x + 3.0, b.max.y - 5.0),
                    egui::pos2(b.max.x - 3.0, b.max.y - 2.0),
                );
                theme::bar_in(&painter, bar, view.fraction as f32, theme::ACCENT);
            }
            let label = node_name(node.code());
            let font = egui::FontId::proportional(11.5);
            let galley = painter.layout_no_wrap(label.to_string(), font, ink);
            let at = egui::pos2(
                b.center().x - galley.size().x / 2.0,
                b.center().y - galley.size().y / 2.0 - if current { 2.0 } else { 0.0 },
            );
            painter.galley(at, galley, ink);
            if view.needs_key[i] {
                // A small lock: a mark in the corner.
                painter.circle_filled(egui::pos2(b.max.x - 7.0, b.min.y + 7.0), 3.0, theme::GRAVE);
            }
        }
        if let Some(node) = hovered {
            response.clone().on_hover_text(node_line(node.code()));
        }

        // The picked node, under the tree.
        ui.add_space(6.0);
        let Some(node) = picked else {
            ui.label(
                egui::RichText::new("Click a node for what it opens and to put the AI onto it.")
                    .small()
                    .color(theme::MUTED),
            );
            return;
        };
        let i = node as usize;
        let def = node.def();
        ui.label(egui::RichText::new(node_name(node.code())).strong());
        ui.add(egui::Label::new(egui::RichText::new(node_line(node.code())).small()).wrap());
        if !def.requires.is_empty() {
            let wants: Vec<&str> = def.requires.iter().map(|r| node_name(r.code())).collect();
            ui.label(
                egui::RichText::new(format!("After: {}", wants.join(", ")))
                    .small()
                    .color(theme::MUTED),
            );
        }
        let state = if view.done[i] {
            if def.minutes == 0 {
                "Known from the start.".to_string()
            } else {
                "Researched.".to_string()
            }
        } else if view.current == Some(node.code()) {
            format!(
                "Being researched — {} of {}.",
                span_text((view.fraction * def.minutes as f64) as f32),
                span_text(def.minutes as f32)
            )
        } else if view.needs_key[i] {
            format!(
                "Locked: wants a tier-{} research key consumed at the desk for it. {}",
                def.tier,
                span_text(def.minutes as f32)
            )
        } else if view.available[i] {
            format!(
                "{}Can be begun: {} of the AI's time.",
                if view.unlocked[i] {
                    "Key consumed. "
                } else {
                    ""
                },
                span_text(def.minutes as f32)
            )
        } else {
            format!(
                "Waiting on what it comes after. {}",
                span_text(def.minutes as f32)
            )
        };
        ui.label(egui::RichText::new(state).small().color(theme::MUTED));
        ui.horizontal(|ui| {
            if view.needs_key[i] {
                let can = view.keys > 0 && view.desk && view.powered;
                let hint = if view.keys == 0 {
                    "no key in the research desk — one is found on a friendly station's desk"
                } else if !view.powered {
                    "the desk has to be running"
                } else {
                    "consumes the key in the desk for this node; it stays open for good"
                };
                if ui
                    .add_enabled(can, egui::Button::new("Consume a key").small())
                    .on_hover_text(hint)
                    .on_disabled_hover_text(hint)
                    .clicked()
                {
                    actions
                        .research_orders
                        .push(ResearchOrder::Unlock(node.code()));
                }
            }
            if view.current == Some(node.code()) {
                if ui
                    .small_button("Stop")
                    .on_hover_text("takes the AI off it; what was put in is lost")
                    .clicked()
                {
                    actions.research_orders.push(ResearchOrder::Cancel);
                }
            } else if !view.done[i] {
                let can = view.available[i] && view.desk;
                let hint = if !view.desk {
                    NO_DESK_HINT
                } else if view.needs_key[i] {
                    "consume a key at the desk first"
                } else if !view.available[i] {
                    "research what it comes after first"
                } else if view.current.is_some() {
                    "puts the AI onto this instead; what it was on is dropped"
                } else {
                    "puts the AI onto it"
                };
                if ui
                    .add_enabled(can, egui::Button::new("Research").small())
                    .on_hover_text(hint)
                    .on_disabled_hover_text(hint)
                    .clicked()
                {
                    actions
                        .research_orders
                        .push(ResearchOrder::Begin(node.code()));
                }
            }
        });
    }

    /// One part on the Build tab: its colour, a button that puts it in
    /// hand, its size, and what it is made of — each material dimmed to a
    /// warning where the hold has fewer free than the part wants.
    fn part_row(&mut self, ui: &mut egui::Ui, kind: PartKind, actions: &Actions) {
        let on = self.tool == Some(Tool::Build(kind));
        let (w, h) = kind.def().footprint;
        ui.horizontal(|ui| {
            theme::swatch(ui, theme::ship_color32(ship::Session::part_color(kind)));
            let button = egui::Button::new(part_name(kind)).min_size(egui::vec2(130.0, 0.0));
            let button = if on {
                button.fill(theme::RAISED_ON)
            } else {
                button
            };
            if ui.add(button).clicked() {
                self.tool = if on { None } else { Some(Tool::Build(kind)) };
            }
            if w != 1 || h != 1 {
                ui.label(
                    egui::RichText::new(format!("{w}×{h}"))
                        .small()
                        .color(theme::MUTED),
                );
            }
            let mut text = egui::text::LayoutJob::default();
            for (i, &(id, units)) in kind.def().recipe.iter().enumerate() {
                let short = actions.free[id as usize] < units;
                text.append(
                    &format!(
                        "{}{units} {}",
                        if i > 0 { ", " } else { "" },
                        resource_name(id).to_lowercase()
                    ),
                    0.0,
                    egui::TextFormat {
                        font_id: egui::FontId::proportional(11.0),
                        color: if short { theme::WARN } else { theme::MUTED },
                        ..Default::default()
                    },
                );
            }
            ui.label(text);
        });
    }

    /// The views: one row a way of looking at the ship, each a toggle, and
    /// one of them on. Plain is the ship as it is; Electricity draws the
    /// conduit under the deck and rings everything on it.
    fn view(&mut self, ui: &mut egui::Ui, actions: &mut Actions) {
        ui.set_max_width(360.0);
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("View").small().color(theme::MUTED));
            theme::question_mark(
                ui,
                "What the ship view shows over the ship. A view changes the picture and nothing else.",
            );
        });
        for (overlay, label, hint) in [
            (Overlay::Plain, "Plain", "The ship as it is."),
            (
                Overlay::Electricity,
                "Electricity",
                "The power cables, and everything that makes, holds or draws power.",
            ),
        ] {
            ui.horizontal(|ui| {
                if theme::toggle(ui, actions.overlay == overlay, label).clicked() {
                    actions.overlay = overlay;
                }
                ui.label(egui::RichText::new(hint).small().color(theme::MUTED));
            });
        }
        if actions.overlay == Overlay::Electricity {
            ui.add(
                egui::Label::new(
                    egui::RichText::new(
                        "Green is wired to a reactor; red is not. A cable joins the cable beside it, and powers whatever stands over it. The yellow number over a part is what it draws a minute — an engine's while it burns, against what it would flat out.",
                    )
                    .small()
                    .color(theme::MUTED),
                )
                .wrap(),
            );
        }
        // And the camera: which way is up, and whether it follows the crew
        // member you steer. This player's own, and no command.
        ui.add_space(4.0);
        ui.label(egui::RichText::new("Camera").small().color(theme::MUTED));
        ui.horizontal(|ui| {
            if theme::toggle(ui, !actions.head_up, "North up").clicked() {
                actions.head_up = false;
            }
            if theme::toggle(ui, actions.head_up, "Head up").clicked() {
                actions.head_up = true;
            }
            ui.add_space(8.0);
            if theme::toggle(ui, actions.follow, "Follow").clicked() {
                actions.follow = true;
            }
            if theme::toggle(ui, !actions.follow, "Free camera").clicked() {
                actions.follow = false;
            }
        });
    }

    /// The tools: one row an action, each a toggle that puts the tool in
    /// the pointer's hand. Mine is the one there is. Selected, a click on
    /// the canvas is a mark rather than a selection, and the tab says how
    /// many rocks are marked and lets the lot be cleared.
    fn actions(&mut self, ui: &mut egui::Ui, actions: &mut Actions) {
        ui.set_max_width(360.0);
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("Action").small().color(theme::MUTED));
            theme::question_mark(
                ui,
                "Pick an action and use it on the canvas. Escape or a right-click puts the pointer down again.",
            );
        });
        ui.horizontal(|ui| {
            let on = self.tool == Some(Tool::Mine);
            let button = ui.add_enabled(actions.at_site, theme::toggle_button(on, "Mine"));
            if button.clicked() {
                self.tool = if on { None } else { Some(Tool::Mine) };
            }
            ui.label(
                egui::RichText::new("Click or drag over rocks outside to mark them to be mined.")
                    .small()
                    .color(theme::MUTED),
            );
        });
        let on = self.tool == Some(Tool::Mine);
        let hint = if !actions.at_site {
            "Hold station at an asteroid belt to mine its rocks."
        } else if on {
            "Click a rock outside to mark it to be mined, or drag across the rocks to mark a whole face; click or drag over marked rocks to unmark them. A Bim with mining on its work list takes a suit out and digs the marked rocks, nearest first."
        } else {
            "An asteroid is rock on the outside; the ore is three tiles in. Silver is iron ore, purple is galvum."
        };
        ui.add(egui::Label::new(egui::RichText::new(hint).small().color(theme::MUTED)).wrap());
        if actions.at_site {
            ui.horizontal(|ui| {
                let word = if actions.marked == 1 { "rock" } else { "rocks" };
                ui.label(format!("{} {word} marked", actions.marked));
                let out_of_reach = actions.marked.saturating_sub(actions.reachable);
                if out_of_reach > 0 {
                    ui.label(
                        egui::RichText::new(format!("{out_of_reach} out of reach"))
                            .color(theme::WARN),
                    );
                    theme::question_mark(
                        ui,
                        "A rock is mined from the tile beside it, straight on, never from a corner. One with rock on every side waits until a rock in front of it is mined — mark those too.",
                    );
                }
                if ui
                    .add_enabled(actions.marked > 0, egui::Button::new("Clear marks"))
                    .clicked()
                {
                    actions.clear = true;
                }
            });
        }
    }

    /// The day, one slot an hour, painted with a brush; and under it the
    /// action thresholds.
    fn schedule(&mut self, ui: &mut egui::Ui, game: &mut Game) {
        ui.horizontal(|ui| {
            for (slot, label, color) in [(1, "Sleep", theme::SLEEP), (0, "Everything", theme::ANY)] {
                theme::swatch(ui, color);
                if theme::toggle(ui, self.brush == slot, label).clicked() {
                    self.brush = slot;
                }
            }
            let above = (game.schedule_ignore_above() * 100.0).round();
            theme::question_mark(
                ui,
                &format!(
                    "Paint the hours the Bim should be asleep. It goes to bed when one comes round — unless it is already more than {above}% rested, in which case it ignores that one — and gets up as soon as it is fully rested."
                ),
            );
        });
        let now = (game.clock_minutes() / 60.0).floor() as usize % schedule::HOURS;
        let released = ui.input(|i| i.pointer.any_released());
        if released {
            self.painting = false;
        }
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 2.0;
            for hour in 0..schedule::HOURS {
                let asleep = game.schedule_slot(hour as u32) == 1;
                let (rect, response) =
                    ui.allocate_exact_size(egui::vec2(18.0, 22.0), egui::Sense::click_and_drag());
                let fill = if asleep { theme::SLEEP } else { theme::ANY };
                ui.painter().rect_filled(rect, 3.0, fill);
                if hour == now {
                    ui.painter().rect_stroke(
                        rect,
                        3.0,
                        egui::Stroke::new(1.5, theme::ACCENT),
                        egui::StrokeKind::Inside,
                    );
                }
                ui.painter().text(
                    rect.center(),
                    egui::Align2::CENTER_CENTER,
                    hour.to_string(),
                    egui::FontId::proportional(10.0),
                    theme::INK,
                );
                let response = response.on_hover_text(format!("{hour:02}:00"));
                if response.drag_started() || response.clicked() {
                    self.painting = true;
                    game.set_schedule_slot(hour as u32, self.brush);
                } else if self.painting && response.hovered() {
                    // Dragging across the strip paints the whole run in one
                    // gesture.
                    game.set_schedule_slot(hour as u32, self.brush);
                }
            }
        });

        // When the Bim sees to itself: a tick box for whether it watches
        // that need at all, and a slider for the level it acts on.
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            ui.label(
                egui::RichText::new("Action threshold")
                    .small()
                    .color(theme::MUTED),
            );
            theme::question_mark(ui, TRIGGER_TIP);
        });
        egui::Grid::new("triggers")
            .num_columns(4)
            .spacing([8.0, 2.0])
            .show(ui, |ui| {
                for i in TRIGGER_NEEDS {
                    let mut on = game.need_trigger_on(i);
                    if ui.checkbox(&mut on, "").changed() {
                        game.set_need_trigger_on(i, on);
                    }
                    ui.label(
                        egui::RichText::new(NEED_NAMES.get(i as usize).copied().unwrap_or("Need"))
                            .color(if on { theme::INK } else { theme::MUTED }),
                    );
                    let mut at = (game.need_trigger(i) * 100.0).round() as u32;
                    if ui
                        .add(egui::Slider::new(&mut at, 0..=100).show_value(false))
                        .changed()
                    {
                        game.set_need_trigger(i, at as f32 / 100.0);
                    }
                    ui.label(
                        egui::RichText::new(format!("{at}%"))
                            .small()
                            .color(theme::MUTED),
                    );
                    ui.end_row();
                }
            });
    }

    /// The order the work gets done in. One row per job, built from the
    /// count the room reports rather than from the table of names, so a
    /// job added on that side shows up as a blank row rather than going
    /// missing.
    fn work(&mut self, ui: &mut egui::Ui, game: &mut Game) {
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("Sort").small().color(theme::MUTED));
            for (sort, label) in [
                (Sort::Up, "Priority 1→5"),
                (Sort::Down, "Priority 5→1"),
                (Sort::Name, "Name A–Z"),
            ] {
                if theme::toggle(ui, self.sort == Some(sort), label).clicked() {
                    self.sort = Some(sort);
                }
            }
        });
        let count = game.work_count();
        let mut rows: Vec<(u32, &str)> = (0..count)
            .map(|job| (job, WORK_NAMES.get(job as usize).copied().unwrap_or("")))
            .collect();
        // Sorting is pressed rather than left on, so nothing moves under
        // the pointer on the click that changed it; the mark is cleared
        // when a box is clicked. Equal priorities keep declaration order.
        match self.sort {
            Some(Sort::Up) => rows.sort_by_key(|&(job, _)| (game.work_priority(job), job)),
            Some(Sort::Down) => {
                rows.sort_by_key(|&(job, _)| (std::cmp::Reverse(game.work_priority(job)), job))
            }
            Some(Sort::Name) => rows.sort_by_key(|&(_, name)| name.to_lowercase()),
            None => {}
        }
        let highest = game.work_highest();
        let lowest = game.work_lowest();
        let never = game.work_never();
        egui::Grid::new("work")
            .num_columns(2)
            .spacing([12.0, 2.0])
            .show(ui, |ui| {
                ui.label(egui::RichText::new("Job").small().color(theme::MUTED));
                ui.label(egui::RichText::new("Priority").small().color(theme::MUTED));
                ui.end_row();
                // Which box was clicked, and which way: a left click takes one
                // off the number, a right click puts one on.
                let mut clicked: Option<(u32, bool)> = None;
                for (job, name) in rows {
                    let level = game.work_priority(job);
                    let off = level == never;
                    let row = ui.label(if off {
                        egui::RichText::new(name).color(theme::MUTED)
                    } else {
                        egui::RichText::new(name)
                    });
                    // A colour a level, so the list reads at a glance: hot at
                    // the top, cooling down the range, and a red cross for a
                    // job the crew are never to do. The box is painted by hand
                    // rather than through the button's text so the cross is a
                    // shape and not a glyph the font may not have.
                    let fill = priority_colour(level, never, highest, lowest);
                    let text = if off {
                        String::new()
                    } else {
                        level.to_string()
                    };
                    let button = ui
                        .add(
                            egui::Button::new(egui::RichText::new(text).color(theme::PANEL_DEEP))
                                .fill(fill)
                                .min_size(egui::vec2(28.0, 0.0)),
                        )
                        .on_hover_text(if off {
                            "Never — the crew do not do this. Click to set a priority.".to_string()
                        } else {
                            format!(
                                "Priority {level} — {highest} is done first, {lowest} last. \
                             Click for one more important; right-click for one less; \
                             past {highest} is never."
                            )
                        });
                    if off {
                        let r = button.rect.shrink(7.0);
                        let stroke = egui::Stroke::new(2.0, theme::PANEL_DEEP);
                        ui.painter()
                            .line_segment([r.left_top(), r.right_bottom()], stroke);
                        ui.painter()
                            .line_segment([r.left_bottom(), r.right_top()], stroke);
                    }
                    if button.clicked() {
                        clicked = Some((job, false));
                    } else if button.secondary_clicked() {
                        clicked = Some((job, true));
                    }
                    let spot = WORK_SPOTS
                        .get(job as usize)
                        .copied()
                        .unwrap_or(SPOT_NOTHING);
                    self.points(&row, spot);
                    self.points(&button, spot);
                    ui.end_row();
                }
                if let Some((job, back)) = clicked {
                    if back {
                        game.cycle_work_priority_back(job);
                    } else {
                        game.cycle_work_priority(job);
                    }
                    self.sort = None;
                }
            });
    }

    /// What is aboard, and what to keep in stock: the three targets, by
    /// `manager::Stock`, each in the row of the thing it is a target for.
    fn management(
        &mut self,
        ui: &mut egui::Ui,
        game: &mut Game,
        mut actions: Option<&mut Actions>,
    ) {
        ui.horizontal(|ui| {
            let mut on = game.is_autonomous();
            if ui.checkbox(&mut on, "").changed() {
                game.set_autonomous(on);
            }
            theme::asks(ui, "Let the Bim decide", AUTONOMY_TIP);
        });
        // The workbench's upgrade: a tick box, and while one is on the
        // bench a line saying what and how far. The ship's only — the
        // room alone has no hold and no bench worth the name.
        if let Some(actions) = actions.as_deref_mut() {
            ui.horizontal(|ui| {
                let mut on = actions.auto_upgrade;
                if ui.checkbox(&mut on, "").changed() {
                    actions.set_auto_upgrade = Some(on);
                }
                theme::asks(ui, UPGRADE_LABEL, UPGRADE_TIP);
            });
            if let Some(upgrade) = actions.upgrade {
                ui.label(
                    egui::RichText::new(upgrade_line(
                        upgrade.resource,
                        upgrade.tier,
                        upgrade.done,
                        upgrade.of,
                        upgrade.waiting,
                    ))
                    .small()
                    .color(theme::MUTED),
                );
            }
        }
        egui::Grid::new("stock")
            .num_columns(4)
            .spacing([12.0, 2.0])
            .show(ui, |ui| {
                for head in ["Item", "Stock", "Location"] {
                    ui.label(egui::RichText::new(head).small().color(theme::MUTED));
                }
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new("Target").small().color(theme::MUTED));
                    theme::question_mark(ui, TARGET_TIP);
                });
                ui.end_row();
                let rows = [
                    ("Vegetables", game.store_veg(), Stock::Veg),
                    ("Tofu", game.store_tofu(), Stock::Tofu),
                    ("Stew", game.store_stew(), Stock::Stew),
                    ("Fibre", game.store_fibre(), Stock::Fibre),
                ];
                for (name, held, which) in rows {
                    // Fibre is the one row that is not food, and the
                    // Target tip above says nothing about it, so its own
                    // word asks.
                    let a = if which == Stock::Fibre {
                        theme::asks(ui, name, FIBRE_TIP)
                    } else {
                        ui.label(name)
                    };
                    let b = ui.label(held.to_string());
                    let c = ui.label("Cold store");
                    let mut target = game.target(which);
                    let input = ui.add(
                        egui::DragValue::new(&mut target)
                            .range(0..=manager::MOST)
                            .speed(0.2),
                    );
                    if input.changed() {
                        game.set_target(which, target);
                    }
                    for r in [&a, &b, &c] {
                        self.points(r, SPOT_FRIDGE);
                    }
                    // The vegetable and tofu targets are the bay's orders and
                    // the stew target the hob's, so resting on the cell rings
                    // the place that answers it.
                    self.points(&input, KEEP_SPOTS[which as usize]);
                    ui.end_row();
                }
                // Then what the benches make, on the ship: the same kind of
                // standing order, kept in the hold rather than the cold
                // store, and answered by the smelter and the workbench.
                let Some(actions) = actions else {
                    return;
                };
                if actions.crafts.is_empty() {
                    return;
                }
                ui.label(
                    egui::RichText::new("Made aboard")
                        .small()
                        .color(theme::MUTED),
                );
                ui.end_row();
                for craft in &actions.crafts {
                    // A recipe the crew have not researched is greyed, and
                    // its box is dead: a target nobody can work to is a
                    // target that reads as a bench that is broken.
                    let tip = match craft.needs {
                        Some(node) => format!(
                            "{RESEARCH_LOCKED}: {} — see the Research tab. {}",
                            node_name(node),
                            craft.recipe
                        ),
                        None => format!("Keep this many made. {}", craft.recipe),
                    };
                    let ink = if craft.needs.is_some() {
                        theme::MUTED
                    } else {
                        theme::INK
                    };
                    ui.label(egui::RichText::new(resource_name(craft.resource)).color(ink))
                        .on_hover_text(&tip);
                    ui.label(egui::RichText::new(craft.held.to_string()).color(ink));
                    ui.label(egui::RichText::new(craft.kept_in).color(ink));
                    let mut target = craft.target;
                    let input = ui
                        .add_enabled(
                            craft.needs.is_none(),
                            egui::DragValue::new(&mut target)
                                .range(0..=craft.most)
                                .speed(0.2),
                        )
                        .on_hover_text(&tip);
                    if input.changed() {
                        actions.keep.push((craft.resource, target));
                    }
                    ui.end_row();
                }
            });
    }

    // --- what the pointer is over -------------------------------------------

    /// The state worth naming beside a fixture, or "" for the things that
    /// have none.
    fn spot_state(game: &Game, spot: u32, x: f32, y: f32) -> String {
        match spot {
            SPOT_SHIP_DOOR => match game.door_at(x, y) {
                None => String::new(),
                Some(door) => {
                    if game.ship_door_is_locked(door) {
                        "locked".into()
                    } else if game.ship_door_is_held(door) {
                        "held open".into()
                    } else if game.ship_door_is_open(door) {
                        "open".into()
                    } else {
                        "shut".into()
                    }
                }
            },
            SPOT_FRIDGE => {
                let open = game.fridge_at(x, y).is_some_and(|i| game.fridge_is_open(i));
                if open { "open" } else { "" }.into()
            }
            SPOT_HOB => {
                let lit = game.hob_at(x, y).is_some_and(|i| game.stove_is_on(i));
                if lit { "lit" } else { "" }.into()
            }
            SPOT_BOARD => {
                let plates = game.plates();
                if plates == 1 {
                    "1 plate in the drawer".into()
                } else {
                    format!("{plates} plates in the drawer")
                }
            }
            SPOT_DISHWASHER => {
                let i = game.dishwasher_at(x, y).unwrap_or(0);
                if game.dishwasher_cycle_left(i) > 0.0 {
                    "running".into()
                } else if game.dishwasher_loaded(i) > 0 {
                    format!("{} plates in it", game.dishwasher_loaded(i))
                } else {
                    String::new()
                }
            }
            SPOT_BAY => {
                let ripe = game
                    .bay_at(x, y)
                    .map(|bay| game.hydro_ripe(bay))
                    .unwrap_or(0);
                if ripe > 0 {
                    format!("{ripe} ready to lift")
                } else {
                    String::new()
                }
            }
            SPOT_DOOR => {
                if game.door_is_locked() {
                    "locked".into()
                } else if game.door_is_open() {
                    "open".into()
                } else {
                    "shut".into()
                }
            }
            _ => String::new(),
        }
    }

    /// What the room makes of a point on the deck, in room coordinates: the
    /// spot code, the thing there and its state — "Hob · lit" — and
    /// whatever is lying on the deck at that spot, or "" when it is clean
    /// or not deck at all.
    pub fn spot_readout(game: &Game, x: f32, y: f32) -> (u32, String, String) {
        let spot = game.spot_at(x, y);
        let mut thing = SPOT_NAMES
            .get(spot as usize)
            .copied()
            .unwrap_or("Something")
            .to_string();
        let state = Self::spot_state(game, spot, x, y);
        if !state.is_empty() {
            thing = format!("{thing} · {state}");
        }
        let mut on_it = String::new();
        if DECK_SPOTS.contains(&spot) {
            let mess = MESS_NAMES
                .get(game.spot_mess(x, y) as usize)
                .copied()
                .unwrap_or("");
            if !mess.is_empty() {
                let deep = (game.spot_mess_depth(x, y) * 100.0).round();
                on_it = format!("{mess} — {deep}% fouled");
            }
        }
        (spot, thing, on_it)
    }
}

/// The room's own idea of who is steered: `bim::PLAYER`.
/// The colour of a priority box: red for never, and from there a ramp
/// from hot at the top of the range to cool grey at the bottom, so the
/// Work tab reads as a heat map rather than a column of threes.
fn priority_colour(level: u32, never: u32, highest: u32, lowest: u32) -> egui::Color32 {
    if level == never {
        return theme::BAD;
    }
    // 0 at the most important, 1 at the least.
    let span = lowest.saturating_sub(highest).max(1) as f32;
    let t = (level.saturating_sub(highest) as f32 / span).clamp(0.0, 1.0);
    // Two straight ramps: warm to the accent green over the first half,
    // the accent down to the muted grey over the second.
    let mix = |a: egui::Color32, b: egui::Color32, k: f32| {
        let l = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * k).round() as u8;
        egui::Color32::from_rgb(l(a.r(), b.r()), l(a.g(), b.g()), l(a.b(), b.b()))
    };
    if t < 0.5 {
        mix(theme::WARN, theme::ACCENT, t * 2.0)
    } else {
        mix(theme::ACCENT, theme::MUTED, (t - 0.5) * 2.0)
    }
}

/// One slot of the inventory: a box with what is in it — its icon, its
/// name, a line of its numbers and, for a piece of armour, its health
/// along the bottom — and the slot's name under the box. An empty slot is
/// drawn hollow, a filled one lit. Painted on a rect of a fixed size
/// rather than laid out, so it is a slot and not whatever space the
/// window happens to have. The response is the box's, for a right-click.
fn slot(
    ui: &mut egui::Ui,
    label: &str,
    item: Option<PackItem>,
    name: &str,
    line: &str,
) -> egui::Response {
    ui.vertical(|ui| {
        let (rect, response) =
            ui.allocate_exact_size(egui::vec2(SLOT_WIDTH, SLOT_HEIGHT), egui::Sense::click());
        let painter = ui.painter();
        painter.rect(
            rect,
            4.0,
            theme::PANEL_DEEP,
            egui::Stroke::new(
                1.0,
                if item.is_some() {
                    theme::ACCENT
                } else {
                    theme::LINE
                },
            ),
            egui::StrokeKind::Inside,
        );
        match item {
            None => {
                painter.text(
                    rect.center(),
                    egui::Align2::CENTER_CENTER,
                    name,
                    egui::FontId::proportional(13.0),
                    theme::MUTED,
                );
            }
            Some(item) => {
                // A tiered thing's slot is washed and edged in its tier's
                // colour, like its cell in a grid.
                if let Some(tint) = theme::item_tint(item) {
                    theme::tint_cell(painter, rect, 4.0, tint);
                }
                let pad = 6.0;
                let icon = egui::Rect::from_min_size(
                    rect.min + egui::vec2(pad, pad),
                    egui::vec2(SLOT_HEIGHT - pad * 2.0, SLOT_HEIGHT - pad * 2.0),
                );
                icons::icon(painter, icon, item);
                let x = icon.max.x + pad;
                painter.text(
                    egui::pos2(x, rect.min.y + pad),
                    egui::Align2::LEFT_TOP,
                    name,
                    egui::FontId::proportional(12.5),
                    theme::INK,
                );
                if !line.is_empty() {
                    painter.text(
                        egui::pos2(x, rect.max.y - pad),
                        egui::Align2::LEFT_BOTTOM,
                        line,
                        egui::FontId::proportional(10.0),
                        theme::MUTED,
                    );
                }
                if let PackItem::Armour(piece) = item {
                    let bar = egui::Rect::from_min_max(
                        egui::pos2(icon.min.x, rect.max.y - 5.0),
                        egui::pos2(icon.max.x, rect.max.y - 3.0),
                    );
                    theme::bar_in(
                        painter,
                        bar,
                        piece.health / piece.stats().health.max(1.0),
                        if piece.broken() {
                            theme::BAD
                        } else {
                            theme::ARMOUR
                        },
                    );
                }
            }
        }
        ui.label(egui::RichText::new(label).small().color(theme::MUTED));
        ui.add_space(4.0);
        response
    })
    .inner
}

/// A slot's box.
const SLOT_WIDTH: f32 = 120.0;
const SLOT_HEIGHT: f32 = 44.0;

/// The line under a worn piece's name: what it is adding and taking off,
/// or that it is past it.
fn worn_line(piece: Piece) -> String {
    if piece.broken() {
        "broken — does nothing".into()
    } else {
        format!(
            "+{} hp · {} prot",
            piece.health.round(),
            piece.stats().protection
        )
    }
}

/// A grid cell for a thing, with its tooltip.
fn cell_of(item: PackItem, count: u32) -> Cell {
    Cell::new(item, count, tip_of(item, count))
}

/// A cell's tooltip: the name, the numbers that matter — a piece's
/// health and protection, and what it has left — and the resource's line.
fn tip_of(item: PackItem, count: u32) -> String {
    // A tier above one is said after the name: "Basic helm — tier 2".
    let tiered = |name: &str, tier: bims::combat::Tier| match tier_word(tier) {
        Some(word) => format!("{name} — {word}"),
        None => name.to_string(),
    };
    match item {
        PackItem::Armour(piece) => {
            let stats = piece.stats();
            let state = if piece.broken() {
                "Broken — still worn, doing nothing".to_string()
            } else {
                format!("{} of {} hp left", piece.health.round(), stats.health)
            };
            let dodge = if piece.dodge() > 0.0 {
                format!(", {}% dodge", (piece.dodge() * 100.0).round())
            } else {
                String::new()
            };
            format!(
                "{} — {}\n+{} hp, {} protection{dodge} · {state}\n{}",
                tiered(armour_name(Some(piece.kind)), piece.tier),
                SLOT_NAMES[piece.kind.slot() as usize].to_lowercase(),
                stats.health,
                stats.protection,
                item_tip(ResourceId::ALL[piece.kind.resource() as usize])
            )
        }
        PackItem::Weapon(weapon) => {
            // The curve in a line, the way the Inventory says it, or a
            // blade's swing — the tier's numbers, not the kind's.
            let stats = weapon.stats();
            let numbers = if stats.melee {
                melee_text(&stats)
            } else {
                format!(
                    "Damage {}, range {} tiles",
                    damage_text(&stats),
                    tidy(stats.range)
                )
            };
            let name = tiered(weapon_name(Some(weapon.kind)), weapon.tier);
            let name = if count > 1 {
                format!("{name} × {count}")
            } else {
                name
            };
            format!(
                "{name}\n{numbers}\n{}",
                item_tip(world::armour::weapon_resource(weapon.kind))
            )
        }
        PackItem::Stack(code) => match ResourceId::ALL.get(code as usize) {
            Some(&id) if count > 1 => format!("{} × {count}\n{}", resource_name(id), item_tip(id)),
            Some(&id) => format!("{}\n{}", resource_name(id), item_tip(id)),
            None => "Something the hold does not know".into(),
        },
        PackItem::Key(tier) => match world::armour::key_resource(tier) {
            Some(id) => format!(
                "{} — tier {tier}
{}",
                resource_name(id),
                item_tip(id)
            ),
            None => "A research key of a tier the hold does not know".into(),
        },
    }
}

/// Which class of the hold a container is a window onto: a workstation's
/// is its part's, if it keeps one (the armoury's lockers), a shelf's the
/// shelves, a cold store's the cold; `None` for a container the room no
/// longer has.
pub fn container_class(game: &Game, container: Container) -> Option<Storage> {
    class_of(game, container)
}

fn class_of(game: &Game, container: Container) -> Option<Storage> {
    match container {
        Container::Bench(i) => PartKind::from_code(game.bench_part(i))?
            .def()
            .capacity
            .map(|(class, _)| class),
        Container::Shelf(_) => game.container_frame(container).map(|_| Storage::Shelf),
        Container::Fridge(_) => game.container_frame(container).map(|_| Storage::ColdStore),
        Container::Desk(_) => game.container_frame(container).map(|_| Storage::Research),
    }
}

/// The cells of a container window over a class of the hold that is no
/// grid — the research desk — and what each is: a stack a resource of
/// the class with anything in it. Everything else lies on its class's
/// grid ([`grid_things`]).
fn container_cells(class: Storage, hold: &Hold) -> (Vec<Option<Cell>>, Vec<HoldCell>) {
    let mut cells = Vec::new();
    let mut what = Vec::new();
    for &id in ResourceId::ALL.iter() {
        if economy::storage(id) != class || world::armour::is_gear(id) {
            continue;
        }
        let count = hold.counts[id as usize];
        if count == 0 {
            continue;
        }
        cells.push(Some(cell_of(world::armour::item_of(id), count)));
        what.push(HoldCell::Stack(id));
    }
    (cells, what)
}

/// The things on a class's grid as the widget draws them — every slot
/// over its footprint, a piece with its health, a gun at its tier, a
/// stack of anything else with its count — and the slot each is, in the
/// same order.
fn grid_things(grid: &Grid, hold: &Hold) -> (Vec<grid::Laid>, Vec<u32>) {
    let mut things = Vec::new();
    let mut slots = Vec::new();
    for slot in &grid.slots {
        let item = match slot.kept {
            Kept::Piece(id) => match hold.pieces.iter().find(|p| p.id == id) {
                Some(&piece) => PackItem::Armour(piece),
                None => continue,
            },
            Kept::Gun(kind, tier) => PackItem::Weapon(kind.at(tier)),
            Kept::Stack(id) => world::armour::item_of(id),
        };
        let laid = slot.laid();
        things.push(grid::Laid {
            x: slot.x as usize,
            y: slot.y as usize,
            cols: laid.cols as usize,
            rows: laid.rows as usize,
            turned: slot.turned,
            cell: cell_of(item, slot.count),
        });
        slots.push(slot.id);
    }
    (things, slots)
}

/// The things in a pack as the grid draws them — every thing over its
/// footprint, turned if it is — and the cell each is kept in, in the
/// same order. The same for a body's pack in the Loot window, off the
/// body's cells and which of them are turned.
fn pack_things(gear: &bims::combat::Gear) -> (Vec<grid::Laid>, Vec<usize>) {
    laid_things(&gear.pack, &gear.turned)
}

fn laid_things(pack: &[Option<PackItem>], turned: &[bool]) -> (Vec<grid::Laid>, Vec<usize>) {
    let mut things = Vec::new();
    let mut heads = Vec::new();
    for (cell, item) in pack.iter().enumerate().take(PACK_CELLS) {
        let Some(item) = *item else {
            continue;
        };
        let turned = turned.get(cell).copied().unwrap_or(false);
        let (rows, cols) = item.laid(turned);
        things.push(grid::Laid {
            x: cell % PACK_COLS,
            y: cell / PACK_COLS,
            cols,
            rows,
            turned,
            cell: cell_of(item, 1),
        });
        heads.push(cell);
    }
    (things, heads)
}

/// What a slot of the lockers holds, as the resource it counts as: the
/// piece's kind looked up, the gun's, or the unit itself.
fn kept_resource(kept: Kept, hold: &Hold) -> Option<ResourceId> {
    match kept {
        Kept::Piece(id) => hold
            .pieces
            .iter()
            .find(|p| p.id == id)
            .map(|p| ResourceId::ALL[p.kind.resource() as usize]),
        Kept::Gun(kind, _) => Some(world::armour::weapon_resource(kind)),
        Kept::Stack(id) => Some(id),
    }
}

/// What a fetch of a container cell asks the world for: the slot itself,
/// so the thing clicked is the thing that goes.
fn fetch_kind(cell: HoldCell) -> FetchKind {
    match cell {
        HoldCell::Slot(class, id) => FetchKind::Slot {
            class: class.code(),
            id,
        },
        HoldCell::Stack(id) => FetchKind::Resource(id as u32),
    }
}

/// The strip of what is within reach of the Bim shown — every container,
/// every body down — a button each, the open one lit, so the rest are a
/// click away. Over the container and Loot windows, and over the
/// inventory when nothing else is up. What was clicked, for
/// `CrewPanels::follow_strip` once the window is laid out.
fn nearby_strip(ui: &mut egui::Ui, nearby: &[Near], open: Option<Open>) -> Option<Open> {
    if nearby.is_empty() {
        return None;
    }
    let mut pick = None;
    ui.horizontal_wrapped(|ui| {
        ui.label(egui::RichText::new("Nearby").small().color(theme::MUTED));
        for near in nearby {
            let lit = open == Some(near.open);
            if ui.selectable_label(lit, &near.label).clicked() && !lit {
                pick = Some(near.open);
            }
        }
    });
    pick
}

/// Whose hands a treatment of `patient` would be: the player's own Bim
/// for a crewmate, while it is alive, awake and aboard; for the player's
/// own — nobody treats their own — the nearest crewmate that is free to.
/// `None` when nobody can.
fn treat_helper(game: &Game, player: usize, patient: usize) -> Option<usize> {
    let up = |h: usize| game.is_alive(h) && !game.is_unconscious(h) && !game.is_outside(h);
    if patient != player {
        return up(player).then_some(player);
    }
    let at = game.bim_pos(patient);
    (0..game.crew_count() as usize)
        .filter(|&h| h != patient && up(h))
        .min_by(|&a, &b| {
            (game.bim_pos(a) - at)
                .len()
                .total_cmp(&(game.bim_pos(b) - at).len())
        })
}

/// The two words beside a Bandage button, or under a menu row: how many
/// wounds are open on the part, and why the button is dead if it is —
/// nothing to dress, or nothing to dress it with.
fn bandage_words(wounds: u32, bandages: u32) -> (String, &'static str) {
    let count = match wounds {
        0 => "no wounds".to_string(),
        1 => "1 wound".to_string(),
        n => format!("{n} wounds"),
    };
    let hint = if wounds == 0 {
        "nothing open on it — a bandage here would be a bandage wasted"
    } else if bandages == 0 {
        "no bandages — the drug lab makes them out of fibre"
    } else {
        "closes every wound on it — ten minutes with hands on"
    };
    (count, hint)
}

pub fn player() -> usize {
    bim::PLAYER
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The work list's rows are built off `work_count`, so a spot table a
    /// row short would ring nothing for the last job and nobody would
    /// notice; the target table is indexed by `manager::Stock` the same way.
    #[test]
    fn every_job_and_every_target_has_a_place_to_ring() {
        assert_eq!(WORK_SPOTS.len(), bims::work::Job::ALL.len());
        assert_eq!(KEEP_SPOTS.len(), Stock::ALL.len());
    }
}
