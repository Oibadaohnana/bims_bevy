//! Every word on the screen.
//!
//! The simulation knows crew member 0, part kind 7 and event code 14, and
//! nothing else about them: no strings come out of the rules crates, and
//! that is deliberate — a native server will one day run the same crates
//! and it has no words to say. These tables are where the words are, indexed
//! by the codes each crate writes out and never renumbers. Adding a part, an
//! event or a job is a variant there and a name here.

use bims::combat::WeaponStats;
use physics::ResourceId;
use shipdesign::parts::PartKind;
use world::{Refusal, WorldEvent};

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
pub const PART_NAMES: [&str; 48] = [
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
    "Fusion reactor",
    "Power conduit",
    "Battery",
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
    "Large fusion reactor",
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
];

pub fn part_name(kind: PartKind) -> &'static str {
    PART_NAMES
        .get(kind as usize)
        .copied()
        .unwrap_or("Something")
}

/// The palette, grouped the way a ship is thought about rather than the way
/// the enum is numbered. The rows are built off `PartKind::ALL` rather than
/// off this list, so a kind added to the enum and forgotten here still gets
/// a button — under "Anything else", where it is obvious. Named kinds, not
/// numbers: the fuel tank's retirement renumbered half the enum, and a
/// table of numbers quietly moved every part after it into the wrong
/// group.
pub const PART_GROUPS: &[(&str, &[u32])] = &[
    // Hull first, in the order a ship is actually built: deck, skin, then the
    // ways through it. The frame is not a tool of its own — see NOT_A_TOOL.
    (
        "Hull",
        &[
            PartKind::Floor as u32,
            PartKind::Wall as u32,
            PartKind::DiagonalWall as u32,
            PartKind::OutsideWall as u32,
            PartKind::DiagonalOutsideWall as u32,
            PartKind::Door as u32,
            PartKind::Airlock as u32,
            PartKind::Sandbags as u32,
        ],
    ),
    (
        "Systems",
        &[
            PartKind::Engine as u32,
            PartKind::HeavyEngine as u32,
            PartKind::Thruster as u32,
            PartKind::Hyperdrive as u32,
            PartKind::Helm as u32,
            PartKind::Reactor as u32,
            PartKind::FusionReactor as u32,
            PartKind::PowerConduit as u32,
            PartKind::Battery as u32,
            PartKind::LifeSupport as u32,
            PartKind::SensorArray as u32,
        ],
    ),
    (
        "Crew",
        &[
            PartKind::Bunk as u32,
            PartKind::Table as u32,
            PartKind::Chair as u32,
            PartKind::Shower as u32,
            PartKind::TradingDesk as u32,
            PartKind::ResearchDesk as u32,
        ],
    ),
    (
        "Galley",
        &[
            PartKind::ColdStore as u32,
            PartKind::Worktop as u32,
            PartKind::Hob as u32,
            PartKind::Dishwasher as u32,
        ],
    ),
    ("Heads", &[PartKind::Toilet as u32, PartKind::Basin as u32]),
    (
        "Storage",
        &[PartKind::Shelf as u32, PartKind::SuitLocker as u32],
    ),
    (
        "Bay",
        &[PartKind::HydroBay as u32, PartKind::BroomLocker as u32],
    ),
    (
        "Workshop",
        &[PartKind::Workbench as u32, PartKind::Armoury as u32],
    ),
    (
        "Light",
        &[PartKind::WallLight as u32, PartKind::StandingLight as u32],
    ),
    // What makes a deck nicer to stand on and does nothing else: the
    // surroundings need reads them.
    (
        "Comforts",
        &[
            PartKind::SmallPlant as u32,
            PartKind::BigPlant as u32,
            PartKind::Picture as u32,
        ],
    ),
];

/// Kinds the palette does not offer, though the ship knows them. Structure
/// is one: deck plating lays its own frame, so to a player the frame and
/// the deck are one thing and a second button for the half underneath would
/// be a trap. The other five are a planet's — a field in the soil and the
/// wild round a town — and no ship carries them.
pub const NOT_A_TOOL: &[u32] = &[
    PartKind::Structure as u32,
    PartKind::Field as u32,
    PartKind::Tree as u32,
    PartKind::Shrub as u32,
    PartKind::Boulder as u32,
    PartKind::Water as u32,
];

/// What a station sells, indexed by `physics::ResourceId`'s code — blank
/// where a resource went (15 to 17, task 127).
pub const RESOURCE_NAMES: [&str; 22] = [
    "Vegetables",
    "Tofu",
    "Suits",
    "Handguns",
    "Medkits",
    "Bandages",
    "Helm",
    "Kevlar",
    "Leg guards",
    "Shotguns",
    "Auto rifles",
    "Sniper rifles",
    "Schwords",
    "Research keys",
    "Tier-two keys",
    // 15 to 17 were the engineer's kits and the soldier's grenade, gone
    // when a class's charges became counters (task 127).
    "",
    "",
    "",
    "Miniguns",
    "Rail lances",
    "Arc greaves",
    "Reflective plates",
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

/// Where goods are stowed, indexed by `economy::Storage`.
pub const STORAGE_NAMES: [&str; 3] = ["Cold stores", "Lockers", "Research desk"];

/// How many units a buy or sell button moves.
pub const TRADE_STEPS: [u32; 3] = [1, 10, 100];

/// Why an edit was refused. Indexed by `EditError`; 0 never appears because
/// 0 is "it took".
pub fn edit_line(code: u32) -> &'static str {
    match code {
        1 => "That falls outside the build area.",
        2 => "Something is already standing there.",
        3 => "There is no deck under it.",
        4 => "There is deck there already.",
        5 => "There is not the money left for that.",
        6 => "That part is not there any more.",
        7 => "Take what is standing on it off first.",
        8 => "The page asked for something that is not a part.",
        9 => "The design is settled — nothing can be moved now.",
        10 => "There is no structure under it. The frame goes down first.",
        11 => "There is already something in that tile on that layer.",
        12 => "There is not the money left for those goods.",
        13 => "Nowhere aboard to put them — the ship needs more storage.",
        14 => "There is not that much aboard to sell.",
        15 => "Sell what is in it first.",
        16 => "There are not the materials aboard to build that.",
        17 => "This station does not sell that.",
        18 => "A wall light hangs from a wall — turn it to one, or put it beside one.",
        _ => "That could not be done.",
    }
}

/// What is wrong with the design. Indexed by `IssueCode` in
/// `crates/shipdesign/src/validate.rs`. An issue with no line here is
/// dropped from the list rather than shown as a placeholder.
pub fn issue_line(code: u32) -> Option<&'static str> {
    Some(match code {
        1 => "The ship is in more than one piece.",
        2 => "Not enough bunks for the crew.",
        3 => "Not enough chairs for the crew.",
        4 => "No table to eat at.",
        5 => "No cold store to keep food in.",
        6 => "No worktop to prepare it on.",
        7 => "No hob to cook it on.",
        8 => "No dishwasher to clear up with.",
        9 => "No toilet.",
        10 => "No basin to wash at.",
        11 => "Somewhere a Bim has to stand is blocked.",
        12 => "Parts nobody could walk between.",
        20 => "No engine — the ship goes nowhere.",
        21 => "No engine pushes it forward, so it cannot set off.",
        22 => "No hydroponic bay. The food aboard is all the food there will be.",
        23 => "No broom locker, so nothing to sweep the deck with.",
        24 => "The outside can see in. The crew will be irradiated here.",
        25 => "Nothing to eat aboard.",
        26 => "No helm, so nobody can fly it.",
        27 => "No thruster, so nothing turns the ship.",
        28 => "No airlock, so no way off it — a station can only be held beside.",
        29 => "No sensor array. Nothing will be seen beyond eyesight.",
        31 => {
            "The airlock is sealed in — no side of it opens onto space, so the ship cannot dock by it. Put it in the skin."
        }
        32 => {
            "An engine is firing into the ship — the tiles behind its bell have to be open space. Put it at the stern, bell outwards."
        }
        33 => "Nothing powers this. Run conduit under it from a reactor.",
        34 => {
            "This run draws more than its reactor makes. The batteries will go flat and the ship will brown out."
        }
        35 => {
            "The reactor cannot feed these engines flat out: the ship will push with a fraction of its thrust. Add a reactor, or take an engine off."
        }
        36 => {
            "The hyperdrive is bolted to nothing: put it against a main engine, block to block, or it will never jump."
        }
        37 => {
            "A wall light or a picture with no wall at its back: put it against a bulkhead or the hull."
        }
        _ => return None,
    })
}

/// The one issue that is not just another row: radiation is the only fault
/// on the list that kills people, it is always first, and it is styled
/// louder than the errors. It does not block; it shouts.
pub const ISSUE_GRAVE: u32 = 24;

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
        Refusal::NotForHire => "that is not a mercenary for hire",
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
        Refusal::NotAPickLevel => "that level has no talent to pick",
        Refusal::LevelNotReached => "that level has not been reached",
        Refusal::AlreadyPicked => "that level's talent is picked, and a pick is never changed",
        Refusal::NoClass => "a crew member with no class has no talents to pick",
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
        Refusal::NoTauntYet => "a taunt wants the third level",
        Refusal::NotACommander => "only a commander can do that",
        Refusal::NoSquadInRange => "nobody of the squad is near enough to hear it",
        Refusal::NoEnemyThere => "there is no enemy under the pointer",
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
        Refusal::TooFar => "that is more than one hyperlane hop away — one hop a trip",
        Refusal::NoProposal => "nobody has put a destination to the crew",
        Refusal::NotAsked => "nobody is being asked about leaving",
        Refusal::PlayerOut => "your Bim is dead — it is back when the mission ends",
        Refusal::CannotTravel => "the ship cannot get there — nothing pushes it",
        Refusal::AlreadyHere => "the crew are here — the next trip goes somewhere else",
        Refusal::NoRelicChoice => "there is no relic to choose now",
        Refusal::NotOnOffer => "that relic is not on offer",
        Refusal::NotAPlayer => "only a player's Bim holds a relic",
        Refusal::NoCache => "there is no relic cache here — walk over first",
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
        Refusal::TopTier => "tier three is as far as combining goes",
        Refusal::NotAPair => "only two of one kind at one tier combine",
        Refusal::NoRestock => "nobody holds Restock Codes",
        Refusal::Restocked => "the shelf has been restocked once this visit already",
        Refusal::NoRankedKit => "only a class with four ranked abilities buys ranks",
        Refusal::NoSkillPoint => "no skill point to spend — the next comes with the next level",
        Refusal::TopRank => "that ability is at its top rank",
        Refusal::RankLocked => "the next rank of that ability wants a higher level",
        Refusal::NotLearnt => "that ability wants a rank first",
        Refusal::AlreadyActive => "it is already running",
        Refusal::OutOfCloakRange => "they are too far off to cloak",
        Refusal::NoSightOfTarget => "the medic cannot see them",
        Refusal::Cloaked => "a cloaked Bim uses no ability",
        Refusal::AwaitingReady => "not yet — the mission starts when every player is ready",
        Refusal::NoReadyCheck => "the mission is already under way",
        Refusal::FightOver => "the fight is won — press Back to ship",
        Refusal::OtherSiteChosen => "the crew fought this system's other site",
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
    }
}

/// Why a room closed under you.
pub fn room_closed(why: wire::Closed) -> &'static str {
    match why {
        wire::Closed::HostLeft => "The host left, and the lobby closed with them.",
    }
}

/// An Accept refused: it was made against a ship that has since changed,
/// or a game that has since begun.
pub const ACCEPT_STALE: &str = "That Accept was for a ship that has since changed.";
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
pub const LOAD_GUEST: &str = "Only the host can load a game.";
/// The Esc sheet's Restart (feature 79): the menu's button, the page's
/// line, the button that does it, and the two reasons it is greyed.
pub const RESTART_BUTTON: &str = "Restart";
pub const RESTART_LINE: &str = "Play this run again from the situation it opened in — the fight, the town, the landing, or the run the lobby started. Everything since is lost, and a saved game is not touched.";
pub const RESTART_AGAIN: &str = "Start again";
pub const RESTART_NONE: &str = "Nothing to restart yet: the run starts when the ship is accepted.";
/// A guest's Restart is greyed with this, as its Load is.
pub const RESTART_GUEST: &str = "Only the host can restart the run.";
/// The line the log carries after a restart, so the screen says what
/// happened as well as showing it.
pub const RESTART_DONE: &str = "Back at the beginning.";
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
/// The setup's name field: what the player calls their crew member.
pub const BIM_NAME: &str = "Your Bim";
pub const BIM_NAME_NOTE: &str = "What your crew member is called; blank keeps the crew's own name";
/// The field's hint, and what the lobby's roster prints for a Bim not
/// yet named: the crew's own name for that berth.
pub const BIM_NAME_HINT: &str = "a name";
/// The setup's hair chooser: the row's title and its note, and the name
/// of every style and every colour, in `bims::character::Hair::ALL`'s and
/// `Shade::ALL`'s order (feature 62).
pub const BIM_HAIR: &str = "Hair";
pub const BIM_HAIR_NOTE: &str = "How your crew member wears it, and its colour";
/// And the colour chooser (feature 84): the ring on the deck that says
/// which of the crew is yours. One a player, so a colour somebody else
/// in the lobby has taken is not offered.
pub const BIM_TINT: &str = "Your colour";
pub const BIM_TINT_NOTE: &str =
    "The ring on the deck under your own crew member. The crew nobody steers have none.";
