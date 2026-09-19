//! World state: the room, the Bim in it, the job it is doing, and the
//! per-frame draw list handed to the renderer.

use crate::bim::{Bim, CREW, PLAYER, TALKS_ABOUT, TRAIL_LIFE};
use crate::character::{ACCENT, Action, BODY_MARGIN, Held, PICK_RADIUS, SWING_TIME, Worn};
use crate::clock::MINUTES_PER_SECOND;
use crate::clock::{self, Clock};
use crate::combat::{
    ArmourKind, Blow, Combat, FIST_DAMAGE, Gear, Hit, Item, LOOT_CELLS, LootCell, MELEE_PERIOD,
    MELEE_RANGE, PACK_CELLS, Piece, Shot, Tactics, WeaponKind, WeaponStats,
};
use crate::door;
use crate::draw::{Color, DrawList};
use crate::filth;
use crate::health::{Health, Malnutrition, Part};
use crate::hydro;
use crate::manager::{Manager, Stock};
use crate::math::{Rect, Vec2, clamp, vec2};
use crate::memory::What;
use crate::nav::{self, Maps, Nav};
use crate::needs::{Need, Urge};
use crate::rng::Rng;
use crate::room::{
    self, Dish, GLOW, HIT_BIM, HIT_BODY, HIT_NONE, HIT_VISITOR, ROOM_H, ROOM_W, Room, Switch, TILE,
    WARN,
};
use crate::schedule::{IGNORE_ABOVE, Schedule, Slot, WAKE_AT};
use crate::sight::{Fog, Stance};
use crate::social;
use crate::task::{self, Kind, SLEEP_MINUTES, Saved, Task};
use crate::work::{self, Job, Priorities};

const TRAIL: Color = ACCENT;
const MARQUEE_EDGE: Color = ACCENT;
const MARQUEE_FILL: Color = Color::rgba(0.50, 0.82, 0.66, 0.10);

/// How long the ping at an ordered destination lasts.
const MARKER_LIFE: f32 = 0.7;

/// How long a body on somebody else's deck stays drawn after the crew
/// last saw it, in seconds at 1x: it walks out of view and is a moment
/// fading, rather than winking out at the bulkhead.
pub const SEEN_FOR: f32 = 2.0;

/// How long the flash a hit puts on a body lasts, in seconds.
const HIT_FLASH: f32 = 0.22;

/// How a shot on a body went — see [`Game::wound`]. All nought for a shot
/// on nobody: a dead Bim, or one that is not there.
#[derive(Clone, Copy, PartialEq, Debug, Default)]
pub struct WoundOutcome {
    /// The shot took a leg — see `Health::shot`.
    pub leg_lost: bool,
    /// What the armour on the part took off it, its protection included.
    pub absorbed: f32,
    /// What reached the body and opened a wound.
    pub through: f32,
    /// The piece on the part is broken by this shot.
    pub piece_broke: bool,
}

/// A container the crew can reach into, for [`Game::container_spot`]: a
/// workstation (the armoury among them), a shelf or a cold store, each
/// by its index in the room's list of that kind.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Container {
    Bench(usize),
    Shelf(usize),
    Fridge(usize),
}
/// The flash itself: the colour of what hit it, lit white in the middle.
const HIT: Color = Color::rgb(1.0, 0.95, 0.85);

/// How often an enemy at war chooses where to stand, in seconds at 1x,
/// each on its own clock. Often enough to follow a crew member who moves,
/// seldom enough that a room of enemies is not a trace of every tile in
/// reach every frame — see `combat::Tactics`.
pub const PLAN_EVERY: f32 = 1.5;

/// How near an enemy has to come, in tiles, for the crew's **alarm**: every
/// crew member but the one the player steers takes arms and fights it,
/// the way an enemy's people do. A hundred tiles is any enemy on the same
/// station.
pub const ALARM_RANGE: f32 = 100.0;
/// How long the alarm holds after a crew member is hit with no enemy in
/// range, in seconds at 1x.
pub const ALARM_HOLD: f32 = 30.0;
/// Where the crew stand round the player's Bim under the alarm, in tiles
/// off it, by rank: behind and beside, a body's width apart, so the ring
/// is a squad at its back rather than a crowd on top of it.
pub const GATHER_SLOTS: [(f32, f32); 6] = [
    (-1.5, 0.0),
    (1.5, 0.0),
    (0.0, 1.5),
    (-1.5, 1.5),
    (1.5, 1.5),
    (0.0, 3.0),
];
/// How far off its slot a gathered crew member may stand before it walks
/// to it again, in tiles.
pub const GATHER_SLACK: f32 = 1.0;

/// How far outside a fixture its highlight ring sits. Enough to read as a ring
/// round the thing rather than an outline drawn on it.
const HIGHLIGHT_MARGIN: f32 = 10.0;

/// How close one Bim has to be to another to count as having seen what
/// happened to it. About a third of the room: across the galley, not through
/// the bulkhead into the heads.
const WITHIN_SIGHT: f32 = 260.0;

/// How long the bathroom door stands open after the Bim has walked through it
/// before sliding shut again. Doors aboard shut themselves; the chains that
/// close one by hand simply get there first.
const DOOR_SHUT_AFTER: f32 = 5.0;
/// How much room the Bim needs either side of the doorway to count as clear
/// of it — its own half-width, near enough.
const DOOR_CLEARANCE: f32 = 16.0;

/// How close two Bims have to be to be squeezing past one another. A little
/// under two body margins: shoulder to shoulder in the gangway beside the
/// table, not merely in the same half of the room.
const CREW_CLEARANCE: f32 = 2.0 * BODY_MARGIN - 8.0;

/// A walk that has covered less than this, in units a second, for this
/// long, is a body the push-out has stopped dead — see `Game::unstick`. A
/// marching Bim at its slowest, crowded and on its final approach, still
/// makes a good deal more than a unit a second.
const STUCK_STEP: f32 = 1.0;
const STUCK_AFTER: f32 = 1.0;

/// How far off its post a Bim may drift before it walks back: about half a
/// cell of the nav grid, which is as close as a route can be relied on to
/// leave it, plus a shove from a shipmate squeezing past.
const POST_SLACK: f32 = 20.0;

/// How long one of them holds the floor before the other gets a word in, in
/// game minutes. Read off the ship's clock rather than off either chain, so
/// the two always agree about whose turn it is and exactly one bubble is up.
const TAKES_A_TURN: f32 = 2.0;

/// How far apart two Bims stand to talk, measured centre to centre.
///
/// A body is `BODY_MARGIN` across the radius, so anything under about fifty
/// puts them inside one another. This leaves a clear gap between them and,
/// just as much to the point, keeps the two *names* apart: the host paints
/// those a body's height above each head, and a pair standing closer than
/// this have their labels written across each other.
const TALKING_GAP: f32 = 76.0;

/// How far back through a diary a Bim will reach for something to say. A few
/// days of entries, so the conversation is about the week rather than about
/// last month. The errands it has finished are capped separately, by
/// `bim::TALKS_ABOUT`.
const RECENT_ENOUGH: usize = 40;

/// What squeezing past costs, as a fraction of the usual pace.
///
/// Bodies do not block one another — they pass through, which is the one thing
/// that cannot deadlock. A route is planned once and never replanned here, so
/// anything that stops a Bim getting where its line goes risks two of them
/// standing nose to nose for ever with errands on both agendas; letting them
/// overlap gives that failure nowhere to happen. The cost is speed instead:
/// close quarters are slow, and two Bims edging round each other in a gangway
/// take noticeably longer than one.
const CROWDED_PACE: f32 = 0.70;

/// How long the Bim hops about for, and how long a bout of sickness takes.
const FIDGET_TIME: f32 = 1.4;
const RETCH_TIME: f32 = 2.2;

/// How often a Bim that wants to be somewhere cleaner looks for somewhere to
/// go, and how far it looks, in tiles. Rechecking every frame would have it
/// replanning on the spot; this is often enough to look decisive.
const FLEE_EVERY: f32 = 2.0;
const FLEE_LOOK: i32 = 4;

/// How long a nod-off lasts, and how often one comes over a Bim that is past
/// the point of staying upright — about one in every three quarters of an hour
/// on its feet, so it spends a fifth of that time out cold.
const NAP_MINUTES: f32 = 15.0;
const NAP_EVERY: f32 = 45.0;

/// The galley, for judging what comes out of it: every tile within two of
/// the hob, and for each of them with a mess on it — as bad as a wetting,
/// see `filth::SPOILS_FOOD` — a one-in-five chance the meal is bad. Rolled once per meal, the moment the pot finishes
/// cooking — a reheated stew included, since it is the hob it is warmed on
/// — or a bowl is filled off the board.
const GALLEY_REACH: i32 = 2;
const BAD_FOOD_PER_DIRTY_TILE: f32 = 0.20;
/// What a bad meal does, and for how long: the restroom need runs three
/// times as fast, and reaching nothing is the accident at once — there is
/// no holding on. Two days to get over it.
const POISONING_LASTS: f32 = 2.0 * clock::DAY;
const POISONED_PURGE: f32 = 3.0;

/// How far off the helm's seat a post still counts as the helm: a route is
/// snapped to the nearest free cell, and the world's own reach is a tile.
const HELM_SLACK: f32 = 52.0;

/// What the Bim is doing, as plain codes the host turns into words. A nap and
/// a full sleep are the same chain but read differently, so they count as two.
pub const JOB_MEAL: u32 = 1;
pub const JOB_STOVE: u32 = 2;
pub const JOB_NAP: u32 = 3;
pub const JOB_SLEEP: u32 = 4;
pub const JOB_HEADS: u32 = 5;
pub const JOB_FRIDGE: u32 = 6;
pub const JOB_BATH_DOOR: u32 = 7;
pub const JOB_BATH_LOCK: u32 = 8;
pub const JOB_DISHWASHER: u32 = 9;
pub const JOB_BOWL: u32 = 10;
pub const JOB_TEND: u32 = 11;
pub const JOB_LEFTOVERS: u32 = 12;
pub const JOB_CLEAN: u32 = 13;
pub const JOB_CHAT: u32 = 14;
pub const JOB_STEW: u32 = 15;
pub const JOB_REHEAT: u32 = 16;
pub const JOB_SHOWER: u32 = 17;
pub const JOB_CRAFT: u32 = 18;
pub const JOB_EVA: u32 = 19;
pub const JOB_HAUL: u32 = 20;
pub const JOB_BUILD: u32 = 21;
pub const JOB_BANDAGE: u32 = 22;

fn job_code(kind: Kind, rest_minutes: f32) -> u32 {
    match kind {
        Kind::Meal(Dish::Stew) => JOB_MEAL,
        Kind::Meal(Dish::Bowl) => JOB_BOWL,
        Kind::Batch => JOB_STEW,
        Kind::Reheat => JOB_REHEAT,
        Kind::Rest if rest_minutes >= SLEEP_MINUTES => JOB_SLEEP,
        Kind::Rest => JOB_NAP,
        Kind::Heads => JOB_HEADS,
        Kind::Switch(Switch::Hob(_)) => JOB_STOVE,
        Kind::Switch(Switch::FridgeDoor(_)) => JOB_FRIDGE,
        Kind::Switch(Switch::BathDoor(_)) => JOB_BATH_DOOR,
        Kind::Switch(Switch::BathLock(_)) => JOB_BATH_LOCK,
        // A ship's door is worked the same two ways, and named the same.
        Kind::Switch(Switch::Door(_, door::Order::Open | door::Order::Close)) => JOB_BATH_DOOR,
        Kind::Switch(Switch::Door(_, door::Order::Lock | door::Order::Unlock)) => JOB_BATH_LOCK,
        Kind::Switch(Switch::Dishwasher(_)) => JOB_DISHWASHER,
        Kind::Tend { .. } => JOB_TEND,
        Kind::Leftovers => JOB_LEFTOVERS,
        Kind::Clean => JOB_CLEAN,
        Kind::Chat => JOB_CHAT,
        Kind::Shower => JOB_SHOWER,
        Kind::Craft { .. } => JOB_CRAFT,
        Kind::Eva => JOB_EVA,
        Kind::Haul { .. } => JOB_HAUL,
        Kind::Build { .. } => JOB_BUILD,
        Kind::Bandage { .. } => JOB_BANDAGE,
    }
}

/// The room after dark. Laid over the finished frame rather than mixed into
/// every colour, so nothing in `room.rs` has to know what time it is.
const NIGHT: Color = Color::rgb(0.03, 0.05, 0.13);
/// How dark it gets at the middle of the night. Short of black: you still
/// want to see the Bim sleeping.
const NIGHT_DEPTH: f32 = 0.46;

struct Marker {
    pos: Vec2,
    age: f32,
    /// A place the Bim cannot get to. Drawn in the one warm colour the room
    /// keeps for things worth noticing, so a refused order is not silent.
    bad: bool,
}

/// One recipe the world would like made, at one bench: what the room's
/// craft job is offered off. The world hands the room a fresh list every
/// step — see `Game::set_craft_orders` — worked out from the hold, the
/// player's targets and the power; the room decides only who goes and
/// whether the bench is free. `minutes` is how long the recipe takes,
/// carried because the room has no recipe table.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Order {
    pub recipe: u32,
    pub bench: usize,
    pub minutes: f32,
}

/// What the world says about the outside, this step: who may go out — a
/// Bim whose dose is already high may not — which rocks are to be mined
/// and where every rock is, in room units, and how long one takes. `None`
/// when there is nowhere to walk to: not at a site, no suit aboard, no
/// room for what comes back. See `Game::set_eva`.
#[derive(Clone, PartialEq)]
pub struct Eva {
    pub allowed: Vec<bool>,
    /// The marked rocks, by the middle of each, in the order they were
    /// marked.
    pub targets: Vec<Vec2>,
    /// Every rock tile, as a solid the outside grid goes round.
    pub rocks: Vec<Rect>,
    /// Moves whenever `rocks` or `targets` do, so the outside grid is
    /// rebuilt then and not every step.
    pub version: u64,
    /// How long one rock takes to mine, in game minutes.
    pub tile_minutes: f32,
}

/// One construction site the world wants worked, this step: where it is,
/// in room units, and what it wants — a load of `haul.0` (a resource code
/// the room never reads) `haul.1` units strong carried to it, or, with
/// `haul` empty, to be put together over `minutes`. The world hands the
/// room a fresh list every step — `Game::set_build_orders` — worked out
/// from the sites, the hold and whether the ship is at rest; the room
/// decides who goes, whether it is reached from the deck or from outside,
/// and reports what was done. See `Kind::Haul` and `Kind::Build`.
#[derive(Clone, PartialEq, Debug)]
pub struct Build {
    pub site: u32,
    /// Every tile of the footprint, as a rect each.
    pub tiles: Vec<Rect>,
    pub haul: Option<(u32, u32)>,
    pub minutes: f32,
}

/// A part of the ship only one Bim can be using at a time.
///
/// Most of the room is shared happily — two of them can walk past each other,
/// sit at the table, sleep — but some of it is one set of hands' worth. There
/// is one pot, one board, one knife and one fridge door; there is one pan.
/// Two chains running through the same fixtures would each set state the other
/// reads, and the visible half of that is a fridge door flapping while two
/// meals fight over it.
///
/// So an errand that needs one of these does not start while the other Bim is
/// on an errand that needs the same. It is not a queue and nobody waits in
/// line: the errand simply is not begun, and `consider_errand` moves on to
/// whatever else that Bim could be doing. It will come round again.
#[derive(Clone, Copy, PartialEq)]
enum Exclusive {
    /// The galley, the heads, the broom and the shower are **not** here any
    /// more: each is a list of fixtures a chain picks one of, and what a
    /// chain holds is its `Picks` — see `task::Picks` and `Game::taken_for`.
    /// One workstation, by its index in `Room::benches`. One pair of hands
    /// on a bench; a second bench of the same kind is another one of these.
    Bench(usize),
    /// The airlock, for a walk outside — to mine, or to a site beyond the
    /// hull. One body out at a time: the suit is counted by the world and
    /// not taken out of the hold, so this is what keeps two Bims from
    /// wearing one suit.
    Airlock,
    /// One construction site aboard, by its id: one pair of hands carrying
    /// to it or putting it together at a time. A site outside is the
    /// airlock's instead, which is one body anyway.
    Site(u32),
}

/// What a chain needs to itself, or `None` when it treads on nothing.
fn exclusive(kind: Kind) -> Option<Exclusive> {
    match kind {
        Kind::Craft { bench, .. } => Some(Exclusive::Bench(bench)),
        Kind::Eva => Some(Exclusive::Airlock),
        Kind::Haul { outside: true, .. } | Kind::Build { outside: true, .. } => {
            Some(Exclusive::Airlock)
        }
        Kind::Haul { site, .. } | Kind::Build { site, .. } => Some(Exclusive::Site(site)),
        // A berth apiece, so going to bed treads on nobody. And a
        // conversation is the one errand that *wants* the other Bim doing the
        // same thing at the same time, so it can hardly reserve anything.
        // A ship's door has a panel each side and takes a moment: nobody
        // needs to wait for it.
        // Everything at a fixture holds the fixture it picked instead.
        // A dressing holds nothing: two crew dressing one patient's two
        // parts at once is two pairs of hands, which is fine.
        Kind::Rest
        | Kind::Chat
        | Kind::Switch(..)
        | Kind::Meal(_)
        | Kind::Batch
        | Kind::Reheat
        | Kind::Leftovers
        | Kind::Tend { .. }
        | Kind::Heads
        | Kind::Clean
        | Kind::Shower
        | Kind::Bandage { .. } => None,
    }
}

/// What became of a right-click on the floor. The host turns these into words.
pub const ORDER_IGNORED: u32 = 0;
pub const ORDER_MOVING: u32 = 1;
/// Reachable, but the door has to be opened on the way, which the Bim is now
/// going to do before carrying on.
pub const ORDER_VIA_DOOR: u32 = 2;
/// Reachable only through a door that is locked.
pub const ORDER_LOCKED: u32 = 3;
/// No route there at all, with every door in the place wide open.
pub const ORDER_NOWHERE: u32 = 4;

pub struct Game {
    room: Room,
    clock: Clock,
    /// Built once, one grid per state of the bathroom door.
    maps: Maps,
    /// Everything the body is pushed out of, the shut door included. Rebuilt
    /// only when the door changes, which physics can afford to be a frame
    /// behind on — routing, which cannot, goes through `maps` instead.
    blockers: Vec<Rect>,
    door_was_shut: bool,
    /// How many of the ship's powered doors were locked, and how many of
    /// those stood shut, last frame. A change to the first rebuilds the
    /// navigation grids — a locked door is the one thing about a powered
    /// door a route has to be planned round — and to the second, the
    /// blockers.
    doors_locked: usize,
    doors_shut: usize,
    /// The ship's door a click last landed on, for the host to ask after
    /// `hit_at` has said it was one.
    hit_door: usize,
    /// And which bay, hob, cold store, dishwasher, locker, shower and
    /// heads, the same way: the one under the last click, for the menu
    /// that opens after `hit_at` said which kind.
    hit_bay: usize,
    hit_hob: usize,
    hit_fridge: usize,
    hit_dishwasher: usize,
    hit_locker: usize,
    hit_shower: usize,
    hit_bath: usize,
    /// And which of the crew, when the click landed on a body rather than
    /// on the deck: `HIT_BIM`, and the bandage menu opens on this one.
    hit_bim: usize,
    /// And which of the crew when the body under the click was dead —
    /// `HIT_BODY` — and which visitor when it was one of theirs, down —
    /// `HIT_VISITOR`: the Loot window opens on it.
    hit_body: usize,
    hit_visitor: usize,
    /// And which workstation and which shelf: the containers whose grids
    /// the app opens after `hit_at` said `HIT_BENCH` or `HIT_SHELF`.
    hit_bench: usize,
    hit_shelf: usize,
    hit_desk: usize,
    /// Which side of the bathroom bulkhead the Bim was on last frame, and how
    /// long the door has left to stand open after it crossed.
    bim_was_inside: bool,
    door_shut_in: f32,
    rng: Rng,
    /// The crew. Two of them, and the index into this is a Bim's whole
    /// identity — it picks the berth, the seat at the table, the coverall, and
    /// the name the host prints over its head.
    bims: Vec<Bim>,
    /// Which of the crew is ticked first this frame. Rotates every frame so
    /// first refusal on the galley and the heads goes round rather than
    /// always falling to the same Bim.
    first_tick: usize,
    /// Corners of the selection box while the mouse is down; `None` otherwise.
    drag: Option<(Vec2, Vec2)>,
    markers: Vec<Marker>,

    /// The day's timetable. Only sleep is on it so far, and there is one of it
    /// for the whole ship: the crew keep the same hours.
    schedule: Schedule,
    /// What the place has been told to keep in stock. The bay works to it.
    manager: Manager,
    /// The order the player wants the work done in. One list for the ship,
    /// like the timetable: both crew work to it. See `work.rs`.
    priorities: Priorities,
    /// The helm's seat, in room units, while the ship wants somebody at it;
    /// `None` at a berth and in a room with no helm. The world sets it every
    /// step — see [`Game::set_helm`] — and `Job::Helm` is offered off it.
    helm: Option<Vec2>,
    /// What the world wants made right now. See [`Order`]. Empty in the
    /// classic room, which has no benches and no world.
    orders: Vec<Order>,
    /// Whether a walk outside is on, and for whom. See [`Eva`]. Never in
    /// the classic room.
    eva: Option<Eva>,
    /// What a body outside is pushed out of: the hull and the rocks. Kept
    /// with the outside grid — see `refresh_outside`.
    outside_blockers: Vec<Rect>,
    /// Which version of the rocks the outside grid was built from, so it
    /// is rebuilt when they change and not otherwise.
    outside_version: Option<u64>,
    /// How many marked rocks can be got to from the port, as of the last
    /// rebuild. What keeps a walk from being offered to rocks nobody can
    /// reach — a rock three deep with nothing in front of it marked — and
    /// what the host says are out of reach.
    rocks_reachable: usize,
    /// Bodies on this deck that are not this room's: a docked station's
    /// people, who walk about in a room of their own while this one draws
    /// the doors they walk through. Only the doors read them — see
    /// `set_visitors`. Empty in the classic room, and on a ship alone.
    visitors: Vec<Vec2>,
    /// Which of the visitors are down — dead or out cold in their own room,
    /// which only the world knows — index for index with `visitors`, for
    /// `hit_at`: a resident that is down is a body a click can loot. Set
    /// right after `set_visitors` every step and cleared with it.
    visitors_down: Vec<bool>,
    /// Which of the visitors may be spoken to — a mercenary for hire, as
    /// the world says — so that a click on one on its feet is a click on
    /// it (`HIT_VISITOR`) and not on the deck. See [`Game::set_visitors_hailable`].
    visitors_hailable: Vec<bool>,
    /// Whoever the helm job posted at the seat, so the post can be taken
    /// away again when the ship no longer wants it. The player's own "take
    /// the helm" is a post like any other and is not recorded here.
    helmsman: Option<usize>,
    autonomous: bool,
    /// A `SPOT_` code the host has asked to have ringed on the deck, or
    /// `SPOT_NOTHING`. This is how a panel that names a place — "Cold store",
    /// "Pot on the hob" — points at the actual thing rather than leaving the
    /// player to hunt for it. Set and cleared by the pointer; nothing in the
    /// simulation reads it.
    highlight: u32,
    /// Whose eyes the picture is through — see `crate::sight::Fog`. The
    /// crew's own unless the world says otherwise, which it does for a
    /// station's room.
    fog: Fog,
    /// Under `Fog::None`, how long each body stays drawn since the world
    /// last said it was in view — [`SEEN_FOR`] from the sighting, counting
    /// down. Anybody past the end of it is not drawn.
    seen_for: Vec<f32>,
    /// Which of the room's tiles are somebody else's — a station's, on a
    /// joined deck — and whose. Kept here so a relayout can hand it to the
    /// fresh grid; the sight itself is the room's.
    foreign: Option<(Rect, Stance)>,
    /// And whose the rest are, for the same reason.
    stance: Stance,
    /// The fight: the enemies the world named, the bolts in the air, the
    /// hits to hand back. See `crate::combat`.
    combat: Combat,
    /// Whether every body in this room is an enemy to whoever is looking
    /// — a hostile station's people. Its people then fight the targets
    /// as enemies do: they record shots for the world to fly in the
    /// crew's room rather than firing bolts here, and while there is a
    /// target they are **at war** (`at_war`) — recruited, every errand put
    /// down, and walking to wherever `Tactics::stand` says.
    hostile_bodies: bool,
    at_war: bool,
    /// The crew's side of that: whether an enemy is within [`ALARM_RANGE`]
    /// of any of them, or one of them was hit within [`ALARM_HOLD`] — and
    /// then every crew member but the player's is under arms and fights,
    /// walking to wherever `Tactics::stand` says. See `Game::tick_combat`.
    alarm: bool,
    attacked_for: f32,
    /// The hits this room's own bodies took since the world last asked,
    /// already applied. See `Game::take_wounds_taken`.
    wounds_taken: Vec<Hit>,
    /// Every worn piece a hit broke since the world last asked — whose,
    /// and what it was — for the world to say so. See `Game::wound`.
    pieces_broken: Vec<(usize, ArmourKind)>,
    list: DrawList,

    /// Maps the fixed-size room onto whatever canvas the host has. The host
    /// applies this before replaying the draw list, and inverts it to turn
    /// pointer positions back into room coordinates.
    view_scale: f32,
    view_offset: Vec2,
}

impl Game {
    pub fn new(seed: u64, width: f32, height: f32) -> Game {
        let room = Room::new();
        let rng = Rng::new(seed);
        // Each starts somewhere in the open floor, clear of the kitchen units
        // and a body's width apart, so the first frame does not begin with the
        // two of them shoving each other out of one spot.
        //
        // Drawn *between* the Bims rather than all up front: every roll in
        // the room is drawn from one stream, and the order the first few are
        // drawn in is what every seed-pinned probe was pinned against.
        Game::with_room(
            room,
            rng,
            seed,
            CREW,
            |who, rng| {
                vec2(
                    rng.range(ROOM_W * 0.30, ROOM_W * 0.70),
                    rng.range(ROOM_H * 0.42 + who as f32 * 0.12, ROOM_H * 0.52),
                )
            },
            width,
            height,
        )
    }

    /// A game in a room laid out from elsewhere — a ship design, through
    /// `crate::aboard` — with one Bim per `start`, standing there.
    ///
    /// **At most as many as the room has beds.** A Bim's index is its whole
    /// identity — its berth, its seat, its coverall — so a bigger crew is
    /// cut to the beds there are rather than handed a bed that does not
    /// exist.
    ///
    /// And possibly **none**: a station nobody lives on still has a galley
    /// and bunks to draw, and the room is what draws them. Everything that
    /// runs a frame loops over the crew there are; only the wasm exports
    /// that steer `PLAYER` assume one, and those act on the ship's room,
    /// which always has somebody.
    pub fn with_layout(
        layout: room::Layout,
        seed: u64,
        starts: &[Vec2],
        width: f32,
        height: f32,
    ) -> Game {
        let room = Room::from_layout(layout);
        let rng = Rng::new(seed);
        let crew = starts.len().min(room.beds.len());
        Game::with_room(room, rng, seed, crew, |who, _| starts[who], width, height)
    }

    fn with_room(
        room: Room,
        mut rng: Rng,
        seed: u64,
        crew: usize,
        mut start: impl FnMut(usize, &mut Rng) -> Vec2,
        width: f32,
        height: f32,
    ) -> Game {
        let maps = Maps::new(
            room.interior,
            &room.solids(),
            room.bath.door,
            BODY_MARGIN,
            room.nav_tile(),
        );
        let bims = (0..crew)
            .map(|who| {
                let at = start(who, &mut rng);
                Bim::new(who, at, &mut rng)
            })
            .collect();
        let mut game = Game {
            room,
            clock: Clock::new(),
            maps,
            blockers: Vec::new(),
            door_was_shut: false,
            doors_locked: 0,
            doors_shut: 0,
            hit_door: 0,
            hit_bay: 0,
            hit_hob: 0,
            hit_fridge: 0,
            hit_dishwasher: 0,
            hit_locker: 0,
            hit_shower: 0,
            hit_bath: 0,
            hit_bim: 0,
            hit_body: 0,
            hit_visitor: 0,
            hit_bench: 0,
            hit_shelf: 0,
            hit_desk: 0,
            bim_was_inside: false,
            door_shut_in: 0.0,
            rng,
            bims,
            first_tick: 0,
            drag: None,
            markers: Vec::new(),
            schedule: Schedule::new(),
            manager: Manager::new(),
            priorities: Priorities::new(),
            helm: None,
            helmsman: None,
            orders: Vec::new(),
            eva: None,
            outside_blockers: Vec::new(),
            outside_version: None,
            rocks_reachable: 0,
            visitors: Vec::new(),
            visitors_down: Vec::new(),
            visitors_hailable: Vec::new(),
            autonomous: true,
            highlight: room::SPOT_NOTHING,
            fog: Fog::Crew,
            seen_for: Vec::new(),
            foreign: None,
            stance: Stance::Friendly,
            combat: Combat::new(seed),
            hostile_bodies: false,
            at_war: false,
            alarm: false,
            attacked_for: 0.0,
            wounds_taken: Vec::new(),
            pieces_broken: Vec::new(),
            list: DrawList::new(),
            view_scale: 1.0,
            view_offset: Vec2::ZERO,
        };
        game.refresh_blockers();
        game.resize(width, height);
        game
    }

    /// Everybody aboard, taken out of this room for good — for a room that
    /// is being replaced by a bigger one, a docked ship's by the ship's and
    /// the station's together. Every errand is given up first, the way a
    /// blocked one is: whatever was carried goes back in the store, whoever
    /// was in bed or on the pan is stood up. What survives is the Bim — its
    /// needs, its health, its diary, where it stands — and not what it was
    /// in the middle of, because the thing it was walking to is in another
    /// room now. The room is left standing, empty, so what the leaving
    /// banked in it — a sheaf of fibre that was in somebody's hands, a
    /// bandage used — can still be read off it (`take_harvested_fibre`,
    /// `take_bandages_used`) before it is dropped.
    /// How many bunks the room has: how many Bims it will hold, since a
    /// Bim's index is its berth. What a hire asks before `adopt`.
    pub fn bed_count(&self) -> usize {
        self.room.beds.len()
    }

    pub fn take_crew(&mut self) -> Vec<Bim> {
        for bim in &mut self.bims {
            if let Some(task) = bim.task.take() {
                task.abandon(&mut bim.character, &mut self.room);
            }
            for saved in bim.queue.drain(..) {
                if let Some(crop) = saved.lifted {
                    self.room.store(crop);
                }
                if saved.holds_stew() {
                    self.room.stew += 1;
                }
            }
            bim.character.hold_main(crate::character::Held::Nothing);
            bim.character.hold_tool(crate::character::Held::Nothing);
            bim.pending_move = None;
        }
        std::mem::take(&mut self.bims)
    }

    /// Take a crew in — from [`Game::take_crew`] on another room — standing
    /// each one `shift` from where it stood there, because the two rooms'
    /// origins differ by that. They go on the end: index is berth and seat,
    /// and the room's beds are in the order its layout listed them, so a
    /// caller that wants the ship's crew in the ship's bunks adopts them
    /// first. Anybody whose feet would land somewhere a body cannot stand —
    /// off the deck of a room that has just shrunk — is stood at their bunk
    /// instead.
    pub fn adopt(&mut self, crew: Vec<Bim>, shift: Vec2) {
        for mut bim in crew {
            let who = self.bims.len();
            if who >= self.room.beds.len() {
                break;
            }
            // Where the feet land, snapped to the nearest cell a body fits in:
            // a spot beside a bunk sits on the edge of the bunk's inflated
            // footprint, and whether the cell under it reads free depends on
            // how the grid happened to fall. Far from anywhere free — off the
            // deck altogether — is the bunk instead.
            let at = bim.character.pos + shift;
            let free = self.maps.pick(true).nearest_free(at);
            let at = if (free - at).len() <= 2.0 * BODY_MARGIN {
                free
            } else {
                self.room.bed_station(who)
            };
            bim.character.stand_at(at);
            bim.character.set_scripted(false);
            // The route it was on comes too, in the new room's coordinates;
            // so does a post, and a post the new room has no floor under is
            // no post at all.
            bim.character.shift_route(shift);
            bim.character
                .set_post(bim.character.post().map(|p| p + shift).filter(|&p| {
                    (self.maps.pick(true).nearest_free(p) - p).len() <= 2.0 * BODY_MARGIN
                }));
            bim.character.selected = bim.character.selected && who == PLAYER;
            self.bims.push(bim);
        }
        self.refresh_blockers();
    }

    /// The fixed furniture plus the door, if the door is currently something
    /// to walk into.
    fn refresh_blockers(&mut self) {
        self.door_was_shut = self.room.closed_door().is_some();
        self.blockers = self.room.solids().to_vec();
        if let Some(door) = self.room.closed_door() {
            self.blockers.push(door);
        }
        let shut = self.room.shut_doors();
        self.doors_shut = shut.len();
        self.blockers.extend(shut);
    }

