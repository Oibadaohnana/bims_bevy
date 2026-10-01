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
/// One sixtieth of a real second at 1x, which is the room's step exactly —
/// and 1x is the only speed there is (task 119), so a minute of the clock
/// is a real second.
pub const STEP_MINUTES: f64 = time::MINUTES_PER_SECOND / 60.0;

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

/// What a trip across a hyperlane puts the world clock on by, in game
/// minutes: **one day**, however far the site lies from where the jump
/// lands (the map rework). A trip between two sites of one system costs
/// nothing (`World::travel_quote`), so the jumps are the whole of what
/// moves the world clock, and the machines grow by the run day
/// (`crate::droid::WaveScaling`). It was the hyperdrive's twenty-minute charge
/// plus the leg flown in the system, never under a day, until then.
pub const JUMP_MINUTES: u64 = time::DAY as u64;

/// How many hyperlanes one trip may cross (the second map rework): a star two lanes
/// off is one trip, through the star between, and costs a
/// [`JUMP_MINUTES`] a lane. It was one lane a trip until then.
pub const MAX_TRIP_HOPS: u32 = 2;

/// How far from everything in a system a jump lands, in world units. Four
/// times a body's arrival radius, so the ship is in empty space and not on
/// the doorstep of whatever it happens to be nearest — a trip from there
/// is a trip. `crate::jump::landing_point` is what uses it.
pub const JUMP_CLEARANCE: f64 = 4.0 * flight::data::ARRIVAL_RADIUS_BODY;

/// How far a crew member may stand from a thing and still reach it, in
/// tiles: a mercenary it hires, a relic cache on a research desk, a
/// deployable it packs up. What those are refused beyond
/// (`Refusal::OutOfReach`).
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

/// How the machines scale (task 147): these and nothing else, the
/// defaults of `crate::droid::WaveScaling` (the app's `scaling.ron` and the
/// game setup's Difficulty tune them). Every one reads the **run day**,
/// one on the day the world opens. Machines a **player** Bim brings to a
/// wave — never the bots, the worth or the levels.
pub const ENEMIES_PER_PLAYER: u32 = 2;
/// How much [`ENEMIES_PER_PLAYER`] grows every [`SCALING_DAYS`] (the "y").
pub const DAY_SCALING: u32 = 1;
/// The days of one step of [`DAY_SCALING`] (the "x").
pub const SCALING_DAYS: u32 = 5;
/// Machines each bot brings — the crew's bots and a defence's
/// defenders — the product rounded up.
pub const ENEMIES_PER_BOT: f32 = 1.0;
/// Every this many days a site has one wave more, one to begin with.
pub const WAVE_DAYS: u32 = 10;
/// The day every Manufacturer carries tier-one gear, a gun and armour;
/// before it that share of them (`day / TIER1_DAYS`), the rest the laser
/// pistol alone.
pub const TIER1_DAYS: u32 = 5;
/// The day every enemy is tier two at the least — the machines and the
/// Manufacturers' gear alike; before it that share of them.
pub const TIER2_DAYS: u32 = 20;
/// The same for tier three.
pub const TIER3_DAYS: u32 = 40;
/// How many waves the Machine Heart's fortress has: the old count of a
/// tier-three site, whatever the day — the fight the run is won by is
/// the hardest there is.
pub const HEART_WAVES: u32 = 4;
/// How long after the last machine of a wave is destroyed the next one
/// arrives, in steps of the **mission clock** (feature 103) — thirty
/// seconds of it at 1× (it was two minutes), which was half an
/// hour of the old world clock. Never while one is still standing. The world clock stands still during a mission
/// (only travel moves it), so a wave is timed from the arrival like every
/// other in-mission timer, and not by the day.
pub const DROID_REINFORCE_STEPS: u64 = 1_800;
/// How far beyond a town's wall the machines' lander sets down, in
/// tiles: far enough that its own picture does not overlap the gate it
/// unloaded through.
pub const DROID_LANDER_TILES: f64 = 7.0;

