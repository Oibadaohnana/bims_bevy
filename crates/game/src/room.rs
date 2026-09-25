//! The deck: where everything on it stands, the state of what on it moves
//! — the doors, the lamps, the blood, a weapon let fall — and how it is
//! drawn.
//!
//! A room is laid out from a ship design (`crate::aboard`), or is a **bare**
//! room — a box of deck inside four walls with nothing on it
//! ([`Room::bare`]), which is what the tests and the probes stand bodies
//! in. The view is scaled to fit the canvas round it (see `Game::resize`).

use crate::blood::Blood;
use crate::cue::Cued;
use crate::door::{Door, Order};
use crate::draw::{Color, DrawList};
use crate::fixtures::{
    self, Bay, Berth, CHAIR_SIZE, Dishwasher, Fridge, Heads, Hob, Locker, Still, Stills, Worktop,
    galley_faces,
};
use crate::math::{Rect, Vec2, lerp, vec2};
use crate::sight::Sight;

/// The size of the test room [`Room::bare`] is usually asked for.
pub const ROOM_W: f32 = 860.0;
pub const ROOM_H: f32 = 580.0;

/// A bare room's bulkheads, all the way round.
const WALL: f32 = 18.0;
/// How far out from a door's opening the Bim stands at its panel. Has to
/// clear `BODY_MARGIN` or the collision push-out fights the spot.
const STAND_OFF: f32 = 26.0;

pub const TILE: f32 = 52.0;

// --- palette ------------------------------------------------------------
//
// One ship's palette: a dark deck, grey composite panels, brushed steel, and a
// cyan running light that everything powered picks up. Warm colours are kept
// for heat and for anything wrong, so those read at a glance against all the
// blue-grey.

/// Outside the deck plating, and the recesses things retract into.
pub const HULL: Color = Color::rgb(0.05, 0.06, 0.08);
pub const DECK: Color = Color::rgb(0.13, 0.15, 0.18);
pub const DECK_SEAM: Color = Color::rgba(0.55, 0.85, 0.95, 0.055);
/// Composite panelling, in the three shades everything built out of it uses.
pub const PANEL: Color = Color::rgb(0.19, 0.22, 0.26);
pub const PANEL_LIT: Color = Color::rgb(0.28, 0.32, 0.37);
pub const PANEL_EDGE: Color = Color::rgb(0.40, 0.46, 0.53);
/// The running light. `GLOW_DIM` is the same colour at strip brightness.
pub const GLOW: Color = Color::rgb(0.38, 0.86, 0.95);
pub const GLOW_DIM: Color = Color::rgba(0.38, 0.86, 0.95, 0.28);
/// Hot, locked, or otherwise worth noticing.
pub const WARN: Color = Color::rgb(0.98, 0.45, 0.32);

const FLOOR: Color = DECK;
const TILE_LINE: Color = DECK_SEAM;
const WALL_C: Color = HULL;
const WALL_TRIM: Color = Color::rgb(0.22, 0.26, 0.31);

pub const STEEL: Color = Color::rgb(0.78, 0.83, 0.87);
pub const GRIP: Color = Color::rgb(0.12, 0.14, 0.17);

/// Dressings every Bim in a bare room starts out **carrying**, so a test
/// can dress a wound without a drug lab. A bandage is a thing in a pack
/// and nothing else: aboard it is `World::restock_bandages` that fills a
/// pack out of the hold.
pub const BANDAGES_AT_DAWN: u32 = 3;

/// Medkits a bare room starts with on its shelf, the same way: a trauma
/// can be treated in a test without an armoury.
pub const MEDKITS_AT_DAWN: u32 = 2;

/// A weapon lying on the deck, let go of by a body knocked out: what it
/// is, where it lies, and whose hand it fell from — that Bim comes back
/// for it when it comes round (`Game::fetch`), and the player can send
/// anybody. `id` is the number a chain names it by.
#[derive(Clone, Copy, PartialEq, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Dropped {
    pub id: u32,
    pub at: Vec2,
    pub weapon: crate::combat::Weapon,
    pub owner: usize,
}

/// Something the Bim can walk up to and work with its hands: one of a
/// ship's powered doors, by index into [`Room::doors`], and what to do to
/// it.
///
/// Nothing aboard is remote-controlled: asking for it starts an errand
/// that walks the Bim over, and the state only changes at the moment its
/// hand arrives. It carries the order rather than reading the door when
/// the hand arrives, because the asking can change the door itself.
#[derive(Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Switch {
    Door(usize, Order),
}

/// Things a click can land on. A click on a fixture that only stands there
/// — the galley, a bunk, the heads, a bay, the broom locker, a shower — is
/// `HIT_NONE`, like a click on bare deck: nothing hangs off one.
pub const HIT_NONE: u32 = 0;
/// One of a ship's powered doors. Which one is [`Room::door_at`]'s answer,
/// which the game records for the host to ask after the click.
pub const HIT_SHIP_DOOR: u32 = 9;
/// One of the crew — a body, not a fixture: `Game::hit_at` answers it
/// before asking the room, and `Game::hit_bim` says which. The menu on it
/// is the bandages, for the player's Bim to dress whoever was clicked.
pub const HIT_BIM: u32 = 11;
/// A workstation, any kind: `Game::hit_bench` says which, and
/// `Game::bench_part` what it is — the app opens the armoury's grid off
/// that and nothing off the rest.
pub const HIT_BENCH: u32 = 12;
/// A shelf — the storage grid. `Game::hit_shelf` says which.
pub const HIT_SHELF: u32 = 13;
/// A dead crew member under the click — a body, for looting. An
/// unconscious crewmate is still `HIT_BIM`, and the menu on it decides
/// what to offer — the bandages, and the looting beside them.
pub const HIT_BODY: u32 = 14;
/// A docked station's resident under the click, down — dead or out cold,
/// which the world says through `Game::set_visitors_down` — or one the
/// world says may be spoken to. `Game::hit_visitor` says which.
pub const HIT_VISITOR: u32 = 15;
/// A station's trading desk — the Trade row, which walks the Bim to it
/// and opens the trade window. `Game::hit_desk` says which.
pub const HIT_DESK: u32 = 16;
/// A weapon lying on the deck — dropped by a body knocked out, for
/// picking up. `Game::hit_dropped` says which.
pub const HIT_DROPPED: u32 = 17;
/// A research desk — the ship's own, for its window, or a station's on
/// the joined deck, for the key on it. `Game::hit_research` says which.
pub const HIT_RESEARCH: u32 = 18;

