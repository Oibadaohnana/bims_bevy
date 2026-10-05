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

/// How many hops a floor is from its start to the Machine Heart (the floor,
/// October 2026; `crate::floor`): a row a hop, the Heart the last row.
/// Row `r` is fought on run day `r` (the start and the first row share
/// day one), so the rows are the days. Fifty until the player halved the
/// tier-one rows (October 2026): the 36 of `scaling.ron`'s tier timings
/// (tier two from day 37, three from 47) became 18, the Heart 18 rows
/// nearer.
pub const FLOOR_HOPS: u32 = 32;
/// The fewest and the most separate ways up a floor: every row between
/// the start and the Heart has this many places, at the least and at the
/// most, and the leftmost and the rightmost are two ways that share none.
pub const FLOOR_MIN_WAYS: u32 = 2;
pub const FLOOR_MAX_WAYS: u32 = 4;
/// How many traders a floor has, scattered over its places (`floor::shape`):
/// one in each of this many even stretches of the rows from
/// [`FLOOR_FIRST_SHOP_ROW`] to the one under the Heart, each where it
/// reaches the place longest stranded without one — a trader within seven
/// rows of nearly every place (97%), one every six or seven rows on a way
/// that goes for them, never a whole row of them. Four whole rows of
/// traders until the player asked for them scattered (October 2026); fifteen
/// until the floor came down from fifty rows to 32, nine keeping the
/// stretch about three rows. **The last of them is always the row under
/// the Heart** (October 2026, the player's: "there should be one shop
/// before the heart as a guarantee"): that row is one place, every way
/// up's last stop, and the other eight are scattered over rows three to
/// twenty-nine (`floor::scatter_shops`).
pub const FLOOR_SHOPS: u32 = 9;
/// The lowest row a trader may be on: the first few are fights.
pub const FLOOR_FIRST_SHOP_ROW: u32 = 3;

/// How far from everything in a system a jump lands, in world units. Four
/// times a body's arrival radius, so the ship is in empty space and not on
/// the doorstep of whatever it happens to be nearest — a trip from there
/// is a trip. `crate::jump::landing_point` is what uses it.
pub const JUMP_CLEARANCE: f64 = 4.0 * flight::data::ARRIVAL_RADIUS_BODY;

/// How far a crew member may stand from a thing and still reach it, in
/// tiles: a body it revives, a research desk, a
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
/// each and two over, a chair each in the hall, a strip
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
/// From the floor's tier-two rows on (task 157), a wave of `n` gets
/// `n / BOMBER_EVERY` Bombers and `n / LANCER_EVERY` Lancers on top of it, at
/// least one of each; nought is none.
pub const BOMBER_EVERY: u32 = 6;
pub const LANCER_EVERY: u32 = 8;
/// How many Guardians the Machine Heart sends for each conduit shot down
/// (October 2026; the player's words: "first 1 guardian, then 2 all the
/// way up to 5 when the last link is destroyed"): this many times the
/// conduit's place in the order they fell — the first one, the second
/// two. The fortress has no waves besides (`heart::guardians_for_link`).
pub const HEART_GUARDIANS_PER_LINK: u32 = 1;
/// How long after the last machine of a wave is destroyed the next one
/// arrives, in steps of the **mission clock** (feature 103) — fifteen
/// seconds of it at 1× (it was thirty, and before that two minutes), which was half an
/// hour of the old world clock. Never while one is still standing. The world clock stands still during a mission
/// (only travel moves it), so a wave is timed from the arrival like every
/// other in-mission timer, and not by the day.
pub const DROID_REINFORCE_STEPS: u64 = 900;
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
/// the mission clock: fifteen seconds at 1×, the machines' own
/// ([`DROID_REINFORCE_STEPS`]) — it was four hours, twice theirs, until
/// every station's waves were brought to half a minute apart, and then to
/// a quarter of one.
pub const MANUFACTURER_REINFORCE_STEPS: u64 = 900;

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
// off that one number: what a desk charges for a gun.

