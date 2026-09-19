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

pub fn crew_name(who: u32) -> String {
    CREW_NAMES
        .get(who as usize)
        .map(|s| s.to_string())
        .unwrap_or_else(|| format!("Crew {}", who + 1))
}

/// What the people living on a station are called. The world knows a
/// resident as a station and a seat and nothing else, so the names are
/// dealt out here, by station and seat, off one list: enough that the two
/// on one station never share a name, and the same pair every time the ship
/// comes back.
pub const RESIDENT_NAMES: [&str; 16] = [
    "Ada", "Tomas", "Priya", "Yusuf", "Mei", "Olu", "Sanne", "Ravi", "Ines", "Kofi", "Hana",
    "Bram", "Leila", "Jonas", "Nour", "Emil",
];

pub fn resident_name(station: u32, who: u32) -> String {
    RESIDENT_NAMES[((station * 2 + who) as usize) % RESIDENT_NAMES.len()].to_string()
}

/// What each part is called. Indexed by the `PartKind` discriminant in
/// `crates/shipdesign/src/parts.rs`.
pub const PART_NAMES: [&str; 38] = [
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
    "Reactor",
    "Power conduit",
    "Battery",
    "Fuel tank",
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
/// a button — under "Anything else", where it is obvious.
pub const PART_GROUPS: &[(&str, &[u32])] = &[
    // Hull first, in the order a ship is actually built: deck, skin, then the
    // ways through it. The frame is not a tool of its own — see NOT_A_TOOL.
    ("Hull", &[0, 1, 29, 16, 30, 2, 23, 37]),
    ("Systems", &[3, 28, 27, 17, 18, 19, 20, 21, 22, 24]),
    ("Crew", &[4, 9, 10, 26, 36]),
    ("Galley", &[5, 6, 7, 8]),
    ("Heads", &[11, 12]),
    ("Storage", &[25, 33]),
    ("Bay", &[13, 14]),
    ("Workshop", &[31, 32, 34, 35]),
];

/// The Build tab's categories, the way a colonist-game player thinks about
/// them rather than the way a shipwright does: what holds the ship
/// together, what it is lived in with, what makes things, and so on. Every
/// kind the palette offers is in exactly one — `every_buildable_part_is_in_one_build_group`
/// pins it — and the frame is left out for `NOT_A_TOOL`'s reason. Each is
/// a name and a line saying what goes under it.
pub const BUILD_GROUPS: &[(&str, &str, &[u32])] = &[
    (
        "Structure",
        "The frame and the skin: deck to walk on, walls to divide it, the hull that keeps the outside out, the ways through, and sandbags for cover.",
        &[0, 1, 29, 16, 30, 2, 23, 37],
    ),
    (
        "Furniture",
        "What the crew live with: somewhere to sleep, sit and eat, somewhere to keep things, and a desk to trade across.",
        &[4, 9, 10, 25, 14, 36],
    ),
    (
        "Production",
        "Where something is made: the workshop benches, the armoury, the drug lab, and the bay that grows the food.",
        &[31, 32, 34, 35, 13],
    ),
    (
        "Galley",
        "Where a meal is cooked and cleared up after.",
        &[5, 6, 7, 8],
    ),
    ("Hygiene", "The heads and the shower.", &[11, 12, 26]),
    (
        "Power",
        "What makes power, what carries it, and what holds it.",
        &[18, 19, 20],
    ),
    (
        "Ship systems",
        "What flies the ship and keeps it alive: the helm, life support, the sensors, the tank, and the suits.",
        &[17, 22, 24, 21, 33],
    ),
    (
        "Propulsion",
        "The engines that push and the thrusters that turn.",
        &[3, 28, 27],
    ),
];

/// Kinds the palette does not offer, though the ship knows them. Structure
/// is the one: deck plating lays its own frame, so to a player the frame and
/// the deck are one thing and a second button for the half underneath would
/// be a trap.
pub const NOT_A_TOOL: &[u32] = &[15];

/// What a station sells, indexed by `physics::ResourceId`.
pub const RESOURCE_NAMES: [&str; 22] = [
    "Ore",
    "Metal",
    "Fuel",
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
];

pub fn resource_name(id: ResourceId) -> &'static str {
    RESOURCE_NAMES
        .get(id as usize)
        .copied()
        .unwrap_or("Something")
}

