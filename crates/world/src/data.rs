//! The numbers the world runs on, kept apart from the loop that uses them.
//!
//! Placeholders, all of them, and the two that are not merely decorative are
//! the ranges: [`VISION_RANGE`] and [`RADAR_RANGE_PER_SENSOR`] decide how much
//! of a system a crew can see, which is the whole of what exploring is in this
//! step. They are set against the world generator's own scale — a one-day hop
//! for the reference ship is 518 400 units — so that eyesight reaches nowhere
//! at all and one sensor array reaches about three days out.

use economy::Money;
use worldgen::Node;

/// How long one step of the world is, in game minutes.
///
/// One sixtieth of a real second at 1x, which is the room's step exactly. The
/// two simulations are separate and always will be, but a player who has
/// learnt what 24x feels like in one should not have to learn it again in the
/// other.
pub const STEP_MINUTES: f64 = time::MINUTES_PER_SECOND / 60.0;

/// A day a minute: the speed the room's own slider tops out at, and the one
/// everything above was tuned against.
pub const DAY_SPEED: u32 = 24;

/// How fast the world will run: twice that, a day every half minute. One
/// constant, and raising it is not a change to this step — see the note on
/// `MAX_STEPS_PER_FRAME` in `crates/app/src/screens/game.rs`, which has to
/// move with it or the top of the range stops being reachable.
pub const TOP_SPEED: u32 = 48;

/// How far the crew can see with their own eyes.
///
/// Small on purpose. A one-day hop is over half a million units, so this
/// reaches roughly a tenth of the way to the nearest thing — which makes a
/// ship with no sensor array a ship that can only fly to what it is already
/// beside.
pub const VISION_RANGE: f64 = 50_000.0;

/// What each sensor array adds. Three days' travel for the reference ship,
/// give or take, so one of them turns a system from a fog into a map.
pub const RADAR_RANGE_PER_SENSOR: f64 = 1_500_000.0;

/// And what no number of them will reach past. Without a ceiling, a hull
/// covered in arrays would discover every system it entered on the first
/// frame, and there would be nothing left to fly out and look at.
pub const RADAR_RANGE_MAX: f64 = 6_000_000.0;

/// How near something has to be for the view to be *about* it rather than
/// about the space around it.
///
/// A body is a great deal bigger than a station even though the generator
/// stores both as points, so it gets the wider circle.
pub fn local_radius(node: Node) -> f64 {
    match node {
        Node::Station(_) => LOCAL_RADIUS_STATION,
        Node::Body(_) => LOCAL_RADIUS_BODY,
    }
}

pub const LOCAL_RADIUS_STATION: f64 = 20_000.0;
pub const LOCAL_RADIUS_BODY: f64 = 60_000.0;

/// How much further out than the entry radius the exit is. A quarter again.
pub const LOCAL_HYSTERESIS: f64 = 1.25;

/// How near the ship has to come to a station's hull for the people living
/// there to be simulated: fifty tiles. Nearer than the local frame by a long
/// way, because a room is a whole simulation and a station seen from the far
/// side of its frame is a shape, not a place. Measured from the ship's
/// position to the station's hull, not its centre, so a big station is not
/// further away than a small one at the same door. The way out is
/// [`LOCAL_HYSTERESIS`] further.
pub const RESIDENTS_RANGE: f64 = 50.0 * shipdesign::TILE as f64;

/// Dressings every one of a station's people carries **in its own pack**
/// when its room opens (feature 87: a bandage is a thing, and there is no
/// count on a shelf anywhere), so it can bind a wound the crew gave it. A
/// station has no hold the world keeps and nobody restocks it, so this is
/// all there ever is while the room is open.
pub const RESIDENT_BANDAGES: u32 = 2;

/// And medkits, the same way: enough to treat one of their own that the
/// crew left dying, once the fight is over.
pub const RESIDENT_MEDKITS: u32 = 1;

