//! Every word on the screen.
//!
//! The simulation knows crew member 0, part kind 7 and event code 14, and
//! nothing else about them: no strings come out of the rules crates, and
//! that is deliberate — a native server will one day run the same crates
//! and it has no words to say. These tables are where the words are, indexed
//! by the codes each crate writes out and never renumbers. Adding a part, an
//! event or a job is a variant there and a name here.

use physics::ResourceId;
use shipdesign::parts::PartKind;
use world::{Refusal, WorldEvent};

/// Which build this is, from `BUILD` at the root: 0.1 up in tenths, one
/// step each time `ship` puts the game on the server.
pub const BUILD: &str = include_str!("../../../BUILD");

/// The corner's line: `build 0.1`.
pub fn build_label() -> String {
    format!("build {}", BUILD.trim())
}

/// Who is aboard, by lobby slot. The room calls crew 0 James.
pub const CREW_NAMES: [&str; 5] = ["James", "Kate", "Priya", "Tomas", "Mateo"];

/// What the players called their crew, in slot order, over the table
/// above (feature 60): set from the session whenever one is opened or
/// loaded (`set_crew_names`), and read by [`crew_name`] wherever a crew
/// member is named — the log, the panels, the name over a head. A slot
/// left blank is the table's. A process-wide cell rather than a resource
/// threaded through forty callers, because the names are words and the
/// words are this file's.
static GIVEN_NAMES: std::sync::RwLock<Vec<String>> = std::sync::RwLock::new(Vec::new());

/// The crew's names as the players gave them, from the session that
/// holds them (`ship::Session::crew_names`). Empty clears them.
pub fn set_crew_names(names: &[String]) {
    let mut given = GIVEN_NAMES.write().unwrap_or_else(|e| e.into_inner());
    given.clear();
    given.extend(names.iter().map(|n| wire::tidy_name(n)));
}

/// A crew member's name: what its player called it, else the table's.
pub fn crew_name(who: u32) -> String {
    if let Some(given) = given_name(who) {
        return given;
    }
    CREW_NAMES
        .get(who as usize)
        .map(|s| s.to_string())
        .unwrap_or_else(|| format!("Crew {}", who + 1))
}

fn given_name(who: u32) -> Option<String> {
    let given = GIVEN_NAMES.read().unwrap_or_else(|e| e.into_inner());
    given.get(who as usize).filter(|n| !n.is_empty()).cloned()
}

/// What the people living on a station are called. The world knows a
/// resident as a station and a seat and nothing else, so the names are
/// dealt out here, by station and seat, off one list: enough that no two
/// on one station share a name, and the same ones every time the ship
/// comes back. Sixty-four because a town on a planet held up to fifty
/// (`world::data::SURFACE_POPULATION`, thirty since feature 66), and
/// the formula below walks the list seat by seat from where the station
/// starts.
pub const RESIDENT_NAMES: [&str; 64] = [
    "Ada", "Tomas", "Priya", "Yusuf", "Mei", "Olu", "Sanne", "Ravi", "Ines", "Kofi", "Hana",
    "Bram", "Leila", "Jonas", "Nour", "Emil", "Amara", "Sofia", "Kenji", "Zara", "Mateo", "Aiko",
    "Femi", "Lena", "Arjun", "Nadia", "Piet", "Yara", "Tariq", "Ingrid", "Diego", "Chioma", "Luca",
    "Maya", "Omar", "Elif", "Sven", "Rosa", "Kwame", "Anya", "Hiro", "Farah", "Niall", "Sita",
    "Bao", "Maren", "Idris", "Lucia", "Teo", "Zanele", "Juno", "Mika", "Esme", "Jamal", "Freya",
    "Andrei", "Suki", "Dara", "Ren", "Amina", "Otto", "Thandi", "Karim", "Wren",
];

pub fn resident_name(station: u32, who: u32) -> String {
    RESIDENT_NAMES[((station * 2 + who) as usize) % RESIDENT_NAMES.len()].to_string()
}

/// What each part is called. Indexed by the `PartKind` discriminant in
/// `crates/shipdesign/src/parts.rs`.
pub const PART_NAMES: [&str; 49] = [
    "Deck plating",
    "Wall",
    "Door",
    "Engine",
    "Bunk",
    "Cold store",
    "Worktop",
    "Hob",
    "Dishwasher",
    "Table",
    "Chair",
    "Toilet",
    "Basin",
    "Hydroponic bay",
    "Broom locker",
    "Structure",
    "Outside wall",
    "Helm",
    "Life support",
    "Airlock",
    "Sensor array",
    "Shelf",
    "Shower",
    "Thruster",
    "Heavy engine",
    "Diagonal wall",
    "Diagonal outside wall",
    "Workbench",
    "Suit locker",
    "Armoury",
    "Trading desk",
    "Sandbags",
    "Research desk",
    "Hyperdrive",
    "Wall light",
    "Standing light",
    "Small plant",
    "Big plant",
    "Picture",
    "Field",
    "Tree",
    "Shrub",
    "Boulder",
    "Water",
    "Railing",
    "Window",
    "Shaft",
    "Crate",
    "Fuel drum",
];

pub fn part_name(kind: PartKind) -> &'static str {
    PART_NAMES
        .get(kind as usize)
        .copied()
        .unwrap_or("Something")
}

/// What a station sells, indexed by `physics::ResourceId`'s code — blank
/// where a resource went (15 to 17, task 127; 6, 8, 20 and 21 when a Bim
/// came to wear one armour, October 2026; 13 and 14 with the research
/// keys).
pub const RESOURCE_NAMES: [&str; 22] = [
    "Vegetables",
    "Tofu",
    "Suits",
    "Handguns",
    "Medkits",
    "Bandages",
    "",
    "Armour",
    "",
    "Shotguns",
    "Auto rifles",
    "Sniper rifles",
    "Schwords",
    // 13 and 14 were the research keys, gone October 2026.
    "",
    "",
    // 15 to 17 were the engineer's kits and the soldier's grenade, gone
    // when a class's charges became counters (task 127).
    "",
    "",
    "",
    "Miniguns",
    "Rail lances",
    "",
    "",
];

pub fn resource_name(id: ResourceId) -> &'static str {
    RESOURCE_NAMES
        .get(id as usize)
        .copied()
        .unwrap_or("Something")
}

/// An equipment tier, by `bims::combat::Tier::code`: what a cell's
/// tooltip and the workbench's events say. "tier 1" for the baseline,
/// which no cell says — see [`tier_word`].
pub fn tier_name(code: u32) -> String {
    format!("tier {code}")
}

/// The word for a tier above one, or none for tier one: what is
/// appended to a tiered item's name in a tooltip.
pub fn tier_word(tier: bims::combat::Tier) -> Option<String> {
    match tier {
        bims::combat::Tier::One => None,
        other => Some(tier_name(other.code())),
    }
}

/// Why an order did nothing.
pub fn refusal(why: Refusal) -> &'static str {
    match why {
        Refusal::NotDocked => "not while the ship is away from a station",
        Refusal::Unaffordable => "there is not the money",
        Refusal::NotAboard => "there is not that much in the armory to sell",
        Refusal::SumTooBig => "the sum will not go",
        Refusal::NotSoldHere => "this station does not sell that",
        Refusal::WontFit => "that will not go there",
        Refusal::NoSuchSite => "that site is not there any more",
        Refusal::OutOfReach => "it is out of reach — walk over first",
        Refusal::NoMarket => "there is nobody here to sell to",
        // A walk ordered on the deck (`Command::Crew`): the room's two
        // refusals.
        Refusal::NoWayThere => "there is no way there at all",
        Refusal::ClassLocked => "a class is chosen before the ship first leaves its berth",
        Refusal::NotAnEngineer => "only an engineer does that",
        Refusal::NoKit => "no charge left: the next is still coming back",
        Refusal::NoSentryYet => "a sentry wants the engineer's ultimate",
        Refusal::CantDeployThere => {
            "that tile will not take it — clear deck floor within reach, not a door, nothing on it"
        }
        Refusal::NoSuchDeployable => "there is nothing of the kind there",
        Refusal::NoClass => "a crew member with no class has nothing to learn",
        Refusal::NotASoldier => "only a soldier does that",
        Refusal::NoGrenade => "no grenade charge left: the next is still coming back",
        Refusal::NoGrenadesYet => "Frag Grenade wants a rank first",
        Refusal::CoolingDown => "that skill is still cooling down",
        Refusal::OutOfThrowRange => "that tile is out of throwing range",
        Refusal::NoLineToTile => "there is a wall or a shut door in the way",
        Refusal::CantThrowThere => "that tile is not deck",
        Refusal::NotAMedic => "only a medic does that",
        Refusal::NoPatient => "there is nobody there to beam",
        Refusal::NotACrewmate => "that is not one of the crew",
        Refusal::OutOfBeamRange => "they are too far off for the beam",
        Refusal::NoSightOfPatient => "the medic cannot see them",
        Refusal::NotATank => "only a tank can do that",
        Refusal::NotACommander => "only a commander can do that",
        Refusal::NoGroundThere => "there is no ground to attack there",
        Refusal::NotCarrying => "only a medic carries somebody, and only one at a time",
        Refusal::NotHurt => "they are on their feet and can walk out themselves",
        Refusal::AlreadyCarried => "somebody has them already",
        Refusal::Jammed => {
            "the machines' jammer holds this system's lanes shut — clear it, or jump outward"
        }
        Refusal::NotEnoughMoney => {
            "there is not enough money left for it — earn some, or call another site off"
        }
        Refusal::NoShipyard => "nothing is built onto the ship on a run",
        Refusal::BetweenMissions => "not between missions — choose where to go next",
        Refusal::MidMission => "the map is read-only during a mission — go back to the ship first",
        Refusal::NoSuchPlace => "there is no such place to go",
        Refusal::TooFar => {
            "that is not a way up from here — only a place joined to this one on the row above"
        }
        Refusal::NoProposal => "nobody has put a destination to the crew",
        Refusal::NotAsked => "nobody is being asked about leaving",
        Refusal::PlayerOut => "your Bim is dead — it is back when the mission ends",
        Refusal::CannotTravel => "the ship cannot get there — nothing pushes it",
        Refusal::AlreadyHere => "the crew are here — the next trip goes somewhere else",
        Refusal::NoRelicChoice => "there is no relic to choose now",
        Refusal::NotOnOffer => "that relic is not on offer",
        Refusal::NotAPlayer => "only a player's own Bim can do that",
        Refusal::ChoosingRelic => "the crew are still choosing a relic",
        Refusal::GearLocked => {
            "gear changes aboard the ship — not out on the deck, and offers between missions"
        }
        Refusal::NotYours => {
            "that is another player's Bim — offer them the thing instead, and they accept it"
        }
        Refusal::NoSuchGear => "that thing is not there any more",
        Refusal::NoOffer => "there is no such offer standing",
        Refusal::TraderClosed => {
            "the trader is closed: the machines have its system until every site of it is cleared"
        }
        Refusal::ClosedOnArrival => {
            "the machines will have the trader's system by the day the crew would get there"
        }
        Refusal::NotAtATrader => "only at a trader",
        Refusal::SoldOut => "that is gone — somebody bought it first",
        Refusal::TopTier => "that item is at its top tier already",
        Refusal::NoRankedKit => "there is no such ability",
        Refusal::NoSkillPoint => "no skill point to spend — the next comes with the next level",
        Refusal::TopRank => "that ability is at its top rank",
        Refusal::RankLocked => "the next rank of that ability wants a higher level",
        Refusal::NotLearnt => "that ability wants a rank first",
        Refusal::AlreadyActive => "it is already running",
        Refusal::AwaitingReady => "not yet — the mission starts when every player is ready",
        Refusal::NoReadyCheck => "the mission is already under way",
        Refusal::FightOver => "the fight is won — press Back to ship",
        Refusal::OtherSiteChosen => "the crew fought this system's other site",
        Refusal::NoSuchItem => "there is no item there to use",
        Refusal::BlinkLocked => "the Blink Drive will not go so soon after a hit",
        Refusal::NowhereToBlink => "there is no ground in sight to blink to",
        Refusal::ItemsFull => "every item slot is full",
        Refusal::BotsCarryNoItems => "only a player's Bim carries items",
        Refusal::NotForSale => "the trader does not sell that",
        Refusal::NoWeaponInHand => "nothing in hand to fire it from",
        Refusal::NoSatchels => "no satchel charge of yours is lying out",
        Refusal::ShieldRecharging => "the shield is broken until its cooldown is over",
        Refusal::NotSellable => "the laser pistol is not for sale",
        Refusal::OutOfItemRange => "they are too far off for that",
        Refusal::NoBonusWave => "a bonus wave is chosen in the ready check, at a fight",
        Refusal::NoWayInNear => "no way in to weld here — stand at an airlock or a gate",
        Refusal::NoChargeNear => "stand at the charge's mark",
        Refusal::ShipCastOff => "the ship has cast off — it waits at the way out",
        Refusal::NoFlagNear => "the flag is not within reach, or somebody else carries it",
        Refusal::NothingToUse => "nothing to use within reach",
        Refusal::BonusWaveHeld => "Overtime keeps the bonus wave on",
    }
}

// --- the wire (feature 59) --------------------------------------------------------

/// What the relay's refusals say — `wire::Refusal`, a code each, the way
/// the world's are. The one with a number in it carries the server's
/// protocol, so the line can say which.
pub fn relay_refusal(why: wire::Refusal) -> String {
    match why {
        wire::Refusal::Protocol { server } => format!(
            "This game speaks protocol {}; the server speaks {server}. One of you is out of date.",
            wire::PROTOCOL
        ),
        wire::Refusal::HelloFirst | wire::Refusal::Greeted => {
            "The server lost its place in the handshake.".into()
        }
        wire::Refusal::NoName => "The server wants a name — set BIMS_NAME.".into(),
        wire::Refusal::InRoom => "Leave the lobby you are in first.".into(),
        wire::Refusal::NoCodes => "The server has no room codes left. Try again shortly.".into(),
        wire::Refusal::NoSuchRoom => "No lobby has that code.".into(),
        wire::Refusal::RoomFull => "That lobby is full.".into(),
        wire::Refusal::Begun => "That game has already begun.".into(),
        wire::Refusal::NotInRoom => "You are not in a lobby.".into(),
        wire::Refusal::NotHost => "Only the host can start the game.".into(),
        wire::Refusal::TooBig => "That was too much to send at once.".into(),
        wire::Refusal::CodeTaken => "A lobby is open at that code already.".into(),
    }
}

/// Why a room closed under you.
pub fn room_closed(why: wire::Closed) -> &'static str {
    match why {
        wire::Closed::HostLeft => "The host left, and the lobby closed with them.",
    }
}

/// While the socket is being opened and the room asked for.
pub const CONNECTING: &str = "Reaching the server…";
/// A join with something that is not six of the code's letters.
pub const NOT_A_CODE: &str = "That is not a room code: six letters or digits.";
/// The connection died; what the socket said follows.
pub const LINK_LOST: &str = "Lost the connection:";
/// The host has gone mid-game: the ship is this player's own from here.
pub const HOST_GONE: &str = "The host has gone. The ship is yours now, and the clock with it.";
/// A guest's copy of the world disagrees with the host's.
pub const DESYNC: &str =
    "Your world has drifted from the host's — what you see may not be what they see.";
/// The guest asked the host for its world (feature 67).
pub const RESYNC_ASKED: &str = "Catching up with the host…";
/// The host's world arrived and took the guest's place.
pub const RESYNC_DONE: &str = "Back on the host's world.";
/// A guest's Load is greyed with this: the host's world is the world.
pub const LOAD_GUEST: &str = "Host only";
/// The Esc sheet's Network buffer slider, under the pointer (task 148).
pub const NET_BUFFER_HINT: &str = "• Plays the host's world a little behind, for smoothness\n• Uses only what the line needs, up to this\n• Guests only";
/// The Esc sheet's Restart (feature 79): the menu's button, the page's
/// line, the button that does it, and the two reasons it is greyed.
pub const RESTART_BUTTON: &str = "Restart";
/// The Esc sheet's way out of a game, back to the start menu.
pub const MENU_BUTTON: &str = "Back to menu";
/// The Esc sheet's pause, and its way back (task 154: Space paused until
/// it became the engineer's remote trigger).
pub const PAUSE_BUTTON: &str = "Pause";
pub const RESUME_BUTTON: &str = "Resume";
pub const RESTART_LINE: &str = "Play this run again from the situation it opened in — the fight, the town, the landing, or the run the lobby started. Everything since is lost, and a saved game is not touched.";
pub const RESTART_AGAIN: &str = "Start again";
pub const RESTART_NONE: &str = "No run yet";
/// A guest's Restart is greyed with this, as its Load is.
pub const RESTART_GUEST: &str = "Host only";
/// The line the log carries after a restart, so the screen says what
/// happened as well as showing it.
pub const RESTART_DONE: &str = "Back at the beginning.";
/// The Esc sheet's Retry mission: the button, why it is greyed, and the
/// log's line after it.
pub const RETRY_BUTTON: &str = "Retry mission";
pub const RETRY_HINT: &str = "• Mission from its start\n• Everything since lost";
pub const RETRY_NONE: &str = "No mission running";
pub const RETRY_GUEST: &str = "Host only";
pub const RETRY_DONE: &str = "Back at the start of the mission.";
/// The host's load does not fit the room.
pub fn load_players(saved: u32, here: u32) -> String {
    format!("That game was saved for {saved} players; {here} are here.")
}
/// The host's world arrived and could not be read.
pub fn world_refused(why: &str) -> String {
    format!("Could not read the host's world: {why}")
}
/// Somebody left the game; their crew member carries on unsteered.
pub fn player_left(name: &str) -> String {
    format!("{name} has left. Their crew member carries on alone.")
}
/// Somebody joined the lobby.
pub fn player_joined(name: &str) -> String {
    format!("{name} joined.")
}
/// Somebody left the lobby; the relay says the roster, not who.
pub const SOMEBODY_LEFT: &str = "Somebody left.";
/// The setup's difficulty: every dial of the wave formula the host picks
/// for the run (`world::droid::Difficulty`, task 147), their notes, and
/// the button that puts them back to the tuning file's (`scaling.ron`).
pub const DIFFICULTY_NOTE: &str = "• The run is four areas, each so many days (a day a row)\n• A wave: (per player + every day's growth so far) × players, + per bot × bots\n• Area 0: Manufacturers only, pistols to full gear\n• Tier 1: machines 0% to 100% by tier 2's door\n• Tier 2 / 3: every enemy that tier from the area's first day, ramping from halfway through the area before";
pub const WAVE_PER_PLAYER: &str = "Enemies per player";
pub const WAVE_PER_PLAYER_NOTE: &str = "On day 1, for each player";
pub const WAVE_PER_BOT: &str = "Enemies per bot";
pub const WAVE_PER_BOT_NOTE: &str =
    "Each bot, and each defender at a defence; rounded up (1.5 × 3 = 5)";
/// The difficulty's table of areas (October 2026): a column an area, a
/// row a dial.
pub const AREA_NAMES: [&str; 4] = ["Area 0", "Tier 1", "Tier 2", "Tier 3"];
pub const AREA_DAYS: &str = "Days";
pub const AREA_DAYS_NOTE: &str =
    "Rows of the map; tier 2's door and the trader before the Heart not counted";
pub const AREA_GROWTH: &str = "Growth / day";
pub const AREA_GROWTH_NOTE: &str = "Enemies per player more each day, carried into the next area";
pub const AREA_WAVES: &str = "Waves";
pub const AREA_WAVES_NOTE: &str = "A site's waves (an elite 2 at least)";
pub const AREA_BOMBERS: &str = "Bombers";
pub const AREA_LANCERS: &str = "Lancers";
pub const AREA_EXTRAS_NOTE: &str = "On top of every wave";
pub const AREA_GUARDIANS: &str = "Guardians";
pub const AREA_GUARDIANS_NOTE: &str = "Per player, in an elite's Guardian wave";
pub const AREA_ELITES: &str = "Elites";
pub const AREA_ELITES_NOTE: &str = "Bombers on top of an elite's Guardian wave";
pub const AREA_DEFENDERS: &str = "Defenders";
pub const AREA_DEFENDERS_NOTE: &str = "Armed defenders at a defence";
/// A cell area 0 has no dial for: it has no machines and no elite.
pub const AREA_NONE: &str = "–";
/// Where the areas fall on the map, for the dials as they stand.
pub fn area_rows_line(tier_one: u32, door: u32, tier_three: u32, heart: u32) -> String {
    let area0 = match tier_one {
        1 => "no area 0".to_string(),
        2 => "area 0 day 1".to_string(),
        n => format!("area 0 days 1–{}", n - 1),
    };
    format!(
        "{area0} · tier 1 days {tier_one}–{} · tier 2 from day {door} (trader) · tier 3 from day {tier_three} · trader day {} · Heart day {heart}",
        door.saturating_sub(1),
        heart.saturating_sub(1),
    )
}
pub const DIFFICULTY_RESET: &str = "Default";
pub const DIFFICULTY_RESET_HOVER: &str = "Back to scaling.ron";
/// The button that writes the dials into `scaling.ron`, and what it says
/// after.
pub const DIFFICULTY_SAVE: &str = "Save as default";
pub const DIFFICULTY_SAVE_HOVER: &str = "• Write into scaling.ron\n• Every new game starts from it";
pub const DIFFICULTY_SAVED: &str = "Saved into scaling.ron.";
pub fn difficulty_not_saved(why: &str) -> String {
    format!("Not saved: {why}")
}
/// The `heart` command's class picker (October 2026).
pub const HEART_CLASS_TITLE: &str = "Choose your class";
pub const HEART_CLASS_NOTE: &str =
    "• Top level, every point to spend\n• Tier-3 kit, the shelf never runs out";
/// What the run's first wave comes to, for the players in the lobby.
pub fn first_wave_line(machines: u32, players: u32) -> String {
    let who = if players == 1 {
        "one player".to_string()
    } else {
        format!("{players} players")
    };
    let m = if machines == 1 { "enemy" } else { "enemies" };
    format!("The first wave: {machines} {m} for {who}")
}
/// The setup's ascensions (October 2026, `world::ascension`): a name and
/// what it adds, nought to `ascension::MOST`, each on top of the ones
/// under it.
pub const ASCENSION: &str = "Ascension";
pub const ASCENSION_NAMES: [&str; 6] = [
    "Standard",
    "Swarming Elites",
    "Tougher Enemies",
    "Aua Making Enemies",
    "More Enemies",
    "One More",
];
/// What each ascension adds, in a line.
pub fn ascension_line(level: u32) -> String {
    use world::ascension as a;
    match level {
        0 => "The run as it plays".into(),
        1 => format!("+{}% chance of an elite fight on the map", a::ELITE_CHANCE),
        2 => format!("Enemies have +{}% hit points", a::ENEMY_HEALTH),
        3 => format!("Enemies deal +{}% damage", a::ENEMY_DAMAGE),
        4 => format!("+{}% enemies at the Heart", a::HEART_ENEMIES),
        _ => format!("+{} wave every mission", a::EXTRA_WAVES),
    }
}
pub fn ascension_name(level: u32) -> &'static str {
    ASCENSION_NAMES
        .get(level as usize)
        .copied()
        .unwrap_or(ASCENSION_NAMES[ASCENSION_NAMES.len() - 1])
}
/// A level not opened yet, and what opens it.
pub fn ascension_locked(level: u32) -> String {
    format!(
        "Win a run at ascension {} to open it",
        level.saturating_sub(1)
    )
}
/// The victory screen's line when a win opened the next ascension.
pub fn ascension_opened(level: u32) -> String {
    format!("Ascension {level} unlocked: {}", ascension_name(level))
}
/// The setup's two tabs: the crew and the run, and the wave formula's
/// dials (`scaling.ron`).
pub const SETUP_TAB_CREW: &str = "Crew";
pub const SETUP_TAB_SCALING: &str = "Scaling";
pub const SETUP_TAB_DEV: &str = "Dev";
/// The Dev tab (October 2026): what a playtest run sets out with. Every
/// player's Bim gets the kit; the host picks.
pub const DEV_NOTE: &str =
    "For playtesting: every player's Bim sets out with this. Nothing here is the run as it plays.";
