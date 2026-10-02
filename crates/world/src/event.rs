//! What happened during a step.
//!
//! The same arrangement as the room's diary and the designer's issue list, and
//! for the same reason: **no strings cross the wasm boundary**, so an event is
//! a code and a number or two, and `EVENT_LINES` in `crates/app/src/names.rs` is where the
//! sentences live. An event whose code has no line there is *dropped* from the
//! page rather than shown as a placeholder, and what catches a missing one is
//! the row count against `ship_event_count()`.
//!
//! # Why there are events at all, when there is already a state
//!
//! A caller that wants to draw a power gauge reads the state. A caller that
//! wants to *say* "docked at Wana 231-4" has to watch for the crossing, and
//! polling the state for a change misses one that happened and reversed
//! inside a single update — which at 24x is an ordinary thing for a step to
//! contain.
//!
//! # Codes are never reused
//!
//! A code is written out and never renumbered, and a variant deleted leaves
//! its code free rather than closing the list up: the app's sentences are
//! matched by variant, not indexed by code, so nothing has to move with it.
//! Flight (1–5, 11–13, 50, 52–55), the radiation dose (17–26), the
//! food spoiling (58), the raids (59–62, 67), the plunder (64) and the
//! execution (48, which it shared with `UpgradeBegun` by mistake) went
//! with the old game (feature 104). Research (44–47, 65–66) went with
//! the research (feature 106). A recipe made and lost (14, 15), a piece
//! put on (34), a thing stowed (35), a body looted (38), the workbench's
//! upgrade and repair (48, 49, 74) and the buyback (102, 103) went with
//! the storage (task 113). Goods across a desk (8, 9) went with the desks
//! (task 114): gear is bought at a trader, on the map.

use bims::combat::ArmourKind;
use shipdesign::PartKind;
use worldgen::Node;

use crate::frame::Frame;