/// How many hours of the **world clock** go by before a wave of machines
/// grows by one, and by one again every time as many more have — and a
/// held station's count of waves by one every second time
/// ([`crate::droid::time_steps`], feature 105). Hours since the world
/// opened (`World::hours_gone`), not the crew's calendar. Only travel
/// moves that clock, so this is a step every so many trips.
///
/// **Three weeks**, from `tests_mission::travel_days_over_ten_galaxies`
/// (`cargo test --release -p world -- --ignored --nocapture
/// travel_days_over_ten_galaxies`): the crew start 8 to 152 hops from the
/// machines' origin, 76 at the median, and a jump is 2.4 days at the
/// median and a trip within a system 2.9. So a run to the origin at the
/// medians is about **167 days** taking one jump a hop and about **363**
/// taking a jump and a trip in every system. A solo wave has thirteen
/// steps between its three at day nought and [`DROID_WAVE_MAX`], so:
///
/// - at three weeks the jump-a-hop run arrives seven steps up — a solo
///   wave of ten, five waves — and the jump-and-a-trip run reaches the
///   cap on about day 273, three quarters of the way, with ten waves at
///   the origin. Both climb for most of the way and neither is flat for
///   long;
/// - at two weeks the longer run was capped by day 182, flat for its
///   second half; at four (near the thirty days this replaced) the
///   shorter run arrived only six steps up, and the first month was the
///   waves of day nought.
///
/// Four players start at six and reach the cap in ten steps, about day
/// 210 — more players, a thicker fight sooner, and never a longer one.
pub const ENEMIES_HOURS: u32 = 21 * 24;

/// The least any trip puts the world clock on by, in hours, however
/// close together its two ends are (feature 105): a trip shorter than
/// this is quoted, shown and taken as this long (`World::travel_quote`).
///
/// It is there to shut the zero-time re-entry: the machines scale on the
/// world clock and nothing else ([`ENEMIES_HOURS`]), and a site left
/// uncleared is put back as the crew met it (`crate::run`), so two sites
/// a few minutes apart would otherwise be a fresh fight at day nought's
/// strength as often as a crew liked. A day, which is a turn of the date
/// the map shows, the least an in-system trip was quoted at is 0.11 of
/// (`tests_mission::travel_days_over_ten_galaxies`), and under the p25
/// of both kinds of trip (1.2 and 1.4 days) — so it only reaches the odd
/// short hop and changes nothing about a typical one.
pub const MIN_TRAVEL_HOURS: u32 = 24;

/// The arena the `droids` command docks at (`crate::station::arena`): how
/// many tiles across — bigger than any kind of station, for corridors
/// worth fighting down — and how many columns of bunks its quarters hold,
/// three tiles apart: four, which were a bunk each for the sixteen of the
/// human garrison it was laid out for, and are kept so the arena is the
/// deck it always was.
pub const ARENA_SIDE: u32 = 72;
pub const ARENA_BUNK_COLUMNS: u32 = 4;

/// How many mercenaries the `test` command's dock has for hire at the
/// least, whatever the roll said (`World::mercenary_for_probe`): one, so
/// there is always somebody to click on and price.
pub const TEST_MERCENARY: u32 = 1;

/// How far out a station is drawn as a hull in the ship view rather than
/// left to the map. Twice the local frame: far enough that a station comes
/// into the picture as a speck and grows, rather than appearing.
pub const STATION_VISIBLE: f64 = 2.0 * LOCAL_RADIUS_STATION;

/// How long the hyperdrive charges before a jump, in game minutes: what a
/// trip across a hyperlane adds to its length (`World::travel_quote`,
/// feature 103). Twenty seconds of real time at 1x, since a game minute
/// is a real second there (`time::MINUTES_PER_SECOND`).
pub const JUMP_CHARGE_MINUTES: f64 = 20.0 * time::MINUTES_PER_SECOND;

