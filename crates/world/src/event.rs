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
//! A caller that wants to draw a fuel gauge reads the state. A caller that
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
    /// A crew member is down: dead, or their health at nought. Said the
    /// step it happens, whatever did it — a shot, blood lost to a wound
    /// nobody dressed, or the room's own hunger.
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
    /// A hired mercenary's month came round and was paid, `fee` euros.
    MercenaryPaid { who: u32, fee: u64 },
    /// A hired mercenary went unpaid — the month came round and the
    /// crew's money would not cover it — and left at the dock, or is
    /// waiting to. Said once a month owed.
    MercenaryLeft { who: u32 },
}

/// Why a command did nothing.
///
/// Separate from [`PlanError`] on purpose: these are about *whether the
/// command was allowed*, and those are about whether the trip could be flown.
/// A player who is told "not enough fuel" when what actually happened is
/// "you are not docked" will go and buy fuel.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u32)]
pub enum Refusal {
    /// Trading anywhere but at a station. Money only works at a dock — see
    /// `shipdesign::materials` for the rule and why.
    NotDocked = 1,
    /// Not the money for it.
    Unaffordable = 2,
    /// Nowhere aboard to stow it.
    NoRoomAboard = 3,
    /// Selling more than is aboard, or more than is not spoken for: fuel held
    /// against a trip under way is not fuel anybody may sell. And, since the
    /// pack, the thing asked for not being there at all: a fetch of a piece
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
            WorldEvent::Docking { station } => station as i64,
            WorldEvent::Crafted { recipe } | WorldEvent::CraftLost { recipe } => recipe as i64,
            WorldEvent::Mined { rock, ore, galvum } => {
                (rock + 1_000 * ore + 1_000_000 * galvum) as i64
            }
            // The station in the thousands, the person in the units.
            WorldEvent::EnemyDown { station, who } => (who + 1_000 * station) as i64,
            // The part in the tens, the person in the units: three parts,
            // and a crew is never ten.
            WorldEvent::CrewHit { who, part } => (who + 10 * part) as i64,
            // The kind in the tens the same way: three kinds, and a crew
            // is never ten.
            WorldEvent::Equipped { who, kind } | WorldEvent::PieceBroke { who, kind } => {
                (who + 10 * kind.code()) as i64
            }
            // The source's kind in the tens, likewise: two kinds.
            WorldEvent::Looted { who, source_kind } => (who + 10 * source_kind) as i64,
            // The fee in the hundreds: a crew is never a hundred.
            WorldEvent::MercenaryPaid { who, fee } => (who as i64) + 100 * (fee as i64),
            WorldEvent::Health { who, .. }
            | WorldEvent::CrewDown { who }
            | WorldEvent::Locked { who }
            | WorldEvent::Hired { who }
            | WorldEvent::MercenaryLeft { who }
            | WorldEvent::Stowed { who } => who as i64,
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
        }
    }
}