    /// The navigation grids again, with the ship's locked doors as solids.
    /// Once per change of lock, never per frame: it is a walk of every cell
    /// against every solid.
    fn refresh_maps(&mut self) {
        let locked = self.room.locked_doors();
        self.doors_locked = locked.len();
        let mut solids = self.room.solids();
        solids.extend(locked);
        self.maps = Maps::new(
            self.room.interior,
            &solids,
            self.room.bath.door,
            BODY_MARGIN,
            self.room.nav_tile(),
        );
    }

    /// Fit the room into the canvas, centred, without distorting it.
    pub fn resize(&mut self, width: f32, height: f32) {
        let w = width.max(1.0);
        let h = height.max(1.0);
        let (room_w, room_h) = (self.room.bounds.width(), self.room.bounds.height());
        self.view_scale = (w / room_w).min(h / room_h);
        self.view_offset = vec2(
            (w - room_w * self.view_scale) * 0.5,
            (h - room_h * self.view_scale) * 0.5,
        );
    }

    pub fn view_scale(&self) -> f32 {
        self.view_scale
    }

    pub fn view_offset(&self) -> Vec2 {
        self.view_offset
    }

    /// One frame of the room: the simulation, then its picture. What the
    /// room's own page calls, once a frame.
    pub fn update(&mut self, dt: f32) {
        self.simulate(dt);
        self.render();
    }

    /// The simulation alone, without drawing it. What the ship game calls
    /// aboard — up to twenty-four times a frame at its top speed, where a
    /// picture of every step but the last would be a picture nobody sees.
    pub fn simulate(&mut self, dt: f32) {
        self.clock.advance(dt);
        self.refresh_outside();
        self.room.update(dt);
        self.judge_the_food();

        // The bays grow on the clock, against the manager's target. It is
        // the game that drives them rather than the room, because the target
        // is the manager's and the room has never heard of the manager.
        let want = self.manager.stock_target();
        let minutes = dt * MINUTES_PER_SECOND;
        let (veg, tofu, fibre) = (self.room.veg, self.room.tofu, self.room.fibre);
        for bay in &mut self.room.bays {
            bay.update(dt, minutes, veg, tofu, fibre, want);
        }

        // Where everybody stands, for the one chain that walks to a
        // crewmate. As of the top of the step, which is a frame behind for
        // whoever is ticked second — a body's width at most, and the
        // bandage walk snaps to a cell beside the patient anyway.
        self.tell_the_room_where_the_crew_are();

        // The timetable is the ship's, not a Bim's, and `due` re-arms itself
        // the  it is asked — so it is asked once a frame here and the
        // answer handed to each of the crew, rather than the first one to look
        // taking the night for itself.
        let bedtime = self.schedule.due(self.clock.minutes());

        // Each in turn, and each entirely on its own account. Nothing below
        // knows or cares which one it is working on except where the room is
        // shared, and that is arbitrated by `galley_taken` and `heads_taken`.
        // Turn and turn about, rather than always in crew order.
        //
        // Whoever is ticked first gets first refusal on everything shared: it
        // looks at the heads, finds them free and starts, and the other then
        // finds them taken. In a fixed order that is the same Bim winning
        // every tie for the whole game. Nothing has been caught going wrong
        // because of it — measured over twenty simulated weeks, neither of
        // them was ever left desperate *and* barred from the pan — but a tie
        // that always breaks the same way is a bias whether or not it has bit
        // yet, and rotating the starting point costs nothing.
        let crew = self.bims.len();
        for step in 0..crew {
            self.tick_bim((self.first_tick + step) % crew, dt, minutes, bedtime);
        }
        // The dressings finished this step, after both have moved: the
        // chain says hands came off a part, and the body it was on is
        // looked at here.
        self.apply_dressings();
        // The blood on the deck, for everybody: a drop is a stain on the
        // tile it lands on, and the deck keeps it until somebody sweeps.
        for bim in &mut self.bims {
            bim.tick_drips(dt, &mut self.rng, &mut self.room.filth);
        }
        // A room with nobody in it — a station nobody lives on — has no tie
        // to break, and no remainder to take.
        if crew > 0 {
            self.first_tick = (self.first_tick + 1) % crew;
        }

        // The locker door shows the broom when nobody has it. Derived rather
        // than set, so an errand given up mid-sweep cannot leave the cupboard
        // claiming to hold a broom that is in somebody's hands.
        self.room.lockers[0].broom_out = self
            .bims
            .iter()
            .any(|b| b.character.main_held() == Held::Broom);

        self.tick_door_closer(dt);
        // The ship's doors open for whoever walks up to them and shut
        // behind; a lock or a shut leaf is a change to what a route or a
        // body has to go round.
        let bodies: Vec<Vec2> = self
            .bims
            .iter()
            .filter(|b| !b.character.is_dead())
            .map(|b| b.character.pos)
            .chain(self.visitors.iter().copied())
            .collect();
        self.room.update_doors(dt, &bodies);
        if self.room.locked_doors().len() != self.doors_locked {
            self.refresh_maps();
        }
        if self.room.closed_door().is_some() != self.door_was_shut
            || self.room.shut_doors().len() != self.doors_shut
        {
            self.refresh_blockers();
        }

        for m in &mut self.markers {
            m.age += dt;
        }
        self.markers.retain(|m| m.age < MARKER_LIFE);

        // The fight, after everybody has moved: who can see whom is asked
        // of where they stand now.
        self.tick_combat(dt);
        for s in &mut self.seen_for {
            *s = (*s - dt).max(0.0);
        }
    }

    /// Everybody in combat mode takes aim, and whoever is loaded fires;
    /// then every bolt in the air flies on, and whatever landed on one of
    /// our own is applied. Combat mode is being under orders with a
    /// weapon to draw — a recruited Bim draws it and shoots at any enemy
    /// it can see, and does nothing else about the enemy: it stands where
    /// it was put, since a recruited Bim goes nowhere unasked.
    ///
    /// A trigger pull is a **burst** of the weapon's `burst` shots,
    /// `burst_gap` apart, each rolled on its own and aimed afresh — the
    /// rifle's eight in two seconds — and `reload` then waits the trigger
    /// rate out, recharge included. A body **in a melee** — a blade
    /// within reach, or a blade in its own hand and anybody within reach
    /// (`Combat::melee_with`) — is locked: it does not fire, and swings
    /// every [`MELEE_PERIOD`] instead, a cut with a blade and a fist
    /// without. The swing is a [`Blow`] on the Bim that **lands when the
    /// animation ends**, [`SWING_TIME`] on, and only if the target is
    /// still within reach then — a swing is not a hit until it has been
    /// swung. A body aiming from a **peek** beside a wall leans out to
    /// it (`Bim::peek`), and that is where a shot at it is aimed and
    /// lands, half of them dodged.
    ///
    /// A room whose bodies are hostile is the other side of that. Its
    /// people are the enemy, and while the world names a target for them
    /// they are **at war**: every one of them under orders, its errand
    /// put down, and marching to wherever `Tactics::stand` says it should
    /// shoot from — every [`PLAN_EVERY`] seconds on its own clock. It
    /// fires the moment it can see a target from where it is, mid-plan or
    /// not, and — like the crew — not while walking. What it fires is a
    /// `Shot`, not a bolt: the world flies it in the crew's room, where
    /// the body it is aimed at actually is; a blow is a melee `Shot` the
    /// world delivers straight to the body (`Game::enemy_strike`).
    fn tick_combat(&mut self, dt: f32) {
        // The doors as they stand this step go into the sight before
        // anybody aims through it. The trace puts them there too, but
        // only at the picture and only through the crew's eyes: a
        // station's room under a joined deck is drawn through nobody's,
        // and its people would otherwise shoot through every shut door
        // as if it stood open.
        let shut = self.shut_now();
        self.room.sight.set_shut(&shut);
        let war = self.hostile_bodies && self.combat.targets().iter().any(|t| t.is_some());
        if war != self.at_war {
            self.at_war = war;
            self.muster(war);
        }
        // The crew's alarm: an enemy within range of any of them, or one of
        // them hit lately, and everybody but the player's own is under
        // arms — a weapon out of the pack if the hand is empty — and fights
        // the way an enemy's people do. Nobody near and nobody hit for a
        // while, and they stand down to their errands.
        self.attacked_for = (self.attacked_for - dt).max(0.0);
        let alarm = !self.hostile_bodies
            && (self.attacked_for > 0.0 || self.enemy_within(ALARM_RANGE * TILE));
        if alarm != self.alarm {
            self.alarm = alarm;
            self.muster_crew(alarm);
        }
        for who in 0..self.bims.len() {
            let bim = &mut self.bims[who];
            bim.reload = (bim.reload - dt).max(0.0);
            bim.hit_flash = (bim.hit_flash - dt).max(0.0);
            bim.melee_timer = (bim.melee_timer - dt).max(0.0);
            if let Some(blow) = bim.blow.as_mut() {
                blow.left -= dt;
            }
            // Not while winding a bandage: both hands are on it, so the
            // weapon is holstered for the dressing and drawn again after.
            // The walk to the patient is not the dressing — it holds its
            // fire the way any walk does, and shoots again on arriving.
            let dressing = bim.task.as_ref().is_some_and(|t| t.is_dressing());
            let armed = bim.is_alive()
                && bim.character.is_recruited()
                && bim.gear.weapon.is_some()
                && !bim.character.is_outside()
                && !bim.character.is_seated()
                && !bim.character.is_napping()
                && !bim.character.is_unconscious()
                && !dressing;
            let weapon = bim.gear.weapon.filter(|_| armed);
            bim.character.set_armed(weapon);
            let Some(weapon) = weapon else {
                bim.burst_left = 0;
                bim.locked = None;
                bim.blow = None;
                bim.peek = None;
                bim.character.set_lean(None);
                continue;
            };
            let stats = weapon.stats();
            // At war it picks its own stand. A crew member under the alarm
            // that is not the player's to steer keeps to the player's side
            // until it sees an enemy for itself, and then picks its own —
            // unless the player ordered it somewhere, when it holds that.
            if war {
                self.plan_stand(who, dt, &stats);
            } else if alarm && who != PLAYER && self.bims[who].character.post().is_none() {
                let from = self.bims[who].character.pos;
                let player_up =
                    self.bims[PLAYER].is_alive() && !self.bims[PLAYER].character.is_outside();
                if !player_up || self.combat.sees_any(&self.room.sight, from) {
                    self.plan_stand(who, dt, &stats);
                } else {
                    self.gather(who, dt);
                }
            }
            let bim = &mut self.bims[who];
            let from = bim.character.pos;

            // A swing that has been swung lands now — on the target it was
            // aimed at if that one is still within reach, and on nobody if
            // it stepped back — locked or not, since the blow was already
            // on its way when the lock was last read.
            if let Some(blow) = bim.blow.take_if(|b| b.left <= 0.0) {
                if self.combat.within_reach(from, blow.target) {
                    self.combat.brawl(
                        from,
                        blow.target,
                        weapon,
                        blow.damage,
                        blow.cut,
                        self.hostile_bodies,
                    );
                }
            }
            let bim = &mut self.bims[who];

            // A melee first: locked, it neither aims nor fires, and the
            // burst it was in the middle of is over.
            let locked = self.combat.melee_with(&self.room.sight, from, &stats);
            bim.locked = locked;
            if let Some(enemy) = locked {
                bim.burst_left = 0;
                bim.peek = None;
                bim.character.set_lean(None);
                if let Some(at) = self.combat.targets()[enemy].map(|t| t.at) {
                    bim.character.face((at - from).angle());
                }
                if bim.melee_timer <= 0.0 {
                    bim.melee_timer = MELEE_PERIOD;
                    let (damage, cut, swing) = if stats.melee {
                        (stats.damage, true, Action::Swing)
                    } else {
                        (FIST_DAMAGE, false, Action::Punch)
                    };
                    bim.character.antic(swing, SWING_TIME);
                    bim.blow = Some(Blow {
                        target: enemy,
                        left: SWING_TIME,
                        damage,
                        cut,
                    });
                }
                continue;
            }
            // A blade with nobody in reach has nothing to aim.
            if stats.melee {
                bim.peek = None;
                bim.character.set_lean(None);
                continue;
            }
            let Some((_, eye, at)) = self.combat.aim(&self.room.sight, from, &stats) else {
                bim.burst_left = 0;
                bim.peek = None;
                bim.character.set_lean(None);
                continue;
            };
            // Squared up to it, and a shot the moment the weapon is ready.
            // Not while walking, for the crew: a Bim on its way somewhere
            // is facing the way it is going, and turns to shoot when it
            // gets there. An enemy shoots when it can — on the move too,
            // from its own eyes and without turning off its route; only a
            // peek wants standing against the wall to lean from.
            let walking = bim.character.is_walking();
            if walking && (!self.hostile_bodies || eye != from) {
                bim.burst_left = 0;
                bim.peek = None;
                bim.character.set_lean(None);
                continue;
            }
            // Aiming from a peek, it leans out to the peek and looks down
            // the corridor from there; from its own eyes, it faces the
            // target square — unless it is walking, when the walk has the
            // facing.
            let peek = (eye != from).then_some(eye);
            bim.peek = peek;
            bim.character.set_lean(peek);
            if !walking {
                bim.character.face((at - eye).angle());
            }
            let shoot = if bim.reload <= 0.0 {
                // The trigger: the first of the burst now, the rest to
                // follow, and the trigger rate to wait out after.
                bim.reload = 1.0 / stats.fire_rate.max(1e-3);
                bim.burst_left = stats.burst.saturating_sub(1);
                bim.burst_timer = stats.burst_gap;
                true
            } else if bim.burst_left > 0 {
                bim.burst_timer -= dt;
                if bim.burst_timer <= 0.0 {
                    bim.burst_left -= 1;
                    bim.burst_timer = stats.burst_gap;
                    true
                } else {
                    false
                }
            } else {
                false
            };
            if shoot {
                if self.hostile_bodies {
                    self.combat.shoot(eye, at, weapon);
                } else {
                    self.combat.fire(eye, at, weapon, false);
                }
            }
        }
        if !self.combat.quiet() || !self.combat.targets().is_empty() {
            // Our own bodies, for a hostile bolt to find: whoever is alive
            // and on the deck, out cold or not — where it leans out to
            // while it peeks, and whether it does.
            let bodies: Vec<Option<(Vec2, bool)>> = self
                .bims
                .iter()
                .map(|b| {
                    (b.is_alive() && !b.character.is_outside())
                        .then_some((b.peek.unwrap_or(b.character.pos), b.peek.is_some()))
                })
                .collect();
            self.combat.step(dt, &self.room.sight, &bodies);
        }
        // What landed on us goes on the body now, and a copy is kept for
        // the world to say so.
        let taken = std::mem::take(&mut self.combat.wounds_taken);
        for hit in &taken {
            self.strike(hit.who, hit.part, hit.damage, hit.cut);
        }
        if !taken.is_empty() {
            self.attacked_for = ALARM_HOLD;
        }
        self.wounds_taken.extend(taken);
    }

    /// To arms, or stand down: every living body under orders with its
    /// errand put down — the queue is kept, so peace picks it all up
    /// again — or let go. What entering and leaving war is.
    fn muster(&mut self, war: bool) {
        for who in 0..self.bims.len() {
            if !self.bims[who].is_alive() {
                continue;
            }
            self.bims[who].character.set_recruited(war);
            if war {
                self.interrupt(who);
                // A post is where peace put it; the fight puts it elsewhere.
                self.bims[who].character.set_post(None);
                self.bims[who].plan_wait = 0.0;
            }
        }
    }

    /// The crew's alarm, on or off: every living crew member but the
    /// player's own under arms with its errand put down — a weapon taken
    /// out of the pack into an empty hand first, since a recruited body
    /// with nothing in its hand is a body standing still — or let go, the
    /// queue kept, so peace picks it all up again. The player's own is
    /// left as the player has it: recruiting it is the player's.
    fn muster_crew(&mut self, alarm: bool) {
        for who in 0..self.bims.len() {
            if who == PLAYER || !self.bims[who].is_alive() {
                continue;
            }
            if alarm {
                if self.bims[who].gear.weapon.is_none()
                    && let Some(cell) = self.bims[who]
                        .gear
                        .pack
                        .iter()
                        .position(|c| matches!(c, Some(Item::Weapon(_))))
                {
                    self.equip(who, cell);
                }
                self.interrupt(who);
                self.bims[who].plan_wait = 0.0;
            }
            // Any post — peace's, or the spot the player ordered it to hold
            // during the alarm — is over either way.
            self.bims[who].character.set_post(None);
            self.bims[who].character.set_recruited(alarm);
        }
    }

    /// Whether any target the world named is within `range` room units of
    /// any living crew member on the deck.
    fn enemy_within(&self, range: f32) -> bool {
        self.combat.targets().iter().flatten().any(|t| {
            self.bims.iter().any(|b| {
                b.is_alive() && !b.character.is_outside() && (b.character.pos - t.at).len() <= range
            })
        })
    }

    /// Whether the crew's alarm is up: an enemy within [`ALARM_RANGE`] or
    /// a crew member hit within [`ALARM_HOLD`]. The app reads it for the
    /// header; every crew member but the player's is under arms while it is.
    pub fn is_alarmed(&self) -> bool {
        self.alarm
    }

    /// One enemy's choice of where to stand, when its clock comes round:
    /// `Tactics::stand`, and a march there if it is more than a tile from
    /// where it is already going. Not a post — it is recruited, so it
    /// stays wherever it arrives without one — and no marker on the deck,
    /// since a ping where an enemy is about to stand would be a picture
    /// through the fog.
    fn plan_stand(&mut self, who: usize, dt: f32, stats: &WeaponStats) {
        let bim = &mut self.bims[who];
        bim.plan_wait -= dt;
        if bim.plan_wait > 0.0 {
            return;
        }
        bim.plan_wait = PLAN_EVERY;
        let from = bim.character.pos;
        let nav = self.maps.for_body(false, self.room.bath.is_open());
        let targets = self.combat.targets().to_vec();
        // Every doorway, open or shut: none is a stand, since a door opens
        // for whoever comes to stand in it. See `Tactics::stand`.
        let doorways: Vec<Rect> = self
            .room
            .doors
            .iter()
            .map(|d| d.rect)
            .chain(self.room.airlocks.iter().copied())
            .collect();
        let Some(to) = Tactics::stand(&self.room.sight, nav, from, &targets, stats, &doorways)
        else {
            return;
        };
        let going = bim.character.destination().unwrap_or(from);
        if (to - going).len() <= TILE {
            return;
        }
        let route = nav.path(from, to);
        if !route.is_empty() {
            self.bims[who].character.follow_path(route);
        }
    }

    /// A crew member under the alarm keeping to the player's side: its
    /// slot in the ring round the player's Bim — [`GATHER_SLOTS`], by its
    /// rank among the crew — snapped to the nearest cell a body fits in,
    /// walked to on its own `plan_wait` clock when it is more than
    /// [`GATHER_SLACK`] from it and not already on its way. Not a post:
    /// the player's Bim moves, and the ring moves with it. It shoots what
    /// it sees from there like any recruited body.
    fn gather(&mut self, who: usize, dt: f32) {
        let bim = &mut self.bims[who];
        bim.plan_wait -= dt;
        if bim.plan_wait > 0.0 {
            return;
        }
        bim.plan_wait = PLAN_EVERY;
        let from = bim.character.pos;
        let rank = (1..who).filter(|&i| self.bims[i].is_alive()).count();
        let nav = self.maps.for_body(false, self.room.bath.is_open());
        let player = self.bims[PLAYER].character.pos;
        // Its own slot, or the next one round that there is a way to — a
        // slot inside a wall the player stands against is nobody's.
        let Some(to) = (0..GATHER_SLOTS.len())
            .map(|i| GATHER_SLOTS[(rank + i) % GATHER_SLOTS.len()])
            .map(|(dx, dy)| nav.nearest_free(player + vec2(dx * TILE, dy * TILE)))
            .find(|&to| nav.can_reach(from, to))
        else {
            return;
        };
        let bim = &self.bims[who];
        let going = bim.character.destination().unwrap_or(from);
        if (to - going).len() <= GATHER_SLACK * TILE || (to - from).len() <= GATHER_SLACK * TILE {
            return;
        }
        let route = nav.path(from, to);
        if !route.is_empty() {
            self.bims[who].character.follow_path(route);
        }
    }

    /// One crew member's half of the frame.
    fn tick_bim(&mut self, who: usize, dt: f32, minutes: f32, bedtime: bool) {
        if self.bims[who].character.is_dead() {
            // Nothing else moves for this one. The room carries on — a cycle
            // finishes, a hob goes out — and so does the other Bim.
            self.move_body(who, dt);
            return;
        }

        // Dropping off where it stands counts as sleeping for what it does to
        // the Bim — rest comes back at the proper rate — but not for what it
        // clears, because a quarter of an hour is worth a few per cent and the
        // status only lifts above four fifths rested.
        let nodded_off = self.bims[who].nap_left > 0.0;
        if nodded_off {
            self.bims[who].nap_left -= minutes;
            if self.bims[who].nap_left <= 0.0 {
                self.bims[who].nap_left = 0.0;
                self.bims[who].character.nod_off(false);
            }
        } else if self.bims[who].health.drowsiness().nods_off()
            && self.bims[who]
                .task
                .as_ref()
                .is_none_or(|t| t.restoring().is_none())
            && self.rng.chance(minutes / NAP_EVERY)
        {
            self.bims[who].nap_left = NAP_MINUTES;
            self.bims[who].character.nod_off(true);
            self.remember(who, What::NoddedOff, 0);
        }

        let restoring = if nodded_off {
            Some(Need::Rest)
        } else {
            self.bims[who].task.as_ref().and_then(|t| t.restoring())
        };
        let tiring = self.bims[who].health.stage().tiring();
        let purging = if self.bims[who].is_poisoned() {
            POISONED_PURGE
        } else {
            1.0
        };
        self.bims[who].needs.update(dt, restoring, tiring, purging);
        // The illness runs its course on its own clock, asleep or awake.
        self.bims[who].poisoned_for = (self.bims[who].poisoned_for - minutes).max(0.0);
        // A mouthful out of a bad pot. Read off what the Bim is doing this
        // instant rather than off the plate: the pot a plate was filled
        // from is the pot on the hob its chain picked.
        let bad_pot = self.bims[who]
            .task
            .as_ref()
            .is_some_and(|t| self.room.hob(t.picks().hob.unwrap_or(0)).food_bad);
        if restoring == Some(Need::Food) && bad_pot {
            self.poison(who);
        }

        // Cleanliness follows the deck rather than the clock: the tiles the
        // Bim is standing among, and what it has on itself.
        let grinding = self.room.filth.grinding(
            self.bims[who].character.pos,
            self.bims[who].character.filth(),
        );
        self.bims[who].needs.scrub(minutes, grinding);
        self.mind_the_mess(who, minutes, restoring == Some(Need::Rest));

        // Going without is measured in time, not in how empty the bar is.
        let (food, rest) = (
            self.bims[who].needs.level(Need::Food),
            self.bims[who].needs.level(Need::Rest),
        );
        self.bims[who]
            .health
            .update(minutes, food, rest, restoring == Some(Need::Rest));
        // Going a stage further into hunger or sleeplessness is worth
        // remembering — once, as it happens, rather than for every frame it
        // goes on lasting. Only downwards: coming back out of it is a relief
        // rather than an event, and the bars say so anyway.
        let hunger = self.bims[who].health.stage().stage();
        if hunger > self.bims[who].worst_hunger {
            self.bims[who].worst_hunger = hunger;
            self.remember(who, What::Hungrier, hunger);
        }
        let weary = self.bims[who].health.drowsiness().stage();
        if weary > self.bims[who].worst_weariness {
            self.bims[who].worst_weariness = weary;
            self.remember(who, What::Wearier, weary);
        }
        // A proper night, or a proper meal, sets the mark back so the next
        // slide into it is noticed afresh.
        if hunger == 0 {
            self.bims[who].worst_hunger = 0;
        }
        if weary == 0 {
            self.bims[who].worst_weariness = 0;
        }
        // Picking its way around a mess is slower than walking through it, and
        // so is squeezing past the other Bim. All three multiply: a starving
        // Bim edging round its shipmate through a fouled galley is slow for
        // three separate reasons and should read as slow.
        let pace = self.bims[who].health.stage().pace()
            * self.bims[who].health.pace()
            * self.bims[who].ordeal.discomfort().pace()
            * self.bims[who].solitude.stage().works_at()
            * self.crowding(who);
        self.bims[who].character.set_pace(pace);
        if self.bims[who].health.is_dead() {
            self.die(who);
            return;
        }

        // Out cold for want of blood, or come round. Going out is like
        // dropping off standing up — the errand is put down, the frame
        // stops here for this Bim — except that it is the blood and not
        // the clock that ends it, and the body lies rather than stands.
        // The errand goes first, so what it puts down is put down standing.
        let out = self.bims[who].health.unconscious();
        if out != self.bims[who].character.is_unconscious() {
            if out {
                self.interrupt(who);
            }
            self.bims[who].character.knock_out(out);
        }
        if out {
            self.move_body(who, dt);
            return;
        }

        // Fully rested is fully rested, whatever the timetable or the clock
        // say: the Bim gets up rather than lying there.
        if self.bims[who].needs.level(Need::Rest) >= WAKE_AT {
            if let Some(task) = &mut self.bims[who].task {
                task.wake();
            }
        }

        // The timetable only ever adds a sleep, and only at the start of a
        // block. It goes on the *back* of the queue: a standing instruction
        // waits its turn, unlike an interruption, which jumps the front.
        //
        // The block is still tracked while the Bim is under orders — `due` is
        // asked once a frame at the top of `update` and has to see the hour go
        // by to re-arm — but nothing is added, because a recruited Bim takes
        // no instruction but the player's.
        if bedtime
            && !self.bims[who].character.is_recruited()
            && self.bims[who].needs.level(Need::Rest) <= IGNORE_ABOVE
        {
            self.bims[who]
                .queue
                .push(Saved::fresh(who, Kind::Rest, SLEEP_MINUTES));
        }

        // Out cold on its feet: the errand waits, and so does everything else.
        if self.bims[who].character.is_napping() {
            self.move_body(who, dt);
            return;
        }

        // Alone too long: the clock, and whatever it has just cost.
        self.bear_the_solitude(who, minutes);
        // Sitting on the deck having given up for a bit. The same shape as a
        // nod-off: the frame stops here for this Bim, and whatever it was in
        // the middle of is still there when it gets up.
        if self.bims[who].sad_left > 0.0 {
            self.bims[who].sad_left -= minutes;
            if self.bims[who].sad_left <= 0.0 {
                self.bims[who].sad_left = 0.0;
                self.bims[who].character.stand();
            }
            self.move_body(who, dt);
            return;
        }

        // A wound the medical row says is urgent — set to the top — is
        // dressed *now*, whatever the Bim was in the middle of: the errand
        // is put down onto the queue the way a need's is, and picked up
        // again when the hands come off. The only row on the list that
        // interrupts; at any other number it waits its turn like the rest,
        // and at never nobody doctors of their own accord. A Bim already
        // walking over to dress somebody is left to it — asking again
        // would restart the walk every frame.
        if self.autonomous
            && self.priorities.of(Job::Medical) == work::HIGHEST
            && !self.bims[who]
                .task
                .as_ref()
                .is_some_and(|t| matches!(t.kind(), Kind::Bandage { .. }))
            && let Some((patient, part)) = self.medical_on_offer(who)
        {
            self.bandage(who, patient, part);
        }

        let fumble = self.bims[who].health.drowsiness().fumble();
        // A Bim with nobody to talk to drags: a tenth longer over everything
        // it does. `Task` applies it only to the working steps — sleeping and
        // eating are not work and are not slowed — and the walking half of the
        // same tenth is in the pace above.
        let effort = self.bims[who].solitude.stage().works_at();
        let taken = self.taken_for(who);
        {
            let (bims, room, maps, rng) =
                (&mut self.bims, &mut self.room, &self.maps, &mut self.rng);
            let bim = &mut bims[who];
            if let Some(task) = &mut bim.task {
                task.update(
                    dt,
                    &mut bim.character,
                    room,
                    maps,
                    rng,
                    fumble,
                    effort,
                    &taken,
                );
                if task.is_done() {
                    // Kept for the conversation and nothing else — the diary
                    // does not hold the day's work any more. A chat is not an
                    // errand worth telling anybody about.
                    if task.kind() != Kind::Chat {
                        let did = job_code(task.kind(), task.rest_minutes());
                        bim.lately.push(did);
                        if bim.lately.len() > TALKS_ABOUT {
                            bim.lately.remove(0);
                        }
                    }
                    bim.task = None;
                }
            }
        }
        self.take_pending_move(who);
        self.pump_queue(who);
        self.consider_errand(who);
        self.return_to_post(who);
        self.flee_filth(who, dt);

        {
            // Dirt travels on boots. Where the body was, and where this frame
            // of walking has put it — taken either side of the one call that
            // moves it, so that a chain setting a Bim down somewhere (into a
            // bunk, onto a chair) is not read as a stride across the deck.
            let was = self.bims[who].character.pos;
            self.move_body(who, dt);
            let now = self.bims[who].character.pos;
            self.room.filth.track(was, now, &mut self.rng);
        }
        self.unstick(who, dt);
        self.bims[who].tick_trail(dt);
    }

    /// `Room::crew`, afresh: every body alive and on the deck, by index.
    /// Once at the top of every step, and again as a bandage is ordered,
    /// since the order may come before the first step and the walk it
    /// starts picks its spot from this.
    fn tell_the_room_where_the_crew_are(&mut self) {
        self.room.crew.clear();
        self.room.crew.extend(
            self.bims
                .iter()
                .map(|b| (b.is_alive() && !b.character.is_outside()).then_some(b.character.pos)),
        );
    }

    /// One frame of the body: on the deck, kept clear of the walls and
    /// pushed out of the furniture; outside, on the outside grid's span,
    /// pushed out of the hull and the rocks. The one call that moves a Bim.
    fn move_body(&mut self, who: usize, dt: f32) {
        let outside = self.bims[who].character.is_outside();
        let interior = match (outside, self.maps.outside()) {
            (true, Some(nav)) => nav.interior(),
            _ => self.room.interior,
        };
        let blockers = if outside {
            &self.outside_blockers
        } else {
            &self.blockers
        };
        let (bims, rng) = (&mut self.bims, &mut self.rng);
        bims[who].character.update(dt, interior, blockers, rng);
    }

    /// The outside grid, kept about whoever is out there.
    ///
    /// Built [`nav::OUTSIDE_RADIUS`] tiles every way from the body outside
    /// — or from the spot outside the port while nobody is — over the hull
    /// and the rocks the world handed over, and built again when the rocks
    /// change or the body has walked [`nav::OUTSIDE_RECENTRE`] tiles from
    /// its middle. Dropped when there is no outside to walk — no rocks and
    /// no construction site, which may be beyond the hull — and nobody out
    /// in it; a body still out there when the site has gone keeps a grid
    /// of the hull alone, to walk back on.
    fn refresh_outside(&mut self) {
        let out = self
            .bims
            .iter()
            .find(|b| b.character.is_outside())
            .map(|b| b.character.pos);
        if self.eva.is_none() && self.room.builds.is_empty() && out.is_none() {
            self.maps.set_outside(None);
            self.outside_version = None;
            self.rocks_reachable = 0;
            return;
        }
        let Some(centre) = out.or(self.room.outside) else {
            return;
        };
        let tile = crate::filth::TILE;
        let stale = match self.maps.outside() {
            None => true,
            Some(nav) => {
                self.outside_version != Some(self.room.rocks_version)
                    || (nav.middle() - centre).len() > nav::OUTSIDE_RECENTRE * tile
            }
        };
        if !stale {
            return;
        }
        let mut solids = self.room.hull.clone();
        solids.extend(self.room.rocks.iter().copied());
        self.maps
            .set_outside(Some(Nav::outside(centre, &solids, BODY_MARGIN, tile)));
        self.outside_blockers = solids;
        self.outside_version = Some(self.room.rocks_version);
        // Whether the walk is worth offering: asked from the port, once per
        // change, since a route to every marked rock is a search of the
        // whole grid when there is none.
        self.rocks_reachable = self
            .room
            .outside
            .map(|from| task::reachable_rocks(&self.room, &self.maps, from).len())
            .unwrap_or(0);
    }

    /// The watchdog on a walk: a body the push-out has stopped dead is given
    /// a fresh route to the same place.
    ///
    /// A route is planned once and the body walks it, turning as it goes —
    /// and the turn is an arc, which can carry it a body's width off the
    /// line it was given. Off the line and against the inflated face of a
    /// table, with the next waypoint straight through it, every step is
    /// undone by the push-out exactly, and it stands there for ever: still
    /// marching, never arriving, the chain waiting on an `arrived()` that
    /// will not come. `line_clear` sampling either side of the line made it
    /// rarer and the seeds that show it moved with every change to the RNG;
    /// this is what catches the ones that are left.
    ///
    /// Marching and not having moved [`STUCK_STEP`] in [`STUCK_AFTER`] is
    /// the signature — nothing else aboard looks like it, since the crew
    /// pass through each other and a shove is never exactly opposite the
    /// walk for a whole second by chance. The new route goes to where the
    /// old one ended, planned from here. Where there is none — the heads'
    /// door shut behind the other one across a walk into it, a door locked
    /// since the route was planned — the chain is put down (`interrupt`)
    /// rather than the Bim told it has arrived: picked up again it plans
    /// its walk afresh, and `Task::enter` refuses one with nowhere to go.
    fn unstick(&mut self, who: usize, dt: f32) {
        let bim = &mut self.bims[who];
        let pos = bim.character.pos;
        let marching = bim.character.is_walking() && bim.character.destination().is_some();
        if marching && (pos - bim.last_pos).len() < STUCK_STEP * dt {
            bim.stuck += dt;
        } else {
            bim.stuck = 0.0;
        }
        bim.last_pos = pos;
        if bim.stuck < STUCK_AFTER {
            return;
        }
        bim.stuck = 0.0;
        let Some(to) = bim.character.destination() else {
            return;
        };
        let outside = self.bims[who].character.is_outside();
        let route = self
            .maps
            .for_body(outside, self.room.bath.is_open())
            .path(pos, to);
        if route.is_empty() {
            // Off the route as well as off the chain: a body left marching
            // on a walk it cannot make never `arrived()`, and a Bim that
            // never arrives starts nothing else either.
            self.bims[who].character.follow_path(Vec::new());
            self.interrupt(who);
        } else {
            self.bims[who].character.follow_path(route);
        }
    }