/// How far from everything in a system a jump lands, in world units. Four
/// times a body's arrival radius, so the ship is in empty space and not on
/// the doorstep of whatever it happens to be nearest — a trip from there
/// is a trip. `crate::jump::landing_point` is what uses it.
pub const JUMP_CLEARANCE: f64 = 4.0 * flight::data::ARRIVAL_RADIUS_BODY;

/// How far a crew member may stand from a container's footprint and still
/// reach into it, in tiles: an armoury, a shelf or a cold store two tiles
/// off is near enough to take a piece out of or put one back. What a stow
/// or a fetch is refused beyond (`Refusal::OutOfReach`); putting on what
/// is already in the pack wants no container at all.
pub const REACH: f32 = 2.0;

/// How far inside a door the people going through it are sent, in tiles:
/// the corridor just inside a station's port, and the deck just inside the
/// ship's.
pub const ASHORE_TILES: f64 = 2.5;

/// The galaxy the **simulation** opens in, and the one a page with no lobby
/// behind it falls back to.
///
/// The game proper never uses it: the lobby's World tab picks a seed and a
/// galaxy type and hands both over. `nix run .#simulation` has no lobby and
/// wants the same world every time, so it starts here — with
/// [`crate::spawn`] picking the dock — unless the query says otherwise.
pub const DEFAULT_SEED: u64 = 0x_5749_4e44_4f57_0001;

/// What the simulation's crew have in hand when it opens. A placeholder, in
/// whole euros: enough to buy a hold of something at the first station and
/// not so much that money stops mattering. The game proper starts on
/// [`START_MONEY_PER_BIM`] instead.
pub const SIMULATION_MONEY: Money = 50_000;

/// What each player's Bim brings to a run, in whole euros, into the one
/// pool the crew share (feature 102): a run is time and money, and this is
/// the money. A crew of three sets out with fifteen thousand, a crew of one
/// with five — nothing added for going alone. The lobby's default, and
/// what `ship::Session::run` opens the world with.
pub const START_MONEY_PER_BIM: Money = 5_000;

/// How long putting a part together takes, in game minutes: this much
/// whatever it is, plus this much per hundred euros of the part's price.
/// A wall at 100 euros is six minutes; a heavy engine at 75 000, a little
/// over six hours. It was per unit of materials until the money rework
/// (feature 95) took the materials away, and nothing is carried to a site
/// now, so the walk is the whole of what is on top.
pub const BUILD_MINUTES_BASE: f64 = 5.0;
pub const BUILD_MINUTES_PER_HUNDRED: f64 = 0.5;

/// Combining two of a kind at the workbench into one of the next tier
/// (`World::upgrade`) is a day's work, taken an hour at a time: each
/// session is one `Order` the room runs like a recipe, and the world counts
/// the sessions, so the progress is the world's and whole hours, and a Bim
/// that goes to eat between two loses nothing. Pinned together as a day
/// by `upgrade_sessions_make_a_day`.
pub const UPGRADE_SESSION_MINUTES: f64 = time::HOUR;
pub const UPGRADE_SESSIONS: u32 = 24;

/// A planet's surface — a rocky planet's or an ice world's — is a place
/// the ship lands at: a **town** laid out on the ground as a station is
/// laid out in a hull (`crate::surface`, `crate::station::Plan::Surface`).
/// This many tiles across — bigger than any station, and than the arena —
/// and built like a fort: a wall round the whole of it with a gate in
/// the north wall and one in the south, the landing pad in its west
/// wall, the watch house and the trading house beside the pad, a
/// gathering hall, houses along its streets, fields or greenhouses to
/// feed it, and the wild over whatever ground the town leaves inside the
/// wall; the plain is beyond it. Room for the biggest town
/// ([`SURFACE_POPULATION`]) — thirty people want sixteen strips of field
/// and a hall with fifteen tables in it — and open ground round that.
pub const SURFACE_SIDE: u32 = 96;

