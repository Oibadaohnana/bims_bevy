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
//! with the old game (feature 104).

use bims::combat::ArmourKind;
use physics::ResourceId;
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
    /// Goods aboard. `units` is negative for a sale.
    Traded {
        slot: u32,
        resource: ResourceId,
        units: i64,
    },
    /// A command was not carried out. The reason is in the code.
    Refused { slot: u32, why: Refusal },
    /// A Bim finished a recipe at a bench and the cargo moved:
    /// `shipdesign::recipes::RECIPES[recipe]`'s inputs out, its output in.
    Crafted { recipe: u32 },
    /// A Bim finished a recipe and nothing was made: the inputs had gone
    /// from the hold, or there was no longer room for the output. The
    /// labour is lost, and this says so.
    CraftLost { recipe: u32 },
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
    /// An enemy's shot landed on a crew member, on that part of them —
    /// `health::Part`'s code — and opened a wound there. The wound is
    /// already on the body: the joined room applied it the step the bolt
    /// landed (`Game::take_wounds_taken`), and this is the world saying
    /// so, once per hit.
    CrewHit { who: u32, part: u32 },
    /// A crew member is dead. Said the step it happens, whatever did it —
    /// bled out through a wound nobody dressed or a trauma nobody
    /// treated, or the room's own hunger. A part shot to nothing is not
    /// this any more: it is `CrewDying`.
    CrewDown { who: u32 },
    /// A crew member put a piece of armour on, out of their pack —
    /// `Command::Equip`. Whatever was worn there is in the pack now. A
    /// weapon swapped the same way says nothing: there is one weapon.
    Equipped { who: u32, kind: ArmourKind },
    /// A crew member put something from their pack into a container —
    /// `Command::Stow` — and the hold's count moved.
    Stowed { who: u32 },
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
    /// A crew member took something off a body — `Command::Loot`: a
    /// dead or unconscious crewmate's, `source_kind` being
    /// `crate::armour::LootSource::code` (always a crewmate's since
    /// feature 104). It is in `who`'s pack now; a piece of armour keeps
    /// the health it had.
    Looted { who: u32, source_kind: u32 },
    /// A mercenary was hired — `Command::Hire` — and is crew member `who`
    /// now, the first month paid. See `crate::mercenary`.
    Hired { who: u32 },
    /// A hired mercenary's month came round and was paid, `fee` euros.
    MercenaryPaid { who: u32, fee: u64 },
    /// A hired mercenary went unpaid — the month came round and the
    /// crew's money would not cover it — and is owed. Said once a month
    /// owed.
    MercenaryLeft { who: u32 },
    /// A hit took a crew member's part to nothing and they are **dying**:
    /// in the state `trauma` — `bims::health::Trauma`'s code — until a
    /// crewmate treats it with a medkit. Said the step it happens, off the
    /// room's `take_traumas`, one per trauma; the room applied it the step
    /// the bolt landed, the way `CrewHit` is said.
    CrewDying { who: u32, trauma: u32 },
    /// A crewmate's medkit took a crew member out of the dying state
    /// `trauma`. Whatever it leaves behind is on the body now.
    CrewTreated { who: u32, trauma: u32 },
    /// A crew member took the research key off a station's research
    /// desk — `Command::TakeKey`. It is in their pack now, and the desk
    /// is bare.
    KeyTaken { who: u32 },
    /// A research key was consumed at the crew's research desk and node
    /// `node`'s lock is open — `Command::Unlock`. See
    /// `shipdesign::research`.
    Unlocked { node: u32 },
    /// The AI went onto a node of the research tree — the head of the
    /// queue, the step it fell idle with one there; `node` is
    /// `shipdesign::research::Node`'s code.
    ResearchBegun { node: u32 },
    /// The AI finished a node: what it gates can be built and made now.
    Researched { node: u32 },
    /// Two of a kind at the same tier went onto the workbench, out of the
    /// hold, to become one of tier `tier` — `World::upgrade`; `resource`
    /// is what they count as in the hold, as a `ResourceId` code.
    UpgradeBegun { resource: u32, tier: u32 },
    /// The upgrade came off the workbench into the hold: one `resource` at
    /// `tier`.
    Upgraded { resource: u32, tier: u32 },
    /// The ship is in another system: the one round `star`, in empty space
    /// — a trip across a hyperlane on its way (`World::jump`).
    Jumped { star: u32 },
    /// The batteries went flat under an overdraw and the ship is browned
    /// out — see `World::run_brownout`: the lamps are dark and the
    /// benches have stopped. Said once, the step it starts.
    Brownout,
    /// The brownout is over: the reactors cover the draw again, or a
    /// battery has something in it. What stopped is running again.
    PowerRestored,
    /// No crew member is standing — dead or out cold, every one — and
    /// the run is over. Said once.
    CrewLost,
    /// A node went onto the research queue — `Command::Research`, one for
    /// the node asked for and one for each prerequisite queued ahead of
    /// it; `node` is `shipdesign::research::Node`'s code.
    ResearchQueued { node: u32 },
    /// A node came off the research queue without being begun — a
    /// `Dequeue`, or in the wake of a `Dequeue` or a `CancelResearch` of
    /// what it needed.
    ResearchDropped { node: u32 },
    /// A crew member reached a level of its class (feature 74,
    /// `crate::class`): who, `Class`'s code, and the level. Said once a
    /// level; a pick level leaves a pick pending until
    /// `Command::PickTalent`.
    LevelUp { who: u32, class: u32, level: u32 },
    /// A crew member picked a talent: who, and `crate::class::Talent`'s
    /// code.
    TalentPicked { who: u32, talent: u32 },
    /// An engineer laid a kit: who, and `crate::deploy::DeployKind`'s code.
    Deployed { who: u32, kind: u32 },
    /// An engineer packed a deployable up into a kit: who, and the kind.
    PackedUp { who: u32, kind: u32 },
    /// A deployable was destroyed — sandbags shot to nothing, a sentry
    /// drained — by the kind.
    DeployableLost { kind: u32 },
    /// A piece of armour was repaired at the workbench — the armourer's
    /// session done — by `bims::combat::ArmourKind`'s code.
    Repaired { kind: u32 },
    /// A soldier braced, or stood easy again (feature 75): who, and
    /// which.
    Braced { who: u32, on: bool },
    /// A soldier threw a grenade: who.
    Thrown { who: u32 },
    /// A medic's heal beam linked to a crew member, or unlinked (feature
    /// 76): who, and the patient — `None` for the link broken, by the
    /// medic or by the world (out of range, out of sight, down).
    Beamed { who: u32, patient: Option<u32> },
    /// A medic triggered a surge: who.
    Surged { who: u32 },
    /// A tank stood as a wall, or stood down again (feature 77): who,
    /// and which.
    Bulwarked { who: u32, on: bool },
    /// A tank taunted: who.
    Taunted { who: u32 },
    /// A commander sent the squad, or released it (feature 78): who,
    /// and `crate::SquadKind`'s code — `u32::MAX` for the order called
    /// off.
    Squadded { who: u32, kind: u32 },
    /// A commander rallied: who.
    Rallied { who: u32 },
    /// A fresh wave of machines has landed at a droid-held station
    /// (feature 83): which station. Everybody's speed request goes
    /// back to 1x with it.
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
    /// A dead player's Bim bought back out of the pool at a mission's
    /// start, aboard with no gear.
    BoughtBack { who: u32 },
    /// A dead player's Bim the pool could not pay for: out until the next
    /// mission.
    StillOut { who: u32 },
    /// A bot died, gone for good, and the pool paid for it — as much of
    /// the penalty as it held.
    BotLost { who: u32, paid: economy::Money },
    /// A town the machines were attacking was left before it was held,
    /// and fell to them: an infested site like any other.
    TownFell { station: u32 },
    /// The host said that player has left the game.
    PlayerGone { slot: u32 },
}