pub const DEV_RESET: &str = "Reset";
pub const DEV_ON: &str = "DEV";
pub const DEV_MISSION: &str = "Mission";
pub const DEV_MISSION_TIP: &str = "• Rolled: as the game plays\n• A defence's mission at every defence, an attack's at every attack\n• Plain: no mission anywhere";
pub const DEV_ROLLED: &str = "Rolled";
pub const DEV_AS_PLAYED: &str = "as it plays";
pub const DEV_EVERY_DEFENCE: &str = "every defence";
pub const DEV_EVERY_ATTACK: &str = "every attack";
pub const DEV_NO_MISSION: &str = "none";
pub const DEV_LEVEL: &str = "Level";
pub const DEV_LEVEL_TIP: &str =
    "• The class's level at the start\n• Skill points to spend with it\n• No class: nothing";
pub const DEV_WEAPON: &str = "Weapon";
pub const DEV_ARMOUR: &str = "Armour";
pub const DEV_CLASS_KIT: &str = "Class kit";
pub const DEV_TIER: &str = "Tier";
pub const DEV_ITEMS: &str = "Items";
pub const DEV_ITEMS_TIP: &str =
    "• Click to buy into the next free slot, free\n• Click a slot to empty it";
pub const DEV_SLOT_EMPTY: &str = "empty";
pub const DEV_RELICS: &str = "Relics";
pub const DEV_RELICS_TIP: &str = "• The crew hold these from the start";
pub const DEV_ARMORY: &str = "Armory";
pub const DEV_ARMORY_TIP: &str = "• Every weapon and armour at every tier in each player's armory";
pub const DEV_ARMORY_FULL: &str = "Full";
pub const DEV_ARMORY_EMPTY: &str = "As it plays";
/// A mission's name on the Dev tab.
pub fn dev_mission_name(mission: world::run::Mission) -> &'static str {
    match mission {
        world::run::Mission::Plain => "Plain",
        world::run::Mission::Breaches => "Seal the breaches",
        world::run::Mission::Sabotage => "Sabotage",
        world::run::Mission::Evacuation => "Evacuation",
        world::run::Mission::Nests => "Nest hunt",
        world::run::Mission::Overseer => "Kill the Overseer",
        world::run::Mission::Heist => "Data heist",
        world::run::Mission::Prison => "Prison break",
        world::run::Mission::FuelRun => "Fuel run",
        world::run::Mission::Salvage => "Salvage sweep",
        world::run::Mission::Bombs => "Bomb disposal",
        world::run::Mission::Doors => "Hold the doors",
        world::run::Mission::Chief => "Protect the commander",
    }
}
pub const SETUP_TITLE: &str = "New run";
pub const LOBBY_TITLE: &str = "Lobby";
/// The setup's money choice.
pub const MONEY_PER_BIM: &str = "Starting money";
pub const MONEY_PER_BIM_TIP: &str = "• Each Bim's share
• All of it one pool";
/// The setup's auto-shoot (October 2026): each player's own, beside the
/// keys and the trigger, never instead of them.
pub const AUTO_SHOOT: &str = "Auto shoot";
pub const AUTO_SHOOT_NOTE: &str = "• Fires at the nearest enemy in reach, or the one you click\n• WASD and the left button work as ever";
pub const AUTO_SHOOT_OFF: &str = "You aim and fire";
pub const AUTO_SHOOT_ON: &str = "It fires by itself";
/// The setup's name field: what the player calls their crew member.
pub const BIM_NAME: &str = "Your Bim";
pub const BIM_NAME_NOTE: &str = "• Your crew member's name\n• Blank: the crew's own";
/// The field's hint, and what the lobby's roster prints for a Bim not
/// yet named: the crew's own name for that berth.
pub const BIM_NAME_HINT: &str = "a name";
/// The setup's hair chooser: the row's title and its note, and the name
/// of every style and every colour, in `bims::character::Hair::ALL`'s and
/// `Shade::ALL`'s order (feature 62).
pub const BIM_HAIR_NOTE: &str = "Hair";
/// And the colour chooser (feature 84): the ring on the deck that says
/// which of the crew is yours. One a player, so a colour somebody else
/// in the lobby has taken is not offered.
pub const BIM_TINT: &str = "Your colour";
pub const BIM_TINT_NOTE: &str = "• The ring under your own crew member on the deck";
pub const BIM_TINT_TAKEN: &str = "taken";
/// The class chooser (features 74 and 75): a name a `world::Class`, in
/// `Class::ALL`'s order, with a line each saying what it does.
pub const BIM_CLASS: &str = "Class";
pub const BIM_CLASS_NOTE: &str =
    "• One class each, picked before launch\n• Adds four abilities\n• Same jobs and money for all";
pub const CLASS_NAMES: [&str; 6] = ["None", "Engineer", "Soldier", "Medic", "Tank", "Commander"];
pub const CLASS_TIPS: [&str; 6] = [
    "• No abilities",
    "• Q Mine\n• F Healing Sentry\n• E Satchel Charge, G sets them off\n• R Sentry with a minigun\n• Packs up mines and sentries",
    "• Q Frag Grenade\n• F Weak Spot: crits\n• E Stun Shot: 2 s charge, stuns\n• R Rampage: faster fire and reload, less damage taken\n• Starts with an auto rifle",
    "• Q Heal Drone\n• F Triage: stronger heals on the hurt\n• E Heal Beam, heals you too\n• R Healing Circle: heals allies, burns enemies, costs you\n• Override Core: healing ×1.5\n• Revives in 4 s (others 10 s), up at 40% (others 30%)",
    "• Q Riot Shield: bounces shots back\n• F Plated: less damage, regen\n• E Reflect Barrier: hits dealt back\n• R Bastion: 600–1200 shield on allies\n• Armour drains at half rate\n• Starts with a pistol and T1 armour",
    "• Q Battle Cry: allies fire faster\n• F Medivac: a medic who revives players\n• E Rally: less damage taken, faster\n• R Reinforcements: Republic soldiers\n• Starts with a pistol",
];
pub fn class_name(class: world::Class) -> &'static str {
    CLASS_NAMES
        .get(class.code() as usize)
        .copied()
        .unwrap_or("Class")
}
pub fn class_tip(class: world::Class) -> &'static str {
    CLASS_TIPS.get(class.code() as usize).copied().unwrap_or("")
}
/// What a few abilities do, in a line, Dota 2's way — the numbers are the
/// [`Stat`] rows under it. Shared by [`ranked_what`] and anything else
/// that says one. (The two-key tables, `ABILITY_NAMES` and
/// `ABILITY_TIPS`, went with the last class of talents, task 139: every
/// box is a ranked ability's now.)
const MINE_WHAT: &str =
    "• Laid on the tile at the pointer\n• Goes off: enemy within 1 tile\n• Hurts enemies only";
const SATCHEL_WHAT: &str =
    "• Hold: aim · release: throw\n• G sets them all off\n• Hurts enemies only";
/// What the medic's Heal Drone and heal beam do (task 153).
const HEAL_DRONE_WHAT: &str =
    "• Flies over walls to the lowest ally, you too\n• Heals slowly, then the next";
const HEAL_BEAM_WHAT: &str = "• Toggle\n• Ally at the pointer, or yourself\n• You heal as much\n• Keep shooting\n• Rank 4: adds your items' regen\n• Number: links left";
/// The box past the class's four (feature 86): the medic's carry, named
/// off the [`crate::keys::Action`] rather than off a slot.
pub const CARRY: &str = "Carry";
pub const CARRY_TIP: &str = "• Picks up the downed ally at the pointer\n• Slow, no shooting\n• Again: set down\n• Their countdown runs on\n• Number: downed nearby";

/// The four abilities of a ranked kit (task 124), Q C E R, by class and
/// slot: what the box, the log and the Skills tab call each. Empty for a
/// class with no ranked kit.
pub fn ranked_ability(class: world::Class, slot: u8) -> &'static str {
    match (class, slot) {
        (world::Class::Soldier, 0) => "Frag Grenade",
        (world::Class::Soldier, 1) => "Weak Spot",
        (world::Class::Soldier, 2) => "Stun Shot",
        (world::Class::Soldier, 3) => "Rampage",
        (world::Class::Engineer, 0) => "Mine",
        (world::Class::Engineer, 1) => "Healing Sentry",
        (world::Class::Engineer, 2) => "Satchel Charge",
        (world::Class::Engineer, 3) => "Sentry",
        (world::Class::Commander, 0) => "Battle Cry",
        (world::Class::Commander, 1) => "Medivac",
        (world::Class::Commander, 2) => "Rally",
        (world::Class::Commander, 3) => "Reinforcements",
        (world::Class::Medic, 0) => "Heal Drone",
        (world::Class::Medic, 1) => "Triage",
        (world::Class::Medic, 2) => "Heal Beam",
        (world::Class::Medic, 3) => "Healing Circle",
        (world::Class::Tank, 0) => "Riot Shield",
        (world::Class::Tank, 1) => "Plated",
        (world::Class::Tank, 2) => "Reflect Barrier",
        (world::Class::Tank, 3) => "Bastion",
        _ => "",
    }
}
/// What a ranked ability does, in a line, whatever its rank: Dota 2's
/// way, the numbers left to [`ranked_stats`].
pub fn ranked_what(class: world::Class, slot: u8) -> &'static str {
    match (class, slot) {
        (world::Class::Soldier, 0) => {
            "• Bursts after 2 s\n• Hurts allies too\n• Half damage at the edge"
        }
        (world::Class::Soldier, 1) => "• Passive\n• Hits may crit\n• Not grenades",
        (world::Class::Soldier, 2) => {
            "• Charge 2 s: no firing, can walk\n• Fires at the pointer, weapon's reach\n• Burst hurts and stuns"
        }
        (world::Class::Soldier, 3) => {
            "• Ultimate\n• Faster fire and reload, less damage taken\n• Aim on the move\n• Works while a Stun Shot charges"
        }
        (world::Class::Engineer, 0) => MINE_WHAT,
        (world::Class::Engineer, 1) => {
            "• Heals allies in reach and sight\n• A new one replaces your oldest"
        }
        (world::Class::Engineer, 2) => SATCHEL_WHAT,
        (world::Class::Engineer, 3) => {
            "• Ultimate\n• Minigun sentry, shoots what it sees\n• Stands until destroyed\n• Ready every mission"
        }
        (world::Class::Commander, 0) => "• Allies near you fire faster",
        (world::Class::Commander, 1) => {
            "• A Republic medic with a pistol\n• Fights, revives downed players\n• Armour by rank"
        }
        (world::Class::Commander, 2) => "• Allies near you take less damage\n• and move faster",
        (world::Class::Commander, 3) => {
            "• Ultimate\n• Republic soldiers beside you\n• Stay all mission, add up\n• Ready every mission"
        }
        (world::Class::Medic, 0) => HEAL_DRONE_WHAT,
        (world::Class::Medic, 1) => {
            "• Passive\n• Heals stronger on the hurt\n• Full bonus near 0 HP, half at 50%"
        }
        (world::Class::Medic, 2) => HEAL_BEAM_WHAT,
        (world::Class::Medic, 3) => {
            "• Ultimate, toggle\n• Allies in sight heal at the beam's rate\n• Costs you as much, can down you\n• Enemies inside burn at half"
        }
        (world::Class::Tank, 0) => {
            "• Toggle\n• Front shots bounce back, can hit enemies\n• Takes their damage, breaks at 0\n• Mends lowered, or up after a pause"
        }
        (world::Class::Tank, 1) => {
            "• Passive\n• Less damage a hit, before armour\n• Health regenerates"
        }
        (world::Class::Tank, 2) => "• Every hit taken dealt back to its source",
        (world::Class::Tank, 3) => "• Ultimate\n• Draining shield on you and allies near",
        _ => "",
    }
}
/// One row of an ability's numbers, the way Dota 2 lists them: a label,
/// its value at each rank — or one value where the ranks change nothing —
/// and the unit said once after the last. A value a rank does not have
/// yet is [`NOT_YET`].
#[derive(Clone, Debug, PartialEq)]
pub struct Stat {
    pub label: &'static str,
    pub values: Vec<String>,
    pub unit: &'static str,
}
/// A value a rank does not have: an upgrade that comes at a later one.
pub const NOT_YET: &str = "—";
impl Stat {
    /// A row read off a per-rank table; four equal values fold into one.
    fn ranks(label: &'static str, unit: &'static str, value: impl Fn(usize) -> String) -> Stat {
        let mut values: Vec<String> = (0..world::class::MAX_RANK as usize).map(value).collect();
        if values.iter().all(|v| *v == values[0]) {
            values.truncate(1);
        }
        Stat {
            label,
            values,
            unit,
        }
    }
    /// A row that is one value at every rank.
    fn one(label: &'static str, unit: &'static str, value: String) -> Stat {
        Stat {
            label,
            values: vec![value],
            unit,
        }
    }
    /// A row that is [`NOT_YET`] below rank `from` and `value` from it.
    fn from_rank(label: &'static str, from: u8, value: String) -> Stat {
        Stat::ranks(label, "", |r| {
            if r + 1 >= from as usize {
                value.clone()
            } else {
                NOT_YET.to_string()
            }
        })
    }
    /// The row as one line of text: `Damage: 60 / 75 / 90 / 110`.
    #[cfg(test)]
    pub fn line(&self) -> String {
        format!("{}: {}{}", self.label, self.values.join(" / "), self.unit)
    }
}
/// What the commander's Medivac medic wears at rank index `r` (nought the
/// first rank), off `class::MEDIVAC_VEST`.
fn medivac_armour(r: usize) -> &'static str {
    match world::class::MEDIVAC_VEST[r] {
        None => "None",
        Some(bims::combat::Tier::One) => "Armour T1",
        Some(bims::combat::Tier::Two) => "Armour T2",
        Some(_) => "Armour T3",
    }
}
/// A ranked ability's numbers, every rank's at once (Dota 2's tooltip):
/// the rules crates' own tables.
pub fn ranked_stats(class: world::Class, slot: u8) -> Vec<Stat> {
    use world::class as c;
    let charges = |n: &'static [u32]| Stat::ranks("Charges", "", move |r| n[r].to_string());
    let cooldown = |s: &'static [f64]| Stat::ranks("Cooldown", " s", move |r| fig(s[r]));
    let mut stats = match (class, slot) {
        (world::Class::Soldier, 0) => vec![
            Stat::ranks("Damage", "", |r| fig(c::GRENADE_DAMAGE[r] as f64)),
            Stat::ranks("Radius", " tiles", |r| fig(c::GRENADE_RADIUS[r] as f64)),
            Stat::one("Range", " tiles", fig(c::GRENADE_RANGE as f64)),
            charges(&c::GRENADE_CHARGES),
            cooldown(&c::GRENADE_COOLDOWN),
        ],
        (world::Class::Soldier, 1) => vec![
            Stat::ranks("Crit chance", "", |r| pc(c::WEAK_SPOT_CHANCE[r] as f64)),
            Stat::ranks("Crit damage", "", |r| pc(c::WEAK_SPOT_DAMAGE[r] as f64)),
        ],
        (world::Class::Soldier, 2) => vec![
            Stat::ranks("Damage", "", |r| fig(c::STUN_SHOT_DAMAGE[r] as f64)),
            Stat::ranks("Radius", " tiles", |r| fig(c::STUN_SHOT_RADIUS[r] as f64)),
            Stat::ranks("Stun", " s", |r| fig(c::STUN_SHOT_STUN[r] as f64)),
            Stat::one("Charge", " s", fig(c::STUN_SHOT_CHARGE)),
            Stat::one("Range", "", "the weapon's".to_string()),
            cooldown(&c::STUN_SHOT_COOLDOWN),
        ],
        (world::Class::Soldier, 3) => vec![
            Stat::ranks("Duration", " s", |r| fig(c::RAMPAGE_SECONDS[r])),
            Stat::ranks("Fire rate", "", |r| by(c::RAMPAGE_FIRE_RATE[r] as f64)),
            Stat::one("Reload speed", "", by(c::RAMPAGE_RELOAD_SPEED as f64)),
            Stat::ranks(
                "Damage taken",
                "",
                |r| by(c::RAMPAGE_DAMAGE_TAKEN[r] as f64),
            ),
            Stat::from_rank(
                "Per kill",
                c::RAMPAGE_EXTEND_RANK,
                format!(
                    "+{} s (max {} s)",
                    fig(c::RAMPAGE_EXTEND_SECONDS),
                    fig(c::RAMPAGE_EXTEND_MAX)
                ),
            ),
            cooldown(&c::RAMPAGE_COOLDOWN),
        ],
        (world::Class::Engineer, 0) => vec![
            Stat::ranks("Damage", "", |r| fig(c::MINE_DAMAGE[r] as f64)),
            Stat::ranks("Blast", " tiles", |r| fig(c::MINE_RADIUS[r] as f64)),
            Stat::one("Trigger", " tile", fig(c::MINE_TRIGGER as f64)),
            Stat::ranks("Laid at once", "", |r| c::MINE_STANDING[r].to_string()),
            Stat::one("Lay time", " min", fig(c::MINE_MINUTES)),
            charges(&c::MINE_CHARGES),
            cooldown(&c::MINE_COOLDOWN),
        ],
        (world::Class::Engineer, 1) => vec![
            Stat::ranks("Heal", "% HP/s", |r| {
                fig((c::HEALING_SENTRY_RATE[r] * c::HEAL_BEAM_HP) as f64 / 60.0)
            }),
            Stat::ranks("Radius", " tiles", |r| {
                fig(c::HEALING_SENTRY_RADIUS[r] as f64)
            }),
            Stat::ranks("Health", "", |r| fig(c::HEALING_SENTRY_HEALTH[r] as f64)),
            Stat::ranks("Lay time", " min", |r| fig(c::HEALING_SENTRY_MINUTES[r])),
            charges(&c::HEALING_SENTRY_CHARGES),
            cooldown(&c::HEALING_SENTRY_COOLDOWN),
        ],
        (world::Class::Engineer, 2) => vec![
            Stat::ranks("Damage", "", |r| fig(c::SATCHEL_DAMAGE[r] as f64)),
            Stat::ranks("Blast", " tiles", |r| fig(c::SATCHEL_RADIUS[r] as f64)),
            Stat::one("Range", " tiles", fig(c::GRENADE_RANGE as f64)),
            Stat::one("Trigger", "", "G".to_string()),
            charges(&c::SATCHEL_CHARGES),
            cooldown(&c::SATCHEL_COOLDOWN),
        ],
        (world::Class::Engineer, 3) => vec![
            Stat::ranks("Minigun tier", "", |r| c::SENTRY_TIER[r].code().to_string()),
            Stat::ranks("Fire rate", "", |r| by(c::SENTRY_FIRE_RATE[r] as f64)),
            Stat::ranks("Damage", "", |r| by(c::SENTRY_DAMAGE[r] as f64)),
            Stat::ranks("Health", "", |r| fig(c::SENTRY_HEALTH[r] as f64)),
            Stat::ranks("Range", " tiles", |r| {
                format!("+{}", fig(c::SENTRY_RANGE[r] as f64))
            }),
            Stat::one("Lay time", " min", fig(c::SENTRY_MINUTES)),
            cooldown(&c::SENTRY_COOLDOWN),
        ],
        (world::Class::Commander, 0) => vec![
            Stat::ranks("Fire rate", "", |r| by(c::BATTLE_CRY_FIRE_RATE[r] as f64)),
            Stat::ranks("Duration", " s", |r| fig(c::BATTLE_CRY_SECONDS[r])),
            Stat::one("Radius", " tiles", fig(c::BATTLE_CRY_TILES as f64)),
            cooldown(&c::BATTLE_CRY_COOLDOWN),
        ],
        (world::Class::Commander, 1) => vec![
            Stat::ranks("Armour", "", |r| medivac_armour(r).to_string()),
            Stat::one("Damage", "", by(c::REINFORCEMENT_DAMAGE as f64)),
            cooldown(&c::MEDIVAC_COOLDOWN),
        ],
        (world::Class::Commander, 2) => vec![
            Stat::ranks("Damage taken", "", |r| by(c::RALLY_DAMAGE_TAKEN[r] as f64)),
            Stat::ranks("Move speed", "", |r| by(c::RALLY_PACE[r] as f64)),
            Stat::ranks("Duration", " s", |r| fig(c::RALLY_SECONDS[r])),
            Stat::one("Radius", " tiles", fig(c::RALLY_TILES as f64)),
            cooldown(&c::RALLY_COOLDOWN),
        ],
        (world::Class::Commander, 3) => vec![
            Stat::ranks("Soldiers", "", |r| c::REINFORCEMENTS[r].to_string()),
            Stat::ranks("Rifle tier", "", |r| {
                c::REINFORCEMENT_TIER[r].code().to_string()
            }),
            Stat::one("Damage", "", by(c::REINFORCEMENT_DAMAGE as f64)),
            Stat::one(
                "Arrive within",
                " tiles",
                fig(c::REINFORCEMENT_REACH_TILES as f64),
            ),
            Stat::one("Cooldown", " s", fig(c::REINFORCEMENT_COOLDOWN)),
        ],
        (world::Class::Medic, 0) => vec![
            Stat::ranks("Heal", "% HP/s", |r| fig(c::HEAL_DRONE_HEAL[r] as f64)),
            Stat::ranks("Duration", " s", |r| fig(c::HEAL_DRONE_SECONDS[r])),
            cooldown(&c::HEAL_DRONE_COOLDOWN),
        ],
        (world::Class::Medic, 1) => vec![Stat::ranks("Healing at no health", "", |r| {
            by(1.0 + c::TRIAGE[r] as f64)
        })],
        (world::Class::Medic, 2) => vec![
            Stat::ranks("Heal", "% HP/s", |r| {
                fig((c::HEAL_BEAM_HP * c::HEAL_BEAM_RATE[r]) as f64 / 60.0)
            }),
            Stat::ranks("Range", " tiles", |r| fig(c::HEAL_BEAM_RANGES[r] as f64)),
            Stat::ranks("Patients", "", |r| c::HEAL_BEAM_PATIENTS[r].to_string()),
            Stat::one("Rank 4 adds", "", "items' regen".to_string()),
        ],
        (world::Class::Medic, 3) => vec![
            Stat::ranks("Radius", " tiles", |r| {
                fig(c::HEALING_CIRCLE_RADIUS[r] as f64)
            }),
            Stat::one(
                "Heal",
                "",
                format!(
                    "beam's rate, costs {}%",
                    (c::HEALING_CIRCLE_COST * 100.0).round()
                ),
            ),
            Stat::one(
                "Burn",
                "",
                format!("{}% of the heal", (c::HEALING_CIRCLE_BURN * 100.0).round()),
            ),
        ],
        // The tank's (task 155).
        (world::Class::Tank, 0) => vec![
            Stat::ranks("Shield", " hp", |r| fig(c::RIOT_SHIELD_HP[r] as f64)),
            Stat::ranks("Mends", " hp/s", |r| fig(c::RIOT_SHIELD_REGEN[r] as f64)),
            Stat::one(
                "Held up, mends after",
                " s",
                fig(c::RIOT_SHIELD_REGEN_DELAY),
            ),
            Stat::one("Broken for", " s", fig(c::RIOT_SHIELD_BROKEN_COOLDOWN)),
        ],
        (world::Class::Tank, 1) => vec![
            Stat::ranks("Damage taken", "", |r| by(c::PLATED_DAMAGE_TAKEN[r] as f64)),
            Stat::ranks("Mends", "% HP/s", |r| fig(c::PLATED_REGEN[r] as f64)),
            Stat::ranks("Armour drain", "", |r| {
                let fortress = r + 1 >= c::FORTRESS_RANK as usize;
                by(if fortress {
                    (c::TANK_DRAIN * c::FORTRESS_DRAIN) as f64
                } else {
                    c::TANK_DRAIN as f64
                })
            }),
        ],
        (world::Class::Tank, 2) => vec![
            Stat::ranks("Duration", " s", |r| fig(c::REFLECT_SECONDS[r])),
            Stat::one("Dealt back", "", pc(c::REFLECT_SHARE as f64)),
            cooldown(&c::REFLECT_COOLDOWN),
        ],
        (world::Class::Tank, 3) => vec![
            Stat::ranks("Radius", " tiles", |r| fig(c::BASTION_RADIUS[r] as f64)),
            Stat::ranks("Shield", " hp", |r| fig(c::BASTION_HP[r] as f64)),
            Stat::ranks("Drains", " hp/s", |r| fig(c::BASTION_DRAIN[r] as f64)),
            cooldown(&c::BASTION_COOLDOWN),
        ],
        _ => Vec::new(),
    };
    // The ultimate's fifth rank, which an Override Core gives (October
    // 2026).
    if slot == world::class::SLOT_R
        && let Some(line) = override_rank(class)
    {
        stats.push(Stat::one("With an Override Core", "", line));
    }
    stats
}
/// What an *Override Core* makes of a class's ultimate at its fifth rank,
/// in a line (October 2026). `None` for no class.
pub fn override_rank(class: world::Class) -> Option<String> {
    use world::class as c;
    let r = (c::OVERRIDE_RANK - 1) as usize;
    Some(match class {
        world::Class::None => return None,
        world::Class::Soldier => format!(
            "{} s, fire rate {}, reload speed {}, damage taken {}, and every grenade back in hand",
            fig(c::RAMPAGE_SECONDS[r]),
            by(c::RAMPAGE_FIRE_RATE[r] as f64),
            by(c::RAMPAGE_RELOAD_SPEED as f64),
            by(c::RAMPAGE_DAMAGE_TAKEN[r] as f64),
        ),
        world::Class::Engineer => format!(
            "health {}, fire rate {}, and two sentries standing at once",
            fig(c::SENTRY_HEALTH[r] as f64),
            by(c::SENTRY_FIRE_RATE[r] as f64),
        ),
        world::Class::Medic => format!(
            "all his healing {} — the beam, the drone and the circle",
            by(c::OVERRIDE_HEAL as f64),
        ),
        world::Class::Tank => format!(
            "{} tiles, and everybody it reached moves {} as fast while it lasts",
            fig(c::BASTION_RADIUS[r] as f64),
            by(c::BASTION_HASTE as f64),
        ),
        world::Class::Commander => format!("{} Bims, each in armour", c::REINFORCEMENTS[r]),
    })
}
/// The foot of a ranked ability's tip: the rank it is at, and the level
/// the next wants.
pub fn ranked_foot(class: world::Class, slot: u8, rank: u8) -> String {
    let top = world::class::MAX_RANK;
    let now = if rank == 0 {
        "Not learnt".to_string()
    } else {
        format!("Rank {rank}/{top}")
    };
    match world::class::rank_level(class, slot, rank + 1).filter(|_| rank < top) {
        Some(level) => format!("{now} · rank {} at Lv {level} · Ctrl-click", rank + 1),
        None => format!("{now} · max"),
    }
}
/// A ranked ability's whole tip as text: the line, the numbers and the
/// foot. The box draws the three itself, the current rank lit.
#[cfg(test)]
pub fn ranked_tip(class: world::Class, slot: u8, rank: u8) -> String {
    let mut tip = ranked_what(class, slot).to_string();
    for stat in ranked_stats(class, slot) {
        tip.push('\n');
        tip.push_str(&stat.line());
    }
    tip.push('\n');
    tip.push_str(&ranked_foot(class, slot, rank));
    tip
}
/// The log's line for a rank-up refused.
pub fn rank_refused(why: world::Refusal) -> String {
    format!("Cannot rank that up: {}.", refusal(why))
}
/// And for a Battle Cry refused (task 129).
pub fn battle_cry_refused(why: world::Refusal) -> String {
    format!("Cannot call a Battle Cry: {}.", refusal(why))
}
/// And for reinforcements refused.
pub fn reinforce_refused(why: world::Refusal) -> String {
    format!("Cannot call reinforcements: {}.", refusal(why))
}
/// And for a Rampage refused.
/// And for a Medivac refused.
pub fn medivac_refused(why: world::Refusal) -> String {
    format!("Cannot call a medic in: {}.", refusal(why))
}
pub fn rampage_refused(why: world::Refusal) -> String {
    format!("Cannot go on a Rampage: {}.", refusal(why))
}
/// And for a Heal Drone and a Healing Circle refused (task 153).
pub fn heal_drone_refused(why: world::Refusal) -> String {
    format!("Cannot drop a Heal Drone: {}.", refusal(why))
}
pub fn healing_circle_refused(why: world::Refusal) -> String {
    format!("Cannot switch the Healing Circle on: {}.", refusal(why))
}
/// The Skills tab of a ranked kit (task 124): what it says of the points
/// waiting, and a rank's line and button.
pub const RANKED_SKILLS_TIP: &str = "• 1 skill point a level\n• Buys a rank: Ctrl+key, Ctrl-click its box, or the button\n• Q F E: levels 1, 3, 5, 7\n• R: levels 6, 9, 12, 15\n• Unspent points kept";
pub fn ranked_points(points: u8) -> String {
    match points {
        0 => "No skill points — the next comes with the next level.".to_string(),
        1 => "One skill point to spend.".to_string(),
        n => format!("{n} skill points to spend."),
    }
}
/// The label before a ranked ability's levels on the Skills tab: the
/// level each rank is learnt at, one after another.
pub const RANK_LEVELS: &str = "Levels";
pub fn rank_learn(rank: u8) -> String {
    format!("Learn rank {rank}")
}
/// How many skill points are waiting, beside the experience bar (task
/// 124): nothing with none.
pub fn points_waiting(points: u8) -> Option<String> {
    match points {
        0 => None,
        1 => Some("1 skill point".to_string()),
        n => Some(format!("{n} skill points")),
    }
}