/// Where goods are stowed, indexed by `economy::Storage`.
pub const STORAGE_NAMES: [&str; 4] = ["Shelves", "Fuel tanks", "Cold stores", "Lockers"];

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
        30 => "No fuel aboard, so no trip can be started.",
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
        1 => "No engine pushes the ship forward.",
        2 => "Nothing to turn with — it cannot aim or stop.",
        3 => "No helm to fly it from.",
        4 => "Not enough fuel that is not already spoken for.",
        5 => "Nobody has found that yet.",
        6 => "The ship is already there.",
        7 => "No fuel aboard at all.",
        _ => "That cannot be flown.",
    }
}

/// Why an order did nothing. Separate from the list above on purpose: "you
/// are not docked" and "you have no fuel" are different things to be told.
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
    }
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
pub const STATE_NAMES: [&str; 6] = [
    "Docked",
    "Holding",
    "Under way",
    "Casting off",
    "Undocking",
    "Docking",
];

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
        WorldEvent::Traded { units, .. } if units >= 0 => format!("{units} aboard."),
        WorldEvent::Traded { units, .. } => format!("{} sold.", -units),
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
        WorldEvent::CrewDown { who: w } => format!("{} is down.", who(w)),
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
    "Cleanliness",
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
            "Not a clock like the others: it follows the mess within three tiles of the Bim and whatever the Bim has on itself. At nothing it treads carefully, then keeps away from the mess, then is sick in it every half hour."
        }
        4 => {
            "The one need that wants another Bim rather than a fixture. Past its trigger the Bim goes and finds the other one, and they stand and talk about whatever they have been doing. This bar is the comfortable end of it; what matters is the count of days underneath, because going without runs on a far longer clock — three days alone and a Bim is low and slow, five and it sits down on the deck, seven and it starts hurting itself."
        }
        5 => {
            "A day's grime, on the clock like food: it runs down over the waking day and the Bim wants a shower about once a day. Past its trigger it goes and takes one, where the ship has a shower — a room without one is a Bim that goes on wanting a wash and nothing worse. Separate from Cleanliness, which is the deck around it."
        }
        _ => "",
    }
}

pub const HEALTH_TIP: &str = "The head, the body and the legs add up to this bar: a shot takes its damage off whichever it lands on, and the head or the body at nothing is death. The legs at nothing is a leg lost. Every hit opens a wound that bleeds until it is dressed — the Blood bar underneath — and below half blood the Bim is slow, below a third out cold, at nothing dead. Only the worst stage of malnutrition costs health of itself, and eating properly walks it back. The lines underneath name whatever is wrong. The blue on the end of a bar is armour: a worn piece adds what it has left to the part, takes every hit first — its protection comes off the damage before anything else, and the rest drains the piece — and only what the piece cannot take reaches the body. At nothing it is broken: still worn, doing nothing, worth nothing put away — discard it and make another.";

pub const BANDAGE_TIP: &str = "A bandage closes every wound on one part of a body — the head, the body or the legs — and stops the bleeding there. Order one here, or right-click a Bim on the deck, and the crew member you steer walks over and dresses it, ten minutes with hands on. Bandages are made at the drug lab out of fibre, or bought where a station sells them.";

/// Why a Bandage row is greyed when the helper cannot do it: the crew
/// member you steer is dead, out cold, or outside in a suit.
pub const HELPER_OUT: &str = "not from where the Bim is";

/// Why a Bandage row is greyed for a patient outside in a suit: nobody
/// can walk to it there.
pub const PATIENT_OUT: &str = "not while the patient is outside — it comes in first";

pub const FIBRE_TIP: &str = "Fibre is the one crop nobody eats: a day in a tray, and two of it make a bandage at the drug lab. A target here has the bay grow it like greens and soy; 0 means never.";

pub const AUTONOMY_TIP: &str = "Off, the Bim starts nothing by itself — no meals, no sleep, no trips to the toilet — but still does everything it is told. The levels carry on moving either way.";

