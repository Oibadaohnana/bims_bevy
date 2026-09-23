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
//! contain. `crates/health` is built on the same distinction and its note says
//! the same thing at more length.

use bims::combat::ArmourKind;
use flight::PlanError;
use physics::ResourceId;
use shipdesign::PartKind;
use worldgen::Node;

use crate::frame::Frame;

/// One thing that happened, in the order it happened.
#[derive(Clone, Copy, PartialEq, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum WorldEvent {
    /// A trip began. Undocking, if it began from a station.
    Departed { slot: u32 },
    /// A trip ended. `station` is `Some` when the ship actually docked, which
    /// wants an airlock as well as a station to aim at.
    Arrived { station: Option<u32> },
    /// A trip was given up. The ship is stopping, not stopped.
    Aborted { slot: u32 },
    /// A destination could not be flown to. Carries the reason, which is the
    /// same `PlanError` a preview would have shown.
    PlanFailed { slot: u32, error: PlanError },
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
    /// A trip was confirmed at a station: the station's people are going
    /// ashore and the crew coming back aboard, and the ship will cast off
    /// once they have.
    CastingOff { slot: u32 },
    /// Everybody is where they belong and the ship is pushing off the
    /// berth. The trip itself is planned once it is clear.
    Undocking { slot: u32 },
    /// A trip has ended at a station's door and the ship is coming
    /// alongside. `Arrived` follows once it is tied up.
    Docking { station: u32 },
    /// A Bim finished a recipe at a bench and the cargo moved:
    /// `shipdesign::recipes::RECIPES[recipe]`'s inputs out, its output in.
    Crafted { recipe: u32 },
    /// A Bim finished a recipe and nothing was made: the inputs had gone
    /// from the hold, or there was no longer room for the output. The
    /// labour is lost, and this says so.
    CraftLost { recipe: u32 },
    /// A walk outside came back with so much rock, ore and galvum on the
    /// shelf. Nought of all three means it mined nothing, or the shelves
    /// were full.
    Mined { rock: u32, ore: u32, galvum: u32 },
    /// A crew member's body crossed a line — see `health::HealthEvent`.
    /// One code per health event, `who` as the value.
    Health {
        who: u32,
        event: health::HealthEvent,
    },
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
    /// One of a hostile station's people went down under the crew's fire.
    /// See `bims::combat`, and `World::visit`, which is where a hit
    /// crosses from the crew's room to theirs.
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
    /// dead or unconscious crewmate's, or a hostile station's person's,
    /// `source_kind` being `crate::armour::LootSource::code`. It is in
    /// `who`'s pack now; a piece of armour keeps the health it had.
    Looted { who: u32, source_kind: u32 },
    /// A mercenary was hired — `Command::Hire` — and is crew member `who`
    /// now, the first month paid. See `crate::mercenary`.
    Hired { who: u32 },
    /// A crew member finished off one of a hostile station's people
    /// lying out cold — `Command::Execute` — and it is dead. `who` did it,
    /// `resident` the body.
    Executed { who: u32, resident: u32 },
    /// A hired mercenary's month came round and was paid, `fee` euros.
    MercenaryPaid { who: u32, fee: u64 },
    /// A hired mercenary went unpaid — the month came round and the
    /// crew's money would not cover it — and left at the dock, or is
    /// waiting to. Said once a month owed.
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
    /// The hyperdrive began charging for a jump to `star`, on `slot`'s
    /// order — see `crate::jump`.
    Charging { slot: u32, star: u32 },
    /// The ship is in another system: the one round `star`, in empty space.
    Jumped { star: u32 },
    /// The charge ran out with no drive to fire: it was taken off or
    /// browned out in the meantime. The ship is where it was, holding.
    JumpFailed,
    /// The ship is coming down onto `body`, on the helm's Land — see
    /// `crate::surface`: over the planet first, then down onto the pad.
    Landing { body: u32 },
    /// And down: tied up at the settlement on `body`, the rooms joined,
    /// the planet the new surroundings.
    Landed { body: u32 },
    /// Off the pad on `body` and climbing, on the way to where a trip to
    /// the planet would have ended; the trip is planned from there.
    LiftedOff { body: u32 },
    /// The batteries went flat under an overdraw and the ship is browned
    /// out — see `World::run_brownout`: the lamps are dark, the bay and
    /// the benches have stopped, and the cold store's food is spoiling.
    /// Said once, the step it starts.
    Brownout,
    /// The brownout is over: the reactors cover the draw again, or a
    /// battery has something in it. What stopped is running again and
    /// what is left in the cold store stays.
    PowerRestored,
    /// An hour of the cold store without power took `units` off the
    /// shelf — vegetables, tofu and stew together, a share of each.
    FoodSpoiled { units: u32 },
    /// A raider is on the radar, closing on the ship with `boarders`
    /// aboard, `minutes` out — see `crate::raid`. Every player's speed
    /// request was put back to 1× with it, once.
    RaidContact { boarders: u32, minutes: u32 },
    /// The raider is tied to the ship, its `boarders` posted at the
    /// ship's airlock — locked against them the same step
    /// (`World::seal_against_raid`) — and forcing it.
    RaidBoarded { boarders: u32 },
    /// The ship's airlock gave — or the crew opened it — and the raider's
    /// `boarders` are coming through. Said once a raid.
    RaidBreached { boarders: u32 },
    /// The raider arrived to find the ship gone — under way, or in
    /// another system — and the raid is off.
    RaidCancelled,
    /// Every boarder is down: the raider is a derelict tied to the ship,
    /// to be looted and cast off from.
    RaidRepelled,
    /// No crew member is standing — dead or out cold, every one — and
    /// the run is over. Said once.
    CrewLost,
    /// Crew member `who` took `units` of a stack off an enemy's shelf into
    /// the pack — a raider's or a hostile station's; see `crate::plunder`.
    Plundered { who: u32, units: u32 },
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
    /// back to 1x with it, the way raid contact does.
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
}