/// What is at a point, for the readout that names whatever the pointer is
/// over.
///
/// A separate set from the `HIT_` codes above and deliberately so. Those
/// answer "what would a click here act on", which is why they are few and why
/// each rect is expanded a little — a near miss on a handle should still open
/// the menu. These answer "what is this", which wants the opposite: everything
/// aboard has a name, including the things no menu hangs off, and pointing a
/// few pixels beside the pan should say deck rather than toilet.
pub const SPOT_NOTHING: u32 = 0;
pub const SPOT_DECK: u32 = 1;
pub const SPOT_BULKHEAD: u32 = 2;
pub const SPOT_WORKTOP: u32 = 3;
pub const SPOT_BOARD: u32 = 4;
pub const SPOT_FRIDGE: u32 = 5;
pub const SPOT_HOB: u32 = 6;
pub const SPOT_DISHWASHER: u32 = 7;
pub const SPOT_TABLE: u32 = 8;
pub const SPOT_CHAIR: u32 = 9;
pub const SPOT_BUNK: u32 = 10;
pub const SPOT_BAY: u32 = 11;
pub const SPOT_TOILET: u32 = 12;
pub const SPOT_BASIN: u32 = 13;
/// The deck between the pan and the basin of the first heads.
pub const SPOT_HEADS_DECK: u32 = 14;
pub const SPOT_LOCKER: u32 = 15;
/// A ship's powered door.
pub const SPOT_SHIP_DOOR: u32 = 16;
/// A workstation. Only ever *pointed at* — the craft row rings every bench —
/// never read back off the deck: the ship's readout names the part itself.
pub const SPOT_BENCH: u32 = 17;
/// The suit locker, the same way.
pub const SPOT_SUIT_LOCKER: u32 = 18;
/// The shower, for the readout and for ringing.
pub const SPOT_SHOWER: u32 = 19;
/// A research desk, for ringing: the research tab's rows ring it.
pub const SPOT_RESEARCH: u32 = 20;

/// How many people a small station houses: a bunk and a chair apiece for
/// two. The world reads it for a relay's and a hub's crowd.
pub const BERTHS: usize = 2;

/// A workstation a Bim can be stood at: the workbench, the armoury, the
/// drug lab. What the craft chain walks to — `task::Kind::Craft` — and
/// nothing else about it is the room's: what is made there, out of what,
/// and whether it has the power to run are the world's, which hands the
/// room a list of `game::Order`s every step. `kind` is the part's code, so
/// the world can say which bench a recipe wants without the room knowing a
/// `PartKind`.
#[derive(Clone, Copy)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Bench {
    pub kind: u32,
    /// Its footprint, for ringing.
    pub frame: Rect,
    /// Where the Bim stands to work it, off the part's use spot.
    pub at: Vec2,
}

impl Bench {
    /// Which way the Bim faces at it: into it.
    pub fn facing(&self) -> f32 {
        let d = self.frame.center() - self.at;
        d.y.atan2(d.x)
    }
}

/// Where everything in a room is, for a room laid out from a ship design
/// through `crate::aboard`. Plain rects and points in room units, and
/// nothing that knows what a ship is: the probes stand this file up with
/// no other crate behind it, and a layout type that named one would take
/// that away.
///
/// A layout always has one of each of the furniture it draws — the
/// counter, the cold store, the hob, the dishwasher, the table, the
/// broom locker, the bay, the heads — the first of its kind by id, or,
/// for a design that has none, a stand-in on the worktop. Everything past
/// the first is in [`More`].
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Layout {
    /// The whole build area.
    pub bounds: Rect,
    /// The deck: the bounding box of every floor tile.
    pub interior: Rect,
    pub counter: Rect,
    pub fridge: Rect,
    pub stove: Rect,
    pub dishwasher: Rect,
    pub table: Rect,
    /// Seat centres, in id order.
    pub chairs: Vec<Vec2>,
    /// Bunk footprints, in the order the crew are dealt them.
    pub beds: Vec<Rect>,
    pub locker: Rect,
    pub bay: Rect,
    /// Which side of the bay it was worked from, as a unit step out of the
    /// frame, off the design's use spots: the trays run across that way.
    pub bay_side: Vec2,
    pub toilet: Rect,
    pub sink: Rect,
    /// The shower, if the layout has one: its footprint and the spot in
    /// front of it — a place a station's people walk to on their round.
    /// The footprint stays in `others`: the ship painter draws it.
    pub shower: Option<(Rect, Vec2)>,
    /// The workstations, in the design's id order. Each stays a solid in
    /// `others` — the ship draws it.
    pub benches: Vec<Bench>,
    /// The suit locker, if the layout has one: its footprint and where the
    /// Bim stands at it. A solid in `others`, like the shower.
    pub suit_locker: Option<(Rect, Vec2)>,
    /// The deck just inside the port, and the spot just outside it beyond
    /// the hull, if the design has a port. Where a walk outside goes out
    /// from and is held at.
    pub gangway: Option<Vec2>,
    pub outside: Option<Vec2>,
    /// Everything else a body cannot walk through.
    pub others: Vec<Rect>,
    /// Everything a line of sight stops at that is not a door: the walls,
    /// the tall parts, and every tile inside the deck's box that has no
    /// deck — `PartDef::blocks_sight` is the rule. The doors are their own
    /// list, since a door is in the way only while it is shut.
    pub opaque: Vec<Rect>,
    /// Which of `opaque` are furniture rather than wall — the tall parts,
    /// `shipdesign::is_wall` failing — so the light picture can shade
    /// behind a cabinet softly where it blacks out behind a bulkhead.
    /// `Sight::set_tall`.
    pub tall: Vec<Rect>,
    /// Low cover — every sandbags part (`shipdesign::is_cover`): nothing
    /// to a walk or a line of sight, but a body close behind ducks a shot
    /// from across it. `Sight::covered` is the rule.
    pub cover: Vec<Rect>,
    /// The lights — every wall light and standing light, `shipdesign::light_tiles`
    /// — each where it is and how far it reaches. A designed deck with none
    /// is dark, and `Sight` says what that costs.
    pub lights: Vec<crate::sight::Light>,
    /// Every tile of the hull, frame and all: what a body outside walks
    /// round, and what the fog of what the crew cannot see is drawn over.
    pub hull: Vec<Rect>,
    /// The shelves, each its footprint and where the Bim stands at it.
    /// Solids in `others` as well — the room has no picture for a shelf;
    /// the ship draws it.
    pub shelves: Vec<(Rect, Vec2)>,
    /// The trading desks, each its footprint and where the Bim stands at
    /// it: a station's, where the crew trade with it — the world wants a
    /// crew member at one to buy or sell. Solids in `others` as well; the
    /// ship draws it. Empty on a ship of its own.
    pub desks: Vec<(Rect, Vec2)>,
    /// The research desks, the same: the ship's own, where a research key
    /// is put and the AI works, and a station's on the joined deck, where
    /// one is found. Solids in `others` as well; the ship draws it.
    pub research: Vec<(Rect, Vec2)>,
    /// The powered doors, each its opening, whether its leaves slide along
    /// `x`, and whether it is an airlock — the airlocks are doors too, so
    /// they lock and are forced like the rest; `airlocks` below keeps
    /// their footprints for the walk outside.
    pub doors: Vec<(Rect, bool, bool)>,
    /// The airlocks, each its footprint. Walked onto, never a solid, and
    /// the ship draws them; they are here for sight alone, which they
    /// stop like a door with nobody at it.
    pub airlocks: Vec<Rect>,
    /// Every piece of furniture beyond the first of its kind. See [`More`].
    pub more: More,
    /// What the room draws beside the furniture: a second table, a basin
    /// with no toilet of its own. Solids in `others` still. See [`Still`].
    pub extras: Vec<(Still, Rect)>,
    /// The planet's plain the deck stands on, if it is on one — see
    /// `crate::terrain` and `aboard::layout_of_on`. `None` everywhere else.
    pub plane: Option<crate::terrain::Plane>,
}

/// Which side of a bunk the deck is on: whichever faces the middle of the
/// room.
fn bed_side(frame: Rect, interior: Rect) -> f32 {
    if frame.center().x < interior.center().x {
        1.0
    } else {
        -1.0
    }
}