pub const TARGET_TIP: &str = "A target is a standing order: keep at least this many in the cold store. Whenever the count falls below it, the work goes on the crew's list by itself and whoever is free does it — for vegetables and tofu, planting a tray in the hydroponic bay (greens, or soy for tofu) and carrying the harvest to the store; for stew, cooking a pot on the hob out of one vegetable and one block of tofu and putting it on the shelf. Once the count is back at the target the job comes off the list, and above it nothing is grown or cooked. 0 means never. How soon it gets done is the Planting and Cooking priorities on the Work tab.";

pub const TRIGGER_TIP: &str = "How low a need may get before the Bim breaks off and does something about it: past the mark on a row it takes itself to bed, or goes and cooks, on its own account. The timetable above says when it may sleep; the Rest threshold is the floor under that — past it the Bim turns in whatever the hour, unless a meal or the heads comes first. Untick one and that need still runs down and still tells on the Bim; only the errand stops.";

pub const SPEED_TIP: &str =
    "How fast the simulation runs. At 24x a whole game day goes by in about a minute.";

pub const BUILD_TIP: &str = "Lay out a part and the crew build it, out of what is on the shelves: whoever is free carries what it is made of to the site a load at a time, then stands beside it and puts it together — Hauling and Building on the Work tab say how soon. A site beyond the hull is reached in a suit, through the airlock. Nothing is built while the ship is moving, and the ship stays put while something is being built.";

pub const ITEMS_TIP: &str = "What is aboard, by where it is kept: ore, metal and components on the shelves; fuel in the tanks; vegetables and tofu in the cold store; armour, weapons and medical things in the lockers. The food is what the crew can eat now — the cold store is refilled from the manifest at every dock. Click the armoury, a shelf or the cold store on the deck to reach into it.";

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
    CREW_NAMES
        .get(who as usize)
        .map(|s| s.to_string())
        .unwrap_or_else(|| fallback.to_string())
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
pub const SPOT_NAMES: [&str; 22] = [
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

pub const IDLE_HINT: &str = "Click a fixture for its menu · 1 or drag to select · right-click the floor to move · r to recruit";

// --- arms and armour ------------------------------------------------------------

/// What a weapon is called, indexed by `bims::combat::WeaponKind::code`;
/// `0` is an empty slot.
pub const WEAPON_NAMES: [&str; 6] = [
    "—",
    "Laser pistol",
    "Shotgun",
    "Auto rifle",
    "Sniper rifle",
    "Schword",
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
        format!("{near} up close, {far} at {} tiles", stats.range)
    } else {
        format!("{near} to {} tiles, {far} at {}", stats.sweet, stats.range)
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
        (true, _) => format!("{} a swing", stats.damage),
        (false, true) => format!("{} a shot", stats.damage),
        (false, false) => curve_text(
            stats,
            format!("{}", stats.damage),
            format!("{}", stats.damage_far),
        ),
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
    format!("Melee — {} {}", stats.damage, fire_rate_text(stats))
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
    "Iron ore off a belt. Two lumps smelt into a bar of metal.",
    "A bar of metal: what most of the ship is built of, and what the workbench works.",
    "Fuel, in the tanks. What a trip burns.",
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
];

pub fn item_tip(id: ResourceId) -> &'static str {
    ITEM_TIPS.get(id as usize).copied().unwrap_or("")
}

/// The container windows' titles. A workstation's window is named for the
/// part — the armoury, the drug lab — off `PART_NAMES`; the other two have
/// no part of their own to be named for.
pub const STORAGE_WINDOW: &str = "Storage";
pub const COLD_STORE_WINDOW: &str = "Cold store";

/// The Loot window's title, with the body's name after it, and the menu
/// row on a body — a dead crew member, one out cold, or one of a hostile
/// station's people lying in its own room — that opens it.
pub const LOOT_WINDOW: &str = "Loot";
pub const LOOT_ROW: &str = "Loot";

pub const LOOT_TIP: &str = "Everything on the body: the pack on its back, the three pieces it wears with the health they have left, and the weapon in its hand. Ctrl-click a thing to take it into the pack of the Bim shown; right-click for the row. Taking wants the Bim within two tiles of the body — the Loot row walks it over — and a free cell in its pack; a piece comes off the body as it is, broken or not, and a weapon goes into the pack to be equipped from there. Nothing is put onto a body. A crewmate that comes round is no longer a body, and the window shuts.";