/// The line under a box whose level is not reached yet.
pub fn ability_locked(level: u8) -> String {
    format!("Level {level}")
}
/// The Skills tab of a crew member with no class (the talent tree it once
/// was went with the talents, task 139: every class is a ranked kit).
pub const SKILLS_NO_CLASS: &str = "This crew member has no class, so there is nothing to learn. A class is chosen on the setup tab, before the ship first leaves its berth.";

/// A number said the way the Skills tab says one: two decimals at most,
/// and no trailing nought at all. `6.0` is "6", `1.15` is "1.15",
/// `26.666` is "26.67".
fn fig(v: f64) -> String {
    let r = (v * 100.0).round() / 100.0;
    if (r - r.round()).abs() < 0.001 {
        format!("{}", r.round() as i64)
    } else {
        let mut s = format!("{r:.2}");
        while s.ends_with('0') {
            s.pop();
        }
        s
    }
}

/// A factor, the way a rank's line says one: `×1.15`.
fn by(v: f64) -> String {
    format!("×{}", fig(v))
}

/// A percentage, whole: `0.75` is "75%".
fn pc(v: f64) -> String {
    format!("{}%", fig(v * 100.0))
}

/// The class row on the crew panel: the class, the level, and what is
/// still wanted for the next.
pub fn class_line(class: world::Class, level: u8, to_next: u32) -> String {
    if to_next == 0 {
        format!("{} — level {level}", class_name(class))
    } else {
        format!(
            "{} — level {level}, {to_next} xp to the next",
            class_name(class)
        )
    }
}
pub const PACK_UP: &str = "Pack up";
/// What a deployable on the deck is called, by `world::DeployKind` code
/// — the sandbags' 0 left free since task 154 — with what it has left.
pub fn deployable_name(code: u32) -> &'static str {
    match world::DeployKind::from_code(code) {
        Some(world::DeployKind::Sentry) => "Sentry",
        Some(world::DeployKind::HealingSentry) => "Healing Sentry",
        Some(world::DeployKind::Mine) => "Mine",
        Some(world::DeployKind::Satchel) => "Satchel Charge",
        None => "Deployable",
    }
}
pub fn deployable_line(d: &world::Deployable) -> String {
    let name = deployable_name(d.kind.code());
    match d.kind {
        world::DeployKind::Mine => format!("{name} — armed"),
        world::DeployKind::Satchel => format!("{name} — Space sets it off"),
        world::DeployKind::Sentry | world::DeployKind::HealingSentry => {
            format!("{name} — {:.0} health", d.health)
        }
    }
}
/// The log's line for a deploy key pressed and refused, off the world's
/// own refusal.
pub fn deploy_refused(why: world::Refusal) -> String {
    format!("Cannot set that up: {}.", refusal(why))
}
/// The same for a grenade thrown and refused (feature 75).
pub fn throw_refused(why: world::Refusal) -> String {
    format!("Cannot throw that: {}.", refusal(why))
}
/// And for the engineer's remote trigger pressed and refused (task 154).
pub fn detonate_refused(why: world::Refusal) -> String {
    format!("Nothing to set off: {}.", refusal(why))
}
/// And for a weld refused (October 2026).
pub fn weld_refused(why: world::Refusal) -> String {
    format!("Cannot weld: {}.", refusal(why))
}
/// The Use key refused at the second set of attacks (October 2026).
pub fn use_refused(why: world::Refusal) -> String {
    format!("Nothing to do here: {}.", refusal(why))
}
/// And for a Stun Shot refused (October 2026).
pub fn stun_shot_refused(why: world::Refusal) -> String {
    format!("Cannot fire a Stun Shot: {}.", refusal(why))
}
/// And for a beam (feature 76).
pub fn beam_refused(why: world::Refusal) -> String {
    format!("Cannot beam: {}.", refusal(why))
}
/// And for the tank's three (task 155): a Riot Shield, a Reflect Barrier
/// and a Bastion refused.
pub fn riot_shield_refused(why: world::Refusal) -> String {
    format!("Cannot raise the shield: {}.", refusal(why))
}
pub fn reflect_refused(why: world::Refusal) -> String {
    format!("Cannot raise the barrier: {}.", refusal(why))
}
pub fn bastion_refused(why: world::Refusal) -> String {
    format!("Cannot throw the Bastion: {}.", refusal(why))
}
/// And for a rally (feature 78).
pub fn rally_refused(why: world::Refusal) -> String {
    format!("Cannot rally: {}.", refusal(why))
}
/// And for either of the two standing orders every player has (feature
/// 84): the attack banner and the retreat.
pub fn orders_refused(why: world::Refusal) -> String {
    format!("Cannot order the crew: {}.", refusal(why))
}
/// And the carry's (feature 86).
pub fn carry_refused(why: world::Refusal) -> String {
    format!("Cannot carry: {}.", refusal(why))
}
/// What the crew that follow you are under, for the strip over the
/// canvas — nothing at all while they are simply following, since the
/// ring round your Bim already says so.
pub fn orders_line(kind: u32) -> Option<&'static str> {
    match kind {
        1 => Some("Crew: attacking the banner"),
        2 => Some("Crew: falling back to the ship"),
        _ => None,
    }
}
pub const ORDERS_TIP: &str = "• Unsteered crew follow you and fight\n• X: attack banner\n• Y: back to the ship\n• Again: follow\n• WASD walk · pointer aims · left button fires\n• F + click: walk there shooting";
/// The commander's rows on the crew panel (feature 78): the rally with
/// its cooldown.
pub fn rally_line(left: f64, cooldown: f64, level_enough: bool) -> String {
    if !level_enough {
        return RALLY_NOT_LEARNT.to_string();
    }
    if left > 0.0 {
        return format!("Rallying — {left:.0} s left");
    }
    if cooldown > 0.0 {
        format!("Rally ready in {cooldown:.0} s")
    } else {
        "Rally ready".to_string()
    }
}
/// The Rally before its first rank (task 129).
pub const RALLY_NOT_LEARNT: &str = "Rally not learnt yet";
/// The Battle Cry's line on the crew panel (task 129), the rally's way.
pub fn battle_cry_line(left: f64, cooldown: f64, learnt: bool) -> String {
    if !learnt {
        return "Battle Cry not learnt yet".to_string();
    }
    if left > 0.0 {
        return format!("Battle Cry — {left:.0} s left");
    }
    if cooldown > 0.0 {
        format!("Battle Cry ready in {cooldown:.0} s")
    } else {
        "Battle Cry ready".to_string()
    }
}
/// The tank's rows on the crew panel (task 155): the Riot Shield, and the
/// Reflect Barrier and the Bastion, each with its cooldown — the shield's
/// the seconds a broken one waits (nought when it is not broken).
pub fn riot_shield_line(up: bool, left: f32, whole: f32, cooldown: f64) -> String {
    if whole <= 0.0 {
        return RIOT_SHIELD_NOT_LEARNT.to_string();
    }
    let hp = format!("{:.0}/{:.0}", left.max(0.0), whole);
    if cooldown > 0.0 {
        format!("Shield broken — {hp}, ready in {:.0} s", cooldown.ceil())
    } else if up {
        format!("Shield up — {hp}")
    } else {
        format!("Shield down — {hp}")
    }
}
/// The Riot Shield before its first rank.
pub const RIOT_SHIELD_NOT_LEARNT: &str = "Riot Shield not learnt yet";
pub const RIOT_SHIELD_TIP: &str = "• Q: raise / lower\n• Bounces front shots back\n• Lowered when downed\n• Breaks at 0: 10 s before it rises\n• Mends lowered, or 5 s after the last hit";
pub fn reflect_line(left: f64, cooldown: f64, learnt: bool) -> String {
    if !learnt {
        return REFLECT_NOT_LEARNT.to_string();
    }
    if left > 0.0 {
        return format!("Reflecting — {left:.0} s left");
    }
    if cooldown > 0.0 {
        format!("Reflect Barrier ready in {cooldown:.0} s")
    } else {
        "Reflect Barrier ready".to_string()
    }
}
/// The Reflect Barrier before its first rank.
pub const REFLECT_NOT_LEARNT: &str = "Reflect Barrier not learnt yet";
/// The Bastion's line on the crew panel, the barrier's way.
pub fn bastion_line(cooldown: f64, learnt: bool) -> String {
    if !learnt {
        return BASTION_NOT_LEARNT.to_string();
    }
    if cooldown > 0.0 {
        format!("Bastion ready in {cooldown:.0} s")
    } else {
        "Bastion ready".to_string()
    }
}
/// The Bastion before its first rank.
pub const BASTION_NOT_LEARNT: &str = "Bastion not learnt yet";
/// The soldier's rows on the crew panel (feature 75): grenades carried,
/// the throw's cooldown, and the Stun Shot (October 2026).
pub const CHARGING: &str = "Charging a shot";
pub const CHARGING_TIP: &str =
    "• Stun Shot charging\n• No firing, can walk\n• Fires at the pointer";
pub fn stun_shot_ready(cooldown: f64) -> String {
    if cooldown > 0.0 {
        format!("Stun Shot ready in {cooldown:.0} s")
    } else {
        "Stun Shot ready".to_string()
    }
}
/// The medic's rows on the crew panel (feature 76; task 130): who the
/// beam holds, and how the Heal Drone and the Healing Circle stand.
pub const BEAM_ON: &str = "Beaming";
pub const BEAM_OFF: &str = "No beam";
pub const BEAM_TIP: &str = "• E on an ally or yourself: link\n• E again, or on nothing: unlink\n• Walk and fire while on\n• The medic heals as much";
pub fn beam_line(patients: &[String]) -> String {
    match patients {
        [] => BEAM_OFF.to_string(),
        [one] => format!("{BEAM_ON} {one}"),
        many => format!("{BEAM_ON} {}", many.join(" and ")),
    }
}
/// The drone's row (task 153): the seconds the one up has left, else
/// when the next may be dropped.
pub fn heal_drone_line(left: f64, cooldown: f64, learnt: bool) -> String {
    if left > 0.0 {
        return format!("Heal Drone up — {left:.0} s left");
    }
    if !learnt {
        return "Heal Drone not learnt yet".to_string();
    }
    if cooldown > 0.0 {
        format!("Heal Drone ready in {cooldown:.0} s")
    } else {
        "Heal Drone ready".to_string()
    }
}
/// The circle's row (task 153): on, or ready to switch on.
pub fn healing_circle_line(on: bool, learnt: bool) -> String {
    match (on, learnt) {
        (true, _) => "Healing Circle on — it costs you as much as it heals".to_string(),
        (false, true) => "Healing Circle off".to_string(),
        (false, false) => "Healing Circle not learnt yet".to_string(),
    }
}
/// The soldier's grenade row: the charges in the pack and, while one is
/// still coming back, how long the next is (feature 90).
pub fn grenades_line(carried: u32, cooldown: f64) -> String {
    let count = match carried {
        1 => "1 grenade".to_string(),
        n => format!("{n} grenades"),
    };
    if cooldown > 0.0 {
        format!("{count} — next in {cooldown:.0} s")
    } else {
        count
    }
}
/// What a heal beam just put back, as it floats off the patient
/// (feature 91): the points with a plus in front, since a number rising
/// off a body has to say which way it went.
pub fn heal_gain(points: u32) -> String {
    format!("+{points}")
}
/// What an enemy down paid, over where it fell: the money and the
/// experience, a line each (`theme::reward_numbers`).
pub fn reward_money(money: u64) -> String {
    format!("+{}", crate::format::euros(money))
}
pub fn reward_xp(xp: u32) -> String {
    format!("+{xp} XP")
}
/// A hit's number over whoever took it (`theme::hit_number`): the damage,
/// and a critical one marked.
pub fn hit_damage(damage: u32, crit: bool) -> String {
    if crit {
        format!("{damage}!")
    } else {
        damage.to_string()
    }
}
pub const HAIR_NAMES: [&str; 8] = [
    "Cropped", "Long", "Bald", "Bob", "Bun", "Mohawk", "Ponytail", "Curly",
];
pub const SHADE_NAMES: [&str; 6] = ["Dark", "Brown", "Black", "Blond", "Red", "Grey"];
pub fn hair_name(hair: bims::character::Hair) -> &'static str {
    HAIR_NAMES
        .get(hair.code() as usize)
        .copied()
        .unwrap_or("Hair")
}
pub fn shade_name(shade: bims::character::Shade) -> &'static str {
    SHADE_NAMES
        .get(shade.code() as usize)
        .copied()
        .unwrap_or("Hair")
}
/// The colours a player's own Bim can be ringed in (feature 84), pinned
/// against `bims::character::Tint::ALL` below.
pub const TINT_NAMES: [&str; 6] = ["Teal", "Amber", "Rose", "Violet", "Sky", "Lime"];
pub fn tint_name(tint: bims::character::Tint) -> &'static str {
    TINT_NAMES
        .get(tint.code() as usize)
        .copied()
        .unwrap_or("Colour")
}

/// The machines (feature 83): a wave landing, and the last of them
/// destroyed.
pub const DROID_REINFORCEMENTS: &str = "Another wave of machines has landed.";
pub const DROID_CLEARED: &str = "The last of the machines is down.";

/// The Machine Heart (feature 108): its seal broken, its overload, its end.
pub const HEART_EXPOSED: &str = "The last conduit is down: the core is exposed.";
pub const HEART_OVERLOAD: &str = "The core is overloading!";
pub const HEART_DESTROYED: &str = "The Machine Heart is destroyed.";

/// Defending a site (features 94 and 111): the site held — a town, a
/// station or a derelict — and who came with the crew afterwards (a
/// town's people alone).
pub const TOWN_HELD: &str = "Held. The last of the machines is destroyed.";
pub fn townsfolk_joined(count: u32) -> String {
    match count {
        1 => "One of the town's people joins the crew.".to_string(),
        n => format!("{n} of the town's people join the crew."),
    }
}

/// What each of the four is called, indexed by
/// `bims::droid::DroidKind::code`; `0` is no machine at all. A droid
/// has no name of its own — it is a machine, not somebody — so the log
/// says its kind.
pub const DROID_NAMES: [&str; 11] = [
    "—",
    "Husk",
    "Trooper",
    "Warden",
    "Guardian",
    // The Machine Heart's (feature 108).
    "Core",
    "Conduit",
    "Fabricator",
    // The tier-two machines (task 157).
    "Bomber",
    "Lancer",
    "Conductor",
];

pub fn droid_name(code: u32) -> &'static str {
    DROID_NAMES.get(code as usize).copied().unwrap_or("Machine")
}