/// How far outside the infection a system still counts as the front, in
/// hops. Three, so a crew have a band of systems to trade in
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
/// five seconds of it at 1× (October 2026, the player's word; it was
/// twenty, 1 200, since task 111, and a minute, 3 600, while only a town
/// on the front was ever defended) — the prep time, to get the crew to
/// where they want to stand before the shooting starts.
pub const DEFENSE_DELAY_STEPS: u64 = 300;
/// How long after the last machine of a wave at a site the crew defend
/// is destroyed the next one lands, in steps of the mission clock: ten
/// seconds of it at 1×, where an attacked station's waves are
/// [`DROID_REINFORCE_STEPS`] (fifteen) apart. The probes' reinforcement
/// dial still shortens it.
pub const DEFENSE_REINFORCE_STEPS: u64 = 600;
/// How many of a defended town's surviving people join the crew when the
/// last wave is destroyed: two, not more and not less, whatever the
/// town's size — fewer only when fewer than two are left besides the
/// guard. A station or a derelict held sends nobody (task 111).
pub const DEFENSE_JOINERS: u32 = 2;

// --- Area defend: a town's defence is holding the FOB (October 2026) --------

/// How long the crew hold the FOB, in steps of the mission clock from the
/// first wave's landing: three minutes at 1×. No wave lands after it;
/// those on the ground are still to be destroyed.
pub const AREA_HOLD_STEPS: u64 = 10_800;
/// How long the enemies stand in the ring with nobody of the crew's side
/// in it to take the FOB: twenty seconds. Any friend in the ring stops
/// the count; no enemy in it puts it back to nought.
pub const AREA_CAPTURE_STEPS: u64 = 1_200;
/// How long after an Area defend's first wave lands the second does,
/// while the hold runs: thirty-one seconds, counted from the landing
/// whether or not the wave is down — they stack. On a clock rather than
/// after the last is down, or the crew could dodge one machine in the
/// middle and let the time run out (the player's word).
pub const AREA_WAVE_STEPS: u64 = 1_860;
/// The first this many waves are followed [`AREA_WAVE_STEPS`] after; each
/// after them [`AREA_WAVE_SOONER_STEPS`] (a second) sooner than the one
/// before, never more often than [`AREA_WAVE_MIN_STEPS`] (five
/// seconds): 31, 30, 29 … — seven waves in the three minutes.
pub const AREA_STEADY_WAVES: u32 = 1;
pub const AREA_WAVE_SOONER_STEPS: u64 = 60;
pub const AREA_WAVE_MIN_STEPS: u64 = 300;
/// How long after the crew arrive an Area defend's first wave lands: five
/// seconds (a station's defence keeps [`DEFENSE_DELAY_STEPS`]). The crew
/// are stood in the ring already.
pub const AREA_PREP_STEPS: u64 = 300;
/// The ring's radius, in tiles: what counts as standing in the FOB. The
/// sandbags lie just inside it.
pub const AREA_RADIUS_TILES: f64 = 5.0;
/// How far from the middle the sandbags lie, in tiles: a tile's width of
/// them from here out, inside the ring.
pub const AREA_BAGS_FROM_TILES: f64 = 3.5;
/// Where the defenders hold, in tiles from the middle: a tile behind the
/// bags, within their cover's reach.
pub const AREA_HOLD_RING_TILES: f32 = 2.5;
/// What standing in the ring heals, in per cent of the body's own bar a
/// second: the crew — players and bots — and the site's defenders and
/// guard, on their feet, while the hold's waves come (the player's word).
/// Two until the player asked for half a per cent.
pub const AREA_HEAL_PERCENT: f32 = 0.5;

// --- defend missions (task 111)-------------------------------------------
//
// Placeholders, all three: how many armed **defenders** stand with a
// site's own people while the machines come for it. They are the site's
// and never the crew's — no loss if they fall — and
// they count towards the wave size as crew would
// (`World::droid_wave_size`).

/// Defenders at a site the machines come for on day nought.
pub const DEFENDERS_BASE: u32 = 2;
/// One defender more every this many days of the world clock.
pub const DEFENDER_DAYS: u32 = DROID_SPREAD_DAYS;
/// And never more than this many.
pub const DEFENDERS_MAX: u32 = 8;

