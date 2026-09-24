//! The crew's panels: everything on the game screen that is about the Bims
//! rather than about the deck they are standing on — the room aboard,
//! stepped by the world.
//!
//! The selected crew member's health and diary, the agendas, the tray with
//! the work list and the management row, the fixture menus, and the
//! tooltips everything hangs off. What is *not* here is the canvas and the
//! pointer, because those are the screen's own — the ship turns the deck
//! with the hull — and the screen hands the room its coordinates.
//!
//! # Tooltips are asked for, never stumbled into
//!
//! Every tooltip hangs off an affordance: a word underlined with
//! [`theme::asks`], or a `?` from [`theme::question_mark`]. Never a row, a
//! bar or a panel — a player crossing the health panel on the way to the
//! deck should get nothing.
//!
//! # A highlight is not a tooltip
//!
//! Resting on a work row rings the fixture it names on the deck.
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
//! [`CrewPanels::orders`] for the screen to send through the seam as a
//! `world::Command`, since the hold is the world's and every player's ship
//! has to agree about what is in it. The hold itself comes the other way
//! as a [`Hold`] snapshot the screen sets every frame. The station's
//! shelves on a joined deck are told from the ship's by
//! `Hold::station_shelves`: they are not the hold's, and a click on one
//! opens nothing.
//!
//! A body is handed in the same way. The Loot window is a grid over what
//! a dead or unconscious crewmate has on it, and what it shows, whether the
//! Bim is still down and whether the looter is within reach come as a
//! [`Body`] snapshot the screen sets every frame *after* the fixture menu
//! has run, since the menu is what opens the window. Taking is an order
//! like the rest, `GearOrder::Loot`, and the walk over to the body is the
//! screen's too (`walk`), since where a body lies is the world's to say —
//! the same walk takes the Bim to a mercenary the Hire row is open on.

use bevy_egui::egui;
use bims::combat::{Item as PackItem, LOOT_CELLS, PACK_CELLS, PACK_COLS, PACK_ROWS, Piece};
use bims::game::{Container, Game};
use bims::order::CrewOrder;
use bims::room::*;
use bims::{door, health};
use physics::ResourceId;
use ship::game::Overlay;
use shipdesign::parts::PartKind;
use shipdesign::research::{KEY_CELLS, NODES, Node};
use shipdesign::{CARGO_SLOTS, Storage};
use world::{FetchKind, Grid, Kept, LootSource};

use crate::format::{clock_text, date_text, ordinal, span_text};
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
        Storage::Locker | Storage::ColdStore => 24.0,
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
        Storage::ColdStore | Storage::Locker => (shipdesign::GRID_COLS as usize, 0),
        Storage::Research => (KEY_CELLS.0 as usize, KEY_CELLS.1 as usize),
    }
}

/// Where a container window sits: to the right of the left-hand stack,
/// under the strip along the top. The inventory pop-up goes beside it
/// while one is open.
const CONTAINER_AT: egui::Vec2 = egui::vec2(290.0, 60.0);

/// The hold, as the panels see it: a snapshot the screen hands over
/// every frame, `None` only before the world has opened. Counts and reach are by `ResourceId`; the pieces are the armour in the
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
    pub grids: [Grid; 2],
    pub grid_capacity: [u32; 3],
    pub used: [u32; 4],
    pub capacity: [u32; 4],
    /// Whether the Bim whose inventory is shown stands within reach of a
    /// container that takes each resource — `World::in_reach`.
    pub reach: [bool; CARGO_SLOTS],
    /// Which research desk on the deck is the station's, while docked —
    /// `World::station_desk` — and which tier of key is still on it,
    /// nought for none (`World::key_at_the_dock`). The desk's row reads
    /// both.
    pub station_desk: Option<usize>,
    pub station_key: u8,
    /// Which shelves on the deck are the station's, while docked —
    /// `World::station_shelves`: not the hold's, so a click on one opens
    /// no window and the nearby strip leaves them out.
    pub station_shelves: Vec<usize>,
    /// The workbench with the slots, if one is aboard — `World::bench` —
    /// as its window draws it.
    pub bench: Option<BenchView>,
}

/// The workbench as the panels see it: which bench it is, its three slots
/// and the work on them (`world::Workbench`, whose `takes` is the rule a
/// pack row is greyed by), whether the Bim shown stands within reach of
/// it, and whether the button could be pressed — `World::can_upgrade`,
/// so the window says why not before the command is sent.
#[derive(Clone, Copy)]
pub struct BenchView {
    pub index: usize,
    pub bench: world::Workbench,
    pub reach: bool,
    pub upgrade: Result<(), world::Refusal>,
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
    /// Whether this one is a **field medic** (feature 86): hired to
    /// fetch the fallen out of the fire and treat them, not to shoot.
    pub medic: bool,
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
    /// How many are in each cell (feature 87): the body's
    /// `Game::loot_counts`, a box of dressings five.
    pub counts: [u32; LOOT_CELLS],
    /// Still dead or out cold. A crewmate that came round is no longer a
    /// body, and the window shuts on it.
    pub down: bool,
    /// Whether the Bim whose inventory is shown stands within reach of
    /// it, alive and awake — `World::in_reach_of_body`.
    pub reach: bool,
}

/// What a row or a ctrl-click asked for, about somebody's gear. The
/// screen sends it as the matching `world::Command`.
#[derive(Clone, Copy, PartialEq, Debug, serde::Serialize, serde::Deserialize)]
pub enum GearOrder {
    /// Put what is in a pack cell into a container.
    Stow { who: u32, cell: u32 },
    /// Put what is in a pack cell onto the workbench's first free input
    /// slot — `Command::StowOnBench`.
    StowOnBench { who: u32, cell: u32 },
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
    /// Take the research key off the station's desk into `who`'s pack —
    /// `Command::TakeKey`. Sent by the screen once `who` is within reach
    /// of the desk, after the desk's row walked them there.
    TakeKey { who: u32 },
    /// Bind **every** open wound on `who` out of its own pack (feature
    /// 87) — the row on a box of dressings in the inventory. The worst
    /// part now and the rest queued behind it, through
    /// `CrewOrder::BandageAll`.
    BandageAll { who: u32 },
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
    /// Not a window of the panels' own: the Trade row walks the Bim to
    /// the desk and asks the screen for the trade window
    /// (`CrewPanels::trade_requested`).
    Trade(usize),
    /// Not a window either: the station's research desk's row walks
    /// the Bim shown to it and asks the screen for the key
    /// (`CrewPanels::key_requested`).
    Key(usize),
    /// Not a window either: the engineer.s deployable within reach packed
    /// up into its pack (`Command::PackUp`) — the row on the nearby strip
    /// beside one (feature 74). By the deployable.s id. There is no
    /// refill any more: a sentry never runs out of shots (feature 88).
    PackUp(u32),
}

/// What the class section and the deployable rows asked for this frame
/// (feature 74), for the screen to send through the seam.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum DeployOrder {
    PackUp(u32),
    Pick {
        level: u32,
        side: world::Side,
    },
    SetClass(world::Class),
    /// The armourer's repair begun on the bench.
    Repair,
}

/// A player's class as the panel shows it, a snapshot the screen hands
/// over every frame off the world: what it is, how far along, the pick
/// waiting if one is, the talents learnt, and whether the class may
/// still be changed (before the first undock).
#[derive(Clone, PartialEq, Debug, Default)]
pub struct ClassView {
    pub class: world::Class,
    pub level: u8,
    pub to_next: u32,
    pub pending: Option<(u8, world::Talent, world::Talent)>,
    /// Every pick made, level and side, in level order — what the Skills
    /// tree draws as taken and as given up (feature 83). The talents
    /// themselves are `talents`, which is the same list read through the
    /// class.
    pub picks: Vec<(u8, world::Side)>,
    pub talents: Vec<world::Talent>,
    pub can_change: bool,
    /// Whether the armourer's repair could be begun now, or why not —
    /// `None` for a crew member without the talent.
    pub repair: Option<Result<(), world::Refusal>>,
    /// The soldier's rows (feature 75): grenade charges in the pack,
    /// seconds of the clock until the next comes back (feature 90), and
    /// whether it is braced — `None` for anybody but a soldier.
    pub soldier: Option<SoldierView>,
    /// The medic's rows (feature 76) — `None` for anybody but a medic.
    pub medic: Option<MedicView>,
    /// The tank's rows (feature 77) — `None` for anybody but a tank.
    pub tank: Option<TankView>,
    /// The commander's rows (feature 78) — `None` for anybody else.
    pub commander: Option<CommanderView>,
}

/// What the panel says of a soldier (feature 75).
#[derive(Clone, Copy, PartialEq, Debug, Default)]
pub struct SoldierView {
    pub grenades: u32,
    pub cooldown: f64,
    pub braced: bool,
}

/// What the panel says of a medic (feature 76): who the beam holds, by
/// name; how charged the surge is, nought to one; whether the level for
/// one has been reached; and whether one is running on the medic now.
#[derive(Clone, PartialEq, Debug, Default)]
pub struct MedicView {
    pub patients: Vec<String>,
    pub charge: f32,
    pub can_surge: bool,
    pub surging: bool,
}

/// What the panel says of a tank (feature 77): whether the wall is up,
/// minutes of the taunt left, seconds until it may taunt again, and
/// whether the level for one has been reached.
#[derive(Clone, Copy, PartialEq, Debug, Default)]
pub struct TankView {
    pub bulwark: bool,
    pub taunt_left: f64,
    pub cooldown: f64,
    pub can_taunt: bool,
}

/// What the panel says of a commander (feature 78): what his squad is
/// under — `world::SquadKind`'s code, `None` with no order — how many
/// are in it, minutes of the rally left, seconds until he may rally
/// again, and whether the level for one has been reached.
#[derive(Clone, Copy, PartialEq, Debug, Default)]
pub struct CommanderView {
    pub squad: Option<u32>,
    pub members: usize,
    pub rally_left: f64,
    pub cooldown: f64,
    pub can_rally: bool,
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
    /// One of the workbench's three slots.
    Bench(u32),
}

/// A cell of a container window: one slot of a class's grid by its id —
/// a piece of armour, a gun at its tier, or a stack of anything else kept
/// there — or, on the research desk, which is no grid, the key.
#[derive(Clone, Copy, PartialEq, Debug)]
enum HoldCell {
    Slot(Storage, u32),
    Stack(ResourceId),
}

/// Which fixture each job on the work list is about, so resting on a row
/// rings the place it happens — every fixture of that kind, since a row
/// names a kind: both hobs for cooking, every bay for the bay jobs. Hauling
/// is the crop carry, and where a haul *ends* is the thing worth pointing
/// at. Indexed by `work::Job` code; the test below pins the length.
const WORK_SPOTS: [u32; 9] = [
    SPOT_LOCKER,
    SPOT_BAY,
    SPOT_BAY,
    SPOT_FRIDGE,
    SPOT_HOB,
    SPOT_HELM,
    SPOT_BENCH,
    // A site is wherever it was laid out; nothing fixed to ring.
    SPOT_NOTHING,
    // A patient is wherever it fell, and a ring round a body would be a
    // selection; nothing fixed to ring.
    SPOT_NOTHING,
];