/// What happened, as a sentence. `None` for an event with nothing to say —
/// there are none today, and the arm is here so a new event is a missing
/// line rather than a blank row.
pub fn event_line(event: WorldEvent) -> Option<String> {
    let who = |slot: u32| crew_name(slot);
    Some(match event {
        WorldEvent::Discovered { .. } => "Something new on the scanner.".into(),
        WorldEvent::FrameChanged { frame } => {
            if frame.code() == 0 {
                "Out into open space.".into()
            } else {
                "Alongside.".into()
            }
        }
        WorldEvent::Refused { why, .. } => {
            format!("That could not be done — {}.", refusal(why))
        }
        WorldEvent::SitePlaced { kind, .. } => {
            format!("{} laid out.", part_name(kind))
        }
        WorldEvent::SiteCancelled { kind } => {
            format!("{} called off.", part_name(kind))
        }
        WorldEvent::Built { kind } => format!("{} built.", part_name(kind)),
        WorldEvent::BuildLost { kind } => {
            format!(
                "{} not built: the materials had gone, or it would no longer go there.",
                part_name(kind)
            )
        }
        WorldEvent::EnemyDown { station, who: w } => {
            format!("{} is down.", resident_name(station, w))
        }
        // A shot that landed on one of the crew.
        WorldEvent::CrewHit { who: w } => format!("{} was hit.", who(w)),
        WorldEvent::CrewDown { who: w } => format!("{} is dead.", who(w)),
        // A body at nothing (task 120): down on the deck with the
        // countdown running, and a crewmate beside it the way back up —
        // the line is what sends a player over.
        WorldEvent::CrewDowned { who: w } => format!(
            "{} is down! {} seconds to revive them.",
            who(w),
            bims::health::DOWNED_SECONDS.round() as u32
        ),
        WorldEvent::CrewRevived { who: w, by } => {
            format!("{} brought {} round.", who(by), who(w))
        }
        WorldEvent::ResidentRevived {
            station,
            who: w,
            by,
        } => {
            format!("{} brought {} round.", who(by), resident_name(station, w))
        }
        // A piece at nothing is still worn and does nothing until it
        // regenerates, once its wearer has gone a while unhit.
        WorldEvent::PieceBroke { who: w, kind } => {
            format!(
                "{}'s {} is down — it stops nothing until it regenerates.",
                who(w),
                armour_name(Some(kind)).to_lowercase()
            )
        }
        // A blade within reach: no more firing until it is out of reach.
        // The line is what tells a player why the gun has gone quiet.
        WorldEvent::Locked { who: w } => {
            format!(
                "{} is locked in melee — no firing until it is clear.",
                who(w)
            )
        }
        WorldEvent::Jumped { star } => format!("Jumped. The ship is in the system of star {star}."),
        WorldEvent::Infested { star } => format!(
            "The machines have this system. Every station round star {star} is theirs: nobody left aboard, nothing to trade."
        ),
        WorldEvent::CrewLost => "Nobody of the crew is standing. The run is over.".into(),
        // Every class is a ranked kit since task 139: a level is a point,
        // up to the sixteenth; past it, weapon damage (October 2026).
        WorldEvent::LevelUp { who: w, level, .. }
            if level > u32::from(world::class::SKILL_LEVELS) =>
        {
            let more = world::class::level_damage(level as u8) - 1.0;
            format!(
                "{} reached level {level} — weapon damage +{:.0}%.",
                who(w),
                more * 100.0
            )
        }
        WorldEvent::LevelUp { who: w, level, .. } => format!(
            "{} reached level {level} — a skill point to spend: Ctrl and an ability's key.",
            who(w)
        ),
        WorldEvent::Deployed { who: w, kind } => format!(
            "{} set up a {}.",
            who(w),
            deployable_name(kind).to_lowercase()
        ),
        WorldEvent::PackedUp { who: w, kind } => format!(
            "{} packed a {} up.",
            who(w),
            deployable_name(kind).to_lowercase()
        ),
        WorldEvent::DeployableLost { kind } => match world::DeployKind::from_code(kind) {
            Some(world::DeployKind::HealingSentry) => "A Healing Sentry is shot to pieces.".into(),
            Some(world::DeployKind::Mine) => "A mine is taken up: too many laid.".into(),
            _ => "A sentry is shot to pieces.".into(),
        },
        WorldEvent::MineTriggered { who: w } => format!("{}'s mine went off.", who(w)),
        WorldEvent::SatchelThrown { who: w } => format!("{} threw a satchel charge.", who(w)),
        WorldEvent::SatchelsBlown { who: w, count: 1 } => {
            format!("{} set off a satchel charge.", who(w))
        }
        WorldEvent::SatchelsBlown { who: w, count } => {
            format!("{} set off {count} satchel charges.", who(w))
        }
        WorldEvent::ShotCharging { who: w } => format!("{} charges a Stun Shot.", who(w)),
        WorldEvent::StunShotFired { who: w } => format!("{} fired a Stun Shot.", who(w)),
        WorldEvent::Thrown { who: w } => format!("{} threw a grenade.", who(w)),
        WorldEvent::Beamed {
            who: w,
            patient: Some(p),
        } if world::medic::guest_of(p).is_some() => format!("{} beamed a defender.", who(w)),
        WorldEvent::Beamed {
            who: w,
            patient: Some(p),
        } => format!("{} beamed {}.", who(w), who(p)),
        WorldEvent::Beamed { who: w, .. } => format!("{}'s beam is off.", who(w)),
        WorldEvent::DroneLaunched { who: w } => format!("{} dropped a Heal Drone.", who(w)),
        WorldEvent::Circled { who: w, on: true } => {
            format!("{}'s Healing Circle is on.", who(w))
        }
        WorldEvent::Circled { who: w, on: false } => {
            format!("{}'s Healing Circle is off.", who(w))
        }
        WorldEvent::ShieldRaised { who: w, on: true } => {
            format!("{} raised the Riot Shield.", who(w))
        }
        WorldEvent::ShieldRaised { who: w, on: false } => {
            format!("{} put the Riot Shield down.", who(w))
        }
        WorldEvent::ShieldBroken { who: w } => format!("{}'s Riot Shield broke.", who(w)),
        WorldEvent::Reflecting { who: w } => format!("{} raised a Reflect Barrier.", who(w)),
        WorldEvent::Bastion { who: w, reached } => format!(
            "{} threw the Bastion over {} {}.",
            who(w),
            reached,
            if reached == 1 { "Bim" } else { "Bims" }
        ),
        WorldEvent::Rallied { who: w } => format!("{} rallied the crew.", who(w)),
        WorldEvent::BattleCried { who: w } => format!("{} called a Battle Cry.", who(w)),
        WorldEvent::Reinforced { who: w, count } => format!(
            "{} brought {count} {} of the Republic.",
            who(w),
            if count == 1 { "soldier" } else { "soldiers" }
        ),
        WorldEvent::Medivac { who: w, .. } => {
            format!("{} called a Republic medic in.", who(w))
        }
        WorldEvent::DroidReinforcements { .. } => DROID_REINFORCEMENTS.into(),
        WorldEvent::DroidDown { kind, .. } => format!("{} is down.", droid_name(kind)),
        WorldEvent::DroidStationCleared { .. } => DROID_CLEARED.into(),
        WorldEvent::Ordered { who: w, kind } => match kind {
            1 => format!("{} put an attack banner down.", who(w)),
            2 => format!("{} called the crew back to the ship.", who(w)),
            _ => format!("{}'s crew are following again.", who(w)),
        },
        WorldEvent::Carried {
            who: w,
            patient: Some(p),
        } => format!("{} has {} in their arms.", who(w), who(p)),
        WorldEvent::Carried { who: w, .. } => format!("{} sets them down.", who(w)),
        WorldEvent::TownHeld { .. } => TOWN_HELD.into(),
        WorldEvent::TownsfolkJoined { count } => townsfolk_joined(count),
        // A picture's event: the numbers over the body say it, and the
        // Republic's own line says the money.
        WorldEvent::EnemyRewarded { .. } => return None,
        WorldEvent::Hit { .. } => return None,
        // A blink is seen and heard, not logged.
        WorldEvent::Blinked { .. } => return None,
        WorldEvent::ItemUsed { who: w, kind } => {
            let item = bims::module::ModuleKind::from_code(kind).map_or("an item", item_name);
            format!("{} used the {item}.", who(w))
        }
        WorldEvent::ItemBought {
            slot,
            kind,
            tier,
            upgrade,
            ..
        } => {
            let item = bims::module::ModuleKind::from_code(kind)
                .zip(bims::combat::Tier::from_code(tier))
                .map_or_else(|| "an item".to_string(), |(k, t)| item_title(k.at(t)));
            if upgrade {
                format!("{} upgraded to {item}.", player_name(slot))
            } else {
                format!("{} bought {item}.", player_name(slot))
            }
        }
        WorldEvent::Bounty { amount } => {
            format!("The Republic pays {}.", crate::format::euros(amount))
        }
        WorldEvent::BountyPending { amount } => format!(
            "The Republic owes {} once this place is cleared.",
            crate::format::euros(amount)
        ),
        WorldEvent::Proposed { slot, .. } => {
            format!("{} proposes a destination.", player_name(slot))
        }
        WorldEvent::ProposalAccepted { slot, yes: true } => {
            format!("{} accepts.", player_name(slot))
        }
        WorldEvent::ProposalAccepted { slot, yes: false } => {
            format!("{} takes their yes back.", player_name(slot))
        }
        WorldEvent::Travelled { minutes, .. } => format!(
            "Arrived after {}. A mission begins.",
            crate::format::trip_length(minutes)
        ),
        WorldEvent::Returning { slot } => format!("{} is heading back to the ship.", who(slot)),
        WorldEvent::DepartureAsked { behind } => departure_asked(behind),
        WorldEvent::DepartureDeclined { slot } => {
            format!(
                "{} will not leave them behind. The ship stays.",
                player_name(slot)
            )
        }
        WorldEvent::LeftSite { cleared: true, .. } => LEFT_CLEARED.into(),
        WorldEvent::LeftSite { cleared: false, .. } => LEFT_UNCLEARED.into(),
        WorldEvent::LeftBehind { who: w } => format!("{} was left behind.", who(w)),
        WorldEvent::Respawned { who: w, paid } => format!(
            "{} is back aboard with everything it wore. The pool pays {}.",
            who(w),
            crate::format::euros(paid)
        ),
        // The trader's (task 114).
        WorldEvent::ShelfBought { slot, to, .. } => {
            if to == u32::MAX {
                format!("{} bought a thing into the armory.", player_name(slot))
            } else {
                format!("{} bought a thing for {}.", player_name(slot), who(to))
            }
        }
        WorldEvent::Sold { slot, value, .. } => format!(
            "{} sold a thing for {}.",
            player_name(slot),
            crate::format::euros(value)
        ),
        WorldEvent::RankedUp {
            who: w,
            class,
            ability_slot,
            rank,
        } => format!(
            "{} raised {} to rank {rank}.",
            who(w),
            ranked_ability(
                world::Class::from_code(class).unwrap_or_default(),
                u8::try_from(ability_slot).unwrap_or(u8::MAX)
            )
        ),
        WorldEvent::Rampaged { who: w } => format!("{} goes on a Rampage.", who(w)),
        WorldEvent::GearChanged { .. } => GEAR_CHANGED.into(),
        WorldEvent::GearOffered { from, to, .. } => format!(
            "{} offers {} a thing: it is theirs when they accept it.",
            player_name(from),
            player_name(to)
        ),
        WorldEvent::OfferTaken { from, to, .. } => {
            format!(
                "{} took what {} offered.",
                player_name(to),
                player_name(from)
            )
        }
        WorldEvent::OfferWithdrawn { from, .. } => {
            format!("{}'s offer is withdrawn.", player_name(from))
        }
        WorldEvent::BotLost { who: w } => format!("{} is gone for good.", who(w)),
        WorldEvent::TownFell { .. } => TOWN_FELL.into(),
        WorldEvent::AreaTaken { .. } => AREA_TAKEN.into(),
        WorldEvent::AreaTimeUp { .. } => AREA_TIME_UP.into(),
        WorldEvent::BonusWaveChosen { slot, on: true } => format!(
            "{} chose the bonus wave: one more at the end, half as big again, for half the site's experience again.",
            player_name(slot)
        ),
        WorldEvent::BonusWaveChosen { slot, on: false } => {
            format!("{} took the bonus wave back.", player_name(slot))
        }
        WorldEvent::CleanSweep { who: w, xp } => {
            format!("Clean Sweep: {} earns {xp} experience more.", who(w))
        }
        WorldEvent::WarChest { slot, money } => format!(
            "War Chest: {} earns {} on the money kept.",
            player_name(slot),
            crate::format::euros(money)
        ),
        WorldEvent::Welding {
            who: w,
            done: false,
            ..
        } => format!("{} is welding a way in shut.", who(w)),
        WorldEvent::Welding {
            who: w, done: true, ..
        } => {
            format!(
                "{} welded a way in shut. The waves come in elsewhere.",
                who(w)
            )
        }
        WorldEvent::Planting { who: w } => format!("{} is planting the charge.", who(w)),
        WorldEvent::FlagCarried {
            who: w,
            taken: true,
        } => format!("{} carries the flag: the people follow.", who(w)),
        WorldEvent::FlagCarried {
            who: w,
            taken: false,
        } => {
            format!("{} put the flag down: the people hold by it.", who(w))
        }
        WorldEvent::Evacuated { aboard, .. } => format!("{aboard} aboard the ship."),
        WorldEvent::Objective {
            what, n, who: w, ..
        } => objective_event(what, n, w),
        WorldEvent::NestDestroyed { left: 0, .. } => "The last nest is destroyed.".into(),
        WorldEvent::NestDestroyed { left, .. } => format!("A nest destroyed: {left} left."),
        WorldEvent::SabotageStage { phase: 1, .. } => {
            "The charge is planted. The ship casts off: hold it until it arms.".into()
        }
        WorldEvent::SabotageStage { phase: 2, .. } => {
            "The charge is armed. Get to the way out before it blows!".into()
        }
        WorldEvent::SabotageStage { phase: 3, .. } => "The charge blew. The ship takes off.".into(),
        WorldEvent::SabotageStage { phase: 4, .. } => "The machines disarmed the charge.".into(),
        WorldEvent::SabotageStage { .. } => String::new(),
        WorldEvent::WeldBurnt { .. } => {
            "The machines burnt through a weld: every way in was shut.".into()
        }
        WorldEvent::PlayerGone { slot } => format!("{} has left the game.", player_name(slot)),
        WorldEvent::Readied { slot, yes: true } => format!("{} is ready.", player_name(slot)),
        WorldEvent::Readied { slot, yes: false } => {
            format!("{} is not ready after all.", player_name(slot))
        }
        WorldEvent::AllReady => "Everybody is ready. The mission is under way.".into(),
        WorldEvent::RelicsOffered { count } => {
            format!("The elite is beaten: {count} relics on offer. Choose one together — or none.")
        }
        WorldEvent::RelicProposed { slot, relic } => match relic_of(relic) {
            Some(r) => format!("{} puts {} to the crew.", player_name(slot), relic_name(r)),
            None => format!("{} would take no relic.", player_name(slot)),
        },
        WorldEvent::RelicAccepted { slot, yes: true } => {
            format!("{} says yes to the relic.", player_name(slot))
        }
        WorldEvent::RelicAccepted { slot, yes: false } => {
            format!("{} takes their yes back.", player_name(slot))
        }
        WorldEvent::RelicGiven { relic } => format!(
            "The crew take {} for the rest of the run.",
            relic_of(relic).map_or("a relic", relic_name)
        ),
        WorldEvent::RelicsDeclined => "The crew take no relic.".into(),
        WorldEvent::RunWon => "The run is won.".into(),
        WorldEvent::HeartExposed { .. } => HEART_EXPOSED.into(),
        WorldEvent::HeartOverload { .. } => HEART_OVERLOAD.into(),
        WorldEvent::HeartDestroyed { .. } => HEART_DESTROYED.into(),
    })
}

// --- relics (feature 106) -----------------------------------------------------------

fn relic_of(code: u32) -> Option<world::Relic> {
    world::Relic::from_code(code)
}

/// Every relic's name, in `world::Relic::ALL`'s order.
pub const RELIC_NAMES: [&str; 19] = [
    "Glass Cannon",
    "Heavy Plating",
    "Hair Trigger",
    "Overclocked Cores",
    "Bounty Contract",
    "Hunter's Pact",
    "Drill Sergeant",
    "Lone Wolves",
    "Black Market",
    "Adrenaline",
    "Salvage Burn",
    "Nanite Mesh",
    "Clean Sweep",
    "Point Blank",
    "Marksman's Creed",
    "Forked Path",
    "War Chest",
    "Overtime",
    "Giant Slayer",
];

/// The items' names (October 2026), by `bims::module::ModuleKind` code.
pub const ITEM_NAMES: [&str; 20] = [
    "Blink Drive",
    "Executioner",
    "Reactor Heart",
    "Override Core",
    "Coolant Loop",
    "Pressure Seal",
    "Steady Grip",
    "Long Barrel",
    "Leech Capacitor",
    "Arc Coil",
    "Field Mender",
    "Reset Capacitor",
    "Ablative Shell",
    "Training Log",
    "Smoke Launcher",
    "Targeting Uplink",
    "Tether Link",
    "Decoy Projector",
    "Adrenal Injector",
    "Overcharger",
];

/// An item's name.
pub fn item_name(kind: bims::module::ModuleKind) -> &'static str {
    ITEM_NAMES
        .get(kind.code() as usize)
        .copied()
        .unwrap_or("Item")
}

/// An item's name with its tier, for a line of the log: "a Blink Drive,
/// tier 2"; the *Override Core*, which has one tier, without.
pub fn item_title(item: bims::module::Module) -> String {
    let name = item_name(item.kind);
    let a = if name.starts_with(['A', 'E', 'I', 'O', 'U']) {
        "an"
    } else {
        "a"
    };
    if item.kind.tiered() {
        format!("{a} {name}, {}", tier_name(item.tier.code()))
    } else {
        format!("{a} {name}")
    }
}

/// What an item does, a bullet a fact (an active one's first says so),
/// off `bims::module`'s numbers at its tier: an item's tooltip.
pub fn item_line(item: bims::module::Module) -> String {
    use bims::module::{self as m, ModuleKind};
    let t = (item.tier.code().clamp(1, 3) - 1) as usize;
    match item.kind {
        ModuleKind::BlinkDrive => format!(
            "• Active\n• To the pointer, up to {} tiles, in sight\n• {} s cooldown\n• Not within {} s of a hit",
            fig(m::BLINK_RANGE_TILES[t] as f64),
            fig(m::BLINK_COOLDOWN_SECONDS[t] as f64),
            fig(m::BLINK_HIT_LOCK_SECONDS as f64),
        ),
        ModuleKind::Executioner => format!(
            "• {} crit chance, {} crit damage\n• Beside Weak Spot: the bigger counts",
            pc(m::EXECUTIONER_CHANCE[t] as f64),
            pc(m::EXECUTIONER_DAMAGE[t] as f64),
        ),
        ModuleKind::ReactorHeart => format!(
            "• +{} health\n• Regen {}% HP/s\n• {}% after {} s unhit",
            fig(m::HEART_HEALTH[t] as f64),
            fig(m::HEART_REGEN[t] as f64),
            fig(m::HEART_QUIET_REGEN[t] as f64),
            fig(m::HEART_QUIET_SECONDS as f64),
        ),
        ModuleKind::OverrideCore => format!(
            "• Ultimate +1 rank, up to {}\n• Needs a rank bought\n• See the ultimate's tooltip",
            world::class::OVERRIDE_RANK,
        ),
        ModuleKind::CoolantLoop => format!(
            "• -{}% ability cooldowns\n• Adds up, to -{}% at most",
            m::COOLANT_LOOP_PERCENT[t],
            m::COOLDOWN_CUT_MOST,
        ),
        ModuleKind::PressureSeal => {
            format!("• +{} health", fig(m::PRESSURE_SEAL_HEALTH[t] as f64))
        }
        ModuleKind::SteadyGrip => format!("• +{}% fire rate", m::STEADY_GRIP_PERCENT[t]),
        ModuleKind::LongBarrel => format!(
            "• +{} tiles weapon range",
            fig(m::LONG_BARREL_TILES[t] as f64),
        ),
        ModuleKind::LeechCapacitor => {
            format!("• {} of weapon damage healed", pc(m::LEECH_SHARE[t] as f64),)
        }
        ModuleKind::ArcCoil => format!(
            "• Every {}th hit arcs to {} enemies within {} tiles\n• {} damage each",
            m::ARC_EVERY,
            m::ARC_TARGETS[t],
            fig(m::ARC_REACH_TILES as f64),
            fig(m::ARC_DAMAGE[t] as f64),
        ),
        ModuleKind::FieldMender => format!(
            "• Active\n• Heals allies within {} tiles, you too, {}% max HP\n• {} s cooldown",
            fig(m::MENDER_TILES as f64),
            fig(m::MENDER_HEAL[t] as f64),
            fig(m::MENDER_COOLDOWN_SECONDS[t] as f64),
        ),
        ModuleKind::ResetCapacitor => format!(
            "• Active\n• Every ability and item ready, charges full\n• {} s cooldown",
            fig(m::RESET_COOLDOWN_SECONDS[t] as f64),
        ),
        ModuleKind::AblativeShell => format!(
            "• Active\n• {} s: damage taken {}\n• Warden lances strip no armour\n• {} s cooldown",
            fig(m::SHELL_SECONDS[t] as f64),
            by(m::SHELL_DAMAGE_TAKEN as f64),
            fig(m::SHELL_COOLDOWN_SECONDS[t] as f64),
        ),
        ModuleKind::TrainingLog => format!(
            "• +{}% XP a kill\n• Two don't add\n• Best bought early",
            m::TRAINING_LOG_XP[t],
        ),
        ModuleKind::SmokeLauncher => format!(
            "• Active\n• Smoke {} tiles wide at the pointer, up to {} tiles off, {} s\n• Blocks enemy sight; crew inside not targeted\n• {} s cooldown",
            fig(2.0 * m::SMOKE_RADIUS_TILES[t] as f64),
            fig(m::SMOKE_RANGE_TILES as f64),
            fig(m::SMOKE_SECONDS[t] as f64),
            fig(m::SMOKE_COOLDOWN_SECONDS[t] as f64),
        ),
        ModuleKind::TargetingUplink => format!(
            "• Bots within {} tiles: +{}% damage\n• Two don't add",
            fig(m::UPLINK_TILES as f64),
            m::UPLINK_DAMAGE_PERCENT[t],
        ),
        ModuleKind::TetherLink => format!(
            "• Active\n• Link the ally at the pointer, up to {} tiles, {} s\n• You take {} of their hits\n• {} s cooldown",
            fig(m::TETHER_RANGE_TILES as f64),
            fig(m::TETHER_SECONDS as f64),
            pc(m::TETHER_SHARE[t] as f64),
            fig(m::TETHER_COOLDOWN_SECONDS[t] as f64),
        ),
        ModuleKind::DecoyProjector => format!(
            "• Active\n• A ghost walks to the pointer, up to {} tiles, half pace, {} s\n• Enemies within {} tiles that see it shoot only it\n• {} s cooldown",
            fig(m::DECOY_RANGE_TILES as f64),
            fig(m::DECOY_SECONDS as f64),
            fig(m::DECOY_TAUNT_TILES as f64),
            fig(m::DECOY_COOLDOWN_SECONDS as f64),
        ),
        ModuleKind::AdrenalInjector => format!(
            "• Under {} health: +{}% fire rate and speed, {} s\n• {} s cooldown",
            pc(m::ADRENAL_BELOW as f64),
            m::ADRENAL_PERCENT[t],
            fig(m::ADRENAL_SECONDS[t] as f64),
            fig(m::ADRENAL_COOLDOWN_SECONDS as f64),
        ),
        ModuleKind::Overcharger => format!(
            "• +{}% weapon damage\n• Crits too",
            m::OVERCHARGE_PERCENT[t],
        ),
    }
}

/// The tier rows under an item's tooltip (October 2026): "Tier 2".
pub fn item_tier_label(tier: u32) -> String {
    format!("Tier {tier}")
}

/// What an item's tier `tier` (1 to 3) gives, short, only the numbers
/// that move with the tier: a row of the tier table under its tooltip,
/// so the player sees what the next tier adds. `None` for a kind made at
/// one tier alone (the *Override Core*, the *Reset Capacitor*): it has
/// nothing to compare.
pub fn item_tier_line(kind: bims::module::ModuleKind, tier: u32) -> Option<String> {
    use bims::module::{self as m, ModuleKind};
    if bims::combat::Tier::ALL
        .into_iter()
        .filter(|&t| kind.made_at(t))
        .count()
        < 2
    {
        return None;
    }
    let t = (tier.clamp(1, 3) - 1) as usize;
    Some(match kind {
        ModuleKind::BlinkDrive => format!(
            "{} tiles · {} s cooldown",
            fig(m::BLINK_RANGE_TILES[t] as f64),
            fig(m::BLINK_COOLDOWN_SECONDS[t] as f64),
        ),
        ModuleKind::Executioner => format!(
            "{} crits for {}",
            pc(m::EXECUTIONER_CHANCE[t] as f64),
            pc(m::EXECUTIONER_DAMAGE[t] as f64),
        ),
        ModuleKind::ReactorHeart => format!(
            "+{} health · {}% HP/s, {}% quiet",
            fig(m::HEART_HEALTH[t] as f64),
            fig(m::HEART_REGEN[t] as f64),
            fig(m::HEART_QUIET_REGEN[t] as f64),
        ),
        ModuleKind::CoolantLoop => format!("-{}% cooldowns", m::COOLANT_LOOP_PERCENT[t]),
        ModuleKind::PressureSeal => {
            format!("+{} health", fig(m::PRESSURE_SEAL_HEALTH[t] as f64))
        }
        ModuleKind::SteadyGrip => format!("+{}% fire rate", m::STEADY_GRIP_PERCENT[t]),
        ModuleKind::LongBarrel => format!("+{} tiles range", fig(m::LONG_BARREL_TILES[t] as f64)),
        ModuleKind::LeechCapacitor => format!("{} back as health", pc(m::LEECH_SHARE[t] as f64)),
        ModuleKind::ArcCoil => format!(
            "{} enemies · {} damage each",
            m::ARC_TARGETS[t],
            fig(m::ARC_DAMAGE[t] as f64),
        ),
        ModuleKind::FieldMender => format!(
            "{}% HP · {} s cooldown",
            fig(m::MENDER_HEAL[t] as f64),
            fig(m::MENDER_COOLDOWN_SECONDS[t] as f64),
        ),
        ModuleKind::ResetCapacitor => {
            format!("{} s cooldown", fig(m::RESET_COOLDOWN_SECONDS[t] as f64))
        }
        ModuleKind::AblativeShell => format!(
            "{} s · {} s cooldown",
            fig(m::SHELL_SECONDS[t] as f64),
            fig(m::SHELL_COOLDOWN_SECONDS[t] as f64),
        ),
        ModuleKind::TrainingLog => format!("+{}% experience", m::TRAINING_LOG_XP[t]),
        ModuleKind::SmokeLauncher => format!(
            "{} tiles · {} s · {} s cooldown",
            fig(2.0 * m::SMOKE_RADIUS_TILES[t] as f64),
            fig(m::SMOKE_SECONDS[t] as f64),
            fig(m::SMOKE_COOLDOWN_SECONDS[t] as f64),
        ),
        ModuleKind::TargetingUplink => format!("bots +{}% damage", m::UPLINK_DAMAGE_PERCENT[t]),
        ModuleKind::TetherLink => format!(
            "{} taken · {} s cooldown",
            pc(m::TETHER_SHARE[t] as f64),
            fig(m::TETHER_COOLDOWN_SECONDS[t] as f64),
        ),
        ModuleKind::AdrenalInjector => format!(
            "+{}% · {} s",
            m::ADRENAL_PERCENT[t],
            fig(m::ADRENAL_SECONDS[t] as f64),
        ),
        ModuleKind::Overcharger => format!("+{}% weapon damage", m::OVERCHARGE_PERCENT[t]),
        ModuleKind::OverrideCore | ModuleKind::DecoyProjector => return None,
    })
}

pub fn relic_name(relic: world::Relic) -> &'static str {
    RELIC_NAMES
        .get(relic.code() as usize)
        .copied()
        .unwrap_or("a relic")
}