/// What the Republic pays for a site on the run's first day, in euros
/// (October 2026, `Rewards::site_money`): a **budget**, as a site's
/// experience is, each enemy down paying its wave's share of it
/// (`World::money_per_down`) — so the money no longer moves with the
/// waves' size, the players' number or a bonus wave's bodies.
///
/// **Fighting is how a crew earn.** Nothing is mined and nothing is made,
/// so the only money that comes into the game comes off a body — and a
/// crew that never fights never gets rich. It was a bounty an enemy by its
/// tier until then (500, 1 500, 4 500, then 425, 1 275, 3 825); the budget
/// is fitted to what those paid a lone player fighting every row:
/// €836 a site on day one, €79 000 on day thirty.
pub const SITE_MONEY: Money = 850;
/// How much more a site pays every day after, in per cent, compounded
/// (`Rewards::site_money_growth_percent`): €850 on day one, €3 490 on day
/// ten, €16 790 on day twenty, €80 690 on day thirty.
pub const SITE_MONEY_GROWTH_PERCENT: u32 = 17;
/// How much of an enemy's share a **defence** pays, in per cent. Task
/// 136 made it nothing — the survivors were the reward — and the player
/// then asked for money for every enemy downed or destroyed, wherever:
/// a hundred, the same as an attack, pending until the site is cleared
/// like any. Tuned in the app's `rewards.ron` (`crate::rewards`).
pub const DEFENSE_BOUNTY_PERCENT: u32 = 100;
/// How much of an enemy's bounty is paid when one of the crew's **bots**
/// took it down, in per cent (the player's, October 2026: "if a bot kills
/// an enemy you should only be rewarded 50% of the gold", then 20%, then
/// "If a bot kills an enemy -> 5% money, if Player kills +10%", then,
/// once the money was a site's budget, "make bot kills pay 50% share",
/// then "bot kills should count the full amount" — the experience is
/// untouched). A bot is a crew member past the players
/// — a bot, a field medic, a townsperson who joined — and not one of a
/// commander's reinforcements or his Medivac's medic, which pay the
/// whole share (the player's: "Reinforcments and medivac from the
/// commander should pay the full share"; they paid as their commander's
/// own kill until then). A sentry's kill and an enemy no crew member hit
/// last pay the whole. Tuned in the app's `rewards.ron`
/// (`crate::rewards`).
pub const BOT_BOUNTY_PERCENT: u32 = 100;
/// How much of an enemy's bounty is paid when a player's own Bim took it
/// down, in per cent: ten more.
pub const PLAYER_BOUNTY_PERCENT: u32 = 110;
/// What an **elite fight** pays an enemy down, in per cent of the bounty
/// anywhere else (October 2026, the player's: "if you fight an elite get
/// more money", beside the elite's experience): the system's elite or an
/// Area defend (`World::is_elite_fight`), on top of the relics', the
/// defence's and who took it down's shares (`World::bounty_here`). Half
/// as much again since the money became a site's budget (October 2026;
/// twice until then): with four elites and a bonus wave at every fight
/// a lone player came to €1.04M at the Heart, the player's "too much".
pub const ELITE_BOUNTY_PERCENT: u32 = 150;

/// How far an enemy's bounty strays from its tier's own, in per cent, by
/// how strong it is: the weaker of a tier this much less, the stronger
/// this much more (`world::droid_bounty_percent`,
/// `manufacturer_bounty_percent`).
pub const BOUNTY_SPREAD_PERCENT: u32 = 10;

// --- experience: a site's budget (October 2026) -----------------------------
//
// A site is worth so much experience to every player, whatever its waves
// are — how big, how many, how many players and bots they were laid for —
// and each enemy down pays its wave's share of it (`World::xp_per_down`).
// So the waves can be tuned for the fight without moving the levels. A
// run that fights every row of the floor but the four or so traders a way
// up meets and takes nothing more reaches the fifteenth level at the
// Heart; the elites, the bonus waves, *Clean Sweep* and the *Training Log*
// are what take a crew to the sixteenth and past.

