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

/// What a station's people keep in their cold store, a head: the targets
/// their own manager works to — the bay planted to keep the greens and the
/// soy up, stew cooked ahead for the shelf. Theirs, not the crew's: the
/// Management tab is the crew's own and reaches nobody ashore.
pub const RESIDENT_VEG_EACH: u32 = 100;
pub const RESIDENT_TOFU_EACH: u32 = 50;
pub const RESIDENT_STEW_EACH: u32 = 2;
/// Dressings every one of a station's people carries **in its own pack**
/// when its room opens (feature 87: a bandage is a thing, and there is no
/// count on a shelf anywhere), so it can bind a wound the crew gave it. A
/// station has no hold the world keeps and nobody restocks it, so this is
/// all there ever is while the room is open.
pub const RESIDENT_BANDAGES: u32 = 2;

/// And medkits, the same way: enough to treat one of their own that the
/// crew left dying, once the fight is over.
pub const RESIDENT_MEDKITS: u32 = 1;

/// How many people an enemy station puts up against the crew, before the
/// crew themselves are counted: a hostile station's room is opened with
/// [`crate::station::enemies_of`] rather than `residents_of` — this many,
/// one more every [`ENEMIES_DAYS`] the game has run, one more a crewmate,
/// and doubled for every half of the crew's starting worth their worth
/// has grown by since, up to [`ENEMIES_MAX`]. So a rich crew finds every
/// enemy's dock harder than a poor one does, and an old crew harder than
/// a new one.
pub const ENEMIES_BASE: u32 = 1;
/// How many whole game days go by before the base grows by one, and by
/// one again every time as many more have: thirty, so the first month
/// is met at [`ENEMIES_BASE`] however long the crew dawdle, and every
/// month after it the enemy is one stronger before the worth is looked
/// at ([`crate::station::base_by_day`]). Days since the world opened
/// (`World::days_gone`), not the crew's calendar, and the same on a
/// raider's bunks ([`crate::raid::boarders_of`]).
pub const ENEMIES_DAYS: u32 = 30;
/// The most an enemy station ever arms. The room sleeps at most as many
/// as it has bunks — a station's quarters hold a handful, the arena's a
/// garrison — and past this it is a crowd, and every step of it is paid
/// for in sight and shots.
pub const ENEMIES_MAX: u32 = 16;

/// The arena the `combat` command docks at (`crate::station::arena`): how
/// many tiles across — bigger than any kind of station, for corridors
/// worth fighting down — and how many columns of bunks its quarters hold,
/// three tiles apart, so that a garrison of [`ENEMIES_MAX`] has a bunk
/// each and the room opens with every one of them in it.
pub const ARENA_SIDE: u32 = 72;
pub const ARENA_BUNK_COLUMNS: u32 = 4;
/// The garrison the arena arms whenever it is hostile, whatever the crew's
/// worth: `World::reinforcements` is set by the `combat` command to make
/// [`crate::station::enemies_of`] up to this. It was fifteen against a
/// crew of fourteen; the crew grew by two field medics
/// (`shipdesign::fixture::COMBAT_CREW`, sixteen), and sixteen crew put up
/// [`ENEMIES_MAX`] by the formula alone — reinforcements only ever add —
/// so the arena's garrison is the cap, and every one of them has a bunk.
pub const ARENA_GARRISON: u32 = ENEMIES_MAX;

/// How many mercenaries the `test` command's dock has for hire at the
/// least, whatever the roll said (`World::mercenary_for_probe`): one, so
/// there is always somebody to click on and price.
pub const TEST_MERCENARY: u32 = 1;

/// How far out a station is drawn as a hull in the ship view rather than
/// left to the map. Twice the local frame: far enough that a station comes
/// into the picture as a speck and grows, rather than appearing.
pub const STATION_VISIBLE: f64 = 2.0 * LOCAL_RADIUS_STATION;

