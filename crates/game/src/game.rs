//! The room's game: the deck, the bodies on it, the errand each is on, the
//! fight, and the per-frame draw list handed to the renderer.

use crate::bim::{Bim, CREW, Hand, PLAYER, TRAIL_LIFE};
use crate::character::{
    ACCENT, Action, BODY_MARGIN, FallBack, Look, Outfit, PICK_RADIUS, SWING_TIME, Tint, Worn,
};
use crate::clock::MINUTES_PER_SECOND;
use crate::clock::{self, Clock};
use crate::combat::{
    ArmourKind, Blow, COVER_WORTH, Combat, FIST_DAMAGE, Gear, Grenade, Hit, Item, MELEE_PERIOD,
    MELEE_RANGE, Piece, Sentry, Shot, Skill, Tactics, Weapon, WeaponKind, WeaponStats,
    line_of_fire,
};
use crate::cue::{Cue, Cued};
use crate::door;
use crate::draw::{Color, DrawList};
use crate::droid::{Droid, DroidPart};
use crate::health::Health;
use crate::math::{Rect, TAU, Vec2, clamp, vec2};
use crate::nav::{self, Maps, Nav};
use crate::rng::Rng;
use crate::room::{self, GLOW, HIT_BIM, HIT_BODY, HIT_NONE, HIT_VISITOR, Room, Switch, TILE, WARN};
use crate::sight::{Fog, Sight, Stance};
use crate::task::{self, Kind, Saved, Task};
use crate::work::{self, Job, Priorities};

/// The Machine Heart's machines in the room (feature 108): the core's
/// beams, and nothing for a conduit or a fabricator to do.
#[path = "heart.rs"]
mod heart;
/// A Manufacturer in a friendly room (task 131): one of their people
/// attacking a site the crew are defending, fought the machines' way.
#[path = "intruder.rs"]
mod intruder;

const TRAIL: Color = ACCENT;

const MARQUEE_EDGE: Color = ACCENT;
const MARQUEE_FILL: Color = Color::rgba(0.50, 0.82, 0.66, 0.10);

/// How long the ping at an ordered destination lasts, in real seconds
/// where the host fades the fight's lights (`Game::fade`).
const MARKER_LIFE: f32 = 0.9;

/// How long a body on somebody else's deck stays drawn after the crew
/// last saw it, in seconds at 1x: it walks out of view and is a moment
/// fading, rather than winking out at the bulkhead.
pub const SEEN_FOR: f32 = 2.0;

/// How long the flash a hit puts on a body lasts, in seconds.
const HIT_FLASH: f32 = 0.22;

/// The part code a Bim's flash is kept under (`Fx::struck`): a hit on a
/// Bim lands nowhere in particular (October 2026), and only a droid's
/// flash reads its code.
const BIM_STRUCK: u32 = 0;

/// A revive finished (task 120): whose hands brought whom round. For the
/// world's relics (`Game::take_revives`).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Revived {
    pub helper: usize,
    pub patient: usize,
}

/// Where a revive's patient is a **visitor** rather than one of this
/// room's own Bims: `GUEST + i` is visitor `i` (`Game::set_visitors`) —
/// a townsperson downed in the residents' room, which the world names
/// revivable (`Game::set_visitors_revivable`). The crew's hands go on it
/// here and the world brings it round in its own room
/// (`Game::take_guest_revives`). Far past any room's bodies.
pub const GUEST: usize = 1 << 16;

/// A revive of a visitor finished: whose hands, which visitor (its index
/// in its own room, `GUEST` taken off) and the share of its bar it gets
/// up at — the helper's, which only this room knows.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct GuestRevived {
    pub helper: usize,
    pub visitor: usize,
    pub share: f32,
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
/// None of this reaches a Bim a player steers, a body on a chain or one
/// holding a post its player clicked for it:
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
    /// Back to the ship, and hold there. The ship is where the last stand
    /// is made. (Nobody runs of its own accord since task 120: a body at
    /// nought is downed where it stands.)
    Retreat,
}

/// How a shot on a body went — see [`Game::wound`]. All nought for a shot
/// on nobody: a dead Bim, or one that is not there.
#[derive(Clone, Copy, PartialEq, Debug, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct WoundOutcome {
    /// The hit took the bar to nothing: the body is downed (task 120).
    pub downed: bool,
    /// What the armour on the part took off it, its protection included.
    pub absorbed: f32,
    /// What got past the armour and came off the bar.
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
/// How long after a target was last seen its trail is still followed, in
/// seconds: one of the hunting side come within [`SEARCHED`] tiles of
/// the spot it is believed at, and nobody there, is a spot searched, and
/// the belief moves on to where the target has got to — its age kept, so
/// the trail goes cold this long after the sighting however often it is
/// followed. Past it a searched spot is where the hunt stands until the
/// belief is forgotten.
pub const TRAIL_FOR: f32 = 20.0;
/// How near one of a side must come to a believed spot for it to count
/// as searched, in tiles.
pub const SEARCHED: f32 = 1.5;
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
/// How long a body counts as **under fire** after an enemy's hit lands
/// on it, in seconds: a bot under fire leaves the ring round its player
/// for a stand of its own and takes up no revive, since a machine
/// shooting from the dark is one nobody sees, and the crew used to stand
/// in the ring being shot at (the user's report).
pub const UNDER_FIRE: f32 = 5.0;
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

/// What the Bim is doing, as plain codes the host turns into words. The
/// numbers are the host's to name, so a code that went — the needs' errands,
/// finishing a body off (feature 104) — leaves its number unused rather than
/// moving the rest.
/// Working a door's panel: holding it or letting it be, and locking or
/// unlocking it.
pub const JOB_DOOR: u32 = 7;
pub const JOB_DOOR_LOCK: u32 = 8;
// 18 was making something at a bench, which went with the crafting (task
// 113).
pub const JOB_EVA: u32 = 19;
pub const JOB_HAUL: u32 = 20;
pub const JOB_BUILD: u32 = 21;
/// Reviving a downed crewmate (task 120; 22 was dressing a wound, and 23
/// treating a trauma, which went with them).
pub const JOB_REVIVE: u32 = 22;
// 24 was picking a dropped weapon up, which went with the dropping (task
// 113), and 25 finishing a body off, which went with every human enemy
// (feature 104).
// 26 was carrying a thing to the workbench, which went with it (task 113).
/// A walk to a spot on the deck waiting its turn — a Shift-click. Only
/// ever on the agenda, never the activity: the walk is given, not run.
pub const JOB_WALK: u32 = 27;
/// Laying an engineer's kit on the deck (feature 74).
pub const JOB_DEPLOY: u32 = 28;

fn job_code(kind: Kind) -> u32 {
    match kind {
        Kind::Switch(Switch::Door(_, door::Order::Open | door::Order::Close)) => JOB_DOOR,
        Kind::Switch(Switch::Door(_, door::Order::Lock | door::Order::Unlock)) => JOB_DOOR_LOCK,
        Kind::Build { .. } => JOB_BUILD,
        Kind::Revive { .. } => JOB_REVIVE,
        Kind::Walk { .. } => JOB_WALK,
        Kind::Deploy { .. } => JOB_DEPLOY,
    }
}

/// The room after dark. Laid over the finished frame rather than mixed into
/// every colour, so nothing in `room.rs` has to know what time it is.
const NIGHT: Color = Color::rgb(0.03, 0.05, 0.13);
/// How dark it gets at the middle of the night. Short of black: you still
/// want to see the Bims.
const NIGHT_DEPTH: f32 = 0.46;

/// A bolt's own light in the dark (task 152): the pool round it, drawn
/// over the darkness as rings of its colour, each wider and fainter —
/// [`BOLT_GLOW_RINGS`] of them out to [`BOLT_GLOW_REACH`] tiles across,
/// the middle at [`BOLT_GLOW`] — and faded by how lit its tile is, so a
/// laser lights its way down a dark corridor and adds nothing to a lamp's
/// pool. The rule's half is `Sight::set_flares`.
const BOLT_GLOW: f32 = 0.6;
const BOLT_GLOW_REACH: f32 = 3.5;
const BOLT_GLOW_RINGS: u32 = 10;

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
///
/// And a side that is `told` where a target is — a room with a
/// reinforcement wave of the machines standing in it (`Droid::seeking`),
/// or a wave attacking a site the crew defend, about everybody under arms
/// there — believes it where it stands now, seen or not: still stale out
/// of sight, so it is walked towards and never fired at.
///
/// **A body shot at knows where from.** Each of `shot_at` — the side's
/// bodies under fire ([`UNDER_FIRE`]) — gives away the target that most
/// likely fired: the nearest with a clear line to it, lit or not
/// (`Sight::sees_from_in_the_dark`), else the nearest of all (a grenade
/// over a wall). That one is believed where it stands, as a told side
/// believes, so a machine shot out of the dark walks at the shooter
/// instead of standing in the fire; still stale, so nothing fires at it
/// until it is seen.
///
/// **And a trail is followed.** A stale belief one of `eyes` has come
/// within [`SEARCHED`] tiles of — the spot searched and nobody there —
/// moves on to where the target is now while it was seen within
/// [`TRAIL_FOR`], its age kept, so a hunt does not end at the corner the
/// quarry was last seen turning.
fn believe(
    seen: &mut Vec<Option<Seen>>,
    sight: &Sight,
    watched: Option<Vec2>,
    told: impl Fn(usize) -> bool,
    eyes: &[Vec2],
    shot_at: &[Vec2],
    at: Vec<Option<(Vec2, Weapon)>>,
) -> (Vec<Option<(Vec2, Weapon)>>, Vec<bool>) {
    seen.resize(at.len(), None);
    let fired: Vec<usize> = shot_at
        .iter()
        .filter_map(|&body| {
            let nearest = |clear: bool| {
                at.iter()
                    .enumerate()
                    .filter_map(|(i, t)| t.map(|(p, _)| (i, p)))
                    .filter(|&(_, p)| !clear || sight.sees_from_in_the_dark(body, p).is_some())
                    .min_by(|a, b| {
                        (a.1 - body)
                            .len()
                            .partial_cmp(&(b.1 - body).len())
                            .unwrap_or(std::cmp::Ordering::Equal)
                    })
                    .map(|(i, _)| i)
            };
            nearest(true).or_else(|| nearest(false))
        })
        .collect();
    let mut believed = Vec::with_capacity(at.len());
    let mut stale = Vec::with_capacity(at.len());
    for (i, target) in at.into_iter().enumerate() {
        let mut in_sight = false;
        match target {
            None => seen[i] = None,
            Some((p, weapon)) => {
                in_sight = watched.is_some_and(|w| (w - p).len() <= AIRLOCK_WATCH * TILE)
                    || eyes.iter().any(|&eye| sight.sees_from(eye, p).is_some());
                if in_sight || told(i) || fired.contains(&i) {
                    seen[i] = Some(Seen {
                        at: p,
                        weapon,
                        ago: 0.0,
                    });
                } else if let Some(was) = seen[i].as_mut() {
                    // The weapon it carries is known whether or not it is
                    // in sight — the world says — and the tactics read it.
                    was.weapon = weapon;
                    let searched = eyes
                        .iter()
                        .any(|&eye| (eye - was.at).len() <= SEARCHED * TILE);
                    if searched && was.ago < TRAIL_FOR {
                        was.at = p;
                    }
                }
            }
        }
        believed.push(seen[i].map(|s| (s.at, s.weapon)));
        stale.push(!in_sight);
    }
    (believed, stale)
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone)]
struct Marker {
    pos: Vec2,
    age: f32,
    /// A place the Bim cannot get to. Drawn in the one warm colour the room
    /// keeps for things worth noticing, so a refused order is not silent.
    bad: bool,
    /// What kind of order put it there: a walk's green or an
    /// attack-move's red. Never read for a refusal.
    #[cfg_attr(feature = "serde", serde(default))]
    kind: Ping,
    /// The Bim the order was for, so that another player's ping is drawn
    /// in their colour and see-through ([`Game::draw_pings`]). A picture
    /// matter only, left out of a save: a ping lives under a second, and
    /// one loaded without it is drawn as the viewer's own.
    #[cfg_attr(feature = "serde", serde(skip))]
    by: Option<usize>,
}

/// The two orders a ping on the deck can stand for, drawn the way Dota
/// draws them: arrows closing on the spot, green for a walk and red for
/// an attack-move.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
enum Ping {
    #[default]
    Move,
    Attack,
}

/// A walk's ping: a bright green, lit a little past white so the bloom
/// lifts it off a dark deck.
const PING_MOVE: Color = Color::rgb(0.30, 1.0, 0.42);
/// An attack-move's: the enemy's red, as hot.
const PING_ATTACK: Color = Color::rgb(1.0, 0.24, 0.18);
/// How much of another player's ping shows through: theirs are drawn in
/// their own colour and this see-through, the viewer's own whole.
const OTHER_PING_ALPHA: f32 = 0.5;
/// How far out the ping's arrows start, and how far in they close, in
/// room units: most of a tile each side down to a body's width.
const PING_OUTER: f32 = 44.0;
const PING_INNER: f32 = 12.0;
/// An arrow's shaft and the barbs of its head, in room units.
const PING_SHAFT: f32 = 20.0;
const PING_BARB: f32 = 12.0;
/// An attack-move is over this close to where it was bound, in tiles.
const ATTACK_MOVE_THERE: f32 = 1.0;
/// How far from a right-click an enemy is still the one clicked, in
/// tiles: a Bim's pick reach and a little, since the biggest machine is
/// over half a tile across (task 126).
const ENEMY_PICK: f32 = 0.7;
/// The attack order's mark on its target: brackets at a machine's width
/// out, in room units, and how long each bracket's arms are.
const FOCUS_MARK: f32 = 34.0;
const FOCUS_ARM: f32 = 11.0;

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

/// A part of the ship only one Bim can be using at a time.
///
/// Most of the room is shared happily — two of them can walk past each
/// other, dress one patient — but some of it is one set of hands' worth: a
/// the airlock, a site. So an errand that needs one of these does not start while the other Bim is
/// on an errand that needs the same. It is not a queue and nobody waits in
/// line: the errand simply is not begun, and `consider_errand` moves on to
/// whatever else that Bim could be doing. It will come round again.
#[derive(Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
enum Exclusive {
    /// The airlock, for a walk outside to a site beyond the hull. One body out at a time: the suit is counted by the world and
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
        Kind::Build { outside: true, .. } => Some(Exclusive::Airlock),
        Kind::Build { site, .. } => Some(Exclusive::Site(site)),
        // A ship's door has a panel each side and takes a moment: nobody
        // needs to wait for it. A revive holds nothing of the ship's: one
        // reviver a patient is the medical row's rule (`revive_on_offer`).
        Kind::Switch(..) | Kind::Revive { .. } | Kind::Walk { .. } | Kind::Deploy { .. } => None,
    }
}

/// What became of a right-click on the floor. The host turns these into words.
pub const ORDER_IGNORED: u32 = 0;
pub const ORDER_MOVING: u32 = 1;
/// No route there at all. (2 and 3 were a walk through the test room's
/// heads door and one it found locked, gone with that room.)
pub const ORDER_NOWHERE: u32 = 4;

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone)]
pub struct Game {
    room: Room,
    /// The room's own time of day. It stands where the room was opened at —
    /// only travel moves the world's day, and every clock a mission keeps
    /// runs on the steps themselves — so nothing here advances it but
    /// `wind_clock`.
    clock: Clock,
    /// The nav grids: the deck's, rebuilt when a door is locked or unlocked,
    /// and the outside's and the plain's while somebody is out there.
    maps: Maps,
    /// Everything the body is pushed out of, the shut doors included.
    /// Rebuilt only when a door changes, which physics can afford to be a
    /// frame behind on — routing, which cannot, goes through `maps` instead.
    blockers: Vec<Rect>,
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
    /// And which of the crew, when the click landed on a body rather than
    /// on the deck: `HIT_BIM`, and the menu on a body opens on this one.
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
    hit_research: usize,
    rng: Rng,
    /// The bodies on the deck: the crew, or a station's people. The index
    /// into this is a Bim's whole identity — it picks its bunk, its
    /// coverall, and the name the host prints over its head.
    bims: Vec<Bim>,
    /// The machines on this deck (feature 83, `crate::droid`), beside the
    /// Bims and never among them: a droid-held station's room has these
    /// and no `bims` at all. They stand **after** the Bims in the one
    /// **body** index space the world hands over and reads back
    /// ([`Game::body_count`]), so a target past `bims.len()` is one of
    /// these and a hit past it lands on one. Empty everywhere else —
    /// aboard a ship, in a bare room, at a station people live on.
    droids: Vec<Droid>,
    /// Which of the crew is ticked first this frame. Rotates every frame so
    /// first refusal on a bench or a site goes round rather than always
    /// falling to the same Bim.
    first_tick: usize,
    /// Corners of the selection box while the mouse is down; `None` otherwise.
    drag: Option<(Vec2, Vec2)>,
    /// A right-drag on the deck with a selection: the line the selected
    /// crew will be spread along, drawn while it is drawn. See
    /// `Game::order_drag_end`.
    order_drag: Option<(Vec2, Vec2)>,
    markers: Vec<Marker>,

    /// The order the player wants the work done in. One list for the ship:
    /// the whole crew work to it. See `work.rs`.
    priorities: Priorities,
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
    /// `set_visitors`. Empty in a bare room, and on a ship alone.
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
    /// Which of the visitors may be revived by the crew's hands — a
    /// townsperson downed in its own room, as the world says — index for
    /// index with `visitors`, told right after `set_visitors` every step
    /// and cleared with it. A revive of one names it `GUEST + i`.
    #[cfg_attr(feature = "serde", serde(skip))]
    visitors_revivable: Vec<bool>,
    /// Every revive of a visitor finished since the world last asked
    /// (`take_guest_revives`), for it to bring the body round in its own
    /// room.
    #[cfg_attr(feature = "serde", serde(skip))]
    guest_revives: Vec<GuestRevived>,
    /// Which of this room's own bodies have somebody else's hands on
    /// them — a crew member kneeling at a townsperson on the joined deck
    /// — as the world says every step (`set_tended`): their countdown
    /// stands the way a revive in this room stands it.
    #[cfg_attr(feature = "serde", serde(skip))]
    tended: Vec<bool>,
    /// Whether a Bim nobody steers picks its own work off the list.
    autonomous: bool,
    /// A `SPOT_` code the host has asked to have ringed on the deck, or
    /// `SPOT_NOTHING`. This is how a panel that names a place — "Making
    /// things", the research desk — points at the actual thing rather than
    /// leaving the player to hunt for it. Set and cleared by the pointer; nothing in the
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
    /// joined deck — and whose: what `is_aboard` reads. The fog does not
    /// (task 128).
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
    /// And whose the rest are, as the world last said (`stance`).
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
    /// Whether the world has left some of the targets off this room's
    /// list because nobody may pick them — a medic's cloak (task 130) —
    /// so an enemy left with nobody it may pick **holds where it stands**,
    /// its facing kept, rather than walking on to where it was going. Said
    /// every step by the world; off, the room is what it was.
    #[cfg_attr(feature = "serde", serde(default))]
    withheld: bool,
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
    /// What each Bim shoots with over its weapon (`set_skills`, feature
    /// 75): a soldier's talents and its brace, `Skill::NONE` for anybody
    /// the world does not name.
    skills: Vec<Skill>,
    /// What each player's standing order to the bots is (`set_orders`,
    /// feature 84): one an entry, by player slot, [`Standing::Follow`] for
    /// a slot that has not said. The world's word, said every step, and
    /// left out of a save with it — the order itself is the world's
    /// (`world::Standing`).
    #[cfg_attr(feature = "serde", serde(skip))]
    standing: Vec<Standing>,
    /// Which players' own Bims the alarm recruited (September 2026): a
    /// player's Bim takes arms by itself when a fight starts, and the
    /// alarm's end lets go of those and of nobody the player recruited.
    /// See `arm_players`.
    #[cfg_attr(feature = "serde", serde(default))]
    alarm_armed: Vec<bool>,
    /// Every revive finished since the world last asked — whose hands, on
    /// whom — for the relics (`take_revives`, task 120).
    #[cfg_attr(feature = "serde", serde(skip))]
    revives: Vec<Revived>,
    /// Whether this room's own Bims revive one another of their own accord
    /// (task 120): the crew's room, and nobody else's — a station's or a
    /// town's people are not the crew. The world's word
    /// (`set_revivers`); on in a bare room.
    revivers: bool,
    /// The tiles of laid sandbags a grenade's burst reached since the
    /// world last asked, for it to take the deployables off
    /// (`take_bags_blown`).
    bags_blown: Vec<(i32, i32)>,
    /// The targets an EMP's burst reached since the world last asked
    /// (task 127): the target's index, the seconds and whether the stun
    /// exposes it — for the world to stun the machines they are
    /// (`take_stuns`).
    #[cfg_attr(feature = "serde", serde(default))]
    stuns: Vec<(usize, f32, bool)>,
    /// Every satchel charge that came down since the world last asked
    /// (task 154): who threw it and where it landed, for the world to lay
    /// it on the deck (`take_satchels_landed`).
    #[cfg_attr(feature = "serde", serde(default))]
    satchels_landed: Vec<(usize, Vec2)>,
    /// Every worn piece a hit broke since the world last asked — whose,
    /// and what it was — for the world to say so. See `Game::wound`.
    pieces_broken: Vec<(usize, ArmourKind)>,
    /// Every Bim a hit downed since the world last asked, for the world to
    /// say so. See `Game::strike`.
    downs: Vec<usize>,
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
    /// alarm. One in a bare room and aboard a ship with one player;
    /// the world sets the ship's (`set_players`). See `crate::order`.
    players: usize,
    /// Whose eyes the picture is drawn for: the selection ring is that
    /// player's. A picture setting, this window's own, left out of a save
    /// with the picture (`set_viewer`).
    #[cfg_attr(feature = "serde", serde(skip))]
    viewer: u32,
    /// Whether this room's people were told where the crew are: a
    /// reinforcement of the Manufacturers' standing here, as the world
    /// says every step before it hands the hostiles over (`set_told`) —
    /// what `Droid::seeking` is for the machines'. Worked out again by
    /// the world every step, so left out of a save.
    #[cfg_attr(feature = "serde", serde(skip))]
    told: bool,
    /// How far the window's clock is between the last step and the next,
    /// nought to one: the bodies are drawn that far from where they stood
    /// before the last step (`Character::was`, `Droid::was`) to where they
    /// stand, so a 60 Hz world moves smoothly on any screen (`set_blend`).
    /// `None` draws them where they stand — every test, probe and server.
    /// A picture setting, this window's own.
    #[cfg_attr(feature = "serde", serde(skip))]
    blend: Option<f32>,
    /// The seconds the last step was, for drawing a bolt back along its
    /// flight by the share of a step still to come (`set_blend`).
    #[cfg_attr(feature = "serde", serde(skip))]
    step_dt: f32,
}

/// How far a body may go in one step and still be blended across: a
/// tile is past any walk, sprint or roll (under nine units a step), so a
/// body that went further was put somewhere — a blink, a respawn, a room
/// taken over — and is drawn where it is.
const BLEND_JUMP: f32 = TILE;

/// Where a body that stood at `was` before the last step and stands at
/// `pos` now is drawn `blend` of the way on (`Game::set_blend`).
fn shown(was: Vec2, pos: Vec2, blend: Option<f32>) -> Vec2 {
    match blend {
        Some(t) if t < 1.0 && (pos - was).len() <= BLEND_JUMP => was + (pos - was) * t,
        _ => pos,
    }
}

impl Game {
    /// A bare room `width` by `height` — walls, a deck plate and nothing on
    /// it ([`Room::bare`]) — with [`CREW`] Bims in it: what the tests and the
    /// probes stand bodies in. Each starts somewhere in the open floor, a
    /// body's width apart, drawn off the room's stream *between* the Bims
    /// rather than all up front.
    pub fn bare(seed: u64, width: f32, height: f32) -> Game {
        let room = Room::bare(width, height);
        let rng = Rng::new(seed);
        let game = Game::with_room(
            room,
            rng,
            seed,
            CREW,
            |who, rng| {
                vec2(
                    rng.range(width * 0.30, width * 0.70),
                    rng.range(height * 0.42 + who as f32 * 0.12, height * 0.52),
                )
            },
            width,
            height,
        );
        game
    }

    /// A game in a room laid out from elsewhere — a ship design, through
    /// `crate::aboard` — with one Bim per `start`, standing there.
    ///
    /// **As many as there are `starts`: the caller has counted the crew.**
    /// A Bim is dealt a bunk to begin with — bunk `i` for Bim `i` as far as
    /// the bunks go (`Room::bunk_of`) — and a room may be given more Bims
    /// than it has bunks — the `droids` command's sixteen on a ship with
    /// five — so the bunks run out: the ones past them stand on the deck to
    /// begin with (`crate::aboard::starts`). The ship's room takes whatever
    /// the world counted, since the world has sized everything else by that
    /// number; a station's room is cut to its bunks before it gets here (the
    /// world's `Residents::open`).
    ///
    /// And possibly **none**: a station nobody lives on still has its
    /// furniture to draw, and the room is what draws it. Everything that
    /// runs a frame loops over the crew there are.
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
        let maps = Maps::new(room.interior, &room.solids(), BODY_MARGIN, room.nav_tile());
        let bims = (0..crew)
            .map(|who| {
                let at = start(who, &mut rng);
                Bim::new(who, at, &mut rng)
            })
            .collect();
        // A bunk apiece in crew order, as far as the bunks go.
        let mut room = room;
        room.bunk_of = (0..crew)
            .map(|who| (who < room.bunks()).then_some(who))
            .collect();
        let mut game = Game {
            room,
            clock: Clock::new(),
            maps,
            blockers: Vec::new(),
            doors_locked: 0,
            doors_shut: 0,
            hit_door: 0,
            hit_bim: 0,
            hit_body: 0,
            hit_visitor: 0,
            hit_bench: 0,
            hit_shelf: 0,
            hit_desk: 0,
            hit_research: 0,
            rng,
            bims,
            first_tick: 0,
            drag: None,
            order_drag: None,
            markers: Vec::new(),
            priorities: Priorities::new(),
            outside_blockers: Vec::new(),
            afield_blockers: Vec::new(),
            outside_version: None,
            visitors: Vec::new(),
            visitors_down: Vec::new(),
            visitors_hailable: Vec::new(),
            visitors_revivable: Vec::new(),
            guest_revives: Vec::new(),
            tended: Vec::new(),
            autonomous: true,
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
            withheld: false,
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
            skills: Vec::new(),
            standing: Vec::new(),
            alarm_armed: Vec::new(),
            revives: Vec::new(),
            revivers: true,
            bags_blown: Vec::new(),
            stuns: Vec::new(),
            satchels_landed: Vec::new(),
            pieces_broken: Vec::new(),
            downs: Vec::new(),
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
            told: false,
            blend: None,
            step_dt: 0.0,
        };
        game.refresh_blockers();
        game.resize(width, height);
        game
    }

    /// How many bunks the room has — the stand-in for a layout with none
    /// not counted.
    pub fn bed_count(&self) -> usize {
        self.room.bunks()
    }

    // --- whose bunk is whose ---------------------------------------------

    /// Which bunk Bim `who` was dealt, or `None` for one with none. See
    /// `Room::bunk_of`.
    pub fn bed_of(&self, who: usize) -> Option<usize> {
        self.room.bed_of(who)
    }

    /// Whose bunk `bed` is, or `None` for one nobody has.
    pub fn bed_owner(&self, bed: usize) -> Option<usize> {
        self.room.bed_owner(bed)
    }

    /// Whether bunk `bed` is somebody else's furniture: a station's, on a
    /// joined deck, by the box `set_foreign` named — not the ship's to deal
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

    /// Whether bunk `bed` is one of the ship's to deal: a real bunk, not the
    /// stand-in, and not a docked station's.
    pub fn bed_assignable(&self, bed: usize) -> bool {
        bed < self.room.bunks() && !self.bed_is_foreign(bed)
    }

    /// The first of the ship's bunks nobody has, if any.
    fn free_bed(&self) -> Option<usize> {
        (0..self.room.bunks())
            .find(|&bed| !self.bed_is_foreign(bed) && self.room.bed_owner(bed).is_none())
    }

    /// Where a body stands to get into the bunk with that index, for the
    /// tests: somewhere on the deck, in a room of its own.
    #[allow(dead_code)]
    pub fn bed_station_for_probe(&self, bed: usize) -> Vec2 {
        self.room.bed_station(bed)
    }

    /// Everybody aboard, taken out of this room for good — for a room that
    /// is being replaced by a bigger one, a docked ship's by the ship's and
    /// the station's together. Every errand is given up first, the way a
    /// blocked one is: whatever was carried goes back where it came from.
    /// What survives is the Bim — its health, its gear, where it stands —
    /// and not what it was in the middle of, because the thing it was
    /// walking to is in another room now.
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
        // arms, and a crew led by a player was carried over recruited
        // with the edge already spent.
        if self.at_war {
            self.at_war = false;
            self.muster(false);
        }
        if self.alarm || self.mustered {
            if self.alarm {
                self.arm_players(false);
            }
            self.alarm = false;
            self.mustered = false;
            self.muster_crew(false);
        }
        for bim in &mut self.bims {
            if let Some(task) = bim.task.take() {
                task.abandon(&mut bim.character, &mut self.room);
            }
            bim.queue.clear();
            bim.character.hold_main(crate::character::Held::Nothing);
            bim.character.hold_tool(crate::character::Held::Nothing);
        }
        // Whose bunk was whose goes with them, for `adopt` to read back:
        // the ship's bunks keep their numbers from one deck to the next
        // (`crate::aboard::layout_of` lists them in id order, and a joined
        // deck puts the ship's parts first).
        for (who, bim) in self.bims.iter_mut().enumerate() {
            bim.bed = self.room.bed_of(who);
        }
        self.room.bunk_of.clear();
        std::mem::take(&mut self.bims)
    }

    /// Take a crew in — from [`Game::take_crew`] on another room — standing
    /// each one `shift` from where it stood there, because the two rooms'
    /// origins differ by that. They go on the end, and the room's bunks are
    /// in the order its layout listed them, so a caller that wants the
    /// ship's crew at the ship's bunks adopts them first. Everybody comes
    /// in, bunks or no bunks — the crew is the world's count, as in
    /// [`Game::with_layout`]. Anybody whose feet would land somewhere a
    /// body cannot stand — off the deck of a room that has just shrunk — is
    /// stood at their bunk instead — or, with none, where they stood.
    ///
    /// **Its bunk comes with it** (`Bim::bed`, which `take_crew` wrote):
    /// kept when it names one of this room's own bunks that nobody here
    /// has, and else the first free one, and else none — so a hire, whose
    /// old bunk was a station's, takes whatever bunk is spare.
    pub fn adopt(&mut self, crew: Vec<Bim>, shift: Vec2) {
        for mut bim in crew {
            let who = self.bims.len();
            let bed = bim
                .bed
                .take()
                .filter(|&b| self.bed_assignable(b) && self.room.bed_owner(b).is_none())
                .or_else(|| self.free_bed());
            self.room.bunk_of.push(bed);
            // Where the feet land, snapped to the nearest cell a body fits in:
            // a spot beside a bunk sits on the edge of the bunk's inflated
            // footprint, and whether the cell under it reads free depends on
            // how the grid happened to fall. Far from anywhere free — off the
            // deck altogether — is the bunk instead.
            let at = bim.character.pos + shift;
            let free = self.maps.deck().nearest_free(at);
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
            bim.character.set_post(
                bim.character
                    .post()
                    .map(|p| p + shift)
                    .filter(|&p| (self.maps.deck().nearest_free(p) - p).len() <= 2.0 * BODY_MARGIN),
            );
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

    /// The fixed furniture plus every door that is currently something to
    /// walk into.
    fn refresh_blockers(&mut self) {
        self.blockers = self.room.solids().to_vec();
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
        // A paused frame leaves no wake: nothing has moved.
        if dt > 0.0 {
            self.combat.wakes();
        }
        // A flickering lamp spits sparks now and then.
        for (i, lamp) in self.room.sight.lamps().iter().enumerate() {
            if lamp.is_flickering() {
                self.combat.fx.lamp_sparks(i as u32, lamp.at, dt);
            }
        }
        self.age_markers(dt);
    }

    /// Whether a host ages this room's passing lights, which is when it
    /// records any (`crate::fx::Fx::is_on`).
    pub fn fx_on(&self) -> bool {
        self.combat.fx.is_on()
    }

    /// The particles this room has spawned since the host last asked
    /// (`crate::fx::Spray`), for its GPU: taken.
    pub fn take_sprays(&mut self) -> Vec<crate::fx::Spray> {
        self.combat.fx.take_sprays()
    }

    /// Spawn a spray in this room for the host's GPU — what the world's
    /// abilities look like, spawned by the host off its events. Nothing
    /// while no host ages the room.
    pub fn spray(&mut self, spray: crate::fx::Spray) {
        self.combat.fx.spray(spray);
    }

    /// How much the room has put out for the host and not had taken —
    /// its cues, the fight's, its particles — as counts: a mark to read
    /// what one step adds (task 156, a guest's rollback).
    pub fn out_mark(&mut self) -> (usize, usize, usize) {
        (
            self.room.cues.len(),
            self.combat.cues.len(),
            self.combat.fx.waiting_sprays().len(),
        )
    }

    /// What the room put out since `mark`: the cues — the room's before
    /// the fight's, as [`Game::take_cues`] hands them — and the particles.
    pub fn out_since(&mut self, mark: (usize, usize, usize)) -> (Vec<Cued>, Vec<crate::fx::Spray>) {
        let tail = |v: &[Cued], from: usize| v.get(from..).unwrap_or_default().to_vec();
        let mut cues = tail(&self.room.cues, mark.0);
        cues.extend(tail(&self.combat.cues, mark.1));
        let sprays = self.combat.fx.waiting_sprays();
        let sprays = sprays.get(mark.2..).unwrap_or_default().to_vec();
        (cues, sprays)
    }

    /// Of what the room put out since `mark`, each thing `heard` holds
    /// taken back out, one for one: a step played again after a guest's
    /// rollback is heard and shown only where it differs from the first
    /// time it was played (task 156).
    pub fn drop_heard(
        &mut self,
        mark: (usize, usize, usize),
        cues: &[Cued],
        sprays: &[crate::fx::Spray],
    ) {
        let mut cues_left: Vec<Cued> = cues.to_vec();
        let mut sift = |list: &mut Vec<Cued>, from: usize| {
            let mut i = from.min(list.len());
            while i < list.len() {
                if let Some(j) = cues_left.iter().position(|c| *c == list[i]) {
                    cues_left.swap_remove(j);
                    list.remove(i);
                } else {
                    i += 1;
                }
            }
        };
        sift(&mut self.room.cues, mark.0);
        sift(&mut self.combat.cues, mark.1);
        let mut sprays_left: Vec<crate::fx::Spray> = sprays.to_vec();
        let list = self.combat.fx.waiting_sprays();
        let mut i = mark.2.min(list.len());
        while i < list.len() {
            if let Some(j) = sprays_left.iter().position(|s| s.same(&list[i])) {
                sprays_left.swap_remove(j);
                list.remove(i);
            } else {
                i += 1;
            }
        }
    }

    /// Cues and particles put back in front of whatever is waiting: what
    /// a world thrown away by a guest's rollback had put out and the host
    /// had not taken yet (task 156).
    pub fn put_back_unread(&mut self, cues: Vec<Cued>, sprays: Vec<crate::fx::Spray>) {
        let mut rest = std::mem::replace(&mut self.room.cues, cues);
        self.room.cues.append(&mut rest);
        let waiting = self.combat.fx.waiting_sprays();
        let mut rest = std::mem::replace(waiting, sprays);
        waiting.append(&mut rest);
    }

    /// The pings on the deck grown older by `dt`, and the spent ones gone.
    fn age_markers(&mut self, dt: f32) {
        for m in &mut self.markers {
            m.age += dt;
        }
        self.markers.retain(|m| m.age < MARKER_LIFE);
    }

    /// The simulation alone, without drawing it. What the ship game calls
    /// aboard — up to twenty-four times a frame at its top speed, where a
    /// picture of every step but the last would be a picture nobody sees.
    pub fn simulate(&mut self, dt: f32) {
        // Where every body stood before this step, for the picture to
        // blend from (`set_blend`). Nothing the rules read.
        for bim in &mut self.bims {
            bim.character.was = bim.character.pos;
        }
        for droid in &mut self.droids {
            droid.was = droid.pos;
        }
        self.step_dt = dt;
        self.refresh_outside();
        self.refresh_afield();
        self.room.update(dt);
        let minutes = dt * MINUTES_PER_SECOND;

        // Where everybody stands, for the one chain that walks to a
        // crewmate. As of the top of the step, which is a frame behind for
        // whoever is ticked second — a body's width at most, and the
        // revive walk snaps to a cell beside the patient anyway.
        self.tell_the_room_where_the_crew_are();

        // Each in turn, and each entirely on its own account. Turn and turn
        // about, rather than always in crew order: whoever is ticked first
        // gets first refusal on everything shared — a bench, a site — and in
        // a fixed order that would be the same Bim winning every tie for the
        // whole game.
        let crew = self.bims.len();
        for step in 0..crew {
            self.tick_bim((self.first_tick + step) % crew, dt, minutes);
        }
        // The revives finished this step, after everybody has moved: the
        // chain says hands came off a body, and the body is looked at
        // here.
        self.apply_revives();
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
        // The blood on the deck, for everybody: a body under twenty hit
        // points drips, and a drop is a stain on the tile it lands on,
        // which the deck keeps.
        for bim in self.bims.iter_mut() {
            bim.tick_drips(dt, &mut self.rng, &mut self.room.blood);
        }
        // A room with nobody in it — a station nobody lives on — has no tie
        // to break, and no remainder to take.
        if crew > 0 {
            self.first_tick = (self.first_tick + 1) % crew;
        }

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
        if self.room.shut_doors().len() != self.doors_shut {
            self.refresh_blockers();
        }

        // The pings age here only in a room nobody draws on a real clock;
        // a host that fades the fight's lights ages them with those
        // (`fade`), so a ping lasts as long at 24× as at 1×.
        if !self.combat.fx.is_on() {
            self.age_markers(dt);
        }

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
    /// minigun's twenty in two seconds — and `reload` then waits the trigger
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
    ///
    /// **And the muzzle's own line to `at` has to be clear** — no wall,
    /// no lamp still lit (`combat::line_of_fire`) — or the eye's is taken
    /// when that one is. The gun is held over the firing shoulder, on the
    /// body's right (`GRIP_ACROSS`), so leaning out past a corner on its
    /// right the muzzle sits beside the corner the eye sees past, and its
    /// bolts clipped the wall or a lamp hanging on it; round a corner on
    /// the left the gun is on the far side and never did. With neither
    /// line clear it is the muzzle, as before.
    fn shot_from(&self, who: usize, eye: Vec2, at: Vec2) -> Vec2 {
        let Some(muzzle) = self.bims[who].character.muzzle() else {
            return eye;
        };
        let sight = &self.room.sight;
        if !sight.clear_line(eye, sight.tile_of(muzzle)) {
            return eye;
        }
        if !line_of_fire(sight, muzzle, at) && line_of_fire(sight, eye, at) {
            return eye;
        }
        muzzle
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
        // And the bolts in flight light the tiles they are in (task 152),
        // for every eye that looks this step.
        let bolts: Vec<Vec2> = self.combat.bolts.iter().map(|b| b.pos).collect();
        self.room.sight.set_flares(&bolts);
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
        // arms and fights
        // the way an enemy's people do. Nobody near and nobody hit for a
        // while, and they stand down to their errands.
        self.attacked_for = (self.attacked_for - dt).max(0.0);
        let alarm = !self.hostile_bodies
            && (self.attacked_for > 0.0
                || self.enemy_unseen_for < ALARM_HOLD
                || self.enemy_within(ALARM_RANGE * TILE));
        // And a player's own Bim takes arms with it, and puts them down
        // after if the alarm was what took them up (`arm_players`) —
        // before `led` is read below, so a Bim the alarm let go does not
        // keep the crew mustered.
        if alarm != self.alarm {
            self.arm_players(alarm);
        }
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
            // A fight is no time to have a bot picked: every player's
            // pick of one is let go as the crew take arms.
            if mustered {
                for who in self.players..self.bims.len() {
                    self.bims[who].character.selected = 0;
                }
            }
        }
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
            bim.under_fire = (bim.under_fire - dt).max(0.0);
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
            // A beam ends the same way (feature 76): the world reads the
            // flag back and breaks the link. And a surge runs its seconds
            // out.
            if bim.beaming
                && (!bim.is_alive() || bim.character.is_unconscious() || bim.character.is_outside())
            {
                bim.beaming = false;
            }
            if let Some(surge) = bim.surge.as_mut() {
                surge.left -= dt;
                if surge.left <= 0.0 {
                    bim.surge = None;
                }
            }
            bim.character.set_surging(bim.surge.is_some());
            // A relic's shield (task 142) runs its seconds out the same way,
            // and a Bastion's (task 155) drains as it goes.
            if let Some(shield) = bim.shield.as_mut() {
                shield.left -= dt;
                shield.hp -= shield.drain * dt;
                if shield.left <= 0.0 || shield.hp <= 0.0 || !bim.is_alive() {
                    bim.shield = None;
                }
            }
            bim.character
                .set_shield(bim.shield.map_or(0.0, |s| s.share()));
            // Stunned by a Stun Shot (October 2026): the stun wears off,
            // and until it has the body neither aims, fires, swings nor
            // walks — whoever's side it fights for.
            bim.stunned = (bim.stunned - dt).max(0.0);
            bim.character.set_stunned(bim.stunned > 0.0);
            if bim.stunned > 0.0 {
                bim.trigger.hold();
                bim.character.trigger_paid();
                bim.locked = None;
                bim.blow = None;
                bim.peek = None;
                bim.character.set_lean(None);
                bim.character.set_aim(None);
                bim.character.halt();
                continue;
            }
            let skill = self.shot_skill(who);
            // A Manufacturer among a site's own people (task 131) fights
            // off the machines' list, and nothing below is its.
            if self.is_intruder(who) {
                self.tick_intruder(who, dt);
                continue;
            }
            // A commander's Medivac medic runs to a player downed, whatever
            // the fight round the body (a revive already under way goes on).
            if self.is_bot(who) {
                self.medivac_rush(who);
            }
            // A bot's revive gives way to the fight: the moment it has
            // something to shoot at, the revive is put down for good and
            // the weapon comes out this very step. It began with the
            // patient out of harm (`revive_on_offer`), and a bot kneeling
            // while an enemy walked up and shot it was a bot that never
            // fired back.
            if self.care_gives_way(who, &skill)
                && let Some(task) = self.bims[who].task.take()
            {
                task.abandon(&mut self.bims[who].character, &mut self.room);
            }
            let bim = &mut self.bims[who];
            // Not while reviving: both hands are on the patient, so the
            // weapon is holstered for the whole errand — the walk included,
            // since a bot that fired on its way over would be a bot that
            // never got there — and drawn again after (task 120: the
            // reviver stands still and cannot fire).
            let reviving = bim
                .task
                .as_ref()
                .is_some_and(|t| matches!(t.kind(), Kind::Revive { .. }));
            // Nor while laying a kit: the hands are on the build — the
            // walk over is armed.
            let laying = bim.task.as_ref().is_some_and(Task::is_laying);
            // In somebody's arms (feature 86): it goes where they go and
            // shoots nothing on the way.
            let carried = self.is_carried(who);
            let bim = &mut self.bims[who];
            // A body its player steers (task 144) has its weapon out
            // whenever it can, recruited or not: it fires when the button
            // is down and never of its own accord.
            let steer = bim.character.steer();
            let armed = bim.is_alive()
                && (bim.character.is_recruited() || bim.braced || steer.is_some())
                && bim.gear.weapon.is_some()
                && !bim.character.is_outside()
                && !bim.character.is_unconscious()
                && !reviving
                && !laying
                // A medic beaming holds its fire (feature 76).
                && !skill.holds_fire
                // And so does one with a crewmate in its arms (feature
                // 86): both hands are the carry.
                && bim.carrying.is_none()
                // A body being carried shoots nothing either.
                && !carried
                // And the medkit in hand is no weapon (task 138).
                && bim.hand == Hand::Weapon;
            // The medkit is drawn in the hand while it is picked and the
            // hands are free for it.
            let kit = bim.hand == Hand::Medkit
                && bim.is_alive()
                && !bim.character.is_unconscious()
                && !bim.character.is_outside()
                && bim.carrying.is_none()
                && !carried;
            bim.character.set_medkit(kit);
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
            // How far a reload of the gun in the hands has come, for the
            // picture of it (October 2026).
            let through = weapon.map_or(0.0, |w| {
                let reload = w.stats().reload_time.max(1e-3);
                let trigger = &bim.trigger;
                if trigger.loaded == w.kind.code() && trigger.is_reloading() {
                    1.0 - trigger.reloading / reload
                } else {
                    0.0
                }
            });
            bim.character.set_reload(through);
            // Off war, a hostile body posted beyond a locked door forces
            // its way to the post (a raider's boarders were, sent to the
            // ship's gangway with the airlock shut against them, until the
            // raids went in feature 104). Before the weapon is asked for,
            // since a body off war is not under arms.
            if self.hostile_bodies && !war && bim.character.post().is_some() {
                self.breach(who, dt);
            }
            let bim = &mut self.bims[who];
            // An enemy it was told to attack that is down, dead or off
            // the list is the end of the order (task 126), and so is the
            // body itself going down.
            if bim.focus.is_some_and(|enemy| {
                !bim.is_alive()
                    || bim.character.is_unconscious()
                    || self.combat.targets().get(enemy).is_none_or(|t| t.is_none())
            }) {
                bim.focus = None;
            }
            let focus = bim.focus;
            let Some(weapon) = weapon else {
                bim.trigger.hold();
                bim.locked = None;
                bim.blow = None;
                bim.peek = None;
                bim.character.set_lean(None);
                bim.character.set_aim(None);
                self.keep_attack_moving(who, false);
                // A field medic bot with a body in its arms has no weapon
                // out, so it never reaches `bot_stand`: its carry is
                // walked here — away from the fight, and set down the
                // moment it stands out of harm, which with the wave down
                // is at once. Left in the arms the body could not be
                // revived (`can_be_revived`), and ran its countdown out
                // there.
                if self.is_bot(who) && self.is_field_medic(who) && self.bims[who].carrying.is_some()
                {
                    self.rescue(who, dt);
                }
                continue;
            };
            // The weapon's numbers through the soldier's skill (feature 75):
            // everybody else's are the weapon's own.
            let stats = skill.stats(weapon);
            // The magazine is counted for the weapon in the hand: one put
            // there since comes full (October 2026).
            self.bims[who].trigger.load(weapon.kind);
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
            if steer.is_some() {
                // Its player's keys walk it, and nothing here plans a
                // stand for it (task 144).
            } else if let Some(enemy) = focus
                && !seen_to
            {
                // The player's own attack order (task 126) comes before
                // anything the body would pick for itself: after that
                // enemy until it has a shot.
                self.chase(who, dt, &stats, enemy);
            } else if war && !seen_to {
                self.plan_stand(who, dt, &stats);
                // And, with nobody it can get to, the doors in the way.
                self.breach(who, dt);
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
                        blow.flat,
                    );
                    if self.hostile_bodies {
                        self.combat.sign_last_shot(who);
                    }
                }
            }
            let bim = &mut self.bims[who];

            // Sprinting or rolling (task 150) its player's Bim fires
            // nothing and swings nothing: the weapon is carried across
            // the chest, or stowed for the roll, and a click meanwhile is
            // owed nothing.
            if steer.is_some() && bim.character.is_dashing() {
                bim.trigger.hold();
                bim.character.trigger_paid();
                bim.locked = None;
                bim.peek = None;
                bim.character.set_lean(None);
                bim.character.set_aim(None);
                continue;
            }

            // A Stun Shot charging (October 2026; `set_braced`): the gun up
            // where it faces and nothing fired from it, nor swung, while the
            // shot charges — walking or not — and a click meanwhile is owed
            // nothing.
            if bim.braced {
                bim.trigger.hold();
                bim.character.trigger_paid();
                bim.locked = None;
                bim.peek = None;
                bim.character.set_lean(None);
                let at = from + Vec2::from_angle(bim.character.heading) * stats.reach();
                bim.character.set_aim(steer.is_some().then_some(at));
                continue;
            }

            // A melee first: locked, it neither aims nor fires, and the
            // burst it was in the middle of is over.
            let locked = self.combat.melee_with(&self.room.sight, from, &stats);
            bim.locked = locked;
            if locked.is_some() {
                self.keep_attack_moving(who, true);
            }
            let bim = &mut self.bims[who];
            if let Some(enemy) = locked {
                bim.trigger.hold();
                bim.peek = None;
                bim.character.set_lean(None);
                bim.character.set_aim(None);
                if let Some(at) = self.combat.targets()[enemy].map(|t| t.at)
                    && steer.is_none()
                {
                    bim.character.face((at - from).angle());
                }
                if bim.melee_timer <= 0.0 {
                    bim.melee_timer = MELEE_PERIOD;
                    // A blow's damage through the soldier's *bruiser*, fist
                    // and blade alike (feature 75).
                    // And the flat damage, before any factor, for Weak
                    // Spot's crit (task 124).
                    let (damage, cut, swing, flat) = if stats.melee {
                        let flat = weapon.stats().damage;
                        (stats.damage * skill.melee, true, Action::Swing, flat)
                    } else {
                        (FIST_DAMAGE * skill.melee, false, Action::Punch, FIST_DAMAGE)
                    };
                    bim.character.antic(swing, SWING_TIME);
                    bim.blow = Some(Blow {
                        target: enemy,
                        left: SWING_TIME,
                        damage,
                        cut,
                        flat,
                    });
                }
                continue;
            }
            // A blade with nobody in reach has nothing to aim.
            if stats.melee {
                bim.peek = None;
                bim.character.set_lean(None);
                bim.character.set_aim(None);
                self.keep_attack_moving(who, false);
                continue;
            }
            // A body its player steers (task 144) aims where it faces —
            // turned to the pointer at once — and fires along it
            // whenever the button is down and the weapon ready, a burst
            // and all; nothing is picked for it.
            if let Some(s) = steer {
                let heading = bim.character.heading;
                let at = from + Vec2::from_angle(heading) * stats.reach();
                bim.peek = None;
                bim.character.set_lean(None);
                bim.character.set_aim(Some(at));
                // A click is a shot even if the button came up before
                // this step saw it held (`TRIGGER_OWED`). A pistol
                // (`WeaponKind::semi_automatic`) fires every click once
                // `SEMI_AUTO_COOLDOWN` has passed since its last shot — a
                // click inside it waits for it, owed — and held down, a
                // shot every time the cooldown runs out.
                let semi = weapon.kind.semi_automatic();
                let owed = if semi && bim.trigger.reload > 0.0 {
                    bim.character.trigger_pending()
                } else {
                    bim.character.trigger_owed(dt)
                };
                if !s.fire && !owed {
                    bim.trigger.hold();
                    continue;
                }
                let fired = if semi {
                    let cooldown = crate::balance::SEMI_AUTO_COOLDOWN / skill.fire_rate.max(1e-3);
                    bim.trigger.press(cooldown, &stats)
                } else {
                    bim.trigger.pull(dt, &stats)
                };
                if fired {
                    bim.character.trigger_paid();
                    let walking = bim.character.is_walking();
                    let muzzle = self.shot_from(who, from, at);
                    self.reveal(who);
                    let shot = self.fired_skill(who, &skill);
                    self.bims[who].shots = self.bims[who].shots.saturating_add(1);
                    self.combat
                        .fire_along(muzzle, heading, weapon, walking, &shot, Some(who));
                }
                continue;
            }
            // A target the player named (task 126) is the only one it
            // fires at: out of sight or out of reach, it holds its fire
            // and walks after it (`chase`).
            let aimed = match focus {
                Some(enemy) => self.combat.aim_only(&self.room.sight, from, &stats, enemy),
                None => self.combat.aim(&self.room.sight, from, &stats),
            };
            // An attack-move stands still for a shot and walks on without
            // one, before the walk is read below.
            self.keep_attack_moving(who, aimed.is_some());
            let bim = &mut self.bims[who];
            let Some((_, eye, at)) = aimed else {
                // Nothing to shoot at, and the magazine short of full: a body
                // nobody steers reloads it now rather than mid-fight.
                bim.trigger.reload_now(&stats);
                bim.trigger.hold();
                bim.peek = None;
                bim.character.set_lean(None);
                bim.character.set_aim(None);
                continue;
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
                let muzzle = self.shot_from(who, eye, at);
                self.reveal(who);
                if self.hostile_bodies {
                    self.lit_muzzle(muzzle, at, weapon);
                    self.combat.shoot(muzzle, at, weapon, walking);
                    self.combat.sign_last_shot(who);
                } else {
                    // Every fifth shot of an *Overcharge Cell* (feature
                    // 106): counted on the body, the shot's damage raised.
                    let shot = self.fired_skill(who, &skill);
                    self.bims[who].shots = self.bims[who].shots.saturating_add(1);
                    self.combat
                        .fire_as(muzzle, at, weapon, false, walking, &shot, Some(who));
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
        // A reload begun this step by the crew is heard (October 2026).
        self.say_reloads();
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
        // the crew's deck alone. It reloads as its gun does — the
        // minigun's hundred and four seconds since October 2026, the
        // player's word; it never ran dry from feature 88 until then.
        if !self.hostile_bodies {
            for i in 0..self.sentries.len() {
                let sentry = self.sentries[i];
                // A Healing Sentry has no barrel (task 127).
                if sentry.heals {
                    continue;
                }
                let stats = sentry.skill.stats(sentry.weapon);
                self.sentries[i].trigger.tick(dt);
                self.sentries[i].flash = (self.sentries[i].flash - dt).max(0.0);
                // Its sensor sees in the dark: a machine standing where
                // the lamps are shot out is fired at like one in the light.
                let aim = self
                    .combat
                    .aim_in_the_dark(&self.room.sight, sentry.at, &stats);
                let Some((_, _, at)) = aim else {
                    self.sentries[i].trigger.hold();
                    continue;
                };
                // The barrel swings onto what it aims at: the picture's
                // alone, since the shot below is fired at `at` whatever
                // way it points.
                let to = at - sentry.at;
                self.sentries[i].turn_toward(to.y.atan2(to.x), dt);
                if self.sentries[i].trigger.pull(dt, &stats) {
                    self.sentries[i].flash = crate::combat::SENTRY_FLASH;
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
            // armour to dodge with and never peeking — so a hit past the
            // crew's count is a hit on a sentry.
            let crew = self.bims.len();
            let mut bodies: Vec<Option<(Vec2, bool, f32)>> = self
                .bims
                .iter()
                .enumerate()
                .map(|(who, b)| {
                    // An intruder (task 131) is the machines' side: their
                    // bolts and its own fly past it.
                    (b.is_alive()
                        && !b.character.is_outside()
                        && !b.character.is_unconscious()
                        && !(b.manufacturer && !self.hostile_bodies))
                        .then_some((
                            b.peek.unwrap_or(b.character.pos),
                            b.peek.is_some(),
                            // Its armour's odds, and a braced soldier's *dug
                            // in* on top (feature 75). Rolling (task 150),
                            // every bolt and beam is slipped.
                            if b.character.is_rolling() {
                                1.0
                            } else {
                                (b.gear.dodge() + self.skill(who).dodge).min(1.0)
                            },
                        ))
                })
                .collect();
            for sentry in &self.sentries {
                bodies.push(Some((sentry.at, false, 0.0)));
            }
            // The Riot Shields up face the way their holders face now
            // (task 155), the step's turn included.
            let plates: Vec<crate::combat::Plate> = self
                .combat
                .plates()
                .iter()
                .map(|p| crate::combat::Plate {
                    facing: self.plate_facing(p.who),
                    ..*p
                })
                .collect();
            self.combat.set_plates(plates);
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
            self.strike_stripping(hit.who, hit.damage, hit.cut, hit.strips);
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
            // A kit being laid is laid on under arms: it was begun for
            // the fight.
            if war && !self.is_deploying(who) {
                self.interrupt(who);
            }
            if war {
                // A post is where peace put it; the fight puts it elsewhere.
                self.bims[who].character.set_post(None);
                self.bims[who].plan_wait = 0.0;
            }
            // A hunt ends with the war.
            self.bims[who].hunting = false;
        }
    }

    /// The crew's alarm, on or off: every living crew member but the
    /// player's own under arms with its errand put down — or let go, the queue kept, so peace picks it all up
    /// again. The player's own is left as the player has it: recruiting
    /// it is the player's.
    fn muster_crew(&mut self, alarm: bool) {
        let attacked = self.under_attack();
        for who in 0..self.bims.len() {
            if (self.is_player(who) && !attacked) || !self.bims[who].is_alive() {
                continue;
            }
            // An intruder (task 131) is not this side's to muster: it
            // takes arms of its own accord (`tick_intruder`).
            if self.is_intruder(who) {
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

    /// The alarm going up or down, for the players' own Bims (September
    /// 2026, the user's word: a Bim takes arms by itself when combat
    /// happens). Going up, every player's own Bim alive, awake and on the
    /// deck that is not recruited already is recruited — its weapon out,
    /// shooting what it sees — and nothing else: no errand put down, no
    /// post dropped, no stand planned, since only the player's input moves
    /// it. Going down, a Bim the alarm recruited is let go again, unless
    /// the player has given it an attack of its own since; one the player
    /// recruited stays as it was. Only the crew's room — the room whose
    /// people revive one another — has players to arm; a station's or a
    /// town's first body is nobody's.
    fn arm_players(&mut self, alarm: bool) {
        if self.hostile_bodies || !self.revivers {
            return;
        }
        let players = self.players.min(self.bims.len());
        if self.alarm_armed.len() < players {
            self.alarm_armed.resize(players, false);
        }
        for who in 0..players {
            let bim = &mut self.bims[who];
            if alarm {
                let fit = bim.is_alive()
                    && !bim.character.is_unconscious()
                    && !bim.character.is_outside();
                if fit && !bim.character.is_recruited() {
                    bim.character.set_recruited(true);
                    self.alarm_armed[who] = true;
                }
            } else if std::mem::take(&mut self.alarm_armed[who])
                && bim.attack_move.is_none()
                && bim.focus.is_none()
            {
                bim.character.set_recruited(false);
            }
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

    /// One crew member put under arms: the errand put down onto the
    /// queue. What it holds and wears is its loadout, which changes only
    /// between missions (task 113), so there is nothing to take out.
    /// What the alarm does to each of them.
    fn take_up_arms(&mut self, who: usize) {
        // A kit being laid is laid on under arms, as in `muster`.
        if !self.is_deploying(who) {
            self.interrupt(who);
        }
        self.bims[who].plan_wait = 0.0;
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

    /// Whether the deck is **clear**: not one of the targets the world
    /// named is still up — the wave just cleared, the last of it down.
    /// Unlike [`Game::calm`] it needs no twenty seconds of quiet: the bots
    /// go to the downed the step it comes true (`revive_on_offer`).
    pub fn deck_clear(&self) -> bool {
        !self.combat.targets().iter().any(Option::is_some)
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
    /// body with no gear and no errands: what is left is the
    /// walk, the lock, the aim and the trigger.
    fn tick_droids(&mut self, dt: f32, war: bool) {
        for i in 0..self.droids.len() {
            self.droids[i].walk(dt);
            if self.droids[i].destroyed {
                continue;
            }
            self.droids[i].under_fire = (self.droids[i].under_fire - dt).max(0.0);
            // A machine held for a picture stands where it was put and
            // does nothing at all (`World::stage_droids_for_probe`).
            if self.droids[i].posing {
                continue;
            }
            // A machine stunned by an EMP (task 127) neither moves, turns,
            // aims nor fires until the stun wears off; `Droid::stun`
            // dropped whatever it had begun.
            if self.droids[i].is_stunned() {
                let d = &mut self.droids[i];
                d.wear_off_stun(dt);
                d.trigger.hold();
                d.halt();
                d.charging(dt, false);
                continue;
            }
            self.droids[i].trigger.tick(dt);
            self.droids[i].melee_timer = (self.droids[i].melee_timer - dt).max(0.0);
            if let Some(blow) = self.droids[i].blow.as_mut() {
                blow.left -= dt;
            }
            let stats = self.droids[i].stats();
            let weapon = self.droids[i].weapon;
            // With nobody it may pick — its targets taken off the list by a
            // cloak (task 130) — it holds where it stands, facing as it
            // was, rather than walking on after one it may not pick; and it
            // fires nothing, as below.
            if !war && self.withheld {
                self.droids[i].halt();
            }
            // The Guardian fights by a rule of its own (feature 100): a
            // heading it turns in whole sub-steps, a shield in front and a
            // beam wound up before it is swept.
            if self.droids[i].is_guardian() {
                self.tick_guardian(i, dt, war, &stats);
                continue;
            }
            // The Machine Heart's machines stand where they were built
            // (feature 108): a conduit and a fabricator do nothing at all,
            // and the core sweeps its beams from where it stands.
            if self.droids[i].kind.is_structure() {
                self.tick_structure(i, dt, war);
                continue;
            }
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
                            blow.flat,
                        );
                        self.combat.sign_last_shot(self.bims.len() + i);
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
                        flat: damage,
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
                self.reveal(self.bims.len() + i);
                // The glow at the gun it is built with, not at the eye
                // the shot is traced from (feature 98).
                let muzzle = self.droids[i].muzzle();
                self.lit_muzzle(muzzle, at, weapon);
                if mark < cross {
                    // Across the seam, as a machine's shot always was:
                    // recorded here and flown by the world in the crew's
                    // room, where the body it is aimed at actually is.
                    self.combat.shoot(eye, at, weapon, walking);
                    self.combat.sign_last_shot(self.bims.len() + i);
                } else {
                    // At one of this room's own bodies (feature 94): the
                    // bolt flies **here**, hostile, and `Combat::step`
                    // finds the townsperson it was aimed at.
                    self.combat.fire(eye, at, weapon, true, walking);
                }
            }
        }
    }

    /// One step of a **Guardian** (feature 100). It has no melee lock and
    /// no peek: it turns to face the nearest crew body it sees — a
    /// taunt's pull first, as for any enemy (`Combat::aim_among`) — at no
    /// more than its turn rate, walks where its stand says with its shield
    /// before it, and when its Sweeper is ready and the target is within
    /// [`crate::droid::WINDUP_COS`] of its heading it plants its feet and
    /// **winds up**: the target and the aim fixed, the heading held, and
    /// after `SWEEPER_WINDUP` the beam let go. Then it cools, and turns
    /// and walks again. With nothing to fight a wind-up is dropped and it
    /// turns the way it walks. Legs gone, it still turns and fires.
    fn tick_guardian(&mut self, i: usize, dt: f32, war: bool, stats: &WeaponStats) {
        use crate::droid::Beam;
        {
            let d = &mut self.droids[i];
            d.trigger.hold();
            d.locked = None;
            d.blow = None;
            d.peek = None;
            if let Beam::Cooling { left } = d.beam {
                d.beam = if left - dt <= 0.0 {
                    Beam::Ready
                } else {
                    Beam::Cooling { left: left - dt }
                };
            }
        }
        // Its grenade is its own rhythm, beside the beam's: dropped
        // whatever the beam is doing.
        self.drop_guardian_grenade(i, dt, war);
        // A sweep let go runs to its end, fight or no fight: the beam is in
        // the air, and the heading is held until it is done.
        if let Beam::Sweep { left, aim, side } = self.droids[i].beam {
            let d = &mut self.droids[i];
            if d.is_walking() {
                d.halt();
            }
            d.turn_toward(Vec2::ZERO, dt);
            d.beam = if left - dt > 0.0 {
                Beam::Sweep {
                    left: left - dt,
                    aim,
                    side,
                }
            } else {
                Beam::Cooling {
                    left: crate::balance::SWEEPER_COOLDOWN,
                }
            };
            return;
        }
        if !war {
            let d = &mut self.droids[i];
            if d.beam.holds_heading() {
                d.beam = Beam::Ready;
            }
            d.charging(dt, false);
            let want = d.walking_toward().unwrap_or(Vec2::ZERO);
            d.turn_toward(want, dt);
            return;
        }
        if let Beam::WindUp {
            left,
            aim,
            at,
            mark,
        } = self.droids[i].beam
        {
            // Planted: nothing walks and nothing turns until it has let go.
            let d = &mut self.droids[i];
            if d.is_walking() {
                d.halt();
            }
            d.turn_toward(Vec2::ZERO, dt);
            d.charging(dt, true);
            if left - dt > 0.0 {
                d.beam = Beam::WindUp {
                    left: left - dt,
                    aim,
                    at,
                    mark,
                };
                return;
            }
            self.let_the_beam_go(i, aim, at, mark);
            return;
        }
        self.plan_droid_stand(i, dt, stats);
        self.breach_droid(i, dt);
        let from = self.droids[i].pos;
        // Only what it sees from its own eyes: a Guardian never leans out
        // of cover, having none but its shield.
        let sighted =
            Combat::aim_among(self.combat.machine_targets(), &self.room.sight, from, stats)
                .filter(|&(_, eye, _)| eye == from);
        let d = &mut self.droids[i];
        d.charging(dt, false);
        let want = sighted
            .map(|(_, _, at)| at - from)
            .or_else(|| d.walking_toward())
            .unwrap_or(Vec2::ZERO);
        d.turn_toward(want, dt);
        if d.beam == Beam::Ready
            && let Some((mark, _, at)) = sighted
        {
            let aim = (at - from).normalize_or_zero();
            if aim != Vec2::ZERO && d.facing().dot(aim) >= crate::droid::WINDUP_COS {
                if d.is_walking() {
                    d.halt();
                }
                d.beam = Beam::WindUp {
                    left: crate::balance::SWEEPER_WINDUP,
                    aim,
                    at,
                    mark,
                };
            }
        }
    }

    /// A Guardian's **grenade** (October 2026): its wait run down, and with
    /// it out, a fight on and a body of the crew's side standing within
    /// [`crate::balance::GUARDIAN_GRENADE_TRIGGER`] tiles of its middle
    /// with a clear line, one dropped at its feet, the wait set to
    /// [`crate::balance::GUARDIAN_GRENADE_COOLDOWN`]. Recorded as a `Shot`
    /// for the world to lay in the crew's room, and laid here as well,
    /// unseen, when the machines' list has bodies of this room's own on
    /// it — a town the crew are defending — as a Sweeper's beam is. What
    /// it does at the centre is the Sweeper's damage (its tier and arms
    /// counted) times [`crate::balance::GUARDIAN_GRENADE_DAMAGE`].
    fn drop_guardian_grenade(&mut self, i: usize, dt: f32, war: bool) {
        use crate::balance::{
            GUARDIAN_GRENADE_COOLDOWN, GUARDIAN_GRENADE_DAMAGE, GUARDIAN_GRENADE_TRIGGER,
        };
        let d = &mut self.droids[i];
        d.grenade_wait = (d.grenade_wait - dt).max(0.0);
        if !war || d.grenade_wait > 0.0 {
            return;
        }
        let at = d.pos;
        let reach = GUARDIAN_GRENADE_TRIGGER * TILE;
        let near = self
            .combat
            .machine_targets()
            .iter()
            .flatten()
            .any(|t| !t.stale && (t.at - at).len() <= reach && self.line_clear(at, t.at));
        if !near {
            return;
        }
        let d = &mut self.droids[i];
        d.grenade_wait = GUARDIAN_GRENADE_COOLDOWN;
        let damage = d.stats().damage * GUARDIAN_GRENADE_DAMAGE;
        let weapon = d.weapon;
        self.reveal(self.bims.len() + i);
        self.combat.shoot_grenade(at, weapon, damage);
        if self.combat.machine_cross() < self.combat.machine_targets().len() {
            self.combat.drop_hostile_grenade(at, damage, true);
        }
    }

    /// A Guardian's grenade recorded in the other room, laid in this one
    /// by the world (October 2026): lying at `at`, bursting on this room's
    /// own bodies with `damage` at the centre — see
    /// `Combat::drop_hostile_grenade`.
    pub fn enemy_grenade(&mut self, at: Vec2, damage: f32) {
        self.combat.drop_hostile_grenade(at, damage, false);
    }

    /// The Sweeper let go at the end of a wind-up (feature 100): the beam
    /// out of the lens, from `SWEEPER_ARC_DEGREES / 2` one side of `aim`
    /// round to as far the other — the side it starts from alternating
    /// sweep to sweep — at the beam's reach, each body it crosses taking
    /// the Sweeper's damage (the arms counted). Recorded as a `Shot` for
    /// the world to lay in the crew's room, and laid **here** as well when
    /// the machines' list has bodies of this room's own on it — a town the
    /// crew are defending — where it is not drawn, the crew's room drawing
    /// the one beam. `at` and `mark` are what the wind-up was fixed on;
    /// the beam is the arc, whoever stands in it.
    fn let_the_beam_go(&mut self, i: usize, aim: Vec2, at: Vec2, mark: usize) {
        use crate::droid::Beam;
        let _ = (at, mark);
        let d = &mut self.droids[i];
        let side = if d.sweeps % 2 == 0 { 1.0 } else { -1.0 };
        d.sweeps = d.sweeps.wrapping_add(1);
        d.fired();
        let stats = d.stats();
        let weapon = d.weapon;
        let from = d.muzzle();
        d.beam = Beam::Sweep {
            left: crate::balance::SWEEPER_SWEEP,
            aim,
            side,
        };
        self.reveal(self.bims.len() + i);
        self.lay_beam(from, aim, side, weapon, stats.reach(), stats.damage, 1.0);
        self.combat.sign_last_shot(self.bims.len() + i);
    }

    /// A Sweeper's beam out of `from` along `aim`, swept from `side` of it
    /// round to the other, `reach` long, `damage` to each body it crosses
    /// and `pace` times a Guardian's speed: recorded as a `Shot` for the
    /// world to lay in the crew's room, and laid here as well when the
    /// machines' list has bodies of this room's own on it. A Guardian's
    /// (feature 100) and a Machine Heart core's (feature 108).
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn lay_beam(
        &mut self,
        from: Vec2,
        aim: Vec2,
        side: f32,
        weapon: Weapon,
        reach: f32,
        damage: f32,
        pace: f32,
    ) {
        use crate::combat::{SWEEP_HALF_COS, SWEEP_HALF_SIN};
        let start = from + aim.rotate_by(SWEEP_HALF_COS, -side * SWEEP_HALF_SIN) * reach;
        let end = from + aim.rotate_by(SWEEP_HALF_COS, side * SWEEP_HALF_SIN) * reach;
        self.combat
            .shoot_sweep(from, start, end, weapon, damage, pace);
        if self.combat.machine_cross() < self.combat.machine_targets().len() {
            self.combat
                .sweep(from, start, end, weapon, damage, false, pace);
        }
    }

    /// A Guardian's Sweeper recorded in the other room, laid in this one
    /// by the world (feature 100): the beam from `from`, turning from the
    /// aim at `start` round to the aim at `end`, over this room's own
    /// bodies, `pace` times a Guardian's speed — see `Combat::sweep`.
    pub fn enemy_sweep(
        &mut self,
        from: Vec2,
        start: Vec2,
        end: Vec2,
        weapon: Weapon,
        damage: f32,
        pace: f32,
    ) {
        self.combat
            .sweep(from, start, end, weapon, damage, true, pace);
    }

    /// The Guardians' beams being swept in this room (feature 100).
    pub fn sweeps(&self) -> &[crate::combat::Sweep] {
        self.combat.sweeps()
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
            // Legs gone: it fights where it stands, and plans nothing
            // (all but a Husk, which crawls on, `Droid::crawling`).
            return;
        }
        let from = self.droids[i].pos;
        let nav = self.maps.for_body(false);
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
        let aim = (!stats.melee)
            .then(|| Combat::aim_among(&targets, &self.room.sight, from, stats))
            .flatten();
        // A Warden shoots from a peek as well as its own eye; a Trooper
        // and a Guardian from their own eye alone, so only that is a shot
        // to them. Each weighs its stands by what it could shoot from
        // there (`Seeing`, `Tactics::stand_scored`).
        let peeks = self.droids[i].kind.takes_cover();
        let seeing = if peeks {
            crate::combat::Seeing::MadeOut
        } else {
            crate::combat::Seeing::OwnEye
        };
        let shot = aim
            .filter(|&(_, eye, _)| peeks || eye == from)
            .map(|(_, _, at)| at);
        // A Guardian weighs no tile of distance for its own sake: the
        // Sweeper's worth falling off past its sweet range
        // (`balance::SWEEPER`) is what places it, so it walks in to that
        // range rather than standing off at the beam's full reach. Nor
        // does any machine with a shot: the far end of its reach is where
        // it goes looking for one, not where it runs to from one.
        let distance_worth = if self.droids[i].is_guardian() || shot.is_some() {
            0.0
        } else {
            crate::combat::DISTANCE_WORTH
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
            distance_worth,
            seeing,
        ) else {
            return;
        };
        let to = stand.at;
        // A shot from here and nothing better than the open over there:
        // it stops where it is and shoots — except a Trooper or a
        // Guardian, which advance in the open.
        let advances = !peeks;
        if shot.is_some() && !stand.cover && !advances {
            if self.droids[i].is_walking() {
                self.droids[i].halt();
            }
            return;
        }
        // A machine with a shot holds or closes: a stand farther from
        // what it is shooting at than it is now — cover included — is
        // ground given, and it stays and shoots instead (the user's
        // report: Troopers ran off to the end of their reach rather than
        // fire).
        if let Some(at) = shot
            && (to - at).len() > (from - at).len()
        {
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
        let nav = self.maps.for_body(false);
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
    fn plan_stand(&mut self, who: usize, dt: f32, stats: &WeaponStats) {
        let bim = &mut self.bims[who];
        bim.plan_wait -= dt;
        if bim.plan_wait > 0.0 {
            return;
        }
        bim.plan_wait = PLAN_EVERY;
        let from = bim.character.pos;
        let nav = self.maps.for_body(false);
        let targets = self.combat.targets().to_vec();
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
    /// 2026): a hostile body sent somewhere behind a locked door — a
    /// raider's boarders were, posted at the ship's gangway with the
    /// airlock shut against them, until feature 104 — forces the door in
    /// its way and then walks on (`return_to_post`).
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
                let door = &mut self.room.doors[i];
                let centre = door.rect.center();
                if let Some(cue) = door.smash(who, dt) {
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
        let nav = self.maps.for_body(false);
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
            .filter(|&p| self.leads(p))
            .map(|p| self.bims[p].character.pos)
            .min_by(|a, b| (*a - from).len().total_cmp(&(*b - from).len()))
            .unwrap_or(self.bims[PLAYER].character.pos);
        self.gather_round(who, dt, player);
    }

    /// A crewmate with nothing in its sight that is being shot at, or has
    /// no player up to keep to, going after the nearest enemy it can walk
    /// to ([`Tactics::charge`]) on its own `plan_wait` clock, until one is
    /// in sight and `plan_stand` takes over. A stand scored from where it
    /// is reads being blind to the enemy as cover, and held it where a
    /// machine out in the dark was shooting it.
    fn seek(&mut self, who: usize, dt: f32) {
        let bim = &mut self.bims[who];
        bim.plan_wait -= dt;
        if bim.plan_wait > 0.0 {
            return;
        }
        bim.plan_wait = PLAN_EVERY;
        let from = bim.character.pos;
        let going = bim.character.destination().unwrap_or(from);
        let nav = self.maps.for_body(false);
        let Some(to) = Tactics::charge(nav, from, self.combat.targets()) else {
            return;
        };
        if (to - going).len() <= TILE {
            return;
        }
        let route = nav.path(from, to);
        if !route.is_empty() {
            self.bims[who].character.follow_path(route);
        }
    }

    /// Whether a player's own Bim is one for the crew to gather round:
    /// alive, on its feet — not downed — and on the deck.
    fn leads(&self, p: usize) -> bool {
        let bim = &self.bims[p];
        bim.is_alive() && !bim.health.downed() && !bim.character.is_outside()
    }

    /// The same ring round a point of the room rather than round a
    /// player's Bim: an attack banner's, or the ship's for a retreat.
    fn gather_round(&mut self, who: usize, dt: f32, anchor: Vec2) {
        let bim = &mut self.bims[who];
        bim.plan_wait -= dt;
        if bim.plan_wait > 0.0 {
            return;
        }
        bim.plan_wait = PLAN_EVERY;
        let from = bim.character.pos;
        let rank = (1..who).filter(|&i| self.bims[i].is_alive()).count();
        let nav = self.maps.for_body(false);
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
    /// for a bare room. It has to be that way round: on a
    /// joined deck `Room::gangway` is the *joined* design's first free
    /// airlock, and the ship's own is mated to the station, so the room
    /// answers the station's far door — which is how a retreat used to
    /// walk the crew out through the building rather than home.
    /// The middle of the room for a room with no port at all — a bare
    /// room, a design without one — since there is nowhere else to mean.
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
    /// enemy aboard it too. Read by the bots' orders, so a crew member
    /// cornered in its own ship fights where it stands whatever its
    /// player's standing order says.
    /// **A room with nowhere else in it is not a last stand.** The rule
    /// wants a ship to be cornered *in*, which means a deck with
    /// somebody else's half to it — a joined station, a town on a planet
    /// ([`Game::set_foreign`]). A bare room and a ship flying alone have
    /// none.
    fn cornered(&self, who: usize) -> bool {
        !self.hostile_bodies
            && self.foreign.is_some()
            && self.is_aboard(self.bims[who].character.pos)
            && self.enemy_aboard()
    }

    /// What a bot under arms does when nothing nearer to hand — a chain,
    /// a post its player clicked for it, a commander's squad order — has
    /// claimed it (feature 84). A field medic's rescue first, then the last
    /// stand, then whatever its player's standing order is.
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
            self.plan_stand(who, dt, stats);
            return;
        }
        match self.standing_for(who) {
            Standing::Retreat => self.fall_back_aboard(who, dt),
            Standing::Attack { at } => self.assault(who, dt, stats, at),
            Standing::Follow => {
                let from = self.bims[who].character.pos;
                // Somebody's own to gather round: any player's, up and in.
                // A player downed is nobody to follow — the crew stood in
                // a ring round the body while the machine that shot it
                // shot them — and a bot under fire fights whether or not
                // it sees who is shooting.
                let player_up = (0..self.players.min(self.bims.len())).any(|p| self.leads(p));
                let under_fire = self.bims[who].under_fire > 0.0;
                if self.combat.sees_any(&self.room.sight, from) {
                    self.plan_stand(who, dt, stats);
                } else if !player_up || under_fire {
                    self.seek(who, dt);
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
    /// [`RESCUE_LOOK`] that is downed, is in nobody's arms already, still
    /// lies in the fire, and can be walked to.
    fn worth_fetching(&self, who: usize) -> Option<usize> {
        let from = self.bims[who].character.pos;
        let nav = self.maps.for_body(false);
        let mut best: Option<(f32, usize)> = None;
        for other in 0..self.bims.len() {
            if other == who || !self.needs_rescue(other) || self.is_carried(other) {
                continue;
            }
            if self.bims[other].carrying.is_some() {
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
    /// down there — the medical row takes over from that point, since a
    /// revive wants the body out of harm and that is where it has just
    /// walked to — else go and fetch the nearest crewmate down. Whether it
    /// claimed the body this step; `false` is "nothing to do", and the
    /// ordinary bot's stand follows.
    fn rescue(&mut self, who: usize, dt: f32) -> bool {
        if self.carrying(who).is_some() {
            let here = self.bims[who].character.pos;
            if self.out_of_harm(here) {
                self.set_down(who);
                return true;
            }
            // Away from the enemy, on its own plan clock, walked with
            // somebody in its arms.
            self.bims[who].plan_wait -= dt;
            if self.bims[who].plan_wait <= 0.0 {
                self.bims[who].plan_wait = PLAN_EVERY;
                let nav = self.maps.for_body(false);
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
            let nav = self.maps.for_body(false);
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
            self.plan_stand(who, dt, stats);
        } else if (at - from).len() > BANNER_HOLD * TILE {
            self.push_towards(who, dt, at);
        } else {
            self.gather_round(who, dt, at);
        }
    }

    /// Falling back: the ring round the ship's own gangway, and nothing
    /// further once it is there.
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
        let nav = self.maps.for_body(false);
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
        // The whole walk there on the deck's grid, which the push reads
        // its ground off — none for a body out on the plain, whose grid
        // is its own window.
        let whole = if self.bims[who].character.is_afield() {
            Vec::new()
        } else {
            nav.path(from, nav.nearest_free(at))
        };
        let step = Tactics::advance(
            &self.room.sight,
            nav,
            from,
            &whole,
            &targets,
            &doorways,
            &taken,
            COVER_WORTH,
        );
        // Off the deck's box on a plain the cover lattice says nothing —
        // the grids are the body's own window out there — so the walk is
        // planned the way an order onto the plain is, leg by leg.
        let walk_it_all = |game: &mut Game| {
            if game.on_a_window(who, at) {
                game.plan_route(who, at);
            } else if !whole.is_empty() {
                game.bims[who].character.follow_path(whole.clone());
            }
        };
        let Some(to) = step else {
            walk_it_all(self);
            return;
        };
        let destination = self.bims[who].character.destination();
        let going = destination.unwrap_or(from);
        if (to - going).len() <= TILE {
            // A body standing on the best spot short of the banner is
            // not there yet: it walks on rather than stand for ever.
            if destination.is_none() && (to - from).len() <= TILE {
                walk_it_all(self);
            }
            return;
        }
        let route = nav.path(from, to);
        if !route.is_empty() {
            self.bims[who].character.follow_path(route);
        }
    }

    /// One crew member's half of the frame.
    fn tick_bim(&mut self, who: usize, dt: f32, minutes: f32) {
        self.continue_far_walk(who);
        if self.bims[who].character.is_dead() {
            // Nothing else moves for this one. The room carries on, and so
            // does the rest of the crew.
            self.move_body(who, dt);
            return;
        }

        // The body's own clock: a downed body's countdown, run on the
        // room's steps (task 120) — nothing mends on its own. It stands
        // while somebody's hands are on it (task 138): a revive under way
        // is a patient that does not die in the middle of it.
        // So does one the crew's hands are on from the joined deck, a
        // townsperson revived there (`set_tended`).
        let held = self.bims[who].health.downed()
            && (self.revive_share(who).is_some() || self.tended.get(who).copied().unwrap_or(false));
        if !held {
            self.bims[who].health.update(dt);
        }
        // Nothing hidden slows a body (October 2026, the player's word):
        // neither a downing earlier in the mission nor a crewmate close
        // by. What it carries and what its class, items and relics say.
        let pace = self.runner(who)
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

        // Downed, or up again. The errand is put down, the frame stops here
        // for this Bim, and a revive — or the countdown — ends it; the body
        // lies rather than stands. The errand goes first, so what it puts
        // down is put down standing.
        let out = self.bims[who].health.downed();
        if out != self.bims[who].character.is_unconscious() {
            if out {
                // The gun stays in the hand, holstered (`tick_combat`): a
                // loadout is never dropped (task 113).
                self.interrupt(who);
                // Nobody can see to a body walking on with a crewmate in
                // its arms: the carry is let go (`carry_the_carried`).
            }
            self.bims[who].character.knock_out(out);
        }
        self.refresh_bleeding(who);
        if out {
            self.move_body(who, dt);
            return;
        }
        // Stunned (October 2026, `tick_combat` wears it off): no errand,
        // no round, no walk — the body stands where the burst caught it.
        if self.bims[who].stunned > 0.0 {
            self.bims[who].character.halt();
            self.move_body(who, dt);
            return;
        }

        // A downed crewmate the medical row says is urgent — set to the
        // top — is revived *now*, whatever the Bim was in the middle of:
        // the errand is put down onto the queue, and picked up again when
        // the hands come off. The only row on the list that interrupts; at
        // any other number it waits its turn like the rest, and at never
        // nobody revives of their own accord. A Bim already on its way to
        // one is left to it — asking again would restart the walk every
        // frame.
        if self.autonomous
            && self.priorities.of(Job::Medical) == work::HIGHEST
            && !self.bims[who]
                .task
                .as_ref()
                .is_some_and(|t| matches!(t.kind(), Kind::Revive { .. }))
            && let Some(patient) = self.revive_on_offer(who)
        {
            self.revive_crewmate(who, patient);
        }

        // What an engineer's talents do (feature 74): a factor on a
        // build's working steps, and nothing on any other errand — the
        // task says which job it serves. (The first of the pair was a
        // craft's, which went with the crafting in task 113.)
        let (_, build) = self.work_factors.get(who).copied().unwrap_or((1.0, 1.0));
        let effort = match self.bims[who].task.as_ref().map(|t| t.kind()) {
            Some(Kind::Build { .. }) => build,
            _ => 1.0,
        };
        // And a commander's aura (feature 78): every errand, not one
        // kind of it — the only factor here that is nobody's own class.
        // A revive runs on the helper's own time and nothing else (task
        // 120: ten seconds, four for a medic).
        let effort = match self.bims[who].task.as_ref().map(|t| t.kind()) {
            Some(Kind::Revive { .. }) => 1.0,
            _ => effort * self.skill(who).effort,
        };
        // The keys walking it are the end of any errand (task 144), not
        // only the step they start to (`order_control`): one begun while
        // they were already down — a revive, a kit — is dropped, never
        // left to walk back to once they come up — its route neither.
        if self.bims[who].character.is_steered_walking() && self.bims[who].task.is_some() {
            self.drop_task(who);
            self.bims[who].character.follow_path(Vec::new());
        }
        {
            let (bims, room, maps) = (&mut self.bims, &mut self.room, &self.maps);
            let bim = &mut bims[who];
            if let Some(task) = &mut bim.task {
                task.update(dt, &mut bim.character, room, maps, effort);
                if task.is_done() {
                    bim.task = None;
                }
            }
        }
        self.pump_queue(who);
        self.consider_errand(who);
        self.return_to_post(who);
        // And a station's people keep to their routine: a guard's round, a
        // trader's desk, a worker's benches, a stroll between rooms
        // (feature 102, `crate::routine`).
        self.keep_to_routine(who, minutes);

        {
            // Blood travels on boots. Where the body was, and where this
            // frame of walking has put it — taken either side of the one
            // call that moves it, so that a chain setting a Bim down
            // somewhere is not read as a stride across the deck.
            let was = self.bims[who].character.pos;
            self.move_body(who, dt);
            let now = self.bims[who].character.pos;
            self.room.blood.track(was, now, &mut self.rng);
        }
        self.unstick(who, dt);
        self.bims[who].tick_trail(dt);
    }

    /// `Room::crew`, afresh: every body alive and on the deck, by index.
    /// Once at the top of every step, and again as a revive is ordered,
    /// since the order may come before the first step and the walk it
    /// starts picks its spot from this.
    fn tell_the_room_where_the_crew_are(&mut self) {
        self.room.crew.clear();
        self.room.crew.extend(
            self.bims
                .iter()
                .map(|b| (b.is_alive() && !b.character.is_outside()).then_some(b.character.pos)),
        );
        // And the visitors the crew may revive, by visitor index: a walk
        // to a townsperson down picks its spot the same way.
        self.room.guests.clear();
        self.room
            .guests
            .extend(self.visitors.iter().enumerate().map(|(i, &at)| {
                self.visitors_revivable
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
        // So is a body its player's keys walk on a plain (task 144): they may
        // take it past the box at any step.
        let afield = ch.is_afield()
            || ch.far().is_some()
            || (ch.is_steered_walking() && self.room.plane.is_some());
        let (interior, blockers) =
            match (outside, afield, self.maps.outside(), self.maps.afield(who)) {
                (true, _, Some(nav), _) => (nav.interior(), &self.outside_blockers),
                (_, true, _, Some(nav)) => (nav.interior(), &self.afield_blockers[who]),
                _ => (self.room.interior, &self.blockers),
            };
        self.bims[who].character.update(dt, interior, blockers);
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
        let tile = TILE;
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
        let tile = TILE;
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
            let ch = &self.bims[who].character;
            let wanted = afield || ch.far().is_some() || ch.is_steered_walking();
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
        let tile = TILE;
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
        let nav = self.maps.for_body(ch.is_outside());
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
    /// old one ended, planned from here. Where there is none — a door locked
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
    /// peace, at their whole pace, because a route is planned once and a
    /// push at a chokepoint is a deadlock waiting to happen — but a fight
    /// replans every `PLAN_EVERY` and `unstick` watches the rest, and a
    /// squad stacked on one tile is one body to shoot at and a picture
    /// nobody can read. So every pair of **recruited** bodies on their
    /// feet on the deck — an enemy's people at war, the crew under the
    /// alarm — closer than `CREW_CLEARANCE` is shoved apart, each half the
    /// overlap, along the line between them (a nudge off the combat stream
    /// when they are on one spot). Both recruited, so a recruited body is
    /// never moved by one walking past about its errands. A push into a
    /// wall is undone by the body's own push-out on its next move.
    fn separate_under_arms(&mut self) {
        let n = self.bims.len();
        let up = |b: &Bim| {
            b.is_alive()
                && b.character.is_recruited()
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
                // A player's own Bim is moved by its player alone, so a
                // bot beside it gives the whole of the ground; two
                // players' own, or two bots, halve it.
                let (held_a, held_b) = (!self.is_bot(a), !self.is_bot(b));
                let (share_a, share_b) = match (held_a, held_b) {
                    (true, false) => (0.0, 1.0),
                    (false, true) => (1.0, 0.0),
                    _ => (0.5, 0.5),
                };
                let overlap = dir * (CREW_CLEARANCE - gap);
                self.bims[a].character.pos = self.bims[a].character.pos - overlap * share_a;
                self.bims[b].character.pos = self.bims[b].character.pos + overlap * share_b;
            }
        }
    }

    // --- interrupting, and getting back to it ----------------------------

    /// Put the running chain down and queue it to be picked up next.
    ///
    /// It goes on the *front*, so when one interruption interrupts another the
    /// chains come back in the order they were displaced: the most recently
    /// dropped is the first one resumed.
    ///
    /// A channel is the exception — a deploy or a revive: displaced, it
    /// is dropped for good, so a kit the player placed somewhere else —
    /// or walked away from — is never laid later behind their back, and a
    /// revive walked away from is never walked back to, the patient up by
    /// then or not. A revive put down was picked up again with no look at
    /// whether its patient still lay there.
    fn interrupt(&mut self, who: usize) {
        if self.is_deploying(who) || self.reviving(who).is_some() {
            self.drop_task(who);
            return;
        }
        if let Some(task) = self.bims[who].task.take() {
            let saved = task.suspend(&mut self.bims[who].character, &mut self.room);
            self.bims[who].queue.insert(0, saved);
        }
    }

    /// The errand dropped for good rather than put down: suspended, so the
    /// hands and the scripting come back the way a suspend leaves them,
    /// and then not kept. What displacing a deploy does to it.
    fn drop_task(&mut self, who: usize) {
        if let Some(task) = self.bims[who].task.take() {
            let _ = task.suspend(&mut self.bims[who].character, &mut self.room);
        }
    }

    /// The same, for a new order from the player: the errand is put down
    /// onto the queue. A Stun Shot charging goes on through it (October
    /// 2026; an order that moved the soldier called it off until then).
    fn interrupt_for_order(&mut self, who: usize) {
        self.interrupt(who);
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

    /// Whether the Bim could actually walk to where a chain would start it.
    ///
    /// A chain that cannot be begun is not begun. A walk with nowhere to go
    /// reports itself arrived the instant it starts, and a chain that takes
    /// that for arrival runs through every remaining step in a single frame.
    fn can_begin(&self, who: usize, kind: Kind) -> bool {
        // Somebody else's hands are already in it. See [`Exclusive`].
        if exclusive(kind).is_some_and(|want| self.taken_by_other(who, want)) {
            return false;
        }
        let from = self.bims[who].character.pos;
        let Some(to) = task::first_station(kind, &self.room, from) else {
            return true;
        };
        self.can_reach(who, to)
    }

    /// Whether any *other* Bim is on an errand that has the run of `want`.
    ///
    /// Only what is actually running counts. A chain on the queue is put down:
    /// its hands are off the bench, and holding the bench for it would have
    /// one Bim's interrupted errand keep the other off it for good.
    fn taken_by_other(&self, who: usize, want: Exclusive) -> bool {
        self.bims.iter().enumerate().any(|(i, bim)| {
            i != who
                && bim
                    .task
                    .as_ref()
                    .is_some_and(|task| exclusive(task.kind()) == Some(want))
        })
    }

    fn can_reach(&self, who: usize, to: Vec2) -> bool {
        let nav = self.maps.deck();
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
            _ => self.maps.for_body(ch.is_outside()),
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
            _ => self.maps.deck().nearest_free(at),
        }
    }

    /// Clear the way for a new errand. Returns false when the Bim is already
    /// on that very errand, so asking twice does not restart it.
    fn take_over(&mut self, who: usize, kind: Kind) -> bool {
        if let Some(task) = &self.bims[who].task
            && task.kind() == kind
        {
            return false;
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
        // A player's own Bim is the exception to the first: everything on
        // its queue is what its player put there, and the alarm recruits
        // it (see `arm_players`), so a Shift-queued walk carries on under
        // arms.
        // And nothing is picked up while its player's keys walk it (task
        // 144): the feet are theirs.
        if (self.bims[who].character.is_recruited() && self.is_bot(who))
            || self.bims[who].braced
            || self.bims[who].task.is_some()
            || self.bims[who].queue.is_empty()
            || !self.bims[who].character.arrived()
            || self.bims[who].character.is_steered_walking()
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
        self.bims[who].task = Some(Task::resume(
            saved,
            &mut self.bims[who].character,
            &mut self.room,
            &self.maps,
        ));
    }

    /// Begin an order that waited its turn on the queue (`Saved::ordered`)
    /// through the same door the live order goes through, so every check
    /// the row makes — a patient still there, a weapon still on the deck —
    /// is made now, against the room as it is now. A walk is given the way
    /// a right-click gives one (`walk_order`): a cross on the deck if there
    /// is no way there after all, and the Bim posted at the spot when the
    /// order was one that posts. Whether anything began is the room's to
    /// show; what could not be is simply gone from the queue.
    fn begin_ordered(&mut self, who: usize, saved: &Saved) {
        match saved.kind() {
            Kind::Walk { post } => {
                let Some(to) = saved.target() else {
                    return;
                };
                let code = self.walk_order(who, to.x, to.y);
                if post && code == ORDER_MOVING {
                    let spot = self.nearest_stand(who, to);
                    self.bims[who].character.set_post(Some(spot));
                }
            }
            Kind::Switch(which) => self.send_to_switch(who, which),
            Kind::Revive { patient } => {
                self.revive_crewmate(who, patient);
            }
            // Nothing a Shift-click can queue: the rest are the room's own
            // errands and the world's, never `Saved::ordered`.
            Kind::Build { .. } | Kind::Deploy { .. } => {}
        }
    }

    /// Whether a queued walk could be given now: on the plain it is
    /// planned on the body's window when it is begun, so always; on the
    /// deck, a way there as the doors stand — a walk locked out waits for
    /// the door like an errand does, rather than being dropped.
    fn walk_ready(&self, who: usize, to: Vec2) -> bool {
        if self.on_a_window(who, to) {
            return true;
        }
        self.can_reach(who, to)
    }

    /// Whether the chain at the front of the queue is one the Bim should be
    /// getting on with right now.
    ///
    /// Queued work whose way is shut waits for the door rather than being
    /// thrown away — but it must not hold up everything else while it waits.
    fn queue_ready(&self, who: usize) -> bool {
        self.bims[who].queue.first().is_some_and(|saved| {
            // Picking a chain back up is beginning one as far as a bench is
            // concerned. `pump_queue` does not go through `can_begin` — it
            // resumes rather than starts — so the check has to be here as
            // well, or a half-done craft resumes at a bench somebody else is
            // standing at.
            if exclusive(saved.kind()).is_some_and(|want| self.taken_by_other(who, want)) {
                return false;
            }
            // An order waiting its turn is begun, not resumed, so it asks
            // what beginning asks, and for a walk a way there (`walk_ready`)
            // — a bench in use or a locked door is waited for, not dropped.
            if saved.is_ordered() {
                return match (saved.kind(), saved.target()) {
                    (Kind::Walk { .. }, Some(to)) => self.walk_ready(who, to),
                    (kind, _) => self.can_begin(who, kind),
                };
            }
            saved
                .resume_station(&self.room, self.bims[who].character.pos)
                .is_none_or(|to| self.can_reach(who, to))
        })
    }

    // --- the agenda, for the host's checklist ----------------------------

    /// The chain running now, if any, followed by everything queued behind it.
    pub fn agenda_len(&self, who: usize) -> u32 {
        self.bims[who].task.is_some() as u32 + self.bims[who].queue.len() as u32
    }

    /// Which errand entry `i` is: see the `JOB_` codes. Zero if out of range.
    pub fn agenda_job(&self, who: usize, i: u32) -> u32 {
        match self.agenda_at(who, i) {
            Some((kind, _, _)) => job_code(kind),
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
        self.bims[who].task = None;
        self.bims[who].queue.clear();
        self.bims[who].character.die();
        // Its bunk is nobody's now: the next `adopt`'s to hand to whoever
        // has none.
        if let Some(bed) = self.room.bunk_of.get_mut(who) {
            *bed = None;
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

    /// A whole bar for `who`: [`crate::health::MAX_HEALTH`], and more with
    /// a *Reactor Heart* carried and a player's level (October 2026).
    pub fn max_health(&self, who: usize) -> f32 {
        self.bims
            .get(who)
            .map_or(crate::health::MAX_HEALTH, |b| b.health.max())
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
        self.stand_at(who, at)
    }

    /// Stand a Bim at a spot, snapped to the nearest cell a body fits in,
    /// and say where that was: what the world does to the crew the step a
    /// site's defence starts (task 111), putting them ashore. Nothing about
    /// what it was doing is kept track of here — an errand in hand walks
    /// on from the new spot.
    pub fn stand_at(&mut self, who: usize, at: Vec2) -> Vec2 {
        let nav = self.maps.deck();
        let spot = nav.nearest_free(at);
        self.bims[who].character.stand_at(spot);
        spot
    }

    /// [`Game::stand_at`], and the body still there: the walk it was on
    /// and its post dropped, so it does not set off back towards where it
    /// was. What a mission's start puts the crew aboard with.
    pub fn stand_still_at(&mut self, who: usize, at: Vec2) -> Vec2 {
        let spot = self.stand_at(who, at);
        let character = &mut self.bims[who].character;
        character.halt();
        character.set_post(None);
        spot
    }

    /// Post a Bim somewhere: drop what it is doing, walk there, and stand
    /// there until told otherwise. What a walk to a desk or a container is
    /// made of. It still goes off on an errand it is given and comes back
    /// afterwards — see [`Game::return_to_post`] — and a player order to
    /// anywhere else takes the post away. Any of the crew, not only the
    /// player's. Snapped to somewhere a body can stand; false, and no post,
    /// when there was no route.
    pub fn send_to(&mut self, who: usize, to: Vec2) -> bool {
        self.dispatch(who, to, true)
    }

    /// Post a Bim somewhere it may not be able to get to yet: [`Game::send_to`],
    /// but the post is kept when there is no route — the walk is planned
    /// again from [`Game::return_to_post`] as the way opens, and a hostile
    /// body forces a locked door in its way (`breach`). What a townsperson
    /// is sent to shelter with (`set_sheltering`). False only for nobody,
    /// or a body down.
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

    /// Whether a Bim holds a post: a fight drops every one (`muster`).
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
        self.mark(who, target, false);
        true
    }

    /// Stand a Bim at its post this instant, for the probes and the
    /// fixtures: the walk is the room's business and a test of a post is
    /// not a test of the walk.
    pub fn post_for_probe(&mut self, who: usize, at: Vec2) -> Vec2 {
        let spot = self.put_for_probe(who, at);
        self.bims[who].character.set_post(Some(spot));
        spot
    }

    /// Take a Bim's post away without sending it anywhere: it finishes the
    /// walk it is on, if any, and picks its errands back up.
    pub fn stand_down(&mut self, who: usize) {
        if let Some(bim) = self.bims.get_mut(who) {
            bim.character.set_post(None);
        }
        // And a revive in hand is let go of: what the app's held revive
        // key sends when the key comes up before the patient is up.
        if self.reviving(who).is_some() {
            self.drop_task(who);
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

    // --- the run's missions (feature 103) -----------------------------------

    /// A living Bim made whole, the way a mission begins: a whole bar, the
    /// slow a downing left forgotten — and up, on its feet where it lay.
    /// Nothing for the dead: a body is bought back ([`Game::revive`]),
    /// never healed back.
    pub fn restore_health(&mut self, who: usize) {
        let Some(bim) = self.bims.get_mut(who) else {
            return;
        };
        if !bim.is_alive() {
            return;
        }
        bim.health.restore();
        bim.character.knock_out(false);
        bim.character.set_bleeding(false);
    }

    /// The slow a downing left on a Bim taken off, the bar as it is: what
    /// a mission's end does (task 120).
    pub fn forget_downed(&mut self, who: usize) {
        if let Some(bim) = self.bims.get_mut(who) {
            bim.health.forget_downed();
        }
    }

    /// `points` of health put back into a living body at once — a relic's
    /// healing (task 118, `Health::heal`). How much went in.
    pub fn heal(&mut self, who: usize, points: f32) -> f32 {
        match self.bims.get_mut(who) {
            Some(bim) if bim.is_alive() => bim.health.heal(points),
            _ => 0.0,
        }
    }

    /// `points` taken off a living body's bar outright — a medic paying
    /// for his own healing circle (task 153): past the armour, a shield
    /// and a surge, no flash and no blood, and at nothing the body is
    /// downed as by any hit. How much came off.
    pub fn drain(&mut self, who: usize, points: f32) -> f32 {
        let Some(bim) = self.bims.get_mut(who).filter(|b| b.is_alive()) else {
            return 0.0;
        };
        let taken = bim.health.hit(points);
        if taken > 0.0 && bim.health.downed() {
            self.downs.push(who);
        }
        taken
    }

    /// A medic's healing circle burning the enemy in it (task 153): every
    /// target standing within `radius` of `at` with nothing opaque between
    /// takes `damage`, the same at the edge, as a blast hit by `by` — the
    /// world carries it across with the step's other hits. Nobody of this
    /// room's own, no sentry and no sandbag is touched.
    pub fn scorch(&mut self, at: Vec2, radius: f32, damage: f32, by: usize) {
        if damage <= 0.0 {
            return;
        }
        let reached: Vec<usize> = self
            .combat
            .targets()
            .iter()
            .enumerate()
            .filter_map(|(i, t)| {
                t.filter(|t| (t.at - at).len() <= radius && self.line_clear(at, t.at))
                    .map(|_| i)
            })
            .collect();
        for i in reached {
            self.combat.blast_target(i, damage, Some(by));
        }
    }

    /// A downed body brought round where it lies (feature 106, a relic's
    /// *Second Wind*): [`Health::revive_at`] `share` of the bar, up again
    /// this instant, and slowed for the mission like any revive. Nothing
    /// for the dead or for a body that is not downed.
    pub fn bring_round(&mut self, who: usize, share: f32) -> bool {
        let Some(bim) = self.bims.get_mut(who) else {
            return false;
        };
        if !bim.is_alive() || !bim.health.revive_at(share) {
            return false;
        }
        bim.character.knock_out(false);
        true
    }

    /// Where a *Blink Drive* would put `who` aimed at `to` with a reach
    /// of `reach` room units (October 2026): the spot `to` is, or the
    /// farthest along the way to it within the reach, then stepped back
    /// towards the body half a tile at a time until it is free ground the
    /// body itself sees (`Sight::sees_from`: the walls, the shut doors and
    /// the dark, traced every step, so every copy of the room agrees) and
    /// could walk to — so a blink at a wall or into the dark lands short. `None` for a body that
    /// cannot go (down, outside, carried or carrying) or when nothing is
    /// more than a tile off.
    pub fn blink_spot(&self, who: usize, to: Vec2, reach: f32) -> Option<Vec2> {
        let bim = self.bims.get(who)?;
        let ch = &bim.character;
        if !bim.is_alive()
            || bim.health.downed()
            || ch.is_unconscious()
            || ch.is_outside()
            || self.is_carried(who)
            || self.carrying(who).is_some()
        {
            return None;
        }
        let from = ch.pos;
        let way = to - from;
        let far = way.len().min(reach.max(0.0));
        if far < crate::room::TILE {
            return None;
        }
        let dir = way.normalize_or_zero();
        let nav = self.nav_for(who);
        let mut along = far;
        while along >= crate::room::TILE {
            let at = nav.nearest_free(from + dir * along);
            if (at - from).len() >= crate::room::TILE
                && self.room.sight.sees_from(from, at).is_some()
                && nav.can_reach(from, at)
            {
                return Some(at);
            }
            along -= crate::room::TILE * 0.5;
        }
        None
    }

    /// Blink `who` to `at` (a [`Game::blink_spot`]): stood there this
    /// instant, the walk and whatever it had in hand dropped — a revive or
    /// a kit is a channel, and a blink walks away from it — and the
    /// drive's light at both ends. The keys go on steering it from there.
    pub fn blink(&mut self, who: usize, at: Vec2) -> bool {
        if who >= self.bims.len() {
            return false;
        }
        let from = self.bims[who].character.pos;
        self.interrupt_for_order(who);
        self.call_off_attack_move(who);
        let ch = &mut self.bims[who].character;
        ch.halt();
        ch.stand_at(at);
        self.combat.fx.blink(from, at);
        true
    }

    /// Dead this instant, where it stands — not at the top of its next
    /// tick, the way [`Game::kill_for_probe`] leaves one: what the world
    /// does to a crew member the ship leaves behind (feature 103), whose
    /// room is taken apart in the same breath. Its errands go with it.
    pub fn kill_now(&mut self, who: usize) {
        let Some(bim) = self.bims.get_mut(who) else {
            return;
        };
        bim.task = None;
        bim.queue.clear();
        bim.health.give_up();
        bim.character.die();
        if let Some(bed) = self.room.bunk_of.get_mut(who) {
            *bed = None;
        }
    }

    /// A dead Bim brought back (feature 103; since task 113 a player's Bim
    /// at its mission's end): alive, whole and awake where its body lay,
    /// **with everything it wore and held** — a loadout is never lost —
    /// and its armour mended. Nothing for one already alive.
    pub fn revive(&mut self, who: usize) {
        let Some(bim) = self.bims.get_mut(who) else {
            return;
        };
        if bim.is_alive() {
            return;
        }
        bim.character.revive();
        bim.health.respawn();
        bim.character.set_bleeding(false);
        bim.task = None;
        bim.queue.clear();
        let mut gear = bim.gear;
        if let Some(piece) = gear.worn_mut() {
            piece.health = piece.stats().health;
        }
        self.issue(who, gear);
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
        let nav = self.maps.deck();
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
        let nav = self.maps.deck();
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

    /// One step of a Bim's round: on its way to the next stop, standing its time out at it, or setting off for the one
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
        let free = bim.is_alive()
            && !bim.character.is_unconscious()
            && !bim.character.is_recruited()
            && !bim.character.is_outside()
            && bim.character.post().is_none()
            && bim.task.is_none()
            && bim.queue.is_empty()
            && bim.carrying.is_none()
            && !bim.braced;
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
        self.visitors_revivable.clear();
    }

    /// Which of the visitors the crew may revive — a townsperson downed in
    /// its own room, never a Manufacturer or a machine — index for index
    /// with [`Game::set_visitors`] and told right after it like
    /// `set_visitors_down`. A revive of one is `CrewOrder::Revive` with the
    /// patient [`GUEST`]` + i`: the walk over and the hands on it here, the
    /// body brought round in its own room by the world
    /// (`take_guest_revives`).
    pub fn set_visitors_revivable(&mut self, revivable: &[bool]) {
        self.visitors_revivable = revivable.to_vec();
    }

    /// Whether `patient` names a visitor the crew may revive this step:
    /// [`GUEST`]` + i` with visitor `i` marked revivable.
    pub fn is_revivable_guest(&self, patient: usize) -> bool {
        patient
            .checked_sub(GUEST)
            .is_some_and(|i| self.visitors_revivable.get(i).copied().unwrap_or(false))
    }

    /// The revivable visitor under a room point, if any, as its patient
    /// index ([`GUEST`]` + i`): a click's reach, the first by index — the
    /// medkit's right-click on a townsperson down.
    pub fn guest_at(&self, x: f32, y: f32) -> Option<usize> {
        let p = vec2(x, y);
        (0..self.visitors.len())
            .find(|&i| {
                self.visitors_revivable.get(i).copied().unwrap_or(false)
                    && (self.visitors[i] - p).len() <= PICK_RADIUS
            })
            .map(|i| GUEST + i)
    }

    /// Every visitor the crew may revive this step, as its patient index
    /// ([`GUEST`]` + i`), by visitor index.
    pub fn revivable_guests(&self) -> Vec<usize> {
        (0..self.visitors.len())
            .filter(|&i| self.visitors_revivable.get(i).copied().unwrap_or(false))
            .map(|i| GUEST + i)
            .collect()
    }

    /// Where a revivable visitor lies, by patient index ([`GUEST`]` + i`).
    pub fn guest_pos(&self, patient: usize) -> Option<Vec2> {
        if !self.is_revivable_guest(patient) {
            return None;
        }
        self.visitors.get(patient - GUEST).copied()
    }

    /// Every revive of a visitor finished since the world last asked:
    /// the world brings each one round in its own room.
    pub fn take_guest_revives(&mut self) -> Vec<GuestRevived> {
        std::mem::take(&mut self.guest_revives)
    }

    /// Which of this room's own bodies have the crew's hands on them from
    /// the joined deck, index for index — the world's word every step. A
    /// downed body tended stands its countdown, as one revived in this
    /// room does (task 138).
    pub fn set_tended(&mut self, tended: &[bool]) {
        self.tended = tended.to_vec();
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

    /// A **Manufacturer** onto the deck (feature 109): a Bim at `at` —
    /// snapped to where a body fits, the way [`Game::adopt`] snaps one —
    /// in the black and gold, carrying `gear`, flagged
    /// [`crate::bim::Bim::manufacturer`] and on the room's side of the
    /// fight ([`Game::set_hostile_bodies`]). Its face is rolled off `seed`
    /// rather than the room's stream, so laying a site's people moves no
    /// roll a fight makes. Answers its index among the Bims.
    ///
    /// It goes on the end of the Bims, and a body index past the Bims is a
    /// machine's: so every Manufacturer of a wave is enlisted **before**
    /// the machines of that wave go on the deck, and none while machines
    /// stand (the world's rule — a site's Troopers are the garrison's and
    /// its reinforcements are Manufacturers alone).
    ///
    /// `plan_wait` is its first wait before it picks where to stand, which
    /// the world staggers across a wave as it does a wave of machines.
    pub fn enlist_manufacturer(
        &mut self,
        at: Vec2,
        gear: Gear,
        seed: u64,
        plan_wait: f32,
    ) -> usize {
        let who = self.bims.len();
        let mut rng = Rng::new(seed);
        let mut bim = Bim::new(who, at, &mut rng);
        // Past the classic pair, off the seed: faces as varied as a
        // station's people, and none of them the crew's first two.
        bim.character
            .set_look(crate::character::Look::of(2 + (seed % 4_096) as usize));
        bim.character
            .set_uniform(crate::character::Uniform::Manufacturer);
        // An enemy's red in any room: in a friendly one it is an
        // intruder (task 131).
        bim.character.set_hostile(true);
        bim.gear = gear;
        bim.manufacturer = true;
        bim.plan_wait = plan_wait;
        bim.breach_wait = plan_wait;
        // Into a fight already under way it comes under arms, as the
        // war's start put everybody here (`muster`): the war musters only
        // as it changes, and one enlisted mid-war stood unarmed where it
        // was put — a reinforcement of theirs, waiting behind the airlock
        // it came in by for as long as the fight went on.
        if self.hostile_bodies && self.at_war {
            bim.character.set_recruited(true);
        }
        self.adopt(vec![bim], Vec2::ZERO);
        who
    }

    /// Whether that body is a Manufacturer (feature 109) — false for a
    /// machine and for every other Bim.
    pub fn is_manufacturer(&self, who: usize) -> bool {
        self.bims.get(who).is_some_and(|b| b.manufacturer)
    }

    /// A commander's **reinforcement** onto the deck (task 129): a Bim at
    /// `at`, snapped to where a body fits as [`Game::adopt`] snaps one,
    /// in the crew's coverall, carrying `gear` and nothing else. Its face
    /// is rolled off `seed` rather than the room's stream, so laying one
    /// moves no roll a fight makes. It goes on the end of the crew and
    /// answers its index.
    pub fn enlist_reinforcement(&mut self, at: Vec2, gear: Gear, seed: u64) -> usize {
        let who = self.bims.len();
        let mut rng = Rng::new(seed);
        let mut bim = Bim::new(who, at, &mut rng);
        bim.character
            .set_look(crate::character::Look::of(2 + (seed % 4_096) as usize));
        bim.gear = gear;
        // Called into a fight the crew are already under arms for, it is
        // under arms with them, as `muster_crew` put them: the muster runs
        // only as it changes, and one enlisted after stood where it was
        // put with its rifle away while the fight went on round it.
        if !self.hostile_bodies && self.mustered {
            bim.character.set_recruited(true);
        }
        self.adopt(vec![bim], Vec2::ZERO);
        self.refresh_worn(who);
        who
    }

    /// The middles of the free deck tiles within `reach` (room units) of
    /// `at`, nearest first — a tile of deck a body fits on, that can be
    /// walked to from `at`, and that nobody, living or dead, and no
    /// machine stands on. Ties go to the upper row, then the left. Where a
    /// commander's reinforcements are stood (task 129).
    pub fn free_tiles_near(&self, at: Vec2, reach: f32) -> Vec<Vec2> {
        let nav = self.maps.deck();
        let span = (reach / TILE).ceil() as i32;
        let (cx, cy) = ((at.x / TILE).floor() as i32, (at.y / TILE).floor() as i32);
        let taken = |p: Vec2| {
            self.bims
                .iter()
                .filter(|b| !b.gone)
                .any(|b| (b.character.pos - p).len() < 0.6 * TILE)
                || self.droids.iter().any(|d| (d.pos - p).len() < 0.6 * TILE)
        };
        let mut tiles: Vec<(f32, i32, i32, Vec2)> = Vec::new();
        for y in (cy - span)..=(cy + span) {
            for x in (cx - span)..=(cx + span) {
                let p = vec2((x as f32 + 0.5) * TILE, (y as f32 + 0.5) * TILE);
                let far = (p - at).len();
                if far > reach || !self.is_deck_tile(p) || taken(p) || !nav.can_reach(at, p) {
                    continue;
                }
                tiles.push((far, y, x, p));
            }
        }
        tiles.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)).then(a.2.cmp(&b.2)));
        tiles.into_iter().map(|t| t.3).collect()
    }

    /// Take a dead body off the deck for good (task 129): a commander's
    /// reinforcement that fell. It is drawn nowhere and seen by nobody
    /// from now on; its index stays until the world drops it at the
    /// mission's end. Nothing for one alive.
    pub fn vanish(&mut self, who: usize) {
        if let Some(bim) = self.bims.get_mut(who)
            && !bim.is_alive()
        {
            bim.gone = true;
            bim.trail.clear();
        }
    }

    /// Whether a body has been taken off the deck ([`Game::vanish`]).
    pub fn is_gone(&self, who: usize) -> bool {
        self.bims.get(who).is_some_and(|b| b.gone)
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
        let nav = self.maps.deck();
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

    /// The route a Bim is on, and the solids it is kept out of, for the probes.
    #[allow(dead_code)]
    pub fn route_for_probe(&self, who: usize) -> (Vec<Vec2>, Vec<Rect>) {
        (
            self.bims[who].character.path_for_probe().to_vec(),
            self.blockers.clone(),
        )
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
        self.bims[who].task.as_ref().map(|t| job_code(t.kind()))
    }

    /// Whether Bim `who`'s walk is over — nothing left of its route, and
    /// no leg on the plain still to go. What a throw walked out to asks
    /// before it gives up (`World::throws`).
    pub fn has_arrived(&self, who: usize) -> bool {
        self.bims.get(who).is_none_or(|b| b.character.arrived())
    }

    /// Whether Bim `who` could walk to `to` on the deck's grid from where
    /// it stands: what the world asks of the spot a throw is walked out
    /// to.
    pub fn reaches(&self, who: usize, to: Vec2) -> bool {
        who < self.bims.len() && self.can_reach(who, to)
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

    /// Where the route a Bim is on ends, or `None` when it is not on one. For
    /// the probes: it is how "standing aside is not replanning" is checked.
    #[allow(dead_code)]
    pub fn destination_for_probe(&self, who: usize) -> Option<Vec2> {
        self.bims[who].character.destination()
    }

    // --- deciding for itself -----------------------------------------------

    /// A Bim with nothing on picks up work off the list.
    ///
    /// Only when it is otherwise free: a walk the player ordered counts as
    /// having something on, so the Bim gets where it was sent before going
    /// off on its own account.
    fn consider_errand(&mut self, who: usize) {
        // Queued work that the Bim cannot get to does not count as having
        // something on: it would block everything else while it waited.
        // And a player's own Bim takes nothing up of its own accord: only
        // the player's input moves it (September 2026, the user's word).
        if !self.is_bot(who)
            || self.bims[who].character.is_recruited()
            || self.bims[who].braced
            || !self.autonomous
            || self.bims[who].task.is_some()
            || self.queue_ready(who)
            || !self.bims[who].character.arrived()
        {
            return;
        }
        // What is left is work, and the player says what order that gets
        // done in.
        self.do_some_work(who);
    }

    /// Whatever work is going, in the order the player asked for.
    ///
    /// Everything here is discretionary: a bench only wants a hand while the
    /// world has an order for it, a site while the world wants it built, a
    /// wound while somebody bleeds. Whichever of those is going is collected,
    /// sorted, and tried in turn — the first one that can actually be
    /// started wins, so a bench somebody else is standing at does not stop
    /// this one getting on with a site.
    ///
    /// The sort is **stable**: all equal, the jobs come out in the order they
    /// were offered.
    fn do_some_work(&mut self, who: usize) -> bool {
        for job in self.work_on_offer(who) {
            let started = match job {
                Job::Build => self.build(who),
                // A downed crewmate to revive: the same errand the player
                // orders on a body, chosen by the room.
                Job::Medical => self
                    .revive_on_offer(who)
                    .is_some_and(|patient| self.revive_crewmate(who, patient)),
            };
            if started {
                return true;
            }
        }
        false
    }

    /// The downed crewmate `who` would revive if it took the medical row
    /// (task 120): the nearest one it can get to that nobody else is on
    /// its way to already — **one reviver a patient** — and that is not in
    /// somebody's arms. `None` with nobody downed, a helper in no state to
    /// do it — dead, downed, outside, carrying somebody, a Manufacturer, or
    /// in a room whose people do not revive one another
    /// ([`Game::set_revivers`]) — or under orders with an enemy in its
    /// sight or a blade at its throat. **Only a bot revives of its own
    /// accord** (task 120): a player's own Bim is the player's, and revives
    /// when it is told to. **A bot under arms with nothing in sight
    /// revives**, and takes its weapon up again the moment `aim` sees
    /// something (`care_gives_way`).
    ///
    /// **A crewmate is revived only where it lies out of the fight**: the
    /// whole room calm (`Game::calm`), or the patient itself out of harm
    /// (`Game::out_of_harm` — no enemy up within [`RESCUE_CLEAR`] tiles of
    /// it and nothing that could see it there): a helper kneeling over a
    /// body with the enemy a corridor away was a second body down. A field
    /// medic carries one clear first (`rescue`).
    ///
    /// **A medic first** (task 125): a bot that is not a medic
    /// (`Skill::medic`, of the class or hired) leaves a patient to a medic
    /// bot free to go to it — ready to revive by the same rule, with no
    /// revive in hand already, and a way to the patient — and takes it
    /// only when there is none: no medic in the crew, the medic down,
    /// busy with another body or in a fight of its own.
    fn revive_on_offer(&self, who: usize) -> Option<usize> {
        if !self.ready_to_revive(who) {
            return None;
        }
        let from = self.bims[who].character.pos;
        let clear = self.deck_clear();
        let calm = clear || self.calm();
        let medic = self.skill(who).medic;
        // The wave cleared, a player's own Bim comes before any bot, the
        // nearest of each first; in the fight the nearest, whoever it is.
        let rank = |p: usize| {
            let far = (self.bims[p].character.pos - from).len();
            (clear && self.is_bot(p), far)
        };
        (0..self.bims.len())
            .filter(|&p| p != who && self.can_be_revived(p))
            .filter(|&p| !self.is_being_seen_to_by_another(p, who))
            .filter(|&p| calm || self.out_of_harm(self.bims[p].character.pos))
            .filter(|&p| task::patient_stand(&self.room, &self.maps, who, p, from).is_some())
            .filter(|&p| medic || !self.a_medic_free_for(p, who))
            .min_by(|&a, &b| {
                let (a, b) = (rank(a), rank(b));
                a.0.cmp(&b.0).then(a.1.total_cmp(&b.1))
            })
    }

    /// Whether `who` may take a revive of its own accord this step,
    /// whoever the patient: a bot in a room whose people revive one
    /// another, alive, awake, on the deck, its arms free, not a
    /// Manufacturer — and, under arms, with no enemy in its sight, no
    /// blade at its throat and not under fire ([`UNDER_FIRE`]): a bot
    /// being shot at fights back first. A revive already in hand goes on
    /// (damage does not interrupt one).
    fn ready_to_revive(&self, who: usize) -> bool {
        if !self.revivers
            || !self.is_bot(who)
            || !self.bims.get(who).is_some_and(|b| b.is_alive())
            || self.bims[who].manufacturer
            || self.bims[who].character.is_unconscious()
            || self.bims[who].health.downed()
            || self.bims[who].character.is_outside()
            || self.bims[who].carrying.is_some()
        {
            return false;
        }
        // The deck clear, nobody is left to fight first.
        !self.bims[who].character.is_recruited()
            || self.deck_clear()
            || (self.bims[who].locked.is_none()
                && self.bims[who].blow.is_none()
                && self.bims[who].under_fire <= 0.0
                && !self
                    .combat
                    .sees_any(&self.room.sight, self.bims[who].character.pos))
    }

    /// Whether a medic bot other than `asking` is free to revive
    /// `patient` (task 125): [`Game::ready_to_revive`], no revive in hand
    /// already, and a way to the patient.
    fn a_medic_free_for(&self, patient: usize, asking: usize) -> bool {
        (0..self.bims.len()).any(|m| {
            m != asking
                && m != patient
                && self.skill(m).medic
                && self.reviving(m).is_none()
                && self.ready_to_revive(m)
                && task::patient_stand(
                    &self.room,
                    &self.maps,
                    m,
                    patient,
                    self.bims[m].character.pos,
                )
                .is_some()
        })
    }

    /// Whether a body may be revived at all: one of this room's Bims,
    /// downed, not a Manufacturer (feature 109: never revived), on the
    /// deck and in nobody's arms.
    fn can_be_revived(&self, patient: usize) -> bool {
        self.bims.get(patient).is_some_and(|b| {
            b.health.downed()
                && !b.manufacturer
                && !b.character.is_outside()
                && !self.is_carried(patient)
        })
    }

    /// Whether a crewmate is on its way to `who`, or has its hands on it:
    /// a revive with `who` as the patient, somebody else's.
    fn is_being_seen_to(&self, who: usize) -> bool {
        self.is_being_seen_to_by_another(who, who)
    }

    /// Whether anybody but `helper` (and the patient itself) has a revive
    /// of `patient` in hand.
    fn is_being_seen_to_by_another(&self, patient: usize, helper: usize) -> bool {
        self.bims.iter().enumerate().any(|(other, b)| {
            other != helper
                && other != patient
                && b.task
                    .as_ref()
                    .is_some_and(|t| t.kind().patient() == Some(patient))
        })
    }

    /// Whether a bot's revive is to be put down for the fight: a body
    /// nobody steers, under arms, with a revive in hand and something to
    /// shoot at from where it stands, a gun's shot in reach or, for a
    /// blade, an enemy in sight. Never a player's own Bim, whose revive is
    /// the player's call, and never a medic whose beam holds its fire
    /// anyway. Being hit is no reason: damage does not interrupt a revive.
    fn care_gives_way(&self, who: usize, skill: &Skill) -> bool {
        let Some(bim) = self.bims.get(who) else {
            return false;
        };
        let reviving = bim
            .task
            .as_ref()
            .is_some_and(|t| matches!(t.kind(), Kind::Revive { .. }));
        if !reviving || !self.is_bot(who) || !bim.character.is_recruited() || skill.holds_fire {
            return false;
        }
        // A Medivac medic keeps its hands on a player whatever it could
        // shoot at: that is what it was called in for.
        if bim.medivac && self.reviving(who).is_some_and(|p| p < self.players) {
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

    /// What work there is for `who` right now, most important first.
    ///
    /// Kept apart from starting any of it because this is the half the player
    /// can actually see the effect of, and the half worth checking: whether a
    /// job is *begun* also depends on the bench being free, and that has
    /// nothing to do with the list.
    fn work_on_offer(&self, who: usize) -> Vec<Job> {
        let mut offered: Vec<Job> = Vec::new();
        // Somebody downed to revive. Offered first, though its code is
        // the last: among equals the list keeps the order offered, and an
        // untouched list has a crewmate revived before anything else is
        // seen to.
        if self.revive_on_offer(who).is_some() {
            offered.push(Job::Medical);
        }
        // A site with nobody at it.
        if self.build_on_offer(who).is_some() {
            offered.push(Job::Build);
        }
        // A job switched off is not on offer.
        offered.retain(|&job| self.waits_on(job) != work::NEVER);
        offered.sort_by_key(|&job| self.waits_on(job));
        offered
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
        if !self.take_over(who, Kind::Build { site, outside }) {
            return false;
        }
        self.bims[who].task = Some(Task::build(
            who,
            site,
            outside,
            minutes,
            &mut self.bims[who].character,
            &mut self.room,
            &self.maps,
        ));
        true
    }

    /// What the world wants built, this step, and who may go outside to
    /// it. Replaces the last list whole: a site
    /// that is no longer on it is not begun again, and a chain already at
    /// one finds it gone at its next walk and gives up. The world says,
    /// every step.
    pub fn set_build_orders(&mut self, builds: Vec<Build>, suit_ok: Vec<bool>) {
        self.room.builds = builds;
        self.room.suit_ok = suit_ok;
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
    pub fn relayout(&mut self, layout: room::Layout) {
        self.room.relayout(layout);
        self.refresh_maps();
        self.refresh_blockers();
        self.room.rocks_version = self.room.rocks_version.wrapping_add(1);
    }

    /// The workstations aboard, in the layout's order.
    pub fn benches(&self) -> &[crate::room::Bench] {
        &self.room.benches
    }

    /// Whether this Bim is outside the hull, in a suit. What the world
    /// doses.
    pub fn is_outside(&self, who: usize) -> bool {
        self.bims.get(who).is_some_and(|b| b.character.is_outside())
    }

    /// The number a job waits on: its own row.
    fn waits_on(&self, job: Job) -> u32 {
        self.priorities.of(job)
    }

    /// How many tiles of the deck have blood on them. For the tests.
    pub fn bloody_tiles(&self) -> u32 {
        self.room.blood.stained_tiles()
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
    /// For the probes: what the list actually decides, without a bench being
    /// busy confusing the reading.
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

    // --- naming what the pointer is over ----------------------------------
    //
    // Three numbers rather than a string, the same as everything else across
    // the boundary: what the thing is, what is on it, and how much. The host
    // does the wording.

    /// What is at a point, in the `SPOT_` codes from `room.rs`.
    pub fn spot_at(&self, x: f32, y: f32) -> u32 {
        self.room.spot(vec2(x, y))
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
    /// Recruited, it does nothing of its own accord: no errand off the work
    /// list, nothing picked back up off the queue, and no pottering about
    /// between jobs. What it will still do is everything the player asks of
    /// it.
    ///
    /// Whatever it is in the middle of is left to finish; a right click
    /// interrupts it anyway.
    ///
    /// Recruit player `slot`'s own crew member, or let it go again.
    pub fn toggle_recruited(&mut self, slot: u32) {
        let who = slot as usize;
        if who >= self.bims.len() {
            return;
        }
        let now = !self.bims[who].character.is_recruited();
        self.bims[who].character.set_recruited(now);
        // Recruited or let go by the player, it is the player's now and
        // not the alarm's to let go (`arm_players`).
        if let Some(armed) = self.alarm_armed.get_mut(who) {
            *armed = false;
        }
        if !now {
            // Weapon away, and an attack-move and a target with it.
            self.bims[who].attack_move = None;
            self.bims[who].focus = None;
        }
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

    /// What `who`'s next shot fires with: its skill for that shot (an
    /// *Overcharge Cell*'s count), and for a player's own Bim every
    /// weapon at full odds ([`Skill::sure`]) — a bot keeps its own.
    fn fired_skill(&self, who: usize, skill: &Skill) -> Skill {
        let shot = skill.for_shot(self.bims[who].shots);
        if self.is_bot(who) { shot } else { shot.sure() }
    }

    /// The other side of [`Game::is_player`]: a Bim that runs on its own
    /// lights. Every body of a room whose bodies are hostile is one —
    /// a station's people answer to nobody at this keyboard, so the
    /// first of them is no more a player's than the last. What decides
    /// whether a crewmate is doctored of its own accord
    /// (`medical_on_offer`).
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

    /// How many are aboard. Two in a bare room; a ship's crew, up to
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

    /// One of them by its droid index, to be changed by hand: stood
    /// somewhere, armed, held, or mended to a whole body again. The
    /// probes' and the tests' — nothing in the game changes a machine
    /// but its own step.
    pub fn droid_mut_for_probe(&mut self, i: usize) -> Option<&mut Droid> {
        self.droids.get_mut(i)
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
            let nav = self.maps.deck();
            // The Machine Heart's are built where they stand (feature 108),
            // on free deck that `world::heart::places` picked: never moved.
            if !droid.kind.is_structure() && !nav.is_free(droid.pos) {
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

    /// Where a wave landing round `spots[0]` is stood (the world's
    /// arrival spots, in this room's units): each on free deck it can
    /// walk off from, a tile apart where there is room
    /// ([`crate::nav::Nav::spread_from`]). The first is the spot just
    /// inside the door or the gate the wave came in by.
    pub fn spread_wave(&self, spots: &[Vec2]) -> Vec<Vec2> {
        self.maps.deck().spread_from(spots, TILE)
    }

    /// Where one of this room's bodies stands, Bim or machine.
    pub fn body_pos(&self, who: usize) -> Vec2 {
        match self.droid_at(who) {
            Some(i) => self.droids[i].pos,
            None => self.bims.get(who).map_or(Vec2::ZERO, |b| b.character.pos),
        }
    }

    /// How far between the last step and the next the picture is drawn
    /// (`blend`), nought to one, or `None` for where the bodies stand.
    /// Once a frame, by the host, before `render`.
    pub fn set_blend(&mut self, blend: Option<f32>) {
        self.blend = blend.map(|t| clamp(t, 0.0, 1.0));
    }

    /// Stand every body where it is drawn this frame, for `render`'s
    /// drawing of them, and say where they really stood for `put_back`.
    /// Nothing with no blend, and a wreck stays where it fell (its
    /// pieces are laid by a hash of its position).
    fn stand_where_shown(&mut self) -> Vec<Vec2> {
        let blend = self.blend;
        if blend.is_none() {
            return Vec::new();
        }
        let mut stood = Vec::with_capacity(self.bims.len() + self.droids.len());
        for bim in &mut self.bims {
            let c = &mut bim.character;
            stood.push(c.pos);
            c.pos = shown(c.was, c.pos, blend);
        }
        for droid in &mut self.droids {
            stood.push(droid.pos);
            if !droid.body.destroyed() {
                droid.pos = shown(droid.was, droid.pos, blend);
            }
        }
        stood
    }

    /// Every bolt in the air drawn back along its flight by the share of
    /// a step still to come (`set_blend`) — never behind the muzzle it
    /// left — for `render`'s drawing of them; where they really are, for
    /// `bolts_put_back`.
    fn bolts_where_shown(&mut self) -> Vec<Vec2> {
        let Some(t) = self.blend else {
            return Vec::new();
        };
        let back = (1.0 - t) * self.step_dt;
        self.combat
            .bolts
            .iter_mut()
            .map(|bolt| {
                let at = bolt.pos;
                let behind = bolt.vel * back;
                bolt.pos = if (at - bolt.fired_from).len() > behind.len() {
                    at - behind
                } else {
                    bolt.fired_from
                };
                at
            })
            .collect()
    }

    /// Every bolt back where it is, after [`Game::bolts_where_shown`].
    fn bolts_put_back(&mut self, flying: Vec<Vec2>) {
        for (bolt, at) in self.combat.bolts.iter_mut().zip(flying) {
            bolt.pos = at;
        }
    }

    /// Every body back where it stands, after [`Game::stand_where_shown`].
    fn put_back(&mut self, stood: Vec<Vec2>) {
        if stood.is_empty() {
            return;
        }
        let crew = self.bims.len();
        for (bim, &at) in self.bims.iter_mut().zip(&stood) {
            bim.character.pos = at;
        }
        for (droid, &at) in self.droids.iter_mut().zip(&stood[crew..]) {
            droid.pos = at;
        }
    }

    /// Where a body is drawn this frame — [`Game::body_pos`] blended back
    /// towards where it stood before the last step — for whatever the
    /// host lays over it (a name, a bar, the camera). The picture only.
    pub fn shown_pos(&self, who: usize) -> Vec2 {
        match self.droid_at(who) {
            Some(i) => shown(self.droids[i].was, self.droids[i].pos, self.blend),
            None => self.bims.get(who).map_or(Vec2::ZERO, |b| {
                shown(b.character.was, b.character.pos, self.blend)
            }),
        }
    }

    /// Take `damage` off a machine's part — the part rolled by the
    /// caller off the combat stream's own draw (`Hit::roll` through
    /// [`DroidPart::hit_by`]), since a Bim has no parts. By **droid** index. Whether anything was struck.
    pub fn strike_droid(&mut self, i: usize, part: DroidPart, damage: f32) -> bool {
        let Some(droid) = self.droids.get_mut(i) else {
            return false;
        };
        let was = droid.destroyed;
        let struck = droid.strike(part, damage);
        if struck.is_some() {
            droid.under_fire = UNDER_FIRE;
        }
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

    /// Machine `i`'s plate stopped a bolt or a blow `damage` hard (a
    /// Guardian's, [`crate::balance::GUARDIAN_SHIELD_HP`] in all): taken off the
    /// plate, which flares out where it stands the moment it breaks.
    /// Whether this was the one that broke it.
    pub fn strike_plate(&mut self, i: usize, damage: f32) -> bool {
        let Some(droid) = self.droids.get_mut(i) else {
            return false;
        };
        let broke = droid.strike_plate(damage);
        if !droid.destroyed {
            droid.under_fire = UNDER_FIRE;
        }
        if broke {
            let (at, reach) = (droid.pos, crate::balance::GUARDIAN_SHIELD_RADIUS);
            self.combat.lull_break();
            self.combat
                .fx
                .burst(at + droid.front() * reach, reach * 0.5);
        }
        broke
    }

    /// Every bolt and blow a target's shield stopped here since the last
    /// call: the target's index and the damage (`Combat::take_plate_hits`),
    /// for the world to take off the machine's plate.
    pub fn take_plate_hits(&mut self) -> Vec<(usize, f32)> {
        self.combat.take_plate_hits()
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
    /// on it. Clicking a fixture leaves the selection alone — opening a
    /// locker should not deselect the Bim you were about to give a job to.
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
        // Under arms a bot is not to be picked at all: a click in a fight
        // is aimed at the fight, and the bots take their orders from the
        // standing orders and never a click (`orderable`).
        let touched: Vec<usize> = (0..self.bims.len())
            .filter(|&i| self.bims[i].is_alive())
            .filter(|&i| !self.mustered || self.is_player(i))
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

    /// Right-click on the floor: send whatever is selected to that spot. Says
    /// what became of the order; see the `ORDER_` codes. An unlocked door is
    /// no wall to the pathfinder — it opens for whoever walks up to it — and
    /// a locked one is, so a spot only through a locked door is nowhere.
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
        if !self.on_a_window(who, to) && !self.can_reach(who, stand) {
            self.mark(who, stand, true);
            return ORDER_NOWHERE;
        }
        self.bims[who]
            .queue
            .push(Saved::ordered(who, Kind::Walk { post }, 0.0, Some(stand)));
        self.mark(who, stand, false);
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

    /// Who a right-click from player `slot` sends: **their own crew
    /// member, and nobody else**, whoever is selected. The bots take the
    /// player's standing orders — the attack banner, the retreat — and a
    /// commander's squad orders, never a click on the deck: a right-click
    /// that walked a selected bot off its stand in the middle of a fight
    /// was a bot nobody meant to move. Another player's own is never
    /// theirs to order either.
    fn orderable(&self, slot: u32) -> Vec<usize> {
        let own = slot as usize;
        if own < self.bims.len() {
            vec![own]
        } else {
            Vec::new()
        }
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
        if !self.is_player(who) && code != ORDER_IGNORED && code != ORDER_NOWHERE {
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

    /// [`Game::order_move`] for one Bim, the checks on who is done
    /// ([`Game::orderable`]: the player's own, selected or not). A plain
    /// order, so what was queued with Shift goes (`drop_ordered`), and so
    /// does an attack-move under way.
    fn order_move_for(&mut self, _slot: u32, who: usize, x: f32, y: f32) -> u32 {
        self.drop_ordered(who);
        self.bims[who].attack_move = None;
        self.bims[who].focus = None;
        self.walk_order(who, x, y)
    }

    /// The attack key and a click at `(x, y)`: player `slot`'s own crew
    /// member walks there **under arms** — recruited, so its weapon is
    /// out — and stops to shoot the moment it has something in its
    /// sights, walking on once nothing is left (Dota's attack-move,
    /// [`Game::keep_attack_moving`]). The codes are
    /// [`Game::order_move`]'s; the ping is the attack's red.
    pub fn order_attack_move(&mut self, slot: u32, x: f32, y: f32) -> u32 {
        let who = slot as usize;
        if who >= self.bims.len() {
            return ORDER_IGNORED;
        }
        self.drop_ordered(who);
        self.bims[who].attack_move = None;
        self.bims[who].focus = None;
        let code = self.walk_order(who, x, y);
        if code == ORDER_MOVING {
            let to = self.nearest_stand(who, vec2(x, y));
            let bim = &mut self.bims[who];
            bim.attack_move = Some(to);
            bim.hand = Hand::Weapon;
            bim.character.set_recruited(true);
            bim.character.select_for(slot, true);
            if let Some(ping) = self.markers.last_mut() {
                ping.kind = Ping::Attack;
            }
        }
        code
    }

    /// Any attack-move `who` is on called off, and any target it was
    /// told to attack: what an errand given it does ([`Game::order`]).
    pub(crate) fn call_off_attack_move(&mut self, who: usize) {
        self.bims[who].attack_move = None;
        self.bims[who].focus = None;
    }

    /// The quickselect (task 138): player `slot`'s own crew member takes
    /// the weapon or the medkit in hand. With the medkit it holds its
    /// fire (`tick_combat`), so an attack it was on is called off; an
    /// attack order puts the weapon back in hand (`order_attack`,
    /// `order_attack_move`).
    pub fn order_hand(&mut self, slot: u32, hand: Hand) {
        let who = slot as usize;
        if who >= self.bims.len() || who >= self.players {
            return;
        }
        if hand == Hand::Medkit {
            self.call_off_attack_move(who);
        }
        self.bims[who].hand = hand;
    }

    /// Player `slot`'s keys and pointer on its own crew member (task 144):
    /// the walk WASD give it, where the pointer aims it and whether the
    /// fire button is down, said whole every time one of them changes.
    /// The step the keys start walking it, whatever it was walking to or
    /// doing is dropped — the queue's orders, an attack, a post and the
    /// errand in hand — since the feet are the keys' now. A Stun Shot
    /// charging goes on as it walks (October 2026).
    pub fn order_control(&mut self, slot: u32, steer: crate::character::Steer) {
        let who = slot as usize;
        if who >= self.bims.len() || who >= self.players {
            return;
        }
        if steer.walk.is_some() && !self.bims[who].character.is_steered_walking() {
            self.drop_ordered(who);
            self.call_off_attack_move(who);
            self.bims[who].character.set_post(None);
            self.drop_task(who);
        }
        self.bims[who].character.set_steer(steer);
    }

    /// Player `slot`'s own Bim dodge-rolls (task 150): the way its keys
    /// walk it, else the way they last did — never at the pointer —
    /// slipping every bolt and beam while it rolls, refused while one is
    /// under way or cooling down, or the body is down, carried or
    /// carrying. Like the keys starting to walk it, the roll drops what
    /// it was on: the queue, an attack, a post, the errand — never a Stun
    /// Shot charging (October 2026).
    pub fn order_dodge(&mut self, slot: u32) {
        let who = slot as usize;
        if who >= self.bims.len()
            || who >= self.players
            || !self.bims[who].is_alive()
            || self.bims[who].carrying.is_some()
            || self.is_carried(who)
        {
            return;
        }
        if !self.bims[who].character.start_roll() {
            return;
        }
        self.drop_ordered(who);
        self.call_off_attack_move(who);
        self.bims[who].character.set_post(None);
        self.drop_task(who);
    }

    /// `CrewOrder::Reload` (October 2026): player `slot`'s own Bim
    /// reloads the magazine in its hand now, shots left in it or not —
    /// nothing for a weapon with none, a full one, one already being
    /// reloaded, or a body that cannot act. A roll, a sprint or a charge
    /// do not stop it; the reload runs on whatever the body does.
    pub fn order_reload(&mut self, slot: u32) {
        let who = slot as usize;
        if who >= self.bims.len()
            || who >= self.players
            || !self.bims[who].is_alive()
            || self.bims[who].character.is_unconscious()
        {
            return;
        }
        let Some(weapon) = self.bims[who].gear.weapon else {
            return;
        };
        let stats = self.shot_skill(who).stats(weapon);
        let trigger = &mut self.bims[who].trigger;
        trigger.load(weapon.kind);
        if trigger.reload_now(&stats) {
            self.say_reload(who, weapon.kind);
        }
    }

    /// Every crew member whose reload began this step said as a
    /// `Cue::Reload`, at the body — in a room whose bodies are not
    /// hostile; an enemy's people reload unheard.
    fn say_reloads(&mut self) {
        if self.hostile_bodies {
            return;
        }
        for who in 0..self.bims.len() {
            let bim = &self.bims[who];
            if bim.trigger.began
                && let Some(weapon) = bim.gear.weapon
            {
                self.say_reload(who, weapon.kind);
            }
        }
    }

    fn say_reload(&mut self, who: usize, weapon: WeaponKind) {
        self.room.cues.push(Cued {
            cue: Cue::Reload {
                weapon,
                by: Some(who),
            },
            at: self.bims[who].character.pos,
        });
    }

    /// The magazine in `who`'s hand (October 2026), for the hero panel
    /// and the reticle: shots left, shots it holds, and the share of a
    /// reload under way still to run (nought with none). `None` for a
    /// body with nothing in its hand or a weapon with no magazine.
    pub fn magazine(&self, who: usize) -> Option<(u32, u32, f32)> {
        let bim = self.bims.get(who)?;
        let weapon = bim.gear.weapon?;
        let stats = weapon.stats();
        if stats.magazine == 0 {
            return None;
        }
        // A weapon changed in the hand since the last step is full.
        let trigger = &bim.trigger;
        let spent = if trigger.loaded == weapon.kind.code() {
            trigger.spent
        } else {
            0
        };
        let left = if trigger.loaded == weapon.kind.code() && trigger.is_reloading() {
            trigger.reloading / stats.reload_time.max(1e-3)
        } else {
            0.0
        };
        Some((
            stats.magazine.saturating_sub(spent),
            stats.magazine,
            left.clamp(0.0, 1.0),
        ))
    }

    /// Whether `who` is rolling (task 150), for the app's sound and the
    /// tests.
    pub fn is_rolling(&self, who: usize) -> bool {
        self.bims.get(who).is_some_and(|b| b.character.is_rolling())
    }

    /// Whether `who` sprints this step (task 150).
    pub fn is_sprinting(&self, who: usize) -> bool {
        self.bims
            .get(who)
            .is_some_and(|b| b.character.is_sprinting())
    }

    /// Whether `who` is steered by its player's keys and pointer (task
    /// 144): it walks, turns and fires only as they say.
    pub fn is_steered(&self, who: usize) -> bool {
        self.bims
            .get(who)
            .is_some_and(|b| b.character.steer().is_some())
    }

    /// Whether `who`'s player holds its trigger down: what another
    /// player's screen closes that player's crosshair on.
    pub fn trigger_held(&self, who: usize) -> bool {
        self.bims
            .get(who)
            .and_then(|b| b.character.steer())
            .is_some_and(|s| s.fire)
    }

    /// How far, in room units, a shot `who` fires now reaches: its
    /// weapon through the skill it shoots with now (`shot_skill`, the
    /// optics' tiles standing still too) — where a bolt dies, and a
    /// blade's arm's length. `None` with nothing to shoot: no weapon, or
    /// the medkit in hand. What the player's crosshair greys past.
    pub fn shot_reach(&self, who: usize) -> Option<f32> {
        let bim = self.bims.get(who)?;
        if bim.hand == Hand::Medkit {
            return None;
        }
        let weapon = bim.gear.weapon?;
        Some(self.shot_skill(who).stats(weapon).reach())
    }

    /// What `who`'s gun would fire at of its own accord (the setup's
    /// auto-shoot, October 2026): `picked` — an index in the room's
    /// target list — where that enemy is up, in reach, made out and on a
    /// clear line of fire, else the nearest such that is no sealed core.
    /// Its index and where it stands; `None` with nothing to shoot or
    /// nobody to shoot at. A steered shot leaves the muzzle along the
    /// heading and never from a peek, so only the body's own line counts.
    /// A reading for the player's screen, which turns the steered Bim onto
    /// it with the trigger held: the room is told nothing a click does
    /// not tell it.
    pub fn auto_aim(&self, who: usize, picked: Option<usize>) -> Option<(usize, Vec2)> {
        let bim = self.bims.get(who)?;
        if bim.hand == Hand::Medkit || !bim.is_alive() || bim.character.is_unconscious() {
            return None;
        }
        let reach = self.shot_skill(who).stats(bim.gear.weapon?).reach();
        let from = bim.character.pos;
        let sight = &self.room.sight;
        let shot = |t: &crate::combat::Target| {
            !t.stale
                && (t.at - from).len() <= reach
                && sight.makes_out(from, t.at)
                && crate::combat::line_of_fire(sight, from, t.at)
        };
        let targets = self.combat.targets();
        if let Some(i) = picked
            && let Some(t) = targets.get(i).copied().flatten()
            && shot(&t)
        {
            return Some((i, t.at));
        }
        targets
            .iter()
            .enumerate()
            .filter_map(|(i, t)| t.filter(|t| !t.sealed() && shot(t)).map(|t| (i, t.at)))
            .min_by(|a, b| (a.1 - from).len().total_cmp(&(b.1 - from).len()))
    }

    /// Whether target `enemy` is still up (on the list, neither down nor
    /// dead): what an enemy picked for the auto-shoot is kept for.
    pub fn enemy_up(&self, enemy: usize) -> bool {
        self.enemy_standing(enemy).is_some()
    }

    /// What `who` holds (task 138): the weapon, or the medkit.
    pub fn hand(&self, who: usize) -> Hand {
        self.bims.get(who).map_or(Hand::Weapon, |b| b.hand)
    }

    /// Where `who`'s attack-move is bound, if it is on one.
    pub fn attack_move_of(&self, who: usize) -> Option<Vec2> {
        self.bims.get(who).and_then(|b| b.attack_move)
    }

    /// A right-click on an enemy (task 126, Dota's attack order): player
    /// `slot`'s own crew member takes up arms, puts down whatever it was
    /// on and keeps at `enemy` — an index in the room's target list —
    /// until it is down or dead or the player orders something else:
    /// [`Game::chase`] walks it after the enemy until it has a shot, and
    /// the aim in [`Game::tick_combat`] fires at that one and nobody
    /// else. Ignored for a body that cannot act or has no weapon and for
    /// an index with nobody standing at it; the ping is the attack's red,
    /// where the enemy stands.
    pub fn order_attack(&mut self, slot: u32, enemy: usize) -> u32 {
        let who = slot as usize;
        let Some(at) = self.enemy_standing(enemy) else {
            return ORDER_IGNORED;
        };
        if who >= self.bims.len()
            || !self.is_alive(who)
            || self.bims[who].character.is_unconscious()
            || self.bims[who].character.is_outside()
            || self.bims[who].gear.weapon.is_none()
        {
            return ORDER_IGNORED;
        }
        self.drop_ordered(who);
        self.interrupt_for_order(who);
        let bim = &mut self.bims[who];
        bim.attack_move = None;
        bim.focus = Some(enemy);
        bim.hand = Hand::Weapon;
        bim.plan_wait = 0.0;
        bim.character.set_post(None);
        bim.character.set_recruited(true);
        bim.character.select_for(slot, true);
        self.markers.push(Marker {
            pos: at,
            age: 0.0,
            bad: false,
            kind: Ping::Attack,
            by: Some(who),
        });
        ORDER_MOVING
    }

    /// The enemy `who` was told to attack, if it is still at it.
    pub fn focus_of(&self, who: usize) -> Option<usize> {
        self.bims.get(who).and_then(|b| b.focus)
    }

    /// Where target `enemy` stands, if it is up: on the list and neither
    /// down nor dead.
    fn enemy_standing(&self, enemy: usize) -> Option<Vec2> {
        self.combat
            .targets()
            .get(enemy)
            .copied()
            .flatten()
            .map(|t| t.at)
    }

    /// The enemy under a point of the deck, for a right-click to attack
    /// (task 126): the nearest target up within [`ENEMY_PICK`] tiles of
    /// the point on a tile somebody of the crew sees, so a click into the
    /// fog finds nobody. `None` with nobody there — and always in a room
    /// with no enemies named, or a town's own people, who are handed over
    /// as nobody's target.
    pub fn enemy_at(&self, x: f32, y: f32) -> Option<usize> {
        let at = vec2(x, y);
        self.combat
            .targets()
            .iter()
            .enumerate()
            .filter_map(|(i, t)| t.map(|t| (i, (t.at - at).len(), t.at)))
            .filter(|&(_, far, pos)| far <= ENEMY_PICK * TILE && self.room.sight.seen_at(pos))
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .map(|(i, _, _)| i)
    }

    /// The walk a target calls for ([`Game::order_attack`]), from the
    /// fight's own turn for the body: with a shot at it from where it
    /// stands — a blade, within reach — it stands still, since a shot on
    /// the move is at half the odds; without one it walks after it,
    /// planned again every `PLAN_EVERY` so the walk follows a target that
    /// moves. Nobody else on the list is its business.
    fn chase(&mut self, who: usize, dt: f32, stats: &WeaponStats, enemy: usize) {
        let Some(at) = self.enemy_standing(enemy) else {
            return;
        };
        let from = self.bims[who].character.pos;
        let in_hand = if stats.melee {
            self.combat.within_reach(from, enemy)
        } else {
            self.combat
                .aim_only(&self.room.sight, from, stats, enemy)
                .is_some()
        };
        let bim = &mut self.bims[who];
        if in_hand {
            if bim.character.is_walking() {
                bim.character.halt();
            }
            return;
        }
        bim.plan_wait -= dt;
        if bim.plan_wait > 0.0 {
            return;
        }
        bim.plan_wait = PLAN_EVERY;
        if bim
            .character
            .destination()
            .is_some_and(|going| (going - at).len() <= TILE)
        {
            return;
        }
        if self.on_a_window(who, at) {
            self.plan_route(who, at);
            return;
        }
        let nav = self.maps.for_body(false);
        let route = nav.path(from, nav.nearest_free(at));
        if !route.is_empty() {
            self.bims[who].character.follow_path(route);
        }
    }

    /// One step of an attack-move, from the fight's own turn for the body
    /// ([`Game::tick_combat`]): with a shot (`shot`, or a melee lock) it
    /// stands still for it — a shot on the move is at half the odds —
    /// and with none it walks on to where it was bound, the order over
    /// once it is there or there is no way there any more.
    fn keep_attack_moving(&mut self, who: usize, shot: bool) {
        let Some(to) = self.bims[who].attack_move else {
            return;
        };
        let ch = &mut self.bims[who].character;
        if shot {
            if !ch.arrived() {
                ch.halt();
            }
            return;
        }
        if !ch.arrived() {
            return;
        }
        if (to - ch.pos).len() <= ATTACK_MOVE_THERE * TILE
            || !self.plan_route(who, to)
            || self.bims[who].character.arrived()
        {
            self.bims[who].attack_move = None;
        }
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
                self.mark(who, want, true);
                return ORDER_NOWHERE;
            }
            self.interrupt_for_order(who);
            // Putting the errand down may have stood the body up somewhere
            // else: the walk is planned again from there.
            self.plan_route(who, want);
            let stand = self.nearest_stand(who, want);
            self.mark(who, stand, false);
            return ORDER_MOVING;
        }

        // Snap to somewhere the Bim can actually stand, so an order onto the
        // table means "the floor beside the table" rather than nothing at all.
        let nav = self.maps.deck();
        let target = nav.nearest_free(want);
        let route = nav.path(self.bims[who].character.pos, target);
        if !route.is_empty() {
            // The order outranks whatever the Bim is on. That chain goes into
            // the queue and is picked up once it has been where it was sent.
            self.interrupt_for_order(who);
            self.bims[who].character.follow_path(route);
            self.mark(who, target, false);
            return ORDER_MOVING;
        }

        // No route there at all.
        self.mark(who, target, true);
        ORDER_NOWHERE
    }

    /// Every bolt's light on the deck round it, as dark as the deck is
    /// there — see [`BOLT_GLOW`]. Over the light map, under the bolts.
    fn draw_bolt_glow(&mut self) {
        for bolt in &self.combat.bolts {
            let dark = 1.0 - self.room.sight.lamplight_at(bolt.pos);
            if dark < 0.05 {
                continue;
            }
            let side = if bolt.hostile {
                crate::combat::HOSTILE_BOLT
            } else {
                crate::combat::FRIENDLY_BOLT
            };
            let light = side.mix(Color::rgb(1.0, 1.0, 1.0), 0.35);
            // The rings' alphas pile up to the middle's.
            let each = BOLT_GLOW * dark / BOLT_GLOW_RINGS as f32;
            for k in 0..BOLT_GLOW_RINGS {
                let across = BOLT_GLOW_REACH * TILE * (k + 1) as f32 / BOLT_GLOW_RINGS as f32;
                self.list.circle(bolt.pos, across, light.alpha(each));
            }
        }
    }

    /// Every ping on the deck, the way Dota draws an order: four arrows
    /// closing on the spot from the corners with a ring drawing in behind
    /// them — green for a walk, red for an attack-move — lit past white
    /// while fresh so the bloom lifts them off a dark deck, and a dark
    /// edge under each stroke so they read on a lit one. A refusal is a
    /// cross in the warm colour where the Bim cannot get to.
    ///
    /// Another player's pings are drawn in that player's own colour
    /// rather than the walk's green, unlit and see-through
    /// ([`OTHER_PING_ALPHA`]), so a deck with four players ordering about
    /// it tells the viewer's own orders from theirs at a glance.
    fn draw_pings(&mut self) {
        const SHADE: Color = Color::rgba(0.0, 0.0, 0.0, 1.0);
        // An enemy a player told its Bim to attack (task 126) wears four
        // red brackets for as long as the order stands — where the crew
        // can see it, so the mark never gives a body in the fog away.
        let marked: Vec<Vec2> = self
            .bims
            .iter()
            .filter_map(|b| b.focus)
            .filter_map(|enemy| self.enemy_standing(enemy))
            .filter(|&at| self.room.sight.seen_at(at))
            .collect();
        let lit = PING_ATTACK.glowing(1.2);
        for at in marked {
            for (dx, dy) in [(1.0, 1.0), (-1.0, 1.0), (-1.0, -1.0), (1.0, -1.0)] {
                let corner = at + vec2(dx, dy) * FOCUS_MARK;
                let arms = [
                    (corner, corner - vec2(dx, 0.0) * FOCUS_ARM),
                    (corner, corner - vec2(0.0, dy) * FOCUS_ARM),
                ];
                for &(from, to) in &arms {
                    self.list.line(from, to, 6.0, SHADE.alpha(0.45));
                }
                for &(from, to) in &arms {
                    self.list.line(from, to, 3.0, lit);
                }
            }
        }
        for m in &self.markers {
            let t = (m.age / MARKER_LIFE).clamp(0.0, 1.0);
            // Whose colour it is when it is another player's own Bim's.
            let theirs =
                m.by.filter(|&w| w != self.viewer as usize && w < self.players)
                    .and_then(|w| self.bims.get(w))
                    .and_then(|b| b.character.tint())
                    .map(|tint| tint.colour());
            let fade = if t < 0.6 { 1.0 } else { (1.0 - t) / 0.4 };
            let fade = if theirs.is_some() {
                fade * OTHER_PING_ALPHA
            } else {
                fade
            };
            let at = m.pos;
            if m.bad {
                let size = vec2(30.0, 4.0);
                for turn in [0.7, -0.7] {
                    self.list.rect(
                        at,
                        size + vec2(3.0, 3.0),
                        turn,
                        2.0,
                        SHADE.alpha(0.5 * fade),
                    );
                    self.list.rect(at, size, turn, 2.0, WARN.alpha(0.95 * fade));
                }
                self.list
                    .ring(at, 16.0 + 18.0 * t, 2.5, WARN.alpha(0.8 * fade));
                continue;
            }
            let colour = match (theirs, m.kind) {
                (Some(c), _) => c,
                (None, Ping::Move) => PING_MOVE,
                (None, Ping::Attack) => PING_ATTACK,
            };
            let lit = match theirs {
                Some(_) => colour,
                None => colour.glowing(1.0 + 0.5 * (1.0 - t)),
            };
            let close = 1.0 - (1.0 - t) * (1.0 - t);
            let r = PING_OUTER + (PING_INNER - PING_OUTER) * close;
            // The ring drawing in behind the arrows, and the spot itself.
            self.list
                .ring(at, 2.0 * (r + 6.0), 3.0, SHADE.alpha(0.35 * fade));
            self.list
                .ring(at, 2.0 * (r + 6.0), 1.8, colour.alpha(0.55 * fade));
            self.list.circle(at, 9.0, SHADE.alpha(0.45 * fade));
            self.list.circle(at, 6.0, lit.alpha(fade));
            // The arrows: one from each corner, its head on the spot's
            // side and its shaft back out, the head's barbs swept back.
            let (arm_c, arm_s) = (0.82_f32, 0.57_f32);
            for (dx, dy) in [(1.0, 1.0), (-1.0, 1.0), (-1.0, -1.0), (1.0, -1.0)] {
                let out = vec2(dx, dy) * std::f32::consts::FRAC_1_SQRT_2;
                let tip = at + out * r;
                let mut strokes = vec![(tip, tip + out * PING_SHAFT)];
                for side in [1.0, -1.0] {
                    let s = arm_s * side;
                    let arm = vec2(out.x * arm_c - out.y * s, out.x * s + out.y * arm_c);
                    strokes.push((tip, tip + arm * PING_BARB));
                }
                for &(from, to) in &strokes {
                    self.list.line(from, to, 7.5, SHADE.alpha(0.45 * fade));
                }
                for &(from, to) in &strokes {
                    self.list.line(from, to, 4.0, lit.alpha(fade));
                }
            }
            // And an attack-move's cross-hair through the spot, so the red
            // is not the only thing that tells the two apart.
            if m.kind == Ping::Attack {
                for turn in [0.0, std::f32::consts::FRAC_PI_2] {
                    self.list
                        .rect(at, vec2(18.0, 2.5), turn, 1.0, lit.alpha(0.9 * fade));
                }
            }
        }
    }

    /// A ping where Bim `who` was sent, or a cross where it cannot go.
    fn mark(&mut self, who: usize, pos: Vec2, bad: bool) {
        self.markers.push(Marker {
            pos,
            age: 0.0,
            bad,
            kind: Ping::Move,
            by: Some(who),
        });
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

    /// The clock put back to its start and wound to `minutes`, for a probe
    /// that moves the world's clock **back** — which nothing in a game
    /// does: [`Game::wind_clock`] only ever goes forward.
    pub fn set_clock_for_probe(&mut self, minutes: f32) {
        self.clock = Clock::new();
        self.wind_clock(minutes);
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

    // --- fixtures ---------------------------------------------------------

    /// Which fixture is at a point, without disturbing the selection. The
    /// host asks this on a left click, so poking at the furniture and
    /// picking the crew can share one button.
    pub fn hit_at(&mut self, x: f32, y: f32) -> u32 {
        self.hit(vec2(x, y), true)
    }

    /// [`Game::hit_at`] for a right-click, which is an order: a body lying
    /// on the deck — down, dead, a wreck, a station's person down — is
    /// not there for it, so the click lands on what it lies on (the deck,
    /// a door) and the Bim walks there. The living are hit as ever.
    pub fn hit_order_at(&mut self, x: f32, y: f32) -> u32 {
        self.hit(vec2(x, y), false)
    }

    fn hit(&mut self, p: Vec2, bodies: bool) -> u32 {
        self.note_fixtures(p);
        // A body before the deck: a click on one of the crew is the crew
        // member, whatever it is standing on — living, `HIT_BIM` (downed
        // too: the menu offers the revive); dead, `HIT_BODY`. One on its
        // feet before one lying under it, so a body on the floor never
        // hides the crewmate standing over it.
        let picked = |i: usize| self.bims[i].character.picked_at(p);
        let standing = (0..self.bims.len()).find(|&i| picked(i) && !self.is_down(i));
        let lying = || (0..self.bims.len()).find(|&i| bodies && picked(i));
        if let Some(i) = standing.or_else(lying) {
            if self.bims[i].is_alive() {
                self.hit_bim = i;
                return HIT_BIM;
            }
            self.hit_body = i;
            return HIT_BODY;
        }
        // Then a visitor that the world marked down — a resident lying in
        // its own room, drawn on this deck — which is a body to loot too.
        // The crew first: a crewmate standing over a body is the crewmate.
        for (i, at) in self.visitors.iter().enumerate() {
            let down = self.visitors_down.get(i).copied().unwrap_or(false);
            let hailable = self.visitors_hailable.get(i).copied().unwrap_or(false);
            if ((down && bodies) || (hailable && !down)) && (*at - p).len() <= PICK_RADIUS {
                self.hit_visitor = i;
                return HIT_VISITOR;
            }
        }
        // Then the room: a door, a bench, a shelf, a desk — or furniture,
        // which is nothing to act on (`Room::hit`).
        self.room.hit(p)
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
    /// so the host can ask `hit_door`, `hit_bench` and the rest after the
    /// code says which kind it was. Both a right-click (`hit_at`) and a
    /// left one (`drag_end`) note them, so the menu or the grid that opens
    /// is the fixture under *that* click.
    fn note_fixtures(&mut self, p: Vec2) {
        let room = &self.room;
        for (hit, at) in [
            (&mut self.hit_door, room.door_at(p)),
            (&mut self.hit_bench, room.bench_at(p)),
            (&mut self.hit_shelf, room.shelf_at(p)),
            (&mut self.hit_desk, room.desk_at(p)),
            (&mut self.hit_research, room.research_at(p)),
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

    /// A body that fires gives itself away: drawn for
    /// [`SEEN_FOR`] from the shot wherever it stands, fog or no fog, so
    /// a bolt out of the dark always has somebody at the end of it. A
    /// picture's question only; nothing in the fight reads it.
    pub(crate) fn reveal(&mut self, who: usize) {
        if self.seen_for.len() <= who {
            self.seen_for.resize(who + 1, 0.0);
        }
        self.seen_for[who] = SEEN_FOR;
    }

    /// Whether a Bim of this room is drawn: every one of the crew's own,
    /// none of a room nobody is looking into, and under a joined deck
    /// every friendly Bim — a site's own people, never a machine, a
    /// Manufacturer or a hostile room's body — and of the rest
    /// the ones the world said are in view, or that fired (`reveal`), a
    /// moment ago.
    pub fn body_seen(&self, who: usize) -> bool {
        // A body taken off the deck (task 129) is nobody's to see.
        if self.is_gone(who) {
            return false;
        }
        if self.show_everybody {
            return true;
        }
        match self.fog {
            Fog::Crew => true,
            Fog::All => false,
            Fog::None => {
                self.is_friendly_body(who) || self.seen_for.get(who).is_some_and(|&left| left > 0.0)
            }
        }
    }

    /// Whether that body is a friend to whoever is looking: a Bim of a
    /// room whose bodies are not hostile, and not a Manufacturer come to
    /// attack it. Never a machine.
    pub fn is_friendly_body(&self, who: usize) -> bool {
        !self.hostile_bodies && self.bims.get(who).is_some_and(|b| !b.manufacturer)
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

    /// Whose the room's tiles are. A station's room is whatever the
    /// station is to the crew. The fog is one fog over every stance
    /// (task 128), so the sight is not told.
    pub fn set_stance(&mut self, stance: Stance) {
        self.stance = stance;
    }

    /// Whose the room's tiles are, as the world last said.
    pub fn stance(&self) -> Stance {
        self.stance
    }

    /// Which of the room's tiles are somebody else's — a station's, on a
    /// joined deck, by its box in room units — and whose. Kept across a
    /// relayout.
    pub fn set_foreign(&mut self, rect: Option<Rect>, stance: Stance) {
        self.foreign = rect.map(|r| (r, stance, false));
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
    }

    /// Whether every body in this room is an enemy to whoever is looking:
    /// a hostile station's people. The ring under each — and the other
    /// side of the fight: its people shoot at the targets as enemies do,
    /// recording shots for the world rather than flying bolts here, and go
    /// to war the moment a target is named. See `tick_combat`.
    pub fn set_hostile_bodies(&mut self, hostile: bool) {
        self.hostile_bodies = hostile;
        for bim in &mut self.bims {
            // A Manufacturer is ringed an enemy's in any room (task 131).
            bim.character.set_hostile(hostile || bim.manufacturer);
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
        // A reinforcement wave of the machines standing here was told
        // where the crew are, and comes looking for them — and so was one
        // of the Manufacturers' people, as the world says (`set_told`).
        let told = self.told || self.droids.iter().any(|d| d.seeking && !d.destroyed);
        // And whoever of them is being shot at knows where from.
        let shot_at: Vec<Vec2> = self
            .bims
            .iter()
            .filter(|b| b.is_alive() && !b.character.is_unconscious() && b.under_fire > 0.0)
            .map(|b| b.character.pos)
            .chain(
                self.droids
                    .iter()
                    .filter(|d| !d.destroyed && d.under_fire > 0.0)
                    .map(|d| d.pos),
            )
            .collect();
        let (believed, stale) = believe(
            &mut self.last_seen,
            &self.room.sight,
            watched,
            |_| told,
            &eyes,
            &shot_at,
            at,
        );
        self.combat.set_targets(believed);
        self.combat.set_stale(&stale);
    }

    /// Whether this room's people were told where the crew are — a
    /// reinforcement of the Manufacturers' standing here — for
    /// [`Game::set_hostiles`] to believe every target it is handed, the
    /// way a reinforcement of the machines' (`Droid::seeking`) makes it.
    /// The world says it every step before it hands the hostiles over.
    pub fn set_told(&mut self, told: bool) {
        self.told = told;
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
    ///
    /// **But they came to attack, and know whom they came for**: the
    /// crew and every one of the site's people under arms (the guard,
    /// the mercenaries, the defenders — whoever is not sheltering) are
    /// told, believed where they stand, so a wave with nobody in sight
    /// does not wait at its gate for the defenders to come out but walks
    /// in after the nearest (the hunter's rule, `plan_droid_stand`). Out
    /// of sight they are still stale and nobody fires at them. Those
    /// sheltering in the houses are found only by looking.
    pub fn set_machine_hostiles(&mut self, at: Vec<Option<(Vec2, Weapon)>>, cross: usize) {
        let eyes: Vec<Vec2> = self
            .droids
            .iter()
            .filter(|d| !d.destroyed)
            .map(|d| d.pos)
            // And the Manufacturers come with a wave (task 131): they are
            // on the machines' side and spot for them.
            .chain(
                self.bims
                    .iter()
                    .filter(|b| b.manufacturer && b.is_alive() && !b.character.is_unconscious())
                    .map(|b| b.character.pos),
            )
            .collect();
        let shot_at: Vec<Vec2> = self
            .droids
            .iter()
            .filter(|d| !d.destroyed && d.under_fire > 0.0)
            .map(|d| d.pos)
            .chain(
                self.bims
                    .iter()
                    .filter(|b| b.manufacturer && b.is_alive() && b.under_fire > 0.0)
                    .map(|b| b.character.pos),
            )
            .collect();
        let sheltering = &self.sheltering;
        let told = |i: usize| i < cross || !sheltering.get(i - cross).copied().unwrap_or(false);
        let (believed, stale) = believe(
            &mut self.machine_seen,
            &self.room.sight,
            None,
            told,
            &eyes,
            &shot_at,
            at,
        );
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
    /// there, so it goes on living (it comes back to the post after an
    /// errand) and never takes arms: the alarm's
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

    /// Which of two taunts on the targets is the more recent, as the
    /// world last said it (task 139): nought for none. For the tests.
    #[allow(dead_code)]
    pub fn hostiles_taunt_order_for_probe(&self) -> Vec<u32> {
        self.combat
            .targets()
            .iter()
            .map(|t| t.map_or(0, |t| t.taunt_order))
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
    pub fn strip_for_probe(&mut self, who: usize, damage: f32, strips: f32) {
        self.strike_stripping(who, damage, false, strips);
    }

    pub fn set_hostiles_peeking(&mut self, peeking: &[bool]) {
        self.combat.set_peeking(peeking);
    }

    /// Which of the hostiles carry a shield and which way it faces, in
    /// this room's frame, index for index with `set_hostiles` (feature
    /// 100): the world hands the other room's [`Game::shield_of`]s across,
    /// turned through the station's frame. A bolt or a blow from inside a
    /// shield's front arc is stopped here, where it would have landed.
    pub fn set_hostiles_shields(&mut self, shields: &[Option<Vec2>]) {
        self.combat.set_shields(shields);
    }

    /// The way the shield of the body with that index faces, in this
    /// room's frame — a standing Guardian's (feature 100) — or `None` for
    /// every other body, Bims first and then the machines as ever.
    pub fn shield_of(&self, who: usize) -> Option<Vec2> {
        who.checked_sub(self.bims.len())
            .and_then(|i| self.droids.get(i))
            .and_then(|d| d.shield())
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
            // A Reactor Heart's health on the bar (October 2026), and
            // the level's.
            let level = self.bims[who].level_health;
            self.bims[who].health.set_max(gear.max_health() + level);
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

    /// [`Game::enemy_fire`] signed with who fired it, by its index on
    /// this room's targets, for a Reflect Barrier to send the hit back to
    /// (task 155).
    pub fn enemy_fire_by(
        &mut self,
        from: Vec2,
        at: Vec2,
        weapon: Weapon,
        moving: bool,
        shooter: Option<usize>,
    ) {
        self.combat.fire_hostile(from, at, weapon, moving, shooter);
    }

    /// [`Game::enemy_sweep`] signed with who swept it, as
    /// [`Game::enemy_fire_by`].
    #[allow(clippy::too_many_arguments)]
    pub fn enemy_sweep_by(
        &mut self,
        from: Vec2,
        start: Vec2,
        end: Vec2,
        weapon: Weapon,
        damage: f32,
        pace: f32,
        shooter: Option<usize>,
    ) {
        self.enemy_sweep(from, start, end, weapon, damage, pace);
        self.combat.sign_last_sweep(shooter);
    }

    /// [`Game::enemy_strike`] signed with who struck: a Reflect Barrier
    /// on the body sends its share back on the striker (task 155).
    pub fn enemy_strike_by(
        &mut self,
        from: Vec2,
        who: usize,
        damage: f32,
        cut: bool,
        shooter: Option<usize>,
    ) -> bool {
        let before = self.wounds_taken.len();
        let landed = self.enemy_strike(from, who, damage, cut);
        if landed
            && let Some(hit) = self.wounds_taken.get(before).copied()
            && let Some(back) = self.combat.reflected_hit(&hit, shooter)
        {
            self.combat.land(back);
        }
        landed
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
        self.strike(who, damage, cut);
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
                    s.facing = was.facing;
                    s.flash = was.flash;
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

    /// Send `who` to lay a kit on the tile at `tile` (room units, the
    /// tile's middle), within `task::DEPLOY_REACH` tiles of which it will
    /// stand for `minutes` of working steps — one of an engineer's
    /// deployables, the world having checked the charge and the tile.
    /// `kind` is the world's code, only carried back on `take_deployed`.
    /// The walk is towards the nearest tile beside it, or the tile
    /// itself; nowhere to stand is `false` and nothing begun. A live
    /// order: what the Bim was on is put down onto the queue, as any order
    /// does — but a deploy it was on is dropped (`interrupt`), so a kit
    /// placed somewhere else replaces the first rather than queueing
    /// behind it. A hit does not put it down.
    pub fn deploy(&mut self, who: usize, tile: Vec2, kind: u32, minutes: f32) -> bool {
        if who >= self.bims.len() || !self.bims[who].is_alive() {
            return false;
        }
        let from = self.bims[who].character.pos;
        if task::deploy_stand(&self.maps, tile, from).is_none() {
            return false;
        }
        self.interrupt_for_order(who);
        self.drop_ordered(who);
        let bim = &mut self.bims[who];
        bim.task = Some(Task::deploy(
            who,
            tile,
            kind,
            minutes,
            &mut bim.character,
            &mut self.room,
            &self.maps,
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
        let nav = self.maps.deck();
        nav.interior().contains(tile)
            && nav.is_free(tile)
            && self.room.door_at(tile).is_none()
            && task::deploy_stand(&self.maps, tile, bim.character.pos).is_some()
    }

    /// Every deployable laid since last asked: who laid it, the middle of
    /// the tile in room units, and the world's code for what it is. The
    /// world puts it down and spends the charge.
    pub fn take_deployed(&mut self) -> Vec<(usize, Vec2, u32)> {
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

    /// The skill a Bim aims and shoots with now: its own, with *Wide Angle
    /// Optics*' tiles on the range while it stands still (task 142). What
    /// `tick_combat` reads for the aim and the shot alike.
    pub fn shot_skill(&self, who: usize) -> Skill {
        let mut skill = self.skill(who);
        if self
            .bims
            .get(who)
            .is_some_and(|b| !b.character.is_walking())
        {
            skill.range += skill.still_range;
        }
        skill
    }

    /// What each Bim's talents do to the one shooter, by index
    /// (`combat::Skill`): a soldier's, worked out by the world every step
    /// from its class, its talents and its brace. `Skill::NONE` for
    /// anybody not named.
    pub fn set_skills(&mut self, skills: Vec<Skill>) {
        // Weak Spot's chances (task 124), for the fight to roll a crit by.
        self.combat
            .set_crit_chances(skills.iter().map(|s| s.crit_chance).collect());
        // And what a Reflect Barrier sends back (task 155), and its glow.
        self.combat
            .set_reflects(skills.iter().map(|s| s.reflect).collect());
        for (who, bim) in self.bims.iter_mut().enumerate() {
            let on = skills.get(who).is_some_and(|s| s.reflect > 0.0);
            bim.character.set_reflecting(on);
        }
        self.skills = skills;
    }

    /// How wide a shield's front is against each crew member's shots, as
    /// a cosine, by index — a relic's *Wide Angle Optics* (task 118). An
    /// empty list is the Guardian's own front for everybody.
    pub fn set_shield_fronts(&mut self, fronts: Vec<f32>) {
        self.combat.set_shield_fronts(fronts);
    }

    /// Lend the fight the world's Weak Spot stream for a step (task 124):
    /// every critical roll comes off it and nothing else does.
    pub fn lend_crit_rng(&mut self, rng: crate::rng::Rng) {
        self.combat.lend_crit_rng(rng);
    }

    /// And take it back after the step, drawn on.
    pub fn take_crit_rng(&mut self) -> Option<crate::rng::Rng> {
        self.combat.take_crit_rng()
    }

    /// The skill the world set for `who`, for the tests.
    #[allow(dead_code)]
    pub fn skill_for_probe(&self, who: usize) -> Skill {
        self.skill(who)
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

    /// The ship's own doors — the powered sliding ones — each as its middle and the direction through it, for the tests.
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

    /// A soldier charging a Stun Shot, or let go (feature 75's brace;
    /// October 2026): whatever it was on put down onto the queue, no
    /// errand taken, the gun up and nothing fired — and walked wherever
    /// its keys, a roll or an order take it, the charge going on, since
    /// the player's word that he walks while it charges — until it is let
    /// go or goes down. The world checks who may (`World::can_stun_shot`); the room
    /// does as told. Whether it changed.
    pub fn set_braced(&mut self, who: usize, on: bool) -> bool {
        if who >= self.bims.len() || self.bims[who].braced == on {
            return false;
        }
        if on {
            if !self.bims[who].is_alive() || self.bims[who].character.is_unconscious() {
                return false;
            }
            self.interrupt(who);
        }
        self.bims[who].braced = on;
        self.bims[who].character.set_braced(on);
        true
    }

    /// Whether a Bim is braced.
    pub fn is_braced(&self, who: usize) -> bool {
        self.bims.get(who).is_some_and(|b| b.braced)
    }

    // --- the tank: the Riot Shield and the hits (task 155) -----------------

    /// The Riot Shields held up among this room's bodies, as the world
    /// says every step: who holds one, the hit points it still takes and
    /// what it takes whole, for the picture. A body not named holds
    /// none. The plate faces the way its holder does, read afresh as the
    /// bolts fly (`tick_combat`).
    pub fn set_riot_shields(&mut self, shields: &[(usize, f32, f32)]) {
        let plates = shields
            .iter()
            .filter(|&&(who, _, _)| who < self.bims.len())
            .map(|&(who, hp, _)| crate::combat::Plate {
                who,
                facing: self.plate_facing(who),
                hp,
            })
            .collect();
        self.combat.set_plates(plates);
        for (who, bim) in self.bims.iter_mut().enumerate() {
            let share = shields.iter().find(|s| s.0 == who).map(|&(_, hp, full)| {
                if full > 0.0 {
                    (hp / full).clamp(0.0, 1.0)
                } else {
                    0.0
                }
            });
            bim.character.set_plate(share);
        }
    }

    /// The way body `who` faces, radians, nought east; nought for nobody.
    pub fn heading_of(&self, who: usize) -> f32 {
        self.bims.get(who).map_or(0.0, |b| b.character.heading)
    }

    /// The way a body's Riot Shield faces: the way the body does.
    fn plate_facing(&self, who: usize) -> Vec2 {
        self.bims
            .get(who)
            .map_or(Vec2::ZERO, |b| Vec2::from_angle(b.character.heading))
    }

    /// What the Riot Shields stopped since the world last asked: the
    /// holder and the damage each bolt carried there.
    pub fn take_plate_blocks(&mut self) -> Vec<(usize, f32)> {
        self.combat.take_plate_blocks()
    }

    /// The plates as the shooter holds them, for the tests.
    #[allow(dead_code)]
    pub fn plates_for_probe(&self) -> Vec<crate::combat::Plate> {
        self.combat.plates().to_vec()
    }

    /// Which target a Bim would shoot at from where it stands, for the
    /// tests: the one [`Combat::aim`] picks over its own weapon.
    #[allow(dead_code)]
    pub fn aims_at_for_probe(&self, who: usize) -> Option<usize> {
        let bim = self.bims.get(who)?;
        let stats = self.shot_skill(who).stats(bim.gear.weapon?);
        self.combat
            .aim(&self.room.sight, bim.character.pos, &stats)
            .map(|(i, _, _)| i)
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

    /// The taunt running on each of the enemies named by
    /// [`Game::set_hostiles`] (a [`crate::combat::Taunt`]: its radius in
    /// room units — nought for none — whether it pulls charging blades,
    /// and which of two is the more recent). Index for index with
    /// `set_hostiles`, the way [`Game::set_hostiles_peeking`] is.
    pub fn set_hostiles_taunting(&mut self, taunts: &[crate::combat::Taunt]) {
        self.combat.set_taunting(taunts);
    }

    /// The same for the machines' own list, index for index with what
    /// [`Game::set_machine_hostiles`] was handed (task 139): the crew
    /// across the seam first. What makes a tank's taunt reach the
    /// machines in a town the crew are defending.
    pub fn set_machine_hostiles_taunting(&mut self, taunts: &[crate::combat::Taunt]) {
        self.combat.set_machine_taunting(taunts);
    }

    /// Enemy hits that have landed on a body: a count that only climbs,
    /// read by the relics for a hit taken (feature 106). It made a tank's
    /// experience until task 119.
    pub fn hits_taken(&self, who: usize) -> u32 {
        self.bims.get(who).map_or(0, |b| b.hits_taken)
    }

    /// How many shots a body has fired (feature 106, `Bim::shots`).
    pub fn shots(&self, who: usize) -> u32 {
        self.bims.get(who).map_or(0, |b| b.shots)
    }

    /// Set that count, for the tests.
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
            bim.under_fire = UNDER_FIRE;
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
    /// hit takes nothing from it (`Game::strike`). The world checks who
    /// may; the room does as told. A body dead is left alone. A surge on a
    /// body already surging runs from now.
    pub fn set_surge(&mut self, who: usize, seconds: f32) {
        if let Some(bim) = self.bims.get_mut(who).filter(|b| b.is_alive()) {
            bim.surge = Some(crate::bim::Surge { left: seconds });
            bim.character.set_surging(true);
        }
    }

    /// A shield of `hp` hit points on a living body for `seconds` (task
    /// 142, a relic's *Lifeline*): a fresh one in place of what was left.
    pub fn set_shield(&mut self, who: usize, hp: f32, seconds: f32) {
        self.set_draining_shield(who, hp, seconds, 0.0);
    }

    /// [`Game::set_shield`] losing `drain` hit points a second of the
    /// room's clock whatever hits it: a tank's Bastion (task 155).
    pub fn set_draining_shield(&mut self, who: usize, hp: f32, seconds: f32, drain: f32) {
        if let Some(bim) = self.bims.get_mut(who).filter(|b| b.is_alive()) {
            bim.shield = Some(crate::bim::Shield {
                hp,
                left: seconds,
                full: hp,
                drain,
            });
            bim.character.set_shield(1.0);
        }
    }

    /// The hit points a body's shield still takes; nought with none.
    pub fn shield_hp(&self, who: usize) -> f32 {
        self.bims
            .get(who)
            .and_then(|b| b.shield)
            .map_or(0.0, |s| s.hp.max(0.0))
    }

    /// Seconds of the room's clock a body's shield has left; nought with
    /// none.
    pub fn shield_left(&self, who: usize) -> f32 {
        self.bims
            .get(who)
            .and_then(|b| b.shield)
            .map_or(0.0, |s| s.left.max(0.0))
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

    // --- the medic's cloak (task 130) ---------------------------------------

    /// Whether a body is under a medic's cloak, as the world last said:
    /// drawn faint and shimmering. Drawing only — who may pick it, and
    /// whether it fires, are the world's (its target lists and the
    /// body's `Skill`).
    pub fn set_cloaked(&mut self, who: usize, on: bool) {
        if let Some(bim) = self.bims.get_mut(who) {
            bim.character.set_cloaked(on);
        }
    }

    /// Whether a body is drawn cloaked.
    pub fn is_cloaked(&self, who: usize) -> bool {
        self.bims.get(who).is_some_and(|b| b.character.is_cloaked())
    }

    /// Whether the world has left targets off this room's list because
    /// nobody may pick them (task 130): an enemy left with nobody it may
    /// pick holds where it stands. Said every step.
    pub fn set_targets_withheld(&mut self, withheld: bool) {
        self.withheld = withheld;
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

    /// Whether a body is worth fetching out of the fire: downed on this
    /// deck (task 120) — a body up walks itself out — and not a
    /// Manufacturer, whom nobody saves.
    pub fn needs_rescue(&self, who: usize) -> bool {
        self.bims.get(who).is_some_and(|b| {
            b.is_alive() && !b.manufacturer && !b.character.is_outside() && b.health.downed()
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
        let nav = self.maps.for_body(false);
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

    /// The world's word of the hit points a crew member's level puts on
    /// its bar (October 2026: ten a level, a player's), said every step:
    /// the whole bar is its items' and this, what is in it kept at the
    /// same share ([`crate::health::Health::set_max`]).
    pub fn set_level_health(&mut self, who: usize, hp: f32) {
        if let Some(bim) = self.bims.get_mut(who) {
            if bim.level_health != hp {
                bim.level_health = hp;
                bim.health.set_max(bim.gear.max_health() + hp);
            }
        }
    }

    /// The world's word that a crew member is a commander's Medivac medic
    /// (his C): said every step, as a field medic's trade is.
    pub fn set_medivac(&mut self, who: usize, on: bool) {
        if let Some(bim) = self.bims.get_mut(who) {
            bim.medivac = on;
        }
    }

    /// Whether this body is a Medivac medic, as the world last said.
    pub fn is_medivac(&self, who: usize) -> bool {
        self.bims.get(who).is_some_and(|b| b.medivac)
    }

    /// The player a Medivac medic runs to: the nearest player's own Bim
    /// downed that it can get beside, that nobody else has hands on or is
    /// on the way to — **the fight round the body never asked**, where a
    /// bot's own revive waits for the patient to lie out of harm. `None`
    /// for anybody but a medic on its feet with its arms free.
    fn medivac_patient(&self, who: usize) -> Option<usize> {
        let bim = self.bims.get(who)?;
        if !bim.medivac
            || !bim.is_alive()
            || bim.health.downed()
            || bim.character.is_unconscious()
            || bim.character.is_outside()
            || bim.carrying.is_some()
        {
            return None;
        }
        let from = bim.character.pos;
        (0..self.players.min(self.bims.len()))
            .filter(|&p| p != who && self.can_be_revived(p))
            .filter(|&p| !self.is_being_seen_to_by_another(p, who))
            .filter(|&p| task::patient_stand(&self.room, &self.maps, who, p, from).is_some())
            .min_by(|&a, &b| {
                (self.bims[a].character.pos - from)
                    .len()
                    .total_cmp(&(self.bims[b].character.pos - from).len())
            })
    }

    /// A Medivac medic with a player down to reach and no revive in hand
    /// puts whatever it was doing down and goes to it.
    fn medivac_rush(&mut self, who: usize) {
        if !self.is_medivac(who) || self.reviving(who).is_some() {
            return;
        }
        if let Some(patient) = self.medivac_patient(who) {
            self.revive_crewmate(who, patient);
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
        let nav = self.maps.deck();
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

    /// The targets an EMP reached since last asked (task 127): each
    /// target's index, the seconds of its stun and whether it exposes.
    pub fn take_stuns(&mut self) -> Vec<(usize, f32, bool)> {
        std::mem::take(&mut self.stuns)
    }

    /// An engineer's satchel charge lobbed from `who`'s hands at `at`
    /// (task 154), room units: the grenade's flight, and it lands rather
    /// than bursts ([`Game::take_satchels_landed`]). The world checks the
    /// throw and spends the charge; the room throws.
    pub fn throw_satchel(&mut self, who: usize, at: Vec2) {
        if who >= self.bims.len() {
            return;
        }
        let from = self.bims[who].character.pos;
        if (at - from).len() > 1e-3 && !self.bims[who].character.is_walking() {
            self.bims[who].character.face((at - from).angle());
        }
        self.combat.lob_satchel(who, from, at);
    }

    /// The satchel charges that came down since last asked: who threw each
    /// and where it landed.
    pub fn take_satchels_landed(&mut self) -> Vec<(usize, Vec2)> {
        std::mem::take(&mut self.satchels_landed)
    }

    /// An engineer's mine or satchel set off where it lies (task 154), by
    /// crew member `who` at `at`: it bursts on this room's next tick with
    /// `radius` room units and `damage` at the centre — every target
    /// standing within it with a clear line, half at the edge and halved
    /// again in cover, as a grenade's; none of this room's own, no sentry
    /// and no sandbag.
    pub fn detonate(&mut self, who: usize, at: Vec2, radius: f32, damage: f32) {
        self.combat.detonate(who, at, radius, damage);
    }

    /// Whether an enemy — a target the world named, standing — is within
    /// `reach` room units of `at`: what sets a mine off (task 154).
    pub fn enemy_near(&self, at: Vec2, reach: f32) -> bool {
        self.combat
            .targets()
            .iter()
            .flatten()
            .any(|t| (t.at - at).len() <= reach)
    }

    /// A soldier's Stun Shot fired from `who`'s gun (October 2026) at
    /// `at`, room units: it flies `flight` seconds and bursts there with
    /// `radius` room units, `damage` to every target in it and a stun of
    /// `stun` seconds (`Game::take_stuns`). The world checks it and
    /// works out where it lands ([`Game::reach_along`]); the room fires.
    pub fn fire_stun_shot(
        &mut self,
        who: usize,
        at: Vec2,
        flight: f32,
        radius: f32,
        damage: f32,
        stun: f32,
    ) {
        let Some(bim) = self.bims.get(who) else {
            return;
        };
        let weapon = bim
            .gear
            .weapon
            .unwrap_or(crate::combat::WeaponKind::LaserPistol.basic());
        let from = bim.character.muzzle().unwrap_or(bim.character.pos);
        if (at - from).len() > 1e-3 && !self.bims[who].character.is_walking() {
            self.bims[who].character.face((at - from).angle());
        }
        self.combat
            .fire_stun_shot(who, from, at, flight, radius, damage, stun, weapon);
    }

    /// How far a shot from `from` towards `to` gets: `to` itself where
    /// nothing opaque stands between, else the last point short of the
    /// first wall or shut door on the line, a quarter of a tile at a
    /// time.
    pub fn reach_along(&self, from: Vec2, to: Vec2) -> Vec2 {
        if self.line_clear(from, to) {
            return to;
        }
        let span = (to - from).len();
        let step = TILE * 0.25;
        let dir = (to - from).normalize_or_zero();
        let mut last = from;
        let mut flown = step;
        while flown < span {
            let p = from + dir * flown;
            if !self.line_clear(from, p) {
                break;
            }
            last = p;
            flown += step;
        }
        last
    }

    /// How far a Stun Shot charging has come, nought to one, for the
    /// glow at the muzzle (October 2026): drawing only.
    pub fn set_shot_charge(&mut self, who: usize, share: f32) {
        if let Some(b) = self.bims.get_mut(who) {
            b.character.set_shot_charge(share);
        }
    }

    /// Where `who`'s Stun Shot charging goes, room units, or `None` with
    /// none: the body faces it at once while it charges, wherever its
    /// feet go (October 2026, the player's word). The world says it every
    /// step.
    pub fn set_shot_at(&mut self, who: usize, at: Option<Vec2>) {
        if let Some(b) = self.bims.get_mut(who) {
            b.character.set_shot_at(at);
        }
    }

    /// The player's steer on `who`, if its keys and pointer have it: where
    /// a Stun Shot is fired along.
    pub fn steer_of(&self, who: usize) -> Option<crate::character::Steer> {
        self.bims.get(who).and_then(|b| b.character.steer())
    }

    /// Machine `i` of this room stunned for `seconds` (task 127,
    /// `Droid::stun`): whether it took.
    pub fn stun_droid(&mut self, i: usize, seconds: f32, expose: bool) -> bool {
        self.droids
            .get_mut(i)
            .is_some_and(|d| d.stun(seconds, expose))
    }

    /// Bim `who` of this room stunned for `seconds` by a Stun Shot
    /// (October 2026): the longer of two stuns, and whatever it had begun
    /// dropped — the walk, the errand put down onto the queue, the blow
    /// on its way. Nobody dead, down or outside. Whether it took.
    pub fn stun_bim(&mut self, who: usize, seconds: f32) -> bool {
        let Some(bim) = self.bims.get(who) else {
            return false;
        };
        if seconds <= 0.0
            || !bim.is_alive()
            || bim.character.is_unconscious()
            || bim.character.is_outside()
        {
            return false;
        }
        self.interrupt(who);
        let bim = &mut self.bims[who];
        bim.stunned = bim.stunned.max(seconds);
        bim.blow = None;
        bim.locked = None;
        bim.peek = None;
        bim.trigger.hold();
        bim.character.halt();
        bim.character.set_stunned(true);
        true
    }

    /// Seconds Bim `who` is still stunned for; nought when it is not.
    pub fn bim_stunned(&self, who: usize) -> f32 {
        self.bims.get(who).map_or(0.0, |b| b.stunned)
    }

    /// A grenade's hit on one of this room's own: `strike` — through the
    /// armour — whose splash is every hit's that takes hit
    /// points (task 120).
    pub fn blast(&mut self, who: usize, damage: f32) -> WoundOutcome {
        self.strike(who, damage, false)
    }

    /// What a grenade's burst reaches, at `g.at` with `g.radius`: every
    /// body of this room's own on its feet and every target standing,
    /// within the radius and with nothing opaque between (walls and shut
    /// doors stop it, sandbags do not), takes `g.damage` at the centre
    /// falling in a straight line to half at the edge — halved again for
    /// a body in cover from the burst's side, peeking or behind bags —
    /// with a roll off the combat stream; the sentries in it the
    /// same, on their health; the laid sandbags in it are gone. The parts
    /// of the ship and the station are untouched. Own bodies first, by
    /// index, then the targets, then the sentries, then the bags, so two
    /// runs on one seed roll the same.
    fn burst(&mut self, g: Grenade) {
        // A satchel charge landing (task 154): nothing burst, it lies where
        // it came down once the world has laid it.
        if g.satchel {
            self.satchels_landed.push((g.by, g.at));
            return;
        }
        // A soldier's Stun Shot (October 2026): every target standing
        // within its radius with nothing opaque between takes its damage,
        // the same at the edge, and is noted for the world to stun. None
        // of this room's own, no sentry and no sandbag is touched.
        if g.shot {
            let reached: Vec<usize> = self
                .combat
                .targets()
                .iter()
                .enumerate()
                .filter_map(|(i, t)| {
                    t.filter(|t| (t.at - g.at).len() <= g.radius && self.line_clear(g.at, t.at))
                        .map(|_| i)
                })
                .collect();
            for i in reached {
                if g.damage > 0.0 {
                    self.combat.blast_target(i, g.damage, Some(g.by));
                }
                self.stuns.push((i, g.stun, false));
            }
            return;
        }
        let reaches = |game: &Game, at: Vec2| -> Option<f32> {
            let d = (at - g.at).len();
            if d > g.radius || !game.line_clear(g.at, at) {
                return None;
            }
            let share = 1.0 - 0.5 * (d / g.radius.max(1e-3));
            Some(g.damage * share)
        };
        // A Guardian's grenade (October 2026) is an enemy's: its hits carry
        // nobody, and it bursts on this room's own alone — no target, and no
        // intruder, who is on the machines' side.
        let by = (!g.hostile).then_some(g.by);
        // This room's own, the thrower included — but never by an
        // engineer's mine or satchel (task 154), which hurts the enemy alone.
        let crew = if g.laid { 0 } else { self.bims.len() };
        for who in 0..crew {
            let b = &self.bims[who];
            if !b.is_alive() || b.character.is_outside() || b.character.is_unconscious() {
                continue;
            }
            if g.hostile && self.is_intruder(who) {
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
            self.blast(who, hit.damage);
            self.wounds_taken.push(hit);
            self.attacked_for = ALARM_HOLD;
            self.combat.cues.push(Cued {
                cue: Cue::Impact { on_crew: true },
                at,
            });
        }
        // The targets: the enemy's people standing on this deck, at their
        // exposed positions, in cover the same way.
        let targets: Vec<Option<(Vec2, bool)>> = if g.hostile {
            Vec::new()
        } else {
            self.combat
                .targets()
                .iter()
                .map(|t| t.map(|t| (t.at, t.peeking)))
                .collect()
        };
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
        // A mine or a satchel touches no sentry and no sandbag (task 154).
        if g.laid {
            return;
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

    /// A hit on a target, landed as a bolt would land it, for the tests
    /// (task 124): the world carries it across on its next step.
    #[allow(dead_code)]
    pub fn land_hit_for_probe(&mut self, hit: crate::combat::Hit) {
        self.combat.land_hit_for_probe(hit);
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

    /// Whether `who` is on a deploy errand, for the world and the panels.
    pub fn is_deploying(&self, who: usize) -> bool {
        self.bims
            .get(who)
            .and_then(|b| b.task.as_ref())
            .is_some_and(|t| matches!(t.kind(), Kind::Deploy { .. }))
    }

    /// Where `who` is putting something together and how far through the
    /// errand it is (feature 91): the middle of the tile an engineer is
    /// laying a kit on, the middle of the construction site whoever it
    /// is is building, in **room** units, with the chain's progress from
    /// nought to one — a revive's bar is [`Game::revive_share`]'s. `None` for every other errand and for a Bim with
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
        // A revive has a bar of its own over the patient
        // ([`Game::revive_share`]), the hands-on seconds alone.
        if self.reviving(who).is_some() {
            return None;
        }
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

    /// Every lamp whose middle lies in `over` switched off, or none: a dark
    /// station (task 152). The world's word; it outlives a relayout. See
    /// `Sight::set_lamps_off`.
    pub fn set_lamps_off(&mut self, over: Option<Rect>) {
        self.room.sight.set_lamps_off(over);
    }

    /// The daylight, as the world set it.
    pub fn daylight(&self) -> Option<Rect> {
        self.room.sight.daylight()
    }

    /// Whether the tile a room point is in counts as lit — half its light
    /// or more (`Sight::lit_at`) — and how much light falls there, nought
    /// to one (`Sight::light_at`).
    pub fn lit_at(&self, x: f32, y: f32) -> bool {
        self.room.sight.lit_at(vec2(x, y))
    }

    pub fn light_at(&self, x: f32, y: f32) -> f32 {
        self.room.sight.light_at(vec2(x, y))
    }

    /// Night on a planet (task 152): the open ground beyond the deck is
    /// seen [`crate::sight::DARK_RANGE`] tiles and no further, under the
    /// dark. The deck's half is the daylight left off (`set_daylight`),
    /// which is the world's to do beside this; only the lamps light it
    /// then. Nothing for a room with no plain.
    pub fn set_night(&mut self, night: bool) {
        self.room.sight.set_night(night);
        if let Some(plane) = self.room.plane.as_mut() {
            plane.set_night(night);
        }
    }

    /// A bolt landed on lamp `i` for `damage`, with nothing fired: the
    /// hit as the fight would land it, flicker and all, and the world
    /// told of it like any other. For probes and `BIMS_LAMPS_OUT`.
    pub fn damage_lamp_for_probe(&mut self, i: usize, damage: f32) {
        self.room.sight.damage_lamp(i, damage);
    }

    /// This room takes the place of `shown`, the one drawn until now — a
    /// guest's rollback (task 156): what the host's pictures were handed
    /// is carried across (`Sight::adopt_picture`, `Plane::adopt_picture`).
    pub fn adopt_picture(&mut self, shown: &mut Game) {
        self.room.sight.adopt_picture(&mut shown.room.sight);
        if let (Some(plane), Some(was)) = (self.room.plane.as_mut(), shown.room.plane.as_mut()) {
            plane.adopt_picture(was);
        }
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

    /// The bar set outright, for a probe or a test: nought is downed, with
    /// the whole countdown ahead.
    #[allow(dead_code)]
    pub fn set_health_for_probe(&mut self, who: usize, points: f32) {
        self.bims[who].health.set_points_for_probe(points);
    }

    /// A body laid where it fell, for a room built over a grave (feature
    /// 85): stood at `at` — snapped to somewhere a body fits, the way an
    /// [`Game::adopt`] snaps one — and dead from this instant, without
    /// waiting for a tick. The death happened before this room was built;
    /// what is being laid out is the station's memory of it, and the living
    /// here never saw it.
    pub fn lay_out_dead(&mut self, who: usize, at: Vec2) {
        if who >= self.bims.len() {
            return;
        }
        let nav = self.maps.deck();
        let spot = nav.nearest_free(at);
        self.bims[who].character.stand_at(spot);
        self.bims[who].character.set_scripted(false);
        self.bims[who].task = None;
        self.bims[who].queue.clear();
        self.bims[who].health.give_up();
        self.bims[who].character.die();
        // Its bunk is nobody's: the living here are dealt the bunks.
        if let Some(bed) = self.room.bunk_of.get_mut(who) {
            *bed = None;
        }
    }

    /// Downed where it stands, for a probe: the bar at nought and the
    /// countdown started. Takes at the top of its next tick, like a hit.
    #[allow(dead_code)]
    pub fn knock_out_for_probe(&mut self, who: usize) {
        self.bims[who].health.set_points_for_probe(0.0);
    }

    /// A shot landed on one of this room's Bims: [`Game::strike`], not a
    /// cut.
    pub fn wound(&mut self, who: usize, damage: f32) -> WoundOutcome {
        self.strike(who, damage, false)
    }

    /// A shot or a blow landed on one of this room's Bims. The armour
    /// takes it first, if there is any and it is not broken — a hit lands
    /// on the body and nowhere in particular (October 2026): the piece's protection comes off the damage —
    /// nothing left is nothing — and what remains drains the piece's
    /// health; only what the piece could not take comes off the one bar
    /// (`Health::hit`, task 120), and at nothing the body is downed. A
    /// piece at nothing is broken and does nothing from then on. A flash on
    /// the body either way. The blood: a hit that took hit points throws a
    /// small splash over the tiles round the body (`Blood::splash`); one
    /// the armour took whole throws none. `cut` is kept for the callers
    /// and changes nothing now.
    ///
    /// The outcome says how it went — what the armour took, protection
    /// included, what got through, whether the piece broke and whether the
    /// body went down — for the caller's information; a piece breaking is also
    /// kept on [`Game::take_pieces_broken`] for the world, since the room
    /// applies an enemy's shots itself.
    pub fn strike(&mut self, who: usize, damage: f32, cut: bool) -> WoundOutcome {
        self.strike_stripping(who, damage, cut, 0.0)
    }

    /// [`Game::strike`] with the Unmaker's **strip** carried (feature
    /// 83, `bims::combat::WeaponStats::strips`): with `strips` above
    /// nought and the body wearing **unbroken** armour, the armour loses
    /// that much with its own protection ignored and the body takes
    /// nothing — what the armour cannot take is **lost**, rather than
    /// reaching the body. A bare body, or one whose armour is already
    /// broken, takes the plain `damage` the ordinary way. Every other
    /// weapon strips nought and this is `strike` exactly.
    ///
    /// A medic's surge still takes the whole of it.
    pub fn strike_stripping(
        &mut self,
        who: usize,
        damage: f32,
        cut: bool,
        strips: f32,
    ) -> WoundOutcome {
        let mut out = WoundOutcome::default();
        if !self.bims.get(who).is_some_and(|b| b.is_alive()) {
            return out;
        }
        // One of the Manufacturers struck knows it is shot at, as a
        // machine does (`believe`'s `shot_at`): the crew's hits reach it
        // here, not through `count_hit_taken`.
        if self.bims[who].manufacturer {
            self.bims[who].under_fire = UNDER_FIRE;
        }
        // What a relic makes of every hit on this body (task 118): *Tether
        // Field*, *Cover Formation*. One for everybody else, and a hit
        // times one is the hit.
        let damage = damage * self.skill(who).damage_taken;
        // An *Ablative Shell* on (October 2026): nothing strips its armour;
        // a lance's bolt lands as any bolt does.
        let strips = if self.skill(who).unstrippable {
            0.0
        } else {
            strips
        };
        // A relic's shield (task 142) takes what it can of the hit before
        // anything else does — a surge excepted, which takes the whole of
        // it and leaves the shield as it was.
        let mut out_shield = 0.0;
        let damage = {
            let bim = &mut self.bims[who];
            match bim.shield.as_mut() {
                Some(shield) if bim.surge.is_none() => {
                    let took = damage.min(shield.hp.max(0.0));
                    shield.hp -= took;
                    if shield.hp <= 0.0 {
                        bim.shield = None;
                    }
                    bim.character
                        .set_shield(bim.shield.map_or(0.0, |s| s.share()));
                    out_shield = took;
                    damage - took
                }
                _ => damage,
            }
        };
        if out_shield > 0.0 && damage <= 0.0 {
            let bim = &mut self.bims[who];
            bim.hit_flash = HIT_FLASH;
            self.combat.fx.struck(who, BIM_STRUCK);
            out.absorbed = out_shield;
            return out;
        }
        if strips > 0.0 {
            let bim = &mut self.bims[who];
            bim.hit_flash = HIT_FLASH;
            if bim.surge.is_some() {
                out.absorbed = damage;
                self.combat.fx.struck(who, BIM_STRUCK);
                return out;
            }
            let mut broke = None;
            if let Some(piece) = bim.gear.worn_mut().as_mut().filter(|p| !p.broken()) {
                // Protection ignored, and the rest of the strip lost
                // with the piece: the lance unmakes the armour and does
                // nothing to what is under it.
                self.combat.fx.struck(who, BIM_STRUCK);
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
            // Nothing worn, or nothing left of it: the body takes the
            // damage like any other hit.
        }
        // A hit on an engineer laying a kit puts nothing down: it keeps
        // at the work under fire.
        let skill = self.skill(who);
        let bim = &mut self.bims[who];
        bim.hit_flash = HIT_FLASH;
        // And the picture's flash on the body (feature 98).
        self.combat.fx.struck(who, BIM_STRUCK);
        // A medic's surge on it takes the whole of the hit (feature 76):
        // no wound, no armour drained, no trauma — the flash and nothing
        // else.
        if bim.surge.is_some() {
            out.absorbed = damage;
            return out;
        }
        let mut through = damage;
        let mut broke = None;
        if let Some(piece) = bim.gear.worn_mut().as_mut().filter(|p| !p.broken()) {
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
        let _ = cut;
        // A body that may not go down (a Juggernaut at the Override Core's
        // fifth rank, October 2026) keeps a hit point whatever comes.
        if skill.unyielding {
            through = through.min((self.bims[who].health.points() - 1.0).max(0.0));
        }
        if through > 0.0 {
            let bim = &mut self.bims[who];
            let taken = bim.health.hit(through);
            out.through = taken;
            out.downed = taken > 0.0 && bim.health.downed();
            if out.downed {
                self.downs.push(who);
            }
            // Every hit that took hit points throws a small splash; one the
            // armour took whole throws none (task 120).
            if taken > 0.0 {
                let at = bim.character.pos;
                let nav = self.maps.for_body(false);
                self.room
                    .blood
                    .splash(at, &mut self.rng, |tile| nav.can_reach(at, tile));
            }
            self.refresh_bleeding(who);
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

    /// Downed — lying where it dropped with the countdown running
    /// (task 120). Read off the body as its last tick left it, the way a
    /// hit is taken at the top of the body's next tick: what the world
    /// reads a fall off, before the step and after it. A machine is never
    /// downed: there is no such state for one, so it is up or it is a
    /// wreck.
    pub fn is_downed(&self, who: usize) -> bool {
        match self.droid_at(who) {
            Some(_) => false,
            None => self.bims[who].character.is_unconscious(),
        }
    }

    /// The seconds a downed Bim has left before it is dead, `None` for one
    /// up, dead or a machine — what the countdown ring over it shows.
    pub fn down_left(&self, who: usize) -> Option<f32> {
        self.bims.get(who).and_then(|b| b.health.down_left())
    }

    /// Whether a Bim has been downed this mission (remembered only: it
    /// no longer slows the walk).
    pub fn was_downed(&self, who: usize) -> bool {
        self.bims.get(who).is_some_and(|b| b.health.was_downed())
    }

    /// The blotch of blood on the coverall while a body bleeds — under
    /// twenty hit points, downed or not. Drawing only.
    fn refresh_bleeding(&mut self, who: usize) {
        let bim = &mut self.bims[who];
        let bleeds = bim.health.bleeds();
        bim.character.set_bleeding(bleeds);
    }

    // --- reviving a downed crewmate (task 120) -------------------------------

    /// Send `who` to revive `patient`, a downed crewmate: the walk over and
    /// the helper's revive time with hands on it (`Skill::revive`), the
    /// patient up again at three tenths of its bar when the hands come off
    /// (`apply_revives`). The player's order, and what the medical row
    /// starts for a bot; it displaces whatever the helper was on, like any
    /// other order.
    ///
    /// Refused for a helper that cannot do it — dead, downed, outside, its
    /// arms full — a patient that is not downed, a Manufacturer (never
    /// revived), outside or in somebody's arms, the helper itself, and a
    /// patient another player's Bim is already reviving: one reviver
    /// counts. A **bot** on its way to the patient or at it gives way to
    /// a player's own Bim — its revive is dropped and the player's hands
    /// count — so a bot that claimed a patient and was held up never
    /// keeps the player from it.
    pub fn revive_crewmate(&mut self, who: usize, patient: usize) -> bool {
        // A townsperson downed on the joined deck (`GUEST`) is revived
        // under the same rules as a crewmate, bar what only its own room
        // knows — whether it is carried — which the world has asked.
        let guest = self.is_revivable_guest(patient);
        if who >= self.bims.len()
            || (patient >= self.bims.len() && !guest)
            || who == patient
            || !self.bims[who].is_alive()
            || self.bims[who].character.is_unconscious()
            || self.bims[who].health.downed()
            || self.bims[who].character.is_outside()
            || self.bims[who].carrying.is_some()
            || !(guest || self.can_be_revived(patient))
        {
            return false;
        }
        if self.is_being_seen_to_by_another(patient, who) {
            let others: Vec<usize> = (0..self.bims.len())
                .filter(|&other| other != who && self.reviving(other) == Some(patient))
                .collect();
            if self.is_bot(who) || others.iter().any(|&other| !self.is_bot(other)) {
                return false;
            }
            for other in others {
                self.drop_task(other);
            }
        }
        let kind = Kind::Revive { patient };
        if !self.take_over(who, kind) {
            return false;
        }
        // The walk picks its spot from where the patient lies, and the
        // order may come before the room has been told this step.
        self.tell_the_room_where_the_crew_are();
        let seconds = self.skill(who).revive.max(0.0);
        self.bims[who].task = Some(Task::revive(
            who,
            patient,
            seconds,
            &mut self.bims[who].character,
            &mut self.room,
            &self.maps,
        ));
        true
    }

    /// Every revive the chains finished this step, done: the patient up
    /// at three tenths of its bar — four for a medic's (task 130,
    /// `Skill::revived_to`) — and slowed for the rest of the mission.
    /// Only where the helper is still beside it — within two tiles — and it
    /// is still downed: a patient carried off mid-revive, or dead in the
    /// meantime, is the seconds lost and nothing else.
    fn apply_revives(&mut self) {
        for (helper, patient) in core::mem::take(&mut self.room.revived) {
            // A townsperson on the joined deck: the same two tiles, and
            // the world brings it round in its own room.
            if let Some(at) = self.guest_pos(patient) {
                if helper < self.bims.len()
                    && (self.bims[helper].character.pos - at).len() <= 2.0 * TILE
                {
                    self.guest_revives.push(GuestRevived {
                        helper,
                        visitor: patient - GUEST,
                        share: self.skill(helper).revived_to,
                    });
                }
                continue;
            }
            if helper >= self.bims.len() || !self.can_be_revived(patient) {
                continue;
            }
            let apart = (self.bims[helper].character.pos - self.bims[patient].character.pos).len();
            if apart > 2.0 * TILE {
                continue;
            }
            // Up at the helper's share of the bar (task 130): a medic's is
            // more than anybody else's.
            let share = self.skill(helper).revived_to;
            if self.bims[patient].health.revive_at(share) {
                self.bims[patient].character.knock_out(false);
                self.refresh_bleeding(patient);
                self.revives.push(Revived { helper, patient });
            }
        }
    }

    /// Every revive finished since the world last asked (task 120).
    pub fn take_revives(&mut self) -> Vec<Revived> {
        std::mem::take(&mut self.revives)
    }

    /// Every Bim a hit downed since the world last asked.
    pub fn take_downs(&mut self) -> Vec<usize> {
        std::mem::take(&mut self.downs)
    }

    /// Whether this room's own Bims revive one another of their own accord
    /// (task 120): the world says the crew's room does and a station's or
    /// a town's does not. A player's order is refused nowhere.
    pub fn set_revivers(&mut self, on: bool) {
        self.revivers = on;
    }

    /// The patient `who` is reviving or on its way to revive, if any.
    pub fn reviving(&self, who: usize) -> Option<usize> {
        self.bims
            .get(who)?
            .task
            .as_ref()
            .and_then(|t| t.kind().patient())
    }

    /// How far the hands on `patient` have got bringing it round, from
    /// nought to one, and whose they are — the walk over not counted —
    /// or `None` while nobody is kneeling at it. The bar the app draws
    /// over a downed body.
    pub fn revive_share(&self, patient: usize) -> Option<(usize, f32)> {
        self.bims.iter().enumerate().find_map(|(helper, b)| {
            let task = b.task.as_ref()?;
            if helper == patient || task.kind().patient() != Some(patient) {
                return None;
            }
            task.revive_share().map(|share| (helper, share))
        })
    }

    /// Whether `who` is in cover this instant, and from where: the point
    /// the threat stands at that its cover is against, in room units, or
    /// `None` in the open. **In cover is what a bolt reaching the body is
    /// dodged for** (`Combat::step`): peeking round a wall
    /// ([`Game::peek`]), or low cover — sandbags — on the straight line
    /// to an enemy within reach of the body (`Sight::cover_between`). A
    /// peek is against the nearest enemy up (or the way it leans, with
    /// none); the bags against the nearest enemy they stand between. Only
    /// for a body up on the deck, and only with an enemy to be in cover
    /// from — a body behind a barricade in peace is just standing there.
    /// Drawing only: nothing the simulation reads.
    pub fn cover_of(&self, who: usize) -> Option<Vec2> {
        let bim = self.bims.get(who)?;
        if !bim.is_alive()
            || bim.health.downed()
            || bim.character.is_unconscious()
            || bim.character.is_outside()
        {
            return None;
        }
        let at = bim.character.pos;
        let threats = self
            .combat
            .targets()
            .iter()
            .flatten()
            .filter(|t| !t.stale)
            .map(|t| t.at);
        let nearest = |a: &Vec2, b: &Vec2| (*a - at).len().total_cmp(&(*b - at).len());
        if let Some(peek) = bim.peek {
            return threats.min_by(nearest).or(Some(peek + (peek - at)));
        }
        threats
            .filter(|&t| self.room.sight.cover_between(at, t).is_some())
            .min_by(nearest)
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

    /// A commander's reinforcement is a Republic soldier
    /// ([`Outfit::Republic`]), trimmed in the colour of the player who
    /// called it in: that player's own Bim's ring (`set_tints`), or the
    /// slot's place in [`Tint::ALL`] where nobody has said one — which
    /// is the colour a session deals the slot by default. Drawing only,
    /// the world's to say every step like `set_outfit`. A `medic` (the
    /// commander's Medivac) wears the medic's [`Outfit::RepublicMedic`].
    pub fn set_republic(&mut self, who: usize, by: usize, medic: bool) {
        let tint = self
            .bims
            .get(by)
            .and_then(|b| b.character.tint())
            .unwrap_or(Tint::ALL[by % Tint::ALL.len()]);
        let outfit = if medic {
            Outfit::RepublicMedic(tint)
        } else {
            Outfit::Republic(tint)
        };
        self.set_outfit(who, outfit);
    }

    pub fn outfit(&self, who: usize) -> Outfit {
        self.bims[who].character.outfit()
    }

    // --- the loadout: what is worn and held, and the charges ------------------

    /// The armour worn, broken or not.
    pub fn worn(&self, who: usize) -> Option<Piece> {
        self.bims[who].gear.worn()
    }

    /// What the armour adds all told — the blue bar on the end of the
    /// green one.
    pub fn armour_health(&self, who: usize) -> f32 {
        self.bims[who].gear.armour_health()
    }

    /// How many of a charge a Bim has left (task 113): a medkit, the
    /// dressings, an engineer's kits, a grenade — `Item::Stack` of the
    /// charge's code. Nought for anybody the room has not got.
    pub fn charges_of(&self, who: usize, item: Item) -> u32 {
        self.bims.get(who).map_or(0, |b| b.gear.units_of(item))
    }

    /// Put `n` more of a charge on a Bim; how many went on.
    pub fn give_stack(&mut self, who: usize, item: Item, n: u32) -> u32 {
        self.bims.get_mut(who).map_or(0, |b| b.gear.add(item, n))
    }

    /// Take up to `n` of a charge off a Bim; how many came off.
    pub fn take_stack(&mut self, who: usize, item: Item, n: u32) -> u32 {
        let Some(bim) = self.bims.get_mut(who) else {
            return 0;
        };
        let mut gone = 0;
        while gone < n && bim.gear.spend(item) {
            gone += 1;
        }
        gone
    }

    /// Leave exactly `n` of a charge on a Bim.
    pub fn set_charges(&mut self, who: usize, item: Item, n: u32) {
        if let Some(bim) = self.bims.get_mut(who) {
            bim.gear.set_units(item, n);
        }
    }

    /// Down: dead, or downed. What the world hands the other room as
    /// `set_visitors_down`, and what a carry and a rescue ask.
    pub fn is_down(&self, who: usize) -> bool {
        if let Some(i) = self.droid_at(who) {
            // A wreck is down for everything that asks.
            return self.droids[i].destroyed;
        }
        self.bims
            .get(who)
            .is_some_and(|b| !b.is_alive() || b.character.is_unconscious())
    }

    /// The picture of what a Bim wears, put right after its gear changed:
    /// the armour and whether it is broken, for the plate and the guards
    /// on the deck.
    fn refresh_worn(&mut self, who: usize) {
        let bim = &mut self.bims[who];
        let worn = bim.gear.worn().map(|piece| Worn {
            kind: piece.kind,
            broken: piece.broken(),
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
                if i < self.room.fridges.len() {
                    self.room.fridge_station(i)
                } else {
                    None
                }
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
    /// that wants a deck seen from somewhere else: none at all is a deck
    /// nobody is looking at. The next `observe` puts the crew's back.
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
    /// the doors open for them too.
    fn shut_now(&self) -> Vec<Rect> {
        let bodies: Vec<Vec2> = self
            .bims
            .iter()
            .map(|b| b.character.pos)
            .chain(self.droids.iter().filter(|d| !d.destroyed).map(|d| d.pos))
            .chain(self.visitors.iter().copied())
            .collect();
        self.room.shut_leaves(&bodies)
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

    /// Whether a room point is in a door the crew did not see the last
    /// time the room was drawn — a door drawn as it was last seen
    /// (`Door::set_seen`), whose sliding the host does not play either.
    pub fn door_unseen_at(&self, x: f32, y: f32) -> bool {
        self.room
            .door_at(vec2(x, y))
            .is_some_and(|i| self.room.doors[i].unseen())
    }

    /// What the fog over a room point is, as of the last trace: 0
    /// nothing, 1 the fog, whoever's the tile is — see `Sight::veil_at`.
    /// For the probes.
    pub fn veil_at(&self, x: f32, y: f32) -> u32 {
        self.room.sight.veil_at(vec2(x, y))
    }

    /// Which of the ship's doors a point is in, if one.
    pub fn door_at(&self, x: f32, y: f32) -> Option<usize> {
        self.room.door_at(vec2(x, y))
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

    /// Whether `who` may lock a door from its panel (task 138): its
    /// skill's `locks_doors`, which no class sets yet. Unlocking is
    /// anybody's.
    pub fn may_lock_doors(&self, who: usize) -> bool {
        self.skill(who).locks_doors
    }

    /// The Bim walks to the door's panel and works it.
    pub fn order_door(&mut self, who: usize, i: usize, order: door::Order) {
        if i < self.room.doors.len() {
            self.send_to_switch(who, Switch::Door(i, order));
        }
    }

    /// True while a task has the Bim, so the host can grey out "Make food".
    pub fn is_busy(&self, who: usize) -> bool {
        self.bims[who].task.is_some()
    }

    /// Send the Bim to work a switch. Nothing aboard changes without it: the
    /// state only moves when the Bim's hand gets there, and this displaces
    /// whatever it was doing exactly like any other errand.
    fn send_to_switch(&mut self, who: usize, which: Switch) {
        // Locking a door is no longer anybody's (task 138): only a body
        // whose skill says so may, and none does yet.
        if matches!(which, Switch::Door(_, door::Order::Lock)) && !self.may_lock_doors(who) {
            return;
        }
        if !self.can_begin(who, Kind::Switch(which)) || !self.take_over(who, Kind::Switch(which)) {
            return;
        }
        self.bims[who].task = Some(Task::work_switch(
            who,
            which,
            &mut self.bims[who].character,
            &mut self.room,
            &self.maps,
        ));
    }

    pub fn activity(&self, who: usize) -> u32 {
        match &self.bims[who].task {
            None => 0,
            Some(task) => job_code(task.kind()),
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
        // A door nobody sees is drawn as it was last seen: the fog hides
        // who walks through it, so it must not slide for them either.
        match self.fog {
            Fog::Crew => self.room.settle_seen_doors(false),
            Fog::All => self.room.settle_seen_doors(true),
            Fog::None => {}
        }
        self.room.draw(&mut self.list);
        // On the deck, under everything: the bodies walk over the blood.
        self.room.blood.draw(&mut self.list);
        // And the scorches the bolts left on the walls and the deck
        // (feature 98), with the blood.
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

        self.bodies_from = self.list.len();

        // Every body stood where it is drawn between the last step and the
        // next (`set_blend`) for the drawing alone, and put back before
        // anything reads a position for a rule (the sight, below).
        let stood = self.stand_where_shown();

        // Bodies in crew order, so who is drawn on top of whom does not
        // change as they walk past each other — and only the ones in view:
        // a station's people behind a bulkhead are not drawn at all.
        let fx = &self.combat.fx;
        for (who, bim) in self.bims.iter().enumerate() {
            if self.body_seen(who) {
                let from = self.list.len();
                bim.character.draw(&mut self.list, self.viewer);
                // Under a medic's cloak (task 130): the whole body faint,
                // and a shimmer over it.
                if bim.character.is_cloaked() {
                    self.list.fade_from(from, crate::character::CLOAK_OPACITY);
                    bim.character.draw_cloak(&mut self.list);
                }
                // A shot that landed: a flash on the body, on the
                // host's clock (feature 98) — or, for a host that ages no
                // effects, the room's own flash over the whole body, gone
                // in a blink of the simulation's.
                if fx.is_on() {
                    for (_, t) in fx.struck_on(who) {
                        let (at, radius) = bim.character.hit_mark();
                        crate::fx::draw_struck(&mut self.list, at, radius, t);
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
        self.put_back(stood);

        // What nobody sees, fogged. Over the deck and the fixtures and
        // under the night, the rings and the marquee: a fog that hid the
        // pointer's own marks would be a fog over the pointer.
        {
            let _timed = crate::timing::scope(crate::timing::Part::Observe);
            self.observe();
        }
        match self.fog {
            // Through the crew's eyes the fog is the light map's — smooth,
            // the one fog over whatever nobody sees, whoever's it is,
            // drawn by the host over this picture — marched again only
            // for a body that moved.
            Fog::Crew => {
                let _timed = crate::timing::scope(crate::timing::Part::LightMap);
                let eyes: Vec<Vec2> = self.bims.iter().map(|b| b.character.pos).collect();
                self.room.sight.light_map(&eyes);
            }
            Fog::All => self.room.sight.draw(&mut self.list, true),
            Fog::None => {}
        }
        // The shots, over the fog: a bolt is always seen, whatever it
        // flies through.
        self.fog_from = self.list.len();
        let flying = self.bolts_where_shown();
        self.draw_bolt_glow();
        self.combat.draw(&mut self.list);
        self.bolts_put_back(flying);
        // The pings where an order landed, over the fog as well, since an
        // order given into the dark is an order all the same.
        self.draw_pings();

        // Night falls over the whole room at once.
        // Aboard a ship the room has no shell, and a night wash the size of
        // the build area would be a dark square hanging in space round the
        // hull: the ship painter owns the sky, so the wash stays a bare
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
        // Every fixture of the kind, since a row names the kind: the craft
        // row rings every bench.
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
    /// fixtures, the blood, the trails and the pings — and the bodies with
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
    use crate::combat::{Tier, WeaponKind};
    use crate::room::{ROOM_H, ROOM_W};

    /// A frame at 1x.
    const DT: f32 = 1.0 / 60.0;

    /// The test room: a bare deck `ROOM_W` by `ROOM_H`, two Bims on it.
    fn room() -> Game {
        Game::bare(3, ROOM_W, ROOM_H)
    }

    /// The test room with a closet shut off in its far corner — two
    /// bulkheads standing in from the walls, and no way in — for a test
    /// that wants somewhere out of every eye; and the middle of the closet.
    fn room_with_a_closet() -> (Game, Vec2) {
        const BULKHEAD: f32 = 16.0;
        let mut game = room();
        let interior = game.room.interior;
        let shell = Rect::from_corners(
            vec2(interior.max.x - 214.0, interior.max.y - 194.0),
            interior.max,
        );
        let walls = [
            Rect::from_corners(shell.min, vec2(shell.max.x, shell.min.y + BULKHEAD)),
            Rect::from_corners(
                vec2(shell.min.x, shell.min.y + BULKHEAD),
                vec2(shell.min.x + BULKHEAD, shell.max.y),
            ),
        ];
        game.room.others.extend(walls);
        game.room.sight = Sight::new(game.room.bounds, interior, TILE, &walls, &[]);
        game.refresh_maps();
        game.refresh_blockers();
        let inside = Rect::from_corners(shell.min + vec2(BULKHEAD, BULKHEAD), shell.max).center();
        (game, inside)
    }

    /// The bunks go with the crew from one room to the next: `take_crew`
    /// carries whose bunk was whose and `adopt` reads it back, keeps a
    /// bunk nobody in the new room has, and gives a Bim arriving with none
    /// the first one spare — or none, past the bunks. A Bim that dies gives
    /// its bunk up.
    #[test]
    fn a_bunk_goes_with_its_bim_from_one_room_to_the_next_and_a_spare_one_is_given() {
        let design = shipdesign::fixture::combat_ship();
        let bunks = crate::aboard::game_aboard(&design, 0, 3).bed_count();
        assert!(bunks >= 2, "the combat ship sleeps five");
        // Dealt in crew order, as far as the bunks go.
        let mut game = crate::aboard::game_aboard(&design, bunks, 3);
        for who in 0..bunks {
            assert_eq!(game.bed_of(who), Some(who));
        }
        // The last dies: its bunk is nobody's.
        let last = bunks - 1;
        game.kill_for_probe(last);
        game.simulate(DT);
        assert!(!game.is_alive(last));
        assert_eq!(game.bed_of(last), None);
        assert_eq!(game.bed_owner(last), None);
        let crew = game.take_crew();
        assert_eq!(crew[0].bed, Some(0));
        assert_eq!(crew[last].bed, None);
        // Into a room of the same ship with nobody in it: the living keep
        // theirs, and the dead one, arriving with none, is given the spare.
        let mut next = crate::aboard::game_aboard(&design, 0, 3);
        assert!(next.room.bunk_of.is_empty());
        next.adopt(crew, Vec2::ZERO);
        for who in 0..last {
            assert_eq!(next.bed_of(who), Some(who), "kept");
        }
        assert_eq!(
            next.bed_of(last),
            Some(last),
            "the spare one, having arrived with none"
        );
        // One more, past the bunks, has none.
        let extra = crate::aboard::game_aboard(&design, 1, 4).take_crew();
        next.adopt(extra, Vec2::ZERO);
        assert_eq!(next.crew_count() as usize, bunks + 1);
        assert_eq!(next.bed_of(bunks), None);
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

    /// Under a joined deck an enemy nobody sees is not drawn —
    /// until it shoots, and then for `SEEN_FOR` after its last shot. A
    /// friendly room's people are drawn wherever they stand.
    #[test]
    fn a_shooter_gives_itself_away_and_a_friend_is_always_drawn() {
        let mut game = room();
        game.set_autonomous(false);
        let kate = game.put_for_probe(1, vec2(ROOM_W * 0.35, ROOM_H * 0.5));
        game.put_for_probe(0, vec2(ROOM_W * 0.45, ROOM_H * 0.5));
        game.set_fog(Fog::None);
        game.set_hostile_bodies(true);
        game.simulate(DT);
        assert!(!game.body_seen(0) && !game.body_seen(1), "out of sight");

        let target = kate + vec2(4.0 * TILE, 0.0);
        game.set_hostiles(vec![Some((target, WeaponKind::LaserPistol.basic()))]);
        for _ in 0..120 {
            game.simulate(DT);
        }
        assert!(!game.take_shots().is_empty(), "somebody shot");
        assert!(
            game.body_seen(0) || game.body_seen(1),
            "a shooter is drawn though nobody sees it"
        );

        // The fight over: gone from the picture once the last shot is
        // `SEEN_FOR` old.
        game.set_hostiles(Vec::new());
        for _ in 0..((SEEN_FOR / DT).ceil() as usize + 2) {
            game.simulate(DT);
        }
        assert!(!game.body_seen(0) && !game.body_seen(1), "back in the dark");

        // The same people as friends: always drawn.
        game.set_hostile_bodies(false);
        assert!(game.body_seen(0) && game.body_seen(1));
        game.set_fog(Fog::All);
        assert!(!game.body_seen(0), "nobody is looking into the room");
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

    /// A dark box with one Trooper in it, the room hostile: for the
    /// machines' hunt, where twelve tiles off in the dark is nobody.
    fn dark_box_with_a_trooper(at: Vec2) -> Game {
        use crate::droid::{Droid, DroidKind};
        let layout = crate::aboard::layout_of(&box_ship_of(28, &[]));
        let (w, h) = (layout.bounds.width(), layout.bounds.height());
        let mut game = Game::with_layout(layout, 7, &[], w, h);
        game.set_autonomous(false);
        game.set_hostile_bodies(true);
        game.add_droid(Droid::new(DroidKind::Trooper, Tier::One, 0, 1, at, 0.0, 5));
        game
    }

    /// A machine shot at from the dark goes for the shooter: struck, it
    /// believes the target with a clear line to it where it stands, walks
    /// at it, and shoots the moment it sees it. Never struck, it stands
    /// where it was put with nobody in its sight (the user's report:
    /// Troopers stood in a corridor while the crew shot them from the
    /// dark).
    #[test]
    fn a_machine_shot_at_from_the_dark_goes_for_the_shooter() {
        let start = tile_middle(3.0, 3.0);
        let shooter = tile_middle(16.0, 16.0);
        let pistol = WeaponKind::LaserPistol.basic();
        let mut game = dark_box_with_a_trooper(start);
        assert!(game.room.sight.sees_from(start, shooter).is_none());
        for _ in 0..(60 * 4) {
            game.set_hostiles(vec![Some((shooter, pistol))]);
            game.simulate(DT);
        }
        assert_eq!(game.believed_for_probe(), vec![None], "nobody seen");
        assert!((game.droids()[0].pos - start).len() < 0.1, "it stood");
        assert!(game.take_shots().is_empty());

        // Struck: it knows where from, and goes.
        game.strike_droid(0, crate::droid::DroidPart::Chassis, 1.0);
        let mut shots = 0;
        for _ in 0..(60 * 20) {
            game.set_hostiles(vec![Some((shooter, pistol))]);
            game.simulate(DT);
            shots += game.take_shots().len();
            if shots > 0 {
                break;
            }
        }
        let pos = game.droids()[0].pos;
        assert!(
            (pos - shooter).len() < (start - shooter).len() - 3.0 * TILE,
            "walked at the shooter: {pos:?}"
        );
        assert!(shots > 0, "and shot at it once it saw it");
    }

    /// A wave attacking a site the crew defend (the machines' own list,
    /// `set_machine_hostiles`) does not wait at its gate with nobody in
    /// sight: told where the crew and the site's people under arms are,
    /// it walks in after them and shoots the moment it sees one. A
    /// townsperson sheltering in a house is found only by looking (the
    /// user's report: on a defend-station mission the machines stood
    /// outside their airlock until the defenders came to them).
    #[test]
    fn a_wave_at_a_defended_site_walks_in_after_the_defenders() {
        use crate::droid::{Droid, DroidKind};
        let start = tile_middle(3.0, 3.0);
        let far = tile_middle(16.0, 16.0);
        let pistol = WeaponKind::LaserPistol.basic();
        // `cross` 1: the one target is the crew's, across the seam; 0: it
        // is this room's townsperson, sheltering or under arms.
        let attack = |cross: usize, sheltering: bool| {
            let layout = crate::aboard::layout_of(&box_ship(&[]));
            let (w, h) = (layout.bounds.width(), layout.bounds.height());
            let mut game = Game::with_layout(layout, 7, &[far], w, h);
            game.set_autonomous(false);
            game.add_droid(Droid::new(
                DroidKind::Trooper,
                Tier::One,
                0,
                1,
                start,
                0.0,
                5,
            ));
            game.set_sheltering(&[sheltering]);
            assert!(game.room.sight.sees_from(start, far).is_none());
            let mut shots = 0;
            for _ in 0..(60 * 20) {
                let list = if cross == 1 {
                    vec![Some((far, pistol)), None]
                } else {
                    vec![Some((game.bim_pos(0), pistol))]
                };
                game.set_machine_hostiles(list, cross);
                game.simulate(DT);
                shots += game.take_shots().len();
                if shots > 0 {
                    break;
                }
            }
            (game.droids()[0].pos, shots)
        };
        let (pos, shots) = attack(1, false);
        assert!(
            (pos - far).len() < (start - far).len() - 3.0 * TILE,
            "walked in after the crew: {pos:?}"
        );
        assert!(shots > 0, "and shot once it saw them");
        let (pos, _) = attack(0, false);
        assert!(
            (pos - far).len() < (start - far).len() - 3.0 * TILE,
            "and after a defender: {pos:?}"
        );
        let (pos, _) = attack(0, true);
        assert!(
            (pos - start).len() < 0.1,
            "a sheltering one is not told: {pos:?}"
        );
    }

    /// A Trooper with a shot holds or closes and shoots: it does not walk
    /// off to the far end of its reach, however the weapons fit (the
    /// user's report: Troopers ran away rather than fire). And it takes
    /// no stand it could not shoot from — twelve tiles off in the dark
    /// is nobody to it.
    #[test]
    fn a_trooper_with_a_shot_holds_or_closes_and_shoots() {
        let start = tile_middle(9.0, 9.0);
        let target = tile_middle(5.0, 9.0);
        for weapon in [
            WeaponKind::Shotgun,
            WeaponKind::LaserPistol,
            WeaponKind::AutoRifle,
        ] {
            let held = weapon.basic();
            let mut game = dark_box_with_a_trooper(start);
            assert!(game.room.sight.sees_from(start, target).is_some());
            let away = (start - target).len();
            let mut farthest = away;
            let mut shots = 0;
            for _ in 0..(60 * 10) {
                game.set_hostiles(vec![Some((target, held))]);
                game.simulate(DT);
                shots += game.take_shots().len();
                farthest = farthest.max((game.droids()[0].pos - target).len());
            }
            assert!(
                farthest <= away + 0.25 * TILE,
                "{weapon:?}: gave ground to {} tiles from {}",
                farthest / TILE,
                away / TILE
            );
            assert!(shots > 0, "{weapon:?}: it shot");
        }
    }

    /// A machine that lost sight of its quarry and searched the spot it
    /// was last seen at follows the trail on to where it went, while the
    /// sighting is fresh; a trail gone cold is not followed.
    #[test]
    fn a_hunting_machine_follows_the_trail_from_the_spot_it_searched() {
        let start = tile_middle(3.0, 3.0);
        let first = tile_middle(10.0, 3.0);
        let gone = tile_middle(22.0, 22.0);
        let pistol = WeaponKind::LaserPistol.basic();
        let hunt = |cold: bool| {
            let mut game = dark_box_with_a_trooper(start);
            game.set_hostiles(vec![Some((first, pistol))]);
            game.simulate(DT);
            assert_eq!(game.believed_for_probe(), vec![Some(first)], "seen");
            if cold {
                // The belief aged past the trail before anybody got there.
                for seen in game.last_seen.iter_mut().flatten() {
                    seen.ago = TRAIL_FOR;
                }
            }
            game.take_shots();
            let mut shots = 0;
            for _ in 0..(60 * 25) {
                game.set_hostiles(vec![Some((gone, pistol))]);
                game.simulate(DT);
                shots += game.take_shots().len();
                if shots > 0 {
                    break;
                }
            }
            (game.droids()[0].pos, shots)
        };
        let (pos, shots) = hunt(false);
        assert!(shots > 0, "found it and shot: {pos:?}");
        let (pos, shots) = hunt(true);
        assert_eq!(shots, 0, "cold: {pos:?}");
        assert!((pos - first).len() < 3.0 * TILE, "it stands at the spot");
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
    /// takes it off the armour with the armour's protection ignored and
    /// leaves the body alone; a bare body takes the damage.
    #[test]
    fn a_strip_unmakes_the_armour_and_leaves_the_body_alone() {
        let mut game = Game::bare(3, ROOM_W, ROOM_H);
        let kevlar = Piece::new(1, ArmourKind::Armour, Tier::One);
        let whole = kevlar.health;
        let gear = game.gear(0);
        game.issue(
            0,
            Gear {
                armour: Some(kevlar),
                ..gear
            },
        );
        let body = game.health(0);
        // A strip bigger than the piece's protection and smaller than
        // its health: the piece loses exactly the strip.
        game.strike_stripping(0, 9.0, false, 8.0);
        assert_eq!(game.gear(0).armour.unwrap().health, whole - 8.0);
        assert_eq!(game.health(0), body, "the part is whole");

        // What the piece cannot take is **lost**, not passed on: a strip
        // far bigger than what is left breaks the piece and no more.
        game.strike_stripping(0, 9.0, false, 1e6);
        assert!(game.gear(0).armour.unwrap().broken());
        assert_eq!(game.health(0), body, "still whole");

        // With the piece broken it shields nothing: the part takes the
        // plain damage the ordinary way.
        game.strike_stripping(0, 9.0, false, 1e6);
        assert_eq!(game.health(0), body - 9.0);

        // And a bare part takes it from the first.
        let mut bare = Game::bare(3, ROOM_W, ROOM_H);
        let legs = bare.health(0);
        bare.strike_stripping(0, 4.0, false, 30.0);
        assert_eq!(bare.health(0), legs - 4.0);
    }

    /// A bare hull twenty tiles across, decked, with these lights on it.
    fn box_ship(lights: &[(shipdesign::PartKind, (u32, u32))]) -> shipdesign::ShipDesign {
        box_ship_of(20, lights)
    }

    /// The same, `side` tiles across: its hull on the second row and
    /// column from each edge (task 152 wanted more than fifteen tiles of
    /// dark deck in a line).
    fn box_ship_of(
        side: u32,
        lights: &[(shipdesign::PartKind, (u32, u32))],
    ) -> shipdesign::ShipDesign {
        use shipdesign::parts::{PartKind, Rotation};
        use shipdesign::{Budget, Edit, ShipDesign, apply};
        let budget = Budget::new(10_000_000);
        let mut design = ShipDesign::new(side);
        let put = |design: &mut ShipDesign, kind: PartKind, origin: (u32, u32)| {
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
        for y in 1..side - 1 {
            for x in 1..side - 1 {
                put(&mut design, PartKind::Structure, (x, y));
            }
        }
        for y in 2..side - 2 {
            for x in 2..side - 2 {
                put(&mut design, PartKind::Floor, (x, y));
            }
        }
        for i in 1..side - 1 {
            put(&mut design, PartKind::OutsideWall, (i, 1));
            put(&mut design, PartKind::OutsideWall, (i, side - 2));
            put(&mut design, PartKind::OutsideWall, (1, i));
            put(&mut design, PartKind::OutsideWall, (side - 2, i));
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

    /// In the dark a Bim sees eight tiles (task 152; ten, then fifteen): a
    /// tile no light reaches is seen only from that close, however clear
    /// the line. A light on a tile is seen from across the deck, and so is
    /// everything its reach makes out — and a wall stops the light like it
    /// stops the eye, so a room behind a bulkhead is dark for all the lamp
    /// on the far side. A tile in a lamp's soft rim counts partly lit: not
    /// lit, but seen from further than a dark one, as far as its light
    /// stretches the eight.
    #[test]
    fn the_dark_is_seen_eight_tiles_and_a_lit_tile_further() {
        use shipdesign::PartKind;
        let eye = tile_middle(3.0, 10.0);
        let mid = tile_middle(10.0, 10.0);
        // Nine tiles short of the rim tile, for the rim's own reach.
        let nearer = tile_middle(10.0, 10.0);
        let rim = tile_middle(19.0, 10.0);
        let far = tile_middle(20.0, 10.0);
        let lamp = [(PartKind::WallLight, (25, 10))];

        // No lights at all: dark everywhere, and seven tiles is seen where
        // sixteen and seventeen are not.
        let layout = crate::aboard::layout_of(&box_ship_of(28, &[]));
        let (w, h) = (layout.bounds.width(), layout.bounds.height());
        let mut dark = Game::with_layout(layout, 7, &[eye], w, h);
        dark.set_autonomous(false);
        dark.simulate(DT);
        dark.observe();
        assert!(!dark.room.sight.lit_at(mid));
        assert!(dark.seen_at(mid.x, mid.y), "seven tiles, in the dark");
        assert!(!dark.seen_at(rim.x, rim.y), "sixteen tiles, in the dark");
        assert!(!dark.seen_at(far.x, far.y), "seventeen tiles, in the dark");
        assert!(dark.room.sight.sees_from(eye, mid).is_some());
        assert!(dark.room.sight.sees_from(eye, far).is_none());
        // A sentry's sensor is not stopped by the dark: seventeen tiles is
        // seen with a clear line.
        assert!(dark.room.sight.sees_from_in_the_dark(eye, far).is_some());

        // A wall light on the far hull: the far tile is lit and seen, and
        // the tile in its rim is partly lit — under half, so not lit, but
        // seen from nine tiles where a dark one is not.
        let layout = crate::aboard::layout_of(&box_ship_of(28, &lamp));
        let mut lit = Game::with_layout(layout, 7, &[eye], w, h);
        lit.set_autonomous(false);
        lit.simulate(DT);
        lit.observe();
        assert!(lit.room.sight.lit_at(far));
        assert!(lit.seen_at(far.x, far.y), "seventeen tiles, lit");
        assert!(lit.room.sight.sees_from(eye, far).is_some());
        assert_eq!(lit.room.sight.lights().len(), 1);
        let share = lit.light_at(rim.x, rim.y);
        assert!(share > 0.1 && share < 0.5, "the rim is partly lit: {share}");
        assert!(!lit.room.sight.lit_at(rim));
        assert!(!lit.seen_at(rim.x, rim.y), "sixteen tiles, partly lit");
        assert!(
            dark.room.sight.sees_from(nearer, rim).is_none(),
            "nine, dark"
        );
        assert!(
            lit.room.sight.sees_from(nearer, rim).is_some(),
            "nine tiles, partly lit"
        );
        // And the light falls off smoothly: no step from full to none.
        let at = |x: f32| lit.light_at(tile_middle(x, 10.0).x, tile_middle(x, 10.0).y);
        assert_eq!(at(22.0), 1.0, "the pool's heart is full");
        assert!(at(20.0) > at(19.0) && at(19.0) > at(18.0), "a soft rim");
        assert_eq!(at(17.0), 0.0, "past the reach, dark");

        // A wall between the lamp and the eye keeps the light behind it:
        // the tile on the eye's side of the wall is dark again, and not
        // seen from seventeen tiles; one within the eight is.
        let mut walled = box_ship_of(28, &lamp);
        {
            use shipdesign::{Budget, Edit, Rotation, apply};
            let budget = Budget::new(10_000_000);
            for y in 2..26 {
                if let Ok(next) = apply(
                    &walled,
                    &budget,
                    Edit::Place {
                        kind: PartKind::Wall,
                        origin: (21, y),
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
        assert!(!walled.room.sight.lit_at(far), "the wall shades it");
        assert!(!walled.seen_at(far.x, far.y), "seventeen tiles, dark");
        let near = tile_middle(10.0, 10.0);
        assert!(
            walled.seen_at(near.x, near.y),
            "seven tiles, dark, and still within the eight"
        );
    }

    /// A bolt in flight lights the tile it is in (task 152): a body there
    /// is seen from as far as a lit one, the tiles beside it half lit, and
    /// the light goes with the bolt.
    #[test]
    fn a_bolt_in_flight_lights_its_tile() {
        let eye = tile_middle(3.0, 10.0);
        let far = tile_middle(22.0, 10.0);
        let layout = crate::aboard::layout_of(&box_ship_of(28, &[]));
        let (w, h) = (layout.bounds.width(), layout.bounds.height());
        let mut game = Game::with_layout(layout, 7, &[eye], w, h);
        game.set_autonomous(false);
        game.simulate(DT);
        assert!(game.room.sight.sees_from(eye, far).is_none(), "dark");
        game.room.sight.set_flares(&[far]);
        assert!(game.room.sight.lit_at(far));
        assert!(game.room.sight.sees_from(eye, far).is_some(), "lit by it");
        let beside = far + vec2(TILE, 0.0);
        assert!((game.light_at(beside.x, beside.y) - 0.5).abs() < 0.01);
        // The lamps' light alone, which the glow reads, is still dark.
        assert_eq!(game.room.sight.lamplight_at(far), 0.0);
        game.observe();
        assert!(game.seen_at(far.x, far.y), "the trace sees it too");
        game.room.sight.set_flares(&[]);
        assert!(
            game.room.sight.sees_from(eye, far).is_none(),
            "gone with it"
        );
        game.observe();
        assert!(!game.seen_at(far.x, far.y));
    }

    /// A lamp is shot out: a bolt that passes within its radius stops
    /// there and takes its damage off it, a hit sets it flickering, one
    /// left at a fifth of its health or under is failing — flickering now
    /// and then on its own — and the third pistol bolt puts it out: the tile it lit is dark and seen
    /// no further than the eight again, the picture round it darker, and
    /// the world's word (`set_lamp_health`) puts it back or out on a
    /// fresh room.
    #[test]
    fn a_lamp_shot_out_goes_dark_and_flickers_on_the_way() {
        use crate::sight::LAMP_HEALTH;
        use shipdesign::PartKind;
        let eye = tile_middle(3.0, 10.0);
        let far = tile_middle(20.0, 10.0);
        let layout = crate::aboard::layout_of(&box_ship_of(28, &[(PartKind::WallLight, (25, 10))]));
        let (w, h) = (layout.bounds.width(), layout.bounds.height());
        let mut game = Game::with_layout(layout, 7, &[eye], w, h);
        game.set_autonomous(false);
        game.simulate(DT);
        game.render();
        assert!(game.room.sight.lit_at(far));
        assert!(game.seen_at(far.x, far.y), "seventeen tiles, lit");
        let lamp = game.lamps()[0];
        assert_eq!(lamp.health, LAMP_HEALTH);
        assert_eq!(lamp.level, 1.0);
        assert_eq!(
            game.lamp_at(tile_middle(25.0, 10.0)).map(|(i, _)| i),
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
        // flicker, and a fifth or less left is failing.
        let damage = WeaponKind::LaserPistol.stats().damage;
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
                let left = (LAMP_HEALTH - hits as f32 * damage).max(0.0);
                assert!((lamp.health - left).abs() < 1e-4, "after {hits}");
                assert_eq!(
                    lamp.is_failing(),
                    left > 0.0 && left <= LAMP_HEALTH * crate::sight::LAMP_FAILING,
                    "after {hits}"
                );
            }
        }
        assert!(game.lamps()[0].is_out(), "{shots} shots");
        // Two since the pistol went to 8 a shot with its magazine
        // (October 2026): three at 6.
        assert_eq!(hits, 2, "the second pistol bolt puts it out");
        assert!(flickered, "a hit sets it flickering");
        assert_eq!(game.take_lamp_changes(), vec![0, 0]);
        assert!(game.take_lamp_changes().is_empty(), "drained");
        assert_eq!(game.lamps()[0].level, 0.0);
        // Out: the tile it lit is dark and seen no further than the eight,
        // and the picture says so.
        game.render();
        assert!(!game.room.sight.lit_at(far));
        assert!(!game.seen_at(far.x, far.y), "seventeen tiles, in the dark");
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

    /// A body downed keeps its gun (task 113: a loadout is never
    /// dropped), holstered, is nobody's target — a bolt flies over it —
    /// and comes round with the gun still in its hand.
    #[test]
    fn a_bim_knocked_out_keeps_its_gun_and_is_no_target() {
        let mut game = room();
        game.set_autonomous(false);
        game.put_for_probe(1, vec2(ROOM_W * 0.35, ROOM_H * 0.5));
        game.put_for_probe(0, vec2(ROOM_W * 0.7, ROOM_H * 0.85));
        assert_eq!(game.weapon(1), Some(WeaponKind::LaserPistol.basic()));
        knock_out(&mut game, 1);
        assert!(game.is_downed(1));
        assert_eq!(
            game.weapon(1),
            Some(WeaponKind::LaserPistol.basic()),
            "the gun stays with her"
        );
        let at = game.bim_pos(1);
        assert_eq!(game.hit_at(at.x + 2.0, at.y), HIT_BIM, "the body");
        // Downed, nothing is aimed at her, and a bolt flies over her.
        game.enemy_fire(
            at + vec2(-3.0 * TILE, 0.0),
            at,
            WeaponKind::LaserPistol.basic(),
            false,
        );
        for _ in 0..(60 * 3) {
            game.simulate(DT);
        }
        assert!(
            game.take_wounds_taken().is_empty(),
            "a body downed is not shot"
        );
        // Revived, she comes round with it in her hand.
        assert!(game.bims[1].health.revive());
        game.simulate(DT);
        assert!(!game.is_downed(1));
        assert_eq!(game.weapon(1), Some(WeaponKind::LaserPistol.basic()));
    }

    /// A hostile room's people have eyes on the airlock whatever they are
    /// doing: a target within `AIRLOCK_WATCH` tiles of the watched spot is
    /// seen with nobody looking, and the hunt is on; one further off is
    /// not. Nobody shoots at it without real sight, as ever.
    #[test]
    fn a_hostile_room_s_airlock_is_watched_and_a_boarder_at_it_is_seen_by_nobody_in_particular() {
        let (mut game, closet) = room_with_a_closet();
        game.set_autonomous(false);
        game.put_for_probe(1, vec2(ROOM_W * 0.35, ROOM_H * 0.5));
        game.put_for_probe(0, vec2(ROOM_W * 0.45, ROOM_H * 0.8));
        game.set_hostile_bodies(true);
        let pistol = WeaponKind::LaserPistol.basic();
        // A target in the closet is nobody to them: out of every eye.
        let hidden = closet;
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
    fn a_hostile_room_chases_what_it_last_saw_and_forgets_it_after_a_minute() {
        // Kate armed in a hostile room, James out of it; a target in plain
        // view is known where it stands.
        let (mut game, closet) = room_with_a_closet();
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

        // The target steps behind the closet's walls: nobody sees it there,
        // so it is believed where it was — and the tactics go there, while
        // nothing is fired at the empty spot.
        let hidden = closet;
        game.set_hostiles(vec![Some((hidden, WeaponKind::LaserPistol.basic()))]);
        game.take_shots();
        let mut shots = 0;
        // A plan's worth and a little: the stand it took with the target
        // in sight holds until then. (Sixty steps passed only while the
        // idle wander, which went in September 2026, nudged it a fraction
        // of a pixel on its first step.)
        for _ in 0..((PLAN_EVERY / DT) as usize + 10) {
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
        // a target it can see, fires on the move — at full odds, since a
        // player's own Bim never misses (`Skill::sure`).
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

        // A bot's odds, off the combat stream: from two tiles the pistol
        // lands about 85 in 100 standing and about half that walking.
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

    /// Peeking round a corner, every shot starts where its line to the
    /// target is clear, the corner on the body's right as on its left.
    /// The gun is held over the right shoulder, so leaning out with the
    /// wall on that side put the muzzle beside the corner, and the bolts
    /// went into the wall (`Game::shot_from`).
    #[test]
    fn a_peek_shoots_clear_of_the_corner_on_either_side() {
        for wall_on_the_right in [true, false] {
            let mut game = room();
            game.set_autonomous(false);
            let origin = game.room.bounds.min;
            let tile = |x: i32, y: i32| {
                let min = origin + vec2(x as f32 * TILE, y as f32 * TILE);
                Rect::from_corners(min, min + vec2(TILE, TILE))
            };
            // A wall three tiles long east of the body's tile, running
            // away from the side it peeks out of: south of the line to
            // the target when the wall is on its right (facing east, in
            // a y-down room), north of it when on its left.
            let (rows, target_row) = if wall_on_the_right {
                ([5, 6, 7], 4)
            } else {
                ([3, 4, 5], 6)
            };
            let walls: Vec<Rect> = rows.iter().map(|&y| tile(6, y)).collect();
            let interior = game.room.interior;
            game.room.others.extend(walls.iter().copied());
            game.room.sight = Sight::new(game.room.bounds, interior, TILE, &walls, &[]);
            game.refresh_maps();
            game.refresh_blockers();
            game.put_for_probe(0, tile(5, 5).center());
            game.put_for_probe(1, tile(2, 9).center());
            game.issue(1, Gear::default());
            game.recruit_for_probe(0, true);
            let target = tile(12, target_row).center();
            game.set_hostiles(vec![Some((target, WeaponKind::LaserPistol.basic()))]);
            let mut shots = 0;
            for _ in 0..(60 * 6) {
                let before = game.bolts_in_flight();
                game.simulate(DT);
                if game.bolts_in_flight() > before {
                    let from = game.combat.bolts.last().expect("a bolt").fired_from;
                    assert!(game.peek(0).is_some(), "it shoots from the peek");
                    assert!(
                        line_of_fire(&game.room.sight, from, target),
                        "wall on the right {wall_on_the_right}: a shot from {from:?} \
                         has the corner in its way"
                    );
                    shots += 1;
                }
            }
            assert!(shots >= 3, "it fires: {shots}");
        }
    }

    #[test]
    fn the_auto_rifle_fires_four_a_second_with_no_burst_and_no_gap() {
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
        // Six seconds to walk to her stand, then every shot timed: four
        // in every second from the first, steadily — the eight-shot
        // bursts and their two seconds' recharge went in October 2026.
        for _ in 0..(60 * 6) {
            game.simulate(DT);
        }
        game.take_shots();
        // A fresh magazine for the timing (October 2026): thirty is more
        // than the six seconds fire, and the walk over may have spent some.
        game.bims[1].trigger.spent = 0;
        game.bims[1].trigger.reloading = 0.0;
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
        // A shot every quarter second, give or take the step it lands on.
        let shots = within(0.0, 6.0);
        assert!((22..=24).contains(&shots), "four a second: {times:?}");
        assert!(
            times.windows(2).all(|w| w[1] - w[0] < 0.3),
            "and never a gap: {times:?}"
        );
        assert!(game.take_hits().is_empty());
    }

    /// The minigun in a hand (October 2026, its magazine): the same
    /// trigger — a bolt every tenth of a second while it has a target,
    /// a hundred to the magazine, then nothing for its four-second
    /// reload. (Task 115's twenty to a pull and five seconds to the next
    /// is what this replaced.)
    #[test]
    fn a_minigun_in_a_hand_fires_its_hundred_then_reloads_four_seconds() {
        let mut game = room();
        game.set_autonomous(false);
        let kate = game.put_for_probe(1, vec2(ROOM_W * 0.35, ROOM_H * 0.5));
        game.put_for_probe(0, vec2(ROOM_W * 0.45, ROOM_H * 0.8));
        game.issue(
            1,
            Gear {
                weapon: Some(WeaponKind::Minigun.basic()),
                ..Gear::default()
            },
        );
        game.issue(0, Gear::default());
        game.set_hostile_bodies(true);
        let target = kate + vec2(4.0 * TILE, 0.0);
        game.set_hostiles(vec![Some((target, WeaponKind::LaserPistol.basic()))]);
        for _ in 0..(60 * 6) {
            game.simulate(DT);
        }
        game.take_shots();
        let mut times = Vec::new();
        for step in 0..(60 * 26) {
            game.simulate(DT);
            let t = step as f32 * DT;
            times.extend(game.take_shots().into_iter().map(|_| t));
        }
        // The shots in magazines, split where a second or more goes by
        // with none; the first is whatever was left of one when the
        // window opened, so the second is the first whole magazine.
        let mut runs: Vec<Vec<f32>> = Vec::new();
        for &t in &times {
            match runs.last_mut() {
                Some(r) if t - r[r.len() - 1] < 1.0 => r.push(t),
                _ => runs.push(vec![t]),
            }
        }
        assert!(runs.len() >= 3, "{times:?}");
        let (magazine, next) = (&runs[1], &runs[2]);
        assert_eq!(magazine.len(), 100, "a hundred to a magazine: {magazine:?}");
        // A tenth apart, to the step.
        for pair in magazine.windows(2) {
            let gap = pair[1] - pair[0];
            assert!(
                (0.1 - DT * 0.5..=0.1 + DT * 1.5).contains(&gap),
                "{magazine:?}"
            );
        }
        let reload = next[0] - magazine[99];
        assert!(
            (reload - 4.0).abs() < 0.1 + DT * 1.5,
            "four seconds to reload: {reload}"
        );
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
            // reach it lands — hit points off the bar, and a small splash of
            // blood round the body (task 120) — and from out of reach it
            // does nothing.
            let mut crew = room();
            crew.set_autonomous(false);
            let james = crew.put_for_probe(0, vec2(ROOM_W * 0.5, ROOM_H * 0.5));
            crew.put_for_probe(1, vec2(ROOM_W * 0.2, ROOM_H * 0.8));
            assert!(!crew.enemy_strike(james + vec2(4.0 * TILE, 0.0), 0, 35.0, true));
            assert_eq!(crew.health(0), crate::health::MAX_HEALTH);
            assert!(crew.enemy_strike(james + vec2(TILE, 0.0), 0, 12.0, true));
            assert_eq!(crew.health(0), crate::health::MAX_HEALTH - 12.0);
            let said = crew.take_wounds_taken();
            assert_eq!(said.len(), 1);
            assert!(said[0].cut && said[0].who == 0);
            let bloody = (-1..=1)
                .flat_map(|dx| (-1..=1).map(move |dy| (dx, dy)))
                .filter(|&(dx, dy)| {
                    let at = james + vec2(dx as f32 * TILE, dy as f32 * TILE);
                    crew.room.blood.at(at) < crate::blood::BASELINE
                })
                .count();
            assert!((1..=2).contains(&bloody), "{bloody} tiles splashed");
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

    /// A blade bot is issued the basic armour (task 113 took the pack it
    /// once put on at the alarm): a resident rolled the schword wears the
    /// basic set, a mercenary with one too.
    #[test]
    fn a_blade_bot_is_issued_the_basic_armour() {
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
        let james = game.put_for_probe(0, vec2(ROOM_W * 0.7, ROOM_H * 0.85));
        assert_eq!(game.weapon(1), Some(WeaponKind::LaserPistol.basic()));
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
        // The player's own Bim takes arms by itself too (September 2026)
        // — and nothing more: it stands where its player left it.
        assert!(game.is_recruited(0), "James takes arms with the alarm");
        assert_eq!(
            game.bim_pos(0),
            james,
            "and is moved by nobody but his player"
        );
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
        assert!(!game.is_recruited(0), "the alarm lets go of what it armed");
        // Hit with nobody in sight: the alarm, for a while.
        let here = game.bim_pos(1);
        assert!(game.enemy_strike(here + vec2(TILE, 0.0), 1, 5.0, false));
        game.simulate(DT);
        assert!(game.is_alarmed());
        assert!(game.bims[1].character.is_recruited());
        // Recruited by his player in the middle of it, James is the
        // player's again, and the alarm's end leaves him under arms.
        game.toggle_recruited(0);
        game.toggle_recruited(0);
        assert!(game.is_recruited(0));
        for _ in 0..((ALARM_HOLD / DT) as usize + 2) {
            game.simulate(DT);
        }
        assert!(!game.is_alarmed(), "the hold ran out");
        assert!(game.is_recruited(0), "what the player recruited stays so");
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

    /// A marquee selects everybody it touches in peace, and a click one;
    /// under arms a bot is not to be picked at all — the crew taking arms
    /// lets go of any pick of one, and neither a sweep nor a click takes
    /// one again. And a right-click is the player's own Bim's alone,
    /// whoever is selected: a line or a point moves James and never Kate.
    #[test]
    fn a_marquee_selects_everybody_in_peace_and_under_arms_the_player_s_own_alone() {
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

        // Kate alone selected, a line moves James — to where it began,
        // since he is one — and Kate nowhere.
        let a = vec2(ROOM_W * 0.35, ROOM_H * 0.8);
        let b = vec2(ROOM_W * 0.55, ROOM_H * 0.8);
        assert_eq!(game.order_line(0, a, b), ORDER_MOVING);
        assert!((game.destination_for_probe(0).unwrap() - a).len() < TILE);
        assert!(game.destination_for_probe(1).is_none());

        // Under the alarm the pick of Kate is let go, and a sweep over the
        // whole room picks James alone.
        let near = kate + vec2(4.0 * TILE, 0.0);
        game.set_hostiles(vec![Some((near, WeaponKind::LaserPistol.basic()))]);
        for _ in 0..5 {
            game.simulate(DT);
        }
        assert!(game.is_alarmed() && game.is_mustered());
        assert!(
            !game.is_selected(1, 0),
            "a bot's pick let go as they take arms"
        );
        game.drag_begin(0.0, 0.0);
        game.drag_end(0, ROOM_W, ROOM_H);
        assert_eq!(game.selected_all(0), vec![0]);
        let kate_now = game.bim_pos(1);
        game.drag_begin(kate_now.x, kate_now.y);
        game.drag_end(0, kate_now.x, kate_now.y);
        assert!(!game.is_selected(1, 0), "a click on a bot picks nobody");
        // A point moves James to it, and no huddle is made round it.
        let c = vec2(ROOM_W * 0.45, ROOM_H * 0.3);
        assert_eq!(game.order_move(0, c.x, c.y), ORDER_MOVING);
        assert!((game.destination_for_probe(0).unwrap() - c).len() < TILE);
        assert!(
            game.bims[1].character.post().is_none(),
            "Kate holds no spot"
        );
        // The drag's line is drawn for the one it would move.
        game.order_drag_begin(a.x, a.y);
        game.order_drag_update(b.x, b.y);
        assert!(game.order_drag.is_some());
        assert_eq!(game.order_drag_end(0, b.x, b.y, true), ORDER_MOVING);
        assert!(game.order_drag.is_none());
        assert!((game.destination_for_probe(0).unwrap() - a).len() < TILE);
    }

    /// Under the alarm a crew member that sees no enemy keeps to the
    /// player's side — walks into its slot beside James and stays there
    /// as he moves — and a right-click never reaches it: selected or not,
    /// the click is James's, and Kate keeps to him.
    #[test]
    fn under_the_alarm_the_crew_gather_round_the_player_and_a_click_moves_him_alone() {
        let mut game = room();
        game.set_autonomous(false);
        let james = game.put_for_probe(0, vec2(ROOM_W * 0.3, ROOM_H * 0.5));
        let kate = game.put_for_probe(1, vec2(ROOM_W * 0.85, ROOM_H * 0.85));
        // Kate selected in peace, the click is James's all the same.
        game.drag_begin(kate.x, kate.y);
        game.drag_end(0, kate.x, kate.y);
        assert_eq!(game.selected(0), Some(1));
        let there = james + vec2(0.0, 2.0 * TILE);
        assert_eq!(game.order_move(0, there.x, there.y), ORDER_MOVING);
        assert!((game.destination_for_probe(0).unwrap() - there).len() < TILE);
        assert!(game.destination_for_probe(1).is_none());
        for _ in 0..(60 * 5) {
            game.simulate(DT);
        }
        let james = game.bim_pos(0);
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
        assert!(
            (game.bim_pos(1) - game.bim_pos(0)).len() < 3.0 * TILE,
            "gathered beside James: {:?} of {:?}",
            game.bim_pos(1),
            game.bim_pos(0)
        );
        assert_eq!(game.bolts_in_flight(), 0, "nothing to shoot at");
        // James is sent across the room; Kate takes no post and follows.
        let spot = vec2(ROOM_W * 0.6, ROOM_H * 0.3);
        assert_eq!(game.order_move(0, spot.x, spot.y), ORDER_MOVING);
        assert!(game.bims[1].character.post().is_none());
        // He gets there — and then idles about as a Bim with nothing to
        // do does — and Kate's ring goes with him.
        let mut got_there = false;
        for _ in 0..(60 * 20) {
            game.simulate(DT);
            got_there |= (game.bim_pos(0) - spot).len() < TILE;
        }
        assert!(got_there, "James went");
        assert!(
            (game.bim_pos(1) - game.bim_pos(0)).len() < 3.0 * TILE,
            "and Kate followed"
        );
        // The alarm over, she is her own again.
        game.set_hostiles(Vec::new());
        game.simulate(DT);
        assert!(!game.is_alarmed());
        assert!(!game.bims[1].character.is_recruited());
    }

    /// A bot under the alarm leaves the ring round its player for a stand
    /// of its own the moment it is shot at, whether or not it sees who
    /// fired — and a downed player is nobody to gather round at all (the
    /// user's report: the crew stood round the body while the last
    /// machine shot at them). Nobody is revived here, so the ring is the
    /// only thing that could hold them by the body.
    #[test]
    fn a_bot_under_fire_or_with_its_player_down_fights_rather_than_gathers() {
        for case in ["quiet", "shot at", "player down"] {
            let (mut game, closet) = room_with_a_closet();
            game.set_autonomous(false);
            game.set_revivers(false);
            let james = game.put_for_probe(0, vec2(ROOM_W * 0.12, ROOM_H * 0.15));
            game.put_for_probe(1, james + vec2(1.5 * TILE, 0.0));
            // The machine in the closet: nobody sees it, and the alarm is up.
            let rifle = WeaponKind::AutoRifle.basic();
            game.set_hostiles(vec![Some((closet, rifle))]);
            for _ in 0..(60 * 2) {
                game.simulate(DT);
            }
            assert!(game.is_alarmed(), "{case}");
            assert!(
                !game.combat.sees_any(&game.room.sight, game.bim_pos(1)),
                "{case}: out of sight"
            );
            match case {
                "shot at" => {
                    let at = game.bim_pos(1);
                    assert!(game.enemy_strike(at, 1, 5.0, false));
                    assert!(game.bims[1].under_fire > 0.0);
                }
                "player down" => knock_out(&mut game, 0),
                _ => {}
            }
            let mut left_the_ring = false;
            for _ in 0..(60 * 4) {
                game.set_hostiles(vec![Some((closet, rifle))]);
                game.simulate(DT);
                let going = game.destination_for_probe(1).unwrap_or(game.bim_pos(1));
                left_the_ring |= (going - james).len() > 3.0 * TILE;
            }
            assert_eq!(left_the_ring, case != "quiet", "{case}");
        }
    }

    /// A commander's reinforcement called in while the crew are under
    /// arms is under arms with them — its rifle out, gathering round the
    /// player as the rest do — and in peace it is not, as they are not
    /// (the user's report: the ones called in stood where they came doing
    /// nothing).
    #[test]
    fn a_reinforcement_called_into_the_alarm_takes_arms_with_the_crew() {
        let rifle = Gear {
            weapon: Some(WeaponKind::AutoRifle.basic()),
            ..Gear::default()
        };
        let mut game = room();
        game.set_autonomous(false);
        let james = game.put_for_probe(0, vec2(ROOM_W * 0.3, ROOM_H * 0.5));
        let calm = game.enlist_reinforcement(vec2(ROOM_W * 0.5, ROOM_H * 0.2), rifle, 1);
        game.simulate(DT);
        assert!(!game.is_mustered());
        assert!(!game.bims[calm].character.is_recruited(), "peace");
        // The alarm up, an enemy out of sight beyond the walls.
        let unseen = james + vec2((ALARM_RANGE - 5.0) * TILE, 0.0);
        game.set_hostiles(vec![Some((unseen, WeaponKind::LaserPistol.basic()))]);
        game.simulate(DT);
        assert!(game.is_mustered());
        let far = vec2(ROOM_W * 0.85, ROOM_H * 0.85);
        let late = game.enlist_reinforcement(far, rifle, 2);
        assert!(game.bims[late].character.is_recruited(), "under arms");
        let mut gathered = false;
        for _ in 0..(60 * 20) {
            game.simulate(DT);
            if (game.bim_pos(late) - game.bim_pos(0)).len() < 3.0 * TILE {
                gathered = true;
                break;
            }
        }
        assert!(game.is_armed(late), "its rifle out");
        assert!(gathered, "beside James: {:?}", game.bim_pos(late));
    }

    /// A bot under fire starts no revive until the fire has stopped for
    /// [`UNDER_FIRE`] seconds; out of it, it does. An enemy is up beyond
    /// the walls, out of sight: with none the deck is clear, and a bot
    /// goes at once whatever hit it.
    #[test]
    fn a_bot_under_fire_takes_up_no_revive() {
        let mut game = room();
        let at = game.put_for_probe(1, vec2(ROOM_W * 0.5, ROOM_H * 0.5));
        let beyond = at + vec2(25.0 * TILE, 0.0);
        game.set_hostiles(vec![Some((beyond, WeaponKind::LaserPistol.basic()))]);
        assert!(!game.deck_clear());
        game.bims[1].character.set_recruited(true);
        assert!(game.ready_to_revive(1));
        assert!(game.enemy_strike(at, 1, 5.0, false));
        assert!(!game.ready_to_revive(1), "shot at");
        for _ in 0..((UNDER_FIRE / DT) as usize + 2) {
            game.simulate(DT);
        }
        game.bims[1].character.set_recruited(true);
        assert!(game.ready_to_revive(1), "the fire over");
    }

    /// An attack-move: the player's own Bim walks where it was sent with
    /// its weapon out, stands still to shoot the moment it has a target in
    /// its sights, walks on once nothing is left, and the order is over at
    /// the spot. A plain walk calls one off.
    #[test]
    fn an_attack_move_stops_to_shoot_and_walks_on_when_nothing_is_left() {
        let mut game = room();
        game.set_autonomous(false);
        let james = game.put_for_probe(0, vec2(ROOM_W * 0.2, ROOM_H * 0.5));
        game.put_for_probe(1, vec2(ROOM_W * 0.2, ROOM_H * 0.85));
        // Kate unarmed: under the alarm she would fire at the target too.
        game.issue(1, Gear::default());
        let spot = vec2(ROOM_W * 0.75, ROOM_H * 0.5);
        let attack_move = |at: Vec2| crate::order::CrewOrder::AttackMove { x: at.x, y: at.y };
        assert_eq!(game.order(0, attack_move(spot)), ORDER_MOVING);
        assert!(game.attack_move_of(0).is_some());
        assert!(game.is_recruited(0), "under arms for it");
        // Nothing to shoot yet: it walks.
        for _ in 0..30 {
            game.simulate(DT);
        }
        assert!(game.is_walking(0));
        let walked_to = game.bim_pos(0);
        assert!((walked_to - james).len() > 4.0);
        // A target comes into its sights: it stands and shoots.
        game.set_hostiles(vec![Some((
            vec2(ROOM_W * 0.5, ROOM_H * 0.2),
            WeaponKind::LaserPistol.basic(),
        ))]);
        let mut fired = 0;
        for _ in 0..(60 * 3) {
            let before = game.bolts_in_flight();
            game.simulate(DT);
            fired += game.bolts_in_flight().saturating_sub(before);
        }
        let stood = game.bim_pos(0);
        assert!(fired > 0, "it shoots");
        assert!(!game.is_walking(0), "standing for the shot");
        assert!((stood - walked_to).len() < TILE, "stopped where it saw it");
        assert!(game.attack_move_of(0).is_some(), "the order still stands");
        // Nothing left to shoot at: on to the spot, and the order is done.
        game.set_hostiles(Vec::new());
        for _ in 0..(60 * 15) {
            game.simulate(DT);
        }
        assert!((game.bim_pos(0) - spot).len() < 1.5 * TILE, "walked on");
        assert!(game.attack_move_of(0).is_none(), "and the order is over");
        // A plain walk calls one under way off.
        assert_eq!(game.order(0, attack_move(james)), ORDER_MOVING);
        assert!(game.attack_move_of(0).is_some());
        assert_eq!(game.order_move(0, spot.x, spot.y), ORDER_MOVING);
        assert!(game.attack_move_of(0).is_none());
    }

    /// A right-click on an enemy (task 126): the player's own Bim takes
    /// up arms and fires at that one and never at a nearer one, walks
    /// after it when it has no shot, and the order ends when the enemy
    /// is down — or at the next order. A click finds the enemy under the
    /// pointer where the crew see, and nobody in the fog or off to one
    /// side.
    #[test]
    fn an_attack_order_keeps_at_its_enemy_until_it_is_down() {
        let mut game = room();
        game.set_autonomous(false);
        let james = game.put_for_probe(0, vec2(ROOM_W * 0.2, ROOM_H * 0.5));
        game.put_for_probe(1, vec2(ROOM_W * 0.2, ROOM_H * 0.85));
        game.issue(1, Gear::default());
        let near = vec2(ROOM_W * 0.35, ROOM_H * 0.2);
        let far = vec2(ROOM_W * 0.8, ROOM_H * 0.5);
        let pistol = WeaponKind::LaserPistol.basic();
        game.set_hostiles(vec![Some((near, pistol)), Some((far, pistol))]);
        game.observe();
        assert_eq!(game.enemy_at(far.x + 10.0, far.y - 8.0), Some(1));
        assert_eq!(game.enemy_at(near.x, near.y), Some(0));
        assert_eq!(game.enemy_at(far.x - 2.0 * TILE, far.y), None);
        let attack = crate::order::CrewOrder::Attack { enemy: 1 };
        assert_eq!(game.order(0, attack), ORDER_MOVING);
        assert_eq!(game.focus_of(0), Some(1));
        assert!(game.is_recruited(0), "under arms for it");
        // Every hit is on the far one, never on the nearer.
        let mut hits = Vec::new();
        for _ in 0..(60 * 6) {
            game.simulate(DT);
            hits.extend(game.take_hits().into_iter().map(|h| h.who));
        }
        assert!(!hits.is_empty(), "it shoots");
        assert!(hits.iter().all(|&who| who == 1), "{hits:?}");
        assert!(
            (game.bim_pos(0) - james).len() < TILE,
            "a shot from where it stood"
        );
        // Down, and the order is over.
        game.set_hostiles(vec![Some((near, pistol)), None]);
        game.simulate(DT);
        assert_eq!(game.focus_of(0), None);
        // An order at nobody is no order.
        assert_eq!(game.order(0, attack), ORDER_IGNORED);

        // A blade has no shot: it walks after its enemy and swings at it.
        game.issue(
            0,
            Gear {
                weapon: Some(WeaponKind::Schword.basic()),
                ..Gear::default()
            },
        );
        // The pistol's last shot — at the near one, once the far was down,
        // and a player's own Bim never misses — lands before the blade's
        // count starts.
        game.set_hostiles(vec![None, None]);
        for _ in 0..60 {
            game.simulate(DT);
        }
        game.take_hits();
        game.set_hostiles(vec![Some((near, pistol)), Some((far, pistol))]);
        assert_eq!(game.order(0, attack), ORDER_MOVING);
        let mut struck = Vec::new();
        for _ in 0..(60 * 12) {
            game.simulate(DT);
            struck.extend(game.take_hits().into_iter().map(|h| h.who));
        }
        assert!(
            (game.bim_pos(0) - far).len() <= MELEE_RANGE * TILE + 1.0,
            "walked up to it"
        );
        assert!(struck.contains(&1), "and struck it: {struck:?}");
        assert!(!struck.contains(&0), "{struck:?}");
        // A walk calls it off.
        assert_eq!(game.order_move(0, james.x, james.y), ORDER_MOVING);
        assert_eq!(game.focus_of(0), None);
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
        // James takes arms with the alarm like everybody, so his hands are
        // empty: the hits counted here are Kate's blade alone.
        game.issue(0, Gear::default());
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
    fn the_armour_takes_a_hit_first_and_its_protection_lifts_when_it_breaks() {
        let near = |a: f32, b: f32| (a - b).abs() < 1e-3;
        let mut game = room();
        game.set_autonomous(false);
        game.put_for_probe(0, vec2(ROOM_W * 0.45, ROOM_H * 0.5));
        let vest = Piece::new(7, ArmourKind::Armour, Tier::One);
        let mut gear = game.gear(0);
        gear.armour = Some(vest);
        game.issue(0, gear);
        assert_eq!(game.worn(0), Some(vest));
        assert_eq!(game.armour_health(0), 45.0);
        assert_eq!(game.armour_health(1), 0.0);

        // The protection off, then the armour takes the rest: fifteen
        // leaves the Bim untouched and the armour at 31.8.
        let out = game.wound(0, 15.0);
        assert_eq!(out.absorbed, 15.0);
        assert_eq!(out.through, 0.0);
        assert!(!out.downed && !out.piece_broke);
        assert!(near(game.worn(0).unwrap().health, 31.8));
        assert_eq!(game.health(0), crate::health::MAX_HEALTH);

        // No more than the protection is no hit at all.
        let out = game.wound(0, 1.5);
        assert_eq!(out.absorbed, 1.5);
        assert!(near(game.worn(0).unwrap().health, 31.8));

        // Forty: 1.8 off, 31.8 into the armour, 6.4 through. The armour
        // is broken — still worn, doing nothing — and the world is told.
        let out = game.wound(0, 40.0);
        assert!(near(out.absorbed, 33.6));
        assert!(near(out.through, 6.4));
        assert!(out.piece_broke);
        let worn = game.worn(0).unwrap();
        assert!(worn.broken());
        assert_eq!(worn.id, 7, "the same piece");
        assert!(near(game.health(0), crate::health::MAX_HEALTH - 6.4));
        assert_eq!(game.armour_health(0), 0.0);
        assert_eq!(game.take_pieces_broken(), vec![(0, ArmourKind::Armour)]);
        assert!(game.take_pieces_broken().is_empty(), "drained");

        // Broken, it neither protects nor takes: the whole shot goes in.
        let out = game.wound(0, 12.0);
        assert_eq!(out.absorbed, 0.0);
        assert_eq!(out.through, 12.0);
        assert!(!out.piece_broke);
        assert!(near(game.health(0), crate::health::MAX_HEALTH - 18.4));
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
            // Downed is down too, but still the crewmate: the menu on it has
            // the revive.
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

    #[test]
    fn a_right_click_order_passes_over_a_body_lying_on_the_deck() {
        let mut game = room();
        game.set_autonomous(false);
        // One dead, one downed: an order lands on the deck under either.
        let dead = game.put_for_probe(1, vec2(ROOM_W * 0.5, ROOM_H * 0.5));
        game.kill_for_probe(1);
        game.simulate(DT);
        game.put_for_probe(0, vec2(ROOM_W * 0.5, ROOM_H * 0.2));
        knock_out(&mut game, 0);
        let downed = game.bims[0].character.pos;
        assert_eq!(game.hit_order_at(dead.x + 4.0, dead.y - 3.0), HIT_NONE);
        assert_eq!(game.hit_order_at(downed.x + 4.0, downed.y - 3.0), HIT_NONE);
        // A left click still finds them.
        assert_eq!(game.hit_at(dead.x + 4.0, dead.y - 3.0), HIT_BODY);
        assert_eq!(game.hit_at(downed.x + 4.0, downed.y - 3.0), HIT_BIM);
        // A station's person down is passed over; one for hire is not.
        let (lying, hailed) = (
            vec2(ROOM_W * 0.2, ROOM_H * 0.8),
            vec2(ROOM_W * 0.8, ROOM_H * 0.8),
        );
        game.set_visitors(vec![lying, hailed]);
        game.set_visitors_down(&[true, false]);
        game.set_visitors_hailable(&[false, true]);
        assert_eq!(game.hit_order_at(lying.x, lying.y), HIT_NONE);
        assert_eq!(game.hit_order_at(hailed.x, hailed.y), HIT_VISITOR);
    }

    #[test]
    fn a_crewmate_standing_over_a_body_is_the_crewmate() {
        let mut game = room();
        game.set_autonomous(false);
        // The body first in the list, the living one standing on it.
        let at = game.put_for_probe(0, vec2(ROOM_W * 0.5, ROOM_H * 0.5));
        game.kill_for_probe(0);
        game.simulate(DT);
        game.put_for_probe(1, at);
        assert_eq!(game.hit_at(at.x, at.y), HIT_BIM);
        assert_eq!(game.hit_bim(), 1);
        assert_eq!(game.hit_order_at(at.x, at.y), HIT_BIM);
        assert_eq!(game.hit_bim(), 1);
    }

    /// Hit a Bim down to nothing so it lies there, downed, from the next
    /// step.
    fn knock_out(game: &mut Game, who: usize) {
        let out = game.wound(who, 1_000.0);
        assert!(out.downed);
        game.simulate(DT);
        assert!(game.is_downed(who), "downed");
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
    /// there is a channel past one over the fog, and under it nothing but
    /// what glowed before the shot — the crew's own comms lamps (October
    /// 2026), one a body. (A blade in a hand and a machine's sparks glow
    /// too since feature 98, and neither is in this room.)
    #[test]
    fn the_pistol_bolt_s_core_is_the_one_thing_brighter_than_white() {
        let emissive = |part: &[f32]| {
            part.chunks_exact(crate::draw::STRIDE)
                .filter(|s| s[8] > 1.0 || s[9] > 1.0 || s[10] > 1.0)
                .count()
        };
        let mut game = room();
        game.set_autonomous(false);
        game.render();
        let (under, over) = game.shapes_fog_split();
        assert_eq!(
            emissive(over),
            0,
            "nothing glows over the fog before a shot"
        );
        let lamps = emissive(under);
        assert_eq!(
            lamps,
            game.crew_count() as usize,
            "a comms lamp on every body"
        );
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
        assert!(emissive(over) > 0, "the core glows");
        assert_eq!(
            emissive(under),
            lamps,
            "and nothing more under the fog does"
        );
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
        plain.strike(1, 1.0, false);
        plain.render();
        assert_eq!(plain.fx_count_for_probe(), (false, 0));
        assert!(flash_over_body(&plain), "the room's own flash");

        // A host ages it: the flash on the body, and not the room's.
        let mut lit = room();
        lit.set_autonomous(false);
        lit.fade(0.0);
        lit.strike(1, 1.0, false);
        lit.render();
        assert_eq!(lit.fx_count_for_probe(), (true, 1));
        assert!(!flash_over_body(&lit), "the body's mark flashes instead");
        let (mark, _) = lit.bims[1].character.hit_mark();
        assert!(
            lit.shapes()
                .chunks_exact(crate::draw::STRIDE)
                .any(|s| (vec2(s[1], s[2]) - mark).len() < 1e-3),
            "a flash where the body is marked"
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

    // --- task 120: one bar, downed, revived ----------------------------------

    /// A hit that takes the bar to nothing downs the Bim: out on the deck,
    /// the countdown running, and no longer on its feet.
    #[test]
    fn hit_points_at_nothing_down_a_bim_and_start_its_countdown() {
        let mut game = room();
        game.set_autonomous(false);
        let out = game.wound(0, 60.0);
        assert!(!out.downed);
        assert_eq!(game.health(0), crate::health::MAX_HEALTH - 60.0);
        let out = game.wound(0, 60.0);
        assert!(out.downed);
        assert_eq!(game.take_downs(), vec![0]);
        game.simulate(DT);
        assert!(game.is_downed(0));
        assert!(game.is_alive(0));
        let left = game.down_left(0).expect("a countdown");
        assert!(left < crate::health::DOWNED_SECONDS && left > 29.0);
    }

    /// The countdown is steps of the room: thirty seconds of them at 1× and
    /// the body is dead; nothing is taken off it while no step is taken.
    #[test]
    fn a_downed_bim_dies_after_its_countdown_of_steps() {
        let mut game = room();
        game.set_autonomous(false);
        game.set_revivers(false);
        knock_out(&mut game, 0);
        let before = game.down_left(0).unwrap();
        // Paused: no step, nothing moves.
        assert_eq!(game.down_left(0), Some(before));
        for _ in 0..(60 * 29) {
            game.simulate(DT);
        }
        assert!(game.is_alive(0), "still downed at 29 s");
        for _ in 0..(60 * 2) {
            game.simulate(DT);
        }
        assert!(!game.is_alive(0), "dead after the countdown");
    }

    /// **A Bim stunned by a Stun Shot stands where it was caught**
    /// (October 2026): no walk until the stun wears off, then on to its
    /// post again; a second stun takes the longer; a body down takes none.
    #[test]
    fn a_stunned_bim_stands_until_it_wears_off_and_a_body_down_takes_none() {
        let mut game = room();
        game.set_autonomous(false);
        game.set_revivers(false);
        game.put_for_probe(0, vec2(ROOM_W * 0.25, ROOM_H * 0.5));
        assert!(game.send_to(0, vec2(ROOM_W * 0.75, ROOM_H * 0.5)));
        for _ in 0..10 {
            game.simulate(DT);
        }
        assert!(game.stun_bim(0, 1.0));
        assert!(game.stun_bim(0, 0.5), "a shorter one keeps the longer");
        assert!((game.bim_stunned(0) - 1.0).abs() < 1e-6);
        let caught = game.bim_pos(0);
        for _ in 0..50 {
            game.simulate(DT);
            assert!((game.bim_pos(0) - caught).len() < 1.0, "stands still");
        }
        assert!(game.bim_stunned(0) > 0.0);
        for _ in 0..60 {
            game.simulate(DT);
        }
        assert_eq!(game.bim_stunned(0), 0.0, "worn off");
        assert!(
            (game.bim_pos(0) - caught).len() > 20.0,
            "walks on to its post"
        );
        knock_out(&mut game, 1);
        game.simulate(DT);
        assert!(!game.stun_bim(1, 1.0), "a body down is not stunned");
    }

    /// A revive takes the helper's `Skill::revive` seconds with its hands on
    /// the patient — ten for anybody, four for a medic's — and the patient
    /// stands at three tenths of its bar, slowed for the rest of the mission.
    #[test]
    fn a_revive_takes_the_helper_s_seconds_and_leaves_the_patient_at_three_tenths() {
        for (seconds, name) in [(crate::health::REVIVE_SECONDS, "anybody"), (4.0, "a medic")] {
            let mut game = room();
            game.set_autonomous(false);
            game.set_revivers(false);
            let mut skills = vec![crate::combat::Skill::NONE; 2];
            skills[1].revive = seconds;
            game.set_skills(skills);
            let at = game.put_for_probe(0, vec2(ROOM_W * 0.5, ROOM_H * 0.5));
            game.put_for_probe(1, at + vec2(TILE, 0.0));
            knock_out(&mut game, 0);
            assert!(game.revive_crewmate(1, 0), "{name}");
            let mut steps = 0;
            while game.is_downed(0) && steps < 60 * 30 {
                game.simulate(DT);
                steps += 1;
            }
            assert!(!game.is_downed(0), "{name}: revived");
            let took = steps as f32 * DT;
            // The walk is a tile at most, and the hands-on time is the whole
            // of the rest.
            assert!(
                took >= seconds && took < seconds + 1.5,
                "{name}: {took} s for a {seconds} s revive"
            );
            let hp = game.health(0);
            assert!((hp - 30.0).abs() < 1e-3, "{name}: {hp}");
            assert!(game.was_downed(0));
            let revived = game.take_revives();
            assert_eq!(revived.len(), 1);
            assert_eq!((revived[0].helper, revived[0].patient), (1, 0));
        }
    }

    /// A bot revives a downed crewmate of its own accord, out of harm, and
    /// with the room's reviving switched off nobody does.
    #[test]
    fn a_bot_revives_a_downed_crewmate_of_its_own_accord() {
        for on in [true, false] {
            let mut game = room();
            game.set_revivers(on);
            let at = game.put_for_probe(0, vec2(ROOM_W * 0.5, ROOM_H * 0.5));
            game.put_for_probe(1, at + vec2(3.0 * TILE, 0.0));
            knock_out(&mut game, 0);
            let mut up = false;
            for _ in 0..(60 * 25) {
                game.simulate(DT);
                if !game.is_downed(0) {
                    up = true;
                    break;
                }
            }
            assert_eq!(up, on, "revivers {on}");
            if on {
                assert!(game.is_alive(0));
                assert_eq!(game.take_revives().len(), 1);
            }
        }
    }

    /// The wave cleared, a bot goes at once to revive the downed — a
    /// player first, though a downed bot lies nearer — and only then the
    /// bot: no waiting out the room's calm, nor its own fire.
    #[test]
    fn a_wave_cleared_the_bots_revive_the_players_first_at_once() {
        let mut game = room();
        let extra = Game::bare(3, ROOM_W, ROOM_H)
            .take_crew()
            .into_iter()
            .take(1)
            .collect();
        game.adopt(extra, Vec2::ZERO);
        assert_eq!(game.crew_count(), 3);
        let mid = vec2(ROOM_W * 0.5, ROOM_H * 0.5);
        game.put_for_probe(2, mid);
        game.put_for_probe(1, mid + vec2(2.0 * TILE, 0.0));
        game.put_for_probe(0, mid + vec2(-7.0 * TILE, 0.0));
        let enemy = mid + vec2(0.0, 4.0 * TILE);
        game.set_hostiles(vec![Some((enemy, WeaponKind::LaserPistol.basic()))]);
        for _ in 0..30 {
            game.simulate(DT);
        }
        assert!(game.is_recruited(2), "under arms");
        knock_out(&mut game, 0);
        knock_out(&mut game, 1);
        assert!(game.enemy_strike(game.body_pos(2), 2, 1.0, false));
        game.simulate(DT);
        // The last of the wave down.
        game.set_hostiles(vec![None]);
        let mut first = None;
        for step in 0..30 {
            game.simulate(DT);
            if let Some(p) = game.reviving(2) {
                first = Some((step, p));
                break;
            }
        }
        let (step, patient) = first.expect("a revive taken up at once");
        assert_eq!(patient, 0, "the player first, at step {step}");
        let mut order = vec![0];
        for _ in 0..(60 * 40) {
            game.simulate(DT);
            if let Some(p) = game.reviving(2)
                && order.last() != Some(&p)
            {
                order.push(p);
            }
            if !game.is_downed(0) && !game.is_downed(1) {
                break;
            }
        }
        assert!(!game.is_downed(0) && !game.is_downed(1), "both up");
        assert_eq!(order, vec![0, 1]);
    }

    /// A commander's Medivac medic runs to a player downed with an enemy
    /// in plain sight beside the body and revives him, where a bot of its
    /// own accord waits for the patient to lie out of harm; and it keeps
    /// its hands on him though it has a shot.
    #[test]
    fn a_medivac_medic_revives_a_player_downed_in_the_fight() {
        for medivac in [true, false] {
            let mut game = room();
            let at = game.put_for_probe(0, vec2(ROOM_W * 0.5, ROOM_H * 0.5));
            game.put_for_probe(1, at + vec2(4.0 * TILE, 0.0));
            game.set_medivac(1, medivac);
            game.set_hostiles(vec![Some((
                at + vec2(0.0, 2.0 * TILE),
                WeaponKind::LaserPistol.basic(),
            ))]);
            knock_out(&mut game, 0);
            let mut up = false;
            for _ in 0..(60 * 20) {
                game.simulate(DT);
                if !game.is_downed(0) {
                    up = true;
                    break;
                }
            }
            assert_eq!(up, medivac, "medivac {medivac}");
            if medivac {
                assert_eq!(game.take_revives().len(), 1);
            }
        }
    }

    /// A medic bot revives first (task 125): with a medic free to go, the
    /// bot beside the downed body leaves it to the medic further off; with
    /// no medic in the crew, or the medic downed itself, the bot beside it
    /// goes. Nobody runs from the fight on the way to nought: a bot shot
    /// down is downed where it stood.
    #[test]
    fn a_medic_bot_revives_first_and_the_others_only_without_one() {
        // 0 the player, far off; 1 downed; 2 a bot beside it; 3 a bot
        // further off, the medic or not.
        for case in ["medic free", "no medic", "medic down"] {
            let mut game = room();
            let extra = Game::bare(4, ROOM_W, ROOM_H)
                .take_crew()
                .into_iter()
                .take(2)
                .collect();
            game.adopt(extra, Vec2::ZERO);
            assert_eq!(game.crew_count(), 4);
            let mid = vec2(ROOM_W * 0.5, ROOM_H * 0.5);
            game.put_for_probe(0, vec2(ROOM_W * 0.1, ROOM_H * 0.15));
            let at = game.put_for_probe(1, mid);
            game.put_for_probe(2, at + vec2(1.5 * TILE, 0.0));
            game.put_for_probe(3, at + vec2(-6.0 * TILE, 0.0));
            let mut medic = Skill::NONE;
            medic.medic = case != "no medic";
            medic.revive = 4.0;
            game.set_skills(vec![Skill::NONE, Skill::NONE, Skill::NONE, medic]);
            // Both in the one step, so bot 2 is not already on its way to
            // the medic when 1 goes down.
            let at_down = game.body_pos(1);
            assert!(game.wound(1, 1_000.0).downed);
            if case == "medic down" {
                assert!(game.wound(3, 1_000.0).downed);
            }
            game.simulate(DT);
            assert!(game.is_downed(1));
            assert!(
                (game.body_pos(1) - at_down).len() < 1e-3,
                "downed where it stood: {case}"
            );
            let mut revived = Vec::new();
            for _ in 0..(60 * 25) {
                game.simulate(DT);
                revived.extend(game.take_revives());
                if !game.is_downed(1) {
                    break;
                }
            }
            let helper = revived.iter().find(|r| r.patient == 1).map(|r| r.helper);
            let wanted = if case == "medic free" { 3 } else { 2 };
            assert_eq!(helper, Some(wanted), "{case}");
        }
    }

    /// A player's own Bim revives nobody without an order: the bots do it.
    #[test]
    fn a_player_s_own_bim_revives_only_when_ordered() {
        let mut game = room();
        let at = game.put_for_probe(1, vec2(ROOM_W * 0.5, ROOM_H * 0.5));
        game.put_for_probe(0, at + vec2(2.0 * TILE, 0.0));
        knock_out(&mut game, 1);
        for _ in 0..(60 * 12) {
            game.simulate(DT);
        }
        assert!(game.is_downed(1), "the player's Bim did not go of itself");
        assert!(game.revive_crewmate(0, 1));
        // The walk back from wherever it wandered, and the ten seconds.
        for _ in 0..(60 * 17) {
            game.simulate(DT);
        }
        assert!(!game.is_downed(1), "ordered, it did");
    }

    /// A player's own Bim faces its pointer, never the patient, so the
    /// revive ends the moment its bar is full whichever way it aims —
    /// it once stood full until the pointer crossed the patient.
    #[test]
    fn a_steered_revive_ends_when_its_bar_is_full() {
        use crate::math::PI;
        use crate::order::{CrewOrder, angle_code};
        let mut game = room();
        game.set_autonomous(false);
        game.set_revivers(false);
        game.set_players(1);
        let at = game.put_for_probe(1, vec2(ROOM_W * 0.5, ROOM_H * 0.5));
        game.put_for_probe(0, at + vec2(TILE, 0.0));
        knock_out(&mut game, 1);
        // Aimed straight away from the patient.
        game.order(
            0,
            CrewOrder::Control {
                walk: None,
                aim: angle_code(0.0),
                fire: false,
                sprint: false,
            },
        );
        game.simulate(DT);
        assert!(game.revive_crewmate(0, 1));
        let mut full = None;
        for step in 0..(60 * 20) {
            game.simulate(DT);
            if full.is_none() && game.revive_share(1).is_some_and(|(_, s)| s >= 0.99) {
                full = Some(step);
            }
            if !game.is_downed(1) {
                let full = full.expect("the bar filled first");
                assert!(step - full <= 8, "full at {full}, up at {step}");
                assert!(
                    game.bims[0].character.faces_aim(),
                    "it faced the pointer throughout"
                );
                return;
            }
        }
        panic!("never revived");
    }

    /// Hands on a downed body stand its countdown (task 138): a revive
    /// slower than the seconds the patient had left still brings it up,
    /// and the countdown runs on again from where it stood once the hands
    /// are off.
    #[test]
    fn a_revive_under_way_stands_the_patient_s_countdown() {
        let mut game = room();
        game.set_autonomous(false);
        game.set_revivers(false);
        let mut skills = vec![crate::combat::Skill::NONE; 2];
        skills[1].revive = 20.0;
        game.set_skills(skills);
        let at = game.put_for_probe(0, vec2(ROOM_W * 0.5, ROOM_H * 0.5));
        game.put_for_probe(1, at + vec2(TILE, 0.0));
        knock_out(&mut game, 0);
        // Twenty-five of its thirty seconds gone before anybody comes.
        for _ in 0..(60 * 25) {
            game.simulate(DT);
        }
        let left = game.down_left(0).expect("still downed");
        assert!(left < 6.0, "{left}");
        assert!(game.revive_crewmate(1, 0));
        // Hands on it, the countdown stands.
        let mut steps = 0;
        while game.revive_share(0).is_none() && steps < 60 * 2 {
            game.simulate(DT);
            steps += 1;
        }
        let kneeling = game.down_left(0).expect("still downed");
        for _ in 0..(60 * 8) {
            game.simulate(DT);
        }
        assert_eq!(game.down_left(0), Some(kneeling), "it stood");
        // Let go, it runs on from there.
        assert!(game.order(0, crate::order::CrewOrder::StandDown { who: 1 }) == 0);
        game.simulate(DT);
        game.simulate(DT);
        assert!(game.down_left(0).unwrap() < kneeling, "and runs again");
        // Twenty seconds of hands on it bring it up though it had five.
        assert!(game.revive_crewmate(1, 0));
        for _ in 0..(60 * 23) {
            game.simulate(DT);
        }
        assert!(game.is_alive(0) && !game.is_downed(0), "revived");
    }

    /// The quickselect (task 138): with the medkit in hand the player's
    /// own Bim holds its fire and the kit is drawn; the weapon back in
    /// hand, it fires. An attack order takes the weapon up by itself, and
    /// only a player's own Bim changes hands.
    #[test]
    fn with_the_medkit_in_hand_the_player_s_bim_holds_its_fire() {
        use crate::bim::Hand;
        use crate::order::CrewOrder;
        let mut game = room();
        game.set_autonomous(false);
        game.put_for_probe(0, vec2(ROOM_W * 0.2, ROOM_H * 0.5));
        game.put_for_probe(1, vec2(ROOM_W * 0.2, ROOM_H * 0.85));
        game.issue(1, Gear::default());
        let near = vec2(ROOM_W * 0.4, ROOM_H * 0.5);
        let pistol = WeaponKind::LaserPistol.basic();
        game.set_hostiles(vec![Some((near, pistol))]);
        assert_eq!(game.hand(0), Hand::Weapon);
        game.order(0, CrewOrder::Hand { hand: Hand::Medkit });
        assert_eq!(game.hand(0), Hand::Medkit);
        let mut hits = 0;
        for _ in 0..(60 * 4) {
            game.simulate(DT);
            hits += game.take_hits().len();
        }
        assert!(game.is_recruited(0), "the alarm took it up");
        assert_eq!(hits, 0, "a medkit shoots nothing");
        assert!(!game.bims[0].character.is_armed());
        assert!(game.bims[0].character.has_medkit());
        game.order(0, CrewOrder::Hand { hand: Hand::Weapon });
        for _ in 0..(60 * 4) {
            game.simulate(DT);
            hits += game.take_hits().len();
        }
        assert!(hits > 0, "the weapon back in hand shoots");
        assert!(!game.bims[0].character.has_medkit());
        // An attack order puts the weapon in hand.
        game.order(0, CrewOrder::Hand { hand: Hand::Medkit });
        game.observe();
        assert_eq!(game.order(0, CrewOrder::Attack { enemy: 0 }), ORDER_MOVING);
        assert_eq!(game.hand(0), Hand::Weapon);
        // A bot's hand is nobody's to change: slot 1 is no player here.
        game.order(1, CrewOrder::Hand { hand: Hand::Medkit });
        assert_eq!(game.hand(1), Hand::Weapon);
    }

    /// Locking a door is not everybody's (task 138): the order is refused
    /// unless the body's skill says it may lock, and unlocking stays
    /// anybody's.
    #[test]
    fn only_a_body_whose_skill_says_so_locks_a_door() {
        let layout = crate::aboard::layout_of(&shipdesign::fixture::playtest_ship());
        let (w, h) = (layout.bounds.width(), layout.bounds.height());
        let mut game = Game::with_layout(layout, 7, &[tile_middle(8.0, 10.0)], w, h);
        game.set_autonomous(false);
        let door = door_at_tile(&game, 15.0, 13.0);
        let run = |game: &mut Game| {
            for _ in 0..(60 * 20) {
                game.simulate(DT);
            }
        };
        game.order_door(0, door, door::Order::Lock);
        run(&mut game);
        assert!(!game.ship_door_is_locked(door), "nobody locks");
        assert!(!game.may_lock_doors(0));
        let mut skill = Skill::NONE;
        skill.locks_doors = true;
        game.set_skills(vec![skill]);
        assert!(game.may_lock_doors(0));
        game.order_door(0, door, door::Order::Lock);
        run(&mut game);
        assert!(game.ship_door_is_locked(door), "the class that may, does");
        game.set_skills(vec![Skill::NONE]);
        game.order_door(0, door, door::Order::Unlock);
        run(&mut game);
        assert!(!game.ship_door_is_locked(door), "anybody unlocks");
    }

    /// A bot on its way to a downed crewmate gives way to the player's own
    /// Bim: the player's order drops the bot's revive and the player's
    /// hands count. The bar over the patient is the hands-on part alone,
    /// and a stand-down lets the revive go.
    #[test]
    fn a_bot_s_revive_gives_way_to_the_player_s_and_a_stand_down_lets_it_go() {
        let mut game = room();
        let extra = Game::bare(4, ROOM_W, ROOM_H)
            .take_crew()
            .into_iter()
            .take(1)
            .collect();
        game.adopt(extra, Vec2::ZERO);
        game.set_autonomous(false);
        game.set_revivers(false);
        let mid = vec2(ROOM_W * 0.5, ROOM_H * 0.5);
        let at = game.put_for_probe(1, mid);
        game.put_for_probe(0, at + vec2(TILE, 0.0));
        game.put_for_probe(2, at + vec2(-6.0 * TILE, 0.0));
        knock_out(&mut game, 1);
        assert!(game.is_bot(2) && !game.is_bot(0));
        assert!(game.revive_crewmate(2, 1), "the bot sets out");
        assert_eq!(game.reviving(2), Some(1));
        assert!(!game.revive_crewmate(2, 1), "one reviver among the bots");
        assert_eq!(game.revive_share(1), None, "walking is not kneeling");
        // The player's Bim takes it over.
        assert!(game.revive_crewmate(0, 1));
        assert_eq!(game.reviving(0), Some(1));
        assert_eq!(game.reviving(2), None, "the bot let it go");
        assert!(
            !game.revive_crewmate(2, 1),
            "and a bot does not take it back"
        );
        // Kneeling, the share climbs; standing down lets go.
        let mut share = None;
        for _ in 0..(60 * 4) {
            game.simulate(DT);
            share = game.revive_share(1);
            if share.is_some_and(|(_, s)| s > 0.1) {
                break;
            }
        }
        let (helper, s) = share.expect("hands on");
        assert_eq!(helper, 0);
        assert!(s > 0.1 && s < 1.0, "{s}");
        game.stand_down(0);
        assert_eq!(game.reviving(0), None);
        assert_eq!(game.revive_share(1), None);
        assert!(game.is_downed(1));
    }

    /// In cover is what a bolt is dodged for: behind sandbags between the
    /// body and an enemy, and not in the open, from the other side, or
    /// with no enemy at all.
    #[test]
    fn a_body_behind_bags_is_in_cover_from_the_enemy_beyond_them() {
        let mut game = room();
        game.set_autonomous(false);
        let body = game.put_for_probe(0, vec2(5.5 * TILE, 5.5 * TILE));
        assert_eq!(game.cover_of(0), None, "no enemy, no cover");
        let bags = Rect::from_min_size(vec2(6.0 * TILE, 5.0 * TILE), vec2(TILE, TILE));
        game.set_laid_cover(&[bags]);
        let enemy = vec2(13.5 * TILE, 5.5 * TILE);
        let gun = WeaponKind::LaserPistol.basic();
        game.set_hostiles(vec![Some((enemy, gun))]);
        assert_eq!(game.cover_of(0), Some(enemy), "the bags between");
        game.set_hostiles(vec![Some((body - vec2(8.0 * TILE, 0.0), gun))]);
        assert_eq!(game.cover_of(0), None, "the enemy on the open side");
    }

    /// Only a hit that takes hit points splashes blood: one the armour
    /// stops whole leaves the deck clean, one that gets through marks a
    /// tile or two round the body.
    #[test]
    fn only_a_hit_that_takes_hit_points_splashes_blood() {
        use crate::combat::{ArmourKind, Piece};
        let mut game = room();
        game.set_autonomous(false);
        game.put_for_probe(0, vec2(ROOM_W * 0.5, ROOM_H * 0.5));
        game.put_for_probe(1, vec2(ROOM_W * 0.2, ROOM_H * 0.8));
        let mut gear = game.gear(0);
        gear.armour = Some(Piece::new(7, ArmourKind::Armour, Tier::One));
        game.issue(0, gear);
        let out = game.wound(0, 1.0);
        assert!(out.absorbed > 0.0 && out.through == 0.0, "{out:?}");
        assert_eq!(game.health(0), crate::health::MAX_HEALTH);
        assert_eq!(game.bloody_tiles(), 0, "absorbed whole: no blood");
        let out = game.wound(0, 60.0);
        assert!(out.through > 0.0);
        assert!(game.health(0) < crate::health::MAX_HEALTH);
        let bloody = game.bloody_tiles();
        assert!((1..=2).contains(&bloody), "{bloody} tiles");
    }

    /// Under twenty hit points a body drips as it goes and leaves a trail;
    /// over them it leaves nothing.
    #[test]
    fn a_body_under_twenty_hit_points_drips_a_trail() {
        for (hp, drips) in [(60.0, false), (15.0, true)] {
            let mut game = room();
            game.set_autonomous(false);
            game.put_for_probe(1, vec2(ROOM_W * 0.1, ROOM_H * 0.9));
            game.put_for_probe(0, vec2(ROOM_W * 0.2, ROOM_H * 0.5));
            game.set_health_for_probe(0, hp);
            assert!(game.send_for_probe(0, vec2(ROOM_W * 0.8, ROOM_H * 0.5)));
            for _ in 0..(60 * 10) {
                game.simulate(DT);
            }
            let bloody = game.bloody_tiles();
            if drips {
                assert!(bloody >= 3, "a trail: {bloody} tiles");
            } else {
                assert_eq!(bloody, 0, "no trail at {hp}");
            }
        }
    }

    /// A machine keeps its own body of parts: a strike comes off the part
    /// struck, a wreck is destroyed outright, never downed and never bled.
    #[test]
    fn a_machine_keeps_its_parts_and_never_bleeds_or_goes_down() {
        use crate::droid::{Droid, DroidKind, DroidPart};
        let mut game = room();
        game.set_autonomous(false);
        for who in 0..2 {
            game.put_for_probe(who, vec2(ROOM_W * 0.1, ROOM_H * (0.2 + 0.1 * who as f32)));
        }
        let at = vec2(ROOM_W * 0.7, ROOM_H * 0.5);
        let body = game.add_droid(Droid::new(DroidKind::Trooper, Tier::One, 0, 1, at, 0.0, 5));
        let i = body - game.bims.len();
        let before = game.droid(i).unwrap().body.health(DroidPart::Arms);
        assert!(game.strike_droid(i, DroidPart::Arms, 5.0));
        let after = game.droid(i).unwrap().body.health(DroidPart::Arms);
        assert_eq!(after, (before - 5.0).max(0.0));
        assert_eq!(game.bloody_tiles(), 0, "a machine does not bleed");
        assert!(game.strike_droid(i, DroidPart::Chassis, 1e6));
        assert!(!game.is_downed(body), "a wreck, not downed");
        assert!(!game.is_alive(body));
        assert!(game.down_left(body).is_none());
        for _ in 0..120 {
            game.simulate(DT);
        }
        assert_eq!(game.bloody_tiles(), 0);
    }

    /// *Wide Angle Optics* (task 142): the range a Bim aims and shoots at
    /// is its skill's, with the still range on while it stands still and
    /// off while it walks.
    #[test]
    fn the_still_range_is_on_standing_still_and_off_walking() {
        let mut game = room();
        game.set_autonomous(false);
        let mut skills = vec![crate::combat::Skill::NONE; 2];
        skills[0].still_range = 7.0;
        game.set_skills(skills);
        let at = game.put_for_probe(0, vec2(ROOM_W * 0.25, ROOM_H * 0.5));
        game.simulate(DT);
        assert_eq!(game.shot_skill(0).range, 7.0, "standing still");
        assert_eq!(game.shot_skill(1).range, 0.0, "nobody else's");
        let there = at + vec2(TILE * 6.0, 0.0);
        assert_eq!(game.order_move(0, there.x, there.y), ORDER_MOVING);
        for _ in 0..5 {
            game.simulate(DT);
        }
        assert!(game.is_walking(0));
        assert_eq!(game.shot_skill(0).range, 0.0, "walking");
    }

    /// *Lifeline*'s shield (task 142): it takes a hit's hit points before
    /// the armour or the body does, runs out with them or with its
    /// seconds, and a surge leaves it alone.
    #[test]
    fn a_shield_takes_the_hit_first_and_runs_out() {
        let mut game = room();
        game.set_autonomous(false);
        game.issue(0, Gear::issued());
        game.set_shield(0, 30.0, 10.0);
        let full = game.health(0);
        game.wound(0, 20.0);
        assert_eq!(game.health(0), full, "the shield took it");
        assert!((game.shield_hp(0) - 10.0).abs() < 1e-4);
        game.wound(0, 25.0);
        assert!(
            (full - game.health(0) - 15.0).abs() < 1e-3,
            "the rest through"
        );
        assert_eq!(game.shield_hp(0), 0.0, "spent");
        // A surge takes the hit whole and the shield keeps what it had.
        game.set_shield(0, 30.0, 10.0);
        game.set_surge(0, 5.0);
        game.wound(0, 20.0);
        assert_eq!(game.shield_hp(0), 30.0);
        // And its seconds run out.
        for _ in 0..(60 * 11) {
            game.simulate(DT);
        }
        assert_eq!(game.shield_hp(0), 0.0, "for its seconds");
        assert_eq!(game.shield_left(0), 0.0);
    }

    #[test]
    fn the_keys_walk_the_player_s_bim_and_it_turns_to_the_aim_at_once() {
        use crate::math::PI;
        use crate::order::{CrewOrder, angle_code};
        let mut game = room();
        game.set_autonomous(false);
        game.set_players(1);
        let from = game.put_for_probe(0, vec2(ROOM_W * 0.3, ROOM_H * 0.5));
        game.bims[0].character.heading = 0.0;
        // East on the keys, the pointer due south of it: a quarter turn
        // from where it faces.
        let control = |walk: Option<f32>, aim: f32| CrewOrder::Control {
            walk: walk.map(angle_code),
            aim: angle_code(aim),
            fire: false,
            sprint: false,
        };
        game.order(0, control(Some(0.0), PI / 2.0));
        assert!(game.is_steered(0));
        // One step and it faces the aim: no turn rate.
        game.simulate(DT);
        assert!(
            game.bims[0].character.faces_aim(),
            "it faces the aim in one step, not {}",
            game.bims[0].character.heading
        );
        for _ in 0..66 {
            game.simulate(DT);
        }
        let to = game.bim_pos(0);
        assert!(
            to.x - from.x > TILE,
            "the keys walked it east: {from:?} to {to:?}"
        );
        assert!((to.y - from.y).abs() < 2.0, "facing south all the while");
        assert!(game.is_walking(0));
        // The keys up: it stops where it is.
        game.order(0, control(None, PI / 2.0));
        for _ in 0..30 {
            game.simulate(DT);
        }
        let stood = game.bim_pos(0);
        for _ in 0..60 {
            game.simulate(DT);
        }
        assert!((game.bim_pos(0) - stood).len() < 0.01, "and stands");
        // A bot is nobody's to steer.
        game.order(1, control(Some(0.0), 0.0));
        assert!(!game.is_steered(1));
    }

    /// Walking away from a channel cancels it: a revive begun while the
    /// keys walk the player's Bim, or one it is kneeling at when they
    /// start, is dropped — never queued, never walked back to once the
    /// keys come up, the patient up by then or not.
    #[test]
    fn walking_away_from_a_revive_drops_it_for_good() {
        use crate::math::PI;
        use crate::order::{CrewOrder, angle_code};
        let mut game = room();
        game.set_autonomous(false);
        game.set_players(1);
        game.put_for_probe(0, vec2(ROOM_W * 0.3, ROOM_H * 0.5));
        let lies = game.put_for_probe(1, vec2(ROOM_W * 0.3 + 2.0 * TILE, ROOM_H * 0.5));
        game.knock_out_for_probe(1);
        game.simulate(DT);
        let control = |walk: Option<f32>| CrewOrder::Control {
            walk: walk.map(angle_code),
            aim: angle_code(0.0),
            fire: false,
            sprint: false,
        };
        // Begun while the keys walk it west, away from her.
        game.order(0, control(Some(PI)));
        game.order(0, CrewOrder::Revive { who: 0, patient: 1 });
        game.simulate(DT);
        assert_eq!(game.reviving(0), None, "the keys walking drop it");
        assert_eq!(game.agenda_len(0), 0, "and nothing is queued");
        for _ in 0..20 {
            game.simulate(DT);
        }
        game.order(0, control(None));
        for _ in 0..30 {
            game.simulate(DT);
        }
        let away = (game.bim_pos(0) - lies).len();
        for _ in 0..120 {
            game.simulate(DT);
        }
        assert_eq!(game.reviving(0), None);
        let stood = game.bim_pos(0);
        assert!(
            (stood - lies).len() >= away - 0.5,
            "it did not walk back to her: {stood:?}, {away} off before"
        );

        // Kneeling at her when the keys go down: dropped too.
        assert!(game.revive_crewmate(0, 1));
        let mut steps = 0;
        while game.revive_share(1).is_none() && steps < 600 {
            game.simulate(DT);
            steps += 1;
        }
        assert!(game.revive_share(1).is_some(), "hands on her");
        game.order(0, control(Some(PI)));
        game.simulate(DT);
        game.order(0, control(None));
        for _ in 0..30 {
            game.simulate(DT);
        }
        assert_eq!(game.reviving(0), None);
        assert_eq!(game.agenda_len(0), 0);
        assert!(game.is_downed(1), "and she is still down");
    }

    #[test]
    fn a_steered_bim_fires_along_its_facing_only_while_the_trigger_is_held() {
        use crate::math::PI;
        use crate::order::{CrewOrder, angle_code};
        let mut game = room();
        game.set_autonomous(false);
        game.set_players(1);
        let james = game.put_for_probe(0, vec2(ROOM_W * 0.3, ROOM_H * 0.5));
        game.bims[0].character.heading = PI / 2.0;
        game.put_for_probe(1, vec2(ROOM_W * 0.2, ROOM_H * 0.85));
        // James with the auto rifle, since a pistol fires a click and not
        // a button held (`a_steered_pistol_fires_every_click_and_once_while_held`).
        game.issue(
            0,
            Gear {
                weapon: Some(WeaponKind::AutoRifle.basic()),
                ..Gear::default()
            },
        );
        // Kate unarmed: under the alarm she would fire at the target too.
        game.issue(1, Gear::default());
        let target = james + vec2(4.0 * TILE, 0.0);
        game.set_hostiles(vec![Some((target, WeaponKind::LaserPistol.basic()))]);
        let control = |aim: f32, fire: bool| CrewOrder::Control {
            walk: None,
            aim: angle_code(aim),
            fire,
            sprint: false,
        };
        // Facing south, the trigger up: nothing is fired at the target in
        // plain view, though the weapon is out.
        game.order(0, control(PI / 2.0, false));
        for _ in 0..120 {
            game.simulate(DT);
        }
        assert_eq!(game.bolts_in_flight(), 0, "nothing fires of its own accord");
        assert!(game.is_armed(0), "the weapon is out all the same");
        // The trigger down, still facing south: it fires, and down the
        // way it faces, so the target takes nothing.
        game.order(0, control(PI / 2.0, true));
        let (mut fired, mut hits) = (0, 0);
        for _ in 0..180 {
            let before = game.bolts_in_flight();
            game.simulate(DT);
            fired += usize::from(game.bolts_in_flight() > before);
            hits += game.take_hits().len();
        }
        assert!(
            fired >= 3,
            "a rifle held down fires as fast as it goes: {fired}"
        );
        assert_eq!(hits, 0, "and its bolts fly where it faces");
        // Turned onto the target, the bolts land.
        game.order(0, control(0.0, true));
        for _ in 0..240 {
            game.simulate(DT);
            hits += game.take_hits().len();
        }
        assert!(hits > 0, "aimed at it, it is hit");

        // A quick click: the press and the release reach the room before
        // one step has seen the button held. It is a shot all the same —
        // one, and no burst of them.
        game.order(0, control(0.0, false));
        for _ in 0..120 {
            game.simulate(DT);
        }
        let before = game.bolts_in_flight();
        game.order(0, control(0.0, true));
        game.order(0, control(0.0, false));
        game.simulate(DT);
        assert_eq!(game.bolts_in_flight(), before + 1, "the click fired");
        let mut more = 0;
        for _ in 0..180 {
            let was = game.bolts_in_flight();
            game.simulate(DT);
            more += usize::from(game.bolts_in_flight() > was);
        }
        assert_eq!(more, 0, "and only once");
    }

    /// What the player's crosshair greys past: the weapon's reach through
    /// the skill it shoots with, the optics' tiles standing still too,
    /// and nothing with the medkit in hand.
    #[test]
    fn a_shot_reaches_the_weapon_s_range_through_the_skill_and_none_with_the_medkit() {
        use crate::bim::Hand;
        use crate::order::CrewOrder;
        let mut game = room();
        game.set_autonomous(false);
        game.put_for_probe(0, vec2(ROOM_W * 0.3, ROOM_H * 0.5));
        let pistol = WeaponKind::LaserPistol.basic();
        assert_eq!(game.weapon(0), Some(pistol));
        assert_eq!(game.shot_reach(0), Some(pistol.stats().reach()));
        let optics = Skill {
            range: 2.0,
            still_range: 3.0,
            ..Skill::NONE
        };
        game.set_skills(vec![optics]);
        assert_eq!(
            game.shot_reach(0),
            Some((pistol.stats().range + 5.0) * TILE)
        );
        game.order(0, CrewOrder::Hand { hand: Hand::Medkit });
        assert_eq!(game.shot_reach(0), None);
    }

    #[test]
    fn auto_aim_picks_the_picked_enemy_else_the_nearest_in_reach() {
        let mut game = room();
        game.set_autonomous(false);
        game.set_players(1);
        let james = game.put_for_probe(0, vec2(ROOM_W * 0.2, ROOM_H * 0.5));
        game.issue(
            0,
            Gear {
                weapon: Some(WeaponKind::AutoRifle.basic()),
                ..Gear::default()
            },
        );
        let near = james + vec2(3.0 * TILE, 0.0);
        let far = james + vec2(6.0 * TILE, 0.0);
        let beyond = james + vec2(100.0 * TILE, 0.0);
        let pistol = WeaponKind::LaserPistol.basic();
        game.set_hostiles(vec![
            Some((far, pistol)),
            Some((near, pistol)),
            None,
            Some((beyond, pistol)),
        ]);
        assert_eq!(game.auto_aim(0, None), Some((1, near)), "the nearest");
        assert_eq!(game.auto_aim(0, Some(0)), Some((0, far)), "the picked one");
        // One down or out of reach is passed over for the nearest.
        assert_eq!(game.auto_aim(0, Some(2)), Some((1, near)));
        assert_eq!(game.auto_aim(0, Some(3)), Some((1, near)));
        assert!(game.enemy_up(0) && !game.enemy_up(2));
        // With the medkit in hand there is nothing to shoot with.
        game.order_hand(0, Hand::Medkit);
        assert_eq!(game.auto_aim(0, None), None);
    }

    #[test]
    fn a_steered_pistol_fires_every_click_and_at_its_cooldown_while_held() {
        use crate::order::{CrewOrder, angle_code};
        let mut game = room();
        game.set_autonomous(false);
        game.set_players(1);
        let james = game.put_for_probe(0, vec2(ROOM_W * 0.3, ROOM_H * 0.5));
        game.bims[0].character.heading = 0.0;
        game.put_for_probe(1, vec2(ROOM_W * 0.2, ROOM_H * 0.85));
        game.issue(1, Gear::default());
        assert_eq!(game.weapon(0), Some(WeaponKind::LaserPistol.basic()));
        assert!(WeaponKind::LaserPistol.semi_automatic());
        assert!(!WeaponKind::AutoRifle.semi_automatic());
        let target = james + vec2(4.0 * TILE, 0.0);
        game.set_hostiles(vec![Some((target, WeaponKind::LaserPistol.basic()))]);
        let control = |fire: bool| CrewOrder::Control {
            walk: None,
            aim: angle_code(0.0),
            fire,
            sprint: false,
        };
        let step = |game: &mut Game, fired: &mut u32| {
            let before = game.bims[0].shots;
            game.simulate(DT);
            *fired += game.bims[0].shots - before;
        };
        game.order(0, control(false));
        for _ in 0..60 {
            game.simulate(DT);
        }
        // Held down for three seconds: the press, and then a shot every
        // time the cooldown runs out — ten in all.
        game.order(0, control(true));
        let mut fired = 0;
        for _ in 0..180 {
            step(&mut game, &mut fired);
        }
        assert_eq!(fired, 10, "a pistol held down fires at its cooldown");
        // A full magazine for what follows (October 2026): the ten took
        // ten of its twelve.
        let refill = |game: &mut Game| {
            game.order(0, control(false));
            game.order(0, CrewOrder::Reload);
            for _ in 0..90 {
                game.simulate(DT);
            }
            assert_eq!(game.magazine(0).map(|m| m.0), Some(12));
        };
        refill(&mut game);
        // Clicked three times a second, twice the 1.5 a second a bot
        // fires it at and just past the cooldown: every click is a shot,
        // the moment it is clicked.
        let wait = |game: &mut Game| {
            for _ in 0..30 {
                game.simulate(DT);
            }
        };
        game.order(0, control(false));
        wait(&mut game);
        let mut fired = 0;
        for _ in 0..12 {
            game.order(0, control(true));
            let mut now = 0;
            step(&mut game, &mut now);
            assert_eq!(now, 1, "the click fired the step it came");
            fired += now;
            step(&mut game, &mut fired);
            game.order(0, control(false));
            for _ in 0..18 {
                step(&mut game, &mut fired);
            }
        }
        assert_eq!(fired, 12, "every click a shot");
        // And a click that is down and up again before a step sees it.
        refill(&mut game);
        let mut fired = 0;
        game.order(0, control(true));
        game.order(0, control(false));
        step(&mut game, &mut fired);
        assert_eq!(fired, 1, "the quick click fired at once");
        // A second click inside the cooldown is not lost: it waits for
        // the cooldown to run out and is fired then, and not before.
        let cooldown = (crate::balance::SEMI_AUTO_COOLDOWN / DT).round() as usize;
        for _ in 0..4 {
            step(&mut game, &mut fired);
        }
        game.order(0, control(true));
        game.order(0, control(false));
        let mut steps: usize = 4;
        while fired < 2 && steps < 60 {
            step(&mut game, &mut fired);
            steps += 1;
        }
        assert_eq!(fired, 2, "the early click fired in the end");
        assert!(
            steps.abs_diff(cooldown) <= 1,
            "{steps} steps after the first shot, the cooldown {cooldown}"
        );
        for _ in 0..60 {
            step(&mut game, &mut fired);
        }
        assert_eq!(fired, 2, "and once");
    }

    /// October 2026: a gun fires its magazine and then nothing for its
    /// reload, which is heard; the reload key begins one with shots left;
    /// and a gun put in the hand comes full.
    #[test]
    fn a_magazine_empties_reloads_and_a_new_gun_comes_full() {
        use crate::order::{CrewOrder, angle_code};
        let reloads = |game: &mut Game| {
            game.take_cues()
                .into_iter()
                .filter(|c| {
                    matches!(
                        c.cue,
                        Cue::Reload {
                            weapon: WeaponKind::LaserPistol,
                            by: Some(0)
                        }
                    )
                })
                .count()
        };
        let mut game = room();
        game.set_autonomous(false);
        game.set_players(1);
        let james = game.put_for_probe(0, vec2(ROOM_W * 0.3, ROOM_H * 0.5));
        game.bims[0].character.heading = 0.0;
        game.put_for_probe(1, vec2(ROOM_W * 0.2, ROOM_H * 0.85));
        game.issue(1, Gear::default());
        game.set_hostiles(vec![Some((
            james + vec2(4.0 * TILE, 0.0),
            WeaponKind::LaserPistol.basic(),
        ))]);
        let control = |fire: bool| CrewOrder::Control {
            walk: None,
            aim: angle_code(0.0),
            fire,
            sprint: false,
        };
        game.order(0, control(false));
        for _ in 0..60 {
            game.simulate(DT);
        }
        assert_eq!(game.magazine(0), Some((12, 12, 0.0)));
        reloads(&mut game);

        // Held down for six seconds: twelve at the cooldown, then nothing
        // for the reload — said once, as the twelfth went — then on.
        game.order(0, control(true));
        let mut times = Vec::new();
        let mut heard = 0;
        for step in 0..360 {
            let before = game.bims[0].shots;
            game.simulate(DT);
            if game.bims[0].shots > before {
                times.push(step as f32 * DT);
            }
            heard += reloads(&mut game);
            if times.len() == 12 && game.bims[0].shots == before + 1 {
                assert_eq!(heard, 1, "the twelfth began the reload");
                let (left, size, share) = game.magazine(0).unwrap();
                assert_eq!((left, size), (0, 12));
                assert!(share > 0.9, "a reload just begun: {share}");
            }
        }
        assert_eq!(heard, 1, "one reload in six seconds");
        assert!(times.len() > 13, "{times:?}");
        let gap = times[12] - times[11];
        assert!(
            gap >= crate::balance::PISTOL_RELOAD && gap < crate::balance::PISTOL_RELOAD + 0.35,
            "the reload between the twelfth and the thirteenth: {gap}"
        );
        assert!(
            times[..12].windows(2).all(|w| w[1] - w[0] < 0.35),
            "the magazine at the cooldown: {times:?}"
        );

        // Let go with shots left: the reload key reloads now, and is
        // heard; pressed again while it runs, nothing.
        game.order(0, control(false));
        for _ in 0..30 {
            game.simulate(DT);
        }
        reloads(&mut game);
        let (left, _, _) = game.magazine(0).unwrap();
        assert!(left < 12);
        game.order(0, CrewOrder::Reload);
        assert_eq!(reloads(&mut game), 1, "the key's reload is heard");
        game.order(0, CrewOrder::Reload);
        assert_eq!(reloads(&mut game), 0, "and not twice");
        assert!(game.magazine(0).unwrap().2 > 0.0);
        for _ in 0..80 {
            game.simulate(DT);
        }
        assert_eq!(game.magazine(0), Some((12, 12, 0.0)));
        // Full, the key does nothing.
        game.order(0, CrewOrder::Reload);
        assert_eq!(reloads(&mut game), 0);

        // Half a magazine fired, and a rifle put in the hand: thirty in it.
        game.order(0, control(true));
        for _ in 0..60 {
            game.simulate(DT);
        }
        game.order(0, control(false));
        assert!(game.magazine(0).unwrap().0 < 12);
        game.issue(
            0,
            Gear {
                weapon: Some(WeaponKind::AutoRifle.basic()),
                ..Gear::default()
            },
        );
        assert_eq!(game.magazine(0), Some((30, 30, 0.0)));
        game.simulate(DT);
        assert_eq!(game.magazine(0), Some((30, 30, 0.0)));
        // A blade has none.
        game.issue(
            0,
            Gear {
                weapon: Some(WeaponKind::Schword.basic()),
                ..Gear::default()
            },
        );
        assert_eq!(game.magazine(0), None);
    }

    /// Smooth frames (October 2026): a body is drawn `blend` of the way
    /// from where it stood before the last step to where it stands, one
    /// put somewhere is drawn there at once, and drawing moves nothing.
    #[test]
    fn a_body_is_drawn_between_its_last_two_steps_and_drawing_moves_nothing() {
        use crate::order::{CrewOrder, angle_code};
        let mut game = room();
        game.set_autonomous(false);
        game.set_players(1);
        game.order(
            0,
            CrewOrder::Control {
                walk: Some(angle_code(0.0)),
                aim: angle_code(0.0),
                fire: false,
                sprint: false,
            },
        );
        for _ in 0..30 {
            game.simulate(DT);
        }
        let (was, now) = (game.bims[0].character.was, game.bim_pos(0));
        assert!(now.x > was.x, "it walked: {was:?} to {now:?}");
        game.set_blend(Some(0.25));
        let shown = game.shown_pos(0);
        assert!(
            (shown - (was + (now - was) * 0.25)).len() < 1e-3,
            "{shown:?}"
        );
        game.render();
        assert_eq!(game.bim_pos(0), now, "drawing moved nothing");
        game.set_blend(None);
        assert_eq!(game.shown_pos(0), now);
        // Put somewhere off its walk: drawn where it is, not on the way.
        game.set_blend(Some(0.25));
        let put = game.put_for_probe(0, vec2(ROOM_W * 0.8, ROOM_H * 0.3));
        assert_eq!(game.shown_pos(0), put);
    }

    /// Shift (task 150): the keys walk the player's Bim at `SPRINT` of its
    /// pace, facing the way it runs rather than the pointer, and the
    /// trigger held fires nothing until the sprint ends.
    #[test]
    fn a_sprint_is_quicker_faces_the_run_and_fires_nothing() {
        use crate::math::PI;
        use crate::order::{CrewOrder, angle_code};
        let mut game = room();
        game.set_autonomous(false);
        game.set_players(1);
        game.issue(1, Gear::default());
        game.issue(
            0,
            Gear {
                weapon: Some(WeaponKind::AutoRifle.basic()),
                ..Gear::default()
            },
        );
        let control = |walk: bool, sprint: bool, fire: bool| CrewOrder::Control {
            walk: walk.then(|| angle_code(0.0)),
            aim: angle_code(PI / 2.0),
            fire,
            sprint,
        };
        let walked = |game: &mut Game, sprint: bool| {
            game.order(0, control(false, false, false));
            for _ in 0..60 {
                game.simulate(DT);
            }
            let from = game.put_for_probe(0, vec2(ROOM_W * 0.15, ROOM_H * 0.5));
            game.order(0, control(true, sprint, false));
            for _ in 0..60 {
                game.simulate(DT);
            }
            game.bim_pos(0).x - from.x
        };
        let walk = walked(&mut game, false);
        assert!(
            game.bims[0].character.faces_aim(),
            "walking, it faces the pointer"
        );
        assert!(!game.is_sprinting(0));
        let sprint = walked(&mut game, true);
        assert!(game.is_sprinting(0));
        assert!(
            sprint > walk * 1.7,
            "a second's sprint goes {sprint}, a walk {walk}"
        );
        assert!(
            game.bims[0].character.heading.abs() < 0.05,
            "sprinting it faces the run: {}",
            game.bims[0].character.heading
        );
        // And it eases up to it from the walk over `SPRINT_EASE` (October
        // 2026): part of the way after half of it, the whole pace by its
        // end; letting go of Shift is the walk at once.
        game.order(0, control(true, false, false));
        for _ in 0..60 {
            game.simulate(DT);
        }
        let walking = crate::balance::MARCH_SPEED;
        assert!((game.bims[0].character.speed - walking).abs() < 0.01);
        game.order(0, control(true, true, false));
        let steps = (crate::balance::SPRINT_EASE / DT).round() as usize;
        for _ in 0..steps / 2 {
            game.simulate(DT);
        }
        let full = crate::balance::MARCH_SPEED * crate::balance::SPRINT;
        let halfway = game.bims[0].character.speed;
        assert!(
            halfway > walking + 1.0 && halfway < full - 1.0,
            "easing up: {halfway} between {walking} and {full}"
        );
        for _ in steps / 2..steps {
            game.simulate(DT);
        }
        assert!(
            (game.bims[0].character.speed - full).abs() < 0.01,
            "full pace by the ease's end: {} of {full}",
            game.bims[0].character.speed
        );
        game.order(0, control(true, false, false));
        game.simulate(DT);
        assert!(
            (game.bims[0].character.speed - walking).abs() < 0.01,
            "Shift up is the walk at once: {}",
            game.bims[0].character.speed
        );
        // The trigger held: nothing fires while it sprints, and the
        // moment it walks again it does.
        game.order(0, control(true, true, true));
        for _ in 0..60 {
            game.simulate(DT);
        }
        assert_eq!(game.bolts_in_flight(), 0, "a sprint fires nothing");
        game.order(0, control(true, false, true));
        game.simulate(DT);
        assert!(
            game.bims[0].character.faces_aim(),
            "back on the pointer at once"
        );
        let mut fired = 0;
        for _ in 0..30 {
            let before = game.bolts_in_flight();
            game.simulate(DT);
            fired += usize::from(game.bolts_in_flight() > before);
        }
        assert!(fired > 0, "walking it fires again");
    }

    /// Alt (task 150): a dodge roll goes the way the keys walk the Bim, or
    /// last walked it — never at the pointer — two and a half tiles in
    /// `ROLL_TIME`, waits out its cooldown before the next, and every bolt
    /// reaching the body while it rolls is slipped.
    #[test]
    fn a_dodge_roll_goes_the_way_the_keys_walked_and_slips_every_bolt() {
        use crate::balance::{ROLL_COOLDOWN, ROLL_DISTANCE, ROLL_TIME};
        use crate::math::PI;
        use crate::order::{CrewOrder, angle_code};
        let mut game = room();
        game.set_autonomous(false);
        game.set_players(1);
        game.issue(1, Gear::default());
        game.put_for_probe(0, vec2(ROOM_W * 0.6, ROOM_H * 0.5));
        let control = |walk: Option<f32>| CrewOrder::Control {
            walk: walk.map(angle_code),
            aim: angle_code(0.0),
            fire: false,
            sprint: false,
        };
        // A step west on the keys, then up: it stands facing the pointer,
        // east.
        game.order(0, control(Some(PI)));
        game.simulate(DT);
        game.order(0, control(None));
        for _ in 0..60 {
            game.simulate(DT);
        }
        let stood = game.bim_pos(0);
        game.order(0, CrewOrder::Dodge);
        assert!(game.is_rolling(0));
        // Alt again in the roll does nothing.
        game.order(0, CrewOrder::Dodge);
        let roll_steps = (ROLL_TIME / DT).ceil() as usize + 1;
        for _ in 0..roll_steps {
            game.simulate(DT);
        }
        assert!(!game.is_rolling(0));
        let rolled = game.bim_pos(0) - stood;
        assert!(
            rolled.x < -0.95 * ROLL_DISTANCE && rolled.x > -1.05 * ROLL_DISTANCE,
            "rolled west, the way it last walked, not at the pointer: {rolled:?}"
        );
        assert!(rolled.y.abs() < 1.0);
        assert!(
            game.bims[0].character.faces_aim(),
            "out of it on the pointer"
        );
        // Inside the cooldown Alt is refused.
        game.order(0, CrewOrder::Dodge);
        assert!(!game.is_rolling(0), "the cooldown holds it");
        for _ in 0..((ROLL_COOLDOWN / DT) as usize) {
            game.simulate(DT);
        }
        // Pistol bolts from three tiles west, one a step: standing, they
        // land; rolling towards the shooter, not one does. Fired in the
        // first half of the roll, so each reaches it before the roll ends.
        let volley = |game: &mut Game, steps: usize| {
            let mut landed = 0;
            for _ in 0..steps {
                let at = game.bim_pos(0);
                game.enemy_fire(
                    at - vec2(3.0 * TILE, 0.0),
                    at,
                    WeaponKind::LaserPistol.basic(),
                    false,
                );
                game.simulate(DT);
                landed += game.take_wounds_taken().len();
            }
            landed
        };
        game.order(0, CrewOrder::Dodge);
        assert!(game.is_rolling(0));
        let rolling = volley(&mut game, roll_steps / 2);
        for _ in 0..30 {
            game.simulate(DT);
            assert_eq!(game.take_wounds_taken().len(), 0, "the volley flew past");
        }
        assert_eq!(rolling, 0, "a roll slips every bolt");
        let standing = volley(&mut game, roll_steps / 2);
        assert!(standing > 0, "standing, the same volley lands: {standing}");
    }

    #[test]
    fn an_angle_code_is_a_turn_in_sixty_five_thousand() {
        use crate::math::{PI, TAU};
        use crate::order::{angle_code, code_angle};
        assert_eq!(angle_code(0.0), 0);
        assert_eq!(angle_code(PI), 32768);
        assert_eq!(angle_code(-PI / 2.0), 49152);
        assert_eq!(angle_code(TAU), 0);
        for code in [0u16, 1, 12345, 32768, 65535] {
            assert_eq!(angle_code(code_angle(code)), code);
        }
    }
}