#[derive(Clone, Copy, PartialEq, Eq)]
enum Tab {
    Work,
    Management,
    /// What the selected crew member has on it: the armour slots and
    /// the weapon, with its numbers.
    Inventory,
    /// What the ship view is drawn to show over the ship: the plain deck,
    /// or the electricity.
    View,
    /// Parts to lay out for the crew to build, by category, with a search
    /// box over them; and the sites laid out so far. Only while the
    /// shipyard is on (feature 102).
    Build,
    /// The research tree: what the crew know, what the AI is on, and what
    /// a key would open.
    Research,
    /// The skills tree (feature 83): your own crew member's class, level
    /// by level, with the points a level's choice leaves to spend.
    Skills,
    /// The ship's facts, drawn by the screen: the tray lays out the tabs
    /// and leaves the body to the caller, since everything on it is the
    /// world's rather than the room's.
    Ship,
}

/// How wide [`CrewPanels::side`] draws itself once somebody is picked:
/// a part's label, its bar, its number and the trauma holding it at
/// nothing, in one row. The width is the panel's own rather than each
/// screen's, so the two agree and so a screen can leave the strip it
/// anchors in the right size — the game's is [`SIDE_W`] plus its
/// margins. Nothing is set while the panel is empty: "Click a Bim to
/// look at it." wants a box its own size.
pub const SIDE_W: f32 = 340.0;

/// The side panel's bars — health's, the parts' and the blood's. Wide,
/// because the health block round them is, and a column of bars that do
/// not line up reads as two panels rather than one.
const BAR_W: f32 = 140.0;
/// The health points beside the bar: the one number on the panel a
/// player reads in a fight, so it is the one number bigger than the
/// type round it.
const HEALTH_NUMBER: f32 = 16.0;
/// What is killing it, over the block that says how fast.
const PERIL_NAME: f32 = 15.0;
/// How long it has at that rate — the line the whole block exists for.
const PERIL_LEFT: f32 = 14.0;

/// The frame round the peril block: the panel's own, filled and edged —
/// in the cross's red for a body that is `grave`, in the caution colour
/// for one that is only bleeding. Nothing else on this panel is framed,
/// which is the point.
fn peril_frame(grave: bool) -> egui::Frame {
    let (fill, edge) = if grave {
        (
            egui::Color32::from_rgba_unmultiplied(56, 14, 12, 220),
            theme::DYING,
        )
    } else {
        (
            egui::Color32::from_rgba_unmultiplied(48, 38, 18, 220),
            theme::CAUTION,
        )
    };
    egui::Frame::new()
        .fill(fill)
        .stroke(egui::Stroke::new(1.0, edge))
        .corner_radius(4.0)
        .inner_margin(6.0)
}

/// What is taking a crew member down now. Worked out from the body, since
/// nothing in the room records a cause: `crew::perils`.
struct Peril {
    /// The headline, in the cross's red.
    cause: &'static str,
    /// How fast, spelled.
    rate: String,
    /// Game minutes at that rate, or `None` for a loss that is running
    /// but cannot reach the end on its own.
    minutes: Option<f32>,
    /// Where the loss is coming from, worst first.
    from: Vec<String>,
    /// What stops it.
    remedy: &'static str,
}

/// What is killing `w` right now, and how long it has at that rate: the
/// blood running out, which is every open wound at
/// [`health::BLEED_PER_WOUND`] an hour and every untreated trauma at its
/// own rate — `Health::update_held`'s own sum, so the number here is the
/// number the room is actually subtracting. A list, empty or of one, so
/// the block can go on drawing whatever else is ever found to kill a Bim.
fn perils(game: &Game, w: usize) -> Vec<Peril> {
    let mut out: Vec<Peril> = Vec::new();

    // The blood.
    let mut from: Vec<(f32, String)> = Vec::new();
    let mut traumatic = false;
    for (i, part) in health::Part::ALL.into_iter().enumerate() {
        if let Some(trauma) = game.trauma(w, part)
            && trauma.bleed() > 0.0
        {
            traumatic = true;
            from.push((
                trauma.bleed(),
                peril_from(trauma_name(trauma.code()), trauma.bleed()),
            ));
        }
        let wounds = game.wounds(w, part);
        if wounds > 0 {
            let rate = wounds as f32 * health::BLEED_PER_WOUND;
            from.push((rate, peril_from(&peril_wounds(SLOT_NAMES[i], wounds), rate)));
        }
    }
    let an_hour: f32 = from.iter().map(|(rate, _)| rate).sum();
    if an_hour > 0.0 {
        from.sort_by(|a, b| b.0.total_cmp(&a.0));
        out.push(Peril {
            cause: PERIL_BLEEDING,
            rate: peril_rate(an_hour),
            minutes: Some(game.blood(w) / an_hour * bims::clock::HOUR),
            from: from.into_iter().map(|(_, line)| line).collect(),
            remedy: if traumatic {
                PERIL_BLEED_MEDKIT
            } else {
                PERIL_BLEED_BANDAGE
            },
        });
    }
    out
}

/// What a dead crew member died of. Nothing records it, so this reads it
/// off the body the way `Health::is_dead` decides: no blood left is the
/// death a fight deals. The other — the head and the body both at nothing
/// with no trauma on either — no fight leaves, since a part a fight takes
/// to nothing always has a trauma on it; it is a body killed outright, a
/// crew member left behind or a probe's, and is said as nothing more.
fn death_line(game: &Game, w: usize) -> &'static str {
    if game.blood(w) <= 0.0 {
        DEATH_BLED_OUT
    } else {
        DEATH_OTHER
    }
}

/// How wide the Skills tree's detail column is: it stands to the *right*
/// of the tree rather than under it, so picking a slot can never shove
/// the tree up from under the pointer — the row is as tall as the tree
/// whatever is written beside it.
const DETAIL_WIDTH: f32 = 320.0;
/// The gap between the tree and the column beside it.
const SKILL_DETAIL_GAP: f32 = 16.0;

/// The Skills tree's geometry: the numbered gutter down the left, a
/// slot's box, and the gaps between them. A fixed level's box is two of
/// [`SKILL_BOX_W`] and the gap wide; a pick level's two are one each.
const SKILL_GUTTER: f32 = 32.0;
const SKILL_BOX_W: f32 = 165.0;
const SKILL_BOX_H: f32 = 30.0;
const SKILL_GAP_X: f32 = 10.0;
const SKILL_GAP_Y: f32 = 8.0;

/// The Skills tab's type: a few points over the panels' small text,
/// since a tree of seventy talents and what each is worth is read rather
/// than glanced at.
const SKILL_TEXT: f32 = 13.5;
/// The type in a slot's box.
const SKILL_BOX_TEXT: f32 = 13.0;
/// The picked slot's name over the column beside the tree.
const SKILL_NAME_TEXT: f32 = 17.0;

/// How big the tree comes out: ten levels down, two slots across.
fn skill_tree_size() -> egui::Vec2 {
    let levels = world::class::LEVELS as f32;
    egui::vec2(
        SKILL_GUTTER + 2.0 * SKILL_BOX_W + SKILL_GAP_X,
        levels * SKILL_BOX_H + (levels - 1.0) * SKILL_GAP_Y,
    )
}

/// One slot of the Skills tree (feature 83): a fixed level's own, which
/// comes with the level, or one of the two a pick level offers.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Slot {
    Fixed(u8),
    Pick(u8, world::Side),
}

impl Slot {
    /// The level it sits at.
    fn level(self) -> u8 {
        match self {
            Slot::Fixed(level) | Slot::Pick(level, _) => level,
        }
    }
}

/// What a slot of the Skills tree is, for the crew member looking at it:
/// what colours its box and what the line under the tree says.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum SkillState {
    /// Learnt — picked, or a fixed level reached.
    Learnt,
    /// The other side of a level already chosen at: gone for good.
    GivenUp,
    /// Reached, unchosen: a point away.
    Open,
    /// The level is not reached yet.
    Locked,
}

impl SkillState {
    /// What a slot is for the crew member the view is of: a fixed level
    /// is learnt once reached, and a pick level's side is learnt if it
    /// was the one chosen, given up if the other was, open if the level
    /// is reached and nothing has been chosen at it, and locked until
    /// the level is.
    fn of(view: &ClassView, slot: Slot) -> SkillState {
        let level = slot.level();
        match slot {
            Slot::Fixed(_) if level <= view.level => SkillState::Learnt,
            Slot::Fixed(_) => SkillState::Locked,
            Slot::Pick(_, side) => match taken_at(view, level) {
                Some(chosen) if chosen == side => SkillState::Learnt,
                Some(_) => SkillState::GivenUp,
                None if level <= view.level => SkillState::Open,
                None => SkillState::Locked,
            },
        }
    }
}

/// Which side was chosen at a level, if the level was chosen at.
fn taken_at(view: &ClassView, level: u8) -> Option<world::Side> {
    view.picks
        .iter()
        .find(|&&(l, _)| l == level)
        .map(|&(_, side)| side)
}

/// How many skill points are waiting: one for every pick level reached
/// and not chosen at (feature 83). Nought for a crew member with no
/// class, which has no levels to choose at.
fn points_left(view: &ClassView) -> usize {
    if view.class == world::Class::None {
        return 0;
    }
    (2..=view.level)
        .filter(|&l| world::class::is_pick_level(view.class, l) && taken_at(view, l).is_none())
        .count()
}

/// **What a slot is worth in numbers** — the line the column beside the
/// tree puts under a slot's name, so a choice between two talents is a
/// choice between two figures. The words and the arithmetic are
/// `names.rs`'s, off the rules crates' own constants: a fixed level's
/// [`level_numbers`], a pick level's [`talent_numbers`] for its side.
fn skill_numbers(class: world::Class, slot: Slot) -> String {
    match slot {
        Slot::Fixed(level) => level_numbers(class, level).unwrap_or_default(),
        Slot::Pick(level, side) => world::class::pick_at(class, level)
            .map(|(left, right)| {
                talent_numbers(match side {
                    world::Side::Left => left,
                    world::Side::Right => right,
                })
            })
            .unwrap_or_default(),
    }
}

/// A tool the pointer is holding, picked on the Build tab. One at a time,
/// and none is the ordinary pointer. There was a `Mine` beside it until
/// the money rework (feature 95) took the mining away.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Tool {
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
    /// And what the View tab shows over the ship. Read in and written
    /// back, like `clear`.
    pub overlay: Overlay,
    /// What the benches make, for the Management tab, and — going the
    /// other way — the targets changed there this frame, as `Order::Keep`s
    /// to be.
    pub crafts: Vec<Craft>,
    pub keep: Vec<(ResourceId, u32)>,
    /// The Build tab: whether the ship is at rest, which is the only time
    /// anything is laid out or built; what the pool has left after the
    /// sites already begun (feature 95); the sites laid out; and — going
    /// the other way — the sites to be called off.
    pub at_rest: bool,
    /// Whether anything is built onto the ship at all (feature 102,
    /// `World::shipyard_enabled`): off in a run, and the Build tab with it.
    pub shipyard: bool,
    pub free_money: economy::Money,
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
    /// By `Node` code: could be queued now — not known, on the AI or
    /// queued already, and nothing it needs still behind a key.
    pub queueable: [bool; NODES],
    /// What the AI is on, and how far, nought to one.
    pub current: Option<u32>,
    pub fraction: f64,
    /// What it goes onto next, in order, as `Node` codes.
    pub queue: Vec<u32>,
    /// A research desk aboard, and running.
    pub desk: bool,
    pub powered: bool,
    /// Keys in the crew's own desk, by tier: `keys[0]` tier one, `keys[1]`
    /// tier two.
    pub keys: [u32; 2],
    /// Whether each part may be laid out, by `PartKind` code.
    pub parts: Vec<bool>,
}