// --- the Manufacturers (feature 109, `crate::manufacturer`) -----------------
//
// The human faction that built the machines and defends what it made. A
// site of theirs is a station they hold from the start of a run: a
// garrison of their people with the machines beside them until they lose
// control of the machines, and waves of their own people after. The whole
// schedule is here.

/// A site of the galaxy is theirs with odds of this many in a hundred —
/// any orbital station but the crew's home and the machines' own derived
/// ones, rolled once a site off the galaxy's seed. Never a town.
pub const MANUFACTURER_SITE_CHANCE: u32 = 10;
/// And at least this many of theirs within [`MANUFACTURER_NEAR_HOPS`]
/// lanes of the crew's own star, none in it: the crew have somebody to
/// fight from the first day. Made up out of the eligible stations there
/// where the roll gave fewer.
pub const MANUFACTURER_NEAR_SITES: usize = 2;
pub const MANUFACTURER_NEAR_HOPS: u16 = 2;
/// The day they **lose the machines**: before it a site of theirs is a
/// fixed garrison with Troopers fighting beside them and no reinforcement;
/// from it on their own people alone, in waves, geared by the machines'
/// tier rules.
pub const MANUFACTURER_DROIDS_LOST_DAY: u32 = 10;
/// The share of a garrison that is a **Trooper** rather than one of their
/// people, by day: each row from its day on, until the next row's. Each
/// body of the garrison is rolled on its own against it.
pub const MANUFACTURER_TROOPER_PERCENT: [(u32, u32); 6] =
    [(0, 0), (5, 10), (7, 25), (8, 50), (9, 60), (10, 0)];
/// How long after a wave of theirs is down the next docks, in steps of
/// the mission clock: thirty seconds at 1×, the machines' own
/// ([`DROID_REINFORCE_STEPS`]) — it was four hours, twice theirs, until
/// every station's waves were brought to half a minute apart.
pub const MANUFACTURER_REINFORCE_STEPS: u64 = 1_800;

// --- the Machine Heart (feature 108, `crate::heart`) ------------------------
//
// The run is won at the crisis's origin: a fortress there holds the core,
// sealed while its conduits stand, then sweeping the Guardian's beam at the
// crew while its fabricators build, and in its overload sweeping two. The
// numbers are placeholders, like every other number here, set so the core
// outlasts a tier-three Warden many times over and the fight is a siege
// rather than a skirmish.

/// How many conduits seal the core before the players are counted.
pub const HEART_CONDUITS_BASE: u32 = 3;
/// And one more for every **player** Bim — never a bot — so a crew of four
/// has seven rooms to fight through before the core can be touched.
pub const HEART_CONDUITS_PER_PLAYER: u32 = 1;
/// What a conduit takes to bring down: a little more than a tier-three
/// Warden's chassis (`bims::balance::WARDEN_BODY` × 1.5²), since it neither
/// dodges nor shoots back.
pub const HEART_CONDUIT_HEALTH: f32 = 300.0;
/// What the core takes to destroy, before the players are counted.
pub const HEART_CORE_HEALTH_BASE: f32 = 1_500.0;
/// And this much more for every **player** Bim.
pub const HEART_CORE_HEALTH_PER_PLAYER: f32 = 500.0;
/// What the core's beams do, as a multiple of a tier-three Guardian's
/// Sweeper (`bims::balance::SWEEPER`): a beam that has to be dodged, not
/// stood in.
pub const HEART_BEAM_DAMAGE_FACTOR: f32 = 1.5;
/// How many fabricators stand round the core.
pub const HEART_FABRICATORS: u32 = 2;
/// What a fabricator takes to bring down.
pub const HEART_FABRICATOR_HEALTH: f32 = 400.0;
/// How often each fabricator still standing builds a tier-three machine
/// once the core is exposed, in steps of the **mission clock**: thirty
/// seconds of it at 1×.
pub const HEART_FABRICATOR_INTERVAL: u64 = 1_800;
/// The share of the core's health under which it goes into its
/// **overload**: two beams at once, faster sweeps and faster building.
pub const HEART_OVERLOAD_FRACTION: f32 = 0.35;
/// How many times faster the core's beams sweep in its overload.
pub const HEART_OVERLOAD_SWEEP_FACTOR: f32 = 2.0;
/// How many times as often the fabricators build in the core's overload.
pub const HEART_OVERLOAD_SPAWN_FACTOR: u64 = 2;

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
/// How much a hop nearer the front adds to a desk's lean on a weapon or a
/// piece of armour, in per cent of the book. The
/// system on the edge of the infection pays `FRONT_BIAS * FRONT_HOPS`
/// — fifteen per cent — over the roll, and the one three hops out five.
pub const FRONT_BIAS: i32 = 5;