    /// How much `who` is slowed by having somebody in the same space.
    ///
    /// The crew do not collide. Two of them walking into each other pass
    /// straight through and both slow down, which is the one resolution that
    /// cannot leave anybody stuck — and being stuck is the real hazard here,
    /// because a route is planned once and never replanned, so a Bim whose
    /// line goes through another has no second plan to fall back on.
    ///
    /// Only bodies on their feet count. One sitting at the table or asleep in
    /// its bunk is tucked into the furniture rather than in the gangway, and
    /// slowing somebody walking past the foot of a bed would be wrong.
    fn crowding(&self, who: usize) -> f32 {
        let at = self.bims[who].character.pos;
        let close = self.bims.iter().enumerate().any(|(i, other)| {
            i != who
                && other.is_alive()
                && !other.character.is_seated()
                && !other.character.is_outside()
                && (other.character.pos - at).len() < CREW_CLEARANCE
        });
        if close { CROWDED_PACE } else { 1.0 }
    }

    // --- what a Bim remembers ---------------------------------------------

    /// Write one line of the diary.
    fn remember(&mut self, who: usize, what: What, detail: u32) {
        let (day, at) = (self.clock.day(), self.clock.minutes());
        self.bims[who].memory.note(day, at, what, detail);
    }

    /// Something happened to `who` that the rest of the crew could see.
    ///
    /// Only what is actually in front of them, and only if they are awake to
    /// see it: a Bim asleep in its bunk on the far side of the compartment did
    /// not witness anything, and a diary that says it did is a diary nobody
    /// can trust.
    fn witnessed(&mut self, who: usize, what: What) {
        let at = self.bims[who].character.pos;
        for other in 0..self.bims.len() {
            if other == who {
                continue;
            }
            let them = &self.bims[other];
            let close = (them.character.pos - at).len() <= WITHIN_SIGHT;
            if !close || !them.is_alive() || them.character.is_napping() {
                continue;
            }
            // In bed with the covers up is not watching the room either.
            if them.task.as_ref().is_some_and(|t| t.rest_left() > 0.0) {
                continue;
            }
            self.remember(other, what, who as u32);
        }
    }

    // --- mess -------------------------------------------------------------

    /// Everything a mess does to the Bim, and everything the Bim does about it
    /// short of an errand: hopping about, accidents, and being sick.
    ///
    /// None of it interrupts a chain. These are things that happen *to* the
    /// Bim — a Bim that wets itself on the way to the galley carries on to the
    /// galley — and the pose is only taken up when nothing else owns it.
    fn mind_the_mess(&mut self, who: usize, minutes: f32, asleep: bool) {
        // Asleep, the needs are frozen — see `needs.rs` — so the accidents that
        // hang off them are frozen too. Without that a Bim that turns in at the
        // middle urge rolls the one-in-ten every hour of a six-hour night and
        // wets the bed about half the time, for a need that is not even moving.
        let urge = if asleep {
            Urge::None
        } else {
            self.bims[who].needs.urge()
        };
        let (restroom, cleanliness) = (
            self.bims[who].needs.level(Need::Restroom),
            self.bims[who].needs.level(Need::Cleanliness),
        );
        let (bims, rng) = (&mut self.bims, &mut self.rng);
        let purging = bims[who].is_poisoned();
        let mishap = bims[who].ordeal.update(
            minutes,
            restroom,
            cleanliness,
            urge == Urge::Extreme,
            urge == Urge::Medium,
            purging,
            rng,
        );

        let at = self.bims[who].character.pos;
        // Relief is relief, however it comes: the need goes back up, and
        // everything else gets worse. Without that the Bim would foul the
        // deck again on the very next frame, for ever.
        if mishap.fouled {
            let (on_bim, relief) = self.room.filth.foul(at);
            self.bims[who].character.soil(on_bim);
            self.bims[who].needs.refill(Need::Restroom, relief);
            self.remember(who, What::Accident, 1);
            self.witnessed(who, What::SawAccident);
        } else if mishap.wet {
            let (on_bim, relief) = self.room.filth.wet(at);
            self.bims[who].character.soil(on_bim);
            self.bims[who].needs.refill(Need::Restroom, relief);
            self.remember(who, What::Accident, 0);
            self.witnessed(who, What::SawAccident);
        }
        if mishap.sick {
            self.room.filth.sick_on(at);
            self.bims[who]
                .needs
                .spend(Need::Food, filth::SICK_COSTS_FOOD);
            self.bims[who].character.antic(Action::Retch, RETCH_TIME);
            self.remember(who, What::WasSick, 0);
            self.witnessed(who, What::SawSickness);
        }

        // Hopping about on the spot: the one warning the player gets that an
        // accident is coming, and the only outward sign of a need that has no
        // bar-width left to lose.
        if !self.bims[who].character.in_antic() && self.rng.chance(minutes * urge.fidget_chance()) {
            self.bims[who].character.antic(Action::Fidget, FIDGET_TIME);
        }
    }

    /// Get away from it.
    ///
    /// Past the first stage of discomfort the Bim will not stand in a mess if
    /// there is anywhere better within a few tiles. It is a walk rather than an
    /// errand, so anything it is actually doing — and anything waiting on the
    /// queue — outranks it: it only moves when it would otherwise be idling.
    fn flee_filth(&mut self, who: usize, dt: f32) {
        self.bims[who].flee_wait = (self.bims[who].flee_wait - dt).max(0.0);
        if !self.bims[who].ordeal.discomfort().flees()
            || self.bims[who].character.is_recruited()
            || !self.autonomous
            || self.bims[who].task.is_some()
            || !self.bims[who].queue.is_empty()
            || !self.bims[who].character.arrived()
            || self.bims[who].flee_wait > 0.0
        {
            return;
        }
        self.bims[who].flee_wait = FLEE_EVERY;
        let Some(to) = self
            .room
            .filth
            .somewhere_cleaner(self.bims[who].character.pos, FLEE_LOOK)
        else {
            return;
        };
        let nav = self.maps.pick(self.room.bath.is_open());
        let route = nav.path(self.bims[who].character.pos, nav.nearest_free(to));
        if !route.is_empty() {
            self.bims[who].character.follow_path(route);
        }
    }

    // --- interrupting, and getting back to it ----------------------------

    /// Put the running chain down and queue it to be picked up next.
    ///
    /// It goes on the *front*, so when one interruption interrupts another the
    /// chains come back in the order they were displaced: the most recently
    /// dropped is the first one resumed.
    fn interrupt(&mut self, who: usize) {
        if let Some(task) = self.bims[who].task.take() {
            // A conversation is dropped rather than put down. Half of one is
            // worth nothing, the other half of it has walked off by now, and
            // a chain that walks back to a spot on the deck to talk to nobody
            // is worse than no chain at all.
            let keep = task.kind() != Kind::Chat;
            self.bims[who].chat_topic = 0;
            let saved = task.suspend(&mut self.bims[who].character, &mut self.room);
            if keep {
                self.bims[who].queue.insert(0, saved);
            }
        }
    }

    /// The same, for a new order from the player, which also cancels any move
    /// that was waiting on a door.
    ///
    /// The errand opening that door goes with it: the two exist only to serve
    /// each other, so keeping the door errand would have the Bim walk back and
    /// work a panel for a destination nobody is going to any more. It is put
    /// down properly rather than dropped on the floor, so the hands and the
    /// scripting come back the way a suspend leaves them.
    fn interrupt_for_order(&mut self, who: usize) {
        let stale = self.bims[who].pending_move.take().is_some() && self.on_a_door_errand(who);
        if let Some(task) = self.bims[who].task.take() {
            let saved = task.suspend(&mut self.bims[who].character, &mut self.room);
            if !stale {
                self.bims[who].queue.insert(0, saved);
            }
        }
    }

    /// Whether the Bim is at this moment on its way to work the door panel.
    fn on_a_door_errand(&self, who: usize) -> bool {
        self.bims[who]
            .task
            .as_ref()
            .is_some_and(|task| matches!(task.kind(), Kind::Switch(Switch::BathDoor(_))))
    }

    /// Whether the Bim could actually walk to where a chain would start it.
    ///
    /// A chain that cannot be begun is not begun. A walk with nowhere to go
    /// reports itself arrived the instant it starts, and a chain that takes
    /// that for arrival runs through every remaining step in a single frame —
    /// which is how a Bim shut in the heads used to end up at the dining
    /// table without having walked there.
    fn can_begin(&self, who: usize, kind: Kind) -> bool {
        // Somebody else's hands are already in it. See [`Exclusive`].
        if exclusive(kind).is_some_and(|want| self.taken_by_other(who, want)) {
            return false;
        }
        // And one of everything the errand walks to is free — the closest
        // to hand, or the next along. See `task::Picks`.
        let taken = self.taken_for(who);
        let from = self.bims[who].character.pos;
        if !task::can_pick_all(kind, &self.room, from, &taken) {
            return false;
        }
        let Some(to) = task::first_station(who, kind, &self.room, from, &taken) else {
            return true;
        };
        self.can_reach(who, to)
    }

    /// Whether any *other* Bim is on an errand that has the run of `want`.
    ///
    /// Only what is actually running counts. A chain on the queue is put down:
    /// its hands are off the pot, and holding the galley for it would have one
    /// Bim's interrupted meal keep the other out of the galley all night.
    fn taken_by_other(&self, who: usize, want: Exclusive) -> bool {
        self.bims.iter().enumerate().any(|(i, bim)| {
            i != who
                && bim
                    .task
                    .as_ref()
                    .is_some_and(|task| exclusive(task.kind()) == Some(want))
        })
    }

    /// Which Bim has a fixture — by its kind and index — counting from 1,
    /// or 0 for nobody: the one whose errand or queued chain picked it. The
    /// host says whose it is when a menu item will not start.
    fn fixture_held_by(&self, holds: impl Fn(&task::Picks) -> bool) -> u32 {
        self.bims
            .iter()
            .position(|bim| {
                bim.task.as_ref().is_some_and(|t| holds(&t.picks()))
                    || bim.queue.iter().any(|s| holds(&s.picks()))
            })
            .map_or(0, |i| i as u32 + 1)
    }

    /// Who is keeping `who` out of the galley, counting from 1, or 0 when
    /// a meal could start now — some worktop, hob and cold store each
    /// free. With several of each, one Bim cooking is not a galley taken;
    /// it is the *last* free one gone that is.
    pub fn galley_busy_by(&self, who: usize) -> u32 {
        let taken = self.taken_for(who);
        let from = self.bims[who].character.pos;
        if task::can_pick_all(Kind::Meal(Dish::Stew), &self.room, from, &taken) {
            return 0;
        }
        self.fixture_held_by(|p| p.hob.is_some() || p.worktop.is_some() || p.fridge.is_some())
    }

    pub fn hob_held_by(&self, i: usize) -> u32 {
        self.fixture_held_by(|p| p.hob == Some(i))
    }

    pub fn fridge_held_by(&self, i: usize) -> u32 {
        self.fixture_held_by(|p| p.fridge == Some(i))
    }

    pub fn dishwasher_held_by(&self, i: usize) -> u32 {
        self.fixture_held_by(|p| p.dishwasher == Some(i))
    }

    pub fn broom_held_by(&self, i: usize) -> u32 {
        self.fixture_held_by(|p| p.locker == Some(i))
    }

    pub fn heads_held_by(&self, i: usize) -> u32 {
        self.fixture_held_by(|p| p.bath == Some(i))
    }

    pub fn shower_held_by(&self, i: usize) -> u32 {
        self.fixture_held_by(|p| p.shower == Some(i))
    }

    /// Every fixture the *other* Bims hold, for `who`'s next pick: their
    /// errands' and their queued chains' alike, since a half-cooked meal
    /// put down keeps its pot on its hob.
    fn taken_for(&self, who: usize) -> task::Taken {
        let mut taken = task::Taken::default();
        for (i, bim) in self.bims.iter().enumerate() {
            if i == who {
                continue;
            }
            if let Some(t) = &bim.task {
                taken.add(&t.picks());
            }
            for s in &bim.queue {
                taken.add(&s.picks());
            }
        }
        taken
    }

    /// Shut the bathroom door behind whoever last walked through it.
    ///
    /// Crossing the bulkhead line arms the closer; three seconds later the
    /// door slides shut, wherever the Bim has got to by then. The chains that
    /// shut the door by hand still do — this only catches the door nobody
    /// thought to close, which otherwise stood open for the rest of the game.
    ///
    /// A Bim's own position is a frame old here, which at three seconds does
    /// not matter, and the doorway check is on the same footing as the walk it
    /// just finished.
    ///
    /// With two aboard every guard below has to hold for *either* of them. The
    /// door does not know whose trip it is: one crew member walking towards it
    /// is reason enough to hold it, and only when nobody is inside, nobody is
    /// heading through and nobody is standing in the opening does it shut.
    fn tick_door_closer(&mut self, dt: f32) {
        let inside = self
            .bims
            .iter()
            .any(|b| self.room.bath.shell.contains(b.character.pos));
        if inside != self.bim_was_inside {
            self.bim_was_inside = inside;
            self.door_shut_in = DOOR_SHUT_AFTER;
        }

        // Nothing to close, or nothing armed: a door shut by hand in the
        // meantime disarms the closer rather than leaving it to fire later.
        if self.door_shut_in <= 0.0 || !self.room.bath.is_open() {
            self.door_shut_in = 0.0;
            return;
        }

        // Somebody is on their way through: the door waits. A route is planned
        // once and never replanned, so a door that shuts across one leaves the
        // Bim walking into the panels for ever, and the chain waiting on an
        // arrival that cannot happen.
        //
        // Asked of each Bim against *its own* side of the bulkhead, not the
        // `inside` above: that one is true if either of them is in there, and
        // a Bim out on the deck heading for the heads while the other is
        // already inside would otherwise not count as crossing.
        let crossing = self.bims.iter().any(|b| {
            let here = self.room.bath.shell.contains(b.character.pos);
            b.character
                .destination()
                .is_some_and(|to| self.room.bath.shell.contains(to) != here)
        });
        if crossing {
            return;
        }

        // Still in the opening. The panels do not close on the Bim, so the
        // count is held where it is until everyone is clear of the doorway —
        // which also means the three seconds run from walking through it,
        // not from the moment the bulkhead line was crossed.
        let doorway = self.room.bath.door.expand(DOOR_CLEARANCE);
        if self.bims.iter().any(|b| doorway.contains(b.character.pos)) {
            return;
        }

        self.door_shut_in -= dt;
        if self.door_shut_in <= 0.0 {
            self.door_shut_in = 0.0;
            self.room.bath.set_open(false);
        }
    }

    fn can_reach(&self, who: usize, to: Vec2) -> bool {
        let nav = self.maps.pick(self.room.bath.is_open());
        nav.can_reach(self.bims[who].character.pos, to)
    }

    /// Clear the way for a new errand. Returns false when the Bim is already
    /// on that very errand, so asking twice does not restart it.
    fn take_over(&mut self, who: usize, kind: Kind, minutes: f32) -> bool {
        if let Some(task) = &self.bims[who].task {
            if task.kind() == kind && (kind != Kind::Rest || task.rest_minutes() == minutes) {
                return false;
            }
        }
        self.interrupt(who);
        true
    }

    /// Back on its feet with something waiting: pick it up. Waits for an
    /// ordered walk to finish first, so the Bim gets where it was sent before
    /// returning to what it was doing.
    fn pump_queue(&mut self, who: usize) {
        // Under orders, work that was put down stays put down. The queue is
        // kept, not thrown away, so letting the Bim go picks it all up again.
        if self.bims[who].character.is_recruited()
            || self.bims[who].task.is_some()
            || self.bims[who].queue.is_empty()
            || !self.bims[who].character.arrived()
        {
            return;
        }
        if !self.queue_ready(who) {
            return;
        }
        let saved = self.bims[who].queue.remove(0);
        let taken = self.taken_for(who);
        self.bims[who].task = Some(Task::resume(
            saved,
            &mut self.bims[who].character,
            &mut self.room,
            &self.maps,
            &taken,
        ));
    }

    /// Whether the chain at the front of the queue is one the Bim should be
    /// getting on with right now.
    ///
    /// Queued work whose way is shut waits for the door rather than being
    /// thrown away — but it must not hold up everything else while it waits.
    /// A Bim with an unreachable meal on the queue still has to be free to go
    /// and do something it *can* reach, or it stands there until it starves.
    fn queue_ready(&self, who: usize) -> bool {
        self.bims[who].queue.first().is_some_and(|saved| {
            if saved.kind() == Kind::Rest && (self.eats_first(who) || self.goes_first(who)) {
                return false;
            }
            // Picking a chain back up is beginning one as far as the galley is
            // concerned. `pump_queue` does not go through `can_begin` — it
            // resumes rather than starts — so the check has to be here as
            // well, or a half-cooked meal resumes into a galley the other Bim
            // is standing in.
            if exclusive(saved.kind()).is_some_and(|want| self.taken_by_other(who, want))
                || saved.picks().clashes(&self.taken_for(who))
            {
                return false;
            }
            saved
                .resume_station(&self.room, self.bims[who].character.pos)
                .is_none_or(|to| self.can_reach(who, to))
        })
    }

    /// Whether a meal comes before a sleep the Bim was about to start.
    ///
    /// Food outranks rest when both are going begging. A Bim that turns in
    /// starving is a Bim that sleeps six hours on an empty stomach and wakes
    /// no better off, having lost health all night, while a meal costs it
    /// three quarters of an hour and puts the need away entirely.
    ///
    /// Only while there is a meal to be had, though: with the cold store empty
    /// or the galley out of reach, holding bedtime off would leave the Bim
    /// standing about all night instead, hungry *and* tired.
    fn eats_first(&self, who: usize) -> bool {
        // Only while the Bim is free to go and see to it. With autonomy off,
        // or under orders, nothing would ever start that meal — and a night
        // held back for a meal that is never cooked is a Bim that never
        // sleeps at all.
        self.autonomous
            && !self.bims[who].character.is_recruited()
            && self.bims[who]
                .needs
                .trigger(Need::Food)
                .caught(self.bims[who].needs.level(Need::Food))
            && self
                .need_station(who, Need::Food)
                .is_some_and(|to| self.can_reach(who, to))
    }

    /// Whether a trip to the heads comes before a sleep the Bim was about to
    /// start. You go before bed.
    ///
    /// Unlike [`Game::eats_first`] this does not wait for the trigger. The
    /// restroom need is the one thing that keeps draining through the night,
    /// and a night is six hours: turn in at four fifths and the Bim wakes at
    /// nothing, which is how three visits a day become four. Anything under
    /// [`needs::BEFORE_BED`] is worth emptying out first.
    ///
    /// The usual guards: only while the Bim is free to see to it — autonomy
    /// on, not under orders — and only while the pan is actually reachable. A
    /// locked door is not a reason to keep a tired Bim up.
    fn goes_first(&self, who: usize) -> bool {
        self.autonomous
            && !self.bims[who].character.is_recruited()
            && self.bims[who].needs.level(Need::Restroom) < crate::needs::BEFORE_BED
            && self.can_use_toilet(who)
    }

    /// Whether the rest trigger is asking for a bed — switched on, and the
    /// level past it. Not whether the Bim can have one.
    fn wants_bed(&self, who: usize) -> bool {
        self.bims[who]
            .needs
            .trigger(Need::Rest)
            .caught(self.bims[who].needs.level(Need::Rest))
    }

    /// Whether the thing waiting at the front of the queue is a lie-down.
    fn bed_is_queued(&self, who: usize) -> bool {
        self.bims[who]
            .queue
            .first()
            .is_some_and(|saved| saved.kind() == Kind::Rest)
    }

    // --- the agenda, for the host's checklist ----------------------------

    /// The chain running now, if any, followed by everything queued behind it.
    pub fn agenda_len(&self, who: usize) -> u32 {
        self.bims[who].task.is_some() as u32 + self.bims[who].queue.len() as u32
    }

    /// Which errand entry `i` is: see the `JOB_` codes. Zero if out of range.
    pub fn agenda_job(&self, who: usize, i: u32) -> u32 {
        match self.agenda_at(who, i) {
            Some((kind, minutes, _)) => job_code(kind, minutes),
            None => 0,
        }
    }

    /// How far through entry `i` is, 0 to 1. A queued chain keeps whatever it
    /// had reached when it was put down.
    pub fn agenda_progress(&self, who: usize, i: u32) -> f32 {
        self.agenda_at(who, i)
            .map_or(0.0, |(_, _, progress)| progress)
    }

    /// Non-zero for the one entry that is actually running.
    pub fn agenda_active(&self, who: usize, i: u32) -> u32 {
        (i == 0 && self.bims[who].task.is_some()) as u32
    }

    fn agenda_at(&self, who: usize, i: u32) -> Option<(Kind, f32, f32)> {
        let running = self.bims[who].task.is_some() as u32;
        if i < running {
            let task = self.bims[who].task.as_ref()?;
            return Some((task.kind(), task.rest_minutes(), task.progress()));
        }
        let saved = self.bims[who].queue.get((i - running) as usize)?;
        Some((saved.kind(), saved.rest_minutes(), saved.progress()))
    }

    // --- deciding for itself ---------------------------------------------

    /// The end of it. Whatever it was doing is dropped, and so is everything
    /// waiting behind it: there is no one left to do any of it.
    fn die(&mut self, who: usize) {
        // The others are told. This is the one thing that can happen aboard
        // that nobody could fail to notice, so it goes in every surviving
        // diary regardless of where they were standing — unlike `witnessed`,
        // which asks whether they could see it.
        for other in 0..self.bims.len() {
            if other != who && self.bims[other].is_alive() {
                self.remember(other, What::CrewDied, who as u32);
            }
        }
        self.bims[who].task = None;
        self.bims[who].queue.clear();
        self.bims[who].character.die();
    }

    pub fn is_alive(&self, who: usize) -> bool {
        self.bims[who].is_alive()
    }

    pub fn health(&self, who: usize) -> f32 {
        self.bims[who].health.points()
    }

    /// 0 when well fed, then 1, 2, 3 for the three stages of malnutrition.
    pub fn malnutrition(&self, who: usize) -> u32 {
        self.bims[who].health.stage().stage()
    }

    /// 0 wide awake, then 1, 2, 3 for sleepy, deprived and wrecked.
    pub fn drowsiness(&self, who: usize) -> u32 {
        self.bims[who].health.drowsiness().stage()
    }

    /// Whether it is actually going somewhere. For the probes: a Bim that is
    /// merely being shoved aside by the other one is not walking.
    #[allow(dead_code)]
    pub fn is_walking(&self, who: usize) -> bool {
        self.bims[who].character.is_walking()
    }

    /// Stand a Bim somewhere, for the probes.
    ///
    /// Nothing in the game does this — a Bim is where it walked to — but a
    /// probe that wants two of them to meet head-on has to be able to set the
    /// meeting up, and waiting for one to happen by chance is not a test.
    /// Snapped to somewhere a body can actually stand, and the spot returned.
    /// "On the deck" is not the same question as "a body fits here": the nav
    /// grid inflates every obstacle by a body's margin, so the strip of deck
    /// along the counter front reads as floor and is not walkable.
    #[allow(dead_code)]
    pub fn put_for_probe(&mut self, who: usize, at: Vec2) -> Vec2 {
        let nav = self.maps.pick(self.room.bath.is_open());
        let spot = nav.nearest_free(at);
        self.bims[who].character.stand_at(spot);
        spot
    }

    /// Post a Bim somewhere: drop what it is doing, walk there, and stand
    /// there until told otherwise. What "take the helm" and "go ashore" are
    /// made of. It still goes off on its errands when a need bites and
    /// comes back afterwards — see [`Game::return_to_post`] — and a player
    /// order to anywhere else takes the post away. Any of the crew, not
    /// only the player's: the ship posts the station's people ashore before
    /// it casts off. Snapped to somewhere a body can stand; false, and no
    /// post, when there was no route.
    pub fn send_to(&mut self, who: usize, to: Vec2) -> bool {
        self.dispatch(who, to, true)
    }

    /// Walk a Bim somewhere, dropping what it is doing, without posting it
    /// there: once there it picks its errands back up. What calling the
    /// crew back aboard before the ship casts off is made of — a crew
    /// member posted at the airlock would stand there for the rest of the
    /// voyage.
    pub fn walk_to(&mut self, who: usize, to: Vec2) -> bool {
        self.dispatch(who, to, false)
    }

    fn dispatch(&mut self, who: usize, to: Vec2, post: bool) -> bool {
        if who >= self.bims.len() || !self.is_alive(who) {
            return false;
        }
        let nav = self.maps.pick(self.room.bath.is_open());
        let target = nav.nearest_free(to);
        let route = nav.path(self.bims[who].character.pos, target);
        if route.is_empty() {
            return false;
        }
        self.interrupt_for_order(who);
        self.bims[who].character.follow_path(route);
        if post {
            self.bims[who].character.set_post(Some(target));
        }
        self.mark(target, false);
        true
    }

    /// Stand a Bim at its post this instant, for the probes and the
    /// fixtures: the walk is the room's business and a test of the helm is
    /// not a test of the walk.
    pub fn post_for_probe(&mut self, who: usize, at: Vec2) -> Vec2 {
        let spot = self.put_for_probe(who, at);
        self.bims[who].character.set_post(Some(spot));
        spot
    }

    /// Take a Bim's post away without sending it anywhere: it finishes the
    /// walk it is on, if any, and picks its errands back up. What the
    /// player's own trip to the helm ends with — the order given, the seat
    /// is the job's to fill.
    pub fn stand_down(&mut self, who: usize) {
        if let Some(bim) = self.bims.get_mut(who) {
            bim.character.set_post(None);
        }
    }

    /// Where the Bim has been posted, if anywhere.
    pub fn post_of(&self, who: usize) -> Option<Vec2> {
        self.bims.get(who).and_then(|b| b.character.post())
    }

    /// Whether the Bim is standing at (or within `slack` of) its post.
    pub fn at_post(&self, who: usize, slack: f32) -> bool {
        self.bims
            .get(who)
            .and_then(|b| {
                b.character
                    .post()
                    .map(|p| (b.character.pos - p).len() <= slack)
            })
            .unwrap_or(false)
    }

    /// Whose coverall a Bim wears: the ship's or the station's. Drawing only.
    /// Where the bodies that are on this deck but not in this room stand,
    /// in room units, for the doors to open for. A docked station's people:
    /// the world sets it every step while the rooms are joined and clears
    /// it after. Nothing else reads it — they are not crew, not selectable,
    /// not solid — so a visitor walking through a door is the whole of it,
    /// bar a click on one that is down (`set_visitors_down`, which this
    /// clears: nobody is down until the world says so again).
    pub fn set_visitors(&mut self, at: Vec<Vec2>) {
        self.visitors = at;
        self.visitors_down.clear();
        self.visitors_hailable.clear();
    }

    /// Which of the visitors are down — dead or out cold in their own
    /// room — index for index with [`Game::set_visitors`], right after it
    /// every step, the way `set_hostiles_peeking` follows `set_hostiles`.
    /// A visitor that is down is a body under a click (`HIT_VISITOR`) for
    /// looting; one on its feet is not hit at all.
    pub fn set_visitors_down(&mut self, down: &[bool]) {
        self.visitors_down = down.to_vec();
    }

    /// Which of the visitors may be spoken to — the world's mercenaries
    /// for hire — index for index with [`Game::set_visitors`] and told
    /// right after it like `set_visitors_down`. One of these on its feet
    /// is hit under a click (`HIT_VISITOR`) the way a body down is, so a
    /// menu can open on it; [`Game::visitor_down`] says which it was. The
    /// room knows nothing of what is said: no strings cross this boundary.
    pub fn set_visitors_hailable(&mut self, hailable: &[bool]) {
        self.visitors_hailable = hailable.to_vec();
    }

    /// Whether that visitor is down, as the world last said — what a menu
    /// on `HIT_VISITOR` asks to know whether it is a body to loot or
    /// somebody to speak to.
    pub fn visitor_down(&self, visitor: usize) -> bool {
        self.visitors_down.get(visitor).copied().unwrap_or(false)
    }

    pub fn set_uniform(&mut self, who: usize, uniform: crate::character::Uniform) {
        if let Some(bim) = self.bims.get_mut(who) {
            bim.character.set_uniform(uniform);
        }
    }

    /// Whose coverall that Bim wears; the crew's for no such Bim.
    pub fn uniform(&self, who: usize) -> crate::character::Uniform {
        self.bims
            .get(who)
            .map_or(crate::character::Uniform::Crew, |b| b.character.uniform())
    }

    /// Back to the post once the errand that took it away is done. Only when
    /// there is nothing running and nothing queued, and only when the last
    /// route has been walked to its end — handing a Bim a fresh route every
    /// frame is how one comes to march on the spot for ever.
    fn return_to_post(&mut self, who: usize) {
        let bim = &self.bims[who];
        let Some(post) = bim.character.post() else {
            return;
        };
        if bim.task.is_some()
            || !bim.queue.is_empty()
            || !bim.character.arrived()
            || bim.character.is_seated()
            || (bim.character.pos - post).len() <= POST_SLACK
        {
            return;
        }
        let nav = self.maps.pick(self.room.bath.is_open());
        let route = nav.path(bim.character.pos, nav.nearest_free(post));
        self.bims[who].character.follow_path(route);
    }

    /// Send a Bim somewhere by the same route a player order would take, but
    /// without needing it selected or it being the player's. For the probes.
    /// False when there was no route, which leaves the Bim wandering — a probe
    /// that does not check gets a wander and calls it a walk.
    #[allow(dead_code)]
    pub fn send_for_probe(&mut self, who: usize, to: Vec2) -> bool {
        let nav = self.maps.pick(self.room.bath.is_open());
        let route = nav.path(self.bims[who].character.pos, nav.nearest_free(to));
        let got = !route.is_empty();
        self.bims[who].character.follow_path(route);
        got
    }

    /// Stop a Bim wandering off, so a probe can watch one walk and nothing
    /// else. The same switch `r` throws, but for any of the crew.
    #[allow(dead_code)]
    pub fn recruit_for_probe(&mut self, who: usize, on: bool) {
        self.bims[who].character.set_recruited(on);
    }

    /// How much this Bim is slowed by the other being in the same space, for
    /// the probes: 1 clear, less than 1 squeezing past.
    #[allow(dead_code)]
    pub fn crowding_for_probe(&self, who: usize) -> f32 {
        self.crowding(who)
    }

    /// The route a Bim is on, and the solids it is kept out of, for the probes.
    #[allow(dead_code)]
    pub fn route_for_probe(&self, who: usize) -> (Vec<Vec2>, Vec<Rect>) {
        (
            self.bims[who].character.path_for_probe().to_vec(),
            self.blockers.clone(),
        )
    }

    /// The two sides of the chopping board, for the layout probe.
    #[allow(dead_code)]
    pub fn board_for_probe(&self) -> [crate::room::Cut; 2] {
        self.room.worktops[0].board_sides
    }

    /// Whether this Bim has the broom in its hands. For the probes.
    #[allow(dead_code)]
    pub fn holds_broom_for_probe(&self, who: usize) -> bool {
        self.bims[who].character.main_held() == Held::Broom
    }

    /// Take an exact amount off one tile, for the probes. `foul_for_probe`
    /// only stages the worst there is, and the interesting question about
    /// what a mess *looks* like is the faint end of the range.
    #[allow(dead_code)]
    pub fn soil_for_probe(&mut self, at: Vec2, cost: f32) {
        self.room.filth.soil(at, cost, filth::Mess::Grime);
    }

    /// Foul one tile of the deck outright, for the probes: staging a mess is
    /// the only way to test the thing that clears one.
    #[allow(dead_code)]
    pub fn foul_for_probe(&mut self, at: Vec2) {
        self.room.filth.sick_on(at);
    }

    /// Run a need down by hand, for the probes.
    #[allow(dead_code)]
    /// Take one thing out of the cold store, for a probe that wants the
    /// shelf bare without waiting for it to be eaten down.
    pub fn take_for_probe(&mut self, crop: hydro::Crop) {
        self.room.take(crop);
    }

    pub fn spend_for_probe(&mut self, who: usize, need: u32, amount: f32) {
        if let Some(need) = Need::from_index(need) {
            self.bims[who].needs.spend(need, amount);
        }
    }