/// The Hire window's title, with the mercenary's name after it, the menu
/// row on a mercenary for hire — one of a friendly station's people in
/// the olive coverall, with a `?` over its head — that opens it, and the
/// button in it.
pub const HIRE_WINDOW: &str = "Hire";
pub const HIRE_ROW: &str = "Hire — see the terms";
pub const HIRE_BUTTON: &str = "Hire";
pub const HIRE_TIP: &str = "A mercenary lives at a friendly station and is for hire: the fee is a month of them, paid now and again every month after out of the crew's money, and it is what they carry — a heavier gun and a piece of armour each cost more. Hiring wants the Bim shown within two tiles of them (opening this walks it over), the money for the first month, and a free bunk aboard. A month the money will not cover has them walk off at the next berth, for hire again.";
pub const NO_BUNK_HINT: &str = "no bunk aboard for one more";
pub const BROKE_HINT: &str = "not the money for the first month";
pub const MERCENARY_MARK: &str = "?";

/// The trade window's word while nobody of yours is at the station's
/// trading desk, the button beside it, and the desk's own menu row.
pub const NOT_AT_DESK: &str = "Nobody of yours is at the trading desk";
pub const WALK_TO_DESK: &str = "Walk over";
pub const TRADE_ROW: &str = "Trade";

/// The header's word while the crew's alarm is up, and what it means.
pub const ALARM_STATUS: &str = "To arms — an enemy is near";
pub const ALARM_TIP: &str = "An enemy within a hundred tiles of anybody, or a crew member hit in the last half minute: every crew member but the one you steer draws a weapon — out of the pack if the hand is empty — and fights, walking to wherever it can shoot from, until nobody is near and nobody has been hit for a while. The one you steer is yours: recruit it yourself, or leave it to its errands.";

pub const INVENTORY_TIP: &str = "What the Bim has on it: head, body and leg protection down the left, the weapon in hand, and the pack on its back — nine cells, one thing each. Recruited, a Bim is in combat mode — it draws the weapon and shoots at any enemy it can see and reach, leaning out from cover to do it, where half the shots at it miss. A blade within reach locks it in melee: the gun goes quiet and it fights with its fists until one of them is out of reach. Right-click a thing in the pack to put it on, put it away or throw it out; right-click a worn piece to take it off. Ctrl-click moves a thing straight into the open container, or out of one into the pack. Beside each slot is the bleeding on that part of the body, and a Bandage button that sends the crew member you steer to dress it.";

pub const CONTAINER_TIP: &str = "What is in the hold, by where it is kept: the armoury is the lockers — each piece of armour a cell of its own with its health under it, everything else a stack — the shelves take materials and, since a storage can hold armour and weapons as well, anything worn or held; the cold store is the food. Ctrl-click a thing to take one into the pack of the Bim shown; right-click for the rows. The Bim has to be within two tiles: clicking a container walks it over.";

