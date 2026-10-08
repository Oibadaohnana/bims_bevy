//! The save file: a game written out, and read back.
//!
//! A game is its [`World`] — the clock, the ship, the room aboard, the
//! stations and everything the crew own — and the four numbers the session
//! round it was opened with: how many players, which this one is, the
//! galaxy's type and the spawn the lobby gave. That is all that goes in.
//! Nothing the app keeps is saved — which panel is open, where the camera
//! is, what is being aimed at — because none of it is the game, and a
//! load stands those up afresh the way an open does ([`Game::resume`]).
//!
//! The text is RON, one value: [`encode`] writes it and [`decode`] reads
//! it, and every type in the world derives serde under the `serde`
//! feature the rules crates carry for this. Where the file goes and what
//! it is called are the app's (`crates/app/src/save.rs`), like every other
//! path. A file from another build is refused by its `version` rather than
//! read wrong: bump [`SAVE_VERSION`] whenever a saved type changes shape.

use world::World;

use crate::Session;
use crate::game::Game;

/// Bumped whenever a saved type changes shape, so an old file is told
/// apart from a broken one. 2: feature 54 grew `Surface` (its biome and
/// population), `Station` (its population), the room's bay (pace and
/// outdoor), its layout (the fields) and its sight (the daylight). 4:
/// the room grew whose bunk is whose (`sleeps_in`) and a Bim the deck's
/// clocks (`ground_left`, `ground_window`, `sore`, `bed`). 5: the
/// workbench grew its slots (`World::bench` for `upgrade`) and the room
/// the carries to it (`ferries` and their three reports). 6: the world
/// grew the raids (`World::raids`, `lost`) and a Bim whether it hunts.
/// 7: the world grew the enemies' shelves laid out as loot
/// (`World::plunder`). 8: a Bim's `selected` became a mask, one bit a
/// player, and the room grew how many of the crew are players' own
/// (`players`) — feature 59, the wire. 9: the crew's names, as the players
/// gave them (`Session::crew_names`) — feature 60. 10: a Bim's look
/// became a yoke, a hair, a shade and a build (`character::Look`) —
/// feature 62. 11: the research grew its queue (`Research::queue`) —
/// feature 64. 12: a docked raid grew whether the ship's airlock has
/// given (`Raid::Docked::breached`) — feature 68. 13: a chain on the queue
/// grew whether it is an order waiting its turn (`Saved::ordered`) and a
/// queued walk (`Kind::Walk`) — feature 69. 14: a station's key became a
/// tier (`World::station_keys`, a `u8` a station), the cargo grew the
/// tier-two key and the research tree the upgrades node — tier two. 15:
/// the world grew what every station has lost (`World::losses`) and every
/// system left as it was left (`World::memories`) — feature 71. 16: the
/// world grew the players' classes and the crew's progress through them
/// (`World::classes`, `progress`, `undocked_once`), the deployables
/// (`World::deployables`, `reused_kits`), the residents' experience
/// flags, the workbench a repair, a Bim its trigger and a room its
/// sentries — feature 74. 17: the cargo grew the grenade, a Bim its brace
/// and its *rampage* stacks, a bolt and a hit whose they were, a room
/// its grenades, the world when each crew member last threw
/// (`World::last_throw`, one entry of `charge_timers` since 27) and the
/// residents who last hit each of them —
/// feature 75. 18: the world grew the medics (`World::medics` — each
/// beam's patients, the surge's charge, the field surgery), a Bim its
/// beam flag and its surge, and a treatment whether it was bare —
/// feature 76. 21: the world grew the machines (`World::infested` —
/// which stations they hold, the waves left, when the next is due — and
/// the tier, the reinforcement clock and the wave cap the probes move),
/// a room its `droids` and a `combat::Hit` its roll and its strip —
/// feature 83. 23: the world grew the dead lying on the stations'
/// decks (`World::graves` — whose deck, where on it, what is still on
/// the body and what it looked like) and the nodes the ship has been at
/// (`World::visited`), both of them filed with the system's memory as
/// well, and a station's room which of its bodies were laid out from a
/// grave (`Residents::grave`) — feature 85. 27: the kits' cooldowns and
/// the soldier's last throw became **one** list of charge timers
/// (`World::charge_timers`, one entry a `class::Charge`), since a
/// grenade is a charge on a cooldown like a kit — feature 90. 28: the
/// crisis (`World::droid_origin` — the star the machines began at — and
/// the day the first star turns, with the machines' hold on a system
/// filed alongside the rest of its memory) — feature 92. The hop table
/// from the origin is **not** in the file: it is derived from the galaxy
/// and the origin, and `Game::resume` works it out again
/// (`World::settle_crisis`) on every read. 29: the jammer (feature 93) —
/// what tier the machines come at is a *reading* now, off how far the
/// system is from the origin, and `World::droid_tier` is an
/// `Option<Tier>` holding the probes' override alone. The machines' own
/// **derived jammer station** is not in the file either: it is rolled off
/// the star's own stream, and `settle_crisis` lays it again
/// (`World::settle_jammer`) on every read. 32: the medicine is two more
/// charges (`class::Charge::Medkit` and `Bandage`, everybody's), so
/// every crew member's entry of `World::charge_timers` is five long.
/// 33: a run (feature 102) — the world's four switches (`needs_enabled`,
/// `human_foes_enabled`, `radiation_enabled`, `shipyard_enabled`), the
/// room's `needs_enabled`, and a station's person's round on its `Bim`
/// with a `lingering` flag on its character.
/// 34: the run's loop (feature 103) — `World::run` (the phase, the
/// mission clock, the pending bounty, the vote, the departure check, the
/// fallen), the droid and town clocks in steps of the mission clock, and
/// the room's `clock_runs`.
/// 35: the old game deleted (feature 104) — `ShipState` is docked or
/// holding and nothing else, the ship keeps no destination and no pending
/// trip, `World::health` (the radiation dose) and the cold store's
/// spoiling clock are gone, and so are the needs' and the dose's switches
/// and the run's free clock; and every human enemy with them — the
/// raids, an enemy's shelf, the hostile list and the arena's
/// reinforcements (on the world and in every system's memory), the human
/// foes' switch, and the room's execution; and the room's needs — a Bim's
/// needs, clocks and poisoning, the galley's, the heads', the bays' and the
/// cold store's state, the timetable, the manager, the deck's mess (blood
/// alone is kept), every errand that served a need and the two flags.
/// 36: the Guardian (feature 100) — a machine keeps a facing vector, the
/// part of a turn owed and where its Sweeper is in its rhythm, a target a
/// shield, and the world the probes' forced wave of machines.
/// 37: relics and unlocks (feature 106) — the run keeps its relics, a
/// held site its cache and a Bim its shots fired; the world keeps no
/// research and no station its key.
/// 38: the Machine Heart (feature 108) — a held station keeps the Heart's
/// fight, a machine what the world told it of the Heart and a body its
/// one health, a shot and a sweep their pace, and the run its summary.
/// 39: the Manufacturers (feature 109) — a held site keeps whose it is, a
/// station's room which of their waves it has laid, and a Bim whether it
/// is one of them.
/// 40: a system's memory keeps its own defences and held towns (feature
/// 111's first fix) — a town id is only its system's, and carried across
/// a jump the list held the next system's town of the same id.
/// 41: procedural stations and towns (feature 112) — a station keeps its
/// gates (a town's, in the order its waves take them) and may be on the
/// generated plan.
/// 42: nothing stored (task 113) — the world keeps its holdings (the
/// armory, the keys, the offers) where it kept the hold's pieces, guns,
/// grids, workbench and craft targets, and a Bim's gear its charges where
/// it kept a pack; the room keeps no weapons on the deck.
/// 43: the trader (task 114) — the run keeps every trader met (its shelf
/// left, its relic) and the vote on the relic, and may be at one
/// (`Phase::Trade`); a station's shelf holds no gear.
/// 44: the minigun and the rail lance (task 115) — two more resources, so
/// a design's cargo is twenty long, and a bolt in the air keeps the bodies
/// a lance slug has struck.
/// 45: the arc greaves and the Reflective plate (task 116) — two more
/// kinds of armour and two more resources, so a design's cargo is
/// twenty-two long; a room remembers who wears a whole plate and a Bim
/// its greaves' cooldown, and a burst may be a discharge.
/// 46: every site an attack, a defence or a trader (task 111) — a
/// station's room knows which of its people are defenders, and the world
/// whether the tests' quiet dial is on.
/// 47: worldgen's `GENERATOR_VERSION` 8 — six hundred stars, every system a
/// station and a planet to land on — and a trader in one system in ten: a
/// save names stars and stations of a galaxy no longer generated.
/// 48: the relics of tasks 117 and 118 — twenty-five more relics, a relic
/// choice without a tier, and the run keeps the new relics' timed effects,
/// cooldowns, marks and notes and whether the trader was restocked.
/// 49: one speed (task 119) — `world::Speed` is paused or 1× and nothing
/// else, so a save with a player at 3×, 10×, 24× or the top speed names a
/// speed that no longer exists.
/// 50: the health rework (task 120) — a body is one bar and a downed
/// countdown where it was parts, blood, wounds and traumas; the medicine
/// charges, the drug lab and the medic's field surgery went, and a Bim
/// lost its fear and its sealing in.
/// 51: the soldier's ranked kit (task 124) — a crew member's progress
/// keeps the ranks bought, the world keeps each soldier's Rampage and Weak
/// Spot's crit stream, and the soldier's talents are gone.
/// 52: the engineer's ranked kit (task 127) — every class charge a counter
/// on the world (`World::charges_held`), the engineer's ultimate's cooldown
/// (`World::engineers`), a deployable's end, a machine's stun, and the
/// sandbag and sentry kits and the grenade gone from the resources.
/// 53: the commander's ranked kit (task 129) — a commander keeps his
/// Battle Cry and whom his cry and rally reached, the world the
/// reinforcements of the mission and a Bim whether it is gone from the
/// deck, a squad's attack marks one enemy, and his talents are gone.
/// 54: the medic's ranked kit (task 130) — a medic keeps his last Nanite
/// Burst and Cloak where the surge's charge was, the world every crew
/// member's cloak, a `Skill` the share a revive gets up at, and the
/// medic's talents and the surge are gone.
/// 55: no cap on a wave (task 132) — the world's `droid_wave_max` went.
/// 56: the ready check — the run keeps its switch, whether the mission is
/// held for it and who has pressed *Ready*.
/// 57: no memories (task 134) — a Bim keeps no diary.
/// 58: one fight a system (task 135) — the run keeps the site chosen in
/// each system.
/// 59: elites — the world keeps the site a probe made an elite.
/// 60: the quickselect (task 138) — a Bim keeps what is in its hands,
/// the figure the medkit it draws, and a skill whether it may lock a door.
/// 61: a machine has one health (task 137) — its body keeps the four
/// parts added together, which every hit comes off.
/// 62: a death after a down is worth no experience — a station's residents
/// no longer keep who has been paid for dying (`xp_dead`).
/// 63: a machine keeps whether it came as a reinforcement that goes
/// looking for the crew (`Droid::seeking`).
/// 64: the engineer's sentry stands until it is destroyed — a laid
/// deployable keeps no end (`Deployable::expires` gone).
/// 65: the tank is a ranked kit (task 139) — the talents picked are gone
/// from `Progress`, a tank keeps his Taunt's and Juggernaut's windows (task 155 replaced them),
/// and `Skill` lost *iron frame* and *breacher*.
/// 66: the commander's squad orders removed — the world keeps no squad
/// order, and `Skill` lost *focus fire*'s `marked_accuracy`.
/// 67: the world keeps the run's difficulty, the game setup's pick
/// (`World::difficulty`).
/// 68: individual money — every player's `World::wallets`, a trader a
/// player (`Trader::owner`), a hired hand's signer (`Hired::by`), and
/// the run's trader-relic vote gone.
/// 69: `World::first_mission_uneased`, the `end` command's switch.
/// 70: `HeartFight::links_down`, the conduits answered with a wave.
/// 71: a Guardian's plate breaks (`Droid::plate_taken`, `plate_age`) and
/// what shields stopped waits for the world (`Combat::plate_hits`).
/// 72: `Bim::under_fire`, how long since an enemy hit a body — a bot
/// under fire fights and takes up no revive.
/// 73: `Commander::last_reinforcement`, the commander's R called in on
/// its cooldown.
/// 74: `Droid::under_fire` — a machine shot at knows where from, and
/// goes for the shooter.
/// 75: the commander's C is the Medivac — `Commander::last_medivac`,
/// `Reinforcement::medic`, `Bim::medivac` — where it was an aura.
/// 76: the reward's own picks (task 146) — `RelicChoice::picks`, `won`,
/// `round`.
/// 77: a player's keys and pointer on its Bim (task 144) —
/// `Character::steer`.
/// 78: a click of the fire button owed its shot (task 144) —
/// `Character::trigger_owed`.
/// 79: one armour slot (October 2026) — `Gear::armour` where `head`,
/// `body` and `legs` were, `ArmourKind::Armour` alone, no part on a
/// `Hit`, `GearSlot::{Weapon, Armour}`, `Character::bleeding`.
/// 80: a smaller galaxy (240 stars, not 600) — no shape moved, but a save
/// may name a star the galaxy no longer has.
/// 81: the electricity gone — the part codes closed up, three layers
/// where there were four, `Ship::charge` and `Lamp::powered` gone.
/// 82: the game setup's difficulty carries the early ease and its days
/// (`world::droid::Difficulty::{early_ease, early_days}`).
/// 83: the wave formula replaced (task 147) — `world::droid::Difficulty`
/// is the whole `WaveScaling` (per player, the day's scaling and its days,
/// per defender, the waves' days and the three tier timings), and
/// `World::first_mission_uneased` is gone.
/// 84: `WaveScaling::enemies_per_defender` is `enemies_per_bot` (the
/// old name still reads), and the crew's bots count towards it.
/// 85: items (October 2026) — `Gear::items` (four `bims::module::Module`
/// slots), `Item::Module`, `Health::max`, `GearSlot::Item1..Item4`,
/// `Run::items` (the blink's cooldowns and when each player was last
/// hit), `Skill::unyielding`, and a trader's shelf rolled every visit.
/// 86: `Trader::items_sold`, the items bought this visit.
/// 87: items, step two — nine more `ModuleKind`s, `ItemClocks::{arc_hits,
/// shell_until}` and `Skill::unstrippable`.
/// 88: the sprint and the dodge roll (task 150) — `Steer::sprint`, and
/// `Character::{last_walk, roll, roll_cool}`.
/// 89: `Bim::level_health`, the hit points a player's level puts on its
/// bar (ten a level).
/// 90: light and dark (task 152) — `Sight::light` (a byte a tile where
/// `lit` was a flag), `Sight::{night, lamps_off}`, `Lamp::off`,
/// `Plane::night`, `Residents::{night, dark}` and the probes' two dials.
/// 91: the relics rebuilt (October 2026) — twelve new ones the crew's,
/// not a Bim's (`Relics::held` one list, the pool, the pending, the
/// timed effects and the rest gone), a choice voted on with no recipient
/// and no picks, no cache on an `Infestation` and no relic on a
/// `Trader`.
/// 92: the soldier's E is a Stun Shot (October 2026) —
/// `Soldier::{charging, last_shot}` and `Grenade::shot`.
/// 93: the medic reworked (task 153) — `Medic` keeps its last drone, its
/// drone in the air and its circle where its last burst and cloak were,
/// and `World::cloaks` went.
/// 94: the engineer reworked (task 154) — `DeployKind::{Mine, Satchel}`
/// where `Sandbags` was, `Charge::{Mine, Satchel}` where `Emp` and
/// `Sandbag` were, `PendingThrow::satchel` where its `emp` was,
/// `Grenade::{laid, satchel}` and `Game::satchels_landed`.
/// 95: the tank reworked (task 155) — `Tank` keeps his Riot Shield, his
/// Reflect Barrier, his Bastion and its haste where his taunt and
/// Juggernaut were, `Bim::bulwark` went, a `Shield` drains
/// (`Shield::drain`) and a `Shot`, a `Bolt` and a `Sweep` keep who took
/// them, `Combat` its plates where its bulwarks were.
/// 96: a broken Riot Shield waits a cooldown — `Tank::shield_broke` (the
/// minute it broke) where `shield_broken` was.
/// 97: the research keys went — `Holdings::keys` with them.
/// 98: a Bim keeps how long a Stun Shot stunned it for (`Bim::stunned`).
/// 99: a Guardian keeps how long until its next grenade (`Droid::grenade_wait`),
/// and a grenade or a shot says whether it is a Guardian's (`Grenade::hostile`,
/// `Grenade::unseen`, `Shot::grenade`).
/// 100: an item keeps what its owner paid for it (`Module::paid`, half of
/// it back on a sale); `Command::Combine` and `WorldEvent::Combined` went.
/// 101: the run keeps the best tier of gun and of armour each player has
/// bought off a shelf (`Run::shelf_bought`), which lifts its later shelves.
/// 102: a trigger keeps its magazine (`Trigger::{spent, reloading, loaded}`)
/// and a weapon its size (`WeaponStats::{magazine, reload_time}`).
/// 103: the run keeps the best tier bought of each kind off a shelf, not
/// of guns and armour (`Run::shelf_bought`).
/// 104: the run keeps whether its map is the floor (`Run::floor`).
/// 105: the mercenaries gone — `World::hired` is `World::field_medics`
/// (crew indices), `least_mercenaries`, `Residents::{fee, medic}`,
/// `Losses::mercenaries` and `Grave::hired` gone.
/// 106: a thing in the armory keeps whose it is (`Stored::owner`).
/// 107: the world keeps the step each player's attack order lapses at
/// (`World::standing_until`).
/// 108: an Area defend (`world::defense::Area` on `Defense::area`), the
/// mission's fixed wave size (`Run::wave_size`), the residents' room's
/// objectives (`bims::game::Game::objectives`) and the tests' dial
/// `World::area_defense_off`.
/// 109: an Area defend's waves on a clock (`defense::Area::gap` gone) and
/// an enemy's volley on its way into the ring (`Droid::volley`,
/// `Bim::volley`).
/// 110: the tier-two machines (task 157) — a machine's rhythms
/// (`bims::droid::Rhythm`: a Bomber's bomb, a Lancer's rail, a Conductor's
/// link, mark, blink and strike call), a grenade's and a shot's `bomb`, and
/// the scaling's `bomber_every` and `lancer_every`.
/// 111: six item slots — `Gear::items` six wide, `GearSlot::{Item5,
/// Item6}`.
/// 112: armour regenerates — a body's seconds since its last hit
/// (`bims::bim::Bim::unhurt`).
/// 113: a site's experience (`Run::{xp_each, clean_xp, clean_spoiled}`),
/// its bonus wave (`Infestation::bonus`, `Defense::bonus`), *Clean Sweep*
/// and the *Training Log*.
/// 114: the bonus wave chosen before the fight — `Run::bonus`, where it
/// was `Infestation::bonus` and `Defense::bonus`.
/// 115: items' step three — the smoke clouds, tethers, decoys' ghosts and
/// adrenal rushes on `ItemClocks` (`smoke`, `tethers`, `ghosts`,
/// `adrenal_until`).
/// 116: a site's money budget — what an enemy of the wave pays
/// (`Run::money_each`).
/// 117: six more relics (codes 13–18) and a bolt's near and far factors
/// (`bims::combat::{Skill, Bolt}::{near_damage, far_damage, near, far}`).
/// 118: how a Bim went over the room's last step (`bims::bim::Bim::moving`),
/// what a machine's bolt is led by.
/// 119: a Husk that has struck one down hunts on (`bims::droid::Droid::blooded`).
/// 120: an elite fight's dials in the scaling (`tier2_guardians`,
/// `tier3_guardians`, `tier2_elites`, `tier3_elites`).
/// 121: the scaling is four areas (`world::droid::Area`, the run's
/// difficulty with it) in place of the tier timings and the dials by the
/// day, and `World::machines_forced` where `defense_by_machines_forced`
/// was.
/// 122: the run's ascension (`World::ascension`).
/// 123: a station's crates and fuel drums (`bims::sight::Prop`, the
/// layout's `props`, a drum's burst on `Grenade::barrel`) and the ways in
/// welded (`world::Run::welded`).
/// 124: Seal the breaches (`world::defense::Breaches` on a `Defense`) and
/// the welding under way (`world::Run::weld_work`).
/// 125: a Sabotage on an `Infestation` (`world::droid::Sabotage`).
/// 126: an Evacuation on a `Defense` (`world::defense::Evacuation`).
/// 127: a nest hunt's nests on an `Infestation` (`world::droid::Nests`).
/// 128: the second set of attacks (`world::objective::Objective` on an
/// `Infestation`), a station laid out for its mission (`Station::fitted`)
/// and a cell's sealed door (`bims::door::Door::sealed`).
/// 129: the defences' missions (`world::objective::Guard` on a `Defense`)
/// and a vault's doors (`stationgen::Fitted::doors`).
/// 131: the Overseer's tile away from the crew (`objective::Overseer::away`).
/// 132: Protect the commander's room and post (`world::objective::Chief`
/// lost its round), and `stationgen::Feature::Command`.
/// 133: a world read back steps as the one written, mid-fight
/// (`tests_resync.rs`): the room keeps a body's fall-back walk, its aim,
/// its Stun Shot's mark and the rest of what a step leaves for the next,
/// the plain's windows (`nav::Maps::afield`) and their blockers, what the
/// world said between steps (standings, whereabouts, revivable visitors,
/// the home, the mark, the revives owed), the bolts' flare, the sentries'
/// facing; the world its probes' wave count and the tuning dials.
/// 134: a mission's enemy budget (`world::run::Budget` on the `Run`,
/// `world::droid::Area::budget`) and an unpaid body
/// (`bims::droid::Droid::unpaid`, `bims::bim::Bim::unpaid`).
pub const SAVE_VERSION: u32 = 134;