/// Why a command did nothing.
///
/// Separate from [`PlanError`] on purpose: these are about *whether the
/// command was allowed*, and those are about whether the trip could be flown.
/// A player who is told "no forward engine" when what actually happened is
/// "you are not at the helm" will go and build one.
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
    /// That player's crew member is not standing at the helm, and the ship
    /// is flown from the helm — see [`crate::World::can_command`].
    NotAtTheHelm = 5,
    /// An abort with nothing to abort.
    NotTravelling = 6,
    /// The sum would not fit in a `Money`. A refusal rather than a wrap, the
    /// same as everywhere else money is added up — see `crates/economy`.
    SumTooBig = 7,
    /// A Confirm while the ship is coming alongside. It is neither at rest
    /// nor on a trip that can be stopped: wait until it is tied up.
    ComingAlongside = 8,
    /// The station the ship is docked at does not sell that —
    /// `worldgen::StationKind::sells`. Galvum is the outposts' alone, an
    /// emitter is nobody's, and a derelict has nobody to sell anything.
    NotSoldHere = 9,
    /// A construction site laid out while the ship is not at rest. Nothing
    /// is built on a ship that is moving — see `crate::build`.
    UnderWay = 10,
    /// A site the rules would not put there, or that would leave the ship
    /// with a fault it has not got. `World::can_place_site` says which,
    /// before anything is sent.
    WontFit = 11,
    /// A site that is not there: built, or cancelled already.
    NoSuchSite = 12,
    /// A Confirm while something is being built. The ship does not move
    /// while it is built on: cancel the site, or let them finish.
    UnderConstruction = 13,
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
    /// No bunk aboard for one more: the crew sleep a bunk each, and a
    /// hire wants a free one.
    NoBunk = 20,
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
    /// An execution at a station that is not an enemy's: a downed
    /// crewmate, a friend's or a stranger's people are never finished off.
    /// And a take off the shelf of one: a friend's or a stranger's shelf is
    /// bought from across the desk, never plundered (`crate::plunder`).
    NotHostile = 26,
    /// An execution by a crew member with nothing in its hand.
    Unarmed = 27,
    /// A jump with no working hyperdrive: none aboard, none bolted to an
    /// engine, or none on a live network — `shipdesign::hyperdrive::ready`.
    NoHyperdrive = 28,
    /// A jump from anywhere but a hold: docked, the rooms are joined and
    /// the station's people are aboard; under way, the ship is flying.
    NotHolding = 29,
    /// A jump to a star the galaxy has not got.
    NoSuchStar = 30,
    /// A jump to the star the ship is already at.
    SameStar = 31,
    /// A Land from anywhere but a hold in the frame of a planet with
    /// ground on it — a rocky planet's or an ice world's (`crate::surface`).
    NoPlanetHere = 32,
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
    /// A walk ordered somewhere the only way to is through a locked
    /// door — `Command::Crew`, the room's `ORDER_LOCKED`.
    DoorLocked = 37,
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
    /// an enemy.
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
    /// A jump to a star the hyperlanes do not join to this one (feature
    /// 93). A charge is one hop, and only down a lane: the chart draws
    /// the route, and the Jump button charges for its first step.
    NoLane = 78,
    /// A jump **inward** — to a star fewer hops from the machines' origin
    /// than this one — out of an infested system whose jammer still
    /// stands (feature 93, `crate::jammer`). Sideways and outward are
    /// accepted, and flying *into* an infested system never is refused.
    Jammed = 79,
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
            WorldEvent::Departed { .. } => 1,
            WorldEvent::Arrived { station: Some(_) } => 2,
            WorldEvent::Arrived { station: None } => 3,
            WorldEvent::Aborted { .. } => 4,
            WorldEvent::PlanFailed { .. } => 5,
            WorldEvent::Discovered { .. } => 6,
            WorldEvent::FrameChanged { .. } => 7,
            WorldEvent::Traded { units, .. } if units >= 0 => 8,
            WorldEvent::Traded { .. } => 9,
            WorldEvent::Refused { .. } => 10,
            WorldEvent::CastingOff { .. } => 11,
            WorldEvent::Undocking { .. } => 12,
            WorldEvent::Docking { .. } => 13,
            WorldEvent::Crafted { .. } => 14,
            WorldEvent::CraftLost { .. } => 15,
            WorldEvent::Mined { .. } => 16,
            // 17 to 26: `HealthEvent` runs 1 to 10.
            WorldEvent::Health { event, .. } => 16 + event.code(),
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
            WorldEvent::Executed { .. } => 48,
            WorldEvent::Charging { .. } => 50,
            WorldEvent::Jumped { .. } => 51,
            WorldEvent::JumpFailed => 52,
            WorldEvent::Landing { .. } => 53,
            WorldEvent::Landed { .. } => 54,
            WorldEvent::LiftedOff { .. } => 55,
            WorldEvent::Brownout => 56,
            WorldEvent::PowerRestored => 57,
            WorldEvent::FoodSpoiled { .. } => 58,
            WorldEvent::RaidContact { .. } => 59,
            WorldEvent::RaidBoarded { .. } => 60,
            WorldEvent::RaidCancelled => 61,
            WorldEvent::RaidRepelled => 62,
            WorldEvent::CrewLost => 63,
            WorldEvent::Plundered { .. } => 64,
            WorldEvent::ResearchQueued { .. } => 65,
            WorldEvent::ResearchDropped { .. } => 66,
            WorldEvent::RaidBreached { .. } => 67,
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
        }
    }

    /// The one number the sentence needs, if it needs one: a station id, a
    /// reason code, a node id, a number of units. The host knows which of
    /// those it is from the code, exactly as `MEMORY_LINES` does.
    pub fn value(self) -> i64 {
        match self {
            WorldEvent::Departed { slot }
            | WorldEvent::Aborted { slot }
            | WorldEvent::CastingOff { slot }
            | WorldEvent::Undocking { slot } => slot as i64,
            WorldEvent::Docking { station }
            | WorldEvent::DroidReinforcements { station }
            | WorldEvent::DroidStationCleared { station }
            | WorldEvent::TownHeld { station } => station as i64,
            WorldEvent::TownsfolkJoined { count } => count as i64,
            WorldEvent::Crafted { recipe } | WorldEvent::CraftLost { recipe } => recipe as i64,
            WorldEvent::Mined { rock, ore, galvum } => {
                (rock + 1_000 * ore + 1_000_000 * galvum) as i64
            }
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
            // The body in the tens, likewise.
            WorldEvent::Executed { who, resident } => (who + 10 * resident) as i64,
            // The one carried in the **hundreds**, the carrier in the
            // units, and the carrier alone for a set down, whose line
            // names nobody else (feature 86). The hundreds rather than
            // the tens the rows above use: the `combat` command sails
            // with fourteen.
            WorldEvent::Carried { who, patient } => (who + 100 * patient.unwrap_or(0)) as i64,
            // The fee in the hundreds: a crew is never a hundred.
            WorldEvent::MercenaryPaid { who, fee } => (who as i64) + 100 * (fee as i64),
            WorldEvent::Health { who, .. }
            | WorldEvent::CrewDown { who }
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
            WorldEvent::Arrived { station } => station.map(i64::from).unwrap_or(-1),
            WorldEvent::PlanFailed { error, .. } => error.code() as i64,
            WorldEvent::Discovered { node } => match node {
                Node::Body(id) | Node::Station(id) => id as i64,
            },
            WorldEvent::FrameChanged { frame } => frame.code() as i64,
            WorldEvent::Traded { units, .. } => units,
            WorldEvent::Refused { why, .. } => why.code() as i64,
            // The star: a galaxy has a thousand, and a slot is never that.
            WorldEvent::Charging { star, .. }
            | WorldEvent::Jumped { star }
            | WorldEvent::Infested { star } => star as i64,
            WorldEvent::JumpFailed => 0,
            // The planet: a body id.
            WorldEvent::Landing { body }
            | WorldEvent::Landed { body }
            | WorldEvent::LiftedOff { body } => body as i64,
            WorldEvent::Brownout | WorldEvent::PowerRestored => 0,
            WorldEvent::FoodSpoiled { units } => units as i64,
            // The minutes out in the hundreds: boarders are never a hundred.
            WorldEvent::RaidContact { boarders, minutes } => {
                (boarders as i64) + 100 * (minutes as i64)
            }
            WorldEvent::RaidBoarded { boarders } | WorldEvent::RaidBreached { boarders } => {
                boarders as i64
            }
            WorldEvent::RaidCancelled | WorldEvent::RaidRepelled | WorldEvent::CrewLost => 0,
            // The units in the hundreds: a crew is never a hundred, and a
            // pack has fifty cells.
            WorldEvent::Plundered { who, units } => (who as i64) + 100 * (units as i64),
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