/// What a site is worth on the run's first day, in experience to every
/// player: a cleared site's whole (`Rewards::site_xp`).
pub const SITE_XP: u32 = 60;
/// How much more a site is worth every day after, in per cent,
/// compounded (`Rewards::site_xp_growth_percent`): 60 on day one, 166 on
/// day ten, 516 on day twenty, 1 797 on day thirty-one — about a level
/// every two days, the level curve growing by 1.3 a level.
pub const SITE_XP_GROWTH_PERCENT: u32 = 12;
/// What an **elite**'s site is worth, in per cent of an attack's.
pub const ELITE_XP_PERCENT: u32 = 200;
/// What the Machine Heart's fight pays an enemy down, as its day's site
/// budget over this many: its machines come for its conduits and from its
/// fabricators, never as a wave a share could be worked out over.
pub const HEART_XP_BODIES: u32 = 20;
/// The **bonus wave**: chosen by the crew in the ready check before a
/// fight, one more wave after the site's own, this much bigger than they
/// are, in per cent, rounded up...
pub const BONUS_WAVE_SIZE_PERCENT: u32 = 150;
/// ...worth this much of the site's experience on top, in per cent...
pub const BONUS_WAVE_XP_PERCENT: u32 = 50;
/// ...and this much of its money (October 2026: half, as the
/// experience, until a bonus wave at every fight came to too much; then
/// a quarter; 29 puts a lone player taking four elites and a bonus wave
/// at every fight at €802k at the Heart, the player's "800k when
/// greedy").
pub const BONUS_WAVE_MONEY_PERCENT: u32 = 29;
/// A player below the best level among the players gets this much more
/// of every enemy's experience, in per cent: a player dead and bought
/// back, or joined late, catches up.
pub const CATCH_UP_XP_PERCENT: u32 = 25;

// --- the run: death and buyback (feature 103) ------------------------------

/// What the pool pays to bring a dead player's Bim back, at the end of
/// the mission it died in (task 113): it respawns aboard the ship with its
/// whole loadout, its level, experience and talents kept. Paid
/// automatically; a pool that holds less goes to nought, and the respawn
/// does not wait for money. The same as a player's share of the starting
/// pool ([`START_MONEY_PER_BIM`]).
pub const BUYBACK_COST: Money = 5_000;

// --- relics (feature 106, rebuilt October 2026, `crate::relic`) ------------
// Every relic is the crew's, a boon with a price: each number a share in
// per cent (the regeneration in per cent of the whole bar a second), the
// boon's first.
// Placeholders, not balanced.