/// A layout's seats, with a stand-in below the table for a layout that
/// brought none.
fn layout_chairs(layout: &Layout) -> Vec<Vec2> {
    let mut chairs = layout.chairs.clone();
    if chairs.is_empty() {
        chairs.push(vec2(layout.table.center().x, layout.table.max.y + 30.0));
    }
    chairs
}

/// Every piece of furniture beyond the first of its kind, in id order,
/// each its footprint — and for a shower or a bay the spot it was worked
/// from, for a toilet the basin nearest it. The first of each is the
/// layout's own field.
#[derive(Clone, Default, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct More {
    pub worktops: Vec<Rect>,
    pub hobs: Vec<Rect>,
    pub fridges: Vec<Rect>,
    pub dishwashers: Vec<Rect>,
    pub lockers: Vec<Rect>,
    pub showers: Vec<(Rect, Vec2)>,
    pub bays: Vec<(Rect, Vec2)>,
    /// The fields, every one: a strip of open ground laid out like a bay.
    /// None is ever the layout's own bay — a ship has no fields — so they
    /// all come here, chained after the bays into `Room::bays`.
    pub fields: Vec<(Rect, Vec2)>,
    /// A heads apiece: the toilet, and the basin nearest it. A basin may
    /// be two heads'.
    pub heads: Vec<(Rect, Rect)>,
}

/// Which of the room's fixtures a picture of it is to have.
///
/// A room laid out from a design that is not finished — the designer's,
/// while the ship is being built — has a layout that puts every fixture
/// it has not got on the worktop, and the worktop on the first deck tile
/// (see `aboard::layout_of`). Drawn whole, that is a heap of galley on one
/// tile; so the designer says which are really there and gets a picture of
/// those. The game's room is always whole and draws everything.
#[derive(Clone, Copy, Default, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Fixtures {
    pub counter: bool,
    pub stove: bool,
    pub fridge: bool,
    pub dishwasher: bool,
    pub table: bool,
    pub chairs: bool,
    pub locker: bool,
    pub beds: bool,
    pub bay: bool,
    pub toilet: bool,
    pub basin: bool,
    pub doors: bool,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Room {
    /// The whole of the room, bulkheads included: what the view fits and
    /// what `spot` calls bulkhead rather than nothing. A ship's is its build
    /// area.
    pub bounds: Rect,
    /// Whether the room draws its own deck plate and walls. A bare room
    /// does; a room laid out from a ship design does not — the ship painter
    /// draws every tile of the hull itself, so this one draws only what
    /// stands on it.
    pub shell: bool,
    /// Blocking parts the room has no opinion about — an engine, a helm, a
    /// tank, an internal wall — which a body still has to walk round.
    pub others: Vec<Rect>,
    /// The powered doors, in the order the layout listed them. Every one is
    /// a way through unless it is locked — see `crate::door`.
    pub doors: Vec<Door>,
    /// The airlocks' footprints, for sight: a passage with nobody at it is
    /// a wall to the eyes, exactly as a powered door with its leaves shut.
    /// The ship draws them and the pathfinder walks through them.
    pub airlocks: Vec<Rect>,
    /// The fixtures beyond the furniture, drawn as themselves.
    pub stills: Stills,
    /// Whether this room draws its doors. A station's room kept open for
    /// its pictures while the ship is docked does not: the joined room has
    /// the same doors, with the people walking through them, and two
    /// pictures of one door in two states is a door in two states.
    pub doors_drawn: bool,
    pub interior: Rect,
    /// The furniture, every piece of each kind, the layout's own first:
    /// drawn where it stands, and every frame a solid (see
    /// [`Room::solids`]). Empty in a bare room; aboard never, since a
    /// layout brings a stand-in for one it has not got. See
    /// `crate::fixtures`.
    pub worktops: Vec<Worktop>,
    pub hobs: Vec<Hob>,
    pub fridges: Vec<Fridge>,
    pub dishwashers: Vec<Dishwasher>,
    pub lockers: Vec<Locker>,
    pub table: Option<Rect>,
    pub chairs: Vec<Vec2>,
    /// The bunks, in the order the layout listed them. A layout without one
    /// gets a stand-in on the worktop (`stand_in_bed`), which nobody is
    /// dealt.
    pub beds: Vec<Berth>,
    /// Whether the one bed in `beds` is the stand-in for a layout with
    /// none. See [`Room::bunks`].
    pub stand_in_bed: bool,
    /// Whose bunk is which: `bunk_of[who]` is the index into `beds` of the
    /// bunk Bim `who` was dealt, or `None`. A bunk is the deck beside which
    /// a body coming aboard is stood when its feet would land off the deck
    /// (`Game::adopt`); it is the game's to keep in step with its crew
    /// (`Game::with_room`, `take_crew`, `adopt`, `die`).
    pub bunk_of: Vec<Option<usize>>,
    /// The heads, each a pan and the basin nearest it, the layout's own
    /// first.
    pub heads: Vec<Heads>,
    /// The bays and the fields, the layout's own bay first.
    pub bays: Vec<Bay>,
    /// The showers, each its footprint and the spot in front of it. See
    /// `Layout::shower`.
    pub showers: Vec<(Rect, Vec2)>,
    /// What the crew can see of the room, shared. Its walls are the
    /// layout's; the shut doors are added at every trace. See `crate::sight`.
    pub sight: Sight,
    /// The workstations. See [`Bench`] and `Layout::benches`.
    pub benches: Vec<Bench>,
    /// The suit locker, the deck inside the port and the spot outside it.
    /// See `Layout::suit_locker`, `Layout::gangway`, `Layout::outside`.
    pub suit_locker: Option<(Rect, Vec2)>,
    pub gangway: Option<Vec2>,
    pub outside: Option<Vec2>,
    /// Every tile of the hull, for the outside grid. See `Layout::hull`.
    pub hull: Vec<Rect>,
    /// The shelves, footprint and stand spot each. See `Layout::shelves`.
    pub shelves: Vec<(Rect, Vec2)>,
    /// The trading desks, the same. See `Layout::desks`.
    pub desks: Vec<(Rect, Vec2)>,
    /// The research desks, the same. See `Layout::research`.
    pub research: Vec<(Rect, Vec2)>,
    /// The construction sites the world wants worked, this step: where
    /// each is and how long it takes to put together. Set by
    /// `Game::set_build_orders`. See `crate::game::Build`.
    pub builds: Vec<crate::game::Build>,
    /// Who may put a suit on and go out to a site outside the hull, by crew
    /// index — the world's say, set with the sites.
    pub suit_ok: Vec<bool>,
    /// The sites put together, each with who put it together — for the
    /// world to give the engineers about the builder their due (feature
    /// 74). `(site, who)`.
    pub built: Vec<(u32, usize)>,
    /// Every kit laid since the world last asked: who laid it, the middle
    /// of the tile in room units, and whether it was a sentry. The world
    /// puts the deployable down. See `crate::task::Kind::Deploy`.
    pub deployed: Vec<(usize, Vec2, bool)>,
    /// What the world wants carried between two benches this step — a
    /// gun or a piece of armour to the workbench, or the upgraded one
    /// back — at most one; set by `Game::set_ferries`. See
    /// `crate::game::Ferry`.
    pub ferries: Vec<crate::game::Ferry>,
    /// What the carrying chains did since the world last asked, drained
    /// every step: the thing was taken at the first bench, put down at the
    /// second, or given up between them.
    pub ferry_picked: Vec<crate::game::Ferry>,
    pub ferry_dropped: Vec<crate::game::Ferry>,
    pub ferry_returned: Vec<crate::game::Ferry>,
    /// Moves whenever the outside grid has to be built again. Nothing
    /// changes it now that the mining is gone; it is kept because
    /// `Game::refresh_outside` reads it, and a planet's ground or another
    /// reason to rebuild would set it.
    pub rocks_version: u64,
    /// The planet's plain the deck stands on, while it is landed on one:
    /// the ground beyond the room's box, what the crew see of it, and
    /// the frame it is read through. See `crate::terrain`. `None`
    /// everywhere else.
    pub plane: Option<crate::terrain::Plane>,
    /// Recipes finished at a bench since the world last asked — indices into
    /// `shipdesign::recipes::RECIPES`. The world drains it every step with
    /// `Game::take_crafted` and moves the cargo.
    pub crafted: Vec<u32>,
    /// The blood on the deck. It lives here because the deck *is* the
    /// room; `Game::render` draws it under the bodies.
    pub blood: Blood,
    /// Every dressing finished since the game last looked — `(helper,
    /// patient, part code)`, pushed by the bandage chain as its hands come
    /// off the patient. The game drains it after everybody has moved and
    /// does the dressing: the patient's body is a `Bim`, which the room
    /// never holds. The helper is on it because by then its chain is over
    /// and gone, and the game still has to ask whether the two are
    /// standing together.
    pub dressed: Vec<(usize, usize, u32)>,
    /// Medkits to hand on the shelf: nought aboard, where every kit is a
    /// thing in a pack, and `MEDKITS_AT_DAWN` in a bare room.
    pub medkits: u32,
    pub medkits_used: u32,
    /// Medkits in each Bim's **own pack**, by index: the world's word
    /// every step (`Game::set_pack_kits`), nought for anybody it does not
    /// name. A helper that carries one opens that where it stands rather
    /// than walking to a cabinet for the hold's — its own kit before a new
    /// one.
    #[cfg_attr(feature = "serde", serde(skip))]
    pub pack_kits: Vec<u32>,
    /// Every helper that took a kit out of its own pack since the world
    /// last asked (`Game::take_pack_kits_used`). Derived from the packs,
    /// and left out of a save with them.
    #[cfg_attr(feature = "serde", serde(skip))]
    pub pack_kits_used: Vec<usize>,
    /// Where a kit is fetched from: the use spot of every container the
    /// world says holds one, set every step aboard (`Game::set_kit_stands`);
    /// empty in a bare room and a station's, where a kit is to hand.
    pub kit_stands: Vec<Vec2>,
    /// Every treatment finished since the game last looked — `(helper,
    /// patient, part code, bare)`, like `dressed`, pushed by the treat
    /// chain as its hands come off; `bare` is a medic's field surgery
    /// with no kit (feature 76). The game does the treating: the trauma
    /// is on the patient's `Health`.
    pub treated: Vec<(usize, usize, u32, bool)>,
    /// Weapons lying on the deck: what a body knocked out let go of, where
    /// it fell. Each numbered from `next_weapon_down`, so a chain walking to
    /// one names it by a number that survives another being picked up.
    pub weapons_down: Vec<Dropped>,
    pub next_weapon_down: u32,
    /// Every pick-up finished since the game last looked — `(who, dropped
    /// id)`, pushed by the fetch chain as the hand closes on it. The game
    /// moves the weapon: the gear is a `Bim`'s.
    pub picked_up: Vec<(usize, u32)>,
    /// Where every one of the crew stands this step, by index — `None` for
    /// one dead or outside. The one thing about the crew the room is told,
    /// set by the game at the top of every step, so that a chain walking
    /// to a *crewmate* — the bandage — can pick its spot as the walk is
    /// entered. Nothing else reads it.
    pub crew: Vec<Option<Vec2>>,
    /// What happened this step that a host may want to hear — see
    /// `crate::cue`. Said by the doors; drained through `Game::take_cues`.
    pub cues: Vec<Cued>,
}