/// What the file holds, read back.
#[derive(serde::Deserialize)]
pub struct Save {
    pub version: u32,
    pub players: u32,
    pub local: u32,
    /// The galaxy type's code, as the lobby gave it — `Session::galaxy`.
    pub galaxy: u32,
    /// Where the game began — `Session::spawn`.
    pub spawn: Option<(u32, u32)>,
    /// The crew's names in slot order — `Session::crew_names`. Empty at a
    /// slot is the app's default; a file from before the names is refused
    /// by its version, so the field is not optional.
    pub crew_names: Vec<String>,
    pub world: World,
}

/// The same, borrowed for writing: a world is not `Clone`. The order of
/// the fields is the order they are written in, and the first two are
/// read off the front by hand (`version_of`, `players_of`): keep
/// `version` first and `players` right behind it.
#[derive(serde::Serialize)]
struct Written<'a> {
    version: u32,
    players: u32,
    local: u32,
    galaxy: u32,
    spawn: Option<(u32, u32)>,
    crew_names: &'a [String],
    world: &'a World,
}

/// The version alone, off the front of the text: `encode` writes it
/// first, as `(version:N,`, and it is read by hand rather than through a
/// struct that ignores the rest — RON tells a struct from a tuple by
/// scanning ahead to the matching bracket, and ignoring a world that way
/// is quadratic in its size: minutes for a save. `None` for text that
/// does not start so, which the full read then refuses in its own words.
fn version_of(text: &str) -> Option<u32> {
    let (version, _) = number_after(text, "(version:")?;
    Some(version)
}