/// One thing that happened, in the order it happened.
#[derive(Clone, Copy, PartialEq, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum WorldEvent {
    /// Something in the system has been seen for the first time.
    Discovered { node: Node },
    /// The view is now about a particular place, or about space again.
    FrameChanged { frame: Frame },
    /// A command was not carried out. The reason is in the code.
    Refused { slot: u32, why: Refusal },
    /// A construction site was laid out for a part of `kind`, and is
    /// `site` from now on. See `crate::build`.
    SitePlaced { site: u32, kind: PartKind },
    /// A site was taken away again, nothing built.
    SiteCancelled { kind: PartKind },
    /// A Bim put a site together and the part is on the ship: its recipe
    /// out of the hold, the part in.
    Built { kind: PartKind },
    /// A Bim put a site together and nothing was built: the materials had
    /// gone from the hold, or the part would no longer go where it was
    /// laid out. The site is gone with the labour, and this says so.
    BuildLost { kind: PartKind },
    /// One of a hostile room's people went down under the crew's fire —
    /// which, with every human friendly (feature 104), no room's people
    /// are: a machine going down is [`WorldEvent::DroidDown`]. See
    /// `bims::combat`, and `World::visit`, which is where a hit crosses
    /// from the crew's room to theirs.
    EnemyDown { station: u32, who: u32 },
    /// An enemy's shot landed on a crew member. The hit is already on the
    /// body: the joined room applied it the step the bolt landed
    /// (`Game::take_wounds_taken`), and this is the world saying so, once
    /// per hit.
    CrewHit { who: u32 },
    /// A crew member is dead. Said the step it happens, whatever did it —
    /// a downed body's countdown run out, most often. Going down is not
    /// this: it is `CrewDowned`.
    CrewDown { who: u32 },
    /// A hit broke a worn piece: it is at nought, still worn, and does
    /// nothing from now on. Said once, the step it happens, off the room's
    /// `take_pieces_broken` — the room wounds its own the step a bolt
    /// lands, the way `CrewHit` is said.
    PieceBroke { who: u32, kind: ArmourKind },
    /// A crew member is locked in a melee: an enemy with a blade within
    /// reach (`bims::combat::MELEE_RANGE`), so they cannot fire and brawl
    /// instead — fists, or their own blade. Said the step the lock forms,
    /// once; walking out of reach breaks it, and the next lock is said
    /// again. `Game::is_locked` is the state.
    Locked { who: u32 },
    /// A mercenary was hired — `Command::Hire` — and is crew member `who`
    /// now, the first month paid. See `crate::mercenary`.
    Hired { who: u32 },
    /// A hired mercenary's month came round and was paid, `fee` euros.
    MercenaryPaid { who: u32, fee: u64 },
    /// A hired mercenary went unpaid — the month came round and the
    /// crew's money would not cover it — and is owed. Said once a month
    /// owed.
    MercenaryLeft { who: u32 },
    /// A hit took a crew member's bar to nothing and they are **downed**
    /// (task 120): lying where they fell with the countdown running until a
    /// crewmate revives them. Said the step it happens, off the room's
    /// `take_downs`; the room applied it the step the bolt landed, the way
    /// `CrewHit` is said.
    CrewDowned { who: u32 },
    /// A crewmate brought a downed crew member round: who, and by whom.
    CrewRevived { who: u32, by: u32 },
    /// The ship is in another system: the one round `star`, in empty space
    /// — a trip across a hyperlane on its way (`World::jump`).
    Jumped { star: u32 },
    /// No crew member is standing — dead or downed, every one — and
    /// the run is over. Said once.
    CrewLost,
    /// A crew member reached a level of its class (feature 74,
    /// `crate::class`): who, `Class`'s code, and the level. Said once a
    /// level; each is a skill point to spend on a rank.
    LevelUp { who: u32, class: u32, level: u32 },
    // `TalentPicked` (69) went with the talents (task 139); its code is
    // left free.
    /// An engineer laid a kit: who, and `crate::deploy::DeployKind`'s code.
    Deployed { who: u32, kind: u32 },
    /// An engineer packed a deployable up into a kit: who, and the kind.
    PackedUp { who: u32, kind: u32 },
    /// A deployable was destroyed — sandbags shot to nothing, a sentry
    /// drained — by the kind.
    DeployableLost { kind: u32 },
    /// A soldier braced, or stood easy again (feature 75): who, and
    /// which.
    Braced { who: u32, on: bool },
    /// A soldier threw a grenade: who.
    Thrown { who: u32 },
    /// A medic's heal beam linked to a crew member, or unlinked (feature
    /// 76): who, and the patient — `None` for the link broken, by the
    /// medic or by the world (out of range, out of sight, down).
    Beamed { who: u32, patient: Option<u32> },
    /// A tank stood as a wall, or stood down again (feature 77): who,
    /// and which.
    Bulwarked { who: u32, on: bool },
    /// A tank taunted: who.
    Taunted { who: u32 },
    // `Squadded` (81) went with the commander's squad orders; its code
    // is not used again.
    /// A commander rallied: who.
    Rallied { who: u32 },
    /// A commander called a Battle Cry (task 129): who.
    BattleCried { who: u32 },
    /// A commander brought his reinforcements to the mission's start
    /// (task 129): who, and how many stood by him — fewer than his rank
    /// gives where the deck round him was short.
    Reinforced { who: u32, count: u32 },
    /// A commander called a medic of the Republic's in with his C, the
    /// Medivac: who, and the crew member the medic is.
    Medivac { who: u32, medic: u32 },
    /// A fresh wave of machines has landed at a droid-held station
    /// (feature 83): which station.
    DroidReinforcements { station: u32 },
    /// The last machine of the last wave at a droid-held station has
    /// been destroyed: which station. Said once, and what
    /// `World::droid_station_cleared` answers for afterwards.
    DroidStationCleared { station: u32 },
    /// A machine has been destroyed: which station, which body of its
    /// room, and `bims::droid::DroidKind`'s code — said in place of
    /// [`WorldEvent::EnemyDown`] for a machine, since a droid has no
    /// name and the log would otherwise call one Sanne.
    DroidDown { station: u32, who: u32, kind: u32 },
    /// The crisis reached this system (feature 92): every station of the
    /// star it names has gone into the machines' hands. Said the step the
    /// flip happens, which is the first step after the star's day with no
    /// room of the crew's open in it.
    Infested { star: u32 },
    /// A player gave their bots a standing order (feature 84): which
    /// player's crew member, and `crate::Standing`'s code — nought for
    /// the order called off, which is the bots back to following.
    Ordered { who: u32, kind: u32 },
    /// A medic took a crewmate up into its arms, or set one down
    /// (feature 86): who is carrying, and whom — `None` for the body
    /// set down, whether by the player, by the medic's own judgement
    /// that it is clear of the fight, or by the world when one of the
    /// two went down.
    Carried { who: u32, patient: Option<u32> },
    /// The crew **held a town** against the machines (feature 94): the
    /// last machine of the last wave destroyed at the settlement it
    /// names. Said once, ever, for a town — a held town is never
    /// attacked again, and the crisis never flips it.
    TownHeld { station: u32 },
    /// And some of the town's survivors went with them: how many, said
    /// once, right after [`WorldEvent::TownHeld`].
    TownsfolkJoined { count: u32 },
    /// The Republic paid a bounty for an enemy taken down (feature 95):
    /// so many euros into the crew's one pool, said once for however many
    /// were downed this step. Fighting is how a crew earn.
    Bounty { amount: economy::Money },
    /// The Republic's bounty for machines destroyed at a site not yet
    /// cleared (feature 103): so many euros **pending**, paid the step
    /// the site is cleared and thrown away if the crew leave before.
    BountyPending { amount: economy::Money },
    /// A player put a destination to the crew, between missions.
    Proposed { slot: u32, star: u32, station: u32 },
    /// A player said yes to the destination on the table, or took it back.
    ProposalAccepted { slot: u32, yes: bool },
    /// The crew travelled and arrived: the world clock on by `minutes`,
    /// the ship docked or landed at the site, and a mission begun there.
    Travelled {
        star: u32,
        station: u32,
        minutes: u64,
    },
    /// A player pressed *Back to ship*.
    Returning { slot: u32 },
    /// The departure check is asking every player whether to leave
    /// `behind` of the crew outside the ship.
    DepartureAsked { behind: u32 },
    /// A player said no to leaving them: the ship stays.
    DepartureDeclined { slot: u32 },
    /// The ship left the site and the world map is up: `cleared` whether
    /// the site was — kept as it is — or not, and put back as the mission
    /// met it.
    LeftSite { station: u32, cleared: bool },
    /// A crew member left outside the ship when it went, and dead for it.
    LeftBehind { who: u32 },
    /// A bot died, gone for good. It costs the pool nothing: only a
    /// player's Bim is paid for (its buyback).
    BotLost { who: u32 },
    /// A town the machines were attacking was left before it was held,
    /// and fell to them: an infested site like any other.
    TownFell { station: u32 },
    /// The host said that player has left the game.
    PlayerGone { slot: u32 },
    /// Relics are on offer to the crew (feature 106): `count` of them, off
    /// an elite's site cleared — the reward screen.
    RelicsOffered { count: u32 },
    /// A player put a relic to the crew — `relic` `u32::MAX` for taking
    /// none — every acceptance cleared.
    RelicProposed { slot: u32, relic: u32 },
    /// A player said yes to the relic on the table, or took a yes back.
    RelicAccepted { slot: u32, yes: bool },
    /// The crew hold a relic for good, chosen off a reward.
    RelicGiven { relic: u32 },
    /// The crew chose to take none of the relics on offer.
    RelicsDeclined,
    // `RelicPending` (111), `RelicLost` (113), `CacheOpened` (114),
    // `RelicFired` (115), `RelicBought` (128), `Restocked` (129),
    // `RelicPicked` (145) and `RelicDice` (146) went with the old relics,
    // the caches, the trader's relic and the dice (October 2026); their
    // codes are left free.
    /// The run is won (`World::run_won`). Said once.
    RunWon,
    /// The Machine Heart's last conduit is down (feature 108): its core is
    /// exposed, sweeping its beam, and its fabricators are building.
    HeartExposed { station: u32 },
    /// The core has gone into its overload: two beams, faster, and the
    /// fabricators building faster.
    HeartOverload { station: u32 },
    /// The core is destroyed. The run is won with it.
    HeartDestroyed { station: u32 },
    /// Crew member `who`'s loadout changed between missions (task 113):
    /// a thing onto or off its slot `part` (a `GearSlot` code).
    GearChanged { who: u32, part: u32 },
    /// Player `from` offered what its Bim has on `part` to player `to`.
    GearOffered { from: u32, part: u32, to: u32 },
    /// Player `to` took the offer: the thing is on its Bim now.
    OfferTaken { from: u32, part: u32, to: u32 },
    /// An offer is gone untaken: declined, taken back, or one of the
    /// two slots changed.
    OfferWithdrawn { from: u32, part: u32, to: u32 },
    /// A research key picked up, counted the moment it was: `keys` is
    /// how many the crew hold now.
    KeyFound { keys: u32 },
    /// A player's Bim that died is back at the mission's end, with its
    /// whole loadout, and the pool paid `paid` for it — the buyback, or
    /// what was left of the pool, which never goes below nought.
    Respawned { who: u32, paid: economy::Money },
    /// Player `slot` bought what was in the shelf's slot `index` at the
    /// trader the crew are at (task 114), for crew member `to`'s loadout
    /// — or the armory, `u32::MAX`.
    ShelfBought { slot: u32, index: u32, to: u32 },
    /// Player `slot` combined two things into one of `tier` at a trader:
    /// onto crew member `who`, or into the armory, `u32::MAX`.
    Combined { slot: u32, who: u32, tier: u32 },
    /// Crew member `who` bought a rank of its ranked kit's ability
    /// `ability_slot` (0 to 3, Q C E R) with a skill point, and it is at
    /// `rank` now (task 124, `Command::RankUp`).
    RankedUp {
        who: u32,
        /// The crew member's class, by code, for the sentence's name.
        class: u32,
        ability_slot: u32,
        rank: u32,
    },
    /// Player `who`'s soldier went on a Rampage (task 124).
    Rampaged { who: u32 },
    /// A tank went Juggernaut (task 139): who.
    Juggernaut { who: u32 },
    /// Player `who`'s engineer threw an EMP (task 127, `Command::Emp`).
    EmpThrown { who: u32 },
    /// A medic's Nanite Burst went off (task 130): who, and how many
    /// friendly Bims it healed, the medic among them.
    NaniteBurst { who: u32, healed: u32 },
    /// A medic cloaked a crew member (task 130): who, and whom — the
    /// medic itself, or a crewmate.
    Cloaked { who: u32, target: u32 },
    /// A player pressed *Ready* for the mission held for the ready check,
    /// or took it back.
    Readied { slot: u32, yes: bool },
    /// Every connected player is ready: the mission held for the ready
    /// check is under way.
    AllReady,
    /// A crew member brought one of a station's people round with the
    /// medkit — a townsperson downed defending its town: which station,
    /// who of its people, and by whom of the crew.
    ResidentRevived { station: u32, who: u32, by: u32 },
    /// An enemy went down and what it was worth, said once, the step it is
    /// counted: which station, which body of its room, the experience
    /// every classed crew member in range was given and the Republic's
    /// bounty for it (paid or pending). A picture's event — the numbers
    /// the app floats over the body — beside `Bounty`, which is the money.
    EnemyRewarded {
        station: u32,
        who: u32,
        xp: u32,
        money: economy::Money,
    },
    /// A hit landed: on one of the station's bodies (`resident`, a body
    /// of its room — a machine past its Bims) or on one of the crew, how
    /// much, whole points, and whether it was a critical one. A picture's
    /// event, for the red number over the body; nothing reads it.
    Hit {
        resident: bool,
        who: u32,
        damage: u32,
        crit: bool,
    },
    /// A player's Bim went off on its *Blink Drive* (October 2026): it
    /// stands where the drive put it.
    Blinked { who: u32 },
    /// A player's Bim used an active item other than the blink — a *Field
    /// Mender*, a *Reset Capacitor*, an *Ablative Shell* (October 2026):
    /// whose, and the item's kind's code.
    ItemUsed { who: u32, kind: u32 },
    /// A player bought an item at a trader (October 2026): which kind at
    /// which tier, and onto which Bim — `u32::MAX` the armory.
    ItemBought {
        slot: u32,
        kind: u32,
        tier: u32,
        to: u32,
    },
}