    /// Whether a Bim could walk to a spot. For the probes.
    #[allow(dead_code)]
    pub fn can_reach_for_probe(&self, who: usize, to: Vec2) -> bool {
        self.can_reach(who, to)
    }

    /// Whether a Bim is sitting or lying. For the probes.
    #[allow(dead_code)]
    pub fn is_seated_for_probe(&self, who: usize) -> bool {
        self.bims[who].character.is_seated()
    }

    /// Where the route a Bim is on ends, or `None` when it is not on one. For
    /// the probes: it is how "standing aside is not replanning" is checked.
    #[allow(dead_code)]
    pub fn destination_for_probe(&self, who: usize) -> Option<Vec2> {
        self.bims[who].character.destination()
    }

    /// Whether the Bim has dropped off on its feet this instant.
    pub fn is_napping(&self, who: usize) -> bool {
        self.bims[who].character.is_napping()
    }

    /// Whether it is standing there having lost the thread of what it was on.
    pub fn is_stalled(&self, who: usize) -> bool {
        self.bims[who].task.as_ref().is_some_and(|t| t.stalled())
    }

    /// How many times the errand running now has been fumbled.
    pub fn task_fumbles(&self, who: usize) -> u32 {
        self.bims[who].task.as_ref().map_or(0, |t| t.fumbles())
    }

    pub fn store_veg(&self) -> u32 {
        self.room.veg
    }

    pub fn store_tofu(&self) -> u32 {
        self.room.tofu
    }

    /// What is in the cold store, set outright: vegetables, blocks of tofu,
    /// pots of stew and fibre. What the world does to a room it opens with
    /// a larder already in it — a station's, whose people have been living
    /// there — where a ship's store starts with what its design carries;
    /// and, every step, how the hold's fibre reaches the bay, since it is
    /// the hold that keeps the count.
    pub fn set_stock(&mut self, veg: u32, tofu: u32, stew: u32, fibre: u32) {
        self.room.veg = veg;
        self.room.tofu = tofu;
        self.room.stew = stew;
        self.room.fibre = fibre;
    }

    /// Pots of stew on the shelf, cooked ahead.
    pub fn store_stew(&self) -> u32 {
        self.room.stew
    }

    /// Fibre in the cold store, for the bay's target and the panel.
    pub fn store_fibre(&self) -> u32 {
        self.room.fibre
    }

    /// Fibre put away since the last call, for the world to move into the
    /// hold. The room's own count keeps it too — `set_stock` is what puts
    /// the hold's number back on the shelf every step.
    pub fn take_harvested_fibre(&mut self) -> u32 {
        core::mem::take(&mut self.room.harvested_fibre)
    }

    // --- the timetable ----------------------------------------------------

    /// What hour `hour` is set aside for: 0 anything, 1 sleep.
    pub fn schedule_slot(&self, hour: u32) -> u32 {
        match self.schedule.slot(hour) {
            Slot::Anything => 0,
            Slot::Sleep => 1,
        }
    }

    pub fn set_schedule_slot(&mut self, hour: u32, slot: u32) {
        self.schedule.set(
            hour,
            if slot == 1 {
                Slot::Sleep
            } else {
                Slot::Anything
            },
        );
    }

    /// Rested above this, a scheduled sleep is passed over.
    pub fn schedule_ignore_above(&self) -> f32 {
        IGNORE_ABOVE
    }

    /// A need fallen past its trigger sends the Bim to see to it.
    ///
    /// Only when it is otherwise free: a need arising mid-errand waits for
    /// that errand to finish rather than interrupting it, and a walk the
    /// player ordered counts as having something on, so the Bim gets where it
    /// was sent before going off on its own account. Nothing here fires while
    /// the Bim is asleep, because nothing drains while it is asleep.
    fn consider_errand(&mut self, who: usize) {
        // Queued work that the Bim cannot get to does not count as having
        // something on: it would block every need there is while it waited.
        // Nor does a scheduled sleep the Bim is too hungry — or too desperate
        // for the heads — to take yet: that is what lets the errand below be
        // started in its place.
        if self.bims[who].character.is_recruited()
            || !self.autonomous
            || self.bims[who].task.is_some()
            || self.queue_ready(who)
            || !self.bims[who].character.arrived()
        {
            return;
        }
        // Most pressing first, but a need it cannot do anything about — an
        // empty fridge, a locked door, a rest that is nobody's business here —
        // must not block the ones it can. An empty cold store would otherwise
        // pin hunger at zero and the Bim would never go to the heads again.
        //
        // These are the needs that are *not* work: there is no row for sleep
        // and none for the heads, because neither is a thing the player gets
        // to put off. Hunger is the odd one — being hungry is a need and
        // cooking is a job — so it is handed to `do_some_work` below and
        // ordered against the rest of the work there.
        for need in self.bims[who].needs.urgent() {
            let started = match need {
                // The timetable is still what sends the Bim to bed on an
                // ordinary day. The rest trigger is the floor under it: a Bim
                // this far gone has either had its night painted out of the
                // schedule or has been kept from taking it, and standing there
                // getting no sleep at all is not an answer. It defers to a
                // meal and to the heads exactly as a scheduled night does —
                // see `eats_first` and `goes_first`.
                Need::Rest => {
                    !self.eats_first(who) && !self.goes_first(who) && self.rest(who, SLEEP_MINUTES)
                }
                // The one thing the priority list is not allowed to do is
                // starve somebody. Once going without has actually begun to
                // tell, a meal jumps the queue whatever the cook row says —
                // the list is a statement about what to do next, not about
                // whether to eat at all.
                Need::Food => self.going_without(who) && self.make_food(who),
                Need::Restroom => self.use_toilet(who),
                // Going and having a word. Not work — there is no row for it
                // in the priority list and no number to put it off with —
                // because the alternative is a player who can set a Bim to
                // die of loneliness by accident.
                Need::Company => self.chat(who),
                // Nothing aboard cleans anything yet. Being filthy is
                // something that happens to the Bim rather than something it
                // can go and see to, so it starts no errand — what it does
                // instead is in `mind_the_mess` and `flee_filth`.
                Need::Cleanliness => false,
                // A day's grime: a shower, where there is one. Where there
                // is not, nothing — the Bim goes on wanting one and nothing
                // else comes of it.
                Need::Hygiene => self.take_shower(who),
            };
            if started {
                return;
            }
        }

        // Bedtime is waiting and the Bim would rather go first. This is the
        // other half of the rule in `goes_first`: that one holds the sleep
        // back, and without this nothing would ever start the trip it is being
        // held for — the need is nowhere near its trigger, so the loop above
        // passed straight over it and the Bim would stand there all night.
        //
        // A bed the rest trigger is asking for counts the same as a queued
        // one. It has to: the loop above has just refused to start that sleep
        // for this very reason, so without this the two rules would deadlock
        // and the Bim would neither go nor turn in.
        if (self.bed_is_queued(who) || self.wants_bed(who))
            && self.goes_first(who)
            && self.use_toilet(who)
        {
            return;
        }

        // Nothing pressing on the body. What is left is work, and the player
        // says what order that gets done in.
        if self.do_some_work(who) {
            return;
        }

        // Nothing could be started — and a shut door may be the whole of the
        // reason. A Bim in the heads is on the wrong side of it for the
        // galley, so every meal reads as unreachable and hunger sits at
        // nothing while the Bim stands there: exactly what happens when a
        // player locks it in and then unlocks the door again, since unlocking
        // leaves the door shut. The panel is on this side of it, so letting
        // itself out is the errand; the need comes round again with the way
        // clear as soon as the door is open.
        if self.shut_in(who) {
            self.send_to_switch(who, Switch::BathDoor(true));
        }
    }

    /// Whatever work is going, in the order the player asked for.
    ///
    /// Everything here is discretionary: a tray is only asking if the bay is
    /// running, the deck only wants sweeping if something is on it, and
    /// cooking is only offered to a Bim that is actually hungry. Whichever of
    /// those is going is collected, sorted, and tried in turn — the first one
    /// that can actually be started wins, so a galley the other Bim is
    /// standing in does not stop this one getting on with the bay.
    ///
    /// The sort is **stable**, and that is what makes an untouched list behave
    /// exactly as the fixed order did before there was a list: all equal, the
    /// jobs come out in the order they were offered, which is the order they
    /// used to be written in.
    fn do_some_work(&mut self, who: usize) -> bool {
        for job in self.work_on_offer(who) {
            let started = match job {
                // Hunger first: a Bim past its food trigger eats, and only
                // one that is not cooks for the shelf. Both want the galley,
                // so a meal that cannot start is not a stew that can.
                Job::Cook => {
                    (self.is_hungry(who) && self.make_food(who))
                        || (self.wants_stew()
                            && !self.priorities.never(Job::Cook)
                            && self.make_stew(who))
                }
                Job::Helm => self.man_the_helm(who),
                Job::Plant | Job::Cut => self.tend_bay(who),
                Job::Clean => self.sweep_up(who),
                Job::Craft => self.craft(who),
                Job::Mine => self.go_outside(who),
                // A load to a site. The harvest's carry is not offered on
                // its own — see `waits_on` — but a site's is an errand.
                Job::Haul => self.haul(who),
                Job::Build => self.build(who),
                // A wound to dress, somebody's: the same errand the player
                // orders from the menu on a body, chosen by the room.
                Job::Medical => self
                    .medical_on_offer(who)
                    .is_some_and(|(patient, part)| self.bandage(who, patient, part)),
            };
            if started {
                return true;
            }
        }
        false
    }

    /// Whom `who` would dress, and where, if it took the medical row: its
    /// own worst part while it bleeds, else the crewmate with the most
    /// wounds open and that one's worst part. `None` with no bandage to
    /// hand, nobody bleeding, or a helper in no state to do it — dead, out
    /// cold, outside, or under orders, since a recruited Bim takes no
    /// errand of its own.
    ///
    /// The patient may be out cold — it lies still, which is the easiest
    /// patient there is — but not outside, where the walk cannot follow,
    /// and not somewhere the helper cannot get to: the chain would start,
    /// find no route and be given up, and this would offer it again on the
    /// next step, for ever. A part somebody else is already walking over to
    /// dress is left to them, so two crew do not dress one wound twice and
    /// waste the second's ten minutes.
    fn medical_on_offer(&self, who: usize) -> Option<(usize, Part)> {
        if self.room.bandages == 0
            || !self.bims.get(who).is_some_and(|b| b.is_alive())
            || self.bims[who].character.is_unconscious()
            || self.bims[who].character.is_outside()
            || self.bims[who].character.is_recruited()
        {
            return None;
        }
        // What the others have in hand, by patient and part.
        let being_dressed: Vec<(usize, u32)> = self
            .bims
            .iter()
            .enumerate()
            .filter(|&(other, _)| other != who)
            .filter_map(|(_, b)| match b.task.as_ref().map(|t| t.kind()) {
                Some(Kind::Bandage { patient, part }) => Some((patient, part)),
                _ => None,
            })
            .collect();
        let worst_part = |patient: usize| -> Option<Part> {
            Part::ALL
                .iter()
                .copied()
                .filter(|p| !being_dressed.contains(&(patient, p.code())))
                .filter(|&p| self.bims[patient].health.wounds(p) > 0)
                .max_by_key(|&p| self.bims[patient].health.wounds(p))
        };
        if let Some(part) = worst_part(who) {
            return Some((who, part));
        }
        let from = self.bims[who].character.pos;
        let patient = (0..self.bims.len())
            .filter(|&p| p != who)
            .filter(|&p| self.bims[p].is_alive() && !self.bims[p].character.is_outside())
            .filter(|&p| self.bims[p].health.bleeding() > 0)
            .filter(|&p| task::patient_stand(&self.room, &self.maps, who, p, from).is_some())
            .max_by_key(|&p| self.bims[p].health.bleeding())?;
        worst_part(patient).map(|part| (patient, part))
    }

    /// What work there is for `who` right now, most important first.
    ///
    /// Kept apart from starting any of it because this is the half the player
    /// can actually see the effect of, and the half worth checking: whether a
    /// job is *begun* also depends on the galley being free and the broom
    /// being in its locker, and those have nothing to do with the list.
    fn work_on_offer(&self, who: usize) -> Vec<Job> {
        let mut offered: Vec<Job> = Vec::new();
        // Somebody bleeding, and a bandage to put on it. Offered first,
        // though its code is the last: among equals the list keeps the
        // order offered, and an untouched list has a wound dressed before
        // the blood under it is swept up or the stew put on.
        if self.medical_on_offer(who).is_some() {
            offered.push(Job::Medical);
        }
        // Cooking: hunger, or the shelf being short of stew. The stew half is
        // not offered without something to make one of — a job that cannot
        // be begun is a job that comes round every frame and starts nothing.
        // Which of the two a Bim then does is `do_some_work`'s: the hungry
        // one eats.
        if self.is_hungry(who) || self.wants_stew() {
            offered.push(Job::Cook);
        }
        // What the bay wants decides which of the two bay rows applies. Asked
        // now rather than assumed: a tray with something ripe in it is a
        // cutting and an empty one is a planting, and the player may well
        // have set those a long way apart.
        if let Some((_, job)) = self.bay_work() {
            offered.push(match job {
                hydro::Job::Harvest(_) => Job::Cut,
                hydro::Job::Plant(_, _) => Job::Plant,
            });
        }
        if self.room.filth.dirty_tiles() > 0 {
            offered.push(Job::Clean);
        }
        // The ship wants somebody at the helm and nobody is posted there.
        if self.helm.is_some() && !self.helm_manned() {
            offered.push(Job::Helm);
        }
        // Something to make, at a bench nobody is at.
        if self.craft_on_offer(who).is_some() {
            offered.push(Job::Craft);
        }
        // A belt to walk out to, a suit to wear, and nobody else out there.
        if self.can_go_outside(who) {
            offered.push(Job::Mine);
        }
        // A site short of something a shelf has, and one with everything
        // there and nobody at it.
        if self.haul_on_offer(who).is_some() {
            offered.push(Job::Haul);
        }
        if self.build_on_offer(who).is_some() {
            offered.push(Job::Build);
        }
        // A job switched off is not on offer. The one thing that survives
        // the switch is a meal for a Bim past its hunger: the cook row at
        // never stops the stew for the shelf, not the eating.
        let hungry = self.is_hungry(who);
        offered.retain(|&job| self.waits_on(job) != work::NEVER || (job == Job::Cook && hungry));
        offered.sort_by_key(|&job| self.waits_on(job));
        offered
    }

    /// The first order whose bench is free and reachable, or none. First in
    /// the world's order, which is the recipe table's — so a smelt is picked
    /// over an emitter when both want doing and both benches stand free.
    fn craft_on_offer(&self, who: usize) -> Option<Order> {
        self.orders.iter().copied().find(|order| {
            self.room.benches.get(order.bench).is_some()
                && self.can_begin(
                    who,
                    Kind::Craft {
                        recipe: order.recipe,
                        bench: order.bench,
                    },
                )
        })
    }

    /// Whether `who` could set out on a walk now: the world says there is
    /// one and this Bim may go, a marked rock can be got to from the port,
    /// the room has a suit locker and a port, and the airlock is nobody
    /// else's.
    fn can_go_outside(&self, who: usize) -> bool {
        self.eva
            .as_ref()
            .is_some_and(|eva| eva.allowed.get(who).copied().unwrap_or(false))
            && self.rocks_reachable > 0
            && self.room.suit_locker.is_some()
            && self.room.gangway.is_some()
            && self.room.outside.is_some()
            && self.can_begin(who, Kind::Eva)
    }

    /// Out through the airlock, to the marked rocks, at the world's minutes
    /// a rock.
    pub fn go_outside(&mut self, who: usize) -> bool {
        if !self.can_go_outside(who) {
            return false;
        }
        let minutes = self.eva.as_ref().map(|e| e.tile_minutes).unwrap_or(0.0);
        if minutes <= 0.0 || !self.take_over(who, Kind::Eva, minutes) {
            return false;
        }
        let taken = self.taken_for(who);
        self.bims[who].task = Some(Task::eva(
            who,
            minutes,
            &mut self.bims[who].character,
            &mut self.room,
            &self.maps,
            &taken,
        ));
        true
    }

    /// Off to make the first thing on offer.
    pub fn craft(&mut self, who: usize) -> bool {
        let Some(order) = self.craft_on_offer(who) else {
            return false;
        };
        let kind = Kind::Craft {
            recipe: order.recipe,
            bench: order.bench,
        };
        if !self.take_over(who, kind, order.minutes) {
            return false;
        }
        let taken = self.taken_for(who);
        self.bims[who].task = Some(Task::craft(
            who,
            order.recipe,
            order.bench,
            order.minutes,
            &mut self.bims[who].character,
            &mut self.room,
            &self.maps,
            &taken,
        ));
        true
    }

    /// Where a site is worked from, for `who`: from the deck if any tile
    /// beside it can be stood on from where the Bim is, else from outside
    /// through the airlock if any can be reached from the spot beyond the
    /// port — which wants a suit locker, a port, the world's leave for this
    /// Bim to go out, and nobody else out there. `None` for a site nobody
    /// can get at either way. `false` is inside, `true` outside.
    fn site_reach(&self, who: usize, site: u32) -> Option<bool> {
        let from = self.bims[who].character.pos;
        if task::site_stand(&self.room, &self.maps, site, from, false).is_some() {
            return Some(false);
        }
        let suited = self.room.suit_ok.get(who).copied().unwrap_or(false)
            && self.room.suit_locker.is_some()
            && self.room.gangway.is_some()
            && !self.taken_by_other(who, Exclusive::Airlock);
        let out = self.room.outside?;
        (suited && task::site_stand(&self.room, &self.maps, site, out, true).is_some())
            .then_some(true)
    }

    /// The first site wanting a load that `who` could carry now: something
    /// to fetch, a shelf it can get to, a way to the site, and nobody else
    /// on that site. In the world's order. The site and whether it is
    /// outside.
    fn haul_on_offer(&self, who: usize) -> Option<(u32, bool)> {
        // No site wanting anything is the usual case, and the shelf is not
        // asked about until there is one: this is asked of every idle Bim
        // every step, and a station has shelves by the dozen.
        if !self.room.builds.iter().any(|b| b.haul.is_some()) {
            return None;
        }
        let from = self.bims[who].character.pos;
        if task::nearest_shelf(&self.room, &self.maps, from).is_none() {
            return None;
        }
        self.room
            .builds
            .iter()
            .filter(|b| b.haul.is_some())
            .find_map(|b| {
                let outside = self.site_reach(who, b.site)?;
                let kind = Kind::Haul {
                    site: b.site,
                    outside,
                };
                self.can_begin(who, kind).then_some((b.site, outside))
            })
    }

    /// The first site with everything there that `who` could put together
    /// now, the same way.
    fn build_on_offer(&self, who: usize) -> Option<(u32, bool, f32)> {
        self.room
            .builds
            .iter()
            .filter(|b| b.haul.is_none() && b.minutes > 0.0)
            .find_map(|b| {
                let outside = self.site_reach(who, b.site)?;
                let kind = Kind::Build {
                    site: b.site,
                    outside,
                };
                self.can_begin(who, kind)
                    .then_some((b.site, outside, b.minutes))
            })
    }

    /// Off with a load to the first site that wants one.
    pub fn haul(&mut self, who: usize) -> bool {
        let Some((site, outside)) = self.haul_on_offer(who) else {
            return false;
        };
        if !self.take_over(who, Kind::Haul { site, outside }, 0.0) {
            return false;
        }
        let taken = self.taken_for(who);
        self.bims[who].task = Some(Task::haul(
            who,
            site,
            outside,
            &mut self.bims[who].character,
            &mut self.room,
            &self.maps,
            &taken,
        ));
        true
    }

    /// Off to put the first site with everything there together.
    pub fn build(&mut self, who: usize) -> bool {
        let Some((site, outside, minutes)) = self.build_on_offer(who) else {
            return false;
        };
        if !self.take_over(who, Kind::Build { site, outside }, minutes) {
            return false;
        }
        let taken = self.taken_for(who);
        self.bims[who].task = Some(Task::build(
            who,
            site,
            outside,
            minutes,
            &mut self.bims[who].character,
            &mut self.room,
            &self.maps,
            &taken,
        ));
        true
    }

    /// What the world wants built, this step, and who may go outside to
    /// it. Replaces the last list whole, like the craft orders: a site
    /// that is no longer on it is not begun again, and a chain already at
    /// one finds it gone at its next walk and gives up. The world says,
    /// every step.
    pub fn set_build_orders(&mut self, builds: Vec<Build>, suit_ok: Vec<bool>) {
        self.room.builds = builds;
        self.room.suit_ok = suit_ok;
    }

    /// The loads taken off a shelf since the last call — `(site, resource,
    /// units)` each — for the world to take off the count.
    pub fn take_picked(&mut self) -> Vec<(u32, u32, u32)> {
        core::mem::take(&mut self.room.picked)
    }

    /// The sites a load arrived at since the last call.
    pub fn take_dropped(&mut self) -> Vec<u32> {
        core::mem::take(&mut self.room.dropped)
    }

    /// The sites whose load was given up short of them since the last
    /// call: what was carried is back on the shelf.
    pub fn take_returned(&mut self) -> Vec<u32> {
        core::mem::take(&mut self.room.returned)
    }

    /// The sites put together since the last call, for the world to put the
    /// parts down.
    pub fn take_built(&mut self) -> Vec<u32> {
        core::mem::take(&mut self.room.built)
    }

    /// Whether anybody is on a building errand — carrying to a site or at
    /// one. What holds the ship at rest while it is being built on.
    pub fn building_under_way(&self) -> bool {
        self.bims.iter().any(|b| {
            b.task.as_ref().is_some_and(|t| {
                !t.is_done() && matches!(t.kind(), Kind::Haul { .. } | Kind::Build { .. })
            })
        })
    }

    /// The room laid out again under the crew, for a ship that has changed
    /// shape — a part built. Everything that is state stays: the crew where
    /// they stand, their errands, the dirt, the crops, the doors; see
    /// `Room::relayout`. The grids and the blockers are rebuilt here, and
    /// the outside grid is marked stale so it comes back with the new hull
    /// under it. A walk already planned through where the part now stands
    /// is caught by `unstick`, which replans it round.
    ///
    /// The one errand given up is a craft at a bench that moved: a bench is
    /// named by its index in the layout's list, and a bench built in among
    /// them would have a chain finishing at the wrong one.
    pub fn relayout(&mut self, layout: room::Layout) {
        let benches_before: Vec<Rect> = self.room.benches.iter().map(|b| b.frame).collect();
        self.room.relayout(layout);
        // A fresh grid for sight: whose the tiles are goes back on it.
        self.room.sight.set_stance(self.stance);
        if let Some((rect, stance)) = self.foreign {
            self.room.sight.set_foreign(Some(rect), stance);
        }
        self.refresh_maps();
        self.refresh_blockers();
        self.room.rocks_version = self.room.rocks_version.wrapping_add(1);
        for who in 0..self.bims.len() {
            let moved = self.bims[who]
                .task
                .as_ref()
                .is_some_and(|t| match t.kind() {
                    Kind::Craft { bench, .. } => {
                        benches_before.get(bench) != self.room.benches.get(bench).map(|b| &b.frame)
                    }
                    _ => false,
                });
            if moved && let Some(task) = self.bims[who].task.take() {
                task.abandon(&mut self.bims[who].character, &mut self.room);
            }
        }
    }

    /// Whether anybody is posted at the helm — by the job or by the player,
    /// it makes no difference to the ship. Posted, not standing: a helmsman
    /// off at the heads is still the helmsman and comes back.
    fn helm_manned(&self) -> bool {
        let Some(seat) = self.helm else {
            return false;
        };
        self.bims.iter().enumerate().any(|(who, bim)| {
            self.is_alive(who)
                && bim
                    .character
                    .post()
                    .is_some_and(|post| (post - seat).len() <= HELM_SLACK)
        })
    }

    /// Take the helm: posted at the seat, the way the player's own order
    /// does it. Recorded, so the post can be lifted when the ship no longer
    /// wants it.
    fn man_the_helm(&mut self, who: usize) -> bool {
        let Some(seat) = self.helm else {
            return false;
        };
        if !self.send_to(who, seat) {
            return false;
        }
        self.helmsman = Some(who);
        true
    }

    /// The workstations aboard, in the layout's order — what an `Order`'s
    /// `bench` indexes.
    pub fn benches(&self) -> &[crate::room::Bench] {
        &self.room.benches
    }

    /// What the world wants made, this step. Replaces the last list whole:
    /// an order that is no longer on it — the target met, the ore sold, the
    /// smelter browned out — is simply not begun again, and a chain already
    /// at the bench finishes what it started. The world says, every step.
    pub fn set_craft_orders(&mut self, orders: Vec<Order>) {
        self.orders = orders;
    }

    /// The recipes finished since the last call, in the order they were
    /// finished. The world moves the cargo for each.
    pub fn take_crafted(&mut self) -> Vec<u32> {
        core::mem::take(&mut self.room.crafted)
    }

    /// How many of `recipe` are being made right now — chains running, not
    /// queued. The world counts them against a target, so a target of one
    /// more is one more and not one more per step the first one takes.
    pub fn crafts_under_way(&self, recipe: u32) -> u32 {
        self.bims
            .iter()
            .filter(|b| {
                b.task.as_ref().is_some_and(
                    |t| matches!(t.kind(), Kind::Craft { recipe: r, .. } if r == recipe),
                )
            })
            .count() as u32
    }

    /// What the outside is, and who may go. The world says, every step; a
    /// walk already under way reads the rocks and the marks as they stand
    /// at each rock, and comes in when there is nothing left it may do.
    pub fn set_eva(&mut self, eva: Option<Eva>) {
        match &eva {
            Some(eva) => {
                self.room.rock_targets = eva.targets.clone();
                self.room.rocks = eva.rocks.clone();
                self.room.rocks_version = eva.version;
                self.room.eva_allowed = eva.allowed.clone();
                self.room.tile_minutes = eva.tile_minutes;
            }
            None => {
                self.room.rock_targets.clear();
                self.room.rocks.clear();
                self.room.eva_allowed.clear();
                // A different outside is a different grid, whatever the
                // world's count says.
                self.room.rocks_version = self.room.rocks_version.wrapping_add(1);
            }
        }
        self.eva = eva;
    }

    /// The rocks mined since the last call, by the middle of each in room
    /// units. The world takes each out of its site and moves what it
    /// yields onto the shelf.
    pub fn take_mined(&mut self) -> Vec<(f32, f32)> {
        core::mem::take(&mut self.room.mined)
            .into_iter()
            .map(|p| (p.x, p.y))
            .collect()
    }

    /// How many of the marked rocks a walk from the port could get to, as
    /// of the last look — the rest are marked for nothing until a rock in
    /// front of them is mined. What the host says beside the count of marks.
    pub fn rocks_reachable(&self) -> usize {
        self.rocks_reachable
    }

    /// Everybody in: whoever is outside is brought back through the door
    /// and the walk given up for good, and a walk waiting on the queue is
    /// dropped. For a ship leaving its site — the rocks it was walking to
    /// are not there any more.
    pub fn recall_outside(&mut self) {
        for who in 0..self.bims.len() {
            if self.bims[who].character.is_outside()
                && let Some(task) = self.bims[who].task.take()
            {
                task.abandon(&mut self.bims[who].character, &mut self.room);
            }
            self.bims[who]
                .queue
                .retain(|saved| saved.kind() != Kind::Eva);
        }
    }

    /// Walks outside finished since the last call. The world reads the
    /// belt for each.
    pub fn take_walks(&mut self) -> u32 {
        core::mem::take(&mut self.room.walks_done)
    }

    /// Whether this Bim is outside the hull, in a suit. What the world
    /// doses.
    pub fn is_outside(&self, who: usize) -> bool {
        self.bims.get(who).is_some_and(|b| b.character.is_outside())
    }

    /// Where the helm's seat is while the ship wants somebody at it, in room
    /// units, or `None` when it does not — at a berth, or in a room with no
    /// helm. The world says, every step. Taking it away lifts the post the
    /// job set, and only that one: a Bim the player stood there stays.
    pub fn set_helm(&mut self, seat: Option<Vec2>) {
        self.helm = seat;
        if seat.is_none()
            && let Some(who) = self.helmsman.take()
            && let Some(bim) = self.bims.get_mut(who)
        {
            bim.character.set_post(None);
        }
        // The job's helmsman was ordered elsewhere: the post is gone and so
        // is the helmsman, and the job comes round again for whoever is free.
        if let Some(who) = self.helmsman
            && self
                .bims
                .get(who)
                .is_none_or(|b| b.character.post().is_none())
        {
            self.helmsman = None;
        }
    }

    /// The number a job actually waits on.
    ///
    /// Only cutting is not simply its own row. A crop lifted out of a tray is
    /// in the Bim's hands until it has been carried to the cold store — that
    /// is one chain, not two — so a cutting is a cut *and* a haul, and it
    /// waits on whichever of the two the player has set later. Set hauling to
    /// the bottom and the bay stops being emptied, which is the truthful
    /// answer: there is nobody to carry it.
    fn waits_on(&self, job: Job) -> u32 {
        match job {
            // Either half switched off switches the cutting off: never is
            // the smallest number, and a max would read it as the most
            // urgent thing aboard.
            Job::Cut if self.priorities.never(Job::Cut) || self.priorities.never(Job::Haul) => {
                work::NEVER
            }
            Job::Cut => self
                .priorities
                .of(Job::Cut)
                .max(self.priorities.of(Job::Haul)),
            other => self.priorities.of(other),
        }
    }

    /// Whether this Bim's food need is past its trigger. Asked of `urgent`
    /// rather than of the level so that switching the food trigger off
    /// switches the cooking off with it.
    fn is_hungry(&self, who: usize) -> bool {
        self.bims[who].needs.urgent().contains(&Need::Food)
    }

    /// Whether the cold store holds fewer pots of stew than the manager asked
    /// for, and what a pot takes to make. The demand half of the cook row's
    /// stew; `make_stew` is the other half.
    fn wants_stew(&self) -> bool {
        self.room.stew < self.manager.stew() && self.room.can_make_stew()
    }

    /// The stew half of the cook row, for the probes: whether the shelf is
    /// asking for one. Read directly because the row is also hunger's, and
    /// a Bim that happens to be hungry when the probe looks would otherwise
    /// read as the shelf asking.
    #[allow(dead_code)]
    pub fn wants_stew_for_probe(&self) -> bool {
        self.wants_stew()
    }

    /// Whether going without food has begun to do this Bim damage, as against
    /// merely being hungry. The one thing no priority overrides.
    fn going_without(&self, who: usize) -> bool {
        self.bims[who].health.stage() >= Malnutrition::Mild
    }

    /// Run this Bim's solitude clock and apply whatever came of it.
    ///
    /// All of it is `social.rs`'s arithmetic; what lives here is everything
    /// that needs to know about the world — where the Bim is standing, what it
    /// is in the middle of, and that the diary exists.
    fn bear_the_solitude(&mut self, who: usize, minutes: f32) {
        // A Bim already sitting at the table, in its bunk or on the pan
        // cannot sink to the deck: it is where a chain put it, and putting it
        // somewhere else would drop it through the furniture.
        let can_sit_down = !self.bims[who].character.is_seated();
        let fallout = {
            let (bims, rng) = (&mut self.bims, &mut self.rng);
            bims[who].solitude.update(minutes, can_sit_down, rng)
        };

        if fallout.brooded {
            let days = (self.bims[who].solitude.alone_for() / clock::DAY) as u32;
            self.remember(who, What::FeltLow, days);
        }
        if fallout.broke_down {
            self.bims[who].sad_left = social::SITS_FOR;
            let (at, facing) = (
                self.bims[who].character.pos,
                self.bims[who].character.heading,
            );
            self.bims[who].character.sit(at, facing);
            self.bims[who].character.set_action(Action::None);
            self.remember(who, What::BrokeDown, 0);
        }
        if fallout.hurt_itself {
            self.bims[who].health.hurt(social::SELF_HARM);
            self.remember(who, What::HurtSelf, social::SELF_HARM as u32);
        }
        if fallout.gave_up {
            self.bims[who].health.give_up();
        }
    }