/// One thing a relic does, in words, off the rules' own number: "+30%
/// weapon damage for the crew", "-20% move speed for the bots", "+2% max HP a
/// second for the crew" — the crew being the players and their bots, never
/// an enemy (the rules lift the crew's skills alone).
pub fn modifier_line(m: world::relic::Modifier) -> String {
    use world::relic::{Stat, Who};
    let sign = if m.amount < 0 { "-" } else { "+" };
    let n = m.amount.unsigned_abs();
    let what = match m.stat {
        // Not a share moved but what a bot's kill pays outright.
        Stat::BotBounty => return format!("A bot's kill pays {n}% of the enemy's money"),
        Stat::Damage => "weapon damage",
        Stat::FireRate => "fire rate",
        Stat::MoveSpeed => "move speed",
        Stat::DamageTaken => "damage taken",
        Stat::Cooldowns => "class ability cooldowns",
        Stat::MachineDamage => "damage to enemies",
        Stat::Bounty => "money for every enemy down",
        Stat::Experience => "experience for every enemy down",
        Stat::WaveSize => "machines in every wave",
        Stat::TraderPrices => "trader prices",
        Stat::Regen => "max HP a second, regenerated",
        Stat::EnemyHealth => "HP for every enemy",
        // Not a share of anything now, but of a clear's own.
        Stat::CleanExperience => {
            return format!(
                "{sign}{n}% of a site's experience again when it is cleared with no player down"
            );
        }
        Stat::NearDamage => {
            return format!(
                "{sign}{n}% weapon damage within {} tiles for the crew",
                bims::balance::NEAR_TILES
            );
        }
        Stat::FarDamage => {
            return format!(
                "{sign}{n}% weapon damage past {} tiles for the crew",
                bims::balance::FAR_TILES
            );
        }
        // Switches, not shares.
        Stat::FreeRoute => {
            return "A trip may go to any place of the next row on the map".into();
        }
        Stat::ForcedBonusWave => {
            return "Every fight that can have a bonus wave has it".into();
        }
        Stat::Interest => {
            return format!(
                "Every site cleared pays each player {n}% of the money it kept (at most {} enemies' pay)",
                world::data::WAR_CHEST_CAP_ENEMIES
            );
        }
        Stat::BonusWavePay => "experience and money from the bonus wave",
        Stat::BigDamage => "damage to big enemies (Wardens, Guardians, tier-two machines)",
        Stat::SmallDamage => "damage to every other enemy",
    };
    // The run's own numbers name nobody: they are the crew's whole.
    let whom = match (m.who, m.stat) {
        (
            Who::Everyone,
            Stat::MachineDamage
            | Stat::Bounty
            | Stat::Experience
            | Stat::WaveSize
            | Stat::TraderPrices
            | Stat::EnemyHealth
            | Stat::BonusWavePay
            | Stat::BigDamage
            | Stat::SmallDamage,
        ) => "",
        (Who::Everyone, _) => " for the crew",
        (Who::Players, _) => " for the players' Bims",
        (Who::Bots, _) => " for the bots",
    };
    format!("{sign}{n}% {what}{whom}")
}

/// Every line of a relic, each with whether it is a boon (true) or the
/// price (false), in the order the relic lists them.
pub fn relic_lines(relic: world::Relic) -> Vec<(String, bool)> {
    relic
        .modifiers()
        .iter()
        .map(|&m| (modifier_line(m), m.helps()))
        .collect()
}

pub const RELICS_HEADING: &str = "Relics";
pub const NO_RELICS: &str = "None yet. Beating an elite (a crowned site) offers relics.";
pub const REWARD_TITLE: &str = "The elite is beaten";
pub const REWARD_INTRO: &str = "• One relic for the whole crew, or none\n• Kept all run\n• Green: gain · red: cost\n• One proposes, the rest accept";
pub const TAKE_NONE: &str = "Take none";
pub const ACCEPT: &str = "Accept";
/// The end of a fight (`screens::fightwon`): up in the mission the moment
/// the site is cleared, with what the fight earned.
pub const FIGHT_WON_TITLE: &str = "Fight won";
pub const FIGHT_WON_CLEARED: &str =
    "The last of the enemy here is down. The site stays cleared, and the bounty is in the pool.";
pub const FIGHT_WON_HELD: &str =
    "The site is held against the machines, and the bounty is in the pool.";
/// The small line over the title.
pub const FIGHT_WON_KICKER: &str = "MISSION COMPLETE";
/// The tile counting every enemy killed: machines and Manufacturers alike.
pub const FIGHT_WON_ENEMIES: &str = "Enemies killed";
/// The tile with the player's own damage.
pub const FIGHT_WON_DAMAGE: &str = "Your damage";
pub const FIGHT_WON_CREW: &str = "The crew";
pub const FIGHT_WON_DAMAGE_HEAD: &str = "Damage";
pub const FIGHT_WON_XP_HEAD: &str = "Experience";
pub const FIGHT_WON_BOUNTY: &str = "Bounty paid";
pub const FIGHT_WON_POOL: &str = "Your share";
pub const FIGHT_WON_BOTS: &str = "The rest of the crew";
pub const FIGHT_WON_JOINED: &str = "Townsfolk joined";
pub const FIGHT_WON_LOST: &str = "Crew lost";
pub const FIGHT_WON_NEXT: &str = "Everything stands still — nobody moves and nobody downed bleeds out — until Back to ship takes everybody back, wherever they are. Aboard, the crew choose where to go next on the map.";
/// Under the tally for a player who has no button to press: out of the
/// fight, the others take the ship home.
pub const FIGHT_WON_WAITING: &str = "Waiting for the others to go back to the ship.";

/// A player's Bim's experience from the fight, and the levels it rose.
pub fn fight_won_xp(xp: u32, from: u8, to: u8) -> String {
    if to > from {
        format!("+{xp} xp · level {from} to {to}")
    } else {
        format!("+{xp} xp")
    }
}

/// The bots' experience from the fight, added up.
pub fn fight_won_bots_xp(xp: u32) -> String {
    format!("+{xp} xp")
}

pub const VICTORY_TITLE: &str = "The run is won";

/// The victory screen's summary of the run (feature 108), a line a number,
/// and the crew's relics under it.
pub fn victory_summary(summary: &world::world::RunSummary) -> Vec<String> {
    vec![
        format!("Days travelled: {:.1}", summary.days),
        format!("Sites cleared: {}", summary.sites_cleared),
        format!("Systems liberated: {}", summary.systems_liberated),
        format!(
            "Machines destroyed: {}",
            crate::format::grouped(u64::from(summary.machines_destroyed))
        ),
        format!("Deaths: {}", summary.deaths),
    ]
}
pub const VICTORY_RELICS: &str = "The crew's relics:";
pub fn victory_relics(relics: &[world::Relic]) -> String {
    if relics.is_empty() {
        return "none".into();
    }
    let names: Vec<&str> = relics.iter().map(|&r| relic_name(r)).collect();
    names.join(", ")
}

/// The line under a proposal: what is on the table and who has said yes.
/// A player's word on the relic on the table.
pub fn relic_answer(who: &str, yes: bool, gone: bool) -> String {
    match (gone, yes) {
        (true, _) => format!("{who}: gone"),
        (false, true) => format!("{who}: yes"),
        (false, false) => format!("{who}: …"),
    }
}

pub fn relic_proposal_line(relic: Option<world::Relic>) -> String {
    match relic {
        Some(r) => format!("On the table: {}.", relic_name(r)),
        None => "On the table: take none.".into(),
    }
}

/// A player named by their slot: their Bim's name, which is what the
/// lobby shows them as too.
fn player_name(slot: u32) -> String {
    crew_name(slot)
}

/// The departure check asking (feature 103).
pub fn departure_asked(behind: u32) -> String {
    match behind {
        1 => "One of the crew is still outside the ship. Leave them behind?".into(),
        n => format!("{n} of the crew are still outside the ship. Leave them behind?"),
    }
}

/// The ship leaving a site it cleared, and one it did not (feature 103).
pub const LEFT_CLEARED: &str = "The ship leaves. This place stays cleared.";
pub const LEFT_UNCLEARED: &str = "The ship leaves before the place is cleared: it is as the crew found it, and the bounty is lost.";
/// A site the machines were attacking, left before it was held (a town,
/// a station or a derelict, task 111).
pub const TOWN_FELL: &str = "The site falls to the machines behind you.";
/// An Area defend's two moments (October 2026).
pub const AREA_TAKEN: &str = "The machines hold the FOB. It is lost, and the run with it.";
/// The ready check's bonus-wave toggle (`worldmap::ready_window`), on and
/// off, and its tip.
pub const BONUS_WAVE_CHOSEN: &str = "Bonus wave: ON";
pub const BONUS_WAVE_NOT_CHOSEN: &str = "Bonus wave: off";
pub const BONUS_WAVE_OVERTIME: &str = "Bonus wave: ON (Overtime)";
pub const BONUS_WAVE_OVERTIME_TIP: &str = "• Overtime: every bonus wave on\n• Pays ×2";
pub const BONUS_WAVE_TIP: &str = "• One more wave, ×1.5 the size\n• +50% the site's XP, plus its bounty\n• Any player toggles\n• Resets every Ready";
pub const AREA_TIME_UP: &str = "Time! No more waves are coming — destroy the last of them.";

// --- the world map and the end of a mission (feature 103) -------------------

pub const MAP_TITLE: &str = "World map";
/// Under the title, between missions and during one.
pub const MAP_BETWEEN: &str =
    "Between missions. Choose where to go next — everybody has to accept.";
pub const MAP_READ_ONLY: &str =
    "During a mission the map is read-only. Go back to the ship to choose where next.";
/// At a trader (task 114): the visit is here, and the vote goes on.
pub const MAP_AT_TRADER: &str =
    "At a trader. Buy what you want, then choose where to go next — everybody has to accept.";
pub const MAP_TIP: &str = "• Climb from the bottom to the Machine Heart\n• 1 row = 1 day\n• Blades: attack · shield: defend · $: trader · crown: elite, relics\n• Up along the lines only\n• Bands: the enemy's tier\n• Click a place, Propose; everybody accepts\n• Wheel: scroll · Ctrl+wheel: zoom · drag: move";
/// The two halves of the list.
pub const MAP_THIS_SYSTEM: &str = "This system";
/// A way up the floor on the list (October 2026): its star and its row's day.
pub fn map_floor_next(star: &str, day: u32) -> String {
    format!("{star} · day {day}")
}

// --- the floor's chart (October 2026) ------------------------------------------

/// The crew's own station at the foot of the floor.
pub const FLOOR_START: &str = "Start";
/// The place the crew are at, and a place a trip may go to.
pub const FLOOR_HERE: &str = "you are here";
pub const FLOOR_WAY_UP: &str = "a way up from here";
pub const FLOOR_OUT_OF_REACH: &str = "not joined to where you are";
/// A row's day in the gutter.
pub fn floor_day(day: u32) -> String {
    format!("Day {day}")
}
/// A tier's band, written down the gutter.
pub fn floor_tier_band(tier: u32) -> String {
    format!("TIER {}", crate::format::roman(tier))
}
/// Where a tier's band begins.
pub fn floor_tier_from(tier: u32, day: u32) -> String {
    format!("Tier {} from day {day}", crate::format::roman(tier))
}
/// A place's day and tier, under the pointer.
pub fn floor_place_day(day: u32, tier: u32) -> String {
    format!("Day {day} · tier {}", crate::format::roman(tier))
}
/// How far up the floor the crew are.
pub fn floor_progress(day: u32, heart: u32, ascension: u32) -> String {
    let line = format!("Day {day} of {heart} — the Machine Heart on day {heart}");
    if ascension == 0 {
        line
    } else {
        format!("Ascension {ascension} · {line}")
    }
}
pub const FLOOR_FOCUS: &str = "Back to where you are";
pub const FLOOR_FOCUS_TIP: &str = "Scroll to your row";
pub const FLOOR_ZOOM_OUT: &str = "-";
pub const FLOOR_ZOOM_IN: &str = "+";
pub const FLOOR_ZOOM_TIP: &str = "Zoom (Ctrl+wheel)";
pub const FLOOR_SKETCH_CLEAR: &str = "Clear drawing";
pub const FLOOR_SKETCH_TIP: &str = "• Right-drag: draw your way\n• Shift+right-drag: rub a line out\n• This: rub out all of yours\n• Seen by all, kept across missions\n• Next stop on it picked for you";
/// A system's heading on the list, by how many hyperlanes off it is
/// (the second map rework: one or two).
pub fn map_next_system(star: &str, hops: u32) -> String {
    match hops {
        1 => format!("{star} · one hop"),
        2 => format!("{star} · two hops"),
        n => format!("{star} · {n} hops"),
    }
}
/// The list's order (task 135): by system, or every site by how long the
/// trip to it is, the nearest first.
pub const MAP_SORT_SYSTEM: &str = "By system";
pub const MAP_SORT_DISTANCE: &str = "By distance";
pub const MAP_SORT_TIP: &str = "• By system: this one, then 1–2 hops off\n• By distance: shortest trip first\n• Out of reach last";
/// The site the crew are at, in the list.
pub const MAP_HERE: &str = "here";
/// A trip within the system, which takes no time (the map rework).
pub const TRIP_FREE: &str = "no time";
/// A trip's length and the day it ends on: a day for a jump, no time
/// within a system.
pub fn trip_quote(minutes: u64, arrival_day: u32) -> String {
    if minutes == 0 {
        return format!("{TRIP_FREE} · day {arrival_day}");
    }
    let length = crate::format::trip_length(minutes);
    format!("{length} · day {arrival_day}")
}
/// What a site is to the crew (task 111), by `world::SiteKind::code`: the
/// word every row of the map's list and every icon of the system map leads
/// with, in capitals so it is read first.
pub const ARRIVE_ATTACK: &str = "ATTACK";
pub const ARRIVE_DEFEND: &str = "DEFEND";
pub const SITE_KIND_NAMES: [&str; 3] = [ARRIVE_ATTACK, ARRIVE_DEFEND, ARRIVE_TRADER];
/// A site kind's word.
pub fn site_kind_word(kind: world::SiteKind) -> &'static str {
    SITE_KIND_NAMES[kind.code() as usize]
}
/// A defence that is an Area defend (October 2026): a town's, holding its
/// FOB — its own word on the map and its own mark.
pub const ARRIVE_AREA_DEFEND: &str = "AREA DEFEND";
/// A mission's word: its kind's, or an Area defend's.
pub fn mission_kind_word(kind: world::SiteKind, area: bool) -> &'static str {
    if area && kind == world::SiteKind::Defend {
        ARRIVE_AREA_DEFEND
    } else {
        site_kind_word(kind)
    }
}
/// Where a mission is fought, beside its kind on the map: on a station's
/// deck or in a settlement on a planet (a station id that names a surface).
pub const SITE_STATION: &str = "station";
pub const SITE_PLANET: &str = "planet";
pub fn site_place_word(station: u32) -> &'static str {
    if world::surface_body(station).is_some() {
        SITE_PLANET
    } else {
        SITE_STATION
    }
}
/// The crew's money on the map, where the top frame is hidden.
pub const MAP_MONEY: &str = "Money";
pub const MAP_MONEY_TIP: &str = "Wallet + your share";
/// What a site kind means, for the `?` beside the map's list.
pub const SITE_KIND_TIP: &str = "• ATTACK (blades): clear it\n• DEFEND (shield): hold every wave, first in 5 s; no pay\n• AREA DEFEND (flag): hold the FOB 3 min, then clear; elite, relic\n• FOB: machines alone in it 20 s = run lost\n• TRADER (green square): shop, no mission\n• Relics: elites only (crown)\n• Taken systems: attack their jammer";
/// What the crew find on arrival, a word each.
pub const ARRIVE_MACHINES: &str = "machines";
pub const ARRIVE_JAMMER: &str = "jammer";
pub const ARRIVE_CLEARED: &str = "cleared";
pub fn arrive_tier(tier: u32) -> String {
    format!("tier {tier}")
}
pub const ARRIVE_QUIET: &str = "On arrival: nobody hostile.";
pub fn arrive_state(infested: bool, tier: u32, jammer: bool, threatened: bool) -> String {
    let mut words = Vec::new();
    if infested {
        words.push(format!("held by the machines at tier {tier}"));
    }
    if jammer {
        words.push("their jammer".to_string());
    }
    if threatened {
        words.push(format!("a defence at tier {tier}"));
    }
    if words.is_empty() {
        return ARRIVE_QUIET.into();
    }
    format!("On arrival: {}.", words.join(", "))
}
pub const PROPOSE: &str = "Propose";
pub const ACCEPT_TRIP: &str = "Accept";
pub const TAKE_BACK: &str = "Take back";
/// The loading screen (`screens::loading`): the run opening, and a trip's
/// site being built.
pub const LOADING_RUN: &str = "Setting out…";
pub const LOADING_MISSION: &str = "Loading mission…";
pub fn accepted_line(who: &str, yes: bool, gone: bool) -> String {
    if gone {
        format!("{who}: gone")
    } else if yes {
        format!("{who}: yes")
    } else {
        format!("{who}: waiting")
    }
}
/// A player whose Bim is dead and waiting to be bought back.
pub fn out_line(cost: u64) -> String {
    format!(
        "Your Bim is dead. It is bought back at the next mission for {} if the pool can pay.",
        crate::format::euros(cost)
    )
}
pub const BACK_TO_SHIP: &str = "Back to ship";
/// The same button once the fight is won and the deck frozen (task 133).
pub const FIGHT_WON_BACK_TO_SHIP: &str = "Fight won — Back to ship";
pub const BACK_TO_SHIP_TIP: &str = "• First press: bots go back\n• Every press: you walk back\n• Leaves when every standing player pressed and is aboard\n• Outside: left behind, dead (vote)\n• Before clear: site resets, bounty lost, XP kept\n• After a win: at once, everybody comes";
/// *Back to ship* pressed (feature 107's words for feature 103's count):
/// the players aboard who have pressed it, of the players the ship waits
/// for — `World::returning_count`, the departure check's own rule.
pub fn returning_line(home: u32, waited: u32) -> String {
    format!("Returning · {home} / {waited}")
}
pub const ASK_AGAIN: &str = "Ask again";
pub const DEPARTURE_TITLE: &str = "Leave them behind?";
pub const DEPARTURE_LINE: &str =
    "The ship is ready to go, but these are still outside it. Left behind is dead.";
pub const LEAVE_YES: &str = "Yes, leave";
pub const LEAVE_NO: &str = "No, wait";
pub fn departure_answer(who: &str, answer: Option<bool>, gone: bool) -> String {
    match (gone, answer) {
        (true, _) => format!("{who}: gone"),
        (false, Some(true)) => format!("{who}: leave"),
        (false, Some(false)) => format!("{who}: wait"),
        (false, None) => format!("{who}: …"),
    }
}
/// The ready check: a mission with a fight in it waits for every player.
pub fn ready_title(kind: world::SiteKind) -> String {
    site_kind_word(kind).to_string()
}
pub const READY_CHECK: &str = "Ready check";
pub const READY_LINE: &str = "The mission starts when every player is ready. Change your loadout and spend your skill points now if you want to.";
pub const READY_YES: &str = "Ready";
pub const READY_NO: &str = "Not ready";
/// What a player's card in the ready check says under its name.
pub fn ready_state(ready: bool, gone: bool) -> &'static str {
    match (gone, ready) {
        (true, _) => "Gone",
        (false, true) => "Ready",
        (false, false) => "Waiting",
    }
}
/// How many of the players are ready, under the cards.
pub fn ready_count(ready: u32, of: u32) -> String {
    format!("{ready} of {of} ready")
}
/// The machines' own station where a system has none, named by nobody.
pub const DERIVED_JAMMER_NAME: &str = "The machines' relay";

/// The Machine Heart on the map (feature 108): its fortress's name, its tag
/// in the list, and the rows of its card — its strength on arrival.
pub const HEART_NAME: &str = "The Machine Heart";
pub const ARRIVE_HEART: &str = "the Machine Heart";
pub const HEART_ON_ARRIVAL: &str = "On arrival:";

/// The trader (task 114): its tag on the map, why one is shut, and the
/// Trader panel's every word.
pub const ARRIVE_TRADER: &str = "TRADER";
pub const TRADER_TIP: &str = "• On the map: no mission, clocks stop\n• New shelf every visit\n• No relics\n• Closed while its system is taken";
pub const TRADER_CLOSED: &str = "closed";
pub const TRADER_CLOSED_ON_ARRIVAL: &str = "closed on arrival";
pub const TRADER_TITLE: &str = "Trader";
/// The Trader panel is drawn as a purchase order: the form's name under
/// the title, and its number — the trader's star and station.
pub const TRADER_FORM: &str = "Purchase order";
pub fn trader_form_no(star: u32, station: u32) -> String {
    format!("No. {star:04}-{station:02}")
}
pub const TRADER_DELIVER_TO: &str = "Deliver to";
/// A stamp by the title where the machines are near; the long line
/// (`front_premium`) is its hover.
pub const TRADER_FRONT_STAMP: &str = "Front prices";
/// The total line at the foot of the form.
pub const TRADER_BALANCE: &str = "Your balance";
pub const TRADER_INTRO: &str = "• Your shelf, your money\n• Onto your Bim or into your armory\n• What it replaces: armory\n• Each kind T1, then a tier past your best\n• Bought: gone till next visit\n• Tab: Armory";
pub const TRADER_WEAPONS: &str = "Weapons";
pub const TRADER_ARMOUR: &str = "Armour";
/// The item shelf (October 2026): every item at its lowest tier, whatever
/// the zone; one of each a visit.
pub const TRADER_ITEMS: &str = "Items";
pub const TRADER_ITEMS_INTRO: &str = "• Tier one, one each a visit\n• Into your Bim's first free slot\n• Owned: Upgrade a tier, next tier's price\n• Bots carry none";
/// The items column's three groups (October 2026), in the order they
/// stand: what an item does for the Bim.
pub const TRADER_ITEMS_UTILITY: &str = "Utility";
pub const TRADER_ITEMS_DAMAGE: &str = "Damage";
pub const TRADER_ITEMS_DURABILITY: &str = "Durability";
/// The search field over the trader's columns, and a column it empties.
pub const TRADER_SEARCH_HINT: &str = "Search…";
pub const TRADER_SEARCH_TIP: &str = "Filter by name, column or group";
pub const TRADER_SEARCH_CLEAR: &str = "×";
pub const TRADER_NO_MATCH: &str = "Nothing matches.";
pub const TRADER_SOLD: &str = "SOLD";
pub const TRADER_BUY: &str = "Buy";
/// The button on an item line the player's own Bim carries one of: it
/// goes a tier up in its slot.
pub const TRADER_UPGRADE: &str = "Upgrade";
/// The note under such a line.
pub const TRADER_UPGRADE_NOTE: &str = "yours, a tier up";
/// The hover over such a line.
pub const TRADER_UPGRADE_TIP: &str = "• You carry this\n• Buy: a tier up, same slot";
/// The note under a shelf line of the weapon or armour the Bim it goes to
/// wears a tier under: bought onto it, the old one is sold at once.
pub const TRADER_SHELF_UPGRADE_NOTE: &str = "better than the one worn";
/// The hover over such a line.
pub const TRADER_SHELF_UPGRADE_TIP: &str = "• Better than the one worn\n• Onto the Bim: the old one sold at once\n• Into the armory: the old one stays on";
/// The stamp on an item line the player's own Bim carries at its top.
pub const TRADER_TOP: &str = "MAX";
pub const TRADER_INTO_ARMORY: &str = "Armory";
/// The trader's two tabs (October 2026): buying, and selling back.
pub const TRADER_TAB_BUY: &str = "Buy";
pub const TRADER_TAB_SELL: &str = "Sell";
pub const TRADER_SELL: &str = "Sell";
pub const TRADER_SELL_INTRO: &str = "• Half of what it cost\n• Items: half of all paid, upgrades too\n• Weapons, armour: half today's price\n• Leaves the slot empty";
pub const TRADER_SELL_NONE: &str = "Nothing to sell.";
/// The note on a line the player's own Bim wears or carries.
pub const TRADER_SELL_WORN: &str = "equipped — yours";
/// Where a thing to sell is.
pub fn sell_from(worn_by: Option<&str>) -> String {
    match worn_by {
        Some(who) => format!("on {who}"),
        None => "in the armory".into(),
    }
}

pub const MANUFACTURER_DOWN: &str = "A Manufacturer is dead.";
pub const ARRIVE_MANUFACTURERS: &str = "Manufacturers";
/// An elite (`world::elite`), in a row of the map's list.
pub const ARRIVE_ELITE: &str = "elite · Guardian in wave 2 · relics";
/// The Heart's card: its links and its core, then a row a link — what
/// shooting it down brings in (October 2026: a wave and its Guardians).
pub fn heart_preview_rows(p: &world::heart::HeartPreview) -> Vec<(String, String)> {
    let mut rows = vec![
        ("Links".to_string(), p.conduits.to_string()),
        (
            "Core".to_string(),
            crate::format::grouped(p.core_health.max(0.0).ceil() as u64),
        ),
    ];
    for link in 1..=p.conduits {
        let guardians = world::heart::guardians_for_link(link);
        rows.push((
            format!("Link {link}"),
            format!(
                "{} machines + {guardians} Guardian{}",
                p.wave,
                if guardians == 1 { "" } else { "s" }
            ),
        ));
    }
    rows
}
/// Under the Heart's rows: the rule the links keep.
pub const HEART_LINKS_SEALED: &str = "A link's wave must be down before the next link can fall.";
/// Beside a name in the departure check: down and cannot walk in.
pub const DOWNED_WORD: &str = "down";
/// The departure list's cost column for a bot: it costs nothing, only
/// gone.
pub const BOT_GONE_WORD: &str = "gone for good";