// --- defending a site (features 94 and 111) --------------------------------

/// How long after the crew arrive at a site the machines are coming for
/// the first wave lands, in steps of the **mission clock** (feature 103):
/// twenty seconds of it at 1× (task 111; it was a minute, 3 600, while
/// only a town on the front was ever defended) — the prep time, to get
/// the crew to where they want to stand before the shooting starts.
pub const DEFENSE_DELAY_STEPS: u64 = 1_200;
/// How long after the last machine of a wave at a site the crew defend
/// is destroyed the next one lands, in steps of the mission clock: ten
/// seconds of it at 1×, where an attacked station's waves are
/// [`DROID_REINFORCE_STEPS`] (thirty) apart. The probes' reinforcement
/// dial still shortens it.
pub const DEFENSE_REINFORCE_STEPS: u64 = 600;
/// How many of a defended town's surviving people join the crew when the
/// last wave is destroyed: two, not more and not less, whatever the
/// town's size — fewer only when fewer than two are left besides the
/// guard. A station or a derelict held sends nobody (task 111).
pub const DEFENSE_JOINERS: u32 = 2;

// --- defend missions (task 111) -------------------------------------------
//
// Placeholders, all three: how many armed **defenders** stand with a
// site's own people while the machines come for it. They are the site's
// and never the crew's — no fee, no hire, no loss if they fall — and
// they count towards the wave size as crew would
// (`World::droid_wave_size`).

/// Defenders at a site the machines come for on day nought.
pub const DEFENDERS_BASE: u32 = 2;
/// One defender more every this many days of the world clock.
pub const DEFENDER_DAYS: u32 = DROID_SPREAD_DAYS;
/// And never more than this many.
pub const DEFENDERS_MAX: u32 = 8;

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
/// How much of [`REPUBLIC_BOUNTY`] a **defence** pays, in per cent. Task
/// 136 made it nothing — the survivors were the reward — and the player
/// then asked for money for every enemy downed or destroyed, wherever:
/// a hundred, the same as an attack, pending until the site is cleared
/// like any. Tuned in the app's `rewards.ron` (`crate::rewards`).
pub const DEFENSE_BOUNTY_PERCENT: u32 = 100;

/// How far an enemy's bounty strays from its tier's own, in per cent, by
/// how strong it is: the weaker of a tier this much less, the stronger
/// this much more (`world::droid_bounty_percent`,
/// `manufacturer_bounty_percent`).
pub const BOUNTY_SPREAD_PERCENT: u32 = 10;

// --- the run: death and buyback (feature 103) ------------------------------

/// What the pool pays to bring a dead player's Bim back, at the end of
/// the mission it died in (task 113): it respawns aboard the ship with its
/// whole loadout, its level, experience and talents kept. Paid
/// automatically; a pool that holds less goes to nought, and the respawn
/// does not wait for money. The same as a player's share of the starting
/// pool ([`START_MONEY_PER_BIM`]).
pub const BUYBACK_COST: Money = 5_000;

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
/// *Steady Grip*: what its fire rate is raised by, in per cent (it was its
/// odds of hitting).
pub const STEADY_GRIP_FIRE_RATE_PERCENT: i32 = 10;
/// *Trauma Kit* (task 120): how many seconds sooner its carrier brings a
/// downed crewmate round — ten to eight, a medic's four to two — never
/// under [`REVIVE_FLOOR_SECONDS`].
pub const TRAUMA_KIT_REVIVE_SECONDS: f32 = 2.0;
/// The shortest a revive ever takes, in seconds, whatever speeds it.
pub const REVIVE_FLOOR_SECONDS: f32 = 1.0;
/// *Second Wind*: how long after going down it gets up again, in seconds
/// of the mission clock — the first time in a mission.
pub const SECOND_WIND_SECONDS: f64 = 5.0;
/// *Second Wind*: the share of its bar it gets up with, in per cent — a
/// revive of its own (task 120), slowed for the mission like any.
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
/// a machine it hit last is destroyed (three since task 118; one before).
pub const KILL_RELAY_SECONDS: f64 = 3.0;
/// *Phase Harness*: the share of its health a hit has to take it under, in
/// per cent — once a mission.
pub const PHASE_HARNESS_BELOW_PERCENT: u32 = 25;
/// *Phase Harness*: how long nothing hurts it after, in seconds — the
/// room's own surge (`bims::game::Game::set_surge`), halo and all.
pub const PHASE_HARNESS_SECONDS: f32 = 2.0;