/// Why a command did nothing.
///
/// Written out and never renumbered, like the events: a refusal deleted
/// leaves its code free (research's, 22–25, 39 and 40, feature 106; the
/// helm's and the flight's, 5, 6, 8, 10, 13 and
/// 28–32, 78 and 83, the hire's bunk, 20, the execution's and the
/// plunder's, 26 and 27, and a walk through the test room's locked heads
/// door, 37, went with the old game in feature 104; the hold's, the
/// pack's, the loot's and the workbench's, 3, 15–18, 34–36, 51 and 81,
/// went with the storage in task 113; the desk's, 21, with the desks in task 114;
/// a rally too early, 71, with the commander's talents in task 129; the
/// surge's, 65–67, with the surge in task 130; the cache's and the
/// restock's, 96, 108 and 109, with the old relics in October 2026).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u32)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Refusal {
    /// Trading anywhere but at a station. Money only works at a dock — see
    /// `shipdesign::materials` for the rule and why.
    NotDocked = 1,
    /// Not the money for it.
    Unaffordable = 2,
    /// Selling more than is aboard, or more than is not spoken for by the
    /// construction sites. And, since the pack, the thing asked for not
    /// being there at all: a fetch of a piece
    /// the hold has not got, a stow or an equip of an empty cell, a stack
    /// where a piece was wanted.
    NotAboard = 4,
    /// The sum would not fit in a `Money`. A refusal rather than a wrap, the
    /// same as everywhere else money is added up — see `crates/economy`.
    SumTooBig = 7,
    /// The station the ship is docked at does not sell that —
    /// `worldgen::StationKind::sells`. Galvum is the outposts' alone, an
    /// emitter is nobody's, and a derelict has nobody to sell anything.
    NotSoldHere = 9,
    /// A site the rules would not put there, or that would leave the ship
    /// with a fault it has not got. `World::can_place_site` says which,
    /// before anything is sent.
    WontFit = 11,
    /// A site that is not there: built, or cancelled already.
    NoSuchSite = 12,
    /// A stow or a fetch with the crew member further than
    /// [`crate::data::REACH`] tiles from any container that takes the
    /// thing — or with no such crew member: dead, outside in a suit, or
    /// not aboard. Walk over first.
    OutOfReach = 14,
    /// A hire of somebody who is not a mercenary for hire — one of the
    /// station's own people, or nobody at all.
    NotForHire = 19,
    /// A sale at a station with nobody to buy: a derelict keeps no desk
    /// (`crate::station::market_kind`). A buy there is
    /// [`Refusal::NotSoldHere`] first, since it stocks nothing either.
    NoMarket = 33,
    /// A walk ordered somewhere there is no way to at all —
    /// `Command::Crew`, the room's `ORDER_NOWHERE`.
    NoWayThere = 38,
    /// A class chosen after the ship first left its berth: a class is
    /// chosen at the start (`Command::SetClass`, `crate::class`).
    ClassLocked = 41,
    /// A deploy, a pack-up or a repair by a crew member that is
    /// not an engineer, or a pick by one with no class.
    NotAnEngineer = 42,
    /// A deploy with no such kit in the pack.
    NoKit = 43,
    /// A sentry laid before the engineer's third level.
    NoSentryYet = 44,
    /// A deploy on a tile that will not take it: not reachable deck
    /// floor, a door or an airlock, a part in the way, or a deployable
    /// there already.
    CantDeployThere = 46,
    /// A pack-up or a strike of a deployable that is not there.
    NoSuchDeployable = 47,
    // 48, 49 and 50 were a talent's pick refused — no pick level, not
    // reached, picked already — and went with the talents (task 139).
    /// A rank asked for a crew member with no class.
    NoClass = 52,
    /// A brace or a throw by a crew member that is not a soldier
    /// (feature 75).
    NotASoldier = 53,
    /// A throw with no grenade in the pack.
    NoGrenade = 54,
    /// A throw before the soldier's third level.
    NoGrenadesYet = 55,
    /// A throw within the cooldown of the last, or a taunt within the
    /// cooldown of the last.
    CoolingDown = 56,
    /// A throw at a tile beyond the grenade's range.
    OutOfThrowRange = 57,
    /// A throw at a tile with a wall or a shut door in the way.
    NoLineToTile = 58,
    /// A throw at a tile that is not deck of the room.
    CantThrowThere = 59,
    /// A beam, a Nanite Burst or a cloak by a crew member that is not a
    /// medic (feature 76; task 130).
    NotAMedic = 60,
    /// A beam with no crew member under the pointer.
    NoPatient = 61,
    /// A beam on somebody that is not a crewmate: the medic itself, or
    /// an enemy. And a loot of a body that is not a crewmate's: one of a
    /// station's people, whose dead are left as they lie — every human
    /// is friendly (feature 104), and a machine carries nothing.
    NotACrewmate = 62,
    /// A beam on a crewmate beyond the beam's range.
    OutOfBeamRange = 63,
    /// A beam on a crewmate the medic cannot see.
    NoSightOfPatient = 64,
    // 65, 66 and 67 were the surge's — too early, not charged, nobody
    // linked — and went with it (task 130).
    /// A bulwark, a taunt or a Juggernaut by a crew member that is not a
    /// tank (features 77 and 139).
    NotATank = 68,
    // 69, a taunt before the tank's third level, went with his talents
    // (task 139): an ability not learnt is `NotLearnt`.
    /// A rally or a battle cry by a crew member that is not a
    /// commander (feature 78).
    NotACommander = 70,
    // 71, a rally before the commander's third level, went with his
    // talents (task 129): an ability not learnt is `NotLearnt`.
    // 72 and 73, nobody of the squad in range and no enemy under the
    // pointer, went with the commander's squad orders.
    /// An attack banner put down on something that is not deck of the
    /// crew's room (feature 84).
    NoGroundThere = 74,
    /// A carry by a crew member that is neither a medic of the class
    /// nor a hired field medic (feature 86), or a set down by one that
    /// is carrying nobody.
    NotCarrying = 75,
    /// A carry of a body that wants none: whole, on its feet and awake.
    /// A crewmate is picked up to be taken out of the fire, not to be
    /// moved about.
    NotHurt = 76,
    /// A carry of a body that is already in somebody's arms.
    AlreadyCarried = 77,
    /// A trip **inward** — to a star fewer hops from the machines' origin
    /// than this one — out of an infested system whose jammer still
    /// stands (feature 93, `crate::jammer`): the travel quote's refusal.
    /// Sideways and outward are accepted, and a trip *into* an infested
    /// system is never refused.
    Jammed = 79,
    /// A construction site the crew cannot pay for: its part's price is
    /// more than the pool has left after the sites already begun
    /// (feature 95, `crate::build`). The site stands and waits; money
    /// earned or a site cancelled lets it go on.
    NotEnoughMoney = 80,
    /// A construction site placed in a run (feature 102): the ship is the
    /// default one and nothing is built onto it but a class's
    /// deployables (`World::shipyard_enabled`).
    NoShipyard = 82,
    /// Anything but choosing a destination between missions: the map is
    /// up and nobody is anywhere to be ordered about.
    BetweenMissions = 84,
    /// A destination put or accepted during a mission: the map is
    /// read-only until everybody is back aboard and the ship has left.
    MidMission = 85,
    /// A destination that is no station or settlement of the system named.
    NoSuchPlace = 86,
    /// A destination further than one hyperlane hop: a trip is one hop at
    /// most.
    TooFar = 87,
    /// An acceptance with nothing on the table.
    NoProposal = 88,
    /// An answer to the departure check with no check asking.
    NotAsked = 89,
    /// *Back to ship* from a player whose Bim is dead: it comes back at
    /// the next mission, if the pool can pay.
    PlayerOut = 90,
    /// A trip the ship cannot make: nothing pushes it, forwards or back.
    CannotTravel = 91,
    /// A trip to the site the crew are at (feature 105): the next
    /// destination is always somewhere else, so the world clock moves
    /// before a site is fought again.
    AlreadyHere = 92,
    /// A relic proposed or accepted with no relic choice being made
    /// (feature 106).
    NoRelicChoice = 93,
    /// A relic proposed that is not among those on offer.
    NotOnOffer = 94,
    /// A command only a player's own Bim may give — a relic proposed,
    /// an item used — given for one no player steers.
    NotAPlayer = 95,
    /// A destination proposed while the crew are still choosing a relic:
    /// the map comes up once they have.
    ChoosingRelic = 97,
    /// A loadout or armory command in a mission (task 113): gear changes
    /// hands between missions, on the map and the reward screen.
    GearLocked = 98,
    /// A loadout command on another player's Bim: a player changes its
    /// own and the bots', and gives another player a thing by offering it.
    NotYours = 99,
    /// A thing asked for that is not there: nothing under that id in the
    /// armory, or nothing on that slot.
    NoSuchGear = 100,
    /// An answer to an offer nobody made.
    NoOffer = 101,
    /// A trader proposed while its system is the machines' and not
    /// liberated (task 114): closed, until every infested site of it is
    /// cleared.
    TraderClosed = 102,
    /// A trader proposed that the crisis will have closed by the day the
    /// crew would arrive.
    ClosedOnArrival = 103,
    /// A purchase, a combining or a trader's relic asked for anywhere
    /// but at a trader.
    NotAtATrader = 104,
    /// A thing off the shelf that is not there — bought already, or the
    /// trader's relic gone: the first command to want it had it.
    SoldOut = 105,
    /// Two things combined at tier three: there is no tier past it.
    TopTier = 106,
    /// Two things combined that are not two of one kind at one tier.
    NotAPair = 107,
    /// A rank asked of a slot past R (task 124; every class has a ranked
    /// kit since task 139).
    NoRankedKit = 110,
    /// A rank asked with no skill point to spend on it.
    NoSkillPoint = 111,
    /// A rank asked of an ability already at its top rank.
    TopRank = 112,
    /// A rank asked before the level it wants is reached.
    RankLocked = 113,
    /// An ability of a ranked kit used at rank nought: not learnt yet.
    NotLearnt = 114,
    /// An ability used while it is already running: a Rampage on top of
    /// a Rampage.
    AlreadyActive = 115,
    /// A cloak on a crewmate beyond its reach (task 130).
    OutOfCloakRange = 116,
    /// A cloak on a crewmate the medic cannot see (task 130).
    NoSightOfTarget = 117,
    /// A class ability used by a crew member under a cloak (task 130): a
    /// cloaked Bim fires nothing and uses no ability.
    Cloaked = 118,
    /// Anything but a vote, the loadouts or *Ready* while a mission is
    /// held for the ready check: nothing has started yet.
    AwaitingReady = 119,
    /// *Ready* pressed with no ready check running.
    NoReadyCheck = 120,
    /// Anything but *Back to ship*, the loadouts or a rank once the
    /// mission's fight is won (task 133): the deck is frozen.
    FightOver = 121,
    /// A trip to a system's Attack or Defend site once the crew have
    /// fought its other one (task 135): one fight a system.
    OtherSiteChosen = 122,
    /// An item asked of a slot with none in it, or with one whose key does
    /// nothing (October 2026).
    NoSuchItem = 123,
    /// A *Blink Drive* within a few seconds of a hit taken.
    BlinkLocked = 124,
    /// A *Blink Drive* aimed where there is no ground the crew see within
    /// its reach.
    NowhereToBlink = 125,
    /// An item onto a Bim whose four item slots are full.
    ItemsFull = 126,
    /// An item onto a bot: only a player's Bim carries one.
    BotsCarryNoItems = 127,
    /// An item asked of a trader that does not sell it at that tier today.
    NotForSale = 128,
}