/// What the Research tab asked for.
#[derive(Clone, Copy, PartialEq, Debug, serde::Serialize, serde::Deserialize)]
pub enum ResearchOrder {
    /// Queue a node for the AI — `Command::Research`; it begins at once
    /// if the AI is idle.
    Begin(u32),
    /// Take the AI off what it is on — `Command::CancelResearch`.
    Cancel,
    /// Take a node off the queue — `Command::Dequeue`.
    Dequeue(u32),
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
    /// Whether the crew can afford it: its price is inside what the pool
    /// has left after the sites already begun (feature 95).
    pub affordable: bool,
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

/// What a menu row does when it is clicked: an order to the room, sent
/// through the seam the way the screen sends one, that walks the player's
/// Bim over to do it.
type Errand = CrewOrder;

/// One row of a fixture's menu: an errand for the Bim, or a window to
/// open — a body's "Loot", a mercenary's "Hire" — which is the panels' to
/// do rather than the room's.
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
        order: CrewOrder,
    ) -> Item {
        Item {
            label: label.into(),
            hint: hint.into(),
            disabled,
            run: Some(order),
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
    /// The Skills tab: the slot picked, whose details are under the tree
    /// (feature 83). Cleared when the class changes under it.
    skill_pick: Option<Slot>,
    /// The inventory pop-up: up from the Inventory key or a container
    /// being opened, until it is shut. A recruit does not open it — a
    /// fight is not the moment for a window over the deck.
    inventory_open: bool,
    /// The hold, as the screen last handed it over. See the module note.
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
    /// The body the Loot row just opened the window on, or the mercenary
    /// the Hire row did: the screen walks the Bim shown over to it and
    /// takes this. See the module note.
    pub walk: Option<LootSource>,
    /// The mercenary's terms the Hire window is over, as the screen last
    /// handed them; `None` while none is open, or once the body is no
    /// longer for hire — hired, or the rooms parted — which shuts it.
    pub terms: Option<Terms>,
    /// The Trade row was picked: the screen opens the trade window and
    /// takes this.
    pub trade_requested: bool,
    /// The workbench window's button was pressed: the screen sends
    /// `Order::Upgrade` and takes this.
    pub upgrade_requested: bool,
    /// The station desk's Take row was picked for this Bim: the screen
    /// sends the take once they are within reach, and takes this.
    pub key_requested: Option<u32>,
    /// A cell's pop-up, if one is up.
    cell_menu: Option<CellMenu>,
    /// What is within reach of the Bim shown, nearest first — a container
    /// or a body down, with a name for the strip — as the screen last
    /// handed it over (`Near`). Fresh every frame: the Bim is walking.
    pub nearby: Vec<Near>,
    /// The player's own class, as the screen last handed it over
    /// (feature 74): drawn under the health of their own crew member.
    pub class_view: Option<ClassView>,
    /// What the class section and the deployable rows asked for this
    /// frame, drained by the screen.
    pub deploy_orders: Vec<DeployOrder>,
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
    /// What the menus and the Management tab asked of the room this frame
    /// — `bims::order::CrewOrder`s — for the screen to send through the
    /// seam. Nothing in here reaches into the room itself, since the
    /// crew's positions are every player's to agree on (feature 59).
    pub crew_orders: Vec<CrewOrder>,
    /// The same, given with Shift held: to wait their turn behind what
    /// the crew member is on (feature 69) — `Game::order_later`,
    /// `Order::CrewLater` on the ship. Drained beside `crew_orders`.
    pub later_orders: Vec<CrewOrder>,
}

impl CrewPanels {
    pub fn new(player: usize, crew_count: u32) -> CrewPanels {
        CrewPanels {
            player,
            crew_count,
            tray_open: true,
            tab: Tab::Inventory,
            tool: None,
            diary_open: vec![false; crew_count as usize],
            sort: Some(Sort::Up),
            ringed: SPOT_NOTHING,
            wanted: SPOT_NOTHING,
            menu: None,
            search: String::new(),
            open_group: None,
            research_pick: None,
            skill_pick: None,
            inventory_open: false,
            hold: None,
            open: None,
            container_rect: None,
            body: None,
            walk: None,
            terms: None,
            trade_requested: false,
            upgrade_requested: false,
            key_requested: None,
            cell_menu: None,
            nearby: Vec::new(),
            class_view: None,
            deploy_orders: Vec::new(),
            locker_drag: None,
            pack_drag: None,
            keys: Keys::default(),
            orders: Vec::new(),
            crew_orders: Vec::new(),
            later_orders: Vec::new(),
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
    /// for a container, its window. A workstation is a container when its
    /// part keeps a class of goods (the armoury and the drug lab are
    /// lockers), a shelf of the ship's always is; the cold store answers
    /// no click and is opened from the nearby strip.
    pub fn open_menu(&mut self, fixture: u32, at: egui::Pos2, game: &mut Game) {
        let container = match fixture {
            HIT_BENCH if self.hold.is_some() => {
                let bench = game.hit_bench();
                let workbench = self.hold.as_ref().and_then(|h| h.bench).map(|b| b.index);
                PartKind::from_code(game.bench_part(bench))
                    .and_then(|kind| kind.def().capacity)
                    .map(|_| Container::Bench(bench))
                    // The workbench keeps no class of goods, but it has
                    // its three slots, and a window for them.
                    .or((workbench == Some(bench)).then_some(Container::Bench(bench)))
            }
            // The ship's own shelves are the hold's; the station's are not,
            // and a click on one opens nothing — what a station sells is
            // bought across its desk.
            HIT_SHELF if self.hold.is_some() => {
                let shelf = game.hit_shelf();
                let ashore = self
                    .hold
                    .as_ref()
                    .is_some_and(|h| h.station_shelves.contains(&shelf));
                if ashore {
                    return;
                }
                Some(Container::Shelf(shelf))
            }
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
            self.crew_orders.push(CrewOrder::SendTo {
                who: who as u32,
                x: spot.x,
                y: spot.y,
            });
        }
        self.open = Some(Open::Container(container));
        self.inventory_open = true;
        self.menu = None;
        self.cell_menu = None;
    }

    /// Open a container's window by name, the way a click on it would:
    /// `BIMS_ARMOURY=1` (or `armoury`) the first bench aboard whose part
    /// is an armoury, `storage` the first shelf, `workbench` the workbench
    /// with the slots. Nothing, on a ship without one.
    pub fn open_named(&mut self, game: &mut Game, what: &str) {
        let container = match what {
            "storage" => game
                .container_frame(Container::Shelf(0))
                .map(|_| Container::Shelf(0)),
            "workbench" => self
                .hold
                .as_ref()
                .and_then(|h| h.bench)
                .map(|b| Container::Bench(b.index)),
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
    /// to the screen (`walk`): where a body lies is the world's to say.
    pub fn open_loot(&mut self, source: LootSource) {
        self.open = Some(Open::Loot(source));
        self.walk = Some(source);
        self.body = None;
        self.inventory_open = true;
        self.menu = None;
        self.cell_menu = None;
    }

    /// The body under a plain left click, if a body is what was clicked:
    /// a dead crewmate (`HIT_BODY`) or one out cold (`HIT_BIM` with the
    /// Bim down). A click on one opens its inventory straight off — the
    /// Loot window, the way the row would — rather than a menu; the
    /// right-click keeps the rows. `None` for anything else.
    pub fn body_under_click(&self, game: &Game, fixture: u32) -> Option<LootSource> {
        match fixture {
            HIT_BODY => Some(LootSource::Crew(game.hit_body() as u32)),
            HIT_BIM => {
                let who = game.hit_bim();
                game.is_down(who).then_some(LootSource::Crew(who as u32))
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

    /// Build the rows for the fixture that was clicked. Nothing here is
    /// remote: every item walks the Bim over to do it by hand. Nor does
    /// anything wait for the Bim to be free — a new errand takes over, and
    /// what it displaced goes on the agenda to be finished afterwards.
    /// Every menu acts on the player's Bim; the other crew take no orders.
    fn items(&self, fixture: u32, game: &Game, name: &dyn Fn(u32) -> String) -> Vec<Item> {
        let who = self.player;
        let busy = game.is_busy(who);
        let takes_over: Option<&str> = busy.then_some("takes over — the rest waits its turn");
        let mut items = Vec::new();
        match fixture {
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
                    CrewOrder::Door {
                        who: who as u32,
                        door: door as u32,
                        order: if held {
                            door::Order::Close
                        } else {
                            door::Order::Open
                        },
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
                    CrewOrder::Door {
                        who: who as u32,
                        door: door as u32,
                        order: if locked {
                            door::Order::Unlock
                        } else {
                            door::Order::Lock
                        },
                    },
                ));
            }
            HIT_BIM => {
                // A body on the deck — the player's own, or a crewmate: a
                // row a part of it, saying what is open there, and the
                // player's Bim walks over and dresses the one picked. The
                // patient may be anybody alive; the hands are always the
                // player's.
                let patient = game.hit_bim();
                // Out of the helper's own pack (feature 87).
                let bandages = game.bandages_of(who);
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
                        CrewOrder::Bandage {
                            who: who as u32,
                            patient: patient as u32,
                            part,
                        },
                    ));
                }
                // And the lot at once (feature 87): the worst part now
                // and the rest queued behind it.
                let open = health::Part::ALL
                    .into_iter()
                    .any(|part| game.wounds(patient, part) > 0);
                items.push(Item::run(
                    BANDAGE_ALL_ROW.to_string(),
                    if bandages == 0 {
                        NO_BANDAGE.to_string()
                    } else if !open {
                        BANDAGE_ALL_WHOLE.to_string()
                    } else if out {
                        HELPER_OUT.to_string()
                    } else if patient_out {
                        PATIENT_OUT.to_string()
                    } else {
                        BANDAGE_ALL_HINT.to_string()
                    },
                    bandages == 0 || out || patient_out || !open,
                    CrewOrder::BandageAll {
                        who: who as u32,
                        patient: patient as u32,
                    },
                ));
                items.push(Item::note("Bandages", format!("{bandages} in the pack")));
                // A part at nothing: the dying state on it, and a medkit
                // in somebody else's hands the only way out. The player's
                // Bim treats a crewmate; for the player's own, the nearest
                // crewmate that is free is sent, since nobody treats
                // their own.
                let dying: Vec<(usize, health::Part, bims::health::Trauma)> = health::Part::ALL
                    .into_iter()
                    .enumerate()
                    .filter_map(|(i, part)| game.trauma(patient, part).map(|t| (i, part, t)))
                    .collect();
                if !dying.is_empty() {
                    let helper = treat_helper(game, who, patient);
                    // Out of the helper's own pack: a medkit is a charge.
                    let medkits = helper.map_or(0, |h| kits_to_hand(game, h));
                    for (i, part, trauma) in dying {
                        let can = helper.is_some() && medkits > 0 && !patient_out;
                        let hint = if helper.is_some() && medkits == 0 {
                            NO_MEDKIT.to_string()
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
                            CrewOrder::Treat {
                                // Disabled with no helper, so the row is
                                // never run with the placeholder.
                                who: helper.unwrap_or(who) as u32,
                                patient: patient as u32,
                                part,
                            },
                        ));
                    }
                    let whose = match helper {
                        Some(h) if h != who => format!("{medkits} in {}'s pack", name(h as u32)),
                        _ => format!("{medkits} in the pack"),
                    };
                    items.push(Item::note("Medkits", whose));
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
            // A dead crew member: nothing to dress, and the one row is the
            // Loot window.
            HIT_BODY => {
                let body = game.hit_body() as u32;
                items.push(Item::opens(
                    LOOT_ROW,
                    format!("{} — everything on the body", name(body)),
                    Open::Loot(LootSource::Crew(body)),
                ));
            }
            // One of the station's people, on its feet and hailable: a
            // mercenary for hire. What it asks is the world's to say — the
            // window reads it. A resident down has no row: what it had on
            // it is its own.
            HIT_VISITOR => {
                let body = game.hit_visitor() as u32;
                if !game.visitor_down(body as usize) {
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
                let key = self.hold.as_ref().map_or(0, |h| h.station_key);
                if key > 0 {
                    items.push(Item::opens(key_row(key), KEY_ROW_HINT, Open::Key(desk)));
                } else {
                    items.push(Item::note(KEY_ROW, NO_KEY_ROW_HINT.into()));
                }
            }
            _ => {}
        }
        // While there is something to wait behind, a word about Shift: a
        // row given with it waits its turn (feature 69). Only under rows
        // that are errands — the Loot and Open rows are windows.
        let something_on = busy || game.agenda_len(who) > 0 || game.is_walking(who);
        if something_on && items.iter().any(|item| item.run.is_some()) {
            items.push(Item::note(SHIFT_LATER, SHIFT_LATER_HINT.into()));
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
        // Shift, as the row was clicked: the errand waits its turn rather
        // than taking over (feature 69). Asked of the context outside any
        // input closure of its own.
        let later = ctx.input(|i| i.modifiers.shift);
        if let Some(i) = chosen {
            let item = items.into_iter().nth(i).unwrap();
            self.menu = None;
            match item.opens {
                Some(Open::Container(container)) => self.open_container(game, container),
                Some(Open::Loot(source)) => self.open_loot(source),
                Some(Open::Hire(resident)) => self.open_hire(resident),
                Some(Open::Trade(desk)) => {
                    if let Some(spot) = game.desk_spot(desk) {
                        let who = self.inventory_who(game) as u32;
                        self.crew_orders.push(CrewOrder::SendTo {
                            who,
                            x: spot.x,
                            y: spot.y,
                        });
                    }
                    self.trade_requested = true;
                }
                Some(Open::Key(desk)) => {
                    let who = self.inventory_who(game);
                    if let Some(spot) = game.research_spot(desk) {
                        self.crew_orders.push(CrewOrder::SendTo {
                            who: who as u32,
                            x: spot.x,
                            y: spot.y,
                        });
                    }
                    self.key_requested = Some(who as u32);
                }
                Some(open @ Open::PackUp(_)) => self.show(open),
                None => {
                    if let Some(order) = item.run {
                        if later {
                            self.later_orders.push(order);
                        } else {
                            self.crew_orders.push(order);
                        }
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

    /// The class section under the health: the class and the level, the
    /// pick waiting as two buttons with what each does behind a `?`, the
    /// talents learnt, and — until the first undock — the class picker.
    fn class_section(&mut self, ui: &mut egui::Ui, view: &ClassView) {
        ui.add_space(4.0);
        if view.can_change {
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new(BIM_CLASS).color(theme::MUTED));
                for class in world::Class::ALL {
                    if theme::toggle(ui, class == view.class, class_name(class)).clicked()
                        && class != view.class
                    {
                        self.deploy_orders.push(DeployOrder::SetClass(class));
                    }
                }
                theme::question_mark(ui, BIM_CLASS_NOTE);
            });
        }
        if view.class == world::Class::None {
            if !view.can_change {
                ui.label(
                    egui::RichText::new(class_name(view.class))
                        .small()
                        .color(theme::MUTED),
                );
            }
            return;
        }
        ui.label(
            egui::RichText::new(class_line(view.class, view.level, view.to_next))
                .small()
                .color(theme::INK),
        );
        if let Some(soldier) = view.soldier {
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new(grenades_line(soldier.grenades, soldier.cooldown))
                        .small()
                        .color(theme::INK),
                );
                let (word, color) = if soldier.braced {
                    (BRACED, theme::CAUTION)
                } else {
                    (STAND_EASY, theme::MUTED)
                };
                ui.label(egui::RichText::new(word).small().color(color));
                theme::question_mark(ui, BRACED_TIP);
            });
        }
        if let Some(medic) = &view.medic {
            ui.horizontal(|ui| {
                let (words, color) = if medic.patients.is_empty() {
                    (BEAM_OFF.to_string(), theme::MUTED)
                } else {
                    (beam_line(&medic.patients), theme::YOURS)
                };
                ui.label(egui::RichText::new(words).small().color(color));
                theme::question_mark(ui, BEAM_TIP);
            });
            ui.horizontal(|ui| {
                let charged = medic.can_surge && medic.charge >= 1.0;
                ui.label(
                    egui::RichText::new(surge_line(medic.charge, medic.can_surge))
                        .small()
                        .color(if charged {
                            theme::CAUTION
                        } else {
                            theme::MUTED
                        }),
                );
                if medic.surging {
                    ui.label(egui::RichText::new(SURGING).small().color(theme::YOURS));
                }
            });
        }
        if let Some(tank) = view.tank {
            ui.horizontal(|ui| {
                let (word, color) = if tank.bulwark {
                    (WALL_UP, theme::CAUTION)
                } else {
                    (WALL_DOWN, theme::MUTED)
                };
                ui.label(egui::RichText::new(word).small().color(color));
                theme::question_mark(ui, WALL_TIP);
            });
            let taunting = tank.taunt_left > 0.0;
            ui.label(
                egui::RichText::new(taunt_line(tank.taunt_left, tank.cooldown, tank.can_taunt))
                    .small()
                    .color(if taunting { theme::WARN } else { theme::MUTED }),
            );
        }
        if let Some(commander) = view.commander {
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new(squad_line(commander.squad, commander.members))
                        .small()
                        .color(if commander.squad.is_some() {
                            theme::CAUTION
                        } else {
                            theme::MUTED
                        }),
                );
                theme::question_mark(ui, SQUAD_TIP);
            });
            let rallying = commander.rally_left > 0.0;
            ui.label(
                egui::RichText::new(rally_line(
                    commander.rally_left,
                    commander.cooldown,
                    commander.can_rally,
                ))
                .small()
                .color(if rallying { theme::WARN } else { theme::MUTED }),
            );
        }
        if let Some((level, left, right)) = view.pending {
            ui.label(
                egui::RichText::new(format!("{PICK_PENDING} level {level}"))
                    .small()
                    .color(theme::CAUTION),
            );
            ui.horizontal(|ui| {
                for (talent, side) in [(left, world::Side::Left), (right, world::Side::Right)] {
                    if ui.button(talent_name(talent)).clicked() {
                        self.deploy_orders.push(DeployOrder::Pick {
                            level: level as u32,
                            side,
                        });
                    }
                    theme::question_mark(ui, talent_tip(talent));
                }
            });
        }
        if !view.talents.is_empty() {
            let learnt: Vec<&str> = view.talents.iter().map(|t| talent_name(*t)).collect();
            ui.label(
                egui::RichText::new(learnt.join(" · "))
                    .small()
                    .color(theme::MUTED),
            );
        }
    }

    /// The selected crew member's panels: the health bars, the health lines
    /// and the character sheet. One at a time, and only when somebody is
    /// picked: the right-hand side answers "who am I looking at", not
    /// "what is everybody up to". `true` when something was drawn.
    pub fn side(
        &mut self,
        ui: &mut egui::Ui,
        game: &mut Game,
        name: &dyn Fn(u32) -> String,
    ) -> bool {
        let Some(who) =
            (0..self.crew_count).find(|&w| game.is_selected(w as usize, self.player as u32))
        else {
            return false;
        };
        let w = who as usize;
        let alive = game.is_alive(w);
        ui.set_min_width(SIDE_W);
        self.who_header(ui, who, name);

        // How it is bearing up. The armour worn adds its health to the
        // body's: the blue on the end of the green is what the pieces
        // still have, and the number reads the two apart.
        //
        // This whole block is drawn big on purpose: in a fight it is the
        // only part of the panel anybody looks at, and the question it has
        // to answer at a glance is not "how much health" but "what is
        // killing it and how long has it got" — which is the peril block
        // under the bar.
        ui.add_space(6.0);
        let points = game.health(w);
        let armour = if alive { game.armour_health(w) } else { 0.0 };
        let perils = if alive { perils(game, w) } else { Vec::new() };
        let hurt = !alive || !perils.is_empty();
        ui.horizontal(|ui| {
            if who == 0 {
                theme::asks(ui, "Health", HEALTH_TIP);
            } else {
                ui.label("Health");
            }
            let total = health::MAX_HEALTH + armour;
            theme::health_bar(
                ui,
                BAR_W,
                points / total,
                armour / total,
                if hurt { theme::BAD } else { theme::ACCENT },
                theme::ARMOUR,
            );
            ui.label(
                egui::RichText::new(format!("{}", points.round()))
                    .size(HEALTH_NUMBER)
                    .strong()
                    .color(if hurt { theme::BAD } else { theme::INK }),
            );
            if armour > 0.0 {
                ui.label(
                    egui::RichText::new(format!("+{}", armour.round()))
                        .size(HEALTH_NUMBER)
                        .color(theme::ARMOUR),
                );
            }
        });

        // What is taking it down, and how long it has at that rate. The
        // one thing on this panel that is framed: it is a warning, and a
        // warning that looks like the rows around it is not one.
        //
        // Graded by what a player would have to do about it. A dying
        // state is the red block, the one the cross on the deck marks, and
        // wants a medkit; a body that is only losing blood through wounds
        // a bandage closes gets the same block and the same countdown in
        // the caution colour, since a scratch that would empty it in ten
        // hours is worth a number and not a fright.
        let grave = alive && game.is_dying(w);
        let peril_ink = if grave { theme::DYING } else { theme::CAUTION };
        if alive && !perils.is_empty() {
            ui.add_space(3.0);
            peril_frame(grave).show(ui, |ui| {
                ui.set_min_width(BAR_W + 60.0);
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new(if grave { PERIL_HEAD } else { PERIL_HEAD_HURT })
                            .small()
                            .strong()
                            .color(peril_ink),
                    );
                    theme::question_mark(ui, PERIL_TIP);
                });
                for peril in &perils {
                    ui.add_space(2.0);
                    ui.horizontal(|ui| {
                        ui.label(
                            egui::RichText::new(peril.cause)
                                .size(PERIL_NAME)
                                .strong()
                                .color(peril_ink),
                        );
                        ui.label(
                            egui::RichText::new(&peril.rate)
                                .small()
                                .color(theme::CAUTION),
                        );
                    });
                    if let Some(minutes) = peril.minutes {
                        ui.label(
                            egui::RichText::new(peril_left(&span_text(minutes)))
                                .size(PERIL_LEFT)
                                .strong()
                                .color(if grave { theme::WARN } else { theme::CAUTION }),
                        );
                    }
                    for from in &peril.from {
                        ui.label(egui::RichText::new(from).small().color(theme::INK));
                    }
                    ui.label(
                        egui::RichText::new(peril.remedy)
                            .small()
                            .color(theme::MUTED),
                    );
                }
            });
            ui.add_space(3.0);
        } else if alive && game.is_dying(w) {
            // A dying state that is not bleeding — a concussion, broken
            // ribs, a knee. It still wants a medkit, but nothing is
            // counting down, and that is worth saying outright rather
            // than leaving a player to guess from a bar that is not
            // moving.
            ui.add_space(3.0);
            peril_frame(true).show(ui, |ui| {
                ui.set_min_width(BAR_W + 60.0);
                for part in health::Part::ALL {
                    if let Some(trauma) = game.trauma(w, part) {
                        ui.label(
                            egui::RichText::new(trauma_name(trauma.code()))
                                .size(PERIL_NAME)
                                .strong()
                                .color(theme::DYING),
                        );
                    }
                }
                ui.label(
                    egui::RichText::new(PERIL_STABLE)
                        .small()
                        .color(theme::MUTED),
                );
            });
            ui.add_space(3.0);
        }

        // What the bar is made of: the head, the body and the legs, a bar
        // each, and the blood under them. Drawn for the dead too — a body
        // with its head at nothing says how it died — and a part held at
        // nothing by an untreated trauma is named in the red of the cross
        // over its head on the deck.
        egui::Grid::new(("body", who))
            .num_columns(4)
            .spacing([8.0, 3.0])
            .show(ui, |ui| {
                for (i, part) in health::Part::ALL.into_iter().enumerate() {
                    let left = game.part_health(w, part);
                    let bonus = if alive { game.part_bonus(w, part) } else { 0.0 };
                    let bleeding = alive && game.wounds(w, part) > 0;
                    let trauma = if alive { game.trauma(w, part) } else { None };
                    ui.label(
                        egui::RichText::new(SLOT_NAMES[i]).color(if trauma.is_some() {
                            theme::DYING
                        } else if bleeding {
                            theme::CAUTION
                        } else {
                            theme::MUTED
                        }),
                    );
                    let total = part.max() + bonus;
                    theme::two_tone_bar(
                        ui,
                        BAR_W,
                        left / total,
                        bonus / total,
                        if trauma.is_some() || bleeding {
                            theme::BAD
                        } else {
                            theme::ACCENT
                        },
                        theme::ARMOUR,
                    );
                    ui.label(
                        egui::RichText::new(if bonus > 0.0 {
                            format!("{} + {}", left.round(), bonus.round())
                        } else {
                            format!("{}", left.round())
                        })
                        .color(theme::MUTED),
                    );
                    // The trauma's own name beside the part it holds at
                    // nothing, so the bar at zero and the reason it is
                    // there are one row rather than two places.
                    if let Some(trauma) = trauma {
                        ui.label(
                            egui::RichText::new(trauma_name(trauma.code()))
                                .small()
                                .color(theme::DYING),
                        );
                    }
                    ui.end_row();
                }
                // Red from `SLOWED_AT` down, which is where the Bim starts
                // to slow — the number alone does not say that, and half
                // of it again is where it goes out cold (feature 89).
                let blood = game.blood(w) / health::MAX_BLOOD;
                let low = blood < health::SLOWED_AT;
                ui.label(egui::RichText::new("Blood").color(if low {
                    theme::BAD
                } else {
                    theme::MUTED
                }));
                theme::bar(
                    ui,
                    BAR_W,
                    blood,
                    if low { theme::BAD } else { theme::ACCENT },
                );
                ui.label(
                    egui::RichText::new(format!("{}%", (blood * 100.0).round()))
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
        // The open wounds and the traumas themselves are the peril block's
        // now — it says how fast each of them is emptying the Bim, which
        // is what a count of wounds was standing in for.
        if alive && game.is_unconscious(w) {
            ui.label(egui::RichText::new("Out cold").small().color(theme::BAD));
        }
        // What each untreated trauma is *doing* to it besides the blood —
        // the slower walk, the slower work — under the block that says
        // what it costs. Then what treated traumas have left behind, and
        // for how long.
        if alive {
            for part in health::Part::ALL {
                if let Some(trauma) = game.trauma(w, part) {
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
        if !alive {
            // The dead say what of, in the red the cross over them was:
            // a bar at nothing is not an answer, and "has died" on its
            // own leaves a player scrolling back through the log for
            // one.
            ui.label(
                egui::RichText::new(format!("{} has died.", name(who)))
                    .size(PERIL_NAME)
                    .strong()
                    .color(theme::DYING),
            );
            ui.label(
                egui::RichText::new(death_line(game, w))
                    .small()
                    .color(theme::MUTED),
            );
        }
        // The class (feature 74): the player's own crew member's, with its
        // level, what the next wants, the pick waiting and the talents
        // learnt. Only their own: a class is a slot's.
        if who == self.player as u32
            && let Some(view) = self.class_view.clone()
        {
            self.class_section(ui, &view);
        }

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

    /// Bottom-left, the tabs — the work list, the management row and the
    /// inventory, then the view to pick, the parts to build while there is
    /// a shipyard, the research, the skills and the ship — and it folds
    /// away. Docked, a Station button sits with the tabs: not a tab but a
    /// press, which the screen answers with the trade window. True when the
    /// Ship tab is open, whose body the caller draws under the tabs itself.
    pub fn tray(
        &mut self,
        ui: &mut egui::Ui,
        game: &mut Game,
        mut actions: Option<&mut Actions>,
        name: &dyn Fn(u32) -> String,
    ) -> bool {
        let mut tabs = vec![
            (Tab::Work, "Work"),
            (Tab::Management, "Management"),
            (Tab::Inventory, "Inventory"),
        ];
        if let Some(actions) = actions.as_deref() {
            tabs.push((Tab::View, "View"));
            // Nothing is built onto the ship in a run (feature 102).
            if actions.shipyard {
                tabs.push((Tab::Build, "Build"));
            } else if self.tab == Tab::Build {
                self.tab = Tab::View;
            }
            tabs.push((Tab::Research, "Research"));
            tabs.push((Tab::Skills, SKILLS));
            tabs.push((Tab::Ship, "Ship"));
        } else if matches!(
            self.tab,
            Tab::View | Tab::Build | Tab::Research | Tab::Skills | Tab::Ship
        ) {
            self.tab = Tab::Inventory;
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
            Tab::Work => self.work(ui, game),
            Tab::Management => self.management(ui, game, actions),
            Tab::Inventory => {
                let who = self.inventory_who(game);
                self.inventory(ui, game, who, name);
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
            Tab::Skills => self.skills(ui),
        }
        false
    }

    /// Open the tray on the Skills tab: what a level of the player's own
    /// (feature 83) does, rather than a window over the deck.
    pub fn show_skills(&mut self) {
        self.tab = Tab::Skills;
        self.tray_open = true;
    }

    // --- the inventory --------------------------------------------------------

    /// Whose inventory the tab, the pop-up and the container windows are
    /// about: the selected crew member, or the one the player steers when
    /// nobody is picked.
    pub fn inventory_who(&self, game: &Game) -> usize {
        (0..self.crew_count)
            .map(|w| w as usize)
            .find(|&w| game.is_selected(w, self.player as u32))
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
        // The hands are the player's whoever is shown, so the dressing
        // comes out of the player's own pack (feature 87).
        let bandages = game.bandages_of(self.player);
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
                } else if game.is_recruited(self.player as u32) && who == self.player {
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
                                // Out of the helper's own pack: a medkit is a charge.
                                let medkits = helper.map_or(0, |h| kits_to_hand(game, h));
                                let can = helper.is_some() && medkits > 0 && !patient_out;
                                let hint = if helper.is_some() && medkits == 0 {
                                    NO_MEDKIT.to_string()
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
                // Ten by five, laid out like the lockers: a drag moves a
                // thing, Turn turns it, through the seam as a repack.
                let (things, heads) = pack_things(&gear);
                let mut drag = self.pack_drag;
                let fits = |i: usize, x: usize, y: usize, turned: bool| -> bool {
                    heads.get(i).is_some_and(|&head| {
                        gear.pack[head].is_some_and(|item| {
                            let to = y * PACK_COLS + x;
                            // Onto a box of the same thing with room in
                            // it, or into a run of free cells (feature
                            // 87) — the rule `Gear::rearrange` applies.
                            (to != head && gear.room_in(to, item) > 0)
                                || gear.fits_turned(to, item, turned, Some(head))
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
                egui::RichText::new(format!("Bandages to hand: {bandages}"))
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
            self.crew_orders.push(CrewOrder::Bandage {
                who: self.player as u32,
                patient: who as u32,
                part,
            });
        }
        if let Some((helper, part)) = treat {
            self.crew_orders.push(CrewOrder::Treat {
                who: helper as u32,
                patient: who as u32,
                part,
            });
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
            // With the workbench's window up the quick move is onto the
            // bench; else into the hold.
            if self.bench_window_open() && self.can_put_on_bench(item).is_ok() {
                self.orders.push(GearOrder::StowOnBench {
                    who: who as u32,
                    cell: i as u32,
                });
            } else if !self.bench_window_open() && self.can_stow(item).is_ok() {
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
        // Not a window: the engineer's pack-up row acts and leaves what is
        // up as it is.
        if let Open::PackUp(id) = open {
            self.deploy_orders.push(DeployOrder::PackUp(id));
            return;
        }
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

    /// The pop-up the Inventory key or a container window opens: the
    /// inventory of the Bim shown, in a window of its own, until it is
    /// shut. Beside the container window while one is up, else at the
    /// top of the screen. Call once a frame after the tray and after
    /// [`CrewPanels::container_window`].
    pub fn inventory_window(
        &mut self,
        ctx: &egui::Context,
        game: &mut Game,
        name: &dyn Fn(u32) -> String,
    ) {
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
            .frame(crate::theme::panel_frame());
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
        // The workbench is a container of its own kind: three slots, not
        // a class of the hold.
        if let Container::Bench(i) = container
            && let Some(view) = self.hold.as_ref().and_then(|h| h.bench)
            && view.index == i
        {
            self.locker_drag = None;
            self.bench_window(ctx, game, view, name);
            return;
        }
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
            .frame(crate::theme::panel_frame())
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

    /// The workbench's window: its two input slots, the button — or the
    /// hours done while the day's work is on — and the output slot, each
    /// slot a cell drawn the way a pack's are, a tier-two thing on blue and
    /// a tier-three on gold. Ctrl-click a slot to take what is in it into
    /// the pack of the Bim shown; right-click for the row. What goes *on*
    /// is the pack's side: Ctrl-click a thing there while this window is
    /// up, or its Bench row. The button asks the world (`can_upgrade`)
    /// and is greyed with the reason. Shut by its cross, by Escape, or by
    /// the container going away under it.
    fn bench_window(
        &mut self,
        ctx: &egui::Context,
        game: &Game,
        view: BenchView,
        name: &dyn Fn(u32) -> String,
    ) {
        let who = self.inventory_who(game);
        let cells: Vec<Option<Cell>> = view
            .bench
            .slots
            .iter()
            .map(|slot| slot.map(|item| cell_of(item, 1)))
            .collect();
        let inputs: Vec<Option<Cell>> = cells
            .iter()
            .take(world::Workbench::OUT)
            .map(|c| {
                c.as_ref()
                    .map(|c| Cell::new(c.item, c.count, c.tip.clone()))
            })
            .collect();
        let output: Vec<Option<Cell>> = cells
            .iter()
            .skip(world::Workbench::OUT)
            .map(|c| {
                c.as_ref()
                    .map(|c| Cell::new(c.item, c.count, c.tip.clone()))
            })
            .collect();
        let mut open = true;
        let mut picked_in = grid::Picked::default();
        let mut picked_out = grid::Picked::default();
        let mut pressed = false;
        let mut strip = None;
        let (nearby, showing) = (self.nearby.clone(), self.open);
        let work = view.bench.work;
        let response = egui::Window::new(BENCH_WINDOW)
            .id(egui::Id::new("container-window"))
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::LEFT_TOP, CONTAINER_AT)
            .frame(crate::theme::panel_frame())
            .show(ctx, |ui| {
                // Wide enough for the hint's line: with nothing wider than
                // the three cells the window would wrap it to nothing.
                ui.set_min_width(300.0);
                strip = nearby_strip(ui, &nearby, showing);
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new("Two of a kind in, one out a tier up")
                            .small()
                            .color(theme::MUTED),
                    );
                    theme::question_mark(ui, BENCH_TIP);
                });
                ui.horizontal(|ui| {
                    picked_in = grid::grid(ui, 2, 1, PACK_CELL, &inputs);
                    ui.add_space(6.0);
                    ui.vertical(|ui| {
                        ui.set_min_width(96.0);
                        match work {
                            Some(upgrade) => {
                                let done = upgrade.done.min(world::data::UPGRADE_SESSIONS);
                                let of = world::data::UPGRADE_SESSIONS;
                                ui.label(
                                    egui::RichText::new(bench_work_line(done, of)).small(),
                                );
                                theme::bar(ui, 90.0, done as f32 / of.max(1) as f32, theme::ACCENT);
                            }
                            None => {
                                let (ok, why) = match view.upgrade {
                                    Ok(()) => (true, String::new()),
                                    Err(why) => (false, refusal(why).to_string()),
                                };
                                let button = ui.add_enabled(ok, egui::Button::new(UPGRADE_BUTTON));
                                if ok && button.clicked() {
                                    pressed = true;
                                }
                                if !ok && !why.is_empty() {
                                    button.on_disabled_hover_text(why);
                                }
                                // The armourer's repair (feature 74): an
                                // engineer with the talent mends the damaged
                                // piece in the first slot, greyed with the
                                // world's own reason.
                                if let Some(repair) =
                                    self.class_view.as_ref().and_then(|v| v.repair)
                                {
                                    let (ok, why) = match repair {
                                        Ok(()) => (true, String::new()),
                                        Err(why) => (false, refusal(why).to_string()),
                                    };
                                    let button = ui
                                        .add_enabled(ok, egui::Button::new(REPAIR))
                                        .on_hover_text(REPAIR_TIP);
                                    if ok && button.clicked() {
                                        self.deploy_orders.push(DeployOrder::Repair);
                                    }
                                    if !ok && !why.is_empty() {
                                        button.on_disabled_hover_text(why);
                                    }
                                }
                            }
                        }
                    });
                    ui.add_space(6.0);
                    picked_out = grid::grid(ui, 1, 1, PACK_CELL, &output);
                });
                let hint = if !view.reach {
                    format!(
                        "{} is not within reach — walk over first; clicking the bench sends the Bim",
                        name(who as u32)
                    )
                } else {
                    format!(
                        "Ctrl-click a thing in the pack to put it on the bench · Ctrl-click a slot to take it into {}'s pack · right-click for the rows",
                        name(who as u32)
                    )
                };
                ui.add(egui::Label::new(egui::RichText::new(hint).small().color(theme::MUTED)).wrap());
            });
        if let Some(response) = response {
            self.container_rect = Some(response.response.rect);
        }
        if !open {
            self.open = None;
        }
        if pressed {
            self.upgrade_requested = true;
        }
        // The pointer on a slot: a right-click is the row, a ctrl-click the
        // quick take — or the row, when the take cannot go.
        let out = world::Workbench::OUT;
        let right = picked_in
            .right_clicked
            .or(picked_out.right_clicked.map(|(i, at)| (i + out, at)));
        let quick = picked_in
            .ctrl_clicked
            .or(picked_out.ctrl_clicked.map(|(i, at)| (i + out, at)));
        if let Some((i, at)) = right {
            self.cell_menu = Some(CellMenu {
                at,
                from: Source::Bench(i as u32),
                who,
                fresh: true,
            });
        }
        if let Some((i, at)) = quick {
            if self.can_take_off_bench(game, who, i).is_ok() {
                self.orders.push(GearOrder::Fetch {
                    who: who as u32,
                    kind: FetchKind::Bench { slot: i as u32 },
                });
            } else {
                self.cell_menu = Some(CellMenu {
                    at,
                    from: Source::Bench(i as u32),
                    who,
                    fresh: true,
                });
            }
        }
        self.follow_strip(strip);
    }

    /// Whether what is in a workbench slot can come into `who`'s pack now,
    /// or why not: something there, the bench not at work on it, reach,
    /// and a free cell. The world checks the same again when the command
    /// lands.
    fn can_take_off_bench(&self, game: &Game, who: usize, slot: usize) -> Result<(), String> {
        let Some(view) = self.hold.as_ref().and_then(|h| h.bench) else {
            return Err("there is no workbench".into());
        };
        if view.bench.slots.get(slot).copied().flatten().is_none() {
            return Err("nothing there".into());
        }
        if view.bench.work.is_some() && slot != world::Workbench::OUT {
            return Err(refusal(world::Refusal::BenchBusy).into());
        }
        if !view.reach {
            return Err(REACH_HINT.into());
        }
        if game.gear(who).free_cell().is_none() {
            return Err("the pack is full".into());
        }
        Ok(())
    }

    /// Whether a thing in the pack can go onto the workbench now, or why
    /// not, in the words the Bench row shows: the window up, the bench
    /// taking it (`Workbench::takes`, the world's rule), and reach.
    fn can_put_on_bench(&self, item: PackItem) -> Result<(), String> {
        let Some(view) = self.hold.as_ref().and_then(|h| h.bench) else {
            return Err("there is no workbench".into());
        };
        if let PackItem::Armour(piece) = item
            && piece.broken()
        {
            return Err("broken — the bench makes nothing of it; discard it".into());
        }
        if let Err(why) = view.bench.takes(item) {
            return Err(refusal(why).into());
        }
        if !view.reach {
            return Err(REACH_HINT.into());
        }
        Ok(())
    }

    /// Whether the workbench's window is the one up.
    fn bench_window_open(&self) -> bool {
        match (self.open, self.hold.as_ref().and_then(|h| h.bench)) {
            (Some(Open::Container(Container::Bench(i))), Some(view)) => view.index == i,
            _ => false,
        }
    }

    /// The Hire window, if a mercenary is open: whose, what it carries —
    /// the weapon and every piece worn, which is what the fee is — and a
    /// month's fee, with the button that sends the hire through the seam
    /// (`GearOrder::Hire`). Greyed, with the reason, while the Bim shown
    /// is out of reach (the row walked it over) or the money is short.
    /// Shut by its cross, by Escape, by the hire
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
            .frame(crate::theme::panel_frame())
            .show(ctx, |ui| {
                // The trade first, where there is one to name (feature
                // 86): a field medic is hired for what it does and not
                // for the gun it carries, and the premium on the month
                // below is that and nothing else.
                if terms.medic {
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new(FIELD_MEDIC).strong().color(theme::HEAL));
                        theme::question_mark(ui, FIELD_MEDIC_TIP);
                    });
                    ui.add_space(4.0);
                }
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
    /// it. Call once a frame after the tray, after
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
        // A crewmate's body, and only a crewmate's: what one of the
        // station's people had on it is its own.
        let LootSource::Crew(whose) = source else {
            self.open = None;
            return;
        };
        let who = self.inventory_who(game);
        let whose = name(whose);
        let cells: Vec<Option<Cell>> = body.cells[PACK_CELLS..]
            .iter()
            .enumerate()
            .map(|(i, slot)| slot.map(|item| cell_of(item, body.counts[PACK_CELLS + i].max(1))))
            .collect();
        let worn_cells = cells.as_slice();
        let (things, heads) = laid_things(
            &body.cells[..PACK_CELLS],
            &body.turned,
            &body.counts[..PACK_CELLS],
        );
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
            .frame(crate::theme::panel_frame())
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
                    // A box of dressings binds wounds (feature 87): the
                    // worst part now and the rest queued behind it, out
                    // of this very pack, which is what a Bim that has
                    // run out of a fight reaches for by itself.
                    PackItem::Stack(code) if code == bims::combat::BANDAGE_CODE => {
                        let open = health::Part::ALL
                            .into_iter()
                            .any(|part| alive && game.wounds(who, part) > 0);
                        rows.push((
                            theme::Row::new(
                                BANDAGE_ALL_ROW,
                                if !alive {
                                    "not any more"
                                } else if open {
                                    BANDAGE_ALL_HINT
                                } else {
                                    BANDAGE_ALL_WHOLE
                                },
                                !alive || !open,
                            ),
                            GearOrder::BandageAll { who: who32 },
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
                // And onto the workbench, for a gun or a piece, while the
                // ship has one.
                if self.hold.as_ref().is_some_and(|h| h.bench.is_some())
                    && matches!(item, PackItem::Armour(_) | PackItem::Weapon(_))
                {
                    let (hint, disabled) = match self.can_put_on_bench(item) {
                        Ok(()) => ("into a slot on the workbench".to_string(), false),
                        Err(why) => (why, true),
                    };
                    rows.push((
                        theme::Row::new("Bench", hint, disabled),
                        GearOrder::StowOnBench {
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
            Source::Bench(slot) => {
                let Some(view) = self.hold.as_ref().and_then(|h| h.bench) else {
                    return rows;
                };
                if view
                    .bench
                    .slots
                    .get(slot as usize)
                    .copied()
                    .flatten()
                    .is_none()
                {
                    return rows;
                }
                let (hint, disabled) = match self.can_take_off_bench(game, who, slot as usize) {
                    Ok(()) => (format!("into {}'s pack", name(who as u32)), false),
                    Err(why) => (why, true),
                };
                rows.push((
                    theme::Row::new("Take", hint, disabled),
                    GearOrder::Fetch {
                        who: who as u32,
                        kind: FetchKind::Bench { slot },
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
                    ui.label(egui::RichText::new(&site.progress).small().color(
                        if site.affordable {
                            theme::MUTED
                        } else {
                            theme::WARN
                        },
                    ));
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

    /// The skills tree (feature 83): the player's own crew member's
    /// class, level by level down the tray — a level's own slot across
    /// the width where it has one, and two side by side where the level
    /// is a choice — with the levels numbered down the left and the
    /// spine beside them lit as far as the level reached. A box is
    /// coloured for its state: learnt, given up, open for a point, or
    /// waiting on a level. A click picks one, and **beside** the tree the
    /// picked slot says what it does, **what it is worth in numbers**,
    /// what state it is in, and — for one that is open — carries the
    /// button that spends the point. To the right rather than below,
    /// since the tray grows upwards off the foot of the window: a column
    /// beside the tree can say as much as it likes without the tree
    /// moving under the pointer. The tree asks nothing of the world it
    /// is not handed: everything here is `ClassView` and
    /// `world::class`'s own tables, the numbers said by
    /// [`crate::names::talent_numbers`] and
    /// [`crate::names::level_numbers`].
    fn skills(&mut self, ui: &mut egui::Ui) {
        let Some(view) = self.class_view.clone() else {
            return;
        };
        let tree = skill_tree_size();
        ui.set_max_width(tree.x + SKILL_DETAIL_GAP + DETAIL_WIDTH + 8.0);
        ui.horizontal(|ui| {
            ui.label(
                egui::RichText::new(SKILLS)
                    .size(SKILL_TEXT)
                    .color(theme::MUTED),
            );
            theme::question_mark(ui, SKILLS_TIP);
        });
        let class = view.class;
        if class == world::Class::None {
            ui.add(
                egui::Label::new(
                    egui::RichText::new(SKILLS_NO_CLASS)
                        .size(SKILL_TEXT)
                        .color(theme::MUTED),
                )
                .wrap(),
            );
            return;
        }
        ui.label(
            egui::RichText::new(class_line(class, view.level, view.to_next))
                .size(SKILL_TEXT)
                .color(theme::INK),
        );
        // A point a pick level reached and not chosen at.
        let points = points_left(&view);
        ui.label(
            egui::RichText::new(skill_points(points))
                .size(SKILL_TEXT)
                .color(if points > 0 {
                    theme::CAUTION
                } else {
                    theme::MUTED
                }),
        );
        ui.add_space(4.0);

        // Every slot of the ten levels, top to bottom, with the words
        // that go on and under it.
        let mut slots: Vec<(Slot, &'static str, &'static str)> = Vec::new();
        for level in 1..=world::class::LEVELS {
            match world::class::pick_at(class, level) {
                Some((left, right)) => {
                    slots.push((
                        Slot::Pick(level, world::Side::Left),
                        talent_name(left),
                        talent_tip(left),
                    ));
                    slots.push((
                        Slot::Pick(level, world::Side::Right),
                        talent_name(right),
                        talent_tip(right),
                    ));
                }
                None => slots.push((
                    Slot::Fixed(level),
                    level_name(class, level).unwrap_or(""),
                    level_line(class, level).unwrap_or(""),
                )),
            }
        }
        // A slot picked before the class changed under it is no slot.
        if self
            .skill_pick
            .is_some_and(|slot| !slots.iter().any(|&(s, ..)| s == slot))
        {
            self.skill_pick = None;
        }

        let mut learn = None;
        ui.horizontal_top(|ui| {
            ui.vertical(|ui| self.skills_tree(ui, &view, &slots));
            ui.add_space(SKILL_DETAIL_GAP);
            ui.vertical(|ui| {
                ui.set_width(DETAIL_WIDTH);
                ui.set_min_height(tree.y);
                learn = self.skills_detail(ui, &view, &slots);
            });
        });
        if let Some((level, side)) = learn {
            self.deploy_orders.push(DeployOrder::Pick {
                level: level as u32,
                side,
            });
            // The point spent, the tree says so next frame; the slot is
            // let go of so the column goes back to its hint.
            self.skill_pick = None;
        }
    }

    /// The tree itself: the spine and its numbers down the gutter, and a
    /// box a slot, coloured for its state and lit under the pointer or
    /// where the pick is. A click sets [`CrewPanels::skill_pick`]; the
    /// column beside it reads that.
    fn skills_tree(
        &mut self,
        ui: &mut egui::Ui,
        view: &ClassView,
        slots: &[(Slot, &'static str, &'static str)],
    ) {
        let state = |slot: Slot| SkillState::of(view, slot);
        let size = skill_tree_size();
        let (rect, response) = ui.allocate_exact_size(size, egui::Sense::click());
        let painter = ui.painter_at(rect);
        let row_y = |level: u8| rect.min.y + (level as f32 - 1.0) * (SKILL_BOX_H + SKILL_GAP_Y);
        let box_of = |slot: Slot| {
            let y = row_y(slot.level());
            let left = rect.min.x + SKILL_GUTTER;
            match slot {
                Slot::Fixed(_) => egui::Rect::from_min_size(
                    egui::pos2(left, y),
                    egui::vec2(2.0 * SKILL_BOX_W + SKILL_GAP_X, SKILL_BOX_H),
                ),
                Slot::Pick(_, world::Side::Left) => egui::Rect::from_min_size(
                    egui::pos2(left, y),
                    egui::vec2(SKILL_BOX_W, SKILL_BOX_H),
                ),
                Slot::Pick(_, world::Side::Right) => egui::Rect::from_min_size(
                    egui::pos2(left + SKILL_BOX_W + SKILL_GAP_X, y),
                    egui::vec2(SKILL_BOX_W, SKILL_BOX_H),
                ),
            }
        };
        // The spine down the gutter, lit as far as the level reached,
        // with the level's number beside each knot.
        let spine = rect.min.x + SKILL_GUTTER - 8.0;
        for level in 1..=world::class::LEVELS {
            let y = row_y(level) + SKILL_BOX_H / 2.0;
            let reached = level <= view.level;
            let ink = if reached { theme::ACCENT } else { theme::LINE };
            if level < world::class::LEVELS {
                let next = row_y(level + 1) + SKILL_BOX_H / 2.0;
                let on = level < view.level;
                painter.line_segment(
                    [egui::pos2(spine, y), egui::pos2(spine, next)],
                    egui::Stroke::new(1.5, if on { theme::ACCENT } else { theme::LINE }),
                );
            }
            painter.circle_filled(egui::pos2(spine, y), 3.0, ink);
            painter.text(
                egui::pos2(rect.min.x + 9.0, y),
                egui::Align2::CENTER_CENTER,
                level.to_string(),
                egui::FontId::proportional(SKILL_BOX_TEXT),
                if reached { theme::INK } else { theme::MUTED },
            );
        }
        let hovered = response
            .hover_pos()
            .and_then(|p| slots.iter().find(|&&(s, ..)| box_of(s).contains(p)));
        if let Some(p) = response.interact_pointer_pos()
            && response.clicked()
            && let Some(&(slot, ..)) = slots.iter().find(|&&(s, ..)| box_of(s).contains(p))
        {
            self.skill_pick = Some(slot);
        }
        for &(slot, label, _) in slots {
            let b = box_of(slot);
            let how = state(slot);
            let (fill, edge, ink) = match how {
                SkillState::Learnt => (theme::RAISED_ON, theme::ACCENT, theme::INK),
                SkillState::Open => (theme::RAISED, theme::CAUTION, theme::INK),
                SkillState::GivenUp | SkillState::Locked => {
                    (theme::PANEL_DEEP, theme::LINE, theme::MUTED)
                }
            };
            let lit = hovered.map(|&(s, ..)| s) == Some(slot) || self.skill_pick == Some(slot);
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
            let font = egui::FontId::proportional(SKILL_BOX_TEXT);
            let mut job =
                egui::text::LayoutJob::simple(label.to_string(), font, ink, b.width() - 10.0);
            // A slot given up is struck through: the level was chosen at,
            // and the other side of it is gone for good.
            if how == SkillState::GivenUp {
                for section in &mut job.sections {
                    section.format.strikethrough = egui::Stroke::new(1.0, ink);
                }
            }
            let galley = painter.layout_job(job);
            painter.galley(
                egui::pos2(
                    b.center().x - galley.size().x / 2.0,
                    b.center().y - galley.size().y / 2.0,
                ),
                galley,
                ink,
            );
        }
        if let Some(&(slot, label, tip)) = hovered {
            let level = slot.level();
            let numbers = skill_numbers(view.class, slot);
            response
                .clone()
                .on_hover_text(format!("Level {level} · {label}\n{tip}\n{numbers}"));
        }
    }

    /// What the picked slot is, beside the tree: its name, the level it
    /// sits at, **what it is worth in numbers**, what it does in words,
    /// what state it is in, and the button that spends the point on one
    /// that is open. Nothing is picked and it is the hint instead.
    fn skills_detail(
        &mut self,
        ui: &mut egui::Ui,
        view: &ClassView,
        slots: &[(Slot, &'static str, &'static str)],
    ) -> Option<(u8, world::Side)> {
        let picked = self
            .skill_pick
            .and_then(|pick| slots.iter().find(|&&(s, ..)| s == pick))
            .copied();
        let Some((slot, label, tip)) = picked else {
            ui.add(
                egui::Label::new(
                    egui::RichText::new(SKILLS_HINT)
                        .size(SKILL_TEXT)
                        .color(theme::MUTED),
                )
                .wrap(),
            );
            return None;
        };
        ui.label(
            egui::RichText::new(label)
                .size(SKILL_NAME_TEXT)
                .strong()
                .color(theme::INK),
        );
        ui.label(
            egui::RichText::new(skill_slot_line(
                slot.level(),
                matches!(slot, Slot::Pick(..)),
            ))
            .size(SKILL_TEXT)
            .color(theme::MUTED),
        );
        ui.add_space(4.0);
        // The figures first: what choosing this actually changes.
        ui.add(
            egui::Label::new(
                egui::RichText::new(skill_numbers(view.class, slot))
                    .size(SKILL_TEXT)
                    .color(theme::ACCENT),
            )
            .wrap(),
        );
        ui.add_space(4.0);
        ui.add(egui::Label::new(egui::RichText::new(tip).size(SKILL_TEXT)).wrap());
        ui.add_space(4.0);
        let state = SkillState::of(view, slot);
        let words = match state {
            SkillState::Learnt => match slot {
                Slot::Fixed(_) => SKILL_COMES_WITH.to_string(),
                Slot::Pick(..) => SKILL_LEARNT.to_string(),
            },
            SkillState::GivenUp => SKILL_GIVEN_UP.to_string(),
            SkillState::Open => SKILL_OPEN.to_string(),
            SkillState::Locked => skill_locked(slot.level()),
        };
        ui.add(
            egui::Label::new(egui::RichText::new(words).size(SKILL_TEXT).color(
                if state == SkillState::Open {
                    theme::CAUTION
                } else {
                    theme::MUTED
                },
            ))
            .wrap(),
        );
        if let (SkillState::Open, Slot::Pick(level, side)) = (state, slot) {
            ui.add_space(4.0);
            if ui
                .button(egui::RichText::new(SKILL_LEARN).size(SKILL_TEXT))
                .on_hover_text(SKILL_LEARN_TIP)
                .clicked()
            {
                return Some((level, side));
            }
        }
        None
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
        // The keys in the desk, a column a tier: a locked node wants one
        // of its own tier consumed for it.
        ui.horizontal(|ui| {
            ui.label(
                egui::RichText::new("Keys in the desk:")
                    .small()
                    .color(theme::MUTED),
            );
            ui.label(
                egui::RichText::new(format!("tier one {}", view.keys[0]))
                    .small()
                    .color(theme::MUTED),
            );
            ui.label(
                egui::RichText::new(format!("tier two {}", view.keys[1]))
                    .small()
                    .color(theme::TIER_TWO),
            );
            ui.label(
                egui::RichText::new("— a locked node wants one of its tier consumed for it")
                    .small()
                    .color(theme::MUTED),
            );
        });
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
            let queued = view.queue.iter().position(|&q| q == node.code());
            let (fill, edge, ink) = if view.done[i] {
                (theme::RAISED_ON, theme::ACCENT, theme::INK)
            } else if current || queued.is_some() {
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
            if let Some(at) = queued {
                // Its place in the queue, counted from one, in the corner.
                let centre = egui::pos2(b.max.x - 6.0, b.min.y + 6.0);
                painter.circle_filled(centre, 5.5, theme::ACCENT);
                painter.text(
                    centre,
                    egui::Align2::CENTER_CENTER,
                    (at + 1).to_string(),
                    egui::FontId::proportional(9.0),
                    theme::PANEL_DEEP,
                );
            }
        }
        if let Some(node) = hovered {
            response.clone().on_hover_text(node_line(node.code()));
        }

        // The queue, in order, under the tree — what the AI goes onto
        // next, with what is on it now at the front — and then the picked
        // node.
        ui.add_space(6.0);
        if view.current.is_some() || !view.queue.is_empty() {
            let mut line: Vec<String> = Vec::new();
            if let Some(code) = view.current {
                line.push(format!("{} (on it)", node_name(code)));
            }
            line.extend(view.queue.iter().map(|&code| node_name(code).to_string()));
            ui.add(
                egui::Label::new(
                    egui::RichText::new(format!("Queue: {}", line.join(" > ")))
                        .small()
                        .color(theme::MUTED),
                )
                .wrap(),
            );
        }
        let Some(node) = picked else {
            ui.label(
                egui::RichText::new("Click a node for what it opens and to queue it for the AI.")
                    .small()
                    .color(theme::MUTED),
            );
            return;
        };
        let i = node as usize;
        let def = node.def();
        let queued = view.queue.iter().position(|&q| q == node.code());
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
        } else if let Some(at) = queued {
            format!(
                "Queued, {} in line: {} of the AI's time.",
                ordinal(at + 1),
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
        } else if view.queueable[i] {
            format!(
                "Can be queued, with what it comes after ahead of it. {}",
                span_text(def.minutes as f32)
            )
        } else {
            format!(
                "Waiting on what it comes after, which is behind a key. {}",
                span_text(def.minutes as f32)
            )
        };
        ui.label(egui::RichText::new(state).small().color(theme::MUTED));
        ui.horizontal(|ui| {
            if view.needs_key[i] {
                let keys = view.keys.get(def.tier as usize - 1).copied().unwrap_or(0);
                let can = keys > 0 && view.desk && view.powered;
                let hint = if keys == 0 {
                    if def.tier == 2 {
                        "no tier-two key in the research desk — one lies on every hostile station's desk"
                    } else {
                        "no tier-one key in the research desk — one is found on a friendly station's desk"
                    }
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
                let hint = if view.queue.is_empty() {
                    "takes the AI off it; what was put in is lost"
                } else {
                    "takes the AI off it and onto the next in the queue; what was put in is lost, and whatever queued needed it comes off"
                };
                if ui.small_button("Stop").on_hover_text(hint).clicked() {
                    actions.research_orders.push(ResearchOrder::Cancel);
                }
            } else if queued.is_some() {
                if ui
                    .small_button("Take off the queue")
                    .on_hover_text("takes it off the queue, and whatever queued needed it")
                    .clicked()
                {
                    actions
                        .research_orders
                        .push(ResearchOrder::Dequeue(node.code()));
                }
            } else if !view.done[i] {
                let can = view.queueable[i] && view.desk;
                let hint = if !view.desk {
                    NO_DESK_HINT
                } else if view.needs_key[i] {
                    "consume a key at the desk first"
                } else if !view.queueable[i] {
                    "something it comes after is behind a key: consume one for that first"
                } else if view.current.is_some() || !view.queue.is_empty() {
                    if view.available[i] {
                        "queues it for the AI, after what is queued"
                    } else {
                        "queues it for the AI, with what it comes after ahead of it"
                    }
                } else if view.available[i] {
                    "puts the AI onto it"
                } else {
                    "queues it, with what it comes after ahead of it; the AI starts on the first"
                };
                let word = if view.current.is_some() || !view.queue.is_empty() {
                    "Queue"
                } else {
                    "Research"
                };
                if ui
                    .add_enabled(can, egui::Button::new(word).small())
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
            // What it costs out of the crew's one pool (feature 95), in
            // the warning colour when there is not that much left after
            // the sites already begun.
            let price = kind.def().price;
            ui.label(
                egui::RichText::new(crate::format::euros(price))
                    .size(11.0)
                    .color(if actions.free_money < price {
                        theme::WARN
                    } else {
                        theme::MUTED
                    }),
            );
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
                    // The one row whose number does not reach the whole
                    // crew: a bot never stands at a bench (feature 89).
                    let row = if job == bims::work::Job::Craft as u32 {
                        row.on_hover_text(WORK_CRAFT_TIP)
                    } else {
                        row
                    };
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
                        self.crew_orders
                            .push(CrewOrder::WorkPriority { job, back: true });
                    } else {
                        self.crew_orders
                            .push(CrewOrder::WorkPriority { job, back: false });
                    }
                    self.sort = None;
                }
            });
    }

    /// How the crew are left to get on: whether a Bim picks its own work,
    /// whether the workbench combines matching gear, and what the benches
    /// keep made.
    fn management(
        &mut self,
        ui: &mut egui::Ui,
        game: &mut Game,
        mut actions: Option<&mut Actions>,
    ) {
        ui.horizontal(|ui| {
            let mut on = game.is_autonomous();
            if ui.checkbox(&mut on, "").changed() {
                self.crew_orders.push(CrewOrder::Autonomous { on });
            }
            theme::asks(ui, "Let the Bim decide", AUTONOMY_TIP);
        });
        // The workbench's upgrade: a tick box, and while one is on the
        // bench a line saying what and how far.
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
        // What the benches make: a standing order to keep so many made,
        // kept in the hold and answered at the bench that makes it.
        let Some(actions) = actions else {
            return;
        };
        if actions.crafts.is_empty() {
            return;
        }
        egui::Grid::new("stock")
            .num_columns(4)
            .spacing([12.0, 2.0])
            .show(ui, |ui| {
                for head in ["Made aboard", "Stock", "Location", "Target"] {
                    ui.label(egui::RichText::new(head).small().color(theme::MUTED));
                }
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
            _ => String::new(),
        }
    }

    /// What the room makes of a point on the deck, in room coordinates: the
    /// spot code, and the thing there with its state — "Door · locked".
    pub fn spot_readout(game: &Game, x: f32, y: f32) -> (u32, String) {
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
        (spot, thing)
    }
}

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
        // A shelf is locker room since the money rework (feature 95).
        Container::Shelf(_) => game.container_frame(container).map(|_| Storage::Locker),
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
    laid_things(&gear.pack, &gear.turned, &gear.count)
}

/// `counts` is how many are in each cell's stack (feature 87) —
/// `Gear::count`, where nought reads as one.
fn laid_things(
    pack: &[Option<PackItem>],
    turned: &[bool],
    counts: &[u32],
) -> (Vec<grid::Laid>, Vec<usize>) {
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
            cell: cell_of(item, counts.get(cell).copied().unwrap_or(1).max(1)),
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
    // The nearest crewmate with a medkit to hand, since a medkit is a
    // charge in each one's own pack; the nearest at all when nobody has
    // one, so the row can say whose pack is empty.
    let at = game.bim_pos(patient);
    let nearest = |with_kit: bool| {
        (0..game.crew_count() as usize)
            .filter(|&h| h != patient && up(h) && (!with_kit || kits_to_hand(game, h) > 0))
            .min_by(|&a, &b| {
                (game.bim_pos(a) - at)
                    .len()
                    .total_cmp(&(game.bim_pos(b) - at).len())
            })
    };
    nearest(true).or_else(|| nearest(false))
}

/// The medkits `who` could treat with: the ones in its own pack — a
/// medkit is a charge every crew member carries — and any on the room's
/// shelf, which aboard is none. The room's own rule for whether a
/// treatment has a kit.
fn kits_to_hand(game: &Game, who: usize) -> u32 {
    let kit = PackItem::Stack(ResourceId::Medkit as u32);
    game.medkits() + game.gear(who).units_of(kit)
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
        NO_BANDAGE
    } else {
        "closes every wound on it — ten minutes with hands on"
    };
    (count, hint)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// What the peril block says is the room's own arithmetic and not a
    /// second opinion: the rate is every open wound at
    /// `health::BLEED_PER_WOUND` an hour plus every untreated trauma's
    /// own `bleed()`, which is the sum `Health::update_held` subtracts,
    /// and the countdown is the blood left divided by it.
    #[test]
    fn a_dying_bim_says_what_is_taking_it_down_and_how_long_it_has() {
        let mut game = Game::new(7, 1200.0, 800.0);
        assert!(
            perils(&game, 0).is_empty(),
            "a whole body is dying of nothing"
        );
        assert!(!game.is_dying(0));

        // A leg taken to nothing: the trauma rolled on it, untreated, and
        // one wound open besides.
        game.wound(0, health::Part::Legs, 1_000.0);
        let trauma = game
            .trauma(0, health::Part::Legs)
            .expect("a part at nothing has a trauma");
        assert!(game.is_dying(0), "the cross hangs off this");
        assert_eq!(game.wounds(0, health::Part::Legs), 1);

        let an_hour = trauma.bleed() + health::BLEED_PER_WOUND;
        let list = perils(&game, 0);
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].cause, PERIL_BLEEDING);
        assert_eq!(list[0].rate, peril_rate(an_hour));
        assert_eq!(
            list[0].minutes,
            Some(game.blood(0) / an_hour * bims::clock::HOUR)
        );
        // The wound is always one of the rows; the trauma is one too
        // whenever it is a trauma that bleeds.
        assert!(list[0].from.contains(&peril_from(
            &peril_wounds("Legs", 1),
            health::BLEED_PER_WOUND
        )));
        assert_eq!(list[0].from.len(), if trauma.bleed() > 0.0 { 2 } else { 1 });
        assert_eq!(
            list[0].remedy,
            if trauma.bleed() > 0.0 {
                PERIL_BLEED_MEDKIT
            } else {
                PERIL_BLEED_BANDAGE
            }
        );

        // And a body with no blood in it died of that, whatever else is
        // wrong with it.
        assert_eq!(death_line(&game, 0), DEATH_OTHER);
        game.set_blood_for_probe(0, 0.0);
        assert_eq!(death_line(&game, 0), DEATH_BLED_OUT);
    }

    /// The work list's rows are built off `work_count`, so a spot table a
    /// row short would ring nothing for the last job and nobody would
    /// notice.
    #[test]
    fn every_job_has_a_place_to_ring() {
        assert_eq!(WORK_SPOTS.len(), bims::work::Job::ALL.len());
    }

    /// The Skills tree's rules (feature 83): a point for every pick level
    /// reached and not chosen at, a slot learnt where it was chosen, the
    /// other side of it given up for good, and everything above the level
    /// reached locked. The tree draws nothing it does not read here.
    #[test]
    fn a_skills_tree_counts_its_points_and_says_what_every_slot_is() {
        let view = |level: u8, picks: &[(u8, world::Side)]| ClassView {
            class: world::Class::Engineer,
            level,
            picks: picks.to_vec(),
            ..Default::default()
        };
        // Level one: the first is learnt, everything above it waits, and
        // there is nothing to spend.
        let one = view(1, &[]);
        assert_eq!(points_left(&one), 0);
        assert_eq!(SkillState::of(&one, Slot::Fixed(1)), SkillState::Learnt);
        assert_eq!(SkillState::of(&one, Slot::Fixed(3)), SkillState::Locked);
        assert_eq!(
            SkillState::of(&one, Slot::Pick(2, world::Side::Left)),
            SkillState::Locked
        );
        // Level five, nothing chosen: three pick levels behind it — two,
        // four and five — and so three points.
        let five = view(5, &[]);
        assert_eq!(points_left(&five), 3);
        assert_eq!(
            SkillState::of(&five, Slot::Pick(2, world::Side::Left)),
            SkillState::Open
        );
        assert_eq!(SkillState::of(&five, Slot::Fixed(3)), SkillState::Learnt);
        assert_eq!(
            SkillState::of(&five, Slot::Pick(6, world::Side::Left)),
            SkillState::Locked
        );
        // One spent on the left of the second: a point fewer, that side
        // learnt and the other gone.
        let spent = view(5, &[(2, world::Side::Left)]);
        assert_eq!(points_left(&spent), 2);
        assert_eq!(
            SkillState::of(&spent, Slot::Pick(2, world::Side::Left)),
            SkillState::Learnt
        );
        assert_eq!(
            SkillState::of(&spent, Slot::Pick(2, world::Side::Right)),
            SkillState::GivenUp
        );
        // The top of the tree with every level chosen at: nothing left to
        // spend. Seven pick levels, which is what a tenth-level run opens
        // with.
        let all: Vec<(u8, world::Side)> = (2..=world::class::LEVELS)
            .filter(|&l| world::class::is_pick_level(world::Class::Engineer, l))
            .map(|l| (l, world::Side::Right))
            .collect();
        assert_eq!(all.len(), 7);
        assert_eq!(points_left(&view(10, &[])), 7);
        assert_eq!(points_left(&view(10, &all)), 0);
        // And a crew member with no class has no levels to spend at.
        assert_eq!(
            points_left(&ClassView {
                level: 10,
                ..Default::default()
            }),
            0
        );
    }
}