/// How long the ship takes to push off its berth once everybody is where
/// they belong, and how long it takes to come alongside once a trip has
/// ended, in game minutes. Both are read off the clock in closed form —
/// see `World::cast_off` and `World::come_alongside` — so a browser at 24x
/// and a server catching up put the ship in the same place.
pub const UNDOCK_MINUTES: f64 = 3.0;
pub const DOCK_MINUTES: f64 = 5.0;

/// How long the hyperdrive charges before it fires, in game minutes: twenty
/// seconds of real time at 1x, since a game minute is a real second there
/// (`time::MINUTES_PER_SECOND`). Read off the clock like a docking — see
/// `crate::jump`.
pub const JUMP_CHARGE_MINUTES: f64 = 20.0 * time::MINUTES_PER_SECOND;

/// How far from everything in a system a jump lands, in world units. Four
/// times a body's arrival radius, so the ship is in empty space and not on
/// the doorstep of whatever it happens to be nearest — a trip from there
/// is a trip. `crate::jump::landing_point` is what uses it.
pub const JUMP_CLEARANCE: f64 = 4.0 * flight::data::ARRIVAL_RADIUS_BODY;

/// How long the ship waits at the berth for the station's people to go
/// ashore and its own to come back aboard before it leaves without them.
/// Whoever is still on the wrong side of the airlock then is put where the
/// room puts a body with no floor under it — see `crew::Aboard::unjoined`.
/// An hour: the walk back from the far end of the biggest station is the
/// better part of half of one.
pub const CASTING_OFF_LIMIT: f64 = 60.0;

/// How far a crew member may stand from the helm's seat and still be at the
/// helm: a tile, which is where a route to the seat can be relied on to
/// leave a body, and no further.
pub const HELM_REACH: f64 = shipdesign::TILE as f64;

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

/// How long one tile of rock takes to mine, in game minutes, with the Bim
/// standing beside it with a pick. A walk is as many of these as there are
/// marked rocks it can get to, on top of the suit and the airlock either
/// end and the walk out to each.
pub const MINE_TILE_MINUTES: f64 = 12.0;

/// What the suit lets through, as a multiplier on the open-air dose rate
/// in `crates/health`: a quarter, so a walk is about twenty-two minutes'
/// worth of dose, and the dose comes off at half a unit a minute inside.
pub const SUIT_INTENSITY: f64 = 0.25;

/// The dose above which a Bim is not sent out again: half the critical
/// line. Two walks back to back are fine; a third waits for the dose to
/// come off. This is what bounds a walk outside — there is no air gauge.
pub const EVA_DOSE_LIMIT: f64 = health::CRITICAL / 2.0;

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

/// How high over a planet a landing begins its descent and a lift-off
/// ends, in world units: where a trip to the body ends, so a ship that
/// has just lifted off is exactly where one that has just arrived is.
/// A landing slides to this point straight over the planet first and
/// comes down from there — the ship approaches from the top — and a
/// lift-off climbs straight up to it.
pub const LANDING_HEIGHT: f64 = flight::data::ARRIVAL_RADIUS_BODY;

/// How long a landing and a lift-off take, in game minutes, read off the
/// clock the way a docking is (`World::come_alongside`, `World::cast_off`).
/// Longer than a docking: the planet has to grow under the ship.
pub const LAND_MINUTES: f64 = 8.0;
pub const LIFT_MINUTES: f64 = 6.0;

/// How long the cold store goes without power before the food in it
/// loses a share, in steps: a game hour — sixty minutes over
/// [`STEP_MINUTES`] — counted on `World::cold_store_out`, an integer clock
/// in the checksum, so two clients spoil the same hour on the same step.
/// A cold store with power is not on the clock: a battery that covers an
/// overdraw costs the larder nothing.
pub const SPOIL_STEPS: u64 = (time::HOUR / STEP_MINUTES) as u64;

/// What share of the food goes each hour the cold store is unpowered:
/// one part in this many of the vegetables, the tofu and the stew,
/// rounded **up**, so a shelf with one thing on it loses it in an hour
/// rather than keeping it for ever. An eighth an hour is a larder gone
/// in a day — a brownout is expensive, never fatal.
pub const SPOIL_DIVISOR: u32 = 8;