pub const BIM_TINT_TAKEN: &str = "taken";
/// The class chooser (features 74 and 75): a name a `world::Class`, in
/// `Class::ALL`'s order, with a line each saying what it does; and a
/// name and a line a `world::Talent`, in `Talent::ALL`'s order — each
/// class's seven pick levels' two each, left then right, the engineer's
/// fourteen, then the soldier's, the medic's and the tank's.
pub const BIM_CLASS: &str = "Class";
pub const BIM_CLASS_NOTE: &str = "What your crew member is. One class each, chosen before the ship leaves its first berth. A class adds its own two keys and its talents, and nothing else: every crew member does every job and brings the same money";
pub const CLASS_NAMES: [&str; 6] = ["None", "Engineer", "Soldier", "Medic", "Tank", "Commander"];
pub const CLASS_TIPS: [&str; 6] = [
    "No class: learns nothing.",
    "Four ranked abilities, a skill point a level: an EMP that stuns the machines (Q), a Healing Sentry that heals the crew round it (C), sandbags for cover (E), and for its ultimate a sentry with a minigun (R). Its charges come back on their own cooldowns, and it packs its sandbags and Healing Sentries up again.",
    "Braces to hold a line (E) — steadier shooting and no errands until stood easy — and from the third level throws grenades (Q), two charges of them, each back thirty seconds after it is thrown. Sets out with an auto rifle in hand and the pistol in the pack.",
    "Four ranked abilities, a skill point a level: a Nanite Burst that heals everybody near him at once (Q), a Healing Aura that makes every heal worth more to the crew round him (C), the heal beam on a crewmate or himself (E), and for his ultimate a cloak no enemy can pick (R). Revives a downed crewmate in four seconds where anybody else takes ten, and gets them up at 40% of their bar where anybody else manages 30%.",
    "Stands as a wall (E) — half pace, and the crew close behind him are in cover against anything shot through him — and from the third level taunts (Q), so every enemy that can see him shoots at him and nobody else for six minutes. His armour drains at half rate, so the same kevlar takes twice as much on him. Sets out with the pistol and a basic helm, kevlar and leg guards on.",
    "Lifts every friendly Bim within eight tiles of him — yours as well as the crew's — a tenth faster at work, and a tenth steadier with a gun; and orders the squad, which is every crew member nobody is steering: attack the enemy under the pointer (E), fall back to a tile (X), stand ground (Z). From the third level he rallies (Q). Hires a mercenary at a quarter off. Sets out with the pistol.",
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
/// The two boxes at the foot of the screen (feature 80): what the class's
/// own keys do, by class code, the **primary** (Q) first and the
/// **secondary** (E) second — the same pairing `screens::game::class_key`
/// dispatches by, so a box and the key under it are never two answers.
/// `Class::None` has no keys and no boxes; its row is there so a code
/// indexes the table.
pub const ABILITY_NAMES: [[&str; 2]; 6] = [
    ["", ""],
    ["EMP", "Sandbags"],
    ["Grenade", "Brace"],
    ["Nanite Burst", "Heal Beam"],
    ["Taunt", "Wall"],
    ["Battle Cry", "Rally"],
];
/// What the engineer's EMP and sandbags do (task 127): its Q box's tip and
/// its ranked ability's words alike. One line each, Dota 2's way: the
/// numbers are the [`Stat`] rows under it.
const EMP_WHAT: &str = "Throw an EMP that bursts after 2 s, stunning every machine in the blast: no moving, aiming or firing, and a Guardian's shield drops. The Heart is immune.";
const SANDBAGS_WHAT: &str = "Lay sandbags on the tile under the pointer: low cover for anyone behind them, worn down by the hits they stop.";
/// What the medic's Nanite Burst and heal beam do (task 130): his Q and E
/// boxes' tips and his ranked abilities' words alike.
const NANITE_BURST_WHAT: &str =
    "Heal every standing ally around you in sight, yourself included. Revives nobody.";
const HEAL_BEAM_WHAT: &str = "Toggle. Beam the crewmate under the pointer, or yourself, healing over time. The number is how many more you could link.";
/// What each box says when it is rested on.
pub const ABILITY_TIPS: [[&str; 2]; 6] = [
    ["", ""],
    [EMP_WHAT, SANDBAGS_WHAT],
    [
        "Throw a grenade that bursts after 2 s, hurting everyone in the blast, allies too. The number is the grenades in the pack.",
        "Toggle. Brace where you stand for steadier aim. Moving ends it.",
    ],
    [NANITE_BURST_WHAT, HEAL_BEAM_WHAT],
    [
        "Every enemy that can see you shoots at you alone.",
        "Toggle. Walk at half pace; a crewmate close behind you is in cover.",
    ],
    [
        "Nearby allies fire faster.",
        "Nearby allies take less damage and move faster.",
    ],
];
pub fn ability_name(class: world::Class, primary: bool) -> &'static str {
    ABILITY_NAMES
        .get(class.code() as usize)
        .map(|pair| pair[usize::from(!primary)])
        .unwrap_or("")
}
pub fn ability_tip(class: world::Class, primary: bool) -> &'static str {
    ABILITY_TIPS
        .get(class.code() as usize)
        .map(|pair| pair[usize::from(!primary)])
        .unwrap_or("")
}
/// The numbers under a class key's box that is not a ranked kit's: the
/// tank's, the one class of talents left. Base values — a talent that
/// stretches one is on the Skills tab.
pub fn ability_stats(class: world::Class, primary: bool) -> Vec<Stat> {
    use world::class as c;
    match (class, primary) {
        (world::Class::Tank, true) => vec![
            Stat::one("Radius", " tiles", fig(c::TAUNT_RADIUS as f64)),
            Stat::one("Duration", " min", fig(c::TAUNT_MINUTES)),
            Stat::one("Cooldown", " s", fig(c::TAUNT_COOLDOWN)),
        ],
        (world::Class::Tank, false) => vec![
            Stat::one("Move speed", "", by(c::BULWARK_PACE as f64)),
            Stat::one("Cover reach", " tiles", fig(c::BULWARK_REACH as f64)),
        ],
        _ => Vec::new(),
    }
}
/// The boxes past the class's own two (feature 86): the commander's
/// other two squad orders, which had keys and no box until now, and the
/// medic's carry. Named off the [`crate::keys::Action`] rather than off
/// a class's pair, since these are one class's each and a pair has no
/// room for a third.
/// The squad's attack (task 129: off E, which the Rally took, onto a key
/// of its own).
pub const SQUAD_ORDER_ATTACK: &str = "Squad attack";
pub const SQUAD_ORDER_ATTACK_TIP: &str = "Send the squad (every crew member nobody steers) at the enemy under the pointer. Again on it to call them off. The number is the squad's size.";
pub const FALL_BACK: &str = "Fall back";
pub const FALL_BACK_TIP: &str = "The squad holds fire and walks back to the pointer, or to you. The number is the squad's size.";
pub const STAND_GROUND: &str = "Stand ground";
pub const STAND_GROUND_TIP: &str =
    "The squad holds where it stands and shoots what it sees. The number is the squad's size.";
pub const CARRY: &str = "Carry";
pub const CARRY_TIP: &str = "Pick up the downed crewmate under the pointer and carry them out of the fire, slowly and without shooting. Again to set them down; their countdown keeps running. The number is how many near you are down.";