    /// Go and have a word with the other one.
    ///
    /// The only errand aboard that takes two Bims, and the only one started
    /// for somebody else as well as for oneself: both crew are handed a chain
    /// in the same frame, each walking to **its own** spot either side of a
    /// meeting point worked out here.
    ///
    /// That is not a flourish. A route is planned once and never replanned, so
    /// a Bim sent to *where the other one is* would be walking at a target
    /// that is itself walking, converge on empty deck, and stand there with
    /// `arrived()` never coming true. Both walking to fixed points is the only
    /// shape of this that terminates.
    fn chat(&mut self, who: usize) -> bool {
        let Some(other) = self.free_to_talk(who) else {
            return false;
        };
        let (here, there) = (self.bims[who].character.pos, self.bims[other].character.pos);
        let nav = self.maps.pick(self.room.bath.is_open());
        // Side by side, always, rather than either side of the line they
        // happened to approach along. Two Bims who met walking north-south
        // would stand one above the other, and the lower one's name — which
        // the host paints a body's height above its head — lands squarely on
        // the upper one. Left and right, and nothing written over anything.
        // Whoever is further left stays on the left, so they do not cross.
        let middle = (here + there) * 0.5;
        let step = vec2(TALKING_GAP * 0.5, 0.0);
        let (mine, theirs) = if here.x <= there.x {
            (middle - step, middle + step)
        } else {
            (middle + step, middle - step)
        };
        let (mine, theirs) = (nav.nearest_free(mine), nav.nearest_free(theirs));
        // Both have to be able to get there, or one of them stands about while
        // the other talks to the bulkhead.
        if !nav.can_reach(here, mine) || !nav.can_reach(there, theirs) {
            return false;
        }

        if !self.take_over(who, Kind::Chat, 0.0) {
            return false;
        }
        self.interrupt(other);
        // What each of them has to say is picked now, from its own diary, and
        // written down now: by the time the chain ends the topic is gone, and
        // a chat cut short should still be a chat that happened.
        for (bim, at, toward) in [(who, mine, theirs), (other, theirs, mine)] {
            let topic = self.something_to_say(bim);
            self.bims[bim].chat_topic = topic;
            let facing = (toward - at).angle();
            let taken = self.taken_for(bim);
            let (bims, room, maps) = (&mut self.bims, &mut self.room, &self.maps);
            bims[bim].task = Some(Task::chat(
                bim,
                at,
                facing,
                &mut bims[bim].character,
                room,
                maps,
                &taken,
            ));
            self.bims[bim].solitude.talked();
        }
        true
    }

    /// The other one, if it is in a state to be talked to.
    ///
    /// Dead, asleep, out cold, shut in the heads or sitting on the deck in
    /// despair all mean no, and so does being under orders: a recruited Bim
    /// takes no instruction but the player's, and a chat walks it to a
    /// spot. Work does not: a Bim sweeping or at the bay is interrupted,
    /// because the alternative is two Bims who are never both free at the same
    /// moment and therefore never speak — and with the deck always finding
    /// something to be swept, that is not a hypothetical.
    fn free_to_talk(&self, who: usize) -> Option<usize> {
        (0..self.bims.len()).find(|&other| {
            other != who
                && self.bims[other].is_alive()
                && self.bims[other].sad_left <= 0.0
                && !self.bims[other].character.is_napping()
                && !self.bims[other].character.is_unconscious()
                && !self.bims[other].character.is_recruited()
                && !self
                    .room
                    .bath
                    .shell
                    .contains(self.bims[other].character.pos)
                && self.bims[other].task.as_ref().is_none_or(|t| {
                    matches!(t.kind(), Kind::Clean | Kind::Tend { .. } | Kind::Chat)
                })
        })
    }

    /// Something for `who` to talk about, picked at random out of the last few
    /// hours. 0 when it has nothing to report, which the host renders as
    /// talking about nothing much.
    ///
    /// Two sources, and the codes do not collide: the errands it has finished
    /// come back as `JOB_` codes, which run from 1, and the things that
    /// actually happened to it come out of the diary as `memory::What` codes,
    /// which start at 20. The host's `CHAT_TOPICS` is indexed by both.
    ///
    /// The diary alone is not enough any more. It keeps only what went wrong,
    /// so a crew that has had a good week would have stood there with nothing
    /// to say to each other — which is why `Bim::lately` exists.
    fn something_to_say(&mut self, who: usize) -> u32 {
        let mut going = self.bims[who].lately.clone();
        // The last stretch of the diary, not the whole book: a bad night three
        // weeks ago is not this afternoon's conversation.
        let held = self.bims[who].memory.len();
        let recent = held.min(RECENT_ENOUGH);
        for i in (held - recent)..held {
            if let Some(moment) = self.bims[who].memory.at(i) {
                going.push(moment.what.code());
            }
        }
        if going.is_empty() {
            return 0;
        }
        going[self.rng.below(going.len() as u32) as usize]
    }

    /// Start on whichever tray of the bay wants a hand, if any.
    ///
    /// One errand per tray rather than one for the whole bay: an interruption
    /// then costs a tray and not the afternoon, and the Bim walks along the
    /// front of the bay tray by tray the way it would.
    fn tend_bay(&mut self, who: usize) -> bool {
        let Some((bay, job)) = self.bay_work() else {
            return false;
        };
        let kind = Kind::Tend {
            bay,
            spot: job.spot(),
        };
        if !self.can_begin(who, kind) || !self.take_over(who, kind, 0.0) {
            return false;
        }
        let taken = self.taken_for(who);
        self.bims[who].task = Some(Task::tend(
            who,
            bay,
            job.spot(),
            &mut self.bims[who].character,
            &mut self.room,
            &self.maps,
            &taken,
        ));
        true
    }

    /// Get the broom out, if the deck wants it.
    ///
    /// The last thing a Bim considers and the first thing it drops. Sweeping
    /// is what it does when there is genuinely nothing else — no need past its
    /// threshold, no night due, no tray asking — so it sits at the bottom of
    /// `consider_errand`, below the bay and above the wander. Anything that
    /// comes up interrupts it exactly like any other errand, and the half-swept
    /// deck waits on the queue.
    pub fn sweep_up(&mut self, who: usize) -> bool {
        if self.room.filth.dirty_tiles() == 0 {
            return false;
        }
        if !self.can_begin(who, Kind::Clean) || !self.take_over(who, Kind::Clean, 0.0) {
            return false;
        }
        let taken = self.taken_for(who);
        let (bims, room, maps) = (&mut self.bims, &mut self.room, &self.maps);
        bims[who].task = Some(Task::clean(
            who,
            &mut bims[who].character,
            room,
            maps,
            &taken,
        ));
        true
    }

    /// How many tiles are dirty enough to be worth the broom. The host greys
    /// the menu item out with it, and says how much there is to do.
    pub fn dirty_tiles(&self) -> u32 {
        self.room.filth.dirty_tiles()
    }

    // --- the bays and the manager -----------------------------------------

    /// The first tray of any bay that wants a hand, in bay order: which bay
    /// and what it wants. Every bay is asked, so a bay the crew built is
    /// tended like the first.
    fn bay_work(&self) -> Option<(usize, hydro::Job)> {
        let (veg, tofu, fibre) = (self.room.veg, self.room.tofu, self.room.fibre);
        self.room
            .bays
            .iter()
            .enumerate()
            .find_map(|(i, bay)| bay.wants_work(veg, tofu, fibre).map(|job| (i, job)))
    }

    /// How many bays there are aboard. Never nought: see `Room::bays`.
    pub fn hydro_bays(&self) -> usize {
        self.room.bays.len()
    }

    /// Which bay the last click landed on, for the menu that opens after
    /// `hit_at` said `HIT_HYDRO` — the same arrangement as `hit_door`.
    pub fn hit_bay(&self) -> usize {
        self.hit_bay
    }

    /// And the other fixtures a click can land on, the same way.
    pub fn hit_hob(&self) -> usize {
        self.hit_hob
    }

    pub fn hit_fridge(&self) -> usize {
        self.hit_fridge
    }

    pub fn hit_dishwasher(&self) -> usize {
        self.hit_dishwasher
    }

    pub fn hit_locker(&self) -> usize {
        self.hit_locker
    }

    pub fn hit_shower(&self) -> usize {
        self.hit_shower
    }

    pub fn hit_bath(&self) -> usize {
        self.hit_bath
    }

    fn bay(&self, bay: usize) -> &hydro::Bay {
        &self.room.bays[bay.min(self.room.bays.len() - 1)]
    }

    fn bay_mut(&mut self, bay: usize) -> &mut hydro::Bay {
        let last = self.room.bays.len() - 1;
        &mut self.room.bays[bay.min(last)]
    }

    pub fn hydro_spots(&self) -> u32 {
        hydro::SPOTS as u32
    }

    /// What is in tray `i` of bay `bay`: 0 empty, 1 greens, 2 soy, 3 fibre.
    pub fn hydro_crop(&self, bay: usize, i: u32) -> u32 {
        self.bay(bay).crop_at(i as usize)
    }

    /// How far along tray `i` of bay `bay` is, 0 to 1.
    pub fn hydro_growth(&self, bay: usize, i: u32) -> f32 {
        self.bay(bay).growth_at(i as usize)
    }

    /// How many trays of bay `bay` are ready to lift.
    pub fn hydro_ripe(&self, bay: usize) -> u32 {
        self.bay(bay).ripe_count()
    }

    /// How many trays are ready to lift over every bay.
    pub fn hydro_ripe_all(&self) -> u32 {
        self.room.bays.iter().map(|b| b.ripe_count()).sum()
    }

    pub fn hydro_automated(&self, bay: usize) -> bool {
        self.bay(bay).automated()
    }

    /// Follow the manager's target, or stop. A setting rather than an errand:
    /// the Bim does the planting, but deciding *whether* to is the player's,
    /// the same as the timetable or letting the Bim decide for itself.
    pub fn set_hydro_automated(&mut self, bay: usize, on: bool) {
        self.bay_mut(bay).set_automated(on);
    }

    /// The standing order: 0 none, 1 greens everywhere, 2 soy everywhere,
    /// 3 fibre everywhere.
    pub fn hydro_forced(&self, bay: usize) -> u32 {
        self.bay(bay).forced().map_or(0, |c| c.code())
    }

    pub fn set_hydro_forced(&mut self, bay: usize, code: u32) {
        self.bay_mut(bay).force(hydro::Crop::from_code(code));
    }

    pub fn hydro_hibernating(&self, bay: usize) -> bool {
        self.bay(bay).hibernating()
    }

    /// What the place is told to keep, by `manager::Stock`.
    pub fn target(&self, which: Stock) -> u32 {
        self.manager.target(which)
    }

    pub fn set_target(&mut self, which: Stock, count: u32) {
        self.manager.set_target(which, count);
    }

    pub fn target_veg(&self) -> u32 {
        self.manager.veg()
    }

    pub fn target_tofu(&self) -> u32 {
        self.manager.tofu()
    }

    pub fn target_stew(&self) -> u32 {
        self.manager.stew()
    }

    /// Whether a shut — but not locked — door is the only thing between the
    /// Bim and something it is waiting to get on with.
    ///
    /// Asked of whatever is at the front of the queue as well as of the needs,
    /// because queued work whose way is shut waits rather than being thrown
    /// away, and would wait for ever.
    fn shut_in(&self, who: usize) -> bool {
        if self.room.bath.is_open() || self.room.bath.locked {
            return false;
        }
        let queued = self.bims[who]
            .queue
            .first()
            .and_then(|saved| saved.resume_station(&self.room, self.bims[who].character.pos));
        let wanted = self.bims[who]
            .needs
            .urgent()
            .into_iter()
            .filter_map(|need| self.need_station(who, need));
        queued
            .into_iter()
            .chain(wanted)
            .any(|to| !self.can_reach(who, to) && self.can_reach_through_door(who, to))
    }

    /// Where a need would first send the Bim, for the needs it could actually
    /// see to. An empty cold store is not a door problem, so hunger with
    /// nothing to cook asks for no door to be opened — otherwise the Bim
    /// would work the panel over and over for a meal it can never start.
    fn need_station(&self, who: usize, need: Need) -> Option<Vec2> {
        let kind = match need {
            // A pot with something in it is the first place it would go, and
            // it needs nothing out of the store to do it.
            Need::Food if self.room.hobs.iter().any(|h| h.pot_servings > 0) => Kind::Leftovers,
            // A stew on the shelf, either recipe: all three start at the
            // fridge, so which it would pick makes no difference to where it
            // has to be able to walk.
            Need::Food if self.room.stew > 0 || self.room.has_ingredients() => {
                Kind::Meal(Dish::Stew)
            }
            // The heads are behind the door, and that chain opens it itself;
            // rest is the timetable's business and never starts a need errand.
            _ => return None,
        };
        task::first_station(
            who,
            kind,
            &self.room,
            self.bims[who].character.pos,
            &self.taken_for(who),
        )
    }

    /// Whether `to` is somewhere the Bim could walk if the door were open.
    fn can_reach_through_door(&self, who: usize, to: Vec2) -> bool {
        let nav = self.maps.pick(true);
        nav.can_reach(self.bims[who].character.pos, to)
    }

    // --- company, and going without it --------------------------------------

    /// 0 for a Bim with company, then 1, 2, 3 for the three stages of going
    /// without it. The host names them.
    pub fn loneliness(&self, who: usize) -> u32 {
        self.bims[who].solitude.stage().stage()
    }

    /// Whole days since it last spoke to anybody. What the stages are built
    /// on, so the readout and the behaviour cannot drift apart.
    pub fn days_alone(&self, who: usize) -> f32 {
        self.bims[who].solitude.alone_for() / clock::DAY
    }

    /// What `who` is saying this instant, as a `memory::What` code, or 0 for
    /// nothing at all.
    ///
    /// **Only one of them talks at a time.** Both hold a topic for the length
    /// of the conversation, and whose turn it is comes off the ship's clock
    /// rather than off either chain's own elapsed time — they arrive a moment
    /// apart, and two bubbles over two Bims a body's width apart would sit on
    /// top of each other.
    pub fn chat_topic(&self, who: usize) -> u32 {
        let talking = self.bims[who]
            .task
            .as_ref()
            .is_some_and(|t| t.step() == task::Step::Talk);
        if !talking {
            return 0;
        }
        let turn = (self.clock.minutes() / TAKES_A_TURN) as u32 as usize;
        if turn % self.bims.len().max(1) == who {
            self.bims[who].chat_topic
        } else {
            0
        }
    }

    /// Wind a Bim's solitude clock forward by hand, for the probes: watching
    /// one reach the far end of this in real time is ten game days of frames,
    /// and what is worth checking is what happens once it is there.
    #[allow(dead_code)]
    pub fn leave_alone_for_probe(&mut self, who: usize, days: f32) {
        self.bims[who]
            .solitude
            .set_alone_for(days * crate::clock::DAY);
    }

    /// Whether `who` is sitting on the deck having given up for a while.
    #[allow(dead_code)]
    pub fn broken_down_for_probe(&self, who: usize) -> bool {
        self.bims[who].sad_left > 0.0
    }

    // --- the order the work gets done in ------------------------------------
    //
    // One list for the ship, so none of these takes a `who`. The host holds
    // the names; everything that crosses here is a code and a number.

    pub fn work_count(&self) -> u32 {
        Job::ALL.len() as u32
    }

    /// The range the boxes cycle through. Read rather than repeated, so the
    /// host's colour scale cannot come to disagree with what a click does.
    pub fn work_highest(&self) -> u32 {
        work::HIGHEST
    }

    pub fn work_lowest(&self) -> u32 {
        work::LOWEST
    }

    pub fn work_priority(&self, job: u32) -> u32 {
        Job::from_code(job).map_or(work::DEFAULT, |job| self.priorities.of(job))
    }

    /// Set one outright. Nothing in the host does — a box is clicked, not
    /// typed into — so this is the probes' way in.
    #[allow(dead_code)]
    pub fn set_work_priority(&mut self, job: u32, level: u32) {
        if let Some(job) = Job::from_code(job) {
            self.priorities.set(job, level);
        }
    }

    /// The jobs on offer to `who` right now, most important first, as codes.
    /// For the probes: what the list actually decides, without the galley
    /// being busy or the broom being out confusing the reading.
    #[allow(dead_code)]
    pub fn work_on_offer_for_probe(&self, who: usize) -> Vec<u32> {
        self.work_on_offer(who).iter().map(|j| j.code()).collect()
    }

    /// What a job actually waits on once the chains it is part of are taken
    /// into account, for the probes. Only cutting differs from its own row —
    /// see [`Game::waits_on`] — and staging a ripe tray to watch it happen
    /// takes most of a day, so the rule is checked here directly.
    #[allow(dead_code)]
    pub fn work_waits_on_for_probe(&self, job: u32) -> u32 {
        Job::from_code(job).map_or(work::DEFAULT, |job| self.waits_on(job))
    }

    /// One click on a box: a step less important, and round to the top from
    /// the bottom. The cycling is the simulation's so the range has one home.
    /// Gives back what it now reads, so the host paints what actually landed
    /// rather than what it expected.
    pub fn cycle_work_priority(&mut self, job: u32) -> u32 {
        match Job::from_code(job) {
            Some(job) => self.priorities.cycle(job),
            None => work::DEFAULT,
        }
    }

    /// The other way round — a right click: a step less important, and
    /// off the bottom to never.
    pub fn cycle_work_priority_back(&mut self, job: u32) -> u32 {
        match Job::from_code(job) {
            Some(job) => self.priorities.cycle_back(job),
            None => work::DEFAULT,
        }
    }

    /// The number that means "never": below the top of the range, and a
    /// job set to it is not done at all.
    pub fn work_never(&self) -> u32 {
        work::NEVER
    }

    // --- what the Bim wants ----------------------------------------------

    /// How full need `i` is, 0 to 1, in the order `Need::ALL` lists them.
    pub fn need_level(&self, who: usize, i: u32) -> f32 {
        Need::from_index(i).map_or(0.0, |need| self.bims[who].needs.level(need))
    }

    pub fn need_count(&self) -> u32 {
        Need::ALL.len() as u32
    }

    /// The level at which need `i` sends the Bim to see to it, and whether it
    /// does so at all. The host reads both rather than repeating them, so the
    /// bar cannot disagree with the behaviour it is meant to be predicting.
    /// The action thresholds are the ship's, not a Bim's: one setting, applied
    /// to the whole crew, the same way the timetable is. So they are written
    /// to every Bim and read back off whichever one — they cannot differ.
    pub fn need_trigger(&self, i: u32) -> f32 {
        Need::from_index(i).map_or(0.0, |need| self.bims[PLAYER].needs.trigger(need).at)
    }

    pub fn need_trigger_on(&self, i: u32) -> bool {
        Need::from_index(i).is_some_and(|need| self.bims[PLAYER].needs.trigger(need).on)
    }

    pub fn set_need_trigger(&mut self, i: u32, at: f32) {
        let Some(need) = Need::from_index(i) else {
            return;
        };
        for bim in &mut self.bims {
            bim.needs.set_trigger_at(need, at);
        }
    }

    pub fn set_need_trigger_on(&mut self, i: u32, on: bool) {
        let Some(need) = Need::from_index(i) else {
            return;
        };
        for bim in &mut self.bims {
            bim.needs.set_trigger_on(need, on);
        }
    }

    /// How badly it needs the heads, 0 to 3. The host names them.
    pub fn urge(&self, who: usize) -> u32 {
        self.bims[who].needs.urge().stage()
    }

    /// How far gone it is for want of somewhere clean, 0 to 3.
    ///
    /// Per Bim, not per deck. The mess is shared; how long *this* one has been
    /// standing in it is not, and a Bim that has just walked in from the heads
    /// is not as far gone as the one that has been beside it for two hours.
    pub fn discomfort(&self, who: usize) -> u32 {
        self.bims[who].ordeal.discomfort().stage()
    }

    pub fn bim_filth(&self, who: usize) -> f32 {
        self.bims[who].character.filth()
    }

    /// What the tile under a point scores, [`filth::FOULED`] to
    /// [`filth::BASELINE`]. For the probes: the host has `deck_filth` for the
    /// one number it needs.
    #[allow(dead_code)]
    pub fn tile_filth(&self, at: Vec2) -> f32 {
        self.room.filth.at(at)
    }

    /// How much of the deck has something on it, 0 to 1 — one number for a
    /// state that is really two hundred, so the host can say "the place is a
    /// tip" without reading every tile.
    pub fn deck_filth(&self) -> f32 {
        self.room.filth.dirty_share()
    }

    // --- naming what the pointer is over ----------------------------------
    //
    // Three numbers rather than a string, the same as everything else across
    // the boundary: what the thing is, what is on it, and how much. The host
    // does the wording.

    /// What is at a point, in the `SPOT_` codes from `room.rs`.
    pub fn spot_at(&self, x: f32, y: f32) -> u32 {
        self.room.spot(vec2(x, y))
    }

    /// What kind of mess is on the tile under a point: 0 none, then grime,
    /// wetting, soiling, sick. The deck grid runs under the furniture as well
    /// as the open floor, so the host only asks this where it makes sense to.
    pub fn spot_mess(&self, x: f32, y: f32) -> u32 {
        self.room.filth.kind_at(vec2(x, y)).code()
    }

    /// How far down that tile has been taken, 0 clean to 1 fouled.
    pub fn spot_mess_depth(&self, x: f32, y: f32) -> f32 {
        self.room.filth.depth_at(vec2(x, y))
    }

    /// Ring a fixture on the deck, by its `SPOT_` code, or `SPOT_NOTHING` to
    /// ring nothing. A code with no single place behind it — deck, bulkhead —
    /// rings nothing too, rather than the whole room.
    pub fn set_highlight(&mut self, spot: u32) {
        self.highlight = spot;
    }

    pub fn set_autonomous(&mut self, on: bool) {
        self.autonomous = on;
    }

    // --- under direct orders ----------------------------------------------

    /// Recruit the Bim, or let it go again.
    ///
    /// Recruited, it does nothing of its own accord: no errand from a need, no
    /// sleep from the timetable, nothing picked back up off the queue, and no
    /// pottering about between jobs. What it will still do is everything the
    /// player asks of it — and everything going without does to it, since the
    /// needs carry on draining and every stage of hunger and drowsiness bites
    /// exactly as before.
    ///
    /// Whatever it is in the middle of is left to finish. Cancelling would
    /// throw away a half-cooked meal for the sake of tidiness, and a right
    /// click interrupts it anyway.
    pub fn toggle_recruited(&mut self) {
        let now = !self.bims[PLAYER].character.is_recruited();
        self.bims[PLAYER].character.set_recruited(now);
        if now {
            // It is the thing being ordered about, so it is the thing selected.
            self.bims[PLAYER].character.selected = true;
        }
    }

    pub fn is_recruited(&self) -> bool {
        self.bims[PLAYER].character.is_recruited()
    }

    pub fn is_autonomous(&self) -> bool {
        self.autonomous
    }

    // --- player input ---------------------------------------------------

    /// Select by control group. There is one Bim, so group 1 is all of them.
    pub fn select_group(&mut self, group: u32) {
        if group == 1 {
            self.select_only(Some(PLAYER));
        }
    }

    pub fn clear_selection(&mut self) {
        self.select_only(None);
    }

    /// Exactly one of the crew is selected, or none.
    ///
    /// Selecting is *looking at*, not taking charge of: any of them can be
    /// picked, and picking one is what puts its panels on screen. Whether it
    /// takes orders is a separate question, and the answer is always the same
    /// one — see `order_move`, which asks after [`PLAYER`] and nobody else.
    ///
    /// One at a time because the panels show one at a time. A marquee over
    /// both has to mean something, and "whichever you can actually order
    /// about" is the least surprising thing it can mean.
    fn select_only(&mut self, who: Option<usize>) {
        for (i, bim) in self.bims.iter_mut().enumerate() {
            bim.character.selected = who == Some(i);
        }
    }

    /// Which of them is selected, or `None`.
    pub fn selected(&self) -> Option<usize> {
        self.bims.iter().position(|b| b.character.selected)
    }

    pub fn is_selected(&self, who: usize) -> bool {
        self.bims[who].character.selected
    }

    pub fn bim_pos(&self, who: usize) -> Vec2 {
        self.bims[who].character.pos
    }

    /// How many are aboard. Two in the classic room; a ship's crew, up to
    /// [`room::BERTHS`], aboard one.
    pub fn crew_count(&self) -> u32 {
        self.bims.len() as u32
    }

    pub fn selected_count(&self) -> u32 {
        self.selected().is_some() as u32
    }

    pub fn drag_begin(&mut self, x: f32, y: f32) {
        let p = vec2(x, y);
        self.drag = Some((p, p));
    }

    pub fn drag_update(&mut self, x: f32, y: f32) {
        if let Some((_, end)) = &mut self.drag {
            *end = vec2(x, y);
        }
    }

    /// Finish a marquee. A plain click is just a box of zero size, so the same
    /// hit test covers picking and dragging; missing entirely deselects.
    ///
    /// Returns the fixture under a click, if any, so the host can open a menu
    /// on it. Clicking a fixture leaves the selection alone — opening the
    /// fridge should not deselect the Bim you were about to give a job to.
    pub fn drag_end(&mut self, x: f32, y: f32) -> u32 {
        self.drag_update(x, y);
        let Some((start, end)) = self.drag.take() else {
            return HIT_NONE;
        };
        let box_ = Rect::from_corners(start, end);

        // Only a click, not a sweep, counts as poking at the furniture.
        if box_.width() < 4.0 && box_.height() < 4.0 {
            self.note_fixtures(box_.center());
            let hit = self.room.hit(box_.center());
            if hit != HIT_NONE {
                return hit;
            }
        }

        // Whoever the box touched. The player's own comes first when it
        // caught both: a sweep across the compartment most likely meant the
        // one that can be told to do something.
        let touched: Vec<usize> = (0..self.bims.len())
            .filter(|&i| {
                box_.touches_circle(
                    self.bims[i].character.pos,
                    self.bims[i].character.pick_radius(),
                )
            })
            .collect();
        let pick = touched
            .iter()
            .copied()
            .find(|&i| i == PLAYER)
            .or_else(|| touched.first().copied());
        self.select_only(pick);
        HIT_NONE
    }

    pub fn drag_cancel(&mut self) {
        self.drag = None;
    }

    /// Right-click on the floor: send whatever is selected to that spot. A task
    /// in progress outranks the player, so orders are ignored while cooking.
    /// Send the selection to a spot, opening a door on the way if that is what
    /// it takes. Says what became of the order; see the `ORDER_` codes.
    ///
    /// A shut door is a wall to the pathfinder, so the route is worked out
    /// twice: once with the door as it stands, and — if that comes back with
    /// nothing — once with it open. A destination only the second finds is one
    /// the Bim can reach by letting itself through, which it goes and does.
    pub fn order_move(&mut self, x: f32, y: f32) -> u32 {
        // The player's own always; a crewmate only while the alarm is up —
        // in combat mode the crew take orders, and only then.
        let Some(who) = self.selected() else {
            return ORDER_IGNORED;
        };
        if who != PLAYER && !self.alarm {
            return ORDER_IGNORED;
        }
        let code = self.order_move_for(who, x, y);
        // A crewmate ordered somewhere holds that spot rather than falling
        // back into the gathering round the player, until the alarm is over.
        if who != PLAYER && code != ORDER_IGNORED && code != ORDER_NOWHERE && code != ORDER_LOCKED {
            let nav = self.maps.pick(true);
            let spot = nav.nearest_free(vec2(x, y));
            self.bims[who].character.set_post(Some(spot));
        }
        code
    }

    /// [`Game::order_move`] for one Bim, the checks on who is done.
    fn order_move_for(&mut self, who: usize, x: f32, y: f32) -> u32 {
        if !self.bims[who].character.selected
            || !self.is_alive(who)
            // Out there it is on a walk, and the walk brings it in.
            || self.bims[who].character.is_outside()
        {
            return ORDER_IGNORED;
        }
        let want = vec2(x, y);
        // A fresh order is the end of standing anywhere in particular.
        self.bims[who].character.set_post(None);

        // Snap to somewhere the Bim can actually stand, so an order onto the
        // table means "the floor beside the table" rather than nothing at all.
        let nav = self.maps.pick(self.room.bath.is_open());
        let target = nav.nearest_free(want);
        let route = nav.path(self.bims[who].character.pos, target);
        if !route.is_empty() {
            // The order outranks whatever the Bim is on. That chain goes into
            // the queue and is picked up once it has been where it was sent.
            self.interrupt_for_order(who);
            self.bims[who].character.follow_path(route);
            self.mark(target, false);
            return ORDER_MOVING;
        }

        // Nothing as things stand. Would there be with the door open?
        let open = self.maps.pick(true);
        let through = open.nearest_free(want);
        if open.path(self.bims[who].character.pos, through).is_empty() {
            self.mark(through, true);
            return ORDER_NOWHERE;
        }

        // A Bim part-way through a trip to the heads has locked the door
        // behind itself, and letting go of that errand unlocks it again. So
        // that is not being locked out: it is the Bim's own door, and the
        // order is what opens it. Any other locked door genuinely blocks, and
        // nothing is attempted — not even dropping what it was doing.
        let own_lock = self.bims[who]
            .task
            .as_ref()
            .is_some_and(|task| task.kind() == Kind::Heads);
        if self.room.bath.locked && !own_lock {
            self.mark(through, true);
            return ORDER_LOCKED;
        }

        // Already on its way to the panel: the new order simply replaces the
        // one that was waiting on the door, and the Bim carries on to it.
        // Interrupting here would put that errand on the queue and start a
        // second one exactly like it — click a few times and the agenda fills
        // with door errands that all open the same door.
        if self.on_a_door_errand(who) {
            self.bims[who].pending_move = Some(want);
            self.mark(through, false);
            return ORDER_VIA_DOOR;
        }

        self.interrupt_for_order(who);

        // Letting go may have opened the door by itself, so look again before
        // sending the Bim off to work a panel it no longer needs to touch.
        let nav = self.maps.pick(self.room.bath.is_open());
        let target = nav.nearest_free(want);
        let route = nav.path(self.bims[who].character.pos, target);
        if !route.is_empty() {
            self.bims[who].character.follow_path(route);
            self.mark(target, false);
            return ORDER_MOVING;
        }

        // Go and open it, then carry on to where it was sent.
        self.bims[who].pending_move = Some(want);
        let taken = self.taken_for(who);
        self.bims[who].task = Some(Task::work_switch(
            who,
            Switch::BathDoor(true),
            &mut self.bims[who].character,
            &mut self.room,
            &self.maps,
            &taken,
        ));
        self.mark(through, false);
        ORDER_VIA_DOOR
    }

    fn mark(&mut self, pos: Vec2, bad: bool) {
        self.markers.push(Marker { pos, age: 0.0, bad });
    }

    /// Once the door is open, take up the order that was waiting on it. This
    /// goes before the queue: the player asked for it now, not after whatever
    /// the door errand displaced.
    fn take_pending_move(&mut self, who: usize) {
        if self.bims[who].task.is_some() || !self.bims[who].character.arrived() {
            return;
        }
        let Some(want) = self.bims[who].pending_move.take() else {
            return;
        };
        let nav = self.maps.pick(self.room.bath.is_open());
        let target = nav.nearest_free(want);
        let route = nav.path(self.bims[who].character.pos, target);
        if route.is_empty() {
            // The door shut again, or was locked while the Bim walked to it.
            self.mark(target, true);
            return;
        }
        self.bims[who].character.follow_path(route);
        self.mark(target, false);
    }

    // --- the clock --------------------------------------------------------

    /// Minutes since midnight. The host formats the reading; no strings cross
    /// the boundary.
    /// Wind the clock on by `minutes` of game time without simulating any of
    /// it. For a room opened partway through a day — a station's residents
    /// when the ship arrives — so its day is the world's day rather than
    /// starting at the waking hour whenever the ship happens to turn up.
    pub fn wind_clock(&mut self, minutes: f32) {
        self.clock.advance(clock::seconds(minutes));
    }

    pub fn clock_minutes(&self) -> f32 {
        self.clock.minutes()
    }

    pub fn clock_day(&self) -> u32 {
        self.clock.day()
    }

    // --- who they are -------------------------------------------------------
    //
    // The ship's calendar, and a birthday apiece. The host formats both; all
    // that crosses is numbers, so "12 April 2367" is assembled from a year, a
    // month and a date on the other side.

    pub fn clock_year(&self) -> u32 {
        self.clock.year()
    }

    pub fn clock_month(&self) -> u32 {
        crate::clock::month_and_date(self.clock.day_of_year()).0
    }

    pub fn clock_date(&self) -> u32 {
        crate::clock::month_and_date(self.clock.day_of_year()).1
    }

    pub fn born_year(&self, who: usize) -> u32 {
        self.bims[who].born_year
    }

    pub fn born_month(&self, who: usize) -> u32 {
        crate::clock::month_and_date(self.bims[who].born_day).0
    }

    pub fn born_date(&self, who: usize) -> u32 {
        crate::clock::month_and_date(self.bims[who].born_day).1
    }

    /// How old it is today, in whole years.
    pub fn age(&self, who: usize) -> u32 {
        self.bims[who].age(self.clock.year(), self.clock.day_of_year())
    }

    // --- what they remember -------------------------------------------------

    pub fn memory_len(&self, who: usize) -> u32 {
        self.bims[who].memory.len() as u32
    }

    /// One line of the diary, oldest first. Four numbers and no words: what
    /// day, what time, what happened, and the one detail that goes with it.
    pub fn memory_day(&self, who: usize, i: u32) -> u32 {
        self.bims[who]
            .memory
            .at(i as usize)
            .map_or(0, |moment| moment.day)
    }

    pub fn memory_at(&self, who: usize, i: u32) -> f32 {
        self.bims[who]
            .memory
            .at(i as usize)
            .map_or(0.0, |moment| moment.at)
    }

    pub fn memory_what(&self, who: usize, i: u32) -> u32 {
        self.bims[who]
            .memory
            .at(i as usize)
            .map_or(0, |moment| moment.what.code())
    }

    pub fn memory_detail(&self, who: usize, i: u32) -> u32 {
        self.bims[who]
            .memory
            .at(i as usize)
            .map_or(0, |moment| moment.detail)
    }

    /// Game minutes of sleep the Bim still has ahead of it, or zero.
    pub fn rest_left(&self, who: usize) -> f32 {
        match &self.bims[who].task {
            None => 0.0,
            Some(t) => t.rest_left(),
        }
    }

    // --- fixtures ---------------------------------------------------------