/// How many people live in a town, at the least and at the most,
/// inclusive, rolled off the surface's own stream
/// (`crate::surface::Surface::all_of`). A town is sized by it: a bunk
/// each and two over for mercenaries, a chair each in the hall, a strip
/// of field for every two of them or a bay under glass for every four, a
/// bathhouse for every twelve. Five is a hamlet round a pad; thirty
/// fills the streets. (Ten to fifty until feature 66.)
pub const SURFACE_POPULATION: (u32, u32) = (5, 30);

// --- the droids (feature 83) ---------------------------------------------

/// How many machines a wave of an infested station is before the players
/// and the clock are counted (`crate::droid::wave_size`): this many, one
/// a **player** Bim, and one every [`ENEMIES_HOURS`] of the world clock.
/// Nothing else — not the bots, the worth or the levels (feature 105).
pub const DROID_WAVE_BASE: u32 = 2;
/// The most a wave ever is. **A performance limit, not a balance one**:
/// every machine is a body stepped, a stand scored and a line traced, and
/// past this a wave costs more of a frame than the fight is worth. See
/// the measurements in the root `CLAUDE.md`.
pub const DROID_WAVE_MAX: u32 = 16;
/// How many waves an infested station has, before the crew are counted
/// (`crate::droid::wave_count`): this many, and one every second
/// [`ENEMIES_HOURS`] of the world clock. Fixed at the crew's **first
/// dock** and never worked out again.
pub const DROID_WAVES_BASE: u32 = 2;
/// How long after the last machine of a wave is destroyed the next one
/// arrives, in steps of the **mission clock** (feature 103) — two minutes
/// of it at 1×, which was two hours of the old world clock. Never while
/// one is still standing. The world clock stands still during a mission
/// (only travel moves it), so a wave is timed from the arrival like every
/// other in-mission timer, and not by the day.
pub const DROID_REINFORCE_STEPS: u64 = 7_200;
/// How far beyond a town's wall the machines' lander sets down, in
/// tiles: far enough that its own picture does not overlap the gate it
/// unloaded through.
pub const DROID_LANDER_TILES: f64 = 7.0;

// --- the crisis (feature 92) ---------------------------------------------
//
// The machines appear at one star and spread a hyperlane hop at a time.
// The rule is two numbers and no state: a star is infested on
// `DROID_SPREAD_DAYS * hops` and every day after, where `hops` is its lane
// distance from the origin (`worldgen::Galaxy::lanes`). **The crisis is
// there from day nought** (feature 102): the origin is theirs when a run
// opens, and every system due by then is infested as if the spread had
// already run; the probes move that first day with
// `World::set_crisis_first_day_for_probe`. Nothing is rolled per tick,
// nothing accumulates, and two clients that agree about the day and the
// graph agree about the whole galaxy.

/// How many days the crisis takes to cross one hyperlane hop. At three
/// lanes a star the galaxy is some thirty hops across, so this is what
/// decides whether the whole of it falls in a season or in a year — see
/// the measurements in the root `CLAUDE.md`.
pub const DROID_SPREAD_DAYS: u32 = 5;
/// How far from the crew's own star the origin is rolled, in hops: far
/// enough that the crisis is a rumour on the chart for a month or two
/// before it is at the door. A galaxy too small to put one that far away
/// puts it as far as it has (`crate::droid::origin`).
pub const DROID_ORIGIN_MIN_HOPS: u16 = 8;

// --- the jammer, and how hard the machines are (feature 93) --------------

/// How near the machines' origin a system has to be for its machines to
/// come at **tier three**, in hops: this many or fewer. Everywhere else
/// they come at tier one, until there is a general rule for what tier an
/// enemy carries. `World::droid_tier` is where it is read, and
/// `BIMS_DROID_TIER` is what overrides it in the probes.
///
/// Two rather than one because the origin itself is a star the crew will
/// hardly ever reach: what this is for is the fight getting harder as they
/// push *towards* where the machines began, and a radius of one would be
/// one system in the whole galaxy.
pub const DROID_TIER_THREE_HOPS: u16 = 2;