impl Room {
    /// A bare room: `width` by `height`, bulkheads all round, a deck plate
    /// inside them and nothing standing on it. Lit throughout, since it is
    /// handed no lamps. What the tests and the probes stand bodies in.
    pub fn bare(width: f32, height: f32) -> Room {
        let interior = Rect::from_corners(vec2(WALL, WALL), vec2(width - WALL, height - WALL));
        let bounds = Rect::from_min_size(Vec2::ZERO, vec2(width, height));
        Room {
            bounds,
            shell: true,
            others: Vec::new(),
            doors: Vec::new(),
            doors_drawn: true,
            airlocks: Vec::new(),
            stills: Stills::default(),
            interior,
            worktops: Vec::new(),
            hobs: Vec::new(),
            fridges: Vec::new(),
            dishwashers: Vec::new(),
            lockers: Vec::new(),
            table: None,
            chairs: Vec::new(),
            beds: Vec::new(),
            stand_in_bed: false,
            bunk_of: Vec::new(),
            heads: Vec::new(),
            bays: Vec::new(),
            showers: Vec::new(),
            sight: Sight::new(bounds, interior, TILE, &[], &[]),
            benches: Vec::new(),
            suit_locker: None,
            gangway: None,
            outside: None,
            hull: Vec::new(),
            shelves: Vec::new(),
            desks: Vec::new(),
            research: Vec::new(),
            builds: Vec::new(),
            suit_ok: Vec::new(),
            built: Vec::new(),
            deployed: Vec::new(),
            ferries: Vec::new(),
            ferry_picked: Vec::new(),
            ferry_dropped: Vec::new(),
            ferry_returned: Vec::new(),
            rocks_version: 0,
            plane: None,
            crafted: Vec::new(),
            blood: Blood::new(interior),
            dressed: Vec::new(),
            medkits: MEDKITS_AT_DAWN,
            medkits_used: 0,
            pack_kits: Vec::new(),
            pack_kits_used: Vec::new(),
            kit_stands: Vec::new(),
            treated: Vec::new(),
            weapons_down: Vec::new(),
            next_weapon_down: 0,
            picked_up: Vec::new(),
            crew: Vec::new(),
            cues: Vec::new(),
        }
    }