/// Why a command did nothing.
///
/// Written out and never renumbered, like the events: a refusal deleted
/// leaves its code free (the helm's and the flight's, 5, 6, 8, 10, 13 and
/// 28–32, 78 and 83, the hire's bunk, 20, the execution's and the
/// plunder's, 26 and 27, and a walk through the test room's locked heads
/// door, 37, went with the old game in feature 104).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u32)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Refusal {
    /// Trading anywhere but at a station. Money only works at a dock — see
    /// `shipdesign::materials` for the rule and why.
    NotDocked = 1,
    /// Not the money for it.
    Unaffordable = 2,
    /// Nowhere aboard to stow it.
    NoRoomAboard = 3,
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
    /// A fetch, or a piece taken off, with no free cell in the pack.
    PackFull = 15,
    /// A stow with the class the thing counts against full up: the
    /// lockers for a piece of armour, the shelves for a bar of metal.
    /// [`Refusal::NoRoomAboard`] is the same wall met buying at the dock.
    NoRoom = 16,
    /// A stow of a broken piece. It is worth nothing in the hold and the
    /// room will not hand it over: discard it instead.
    Broken = 17,
    /// A loot of a body that is not one: the Bim is alive and awake — a
    /// crewmate who came round while the window was open — or there is
    /// no such Bim. Down, dead or out cold, is asked when the command
    /// lands, not when the window opened.
    NotDown = 18,
    /// A hire of somebody who is not a mercenary for hire — one of the
    /// station's own people, or nobody at all.
    NotForHire = 19,
    /// A buy or a sell by a player whose crew member is not at the
    /// station's trading desk — see `World::at_the_desk`.
    NotAtTheDesk = 21,
    /// A take of a key off a desk with none on it, or an unlock with no
    /// key of the node's tier in the crew's own desk — the other tier's
    /// key there opens nothing, and stays.
    NoKey = 22,
    /// Research, or an unlock, with no research desk aboard, or one that is
    /// not powered: the AI runs on it.
    NoResearchDesk = 23,
    /// Research of a node that cannot be queued: known, on the AI or
    /// queued already, or behind a lock still shut, itself or something it
    /// needs — and an unlock of a node with no lock, or one open already.
    NotResearchable = 24,
    /// A site for a part the crew do not know how to build yet — see
    /// `shipdesign::research`.
    NotResearched = 25,
    /// A sale at a station with nobody to buy: a derelict keeps no desk
    /// (`crate::station::market_kind`). A buy there is
    /// [`Refusal::NotSoldHere`] first, since it stocks nothing either.
    NoMarket = 33,
    /// A thing put on the workbench, taken off it or an upgrade begun
    /// with no workbench aboard, or with the crew member further than
    /// [`crate::data::REACH`] from it — see `World::workbench`.
    NoWorkbench = 34,
    /// An upgrade begun with the bench's two input slots not holding two
    /// of a kind at the same tier below three — or a thing put on the
    /// bench that would not pair with what is there: a different kind, a
    /// different tier, or a tier-three thing, which has nowhere to go.
    NoPair = 35,
    /// A thing put on the workbench or an input taken off it while the
    /// day's work is under way on the pair; the output waits in its slot
    /// and can be taken any time.
    BenchBusy = 36,
    /// A walk ordered somewhere there is no way to at all —
    /// `Command::Crew`, the room's `ORDER_NOWHERE`.
    NoWayThere = 38,
    /// A `Dequeue` of a node that is not on the research queue.
    NotQueued = 39,
    /// An upgrade begun at the workbench before the crew know how —
    /// `shipdesign::research::Node::Upgrades`, the tier-two node, not
    /// yet researched. Said before the bench is looked at, so a pair on
    /// it waits.
    NoUpgrades = 40,
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
    /// A pick at a level that is not a pick level — a fixed one, or none.
    NotAPickLevel = 48,
    /// A pick at a level the crew member has not reached.
    LevelNotReached = 49,
    /// A pick at a level already picked: a pick is never changed.
    AlreadyPicked = 50,
    /// Something an engineer's talent gates, asked for without the
    /// talent: a repair without *armourer*.
    NoTalent = 51,
    /// A pick by a crew member with no class.
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
    /// A beam or a surge by a crew member that is not a medic (feature
    /// 76).
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
    /// A surge before the medic's third level.
    NoSurgeYet = 65,
    /// A surge with the charge not full.
    NotCharged = 66,
    /// A surge with no patient linked.
    NotLinked = 67,
    /// A bulwark or a taunt by a crew member that is not a tank
    /// (feature 77).
    NotATank = 68,
    /// A taunt before the tank's third level.
    NoTauntYet = 69,
    /// A squad order or a rally by a crew member that is not a
    /// commander (feature 78).
    NotACommander = 70,
    /// A rally before the commander's third level.
    NoRallyYet = 71,
    /// A squad order with nobody of the squad in range of him.
    NoSquadInRange = 72,
    /// An attack with no enemy where the pointer was.
    NoEnemyThere = 73,
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
    /// A stow of a medkit or a bandage: those are everybody's charges
    /// (`class::Charge::everybody`), which come back into the pack on a
    /// cooldown of their own — one put in the hold would be one more
    /// every minute for nothing, so they stay where they are.
    ChargeKept = 81,
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
            WorldEvent::Traded { units, .. } if units >= 0 => 8,
            WorldEvent::Traded { .. } => 9,
            WorldEvent::Refused { .. } => 10,
            WorldEvent::Crafted { .. } => 14,
            WorldEvent::CraftLost { .. } => 15,
            // 1 to 5 and 11 to 13 are free: flight's (feature 104). 16 was
            // `Mined`, which went with the mining (feature 95), and 17 to
            // 26 the radiation dose's (feature 104).
            WorldEvent::SitePlaced { .. } => 27,
            WorldEvent::SiteCancelled { .. } => 28,
            WorldEvent::Built { .. } => 29,
            WorldEvent::BuildLost { .. } => 30,
            WorldEvent::EnemyDown { .. } => 31,
            WorldEvent::CrewHit { .. } => 32,
            WorldEvent::CrewDown { .. } => 33,
            WorldEvent::Equipped { .. } => 34,
            WorldEvent::Stowed { .. } => 35,
            WorldEvent::PieceBroke { .. } => 36,
            WorldEvent::Locked { .. } => 37,
            WorldEvent::Looted { .. } => 38,
            WorldEvent::Hired { .. } => 39,
            WorldEvent::MercenaryPaid { .. } => 40,
            WorldEvent::MercenaryLeft { .. } => 41,
            WorldEvent::CrewDying { .. } => 42,
            WorldEvent::CrewTreated { .. } => 43,
            WorldEvent::KeyTaken { .. } => 44,
            WorldEvent::Unlocked { .. } => 45,
            WorldEvent::ResearchBegun { .. } => 46,
            WorldEvent::Researched { .. } => 47,
            WorldEvent::UpgradeBegun { .. } => 48,
            WorldEvent::Upgraded { .. } => 49,
            // 50 and 52 to 55 were the charge and the landing (feature 104).
            WorldEvent::Jumped { .. } => 51,
            WorldEvent::Brownout => 56,
            WorldEvent::PowerRestored => 57,
            // 58 was the food spoiling, 59 to 62 and 67 the raids and 64
            // the plunder (feature 104).
            WorldEvent::CrewLost => 63,
            WorldEvent::ResearchQueued { .. } => 65,
            WorldEvent::ResearchDropped { .. } => 66,
            WorldEvent::LevelUp { .. } => 68,
            WorldEvent::TalentPicked { .. } => 69,
            WorldEvent::Deployed { .. } => 70,
            WorldEvent::PackedUp { .. } => 71,
            WorldEvent::DeployableLost { .. } => 72,
            WorldEvent::Repaired { .. } => 74,
            WorldEvent::Braced { .. } => 75,
            WorldEvent::Thrown { .. } => 76,
            WorldEvent::Beamed { .. } => 77,
            WorldEvent::Surged { .. } => 78,
            WorldEvent::Bulwarked { .. } => 79,
            WorldEvent::Taunted { .. } => 80,
            WorldEvent::Squadded { .. } => 81,
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
            WorldEvent::BoughtBack { .. } => 102,
            WorldEvent::StillOut { .. } => 103,
            WorldEvent::BotLost { .. } => 104,
            WorldEvent::TownFell { .. } => 105,
            WorldEvent::PlayerGone { .. } => 106,
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
            WorldEvent::LeftBehind { who }
            | WorldEvent::BoughtBack { who }
            | WorldEvent::StillOut { who } => who as i64,
            // The penalty paid in the hundreds: a crew is never a hundred.
            WorldEvent::BotLost { who, paid } => (who as i64) + 100 * (paid as i64),
            WorldEvent::TownFell { station } => station as i64,
            WorldEvent::Crafted { recipe } | WorldEvent::CraftLost { recipe } => recipe as i64,
            // The station in the thousands, the person in the units.
            WorldEvent::EnemyDown { station, who } => (who + 1_000 * station) as i64,
            // The kind in the hundreds and the body under it, the way
            // `CrewHit` packs a part with a crew member. The station is
            // left out: the log says which machine, not whose.
            WorldEvent::DroidDown { who, kind, .. } => (who + 100 * kind) as i64,
            // The order's code in the hundreds, nought being the
            // following every slot starts on.
            WorldEvent::Ordered { who, kind } => (who as i64) + 100 * (kind as i64),
            // The part in the tens, the person in the units: three parts,
            // and a crew is never ten.
            WorldEvent::CrewHit { who, part } => (who + 10 * part) as i64,
            // The trauma in the tens the same way: ten of them, and a crew
            // is never ten.
            WorldEvent::CrewDying { who, trauma } | WorldEvent::CrewTreated { who, trauma } => {
                (who + 10 * trauma) as i64
            }
            // The kind in the tens the same way: three kinds, and a crew
            // is never ten.
            WorldEvent::Equipped { who, kind } | WorldEvent::PieceBroke { who, kind } => {
                (who + 10 * kind.code()) as i64
            }
            // The source's kind in the tens, likewise: two kinds.
            WorldEvent::Looted { who, source_kind } => (who + 10 * source_kind) as i64,
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
            | WorldEvent::MercenaryLeft { who }
            | WorldEvent::KeyTaken { who }
            | WorldEvent::Stowed { who } => who as i64,
            WorldEvent::Unlocked { node }
            | WorldEvent::ResearchBegun { node }
            | WorldEvent::Researched { node }
            | WorldEvent::ResearchQueued { node }
            | WorldEvent::ResearchDropped { node } => node as i64,
            // The tier in the hundreds: under a hundred resources, and a
            // tier is never a hundred.
            WorldEvent::UpgradeBegun { resource, tier }
            | WorldEvent::Upgraded { resource, tier } => (resource + 100 * tier) as i64,
            WorldEvent::SitePlaced { kind, .. }
            | WorldEvent::SiteCancelled { kind }
            | WorldEvent::Built { kind }
            | WorldEvent::BuildLost { kind } => kind.code() as i64,
            WorldEvent::Discovered { node } => match node {
                Node::Body(id) | Node::Station(id) => id as i64,
            },
            WorldEvent::FrameChanged { frame } => frame.code() as i64,
            WorldEvent::Traded { units, .. } => units,
            WorldEvent::Refused { why, .. } => why.code() as i64,
            // The star: a galaxy has a thousand.
            WorldEvent::Jumped { star } | WorldEvent::Infested { star } => star as i64,
            WorldEvent::Brownout | WorldEvent::PowerRestored | WorldEvent::CrewLost => 0,
            // The level, the talent and the kind in the hundreds, the same
            // way: a crew is never a hundred.
            WorldEvent::LevelUp { who, level, .. } => (who as i64) + 100 * (level as i64),
            WorldEvent::TalentPicked { who, talent } => (who as i64) + 100 * (talent as i64),
            WorldEvent::Deployed { who, kind } | WorldEvent::PackedUp { who, kind } => {
                (who as i64) + 100 * (kind as i64)
            }
            WorldEvent::DeployableLost { kind } | WorldEvent::Repaired { kind } => kind as i64,
            WorldEvent::Thrown { who }
            | WorldEvent::Surged { who }
            | WorldEvent::Taunted { who }
            | WorldEvent::Rallied { who } => who as i64,
            // The order's code in the hundreds, and nought for the
            // order called off: a crew is never a hundred.
            WorldEvent::Squadded { who, kind } => {
                (who as i64) + 100 * if kind == u32::MAX { 0 } else { kind as i64 + 1 }
            }
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