// --- the front (feature 94) ----------------------------------------------
//
// The crisis has an edge, and the systems just outside it are the front.
// How far outside is `World::front` — the hops from the origin less the
// radius the infection has reached by today — and everything here is read
// off that one number: what a desk charges for a gun, and how many hands
// are for hire.

/// How far outside the infection a system still counts as the front, in
/// hops. Three, so a crew have a band of systems to trade and hire in
/// rather than one, and so that the band moves past them at a hop every
/// [`DROID_SPREAD_DAYS`] the way the infection does.
pub const FRONT_HOPS: u16 = 3;
/// How much a hop nearer the front adds to a desk's lean on a weapon, a
/// piece of armour, a medkit or a bandage, in per cent of the book. The
/// system on the edge of the infection pays `FRONT_BIAS * FRONT_HOPS`
/// — fifteen per cent — over the roll, and the one three hops out five.
pub const FRONT_BIAS: i32 = 5;

// --- defending a town (feature 94) ---------------------------------------

/// How long after the crew set down at a threatened town the first wave
/// of machines lands, in steps of the **mission clock** (feature 103): a
/// minute of it at 1×, which was an hour of the old world clock — long
/// enough to walk the town, talk to the desk and hire what is for hire
/// before the shooting starts.
pub const DEFENSE_DELAY_STEPS: u64 = 3_600;
/// What share of a defended town's surviving people join the crew, in per
/// cent, when the last wave is destroyed. A fifth, rounded down, and never
/// fewer than one while there is anybody but the guard left to come.
pub const DEFENSE_JOIN_PERCENT: u32 = 20;

/// What the Republic pays for an enemy taken down, by the tier of the gear
/// it carried: index one is tier one, and index nought is no tier at all
/// and is never asked for (feature 95).
///
/// **Fighting is how a crew earn.** Nothing is mined and nothing is made
/// but medicine, so the only money that comes into the game comes across a
/// desk or off a body — and a crew that never fights never gets rich.
/// Each step is three times the last on purpose: a tier-three enemy is
/// worth pushing towards, which is what the crisis wants of them.
/// Placeholders, like every other number here.
pub const REPUBLIC_BOUNTY: [Money; 4] = [0, 500, 1_500, 4_500];

// --- the run: death and buyback (feature 103) ------------------------------

/// What the pool pays to bring a dead player's Bim back, at the start of
/// the next mission: it respawns aboard the ship with no gear, its level,
/// experience and talents kept. Paid automatically, longest-dead first,
/// while the pool holds this much; a player the pool cannot pay for stays
/// out and is tried again at the mission after. The same as a player's
/// share of the starting pool ([`START_MONEY_PER_BIM`]), so a crew that
/// has earned nothing yet can buy one of its own back once.
pub const BUYBACK_COST: Money = 5_000;
/// What a **bot** Bim's death costs the pool — a hired hand, a townsperson
/// who joined, any crew member no player steers. It is gone for good, and
/// the pool pays this the moment it dies, never going below nought: what
/// it cannot pay is dropped.
pub const BOT_DEATH_PENALTY: Money = 5_000;

// --- how hard the machines are, by time and distance (feature 106) ---------

/// How long the world clock runs before the machines come at **tier two**
/// anywhere, in hours: a fortnight, which is five or six trips (a trip is
/// two or three days as a rule — the root `CLAUDE.md`, *How long a trip
/// is*). Tier two used to wait on the crew researching it; with research
/// gone from the game it waits on time, which is what the machines scale
/// on (feature 105). Past it, [`ENEMY_TIER2_SURE_HOPS`] is the ramp.
pub const ENEMY_TIER2_HOURS: u32 = 14 * 24;
/// The distance ramp once [`ENEMY_TIER2_HOURS`] is past: a site's machines
/// are tier two with odds of one in this many a hop from the crew's own
/// star — nought at home, half at half of it — and **always** at this
/// many hops or more. Rolled once a site, off the galaxy's seed, so a
/// quote on the map and the wave on arrival agree. Tier three within
/// [`DROID_TIER_THREE_HOPS`] of the origin comes first.
pub const ENEMY_TIER2_SURE_HOPS: u16 = 6;