    /// Which fixture is at a point, without disturbing the selection. The
    /// host asks this on a right-click, so poking at the furniture and
    /// ordering the Bim about can share one button.
    pub fn hit_at(&mut self, x: f32, y: f32) -> u32 {
        let p = vec2(x, y);
        self.note_fixtures(p);
        // A body before the deck: a click on one of the crew is the crew
        // member, whatever it is standing on — living, `HIT_BIM` (out cold
        // too: the menu offers the bandages and the looting side by side);
        // dead, `HIT_BODY`, and all there is to do for them is loot them.
        for (i, bim) in self.bims.iter().enumerate() {
            if (bim.character.pos - p).len() <= bim.character.pick_radius() {
                if bim.is_alive() {
                    self.hit_bim = i;
                    return HIT_BIM;
                }
                self.hit_body = i;
                return HIT_BODY;
            }
        }
        // Then a visitor that the world marked down — a resident lying in
        // its own room, drawn on this deck — which is a body to loot too.
        // The crew first: a crewmate standing over a body is the crewmate.
        for (i, at) in self.visitors.iter().enumerate() {
            let down = self.visitors_down.get(i).copied().unwrap_or(false);
            let hailable = self.visitors_hailable.get(i).copied().unwrap_or(false);
            if (down || hailable) && (*at - p).len() <= PICK_RADIUS {
                self.hit_visitor = i;
                return HIT_VISITOR;
            }
        }
        self.room.hit(vec2(x, y))
    }

    /// The crew member the last click landed on, after [`Game::hit_at`]
    /// said `HIT_BIM`.
    pub fn hit_bim(&self) -> usize {
        self.hit_bim
    }

    /// The dead crew member the last click landed on, after `HIT_BODY`.
    pub fn hit_body(&self) -> usize {
        self.hit_body
    }

    /// The visitor — an index into what [`Game::set_visitors`] was handed,
    /// the world's residents in order — the last click landed on, after
    /// `HIT_VISITOR`.
    pub fn hit_visitor(&self) -> usize {
        self.hit_visitor
    }

    /// The workstation the last click landed on, after [`Game::hit_at`] or
    /// [`Game::drag_end`] said `HIT_BENCH`; [`Game::bench_part`] says what
    /// kind it is.
    pub fn hit_bench(&self) -> usize {
        self.hit_bench
    }

    /// The shelf the last click landed on, after `HIT_SHELF`.
    pub fn hit_shelf(&self) -> usize {
        self.hit_shelf
    }

    /// The trading desk the last click landed on, after `HIT_DESK`.
    pub fn hit_desk(&self) -> usize {
        self.hit_desk
    }

    /// Where the Bim stands to use that trading desk, for `send_to`;
    /// `None` for no such desk.
    pub fn desk_spot(&self, desk: usize) -> Option<Vec2> {
        self.room.desks.get(desk).map(|d| d.1)
    }

    /// Every trading desk on the deck: its footprint and its stand spot.
    /// What the world asks to know whether a crew member is at one.
    pub fn desks(&self) -> &[(Rect, Vec2)] {
        &self.room.desks
    }

    /// What kind of part a workstation is — its `PartKind` code, the way
    /// the world handed it over in `Bench::kind` — so the app knows whether
    /// the bench under a click is the armoury. Nought for no such bench.
    pub fn bench_part(&self, i: usize) -> u32 {
        self.room.benches.get(i).map_or(0, |b| b.kind)
    }

    /// Remember which of every kind of fixture a click landed on, if one,
    /// so the host can ask `hit_door`, `hit_fridge`, `hit_bench` and the
    /// rest after the code says which kind it was. Both a right-click
    /// (`hit_at`) and a left one (`drag_end`) note them, so the menu or
    /// the grid that opens is the fixture under *that* click.
    fn note_fixtures(&mut self, p: Vec2) {
        let room = &self.room;
        for (hit, at) in [
            (&mut self.hit_door, room.door_at(p)),
            (&mut self.hit_bay, room.bay_at(p)),
            (&mut self.hit_hob, room.hob_at(p)),
            (&mut self.hit_fridge, room.fridge_at(p)),
            (&mut self.hit_dishwasher, room.dishwasher_at(p)),
            (&mut self.hit_locker, room.locker_at(p)),
            (&mut self.hit_shower, room.shower_at(p)),
            (&mut self.hit_bath, room.heads_at(p)),
            (&mut self.hit_bench, room.bench_at(p)),
            (&mut self.hit_shelf, room.shelf_at(p)),
            (&mut self.hit_desk, room.desk_at(p)),
        ] {
            if let Some(i) = at {
                *hit = i;
            }
        }
    }

    /// The ship's door the last click landed on.
    pub fn hit_door(&self) -> usize {
        self.hit_door
    }

    pub fn ship_door_count(&self) -> usize {
        self.room.doors.len()
    }

    /// The opening of one of the ship's doors, in room units. For the probes.
    #[allow(dead_code)]
    pub fn ship_door_opening_for_probe(&self, i: usize) -> Rect {
        self.room.doors[i].rect
    }

    /// Whether this room draws its doors. Off for a station's room kept
    /// open only for its pictures while the ship is docked: the joined room
    /// has the same doors and the people going through them.
    pub fn set_doors_drawn(&mut self, drawn: bool) {
        self.room.doors_drawn = drawn;
    }

    // --- sight ------------------------------------------------------------------

    /// Whose eyes this room is drawn through. See `crate::sight::Fog`.
    pub fn set_fog(&mut self, fog: Fog) {
        self.fog = fog;
    }

    /// Under `Fog::None`: which of this room's bodies the crew of the room
    /// over it can see, one flag a Bim. Set by the world once a step. A
    /// body seen stays drawn for [`SEEN_FOR`] after it was last in view.
    pub fn set_seen(&mut self, seen: &[bool]) {
        self.seen_for.resize(seen.len(), 0.0);
        for (left, &now) in self.seen_for.iter_mut().zip(seen) {
            if now {
                *left = SEEN_FOR;
            }
        }
    }

    /// Whether a Bim of this room is drawn: every one of the crew's own,
    /// none of a room nobody is looking into, and under a joined deck the
    /// ones the world said are in view — or were, a moment ago.
    pub fn body_seen(&self, who: usize) -> bool {
        match self.fog {
            Fog::Crew => true,
            Fog::All => false,
            Fog::None => self.seen_for.get(who).is_some_and(|&left| left > 0.0),
        }
    }

    // --- the fight --------------------------------------------------------------

    /// Whose the room's tiles are: what the fog over them looks like. A
    /// station's room is whatever the station is to the crew.
    pub fn set_stance(&mut self, stance: Stance) {
        self.stance = stance;
        self.room.sight.set_stance(stance);
    }

    /// Which of the room's tiles are somebody else's — a station's, on a
    /// joined deck, by its box in room units — and whose. Kept across a
    /// relayout.
    pub fn set_foreign(&mut self, rect: Option<Rect>, stance: Stance) {
        self.foreign = rect.map(|r| (r, stance));
        self.room.sight.set_foreign(rect, stance);
    }

    /// Whether every body in this room is an enemy to whoever is looking:
    /// a hostile station's people. The ring under each — and the other
    /// side of the fight: its people shoot at the targets as enemies do,
    /// recording shots for the world rather than flying bolts here, and go
    /// to war the moment a target is named. See `tick_combat`.
    pub fn set_hostile_bodies(&mut self, hostile: bool) {
        self.hostile_bodies = hostile;
        for bim in &mut self.bims {
            bim.character.set_hostile(hostile);
        }
    }

    /// Where the enemies stand, in room units, and what each carries,
    /// index for index with whoever the world says they are — `None` for
    /// one that is down. The world sets it every step while there are
    /// any, and clears it after. What a Bim in combat mode shoots at; in
    /// a room whose bodies are hostile, the crew, and what its people go
    /// to war over. The weapon says which of them lock a gunner in a
    /// melee (`crate::combat`). Where one is peeking, this is the peek
    /// position (`Game::exposed_at`) and [`Game::set_hostiles_peeking`]
    /// says so, after.
    pub fn set_hostiles(&mut self, at: Vec<Option<(Vec2, WeaponKind)>>) {
        self.combat.set_targets(at);
    }

    /// Which of the enemies named by [`Game::set_hostiles`] are peeking
    /// from cover, index for index: a bolt reaching one is dodged half
    /// the time. Nobody is until this says so, every step.
    pub fn set_hostiles_peeking(&mut self, peeking: &[bool]) {
        self.combat.set_peeking(peeking);
    }

    /// The enemy a Bim is locked in a melee with — an index into the
    /// hostiles — or `None` free to fire.
    pub fn is_locked(&self, who: usize) -> Option<usize> {
        self.bims[who].locked
    }

    /// What is in a Bim's hand.
    pub fn weapon(&self, who: usize) -> Option<WeaponKind> {
        self.bims[who].gear.weapon
    }

    /// Where a Bim leans out to while it aims from a peek beside a wall,
    /// or `None` standing square.
    pub fn peek(&self, who: usize) -> Option<Vec2> {
        self.bims[who].peek
    }

    /// Where a shot at a Bim is aimed: the peek it leans out to while it
    /// peeks, else where it stands. What the world hands the other room
    /// as the target's position.
    pub fn exposed_at(&self, who: usize) -> Vec2 {
        self.bims[who].peek.unwrap_or(self.bims[who].character.pos)
    }

    /// What a Bim carries, issued: the world's, for a station's people
    /// (`Gear::issued_for`). The picture follows.
    pub fn issue(&mut self, who: usize, gear: Gear) {
        if who < self.bims.len() {
            self.bims[who].gear = gear;
            self.refresh_worn(who);
        }
    }

    /// Every hit that landed on one of the hostiles since last asked, for
    /// the world to carry to the body — which of them, where on it, and
    /// how hard.
    pub fn take_hits(&mut self) -> Vec<Hit> {
        self.combat.take_hits()
    }

    /// The shots this room's people took while its bodies were hostile,
    /// since last asked, for the world to fire in the crew's room through
    /// [`Game::enemy_fire`].
    pub fn take_shots(&mut self) -> Vec<Shot> {
        self.combat.take_shots()
    }

    /// An enemy's shot, fired here as a hostile bolt: red, and looking
    /// for this room's own bodies. `from` and `at` in this room's units.
    pub fn enemy_fire(&mut self, from: Vec2, at: Vec2, weapon: WeaponKind) {
        self.combat.fire(from, at, weapon, true);
    }

    /// An enemy's blow, delivered: a melee `Shot` the world carried
    /// across, landing on one of this room's own — if the enemy at `from`
    /// (in this room's units) is still within reach of the body, since
    /// the lock was read a step ago in the other room and a body walks.
    /// Half a tile of slack over [`MELEE_RANGE`] for the step between.
    /// The part is rolled here, the wound applied now like a bolt's, and
    /// the hit kept for the world to say (`take_wounds_taken`). Whether it
    /// landed.
    pub fn enemy_strike(&mut self, from: Vec2, who: usize, damage: f32, cut: bool) -> bool {
        if !self
            .bims
            .get(who)
            .is_some_and(|b| b.is_alive() && !b.character.is_outside())
        {
            return false;
        }
        if (self.exposed_at(who) - from).len() > (MELEE_RANGE + 0.5) * TILE {
            return false;
        }
        let hit = self.combat.struck(who, damage, cut);
        self.strike(who, hit.part, damage, cut);
        self.wounds_taken.push(hit);
        self.attacked_for = ALARM_HOLD;
        true
    }

    /// Every hostile bolt that landed on one of this room's own since
    /// last asked. Already applied — `tick_combat` wounds the body the
    /// step the bolt lands — so this is for the world to say what
    /// happened, not to do anything about it.
    pub fn take_wounds_taken(&mut self) -> Vec<Hit> {
        std::mem::take(&mut self.wounds_taken)
    }

    /// Every wound closed and every part back to its full, blood and all —
    /// a living body as good as new. For a probe that wants a fight to
    /// run for as long as it takes to see the other side's shot land or
    /// go down, and not to end on the one-in-twenty head shot that would
    /// otherwise decide it: patch this one up before every step and the
    /// run is about the other body. Does nothing for a body already dead.
    #[allow(dead_code)]
    pub fn patch_up_for_probe(&mut self, who: usize) {
        if self.bims[who].is_alive() {
            self.bims[who].health = Health::new();
        }
    }

    /// A shot landed on one of this room's Bims: [`Game::strike`], not a
    /// cut.
    pub fn wound(&mut self, who: usize, part: Part, damage: f32) -> WoundOutcome {
        self.strike(who, part, damage, false)
    }

    /// A shot or a blow landed on one of this room's Bims. The armour on
    /// that part takes it first, if there is any and it is not broken:
    /// the piece's protection comes off the damage — nothing left is
    /// nothing, no wound — and what remains drains the piece's health;
    /// only what the piece could not take reaches the body
    /// (`Health::shot`: the damage off that part, a wound opened there —
    /// three units for a `cut`, which bleeds three times as fast). A piece
    /// at nothing is broken and does nothing from then on. A flash on the
    /// body either way. What kills it is the ordinary check at the top of
    /// its next tick. The blood: a wound that opens drips at once
    /// (`Bim::tick_drips`), and a cut throws it over the tiles round the
    /// body besides (`Filth::splash_blood`).
    ///
    /// The outcome says how it went — what the armour took, protection
    /// included, what got through, whether the piece broke and whether a
    /// leg went — for the caller's information; a piece breaking is also
    /// kept on [`Game::take_pieces_broken`] for the world, since the room
    /// applies an enemy's shots itself.
    pub fn strike(&mut self, who: usize, part: Part, damage: f32, cut: bool) -> WoundOutcome {
        let mut out = WoundOutcome::default();
        if !self.bims.get(who).is_some_and(|b| b.is_alive()) {
            return out;
        }
        let bim = &mut self.bims[who];
        bim.hit_flash = HIT_FLASH;
        let mut through = damage;
        let mut broke = None;
        if let Some(piece) = bim.gear.worn_mut(part).as_mut().filter(|p| !p.broken()) {
            through -= piece.effective_protection();
            if through <= 0.0 {
                out.absorbed = damage;
                return out;
            }
            piece.health -= through;
            through = (-piece.health).max(0.0);
            piece.health = piece.health.max(0.0);
            out.absorbed = damage - through;
            if piece.broken() {
                broke = Some(piece.kind);
            }
        }
        if through > 0.0 {
            out.through = through;
            out.leg_lost = bim.health.shot(part, through, cut);
            let wounds = Part::ALL.map(|p| bim.health.wounds(p) > 0);
            bim.character.set_wounds(wounds);
            // The first drop now, on the tile under it; a cut splashes.
            bim.drip_timer = 0.0;
            if cut {
                let at = bim.character.pos;
                let nav = self.maps.for_body(false, self.room.bath.is_open());
                self.room
                    .filth
                    .splash_blood(at, &mut self.rng, |tile| nav.can_reach(at, tile));
            }
        }
        if let Some(kind) = broke {
            out.piece_broke = true;
            self.pieces_broken.push((who, kind));
            self.refresh_worn(who);
        }
        out
    }

    /// Every worn piece a hit broke since the world last asked: whose,
    /// and what it was.
    pub fn take_pieces_broken(&mut self) -> Vec<(usize, ArmourKind)> {
        std::mem::take(&mut self.pieces_broken)
    }

    /// One part's health — the head, the body or the legs. The three add
    /// up to [`Game::health`].
    pub fn part_health(&self, who: usize, part: Part) -> f32 {
        self.bims[who].health.part(part)
    }

    /// The blood it has left, out of `health::MAX_BLOOD`.
    pub fn blood(&self, who: usize) -> f32 {
        self.bims[who].health.blood()
    }

    /// Open wounds on one part.
    pub fn wounds(&self, who: usize, part: Part) -> u32 {
        self.bims[who].health.wounds(part)
    }

    /// Open wounds all told: nought is not bleeding.
    pub fn bleeding(&self, who: usize) -> u32 {
        self.bims[who].health.bleeding()
    }

    /// How many legs it has lost: none, one, or both.
    pub fn legs_lost(&self, who: usize) -> u32 {
        self.bims[who].health.legs_lost()
    }

    /// Out cold for want of blood — lying where it dropped, alive.
    pub fn is_unconscious(&self, who: usize) -> bool {
        self.bims[who].character.is_unconscious()
    }

    // --- dressing a wound ----------------------------------------------------

    /// Send `who` to dress `part` of `patient` — itself, or a crewmate —
    /// with one of the room's bandages: the walk to the patient and ten
    /// minutes with hands on it, and the wounds on that part closed when
    /// the hands come off (`apply_dressings`). The player's order, from the
    /// inventory or the menu on a body; it displaces whatever the Bim was
    /// on, like any other order.
    ///
    /// Refused for a helper that cannot do it — dead, out cold, outside —
    /// a patient that is dead or outside, no bandage to hand, or a part
    /// with nothing open on it: a bandage on a whole part is a bandage
    /// wasted, and the menu is greyed for the same reasons.
    pub fn bandage(&mut self, who: usize, patient: usize, part: Part) -> bool {
        if who >= self.bims.len()
            || patient >= self.bims.len()
            || !self.bims[who].is_alive()
            || !self.bims[patient].is_alive()
            || self.bims[who].character.is_unconscious()
            || self.bims[who].character.is_outside()
            // A patient outside in a suit is nowhere the helper can walk
            // to: `patient_stand` reads the room's crew list, which has
            // nobody outside on it, and the chain would drop on its
            // first step with the helper's own errand already shoved
            // aside.
            || self.bims[patient].character.is_outside()
            || self.room.bandages == 0
            || self.bims[patient].health.wounds(part) == 0
        {
            return false;
        }
        let kind = Kind::Bandage {
            patient,
            part: part.code(),
        };
        if !self.take_over(who, kind, task::BANDAGE_MINUTES) {
            return false;
        }
        // The walk picks its spot from where the patient stands, and the
        // order may come before the room has been told this step.
        self.tell_the_room_where_the_crew_are();
        let taken = self.taken_for(who);
        self.bims[who].task = Some(Task::bandage(
            who,
            patient,
            part.code(),
            &mut self.bims[who].character,
            &mut self.room,
            &self.maps,
            &taken,
        ));
        true
    }

    /// Every dressing the chains finished this step, done: the wounds on
    /// the part closed, a bandage off the count, the blotch off the body.
    /// Only where the helper is still beside the patient — within two
    /// tiles, or is the patient — and a bandage is still to hand: a patient
    /// that walked off mid-dressing, or a store the world emptied since the
    /// order, is ten minutes lost and nothing else. A patient dead in the
    /// meantime is past dressing.
    fn apply_dressings(&mut self) {
        for (helper, patient, part) in core::mem::take(&mut self.room.dressed) {
            let Some(part) = Part::from_code(part) else {
                continue;
            };
            if helper >= self.bims.len()
                || patient >= self.bims.len()
                || !self.bims[patient].is_alive()
            {
                continue;
            }
            let apart = (self.bims[helper].character.pos - self.bims[patient].character.pos).len();
            if (helper != patient && apart > 2.0 * TILE) || self.room.bandages == 0 {
                continue;
            }
            let bim = &mut self.bims[patient];
            if bim.health.bandage(part) {
                self.room.bandages -= 1;
                self.room.bandages_used += 1;
            }
            let wounds = Part::ALL.map(|p| bim.health.wounds(p) > 0);
            bim.character.set_wounds(wounds);
        }
    }

    /// Bandages to hand. Aboard, the hold's count, set by the world every
    /// step; in the classic room, what it started with less what was used.
    pub fn bandages(&self) -> u32 {
        self.room.bandages
    }

    pub fn set_bandages(&mut self, n: u32) {
        self.room.bandages = n;
    }

    /// Bandages used up since the last call, for the world to take off
    /// the hold.
    pub fn take_bandages_used(&mut self) -> u32 {
        core::mem::take(&mut self.room.bandages_used)
    }

    /// What a Bim has on it.
    pub fn gear(&self, who: usize) -> Gear {
        self.bims[who].gear
    }

    // --- the pack and what is worn ------------------------------------------

    /// What is in a Bim's pack, cell by cell.
    pub fn pack(&self, who: usize) -> [Option<Item>; PACK_CELLS] {
        self.bims[who].gear.pack
    }

    /// The piece worn on a part, broken or not.
    pub fn worn(&self, who: usize, part: Part) -> Option<Piece> {
        self.bims[who].gear.worn(part)
    }

    /// What the armour adds to one part's health: the piece's health left,
    /// nothing for none or a broken one.
    pub fn part_bonus(&self, who: usize, part: Part) -> f32 {
        self.bims[who].gear.part_bonus(part)
    }

    /// What the armour adds all told — the blue bar on the end of the
    /// green one.
    pub fn armour_health(&self, who: usize) -> f32 {
        self.bims[who].gear.armour_health()
    }

    /// Put an item in a Bim's pack: into `cell`, or the first free one.
    /// The world's half of a fetch — the piece has already left the hold.
    /// `false`, and the item is not taken, when the cell is full or there
    /// is none free.
    pub fn give(&mut self, who: usize, cell: Option<usize>, item: Item) -> bool {
        let Some(bim) = self.bims.get_mut(who) else {
            return false;
        };
        let Some(cell) = cell.or_else(|| bim.gear.free_cell()) else {
            return false;
        };
        match bim.gear.pack.get_mut(cell) {
            Some(slot @ None) => {
                *slot = Some(item);
                true
            }
            _ => false,
        }
    }

    /// Take an item out of a Bim's pack, for the hold or a shelf: the
    /// other half of a stow. A **broken** piece is refused and left where
    /// it is — the hold counts pieces as resources and a broken one is
    /// worth nothing there; it can only be [`Game::discard`]ed.
    pub fn take(&mut self, who: usize, cell: usize) -> Option<Item> {
        let slot = self.bims.get_mut(who)?.gear.pack.get_mut(cell)?;
        if let Some(Item::Armour(piece)) = slot
            && piece.broken()
        {
            return None;
        }
        slot.take()
    }

    /// Throw an item away for good. What the pop-up offers for a broken
    /// piece. `false` when the cell was empty.
    pub fn discard(&mut self, who: usize, cell: usize) -> bool {
        self.bims
            .get_mut(who)
            .and_then(|b| b.gear.pack.get_mut(cell))
            .is_some_and(|slot| slot.take().is_some())
    }

    /// Put on what is in a pack cell: a piece goes on the part it is cut
    /// for and whatever was worn there comes back into the cell; a weapon
    /// swaps with the weapon slot the same way. Refused for a stack, an
    /// empty cell, or a Bim that is dead. Returns what came off, if
    /// anything, for the caller's information — nothing both for a
    /// refusal and for an empty slot.
    pub fn equip(&mut self, who: usize, cell: usize) -> Option<Item> {
        if !self.bims.get(who).is_some_and(|b| b.is_alive()) {
            return None;
        }
        let gear = &mut self.bims[who].gear;
        let off = match *gear.pack.get(cell)? {
            Some(Item::Armour(piece)) => {
                let slot = gear.worn_mut(piece.kind.slot());
                let off = slot.replace(piece).map(Item::Armour);
                gear.pack[cell] = off;
                off
            }
            Some(Item::Weapon(weapon)) => {
                let off = gear.weapon.replace(weapon).map(Item::Weapon);
                gear.pack[cell] = off;
                off
            }
            Some(Item::Stack(_)) | None => return None,
        };
        self.refresh_worn(who);
        off
    }

    /// Take off what is worn on a part, into the first free pack cell.
    /// `false` with nothing there, a full pack, or a Bim that is dead.
    pub fn unequip(&mut self, who: usize, part: Part) -> bool {
        if !self.bims.get(who).is_some_and(|b| b.is_alive()) {
            return false;
        }
        let gear = &mut self.bims[who].gear;
        let Some(cell) = gear.free_cell() else {
            return false;
        };
        let Some(piece) = gear.worn_mut(part).take() else {
            return false;
        };
        gear.pack[cell] = Some(Item::Armour(piece));
        self.refresh_worn(who);
        true
    }

    // --- looting a body -------------------------------------------------------

    /// Down: dead, or out cold. What a body has to be to be looted, and
    /// what the world hands the other room as `set_visitors_down`.
    pub fn is_down(&self, who: usize) -> bool {
        self.bims
            .get(who)
            .is_some_and(|b| !b.is_alive() || b.character.is_unconscious())
    }

    /// What a body shows when it is looted, cell by cell in [`LootCell`]
    /// order: the pack's nine, then the head, the body, the legs and the
    /// weapon in hand — a worn piece with its health, a weapon as the item
    /// a pack would hold it as. Down or not: the window asks, and whether
    /// anything may be taken is [`Game::take_from_body`]'s to say.
    pub fn loot_cells(&self, who: usize) -> [Option<Item>; LOOT_CELLS] {
        let mut cells = [None; LOOT_CELLS];
        let Some(bim) = self.bims.get(who) else {
            return cells;
        };
        let gear = &bim.gear;
        cells[..PACK_CELLS].copy_from_slice(&gear.pack);
        cells[LootCell::Head.code() as usize] = gear.head.map(Item::Armour);
        cells[LootCell::Body.code() as usize] = gear.body.map(Item::Armour);
        cells[LootCell::Legs.code() as usize] = gear.legs.map(Item::Armour);
        cells[LootCell::Weapon.code() as usize] = gear.weapon.map(Item::Weapon);
        cells
    }

    /// Take one thing off a body: a pack cell emptied, a worn piece
    /// stripped — its damage with it, broken or not: the looter's pack can
    /// hold what the hold will not — or the weapon out of its hand. The
    /// room's half of a loot; the world puts what comes back into the
    /// looter's pack (`give`), and reach is its check. Refused, `None`,
    /// for a Bim that is alive and awake — a crewmate that came round is
    /// no longer a body — and for an empty cell. The picture follows.
    pub fn take_from_body(&mut self, who: usize, cell: LootCell) -> Option<Item> {
        if !self.is_down(who) {
            return None;
        }
        let gear = &mut self.bims[who].gear;
        let taken = match cell {
            LootCell::Pack(cell) => gear.pack.get_mut(cell as usize)?.take(),
            LootCell::Head => gear.head.take().map(Item::Armour),
            LootCell::Body => gear.body.take().map(Item::Armour),
            LootCell::Legs => gear.legs.take().map(Item::Armour),
            LootCell::Weapon => gear.weapon.take().map(Item::Weapon),
        }?;
        // A body that is down draws no weapon — `tick_combat` holsters it
        // — but the hand is emptied now, not next step.
        self.bims[who].character.set_armed(None);
        self.refresh_worn(who);
        Some(taken)
    }

    /// The picture of what a Bim wears, put right after its gear changed:
    /// each part's piece and whether it is broken, for the cap, the plate
    /// and the guards on the deck.
    fn refresh_worn(&mut self, who: usize) {
        let bim = &mut self.bims[who];
        let worn = Part::ALL.map(|p| {
            bim.gear.worn(p).map(|piece| Worn {
                kind: piece.kind,
                broken: piece.broken(),
            })
        });
        bim.character.set_worn(worn);
    }

    /// Where a Bim stands to reach into a container — a bench's use spot,
    /// a shelf's, the spot in front of a cold store — for `send_to`;
    /// `None` for one the room does not have.
    pub fn container_spot(&self, container: Container) -> Option<Vec2> {
        match container {
            Container::Bench(i) => self.room.benches.get(i).map(|b| b.at),
            Container::Shelf(i) => self.room.shelves.get(i).map(|s| s.1),
            Container::Fridge(i) => {
                (i < self.room.fridges.len()).then(|| self.room.fridge_station(i))
            }
        }
    }

    /// A container's footprint, for the reach check: whether a Bim is
    /// within so many tiles of it.
    pub fn container_frame(&self, container: Container) -> Option<Rect> {
        match container {
            Container::Bench(i) => self.room.benches.get(i).map(|b| b.frame),
            Container::Shelf(i) => self.room.shelves.get(i).map(|s| s.0),
            Container::Fridge(i) => self.room.fridges.get(i).map(|f| f.frame),
        }
    }

    /// Whether a Bim stands within `tiles` of a container's footprint —
    /// alive, aboard, and near enough to reach into it.
    pub fn within_reach(&self, who: usize, container: Container, tiles: f32) -> bool {
        let Some(frame) = self.container_frame(container) else {
            return false;
        };
        let Some(bim) = self.bims.get(who).filter(|b| b.is_alive()) else {
            return false;
        };
        if bim.character.is_outside() {
            return false;
        }
        let p = bim.character.pos;
        let near = vec2(
            clamp(p.x, frame.min.x, frame.max.x),
            clamp(p.y, frame.min.y, frame.max.y),
        );
        (p - near).len() <= tiles * TILE
    }

    /// The numbers of the weapon in a Bim's hand, if there is one.
    pub fn weapon_stats(&self, who: usize) -> Option<WeaponStats> {
        self.bims[who].gear.weapon.map(|w| w.stats())
    }

    /// Whether a Bim has its weapon drawn: in combat mode, and able to
    /// use it this instant.
    pub fn is_armed(&self, who: usize) -> bool {
        self.bims[who].character.is_armed()
    }

    /// How many shots are in the air.
    pub fn bolts_in_flight(&self) -> usize {
        self.combat.bolts.len()
    }

    /// Where a Bim looks from, for the probes: itself, and the peek either
    /// side of it when it stands against a wall.
    #[allow(dead_code)]
    pub fn eyes_for_probe(&self, who: usize) -> Vec<Vec2> {
        self.room
            .sight
            .eyes_from(self.bims[who].character.pos)
            .into_iter()
            .map(|eye| eye.at)
            .collect()
    }

    /// Trace what the crew can see from where they stand now, if anything
    /// has moved since the last trace. Every frame before the picture,
    /// and a probe's to call when it wants to ask `seen_at` without one.
    pub fn observe(&mut self) {
        if self.fog != Fog::Crew {
            return;
        }
        let eyes: Vec<Vec2> = self.bims.iter().map(|b| b.character.pos).collect();
        let shut = self.shut_now();
        self.room.sight.observe(&eyes, &shut);
    }

    /// What is in the way of a line of sight besides the walls, right
    /// now: a door with its leaves shut, an airlock with nobody at it —
    /// everybody on the deck counts, the station's people included, since
    /// the doors open for them too — and the heads' door.
    fn shut_now(&self) -> Vec<Rect> {
        let bodies: Vec<Vec2> = self
            .bims
            .iter()
            .map(|b| b.character.pos)
            .chain(self.visitors.iter().copied())
            .collect();
        let mut shut = self.room.shut_leaves(&bodies);
        if let Some(door) = self.room.closed_door() {
            shut.push(door);
        }
        shut
    }

    /// Whether the crew see the tile a room point is in, as of the last
    /// trace.
    pub fn seen_at(&self, x: f32, y: f32) -> bool {
        self.room.sight.seen_at(vec2(x, y))
    }

    /// What the fog over a room point is, as of the last trace — see
    /// `Sight::veil_at`. For the probes.
    pub fn veil_at(&self, x: f32, y: f32) -> u32 {
        self.room.sight.veil_at(vec2(x, y))
    }

    /// Which of the ship's doors a point is in, if one.
    pub fn door_at(&self, x: f32, y: f32) -> Option<usize> {
        self.room.door_at(vec2(x, y))
    }

    /// Which bay a point is on, if one; and the hob and the cold store.
    pub fn bay_at(&self, x: f32, y: f32) -> Option<usize> {
        self.room.bay_at(vec2(x, y))
    }

    pub fn hob_at(&self, x: f32, y: f32) -> Option<usize> {
        self.room.hob_at(vec2(x, y))
    }

    pub fn fridge_at(&self, x: f32, y: f32) -> Option<usize> {
        self.room.fridge_at(vec2(x, y))
    }

    pub fn dishwasher_at(&self, x: f32, y: f32) -> Option<usize> {
        self.room.dishwasher_at(vec2(x, y))
    }

    /// Where bay `bay` stands. For the probes, which click on it.
    pub fn bay_frame_for_probe(&self, bay: usize) -> Rect {
        self.bay(bay).frame
    }

    pub fn ship_door_is_open(&self, i: usize) -> bool {
        self.room.doors.get(i).is_some_and(|d| d.is_open())
    }

    pub fn ship_door_is_held(&self, i: usize) -> bool {
        self.room.doors.get(i).is_some_and(|d| d.held)
    }

    pub fn ship_door_is_locked(&self, i: usize) -> bool {
        self.room.doors.get(i).is_some_and(|d| d.locked)
    }

    /// The Bim walks to the door's panel and works it — the bathroom door's
    /// arrangement, for any of the ship's doors.
    pub fn order_door(&mut self, who: usize, i: usize, order: door::Order) {
        if i < self.room.doors.len() {
            self.send_to_switch(who, Switch::Door(i, order));
        }
    }

    pub fn fridge_is_open(&self, i: usize) -> bool {
        self.room.fridge_is_open(i)
    }

    pub fn stove_is_on(&self, i: usize) -> bool {
        self.room.hob(i).on
    }

    /// Game minutes before hob `i` turns itself off, or zero when it is
    /// not counting down at all.
    pub fn stove_idle_left(&self, i: usize) -> f32 {
        self.room.stove_idle_left(i)
    }

    /// True while a task has the Bim, so the host can grey out "Make food".
    pub fn is_busy(&self, who: usize) -> bool {
        self.bims[who].task.is_some()
    }

