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
pub const PART_NAMES: [&str; 50] = [
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
    "Smelter",
    "Workbench",
    "Suit locker",
    "Armoury",
    "Drug lab",
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
        &[
            PartKind::Smelter as u32,
            PartKind::Workbench as u32,
            PartKind::Armoury as u32,
            PartKind::DrugLab as u32,
        ],
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

/// The Build tab's categories, the way a colonist-game player thinks about
/// them rather than the way a shipwright does: what holds the ship
/// together, what it is lived in with, what makes things, and so on. Every
/// kind the palette offers is in exactly one — `every_buildable_part_is_in_one_build_group`
/// pins it — and the frame is left out for `NOT_A_TOOL`'s reason. Each is
/// a name and a line saying what goes under it, the button's tooltip.
pub const BUILD_GROUPS: &[(&str, &str, &[u32])] = &[
    (
        "Structure",
        "The frame and the skin: deck to walk on, walls to divide it, the hull that keeps the outside out, the ways through, and sandbags for cover.",
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
        "Furniture",
        "What the crew live with: somewhere to sleep, sit and eat, somewhere to keep things, a desk to trade across, the research desk the AI works at, the lights — a deck no light reaches is a dark one, and the crew see ten tiles in the dark — and the comforts, a plant or a picture, which lift the surroundings of the deck round them.",
        &[
            PartKind::WallLight as u32,
            PartKind::StandingLight as u32,
            PartKind::SmallPlant as u32,
            PartKind::BigPlant as u32,
            PartKind::Picture as u32,
            PartKind::Bunk as u32,
            PartKind::Table as u32,
            PartKind::Chair as u32,
            PartKind::Shelf as u32,
            PartKind::BroomLocker as u32,
            PartKind::TradingDesk as u32,
            PartKind::ResearchDesk as u32,
        ],
    ),
    (
        "Production",
        "Where something is made: the workshop benches, the armoury, the drug lab, and the bay that grows the food.",
        &[
            PartKind::Smelter as u32,
            PartKind::Workbench as u32,
            PartKind::Armoury as u32,
            PartKind::DrugLab as u32,
            PartKind::HydroBay as u32,
        ],
    ),
    (
        "Galley",
        "Where a meal is cooked and cleared up after.",
        &[
            PartKind::ColdStore as u32,
            PartKind::Worktop as u32,
            PartKind::Hob as u32,
            PartKind::Dishwasher as u32,
        ],
    ),
    (
        "Hygiene",
        "The heads and the shower.",
        &[
            PartKind::Toilet as u32,
            PartKind::Basin as u32,
            PartKind::Shower as u32,
        ],
    ),
    (
        "Power",
        "What makes power, what carries it, and what holds it.",
        &[
            PartKind::Reactor as u32,
            PartKind::FusionReactor as u32,
            PartKind::PowerConduit as u32,
            PartKind::Battery as u32,
        ],
    ),
    (
        "Ship systems",
        "What flies the ship and keeps it alive: the helm, life support, the sensors, and the suits.",
        &[
            PartKind::Helm as u32,
            PartKind::LifeSupport as u32,
            PartKind::SensorArray as u32,
            PartKind::SuitLocker as u32,
        ],
    ),
    (
        "Propulsion",
        "The engines that push, the thrusters that turn, and the hyperdrive that jumps.",
        &[
            PartKind::Engine as u32,
            PartKind::HeavyEngine as u32,
            PartKind::Thruster as u32,
            PartKind::Hyperdrive as u32,
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

/// What a station sells, indexed by `physics::ResourceId`.
pub const RESOURCE_NAMES: [&str; 26] = [
    "Ore",
    "Metal",
    "Components",
    "Vegetables",
    "Tofu",
    "Galvum",
    "Emitters",
    "Suits",
    "Handguns",
    "Vests",
    "Medkits",
    "Rock",
    "Fibre",
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
    "Sandbag kits",
    "Sentry kits",
    "Grenades",
];

pub fn resource_name(id: ResourceId) -> &'static str {
    RESOURCE_NAMES
        .get(id as usize)
        .copied()
        .unwrap_or("Something")
}

/// The same off a `ResourceId` code, for an event that carries one.
pub fn resource_name_by_code(code: u32) -> &'static str {
    RESOURCE_NAMES
        .get(code as usize)
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
pub const STORAGE_NAMES: [&str; 4] = ["Shelves", "Cold stores", "Lockers", "Research desk"];

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

/// Why a trip could not be planned. Indexed by `flight::PlanError`.
pub fn plan_error(code: u32) -> &'static str {
    match code {
        1 => "No engine pushes the ship forward — none built, or none on a live reactor.",
        2 => "Nothing to turn with — it cannot aim or stop.",
        3 => "No helm to fly it from.",
        5 => "Nobody has found that yet.",
        6 => "The ship is already there.",
        _ => "That cannot be flown.",
    }
}

/// Why an order did nothing. Separate from the list above on purpose: "you
/// are not docked" and "you have no engine" are different things to be told.
pub fn refusal(why: Refusal) -> &'static str {
    match why {
        Refusal::NotDocked => "not while the ship is away from a station",
        Refusal::Unaffordable => "there is not the money",
        Refusal::NoRoomAboard => "there is nowhere aboard to put it",
        Refusal::NotAboard => "there is not that much aboard to sell",
        Refusal::NotAtTheHelm => "nobody of yours is at the helm",
        Refusal::NotTravelling => "there is no trip to stop",
        Refusal::SumTooBig => "the sum will not go",
        Refusal::ComingAlongside => "the ship is coming alongside; wait until it is tied up",
        Refusal::NotSoldHere => "this station does not sell that",
        Refusal::UnderWay => "nothing is built on a ship that is moving",
        Refusal::WontFit => "that will not go there",
        Refusal::NoSuchSite => "that site is not there any more",
        Refusal::UnderConstruction => {
            "the ship stays put while something is being built — cancel the site, or let them finish"
        }
        Refusal::OutOfReach => "it is out of reach — walk over first",
        Refusal::PackFull => "the pack is full",
        Refusal::NoRoom => "there is no room for it aboard",
        Refusal::Broken => "it is broken, and worth nothing put away — discard it",
        Refusal::NotDown => "nobody on their feet is looted — it is not down any more",
        Refusal::NotForHire => "that is not a mercenary for hire",
        Refusal::NoBunk => "there is no bunk aboard for one more",
        Refusal::NotAtTheDesk => "nobody of yours is at the trading desk — walk over first",
        Refusal::NoKey => "there is no research key there",
        Refusal::NoResearchDesk => "the ship has no research desk running — the AI works on one",
        Refusal::NotResearchable => "that cannot be queued for research now",
        Refusal::NotResearched => "the crew do not know how to build that yet",
        Refusal::NotHostile => "only an enemy's people are finished off",
        Refusal::Unarmed => "nothing in hand to do it with",
        Refusal::NoHyperdrive => {
            "there is no working hyperdrive — one bolted to an engine, on a live cable"
        }
        Refusal::NotHolding => "the ship has to be holding on its own, away from any berth",
        Refusal::NoSuchStar => "there is no such star",
        Refusal::SameStar => "the ship is at that star already",
        Refusal::NoPlanetHere => {
            "a landing wants the ship holding over a rocky planet or an ice world"
        }
        Refusal::NoMarket => "there is nobody here to sell to",
        Refusal::NoWorkbench => "there is no workbench aboard to put it on",
        Refusal::NoPair => {
            "the bench takes two of a kind at one tier — the same weapon or piece, below tier three"
        }
        Refusal::BenchBusy => "the bench is at work on what is on it — wait for the day to finish",
        // A walk ordered on the deck (`Command::Crew`): the room's two
        // refusals, said the way `order_refused` says them in the test room.
        Refusal::DoorLocked => "the bathroom door is locked on the only way there",
        Refusal::NoWayThere => "there is no way there at all",
        Refusal::NotQueued => "that is not on the research queue",
        Refusal::NoUpgrades => {
            "the crew do not know how to upgrade gear yet — research Upgrades, behind a tier-two key"
        }
        Refusal::ClassLocked => "a class is chosen before the ship first leaves its berth",
        Refusal::NotAnEngineer => "only an engineer does that",
        Refusal::NoKit => "there is no such kit in the pack",
        Refusal::NoSentryYet => "a sentry wants the engineer's third level",
        Refusal::CantDeployThere => {
            "that tile will not take it — clear deck floor within reach, not a door, nothing on it"
        }
        Refusal::NoSuchDeployable => "there is nothing of the kind there",
        Refusal::NotAPickLevel => "that level has no talent to pick",
        Refusal::LevelNotReached => "that level has not been reached",
        Refusal::AlreadyPicked => "that level's talent is picked, and a pick is never changed",
        Refusal::NoTalent => "the engineer has not learnt that",
        Refusal::NoClass => "a crew member with no class has no talents to pick",
        Refusal::NotASoldier => "only a soldier does that",
        Refusal::NoGrenade => "no grenade charge in the pack: the next is still coming back",
        Refusal::NoGrenadesYet => "grenades want the soldier's third level",
        Refusal::CoolingDown => "that skill is still cooling down",
        Refusal::OutOfThrowRange => "that tile is out of throwing range",
        Refusal::NoLineToTile => "there is a wall or a shut door in the way",
        Refusal::CantThrowThere => "that tile is not deck",
        Refusal::NotAMedic => "only a medic does that",
        Refusal::NoPatient => "there is nobody there to beam",
        Refusal::NotACrewmate => "the beam holds a crewmate, not that",
        Refusal::OutOfBeamRange => "they are too far off for the beam",
        Refusal::NoSightOfPatient => "the medic cannot see them",
        Refusal::NoSurgeYet => "a surge wants the medic's third level",
        Refusal::NotCharged => "the surge is not charged yet",
        Refusal::NotLinked => "the beam is on nobody",
        Refusal::NotATank => "only a tank can do that",
        Refusal::NoTauntYet => "a taunt wants the third level",
        Refusal::NotACommander => "only a commander can do that",
        Refusal::NoRallyYet => "a rally wants the commander's third level",
        Refusal::NoSquadInRange => "nobody of the squad is near enough to hear it",
        Refusal::NoEnemyThere => "there is no enemy under the pointer",
        Refusal::NoGroundThere => "there is no ground to attack there",
        Refusal::NotCarrying => "only a medic carries somebody, and only one at a time",
        Refusal::NotHurt => "they are on their feet and can walk out themselves",
        Refusal::AlreadyCarried => "somebody has them already",
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
pub const RESTART_LINE: &str = "Play this run again from the situation it opened in — the fight, the raid, the landing, or the game the yard started. Everything since is lost, and a saved game is not touched.";
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
    "Lays sandbags for cover (E) and, from the third level, a sentry that shoots for itself (Q); packs either up again; mends armour at the workbench once it has learnt to. Sets out with three sandbag kits and one sentry kit.",
    "Braces to hold a line (E) — steadier shooting, never running, no errands until stood easy — and from the third level throws grenades (Q), two charges of them, each back thirty seconds after it is thrown. Sets out with an auto rifle in hand and the pistol in the pack.",
    "Holds a crewmate up with the heal beam (E) — their wounds stop bleeding and their blood comes back — and from the third level shields them both with a surge (Q), which takes every hit for eight minutes. Fires nothing while the beam is on. Sets out with the pistol, two medkits and four bandages.",
    "Stands as a wall (E) — half pace, and the crew close behind him are in cover against anything shot through him — and from the third level taunts (Q), so every enemy that can see him shoots at him and nobody else for six minutes. His armour drains at half rate, so the same kevlar takes twice as much on him. Sets out with the pistol and a basic helm, kevlar and leg guards on.",
    "Lifts every friendly Bim within eight tiles of him — yours as well as the crew's — a tenth faster at work, a tenth steadier with a gun, and slower to run; and orders the squad, which is every crew member nobody is steering: attack the enemy under the pointer (E), fall back to a tile (X), stand ground (Z). From the third level he rallies (Q). Hires a mercenary at a quarter off. Sets out with the pistol.",
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
    ["Sentry", "Sandbags"],
    ["Grenade", "Brace"],
    ["Surge", "Heal beam"],
    ["Taunt", "Wall"],
    ["Rally", "Squad"],
];
/// What each box says when it is rested on.
pub const ABILITY_TIPS: [[&str; 2]; 6] = [
    ["", ""],
    [
        "Lay a sentry on the deck tile under the pointer: it shoots for itself at whatever it can see for as long as it stands, and is packed up from the Nearby strip. The number is the charges in the pack, each back a minute after it is spent; laying one over your limit destroys your oldest.",
        "Lay sandbags on the deck tile under the pointer: low cover, walked and seen over, ducked behind, and gone once shot to pieces. The number is the charges in the pack, each back three quarters of a minute after it is spent.",
    ],
    [
        "Throw a grenade at the deck tile under the pointer — in range, with nothing solid in the way. It bursts two seconds later and hurts whoever is near it, yours as well as theirs. The number is the grenades in the pack.",
        "Brace where you stand: steadier shooting, no running and no errands until you stand easy. The key again stands easy, and so does any order that moves you.",
    ],
    [
        "Trigger the surge on the beam's patients and yourself: every hit is taken whole for its length — no wound, no armour drained. The bar is the charge, which fills while the beam holds somebody who is bleeding or short of blood.",
        "Hold the heal beam on the crew member under the pointer: their wounds stop bleeding and their blood comes back. The key on nobody, or on the one held, unlinks it. You fire nothing while it is on. The number is how many more you could hold.",
    ],
    [
        "Taunt: every enemy that can see you shoots at you and nobody else while it lasts.",
        "Stand as a wall: half pace, and a crewmate close behind you is in cover against anything shot through you. The key again puts it down; so does going down.",
    ],
    [
        "Call a rally: every friendly Bim near you shoots steadier and holds its nerve while it lasts.",
        "Send the squad at the enemy under the pointer. The squad is every crew member nobody is steering; the number is how many are in it now.",
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
/// The boxes past the class's own two (feature 86): the commander's
/// other two squad orders, which had keys and no box until now, and the
/// medic's carry. Named off the [`crate::keys::Action`] rather than off
/// a class's pair, since these are one class's each and a pair has no
/// room for a third.
pub const FALL_BACK: &str = "Fall back";
pub const FALL_BACK_TIP: &str = "Call the squad back to the deck tile under the pointer, or to yourself with the pointer on nothing. They hold their fire and walk, and hold the ring round the spot when they get there. The number is how many are in the squad.";
pub const STAND_GROUND: &str = "Stand ground";
pub const STAND_GROUND_TIP: &str = "The squad holds exactly where it stands — no walk to cover, no running — shooting whatever it can see. The number is how many are in the squad.";
pub const CARRY: &str = "Carry";
pub const CARRY_TIP: &str = "Pick the crewmate under the pointer up — out cold, dying, or bleeding — and carry them out of the fire. You hold your fire and walk slowly while you do. The key again sets them down, and treating them is what comes next. The number is how many near you are worth fetching.";

/// The line under a box whose level is not reached yet.
pub fn ability_locked(level: u8) -> String {
    format!("Level {level}")
}
/// The Skills tab (feature 83): the class's ten levels as a tree in the
/// tray, what a level's slot says, and the button that spends a point.
pub const SKILLS: &str = "Skills";
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
pub const TALENT_NAMES: [&str; 70] = [
    "Reinforced sand",
    "Site foreman",
    "Sandbagger",
    "Bulk bags",
    "Armoured sentry",
    "Enhanced optics",
    "Armourer",
    "Higher quality armour",
    "Dug in",
    "Quick build",
    "Extra bags",
    "Steady hands",
    "Second sentry",
    "Sentry mark III",
    "Marksman",
    "Point blank",
    "Runner",
    "Steady aim",
    "Iron nerve",
    "Cover master",
    "Long throw",
    "Short fuse",
    "Frag",
    "Quick draw",
    "Bruiser",
    "Dug in",
    "Deadeye",
    "Rampage",
    "Field dressing",
    "Surgeon",
    "Long beam",
    "Strong beam",
    "Clean hands",
    "Steady hands",
    "Quick charge",
    "Long surge",
    "Self-care",
    "Double link",
    "Gunner medic",
    "Closing surge",
    "Mass surge",
    "Field surgeon",
    "Pack mule",
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
    "Wide presence",
    "Strong presence",
    "Haggler",
    "Outfitter",
    "Focus fire",
    "Pincer",
    "Long rally",
    "Quick rally",
    "Steady ranks",
    "Double time",
    "Relentless",
    "Grit",
    "Anchor",
    "Warcry",
];
pub const TALENT_TIPS: [&str; 70] = [
    "Every bag you lay takes fifty more before it is gone.",
    "Put a construction site together a quarter faster.",
    "Lay sandbags in half the time.",
    "One sandbag charge lays two tiles side by side.",
    "A sentry with half again the health.",
    "A sentry that shoots ten tiles further.",
    "Mend a damaged piece of armour at the workbench: the piece and a bar of metal in, ten points back on it.",
    "Armour you wear holds five per cent more and stops one more point of every hit.",
    "Sandbags anywhere between a sentry and the shooter are cover for it.",
    "Set a sentry up in half the time.",
    "One more sandbag charge.",
    "A hit no longer stops the laying of a kit.",
    "Two sentry charges, so two may stand at once.",
    "The sentry carries a tier-three sniper rifle at double the rate and a fifth more damage.",
    "Every weapon's odds up by fifteen per cent.",
    "A fifth more damage within the weapon's sweet range.",
    "A fifth faster on foot while an enemy is in sight.",
    "The odds on the move halved less: three quarters of standing still, not half.",
    "Never runs from a fight, however badly hurt.",
    "Half again the odds of a bolt missing in cover.",
    "Grenades thrown half again as far.",
    "A grenade's fuse half as long.",
    "A grenade's burst half again as wide.",
    "A thrown grenade's charge comes back in half the time.",
    "Fists and the schword hit half again as hard.",
    "Ten per cent more chance of slipping a bolt while braced.",
    "Every weapon's odds at the edge of its range are its odds up close.",
    "Each enemy downed raises the fire rate by a tenth, up to three times, until the fight ends.",
    "Bandages a wound in half the time.",
    "Treats a trauma with a medkit in half the time.",
    "The heal beam reaches half again as far.",
    "The heal beam gives back half again the blood an hour.",
    "A trauma this medic treats leaves nothing lasting behind.",
    "A part this medic treats comes back half again as far.",
    "The surge charges half again as fast.",
    "A surge lasts half again as long.",
    "The medic's own wounds do not bleed while the beam is on.",
    "The beam holds two crewmates at once, each at the full rate.",
    "Fires while beaming, at half the rate.",
    "A surge ending closes every open wound on the patient.",
    "A surge covers every crew member within three tiles of the patient.",
    "Once a fight, treats a trauma with no medkit at all, in half the time.",
    "Carries two loads a trip when hauling to a construction site.",
    "Every piece of armour he wears protects half again as much.",
    "Forces a locked door in half the time.",
    "Never runs from a fight, and loses no pace to low blood while his kevlar holds.",
    "The wall shelters twice as far to either side.",
    "Walks half again as fast with the wall up.",
    "A taunt reaches half again as far.",
    "A taunt lasts half again as long.",
    "His wounds and traumas do not bleed while he taunts.",
    "Ten per cent more chance of slipping a bolt while the wall is up.",
    "A bolt that would hit somebody the wall shelters hits him instead.",
    "A taunt turns every charging blade within its reach towards him.",
    "His armour drains at half rate again — a quarter of anybody else's.",
    "While he taunts, every crew member within three tiles drains armour at half rate too.",
    "The aura reaches half again as far.",
    "Every one of the aura's bonuses is half again as deep.",
    "Hires a mercenary at two fifths off instead of a quarter.",
    "A mercenary he hires arrives wearing the lowest basic piece it was missing, at no extra cost.",
    "The squad's odds against the enemy it has been sent after are up by fifteen per cent.",
    "An attack may mark two enemies at once, the squad split between them.",
    "A rally lasts half again as long.",
    "The rally's cooldown is half as long.",
    "Bims in his aura bleed at three quarters the rate.",
    "Bims in his aura walk a tenth faster.",
    "An attack's mark lasts until that enemy is dead, not merely down.",
    "During a rally, Bims in it lose no pace at all to wounds or traumas.",
    "The aura's bonuses are twice as deep while he stands still.",
    "A rally covers every friendly Bim in the room, however far off.",
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
        (world::Class::Engineer, 1) => "Lays sandbags and packs deployables up.",
        (world::Class::Engineer, 3) => "May set up a sentry.",
        (world::Class::Engineer, 7) => "Sentry mark II: its rifle at tier two.",
        (world::Class::Soldier, 1) => "Brace: holds a line, and shoots steadier for it.",
        (world::Class::Soldier, 3) => "May throw grenades.",
        (world::Class::Soldier, 7) => "Drill: every weapon's fire rate up by a fifth.",
        (world::Class::Medic, 1) => "Heal beam: holds a crewmate's blood up.",
        (world::Class::Medic, 3) => "Surge: may shield the medic and the patient.",
        (world::Class::Medic, 7) => "Mender: a beamed patient's parts mend ten times as fast.",
        (world::Class::Tank, 1) => "Bulwark: stands as a wall, and his armour drains at half rate.",
        (world::Class::Tank, 3) => "Taunt: may draw the enemy's fire onto himself.",
        (world::Class::Tank, 7) => "Iron frame: a hit rolled on his head lands on his body.",
        (world::Class::Commander, 1) => {
            "Aura: every friendly Bim near him works, shoots and holds better. Hires at a quarter off. Squad orders: attack (E), fall back (X), stand ground (Z)."
        }
        (world::Class::Commander, 3) => "Rally: may call it.",
        (world::Class::Commander, 7) => {
            "Long reach: a squad order reaches every squad member in the room."
        }
        _ => return None,
    })
}
/// What a fixed level is *called*, for its one slot on the Skills tree —
/// the same three levels [`level_line`] describes, said in a word or two.
/// `None` wherever `level_line` is `None`: a pick level has two talents
/// with names of their own, and the classless one has nothing at all.
pub fn level_name(class: world::Class, level: u8) -> Option<&'static str> {
    Some(match (class, level) {
        (world::Class::Engineer, 1) => "Sandbags",
        (world::Class::Engineer, 3) => "Sentry",
        (world::Class::Engineer, 7) => "Sentry mark II",
        (world::Class::Soldier, 1) => "Brace",
        (world::Class::Soldier, 3) => "Grenades",
        (world::Class::Soldier, 7) => "Drill",
        (world::Class::Medic, 1) => "Heal beam",
        (world::Class::Medic, 3) => "Surge",
        (world::Class::Medic, 7) => "Mender",
        (world::Class::Tank, 1) => "Bulwark",
        (world::Class::Tank, 3) => "Taunt",
        (world::Class::Tank, 7) => "Iron frame",
        (world::Class::Commander, 1) => "Aura and squad",
        (world::Class::Commander, 3) => "Rally",
        (world::Class::Commander, 7) => "Long reach",
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

/// How far a sentry's gun reaches at a tier, in tiles: the auto rifle at
/// the first two and the sniper rifle at the third, which is the gun
/// *sentry mark III* hands it (feature 88). The one place the app says a
/// sentry's range, off `bims::combat`'s own tables.
fn sentry_range(tier: bims::combat::Tier) -> f32 {
    use bims::combat::{Tier, WeaponKind};
    let kind = if tier == Tier::Three {
        WeaponKind::SniperRifle
    } else {
        WeaponKind::AutoRifle
    };
    kind.at(tier).stats().range
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
        // --- the engineer's ---
        T::ReinforcedSand => format!(
            "Sandbag health {}",
            step(
                world::deploy::SANDBAG_HEALTH as f64,
                (world::deploy::SANDBAG_HEALTH + c::REINFORCED_SAND_HEALTH) as f64,
                ""
            )
        ),
        T::SiteForeman => format!("Building {} — every working step at a site", by(c::SITE_FOREMAN_EFFORT as f64)),
        T::Sandbagger => format!(
            "Laying sandbags {} game minutes ({})",
            step(
                world::deploy::DEPLOY_SANDBAG_MINUTES,
                world::deploy::DEPLOY_SANDBAG_MINUTES * c::SANDBAGGER_TIME,
                ""
            ),
            by(c::SANDBAGGER_TIME)
        ),
        T::BulkBags => format!(
            "One charge lays 1 -> 2 tiles of sandbags, each at its own {} health",
            fig(world::deploy::SANDBAG_HEALTH as f64)
        ),
        T::ArmouredSentry => format!(
            "Sentry health {} ({})",
            step(
                world::deploy::SENTRY_HEALTH as f64,
                (world::deploy::SENTRY_HEALTH * c::ARMOURED_SENTRY_HEALTH) as f64,
                ""
            ),
            by(c::ARMOURED_SENTRY_HEALTH as f64)
        ),
        // The auto rifle's range is the same at tiers one and two — only
        // tier three scales it — so there is one figure to show.
        T::EnhancedOptics => format!(
            "Sentry fire range {} tiles",
            step(
                sentry_range(bims::combat::Tier::One) as f64,
                (sentry_range(bims::combat::Tier::One) + c::ENHANCED_OPTICS_RANGE) as f64,
                ""
            )
        ),
        T::Armourer => format!(
            "{} points back on a piece of armour per metal, {} game minutes at the workbench",
            fig(c::ARMOUR_REPAIR_PER_METAL as f64),
            c::ARMOUR_REPAIR_MINUTES
        ),
        T::BetterArmour => format!(
            "Armour he wears holds {} more and stops {} more of every hit — a basic kevlar's {} becomes {} of protection, and its 20 health absorbs {}",
            pc((c::BETTER_ARMOUR_HEALTH - 1.0) as f64),
            fig(c::BETTER_ARMOUR_PROTECTION as f64),
            fig(2.0),
            fig(2.0 + c::BETTER_ARMOUR_PROTECTION as f64),
            fig((20.0 * c::BETTER_ARMOUR_HEALTH) as f64)
        ),
        T::DugIn => "Sandbags on the line between a sentry and its shooter are cover for the sentry — no number of its own, the shooter's odds fall as they do against a body in cover".to_string(),
        T::QuickBuild => format!(
            "Laying a sentry {} game minutes ({})",
            step(
                world::deploy::DEPLOY_SENTRY_MINUTES,
                world::deploy::DEPLOY_SENTRY_MINUTES * c::QUICK_BUILD_TIME,
                ""
            ),
            by(c::QUICK_BUILD_TIME)
        ),
        T::ExtraBags => format!(
            "Sandbag charges {}, each back after {} seconds",
            step(
                world::deploy::SANDBAG_CHARGES as f64,
                (world::deploy::SANDBAG_CHARGES + c::EXTRA_BAGS_CHARGES) as f64,
                ""
            ),
            fig(world::deploy::SANDBAG_COOLDOWN)
        ),
        T::SteadyHands => "A hit no longer stops a laying: the deploy runs to its end whatever lands".to_string(),
        T::SecondSentry => format!(
            "Sentry charges {} — and so sentries standing at once, a third laid destroying the oldest",
            step(
                world::deploy::SENTRY_CHARGES as f64,
                c::SECOND_SENTRY_CHARGES as f64,
                ""
            )
        ),
        T::SentryMarkThree => format!(
            "A tier-3 sniper rifle in place of the auto rifle: fire rate {}, damage {}, range {} tiles against the tier-2 auto rifle's {}",
            by(c::SENTRY_MARK_THREE_FIRE_RATE as f64),
            by(c::SENTRY_MARK_THREE_DAMAGE as f64),
            fig(sentry_range(bims::combat::Tier::Three) as f64),
            fig(sentry_range(bims::combat::Tier::Two) as f64)
        ),
        // --- the soldier's ---
        T::Marksman => format!(
            "Every weapon's hit chance {} — a {} shot becomes {}",
            by(c::MARKSMAN_ACCURACY as f64),
            pc(0.6),
            pc(0.6 * c::MARKSMAN_ACCURACY as f64)
        ),
        T::PointBlank => format!(
            "Damage within the weapon's sweet range {} — a 30-point hit becomes {}",
            by(c::POINT_BLANK_DAMAGE as f64),
            fig(30.0 * c::POINT_BLANK_DAMAGE as f64)
        ),
        T::Runner => format!(
            "Pace {} while an enemy is in sight",
            by(c::RUNNER_PACE as f64)
        ),
        T::SteadyAim => format!(
            "Hit chance on the move {} -> {} of standing still — the penalty halved",
            pc(bims::combat::WALKING_ACCURACY as f64),
            pc(c::steady_aim_walking() as f64)
        ),
        T::IronNerve => "Never runs, however badly hurt: the flee roll is off him for good".to_string(),
        T::CoverMaster => format!(
            "Odds of a bolt missing him in cover {} -> {} ({})",
            pc(bims::balance::DODGE_IN_COVER as f64),
            pc((bims::balance::DODGE_IN_COVER * c::COVER_MASTER_DODGE) as f64),
            by(c::COVER_MASTER_DODGE as f64)
        ),
        T::LongThrow => format!(
            "Grenade range {} tiles ({})",
            step(
                c::GRENADE_RANGE as f64,
                (c::GRENADE_RANGE * c::LONG_THROW_RANGE) as f64,
                ""
            ),
            by(c::LONG_THROW_RANGE as f64)
        ),
        T::ShortFuse => format!(
            "Grenade fuse {} seconds ({})",
            step(
                c::GRENADE_FUSE as f64,
                (c::GRENADE_FUSE * c::SHORT_FUSE_TIME) as f64,
                ""
            ),
            by(c::SHORT_FUSE_TIME as f64)
        ),
        T::Frag => format!(
            "Grenade burst {} tiles across the radius ({}); {} damage at the centre either way",
            step(
                c::GRENADE_RADIUS as f64,
                (c::GRENADE_RADIUS * c::FRAG_RADIUS) as f64,
                ""
            ),
            by(c::FRAG_RADIUS as f64),
            fig(c::GRENADE_DAMAGE as f64)
        ),
        T::QuickDraw => format!(
            "Seconds a grenade charge takes to come back {} ({})",
            step(
                c::GRENADE_COOLDOWN,
                c::GRENADE_COOLDOWN * c::QUICK_DRAW_COOLDOWN,
                ""
            ),
            by(c::QUICK_DRAW_COOLDOWN)
        ),
        T::Bruiser => format!(
            "Fists and the schword {} damage",
            by(c::BRUISER_MELEE as f64)
        ),
        T::DugInBraced => format!(
            "Odds of slipping a bolt while braced +{}",
            pc(c::DUG_IN_DODGE as f64)
        ),
        T::Deadeye => "A weapon's odds at the far edge of its range become its odds up close: the fall-off between sweet range and range is gone".to_string(),
        T::Rampage => format!(
            "Fire rate {} an enemy downed, up to {} of them ({} in all), until the fight ends",
            by(c::RAMPAGE_FIRE_RATE as f64),
            c::RAMPAGE_STACKS,
            by((c::RAMPAGE_FIRE_RATE as f64).powi(c::RAMPAGE_STACKS as i32))
        ),
        // --- the medic's ---
        T::FieldDressing => format!(
            "Bandaging {} game minutes ({})",
            step(
                bims::task::BANDAGE_MINUTES as f64,
                (bims::task::BANDAGE_MINUTES * c::FIELD_DRESSING_TIME) as f64,
                ""
            ),
            by(c::FIELD_DRESSING_TIME as f64)
        ),
        T::Surgeon => format!(
            "Treating a trauma with a medkit {} game minutes ({})",
            step(
                bims::task::TREAT_MINUTES as f64,
                (bims::task::TREAT_MINUTES * c::SURGEON_TIME) as f64,
                ""
            ),
            by(c::SURGEON_TIME as f64)
        ),
        T::LongBeam => format!(
            "Heal beam reach {} tiles ({})",
            step(
                c::HEAL_BEAM_RANGE as f64,
                (c::HEAL_BEAM_RANGE * c::LONG_BEAM_RANGE) as f64,
                ""
            ),
            by(c::LONG_BEAM_RANGE as f64)
        ),
        T::StrongBeam => format!(
            "Heal beam {} blood an hour ({})",
            step(
                c::HEAL_BEAM_BLOOD as f64,
                (c::HEAL_BEAM_BLOOD * c::STRONG_BEAM_BLOOD) as f64,
                ""
            ),
            by(c::STRONG_BEAM_BLOOD as f64)
        ),
        T::CleanHands => "A trauma this medic treats leaves no lasting mark at all, where an ordinary treatment leaves one behind".to_string(),
        T::SteadyHandsMedic => format!(
            "A treated part comes back to {} -> {} of its health ({})",
            pc(bims::health::TREATED_TO as f64),
            pc((bims::health::TREATED_TO * c::STEADY_HANDS_TREATED).min(1.0) as f64),
            by(c::STEADY_HANDS_TREATED as f64)
        ),
        T::QuickCharge => format!(
            "The surge charges in {} game minutes of beaming ({} the rate)",
            step(
                c::SURGE_CHARGE_MINUTES,
                c::SURGE_CHARGE_MINUTES / c::QUICK_CHARGE_RATE,
                ""
            ),
            by(c::QUICK_CHARGE_RATE)
        ),
        T::LongSurge => format!(
            "A surge lasts {} game minutes ({})",
            step(c::SURGE_MINUTES, c::SURGE_MINUTES * c::LONG_SURGE_TIME, ""),
            by(c::LONG_SURGE_TIME)
        ),
        T::SelfCare => "The medic's own wounds bleed at nothing while the beam is on".to_string(),
        T::DoubleLink => format!(
            "Patients on the beam at once {}, each at the full {} blood an hour",
            step(1.0, c::DOUBLE_LINK_PATIENTS as f64, ""),
            fig(c::HEAL_BEAM_BLOOD as f64)
        ),
        T::GunnerMedic => format!(
            "Fires while beaming, at {} fire rate — he cannot fire at all without it",
            by(c::GUNNER_MEDIC_FIRE_RATE as f64)
        ),
        T::ClosingSurge => "Every open wound on the patient closes the moment the surge ends".to_string(),
        T::MassSurge => format!(
            "A surge covers every crew member within {} tiles of the patient, not the patient alone",
            fig(c::MASS_SURGE_TILES as f64)
        ),
        T::FieldSurgeon => format!(
            "Once a fight: a trauma treated with no medkit at all, in {} game minutes ({})",
            step(
                bims::task::TREAT_MINUTES as f64,
                (bims::task::TREAT_MINUTES * c::FIELD_SURGEON_TIME) as f64,
                ""
            ),
            by(c::FIELD_SURGEON_TIME as f64)
        ),
        // --- the tank's ---
        T::PackMule => format!(
            "Hauling to a site {} loads a trip — {} units",
            step(1.0, c::PACK_MULE_LOADS as f64, ""),
            step(
                world::data::HAUL_LOAD as f64,
                (world::data::HAUL_LOAD * c::PACK_MULE_LOADS) as f64,
                ""
            )
        ),
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
        T::Unmovable => "Never runs from a fight, and loses no pace to low blood at all while his kevlar has anything left".to_string(),
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
        T::HoldFast => "His wounds and traumas bleed at nothing for the whole of a taunt".to_string(),
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
        // --- the commander's ---
        T::WidePresence => format!(
            "Aura reach {} tiles ({})",
            step(
                c::AURA_TILES as f64,
                (c::AURA_TILES * c::WIDE_PRESENCE_RADIUS) as f64,
                ""
            ),
            by(c::WIDE_PRESENCE_RADIUS as f64)
        ),
        T::StrongPresence => format!(
            "Each aura bonus {} deeper: work and aim {}, nerve {}",
            by(c::STRONG_PRESENCE as f64),
            step(
                c::AURA_WORK as f64,
                c::aura_bonus(c::AURA_WORK, c::STRONG_PRESENCE) as f64,
                ""
            ),
            step(
                c::AURA_NERVE as f64,
                c::aura_bonus(c::AURA_NERVE, c::STRONG_PRESENCE) as f64,
                ""
            )
        ),
        T::Haggler => format!(
            "Off a mercenary's fee {}% -> {}%",
            c::HIRE_DISCOUNT_PERCENT,
            c::HAGGLER_DISCOUNT_PERCENT
        ),
        T::Outfitter => "A mercenary he hires arrives wearing the lowest basic piece it was missing, at no extra cost".to_string(),
        T::FocusFire => format!(
            "The squad's hit chance against the enemy it was sent after {}",
            by(c::FOCUS_FIRE_ACCURACY as f64)
        ),
        T::Pincer => format!(
            "Enemies an attack marks at once {}",
            step(1.0, c::PINCER_MARKS as f64, "")
        ),
        T::LongRally => format!(
            "A rally lasts {} game minutes ({})",
            step(c::RALLY_MINUTES, c::RALLY_MINUTES * c::LONG_RALLY_TIME, ""),
            by(c::LONG_RALLY_TIME)
        ),
        T::QuickRally => format!(
            "Seconds between rallies {} ({})",
            step(
                c::RALLY_COOLDOWN,
                c::RALLY_COOLDOWN * c::QUICK_RALLY_COOLDOWN,
                ""
            ),
            by(c::QUICK_RALLY_COOLDOWN)
        ),
        T::SteadyRanks => format!(
            "Bims in the aura bleed at {} the rate",
            by(c::STEADY_RANKS_BLEED as f64)
        ),
        T::DoubleTime => format!(
            "Bims in the aura walk at {} pace",
            by(c::DOUBLE_TIME_PACE as f64)
        ),
        T::Relentless => "An attack's mark holds until that enemy is dead, not merely down — and the squad walks towards it even unseen".to_string(),
        T::Grit => "During a rally, Bims in it lose no pace at all to wounds or traumas".to_string(),
        T::Anchor => format!(
            "Standing still, each aura bonus {} deeper: work and aim {}, nerve {}",
            by(c::ANCHOR_BONUS as f64),
            step(
                c::AURA_WORK as f64,
                c::aura_bonus(c::AURA_WORK, c::ANCHOR_BONUS) as f64,
                ""
            ),
            step(
                c::AURA_NERVE as f64,
                c::aura_bonus(c::AURA_NERVE, c::ANCHOR_BONUS) as f64,
                ""
            )
        ),
        T::Warcry => "A rally covers every friendly Bim in the room, however far off — where it otherwise reaches only those near him".to_string(),
    }
}

/// The same for a **fixed** level, which has no talent to look up: what
/// the level's own ability is worth in numbers, said once. `None`
/// wherever [`level_line`] is `None`.
pub fn level_numbers(class: world::Class, level: u8) -> Option<String> {
    use world::class as c;
    Some(match (class, level) {
        (world::Class::Engineer, 1) => format!(
            "{} sandbag charges, each back {} seconds after it is spent. A laying takes {} game minutes; laid bags hold {} health and are gone at nothing",
            world::deploy::SANDBAG_CHARGES,
            fig(world::deploy::SANDBAG_COOLDOWN),
            fig(world::deploy::DEPLOY_SANDBAG_MINUTES),
            fig(world::deploy::SANDBAG_HEALTH as f64)
        ),
        (world::Class::Engineer, 3) => format!(
            "{} sentry charge, back {} seconds after it is spent — and so one standing at a time, a second laid destroying the first. {} game minutes to lay, {} health, and never out of shots",
            world::deploy::SENTRY_CHARGES,
            fig(world::deploy::SENTRY_COOLDOWN),
            fig(world::deploy::DEPLOY_SENTRY_MINUTES),
            fig(world::deploy::SENTRY_HEALTH as f64)
        ),
        (world::Class::Engineer, 7) => format!(
            "The sentry's rifle takes the tier-2 factors, where it fired at tier 1: {} tiles of range",
            fig(sentry_range(bims::combat::Tier::Two) as f64)
        ),
        (world::Class::Soldier, 1) => format!(
            "Braced, his hit chance is {} — a {} shot becomes {}",
            by(c::BRACE_ACCURACY as f64),
            pc(0.6),
            pc(0.6 * c::BRACE_ACCURACY as f64)
        ),
        (world::Class::Soldier, 3) => format!(
            "{} grenade charges, each back {} seconds after it is thrown: thrown {} tiles, a {}-second fuse, a {}-tile burst doing {} at the centre and half that at the edge",
            c::GRENADE_CHARGES,
            fig(c::GRENADE_COOLDOWN),
            fig(c::GRENADE_RANGE as f64),
            fig(c::GRENADE_FUSE as f64),
            fig(c::GRENADE_RADIUS as f64),
            fig(c::GRENADE_DAMAGE as f64)
        ),
        (world::Class::Soldier, 7) => {
            format!("Every weapon's fire rate {}", by(c::DRILL_FIRE_RATE as f64))
        }
        (world::Class::Medic, 1) => format!(
            "The beam reaches {} tiles and gives back {} blood an hour. {} medkits and {} bandages to start",
            fig(c::HEAL_BEAM_RANGE as f64),
            fig(c::HEAL_BEAM_BLOOD as f64),
            c::MEDIC_START_MEDKITS,
            c::MEDIC_START_BANDAGES
        ),
        (world::Class::Medic, 3) => format!(
            "The surge charges over {} game minutes of beaming a patient that needs it, and runs {}",
            fig(c::SURGE_CHARGE_MINUTES),
            fig(c::SURGE_MINUTES)
        ),
        (world::Class::Medic, 7) => format!(
            "A beamed patient's parts mend at {} the ordinary rate",
            by(c::MENDER_RECOVER as f64)
        ),
        (world::Class::Tank, 1) => format!(
            "Armour worn by him drains at {} the ordinary rate, so a piece absorbs twice as much. Bulwark shelters everybody within {} tiles, at {} his own pace",
            by(c::TANK_DRAIN as f64),
            fig(c::BULWARK_REACH as f64),
            by(c::BULWARK_PACE as f64)
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
        (world::Class::Commander, 1) => format!(
            "The aura reaches {} tiles: work {}, aim {}, and a dying Bim holds its ground {} seconds ({}) where it would otherwise run at once. Hires at {}% off. A squad order reaches {} tiles",
            fig(c::AURA_TILES as f64),
            by(c::AURA_WORK as f64),
            by(c::AURA_AIM as f64),
            fig((c::NERVE_HOLD * c::AURA_NERVE) as f64),
            by(c::AURA_NERVE as f64),
            c::HIRE_DISCOUNT_PERCENT,
            fig(c::SQUAD_RANGE as f64)
        ),
        (world::Class::Commander, 3) => format!(
            "A rally puts every friendly Bim it reaches at aim {} and stops any of them running; it runs {} game minutes, with {} seconds between",
            by(c::RALLY_AIM as f64),
            fig(c::RALLY_MINUTES),
            fig(c::RALLY_COOLDOWN)
        ),
        (world::Class::Commander, 7) => format!(
            "A squad order reaches the whole room, where it otherwise reaches {} tiles",
            fig(c::SQUAD_RANGE as f64)
        ),
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
pub const REPAIR: &str = "Repair";
pub const REPAIR_TIP: &str = "Mend the damaged piece in the first slot for a bar of metal: the armourer's own session at the bench, ten minutes.";
/// What a deployable on the deck is called, by `world::DeployKind` code,
/// with what it has left.
pub const DEPLOYABLE_NAMES: [&str; 2] = ["Sandbags", "Sentry"];
pub fn deployable_line(d: &world::Deployable) -> String {
    let name = DEPLOYABLE_NAMES
        .get(d.kind.code() as usize)
        .copied()
        .unwrap_or("Deployable");
    match d.kind {
        world::DeployKind::Sandbags => format!("{name} — {:.0} left", d.health),
        world::DeployKind::Sentry => format!("{name} — {:.0} health", d.health),
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
/// And for a beam and a surge (feature 76).
pub fn beam_refused(why: world::Refusal) -> String {
    format!("Cannot beam: {}.", refusal(why))
}
pub fn surge_refused(why: world::Refusal) -> String {
    format!("Cannot surge: {}.", refusal(why))
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
pub const ORDERS_TIP: &str = "The crew nobody is steering keep to your side and fight for themselves when they see an enemy. F puts an attack banner down for them to fight their way to; T calls them back to the ship; either key again lets them follow you again. Nobody leaves a fight aboard the ship.";
/// The commander's rows on the crew panel (feature 78): what the squad
/// is under, and the rally with its cooldown.
pub const SQUAD_NONE: &str = "Squad: free";
pub const SQUAD_TIP: &str = "The squad is every crew member nobody is steering. E sends it at the enemy under the pointer, X calls it back to a tile, Z has it hold where it stands; the same key again lets it go. Your own Bim is never ordered by it.";
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
        return format!("Rally at level {}", world::class::RALLY_LEVEL);
    }
    if left > 0.0 {
        return format!("Rallying — {left:.0} min left");
    }
    if cooldown > 0.0 {
        format!("Rally ready in {cooldown:.0} s")
    } else {
        "Rally ready".to_string()
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
pub const BRACED_TIP: &str = "Holding a line: no errands, no running, steadier shooting. E stands easy; so does any order that moves them.";
pub const STAND_EASY: &str = "Standing easy";
/// The medic's rows on the crew panel (feature 76): who the beam holds,
/// and how charged the surge is.
pub const BEAM_ON: &str = "Beaming";
pub const BEAM_OFF: &str = "No beam";
pub const BEAM_TIP: &str = "The heal beam holds a crewmate up: their wounds and traumas stop bleeding and their blood comes back. E over a crew member links it; E again, or on nothing, unlinks. The medic may walk, and fires nothing while it is on.";
pub const SURGING: &str = "Surging";
pub fn beam_line(patients: &[String]) -> String {
    match patients {
        [] => BEAM_OFF.to_string(),
        [one] => format!("{BEAM_ON} {one}"),
        many => format!("{BEAM_ON} {}", many.join(" and ")),
    }
}
pub fn surge_line(charge: f32, level_enough: bool) -> String {
    if !level_enough {
        return format!("Surge at level {}", world::class::SURGE_LEVEL);
    }
    if charge >= 1.0 {
        "Surge charged".to_string()
    } else {
        format!("Surge {:.0}%", charge * 100.0)
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

/// Why a blueprint will not go where the pointer is, from
/// `World::can_place_site`: the ship is moving, the rules refuse the tile
/// (an `EditError`, said the designer's way), or the ship would then have
/// a fault it has not got (an `IssueCode`, likewise).
pub fn site_refusal_line(why: world::SiteRefusal) -> String {
    match why {
        world::SiteRefusal::UnderWay => "Nothing is built while the ship is moving.".into(),
        world::SiteRefusal::WontFit(code) => edit_line(code).into(),
        world::SiteRefusal::Fault(code) => issue_line(code)
            .map(|line| format!("It would go, but then: {line}"))
            .unwrap_or_else(|| "It would leave the ship with a fault.".into()),
        world::SiteRefusal::NotResearched(node) => format!(
            "The crew do not know how to build that yet — research {}.",
            node_name(node)
        ),
    }
}

/// How much of what a site is made of has reached it, in words: "2 of 2
/// metal, 0 of 1 components".
pub fn site_progress(site: &world::BuildSite, design: &shipdesign::ShipDesign) -> String {
    site.recipe(design)
        .iter()
        .map(|&(id, units)| {
            format!(
                "{} of {units} {}",
                site.delivered[id as usize].min(units),
                resource_name(id).to_lowercase()
            )
        })
        .collect::<Vec<_>>()
        .join(", ")
}

/// What the ship is doing, indexed by `world::ShipState::code`.
pub const STATE_NAMES: [&str; 7] = [
    "Docked",
    "Holding",
    "Under way",
    "Casting off",
    "Undocking",
    "Docking",
    "Charging",
];

/// The machines (feature 83): a wave landing, and the last of them
/// destroyed.
pub const DROID_REINFORCEMENTS: &str = "Another wave of machines has landed.";
pub const DROID_CLEARED: &str = "The last of the machines is down.";

/// What each of the three is called, indexed by
/// `bims::droid::DroidKind::code`; `0` is no machine at all. A droid
/// has no name of its own — it is a machine, not somebody — so the log
/// says its kind.
pub const DROID_NAMES: [&str; 4] = ["—", "Husk", "Trooper", "Warden"];

pub fn droid_name(code: u32) -> &'static str {
    DROID_NAMES.get(code as usize).copied().unwrap_or("Machine")
}

/// Which part of a trip the ship is in, indexed by `flight::Phase`.
pub const PHASE_NAMES: [&str; 5] = ["Aligning", "Burning", "Turning", "Braking", "Holding"];

/// What happened, as a sentence. `None` for an event with nothing to say —
/// there are none today, and the arm is here so a new event is a missing
/// line rather than a blank row.
pub fn event_line(event: WorldEvent) -> Option<String> {
    use health::HealthEvent;
    let who = |slot: u32| crew_name(slot);
    Some(match event {
        WorldEvent::Departed { .. } => "Under way.".into(),
        WorldEvent::Arrived { station: Some(id) } => format!("Docked at station {id}."),
        WorldEvent::Arrived { station: None } => "Holding station.".into(),
        WorldEvent::Aborted { .. } => "Stopping.".into(),
        WorldEvent::PlanFailed { error, .. } => {
            format!("Cannot fly there — {}", plan_error(error.code()))
        }
        WorldEvent::Discovered { .. } => "Something new on the scanner.".into(),
        WorldEvent::FrameChanged { frame } => {
            if frame.code() == 0 {
                "Out into open space.".into()
            } else {
                "Alongside.".into()
            }
        }
        WorldEvent::Traded {
            resource, units, ..
        } if units >= 0 => {
            format!("{units} {} aboard.", resource_name(resource).to_lowercase())
        }
        WorldEvent::Traded {
            resource, units, ..
        } => {
            format!(
                "{} {} sold.",
                -units,
                resource_name(resource).to_lowercase()
            )
        }
        WorldEvent::Refused { why, .. } => {
            format!("That could not be done — {}.", refusal(why))
        }
        WorldEvent::CastingOff { .. } => "Casting off: everybody back aboard.".into(),
        WorldEvent::Undocking { .. } => "Clear of the berth.".into(),
        WorldEvent::Docking { station } => format!("Coming alongside station {station}."),
        WorldEvent::Crafted { recipe } => format!("Made {}.", made_name(recipe)),
        WorldEvent::CraftLost { recipe } => {
            format!(
                "Nothing made: the materials for {} were gone.",
                made_name(recipe)
            )
        }
        WorldEvent::Mined { rock, ore, galvum } => {
            if rock == 0 && ore == 0 && galvum == 0 {
                "Back from outside with nothing.".into()
            } else {
                let mut got: Vec<String> = Vec::new();
                if rock > 0 {
                    got.push(format!("{rock} rock"));
                }
                if ore > 0 {
                    got.push(format!("{ore} ore"));
                }
                if galvum > 0 {
                    got.push(format!("{galvum} galvum"));
                }
                format!("Back from outside with {}.", got.join(", "))
            }
        }
        WorldEvent::Health { who: w, event } => match event {
            HealthEvent::RadiationDetected => {
                format!("{} has picked up a dose of radiation.", who(w))
            }
            HealthEvent::CriticalDose => {
                format!("{}'s dose is critical — it is doing damage.", who(w))
            }
            HealthEvent::RadiationSickness => format!("{} has radiation sickness.", who(w)),
            HealthEvent::SicknessSubsided => {
                format!(
                    "{}'s sickness has subsided; the dose is still critical.",
                    who(w)
                )
            }
            HealthEvent::BelowCritical => {
                format!("{}'s dose is below critical again.", who(w))
            }
            HealthEvent::DoseCleared => format!("{}'s dose is clear.", who(w)),
            HealthEvent::CancerOnset => format!("{} has cancer.", who(w)),
            HealthEvent::CancerAdvanced => format!("{}'s cancer has advanced.", who(w)),
            HealthEvent::CancerTerminal => format!("{}'s cancer is terminal.", who(w)),
            HealthEvent::Died => format!("{} has died.", who(w)),
        },
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
        // A shot that landed on one of the crew: which part, and the fact
        // that every hit bleeds until it is dressed — the line is what
        // sends a player to the bandages.
        WorldEvent::CrewHit { who: w, part } => {
            format!(
                "{} was hit in the {} — and is bleeding.",
                who(w),
                body_part_name(part)
            )
        }
        WorldEvent::CrewDown { who: w } => format!("{} is dead.", who(w)),
        // A part shot to nothing: the dying state it rolled, and the fact
        // that a medkit in a crewmate's hands is the only way out of it —
        // the line is what sends a player to the armoury.
        WorldEvent::CrewDying { who: w, trauma } => {
            format!(
                "{} is dying — {}. {} Another crew member has to treat it with a medkit.",
                who(w),
                trauma_name(trauma).to_lowercase(),
                trauma_line(trauma)
            )
        }
        WorldEvent::CrewTreated { who: w, trauma } => {
            let after = trauma_after(trauma);
            format!(
                "{} was treated for the {}{}",
                who(w),
                trauma_name(trauma).to_lowercase(),
                if after.is_empty() {
                    ".".to_string()
                } else {
                    format!(" — {after}")
                }
            )
        }
        WorldEvent::Equipped { who: w, kind } => {
            format!(
                "{} put on the {}.",
                who(w),
                armour_name(Some(kind)).to_lowercase()
            )
        }
        WorldEvent::Stowed { who: w } => format!("{} put it away.", who(w)),
        // A piece at nothing is still worn and does nothing from now on:
        // the line is what sends a player to the armoury for another.
        WorldEvent::PieceBroke { who: w, kind } => {
            format!(
                "{}'s {} is broken — it stops nothing now.",
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
        // One thing off a body into the pack; whose body by its kind,
        // `LootSource::code` — a crewmate's, or one of the station's
        // people's.
        WorldEvent::Looted {
            who: w,
            source_kind,
        } => {
            format!(
                "{} took something off {}.",
                who(w),
                if source_kind == 1 {
                    "one of the station's people"
                } else {
                    "a crewmate"
                }
            )
        }
        WorldEvent::Hired { who: w } => {
            format!("{} signed on — a hired hand, paid by the month.", who(w))
        }
        WorldEvent::Executed { who: w, .. } => {
            format!(
                "{} finished off one of the station's people where it lay.",
                who(w)
            )
        }
        WorldEvent::MercenaryPaid { who: w, fee } => {
            format!(
                "{}'s month came round: {} paid.",
                who(w),
                crate::format::euros(fee)
            )
        }
        WorldEvent::MercenaryLeft { who: w } => format!(
            "{}'s month came round and there was not the money — a hired hand unpaid walks off at the next berth.",
            who(w)
        ),
        WorldEvent::KeyTaken { who: w } => {
            format!("{} took the research key off the station's desk.", who(w))
        }
        WorldEvent::Unlocked { node } => format!(
            "The key was consumed at the research desk: {} is open to research.",
            node_name(node)
        ),
        WorldEvent::ResearchBegun { node } => {
            format!("The AI is researching {}.", node_name(node))
        }
        WorldEvent::Researched { node } => format!("Researched: {}.", node_name(node)),
        WorldEvent::UpgradeBegun { resource, tier } => format!(
            "Two of {} went onto the workbench: one will come off at {}.",
            resource_name_by_code(resource),
            tier_name(tier)
        ),
        WorldEvent::Upgraded { resource, tier } => format!(
            "A {} came off the workbench at {}.",
            resource_name_by_code(resource).to_lowercase(),
            tier_name(tier)
        ),
        WorldEvent::Charging { slot, star } => {
            format!("{} charges the hyperdrive for star {star}.", who(slot))
        }
        WorldEvent::Jumped { star } => format!("Jumped. The ship is in the system of star {star}."),
        WorldEvent::JumpFailed => "The hyperdrive did not fire: nothing working to fire.".into(),
        WorldEvent::Infested { star } => format!(
            "The machines have this system. Every station round star {star} is theirs: nobody left aboard, nothing to trade, nobody to hire."
        ),
        WorldEvent::Landing { .. } => "Coming down onto the planet.".into(),
        WorldEvent::Landed { .. } => "Landed. The settlement is beside the pad.".into(),
        WorldEvent::LiftedOff { .. } => "Lifting off.".into(),
        WorldEvent::Brownout => {
            "Brownout: the batteries are flat and the ship draws more than it makes. The lamps are out, the bay and the benches have stopped, and the cold store is warming."
                .into()
        }
        WorldEvent::PowerRestored => "Power restored.".into(),
        WorldEvent::FoodSpoiled { units } => {
            format!("{units} of the food in the cold store spoiled for want of power.")
        }
        WorldEvent::RaidContact { boarders, minutes } => format!(
            "Raiders. A hostile ship is on the radar and closing, incoming in {}, with {boarders} aboard. Everybody back to 1×.",
            crate::format::in_words(minutes as f64)
        ),
        WorldEvent::RaidBoarded { boarders } => format!(
            "The raider is alongside: the airlock is locked in its face, and {boarders} boarders are forcing it."
        ),
        WorldEvent::RaidBreached { boarders } => {
            format!("The airlock gave: {boarders} boarders are coming through it.")
        }
        WorldEvent::RaidCancelled => "The raider lost the ship, and gave up.".into(),
        WorldEvent::RaidRepelled => {
            "The boarders are all down. The raider is a derelict tied to the ship: loot it, and cast off to be rid of it."
                .into()
        }
        WorldEvent::CrewLost => "Nobody of the crew is standing. The run is over.".into(),
        WorldEvent::Plundered { who: w, units } => match units {
            1 => format!("{} took a thing off the enemy's shelf.", who(w)),
            n => format!("{} took {n} off the enemy's shelf.", who(w)),
        },
        WorldEvent::ResearchQueued { node } => {
            format!("Queued for research: {}.", node_name(node))
        }
        WorldEvent::ResearchDropped { node } => {
            format!("Off the research queue: {}.", node_name(node))
        }
        WorldEvent::LevelUp {
            who: w,
            class,
            level,
        } => match world::class::is_pick_level(level as u8) {
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
            _ => "A sentry is shot to pieces.".into(),
        },
        WorldEvent::Repaired { kind } => format!(
            "The workbench mended {}.",
            armour_name(bims::combat::ArmourKind::from_code(kind)).to_lowercase()
        ),
        WorldEvent::Braced { who: w, on: true } => format!("{} braced.", who(w)),
        WorldEvent::Braced { who: w, on: false } => format!("{} stood easy.", who(w)),
        WorldEvent::Thrown { who: w } => format!("{} threw a grenade.", who(w)),
        WorldEvent::Beamed {
            who: w,
            patient: Some(p),
        } => format!("{} beamed {}.", who(w), who(p)),
        WorldEvent::Beamed { who: w, .. } => format!("{}'s beam is off.", who(w)),
        WorldEvent::Surged { who: w } => format!("{} surged.", who(w)),
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
    })
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

/// The dying states, indexed by `bims::health::Trauma::code`: what a part
/// shot to nothing turned into. Pinned against `Trauma::ALL` below.
pub const TRAUMA_NAMES: [&str; 10] = [
    "Heavy concussion",
    "Skull fracture",
    "Cranial trauma",
    "Internal bleeding",
    "Broken ribs",
    "Severe chest trauma",
    "Fractured femur",
    "Shattered knee",
    "Crushed right leg",
    "Crushed left leg",
];

/// What each does **until it is treated**, as a sentence.
pub const TRAUMA_LINES: [&str; 10] = [
    "A quarter slower walking and working.",
    "Losing 10 blood every quarter hour.",
    "Half as fast walking and working, and losing 5 blood every quarter hour.",
    "Losing 10 blood every quarter hour, and nothing shows.",
    "A quarter slower walking and working.",
    "Losing 5 blood every quarter hour and walking at half pace.",
    "Losing 10 blood every quarter hour.",
    "Can barely move — a quarter of its pace.",
    "The leg is lost, for good, and the stump is losing 10 blood every quarter hour.",
    "The leg is lost, for good, and the stump is losing 10 blood every quarter hour.",
];

/// What each leaves **after** a medkit, as the tail of a sentence, or
/// nothing.
pub const TRAUMA_AFTER: [&str; 10] = [
    "a quarter slower walking and working for the next two days.",
    "",
    "half as fast walking and working for the next day.",
    "",
    "a quarter slower walking and working for the next two days.",
    "walking at half pace for the next day.",
    "",
    "a quarter slower walking for the next two days.",
    "a fifth slower walking, for ever.",
    "a fifth slower walking, for ever.",
];

pub fn trauma_name(code: u32) -> &'static str {
    TRAUMA_NAMES.get(code as usize).copied().unwrap_or("Trauma")
}

pub fn trauma_line(code: u32) -> &'static str {
    TRAUMA_LINES.get(code as usize).copied().unwrap_or("")
}

/// The same, short, for the panel's line while it lasts: what it costs,
/// without the how long — the panel counts that down itself.
pub const TRAUMA_LASTING: [&str; 10] = [
    "a quarter slower",
    "",
    "half as fast",
    "",
    "a quarter slower",
    "walking at half pace",
    "",
    "a quarter slower walking",
    "",
    "",
];

pub fn trauma_lasting(code: u32) -> &'static str {
    TRAUMA_LASTING.get(code as usize).copied().unwrap_or("")
}

pub fn trauma_after(code: u32) -> &'static str {
    TRAUMA_AFTER.get(code as usize).copied().unwrap_or("")
}

/// What a recipe makes, in words: "4 components".
pub fn made_name(recipe: u32) -> String {
    match shipdesign::RECIPES.get(recipe as usize) {
        Some(r) => format!(
            "{} {}",
            r.output.1,
            resource_name(r.output.0).to_lowercase()
        ),
        None => "something".into(),
    }
}

/// What each kind of body is called. Indexed by `worldgen::BodyKind`.
pub const BODY_KIND_NAMES: [&str; 4] = ["Rocky planet", "Gas giant", "Ice world", "Asteroid belt"];

/// What a rock tile of a mining site is made of, by `world::Rock` code.
pub const ROCK_NAMES: [&str; 3] = ["Rock", "Iron ore", "Galvum"];

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

/// The needs, in the order the room indexes them.
pub const NEED_NAMES: [&str; 6] = [
    "Rest",
    "Food",
    "Restroom",
    "Surroundings",
    "Socializing",
    "Washing",
];

/// Which of those get a trigger in the schedule tab.
pub const TRIGGER_NEEDS: [u32; 3] = [0, 1, 5];

/// What each bar is, for the tooltip on its row.
pub fn need_tip(need: usize) -> &'static str {
    match need {
        0 => {
            "Runs down all day. The timetable is what sends the Bim to bed on an ordinary night — the level only decides whether a scheduled night is worth taking. The trigger under the timetable is the floor beneath that: past it the Bim turns in whatever the hour, unless a meal or the heads comes first."
        }
        1 => {
            "Past its trigger — a tenth, until you move it — the Bim goes and cooks itself a meal. Empty for eight hours and malnutrition sets in; a day of it is fatal."
        }
        2 => {
            "Under 10% the Bim takes itself to the toilet. If it cannot — shut in, under orders — it fidgets, then risks wetting itself, and an hour after the bar empties it has an accident. A meal cooked in a dirty galley — every tile within two of the hob with a mess on it, a wetting or worse, is a one-in-five chance — is food poisoning: for two days this runs three times as fast and empties straight into an accident."
        }
        3 => {
            "Not a clock like the others: it follows the deck within three tiles of the Bim — the mess on it, lifted by any plant or picture in reach — and whatever the Bim has on itself. At nothing it treads carefully, then keeps away from the mess, then is sick in it every half hour."
        }
        4 => {
            "The one need that wants another Bim rather than a fixture. Past its trigger the Bim goes and finds the other one, and they stand and talk about whatever they have been doing. This bar is the comfortable end of it; what matters is the count of days underneath, because going without runs on a far longer clock that starts once this bar is empty — six days of that and a Bim is low and slow, eight and it sits down on the deck, ten and it starts hurting itself."
        }
        5 => {
            "A day's grime, on the clock like food: it runs down over the waking day and the Bim wants a shower about once a day. Past its trigger it goes and takes one, where the ship has a shower — a room without one is a Bim that goes on wanting a wash and nothing worse. Separate from Surroundings, which is the deck around it."
        }
        _ => "",
    }
}

pub const HEALTH_TIP: &str = "The head, the body and the legs add up to this bar: a shot takes its damage off whichever it lands on, and the head or the body at nothing is death. The legs at nothing is a leg lost. Every hit opens a wound that bleeds until it is dressed — the Blood bar underneath — and below three quarters of its blood the Bim is slow, below half it is out cold where it stands — nothing aims at a body that far gone — and at nothing it is dead. Only the worst stage of malnutrition costs health of itself, and eating properly walks it back. The lines underneath name whatever is wrong. The blue on the end of a bar is armour: a worn piece adds what it has left to the part, takes every hit first — its protection comes off the damage before anything else, and the rest drains the piece — and only what the piece cannot take reaches the body. At nothing it is broken: still worn, doing nothing, worth nothing put away — discard it and make another.";

// --- what is killing it -----------------------------------------------------
//
// The peril block under the health bar: the one thing on the panel that
// answers "what is this Bim dying of *now*", with the rate it is dying
// at and how long it has at that rate. The words are here; the sums are
// `crew::perils`, off the room's own constants.

/// The headline over the block. The first is for a body in a dying
/// state or starving — the red one, the one the cross on the deck
/// marks, the one a medkit or a meal is the answer to. The second is
/// for a body that is only losing blood through wounds a bandage
/// closes: the same block and the same countdown, in the caution
/// colour, because a scratch that would empty it in ten hours is worth
/// a number and not a fright.
pub const PERIL_HEAD: &str = "DYING OF";
pub const PERIL_HEAD_HURT: &str = "LOSING";
/// A body losing blood faster than it makes it: the wounds and the
/// untreated traumas together.
pub const PERIL_BLEEDING: &str = "Blood loss";
/// The last stage of malnutrition, which is the only one that costs
/// health of itself.
pub const PERIL_STARVING: &str = "Starvation";
/// What each of them wants done about it.
pub const PERIL_BLEED_MEDKIT: &str = "A crewmate with a medkit, then a bandage on the rest.";
pub const PERIL_BLEED_BANDAGE: &str = "A bandage on each part closes the wounds.";
pub const PERIL_STARVE_FIX: &str = "It has to eat, and soon.";
/// The line under a body that is in a dying state but losing nothing —
/// a concussion, broken ribs, a shattered knee. It will not die of it,
/// and saying so is the point of the line.
pub const PERIL_STABLE: &str = "Not losing blood — it will hold until a medkit reaches it.";
/// How long it has left, when the rate says.
pub fn peril_left(span: &str) -> String {
    format!("{span} left at this rate")
}
/// One row of the block: where the loss is coming from, and how much.
pub fn peril_from(what: &str, an_hour: f32) -> String {
    format!("{what} · {} an hour", (an_hour.round() as i64))
}
/// What open wounds on one part are called in that list.
pub fn peril_wounds(part: &str, n: u32) -> String {
    if n == 1 {
        format!("{part} · 1 open wound")
    } else {
        format!("{part} · {n} open wounds")
    }
}
/// The total across the top of the block.
pub fn peril_rate(an_hour: f32) -> String {
    format!("{} blood an hour", an_hour.round() as i64)
}
/// How fast starvation is taking the health bar down.
pub fn peril_drain(a_day: f32) -> String {
    format!("{} health a day", a_day.round() as i64)
}

/// What a dead body says it died of. Nothing records a cause of death,
/// so this reads it off the body the same way a person would: no blood
/// left is one death, and the head and the body both at nothing with no
/// trauma on either is the other — starved if it was starving, and
/// otherwise its own hand, which is the only thing left that empties
/// both parts and leaves them clean.
pub const DEATH_BLED_OUT: &str = "Bled out.";
pub const DEATH_STARVED: &str = "Starved.";
pub const DEATH_GAVE_UP: &str = "Gave up.";

pub const PERIL_TIP: &str = "What is taking this Bim down right now, and how long it has at that rate. Blood runs out through every open wound — ten an hour each — and through every untreated trauma that bleeds, and at nothing left the Bim is dead; a medkit ends a trauma, a bandage closes the wounds on a part. Extreme malnutrition is the other one: it takes health off the head, the body and the legs until the head and the body are both at nothing. The countdown assumes nothing changes — a bandage, a medkit or a meal moves it at once.";

pub const BANDAGE_TIP: &str = "A bandage closes every wound on one part of a body — the head, the body or the legs — and stops the bleeding there. Order one here, or right-click a Bim on the deck, and the crew member you steer walks over and dresses it, ten minutes with hands on. The dressing comes out of that Bim's own pack: five to a box, and the Management tab says how many each of the crew keeps on them. Bandages are made at the drug lab out of fibre, or bought where a station sells them.";

/// Feature 87: the dressings are in the pack, and a box of them has a
/// row of its own.
pub const NO_BANDAGE: &str = "no bandages in the pack — the Management tab says how many to carry";
pub const BANDAGE_ALL_HINT: &str =
    "one dressing a wounded part, the worst first and the rest queued behind it";
pub const BANDAGE_ALL_ROW: &str = "Bandage all wounds";
pub const BANDAGE_ALL_WHOLE: &str = "nothing open on this Bim";
/// The Management tab's row for how many dressings each of the crew is
/// to carry, and where they are kept.
pub const BANDAGES_ROW: &str = "Bandages";
pub const BANDAGES_KEPT_IN: &str = "Each pack";
pub const BANDAGES_TARGET_TIP: &str = "How many dressings every crew member keeps in their own pack. Out of combat they top themselves up out of the hold, one at a time — nobody walks for it — and a wound is bound with a dressing out of the binder's own pack. Five go in one box, and a box takes two cells by two.";

/// Why a Bandage row is greyed when the helper cannot do it: the crew
/// member you steer is dead, out cold, or outside in a suit.
pub const HELPER_OUT: &str = "not from where the Bim is";

/// What the log says when a right-click on a gun on the deck cannot send
/// the Bim for it: dead, out cold, outside, or the gun already gone.
pub const PICK_UP_REFUSED: &str = "Can't pick that up from here.";

/// The greyed line under a fixture menu's rows while the Bim has
/// something on: a row clicked with Shift held waits its turn behind it
/// rather than taking over (feature 69), and so does a right-click on the
/// deck.
pub const SHIFT_LATER: &str = "Shift-click: afterwards";
pub const SHIFT_LATER_HINT: &str =
    "a row or a spot on the deck given with Shift waits its turn behind what the Bim is on";

/// Why a Bandage row is greyed for a patient outside in a suit: nobody
/// can walk to it there.
pub const PATIENT_OUT: &str = "not while the patient is outside — it comes in first";

pub const FIBRE_TIP: &str = "Fibre is the one crop nobody eats: a day in a tray, and two of it make a bandage at the drug lab. A target here has the bay grow it like greens and soy; 0 means never.";

pub const AUTONOMY_TIP: &str = "Off, the Bim starts nothing by itself — no meals, no sleep, no trips to the toilet — but still does everything it is told. The levels carry on moving either way.";
/// The Management tab's other tick box: the workbench's upgrade.
/// The end of the run (`screens::game::over`): the title, the line under
/// it, and the way back.
pub const OVER_TITLE: &str = "The crew are down";
pub const OVER_LINE: &str = "Nobody of the crew is standing. The run is over.";
pub const OVER_BACK: &str = "Back to the menu";

pub const UPGRADE_LABEL: &str = "Combine matching gear";
pub const UPGRADE_TIP: &str = "Ticked, whoever is free carries two of a kind at the same tier — two pistols, two helms — from the lockers to the workbench's two slots one at a time, presses Upgrade for you, and a day of work later carries the one that comes off a tier up back to the lockers: a quarter more damage and accuracy for a weapon, half again the health and protection for armour, and at tier three more range or a chance to dodge. Unticked, the bench is yours: put a pair on it from the pack and press the button in its window. The hours done are kept whoever is at the bench. Nothing is upgraded, ticked or not, until the Upgrades node of the research tree is known — tier two, behind a tier-two key off a hostile station's desk.";

/// The line under it while something is on the bench: what, to which
/// tier, and how far — or that it is done and waiting in the output slot.
pub fn upgrade_line(resource: ResourceId, tier: u32, done: u32, of: u32, waiting: bool) -> String {
    let name = resource_name(resource);
    if waiting {
        format!("{name} at {} is ready on the workbench", tier_name(tier))
    } else {
        format!(
            "Upgrading {} to {} — {done} of {of} hours",
            name.to_lowercase(),
            tier_name(tier)
        )
    }
}

/// The workbench's window: its title, the button, and the tip on the `?`.
pub const BENCH_WINDOW: &str = "Workbench";
pub const UPGRADE_BUTTON: &str = "Upgrade";
pub const BENCH_TIP: &str = "Two of a kind at the same tier go in the two slots on the left — two pistols, two helms, below tier three — and Upgrade starts a day of work on them; the one that comes out, a tier up, appears in the slot on the right. Ctrl-click a thing in the pack to put it on the bench while this window is up, and Ctrl-click a slot to take what is in it back. Tier two is drawn on blue, tier three on gold, wherever a thing lies. With Combine matching gear ticked on the Management tab the crew carry the pairs over and press the button themselves.";
/// The words between the slots while the day's work is on.
pub fn bench_work_line(done: u32, of: u32) -> String {
    format!("{done} of {of} h")
}

pub const TARGET_TIP: &str = "A target is a standing order: keep at least this many in the cold store. Whenever the count falls below it, the work goes on the crew's list by itself and whoever is free does it — for vegetables and tofu, planting a tray in the hydroponic bay (greens, or soy for tofu) and carrying the harvest to the store; for stew, cooking a pot on the hob out of one vegetable and one block of tofu and putting it on the shelf. Once the count is back at the target the job comes off the list, and above it nothing is grown or cooked. 0 means never. How soon it gets done is the Planting and Cooking priorities on the Work tab.";

pub const TRIGGER_TIP: &str = "How low a need may get before the Bim breaks off and does something about it: past the mark on a row it takes itself to bed, or goes and cooks, on its own account. The timetable above says when it may sleep; the Rest threshold is the floor under that — past it the Bim turns in whatever the hour, unless a meal or the heads comes first. Untick one and that need still runs down and still tells on the Bim; only the errand stops.";

pub const SPEED_TIP: &str = "How fast the simulation runs. At 24x a whole game day goes by in about a minute, and at 48x in half of one.";

pub const BUILD_TIP: &str = "Lay out a part and the crew build it, out of what is on the shelves: whoever is free carries what it is made of to the site a load at a time, then stands beside it and puts it together — Hauling and Building on the Work tab say how soon. A site beyond the hull is reached in a suit, through the airlock. Nothing is built while the ship is moving, and the ship stays put while something is being built.";

pub const ITEMS_TIP: &str = "What is aboard, by where it is kept: ore, metal and components on the shelves; vegetables and tofu in the cold store; armour, weapons and medical things in the lockers. The food is what the crew can eat now — the cold store is refilled from the manifest at every dock. Click the armoury, a shelf or the cold store on the deck to reach into it.";

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

/// How a Bim says each thing it remembers, by the code from
/// `crates/game/src/memory.rs`. First person, because it is its diary. `d`
/// is the one detail that came with the entry. Only things that actually
/// went wrong are in here, because only those are written down; an entry
/// with no line here is dropped from the page rather than padded out.
pub fn memory_line(what: u32, d: u32) -> Option<String> {
    Some(match what {
        20 => {
            if d == 1 {
                "Could not hold it. I would rather not talk about it.".into()
            } else {
                "Did not quite make it to the heads.".into()
            }
        }
        21 => "Was sick on the deck.".into(),
        22 => "Dropped off where I was standing.".into(),
        23 => match d {
            1 => "Getting hungry. Properly hungry.",
            2 => "I have not eaten in a long time.",
            3 => "I am starving. I can feel it in my hands.",
            _ => "Going hungry.",
        }
        .into(),
        24 => match d {
            1 => "Tired. I should sleep.",
            2 => "I have not slept in far too long.",
            3 => "I cannot keep my eyes open.",
            _ => "Going without sleep.",
        }
        .into(),
        25 => format!("Saw {} have an accident.", crew_or(d, "one of the crew")),
        26 => format!("Saw {} being sick.", crew_or(d, "one of the crew")),
        27 => {
            if d >= 7 {
                "Nobody has spoken to me in a week.".into()
            } else if d >= 5 {
                "The quiet is starting to get to me.".into()
            } else {
                "Feeling low. It has been a few days since anyone said anything.".into()
            }
        }
        28 => "Sat down on the deck and could not get up for a while.".into(),
        29 => format!("Hurt myself. {d} points of it."),
        30 => format!("{} died today.", crew_or(d, "One of the crew")),
        31 => "Something I ate. Cooked in that galley — I have never been so ill.".into(),
        _ => return None,
    })
}

fn crew_or(who: u32, fallback: &str) -> String {
    given_name(who).unwrap_or_else(|| {
        CREW_NAMES
            .get(who as usize)
            .map(|s| s.to_string())
            .unwrap_or_else(|| fallback.to_string())
    })
}

/// What a Bim says it is talking about, by the code from `chat_topic`.
/// Third person and short: this goes in a bubble over its head.
///
/// Two code spaces, and they do not overlap: small talk comes back as a
/// `JOB_` code from 1, and the things that happened *to* it as a
/// `memory::What` code from 20.
pub fn chat_topic(code: u32) -> &'static str {
    match code {
        1 => "cooking",
        2 => "the cooker",
        3 => "that nap",
        4 => "the night",
        5 => "the heads",
        6 => "the fridge",
        7 => "that door",
        8 => "the lock",
        9 => "the dishwasher",
        10 => "cooking",
        11 => "the bay",
        12 => "leftovers",
        13 => "the sweeping",
        20 => "an accident",
        21 => "being sick",
        22 => "dropping off",
        23 => "being hungry",
        24 => "being tired",
        25 => "what happened",
        26 => "what happened",
        27 => "how it has been",
        28 => "a bad day",
        29 => "a bad day",
        30 => "the one who died",
        31 => "a bad meal",
        _ => "nothing much",
    }
}

/// What `spot_at` says is under the pointer. Must match the `SPOT_` codes
/// in `crates/game/src/room.rs`.
pub const SPOT_NAMES: [&str; 23] = [
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
    "Bathroom door",
    "Deck plating",
    "Broom locker",
    "Door",
    "Helm",
    "Workbench",
    "Suit locker",
    "Shower",
    "Research desk",
];

/// Which spots are deck: the only ones a mess can be lying on.
pub const DECK_SPOTS: [u32; 2] = [1, 15];

/// The spots that are only a word for *where* the pointer is — outside,
/// deck, a bulkhead — rather than a thing the room has a picture of. Aboard
/// the ship every part the room does not draw reads as one of these, so the
/// ship's readout names those off the design instead.
pub const PLAIN_SPOTS: [u32; 4] = [0, 1, 2, 15];

/// What is on the deck there, by the code from `spot_mess`. Ordered least
/// bad first, the same as `filth::Mess`.
pub const MESS_NAMES: [&str; 6] = ["", "Grime", "Wet", "Soiled", "Vomit", "Blood"];

/// The three stages of going without sleep.
pub const DROWSINESS: [&str; 4] = [
    "",
    "Sleepy — fumbling, errands a quarter longer",
    "Sleep deprived — errands half as long again",
    "Past it — errands twice as long, and dropping off on its feet",
];

/// The three stages of going without food.
pub const CONDITIONS: [&str; 4] = [
    "",
    "Mild malnutrition — moving slowly",
    "Malnutrition — slower, and tiring twice as fast",
    "Extreme malnutrition — losing health",
];

/// How badly the Bim needs the toilet.
pub const URGES: [&str; 4] = [
    "",
    "Needs the toilet — fidgeting",
    "Needs the toilet badly — may not make it",
    "Bursting — an accident within the hour",
];

/// How far gone it is for want of a clean place to stand.
pub const DISCOMFORTS: [&str; 4] = [
    "",
    "Uneasy about the mess — treading carefully",
    "Sickened by the mess — keeping away from it",
    "Sickened by the mess — being sick in it every half hour",
];

/// And for want of anybody to talk to.
pub const LONELINESS: [&str; 4] = [
    "",
    "Desocialized — low, and a tenth slower at everything",
    "Badly desocialized — sits down on the deck every few hours",
    "Isolated — hurting itself, and past ten days it may stop altogether",
];

/// The errand codes shared by `activity()` and `agenda_job()`.
pub fn job_name(code: u32) -> &'static str {
    match code {
        1 => "Making food",
        2 => "Working the cooker",
        3 => "Nap",
        4 => "Sleep",
        5 => "Using the toilet",
        6 => "Fridge door",
        7 => "Bathroom door",
        8 => "Door lock",
        9 => "Starting the wash",
        11 => "Tending the bay",
        12 => "Eating leftovers",
        13 => "Sweeping up",
        14 => "Talking",
        15 => "Cooking stew for the store",
        16 => "Warming up a stew",
        17 => "Taking a shower",
        18 => "Making something",
        19 => "Mining outside",
        20 => "Carrying materials",
        21 => "Building",
        22 => "Dressing a wound",
        23 => "Treating a trauma",
        24 => "Picking a weapon up",
        25 => "Finishing off",
        26 => "Carrying gear to the workbench",
        27 => "Walking over",
        28 => "Setting up a kit",
        _ => "Busy",
    }
}

/// The same errands as the status line says them. A lie-down is left out:
/// the countdown from `rest_left()` is more use than the name.
pub fn activity_line(code: u32) -> Option<&'static str> {
    Some(match code {
        1 => "Making food…",
        2 => "Off to the cooker…",
        5 => "Using the toilet — a wash to follow",
        11 => "In the hydroponics…",
        12 => "Helping itself to the pot…",
        13 => "Sweeping the deck…",
        14 => "Having a word with the other one…",
        15 => "Cooking a stew for the store…",
        16 => "Warming a stew through…",
        17 => "In the shower…",
        18 => "At the bench…",
        19 => "Outside, mining…",
        20 => "Carrying a load to the site…",
        21 => "Building…",
        22 => "Dressing a wound…",
        23 => "Treating with a medkit…",
        24 => "Going for the weapon on the deck…",
        25 => "Finishing off a body…",
        26 => "Carrying gear to the workbench…",
        27 => "Walking over…",
        28 => "Setting a kit up…",
        _ => return None,
    })
}

/// What `order_move()` made of a right-click. Only the refusals are worth
/// saying out loud; the rest the Bim shows you by walking.
pub fn order_refused(code: u32) -> Option<&'static str> {
    match code {
        3 => Some("Can't get there — the bathroom door is locked."),
        4 => Some("Can't get there at all."),
        _ => None,
    }
}

/// The jobs on the work list, by `work::Job` code, and which fixture each
/// is about so resting on a row rings the place it happens.
pub const WORK_NAMES: [&str; 10] = [
    "Cleaning",
    "Planting",
    "Plant cutting",
    "Hauling",
    "Cooking",
    "Controlling the ship",
    "Making things",
    "Mining outside",
    "Building",
    "Medical",
];

/// The note on the *Making things* row: standing at a bench is your own
/// Bim's work, and the number on the row only ever says when it gets
/// round to it (feature 89).
pub const WORK_CRAFT_TIP: &str = "Only the Bim you steer stands at a bench — the smelter, the workbench, the armoury, the drug lab. The rest of the crew plant, cut, sweep, cook, haul, mine, build, doctor and fight, whatever this number says.";

pub const IDLE_HINT: &str = "Click a fixture for its menu · 1 or drag to select · right-click the floor to move · r to recruit";

// --- arms and armour ------------------------------------------------------------

/// What a weapon is called, indexed by `bims::combat::WeaponKind::code`;
/// `0` is an empty slot. The last two are a droid's built-in arms
/// (feature 83): they are named here because the picture names what it
/// draws, and nowhere else — nothing carries one.
pub const WEAPON_NAMES: [&str; 8] = [
    "—",
    "Laser pistol",
    "Shotgun",
    "Auto rifle",
    "Sniper rifle",
    "Schword",
    "Claw",
    "Unmaker",
];

/// What a piece of armour is called, indexed by `bims::combat::ArmourKind::code`;
/// `0` is an empty slot.
pub const ARMOUR_NAMES: [&str; 4] = ["—", "Basic helm", "Basic kevlar", "Basic leg guards"];

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

fn percent(odds: f32) -> String {
    format!("{}%", (odds * 100.0).round())
}

/// "90% to 4 tiles, 60% at 10".
pub fn accuracy_text(stats: &WeaponStats) -> String {
    curve_text(stats, percent(stats.accuracy), percent(stats.accuracy_far))
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
pub const ITEM_TIPS: [&str; 26] = [
    "Iron ore off a belt. Two lumps smelt into a bar of metal.",
    "A bar of metal: what most of the ship is built of, and what the workbench works.",
    "Components, worked out of metal at the workbench. Four to a bar.",
    "A vegetable off the bay. Two of them make a stew.",
    "A block of tofu, pressed from soy.",
    "Galvum, the rare crystal. Emitters and kevlar want it.",
    "An emitter: metal, components and galvum at the workbench. What a laser fires through.",
    "A pressure suit, for a walk outside.",
    "A laser handgun: two components and an emitter at the armoury.",
    "A vest, from the armoury.",
    "A medkit, from the armoury.",
    "Rock off a belt. Worth little.",
    "Fibre, the one crop nobody eats. Two of it roll into a bandage at the drug lab.",
    "A bandage. Closes every wound on one part of a body.",
    "A basic helm, for the head: two bars of metal at the workbench.",
    "Basic kevlar, for the body: three bars of metal and a galvum at the workbench.",
    "Basic leg guards: a bar of metal at the workbench.",
    "A shotgun: four bars of metal and two components at the armoury. Hits hard up close.",
    "An auto rifle: three bars of metal, three components and an emitter at the armoury. Fires in bursts.",
    "A sniper rifle: four bars of metal, two components and two emitters at the armoury. Reaches furthest.",
    "A schword, a blade with a laser edge: a bar of metal, a component and two emitters at the armoury. Cuts, at arm's length.",
    "A tier-one research key: an artifact off a station's research desk. Two cells tall. Put it in the ship's research desk and consume it there to open the locked part of the research tree.",
    "A tier-two research key: an artifact off a hostile station's research desk. Two cells tall. Put it in the ship's research desk and consume it there to open the upgrades node, which wants it and no other.",
    "A sandbag kit: a bar of metal at the workbench. An engineer lays it on a deck tile as a barricade to duck behind — E, over the tile.",
    "A sentry kit: two bars of metal, two components and an emitter at the workbench. An engineer of the third level sets it up as a turret with an auto rifle's aim — Q, over the tile.",
    "A grenade: a soldier of the third level carries two of them as charges and throws one — Q, over the tile — and it bursts on everything within two and a half tiles, friend and foe alike, two seconds on. A thrown one comes back into the pack thirty seconds later; nobody has to make one, though the armoury still can, out of a bar of metal and a component.",
];

pub fn item_tip(id: ResourceId) -> &'static str {
    ITEM_TIPS.get(id as usize).copied().unwrap_or("")
}

/// The research tree's nodes, indexed by `shipdesign::research::Node`, and
/// what each of them opens.
pub const NODE_NAMES: [&str; 10] = [
    "Living aboard",
    "Mining",
    "Medicine",
    "Smelting",
    "Workshop",
    "Fusion power",
    "Armoury",
    "Emitters",
    "Hyperdrive",
    "Upgrades",
];

pub const NODE_LINES: [&str; 10] = [
    "Everything a crew needs to live and to fly: the hull, the galley, the heads, the bunks, the hydroponic bay, the fusion reactor, the helm and the engines. Known from the start.",
    "A walk outside with a pick: the suit locker and the suit. Known from the start.",
    "Bandages and medkits at the drug lab. Known from the start.",
    "The smelter: two ore into a bar of metal.",
    "The workbench: a bar of metal into four components.",
    "The large fusion reactor: four reactors' power in a three-by-three block — a heavy engine flat out, and every bench and system aboard.",
    "The armoury, every weapon it makes, the vest, and the three pieces of armour at the workbench.",
    "The emitter at the workbench: what a laser fires through, and what the guns want — and the engineer's sentry kit, which fires through one.",
    "The hyperdrive: a jump to another star, bolted to a main engine. Charged from the helm for twenty seconds, and then the ship is in empty space round the star picked on the galaxy chart.",
    "The workbench's upgrades: two weapons or pieces of a kind at one tier into one of the next — tier one to two, and two to three. The first tier-two node: it wants a tier-two key, which lies on the research desk of every hostile station.",
];

pub fn node_name(code: u32) -> &'static str {
    NODE_NAMES
        .get(code as usize)
        .copied()
        .unwrap_or("something")
}

pub fn node_line(code: u32) -> &'static str {
    NODE_LINES.get(code as usize).copied().unwrap_or("")
}

/// The Research tab.
pub const RESEARCH_TIP: &str = "Research is done by the ship's AI at the research desk, on the desk's power — the crew have stopped being able to. Pick a node and queue it: whatever it needs that is not yet known goes onto the queue ahead of it, the AI works through the queue in order on the clock — days at a time — and what a node opens can be built from the Build tab and made at the benches after. A node taken off the queue takes with it whatever was waiting on it. The nodes behind a lock want a research key each — one key opens one node, and a node wants a key of its own tier: a tier-one key is found on the research desk of most friendly stations, a tier-two key on the desk of every hostile one — lit up either way, so it can be seen from the door — and a crew member within two tiles takes it into their pack, where it is two cells tall. Put it in the ship's own desk and consume it there for the node, and that node is open for good.";
pub const NO_DESK_HINT: &str =
    "No research desk aboard — the AI works on one. Build one from the Build tab.";
pub const DESK_DARK_HINT: &str =
    "The research desk is unpowered: nothing is researched until it is.";
pub const KEY_ROW: &str = "Take the research key";
pub const KEY_ROW_HINT: &str = "walk over and take it into the pack — it is two cells tall";

/// The desk's row, naming the tier of key that lies on it.
pub fn key_row(tier: u8) -> String {
    match tier {
        2 => "Take the tier-two research key".to_string(),
        _ => KEY_ROW.to_string(),
    }
}
pub const NO_KEY_ROW_HINT: &str = "there is no key on this desk";
pub const RESEARCH_WINDOW: &str = "Research desk";
pub const RESEARCH_LOCKED: &str = "needs research";

/// The container windows' titles. A workstation's window is named for the
/// part — the armoury, the drug lab — off `PART_NAMES`; the other two have
/// no part of their own to be named for.
pub const STORAGE_WINDOW: &str = "Storage";
pub const COLD_STORE_WINDOW: &str = "Cold store";

/// The Plunder window's title — an enemy's shelf, a raider's or a hostile
/// station's, laid out as loot — and its `?`; and the one row a friend's
/// shelf on the joined deck gets under a click, since that one is the
/// desk's to sell from.
pub const PLUNDER_WINDOW: &str = "Enemy's shelf";
pub const PLUNDER_TIP: &str = "Everything on the shelf of the station the ship is tied to, an enemy's: whatever its kind stocks, a stack to three of each, as the crew found it and have left it — a stack taken stays taken. Ctrl-click a stack to take it into the pack of the Bim shown, one to a cell, as far as the pack goes; right-click for the row. Taking wants the Bim within two tiles of one of the station's shelves — clicking the shelf walks it over. Nothing is put onto the shelf. A friend's shelf is bought from across its desk instead.";
pub const SHELF_ASHORE_ROW: &str = "The station's shelf";
pub const SHELF_ASHORE_HINT: &str =
    "bought from across the desk — only an enemy's shelf is taken from";

/// The Loot window's title, with the body's name after it, and the menu
/// row on a body — a dead crew member, one out cold, or one of a hostile
/// station's people lying in its own room — that opens it.
pub const LOOT_WINDOW: &str = "Loot";
pub const LOOT_ROW: &str = "Loot";

/// The row on one of an enemy station's people lying out cold: finish it
/// off. RimWorld's execution — the Bim shown walks over and shoots it
/// where it lies, or cuts it from beside it with a blade.
pub const KILL_ROW: &str = "Kill";
/// Why the Kill row is greyed: nothing in the Bim's hand to do it with.
pub const KILL_UNARMED: &str = "nothing in hand to do it with";

/// What the Kill row says it will do, by the weapon in hand.
pub fn kill_hint(weapon: Option<bims::combat::WeaponKind>) -> &'static str {
    match weapon {
        Some(w) if w.stats().melee => "cut it where it lies, from beside it",
        Some(_) => "shoot it where it lies, from close by",
        None => KILL_UNARMED,
    }
}

pub const LOOT_TIP: &str = "Everything on the body: the pack on its back, the three pieces it wears with the health they have left, and the weapon in its hand. Ctrl-click a thing to take it into the pack of the Bim shown; right-click for the row. Taking wants the Bim within two tiles of the body — the Loot row walks it over — and a free cell in its pack; a piece comes off the body as it is, broken or not, and a weapon goes into the pack to be equipped from there. Nothing is put onto a body. A crewmate that comes round is no longer a body, and the window shuts.";

/// The Hire window's title, with the mercenary's name after it, the menu
/// row on a mercenary for hire — one of a friendly station's people in
/// the olive coverall, with a `?` over its head — that opens it, and the
/// button in it.
pub const HIRE_WINDOW: &str = "Hire";
pub const HIRE_ROW: &str = "Hire — see the terms";
pub const HIRE_BUTTON: &str = "Hire";
pub const HIRE_TIP: &str = "A mercenary lives at a friendly station and is for hire: the fee is a month of them, paid now and again every month after out of the crew's money, and it is what they carry — a heavier gun and a piece of armour each cost more. Hiring wants the Bim shown within two tiles of them (opening this walks it over), the money for the first month, and a free bunk aboard. A month the money will not cover has them walk off at the next berth, for hire again.";
/// A mercenary hired for its trade rather than its gun (feature 86).
pub const FIELD_MEDIC: &str = "Field medic";
pub const FIELD_MEDIC_TIP: &str = "A field medic is hired to save your crew, not to win the fight. Under arms it keeps to the far end of its weapon's reach, fetches whoever goes down out of the fire — in its arms, at half pace, holding its fire — sets them down where it is quiet, and treats them there. It sets out with two medkits and fills up out of the hold between fights. It has none of a medic's own skills: the premium on the month is the trade.";
pub const NO_BUNK_HINT: &str = "no bunk aboard for one more";
pub const BROKE_HINT: &str = "not the money for the first month";
pub const MERCENARY_MARK: &str = "?";

/// The trade window's word while nobody of yours is at the station's
/// trading desk, the button beside it, and the desk's own menu row.
pub const NOT_AT_DESK: &str = "Nobody of yours is at the trading desk";
pub const WALK_TO_DESK: &str = "Walk over";
pub const TRADE_ROW: &str = "Trade";

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
pub const BID_HEAD: &str = "Pays";
/// A price where nobody quotes one: a derelict's desk, or nowhere.
pub const NO_QUOTE: &str = "—";
pub const ABOARD_HEAD: &str = "Aboard";
pub const CART_HEAD: &str = "Cart";

/// The red warning along the top while a raid is on — feature 68: what
/// it says while the raider closes (`span` from `format::in_words`,
/// counted down every frame), while its boarders stand at the ship's
/// locked airlock, while they heave at it, and once it has given.
pub fn raid_incoming(span: &str, boarders: u32) -> String {
    format!("RAIDERS — incoming in {span}, {boarders} aboard")
}
pub fn raid_at_the_airlock(boarders: u32) -> String {
    format!("RAIDERS ALONGSIDE — {boarders} at the locked airlock")
}
pub fn raid_forcing(boarders: u32) -> String {
    format!("RAIDERS ALONGSIDE — {boarders} forcing the airlock")
}
pub fn raid_aboard(boarders: u32) -> String {
    format!("RAIDERS ABOARD — {boarders} through the airlock")
}
/// The red warning along the top while the station alongside is held by
/// the machines (feature 83): which wave is on the deck and how many are
/// standing, then the countdown to the next one landing (`span` from
/// `format::in_words`, counted down every frame), then the last of them
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
pub const DROIDS_TIP: &str = "The station is held by the machines, and they come in waves. How many waves there are was fixed the first time you docked here and never changes; how big each one is, is worked out as it appears, so a richer crew meets more of them. No wave arrives while a machine of the last one is still standing — the countdown starts when the last of them is destroyed — and the next comes in through the airlock farthest from your own, or through a gate of the town on a planet. Everybody's speed goes back to 1× when one lands.";

pub const RAID_TIP: &str = "A hostile ship is closing on yours and will tie up alongside. Its boarders come for the ship through your airlock, which is locked in their face the moment they arrive: they have to force it — half a minute of heaving, the bar over the door — and you may unlock it from its panel yourself to meet them in the passage. Everybody's speed was put back to 1× when it came onto the radar; leaving before it arrives loses it.";

/// The header's word while the crew's alarm is up, and what it means.
pub const ALARM_STATUS: &str = "To arms — an enemy is near";
pub const ALARM_TIP: &str = "An enemy within thirty tiles of anybody or in anybody's sight, or a crew member hit, in the last half minute: every crew member but the one you steer draws a weapon — out of the pack if the hand is empty — and fights, walking to wherever it can shoot from, until nobody is near, nobody has seen one and nobody has been hit for half a minute — then it goes back to its day, however many of the station's people are still alive somewhere on it. The one you steer is yours: recruit it yourself, or leave it to its errands.";

/// The rows on a bunk. A bunk is one crew member's: the one shown may be
/// given it — whoever had it loses it — or give it up. Nap and Sleep are
/// offered on the Bim's own bunk alone.
pub const BED_ASSIGN_ROW: &str = "Assign to";
pub const BED_ASSIGN_HINT: &str = "makes this bunk theirs — whoever had it sleeps on the deck";
pub const BED_UNASSIGN_ROW: &str = "Give up this bunk";
pub const BED_UNASSIGN_HINT: &str =
    "nobody's, until it is given to somebody — they sleep on the deck meanwhile";
pub const BED_OWN_HINT: &str = "their own";
pub const BED_NOBODY_S: &str = "nobody's";
/// A station's bunk on the joined deck: not the ship's to give.
pub const BED_FOREIGN_HINT: &str = "the station's — not yours to give";
/// The tag written on a bunk nobody has, on the deck (feature 61); a bunk
/// somebody has wears their name.
pub const BED_TAG_UNASSIGNED: &str = "Unassigned";
/// The status lines: a Bim with no bunk, and one sore from the deck.
pub const NO_BED_LINE: &str = "No bunk — sleeps on the deck, three hours in every six";
pub const SORE_LINE: &str = "Slept on the deck";
pub const SORE_TIP: &str = "A crew member with no bunk of its own lies down on the deck where it stands, three hours out of every six at most, and is sore for half a day after: rest runs out half as fast again. Click a bunk to give it one — a bunk is one crew member's, and whoever had it loses it.";

pub const INVENTORY_TIP: &str = "What the Bim has on it: head, body and leg protection down the left, the weapon in hand, and the pack on its back — nine cells, one thing each. Recruited, a Bim is in combat mode — it draws the weapon and shoots at any enemy it can see and reach, leaning out from cover to do it, where half the shots at it miss. A blade within reach locks it in melee: the gun goes quiet and it fights with its fists until one of them is out of reach. Right-click a thing in the pack to put it on, put it away or throw it out; right-click a worn piece to take it off. Ctrl-click moves a thing straight into the open container, or out of one into the pack. Beside each slot is the bleeding on that part of the body, and a Bandage button that sends the crew member you steer to dress it.";

pub const CONTAINER_TIP: &str = "What is in the hold, by where it is kept: each hold is a grid ten across, every part of its kind aboard so many rows of it, and everything kept there laid over the cells it takes — a pistol a row of two, a rifle seven, a sniper rifle the whole width; a vest four by four; a crate of vegetables one by two, a block of tofu four by four — a piece of armour with its health under it. Goods that stack take one footprint a stack, ten ore or twenty components to a cell, with the count in the corner; so what a shelf holds is its cells times the stacks. A thing goes in only where there is a run of cells for it: drag a thing to move it, press the Turn key (R, unless you changed it) while carrying it to turn it a quarter round, and let go where the ghost shows green. The armoury is the lockers, the shelves take materials and anything worn or held, the cold store is the food. Ctrl-click a thing to take one into the pack of the Bim shown; right-click for the rows. The Bim has to be within two tiles: clicking a container walks it over.";

pub const REACH_HINT: &str = "walk over first — it is out of reach";
pub const ACCURACY_TIP: &str = "The odds of a shot landing: its best out to the first distance, then falling in a straight line to the second at the weapon's range, beyond which it does not shoot.";
pub const DAMAGE_TIP: &str = "What a shot takes off the part it lands on, by how far it flew: its best out to the first distance, falling in a straight line to the second at the range. Armour on the part takes it first.";
pub const FIRE_RATE_TIP: &str = "How often the trigger goes. A burst weapon fires its burst a shot at a time and then recharges for the rest.";
pub const DPS_TIP: &str = "Damage a second with every shot landing up close: the shots a trigger pull, the fire rate and the damage a shot multiplied.";

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
        assert_eq!(crew_or(1, "One of the crew"), "Ada");
        assert!(memory_line(30, 1).unwrap().starts_with("Ada died"));
        set_crew_names(&[]);
        assert_eq!(crew_name(1), "Kate");
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
            assert_eq!(RESOURCE_NAMES.len(), ResourceId::ALL.len());
            assert_eq!(STORAGE_NAMES.len(), shipdesign::Storage::ALL.len());
        }

        // --- every_research_node_has_a_name_and_a_line ---
        {
            assert_eq!(NODE_NAMES.len(), shipdesign::research::Node::ALL.len());
            assert_eq!(NODE_LINES.len(), shipdesign::research::Node::ALL.len());
            assert_eq!(ITEM_TIPS.len(), ResourceId::ALL.len());
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

        // --- every_buildable_part_is_in_one_build_group ---
        {
            for &kind in PartKind::ALL.iter() {
                let code = kind as u32;
                let groups = BUILD_GROUPS
                    .iter()
                    .filter(|(_, _, kinds)| kinds.contains(&code))
                    .count();
                let expected = if NOT_A_TOOL.contains(&code) { 0 } else { 1 };
                assert_eq!(groups, expected, "{kind:?} is in {groups} build groups");
            }
        }
    }

    #[test]
    fn every_table_of_the_room_is_as_long_as_its_enum() {
        // --- the_work_list_names_every_job ---
        {
            assert_eq!(WORK_NAMES.len(), bims::work::Job::ALL.len());
        }

        // --- the_classes_and_the_talents_are_named_and_every_pick_level_tipped ---
        {
            assert_eq!(CLASS_NAMES.len(), world::Class::ALL.len());
            assert_eq!(CLASS_TIPS.len(), world::Class::ALL.len());
            assert_eq!(TALENT_NAMES.len(), world::Talent::ALL.len());
            assert_eq!(TALENT_TIPS.len(), world::Talent::ALL.len());
            assert_eq!(DEPLOYABLE_NAMES.len(), 2);
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
            for (i, talent) in world::Talent::ALL.iter().enumerate() {
                assert_eq!(talent.code() as usize, i);
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
                Refusal::NoTalent,
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
                Refusal::NoSurgeYet,
                Refusal::NotCharged,
                Refusal::NotLinked,
                Refusal::NotATank,
                Refusal::NoTauntYet,
                Refusal::NotACommander,
                Refusal::NoRallyYet,
                Refusal::NoSquadInRange,
                Refusal::NoEnemyThere,
            ] {
                assert!(!refusal(why).is_empty());
                assert!(deploy_refused(why).contains(refusal(why)));
                assert!(throw_refused(why).contains(refusal(why)));
                assert!(brace_refused(why).contains(refusal(why)));
                assert!(beam_refused(why).contains(refusal(why)));
                assert!(surge_refused(why).contains(refusal(why)));
                assert!(bulwark_refused(why).contains(refusal(why)));
                assert!(taunt_refused(why).contains(refusal(why)));
                assert!(squad_refused(why).contains(refusal(why)));
                assert!(rally_refused(why).contains(refusal(why)));
            }
            assert_eq!(squad_line(None, 0), SQUAD_NONE);
            assert_eq!(squad_line(Some(0), 3), "Squad attacking — 3");
            assert_eq!(squad_line(Some(1), 2), "Squad falling back — 2");
            assert_eq!(squad_line(Some(2), 1), "Squad holding ground — 1");
            assert_eq!(
                rally_line(0.0, 0.0, false),
                format!("Rally at level {}", world::class::RALLY_LEVEL)
            );
            assert_eq!(rally_line(0.0, 0.0, true), "Rally ready");
            assert_eq!(rally_line(0.0, 7.2, true), "Rally ready in 7 s");
            assert_eq!(rally_line(4.0, 12.0, true), "Rallying — 4 min left");
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
            assert_eq!(
                surge_line(0.0, false),
                format!("Surge at level {}", world::class::SURGE_LEVEL)
            );
            assert_eq!(surge_line(0.5, true), "Surge 50%");
            assert_eq!(surge_line(1.0, true), "Surge charged");
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
                WorldEvent::Repaired { kind: 1 },
                WorldEvent::Braced { who: 0, on: true },
                WorldEvent::Braced { who: 0, on: false },
                WorldEvent::Thrown { who: 0 },
                WorldEvent::Beamed {
                    who: 0,
                    patient: Some(1),
                },
                WorldEvent::Beamed {
                    who: 0,
                    patient: None,
                },
                WorldEvent::Surged { who: 0 },
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

        // --- the_readout_names_every_mess ---
        {
            // Blood is the last kind; the table is indexed by the code.
            assert_eq!(
                MESS_NAMES.len(),
                bims::filth::Mess::Blood.code() as usize + 1
            );
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
                event_line(WorldEvent::Equipped {
                    who: 0,
                    kind: ArmourKind::BasicHelm
                })
                .is_some()
            );
            assert!(event_line(WorldEvent::Stowed { who: 0 }).is_some());
            assert!(
                event_line(WorldEvent::PieceBroke {
                    who: 0,
                    kind: ArmourKind::BasicKevlar
                })
                .is_some()
            );
            assert!(event_line(WorldEvent::Locked { who: 0 }).is_some());
            let begun = event_line(WorldEvent::UpgradeBegun {
                resource: 8,
                tier: 2,
            })
            .unwrap();
            assert!(
                begun.contains("tier 2") && begun.contains(resource_name(ResourceId::Handgun)),
                "{begun}"
            );
            assert!(
                event_line(WorldEvent::Upgraded {
                    resource: 14,
                    tier: 3
                })
                .is_some()
            );
            // A loot names whose body it was by the kind code, and both kinds
            // read differently.
            let off_crew = event_line(WorldEvent::Looted {
                who: 0,
                source_kind: 0,
            });
            let off_resident = event_line(WorldEvent::Looted {
                who: 0,
                source_kind: 1,
            });
            assert!(off_crew.is_some() && off_resident.is_some());
            assert_ne!(off_crew, off_resident);
            // And the refusals a gear command can come back with each say
            // something other than the fallback.
            for why in [
                Refusal::OutOfReach,
                Refusal::PackFull,
                Refusal::NoRoom,
                Refusal::Broken,
                Refusal::NotDown,
            ] {
                assert!(!refusal(why).is_empty());
            }
            assert!(!LOOT_WINDOW.is_empty() && !LOOT_ROW.is_empty() && !LOOT_TIP.is_empty());
            assert!(!PLUNDER_WINDOW.is_empty() && !PLUNDER_TIP.is_empty());
            // A plunder of one reads differently from a plunder of many.
            let one = event_line(WorldEvent::Plundered { who: 0, units: 1 });
            let many = event_line(WorldEvent::Plundered { who: 0, units: 7 });
            assert!(one.is_some() && many.is_some());
            assert_ne!(one, many);
        }

        // --- every_part_of_a_body_has_a_name ---
        {
            // `CrewHit` carries the part as a code, and the hit line runs it
            // into a sentence; the bandage menu names the same three.
            assert_eq!(BODY_PART_NAMES.len(), bims::health::Part::ALL.len());
            for part in bims::health::Part::ALL {
                assert!(!body_part_name(part.code()).is_empty());
            }
        }

        // --- the_bandage_job_has_a_name_and_a_line ---
        {
            // The newest job code, the one most likely to have been forgotten:
            // `bims::game::JOB_BANDAGE` on the agenda and the status line.
            for code in [
                bims::game::JOB_BANDAGE,
                bims::game::JOB_TREAT,
                bims::game::JOB_FETCH,
                bims::game::JOB_EXECUTE,
                bims::game::JOB_FERRY,
                bims::game::JOB_WALK,
                bims::game::JOB_DEPLOY,
            ] {
                assert_ne!(job_name(code), job_name(u32::MAX));
                assert!(activity_line(code).is_some());
            }
        }

        // --- every_trauma_has_a_name_and_a_line_and_the_events_say_them ---
        {
            use bims::health::Trauma;
            assert_eq!(TRAUMA_NAMES.len(), Trauma::ALL.len());
            assert_eq!(TRAUMA_LINES.len(), Trauma::ALL.len());
            assert_eq!(TRAUMA_AFTER.len(), Trauma::ALL.len());
            assert_eq!(TRAUMA_LASTING.len(), Trauma::ALL.len());
            for t in Trauma::ALL {
                assert!(!trauma_name(t.code()).is_empty());
                assert!(!trauma_line(t.code()).is_empty());
                // What is said to linger is what the rules say lingers: a leg
                // lost for ever, or a lasting penalty.
                assert_eq!(
                    trauma_after(t.code()).is_empty(),
                    t.after().is_none() && !t.loses_leg(),
                    "{t:?}"
                );
                assert_eq!(
                    trauma_lasting(t.code()).is_empty(),
                    t.after().is_none(),
                    "{t:?}"
                );
                assert!(
                    event_line(WorldEvent::CrewDying {
                        who: 0,
                        trauma: t.code()
                    })
                    .is_some()
                );
                assert!(
                    event_line(WorldEvent::CrewTreated {
                        who: 0,
                        trauma: t.code()
                    })
                    .is_some()
                );
            }
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
            assert_eq!(accuracy_text(&shotgun), "81% to 4 tiles, 54% at 10");
            assert_eq!(damage_text(&shotgun), "60 to 4 tiles, 36 at 10");
            let pistol = WeaponKind::LaserPistol.stats();
            assert_eq!(accuracy_text(&pistol), "86% up close, 58% at 22 tiles");
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
            assert_eq!(DROID_NAMES.len(), bims::droid::DroidKind::ALL.len() + 1);
            for kind in bims::droid::DroidKind::ALL {
                assert!(!droid_name(kind.code()).is_empty());
                assert_ne!(droid_name(kind.code()), DROID_NAMES[0]);
            }
            assert_eq!(droid_name(99), "Machine", "a kind with no row");
        }

        // --- every_thing_in_a_cell_has_a_tip ---
        {
            // A cell's tooltip is the resource's line; a blank one is an icon
            // nobody can ask about.
            assert_eq!(ITEM_TIPS.len(), ResourceId::ALL.len());
            for &id in ResourceId::ALL.iter() {
                assert!(!item_tip(id).is_empty(), "{id:?} has no tip");
            }
        }

        // --- the_loot_window_has_a_label_for_every_cell ---
        {
            // The Loot window is the pack's nine cells and a row of four under
            // them labelled off `SLOT_NAMES`, which is how the row's length is
            // tied to what a body shows.
            assert_eq!(
                bims::combat::PACK_CELLS + SLOT_NAMES.len(),
                bims::combat::LOOT_CELLS
            );
            for (i, code) in [
                bims::combat::LootCell::Head,
                bims::combat::LootCell::Body,
                bims::combat::LootCell::Legs,
                bims::combat::LootCell::Weapon,
            ]
            .into_iter()
            .enumerate()
            {
                assert_eq!(code.code() as usize, bims::combat::PACK_CELLS + i);
            }
        }
    }
}