// --- task 118's relics: five patches of five --------------------------------
// Placeholder numbers, not balanced. A "second" is one of the mission clock,
// a real second at 1×; a "tile" is the room's.

/// *Servo Cutter*: what a hit on a machine's arms or legs, while it has
/// them, does more, in per cent.
pub const SERVO_CUTTER_DAMAGE_PERCENT: i32 = 25;
/// *Crippler's Mark*: what a hit on a machine missing its arms or its legs
/// does more, in per cent (task 142; it was twenty).
pub const CRIPPLERS_MARK_DAMAGE_PERCENT: i32 = 35;
/// *Parts Broker*: what the bounty for a machine it destroys missing its
/// arms or its legs is raised by, in per cent.
pub const PARTS_BROKER_BOUNTY_PERCENT: i32 = 50;
/// *Total Teardown*: what a hit on a limb already gone does more to the
/// chassis it tears into, in per cent — a hundred is double.
pub const TOTAL_TEARDOWN_DAMAGE_PERCENT: i32 = 100;
/// *Pressure Seal*: the health it puts back a second, all the time it is
/// alive.
pub const PRESSURE_SEAL_HP_PER_SECOND: f32 = 0.5;
/// *Quick Wrap*: the health every revive it makes puts back into the body
/// revived, on top of the three tenths a revive gives (task 120; it was
/// every dressing).
pub const QUICK_WRAP_HEAL: f32 = 10.0;
/// *Clot Booster*: the health it puts back a second for so many seconds
/// after going down — the part of them it is revived for (task 120).
pub const CLOT_BOOSTER_HP_PER_SECOND: f32 = 2.0;
pub const CLOT_BOOSTER_SECONDS: f64 = 15.0;
/// *Tether Field*: how much less of every hit a crewmate it revived takes,
/// in per cent, and for how many seconds (task 120; it was one it
/// dressed. Task 142; it was 25 per cent for six seconds).
pub const TETHER_FIELD_PERCENT: i32 = 40;
pub const TETHER_FIELD_SECONDS: f64 = 10.0;
/// *Lifeline* (task 142): once a mission, when a player's Bim within so
/// many tiles of its holder — or the holder itself — falls under
/// [`LIFELINE_BELOW_PERCENT`] of its health, the holder and that Bim each
/// get a shield of [`LIFELINE_SHIELD_HP`] hit points for
/// [`LIFELINE_SECONDS`]. A bot never sets it off. (It was both
/// untouchable for three seconds when a crewmate within four tiles went
/// down.)
pub const LIFELINE_TILES: f32 = 5.0;
pub const LIFELINE_BELOW_PERCENT: u32 = 25;
pub const LIFELINE_SHIELD_HP: f32 = 300.0;
pub const LIFELINE_SECONDS: f32 = 10.0;
/// *Blind Spot*: what a hit on a machine from the side or behind does
/// more, in per cent.
pub const BLIND_SPOT_DAMAGE_PERCENT: i32 = 15;
/// *Sprint Coil*: its pace raised, in per cent, for how many seconds, at a
/// mission's start and after every ability used.
pub const SPRINT_COIL_SPEED_PERCENT: i32 = 20;
pub const SPRINT_COIL_SECONDS: f64 = 3.0;
/// *Signal Scrambler*: how long no machine aims at it after it destroys
/// one from the side or behind, in seconds, and how long before it can
/// again.
pub const SIGNAL_SCRAMBLER_SECONDS: f64 = 5.0;
pub const SIGNAL_SCRAMBLER_COOLDOWN: f64 = 20.0;
/// *Wide Angle Optics* (task 142): the tiles added to its weapon's range
/// while it stands still (`bims::combat::Skill::still_range`). It was a
/// machine's front narrowed to thirty degrees a side for its hits.
pub const WIDE_ANGLE_OPTICS_TILES: f32 = 7.0;
/// *Crossfire*: what it and the crewmate opposite do more to the machine
/// between them, in per cent; the cosine two bearings from the machine
/// must be under to be opposite (a hundred and twenty degrees); and how
/// near the machine each must stand, in tiles.
pub const CROSSFIRE_DAMAGE_PERCENT: i32 = 40;
pub const CROSSFIRE_APART_COS: f32 = -0.5;
pub const CROSSFIRE_TILES: f32 = 12.0;
/// *Field Radio*: what the fire rate of a crewmate within so many tiles of
/// it is raised by, in per cent (task 142; it was five within three; and it
/// was the odds of hitting).
pub const FIELD_RADIO_FIRE_RATE_PERCENT: i32 = 7;
pub const FIELD_RADIO_TILES: f32 = 5.0;
/// *Spotter*: what the machine it hit last takes more from every crewmate,
/// in per cent, and for how many seconds after the hit.
pub const SPOTTER_DAMAGE_PERCENT: i32 = 10;
pub const SPOTTER_SECONDS: f64 = 3.0;
/// *Squad Morale*: the seconds every class cooldown running on it loses
/// when a machine is destroyed — by anybody (task 142; it was its own
/// kills and the bots').
pub const SQUAD_MORALE_SECONDS: f64 = 1.0;
/// *Cover Formation*: how much less of every hit the crew within so many
/// tiles of it — bots, players and the holder itself — take, in per cent
/// (task 142; it was the bots alone, twenty within three).
pub const COVER_FORMATION_PERCENT: i32 = 15;
pub const COVER_FORMATION_TILES: f32 = 5.0;
/// *Rally Point*: how near a crewmate down has to be, in tiles, and the
/// share of its health it gets up with, in per cent — once a mission, on
/// an ability used.
pub const RALLY_POINT_TILES: f32 = 4.0;
pub const RALLY_POINT_HEALTH_PERCENT: u32 = 20;
/// *Hazard Pay*: what its holder is paid every site cleared, shared by the
/// players — [`HAZARD_PAY`] over their count, into the holder's own
/// wallet (task 142).
pub const HAZARD_PAY: Money = 500;
/// *Trade License*: how much less a trader asks of every player, in per
/// cent, and of its holder (task 142).
pub const TRADE_LICENSE_PERCENT: i32 = 15;
pub const TRADE_LICENSE_HOLDER_PERCENT: i32 = 20;
/// *Strong Will* (task 142, in *Scrap Collector*'s place): how much longer
/// its holder's abilities last, in per cent — a Rampage, an EMP's stun, a
/// Cloak, a Taunt, a Juggernaut, a Rally and a Battle Cry.
pub const STRONG_WILL_PERCENT: i32 = 20;
/// *War Chest*: its damage up this many per cent for every thousand in
/// the pool a player, and never more than the cap.
pub const WAR_CHEST_PERCENT_PER_THOUSAND: i32 = 2;
pub const WAR_CHEST_CAP_PERCENT: i32 = 20;