/// How many relics an elite's clear offers the crew to choose one of.
pub const RELIC_OFFER: usize = 3;
/// *Glass Cannon*: everybody's weapon damage up, and everybody takes more.
pub const GLASS_CANNON_DAMAGE: i32 = 30;
pub const GLASS_CANNON_TAKEN: i32 = 25;
/// *Heavy Plating*: everybody takes less, and walks slower.
pub const HEAVY_PLATING_TAKEN: i32 = 25;
pub const HEAVY_PLATING_SPEED: i32 = 20;
/// *Hair Trigger*: everybody fires faster, and the class cooldowns are
/// longer.
pub const HAIR_TRIGGER_FIRE_RATE: i32 = 35;
pub const HAIR_TRIGGER_COOLDOWNS: i32 = 30;
/// *Overclocked Cores*: the class cooldowns shorter, and everybody's
/// weapon damage down.
pub const OVERCLOCKED_CORES_COOLDOWNS: i32 = 35;
pub const OVERCLOCKED_CORES_DAMAGE: i32 = 20;
/// *Bounty Contract*: every enemy down pays more, and the machines take
/// less from the crew. It was +100% until October 2026 (the player's word).
pub const BOUNTY_CONTRACT_BOUNTY: i32 = 50;
pub const BOUNTY_CONTRACT_DAMAGE: i32 = 20;
/// *Hunter's Pact*: every enemy down is worth more experience, and every
/// wave has more machines (rounded up).
pub const HUNTERS_PACT_EXPERIENCE: i32 = 50;
pub const HUNTERS_PACT_WAVES: i32 = 25;
/// *Drill Sergeant*: the bots do more and take less, the players do
/// less. (It lifted a bot's kill's pay too, to half and then the whole,
/// until every bot's kill paid the whole share, October 2026.)
pub const DRILL_SERGEANT_BOT_DAMAGE: i32 = 50;
pub const DRILL_SERGEANT_BOT_TAKEN: i32 = 30;
pub const DRILL_SERGEANT_PLAYER_DAMAGE: i32 = 20;
/// *Lone Wolves*: the players do more and walk faster, the bots do less.
pub const LONE_WOLVES_PLAYER_DAMAGE: i32 = 35;
pub const LONE_WOLVES_PLAYER_SPEED: i32 = 15;
pub const LONE_WOLVES_BOT_DAMAGE: i32 = 50;
/// *Black Market*: a trader asks less, and every enemy — a machine or one
/// of the Manufacturers' people — comes with this many per cent more hit
/// points (the player's, October 3rd 2026; it was one more wave at every
/// elite, and before that less bounty).
pub const BLACK_MARKET_PRICES: i32 = 40;
pub const BLACK_MARKET_ENEMY_HEALTH: i32 = 35;
/// *Adrenaline*: everybody walks faster, and takes more.
pub const ADRENALINE_SPEED: i32 = 30;
pub const ADRENALINE_TAKEN: i32 = 15;
/// *Salvage Burn*: the machines take more from the crew, and every enemy
/// down pays less.
pub const SALVAGE_BURN_DAMAGE: i32 = 40;
pub const SALVAGE_BURN_BOUNTY: i32 = 40;
/// *Nanite Mesh*: everybody on its feet gets hit points back a second,
/// and everybody's weapon damage is down.
pub const NANITE_MESH_REGEN: i32 = 2;
pub const NANITE_MESH_DAMAGE: i32 = 15;
/// *Clean Sweep* (October 2026): a site cleared with no player downed
/// since the last clear pays every player this much more of the
/// experience it earned there, in per cent — and everybody takes more.
pub const CLEAN_SWEEP_EXPERIENCE: i32 = 25;
pub const CLEAN_SWEEP_TAKEN: i32 = 10;
/// *Point Blank* (October 2026): every bolt does this many per cent more
/// within `bims::balance::NEAR_TILES` of where it was fired, and this many
/// less past `FAR_TILES` — a blend between ([`crate::relic::Stat::NearDamage`]).
pub const POINT_BLANK_NEAR: i32 = 30;
pub const POINT_BLANK_FAR: i32 = 20;
/// *Marksman's Creed* (October 2026): *Point Blank* turned round — far
/// shots up, near ones down.
pub const MARKSMANS_CREED_FAR: i32 = 30;
pub const MARKSMANS_CREED_NEAR: i32 = 20;
/// *Forked Path* (October 2026): a trip on the floor may go to any place
/// of the row above, and every enemy comes with this many per cent more
/// hit points.
pub const FORKED_PATH_ENEMY_HEALTH: i32 = 15;
/// *War Chest* (October 2026): every site cleared pays each player this
/// many per cent of the money in its own wallet, at most
/// [`WAR_CHEST_CAP_PERCENT`] of what a site pays a player that day
/// (`Rewards::site_money_on`) — and a trader asks this many per cent more.
pub const WAR_CHEST_INTEREST: i32 = 10;
pub const WAR_CHEST_CAP_PERCENT: Money = 50;
pub const WAR_CHEST_PRICES: i32 = 20;
/// *Overtime* (October 2026): the bonus wave pays this many per cent more
/// experience and money (its 50% of the site's experience made the
/// whole), and every site that can have one has it, not to be taken back.
pub const OVERTIME_PAY: i32 = 100;
/// *Giant Slayer* (October 2026): the crew's hits on a big enemy
/// (`crate::relic::is_big`) do this many per cent more, on every other
/// enemy this many less.
pub const GIANT_SLAYER_BIG: i32 = 35;
pub const GIANT_SLAYER_SMALL: i32 = 20;

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
/// first day, at least [`ELITE_WAVES`] waves, Guardians in the second —
/// and the only fights that drop relics. Never the crew's own system.
pub const ELITE_SYSTEM_CHANCE: u32 = 10;
/// How many of the floor's first rows of fights hold no elite (October
/// 2026, the player's: "they shouldn't spawn the first 5 fights"): an
/// elite rolled on rows one to this is a plain fight. The rows above keep
/// every one, tier one included; off the floor nothing changes.
pub const FLOOR_NO_ELITE_ROWS: u32 = 5;
/// The fewest waves an elite's machines come in.
pub const ELITE_WAVES: u32 = 2;
/// The wave an elite's Guardians come in (the first is wave one).
pub const ELITE_GUARDIAN_WAVE: u32 = 2;
/// How many Guardians an elite's [`ELITE_GUARDIAN_WAVE`] holds at least,
/// by the wave's tier (one, two, three): one at tier one, two at tier
/// two, three at tier three (`crate::elite::with_guardian`).
pub const ELITE_GUARDIANS: [u32; 3] = [1, 2, 3];
/// And at least this many traders within [`TRADER_NEAR_HOPS`] lanes of the
/// crew's own star, their own system counted: somewhere to buy a gun
/// before the first fight has paid for one. Made up out of the systems
/// there where the roll gave fewer.
pub const TRADER_NEAR_SITES: usize = 1;
pub const TRADER_NEAR_HOPS: u16 = 1;
/// **A trader every this many hops** (October 2026): from every system of
/// the galaxy — a trader's own included — some *other* system's trader is
/// at most this many lanes away, so a crew leaving a trader always has the
/// next within it. Made up on top of the roll and the near ones where the
/// galaxy left a gap (`crate::trader::near_sites`); a system with nothing
/// eligible within reach (the edge of a galaxy hemmed in by elites and the
/// Manufacturers) is the only one left without.
pub const TRADER_EVERY_HOPS: u16 = 5;
/// How many weapons a trader's shelf holds: every kind but the laser
/// pistol, one of each (October 2026; it was four drawn, then one, then
/// two), each at its own tier (`trader::shelf`). The app reads it to put
/// the shelf's first slots under its Weapons heading.
pub const TRADER_WEAPONS: usize = 6;
/// How many pieces of armour it holds beside them: the one armour (it
/// was three).
pub const TRADER_ARMOUR: usize = 1;
/// What a minigun off a trader's shelf costs, times its tier's ask
/// (October 2026, the player's word, when its magazine made it fire ten a
/// second): the sale back is half of that, as for anything bought.
pub const MINIGUN_SHELF_PRICE: Money = 3;
/// What an item costs at a trader (October 2026, `crate::items`), by its
/// kind's code and its tier, one to three: the *Blink Drive*, the
/// *Executioner*, the *Reactor Heart* and the *Override Core* (made at
/// tier one alone, so its row is one price), then step two's nine (October
/// 2026): the Coolant Loop, the Pressure Seal, the Steady Grip, the Long
/// Barrel, the Leech Capacitor, the Arc Coil, the Field Mender, the Reset
/// Capacitor and the Ablative Shell. Four and a half times the first
/// placeholders (the player's +200%, then +50%), then three times that
/// (October 2026, the player's). The Reset Capacitor is made at tier
/// three alone and costs five times its old tier three (October 2026;
/// 54 000), 270 000, then three times that, 810 000 — and since October
/// 2026 85 000, about the Override Core's (the player's: "reset capacitor
/// should be cheaper and cost around the price of the override core"),
/// its row one price like the Core's, the lower tiers only a saved one's. The *Training Log* (October 2026) last, at
/// the Pressure Seal's: an investment, best bought early. Then step three
/// (October 2026, the player's pick): the *Smoke Launcher* and the
/// *Adrenal Injector* at the Coolant Loop's, the *Targeting Uplink*, the
/// *Tether Link* and the *Overcharger* at the Reactor Heart's, and the
/// *Decoy Projector*, one tier, one price.
pub const ITEM_PRICE: [[Money; 3]; 20] = [
    [27_000, 47_250, 81_000],
    [33_750, 60_750, 108_000],
    [27_000, 54_000, 94_500],
    [81_000, 81_000, 81_000],
    [20_250, 40_500, 67_500],
    [13_500, 27_000, 47_250],
    [20_250, 40_500, 67_500],
    [20_250, 40_500, 67_500],
    [27_000, 54_000, 94_500],
    [33_750, 60_750, 108_000],
    [27_000, 54_000, 94_500],
    [85_000, 85_000, 85_000],
    [33_750, 60_750, 108_000],
    [13_500, 27_000, 47_250],
    [20_250, 40_500, 67_500],
    [27_000, 54_000, 94_500],
    [27_000, 54_000, 94_500],
    [54_000, 54_000, 54_000],
    [20_250, 40_500, 67_500],
    [27_000, 54_000, 94_500],
];
/// What a thing sold back at a trader fetches, in per cent of what was
/// paid for it (October 2026, the player's: half). Nothing is combined
/// any more; an item is upgraded at the next tier's price instead.
pub const SELL_BACK_PERCENT: Money = 50;