// --- relics (feature 106, `crate::relic`) -----------------------------------

/// *Focusing Lens*: what its weapon's damage is raised by, in per cent.
pub const FOCUSING_LENS_DAMAGE_PERCENT: i32 = 10;
/// *Servo Braces*: what its pace is raised by, in per cent.
pub const SERVO_BRACES_SPEED_PERCENT: i32 = 10;
/// *Field Plating*: what the protection of what it wears is raised by, in
/// per cent.
pub const FIELD_PLATING_ARMOUR_PERCENT: i32 = 10;
/// *Coolant Loop*: what its class's cooldowns are **shortened** by, in per
/// cent.
pub const COOLANT_LOOP_COOLDOWN_PERCENT: i32 = 10;
/// *Steady Grip*: what its odds of hitting are raised by, in per cent.
pub const STEADY_GRIP_ACCURACY_PERCENT: i32 = 10;
/// *Trauma Kit*: what a medkit, a bandage and a medic's beam put back into
/// it is raised by, in per cent — where a medkit starts a part again from,
/// and the blood and the mending a beam gives. A bandage closes wounds and
/// puts nothing back, so it is the one of the three the relic cannot
/// raise.
pub const TRAUMA_KIT_HEALING_PERCENT: i32 = 25;
/// *Second Wind*: how long after going down it gets up again, in seconds
/// of the mission clock — the first time in a mission.
pub const SECOND_WIND_SECONDS: f64 = 5.0;
/// *Second Wind*: the share of its health it gets up with, in per cent;
/// the blood comes back to where a body stands up
/// (`bims::health::SLOWED_AT`) and every wound is closed, or it would be
/// down again at once.
pub const SECOND_WIND_HEALTH_PERCENT: u32 = 25;
/// *Salvage Beacon*: what the bounty for a machine it destroyed is raised
/// by, in per cent — paid on the site's clear like every bounty.
pub const SALVAGE_BEACON_BOUNTY_PERCENT: i32 = 20;
/// *Overcharge Cell*: every how many shots is the charged one.
pub const OVERCHARGE_CELL_EVERY: u32 = 5;
/// *Overcharge Cell*: what the charged shot's damage is raised by, in per
/// cent — a hundred is double.
pub const OVERCHARGE_CELL_DAMAGE_PERCENT: i32 = 100;
/// *Last Stand*: what its weapon's damage is raised by while another
/// player's Bim is down, in per cent.
pub const LAST_STAND_DAMAGE_PERCENT: i32 = 25;
/// *Kill Relay*: the seconds every class cooldown running on it loses when
/// a machine it hit last is destroyed.
pub const KILL_RELAY_SECONDS: f64 = 1.0;
/// *Phase Harness*: the share of its health a hit has to take it under, in
/// per cent — once a mission.
pub const PHASE_HARNESS_BELOW_PERCENT: u32 = 25;
/// *Phase Harness*: how long nothing hurts it after, in seconds — the
/// room's own surge (`bims::game::Game::set_surge`), halo and all.
pub const PHASE_HARNESS_SECONDS: f32 = 2.0;

/// How many relics a site cleared with machines in it offers.
pub const RELIC_OFFER: usize = 3;
/// The odds a site the machines hold hides a **relic cache** on its
/// research desk, in per cent: rolled once a site, off the galaxy's seed,
/// when the machines take it.
pub const RELIC_CACHE_CHANCE: u32 = 40;
/// How many relics a won run unlocks in each player's profile: the first
/// still locked, in list order.
pub const RELICS_UNLOCKED_PER_WIN: usize = 2;