/// The four abilities of a ranked kit (task 124), Q C E R, by class and
/// slot: what the box, the log and the Skills tab call each. Empty for a
/// class with no ranked kit.
pub fn ranked_ability(class: world::Class, slot: u8) -> &'static str {
    match (class, slot) {
        (world::Class::Soldier, 0) => "Frag Grenade",
        (world::Class::Soldier, 1) => "Weak Spot",
        (world::Class::Soldier, 2) => "Brace",
        (world::Class::Soldier, 3) => "Rampage",
        (world::Class::Engineer, 0) => "EMP",
        (world::Class::Engineer, 1) => "Healing Sentry",
        (world::Class::Engineer, 2) => "Sandbags",
        (world::Class::Engineer, 3) => "Sentry",
        (world::Class::Commander, 0) => "Battle Cry",
        (world::Class::Commander, 1) => "Command Aura",
        (world::Class::Commander, 2) => "Rally",
        (world::Class::Commander, 3) => "Reinforcements",
        (world::Class::Medic, 0) => "Nanite Burst",
        (world::Class::Medic, 1) => "Healing Aura",
        (world::Class::Medic, 2) => "Heal Beam",
        (world::Class::Medic, 3) => "Cloak",
        _ => "",
    }
}
/// What a ranked ability does, in a line, whatever its rank: Dota 2's
/// way, the numbers left to [`ranked_stats`].
pub fn ranked_what(class: world::Class, slot: u8) -> &'static str {
    match (class, slot) {
        (world::Class::Soldier, 0) => {
            "Throw a grenade that bursts after 2 s, hurting everyone in the blast, allies too. Half damage at the edge."
        }
        (world::Class::Soldier, 1) => {
            "Passive. Your hits may strike a weak spot for extra damage. Grenades never do."
        }
        (world::Class::Soldier, 2) => {
            "Toggle. Brace where you stand: steadier aim and less damage taken. Moving ends it."
        }
        (world::Class::Soldier, 3) => {
            "Ultimate. Fire faster, take less damage and aim on the move. Stacks with Brace."
        }
        (world::Class::Engineer, 0) => EMP_WHAT,
        (world::Class::Engineer, 1) => {
            "Lay a sentry that heals crewmates in its reach and sight. One per charge; a new one replaces your oldest."
        }
        (world::Class::Engineer, 2) => SANDBAGS_WHAT,
        (world::Class::Engineer, 3) => {
            "Ultimate. Lay a minigun sentry that shoots whatever it sees until it expires or is destroyed. Ready at every mission's start."
        }
        (world::Class::Commander, 0) => "Allies around you as you shout fire faster.",
        (world::Class::Commander, 1) => {
            "Passive. Allies near you deal more damage. Does not stack."
        }
        (world::Class::Commander, 2) => {
            "Allies around you as you call it take less damage and move faster."
        }
        (world::Class::Commander, 3) => {
            "Ultimate, passive. Every mission starts with Republic soldiers beside you. They follow squad orders; the fallen are back next mission."
        }
        (world::Class::Medic, 0) => NANITE_BURST_WHAT,
        (world::Class::Medic, 1) => {
            "Passive. Allies near you receive more from every heal. Does not stack."
        }
        (world::Class::Medic, 2) => HEAL_BEAM_WHAT,
        (world::Class::Medic, 3) => {
            "Ultimate. Cloak an ally, downed or not, or yourself: enemies ignore them, they move faster but cannot shoot. Blasts still hit."
        }
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
/// A ranked ability's numbers, every rank's at once (Dota 2's tooltip):
/// the rules crates' own tables.
pub fn ranked_stats(class: world::Class, slot: u8) -> Vec<Stat> {
    use world::class as c;
    let charges = |n: &'static [u32; 4]| Stat::ranks("Charges", "", move |r| n[r].to_string());
    let cooldown = |s: &'static [f64; 4]| Stat::ranks("Cooldown", " s", move |r| fig(s[r]));
    match (class, slot) {
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
            Stat::ranks("Fewer misses", "", |r| pc(c::BRACE_MISS_CUT[r] as f64)),
            Stat::ranks("Damage taken", "", |r| by(c::BRACE_DAMAGE_TAKEN[r] as f64)),
            Stat::from_rank(
                "Full aim at range",
                c::BRACE_DEADEYE_RANK,
                "Yes".to_string(),
            ),
        ],
        (world::Class::Soldier, 3) => vec![
            Stat::ranks("Duration", " s", |r| fig(c::RAMPAGE_SECONDS[r])),
            Stat::ranks("Fire rate", "", |r| by(c::RAMPAGE_FIRE_RATE[r] as f64)),
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
            Stat::ranks("Radius", " tiles", |r| fig(c::EMP_RADIUS[r] as f64)),
            Stat::ranks("Stun", " s", |r| fig(c::EMP_STUN[r] as f64)),
            Stat::one("Range", " tiles", fig(c::GRENADE_RANGE as f64)),
            Stat::from_rank(
                "Stunned take",
                c::EMP_EXPOSE_RANK,
                format!("+{}%", c::EMP_EXPOSE_PERCENT),
            ),
            charges(&c::EMP_CHARGES),
            cooldown(&c::EMP_COOLDOWN),
        ],
        (world::Class::Engineer, 1) => vec![
            Stat::ranks("Heal", " /s", |r| {
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
            Stat::ranks("Bag health", "", |r| fig(c::SANDBAG_HEALTH[r] as f64)),
            Stat::ranks("Lay time", " min", |r| fig(c::SANDBAG_MINUTES[r])),
            Stat::ranks("Tiles a charge", "", |r| {
                if r + 1 >= c::SANDBAG_DOUBLE_RANK as usize {
                    "2".to_string()
                } else {
                    "1".to_string()
                }
            }),
            charges(&c::SANDBAG_CHARGES),
            cooldown(&c::SANDBAG_COOLDOWN),
        ],
        (world::Class::Engineer, 3) => vec![
            Stat::ranks("Minigun tier", "", |r| c::SENTRY_TIER[r].code().to_string()),
            Stat::ranks("Fire rate", "", |r| by(c::SENTRY_FIRE_RATE[r] as f64)),
            Stat::ranks("Health", "", |r| fig(c::SENTRY_HEALTH[r] as f64)),
            Stat::ranks("Duration", " s", |r| fig(c::SENTRY_SECONDS[r])),
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
            Stat::ranks("Damage", "", |r| by(c::AURA_DAMAGE[r] as f64)),
            Stat::ranks("Radius", " tiles", |r| fig(c::AURA_TILES[r] as f64)),
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
            Stat::one(
                "Arrive within",
                " tiles",
                fig(c::REINFORCEMENT_REACH_TILES as f64),
            ),
        ],
        (world::Class::Medic, 0) => vec![
            Stat::ranks("Heal", "", |r| fig(c::NANITE_BURST_HEAL[r] as f64)),
            Stat::ranks(
                "Radius",
                " tiles",
                |r| fig(c::NANITE_BURST_RADIUS[r] as f64),
            ),
            cooldown(&c::NANITE_BURST_COOLDOWN),
        ],
        (world::Class::Medic, 1) => vec![
            Stat::ranks("Healing received", "", |r| {
                by(c::HEALING_AURA_FACTOR[r] as f64)
            }),
            Stat::ranks(
                "Radius",
                " tiles",
                |r| fig(c::HEALING_AURA_RADIUS[r] as f64),
            ),
        ],
        (world::Class::Medic, 2) => vec![
            Stat::ranks("Heal", " /s", |r| {
                fig((c::HEAL_BEAM_HP * c::HEAL_BEAM_RATE[r]) as f64 / 60.0)
            }),
            Stat::ranks("Range", " tiles", |r| fig(c::HEAL_BEAM_RANGES[r] as f64)),
            Stat::ranks("Patients", "", |r| c::HEAL_BEAM_PATIENTS[r].to_string()),
            Stat::from_rank(
                "Fire rate while beaming",
                c::HEAL_BEAM_FIRE_RANK,
                by(c::HEAL_BEAM_FIRE_RATE as f64),
            ),
        ],
        (world::Class::Medic, 3) => vec![
            Stat::ranks("Duration", " s", |r| fig(c::CLOAK_SECONDS[r])),
            Stat::ranks("Move speed", "", |r| by(c::CLOAK_PACE[r] as f64)),
            Stat::one("Range", " tiles", fig(c::CLOAK_RANGE as f64)),
            cooldown(&c::CLOAK_COOLDOWN),
        ],
        _ => Vec::new(),
    }
}
/// The foot of a ranked ability's tip: the rank it is at, and the level
/// the next wants.
pub fn ranked_foot(class: world::Class, slot: u8, rank: u8) -> String {
    let top = world::class::MAX_RANK;
    let now = if rank == 0 {
        "Not learnt.".to_string()
    } else {
        format!("Rank {rank}/{top}.")
    };
    match world::class::rank_level(class, slot, rank + 1).filter(|_| rank < top) {
        Some(level) => format!(
            "{now} Next, rank {} at level {level}: Ctrl-click to learn.",
            rank + 1
        ),
        None => format!("{now} The top rank."),
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
/// And for a Rampage refused.
pub fn rampage_refused(why: world::Refusal) -> String {
    format!("Cannot go on a Rampage: {}.", refusal(why))
}
/// And for a Nanite Burst and a Cloak refused (task 130).
pub fn nanite_burst_refused(why: world::Refusal) -> String {
    format!("Cannot set off a Nanite Burst: {}.", refusal(why))
}
pub fn cloak_refused(why: world::Refusal) -> String {
    format!("Cannot cloak: {}.", refusal(why))
}
/// The Skills tab of a ranked kit (task 124): what it says of the points
/// waiting, and a rank's line and button.
pub const RANKED_SKILLS_TIP: &str = "A skill point a level, from the first. Each buys one rank of one of your four abilities — Ctrl and its key, a Ctrl-click on its box, or the button here. Q, C and E rank up at levels 1, 3, 5 and 7; R, the ultimate, at 6, 9, 12 and 15. A point not spent is kept.";
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
/// The talent tree (features 83 and 107): the class's ten levels as a tree on the
/// character sheet, what a level's slot says, and the button that spends a point.
pub const SKILLS_TIP: &str = "Your own crew member's class, level by level. A level with two slots is a choice: one skill point, spent on one of them, and it cannot be changed. The levels with one slot come on their own as you climb.";
pub const SKILLS_NO_CLASS: &str = "This crew member has no class, so there is nothing to learn. A class is chosen on the setup tab, before the ship first leaves its berth.";
/// How many picks are waiting: what a point is spent on, and how many
/// there are.
pub fn skill_points(points: usize) -> String {
    match points {
        0 => "No skill points — the next one comes with the next level that offers a choice."
            .to_string(),
        1 => "One skill point to spend.".to_string(),
        n => format!("{n} skill points to spend."),
    }
}
pub const SKILLS_HINT: &str = "Click a slot for what it does. A slot at a level you have reached, at a level you have not chosen at, is yours for a point.";
pub const SKILL_LEARN: &str = "Learn";
pub const SKILL_LEARN_TIP: &str =
    "Spends a skill point on this slot. The other at the level is given up for good.";
/// What the tree says under the slot picked, by its state.
pub const SKILL_LEARNT: &str = "Learnt.";
pub const SKILL_GIVEN_UP: &str = "Given up: the other slot at this level was taken.";
pub const SKILL_COMES_WITH: &str = "Comes with the level, with nothing to choose.";
pub fn skill_locked(level: u8) -> String {
    format!("Waiting on level {level}.")
}
pub const SKILL_OPEN: &str = "Open: one skill point, and it cannot be changed.";
/// Where a slot of the Skills tree sits, said under its name in the
/// column beside the tree: the level, and whether it comes with the
/// level or is one of the two that level offers.
pub fn skill_slot_line(level: u8, pick: bool) -> String {
    if pick {
        format!("Level {level} — one of two")
    } else {
        format!("Level {level} — the level's own")
    }
}
/// What a talent that does nothing any more says, in its tip and in its
/// numbers (task 120): the talents the old body's blood, wounds, traumas
/// and running fed — the medic's dressings and treatments, the nerve that
/// held a body from running, the bleeding a taunt or a rally stopped —
/// keep their names and their slots on the tree until they are designed
/// again, and say so in the one phrase rather than a promise the rules no
/// longer keep.
pub const TALENT_NO_EFFECT: &str = "No effect for now — to be redesigned.";
/// A talent's name by its code: a code no talent has any more (the
/// engineer's, 0 to 13, task 127, and the soldier's, 14 to 27, task 124) is
/// an empty place.
pub const TALENT_NAMES: [&str; 55] = [
    // 0 to 13 were the engineer's talents, gone with its ranked kit
    // (task 127): the codes stay free, and so do their places here.
    "",
    "",
    "",
    "",
    "",
    "",
    "",
    "",
    "",
    "",
    "",
    "",
    "",
    "",
    "",
    "",
    "",
    "",
    "",
    "",
    "",
    "",
    "",
    "",
    "",
    "",
    "",
    "",
    // 28 to 41 were the medic's talents, gone with his ranked kit
    // (task 130): the codes stay free, and so do their places here.
    "",
    "",
    "",
    "",
    "",
    "",
    "",
    "",
    "",
    "",
    "",
    "",
    "",
    "",
    "Plated",
    "Breacher",
    "Unmovable",
    "Wide wall",
    "Fast wall",
    "Loud taunt",
    "Long taunt",
    "Hold fast",
    "Guarded",
    "Interpose",
    "Magnet",
    "Fortress",
    "Rallying wall",
    // 55 to 68 were the commander's talents, gone with his ranked kit
    // (task 129): the codes stay free; no talent follows them, so the
    // table ends at the last one there is.
];
pub const TALENT_TIPS: [&str; 55] = [
    // 0 to 13 were the engineer's talents, gone with its ranked kit
    // (task 127): the codes stay free, and so do their places here.
    "",
    "",
    "",
    "",
    "",
    "",
    "",
    "",
    "",
    "",
    "",
    "",
    "",
    "",
    // 14 to 27 were the soldier's talents, gone with its ranked kit (task
    // 124): the codes stay free, and so do their places here.
    "",
    "",
    "",
    "",
    "",
    "",
    "",
    "",
    "",
    "",
    "",
    "",
    "",
    "",
    // 28 to 41 were the medic's talents, gone with his ranked kit
    // (task 130): the codes stay free, and so do their places here.
    "",
    "",
    "",
    "",
    "",
    "",
    "",
    "",
    "",
    "",
    "",
    "",
    "",
    "",
    "Every piece of armour he wears protects half again as much.",
    "Forces a locked door in half the time.",
    TALENT_NO_EFFECT,
    "The wall shelters twice as far to either side.",
    "Walks half again as fast with the wall up.",
    "A taunt reaches half again as far.",
    "A taunt lasts half again as long.",
    TALENT_NO_EFFECT,
    "Ten per cent more chance of slipping a bolt while the wall is up.",
    "A bolt that would hit somebody the wall shelters hits him instead.",
    "A taunt turns every charging blade within its reach towards him.",
    "His armour drains at half rate again — a quarter of anybody else's.",
    "While he taunts, every crew member within three tiles drains armour at half rate too.",
    // 55 to 68 were the commander's talents, gone with his ranked kit
    // (task 129): the codes stay free; no talent follows them, so the
    // table ends at the last one there is.
];
pub fn talent_name(talent: world::Talent) -> &'static str {
    TALENT_NAMES
        .get(talent.code() as usize)
        .copied()
        .unwrap_or("Talent")
}
pub fn talent_tip(talent: world::Talent) -> &'static str {
    TALENT_TIPS
        .get(talent.code() as usize)
        .copied()
        .unwrap_or("")
}
/// The fixed levels' lines, for the panel: what a class's first, third
/// and seventh give.
pub fn level_line(class: world::Class, level: u8) -> Option<&'static str> {
    Some(match (class, level) {
        (world::Class::Tank, 1) => "Bulwark: stands as a wall, and his armour drains at half rate.",
        (world::Class::Tank, 2) => {
            "Plated: every piece of armour he wears protects half again as much."
        }
        (world::Class::Tank, 3) => "Taunt: may draw the enemy's fire onto himself.",
        (world::Class::Tank, 7) => "Iron frame: a hit rolled on his head lands on his body.",
        _ => return None,
    })
}
/// What a fixed level is *called*, for its one slot on the Skills tree —
/// the same three levels [`level_line`] describes, said in a word or two.
/// `None` wherever `level_line` is `None`: a pick level has two talents
/// with names of their own, and the classless one has nothing at all.
pub fn level_name(class: world::Class, level: u8) -> Option<&'static str> {
    Some(match (class, level) {
        (world::Class::Tank, 1) => "Bulwark",
        (world::Class::Tank, 2) => "Plated",
        (world::Class::Tank, 3) => "Taunt",
        (world::Class::Tank, 7) => "Iron frame",
        _ => return None,
    })
}

/// A number said the way the Skills tree says one: two decimals at most,
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

/// A factor, the way a talent's line says one: `×1.15`.
fn by(v: f64) -> String {
    format!("×{}", fig(v))
}

/// A before and an after: `6 -> 9`, with the unit said once at the end.
fn step(before: f64, after: f64, unit: &str) -> String {
    let (a, b) = (fig(before), fig(after));
    if unit.is_empty() {
        format!("{a} -> {b}")
    } else {
        format!("{a} -> {b} {unit}")
    }
}

/// A percentage, whole: `0.75` is "75%".
fn pc(v: f64) -> String {
    format!("{}%", fig(v * 100.0))
}

/// **What a talent is actually worth, in numbers** — the line the Skills
/// tree puts under a slot's name, so the choice between two of them is a
/// choice between two figures rather than between two adjectives. Every
/// number here is the rules crates' own constant: `world::class`'s
/// factors, and the base each one multiplies, wherever there is one to
/// show — a sentry's health, a beam's reach, a grenade's fuse. A talent
/// that multiplies something the weapon in hand decides (accuracy, fire
/// rate, melee damage) says the factor and, where it helps, what it does
/// to one worked figure.
pub fn talent_numbers(talent: world::Talent) -> String {
    use world::Talent as T;
    use world::class as c;
    match talent {
        // The medic's went with his ranked kit (task 130).
        // --- the tank's ---
        T::Plated => format!(
            "Every piece of armour he wears protects {}",
            by(c::PLATED_PROTECTION as f64)
        ),
        T::Breacher => format!(
            "Forcing a locked door {} seconds, an airlock {} ({})",
            step(
                bims::door::SMASH_DOOR as f64,
                (bims::door::SMASH_DOOR * c::BREACHER_TIME) as f64,
                ""
            ),
            step(
                bims::door::SMASH_AIRLOCK as f64,
                (bims::door::SMASH_AIRLOCK * c::BREACHER_TIME) as f64,
                ""
            ),
            by(c::BREACHER_TIME as f64)
        ),
        T::Unmovable => TALENT_NO_EFFECT.to_string(),
        T::WideWall => format!(
            "Bulwark reach {} tiles ({})",
            step(
                c::BULWARK_REACH as f64,
                (c::BULWARK_REACH * c::WIDE_WALL_REACH) as f64,
                ""
            ),
            by(c::WIDE_WALL_REACH as f64)
        ),
        T::FastWall => format!(
            "Pace with the wall up {} of his own ({})",
            step(
                c::BULWARK_PACE as f64,
                (c::BULWARK_PACE * c::FAST_WALL_PACE) as f64,
                ""
            ),
            by(c::FAST_WALL_PACE as f64)
        ),
        T::LoudTaunt => format!(
            "Taunt radius {} tiles ({})",
            step(
                c::TAUNT_RADIUS as f64,
                (c::TAUNT_RADIUS * c::LOUD_TAUNT_RADIUS) as f64,
                ""
            ),
            by(c::LOUD_TAUNT_RADIUS as f64)
        ),
        T::LongTaunt => format!(
            "A taunt lasts {} game minutes ({}); {} seconds between them either way",
            step(c::TAUNT_MINUTES, c::TAUNT_MINUTES * c::LONG_TAUNT_TIME, ""),
            by(c::LONG_TAUNT_TIME),
            fig(c::TAUNT_COOLDOWN)
        ),
        T::HoldFast => TALENT_NO_EFFECT.to_string(),
        T::Guarded => format!(
            "Odds of slipping a bolt while Bulwark is up +{}",
            pc(c::GUARDED_DODGE as f64)
        ),
        T::Interpose => format!(
            "A bolt that would hit anybody within the wall's {} tiles hits him instead",
            fig(c::BULWARK_REACH as f64)
        ),
        T::Magnet => format!(
            "Every charging blade within the taunt's {} tiles turns towards him",
            fig(c::TAUNT_RADIUS as f64)
        ),
        T::Fortress => format!(
            "His armour drains at {} of anybody else's ({} again on the first level's {})",
            fig((c::TANK_DRAIN * c::FORTRESS_DRAIN) as f64),
            by(c::FORTRESS_DRAIN as f64),
            fig(c::TANK_DRAIN as f64)
        ),
        T::RallyingWall => format!(
            "While he taunts, every crew member within {} tiles drains armour at {} too",
            fig(c::RALLYING_WALL_TILES as f64),
            fig(c::RALLYING_WALL_DRAIN as f64)
        ),
    }
}