    /// A room laid out from somewhere else — a ship design, through
    /// `crate::aboard`. Every fixture is where the layout says; the room
    /// draws none of its own shell.
    ///
    /// The room has as many beds and chairs as the layout brings — every
    /// bunk and every chair of a design, in id order, which is what makes a
    /// docked ship and the station it is docked to one room with everybody's
    /// bunk in it. A layout with none of one gets a single stand-in.
    pub fn from_layout(layout: Layout) -> Room {
        let interior = layout.interior;
        let counter = layout.counter;
        let mut beds: Vec<Berth> = layout
            .beds
            .iter()
            .map(|&frame| Berth::new(frame, bed_side(frame, interior)))
            .collect();
        let stand_in_bed = beds.is_empty();
        if stand_in_bed {
            beds.push(Berth::new(counter, bed_side(counter, interior)));
        }
        let chairs = layout_chairs(&layout);
        let (board, drawer, dish_face) = galley_faces(counter, layout.dishwasher);
        let mut dishwasher = Dishwasher::at(dish_face);
        dishwasher.body = Some(layout.dishwasher);
        // On a plain the fog is drawn over the whole box, the ground round
        // the deck included; elsewhere over the hull, and the void is
        // nobody's.
        let fogged: Vec<Rect> = if layout.plane.is_some() {
            vec![interior]
        } else {
            layout.hull.clone()
        };
        let mut sight = Sight::new(layout.bounds, interior, TILE, &layout.opaque, &fogged);
        // On a plain the eye reaches the plain's sight range and no further,
        // indoors and out.
        if layout.plane.is_some() {
            sight.set_range(Some(crate::terrain::VIEW as f32 * TILE));
        }
        sight.set_cover(&layout.cover);
        sight.set_tall(&layout.tall);
        sight.set_lights(&layout.lights);
        let bays = bays_of(&layout);

        Room {
            bounds: layout.bounds,
            shell: false,
            others: layout.others,
            doors: layout
                .doors
                .iter()
                .map(|&(rect, along_x, airlock)| Door::of_kind(rect, along_x, airlock))
                .collect(),
            doors_drawn: true,
            airlocks: layout.airlocks,
            stills: Stills::from_extras(&layout.extras),
            interior,
            worktops: core::iter::once(Worktop::new(counter, board, drawer))
                .chain(layout.more.worktops.iter().map(|&c| {
                    let (board, drawer, _) = galley_faces(c, c);
                    Worktop::new(c, board, drawer)
                }))
                .collect(),
            hobs: core::iter::once(layout.stove)
                .chain(layout.more.hobs.iter().copied())
                .map(Hob::new)
                .collect(),
            fridges: core::iter::once(layout.fridge)
                .chain(layout.more.fridges.iter().copied())
                .map(Fridge::new)
                .collect(),
            dishwashers: core::iter::once(dishwasher)
                .chain(layout.more.dishwashers.iter().map(|&body| {
                    let (_, _, face) = galley_faces(body, body);
                    let mut washer = Dishwasher::at(face);
                    washer.body = Some(body);
                    washer
                }))
                .collect(),
            lockers: core::iter::once(layout.locker)
                .chain(layout.more.lockers.iter().copied())
                .map(Locker::new)
                .collect(),
            table: Some(layout.table),
            chairs,
            beds,
            stand_in_bed,
            bunk_of: Vec::new(),
            heads: core::iter::once((layout.toilet, layout.sink))
                .chain(layout.more.heads.iter().copied())
                .map(|(toilet, sink)| Heads::new(toilet, sink))
                .collect(),
            bays,
            showers: layout
                .shower
                .into_iter()
                .chain(layout.more.showers.iter().copied())
                .collect(),
            sight,
            benches: layout.benches,
            suit_locker: layout.suit_locker,
            gangway: layout.gangway,
            outside: layout.outside,
            hull: layout.hull,
            shelves: layout.shelves,
            desks: layout.desks,
            research: layout.research,
            builds: Vec::new(),
            suit_ok: Vec::new(),
            built: Vec::new(),
            deployed: Vec::new(),
            ferries: Vec::new(),
            ferry_picked: Vec::new(),
            ferry_dropped: Vec::new(),
            ferry_returned: Vec::new(),
            rocks_version: 0,
            plane: layout.plane,
            crafted: Vec::new(),
            blood: Blood::new(interior),
            dressed: Vec::new(),
            medkits: 0,
            medkits_used: 0,
            pack_kits: Vec::new(),
            pack_kits_used: Vec::new(),
            kit_stands: Vec::new(),
            treated: Vec::new(),
            weapons_down: Vec::new(),
            next_weapon_down: 0,
            picked_up: Vec::new(),
            crew: Vec::new(),
            cues: Vec::new(),
        }
    }

    /// The same room laid out again from a design that has changed under
    /// it — a part built — with everything that is **state** kept: the
    /// blood on the deck, the doors' locks and leaves, whose bunk is whose.
    /// Geometry is taken from the new layout; a bunk whose frame did not
    /// move is the same bunk, and anything new goes on the end, so a bunk
    /// index the crew already hold still names the same bed. The nav grids
    /// and the blockers are the game's to rebuild — `Game::relayout` —
    /// since only it knows about the locked doors.
    pub fn relayout(&mut self, layout: Layout) {
        let interior = layout.interior;
        let counter = layout.counter;
        self.bounds = layout.bounds;
        // The deck: the blood on it comes across by position, so a deck that
        // grew a tile keeps every stain where it was.
        if interior != self.interior {
            self.blood = self.blood.resized(interior);
        }
        self.interior = interior;
        let chairs = layout_chairs(&layout);
        self.others = layout.others.clone();
        // The walls moved: the mask is traced again from the new ones the
        // first time anybody looks. The daylight is the world's word, not
        // the layout's — a settlement's ground is under a sky whatever is
        // built on it — so it is carried across, where a lamp's health is
        // put back by the world (`Game::set_lamp_health`).
        let daylight = self.sight.daylight();
        // The plain is the room's own — what the crew have seen of it stays
        // seen — with its box moved to the new layout's.
        if let (Some(mine), Some(theirs)) = (self.plane.as_mut(), layout.plane.as_ref()) {
            mine.set_deck(theirs.deck());
            // Its picture was marched over the old deck's walls.
            mine.forget_views();
        }
        let fogged: Vec<Rect> = if self.plane.is_some() {
            vec![interior]
        } else {
            layout.hull.clone()
        };
        let range = self.sight.range();
        self.sight = Sight::new(layout.bounds, interior, TILE, &layout.opaque, &fogged);
        self.sight.set_range(range);
        self.sight.set_cover(&layout.cover);
        self.sight.set_tall(&layout.tall);
        self.sight.set_lights(&layout.lights);
        self.sight.set_daylight(daylight);
        // The doors, by opening: one that was there keeps its state, one
        // that is new starts shut.
        let mut doors: Vec<Door> = Vec::with_capacity(layout.doors.len());
        let mut old_doors = core::mem::take(&mut self.doors);
        for (rect, along_x, airlock) in layout.doors.iter().copied() {
            match old_doors.iter().position(|d| d.rect == rect) {
                Some(i) => doors.push(old_doors.swap_remove(i)),
                None => doors.push(Door::of_kind(rect, along_x, airlock)),
            }
        }
        self.doors = doors;
        self.airlocks = layout.airlocks.clone();
        self.stills = Stills::from_extras(&layout.extras);
        // The furniture, laid out afresh: it has nothing to remember.
        let (board, drawer, dish_face) = galley_faces(counter, layout.dishwasher);
        self.worktops = core::iter::once(Worktop::new(counter, board, drawer))
            .chain(layout.more.worktops.iter().map(|&c| {
                let (b, d, _) = galley_faces(c, c);
                Worktop::new(c, b, d)
            }))
            .collect();
        self.hobs = core::iter::once(layout.stove)
            .chain(layout.more.hobs.iter().copied())
            .map(Hob::new)
            .collect();
        self.fridges = core::iter::once(layout.fridge)
            .chain(layout.more.fridges.iter().copied())
            .map(Fridge::new)
            .collect();
        self.dishwashers = core::iter::once((layout.dishwasher, dish_face))
            .chain(layout.more.dishwashers.iter().map(|&body| {
                let (_, _, face) = galley_faces(body, body);
                (body, face)
            }))
            .map(|(body, face)| {
                let mut washer = Dishwasher::at(face);
                washer.body = Some(body);
                washer
            })
            .collect();
        self.lockers = core::iter::once(layout.locker)
            .chain(layout.more.lockers.iter().copied())
            .map(Locker::new)
            .collect();
        self.showers = layout
            .shower
            .into_iter()
            .chain(layout.more.showers.iter().copied())
            .collect();
        self.table = Some(layout.table);
        self.chairs = chairs;
        // The bunks keep their order — index is identity — and a bunk that
        // did not move keeps the side it was laid out with.
        let mut beds: Vec<Berth> = Vec::with_capacity(layout.beds.len());
        let old_frames: Vec<Rect> = self.beds.iter().map(|b| b.frame).collect();
        let mut old_beds = core::mem::take(&mut self.beds);
        for frame in &layout.beds {
            match old_beds.iter().position(|b| b.frame == *frame) {
                Some(i) => beds.push(old_beds.remove(i)),
                None => beds.push(Berth::new(*frame, bed_side(*frame, interior))),
            }
        }
        // Whose bunk is which follows the bunk by its frame: a bunk taken
        // out of the design ahead of somebody's shifts the rest down, and
        // that somebody's number would otherwise name a stranger's bunk —
        // or none. A bunk gone is a Bim with none.
        let was_stand_in = self.stand_in_bed;
        self.stand_in_bed = beds.is_empty();
        if self.stand_in_bed {
            beds.push(Berth::new(counter, bed_side(counter, interior)));
        }
        for bed in &mut self.bunk_of {
            *bed = bed
                .filter(|_| !was_stand_in)
                .and_then(|b| old_frames.get(b).copied())
                .and_then(|frame| beds.iter().position(|b| b.frame == frame));
        }
        self.beds = beds;
        self.bays = bays_of(&layout);
        self.heads = core::iter::once((layout.toilet, layout.sink))
            .chain(layout.more.heads.iter().copied())
            .map(|(toilet, sink)| Heads::new(toilet, sink))
            .collect();
        self.benches = layout.benches;
        self.suit_locker = layout.suit_locker;
        self.gangway = layout.gangway;
        self.outside = layout.outside;
        self.hull = layout.hull;
        self.shelves = layout.shelves;
        self.desks = layout.desks;
        self.research = layout.research;
    }