/// The odds of each relic tier, in per cent, on day nought of the world
/// clock: tier one, two, three (task 117, `relic::tier_odds`). A reward,
/// a cache and a trader draw by the same odds. The tier is what the odds
/// and a trader's price read, and the player is never shown it.
pub const RELIC_ODDS_START: [u32; 3] = [70, 25, 5];
/// The odds from [`RELIC_ODDS_FULL_DAY`] on. They move in a straight line
/// from [`RELIC_ODDS_START`] to these and then stay.
pub const RELIC_ODDS_END: [u32; 3] = [40, 35, 25];
/// The day of the world clock the odds reach [`RELIC_ODDS_END`].
pub const RELIC_ODDS_FULL_DAY: u32 = 30;
/// How many relics a site cleared with machines in it offers.
pub const RELIC_OFFER: usize = 3;
/// The odds a site the machines hold hides a **relic cache** on its
/// research desk, in per cent: rolled once a site, off the galaxy's seed,
/// when the machines take it.
pub const RELIC_CACHE_CHANCE: u32 = 40;
/// How many relics a won run unlocks in each player's profile: the first
/// still locked, in list order.
pub const RELICS_UNLOCKED_PER_WIN: usize = 2;

// --- the trader (task 114, `crate::trader`) ---------------------------------