/// The same for a **fixed** level, which has no talent to look up: what
/// the level's own ability is worth in numbers, said once. `None`
/// wherever [`level_line`] is `None`.
pub fn level_numbers(class: world::Class, level: u8) -> Option<String> {
    use world::class as c;
    Some(match (class, level) {
        (world::Class::Tank, 1) => format!(
            "Armour worn by him drains at {} the ordinary rate, so a piece absorbs twice as much. Bulwark shelters everybody within {} tiles, at {} his own pace",
            by(c::TANK_DRAIN as f64),
            fig(c::BULWARK_REACH as f64),
            by(c::BULWARK_PACE as f64)
        ),
        (world::Class::Tank, 2) => format!(
            "Every piece of armour he wears stops {} what it says it does",
            by(c::PLATED_PROTECTION as f64)
        ),
        (world::Class::Tank, 3) => format!(
            "A taunt reaches {} tiles, runs {} game minutes, and wants {} seconds between",
            fig(c::TAUNT_RADIUS as f64),
            fig(c::TAUNT_MINUTES),
            fig(c::TAUNT_COOLDOWN)
        ),
        (world::Class::Tank, 7) => {
            "Every hit rolled on his head lands on his body instead".to_string()
        }
        _ => return None,
    })
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
pub const PICK_PENDING: &str = "A talent to pick:";
pub const PACK_UP: &str = "Pack up";
/// What a deployable on the deck is called, by `world::DeployKind` code,
/// with what it has left.
pub const DEPLOYABLE_NAMES: [&str; 3] = ["Sandbags", "Sentry", "Healing Sentry"];
pub fn deployable_line(d: &world::Deployable) -> String {
    let name = DEPLOYABLE_NAMES
        .get(d.kind.code() as usize)
        .copied()
        .unwrap_or("Deployable");
    match d.kind {
        world::DeployKind::Sandbags => format!("{name} — {:.0} left", d.health),
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
/// And for a brace refused.
pub fn brace_refused(why: world::Refusal) -> String {
    format!("Cannot brace: {}.", refusal(why))
}
/// And for a beam (feature 76).
pub fn beam_refused(why: world::Refusal) -> String {
    format!("Cannot beam: {}.", refusal(why))
}
/// And for a bulwark and a taunt (feature 77).
pub fn bulwark_refused(why: world::Refusal) -> String {
    format!("Cannot stand as a wall: {}.", refusal(why))
}
pub fn taunt_refused(why: world::Refusal) -> String {
    format!("Cannot taunt: {}.", refusal(why))
}
/// And for a squad order and a rally (feature 78).
pub fn squad_refused(why: world::Refusal) -> String {
    format!("Cannot order the squad: {}.", refusal(why))
}
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
pub const ORDERS_TIP: &str = "The crew nobody is steering keep to your side and fight for themselves when they see an enemy. X puts an attack banner down for them to fight their way to; Y calls them back to the ship; either key again lets them follow you again. A right-click moves your own Bim and nobody else, and F then a click walks it there shooting whatever it meets. Nobody leaves a fight aboard the ship.";
/// The commander's rows on the crew panel (feature 78): what the squad
/// is under, and the rally with its cooldown.
pub const SQUAD_NONE: &str = "Squad: free";
pub const SQUAD_TIP: &str = "The squad is every crew member nobody is steering, your reinforcements among them. H sends it at the enemy under the pointer, T calls it back to a tile, Z has it hold where it stands; the same key again lets it go. Your own Bim is never ordered by it.";
pub fn squad_line(kind: Option<u32>, members: usize) -> String {
    let Some(kind) = kind else {
        return SQUAD_NONE.to_string();
    };
    let what = match kind {
        0 => "attacking",
        1 => "falling back",
        _ => "holding ground",
    };
    format!("Squad {what} — {members}")
}
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
/// The tank's rows on the crew panel (feature 77): the wall, and the
/// taunt with its cooldown.
pub const WALL_UP: &str = "Wall up";
pub const WALL_DOWN: &str = "Wall down";
pub const WALL_TIP: &str = "Standing as a wall: half pace, and a crewmate close behind him is in cover against anything shot through him. E puts it up and down; going down takes it down.";
pub fn taunt_line(left: f64, cooldown: f64, level_enough: bool) -> String {
    if !level_enough {
        return format!("Taunt at level {}", world::class::TAUNT_LEVEL);
    }
    if left > 0.0 {
        return format!("Taunting — {left:.0} min left");
    }
    if cooldown > 0.0 {
        format!("Taunt ready in {cooldown:.0} s")
    } else {
        "Taunt ready".to_string()
    }
}
/// The soldier's rows on the crew panel (feature 75): grenades carried,
/// the throw's cooldown, and the brace.
pub const BRACED: &str = "Braced";
pub const BRACED_TIP: &str = "Holding a line: no errands, steadier shooting. E stands easy; so does any order that moves them.";
pub const STAND_EASY: &str = "Standing easy";
/// The medic's rows on the crew panel (feature 76; task 130): who the
/// beam holds, and whether the Nanite Burst and the Cloak are ready.
pub const BEAM_ON: &str = "Beaming";
pub const BEAM_OFF: &str = "No beam";
pub const BEAM_TIP: &str = "The heal beam puts hit points back into a crewmate, or into the medic itself. E over a crew member — your own Bim included — links it; E again, or on nothing, unlinks. The medic may walk, and fires nothing while it is on until the beam's third rank.";
pub fn beam_line(patients: &[String]) -> String {
    match patients {
        [] => BEAM_OFF.to_string(),
        [one] => format!("{BEAM_ON} {one}"),
        many => format!("{BEAM_ON} {}", many.join(" and ")),
    }
}
pub fn nanite_burst_line(cooldown: f64, learnt: bool) -> String {
    if !learnt {
        return "Nanite Burst not learnt yet".to_string();
    }
    if cooldown > 0.0 {
        format!("Nanite Burst ready in {cooldown:.0} s")
    } else {
        "Nanite Burst ready".to_string()
    }
}
/// The cloak's row: the seconds of one on the medic himself, else when
/// the next may be cast.
pub fn cloak_line(cloaked: f64, cooldown: f64, learnt: bool) -> String {
    if cloaked > 0.0 {
        return format!("Cloaked — {cloaked:.0} s left");
    }
    if !learnt {
        return "Cloak not learnt yet".to_string();
    }
    if cooldown > 0.0 {
        format!("Cloak ready in {cooldown:.0} s")
    } else {
        "Cloak ready".to_string()
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
pub const DROID_NAMES: [&str; 8] = [
    "—",
    "Husk",
    "Trooper",
    "Warden",
    "Guardian",
    // The Machine Heart's (feature 108).
    "Core",
    "Conduit",
    "Fabricator",
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
        // A shot that landed on one of the crew: which part, since the
        // armour on it took the hit first.
        WorldEvent::CrewHit { who: w, part } => {
            format!("{} was hit in the {}.", who(w), body_part_name(part))
        }
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
        // A piece at nothing is still worn and does nothing for the rest of
        // the mission; it is whole again at the next (task 113).
        WorldEvent::PieceBroke { who: w, kind } => {
            let (is, it) = if armour_is_a_pair(kind) {
                ("are", "they stop")
            } else {
                ("is", "it stops")
            };
            format!(
                "{}'s {} {is} broken — {it} nothing until the next mission.",
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
        WorldEvent::Hired { who: w } => {
            format!("{} signed on — a hired hand, paid by the month.", who(w))
        }
        WorldEvent::MercenaryPaid { who: w, fee } => {
            format!(
                "{}'s month came round: {} paid.",
                who(w),
                crate::format::euros(fee)
            )
        }
        WorldEvent::MercenaryLeft { who: w } => format!(
            "{}'s month came round and there was not the money — the hand is owed until it is paid.",
            who(w)
        ),
        WorldEvent::Jumped { star } => format!("Jumped. The ship is in the system of star {star}."),
        WorldEvent::Infested { star } => format!(
            "The machines have this system. Every station round star {star} is theirs: nobody left aboard, nothing to trade, nobody to hire."
        ),
        WorldEvent::Brownout => {
            "Brownout: the batteries are flat and the ship draws more than it makes. The lamps are out and the benches have stopped."
                .into()
        }
        WorldEvent::PowerRestored => "Power restored.".into(),
        WorldEvent::CrewLost => "Nobody of the crew is standing. The run is over.".into(),
        WorldEvent::LevelUp {
            who: w,
            class,
            level,
        } if world::class::ranked(world::Class::from_code(class).unwrap_or_default()) => format!(
            "{} reached level {level} — a skill point to spend: Ctrl and an ability's key.",
            who(w)
        ),
        WorldEvent::LevelUp {
            who: w,
            class,
            level,
        } => match world::class::is_pick_level(
            world::Class::from_code(class).unwrap_or_default(),
            level as u8,
        ) {
            true => format!(
                "{} reached level {level} — a skill point to spend, on the Skills tab.",
                who(w)
            ),
            false => match level_line(
                world::Class::from_code(class).unwrap_or_default(),
                level as u8,
            ) {
                Some(line) => format!("{} reached level {level}: {line}", who(w)),
                None => format!("{} reached level {level}.", who(w)),
            },
        },
        WorldEvent::TalentPicked { who: w, talent } => {
            let talent = world::Talent::from_code(talent);
            format!(
                "{} learnt {}.",
                who(w),
                talent.map(talent_name).unwrap_or("a talent")
            )
        }
        WorldEvent::Deployed { who: w, kind } => format!(
            "{} set up {}.",
            who(w),
            DEPLOYABLE_NAMES
                .get(kind as usize)
                .copied()
                .unwrap_or("something")
                .to_lowercase()
        ),
        WorldEvent::PackedUp { who: w, kind } => format!(
            "{} packed {} up.",
            who(w),
            DEPLOYABLE_NAMES
                .get(kind as usize)
                .copied()
                .unwrap_or("something")
                .to_lowercase()
        ),
        WorldEvent::DeployableLost { kind } => match kind {
            0 => "The sandbags are shot to pieces.".into(),
            2 => "A Healing Sentry is shot to pieces.".into(),
            _ => "A sentry is shot to pieces.".into(),
        },
        WorldEvent::EmpThrown { who: w } => format!("{} threw an EMP.", who(w)),
        WorldEvent::SentryDone { who: w } => format!("{}'s sentry has stood its time.", who(w)),
        WorldEvent::Braced { who: w, on: true } => format!("{} braced.", who(w)),
        WorldEvent::Braced { who: w, on: false } => format!("{} stood easy.", who(w)),
        WorldEvent::Thrown { who: w } => format!("{} threw a grenade.", who(w)),
        WorldEvent::Beamed {
            who: w,
            patient: Some(p),
        } => format!("{} beamed {}.", who(w), who(p)),
        WorldEvent::Beamed { who: w, .. } => format!("{}'s beam is off.", who(w)),
        WorldEvent::NaniteBurst { who: w, healed } => {
            format!("{}'s Nanite Burst healed {healed}.", who(w))
        }
        WorldEvent::Cloaked { who: w, target } if w == target => format!("{} cloaked.", who(w)),
        WorldEvent::Cloaked { who: w, target } => format!("{} cloaked {}.", who(w), who(target)),
        WorldEvent::Bulwarked { who: w, on: true } => format!("{} stood as a wall.", who(w)),
        WorldEvent::Bulwarked { who: w, on: false } => format!("{} stood the wall down.", who(w)),
        WorldEvent::Taunted { who: w } => format!("{} taunted the enemy.", who(w)),
        WorldEvent::Squadded { who: w, kind } => match kind {
            0 => format!("{} sent the squad in.", who(w)),
            1 => format!("{} called the squad back.", who(w)),
            2 => format!("{} had the squad hold its ground.", who(w)),
            _ => format!("{} released the squad.", who(w)),
        },
        WorldEvent::Rallied { who: w } => format!("{} rallied the crew.", who(w)),
        WorldEvent::BattleCried { who: w } => format!("{} called a Battle Cry.", who(w)),
        WorldEvent::Reinforced { who: w, count } => format!(
            "{} brought {count} {} of the Republic.",
            who(w),
            if count == 1 { "soldier" } else { "soldiers" }
        ),
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
            format!("{} will not leave them behind. The ship stays.", player_name(slot))
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
        WorldEvent::Combined { slot, tier, .. } => {
            format!("{} combined two into one of tier {tier}.", player_name(slot))
        }
        WorldEvent::RelicBought { slot, relic, price } => format!(
            "{} holds {} now. The pool paid {}.",
            who(slot),
            world::Relic::from_code(relic).map_or("a relic", relic_name),
            crate::format::euros(price)
        ),
        WorldEvent::Restocked { slot } => format!(
            "{} had the trader restock the shelf — Restock Codes.",
            player_name(slot)
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
            format!("{} took what {} offered.", player_name(to), player_name(from))
        }
        WorldEvent::OfferWithdrawn { from, .. } => {
            format!("{}'s offer is withdrawn.", player_name(from))
        }
        WorldEvent::KeyFound { keys } => format!(
            "A research key picked up — the crew hold {keys}."
        ),
        WorldEvent::BotLost { who: w } => format!("{} is gone for good.", who(w)),
        WorldEvent::TownFell { .. } => TOWN_FELL.into(),
        WorldEvent::PlayerGone { slot } => format!("{} has left the game.", player_name(slot)),
        WorldEvent::Readied { slot, yes: true } => format!("{} is ready.", player_name(slot)),
        WorldEvent::Readied { slot, yes: false } => {
            format!("{} is not ready after all.", player_name(slot))
        }
        WorldEvent::AllReady => "Everybody is ready. The mission is under way.".into(),
        WorldEvent::RelicsOffered { source: 0, count } => {
            format!("The site is cleared: {count} relics on offer. Choose one together, or none.")
        }
        WorldEvent::RelicsOffered { .. } => {
            "The cache holds a relic. Choose who takes it — kept if the site is cleared.".into()
        }
        WorldEvent::RelicProposed { slot, relic, to } => match relic_of(relic) {
            Some(r) => format!(
                "{} puts {} for {}.",
                player_name(slot),
                relic_name(r),
                player_name(to)
            ),
            None => format!("{} would take no relic.", player_name(slot)),
        },
        WorldEvent::RelicAccepted { slot, yes: true } => {
            format!("{} says yes to the relic.", player_name(slot))
        }
        WorldEvent::RelicAccepted { slot, yes: false } => {
            format!("{} takes their yes back.", player_name(slot))
        }
        WorldEvent::RelicGiven { slot, relic } => format!(
            "{} has {} for the rest of the run.",
            player_name(slot),
            relic_of(relic).map_or("a relic", relic_name)
        ),
        WorldEvent::RelicPending { slot, relic } => format!(
            "{} is {}'s once the site is cleared.",
            relic_of(relic).map_or("The relic", relic_name),
            player_name(slot)
        ),
        WorldEvent::RelicsDeclined => "The crew take no relic.".into(),
        WorldEvent::RelicLost { slot, relic } => format!(
            "{} is lost — the site was left uncleared, and {} never had it.",
            relic_of(relic).map_or("The relic", relic_name),
            player_name(slot)
        ),
        WorldEvent::CacheOpened { who: w } => format!("{} opened the relic cache.", who(w)),
        WorldEvent::RelicFired { who: w, relic } => match relic_of(relic) {
            Some(world::Relic::SecondWind) => format!("{} gets up again — Second Wind.", who(w)),
            Some(world::Relic::PhaseHarness) => {
                format!("{} phases out — nothing hurts for a moment.", who(w))
            }
            Some(world::Relic::SignalScrambler) => {
                format!("{} drops off the machines' sights — Signal Scrambler.", who(w))
            }
            Some(world::Relic::Lifeline) => format!(
                "{} throws a Lifeline — nothing hurts either of them for a moment.",
                who(w)
            ),
            Some(world::Relic::RallyPoint) => {
                format!("{} calls a Rally Point — the fallen get up.", who(w))
            }
            Some(world::Relic::HazardPay) => format!(
                "Hazard Pay: the crew are paid {} for the site.",
                crate::format::euros(world::data::HAZARD_PAY)
            ),
            // Fired every kill or every ability: the log would be nothing
            // else in a fight.
            Some(
                world::Relic::SquadMorale
                | world::Relic::SprintCoil
                | world::Relic::TetherField,
            ) => String::new(),
            Some(r) => format!("{}: {}.", who(w), relic_name(r)),
            None => String::new(),
        },
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
pub const RELIC_NAMES: [&str; 37] = [
    "Focusing Lens",
    "Servo Braces",
    "Field Plating",
    "Coolant Loop",
    "Steady Grip",
    "Trauma Kit",
    "Second Wind",
    "Salvage Beacon",
    "Overcharge Cell",
    "Last Stand",
    "Kill Relay",
    "Phase Harness",
    // Task 118.
    "Marksman's Habit",
    "Servo Cutter",
    "Crippler's Mark",
    "Pressure Seal",
    "Quick Wrap",
    "Clot Booster",
    "Blind Spot",
    "Sprint Coil",
    "Signal Scrambler",
    "Field Radio",
    "Spotter",
    "Squad Morale",
    "Hazard Pay",
    "Trade License",
    "Restock Codes",
    "Parts Broker",
    "Tether Field",
    "Wide Angle Optics",
    "Cover Formation",
    "Scrap Collector",
    "Total Teardown",
    "Lifeline",
    "Crossfire",
    "Rally Point",
    "War Chest",
];

pub fn relic_name(relic: world::Relic) -> &'static str {
    RELIC_NAMES
        .get(relic.code() as usize)
        .copied()
        .unwrap_or("a relic")
}

/// What a relic does, in a line, off the rules' own numbers.
pub fn relic_line(relic: world::Relic) -> String {
    use world::Relic::*;
    use world::data as d;
    match relic {
        FocusingLens => format!("+{}% weapon damage.", d::FOCUSING_LENS_DAMAGE_PERCENT),
        ServoBraces => format!("+{}% move speed.", d::SERVO_BRACES_SPEED_PERCENT),
        FieldPlating => format!("+{}% armour.", d::FIELD_PLATING_ARMOUR_PERCENT),
        CoolantLoop => format!(
            "-{}% class ability cooldowns.",
            d::COOLANT_LOOP_COOLDOWN_PERCENT
        ),
        SteadyGrip => format!("+{}% accuracy.", d::STEADY_GRIP_ACCURACY_PERCENT),
        TraumaKit => format!(
            "Revives a downed crewmate {} s faster — {} s where it took {}, a medic's {} s where it took {} — and never in under {} s.",
            fig(d::TRAUMA_KIT_REVIVE_SECONDS as f64),
            fig(world::class::revive_time(false, d::TRAUMA_KIT_REVIVE_SECONDS) as f64),
            fig(bims::health::REVIVE_SECONDS as f64),
            fig(world::class::revive_time(true, d::TRAUMA_KIT_REVIVE_SECONDS) as f64),
            fig(world::class::MEDIC_REVIVE_SECONDS as f64),
            fig(d::REVIVE_FLOOR_SECONDS as f64)
        ),
        SecondWind => format!(
            "The first time this Bim goes down in a mission, it gets up after {} s with {}% health.",
            d::SECOND_WIND_SECONDS,
            d::SECOND_WIND_HEALTH_PERCENT
        ),
        SalvageBeacon => format!(
            "+{}% bounty for this Bim's kills, paid when the site is cleared.",
            d::SALVAGE_BEACON_BOUNTY_PERCENT
        ),
        OverchargeCell => format!(
            "Every {}th shot deals {}.",
            d::OVERCHARGE_CELL_EVERY,
            if d::OVERCHARGE_CELL_DAMAGE_PERCENT == 100 {
                "double damage".to_string()
            } else {
                format!("+{}% damage", d::OVERCHARGE_CELL_DAMAGE_PERCENT)
            }
        ),
        LastStand => format!(
            "+{}% weapon damage while another player's Bim is down.",
            d::LAST_STAND_DAMAGE_PERCENT
        ),
        KillRelay => format!(
            "Each kill takes {} s off this Bim's class ability cooldowns.",
            d::KILL_RELAY_SECONDS
        ),
        PhaseHarness => format!(
            "Once a mission, when a hit takes this Bim under {}% health, it takes no damage for {} s.",
            d::PHASE_HARNESS_BELOW_PERCENT,
            d::PHASE_HARNESS_SECONDS
        ),
        // Task 118: Dismantler.
        MarksmansHabit => "This Bim's first hit on each machine lands on its arms or legs.".into(),
        ServoCutter => format!(
            "+{}% damage to a machine's arms and legs.",
            d::SERVO_CUTTER_DAMAGE_PERCENT
        ),
        CripplersMark => format!(
            "+{}% damage to machines missing their arms or legs.",
            d::CRIPPLERS_MARK_DAMAGE_PERCENT
        ),
        PartsBroker => format!(
            "+{}% bounty for machines this Bim destroys while they are missing a limb.",
            d::PARTS_BROKER_BOUNTY_PERCENT
        ),
        TotalTeardown => format!(
            "A hit on a limb already destroyed tears into the chassis for {}.",
            percent_more(d::TOTAL_TEARDOWN_DAMAGE_PERCENT)
        ),
        // Lifeline.
        PressureSeal => format!(
            "This Bim regenerates {} HP a second.",
            d::PRESSURE_SEAL_HP_PER_SECOND
        ),
        QuickWrap => format!(
            "Every crewmate this Bim revives gets {} HP on top.",
            d::QUICK_WRAP_HEAL
        ),
        ClotBooster => format!(
            "For the first {} s after this Bim goes down, it heals {} HP a second whenever it is back on its feet — a downed body is healed by nothing but a revive.",
            d::CLOT_BOOSTER_SECONDS,
            d::CLOT_BOOSTER_HP_PER_SECOND
        ),
        TetherField => format!(
            "A crewmate this Bim revives takes {}% less of every hit for {} s.",
            d::TETHER_FIELD_PERCENT,
            d::TETHER_FIELD_SECONDS
        ),
        Lifeline => format!(
            "Once a mission, when a crewmate within {} tiles goes down, both of them are untouchable for {} s.",
            d::LIFELINE_TILES,
            d::LIFELINE_SECONDS
        ),
        // Flanker.
        BlindSpot => format!(
            "+{}% damage on hits that strike a machine from the side or behind.",
            d::BLIND_SPOT_DAMAGE_PERCENT
        ),
        SprintCoil => format!(
            "+{}% move speed for {} s at a mission's start and after each ability used.",
            d::SPRINT_COIL_SPEED_PERCENT,
            d::SPRINT_COIL_SECONDS
        ),
        SignalScrambler => format!(
            "After this Bim destroys a machine from the side or behind, no machine aims at it for {} s. Once every {} s.",
            d::SIGNAL_SCRAMBLER_SECONDS,
            d::SIGNAL_SCRAMBLER_COOLDOWN
        ),
        WideAngleOptics => {
            "This Bim's side-or-behind zone is 30° wider on each side — and so is a Guardian's shield narrower against its shots."
                .into()
        }
        Crossfire => format!(
            "While this Bim and a crewmate stand on opposite sides of a machine, both deal +{}% damage to it.",
            d::CROSSFIRE_DAMAGE_PERCENT
        ),
        // Command Net.
        FieldRadio => format!(
            "Crewmates within {} tiles aim {}% better.",
            d::FIELD_RADIO_TILES,
            d::FIELD_RADIO_ACCURACY_PERCENT
        ),
        Spotter => format!(
            "The machine this Bim hit last takes +{}% damage from every crewmate for {} s.",
            d::SPOTTER_DAMAGE_PERCENT,
            d::SPOTTER_SECONDS
        ),
        SquadMorale => format!(
            "Each machine this Bim or a bot destroys takes {} s off this Bim's class ability cooldowns.",
            d::SQUAD_MORALE_SECONDS
        ),
        CoverFormation => format!(
            "Bots within {} tiles take {}% less damage.",
            d::COVER_FORMATION_TILES,
            d::COVER_FORMATION_PERCENT
        ),
        RallyPoint => format!(
            "Once a mission, using an ability gets every downed crewmate within {} tiles back up at {}% health.",
            d::RALLY_POINT_TILES,
            d::RALLY_POINT_HEALTH_PERCENT
        ),
        // Supply Line.
        HazardPay => format!(
            "The crew are paid {} each time a site is cleared.",
            crate::format::euros(d::HAZARD_PAY)
        ),
        TradeLicense => format!(
            "Trader prices are {}% lower for the crew.",
            d::TRADE_LICENSE_PERCENT
        ),
        RestockCodes => {
            "Once a trader visit, the trader's weapons and armour can be rolled again. The relic is not."
                .into()
        }
        ScrapCollector => format!(
            "Each machine this Bim destroys earns the crew {}, paid when the site is cleared.",
            crate::format::euros(d::SCRAP_COLLECTOR_PAY)
        ),
        WarChest => format!(
            "+{}% damage for every {} in the crew's pool a player, up to +{}%.",
            d::WAR_CHEST_PERCENT_PER_THOUSAND,
            crate::format::euros(1_000),
            d::WAR_CHEST_CAP_PERCENT
        ),
    }
}

/// "double damage" for a hundred per cent, else "+n% damage".
fn percent_more(percent: i32) -> String {
    if percent == 100 {
        "double damage".to_string()
    } else {
        format!("+{percent}% damage")
    }
}

pub const RELICS_HEADING: &str = "Relics";
pub const NO_RELICS: &str = "None yet. A site cleared of machines offers relics.";
pub const REWARD_TITLE: &str = "The site is cleared";
pub const CACHE_TITLE: &str = "A relic cache";
pub const REWARD_INTRO: &str = "Choose a relic and whose Bim takes it. Every player has to say yes; a new proposal clears them.";
pub const CACHE_INTRO: &str =
    "One relic out of the cache. It is kept only if the site is cleared before the crew leave.";
pub const TAKE_NONE: &str = "Take none";
pub const ACCEPT: &str = "Accept";
pub const FOR_BIM: &str = "For";
/// The end of a fight (`screens::fightwon`): up in the mission the moment
/// the site is cleared, with what the fight earned.
pub const FIGHT_WON_TITLE: &str = "Fight won";
pub const FIGHT_WON_CLEARED: &str =
    "The last of the enemy here is down. The site stays cleared, and the bounty is in the pool.";
pub const FIGHT_WON_HELD: &str =
    "The site is held against the machines, and the bounty is in the pool.";
pub const FIGHT_WON_MACHINES: &str = "Machines destroyed";
pub const FIGHT_WON_PEOPLE: &str = "Manufacturers down";
pub const FIGHT_WON_BOUNTY: &str = "Bounty paid";
pub const FIGHT_WON_POOL: &str = "The pool";
pub const FIGHT_WON_BOTS: &str = "The rest of the crew";
pub const FIGHT_WON_RELIC: &str = "Relic kept";
pub const FIGHT_WON_JOINED: &str = "Townsfolk joined";
pub const FIGHT_WON_LOST: &str = "Crew lost";
pub const FIGHT_WON_NEXT: &str = "Back aboard, the crew choose a relic of the site's tier together, then where to go next on the map. Stay, and everything stands still — nobody moves and nobody downed bleeds out — until the button at the bottom right takes everybody back, wherever they are.";
pub const FIGHT_WON_STAY: &str = "Stay here";

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
pub const VICTORY_UNLOCKED: &str = "Unlocked for your next runs:";
pub const VICTORY_NOTHING_NEW: &str = "Every relic is unlocked already.";

/// The victory screen's summary of the run (feature 108), a line a number,
/// and each player's Bim's relics under it.
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
pub const VICTORY_RELICS: &str = "Relics held:";
pub fn victory_relics_of(who: u32, relics: &[world::Relic]) -> String {
    let names: Vec<&str> = relics.iter().map(|&r| relic_name(r)).collect();
    let held = if names.is_empty() {
        "none".to_string()
    } else {
        names.join(", ")
    };
    format!("{}: {held}", crew_name(who))
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

pub fn relic_proposal_line(relic: Option<world::Relic>, to: u32) -> String {
    match relic {
        Some(r) => format!("On the table: {} for {}.", relic_name(r), player_name(to)),
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
pub const MAP_TIP: &str = "A trip is one step: to another station or settlement in this system, or to one in a system a hyperlane joins to this one. Nothing is flown. The world clock goes on by the trip's length the moment everybody has accepted — the crisis spreads by the day — and the crew arrive docked or landed with a mission begun. The world clock moves for nothing else: not during a mission, and not here.";
/// The two halves of the list.
pub const MAP_THIS_SYSTEM: &str = "This system";
pub fn map_next_system(star: &str) -> String {
    format!("{star} · one hop")
}
/// The list's order (task 135): by system, or every site by how long the
/// trip to it is, the nearest first.
pub const MAP_SORT_SYSTEM: &str = "By system";
pub const MAP_SORT_DISTANCE: &str = "By distance";
pub const MAP_SORT_TIP: &str = "By system lists this system's sites, then each system a hyperlane joins, under its name. By distance lists every site together, the shortest trip first, with its system beside it; the sites no trip can go to come last.";
/// The site the crew are at, in the list.
pub const MAP_HERE: &str = "here";
/// Beside a trip that is the least a trip may be (`world::data::MIN_TRAVEL_HOURS`,
/// feature 105) rather than its flown length.
pub const TRIP_MINIMUM: &str = "the minimum";
/// A trip's length and the day it ends on — and, where the trip is the
/// least a trip may be, that it is.
pub fn trip_quote(minutes: u64, minimum: bool, arrival_day: u32) -> String {
    let length = crate::format::trip_length(minutes);
    if minimum {
        format!("{length} ({TRIP_MINIMUM}) · day {arrival_day}")
    } else {
        format!("{length} · day {arrival_day}")
    }
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
/// What a site kind means, for the `?` beside the map's list.
pub const SITE_KIND_TIP: &str = "Every site is one of three. ATTACK: the machines, the Manufacturers or the Machine Heart hold it — go in and clear it. DEFEND: the machines are coming for it — twenty seconds after you arrive the first wave lands, and its own people and armed defenders fight beside you; hold the last wave and it is cleared (no money: its people are the reward), leave before and it falls. TRADER: buy gear and relics on the map; the machines never come for one. A system has as many sites to attack as to defend; in one the machines have taken, every site is an attack.";
/// A defence held, on the map: its fight is over.
pub const SITE_HELD: &str = "held";
/// Under a site on the system map whose system's other fight the crew
/// fought (task 135): it is refused for the rest of the run.
pub const SITE_PASSED: &str = "not chosen";
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
pub fn out_line() -> String {
    format!(
        "Your Bim is dead. It is bought back at the next mission for {} if the pool can pay.",
        crate::format::euros(world::data::BUYBACK_COST)
    )
}
pub const BACK_TO_SHIP: &str = "Back to ship";
/// The same button once the fight is won and the deck frozen (task 133).
pub const FIGHT_WON_BACK_TO_SHIP: &str = "Fight won — Back to ship";
pub const BACK_TO_SHIP_TIP: &str = "Say you are done here. The first press sends every bot back to the ship, and every press walks your own Bim there too. The ship leaves once every player still on their feet has pressed it and is aboard: anybody outside then is left behind, and dead for it, if everybody agrees. Leave before the place is cleared and it is put back as you found it — the bounty is lost, the experience is kept. Once the fight is won everything stands still, nobody bleeds out, and nobody has to walk: the ship leaves when every player has pressed it and takes everybody alive, the downed too.";
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
    format!("{} — ready?", site_kind_word(kind))
}
pub const READY_LINE: &str =
    "Nothing moves until every player is ready. Change your loadout now if you want to.";
pub const READY_YES: &str = "Ready";
pub const READY_NO: &str = "Not ready";
pub fn ready_answer(who: &str, ready: bool, gone: bool) -> String {
    match (gone, ready) {
        (true, _) => format!("{who}: gone"),
        (false, true) => format!("{who}: ready"),
        (false, false) => format!("{who}: …"),
    }
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
pub const TRADER_TIP: &str = "A trader is visited on the map: no mission, no room, and neither clock moves while the crew are there. Its shelf is rolled once for the run and never restocked, and its relic is drawn the first time the crew arrive. It is closed while the machines have its system, until every site of the system they took is cleared.";
pub const TRADER_CLOSED: &str = "closed";
pub const TRADER_CLOSED_ON_ARRIVAL: &str = "closed on arrival";
pub const TRADER_TITLE: &str = "Trader";
pub const TRADER_INTRO: &str = "Buy off the shelf out of the pool — onto your own Bim, a bot, or into the armory. What it replaces goes into the armory. Nothing comes back once it is sold.";
pub const TRADER_WEAPONS: &str = "Weapons";
pub const TRADER_ARMOUR: &str = "Armour";
pub const TRADER_SOLD: &str = "sold";
pub const TRADER_BUY: &str = "Buy";
pub const TRADER_INTO_ARMORY: &str = "Into the armory";
pub const TRADER_RELIC: &str = "Relic";
pub const TRADER_NO_RELIC: &str = "The relic here is sold.";
pub const TRADER_RELIC_INTRO: &str = "Bought together: propose it for a player's Bim and every player has to say yes. A new proposal clears them. The pool pays when it carries.";
pub const TRADER_PROPOSE: &str = "Propose";
pub const TRADER_WITHDRAW: &str = "Withdraw";
pub const TRADER_COMBINE: &str = "Combine";
pub const TRADER_COMBINE_INTRO: &str = "Two weapons or two pieces of one kind at one tier make one of the next tier, whole. Out of the armory, off your own Bim or off a bot. Where one of the two is worn, the result is worn in its place. Tier three is as far as it goes.";
pub const TRADER_COMBINE_NONE: &str =
    "Nothing to combine: no two of one kind at one tier below three.";
pub const TRADER_ARMORY_HINT: &str = "Tab opens the Armory beside this.";
/// *Restock Codes* (task 118): the button, and what it does.
pub const TRADER_RESTOCK: &str = "Restock the shelf";
pub const TRADER_RESTOCK_TIP: &str =
    "Restock Codes: roll the trader's weapons and armour again, once a visit. The relic stays.";
/// A thing on the shelf: its name and tier.
pub fn shelf_line(name: &str, tier: u32) -> String {
    format!("{name} · tier {tier}")
}
/// A pair that combines: what it is and where the two are.
pub fn combine_line(name: &str, tier: u32, from: &str) -> String {
    format!("Two {name} · tier {tier} into tier {} · {from}", tier + 1)
}
/// Where a thing to combine is.
pub fn combine_from(worn_by: Option<&str>) -> String {
    match worn_by {
        Some(who) => format!("worn by {who}"),
        None => "armory".into(),
    }
}

/// The Manufacturers (feature 109): the name over one of theirs on the
/// deck, the log's line when one dies, and a site of theirs' tag in the
/// map's list.
pub const MANUFACTURER_NAME: &str = "Manufacturer";
pub const MANUFACTURER_DOWN: &str = "A Manufacturer is dead.";
pub const ARRIVE_MANUFACTURERS: &str = "Manufacturers";
/// An elite (`world::elite`), in a row of the map's list and under its
/// site on the system map, and the galaxy chart's line on its star.
pub const ARRIVE_ELITE: &str = "elite · Guardian in wave 2 · relics";
pub const SITE_ELITE: &str = "ELITE";
pub const CHART_ELITE: &str = "An elite in this system: a Guardian in its second wave, and relics.";
pub fn heart_preview_rows(p: &world::heart::HeartPreview) -> [(&'static str, String); 4] {
    [
        ("Conduits", p.conduits.to_string()),
        (
            "Core",
            crate::format::grouped(p.core_health.max(0.0).ceil() as u64),
        ),
        ("Wave size", p.wave_size.to_string()),
        ("Waves", p.wave_count.to_string()),
    ]
}
/// What the galaxy chart says under the machines' origin, once the crew
/// have seen it.
pub const HEART_CHART_LINE: &str = "Where the machines began: the Machine Heart's fortress";
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
        words.push_str(" · heading back to the ship");
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
pub const NO_CLASS: &str = "No class";
/// The hero panel greyed over while its Bim is down.
pub const DOWNED_BANNER: &str = "Downed";
/// The tag beside the hero's health while it is critically hit
/// (feature 110), and what it means.
pub const CRITICAL_TAG: &str = "CRITICAL";
pub const CRITICAL_TIP: &str = "Badly hurt: under twenty hit points and bleeding \
    on the deck, or down with the countdown running. Get a medic's beam on it, \
    or get out of the fight.";
/// The `+1` on the hero panel.
pub const TALENT_WAITING_TIP: &str =
    "A talent to pick — open the character sheet to spend the point.";
/// What a downed body has left, in one line — the hero panel's and the
/// portraits' (task 120).
pub fn downed_short(seconds: f32) -> String {
    format!("DOWNED — dies in {} s", seconds.ceil().max(0.0) as u32)
}
/// A Bim that was downed this mission and walks slower for the rest of
/// it, in one line.
pub fn slowed_short() -> String {
    format!(
        "Slowed · {}% for the mission",
        ((1.0 - bims::health::DOWNED_PACE) * 100.0).round() as u32
    )
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
pub const ARMORY_EMPTY_SLOT: &str = "—";
pub const ARMORY_TAKE_OFF: &str = "Take off, into the armory";
pub const ARMORY_OFFER_TO: &str = "Offer to";
pub const ARMORY_PUT_ON: &str = "Put on";
pub const ARMORY_OFFERS: &str = "offers:";
pub const ARMORY_ACCEPT: &str = "Accept";
pub const ARMORY_DECLINE: &str = "Decline";
pub const ARMORY_OFFERED_TO: &str = "offered to";
pub const ARMORY_TAKE_BACK: &str = "Take back";
pub const ARMORY_STOCK: &str = "The armory";
pub const ARMORY_NOTHING: &str = "Nothing in the armory.";
pub const ARMORY_KEY: &str = "research key";
pub const ARMORY_KEYS: &str = "research keys";
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
pub const SHEET_TALENTS: &str = "Talents";
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

/// The Esc sheet's view toggle, which the View tab was.
pub const VIEW_HEADING: &str = "View";
pub const VIEW_PLAIN: &str = "Plain";
pub const VIEW_PLAIN_HINT: &str = "The ship as it is.";
pub const VIEW_POWER: &str = "Electricity";
pub const VIEW_POWER_HINT: &str =
    "The power cables, and everything that makes, holds or draws power.";

/// The world map's column.
pub const MAP_CLOSE: &str = "Close";
pub const MAP_DAY: &str = "Day";
pub const MAP_POOL: &str = "Pool";
pub const MAP_PICK_HINT: &str =
    "Pick a place on the list or on the chart: the trip is quoted here, and put to the crew.";

/// The galaxy chart's tag under a star: the least and the most tier its
/// sites' enemies come at (`World::system_tiers`) — one number where the
/// rule is sure, a range on the distance ramp. The world map's rows say
/// "tier 1", so the chart says it the same way.
pub fn system_tier(low: bims::combat::Tier, high: bims::combat::Tier) -> String {
    format!("T{}", tier_span(low, high))
}
fn tier_span(low: bims::combat::Tier, high: bims::combat::Tier) -> String {
    if low == high {
        low.code().to_string()
    } else {
        format!("{}–{}", low.code(), high.code())
    }
}
/// The same, as the star panel's line.
pub fn system_tier_line(low: bims::combat::Tier, high: bims::combat::Tier) -> String {
    format!("Tier {}", tier_span(low, high))
}
pub const SYSTEM_TIER_TIP: &str = "What tier the machines and the Manufacturers come at in this system on the day it is quoted: tier 3 within two hyperlanes of where the machines began, tier 2 further out once a fortnight has gone by — sure six lanes from home, a roll a site nearer — and tier 1 everywhere else. On the chart a star at tier 2 is ringed in amber and one at tier 3 in red; zoomed in, every star has its tier written under it.";
/// The star panel on the galaxy chart: a system with a trader.
pub const CHART_TRADER: &str = "Trader in this system";
pub const CHART_TRADER_CLOSED: &str = "Trader in this system · closed";
/// How the crew get to a star picked on the chart.
pub const CHART_HERE: &str = "The crew are here.";
pub const CHART_ONE_LANE: &str = "One hyperlane away: its places are on the list.";
pub fn chart_lanes_away(hops: usize) -> String {
    format!("{hops} hyperlanes away — a trip crosses one lane at a time.")
}
pub const BUYBACK_HEADING: &str = "Buyback";
pub const BUYBACK_COVERED: &str = "covered";
pub const BUYBACK_SHORT: &str = "not covered";
/// The destination card's rows.
pub const CARD_HOPS: &str = "Where";
pub const CARD_TRAVEL: &str = "Travel";
pub const CARD_ARRIVAL: &str = "Arrive on day";
pub fn hops_words(jump: bool) -> String {
    if jump {
        "one hop away".into()
    } else {
        "this system".into()
    }
}
pub fn days_words(days: f64, minimum: bool) -> String {
    if minimum {
        format!("{days:.1} days ({TRIP_MINIMUM})")
    } else {
        format!("{days:.1} days")
    }
}
pub fn proposed_by(who: &str) -> String {
    format!("Proposed by {who}")
}
/// What leaving a player's Bim behind costs: it is out until bought back.
pub fn buyback_cost(cost: u64) -> String {
    format!("buyback {}", crate::format::euros(cost))
}

/// The parts of a body a shot can land on, indexed by
/// `bims::health::Part::code`: the head, the body, the legs. Lower case,
/// because every line that names one runs it into a sentence.
pub const BODY_PART_NAMES: [&str; 3] = ["head", "body", "leg"];

pub fn body_part_name(code: u32) -> &'static str {
    BODY_PART_NAMES
        .get(code as usize)
        .copied()
        .unwrap_or("body")
}

/// What each kind of body is called. Indexed by `worldgen::BodyKind`.
pub const BODY_KIND_NAMES: [&str; 4] = ["Rocky planet", "Gas giant", "Ice world", "Asteroid belt"];

/// And each kind of station, by `worldgen::StationKind`.
pub const STATION_KIND_NAMES: [&str; 5] =
    ["Orbital", "Refinery", "Mining outpost", "Derelict", "Relay"];

/// `worldgen::StarClass`, hottest first.
pub const STAR_CLASS_NAMES: [&str; 7] = ["O", "B", "A", "F", "G", "K", "M"];

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

pub const HEALTH_TIP: &str = "Hit points: one bar for the whole Bim. A hit comes off the armour worn where it lands first — its protection off the damage before anything else, the rest draining the piece — and only what the piece cannot take reaches the bar. The blue on the end is that armour. Under twenty the Bim bleeds on the deck; at nothing it is down: it lies where it fell, can do nothing and is shot at by nothing, and dies thirty seconds later unless a crewmate standing beside it brings it round — ten seconds with hands on, a medic's four. It gets up at three tenths of its bar and walks thirty per cent slower for the rest of the mission. A piece at nothing is broken for the rest of the mission and whole again at the next.";

// --- down, and the revive (task 120) -----------------------------------------
//
// The block under the health bar: the one framed thing on the panel, for
// a body that is down with the countdown running, and a line under it
// for one that was down this mission and walks slower for it. The words
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
pub const DOWNED_TIP: &str = "At nothing a Bim goes down: it lies where it fell, can do nothing, and nothing shoots at it. Unless a crewmate revives it by standing beside it it dies when the countdown runs out. Revived, it gets up at three tenths of its bar and walks thirty per cent slower for the rest of the mission.";
/// The line under the bar of a Bim that was downed this mission.
pub fn slowed_note() -> String {
    format!(
        "Was down this mission: walks {}% slower until it ends.",
        ((1.0 - bims::health::DOWNED_PACE) * 100.0).round() as u32
    )
}
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
pub const CARRY_MEDICS_ONLY: &str = "only a medic or a hired field medic can carry";
pub const CARRY_ARMS_FULL: &str = "your arms are full — set them down first";
pub const CARRY_TAKEN: &str = "somebody is already carrying them";
/// What the log says when the revive key is held with nobody down close
/// enough to get up.
pub const REVIVE_NOBODY_NEAR: &str = "Nobody down close enough to get up.";
/// The quickselect in the hero panel (task 138), each after its key.
pub const HAND_WEAPON: &str = "Weapon";
pub const HAND_MEDKIT: &str = "Medkit";
pub const HAND_WEAPON_TIP: &str =
    "The weapon in hand: your Bim fires as it always does. An attack order takes it up by itself.";
pub const HAND_MEDKIT_TIP: &str = "The medkit in hand: your Bim holds its fire, and a right-click on a downed crewmate walks over and revives them — their countdown stands while your hands are on them.";
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
/// kinds made only from a tier up (task 115).
pub const WEAPON_NAMES: [&str; 11] = [
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
];

/// What a piece of armour is called, indexed by `bims::combat::ArmourKind::code`;
/// `0` is an empty slot. Four and five are the two pieces made only from a
/// tier up (task 116).
pub const ARMOUR_NAMES: [&str; 6] = [
    "—",
    "Basic helm",
    "Basic kevlar",
    "Basic leg guards",
    "Arc greaves",
    "Reflective plate",
];

/// What a piece does beyond its health and protection (task 116), put on
/// the line of its numbers: the Reflective plate's bolts sent back and a
/// pair of arc greaves' discharge at the piece's tier, off the balance's
/// own constants. Nothing for the three basic pieces.
pub fn armour_effect(piece: &bims::combat::Piece) -> String {
    use bims::balance::{ARC_COOLDOWN, ARC_DAMAGE, ARC_RADIUS, REFLECT_DAMAGE, REFLECT_ODDS};
    use bims::combat::ArmourKind;
    match piece.kind {
        ArmourKind::ReflectivePlate => format!(
            ", sends {}% of the bolts on the body back at {}% damage",
            (REFLECT_ODDS * 100.0).round(),
            (REFLECT_DAMAGE * 100.0).round()
        ),
        ArmourKind::ArcGreaves => format!(
            ", a blow on the wearer arcs {} into every enemy within {} tiles, once a {} s",
            tidy(ARC_DAMAGE * piece.tier.armour_factor()),
            tidy(ARC_RADIUS),
            tidy(ARC_COOLDOWN)
        ),
        ArmourKind::BasicHelm | ArmourKind::BasicKevlar | ArmourKind::BasicLegs => String::new(),
    }
}

/// A number to the hundredth, the trailing noughts dropped — a piece's
/// protection, which a tier's factor leaves at 1.35 or 2.7.
pub fn tidy_hundredths(x: f32) -> String {
    let text = format!("{:.2}", (x * 100.0).round() / 100.0);
    text.trim_end_matches('0').trim_end_matches('.').to_string()
}

/// Whether a piece's name is a pair — the leg guards, the greaves — and
/// is said "are" rather than "is".
pub fn armour_is_a_pair(kind: bims::combat::ArmourKind) -> bool {
    kind.slot() == bims::health::Part::Legs
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

/// The three armour slots, top to bottom, and the weapon's.
pub const SLOT_NAMES: [&str; 4] = ["Head", "Body", "Legs", "Weapon"];

/// A weapon's numbers as the Inventory says them, off the two-point
/// curves in `bims::combat::WeaponStats`: each number is its best out to
/// the sweet distance and falls in a straight line to the far one at the
/// range. "90% to 4 tiles, 60% at 10" is the shape; a curve with no sweet
/// distance starts "up close", and a flat one is just the number, since
/// "12 to 0 tiles, 12 at 12" would be three numbers for one.
fn curve_text(stats: &WeaponStats, near: String, far: String) -> String {
    if near == far {
        near
    } else if stats.sweet <= 0.0 {
        format!("{near} up close, {far} at {} tiles", tidy(stats.range))
    } else {
        format!(
            "{near} to {} tiles, {far} at {}",
            tidy(stats.sweet),
            tidy(stats.range)
        )
    }
}

/// A stat to a tenth, without a float's noise: a tier-three pistol's range
/// is "26.4 tiles", not "26.400002", and a whole number stays whole.
pub fn tidy(x: f32) -> String {
    let tenths = (x * 10.0).round() / 10.0;
    if tenths.fract() == 0.0 {
        format!("{}", tenths as i64)
    } else {
        format!("{tenths:.1}")
    }
}

/// "100 to 4 tiles, 60 at 10"; "12 a shot" for a flat curve, "90 a
/// swing" for a blade.
pub fn damage_text(stats: &WeaponStats) -> String {
    let flat = stats.damage == stats.damage_far;
    match (stats.melee, flat) {
        (true, _) => format!("{} a swing", tidy(stats.damage)),
        (false, true) => format!("{} a shot", tidy(stats.damage)),
        (false, false) => curve_text(stats, tidy(stats.damage), tidy(stats.damage_far)),
    }
}

/// How often it goes off. A burst weapon's is "8 in 2 s, then 2 s": the
/// burst counted at a gap a shot, and the rest of the trigger's period
/// after it as the recharge; a single-shot gun's "1.5 a second"; a
/// blade's "a swing every 2 s".
pub fn fire_rate_text(stats: &WeaponStats) -> String {
    let period = 1.0 / stats.fire_rate.max(1e-3);
    if stats.melee {
        format!("a swing every {} s", period)
    } else if stats.burst > 1 {
        let burst = stats.burst as f32 * stats.burst_gap;
        format!(
            "{} in {} s, then {} s",
            stats.burst,
            burst,
            (period - burst).max(0.0)
        )
    } else {
        format!("{} a second", stats.fire_rate)
    }
}

/// What a blade is, in one line: "Melee — 70 a swing every 2 s".
pub fn melee_text(stats: &WeaponStats) -> String {
    format!("Melee — {} {}", tidy(stats.damage), fire_rate_text(stats))
}

/// The Inventory's status for a Bim locked in a melee — a blade within
/// reach — and the header's: no firing until one of them is out of reach.
pub const LOCKED_STATUS: &str = "locked in melee";

/// Why, with the room's numbers rather than a copy of them.
pub fn locked_tip() -> String {
    use bims::combat::{FIST_DAMAGE, MELEE_PERIOD, WeaponKind};
    format!(
        "A blade within reach: the gun stays quiet and it fights with its fists, {} every {} s — a schword in its own hand cuts for {} instead. It is free again when one of them steps out of reach.",
        FIST_DAMAGE,
        MELEE_PERIOD,
        WeaponKind::Schword.stats().damage
    )
}

/// What each thing in a grid cell is, for the tooltip under its icon,
/// indexed by `physics::ResourceId`. A piece of armour's numbers are put
/// after its line by the grid, off the piece itself.
pub const ITEM_TIPS: [&str; 22] = [
    "A vegetable off the bay.",
    "A block of tofu, pressed from soy.",
    "A pressure suit, for a walk outside.",
    "A laser handgun. Bought at a trader.",
    // The medkit and the bandage are still resources — the trade's book
    // has a row for each — but nothing uses either since task 120: a
    // downed crewmate is revived by hand.
    "A medkit. Nothing uses one any more: a downed crewmate is revived by a crewmate standing beside it.",
    "A bandage. Nothing uses one any more: a downed crewmate is revived by a crewmate standing beside it.",
    "A basic helm, for the head. Bought at a trader.",
    "Basic kevlar, for the body. Bought at a trader.",
    "Basic leg guards. Bought at a trader.",
    "A shotgun. Hits hard up close.",
    "An auto rifle. Fires in bursts.",
    "A sniper rifle. Reaches furthest.",
    "A schword, a blade with a laser edge. Cuts, at arm's length.",
    "A tier-one research key: an artifact off a station's research desk. Two cells tall. Put it in the ship's research desk and consume it there to open the locked part of the research tree.",
    "A tier-two research key: an artifact off the research desk of one of the few stations that keep one. Two cells tall. Put it in the ship's research desk and consume it there to open the upgrades node, which wants it and no other.",
    // 15 to 17: gone (task 127).
    "",
    "",
    "",
    "A minigun, tier 2 and up: twenty light bolts to a pull, ten a second, then a long cool. Shreds the machines; good armour shrugs off much of each bolt. Bought at a trader or combined, and only ever the crew's.",
    "A rail lance, tier 3 only: one slug every five seconds that goes through a body and on into the next — up to three, each after the first taking less. Walls and a Guardian's shield from the front stop it. Bought at a trader, and only ever the crew's.",
    "Arc greaves, tier 2 and up: leg guards wired to discharge. A melee blow landing on the wearer throws an arc into every enemy within two tiles — 15 at tier 2, 22.5 at tier 3 — once a second; a bolt never sets them off, and the discharge costs them nothing. Thinner than leg guards. Bought at a trader or combined, and only ever the crew's.",
    "A Reflective plate, tier 3 only: a mirrored body plate. Two in five enemy bolts landing on the body go back the way they came at half damage, doing the wearer and the plate nothing. Beams and blows are not bolts and are never sent back. No tier-3 dodge, and less armour than tier-3 kevlar. Bought at a trader, and only ever the crew's.",
];

pub fn item_tip(id: ResourceId) -> &'static str {
    ITEM_TIPS.get(id as usize).copied().unwrap_or("")
}

/// The research desk's row where a relic cache lies on it (feature 106).
pub const CACHE_ROW: &str = "Open the relic cache";
pub const CACHE_ROW_HINT: &str = "walk over and open it — one relic, kept if the site is cleared";

/// The Hire window's title, with the mercenary's name after it, the menu
/// row on a mercenary for hire — one of a friendly station's people in
/// the olive coverall, with a `?` over its head — that opens it, and the
/// button in it.
pub const HIRE_WINDOW: &str = "Hire";
pub const HIRE_ROW: &str = "Hire — see the terms";
pub const HIRE_BUTTON: &str = "Hire";
pub const HIRE_TIP: &str = "A mercenary lives at a friendly station and is for hire: the fee is a month of them, paid now and again every month after out of the crew's money, and it is what they carry — a heavier gun and a piece of armour each cost more. Hiring wants the Bim shown within two tiles of them (opening this walks it over) and the money for the first month. A month the money will not cover has them walk off at the next berth, for hire again.";
/// A mercenary hired for its trade rather than its gun (feature 86).
pub const FIELD_MEDIC: &str = "Field medic";
pub const FIELD_MEDIC_TIP: &str = "A field medic is hired to save your crew, not to win the fight. Under arms it keeps to the far end of its weapon's reach, fetches whoever goes down out of the fire — in its arms, at half pace, holding its fire — sets them down where it is quiet, and revives them there — in a medic's four seconds, where anybody else takes ten. It has none of a medic's own skills: the premium on the month is the trade.";
pub const BROKE_HINT: &str = "not the money for the first month";
pub const MERCENARY_MARK: &str = "?";

/// The Trader panel's line at a trader near the front (feature 94, task 114): the
/// guns, the armour and the medicine here are dearer than they are
/// anywhere quieter, and how much dearer is how near the machines are.
/// `front_premium` takes the hops.
pub const FRONT_PREMIUM_TIP: &str = "Weapons and armour are dearer this near the machines.";
pub fn front_premium(hops: u16) -> String {
    match hops {
        1 => "Front prices · the infection is one hop away".to_string(),
        n => format!("Front prices · the infection is {n} hops away"),
    }
}

/// The cart under the trade rows: nothing is bought or sold until it is
/// confirmed, and these are its words — the empty cart, which way the
/// money goes, what is left after, the two buttons, and why it cannot go.
pub const CART_EMPTY: &str = "Nothing in the cart.";
pub const YOU_PAY: &str = "You pay";
pub const YOU_EARN: &str = "You earn";
pub const MONEY_AFTER: &str = "Money after";
pub const CONFIRM_TRADE: &str = "Confirm trade";
pub const CLEAR_CART: &str = "Clear";
pub const SHORT_BY: &str = "Short by";
pub const NO_ROOM_FOR: &str = "No room for";
/// The trade rows' column headings: what one costs bought here (the
/// desk's ask), what the desk pays for one (its bid), and the rest.
pub const ASK_HEAD: &str = "Costs";
/// The tier column of the trade rows: which tier a gun or a piece of
/// armour is bought at (feature 95), blank for everything else.
pub const TIER_HEAD: &str = "Tier";
pub const BID_HEAD: &str = "Pays";
/// A price where nobody quotes one: a derelict's desk, or nowhere.
pub const NO_QUOTE: &str = "—";
pub const ABOARD_HEAD: &str = "Aboard";
pub const CART_HEAD: &str = "Cart";

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
pub const HEART_TIP: &str = "The Machine Heart, where the machines began. Its core cannot be hurt while any conduit stands — they are spread through the fortress's rooms, a line of red light from each to the core. Bring the last one down and the core sweeps a beam at whoever is nearest, and the fabricators beside it build a machine every half minute until they are wrecked. Take the core under a third of its health and it overloads: two beams, faster, and the fabricators building twice as fast. Destroy it and the run is won. Go back to the ship first and the fortress is as you found it.";
/// The top bar's count of how many enemies a wave is here and now
/// (`World::droid_wave_size`), shown always so `waves.ron` can be tuned
/// by eye.
pub fn wave_size_chip(n: u32) -> String {
    format!("Wave size {n}")
}
pub const WAVE_SIZE_TIP: &str = "How many enemies each wave is here and now: the base, one more for each player and more as the days go by (the first mission a little fewer). A wave already on the deck keeps the size it landed with. Tuned in waves.ron.";
pub const DROIDS_TIP: &str = "The station is held by the machines, and they come in waves. How many waves there are is worked out when you arrive, and how big each one is as it appears. Go back to the ship before the last wave is down and the station is as you found it — the bounty for what you destroyed is lost, the experience is kept — and the next visit is a fresh fight. No wave arrives while a machine of the last one is still standing — the countdown starts when the last of them is destroyed — and the next comes in through the airlock farthest from your own, or through a gate of the town on a planet.";

/// The same warning over a town the crew are defending (feature 94):
/// the fight is the machines', but the town's people are in it too.
pub const DEFENSE_TIP: &str = "The machines are coming for this place, and the first wave lands twenty seconds after you arrive — at a far airlock, or outside a town's gate — a wave at a time after that. Its armed defenders, a town's guard and whatever mercenaries live here fight beside you; everybody else goes indoors and stays there. Hold the last wave and the place is cleared — the Republic pays nothing for a defence; the people who live through it are the reward — and a town is yours to keep: it stays friendly even after its system falls, and some of its people join your crew. Go back to the ship before the last wave is down and it falls to the machines behind you.";
/// [`DEFENSE_TIP`] before day ten (task 131), when the attackers are the
/// Manufacturers' people and the machines they still command.
pub const DEFENSE_TIP_MANUFACTURERS: &str = "The Manufacturers are coming for this place — their people, and as the days go on more of the machines they still command beside them — and the first wave lands twenty seconds after you arrive, at a far airlock or outside a town's gate, a wave at a time after that. From the tenth day it is the machines alone. Its armed defenders, a town's guard and whatever mercenaries live here fight beside you; everybody else goes indoors and stays there. Hold the last wave and the place is cleared — the Republic pays nothing for a defence; the people who live through it are the reward — and a town is yours to keep: it stays friendly even after its system falls, and some of its people join your crew. Go back to the ship before the last wave is down and it falls behind you.";

/// The header's word while the crew's alarm is up, and what it means.
pub const ALARM_STATUS: &str = "To arms — an enemy is near";
pub const ALARM_TIP: &str = "An enemy within thirty tiles of anybody or in anybody's sight, or a crew member hit, in the last half minute: every crew member but the one you steer draws its weapon and fights, walking to wherever it can shoot from, until nobody is near, nobody has seen one and nobody has been hit for half a minute — then it goes back to its day, however many of the station's people are still alive somewhere on it. The one you steer is yours: recruit it yourself, or leave it to its errands.";

pub const REACH_HINT: &str = "walk over first — it is out of reach";

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
    /// ten to revive, a medic's four, three tenths of the bar back and
    /// thirty per cent slower after (task 120) — so a change to the
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
        assert_eq!((h::REVIVED_TO, h::DOWNED_PACE), (0.3, 0.7));
        assert_eq!(h::BLEEDS_UNDER, 20.0);
        for words in [HEALTH_TIP, DOWNED_TIP] {
            assert!(
                words.contains("three tenths") && words.contains("thirty per cent"),
                "{words}"
            );
        }
        assert!(HEALTH_TIP.contains("ten seconds") && HEALTH_TIP.contains("four"));
        assert!(HEALTH_TIP.contains("twenty") && CRITICAL_TIP.contains("twenty"));
        assert!(FIELD_MEDIC_TIP.contains("four seconds") && FIELD_MEDIC_TIP.contains("ten"));
        assert_eq!(downed_short(29.2), "DOWNED — dies in 30 s");
        assert_eq!(
            slowed_note(),
            "Was down this mission: walks 30% slower until it ends."
        );
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

        // --- every_resource_and_storage_class_has_a_name ---
        {
            assert_eq!(RESOURCE_NAMES.len(), ResourceId::CODES);
            assert_eq!(STORAGE_NAMES.len(), shipdesign::Storage::ALL.len());
        }

        // --- every_relic_has_a_name_and_a_line (feature 106) ---
        {
            assert_eq!(RELIC_NAMES.len(), world::Relic::ALL.len());
            for relic in world::Relic::ALL {
                assert!(!relic_line(relic).is_empty());
            }
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

        // --- every_issue_the_validator_can_raise_has_a_line ---
        {
            // The codes are written out in `IssueCode` and never renumbered:
            // 1 to 12 and 20 to 37, with the gap on purpose — and 30, the fuel
            // warning, retired with the fuel and left a hole.
            for code in (1..=12).chain(20..=37).filter(|&c| c != 30) {
                assert!(issue_line(code).is_some(), "issue {code} has no line");
            }
            assert!(issue_line(30).is_none(), "30 was retired");
        }
    }

    #[test]
    fn every_table_of_the_room_is_as_long_as_its_enum() {
        // --- the_classes_and_the_talents_are_named_and_every_pick_level_tipped ---
        {
            assert_eq!(CLASS_NAMES.len(), world::Class::ALL.len());
            assert_eq!(SITE_KIND_NAMES.len(), world::SiteKind::ALL.len());
            for (i, kind) in world::SiteKind::ALL.into_iter().enumerate() {
                assert_eq!(kind.code() as usize, i, "{kind:?}");
            }
            assert_eq!(CLASS_TIPS.len(), world::Class::ALL.len());
            // Indexed by code, with the codes no talent has any more left
            // empty (task 124).
            let places = world::Talent::ALL.iter().map(|t| t.code()).max().unwrap() as usize + 1;
            assert_eq!(TALENT_NAMES.len(), places);
            assert_eq!(TALENT_TIPS.len(), places);
            assert_eq!(DEPLOYABLE_NAMES.len(), world::DeployKind::ALL.len());
            // The two boxes at the foot of the screen: a name and a tip
            // for every class's two keys, and none for the classless
            // one, which has no keys (feature 80).
            assert_eq!(ABILITY_NAMES.len(), world::Class::ALL.len());
            assert_eq!(ABILITY_TIPS.len(), world::Class::ALL.len());
            for class in world::Class::ALL {
                for primary in [true, false] {
                    let named = !ability_name(class, primary).is_empty();
                    assert_eq!(named, class != world::Class::None);
                    assert_eq!(!ability_tip(class, primary).is_empty(), named);
                    assert_eq!(
                        world::class::key_level(class, primary).is_some(),
                        named,
                        "{class:?} has a key exactly where it has a box"
                    );
                }
            }
            for talent in world::Talent::ALL.iter() {
                assert!(!talent_name(*talent).is_empty(), "{talent:?}");
                assert!(!talent_tip(*talent).is_empty());
                // And what it is actually worth, in numbers (feature 83).
                assert!(
                    !talent_numbers(*talent).is_empty(),
                    "{talent:?} says what it is worth"
                );
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
                // A ranked kit has no levels of talents (task 124): its
                // words are the four abilities', pinned below.
                if world::class::ranked(class) {
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
                        assert!(ranked_tip(class, slot, 1).contains("Next, rank 2"));
                        assert!(ranked_tip(class, slot, 4).contains("top rank"));
                        assert!(!ranked_tip(class, slot, 2).contains("roll"));
                    }
                    continue;
                }
                for level in 1..=world::class::LEVELS {
                    assert_eq!(
                        world::class::pick_at(class, level).is_none(),
                        level_line(class, level).is_some(),
                        "{class:?} level {level}: a fixed level has a line and a pick level two talents"
                    );
                    // And the Skills tree's one slot is named wherever
                    // that line is said (feature 83).
                    assert_eq!(
                        level_line(class, level).is_some(),
                        level_name(class, level).is_some(),
                        "{class:?} level {level}: a fixed level is named as well as described"
                    );
                    // And a fixed level says what it is worth in numbers
                    // wherever it is named at all (feature 83).
                    assert_eq!(
                        level_name(class, level).is_some(),
                        level_numbers(class, level).is_some_and(|n| !n.is_empty()),
                        "{class:?} level {level}: a fixed level says what it is worth"
                    );
                }
            }
            for level in 1..=world::class::LEVELS {
                assert!(level_line(world::Class::None, level).is_none());
                assert!(level_name(world::Class::None, level).is_none());
                assert!(level_numbers(world::Class::None, level).is_none());
            }
            // The figures themselves, said the tree's way: no trailing
            // nought, two decimals at most.
            assert_eq!(fig(6.0), "6");
            assert_eq!(fig(1.15), "1.15");
            assert_eq!(fig(40.0 / 1.5), "26.67");
            assert_eq!(by(1.5), "×1.5");
            assert_eq!(step(6.0, 9.0, "tiles"), "6 -> 9 tiles");
            assert_eq!(pc(0.75), "75%");
            // The new refusals and events all say something.
            for why in [
                Refusal::ClassLocked,
                Refusal::NotAnEngineer,
                Refusal::NoKit,
                Refusal::NoSentryYet,
                Refusal::CantDeployThere,
                Refusal::NoSuchDeployable,
                Refusal::NotAPickLevel,
                Refusal::LevelNotReached,
                Refusal::AlreadyPicked,
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
                Refusal::OutOfCloakRange,
                Refusal::NoSightOfTarget,
                Refusal::Cloaked,
                Refusal::NotATank,
                Refusal::NoTauntYet,
                Refusal::NotACommander,
                Refusal::NoSquadInRange,
                Refusal::NoEnemyThere,
            ] {
                assert!(!refusal(why).is_empty());
                assert!(deploy_refused(why).contains(refusal(why)));
                assert!(throw_refused(why).contains(refusal(why)));
                assert!(brace_refused(why).contains(refusal(why)));
                assert!(beam_refused(why).contains(refusal(why)));
                assert!(nanite_burst_refused(why).contains(refusal(why)));
                assert!(cloak_refused(why).contains(refusal(why)));
                assert!(bulwark_refused(why).contains(refusal(why)));
                assert!(taunt_refused(why).contains(refusal(why)));
                assert!(squad_refused(why).contains(refusal(why)));
                assert!(rally_refused(why).contains(refusal(why)));
            }
            assert_eq!(squad_line(None, 0), SQUAD_NONE);
            assert_eq!(squad_line(Some(0), 3), "Squad attacking — 3");
            assert_eq!(squad_line(Some(1), 2), "Squad falling back — 2");
            assert_eq!(squad_line(Some(2), 1), "Squad holding ground — 1");
            assert_eq!(rally_line(0.0, 0.0, false), RALLY_NOT_LEARNT);
            assert_eq!(rally_line(0.0, 0.0, true), "Rally ready");
            assert_eq!(rally_line(0.0, 7.2, true), "Rally ready in 7 s");
            assert_eq!(rally_line(4.0, 12.0, true), "Rallying — 4 s left");
            assert_eq!(battle_cry_line(2.0, 9.0, true), "Battle Cry — 2 s left");
            assert_eq!(battle_cry_line(0.0, 0.0, true), "Battle Cry ready");
            assert_eq!(
                taunt_line(0.0, 0.0, false),
                format!("Taunt at level {}", world::class::TAUNT_LEVEL)
            );
            assert_eq!(taunt_line(0.0, 0.0, true), "Taunt ready");
            assert_eq!(taunt_line(0.0, 7.2, true), "Taunt ready in 7 s");
            assert_eq!(taunt_line(4.0, 12.0, true), "Taunting — 4 min left");
            assert_eq!(grenades_line(1, 0.0), "1 grenade");
            assert_eq!(grenades_line(2, 3.4), "2 grenades — next in 3 s");
            assert_eq!(beam_line(&[]), BEAM_OFF);
            assert_eq!(beam_line(&["Kate".to_string()]), "Beaming Kate");
            assert_eq!(
                beam_line(&["Kate".to_string(), "Ali".to_string()]),
                "Beaming Kate and Ali"
            );
            assert_eq!(nanite_burst_line(0.0, false), "Nanite Burst not learnt yet");
            assert_eq!(nanite_burst_line(3.4, true), "Nanite Burst ready in 3 s");
            assert_eq!(nanite_burst_line(0.0, true), "Nanite Burst ready");
            assert_eq!(cloak_line(4.2, 30.0, true), "Cloaked — 4 s left");
            assert_eq!(cloak_line(0.0, 0.0, false), "Cloak not learnt yet");
            assert_eq!(cloak_line(0.0, 12.0, true), "Cloak ready in 12 s");
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
                WorldEvent::TalentPicked { who: 0, talent: 0 },
                WorldEvent::Deployed { who: 0, kind: 0 },
                WorldEvent::PackedUp { who: 0, kind: 1 },
                WorldEvent::DeployableLost { kind: 1 },
                WorldEvent::Braced { who: 0, on: true },
                WorldEvent::Braced { who: 0, on: false },
                WorldEvent::Thrown { who: 0 },
                WorldEvent::EmpThrown { who: 0 },
                WorldEvent::SentryDone { who: 0 },
                WorldEvent::DeployableLost { kind: 2 },
                WorldEvent::Beamed {
                    who: 0,
                    patient: Some(1),
                },
                WorldEvent::Beamed {
                    who: 0,
                    patient: None,
                },
                WorldEvent::NaniteBurst { who: 0, healed: 3 },
                WorldEvent::Cloaked { who: 0, target: 0 },
                WorldEvent::Cloaked { who: 0, target: 1 },
                WorldEvent::Bulwarked { who: 0, on: true },
                WorldEvent::Bulwarked { who: 0, on: false },
                WorldEvent::Taunted { who: 0 },
                WorldEvent::Squadded { who: 0, kind: 0 },
                WorldEvent::Squadded { who: 0, kind: 1 },
                WorldEvent::Squadded { who: 0, kind: 2 },
                WorldEvent::Squadded {
                    who: 0,
                    kind: u32::MAX,
                },
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
            assert!(event_line(WorldEvent::CrewHit { who: 0, part: 2 }).is_some());
            assert!(event_line(WorldEvent::CrewDown { who: 0 }).is_some());
            assert!(
                event_line(WorldEvent::PieceBroke {
                    who: 0,
                    kind: ArmourKind::BasicKevlar
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
                WorldEvent::KeyFound { keys: 2 },
                WorldEvent::Respawned {
                    who: 1,
                    paid: 5_000,
                },
                WorldEvent::ShelfBought {
                    slot: 0,
                    index: 1,
                    to: u32::MAX,
                },
                WorldEvent::Combined {
                    slot: 0,
                    who: 0,
                    tier: 2,
                },
                WorldEvent::RelicBought {
                    slot: 1,
                    relic: 0,
                    price: 3_000,
                },
                WorldEvent::Restocked { slot: 0 },
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
                Refusal::NotAPair,
                Refusal::NoRestock,
                Refusal::Restocked,
            ] {
                assert!(!refusal(why).is_empty());
            }
        }

        // --- every_part_of_a_body_has_a_name ---
        {
            // `CrewHit` carries the part as a code, and the hit line runs it
            // into a sentence.
            assert_eq!(BODY_PART_NAMES.len(), bims::health::Part::ALL.len());
            for part in bims::health::Part::ALL {
                assert!(!body_part_name(part.code()).is_empty());
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

        // --- the_weapon_lines_say_the_curves_the_user_asked_for ---
        {
            // The four sentences the Inventory was rewritten for, off the
            // real tables: a two-point curve, a flat one, a burst, a blade.
            use bims::combat::WeaponKind;
            let shotgun = WeaponKind::Shotgun.stats();
            assert_eq!(damage_text(&shotgun), "60 to 4 tiles, 36 at 10");
            let pistol = WeaponKind::LaserPistol.stats();
            assert_eq!(damage_text(&pistol), "7.2 a shot");
            assert_eq!(fire_rate_text(&pistol), "1.5 a second");
            let rifle = WeaponKind::AutoRifle.stats();
            assert_eq!(fire_rate_text(&rifle), "8 in 2 s, then 2 s");
            let schword = WeaponKind::Schword.stats();
            assert_eq!(melee_text(&schword), "Melee — 42 a swing every 2 s");
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
}