/// How many players the game was saved for, read off the front the same
/// way: `players` is written right after the version, as
/// `(version:N,players:M,`, so a host can tell whether a file fits the
/// room before it reads the world (feature 67). `None` for text that
/// does not start so.
pub fn players_of(text: &str) -> Option<u32> {
    let (_, rest) = number_after(text, "(version:")?;
    let (players, _) = number_after(rest, ",players:")?;
    Some(players)
}

/// The digits after `head` at the front of `text`, and what follows them.
fn number_after<'a>(text: &'a str, head: &str) -> Option<(u32, &'a str)> {
    let rest = text.strip_prefix(head)?;
    let end = rest
        .find(|c: char| !c.is_ascii_digit())
        .unwrap_or(rest.len());
    let number = rest[..end].parse().ok()?;
    Some((number, &rest[end..]))
}

/// Why a file could not be read.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum LoadError {
    /// Saved by a build with a different [`SAVE_VERSION`].
    Version(u32),
    /// Not a save at all, or one cut short: what the parser said.
    Syntax(String),
}

/// The game as text. `None` with no world.
pub fn encode(session: &Session) -> Option<String> {
    let game = session.game.as_ref()?;
    let written = Written {
        version: SAVE_VERSION,
        players: session.players,
        local: session.local,
        galaxy: session.galaxy,
        spawn: session.spawn,
        crew_names: &session.crew_names,
        world: &game.world,
    };
    ron::to_string(&written).ok()
}