// --- the HUD (feature 107) ----------------------------------------------------

/// The top frame: the day, the bounty waiting on the site being cleared,
/// and the pause.
pub fn day_word(day: u32) -> String {
    format!("Day {day}")
}
pub fn on_clear_line(pending: u64) -> String {
    format!("+{} on clear", crate::format::euros(pending))
}
pub const PAUSED_CHIP: &str = "Paused";
/// The recruited warning, which the old strip spelt in place.
pub const RECRUITED_STATUS: &str = "Recruited — the crew follow you";

/// The banner over the canvas for a player whose Bim is out.
pub const OUT_BANNER: &str = "You're out";
pub fn out_banner_line(cost: u64, pool: u64) -> String {
    format!(
        "buyback {} · pool {}",
        crate::format::euros(cost),
        crate::format::euros(pool)
    )
}

/// Under a portrait instead of its level: dead, or out until bought back.
pub const PORTRAIT_OUT: &str = "out";
/// What resting on a portrait says: the name, the class and the level,
/// and whatever marker it wears.
pub fn portrait_tip(
    name: &str,
    class: &str,
    level: Option<u8>,
    downed: bool,
    out: bool,
    returning: bool,
) -> String {
    let mut words = match level {
        Some(level) => format!("{name} · {class}, level {level}"),
        None => format!("{name} · {class}"),
    };
    if out {
        words.push_str(" · out");
    } else if downed {
        words.push_str(" · down");
    }
    if returning && !out {
        words.push_str(" · returning");
    }
    words
}

/// The hero panel's experience line, and what stands for it without a
/// class.
pub fn hero_xp_line(level: u8, into: u32, of: u32) -> String {
    format!("Lv {level} · {into} / {of} XP")
}
pub fn hero_xp_max(level: u8) -> String {
    format!("Lv {level} · Max")
}
/// The number inside the hero panel's experience bar: what is in of what
/// the level wants, and `Max` at the top.
pub fn xp_bar_number(into: u32, of: u32) -> String {
    format!("{into} / {of} XP")
}
pub const XP_BAR_MAX: &str = "Max";
/// Under the level in its circle: the experience still wanted for the
/// next level.
pub fn xp_to_go(left: u32) -> String {
    format!("{left} to go")
}
pub const NO_CLASS: &str = "No class";
/// The hero panel greyed over while its Bim is down.
pub const DOWNED_BANNER: &str = "Downed";
/// What the hero's health bar's tooltip says while it is critically
/// hit (feature 110).
pub const CRITICAL_TIP: &str = "• Under 20 HP: bleeding\n• Get a medic's beam on it, or get out";
/// What a downed body has left, in one line — the hero panel's and the
/// portraits' (task 120).
pub fn downed_short(seconds: f32) -> String {
    format!("DOWNED — dies in {} s", seconds.ceil().max(0.0) as u32)
}

/// A line of the log for experience gained, and what it is gathered by.
pub fn xp_gain_line(source: &str, xp: u32) -> String {
    format!("{source} · +{xp} XP")
}
pub const XP_FROM_MACHINES: &str = "Machines down";
pub const XP_FROM_WORK: &str = "Experience";

/// A word with the key that does the same beside it.
pub fn with_key(word: &str, key: &str) -> String {
    format!("{word} ({key})")
}

/// The tray's buttons.
pub const TRAY_ARMORY: &str = "Armory";
pub const TRAY_SQUAD: &str = "Squad";
pub const TRAY_MAP: &str = "Map";
/// The Stash panel.
// The Armory panel (task 113): every crew member's loadout and the
// ship's armory, money and keys.
pub const ARMORY_TITLE: &str = "Armory";
pub const ARMORY_HOW: &str = "Drag a thing onto a Bim to put it on, or onto the armory to take it off. Yours onto another player's Bim is an offer. Right-click for the same.";
pub const ARMORY_LOCKED: &str = "In a mission: drag a thing onto a Bim inside the ship to put it on. A Bim out on the deck keeps what it has, and offers wait for the map.";
pub const ARMORY_BOT: &str = "bot";
/// The Armory panel's rows: the players' own Bims, then the bots.
pub const ARMORY_PLAYERS: &str = "Players";
pub const ARMORY_BOTS: &str = "Bots";
pub const ARMORY_EMPTY_SLOT: &str = "—";
pub const ARMORY_TAKE_OFF: &str = "Take off, into the armory";
pub const ARMORY_OFFER_TO: &str = "Offer to";
pub const ARMORY_PUT_ON: &str = "Put on";
pub const ARMORY_OFFERS: &str = "offers:";
pub const ARMORY_ACCEPT: &str = "Accept";
pub const ARMORY_DECLINE: &str = "Decline";
pub const ARMORY_OFFERED_TO: &str = "offered to";
pub const ARMORY_TAKE_BACK: &str = "Take back";
pub const ARMORY_STOCK: &str = "Your armory";
pub const ARMORY_NOTHING: &str = "Nothing in your armory.";
/// The log's line for a loadout changed between missions.
pub const GEAR_CHANGED: &str = "Gear changed hands.";
/// The Squad panel.
pub const SQUAD_FOLLOWING: &str = "Crew: following you";
pub const SQUAD_ATTACK: &str = "Attack";
pub const SQUAD_RETREAT: &str = "Retreat";
pub const SQUAD_FOLLOW: &str = "Follow me";
pub const SQUAD_NO_BOTS: &str = "No bots in the crew.";
pub fn bot_class_line(class: world::Class, level: Option<u8>) -> String {
    match level {
        Some(level) => format!("{} · {level}", class_name(class)),
        None => class_name(class).to_string(),
    }
}

/// The character sheet.
pub fn sheet_title(class: &str, level: u8) -> String {
    format!("{class} · level {level}")
}
pub const SHEET_CLOSE: &str = "Close the character sheet";
pub const SHEET_BODY: &str = "Body";
pub const SHEET_GEAR: &str = "Gear";
pub const SHEET_SKILLS: &str = "Skills";
pub const SHEET_HEALTH: &str = "Health";
pub const NOTHING_WORN: &str = "nothing worn";
pub const NOTHING_IN_HAND: &str = "nothing in hand";
/// The health against what it can hold, with what armour adds.
pub fn health_line(left: f32, most: f32, bonus: f32) -> String {
    if bonus > 0.0 {
        format!("{} / {} + {}", left.round(), most.round(), bonus.round())
    } else {
        format!("{} / {}", left.round(), most.round())
    }
}
/// A piece worn: its name, tier and how much of it is left.
pub fn worn_piece_line(name: &str, tier: u32, health: f32, most: f32) -> String {
    format!("{name} T{tier} · {} / {}", health.round(), most.round())
}
/// The weapon in hand, and its tier.
pub fn held_weapon_line(name: &str, tier: u32) -> String {
    format!("{name} T{tier}")
}

/// The world map's column.
pub const MAP_CLOSE: &str = "Close";
/// The column pops out from the right edge and is retracted to begin
/// with, leaving the width to the charts: the tab that brings it out,
/// and the one on its head that puts it away.
pub const MAP_COLUMN_OPEN: &str = "« Destinations";
pub const MAP_COLUMN_OPEN_TIP: &str = "Day, money, destinations, quotes";
pub const MAP_COLUMN_SHUT: &str = "»";
pub const MAP_COLUMN_SHUT_TIP: &str = "Hide the column";
/// The Trader panel shut to look at the map, and the button that
/// brings it back.
pub const TRADER_SHUT: &str = "×";
pub const TRADER_SHUT_TIP: &str = "• Hide, to see the map\n• Trader button: back";
pub const TRADER_REOPEN: &str = "Trader";
pub const TRADER_REOPEN_TIP: &str = "Back to the trader";
pub const MAP_DAY: &str = "Day";
pub const MAP_POOL: &str = "Your money";
pub const MAP_PICK_HINT: &str =
    "Pick a place on the list or on the chart: the trip is quoted here, and put to the crew.";
/// The bar at the foot of the map (the second map rework): the trip picked and the
/// button that puts it to the crew, or the one on the table and the
/// answers to it.
pub const PROPOSE_NOTHING: &str = "Pick a place on the chart or the list to go to.";
pub const PROPOSE_TIP: &str = "• Put it to the crew\n• Everybody accepts";
pub fn propose_trip(site: &str) -> String {
    format!("Propose · {site}")
}
pub fn on_the_table(who: &str, site: &str) -> String {
    format!("{who} proposed {site}")
}

pub const BUYBACK_HEADING: &str = "Buyback";
pub const BUYBACK_COVERED: &str = "covered";
pub const BUYBACK_SHORT: &str = "not covered";
/// The destination card's rows.
pub const CARD_HOPS: &str = "Where";
pub const CARD_TRAVEL: &str = "Travel";
pub const CARD_ARRIVAL: &str = "Arrive on day";
pub fn hops_words(hops: u32) -> String {
    match hops {
        0 => "this system".into(),
        1 => "one hop away".into(),
        2 => "two hops away".into(),
        n => format!("{n} hops away"),
    }
}
pub fn days_words(days: f64) -> String {
    if days <= 0.0 {
        return TRIP_FREE.to_string();
    }
    let days = days.round() as u64;
    if days == 1 {
        "1 day".to_string()
    } else {
        format!("{days} days")
    }
}
pub fn proposed_by(who: &str) -> String {
    format!("Proposed by {who}")
}
/// What leaving a player's Bim behind costs: it is out until bought back.
pub fn buyback_cost(cost: u64) -> String {
    format!("buyback {}", crate::format::euros(cost))
}

/// Star words: 48, the generator's `STAR_WORDS`. A star is "Word-Number".
pub const STAR_WORDS: [&str; 48] = [
    "Tanis", "Vesper", "Halden", "Orrin", "Cassel", "Marrow", "Ilex", "Sorrel", "Brannoc",
    "Kestrel", "Ashby", "Corvane", "Dunmere", "Ferris", "Galt", "Harrow", "Isolde", "Jarrah",
    "Kell", "Lorne", "Maund", "Nerys", "Ostrel", "Perrin", "Quill", "Rath", "Selk", "Tarn",
    "Ulric", "Varn", "Wendel", "Yarrow", "Ambrel", "Bright", "Calder", "Dray", "Elm", "Fenwick",
    "Gorse", "Hollin", "Ivory", "Juniper", "Kirsch", "Lund", "Mossley", "Nook", "Orme", "Pell",
];

/// Station words: 32, the generator's `STATION_WORDS`. A station is "Word
/// Number", with its mark after if it has one.
pub const STATION_WORDS: [&str; 32] = [
    "Cordell Yard",
    "Meridian Dock",
    "Hask Platform",
    "Verity Ring",
    "Stannard Halt",
    "Lowry Anchorage",
    "Pike Terminal",
    "Dawes Berth",
    "Kite Reach",
    "Fallow Point",
    "Greave Works",
    "Ashlar Hub",
    "Tolley Landing",
    "Wren Gantry",
    "Copper Cross",
    "Sable Moor",
    "Harkin Depot",
    "Ember Station",
    "Ninefold Yard",
    "Quarry Spur",
    "Rook Haven",
    "Tamsin Dock",
    "Ullage Post",
    "Vane Outlook",
    "Wick Refuge",
    "Yeoman Reach",
    "Zephyr Hold",
    "Brindle Wharf",
    "Carrow Deep",
    "Dolan Rest",
    "Eyrie Platform",
    "Fisk Terminus",
];

/// A star's name off its three numbers.
pub fn star_name(name: worldgen::Name) -> String {
    let word = STAR_WORDS
        .get(name.word as usize)
        .copied()
        .unwrap_or("Star");
    format!("{word}-{}", name.number)
}

/// A station's name off its three numbers.
pub fn station_name(name: worldgen::Name) -> String {
    let word = STATION_WORDS
        .get(name.word as usize)
        .copied()
        .unwrap_or("Station");
    if name.part == worldgen::name::NO_NUMBER {
        format!("{word} {}", name.number)
    } else {
        format!("{word} {} Mk {}", name.number, name.part)
    }
}

// --- the room's words ------------------------------------------------------

pub const HEALTH_TIP: &str = "• A hit takes armour first: protection off, the rest drains it\n• Blue: armour · cyan: shield, drains\n• Under 20 HP: bleeding\n• 0: downed, dies in 30 s\n• Revive: ally beside, 10 s (medic 4 s)\n• Up at 30%\n• Armour at 0: broken till next mission";

// --- down, and the revive (task 120) -----------------------------------------
//
// The block under the health bar: the one framed thing on the panel, for
// a body that is down with the countdown running. The words
// are here; the numbers are the room's (`bims::health`).

/// The headline over the block while the body is down.
pub const DOWNED_HEAD: &str = "DOWNED";
/// How long it has, counting down.
pub fn downed_left(seconds: f32) -> String {
    format!("dies in {} s", seconds.ceil().max(0.0) as u32)
}
/// What brings it back.
pub fn downed_remedy() -> String {
    format!(
        "A crewmate standing beside it brings it round — {} seconds with hands on, a medic's {}.",
        bims::health::REVIVE_SECONDS.round() as u32,
        world::class::MEDIC_REVIVE_SECONDS.round() as u32
    )
}
/// Who is at it, while somebody is.
pub fn downed_reviver(who: &str) -> String {
    format!("{who} is bringing it round.")
}
pub const DOWNED_TIP: &str = "• 0 HP: down, helpless, not shot at\n• An ally beside revives\n• Dies when the countdown ends\n• Up at 30%";
/// A Bim under [`bims::health::BLEEDS_UNDER`]: bleeding on the deck.
pub const BADLY_HURT: &str = "Badly hurt — bleeding";
/// What a dead body says under its name.
pub fn died_line(name: &str) -> String {
    format!("{name} has died.")
}

/// The revive row on the menu over a crewmate's body, and why it is
/// greyed when it is.
pub const REVIVE_ROW: &str = "Get up";
pub fn revive_hint(seconds: f32) -> String {
    format!("walk over and bring them round — {seconds:.0} seconds with hands on")
}
pub const REVIVE_NOT_DOWN: &str = "only a downed crewmate can be revived";
pub const REVIVE_YOURSELF: &str = "nobody revives themselves — a crewmate has to";
pub const REVIVE_CARRIED: &str = "not while somebody is carrying them — set them down first";
pub fn revive_taken(who: &str) -> String {
    format!("{who} is already bringing them round")
}
/// The carry row beside it, and why it is greyed when it is.
pub const CARRY_ROW: &str = "Carry";
pub const CARRY_ROW_HINT: &str =
    "walk over and pick them up out of the fire — you hold your fire while you carry";
pub const CARRY_YOURSELF: &str = "nobody carries themselves";
pub const CARRY_NOT_DOWN: &str = "only a downed crewmate can be carried";
pub const CARRY_MEDICS_ONLY: &str = "only a medic or a field medic can carry";
pub const CARRY_ARMS_FULL: &str = "your arms are full — set them down first";
pub const CARRY_TAKEN: &str = "somebody is already carrying them";
/// What the log says when the revive key is held with nobody down close
/// enough to get up.
pub const REVIVE_NOBODY_NEAR: &str = "Nobody down close enough to get up.";
/// The quickselect in the hero panel (task 138): the one in hand lit,
/// and one key swapping them (October 2026).
pub const HAND_WEAPON: &str = "Weapon";
pub const HAND_MEDKIT: &str = "Medkit";
pub const HAND_WEAPON_TIP: &str = "• Fires as ever\n• An attack order takes it up";
pub const HAND_MEDKIT_TIP: &str =
    "• Holds fire\n• Right-click a downed ally: revive\n• Their countdown stops";
/// Under the two: the key that swaps them.
pub fn hand_swap_key(key: &str) -> String {
    format!("{key}: swap")
}
/// The trigger's clocks under the hero panel's experience bar: the wait
/// between two shots and a magazine's reload.
pub const TRIGGER_FIRE: &str = "Fire";
pub const TRIGGER_RELOAD: &str = "Reload";
/// Seconds to the hundredth, as the trigger's clocks count: "0.50s".
pub fn trigger_seconds(seconds: f32) -> String {
    format!("{:.2}s", seconds.max(0.0))
}
/// An item's tip in the hero panel (October 2026): its name with its
/// tier, then what it does (an active one says so first).
pub fn module_tip(item: bims::module::Module) -> String {
    let name = item_name(item.kind);
    let name = match tier_word(item.tier).filter(|_| item.kind.tiered()) {
        Some(tier) => format!("{name} — {tier}"),
        None => name.to_string(),
    };
    format!("{name}\n{}", item_line(item))
}
/// The countdown's seconds over a downed body on the deck.
pub fn downed_seconds(seconds: f32) -> String {
    format!("{}", seconds.ceil().max(0.0) as u32)
}

/// Why a Revive row is greyed when the helper cannot do it: the crew
/// member you steer is dead, down, or outside in a suit.
pub const HELPER_OUT: &str = "not from where the Bim is";

/// The greyed line under a fixture menu's rows while the Bim has
/// something on: a row clicked with Shift held waits its turn behind it
/// rather than taking over (feature 69), and so does a right-click on the
/// deck.
pub const SHIFT_LATER: &str = "Shift-click: afterwards";
pub const SHIFT_LATER_HINT: &str =
    "a row or a spot on the deck given with Shift waits its turn behind what the Bim is on";

/// Why a Revive row is greyed for a patient outside in a suit: nobody
/// can walk to it there.
pub const PATIENT_OUT: &str = "not while the patient is outside — it comes in first";

/// The end of the run (`screens::game::over`): the title, the line under
/// it, and the way back.
pub const OVER_TITLE: &str = "The crew are down";
pub const OVER_LINE: &str = "Nobody of the crew is standing. The run is over.";
/// The same screen when the machines took an Area defend's FOB.
pub const OVER_TITLE_AREA: &str = "The FOB has fallen";
pub const OVER_LINE_AREA: &str =
    "The machines held the ring for twenty seconds with nobody of yours in it. The run is over.";
pub const OVER_BACK: &str = "Back to the menu";

/// Months of the ship's calendar. Twelve of them and no leap years — see
/// `crates/game/src/clock.rs`, which does the arithmetic; these are only
/// the words.
pub const MONTH_NAMES: [&str; 12] = [
    "January",
    "February",
    "March",
    "April",
    "May",
    "June",
    "July",
    "August",
    "September",
    "October",
    "November",
    "December",
];

/// What `spot_at` says is under the pointer. Must match the `SPOT_` codes
/// in `crates/game/src/room.rs`.
pub const SPOT_NAMES: [&str; 21] = [
    "Outside the hull",
    "Deck plating",
    "Bulkhead",
    "Worktop",
    "Chopping board",
    "Cold store",
    "Hob",
    "Dishwasher",
    "Table",
    "Chair",
    "Bunk",
    "Hydroponic bay",
    "Toilet",
    "Washbasin",
    "Deck plating",
    "Broom locker",
    "Door",
    "Workbench",
    "Suit locker",
    "Shower",
    "Research desk",
];

/// The spots that are only a word for *where* the pointer is — outside,
/// deck, a bulkhead — rather than a thing the room has a picture of. Aboard
/// the ship every part the room does not draw reads as one of these, so the
/// ship's readout names those off the design instead.
pub const PLAIN_SPOTS: [u32; 4] = [
    bims::room::SPOT_NOTHING,
    bims::room::SPOT_DECK,
    bims::room::SPOT_BULKHEAD,
    bims::room::SPOT_HEADS_DECK,
];

// --- arms and armour ------------------------------------------------------------

/// What a weapon is called, indexed by `bims::combat::WeaponKind::code`;
/// `0` is an empty slot. Codes six to eight are the machines' built-in arms
/// (features 83 and 100): they are named here because the picture names what it
/// draws, and nowhere else — nothing carries one. Nine and ten are the two
/// kinds made only from a tier up (task 115), and eleven the Lancer's rail
/// (task 157).
pub const WEAPON_NAMES: [&str; 12] = [
    "—",
    "Laser pistol",
    "Shotgun",
    "Auto rifle",
    "Sniper rifle",
    "Schword",
    "Claw",
    "Unmaker",
    "Sweeper",
    "Minigun",
    "Rail lance",
    // The Lancer's built-in rail (task 157).
    "Rail",
];

/// What a piece of armour is called, indexed by `bims::combat::ArmourKind::code`;
/// `0` is an empty slot. There is one kind since October 2026: the armour,
/// where there were a helm, kevlar and leg guards, arc greaves and a
/// Reflective plate.
pub const ARMOUR_NAMES: [&str; 2] = ["—", "Armour"];

/// A number to the hundredth, the trailing noughts dropped — a piece's
/// protection, which a tier's factor leaves at 1.35 or 2.7.
pub fn tidy_hundredths(x: f32) -> String {
    let text = format!("{:.2}", (x * 100.0).round() / 100.0);
    text.trim_end_matches('0').trim_end_matches('.').to_string()
}

pub fn weapon_name(kind: Option<bims::combat::WeaponKind>) -> &'static str {
    WEAPON_NAMES[kind.map(|k| k.code() as usize).unwrap_or(0)]
}

pub fn armour_name(kind: Option<bims::combat::ArmourKind>) -> &'static str {
    ARMOUR_NAMES
        .get(kind.map(|k| k.code() as usize).unwrap_or(0))
        .copied()
        .unwrap_or("Armour")
}

/// A loadout's two slots, by `world::GearSlot::code`: the weapon and the
/// armour (October 2026: there were a head, a body and legs).
pub const SLOT_NAMES: [&str; 8] = [
    "Weapon", "Armour", "Item 1", "Item 2", "Item 3", "Item 4", "Item 5", "Item 6",
];

/// The hero panel's magazine column's tooltip.
pub fn magazine_tip(left: u32, size: u32) -> String {
    format!("• {left}/{size}\n• Empty: reloads itself\n• Reload key: sooner\n• Endless magazines")
}

/// A number to the tenth, as a gear table says it: `8.75` is "8.8".
fn fig1(v: f64) -> String {
    fig((v * 10.0).round() / 10.0)
}

/// A percentage to the tenth: `0.95625` is "95.6%".
fn pc1(v: f64) -> String {
    format!("{}%", fig1(v * 100.0))
}

/// A row of a weapon's or a piece's table: a value a tier in `tiers`,
/// every tier's equal ones folded into one, as an ability's ranks are.
fn tier_row(
    label: &'static str,
    unit: &'static str,
    tiers: &[bims::combat::Tier],
    value: impl Fn(bims::combat::Tier) -> String,
) -> Stat {
    let mut values: Vec<String> = tiers.iter().map(|&t| value(t)).collect();
    if values.iter().all(|v| *v == values[0]) {
        values.truncate(1);
    }
    Stat {
        label,
        values,
        unit,
    }
}

/// The tier row heading a table, and where `tier` stands among `tiers`
/// counted from one — what [`crate::theme::stat_rows`] lights.
fn tier_head(tiers: &[bims::combat::Tier], tier: bims::combat::Tier) -> (Stat, u8) {
    let head = Stat {
        label: "Tier",
        values: tiers.iter().map(|t| t.code().to_string()).collect(),
        unit: "",
    };
    let lit = tiers
        .iter()
        .position(|&t| t == tier)
        .map_or(0, |i| i as u8 + 1);
    (head, lit)
}