    /// Send the Bim to work a switch. Nothing aboard changes without it: the
    /// state only moves when the Bim's hand gets there, and this displaces
    /// whatever it was doing exactly like any other errand.
    fn send_to_switch(&mut self, who: usize, which: Switch) {
        if !self.can_begin(who, Kind::Switch(which))
            || !self.take_over(who, Kind::Switch(which), 0.0)
        {
            return;
        }
        let taken = self.taken_for(who);
        self.bims[who].task = Some(Task::work_switch(
            who,
            which,
            &mut self.bims[who].character,
            &mut self.room,
            &self.maps,
            &taken,
        ));
    }

    pub fn toggle_fridge(&mut self, who: usize, i: usize) {
        self.send_to_switch(who, Switch::FridgeDoor(i));
    }

    /// Flipping the hob is a physical act: the Bim walks over and does it,
    /// rather than the switch moving by itself.
    pub fn toggle_stove(&mut self, who: usize, i: usize) {
        self.send_to_switch(who, Switch::Hob(i));
    }

    /// Kick off the make-a-meal chain. Ignored if one is already running.
    /// Cook whichever the Bim fancies. Both recipes cost two out of the cold
    /// store, so with fewer than that left there is nothing to be done.
    pub fn make_food(&mut self, who: usize) -> bool {
        // Already on a meal: asking again is not a second dinner. `cook` turns
        // down the *same* dish by itself, but this one picks at random and
        // would otherwise displace a half-chopped stew with a bowl.
        if self.bims[who]
            .task
            .as_ref()
            .is_some_and(|task| matches!(task.kind(), Kind::Meal(_) | Kind::Leftovers))
        {
            return false;
        }

        // There is a pot on the hob with something in it. Cooking a second one
        // on top of that would throw the first away, and the whole point of a
        // pot holding two is that the Bim comes back to it.
        if self.eat_leftovers(who) {
            return true;
        }
        // A stew on the shelf is a meal already made: warm it through rather
        // than start from raw. It was cooked to be eaten, and cooking round
        // it would leave the shelf full and the store bare.
        if self.reheat(who) {
            return true;
        }
        // Half and half, which over two meals a day comes out at about one
        // bowl a day — the tofu the Bim wants daily. If the store cannot run
        // to what it fancies it has the other, which is what keeps a Bim with
        // plenty of greens and no soy eating at all.
        let (first, second) = if self.rng.chance(0.5) {
            (Dish::Stew, Dish::Bowl)
        } else {
            (Dish::Bowl, Dish::Stew)
        };
        self.cook(who, first) || self.cook(who, second)
    }

    /// Warm a stew from the cold store through and eat it. Nothing to do
    /// when there is none on the shelf.
    pub fn reheat(&mut self, who: usize) -> bool {
        if self.room.stew == 0
            || !self.can_begin(who, Kind::Reheat)
            || !self.take_over(who, Kind::Reheat, 0.0)
        {
            return false;
        }
        let taken = self.taken_for(who);
        self.bims[who].task = Some(Task::reheat(
            who,
            &mut self.bims[who].character,
            &mut self.room,
            &self.maps,
            &taken,
        ));
        true
    }

    /// Cook a stew for the cold store: a vegetable and a block of tofu,
    /// chopped, through the pot, and put away in a tub. The player's own
    /// order from the hob's menu as well as the stew job's errand, and
    /// refused without both halves of the recipe.
    pub fn make_stew(&mut self, who: usize) -> bool {
        if self.bims[who]
            .task
            .as_ref()
            .is_some_and(|task| task.kind() == Kind::Batch)
        {
            return false;
        }
        if !self.room.can_make_stew()
            || !self.can_begin(who, Kind::Batch)
            || !self.take_over(who, Kind::Batch, 0.0)
        {
            return false;
        }
        let taken = self.taken_for(who);
        self.bims[who].task = Some(Task::batch(
            who,
            &mut self.bims[who].character,
            &mut self.room,
            &self.maps,
            &taken,
        ));
        true
    }

    /// Helpings left in the pot, 0 when there is nothing to come back to.
    pub fn pot_servings(&self, i: usize) -> u32 {
        self.room.hob(i).pot_servings
    }

    /// How many a fresh pot holds, so the host can say so without repeating it.
    pub fn pot_capacity(&self) -> u32 {
        task::SERVINGS_PER_POT
    }

    /// Go and have what is left in the pot: a plate, the rest of the stew, and
    /// the same sit-down and clearing-up as a meal that was cooked.
    pub fn eat_leftovers(&mut self, who: usize) -> bool {
        if self.room.hobs[0].pot_servings == 0
            || !self.can_begin(who, Kind::Leftovers)
            || !self.take_over(who, Kind::Leftovers, 0.0)
        {
            return false;
        }
        let taken = self.taken_for(who);
        self.bims[who].task = Some(Task::leftovers(
            who,
            &mut self.bims[who].character,
            &mut self.room,
            &self.maps,
            &taken,
        ));
        true
    }

    pub fn cook(&mut self, who: usize, dish: Dish) -> bool {
        if !self.room.can_cook(dish)
            || !self.can_begin(who, Kind::Meal(dish))
            || !self.take_over(who, Kind::Meal(dish), 0.0)
        {
            return false;
        }
        let taken = self.taken_for(who);
        self.bims[who].task = Some(Task::make_food(
            who,
            dish,
            &mut self.bims[who].character,
            &mut self.room,
            &self.maps,
            &taken,
        ));
        true
    }

    pub fn dishwasher_loaded(&self, i: usize) -> u32 {
        self.room.dishwashers[i.min(self.room.dishwashers.len() - 1)].loaded
    }

    pub fn dishwasher_capacity(&self) -> u32 {
        crate::dish::CAPACITY
    }

    /// Clean plates in the chopping board's drawer.
    pub fn plates(&self) -> u32 {
        self.room.plates
    }

    pub fn plate_drawer_capacity(&self) -> u32 {
        crate::room::PLATE_DRAWER
    }

    /// Game minutes left of the cycle, or zero when it is not running.
    pub fn dishwasher_cycle_left(&self, i: usize) -> f32 {
        self.room.dishwashers[i.min(self.room.dishwashers.len() - 1)].cycle_left
    }

    /// Start a cycle early, without waiting for the rack to fill. The Bim
    /// walks over and presses the button, the same as it does when its own
    /// clearing-up fills the rack.
    pub fn run_dishwasher(&mut self, who: usize, i: usize) {
        self.send_to_switch(who, Switch::Dishwasher(i));
    }

    pub fn door_is_open(&self) -> bool {
        self.room.bath.is_open()
    }

    pub fn door_is_locked(&self) -> bool {
        self.room.bath.locked
    }

    /// The Bim walks to the door panel and works it. There is one on each side
    /// of the bulkhead, so it uses whichever it is nearest.
    ///
    /// Which way it works it is decided here, from the door the player is
    /// looking at, and carried along. Reading the door again when the hand
    /// arrives would undo the very thing that was asked for: setting off
    /// drops whatever the Bim was on, and dropping a trip to the heads opens
    /// the door it had shut behind itself.
    pub fn toggle_door(&mut self, who: usize) {
        let want = !self.room.bath.is_open();
        self.send_to_switch(who, Switch::BathDoor(want));
    }

    /// Locking shuts it too. Unlocking leaves it shut but openable.
    ///
    /// Decided at the click, for the same reason: a Bim asked to unlock while
    /// sitting on the pan used to let go of the errand — which takes its own
    /// lock off — walk to the panel, find the door unlocked, and lock itself
    /// back in.
    pub fn toggle_door_lock(&mut self, who: usize) {
        let want = !self.room.bath.locked;
        self.send_to_switch(who, Switch::BathLock(want));
    }

    /// Use the heads, then wash. Refused through a locked door.
    /// Whether the Bim could set off for the heads right now.
    ///
    /// A locked door only stops a Bim on the wrong side of it. One already in
    /// there has no door to get through and starts at the pan.
    pub fn can_use_toilet(&self, who: usize) -> bool {
        let shut_out =
            self.room.bath.locked && !self.room.bath.shell.contains(self.bims[who].character.pos);
        !shut_out && self.can_begin(who, Kind::Heads)
    }

    pub fn use_toilet(&mut self, who: usize) -> bool {
        if !self.can_use_toilet(who) || !self.take_over(who, Kind::Heads, 0.0) {
            return false;
        }
        let taken = self.taken_for(who);
        self.bims[who].task = Some(Task::use_toilet(
            who,
            &mut self.bims[who].character,
            &mut self.room,
            &self.maps,
            &taken,
        ));
        true
    }

    /// The moment something is made in the galley, judge it: for every tile
    /// within [`GALLEY_REACH`] of the hob with a mess on it, one roll at
    /// [`BAD_FOOD_PER_DIRTY_TILE`], and one bad roll is a bad meal. A clean
    /// galley draws nothing from the stream, so a clean run is unchanged.
    fn judge_the_food(&mut self) {
        for hob in self.room.take_judgements() {
            let dirty = self
                .room
                .filth
                .dirty_tiles_within(self.room.pot_pos(hob), GALLEY_REACH);
            let mut bad = false;
            for _ in 0..dirty {
                if self.rng.chance(BAD_FOOD_PER_DIRTY_TILE) {
                    bad = true;
                }
            }
            self.room.hob_mut(hob).food_bad = bad;
        }
    }

    /// Food poisoning, from now: [`POISONING_LASTS`] of it. Remembered once
    /// per bout — a second mouthful of the same meal is the same illness.
    fn poison(&mut self, who: usize) {
        if !self.bims[who].is_poisoned() {
            self.remember(who, What::FoodPoisoning, 0);
        }
        self.bims[who].poisoned_for = POISONING_LASTS;
    }

    /// Game hours of food poisoning left, nought when well. For the panel.
    pub fn poisoning(&self, who: usize) -> f32 {
        self.bims[who].poisoned_for / clock::HOUR
    }

    /// Make `who` ill this instant, for the probes: the roll is the galley's
    /// business and what the illness does is the thing under test.
    #[allow(dead_code)]
    pub fn poison_for_probe(&mut self, who: usize) {
        self.poison(who);
    }

    /// Whether what the galley last made is bad, for the probes.
    #[allow(dead_code)]
    pub fn food_is_bad(&self) -> bool {
        self.room.hobs.iter().any(|h| h.food_bad)
    }

    /// Where the pot stands, for a probe that wants to foul the galley.
    #[allow(dead_code)]
    pub fn pot_pos_for_probe(&self) -> Vec2 {
        self.room.pot_pos(0)
    }

    /// Whether a shower can be begun: there is one, nobody else is in it,
    /// and there is a way to it.
    pub fn can_shower(&self, who: usize) -> bool {
        self.room.showers.first().copied().is_some() && self.can_begin(who, Kind::Shower)
    }

    /// Off to the shower. The need's own errand and nothing else's: there is
    /// no menu item and no job row for it, like the heads.
    pub fn take_shower(&mut self, who: usize) -> bool {
        if !self.can_shower(who) || !self.take_over(who, Kind::Shower, 0.0) {
            return false;
        }
        let taken = self.taken_for(who);
        self.bims[who].task = Some(Task::shower(
            who,
            &mut self.bims[who].character,
            &mut self.room,
            &self.maps,
            &taken,
        ));
        true
    }

    /// Send the Bim to bed for `minutes` of game time. Ignored while it is
    /// already busy, like every other errand.
    pub fn rest(&mut self, who: usize, minutes: f32) -> bool {
        if !self.can_begin(who, Kind::Rest) || !self.take_over(who, Kind::Rest, minutes) {
            return false;
        }
        let taken = self.taken_for(who);
        self.bims[who].task = Some(Task::rest(
            who,
            minutes,
            &mut self.bims[who].character,
            &mut self.room,
            &self.maps,
            &taken,
        ));
        true
    }

    /// What the Bim is busy with, for the host's status line: 0 idle, then
    /// one number per errand. Numbers rather than names, because no strings
    /// cross the boundary.
    /// Which fixtures a Bim's errand is using. For the probes.
    pub fn picks_for_probe(&self, who: usize) -> Option<task::Picks> {
        self.bims[who].task.as_ref().map(|t| t.picks())
    }

    pub fn activity(&self, who: usize) -> u32 {
        match &self.bims[who].task {
            None => 0,
            Some(task) => job_code(task.kind(), task.rest_minutes()),
        }
    }

    /// Which step of the chain is running, for the host's status line.
    /// Zero means idle.
    pub fn task_step(&self, who: usize) -> u32 {
        match &self.bims[who].task {
            None => 0,
            Some(t) => t.step() as u32 + 1,
        }
    }

    // --- rendering --------------------------------------------------------

    pub fn render(&mut self) {
        self.list.clear();
        self.room.draw(&mut self.list);
        // On the deck, under everything: the Bim walks over its own mess.
        self.room.filth.draw(&mut self.list);

        // The path fades out behind each Bim, so the shape of a wander is
        // visible. Both trails are the same colour: they are the shape of the
        // room being used, not a label saying who went where — the two are
        // told apart by the bodies at the head of them.
        for bim in &self.bims {
            for f in &bim.trail {
                let t = 1.0 - f.age / TRAIL_LIFE;
                self.list
                    .circle(f.pos, 3.0 + 4.0 * t, TRAIL.alpha(0.16 * t * t));
            }
        }

        // Destination pings: a ring that expands and fades where the order landed.
        for m in &self.markers {
            let t = m.age / MARKER_LIFE;
            let fade = 1.0 - t;
            let colour = if m.bad { WARN } else { ACCENT };
            self.list.circle(m.pos, 5.0, colour.alpha(0.55 * fade));
            self.list
                .ring(m.pos, 10.0 + 26.0 * t, 2.0, colour.alpha(0.7 * fade * fade));
            // A cross through a place it cannot get to, so a refusal reads as
            // a refusal and not as a ping that happens to be a different hue.
            if m.bad {
                for turn in [0.7, -0.7] {
                    self.list
                        .rect(m.pos, vec2(26.0, 3.0), turn, 1.5, WARN.alpha(0.85 * fade));
                }
            }
        }

        // Bodies in crew order, so who is drawn on top of whom does not
        // change as they walk past each other — and only the ones in view:
        // a station's people behind a bulkhead are not drawn at all.
        for (who, bim) in self.bims.iter().enumerate() {
            if self.body_seen(who) {
                bim.character.draw(&mut self.list);
                // A shot that landed: a flash on the body, gone in a blink.
                if bim.hit_flash > 0.0 {
                    let t = bim.hit_flash / HIT_FLASH;
                    let at = bim.character.drawn_at();
                    self.list
                        .circle(at, 42.0 + 30.0 * (1.0 - t), HIT.alpha(0.45 * t));
                    self.list.ring(at, 50.0, 3.0, HIT.alpha(0.9 * t));
                }
            }
        }
        // Bedding and bunk rails go over the Bim, so getting into bed puts it
        // under the covers rather than on top of them.
        self.room.draw_over(&mut self.list);

        // What nobody sees, fogged. Over the deck and the fixtures and
        // under the night, the rings and the marquee: a fog that hid the
        // pointer's own marks would be a fog over the pointer.
        self.observe();
        match self.fog {
            Fog::Crew => self.room.sight.draw(&mut self.list, false),
            Fog::All => self.room.sight.draw(&mut self.list, true),
            Fog::None => {}
        }
        // The shots, over the fog: a bolt is always seen, whatever it
        // flies through.
        self.combat.draw(&mut self.list);

        // Night falls over the whole room at once.
        // Aboard a ship the room has no shell, and a night wash the size of
        // the build area would be a dark square hanging in space round the
        // hull: the ship painter owns the sky, so the wash stays the classic
        // room's.
        let dark = (1.0 - self.clock.daylight()) * NIGHT_DEPTH;
        if dark > 0.002 && self.room.shell {
            let room = self.room.bounds;
            self.list
                .rect(room.center(), room.size(), 0.0, 0.0, NIGHT.alpha(dark));
        }

        // Whatever a panel is pointing at, ringed. Over the night wash rather
        // than under it: a highlight that dims at three in the morning is no
        // highlight. In the ship's own cyan, so it reads as neither a
        // selection (which is the Bim's green) nor trouble (which is warm).
        // Every fixture of the kind, since a row names the kind: the cook
        // row rings both hobs of a ship with two galleys.
        for area in self.room.spot_rects(self.highlight) {
            let size = area.size() + vec2(HIGHLIGHT_MARGIN, HIGHLIGHT_MARGIN);
            self.list
                .rect(area.center(), size, 0.0, 6.0, GLOW.alpha(0.16));
            self.list
                .stroke_rect(area.center(), size, 0.0, 6.0, 2.0, GLOW.alpha(0.95));
        }

        // The marquee sits on top of everything, like the cursor it belongs to.
        if let Some((start, end)) = self.drag {
            let box_ = Rect::from_corners(start, end);
            self.list
                .rect(box_.center(), box_.size(), 0.0, 0.0, MARQUEE_FILL);
            self.list.stroke_rect(
                box_.center(),
                box_.size(),
                0.0,
                0.0,
                1.5,
                MARQUEE_EDGE.alpha(0.8),
            );
        }
    }

