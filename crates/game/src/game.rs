//! World state: the room, the Bim in it, the job it is doing, and the
//! per-frame draw list handed to the renderer.

use crate::bim::{
    Bim, CREW, GROUND_SLEEP, GROUND_WINDOW, PLAYER, SORE_LASTS, TALKS_ABOUT, TRAIL_LIFE,
};
use crate::character::{
    ACCENT, Action, BODY_MARGIN, FallBack, Held, Look, Outfit, PICK_RADIUS, SWING_TIME, Tint, Worn,
};
use crate::clock::MINUTES_PER_SECOND;
use crate::clock::{self, Clock};
use crate::combat::{
    ArmourKind, Blow, COVER_WORTH, Combat, FIST_DAMAGE, Gear, Grenade, Hit, Item, LOOT_CELLS,
    LootCell, MELEE_PERIOD, MELEE_RANGE, PACK_CELLS, Piece, Sentry, Shot, Skill, Tactics, Weapon,
    WeaponStats,
};
use crate::cue::{Cue, Cued};
use crate::door;
use crate::draw::{Color, DrawList};
use crate::droid::{Droid, DroidPart};
use crate::filth;
use crate::health::{Beamed, Doctoring, Health, Lasting, Malnutrition, Part, Trauma};
use crate::hydro;
use crate::manager::{Manager, Stock};
use crate::math::{Rect, TAU, Vec2, clamp, vec2};
use crate::memory::What;
use crate::nav::{self, Maps, Nav};
use crate::needs::{Need, Urge};
use crate::rng::Rng;
use crate::room::{
    self, Dish, Dropped, GLOW, HIT_BIM, HIT_BODY, HIT_DROPPED, HIT_NONE, HIT_VISITOR, ROOM_H,
    ROOM_W, Room, Switch, TILE, WARN,
};
use crate::schedule::{IGNORE_ABOVE, Schedule, Slot, WAKE_AT};
use crate::sight::{Fog, Sight, Stance};
use crate::social;
use crate::task::{self, Kind, SLEEP_MINUTES, Saved, Task};
use crate::work::{self, Job, Priorities};

const TRAIL: Color = ACCENT;

/// A box of dressings as a pack item (feature 87): what a Bim binds a
/// wound with, and the one thing in a pack that stacks.
pub const BANDAGE: Item = Item::Stack(crate::combat::BANDAGE_CODE);
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

/// Where the gun of a body going out cold lands, in room units from the body:
/// out past the hand lying along its right side, off the figure — the
/// fallen one is stretched along its heading (`Character::draw_flat`),
/// so ahead of the body is under its head.
const DROP_FLUNG: Vec2 = vec2(-14.0, 44.0);

/// A dressing or a treatment finished (feature 76): whose hands, on
/// whom, and what was used — a bandage, a medkit, or nothing at all (a
/// medic's field surgery). For the world to give the medic its
/// experience by (`Game::take_healings`).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Healed {
    pub helper: usize,
    pub patient: usize,
    pub with: Healing,
}

/// What a [`Healed`] used.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Healing {
    Bandage,
    Medkit,
    Bare,
}

/// What a commander's squad order tells one body to do (feature 78,
/// `world::commander`): the world's word, said afresh every step for
/// every Bim, and never kept in a save — the order itself is the
/// world's. [`Squad::None`] for a body under none, which is everybody
/// until a commander says otherwise, and **always** a body a player
/// steers: an order never moves, holds or aims one of those.
#[derive(Clone, Copy, PartialEq, Debug, Default)]
pub enum Squad {
    #[default]
    None,
    /// Fire on that target ahead of any nearer one, and advance on it
    /// the way a body that sees an enemy for itself does. `seen` is
    /// whether it is a sighting rather than a belief: a mark nobody can
    /// see is walked towards and never fired at.
    Attack { enemy: usize, seen: bool },
    /// Walk to a slot round that point, holding fire on the way, and
    /// hold there shooting what can be seen.
    FallBack { at: Vec2 },
    /// Hold exactly where it stands, shooting what it can see: no walk
    /// to cover, and no running.
    StandGround,
}

/// What a player's **standing order** tells the bots that follow them to
/// do (feature 84, `world::Standing`): one a player slot, the world's word,
/// said afresh every step. [`Standing::Follow`] until a player says
/// otherwise, and what every slot goes back to when the order is given
/// again.
///
/// Whose order a bot is under is whose Bim it is nearest
/// ([`Game::orders_for`]): with one player that is the one order there
/// is, and with several each player leads the bots about them.
///
/// None of this reaches a Bim a player steers, a body on a chain, a body
/// running for its life or one holding a post its player clicked for it:
/// an order to one crew member is that crew member's and outranks the
/// standing order to the rest.
#[derive(Clone, Copy, PartialEq, Debug, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Standing {
    /// Keep to the player's side — the ring round their Bim — and pick
    /// your own stand the moment you see an enemy for yourself.
    #[default]
    Follow,
    /// Fight your way to that point of the room and hold it: cover on
    /// the way where there is any, a stand of your own while anything is
    /// in your weapon's reach, and a push on towards it when there is
    /// not.
    Attack { at: Vec2 },
    /// Back to the ship, and hold there. The ship is where a dying body
    /// runs to anyway, and where the last stand is made.
    Retreat,
}

/// How a shot on a body went — see [`Game::wound`]. All nought for a shot
/// on nobody: a dead Bim, or one that is not there.
#[derive(Clone, Copy, PartialEq, Debug, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct WoundOutcome {
    /// The shot took the part to nothing, and this is the dying state it
    /// rolled — see `Health::shot`.
    pub trauma: Option<Trauma>,
    /// The shot took a leg: the trauma is a crushed one.
    pub leg_lost: bool,
    /// What the armour on the part took off it, its protection included.
    pub absorbed: f32,
    /// What reached the body and opened a wound.
    pub through: f32,
    /// The piece on the part is broken by this shot.
    pub piece_broke: bool,
}

/// A container the crew can reach into, for [`Game::container_spot`]: a
/// workstation (the armoury among them), a shelf, a cold store or a
/// research desk, each by its index in the room's list of that kind.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Container {
    Bench(usize),
    Shelf(usize),
    Fridge(usize),
    /// A research desk: the slot a research key sits in.
    Desk(usize),
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
/// the way an enemy's people do. Thirty tiles is a compartment or two
/// away — half again the room's `CALM_RANGE`, so a crewmate between the
/// two is under arms and doctors — and an enemy **in sight** at any range
/// is the alarm too (`enemy_unseen_for`). It used to be a hundred, any
/// enemy on the same station, and a crew docked at an enemy's stood under
/// arms for as long as they were tied up there, whoever was left alive at
/// the far end of it: nobody ate, nobody slept. The alarm is a fight, not a
/// berth.
pub const ALARM_RANGE: f32 = 30.0;
/// How long the alarm holds after a crew member is hit, or after the last
/// enemy was in anybody's sight, with none in range, in seconds at 1x.
pub const ALARM_HOLD: f32 = 30.0;
/// How long an enemy's people go on believing a crew member is where one
/// of them last saw it, in seconds, once nobody sees it any more. They
/// chase that spot for this long and then give the hunt up.
pub const FORGET_AFTER: f32 = 60.0;
/// How long a room has to have been **calm**, in seconds at 1x: no shot
/// fired and no blow landed here, either side's, and no enemy in any of
/// its people's sight, for this long — and none within [`CALM_RANGE`]
/// right now. In the calm anybody may doctor any crewmate; in a fight
/// only a crewmate lying out of harm is gone to (`Game::out_of_harm`),
/// and a helper's own wound is not waited for at all: it dresses that
/// whenever it has nothing in its own sight. See `Game::calm` and
/// `Game::medical_on_offer`.
pub const CALM_AFTER: f32 = 20.0;
/// How near an enemy may be, in tiles, for the room to count as calm.
pub const CALM_RANGE: f32 = 20.0;
/// How near a crewmate walking over with a bandage or a kit has to be,
/// in tiles, for a Bim running from the fight to stop and be doctored.
/// See `Game::is_fleeing`.
pub const HELPER_NEAR: f32 = 3.0;
/// How far the eyes at a hostile station's airlock reach, in tiles about
/// the tile just inside its door: a crew member coming through it is
/// seen whether or not one of the station's people is looking, so a
/// boarding is what puts them at war. See `Game::set_watched`.
pub const AIRLOCK_WATCH: f32 = 2.5;
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
/// How near an **attack banner** counts as reached, in tiles (feature
/// 84): inside this, a bot with nothing in its weapon's reach holds the
/// ring round the banner instead of pushing on to stand on it. Two tiles
/// wider than the gather ring itself, so a bot settled in the ring is
/// not shoved on towards the middle of it every plan.
pub const BANNER_HOLD: f32 = 3.0;
/// What a Bim with a crewmate in its arms walks at (feature 86): both
/// arms full, so a little over half pace. The carry is meant to be a
/// choice — the ground given to fetch somebody out is ground the medic
/// has to make good again.
pub const CARRY_PACE: f32 = 0.6;
/// How near a medic has to stand to pick a body up, in tiles. A body's
/// width and a bit: near enough that it is a reach rather than a walk,
/// wide enough that the two are not shoved apart by
/// [`Game::separate_under_arms`] the step before.
pub const CARRY_REACH: f32 = 1.6;
/// How far a **field medic** looks for somebody to fetch out, in tiles
/// (feature 86). About a compartment: it goes for the fallen it can see
/// its way to rather than across the whole station, which is what the
/// rest of the crew are for.
pub const RESCUE_LOOK: f32 = 18.0;
/// How far off the fight a field medic carries a body before it sets it
/// down, in tiles: out of the nearest enemy's sight is the real test
/// (`Game::out_of_harm`), and this is the fallback where nothing can be
/// seen at all.
pub const RESCUE_CLEAR: f32 = 12.0;
/// Where several selected crew go for one right-click, in tiles off the
/// point, in crew order: the point itself, then a ring round it a body's
/// width out, so an order for a squad is a huddle and not a stack.
pub const CLUSTER_SLOTS: [(f32, f32); 9] = [
    (0.0, 0.0),
    (-1.0, 0.0),
    (1.0, 0.0),
    (0.0, -1.0),
    (0.0, 1.0),
    (-1.0, -1.0),
    (1.0, -1.0),
    (-1.0, 1.0),
    (1.0, 1.0),
];

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
/// One door's lock as the world carries it between rooms — see
/// `Game::door_states`.
#[derive(Clone, Copy, PartialEq, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct DoorState {
    /// The middle of the opening, in the room's units.
    pub centre: Vec2,
    pub locked: bool,
    /// Locked from the panel by the crew, rather than by one of the room's
    /// own bodies sealing itself in.
    pub by_crew: bool,
    /// How far a smashing has got, while one is on.
    pub smash: Option<f32>,
    /// The lock changed since the world last looked.
    pub changed: bool,
}

/// Where a body stands to work a door's panel or heave at it: the room's
/// own stand-off, a tile's half out of the opening.
const DOOR_STAND_OFF: f32 = 26.0;
/// Seconds between the wounds an enemy sealed in binds. See
/// `Game::seal_and_bind`.
pub const BIND_EVERY: f32 = 10.0;

/// How long a nod-off lasts, and how often one comes over a Bim that is past
/// the point of staying upright — about one in every three quarters of an hour
/// on its feet, so it spends a fifth of that time out cold.
const NAP_MINUTES: f32 = 15.0;
/// The least of the deck's allowance a Bim with no bunk lies down for, in
/// game minutes: under this it stays up until the window comes round, since
/// a lie-down of two minutes is a walk and a climb for nothing. See
/// `Game::can_sleep_on_ground` and `crate::bim::GROUND_SLEEP`.
const GROUND_SLEEP_MIN: f32 = 15.0;
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
pub const JOB_TREAT: u32 = 23;
pub const JOB_FETCH: u32 = 24;
pub const JOB_EXECUTE: u32 = 25;
pub const JOB_FERRY: u32 = 26;
/// A walk to a spot on the deck waiting its turn — a Shift-click. Only
/// ever on the agenda, never the activity: the walk is given, not run.
pub const JOB_WALK: u32 = 27;
/// Laying an engineer's kit on the deck (feature 74).
pub const JOB_DEPLOY: u32 = 28;

fn job_code(kind: Kind, rest_minutes: f32) -> u32 {
    match kind {
        Kind::Meal(Dish::Stew) => JOB_MEAL,
        Kind::Meal(Dish::Bowl) => JOB_BOWL,
        Kind::Batch => JOB_STEW,
        Kind::Reheat => JOB_REHEAT,
        // Anything longer than the menu's nap is a sleep: a night on the deck
        // is three hours, not six, and is a sleep for all that.
        Kind::Rest if rest_minutes > task::NAP_MINUTES => JOB_SLEEP,
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
        Kind::Build { .. } => JOB_BUILD,
        Kind::Bandage { .. } => JOB_BANDAGE,
        Kind::Treat { .. } => JOB_TREAT,
        Kind::Fetch { .. } => JOB_FETCH,
        Kind::Execute { .. } => JOB_EXECUTE,
        Kind::Ferry { .. } => JOB_FERRY,
        Kind::Walk { .. } => JOB_WALK,
        Kind::Deploy { .. } => JOB_DEPLOY,
    }
}

/// The room after dark. Laid over the finished frame rather than mixed into
/// every colour, so nothing in `room.rs` has to know what time it is.
const NIGHT: Color = Color::rgb(0.03, 0.05, 0.13);
/// How dark it gets at the middle of the night. Short of black: you still
/// want to see the Bim sleeping.
const NIGHT_DEPTH: f32 = 0.46;

/// What a hostile room's people believe about one of the world's targets:
/// where one of them last saw it, what it carries, and how long ago.
#[derive(Clone, Copy, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
struct Seen {
    at: Vec2,
    weapon: Weapon,
    ago: f32,
}

/// What a side of a fight believes about the targets it was handed, and
/// which of those beliefs are stale: a target any of `eyes` can see, or
/// one within [`AIRLOCK_WATCH`] of `watched`, is known where it stands
/// and remembered there; one nobody sees is believed where it was last
/// seen, with the weapon the world says it carries now; one the world no
/// longer names is forgotten outright. `seen` is the side's memory, kept
/// between steps and aged in `simulate`.
///
/// Two sides keep one: a hostile room's people about the crew
/// (`Game::last_seen`), and the machines about their own targets when the
/// world has given them a list of their own (`Game::machine_seen`,
/// feature 94).
fn believe(
    seen: &mut Vec<Option<Seen>>,
    sight: &Sight,
    watched: Option<Vec2>,
    eyes: &[Vec2],
    at: Vec<Option<(Vec2, Weapon)>>,
) -> (Vec<Option<(Vec2, Weapon)>>, Vec<bool>) {
    seen.resize(at.len(), None);
    let mut believed = Vec::with_capacity(at.len());
    let mut stale = Vec::with_capacity(at.len());
    for (i, target) in at.into_iter().enumerate() {
        let mut in_sight = false;
        match target {
            None => seen[i] = None,
            Some((p, weapon)) => {
                in_sight = watched.is_some_and(|w| (w - p).len() <= AIRLOCK_WATCH * TILE)
                    || eyes.iter().any(|&eye| sight.sees_from(eye, p).is_some());
                if in_sight {
                    seen[i] = Some(Seen {
                        at: p,
                        weapon,
                        ago: 0.0,
                    });
                } else if let Some(was) = seen[i].as_mut() {
                    // The weapon it carries is known whether or not it is
                    // in sight — the world says — and the tactics read it.
                    was.weapon = weapon;
                }
            }
        }
        believed.push(seen[i].map(|s| (s.at, s.weapon)));
        stale.push(!in_sight);
    }
    (believed, stale)
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
struct Marker {
    pos: Vec2,
    age: f32,
    /// A place the Bim cannot get to. Drawn in the one warm colour the room
    /// keeps for things worth noticing, so a refused order is not silent.
    bad: bool,
}

/// A dash and the gap after it, in room units, on the thread through the
/// queued walks — see `Game::render`.
const DASH: f32 = 10.0;
const DASH_GAP: f32 = 7.0;

/// A dashed line from `a` to `b`: what a walk not yet walked is drawn as,
/// where a solid one is the line a right-drag is drawing now.
fn dashed(list: &mut DrawList, a: Vec2, b: Vec2, colour: Color) {
    let whole = (b - a).len();
    if whole <= 0.5 {
        return;
    }
    let along = (b - a) * (1.0 / whole);
    let mut at = 0.0;
    while at < whole {
        let end = (at + DASH).min(whole);
        list.line(a + along * at, a + along * end, 2.0, colour);
        at += DASH + DASH_GAP;
    }
}

/// One recipe the world would like made, at one bench: what the room's
/// craft job is offered off. The world hands the room a fresh list every
/// step — see `Game::set_craft_orders` — worked out from the hold, the
/// player's targets and the power; the room decides only who goes and
/// whether the bench is free. `minutes` is how long the recipe takes,
/// carried because the room has no recipe table.
#[derive(Clone, Copy, PartialEq, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Order {
    pub recipe: u32,
    pub bench: usize,
    pub minutes: f32,
    /// The one Bim that may take it, or anybody: an engineer's repair at
    /// the workbench is its own (feature 74).
    pub only: Option<usize>,
}

/// One of the ship's bunks, for the name the host writes on it
/// (`Game::bunk_tags`): which, where its middle is in room units, and
/// whose it is — `None` for one nobody has, which says so on the deck.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct BunkTag {
    pub bed: usize,
    pub at: Vec2,
    pub owner: Option<usize>,
}

/// One construction site the world wants worked, this step: where it is,
/// in room units, and how long putting it together takes. The world hands
/// the room a fresh list every step — `Game::set_build_orders` — worked
/// out from the sites, the pool and whether the ship is at rest; the room
/// decides who goes and whether it is reached from the deck or from
/// outside, and reports what was done. Nothing is carried to a site: a
/// part is bought (feature 95). See `Kind::Build`.
#[derive(Clone, PartialEq, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Build {
    pub site: u32,
    /// Every tile of the footprint, as a rect each.
    pub tiles: Vec<Rect>,
    pub minutes: f32,
}

/// One thing the world wants carried between two workstations, this step:
/// out of the cabinet at bench `from` and onto bench `to` — a gun or a
/// piece of armour from the lockers to the workbench for an upgrade, or
/// the upgraded one back — by their indices in `Room::benches`. What the
/// thing is stays the world's: the room says a Bim took something at
/// `from` and put it down at `to` (`Room::ferry_picked`,
/// `Room::ferry_dropped`), or gave up on the way (`ferry_returned`), and
/// the world moves it. The world hands the room a fresh list every step
/// (`Game::set_ferries`), at most one long, and keeps the order on it
/// until the drop lands, so a chain on its way finds it there. See
/// `Kind::Ferry`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Ferry {
    pub from: usize,
    pub to: usize,
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
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
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
        Kind::Build { outside: true, .. } => Some(Exclusive::Airlock),
        Kind::Build { site, .. } => Some(Exclusive::Site(site)),
        // A carry holds the bench it is putting the thing on: two Bims
        // ferrying to the workbench at once would be two hands in one slot,
        // and a Bim at work on it is at it already.
        Kind::Ferry { to, .. } => Some(Exclusive::Bench(to)),
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
        | Kind::Bandage { .. }
        | Kind::Treat { .. }
        | Kind::Fetch { .. }
        | Kind::Execute { .. }
        | Kind::Walk { .. }
        | Kind::Deploy { .. } => None,
    }
}

/// What the medical row has for a Bim: a part of somebody's to dress, or
/// a crewmate's dying state to treat. See `Game::medical_on_offer`.
#[derive(Clone, Copy, PartialEq, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
enum Care {
    Bandage(usize, Part),
    Treat(usize, Part),
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

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
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
    /// And which bunk, after `HIT_BED`: the menu assigns it.
    hit_bed: usize,
    hit_research: usize,
    /// Which side of the bathroom bulkhead the Bim was on last frame, and how
    /// long the door has left to stand open after it crossed.
    bim_was_inside: bool,
    door_shut_in: f32,
    rng: Rng,
    /// The crew. Two of them, and the index into this is a Bim's whole
    /// identity — it picks the berth, the seat at the table, the coverall, and
    /// the name the host prints over its head.
    bims: Vec<Bim>,
    /// The machines on this deck (feature 83, `crate::droid`), beside the
    /// Bims and never among them: a droid-held station's room has these
    /// and no `bims` at all. They stand **after** the Bims in the one
    /// **body** index space the world hands over and reads back
    /// ([`Game::body_count`]), so a target past `bims.len()` is one of
    /// these and a hit past it lands on one. Empty everywhere else —
    /// aboard a ship, in the test room, at a station people live on.
    droids: Vec<Droid>,
    /// Which of the crew is ticked first this frame. Rotates every frame so
    /// first refusal on the galley and the heads goes round rather than
    /// always falling to the same Bim.
    first_tick: usize,
    /// Corners of the selection box while the mouse is down; `None` otherwise.
    drag: Option<(Vec2, Vec2)>,
    /// A right-drag on the deck with a selection: the line the selected
    /// crew will be spread along, drawn while it is drawn. See
    /// `Game::order_drag_end`.
    order_drag: Option<(Vec2, Vec2)>,
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
    /// What a body outside is pushed out of: the hull and the rocks. Kept
    /// with the outside grid — see `refresh_outside`.
    outside_blockers: Vec<Rect>,
    /// What a body afield is pushed out of, one list a body: the ground's
    /// runs and the deck's solids that fall in its window. Kept with the
    /// windows — see `refresh_afield` — and not saved with them.
    #[cfg_attr(feature = "serde", serde(skip))]
    afield_blockers: Vec<Vec<Rect>>,
    /// Which version of the rocks the outside grid was built from, so it
    /// is rebuilt when they change and not otherwise.
    outside_version: Option<u64>,
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
    /// Whether the Bims here have needs at all (feature 102): whether
    /// food, rest, the heads, company, washing and the deck round them
    /// drain, bite and send anybody on an errand; whether the pot goes
    /// bad, the bays are grown and tended and the bunks are slept in.
    /// **On** in the classic room, which is the behaviour test room and
    /// is what it always was; the world switches it **off** for every
    /// room of a run and on again only for the tests that are about
    /// needs (`World::needs_enabled`), and says so every step, since a
    /// room is built afresh at every dock and undock. Off, the need
    /// levels stand where they are, and a fixture that only served a
    /// need is furniture: no menu opens on it.
    needs_enabled: bool,
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
    /// Draw every body whatever the world says is in view: a probe's
    /// switch for a picture (`show_everybody_for_probe`), never set in
    /// the game and left out of a save with the rest of the picture.
    #[cfg_attr(feature = "serde", serde(skip))]
    show_everybody: bool,
    /// Under `Fog::None`, how long each body stays drawn since the world
    /// last said it was in view — [`SEEN_FOR`] from the sighting, counting
    /// down. Anybody past the end of it is not drawn.
    seen_for: Vec<f32>,
    /// Which of the room's tiles are somebody else's — a station's, on a
    /// joined deck — and whose. Kept here so a relayout can hand it to the
    /// fresh grid; the sight itself is the room's.
    foreign: Option<(Rect, Stance, bool)>,
    /// Where the **ship's own** gangway is, in room units (feature 84):
    /// the deck a few tiles inside the ship's airlock, which is where a
    /// fall back goes. The world says it every step, because the room
    /// cannot work it out for itself: `Room::gangway` on a joined deck
    /// is the *joined* design's first free airlock, and with the ship's
    /// own mated to the station that is the station's far door — the
    /// other end of the building from the ship.
    #[cfg_attr(feature = "serde", serde(skip))]
    home: Option<Vec2>,
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
    /// of any of them or was in anybody's sight within [`ALARM_HOLD`], or
    /// one of them was hit within as long — and
    /// then every crew member but the player's is under arms and fights,
    /// walking to wherever `Tactics::stand` says. See `Game::tick_combat`.
    alarm: bool,
    /// Whether the crew's bots are **under arms** — the alarm, or a
    /// player's own Bim armed and leading them (feature 84). This, not
    /// the alarm, is what `muster_crew` is edged on, what keeps a bot at
    /// the player's side and what lets the crew take a player's orders:
    /// a player that draws its weapon has the crew draw theirs and come
    /// along, and a fight found on the way is the alarm on top of it.
    mustered: bool,
    attacked_for: f32,
    /// How long since any of this room's living, waking people on the
    /// deck had an enemy in sight, in seconds at 1x — for a hostile
    /// room, since any target was a sighting rather than a belief, the
    /// airlock's eyes included. Half of `calm`, with the fight's `lull`.
    enemy_unseen_for: f32,
    /// A spot this room's people have eyes on whatever they are doing:
    /// the station's airlock, for a hostile station's room — a target
    /// within [`AIRLOCK_WATCH`] tiles of it is seen. See `set_watched`.
    watched: Option<Vec2>,
    /// The hits this room's own bodies took since the world last asked,
    /// already applied. See `Game::take_wounds_taken`.
    wounds_taken: Vec<Hit>,
    /// The engineers' sentries on this deck, as the world last said
    /// (`set_sentries`, feature 74): shooters that are not bodies, fired
    /// by `tick_combat` after the crew and standing after them on the
    /// list a hostile bolt looks for. Their triggers are the room's to
    /// keep between steps; everything else about them is the world's.
    sentries: Vec<Sentry>,
    /// The hits each sentry took since the world last asked, by the
    /// world's id — a hostile bolt's damage, or a blow's — for the world
    /// to take off its health.
    sentry_hits: Vec<(u32, f32)>,
    /// The per-Bim work factors the world set (`set_work_factors`): what
    /// an engineer's talents do to a craft's and a build's working steps,
    /// one each way, applied in the `effort` product and nowhere else.
    work_factors: Vec<(f32, f32)>,
    /// Which Bims keep at a deploy when hit (`set_steady_hands`): the
    /// engineer's *steady hands* talent. Anybody else drops it.
    steady_hands: Vec<bool>,
    /// What each Bim shoots with over its weapon (`set_skills`, feature
    /// 75): a soldier's talents and its brace, `Skill::NONE` for anybody
    /// the world does not name.
    skills: Vec<Skill>,
    /// What a commander's squad order tells each Bim to do (`set_squad`,
    /// feature 78): `Squad::None` for a body under none. The world's
    /// word, said every step, and left out of a save with it — the
    /// order itself is the world's (`world::SquadOrder`).
    #[cfg_attr(feature = "serde", serde(skip))]
    squad: Vec<Squad>,
    /// What each player's standing order to the bots is (`set_orders`,
    /// feature 84): one an entry, by player slot, [`Standing::Follow`] for
    /// a slot that has not said. The world's word, said every step, and
    /// left out of a save with it — the order itself is the world's
    /// (`world::Standing`).
    #[cfg_attr(feature = "serde", serde(skip))]
    standing: Vec<Standing>,
    /// Which Bims the squad order itself put under arms, so that its
    /// ending lets go of those and of nobody else — a Bim recruited by
    /// the alarm, or by a probe, is left as it was found. Derived from
    /// `squad` a step behind, and left out of a save with it.
    #[cfg_attr(feature = "serde", serde(skip))]
    squad_armed: Vec<bool>,
    /// What a medic's beam does to each body this step (`set_held`,
    /// feature 76): `None` for a body no beam holds. The world's word,
    /// said every step, and left out of a save with it.
    #[cfg_attr(feature = "serde", serde(skip))]
    held: Vec<Option<Beamed>>,
    /// What each Bim's doctoring runs at (`set_doctoring`, feature 76):
    /// a medic's talents on its bandaging and treating. The world's word
    /// every step, `Doctoring::NONE` for anybody not named.
    #[cfg_attr(feature = "serde", serde(skip))]
    doctoring: Vec<Doctoring>,
    /// Every dressing and treatment finished since the world last asked
    /// — whose hands, on whom, and what was used — for the medic's
    /// experience (`take_healings`, feature 76).
    #[cfg_attr(feature = "serde", serde(skip))]
    healings: Vec<Healed>,
    /// The tiles of laid sandbags a grenade's burst reached since the
    /// world last asked, for it to take the deployables off
    /// (`take_bags_blown`).
    bags_blown: Vec<(i32, i32)>,
    /// Every worn piece a hit broke since the world last asked — whose,
    /// and what it was — for the world to say so. See `Game::wound`.
    pieces_broken: Vec<(usize, ArmourKind)>,
    /// Every dying state a hit put a body in since the world last asked,
    /// and every one a medkit took it out of — whose, and which — for the
    /// world to say so. See `Game::strike` and `Game::apply_treatments`.
    traumas: Vec<(usize, Trauma)>,
    treated: Vec<(usize, Trauma)>,
    /// The dropped weapon a click last landed on, for the host to ask
    /// after `hit_at` said `HIT_DROPPED`: its id.
    hit_dropped: u32,
    /// The dropped weapon the pointer rests on, ringed in `render` so the
    /// gun under the mouse reads as something to click; `None` for none.
    hover_dropped: Option<u32>,
    /// What a hostile room's people believe about each of the world's
    /// targets: where it was when one of them last saw it, and how long
    /// ago. Index for index with `set_hostiles`; `None` for one never
    /// seen, down, or forgotten. See `set_hostiles`.
    last_seen: Vec<Option<Seen>>,
    /// What the **machines** in this room believe about each of their
    /// own targets (feature 94): `last_seen` for the list they read when
    /// the world has given them one of their own — a town the crew are
    /// defending, where the droids are in the room with the people they
    /// came for. Empty everywhere else.
    machine_seen: Vec<Option<Seen>>,
    /// Which of this room's own bodies **shelter** rather than fight
    /// (feature 94): a townsperson who is neither the guard nor a
    /// mercenary, while the machines are on the town. Not mustered by
    /// the alarm, and posted indoors at the nearest bunk. Empty
    /// everywhere else, which is everybody fighting as they always did.
    sheltering: Vec<bool>,
    /// The picture, left out of a save: `render` draws it again.
    #[cfg_attr(feature = "serde", serde(skip))]
    list: DrawList,
    /// Where in `list` the bodies start — the dropped weapons, then the
    /// Bims with their hit flashes, and everything drawn over them — in
    /// floats, for a host that has to lift them over a picture of its
    /// own. See `render` and `shapes_split`.
    bodies_from: usize,
    /// Where in `list` the fog's place is, in floats: after the bodies
    /// and the bedding and before the shots, the rings and the marquee.
    /// Under `Fog::All` the fog is drawn there in the list; under
    /// `Fog::Crew` the host lays its smooth fog there itself, which it can
    /// only do if it knows where — see `shapes_fog_split`. A picture, so
    /// left out of a save like the list.
    #[cfg_attr(feature = "serde", serde(skip))]
    fog_from: usize,

    /// Maps the fixed-size room onto whatever canvas the host has. The host
    /// applies this before replaying the draw list, and inverts it to turn
    /// pointer positions back into room coordinates.
    view_scale: f32,
    view_offset: Vec2,
    /// How many of the crew are players' own — the first `players` of
    /// them, slot *i* steering Bim *i* — and take orders from their
    /// player alone; the rest are bots, ordered about only under the
    /// alarm. One in the classic room and aboard a ship with one player;
    /// the world sets the ship's (`set_players`). See `crate::order`.
    players: usize,
    /// Whose eyes the picture is drawn for: the selection ring is that
    /// player's. A picture setting, this window's own, left out of a save
    /// with the picture (`set_viewer`).
    #[cfg_attr(feature = "serde", serde(skip))]
    viewer: u32,
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
        let mut game = Game::with_room(
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
        );
        // A box of dressings apiece, so `bims room` can try the bandage
        // chain without a drug lab (feature 87: there is no count on a
        // shelf any more, only what a Bim carries). It draws nothing off
        // any stream, so every seeded probe is where it was.
        for who in 0..game.bims.len() {
            game.give_stack(who, BANDAGE, room::BANDAGES_AT_DAWN);
        }
        game
    }

    /// Put `n` of a stackable thing in a Bim's pack, boxes as full as
    /// the thing stacks, as far as the pack has room (feature 87). What
    /// the classic room deals its dressings with and what the world's
    /// restock tops a pack up by; the number that actually went in.
    pub fn give_stack(&mut self, who: usize, item: Item, n: u32) -> u32 {
        let mut left = n;
        while left > 0 {
            let Some(bim) = self.bims.get_mut(who) else {
                break;
            };
            let cell = bim
                .gear
                .stack_with_room(item)
                .or_else(|| bim.gear.free_cell_for(item));
            let Some(cell) = cell else { break };
            let fits = match bim.gear.room_in(cell, item) {
                0 => item.stack_limit(),
                spare => spare,
            }
            .min(left);
            if !bim.gear.put_many(cell, item, fits) {
                break;
            }
            left -= fits;
        }
        n - left
    }

    /// A game in a room laid out from elsewhere — a ship design, through
    /// `crate::aboard` — with one Bim per `start`, standing there.
    ///
    /// **As many as there are `starts`: the caller has counted the crew.**
    /// A Bim's index is its seat, and its bunk to begin with — bunk `i`
    /// for Bim `i` as far as the bunks go (`Room::sleeps_in`, the player's
    /// to change with `assign_bed`) — and a room may be given more Bims
    /// than it has bunks — the `combat` command's fourteen on a ship with
    /// five — so `seat` clamps and the bunks run out: the ones past the
    /// bunks stand on the deck to begin with (`crate::aboard::starts`),
    /// share the last chair, and sleep on the deck. The ship's room takes
    /// whatever the world counted, since the world has sized everything
    /// else by that number and a room that quietly held fewer was a world
    /// that disagreed with itself; a station's room is cut to its bunks
    /// before it gets here (the world's `Residents::open`), so a garrison
    /// bigger than an orbital's four bunks is four.
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
        let crew = starts.len();
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
        // A bunk apiece in crew order, as far as the bunks go: the ones past
        // them sleep on the deck until the player gives them one
        // (`assign_bed`), or one comes free.
        let mut room = room;
        room.sleeps_in = (0..crew)
            .map(|who| (who < room.bunks()).then_some(who))
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
            hit_bed: 0,
            hit_research: 0,
            bim_was_inside: false,
            door_shut_in: 0.0,
            rng,
            bims,
            first_tick: 0,
            drag: None,
            order_drag: None,
            markers: Vec::new(),
            schedule: Schedule::new(),
            manager: Manager::new(),
            priorities: Priorities::new(),
            helm: None,
            helmsman: None,
            orders: Vec::new(),
            outside_blockers: Vec::new(),
            afield_blockers: Vec::new(),
            outside_version: None,
            visitors: Vec::new(),
            visitors_down: Vec::new(),
            visitors_hailable: Vec::new(),
            autonomous: true,
            needs_enabled: true,
            highlight: room::SPOT_NOTHING,
            fog: Fog::Crew,
            show_everybody: false,
            seen_for: Vec::new(),
            foreign: None,
            home: None,
            stance: Stance::Friendly,
            combat: Combat::new(seed),
            droids: Vec::new(),
            hostile_bodies: false,
            at_war: false,
            alarm: false,
            mustered: false,
            attacked_for: 0.0,
            // A fresh room has been calm for ever.
            enemy_unseen_for: f32::MAX,
            watched: None,
            wounds_taken: Vec::new(),
            sentries: Vec::new(),
            sentry_hits: Vec::new(),
            work_factors: Vec::new(),
            steady_hands: Vec::new(),
            skills: Vec::new(),
            squad: Vec::new(),
            standing: Vec::new(),
            squad_armed: Vec::new(),
            held: Vec::new(),
            doctoring: Vec::new(),
            healings: Vec::new(),
            bags_blown: Vec::new(),
            pieces_broken: Vec::new(),
            traumas: Vec::new(),
            treated: Vec::new(),
            hit_dropped: 0,
            hover_dropped: None,
            last_seen: Vec::new(),
            machine_seen: Vec::new(),
            sheltering: Vec::new(),
            list: DrawList::new(),
            bodies_from: 0,
            fog_from: 0,
            view_scale: 1.0,
            view_offset: Vec2::ZERO,
            players: 1,
            viewer: 0,
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
    /// banked in it — a sheaf of fibre that was in somebody's hands —
    /// can still be read off it (`take_harvested_fibre`) before it is
    /// dropped.
    /// How many bunks the room has — the stand-in for a layout with none
    /// not counted. What a hire asks before `adopt`: a hire wants a bunk
    /// to spare, though a crew past the bunks is carried (the `combat`
    /// command's fourteen on five) and sleeps on the deck.
    pub fn bed_count(&self) -> usize {
        self.room.bunks()
    }

    // --- whose bunk is whose ---------------------------------------------

    /// Which bunk Bim `who` sleeps in, or `None` for one with no bunk, which
    /// sleeps on the deck — for [`crate::bim::GROUND_SLEEP`] in every
    /// [`crate::bim::GROUND_WINDOW`], and sore after (`is_sore`). See
    /// `Room::sleeps_in`.
    pub fn bed_of(&self, who: usize) -> Option<usize> {
        self.room.bed_of(who)
    }

    /// Whose bunk `bed` is, or `None` for one nobody has.
    pub fn bed_owner(&self, bed: usize) -> Option<usize> {
        self.room.bed_owner(bed)
    }

    /// Whether bunk `bed` is somebody else's furniture: a station's, on a
    /// joined deck, by the box `set_foreign` named — not the ship's to give
    /// anybody.
    pub fn bed_is_foreign(&self, bed: usize) -> bool {
        let Some(berth) = self.room.beds.get(bed) else {
            return false;
        };
        let centre = berth.frame.center();
        match self.foreign {
            Some((rect, _, false)) => rect.contains(centre),
            Some((rect, _, true)) => !rect.contains(centre),
            None => false,
        }
    }

    /// Whether bunk `bed` is one of the ship's to give: a real bunk, not the
    /// stand-in, and not a docked station's.
    pub fn bed_assignable(&self, bed: usize) -> bool {
        bed < self.room.bunks() && !self.bed_is_foreign(bed)
    }

    /// Give Bim `who` bunk `bed`, or no bunk at all. A bunk is one Bim's:
    /// whoever had it loses it. Anybody in the middle of a lie-down that
    /// this moves — the old owner in that bunk, `who` in its old one or on
    /// the deck — gets up first, since the chain would otherwise put the
    /// blanket back on the wrong bed; a sleep waiting on the queue is left
    /// there and walks to the new bunk when its turn comes. `false`, and
    /// nothing moved, for a bunk that is not the ship's to give
    /// (`bed_assignable`) or a Bim not aboard.
    pub fn assign_bed(&mut self, who: usize, bed: Option<usize>) -> bool {
        if who >= self.bims.len() || bed.is_some_and(|b| !self.bed_assignable(b)) {
            return false;
        }
        if self.room.bed_of(who) == bed {
            return true;
        }
        let was = bed.and_then(|b| self.room.bed_owner(b));
        for sleeper in [Some(who), was].into_iter().flatten() {
            if self.bims[sleeper]
                .task
                .as_ref()
                .is_some_and(|t| t.kind() == Kind::Rest)
                && let Some(task) = self.bims[sleeper].task.take()
            {
                task.abandon(&mut self.bims[sleeper].character, &mut self.room);
            }
        }
        if let Some(was) = was {
            self.room.sleeps_in[was] = None;
        }
        if self.room.sleeps_in.len() <= who {
            self.room.sleeps_in.resize(who + 1, None);
        }
        self.room.sleeps_in[who] = bed;
        true
    }

    /// The first of the ship's bunks nobody has, if any.
    fn free_bed(&self) -> Option<usize> {
        (0..self.room.bunks())
            .find(|&bed| !self.bed_is_foreign(bed) && self.room.bed_owner(bed).is_none())
    }

    /// A bot takes a bunk that is going spare. Every step: for each of the
    /// ship's bunks nobody has, the first crew member alive with none that
    /// is not a player's own gets it — a hire, one of the `combat`
    /// command's fourteen, whoever was put on the deck when a bunk changed
    /// hands. A player's Bim is left as it is: giving up a bunk is the
    /// player's to do, and so is taking one. Through `assign_bed`, so a
    /// lie-down on the deck is stood up first.
    fn settle_bunks(&mut self) {
        while let Some(bed) = self.free_bed() {
            let Some(who) = (self.players..self.bims.len())
                .find(|&who| self.bims[who].is_alive() && self.room.bed_of(who).is_none())
            else {
                return;
            };
            self.assign_bed(who, Some(bed));
        }
    }

    /// A tag for every bunk of the ship's, for the host to write a name
    /// on: where the bunk's middle is, in room units, and whose it is —
    /// `None` for one nobody has. A station's bunks on a joined deck are
    /// left out, being nobody's to give (`bed_assignable`).
    pub fn bunk_tags(&self) -> Vec<BunkTag> {
        (0..self.room.bunks())
            .filter(|&bed| self.bed_assignable(bed))
            .map(|bed| BunkTag {
                bed,
                at: self.room.beds[bed].frame.center(),
                owner: self.room.bed_owner(bed),
            })
            .collect()
    }

    /// Whether Bim `who` may lie down on the deck now: no bunk of its own,
    /// and some of the window's allowance left — at least
    /// [`GROUND_SLEEP_MIN`] of it, since a lie-down of two minutes is a
    /// walk and a climb for nothing. A Bim with a bunk never lies on the
    /// deck. See [`crate::bim::GROUND_SLEEP`].
    pub fn can_sleep_on_ground(&self, who: usize) -> bool {
        self.room.bed_of(who).is_none() && self.bims[who].ground_left >= GROUND_SLEEP_MIN
    }

    /// Game minutes of lying on the deck Bim `who` has left in the window
    /// that is open — or the whole allowance, with none open. For the panel.
    pub fn ground_sleep_left(&self, who: usize) -> f32 {
        self.bims[who].ground_left
    }

    /// Game hours of being sore from the deck left, nought when not. For the
    /// panel.
    pub fn soreness(&self, who: usize) -> f32 {
        self.bims[who].sore / clock::HOUR
    }

    pub fn is_sore(&self, who: usize) -> bool {
        self.bims[who].is_sore()
    }

    /// Where a body stands to get into the bunk with that index, for the
    /// tests: somewhere on the deck, in a room of its own.
    #[allow(dead_code)]
    pub fn bed_station_for_probe(&self, bed: usize) -> Vec2 {
        self.room.bed_station(bed)
    }

    pub fn take_crew(&mut self) -> Vec<Bim> {
        // The war and the muster end with the room: both are mustered on
        // their edge, and a fresh room starts at peace, so a body carried
        // over recruited would stand under arms with nothing to let it go
        // — the crew the whole flight after a hostile dock, a station's
        // people after the ship has gone. Stood down here, the new room
        // musters them again the step an enemy is in range, or the step
        // it reads a player's standing order.
        //
        // **It is `mustered` and not `alarm`**, since feature 84: the
        // alarm is only one of the two things that put the crew under
        // arms, and a crew led by a player — or by a squad order, whose
        // `squad_armed` is rebuilt empty in the new room and so can
        // never let anybody go — was carried over recruited with the
        // edge already spent.
        if self.at_war {
            self.at_war = false;
            self.muster(false);
        }
        if self.alarm || self.mustered {
            self.alarm = false;
            self.mustered = false;
            self.muster_crew(false);
            self.squad_armed.clear();
        }
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
        // Whose bunk was whose goes with them, for `adopt` to read back:
        // the ship's bunks keep their numbers from one deck to the next
        // (`crate::aboard::layout_of` lists them in id order, and a joined
        // deck puts the ship's parts first).
        for (who, bim) in self.bims.iter_mut().enumerate() {
            bim.bed = self.room.bed_of(who);
        }
        self.room.sleeps_in.clear();
        // The room is about to be thrown away, and a gun on its deck with
        // it: whoever dropped one takes it along — into the hand, else the
        // pack, else it is lost with the deck.
        for (who, bim) in self.bims.iter_mut().enumerate() {
            while let Some(i) = self.room.weapons_down.iter().position(|d| d.owner == who) {
                let d = self.room.weapons_down.remove(i);
                if bim.gear.weapon.is_none() {
                    bim.gear.weapon = Some(d.weapon);
                } else if let Some(cell) = bim.gear.free_cell() {
                    bim.gear.pack[cell] = Some(Item::Weapon(d.weapon));
                }
            }
        }
        std::mem::take(&mut self.bims)
    }

    /// Take a crew in — from [`Game::take_crew`] on another room — standing
    /// each one `shift` from where it stood there, because the two rooms'
    /// origins differ by that. They go on the end: index is seat, and the
    /// room's beds are in the order its layout listed them, so a caller
    /// that wants the ship's crew in the ship's bunks adopts them first.
    /// Everybody comes in, bunks or no bunks — the crew is the world's
    /// count, as in [`Game::with_layout`], and a crew past the bunks
    /// sleeps on the deck. Anybody whose feet would land somewhere a
    /// body cannot stand — off the deck of a room that has just shrunk — is
    /// stood at their bunk instead — or, with none, where they stood.
    ///
    /// **Its bunk comes with it** (`Bim::bed`, which `take_crew` wrote):
    /// kept when it names one of this room's own bunks that nobody here
    /// has, and else the first free one, and else none — so a crew past
    /// the bunks sleeps on the deck, and a hire, whose old berth was a
    /// station's, takes whatever bunk is spare.
    pub fn adopt(&mut self, crew: Vec<Bim>, shift: Vec2) {
        for mut bim in crew {
            let who = self.bims.len();
            let bed = bim
                .bed
                .take()
                .filter(|&b| self.bed_assignable(b) && self.room.bed_owner(b).is_none())
                .or_else(|| self.free_bed());
            self.room.sleeps_in.push(bed);
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
                bed.map_or(free, |b| self.room.bed_station(b))
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
            // And its round (feature 102), stop for stop, where it had got
            // to on it: the same fixtures, laid out somewhere else.
            if let Some(routine) = bim.routine.as_mut() {
                routine.shift(shift);
            }
            // A player's own keeps its selections across the move; a
            // crewmate picked by a marquee is let go of.
            if !self.is_player(who) {
                bim.character.selected = 0;
            }
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

    /// Age the fight's passing lights by `dt` **real** seconds — the
    /// host's frame, nought while the game is paused — and switch them on
    /// for this room (feature 98, `crate::fx`). Drawing only: a host that
    /// never calls it gets the picture the room always drew, and nothing
    /// the simulation reads is touched.
    pub fn fade(&mut self, dt: f32) {
        self.combat.fx.age(dt);
    }

    /// The simulation alone, without drawing it. What the ship game calls
    /// aboard — up to twenty-four times a frame at its top speed, where a
    /// picture of every step but the last would be a picture nobody sees.
    pub fn simulate(&mut self, dt: f32) {
        self.clock.advance(dt);
        self.refresh_outside();
        self.refresh_afield();
        self.room.update(dt);
        // A pot goes bad on a filthy galley only where somebody eats out
        // of it (feature 102: with the needs off nobody does).
        if self.needs_enabled {
            self.judge_the_food();
        }

        // The bays grow on the clock, against the manager's target. It is
        // the game that drives them rather than the room, because the target
        // is the manager's and the room has never heard of the manager.
        // With the needs off a bay is furniture: nothing grows in it.
        let want = self.manager.stock_target();
        let minutes = dt * MINUTES_PER_SECOND;
        let (veg, tofu, fibre) = (self.room.veg, self.room.tofu, self.room.fibre);
        if self.needs_enabled {
            for bay in &mut self.room.bays {
                bay.update(dt, minutes, veg, tofu, fibre, want);
            }
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
        // Asked whether it is needed or not, since asking is what re-arms
        // it; with the needs off nobody is sent to bed.
        let bedtime = self.schedule.due(self.clock.minutes()) && self.needs_enabled;

        // A bunk going spare goes to a bot with none before anybody is sent
        // to bed, so the night is spent in it rather than on the deck. With
        // the needs off a bunk is furniture, and nobody is dealt one.
        if self.needs_enabled {
            self.settle_bunks();
        }

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
        self.apply_treatments();
        self.apply_pickups();
        // Under arms, bodies do not stack: whoever is recruited is pushed
        // apart from whoever else is, after everybody has moved.
        self.separate_under_arms();
        // And a body in somebody's arms goes where the arms went
        // (feature 86), after the shoving rather than before it, or it
        // would be pushed out of them again.
        self.carry_the_carried();
        // A belief about where the crew are ages, and is given up after a
        // minute unseen.
        for seen in self
            .last_seen
            .iter_mut()
            .chain(self.machine_seen.iter_mut())
        {
            if let Some(s) = seen.as_mut() {
                s.ago += dt;
                if s.ago > FORGET_AFTER {
                    *seen = None;
                }
            }
        }
        // The blood on the deck, for everybody: a drop is a stain on the
        // tile it lands on, and the deck keeps it until somebody sweeps.
        // A body a medic's beam holds bleeds nothing, so it drips nothing.
        for (who, bim) in self.bims.iter_mut().enumerate() {
            if self.held.get(who).copied().flatten().is_none() {
                bim.tick_drips(dt, &mut self.rng, &mut self.room.filth);
            }
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
        // A machine on the deck is a body for the doors like anybody
        // else: an unlocked door opens for a droid walking up to it,
        // which is the whole of "droids open doors" (feature 83). A
        // wreck opens nothing.
        let bodies: Vec<Vec2> = self
            .bims
            .iter()
            .filter(|b| !b.character.is_dead())
            .map(|b| b.character.pos)
            .chain(self.droids.iter().filter(|d| !d.destroyed).map(|d| d.pos))
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
    /// Where a shot of this Bim's leaves (feature 84): the emitter of
    /// the gun in its hands ([`Character::muzzle`]), which is what the
    /// picture shows and what the barrel points along — or the `eye` it
    /// was aimed from when the muzzle is round a corner from it. The
    /// fallback is not a nicety: a body leaning out of a **peek** is
    /// drawn only part of the way there (`character::LEAN`), so its gun
    /// can still be the wall's side of the corner its eye sees past, and
    /// a bolt started there would land on that wall. A body with nothing
    /// drawn in its hands has no muzzle and shoots from the eye as it
    /// always did.
    fn shot_from(&self, who: usize, eye: Vec2) -> Vec2 {
        let Some(muzzle) = self.bims[who].character.muzzle() else {
            return eye;
        };
        let tile = self.room.sight.tile_of(muzzle);
        if self.room.sight.clear_line(eye, tile) {
            muzzle
        } else {
            eye
        }
    }

    /// An enemy's muzzle glowing where its body stands (feature 98): a
    /// hostile room's Bim recording a shot for the world to fly, or a
    /// machine firing. The bolt flies elsewhere — the crew's room, or
    /// from the machine's eye — so its own `fire_as` lights nothing, and
    /// the glow is the shooter's room's. Drawing only.
    fn lit_muzzle(&mut self, muzzle: Vec2, at: Vec2, weapon: Weapon) {
        self.combat.fx.muzzle(muzzle, at - muzzle, weapon, true);
    }

    fn tick_combat(&mut self, dt: f32) {
        // The doors as they stand this step go into the sight before
        // anybody aims through it. The trace puts them there too, but
        // only at the picture and only through the crew's eyes: a
        // station's room under a joined deck is drawn through nobody's,
        // and its people would otherwise shoot through every shut door
        // as if it stood open.
        let shut = self.shut_now();
        self.room.sight.set_shut(&shut);
        // The calm's two clocks: how long since the last shot or blow
        // here, and how long since anybody on this side had an enemy in
        // sight. A hostile room's sightings were traced by `set_hostiles`
        // — a target that is not stale is one somebody sees, or the
        // airlock's eyes do; the crew's room looks for itself, from every
        // living, waking body on the deck.
        self.combat.age(dt);
        let enemy_seen = if self.hostile_bodies {
            self.combat.targets().iter().flatten().any(|t| !t.stale)
        } else {
            !self.combat.targets().is_empty()
                && self.bims.iter().any(|b| {
                    b.is_alive()
                        && !b.character.is_unconscious()
                        && !b.character.is_outside()
                        && self.combat.sees_any(&self.room.sight, b.character.pos)
                })
        };
        self.enemy_unseen_for = if enemy_seen {
            0.0
        } else {
            (self.enemy_unseen_for + dt).min(f32::MAX)
        };
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
            && (self.attacked_for > 0.0
                || self.enemy_unseen_for < ALARM_HOLD
                || self.enemy_within(ALARM_RANGE * TILE));
        self.alarm = alarm;
        // And the crew's bots are under arms for the alarm **or** because
        // a player is leading them (feature 84): a player's own Bim with
        // its weapon out, or a standing order given. That is what walks
        // the crew off the ship behind a player who is going somewhere
        // dangerous, with no enemy anywhere near yet.
        let mustered = alarm || self.led();
        if mustered != self.mustered {
            self.mustered = mustered;
            self.muster_crew(mustered);
        }
        // And a commander's squad, which is under arms with the alarm
        // or without it (feature 78).
        self.muster_squad(mustered);
        // The soldiers' odds in cover, for the bolts below (feature 75).
        let cover_odds: Vec<f32> = (0..self.bims.len())
            .map(|who| self.skill(who).cover_dodge)
            .collect();
        self.combat.set_own_cover_dodge(cover_odds);
        for who in 0..self.bims.len() {
            let bim = &mut self.bims[who];
            bim.trigger.tick(dt);
            // How a fall back is walked is worked out afresh every step
            // (feature 84): `fall_back_aboard` sets it below for a bot
            // under the order, and the aim turns a sprint into a
            // backing walk the moment there is something to shoot at.
            bim.character.set_falling_back(None);
            bim.hit_flash = (bim.hit_flash - dt).max(0.0);
            bim.melee_timer = (bim.melee_timer - dt).max(0.0);
            if let Some(blow) = bim.blow.as_mut() {
                blow.left -= dt;
            }
            // A brace ends when the soldier goes down (feature 75); while
            // it holds, the picture says so.
            if bim.braced
                && (!bim.is_alive() || bim.character.is_unconscious() || bim.character.is_outside())
            {
                bim.braced = false;
            }
            bim.character.set_braced(bim.braced);
            // And a bulwark when the tank goes down (feature 77).
            if bim.bulwark
                && (!bim.is_alive() || bim.character.is_unconscious() || bim.character.is_outside())
            {
                bim.bulwark = false;
            }
            // A beam ends the same way (feature 76): the world reads the
            // flag back and breaks the link. And a surge runs its seconds
            // out; ending, a *closing surge* closes every open wound.
            if bim.beaming
                && (!bim.is_alive() || bim.character.is_unconscious() || bim.character.is_outside())
            {
                bim.beaming = false;
            }
            if let Some(surge) = bim.surge.as_mut() {
                surge.left -= dt;
                if surge.left <= 0.0 {
                    let closing = surge.closing;
                    bim.surge = None;
                    if closing && bim.is_alive() {
                        for part in Part::ALL {
                            bim.health.bandage(part);
                        }
                        bim.character.set_wounds([false; 3]);
                    }
                }
            }
            bim.character.set_surging(bim.surge.is_some());
            let skill = self.skills.get(who).copied().unwrap_or(Skill::NONE);
            // A bot's doctoring gives way to the fight: the moment it has
            // something to shoot at, the dressing or the treatment is put
            // down for good and the weapon comes out this very step. It
            // began with nothing in its sight (`medical_on_offer`), and a
            // bot winding a bandage while an enemy walked up and shot it
            // was a bot that never fired back.
            if self.care_gives_way(who, &skill)
                && let Some(task) = self.bims[who].task.take()
            {
                task.abandon(&mut self.bims[who].character, &mut self.room);
            }
            let bim = &mut self.bims[who];
            // Not while doctoring: both hands are on the bandage, or on the
            // kit on the way to the patient, so the weapon is holstered for
            // the whole errand — the walk included, since a bot that fired
            // on its way over would be a bot that never got there — and
            // drawn again after. The tactics leave it alone for the same
            // span (`doctoring`, below).
            let dressing = bim
                .task
                .as_ref()
                .is_some_and(|t| matches!(t.kind(), Kind::Bandage { .. } | Kind::Treat { .. }));
            // A body dying with an enemy about runs, whoever it is — the
            // player's own too. An enemy's people run with the weapon
            // holstered; the crew's give ground with the gun up and shoot
            // back as they go (`shoot_on_the_run`). How long it has been
            // in that state is `fear`, and a commander's aura holds it
            // there for a while first (feature 78); out of it the count
            // starts again.
            if self.would_flee(who) {
                self.bims[who].fear += dt;
            } else {
                self.bims[who].fear = 0.0;
            }
            let fleeing = self.is_fleeing(who);
            // A gun, not a blade: a body on the run shoots from where it
            // is and never closes to swing.
            let fights_on_the_run = !self.hostile_bodies
                && self.bims[who].gear.weapon.is_some_and(|w| !w.stats().melee);
            // In somebody's arms (feature 86): it goes where they go and
            // shoots nothing on the way.
            let carried = self.is_carried(who);
            // Finishing a body off draws the weapon whether or not the Bim
            // is under arms: the body it is at, while the hands are at it.
            let executing = self.bims[who]
                .task
                .as_ref()
                .and_then(|t| t.executing_at(&self.room));
            let bim = &mut self.bims[who];
            let armed = bim.is_alive()
                && (bim.character.is_recruited() || executing.is_some() || bim.braced)
                && bim.gear.weapon.is_some()
                && !bim.character.is_outside()
                && !bim.character.is_seated()
                && !bim.character.is_napping()
                && !bim.character.is_unconscious()
                && !dressing
                && (!fleeing || fights_on_the_run)
                // A medic beaming holds its fire (feature 76).
                && !skill.holds_fire
                // And so does one with a crewmate in its arms (feature
                // 86): both hands are the carry.
                && bim.carrying.is_none()
                // A body being carried shoots nothing either.
                && !carried;
            let weapon = bim.gear.weapon.filter(|_| armed);
            // The hand changing is heard: the weapon coming out, or going
            // back. Said for everybody; the app plays a player's own.
            if bim.character.is_armed() != weapon.is_some() {
                self.room.cues.push(Cued {
                    cue: Cue::Holster {
                        drawn: weapon.is_some(),
                        player: !self.hostile_bodies && who < self.players,
                    },
                    at: bim.character.pos,
                });
            }
            bim.character.set_armed(weapon.map(|w| w.kind));
            if fleeing {
                bim.locked = None;
                bim.blow = None;
                bim.peek = None;
                bim.character.set_lean(None);
                bim.character.set_aim(None);
                bim.smashing = None;
                // Out of the enemy's sight it binds its own wound where it
                // stands (`medical_on_offer`, off the medical row) and the
                // run waits for the hands to come off; an enemy coming into
                // view is the dressing put down and the run taken up again.
                let from = self.bims[who].character.pos;
                let seen = self.combat.sees_any(&self.room.sight, from);
                // Out of danger with a box of dressings on it, it binds
                // **every** wound rather than the one (feature 87): the
                // worst part now and the rest queued behind it, which is
                // the *Bandage all wounds* the pop-up offers. Only the
                // crew's — an enemy's run seals itself in and binds out
                // of its own pockets (`seal_and_bind`).
                let dressing = dressing
                    || (!seen
                        && !self.hostile_bodies
                        && !self.is_being_seen_to(who)
                        && self.bandage_all(who, who));
                if !dressing || seen {
                    if dressing {
                        self.interrupt(who);
                    }
                    self.flee(who, dt);
                }
                // An enemy's run locks the door behind it, and sealed in it
                // binds its wounds; a crew member's does neither.
                if self.hostile_bodies {
                    self.seal_and_bind(who, dt);
                }
                // A crew member's run shoots back; the hands on a bandage
                // hold the fire, and so does an enemy's run, whose weapon
                // is holstered.
                match weapon.filter(|_| !dressing || seen) {
                    Some(weapon) => self.shoot_on_the_run(who, dt, weapon, &skill),
                    None => self.bims[who].trigger.hold(),
                }
                continue;
            }
            // Off war, a hostile body posted beyond a locked door forces
            // its way to the post: a raider's boarders, sent to the ship's
            // gangway with the airlock shut against them. Before the
            // weapon is asked for, since a body off war is not under arms.
            if self.hostile_bodies && !war && bim.character.post().is_some() {
                self.breach(who, dt);
            }
            let bim = &mut self.bims[who];
            let Some(weapon) = weapon else {
                bim.trigger.hold();
                bim.locked = None;
                bim.blow = None;
                bim.peek = None;
                bim.character.set_lean(None);
                bim.character.set_aim(None);
                continue;
            };
            // The weapon's numbers through the soldier's skill (feature 75):
            // everybody else's are the weapon's own.
            let stats = skill.stats(weapon);
            // At a body it is finishing off: squared up to it, a pull of the
            // trigger every `1 / fire_rate` — the bolt flies at where it
            // lies, and over it, since a body down is nobody's target — or a
            // swing every `MELEE_PERIOD`; nothing else, until the chain says
            // it is done. The room says who was finished, the world kills.
            if let Some(at) = executing {
                let from = bim.character.pos;
                bim.trigger.hold();
                bim.locked = None;
                bim.blow = None;
                bim.peek = None;
                bim.character.set_lean(None);
                // Squared up to the body and the gun down at it: the
                // execution's shot leaves the muzzle like any other
                // (feature 84).
                bim.character.set_aim(Some(at));
                if (at - from).len() > 1e-3 {
                    bim.character.face((at - from).angle());
                }
                if stats.melee {
                    // Set outright rather than as an antic: a body on a chain
                    // is scripted, and `antic` defers to the script.
                    if bim.melee_timer <= 0.0 {
                        bim.melee_timer = MELEE_PERIOD;
                        bim.character.set_action(Action::Swing);
                    } else if bim.melee_timer < MELEE_PERIOD - SWING_TIME {
                        bim.character.set_action(Action::None);
                    }
                } else if bim.trigger.pull_single(&stats) {
                    let muzzle = self.shot_from(who, from);
                    if self.hostile_bodies {
                        self.lit_muzzle(muzzle, at, weapon);
                        self.combat.shoot(muzzle, at, weapon, false);
                    } else {
                        self.combat
                            .fire_as(muzzle, at, weapon, false, false, &skill, Some(who));
                    }
                }
                continue;
            }
            // At war it picks its own stand. A crew member under arms
            // that is not the player's to steer does what its player's
            // standing order says (feature 84, `bot_stand`) — keeping to
            // their side until it sees an enemy for itself, and then
            // picking its own — unless the player ordered *it*
            // somewhere, when it holds that. A patient somebody is
            // walking over to holds still for them instead (`tick_bim`),
            // and shoots from where it stands.
            let seen_to = self.is_being_seen_to(who);
            let holds_post = self.bims[who].character.post().is_some();
            let squad = self.squad_of(who);
            if war && !seen_to {
                self.plan_stand(who, dt, &stats, None);
                // And, with nobody it can get to, the doors in the way.
                self.breach(who, dt);
            } else if squad != Squad::None && !seen_to {
                // A commander's squad order comes before the ring round
                // the player and before the squad member's own stand
                // (feature 78) — and never reaches a Bim a player
                // steers, which the world sees to.
                self.squad_stand(who, dt, &stats, squad);
            } else if mustered
                && !seen_to
                && (!self.is_player(who) || self.under_attack())
                && !holds_post
            {
                // A bot under arms with nothing else claiming it: its
                // player's standing order, and its own tactics inside
                // that (feature 84).
                self.bot_stand(who, dt, &stats);
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
                        Some(who),
                    );
                }
            }
            let bim = &mut self.bims[who];

            // A melee first: locked, it neither aims nor fires, and the
            // burst it was in the middle of is over.
            let locked = self.combat.melee_with(&self.room.sight, from, &stats);
            bim.locked = locked;
            if let Some(enemy) = locked {
                bim.trigger.hold();
                bim.peek = None;
                bim.character.set_lean(None);
                bim.character.set_aim(None);
                if let Some(at) = self.combat.targets()[enemy].map(|t| t.at) {
                    bim.character.face((at - from).angle());
                }
                if bim.melee_timer <= 0.0 {
                    bim.melee_timer = MELEE_PERIOD;
                    // A blow's damage through the soldier's *bruiser*, fist
                    // and blade alike (feature 75).
                    let (damage, cut, swing) = if stats.melee {
                        (stats.damage * skill.melee, true, Action::Swing)
                    } else {
                        (FIST_DAMAGE * skill.melee, false, Action::Punch)
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
                bim.character.set_aim(None);
                continue;
            }
            // Falling back under a commander's order holds its fire
            // while it walks, and shoots what it sees once it is there
            // (feature 78).
            if matches!(squad, Squad::FallBack { .. }) && bim.character.is_walking() {
                bim.trigger.hold();
                bim.peek = None;
                bim.character.set_lean(None);
                bim.character.set_aim(None);
                continue;
            }
            // The enemy a squad order marked comes before any nearer
            // one, and a mark nobody can see is never fired at.
            let mark = match squad {
                Squad::Attack { enemy, seen: true } => Some(enemy),
                _ => None,
            };
            let Some((which, eye, at)) =
                self.combat.aim_marked(&self.room.sight, from, &stats, mark)
            else {
                bim.trigger.hold();
                bim.peek = None;
                bim.character.set_lean(None);
                bim.character.set_aim(None);
                continue;
            };
            // *Focus fire* is the odds against the mark alone, so the
            // shot's numbers are read again once it is known which.
            let stats = if mark == Some(which) {
                skill.stats_at(weapon, true)
            } else {
                stats
            };
            // Squared up to it, and a shot the moment the weapon is ready.
            // On the move too, crew and enemy alike, at half the odds
            // (`WALKING_ACCURACY`): a Bim on its way somewhere shoots
            // from its own eyes without turning off its route, and the
            // bots stand still for it unless the walk is to cover
            // (`plan_stand`). Only a peek wants standing against the wall
            // to lean from.
            let walking = bim.character.is_walking();
            if walking && eye != from {
                bim.trigger.hold();
                bim.peek = None;
                bim.character.set_lean(None);
                bim.character.set_aim(None);
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
            } else if bim.character.is_falling_back() {
                // Falling back with something to shoot at (feature 84):
                // it gives ground **backwards**, facing the enemy, so
                // the gun stays on the fight while the feet carry it
                // home. That is slower than the sprint it was on, and
                // `Character::update` is where the two part company.
                bim.character
                    .set_falling_back(Some(FallBack::Backwards((at - eye).angle())));
            }
            // The gun is swung onto the target before anything is fired
            // (feature 84), since the shot leaves the muzzle and the
            // muzzle is the end of the barrel: the aim goes on the
            // picture first, and `Character::muzzle` is read after it.
            bim.character.set_aim(Some(at));
            // One trigger for a Bim and a sentry alike (`Trigger::pull`).
            if bim.trigger.pull(dt, &stats) {
                // Out of the gun, not out of the chest: the emitter where
                // the picture puts it, falling back to the eye for a body
                // with nothing drawn in its hands.
                let muzzle = self.shot_from(who, eye);
                if self.hostile_bodies {
                    self.lit_muzzle(muzzle, at, weapon);
                    self.combat.shoot(muzzle, at, weapon, walking);
                } else {
                    self.combat
                        .fire_as(muzzle, at, weapon, false, walking, &skill, Some(who));
                }
            }
        }
        // The machines, after the crew (feature 83): the same fight, a
        // different body. In a hostile room everything they do is
        // recorded rather than flown; in a town the crew are defending
        // (feature 94) what they do to the town's own people flies here.
        //
        // **Their war is their own list's**, not the room's: the town's
        // room is friendly, so `war` is false in it and the machines
        // would stand about doing nothing. Everywhere else the two are
        // the same question, `machine_targets` being the room's own list
        // unless the world gave the machines one.
        let machine_war = self.combat.machine_targets().iter().any(|t| t.is_some());
        self.tick_droids(dt, war || machine_war);
        // The grenades (feature 75): the fuses burn down, and each that
        // runs out bursts on everything round it.
        for grenade in self.combat.tick_grenades(dt) {
            self.burst(grenade);
        }
        // The sentries, after the crew (feature 74): each is a shooter
        // with no body — a position, a weapon, its owner's skill and a
        // trigger — and shoots the way a Bim standing still does, at the
        // nearest visible enemy in range, through the one `aim` and the
        // one `fire_as`. Never in a hostile room: the world lays them on
        // the crew's deck alone. It never runs dry (feature 88).
        if !self.hostile_bodies {
            for i in 0..self.sentries.len() {
                let sentry = self.sentries[i];
                let stats = sentry.skill.stats(sentry.weapon);
                self.sentries[i].trigger.tick(dt);
                let Some((_, _, at)) = self.combat.aim(&self.room.sight, sentry.at, &stats) else {
                    self.sentries[i].trigger.hold();
                    continue;
                };
                if self.sentries[i].trigger.pull(dt, &stats) {
                    self.combat.fire_as(
                        sentry.at,
                        at,
                        sentry.weapon,
                        false,
                        false,
                        &sentry.skill,
                        None,
                    );
                }
            }
        }
        if !self.combat.quiet() || !self.combat.targets().is_empty() {
            // Our own bodies, for a hostile bolt to find: whoever is alive
            // and on its feet on the deck — where it leans out to while it
            // peeks, whether it does, and the odds its armour dodges a
            // bolt. A body out cold is nobody's target and a bolt flies
            // over it. The sentries stand after the crew on the list — no
            // armour to dodge with, and "peeking" only when dug in with
            // sandbags anywhere between it and the shooter, which is what
            // that talent means — so a hit past the crew's count is a hit
            // on a sentry.
            let crew = self.bims.len();
            let mut bodies: Vec<Option<(Vec2, bool, f32)>> = self
                .bims
                .iter()
                .enumerate()
                .map(|(who, b)| {
                    (b.is_alive() && !b.character.is_outside() && !b.character.is_unconscious())
                        .then_some((
                            b.peek.unwrap_or(b.character.pos),
                            b.peek.is_some(),
                            // Its armour's odds, and a braced soldier's *dug
                            // in* on top (feature 75).
                            (b.gear.dodge() + self.skill(who).dodge).min(1.0),
                        ))
                })
                .collect();
            for sentry in &self.sentries {
                // Dug in: judged against every bolt in the air this step,
                // from where each was fired.
                let dug_in = sentry.dug_in
                    && self.combat.bolts.iter().any(|b| {
                        b.hostile
                            && self
                                .room
                                .sight
                                .cover_anywhere_between(sentry.at, b.fired_from)
                                .is_some()
                    });
                bodies.push(Some((sentry.at, dug_in, 0.0)));
            }
            self.combat.step(dt, &self.room.sight, &bodies);
            // What landed on a lamp comes off the lamp: at nought it is
            // out, and the deck round it goes dark.
            for (lamp, damage) in self.combat.take_lamp_hits() {
                self.room.sight.damage_lamp(lamp, damage);
            }
            // And what landed on a sentry is the sentry's, not a body's.
            let taken = std::mem::take(&mut self.combat.wounds_taken);
            for hit in taken {
                if hit.who >= crew {
                    if let Some(sentry) = self.sentries.get(hit.who - crew) {
                        self.sentry_hits.push((sentry.id, hit.damage));
                    }
                } else {
                    self.combat.wounds_taken.push(hit);
                }
            }
        }
        // What landed on us goes on the body now, and a copy is kept for
        // the world to say so.
        let taken = std::mem::take(&mut self.combat.wounds_taken);
        for hit in &taken {
            self.count_hit_taken(hit);
            // The Unmaker's strip rides on the hit (feature 83): every
            // other weapon carries nought and this is the old call.
            self.strike_stripping(hit.who, hit.part, hit.damage, hit.cut, hit.strips);
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
            // A hunt ends with the war.
            self.bims[who].hunting = false;
        }
    }

    /// The crew's alarm, on or off: every living crew member but the
    /// player's own under arms with its errand put down — a weapon taken
    /// out of the pack into an empty hand first, since a recruited body
    /// with nothing in its hand is a body standing still, and for a blade
    /// in the hand every piece of armour in the pack put on over a part
    /// that has none or a broken one, since a melee bot always wears what
    /// armour it has — or let go, the queue kept, so peace picks it all up
    /// again. The player's own is left as the player has it: recruiting
    /// it is the player's.
    fn muster_crew(&mut self, alarm: bool) {
        let attacked = self.under_attack();
        for who in 0..self.bims.len() {
            if (self.is_player(who) && !attacked) || !self.bims[who].is_alive() {
                continue;
            }
            // A townsperson sheltering from the machines takes no arms
            // and keeps the post that put it indoors (feature 94).
            if self.sheltering.get(who).copied().unwrap_or(false) {
                continue;
            }
            if alarm {
                self.take_up_arms(who);
            }
            // Any post — peace's, or the spot the player ordered it to hold
            // during the alarm — is over either way.
            self.bims[who].character.set_post(None);
            self.bims[who].character.set_recruited(alarm);
        }
    }

    /// Whether a player is **leading** the crew without an alarm to make
    /// them (feature 84): any player's own Bim alive, on the deck and
    /// under arms, or any player's standing order something other than
    /// [`Standing::Follow`]. Either is a deliberate act of a player's, so
    /// it can muster the crew off their errands where nothing about the
    /// world would.
    fn led(&self) -> bool {
        if self.hostile_bodies {
            return false;
        }
        self.standing.iter().any(|&o| o != Standing::Follow)
            || (0..self.players.min(self.bims.len())).any(|p| {
                let bim = &self.bims[p];
                bim.is_alive() && bim.character.is_recruited() && !bim.character.is_outside()
            })
    }

    /// Whether the crew's bots are under arms — see the `mustered` field.
    pub fn is_mustered(&self) -> bool {
        self.mustered
    }

    /// One crew member put under arms: a weapon out of the pack into an
    /// empty hand, every piece of armour it has on for a blade, and the
    /// errand put down onto the queue. What the alarm does to each of
    /// them, and what a commander's squad order does to one without
    /// waiting for an alarm (feature 78).
    fn take_up_arms(&mut self, who: usize) {
        if self.bims[who].gear.weapon.is_none()
            && let Some(cell) = self.bims[who]
                .gear
                .pack
                .iter()
                .position(|c| matches!(c, Some(Item::Weapon(_))))
        {
            self.equip(who, cell);
        }
        if self.bims[who].gear.weapon.is_some_and(|w| w.stats().melee) {
            self.wear_what_it_has(who);
        }
        self.interrupt(who);
        self.bims[who].plan_wait = 0.0;
    }

    /// Every crew member under a commander's squad order under arms,
    /// alarm or no alarm — an order works with the alarm and without it
    /// (feature 78) — and let go again when the order ends, unless the
    /// alarm is holding it. A player's own Bim is never in a squad
    /// order and is never touched.
    fn muster_squad(&mut self, alarm: bool) {
        if self.hostile_bodies {
            return;
        }
        for who in 0..self.bims.len() {
            if self.is_player(who) || !self.bims[who].is_alive() {
                continue;
            }
            let under = self.squad_of(who) != Squad::None;
            let recruited = self.bims[who].character.is_recruited();
            let was_the_order_s = self.squad_armed.get(who).copied().unwrap_or(false);
            if under && !recruited {
                self.take_up_arms(who);
                self.bims[who].character.set_post(None);
                self.bims[who].character.set_recruited(true);
            } else if !under && !alarm && recruited && was_the_order_s {
                // Only what the order put under arms is let go by its
                // ending: a Bim somebody else recruited stays recruited.
                self.bims[who].character.set_recruited(false);
            }
            if self.squad_armed.len() <= who {
                self.squad_armed.resize(who + 1, false);
            }
            self.squad_armed[who] = under;
        }
    }

    /// Every unbroken piece of armour in the pack put on, where the part
    /// wears nothing or a broken piece: what a melee bot does at the
    /// alarm. A better piece already worn is left alone.
    fn wear_what_it_has(&mut self, who: usize) {
        for cell in 0..PACK_CELLS {
            let Some(Item::Armour(piece)) = self.bims[who].gear.pack[cell] else {
                continue;
            };
            if piece.broken() {
                continue;
            }
            let worn = self.bims[who].gear.worn(piece.kind.slot());
            if worn.is_none_or(|w| w.broken()) {
                self.equip(who, cell);
            }
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

    /// Whether the room is **calm** enough for anybody to go and doctor a
    /// crewmate: nothing fired and no blow landed here for [`CALM_AFTER`],
    /// nobody on this side with an enemy in sight for as long, and no
    /// enemy within [`CALM_RANGE`] tiles of any of them right now. A
    /// fight's lull, in other words — twenty seconds of quiet with the
    /// enemy out of sight and out of reach — rather than the fight's end,
    /// which nobody in the room can know.
    pub fn calm(&self) -> bool {
        self.combat.lull() >= CALM_AFTER
            && self.enemy_unseen_for >= CALM_AFTER
            && !self.enemy_within(CALM_RANGE * TILE)
    }

    /// Whether the crew's alarm is up: an enemy within [`ALARM_RANGE`] or
    /// in sight within [`ALARM_HOLD`], or a crew member hit within as long. The app reads it for the
    /// header; every crew member but the player's is under arms while it is.
    pub fn is_alarmed(&self) -> bool {
        self.alarm
    }

    // --- the machines' step (feature 83) --------------------------------------

    /// One step of every machine on the deck, after the crew: the body
    /// moved, a stand chosen, the doors forced, and the arm fired or
    /// swung. A droid is only ever in a **hostile** room, so nothing it
    /// does flies here — every shot is a `combat::Shot` for the world to
    /// fire in the crew's room, exactly as a hostile Bim's is.
    ///
    /// It is the hostile half of [`Game::tick_combat`] written for a
    /// body with no gear, no needs and no errands: what is left is the
    /// walk, the lock, the aim and the trigger.
    fn tick_droids(&mut self, dt: f32, war: bool) {
        for i in 0..self.droids.len() {
            self.droids[i].walk(dt);
            if self.droids[i].destroyed {
                continue;
            }
            // A machine held for a picture stands where it was put and
            // does nothing at all (`World::stage_droids_for_probe`).
            if self.droids[i].posing {
                continue;
            }
            self.droids[i].trigger.tick(dt);
            self.droids[i].melee_timer = (self.droids[i].melee_timer - dt).max(0.0);
            if let Some(blow) = self.droids[i].blow.as_mut() {
                blow.left -= dt;
            }
            let stats = self.droids[i].stats();
            let weapon = self.droids[i].weapon;
            if !war {
                // Nothing to fight: it stands where it was posted.
                self.droids[i].trigger.hold();
                self.droids[i].locked = None;
                self.droids[i].blow = None;
                self.droids[i].peek = None;
                self.droids[i].charging(dt, false);
                continue;
            }
            self.plan_droid_stand(i, dt, &stats);
            // And, with nobody it can get to, the doors in the way: an
            // unlocked one opens for it as it walks up (`update_doors`),
            // and a locked one is heaved at like a boarder's.
            self.breach_droid(i, dt);
            let from = self.droids[i].pos;

            // Whose the machines' targets are, and where the list turns
            // from the world's into this room's own (feature 94): in
            // every room but a town under attack these are the room's
            // one list and its whole length, and the code below reads
            // exactly as it did.
            let cross = self.combat.machine_cross();

            // A swing that has been swung lands now, on the target it
            // was aimed at if that one is still within reach.
            if let Some(blow) = self.droids[i].blow.take_if(|b| b.left <= 0.0) {
                if blow.target < cross {
                    // Across the seam: a melee `Shot` for the world to
                    // carry, if the target is still within reach.
                    let at = self
                        .combat
                        .machine_targets()
                        .get(blow.target)
                        .copied()
                        .flatten()
                        .filter(|t| (t.at - from).len() <= MELEE_RANGE * TILE)
                        .map(|t| t.at);
                    if let Some(at) = at {
                        self.combat.brawl_at(
                            from,
                            blow.target,
                            at,
                            weapon,
                            blow.damage,
                            blow.cut,
                            true,
                            None,
                        );
                    }
                } else {
                    // A body of this room (feature 94): the blow lands
                    // here, and `enemy_strike` re-checks the reach.
                    self.enemy_strike(from, blow.target - cross, blow.damage, blow.cut);
                }
            }

            // A melee first: locked, it neither aims nor fires.
            let locked = Combat::melee_among(
                self.combat.machine_targets(),
                &self.room.sight,
                from,
                &stats,
            );
            self.droids[i].locked = locked;
            if let Some(enemy) = locked {
                self.droids[i].trigger.hold();
                self.droids[i].peek = None;
                self.droids[i].charging(dt, false);
                if let Some(at) = self.combat.machine_targets()[enemy].map(|t| t.at) {
                    self.droids[i].face((at - from).angle());
                }
                if self.droids[i].melee_timer <= 0.0 {
                    self.droids[i].melee_timer = MELEE_PERIOD;
                    // A claw's blow is a crush, not a cut: it does not
                    // bleed the way a blade's does. A gunner's machine
                    // with a claw at its throat has no fists to fall
                    // back on — it has an arm, and the arm is the gun —
                    // so a Trooper or a Warden locked in a melee jabs
                    // with what it has at `FIST_DAMAGE`.
                    let (damage, cut) = if stats.melee {
                        (stats.damage, false)
                    } else {
                        (FIST_DAMAGE, false)
                    };
                    self.droids[i].struck_out();
                    self.droids[i].blow = Some(Blow {
                        target: enemy,
                        left: SWING_TIME,
                        damage,
                        cut,
                    });
                }
                continue;
            }
            // A claw with nobody in reach has nothing to aim.
            if stats.melee {
                self.droids[i].peek = None;
                self.droids[i].charging(dt, false);
                continue;
            }
            let Some((mark, eye, at)) = Combat::aim_among(
                self.combat.machine_targets(),
                &self.room.sight,
                from,
                &stats,
                None,
            ) else {
                self.droids[i].trigger.hold();
                self.droids[i].peek = None;
                self.droids[i].charging(dt, false);
                continue;
            };
            // A machine that does not take cover does not peek either:
            // a Trooper shoots from its own eyes and walks into the open
            // to get them, rather than leaning round a wall it would be
            // shot at from behind.
            let takes_cover = self.droids[i].kind.takes_cover();
            if !takes_cover && eye != from {
                self.droids[i].trigger.hold();
                self.droids[i].peek = None;
                self.droids[i].charging(dt, false);
                continue;
            }
            // **A Trooper fires on the move**; everything else stands
            // still for the shot, since walking halves the odds.
            let walking = self.droids[i].is_walking();
            if walking && takes_cover && eye != from {
                self.droids[i].trigger.hold();
                self.droids[i].peek = None;
                self.droids[i].charging(dt, false);
                continue;
            }
            let peek = (eye != from).then_some(eye);
            self.droids[i].peek = peek;
            if !walking {
                self.droids[i].face((at - eye).angle());
            }
            self.droids[i].charging(dt, true);
            if self.droids[i].trigger.pull(dt, &stats) {
                self.droids[i].fired();
                // The glow at the gun it is built with, not at the eye
                // the shot is traced from (feature 98).
                let muzzle = self.droids[i].muzzle();
                self.lit_muzzle(muzzle, at, weapon);
                if mark < cross {
                    // Across the seam, as a machine's shot always was:
                    // recorded here and flown by the world in the crew's
                    // room, where the body it is aimed at actually is.
                    self.combat.shoot(eye, at, weapon, walking);
                } else {
                    // At one of this room's own bodies (feature 94): the
                    // bolt flies **here**, hostile, and `Combat::step`
                    // finds the townsperson it was aimed at.
                    self.combat.fire(eye, at, weapon, true, walking);
                }
            }
        }
    }

    /// Where a machine walks to: the same scoring a hostile Bim's stand
    /// is picked by, with the kind's own weight on cover — nought for a
    /// Trooper, which advances in the open. A Husk has a claw, so
    /// `stand_scored` hands it straight to [`Tactics::charge`].
    fn plan_droid_stand(&mut self, i: usize, dt: f32, stats: &WeaponStats) {
        self.droids[i].plan_wait -= dt;
        if self.droids[i].plan_wait > 0.0 {
            return;
        }
        self.droids[i].plan_wait = PLAN_EVERY;
        if !self.droids[i].can_move() {
            // Legs gone: it fights where it stands, and plans nothing.
            return;
        }
        let from = self.droids[i].pos;
        let nav = self.maps.for_body(false, self.room.bath.is_open());
        // The machines' own list where the world gave them one (feature
        // 94) — a town the crew are defending, where the people they came
        // for are in the room with them — else the room's.
        let targets = self.combat.machine_targets().to_vec();
        let nobody_in_sight = targets.iter().flatten().all(|t| t.stale);
        // The hunter's rule, as a hostile Bim's: with nobody in sight a
        // gunner walks to where something was last seen, and from then
        // on holds or closes and never gives ground.
        if !stats.melee && nobody_in_sight {
            self.droids[i].hunting = true;
            let Some(to) = Tactics::charge(nav, from, &targets) else {
                return;
            };
            if (to - self.droids[i].destination()).len() <= TILE {
                return;
            }
            let route = nav.path(from, to);
            if !route.is_empty() {
                self.droids[i].follow_path(route);
            }
            return;
        }
        let closing = self.droids[i].hunting;
        let doorways: Vec<Rect> = self
            .room
            .doors
            .iter()
            .map(|d| d.rect)
            .chain(self.room.airlocks.iter().copied())
            .collect();
        // Where the rest of its side stand, or are walking to: nobody's
        // stand but theirs, so a wave does not pile onto one tile.
        let taken: Vec<Vec2> = self
            .droids
            .iter()
            .enumerate()
            .filter(|&(j, d)| j != i && !d.destroyed)
            .map(|(_, d)| d.destination())
            .chain(
                self.bims
                    .iter()
                    .filter(|b| b.is_alive() && b.character.is_recruited())
                    .map(|b| b.character.destination().unwrap_or(b.character.pos)),
            )
            .collect();
        let cover_worth = if self.droids[i].kind.takes_cover() {
            crate::combat::COVER_WORTH
        } else {
            0.0
        };
        let Some(stand) = Tactics::stand_scored(
            &self.room.sight,
            nav,
            from,
            &targets,
            stats,
            &doorways,
            &taken,
            closing,
            cover_worth,
            crate::combat::DISTANCE_WORTH,
        ) else {
            return;
        };
        let to = stand.at;
        // A shot from here and nothing better than the open over there:
        // it stops where it is and shoots — except a Trooper, which
        // advances in the open and fires as it walks.
        let advances = !self.droids[i].kind.takes_cover();
        let has_a_shot = !stats.melee
            && Combat::aim_among(&targets, &self.room.sight, from, stats, None).is_some();
        if has_a_shot && !stand.cover && !advances {
            if self.droids[i].is_walking() {
                self.droids[i].halt();
            }
            return;
        }
        if (to - self.droids[i].destination()).len() <= TILE {
            return;
        }
        let route = nav.path(from, to);
        if !route.is_empty() {
            self.droids[i].follow_path(route);
        }
    }

    /// A machine with nobody it can get to goes for the doors: the same
    /// rule as [`Game::breach`], on the machine's own clock. One body to
    /// a door; a machine never locks one itself, so there is no lock of
    /// its own to undo.
    fn breach_droid(&mut self, i: usize, dt: f32) {
        let from = self.droids[i].pos;
        if let Some(d) = self.droids[i].smashing {
            let at_it = self.room.doors.get(d).is_some_and(|door| {
                door.locked && (door.station(from, DOOR_STAND_OFF) - from).len() <= TILE * 0.75
            });
            if at_it {
                if self.droids[i].is_walking() {
                    self.droids[i].halt();
                }
                let door = &mut self.room.doors[d];
                let centre = door.rect.center();
                // A machine heaves at the rate anybody without the tank's
                // *breacher* does.
                if let Some(cue) = door.smash_at(self.bims.len() + i, dt, 1.0) {
                    self.room.cues.push(Cued { cue, at: centre });
                }
                if !self.room.doors[d].locked {
                    self.droids[i].smashing = None;
                }
                return;
            }
            if let Some(door) = self.room.doors.get_mut(d)
                && door.smash.is_some_and(|s| s.by == self.bims.len() + i)
            {
                door.drop_smash();
            }
            self.droids[i].smashing = None;
        }
        self.droids[i].breach_wait -= dt;
        if self.droids[i].breach_wait > 0.0 {
            return;
        }
        self.droids[i].breach_wait = PLAN_EVERY;
        if !self.droids[i].can_move() {
            return;
        }
        let nav = self.maps.for_body(false, self.room.bath.is_open());
        let goals: Vec<Vec2> = self
            .combat
            .target_positions()
            .into_iter()
            .flatten()
            .collect();
        if goals.is_empty() || goals.iter().any(|&t| !nav.path(from, t).is_empty()) {
            return;
        }
        let me = self.bims.len() + i;
        let mut best: Option<(f32, usize, Vec2)> = None;
        for (d, door) in self.room.doors.iter().enumerate() {
            if !door.locked || door.smash.is_some_and(|s| s.by != me) {
                continue;
            }
            let at = door.station(from, DOOR_STAND_OFF);
            let near = (at - from).len() <= TILE * 0.75;
            if !near && nav.path(from, at).is_empty() {
                continue;
            }
            let cost = (at - from).len();
            if best.is_none_or(|(c, _, _)| cost < c) {
                best = Some((cost, d, at));
            }
        }
        let Some((_, d, at)) = best else {
            return;
        };
        if (at - from).len() <= TILE * 0.75 {
            self.droids[i].halt();
            self.droids[i].smashing = Some(d);
            return;
        }
        let route = nav.path(from, at);
        if !route.is_empty() {
            self.droids[i].follow_path(route);
        }
    }

    /// One bot's choice of where to stand, when its clock comes round:
    /// `Tactics::stand_with_cover`, and a march there if it is more than a
    /// tile from where it is already going — **unless it has a shot from
    /// where it is and the stand is not cover**: walking halves the odds
    /// (`WALKING_ACCURACY`), so a bot that can shoot stands still to do
    /// it, and moves only to reach cover or because it has no shot at all
    /// (nothing in sight, or out of reach). Not a post — it is recruited,
    /// so it stays wherever it arrives without one — and no marker on the
    /// deck, since a ping where an enemy is about to stand would be a
    /// picture through the fog.
    ///
    /// **A hostile gunner with nobody in sight hunts** (September 2026):
    /// with every target stale — believed, seen by nobody on its side —
    /// it goes to where it last saw the nearest of them the way a blade
    /// charges ([`Tactics::charge`]), and picks a stand again the moment
    /// one is in sight. A stand scored against a belief was a spot with
    /// a view of the doorway the quarry went through, at the far end of
    /// the weapon's range, and a hunter that stood there never followed
    /// anybody through a passage. The crew's bots take the list as it
    /// comes and never hunt: they are the player's to send.
    fn plan_stand(&mut self, who: usize, dt: f32, stats: &WeaponStats, mark: Option<usize>) {
        let bim = &mut self.bims[who];
        bim.plan_wait -= dt;
        if bim.plan_wait > 0.0 {
            return;
        }
        bim.plan_wait = PLAN_EVERY;
        let from = bim.character.pos;
        let nav = self.maps.for_body(false, self.room.bath.is_open());
        // A squad member attacking a marked enemy advances on **it**
        // (feature 78): the rest are nobody's business for the stand,
        // so they are taken off the list the spot is scored against.
        let mut targets = self.combat.targets().to_vec();
        if let Some(mark) = mark
            && targets.get(mark).is_some_and(|t| t.is_some())
        {
            for (i, t) in targets.iter_mut().enumerate() {
                if i != mark {
                    *t = None;
                }
            }
        }
        let targets = targets;
        let nobody_in_sight = targets.iter().flatten().all(|t| t.stale);
        if self.hostile_bodies && !stats.melee && nobody_in_sight {
            bim.hunting = true;
            let Some(to) = Tactics::charge(nav, from, &targets) else {
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
            return;
        }
        // And a hunter holds or closes for the rest of the war, never gives
        // ground — the other half of the hunt: a stand back at the far end
        // of its range lost it the quarry it had just found, and it hunted
        // and stepped back by turns in the doorway for ever. A rifle that
        // has its target in sight from the start was never hunting, and
        // walks off to its range as it always did.
        let closing = self.hostile_bodies && bim.hunting;
        // Every doorway, open or shut: none is a stand, since a door opens
        // for whoever comes to stand in it. See `Tactics::stand`.
        let doorways: Vec<Rect> = self
            .room
            .doors
            .iter()
            .map(|d| d.rect)
            .chain(self.room.airlocks.iter().copied())
            .collect();
        // Where the rest of its side stand, or are walking to: nobody's
        // stand but theirs.
        let taken: Vec<Vec2> = self
            .bims
            .iter()
            .enumerate()
            .filter(|&(i, b)| {
                i != who && b.is_alive() && b.character.is_recruited() && !b.character.is_outside()
            })
            .map(|(_, b)| b.character.destination().unwrap_or(b.character.pos))
            .collect();
        let Some(stand) = Tactics::stand_with_cover(
            &self.room.sight,
            nav,
            from,
            &targets,
            stats,
            &doorways,
            &taken,
            closing,
            // A field medic weighs distance heavily and so stands at the
            // far end of its reach (feature 86); everybody else takes
            // the plain preference for distance.
            if self.is_field_medic(who) {
                crate::combat::KEEP_BACK_WORTH
            } else {
                crate::combat::DISTANCE_WORTH
            },
        ) else {
            return;
        };
        let to = stand.at;
        // A shot from here, and nothing better than the open over there:
        // it stops where it is and shoots. A walk already under way is
        // kept only for cover.
        let has_a_shot = !stats.melee && self.combat.aim(&self.room.sight, from, stats).is_some();
        let bim = &mut self.bims[who];
        if has_a_shot && !stand.cover {
            if bim.character.is_walking() {
                bim.character.halt();
            }
            return;
        }
        let going = bim.character.destination().unwrap_or(from);
        if (to - going).len() <= TILE {
            return;
        }
        let route = nav.path(from, to);
        if !route.is_empty() {
            self.bims[who].character.follow_path(route);
        }
    }

    /// An enemy with nobody it can get to goes for the doors. On its own
    /// clock like a stand: if a route to any target is open it does
    /// nothing here; otherwise the nearest locked door whose panel it can
    /// reach — one it locked itself first, which it unlocks on arrival
    /// (`door::Locker::Body`), else one it **smashes**: standing at the
    /// panel it heaves at the door every frame (`Door::smash`),
    /// `SMASH_DOOR` or `SMASH_AIRLOCK` seconds of it, and the lock gives.
    /// One body to a door; a second finds another door or waits. The
    /// crew's side never does this — only a hostile room's people, since
    /// the crew are the player's to send.
    ///
    /// Off war the same goes for a **post** it cannot get to (September
    /// 2026): a hostile body sent somewhere behind a locked door —
    /// a raider's boarders, posted at the ship's gangway with the airlock
    /// shut against them — forces the door in its way and then walks on
    /// (`return_to_post`).
    fn breach(&mut self, who: usize, dt: f32) {
        let from = self.bims[who].character.pos;
        // Heaving at the door it stands at: every frame, until it gives or
        // the body has moved off.
        if let Some(i) = self.bims[who].smashing {
            let at_it = self.room.doors.get(i).is_some_and(|d| {
                d.locked && (d.station(from, DOOR_STAND_OFF) - from).len() <= TILE * 0.75
            });
            if at_it {
                if self.bims[who].character.is_walking() {
                    self.bims[who].character.halt();
                }
                // *Breacher* (feature 77): a tank heaves the lock open in
                // half the time; everybody else's rate is one.
                let rate = self.skill(who).smash_rate;
                let door = &mut self.room.doors[i];
                let centre = door.rect.center();
                if let Some(cue) = door.smash_at(who, dt, rate) {
                    self.room.cues.push(Cued { cue, at: centre });
                }
                if !self.room.doors[i].locked {
                    self.bims[who].smashing = None;
                }
                return;
            }
            if let Some(door) = self.room.doors.get_mut(i)
                && door.smash.is_some_and(|s| s.by == who)
            {
                door.drop_smash();
            }
            self.bims[who].smashing = None;
        }

        let bim = &mut self.bims[who];
        bim.breach_wait -= dt;
        if bim.breach_wait > 0.0 {
            return;
        }
        bim.breach_wait = PLAN_EVERY;
        let nav = self.maps.for_body(false, self.room.bath.is_open());
        // What it wants to get to: at war its targets, believed or seen;
        // off war its post, if it has one.
        let goals: Vec<Vec2> = if self.at_war {
            self.combat
                .target_positions()
                .into_iter()
                .flatten()
                .collect()
        } else {
            self.bims[who].character.post().into_iter().collect()
        };
        if goals.is_empty() || goals.iter().any(|&t| !nav.path(from, t).is_empty()) {
            return;
        }
        // The nearest locked door it can get to the panel of, its own
        // locks before anybody else's — those cost nothing to open.
        let mut best: Option<(f32, usize, Vec2)> = None;
        for (i, door) in self.room.doors.iter().enumerate() {
            if !door.locked || door.smash.is_some_and(|s| s.by != who) {
                continue;
            }
            let at = door.station(from, DOOR_STAND_OFF);
            let near = (at - from).len() <= TILE * 0.75;
            if !near && nav.path(from, at).is_empty() {
                continue;
            }
            let own = door.locked_by == door::Locker::Body(who);
            let cost = (at - from).len() + if own { 0.0 } else { 1.0e6 };
            if best.is_none_or(|(c, _, _)| cost < c) {
                best = Some((cost, i, at));
            }
        }
        let Some((_, i, at)) = best else {
            return;
        };
        if (at - from).len() <= TILE * 0.75 {
            if self.room.doors[i].locked_by == door::Locker::Body(who) {
                self.room.doors[i].unlock();
                self.bims[who].sealed_in = None;
            } else {
                self.bims[who].smashing = Some(i);
            }
            return;
        }
        let going = self.bims[who].character.destination().unwrap_or(from);
        if (at - going).len() <= TILE * 0.5 {
            return;
        }
        let route = nav.path(from, at);
        if route.is_empty() {
            return;
        }
        if self.bims[who].task.is_some() {
            self.interrupt(who);
        }
        self.bims[who].character.follow_path(route);
    }

    /// A fleeing enemy that has just come through a door locks it behind
    /// itself, and sealed in it binds its wounds: every [`BIND_EVERY`]
    /// seconds one wounded part is dressed, or its trauma treated once no
    /// wound is open on it — a field dressing out of its own pockets — and
    /// with nothing bleeding it is dying no more, stops running, unlocks
    /// its door (`breach`, its own lock first) and fights again. The
    /// crew's own fleeing Bims do neither: a crewmate with a kit sees to
    /// them, and locking the crew out of their own rooms is nobody's idea.
    fn seal_and_bind(&mut self, who: usize, dt: f32) {
        // Through the door its run took it to: the far side, and clear of
        // the opening, so the leaves can shut.
        if let Some((i, side)) = self.bims[who].seal {
            match self.room.doors.get(i) {
                Some(door) if door.locked => self.bims[who].seal = None,
                Some(door) => {
                    let from = self.bims[who].character.pos;
                    let across = (from - door.rect.center()).dot(door.through());
                    if across.signum() != side.signum() && across.abs() > TILE * 0.9 {
                        self.room.doors[i].lock(door::Locker::Body(who));
                        self.bims[who].seal = None;
                        self.bims[who].sealed_in = Some(i);
                    }
                }
                None => self.bims[who].seal = None,
            }
        }
        let sealed = self.bims[who]
            .sealed_in
            .is_some_and(|i| self.room.doors.get(i).is_some_and(|d| d.locked));
        if !sealed {
            self.bims[who].bind_timer = 0.0;
            return;
        }
        let bim = &mut self.bims[who];
        bim.bind_timer += dt;
        if bim.bind_timer < BIND_EVERY {
            return;
        }
        bim.bind_timer = 0.0;
        // The part bleeding most, dressed; a part at nothing with no wound
        // left on it, treated.
        let worst = Part::ALL
            .into_iter()
            .max_by_key(|&p| bim.health.wounds(p))
            .filter(|&p| bim.health.wounds(p) > 0);
        match worst {
            Some(part) => {
                bim.health.bandage(part);
            }
            None => {
                if let Some(part) = Part::ALL
                    .into_iter()
                    .find(|&p| bim.health.trauma(p).is_some())
                {
                    bim.health.treat(part);
                }
            }
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
        let from = self.bims[who].character.pos;
        // Round the nearest player's own that is up and in; with several
        // players the crew gather round whichever is closest.
        let player = (0..self.players.min(self.bims.len()))
            .filter(|&p| self.bims[p].is_alive() && !self.bims[p].character.is_outside())
            .map(|p| self.bims[p].character.pos)
            .min_by(|a, b| (*a - from).len().total_cmp(&(*b - from).len()))
            .unwrap_or(self.bims[PLAYER].character.pos);
        self.gather_round(who, dt, player);
    }

    /// The same ring round a point of the room rather than round a
    /// player's Bim: what a commander's **fall back** puts the squad in
    /// (feature 78), anchored on the tile he named instead of on
    /// somebody who moves.
    fn gather_round(&mut self, who: usize, dt: f32, anchor: Vec2) {
        let bim = &mut self.bims[who];
        bim.plan_wait -= dt;
        if bim.plan_wait > 0.0 {
            return;
        }
        bim.plan_wait = PLAN_EVERY;
        let from = bim.character.pos;
        let rank = (1..who).filter(|&i| self.bims[i].is_alive()).count();
        let nav = self.maps.for_body(false, self.room.bath.is_open());
        let player = anchor;
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

    // --- the bots a player leads (feature 84) ------------------------------

    /// What each player's standing order is, by slot
    /// ([`Standing`]): the world's word, said every step.
    pub fn set_standing(&mut self, standing: Vec<Standing>) {
        self.standing = standing;
    }

    /// Which order a bot is under: the one given by the player whose own
    /// Bim is nearest it, counting only players that are alive and on the
    /// deck. [`Standing::Follow`] with nobody up, which is the same as
    /// nobody having said anything.
    fn standing_for(&self, who: usize) -> Standing {
        let from = self.bims[who].character.pos;
        (0..self.players.min(self.bims.len()))
            .filter(|&p| self.bims[p].is_alive() && !self.bims[p].character.is_outside())
            .min_by(|&a, &b| {
                (self.bims[a].character.pos - from)
                    .len()
                    .total_cmp(&(self.bims[b].character.pos - from).len())
            })
            .and_then(|p| self.standing.get(p).copied())
            .unwrap_or_default()
    }

    /// That order, for the tests and the world's readout.
    pub fn standing_for_probe(&self, who: usize) -> Standing {
        self.standing_for(who)
    }

    /// Whether a body is giving ground backwards — falling back with
    /// the gun on the enemy rather than running (feature 84). For the
    /// tests; the picture reads it off the character itself.
    pub fn is_backing_for_probe(&self, who: usize) -> bool {
        self.bims.get(who).is_some_and(|b| b.character.is_backing())
    }

    /// Whether a point of the room is **the ship's own deck** — which is
    /// the crew's last stand (feature 84). What `set_foreign` said is
    /// what answers it: a joined deck's station box is somebody else's
    /// and everything else the ship's; a planet's is the other way
    /// about, the ship's own hull box against the town and the ground.
    /// With nothing foreign said the whole room is the ship's, which is
    /// what a ship flying on its own is.
    pub fn is_aboard(&self, at: Vec2) -> bool {
        match self.foreign {
            None => true,
            Some((rect, _, false)) => !rect.contains(at),
            Some((rect, _, true)) => rect.contains(at),
        }
    }

    /// Where a body falling back to the ship makes for: the deck just
    /// inside the **ship's** port, which is the one spot every joined
    /// deck and every planet has and which is aboard by either rule.
    ///
    /// The world says where that is ([`Game::set_home`]) and the room's
    /// own `gangway` is only the fallback, for a ship flying alone and
    /// for the classic test room. It has to be that way round: on a
    /// joined deck `Room::gangway` is the *joined* design's first free
    /// airlock, and the ship's own is mated to the station, so the room
    /// answers the station's far door — which is how a retreat used to
    /// walk the crew out through the building rather than home.
    /// The middle of the room for a ship with no port at all — the
    /// classic room, a design without one — since there is nowhere else
    /// to mean.
    pub(crate) fn ship_anchor(&self) -> Vec2 {
        match self.home.or(self.room.gangway) {
            Some(at) => at,
            None => match self.foreign {
                Some((rect, _, true)) => rect.center(),
                _ => self.room.interior.center(),
            },
        }
    }

    /// Where the ship's own gangway is, in room units, said by the world
    /// every step (feature 84). `None` leaves the room to its own
    /// `gangway`, which is right for a ship flying alone.
    pub fn set_home(&mut self, at: Option<Vec2>) {
        self.home = at;
    }

    /// The spot a fall back gathers on, for the app's defend sign.
    pub fn fall_back_point(&self) -> Vec2 {
        self.ship_anchor()
    }

    /// Whether the fight has come **aboard**: any target that is up
    /// standing on the ship's own deck. This is what makes the last
    /// stand — a body aboard with one of these does not run and does not
    /// fall back, whatever it was told.
    fn enemy_aboard(&self) -> bool {
        self.combat
            .targets()
            .iter()
            .flatten()
            .any(|t| self.is_aboard(t.at))
    }

    /// Whether `who` is making the last stand: aboard the ship with the
    /// enemy aboard it too. Read by [`Game::would_flee`] as well as by
    /// the bots' orders, so a dying crew member cornered in its own ship
    /// fights where it stands rather than running further in.
    /// **A room with nowhere else in it is not a last stand.** The rule
    /// wants a ship to be cornered *in*, which means a deck with
    /// somebody else's half to it — a joined station, a raider tied on,
    /// a town on a planet ([`Game::set_foreign`]). The classic test room
    /// and a ship flying alone have none, so a dying body there runs the
    /// way it always did.
    fn cornered(&self, who: usize) -> bool {
        !self.hostile_bodies
            && self.foreign.is_some()
            && self.is_aboard(self.bims[who].character.pos)
            && self.enemy_aboard()
    }

    /// What a bot under arms does when nothing nearer to hand — a chain,
    /// a post its player clicked for it, a commander's squad order — has
    /// claimed it (feature 84). The last stand first, then a dying run,
    /// then whatever its player's standing order is.
    fn bot_stand(&mut self, who: usize, dt: f32, stats: &WeaponStats) {
        // A field medic's business is the fallen (feature 86), and it
        // comes before the last stand and before its player's standing
        // order: a body bleeding out on the deck waits for neither. With
        // nobody to fetch it falls through to the rest of this and
        // fights — from the far end of its reach, which is `plan_stand`'s
        // own doing.
        if self.is_field_medic(who) && self.rescue(who, dt) {
            return;
        }
        // A town's defender has no player to gather round and nowhere
        // to fall back to (feature 94): it is defending the place it
        // lives in, so it picks its own stand and fights.
        if self.under_attack() || self.cornered(who) {
            self.plan_stand(who, dt, stats, None);
            return;
        }
        match self.standing_for(who) {
            Standing::Retreat => self.fall_back_aboard(who, dt),
            Standing::Attack { at } => self.assault(who, dt, stats, at),
            Standing::Follow => {
                let from = self.bims[who].character.pos;
                // Somebody's own to gather round: any player's, up and in.
                let player_up = (0..self.players.min(self.bims.len()))
                    .any(|p| self.bims[p].is_alive() && !self.bims[p].character.is_outside());
                if !player_up || self.combat.sees_any(&self.room.sight, from) {
                    self.plan_stand(who, dt, stats, None);
                } else {
                    self.gather(who, dt);
                }
            }
        }
    }

    /// Whether a point of the deck is out of the fight (feature 86): no
    /// target still up within [`RESCUE_CLEAR`] tiles of it, and nothing
    /// a body standing there could see. Both, because an enemy round the
    /// corner three tiles off is danger a line of sight says nothing
    /// about, and one across the hall in plain view is danger the
    /// distance says nothing about.
    fn out_of_harm(&self, at: Vec2) -> bool {
        let near = self
            .combat
            .targets()
            .iter()
            .flatten()
            .any(|t| (t.at - at).len() <= RESCUE_CLEAR * TILE);
        !near && !self.combat.sees_any(&self.room.sight, at)
    }

    /// Whether a body stands clear of the fight (feature 86): what
    /// [`Game::out_of_harm`] says of where it is. The world asks it of a
    /// field medic that has run dry, since a medic with no kit left is
    /// worth filling up again the moment it is somewhere safe to.
    pub fn is_out_of_harm(&self, who: usize) -> bool {
        self.bims
            .get(who)
            .is_some_and(|b| self.out_of_harm(b.character.pos))
    }

    /// The crewmate a field medic would go for: the nearest within
    /// [`RESCUE_LOOK`] that is out cold or in a dying state, is in
    /// nobody's arms already, still stands in the fire, and can be
    /// walked to. A body merely bleeding is the medical row's — it is
    /// on its feet and can walk itself out — and carrying one would be
    /// taking a crew member out of the fight for it.
    fn worth_fetching(&self, who: usize) -> Option<usize> {
        let from = self.bims[who].character.pos;
        let nav = self.maps.for_body(false, self.room.bath.is_open());
        let mut best: Option<(f32, usize)> = None;
        for other in 0..self.bims.len() {
            if other == who || !self.needs_rescue(other) || self.is_carried(other) {
                continue;
            }
            let down =
                self.bims[other].character.is_unconscious() || self.bims[other].health.dying();
            if !down || self.bims[other].carrying.is_some() {
                continue;
            }
            let at = self.bims[other].character.pos;
            if self.out_of_harm(at) {
                continue;
            }
            let gap = (at - from).len();
            if gap > RESCUE_LOOK * TILE || !nav.can_reach(from, at) {
                continue;
            }
            if best.is_none_or(|(b, _)| gap < b) {
                best = Some((gap, other));
            }
        }
        best.map(|(_, other)| other)
    }

    /// A field medic's own branch of [`Game::bot_stand`] (feature 86):
    /// carry whoever is in its arms clear of the fight and set them
    /// down there — the medical row takes over from that point, since
    /// doctoring wants the calm and the calm is what it has just walked
    /// to — else go and fetch the nearest crewmate down. Whether it
    /// claimed the body this step; `false` is "nothing to do", and the
    /// ordinary bot's stand follows.
    fn rescue(&mut self, who: usize, dt: f32) -> bool {
        if self.carrying(who).is_some() {
            let here = self.bims[who].character.pos;
            if self.out_of_harm(here) {
                self.set_down(who);
                return true;
            }
            // Away from the enemy, on its own plan clock: the dying
            // body's own run, walked with somebody in its arms.
            self.bims[who].plan_wait -= dt;
            if self.bims[who].plan_wait <= 0.0 {
                self.bims[who].plan_wait = PLAN_EVERY;
                let nav = self.maps.for_body(false, self.room.bath.is_open());
                let targets = self.combat.targets().to_vec();
                if let Some(to) = Tactics::flee(nav, here, &targets) {
                    let going = self.bims[who].character.destination().unwrap_or(here);
                    if (to - going).len() > TILE {
                        let route = nav.path(here, to);
                        if !route.is_empty() {
                            self.bims[who].character.follow_path(route);
                        }
                    }
                }
            }
            return true;
        }
        let Some(patient) = self.worth_fetching(who) else {
            return false;
        };
        if self.can_take_up(who, patient) {
            self.take_up(who, patient);
            return true;
        }
        // Over to it, on the plan clock like every other walk under
        // arms; the patient is a body that may have been dragged or
        // shot since, so the route is planned afresh each time.
        self.bims[who].plan_wait -= dt;
        if self.bims[who].plan_wait <= 0.0 {
            self.bims[who].plan_wait = PLAN_EVERY;
            let from = self.bims[who].character.pos;
            let at = self.bims[patient].character.pos;
            let nav = self.maps.for_body(false, self.room.bath.is_open());
            let to = nav.nearest_free(at);
            let route = nav.path(from, to);
            if !route.is_empty() {
                self.bims[who].character.follow_path(route);
            }
        }
        true
    }

    /// An **attack banner**: the bot fights its way to the point its
    /// player put down and holds it.
    ///
    /// * Anything up within its weapon's reach and it fights — its own
    ///   stand, cover and all, the same one it would pick if it had seen
    ///   the enemy for itself.
    /// * Nothing in reach and the banner still off, and it **pushes**:
    ///   the ground made good towards the banner with the cover on the
    ///   way taken where there is any ([`Tactics::advance`]), and the
    ///   whole walk planned instead where nothing near is any nearer —
    ///   a banner round a corner.
    /// * Nothing in reach and the banner reached, and it holds the ring
    ///   round it the way a fall back holds one.
    fn assault(&mut self, who: usize, dt: f32, stats: &WeaponStats, at: Vec2) {
        let from = self.bims[who].character.pos;
        let in_reach = self
            .combat
            .targets()
            .iter()
            .flatten()
            .any(|t| !t.stale && (t.at - from).len() <= stats.reach());
        if in_reach {
            self.plan_stand(who, dt, stats, None);
        } else if (at - from).len() > BANNER_HOLD * TILE {
            self.push_towards(who, dt, at);
        } else {
            self.gather_round(who, dt, at);
        }
    }

    /// Falling back: the ring round the ship's own gangway, and nothing
    /// further once it is there. The same walk a dying body makes, so
    /// that a retreat called and a body going down look the same on the
    /// deck.
    ///
    /// It is also walked differently from every other walk: the body is
    /// marked as falling back here, at a **sprint** — head down, a
    /// little faster than a walk — and the aim below turns that into a
    /// **backing** walk the step it finds something to shoot at, so a
    /// crew member covering the retreat gives ground with its gun up
    /// rather than turning its back on the enemy ([`FallBack`]).
    fn fall_back_aboard(&mut self, who: usize, dt: f32) {
        let anchor = self.ship_anchor();
        self.bims[who]
            .character
            .set_falling_back(Some(FallBack::Sprint));
        self.gather_round(who, dt, anchor);
    }

    /// One push towards a point, on the body's own plan clock: the spot
    /// [`Tactics::advance`] picks, or the whole way there when it picks
    /// none.
    fn push_towards(&mut self, who: usize, dt: f32, at: Vec2) {
        let bim = &mut self.bims[who];
        bim.plan_wait -= dt;
        if bim.plan_wait > 0.0 {
            return;
        }
        bim.plan_wait = PLAN_EVERY;
        let from = bim.character.pos;
        let nav = self.maps.for_body(false, self.room.bath.is_open());
        let targets = self.combat.targets().to_vec();
        let doorways: Vec<Rect> = self
            .room
            .doors
            .iter()
            .map(|d| d.rect)
            .chain(self.room.airlocks.iter().copied())
            .collect();
        let taken: Vec<Vec2> = self
            .bims
            .iter()
            .enumerate()
            .filter(|&(i, b)| {
                i != who && b.is_alive() && b.character.is_recruited() && !b.character.is_outside()
            })
            .map(|(_, b)| b.character.destination().unwrap_or(b.character.pos))
            .collect();
        let step = Tactics::advance(
            &self.room.sight,
            nav,
            from,
            at,
            &targets,
            &doorways,
            &taken,
            COVER_WORTH,
        );
        // Off the deck's box on a plain the cover lattice says nothing —
        // the grids are the body's own window out there — so the walk is
        // planned the way an order onto the plain is, leg by leg.
        let Some(to) = step else {
            if self.on_a_window(who, at) {
                self.plan_route(who, at);
            } else {
                let to = nav.nearest_free(at);
                let route = nav.path(from, to);
                if !route.is_empty() {
                    self.bims[who].character.follow_path(route);
                }
            }
            return;
        };
        let going = self.bims[who].character.destination().unwrap_or(from);
        if (to - going).len() <= TILE {
            return;
        }
        let route = nav.path(from, to);
        if !route.is_empty() {
            self.bims[who].character.follow_path(route);
        }
    }

    /// A squad member's stand under a commander's order (feature 78),
    /// in place of the ring round the player and of its own tactics:
    ///
    /// * **attack** — the stand its own tactics would pick, scored
    ///   against the marked enemy alone, so it advances on that one to
    ///   cover within range and peeks round it. A mark nobody can see is
    ///   still walked towards (*relentless*) and never fired at, which
    ///   is what `Target::stale` already does.
    /// * **fall back** — the gather ring anchored on the tile he named.
    /// * **stand ground** — exactly where it stands: a walk under way
    ///   is halted and nothing is planned.
    fn squad_stand(&mut self, who: usize, dt: f32, stats: &WeaponStats, squad: Squad) {
        match squad {
            Squad::None => {}
            Squad::Attack { enemy, .. } => self.plan_stand(who, dt, stats, Some(enemy)),
            Squad::FallBack { at } => {
                // A commander's fall back holds its fire while it walks
                // (below), so it is the **full sprint**: head down and a
                // little faster, never the backing walk a crew member
                // covering its own retreat gives ground with.
                self.bims[who]
                    .character
                    .set_falling_back(Some(FallBack::Sprint));
                self.gather_round(who, dt, at);
            }
            Squad::StandGround => {
                if self.bims[who].character.is_walking() {
                    self.bims[who].character.halt();
                }
            }
        }
    }

    /// One crew member's half of the frame.
    fn tick_bim(&mut self, who: usize, dt: f32, minutes: f32, bedtime: bool) {
        self.continue_far_walk(who);
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
        } else if self.needs_enabled
            && self.bims[who].health.drowsiness().nods_off()
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
        // Lying on the deck: the window opens with the first minute of it,
        // the allowance runs down, and at nothing the Bim gets up. Sore
        // from it for `SORE_LASTS` after the last minute.
        let on_deck = self.bims[who]
            .task
            .as_ref()
            .is_some_and(|t| t.sleeps_on_ground() && t.restoring() == Some(Need::Rest));
        if on_deck {
            let bim = &mut self.bims[who];
            if bim.ground_window <= 0.0 {
                bim.ground_window = GROUND_WINDOW;
            }
            bim.ground_left = (bim.ground_left - minutes).max(0.0);
            bim.sore = SORE_LASTS;
            if bim.ground_left <= 0.0
                && let Some(task) = bim.task.as_mut()
            {
                task.wake();
            }
        } else {
            self.bims[who].sore = (self.bims[who].sore - minutes).max(0.0);
        }
        if self.bims[who].ground_window > 0.0 {
            let bim = &mut self.bims[who];
            bim.ground_window -= minutes;
            if bim.ground_window <= 0.0 {
                bim.ground_window = 0.0;
                bim.ground_left = GROUND_SLEEP;
            }
        }
        let tiring = self.bims[who].health.stage().tiring()
            * if self.bims[who].is_sore() {
                crate::needs::SORE_TIRING
            } else {
                1.0
            };
        let purging = if self.bims[who].is_poisoned() {
            POISONED_PURGE
        } else {
            1.0
        };
        // Nothing drains with the needs off (feature 102): the levels
        // stand where they are, for the crew and for every station's people.
        if self.needs_enabled {
            self.bims[who].needs.update(dt, restoring, tiring, purging);
        }
        // The illness runs its course on its own clock, asleep or awake.
        self.bims[who].poisoned_for = (self.bims[who].poisoned_for - minutes).max(0.0);
        // A mouthful out of a bad pot. Read off what the Bim is doing this
        // instant rather than off the plate: the pot a plate was filled
        // from is the pot on the hob its chain picked.
        let bad_pot = self.bims[who]
            .task
            .as_ref()
            .is_some_and(|t| self.room.hob(t.picks().hob.unwrap_or(0)).food_bad);
        if self.needs_enabled && restoring == Some(Need::Food) && bad_pot {
            self.poison(who);
        }

        // Surroundings follows the deck rather than the clock: the tiles the
        // Bim is standing among, and what it has on itself.
        // With the needs off the mess is only a picture: it does nothing
        // to anybody standing in it.
        if self.needs_enabled {
            let grinding = self.room.filth.grinding(
                self.bims[who].character.pos,
                self.bims[who].character.filth(),
            );
            self.bims[who].needs.scrub(minutes, grinding);
            self.mind_the_mess(who, minutes, restoring == Some(Need::Rest));
        }

        // Going without is measured in time, not in how empty the bar is.
        // With the needs off nobody goes without, so nobody starves and
        // nobody is worn down for want of sleep: the body is told it has
        // eaten and slept, and what is left of the call is the blood, the
        // wounds and the mending, which a fight still needs.
        let (food, rest) = if self.needs_enabled {
            (
                self.bims[who].needs.level(Need::Food),
                self.bims[who].needs.level(Need::Rest),
            )
        } else {
            (1.0, 1.0)
        };
        // A medic's beam on the body holds its blood (feature 76).
        let held = self.held.get(who).copied().flatten();
        self.bims[who]
            .health
            .update_held(minutes, food, rest, restoring == Some(Need::Rest), held);
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
        // *Unmovable* (feature 77): low blood costs a tank no pace while
        // its kevlar is on and unbroken.
        let steady = self.skill(who).steady_pace
            && self.bims[who]
                .gear
                .worn(Part::Body)
                .is_some_and(|p| !p.broken());
        // *Grit* (feature 78): during a rally nothing the fight has done
        // to it costs it any pace at all.
        let hurt = if self.skill(who).unhurt {
            1.0
        } else if steady {
            self.bims[who].health.pace_steady()
        } else {
            self.bims[who].health.pace()
        };
        let pace = self.bims[who].health.stage().pace()
            * hurt
            * self.bims[who].ordeal.discomfort().pace()
            * self.bims[who].solitude.stage().works_at()
            * self.crowding(who)
            * self.runner(who)
            * self.skill(who).walk
            // A crewmate in the arms is a load (feature 86).
            * if self.bims[who].carrying.is_some() {
                CARRY_PACE
            } else {
                1.0
            };
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
                // And the gun goes on the deck: a body out cold holds
                // nothing, and it has to come back for it.
                self.drop_weapon(who);
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

        // Alone too long: the clock, and whatever it has just cost. Nobody
        // is lonely with the needs off.
        if self.needs_enabled {
            self.bear_the_solitude(who, minutes);
        }
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

        // A patient somebody is walking over to — to dress, or to treat —
        // holds still for them: whatever it was on is put down onto the
        // queue and the frame stops here for it, like a nap, until the
        // hands come off. A patient that walked off mid-way was minutes
        // lost and the walk begun again, for as long as it kept walking.
        // Not while it runs from a fight: the helper follows.
        if !self.is_fleeing(who) && self.is_being_seen_to(who) {
            if self.bims[who].task.is_some() {
                self.interrupt(who);
            }
            self.bims[who].character.halt();
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
                .is_some_and(|t| matches!(t.kind(), Kind::Bandage { .. } | Kind::Treat { .. }))
            && let Some(care) = self.medical_on_offer(who)
        {
            self.give_care(who, care);
        }
        // A bot that came round finds its hand empty and its gun on the
        // deck, and goes for it.
        self.fetch_own_weapon(who);

        let fumble = self.bims[who].health.drowsiness().fumble();
        // A Bim with nobody to talk to drags: a tenth longer over everything
        // it does. `Task` applies it only to the working steps — sleeping and
        // eating are not work and are not slowed — and the walking half of the
        // same tenth is in the pace above. A trauma on it, untreated or
        // lasting, slows the work the same way.
        let effort = self.bims[who].solitude.stage().works_at() * self.bims[who].health.works_at();
        // And what an engineer's talents do (feature 74): a factor on a
        // craft's working steps, another on a build's, and nothing on any
        // other errand — the task says which job it serves.
        let (craft, build) = self.work_factors.get(who).copied().unwrap_or((1.0, 1.0));
        // And a medic's (feature 76): its bandaging, its treating, and a
        // treatment with no kit at its own pace.
        let doctoring = self.doctoring_of(who);
        let effort = effort
            * match self.bims[who].task.as_ref().map(|t| t.kind()) {
                Some(Kind::Craft { .. }) => craft,
                Some(Kind::Build { .. }) => build,
                Some(Kind::Bandage { .. }) => doctoring.bandage,
                Some(Kind::Treat { bare: true, .. }) => doctoring.bare.unwrap_or(doctoring.treat),
                Some(Kind::Treat { .. }) => doctoring.treat,
                _ => 1.0,
            };
        // And a commander's aura (feature 78): every errand, not one
        // kind of it — the only factor here that is nobody's own class.
        let effort = effort * self.skill(who).effort;
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
        if self.needs_enabled {
            self.flee_filth(who, dt);
        }
        // And a station's people, with the needs off, keep to their
        // routine: a guard's round, a trader's desk, a worker's benches, a
        // stroll between rooms (feature 102, `crate::routine`).
        self.keep_to_routine(who, minutes);

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
        // And where the other room's people lie, for an execution to walk
        // to: a visitor the world marked down, where it was put.
        self.room.bodies_down.clear();
        self.room
            .bodies_down
            .extend(self.visitors.iter().enumerate().map(|(i, &at)| {
                self.visitors_down
                    .get(i)
                    .copied()
                    .unwrap_or(false)
                    .then_some(at)
            }));
    }

    /// One frame of the body: on the deck, kept clear of the walls and
    /// pushed out of the furniture; outside, on the outside grid's span,
    /// pushed out of the hull and the rocks. The one call that moves a Bim.
    fn move_body(&mut self, who: usize, dt: f32) {
        let outside = self.bims[who].character.is_outside();
        // A body on a window walk is held to the window and pushed out of
        // what is in it — bound beyond the deck's box as well as out past
        // it, or the box's edge would hold it in.
        let ch = &self.bims[who].character;
        let afield = ch.is_afield() || ch.far().is_some();
        let (interior, blockers) =
            match (outside, afield, self.maps.outside(), self.maps.afield(who)) {
                (true, _, Some(nav), _) => (nav.interior(), &self.outside_blockers),
                (_, true, _, Some(nav)) => (nav.interior(), &self.afield_blockers[who]),
                _ => (self.room.interior, &self.blockers),
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
        if self.room.builds.is_empty() && out.is_none() {
            self.maps.set_outside(None);
            self.outside_version = None;
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
        let solids = self.room.hull.clone();
        self.maps
            .set_outside(Some(Nav::outside(centre, &solids, BODY_MARGIN, tile)));
        self.outside_blockers = solids;
        self.outside_version = Some(self.room.rocks_version);
    }

    /// The plain, a window a body: who is out on it, and a grid for each
    /// of them.
    ///
    /// On a planet the room's own grids cover the deck and a margin of
    /// ground round it (`aboard::layout_of_on`) and no more; a body past
    /// that box is **afield**, told so here from where it stands, and
    /// walks a [`Nav::outside`] of its own — [`nav::OUTSIDE_RADIUS`] tiles
    /// every way about it, a cell a tile, over the ground's runs
    /// (`terrain::Plane::solids_in`) and whatever of the deck's solids
    /// fall in it — built again once it has walked [`nav::OUTSIDE_RECENTRE`]
    /// tiles from the middle. A body on the deck bound beyond the box
    /// (`Character::far`) keeps its window too, for the next leg; one on
    /// the deck with nowhere far to go has none. The chunks of ground
    /// nobody is near any more are let go of.
    fn refresh_afield(&mut self) {
        if self.room.plane.is_none() {
            if !self.afield_blockers.is_empty() {
                self.maps.clear_afield();
                self.afield_blockers.clear();
            }
            // A body carried in off a plain is on this deck now.
            for bim in &mut self.bims {
                bim.character.set_afield(false);
            }
            return;
        }
        let interior = self.room.interior;
        let tile = crate::filth::TILE;
        let n = self.bims.len();
        if self.afield_blockers.len() != n {
            self.afield_blockers.resize_with(n, Vec::new);
        }
        let mut about: Vec<(i32, i32)> = Vec::new();
        for who in 0..n {
            let pos = self.bims[who].character.pos;
            let afield = !interior.contains(pos);
            self.bims[who].character.set_afield(afield);
            // Everybody's chunks are kept, the deck's eyes' included.
            about.push(crate::terrain::Plane::tile_of(pos, tile));
            let wanted = afield || self.bims[who].character.far().is_some();
            if !wanted {
                if self.maps.afield(who).is_some() {
                    self.maps.set_afield(who, None);
                    self.afield_blockers[who].clear();
                }
                continue;
            }
            let stale = match self.maps.afield(who) {
                None => true,
                Some(nav) => (nav.middle() - pos).len() > nav::OUTSIDE_RECENTRE * tile,
            };
            if stale {
                self.build_window(who);
            }
        }
        if let Some(plane) = self.room.plane.as_mut()
            && !about.is_empty()
        {
            plane.trim(&about, 4 * nav::OUTSIDE_RADIUS);
        }
    }

    /// Body `who`'s window, built about where it stands now. See
    /// [`Game::refresh_afield`].
    fn build_window(&mut self, who: usize) {
        let tile = crate::filth::TILE;
        let pos = self.bims[who].character.pos;
        let (tx, ty) = crate::terrain::Plane::tile_of(pos, tile);
        let r = nav::OUTSIDE_RADIUS;
        let Some(plane) = self.room.plane.as_mut() else {
            return;
        };
        plane.load(tx - r, ty - r, tx + r, ty + r);
        let deck = plane.deck();
        let on_deck = move |x: i32, y: i32| x >= deck.0 && y >= deck.1 && x < deck.2 && y < deck.3;
        let mut solids = plane.solids_in(tx - r, ty - r, tx + r, ty + r, tile, &on_deck);
        // The deck's own, where the window overlaps it: every solid the
        // grids are built from, and the doors that are locked — a shut
        // one opens for a body walking up to it.
        let span = Rect::from_corners(
            vec2((tx - r) as f32 * tile, (ty - r) as f32 * tile),
            vec2((tx + r + 1) as f32 * tile, (ty + r + 1) as f32 * tile),
        );
        let touches = |s: &Rect| {
            s.max.x >= span.min.x
                && s.min.x <= span.max.x
                && s.max.y >= span.min.y
                && s.min.y <= span.max.y
        };
        solids.extend(self.room.solids().into_iter().filter(touches));
        solids.extend(self.room.locked_doors().into_iter().filter(touches));
        self.maps
            .set_afield(who, Some(Nav::outside(pos, &solids, BODY_MARGIN, tile)));
        if self.afield_blockers.len() <= who {
            self.afield_blockers.resize_with(who + 1, Vec::new);
        }
        self.afield_blockers[who] = solids;
    }

    /// A route for `who` to `to`, walked: on the deck's grid when both
    /// ends are on the deck, and on the body's window otherwise — the
    /// plain, or the way in from it — with what lies beyond the window
    /// kept as `far` for the legs after. True when there is a route.
    fn plan_route(&mut self, who: usize, to: Vec2) -> bool {
        let on_plane = self.room.plane.is_some();
        let ch = &self.bims[who].character;
        let window = on_plane && (ch.is_afield() || !self.room.interior.contains(to));
        if window {
            if self.maps.afield(who).is_none() {
                self.build_window(who);
            }
            let Some(nav) = self.maps.afield(who) else {
                return false;
            };
            return self.bims[who].character.walk_to(nav, to, true);
        }
        let nav = self
            .maps
            .for_body(ch.is_outside(), self.room.bath.is_open());
        self.bims[who].character.walk_to(nav, to, false)
    }

    /// The next leg of a walk bound beyond its window: once the leg on
    /// the window has ended, a fresh route from here — the window has
    /// been built again about the body by now — and, when there is none,
    /// the walk given up and the chain on it put down, as `unstick` does.
    fn continue_far_walk(&mut self, who: usize) {
        let ch = &self.bims[who].character;
        let Some(to) = ch.far() else {
            return;
        };
        if !ch.path_done() {
            return;
        }
        if !self.plan_route(who, to) {
            self.bims[who].character.follow_path(Vec::new());
            if self.bims[who].task.is_some() {
                self.interrupt(who);
            }
        }
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
        let route = self.nav_for(who).path(pos, to);
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

    /// Bodies under arms keep apart. The crew pass through each other in
    /// peace (`crowding`, below) because a route is planned once and a
    /// push at a chokepoint is a deadlock waiting to happen — but a fight
    /// replans every `PLAN_EVERY` and `unstick` watches the rest, and a
    /// squad stacked on one tile is one body to shoot at and a picture
    /// nobody can read. So every pair of **recruited** bodies on their
    /// feet on the deck — an enemy's people at war, the crew under the
    /// alarm — closer than `CREW_CLEARANCE` is shoved apart, each half the
    /// overlap, along the line between them (a nudge off the combat stream
    /// when they are on one spot). Both recruited, so a recruited James
    /// standing in the classic room is never moved by a Kate walking past,
    /// which is what the seeded probes rely on. A push into a wall is
    /// undone by the body's own push-out on its next move.
    fn separate_under_arms(&mut self) {
        let n = self.bims.len();
        let up = |b: &Bim| {
            b.is_alive()
                && b.character.is_recruited()
                && !b.character.is_seated()
                && !b.character.is_outside()
                && !b.character.is_unconscious()
        };
        // A body in somebody's arms is where the arms put it (feature
        // 86), so neither it nor its carrier is shoved off the other.
        let carried: Vec<bool> = (0..n).map(|w| self.is_carried(w)).collect();
        for a in 0..n {
            for b in (a + 1)..n {
                if carried[a] || carried[b] {
                    continue;
                }
                if !up(&self.bims[a]) || !up(&self.bims[b]) {
                    continue;
                }
                let between = self.bims[b].character.pos - self.bims[a].character.pos;
                let gap = between.len();
                if gap >= CREW_CLEARANCE {
                    continue;
                }
                let dir = if gap > 1e-3 {
                    between * (1.0 / gap)
                } else {
                    Vec2::from_angle(self.combat.roll() * TAU)
                };
                let shove = dir * ((CREW_CLEARANCE - gap) * 0.5);
                self.bims[a].character.pos = self.bims[a].character.pos - shove;
                self.bims[b].character.pos = self.bims[b].character.pos + shove;
            }
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
        let (restroom, surroundings) = (
            self.bims[who].needs.level(Need::Restroom),
            self.bims[who].needs.level(Need::Surroundings),
        );
        let (bims, rng) = (&mut self.bims, &mut self.rng);
        let purging = bims[who].is_poisoned();
        let mishap = bims[who].ordeal.update(
            minutes,
            restroom,
            surroundings,
            urge == Urge::Extreme,
            urge == Urge::Medium,
            purging,
            rng,
        );

        let at = self.bims[who].character.pos;
        // Relief is relief, however it comes: the need goes back up, and
        // everything else gets worse. Without that the Bim would foul the
        // deck again on the very next frame, for ever. What it is left
        // covered in is on the Bim until it washes, and wanting to is the
        // washing need dropping with it — to nothing, for an accident — so
        // the shower is the next errand where there is one.
        if mishap.fouled {
            let (on_bim, relief) = self.room.filth.foul(at);
            self.bims[who].character.soil(on_bim);
            self.bims[who].needs.soiled(on_bim);
            self.bims[who].needs.refill(Need::Restroom, relief);
            self.remember(who, What::Accident, 1);
            self.witnessed(who, What::SawAccident);
        } else if mishap.wet {
            let (on_bim, relief) = self.room.filth.wet(at);
            self.bims[who].character.soil(on_bim);
            self.bims[who].needs.soiled(on_bim);
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

        // Hopping about on the spot: the last stage only — see
        // `Urge::fidget_chance` — so it is the one warning the player gets
        // that an accident is coming, and the only outward sign of a need
        // that has no bar-width left to lose.
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
            || self.is_fleeing(who)
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

    /// The errand dropped for good rather than put down: suspended, so the
    /// hands and the scripting come back the way a suspend leaves them,
    /// and then not kept. What a hit does to a deploy.
    fn drop_task(&mut self, who: usize) {
        if let Some(task) = self.bims[who].task.take() {
            let _ = task.suspend(&mut self.bims[who].character, &mut self.room);
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
        // An order that moves a braced soldier is the end of the brace
        // (feature 75).
        self.bims[who].braced = false;
        let stale = self.bims[who].pending_move.take().is_some() && self.on_a_door_errand(who);
        if let Some(task) = self.bims[who].task.take() {
            let saved = task.suspend(&mut self.bims[who].character, &mut self.room);
            if !stale {
                self.bims[who].queue.insert(0, saved);
            }
        }
    }

    /// A plain order is the end of what was queued with Shift: the orders
    /// waiting their turn are dropped, and what the Bim had *put down* —
    /// its own errand, displaced by an order — stays to be picked up.
    /// RimWorld's rule, so a queue can be called off by giving any order
    /// without the key. The live orders call it ([`Game::order`],
    /// [`Game::order_move_for`]); an order begun off the queue does not.
    pub(crate) fn drop_ordered(&mut self, who: usize) {
        self.bims[who].queue.retain(|saved| !saved.is_ordered());
    }

    /// The spots the walks waiting their turn on `who`'s queue are bound
    /// for, in order — the Shift-clicks on the deck — for the picture and
    /// the probes; nothing for a Bim with none.
    pub fn queued_walks(&self, who: usize) -> Vec<Vec2> {
        self.bims
            .get(who)
            .map(|bim| {
                bim.queue
                    .iter()
                    .filter(|saved| matches!(saved.kind(), Kind::Walk { .. }))
                    .filter_map(|saved| saved.target())
                    .collect()
            })
            .unwrap_or_default()
    }

    /// How many orders wait their turn on `who`'s queue — given with
    /// Shift, not yet begun. For the probes.
    pub fn ordered_count(&self, who: usize) -> u32 {
        self.bims.get(who).map_or(0, |bim| {
            bim.queue.iter().filter(|s| s.is_ordered()).count() as u32
        })
    }

    /// Whether the Bim is walking a route it was *given* rather than
    /// running a chain: a route in hand with no errand behind it, which on
    /// the deck means a right-click's walk, a Shift-click's when its turn
    /// came, or the walk back to a post.
    ///
    /// It is worth asking because a walk is not a chain. An errand that
    /// interrupts a chain puts it on the queue and it is picked up again
    /// (`Game::interrupt`); an errand that interrupts a *walk* simply loses
    /// it — nothing is saved, nothing comes back — so the Bim ends up
    /// wherever the errand took it and a Shift chain behind it is walked
    /// from the wrong place. So the one errand a Bim starts on *somebody
    /// else* — going over for a word — leaves one alone (`free_to_talk`).
    /// Everything a Bim starts on itself already asks `arrived()`, which
    /// is the same rule said the other way round.
    fn on_a_given_walk(&self, who: usize) -> bool {
        self.bims
            .get(who)
            .is_some_and(|bim| bim.task.is_none() && !bim.character.arrived())
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

    /// Where `who` would stand for an order to `at`: the free cell nearest
    /// it on the grid the walk would be planned on — the body's window
    /// for a point on the plain, else the deck's.
    /// The grid `who`'s walk is on: its window while it is afield or bound
    /// beyond the deck's box, else the deck's — the outside's for a body
    /// out through the airlock.
    fn nav_for(&self, who: usize) -> &Nav {
        let ch = &self.bims[who].character;
        let on_window = ch.is_afield() || ch.far().is_some();
        match (on_window, self.maps.afield(who)) {
            (true, Some(nav)) => nav,
            _ => self
                .maps
                .for_body(ch.is_outside(), self.room.bath.is_open()),
        }
    }

    fn nearest_stand(&self, who: usize, at: Vec2) -> Vec2 {
        let ch = &self.bims[who].character;
        let window =
            self.room.plane.is_some() && (ch.is_afield() || !self.room.interior.contains(at));
        match (window, self.maps.afield(who)) {
            // Beyond the window the point is what it is: the stand there is
            // found when the walk gets there.
            (true, Some(nav)) if nav.interior().contains(at) => nav.nearest_free(at),
            (true, _) => at,
            _ => self.maps.pick(true).nearest_free(at),
        }
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
        // A braced soldier holds its ground the same way (feature 75).
        if self.bims[who].character.is_recruited()
            || self.bims[who].braced
            || self.is_fleeing(who)
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
        // An order given for later is begun now, the way its row or
        // right-click would have begun it — and dropped if it cannot be,
        // the way the row would have refused. A chain put down is picked
        // up where it was.
        if saved.is_ordered() {
            self.begin_ordered(who, &saved);
            return;
        }
        let taken = self.taken_for(who);
        self.bims[who].task = Some(Task::resume(
            saved,
            &mut self.bims[who].character,
            &mut self.room,
            &self.maps,
            &taken,
        ));
    }

    /// Begin an order that waited its turn on the queue (`Saved::ordered`)
    /// through the same door the live order goes through, so every check
    /// the row makes — something to cook, a way to the pan, a bunk or the
    /// deck's allowance — is made now, against the room as it is now. A
    /// walk is given the way a right-click gives one (`walk_order`): the
    /// door opened on the way, a cross on the deck if there is no way
    /// there after all, and the Bim posted at the spot when the order
    /// was one that posts. Whether anything began is the room's to show;
    /// what could not be is simply gone from the queue.
    fn begin_ordered(&mut self, who: usize, saved: &Saved) {
        let minutes = saved.rest_minutes();
        match saved.kind() {
            Kind::Walk { post } => {
                let Some(to) = saved.target() else {
                    return;
                };
                let code = self.walk_order(who, to.x, to.y);
                if post && (code == ORDER_MOVING || code == ORDER_VIA_DOOR) {
                    let spot = self.nearest_stand(who, to);
                    self.bims[who].character.set_post(Some(spot));
                }
            }
            Kind::Meal(dish) => {
                self.cook(who, dish);
            }
            Kind::Batch => {
                self.make_stew(who);
            }
            Kind::Reheat => {
                self.reheat(who);
            }
            Kind::Leftovers => {
                self.eat_leftovers(who);
            }
            Kind::Clean => {
                self.sweep_up(who);
            }
            Kind::Shower => {
                self.take_shower(who);
            }
            Kind::Heads => {
                self.use_toilet(who);
            }
            Kind::Rest => {
                self.rest(who, minutes);
            }
            Kind::Switch(which) => self.send_to_switch(who, which),
            Kind::Fetch { item } => {
                self.fetch(who, item);
            }
            Kind::Bandage { patient, part } => {
                if let Some(part) = Part::from_code(part) {
                    self.bandage(who, patient, part);
                }
            }
            Kind::Treat { patient, part, .. } => {
                if let Some(part) = Part::from_code(part) {
                    self.treat(who, patient, part);
                }
            }
            // Nothing a Shift-click can queue: the rest are the room's own
            // errands and the world's, never `Saved::ordered`.
            Kind::Tend { .. }
            | Kind::Chat
            | Kind::Craft { .. }
            | Kind::Build { .. }
            | Kind::Execute { .. }
            | Kind::Ferry { .. }
            | Kind::Deploy { .. } => {}
        }
    }

    /// Whether a queued walk could be given now: on the plain it is
    /// planned on the body's window when it is begun, so always; on the
    /// deck, a way there as the door stands, or with it open when it is
    /// not locked — a walk locked out waits for the door like an errand
    /// does, rather than being dropped.
    fn walk_ready(&self, who: usize, to: Vec2) -> bool {
        if self.on_a_window(who, to) {
            return true;
        }
        self.can_reach(who, to) || (!self.room.bath.locked && self.can_reach_through_door(who, to))
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
            // A sleep for a Bim with no bunk waits for the deck's allowance
            // to come back, like one waiting on a door: kept, not thrown
            // away, and everything else free to go on meanwhile.
            if saved.kind() == Kind::Rest
                && self.room.bed_of(who).is_none()
                && !self.can_sleep_on_ground(who)
            {
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
            // An order waiting its turn is begun, not resumed, so it asks
            // what beginning asks: one of everything free, and for a walk
            // a way there (`walk_ready`) — a galley in use or a locked
            // door is waited for, not dropped. What could not be begun
            // with nobody else aboard — leftovers with every pot empty —
            // is ready so that beginning it drops it, rather than
            // waiting on the agenda for a pot nobody is filling.
            if saved.is_ordered() {
                return match (saved.kind(), saved.target()) {
                    (Kind::Walk { .. }, Some(to)) => self.walk_ready(who, to),
                    (kind, _) => {
                        let from = self.bims[who].character.pos;
                        self.can_begin(who, kind)
                            || !task::can_pick_all(kind, &self.room, from, &task::Taken::default())
                    }
                };
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
        // Its bunk is nobody's now: the player's to give, or the next
        // `adopt`'s to hand to whoever has none.
        if let Some(bed) = self.room.sleeps_in.get_mut(who) {
            *bed = None;
        }
        // The gun it let go of going out cold lies beside the body, and
        // the body is what gets looted: back into its hand, so the Loot
        // window shows it and a crew looting a station's dead is not
        // reaching for a floor in the other room.
        if self.bims[who].gear.weapon.is_none()
            && let Some(i) = self.room.weapons_down.iter().position(|d| d.owner == who)
        {
            let d = self.room.weapons_down.remove(i);
            self.bims[who].gear.weapon = Some(d.weapon);
        }
    }

    /// Whether a body on this deck is still up: a Bim alive, or a
    /// machine not yet a wreck (feature 83).
    pub fn is_alive(&self, who: usize) -> bool {
        match self.droid_at(who) {
            Some(i) => !self.droids[i].destroyed,
            None => self.bims[who].is_alive(),
        }
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

    /// Post a Bim somewhere it may not be able to get to yet: [`Game::send_to`],
    /// but the post is kept when there is no route — the walk is planned
    /// again from [`Game::return_to_post`] as the way opens, and a hostile
    /// body forces a locked door in its way (`breach`). What a raider's
    /// boarders are posted at the ship's gangway with, the airlock locked
    /// against them or not. False only for nobody, or a body down.
    pub fn post_at(&mut self, who: usize, to: Vec2) -> bool {
        if who >= self.bims.len() || !self.is_alive(who) {
            return false;
        }
        if self.dispatch(who, to, true) {
            return true;
        }
        self.interrupt_for_order(who);
        let target = self.nearest_stand(who, to);
        self.bims[who].character.set_post(Some(target));
        true
    }

    /// Whether a Bim holds a post — the world re-posts a raider's boarders
    /// after a fight, which drops every post (`muster`).
    pub fn has_post(&self, who: usize) -> bool {
        self.bims
            .get(who)
            .is_some_and(|b| b.character.post().is_some())
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
        // Called back from the plain — the ship casting off — the same way
        // an order brings a body in: on its window, leg by leg.
        if !self.plan_route(who, to) {
            return false;
        }
        let target = self.nearest_stand(who, to);
        self.interrupt_for_order(who);
        // Putting the errand down may have stood the body up somewhere
        // else; a leg on a window is planned again from there.
        if self.room.plane.is_some() {
            self.plan_route(who, to);
        }
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

    // --- the needs switch, and the peacetime rounds (feature 102) -----------

    /// Switch the needs on or off for everybody in this room — see the
    /// field. The world says it every step; the classic room leaves it on.
    pub fn set_needs_enabled(&mut self, on: bool) {
        self.needs_enabled = on;
    }

    /// Whether the Bims here have needs at all.
    pub fn needs_enabled(&self) -> bool {
        self.needs_enabled
    }

    /// Deal a Bim its peacetime role and plan its round off this room's
    /// own fixtures (`crate::routine`), as reached from where it stands:
    /// `gates` are a town's openings in its wall, which the world knows
    /// and the room does not, and `seed` decides every choice. What the
    /// world does for a station's people when their room opens. False,
    /// and nothing dealt, for nobody or a body that is down.
    pub fn set_role(
        &mut self,
        who: usize,
        role: crate::routine::Role,
        gates: &[Vec2],
        seed: u64,
    ) -> bool {
        if who >= self.bims.len() || !self.bims[who].is_alive() {
            return false;
        }
        let from = self.bims[who].character.pos;
        let nav = self.maps.pick(true);
        let anchors = crate::routine::Anchors::of(&self.room, nav, from, gates);
        self.bims[who].routine = Some(crate::routine::plan(role, &anchors, seed));
        true
    }

    /// A Bim's round, if it has one.
    pub fn routine(&self, who: usize) -> Option<&crate::routine::Routine> {
        self.bims.get(who).and_then(|b| b.routine.as_ref())
    }

    /// Whether a Bim could walk from where it stands to every one of
    /// `points` over the deck's grid — for the tests of the rounds, which
    /// must never name a stop nobody can get to.
    pub fn reachable_for_probe(&self, who: usize, points: &[Vec2]) -> bool {
        let Some(bim) = self.bims.get(who) else {
            return false;
        };
        let nav = self.maps.pick(true);
        points.iter().all(|&p| nav.can_reach(bim.character.pos, p))
    }

    /// Take a Bim's round away: one of a station's people taken aboard,
    /// who is crew now and follows the crew's ways.
    pub fn clear_routine(&mut self, who: usize) {
        if let Some(bim) = self.bims.get_mut(who) {
            bim.routine = None;
            bim.character.set_lingering(false);
        }
    }

    /// One step of a Bim's round, with the needs off: on its way to the
    /// next stop, standing its time out at it, or setting off for the one
    /// after. **Only while the body is its own** — up, awake, not under
    /// arms, posted, sheltering, running, carrying or given an errand;
    /// any of those and the round waits where it stood, to be walked to
    /// again from wherever the body is when it is free. A stop it can find
    /// no route to is passed over for the next, one a step, so a round
    /// with a door locked across it goes round what it can reach.
    fn keep_to_routine(&mut self, who: usize, minutes: f32) {
        if self.bims[who].routine.is_none() {
            return;
        }
        let bim = &self.bims[who];
        let free = !self.needs_enabled
            && bim.is_alive()
            && !bim.character.is_unconscious()
            && !bim.character.is_recruited()
            && !bim.character.is_outside()
            && bim.character.post().is_none()
            && bim.task.is_none()
            && bim.queue.is_empty()
            && bim.carrying.is_none()
            && !bim.braced
            && !self.is_fleeing(who);
        if !free {
            let bim = &mut self.bims[who];
            bim.character.set_lingering(false);
            if let Some(routine) = bim.routine.as_mut() {
                routine.waiting = None;
            }
            return;
        }
        if !self.bims[who].character.arrived() {
            return;
        }
        let pos = self.bims[who].character.pos;
        let Some(routine) = self.bims[who].routine.as_mut() else {
            return;
        };
        let Some(stop) = routine.current() else {
            return;
        };
        match routine.waiting {
            Some(left) if left > minutes => {
                routine.waiting = Some(left - minutes);
            }
            Some(_) => {
                routine.advance();
                self.bims[who].character.set_lingering(false);
            }
            None if (pos - stop.at).len() <= crate::routine::AT_STOP => {
                routine.waiting = Some(stop.minutes);
                self.bims[who].character.set_lingering(true);
            }
            None => {
                self.bims[who].character.set_lingering(false);
                if !self.plan_route(who, stop.at)
                    && let Some(routine) = self.bims[who].routine.as_mut()
                {
                    routine.advance();
                }
            }
        }
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
        // By the same route an order takes: on a window, when the post is
        // out on the plain.
        self.plan_route(who, post);
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

    /// Where the board is, and where the heads' door is: where the cues
    /// probe expects a knife stroke and a door to be heard.
    #[allow(dead_code)]
    pub fn board_pos_for_probe(&self, i: usize) -> Vec2 {
        self.room.worktops[i].board.center()
    }

    #[allow(dead_code)]
    pub fn bath_door_for_probe(&self) -> Vec2 {
        self.room.bath.door.center()
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

    /// Where a Bim's walk on a window is really bound. For the probes.
    #[allow(dead_code)]
    pub fn far_for_probe(&self, who: usize) -> Option<Vec2> {
        self.bims[who].character.far()
    }

    /// What a Bim is on, as a code, for the probes: 0 nothing, else the
    /// chain's job code.
    #[allow(dead_code)]
    pub fn task_kind_for_probe(&self, who: usize) -> Option<u32> {
        self.bims[who]
            .task
            .as_ref()
            .map(|t| job_code(t.kind(), t.rest_minutes()))
    }

    /// Whether a Bim's walk is over. For the probes.
    #[allow(dead_code)]
    pub fn arrived_for_probe(&self, who: usize) -> bool {
        self.bims[who].character.arrived()
    }

    /// Whether a Bim is out on the plain. For the probes.
    #[allow(dead_code)]
    pub fn is_afield_for_probe(&self, who: usize) -> bool {
        self.bims[who].character.is_afield()
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

    /// Whether the Bim is asleep, in a bed or on its feet: the hands are
    /// on [`Action::Sleep`], which a doze in a bunk and a nap where it
    /// stands both set. What the world asks before letting one fly the
    /// ship (`World::at_the_helm`) — out cold is `is_unconscious`, and a
    /// different thing.
    pub fn is_asleep(&self, who: usize) -> bool {
        self.bims[who].character.action() == Action::Sleep
    }

    /// Drop the Bim off where it stands for `minutes`, for a probe: the
    /// nap the drowsiness rolls, given rather than rolled, and over when
    /// the minutes are.
    #[allow(dead_code)]
    pub fn nod_off_for_probe(&mut self, who: usize, minutes: f32) {
        self.bims[who].nap_left = minutes;
        self.bims[who].character.nod_off(true);
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
            || self.bims[who].braced
            || self.is_fleeing(who)
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
        //
        // With the needs off (feature 102) there is nothing pressing on
        // the body ever, and no errand is started for one.
        let urgent = if self.needs_enabled {
            self.bims[who].needs.urgent()
        } else {
            Vec::new()
        };
        for need in urgent {
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
                Need::Surroundings => false,
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
        if self.needs_enabled
            && (self.bed_is_queued(who) || self.wants_bed(who))
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
                // The harvest's carry is not offered on its own — see
                // `waits_on` — but a thing to the workbench and back is an
                // errand.
                Job::Haul => self.ferry(who),
                Job::Build => self.build(who),
                // A wound to dress, somebody's: the same errand the player
                // orders from the menu on a body, chosen by the room.
                Job::Medical => self
                    .medical_on_offer(who)
                    .is_some_and(|care| self.give_care(who, care)),
            };
            if started {
                return true;
            }
        }
        false
    }

    /// What `who` would do if it took the medical row. **Itself first**: its
    /// own worst part while it bleeds, dressed on the spot. Then a crewmate
    /// dying — the one nearest, its worst-bleeding trauma, while there is
    /// a medkit for it, in the helper's own pack or on a shelf to fetch —
    /// since a trauma left alone bleeds ten a
    /// quarter hour where a wound bleeds ten an hour; never its own, since
    /// a Bim cannot treat its own. Then the crewmate with the most wounds
    /// open and that one's worst part. `None` with nothing to hand for
    /// anything, nobody hurt, or a helper in no state to do it — dead, out
    /// cold, outside, or under orders with an enemy in its sight or a
    /// blade at its throat; **a bot under arms with nothing in sight
    /// doctors** — the fight is over for it, whatever the alarm says — and
    /// takes its weapon up again the moment `aim` sees something. The
    /// player's own Bim recruited is the player's: it never doctors of its
    /// own accord under orders.
    ///
    /// **A crewmate is doctored only where it lies out of the fight**: the
    /// whole room calm (`Game::calm` — no shot or blow here for
    /// [`CALM_AFTER`], no enemy in anybody's sight for as long, and none
    /// within [`CALM_RANGE`] tiles), or the patient itself out of harm
    /// (`Game::out_of_harm` — no enemy up within [`RESCUE_CLEAR`] tiles of
    /// it and nothing that could see it there) — a helper bent over a
    /// patient with the enemy a corridor away was a second body down, and
    /// one waiting for the whole station to go quiet left the body a
    /// field medic had carried clear to bleed out behind the lines. Its
    /// own wound it dresses regardless, on the spot, whenever it has
    /// nothing in its own sight; and a bot puts either down the moment it
    /// has something to shoot at (`Game::care_gives_way`).
    ///
    /// The patient may be out cold — it lies still, which is the easiest
    /// patient there is — but not outside, where the walk cannot follow,
    /// and not somewhere the helper cannot get to: the chain would start,
    /// find no route and be given up, and this would offer it again on the
    /// next step, for ever. A part somebody else is already walking over to
    /// dress or treat is left to them, so two crew do not doctor one part
    /// twice and waste the second's minutes.
    fn medical_on_offer(&self, who: usize) -> Option<Care> {
        if !self.bims.get(who).is_some_and(|b| b.is_alive())
            || self.bims[who].character.is_unconscious()
            || self.bims[who].character.is_outside()
        {
            return None;
        }
        if self.bims[who].character.is_recruited() {
            let bot = self.is_bot(who);
            let quiet = self.bims[who].locked.is_none()
                && self.bims[who].blow.is_none()
                && !self
                    .combat
                    .sees_any(&self.room.sight, self.bims[who].character.pos);
            if !bot || !quiet {
                return None;
            }
        }
        // What the others have in hand, by patient and part — a dressing
        // and a treatment apart, since a part being bandaged (its wounds)
        // can still want its trauma treated, and the other way round.
        let in_hand = |wanted: fn(Kind) -> Option<(usize, u32)>| -> Vec<(usize, u32)> {
            self.bims
                .iter()
                .enumerate()
                .filter(|&(other, _)| other != who)
                .filter_map(|(_, b)| b.task.as_ref().and_then(|t| wanted(t.kind())))
                .collect()
        };
        let being_dressed = in_hand(|k| match k {
            Kind::Bandage { patient, part } => Some((patient, part)),
            _ => None,
        });
        let being_treated = in_hand(|k| match k {
            Kind::Treat { patient, part, .. } => Some((patient, part)),
            _ => None,
        });
        let from = self.bims[who].character.pos;
        // Anybody else's waits until it lies out of the fight: the whole
        // room calm, or the patient itself **out of harm** — no enemy up
        // within `RESCUE_CLEAR` tiles of it and nothing that could see it
        // there, which is where a field medic sets a body down and where
        // a dying run ends. It used to be the room's calm alone, and a
        // fight in waves is never calm from the first machine to the
        // last: a crew member carried clear lay there untreated until
        // the whole station had been cleared.
        let calm = self.calm();
        // And in a fight, **one helper a patient**: a patient somebody
        // else is already on its way to is theirs, every part of it. A
        // part apiece — the calm's rule — took nine bots off the line at
        // the first lull for three patients, and the fight came back to a
        // crew bent over its wounded.
        let seen_to_by_another = |patient: usize| {
            self.bims.iter().enumerate().any(|(other, b)| {
                other != who
                    && other != patient
                    && b.task
                        .as_ref()
                        .is_some_and(|t| t.kind().patient() == Some(patient))
            })
        };
        let can_get_to = |patient: usize| {
            self.bims[patient].is_alive()
                && !self.bims[patient].character.is_outside()
                && (calm
                    || (self.out_of_harm(self.bims[patient].character.pos)
                        && !seen_to_by_another(patient)))
                && task::patient_stand(&self.room, &self.maps, who, patient, from).is_some()
        };
        let worst_part = |patient: usize| -> Option<Part> {
            Part::ALL
                .iter()
                .copied()
                .filter(|p| !being_dressed.contains(&(patient, p.code())))
                .filter(|&p| self.bims[patient].health.wounds(p) > 0)
                .max_by_key(|&p| self.bims[patient].health.wounds(p))
        };
        // Its own wound first, while it is carrying a dressing for it
        // (feature 87: a bandage comes out of the helper's own pack).
        if self.bandages_of(who) > 0
            && let Some(part) = worst_part(who)
        {
            return Some(Care::Bandage(who, part));
        }
        // A kit for it: one in the helper's own pack, else one on a shelf
        // to walk to.
        if self.room.carries_kit(who) || self.room.medkits > 0 {
            let worst_trauma = |patient: usize| -> Option<Part> {
                Part::ALL
                    .iter()
                    .copied()
                    .filter(|p| !being_treated.contains(&(patient, p.code())))
                    .filter_map(|p| self.bims[patient].health.trauma(p).map(|t| (p, t)))
                    .max_by(|a, b| a.1.bleed().total_cmp(&b.1.bleed()))
                    .map(|(p, _)| p)
            };
            let dying = (0..self.bims.len())
                .filter(|&p| p != who && can_get_to(p))
                .filter(|&p| worst_trauma(p).is_some())
                .min_by(|&a, &b| {
                    (self.bims[a].character.pos - from)
                        .len()
                        .total_cmp(&(self.bims[b].character.pos - from).len())
                });
            if let Some(patient) = dying
                && let Some(part) = worst_trauma(patient)
            {
                return Some(Care::Treat(patient, part));
            }
        }
        if self.bandages_of(who) == 0 {
            return None;
        }
        let patient = (0..self.bims.len())
            .filter(|&p| p != who)
            .filter(|&p| can_get_to(p))
            .filter(|&p| self.bims[p].health.bleeding() > 0)
            .max_by_key(|&p| self.bims[p].health.bleeding())?;
        worst_part(patient).map(|part| Care::Bandage(patient, part))
    }

    /// Whether a crewmate is on its way to `who`, or has its hands on it:
    /// a dressing or a treatment with `who` as the patient, somebody
    /// else's.
    fn is_being_seen_to(&self, who: usize) -> bool {
        self.bims.iter().enumerate().any(|(other, b)| {
            other != who
                && b.task
                    .as_ref()
                    .is_some_and(|t| t.kind().patient() == Some(who))
        })
    }

    /// Whether a bot's doctoring is to be put down for the fight: a body
    /// nobody steers, under arms, with a dressing or a treatment in hand —
    /// its own wound or a crewmate's — and something to shoot at from
    /// where it stands, a gun's shot in reach or, for a blade, an enemy in
    /// sight. Never a player's own Bim, whose bandage is the player's
    /// call; never a body on the run, whose own dressing the run looks
    /// after (`tick_combat`); and never a medic whose beam holds its fire
    /// anyway.
    fn care_gives_way(&self, who: usize, skill: &Skill) -> bool {
        let Some(bim) = self.bims.get(who) else {
            return false;
        };
        let doctoring = bim
            .task
            .as_ref()
            .is_some_and(|t| matches!(t.kind(), Kind::Bandage { .. } | Kind::Treat { .. }));
        if !doctoring
            || !self.is_bot(who)
            || !bim.character.is_recruited()
            || skill.holds_fire
            || self.is_fleeing(who)
        {
            return false;
        }
        let Some(weapon) = bim.gear.weapon else {
            return false;
        };
        let stats = skill.stats(weapon);
        let from = bim.character.pos;
        if stats.melee {
            self.combat.sees_any(&self.room.sight, from)
        } else {
            self.combat.aim(&self.room.sight, from, &stats).is_some()
        }
    }

    /// Start what the medical row offered: the dressing or the treatment.
    fn give_care(&mut self, who: usize, care: Care) -> bool {
        match care {
            Care::Bandage(patient, part) => self.bandage(who, patient, part),
            Care::Treat(patient, part) => self.treat(who, patient, part),
        }
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
        // A thing to carry between two benches.
        if self.ferry_on_offer(who).is_some() {
            offered.push(Job::Haul);
        }
        // A site with nobody at it.
        if self.build_on_offer(who).is_some() {
            offered.push(Job::Build);
        }
        // With the needs off (feature 102) the galley, the bays and the
        // broom serve nobody: a pot, a tray or a sweep is work for a body
        // that eats and minds the mess, and there is none.
        if !self.needs_enabled {
            offered.retain(|job| !matches!(job, Job::Cook | Job::Plant | Job::Cut | Job::Clean));
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
    ///
    /// **A bot never stands at a bench** (feature 89): an open order — the
    /// smelter, the workbench, the armoury, the drug lab — is the
    /// players' own work, and a Bim no player steers passes it over
    /// whatever the Craft row says. It plants, sweeps, cooks, hauls and
    /// shoots as it always did. An order *named* for one Bim is still
    /// that Bim's, bot or not, since it is a thing already begun rather
    /// than the ship's standing want: an engineer's armour repair at the
    /// workbench (feature 74) would otherwise sit on the bench for ever.
    fn craft_on_offer(&self, who: usize) -> Option<Order> {
        self.orders.iter().copied().find(|order| {
            let mine = match order.only {
                Some(only) => only == who,
                None => !self.is_bot(who),
            };
            mine && self.room.benches.get(order.bench).is_some()
                && self.can_begin(
                    who,
                    Kind::Craft {
                        recipe: order.recipe,
                        bench: order.bench,
                    },
                )
        })
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

    /// The first site with everything there that `who` could put together
    /// now, the same way.
    fn build_on_offer(&self, who: usize) -> Option<(u32, bool, f32)> {
        self.room
            .builds
            .iter()
            .filter(|b| b.minutes > 0.0)
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

    /// The first thing the world wants carried between two benches that
    /// `who` could carry now: both benches in the room, a way to the first
    /// from where the Bim stands, and nobody else carrying to or working at
    /// the second. Asked of every idle Bim every step, so the walk is not
    /// planned until there is an order to plan it for.
    fn ferry_on_offer(&self, who: usize) -> Option<Ferry> {
        if self.room.ferries.is_empty() {
            return None;
        }
        let from = self.bims[who].character.pos;
        self.room.ferries.iter().copied().find(|f| {
            let (Some(a), Some(_)) = (self.room.benches.get(f.from), self.room.benches.get(f.to))
            else {
                return false;
            };
            self.maps
                .pick(self.room.bath.is_open())
                .can_reach(from, a.at)
                && self.can_begin(
                    who,
                    Kind::Ferry {
                        from: f.from,
                        to: f.to,
                    },
                )
        })
    }

    /// Off to carry the first thing the world wants carried.
    pub fn ferry(&mut self, who: usize) -> bool {
        let Some(order) = self.ferry_on_offer(who) else {
            return false;
        };
        let kind = Kind::Ferry {
            from: order.from,
            to: order.to,
        };
        if !self.take_over(who, kind, 0.0) {
            return false;
        }
        let taken = self.taken_for(who);
        self.bims[who].task = Some(Task::ferry(
            who,
            order.from,
            order.to,
            &mut self.bims[who].character,
            &mut self.room,
            &self.maps,
            &taken,
        ));
        true
    }

    /// What the world wants carried between benches, this step. Replaces
    /// the last list whole, like the build orders; the world keeps an
    /// order on it until the thing is put down, so a chain on its way is
    /// not left walking to nothing.
    pub fn set_ferries(&mut self, ferries: Vec<Ferry>) {
        self.room.ferries = ferries;
    }

    /// What the world asked carried this step, as the room still has it: a
    /// carry drops off the list as its thing is put down.
    pub fn ferries(&self) -> &[Ferry] {
        &self.room.ferries
    }

    /// The carries whose thing was taken off the first bench since the
    /// last call, for the world to take it out of the hold or off the
    /// workbench.
    pub fn take_ferry_picked(&mut self) -> Vec<Ferry> {
        core::mem::take(&mut self.room.ferry_picked)
    }

    /// The carries whose thing was put down on the second bench since the
    /// last call.
    pub fn take_ferry_dropped(&mut self) -> Vec<Ferry> {
        core::mem::take(&mut self.room.ferry_dropped)
    }

    /// The carries given up between the two since the last call: what was
    /// carried goes back where it came from, or into the hold.
    pub fn take_ferry_returned(&mut self) -> Vec<Ferry> {
        core::mem::take(&mut self.room.ferry_returned)
    }

    /// The sites put together since the last call, each with who put it
    /// together, for the world to put the parts down.
    pub fn take_built(&mut self) -> Vec<(u32, usize)> {
        core::mem::take(&mut self.room.built)
    }

    /// Whether anybody is on a building errand **for `site`**: walking
    /// to it or standing at it. What the world asks before it counts a
    /// site's price as spoken for (feature 95) — a site nobody has
    /// walked to costs nothing to lay out and nothing to give up.
    pub fn building_at(&self, site: u32) -> bool {
        self.bims.iter().any(|b| {
            b.task.as_ref().is_some_and(|t| {
                !t.is_done() && matches!(t.kind(), Kind::Build { site: s, .. } if s == site)
            })
        })
    }

    /// Whether anybody is on a building errand. What holds the ship at
    /// rest while it is being built on.
    pub fn building_under_way(&self) -> bool {
        self.bims.iter().any(|b| {
            b.task
                .as_ref()
                .is_some_and(|t| !t.is_done() && matches!(t.kind(), Kind::Build { .. }))
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
        match self.foreign {
            Some((rect, stance, true)) => self.room.sight.set_foreign_outside(rect, stance),
            Some((rect, stance, false)) => self.room.sight.set_foreign(Some(rect), stance),
            None => {}
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

    /// Everybody in: whoever is outside is brought back through the door
    /// and the errand given up for good. For a ship that is leaving.
    pub fn recall_outside(&mut self) {
        for who in 0..self.bims.len() {
            if self.bims[who].character.is_outside()
                && let Some(task) = self.bims[who].task.take()
            {
                task.abandon(&mut self.bims[who].character, &mut self.room);
            }
        }
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
        // The clock runs from the company bar running dry, not from the
        // last word: a bar with anything on it is a Bim that has been
        // talked to lately enough.
        let starved = self.bims[who].needs.level(Need::Company) <= 0.0;
        let fallout = {
            let (bims, rng) = (&mut self.bims, &mut self.rng);
            bims[who]
                .solitude
                .update(minutes, starved, can_sit_down, rng)
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
    /// something to be swept, that is not a hypothetical. A walk the player
    /// gave is the other no: see `on_a_given_walk`, and the Shift-clicks
    /// still waiting their turn count with it.
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
                // And neither does the player's own walk. A Bim on its way
                // somewhere it was sent is left to get there, and so is one
                // with Shift-clicks still waiting their turn: a chat walks
                // it to a meeting spot, which loses the route it is on
                // (`on_a_given_walk`) and puts every leg of the chain
                // behind it somewhere else. Work is interrupted and comes
                // back; a walk does not.
                && !self.on_a_given_walk(other)
                && self.ordered_count(other) == 0
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

    /// Whether the bays are running: every bay aboard at once, since the
    /// world asks power of a *kind* of part and not of one. Off, each
    /// hibernates — see `hydro::Bay::set_powered`. On unless the world
    /// says otherwise.
    pub fn set_hydro_powered(&mut self, on: bool) {
        for bay in &mut self.room.bays {
            bay.set_powered(on);
        }
    }

    pub fn hydro_powered(&self, bay: usize) -> bool {
        self.bay(bay).powered()
    }

    /// Whether bay `bay` is a field — open ground at half a bay's pace,
    /// with nothing to plug in. See `hydro::Bay::field`.
    pub fn hydro_is_field(&self, bay: usize) -> bool {
        self.bay(bay).is_field()
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

    /// Days the company bar has been empty since it last spoke to anybody.
    /// What the stages are built on, so the readout and the behaviour
    /// cannot drift apart.
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
    ///
    /// Recruit player `slot`'s own crew member, or let it go again.
    pub fn toggle_recruited(&mut self, slot: u32) {
        let who = slot as usize;
        if who >= self.bims.len() {
            return;
        }
        let now = !self.bims[who].character.is_recruited();
        self.bims[who].character.set_recruited(now);
        if now {
            // It is the thing being ordered about, so it is the thing selected.
            self.bims[who].character.select_for(slot, true);
        }
    }

    /// Whether player `slot`'s own crew member is recruited.
    pub fn is_recruited(&self, slot: u32) -> bool {
        self.bims
            .get(slot as usize)
            .is_some_and(|b| b.character.is_recruited())
    }

    /// How many of the crew are players' own — see `crate::order`. The
    /// first `players` Bims, slot *i* steering Bim *i*.
    pub fn set_players(&mut self, players: u32) {
        self.players = players.max(1) as usize;
    }

    pub fn players(&self) -> u32 {
        self.players as u32
    }

    /// Whether Bim `who` is a player's own: steered by the mouse rather
    /// than by its own lights, and never a bot.
    pub fn is_player(&self, who: usize) -> bool {
        who < self.players
    }

    /// The other side of [`Game::is_player`]: a Bim that runs on its own
    /// lights. Every body of a room whose bodies are hostile is one —
    /// a station's people answer to nobody at this keyboard, so the
    /// first of them is no more a player's than the last. What decides
    /// whether a crewmate is doctored of its own accord
    /// (`medical_on_offer`) and, since feature 89, whether it will stand
    /// at a bench at all (`craft_on_offer`).
    pub fn is_bot(&self, who: usize) -> bool {
        self.hostile_bodies || !self.is_player(who)
    }

    /// Whose eyes the picture is drawn for — `render` rings that player's
    /// selection and nobody else's. A picture setting, this window's own.
    pub fn set_viewer(&mut self, slot: u32) {
        self.viewer = slot;
    }

    pub fn viewer(&self) -> u32 {
        self.viewer
    }

    pub fn is_autonomous(&self) -> bool {
        self.autonomous
    }

    // --- player input ---------------------------------------------------

    /// Select by control group, for player `slot`. Group 1 is the player's
    /// own crew member.
    pub fn select_group(&mut self, slot: u32, group: u32) {
        if group == 1 && (slot as usize) < self.bims.len() {
            self.select_only(slot, Some(slot as usize));
        }
    }

    pub fn clear_selection(&mut self, slot: u32) {
        self.select_only(slot, None);
    }

    /// One of the crew selected by player `slot`, or none.
    ///
    /// Selecting is *looking at*, not taking charge of: any of them can be
    /// picked, and picking one is what puts its panels on screen. Whether it
    /// takes orders is a separate question — see `order_move`, which asks
    /// after the player's own always and a crewmate only under the alarm.
    ///
    /// A click picks one; a marquee picks everybody it touches
    /// (`select_many`), and the panels show the first of them. Every
    /// player has a selection of their own: `Character::selected` is a
    /// mask, and one player's click leaves the others' alone.
    fn select_only(&mut self, slot: u32, who: Option<usize>) {
        for (i, bim) in self.bims.iter_mut().enumerate() {
            bim.character.select_for(slot, who == Some(i));
        }
    }

    /// Several at once: what a marquee does. Nobody else stays selected.
    fn select_many(&mut self, slot: u32, who: &[usize]) {
        for (i, bim) in self.bims.iter_mut().enumerate() {
            bim.character.select_for(slot, who.contains(&i));
        }
    }

    /// Which of them player `slot` has selected, or `None` — the first,
    /// when several are: the player's own if it is among them, since it
    /// comes first.
    pub fn selected(&self, slot: u32) -> Option<usize> {
        self.bims
            .iter()
            .position(|b| b.character.is_selected_by(slot))
    }

    /// Everybody player `slot` has selected, in crew order.
    pub fn selected_all(&self, slot: u32) -> Vec<usize> {
        (0..self.bims.len())
            .filter(|&i| self.bims[i].character.is_selected_by(slot))
            .collect()
    }

    pub fn is_selected(&self, who: usize, slot: u32) -> bool {
        self.bims[who].character.is_selected_by(slot)
    }

    pub fn bim_pos(&self, who: usize) -> Vec2 {
        self.bims[who].character.pos
    }

    /// How many are aboard. Two in the classic room; a ship's crew, up to
    /// [`room::BERTHS`], aboard one. **The Bims alone** — the machines on
    /// the deck are [`Game::droid_count`], and the two together are
    /// [`Game::body_count`].
    pub fn crew_count(&self) -> u32 {
        self.bims.len() as u32
    }

    // --- the bodies on this deck (feature 83) ---------------------------------
    //
    // A droid-held station's room holds machines rather than people, and
    // the world hands both rooms one list of **bodies**: this room's Bims
    // first, then its droids. Every `who` the world sends in and reads
    // back — a target, a hit, a position, a peek — is an index into that
    // one space, and the handful of methods below are where it is taken
    // apart. Inside the room a `who` is still a Bim's, which is why
    // nothing else had to change.

    /// How many machines are on this deck. Nought everywhere but a
    /// droid-held station's room.
    pub fn droid_count(&self) -> u32 {
        self.droids.len() as u32
    }

    /// How many bodies there are on this deck all told: the Bims, then
    /// the machines. What the world sizes its target lists by.
    pub fn body_count(&self) -> u32 {
        (self.bims.len() + self.droids.len()) as u32
    }

    /// Which machine a body index names, or `None` for one of the Bims
    /// (or for an index past the end of both).
    fn droid_at(&self, who: usize) -> Option<usize> {
        let i = who.checked_sub(self.bims.len())?;
        (i < self.droids.len()).then_some(i)
    }

    /// The machines as they stand, for the world to read and the painter
    /// to draw.
    pub fn droids(&self) -> &[Droid] {
        &self.droids
    }

    /// One of them by its **droid** index — not a body index.
    pub fn droid(&self, i: usize) -> Option<&Droid> {
        self.droids.get(i)
    }

    /// Put a machine on the deck, at the end of the body list. Answers
    /// the **body** index it took, which is what the world keeps.
    pub fn add_droid(&mut self, droid: Droid) -> usize {
        self.droids.push(droid);
        self.bims.len() + self.droids.len() - 1
    }

    /// Every machine off the deck: what a room built afresh starts from.
    pub fn clear_droids(&mut self) {
        self.droids.clear();
    }

    /// The machines out of this room, for a fresh one to take them —
    /// what [`Game::take_crew`] is for the Bims. A dock, an undock and a
    /// relayout all throw the room away and build another; the machines
    /// go across with it, wrecks and all.
    pub fn take_droids(&mut self) -> Vec<Droid> {
        core::mem::take(&mut self.droids)
    }

    /// Machines into a fresh room, each shifted by `shift` — the way
    /// `Room::adopt` carries the Bims across a join. Every route is
    /// dropped: a route is the old room's grid, and the new one's is not
    /// the same. Where they stand is the truth, and the next plan is
    /// made from there.
    ///
    /// **A spot no body fits in is snapped to the nearest that one
    /// does**, exactly as `Game::adopt` snaps a Bim carried between
    /// rooms. A reinforcement wave is posted in rings round the airlock
    /// its ship tied up at (`world::droid::arriving_wave`) and a ring
    /// falls where it falls: at the arena four machines of sixteen
    /// landed in a bulkhead or out in the void, where they could neither
    /// walk nor see — they stood there for good, never fired a shot, and
    /// held the *next* wave up with them, since nothing arrives while
    /// one is still standing.
    pub fn adopt_droids(&mut self, droids: Vec<Droid>, shift: Vec2) {
        for mut droid in droids {
            droid.pos = droid.pos + shift;
            let nav = self.maps.pick(true);
            if !nav.is_free(droid.pos) {
                droid.pos = nav.nearest_free(droid.pos);
            }
            droid.halt();
            droid.peek = None;
            droid.blow = None;
            droid.locked = None;
            droid.smashing = None;
            // **The stagger is kept.** `World::build_wave` spreads each
            // machine's first plan over `PLAN_EVERY` so a wave of
            // sixteen does not score sixteen lattices of stands on one
            // step, and zeroing the wait here threw that away for every
            // wave laid. A machine carried across a join waits at most a
            // plan's period before it picks a stand again, which is what
            // it did anyway.
            self.droids.push(droid);
        }
    }

    /// Where one of this room's bodies stands, Bim or machine.
    pub fn body_pos(&self, who: usize) -> Vec2 {
        match self.droid_at(who) {
            Some(i) => self.droids[i].pos,
            None => self.bims.get(who).map_or(Vec2::ZERO, |b| b.character.pos),
        }
    }

    /// Take `damage` off a machine's part — the part rolled by the
    /// caller off the combat stream's own draw (`Hit::roll` through
    /// [`DroidPart::hit_by`]), since a droid's four parts are not a
    /// body's three. By **droid** index. Whether anything was struck.
    pub fn strike_droid(&mut self, i: usize, part: DroidPart, damage: f32) -> bool {
        let Some(droid) = self.droids.get_mut(i) else {
            return false;
        };
        let was = droid.destroyed;
        let struck = droid.strike(part, damage);
        let (at, size, gone) = (droid.pos, droid.kind.half_width(), !was && droid.destroyed);
        // The flash on the part it struck, and a machine bursting apart
        // where it has just gone (feature 98) — drawing only.
        if let Some(struck) = struck {
            self.combat.fx.struck(self.bims.len() + i, struck.code());
        }
        // A machine that has just gone throws its wreck onto the deck's
        // noise the way a bolt landing does: the fight is not quiet with
        // one falling over.
        if gone {
            self.combat.lull_break();
            self.combat.fx.burst(at, size);
        }
        true
    }

    pub fn selected_count(&self, slot: u32) -> u32 {
        self.selected_all(slot).len() as u32
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
    pub fn drag_end(&mut self, slot: u32, x: f32, y: f32) -> u32 {
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

        // Whoever the box touched — all of them, for a sweep; a click on a
        // spot two share picks the player's own, which comes first, since a
        // click most likely meant the one that can be told to do something.
        let touched: Vec<usize> = (0..self.bims.len())
            .filter(|&i| self.bims[i].is_alive())
            .filter(|&i| {
                box_.touches_circle(
                    self.bims[i].character.pos,
                    self.bims[i].character.pick_radius(),
                )
            })
            .collect();
        if box_.width() < 4.0 && box_.height() < 4.0 {
            self.select_only(slot, touched.first().copied());
        } else {
            self.select_many(slot, &touched);
        }
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
    ///
    /// With several selected, each gets a spot of its own round the point
    /// ([`CLUSTER_SLOTS`], in crew order) rather than all of them the one
    /// tile; a right-*drag* is a line instead — [`Game::order_line`].
    pub fn order_move(&mut self, slot: u32, x: f32, y: f32) -> u32 {
        let Some((squad, spots)) = self.huddle(slot, vec2(x, y)) else {
            return ORDER_IGNORED;
        };
        self.order_squad(slot, &squad, &spots)
    }

    /// Who a right-click at `at` sends, and where each goes: one alone to
    /// the point, several to a spot apiece round it ([`CLUSTER_SLOTS`],
    /// in crew order). `None` when nobody selected takes orders.
    pub(crate) fn huddle(&self, slot: u32, at: Vec2) -> Option<(Vec<usize>, Vec<Vec2>)> {
        let squad = self.orderable(slot);
        if squad.is_empty() {
            return None;
        }
        let spots: Vec<Vec2> = if squad.len() == 1 {
            vec![at]
        } else {
            (0..squad.len())
                .map(|i| {
                    let (dx, dy) = CLUSTER_SLOTS[i % CLUSTER_SLOTS.len()];
                    at + vec2(dx * TILE, dy * TILE)
                })
                .collect()
        };
        Some((squad, spots))
    }

    /// Who a right-drag from `from` to `to` sends, and where each stands
    /// along the line — see [`Game::order_line`]. `None` when nobody
    /// selected takes orders.
    pub(crate) fn formation(
        &self,
        slot: u32,
        from: Vec2,
        to: Vec2,
    ) -> Option<(Vec<usize>, Vec<Vec2>)> {
        let mut squad = self.orderable(slot);
        if squad.is_empty() {
            return None;
        }
        let n = squad.len();
        if n == 1 {
            return Some((squad, vec![from]));
        }
        let along = (to - from).normalize_or_zero();
        squad.sort_by(|&a, &b| {
            let pa = (self.bims[a].character.pos - from).dot(along);
            let pb = (self.bims[b].character.pos - from).dot(along);
            pa.total_cmp(&pb)
        });
        let spots: Vec<Vec2> = (0..n)
            .map(|i| from + (to - from) * (i as f32 / (n - 1) as f32))
            .collect();
        Some((squad, spots))
    }

    /// One queued walk each, spot for spot; the best code of them, as
    /// [`Game::order_squad`] answers.
    pub(crate) fn queue_squad(&mut self, squad: &[usize], spots: &[Vec2]) -> u32 {
        let mut best = ORDER_IGNORED;
        for (&who, &spot) in squad.iter().zip(spots) {
            // A crewmate holds the spot it was sent to, as it does when
            // sent there now; the player's own goes on to the next thing.
            let post = !self.is_player(who);
            let code = self.queue_walk(who, spot, post);
            if code == ORDER_MOVING || best == ORDER_IGNORED {
                best = code;
            }
        }
        best
    }

    /// Put an order on the back of its Bim's queue, to be begun when its
    /// turn comes — see [`Game::order_later`]. Nothing for a body down.
    pub(crate) fn queue_order(&mut self, saved: Saved) {
        let who = saved.who();
        if who < self.bims.len() && self.is_alive(who) {
            self.bims[who].queue.push(saved);
        }
    }

    /// Put a walk to `to` on the back of `who`'s queue, to be given when
    /// its turn comes — see [`Game::order_later`]. Refused now, with a
    /// cross, when there is no way there even with the door open: the
    /// deck is one piece or it is not, wherever the Bim will be standing
    /// by then. On the plain the walk is planned on the body's window
    /// when it is begun, so nothing is asked now. A ping where the Bim
    /// will stand, as a live order pings.
    pub(crate) fn queue_walk(&mut self, who: usize, to: Vec2, post: bool) -> u32 {
        if !self.is_alive(who) || self.bims[who].character.is_outside() {
            return ORDER_IGNORED;
        }
        let stand = self.nearest_stand(who, to);
        if !self.on_a_window(who, to) && !self.can_reach_through_door(who, stand) {
            self.mark(stand, true);
            return ORDER_NOWHERE;
        }
        self.bims[who]
            .queue
            .push(Saved::ordered(who, Kind::Walk { post }, 0.0, Some(stand)));
        self.mark(stand, false);
        ORDER_MOVING
    }

    /// Whether a walk by `who` to `at` is on the plain's windows rather
    /// than the deck's grid: the room is on a planet, and the body is
    /// afield or the point is beyond the deck's box.
    fn on_a_window(&self, who: usize, at: Vec2) -> bool {
        self.room.plane.is_some()
            && (self.bims[who].character.is_afield() || !self.room.interior.contains(at))
    }

    /// A right-drag from `from` to `to` with a selection: the crew that
    /// take orders are spread evenly along that line, ends included, each
    /// to the point nearest its own place along it so nobody crosses
    /// anybody. One alone goes to where the drag began. The codes are
    /// `order_move`'s: `ORDER_MOVING` if anybody set off, else the first
    /// refusal.
    pub fn order_line(&mut self, slot: u32, from: Vec2, to: Vec2) -> u32 {
        let Some((squad, spots)) = self.formation(slot, from, to) else {
            return ORDER_IGNORED;
        };
        self.order_squad(slot, &squad, &spots)
    }

    /// Whoever player `slot` has selected and takes orders from them:
    /// their own crew member always; a crewmate only while the crew are
    /// **under arms** — the alarm, or a player leading them (feature 84,
    /// `Game::led`) — since a crewmate at the hob is on an errand and
    /// not a soldier. Another player's own is never theirs to order.
    fn orderable(&self, slot: u32) -> Vec<usize> {
        self.selected_all(slot)
            .into_iter()
            .filter(|&who| who == slot as usize || (self.mustered && !self.is_player(who)))
            .collect()
    }

    /// One order each, spot for spot; the best code of them.
    fn order_squad(&mut self, slot: u32, squad: &[usize], spots: &[Vec2]) -> u32 {
        let mut best = ORDER_IGNORED;
        for (&who, &spot) in squad.iter().zip(spots) {
            let code = self.order_one(slot, who, spot);
            if code == ORDER_MOVING || best == ORDER_IGNORED {
                best = code;
            }
        }
        best
    }

    /// [`Game::order_move`] for one of the crew that takes orders.
    fn order_one(&mut self, slot: u32, who: usize, at: Vec2) -> u32 {
        let code = self.order_move_for(slot, who, at.x, at.y);
        // A crewmate ordered somewhere holds that spot rather than falling
        // back into the gathering round the player, until the alarm is over.
        if !self.is_player(who)
            && code != ORDER_IGNORED
            && code != ORDER_NOWHERE
            && code != ORDER_LOCKED
        {
            let spot = self.nearest_stand(who, at);
            self.bims[who].character.set_post(Some(spot));
        }
        code
    }

    /// A right-drag on the deck begins: the line is drawn from here.
    pub fn order_drag_begin(&mut self, x: f32, y: f32) {
        let p = vec2(x, y);
        self.order_drag = Some((p, p));
    }

    pub fn order_drag_update(&mut self, x: f32, y: f32) {
        if let Some((_, end)) = &mut self.order_drag {
            *end = vec2(x, y);
        }
    }

    /// The right-drag ends: a line longer than a click is
    /// [`Game::order_line`], anything shorter [`Game::order_move`] at the
    /// point. The screen is what decides a drag is a drag, since a click
    /// is judged in points on the glass, not in room units.
    pub fn order_drag_end(&mut self, slot: u32, x: f32, y: f32, dragged: bool) -> u32 {
        self.order_drag_update(x, y);
        let Some((from, to)) = self.order_drag.take() else {
            return ORDER_IGNORED;
        };
        if dragged {
            self.order_line(slot, from, to)
        } else {
            self.order_move(slot, to.x, to.y)
        }
    }

    pub fn order_drag_cancel(&mut self) {
        self.order_drag = None;
    }

    /// [`Game::order_move`] for one Bim, the checks on who is done. A
    /// plain order, so what was queued with Shift goes (`drop_ordered`).
    fn order_move_for(&mut self, slot: u32, who: usize, x: f32, y: f32) -> u32 {
        if !self.bims[who].character.is_selected_by(slot) {
            return ORDER_IGNORED;
        }
        self.drop_ordered(who);
        self.walk_order(who, x, y)
    }

    /// The walk a right-click gives, to whoever it was decided it goes to:
    /// what [`Game::order_move_for`] does once it has asked whose the Bim
    /// is, and what a walk waiting its turn on the queue is given with
    /// when its turn comes ([`Game::begin_ordered`]).
    fn walk_order(&mut self, who: usize, x: f32, y: f32) -> u32 {
        if !self.is_alive(who)
            // Out there it is on a walk, and the walk brings it in.
            || self.bims[who].character.is_outside()
        {
            return ORDER_IGNORED;
        }
        let want = vec2(x, y);
        // A fresh order is the end of standing anywhere in particular.
        self.bims[who].character.set_post(None);

        // On the plain, or bound for it: the body's window, and no door
        // to work — the ground has none.
        if self.on_a_window(who, want) {
            if !self.plan_route(who, want) {
                self.mark(want, true);
                return ORDER_NOWHERE;
            }
            self.interrupt_for_order(who);
            // Putting the errand down may have stood the body up somewhere
            // else: the walk is planned again from there.
            self.plan_route(who, want);
            let stand = self.nearest_stand(who, want);
            self.mark(stand, false);
            return ORDER_MOVING;
        }

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
        if self.room.plane.is_some()
            && (self.bims[who].character.is_afield() || !self.room.interior.contains(want))
        {
            let stand = self.nearest_stand(who, want);
            let ok = self.plan_route(who, want);
            self.mark(stand, !ok);
            return;
        }
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
            if bim.character.picked_at(p) {
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
        // Then a weapon lying on the deck, to pick up.
        for d in &self.room.weapons_down {
            if (d.at - p).len() <= PICK_RADIUS {
                self.hit_dropped = d.id;
                return HIT_DROPPED;
            }
        }
        let hit = self.room.hit(vec2(x, y));
        // With the needs off (feature 102) the galley, the bunks, the heads,
        // the shower, the bay and the broom are furniture: nothing to ask
        // of them, so no menu opens on them.
        if !self.needs_enabled
            && matches!(
                hit,
                room::HIT_FRIDGE
                    | room::HIT_STOVE
                    | room::HIT_BED
                    | room::HIT_TOILET
                    | room::HIT_DISHWASHER
                    | room::HIT_HYDRO
                    | room::HIT_LOCKER
                    | room::HIT_SHOWER
            )
        {
            return room::HIT_NONE;
        }
        hit
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

    /// The bunk the last click landed on, after `HIT_BED`.
    pub fn hit_bed(&self) -> usize {
        self.hit_bed
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

    /// The research desk the last click landed on, after `HIT_RESEARCH`.
    pub fn hit_research(&self) -> usize {
        self.hit_research
    }

    /// Where the Bim stands at that research desk, for `send_to`; `None`
    /// for no such desk.
    pub fn research_spot(&self, desk: usize) -> Option<Vec2> {
        self.room.research.get(desk).map(|d| d.1)
    }

    /// Every research desk on the deck: its footprint and its stand spot,
    /// the ship's own first and a docked station's after. What the
    /// world asks to tell one from the other.
    pub fn research_desks(&self) -> &[(Rect, Vec2)] {
        &self.room.research
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
            (&mut self.hit_research, room.research_at(p)),
            (&mut self.hit_bed, room.bed_at(p)),
        ] {
            if let Some(i) = at {
                *hit = i;
            }
        }
    }

    // --- the doors, for the world to carry between two rooms ---------------------

    /// Every powered door's lock, in this room's units, for the world to
    /// carry to the same door in another room — the joined deck and the
    /// station's own room have the station's doors twice, and a lock the
    /// crew set on one has to stop the people walking in the other, as a
    /// smash in theirs has to open the door the crew are looking at.
    pub fn door_states(&self) -> Vec<DoorState> {
        self.room
            .doors
            .iter()
            .map(|d| DoorState {
                centre: d.rect.center(),
                locked: d.locked,
                by_crew: d.locked_by == door::Locker::Crew,
                smash: d.smash_progress(),
                changed: d.changed,
            })
            .collect()
    }

    /// The door whose opening `p` is in, if any.
    pub fn door_index_at(&self, p: Vec2) -> Option<usize> {
        self.room.door_at(p)
    }

    /// Put another room's lock on door `i`: locked or not, and whose. Not
    /// a change of this room's own making, so it is not carried back.
    pub fn mirror_door_lock(&mut self, i: usize, locked: bool, by_crew: bool) {
        let Some(door) = self.room.doors.get_mut(i) else {
            return;
        };
        if locked {
            door.lock(if by_crew {
                door::Locker::Crew
            } else {
                // Somebody's in the other room; nobody's here.
                door::Locker::Body(usize::MAX)
            });
        } else {
            door.unlock();
        }
        door.changed = false;
    }

    /// Show another room's smashing on door `i`: the bar, and nothing else
    /// — nobody here is heaving at it. `None` takes it off.
    pub fn mirror_door_smash(&mut self, i: usize, progress: Option<f32>) {
        let Some(door) = self.room.doors.get_mut(i) else {
            return;
        };
        door.smash = progress.map(|p| door::Smash {
            by: usize::MAX,
            done: p * door.smash_time(),
            since_heave: 0.0,
        });
    }

    /// The world has read door `i`'s change.
    pub fn door_change_seen(&mut self, i: usize) {
        if let Some(door) = self.room.doors.get_mut(i) {
            door.changed = false;
        }
    }

    /// Whose lock is on door `i`, while it is locked.
    pub fn door_locked_by(&self, i: usize) -> Option<door::Locker> {
        self.room
            .doors
            .get(i)
            .filter(|d| d.locked)
            .map(|d| d.locked_by)
    }

    /// Work door `i`'s panel without the walk: what the hand does when it
    /// gets there. For the probes, and the world's tests.
    pub fn order_door_now(&mut self, i: usize, order: door::Order) {
        if let Some(door) = self.room.doors.get_mut(i) {
            door.order(order);
        }
    }

    /// Lock door `i` as `by` would, or unlock it, without the walk. For
    /// the probes.
    pub fn set_door_locked_for_probe(&mut self, i: usize, by: Option<door::Locker>) {
        if let Some(door) = self.room.doors.get_mut(i) {
            match by {
                Some(by) => door.lock(by),
                None => door.unlock(),
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

    pub fn doors_drawn(&self) -> bool {
        self.room.doors_drawn
    }

    // --- sight ------------------------------------------------------------------

    /// Whose eyes this room is drawn through. See `crate::sight::Fog`.
    pub fn set_fog(&mut self, fog: Fog) {
        self.fog = fog;
    }

    pub fn fog(&self) -> Fog {
        self.fog
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
        if self.show_everybody {
            return true;
        }
        match self.fog {
            Fog::Crew => true,
            Fog::All => false,
            Fog::None => self.seen_for.get(who).is_some_and(|&left| left > 0.0),
        }
    }

    /// Draw every body of this room whatever the world says is in view:
    /// what a rack of machines laid out for a screenshot wants
    /// (`world::World::stage_droids_for_probe`), since a station's room
    /// is `Fog::None` and its bodies are drawn only where the crew can
    /// see them. A picture setting and nothing else; nothing in the
    /// game sets it.
    pub fn show_everybody_for_probe(&mut self, show: bool) {
        self.show_everybody = show;
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
        self.foreign = rect.map(|r| (r, stance, false));
        self.room.sight.set_foreign(rect, stance);
    }

    /// The other way about: every tile **outside** `rect` is somebody
    /// else's — on a planet, everything but the ship's own box, the
    /// ground round it included.
    /// The planet's plain the deck stands on, if it is on one. See
    /// `crate::terrain`.
    pub fn plane(&self) -> Option<&crate::terrain::Plane> {
        self.room.plane.as_ref()
    }

    /// The room's box: the deck's, or on a plain the deck's and the
    /// margin of ground round it the grids cover.
    pub fn interior(&self) -> Rect {
        self.room.interior
    }

    pub fn set_foreign_outside(&mut self, rect: Rect, stance: Stance) {
        self.foreign = Some((rect, stance, true));
        self.room.sight.set_foreign_outside(rect, stance);
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
    ///
    /// **A hostile room's people know only what they have seen.** The
    /// crew's room takes the list as it comes; a room whose bodies are
    /// hostile keeps `last_seen`: a target one of its living, waking
    /// people on the deck can see right now (`Sight::sees_from`, any
    /// range) is known where it is, one nobody sees is believed where it
    /// was last seen — the tactics walk there, through the passage and
    /// onto the ship if that is where it went, while `aim` still wants
    /// real sight so nobody shoots at a belief — and one unseen for
    /// [`FORGET_AFTER`] is nobody to them, so with the last crew member
    /// out of sight for a minute the hunt is over and the room leaves war.
    /// **The airlock is watched** (`set_watched`): a target within
    /// [`AIRLOCK_WATCH`] tiles of the spot is seen whether or not anybody
    /// is looking, so a crew that boards is noticed at the door, and
    /// hunted from there.
    pub fn set_hostiles(&mut self, at: Vec<Option<(Vec2, Weapon)>>) {
        if !self.hostile_bodies {
            self.last_seen.clear();
            self.combat.set_targets(at);
            return;
        }
        // Every one of this room's own bodies that can look: the Bims up
        // and awake, and the machines that are not wrecks (feature 83) —
        // a droid-held station has nothing but the machines, so leaving
        // them out would be a room that never sees anybody.
        let eyes: Vec<Vec2> = self
            .bims
            .iter()
            .filter(|b| b.is_alive() && !b.character.is_unconscious() && !b.character.is_outside())
            .map(|b| b.character.pos)
            .chain(self.droids.iter().filter(|d| !d.destroyed).map(|d| d.pos))
            .collect();
        let watched = self.watched;
        let (believed, stale) = believe(&mut self.last_seen, &self.room.sight, watched, &eyes, at);
        self.combat.set_targets(believed);
        self.combat.set_stale(&stale);
    }

    /// **The machines' own targets** (feature 94), for the one fight
    /// that is not one room against another: a town the crew are
    /// defending, where the droids stand in the residents' room with the
    /// people they came for. The front `cross` of the list are the
    /// world's across the seam — the crew, shot at with a recorded
    /// [`crate::combat::Shot`] as a hostile room's people shoot — and
    /// the rest are **this room's own bodies** by index, shot at with a
    /// hostile bolt that flies here and lands on the body.
    ///
    /// The machines keep a belief of their own, the way a hostile room's
    /// people keep one about the crew: eyes are the machines' alone,
    /// since the town's people are not on their side and do not spot for
    /// them, and nothing here is watched.
    pub fn set_machine_hostiles(&mut self, at: Vec<Option<(Vec2, Weapon)>>, cross: usize) {
        let eyes: Vec<Vec2> = self
            .droids
            .iter()
            .filter(|d| !d.destroyed)
            .map(|d| d.pos)
            .collect();
        let (believed, stale) = believe(&mut self.machine_seen, &self.room.sight, None, &eyes, at);
        self.combat.set_machine_targets(believed, cross);
        self.combat.set_machine_stale(&stale);
    }

    /// The machines read the room's own target list again: what every
    /// room but a town under attack does.
    pub fn clear_machine_hostiles(&mut self) {
        self.machine_seen.clear();
        self.combat.set_machine_targets(Vec::new(), 0);
    }

    /// Which of this room's own bodies **shelter** rather than fight
    /// (feature 94): a townsperson who is neither the guard nor a
    /// mercenary while the machines are on the town. Said every step by
    /// the world; an empty list is everybody fighting, which is every
    /// other room.
    ///
    /// One newly told to shelter is walked **into the nearest house** —
    /// the bunk nearest it, which in a town is inside one — and *posted*
    /// there, so it goes on living (it eats, it sleeps, it comes back to
    /// the post afterwards) and never takes arms: the alarm's
    /// `muster_crew` leaves it alone, so it holds no weapon and is
    /// nobody's shooter. One told to stop takes its post off and is
    /// mustered again like anybody else the next step.
    pub fn set_sheltering(&mut self, sheltering: &[bool]) {
        if sheltering.is_empty() {
            if self.sheltering.is_empty() {
                return;
            }
            // The attack is over: everybody's post comes off and the
            // next step musters them like anybody else.
            for who in 0..self.bims.len().min(self.sheltering.len()) {
                if self.sheltering[who] && self.bims[who].is_alive() {
                    self.bims[who].character.set_post(None);
                }
            }
            self.sheltering.clear();
            return;
        }
        self.sheltering.resize(self.bims.len(), false);
        for who in 0..self.bims.len() {
            let want = sheltering.get(who).copied().unwrap_or(false);
            let was = self.sheltering[who];
            self.sheltering[who] = want;
            if !self.bims[who].is_alive() {
                continue;
            }
            if !want {
                if was {
                    self.bims[who].character.set_post(None);
                }
                continue;
            }
            // Posted once, and again if anything took the post away —
            // the alarm coming up before the world said who shelters,
            // an order, a room built afresh.
            if self.bims[who].character.post().is_some() {
                continue;
            }
            self.bims[who].character.set_recruited(false);
            if let Some(at) = self.nearest_shelter(who) {
                self.post_at(who, at);
            }
        }
    }

    /// The nearest bunk to a body, in room units — the house it runs
    /// into. `None` in a room with no bunks at all, and then a body told
    /// to shelter simply stands down rather than fighting.
    fn nearest_shelter(&self, who: usize) -> Option<Vec2> {
        let from = self.bims.get(who)?.character.pos;
        self.room
            .beds
            .iter()
            .map(|b| b.frame.center())
            .min_by(|a, b| (*a - from).len().total_cmp(&(*b - from).len()))
    }

    /// Whether this body is sheltering from an attack (feature 94).
    pub fn is_sheltering(&self, who: usize) -> bool {
        self.sheltering.get(who).copied().unwrap_or(false)
    }

    /// Whether the world has told this room who shelters — a town under
    /// attack (feature 94), and nothing else. While it has, **every body
    /// that is not sheltering takes arms**, body nought included: a
    /// station's room has no player in it, and `players` is never under
    /// one, so the guard would otherwise be skipped as a player's own and
    /// stand there unarmed while the machines walked in.
    fn under_attack(&self) -> bool {
        !self.sheltering.is_empty()
    }

    /// A spot this room's people have eyes on whatever they are doing,
    /// in room units — or none. A hostile station's airlock: the world
    /// sets it to the tile just inside the station's door while the ship
    /// is docked (`world::crew::Residents::join`), and `set_hostiles`
    /// counts a target within [`AIRLOCK_WATCH`] tiles of it as seen, so
    /// its people notice a boarding at the door rather than when one of
    /// them happens to look down the right corridor. Nothing to a room
    /// whose bodies are not hostile.
    pub fn set_watched(&mut self, at: Option<Vec2>) {
        self.watched = at;
    }

    /// How many powered doors stand shut right now, for the tests.
    #[allow(dead_code)]
    pub fn doors_shut_for_probe(&self) -> usize {
        self.shut_now().len()
    }

    /// Whether `who`'s own eyes — or a peek beside a wall — see a point
    /// of the room, for the tests.
    #[allow(dead_code)]
    pub fn sees_for_probe(&self, who: usize, p: Vec2) -> bool {
        self.sees(who, p)
    }

    /// Where a hostile room's people believe each target is, for the
    /// tests: `last_seen` as handed to the fight.
    #[allow(dead_code)]
    pub fn believed_for_probe(&self) -> Vec<Option<Vec2>> {
        self.last_seen.iter().map(|s| s.map(|s| s.at)).collect()
    }

    /// Which of the enemies named by [`Game::set_hostiles`] are peeking
    /// from cover, index for index: a bolt reaching one is dodged half
    /// the time. Nobody is until this says so, every step.
    /// Where the targets the world named stand, index for index — `None`
    /// for one down. For the tests.
    #[allow(dead_code)]
    pub fn combat_targets_for_probe(&self) -> Vec<Option<Vec2>> {
        self.combat.target_positions()
    }

    /// The taunt on each target as the world last said it — the radius
    /// in room units, nought for none, and whether it pulls a blade
    /// (feature 77). For the tests.
    #[allow(dead_code)]
    pub fn hostiles_taunting_for_probe(&self) -> Vec<(f32, bool)> {
        self.combat
            .targets()
            .iter()
            .map(|t| t.map_or((0.0, false), |t| (t.taunting, t.magnet)))
            .collect()
    }

    /// The targets as the room holds them, for a test that wants to see
    /// what the world handed over.
    pub fn hostiles_for_probe(&self) -> Vec<Option<crate::combat::Target>> {
        self.combat.targets().to_vec()
    }

    /// Lock or unlock one of the room's doors outright, by its index in
    /// [`Game::door_states`] — no errand, no panel, no walk. For a test
    /// that wants a body shut in or out.
    pub fn lock_door_for_probe(&mut self, door: usize, locked: bool) -> bool {
        let Some(d) = self.room.doors.get_mut(door) else {
            return false;
        };
        if locked {
            d.lock(door::Locker::Crew);
        } else {
            d.unlock();
        }
        self.refresh_maps();
        self.refresh_blockers();
        true
    }

    /// One hit with a **strip** on it, laid on a body without anything
    /// flying: the Unmaker's rule asked of `strike_stripping` straight
    /// (feature 83). For a test, and nothing in the game calls it.
    pub fn strip_for_probe(&mut self, who: usize, part: Part, damage: f32, strips: f32) {
        self.strike_stripping(who, part, damage, false, strips);
    }

    pub fn set_hostiles_peeking(&mut self, peeking: &[bool]) {
        self.combat.set_peeking(peeking);
    }

    /// The odds each hostile dodges a bolt for its armour, index for
    /// index with `set_hostiles`, the way `set_hostiles_peeking` is: the
    /// world hands the other room's `Game::dodge`s across.
    pub fn set_hostiles_dodge(&mut self, dodge: &[f32]) {
        self.combat.set_dodge(dodge);
    }

    /// The odds a bolt reaching this Bim is dodged for what it wears —
    /// `Gear::dodge`, nought for anything below tier three.
    /// A machine wears nothing and slips nothing: its dodge is nought.
    pub fn dodge(&self, who: usize) -> f32 {
        if self.droid_at(who).is_some() {
            return 0.0;
        }
        self.bims.get(who).map_or(0.0, |b| b.gear.dodge())
    }

    /// The enemy a Bim is locked in a melee with — an index into the
    /// hostiles — or `None` free to fire.
    pub fn is_locked(&self, who: usize) -> Option<usize> {
        self.bims[who].locked
    }

    /// What is in a body's hand — or, for a machine, the arm it was
    /// built with, which is the same value and never an item.
    pub fn weapon(&self, who: usize) -> Option<Weapon> {
        match self.droid_at(who) {
            Some(i) => Some(self.droids[i].weapon),
            None => self.bims[who].gear.weapon,
        }
    }

    /// Where a body leans out to while it aims from a peek beside a
    /// wall, or `None` standing square.
    pub fn peek(&self, who: usize) -> Option<Vec2> {
        match self.droid_at(who) {
            Some(i) => self.droids[i].peek,
            None => self.bims[who].peek,
        }
    }

    /// Where a shot at a body is aimed: the peek it leans out to while
    /// it peeks, else where it stands. What the world hands the other
    /// room as the target's position.
    pub fn exposed_at(&self, who: usize) -> Vec2 {
        match self.droid_at(who) {
            Some(i) => self.droids[i].exposed_at(),
            None => self.bims[who].peek.unwrap_or(self.bims[who].character.pos),
        }
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
    /// for this room's own bodies. `from` and `at` in this room's units;
    /// `moving` while the enemy walked as it fired, for the odds.
    pub fn enemy_fire(&mut self, from: Vec2, at: Vec2, weapon: Weapon, moving: bool) {
        self.combat.fire(from, at, weapon, true, moving);
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
        self.count_hit_taken(&hit);
        self.strike(who, hit.part, damage, cut);
        self.wounds_taken.push(hit);
        self.attacked_for = ALARM_HOLD;
        let at = self.bims[who].character.pos;
        self.combat.cues.push(Cued {
            cue: Cue::Blow { cut, on_crew: true },
            at,
        });
        // An enemy blade's cut glows round the swinger, in the enemy's red
        // (feature 98). Only a blade cuts, so it is a schword's.
        if cut {
            let blade = crate::combat::WeaponKind::Schword.basic();
            self.combat.fx.cut(from, at, blade, true);
        }
        true
    }

    /// Every hostile bolt that landed on one of this room's own since
    /// last asked. Already applied — `tick_combat` wounds the body the
    /// step the bolt lands — so this is for the world to say what
    /// happened, not to do anything about it.
    pub fn take_wounds_taken(&mut self) -> Vec<Hit> {
        std::mem::take(&mut self.wounds_taken)
    }

    // --- the engineer's deployables (feature 74) ---------------------------

    /// The sentries on this deck, as the world keeps them: where each
    /// stands in room units, its weapon, its owner's skill and whether it
    /// is dug in. Said every step; one the world names again keeps its
    /// trigger, one it does not name is gone. Nothing in a room whose
    /// bodies are hostile — the crew's deck is the one they are laid on.
    pub fn set_sentries(&mut self, sentries: Vec<Sentry>) {
        let old = std::mem::take(&mut self.sentries);
        self.sentries = sentries
            .into_iter()
            .map(|mut s| {
                if let Some(was) = old.iter().find(|o| o.id == s.id) {
                    s.trigger = was.trigger;
                }
                s
            })
            .collect();
    }

    /// The sentries as last said, with their triggers.
    pub fn sentries(&self) -> &[Sentry] {
        &self.sentries
    }

    /// The damage each sentry took since last asked, by id, a hit a row.
    pub fn take_sentry_hits(&mut self) -> Vec<(u32, f32)> {
        std::mem::take(&mut self.sentry_hits)
    }

    /// The bolts the sandbags stopped since last asked: the tile of cover
    /// each landed in and its damage, for the world to take off a laid
    /// deployable there. See `Combat::take_cover_hits`.
    pub fn take_cover_hits(&mut self) -> Vec<((i32, i32), f32)> {
        self.combat.take_cover_hits()
    }

    /// The low cover laid on this deck at run time — deployed sandbags,
    /// in room units — the whole list, over the layout's own. Said every
    /// step; a fresh `Sight` starts with none, so a relayout, a join or
    /// an unjoin is answered by the next step's call.
    pub fn set_laid_cover(&mut self, laid: &[Rect]) {
        self.room.sight.set_laid_cover(laid);
    }

    /// The low cover laid at run time, as last set.
    pub fn laid_cover(&self) -> &[Rect] {
        self.room.sight.laid_cover()
    }

    /// An enemy's blow on a sentry — a melee `Shot` the world carried
    /// across and found nearest a sentry — landing if the enemy at `from`
    /// is still within reach of it, with the same slack a body gets.
    /// Whether it landed; the damage goes out through `take_sentry_hits`.
    pub fn enemy_strike_sentry(&mut self, from: Vec2, sentry: usize, damage: f32) -> bool {
        let Some(s) = self.sentries.get(sentry) else {
            return false;
        };
        if (s.at - from).len() > (MELEE_RANGE + 0.5) * TILE {
            return false;
        }
        let at = s.at;
        self.sentry_hits.push((s.id, damage));
        self.combat.lull_break();
        self.attacked_for = ALARM_HOLD;
        self.combat.cues.push(Cued {
            cue: Cue::Blow {
                cut: false,
                on_crew: true,
            },
            at,
        });
        true
    }

    /// What an engineer's talents do to a Bim's working steps, by index:
    /// a factor on a craft's and a factor on a build's, one each — the
    /// `effort` product takes the one for the errand on hand and no
    /// other. One for everybody the world does not name.
    pub fn set_work_factors(&mut self, factors: Vec<(f32, f32)>) {
        self.work_factors = factors;
    }

    /// Which Bims keep at a deploy when a hit lands on them — the
    /// engineer's *steady hands* — by index; anybody not named drops it
    /// and keeps the kit.
    pub fn set_steady_hands(&mut self, steady: Vec<bool>) {
        self.steady_hands = steady;
    }

    /// Send `who` to lay a kit on the tile at `tile` (room units, the
    /// tile's middle) beside which it will stand for `minutes` of working
    /// steps — an engineer's sandbags or sentry, the world having checked
    /// the kit is in the pack and the tile will take it. `sentry` is only
    /// carried back on `take_deployed`. The walk is to the nearest tile
    /// beside it, or the tile itself; nowhere to stand is `false` and
    /// nothing begun. A live order: what the Bim was on is put down onto
    /// the queue, as any order does.
    pub fn deploy(&mut self, who: usize, tile: Vec2, sentry: bool, minutes: f32) -> bool {
        if who >= self.bims.len() || !self.bims[who].is_alive() {
            return false;
        }
        let from = self.bims[who].character.pos;
        if task::deploy_stand(&self.room, &self.maps, tile, from).is_none() {
            return false;
        }
        self.interrupt_for_order(who);
        self.drop_ordered(who);
        let taken = self.taken_for(who);
        let bim = &mut self.bims[who];
        bim.task = Some(Task::deploy(
            who,
            tile,
            sentry,
            minutes,
            &mut bim.character,
            &mut self.room,
            &self.maps,
            &taken,
        ));
        true
    }

    /// Whether the tile whose middle is `tile` will take a kit laid by
    /// `who`: walkable deck floor with nothing blocking on it, not a door
    /// or an airlock, and a tile beside it — or itself — that `who` can
    /// walk to. Whether a deployable is there already is the world's to
    /// ask, since the world keeps them.
    pub fn deploy_tile_ok(&self, who: usize, tile: Vec2) -> bool {
        let Some(bim) = self.bims.get(who) else {
            return false;
        };
        let nav = self.maps.pick(self.room.bath.is_open());
        nav.interior().contains(tile)
            && nav.is_free(tile)
            && self.room.door_at(tile).is_none()
            && task::deploy_stand(&self.room, &self.maps, tile, bim.character.pos).is_some()
    }

    /// Every kit laid since last asked: who laid it, the middle of the
    /// tile in room units, and whether it was a sentry. The world puts
    /// the deployable down and takes the kit out of the pack.
    pub fn take_deployed(&mut self) -> Vec<(usize, Vec2, bool)> {
        std::mem::take(&mut self.room.deployed)
    }

    /// Whether a body at `body` is in low cover from something at `from`
    /// on this deck — `Sight::covered` — for the tests.
    #[allow(dead_code)]
    pub fn covered_for_probe(&self, body: Vec2, from: Vec2) -> bool {
        self.room.sight.covered(body, from)
    }

    // --- the soldier: the brace, the skills and the grenades (feature 75) --

    /// What a Bim shoots with over its weapon: the skill the world set,
    /// or none.
    fn skill(&self, who: usize) -> Skill {
        self.skills.get(who).copied().unwrap_or(Skill::NONE)
    }

    /// What each Bim's talents do to the one shooter, by index
    /// (`combat::Skill`): a soldier's, worked out by the world every step
    /// from its class, its talents and its brace. `Skill::NONE` for
    /// anybody not named.
    pub fn set_skills(&mut self, skills: Vec<Skill>) {
        self.skills = skills;
    }

    /// The skill the world set for `who`, for the tests.
    #[allow(dead_code)]
    pub fn skill_for_probe(&self, who: usize) -> Skill {
        self.skill(who)
    }

    // --- the commander: the squad's orders (feature 78) --------------------

    /// What a commander's squad order tells `who` to do, or
    /// [`Squad::None`].
    fn squad_of(&self, who: usize) -> Squad {
        self.squad.get(who).copied().unwrap_or_default()
    }

    /// What the squad is under, by index (`world::SquadOrder`): the
    /// world works it out every step from the commander's order, who is
    /// in range of him and who a player steers, and says it here.
    /// Anybody not named is under nothing.
    pub fn set_squad(&mut self, squad: Vec<Squad>) {
        self.squad = squad;
    }

    /// What the world last told `who` to do, for the tests.
    #[allow(dead_code)]
    pub fn squad_for_probe(&self, who: usize) -> Squad {
        self.squad_of(who)
    }

    /// A body stopped where it stands, for the tests: the walk dropped,
    /// which `put_for_probe` on its own does not do.
    #[allow(dead_code)]
    pub fn halt_for_probe(&mut self, who: usize) {
        if let Some(bim) = self.bims.get_mut(who) {
            bim.character.halt();
        }
    }

    /// Whether a body is standing still — not walking anywhere: what a
    /// commander's *anchor* reads off him.
    pub fn is_standing_still(&self, who: usize) -> bool {
        self.bims
            .get(who)
            .is_some_and(|b| !b.character.is_walking())
    }

    /// The ship's own doors — the powered sliding ones, not the heads' —
    /// each as its middle and the direction through it, for the tests.
    #[allow(dead_code)]
    pub fn ship_doors_for_probe(&self) -> Vec<(Vec2, Vec2)> {
        self.room
            .doors
            .iter()
            .map(|d| (d.rect.center(), d.through()))
            .collect()
    }

    /// *Runner*: the pace factor while an enemy is in sight, one
    /// otherwise and for anybody without the talent.
    fn runner(&self, who: usize) -> f32 {
        let pace = self.skill(who).pace;
        if pace == 1.0 {
            return 1.0;
        }
        let from = self.bims[who].character.pos;
        if self.combat.sees_any(&self.room.sight, from) {
            pace
        } else {
            1.0
        }
    }

    /// Brace, or stand easy: a soldier braced holds where it stands —
    /// whatever it was on put down onto the queue, its walk dropped, no
    /// errand taken, never running, under arms — until it is toggled off,
    /// ordered anywhere, or goes down. The world checks who may
    /// (`World::can_brace`); the room does as told. Whether it changed.
    pub fn set_braced(&mut self, who: usize, on: bool) -> bool {
        if who >= self.bims.len() || self.bims[who].braced == on {
            return false;
        }
        if on {
            if !self.bims[who].is_alive() || self.bims[who].character.is_unconscious() {
                return false;
            }
            self.interrupt(who);
            self.bims[who].pending_move = None;
            self.bims[who].character.halt();
            self.bims[who].character.set_post(None);
        }
        self.bims[who].braced = on;
        self.bims[who].character.set_braced(on);
        true
    }

    /// Whether a Bim is braced.
    pub fn is_braced(&self, who: usize) -> bool {
        self.bims.get(who).is_some_and(|b| b.braced)
    }

    /// A soldier's *rampage* stacks, as the world last set them.
    pub fn rampage(&self, who: usize) -> u32 {
        self.bims.get(who).map_or(0, |b| b.rampage)
    }

    /// Set a soldier's *rampage* stacks: the world counts the enemies it
    /// downs and clears them when the fight ends.
    pub fn set_rampage(&mut self, who: usize, stacks: u32) {
        if let Some(bim) = self.bims.get_mut(who) {
            bim.rampage = stacks;
        }
    }

    // --- the tank: the wall, the taunt and the hits (feature 77) -----------

    /// Stand as a wall, or stand down: a tank with Bulwark on walks at
    /// half pace and puts the crew close behind him in cover against a
    /// shot that would come through him. Unlike a brace it takes no
    /// errand away — he is a wall that walks. Off again when he goes
    /// down; the world checks who may (`World::can_bulwark`). Whether
    /// it changed.
    pub fn set_bulwark(&mut self, who: usize, on: bool) -> bool {
        if who >= self.bims.len() || self.bims[who].bulwark == on {
            return false;
        }
        if on && (!self.bims[who].is_alive() || self.bims[who].character.is_unconscious()) {
            return false;
        }
        self.bims[who].bulwark = on;
        true
    }

    /// Whether a Bim stands as a wall.
    pub fn is_bulwark(&self, who: usize) -> bool {
        self.bims.get(who).is_some_and(|b| b.bulwark)
    }

    /// Which target a Bim would shoot at from where it stands, for the
    /// tests: the one [`Combat::aim`] picks over its own weapon.
    #[allow(dead_code)]
    pub fn aims_at_for_probe(&self, who: usize) -> Option<usize> {
        let bim = self.bims.get(who)?;
        let stats = self.skill(who).stats(bim.gear.weapon?);
        let mark = match self.squad_of(who) {
            Squad::Attack { enemy, seen: true } => Some(enemy),
            _ => None,
        };
        self.combat
            .aim_marked(&self.room.sight, bim.character.pos, &stats, mark)
            .map(|(i, _, _)| i)
    }

    /// The walls the world last handed the shooter, for the tests.
    #[allow(dead_code)]
    pub fn bulwarks_for_probe(&self) -> Vec<crate::combat::Bulwark> {
        self.combat.bulwarks().to_vec()
    }

    /// Whether nothing is in the air or lit, for a probe that fires a
    /// bolt and wants to know when it has landed.
    #[allow(dead_code)]
    pub fn combat_quiet_for_probe(&self) -> bool {
        self.combat.quiet()
    }

    /// Whether the fight's passing lights are on in this room and how
    /// many are alive (feature 98), for the tests.
    #[allow(dead_code)]
    pub fn fx_count_for_probe(&self) -> (bool, usize) {
        (self.combat.fx.is_on(), self.combat.fx.count())
    }

    /// How fast a Bim is walking, as a fraction of its usual pace, for
    /// the tests.
    #[allow(dead_code)]
    pub fn pace_for_probe(&self, who: usize) -> f32 {
        self.bims.get(who).map_or(0.0, |b| b.character.pace())
    }

    /// A Bim's blood set to `share` of full, for a test that wants one
    /// slowed without shooting it.
    #[allow(dead_code)]
    pub fn bleed_for_probe(&mut self, who: usize, share: f32) {
        if let Some(bim) = self.bims.get_mut(who) {
            bim.health.set_blood_for_probe(share);
        }
    }

    /// The tanks standing as walls among this room's own bodies, as the
    /// world last said: which body, how far it reaches in room units,
    /// and whether it *interposes*. Handed to the one shooter every
    /// step, beside the skills.
    pub fn set_bulwarks(&mut self, bulwarks: Vec<crate::combat::Bulwark>) {
        self.combat.set_bulwarks(bulwarks);
    }

    /// How far a taunt runs on each of the enemies named by
    /// [`Game::set_hostiles`], in room units — nought for one not
    /// taunting — and whether it pulls charging blades (*magnet*).
    /// Index for index with `set_hostiles`, the way
    /// [`Game::set_hostiles_peeking`] is.
    pub fn set_hostiles_taunting(&mut self, radius: &[f32], magnet: &[bool]) {
        self.combat.set_taunting(radius, magnet);
    }

    /// Enemy hits that have landed on a body since the last point of
    /// experience they made — the tank's (feature 77).
    pub fn hits_taken(&self, who: usize) -> u32 {
        self.bims.get(who).map_or(0, |b| b.hits_taken)
    }

    /// Seconds a body has been dying with an enemy about (feature 78):
    /// what `is_fleeing` measures a commander's aura's hold against.
    pub fn fear(&self, who: usize) -> f32 {
        self.bims.get(who).map_or(0.0, |b| b.fear)
    }

    /// Set that count: the world takes the whole points out of it and
    /// leaves the remainder.
    pub fn set_hits_taken(&mut self, who: usize, hits: u32) {
        if let Some(bim) = self.bims.get_mut(who) {
            bim.hits_taken = hits;
        }
    }

    /// One more enemy hit landed on a body: counted where the hit is
    /// applied, so armour, a surge and the body itself all count and a
    /// miss or a dodge does not. A hit from this room's own side — a
    /// soldier's grenade, which carries `by` — is not one.
    fn count_hit_taken(&mut self, hit: &crate::combat::Hit) {
        if hit.by.is_none()
            && let Some(bim) = self.bims.get_mut(hit.who)
        {
            bim.hits_taken = bim.hits_taken.saturating_add(1);
        }
    }

    // --- the medic: the beam and the surge (feature 76) --------------------

    /// Whether a medic holds a beam, as the world last said and the room
    /// has not since ended: off again on an order to an errand
    /// (`Game::order`) and when the medic goes down. The world reads it
    /// back every step and breaks the link when it is off.
    pub fn is_beaming(&self, who: usize) -> bool {
        self.bims.get(who).is_some_and(|b| b.beaming)
    }

    /// The world's word that a medic holds a beam, or holds none. The
    /// picture and the rule above; who is held is the world's list.
    pub fn set_beaming(&mut self, who: usize, on: bool) {
        if let Some(bim) = self.bims.get_mut(who) {
            bim.beaming = on;
        }
    }

    /// A medic's surge on a body: for `seconds` of the room's clock a
    /// hit takes nothing from it (`Game::strike`); `closing` closes
    /// every open wound as it ends. The world checks who may; the room
    /// does as told. A body dead is left alone. A surge on a body already
    /// surging runs from now.
    pub fn set_surge(&mut self, who: usize, seconds: f32, closing: bool) {
        if let Some(bim) = self.bims.get_mut(who).filter(|b| b.is_alive()) {
            bim.surge = Some(crate::bim::Surge {
                left: seconds,
                closing,
            });
            bim.character.set_surging(true);
        }
    }

    /// Seconds of the room's clock a body's surge has left; nought with
    /// none running.
    pub fn surge_left(&self, who: usize) -> f32 {
        self.bims
            .get(who)
            .and_then(|b| b.surge)
            .map_or(0.0, |s| s.left.max(0.0))
    }

    /// Whether a surge runs on a body.
    pub fn is_surging(&self, who: usize) -> bool {
        self.bims.get(who).is_some_and(|b| b.surge.is_some())
    }

    /// Whether a surge on a body closes its wounds as it ends.
    pub fn surge_closing(&self, who: usize) -> bool {
        self.bims
            .get(who)
            .and_then(|b| b.surge)
            .is_some_and(|s| s.closing)
    }

    // --- carrying a body out of the fire (feature 86) ----------------------

    /// Whom `who` has in its arms, if anybody.
    pub fn carrying(&self, who: usize) -> Option<usize> {
        self.bims.get(who).and_then(|b| b.carrying)
    }

    /// Who is carrying `who`, if anybody. Derived rather than kept: a
    /// crew is a handful of bodies, and one truth about a carry is one
    /// thing to put back when an index moves.
    pub fn carried_by(&self, who: usize) -> Option<usize> {
        self.bims.iter().position(|b| b.carrying == Some(who))
    }

    /// Whether `who` is in somebody's arms.
    pub fn is_carried(&self, who: usize) -> bool {
        self.carried_by(who).is_some()
    }

    /// Whether a body is worth fetching out of the fire: alive, on this
    /// deck, and either out cold, in a dying state, or bleeding through
    /// a wound nobody has dressed. The hurt as well as the unconscious,
    /// because a crew member that is still on its feet and losing blood
    /// is the one a medic can actually save.
    pub fn needs_rescue(&self, who: usize) -> bool {
        self.bims.get(who).is_some_and(|b| {
            b.is_alive()
                && !b.character.is_outside()
                && (b.character.is_unconscious() || b.health.dying() || b.health.bleeding() > 0)
        })
    }

    /// Whether `who` could pick `patient` up this instant: `who` alive,
    /// awake, on the deck and with its arms free, `patient` somebody
    /// else who [`Game::needs_rescue`] and is in nobody's arms already,
    /// and the two within [`CARRY_REACH`] of one another. The world asks
    /// this before it sends the order and the room asks it again as the
    /// order lands.
    pub fn can_take_up(&self, who: usize, patient: usize) -> bool {
        if who == patient || who >= self.bims.len() || patient >= self.bims.len() {
            return false;
        }
        let carrier = &self.bims[who];
        if !carrier.is_alive()
            || carrier.character.is_unconscious()
            || carrier.character.is_outside()
            || carrier.carrying.is_some()
        {
            return false;
        }
        if !self.needs_rescue(patient) || self.is_carried(patient) {
            return false;
        }
        // Nobody carries a carrier: the arms at the end of the chain
        // would be holding two bodies.
        if self.bims[patient].carrying.is_some() {
            return false;
        }
        let gap = (self.bims[patient].character.pos - carrier.character.pos).len();
        gap <= CARRY_REACH * TILE
    }

    /// Pick a crewmate up: whatever the carrier was on is put down onto
    /// its queue, its walk dropped, and the body in its arms stops
    /// walking anywhere of its own from this step. Whether it happened.
    pub fn take_up(&mut self, who: usize, patient: usize) -> bool {
        if !self.can_take_up(who, patient) {
            return false;
        }
        // The one being carried is off whatever it was doing, and so is
        // the medic: both its arms are the carry now.
        self.interrupt(patient);
        self.bims[patient].pending_move = None;
        self.bims[patient].character.halt();
        self.bims[patient].character.set_post(None);
        self.bims[patient].character.set_falling_back(None);
        self.bims[who].carrying = Some(patient);
        true
    }

    /// Set down whatever `who` is carrying, where it stands: who it was,
    /// or `None` for empty arms. The body is put on the nearest free
    /// spot beside the carrier so that the two do not stand inside each
    /// other.
    pub fn set_down(&mut self, who: usize) -> Option<usize> {
        let patient = self.bims.get_mut(who)?.carrying.take()?;
        let here = self.bims[who].character.pos;
        let nav = self.maps.for_body(false, self.room.bath.is_open());
        let beside = nav.nearest_free(here + Vec2::from_angle(self.combat.roll() * TAU) * TILE);
        self.bims[patient].character.stand_at(beside);
        self.bims[patient].character.halt();
        Some(patient)
    }

    /// Whether this body is a field medic, as the world last said.
    pub fn is_field_medic(&self, who: usize) -> bool {
        self.bims.get(who).is_some_and(|b| b.field_medic)
    }

    /// The world's word that a crew member is a hired field medic
    /// (feature 86): said every step, the way the squad's orders are,
    /// since it is the world that keeps the contract.
    pub fn set_field_medic(&mut self, who: usize, on: bool) {
        if let Some(bim) = self.bims.get_mut(who) {
            bim.field_medic = on;
        }
    }

    /// Every carried body stood where its carrier stands, and every
    /// carry that can no longer hold let go: a carrier dead, out cold or
    /// outside, or a body that has died in its arms. Run at the end of
    /// the step, after everybody has moved and after
    /// [`Game::separate_under_arms`], so the body ends the step in the
    /// carrier's arms rather than a frame behind them.
    fn carry_the_carried(&mut self) {
        for who in 0..self.bims.len() {
            let Some(patient) = self.bims[who].carrying else {
                continue;
            };
            let carrier_up = self.bims[who].is_alive()
                && !self.bims[who].character.is_unconscious()
                && !self.bims[who].character.is_outside();
            if !carrier_up || patient >= self.bims.len() || !self.bims[patient].is_alive() {
                self.set_down(who);
                continue;
            }
            // In the arms: at the carrier's own spot, a little ahead of
            // it so the two read as one body carrying another rather
            // than as two standing in the same place.
            let at = self.bims[who].character.pos
                + Vec2::from_angle(self.bims[who].character.heading) * (0.35 * TILE);
            self.bims[patient].character.halt();
            self.bims[patient].character.stand_at(at);
        }
    }

    /// The living crew member under a room point, if any: a click's
    /// reach (`Character::picked_at`), the first by index. Nothing is
    /// noted — the hover's and the E key's question.
    pub fn crew_at(&self, x: f32, y: f32) -> Option<usize> {
        let p = vec2(x, y);
        self.bims
            .iter()
            .position(|b| b.is_alive() && b.character.picked_at(p))
    }

    /// Whether `who`'s own eyes — or a peek beside a wall — see a point
    /// of the room: the trace's rule, walls, shut doors and the dark.
    pub fn sees(&self, who: usize, p: Vec2) -> bool {
        self.bims
            .get(who)
            .is_some_and(|b| self.room.sight.sees_from(b.character.pos, p).is_some())
    }

    /// Whether nothing opaque — a wall, a shut door — stands on the
    /// straight line from `a` to `b`: what a throw and a burst ask.
    /// Sandbags are seen over, so they stop neither.
    pub fn line_clear(&self, a: Vec2, b: Vec2) -> bool {
        self.room.sight.first_opaque_along(a, b).is_none()
    }

    /// Whether the tile whose middle is `at` is deck of this room a body
    /// could stand on: walkable floor with nothing blocking on it. What a
    /// grenade may be thrown at.
    pub fn is_deck_tile(&self, at: Vec2) -> bool {
        let nav = self.maps.pick(self.room.bath.is_open());
        nav.interior().contains(at) && nav.is_free(at)
    }

    /// Whether an **attack banner** may be put down here (feature 84):
    /// a deck tile, or anywhere at all on a plain — the ground beyond
    /// the deck's box is walked on the body's own window rather than on
    /// the room's grid, so there is no tile here to ask, and a bot sent
    /// out there walks the far walk an ordered one walks.
    pub fn is_banner_tile(&self, at: Vec2) -> bool {
        self.is_deck_tile(at) || self.room.plane.is_some()
    }

    /// Throw a grenade from `who`'s hands at `at` (room units, a tile's
    /// middle): in the air and then on the tile with its fuse burning
    /// for `fuse` seconds, bursting with `radius` room units and `damage`
    /// at the centre. The world checks the throw (`World::can_throw`) and
    /// takes the grenade out of the pack; the room throws. The thrower
    /// faces the tile.
    pub fn throw_grenade(&mut self, who: usize, at: Vec2, fuse: f32, radius: f32, damage: f32) {
        if who >= self.bims.len() {
            return;
        }
        let from = self.bims[who].character.pos;
        if (at - from).len() > 1e-3 && !self.bims[who].character.is_walking() {
            self.bims[who].character.face((at - from).angle());
        }
        self.combat.throw(who, from, at, fuse, radius, damage);
    }

    /// The grenades in the air or lying with their fuses burning.
    pub fn grenades(&self) -> &[Grenade] {
        &self.combat.grenades
    }

    /// The tiles of laid sandbags a burst reached since last asked, for
    /// the world to take the deployables off.
    pub fn take_bags_blown(&mut self) -> Vec<(i32, i32)> {
        std::mem::take(&mut self.bags_blown)
    }

    /// A grenade's hit on one of this room's own: `strike` — through the
    /// armour on the part, as a strike rather than a cut — and the blood
    /// thrown over the tiles round the body the way a cut throws it.
    pub fn blast(&mut self, who: usize, part: Part, damage: f32) -> WoundOutcome {
        let out = self.strike(who, part, damage, false);
        if out.through > 0.0 {
            let at = self.bims[who].character.pos;
            let nav = self.maps.for_body(false, self.room.bath.is_open());
            self.room
                .filth
                .splash_blood(at, &mut self.rng, |tile| nav.can_reach(at, tile));
        }
        out
    }

    /// What a grenade's burst reaches, at `g.at` with `g.radius`: every
    /// body of this room's own on its feet and every target standing,
    /// within the radius and with nothing opaque between (walls and shut
    /// doors stop it, sandbags do not), takes `g.damage` at the centre
    /// falling in a straight line to half at the edge — halved again for
    /// a body in cover from the burst's side, peeking or behind bags —
    /// on a part rolled off the combat stream; the sentries in it the
    /// same, on their health; the laid sandbags in it are gone. The parts
    /// of the ship and the station are untouched. Own bodies first, by
    /// index, then the targets, then the sentries, then the bags, so two
    /// runs on one seed roll the same.
    fn burst(&mut self, g: Grenade) {
        let reaches = |game: &Game, at: Vec2| -> Option<f32> {
            let d = (at - g.at).len();
            if d > g.radius || !game.line_clear(g.at, at) {
                return None;
            }
            let share = 1.0 - 0.5 * (d / g.radius.max(1e-3));
            Some(g.damage * share)
        };
        let by = Some(g.by);
        // This room's own, the thrower included.
        let crew = self.bims.len();
        for who in 0..crew {
            let b = &self.bims[who];
            if !b.is_alive() || b.character.is_outside() || b.character.is_unconscious() {
                continue;
            }
            let at = b.character.pos;
            let Some(mut damage) = reaches(self, at) else {
                continue;
            };
            if b.peek.is_some() || self.room.sight.cover_between(at, g.at).is_some() {
                damage *= 0.5;
            }
            let hit = self.combat.blast(who, damage, by);
            self.blast(who, hit.part, hit.damage);
            self.wounds_taken.push(hit);
            self.attacked_for = ALARM_HOLD;
            self.combat.cues.push(Cued {
                cue: Cue::Impact { on_crew: true },
                at,
            });
        }
        // The targets: the enemy's people standing on this deck, at their
        // exposed positions, in cover the same way.
        let targets: Vec<Option<(Vec2, bool)>> = self
            .combat
            .targets()
            .iter()
            .map(|t| t.map(|t| (t.at, t.peeking)))
            .collect();
        for (i, target) in targets.into_iter().enumerate() {
            let Some((at, peeking)) = target else {
                continue;
            };
            let Some(mut damage) = reaches(self, at) else {
                continue;
            };
            if peeking || self.room.sight.cover_between(at, g.at).is_some() {
                damage *= 0.5;
            }
            self.combat.blast_target(i, damage, by);
        }
        // The sentries, on their one pool.
        let sentries: Vec<(u32, Vec2)> = self.sentries.iter().map(|s| (s.id, s.at)).collect();
        for (id, at) in sentries {
            if let Some(damage) = reaches(self, at) {
                self.sentry_hits.push((id, damage));
            }
        }
        // And the laid sandbags: gone, whatever they had left.
        let bags: Vec<Vec2> = self
            .room
            .sight
            .laid_cover()
            .iter()
            .map(|r| r.center())
            .collect();
        for at in bags {
            if reaches(self, at).is_some() {
                self.bags_blown
                    .push(((at.x / TILE).floor() as i32, (at.y / TILE).floor() as i32));
            }
        }
    }

    /// The work factors the world set for `who`, for the tests.
    #[allow(dead_code)]
    pub fn work_factors_for_probe(&self, who: usize) -> (f32, f32) {
        self.work_factors.get(who).copied().unwrap_or((1.0, 1.0))
    }

    /// A bolt the sandbags on `tile` stopped, said as the fight would say
    /// it — for the tests, which want the bags hit without a fight.
    #[allow(dead_code)]
    pub fn cover_hit_for_probe(&mut self, tile: (i32, i32), damage: f32) {
        self.combat.cover_hit_for_probe(tile, damage);
    }

    /// A hit on sentry `id`, said as a bolt landing on it would be.
    #[allow(dead_code)]
    pub fn sentry_hit_for_probe(&mut self, id: u32, damage: f32) {
        self.sentry_hits.push((id, damage));
    }

    /// Where a Bim stands to work bench `bench`, for the tests.
    #[allow(dead_code)]
    pub fn bench_spot_for_probe(&self, bench: usize) -> Vec2 {
        self.room.benches[bench].at
    }

    /// Whether `who` is on a craft of `recipe` right now, for the tests.
    #[allow(dead_code)]
    pub fn is_at_work_for_probe(&self, who: usize, recipe: u32) -> bool {
        self.bims
            .get(who)
            .and_then(|b| b.task.as_ref())
            .is_some_and(|t| matches!(t.kind(), Kind::Craft { recipe: r, .. } if r == recipe))
    }

    /// Whether `who` is on a deploy errand, for the world and the panels.
    pub fn is_deploying(&self, who: usize) -> bool {
        self.bims
            .get(who)
            .and_then(|b| b.task.as_ref())
            .is_some_and(|t| matches!(t.kind(), Kind::Deploy { .. }))
    }

    /// Where `who` is putting something together and how far through the
    /// errand it is (feature 91): the middle of the tile an engineer is
    /// laying a kit on, or the middle of the construction site whoever it
    /// is is building, in **room** units, with the chain's progress from
    /// nought to one. `None` for every other errand and for a Bim with
    /// nothing on hand.
    ///
    /// The walk counts towards it, because the number is the errand's own
    /// (`Task::progress`) and because a bar standing on the tile before
    /// the builder arrives is what says *that* tile is the one being
    /// worked. A site the world has stopped asking for — the ship under
    /// way, the order cancelled — has no entry in `Room::builds` and so
    /// no bar, which is the right answer: nothing is being built there
    /// any more.
    pub fn working_at(&self, who: usize) -> Option<(Vec2, f32)> {
        let task = self.bims.get(who)?.task.as_ref()?;
        let at = match task.kind() {
            Kind::Deploy { .. } => task.kind().deploy_tile()?,
            Kind::Build { site, .. } => {
                let build = self.room.builds.iter().find(|b| b.site == site)?;
                let mut box_of = *build.tiles.first()?;
                for tile in &build.tiles[1..] {
                    box_of.min = vec2(box_of.min.x.min(tile.min.x), box_of.min.y.min(tile.min.y));
                    box_of.max = vec2(box_of.max.x.max(tile.max.x), box_of.max.y.max(tile.max.y));
                }
                (box_of.min + box_of.max) * 0.5
            }
            _ => return None,
        };
        Some((at, task.progress()))
    }

    /// The lamps: where each is, what it has left and how bright it is
    /// shown. See `sight::Lamp`. A bolt that lands on one takes its
    /// damage off it, and at nought it is out.
    pub fn lamps(&self) -> &[crate::sight::Lamp] {
        self.room.sight.lamps()
    }

    /// The lamp on the tile a room point is in, if any, with its index.
    pub fn lamp_at(&self, p: Vec2) -> Option<(usize, &crate::sight::Lamp)> {
        self.room.sight.lamp_at(p)
    }

    /// Which lamps' health changed since last asked — the world's cue to
    /// remember it across a relayout and to carry it to the other room a
    /// docked fight is mirrored in.
    pub fn take_lamp_changes(&mut self) -> Vec<usize> {
        self.room.sight.take_lamp_changes()
    }

    /// Lamp `i` as the world remembers it. See `Sight::set_lamp_health`.
    pub fn set_lamp_health(&mut self, i: usize, health: f32) {
        self.room.sight.set_lamp_health(i, health);
    }

    /// Daylight over `over`, or none: the tiles under it are lit whatever
    /// the lamps say. The world's word for a settlement's ground; it
    /// outlives a relayout. See `Sight::set_daylight`.
    pub fn set_daylight(&mut self, over: Option<Rect>) {
        self.room.sight.set_daylight(over);
    }

    /// Lamp `i` has power or has not — dark and whole without it. The
    /// world's, off the ship's wiring and its brownout. See
    /// `Sight::set_lamp_powered`.
    pub fn set_lamp_powered(&mut self, i: usize, powered: bool) {
        self.room.sight.set_lamp_powered(i, powered);
    }

    /// A bolt landed on lamp `i` for `damage`, with nothing fired: the
    /// hit as the fight would land it, flicker and all, and the world
    /// told of it like any other. For probes and `BIMS_LAMPS_OUT`.
    pub fn damage_lamp_for_probe(&mut self, i: usize, damage: f32) {
        self.room.sight.damage_lamp(i, damage);
    }

    /// Everything worth hearing since last asked — the doors, the board
    /// and the fight — in the order it happened. See `crate::cue`. The
    /// host plays them; a server drops them.
    pub fn take_cues(&mut self) -> Vec<Cued> {
        let mut cues = std::mem::take(&mut self.room.cues);
        cues.append(&mut self.combat.cues);
        cues
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

    /// The opposite, for a probe or a test that wants a body: dead on its
    /// next tick, by the same check as any death.
    #[allow(dead_code)]
    pub fn kill_for_probe(&mut self, who: usize) {
        self.bims[who].health.give_up();
    }

    /// The blood set to a share of `health::MAX_BLOOD`, for a probe or a
    /// test that wants a body out cold or bled out without waiting for a
    /// wound to empty it — `Health::set_blood_for_probe`.
    #[allow(dead_code)]
    pub fn set_blood_for_probe(&mut self, who: usize, share: f32) {
        self.bims[who].health.set_blood_for_probe(share);
    }

    /// A body laid where it fell, for a room built over a grave (feature
    /// 85): stood at `at` — snapped to somewhere a body fits, the way an
    /// [`Game::adopt`] snaps one — and dead from this instant, without
    /// waiting for a tick and without a word in anybody's diary. The
    /// death happened before this room was built; what is being laid out
    /// is the station's memory of it, and the living here never saw it.
    pub fn lay_out_dead(&mut self, who: usize, at: Vec2) {
        if who >= self.bims.len() {
            return;
        }
        let nav = self.maps.pick(self.room.bath.is_open());
        let spot = nav.nearest_free(at);
        self.bims[who].character.stand_at(spot);
        self.bims[who].character.set_scripted(false);
        self.bims[who].task = None;
        self.bims[who].queue.clear();
        self.bims[who].health.give_up();
        self.bims[who].character.die();
        // Its bunk is nobody's: a body does not sleep, and the living
        // here want the beds.
        if let Some(bed) = self.room.sleeps_in.get_mut(who) {
            *bed = None;
        }
    }

    /// Out cold where it stands, for a probe: the blood put just under the
    /// line with nothing open, so it lies there for days rather than
    /// bleeding out in minutes. Takes at the top of its next tick, like a
    /// knock-out.
    #[allow(dead_code)]
    pub fn knock_out_for_probe(&mut self, who: usize) {
        self.bims[who]
            .health
            .set_blood_for_probe(crate::health::OUT_AT * 0.9);
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
        self.strike_stripping(who, part, damage, cut, 0.0)
    }

    /// [`Game::strike`] with the Unmaker's **strip** carried (feature
    /// 83, `bims::combat::WeaponStats::strips`): with `strips` above
    /// nought and the struck part wearing an **unbroken** piece, the
    /// piece loses that much with its own protection ignored and the
    /// part takes nothing — what the piece cannot take is **lost**,
    /// rather than reaching the body. A bare part, or one whose piece is
    /// already broken, takes the plain `damage` the ordinary way. Every
    /// other weapon strips nought and this is `strike` exactly.
    ///
    /// A medic's surge still takes the whole of it, and a tank's *iron
    /// frame* still moves a head shot onto the body first: the strip
    /// then lands on the kevlar, which is the piece that would have
    /// taken the damage.
    pub fn strike_stripping(
        &mut self,
        who: usize,
        part: Part,
        damage: f32,
        cut: bool,
        strips: f32,
    ) -> WoundOutcome {
        let mut out = WoundOutcome::default();
        if !self.bims.get(who).is_some_and(|b| b.is_alive()) {
            return out;
        }
        if strips > 0.0 {
            let skill = self.skill(who);
            let part = if skill.iron_frame && part == Part::Head {
                Part::Body
            } else {
                part
            };
            let bim = &mut self.bims[who];
            bim.hit_flash = HIT_FLASH;
            if bim.surge.is_some() {
                out.absorbed = damage;
                self.combat.fx.struck(who, part.code());
                return out;
            }
            let mut broke = None;
            if let Some(piece) = bim.gear.worn_mut(part).as_mut().filter(|p| !p.broken()) {
                // Protection ignored, and the rest of the strip lost
                // with the piece: the lance unmakes the armour and does
                // nothing to what is under it.
                self.combat.fx.struck(who, part.code());
                piece.health = (piece.health - strips).max(0.0);
                if piece.broken() {
                    broke = Some(piece.kind);
                }
                out.absorbed = damage;
                if let Some(kind) = broke {
                    out.piece_broke = true;
                    self.pieces_broken.push((who, kind));
                    self.refresh_worn(who);
                }
                return out;
            }
            // Nothing over it, or nothing left of what is: the part
            // takes the damage like any other hit.
        }
        // A hit on an engineer laying a kit is the kit put down where it
        // was — in the pack — and the errand dropped, not put down onto
        // the queue to be picked up again under fire; unless its hands are
        // steady (`set_steady_hands`), when it keeps at it.
        if self.is_deploying(who) && !self.steady_hands.get(who).copied().unwrap_or(false) {
            self.drop_task(who);
        }
        let skill = self.skill(who);
        // *Iron frame* (feature 77): a hit rolled on the head lands on
        // the body, so the kevlar takes what the helm would have.
        let part = if skill.iron_frame && part == Part::Head {
            Part::Body
        } else {
            part
        };
        let bim = &mut self.bims[who];
        bim.hit_flash = HIT_FLASH;
        // And the picture's flash, on the part it struck (feature 98).
        self.combat.fx.struck(who, part.code());
        // A medic's surge on it takes the whole of the hit (feature 76):
        // no wound, no armour drained, no trauma — the flash and nothing
        // else.
        if bim.surge.is_some() {
            out.absorbed = damage;
            return out;
        }
        let mut through = damage;
        let mut broke = None;
        if let Some(piece) = bim.gear.worn_mut(part).as_mut().filter(|p| !p.broken()) {
            // *Plated* multiplies what the piece stops; an engineer's
            // *higher quality armour* adds a point on top (feature 88).
            through -= piece.effective_protection() * skill.armour_protection
                + skill.armour_protection_add;
            if through <= 0.0 {
                out.absorbed = damage;
                return out;
            }
            // What the piece can still take: its health at the rate it
            // drains — a tank's armour drains slower, so the same piece
            // absorbs more on him, and what it has stored never changes
            // meaning (feature 77).
            let drain = skill.armour_drain.max(1e-6);
            let capacity = piece.health / drain;
            if through <= capacity {
                piece.health -= through * drain;
                through = 0.0;
                out.absorbed = damage;
            } else {
                through -= capacity;
                piece.health = 0.0;
                out.absorbed = damage - through;
            }
            if piece.broken() {
                broke = Some(piece.kind);
            }
        }
        if through > 0.0 {
            out.through = through;
            // The dying state a part reaching nothing turns into is
            // rolled off the combat stream, like the part the bolt hit.
            let roll = self.combat.roll();
            let bim = &mut self.bims[who];
            out.trauma = bim.health.shot(part, through, cut, roll);
            out.leg_lost = out.trauma.is_some_and(|t| t.loses_leg());
            if let Some(trauma) = out.trauma {
                self.traumas.push((who, trauma));
            }
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
    /// A machine is never out cold: there is no dying state, so it is
    /// up or it is a wreck.
    pub fn is_unconscious(&self, who: usize) -> bool {
        match self.droid_at(who) {
            Some(_) => false,
            None => self.bims[who].character.is_unconscious(),
        }
    }

    // --- dressing a wound ----------------------------------------------------

    /// Send `who` to dress `part` of `patient` — itself, or a crewmate —
    /// with a dressing out of its own pack: the walk to the patient and ten
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
            || self.bandages_of(who) == 0
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
    /// the part closed, one dressing out of the helper's pack, the blotch
    /// off the body. Only where the helper is still beside the patient —
    /// within two tiles, or is the patient — and it is still carrying a
    /// dressing: a patient that walked off mid-dressing, or a pack
    /// emptied since the order, is ten minutes lost and nothing else. A
    /// patient dead in the meantime is past dressing.
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
            if (helper != patient && apart > 2.0 * TILE) || self.bandages_of(helper) == 0 {
                continue;
            }
            let bim = &mut self.bims[patient];
            if bim.health.bandage(part) {
                let wounds = Part::ALL.map(|p| bim.health.wounds(p) > 0);
                bim.character.set_wounds(wounds);
                // Out of the helper's own pack (feature 87), the box
                // emptied when it was the last one in it.
                self.spend_bandage(helper);
                self.healings.push(Healed {
                    helper,
                    patient,
                    with: Healing::Bandage,
                });
                continue;
            }
            let bim = &mut self.bims[patient];
            let wounds = Part::ALL.map(|p| bim.health.wounds(p) > 0);
            bim.character.set_wounds(wounds);
        }
    }

    /// Every dressing and treatment finished since the world last asked,
    /// with what it used (feature 76).
    pub fn take_healings(&mut self) -> Vec<Healed> {
        std::mem::take(&mut self.healings)
    }

    /// How many dressings that Bim has **in its own pack** (feature 87).
    /// There is no count on a shelf any more: a bandage is a thing, and
    /// the one a Bim binds a wound with is the one it is carrying.
    pub fn bandages_of(&self, who: usize) -> u32 {
        self.bims
            .get(who)
            .map_or(0, |bim| bim.gear.units_of(BANDAGE))
    }

    /// Leave exactly `n` dressings in that Bim's pack: every box taken
    /// out and `n` dealt again. For the tests and the probes, which
    /// want a Bim carrying one and not a boxful.
    pub fn set_bandages_for_probe(&mut self, who: usize, n: u32) {
        let Some(bim) = self.bims.get_mut(who) else {
            return;
        };
        for cell in 0..PACK_CELLS {
            if bim.gear.pack[cell] == Some(BANDAGE) {
                bim.gear.take_out(cell);
            }
        }
        self.give_stack(who, BANDAGE, n);
    }

    /// Spend one out of that Bim's pack: the emptiest box first, so the
    /// pack tidies itself by being used. `false` when it carries none.
    fn spend_bandage(&mut self, who: usize) -> bool {
        let Some(bim) = self.bims.get_mut(who) else {
            return false;
        };
        let Some(cell) = bim.gear.stack_to_spend(BANDAGE) else {
            return false;
        };
        bim.gear.take_one(cell).is_some()
    }

    /// The part of `patient` worth dressing next: the one bleeding most,
    /// or `None` for a body with nothing open on it.
    pub fn worst_wound(&self, patient: usize) -> Option<Part> {
        let bim = self.bims.get(patient)?;
        Part::ALL
            .into_iter()
            .filter(|&p| bim.health.wounds(p) > 0)
            .max_by_key(|&p| bim.health.wounds(p))
    }

    /// **Bandage every wound on one body** (feature 87): the worst part
    /// now and the rest queued behind it, the way a Shift-click queues
    /// orders — one dressing a part, out of `who`'s own pack, for as
    /// many parts as it has dressings for. What the pop-up on a box of
    /// dressings sends, and what a Bim that has run out of the fight
    /// reaches for. `false` when nothing was started at all.
    pub fn bandage_all(&mut self, who: usize, patient: usize) -> bool {
        if who >= self.bims.len() || patient >= self.bims.len() {
            return false;
        }
        let mut parts: Vec<Part> = Part::ALL
            .into_iter()
            .filter(|&p| self.bims[patient].health.wounds(p) > 0)
            .collect();
        parts.sort_by_key(|&p| core::cmp::Reverse(self.bims[patient].health.wounds(p)));
        // No more than there are dressings for: the rest would be ten
        // minutes' walking with nothing in hand at the end of it.
        parts.truncate(self.bandages_of(who) as usize);
        let mut started = false;
        for part in parts {
            if !started {
                started = self.bandage(who, patient, part);
                continue;
            }
            self.order_later(
                0,
                crate::order::CrewOrder::Bandage {
                    who: who as u32,
                    patient: patient as u32,
                    part,
                },
            );
        }
        started
    }

    // --- treating a dying state ---------------------------------------------

    /// The dying state on one part of a Bim, untreated — see
    /// `health::Trauma`. `None` for a part above nothing.
    pub fn trauma(&self, who: usize, part: Part) -> Option<Trauma> {
        self.bims[who].health.trauma(part)
    }

    /// Whether any part is at nothing with its trauma untreated: it runs
    /// from a fight, and needs a medkit from a crewmate.
    pub fn is_dying(&self, who: usize) -> bool {
        self.bims[who].health.dying()
    }

    /// What treated traumas have left on it, each with the game minutes it
    /// has to run.
    pub fn lasting(&self, who: usize) -> &[Lasting] {
        self.bims[who].health.lasting()
    }

    /// Every dying state a hit put one of this room's Bims in since the
    /// world last asked — whose, and which — and every one a medkit took
    /// one out of.
    pub fn take_traumas(&mut self) -> Vec<(usize, Trauma)> {
        std::mem::take(&mut self.traumas)
    }

    pub fn take_treated(&mut self) -> Vec<(usize, Trauma)> {
        std::mem::take(&mut self.treated)
    }

    /// Send `who` to treat the trauma on `part` of `patient` — a crewmate,
    /// never itself: a Bim with a part at nothing is past doctoring
    /// itself — with the kit in its own pack if it has one, else one of
    /// the room's medkits off a shelf: the walk over and
    /// [`task::TREAT_MINUTES`] with hands on it, and the trauma over when
    /// the hands come off (`apply_treatments`). Refused for the same
    /// reasons a bandage is, for a helper that is the patient, no medkit,
    /// or a part with no trauma on it.
    pub fn treat(&mut self, who: usize, patient: usize, part: Part) -> bool {
        if who >= self.bims.len()
            || patient >= self.bims.len()
            || who == patient
            || !self.bims[who].is_alive()
            || !self.bims[patient].is_alive()
            || self.bims[who].character.is_unconscious()
            || self.bims[who].character.is_outside()
            || self.bims[patient].character.is_outside()
            || self.bims[patient].health.trauma(part).is_none()
        {
            return false;
        }
        // A kit in the helper's own pack is a kit: it is opened where the
        // helper stands and no shelf is walked to. Only with none there
        // and none on a shelf is the treatment bare-handed — a medic the
        // world lets do it (`Doctoring::bare`, its *field surgery*) — and
        // anybody else is refused.
        let bare = self.room.medkits == 0 && !self.room.carries_kit(who);
        if bare && self.doctoring_of(who).bare.is_none() {
            return false;
        }
        let kind = Kind::Treat {
            patient,
            part: part.code(),
            bare,
        };
        if !self.take_over(who, kind, task::TREAT_MINUTES) {
            return false;
        }
        self.tell_the_room_where_the_crew_are();
        let taken = self.taken_for(who);
        self.bims[who].task = Some(Task::treat(
            who,
            patient,
            part.code(),
            bare,
            &mut self.bims[who].character,
            &mut self.room,
            &self.maps,
            &taken,
        ));
        true
    }

    /// Every treatment the chains finished this step, done: the trauma on
    /// the part over (`Health::treat`), a medkit off the count, and the
    /// world told. Under the same conditions as a dressing — the helper
    /// within two tiles of the patient, a medkit still to hand, the
    /// patient alive.
    fn apply_treatments(&mut self) {
        for (helper, patient, part, bare) in core::mem::take(&mut self.room.treated) {
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
            // The kit was the one in the helper's hands, fetched by the chain
            // and spent as the hands came off (`Step::Dress`); the room says
            // so only when there was one.
            if apart > 2.0 * TILE {
                continue;
            }
            // A medic's hands (feature 76): where the part starts again
            // from, and whether anything lasting is left.
            let doctoring = self.doctoring_of(helper);
            if let Some(trauma) = self.bims[patient].health.treat_as(
                part,
                doctoring.clean_hands,
                doctoring.treated_to,
            ) {
                self.treated.push((patient, trauma));
                self.healings.push(Healed {
                    helper,
                    patient,
                    with: if bare { Healing::Bare } else { Healing::Medkit },
                });
            }
        }
    }

    /// What a Bim's doctoring runs at, as the world last said (feature
    /// 76): `Doctoring::NONE` for anybody it did not name.
    fn doctoring_of(&self, who: usize) -> Doctoring {
        self.doctoring.get(who).copied().unwrap_or(Doctoring::NONE)
    }

    /// What each Bim's doctoring runs at, by index (feature 76): a
    /// medic's talents on its bandaging, its treating, and whether it
    /// may treat with no kit. `Doctoring::NONE` for anybody not named.
    pub fn set_doctoring(&mut self, doctoring: Vec<Doctoring>) {
        self.doctoring = doctoring;
    }

    /// What a medic's beam does to each body this step, by index
    /// (feature 76): `None` for a body no beam holds. Applied in the
    /// body's health tick and nowhere else.
    pub fn set_held(&mut self, held: Vec<Option<Beamed>>) {
        self.held = held;
    }

    /// Whether a beam holds a body this step.
    pub fn is_held(&self, who: usize) -> bool {
        self.held.get(who).copied().flatten().is_some()
    }

    /// Medkits on the shelf, like the bandages: the hold's aboard, set by
    /// the world every step; the classic room's few otherwise. One in a
    /// helper's hands is not on the shelf.
    pub fn medkits(&self) -> u32 {
        self.room.medkits
    }

    /// The hold's count, less every kit a helper is carrying — those are
    /// the hold's still, until the treatment is done and `take_medkits_used`
    /// says so, but they are not on the shelf for a second helper to set
    /// out for.
    pub fn set_medkits(&mut self, n: u32) {
        let in_hand = self
            .bims
            .iter()
            .filter(|b| b.character.main_held() == Held::Medkit)
            .count() as u32;
        self.room.medkits = n.saturating_sub(in_hand);
    }

    /// Where the kits are fetched from: the use spots of the containers
    /// that hold one, the world's word every step. None — the classic
    /// room, a station's — and a kit is taken where the helper stands.
    pub fn set_kit_stands(&mut self, stands: &[Vec2]) {
        if self.room.kit_stands != stands {
            self.room.kit_stands = stands.to_vec();
        }
    }

    /// Medkits used up since the last call, for the world to take off the
    /// hold.
    pub fn take_medkits_used(&mut self) -> u32 {
        core::mem::take(&mut self.room.medkits_used)
    }

    /// How many medkits each Bim carries in its **own pack**, by index —
    /// its medkit charges, the world's word every step. A helper that
    /// carries one treats with it where it stands rather than walking to
    /// a cabinet: its own kit before a new one. A kit already in that
    /// Bim's hands is not counted again — the world takes it out of the
    /// pack only when the treatment is done, so until then the pack still
    /// holds the one being carried to the patient.
    pub fn set_pack_kits(&mut self, mut kits: Vec<u32>) {
        for (who, n) in kits.iter_mut().enumerate() {
            if self
                .bims
                .get(who)
                .is_some_and(|b| b.character.main_held() == Held::Medkit)
            {
                *n = n.saturating_sub(1);
            }
        }
        self.room.pack_kits = kits;
    }

    /// Every helper that opened a kit out of its own pack since the last
    /// call. The world drains it and does nothing more with it: a kit
    /// stays in its pack until the treatment is done, and the world
    /// takes it out then, off the finished treatment (`take_healings`).
    pub fn take_pack_kits_used(&mut self) -> Vec<usize> {
        core::mem::take(&mut self.room.pack_kits_used)
    }

    // --- a weapon on the deck ------------------------------------------------

    /// The weapons lying on the deck, dropped by bodies knocked out.
    pub fn weapons_down(&self) -> &[Dropped] {
        &self.room.weapons_down
    }

    /// The dropped weapon the last click landed on, by id, after
    /// [`Game::hit_at`] said `HIT_DROPPED`.
    pub fn hit_dropped(&self) -> u32 {
        self.hit_dropped
    }

    /// The dropped weapon under a room point, by id: within `PICK_RADIUS`
    /// of where it lies, the same reach as a click. Nothing is noted; this
    /// is the hover's question, asked every frame.
    pub fn dropped_at(&self, x: f32, y: f32) -> Option<u32> {
        let p = vec2(x, y);
        self.room
            .weapons_down
            .iter()
            .find(|d| (d.at - p).len() <= PICK_RADIUS)
            .map(|d| d.id)
    }

    /// Ring the dropped weapon with this id on the deck — the one the
    /// pointer rests on — or none. Worked out afresh every frame by the
    /// host, like the highlight, so nothing stays lit.
    pub fn set_hover_dropped(&mut self, id: Option<u32>) {
        self.hover_dropped = id;
    }

    /// Send `who` to pick the dropped weapon `item` up: the walk over and a
    /// moment bending for it, and it is in the hand if the hand is empty,
    /// Send `who` to finish off the other room's `visitor` lying on this
    /// deck: the walk to within reach — `EXECUTE_RANGE` with a clear line
    /// for a gun, beside it for a blade — and `EXECUTE_SECONDS` shooting or
    /// hacking at it, then `Room::executed` for the world to kill it in its
    /// own room (`take_executed`). The player's order from the menu on a
    /// downed enemy; whether it *is* an enemy is the world's check. Refused
    /// for a Bim that cannot — dead, out cold, outside, nothing in its
    /// hand — or a visitor that is not down.
    pub fn execute(&mut self, who: usize, visitor: usize) -> bool {
        if who >= self.bims.len()
            || !self.bims[who].is_alive()
            || self.bims[who].character.is_unconscious()
            || self.bims[who].character.is_outside()
            || !self.visitor_down(visitor)
        {
            return false;
        }
        let Some(weapon) = self.bims[who].gear.weapon else {
            return false;
        };
        let kind = Kind::Execute {
            visitor,
            blade: weapon.stats().melee,
        };
        if !self.take_over(who, kind, 0.0) {
            return false;
        }
        self.tell_the_room_where_the_crew_are();
        let taken = self.taken_for(who);
        self.bims[who].task = Some(Task::execute(
            who,
            visitor,
            weapon.stats().melee,
            &mut self.bims[who].character,
            &mut self.room,
            &self.maps,
            &taken,
        ));
        true
    }

    /// Every execution finished since last asked — `(who, visitor)` — for
    /// the world to carry to the body's own room (`execute_body`).
    #[allow(dead_code)]
    pub fn bodies_down_for_probe(&self) -> Vec<Option<Vec2>> {
        self.room.bodies_down.clone()
    }

    pub fn take_executed(&mut self) -> Vec<(usize, usize)> {
        core::mem::take(&mut self.room.executed)
    }

    /// One of this room's own finished off where it lay, in the other
    /// room: dead outright — the head and the body to nothing with no
    /// trauma to keep it dying, which is `Health::give_up` — if it was
    /// still down. Whether it was.
    pub fn execute_body(&mut self, who: usize) -> bool {
        // A machine is never down-and-alive: it is up or it is a wreck,
        // so there is nothing to finish off.
        if self.droid_at(who).is_some() {
            return false;
        }
        if !self.is_down(who) || !self.is_alive(who) {
            return false;
        }
        self.bims[who].health.give_up();
        true
    }

    /// Whether [`Game::fetch`] would start: the Bim alive, awake and in,
    /// and the weapon still lying there. What the screen asks before it
    /// sends the order, so a refusal can be said at the click.
    pub fn can_fetch(&self, who: usize, item: u32) -> bool {
        who < self.bims.len()
            && self.bims[who].is_alive()
            && !self.bims[who].character.is_unconscious()
            && !self.bims[who].character.is_outside()
            && self.room.weapons_down.iter().any(|d| d.id == item)
    }

    /// else in the pack (`apply_pickups`). The player's order from the menu
    /// on it, and what a bot does for its own gun the moment it comes round
    /// (`fetch_own_weapon`). Refused for a Bim that cannot — dead, out
    /// cold, outside — or a weapon that is not there any more.
    pub fn fetch(&mut self, who: usize, item: u32) -> bool {
        if !self.can_fetch(who, item) {
            return false;
        }
        let kind = Kind::Fetch { item };
        if !self.take_over(who, kind, 0.0) {
            return false;
        }
        let taken = self.taken_for(who);
        self.bims[who].task = Some(Task::fetch(
            who,
            item,
            &mut self.bims[who].character,
            &mut self.room,
            &self.maps,
            &taken,
        ));
        true
    }

    /// Every pick-up the chains finished this step, done — as long as the
    /// Bim is still within two tiles of it. The player's own Bim puts the
    /// weapon in its **pack** (the hand if the pack is full), since a
    /// right-click on a gun is "into the inventory" and what goes in the
    /// hand is the player's to choose; a bot — a crewmate, or one of a
    /// hostile room's — puts it in its hand if that is empty, else the
    /// pack, since it came for its own gun to fight with. Left where it
    /// lies when neither has room.
    fn apply_pickups(&mut self) {
        for (who, item) in core::mem::take(&mut self.room.picked_up) {
            if who >= self.bims.len() || !self.bims[who].is_alive() {
                continue;
            }
            let Some(i) = self.room.weapons_down.iter().position(|d| d.id == item) else {
                continue;
            };
            let dropped = self.room.weapons_down[i];
            if (self.bims[who].character.pos - dropped.at).len() > 2.0 * TILE {
                continue;
            }
            let bot = self.is_bot(who);
            let gear = &mut self.bims[who].gear;
            let hand_first = bot && gear.weapon.is_none();
            if hand_first {
                gear.weapon = Some(dropped.weapon);
            } else if let Some(cell) = gear.free_cell() {
                gear.pack[cell] = Some(Item::Weapon(dropped.weapon));
            } else if gear.weapon.is_none() {
                gear.weapon = Some(dropped.weapon);
            } else {
                continue;
            }
            self.room.weapons_down.remove(i);
        }
    }

    /// The weapon in the hand of a body going out cold, let go of where
    /// it lies: onto the deck, numbered, for whoever comes for it.
    fn drop_weapon(&mut self, who: usize) {
        let bim = &mut self.bims[who];
        let Some(weapon) = bim.gear.weapon.take() else {
            return;
        };
        bim.character.set_armed(None);
        let id = self.room.next_weapon_down;
        self.room.next_weapon_down += 1;
        // Dropped out of the hand as it falls, so it lies beside the
        // fallen figure and not under it.
        let at = bim.character.pos + DROP_FLUNG.rotate(bim.character.heading);
        self.room.weapons_down.push(Dropped {
            id,
            at,
            weapon,
            owner: who,
        });
    }

    /// A bot back on its feet with nothing in its hand goes for the gun it
    /// dropped, if it still lies there and there is a way to it. Every Bim
    /// but the player's own in the crew's room, and every one of a
    /// hostile room's; not one already on its way, not one dying with an
    /// enemy about — it is running — and not while it is under arms at a
    /// post the player gave it.
    fn fetch_own_weapon(&mut self, who: usize) {
        let bot = self.is_bot(who);
        let bim = &self.bims[who];
        if !bot
            || bim.gear.weapon.is_some()
            || bim
                .task
                .as_ref()
                .is_some_and(|t| matches!(t.kind(), Kind::Fetch { .. }))
            || self.is_fleeing(who)
        {
            return;
        }
        let from = bim.character.pos;
        let Some(item) = self
            .room
            .weapons_down
            .iter()
            .filter(|d| d.owner == who)
            .find(|d| task::dropped_stand(&self.room, &self.maps, d.id, from).is_some())
            .map(|d| d.id)
        else {
            return;
        };
        self.fetch(who, item);
    }

    // --- running from a fight ------------------------------------------------

    /// Whether `who` is running from the fight: **dying** — a part at
    /// nothing with its trauma untreated, `Health::dying`, and nothing
    /// short of it: a wound bleeding or blood run low is fought on
    /// through — on its feet on the deck, and an enemy up somewhere. It
    /// goes nowhere else while it is — no errand, no post, no order — and
    /// does not shoot. Whoever it is: the player's own, a crew member
    /// nobody steers, an enemy's people. Not while a crewmate is nearly
    /// at it with a bandage or a kit (`helper_near`): it holds still for
    /// them the way any patient does, or nobody could ever catch it to
    /// dress it.
    pub fn is_fleeing(&self, who: usize) -> bool {
        // A commander's aura buys it seconds before it goes (feature
        // 78): `fear` is how long it has been in this state, counted in
        // `tick_combat`, and it stands its ground until the hold is up.
        self.would_flee(who) && self.bims[who].fear >= self.skill(who).nerve_hold
    }

    /// Whether everything but the hold says it runs: what `fear` counts
    /// up under, and what [`Game::is_fleeing`] is once the hold is up.
    fn would_flee(&self, who: usize) -> bool {
        let bim = &self.bims[who];
        // A braced soldier, or one with *iron nerve*, never runs (feature
        // 75).
        !bim.braced
            && !self.skill(who).nerve
            && bim.is_alive()
            && bim.health.dying()
            && !bim.character.is_outside()
            && !bim.character.is_unconscious()
            && self.combat.targets().iter().any(|t| t.is_some())
            && !self.helper_near(who)
            // The last stand (feature 84): dying aboard the ship with the
            // enemy aboard it too, there is nowhere left to run to, and a
            // body that went on running would be shot in the back walking
            // deeper into its own hull. It fights where it stands.
            && !self.cornered(who)
    }

    /// Whether a crewmate on its way to doctor `who` is within
    /// [`HELPER_NEAR`] tiles of it, or has its hands on it already.
    fn helper_near(&self, who: usize) -> bool {
        let at = self.bims[who].character.pos;
        self.bims.iter().enumerate().any(|(other, b)| {
            other != who
                && b.task
                    .as_ref()
                    .is_some_and(|t| t.kind().patient() == Some(who))
                && (b.task.as_ref().is_some_and(|t| t.is_dressing())
                    || (b.character.pos - at).len() <= HELPER_NEAR * TILE)
        })
    }

    /// One dying body's run, when its clock comes round: away from where
    /// the enemy are on average (`Tactics::flee`), whatever it was doing
    /// put down first. Its own `plan_wait` clock, like a stand.
    ///
    /// **The crew run for the ship** (feature 84): a dying crew member
    /// makes for the deck just inside its own port rather than merely
    /// for the far side of the station, since that is where the medkits,
    /// the shut airlock and whoever is left are — and since a crew that
    /// scatters under fire is a crew nobody can doctor. Only where there
    /// is a way there; a body with the enemy between it and the port
    /// runs the way it always did. An enemy's people are not the crew
    /// and have no ship: theirs is `Tactics::flee` throughout.
    fn flee(&mut self, who: usize, dt: f32) {
        let bim = &mut self.bims[who];
        bim.plan_wait -= dt;
        if bim.plan_wait > 0.0 {
            return;
        }
        bim.plan_wait = PLAN_EVERY;
        let from = bim.character.pos;
        let nav = self.maps.for_body(false, self.room.bath.is_open());
        let targets = self.combat.targets().to_vec();
        let home = (!self.hostile_bodies && !self.is_aboard(from)).then(|| {
            let anchor = nav.nearest_free(self.ship_anchor());
            (nav.can_reach(from, anchor)).then_some(anchor)
        });
        let Some(to) = home
            .flatten()
            .or_else(|| Tactics::flee(nav, from, &targets))
        else {
            return;
        };
        let going = self.bims[who].character.destination().unwrap_or(from);
        if (to - going).len() <= TILE {
            return;
        }
        let route = nav.path(from, to);
        if route.is_empty() {
            return;
        }
        if self.bims[who].task.is_some() {
            self.interrupt(who);
        }
        // An enemy's run through a door is a door to lock behind it: the
        // first door the route passes, and which side it set out from.
        if self.hostile_bodies {
            self.bims[who].seal = self
                .room
                .doors
                .iter()
                .enumerate()
                .filter(|(_, d)| !d.locked)
                .find(|(_, d)| route.iter().any(|&p| d.rect.expand(TILE * 0.6).contains(p)))
                .map(|(i, d)| (i, (from - d.rect.center()).dot(d.through())));
        }
        self.bims[who].character.follow_path(route);
    }

    /// A dying crew member's run is a **fighting withdrawal**: it goes
    /// where [`Game::flee`] sends it, but with the gun up, and shoots
    /// what it sees on the way. A crew that holstered as it ran was a
    /// crew member walking away from an enemy in plain view without a
    /// shot, which is what the fight looked like from the player's
    /// chair. The walk is a **backing** one, the fall back's
    /// ([`FallBack::Backwards`]): the facing on the target and the feet
    /// on the route, at the fall back's slower pace; standing, it faces
    /// the target square. From its own eyes only — a body on the run
    /// leans round nothing — and at the walking odds while it moves.
    fn shoot_on_the_run(&mut self, who: usize, dt: f32, weapon: Weapon, skill: &Skill) {
        let stats = skill.stats(weapon);
        let from = self.bims[who].character.pos;
        let shot = self
            .combat
            .aim(&self.room.sight, from, &stats)
            .filter(|&(_, eye, _)| eye == from);
        let bim = &mut self.bims[who];
        let Some((_, eye, at)) = shot else {
            bim.trigger.hold();
            return;
        };
        let walking = bim.character.is_walking();
        if walking {
            bim.character
                .set_falling_back(Some(FallBack::Backwards((at - eye).angle())));
        } else {
            bim.character.face((at - eye).angle());
        }
        bim.character.set_aim(Some(at));
        if bim.trigger.pull(dt, &stats) {
            let muzzle = self.shot_from(who, eye);
            self.combat
                .fire_as(muzzle, at, weapon, false, walking, skill, Some(who));
        }
    }

    /// What a Bim has on it.
    pub fn gear(&self, who: usize) -> Gear {
        self.bims[who].gear
    }

    /// What a Bim looks like — the yoke, the hair, the build
    /// (`character::Look`).
    pub fn look(&self, who: usize) -> Look {
        self.bims[who].character.look()
    }

    /// Give a Bim another look: the hair a player chose at the start
    /// (feature 62). Drawing only, so nothing the world checks moves.
    pub fn set_look(&mut self, who: usize, look: Look) {
        if who < self.bims.len() {
            self.bims[who].character.set_look(look);
        }
    }

    /// The colour each player's own Bim is ringed in, in slot order
    /// (feature 84, `character::Tint`): slot *i*'s Bim takes the *i*th,
    /// and everybody past the players — the bots, the hires — is left
    /// with none, which is what draws no circle at all. Drawing only,
    /// like the hair, so nothing the world checks moves; said again
    /// whenever a choice arrives or the crew change.
    pub fn set_tints(&mut self, tints: &[Tint]) {
        for who in 0..self.bims.len() {
            let tint = (who < self.players as usize)
                .then(|| tints.get(who).copied())
                .flatten();
            self.bims[who].character.set_tint(tint);
        }
    }

    /// The colour a Bim is ringed in, for the tests and the app.
    pub fn tint(&self, who: usize) -> Option<Tint> {
        self.bims.get(who).and_then(|b| b.character.tint())
    }

    /// What a Bim's class wears (feature 81). Drawing only, and the
    /// world's to say: it hands the room one an entry every step off its
    /// own classes, so nothing here decides it and nothing is saved.
    pub fn set_outfit(&mut self, who: usize, outfit: Outfit) {
        if who < self.bims.len() {
            self.bims[who].character.set_outfit(outfit);
        }
    }

    pub fn outfit(&self, who: usize) -> Outfit {
        self.bims[who].character.outfit()
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

    /// Put an item in a Bim's pack: its corner in `cell`, turned if only
    /// that fits, or in the first place the whole of it fits. The world's
    /// half of a fetch — the piece has already left the hold. `false`, and
    /// the item is not taken, when it would not lie there or nowhere
    /// fits.
    pub fn give(&mut self, who: usize, cell: Option<usize>, item: Item) -> bool {
        let Some(bim) = self.bims.get_mut(who) else {
            return false;
        };
        // A stackable thing joins a box that has room before it asks for
        // a cell of its own (feature 87): five dressings go where one
        // does.
        let cell = cell
            .or_else(|| bim.gear.stack_with_room(item))
            .or_else(|| bim.gear.free_cell_for(item));
        let Some(cell) = cell else {
            return false;
        };
        bim.gear.put(cell, item)
    }

    /// Take up to `n` of a stackable thing out of a Bim's pack, the
    /// emptiest box first (feature 87); how many actually came out.
    /// The other half of [`Game::give_stack`].
    pub fn take_stack(&mut self, who: usize, item: Item, n: u32) -> u32 {
        let mut gone = 0;
        while gone < n {
            let Some(bim) = self.bims.get_mut(who) else {
                break;
            };
            let Some(cell) = bim.gear.stack_to_spend(item) else {
                break;
            };
            if bim.gear.take_one(cell).is_none() {
                break;
            }
            gone += 1;
        }
        gone
    }

    /// Move a thing across a Bim's pack: the one kept in `cell` — or
    /// reaching over it — to the cell `to`, turned or not. A drag in the
    /// inventory. `false`, and nothing moved, when it would not lie there.
    pub fn rearrange(&mut self, who: usize, cell: usize, to: usize, turned: bool) -> bool {
        self.bims
            .get_mut(who)
            .is_some_and(|bim| bim.gear.rearrange(cell, to, turned))
    }

    /// Take an item out of a Bim's pack, for the hold or a shelf: the
    /// other half of a stow. A **broken** piece is refused and left where
    /// it is — the hold counts pieces as resources and a broken one is
    /// worth nothing there; it can only be [`Game::discard`]ed.
    pub fn take(&mut self, who: usize, cell: usize) -> Option<Item> {
        let gear = &mut self.bims.get_mut(who)?.gear;
        // A thing is taken by any of its cells.
        let cell = gear.head_of(cell);
        if let Some(Item::Armour(piece)) = gear.pack.get(cell)?
            && piece.broken()
        {
            return None;
        }
        gear.take_out(cell)
    }

    /// Throw an item away for good. What the pop-up offers for a broken
    /// piece. `false` when the cell was empty.
    pub fn discard(&mut self, who: usize, cell: usize) -> bool {
        let Some(gear) = self.bims.get_mut(who).map(|b| &mut b.gear) else {
            return false;
        };
        gear.take_out(cell).is_some()
    }

    /// Put on what is in a pack cell: a piece goes on the part it is cut
    /// for and whatever was worn there comes back into the pack — into
    /// the cells the piece left if it fits there, else the first place it
    /// does; a weapon swaps with the weapon slot the same way. Refused
    /// for a stack, an empty cell, a Bim that is dead, or a pack with no
    /// room for what would come off, in which case nothing moves.
    /// Returns what came off, if anything, for the caller's information
    /// — nothing both for a refusal and for an empty slot.
    pub fn equip(&mut self, who: usize, cell: usize) -> Option<Item> {
        if !self.bims.get(who).is_some_and(|b| b.is_alive()) {
            return None;
        }
        let gear = &mut self.bims[who].gear;
        let cell = gear.head_of(cell);
        let item = (*gear.pack.get(cell)?)?;
        let worn = match item {
            Item::Armour(piece) => gear.worn(piece.kind.slot()).map(Item::Armour),
            Item::Weapon(_) => gear.weapon.map(Item::Weapon),
            Item::Stack(_) | Item::Key(_) => return None,
        };
        // The item out first, so what comes off can take its cells.
        gear.take_out(cell);
        if let Some(off) = worn
            && !gear.put(cell, off)
            && !gear.free_cell_for(off).is_some_and(|c| gear.put(c, off))
        {
            gear.put(cell, item);
            return None;
        }
        match item {
            Item::Armour(piece) => *gear.worn_mut(piece.kind.slot()) = Some(piece),
            Item::Weapon(weapon) => gear.weapon = Some(weapon),
            Item::Stack(_) | Item::Key(_) => {}
        }
        self.refresh_worn(who);
        worn
    }

    /// Take off what is worn on a part, into the first place in the pack
    /// it fits. `false` with nothing there, no room for it, or a Bim that
    /// is dead.
    pub fn unequip(&mut self, who: usize, part: Part) -> bool {
        if !self.bims.get(who).is_some_and(|b| b.is_alive()) {
            return false;
        }
        let gear = &mut self.bims[who].gear;
        let Some(piece) = gear.worn(part) else {
            return false;
        };
        let Some(cell) = gear.free_cell_for(Item::Armour(piece)) else {
            return false;
        };
        if !gear.put(cell, Item::Armour(piece)) {
            return false;
        }
        *gear.worn_mut(part) = None;
        self.refresh_worn(who);
        true
    }

    // --- looting a body -------------------------------------------------------

    /// Down: dead, or out cold. What a body has to be to be looted, and
    /// what the world hands the other room as `set_visitors_down`.
    pub fn is_down(&self, who: usize) -> bool {
        if let Some(i) = self.droid_at(who) {
            // A wreck is down for everything that asks — and, unlike a
            // body, there is nothing on it to take.
            return self.droids[i].destroyed;
        }
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
        // A machine carries nothing: no pack, no armour, and an arm that
        // is part of it (feature 83). Every cell empty, and
        // `take_from_body` refuses it besides, so the Loot window never
        // opens on a wreck.
        if self.droid_at(who).is_some() {
            return cells;
        }
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
    /// How many are in one of a body's loot cells (feature 87): the
    /// stack in a pack cell, one for a worn piece or the weapon, nought
    /// for an empty cell or a body nothing comes off. Asked **before**
    /// [`Game::take_from_body`], which takes the whole stack.
    pub fn body_units(&self, who: usize, cell: LootCell) -> u32 {
        if self.droid_at(who).is_some() || !self.is_down(who) {
            return 0;
        }
        let gear = &self.bims[who].gear;
        match cell {
            LootCell::Pack(cell) => gear.units(cell as usize),
            LootCell::Head => u32::from(gear.head.is_some()),
            LootCell::Body => u32::from(gear.body.is_some()),
            LootCell::Legs => u32::from(gear.legs.is_some()),
            LootCell::Weapon => u32::from(gear.weapon.is_some()),
        }
    }

    /// How many are in each of a body's loot cells, for the window's
    /// numbers — [`Game::loot_cells`]'s counts, index for index.
    pub fn loot_counts(&self, who: usize) -> [u32; LOOT_CELLS] {
        let mut counts = [0; LOOT_CELLS];
        for (i, count) in counts.iter_mut().enumerate() {
            if let Some(cell) = LootCell::from_code(i as u32) {
                *count = self.body_units(who, cell);
            }
        }
        counts
    }

    pub fn take_from_body(&mut self, who: usize, cell: LootCell) -> Option<Item> {
        // Nothing comes off a machine, wreck or not (feature 83).
        if self.droid_at(who).is_some() {
            return None;
        }
        if !self.is_down(who) {
            return None;
        }
        let gear = &mut self.bims[who].gear;
        let taken = match cell {
            LootCell::Pack(cell) => gear.take_out(cell as usize),
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
            Container::Desk(i) => self.room.research.get(i).map(|d| d.1),
        }
    }

    /// A container's footprint, for the reach check: whether a Bim is
    /// within so many tiles of it.
    pub fn container_frame(&self, container: Container) -> Option<Rect> {
        match container {
            Container::Bench(i) => self.room.benches.get(i).map(|b| b.frame),
            Container::Shelf(i) => self.room.shelves.get(i).map(|s| s.0),
            Container::Fridge(i) => self.room.fridges.get(i).map(|f| f.frame),
            Container::Desk(i) => self.room.research.get(i).map(|d| d.0),
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
    /// Give `who`'s errand up for good, for the tests: what a chain that
    /// finds nowhere to go does, banking whatever was in the hands.
    #[allow(dead_code)]
    pub fn abandon_for_probe(&mut self, who: usize) {
        if let Some(task) = self.bims[who].task.take() {
            task.abandon(&mut self.bims[who].character, &mut self.room);
        }
    }

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
        self.observe_from(&eyes);
    }

    /// The same trace from these eyes instead of the crew's, for a probe
    /// that wants to see what is remembered: none at all is a deck nobody
    /// is looking at. The next `observe` puts the crew's back.
    #[allow(dead_code)]
    pub fn observe_from_for_probe(&mut self, eyes: &[Vec2]) {
        self.observe_from(eyes);
    }

    fn observe_from(&mut self, eyes: &[Vec2]) {
        let shut = self.shut_now();
        self.room.sight.observe(eyes, &shut);
        // And the plain beyond the box, from the same eyes: what stops a
        // line on the deck is the mask's cells, doors and all.
        if let Some(plane) = self.room.plane.as_mut() {
            let sight = &self.room.sight;
            plane.observe(eyes, TILE, &|x, y| sight.opaque_room_tile(x, y));
        }
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
            .chain(self.droids.iter().filter(|d| !d.destroyed).map(|d| d.pos))
            .chain(self.visitors.iter().copied())
            .collect();
        let mut shut = self.room.shut_leaves(&bodies);
        if let Some(door) = self.room.closed_door() {
            shut.push(door);
        }
        shut
    }

    /// The smooth picture of what the crew see and what is lit, for the
    /// host to draw over the room — `sight::LightMap`, worked out in
    /// `render`. `None` for a room not drawn through the crew's eyes.
    pub fn light_map(&self) -> Option<&crate::sight::LightMap> {
        (self.fog == Fog::Crew && self.room.sight.map().width > 0).then(|| self.room.sight.map())
    }

    /// The same picture of the plain beyond the box, on a planet — a
    /// picture a chunk, `terrain::Plane::picture` — from the crew's
    /// eyes, for the room tiles `window` covers (both ends in): what a
    /// host is about to draw, since the plain is too big to picture
    /// whole. Asked once a frame after `render`, by the host that knows
    /// its camera; nothing anywhere but a plain through the crew's
    /// eyes. The pictures are `plain_pictures`.
    pub fn picture_plain(&mut self, window: (i32, i32, i32, i32)) {
        if self.fog != Fog::Crew {
            return;
        }
        let eyes: Vec<Vec2> = self.bims.iter().map(|b| b.character.pos).collect();
        let sight = &self.room.sight;
        if let Some(plane) = self.room.plane.as_mut() {
            plane.picture(
                &eyes,
                TILE,
                &|x, y| sight.opaque_room_tile(x, y),
                sight.cells_version(),
                window,
            );
        }
    }

    /// The plain's pictures as last composed, by the room's chunk —
    /// `terrain::Plane::pictures`. Empty off a planet.
    pub fn plain_pictures(&self) -> Vec<((i32, i32), &crate::sight::LightMap)> {
        match (&self.room.plane, self.fog) {
            (Some(plane), Fog::Crew) => plane.pictures().collect(),
            _ => Vec::new(),
        }
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
    /// the same sit-down and clearing-up as a meal that was cooked. Any hob's
    /// pot: the chain picks the one with something in it.
    pub fn eat_leftovers(&mut self, who: usize) -> bool {
        if self.room.hobs.iter().all(|h| h.pot_servings == 0)
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
    /// already busy, like every other errand. A Bim with no bunk lies down
    /// on the deck instead, for what is left of the deck's allowance
    /// (`can_sleep_on_ground`) — and not at all with none left.
    pub fn rest(&mut self, who: usize, minutes: f32) -> bool {
        let minutes = if self.room.bed_of(who).is_some() {
            minutes
        } else if self.can_sleep_on_ground(who) {
            minutes.min(self.bims[who].ground_left)
        } else {
            return false;
        };
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
        // And the scorches the bolts left on the walls and the deck
        // (feature 98), with the mess.
        self.combat.fx.draw_ground(&mut self.list);

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

        // The walks waiting their turn — the Shift-clicks on the deck —
        // for the viewer's own crew member and whoever it has selected: a
        // dashed thread from where the Bim is bound now through each spot
        // in turn, a pip at every one, so what was queued can be read off
        // the deck until it is walked. In the ping's colour, since a
        // queued walk is a ping that stays.
        for (who, bim) in self.bims.iter().enumerate() {
            if who != self.viewer as usize && !bim.character.is_selected_by(self.viewer) {
                continue;
            }
            let mut from = bim.character.destination().unwrap_or(bim.character.pos);
            for saved in &bim.queue {
                let (Kind::Walk { .. }, Some(to)) = (saved.kind(), saved.target()) else {
                    continue;
                };
                dashed(&mut self.list, from, to, ACCENT.alpha(0.65));
                self.list.circle(to, 6.0, ACCENT.alpha(0.9));
                self.list.ring(to, 16.0, 2.0, ACCENT.alpha(0.8));
                from = to;
            }
        }

        // What lies on the deck under the bodies: the guns dropped by
        // whoever went out cold, where they fell — the one under the
        // pointer ringed in the highlight's cyan, so it reads as something
        // a right-click picks up.
        self.bodies_from = self.list.len();
        for d in &self.room.weapons_down {
            if self.hover_dropped == Some(d.id) {
                let size = vec2(PICK_RADIUS, PICK_RADIUS) * 2.0;
                self.list
                    .rect(d.at, size, 0.0, PICK_RADIUS, GLOW.alpha(0.16));
                self.list
                    .stroke_rect(d.at, size, 0.0, PICK_RADIUS, 2.0, GLOW.alpha(0.95));
            }
            crate::character::draw_dropped(&mut self.list, d.at, d.weapon.kind);
        }

        // Bodies in crew order, so who is drawn on top of whom does not
        // change as they walk past each other — and only the ones in view:
        // a station's people behind a bulkhead are not drawn at all.
        let fx = &self.combat.fx;
        for (who, bim) in self.bims.iter().enumerate() {
            if self.body_seen(who) {
                bim.character.draw(&mut self.list, self.viewer);
                // A shot that landed: a flash on the part it struck, on the
                // host's clock (feature 98) — or, for a host that ages no
                // effects, the room's own flash over the whole body, gone
                // in a blink of the simulation's.
                if fx.is_on() {
                    for (part, t) in fx.struck_on(who) {
                        if let Some(part) = Part::from_code(part) {
                            let (at, radius) = bim.character.part_mark(part);
                            crate::fx::draw_struck(&mut self.list, at, radius, t);
                        }
                    }
                } else if bim.hit_flash > 0.0 {
                    let t = bim.hit_flash / HIT_FLASH;
                    let at = bim.character.drawn_at();
                    self.list
                        .circle(at, 42.0 + 30.0 * (1.0 - t), HIT.alpha(0.45 * t));
                    self.list.ring(at, 50.0, 3.0, HIT.alpha(0.9 * t));
                }
            }
        }
        // And the machines after them (feature 83), in the body order the
        // world knows them by — a droid's index is `bims.len() + i`, so
        // `body_seen` is asked with that. A wreck is drawn like anything
        // else that is down: it lies where it fell.
        for i in 0..self.droids.len() {
            let body = self.bims.len() + i;
            if self.body_seen(body) {
                self.droids[i].draw(&mut self.list);
                for (part, t) in fx.struck_on(body) {
                    if let Some(part) = DroidPart::from_code(part) {
                        let (at, radius) = self.droids[i].part_mark(part);
                        crate::fx::draw_struck(&mut self.list, at, radius, t);
                    }
                }
            }
        }
        // The plates a machine threw bursting apart (feature 98), landing
        // among the bodies.
        fx.draw_debris(&mut self.list);
        // Bedding and bunk rails go over the Bim, so getting into bed puts it
        // under the covers rather than on top of them.
        self.room.draw_over(&mut self.list);

        // What nobody sees, fogged. Over the deck and the fixtures and
        // under the night, the rings and the marquee: a fog that hid the
        // pointer's own marks would be a fog over the pointer.
        self.observe();
        match self.fog {
            // Through the crew's eyes the fog is the light map's — smooth,
            // the crew's own semi and a stranger's grey and black alike,
            // drawn by the host over this picture — marched again only
            // for a body that moved.
            Fog::Crew => {
                let eyes: Vec<Vec2> = self.bims.iter().map(|b| b.character.pos).collect();
                self.room.sight.light_map(&eyes);
            }
            Fog::All => self.room.sight.draw(&mut self.list, true),
            Fog::None => {}
        }
        // The shots, over the fog: a bolt is always seen, whatever it
        // flies through.
        self.fog_from = self.list.len();
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
        // And the line a right-drag is drawing: where each of the selected
        // will stand along it, a pip apiece, so the order can be read
        // before it is given.
        if let Some((from, to)) = self.order_drag {
            let n = self.orderable(self.viewer).len();
            if n > 0 && (to - from).len() > 1.0 {
                self.list.line(from, to, 1.5, MARQUEE_EDGE.alpha(0.8));
                let pips = n.max(2);
                for i in 0..n.max(1) {
                    let at = from + (to - from) * (i as f32 / (pips - 1) as f32);
                    self.list.circle(at, 10.0, MARQUEE_EDGE.alpha(0.9));
                }
            }
        }
    }

    /// The frame's shapes, for a host that embeds this room in a picture of
    /// its own rather than replaying the buffer straight to a canvas — the
    /// ship game paints the room turned with the ship.
    pub fn shapes(&self) -> &[f32] {
        self.list.data()
    }

    /// The same frame in two pieces: the deck — the room's floor and
    /// fixtures, the mess, the trails and the pings — and the bodies with
    /// what goes over them: the dropped weapons, the Bims with their hit
    /// flashes, the bedding, the fog, the shots. For a host that lays
    /// another picture over this room's deck and wants whoever has walked
    /// onto it drawn over that rather than under: the ship painter, whose
    /// hull and room aboard go over a docked station's, and whose people
    /// the station's people follow aboard.
    pub fn shapes_split(&self) -> (&[f32], &[f32]) {
        self.list.data().split_at(self.bodies_from)
    }

    /// The same frame cut where the fog goes: everything the fog lies over
    /// — the deck, the bodies, the bedding — and everything over the fog —
    /// the shots, the night, the rings, the marquee. For a host that draws
    /// the smooth fog itself (`Fog::Crew`): its fog goes between the two,
    /// so a bolt is seen whatever it flies through.
    pub fn shapes_fog_split(&self) -> (&[f32], &[f32]) {
        let data = self.list.data();
        data.split_at(self.fog_from.min(data.len()))
    }

    /// [`Game::shapes_split`] with its second half cut where the fog goes:
    /// the deck, the bodies under the fog, and what is over the fog.
    pub fn shapes_in_three(&self) -> (&[f32], &[f32], &[f32]) {
        let data = self.list.data();
        let bodies = self.bodies_from.min(data.len());
        let fog = self.fog_from.clamp(bodies, data.len());
        let (deck, rest) = data.split_at(bodies);
        let (under, over) = rest.split_at(fog - bodies);
        (deck, under, over)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::combat::{PACK_COLS, Tier, WeaponKind};
    use crate::health::MAX_BLOOD;
    use crate::room::BERTHS;

    /// A frame at 1x.
    const DT: f32 = 1.0 / 60.0;

    fn room() -> Game {
        Game::new(3, ROOM_W, ROOM_H)
    }

    /// The classic room with nobody's dressings in the way (feature 87):
    /// every Bim starts with a box of them in the first cells its
    /// footprint fits, which is exactly where a test that lays a pack
    /// out by hand wants to put something else.
    fn room_with_bare_packs() -> Game {
        let mut game = room();
        for who in 0..game.crew_count() as usize {
            game.set_bandages_for_probe(who, 0);
        }
        game
    }

    /// A bunk is one Bim's. The classic room starts with bunk `i` Bim `i`'s;
    /// the player may give either to either, and whoever had it loses it.
    /// A Bim with no bunk lies down on the deck where it stands — for the
    /// deck's allowance, three hours, and not again until the six-hour
    /// window it opened has closed — and is sore for half a day after,
    /// rest running out half as fast again. Given a bunk back, it sleeps in
    /// it; a bunk that is not the ship's is refused.
    #[test]
    fn a_bunk_is_one_bim_s_and_a_bim_with_none_sleeps_on_the_deck_three_hours_in_six() {
        use crate::needs::SORE_TIRING;
        let mut game = room();
        game.set_autonomous(false);
        // And the timetable wiped: a scheduled night is not the Bim's idea,
        // and it would send him to the deck again in the middle of this.
        for hour in 0..24 {
            game.set_schedule_slot(hour, 0);
        }
        assert_eq!(game.bed_count(), BERTHS);
        assert_eq!(game.bed_of(0), Some(0));
        assert_eq!(game.bed_of(1), Some(1));
        assert_eq!(game.bed_owner(0), Some(0));
        assert!(!game.assign_bed(0, Some(BERTHS)), "no such bunk");
        assert_eq!(game.bed_of(0), Some(0), "and nothing moved");

        // Kate is given James's bunk: it is hers, and he has none.
        assert!(game.assign_bed(1, Some(0)));
        assert_eq!(game.bed_of(1), Some(0));
        assert_eq!(game.bed_of(0), None);
        assert_eq!(game.bed_owner(1), None, "her old one is nobody's");

        // Sent to bed, he lies down on the deck where he stands rather than
        // walking to anybody's bunk.
        let here = game.put_for_probe(0, vec2(ROOM_W * 0.5, ROOM_H * 0.6));
        // Tired enough that three hours does not rest him: fully rested is
        // up, whatever the clock says.
        game.bims[0].needs.spend(Need::Rest, 0.85);
        assert!(game.can_sleep_on_ground(0));
        assert!(game.rest(0, SLEEP_MINUTES));
        assert!(game.bims[0].task.as_ref().unwrap().sleeps_on_ground());
        let mut minutes = 0.0;
        while !game.is_seated_for_probe(0) && minutes < 30.0 {
            game.simulate(1.0);
            minutes += MINUTES_PER_SECOND;
        }
        assert!(game.is_seated_for_probe(0), "lying down");
        assert!(
            (game.bim_pos(0) - here).len() < TILE,
            "on the deck where he stood: {:?} from {here:?}",
            game.bim_pos(0)
        );
        game.simulate(1.0);
        minutes += MINUTES_PER_SECOND;
        assert!(game.is_sore(0), "sore from the first minute of it");
        assert_eq!(game.activity(0), JOB_SLEEP);
        // Three hours and he is up, the allowance spent, and he may not lie
        // down again until the window closes: six hours after it opened.
        let mut slept = 0.0;
        while game.is_seated_for_probe(0) && minutes < 5.0 * clock::HOUR {
            game.simulate(1.0);
            minutes += MINUTES_PER_SECOND;
            slept += MINUTES_PER_SECOND;
        }
        assert!(!game.is_seated_for_probe(0), "up again");
        assert!(
            (slept - GROUND_SLEEP).abs() < 10.0,
            "three hours on the deck, not six: {slept}"
        );
        let up_at = minutes;
        for _ in 0..5 {
            game.simulate(1.0);
            minutes += MINUTES_PER_SECOND;
        }
        assert!(game.ground_sleep_left(0) < 1.0);
        assert!(!game.can_sleep_on_ground(0));
        assert!(!game.rest(0, SLEEP_MINUTES), "refused");
        assert!(game.bims[0].task.is_none());
        assert!(game.is_sore(0));
        assert!(
            (game.soreness(0) - SORE_LASTS / clock::HOUR).abs() < 0.25,
            "{}",
            game.soreness(0)
        );
        // Sore, rest runs out half as fast again as it does for Kate, who
        // is not.
        let (james, kate) = (
            game.bims[0].needs.level(Need::Rest),
            game.bims[1].needs.level(Need::Rest),
        );
        for _ in 0..(2 * 60) {
            game.simulate(1.0);
            minutes += MINUTES_PER_SECOND;
        }
        let his = james - game.bims[0].needs.level(Need::Rest);
        let hers = kate - game.bims[1].needs.level(Need::Rest);
        assert!(hers > 0.0);
        assert!(
            (his / hers - SORE_TIRING).abs() < 0.05,
            "sore: {his} against {hers}"
        );
        while minutes < GROUND_WINDOW - 5.0 {
            game.simulate(1.0);
            minutes += MINUTES_PER_SECOND;
        }
        assert!(!game.can_sleep_on_ground(0), "the window is not out yet");
        while minutes < GROUND_WINDOW + 5.0 {
            game.simulate(1.0);
            minutes += MINUTES_PER_SECOND;
        }
        assert!(game.can_sleep_on_ground(0), "and now it is");
        assert!((game.ground_sleep_left(0) - GROUND_SLEEP).abs() < 0.01);
        // The soreness runs from when he got up.
        assert!(game.is_sore(0));
        while minutes < up_at + SORE_LASTS + 5.0 {
            game.simulate(1.0);
            minutes += MINUTES_PER_SECOND;
        }
        assert!(!game.is_sore(0), "over");

        // His bunk back, and the next sleep is in it. Stood back on the open
        // deck first: the day's chats walk him about (see "Two Bims standing
        // together is a layout problem"), and which side of the heads' door
        // he happens to have ended up on moves with the RNG stream — this
        // half of the test is about the bunk, not about where he wandered.
        game.put_for_probe(0, here);
        assert!(game.assign_bed(0, Some(1)));
        assert_eq!(game.bed_of(0), Some(1));
        assert!(
            !game.can_sleep_on_ground(0),
            "a Bim with a bunk never lies on the deck"
        );
        assert!(game.rest(0, SLEEP_MINUTES));
        assert!(!game.bims[0].task.as_ref().unwrap().sleeps_on_ground());
        minutes = 0.0;
        while !game.is_seated_for_probe(0) && minutes < 10.0 * 60.0 {
            game.simulate(1.0);
            minutes += MINUTES_PER_SECOND;
        }
        assert!(game.is_seated_for_probe(0));
        assert!(
            (game.bim_pos(0) - game.room.bed_lie_pos(1)).len() < 1.0,
            "in bunk 1"
        );
        // Giving Kate his bunk while he is in it stands him up first, so
        // the blanket is made and he is not left lying in her bed.
        assert!(game.assign_bed(1, Some(1)));
        assert!(!game.is_seated_for_probe(0), "up");
        assert!(game.bims[0].task.is_none());
        assert_eq!(game.bed_of(0), None);
        assert_eq!(game.bed_of(1), Some(1));
        assert!(!game.room.beds[1].occupied(), "the blanket made");
        // Nobody's bunk is nobody's until somebody dies with one: his was
        // freed when he lost it, and hers frees when she dies.
        assert_eq!(game.bed_owner(0), None);
        game.kill_for_probe(1);
        game.simulate(DT);
        assert!(!game.is_alive(1));
        assert_eq!(game.bed_owner(1), None, "a dead Bim's bunk is nobody's");
    }

    /// The bunks go with the crew from one room to the next: `take_crew`
    /// carries whose bunk was whose and `adopt` reads it back, keeps a
    /// bunk nobody in the new room has, and gives a Bim arriving with none
    /// the first one spare — or none, past the bunks.
    #[test]
    fn a_bunk_goes_with_its_bim_from_one_room_to_the_next_and_a_spare_one_is_given() {
        let mut game = room();
        assert!(game.assign_bed(0, None));
        let crew = game.take_crew();
        assert_eq!(crew[0].bed, None);
        assert_eq!(crew[1].bed, Some(1));
        let mut next = room();
        let old = next.take_crew();
        assert_eq!(old.len(), BERTHS);
        assert!(next.room.sleeps_in.is_empty());
        next.adopt(crew, Vec2::ZERO);
        assert_eq!(next.bed_of(1), Some(1), "kept");
        assert_eq!(
            next.bed_of(0),
            Some(0),
            "the spare one, having arrived with none"
        );
        // A third, past the bunks, has none.
        next.adopt(old, Vec2::ZERO);
        assert_eq!(next.crew_count(), 4);
        assert_eq!(next.bed_of(2), None);
        assert_eq!(next.bed_of(3), None);
    }

    /// A bunk nobody has goes to a bot with none at the next step, and
    /// never to a player's Bim: giving one up and taking one are the
    /// player's own to do. The tags say whose every bunk is, and that one
    /// is nobody's.
    #[test]
    fn a_bot_with_no_bunk_takes_one_that_is_going_spare_and_a_player_does_not() {
        let mut game = room();
        assert_eq!(game.players(), 1, "James is the player's, Kate a bot");
        // Kate's given up: she has it back a step later.
        assert!(game.assign_bed(1, None));
        assert_eq!(game.bed_owner(1), None);
        game.simulate(DT);
        assert_eq!(game.bed_of(1), Some(1), "taken again");
        // James's given up: it stays nobody's, however long, since he is
        // the player's and Kate has one.
        assert!(game.assign_bed(0, None));
        for _ in 0..60 {
            game.simulate(DT);
        }
        assert_eq!(game.bed_of(0), None);
        assert_eq!(game.bed_owner(0), None);
        let tags = game.bunk_tags();
        assert_eq!(tags.len(), BERTHS);
        assert_eq!(tags[0].bed, 0);
        assert_eq!(tags[0].owner, None, "nobody's, and says so");
        assert_eq!(tags[1].owner, Some(1));
        assert!(
            game.room.beds[0].frame.contains(tags[0].at),
            "the tag is on the bunk"
        );
        // Kate moved to his: hers comes free, and he still does not take
        // it — nor does she, having one.
        assert!(game.assign_bed(1, Some(0)));
        game.simulate(DT);
        assert_eq!(game.bed_of(1), Some(0));
        assert_eq!(game.bed_of(0), None);
        assert_eq!(game.bed_owner(1), None);
        // A third, arriving with none past a bunk going spare, is given it
        // on arrival; and a fourth, past the bunks, has none until Kate
        // dies — then hers is the fourth's the step after.
        let mut other = room();
        let more = other.take_crew();
        game.adopt(more, Vec2::ZERO);
        assert_eq!(game.crew_count(), 4);
        assert_eq!(game.bed_of(2), Some(1), "the spare one, on arrival");
        assert_eq!(game.bed_of(3), None);
        game.simulate(DT);
        assert_eq!(game.bed_of(3), None, "none going spare");
        game.kill_for_probe(1);
        game.simulate(DT);
        assert!(!game.is_alive(1));
        game.simulate(DT);
        assert_eq!(game.bed_of(3), Some(0), "the dead Bim's, the step after");
        assert_eq!(game.bed_of(0), None, "and the player's still none");
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
        game.set_hostiles(vec![Some((target, WeaponKind::LaserPistol.basic()))]);
        game.simulate(DT);
        assert!(game.is_recruited(0), "at war, under orders");
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
                .all(|s| s.at == target && s.weapon == WeaponKind::LaserPistol.basic())
        );
        assert!(game.take_shots().is_empty(), "drained");
        assert!(
            game.take_hits().is_empty(),
            "nothing landed on the targets: nothing flew"
        );

        // Peace: the target gone, the orders lifted.
        game.set_hostiles(Vec::new());
        game.simulate(DT);
        assert!(!game.is_recruited(0));
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
            game.enemy_fire(from, james, WeaponKind::LaserPistol.basic(), false);
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
        let had = game.bandages_of(who);
        assert!(had > 0);

        // Shot mid-sweep with the medical row taken down to the middle —
        // it starts at the top now — it is on offer, but it waits its turn
        // behind the sweep.
        assert_eq!(
            game.priorities.of(Job::Medical),
            work::HIGHEST,
            "the default"
        );
        game.set_work_priority(Job::Medical.code(), work::DEFAULT);
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
        assert_eq!(game.bandages_of(who), had - 1);
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
    fn a_crewmate_bleeding_or_dying_is_treated_by_whoever_is_free() {
        // --- a_crewmate_bleeding_is_dressed_by_whoever_is_free ---
        {
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
                Some(Care::Bandage(1, Part::Body)),
                "the part with the most wounds first"
            );
            assert_eq!(
                game.medical_on_offer(1),
                None,
                "a patient out cold is nobody's doctor"
            );
            let had = game.bandages_of(0);
            // The walk over and ten minutes on the body, then the legs: two
            // dressings, well inside the hour.
            let mut steps = 0;
            while game.bleeding(1) > 0 && steps < 60 * 60 {
                game.simulate(DT);
                steps += 1;
            }
            assert_eq!(game.bleeding(1), 0, "dressed within the hour");
            assert_eq!(game.bandages_of(0), had - 2);
            assert!(game.is_alive(1));
            assert!(
                !game
                    .work_on_offer_for_probe(0)
                    .contains(&Job::Medical.code()),
                "nothing left to dress"
            );
        }

        // --- a_dying_crewmate_is_treated_with_a_medkit_by_whoever_is_free ---
        {
            let mut game = room();
            game.put_for_probe(0, vec2(ROOM_W * 0.55, ROOM_H * 0.5));
            game.put_for_probe(1, vec2(ROOM_W * 0.35, ROOM_H * 0.5));
            game.set_autonomous(false);
            let out = game.wound(1, Part::Body, Part::Body.max());
            let trauma = out.trauma.expect("dying");
            assert_eq!(trauma.part(), Part::Body);
            assert!(!game.treat(1, 1, Part::Body), "nobody treats their own");
            assert!(!game.treat(0, 1, Part::Head), "nothing on the head");
            game.set_autonomous(true);
            game.simulate(DT);
            assert_eq!(
                game.medical_on_offer(0),
                Some(Care::Treat(1, Part::Body)),
                "the trauma before her wound"
            );
            // Her own trauma is nobody's to treat; the wound the shot opened
            // she dresses herself, and that comes first for her.
            assert_eq!(
                game.medical_on_offer(1),
                Some(Care::Bandage(1, Part::Body)),
                "her own wound, never her own trauma"
            );
            let had = game.medkits();
            assert!(had > 0);
            let mut steps = 0;
            while game.is_dying(1) && steps < 60 * 60 {
                game.simulate(DT);
                steps += 1;
            }
            assert!(!game.is_dying(1), "treated within the hour");
            assert_eq!(game.medkits(), had - 1);
            assert_eq!(game.take_medkits_used(), 1);
            assert_eq!(game.take_treated(), vec![(1, trauma)]);
            assert_eq!(
                game.part_health(1, Part::Body),
                Part::Body.max() * crate::health::TREATED_TO
            );
            assert!(game.is_alive(1));
            // What it left behind, if anything, is on her for a while.
            assert_eq!(game.lasting(1).len(), usize::from(trauma.after().is_some()));
            // The wound is still open: the medical row goes on to it.
            assert_eq!(game.medical_on_offer(0), Some(Care::Bandage(1, Part::Body)));
        }
    }

    #[test]
    fn a_bim_merely_hurt_fights_on_and_runs_only_dying_shooting_back() {
        // --- a_dying_crew_member_backs_away_from_the_enemy_shooting_as_it_goes ---
        {
            let mut game = room();
            game.set_autonomous(false);
            // Kate under arms with James's pistol in her hand, an enemy four
            // tiles to her right: she shoots at it. James unarmed, out of it.
            let kate = game.put_for_probe(1, vec2(ROOM_W * 0.35, ROOM_H * 0.5));
            game.put_for_probe(0, vec2(ROOM_W * 0.7, ROOM_H * 0.85));
            game.issue(0, Gear::default());
            let near = kate + vec2(4.0 * TILE, 0.0);
            game.set_hostiles(vec![Some((near, WeaponKind::LaserPistol.basic()))]);
            for _ in 0..30 {
                game.simulate(DT);
            }
            assert!(game.is_alarmed());
            assert!(game.is_armed(1));
            // Her body at nothing: dying, and giving ground — further from
            // the enemy every second — but with the gun up and firing as she
            // goes, backwards, her face to the enemy. A crew member that
            // holstered and walked off from an enemy in plain view was the
            // fight the player saw.
            let out = game.wound(1, Part::Body, Part::Body.max());
            assert!(out.trauma.is_some());
            assert!(game.is_dying(1));
            game.take_hits();
            let before = (game.bim_pos(1) - near).len();
            let mut fired = 0;
            let mut backing = 0;
            for _ in 0..(60 * 4) {
                let had = game.bolts_in_flight();
                game.simulate(DT);
                fired += game.bolts_in_flight().saturating_sub(had);
                backing += usize::from(game.is_backing_for_probe(1));
            }
            assert!(game.is_fleeing(1));
            assert!(game.is_armed(1), "the gun up while it runs");
            assert!(fired > 0, "and shooting back");
            assert!(backing > 0, "giving ground backwards, face to the enemy");
            let after = (game.bim_pos(1) - near).len();
            assert!(
                after > before + 2.0 * TILE,
                "ran from {before} to {after} off the enemy"
            );
            // The player's own runs too, whatever the player said.
            game.toggle_recruited(0);
            let james = game.bim_pos(0);
            game.set_hostiles(vec![Some((
                james + vec2(3.0 * TILE, 0.0),
                WeaponKind::LaserPistol.basic(),
            ))]);
            game.wound(0, Part::Head, Part::Head.max());
            let before = 3.0 * TILE;
            for _ in 0..(60 * 4) {
                game.simulate(DT);
            }
            assert!(game.is_fleeing(0));
            let after = (game.bim_pos(0) - (james + vec2(3.0 * TILE, 0.0))).len();
            assert!(after > before + TILE, "{after}");
            // The enemy gone, it stops running and stands.
            game.set_hostiles(Vec::new());
            assert!(!game.is_fleeing(0));
            assert!(!game.is_fleeing(1));
            assert!(game.is_dying(1), "and waits for a medkit");
        }

        // --- a_crew_member_merely_hurt_fights_on_and_runs_only_dying ---
        {
            let mut game = room();
            game.set_autonomous(false);
            // Kate under arms, an enemy four tiles to her right: she shoots at
            // it. One wound on her body — bleeding, nowhere near dying — and
            // she stands where she is and shoots on, the wound left for the
            // calm; her body at nothing, and she runs the other way, her gun
            // still up.
            let kate = game.put_for_probe(1, vec2(ROOM_W * 0.35, ROOM_H * 0.5));
            game.put_for_probe(0, vec2(ROOM_W * 0.7, ROOM_H * 0.85));
            game.issue(0, Gear::default());
            let near = kate + vec2(4.0 * TILE, 0.0);
            game.set_hostiles(vec![Some((near, WeaponKind::LaserPistol.basic()))]);
            for _ in 0..30 {
                game.simulate(DT);
            }
            assert!(game.is_alarmed());
            assert!(game.is_armed(1));
            assert!(!game.is_fleeing(1), "whole, she stands and shoots");
            let out = game.wound(1, Part::Body, 4.0);
            assert!(out.trauma.is_none(), "a wound, not a dying state");
            assert!(!game.is_dying(1));
            assert!(game.bims[1].health.is_hurt());
            let before = (game.bim_pos(1) - near).len();
            for _ in 0..(60 * 4) {
                game.simulate(DT);
            }
            assert!(!game.is_fleeing(1), "hurt is not dying: she holds");
            assert!(game.is_armed(1), "and keeps shooting");
            let after = (game.bim_pos(1) - near).len();
            assert!(
                after < before + 2.0 * TILE,
                "no run: {before} to {after} off the enemy"
            );
            // Her body at nothing: dying, and now she runs.
            let out = game.wound(1, Part::Body, Part::Body.max());
            assert!(out.trauma.is_some());
            assert!(game.is_dying(1));
            game.take_hits();
            for _ in 0..(60 * 4) {
                game.simulate(DT);
            }
            assert!(game.is_fleeing(1));
            assert!(game.is_armed(1), "a crew member's run keeps the gun up");

            // The player's own, recruited and wounded short of dying, walks
            // where it is sent: only dying makes it run.
            game.issue(0, Gear::issued());
            game.toggle_recruited(0);
            assert!(!game.wound(0, Part::Body, 4.0).leg_lost);
            assert!(game.bims[0].health.is_hurt());
            for _ in 0..30 {
                game.simulate(DT);
            }
            assert!(!game.is_fleeing(0), "the player's own holds its ground");
            assert!(game.is_armed(0));

            // An enemy's people the same: one of them shot once fights on.
            let mut foes = room();
            foes.set_autonomous(false);
            let foe = foes.put_for_probe(1, vec2(ROOM_W * 0.35, ROOM_H * 0.5));
            foes.put_for_probe(0, vec2(ROOM_W * 0.7, ROOM_H * 0.85));
            foes.set_hostile_bodies(true);
            foes.set_hostiles(vec![Some((
                foe + vec2(4.0 * TILE, 0.0),
                WeaponKind::LaserPistol.basic(),
            ))]);
            assert!(!foes.wound(1, Part::Body, 4.0).leg_lost);
            for _ in 0..30 {
                foes.simulate(DT);
            }
            assert!(foes.bims[1].health.is_hurt());
            assert!(!foes.is_fleeing(1), "a garrison does not run at a scratch");
            assert!(foes.is_armed(1));
            // And dying, one of them runs with its weapon holstered: only
            // the crew's run is a fighting one.
            assert!(foes.wound(1, Part::Body, Part::Body.max()).trauma.is_some());
            // The other one unarmed, so every shot would be the runner's.
            foes.issue(0, Gear::default());
            foes.take_shots();
            let mut shots = 0;
            for _ in 0..(60 * 2) {
                foes.simulate(DT);
                shots += foes.take_shots().len();
            }
            assert!(foes.is_fleeing(1));
            assert!(!foes.is_armed(1), "an enemy's run is silent");
            assert_eq!(shots, 0);
        }
    }

    #[test]
    fn a_hurt_crew_member_out_of_the_enemy_s_sight_binds_its_own_wound_and_comes_back() {
        let mut game = room();
        game.set_autonomous(true);
        let james = game.put_for_probe(0, vec2(ROOM_W * 0.3, ROOM_H * 0.5));
        game.put_for_probe(1, vec2(ROOM_W * 0.5, ROOM_H * 0.5));
        game.recruit_for_probe(0, true);
        // An enemy within range but beyond the walls — twenty-five tiles off, the
        // room being sixteen across: the alarm, nothing to
        // shoot at, and a wounded Kate — not running, hurt is not dying —
        // with nothing in her sight, binding the wound where she stands.
        let unseen = james + vec2((ALARM_RANGE - 5.0) * TILE, 0.0);
        game.set_hostiles(vec![Some((unseen, WeaponKind::LaserPistol.basic()))]);
        assert!(!game.wound(1, Part::Body, 4.0).leg_lost);
        let had = game.bandages_of(1);
        for _ in 0..30 {
            game.simulate(DT);
        }
        assert!(game.is_alarmed());
        assert!(!game.is_fleeing(1), "hurt, not dying: no run");
        assert_eq!(game.activity(1), JOB_BANDAGE, "binding it, unseen");
        let mut steps = 0;
        while game.bleeding(1) > 0 && steps < 60 * 60 {
            game.simulate(DT);
            if game.bleeding(1) > 0 {
                assert_eq!(
                    game.activity(1),
                    JOB_BANDAGE,
                    "the dressing is not put down"
                );
            }
            steps += 1;
        }
        assert_eq!(game.bleeding(1), 0, "dressed within the hour");
        assert_eq!(game.bandages_of(1), had - 1);
        for _ in 0..30 {
            game.simulate(DT);
        }
        assert!(game.is_armed(1), "armed again once dressed");
    }

    /// A bot's dressing gives way to a shot: begun with nothing in its
    /// sight, it is put down for good the moment an enemy walks into its
    /// weapon's reach, and the weapon comes out and fires that step —
    /// where it used to wind on for the ten minutes with a target in front
    /// of it. Out of sight again, it binds the wound after all.
    #[test]
    fn a_bot_puts_its_dressing_down_the_moment_it_has_a_shot() {
        let mut game = room();
        game.set_autonomous(true);
        let kate = game.put_for_probe(1, vec2(ROOM_W * 0.35, ROOM_H * 0.5));
        game.put_for_probe(0, vec2(ROOM_W * 0.7, ROOM_H * 0.85));
        // James the player's, recruited and unarmed: every bolt is Kate's.
        game.recruit_for_probe(0, true);
        game.issue(0, Gear::default());
        game.issue(1, Gear::issued());
        game.set_bandages_for_probe(1, room::BANDAGES_AT_DAWN);
        let pistol = WeaponKind::LaserPistol.basic();
        let unseen = kate + vec2((ALARM_RANGE - 5.0) * TILE, 0.0);
        game.set_hostiles(vec![Some((unseen, pistol))]);
        assert!(!game.wound(1, Part::Body, 4.0).leg_lost);
        for _ in 0..30 {
            game.simulate(DT);
        }
        assert_eq!(game.activity(1), JOB_BANDAGE, "binding it, unseen");
        assert!(!game.is_armed(1), "both hands on the bandage");
        let had = game.bandages_of(1);

        // An enemy four tiles off in plain view: the bandage is put down
        // and the gun comes out the same step.
        game.set_hostiles(vec![Some((kate + vec2(4.0 * TILE, 0.0), pistol))]);
        game.simulate(DT);
        assert_ne!(game.activity(1), JOB_BANDAGE, "put down at once");
        assert!(game.is_armed(1), "and the gun out");
        let mut fired = 0;
        for _ in 0..60 {
            let had = game.bolts_in_flight();
            game.simulate(DT);
            fired += game.bolts_in_flight().saturating_sub(had);
            assert_ne!(game.activity(1), JOB_BANDAGE, "not taken up under fire");
        }
        assert!(fired > 0, "shooting");
        assert_eq!(game.bandages_of(1), had, "the dressing never spent");
        assert!(game.bleeding(1) > 0);

        // Gone beyond the walls again: she binds it after all.
        game.set_hostiles(vec![Some((unseen, pistol))]);
        let mut steps = 0;
        while game.bleeding(1) > 0 && steps < 60 * 60 {
            game.simulate(DT);
            steps += 1;
        }
        assert_eq!(game.bleeding(1), 0, "dressed once out of sight");
    }

    #[test]
    fn a_helper_follows_a_patient_that_moved_and_a_runner_holds_still_for_it() {
        let mut game = room();
        game.set_autonomous(false);
        // James sent to dress Kate; she is moved across the room while he
        // is on his way, and he walks to where she is now rather than
        // dressing the spot she left.
        game.put_for_probe(0, vec2(ROOM_W * 0.55, ROOM_H * 0.5));
        game.put_for_probe(1, vec2(ROOM_W * 0.35, ROOM_H * 0.5));
        game.recruit_for_probe(1, true);
        let had = game.bandages_of(0);
        assert!(!game.wound(1, Part::Body, 12.0).leg_lost);
        assert!(game.bandage(0, 1, Part::Body));
        for _ in 0..30 {
            game.simulate(DT);
        }
        assert_eq!(game.activity(0), JOB_BANDAGE);
        game.put_for_probe(1, vec2(ROOM_W * 0.35, ROOM_H * 0.85));
        let mut steps = 0;
        while game.bleeding(1) > 0 && steps < 60 * 60 {
            game.simulate(DT);
            steps += 1;
        }
        assert_eq!(game.bleeding(1), 0, "dressed where she moved to");
        assert_eq!(game.bandages_of(0), had - 1);
        let apart = (game.bim_pos(0) - game.bim_pos(1)).len();
        assert!(apart <= 2.0 * TILE, "{apart}");

        // Kate shot to a dying state and running from an enemy to her
        // right; James, the player's own, ordered to bandage her from the
        // side she runs towards. She holds still once he is nearly at
        // her, and the wound is dressed — the run goes on, since dying
        // wants a kit, but she no longer bleeds while she runs.
        game.recruit_for_probe(1, false);
        let kate = game.put_for_probe(1, vec2(ROOM_W * 0.4, ROOM_H * 0.5));
        game.put_for_probe(0, vec2(ROOM_W * 0.12, ROOM_H * 0.5));
        let near = kate + vec2(4.0 * TILE, 0.0);
        game.set_hostiles(vec![Some((near, WeaponKind::LaserPistol.basic()))]);
        assert!(game.wound(1, Part::Body, Part::Body.max()).trauma.is_some());
        game.take_hits();
        for _ in 0..30 {
            game.simulate(DT);
        }
        assert!(game.is_dying(1));
        assert!(game.is_fleeing(1));
        assert!(game.bandage(0, 1, Part::Body));
        let mut steps = 0;
        while game.bleeding(1) > 0 && steps < 60 * 90 {
            game.simulate(DT);
            steps += 1;
        }
        assert_eq!(game.bleeding(1), 0, "caught and dressed");
        assert_eq!(game.bandages_of(0), had - 2);
        assert!(game.is_dying(1), "and still wants a kit");
    }

    /// The playtest ship as a hostile room — its people the enemies —
    /// with a body at each of `starts`, in tiles. The room has two
    /// bulkhead doors (row 6 and row 13) and the airlock, all doors now.
    fn hostile_ship(starts: &[(f32, f32)]) -> Game {
        let layout = crate::aboard::layout_of(&shipdesign::fixture::playtest_ship());
        let (w, h) = (layout.bounds.width(), layout.bounds.height());
        let at: Vec<Vec2> = starts.iter().map(|&(x, y)| tile_middle(x, y)).collect();
        let mut game = Game::with_layout(layout, 7, &at, w, h);
        game.set_autonomous(false);
        game.set_hostile_bodies(true);
        game
    }

    fn tile_middle(x: f32, y: f32) -> Vec2 {
        vec2((x + 0.5) * TILE, (y + 0.5) * TILE)
    }

    /// The door whose opening holds tile `(x, y)`.
    fn door_at_tile(game: &Game, x: f32, y: f32) -> usize {
        game.door_index_at(tile_middle(x, y))
            .unwrap_or_else(|| panic!("no door at ({x}, {y})"))
    }

    /// An airlock is a door: it locks from the panel, it is a wall to the
    /// pathfinder locked, and it is a solid to the eye shut like the rest.
    /// An enemy with its target behind a locked door goes to the door and
    /// forces it: fifteen seconds of heaving, heard every couple of
    /// seconds and drawn as a bar, and the lock gives. One at a door.
    #[test]
    fn an_airlock_is_a_door_that_locks_and_an_enemy_smashes_through_it_in_fifteen_seconds() {
        // --- an_airlock_is_a_door_that_locks ---
        {
            let mut game = hostile_ship(&[(8.0, 10.0)]);
            // The playtest airlock is the starboard skin at (17, 11)–(17, 12).
            let lock = door_at_tile(&game, 17.0, 11.0);
            assert!(game.room.doors[lock].airlock);
            assert_eq!(game.room.doors[lock].smash_time(), door::SMASH_AIRLOCK);
            let bulkhead = door_at_tile(&game, 15.0, 13.0);
            assert!(!game.room.doors[bulkhead].airlock);
            assert_eq!(game.room.doors[bulkhead].smash_time(), door::SMASH_DOOR);
            assert!(game.room.locked_doors().is_empty());
            game.room.doors[lock].order(door::Order::Lock);
            game.simulate(DT);
            assert_eq!(game.room.locked_doors().len(), 1);
            assert!(game.room.doors[lock].locked);
            assert_eq!(game.room.doors[lock].locked_by, door::Locker::Crew);
            game.room.doors[lock].order(door::Order::Unlock);
            game.simulate(DT);
            assert!(game.room.locked_doors().is_empty());
        }

        // --- an_enemy_smashes_through_a_locked_door_in_fifteen_seconds ---
        {
            // The enemy a tile inside engineering with the aft door open for
            // it, the target on the main deck in plain view through the
            // doorway — a hostile room's people hunt only what they have
            // seen — and then the crew lock the door between them.
            let mut game = hostile_ship(&[(15.0, 14.0)]);
            let door = door_at_tile(&game, 15.0, 13.0);
            let target = tile_middle(15.0, 10.0);
            for _ in 0..30 {
                game.simulate(DT);
            }
            assert!(
                game.room.doors[door].is_open(),
                "open for the body a tile off"
            );
            // The lock, and the sighting through the leaves as they shut: the
            // nav is a wall from the order, so the enemy never gets through.
            game.room.doors[door].order(door::Order::Lock);
            game.set_hostiles(vec![Some((target, WeaponKind::LaserPistol.basic()))]);
            game.simulate(DT);
            assert!(game.at_war, "seen through the closing door");
            let mut heaves = 0;
            let mut forced_at = None;
            let mut bar_seen = false;
            for frame in 0..(60 * 30) {
                game.simulate(DT);
                for cued in game.take_cues() {
                    match cued.cue {
                        Cue::DoorSmash => heaves += 1,
                        Cue::DoorForced => forced_at = forced_at.or(Some(frame)),
                        _ => {}
                    }
                }
                if game.room.doors[door].smash_progress().is_some() {
                    bar_seen = true;
                }
                if forced_at.is_some() {
                    break;
                }
            }
            let forced = forced_at.expect("the door should have been forced");
            assert!(bar_seen, "the smashing is drawn");
            assert!(heaves >= 5, "{heaves} heaves");
            // Fifteen seconds of heaving, after however long the walk took.
            assert!(
                forced >= (door::SMASH_DOOR / DT) as i32,
                "forced at frame {forced}"
            );
            assert!(!game.room.doors[door].locked);
            assert!(game.room.doors[door].smash.is_none());
            assert!(game.room.locked_doors().is_empty());
        }
    }

    /// A room with machines and no Bims at all (feature 83): the deck's
    /// nav, sight and fight serve them, an unlocked door opens for one
    /// walking up to it, and a locked one between it and its target is
    /// heaved at until the lock gives — the boarder's rule, with a body
    /// that is not a Bim.
    #[test]
    fn machines_walk_a_deck_open_its_doors_and_force_a_locked_one() {
        use crate::droid::{Droid, DroidKind};
        // The playtest ship as a hostile room with nobody in it, and one
        // Trooper a tile inside engineering with the aft door beside it.
        let mut game = hostile_ship(&[]);
        assert_eq!(game.crew_count(), 0, "no Bims at all");
        let at = tile_middle(15.0, 14.0);
        let who = game.add_droid(Droid::new(DroidKind::Trooper, Tier::One, 0, 1, at, 0.0, 5));
        assert_eq!(who, 0, "the machines start where the Bims end");
        assert_eq!(game.body_count(), 1);
        assert_eq!(game.droid_count(), 1);

        // --- an unlocked door opens for it ---
        let door = door_at_tile(&game, 15.0, 13.0);
        assert!(!game.room.doors[door].is_open(), "shut to begin with");
        for _ in 0..30 {
            game.simulate(DT);
        }
        assert!(
            game.room.doors[door].is_open(),
            "open for the machine a tile off"
        );

        // --- and a locked one is forced ---
        // The target on the main deck in plain view through the doorway,
        // so the machine has seen it; then the door is locked between.
        let target = tile_middle(15.0, 10.0);
        game.room.doors[door].order(door::Order::Lock);
        game.set_hostiles(vec![Some((target, WeaponKind::LaserPistol.basic()))]);
        game.simulate(DT);
        assert!(game.at_war, "seen through the closing door");
        let mut forced = None;
        let mut bar_seen = false;
        for frame in 0..(60 * 30) {
            game.simulate(DT);
            for cued in game.take_cues() {
                if matches!(cued.cue, Cue::DoorForced) {
                    forced = forced.or(Some(frame));
                }
            }
            if game.room.doors[door].smash_progress().is_some() {
                bar_seen = true;
            }
            if forced.is_some() {
                break;
            }
        }
        let forced = forced.expect("a machine forces a locked door");
        assert!(bar_seen, "and the heaving is drawn");
        assert!(
            forced >= (door::SMASH_DOOR / DT) as i32,
            "forced at frame {forced}"
        );
        assert!(!game.room.doors[door].locked);
        // And it is still the one machine, still standing.
        assert_eq!(game.droid_count(), 1);
        assert!(!game.droids()[0].destroyed);
    }

    /// A machine shoots the way a hostile Bim does: the shot is recorded
    /// rather than flown, since a hostile room's bolts fly in the crew's.
    /// And a Husk's claw locks and strikes at arm's length.
    #[test]
    fn a_machine_records_its_shots_and_a_husk_strikes_at_reach() {
        use crate::droid::{Droid, DroidKind};
        // --- a Trooper fires at what it can see ---
        {
            let mut game = hostile_ship(&[]);
            let at = tile_middle(15.0, 12.0);
            game.add_droid(Droid::new(DroidKind::Trooper, Tier::One, 0, 1, at, 0.0, 5));
            let target = tile_middle(15.0, 9.0);
            let mut shots = 0;
            for _ in 0..(60 * 10) {
                game.set_hostiles(vec![Some((target, WeaponKind::LaserPistol.basic()))]);
                game.simulate(DT);
                shots += game.take_shots().len();
                if shots > 0 {
                    break;
                }
            }
            assert!(shots > 0, "the machine fired");
            // Nothing flew here: a hostile room records and the world
            // carries. The crew's room is where a bolt is drawn.
            assert!(game.combat.bolts.is_empty(), "no bolt in a hostile room");
        }

        // --- a Husk locks a target within reach and lands a blow ---
        {
            let mut game = hostile_ship(&[]);
            let at = tile_middle(15.0, 12.0);
            game.add_droid(Droid::new(DroidKind::Husk, Tier::One, 0, 1, at, 0.0, 6));
            // Half a tile away: inside `MELEE_RANGE`.
            let target = at + vec2(TILE * 0.6, 0.0);
            let mut blows = 0;
            for _ in 0..(60 * 10) {
                game.set_hostiles(vec![Some((target, WeaponKind::LaserPistol.basic()))]);
                game.simulate(DT);
                for shot in game.take_shots() {
                    if shot.melee {
                        blows += 1;
                        // A claw crushes; it is not a cut and does not
                        // bleed the way a blade's does.
                        assert!(!shot.cut, "a claw is no blade");
                        assert!(shot.damage > 0.0);
                    }
                }
                if blows > 0 {
                    break;
                }
            }
            assert!(blows > 0, "the claws landed a blow");
            assert_eq!(game.droids()[0].locked, Some(0), "and it is locked on");
        }
    }

    /// The Unmaker's rule, where it is applied: a hit carrying a strip
    /// takes it off the piece over the part with the piece's protection
    /// ignored and leaves the part alone; a bare part takes the damage.
    #[test]
    fn a_strip_unmakes_the_armour_and_leaves_the_body_alone() {
        let mut game = Game::new(3, 900.0, 700.0);
        let kevlar = Piece::new(1, ArmourKind::BasicKevlar, Tier::One);
        let whole = kevlar.health;
        let gear = game.gear(0);
        game.issue(
            0,
            Gear {
                body: Some(kevlar),
                ..gear
            },
        );
        let body = game.part_health(0, Part::Body);
        // A strip bigger than the piece's protection and smaller than
        // its health: the piece loses exactly the strip.
        game.strike_stripping(0, Part::Body, 9.0, false, 8.0);
        assert_eq!(game.gear(0).body.unwrap().health, whole - 8.0);
        assert_eq!(game.part_health(0, Part::Body), body, "the part is whole");
        assert_eq!(game.wounds(0, Part::Body), 0);

        // What the piece cannot take is **lost**, not passed on: a strip
        // far bigger than what is left breaks the piece and no more.
        game.strike_stripping(0, Part::Body, 9.0, false, 1e6);
        assert!(game.gear(0).body.unwrap().broken());
        assert_eq!(game.part_health(0, Part::Body), body, "still whole");

        // With the piece broken it shields nothing: the part takes the
        // plain damage the ordinary way.
        game.strike_stripping(0, Part::Body, 9.0, false, 1e6);
        assert_eq!(game.part_health(0, Part::Body), body - 9.0);

        // And a bare part takes it from the first.
        let mut bare = Game::new(3, 900.0, 700.0);
        let legs = bare.part_health(0, Part::Legs);
        bare.strike_stripping(0, Part::Legs, 4.0, false, 30.0);
        assert_eq!(bare.part_health(0, Part::Legs), legs - 4.0);
    }

    /// A dying enemy running through a door locks it behind itself, binds
    /// its wounds sealed in — a dressing every ten seconds, the trauma
    /// treated once nothing bleeds — and, dying no more, unlocks its own
    /// door and comes out to fight.
    #[test]
    fn a_fleeing_enemy_seals_itself_in_binds_its_wounds_and_comes_back() {
        // The enemy just north of the aft door, the target four tiles
        // north of it: the run goes south, through the door.
        let mut game = hostile_ship(&[(15.0, 12.0)]);
        let door = door_at_tile(&game, 15.0, 13.0);
        let target = tile_middle(15.0, 8.0);
        game.set_hostiles(vec![Some((target, WeaponKind::LaserPistol.basic()))]);
        game.simulate(DT);
        let out = game.wound(0, Part::Body, Part::Body.max());
        assert!(out.trauma.is_some());
        assert!(game.is_dying(0));
        game.take_hits();
        let mut sealed_at = None;
        for frame in 0..(60 * 20) {
            game.simulate(DT);
            if game.room.doors[door].locked {
                sealed_at = Some(frame);
                break;
            }
        }
        let sealed = sealed_at.expect("it should have locked the door behind it");
        assert_eq!(game.room.doors[door].locked_by, door::Locker::Body(0));
        let south = game.bim_pos(0).y > game.room.doors[door].rect.max.y;
        assert!(south, "sealed in on the far side");
        // Bound, treated, and out again: the lock lifted by its own hand.
        let mut unlocked_at = None;
        for frame in 0..(60 * 60) {
            game.simulate(DT);
            if !game.room.doors[door].locked {
                unlocked_at = Some(frame);
                break;
            }
        }
        let unlocked = unlocked_at.expect("it should have come back out");
        assert!(
            unlocked as f32 * DT >= 2.0 * BIND_EVERY - 1.0,
            "unlocked after {unlocked} frames, sealed at {sealed}"
        );
        assert!(!game.is_dying(0));
        assert_eq!(game.bleeding(0), 0);
        assert!(!game.is_fleeing(0));
    }

    /// A bare hull twenty tiles across, decked, with these lights on it.
    fn box_ship(lights: &[(shipdesign::PartKind, (u32, u32))]) -> shipdesign::ShipDesign {
        use shipdesign::parts::{PartKind, Rotation};
        use shipdesign::{Budget, Edit, ShipDesign, apply};
        let budget = Budget::new(10_000_000);
        let mut design = ShipDesign::new(20);
        let mut put = |design: &mut ShipDesign, kind: PartKind, origin: (u32, u32)| {
            if let Ok(next) = apply(
                design,
                &budget,
                Edit::Place {
                    kind,
                    origin,
                    rotation: Rotation::R0,
                },
            ) {
                *design = next;
            }
        };
        for y in 1..19 {
            for x in 1..19 {
                put(&mut design, PartKind::Structure, (x, y));
            }
        }
        for y in 2..18 {
            for x in 2..18 {
                put(&mut design, PartKind::Floor, (x, y));
            }
        }
        for i in 1..19 {
            put(&mut design, PartKind::OutsideWall, (i, 1));
            put(&mut design, PartKind::OutsideWall, (i, 18));
            put(&mut design, PartKind::OutsideWall, (1, i));
            put(&mut design, PartKind::OutsideWall, (18, i));
        }
        for &(kind, at) in lights {
            // A wall light hangs from the hull beside it.
            let turn = shipdesign::wall_light_rotation(&design, at).unwrap_or(Rotation::R0);
            if let Ok(next) = apply(
                &design,
                &budget,
                Edit::Place {
                    kind,
                    origin: at,
                    rotation: turn,
                },
            ) {
                design = next;
            }
        }
        design
    }

    /// A comfort lifts the surroundings of the tiles round it: a big plant
    /// is worth half a clean tile over the seven-by-seven about it and
    /// nothing a tile further out; a picture hung on the hull the same,
    /// less; two big plants and a small one on the same spot are capped at
    /// a clean tile's worth. And the lift is on top of the mess, not in
    /// its place: the deck under the plant is still what it was, so a Bim
    /// grinding beside a spill grinds less and one on a fouled deck no
    /// less.
    #[test]
    fn a_plant_lifts_the_surroundings_and_the_mess_stays_underneath() {
        use crate::filth::{BASELINE, LIFT_CAP};
        use shipdesign::{BIG_PLANT_LIFT, PICTURE_LIFT, PartKind, SMALL_PLANT_LIFT};
        let layout = crate::aboard::layout_of(&box_ship(&[
            (PartKind::BigPlant, (10, 10)),
            (PartKind::Picture, (17, 4)),
        ]));
        let (w, h) = (layout.bounds.width(), layout.bounds.height());
        let mut game = Game::with_layout(layout, 7, &[tile_middle(3.0, 10.0)], w, h);
        game.set_autonomous(false);
        let big = BIG_PLANT_LIFT as f32;
        let filth = &game.room.filth;
        assert_eq!(filth.lift_at(tile_middle(10.0, 10.0)), big);
        assert_eq!(filth.lift_at(tile_middle(13.0, 13.0)), big, "three out");
        assert_eq!(filth.lift_at(tile_middle(14.0, 10.0)), 0.0, "four out");
        assert_eq!(filth.lift_at(tile_middle(16.0, 4.0)), PICTURE_LIFT as f32);
        assert_eq!(filth.lift_at(tile_middle(16.0, 8.0)), 0.0);
        // The need reads the average plus the lift: a clean deck and a
        // plant is better than a clean deck.
        assert_eq!(filth.around(tile_middle(3.0, 10.0)), BASELINE);
        assert_eq!(filth.around(tile_middle(10.0, 10.0)), BASELINE + big);

        // A fouled tile under the plant is a fouled tile still, and the
        // average round it goes down exactly as it would with no plant.
        game.room.filth.foul(tile_middle(10.0, 10.0));
        let filth = &game.room.filth;
        let fouled = filth.around(tile_middle(10.0, 10.0));
        assert!(fouled < BASELINE + big && fouled > 0.0);
        let lifted = filth.around(tile_middle(10.0, 10.0)) - filth.around(tile_middle(3.0, 10.0));
        let expected = big - (BASELINE - filth.at(tile_middle(10.0, 10.0))) / 49.0;
        assert!((lifted - expected).abs() < 1e-4, "{lifted} vs {expected}");
        // Grinding is the room's average, lifted: the same fouled tile with
        // the plant's lift taken away is a worse place to stand.
        let with = filth.grinding(tile_middle(10.0, 10.0), 0.0);
        let mut bare = crate::filth::Filth::new(game.room.interior);
        bare.foul(tile_middle(10.0, 10.0));
        assert!(with >= bare.grinding(tile_middle(10.0, 10.0), 0.0));

        // Piled up, the lift stops at the cap.
        let layout = crate::aboard::layout_of(&box_ship(&[
            (PartKind::BigPlant, (10, 10)),
            (PartKind::BigPlant, (11, 10)),
            (PartKind::SmallPlant, (7, 12)),
        ]));
        let game = Game::with_layout(layout, 7, &[tile_middle(3.0, 10.0)], w, h);
        assert_eq!(game.room.filth.lift_at(tile_middle(10.0, 10.0)), LIFT_CAP);
        assert!(2.0 * big + SMALL_PLANT_LIFT as f32 > LIFT_CAP);
        assert_eq!(
            game.room.filth.lift_at(tile_middle(7.0, 12.0)),
            big + SMALL_PLANT_LIFT as f32,
            "in reach of the small one and one big one"
        );
    }

    /// In the dark a Bim sees ten tiles: a tile no light reaches is seen
    /// only from that close, however clear the line. A light on a tile is
    /// seen from across the deck, and so is everything its reach makes
    /// out — and a wall stops the light like it stops the eye, so a room
    /// behind a bulkhead is dark for all the lamp on the far side.
    #[test]
    fn the_dark_is_seen_ten_tiles_and_a_lit_tile_further() {
        use shipdesign::PartKind;
        let eye = tile_middle(3.0, 10.0);
        let mid = tile_middle(11.0, 10.0);
        let far = tile_middle(15.0, 10.0);

        // No lights at all: dark everywhere, and eight tiles is seen where
        // twelve is not.
        let layout = crate::aboard::layout_of(&box_ship(&[]));
        let (w, h) = (layout.bounds.width(), layout.bounds.height());
        let mut dark = Game::with_layout(layout, 7, &[eye], w, h);
        dark.set_autonomous(false);
        dark.simulate(DT);
        dark.observe();
        assert!(!dark.room.sight.lit_at(mid));
        assert!(dark.seen_at(mid.x, mid.y), "eight tiles, in the dark");
        assert!(!dark.seen_at(far.x, far.y), "twelve tiles, in the dark");
        assert!(dark.room.sight.sees_from(eye, mid).is_some());
        assert!(dark.room.sight.sees_from(eye, far).is_none());

        // A wall light on the far hull: the far tile is lit and seen.
        let layout = crate::aboard::layout_of(&box_ship(&[(PartKind::WallLight, (17, 10))]));
        let mut lit = Game::with_layout(layout, 7, &[eye], w, h);
        lit.set_autonomous(false);
        lit.simulate(DT);
        lit.observe();
        assert!(lit.room.sight.lit_at(far));
        assert!(lit.seen_at(far.x, far.y), "twelve tiles, lit");
        assert!(lit.room.sight.sees_from(eye, far).is_some());
        assert_eq!(lit.room.sight.lights().len(), 1);

        // A wall between the lamp and the eye keeps the light behind it:
        // the tile on the eye's side of the wall is dark again, and seen
        // only because it is within the ten.
        let mut walled = box_ship(&[(PartKind::WallLight, (17, 10))]);
        {
            use shipdesign::{Budget, Edit, Rotation, apply};
            let budget = Budget::new(10_000_000);
            for y in 2..18 {
                if let Ok(next) = apply(
                    &walled,
                    &budget,
                    Edit::Place {
                        kind: PartKind::Wall,
                        origin: (13, y),
                        rotation: Rotation::R0,
                    },
                ) {
                    walled = next;
                }
            }
        }
        let layout = crate::aboard::layout_of(&walled);
        let mut walled = Game::with_layout(layout, 7, &[eye], w, h);
        walled.set_autonomous(false);
        walled.simulate(DT);
        walled.observe();
        let before_wall = tile_middle(12.0, 10.0);
        assert!(!walled.room.sight.lit_at(before_wall), "the wall shades it");
        assert!(
            walled.seen_at(before_wall.x, before_wall.y),
            "nine tiles, dark, and still within the ten"
        );
    }

    /// A lamp is shot out: a bolt that passes within its radius stops
    /// there and takes its damage off it, a hit sets it flickering, two
    /// pistol bolts leave it failing — flickering now and then on its
    /// own — and the third puts it out: the tile it lit is dark and seen
    /// no further than the ten again, the picture round it darker, and
    /// the world's word (`set_lamp_health`) puts it back or out on a
    /// fresh room.
    #[test]
    fn a_lamp_shot_out_goes_dark_and_flickers_on_the_way() {
        use crate::sight::LAMP_HEALTH;
        use shipdesign::PartKind;
        let eye = tile_middle(3.0, 10.0);
        let far = tile_middle(15.0, 10.0);
        let layout = crate::aboard::layout_of(&box_ship(&[(PartKind::WallLight, (17, 10))]));
        let (w, h) = (layout.bounds.width(), layout.bounds.height());
        let mut game = Game::with_layout(layout, 7, &[eye], w, h);
        game.set_autonomous(false);
        game.simulate(DT);
        game.render();
        assert!(game.room.sight.lit_at(far));
        assert!(game.seen_at(far.x, far.y), "twelve tiles, lit");
        let lamp = game.lamps()[0];
        assert_eq!(lamp.health, LAMP_HEALTH);
        assert_eq!(lamp.level, 1.0);
        assert_eq!(
            game.lamp_at(tile_middle(17.0, 10.0)).map(|(i, _)| i),
            Some(0)
        );
        // The lamplight over the tile in front of it, as drawn.
        let glow_at = |game: &Game, p: Vec2| -> u8 {
            let map = game.light_map().expect("the crew's picture");
            let x = ((p.x - map.origin.x) / map.px) as usize;
            let y = ((p.y - map.origin.y) / map.px) as usize;
            map.glow[y * map.width + x]
        };
        let before = glow_at(&game, far);
        assert!(before > 0, "lamplight falls there");

        // Shot from two tiles off, until it is out. A miss goes wide of
        // the lamp; every hit is the pistol's damage off it and a moment's
        // flicker, and after two it is failing.
        let from = lamp.at - vec2(2.0 * TILE, 0.0);
        let (mut shots, mut flickered, mut hits) = (0, false, 0);
        while !game.lamps()[0].is_out() && shots < 60 {
            game.enemy_fire(from, lamp.at, WeaponKind::LaserPistol.basic(), false);
            shots += 1;
            let health = game.lamps()[0].health;
            for _ in 0..30 {
                game.simulate(DT);
                flickered |= game.lamps()[0].level < 1.0;
            }
            if game.lamps()[0].health < health {
                hits += 1;
                let lamp = game.lamps()[0];
                assert_eq!(
                    lamp.is_failing(),
                    hits == 2 && !lamp.is_out(),
                    "after {hits}"
                );
            }
        }
        assert!(game.lamps()[0].is_out(), "{shots} shots");
        assert_eq!(hits, 3, "two leave it failing, the third puts it out");
        assert!(flickered, "a hit sets it flickering");
        assert_eq!(game.take_lamp_changes(), vec![0, 0, 0]);
        assert!(game.take_lamp_changes().is_empty(), "drained");
        assert_eq!(game.lamps()[0].level, 0.0);
        // Out: the tile it lit is dark and seen no further than the ten,
        // and the picture says so.
        game.render();
        assert!(!game.room.sight.lit_at(far));
        assert!(!game.seen_at(far.x, far.y), "twelve tiles, in the dark");
        assert_eq!(glow_at(&game, far), 0, "no lamplight there now");
        for _ in 0..60 {
            game.simulate(DT);
            assert_eq!(game.lamps()[0].level, 0.0, "out is out");
        }
        // A bolt flies past a lamp that is out.
        game.enemy_fire(from, lamp.at, WeaponKind::LaserPistol.basic(), false);
        for _ in 0..30 {
            game.simulate(DT);
        }
        assert!(game.take_lamp_changes().is_empty());

        // A failing lamp flickers on its own now and then: put back to
        // failing, it dims within a few seconds with nobody shooting.
        game.set_lamp_health(0, LAMP_HEALTH * 0.1);
        assert!(game.lamps()[0].is_failing());
        game.render();
        assert!(game.room.sight.lit_at(far), "lit again");
        assert!(game.seen_at(far.x, far.y));
        assert_eq!(glow_at(&game, far), before);
        let mut dimmed = false;
        for _ in 0..(60 * 30) {
            game.simulate(DT);
            if game.lamps()[0].level < 1.0 {
                dimmed = true;
                break;
            }
        }
        assert!(dimmed, "a failing lamp flickers on its own");
        // And the flicker is the picture's alone: the tile stays lit.
        assert!(game.room.sight.lit_at(far));
        // The world's word puts it out without a flicker, for good.
        game.set_lamp_health(0, 0.0);
        assert!(game.lamps()[0].is_out());
        game.render();
        assert!(!game.room.sight.lit_at(far));
        assert_eq!(glow_at(&game, far), 0);
    }

    #[test]
    fn a_bim_knocked_out_drops_its_gun_and_a_bot_comes_back_for_it() {
        let mut game = room_with_bare_packs();
        game.set_autonomous(false);
        game.put_for_probe(1, vec2(ROOM_W * 0.35, ROOM_H * 0.5));
        game.put_for_probe(0, vec2(ROOM_W * 0.7, ROOM_H * 0.85));
        assert_eq!(game.weapon(1), Some(WeaponKind::LaserPistol.basic()));
        for _ in 0..10 {
            game.wound(1, Part::Body, 1.0);
        }
        let mut minutes = 0.0;
        while !game.is_unconscious(1) && minutes < 60.0 {
            game.simulate(1.0);
            minutes += MINUTES_PER_SECOND;
        }
        assert!(game.is_unconscious(1));
        // The gun on the deck under her, her hand empty.
        assert_eq!(game.weapon(1), None);
        assert_eq!(game.weapons_down().len(), 1);
        let d = game.weapons_down()[0];
        assert_eq!(d.weapon, WeaponKind::LaserPistol.basic());
        assert_eq!(d.owner, 1);
        assert!((d.at - game.bim_pos(1)).len() < TILE, "flung beside her");
        let at = game.bim_pos(1);
        assert_eq!(game.hit_at(at.x + 2.0, at.y), HIT_BIM, "the body first");
        assert_eq!(game.hit_at(d.at.x, d.at.y), HIT_DROPPED);
        // Out cold, nothing is aimed at her, and a bolt flies over her.
        let wounds = game.bleeding(1);
        let at = game.bim_pos(1);
        game.enemy_fire(
            at + vec2(-3.0 * TILE, 0.0),
            at,
            WeaponKind::LaserPistol.basic(),
            false,
        );
        for _ in 0..(60 * 3) {
            game.simulate(DT);
        }
        assert_eq!(game.bleeding(1), wounds, "a body out cold is not shot");
        game.take_wounds_taken();
        // Dressed, she comes round — and, a bot, goes for it: the chain,
        // and the gun back in her hand.
        assert!(game.bims[1].health.bandage(Part::Body));
        while game.is_unconscious(1) && minutes < 24.0 * 60.0 {
            game.simulate(1.0);
            minutes += MINUTES_PER_SECOND;
        }
        assert!(!game.is_unconscious(1));
        let mut steps = 0;
        while game.weapon(1).is_none() && steps < 60 * 30 {
            game.simulate(DT);
            steps += 1;
        }
        assert_eq!(
            game.weapon(1),
            Some(WeaponKind::LaserPistol.basic()),
            "picked up"
        );
        assert!(game.weapons_down().is_empty());
        // The player's own is not a bot: its gun lies where it fell until
        // the player sends it, and the click on it says what it is.
        for _ in 0..10 {
            game.wound(0, Part::Body, 1.0);
        }
        minutes = 0.0;
        while !game.is_unconscious(0) && minutes < 60.0 {
            game.simulate(1.0);
            minutes += MINUTES_PER_SECOND;
        }
        assert!(game.bims[0].health.bandage(Part::Body));
        while game.is_unconscious(0) && minutes < 24.0 * 60.0 {
            game.simulate(1.0);
            minutes += MINUTES_PER_SECOND;
        }
        assert!(!game.is_unconscious(0));
        for _ in 0..(60 * 5) {
            game.simulate(DT);
        }
        assert_eq!(game.weapon(0), None, "the player's waits to be told");
        assert_eq!(game.weapons_down().len(), 1);
        let d = game.weapons_down()[0];
        // Somewhere off the body's own tile, with a way back to it: the
        // walk has to move. Which way is free depends on where the gun
        // fell — a flung thing is a roll, and a roll moves whenever
        // anything else in the room does — so take the first side that
        // works rather than assuming east.
        let mut stand = None;
        for off in [
            vec2(4.0 * TILE, 0.0),
            vec2(-4.0 * TILE, 0.0),
            vec2(0.0, 4.0 * TILE),
            vec2(0.0, -4.0 * TILE),
        ] {
            let p = game.put_for_probe(0, d.at + off);
            if (p - d.at).len() > TILE && game.can_reach_for_probe(0, d.at) {
                stand = Some(p);
                break;
            }
        }
        let stand = stand.expect("somewhere to stand a walk away from the gun");
        assert!((stand - d.at).len() > TILE);
        assert_eq!(game.hit_at(d.at.x + 3.0, d.at.y - 2.0), HIT_DROPPED);
        assert_eq!(game.hit_dropped(), d.id);
        // The hover asks the same reach and notes nothing.
        assert_eq!(game.dropped_at(d.at.x + 3.0, d.at.y - 2.0), Some(d.id));
        assert_eq!(game.dropped_at(d.at.x + 3.0 * TILE, d.at.y), None);
        assert!(game.fetch(0, d.id));
        assert_eq!(game.activity(0), JOB_FETCH);
        let mut steps = 0;
        while game.weapons_down().len() == 1 && steps < 60 * 30 {
            game.simulate(DT);
            steps += 1;
        }
        // The player's own puts it in the pack — the right-click is "into
        // the inventory" — and the hand stays as the player left it.
        assert!(game.weapons_down().is_empty());
        assert_eq!(game.weapon(0), None, "not in the hand");
        assert_eq!(
            game.pack(0)[0],
            Some(Item::Weapon(WeaponKind::LaserPistol.basic()))
        );
        assert!(!game.fetch(0, d.id), "nothing there any more");
    }

    #[test]
    fn a_treatment_walks_to_the_kit_first_and_carries_it_to_the_patient() {
        let mut game = room();
        game.set_autonomous(false);
        let james = game.put_for_probe(0, vec2(ROOM_W * 0.55, ROOM_H * 0.5));
        game.put_for_probe(1, vec2(ROOM_W * 0.35, ROOM_H * 0.5));
        // The kits live in a cabinet across the room, the world's word.
        let cabinet = vec2(ROOM_W * 0.75, ROOM_H * 0.5);
        game.set_kit_stands(&[cabinet]);
        let had = game.medkits();
        assert!(had > 0);
        assert!(game.wound(1, Part::Body, Part::Body.max()).trauma.is_some());
        assert!(game.treat(0, 1, Part::Body));
        assert_eq!(game.activity(0), JOB_TREAT);
        // First to the cabinet, empty-handed.
        let to = game.destination_for_probe(0).expect("walking");
        assert!(
            (to - cabinet).len() < 2.0 * TILE,
            "to the cabinet first: {to:?}"
        );
        assert!((to - james).len() > 3.0 * TILE);
        assert_ne!(game.bims[0].character.main_held(), Held::Medkit);
        // The kit in hand as it leaves the cabinet, and one off the shelf.
        let mut steps = 0;
        while game.bims[0].character.main_held() != Held::Medkit && steps < 60 * 60 {
            game.simulate(DT);
            steps += 1;
        }
        assert_eq!(
            game.bims[0].character.main_held(),
            Held::Medkit,
            "the kit in hand"
        );
        assert_eq!(game.medkits(), had - 1, "off the shelf");
        assert!(
            (game.bim_pos(0) - cabinet).len() < 2.0 * TILE,
            "taken at the cabinet"
        );
        // A world setting the shelf again does not count the one in hand.
        game.set_medkits(had);
        assert_eq!(game.medkits(), had - 1);
        // Then to the patient, and the kit is spent on her.
        while game.is_dying(1) && steps < 60 * 120 {
            game.simulate(DT);
            steps += 1;
        }
        assert!(!game.is_dying(1), "treated");
        assert_eq!(game.bims[0].character.main_held(), Held::Nothing, "spent");
        assert_eq!(game.take_medkits_used(), 1);
        assert_eq!(game.medkits(), had - 1);

        // A treatment given up on the way puts the kit back.
        game.set_kit_stands(&[]);
        assert!(game.wound(1, Part::Legs, Part::Legs.max()).trauma.is_some());
        let shelf = game.medkits();
        assert!(game.treat(0, 1, Part::Legs));
        for _ in 0..60 {
            game.simulate(DT);
        }
        assert_eq!(
            game.bims[0].character.main_held(),
            Held::Medkit,
            "on the spot, no cabinet"
        );
        assert_eq!(game.medkits(), shelf - 1);
        game.abandon_for_probe(0);
        assert_eq!(game.bims[0].character.main_held(), Held::Nothing);
        assert_eq!(game.medkits(), shelf, "back on the shelf");
    }

    /// A crewmate is doctored where it lies out of harm: nobody goes over
    /// while an enemy is near it, hidden or not, or in sight of it; but a
    /// shot a moment ago somewhere else, or a sighting that has ended, does
    /// not hold a helper back the way it holds back the room's calm. In a
    /// fight one helper goes to a patient; in the calm, one a part. The
    /// helper's own wound is not waited for.
    #[test]
    fn under_the_alarm_a_crewmate_doctors_with_nothing_in_sight_where_the_patient_lies_out_of_harm()
    {
        // --- under_the_alarm_a_crewmate_with_nothing_in_sight_doctors_and_one_with_an_enemy_in_sight_does_not ---
        {
            let mut game = room();
            game.set_autonomous(true);
            let james = game.put_for_probe(0, vec2(ROOM_W * 0.3, ROOM_H * 0.5));
            game.put_for_probe(1, vec2(ROOM_W * 0.5, ROOM_H * 0.5));
            assert_eq!(
                game.priorities.of(Job::Medical),
                work::HIGHEST,
                "the default"
            );
            // James under the player's orders, so he does not dress himself —
            // a recruited player's Bim waits to be told — and Kate has to.
            game.recruit_for_probe(0, true);
            // An enemy within range but out of sight — beyond the room's walls,
            // twenty-five tiles off — is the alarm without a target to act on;
            // James bleeds.
            let unseen = james + vec2((ALARM_RANGE - 5.0) * TILE, 0.0);
            game.set_hostiles(vec![Some((unseen, WeaponKind::LaserPistol.basic()))]);
            assert!(!game.wound(0, Part::Body, 4.0).leg_lost);
            for _ in 0..30 {
                game.simulate(DT);
            }
            assert!(game.is_alarmed());
            assert!(game.bims[1].character.is_recruited(), "Kate under arms");
            assert_eq!(
                game.activity(1),
                JOB_BANDAGE,
                "and doctoring, nothing in sight"
            );
            assert!(!game.is_armed(1), "holstered for it");
            let mut steps = 0;
            while game.bleeding(0) > 0 && steps < 60 * 60 {
                game.simulate(DT);
                steps += 1;
            }
            assert_eq!(game.bleeding(0), 0, "James dressed under the alarm");

            // An enemy in plain view: the wound waits, the gun does not.
            assert!(!game.wound(0, Part::Body, 4.0).leg_lost);
            let seen = james + vec2(5.0 * TILE, 0.0);
            game.set_hostiles(vec![Some((seen, WeaponKind::LaserPistol.basic()))]);
            for _ in 0..30 {
                game.simulate(DT);
            }
            assert_ne!(game.activity(1), JOB_BANDAGE, "not with an enemy in sight");
            assert!(game.is_armed(1));
        }

        // --- a_crewmate_is_doctored_where_it_lies_out_of_harm_whatever_the_room_s_clocks_say ---
        {
            let mut game = room();
            game.set_autonomous(true);
            assert!(game.calm(), "a fresh room");
            let james = game.put_for_probe(0, vec2(ROOM_W * 0.3, ROOM_H * 0.5));
            game.put_for_probe(1, vec2(ROOM_W * 0.5, ROOM_H * 0.5));
            // Nobody armed, so no shot of theirs starts the lull over; James
            // under the player's orders, so he waits to be told and Kate has
            // to dress him.
            game.issue(0, Gear::default());
            game.issue(1, Gear::default());
            // A fresh gear has an empty pack, and a dressing comes out
            // of the binder's own pack (feature 87).
            game.set_bandages_for_probe(1, room::BANDAGES_AT_DAWN);
            game.recruit_for_probe(0, true);
            let pistol = WeaponKind::LaserPistol.basic();
            let quiet_steps = (CALM_AFTER / DT) as usize + 5;

            // An enemy in the heads: out of everybody's sight, but a few
            // tiles from James — the room is smaller than twenty tiles
            // across. He does not lie out of harm, and nobody goes to him.
            let hidden = game.room.spot_rects(room::SPOT_TOILET)[0].center();
            assert!((hidden - james).len() <= RESCUE_CLEAR * TILE);
            assert!(!game.wound(0, Part::Body, 4.0).leg_lost);
            game.set_hostiles(vec![Some((hidden, pistol))]);
            for _ in 0..quiet_steps {
                game.simulate(DT);
                assert!(!game.sees_for_probe(1, hidden), "hidden from Kate");
                assert!(!game.calm(), "an enemy within twenty tiles");
                assert_ne!(game.activity(1), JOB_BANDAGE, "not with one so near");
            }
            assert!(game.is_alarmed());
            assert!(game.bleeding(0) > 0, "James still bleeds");

            // The enemy off beyond the walls, out of reach — and a shot just
            // fired in here. The room is not calm, but James lies out of
            // harm, and she goes to him at once rather than twenty seconds
            // after the last shot: a fight in waves is never twenty seconds
            // quiet, and the wounded waited for its end.
            let far = james + vec2(30.0 * TILE, 0.0);
            game.set_hostiles(vec![Some((far, pistol))]);
            let kate = game.bim_pos(1);
            game.enemy_fire(
                kate + vec2(0.0, 2.0 * TILE),
                kate + vec2(0.0, 3.0 * TILE),
                pistol,
                false,
            );
            let mut steps = 0;
            while game.activity(1) != JOB_BANDAGE && steps < 60 {
                game.simulate(DT);
                steps += 1;
            }
            assert!(!game.calm(), "a shot a moment ago");
            assert_eq!(
                game.activity(1),
                JOB_BANDAGE,
                "and she doctors all the same"
            );
            while game.bleeding(0) > 0 && steps < 60 * 60 {
                game.simulate(DT);
                steps += 1;
            }
            assert_eq!(game.bleeding(0), 0, "James dressed");

            // In plain view of both of them: she does not go, the gun being
            // what she would want in her hands (had she one) and he being
            // anything but out of harm. Gone beyond the walls again, and she
            // goes at once — the sighting a moment ago holds the room's calm
            // back, not her.
            assert!(!game.wound(0, Part::Body, 4.0).leg_lost);
            let seen = james + vec2(5.0 * TILE, 0.0);
            game.set_hostiles(vec![Some((seen, pistol))]);
            for _ in 0..30 {
                game.simulate(DT);
                assert_ne!(game.activity(1), JOB_BANDAGE, "not with an enemy in sight");
            }
            game.set_hostiles(vec![Some((far, pistol))]);
            let mut steps = 0;
            while game.activity(1) != JOB_BANDAGE && steps < 60 {
                game.simulate(DT);
                steps += 1;
            }
            assert!(!game.calm(), "seen a moment ago");
            assert_eq!(game.activity(1), JOB_BANDAGE, "and she doctors again");

            // Her own wound she dresses whatever the clocks say: the enemy
            // back in the heads, a shot just fired, and she binds herself.
            let mut steps = 0;
            while game.bleeding(0) > 0 && steps < 60 * 60 {
                game.simulate(DT);
                steps += 1;
            }
            assert!(!game.wound(1, Part::Body, 4.0).leg_lost);
            game.set_hostiles(vec![Some((hidden, pistol))]);
            game.enemy_fire(
                kate + vec2(0.0, 2.0 * TILE),
                kate + vec2(0.0, 3.0 * TILE),
                pistol,
                false,
            );
            let mut steps = 0;
            while game.activity(1) != JOB_BANDAGE && steps < 60 * 10 {
                game.simulate(DT);
                steps += 1;
            }
            assert!(!game.calm());
            assert_eq!(game.activity(1), JOB_BANDAGE, "her own, on the spot");
        }

        // --- in_a_fight_one_helper_goes_to_a_patient_and_in_the_calm_one_a_part ---
        {
            let mut game = room();
            let more = room().take_crew();
            game.adopt(more, Vec2::ZERO);
            assert_eq!(game.crew_count(), 4);
            game.set_autonomous(true);
            let james = game.put_for_probe(0, vec2(ROOM_W * 0.3, ROOM_H * 0.5));
            for who in 1..4 {
                game.put_for_probe(who, vec2(ROOM_W * (0.4 + 0.1 * who as f32), ROOM_H * 0.5));
                game.issue(who, Gear::default());
                game.set_bandages_for_probe(who, room::BANDAGES_AT_DAWN);
            }
            game.issue(0, Gear::default());
            game.recruit_for_probe(0, true);
            // Three parts of James bleeding, and an enemy far off beyond
            // the walls with a shot just fired: a fight, and James out of
            // harm in it.
            for part in Part::ALL {
                assert!(!game.wound(0, part, 1.0).leg_lost);
            }
            let pistol = WeaponKind::LaserPistol.basic();
            game.set_hostiles(vec![Some((james + vec2(30.0 * TILE, 0.0), pistol))]);
            game.enemy_fire(
                james + vec2(0.0, 2.0 * TILE),
                james + vec2(0.0, 3.0 * TILE),
                pistol,
                false,
            );
            let helpers = |game: &Game| (1..4).filter(|&w| game.activity(w) == JOB_BANDAGE).count();
            for _ in 0..30 {
                game.simulate(DT);
                assert!(helpers(&game) <= 1, "one helper to a patient in a fight");
            }
            assert!(!game.calm());
            assert_eq!(helpers(&game), 1, "and one goes");
            // The fight over and the room calm, three fresh wounds: a part
            // apiece again.
            game.set_hostiles(Vec::new());
            let mut steps = 0;
            while !game.calm() && steps < 60 * 60 {
                game.simulate(DT);
                steps += 1;
            }
            assert!(game.calm());
            for part in Part::ALL {
                assert!(!game.wound(0, part, 1.0).leg_lost);
            }
            let mut most = 0;
            for _ in 0..60 {
                game.simulate(DT);
                most = most.max(helpers(&game));
            }
            assert!(most > 1, "in the calm a part apiece: {most}");
        }
    }

    /// A hostile room's people have eyes on the airlock whatever they are
    /// doing: a target within `AIRLOCK_WATCH` tiles of the watched spot is
    /// seen with nobody looking, and the hunt is on; one further off is
    /// not. Nobody shoots at it without real sight, as ever.
    #[test]
    fn a_hostile_room_s_airlock_is_watched_and_a_boarder_at_it_is_seen_by_nobody_in_particular() {
        let mut game = room();
        game.set_autonomous(false);
        game.put_for_probe(1, vec2(ROOM_W * 0.35, ROOM_H * 0.5));
        game.put_for_probe(0, vec2(ROOM_W * 0.45, ROOM_H * 0.8));
        game.set_hostile_bodies(true);
        let pistol = WeaponKind::LaserPistol.basic();
        // A target in the heads is nobody to them: out of every eye.
        let hidden = game.room.spot_rects(room::SPOT_TOILET)[0].center();
        game.set_hostiles(vec![Some((hidden, pistol))]);
        game.simulate(DT);
        assert_eq!(game.believed_for_probe(), vec![None]);
        assert!(!game.bims[1].character.is_recruited(), "nothing to hunt");
        // Eyes on a spot three tiles from it: still nobody.
        game.set_watched(Some(hidden + vec2((AIRLOCK_WATCH + 0.5) * TILE, 0.0)));
        game.set_hostiles(vec![Some((hidden, pistol))]);
        game.simulate(DT);
        assert_eq!(game.believed_for_probe(), vec![None], "beyond the watch");
        // Eyes on the spot a tile from it: seen where it stands, and the
        // room at war — but nobody has a shot at it.
        game.set_watched(Some(hidden + vec2(TILE, 0.0)));
        game.take_shots();
        let mut shots = 0;
        for _ in 0..30 {
            game.set_hostiles(vec![Some((hidden, pistol))]);
            game.simulate(DT);
            shots += game.take_shots().len();
        }
        assert_eq!(
            game.believed_for_probe(),
            vec![Some(hidden)],
            "seen at the door"
        );
        assert_eq!(game.combat_targets_for_probe(), vec![Some(hidden)]);
        assert!(game.bims[1].character.is_recruited(), "at war");
        assert_eq!(shots, 0, "a shot still wants sight");
        assert!(!game.calm(), "an enemy in sight, if only the airlock's");
        // The watch off — the ship gone — and the belief ages out like
        // any other.
        game.set_watched(None);
        let steps = (FORGET_AFTER / DT) as usize + 5;
        for _ in 0..steps {
            game.set_hostiles(vec![Some((hidden, pistol))]);
            game.simulate(DT);
        }
        assert_eq!(game.believed_for_probe(), vec![None]);
        assert!(!game.bims[1].character.is_recruited(), "stood down");
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
        assert!(game.blood(0) < MAX_BLOOD * crate::health::OUT_AT);
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

        // The second shot that empties the legs is a dying state for them
        // — which one is the combat stream's roll — and not a death.
        assert_eq!(game.wound(1, Part::Legs, 12.0).trauma, None);
        let out = game.wound(1, Part::Legs, 12.0);
        let trauma = out.trauma.expect("a trauma");
        assert_eq!(trauma.part(), Part::Legs);
        assert_eq!(out.leg_lost, trauma.loses_leg());
        assert!(game.is_dying(1));
        assert_eq!(game.trauma(1, Part::Legs), Some(trauma));
        assert_eq!(game.take_traumas(), vec![(1, trauma)]);
        game.simulate(DT);
        assert!(game.is_alive(1));
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

    /// Feature 87: a dressing is a thing in a pack, five to a box, and
    /// it is the **binder's own** that is spent. *Bandage all wounds*
    /// dresses the worst part now and queues the rest behind it; and a
    /// Bim running from a fight reaches for it itself the moment nothing
    /// can see it.
    #[test]
    fn a_dressing_comes_out_of_the_pack_and_a_bim_out_of_sight_binds_every_wound() {
        // --- one box, five dressings, and the pack is what is spent ---
        {
            let mut game = room_with_bare_packs();
            game.set_autonomous(false);
            game.put_for_probe(0, vec2(ROOM_W * 0.45, ROOM_H * 0.5));
            assert_eq!(game.bandages_of(0), 0, "nothing to bind with");
            assert!(!game.wound(0, Part::Body, 4.0).leg_lost);
            assert!(!game.bandage(0, 0, Part::Body), "and so nothing is bound");
            // Seven of them: a full box and a part one, two cells.
            assert_eq!(game.give_stack(0, BANDAGE, 7), 7);
            assert_eq!(game.bandages_of(0), 7);
            let boxes = game.pack(0).iter().filter(|c| **c == Some(BANDAGE)).count();
            assert_eq!(boxes, 2, "five to a box");
            assert!(game.bandage(0, 0, Part::Body));
            let mut steps = 0;
            while game.bleeding(0) > 0 && steps < 60 * 30 {
                game.simulate(DT);
                steps += 1;
            }
            assert_eq!(game.bleeding(0), 0, "bound");
            assert_eq!(game.bandages_of(0), 6, "one dressing out of the pack");
            // The emptiest box first, so the pack tidies itself.
            assert_eq!(
                game.pack(0).iter().filter(|c| **c == Some(BANDAGE)).count(),
                2
            );
        }

        // --- bandage all wounds: the worst now, the rest queued ---
        {
            let mut game = room();
            game.set_autonomous(false);
            game.put_for_probe(0, vec2(ROOM_W * 0.45, ROOM_H * 0.5));
            game.set_bandages_for_probe(0, 5);
            assert!(!game.wound(0, Part::Head, 2.0).leg_lost);
            assert!(!game.wound(0, Part::Body, 4.0).leg_lost);
            assert!(!game.wound(0, Part::Legs, 3.0).leg_lost);
            assert_eq!(game.bleeding(0), 3, "three parts open");
            assert!(game.bandage_all(0, 0));
            assert_eq!(game.activity(0), JOB_BANDAGE);
            assert_eq!(game.ordered_count(0), 2, "the other two wait their turn");
            let mut steps = 0;
            while game.bleeding(0) > 0 && steps < 60 * 90 {
                game.simulate(DT);
                steps += 1;
            }
            assert_eq!(game.bleeding(0), 0, "every wound closed");
            assert_eq!(game.bandages_of(0), 2, "three dressings spent");
            // Nothing open: the row does nothing at all.
            assert!(!game.bandage_all(0, 0));
        }

        // --- out of the enemy's sight, a runner binds its own ---
        {
            let mut game = room();
            game.set_autonomous(false);
            let james = game.put_for_probe(0, vec2(ROOM_W * 0.3, ROOM_H * 0.5));
            game.put_for_probe(1, vec2(ROOM_W * 0.5, ROOM_H * 0.5));
            game.set_bandages_for_probe(1, 5);
            // Kate shot to a dying state, with wounds open on two parts,
            // and an enemy beyond the walls: out of her sight, so she
            // binds where she stands.
            assert!(game.wound(1, Part::Body, Part::Body.max()).trauma.is_some());
            assert!(!game.wound(1, Part::Legs, 3.0).leg_lost);
            game.take_hits();
            let pistol = WeaponKind::LaserPistol.basic();
            let far = james + vec2(25.0 * TILE, 0.0);
            game.set_hostiles(vec![Some((far, pistol))]);
            for _ in 0..10 {
                game.simulate(DT);
            }
            assert!(game.is_dying(1) && game.is_fleeing(1), "she runs");
            let mut steps = 0;
            while game.bleeding(1) > 0 && steps < 60 * 90 {
                game.simulate(DT);
                steps += 1;
            }
            assert_eq!(game.bleeding(1), 0, "she bound every wound herself");
            assert!(game.bandages_of(1) < 5, "out of her own pack");
            assert!(game.is_dying(1), "and still wants a kit");
        }
    }

    #[test]
    fn a_bandage_is_walked_over_closes_one_part_and_holsters_the_weapon_while_wound() {
        // --- a_bandage_is_walked_over_and_closes_the_wounds_on_one_part ---
        {
            let mut game = room();
            game.set_autonomous(false);
            // Kate stands still — recruited, so she does not wander off while
            // James walks over — a few tiles from him.
            game.put_for_probe(0, vec2(ROOM_W * 0.55, ROOM_H * 0.5));
            game.put_for_probe(1, vec2(ROOM_W * 0.35, ROOM_H * 0.5));
            game.recruit_for_probe(1, true);
            let had = game.bandages_of(0);
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
            assert_eq!(game.bandages_of(0), had - 1);
            assert!(game.is_alive(1));
            // Beside the patient, not on it.
            let apart = (game.bim_pos(0) - game.bim_pos(1)).len();
            assert!(apart <= 2.0 * TILE, "{apart}");
            assert_eq!(game.activity(0), 0, "the errand is over");

            // Its own wounds, on the spot, and the count runs out.
            game.set_bandages_for_probe(0, 1);
            game.wound(0, Part::Legs, 3.0);
            assert!(game.bandage(0, 0, Part::Legs));
            let mut steps = 0;
            while game.bleeding(0) > 0 && steps < 60 * 30 {
                game.simulate(DT);
                steps += 1;
            }
            assert_eq!(game.bleeding(0), 0);
            assert_eq!(game.bandages_of(0), 0);
            game.wound(0, Part::Legs, 3.0);
            assert!(!game.bandage(0, 0, Part::Legs), "none left");
        }

        // --- winding_a_bandage_holsters_the_weapon_and_it_is_drawn_again_after ---
        {
            let mut game = room();
            game.set_autonomous(false);
            let james = game.put_for_probe(0, vec2(ROOM_W * 0.45, ROOM_H * 0.5));
            game.put_for_probe(1, vec2(ROOM_W * 0.2, ROOM_H * 0.85));
            // Kate unarmed, so the only bolts are his.
            game.issue(1, Gear::default());
            game.recruit_for_probe(0, true);
            // An enemy four tiles off in plain view: James shoots at it.
            let target = james + vec2(4.0 * TILE, 0.0);
            game.set_hostiles(vec![Some((target, WeaponKind::LaserPistol.basic()))]);
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
    fn a_hostile_room_chases_what_it_last_saw_and_forgets_it_after_a_minute() {
        // Kate armed in a hostile room, James out of it; a target in plain
        // view is known where it stands.
        let mut game = room();
        game.set_autonomous(false);
        let kate = game.put_for_probe(1, vec2(ROOM_W * 0.35, ROOM_H * 0.5));
        game.put_for_probe(0, vec2(ROOM_W * 0.45, ROOM_H * 0.8));
        game.issue(0, Gear::default());
        game.set_hostile_bodies(true);
        let seen_at = kate + vec2(4.0 * TILE, 0.0);
        game.set_hostiles(vec![Some((seen_at, WeaponKind::LaserPistol.basic()))]);
        game.simulate(DT);
        assert_eq!(game.believed_for_probe(), vec![Some(seen_at)]);
        assert_eq!(game.combat_targets_for_probe(), vec![Some(seen_at)]);
        assert!(game.bims[1].character.is_recruited(), "at war");

        // The target steps behind the heads' walls: nobody sees it there,
        // so it is believed where it was — and the tactics go there, while
        // nothing is fired at the empty spot.
        let hidden = game.room.spot_rects(room::SPOT_TOILET)[0].center();
        game.set_hostiles(vec![Some((hidden, WeaponKind::LaserPistol.basic()))]);
        game.take_shots();
        let mut shots = 0;
        for _ in 0..60 {
            game.simulate(DT);
            game.set_hostiles(vec![Some((hidden, WeaponKind::LaserPistol.basic()))]);
            shots += game.take_shots().len();
        }
        assert_eq!(
            game.believed_for_probe(),
            vec![Some(seen_at)],
            "believed where last seen"
        );
        assert_eq!(game.combat_targets_for_probe(), vec![Some(seen_at)]);
        assert_eq!(shots, 0, "nobody shoots at a belief");
        assert!(game.bims[1].character.is_recruited(), "still hunting");
        assert!(
            game.destination_for_probe(1)
                .is_some_and(|d| (d - seen_at).len() < 4.0 * TILE)
                || (game.bim_pos(1) - seen_at).len() < 4.0 * TILE,
            "walking to where it was last seen"
        );

        // A minute unseen and the belief is dropped: nothing to be at war
        // over, and she goes back to her day.
        let steps = (FORGET_AFTER / DT) as usize + 5;
        for _ in 0..steps {
            game.simulate(DT);
            game.set_hostiles(vec![Some((hidden, WeaponKind::LaserPistol.basic()))]);
        }
        assert_eq!(game.believed_for_probe(), vec![None]);
        assert_eq!(game.combat_targets_for_probe(), vec![None]);
        assert!(!game.bims[1].character.is_recruited(), "stood down");

        // Seen again — the target walks out — and the hunt is on again.
        game.set_hostiles(vec![Some((seen_at, WeaponKind::LaserPistol.basic()))]);
        game.simulate(DT);
        assert_eq!(game.believed_for_probe(), vec![Some(seen_at)]);
        assert!(game.bims[1].character.is_recruited());
        // The crew's own room takes the list as it comes: no belief.
        game.set_hostile_bodies(false);
        game.set_hostiles(vec![Some((hidden, WeaponKind::LaserPistol.basic()))]);
        assert_eq!(game.combat_targets_for_probe(), vec![Some(hidden)]);
        assert!(game.believed_for_probe().is_empty());
    }

    #[test]
    fn a_bot_with_a_shot_stands_still_and_the_player_s_bim_fires_on_the_move_at_half_the_odds() {
        // Kate armed in a hostile room, a target in plain view a few
        // tiles off in an open room: the stand the tactics would walk to
        // is no cover, so she stays put and shoots from where she is —
        // walking would halve her odds.
        let mut game = room();
        game.set_autonomous(false);
        let kate = game.put_for_probe(1, vec2(ROOM_W * 0.35, ROOM_H * 0.5));
        game.put_for_probe(0, vec2(ROOM_W * 0.45, ROOM_H * 0.8));
        game.issue(0, Gear::default());
        game.set_hostile_bodies(true);
        let target = kate + vec2(3.0 * TILE, 0.0);
        game.set_hostiles(vec![Some((target, WeaponKind::LaserPistol.basic()))]);
        let mut shots = 0;
        let mut walked = false;
        for _ in 0..(60 * 4) {
            game.simulate(DT);
            shots += game.take_shots().len();
            walked |= game.is_walking(1);
        }
        assert!(shots > 0, "she shoots");
        assert!(!walked, "and never walks for an open stand");
        assert!((game.bim_pos(1) - kate).len() < TILE, "stood where she was");

        // The player's own Bim, recruited and sent across the room past
        // a target it can see, fires on the move — and every one of those
        // shots is at half the odds.
        let mut game = room();
        game.set_autonomous(false);
        let james = game.put_for_probe(0, vec2(ROOM_W * 0.3, ROOM_H * 0.5));
        game.put_for_probe(1, vec2(ROOM_W * 0.2, ROOM_H * 0.85));
        // Kate unarmed: under the alarm she would fire at the target too.
        game.issue(1, Gear::default());
        game.recruit_for_probe(0, true);
        game.set_hostiles(vec![Some((
            james + vec2(3.0 * TILE, 0.0),
            WeaponKind::LaserPistol.basic(),
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
        assert!(fired_walking, "a Bim on the move still fires");

        // The odds themselves, off the combat stream: from two tiles the
        // pistol lands about 85 in 100 standing and about half that
        // walking.
        let landed = |moving: bool| {
            let mut combat = crate::combat::Combat::new(3);
            let hall = Rect::from_min_size(Vec2::ZERO, vec2(20.0 * TILE, 10.0 * TILE));
            let sight = crate::sight::Sight::new(hall, hall, TILE, &[], &[]);
            let theirs = vec2(10.0 * TILE, 5.0 * TILE);
            combat.set_targets(vec![Some((theirs, WeaponKind::LaserPistol.basic()))]);
            let mut hits = 0;
            for _ in 0..400 {
                combat.fire(
                    theirs - vec2(2.0 * TILE, 0.0),
                    theirs,
                    WeaponKind::LaserPistol.basic(),
                    false,
                    moving,
                );
                for _ in 0..40 {
                    combat.step(0.05, &sight, &[]);
                }
                hits += combat.take_hits().len();
            }
            hits
        };
        let (still, walking) = (landed(false), landed(true));
        assert!(still > 300 && still < 380, "{still} of 400 standing");
        assert!(walking > 130 && walking < 210, "{walking} of 400 walking");
    }

    /// Feature 84: the shot leaves the **gun**, and the gun is pointed
    /// down the shot — the two are one line, which is what the muzzle
    /// being the origin buys.
    #[test]
    fn a_shot_leaves_the_muzzle_and_the_barrel_points_down_it() {
        let mut game = room();
        game.set_autonomous(false);
        let james = game.put_for_probe(0, vec2(ROOM_W * 0.3, ROOM_H * 0.5));
        game.put_for_probe(1, vec2(ROOM_W * 0.2, ROOM_H * 0.85));
        // The rifle, whose barrel is long enough that a shot out of the
        // chest and a shot out of the muzzle are a tile apart; Kate
        // unarmed, or she fires at the target too.
        game.issue(
            0,
            Gear {
                weapon: Some(WeaponKind::AutoRifle.basic()),
                ..Gear::issued()
            },
        );
        game.issue(1, Gear::default());
        game.recruit_for_probe(0, true);
        let target = james + vec2(5.0 * TILE, 0.0);
        game.set_hostiles(vec![Some((target, WeaponKind::LaserPistol.basic()))]);
        // A second and a half to square up to it first — a body turns at
        // its own rate, and the first shot goes off while it is still
        // coming round.
        for _ in 0..90 {
            game.simulate(DT);
        }
        let mut fired_from = None;
        for _ in 0..(60 * 4) {
            let before = game.bolts_in_flight();
            game.simulate(DT);
            if game.bolts_in_flight() > before {
                fired_from = game.combat.bolts.last().map(|b| b.fired_from);
                break;
            }
        }
        let from = fired_from.expect("he fires");
        let body = game.bim_pos(0);
        // Out in front of him, about a tile — the grip ahead of the
        // chest and the barrel ahead of that — and never across the room.
        let ahead = (from - body).len();
        assert!(ahead > TILE * 0.8, "the shot leaves the gun: {ahead} off");
        assert!(ahead < TILE * 1.6, "and the gun is in his hands: {ahead}");
        // And on the line to what he shot at, bar the grip's own offset
        // towards the firing shoulder: the barrel is pointed down it.
        let along = (target - body).normalize_or_zero();
        let off = (from - body).dot(along.perp()).abs();
        assert!(off < 20.0, "the barrel points at it: {off} off the line");
        // The origin *is* the muzzle of the gun the picture draws.
        let muzzle = game.bims[0].character.muzzle().expect("a gun is up");
        assert!((muzzle - from).len() < 8.0, "fired from the muzzle");
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
                weapon: Some(WeaponKind::AutoRifle.basic()),
                ..Gear::default()
            },
        );
        game.issue(0, Gear::default());
        assert_eq!(game.weapon(1), Some(WeaponKind::AutoRifle.basic()));
        game.set_hostile_bodies(true);
        let target = kate + vec2(4.0 * TILE, 0.0);
        game.set_hostiles(vec![Some((target, WeaponKind::LaserPistol.basic()))]);
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
    fn a_blade_charges_locks_the_gunner_and_its_blow_is_a_cut_that_splashes_the_deck() {
        // --- a_blade_within_reach_locks_the_gunner_who_stops_firing_and_lands_fists ---
        {
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
            game.set_hostiles(vec![Some((beside, WeaponKind::LaserPistol.basic()))]);
            for _ in 0..30 {
                game.simulate(DT);
            }
            assert_eq!(game.is_locked(0), None);
            assert!(
                game.bolts_in_flight() > 0 || !game.take_hits().is_empty(),
                "shooting"
            );
            game.set_hostiles(vec![Some((beside, WeaponKind::Schword.basic()))]);
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
                WeaponKind::Schword.basic(),
            ))]);
            game.simulate(DT);
            assert_eq!(game.is_locked(0), None);
            for _ in 0..60 {
                game.simulate(DT);
            }
            assert!(game.bolts_in_flight() > 0 || !game.take_hits().is_empty());
        }

        // --- a_blade_enemy_charges_and_its_blow_is_a_cut_that_splashes_the_deck ---
        {
            let mut game = room();
            game.set_autonomous(false);
            let kate = game.put_for_probe(1, vec2(ROOM_W * 0.30, ROOM_H * 0.5));
            game.put_for_probe(0, vec2(ROOM_W * 0.45, ROOM_H * 0.85));
            game.issue(
                1,
                Gear {
                    weapon: Some(WeaponKind::Schword.basic()),
                    ..Gear::default()
                },
            );
            game.issue(0, Gear::default());
            game.set_hostile_bodies(true);
            // A target five tiles off in the open: a blade goes to it.
            let target = kate + vec2(5.0 * TILE, 0.0);
            game.set_hostiles(vec![Some((target, WeaponKind::LaserPistol.basic()))]);
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
                    .all(|s| s.melee && s.cut && s.damage == 42.0 && s.at == target)
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
    }

    #[test]
    fn bodies_under_arms_are_pushed_apart_and_in_peace_they_are_not() {
        // Two of a hostile room's people put on one spot at war: a step
        // later they stand a body's clearance apart, and the pair keep
        // that clearance while the fight is on.
        let mut game = room();
        game.set_autonomous(false);
        let spot = vec2(ROOM_W * 0.4, ROOM_H * 0.5);
        game.put_for_probe(0, spot);
        game.put_for_probe(1, spot + vec2(3.0, 0.0));
        game.set_hostile_bodies(true);
        game.set_hostiles(vec![Some((
            spot + vec2(6.0 * TILE, 0.0),
            WeaponKind::LaserPistol.basic(),
        ))]);
        for _ in 0..30 {
            game.simulate(DT);
        }
        assert!(game.bims[0].character.is_recruited() && game.bims[1].character.is_recruited());
        let gap = (game.bim_pos(0) - game.bim_pos(1)).len();
        assert!(gap >= CREW_CLEARANCE - 1.0, "apart: {gap}");

        // The same two at peace pass through each other: put on one spot
        // with nobody recruited, they are left where they are.
        let mut game = room();
        game.set_autonomous(false);
        game.put_for_probe(0, spot);
        game.put_for_probe(1, spot + vec2(3.0, 0.0));
        for _ in 0..30 {
            game.simulate(DT);
        }
        let gap = (game.bim_pos(0) - game.bim_pos(1)).len();
        assert!(gap < CREW_CLEARANCE, "not pushed: {gap}");
    }

    #[test]
    fn a_crewmate_with_a_blade_puts_the_armour_in_its_pack_on_at_the_alarm() {
        let mut game = room();
        game.set_autonomous(false);
        let kate = game.put_for_probe(1, vec2(ROOM_W * 0.35, ROOM_H * 0.5));
        game.put_for_probe(0, vec2(ROOM_W * 0.7, ROOM_H * 0.85));
        // The schword and a helm and leg guards in the pack, a broken vest
        // on the body: the alarm draws the blade and puts on what is whole,
        // and leaves the broken vest, since there is nothing to replace it.
        let mut gear = Gear::default();
        gear.pack[0] = Some(Item::Weapon(WeaponKind::Schword.basic()));
        gear.pack[1] = Some(Item::Armour(Piece::new(
            1,
            ArmourKind::BasicHelm,
            Tier::One,
        )));
        let mut cracked = Piece::new(2, ArmourKind::BasicKevlar, Tier::One);
        cracked.health = 0.0;
        gear.body = Some(cracked);
        gear.pack[4] = Some(Item::Armour(Piece::new(
            3,
            ArmourKind::BasicLegs,
            Tier::One,
        )));
        game.issue(1, gear);
        assert_eq!(game.armour_health(1), 0.0);
        game.set_hostiles(vec![Some((
            kate + vec2(6.0 * TILE, 0.0),
            WeaponKind::LaserPistol.basic(),
        ))]);
        for _ in 0..10 {
            game.simulate(DT);
        }
        assert!(game.is_alarmed());
        assert_eq!(game.weapon(1), Some(WeaponKind::Schword.basic()));
        let worn = game.gear(1);
        assert!(worn.head.is_some_and(|p| p.id == 1), "the helm on");
        assert!(worn.legs.is_some_and(|p| p.id == 3), "the guards on");
        assert!(
            worn.body.is_some_and(|p| p.id == 2 && p.broken()),
            "the cracked vest as it was"
        );
        assert_eq!(game.armour_health(1), 25.0);

        // A gunner leaves its armour in the pack: the alarm is not a
        // dressing-up for a body that keeps its distance.
        let mut game = room();
        game.set_autonomous(false);
        let kate = game.put_for_probe(1, vec2(ROOM_W * 0.35, ROOM_H * 0.5));
        game.put_for_probe(0, vec2(ROOM_W * 0.7, ROOM_H * 0.85));
        let mut gear = Gear::default();
        gear.pack[0] = Some(Item::Weapon(WeaponKind::LaserPistol.basic()));
        gear.pack[1] = Some(Item::Armour(Piece::new(
            1,
            ArmourKind::BasicHelm,
            Tier::One,
        )));
        game.issue(1, gear);
        game.set_hostiles(vec![Some((
            kate + vec2(6.0 * TILE, 0.0),
            WeaponKind::LaserPistol.basic(),
        ))]);
        for _ in 0..10 {
            game.simulate(DT);
        }
        assert_eq!(game.weapon(1), Some(WeaponKind::LaserPistol.basic()));
        assert!(game.gear(1).head.is_none());

        // And what the world issues: a resident rolled the schword wears
        // the basic set, a mercenary with one too.
        let blades = (0..400u64)
            .filter(|&seed| Gear::issued_for(seed).weapon == Some(WeaponKind::Schword.basic()));
        let mut seen = 0;
        for seed in blades {
            seen += 1;
            assert_eq!(Gear::issued_for(seed).armour_health(), 45.0);
            let hired = Gear::hired_for(seed, 10);
            if hired.weapon == Some(WeaponKind::Schword.basic()) {
                assert_eq!(hired.armour_health(), 45.0, "seed {seed}");
            }
        }
        assert!(seen > 10);
        let mercenary_blades = (0..2000u64)
            .map(|seed| Gear::hired_for(seed, 10))
            .filter(|g| g.weapon == Some(WeaponKind::Schword.basic()))
            .collect::<Vec<_>>();
        assert!(!mercenary_blades.is_empty());
        assert!(mercenary_blades.iter().all(|g| g.armour_health() == 45.0));
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
        gear.pack[2] = Some(Item::Weapon(WeaponKind::LaserPistol.basic()));
        game.issue(1, gear);
        assert!(game.weapon(1).is_none());
        // A target far off: nothing.
        // Far from everybody: James stands nearer it than she does.
        let far = kate + vec2((ALARM_RANGE + 20.0) * TILE, 0.0);
        game.set_hostiles(vec![Some((far, WeaponKind::LaserPistol.basic()))]);
        for _ in 0..10 {
            game.simulate(DT);
        }
        assert!(!game.is_alarmed());
        assert!(!game.bims[1].character.is_recruited());
        assert!(!game.is_recruited(0), "James untouched");
        // Within range and in view: to arms, the pistol out of the pack,
        // and shots at it.
        let near = kate + vec2(4.0 * TILE, 0.0);
        game.set_hostiles(vec![Some((near, WeaponKind::LaserPistol.basic()))]);
        let mut shot = false;
        for _ in 0..(60 * 3) {
            game.simulate(DT);
            shot |= game.bolts_in_flight() > 0;
        }
        assert!(game.is_alarmed());
        assert!(game.bims[1].character.is_recruited(), "Kate under arms");
        assert_eq!(
            game.weapon(1),
            Some(WeaponKind::LaserPistol.basic()),
            "drawn from the pack"
        );
        assert!(shot, "and shooting");
        assert!(!game.is_recruited(0), "James is the player's");
        // Gone: stood down — once nobody has seen it for the hold, since
        // an enemy that steps out of sight is not an enemy gone.
        game.set_hostiles(Vec::new());
        game.take_hits();
        for _ in 0..(60 * 2) {
            game.simulate(DT);
        }
        assert!(game.is_alarmed(), "seen two seconds ago");
        for _ in 0..((ALARM_HOLD / DT) as usize + 2) {
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

    /// A room taken apart stands the crew down whether the **alarm** or
    /// a **player leading them** put them under arms (feature 84): both
    /// go through `mustered`, and `take_crew` used to reset the alarm
    /// alone — so a crew mustered by a player's drawn weapon, or by a
    /// squad order, was carried into the fresh room still recruited with
    /// the edge already spent and nothing left to let them go. Recruited
    /// and not mustered is a Bim that does nothing at all: no errand, no
    /// queue, and not even the bots' own stand.
    #[test]
    fn a_room_taken_apart_stands_down_a_crew_a_player_was_leading() {
        let mut game = room();
        game.set_autonomous(false);
        // No alarm anywhere near: the player draws its own weapon, which
        // is `led()` and musters the rest behind it.
        game.toggle_recruited(0);
        for _ in 0..4 {
            game.simulate(DT);
        }
        assert!(!game.is_alarmed(), "nobody is shooting at anybody");
        assert!(game.is_mustered(), "the crew are led");
        assert!(game.bims[1].character.is_recruited(), "Kate under arms");

        let crew = game.take_crew();
        assert!(!game.is_mustered(), "the room is at peace again");
        assert!(
            !crew[1].character.is_recruited(),
            "and Kate goes to the next room with nothing on her"
        );
        assert!(
            crew[0].character.is_recruited(),
            "the player's own is the player's, here as everywhere"
        );
    }

    /// Under the alarm a crew member that sees no enemy keeps to the
    /// player's side — walks into its slot beside James and stays there
    /// as he moves — and takes the player's orders, which it does not
    /// otherwise: an order holds it where it was sent until the alarm is
    /// over, when its post and its recruitment go together.
    #[test]
    fn a_marquee_selects_everybody_it_touches_and_a_right_drag_forms_them_up_along_a_line() {
        let mut game = room();
        game.set_autonomous(false);
        let james = game.put_for_probe(0, vec2(ROOM_W * 0.3, ROOM_H * 0.5));
        let kate = game.put_for_probe(1, vec2(ROOM_W * 0.5, ROOM_H * 0.5));
        // A sweep over both selects both; the panels' one is James, first.
        game.drag_begin(james.x - TILE, james.y - TILE);
        game.drag_end(0, kate.x + TILE, kate.y + TILE);
        assert_eq!(game.selected_count(0), 2);
        assert_eq!(game.selected_all(0), vec![0, 1]);
        assert_eq!(game.selected(0), Some(0));
        // A click on one is that one alone.
        game.drag_begin(kate.x, kate.y);
        game.drag_end(0, kate.x, kate.y);
        assert_eq!(game.selected_all(0), vec![1]);
        game.drag_begin(james.x - TILE, james.y - TILE);
        game.drag_end(0, kate.x + TILE, kate.y + TILE);

        // In peace a line moves the player's own alone — Kate takes no
        // orders — and one alone goes to where the drag began.
        let a = vec2(ROOM_W * 0.35, ROOM_H * 0.8);
        let b = vec2(ROOM_W * 0.55, ROOM_H * 0.8);
        assert_eq!(game.order_line(0, a, b), ORDER_MOVING);
        assert!((game.destination_for_probe(0).unwrap() - a).len() < TILE);
        assert!(
            game.destination_for_probe(1)
                .is_none_or(|d| (d - b).len() > TILE)
        );

        // Under the alarm both take it: spread along the line, each to the
        // end nearest its own place, so James — the western one — takes
        // the western end.
        let near = kate + vec2(4.0 * TILE, 0.0);
        game.set_hostiles(vec![Some((near, WeaponKind::LaserPistol.basic()))]);
        for _ in 0..5 {
            game.simulate(DT);
        }
        assert!(game.is_alarmed());
        game.drag_begin(0.0, 0.0);
        game.drag_end(0, ROOM_W, ROOM_H);
        assert_eq!(game.selected_count(0), 2);
        assert_eq!(
            game.order_line(0, b, a),
            ORDER_MOVING,
            "the drag's direction does not matter"
        );
        let (dj, dk) = (
            game.destination_for_probe(0).unwrap(),
            game.destination_for_probe(1).unwrap(),
        );
        assert!((dj - a).len() < TILE, "James to the west end: {dj:?}");
        assert!((dk - b).len() < TILE, "Kate to the east end: {dk:?}");
        // The line is drawn while it is dragged, and cleared when it ends.
        game.order_drag_begin(a.x, a.y);
        game.order_drag_update(b.x, b.y);
        assert!(game.order_drag.is_some());
        assert_eq!(game.order_drag_end(0, b.x, b.y, true), ORDER_MOVING);
        assert!(game.order_drag.is_none());
        // A point for two is a huddle: two spots, not one.
        let c = vec2(ROOM_W * 0.45, ROOM_H * 0.3);
        assert_eq!(game.order_move(0, c.x, c.y), ORDER_MOVING);
        let (dj, dk) = (
            game.destination_for_probe(0).unwrap(),
            game.destination_for_probe(1).unwrap(),
        );
        assert!((dj - dk).len() >= CREW_CLEARANCE, "apart: {dj:?} {dk:?}");
        assert!((dj - c).len() < 1.5 * TILE && (dk - c).len() < 1.5 * TILE);
    }

    #[test]
    fn under_the_alarm_the_crew_gather_round_the_player_and_take_orders() {
        let mut game = room();
        game.set_autonomous(false);
        let james = game.put_for_probe(0, vec2(ROOM_W * 0.3, ROOM_H * 0.5));
        let kate = game.put_for_probe(1, vec2(ROOM_W * 0.85, ROOM_H * 0.85));
        // Kate cannot be ordered about in peace.
        game.drag_begin(kate.x, kate.y);
        game.drag_end(0, kate.x, kate.y);
        assert_eq!(game.selected(0), Some(1));
        assert_eq!(game.order_move(0, james.x, james.y), ORDER_IGNORED);
        // An enemy within range but out of sight — beyond the room's walls,
        // twenty-five tiles off — is the alarm without a target to act on.
        let unseen = james + vec2((ALARM_RANGE - 5.0) * TILE, 0.0);
        game.set_hostiles(vec![Some((unseen, WeaponKind::LaserPistol.basic()))]);
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
        game.drag_end(0, at.x, at.y);
        assert_eq!(game.selected(0), Some(1));
        // Somewhere on the open deck, away from the heads and their door.
        let spot = vec2(ROOM_W * 0.5, ROOM_H * 0.5);
        // Through the heads' door if her slot was inside them.
        let code = game.order_move(0, spot.x, spot.y);
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
        assert_eq!(game.order_move(0, james.x, james.y), ORDER_IGNORED);
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
                weapon: Some(WeaponKind::Schword.basic()),
                ..Gear::default()
            },
        );
        game.recruit_for_probe(1, true);
        let beside = kate + vec2(TILE, 0.0);
        game.set_hostiles(vec![Some((beside, WeaponKind::LaserPistol.basic()))]);
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
        assert!(landed[0].who == 0 && landed[0].cut && landed[0].damage == 42.0);

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
            WeaponKind::LaserPistol.basic(),
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
        let mut game = room_with_bare_packs();
        game.set_autonomous(false);
        game.put_for_probe(0, vec2(ROOM_W * 0.45, ROOM_H * 0.5));
        let vest = Piece::new(7, ArmourKind::BasicKevlar, Tier::One);
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
                trauma: None,
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
    fn equipping_swaps_with_what_is_worn_and_give_and_take_round_trip() {
        // --- equipping_swaps_with_what_is_worn_and_a_weapon_with_the_weapon ---
        {
            let mut game = room_with_bare_packs();
            let helm = Piece::new(1, ArmourKind::BasicHelm, Tier::One);
            let other = Piece {
                health: 4.0,
                ..Piece::new(2, ArmourKind::BasicHelm, Tier::One)
            };
            // A helm lies two by four: the first three cells in along the
            // top rows, and the other, with no room beside it, on the third.
            assert!(game.give(0, Some(3), Item::Armour(helm)));
            assert!(game.give(0, None, Item::Armour(other)));
            assert_eq!(
                game.pack(0)[2 * PACK_COLS],
                Some(Item::Armour(other)),
                "first place it fits"
            );
            assert_eq!(game.equip(0, 4), None, "by any of its cells");
            assert_eq!(game.worn(0, Part::Head), Some(helm));
            assert_eq!(game.pack(0)[4], None);
            // The other goes on and the first comes back into its cells, its
            // damage with it — by any of its cells.
            assert_eq!(game.equip(0, 2 * PACK_COLS + 1), Some(Item::Armour(helm)));
            assert_eq!(game.worn(0, Part::Head), Some(other));
            assert_eq!(game.pack(0)[2 * PACK_COLS], Some(Item::Armour(helm)));
            assert_eq!(game.armour_health(0), 4.0);
            // Off again, into the first place it fits: the top rows, empty
            // since the first helm went on.
            assert!(game.unequip(0, Part::Head));
            assert_eq!(game.worn(0, Part::Head), None);
            assert_eq!(game.pack(0)[0], Some(Item::Armour(other)));
            assert!(!game.unequip(0, Part::Head), "nothing there now");

            // A weapon swaps with the hand; a stack is refused and stays put.
            let low = 4 * PACK_COLS;
            assert!(game.give(0, Some(low), Item::Weapon(WeaponKind::LaserPistol.basic())));
            assert_eq!(
                game.equip(0, low),
                Some(Item::Weapon(WeaponKind::LaserPistol.basic()))
            );
            assert_eq!(game.gear(0).weapon, Some(WeaponKind::LaserPistol.basic()));
            assert!(game.give(0, Some(5), Item::Stack(5)));
            assert_eq!(game.equip(0, 5), None);
            assert_eq!(game.pack(0)[5], Some(Item::Stack(5)));
            assert_eq!(game.equip(0, 6), None, "an empty cell");
            assert_eq!(game.equip(0, 99), None, "no such cell");
        }

        // --- give_and_take_round_trip_and_a_broken_piece_is_only_ever_discarded ---
        {
            let mut game = room_with_bare_packs();
            let legs = Piece {
                health: 3.5,
                ..Piece::new(9, ArmourKind::BasicLegs, Tier::One)
            };
            assert!(game.give(0, Some(2), Item::Armour(legs)));
            assert!(!game.give(0, Some(2), Item::Stack(17)), "the cell is full");
            assert!(
                !game.give(0, Some(2 + PACK_COLS), Item::Stack(17)),
                "and so is the cell the guards reach over"
            );
            assert_eq!(game.take(0, 2), Some(Item::Armour(legs)), "damage kept");
            assert_eq!(game.take(0, 2), None, "gone");
            // Fifty one-cell things in, the fifty-first has nowhere to go.
            for _ in 0..PACK_CELLS {
                assert!(game.give(0, None, Item::Stack(17)));
            }
            assert!(!game.give(0, None, Item::Stack(17)), "full");
            for i in 0..PACK_CELLS {
                assert_eq!(game.take(0, i), Some(Item::Stack(17)));
            }

            let broken = Piece {
                health: 0.0,
                ..Piece::new(10, ArmourKind::BasicHelm, Tier::One)
            };
            assert!(game.give(0, Some(0), Item::Armour(broken)));
            assert_eq!(game.take(0, 0), None, "a broken piece is not stowed");
            assert_eq!(game.pack(0)[0], Some(Item::Armour(broken)), "and stays");
            assert!(game.discard(0, 0));
            assert_eq!(game.pack(0)[0], None);
            assert!(!game.discard(0, 0), "nothing to throw away");
        }
    }

    #[test]
    fn a_click_on_a_bim_is_the_bim_and_on_a_body_down_is_the_body() {
        // --- a_click_on_a_bim_is_the_bim ---
        {
            let mut game = room();
            let at = game.put_for_probe(1, vec2(ROOM_W * 0.5, ROOM_H * 0.5));
            assert_eq!(game.hit_at(at.x + 4.0, at.y - 3.0), HIT_BIM);
            assert_eq!(game.hit_bim(), 1);
            assert_eq!(game.hit_at(at.x + 60.0, at.y), HIT_NONE);
        }

        // --- a_dead_bim_under_the_click_is_a_body_and_an_unconscious_one_is_still_the_bim ---
        {
            let mut game = room();
            game.set_autonomous(false);
            let at = game.put_for_probe(1, vec2(ROOM_W * 0.5, ROOM_H * 0.5));
            game.put_for_probe(0, vec2(ROOM_W * 0.2, ROOM_H * 0.8));
            assert!(!game.is_down(1));
            // Dead on the next step, and a body.
            game.kill_for_probe(1);
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

        // --- a_visitor_marked_down_is_a_body_and_one_on_its_feet_is_not ---
        {
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
    fn looting_strips_a_worn_helm_with_its_damage_and_an_awake_bim_is_refused() {
        let mut game = room_with_bare_packs();
        game.set_autonomous(false);
        game.put_for_probe(0, vec2(ROOM_W * 0.45, ROOM_H * 0.5));
        let helm = Piece {
            health: 6.0,
            ..Piece::new(7, ArmourKind::BasicHelm, Tier::One)
        };
        assert!(game.give(0, Some(0), Item::Armour(helm)));
        assert_eq!(game.equip(0, 0), None);
        assert!(game.give(0, Some(4), Item::Stack(5)));
        assert_eq!(game.gear(0).weapon, Some(WeaponKind::LaserPistol.basic()));
        // The window shows the pack, the worn pieces and the hand.
        let cells = game.loot_cells(0);
        assert_eq!(cells[4], Some(Item::Stack(5)));
        assert_eq!(
            cells[LootCell::Head.code() as usize],
            Some(Item::Armour(helm))
        );
        assert_eq!(cells[LootCell::Body.code() as usize], None);
        assert_eq!(
            cells[LootCell::Weapon.code() as usize],
            Some(Item::Weapon(WeaponKind::LaserPistol.basic()))
        );
        // Alive and awake, nothing comes off it.
        assert!(!game.is_down(0));
        assert_eq!(game.take_from_body(0, LootCell::Head), None);
        assert_eq!(game.take_from_body(0, LootCell::Pack(4)), None);
        assert_eq!(game.worn(0, Part::Head), Some(helm), "still worn");
        // Dead, everything does: the helm with its damage, the hand, the
        // pack cell, and each only once.
        game.kill_for_probe(0);
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
            Some(Item::Weapon(WeaponKind::LaserPistol.basic()))
        );
        assert_eq!(game.weapon(0), None);
        assert_eq!(
            game.take_from_body(0, LootCell::Pack(4)),
            Some(Item::Stack(5))
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
        assert_eq!(LootCell::Weapon.code() as usize, PACK_CELLS + 3);
    }

    #[test]
    fn an_execution_walks_to_the_body_and_the_room_says_who_was_finished() {
        let mut game = room();
        game.set_autonomous(false);
        let james = game.put_for_probe(0, vec2(ROOM_W * 0.3, ROOM_H * 0.5));
        game.put_for_probe(1, vec2(ROOM_W * 0.2, ROOM_H * 0.85));
        game.issue(1, Gear::default());
        // One of the other room's people lying eight tiles off, marked
        // down by the world; a second on its feet beside it.
        let body = james + vec2(8.0 * TILE, 0.0);
        let up = body + vec2(0.0, 2.0 * TILE);
        game.set_visitors(vec![body, up]);
        game.set_visitors_down(&[true, false]);
        assert!(!game.execute(0, 1), "one on its feet is nobody to finish");
        assert!(!game.execute(0, 5), "nor one that is not there");
        // With the pistol: to within three tiles, then three seconds of
        // shooting — bolts in the air — and the room says whose body.
        assert!(game.execute(0, 0));
        assert_eq!(game.activity(0), JOB_EXECUTE);
        let to = game.destination_for_probe(0).expect("walking");
        assert!(
            (to - body).len() <= (task::EXECUTE_RANGE + 0.5) * TILE,
            "to within range: {to:?}"
        );
        assert!((to - body).len() > TILE, "a gun keeps a little off");
        let mut fired = false;
        let mut steps = 0;
        let mut done = Vec::new();
        while done.is_empty() && steps < 60 * 30 {
            game.simulate(DT);
            fired |= game.bolts_in_flight() > 0;
            done.extend(game.take_executed());
            steps += 1;
        }
        assert_eq!(done, vec![(0, 0)], "finished");
        assert!(fired, "shot at where it lay");
        assert!(game.is_armed(0) || game.activity(0) != JOB_EXECUTE);
        assert!(
            game.bims[0].task.is_none() || game.activity(0) != JOB_EXECUTE,
            "over"
        );

        // With the schword: beside it, and swings; no bolt. The pistol's
        // bolts flown out first.
        for _ in 0..60 * 3 {
            game.simulate(DT);
        }
        assert_eq!(game.bolts_in_flight(), 0);
        game.issue(
            0,
            Gear {
                weapon: Some(WeaponKind::Schword.basic()),
                ..Gear::default()
            },
        );
        let james = game.put_for_probe(0, vec2(ROOM_W * 0.3, ROOM_H * 0.5));
        game.set_visitors(vec![body, up]);
        game.set_visitors_down(&[true, false]);
        assert!(game.execute(0, 0));
        let to = game.destination_for_probe(0).expect("walking");
        assert!((to - body).len() <= 1.5 * TILE, "beside it: {to:?}");
        let mut fired = false;
        let mut swung = false;
        let mut steps = 0;
        let mut done = Vec::new();
        while done.is_empty() && steps < 60 * 30 {
            game.simulate(DT);
            fired |= game.bolts_in_flight() > 0;
            swung |= game.bims[0].character.action() == Action::Swing;
            done.extend(game.take_executed());
            steps += 1;
        }
        assert_eq!(done, vec![(0, 0)]);
        assert!(swung, "hacked at: fired {fired}");
        assert!(!fired, "not shot");
        let _ = james;

        // The body coming round mid-walk ends it: nowhere to go.
        game.put_for_probe(0, vec2(ROOM_W * 0.3, ROOM_H * 0.5));
        assert!(game.execute(0, 0));
        game.set_visitors(vec![body, up]);
        game.set_visitors_down(&[false, false]);
        for _ in 0..60 * 20 {
            game.simulate(DT);
        }
        assert!(game.take_executed().is_empty());
        assert_ne!(game.activity(0), JOB_EXECUTE, "given up");

        // In the body's own room, the killing: down and alive, dead after;
        // one on its feet is left alone.
        let mut theirs = room();
        theirs.set_autonomous(false);
        assert!(!theirs.execute_body(1), "on her feet");
        for _ in 0..10 {
            theirs.wound(1, Part::Body, 1.0);
        }
        let mut minutes = 0.0;
        while !theirs.is_unconscious(1) && minutes < 60.0 {
            theirs.simulate(1.0);
            minutes += MINUTES_PER_SECOND;
        }
        assert!(theirs.is_unconscious(1) && theirs.is_alive(1));
        assert!(theirs.execute_body(1));
        theirs.simulate(DT);
        assert!(!theirs.is_alive(1), "dead");
        assert!(!theirs.execute_body(1), "and dead is not down to finish");
    }

    /// A Bim that soils itself wants a wash: the accident leaves its own
    /// filth on it and empties the washing need with it, where the deck
    /// under it is somebody's broom's business. A wetting is half as bad on
    /// both counts. Forced rather than waited an hour for: a poisoned Bim
    /// at the extreme urge goes at once.
    #[test]
    fn an_accident_empties_the_washing_need_along_with_covering_the_bim() {
        let mut game = room();
        game.set_autonomous(false);
        game.put_for_probe(0, vec2(ROOM_W * 0.5, ROOM_H * 0.5));
        assert_eq!(game.need_level(0, Need::Hygiene as u32), 1.0);
        game.poison_for_probe(0);
        game.spend_for_probe(0, Need::Restroom as u32, 1.0);
        game.simulate(DT);
        assert_eq!(game.bims[0].character.filth(), 1.0, "covered");
        assert_eq!(
            game.need_level(0, Need::Hygiene as u32),
            0.0,
            "wants a wash"
        );
        assert!(
            game.need_level(0, Need::Restroom as u32) > 0.0,
            "and is relieved"
        );

        // A wetting on a clean Bim: the bar drops to what is still clean of
        // it, and a second one does not lift it back.
        game.bims[0].character.wash(1.0);
        game.bims[0].needs.refill(Need::Hygiene, 1.0);
        game.bims[0].needs.soiled(0.45);
        assert!((game.need_level(0, Need::Hygiene as u32) - 0.55).abs() < 1e-6);
        game.bims[0].needs.spend(Need::Hygiene, 0.3);
        game.bims[0].needs.soiled(0.45);
        assert!((game.need_level(0, Need::Hygiene as u32) - 0.25).abs() < 1e-6);
    }

    /// The host lays its smooth fog between the room's picture and what
    /// the room draws over its fog (feature 97): a bolt in flight is in
    /// the second half, so it is seen whatever it flies through, and the
    /// halves are the whole picture, in order — the three-way cut too.
    #[test]
    fn a_bolt_in_flight_is_drawn_over_the_fog() {
        let mut game = room();
        game.set_autonomous(false);
        let kate = game.put_for_probe(1, vec2(ROOM_W * 0.35, ROOM_H * 0.5));
        let near = kate + vec2(4.0 * TILE, 0.0);
        game.set_hostiles(vec![Some((near, WeaponKind::LaserPistol.basic()))]);
        for _ in 0..(60 * 10) {
            if !game.combat.bolts.is_empty() {
                break;
            }
            game.simulate(DT);
        }
        assert!(!game.combat.bolts.is_empty(), "somebody fired");
        game.render();
        let (under, over) = game.shapes_fog_split();
        assert_eq!([under, over].concat(), game.shapes());
        let head = game.combat.bolts[0].pos;
        let at_the_bolt = |part: &[f32]| {
            part.chunks_exact(crate::draw::STRIDE)
                .any(|s| (vec2(s[1], s[2]) - head).len() < TILE)
        };
        assert!(at_the_bolt(over), "the bolt is over the fog");
        let (deck, bodies, over_too) = game.shapes_in_three();
        assert_eq!(over_too, over);
        assert_eq!([deck, bodies].concat(), under);
    }

    /// The pistol bolt's core is the one emissive colour in a pistol fight
    /// (feature 97) a host ages no passing lights in: with a bolt in flight
    /// there is a channel past one over the fog and nowhere under it, and
    /// with none there is none at all. (A blade in a hand and a machine's
    /// sparks glow too since feature 98, and neither is in this room.)
    #[test]
    fn the_pistol_bolt_s_core_is_the_one_thing_brighter_than_white() {
        let emissive = |part: &[f32]| {
            part.chunks_exact(crate::draw::STRIDE)
                .any(|s| s[8] > 1.0 || s[9] > 1.0 || s[10] > 1.0)
        };
        let mut game = room();
        game.set_autonomous(false);
        game.render();
        assert!(!emissive(game.shapes()), "nothing glows before a shot");
        let kate = game.put_for_probe(1, vec2(ROOM_W * 0.35, ROOM_H * 0.5));
        let near = kate + vec2(4.0 * TILE, 0.0);
        game.set_hostiles(vec![Some((near, WeaponKind::LaserPistol.basic()))]);
        for _ in 0..(60 * 10) {
            if !game.combat.bolts.is_empty() {
                break;
            }
            game.simulate(DT);
        }
        assert_eq!(game.combat.bolts[0].weapon.kind, WeaponKind::LaserPistol);
        game.render();
        let (under, over) = game.shapes_fog_split();
        assert!(emissive(over), "the core glows");
        assert!(!emissive(under), "and nothing under the fog does");
    }

    /// The fight's passing lights (feature 98), in a room a host ages:
    /// a part struck flashes where the part is and the room's own flash
    /// over the whole body stands aside; a machine destroyed bursts, and
    /// one only struck does not; and a room nobody ages records nothing
    /// and draws the old flash — the picture every test and probe sees.
    #[test]
    fn a_room_a_host_ages_flashes_the_part_struck_and_bursts_a_machine() {
        use crate::droid::{Droid, DroidKind, DroidPart};
        let flash_over_body = |game: &Game| {
            let at = game.bims[1].character.drawn_at();
            game.shapes().chunks_exact(crate::draw::STRIDE).any(|s| {
                (vec2(s[1], s[2]) - at).len() < 1.0 && s[3] >= 42.0 && (s[8] - HIT.r).abs() < 1e-6
            })
        };
        // Nobody ages it: nothing recorded, and the whole-body flash.
        let mut plain = room();
        plain.set_autonomous(false);
        plain.strike(1, Part::Head, 1.0, false);
        plain.render();
        assert_eq!(plain.fx_count_for_probe(), (false, 0));
        assert!(flash_over_body(&plain), "the room's own flash");

        // A host ages it: the part's flash, and not the body's.
        let mut lit = room();
        lit.set_autonomous(false);
        lit.fade(0.0);
        lit.strike(1, Part::Head, 1.0, false);
        lit.render();
        assert_eq!(lit.fx_count_for_probe(), (true, 1));
        assert!(!flash_over_body(&lit), "the part flashes instead");
        let (head, _) = lit.bims[1].character.part_mark(Part::Head);
        assert!(
            lit.shapes()
                .chunks_exact(crate::draw::STRIDE)
                .any(|s| (vec2(s[1], s[2]) - head).len() < 1e-3),
            "a flash where the head is"
        );
        // Real seconds, not the simulation's: a minute of steps leaves
        // it, a quarter of a second of frames takes it.
        for _ in 0..60 {
            lit.simulate(DT);
        }
        assert_eq!(lit.fx_count_for_probe().1, 1, "the steps do not age it");
        lit.fade(DT);
        lit.fade(0.25);
        assert_eq!(lit.fx_count_for_probe().1, 0, "the frames do");

        // A machine: struck, a flash; destroyed, a burst beside it.
        let at = vec2(ROOM_W * 0.5, ROOM_H * 0.5);
        let i = lit.add_droid(Droid::new(DroidKind::Trooper, Tier::One, 0, 1, at, 0.0, 5))
            - lit.crew_count() as usize;
        lit.strike_droid(i, DroidPart::Arms, 1.0);
        assert_eq!(lit.fx_count_for_probe().1, 1, "a flash on its arm");
        lit.strike_droid(i, DroidPart::Chassis, 10_000.0);
        assert!(lit.droids()[i].destroyed);
        assert_eq!(lit.fx_count_for_probe().1, 3, "a flash and a burst");
        lit.strike_droid(i, DroidPart::Chassis, 10_000.0);
        assert_eq!(lit.fx_count_for_probe().1, 3, "a wreck takes nothing more");
    }
}