/// A weapon's numbers, every tier it is made at side by side (the
/// abilities' tooltip, task 124): the tier row, then only numbers. The
/// table and the place of the weapon's own tier in it, to light.
pub fn weapon_stats(weapon: bims::combat::Weapon) -> (Vec<Stat>, u8) {
    use bims::combat::Tier;
    let kind = weapon.kind;
    let tiers: Vec<Tier> = Tier::ALL.into_iter().filter(|&t| kind.made_at(t)).collect();
    let at = |t: Tier| kind.at(t).stats();
    let (head, lit) = tier_head(&tiers, weapon.tier);
    let mut rows = vec![head];
    let melee = at(weapon.tier).melee;
    rows.push(tier_row("Damage", "", &tiers, |t| {
        fig1(at(t).damage as f64)
    }));
    if tiers.iter().any(|&t| at(t).damage_far != at(t).damage) {
        rows.push(tier_row("Damage far", "", &tiers, |t| {
            fig1(at(t).damage_far as f64)
        }));
    }
    rows.push(tier_row("Range", " tiles", &tiers, |t| {
        fig(at(t).range as f64)
    }));
    if !melee {
        if tiers.iter().any(|&t| at(t).sweet < at(t).range) {
            rows.push(tier_row("Falloff from", " tiles", &tiers, |t| {
                fig(at(t).sweet as f64)
            }));
        }
        rows.push(tier_row("Accuracy", "", &tiers, |t| {
            pc1(at(t).accuracy as f64)
        }));
        if tiers.iter().any(|&t| at(t).accuracy_far != at(t).accuracy) {
            rows.push(tier_row("Accuracy far", "", &tiers, |t| {
                pc1(at(t).accuracy_far as f64)
            }));
        }
        if at(weapon.tier).burst > 1 {
            rows.push(tier_row("Burst", "", &tiers, |t| at(t).burst.to_string()));
        }
    }
    // The wait between two pulls, as the hero panel counts it down: a
    // pistol's click cooldown, every other weapon's trigger rate.
    rows.push(tier_row("Fire cooldown", " s", &tiers, |t| {
        if kind.semi_automatic() {
            fig(bims::balance::SEMI_AUTO_COOLDOWN as f64)
        } else {
            fig(1.0 / at(t).fire_rate.max(1e-3) as f64)
        }
    }));
    if at(weapon.tier).magazine > 0 {
        rows.push(tier_row("Magazine", "", &tiers, |t| {
            at(t).magazine.to_string()
        }));
        rows.push(tier_row("Reload", " s", &tiers, |t| {
            fig(at(t).reload_time as f64)
        }));
    }
    (rows, lit)
}

/// A piece of armour's numbers, every tier side by side, as
/// [`weapon_stats`]'s.
pub fn armour_stats(kind: bims::combat::ArmourKind, tier: bims::combat::Tier) -> (Vec<Stat>, u8) {
    use bims::combat::{Piece, Tier};
    let tiers = Tier::ALL;
    let at = |t: Tier| Piece::new(0, kind, t);
    let (head, lit) = tier_head(&tiers, tier);
    let rows = vec![
        head,
        tier_row("Health", "", &tiers, |t| {
            format!("+{}", fig1(at(t).stats().health as f64))
        }),
        tier_row("Protection", "", &tiers, |t| {
            fig(at(t).stats().protection as f64)
        }),
        tier_row("Dodge", "", &tiers, |t| pc1(at(t).dodge() as f64)),
    ];
    (rows, lit)
}

/// The Inventory's status for a Bim locked in a melee — a blade within
/// reach — and the header's: no firing until one of them is out of reach.
pub const LOCKED_STATUS: &str = "locked in melee";

/// Why, with the room's numbers rather than a copy of them.
pub fn locked_tip() -> String {
    use bims::combat::{FIST_DAMAGE, MELEE_PERIOD, WeaponKind};
    format!(
        "• Blade in reach: no gun\n• Fists: {} every {} s\n• Schword: {}\n• Free once out of reach",
        FIST_DAMAGE,
        MELEE_PERIOD,
        WeaponKind::Schword.stats().damage
    )
}

/// What each thing in a grid cell is, for the tooltip under its icon,
/// indexed by `physics::ResourceId`. A piece of armour's numbers are put
/// after its line by the grid, off the piece itself.
pub const ITEM_TIPS: [&str; 22] = [
    "• Food",
    "• Food",
    "• For a walk outside",
    "• Sidearm",
    // The medkit and the bandage are still resources — the trade's book
    // has a row for each — but nothing uses either since task 120: a
    // downed crewmate is revived by hand.
    "• Unused",
    "• Unused",
    // 6 and 8 were the helm and the leg guards (gone October 2026).
    "",
    "• Whole body\n• Takes every hit first",
    "",
    "• Hard up close",
    "• Steady fire while held",
    "• Longest reach",
    "• Laser blade, arm's length",
    // 13 and 14 were the research keys (gone October 2026).
    "",
    "",
    // 15 to 17: gone (task 127).
    "",
    "",
    "",
    "• T2 and up\n• 10 shots/s, 100 a magazine, 4 s reload\n• Shreds machines, weak on armour\n• Crew only",
    "• T3 only\n• 1 slug / 5 s, through every enemy in line\n• Stopped by walls, a Guardian's shield\n• Crew only",
    // 20 and 21 were the arc greaves and the Reflective plate.
    "",
    "",
];

pub fn item_tip(id: ResourceId) -> &'static str {
    ITEM_TIPS.get(id as usize).copied().unwrap_or("")
}

/// The Trader panel's line at a trader near the front (feature 94, task 114): the
/// guns, the armour and the medicine here are dearer than they are
/// anywhere quieter, and how much dearer is how near the machines are.
/// `front_premium` takes the hops.
pub const FRONT_PREMIUM_TIP: &str = "• Weapons and armour dearer";
pub fn front_premium(hops: u16) -> String {
    match hops {
        1 => "• Infection 1 hop away".to_string(),
        n => format!("• Infection {n} hops away"),
    }
}

/// The red warning along the top while the station alongside is held by
/// the machines (feature 83): which wave is on the deck and how many are
/// standing, then the countdown to the next one landing (`span` from
/// `format::countdown`, counted down every frame), then the last of them
/// gone. `wave` counts from one and `waves` is how many there are all
/// told, the one aboard counted.
/// Short on purpose: the line stands in a row that already holds the
/// clock, the alarm and whatever else, and a long one is a line the
/// crew's panels cut in half.
pub fn droids_standing(wave: u32, waves: u32, standing: u32) -> String {
    format!("MACHINES — wave {wave} of {waves}, {standing} up")
}
pub fn droids_next_wave(span: &str, wave: u32, waves: u32) -> String {
    format!("MACHINES — wave {wave} of {waves} in {span}")
}
pub const DROIDS_CLEARED: &str = "MACHINES — the last wave is down";
/// A defence before day ten (task 131), whose waves are the Manufacturers'
/// people with the day's share of Troopers: the same three lines under
/// their name.
pub fn manufacturers_standing(wave: u32, waves: u32, standing: u32) -> String {
    format!("MANUFACTURERS — wave {wave} of {waves}, {standing} up")
}
pub fn manufacturers_next_wave(span: &str, wave: u32, waves: u32) -> String {
    format!("MANUFACTURERS — wave {wave} of {waves} in {span}")
}
pub const MANUFACTURERS_CLEARED: &str = "MANUFACTURERS — the last wave is down";
/// A defence before its first wave (task 111): the prep time, counting.
pub fn defense_prepare(span: &str) -> String {
    format!("Prepare: {span}")
}
/// An Area defend's line (October 2026): the wave on the ground or the
/// next, and the hold's time left; once it is out, the last wave.
pub fn area_standing(wave: u32, standing: u32, next: Option<&str>, hold: &str) -> String {
    match next {
        Some(next) => {
            format!("AREA DEFEND — wave {wave}, {standing} up, the next in {next} · hold {hold}")
        }
        None => format!("AREA DEFEND — wave {wave}, {standing} up · hold {hold}"),
    }
}
pub fn area_next_wave(span: &str, wave: u32, hold: &str) -> String {
    format!("AREA DEFEND — wave {wave} in {span} · hold {hold}")
}
pub fn area_last_wave(standing: u32) -> String {
    format!("AREA DEFEND — time is up: the last wave, {standing} up")
}
/// The top count's words at an Area defend: which wave (there is no last
/// but the one standing when the time runs out), the hold's time left,
/// and the FOB's bar.
pub fn area_wave(wave: u32) -> String {
    format!("Wave {wave}")
}
pub fn area_hold(span: &str) -> String {
    format!("Hold {span}")
}
pub fn area_next(span: &str) -> String {
    format!("next {span}")
}
pub const AREA_LAST_WAVE: &str = "Last wave";
/// Seal the breaches' line (October 2026): the breaches still open, the
/// machines up and the next wave; once all are welded, what is left.
pub fn breaches_line(open: u32, total: u32, standing: u32, next: Option<&str>) -> String {
    match next {
        Some(next) => format!(
            "SEAL THE BREACHES — {open} of {total} open · {standing} up · next wave in {next}"
        ),
        None => format!("SEAL THE BREACHES — {open} of {total} open · {standing} up"),
    }
}
pub fn breaches_prepare(open: u32, span: &str) -> String {
    format!("SEAL THE BREACHES — {open} open · first wave in {span}")
}
pub fn breaches_sealed(standing: u32) -> String {
    format!("SEAL THE BREACHES — every breach welded: {standing} left aboard")
}
pub const BREACHES_TIP: &str = "• Weld every breach shut: stand at it, V
• 15 s of hands on it, faster with two
• A wave every 30 s while one is open, smaller as they close
• The machines go for the breach being welded
• All welded and the deck clear: held";
/// The top count's line at Seal the breaches.
pub fn breaches_count(open: u32, total: u32) -> String {
    format!("Breaches open {open}/{total} · weld: V")
}
pub const BREACHES_SEALED_COUNT: &str = "Every breach welded";
/// An Evacuation's line, its count and its tip.
pub fn evacuation_line(aboard: u32, alive: u32, carried: bool) -> String {
    if carried {
        format!("EVACUATION — {aboard} of {alive} aboard · they follow the flag")
    } else {
        format!("EVACUATION — {aboard} of {alive} aboard · take up the flag: V")
    }
}
pub fn evacuation_count(aboard: u32, alive: u32) -> String {
    format!("Aboard {aboard}/{alive} · flag: V")
}
pub const EVACUATION_TIP: &str = "• Take up the flag: V beside it
• The people follow its carrier, who still shoots
• V again: put it down, they hold by it
• Everyone alive aboard the ship: held
• Reward: the share of them saved
• A wave every 25 s until then";
/// What the Use key would do where the player's own Bim stands
/// (October 2026), said under it: the key, then the deed.
pub fn use_prompt(key: &str, what: &str) -> String {
    format!("{key} — {what}")
}
pub const USE_TAKE_FLAG: &str = "take the flag";
pub const USE_DROP_FLAG: &str = "put the flag down";
pub const USE_PLANT: &str = "plant the charge";
pub const USE_WELD: &str = "weld shut";
pub const USE_HACK: &str = "hack the terminal";
pub const USE_CUT: &str = "cut the cell open";
pub const USE_TAKE_DRUM: &str = "take up the drum";
pub const USE_TAKE_CRATE: &str = "take up the crate";
pub const USE_PUT_DOWN: &str = "put it down";
pub const USE_DEFUSE: &str = "defuse the charge";

/// The log's line for the second set of attacks moving on (October 2026,
/// `WorldEvent::Objective`, its `what` codes).
pub fn objective_event(what: u32, n: u32, w: u32) -> String {
    let by = || {
        if w == u32::MAX {
            String::new()
        } else {
            crew_name(w)
        }
    };
    match what {
        1 if n == 0 => "Every terminal taken: the data is ours. Clear the deck.".into(),
        1 => format!("A terminal taken: {n} left. The alarm is louder."),
        3 => format!("{} took up a crate: a wave is coming for it.", by()),
        4 => format!("A crate is aboard: {n} home."),
        5 => format!("{} took up a drum: no shooting with it.", by()),
        6 => format!("The drum in {}'s arms went up!", by()),
        7 => format!("A drum in the reactor: {n} in."),
        8 => "The reactor is critical. Clear the deck.".into(),
        9 => format!("The cell is cut open: {n} of the Republic's are out and follow the crew."),
        10 => "The Overseer is hurt and running for his airlock!".into(),
        11 => "The Overseer got away, and the bounty with him.".into(),
        12 => "The Overseer is down.".into(),
        13 => "A fresh drum stands in the depot.".into(),
        20 if n == 0 => "Every charge defused. Clear the deck.".into(),
        20 => format!("A charge defused: {n} left."),
        21 => "The timer ran out: the station went up.".into(),
        22 if n == 2 => "The machines broke the inner door: protect the commander!".into(),
        22 => "The machines broke an outer door: the inner one is next.".into(),
        23 => "The commander is down.".into(),
        24 => "The hold is done: the Republic's soldiers are on their way.".into(),
        25 => format!("{n} of the Republic's soldiers came aboard."),
        26 => "The commander is dead: the site falls.".into(),
        27 => "The commander is safe. Clear the deck.".into(),
        _ => String::new(),
    }
}

/// The threat line of the second set of attacks (October 2026).
pub fn objective_line(look: &world::ObjectiveLook) -> String {
    use world::run::Mission;
    let wave = look
        .wave_in
        .map(|s| format!(" · next wave {}", crate::format::countdown(s as f64)))
        .unwrap_or_default();
    let time = || crate::format::countdown(look.time_left.unwrap_or(0.0) as f64);
    match look.mission {
        Mission::Bombs if look.done < look.total => format!(
            "BOMB DISPOSAL — {} of {} charges defused · V at one, 8 s · {} left",
            look.done,
            look.total,
            time()
        ),
        Mission::Bombs => "BOMB DISPOSAL — every charge defused: clear the deck".into(),
        Mission::Doors => match look.phase {
            0 => format!("HOLD THE DOORS — the outer doors stand · hold {}", time()),
            1 => format!(
                "HOLD THE DOORS — an outer door is broken: the inner one · hold {}",
                time()
            ),
            2 => format!(
                "HOLD THE DOORS — the vault is open: keep the commander up · hold {}",
                time()
            ),
            _ => "HOLD THE DOORS — held: the Republic's soldiers are coming, clear the deck".into(),
        },
        Mission::Chief if look.phase == 0 => {
            format!(
                "PROTECT THE COMMANDER — every machine hunts him · {} left",
                time()
            )
        }
        Mission::Chief => "PROTECT THE COMMANDER — he is safe: clear the deck".into(),
        Mission::Overseer => match look.phase {
            0 => format!("KILL THE OVERSEER — in his office, slow on his feet{wave}"),
            1 => format!("THE OVERSEER IS FLEEING — stop him before his airlock{wave}"),
            2 => "The Overseer got away with the bounty: clear the deck".into(),
            _ => "The Overseer is down: clear the deck".into(),
        },
        Mission::Heist if look.done < look.total => format!(
            "DATA HEIST — {} of {} terminals · V at one, 10 s{wave}",
            look.done, look.total
        ),
        Mission::Heist => "DATA HEIST — every terminal taken: clear the deck".into(),
        Mission::Prison if look.phase == 0 => {
            "PRISON BREAK — cut the cell open: V at its door, 8 s".into()
        }
        Mission::Prison => format!("PRISON BREAK — {} out: keep them alive", look.done),
        Mission::FuelRun if look.phase == 0 => format!(
            "FUEL RUN — {} of {} drums in the reactor · V to carry, a hit and it blows{wave}",
            look.done, look.total
        ),
        Mission::FuelRun => "FUEL RUN — the reactor is critical: clear the deck".into(),
        Mission::Salvage => format!(
            "SALVAGE — {} of {} crates aboard · each taken brings a wave",
            look.done, look.total
        ),
        _ => String::new(),
    }
}

/// The count under the top's (October 2026): what is done of how much.
pub fn objective_count(look: &world::ObjectiveLook) -> String {
    use world::run::Mission;
    match look.mission {
        Mission::Overseer => match look.phase {
            0 => "Overseer in his office".into(),
            1 => "Overseer fleeing".into(),
            2 => "Overseer escaped".into(),
            _ => "Overseer down".into(),
        },
        Mission::Heist => format!("Terminals {}/{}", look.done, look.total),
        Mission::Prison if look.phase == 0 => "Cell sealed · V".into(),
        Mission::Prison => format!("Prisoners out {}/{}", look.done, look.total),
        Mission::FuelRun if look.phase == 0 => format!("Drums {}/{}", look.done, look.total),
        Mission::FuelRun => "Reactor critical".into(),
        Mission::Salvage => format!("Salvage aboard {}/{}", look.done, look.total),
        Mission::Bombs | Mission::Doors | Mission::Chief => match look.time_left {
            Some(s) => {
                let what = match look.mission {
                    Mission::Bombs => format!("Charges {}/{} ·", look.done, look.total),
                    Mission::Doors => format!("Doors broken {}/3 ·", look.done),
                    _ => "Commander ·".into(),
                };
                format!("{what} {}", crate::format::countdown(s as f64))
            }
            None => "Clear the deck".into(),
        },
        _ => String::new(),
    }
}

/// The tip of the second set's threat line (October 2026).
pub fn objective_tip(mission: world::run::Mission) -> &'static str {
    use world::run::Mission;
    match mission {
        Mission::Overseer => {
            "• Kill him: waves keep coming while he lives
• He walks slowly; hurt, he runs for a far airlock
• Out of it, the bounty goes with him"
        }
        Mission::Heist => {
            "• Hold V at a terminal, 10 s (engineer: 5 s)
• Each taken: waves sooner and bigger
• All taken and the deck clear: done"
        }
        Mission::Prison => {
            "• Hold V at the cell door, 8 s (engineer: 4 s)
• Freed prisoners follow as armed bots
• Alive and aboard at the end: they join the crew"
        }
        Mission::FuelRun => {
            "• V takes up a drum; the carrier can't shoot
• A hit on the carrier: the drum blows
• Three in the reactor: it goes critical
• A lost drum comes back in the depot"
        }
        Mission::Salvage => {
            "• V takes up a crate; the carrier can't shoot
• Each taken brings a bigger wave at once
• Carried aboard the ship: paid at once
• Leave whenever you like"
        }
        Mission::Bombs => {
            "• Hold V at a charge, 8 s (engineer: 4 s)
• All charges share one 3:00 timer
• Timer out: the station explodes, the run is lost"
        }
        Mission::Doors => {
            "• The machines break a door in 20 s at it, unbroken
• Two outer doors, then the inner one, then the commander
• The commander down: the run is lost
• Hold 3:00, then the Republic's soldiers come"
        }
        Mission::Chief => {
            "• Every machine hunts the commander
• Keep him alive 2:00; downed, revive him
• Dead: the site falls"
        }
        _ => "",
    }
}
/// And for the flag refused (October 2026).
pub fn flag_refused(why: world::Refusal) -> String {
    format!("No flag: {}.", refusal(why))
}
/// A nest hunt's line, its count and its tip.
pub fn nests_line(standing: u32, total: u32) -> String {
    if standing == 0 {
        "NEST HUNT — every nest destroyed: clear the deck".to_string()
    } else {
        format!(
            "NEST HUNT — {standing} of {total} nests standing · each builds a machine every 20 s"
        )
    }
}
pub fn nests_count(standing: u32, total: u32) -> String {
    format!("Nests {standing}/{total}")
}
pub const NESTS_TIP: &str = "• Nests grow in the walls: destroy them
• Each builds a machine every 20 s
• A fuel drum stands by one of them
• All nests and machines down: cleared";
/// A Sabotage's line (October 2026), by its phase: the charge to plant,
/// held with the machines' disarming, the escape's clock.
pub const SABOTAGE_PLANT: &str = "SABOTAGE — reach the charge (amber mark) and plant it: V";
pub fn sabotage_hold(left: &str, disarmed: u32) -> String {
    format!("SABOTAGE — hold the charge {left} · disarmed {disarmed}%")
}
pub fn sabotage_escape(left: &str) -> String {
    format!("SABOTAGE — get to the way out (green) · it blows in {left}")
}
pub const SABOTAGE_BLOWN: &str = "SABOTAGE — the charge blew";
pub const SABOTAGE_TIP: &str = "• Plant the charge at the amber mark: V, 12 s, faster with two
• Then hold it 45 s: machines at it disarm it
• Disarmed = run lost
• Then the ship waits at the green way out: 75 s
• Not there when it blows = left behind";
/// The top count's line at a Sabotage.
pub const SABOTAGE_COUNT_PLANT: &str = "Plant the charge · V";
pub fn sabotage_count_hold(left: &str, disarmed: u32) -> String {
    format!("Hold {left} · disarmed {disarmed}%")
}
pub fn sabotage_count_escape(left: &str) -> String {
    format!("Get out {left}")
}
/// The run lost to a charge disarmed.
pub const OVER_TITLE_SABOTAGE: &str = "The charge was disarmed";
pub const OVER_TITLE_BOMBS: &str = "The station went up";
pub const OVER_LINE_BOMBS: &str = "A charge was left when the timer ran out.";
pub const OVER_TITLE_DOORS: &str = "The commander fell";
pub const OVER_LINE_DOORS: &str = "The machines broke into the vault and brought him down.";
pub const OVER_LINE_SABOTAGE: &str = "The machines got to the charge before it could be armed.";
/// And for a planting refused (October 2026).
pub fn plant_refused(why: world::Refusal) -> String {
    format!("Cannot plant: {}.", refusal(why))
}
pub const AREA_FOB: &str = "FOB";
pub const AREA_CONTESTED: &str = "contested";
pub const AREA_DEFENSE_TIP: &str = "• Hold the FOB (sandbag ring) 3 min\n• First wave in 5 s, then every 31 s, 1 s sooner each\n• Time up: clear the rest\n• Machines alone in it 20 s = run lost\n• Anybody of yours inside stops the count\n• Green arrow: the FOB off screen";

/// The Machine Heart's line (feature 108), ahead of the wave's in the same
/// red chip while the crew are in its fortress: the core's health and how
/// many conduits still seal it, and the word for its phase.
pub fn heart_line(
    phase: world::heart::HeartPhase,
    core: f32,
    max: f32,
    left: u32,
    of: u32,
) -> String {
    use world::heart::HeartPhase;
    let core = crate::format::grouped(core.max(0.0).ceil() as u64);
    let max = crate::format::grouped(max.max(0.0).ceil() as u64);
    match phase {
        HeartPhase::Sealed => {
            format!("HEART — core {core}/{max} sealed, {left} of {of} conduits left")
        }
        HeartPhase::Exposed => format!("HEART — core {core}/{max} exposed, 0 of {of} conduits"),
        HeartPhase::Overload => format!("HEART — core {core}/{max} OVERLOADING"),
        HeartPhase::Destroyed => "HEART — the core is destroyed".into(),
    }
}
/// The waves' half of that chip, and the two put together.
pub fn heart_wave(wave: u32, waves: u32, standing: u32) -> String {
    format!("wave {wave} of {waves}, {standing} up")
}
pub fn heart_next_wave(span: &str, wave: u32, waves: u32) -> String {
    format!("wave {wave} of {waves} in {span}")
}
pub fn heart_and_waves(heart: &str, waves: &str) -> String {
    format!("{heart} · {waves}")
}
pub const HEART_TIP: &str = "• Core safe while conduits stand (red lines)\n• Last conduit down: beam at the nearest, a machine built every 30 s\n• Core under 1/3: two beams, ×2 building\n• Core destroyed: run won\n• Leave: it resets";
/// The top bar's count of how many enemies a wave is here and now
/// (`World::droid_wave_size`), shown always so `scaling.ron` can be tuned
/// by eye.
pub fn wave_size_chip(n: u32) -> String {
    format!("Wave size {n}")
}
/// The word beside the big red count of enemies standing, at the top of
/// the screen: which wave of how many.
pub fn wave_counter(wave: u32, waves: u32) -> String {
    format!("Wave {wave}/{waves}")
}
pub const WAVE_SIZE_TIP: &str = "• Enemies a wave, here and now\n• Grows with players and days\n• A landed wave keeps its size\n• scaling.ron";
pub const DROIDS_TIP: &str = "• Waves; the next once the last is down\n• In at the farthest airlock, or a town gate\n• Leave early: site resets, bounty lost, XP kept";

/// The same warning over a town the crew are defending (feature 94):
/// the fight is the machines', but the town's people are in it too.
pub const DEFENSE_TIP: &str = "• First wave in 5 s, then one at a time\n• Defenders and guards fight beside you\n• Hold every wave: cleared, no pay\n• A town held stays friendly; some join you\n• Leave early: it falls";
/// [`DEFENSE_TIP`] in the scaling's area 0 (task 131; October 2026),
/// when the attackers are the Manufacturers' people alone.
pub const DEFENSE_TIP_MANUFACTURERS: &str = "• Manufacturers only, for now\n• Machines join them from tier 1\n• First wave in 5 s, then one at a time\n• Defenders and guards fight beside you\n• Hold every wave: cleared, no pay\n• A town held stays friendly; some join you\n• Leave early: it falls";

/// The header's word while the crew's alarm is up, and what it means.
pub const ALARM_STATUS: &str = "To arms — an enemy is near";
pub const ALARM_TIP: &str = "• Enemy within 30 tiles, seen, or crew hit, in the last 30 s\n• Every crew member but yours fights\n• Ends after 30 s quiet";