    // --- geometry the rest of the game asks about ------------------------

    /// Where the Bim stands at a cold store — a container in the hold,
    /// reached from the Nearby strip: below it, to the side its door swings
    /// away from. `None` in a room with none.
    pub fn fridge_station(&self, i: usize) -> Option<Vec2> {
        let f = self.fridges.get(i).or(self.fridges.last())?.frame;
        Some(vec2(f.center().x - 6.0, f.max.y + STAND_OFF))
    }

    /// Where the Bim stands at the suit locker, or nowhere: a room without
    /// one.
    pub fn suit_locker_station(&self) -> Option<Vec2> {
        self.suit_locker.map(|(_, at)| at)
    }

    /// Which way the Bim faces at the suit locker: into it.
    pub fn suit_locker_facing(&self) -> f32 {
        match self.suit_locker {
            Some((frame, at)) => {
                let d = frame.center() - at;
                d.y.atan2(d.x)
            }
            None => 0.0,
        }
    }

    /// Which way out of the port is: from the gangway towards the spot
    /// outside.
    pub fn port_facing(&self) -> f32 {
        match (self.gangway, self.outside) {
            (Some(inside), Some(out)) => {
                let d = out - inside;
                d.y.atan2(d.x)
            }
            _ => 0.0,
        }
    }

    /// The tile the room's nav grid is phased to, where the room is laid
    /// out on tiles: a ship's is, a bare room is not. See `nav::Nav::tiled`
    /// for why that matters to a one-tile corridor.
    pub fn nav_tile(&self) -> Option<f32> {
        if self.shell { None } else { Some(TILE) }
    }

    /// The door itself: where the Bim's hand has to end up.
    fn switch_target(&self, which: Switch) -> Vec2 {
        match which {
            Switch::Door(i, _) => self.doors[i.min(self.doors.len() - 1)].rect.center(),
        }
    }

    /// Where it stands to reach that: the panel on the side it set off
    /// from.
    pub fn switch_station(&self, which: Switch, from: Vec2) -> Vec2 {
        match which {
            Switch::Door(i, _) => self.doors[i.min(self.doors.len() - 1)].station(from, STAND_OFF),
        }
    }

    /// Which way it turns to work it: towards the thing itself.
    pub fn switch_facing(&self, which: Switch, from: Vec2) -> f32 {
        (self.switch_target(which) - self.switch_station(which, from)).angle()
    }

    /// Work it. Called the instant the Bim's hand reaches it, so what it does
    /// happens then and not when the player asked.
    pub fn work_switch(&mut self, which: Switch) {
        match which {
            Switch::Door(i, order) => {
                if let Some(door) = self.doors.get_mut(i) {
                    door.order(order);
                }
            }
        }
    }

    /// The powered doors a route may not be planned through: the locked
    /// ones. What the navigation grids are rebuilt with when it changes.
    pub fn locked_doors(&self) -> Vec<Rect> {
        self.doors
            .iter()
            .filter(|d| !d.passable())
            .map(|d| d.rect)
            .collect()
    }

    /// What an eye stops at that a step may not: every powered door whose
    /// leaves are shut, locked or not; and every airlock with nobody within
    /// a door's `REACH` of it, or locked with its leaves shut. An airlock
    /// is a door now, but to the eye it is still the passage it was: open
    /// the instant somebody is at it, since the ship painter's
    /// `airlock_ajar` is a picture and the leaves' travel is not what the
    /// hunt through it turns on — a chase read off the leaves lost its
    /// quarry in the fifth of a second they took to part.
    pub fn shut_leaves(&self, bodies: &[Vec2]) -> Vec<Rect> {
        self.doors
            .iter()
            .filter(|d| {
                if d.airlock {
                    let near = bodies
                        .iter()
                        .any(|&p| d.rect.expand(crate::door::REACH).contains(p));
                    !near || (d.locked && !d.is_open())
                } else {
                    !d.is_open()
                }
            })
            .map(|d| d.rect)
            .collect()
    }

    /// The powered doors a body walks into right now: locked and shut.
    pub fn shut_doors(&self) -> Vec<Rect> {
        self.doors
            .iter()
            .filter(|d| d.blocks())
            .map(|d| d.rect)
            .collect()
    }

    /// Which powered door a point is in, with a little slack, as a click
    /// wants; `None` off all of them.
    pub fn door_at(&self, p: Vec2) -> Option<usize> {
        self.doors
            .iter()
            .position(|d| d.rect.expand(4.0).contains(p))
    }

    /// One frame of the powered doors, for the bodies standing where
    /// `bodies` says.
    pub fn update_doors(&mut self, dt: f32, bodies: &[Vec2]) {
        for door in &mut self.doors {
            if let Some(cue) = door.update(dt, bodies) {
                self.cues.push(Cued {
                    cue,
                    at: door.rect.center(),
                });
            }
        }
    }

    /// How many bunks there are: `beds` less the stand-in.
    pub fn bunks(&self) -> usize {
        if self.stand_in_bed {
            0
        } else {
            self.beds.len()
        }
    }