/// A star's system has a **trader** with odds of this many in a hundred,
/// rolled once a system off the galaxy's seed: one in ten systems. Its
/// trader is the lowest-numbered station with a desk that is not the
/// crew's home, not the jammer's own station, not the Manufacturers' and
/// not one of the machines' derived ones (`trader::pick`). Never a town.
/// It was twenty in a hundred a *station*, which with every system given a
/// station (worldgen's `GENERATOR_VERSION` 8) was a trader in half of them.
pub const TRADER_SYSTEM_CHANCE: u32 = 10;
/// The odds a star's system holds an **elite**, in per cent, off the
/// galaxy's seed (`crate::elite`): its station the machines' from the
/// first day, at least [`ELITE_WAVES`] waves, a Guardian in the second —
/// and the only fights that drop relics. Never the crew's own system.
pub const ELITE_SYSTEM_CHANCE: u32 = 10;
/// The fewest waves an elite's machines come in.
pub const ELITE_WAVES: u32 = 2;
/// The wave an elite's Guardian comes in (the first is wave one).
pub const ELITE_GUARDIAN_WAVE: u32 = 2;
/// And at least this many traders within [`TRADER_NEAR_HOPS`] lanes of the
/// crew's own star, their own system counted: somewhere to buy a gun
/// before the first fight has paid for one. Made up out of the systems
/// there where the roll gave fewer.
pub const TRADER_NEAR_SITES: usize = 1;
pub const TRADER_NEAR_HOPS: u16 = 1;
/// How many weapons a trader's shelf holds: one (October 2026; it was
/// four), any kind made at the day's tier, rolled again every visit.
pub const TRADER_WEAPONS: usize = 1;
/// How many pieces of armour it holds beside it: one (it was three).
pub const TRADER_ARMOUR: usize = 1;
/// What an item costs at a trader (October 2026, `crate::items`), by its
/// kind's code and its tier, one to three: the *Blink Drive*, the
/// *Executioner*, the *Reactor Heart* and the *Override Core* (made at
/// tier one alone, so its row is one price), then step two's nine (October
/// 2026): the Coolant Loop, the Pressure Seal, the Steady Grip, the Long
/// Barrel, the Leech Capacitor, the Arc Coil, the Field Mender, the Reset
/// Capacitor and the Ablative Shell. Four and a half times the first
/// placeholders (the player's +200%, then +50%).
pub const ITEM_PRICE: [[Money; 3]; 13] = [
    [9_000, 15_750, 27_000],
    [11_250, 20_250, 36_000],
    [9_000, 18_000, 31_500],
    [27_000, 27_000, 27_000],
    [6_750, 13_500, 22_500],
    [4_500, 9_000, 15_750],
    [6_750, 13_500, 22_500],
    [6_750, 13_500, 22_500],
    [9_000, 18_000, 31_500],
    [11_250, 20_250, 36_000],
    [9_000, 18_000, 31_500],
    [18_000, 31_500, 54_000],
    [11_250, 20_250, 36_000],
];
/// What the relic at a trader costs, by the relic's own tier: one, two,
/// three (task 117). The tier is never shown, so the price is the only
/// sign of it.
pub const RELIC_PRICE: [Money; 3] = [1_500, 3_000, 5_000];
/// What combining two things of a kind and a tier into one of the next
/// costs, out of the pool (the workbench's upgrade, at a trader now). A
/// placeholder, and nothing yet.
pub const COMBINE_FEE: Money = 0;