#[cfg(test)]
mod tests {
    use super::*;

    /// A player's name for their Bim is what every line calls it; a slot
    /// left blank, or past the list, keeps the table's; and the names are
    /// tidied on the way in like everything off the wire.
    #[test]
    fn the_crew_are_called_what_their_players_called_them() {
        set_crew_names(&["".to_string(), " Ada \n".to_string()]);
        assert_eq!(crew_name(0), "James");
        assert_eq!(crew_name(1), "Ada");
        assert_eq!(crew_name(2), "Priya");
        set_crew_names(&[]);
        assert_eq!(crew_name(1), "Kate");
    }

    /// The revive's words spell the numbers out — thirty seconds down,
    /// ten to revive, a medic's four, three tenths of the bar back
    /// (task 120; no slow after since October 2026) — so a change to the
    /// rules' numbers has to come here as well.
    #[test]
    fn the_revive_s_words_say_the_rules_numbers() {
        use bims::health as h;
        assert_eq!(
            (
                h::DOWNED_SECONDS,
                h::REVIVE_SECONDS,
                world::class::MEDIC_REVIVE_SECONDS
            ),
            (30.0, 10.0, 4.0)
        );
        assert_eq!(h::REVIVED_TO, 0.3);
        assert_eq!(h::BLEEDS_UNDER, 20.0);
        for words in [HEALTH_TIP, DOWNED_TIP] {
            assert!(
                words.contains("30%") && !words.contains("slower"),
                "{words}"
            );
        }
        assert!(HEALTH_TIP.contains("30 s") && HEALTH_TIP.contains("10 s (medic 4 s)"));
        assert!(HEALTH_TIP.contains("20 HP") && CRITICAL_TIP.contains("20 HP"));
        assert_eq!(downed_short(29.2), "DOWNED — dies in 30 s");
    }

    #[test]
    fn every_ascension_has_a_name() {
        assert_eq!(ASCENSION_NAMES.len() as u32, world::ascension::MOST + 1);
        for level in 0..=world::ascension::MOST {
            assert!(!ascension_line(level).is_empty());
        }
    }

    #[test]
    fn every_table_of_the_rules_is_as_long_as_its_enum() {
        // --- every_part_has_a_name_and_every_name_a_part ---
        {
            assert_eq!(PART_NAMES.len(), PartKind::ALL.len());
            for &kind in PartKind::ALL.iter() {
                assert!(!part_name(kind).is_empty());
            }
        }

        // --- every_resource_has_a_name ---
        {
            assert_eq!(RESOURCE_NAMES.len(), ResourceId::CODES);
        }

        // --- every_relic_has_a_name_and_a_line (feature 106) ---
        {
            assert_eq!(RELIC_NAMES.len(), world::Relic::ALL.len());
            for relic in world::Relic::ALL {
                // A boon and a price on every one, each its own line.
                let lines = relic_lines(relic);
                assert!(lines.iter().any(|&(_, good)| good), "{relic:?}");
                assert!(lines.iter().any(|&(_, good)| !good), "{relic:?}");
            }
            let words = |r| -> Vec<String> { relic_lines(r).into_iter().map(|(l, _)| l).collect() };
            assert_eq!(
                words(world::Relic::GlassCannon),
                [
                    "+30% weapon damage for the crew",
                    "+25% damage taken for the crew"
                ]
            );
            assert_eq!(
                words(world::Relic::BountyContract),
                ["+50% money for every enemy down", "-20% damage to enemies"]
            );
            assert_eq!(
                words(world::Relic::BlackMarket),
                ["-40% trader prices", "+35% HP for every enemy"]
            );
            assert_eq!(
                words(world::Relic::PointBlank),
                [
                    "+30% weapon damage within 4 tiles for the crew",
                    "-20% weapon damage past 7 tiles for the crew"
                ]
            );
            assert_eq!(
                words(world::Relic::Overtime),
                [
                    "+100% experience and money from the bonus wave",
                    "Every fight that can have a bonus wave has it"
                ]
            );
            assert_eq!(
                words(world::Relic::DrillSergeant)[2..],
                ["-20% weapon damage for the players' Bims"]
            );
        }

        // --- every_item_and_spot_has_a_line ---
        {
            assert_eq!(ITEM_TIPS.len(), ResourceId::CODES);
            assert_eq!(
                SPOT_NAMES.len(),
                bims::room::SPOT_RESEARCH as usize + 1,
                "the research desk is the last spot"
            );
        }

        // --- the_word_tables_are_as_long_as_the_generator_expects ---
        {
            assert_eq!(STAR_WORDS.len(), worldgen::name::STAR_WORDS as usize);
            assert_eq!(STATION_WORDS.len(), worldgen::name::STATION_WORDS as usize);
        }
    }

    /// A level past the sixteenth gives weapon damage, not a skill point
    /// (October 2026), and the log says so.
    #[test]
    fn a_level_past_sixteen_says_its_weapon_damage() {
        let line = |level| {
            event_line(WorldEvent::LevelUp {
                who: 0,
                class: 2,
                level,
            })
            .unwrap()
        };
        assert!(line(16).contains("a skill point to spend"), "{}", line(16));
        assert!(line(17).ends_with("weapon damage +5%."), "{}", line(17));
        assert!(line(20).ends_with("weapon damage +20%."), "{}", line(20));
    }

    #[test]
    fn every_table_of_the_room_is_as_long_as_its_enum() {
        // --- the_classes_and_their_four_abilities_are_named ---
        {
            assert_eq!(CLASS_NAMES.len(), world::Class::ALL.len());
            assert_eq!(SITE_KIND_NAMES.len(), world::SiteKind::ALL.len());
            for (i, kind) in world::SiteKind::ALL.into_iter().enumerate() {
                assert_eq!(kind.code() as usize, i, "{kind:?}");
            }
            assert_eq!(CLASS_TIPS.len(), world::Class::ALL.len());
            for kind in world::DeployKind::ALL {
                assert_ne!(deployable_name(kind.code()), "Deployable", "{kind:?}");
            }
            for (i, class) in world::Class::ALL.iter().enumerate() {
                assert_eq!(class.code() as usize, i);
            }
            for class in [
                world::Class::Engineer,
                world::Class::Soldier,
                world::Class::Medic,
                world::Class::Tank,
                world::Class::Commander,
            ] {
                // Every class is a ranked kit (task 139): its words are the
                // four abilities'.
                assert!(world::class::ranked(class));
                for slot in 0..world::class::SLOTS as u8 {
                    assert!(!ranked_ability(class, slot).is_empty());
                    assert!(!ranked_what(class, slot).is_empty());
                    // Dota 2's way: a line of words, and numbers
                    // for every rank under it.
                    let stats = ranked_stats(class, slot);
                    assert!(!stats.is_empty(), "{class:?} {slot} has numbers");
                    assert!(
                        stats.iter().any(|s| s.values.len() > 1),
                        "{class:?} {slot}: a rank changes something"
                    );
                    for stat in &stats {
                        assert!(
                            [1, world::class::MAX_RANK as usize].contains(&stat.values.len()),
                            "{class:?} {slot} {}",
                            stat.label
                        );
                    }
                    assert!(
                        ranked_what(class, slot).len() <= 160,
                        "{class:?} {slot}: a line, not a paragraph"
                    );
                    assert!(ranked_tip(class, slot, 0).contains("Not learnt"));
                    assert!(ranked_tip(class, slot, 1).contains("rank 2 at Lv"));
                    assert!(ranked_tip(class, slot, 4).contains("max"));
                    assert!(!ranked_tip(class, slot, 2).contains("roll"));
                }
            }
            // And the classless one has none.
            for slot in 0..world::class::SLOTS as u8 {
                assert!(ranked_ability(world::Class::None, slot).is_empty());
            }
            // The figures themselves, said the tree's way: no trailing
            // nought, two decimals at most.
            assert_eq!(fig(6.0), "6");
            assert_eq!(fig(1.15), "1.15");
            assert_eq!(fig(40.0 / 1.5), "26.67");
            assert_eq!(by(1.5), "×1.5");
            assert_eq!(pc(0.75), "75%");
            // The new refusals and events all say something.
            for why in [
                Refusal::ClassLocked,
                Refusal::NotAnEngineer,
                Refusal::NoKit,
                Refusal::NoSentryYet,
                Refusal::CantDeployThere,
                Refusal::NoSuchDeployable,
                Refusal::NoClass,
                Refusal::NotASoldier,
                Refusal::NoGrenade,
                Refusal::NoGrenadesYet,
                Refusal::CoolingDown,
                Refusal::OutOfThrowRange,
                Refusal::NoLineToTile,
                Refusal::CantThrowThere,
                Refusal::NotAMedic,
                Refusal::NoPatient,
                Refusal::NotACrewmate,
                Refusal::OutOfBeamRange,
                Refusal::NoSightOfPatient,
                Refusal::NotATank,
                Refusal::NotACommander,
            ] {
                assert!(!refusal(why).is_empty());
                assert!(deploy_refused(why).contains(refusal(why)));
                assert!(throw_refused(why).contains(refusal(why)));
                assert!(stun_shot_refused(why).contains(refusal(why)));
                assert!(beam_refused(why).contains(refusal(why)));
                assert!(heal_drone_refused(why).contains(refusal(why)));
                assert!(healing_circle_refused(why).contains(refusal(why)));
                assert!(riot_shield_refused(why).contains(refusal(why)));
                assert!(reflect_refused(why).contains(refusal(why)));
                assert!(bastion_refused(why).contains(refusal(why)));
                assert!(rally_refused(why).contains(refusal(why)));
            }
            assert_eq!(rally_line(0.0, 0.0, false), RALLY_NOT_LEARNT);
            assert_eq!(rally_line(0.0, 0.0, true), "Rally ready");
            assert_eq!(rally_line(0.0, 7.2, true), "Rally ready in 7 s");
            assert_eq!(rally_line(4.0, 12.0, true), "Rallying — 4 s left");
            assert_eq!(battle_cry_line(2.0, 9.0, true), "Battle Cry — 2 s left");
            assert_eq!(battle_cry_line(0.0, 0.0, true), "Battle Cry ready");
            assert_eq!(reflect_line(0.0, 0.0, false), REFLECT_NOT_LEARNT);
            assert_eq!(reflect_line(0.0, 0.0, true), "Reflect Barrier ready");
            assert_eq!(reflect_line(0.0, 7.2, true), "Reflect Barrier ready in 7 s");
            assert_eq!(reflect_line(4.0, 12.0, true), "Reflecting — 4 s left");
            assert_eq!(bastion_line(0.0, false), BASTION_NOT_LEARNT);
            assert_eq!(bastion_line(90.0, true), "Bastion ready in 90 s");
            assert_eq!(bastion_line(0.0, true), "Bastion ready");
            assert_eq!(
                riot_shield_line(false, 0.0, 0.0, 0.0),
                RIOT_SHIELD_NOT_LEARNT
            );
            assert_eq!(riot_shield_line(true, 18.4, 20.0, 0.0), "Shield up — 18/20");
            assert_eq!(
                riot_shield_line(false, 3.0, 20.0, 6.2),
                "Shield broken — 3/20, ready in 7 s"
            );
            assert_eq!(grenades_line(1, 0.0), "1 grenade");
            assert_eq!(grenades_line(2, 3.4), "2 grenades — next in 3 s");
            assert_eq!(beam_line(&[]), BEAM_OFF);
            assert_eq!(beam_line(&["Kate".to_string()]), "Beaming Kate");
            assert_eq!(
                beam_line(&["Kate".to_string(), "Ali".to_string()]),
                "Beaming Kate and Ali"
            );
            assert_eq!(
                heal_drone_line(0.0, 0.0, false),
                "Heal Drone not learnt yet"
            );
            assert_eq!(heal_drone_line(0.0, 3.4, true), "Heal Drone ready in 3 s");
            assert_eq!(heal_drone_line(0.0, 0.0, true), "Heal Drone ready");
            assert_eq!(heal_drone_line(4.2, 30.0, true), "Heal Drone up — 4 s left");
            assert_eq!(
                healing_circle_line(false, false),
                "Healing Circle not learnt yet"
            );
            assert_eq!(healing_circle_line(false, true), "Healing Circle off");
            assert!(healing_circle_line(true, true).starts_with("Healing Circle on"));
            for event in [
                WorldEvent::LevelUp {
                    who: 0,
                    class: 1,
                    level: 2,
                },
                WorldEvent::LevelUp {
                    who: 0,
                    class: 2,
                    level: 3,
                },
                WorldEvent::LevelUp {
                    who: 0,
                    class: 2,
                    level: 8,
                },
                WorldEvent::LevelUp {
                    who: 0,
                    class: 2,
                    level: 18,
                },
                WorldEvent::Bastion { who: 0, reached: 3 },
                WorldEvent::Deployed { who: 0, kind: 3 },
                WorldEvent::PackedUp { who: 0, kind: 1 },
                WorldEvent::DeployableLost { kind: 1 },
                WorldEvent::ShotCharging { who: 0 },
                WorldEvent::StunShotFired { who: 0 },
                WorldEvent::Thrown { who: 0 },
                WorldEvent::MineTriggered { who: 0 },
                WorldEvent::SatchelThrown { who: 0 },
                WorldEvent::SatchelsBlown { who: 0, count: 2 },
                WorldEvent::DeployableLost { kind: 2 },
                WorldEvent::Beamed {
                    who: 0,
                    patient: Some(1),
                },
                WorldEvent::Beamed {
                    who: 0,
                    patient: None,
                },
                WorldEvent::DroneLaunched { who: 0 },
                WorldEvent::Circled { who: 0, on: true },
                WorldEvent::Circled { who: 0, on: false },
                WorldEvent::ShieldRaised { who: 0, on: true },
                WorldEvent::ShieldRaised { who: 0, on: false },
                WorldEvent::ShieldBroken { who: 0 },
                WorldEvent::Reflecting { who: 0 },
                WorldEvent::Rallied { who: 0 },
                WorldEvent::BattleCried { who: 0 },
                WorldEvent::Reinforced { who: 0, count: 3 },
            ] {
                assert!(
                    event_line(event).is_some_and(|l| !l.is_empty()),
                    "{event:?}"
                );
            }
        }

        // --- the_chooser_names_every_hair_and_shade ---
        {
            assert_eq!(HAIR_NAMES.len(), bims::character::Hair::ALL.len());
            assert_eq!(SHADE_NAMES.len(), bims::character::Shade::ALL.len());
            for (i, hair) in bims::character::Hair::ALL.iter().enumerate() {
                assert_eq!(hair.code() as usize, i);
            }
            for (i, shade) in bims::character::Shade::ALL.iter().enumerate() {
                assert_eq!(shade.code() as usize, i);
            }
            // And every colour a player's own Bim can wear (feature 84).
            assert_eq!(TINT_NAMES.len(), bims::character::Tint::ALL.len());
            for (i, tint) in bims::character::Tint::ALL.iter().enumerate() {
                assert_eq!(tint.code() as usize, i);
                assert_eq!(bims::character::Tint::from_code(i as u8), *tint);
            }
        }

        // --- every_event_has_a_line ---
        {
            // A code with no sentence is a row that never appears; the newest
            // event is the one most likely to have been forgotten.
            use bims::combat::ArmourKind;
            assert!(event_line(WorldEvent::EnemyDown { station: 3, who: 1 }).is_some());
            assert!(event_line(WorldEvent::CrewHit { who: 0 }).is_some());
            assert!(event_line(WorldEvent::CrewDown { who: 0 }).is_some());
            assert!(
                event_line(WorldEvent::PieceBroke {
                    who: 0,
                    kind: ArmourKind::Armour
                })
                .is_some()
            );
            assert!(event_line(WorldEvent::Locked { who: 0 }).is_some());
            // The loadouts' and the armory's (task 113).
            for event in [
                WorldEvent::GearChanged { who: 0, part: 2 },
                WorldEvent::GearOffered {
                    from: 0,
                    part: 0,
                    to: 1,
                },
                WorldEvent::OfferTaken {
                    from: 0,
                    part: 0,
                    to: 1,
                },
                WorldEvent::OfferWithdrawn {
                    from: 0,
                    part: 0,
                    to: 1,
                },
                WorldEvent::Respawned {
                    who: 1,
                    paid: 5_000,
                },
                WorldEvent::ShelfBought {
                    slot: 0,
                    index: 1,
                    to: u32::MAX,
                },
                WorldEvent::Sold {
                    slot: 0,
                    who: 0,
                    value: 1_000,
                },
                WorldEvent::ItemBought {
                    slot: 0,
                    kind: 1,
                    tier: 2,
                    to: 0,
                    upgrade: true,
                },
                // The relic vote (October 2026).
                WorldEvent::RelicsOffered { count: 3 },
                WorldEvent::RelicProposed { slot: 0, relic: 3 },
                WorldEvent::RelicProposed {
                    slot: 1,
                    relic: u32::MAX,
                },
                WorldEvent::RelicGiven { relic: 3 },
                WorldEvent::RelicsDeclined,
            ] {
                assert!(event_line(event).is_some(), "{event:?}");
            }
            // And the refusals a gear command can come back with each say
            // something other than the fallback.
            for why in [
                Refusal::GearLocked,
                Refusal::NotYours,
                Refusal::NoSuchGear,
                Refusal::NoOffer,
                Refusal::TraderClosed,
                Refusal::ClosedOnArrival,
                Refusal::NotAtATrader,
                Refusal::SoldOut,
                Refusal::TopTier,
            ] {
                assert!(!refusal(why).is_empty());
            }
        }

        // --- a_body_down_and_brought_round_are_said ---
        {
            assert!(
                event_line(WorldEvent::CrewDowned { who: 0 })
                    .is_some_and(|line| line.contains("is down!"))
            );
            assert!(
                event_line(WorldEvent::CrewRevived { who: 0, by: 1 })
                    .is_some_and(|line| line.contains("brought") && line.contains("round"))
            );
            assert!(
                event_line(WorldEvent::ResidentRevived {
                    station: 3,
                    who: 1,
                    by: 0
                })
                .is_some_and(|line| line.contains(&resident_name(3, 1)))
            );
        }
    }

    #[test]
    fn every_table_of_the_fight_is_as_long_as_its_enum() {
        // --- every_weapon_has_a_name_and_the_empty_slot_is_first ---
        {
            // Slot 0 is empty; every kind's code indexes its name.
            assert_eq!(
                WEAPON_NAMES.len(),
                bims::combat::WeaponKind::EVERY.len() + 1
            );
            for &kind in bims::combat::WeaponKind::EVERY.iter() {
                assert!(!weapon_name(Some(kind)).is_empty());
                assert_ne!(weapon_name(Some(kind)), WEAPON_NAMES[0]);
            }
            // The same for the armour: slot 0 empty, then one name a kind.
            assert_eq!(ARMOUR_NAMES.len(), bims::combat::ArmourKind::ALL.len() + 1);
            assert_eq!(armour_name(None), ARMOUR_NAMES[0]);
            for &kind in bims::combat::ArmourKind::ALL.iter() {
                assert!(!armour_name(Some(kind)).is_empty());
                assert_ne!(armour_name(Some(kind)), ARMOUR_NAMES[0]);
            }
        }

        // --- a_weapon_and_a_piece_say_every_tier_s_numbers ---
        {
            // The tables under a weapon's and a piece's tooltip, off the
            // real numbers: a value a tier made, the own tier lit, the
            // same at every tier folded into one.
            use bims::combat::{ArmourKind, Tier, WeaponKind};
            let row = |rows: &[Stat], label: &str| {
                rows.iter()
                    .find(|s| s.label == label)
                    .map(Stat::line)
                    .unwrap_or_default()
            };
            let (rifle, lit) = weapon_stats(WeaponKind::AutoRifle.at(Tier::Two));
            assert_eq!(lit, 2);
            assert_eq!(row(&rifle, "Tier"), "Tier: 1 / 2 / 3");
            assert_eq!(row(&rifle, "Magazine"), "Magazine: 30");
            assert_eq!(row(&rifle, "Reload"), "Reload: 1.8 s");
            assert_eq!(row(&rifle, "Fire cooldown"), "Fire cooldown: 0.25 s");
            let pistol = weapon_stats(WeaponKind::LaserPistol.basic()).0;
            assert_eq!(row(&pistol, "Fire cooldown"), "Fire cooldown: 0.3 s");
            // A minigun starts at tier two: two columns, tier two first.
            let (minigun, lit) = weapon_stats(WeaponKind::Minigun.basic());
            assert_eq!((row(&minigun, "Tier"), lit), ("Tier: 2 / 3".into(), 1));
            let blade = weapon_stats(WeaponKind::Schword.basic()).0;
            assert!(row(&blade, "Accuracy").is_empty() && row(&blade, "Magazine").is_empty());
            let (armour, lit) = armour_stats(ArmourKind::Armour, Tier::Three);
            assert_eq!(lit, 3);
            assert_eq!(row(&armour, "Dodge"), "Dodge: 0% / 0% / 27.1%");
            assert!(!locked_tip().is_empty());
        }

        // --- every_machine_has_a_name_and_the_empty_slot_is_first ---
        {
            // Feature 83: a droid has no name of its own, so the log
            // says its kind. Slot 0 is no machine at all.
            assert_eq!(DROID_NAMES.len(), bims::droid::DroidKind::EVERY.len() + 1);
            for kind in bims::droid::DroidKind::EVERY {
                assert!(!droid_name(kind.code()).is_empty());
                assert_ne!(droid_name(kind.code()), DROID_NAMES[0]);
            }
            assert_eq!(droid_name(99), "Machine", "a kind with no row");
        }

        // --- every_thing_in_a_cell_has_a_tip ---
        {
            // A cell's tooltip is the resource's line; a blank one is an icon
            // nobody can ask about.
            assert_eq!(ITEM_TIPS.len(), ResourceId::CODES);
            for &id in ResourceId::ALL.iter() {
                assert!(!item_tip(id).is_empty(), "{id:?} has no tip");
            }
        }
    }

    /// The items (October 2026): a name a kind, a line at every tier it is
    /// made at, a name a loadout slot, and the Override Core's fifth rank
    /// said for every class.
    #[test]
    fn every_item_has_a_name_a_line_and_every_slot_a_name() {
        use bims::module::ModuleKind;
        assert_eq!(ITEM_NAMES.len(), ModuleKind::ALL.len());
        for kind in ModuleKind::ALL {
            assert_eq!(item_name(kind), ITEM_NAMES[kind.code() as usize]);
            for tier in bims::combat::Tier::ALL
                .into_iter()
                .filter(|&t| kind.made_at(t))
            {
                let item = kind.at(tier);
                assert!(!item_line(item).is_empty());
                assert!(module_tip(item).starts_with(item_name(kind)));
            }
            // A kind made at every tier has a row a tier, each its own
            // numbers; one made at a tier alone has no table.
            let rows: Vec<_> = (1..=3).filter_map(|t| item_tier_line(kind, t)).collect();
            if kind.min_tier() == bims::combat::Tier::One && kind.tiered() {
                assert_eq!(rows.len(), 3, "{kind:?}");
                assert!(rows[0] != rows[1] && rows[1] != rows[2], "{kind:?}");
            } else {
                assert!(rows.is_empty(), "{kind:?}");
            }
        }
        assert_eq!(
            item_tier_line(ModuleKind::BlinkDrive, 2).as_deref(),
            Some("8 tiles · 12 s cooldown")
        );
        assert_eq!(
            item_title(ModuleKind::Executioner.at(bims::combat::Tier::Two)),
            "an Executioner, tier 2"
        );
        assert_eq!(
            item_title(ModuleKind::OverrideCore.at(bims::combat::Tier::One)),
            "an Override Core"
        );
        assert_eq!(SLOT_NAMES.len(), world::GearSlot::ALL.len());
        for class in world::Class::ALL {
            assert_eq!(override_rank(class).is_some(), class != world::Class::None);
        }
        for why in [
            Refusal::NoSuchItem,
            Refusal::BlinkLocked,
            Refusal::NowhereToBlink,
            Refusal::ItemsFull,
            Refusal::BotsCarryNoItems,
            Refusal::NotForSale,
            Refusal::NotSellable,
            Refusal::OutOfItemRange,
            Refusal::NoWayInNear,
            Refusal::NoChargeNear,
            Refusal::ShipCastOff,
            Refusal::NoFlagNear,
            Refusal::NothingToUse,
        ] {
            assert!(!refusal(why).is_empty());
        }
    }
}