impl Refusal {
    pub fn code(self) -> u32 {
        self as u32
    }
}

impl WorldEvent {
    /// The number that crosses the wasm boundary, indexing `EVENT_LINES`.
    ///
    /// Written out and never renumbered, the same as every other code table
    /// in this workspace.
    pub fn code(self) -> u32 {
        match self {
            WorldEvent::Discovered { .. } => 6,
            WorldEvent::FrameChanged { .. } => 7,
            WorldEvent::Refused { .. } => 10,
            // 14 and 15 were a recipe made and lost, 34, 35 and 38 a piece put
            // on, a thing stowed and a body looted, 48 and 49 the workbench's
            // upgrade, 74 its repair, and 102 and 103 a buyback and a player
            // still out: all went with the storage (task 113).
            // 1 to 5 and 11 to 13 are free: flight's (feature 104). 16 was
            // `Mined`, which went with the mining (feature 95), and 17 to
            // 26 the radiation dose's (feature 104).
            // 133 was a sentry standing its time out, gone when the sentry
            // came to stand until it is destroyed.
            WorldEvent::SitePlaced { .. } => 27,
            WorldEvent::SiteCancelled { .. } => 28,
            WorldEvent::Built { .. } => 29,
            WorldEvent::BuildLost { .. } => 30,
            WorldEvent::EnemyDown { .. } => 31,
            WorldEvent::CrewHit { .. } => 32,
            WorldEvent::CrewDown { .. } => 33,
            WorldEvent::PieceBroke { .. } => 36,
            WorldEvent::Locked { .. } => 37,
            WorldEvent::Hired { .. } => 39,
            WorldEvent::MercenaryPaid { .. } => 40,
            WorldEvent::MercenaryLeft { .. } => 41,
            WorldEvent::CrewDowned { .. } => 42,
            WorldEvent::CrewRevived { .. } => 43,
            // 50 and 52 to 55 were the charge and the landing (feature 104).
            WorldEvent::Jumped { .. } => 51,
            // 56 and 57 are free: the brownout's and the power coming back,
            // which went with the electricity (October 2026).
            // 58 was the food spoiling, 59 to 62 and 67 the raids and 64
            // the plunder (feature 104).
            WorldEvent::CrewLost => 63,
            WorldEvent::LevelUp { .. } => 68,
            WorldEvent::Deployed { .. } => 70,
            WorldEvent::PackedUp { .. } => 71,
            WorldEvent::DeployableLost { .. } => 72,
            WorldEvent::Braced { .. } => 75,
            WorldEvent::Thrown { .. } => 76,
            WorldEvent::Beamed { .. } => 77,
            // 78 was a surge, which went with it (task 130).
            WorldEvent::Bulwarked { .. } => 79,
            WorldEvent::Taunted { .. } => 80,
            // 81 was a squad order, which went with them.
            WorldEvent::Rallied { .. } => 82,
            WorldEvent::DroidReinforcements { .. } => 83,
            WorldEvent::DroidStationCleared { .. } => 84,
            WorldEvent::DroidDown { .. } => 85,
            WorldEvent::Ordered { .. } => 86,
            WorldEvent::Carried {
                patient: Some(_), ..
            } => 87,
            WorldEvent::Carried { patient: None, .. } => 88,
            WorldEvent::Infested { .. } => 89,
            WorldEvent::TownHeld { .. } => 90,
            WorldEvent::TownsfolkJoined { .. } => 91,
            WorldEvent::Bounty { .. } => 92,
            WorldEvent::BountyPending { .. } => 93,
            WorldEvent::Proposed { .. } => 94,
            WorldEvent::ProposalAccepted { .. } => 95,
            WorldEvent::Travelled { .. } => 96,
            WorldEvent::Returning { .. } => 97,
            WorldEvent::DepartureAsked { .. } => 98,
            WorldEvent::DepartureDeclined { .. } => 99,
            WorldEvent::LeftSite { .. } => 100,
            WorldEvent::LeftBehind { .. } => 101,
            WorldEvent::BotLost { .. } => 104,
            WorldEvent::TownFell { .. } => 105,
            WorldEvent::PlayerGone { .. } => 106,
            WorldEvent::RelicsOffered { .. } => 107,
            WorldEvent::RelicProposed { .. } => 108,
            WorldEvent::RelicAccepted { .. } => 109,
            WorldEvent::RelicGiven { .. } => 110,
            WorldEvent::RelicsDeclined => 112,
            WorldEvent::RunWon => 116,
            WorldEvent::HeartExposed { .. } => 117,
            WorldEvent::HeartOverload { .. } => 118,
            WorldEvent::HeartDestroyed { .. } => 119,
            WorldEvent::GearChanged { .. } => 120,
            WorldEvent::GearOffered { .. } => 121,
            WorldEvent::OfferTaken { .. } => 122,
            WorldEvent::OfferWithdrawn { .. } => 123,
            WorldEvent::KeyFound { .. } => 124,
            WorldEvent::Respawned { .. } => 125,
            WorldEvent::ShelfBought { .. } => 126,
            WorldEvent::Combined { .. } => 127,
            WorldEvent::RankedUp { .. } => 130,
            WorldEvent::Rampaged { .. } => 131,
            WorldEvent::Juggernaut { .. } => 141,
            WorldEvent::EmpThrown { .. } => 132,
            WorldEvent::BattleCried { .. } => 134,
            WorldEvent::Reinforced { .. } => 135,
            WorldEvent::Medivac { .. } => 144,
            WorldEvent::NaniteBurst { .. } => 136,
            WorldEvent::Cloaked { .. } => 137,
            WorldEvent::Readied { .. } => 138,
            WorldEvent::AllReady => 139,
            WorldEvent::ResidentRevived { .. } => 140,
            WorldEvent::EnemyRewarded { .. } => 142,
            WorldEvent::Hit { .. } => 143,
            WorldEvent::Blinked { .. } => 147,
            WorldEvent::ItemUsed { .. } => 149,
            WorldEvent::ItemBought { .. } => 148,
        }
    }