    /// Which bunk Bim `who` was dealt, or `None`. See [`Room::bunk_of`].
    pub fn bed_of(&self, who: usize) -> Option<usize> {
        self.bunk_of.get(who).copied().flatten()
    }

    /// Whose bunk `bed` is, or `None` for one nobody has.
    pub fn bed_owner(&self, bed: usize) -> Option<usize> {
        self.bunk_of.iter().position(|&b| b == Some(bed))
    }

    /// The deck beside bunk `bed`, clamped so an index that has wandered
    /// cannot panic in the middle of a frame.
    pub fn bed_station(&self, bed: usize) -> Vec2 {
        self.beds[bed.min(self.beds.len() - 1)].station()
    }

    /// Everything fixed that a body has to walk round: every piece of
    /// furniture's frame, in the order the room has always listed them —
    /// the first worktop, hob and cold store, the table, the bunks, the
    /// first bay, then the other bays — and everything else after. The
    /// push-out walks them in order, and the nav grids are built on them,
    /// so the order and the frames are what every route and every fight
    /// is laid on.
    pub fn solids(&self) -> Vec<Rect> {
        let mut all: Vec<Rect> = Vec::new();
        all.extend(self.worktops.first().map(|w| w.frame));
        all.extend(self.hobs.first().map(|h| h.frame));
        all.extend(self.fridges.first().map(|f| f.frame));
        all.extend(self.table);
        all.extend(self.beds.iter().map(|b| b.frame));
        all.extend(self.bays.iter().map(|b| b.frame));
        all.extend(self.others.iter().copied());
        all
    }

    /// Which fixture, if any, is under a click. The furniture comes first,
    /// with a click's slack round each, and is nothing to act on — so a
    /// click on a hob is not a click on a bench beside it.
    pub fn hit(&self, p: Vec2) -> u32 {
        let furniture = self.hobs.iter().any(|h| h.frame.contains(p))
            || self.fridges.iter().any(|f| f.frame.expand(4.0).contains(p))
            || self
                .dishwashers
                .iter()
                .any(|d| d.face.expand(6.0).contains(p))
            || (!self.stand_in_bed && self.beds.iter().any(|b| b.frame.expand(4.0).contains(p)))
            || self.bays.iter().any(|b| b.frame.expand(6.0).contains(p))
            || self.lockers.iter().any(|l| l.frame.expand(8.0).contains(p));
        if furniture {
            HIT_NONE
        } else if self.door_at(p).is_some() {
            HIT_SHIP_DOOR
        } else if self
            .showers
            .iter()
            .any(|(frame, _)| frame.expand(6.0).contains(p))
        {
            HIT_NONE
        } else if self.bench_at(p).is_some() {
            HIT_BENCH
        } else if self.shelf_at(p).is_some() {
            HIT_SHELF
        } else if self.desk_at(p).is_some() {
            HIT_DESK
        } else if self.research_at(p).is_some() {
            HIT_RESEARCH
        } else {
            HIT_NONE
        }
    }

    /// Which workstation a click landed on, by index into `benches`.
    pub fn bench_at(&self, p: Vec2) -> Option<usize> {
        self.benches
            .iter()
            .position(|b| b.frame.expand(4.0).contains(p))
    }

    /// Which shelf a click landed on, by index into `shelves`.
    pub fn shelf_at(&self, p: Vec2) -> Option<usize> {
        self.shelves
            .iter()
            .position(|(frame, _)| frame.expand(4.0).contains(p))
    }

    /// Which trading desk a click landed on, by index into `desks`.
    pub fn desk_at(&self, p: Vec2) -> Option<usize> {
        self.desks
            .iter()
            .position(|(frame, _)| frame.expand(4.0).contains(p))
    }

    /// Which research desk a click landed on, by index into `research`.
    pub fn research_at(&self, p: Vec2) -> Option<usize> {
        self.research
            .iter()
            .position(|(frame, _)| frame.expand(4.0).contains(p))
    }

    /// What is at a point, in the `SPOT_` codes. Never `None`: everywhere the
    /// pointer can be is *something*, even if that something is the hull.
    ///
    /// Order matters the same way it does in [`Room::hit`] — the hob and the
    /// board both sit within the counter run, so they are tested before it —
    /// and the first heads is taken whole, the deck between its pan and its
    /// basin included.
    pub fn spot(&self, p: Vec2) -> u32 {
        if let Some(heads) = self.heads.first()
            && heads.shell.contains(p)
        {
            return if heads.toilet.contains(p) {
                SPOT_TOILET
            } else if heads.sink.contains(p) {
                SPOT_BASIN
            } else {
                SPOT_HEADS_DECK
            };
        }
        if self.hobs.iter().any(|h| h.frame.contains(p)) {
            SPOT_HOB
        } else if self.worktops.iter().any(|w| w.board.contains(p)) {
            SPOT_BOARD
        } else if self.fridges.iter().any(|f| f.frame.contains(p)) {
            SPOT_FRIDGE
        } else if self.dishwashers.iter().any(|d| d.face.contains(p)) {
            SPOT_DISHWASHER
        } else if self.worktops.iter().any(|w| w.frame.contains(p)) {
            SPOT_WORKTOP
        } else if self.bays.iter().any(|b| b.frame.contains(p)) {
            SPOT_BAY
        } else if self.lockers.iter().any(|l| l.frame.contains(p)) {
            SPOT_LOCKER
        } else if self.beds.iter().any(|b| b.frame.contains(p)) {
            SPOT_BUNK
        } else if self
            .chairs
            .iter()
            .any(|&c| Rect::from_center_size(c, CHAIR_SIZE).contains(p))
        {
            SPOT_CHAIR
        } else if self.table.is_some_and(|t| t.contains(p)) {
            SPOT_TABLE
        } else if self.doors.iter().any(|d| d.rect.contains(p)) {
            SPOT_SHIP_DOOR
        } else if self.showers.iter().any(|(frame, _)| frame.contains(p)) {
            SPOT_SHOWER
        } else if self.interior.contains(p) {
            SPOT_DECK
        } else if self.bounds.contains(p) {
            SPOT_BULKHEAD
        } else {
            SPOT_NOTHING
        }
    }

    /// Where a `SPOT_` code *is*, for ringing it on the deck: every fixture
    /// of that kind, each its own rect.
    ///
    /// The inverse of [`Room::spot`], and only defined for the things that
    /// are objects in places. Deck plating and bulkheads have no rect — they
    /// are everywhere the furniture is not — so they answer nothing, and a
    /// panel that points at one gets nothing rather than a ring round the
    /// whole room. The chairs and the beds come in adjacent pairs, and
    /// ringing both would read as one enormous fixture, so the first stands
    /// for the pair.
    pub fn spot_rects(&self, spot: u32) -> Vec<Rect> {
        match spot {
            SPOT_WORKTOP => self.worktops.iter().map(|w| w.frame).collect(),
            SPOT_BOARD => self.worktops.iter().map(|w| w.board).collect(),
            SPOT_FRIDGE => self.fridges.iter().map(|f| f.frame).collect(),
            SPOT_HOB => self.hobs.iter().map(|h| h.frame).collect(),
            SPOT_DISHWASHER => self.dishwashers.iter().map(|d| d.face).collect(),
            SPOT_TABLE => self.table.into_iter().collect(),
            SPOT_CHAIR => self
                .chairs
                .first()
                .map(|&c| Rect::from_center_size(c, CHAIR_SIZE))
                .into_iter()
                .collect(),
            SPOT_BUNK => self.beds.first().map(|b| b.frame).into_iter().collect(),
            SPOT_BAY => self.bays.iter().map(|b| b.frame).collect(),
            SPOT_LOCKER => self.lockers.iter().map(|l| l.frame).collect(),
            SPOT_TOILET => self.heads.first().map(|h| h.toilet).into_iter().collect(),
            SPOT_BASIN => self.heads.first().map(|h| h.sink).into_iter().collect(),
            SPOT_BENCH => self.benches.iter().map(|b| b.frame).collect(),
            SPOT_SHOWER => self.showers.iter().map(|(frame, _)| *frame).collect(),
            SPOT_SUIT_LOCKER => self
                .suit_locker
                .map(|(frame, _)| frame)
                .into_iter()
                .collect(),
            SPOT_RESEARCH => self.research.iter().map(|(frame, _)| *frame).collect(),
            _ => Vec::new(),
        }
    }