/// Raiders — `crate::raid`. How many boarders a raider carries at most:
/// the raider has a bunk for each, and a ship's deck is not an arena.
/// The count itself is [`crate::raid::boarders_of`]: one, one a month
/// gone by and one a crewmate, doubled with the crew's worth the way a
/// station's garrison is, and never more than this.
pub const BOARDERS_MAX: u32 = 6;

/// How long between raids, in whole game minutes: a gap of at least the
/// first and less than the first plus the second, rolled off the raid's
/// own stream (`crate::raid::Raids::gap`). A day to three days, so a
/// crew holding at a belt for a week is raided two or three times and a
/// crew that keeps moving seldom is — a raid comes only while the ship
/// is holding, and one that falls due under way waits for the next hold.
pub const RAID_GAP_MIN: u64 = (time::DAY as u64) / (time::MINUTE as u64);
pub const RAID_GAP_SPREAD: u64 = 2 * RAID_GAP_MIN;

/// How fast a raider closes on the ship, in world units a game minute,
/// once it is on the radar: it appears at the edge of the ship's range
/// (`World::detection_range`) and comes straight in, so the warning a
/// crew gets is that range over this — a minute with nobody's eyes but
/// their own ([`VISION_RANGE`]), half an hour with one sensor array
/// ([`RADAR_RANGE_PER_SENSOR`]), two hours with the most
/// ([`RADAR_RANGE_MAX`]). Far faster than any ship the crew can fly: a
/// raider is a fast hull, and a raid is not something to outrun.
pub const RAIDER_SPEED: f64 = 50_000.0;

/// How many tiles across a raider's hull is — `crate::station::Plan::Raider`:
/// the playtest ship's size, a port in its west skin, a bunk a boarder,
/// a reactor and a shelf. Small, since it is boarded from rather than
/// walked.
pub const RAIDER_SIDE: u32 = 20;

/// How many stacks of each good an enemy's shelf is found holding, at the
/// most — `crate::plunder`: one to this many, rolled a good at a time off
/// the station's own seed, each a full `economy::stack_size` of the good.
/// So a raider's or an enemy station's shelf holds ten to thirty ore, one
/// to three medkits, and so on for everything its kind stocks — a haul
/// worth the fight, and nothing a friendly desk would not sell.
pub const PLUNDER_STACKS_MAX: u32 = 3;

// --- the droids (feature 83) ---------------------------------------------

/// How many machines a wave of an infested station is, before anything
/// about the crew is counted (`crate::droid::wave_size`): this many, one
/// a crew member, one every [`ENEMIES_DAYS`] the game has run, one for
/// every half of the crew's starting worth their worth has grown by, and
/// one for every three levels the crew have between them.
pub const DROID_WAVE_BASE: u32 = 2;
/// The most a wave ever is. **A performance limit, not a balance one**:
/// every machine is a body stepped, a stand scored and a line traced, and
/// past this a wave costs more of a frame than the fight is worth. See
/// the measurements in the root `CLAUDE.md`.
pub const DROID_WAVE_MAX: u32 = 16;
/// How many waves an infested station has, before the crew are counted
/// (`crate::droid::wave_count`): this many, one every [`ENEMIES_DAYS`],
/// one for every two halves the worth has grown by, and one for every ten
/// levels the crew have between them. Fixed at the crew's **first dock**
/// and never worked out again.
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
// already run. Nothing is rolled per tick, nothing accumulates, and two
// clients that agree about the day and the graph agree about the whole
// galaxy.

/// The day the origin used to turn, before the crisis was there from the
/// start (feature 102). **No longer read**: a run opens with the origin
/// already the machines' (`World::start`), and the probes move the day
/// with `World::set_crisis_first_day_for_probe`. Step three of the
/// roguelike redesign deletes it.
pub const DROID_FIRST_DAY: u32 = 10;
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