    /// The one number the sentence needs, if it needs one: a station id, a
    /// reason code, a node id, a number of units. The host knows which of
    /// those it is from the code, exactly as `MEMORY_LINES` does.
    pub fn value(self) -> i64 {
        match self {
            WorldEvent::DroidReinforcements { station }
            | WorldEvent::DroidStationCleared { station }
            | WorldEvent::TownHeld { station } => station as i64,
            WorldEvent::TownsfolkJoined { count } => count as i64,
            WorldEvent::Bounty { amount } | WorldEvent::BountyPending { amount } => amount as i64,
            // The station in the thousands, the player in the units — the
            // star is the log's to look up, a site being named by both.
            WorldEvent::Proposed { slot, station, .. } => (slot as i64) + 1_000 * (station as i64),
            WorldEvent::ProposalAccepted { slot, yes } => (slot as i64) + 100 * i64::from(yes),
            // The minutes: how long the trip was.
            WorldEvent::Travelled { minutes, .. } => minutes as i64,
            WorldEvent::Returning { slot }
            | WorldEvent::DepartureDeclined { slot }
            | WorldEvent::PlayerGone { slot } => slot as i64,
            WorldEvent::DepartureAsked { behind } => behind as i64,
            WorldEvent::LeftSite { station, cleared } => (station as i64) * 2 + i64::from(cleared),
            WorldEvent::LeftBehind { who } => who as i64,
            WorldEvent::BotLost { who } => who as i64,
            // The buyback paid in the hundreds: a crew is never a hundred.
            WorldEvent::Respawned { who, paid } => (who as i64) + 100 * (paid as i64),
            // The slot in the tens, the Bim in the units: four slots, and a
            // crew is never ten (task 113).
            WorldEvent::GearChanged { who, part } => (who + 10 * part) as i64,
            // An offer: the giver in the units, the slot in the tens, the
            // receiver in the hundreds.
            WorldEvent::GearOffered { from, part, to }
            | WorldEvent::OfferTaken { from, part, to }
            | WorldEvent::OfferWithdrawn { from, part, to } => (from + 10 * part + 100 * to) as i64,
            WorldEvent::KeyFound { keys } => keys as i64,
            // The buyer in the units, the slot in the tens and the Bim plus
            // one in the thousands, nought for the armory (task 114).
            WorldEvent::ShelfBought { slot, index, to } => {
                let to = if to == u32::MAX { 0 } else { to as i64 + 1 };
                (slot as i64) + 10 * (index as i64) + 1_000 * to
            }
            WorldEvent::Combined { slot, who, tier } => {
                let who = if who == u32::MAX { 0 } else { who as i64 + 1 };
                (slot as i64) + 10 * (tier as i64) + 100 * who
            }
            // The slot in the hundreds and the rank in the ten thousands, the
            // crew member in the units: a crew is never a hundred.
            WorldEvent::RankedUp {
                who,
                ability_slot,
                rank,
                ..
            } => (who as i64) + 100 * (ability_slot as i64) + 10_000 * (rank as i64),
            WorldEvent::Rampaged { who } => who as i64,
            WorldEvent::Juggernaut { who } => who as i64,
            WorldEvent::EmpThrown { who } => who as i64,
            WorldEvent::TownFell { station }
            | WorldEvent::HeartExposed { station }
            | WorldEvent::HeartOverload { station }
            | WorldEvent::HeartDestroyed { station } => station as i64,
            WorldEvent::RelicGiven { relic } => relic as i64,
            // The relic plus one in the hundreds, nought being none.
            WorldEvent::RelicProposed { slot, relic } => {
                let relic = if relic == u32::MAX {
                    0
                } else {
                    relic as i64 + 1
                };
                (slot as i64) + 100 * relic
            }
            WorldEvent::RelicAccepted { slot, yes } => (slot as i64) + 100 * i64::from(yes),
            WorldEvent::RelicsOffered { count } => count as i64,
            WorldEvent::RelicsDeclined | WorldEvent::RunWon | WorldEvent::AllReady => 0,
            WorldEvent::Readied { slot, yes } => (slot as i64) + 100 * i64::from(yes),
            // The station in the thousands, the person in the units.
            WorldEvent::EnemyDown { station, who } => (who + 1_000 * station) as i64,
            // The kind in the hundreds and the body under it. The station is
            // left out: the log says which machine, not whose.
            WorldEvent::DroidDown { who, kind, .. } => (who + 100 * kind) as i64,
            // The order's code in the hundreds, nought being the
            // following every slot starts on.
            WorldEvent::Ordered { who, kind } => (who as i64) + 100 * (kind as i64),
            WorldEvent::CrewHit { who } => who as i64,
            WorldEvent::CrewDowned { who } => who as i64,
            // The helper in the **hundreds**, the one brought round in the
            // units, the way a carry is packed.
            WorldEvent::CrewRevived { who, by } => (who + 100 * by) as i64,
            // The station in the millions, the helper in the thousands, the
            // one brought round in the units.
            WorldEvent::ResidentRevived { station, who, by } => {
                who as i64 + 1_000 * by as i64 + 1_000_000 * station as i64
            }
            // The money: the body is in the event for the picture.
            WorldEvent::EnemyRewarded { money, .. } => money as i64,
            WorldEvent::Hit { damage, .. } => i64::from(damage),
            WorldEvent::Blinked { who } => who as i64,
            // The kind in the hundreds, the player in the units.
            WorldEvent::ItemUsed { who, kind } => (who as i64) + 100 * (kind as i64),
            // The buyer in the units, the kind in the tens, the tier in the
            // hundreds and the Bim plus one in the thousands, nought for the
            // armory.
            WorldEvent::ItemBought {
                slot,
                kind,
                tier,
                to,
            } => {
                let to = if to == u32::MAX { 0 } else { to as i64 + 1 };
                (slot as i64) + 10 * (kind as i64) + 100 * (tier as i64) + 1_000 * to
            }
            // The kind in the tens the same way: three kinds, and a crew
            // is never ten.
            WorldEvent::PieceBroke { who, kind } => (who + 10 * kind.code()) as i64,
            // The one carried in the **hundreds**, the carrier in the
            // units, and the carrier alone for a set down, whose line
            // names nobody else (feature 86). The hundreds rather than
            // the tens the rows above use: the `combat` command sails
            // with fourteen.
            WorldEvent::Carried { who, patient } => (who + 100 * patient.unwrap_or(0)) as i64,
            // The fee in the hundreds: a crew is never a hundred.
            WorldEvent::MercenaryPaid { who, fee } => (who as i64) + 100 * (fee as i64),
            WorldEvent::CrewDown { who }
            | WorldEvent::Locked { who }
            | WorldEvent::Hired { who }
            | WorldEvent::MercenaryLeft { who } => who as i64,
            WorldEvent::SitePlaced { kind, .. }
            | WorldEvent::SiteCancelled { kind }
            | WorldEvent::Built { kind }
            | WorldEvent::BuildLost { kind } => kind.code() as i64,
            WorldEvent::Discovered { node } => match node {
                Node::Body(id) | Node::Station(id) => id as i64,
            },
            WorldEvent::FrameChanged { frame } => frame.code() as i64,
            WorldEvent::Refused { why, .. } => why.code() as i64,
            // The star: a galaxy has a thousand.
            WorldEvent::Jumped { star } | WorldEvent::Infested { star } => star as i64,
            WorldEvent::CrewLost => 0,
            // The level, the talent and the kind in the hundreds, the same
            // way: a crew is never a hundred.
            WorldEvent::LevelUp { who, level, .. } => (who as i64) + 100 * (level as i64),
            WorldEvent::Deployed { who, kind } | WorldEvent::PackedUp { who, kind } => {
                (who as i64) + 100 * (kind as i64)
            }
            WorldEvent::DeployableLost { kind } => kind as i64,
            WorldEvent::Thrown { who }
            | WorldEvent::Taunted { who }
            | WorldEvent::Rallied { who }
            | WorldEvent::BattleCried { who } => who as i64,
            // The count in the hundreds: a crew is never a hundred.
            WorldEvent::Reinforced { who, count } => (who as i64) + 100 * (count as i64),
            // The medic in the hundreds.
            WorldEvent::Medivac { who, medic } => (who as i64) + 100 * (medic as i64),
            // How many it healed, and whom it cloaked, in the hundreds.
            WorldEvent::NaniteBurst { who, healed } => (who as i64) + 100 * (healed as i64),
            WorldEvent::Cloaked { who, target } => (who as i64) + 100 * (target as i64),
            WorldEvent::Braced { who, on } | WorldEvent::Bulwarked { who, on } => {
                (who as i64) + 100 * i64::from(on)
            }
            // The patient plus one in the hundreds: nought is the link
            // broken.
            WorldEvent::Beamed { who, patient } => {
                (who as i64) + 100 * patient.map_or(0, |p| p as i64 + 1)
            }
        }
    }
}