    /// The frame's shapes, for a host that embeds this room in a picture of
    /// its own rather than replaying the buffer straight to a canvas — the
    /// ship game paints the room turned with the ship.
    pub fn shapes(&self) -> &[f32] {
        self.list.data()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::health::MAX_BLOOD;

    /// A frame at 1x.
    const DT: f32 = 1.0 / 60.0;

    fn room() -> Game {
        Game::new(3, ROOM_W, ROOM_H)
    }

    #[test]
    fn a_hostile_room_goes_to_war_over_a_target_and_records_its_shots() {
        let mut game = room();
        game.set_autonomous(false);
        let kate = game.put_for_probe(1, vec2(ROOM_W * 0.35, ROOM_H * 0.5));
        game.put_for_probe(0, vec2(ROOM_W * 0.45, ROOM_H * 0.5));
        game.set_hostile_bodies(true);
        // A target out in the open, four tiles from both.
        let target = kate + vec2(4.0 * TILE, 0.0);
        game.set_hostiles(vec![Some((target, WeaponKind::LaserPistol))]);
        game.simulate(DT);
        assert!(game.is_recruited(), "at war, under orders");
        assert!(game.bims[1].character.is_recruited());
        // They shoot, and what they shoot is recorded rather than flown.
        for _ in 0..120 {
            game.simulate(DT);
        }
        assert_eq!(
            game.bolts_in_flight(),
            0,
            "no bolt flies in the enemy's room"
        );
        let shots = game.take_shots();
        assert!(!shots.is_empty(), "somebody shot");
        assert!(
            shots
                .iter()
                .all(|s| s.at == target && s.weapon == WeaponKind::LaserPistol)
        );
        assert!(game.take_shots().is_empty(), "drained");
        assert!(
            game.take_hits().is_empty(),
            "nothing landed on the targets: nothing flew"
        );

        // Peace: the target gone, the orders lifted.
        game.set_hostiles(Vec::new());
        game.simulate(DT);
        assert!(!game.is_recruited());
        assert!(!game.bims[1].character.is_recruited());
        assert!(!game.is_armed(0));
    }

    #[test]
    fn an_enemy_s_shot_wounds_the_body_it_lands_on_and_the_wound_bleeds() {
        let mut game = room();
        game.set_autonomous(false);
        let james = game.put_for_probe(0, vec2(ROOM_W * 0.45, ROOM_H * 0.5));
        game.put_for_probe(1, vec2(ROOM_W * 0.25, ROOM_H * 0.8));
        let from = james + vec2(3.0 * TILE, 0.0);
        // Until one lands: the odds at three tiles are nine in ten.
        let mut landed = Vec::new();
        for _ in 0..30 {
            game.enemy_fire(from, james, WeaponKind::LaserPistol);
            for _ in 0..30 {
                game.simulate(DT);
            }
            landed.extend(game.take_wounds_taken());
            if !landed.is_empty() {
                break;
            }
        }
        assert!(
            !landed.is_empty(),
            "a shot from three tiles lands within thirty"
        );
        assert!(landed.iter().all(|h| h.who == 0));
        assert_eq!(game.bleeding(0), landed.len() as u32);
        assert!(game.health(0) < 100.0);
        assert!(game.take_wounds_taken().is_empty(), "drained");
        let hit = landed[0];
        assert_eq!(game.wounds(0, hit.part), 1);
        assert!(game.part_health(0, hit.part) < hit.part.max());

        // The blood goes while the wound is open, and the walk slows.
        // Recruited, so it stands where it is rather than walking off
        // the mess it is making (`flee_filth`), and the trail is a pool.
        game.recruit_for_probe(0, true);
        let before = game.blood(0);
        for _ in 0..600 {
            game.simulate(DT);
        }
        assert!(game.blood(0) < before, "bleeding");
        // And lands on the deck as filth: the tile under it reads blood,
        // deep enough to be worth the broom.
        let under = game.bim_pos(0);
        assert_eq!(game.room.filth.kind_at(under), filth::Mess::Blood);
        assert!(game.room.filth.depth_at(under) > 0.0);
        assert!(game.dirty_tiles() > 0, "a stain the broom is for");
        // A print off it is blood too, not grime: walked onto a clean tile
        // until one of the rolls carries something.
        let next = under + vec2(TILE, 0.0);
        assert_eq!(game.room.filth.kind_at(next), filth::Mess::None);
        let mut carried = false;
        for _ in 0..100 {
            if game.room.filth.track(under, next, &mut game.rng) {
                carried = true;
                break;
            }
        }
        assert!(carried, "a boot out of a bloody tile carries some of it");
        assert_eq!(game.room.filth.kind_at(next), filth::Mess::Blood);
        // A sweep takes it up like any stain.
        game.room.filth.sweep(under);
        assert_eq!(game.room.filth.kind_at(under), filth::Mess::None);
        assert_eq!(game.room.filth.depth_at(under), 0.0);
    }

    #[test]
    fn medical_at_the_top_drops_the_sweep_to_dress_its_own_wound() {
        let mut game = room();
        // Kate under orders, so she neither sweeps nor comes over to dress
        // James: a part somebody else is walking over to dress is left to
        // them, and this is about what James does for himself.
        game.recruit_for_probe(1, true);
        // A deck worth sweeping, and James gets the broom out.
        for x in 0..4 {
            game.foul_for_probe(vec2(ROOM_W * 0.45 + x as f32 * TILE, ROOM_H * 0.55));
        }
        let who = 0;
        for _ in 0..(20 * 60 * 60) {
            game.simulate(DT);
            if game.activity(who) == JOB_CLEAN {
                break;
            }
        }
        assert_eq!(
            game.activity(who),
            JOB_CLEAN,
            "sweeping within twenty minutes"
        );
        let had = game.bandages();
        assert!(had > 0);

        // Shot mid-sweep with the medical row left where it started: it
        // is on offer, but it waits its turn behind the sweep.
        assert!(!game.wound(who, Part::Legs, 3.0).leg_lost);
        game.simulate(DT);
        assert_eq!(game.activity(who), JOB_CLEAN, "a wound at 3 waits");
        assert!(
            game.work_on_offer_for_probe(who)
                .contains(&Job::Medical.code()),
            "but it is on offer"
        );

        // At the top it is urgent: the sweep is put down, the dressing
        // starts on the spot, and the sweep is on the queue behind it.
        game.set_work_priority(Job::Medical.code(), work::HIGHEST);
        game.simulate(DT);
        assert_eq!(game.activity(who), JOB_BANDAGE);
        assert_eq!(game.agenda_len(who), 2);
        assert_eq!(game.agenda_job(who, 1), JOB_CLEAN, "the sweep waits");
        let mut steps = 0;
        while game.bleeding(who) > 0 && steps < 60 * 60 {
            game.simulate(DT);
            steps += 1;
        }
        assert_eq!(game.bleeding(who), 0, "dressed within the hour");
        assert_eq!(game.bandages(), had - 1);
        // Nothing to dress: the row is off the list again, and the sweep
        // is picked back up.
        assert!(
            !game
                .work_on_offer_for_probe(who)
                .contains(&Job::Medical.code())
        );
        for _ in 0..600 {
            game.simulate(DT);
            if game.activity(who) == JOB_CLEAN {
                break;
            }
        }
        assert_eq!(game.activity(who), JOB_CLEAN, "back to the broom");

        // Never is never: a fresh wound with the row switched off is left
        // open, urgent or not, and the player's own order still works.
        game.set_work_priority(Job::Medical.code(), work::NEVER);
        assert!(!game.wound(who, Part::Legs, 3.0).leg_lost);
        for _ in 0..600 {
            game.simulate(DT);
        }
        assert!(game.bleeding(who) > 0, "nobody doctors at never");
        assert!(
            !game
                .work_on_offer_for_probe(who)
                .contains(&Job::Medical.code())
        );
        assert!(game.bandage(who, who, Part::Legs), "ordered, it still goes");
        assert_eq!(game.activity(who), JOB_BANDAGE);
    }

    #[test]
    fn a_crewmate_bleeding_is_dressed_by_whoever_is_free() {
        let mut game = room();
        // Kate, out cold on the deck a few tiles from James: two wounds on
        // the body and the blood run down by hand, so she lies still.
        game.put_for_probe(0, vec2(ROOM_W * 0.55, ROOM_H * 0.5));
        game.put_for_probe(1, vec2(ROOM_W * 0.35, ROOM_H * 0.5));
        game.set_autonomous(false);
        for _ in 0..10 {
            game.wound(1, Part::Body, 1.0);
        }
        assert!(!game.wound(1, Part::Legs, 2.0).leg_lost);
        let mut minutes = 0.0;
        while !game.is_unconscious(1) && minutes < 60.0 {
            game.simulate(1.0);
            minutes += MINUTES_PER_SECOND;
        }
        assert!(game.is_unconscious(1));
        game.set_autonomous(true);
        game.simulate(DT);
        // James has nothing else on but the blood she has left on the deck,
        // and the dressing comes before the sweeping among equals.
        assert_eq!(
            game.work_on_offer_for_probe(0).first().copied(),
            Some(Job::Medical.code()),
            "{:?}",
            game.work_on_offer_for_probe(0)
        );
        assert_eq!(
            game.medical_on_offer(0),
            Some((1, Part::Body)),
            "the part with the most wounds first"
        );
        assert_eq!(
            game.medical_on_offer(1),
            None,
            "a patient out cold is nobody's doctor"
        );
        let had = game.bandages();
        // The walk over and ten minutes on the body, then the legs: two
        // dressings, well inside the hour.
        let mut steps = 0;
        while game.bleeding(1) > 0 && steps < 60 * 60 {
            game.simulate(DT);
            steps += 1;
        }
        assert_eq!(game.bleeding(1), 0, "dressed within the hour");
        assert_eq!(game.bandages(), had - 2);
        assert!(game.is_alive(1));
        assert!(
            !game
                .work_on_offer_for_probe(0)
                .contains(&Job::Medical.code()),
            "nothing left to dress"
        );
    }

    #[test]
    fn enough_wounds_knock_a_bim_out_and_it_lies_there_till_it_comes_round() {
        let mut game = room();
        game.set_autonomous(false);
        game.put_for_probe(0, vec2(ROOM_W * 0.45, ROOM_H * 0.5));
        for _ in 0..10 {
            assert!(!game.wound(0, Part::Body, 1.0).leg_lost);
        }
        assert_eq!(game.bleeding(0), 10);
        assert!(!game.is_unconscious(0));
        // Ten wounds are the whole of the blood in an hour: under thirty
        // per cent inside three quarters of one.
        let mut minutes = 0.0;
        while !game.is_unconscious(0) && minutes < 60.0 {
            game.simulate(1.0);
            minutes += MINUTES_PER_SECOND;
        }
        assert!(
            game.is_unconscious(0),
            "out cold at {} blood",
            game.blood(0)
        );
        assert!(game.blood(0) < MAX_BLOOD * 0.3);
        assert!(game.is_alive(0));
        assert!(!game.is_walking(0));
        assert!(!game.is_armed(0));
        // Dressed — by hand here; the chain that does it is the bandage
        // errand's — the blood comes back and it comes round.
        assert!(game.bims[0].health.bandage(Part::Body));
        assert_eq!(game.bleeding(0), 0);
        while game.is_unconscious(0) && minutes < 24.0 * 60.0 {
            game.simulate(1.0);
            minutes += MINUTES_PER_SECOND;
        }
        assert!(
            !game.is_unconscious(0),
            "come round at {} blood",
            game.blood(0)
        );
        assert!(game.is_alive(0));

        // A leg goes on the second shot that empties it.
        assert!(!game.wound(1, Part::Legs, 12.0).leg_lost);
        assert!(game.wound(1, Part::Legs, 12.0).leg_lost);
        assert_eq!(game.legs_lost(1), 1);
        // And bled out is dead.
        for _ in 0..10 {
            game.wound(1, Part::Body, 1.0);
        }
        let mut minutes = 0.0;
        while game.is_alive(1) && minutes < 120.0 {
            game.simulate(1.0);
            minutes += MINUTES_PER_SECOND;
        }
        assert!(!game.is_alive(1), "bled out");
    }

    #[test]
    fn a_bandage_is_walked_over_and_closes_the_wounds_on_one_part() {
        let mut game = room();
        game.set_autonomous(false);
        // Kate stands still — recruited, so she does not wander off while
        // James walks over — a few tiles from him.
        game.put_for_probe(0, vec2(ROOM_W * 0.55, ROOM_H * 0.5));
        game.put_for_probe(1, vec2(ROOM_W * 0.35, ROOM_H * 0.5));
        game.recruit_for_probe(1, true);
        let had = game.bandages();
        assert!(had > 0, "the classic room starts with some");
        assert!(
            !game.bandage(0, 1, Part::Body),
            "nothing to dress on a whole part"
        );
        assert!(!game.wound(1, Part::Body, 12.0).leg_lost);
        assert!(!game.wound(1, Part::Body, 12.0).leg_lost);
        assert_eq!(game.bleeding(1), 2);
        assert!(
            !game.bandage(0, 1, Part::Head),
            "the head is whole: a bandage there is wasted"
        );
        assert!(game.bandage(0, 1, Part::Body));
        assert_eq!(game.activity(0), JOB_BANDAGE);
        assert!(!game.bandage(0, 1, Part::Body), "already on it");
        // The walk and ten minutes' dressing: well inside an hour.
        let mut steps = 0;
        while game.bleeding(1) > 0 && steps < 60 * 60 {
            game.simulate(DT);
            steps += 1;
        }
        assert_eq!(game.bleeding(1), 0, "dressed within the hour");
        assert_eq!(game.wounds(1, Part::Body), 0);
        assert_eq!(game.bandages(), had - 1);
        assert_eq!(game.take_bandages_used(), 1);
        assert_eq!(game.take_bandages_used(), 0, "drained");
        assert!(game.is_alive(1));
        // Beside the patient, not on it.
        let apart = (game.bim_pos(0) - game.bim_pos(1)).len();
        assert!(apart <= 2.0 * TILE, "{apart}");
        assert_eq!(game.activity(0), 0, "the errand is over");

        // Its own wounds, on the spot, and the count runs out.
        game.set_bandages(1);
        game.wound(0, Part::Legs, 3.0);
        assert!(game.bandage(0, 0, Part::Legs));
        let mut steps = 0;
        while game.bleeding(0) > 0 && steps < 60 * 30 {
            game.simulate(DT);
            steps += 1;
        }
        assert_eq!(game.bleeding(0), 0);
        assert_eq!(game.bandages(), 0);
        game.wound(0, Part::Legs, 3.0);
        assert!(!game.bandage(0, 0, Part::Legs), "none left");
    }

    #[test]
    fn winding_a_bandage_holsters_the_weapon_and_it_is_drawn_again_after() {
        let mut game = room();
        game.set_autonomous(false);
        let james = game.put_for_probe(0, vec2(ROOM_W * 0.45, ROOM_H * 0.5));
        game.put_for_probe(1, vec2(ROOM_W * 0.2, ROOM_H * 0.85));
        // Kate unarmed, so the only bolts are his.
        game.issue(1, Gear::default());
        game.recruit_for_probe(0, true);
        // An enemy four tiles off in plain view: James shoots at it.
        let target = james + vec2(4.0 * TILE, 0.0);
        game.set_hostiles(vec![Some((target, WeaponKind::LaserPistol))]);
        // A pistol bolt reaches a body four tiles off in a quarter of a
        // second, so it is watched for over the run rather than at its end.
        let shot_over = |game: &mut Game, steps: i32| {
            let mut seen = false;
            for _ in 0..steps {
                game.simulate(DT);
                seen |= game.bolts_in_flight() > 0;
            }
            seen
        };
        assert!(shot_over(&mut game, 60), "shooting before the dressing");
        assert!(game.is_armed(0));

        // Wounded, he dresses himself on the spot — and for the dressing
        // both hands are on the bandage: the weapon goes away and nothing
        // is fired until it is done.
        game.wound(0, Part::Body, 12.0);
        assert!(game.bandage(0, 0, Part::Body));
        let mut dressed_steps = 0;
        let mut fired_while_dressing = false;
        let mut steps = 0;
        while game.bleeding(0) > 0 && steps < 60 * 30 {
            game.simulate(DT);
            steps += 1;
            let dressing = game.bims[0].task.as_ref().is_some_and(|t| t.is_dressing());
            if dressing {
                dressed_steps += 1;
                assert!(!game.is_armed(0), "holstered while winding");
                assert_eq!(game.bims[0].character.action(), Action::Bandage);
                // Anything still in the air was fired before the hands
                // went to the bandage and lands within a second.
                if dressed_steps > 60 && game.bolts_in_flight() > 0 {
                    fired_while_dressing = true;
                }
            }
        }
        assert_eq!(game.bleeding(0), 0);
        assert!(
            dressed_steps > 60,
            "the dressing takes minutes: {dressed_steps}"
        );
        assert!(!fired_while_dressing);
        // And drawn again the moment the dressing is over.
        assert!(shot_over(&mut game, 90), "shooting again after");
        assert!(game.is_armed(0));
    }

    #[test]
    fn a_fibre_target_has_the_bay_grow_fibre_into_the_store() {
        let mut game = room();
        assert_eq!(game.store_fibre(), 0);
        assert_eq!(game.target(Stock::Fibre), 0, "none asked for at dawn");
        game.set_target(Stock::Fibre, 2);
        // A day to ripen, and the crew's own meals and nights round it:
        // three days is plenty, and a bay left alone that long is a fault.
        // At frame rate — a walk cannot be stepped a whole second at a
        // time — so it is a few hundred thousand cheap steps.
        let mut planted = false;
        let mut minutes = 0.0;
        while game.store_fibre() == 0 && minutes < 3.0 * clock::DAY {
            game.simulate(DT);
            minutes += DT * MINUTES_PER_SECOND;
            planted |= (0..hydro::SPOTS as u32)
                .any(|i| game.hydro_crop(0, i) == hydro::Crop::Fibre.code());
        }
        assert!(planted, "a tray of fibre went in");
        assert!(game.store_fibre() >= 1, "and came up, and was put away");
        assert_eq!(game.take_harvested_fibre(), game.store_fibre());
        assert_eq!(game.take_harvested_fibre(), 0, "drained");
        // The world's number goes back on the shelf every step.
        game.set_stock(20, 10, 0, 5);
        assert_eq!(game.store_fibre(), 5);
    }

    #[test]
    fn an_enemy_shoots_on_the_move_and_the_crew_do_not() {
        // Kate armed in a hostile room, a target in plain view a few
        // tiles off, and the stand the tactics want somewhere else: she
        // records shots while she is still walking there.
        let mut game = room();
        game.set_autonomous(false);
        let kate = game.put_for_probe(1, vec2(ROOM_W * 0.35, ROOM_H * 0.5));
        game.put_for_probe(0, vec2(ROOM_W * 0.45, ROOM_H * 0.8));
        game.issue(0, Gear::default());
        game.set_hostile_bodies(true);
        let target = kate + vec2(3.0 * TILE, 0.0);
        game.set_hostiles(vec![Some((target, WeaponKind::LaserPistol))]);
        let mut shot_walking = false;
        for _ in 0..(60 * 4) {
            game.simulate(DT);
            if !game.take_shots().is_empty() && game.is_walking(1) {
                shot_walking = true;
            }
        }
        assert!(shot_walking, "an enemy fires while it repositions");

        // The crew hold their fire on the move: James recruited, sent
        // across the room past a target he can see, fires nothing until
        // he arrives.
        let mut game = room();
        game.set_autonomous(false);
        let james = game.put_for_probe(0, vec2(ROOM_W * 0.3, ROOM_H * 0.5));
        game.put_for_probe(1, vec2(ROOM_W * 0.2, ROOM_H * 0.85));
        // Kate unarmed: under the alarm she would fire at the target too.
        game.issue(1, Gear::default());
        game.recruit_for_probe(0, true);
        game.set_hostiles(vec![Some((
            james + vec2(3.0 * TILE, 0.0),
            WeaponKind::LaserPistol,
        ))]);
        assert!(game.send_for_probe(0, james + vec2(8.0 * TILE, 0.0)));
        // A bolt in the air is one that was fired: more of them after a
        // step than before it, while he walks, is a shot on the move.
        let mut fired_walking = false;
        for _ in 0..(60 * 3) {
            let before = game.bolts_in_flight();
            game.simulate(DT);
            if game.is_walking(0) && game.bolts_in_flight() > before {
                fired_walking = true;
            }
        }
        assert!(!fired_walking, "a crew member on the move holds its fire");
    }

    #[test]
    fn a_burst_is_eight_shots_in_two_seconds_and_then_a_gap() {
        let mut game = room();
        game.set_autonomous(false);
        let kate = game.put_for_probe(1, vec2(ROOM_W * 0.35, ROOM_H * 0.5));
        game.put_for_probe(0, vec2(ROOM_W * 0.45, ROOM_H * 0.8));
        // Kate with the rifle, in a hostile room so every shot is
        // recorded rather than flown; James out of the fight.
        game.issue(
            1,
            Gear {
                weapon: Some(WeaponKind::AutoRifle),
                ..Gear::default()
            },
        );
        game.issue(0, Gear::default());
        assert_eq!(game.weapon(1), Some(WeaponKind::AutoRifle));
        game.set_hostile_bodies(true);
        let target = kate + vec2(4.0 * TILE, 0.0);
        game.set_hostiles(vec![Some((target, WeaponKind::LaserPistol))]);
        // Six seconds to walk to her stand — a burst a walk interrupts is
        // over — then every shot timed: from a trigger pull, eight within
        // two seconds, nothing for the next two, and the trigger again.
        for _ in 0..(60 * 6) {
            game.simulate(DT);
        }
        game.take_shots();
        let mut times = Vec::new();
        for step in 0..(60 * 8) {
            game.simulate(DT);
            let t = step as f32 * DT;
            times.extend(game.take_shots().into_iter().map(|_| t));
        }
        let t0 = times[0];
        let within = |lo: f32, hi: f32| {
            times
                .iter()
                .filter(|&&t| t >= t0 + lo && t < t0 + hi)
                .count()
        };
        assert_eq!(within(0.0, 2.0), 8, "eight in two seconds: {times:?}");
        assert_eq!(within(2.0, 4.0), 0, "then the recharge: {times:?}");
        assert!(within(3.9, 6.0) >= 7, "and again: {times:?}");
        assert!(game.take_hits().is_empty());
    }

    #[test]
    fn a_blade_within_reach_locks_the_gunner_who_stops_firing_and_lands_fists() {
        let mut game = room();
        game.set_autonomous(false);
        let james = game.put_for_probe(0, vec2(ROOM_W * 0.45, ROOM_H * 0.5));
        game.put_for_probe(1, vec2(ROOM_W * 0.25, ROOM_H * 0.8));
        // Kate unarmed, or the alarm has her shooting the target too and
        // her hits are counted with his fists.
        game.issue(1, Gear::default());
        game.recruit_for_probe(0, true);
        // A pistol beside him is a target; a blade beside him is a fight.
        let beside = james + vec2(TILE, 0.0);
        game.set_hostiles(vec![Some((beside, WeaponKind::LaserPistol))]);
        for _ in 0..30 {
            game.simulate(DT);
        }
        assert_eq!(game.is_locked(0), None);
        assert!(
            game.bolts_in_flight() > 0 || !game.take_hits().is_empty(),
            "shooting"
        );
        game.set_hostiles(vec![Some((beside, WeaponKind::Schword))]);
        game.simulate(DT);
        assert_eq!(game.is_locked(0), Some(0), "locked");
        game.take_hits();
        let flying = game.bolts_in_flight();
        let mut blows = Vec::new();
        for _ in 0..(60 * 5) {
            game.simulate(DT);
            blows.extend(game.take_hits());
        }
        assert!(
            game.bolts_in_flight() <= flying,
            "nothing fired while locked"
        );
        assert!(
            blows.len() >= 2 && blows.len() <= 3,
            "a fist every two seconds: {}",
            blows.len()
        );
        assert!(
            blows
                .iter()
                .all(|h| h.who == 0 && h.damage == FIST_DAMAGE && !h.cut)
        );
        // Out of reach, the lock ends and the shooting starts again.
        game.set_hostiles(vec![Some((
            james + vec2(4.0 * TILE, 0.0),
            WeaponKind::Schword,
        ))]);
        game.simulate(DT);
        assert_eq!(game.is_locked(0), None);
        for _ in 0..60 {
            game.simulate(DT);
        }
        assert!(game.bolts_in_flight() > 0 || !game.take_hits().is_empty());
    }

    #[test]
    fn a_blade_enemy_charges_and_its_blow_is_a_cut_that_splashes_the_deck() {
        let mut game = room();
        game.set_autonomous(false);
        let kate = game.put_for_probe(1, vec2(ROOM_W * 0.30, ROOM_H * 0.5));
        game.put_for_probe(0, vec2(ROOM_W * 0.45, ROOM_H * 0.85));
        game.issue(
            1,
            Gear {
                weapon: Some(WeaponKind::Schword),
                ..Gear::default()
            },
        );
        game.issue(0, Gear::default());
        game.set_hostile_bodies(true);
        // A target five tiles off in the open: a blade goes to it.
        let target = kate + vec2(5.0 * TILE, 0.0);
        game.set_hostiles(vec![Some((target, WeaponKind::LaserPistol))]);
        let mut swings = Vec::new();
        for _ in 0..(60 * 12) {
            game.simulate(DT);
            swings.extend(game.take_shots());
            if swings.len() >= 2 {
                break;
            }
        }
        assert!(
            (game.bim_pos(1) - target).len() <= MELEE_RANGE * TILE + 1.0,
            "charged to within reach: {:?} of {:?}",
            game.bim_pos(1),
            target
        );
        assert_eq!(game.is_locked(1), Some(0));
        assert!(swings.len() >= 2, "swung: {}", swings.len());
        assert!(
            swings
                .iter()
                .all(|s| s.melee && s.cut && s.damage == 35.0 && s.at == target)
        );
        assert_eq!(game.bolts_in_flight(), 0, "nothing flies from a blade");

        // The blow, carried into the crew's room by the world: within
        // reach it lands as a cut — three units, and blood over the deck
        // round the body — and from out of reach it does nothing.
        let mut crew = room();
        crew.set_autonomous(false);
        let james = crew.put_for_probe(0, vec2(ROOM_W * 0.5, ROOM_H * 0.5));
        crew.put_for_probe(1, vec2(ROOM_W * 0.2, ROOM_H * 0.8));
        assert!(!crew.enemy_strike(james + vec2(4.0 * TILE, 0.0), 0, 35.0, true));
        assert_eq!(crew.bleeding(0), 0);
        assert!(crew.enemy_strike(james + vec2(TILE, 0.0), 0, 12.0, true));
        assert_eq!(crew.bleeding(0), 3, "a cut is three wounds");
        let said = crew.take_wounds_taken();
        assert_eq!(said.len(), 1);
        assert!(said[0].cut && said[0].who == 0);
        let bloody = (-1..=1)
            .flat_map(|dx| (-1..=1).map(move |dy| (dx, dy)))
            .filter(|&(dx, dy)| {
                let at = james + vec2(dx as f32 * TILE, dy as f32 * TILE);
                crew.room.filth.kind_at(at) == filth::Mess::Blood
            })
            .count();
        assert!((3..=5).contains(&bloody), "{bloody} tiles splashed");
    }

    /// The crew's alarm: an enemy within `ALARM_RANGE` of anybody puts
    /// every crew member but the player's under arms — Kate draws the
    /// pistol out of her pack and shoots what she can see — and nobody
    /// near stands her down again; a crew member hit with nobody near
    /// raises it for `ALARM_HOLD`. James is the player's and is left as
    /// he is throughout.
    #[test]
    fn the_crew_take_arms_when_an_enemy_comes_within_range_and_stand_down_after() {
        let mut game = room();
        game.set_autonomous(false);
        let kate = game.put_for_probe(1, vec2(ROOM_W * 0.35, ROOM_H * 0.5));
        game.put_for_probe(0, vec2(ROOM_W * 0.7, ROOM_H * 0.85));
        // Her pistol in the pack, not in the hand: the alarm equips it.
        let mut gear = Gear::default();
        gear.pack[2] = Some(Item::Weapon(WeaponKind::LaserPistol));
        game.issue(1, gear);
        assert!(game.weapon(1).is_none());
        // A target far off: nothing.
        // Far from everybody: James stands nearer it than she does.
        let far = kate + vec2((ALARM_RANGE + 20.0) * TILE, 0.0);
        game.set_hostiles(vec![Some((far, WeaponKind::LaserPistol))]);
        for _ in 0..10 {
            game.simulate(DT);
        }
        assert!(!game.is_alarmed());
        assert!(!game.bims[1].character.is_recruited());
        assert!(!game.is_recruited(), "James untouched");
        // Within range and in view: to arms, the pistol out of the pack,
        // and shots at it.
        let near = kate + vec2(4.0 * TILE, 0.0);
        game.set_hostiles(vec![Some((near, WeaponKind::LaserPistol))]);
        let mut shot = false;
        for _ in 0..(60 * 3) {
            game.simulate(DT);
            shot |= game.bolts_in_flight() > 0;
        }
        assert!(game.is_alarmed());
        assert!(game.bims[1].character.is_recruited(), "Kate under arms");
        assert_eq!(
            game.weapon(1),
            Some(WeaponKind::LaserPistol),
            "drawn from the pack"
        );
        assert!(shot, "and shooting");
        assert!(!game.is_recruited(), "James is the player's");
        // Gone: stood down.
        game.set_hostiles(Vec::new());
        game.take_hits();
        for _ in 0..(60 * 2) {
            game.simulate(DT);
        }
        assert!(!game.is_alarmed());
        assert!(
            !game.bims[1].character.is_recruited(),
            "back to her errands"
        );
        // Hit with nobody in sight: the alarm, for a while.
        let here = game.bim_pos(1);
        assert!(game.enemy_strike(here + vec2(TILE, 0.0), 1, 5.0, false));
        game.simulate(DT);
        assert!(game.is_alarmed());
        assert!(game.bims[1].character.is_recruited());
        for _ in 0..((ALARM_HOLD / DT) as usize + 2) {
            game.simulate(DT);
        }
        assert!(!game.is_alarmed(), "the hold ran out");
    }

    /// Under the alarm a crew member that sees no enemy keeps to the
    /// player's side — walks into its slot beside James and stays there
    /// as he moves — and takes the player's orders, which it does not
    /// otherwise: an order holds it where it was sent until the alarm is
    /// over, when its post and its recruitment go together.
    #[test]
    fn under_the_alarm_the_crew_gather_round_the_player_and_take_orders() {
        let mut game = room();
        game.set_autonomous(false);
        let james = game.put_for_probe(0, vec2(ROOM_W * 0.3, ROOM_H * 0.5));
        let kate = game.put_for_probe(1, vec2(ROOM_W * 0.85, ROOM_H * 0.85));
        // Kate cannot be ordered about in peace.
        game.drag_begin(kate.x, kate.y);
        game.drag_end(kate.x, kate.y);
        assert_eq!(game.selected(), Some(1));
        assert_eq!(game.order_move(james.x, james.y), ORDER_IGNORED);
        // An enemy within range but out of sight — beyond the room's walls
        // — is the alarm without a target to act on.
        let unseen = james + vec2(30.0 * TILE, 0.0);
        game.set_hostiles(vec![Some((unseen, WeaponKind::LaserPistol))]);
        game.simulate(DT);
        game.observe();
        assert!(
            !game.combat.sees_any(&game.room.sight, kate),
            "out of sight"
        );
        for _ in 0..(60 * 20) {
            game.simulate(DT);
            if !game.is_walking(1) && (game.bim_pos(1) - game.bim_pos(0)).len() < 3.0 * TILE {
                break;
            }
        }
        assert!(game.is_alarmed());
        // James wanders in the classic room — the chat walks him about — so
        // it is where he is now that counts.
        assert!(
            (game.bim_pos(1) - game.bim_pos(0)).len() < 3.0 * TILE,
            "gathered beside James: {:?} of {:?}",
            game.bim_pos(1),
            game.bim_pos(0)
        );
        assert_eq!(game.bolts_in_flight(), 0, "nothing to shoot at");
        // James moves; the ring follows.
        game.put_for_probe(0, vec2(ROOM_W * 0.6, ROOM_H * 0.3));
        for _ in 0..(60 * 20) {
            game.simulate(DT);
            if !game.is_walking(1) && (game.bim_pos(1) - game.bim_pos(0)).len() < 3.0 * TILE {
                break;
            }
        }
        assert!(
            (game.bim_pos(1) - game.bim_pos(0)).len() < 3.0 * TILE,
            "followed"
        );
        // Ordered somewhere, she goes and holds it.
        let at = game.bim_pos(1);
        game.drag_begin(at.x, at.y);
        game.drag_end(at.x, at.y);
        assert_eq!(game.selected(), Some(1));
        // Somewhere on the open deck, away from the heads and their door.
        let spot = vec2(ROOM_W * 0.5, ROOM_H * 0.5);
        // Through the heads' door if her slot was inside them.
        let code = game.order_move(spot.x, spot.y);
        assert!(code == ORDER_MOVING || code == ORDER_VIA_DOOR, "{code}");
        for _ in 0..(60 * 20) {
            game.simulate(DT);
        }
        assert!(
            (game.bim_pos(1) - spot).len() < 2.0 * TILE,
            "holds the spot"
        );
        assert!(game.bims[1].character.post().is_some());
        // The alarm over, the post goes with it and she is her own again.
        game.set_hostiles(Vec::new());
        game.simulate(DT);
        assert!(!game.is_alarmed());
        assert!(game.bims[1].character.post().is_none());
        assert!(!game.bims[1].character.is_recruited());
        assert_eq!(game.order_move(james.x, james.y), ORDER_IGNORED);
    }

    /// A swing is not a hit until it has been swung: the blow lands
    /// `SWING_TIME` after the lock forms, not the step it does, and a
    /// target that stepped out of reach in the meantime is missed — the
    /// swing goes on, the animation and the cooldown with it, on nobody.
    #[test]
    fn a_blow_lands_when_the_swing_ends_and_misses_a_target_that_stepped_back() {
        let mut game = room();
        game.set_autonomous(false);
        let kate = game.put_for_probe(1, vec2(ROOM_W * 0.30, ROOM_H * 0.5));
        game.put_for_probe(0, vec2(ROOM_W * 0.45, ROOM_H * 0.85));
        game.issue(
            1,
            Gear {
                weapon: Some(WeaponKind::Schword),
                ..Gear::default()
            },
        );
        game.recruit_for_probe(1, true);
        let beside = kate + vec2(TILE, 0.0);
        game.set_hostiles(vec![Some((beside, WeaponKind::LaserPistol))]);
        game.simulate(DT);
        assert_eq!(game.is_locked(1), Some(0), "locked");
        assert!(game.take_hits().is_empty(), "the swing has only begun");
        // Most of the swing: nothing has landed yet.
        let steps = (SWING_TIME / DT) as usize;
        for _ in 0..(steps - 2) {
            game.simulate(DT);
        }
        assert!(game.take_hits().is_empty(), "still swinging");
        // The end of it: the cut lands.
        let mut landed = Vec::new();
        for _ in 0..4 {
            game.simulate(DT);
            landed.extend(game.take_hits());
        }
        assert_eq!(landed.len(), 1, "one blow as the swing ends");
        assert!(landed[0].who == 0 && landed[0].cut && landed[0].damage == 35.0);

        // The next swing begins at MELEE_PERIOD; the target steps back
        // during it, and the blow lands on nobody. Run until it starts —
        // nothing lands between swings — and step back the moment it has.
        let mut waited = 0;
        while game.bims[1].blow.is_none() {
            game.simulate(DT);
            assert!(game.take_hits().is_empty(), "nothing between swings");
            waited += 1;
            assert!(waited <= (MELEE_PERIOD / DT) as usize, "a second swing");
        }
        game.set_hostiles(vec![Some((
            kate + vec2(4.0 * TILE, 0.0),
            WeaponKind::LaserPistol,
        ))]);
        for _ in 0..(steps + 4) {
            game.simulate(DT);
            assert!(game.take_hits().is_empty(), "a swing at nobody");
        }
        assert!(game.bims[1].blow.is_none(), "and it is over");
        assert_eq!(game.is_locked(1), None);
    }

    #[test]
    fn a_vest_takes_a_hit_first_and_its_protection_lifts_when_it_breaks() {
        let mut game = room();
        game.set_autonomous(false);
        game.put_for_probe(0, vec2(ROOM_W * 0.45, ROOM_H * 0.5));
        let vest = Piece::new(7, ArmourKind::BasicKevlar);
        assert!(game.give(0, None, Item::Armour(vest)));
        assert!(game.equip(0, 0).is_none(), "nothing was worn before");
        assert_eq!(game.worn(0, Part::Body), Some(vest));
        assert_eq!(game.armour_health(0), 20.0);
        assert_eq!(game.part_bonus(0, Part::Body), 20.0);
        assert_eq!(game.part_bonus(0, Part::Head), 0.0);
        assert!(game.pack(0).iter().all(|c| c.is_none()));

        // Two points of protection off, then the vest takes the rest:
        // fifteen leaves the Bim untouched and the vest at seven.
        let out = game.wound(0, Part::Body, 15.0);
        assert_eq!(
            out,
            WoundOutcome {
                leg_lost: false,
                absorbed: 15.0,
                through: 0.0,
                piece_broke: false,
            }
        );
        assert_eq!(game.worn(0, Part::Body).unwrap().health, 7.0);
        assert_eq!(game.part_health(0, Part::Body), Part::Body.max());
        assert_eq!(game.wounds(0, Part::Body), 0, "no wound opened");
        assert_eq!(game.armour_health(0), 7.0);

        // No more than the protection is no hit at all.
        let out = game.wound(0, Part::Body, 2.0);
        assert_eq!(out.absorbed, 2.0);
        assert_eq!(game.worn(0, Part::Body).unwrap().health, 7.0);
        assert_eq!(game.wounds(0, Part::Body), 0);

        // Twelve: two off, seven into the vest, three through. The vest is
        // broken — still worn, doing nothing — and the world is told.
        let out = game.wound(0, Part::Body, 12.0);
        assert_eq!(out.absorbed, 9.0);
        assert_eq!(out.through, 3.0);
        assert!(out.piece_broke);
        let worn = game.worn(0, Part::Body).unwrap();
        assert!(worn.broken());
        assert_eq!(worn.id, 7, "the same piece");
        assert_eq!(game.part_health(0, Part::Body), Part::Body.max() - 3.0);
        assert_eq!(game.wounds(0, Part::Body), 1);
        assert_eq!(game.armour_health(0), 0.0);
        assert_eq!(
            game.take_pieces_broken(),
            vec![(0, ArmourKind::BasicKevlar)]
        );
        assert!(game.take_pieces_broken().is_empty(), "drained");

        // Broken, it neither protects nor takes: the whole shot goes in.
        let out = game.wound(0, Part::Body, 12.0);
        assert_eq!(out.absorbed, 0.0);
        assert_eq!(out.through, 12.0);
        assert!(!out.piece_broke);
        assert_eq!(game.part_health(0, Part::Body), Part::Body.max() - 15.0);

        // A part with nothing on it is hit as before.
        let out = game.wound(0, Part::Legs, 12.0);
        assert_eq!(out.through, 12.0);
        assert_eq!(game.part_health(0, Part::Legs), Part::Legs.max() - 12.0);
    }

    #[test]
    fn equipping_swaps_with_what_is_worn_and_a_weapon_with_the_weapon() {
        let mut game = room();
        let helm = Piece::new(1, ArmourKind::BasicHelm);
        let other = Piece {
            health: 4.0,
            ..Piece::new(2, ArmourKind::BasicHelm)
        };
        assert!(game.give(0, Some(4), Item::Armour(helm)));
        assert!(game.give(0, None, Item::Armour(other)));
        assert_eq!(
            game.pack(0)[0],
            Some(Item::Armour(other)),
            "first free cell"
        );
        assert_eq!(game.equip(0, 4), None);
        assert_eq!(game.worn(0, Part::Head), Some(helm));
        assert_eq!(game.pack(0)[4], None);
        // The other goes on and the first comes back into its cell, its
        // damage with it.
        assert_eq!(game.equip(0, 0), Some(Item::Armour(helm)));
        assert_eq!(game.worn(0, Part::Head), Some(other));
        assert_eq!(game.pack(0)[0], Some(Item::Armour(helm)));
        assert_eq!(game.armour_health(0), 4.0);
        // Off again, into the first free cell.
        assert!(game.unequip(0, Part::Head));
        assert_eq!(game.worn(0, Part::Head), None);
        assert_eq!(game.pack(0)[1], Some(Item::Armour(other)));
        assert!(!game.unequip(0, Part::Head), "nothing there now");

        // A weapon swaps with the hand; a stack is refused and stays put.
        assert!(game.give(0, Some(8), Item::Weapon(WeaponKind::LaserPistol)));
        assert_eq!(
            game.equip(0, 8),
            Some(Item::Weapon(WeaponKind::LaserPistol))
        );
        assert_eq!(game.gear(0).weapon, Some(WeaponKind::LaserPistol));
        assert!(game.give(0, Some(5), Item::Stack(14)));
        assert_eq!(game.equip(0, 5), None);
        assert_eq!(game.pack(0)[5], Some(Item::Stack(14)));
        assert_eq!(game.equip(0, 6), None, "an empty cell");
        assert_eq!(game.equip(0, 99), None, "no such cell");
    }

    #[test]
    fn give_and_take_round_trip_and_a_broken_piece_is_only_ever_discarded() {
        let mut game = room();
        let legs = Piece {
            health: 3.5,
            ..Piece::new(9, ArmourKind::BasicLegs)
        };
        assert!(game.give(0, Some(2), Item::Armour(legs)));
        assert!(!game.give(0, Some(2), Item::Stack(1)), "the cell is full");
        assert_eq!(game.take(0, 2), Some(Item::Armour(legs)), "damage kept");
        assert_eq!(game.take(0, 2), None, "gone");
        // Nine in, the tenth has nowhere to go.
        for i in 0..PACK_CELLS {
            assert!(game.give(0, None, Item::Stack(i as u32)));
        }
        assert!(!game.give(0, None, Item::Stack(9)), "full");
        for i in 0..PACK_CELLS {
            assert_eq!(game.take(0, i), Some(Item::Stack(i as u32)));
        }

        let broken = Piece {
            health: 0.0,
            ..Piece::new(10, ArmourKind::BasicHelm)
        };
        assert!(game.give(0, Some(0), Item::Armour(broken)));
        assert_eq!(game.take(0, 0), None, "a broken piece is not stowed");
        assert_eq!(game.pack(0)[0], Some(Item::Armour(broken)), "and stays");
        assert!(game.discard(0, 0));
        assert_eq!(game.pack(0)[0], None);
        assert!(!game.discard(0, 0), "nothing to throw away");
    }

    #[test]
    fn a_click_on_a_bim_is_the_bim() {
        let mut game = room();
        let at = game.put_for_probe(1, vec2(ROOM_W * 0.5, ROOM_H * 0.5));
        assert_eq!(game.hit_at(at.x + 4.0, at.y - 3.0), HIT_BIM);
        assert_eq!(game.hit_bim(), 1);
        assert_eq!(game.hit_at(at.x + 60.0, at.y), HIT_NONE);
    }

    /// Bleed a Bim out until it lies there, by the wounds and the clock,
    /// the way the knock-out test does.
    fn knock_out(game: &mut Game, who: usize) {
        for _ in 0..10 {
            game.wound(who, Part::Body, 1.0);
        }
        let mut minutes = 0.0;
        while !game.is_unconscious(who) && minutes < 60.0 {
            game.simulate(1.0);
            minutes += MINUTES_PER_SECOND;
        }
        assert!(game.is_unconscious(who), "out cold");
    }

    #[test]
    fn a_dead_bim_under_the_click_is_a_body_and_an_unconscious_one_is_still_the_bim() {
        let mut game = room();
        game.set_autonomous(false);
        let at = game.put_for_probe(1, vec2(ROOM_W * 0.5, ROOM_H * 0.5));
        game.put_for_probe(0, vec2(ROOM_W * 0.2, ROOM_H * 0.8));
        assert!(!game.is_down(1));
        // Shot through the head: dead on the next step, and a body.
        game.wound(1, Part::Head, 1000.0);
        game.simulate(DT);
        assert!(!game.is_alive(1));
        assert!(game.is_down(1));
        assert_eq!(game.hit_at(at.x + 4.0, at.y - 3.0), HIT_BODY);
        assert_eq!(game.hit_body(), 1);
        assert_eq!(game.hit_at(at.x + 60.0, at.y), HIT_NONE);
        // Out cold is down too, but still the crewmate: the menu on it has
        // the bandages as well as the looting.
        game.put_for_probe(0, vec2(ROOM_W * 0.5, ROOM_H * 0.3));
        knock_out(&mut game, 0);
        let at = game.bims[0].character.pos;
        assert!(game.is_alive(0));
        assert!(game.is_down(0));
        assert_eq!(game.hit_at(at.x + 4.0, at.y - 3.0), HIT_BIM);
        assert_eq!(game.hit_bim(), 0);
    }

    #[test]
    fn looting_strips_a_worn_helm_with_its_damage_and_an_awake_bim_is_refused() {
        let mut game = room();
        game.set_autonomous(false);
        game.put_for_probe(0, vec2(ROOM_W * 0.45, ROOM_H * 0.5));
        let helm = Piece {
            health: 6.0,
            ..Piece::new(7, ArmourKind::BasicHelm)
        };
        assert!(game.give(0, Some(0), Item::Armour(helm)));
        assert_eq!(game.equip(0, 0), None);
        assert!(game.give(0, Some(4), Item::Stack(14)));
        assert_eq!(game.gear(0).weapon, Some(WeaponKind::LaserPistol));
        // The window shows the pack, the worn pieces and the hand.
        let cells = game.loot_cells(0);
        assert_eq!(cells[4], Some(Item::Stack(14)));
        assert_eq!(
            cells[LootCell::Head.code() as usize],
            Some(Item::Armour(helm))
        );
        assert_eq!(cells[LootCell::Body.code() as usize], None);
        assert_eq!(
            cells[LootCell::Weapon.code() as usize],
            Some(Item::Weapon(WeaponKind::LaserPistol))
        );
        // Alive and awake, nothing comes off it.
        assert!(!game.is_down(0));
        assert_eq!(game.take_from_body(0, LootCell::Head), None);
        assert_eq!(game.take_from_body(0, LootCell::Pack(4)), None);
        assert_eq!(game.worn(0, Part::Head), Some(helm), "still worn");
        // Dead, everything does: the helm with its damage, the hand, the
        // pack cell, and each only once.
        game.wound(0, Part::Body, 1000.0);
        game.simulate(DT);
        assert!(game.is_down(0));
        assert_eq!(
            game.take_from_body(0, LootCell::Head),
            Some(Item::Armour(helm))
        );
        assert_eq!(game.worn(0, Part::Head), None, "stripped");
        assert_eq!(game.take_from_body(0, LootCell::Head), None, "bare now");
        assert_eq!(
            game.take_from_body(0, LootCell::Weapon),
            Some(Item::Weapon(WeaponKind::LaserPistol))
        );
        assert_eq!(game.weapon(0), None);
        assert_eq!(
            game.take_from_body(0, LootCell::Pack(4)),
            Some(Item::Stack(14))
        );
        assert_eq!(game.pack(0)[4], None);
        assert_eq!(game.take_from_body(0, LootCell::Pack(4)), None);
        assert_eq!(
            game.take_from_body(0, LootCell::Pack(99)),
            None,
            "no such cell"
        );
        assert!(game.loot_cells(0).iter().all(|c| c.is_none()), "bare");
        // The codes round-trip, and the window's thirteen are all of them.
        for code in 0..LOOT_CELLS as u32 {
            assert_eq!(LootCell::from_code(code).map(|c| c.code()), Some(code));
        }
        assert_eq!(LootCell::from_code(LOOT_CELLS as u32), None);
        assert_eq!(LootCell::Pack(8).code(), 8);
        assert_eq!(LootCell::Weapon.code(), 12);
    }

    #[test]
    fn a_visitor_marked_down_is_a_body_and_one_on_its_feet_is_not() {
        let mut game = room();
        game.set_autonomous(false);
        // The crew out of the way, and two of the station's people on the
        // deck: one down, one not.
        for who in 0..2 {
            game.put_for_probe(who, vec2(ROOM_W * 0.15, ROOM_H * 0.85));
        }
        let up = vec2(ROOM_W * 0.5, ROOM_H * 0.5);
        let down = up + vec2(120.0, 0.0);
        game.set_visitors(vec![up, down]);
        assert_eq!(
            game.hit_at(down.x + 4.0, down.y),
            HIT_NONE,
            "nobody said so"
        );
        game.set_visitors_down(&[false, true]);
        assert_eq!(game.hit_at(down.x + 4.0, down.y), HIT_VISITOR);
        assert_eq!(game.hit_visitor(), 1);
        assert_eq!(game.hit_at(up.x, up.y + 3.0), HIT_NONE, "on its feet");
        // A crewmate standing over the body is the crewmate.
        let at = game.put_for_probe(1, down);
        assert_eq!(game.hit_at(at.x, at.y), HIT_BIM);
        // The visitors set again is nobody down until the world says so.
        game.set_visitors(vec![up, down]);
        game.put_for_probe(1, vec2(ROOM_W * 0.15, ROOM_H * 0.85));
        assert_eq!(game.hit_at(down.x + 4.0, down.y), HIT_NONE);
    }
}