/// The text read back.
pub fn decode(text: &str) -> Result<Save, LoadError> {
    if let Some(version) = version_of(text)
        && version != SAVE_VERSION
    {
        return Err(LoadError::Version(version));
    }
    ron::from_str(text).map_err(|e| LoadError::Syntax(e.to_string()))
}

/// A player's profile as text (feature 106, `world::relic::Profile`):
/// what the app keeps between runs as `bims/profile.ron`. Here beside the
/// save because this crate is the one that speaks RON.
pub fn profile_text(profile: &world::Profile) -> Option<String> {
    ron::ser::to_string_pretty(profile, ron::ser::PrettyConfig::default()).ok()
}

/// A profile read back, or `None` for text that is not one. Codes a build
/// does not know are left in it and ignored.
pub fn read_profile(text: &str) -> Option<world::Profile> {
    ron::from_str(text).ok()
}

impl Session {
    /// The game, written out — [`encode`].
    pub fn save(&self) -> Option<String> {
        encode(self)
    }

    /// A session stood up round a saved game: the world as it was, and
    /// the game round it as at an open ([`Game::resume`]) — as the player
    /// the file says it was saved by.
    pub fn restore(text: &str, width: f32, height: f32) -> Result<Session, LoadError> {
        let save = decode(text)?;
        let local = save.local;
        Ok(Self::stood_up(save, local, width, height))
    }

    /// [`Session::restore`], but as the player in slot `local` rather than
    /// the one the file was saved by: a guest handed the host's world
    /// over the wire comes up as *its* crew member (feature 67). A slot
    /// the crew has not got is the last one, as `Game::resume` clamps it.
    pub fn restore_as(
        text: &str,
        local: u32,
        width: f32,
        height: f32,
    ) -> Result<Session, LoadError> {
        let save = decode(text)?;
        Ok(Self::stood_up(save, local, width, height))
    }

    fn stood_up(save: Save, local: u32, width: f32, height: f32) -> Session {
        let Save {
            players,
            galaxy,
            spawn,
            world,
            crew_names,
            ..
        } = save;
        let local = local.min(players.saturating_sub(1));
        let seed = world.galaxy_seed;
        let game = Game::resume(world, local, width, height);
        let mut session = Session::resumed(players.max(1), local, game, seed, galaxy, spawn);
        session.crew_names = crew_names;
        session
    }
}