pub const REACH_HINT: &str = "walk over first — it is out of reach";
pub const ACCURACY_TIP: &str = "The odds of a shot landing: its best out to the first distance, then falling in a straight line to the second at the weapon's range, beyond which it does not shoot.";
pub const DAMAGE_TIP: &str = "What a shot takes off the part it lands on, by how far it flew: its best out to the first distance, falling in a straight line to the second at the range. Armour on the part takes it first.";
pub const FIRE_RATE_TIP: &str = "How often the trigger goes. A burst weapon fires its burst a shot at a time and then recharges for the rest.";
pub const DPS_TIP: &str = "Damage a second with every shot landing up close: the shots a trigger pull, the fire rate and the damage a shot multiplied.";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_part_has_a_name_and_every_name_a_part() {
        assert_eq!(PART_NAMES.len(), PartKind::ALL.len());
        for &kind in PartKind::ALL.iter() {
            assert!(!part_name(kind).is_empty());
        }
    }

    #[test]
    fn every_resource_and_storage_class_has_a_name() {
        assert_eq!(RESOURCE_NAMES.len(), ResourceId::ALL.len());
        assert_eq!(STORAGE_NAMES.len(), shipdesign::Storage::ALL.len());
    }

    #[test]
    fn the_word_tables_are_as_long_as_the_generator_expects() {
        assert_eq!(STAR_WORDS.len(), worldgen::name::STAR_WORDS as usize);
        assert_eq!(STATION_WORDS.len(), worldgen::name::STATION_WORDS as usize);
    }

    #[test]
    fn every_issue_the_validator_can_raise_has_a_line() {
        // The codes are written out in `IssueCode` and never renumbered:
        // 1 to 12 and 20 to 34, with the gap on purpose.
        for code in (1..=12).chain(20..=34) {
            assert!(issue_line(code).is_some(), "issue {code} has no line");
        }
    }

    #[test]
    fn the_work_list_names_every_job() {
        assert_eq!(WORK_NAMES.len(), bims::work::Job::ALL.len());
    }

    #[test]
    fn the_readout_names_every_mess() {
        // Blood is the last kind; the table is indexed by the code.
        assert_eq!(
            MESS_NAMES.len(),
            bims::filth::Mess::Blood.code() as usize + 1
        );
    }

    #[test]
    fn every_weapon_has_a_name_and_the_empty_slot_is_first() {
        // Slot 0 is empty; every kind's code indexes its name.
        assert_eq!(WEAPON_NAMES.len(), bims::combat::WeaponKind::ALL.len() + 1);
        for &kind in bims::combat::WeaponKind::ALL.iter() {
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

    #[test]
    fn the_weapon_lines_say_the_curves_the_user_asked_for() {
        // The four sentences the Inventory was rewritten for, off the
        // real tables: a two-point curve, a flat one, a burst, a blade.
        use bims::combat::WeaponKind;
        let shotgun = WeaponKind::Shotgun.stats();
        assert_eq!(accuracy_text(&shotgun), "90% to 4 tiles, 60% at 10");
        assert_eq!(damage_text(&shotgun), "50 to 4 tiles, 30 at 10");
        let pistol = WeaponKind::LaserPistol.stats();
        assert_eq!(accuracy_text(&pistol), "95% up close, 65% at 12 tiles");
        assert_eq!(damage_text(&pistol), "6 a shot");
        assert_eq!(fire_rate_text(&pistol), "1.5 a second");
        let rifle = WeaponKind::AutoRifle.stats();
        assert_eq!(fire_rate_text(&rifle), "8 in 2 s, then 2 s");
        let schword = WeaponKind::Schword.stats();
        assert_eq!(melee_text(&schword), "Melee — 35 a swing every 2 s");
        assert!(!locked_tip().is_empty());
    }

    #[test]
    fn every_thing_in_a_cell_has_a_tip() {
        // A cell's tooltip is the resource's line; a blank one is an icon
        // nobody can ask about.
        assert_eq!(ITEM_TIPS.len(), ResourceId::ALL.len());
        for &id in ResourceId::ALL.iter() {
            assert!(!item_tip(id).is_empty(), "{id:?} has no tip");
        }
    }

    #[test]
    fn every_event_has_a_line() {
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
    }

    #[test]
    fn the_loot_window_has_a_label_for_every_cell() {
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

    #[test]
    fn every_part_of_a_body_has_a_name() {
        // `CrewHit` carries the part as a code, and the hit line runs it
        // into a sentence; the bandage menu names the same three.
        assert_eq!(BODY_PART_NAMES.len(), bims::health::Part::ALL.len());
        for part in bims::health::Part::ALL {
            assert!(!body_part_name(part.code()).is_empty());
        }
    }

    #[test]
    fn the_bandage_job_has_a_name_and_a_line() {
        // The newest job code, the one most likely to have been forgotten:
        // `bims::game::JOB_BANDAGE` on the agenda and the status line.
        let code = bims::game::JOB_BANDAGE;
        assert_ne!(job_name(code), job_name(u32::MAX));
        assert!(activity_line(code).is_some());
    }

    #[test]
    fn every_buildable_part_is_in_one_build_group() {
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