    /// The first of [`Room::spot_rects`], for a reader that wants one place
    /// to look.
    #[allow(dead_code)]
    pub fn spot_rect(&self, spot: u32) -> Option<Rect> {
        self.spot_rects(spot).first().copied()
    }

    /// Whether a Bim has a medkit of its own in its pack — what the world
    /// last said (`pack_kits`). A treatment by one that has is opened
    /// where it stands: its own kit before a new one off a shelf.
    pub fn carries_kit(&self, who: usize) -> bool {
        self.pack_kits.get(who).is_some_and(|&n| n > 0)
    }

    /// One frame of the room itself: the lamps' flicker — a shot one for a
    /// moment, a failing one now and then. See `sight::Lamp`.
    pub fn update(&mut self, dt: f32) {
        self.sight.tick_lamps(dt);
    }

    // --- drawing ---------------------------------------------------------

    /// The deck plate and walls of a bare room, then every piece of
    /// furniture, the extras and the doors. The bunks' bedding is
    /// [`Room::draw_over`]'s, after the bodies.
    pub fn draw(&self, list: &mut DrawList) {
        if self.shell {
            self.draw_floor(list);
        }
        for worktop in &self.worktops {
            worktop.draw(list);
        }
        for washer in &self.dishwashers {
            washer.draw(list);
        }
        for hob in &self.hobs {
            hob.draw(list);
        }
        for fridge in &self.fridges {
            fridge.draw(list);
        }
        self.draw_seating(list, true, true);
        for locker in &self.lockers {
            locker.draw(list);
        }
        for bed in &self.beds {
            bed.draw(list);
        }
        for bay in &self.bays {
            bay.draw(list);
        }
        for heads in &self.heads {
            heads.draw(list, true, true);
        }
        self.stills.draw(list);
        if self.doors_drawn {
            for door in &self.doors {
                door.draw(list);
            }
        }
    }

    /// The fixtures alone, and only the ones asked for: the picture the
    /// designer puts on a ship it is building. See [`Fixtures`]. A bunk's
    /// bedding goes on with it, there being nobody on the deck beside it.
    pub fn draw_fixtures(&self, list: &mut DrawList, has: Fixtures) {
        if has.counter {
            for worktop in &self.worktops {
                worktop.draw(list);
            }
        }
        if has.dishwasher {
            for washer in &self.dishwashers {
                washer.draw(list);
            }
        }
        if has.stove {
            for hob in &self.hobs {
                hob.draw(list);
            }
        }
        if has.fridge {
            for fridge in &self.fridges {
                fridge.draw(list);
            }
        }
        if has.table || has.chairs {
            self.draw_seating(list, has.table, has.chairs);
        }
        if has.locker {
            for locker in &self.lockers {
                locker.draw(list);
            }
        }
        if has.beds {
            for bed in &self.beds {
                bed.draw(list);
                bed.draw_bedding(list);
            }
        }
        if has.bay {
            for bay in &self.bays {
                bay.draw(list);
            }
        }
        if has.toilet || has.basin {
            for heads in &self.heads {
                heads.draw(list, has.toilet, has.basin);
            }
        }
        if has.doors {
            for door in &self.doors {
                door.draw(list);
            }
        }
        // And every extra, which is there by definition: a layout only
        // lists what the design has.
        self.stills.draw(list);
    }

    /// The parts of the room that belong *above* the bodies: the bunks'
    /// bedding.
    pub fn draw_over(&self, list: &mut DrawList) {
        for bed in &self.beds {
            bed.draw_bedding(list);
        }
    }

    /// The table and its chairs, either without the other for a room that
    /// has only one of them yet.
    fn draw_seating(&self, list: &mut DrawList, table: bool, chairs: bool) {
        let Some(t) = self.table else {
            return;
        };
        if chairs {
            fixtures::draw_chairs(list, &self.chairs, t);
        }
        if table {
            fixtures::draw_table_top(list, t);
        }
    }

    /// A bare room's deck plate, the seams on it, its walls and the light
    /// run at their foot.
    fn draw_floor(&self, list: &mut DrawList) {
        let room = self.bounds;
        list.rect(room.center(), room.size(), 0.0, 0.0, WALL_C);
        list.rect(
            self.interior.center(),
            self.interior.size() + vec2(6.0, 6.0),
            0.0,
            0.0,
            WALL_TRIM,
        );
        list.rect(
            self.interior.center(),
            self.interior.size(),
            0.0,
            0.0,
            FLOOR,
        );

        let mut x = self.interior.min.x + TILE;
        while x < self.interior.max.x {
            list.line(
                vec2(x, self.interior.min.y),
                vec2(x, self.interior.max.y),
                1.0,
                TILE_LINE,
            );
            x += TILE;
        }
        let mut y = self.interior.min.y + TILE;
        while y < self.interior.max.y {
            list.line(
                vec2(self.interior.min.x, y),
                vec2(self.interior.max.x, y),
                1.0,
                TILE_LINE,
            );
            y += TILE;
        }

        // A light run at the foot of the hull, all the way round. It is the
        // one thing that lights the deck, so it sets how the room reads.
        let skirt = self.interior.expand(-3.0);
        list.stroke_rect(
            skirt.center(),
            skirt.size(),
            0.0,
            0.0,
            2.5,
            GLOW.alpha(0.16),
        );
        // Brighter at intervals, where the fittings are.
        let stops = 9;
        for i in 0..stops {
            let x = lerp(
                skirt.min.x + 40.0,
                skirt.max.x - 40.0,
                i as f32 / (stops - 1) as f32,
            );
            list.rect(
                vec2(x, skirt.max.y),
                vec2(26.0, 3.0),
                0.0,
                1.5,
                GLOW.alpha(0.45),
            );
            list.rect(
                vec2(x, skirt.min.y),
                vec2(26.0, 3.0),
                0.0,
                1.5,
                GLOW.alpha(0.30),
            );
        }
    }
}

/// The layout's bays, its own first, then the rest, then the fields.
fn bays_of(layout: &Layout) -> Vec<Bay> {
    core::iter::once((layout.bay, layout.bay_side))
        .chain(layout.more.bays.iter().copied())
        .map(|(frame, side)| Bay::at(frame, side))
        .chain(
            layout
                .more
                .fields
                .iter()
                .map(|&(frame, side)| Bay::field(frame, side)),
        )
        .collect()
}
